"""Render the Material Notepad icon set.

Every icon is a Material You style document: a sheet with a folded corner on a
tonal rounded square. File-type icons add a coloured badge in the bottom-right
corner with the extension written on it; each type gets its own colour.

Outputs:
  assets/icon.ico                      application icon (no badge)
  assets/icons/<type>.ico              one icon per supported text extension
  assets/icons/preview.png             contact sheet used by the README

Run with `python tools/make_icons.py` (needs Pillow).
"""

from __future__ import annotations

import pathlib

from PIL import Image, ImageDraw, ImageFont

ROOT = pathlib.Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "assets" / "icons"
APP_ICON = ROOT / "assets" / "icon.ico"
PREVIEW = ICON_DIR / "preview.png"
BOLD_FONT = ROOT / "assets" / "fonts" / "Roboto-Bold.ttf"

BACKGROUND = (103, 80, 164, 255)  # Material You primary
EDGE = (202, 196, 208, 255)      # sheet outline, visible on light backgrounds
SHEET = (255, 255, 255, 255)
SHEET_FOLD = (222, 210, 250, 255)
SHEET_LINE = (103, 80, 164, 110)

# One colour per extension, distinct on the purple background.
TYPE_COLORS: dict[str, tuple[int, int, int, int]] = {
    "txt": (11, 87, 208, 255),    # blue
    "cfg": (179, 38, 30, 255),    # red
    "json": (230, 108, 10, 255),  # orange
    "md": (123, 31, 162, 255),    # purple
    "ini": (0, 121, 107, 255),    # teal
    "log": (95, 99, 104, 255),    # grey
    "csv": (24, 128, 56, 255),    # green
}

ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]


def _arc(cx: float, cy: float, r: float, start: float, end: float, steps: int = 10):
    """Points along a quarter arc, angles in radians."""
    import math

    return [
        (cx + r * math.cos(start + (end - start) * i / steps),
         cy + r * math.sin(start + (end - start) * i / steps))
        for i in range(steps + 1)
    ]


def _document_points(
    left: float, top: float, right: float, bottom: float, radius: float, fold: float
):
    """Outline of a sheet whose top-right corner is folded away."""
    import math

    points: list[tuple[float, float]] = []
    points.append((left + radius, top))
    points.append((right - fold, top))
    points.append((right, top + fold))
    points.append((right, bottom - radius))
    points += _arc(right - radius, bottom - radius, radius, 0.0, math.pi / 2)
    points.append((left + radius, bottom))
    points += _arc(left + radius, bottom - radius, radius, math.pi / 2, math.pi)
    points.append((left, top + radius))
    points += _arc(left + radius, top + radius, radius, math.pi, 3 * math.pi / 2)
    return points


def draw(size: int, badge: tuple[str, tuple[int, int, int, int]] | None) -> Image.Image:
    """A document that fills the canvas: no container, no padding around the art."""
    scale = 8 if size <= 96 else 4
    big = size * scale
    image = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ctx = ImageDraw.Draw(image)

    fold = big * 0.22
    left = big * 0.15
    top = big * 0.02
    right = big * 0.87
    bottom = big * 0.98
    radius = big * 0.075
    stroke = max(1, int(big * 0.012))

    outline = _document_points(left, top, right, bottom, radius, fold)
    ctx.polygon(outline, fill=SHEET)

    # Folded flap: the triangle inside the cut corner.
    flap = [(right - fold, top), (right, top + fold), (right - fold, top + fold)]
    ctx.polygon(flap, fill=SHEET_FOLD)

    # Edges: the document outline plus the fold crease.
    ctx.line(outline + [outline[0]], fill=EDGE, width=stroke, joint="curve")
    ctx.line(
        [(right - fold, top), (right, top + fold), (right - fold, top + fold)],
        fill=EDGE,
        width=stroke,
        joint="curve",
    )

    # Text lines on the sheet.
    line_height = max(1, int(big * 0.058))
    inner_left = left + big * 0.10
    inner_right = right - big * 0.10
    widths = (0.90, 0.60, 0.78)
    line_tops = (0.24, 0.40, 0.56)
    for factor, top_factor in zip(widths, line_tops):
        y = top + (bottom - top) * top_factor
        if y + line_height > bottom - big * 0.05:
            break
        ctx.rounded_rectangle(
            (inner_left, y, inner_left + (inner_right - inner_left) * factor, y + line_height),
            radius=line_height / 2,
            fill=SHEET_LINE,
        )

    if badge is not None:
        label, color = badge
        badge_width = big * 0.58
        badge_height = big * 0.27
        badge_right = big * 0.99
        badge_bottom = big * 0.99
        badge_left = badge_right - badge_width
        badge_top = badge_bottom - badge_height
        ctx.rounded_rectangle(
            (badge_left, badge_top, badge_right, badge_bottom),
            radius=big * 0.08,
            fill=color,
        )
        # Fit the label into the badge.
        target_width = badge_width * 0.84
        font_size = int(badge_height * 0.66)
        while font_size > 4:
            font = ImageFont.truetype(str(BOLD_FONT), font_size)
            if ctx.textlength(label, font=font) <= target_width:
                break
            font_size -= 1
        font = ImageFont.truetype(str(BOLD_FONT), font_size)
        bbox = ctx.textbbox((0, 0), label, font=font)
        text_width = bbox[2] - bbox[0]
        text_height = bbox[3] - bbox[1]
        ctx.text(
            (
                badge_left + (badge_width - text_width) / 2 - bbox[0],
                badge_top + (badge_height - text_height) / 2 - bbox[1],
            ),
            label,
            font=font,
            fill=(255, 255, 255, 255),
        )

    return image.resize((size, size), Image.LANCZOS)


def save_ico(path: pathlib.Path, badge: tuple[str, tuple[int, int, int, int]] | None) -> None:
    frames = [draw(size, badge) for size in ICO_SIZES]
    frames[-1].save(
        path,
        format="ICO",
        sizes=[(size, size) for size in ICO_SIZES],
        append_images=frames[:-1],
    )


def save_preview() -> None:
    cell = 160
    padding = 24
    columns = len(TYPE_COLORS) + 1
    width = columns * (cell + padding) + padding
    height = cell + padding * 2 + 28
    sheet = Image.new("RGBA", (width, height), (247, 243, 250, 255))
    ctx = ImageDraw.Draw(sheet)
    font = ImageFont.truetype(str(BOLD_FONT), 16)

    entries: list[tuple[str, tuple[str, tuple[int, int, int, int]] | None]] = [
        ("app", None),
        *[(f".{name}", (name.upper(), color)) for name, color in TYPE_COLORS.items()],
    ]
    for index, (label, badge) in enumerate(entries):
        icon = draw(128, badge)
        x = padding + index * (cell + padding)
        sheet.alpha_composite(icon, (x, padding))
        text_width = ctx.textlength(label, font=font)
        ctx.text(
            (x + (128 - text_width) / 2, padding + 128 + 12),
            label,
            font=font,
            fill=(28, 27, 31, 255),
        )
    sheet.convert("RGB").save(PREVIEW, format="PNG")


def main() -> int:
    ICON_DIR.mkdir(parents=True, exist_ok=True)

    save_ico(APP_ICON, None)
    print(f"{APP_ICON.relative_to(ROOT)}: {APP_ICON.stat().st_size / 1024:.1f} KiB")

    for name, color in TYPE_COLORS.items():
        path = ICON_DIR / f"{name}.ico"
        save_ico(path, (name.upper(), color))
        print(f"{path.relative_to(ROOT)}: {path.stat().st_size / 1024:.1f} KiB")

    save_preview()
    print(f"{PREVIEW.relative_to(ROOT)}: {PREVIEW.stat().st_size / 1024:.1f} KiB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
