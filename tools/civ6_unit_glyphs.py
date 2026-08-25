#!/usr/bin/env python3
r"""Cut the spectator's unit glyphs out of Civilization VI's own icon atlases.

The command map draws a white symbol inside every unit counter.  Until now that
symbol came off the **Civilization Wiki**: `tools/civ6_unit_flags.swift`
downloaded 89 archived Civilopedia cards and recovered the glyphs by taking a
low per-pixel percentile across the set to subtract the shared card background.
It worked, and it was the last piece of CIVVIS art not read off the install --
a third-party archive standing in for a file on this disk.

This reads the file on this disk instead::

    python3 tools/civ6_unit_glyphs.py            # web/assets/civ6-unit-flags.{png,json}
    python3 tools/civ6_unit_glyphs.py --list     # the resolved icon table
    python3 tools/civ6_unit_glyphs.py --verify   # re-check the committed art, no install

`civ6_unit_flag_plates` already parses the `CIVBLP` package format and its
docstring is the long version of the container; this tool imports that parser
rather than carrying a second one.

------------------------------------------------------- which sheet, and why not

⚠ `docs/CIV6_ART.md` named `UnitFlagAtlasWhite` in `InWorld.blp` as "the game's
own in-world glyph sheet" and left re-cutting from it as this task.  It is not
the sheet, and cutting it would have been a step backwards.  Three findings, in
the order they settle it:

1. `UnitFlagManager.lua:SetFlagUnitEmblem` asks for ``"ICON_" ..
   GameInfo.Units[type].UnitType`` -- ``ICON_UNIT_WARRIOR``, with no suffix.
   That name is defined only in ``ICON_ATLAS_UNITS`` and its DLC siblings.  No
   shipped Lua anywhere in the install ever asks for an ``ICON_UNIT_*_WHITE``,
   the only names bound to ``ICON_ATLAS_UNIT_FLAG_SYMBOLS_WHITE``.
2. Nothing in the install says what `UnitFlagAtlasWhite`'s cells *are*.  Its 93
   glyphs do not correspond to the 102 of `Units256`/`Units32` -- matching the
   two sheets cell for cell scores a median silhouette IoU of 0.39, no better
   than pairing each cell with an unrelated one (0.40), and only 4 of 93 cells
   have a strong twin.  It is an orphan sheet, and naming its cells would have
   meant a hand-written guess -- exactly the roster beside the renderer this
   task exists to remove.
3. It could not cover the roster anyway.  `Icons_UnitFlags.xml` declares no
   flag symbol for the Guru, the Warrior Monk, Modern Armor, Modern AT or the
   Missile Cruiser -- their declared indices land on empty cells -- and none at
   all for any expansion or DLC unique: no Toa, no Tagma, no Nihang.

So the glyphs come from ``ICON_ATLAS_UNITS`` and the eight DLC unit atlases
beside it, at ``IconSize="256"``, and the game's own icon tables say which cell
belongs to which unit.

----------------------------------------------------------------- the name chain

Every step is read, none is written down here:

    ruleset unit  ->  Civilization VI unit type  ->  icon name  ->  atlas + index

- the **roster** is `data/units.json`, the ruleset `Rules::embedded()` ships;
- the **type** is `civ6_unit_type` in `src/bin/civvis_orders.rs`, the fleet's one
  CIVVIS-to-Civilization VI unit translation, parsed out of that function rather
  than copied beside it, because the live order channel already has to be right
  about `UNIT_NUBIAN_PITATI` and two tables would drift;
- the **icon** is `"ICON_" + type`, the string `UnitFlagManager.lua` builds;
- the **atlas and index** are the `IconTextureAtlases` and `IconDefinitions`
  rows of every icon XML in the install.

A unit whose own icon the game does not define borrows the icon of the unit it
**replaces**, which is again the ruleset's own answer (`UnitSpec::replaces`, the
`CivilizationUniqueUnits` row Civilization VI ships).  Exactly one unit needs
that: Civilization VI defines `ICON_UNIT_ETHIOPIAN_OROMO_CAVALRY_PORTRAIT` and
no matching symbol icon, so the Oromo Cavalry stands on the Courser's glyph, as
it does in the game.  Anything still unresolved after that stops the run --
a blank cell is not an outcome this tool has.

---------------------------------------------------------------- how it is checked

`civ6_unit_flag_plates` verifies a parse by predicting numbers the parse never
read.  This holds that bar and raises it:

- ``_check`` -- every sprite's ``blocks * 4 + padded index bytes`` against the
  length of the buffer it points at, no index naming a block outside the
  dictionary, and the mip chain of any shared page.  ~500 predictions per
  package;
- ``roundtrip`` -- the whole blob **re-encoded from the decoded pixels** and
  compared byte for byte.  Every atlas this cuts passes it, which means every
  byte of every glyph is confirmed, and flipping one alpha bit fails it;
- the XML's ``IconsPerRow * IconSize`` and ``IconsPerColumn * IconSize`` against
  the decoded texture's own width and height -- a text file the pixels never
  see;
- the **cell census**: every index the icon tables declare for an atlas must
  land on a cell that has ink, and every cell with ink must be declared.  A grid
  origin or stride that is wrong by one pixel of a cell fails this.

``--self-test`` runs the last two against a deliberate one-pixel edit and a
deliberately shifted grid, so the oracles are shown to be capable of failing.
"""

import array
import json
import re
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env as env  # noqa: E402
import civ6_unit_flag_plates as blp  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

#: The committed atlas and its manifest.
ATLAS = ROOT / "web/assets/civ6-unit-flags.png"
MANIFEST = ROOT / "web/assets/civ6-unit-flags.json"

#: The ruleset roster, and the one CIVVIS-to-Civilization VI unit translation.
UNITS = ROOT / "data/units.json"
ORDERS = ROOT / "src/bin/civvis_orders.rs"

#: The atlas the spectator draws: 12 columns of 64px cells, as #2298 measured.
CELL, COLUMNS = 64, 12
#: Cut from the 256px icons and box-averaged 4:1, which keeps Civilization VI's
#: thin weapon lines antialiased at map scale instead of aliasing them away.
SOURCE = 256
#: Ignore the fringe when measuring a silhouette, the way the renderer does.
ALPHA_FLOOR = 12

#: Our own control mod is installed *into* the game's asset tree. Reading its
#: rows back would be reading our own invented names as evidence the game has
#: them, which is the defect `civ6_type_names.py` exists to prevent.
OUR_OWN_MOD = "CivvisControl"

#: XML comments, stripped before any row is read. ⚠ Not cosmetic: Firaxis moved
#: the Winged Hussar's icon into the Poland pack and left the base row behind as
#: ``<!--<Row Name="ICON_UNIT_POLISH_HUSSAR" Atlas="ICON_ATLAS_UNITS"
#: Index="46"/>-->``. Reading disabled rows put the Hussar on cell 46, which
#: draws a banner, and the sheet looked entirely plausible.
_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)

_ATLAS_ROW = re.compile(
    r'<Row\s+Name="(ICON_ATLAS_[A-Z0-9_]+)"\s+IconSize="(\d+)"\s+'
    r'IconsPerRow="(\d+)"\s+IconsPerColumn="(\d+)"\s+Filename="([^"]+)"')
_ICON_ROW = re.compile(
    r'<Row\s+Name="(ICON_UNIT_[A-Z0-9_]+)"\s+(?:Atlas="(ICON_ATLAS_[A-Z0-9_]+)"'
    r'\s+Index="(\d+)"|Index="(\d+)"\s+Atlas="(ICON_ATLAS_[A-Z0-9_]+)")\s*/>')


# ------------------------------------------------------------------ the ruleset


def roster():
    """Every unit the shipped ruleset defines, in one stable order."""
    return sorted(json.loads(UNITS.read_text()))


def replacements():
    """{unit: the unit it replaces}, from the ruleset's own uniques."""
    units = json.loads(UNITS.read_text())
    return {name: spec["replaces"] for name, spec in units.items()
            if spec.get("replaces")}


def civ6_unit_types():
    """The CIVVIS-to-Civilization VI unit translation, read from its one home.

    `src/bin/civvis_orders.rs::civ6_unit_type` is what the live order channel
    emits, and a name it gets wrong is silently discarded by the game. Parsing
    that function keeps one table in the repository instead of two.
    """
    source = ORDERS.read_text()
    body = source.split("fn civ6_unit_type(")[1].split("\n}\n")[0]
    alias = dict(re.findall(r'"([a-z0-9_]+)" => "([A-Z0-9_]+)",', body))
    if len(alias) < 10 or "tagma" not in alias:
        raise SystemExit("civ6_unit_type no longer reads as a match on unit "
                         "names; the glyph cut needs that translation")
    return lambda unit: "UNIT_" + alias.get(unit, unit.upper())


# ------------------------------------------------------- the game's icon tables


def icon_tables():
    """({atlas: {size: (per row, per column, filename, pack)}}, {icon: [(atlas, index, pack)]}).

    ``pack`` is the top directory the row came from -- ``Base`` or ``DLC/<name>``
    -- which is both how a texture is found in the right package and how a base
    definition is preferred to a scenario's replacement of it.
    """
    assets = env.assets_dir()
    atlases, icons = {}, {}
    for path in assets.rglob("*.xml"):
        parts = path.relative_to(assets).parts
        if OUR_OWN_MOD in parts:
            continue
        pack = "/".join(parts[:2]) if parts[0] == "DLC" else parts[0]
        try:
            text = path.read_bytes().decode("latin1")
        except OSError:
            continue
        if "IconTextureAtlases" not in text and "ICON_UNIT_" not in text:
            continue
        text = _COMMENT.sub("", text)
        for name, size, per_row, per_column, filename in _ATLAS_ROW.findall(text):
            atlases.setdefault(name, {})[int(size)] = (
                int(per_row), int(per_column), Path(filename).stem, pack)
        for name, atlas, index, index2, atlas2 in _ICON_ROW.findall(text):
            icons.setdefault(name, []).append(
                (atlas or atlas2, int(index or index2), pack))
    if "ICON_ATLAS_UNITS" not in atlases:
        raise SystemExit("no unit icon atlas in the install's icon tables")
    return atlases, icons


def seat(index, per_row, across):
    """Where the engine's `(index % IconsPerRow, index / IconsPerRow)` lands.

    Restated in the texture's own row width, because the table's `IconsPerRow`
    over-declares a short DLC sheet and the two disagree only where nothing is
    indexed.
    """
    return index % per_row + (index // per_row) * across


def pick(entries):
    """The base game's definition of an icon, or the only one there is.

    Two names are defined twice. The Scout has a `ScoutCat` DLC icon that draws
    a cat, and the Winged Hussar has a Poland pack icon beside the base game's.
    Preferring `Base` is what makes the sheet the base game's own and what makes
    it the same sheet on every install, whatever is enabled.
    """
    base = [row for row in entries if row[2] == "Base"]
    return sorted(base or entries)[0]


# ------------------------------------------------------------- the icon packages


def packages():
    """{texture name: (relative package path, sprite)} for every UI icon package."""
    assets = env.assets_dir()
    index = {}
    for path in sorted(assets.rglob("Icons.blp")):
        relative = path.relative_to(assets)
        pack = "/".join(relative.parts[:2]) if relative.parts[0] == "DLC" \
            else relative.parts[0]
        try:
            package = blp.Package(path.read_bytes())
            sprites = package.assets()
            blp._check(package, sprites)
        except BaseException:            # a package this cut never opens
            continue
        for name, sprite in sprites.items():
            index.setdefault((pack, name), (str(relative), package, sprite))
    return index


def cells(package, sprite, size, per_row, per_column):
    """The decoded atlas, verified against every source that describes it.

    ``IconSize`` has to divide both of the texture's own dimensions -- the
    sheet is a whole number of cells -- and every byte of it has to re-encode
    to the package's own bytes.

    ⚠ Only ``IconSize`` is checked against the texture, because Civilization
    VI's own tables are wrong about the other two. Every DLC unit atlas
    declares ``IconsPerColumn="1"`` and several ship more -- `XP2_Units256` is
    1024x1536, six rows of four, and `Expansion2_Icons_Units.xml` indexes cell
    19 inside it anyway. `Portugal_Icons_Units.xml` declares a 4x4 sheet for a
    pack that ships one unit, and `Portugal_Units256` is a single 256x256 cell.
    Neither error can reach the game, because the engine reads a cell as
    ``(index % IconsPerRow, index / IconsPerRow)`` and every index those packs
    actually declare lands in the texture regardless. So the row width is used
    as the game uses it and :func:`census` checks the thing that has to be
    true: every declared index falls inside the sheet, on art.
    """
    width, height, pixels = blp.decode_sprite(package, sprite)
    if width % size or height % size or not width or not height:
        raise SystemExit(
            f"{sprite['name']} is {width}x{height}, not a whole number of the "
            f"{size}px cells its icon table declares")
    blp.roundtrip(package, sprite, pixels)
    return width, height, pixels


def ink(pixels, width, size, index, per_row):
    """(opaque pixels, bounding box) of one cell's artwork."""
    left, top = (index % per_row) * size, (index // per_row) * size
    x0 = y0 = size
    x1 = y1 = -1
    count = 0
    for y in range(size):
        at = ((top + y) * width + left) * 4 + 3
        for x in range(size):
            if pixels[at + x * 4] < ALPHA_FLOOR:
                continue
            count += 1
            x0, x1 = min(x0, x), max(x1, x)
            y0, y1 = min(y0, y), max(y1, y)
    return count, (x0, y0, x1 - x0 + 1, y1 - y0 + 1) if count else None


def census(pixels, width, height, size, per_row, declared, name):
    """Every index the icon tables declare lands inside the sheet, on art.

    A text table and a decoded texture describe the same sheet without either
    having read the other, so this is one prediction per declared icon -- 100
    of them for `ICON_ATLAS_UNITS` -- about pixels the tables never mention.
    Naming the wrong texture for an atlas, or reading it on the wrong row
    width, drops declared icons off the sheet or onto empty cells.

    ⚠ Not the reverse. `Units256` inks cells 26 and 98 and no icon definition
    anywhere in the install names either: Civilization VI ships two unit
    symbols its own tables cannot reach. Those are reported, not enforced.
    """
    across, down = width // size, height // size
    inked = {index for index in range(across * down)
             if ink(pixels, width, size, index, across)[0]}
    outside = sorted(index for index in declared
                     if index % per_row >= across or index // per_row >= down)
    if outside:
        raise SystemExit(f"{name} declares icons at cells {outside}, off a "
                         f"sheet {across} cells across and {down} down")
    seats = {index % per_row + (index // per_row) * across for index in declared}
    blank = sorted(seats - inked)
    if blank:
        raise SystemExit(f"{name} declares icons on cells {blank}, and those "
                         f"cells are empty")
    return len(inked), sorted(inked - seats)


# --------------------------------------------------------------------- the sheet


def shrink(pixels, width, size, index, per_row, cell):
    """One cell, box-averaged down to the sheet's cell size, as a white mask.

    Civilization VI stores these premultiplied and tints them flat -- its own
    `UnitFlagManager` calls `UnitIcon:SetColor(secondaryColor)` -- and the
    spectator does the same with a `source-in` fill, so what carries the shape
    is the alpha. Averaging `k * k` source pixels into one keeps a Crossbowman's
    string and a Frigate's rigging visible at counter size rather than dropping
    them between samples.
    """
    k = size // cell
    left, top = (index % per_row) * size, (index // per_row) * size
    out = bytearray(cell * cell * 4)
    for y in range(cell):
        rows = [((top + y * k + dy) * width + left) * 4 + 3 for dy in range(k)]
        for x in range(cell):
            total = 0
            for row in rows:
                at = row + x * k * 4
                for dx in range(k):
                    total += pixels[at + dx * 4]
            alpha = (total + k * k // 2) // (k * k)
            out[(y * cell + x) * 4:(y * cell + x) * 4 + 4] = (255, 255, 255, alpha)
    return out


def fnv1a64(data):
    """A compact change detector, the one `rules.rs` already uses."""
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return value


def measure(sheet, width, index, cell, columns):
    """(opaque pixels, bounding box) of one finished cell of the sheet."""
    return ink(sheet, width, cell, index, columns)


# ---------------------------------------------------------------------- the runs


def resolve():
    """One row per ruleset unit: where in the install its glyph comes from."""
    atlases, icons = icon_tables()
    civ6 = civ6_unit_types()
    replaced = replacements()
    rows, missing = [], []
    for unit in roster():
        kind = civ6(unit)
        icon = "ICON_" + kind
        via = None
        if icon not in icons:
            stand_in = replaced.get(unit)
            candidate = "ICON_" + civ6(stand_in) if stand_in else None
            if candidate and candidate in icons:
                icon, via = candidate, stand_in
            else:
                missing.append((unit, kind))
                continue
        atlas, index, _pack = pick(icons[icon])
        if SOURCE not in atlases.get(atlas, {}):
            missing.append((unit, f"{atlas} has no {SOURCE}px texture"))
            continue
        per_row, per_column, texture, pack = atlases[atlas][SOURCE]
        rows.append(dict(type=unit, civ6_type=kind, icon=icon, atlas=atlas,
                         atlas_index=index, texture=texture, pack=pack,
                         per_row=per_row, per_column=per_column, via=via))
    if missing:
        raise SystemExit("the install defines no unit glyph for: "
                         + ", ".join(f"{unit} ({why})" for unit, why in missing))
    return atlases, icons, rows


def cut():
    """Write the atlas and its manifest from the installed game."""
    atlases, icons, rows = resolve()
    library = packages()
    sheet_rows = -(-len(rows) // COLUMNS)
    width = COLUMNS * CELL
    sheet = bytearray(width * sheet_rows * CELL * 4)
    opened = {}
    for index, row in enumerate(rows):
        key = (row["pack"], row["texture"])
        if key not in opened:
            if key not in library:
                raise SystemExit(f"no package under {row['pack']} holds the "
                                 f"texture {row['texture']}")
            relative, package, sprite = library[key]
            size = SOURCE
            decoded = cells(package, sprite, size, row["per_row"],
                            row["per_column"])
            width_, height_, pixels_ = decoded
            declared = {index for entries in icons.values()
                        for atlas, index, _pack in entries if atlas == row["atlas"]}
            drawn, unnamed = census(pixels_, width_, height_, size,
                                    row["per_row"], declared, row["atlas"])
            opened[key] = (relative, decoded, drawn)
            print(f"{row['atlas']:36s} {row['texture']:20s} "
                  f"{decoded[0]:4d}x{decoded[1]:<4d} {drawn:3d} inked, "
                  f"{len(declared):3d} named"
                  + (f", {len(unnamed)} unnamed {unnamed}" if unnamed else ""))
        relative, (source_width, _h, pixels), _drawn = opened[key]
        row["package"] = relative
        cell = shrink(pixels, source_width, SOURCE,
                      seat(row["atlas_index"], row["per_row"],
                           source_width // SOURCE), source_width // SOURCE, CELL)
        column, line = index % COLUMNS, index // COLUMNS
        for y in range(CELL):
            at = ((line * CELL + y) * width + column * CELL) * 4
            sheet[at:at + CELL * 4] = cell[y * CELL * 4:(y + 1) * CELL * 4]
        row["index"] = index

    blank = [row["type"] for row in rows
             if not measure(sheet, width, row["index"], CELL, COLUMNS)[0]]
    if blank:
        raise SystemExit(f"cut a blank cell for {blank}")

    blp.write_png(ATLAS, width, sheet_rows * CELL, sheet)
    art = ATLAS.read_bytes()
    for row in rows:
        count, box = measure(sheet, width, row["index"], CELL, COLUMNS)
        row["ink"], row["box"] = count, list(box)
        for key in ("pack", "per_row", "per_column"):
            row.pop(key)
        if row["via"] is None:
            row.pop("via")
    manifest = {
        "description": "Civilization VI unit glyphs used by the CIVVIS command map",
        "source": "Civilization VI's own ICON_ATLAS_*_UNITS icon atlases, "
                  "read out of the installed game by tools/civ6_unit_glyphs.py",
        "copyright": "Civilization VI unit artwork is owned by Firaxis Games "
                     "and 2K; no ownership is claimed here.",
        "cell_size": CELL,
        "columns": COLUMNS,
        "rows": sheet_rows,
        "source_cell_size": SOURCE,
        "png_bytes": len(art),
        "png_fnv1a64": f"{fnv1a64(art):#018x}",
        "units": [{key: row[key] for key in
                   ("type", "index", "civ6_type", "icon", "atlas",
                    "atlas_index", "texture", "package", "via", "ink", "box")
                   if key in row} for row in rows],
    }
    MANIFEST.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"wrote {ATLAS} ({width}x{sheet_rows * CELL}, {len(rows)} units) "
          f"and {MANIFEST}")


def verify():
    """Re-check the committed atlas against its manifest. No install needed."""
    manifest = json.loads(MANIFEST.read_text())
    art = ATLAS.read_bytes()
    if len(art) != manifest["png_bytes"] or \
            f"{fnv1a64(art):#018x}" != manifest["png_fnv1a64"]:
        raise SystemExit("the manifest describes a different atlas than the "
                         "one committed beside it")
    width, height, pixels = read_png(art)
    cell, columns = manifest["cell_size"], manifest["columns"]
    if width != columns * cell or height != manifest["rows"] * cell:
        raise SystemExit(f"the atlas is {width}x{height}, not the "
                         f"{columns * cell}x{manifest['rows'] * cell} the "
                         f"manifest describes")
    units = manifest["units"]
    if [row["type"] for row in units] != roster():
        raise SystemExit("the atlas roster is not the ruleset's units in order")
    for row in units:
        count, box = ink(pixels, width, cell, row["index"], columns)
        if not count:
            raise SystemExit(f"{row['type']} has a blank cell")
        if count != row["ink"] or list(box) != row["box"]:
            raise SystemExit(f"{row['type']} measures {count} px {box}, not "
                             f"the {row['ink']} px {tuple(row['box'])} the "
                             f"manifest records")
    seats = {row["index"] for row in units}
    if seats != set(range(len(units))):
        raise SystemExit("the units do not fill the sheet's cells exactly once")
    print(f"{len(units)} units, {width}x{height}, every cell inked and every "
          f"measurement reproduced")


def read_png(data):
    """(width, height, RGBA) of an 8-bit truecolour-alpha PNG."""
    width = height = None
    stream = b""
    at = 8
    while at < len(data):
        length = struct.unpack_from(">I", data, at)[0]
        kind = data[at + 4:at + 8]
        if kind == b"IHDR":
            width, height, depth, colour = struct.unpack_from(">IIBB", data, at + 8)
            if (depth, colour) != (8, 6):
                raise SystemExit(f"the atlas is depth {depth} colour {colour}")
        elif kind == b"IDAT":
            stream += data[at + 8:at + 8 + length]
        at += 12 + length
    raw = zlib.decompress(stream)
    stride = width * 4
    out = bytearray()
    previous = bytearray(stride)
    at = 0
    for _ in range(height):
        filtered, line = raw[at], bytearray(raw[at + 1:at + 1 + stride])
        at += 1 + stride
        if filtered == 1:
            for x in range(4, stride):
                line[x] = (line[x] + line[x - 4]) & 0xFF
        elif filtered == 2:
            for x in range(stride):
                line[x] = (line[x] + previous[x]) & 0xFF
        elif filtered == 3:
            for x in range(stride):
                left = line[x - 4] if x >= 4 else 0
                line[x] = (line[x] + ((left + previous[x]) >> 1)) & 0xFF
        elif filtered == 4:
            for x in range(stride):
                a = line[x - 4] if x >= 4 else 0
                b = previous[x]
                c = previous[x - 4] if x >= 4 else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                near = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[x] = (line[x] + near) & 0xFF
        elif filtered:
            raise SystemExit(f"unsupported PNG filter {filtered}")
        out += line
        previous = line
    return width, height, out


def self_test():
    """Show the oracles can fail: a one-pixel edit and a shifted grid."""
    atlases, icons, _rows = resolve()
    per_row, per_column, texture, pack = atlases["ICON_ATLAS_UNITS"][SOURCE]
    _relative, package, sprite = packages()[(pack, texture)]
    width, height, pixels = cells(package, sprite, SOURCE, per_row, per_column)
    declared = {index for entries in icons.values()
                for atlas, index, _pack in entries if atlas == "ICON_ATLAS_UNITS"}
    drawn, unnamed = census(pixels, width, height, SOURCE, per_row,
                            declared, "ICON_ATLAS_UNITS")
    print(f"honest parse: {width}x{height}, {len(declared)} declared icons all "
          f"land on inked cells of {drawn} ({len(unnamed)} inked cells the "
          f"install never names), {sprite['blob'][1]} bytes re-encoded exactly")

    edited = bytearray(pixels)
    edited[(1000 * width + 1000) * 4 + 3] ^= 1
    try:
        blp.roundtrip(package, sprite, edited)
        raise SystemExit("FAILED: one flipped alpha bit went unnoticed")
    except SystemExit as failure:
        if "FAILED" in str(failure):
            raise
        print(f"one alpha bit flipped -> {failure}")

    try:
        census(pixels, width, height, SOURCE, per_row * 2, declared,
               "ICON_ATLAS_UNITS")
        raise SystemExit("FAILED: a doubled row stride went unnoticed")
    except SystemExit as failure:
        if "FAILED" in str(failure):
            raise
        print(f"row stride doubled -> {str(failure)[:120]}")


def listing():
    """Print the resolved icon table without cutting anything."""
    _atlases, _icons, rows = resolve()
    for index, row in enumerate(rows):
        stand_in = f"  (via {row['via']})" if row["via"] else ""
        print(f"{index:3d} {row['type']:26s} {row['icon']:44s} "
              f"{row['atlas']:36s} {row['atlas_index']:3d}{stand_in}")
    print(f"{len(rows)} units over "
          f"{len({row['atlas'] for row in rows})} atlases")


if __name__ == "__main__":
    flags = set(sys.argv[1:])
    if "--verify" in flags:
        verify()
    elif "--self-test" in flags:
        self_test()
    elif "--list" in flags:
        listing()
    else:
        cut()
