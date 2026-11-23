"""Sanity check: every type icon carries its own badge colour."""

import pathlib

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "assets" / "icons"

EXPECTED = {
    "txt": (11, 87, 208),
    "cfg": (179, 38, 30),
    "json": (230, 108, 10),
    "md": (123, 31, 162),
    "ini": (0, 121, 107),
    "log": (95, 99, 104),
    "csv": (24, 128, 56),
}


def main() -> int:
    failures = 0
    for name, rgb in EXPECTED.items():
        image = Image.open(ICON_DIR / f"{name}.ico")
        image.size = (256, 256)
        image = image.convert("RGB")
        # Dominant colour inside the badge, ignoring the white lettering.
        # Badge spans roughly x 0.40..0.99, y 0.71..0.98 of the canvas.
        box = image.crop((int(256 * 0.42), int(256 * 0.73), int(256 * 0.98), int(256 * 0.97)))
        counts: dict[tuple[int, int, int], int] = {}
        for pixel in list(box.getdata()):
            if pixel == (255, 255, 255):
                continue
            counts[pixel] = counts.get(pixel, 0) + 1
        colour, _ = max(counts.items(), key=lambda item: item[1])
        ok = all(abs(colour[i] - rgb[i]) < 32 for i in range(3))
        failures += 0 if ok else 1
        print(f"{name:5s} badge={colour} expected={rgb} {'OK' if ok else 'MISMATCH'}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
