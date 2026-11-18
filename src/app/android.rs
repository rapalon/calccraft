//! The Android app's ties to the system, through JNI where only Java can tell: the parts of
//! the screen under the status bar, the navigation bar and a camera cutout (NativeActivity
//! draws under them), the user's languages, the back button, the keys' vibration, whether
//! the system keyboard is open, the screen orientation, and the share sheet.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{Context, Event, Key, RawInput, SafeAreaInsets, epaint::MarginF32};
use jni::JNIEnv;
use jni::objects::{JIntArray, JObject, JString, JValue};
use winit::platform::android::activity::AndroidApp;

use super::{orientation, vibration};

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

/// `VibrationEffect.Composition.PRIMITIVE_CLICK` and `PRIMITIVE_TICK`.
const PRIMITIVE_CLICK: i32 = 1;
const PRIMITIVE_TICK: i32 = 7;
/// `VibrationEffect.EFFECT_HEAVY_CLICK`.
const EFFECT_HEAVY_CLICK: i32 = 5;
/// `VibrationEffect.DEFAULT_AMPLITUDE`, for vibrators without amplitude control.
const DEFAULT_AMPLITUDE: i32 = -1;

/// The phone's default vibrator (`VibratorManager.getDefaultVibrator` from Android 12, the
/// `vibrator` service before), or `None` when it has no vibrator.
fn vibrator<'local>(
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

/// What `vibrator` can do, asked once: it doesn't change while the app runs. The first time,
/// the log (tag `CalcCraft`) gets the capabilities and the style Auto picks from them.
fn read_capabilities(
    env: &mut JNIEnv,
    vibrator: &JObject,
    sdk: i32,
) -> jni::errors::Result<vibration::Capabilities> {
    static CAPABILITIES: OnceLock<vibration::Capabilities> = OnceLock::new();
    if let Some(capabilities) = CAPABILITIES.get() {
        return Ok(*capabilities);
    }
    let mut capabilities = vibration::Capabilities {
        sdk,
        amplitude_control: false,
        click_primitive: false,
        resonant_frequency: f32::NAN,
        click_ms: 0,
        tick_ms: 0,
    };
    if sdk >= 26 {
        capabilities.amplitude_control = env
            .call_method(vibrator, "hasAmplitudeControl", "()Z", &[])?
            .z()?;
    }
    if sdk >= 30 {
        let primitives = env.new_int_array(1)?;
        env.set_int_array_region(&primitives, 0, &[PRIMITIVE_CLICK])?;
        capabilities.click_primitive = env
            .call_method(
                vibrator,
                "areAllPrimitivesSupported",
                "([I)Z",
                &[JValue::Object(&primitives)],
            )?
            .z()?;
    }
    if sdk >= 31 {
        capabilities.resonant_frequency = env
            .call_method(vibrator, "getResonantFrequency", "()F", &[])?
            .f()?;
        let primitives = env.new_int_array(2)?;
        env.set_int_array_region(&primitives, 0, &[PRIMITIVE_CLICK, PRIMITIVE_TICK])?;
        let durations = env
            .call_method(
                vibrator,
                "getPrimitiveDurations",
                "([I)[I",
                &[JValue::Object(&primitives)],
            )?
            .l()?;
        if !durations.is_null() {
            let durations = JIntArray::from(durations);
            let mut ms = [0; 2];
            env.get_int_array_region(&durations, 0, &mut ms)?;
            [capabilities.click_ms, capabilities.tick_ms] = ms;
        }
    }
    // A warning, the lowest level `android_main`'s logger keeps (info would let egui's in).
    log::warn!(
        "Key vibration: Auto plays {:?} (API {sdk}, amplitude control {}, click primitive {}, \
         resonant frequency {} Hz, CLICK {} ms, TICK {} ms, calibrated {})",
        vibration::auto(&capabilities),
        capabilities.amplitude_control,
        capabilities.click_primitive,
        capabilities.resonant_frequency,
        capabilities.click_ms,
        capabilities.tick_ms,
        capabilities.calibrated(),
    );
    Ok(*CAPABILITIES.get_or_init(|| capabilities))
}

/// What the phone's vibrator can do, or `None` without one. Logs Auto's pick the first time.
pub(super) fn vibrator_capabilities() -> Option<vibration::Capabilities> {
    with_activity(|env, activity| {
        let sdk = sdk_version(env)?;
        match vibrator(env, activity, sdk)? {
            Some(vibrator) => read_capabilities(env, &vibrator, sdk).map(Some),
            None => Ok(None),
        }
    })
    .flatten()
}

/// A `VibrationEffect.createOneShot(ms, amplitude)`.
fn one_shot<'local>(
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

/// A key tap of the vibration motor with the settings in effect (Settings → Key vibration:
/// strength, style and Pulse duration), as `vibration::tap` says it plays on this phone. It
/// goes through `Vibrator`, not `performHapticFeedback(KEYBOARD_TAP)`, whose strength the
/// phone maker sets (barely felt on a Samsung S26 Ultra), and stays off for the style Off and
/// while touch feedback is off in Android's settings.
pub(super) fn key_tap() {
    let strength = vibration::strength();
    let style = vibration::style();
    if style == vibration::Style::Off || !touch_feedback_enabled() {
        return;
    }
    with_activity(|env, activity| {
        let sdk = sdk_version(env)?;
        let Some(vibrator) = vibrator(env, activity, sdk)? else {
            return Ok(());
        };
        let capabilities = read_capabilities(env, &vibrator, sdk)?;
        let Some(tap) = vibration::tap(style, vibration::duration(), &capabilities) else {
            return Ok(());
        };
        let effect = match tap {
            vibration::Tap::Click => {
                let scale = vibration::click_scale(strength);
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
            vibration::Tap::Pulse { ms } => {
                one_shot(env, i64::from(ms), vibration::amplitude(strength))?
            }
            vibration::Tap::HeavyClick => env
                .call_static_method(
                    "android/os/VibrationEffect",
                    "createPredefined",
                    "(I)Landroid/os/VibrationEffect;",
                    &[JValue::Int(EFFECT_HEAVY_CLICK)],
                )?
                .l()?,
            vibration::Tap::Duration => {
                let ms = vibration::duration_ms(strength);
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

/// Whether the system keyboard is open (`WindowInsets.isVisible(Type.ime())`), or `None`
/// before Android 11, which can't tell. The Graphing mode's keyboard toggle goes off when
/// the keyboard closes (Back), which the app isn't told otherwise: the keyboard takes that
/// Back itself. egui-winit opens and closes the keyboard (`showSoftInput`).
pub(super) fn ime_visible() -> Option<bool> {
    with_activity(|env, activity| {
        if sdk_version(env)? < 30 {
            return Ok(None);
        }
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
        let ime = env
            .call_static_method("android/view/WindowInsets$Type", "ime", "()I", &[])?
            .i()?;
        let visible = env
            .call_method(&insets, "isVisible", "(I)Z", &[JValue::Int(ime)])?
            .z()?;
        Ok(Some(visible))
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

/// Asks Android for the screen orientation of Settings → Screen orientation
/// (`Activity.setRequestedOrientation`, `orientation::Orientation::requested`). The manifest
/// asks for none, and lets the app handle the rotation itself (`configChanges`), so the
/// activity isn't restarted: the next frames just get the new size. It goes through the
/// activity itself, so it works for NativeActivity and GameActivity alike.
pub(super) fn set_orientation(orientation: orientation::Orientation) {
    with_activity(|env, activity| {
        env.call_method(
            activity,
            "setRequestedOrientation",
            "(I)V",
            &[JValue::Int(orientation.requested())],
        )?;
        Ok(())
    });
}

/// Whether Android honors the orientation the app asks for (`orientation::honored`): not on
/// large screens from Android 16. Asked once: neither changes while the app runs (a foldable's
/// two screens aside, where the setting simply stays).
pub(super) fn orientation_honored() -> bool {
    static HONORED: OnceLock<bool> = OnceLock::new();
    *HONORED.get_or_init(|| {
        with_activity(|env, activity| {
            let sdk = sdk_version(env)?;
            let resources = env
                .call_method(
                    activity,
                    "getResources",
                    "()Landroid/content/res/Resources;",
                    &[],
                )?
                .l()?;
            let configuration = env
                .call_method(
                    &resources,
                    "getConfiguration",
                    "()Landroid/content/res/Configuration;",
                    &[],
                )?
                .l()?;
            let smallest = env
                .get_field(&configuration, "smallestScreenWidthDp", "I")?
                .i()?;
            Ok(orientation::honored(sdk, smallest))
        })
        .unwrap_or(true)
    })
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

/// The app's private cache folder (`Context.getCacheDir()`), where the shared graph goes.
pub(super) fn cache_dir() -> Option<std::path::PathBuf> {
    with_activity(|env, activity| {
        let dir = env
            .call_method(activity, "getCacheDir", "()Ljava/io/File;", &[])?
            .l()?;
        let path = env
            .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
            .l()?;
        let path: String = env.get_string(&JString::from(path))?.into();
        Ok(std::path::PathBuf::from(path))
    })
}

/// `Intent.FLAG_GRANT_READ_URI_PERMISSION`.
const FLAG_GRANT_READ_URI_PERMISSION: i32 = 0x0000_0001;

/// Opens the system share sheet with the PNG `name` of the app's `ShareProvider`
/// (`content://<package>.share/<name>`, android/AndroidManifest.xml): `ACTION_SEND` with
/// `image/png`, the URI as `EXTRA_STREAM` and as `ClipData`, and a read grant, in a chooser,
/// which passes the grant on to the app the user picks. Returns whether it started.
pub(super) fn share_image(name: &str) -> bool {
    with_activity(|env, activity| {
        let package = env
            .call_method(activity, "getPackageName", "()Ljava/lang/String;", &[])?
            .l()?;
        let package: String = env.get_string(&JString::from(package))?.into();
        let text = env.new_string(format!("content://{package}.share/{name}"))?;
        let uri = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            )?
            .l()?;
        let send = new_intent(env, "android.intent.action.SEND")?;
        let mime = env.new_string("image/png")?;
        env.call_method(
            &send,
            "setType",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&mime)],
        )?;
        let key = env.new_string("android.intent.extra.STREAM")?;
        env.call_method(
            &send,
            "putExtra",
            "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;",
            &[JValue::Object(&key), JValue::Object(&uri)],
        )?;
        // The grant covers the intent's data and ClipData, not its extras: the ClipData
        // carries the URI to the picked app (and gives the share sheet its preview).
        let label = env.new_string(name)?;
        let clip = env
            .call_static_method(
                "android/content/ClipData",
                "newRawUri",
                "(Ljava/lang/CharSequence;Landroid/net/Uri;)Landroid/content/ClipData;",
                &[JValue::Object(&label), JValue::Object(&uri)],
            )?
            .l()?;
        env.call_method(
            &send,
            "setClipData",
            "(Landroid/content/ClipData;)V",
            &[JValue::Object(&clip)],
        )?;
        env.call_method(
            &send,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(FLAG_GRANT_READ_URI_PERMISSION)],
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
