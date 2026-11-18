//! The Graphing mode's share button (`OnShareClick`, `OnDataRequested`): Windows' share
//! sheet, with the graph as an image and the equations and variables as HTML.

use eframe::egui::{Color32, ColorImage};

use super::region::formats;
use super::strings::tr;

/// What a share holds. Other systems have no share sheet, so nothing reads it there.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) struct Content {
    pub(super) title: String,
    /// The graph, the equations and the variables, as `html` writes them.
    pub(super) html: String,
    /// The graph, as `graph.png`.
    pub(super) image: ColorImage,
}

/// The HTML of a share (`OnDataRequested`): the graph, the equations (as typed) after a
/// square in their color, and the variables with their values.
pub(super) fn html(equations: &[(String, Color32)], variables: &[(char, String)]) -> String {
    let mut html = format!(
        "<p><img src='graph.png' width='600' alt='{}'></p>",
        tr("GraphImageAltText")
    );
    let shared: Vec<_> = equations
        .iter()
        .filter(|(text, _)| !text.is_empty())
        .collect();
    if !shared.is_empty() {
        // Like Windows': "font-size : 13pt", and rows without `</tr>`.
        html += &format!(
            "<span style=\"color: rgb(68, 114, 196); font-style: bold; font-size : 13pt;\">{}\
             </span><table cellpadding=\"0\" cellspacing=\"0\" >",
            tr("EquationsShareHeader")
        );
        for (text, color) in shared {
            html += &format!(
                "<tr style=\"margin: 0pt 0pt 0pt 0pt; padding: 0pt 0pt 0pt 0pt; \"><td><span \
                 style=\"font-size: 22pt; line-height: 0;color:rgb({},{},{});\">&#x25A0;</span>\
                 </td><td><div style=\"margin: 4pt 0pt 0pt 6pt;\">{}</div></td>",
                color.r(),
                color.g(),
                color.b(),
                escape(text)
            );
        }
        html += "</table>";
    }
    if !variables.is_empty() {
        html += &format!(
            "<br><span style=\"color: rgb(68, 114, 196); font-style: bold; font-size: 13pt;\">{}\
             </span><br><div style=\"margin: 4pt 0pt 0pt 0pt;\">",
            tr("VariablesShareHeader")
        );
        let values: Vec<String> = variables
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        html += &values.join(&formats().list_separator);
        html += "</div>";
    }
    html += "<br><br>";
    html
}

/// `EscapeHtmlSpecialCharacters`.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped += "&amp;",
            '"' => escaped += "&quot;",
            '\'' => escaped += "&apos;",
            '<' => escaped += "&lt;",
            '>' => escaped += "&gt;",
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Opens the share sheet of the window `hwnd` with `content`. Returns whether it opened.
#[cfg(windows)]
pub(super) fn show(hwnd: Option<isize>, content: Content) -> bool {
    hwnd.is_some_and(|hwnd| sheet::open(hwnd, &content).is_ok())
}

/// The web version downloads the graph as `graph.png`: browsers' share sheets (the Web Share
/// API) exist only on some systems and take only files, so a download works everywhere.
#[cfg(target_arch = "wasm32")]
pub(super) fn show(_hwnd: Option<isize>, content: Content) -> bool {
    png(&content.image).is_ok_and(|bytes| super::web::download("graph.png", "image/png", &bytes))
}

/// Android's share sheet gets the graph alone, as a PNG (android/README.md, "How it works"):
/// written to the app's cache as the one file `share/graph.png`, which the app's
/// `ShareProvider` serves to the app the user picks.
#[cfg(target_os = "android")]
pub(super) fn show(_hwnd: Option<isize>, content: Content) -> bool {
    let bytes = match png(&content.image) {
        Ok(bytes) => bytes,
        Err(error) => {
            log::warn!("Share: the graph's PNG failed: {error}");
            return false;
        }
    };
    let Some(cache) = super::android::cache_dir() else {
        return false;
    };
    if let Err(error) = write_single_file(&cache.join(SHARE_DIR), SHARED_IMAGE, &bytes) {
        log::warn!("Share: writing the graph to the cache failed: {error}");
        return false;
    }
    super::android::share_image(SHARED_IMAGE)
}

/// There's no share sheet but Windows', Android's and the browser's download.
#[cfg(not(any(windows, target_arch = "wasm32", target_os = "android")))]
pub(super) fn show(_hwnd: Option<isize>, _content: Content) -> bool {
    false
}

/// The folder of the app's cache that holds the shared picture, and nothing else.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const SHARE_DIR: &str = "share";

/// The shared picture's name: `ShareProvider.java` serves only this file, as
/// `content://<application id>.share/graph.png`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const SHARED_IMAGE: &str = "graph.png";

/// Writes `bytes` as `dir/name`, the only file left in `dir`: whatever else is there (files
/// and folders an earlier version or an interrupted share left) is deleted first, and the
/// file itself is overwritten. So shares never pile up in the cache, however many there are
/// (Konst's decision of 2026-11-18). Creates `dir` if needed.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn write_single_file(
    dir: &std::path::Path,
    name: &str,
    bytes: &[u8],
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name() == name && entry.file_type()?.is_file() {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    let path = dir.join(name);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// The image as a PNG file.
#[cfg(any(windows, target_arch = "wasm32", target_os = "android"))]
pub(super) fn png(image: &ColorImage) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    let [width, height] = image.size;
    let mut encoder = png::Encoder::new(&mut bytes, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(image.as_raw())?;
    writer.finish()?;
    Ok(bytes)
}

#[cfg(windows)]
mod sheet {
    use std::cell::RefCell;
    use std::sync::{Mutex, PoisonError};

    use eframe::egui::ColorImage;
    use windows::ApplicationModel::DataTransfer::{
        DataRequestedEventArgs, DataTransferManager, HtmlFormatHelper,
    };
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::Streams::{
        DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference,
    };
    use windows::Win32::Foundation::{E_FAIL, HWND};
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows::core::{Error, HSTRING, Ref, Result, factory};

    use super::Content;

    /// A share, ready for the sheet's request.
    struct Prepared {
        title: HSTRING,
        html: HSTRING,
        image: RandomAccessStreamReference,
    }

    /// The last share: the sheet asks for it (`DataRequested`) once it's open.
    static PENDING: Mutex<Option<Prepared>> = Mutex::new(None);

    thread_local! {
        /// The window's share manager, which has the request handler.
        static MANAGER: RefCell<Option<DataTransferManager>> = const { RefCell::new(None) };
    }

    pub(super) fn open(hwnd: isize, content: &Content) -> Result<()> {
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        let prepared = Prepared {
            title: HSTRING::from(&content.title),
            html: HtmlFormatHelper::CreateHtmlFormat(&HSTRING::from(&content.html))?,
            image: image_stream(&content.image)?,
        };
        *PENDING.lock().unwrap_or_else(PoisonError::into_inner) = Some(prepared);
        let interop = factory::<DataTransferManager, IDataTransferManagerInterop>()?;
        MANAGER.with_borrow_mut(|manager| {
            if manager.is_none() {
                // SAFETY: hwnd is eframe's window, which lives as long as the app.
                let new: DataTransferManager = unsafe { interop.GetForWindow(hwnd) }?;
                new.DataRequested(&TypedEventHandler::new(data_requested))?;
                *manager = Some(new);
            }
            Ok::<_, Error>(())
        })?;
        // SAFETY: as above.
        unsafe { interop.ShowShareUIForWindow(hwnd) }
    }

    /// Fills the sheet's request with the last share.
    fn data_requested(
        _: Ref<DataTransferManager>,
        args: Ref<DataRequestedEventArgs>,
    ) -> Result<()> {
        let pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(share) = pending.as_ref() else {
            return Ok(());
        };
        let data = args.ok()?.Request()?.Data()?;
        let properties = data.Properties()?;
        properties.SetTitle(&share.title)?;
        data.SetHtmlFormat(&share.html)?;
        data.ResourceMap()?
            .Insert(&HSTRING::from("graph.png"), &share.image)?;
        data.SetBitmap(&share.image)?;
        // For share targets without HTML.
        properties.SetThumbnail(&share.image)?;
        Ok(())
    }

    /// The image as a PNG stream.
    fn image_stream(image: &ColorImage) -> Result<RandomAccessStreamReference> {
        let png = super::png(image).map_err(|_| Error::from(E_FAIL))?;
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(&png)?;
        writer.StoreAsync()?.join()?;
        writer.DetachStream()?;
        stream.Seek(0)?;
        RandomAccessStreamReference::CreateFromStream(&stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_html() {
        let html = html(
            &[
                ("x^2".to_string(), Color32::from_rgb(0, 99, 177)),
                (String::new(), Color32::BLACK),
                ("y<a·x".to_string(), Color32::from_rgb(232, 17, 35)),
            ],
            &[('a', "1".to_string()), ('b', "0.5".to_string())],
        );
        assert!(html.starts_with(
            "<p><img src='graph.png' width='600' alt='Image of a graph with equations'></p>"
        ));
        assert!(html.contains("font-size : 13pt;\">Equations</span><table"));
        assert!(html.contains("color:rgb(0,99,177);\">&#x25A0;</span></td><td><div style=\"margin: 4pt 0pt 0pt 6pt;\">x^2</div></td>"));
        assert!(html.contains(">y&lt;a·x</div>"));
        assert_eq!(html.matches("&#x25A0;").count(), 2);
        assert!(html.ends_with(
            "font-size: 13pt;\">Variables</span><br><div style=\"margin: 4pt 0pt 0pt 0pt;\">a=1, b=0.5</div><br><br>"
        ));
        // Without equations or variables, only the graph.
        let html = super::html(&[(String::new(), Color32::BLACK)], &[]);
        assert_eq!(
            html,
            "<p><img src='graph.png' width='600' alt='Image of a graph with equations'></p><br><br>"
        );
    }

    #[test]
    fn escaping() {
        assert_eq!(escape("a<b & c>'d\""), "a&lt;b &amp; c&gt;&apos;d&quot;");
        assert_eq!(escape("sin(x)"), "sin(x)");
    }

    /// The names in a folder, sorted.
    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Android's share cache keeps one picture, however many shares there are.
    #[test]
    fn single_shared_file() {
        let root = std::env::temp_dir().join(format!(
            "calccraft-share-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join(SHARE_DIR);

        // The first share creates the folder.
        let path = write_single_file(&dir, SHARED_IMAGE, b"first").unwrap();
        assert_eq!(path, dir.join("graph.png"));
        assert_eq!(names(&dir), ["graph.png"]);
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        // A second, shorter one overwrites it completely.
        write_single_file(&dir, SHARED_IMAGE, b"2nd").unwrap();
        assert_eq!(names(&dir), ["graph.png"]);
        assert_eq!(std::fs::read(&path).unwrap(), b"2nd");

        // Anything else in the folder goes: files, folders, and a folder named like the file.
        std::fs::write(dir.join("old-graph.png"), b"old").unwrap();
        std::fs::create_dir_all(dir.join("leftover/inner")).unwrap();
        std::fs::write(dir.join("leftover/inner/x.png"), b"x").unwrap();
        write_single_file(&dir, SHARED_IMAGE, b"third").unwrap();
        assert_eq!(names(&dir), ["graph.png"]);
        assert_eq!(std::fs::read(&path).unwrap(), b"third");

        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir_all(dir.join("graph.png/sub")).unwrap();
        write_single_file(&dir, SHARED_IMAGE, b"fourth").unwrap();
        assert_eq!(names(&dir), ["graph.png"]);
        assert_eq!(std::fs::read(&path).unwrap(), b"fourth");

        // Nothing outside the folder is touched.
        std::fs::write(root.join("keep.txt"), b"keep").unwrap();
        write_single_file(&dir, SHARED_IMAGE, b"fifth").unwrap();
        assert_eq!(names(&root), ["keep.txt", "share"]);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
