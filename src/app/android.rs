//! The Android app's ties to the system, through JNI where only Java can tell: the parts of
//! the screen under the status bar, the navigation bar and a camera cutout (NativeActivity
//! draws under them), the user's languages, the back button, and the keys' vibration.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{Context, Event, Key, RawInput, SafeAreaInsets, epaint::MarginF32};
use jni::JNIEnv;
use jni::objects::{JObject, JString, JValue};
use winit::platform::android::activity::AndroidApp;

use super::vibration;

/// The app's activity, which `android_main` gets from Android.
static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Keeps the activity for the JNI calls (`android_main` calls this first).
pub(crate) fn init(app: &AndroidApp) {
    APP.set(app.clone()).ok();
}

/// Runs `f` with this thread's JNI environment and the app's activity, in a local reference
/// frame: the app's thread stays attached to Java, so its local references would pile up.
/// The first failure goes to the log, with the Java exception if there is one.
pub(super) fn with_activity<T>(
    f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>,
) -> Option<T> {
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
pub(super) fn sdk_version(env: &mut JNIEnv) -> jni::errors::Result<i32> {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()
}

/// The activity window's top view (`getWindow().getDecorView()`).
pub(super) fn decor_view<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject,
) -> jni::errors::Result<JObject<'local>> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
        .l()
}

/// `VibrationEffect.Composition.PRIMITIVE_CLICK`: the phone maker's crisp click waveform.
const PRIMITIVE_CLICK: i32 = 1;
/// `VibrationEffect.DEFAULT_AMPLITUDE`, for vibrators without amplitude control.
const DEFAULT_AMPLITUDE: i32 = -1;

/// How this phone's vibrator can tap, best first (see `vibration`'s mappings).
#[derive(Clone, Copy, Debug)]
enum TapKind {
    /// A `PRIMITIVE_CLICK` composition with a scale (Android 11+, where the vibrator has it).
    Click,
    /// A short one-shot with an amplitude (`hasAmplitudeControl`, Android 8+).
    Amplitude,
    /// A one-shot whose duration is all there is (an on/off motor, or before Android 8).
    Duration,
}

/// The phone's default vibrator (`VibratorManager.getDefaultVibrator` from Android 12, the
/// `vibrator` service before), or `None` when it has no vibrator.
pub(super) fn vibrator<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject,
    sdk: i32,
) -> jni::errors::Result<Option<JObject<'local>>> {
    let service = |env: &mut JNIEnv<'local>, name: &str| {
        let name = env.new_string(name)?;
        env.call_method(
            activity,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&name)],
        )?
        .l()
    };
    let vibrator = if sdk >= 31 {
        let manager = service(env, "vibrator_manager")?;
        if manager.is_null() {
            return Ok(None);
        }
        env.call_method(
            &manager,
            "getDefaultVibrator",
            "()Landroid/os/Vibrator;",
            &[],
        )?
        .l()?
    } else {
        service(env, "vibrator")?
    };
    if vibrator.is_null() || !env.call_method(&vibrator, "hasVibrator", "()Z", &[])?.z()? {
        return Ok(None);
    }
    Ok(Some(vibrator))
}

/// What `vibrator` can play, asked once: it doesn't change while the app runs.
fn tap_kind(env: &mut JNIEnv, vibrator: &JObject, sdk: i32) -> jni::errors::Result<TapKind> {
    static KIND: OnceLock<TapKind> = OnceLock::new();
    if let Some(kind) = KIND.get() {
        return Ok(*kind);
    }
    let mut kind = TapKind::Duration;
    if sdk >= 30 {
        let primitives = env.new_int_array(1)?;
        env.set_int_array_region(&primitives, 0, &[PRIMITIVE_CLICK])?;
        if env
            .call_method(
                vibrator,
                "areAllPrimitivesSupported",
                "([I)Z",
                &[JValue::Object(&primitives)],
            )?
            .z()?
        {
            kind = TapKind::Click;
        }
    }
    if matches!(kind, TapKind::Duration)
        && sdk >= 26
        && env
            .call_method(vibrator, "hasAmplitudeControl", "()Z", &[])?
            .z()?
    {
        kind = TapKind::Amplitude;
    }
    log::info!("Key vibration: {kind:?} (Android API {sdk})");
    Ok(*KIND.get_or_init(|| kind))
}

/// A `VibrationEffect.createOneShot(ms, amplitude)`.
pub(super) fn one_shot<'local>(
    env: &mut JNIEnv<'local>,
    ms: i64,
    amplitude: i32,
) -> jni::errors::Result<JObject<'local>> {
    env.call_static_method(
        "android/os/VibrationEffect",
        "createOneShot",
        "(JI)Landroid/os/VibrationEffect;",
        &[JValue::Long(ms), JValue::Int(amplitude)],
    )?
    .l()
}

/// Whether touch feedback is on in Android's settings (`Settings.System`
/// `HAPTIC_FEEDBACK_ENABLED`, on when it can't be read). The keys don't vibrate while it's off.
pub(super) fn touch_feedback_enabled() -> bool {
    with_activity(|env, activity| {
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;
        let name = env.new_string("haptic_feedback_enabled")?;
        env.call_static_method(
            "android/provider/Settings$System",
            "getInt",
            "(Landroid/content/ContentResolver;Ljava/lang/String;I)I",
            &[
                JValue::Object(&resolver),
                JValue::Object(&name),
                JValue::Int(1),
            ],
        )?
        .i()
    })
    .is_none_or(|enabled| enabled != 0)
}

/// A key tap of the vibration motor at `strength` (Settings → Key vibration, 0 is Off), the
/// best way this phone's vibrator can (`TapKind`). It goes through `Vibrator`, not
/// `performHapticFeedback(KEYBOARD_TAP)`, whose strength the phone maker sets (barely felt on
/// a Samsung S26 Ultra), and stays off while touch feedback is off in Android's settings.
pub(super) fn key_tap(strength: u8) {
    if strength == 0 || !touch_feedback_enabled() {
        return;
    }
    with_activity(|env, activity| {
        let sdk = sdk_version(env)?;
        let Some(vibrator) = vibrator(env, activity, sdk)? else {
            return Ok(());
        };
        let effect = match tap_kind(env, &vibrator, sdk)? {
            TapKind::Click => {
                let scale = vibration::click_scale(strength).unwrap_or_default();
                let composition = env
                    .call_static_method(
                        "android/os/VibrationEffect",
                        "startComposition",
                        "()Landroid/os/VibrationEffect$Composition;",
                        &[],
                    )?
                    .l()?;
                let composition = env
                    .call_method(
                        &composition,
                        "addPrimitive",
                        "(IF)Landroid/os/VibrationEffect$Composition;",
                        &[JValue::Int(PRIMITIVE_CLICK), JValue::Float(scale)],
                    )?
                    .l()?;
                env.call_method(
                    &composition,
                    "compose",
                    "()Landroid/os/VibrationEffect;",
                    &[],
                )?
                .l()?
            }
            TapKind::Amplitude => {
                let amplitude = vibration::amplitude(strength).unwrap_or(1);
                one_shot(env, vibration::AMPLITUDE_MS, amplitude)?
            }
            TapKind::Duration => {
                let ms = vibration::duration_ms(strength).unwrap_or(vibration::MIN_MS);
                if sdk < 26 {
                    // No `VibrationEffect` before Android 8.
                    env.call_method(&vibrator, "vibrate", "(J)V", &[JValue::Long(ms)])?;
                    return Ok(());
                }
                one_shot(env, ms, DEFAULT_AMPLITUDE)?
            }
        };
        env.call_method(
            &vibrator,
            "vibrate",
            "(Landroid/os/VibrationEffect;)V",
            &[JValue::Object(&effect)],
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
