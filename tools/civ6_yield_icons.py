#!/usr/bin/env python3
"""Cut the spectator's yield-sign atlas out of Civilization VI's own map overlay.

The strategic map's yield signs used to be hand-drawn pictographs -- a wheat
ellipse, a cogwheel, a coin ring -- laid on a flat coloured disc.  The base
game draws something more specific: one finished circular icon per yield, the
colour and the object in a single piece of art.  Those icons live in the map
overlay texture the game itself uses for exactly this job, so they can be cut
rather than imitated:

    <install>/Base/Platforms/Windows/BLPs/SHARED_DATA/TEXTURE_YieldOverlayAtlas

where ``<install>`` is whatever ``civ6_env.assets_dir()`` resolves to -- this
tool does not go looking for the game itself, because exactly one module in
``tools/`` is allowed to.

That file is a Firaxis CIVBIG container, not a DDS: the magic is ``CIVBIG\\0\\0``,
the mip count sits at 0x22, width at 0x26 and height at 0x28 (u16 LE), and the
DXT5/BC3 payload starts at 0x30 with the top mip first.  This Mac's python3 has
neither Pillow nor numpy, so the BC3 decode and the PNG encode are both done
here against the standard library alone.

The atlas is a 1024x1024 sheet of 128 px cells: six yield columns -- food,
production, gold, science, culture, faith -- by a row per count, 1 to 5, then
numeral badges for 6 to 11.  The count-5 row is the same artwork as the
count-1 row at more than twice the resolution, so it is the row worth cutting.

Each sign is a coloured disc.  The dark ring a player sees around it is not
part of the icon -- it is the shaded plate underneath showing through, which
the spectator draws itself (``drawYieldPlate``).  So the cut is the coloured
disc alone, and the plate's 13% pad puts the ring back.

Usage:
    python3 tools/civ6_yield_icons.py web/assets/civ6-yield-icons.png
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env as env  # noqa: E402

#: The map overlay the game draws its own tile yields with, under the assets
#: root. ``TEXTURE_<name>_FOW`` beside it is the fogged variant.
YIELD_OVERLAY = "Base/Platforms/Windows/BLPs/SHARED_DATA/TEXTURE_YieldOverlayAtlas"

# The atlas's own grid, and the row whose artwork is largest.
ATLAS_CELL = 128
LARGEST_COUNT_ROW = 4
# Column order in the base game's sheet, which becomes this atlas's cell order.
YIELDS = ["food", "production", "gold", "science", "culture", "faith"]

# The spectator's own cell. Bigger than the disc it holds so a sign never
# touches its neighbour's cell when a browser samples across the boundary.
CELL = 80


def decode_civbig(path):
    """Return (payload, width, height) for a Firaxis CIVBIG texture."""
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:6] != b"CIVBIG":
        raise SystemExit(f"{path} is not a CIVBIG texture")
    width, height = struct.unpack_from("<HH", data, 0x26)
    return data, width, height


def decode_bc3(data, width, height, offset):
    """Decode a DXT5/BC3 payload to straight RGBA bytes."""
    pixels = bytearray(width * height * 4)
    at = offset
    for block_y in range((height + 3) // 4):
        for block_x in range((width + 3) // 4):
            alpha0, alpha1 = data[at], data[at + 1]
            alpha_bits = int.from_bytes(data[at + 2:at + 8], "little")
            colour0, colour1 = struct.unpack_from("<HH", data, at + 8)
            colour_bits = struct.unpack_from("<I", data, at + 12)[0]
            at += 16
            alphas = [alpha0, alpha1]
            if alpha0 > alpha1:
                alphas += [((7 - i) * alpha0 + i * alpha1) // 7 for i in range(1, 7)]
            else:
                alphas += [((5 - i) * alpha0 + i * alpha1) // 5 for i in range(1, 5)]
                alphas += [0, 255]

            def rgb565(value):
                return (((value >> 11) & 31) * 255 // 31,
                        ((value >> 5) & 63) * 255 // 63,
                        (value & 31) * 255 // 31)

            first, second = rgb565(colour0), rgb565(colour1)
            colours = [first, second]
            if colour0 > colour1:
                colours.append(tuple((2 * a + b) // 3 for a, b in zip(first, second)))
                colours.append(tuple((a + 2 * b) // 3 for a, b in zip(first, second)))
            else:
                colours.append(tuple((a + b) // 2 for a, b in zip(first, second)))
                colours.append((0, 0, 0))
            for row in range(4):
                for column in range(4):
                    x, y = block_x * 4 + column, block_y * 4 + row
                    if x >= width or y >= height:
                        continue
                    texel = row * 4 + column
                    red, green, blue = colours[(colour_bits >> (2 * texel)) & 3]
                    at_pixel = (y * width + x) * 4
                    pixels[at_pixel] = red
                    pixels[at_pixel + 1] = green
                    pixels[at_pixel + 2] = blue
                    pixels[at_pixel + 3] = alphas[(alpha_bits >> (3 * texel)) & 7]
    return pixels


def write_png(path, width, height, pixels):
    raw = b"".join(b"\x00" + bytes(pixels[y * width * 4:(y + 1) * width * 4])
                   for y in range(height))

    def chunk(kind, body):
        payload = kind + body
        return (struct.pack(">I", len(body)) + payload
                + struct.pack(">I", zlib.crc32(payload)))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as handle:
        handle.write(png)


def measure_disc(pixels, width, cell_x, cell_y):
    """Centre and radius of one sign's coloured disc, in cell coordinates.

    The plate under the sign is a near-black rgb(8,12,16), so the disc is
    every pixel that is not that.  Measuring the widest row and the tallest
    column finds the circle without trusting either the cell's nominal centre
    or a bounding box that a flask's glow can inflate.
    """
    def bright(x, y):
        at = ((cell_y + y) * width + cell_x + x) * 4
        return (pixels[at + 3] > 200
                and max(pixels[at], pixels[at + 1], pixels[at + 2]) > 60)

    def widest(fixed_is_row):
        best = (0, 0, 0)
        for fixed in range(ATLAS_CELL):
            run_start = None
            for moving in range(ATLAS_CELL):
                x, y = ((moving, fixed) if fixed_is_row else (fixed, moving))
                if bright(x, y):
                    if run_start is None:
                        run_start = moving
                    run_end = moving
                elif run_start is not None and moving - run_start > best[0]:
                    best = (run_end - run_start + 1, run_start, run_end)
                    run_start = None
                elif run_start is not None:
                    run_start = None
            if run_start is not None and run_end - run_start + 1 > best[0]:
                best = (run_end - run_start + 1, run_start, run_end)
        return best

    span_x, start_x, end_x = widest(True)
    span_y, start_y, end_y = widest(False)
    return ((start_x + end_x) / 2, (start_y + end_y) / 2, max(span_x, span_y) / 2)


def main(destination):
    data, width, height = decode_civbig(env.assets_dir() / YIELD_OVERLAY)
    pixels = decode_bc3(data, width, height, 0x30)

    discs = []
    for column, kind in enumerate(YIELDS):
        cell_x, cell_y = column * ATLAS_CELL, LARGEST_COUNT_ROW * ATLAS_CELL
        centre_x, centre_y, radius = measure_disc(pixels, width, cell_x, cell_y)
        discs.append((kind, cell_x, cell_y, centre_x, centre_y, radius))
        print(f"{kind:11s} disc centre ({centre_x:.1f}, {centre_y:.1f}) "
              f"radius {radius:.1f}")

    # One radius for all six: the base game authors them at one size, and a
    # per-icon radius would reintroduce exactly the uneven-icon problem the
    # unit atlas already had to measure its way out of.
    radius = max(disc[5] for disc in discs)
    if 2 * radius > CELL:
        raise SystemExit(f"a {2 * radius:.0f}px disc does not fit a {CELL}px cell")
    print(f"cutting every sign at radius {radius:.1f} into {CELL}px cells")

    sheet_width = CELL * len(YIELDS)
    sheet = bytearray(sheet_width * CELL * 4)
    for index, (kind, cell_x, cell_y, centre_x, centre_y, _) in enumerate(discs):
        for y in range(CELL):
            for x in range(CELL):
                dx, dy = x - (CELL - 1) / 2, y - (CELL - 1) / 2
                distance = (dx * dx + dy * dy) ** .5
                # One pixel of feather, so the cut edge is as smooth as the
                # art's own and no ring of hard pixels appears at small sizes.
                coverage = min(1.0, max(0.0, radius + .5 - distance))
                if coverage <= 0:
                    continue
                source_x = round(cell_x + centre_x + dx)
                source_y = round(cell_y + centre_y + dy)
                at = (source_y * width + source_x) * 4
                out = ((y * sheet_width) + index * CELL + x) * 4
                sheet[out] = pixels[at]
                sheet[out + 1] = pixels[at + 1]
                sheet[out + 2] = pixels[at + 2]
                sheet[out + 3] = round(pixels[at + 3] * coverage)

    write_png(destination, sheet_width, CELL, sheet)
    print(f"wrote {destination} ({sheet_width}x{CELL})")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "web/assets/civ6-yield-icons.png")
