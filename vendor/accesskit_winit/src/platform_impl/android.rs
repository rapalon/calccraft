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
// - egui's root node has no bounds, so the host view's node had an empty rectangle and
//   services that prune empty nodes dropped the whole tree (Marionet saw one empty Group):
//   `fill_root_bounds` (root_bounds.rs) gives the root the window's rectangle. The adapter
//   also logs (tag CalcCraft) the view the delegate went on, the first request for the tree
//   and the first tree update.
// - accesskit_consumer panics on an update whose focus isn't in the tree, which ended the app:
//   `FocusGuard` (focus_guard.rs) gives such a focus to the root, with one warning, and a
//   panic in the adapter all the same turns the adapter off instead of ending the app.
//
// The rest of the crate is the release's code.

use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_android::{
    jni::{
        errors::Result,
        objects::{JObject, JString, JValue},
        JNIEnv, JavaVM,
    },
    InjectingAdapter,
};
use super::focus_guard::FocusGuard;
use super::root_bounds::fill_root_bounds;
use winit::{
    dpi::PhysicalSize,
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
    /// Keeps every update's focus in the tree (`FocusGuard::check`).
    guard: FocusGuard,
    /// Whether the first tree update has been logged (CalcCraft's diagnostics).
    updated: bool,
    /// The window's size, the root node's bounds (`fill_root_bounds`).
    size: PhysicalSize<u32>,
}

/// The activation handler, logging the first request for the tree: an accessibility service
/// asked the delegate for a node (CalcCraft's diagnostics, `adb logcat -s CalcCraft`).
struct LoggedActivation<H> {
    inner: H,
    requested: bool,
}

impl<H: ActivationHandler> ActivationHandler for LoggedActivation<H> {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        if !self.requested {
            self.requested = true;
            log::info!("AccessKit: an accessibility service asked for the tree");
        }
        self.inner.request_initial_tree()
    }
}

impl Adapter {
    pub fn new(
        event_loop: &ActiveEventLoop,
        window: &Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        _deactivation_handler: impl 'static + DeactivationHandler,
    ) -> Self {
        let app = event_loop.android_app();
        let activation_handler = LoggedActivation {
            inner: activation_handler,
            requested: false,
        };
        let adapter = match new_adapter(app, activation_handler, action_handler) {
            Ok(adapter) => adapter,
            Err(error) => {
                log::warn!("AccessKit: no accessibility tree, the JNI setup failed: {error}");
                None
            }
        };
        Self {
            adapter,
            guard: FocusGuard::default(),
            updated: false,
            size: window.inner_size(),
        }
    }

    pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
        let Some(adapter) = &mut self.adapter else {
            return;
        };
        let updated = &mut self.updated;
        let guard = &mut self.guard;
        let size = self.size;
        // The app must never end because of its accessibility tree: a panic in the adapter
        // (accesskit_consumer checks every update with panics) turns the adapter off instead.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            adapter.update_if_active(|| {
                let mut update = updater();
                fill_root_bounds(&mut update, size.width.into(), size.height.into());
                guard.check(&mut update);
                if !*updated {
                    *updated = true;
                    log::info!(
                        "AccessKit: the first tree update, {} nodes, the root in a {}x{} window",
                        update.nodes.len(),
                        size.width,
                        size.height
                    );
                }
                update
            });
        }));
        if result.is_err() {
            // Dropping the adapter puts the view's old delegate back, and the Java side's calls
            // find no adapter any more (no tree) instead of its poisoned lock.
            log::error!("AccessKit: no accessibility tree any more, the adapter panicked");
            self.adapter = None;
        }
    }

    pub fn process_event(&mut self, _window: &Window, event: &WindowEvent) {
        // The root node's bounds are the window's (`fill_root_bounds`).
        if let WindowEvent::Resized(size) = event {
            self.size = *size;
        }
    }
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
        let name = JString::from(
            env.call_method(&view, "toString", "()Ljava/lang/String;", &[])?
                .l()?,
        );
        let name: String = env.get_string(&name)?.into();
        log::info!("AccessKit: the accessibility delegate goes on {name}");
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
