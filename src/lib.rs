//! CalcCraft, a clone of the Windows 11 Calculator. `main.rs` starts it on desktops and in the
//! browser; on Android, the system loads this library and calls `android_main`.

mod app;
mod brand;
pub mod channel;
mod converter;
mod dates;
mod engine;
mod graphing;
mod icon;
mod locale;
#[cfg(test)]
mod monkey;
#[cfg(feature = "support-prompt")]
pub mod support;

/// Runs the app in a window until it is closed.
#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> eframe::Result {
    use eframe::egui;

    let (w, h) = if let Ok(sz) = std::env::var("CALCCRAFT_WINDOW_SIZE") {
        if let Some((w, h)) = sz.split_once('x') {
            (w.parse().unwrap_or(320.0), h.parse().unwrap_or(500.0))
        } else {
            (320.0, 500.0)
        }
    } else {
        (320.0, 500.0)
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(brand::TITLE)
            .with_inner_size([w, h])
            // A smaller `CALCCRAFT_WINDOW_SIZE` lowers the minimum too, for screenshots at the
            // Android app's sizes (a phone in landscape is some 360 points high).
            .with_min_inner_size(app::MIN_SIZE.min(egui::vec2(w, h)))
            .with_icon(app::window_icon()),
        // Settings are saved, but the window always opens at the default size.
        persist_window: false,
        ..Default::default()
    };
    // The app name also names the settings folder: %APPDATA%\CalcCraft.
    eframe::run_native(
        "CalcCraft",
        options,
        Box::new(|cc| Ok(Box::new(app::CalculatorApp::new(cc)))),
    )
}

/// The web version (`index.html`, built with Trunk): the app draws on the page's canvas,
/// and the settings go to the browser's local storage.
#[cfg(target_arch = "wasm32")]
pub fn start_web() {
    use wasm_bindgen::JsCast;

    // Panics and warnings go to the browser's console.
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .expect("a page");
        let canvas = document
            .get_element_by_id("calccraft")
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("index.html has the canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(app::CalculatorApp::new(cc)))),
            )
            .await;
        // The loading message goes once the app runs, or tells why it couldn't start.
        if let Some(loading) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => loading.remove(),
                Err(error) => {
                    loading.set_text_content(Some(&format!("CalcCraft couldn't start: {error:?}")))
                }
            }
        }
    });
}

/// The Android app (`android/AndroidManifest.xml`): Android's `NativeActivity` loads this
/// library, and android-activity calls this on the app's own thread. The app fills the
/// screen, and its settings go to the app's internal storage.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    // Warnings and panics go to `adb logcat` (a panic's message would go to stderr, which
    // Android drops). The accessibility hook (vendor/accesskit_winit) also logs, once each,
    // where its delegate went and when a service first asked for the tree and got it.
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_filter(
                android_logger::FilterBuilder::new()
                    .filter_level(log::LevelFilter::Warn)
                    .filter_module("accesskit_winit", log::LevelFilter::Info)
                    .build(),
            )
            .with_tag("CalcCraft"),
    );
    std::panic::set_hook(Box::new(|info| log::error!("{info}")));
    app::android::init(&android_app);
    let options = eframe::NativeOptions {
        persistence_path: android_app
            .internal_data_path()
            .map(|dir| dir.join("app.ron")),
        android_app: Some(android_app),
        ..Default::default()
    };
    // A panic is caught here, not by android-activity: it would end only this thread and
    // leave the process running, and the next start would then fail for the reason below
    // ("EventLoop can't be recreated"). The hook above has logged it.
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            "CalcCraft",
            options,
            Box::new(|cc| Ok(Box::new(app::CalculatorApp::new(cc)))),
        )
    }));
    // winit allows one event loop per process, so an activity that Android starts again in
    // this process (after destroying the last one) can't run the app: it ends the process,
    // after a panic too, and the next start gets a new one.
    match run {
        Ok(Ok(())) => std::process::exit(0),
        Ok(Err(error)) => {
            log::error!("CalcCraft couldn't start: {error}");
            std::process::exit(0);
        }
        Err(_) => {
            log::error!("CalcCraft ends after the panic above");
            std::process::exit(1);
        }
    }
}

/// NativeActivity's entry point, named by `android.app.func_name` in
/// `android/AndroidManifest.xml` in place of android-activity's `ANativeActivity_onCreate`,
/// which it runs first and whose `onDestroy` callback it then replaces with
/// [`end_on_destroy`] (android/README.md, "One activity, one process").
///
/// # Safety
///
/// Only NativeActivity calls it, on Android's main thread, with its `ANativeActivity`.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
unsafe extern "C" fn calccraft_on_create(
    activity: *mut ndk_sys::ANativeActivity,
    saved_state: *mut std::ffi::c_void,
    saved_state_size: usize,
) {
    unsafe extern "C" {
        /// android-activity's entry point (`native_activity/glue.rs`): it sets the activity's
        /// callbacks and starts `android_main` on the app's thread.
        fn ANativeActivity_onCreate(
            activity: *mut ndk_sys::ANativeActivity,
            saved_state: *mut std::ffi::c_void,
            saved_state_size: usize,
        );
    }
    // SAFETY: NativeActivity's arguments, passed on as it would have; `callbacks` points at the
    // activity's callback table, which lives as long as the activity.
    unsafe {
        ANativeActivity_onCreate(activity, saved_state, saved_state_size);
        if let Some(callbacks) = activity.as_ref().and_then(|a| a.callbacks.as_mut()) {
            callbacks.onDestroy = Some(end_on_destroy);
        }
    }
}

/// The activity's `onDestroy`: ends the process.
///
/// android-activity's own callback sends `Destroy` to the app's thread and waits for
/// `android_main` to return, but winit 0.30 ignores `Destroy` and never leaves its event loop,
/// so Android's main thread waited forever (an ANR, when Bold text or new wallpaper colours
/// restarted the activity). And a new activity couldn't run in this process anyway: winit
/// allows one event loop per process. The app's state is already saved: Android destroys the
/// window first, and eframe saves when it goes (`Suspended`). `_exit`, not `exit`: the app's
/// and the GL driver's threads still run, and `exit` would run the destructors under them.
#[cfg(target_os = "android")]
unsafe extern "C" fn end_on_destroy(_activity: *mut ndk_sys::ANativeActivity) {
    log::warn!("CalcCraft ends: Android destroyed the activity");
    // SAFETY: `_exit` takes no pointers and doesn't return.
    unsafe { libc::_exit(0) }
}
