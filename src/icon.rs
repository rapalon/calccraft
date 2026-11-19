//! The app icon: Konst's calculator on blue (android/icon-source/calccraft-icon-512.png),
//! as a rounded tile.
//!
//! `tools/app-icon.py` scales the artwork down to each size: straight-alpha RGBA for the
//! window icon (`app::window_icon`) and the small images of the executable's .ico, and a PNG
//! for its 256 px image. `build.rs` includes this file, so it uses only `std`.

/// The sizes of the RGBA images, in pixels: small and large icons at 100–250 % scaling.
#[allow(dead_code)] // build.rs's .ico; the app itself uses only the 64 px image
pub const SIZES: [u32; 7] = [16, 20, 24, 32, 40, 48, 64];

/// The 256 px image Explorer uses for large views, as a PNG (an .ico may hold one as it is).
#[allow(dead_code)] // build.rs's .ico
pub const PNG_256: &[u8] = include_bytes!("../assets/icon/icon-256.png");

/// The icon at `size` × `size` pixels (one of `SIZES`), as RGBA with straight alpha.
pub fn rgba(size: u32) -> Option<&'static [u8]> {
    Some(match size {
        16 => include_bytes!("../assets/icon/icon-16.rgba"),
        20 => include_bytes!("../assets/icon/icon-20.rgba"),
        24 => include_bytes!("../assets/icon/icon-24.rgba"),
        32 => include_bytes!("../assets/icon/icon-32.rgba"),
        40 => include_bytes!("../assets/icon/icon-40.rgba"),
        48 => include_bytes!("../assets/icon/icon-48.rgba"),
        64 => include_bytes!("../assets/icon/icon-64.rgba"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        pixels[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn every_size_is_there() {
        for size in SIZES {
            assert_eq!(
                rgba(size).unwrap().len(),
                (size * size * 4) as usize,
                "{size}"
            );
        }
        assert!(rgba(256).is_none());
        let png = png::Decoder::new(std::io::Cursor::new(PNG_256))
            .read_info()
            .unwrap();
        assert_eq!((png.info().width, png.info().height), (256, 256));
    }

    #[test]
    fn a_rounded_blue_tile_with_the_calculator() {
        let pixels = rgba(64).unwrap();
        // The corners are rounded off; the left edge's middle is the tile's blue.
        assert_eq!(pixel(pixels, 64, 0, 0)[3], 0);
        assert_eq!(pixel(pixels, 64, 2, 32), [0x2E, 0x67, 0xEB, 0xFF]);
        // The middle is the calculator, opaque and not blue.
        let middle = pixel(pixels, 64, 32, 40);
        assert_eq!(middle[3], 0xFF);
        assert!(middle[2] < 0xC0, "{middle:?}");
    }

    /// The Android launcher icons and the Play listing icon that `tools/app-icon.py` wrote.
    #[test]
    fn android_icons_are_complete() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let res = root.join("android/res");
        let adaptive =
            std::fs::read_to_string(res.join("mipmap-anydpi-v26/ic_launcher.xml")).unwrap();
        for layer in [
            "@color/ic_launcher_background",
            "@drawable/ic_launcher_foreground",
            "@drawable/ic_launcher_monochrome",
        ] {
            assert!(adaptive.contains(layer), "{layer}");
        }
        let size_of = |path: &std::path::Path| {
            let file = std::fs::File::open(path).unwrap();
            let png = png::Decoder::new(std::io::BufReader::new(file))
                .read_info()
                .unwrap();
            let info = png.info();
            ((info.width, info.height), info.color_type)
        };
        for (density, dp) in [
            ("mdpi", 1.0),
            ("hdpi", 1.5),
            ("xhdpi", 2.0),
            ("xxhdpi", 3.0),
            ("xxxhdpi", 4.0),
        ] {
            // The adaptive icon's layers: the 108 dp canvas, with transparency.
            let layer = (108.0 * dp) as u32;
            for name in ["ic_launcher_foreground", "ic_launcher_monochrome"] {
                let path = res.join(format!("drawable-{density}/{name}.png"));
                assert_eq!(
                    size_of(&path),
                    ((layer, layer), png::ColorType::Rgba),
                    "{path:?}"
                );
            }
            // Android 7's PNG icons: 48 dp, square and round.
            let icon = (48.0 * dp) as u32;
            for name in ["ic_launcher", "ic_launcher_round"] {
                let path = res.join(format!("mipmap-{density}/{name}.png"));
                assert_eq!(size_of(&path).0, (icon, icon), "{path:?}");
            }
        }
        // Google Play's listing icon is Konst's square icon as it is: 512 × 512, opaque (Play
        // adds the shape and the shadow).
        let store = root.join("android/store/icon-512.png");
        assert_eq!(size_of(&store), ((512, 512), png::ColorType::Rgb));
        assert_eq!(
            std::fs::read(&store).unwrap(),
            std::fs::read(root.join("android/icon-source/calccraft-icon-512.png")).unwrap()
        );
    }
}
