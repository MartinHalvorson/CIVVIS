#!/usr/bin/env python3
r"""Cut the spectator's unit-flag plates out of Civilization VI's own in-world UI.

A CIVVIS unit used to stand on a marker the viewer invented: a circle for
anything that fights and a rounded triangle, point *down*, for anything that
does not.  The base game draws eight authored silhouettes instead -- and its
civilian triangle points *up* -- so the markers read as a different game's
counters no matter how carefully the glyph inside them was cut.  Those eight
shapes can be taken rather than imitated:

    <install>/Base/Platforms/Windows/BLPs/UI/InWorld.blp

where ``<install>`` is whatever ``civ6_env.assets_dir()`` resolves to -- this
tool does not go looking for the game itself, because exactly one module in
``tools/`` is allowed to.

Usage::

    python3 tools/civ6_unit_flag_plates.py web/assets/civ6-unit-flag-plates.png
    python3 tools/civ6_unit_flag_plates.py --list        # the whole catalogue

---------------------------------------------------------------- the container

``InWorld.blp`` is **not** the ``CIVBIG`` texture container ``civ6_yield_icons``
reads, and it holds no loose ``TEXTURE_*`` files.  It is ``CIVBLP\0\2\0`` -- a
serialized C++ object graph, and the graph carries its own reflection data, so
nothing below is guesswork:

===========  ====================================================
file header
===========  ====================================================
``0x00``     magic ``CIVBLP`` + u16 version (2)
``0x08``     u32 offset of the package section (``0x400``)
``0x0C``     u32 **size** of the package section (``0x13200``)
``0x10``     u32 offset of the big-data section (``0x13600``)
``0x14``     u32 entry count (122)
``0x18``     u32 file size
===========  ====================================================

⚠ ``0x0C`` is a size, not an offset.  Reading it as "the allocation table" is
what stalled the first attempt at this file: ``0x08 + 0x0C == 0x10`` exactly,
and the 0x400 bytes before the big data are the table's zero tail.

The package section is laid out ``[type-info stripe][stripe 0][stripe 1][the
allocation table]``.  The **allocation table** is an array of 40-byte
``Serialization::PackageAllocation`` records, one per allocation, indexed from
**1** (index 0 is the null pointer):

======  ====  ===============================================================
offset  type  field
======  ====  ===============================================================
``0``   u8    ``byStripe`` -- 0 or 1, which stripe holds the bytes
``1``   u8    ``byAllocType``
``6``   u16   ``wParentAlloc``
``8``   u32   ``dwOffset`` -- from that stripe's base, 16-byte aligned
``12``  u32   ``dwAllocSize``
``16``  u32   ``dwElementCount``
``24``  u64   ``qwUserData``
``32``  ptr64 ``sTypeName`` -- **an allocation index**, not an address
======  ====  ===============================================================

Every ``ptr64`` in the graph is stored as ``(u32 allocation index, u32 byte
offset into it)``, which is what makes the file relocatable and what makes the
table readable without knowing where anything lives.  Allocations tile their
stripe in index order at 16-byte alignment, and the stripes are laid end to end
finishing exactly at the table, so both stripe bases fall out of the table
itself::

    stripe1_base = table_start - max(off + size for stripe 1)
    stripe0_base = stripe1_base - max(off + size for stripe 0)

The table is found by seeking the one record whose ``dwAllocSize`` is
``16 * entry_count`` beside a ``dwElementCount`` of ``entry_count`` -- the
package's ``BLP::Package::EntryMap`` array -- and walking outward by 40 bytes.

------------------------------------------------------------------ the entries

Resolving ``sTypeName`` names every allocation.  This file holds one
``ForgeUI::TexturePackageEntry`` (a sprite on the shared 256x256 RGBA page) and
121 ``ForgeUI::BCTexturePackageEntry`` (a sprite that is its own page).  Both
begin with a ``BLP::PackageAssetEntry`` header and share these offsets:

======  ==================================================================
``56``  ptr64 ``m_Name`` -- allocation index of a ``[u32 cap][u32 len][chars]``
``64``  u32   asset name hash; the key in ``EntryMap`` and in the buffer table
``72``  u32   ``m_uiFlags``
``76``  u32   ``m_nPageIndex``
``80``  u16   ``m_nXOffset``      ``82`` u16 ``m_nYOffset``
``84``  u16   ``m_nTextureWidth`` ``86`` u16 ``m_nTextureHeight``
======  ==================================================================

and the block-compressed subclass adds

======  ==================================================================
``100`` u32   block count
``104`` u16   block edge, in pixels (1, 2, 4 or 8)
``106`` u16   ``m_nBytesPerIndex`` (1 or 2)
======  ==================================================================

A sprite's pixels live in the big-data section, addressed by a
``BLP::TBufferEntry`` (offset at ``+32``, size at ``+40``, name hash at ``+48``)
found by matching that hash.  The blob is a **deduplicated block dictionary**::

    [ block count * 4 bytes ]  dictionary: RGBA texels, edge*edge per block
    [ index array           ]  one index per edge*edge block, row major,
                               `bytes per index` wide, padded to 4 bytes

which reproduces every one of the 121 blob sizes exactly -- that identity is
the parse's oracle and ``_check`` asserts it for all of them.  The 256x256 page
that carries the one non-BC sprite is a plain ``BLP::TextureEntry``: DXGI
format 28 (``R8G8B8A8``), 9 mips, 349,524 bytes at big-data offset 0, and the
mip sum reproducing that byte count is the second oracle.

⚠ That page is what an earlier attempt decoded at width 256 and read as "the
military flag".  It is not: it is ``BuilderRecommendation``, 128x128 at (2,2),
the improvement-recommendation banner, which happens to share the flag family's
silhouette.  The real flags are BC sprites and none of them is at ``0x13600``.

--------------------------------------------------------------------- the plates

``UnitFlagManager.lua`` names seven flag styles plus the embarked state, each a
128x128 ``UnitFlag<Style>_Combo`` texture holding a 2x2 grid of 64px cells:

===========  =========================================================
cell         content
===========  =========================================================
top-left     the flag: shaded body **and** its rim -- what this cuts
top-right    the same flag with the promotion tab behind it
bottom row   the body alone, unrimmed, as a soft highlight layer
===========  =========================================================

All eight styles are authored in one 64px cell at one scale, horizontally
centred, and deliberately seated at *different* heights -- the military pin
hangs its point below centre, the civilian triangle stands its apex above it.
So the cut is one shared square window measured across the whole set rather
than a per-style crop: normalising each shape into its own box would throw away
the base game's relative sizes, which is the same mistake the unit-glyph atlas
had to measure its way out of.  The window is printed on every run.
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env as env  # noqa: E402

#: The in-world UI package, under the assets root.
IN_WORLD = "Base/Platforms/Windows/BLPs/UI/InWorld.blp"

#: Bytes of one ``Serialization::PackageAllocation``.
ALLOCATION = 40

#: Field offsets inside a ``ForgeUI::TexturePackageEntry`` and its BC subclass.
ENTRY_NAME, ENTRY_HASH = 0x38, 0x40
ENTRY_PAGE, ENTRY_X, ENTRY_WIDTH = 0x4C, 0x50, 0x54
ENTRY_BLOCKS, ENTRY_EDGE, ENTRY_INDEX_BYTES = 0x64, 0x68, 0x6A

#: Field offsets inside a ``BLP::TBufferEntry`` / ``BLP::TextureEntry``.
BUFFER_OFFSET, BUFFER_SIZE, BUFFER_HASH = 0x20, 0x28, 0x30
BUFFER_ENTRY = 72
TEXTURE_FORMAT, TEXTURE_WIDTH, TEXTURE_MIPS = 0x58, 0x5A, 0x62
#: ``DXGI_FORMAT_R8G8B8A8_UNORM`` -- the byte order every sprite here uses.
TEXTURE_RGBA = 28

#: The styles ``UnitFlagManager.lua`` names, in the order they are written into
#: the sheet. ``base`` is the military flag every combat land unit stands on.
STYLES = ["base", "civilian", "naval", "support",
          "trade", "religion", "fortify", "embark"]

#: The cell of a ``_Combo`` sheet holding the rimmed flag, and its size.
COMBO_CELL = 64
COMBO_COLUMN = COMBO_ROW = 0

#: Ignore the anti-aliased fringe when measuring, then keep it when cutting.
ALPHA_FLOOR = 8
#: Room left around the widest style so its soft edge is never clipped.
WINDOW_PAD = 2


# --------------------------------------------------------------- the container


class Package:
    """A parsed ``CIVBLP`` package: allocations, named assets, and big data."""

    def __init__(self, data):
        if data[:6] != b"CIVBLP":
            raise SystemExit("not a CIVBLP package")
        self.data = data
        section, size, self.big, self.entries = struct.unpack_from("<4I", data, 8)
        if section + size != self.big:
            raise SystemExit(
                f"package section {section:#x}+{size:#x} does not meet big data "
                f"{self.big:#x}; the header's second word is a size, not an offset")
        self.table = self._find_table(section, self.big)
        self.allocations = self._read_allocations()
        self._base = self._stripe_bases()

    # The table is located from the one allocation whose size and element count
    # are the package's own entry count -- the `EntryMap` array -- and then
    # grown outward. Nothing here depends on a hard-coded address.
    def _find_table(self, section, end):
        needle = struct.pack("<2I", 16 * self.entries, self.entries)
        seed = self.data.find(needle, section, end)
        if seed < 0:
            raise SystemExit("no allocation record for the package entry map")
        start = seed - 12
        while start - ALLOCATION >= section and self._plausible(start - ALLOCATION):
            start -= ALLOCATION
        return start

    def _plausible(self, at):
        record = self.data[at:at + ALLOCATION]
        if len(record) < ALLOCATION or record[0] > 1:
            return False
        offset, size, count = struct.unpack_from("<3I", record, 8)
        return offset % 16 == 0 and (size or count) and size < 1 << 24

    def _read_allocations(self):
        out = [None]                      # index 0 is the null pointer
        at = self.table
        while self._plausible(at):
            record = self.data[at:at + ALLOCATION]
            out.append((record[0], *struct.unpack_from("<3I", record, 8),
                        struct.unpack_from("<I", record, 32)[0]))
            at += ALLOCATION
        return out

    def _stripe_bases(self):
        span = {}
        for stripe, offset, size, _count, _type in self.allocations[1:]:
            span[stripe] = max(span.get(stripe, 0), offset + size)
        base = {1: self.table - span.get(1, 0)}
        base[0] = base[1] - span.get(0, 0)
        return base

    def at(self, index):
        """(file address, byte length) of one allocation."""
        stripe, offset, size, _count, _type = self.allocations[index]
        return self._base[stripe] + offset, size

    def type_name(self, index):
        """The ``String::Global`` an allocation's ``sTypeName`` points at."""
        at, size = self.at(self.allocations[index][4])
        return self.data[at:at + size - 1].decode("latin1")

    def text(self, index):
        """A ``[u32 capacity][u32 length][chars]`` string allocation."""
        at, _size = self.at(index)
        length = struct.unpack_from("<I", self.data, at + 4)[0]
        return self.data[at + 8:at + 8 + length].decode("latin1")

    def buffers(self):
        """{asset name hash: (big-data offset, byte length)}."""
        for index in range(1, len(self.allocations)):
            if self.type_name(index) != "BLP::TBufferEntry":
                continue
            at, _size = self.at(index)
            out = {}
            for k in range(self.allocations[index][3]):
                entry = at + BUFFER_ENTRY * k
                out[struct.unpack_from("<I", self.data, entry + BUFFER_HASH)[0]] = (
                    struct.unpack_from("<I", self.data, entry + BUFFER_OFFSET)[0],
                    struct.unpack_from("<I", self.data, entry + BUFFER_SIZE)[0])
            return out
        raise SystemExit("no texture-buffer table in the package")

    def assets(self):
        """{sprite name: description} for every named entry in the package."""
        buffers = self.buffers()
        out = {}
        for index in range(1, len(self.allocations)):
            kind = self.type_name(index)
            if not kind.startswith("ForgeUI::") or "PackageEntry" not in kind:
                continue
            at, _size = self.at(index)
            u32 = lambda o: struct.unpack_from("<I", self.data, at + o)[0]  # noqa: E731
            u16 = lambda o: struct.unpack_from("<H", self.data, at + o)[0]  # noqa: E731
            name = self.text(u32(ENTRY_NAME))
            sprite = dict(name=name, page=u32(ENTRY_PAGE),
                          x=u16(ENTRY_X), y=u16(ENTRY_X + 2),
                          width=u16(ENTRY_WIDTH), height=u16(ENTRY_WIDTH + 2),
                          packed=kind.startswith("ForgeUI::BC"))
            if sprite["packed"]:
                sprite.update(blocks=u32(ENTRY_BLOCKS), edge=u16(ENTRY_EDGE),
                              index_bytes=u16(ENTRY_INDEX_BYTES),
                              blob=buffers[u32(ENTRY_HASH)])
            out[name] = sprite
        return out

    def page(self):
        """(format, width, height, mips, offset, size) of the shared RGBA page."""
        for index in range(1, len(self.allocations)):
            if self.type_name(index) != "BLP::TextureEntry":
                continue
            at, _size = self.at(index)
            fmt = struct.unpack_from("<H", self.data, at + TEXTURE_FORMAT)[0]
            width, height = struct.unpack_from("<2H", self.data, at + TEXTURE_WIDTH)
            mips = struct.unpack_from("<H", self.data, at + TEXTURE_MIPS)[0]
            offset = struct.unpack_from("<I", self.data, at + BUFFER_OFFSET)[0]
            size = struct.unpack_from("<I", self.data, at + BUFFER_SIZE)[0]
            return fmt, width, height, mips, offset, size
        raise SystemExit("no page texture in the package")


def decode_sprite(package, sprite):
    """Straight RGBA bytes for one sprite, block dictionary expanded."""
    data = package.data
    width, height = sprite["width"], sprite["height"]
    pixels = bytearray(width * height * 4)
    if not sprite["packed"]:
        # The shared page is raw RGBA with its top mip first; the sprite is a
        # window into it.
        _fmt, page_width, _h, _m, offset, _s = package.page()
        top = package.big + offset
        for y in range(height):
            row = top + ((sprite["y"] + y) * page_width + sprite["x"]) * 4
            pixels[y * width * 4:(y + 1) * width * 4] = data[row:row + width * 4]
        return width, height, pixels
    offset, _size = sprite["blob"]
    dictionary = package.big + offset
    indices = dictionary + sprite["blocks"] * 4
    edge, stride = sprite["edge"], sprite["index_bytes"]
    across = (width + edge - 1) // edge
    for block_y in range((height + edge - 1) // edge):
        for block_x in range(across):
            at = indices + (block_y * across + block_x) * stride
            block = (data[at] if stride == 1
                     else struct.unpack_from("<H", data, at)[0])
            source = dictionary + block * edge * edge * 4
            for row in range(edge):
                y = block_y * edge + row
                if y >= height:
                    break
                for column in range(edge):
                    x = block_x * edge + column
                    if x >= width:
                        continue
                    texel = source + (row * edge + column) * 4
                    out = (y * width + x) * 4
                    pixels[out:out + 4] = data[texel:texel + 4]
    return width, height, pixels


def _check(package, sprites):
    """Prove the parse by predicting bytes it did not use to build itself."""
    fmt, width, height, mips, offset, size = package.page()
    expected = sum(4 * max(1, width >> level) * max(1, height >> level)
                   for level in range(mips))
    if fmt != TEXTURE_RGBA or offset or size != expected:
        raise SystemExit(f"page format {fmt} {width}x{height} mips={mips} at "
                         f"{offset} is {size} bytes, not the {expected} an RGBA "
                         f"mip chain needs")
    for sprite in sprites.values():
        if not sprite["packed"]:
            continue
        edge, stride = sprite["edge"], sprite["index_bytes"]
        across = (sprite["width"] + edge - 1) // edge
        down = (sprite["height"] + edge - 1) // edge
        budget = sprite["blocks"] * 4 + -(-across * down * stride // 4) * 4
        if budget != sprite["blob"][1]:
            raise SystemExit(f"{sprite['name']} needs {budget} bytes and its "
                             f"buffer holds {sprite['blob'][1]}")


# ---------------------------------------------------------------- the PNG file


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


def flag_cell(package, sprites, style):
    """The rimmed 64px flag cell of one ``UnitFlag<Style>_Combo`` sheet."""
    name = f"UnitFlag{style.capitalize()}_Combo"
    width, height, pixels = decode_sprite(package, sprites[name])
    if (width, height) != (COMBO_CELL * 2, COMBO_CELL * 2):
        raise SystemExit(f"{name} is {width}x{height}, not a 2x2 sheet of "
                         f"{COMBO_CELL}px cells")
    cell = bytearray(COMBO_CELL * COMBO_CELL * 4)
    for y in range(COMBO_CELL):
        source = ((COMBO_ROW * COMBO_CELL + y) * width
                  + COMBO_COLUMN * COMBO_CELL) * 4
        cell[y * COMBO_CELL * 4:(y + 1) * COMBO_CELL * 4] = \
            pixels[source:source + COMBO_CELL * 4]
    return cell


def measure(cell):
    """Half-extent of a cell's artwork from the cell's own centre, in pixels."""
    centre = (COMBO_CELL - 1) / 2
    reach_x = reach_y = 0.0
    for y in range(COMBO_CELL):
        for x in range(COMBO_CELL):
            if cell[(y * COMBO_CELL + x) * 4 + 3] < ALPHA_FLOOR:
                continue
            reach_x = max(reach_x, abs(x - centre) + .5)
            reach_y = max(reach_y, abs(y - centre) + .5)
    return reach_x, reach_y


def main(destination, listing=False):
    package = Package((env.assets_dir() / IN_WORLD).read_bytes())
    sprites = package.assets()
    _check(package, sprites)
    if listing:
        print(f"{len(sprites)} sprites, page {package.page()}")
        for sprite in sprites.values():
            print(f"  {sprite['name']:30s} {sprite['width']:4d}x"
                  f"{sprite['height']:<4d} page {sprite['page']:3d} "
                  f"{'blocks %5d edge %d idx %d' % (sprite['blocks'], sprite['edge'], sprite['index_bytes']) if sprite['packed'] else 'shared page'}")
        return

    # Discovered, not trusted: a patch that adds a ninth flag style fails here
    # rather than shipping a sheet that quietly omits it.
    shipped = sorted(name[len("UnitFlag"):-len("_Combo")].lower()
                     for name in sprites
                     if name.startswith("UnitFlag") and name.endswith("_Combo"))
    if shipped != sorted(STYLES):
        raise SystemExit(f"the package ships flag styles {shipped}, "
                         f"this tool cuts {sorted(STYLES)}")

    cells = [flag_cell(package, sprites, style) for style in STYLES]
    reach = max(max(measure(cell)) for cell in cells)
    half = int(reach) + WINDOW_PAD
    size = half * 2
    if size > COMBO_CELL:
        raise SystemExit(f"a {size}px window does not fit a {COMBO_CELL}px cell")
    for style, cell in zip(STYLES, cells):
        reach_x, reach_y = measure(cell)
        print(f"{style:9s} reaches {reach_x:4.1f} x {reach_y:4.1f} px from centre")
    print(f"cutting every style on one {size}x{size} window "
          f"(widest reach {reach:.1f} + {WINDOW_PAD}px)")

    sheet_width = size * len(STYLES)
    sheet = bytearray(sheet_width * size * 4)
    origin = (COMBO_CELL - size) // 2
    for index, cell in enumerate(cells):
        for y in range(size):
            source = ((origin + y) * COMBO_CELL + origin) * 4
            out = (y * sheet_width + index * size) * 4
            sheet[out:out + size * 4] = cell[source:source + size * 4]
    write_png(destination, sheet_width, size, sheet)
    print(f"wrote {destination} ({sheet_width}x{size}, "
          f"{len(STYLES)} styles: {', '.join(STYLES)})")


if __name__ == "__main__":
    arguments = [word for word in sys.argv[1:] if not word.startswith("-")]
    main(arguments[0] if arguments else "web/assets/civ6-unit-flag-plates.png",
         listing="--list" in sys.argv[1:])
