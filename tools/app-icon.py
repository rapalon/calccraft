# /// script
# requires-python = ">=3.10"
# dependencies = ["fonttools==4.67.0", "pillow==12.0.0", "skia-pathops==0.9.2"]
# ///
"""Builds every app icon from one vector drawing of Konst's icon.

The reference is android/icon-source/calccraft-icon-512.png (the artwork, 512 x 512). The
drawing below redraws it on the same 512 grid with its sampled colours: rounded rectangles for
the calculator, its display and its keys, and the display's "12,345" as outlines of the
embedded Inter (assets/fonts/Inter.ttf, SIL OFL) at the weight and size fitted to the
reference. From that one drawing it writes:

- android/icon-source/icon-{background,foreground,monochrome}.svg: the adaptive icon's layers
  on Android's 108 x 108 dp canvas (the calculator scaled so the 512 grid fills the 72 dp the
  launcher shows, inside the 66 dp safe circle);
- android/res/: the same layers as vector drawables, the adaptive icon (mipmap-anydpi-v26,
  with the monochrome layer of Android 13's themed icons), and PNG launcher icons, square and
  round, for Android 7 (API 24 and 25), which has no adaptive icons;
- android/store/icon-512.png: Google Play's listing icon, full-bleed and opaque (Play rounds it
  and adds the shadow);
- assets/icon/: the desktop icons (src/icon.rs: the window icon and the .ico of build.rs) as
  straight-alpha RGBA, and a 256 px PNG for the .ico's largest image;
- assets/web/: the browser's tab icon, the iOS home screen icon and the PWA's icons;
- target/icon-previews/ (not committed): the launcher icon at its real size in the launchers'
  masks, on light and dark wallpapers, and the themed (monochrome) look.

Run it from the repository root after changing the icon:

    uv run tools/app-icon.py
"""

import io
import math
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import OverlapMode, instantiateVariableFont
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "android" / "icon-source" / "calccraft-icon-512.png"
INTER = ROOT / "assets" / "fonts" / "Inter.ttf"

# Colours sampled from the reference (means over each area's flat inside).
BACKGROUND = "#2E67EB"
BODY = "#F2F3F8"
DISPLAY = "#FFFFFF"
TEXT = "#0A162D"
NAVY = "#243B65"
BLUE = "#2D66EF"
GREEN = "#0FBB7F"

# The text: Inter at this weight and size (in 512-grid pixels), its right edge and baseline,
# fitted to the reference's "12,345" (target/iconwork/fit.py minimised the pixel difference).
TEXT_STRING = "12,345"
TEXT_WEIGHT = 620
TEXT_SIZE = 39.5
TEXT_RIGHT = 340.0
TEXT_BASELINE = 168.5


class Rect:
    """A rounded rectangle on the 512 grid."""

    def __init__(self, left, top, right, bottom, radius, colour):
        self.left, self.top, self.right, self.bottom = left, top, right, bottom
        self.radius, self.colour = radius, colour

    def path(self, t):
        """SVG path data (also valid in an Android vector drawable) through the transform t."""
        (x0, y0), (x1, y1) = t(self.left, self.top), t(self.right, self.bottom)
        r = self.radius * (x1 - x0) / (self.right - self.left)
        f = fmt
        return (
            f"M{f(x0 + r)},{f(y0)}H{f(x1 - r)}A{f(r)},{f(r)} 0 0 1 {f(x1)},{f(y0 + r)}"
            f"V{f(y1 - r)}A{f(r)},{f(r)} 0 0 1 {f(x1 - r)},{f(y1)}H{f(x0 + r)}"
            f"A{f(r)},{f(r)} 0 0 1 {f(x0)},{f(y1 - r)}V{f(y0 + r)}"
            f"A{f(r)},{f(r)} 0 0 1 {f(x0 + r)},{f(y0)}Z"
        )


def fmt(v):
    return f"{v:.3f}".rstrip("0").rstrip(".")


# The calculator, measured on the reference: the body, the display bar, and a 4 x 5 grid of
# keys 41 x 35 px (51 px apart across, 43.9 down) whose bottom row has the wide 0.
BODY_RECT = Rect(137, 78, 375, 434, 16, BODY)
DISPLAY_RECT = Rect(159, 126, 353, 181, 7, DISPLAY)


def keys():
    left = [159.5 + 51 * i for i in range(4)]
    top = [203 + 43.9 * j for j in range(5)]
    width, height, radius = 41, 35, 6
    colours = [
        [GREEN, NAVY, NAVY, BLUE],
        [NAVY, NAVY, NAVY, BLUE],
        [NAVY, NAVY, NAVY, BLUE],
        [NAVY, NAVY, NAVY, BLUE],
    ]
    out = []
    for j in range(4):
        for i in range(4):
            out.append(
                Rect(left[i], top[j], left[i] + width, top[j] + height, radius, colours[j][i])
            )
    y = top[4]
    out.append(Rect(left[0], y, left[1] + width, y + height, radius, NAVY))  # the wide 0
    out.append(Rect(left[2], y, left[2] + width, y + height, radius, NAVY))
    out.append(Rect(left[3], y, left[3] + width, y + height, radius, GREEN))  # =
    return out


_FONT = None


def static_font():
    """Inter at TEXT_WEIGHT (and its display optical size), as a static font."""
    global _FONT
    if _FONT is None:
        font = TTFont(INTER)
        axes = {a.axisTag: a for a in font["fvar"].axes}
        location = {"wght": TEXT_WEIGHT}
        if "opsz" in axes:
            location["opsz"] = axes["opsz"].maxValue
        # The variable font's glyphs overlap their own contours (Inter's 4); merged, they make
        # clean paths that any fill rule draws the same.
        _FONT = instantiateVariableFont(font, location, overlap=OverlapMode.REMOVE)
    return _FONT


def text_path(t):
    """The display's text as one SVG path through the transform t (512 grid to output)."""
    font = static_font()
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()
    scale = TEXT_SIZE / font["head"].unitsPerEm
    names = [cmap[ord(c)] for c in TEXT_STRING]
    advance = sum(glyphs[n].width for n in names) * scale
    # Right-align the ink, not the advance: the reference's right edge is the 5's ink.
    from fontTools.pens.boundsPen import BoundsPen

    last = BoundsPen(glyphs)
    glyphs[names[-1]].draw(last)
    right_bearing = (glyphs[names[-1]].width - last.bounds[2]) * scale
    x = TEXT_RIGHT + right_bearing - advance
    pen = SVGPathPen(glyphs, ntos=fmt)
    for name in names:
        ox = x

        def transform(px, py, ox=ox):
            return t(ox + px * scale, TEXT_BASELINE - py * scale)

        glyphs[name].draw(TransformPen(pen, affine(transform)))
        x += glyphs[name].width * scale
    return pen.getCommands()


def affine(func):
    """The 6-tuple of an affine map given as a function (every transform here is affine)."""
    o, ex, ey = func(0, 0), func(1, 0), func(0, 1)
    return (ex[0] - o[0], ex[1] - o[1], ey[0] - o[0], ey[1] - o[1], o[0], o[1])


def identity(x, y):
    return (x, y)


def adaptive(x, y):
    """The 512 grid onto the adaptive icon's 108 dp canvas: the 72 dp the launcher shows."""
    s = 72 / 512
    return (18 + x * s, 18 + y * s)


# ---------------------------------------------------------------------------------------------
# Vector outputs


def svg(width, paths, background=None):
    body = []
    if background:
        body.append(f'  <rect width="{width}" height="{width}" fill="{background}"/>')
    for colour, d, rule in paths:
        extra = ' fill-rule="evenodd"' if rule == "evenodd" else ""
        body.append(f'  <path fill="{colour}"{extra} d="{d}"/>')
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{width}" '
        f'viewBox="0 0 {width} {width}">\n' + "\n".join(body) + "\n</svg>\n"
    )


def vector_drawable(comment, paths):
    body = []
    for colour, d, rule in paths:
        extra = '\n        android:fillType="evenOdd"' if rule == "evenodd" else ""
        body.append(
            f'    <path\n        android:fillColor="{colour}"{extra}\n'
            f'        android:pathData="{d}" />'
        )
    return (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        f"<!-- {comment}\n     Generated by tools/app-icon.py: edit the script, not this file. -->\n"
        '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
        '    android:width="108dp"\n    android:height="108dp"\n'
        '    android:viewportWidth="108"\n    android:viewportHeight="108">\n'
        + "\n".join(body)
        + "\n</vector>\n"
    )


def colour_paths(t):
    """The calculator in colour: body, display, keys, text."""
    paths = [(BODY_RECT.colour, BODY_RECT.path(t), "nonzero")]
    paths.append((DISPLAY_RECT.colour, DISPLAY_RECT.path(t), "nonzero"))
    for key in keys():
        paths.append((key.colour, key.path(t), "nonzero"))
    paths.append((TEXT, text_path(t), "nonzero"))
    return paths


def monochrome_paths(t, colour="#000000"):
    """One colour: the body with the display and the keys cut out, and the text in the display.

    Android tints this layer with the user's theme colour (only its alpha counts)."""
    cutouts = [DISPLAY_RECT] + keys()
    d = BODY_RECT.path(t) + "".join(r.path(t) for r in cutouts)
    return [(colour, d, "evenodd"), (colour, text_path(t), "nonzero")]


# ---------------------------------------------------------------------------------------------
# Raster outputs


def rgb(hex_colour):
    return tuple(int(hex_colour[i : i + 2], 16) for i in (1, 3, 5))


def render(size, scale=None, offset=(0.0, 0.0), background=BACKGROUND, mono=None):
    """The icon at size x size px, RGBA. `scale` is output px per 512-grid px (default: the
    512 grid fills the image), `offset` moves the grid's origin, and `mono` draws the
    monochrome silhouette in that colour instead of the colour drawing."""
    if scale is None:
        scale = size / 512
    ss = max(4, math.ceil(2048 / size))  # supersampling
    big = Image.new("RGBA", (size * ss, size * ss), (0, 0, 0, 0))
    draw = ImageDraw.Draw(big)
    if background:
        draw.rectangle((0, 0, size * ss, size * ss), fill=rgb(background) + (255,))

    def box(r):
        k = scale * ss
        return (
            (offset[0] + r.left * scale) * ss,
            (offset[1] + r.top * scale) * ss,
            (offset[0] + r.right * scale) * ss - 1,
            (offset[1] + r.bottom * scale) * ss - 1,
        ), r.radius * k

    if mono:
        # Draw the silhouette as a mask: body on, cut-outs off, text on.
        mask = Image.new("L", big.size, 0)
        md = ImageDraw.Draw(mask)
        b, rad = box(BODY_RECT)
        md.rounded_rectangle(b, rad, fill=255)
        for r in [DISPLAY_RECT] + keys():
            b, rad = box(r)
            md.rounded_rectangle(b, rad, fill=0)
        draw_text(md, scale * ss, (offset[0] * ss, offset[1] * ss), 255)
        big.paste(Image.new("RGBA", big.size, rgb(mono) + (255,)), (0, 0), mask)
    else:
        for r in [BODY_RECT, DISPLAY_RECT] + keys():
            b, rad = box(r)
            draw.rounded_rectangle(b, rad, fill=rgb(r.colour) + (255,))
        draw_text(draw, scale * ss, (offset[0] * ss, offset[1] * ss), rgb(TEXT) + (255,))
    return big.resize((size, size), Image.Resampling.BOX)


_PIL_FONT = {}


def draw_text(draw, k, origin, fill):
    """The text with FreeType from the same static Inter instance as the vector outlines."""
    key = round(TEXT_SIZE * k)
    if key not in _PIL_FONT:
        data = io.BytesIO()
        static_font().save(data)
        data.seek(0)
        _PIL_FONT[key] = ImageFont.truetype(data, key)
    font = _PIL_FONT[key]
    # Right-align the ink at TEXT_RIGHT, baseline at TEXT_BASELINE (anchor "rs" aligns the
    # advance, so measure the ink).
    ink = font.getbbox(TEXT_STRING, anchor="ls")
    x = origin[0] + TEXT_RIGHT * k - ink[2]
    draw.text((x, origin[1] + TEXT_BASELINE * k), TEXT_STRING, font=font, fill=fill, anchor="ls")


def rounded_mask(size, radius_fraction, inset=0):
    ss = 8
    m = Image.new("L", (size * ss, size * ss), 0)
    d = ImageDraw.Draw(m)
    d.rounded_rectangle(
        (inset * ss, inset * ss, (size - inset) * ss - 1, (size - inset) * ss - 1),
        radius_fraction * (size - 2 * inset) * ss,
        fill=255,
    )
    return m.resize((size, size), Image.Resampling.BOX)


def circle_mask(size, inset=0):
    ss = 8
    m = Image.new("L", (size * ss, size * ss), 0)
    ImageDraw.Draw(m).ellipse(
        (inset * ss, inset * ss, (size - inset) * ss - 1, (size - inset) * ss - 1), fill=255
    )
    return m.resize((size, size), Image.Resampling.BOX)


def masked(image, mask):
    """The image with its alpha multiplied by the mask."""
    from PIL import ImageChops

    out = image.convert("RGBA")
    out.putalpha(ImageChops.multiply(out.getchannel("A"), mask))
    return out


def save_png(image, path, mode="RGBA"):
    path.parent.mkdir(parents=True, exist_ok=True)
    image.convert(mode).save(path, optimize=True)


# The tile shape of the desktop, web and Android 7 icons: a rounded square like Android's
# rounded-square mask, 18 % corners.
TILE_RADIUS = 0.18


def tile(size, inset=0):
    """The icon as a rounded tile with transparent corners, `inset` px from each edge."""
    inner = size - 2 * inset
    icon = render(inner)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.paste(icon, (inset, inset))
    return masked(canvas, rounded_mask(size, TILE_RADIUS * inner / size, inset))


def round_tile(size, inset=0):
    inner = size - 2 * inset
    icon = render(inner)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.paste(icon, (inset, inset))
    return masked(canvas, circle_mask(size, inset))


# The desktop icon's sizes: the .ico's (build.rs) and the window icon (64).
DESKTOP_SIZES = [16, 20, 24, 32, 40, 48, 64]

ANDROID_DENSITIES = {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}


def write_sources():
    src = ROOT / "android" / "icon-source"
    (src / "icon-background.svg").write_text(
        svg(108, [], background=BACKGROUND), encoding="utf-8", newline="\n"
    )
    (src / "icon-foreground.svg").write_text(
        svg(108, colour_paths(adaptive)), encoding="utf-8", newline="\n"
    )
    (src / "icon-monochrome.svg").write_text(
        svg(108, monochrome_paths(adaptive)), encoding="utf-8", newline="\n"
    )


def write_android():
    res = ROOT / "android" / "res"
    drawable = res / "drawable"
    drawable.mkdir(parents=True, exist_ok=True)
    (drawable / "ic_launcher_foreground.xml").write_text(
        vector_drawable(
            "The adaptive icon's foreground: the calculator, inside the 66 dp safe circle.",
            colour_paths(adaptive),
        ),
        encoding="utf-8",
        newline="\n",
    )
    (drawable / "ic_launcher_monochrome.xml").write_text(
        vector_drawable(
            "The themed icon (Android 13 and later): the launcher tints this silhouette.",
            monochrome_paths(adaptive),
        ),
        encoding="utf-8",
        newline="\n",
    )
    # Android 7 (API 24, 25): PNG icons, a rounded square and a circle, 48 dp with the usual
    # 2 dp margin.
    for density, factor in ANDROID_DENSITIES.items():
        size = round(48 * factor)
        inset = round(2 * factor)
        folder = res / f"mipmap-{density}"
        save_png(tile(size, inset), folder / "ic_launcher.png")
        save_png(round_tile(size, inset), folder / "ic_launcher_round.png")
    old = res / "mipmap-xxxhdpi" / "ic_launcher_foreground.png"
    if old.exists():
        old.unlink()


def write_store():
    save_png(render(512), ROOT / "android" / "store" / "icon-512.png", mode="RGB")


def write_desktop():
    out = ROOT / "assets" / "icon"
    out.mkdir(parents=True, exist_ok=True)
    for size in DESKTOP_SIZES:
        image = tile(size)
        (out / f"icon-{size}.rgba").write_bytes(image.convert("RGBA").tobytes())
    save_png(tile(256), out / "icon-256.png")


def write_web():
    web = ROOT / "assets" / "web"
    save_png(tile(32), web / "icon-32.png")
    # iOS rounds the home screen icon itself and shows transparency as black: full-bleed.
    save_png(render(180), web / "icon-180.png", mode="RGB")
    save_png(tile(192), web / "icon-192.png")
    save_png(tile(512), web / "icon-512.png")
    # Maskable: the browser crops to its shape and keeps a circle of 80 % for sure. The
    # calculator's farthest corner is 41 % of the width from the middle, so it is drawn at 95 %.
    for size in (192, 512):
        s = size / 512 * 0.95
        o = (size - 512 * s) / 2
        save_png(render(size, s, (o, o)), web / f"icon-maskable-{size}.png", mode="RGB")


# ---------------------------------------------------------------------------------------------
# Previews


def shape_mask(shape, size):
    ss = 8
    n = size * ss
    m = Image.new("L", (n, n), 0)
    d = ImageDraw.Draw(m)
    if shape == "circle":
        d.ellipse((0, 0, n - 1, n - 1), fill=255)
    elif shape == "rounded-square":
        d.rounded_rectangle((0, 0, n - 1, n - 1), 0.2 * n, fill=255)
    elif shape == "squircle":
        pts = []
        for i in range(720):
            a = 2 * math.pi * i / 720
            c, s = math.cos(a), math.sin(a)
            x = math.copysign(abs(c) ** 0.5, c)
            y = math.copysign(abs(s) ** 0.5, s)
            pts.append(((x + 1) / 2 * (n - 1), (y + 1) / 2 * (n - 1)))
        d.polygon(pts, fill=255)
    elif shape == "teardrop":
        # A circle with the bottom right corner squared off (Android's teardrop).
        d.ellipse((0, 0, n - 1, n - 1), fill=255)
        d.rounded_rectangle(
            (n // 2, n // 2, n - 1, n - 1), 0.1 * n, fill=255, corners=(False, False, True, False)
        )
    return m.resize((size, size), Image.Resampling.BOX)


def launcher_icon(shape, size, mono=None, mono_bg=None):
    """What a launcher shows: the 72 dp middle of the 108 dp layers, cut to its mask."""
    canvas = round(size * 108 / 72)
    s = canvas / 108 * 72 / 512
    o = canvas / 108 * 18
    if mono:
        layers = render(canvas, s, (o, o), background=mono_bg, mono=mono)
    else:
        layers = render(canvas, s, (o, o))
    cut = (canvas - size) // 2
    view = layers.crop((cut, cut, cut + size, cut + size))
    return masked(view, shape_mask(shape, size))


def wallpaper(width, height, dark):
    top, bottom = ((0x1B, 0x1F, 0x2E), (0x0B, 0x0D, 0x14)) if dark else (
        (0xF4, 0xEE, 0xE6),
        (0xD9, 0xE4, 0xF2),
    )
    image = Image.new("RGBA", (width, height))
    for y in range(height):
        t = y / max(1, height - 1)
        row = tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,)
        ImageDraw.Draw(image).line((0, y, width, y), fill=row)
    return image


# Android 13's themed icons in a Material You blue scheme: light and dark.
THEMED = {"light": ("#0B1D49", "#D8E2FF"), "dark": ("#D8E2FF", "#233A6B")}


def write_previews():
    out = ROOT / "target" / "icon-previews"
    out.mkdir(parents=True, exist_ok=True)
    shapes = ["circle", "squircle", "rounded-square", "teardrop"]
    for size in (144, 48):
        gap = size // 2
        cols = len(shapes)
        rows = [("light", None), ("dark", None), ("light", "themed"), ("dark", "themed")]
        sheet = Image.new("RGBA", (gap + cols * (size + gap), len(rows) * (size + gap) + gap))
        sheet.paste(wallpaper(sheet.width, sheet.height, True), (0, 0))
        for r, (paper, look) in enumerate(rows):
            band = wallpaper(sheet.width, size + gap, paper == "dark")
            sheet.paste(band, (0, gap // 2 + r * (size + gap)))
            for c, shape in enumerate(shapes):
                if look == "themed":
                    fg, bg = THEMED[paper]
                    icon = launcher_icon(shape, size, mono=fg, mono_bg=bg)
                else:
                    icon = launcher_icon(shape, size)
                x, y = gap + c * (size + gap), gap + r * (size + gap)
                sheet.alpha_composite(icon, (x, y))
                single = wallpaper(size * 2, size * 2, paper == "dark")
                single.alpha_composite(icon, (size // 2, size // 2))
                name = f"{size}px-{shape}-{paper}{'-themed' if look else ''}.png"
                save_png(single, out / name, mode="RGB")
        sheet.paste(wallpaper(sheet.width, gap // 2, False), (0, 0))
        save_png(sheet, out / f"sheet-{size}px.png", mode="RGB")
    # The redrawing next to the reference, and their difference, at full size.
    reference = Image.open(REFERENCE).convert("RGB")
    redrawn = render(512).convert("RGB")
    compare = Image.new("RGB", (1024, 512))
    compare.paste(reference, (0, 0))
    compare.paste(redrawn, (512, 0))
    save_png(compare, out / "reference-vs-redrawn.png", mode="RGB")
    save_png(render(512, background=BACKGROUND), out / "store-icon-512.png", mode="RGB")


def difference():
    """Mean absolute difference per channel between the reference and the redrawing."""
    reference = Image.open(REFERENCE).convert("RGB").tobytes()
    redrawn = render(512).convert("RGB").tobytes()
    return sum(abs(a - b) for a, b in zip(reference, redrawn)) / len(reference)


def main():
    write_sources()
    write_android()
    write_store()
    write_desktop()
    write_web()
    write_previews()
    print(f"mean difference from the reference: {difference():.2f} / 255 per channel")


if __name__ == "__main__":
    main()
