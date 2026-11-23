"""Dump the frames of an .ico at their native sizes, for visual inspection."""

import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "icon-frames"


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    names = sys.argv[1:] or ["txt", "json", "cfg"]
    for name in names:
        path = ROOT / "assets" / "icons" / f"{name}.ico"
        image = Image.open(path)
        sizes = sorted(image.ico.sizes())
        print(name, sizes)
        for size in sizes:
            if size[0] not in (16, 24, 32, 48, 64, 128, 256):
                continue
            image.size = size
            frame = image.convert("RGBA")
            frame.save(OUT / f"{name}-{size[0]}.png")
    print(f"raw frames -> {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
