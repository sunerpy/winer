#!/usr/bin/env python3
"""winer's mark, drawn natively at every size it ships in: a gold hexagon with a W cut out of it.

    python3 scripts/brand/icons.py            # writes app/src-tauri/icons/ and the two master SVGs
    python3 scripts/brand/icons.py --sheet    # also writes target/brand/sheet.png to look at

Needs `cairosvg` and `Pillow` (`pip install cairosvg pillow`); nothing at build time does.

Three levels of detail, because a downscale of the 512px art is a smudge at 16px:

- full (64px and up): the navy plate with its gradient, the hexagon in a gold gradient, a light
  inner rule, a slender W;
- mid (32-48px): flat colours, a bigger hexagon, a heavier W;
- small (16-24px): no plate. The hexagon fills the canvas with its vertical edges on whole pixels and
  a navy outline, so it reads on a light taskbar (gold on #f3f3f3 is 1.6:1) as well as a dark one.

The tray uses the small and mid drawings without a plate at 16, 20, 24, 32, 40 and 48px, one per
display scale, so Windows never has to resample one (`app/src-tauri/src/tray.rs`).
"""

import io
import math
import struct
import sys
from pathlib import Path

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ICONS = ROOT / "app/src-tauri/icons"

NAVY, NAVY_HI, NAVY_LO = "#0f1a26", "#1a2c42", "#0a1119"
GOLD, GOLD_HI, GOLD_LO = "#e3bd66", "#f7dc92", "#c08e34"
RULE = "#fff3c4"

ICO_SIZES = [32, 16, 20, 24, 40, 48, 64, 256]  # 32 first: dev tooling shows the first layer
TRAY_SIZES = [16, 20, 24, 32, 40, 48]


def level(size: int) -> str:
    return "full" if size >= 64 else "mid" if size >= 32 else "small"


def hexagon(r: float, cx: float = 256, cy: float = 256) -> list[tuple[float, float]]:
    """A pointy-top hexagon of circumradius `r`."""
    return [
        (cx + r * math.cos(math.radians(-90 + 60 * i)), cy + r * math.sin(math.radians(-90 + 60 * i)))
        for i in range(6)
    ]


def snapped_radius(size: int, margin_px: float, outline: float = 0) -> float:
    """The radius whose vertical edges (outline included) sit `margin_px` whole pixels in."""
    unit = 512 / size
    return ((256 - margin_px * unit) - outline / 2) / math.cos(math.radians(30))


def stroke_outline(points, width):
    """The outline of a polyline of `width` with mitred joints and square-cut (butt) ends."""
    half = width / 2

    def offset(a, b, side):
        dx, dy = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dy)
        nx, ny = -dy / length * half * side, dx / length * half * side
        return (a[0] + nx, a[1] + ny), (b[0] + nx, b[1] + ny)

    def meet(p1, p2, p3, p4):
        (x1, y1), (x2, y2), (x3, y3), (x4, y4) = p1, p2, p3, p4
        d = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
        t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / d
        return (x1 + t * (x2 - x1), y1 + t * (y2 - y1))

    sides = []
    for side in (1, -1):
        lines = [offset(points[i], points[i + 1], side) for i in range(len(points) - 1)]
        edge = [lines[0][0]]
        edge += [meet(*lines[i], *lines[i + 1]) for i in range(len(lines) - 1)]
        edge.append(lines[-1][1])
        sides.append(edge)
    return sides[0] + sides[1][::-1]


def d(points) -> str:
    return "M" + " L".join(f"{x:.1f},{y:.1f}" for x, y in points) + " Z"


# The W, per level: its five points and its stroke. Heavier as the canvas shrinks.
W = {
    "full": ([(170, 208), (210, 328), (256, 252), (302, 328), (342, 208)], 46),
    "mid": ([(162, 204), (206, 334), (256, 254), (306, 334), (350, 204)], 58),
    "small": ([(150, 196), (200, 336), (256, 252), (312, 336), (362, 196)], 70),
}
OUTLINE = {"small": 34, "mid": 22}


def mark(size: int, plate: bool) -> str:
    """The SVG for one size; `plate` is false for the tray."""
    lv = level(size)
    w_points, w_width = W[lv]
    w_path = d(stroke_outline(w_points, w_width))
    defs = (
        f'<linearGradient id="plate" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{NAVY_HI}"/>'
        f'<stop offset="1" stop-color="{NAVY_LO}"/></linearGradient>'
        f'<linearGradient id="gold" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{GOLD_HI}"/>'
        f'<stop offset="1" stop-color="{GOLD_LO}"/></linearGradient>'
    )
    body = ""
    if plate and lv != "small":
        fill = "url(#plate)" if lv == "full" else NAVY
        body += f'<rect width="512" height="512" rx="112" fill="{fill}"/>'
        r = 182 if lv == "full" else 205
        gold = "url(#gold)" if lv == "full" else GOLD
        body += f'<path d="{d(hexagon(r))}" fill="{gold}"/>'
        if lv == "full":
            body += f'<path d="{d(hexagon(r - 14))}" fill="none" stroke="{RULE}" stroke-opacity=".35" stroke-width="6"/>'
    else:
        # No plate: the outlined hexagon is the whole icon, its vertical edges on pixel boundaries.
        outline = OUTLINE["small" if lv == "small" else "mid"]
        r = snapped_radius(size, 1 if size <= 20 else 2, outline)
        body += (
            f'<path d="{d(hexagon(r))}" fill="{GOLD}" stroke="{NAVY}" stroke-width="{outline}" '
            'stroke-linejoin="round"/>'
        )
    body += f'<path d="{w_path}" fill="{NAVY}"/>'
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 512 512">'
        f"<defs>{defs}</defs>{body}</svg>"
    )


def render(svg: str, size: int) -> Image.Image:
    png = cairosvg.svg2png(bytestring=svg.encode(), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def png_bytes(image: Image.Image) -> bytes:
    out = io.BytesIO()
    image.save(out, format="PNG", optimize=True)
    return out.getvalue()


def write_ico(path: Path, sizes: list[int]) -> None:
    """Writes the directory by hand: Pillow reorders layers and silently drops some."""
    images = [png_bytes(render(mark(size, plate=size >= 32), size)) for size in sizes]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, blobs = b"", b""
    for size, blob in zip(sizes, images):
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset)
        blobs += blob
        offset += len(blob)
    path.write_bytes(header + entries + blobs)
    raw = path.read_bytes()
    count = struct.unpack("<H", raw[4:6])[0]
    written = [raw[6 + i * 16] or 256 for i in range(count)]
    assert written == sizes, written


def master(plate: bool, comment: str) -> str:
    """The 512-unit master: the full drawing on its plate, or the plate-less mid drawing."""
    size = 512 if plate else 48
    svg = mark(size, plate).replace(f'width="{size}" height="{size}"', 'width="512" height="512"')
    return svg.replace("<defs>", f"<!-- {comment} --><defs>", 1)


def main() -> None:
    ICONS.mkdir(parents=True, exist_ok=True)
    write_ico(ICONS / "icon.ico", ICO_SIZES)
    for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)):
        render(mark(size, plate=True), size).save(ICONS / name, optimize=True)
    for size in TRAY_SIZES:
        (ICONS / f"tray-{size}.rgba").write_bytes(render(mark(size, plate=False), size).tobytes())
    (ROOT / "app/src-tauri/app-icon.svg").write_text(
        master(True, "Generated by scripts/brand/icons.py: edit the script, not this file.") + "\n"
    )
    (ROOT / "app/src-tauri/tray-icon.svg").write_text(
        master(False, "Generated by scripts/brand/icons.py: the plate-less mark the tray and small sizes use.")
        + "\n"
    )
    print(f"wrote {ICONS.relative_to(ROOT)}: icon.ico {ICO_SIZES}, png 32/128/256/512, tray {TRAY_SIZES}")
    if "--sheet" in sys.argv:
        sheet()


def sheet() -> None:
    """Every shipped size at its real pixels, then the tray glyphs on a light and a dark taskbar,
    the small ones also magnified three times."""
    out = Image.new("RGBA", (1100, 420), (255, 255, 255, 255))
    x = 16
    for size in (256, 64, 48, 32, 24, 20, 16):
        out.alpha_composite(render(mark(size, plate=size >= 32), size), (x, 16))
        x += size + 16
    for row, bg in enumerate(((243, 243, 243, 255), (32, 32, 32, 255))):
        strip = Image.new("RGBA", (1068, 96), bg)
        sx = 12
        for size in TRAY_SIZES:
            glyph = render(mark(size, plate=False), size)
            strip.alpha_composite(glyph, (sx, (96 - size) // 2))
            sx += size + 12
            if size <= 24:
                big = glyph.resize((size * 3, size * 3), Image.NEAREST)
                strip.alpha_composite(big, (sx, (96 - big.height) // 2))
                sx += big.width + 20
        out.alpha_composite(strip, (16, 196 + row * 108))
    target = ROOT / "target/brand"
    target.mkdir(parents=True, exist_ok=True)
    out.convert("RGB").save(target / "sheet.png")
    print(f"wrote {target.relative_to(ROOT)}/sheet.png")


if __name__ == "__main__":
    main()
