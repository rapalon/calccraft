# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow==12.0.0"]
# ///
"""Builds every app icon from Konst's two PNGs in android/icon-source/.

- calccraft-icon-512.png: the square icon (512 x 512, opaque): the calculator with "42" on
  blue, cut off at the bottom. It is Google Play's listing icon as it is, and the source of
  the square icons (Android 7, desktop, web).
- calccraft-foreground.png: the same calculator on a transparent background, for the
  adaptive icon's foreground and its monochrome (themed icon) silhouette.

The artwork is raster, so nothing is redrawn: the images are only scaled, placed and masked.
The adaptive foreground puts the calculator where the square icon has it (the 512 px square
is the 72 dp a launcher shows of the 108 dp canvas), which keeps the display and the top key
rows inside the 66 dp safe circle; like the square, the calculator runs off the bottom.

It writes:
- android/res/: the adaptive icon's foreground and monochrome layers (drawable-*dpi), the
  background colour (values/colors.xml's ic_launcher_background, sampled from the square
  icon), and the square and round PNG icons of Android 7 (mipmap-*dpi, API 24 and 25);
- android/store/icon-512.png: a copy of the square icon;
- assets/icon/: the desktop icons (src/icon.rs: the window icon and build.rs's .ico) as
  straight-alpha RGBA, and a 256 px PNG for the .ico's largest image;
- assets/web/: the browser's tab icon, the iOS home screen icon and the PWA's icons;
- target/icon-preview/ (not committed): the launcher icon in the launchers' masks at 48 dp
  and 108 dp, and the themed (monochrome) look on light and dark.

Run it from the repository root after changing the artwork:

    uv run tools/app-icon.py

CalcCraft Pro (the play-paid app, com.rapalon.calccraft.pro) has its own artwork in
android/icon-source/pro/ (calccraft-pro-icon-512.png and calccraft-pro-foreground.png: the same
calculator with a gold key, placed as in the free icon). With --pro the same steps write:
- android/res-pro/: the Pro launcher icon's layers, Android 7 icons and background colour, which
  build-apk.ps1 lays over android/res/ for play-paid only (aapt2's -R overlay);
- android/store/pro/icon-512.png: a copy of the Pro square icon;
- target/icon-preview-pro/ (not committed): the previews.
The free app's icons, the desktop and the web icons stay the free artwork's.

    uv run tools/app-icon.py --pro
"""

import math
import re
import shutil
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
# --pro: CalcCraft Pro's artwork and outputs (see above).
PRO = "--pro" in sys.argv[1:]
SOURCE = ROOT / "android" / "icon-source" / ("pro" if PRO else "")
SQUARE = SOURCE / ("calccraft-pro-icon-512.png" if PRO else "calccraft-icon-512.png")
FOREGROUND = SOURCE / ("calccraft-pro-foreground.png" if PRO else "calccraft-foreground.png")
RES = ROOT / "android" / ("res-pro" if PRO else "res")
STORE = ROOT / "android" / "store" / ("pro" if PRO else "") / "icon-512.png"
PREVIEW = ROOT / "target" / ("icon-preview-pro" if PRO else "icon-preview")

# Where the square icon has the calculator: its body's left and right edges and its top,
# measured on the 512 px square. The foreground PNG is the same body, edge to edge.
SQUARE_BODY_LEFT = 66
SQUARE_BODY_RIGHT = 446
SQUARE_BODY_TOP = 85

ANDROID_DENSITIES = {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}

# The tile shape of the desktop, web and Android 7 icons: a rounded square, 18 % corners.
TILE_RADIUS = 0.18

# The desktop icon's sizes: the .ico's (build.rs) and the window icon (64).
DESKTOP_SIZES = [16, 20, 24, 32, 40, 48, 64]


def square():
    return Image.open(SQUARE).convert("RGBA")


def foreground_art():
    return Image.open(FOREGROUND).convert("RGBA")


def background_colour():
    """The square icon's blue: the most common colour of its top edge."""
    im = Image.open(SQUARE).convert("RGB")
    row = [im.getpixel((x, 0)) for x in range(im.width)]
    return max(set(row), key=row.count)


def hex_colour(rgb):
    return "#%02X%02X%02X" % rgb


def scaled(image, size):
    """Downscaled with Lanczos (both PNGs are larger than any size written)."""
    return image.resize(size, Image.Resampling.LANCZOS)


def adaptive_layer(art, px):
    """`art` (the foreground PNG or its silhouette) on the 108 dp canvas of `px` pixels, where
    the square icon has the calculator."""
    dp = px / 108
    s = 72 / 512 * dp  # canvas pixels per square-icon pixel
    width = (SQUARE_BODY_RIGHT - SQUARE_BODY_LEFT) * s
    height = width * art.height / art.width
    x = 18 * dp + SQUARE_BODY_LEFT * s
    y = 18 * dp + SQUARE_BODY_TOP * s
    # Supersample the placement so the sub-pixel position is kept.
    ss = 4
    big = Image.new("RGBA", (px * ss, px * ss), (0, 0, 0, 0))
    piece = scaled(art, (round(width * ss), round(height * ss)))
    big.alpha_composite(piece, (round(x * ss), round(y * ss)))
    return big.resize((px, px), Image.Resampling.BOX)


def silhouette(art):
    """The themed icon's one-colour picture, from the foreground PNG: the calculator's body,
    with the display and the keys cut out and the display's digits kept. White, opaque where
    drawn (Android uses only its alpha and tints it)."""
    r, g, b, a = art.split()
    body = a.point(lambda v: 255 if v >= 128 else 0)
    lum = Image.merge("RGB", (r, g, b)).convert("L")
    dark = lum.point(lambda v: 255 if v < 105 else 0)
    # The green key: clearly greener than red and blue.
    green = ImageChops.subtract(g, ImageChops.lighter(r, b)).point(lambda v: 255 if v > 15 else 0)
    holes = ImageChops.lighter(dark, green)
    if PRO:
        # The Pro icon's gold key (#E6C980 to #B99148): far more red than blue, and green
        # well above blue too, which the grey body and the white keys are not.
        warm = ImageChops.subtract(r, b).point(lambda v: 255 if v > 60 else 0)
        yellow = ImageChops.subtract(g, b).point(lambda v: 255 if v > 30 else 0)
        holes = ImageChops.lighter(holes, ImageChops.multiply(warm, yellow))
    # Fill the keys' gloss highlights and the green key's light gradient (a closing as wide as
    # a highlight: the keys are some 180 px), then drop specks.
    holes = holes.filter(ImageFilter.MaxFilter(25)).filter(ImageFilter.MinFilter(25))
    holes = holes.filter(ImageFilter.MinFilter(5)).filter(ImageFilter.MaxFilter(5))
    # The digits: light pixels inside the display's dark area.
    display = holes.crop((0, 0, art.width, art.height // 4))
    digits_full = Image.new("L", art.size, 0)
    digits_full.paste(display, (0, 0))
    digits = ImageChops.multiply(
        lum.point(lambda v: 255 if v > 170 else 0),
        digits_full.filter(ImageFilter.MinFilter(9)),
    )
    shape = ImageChops.lighter(ImageChops.subtract(body, holes), digits)
    shape = shape.filter(ImageFilter.GaussianBlur(1.2)).point(lambda v: 255 if v >= 128 else 0)
    out = Image.new("RGBA", art.size, (255, 255, 255, 0))
    out.putalpha(shape)
    return out


def rounded_mask(size, radius_fraction, inset=0):
    ss = 8
    m = Image.new("L", (size * ss, size * ss), 0)
    ImageDraw.Draw(m).rounded_rectangle(
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
    out = image.convert("RGBA")
    out.putalpha(ImageChops.multiply(out.getchannel("A"), mask))
    return out


def tile(size, inset=0, shape="rounded"):
    """The square icon as a rounded (or round) tile, `inset` px from each edge."""
    inner = size - 2 * inset
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.paste(scaled(square(), (inner, inner)), (inset, inset))
    mask = circle_mask(size, inset) if shape == "round" else rounded_mask(
        size, TILE_RADIUS * inner / size, inset
    )
    return masked(canvas, mask)


def save_png(image, path, mode="RGBA"):
    path.parent.mkdir(parents=True, exist_ok=True)
    image.convert(mode).save(path, optimize=True)


def write_android():
    res = RES
    art = foreground_art()
    mono = silhouette(art)
    for density, factor in ANDROID_DENSITIES.items():
        px = round(108 * factor)
        save_png(adaptive_layer(art, px), res / f"drawable-{density}" / "ic_launcher_foreground.png")
        save_png(adaptive_layer(mono, px), res / f"drawable-{density}" / "ic_launcher_monochrome.png")
        # Android 7 (API 24, 25): PNG icons from the square one, 48 dp with the usual 2 dp
        # margin, a rounded square and a circle.
        size = round(48 * factor)
        inset = round(2 * factor)
        save_png(tile(size, inset), res / f"mipmap-{density}" / "ic_launcher.png")
        save_png(tile(size, inset, "round"), res / f"mipmap-{density}" / "ic_launcher_round.png")
    # The vector layers of the previous icon.
    for stale in ("ic_launcher_foreground.xml", "ic_launcher_monochrome.xml"):
        path = res / "drawable" / stale
        if path.exists():
            path.unlink()
    colors = res / "values" / "colors.xml"
    if PRO and not colors.exists():
        colors.parent.mkdir(parents=True, exist_ok=True)
        colors.write_text(PRO_COLORS, encoding="utf-8", newline="\n")
    text = colors.read_text(encoding="utf-8")
    text = re.sub(
        r'(<color name="ic_launcher_background">)#[0-9A-Fa-f]{6}(</color>)',
        rf"\g<1>{hex_colour(background_colour())}\g<2>",
        text,
    )
    colors.write_text(text, encoding="utf-8", newline="\n")


# android/res-pro/values/colors.xml when it doesn't exist yet; its colour is then sampled.
PRO_COLORS = """<?xml version="1.0" encoding="utf-8"?>
<resources>
    <!-- CalcCraft Pro's launcher icon background, sampled from
         android/icon-source/pro/calccraft-pro-icon-512.png by tools/app-icon.py --pro, which
         rewrites this value. build-apk.ps1 lays android/res-pro/ over android/res/ for the
         play-paid app only. -->
    <color name="ic_launcher_background">#000000</color>
</resources>
"""


def write_store():
    store = STORE
    store.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(SQUARE, store)


def write_desktop():
    out = ROOT / "assets" / "icon"
    out.mkdir(parents=True, exist_ok=True)
    for size in DESKTOP_SIZES:
        (out / f"icon-{size}.rgba").write_bytes(tile(size).convert("RGBA").tobytes())
    save_png(tile(256), out / "icon-256.png")


def write_web():
    web = ROOT / "assets" / "web"
    save_png(tile(32), web / "icon-32.png")
    # iOS rounds the home screen icon itself and shows transparency as black: full-bleed.
    save_png(scaled(square(), (180, 180)), web / "icon-180.png", mode="RGB")
    save_png(tile(192), web / "icon-192.png")
    save_png(tile(512), web / "icon-512.png")
    # Maskable: the browser crops to its shape and keeps a circle of 80 % for sure; the
    # square icon's display and keys are inside it as they are.
    for size in (192, 512):
        save_png(scaled(square(), (size, size)), web / f"icon-maskable-{size}.png", mode="RGB")


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
    return m.resize((size, size), Image.Resampling.BOX)


def launcher_icon(shape, size, mono=None):
    """What a launcher shows at `size` px: the 72 dp middle of the 108 dp layers, cut to its
    mask. `mono` is a (foreground, background) colour pair for the themed look."""
    canvas = round(size * 108 / 72)
    if mono:
        fg_colour, bg_colour = mono
        layer = adaptive_layer(silhouette(foreground_art()), canvas)
        tinted = Image.new("RGBA", layer.size, fg_colour)
        tinted.putalpha(layer.getchannel("A"))
        base = Image.new("RGBA", layer.size, bg_colour)
    else:
        layer = adaptive_layer(foreground_art(), canvas)
        tinted = layer
        base = Image.new("RGBA", layer.size, background_colour() + (255,))
    base.alpha_composite(tinted)
    cut = (canvas - size) // 2
    view = base.crop((cut, cut, cut + size, cut + size))
    return masked(view, shape_mask(shape, size))


def wallpaper(width, height, dark):
    top, bottom = ((0x1B, 0x1F, 0x2E), (0x0B, 0x0D, 0x14)) if dark else (
        (0xF4, 0xEE, 0xE6),
        (0xD9, 0xE4, 0xF2),
    )
    image = Image.new("RGBA", (width, height))
    draw = ImageDraw.Draw(image)
    for y in range(height):
        t = y / max(1, height - 1)
        draw.line((0, y, width, y), fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,))
    return image


# Android 13's themed icons in a Material You blue scheme: light and dark.
THEMED = {"light": ("#0B1D49", "#D8E2FF"), "dark": ("#D8E2FF", "#233A6B")}


def write_previews():
    out = PREVIEW
    out.mkdir(parents=True, exist_ok=True)
    shapes = ["circle", "squircle", "rounded-square"]
    # 48 dp and 108 dp at xxhdpi (3 px a dp).
    for dp in (48, 108):
        size = dp * 3
        for shape in shapes:
            for paper in ("light", "dark"):
                for look in (None, "themed"):
                    icon = launcher_icon(shape, size, THEMED[paper] if look else None)
                    tile_image = wallpaper(size * 2, size * 2, paper == "dark")
                    tile_image.alpha_composite(icon, (size // 2, size // 2))
                    name = f"{dp}dp-{shape}-{paper}{'-themed' if look else ''}.png"
                    save_png(tile_image, out / name, mode="RGB")
    # One sheet of all of them at 48 dp.
    size = 144
    gap = size // 2
    rows = [("light", None), ("dark", None), ("light", "themed"), ("dark", "themed")]
    sheet = wallpaper(gap + len(shapes) * (size + gap), gap + len(rows) * (size + gap), True)
    for r, (paper, look) in enumerate(rows):
        band = wallpaper(sheet.width, size + gap, paper == "dark")
        sheet.paste(band, (0, gap // 2 + r * (size + gap)))
        for c, shape in enumerate(shapes):
            icon = launcher_icon(shape, size, THEMED[paper] if look else None)
            sheet.alpha_composite(icon, (gap + c * (size + gap), gap + r * (size + gap)))
    save_png(sheet, out / "sheet-48dp.png", mode="RGB")
    # The bare layers, for checking the safe zone: the 108 dp canvas with the 72 dp view and
    # the 66 dp safe circle drawn on it.
    canvas = 432
    layers = Image.new("RGBA", (canvas, canvas), background_colour() + (255,))
    layers.alpha_composite(adaptive_layer(foreground_art(), canvas))
    d = ImageDraw.Draw(layers)
    k = canvas / 108
    d.rectangle((18 * k, 18 * k, 90 * k, 90 * k), outline=(255, 64, 64, 255), width=2)
    d.ellipse((21 * k, 21 * k, 87 * k, 87 * k), outline=(255, 220, 0, 255), width=2)
    save_png(layers, out / "adaptive-layers-108dp.png", mode="RGB")
    mono = Image.new("RGBA", (canvas, canvas), (0x80, 0x80, 0x80, 255))
    mono.alpha_composite(adaptive_layer(silhouette(foreground_art()), canvas))
    save_png(mono, out / "monochrome-layer-108dp.png", mode="RGB")


def main():
    write_android()
    write_store()
    # The desktop and web icons are the free artwork's only.
    if not PRO:
        write_desktop()
        write_web()
    write_previews()
    print("background", hex_colour(background_colour()))


if __name__ == "__main__":
    main()
