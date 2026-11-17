# /// script
# requires-python = ">=3.10"
# dependencies = ["fonttools==4.67.0", "skia-pathops==0.9.2"]
# ///
"""Builds assets/fonts/CalcCraftIcons.ttf, the icon font embedded in the app.

The font is a subset of Microsoft's Fluent System Icons (MIT license), the
"Resizable" font, which has the 20 px design of every icon. Each icon is put at
the code point of the Segoe Fluent Icons glyph that the Windows Calculator uses
for it, so the app's `ICON_*` constants (src/app/icons.rs) keep Windows' code
points. Fluent's 20 px grid has a 2 px margin, so the glyphs are scaled by 1.25
to fill the em like Segoe Fluent Icons. Fluent System Icons has no ÷, = or angle,
so those are put together here from Fluent's own minus stroke.

Run it from the repository root (uv installs fontTools):

    uv run tools/icon-font.py

It downloads the font from the pinned commit below. To use files you already
have, pass the paths of FluentSystemIcons-Resizable.ttf and its .json.
"""

import json
import math
import sys
import tempfile
import urllib.request
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.recordingPen import RecordingPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.ttLib.removeOverlaps import removeOverlaps

COMMIT = "a347af95478f989038cccbf085978865264c048a"
SOURCE = f"https://raw.githubusercontent.com/microsoft/fluentui-system-icons/{COMMIT}/fonts"
OUTPUT = Path(__file__).resolve().parent.parent / "assets" / "fonts" / "CalcCraftIcons.ttf"

# Segoe Fluent Icons code point -> Fluent System Icons name (the 20 px regular
# icon, or the filled one with "!").
ICONS = {
    0xE700: "navigation",  # GlobalNavigationButton
    0xE70D: "chevron_down",
    0xE70E: "chevron_up",
    0xE713: "settings",
    0xE72B: "arrow_left",  # Back
    0xE72D: "share",
    0xE74D: "delete",
    0xE751: "arrow_enter_left",  # ReturnKey
    0xE76B: "chevron_left",
    0xE76C: "chevron_right",
    0xE77F: "clipboard_paste",
    0xE787: "calendar_ltr",
    0xE790: "color",  # the App theme card
    0xE7A7: "arrow_undo",
    0xE7BA: "warning",
    0xE805: "top_speed",  # Speed
    0xE809: "table",  # Area
    0xE81C: "history",
    0xE821: "scales",  # Weight and mass
    0xE890: "eye",  # View
    0xE8C6: "cut",
    0xE8C8: "copy",
    0xE8EF: "calculator",
    0xE909: "globe",  # the Regional format card
    0xE917: "clock",  # Time
    0xE943: "code",  # the Programmer mode
    0xE945: "flash",  # Power, and the function analysis
    0xE947: "dismiss",  # ×
    0xE948: "add",
    0xE949: "subtract",
    0xE94F: "backspace",
    0xE9CA: "temperature",
    0xEB52: "heart!",  # HeartFill: the support prompt's thank-you line
    0xEC4A: "gauge",  # Pressure
    0xECAD: "fire",  # Energy
    0xECC6: "ruler",  # Length
    0xECC9: "subtract_circle",  # remove an equation
    0xED1A: "eye_off",  # Hide
    0xEDA2: "storage",  # Data
    0xEDDB: "triangle_up!",  # the calendar's CaretUpSolid8
    0xEDDC: "triangle_down!",
    0xEE47: "picture_in_picture_exit",  # Back to full view
    0xEE49: "picture_in_picture_enter",  # Keep on top
    0xF158: "cube",  # Volume
    0xF196: "beaker",  # the Scientific mode
    0xF2B7: "local_language",  # the Language card
}
DIVIDE = 0xE94A
EQUALS = 0xE94E
ANGLE = 0xF461

UPM = 2000
# Fluent's units (500 to the em) to ours, with the 1.25 scale around the center.
SCALE = UPM / 500 * 1.25
SHIFT = UPM / 2 - 250 * SCALE


def fetch(name: str, folder: Path) -> Path:
    path = folder / name
    urllib.request.urlretrieve(f"{SOURCE}/{name}", path)
    return path


def record(glyph_set, name: str, matrix=(SCALE, 0, 0, SCALE, SHIFT, SHIFT)) -> RecordingPen:
    pen = RecordingPen()
    glyph_set[name].draw(TransformPen(pen, matrix))
    return pen


def bounds(pen: RecordingPen):
    bounds_pen = BoundsPen(None)
    pen.replay(bounds_pen)
    return bounds_pen.bounds


def main() -> None:
    if len(sys.argv) == 3:
        ttf, names = Path(sys.argv[1]), Path(sys.argv[2])
    else:
        folder = Path(tempfile.mkdtemp())
        ttf = fetch("FluentSystemIcons-Resizable.ttf", folder)
        names = fetch("FluentSystemIcons-Resizable.json", folder)

    source = TTFont(ttf)
    assert source["head"].unitsPerEm == 500, "the scale assumes a 500 unit em"
    glyph_set = source.getGlyphSet()
    cmap = source.getBestCmap()
    code_points = json.loads(names.read_text())

    def source_glyph(icon: str) -> str:
        style = "filled" if icon.endswith("!") else "regular"
        return cmap[code_points[f"ic_fluent_{icon.rstrip('!')}_20_{style}"]]

    outlines = {cp: record(glyph_set, source_glyph(icon)) for cp, icon in ICONS.items()}

    # ÷, = and the angle from the minus stroke, placed like Segoe Fluent Icons' glyphs.
    minus = source_glyph("subtract")
    x0, y0, x1, y1 = bounds(outlines[0xE949])
    stroke, length = y1 - y0, x1 - x0
    center = UPM / 2

    def bar(dx=0.0, dy=0.0, angle=0.0, stretch=1.0) -> RecordingPen:
        """The minus stroke, stretched along its length, turned and moved."""
        c, s = math.cos(angle), math.sin(angle)
        # Scale around the em's center, then rotate around it, then move.
        a, b, cc, d = SCALE * stretch * c, SCALE * stretch * s, -SCALE * s, SCALE * c
        e = center - (a * 250 + cc * 250) + dx
        f = center - (b * 250 + d * 250) + dy
        return record(glyph_set, minus, (a, b, cc, d, e, f))

    def dot(cx: float, cy: float, r: float) -> RecordingPen:
        """A filled circle, from eight quadratic arcs."""
        pen = RecordingPen()
        k = r / math.cos(math.pi / 8)
        points = [
            (cx + k * math.cos(-i * math.pi / 4), cy + k * math.sin(-i * math.pi / 4))
            for i in range(8)
        ]
        pen.qCurveTo(*points, None)
        pen.closePath()
        return pen

    def combine(*pens: RecordingPen) -> RecordingPen:
        pen = RecordingPen()
        for part in pens:
            pen.value.extend(part.value)
        return pen

    gap = 0.15625 * UPM  # half the distance between ='s bars
    outlines[EQUALS] = combine(bar(dy=gap), bar(dy=-gap))
    dots = 0.28125 * UPM  # the dots' distance from ÷'s bar
    outlines[DIVIDE] = combine(
        bar(),
        dot(center, center + dots, 0.0625 * UPM),
        dot(center, center - dots, 0.0625 * UPM),
    )
    # The angle: two legs from an apex at the top to feet at the bottom corners.
    apex = (center, 0.84 * UPM)
    feet_y, half_width = 0.16 * UPM, 0.355 * UPM
    leg = math.hypot(half_width, apex[1] - feet_y)
    stretch = (leg + stroke) / length
    tilt = math.atan2(apex[1] - feet_y, half_width)
    middle_y = (apex[1] + feet_y) / 2 - center
    outlines[ANGLE] = combine(
        bar(-half_width / 2, middle_y, tilt, stretch),
        bar(half_width / 2, middle_y, -tilt, stretch),
    )

    order = [".notdef"] + [f"uni{cp:04X}" for cp in sorted(outlines)]
    glyphs = {".notdef": TTGlyphPen(None).glyph()}
    metrics = {".notdef": (UPM, 0)}
    for cp, outline in outlines.items():
        pen = TTGlyphPen(None)
        outline.replay(pen)
        name = f"uni{cp:04X}"
        glyphs[name] = pen.glyph()
        metrics[name] = (UPM, round(bounds(outline)[0]))

    builder = FontBuilder(UPM, isTTF=True)
    builder.setupGlyphOrder(order)
    builder.setupCharacterMap({cp: f"uni{cp:04X}" for cp in outlines})
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics(metrics)
    # Like Segoe Fluent Icons: the line is one em, the icon fills it.
    builder.setupHorizontalHeader(ascent=UPM, descent=0)
    builder.setupOS2(
        sTypoAscender=UPM,
        sTypoDescender=0,
        sTypoLineGap=0,
        usWinAscent=UPM,
        usWinDescent=0,
    )
    builder.setupNameTable(
        {
            "copyright": "Copyright (c) 2020 Microsoft Corporation",
            "familyName": "CalcCraft Icons",
            "styleName": "Regular",
            "uniqueFontIdentifier": "CalcCraft Icons",
            "fullName": "CalcCraft Icons",
            "version": "Version 1.000",
            "psName": "CalcCraftIcons",
            "description": f"A subset of Fluent System Icons, commit {COMMIT}",
            "licenseDescription": "MIT License",
            "licenseInfoURL": "https://github.com/microsoft/fluentui-system-icons/blob/main/LICENSE",
        }
    )
    builder.setupPost()
    # The angle's legs overlap at the apex.
    removeOverlaps(builder.font, [f"uni{ANGLE:04X}", f"uni{DIVIDE:04X}", f"uni{EQUALS:04X}"])
    glyf, hmtx = builder.font["glyf"], builder.font["hmtx"]
    for name in order:
        glyf[name].recalcBounds(glyf)
        hmtx[name] = (UPM, getattr(glyf[name], "xMin", 0))
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    builder.save(OUTPUT)
    print(f"{OUTPUT}: {len(outlines)} icons, {OUTPUT.stat().st_size} bytes")


if __name__ == "__main__":
    main()
