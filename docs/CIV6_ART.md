# Reading Civilization VI's own art off the installed game

CIVVIS' spectator is judged against a game that is installed on the same disk.
Every mark on its map is therefore either **cut** from that install or
**imitated**, and the difference is visible: an imitation is somebody's memory
of a shape, and it reads as a different game's counter no matter how carefully
it is drawn.

This document records the container formats, so the next piece of art is a
morning's work rather than a week's.

Two rules hold for all of it:

- **Resolve the install through `tools/civ6_env.py` and nothing else.**
  `tools/test_civ6_env.py` fails any tool in `tools/` that greps for a Steam
  path of its own, because a private search silently finds nothing on somebody
  else's machine.
- **This Mac's `python3` has neither Pillow nor numpy.** Decoders and PNG
  writers here are stdlib: `struct` and `zlib` are enough for both.

## `TEXTURE_*` — the `CIVBIG` container

Loose files under `Base/Platforms/Windows/BLPs/**`, one texture each.
`tools/civ6_yield_icons.py` reads them.

| offset | meaning |
|---|---|
| `0x00` | magic `CIVBIG\0\0` |
| `0x22` | mip count (u16 LE) |
| `0x26` | width (u16 LE) |
| `0x28` | height (u16 LE) |
| `0x30` | DXT5/BC3 payload, top mip first |

A hand-written BC3 decoder is about forty lines.

## `*.blp` — the `CIVBLP` package

`Base/Platforms/Windows/BLPs/UI/InWorld.blp` and its neighbours are *not* that
format and hold no loose textures. A `.blp` is a serialized C++ object graph
that carries its **own reflection data**, so the layout of everything inside it
can be read out of the file rather than guessed.
`tools/civ6_unit_flag_plates.py` parses it and its docstring is the long
version; the short version:

| offset | meaning |
|---|---|
| `0x00` | magic `CIVBLP` + u16 version (2) |
| `0x08` | offset of the package section (`0x400`) |
| `0x0C` | **size** of the package section |
| `0x10` | offset of the big-data section |
| `0x14` | entry count |
| `0x18` | file size |

⚠ **`0x0C` is a size, not an offset.** `0x08 + 0x0C == 0x10` exactly. Reading
it as "the allocation table" is what stalled the first attempt on this file:
the 0x400 bytes it points at are the real table's zero tail, and every
structure derived from that address is off.

The package section is `[type-info stripe][stripe 0][stripe 1][allocation
table]`. The allocation table is 40-byte `Serialization::PackageAllocation`
records — stripe, offset, size, element count, and a type name — indexed from
**1**, because index 0 is the null pointer. Every pointer in the graph is
stored as `(u32 allocation index, u32 offset into it)`, which is what makes the
package relocatable and what makes it readable without knowing where anything
was loaded.

Three properties turn that into a parser with no magic addresses:

1. The table is found from the single record reading `(offset 0, size
   16 * entry count, element count entry count)` — the package's own entry map,
   which is always allocation **1** — then walked forward in 40-byte steps to
   the terminating zero record.
2. Allocations tile their stripe in index order at 16-byte alignment, and the
   stripes are laid end to end finishing exactly at the table, so both stripe
   bases fall out of the table itself.
3. Resolving each record's type name names every allocation, and the names are
   the real C++ ones: `ForgeUI::TexturePackageEntry`, `BLP::TextureEntry`,
   `BLP::TBufferEntry`, `char`.

⚠ Property 1 is the reason one parser now reads every UI package instead of one
file. It used to seek the two-word `(size, count)` pair and then walk
*backwards* while the preceding record looked plausible, which is fine in
`InWorld.blp` and wrong wherever the table is preceded by string data that
reads as a plausible record. `Portugal/…/Icons.blp` is one: the walk stepped
one record too far, every allocation index shifted by one, and the parse died
resolving a type name out of range. Anchoring on `dwOffset == 0` needs no walk
and no guess, and it finds the table in 372 of the installed packages including
all 35 UI `Icons.blp`.

Two more package shapes that `InWorld.blp` does not have and most others do:

- **several `BLP::TBufferEntry` allocations.** Reading only the first leaves the
  rest of the sprites pointing at nothing.
- **no shared page at all.** `page()` returns `None` rather than refusing the
  package; a `BLP::TextureEntry` is the exception, not the rule.

### Sprites inside a UI package

`InWorld.blp` holds 122 named sprites — every city banner, unit flag, meter and
promotion tag the in-world UI draws. One of them lives on a shared 256×256
`R8G8B8A8` page with a 9-mip chain; the other 121 are each their own page,
stored as a **deduplicated block dictionary**:

```
[ block count * 4 bytes ]   dictionary of RGBA texels, edge*edge per block
[ index array           ]   one index per edge*edge block, row major,
                            1 or 2 bytes wide, padded to 4
```

with `edge` ∈ {1, 2, 4, 8}. At `edge = 1` that degenerates to an ordinary
palettised image; at 8 it is a coarse tile dictionary. Nothing in it is
DXT/BC despite the class being named `BCTexturePackageEntry`.

### How to know a parse of one of these is right

Three cheap oracles, all asserted by `_check` in
`tools/civ6_unit_flag_plates.py`:

- the shared page's declared byte count must equal the sum of its own mip
  chain at 4 bytes per texel;
- **every** sprite's `blocks * 4 + padded index bytes` must equal the byte
  length of the buffer it points at — 121 independent predictions of a number
  the parse never read in `InWorld.blp`, 548 in `Icons.blp`;
- and from the other end, no index in a sprite's array may name a block beyond
  the `blocks / edge²` the dictionary holds. The size identity alone would
  survive a wrong `edge` paired with a compensating `blocks`; this would not.

Those predict **lengths**. `roundtrip()` predicts every **byte**, and is the
strongest thing this container allows. A sprite's whole stored blob is a pure
function of its image: the dictionary is the distinct `edge × edge` blocks in
**first-occurrence order**, deduplicated, and the index array names each
block's place in it, row major, padded to four bytes. So re-encode a decoded
sprite and the package's own bytes come back — 98 of `InWorld.blp`'s sprites
and 402 of `Icons.blp`'s do, exactly. Flip one alpha bit of a 4096×2048 atlas
and the comparison fails.

⚠ It applies only where width and height are whole multiples of `edge`. A
ragged sprite was blocked by the encoder over a padded image, and the padding
is not in the decode to re-encode; 7 of `InWorld.blp`'s 23 ragged sprites
cannot be reproduced for that reason. Every icon atlas is a whole number of
cells, so a cut is verified this way end to end.

A parse that predicts numbers it did not use to build itself is right. One that
merely produces a plausible picture is not: the first attempt on this file
decoded the raw bytes at the big-data offset as a 256-wide image, got a
recognisable flag silhouette, and concluded it had found the military flag. It
had found `BuilderRecommendation`, the improvement-recommendation banner, which
shares the flag family's shape. None of the eight real flags is at that offset.

## Naming a cell: the game's own icon tables

A texture is half of a cut. The other half is which cell is which, and
Civilization VI answers that in data rather than in art. Every icon in the game
is a row in one of two tables, spread over 338 XML files under `Base/Assets/UI/
Icons/` and each DLC's `Data/`:

```xml
<Row Name="ICON_ATLAS_UNITS" IconSize="256" IconsPerRow="16"
     IconsPerColumn="8" Filename="Units256.dds"/>
<Row Name="ICON_UNIT_WARRIOR" Atlas="ICON_ATLAS_UNITS" Index="20"/>
```

`Filename`'s stem is the texture's name inside that pack's `BLPs/UI/Icons.blp`,
and a cell is `(index % IconsPerRow, index / IconsPerRow)`. Three warnings, all
learned by getting them wrong:

- ⚠ **Strip XML comments first.** Firaxis moved the Winged Hussar's icon into
  the Poland pack and left the base row behind commented out. Reading disabled
  rows put the Hussar on `ICON_ATLAS_UNITS` cell 46, which draws a banner, and
  the sheet looked entirely plausible — the cross-check below is what caught it.
- ⚠ **`IconsPerColumn` is wrong, and `IconsPerRow` sometimes is.** Every DLC
  unit atlas declares `IconsPerColumn="1"` and several ship more:
  `XP2_Units256` is 1024×1536, six rows of four, and `Expansion2_Icons_Units.xml`
  indexes cell 19 inside it anyway. `Portugal_Icons_Units.xml` declares a 4×4
  sheet for a pack that ships one unit, and `Portugal_Units256` is a single
  256×256 cell. Neither error reaches the game, because every index those packs
  actually declare lands in the texture regardless. Check what has to be true —
  every declared index falls inside the sheet, on ink — not what the table says.
- ⚠ **The reverse census does not hold.** `Units256` inks cells 26, 46 and 98
  and no icon definition anywhere in the install names any of them.

## What has been cut so far

| asset | tool | source |
|---|---|---|
| tile yield signs | `tools/civ6_yield_icons.py` | `TEXTURE_YieldOverlayAtlas` |
| unit flag plates | `tools/civ6_unit_flag_plates.py` | `BLPs/UI/InWorld.blp` |
| unit glyphs | `tools/civ6_unit_glyphs.py` | `ICON_ATLAS_*_UNITS` in 8 `Icons.blp` |
| city-banner wall shields | `tools/civ6_city_banner_art.py` | `Banner_StrengthIcon_Shields` in `BLPs/UI/InWorld.blp` |
| district colours | inline in `web/assets/app.js` | `Base/Assets/UI/Civ6_ColorAtlas.xml` |

Every mark CIVVIS draws is now cut, not imitated and not scraped.
`tools/civ6_unit_flags.swift` — 89 Civilopedia cards downloaded from the
Civilization Wiki, their shared background removed by a low per-pixel
percentile across the set — is gone, and with it the last third-party
dependency in this repository's art.

### ⚠ `UnitFlagAtlasWhite` is not the in-world glyph sheet

This document used to say it was, and left cutting it as the follow-up. That
was wrong on all three counts that matter:

1. `UnitFlagManager.lua:SetFlagUnitEmblem` asks for `"ICON_" ..
   GameInfo.Units[type].UnitType` — `ICON_UNIT_WARRIOR`, no suffix — which is
   defined only in `ICON_ATLAS_UNITS` and its DLC siblings. No shipped Lua
   anywhere in the install asks for an `ICON_UNIT_*_WHITE`, the only names bound
   to `ICON_ATLAS_UNIT_FLAG_SYMBOLS_WHITE`. (That atlas row's own `Filename` is
   `Units32.dds`, which is `ICON_ATLAS_UNITS`' texture, not `UnitFlagAtlasWhite`.)
2. **Nothing in the install says what its cells are.** Its 93 glyphs are not the
   102 of `Units32`/`Units256`: matching the two cell for cell scores a median
   silhouette IoU of 0.39, no better than pairing each cell with an unrelated
   one (0.40), and only 4 of 93 have a strong twin. It is an orphan sheet, and
   naming its cells would have meant a hand-written guess.
3. It could not cover the roster. `Icons_UnitFlags.xml` declares no flag symbol
   for the Guru, the Warrior Monk, Modern Armor, Modern AT or the Missile
   Cruiser — their declared indices land on empty cells — and none at all for
   any expansion or DLC unique: no Toa, no Tagma, no Nihang.

It is still a real, decodable 512×256 sheet, and `UnitFlagAtlas22` in
`Icons.blp` is the same set at 22px. Nobody knows what either is for.

### The cross-check that caught a wrong cell

Two independent renderings of the same 90 units — the sheet cut from the
install, and the retired wiki scrape — should look alike unit for unit, because
the Civilopedia card art and the flag symbol are the same drawing. Normalising
each cell to its own silhouette box and comparing on a 24×24 grid:

| pairing | median IoU |
|---|---|
| each unit against **its own** old glyph | **0.867** |
| each unit against a **different** unit's old glyph | 0.284 |

89 of 90 resemble their own old glyph more than the control. The failures are
the interesting part and all three are explained: `nihang` (0.36) is the defect
being fixed — its old cell was a byte-identical copy of `warrior_monk`'s;
`oromo_cavalry` (0.48) deliberately stands on the Courser's icon, because
Civilization VI ships `ICON_UNIT_ETHIOPIAN_OROMO_CAVALRY_PORTRAIT` and no
matching symbol; and `winged_hussar` scored 0.26 until the commented-out row
above was found, at which point it joined the rest.

A comparison against a second, independent rendering is worth keeping in mind
for the next cut. It is not a standing test — the scrape it compares against is
deleted — but it is the only check here that could catch a *plausible* wrong
cell, and it did.

## When measuring, measure the set

Two lessons, learned twice:

- **A per-icon crop destroys a set's relative sizes.** Civilization VI authors
  each unit icon with its own margin — 38 to 57 px of ink in the same 64px cell,
  averaging 49 — so the cells *must* be measured individually or the same
  counter is drawn at sizes 1.5× apart (#2298). Cutting from the install did not
  change that: the margins are the game's, and the renderer still measures every
  cell's alpha once on load. The unit flags are the opposite case: all eight are
  authored in one
  64px cell at one scale, horizontally centred and deliberately seated at
  different heights, so they must be cut on **one shared window** or a Trade
  arrow ends up the size of a Support diamond.
- **A rim you can see is often not in the icon.** The dark ring around a yield
  sign is the plate underneath showing through, not part of the art; cutting it
  in would have drawn the ring twice. The unit flags are the other way round —
  the rim is authored into the flag — which is why the plate sheet keeps the
  top-left cell of each `_Combo` sheet and not the unrimmed body below it.
