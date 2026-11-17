//! The Android app's ties to the system, through JNI where only Java can tell: the parts of
//! the screen under the status bar, the navigation bar and a camera cutout (NativeActivity
//! draws under them), the user's languages, the back button, and the keys' vibration.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{Context, Event, Key, RawInput, SafeAreaInsets, epaint::MarginF32};
use jni::JNIEnv;
use jni::objects::{JObject, JString, JValue};
use winit::platform::android::activity::AndroidApp;

/// The app's activity, which `android_main` gets from Android.
static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Keeps the activity for the JNI calls (`android_main` calls this first).
pub(crate) fn init(app: &AndroidApp) {
    APP.set(app.clone()).ok();
}

/// Runs `f` with this thread's JNI environment and the app's activity, in a local reference
/// frame: the app's thread stays attached to Java, so its local references would pile up.
/// The first failure goes to the log, with the Java exception if there is one.
fn with_activity<T>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    static FAILED: AtomicBool = AtomicBool::new(false);
    let app = APP.get()?;
    // SAFETY: android-activity's JavaVM and activity live as long as the app. The activity
    // (the NativeActivity, not ndk-context's Application) is a global reference that
    // android-activity owns, and a `JObject` never deletes it.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread_permanently().ok()?;
    let result = env.with_local_frame(32, |env| f(env, &activity));
    let first = result.is_err() && !FAILED.swap(true, Ordering::Relaxed);
    // A Java exception would stay pending and break the next JNI call.
    if env.exception_check().unwrap_or(false) {
        if first {
            env.exception_describe().ok();
        }
        env.exception_clear().ok();
    }
    result
        .inspect_err(|error| {
            if first {
                log::warn!("JNI call failed: {error}");
            }
        })
        .ok()
}

/// Android's version (`Build.VERSION.SDK_INT`).
fn sdk_version(env: &mut JNIEnv) -> jni::errors::Result<i32> {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()
}

/// The activity window's top view (`getWindow().getDecorView()`).
fn decor_view<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject,
) -> jni::errors::Result<JObject<'local>> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
        .l()
}

/// `HapticFeedbackConstants.KEYBOARD_TAP`: the system keyboard's key tap.
const KEYBOARD_TAP: i32 = 3;

/// A light tap of the vibration motor, as the system keyboard gives for a key: Android tunes
/// it for each phone, and leaves it out when the user has turned touch feedback off.
pub(super) fn key_tap() {
    with_activity(|env, activity| {
        let decor = decor_view(env, activity)?;
        env.call_method(
            &decor,
            "performHapticFeedback",
            "(I)Z",
            &[JValue::Int(KEYBOARD_TAP)],
        )?;
        Ok(())
    });
}

/// How far the system bars and a display cutout reach into the window, in pixels (left, top,
/// right, bottom). NativeActivity draws on the whole window's surface, under the bars, on
/// every Android version, whatever space its content view leaves them.
fn covered_edges() -> Option<[i32; 4]> {
    with_activity(|env, activity| {
        let decor = decor_view(env, activity)?;
        let insets = env
            .call_method(
                &decor,
                "getRootWindowInsets",
                "()Landroid/view/WindowInsets;",
                &[],
            )?
            .l()?;
        if insets.is_null() {
            return Ok(None);
        }
        let insets = if sdk_version(env)? >= 30 {
            let kind = |env: &mut JNIEnv, name| {
                env.call_static_method("android/view/WindowInsets$Type", name, "()I", &[])?
                    .i()
            };
            let kinds = kind(env, "systemBars")? | kind(env, "displayCutout")?;
            let edges = env
                .call_method(
                    &insets,
                    "getInsets",
                    "(I)Landroid/graphics/Insets;",
                    &[JValue::Int(kinds)],
                )?
                .l()?;
            let mut edge = |name| env.get_field(&edges, name, "I")?.i();
            [edge("left")?, edge("top")?, edge("right")?, edge("bottom")?]
        } else {
            let mut edge = |name| env.call_method(&insets, name, "()I", &[])?.i();
            [
                edge("getSystemWindowInsetLeft")?,
                edge("getSystemWindowInsetTop")?,
                edge("getSystemWindowInsetRight")?,
                edge("getSystemWindowInsetBottom")?,
            ]
        };
        Ok(Some(insets))
    })
    .flatten()
}

/// Called with every frame's input: tells egui which parts of the screen the system bars
/// cover (egui's safe area, which the layout keeps clear), and handles the back button.
/// While the app has something to close (`back_closes`: a menu, popup, flyout or the
/// Settings page), back is Esc, which closes it; otherwise it leaves the app, as on the first
/// screen of any Android app (Esc would clear the calculator, as in Windows).
pub(super) fn input_hook(ctx: &Context, raw_input: &mut RawInput, back_closes: bool) {
    if let Some([left, top, right, bottom]) = covered_edges() {
        let pixels_per_point =
            raw_input.viewport().native_pixels_per_point.unwrap_or(1.0) * ctx.zoom_factor();
        let points = |pixels: i32| pixels as f32 / pixels_per_point;
        raw_input.safe_area_insets = Some(SafeAreaInsets(MarginF32 {
            left: points(left),
            right: points(right),
            top: points(top),
            bottom: points(bottom),
        }));
    }
    let mut leave = false;
    raw_input.events.retain_mut(|event| match event {
        Event::Key { key, pressed, .. } if *key == Key::BrowserBack => {
            if back_closes {
                *key = Key::Escape;
                return true;
            }
            leave |= *pressed;
            false
        }
        _ => true,
    });
    if leave {
        move_task_to_back();
    }
}

/// Sends the app to the background, as the back button does on an app's first screen.
fn move_task_to_back() {
    with_activity(|env, activity| {
        env.call_method(activity, "moveTaskToBack", "(Z)Z", &[JValue::Bool(1)])?;
        Ok(())
    });
}

/// The user's languages, most preferred first (`LocaleList`, or the default `Locale` before
/// Android 7).
pub(super) fn languages() -> Vec<String> {
    with_activity(|env, _| {
        let tags = if sdk_version(env)? >= 24 {
            let list = env
                .call_static_method(
                    "android/os/LocaleList",
                    "getDefault",
                    "()Landroid/os/LocaleList;",
                    &[],
                )?
                .l()?;
            env.call_method(&list, "toLanguageTags", "()Ljava/lang/String;", &[])?
                .l()?
        } else {
            let locale = env
                .call_static_method(
                    "java/util/Locale",
                    "getDefault",
                    "()Ljava/util/Locale;",
                    &[],
                )?
                .l()?;
            env.call_method(&locale, "toLanguageTag", "()Ljava/lang/String;", &[])?
                .l()?
        };
        let tags: String = env.get_string(&JString::from(tags))?.into();
        Ok(tags.split(',').map(str::to_string).collect())
    })
    .unwrap_or_default()
}

/// `Intent.FLAG_ACTIVITY_NEW_TASK`: harmless from an activity, needed from any other context.
const FLAG_ACTIVITY_NEW_TASK: i32 = 0x1000_0000;

/// A new `Intent` with `action`, as a local reference.
fn new_intent<'local>(
    env: &mut JNIEnv<'local>,
    action: &str,
) -> jni::errors::Result<JObject<'local>> {
    let action = env.new_string(action)?;
    env.new_object(
        "android/content/Intent",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&action)],
    )
}

/// Starts `intent` from the activity, in a new task.
fn start_activity(
    env: &mut JNIEnv,
    activity: &JObject,
    intent: &JObject,
) -> jni::errors::Result<()> {
    env.call_method(
        intent,
        "addFlags",
        "(I)Landroid/content/Intent;",
        &[JValue::Int(FLAG_ACTIVITY_NEW_TASK)],
    )?;
    env.call_method(
        activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(intent)],
    )?;
    Ok(())
}

/// Opens `url` in the default browser (`ACTION_VIEW`). egui's `open_url` does nothing here:
/// eframe's Android build has no `webbrowser`. Returns whether the intent started.
pub(crate) fn open_url(url: &str) -> bool {
    with_activity(|env, activity| {
        let text = env.new_string(url)?;
        let uri = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            )?
            .l()?;
        let intent = new_intent(env, "android.intent.action.VIEW")?;
        env.call_method(
            &intent,
            "setData",
            "(Landroid/net/Uri;)Landroid/content/Intent;",
            &[JValue::Object(&uri)],
        )?;
        start_activity(env, activity, &intent)
    })
    .is_some()
}

/// Opens the system share sheet with `text` (`ACTION_SEND`, `text/plain`, in a chooser).
/// Returns whether it started.
pub(crate) fn share_sheet(text: &str) -> bool {
    with_activity(|env, activity| {
        let send = new_intent(env, "android.intent.action.SEND")?;
        let mime = env.new_string("text/plain")?;
        env.call_method(
            &send,
            "setType",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&mime)],
        )?;
        let key = env.new_string("android.intent.extra.TEXT")?;
        let body = env.new_string(text)?;
        env.call_method(
            &send,
            "putExtra",
            "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&key), JValue::Object(&body)],
        )?;
        let chooser = env
            .call_static_method(
                "android/content/Intent",
                "createChooser",
                "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
                &[JValue::Object(&send), JValue::Object(&JObject::null())],
            )?
            .l()?;
        start_activity(env, activity, &chooser)
    })
    .is_some()
}

/// Puts `text` on the Android clipboard (`ClipboardManager.setPrimaryClip`). egui-winit's
/// clipboard is compiled out for Android, so `Context::copy_text` never reaches the system.
/// Returns whether it was set.
pub(crate) fn copy_text(text: &str) -> bool {
    with_activity(|env, activity| {
        let service = env.new_string("clipboard")?;
        let manager = env
            .call_method(
                activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&service)],
            )?
            .l()?;
        if manager.is_null() {
            return Ok(false);
        }
        let label = env.new_string("CalcCraft")?;
        let body = env.new_string(text)?;
        let clip = env
            .call_static_method(
                "android/content/ClipData",
                "newPlainText",
                "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
                &[JValue::Object(&label), JValue::Object(&body)],
            )?
            .l()?;
        env.call_method(
            &manager,
            "setPrimaryClip",
            "(Landroid/content/ClipData;)V",
            &[JValue::Object(&clip)],
        )?;
        Ok(true)
    })
    .unwrap_or(false)
}
