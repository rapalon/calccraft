// Copyright 2025 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file).

// Changed by Rapalon Labs for CalcCraft (2026-11-19; Apache-2.0, section 4(b)) from the
// crates.io release accesskit_winit 0.32.2 (https://github.com/AccessKit/accesskit):
//
// - Upstream's adapter is for GameActivity and reads the activity's `mSurfaceView` field.
//   CalcCraft's Android app is a NativeActivity (android/README.md, "Accessibility"), so this
//   copy hooks NativeActivity's content view instead: the window's decor view, its
//   `android.R.id.content` frame, and that frame's one child, `NativeActivity.NativeContentView`.
//   Any `View` takes an accessibility delegate, which is all `InjectingAdapter` needs.
// - `accesskit_android` loads its Java delegate with `dalvik.system.InMemoryDexClassLoader`,
//   which exists from API 26 (Android 8.0). Upstream creates the adapter unconditionally and
//   would panic there; this copy asks `Build.VERSION.SDK_INT` first and, below 26, logs one
//   line and leaves the app without an accessibility tree. A JNI failure is logged the same
//   way, never a panic.
//
// The rest of the crate is the release's code.

use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_android::{
    jni::{
        errors::Result,
        objects::{JObject, JValue},
        JNIEnv, JavaVM,
    },
    InjectingAdapter,
};
use winit::{
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    platform::android::{activity::AndroidApp, ActiveEventLoopExtAndroid},
    window::Window,
};

/// The first Android version with `dalvik.system.InMemoryDexClassLoader`, which loads the
/// adapter's Java delegate (Android 8.0).
const FIRST_SDK: i32 = 26;

pub struct Adapter {
    adapter: Option<InjectingAdapter>,
}

impl Adapter {
    pub fn new(
        event_loop: &ActiveEventLoop,
        _window: &Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        _deactivation_handler: impl 'static + DeactivationHandler,
    ) -> Self {
        let app = event_loop.android_app();
        let adapter = match new_adapter(app, activation_handler, action_handler) {
            Ok(adapter) => adapter,
            Err(error) => {
                log::warn!("AccessKit: no accessibility tree, the JNI setup failed: {error}");
                None
            }
        };
        Self { adapter }
    }

    pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
        if let Some(adapter) = &mut self.adapter {
            adapter.update_if_active(updater);
        }
    }

    pub fn process_event(&mut self, _window: &Window, _event: &WindowEvent) {}
}

/// The adapter on the activity's content view, or `None` where there can't be one: before
/// Android 8.0, or without a content view (then the log says which).
fn new_adapter(
    app: &AndroidApp,
    activation_handler: impl 'static + ActivationHandler + Send,
    action_handler: impl 'static + ActionHandler + Send,
) -> Result<Option<InjectingAdapter>> {
    // SAFETY: android-activity's JavaVM and activity live as long as the app, and the
    // activity is a global reference it owns, which a `JObject` never deletes (as upstream).
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) }?;
    let mut env = vm.get_env()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as *mut _) };
    let result = (|| {
        let sdk = env
            .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
            .i()?;
        if sdk < FIRST_SDK {
            log::warn!(
                "AccessKit: no accessibility tree before Android 8.0 (API {FIRST_SDK}); this is API {sdk}"
            );
            return Ok(None);
        }
        let Some(view) = content_view(&mut env, &activity)? else {
            log::warn!("AccessKit: no accessibility tree, the activity has no content view");
            return Ok(None);
        };
        Ok(Some(InjectingAdapter::new(
            &mut env,
            &view,
            activation_handler,
            action_handler,
        )))
    })();
    // A Java exception left pending would break the app's next JNI call.
    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
        env.exception_clear().ok();
    }
    result
}

/// NativeActivity's content view: the one child of the window's `android.R.id.content` frame.
fn content_view<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject,
) -> Result<Option<JObject<'local>>> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    if window.is_null() {
        return Ok(None);
    }
    let decor = env
        .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
        .l()?;
    let content_id = env.get_static_field("android/R$id", "content", "I")?.i()?;
    let content = env
        .call_method(
            &decor,
            "findViewById",
            "(I)Landroid/view/View;",
            &[JValue::Int(content_id)],
        )?
        .l()?;
    if content.is_null() {
        return Ok(None);
    }
    let count = env.call_method(&content, "getChildCount", "()I", &[])?.i()?;
    if count == 0 {
        return Ok(None);
    }
    let child = env
        .call_method(
            &content,
            "getChildAt",
            "(I)Landroid/view/View;",
            &[JValue::Int(0)],
        )?
        .l()?;
    Ok((!child.is_null()).then_some(child))
}
