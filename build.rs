//! Embeds the app icon and version information in the Windows executable, so
//! Explorer, the taskbar and Task Manager show the calculator icon and the company's credit.
//!
//! The icon is rendered from `src/icon.rs`, the same drawing as the window icon,
//! into an .ico file with one image per size Windows asks for.

#[path = "src/icon.rs"]
#[allow(dead_code)]
mod icon;

/// Sizes in the .ico file: small and large icons at 100–250 % scaling, and the
/// 256 px "jumbo" icon that Explorer uses for large views.
const ICON_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let ico_path = std::path::Path::new(&out_dir).join("calculator.ico");
    std::fs::write(&ico_path, ico(&ICON_SIZES)).expect("writing the .ico file");

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(ico_path.to_str().expect("OUT_DIR is valid UTF-8"))
        .set("ProductName", "CalcCraft")
        .set("CompanyName", "Rapalon Labs, Inc.")
        // Task Manager shows the file description as the app's name.
        .set("FileDescription", "CalcCraft by Rapalon Labs");
    resource
        .compile()
        .expect("embedding the icon needs rc.exe from the Windows SDK");
}

/// An .ico file with a 32-bit BMP image for each size.
fn ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&size| bmp(size)).collect();

    let mut file = Vec::new();
    // ICONDIR: reserved, type 1 (icon), image count.
    file.extend(0u16.to_le_bytes());
    file.extend(1u16.to_le_bytes());
    file.extend((sizes.len() as u16).to_le_bytes());

    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, image) in sizes.iter().zip(&images) {
        // ICONDIRENTRY; a width or height of 0 means 256.
        let dimension = if size >= 256 { 0 } else { size as u8 };
        file.extend([dimension, dimension, 0, 0]);
        file.extend(1u16.to_le_bytes()); // color planes
        file.extend(32u16.to_le_bytes()); // bits per pixel
        file.extend((image.len() as u32).to_le_bytes());
        file.extend(offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in images {
        file.extend(image);
    }
    file
}

/// One icon image: a BITMAPINFOHEADER, BGRA pixels bottom-up, then the 1-bit
/// AND mask (set where the pixel is fully transparent, for old renderers).
fn bmp(size: u32) -> Vec<u8> {
    let rgba = icon::rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let pixels_len = size * size * 4;
    let mask_len = mask_row * size;

    let mut image = Vec::with_capacity((40 + pixels_len + mask_len) as usize);
    image.extend(40u32.to_le_bytes()); // header size
    image.extend((size as i32).to_le_bytes());
    image.extend((2 * size as i32).to_le_bytes()); // the pixels and the mask
    image.extend(1u16.to_le_bytes()); // planes
    image.extend(32u16.to_le_bytes()); // bits per pixel
    image.extend(0u32.to_le_bytes()); // BI_RGB, uncompressed
    image.extend((pixels_len + mask_len).to_le_bytes());
    image.extend([0u8; 16]); // resolution and palette: unused

    let row = |y: u32| &rgba[(y * size * 4) as usize..((y + 1) * size * 4) as usize];
    for y in (0..size).rev() {
        for pixel in row(y).as_chunks::<4>().0 {
            image.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    for y in (0..size).rev() {
        let mut bits = vec![0u8; mask_row as usize];
        for (x, pixel) in row(y).as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] == 0 {
                bits[x / 8] |= 0x80 >> (x % 8);
            }
        }
        image.extend(bits);
    }
    image
}
