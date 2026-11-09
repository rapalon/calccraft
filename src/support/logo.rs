//! The Rapalon Labs logo: the "R" monogram of rapalon.com (`monogramSvg` in the website's
//! `src/lib/brand.ts`). Like the app icon it's drawn in code, so no image file ships and it is
//! sharp at every display scale. Keep the outline and the colors as the website writes them.

/// The monogram's coordinates: a 512 × 512 tile, with this corner radius.
const TILE: f32 = 512.0;
const TILE_RADIUS: f32 = 112.0;
/// The height of the R as a share of the tile, and its bounding box in the outline's units.
const R_SCALE: f32 = 0.6;
const R_X: f32 = 52.0;
const R_Y: f32 = 161.0;
const R_WIDTH: f32 = 692.0;
const R_HEIGHT: f32 = 700.0;

/// A capital R from Fraunces SemiBold: an SVG path of absolute `M`, `L`, `Q` and `Z` commands.
const R_PATH: &str = "M679.5 401.5Q679.5 449.5 658 488.5Q636.5 527.5 597 553.5Q575 568.5 548 578L672 777Q681.5 791.5 691.3 799.5Q701 807.5 717.5 812.5Q733 817 738.5 822.8Q744 828.5 744 838Q744 848.5 736.3 854.8Q728.5 861 713 861L500 861Q470.5 861 470.5 838Q470.5 831.5 474 827Q477.5 822.5 485 819L499.5 815Q510.5 810.5 512 803.5Q513.5 796.5 506 784L403 607.5Q397.5 607.5 392 607.5Q361 607.5 332.8 603.3Q304.5 599 281 590.5L281 777.5Q281 788.5 286.5 795.5Q292 802.5 303 806L337.5 812.5Q349.5 817 354 822.8Q358.5 828.5 358.5 838Q358.5 861 328.5 861L82 861Q66.5 861 59.3 854.8Q52 848.5 52 838Q52 819 73 812.5L101.5 806Q113.5 802.5 120 795.5Q126.5 788.5 126.5 777.5L126.5 244.5Q126.5 233.5 120 226.5Q113.5 219.5 101.5 216L73 209.5Q52 203 52 184Q52 173 59.3 167Q66.5 161 82 161L330.5 161Q443 161 520.8 191.8Q598.5 222.5 639 276.8Q679.5 331 679.5 401.5ZM281 207.5L281 545Q298.5 551 318.5 554Q341 557.5 366 557.5Q438.5 557.5 479.3 518.5Q520 479.5 520 403.5Q520 346.5 495.8 302.3Q471.5 258 427.8 232.8Q384 207.5 325 207.5L281 207.5Z";

const NIGHT: [f32; 3] = [0x05 as f32, 0x08 as f32, 0x12 as f32];
const DUSK: [f32; 3] = [0x18 as f32, 0x21 as f32, 0x3b as f32];
const GLOW: [f32; 3] = [0xff as f32, 0xd8 as f32, 0xa0 as f32];
const AMBER: [f32; 3] = [0xf7 as f32, 0xa2 as f32, 0x4f as f32];
const INK: [f32; 3] = [0xf7 as f32, 0xf3 as f32, 0xec as f32];

/// Straight pieces per curve, and scanlines per pixel row.
const CURVE_STEPS: usize = 8;
const SUBROWS: usize = 4;

type Point = (f32, f32);

/// The outline as closed polygons, in the path's units.
fn polygons() -> Vec<Vec<Point>> {
    let mut rest = R_PATH;
    let mut polygons = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    while let Some(command) = rest.chars().next() {
        rest = &rest[1..];
        let end = rest.find(char::is_alphabetic).unwrap_or(rest.len());
        let numbers: Vec<f32> = rest[..end]
            .split_whitespace()
            .map(|n| n.parse().expect("the outline's numbers"))
            .collect();
        rest = &rest[end..];
        match command {
            'M' => current = vec![(numbers[0], numbers[1])],
            'L' => current.push((numbers[0], numbers[1])),
            'Q' => {
                let start = *current.last().expect("a curve follows a point");
                for step in 1..=CURVE_STEPS {
                    let t = step as f32 / CURVE_STEPS as f32;
                    let bezier = |a: f32, b: f32, c: f32| {
                        (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * b + t * t * c
                    };
                    current.push((
                        bezier(start.0, numbers[0], numbers[2]),
                        bezier(start.1, numbers[1], numbers[3]),
                    ));
                }
            }
            _ => polygons.push(std::mem::take(&mut current)),
        }
    }
    polygons
}

/// How much of each pixel the R covers (0 to 1), `size` × `size` pixels, row by row. The counter
/// of the R stays open because the path is filled by SVG's default nonzero rule.
fn r_coverage(size: usize) -> Vec<f32> {
    let scale = size as f32 / TILE * (TILE * R_SCALE / R_HEIGHT);
    let dx = (size as f32 - R_WIDTH * scale) / 2.0 - R_X * scale;
    let dy = (size as f32 - R_HEIGHT * scale) / 2.0 - R_Y * scale;
    let polygons: Vec<Vec<Point>> = polygons()
        .into_iter()
        .map(|polygon| {
            let place = |(x, y): Point| (x * scale + dx, y * scale + dy);
            polygon.into_iter().map(place).collect()
        })
        .collect();

    let mut coverage = vec![0.0; size * size];
    for subrow in 0..size * SUBROWS {
        let y = (subrow as f32 + 0.5) / SUBROWS as f32;
        let row = subrow / SUBROWS;
        let mut crossings: Vec<(f32, i32)> = Vec::new();
        for polygon in &polygons {
            for (i, &(ax, ay)) in polygon.iter().enumerate() {
                let (bx, by) = polygon[(i + 1) % polygon.len()];
                if (ay <= y) != (by <= y) {
                    let x = ax + (y - ay) / (by - ay) * (bx - ax);
                    crossings.push((x, if by > ay { 1 } else { -1 }));
                }
            }
        }
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut winding = 0;
        for pair in crossings.windows(2) {
            winding += pair[0].1;
            if winding != 0 {
                let (from, to) = (pair[0].0.max(0.0), pair[1].0.min(size as f32));
                for x in from as usize..(to.ceil() as usize).min(size) {
                    let overlap = to.min(x as f32 + 1.0) - from.max(x as f32);
                    coverage[row * size + x] += overlap.max(0.0) / SUBROWS as f32;
                }
            }
        }
    }
    coverage
}

/// How much of the pixel at (x, y), in tile units, lies inside the tile's rounded corners, with
/// `pixel` tile units to a pixel.
fn tile_coverage(x: f32, y: f32, pixel: f32) -> f32 {
    let qx = (x - TILE / 2.0).abs() - (TILE / 2.0 - TILE_RADIUS);
    let qy = (y - TILE / 2.0).abs() - (TILE / 2.0 - TILE_RADIUS);
    let outside = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - TILE_RADIUS;
    (0.5 - outside / pixel).clamp(0.0, 1.0)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// The tile's color at (x, y), in tile units: the night sky with the dawn glow rising from below
/// the bottom edge (the website's `sky` and `dawn` gradients).
fn sky(x: f32, y: f32) -> [f32; 3] {
    let base = mix(NIGHT, DUSK, y / TILE);
    let distance = (x - 0.5 * TILE).hypot(y - 1.1 * TILE) / (0.75 * TILE);
    let (color, opacity) = if distance < 0.45 {
        let t = distance / 0.45;
        (mix(GLOW, AMBER, t), 0.85 + (0.3 - 0.85) * t)
    } else {
        (AMBER, 0.3 * (1.0 - ((distance - 0.45) / 0.55).min(1.0)))
    };
    mix(base, color, opacity)
}

/// The logo as unmultiplied RGBA pixels, `size` × `size`, row by row.
pub(super) fn rgba(size: u32) -> Vec<u8> {
    let side = size as usize;
    let pixel = TILE / size as f32;
    let coverage = r_coverage(side);
    let mut image = Vec::with_capacity(side * side * 4);
    for row in 0..side {
        for column in 0..side {
            let x = (column as f32 + 0.5) * pixel;
            let y = (row as f32 + 0.5) * pixel;
            let color = mix(sky(x, y), INK, coverage[row * side + column].min(1.0));
            let alpha = tile_coverage(x, y, pixel);
            image.extend(color.map(|channel| channel.round() as u8));
            image.push((alpha * 255.0).round() as u8);
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u32 = 128;

    fn at(image: &[u8], x: f32, y: f32) -> [u8; 4] {
        let side = SIZE as usize;
        let row = (y / TILE * SIZE as f32) as usize;
        let column = (x / TILE * SIZE as f32) as usize;
        let i = (row * side + column) * 4;
        [image[i], image[i + 1], image[i + 2], image[i + 3]]
    }

    fn is_ink(pixel: [u8; 4]) -> bool {
        pixel[0] > 240 && pixel[2] > 225 && pixel[3] == 255
    }

    #[test]
    fn the_outline_is_the_rs_two_closed_contours_in_its_bounding_box() {
        let polygons = polygons();
        assert_eq!(polygons.len(), 2);
        let points = || polygons.iter().flatten();
        let (min_x, max_x) =
            points().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (min_y, max_y) =
            points().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        assert!((min_x - R_X).abs() < 1.0 && (max_x - (R_X + R_WIDTH)).abs() < 1.0);
        assert!((min_y - R_Y).abs() < 1.0 && (max_y - (R_Y + R_HEIGHT)).abs() < 1.0);
    }

    #[test]
    fn the_tile_has_round_corners_and_is_opaque_inside() {
        let image = rgba(SIZE);
        assert_eq!(image.len(), (SIZE * SIZE * 4) as usize);
        assert_eq!(at(&image, 2.0, 2.0)[3], 0);
        assert_eq!(at(&image, 509.0, 509.0)[3], 0);
        assert_eq!(at(&image, 40.0, 256.0)[3], 255);
    }

    #[test]
    fn the_r_is_drawn_with_an_open_counter() {
        let image = rgba(SIZE);
        // In tile units the R spans about 154 pixels of the outline's 692 units across.
        let scale = TILE * R_SCALE / R_HEIGHT;
        let dx = (TILE - R_WIDTH * scale) / 2.0 - R_X * scale;
        let dy = (TILE - R_HEIGHT * scale) / 2.0 - R_Y * scale;
        let place = |x: f32, y: f32| (x * scale + dx, y * scale + dy);
        // The stem, and the bowl's right side.
        let (x, y) = place(200.0, 500.0);
        assert!(is_ink(at(&image, x, y)));
        let (x, y) = place(600.0, 400.0);
        assert!(is_ink(at(&image, x, y)));
        // The counter inside the bowl shows the sky.
        let (x, y) = place(400.0, 380.0);
        assert!(!is_ink(at(&image, x, y)));
        // And so does the tile beside the R.
        assert!(!is_ink(at(&image, 40.0, 256.0)));
    }

    #[test]
    fn the_glow_rises_from_the_bottom() {
        let image = rgba(SIZE);
        let top = at(&image, 256.0, 30.0);
        let bottom = at(&image, 256.0, 495.0);
        assert!(bottom[0] > top[0] + 100, "{top:?} {bottom:?}");
    }

    #[test]
    fn small_sizes_stay_in_bounds() {
        for size in [1, 2, 16, 22, 44, 66] {
            assert_eq!(rgba(size).len(), (size * size * 4) as usize);
        }
    }
}
