//! CalcCraft, a clone of the Windows 11 Calculator. `main.rs` starts it on desktops and in the
//! browser; on Android, the system loads this library and calls `android_main`.

mod app;
mod brand;
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
            .with_min_inner_size(app::MIN_SIZE)
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
    // Android drops).
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Warn)
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
    if let Err(error) = eframe::run_native(
        "CalcCraft",
        options,
        Box::new(|cc| Ok(Box::new(app::CalculatorApp::new(cc)))),
    ) {
        log::error!("CalcCraft couldn't start: {error}");
    }
    // winit allows one event loop per process, so an activity that Android starts again in
    // this process (after destroying the last one) can't run the app: it ends the process,
    // and the next start gets a new one.
    std::process::exit(0);
}
