"""Renders the PrismDMX mark into the icon files the Tauri bundler asks for.

`ui/public/favicon.svg` is the mark, and it is the *outline* of it that is
reproduced here: the SVG's own body is a filled path with a dozen blurred
ellipses masked into it, and a 32-pixel icon has nowhere to put a blur. The
outline is transcribed once, below, as the points the path's `M`/`V`/`H`/`l`
commands land on — the four rounded shoulders are beziers spanning less than
half a pixel at icon sizes and are taken as corners.

Run it from the repository root:

    python tools/make-icons/make-icons.py

It writes `crates/prism-app/icons/`, which **is** committed: the bundler needs
the files at build time and a release that had to run Python first would be a
release with a second toolchain in it.
"""

from pathlib import Path

from PIL import Image, ImageDraw

# The viewBox of ui/public/favicon.svg, and the points its path visits.
VIEWBOX = (48.0, 46.0)
OUTLINE = [
    (25.946, 44.938),
    (23.925, 44.240),
    (23.925, 33.937),
    (21.663, 31.675),
    (10.287, 31.675),
    (9.367, 29.887),
    (16.847, 19.416),
    (15.005, 15.838),
    (1.237, 15.838),
    (0.317, 14.050),
    (10.013, 0.474),
    (10.933, 0.000),
    (39.827, 0.000),
    (40.747, 1.788),
    (33.267, 12.259),
    (35.109, 15.838),
    (46.486, 15.838),
    (47.376, 17.668),
]

# The mark's own purple, from the same file.
PURPLE = (134, 59, 255, 255)
# How many times over the shape is drawn before it is scaled down. Eight is
# enough that a 32-pixel edge is smooth and cheap enough that the whole set
# renders in under a second.
SUPERSAMPLE = 8


def mark(size: int) -> Image.Image:
    """The mark, `size` by `size`, on transparency."""
    big = size * SUPERSAMPLE
    # Fitted to the square with a margin, keeping the aspect ratio: the mark is
    # taller than it is wide, so the width is what the margin is measured on.
    scale = big * 0.86 / VIEWBOX[1]
    offset_x = (big - VIEWBOX[0] * scale) / 2
    offset_y = (big - VIEWBOX[1] * scale) / 2
    image = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(image).polygon(
        [(x * scale + offset_x, y * scale + offset_y) for x, y in OUTLINE],
        fill=PURPLE,
    )
    return image.resize((size, size), Image.LANCZOS)


def main() -> None:
    out = Path("crates/prism-app/icons")
    out.mkdir(parents=True, exist_ok=True)
    for name, size in [
        ("32x32.png", 32),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("icon.png", 512),
    ]:
        mark(size).save(out / name)
    # One .ico carrying every size Windows asks for: the taskbar wants 32, the
    # installer's header wants 48, and the Alt-Tab card wants 256.
    mark(256).save(
        out / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
