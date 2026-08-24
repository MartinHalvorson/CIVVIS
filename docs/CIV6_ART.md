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

1. The table is found from the single record whose size is `16 * entry count`
   beside an element count of `entry count` — the package's own entry map —
   then walked outward in 40-byte steps.
2. Allocations tile their stripe in index order at 16-byte alignment, and the
   stripes are laid end to end finishing exactly at the table, so both stripe
   bases fall out of the table itself.
3. Resolving each record's type name names every allocation, and the names are
   the real C++ ones: `ForgeUI::TexturePackageEntry`, `BLP::TextureEntry`,
   `BLP::TBufferEntry`, `char`.

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

Both oracles are cheap and both are asserted by `_check` in
`tools/civ6_unit_flag_plates.py`:

- the shared page's declared byte count must equal the sum of its own mip
  chain at 4 bytes per texel;
- **every** sprite's `blocks * 4 + padded index bytes` must equal the byte
  length of the buffer it points at — 121 independent predictions of a number
  the parse never read.

A parse that predicts numbers it did not use to build itself is right. One that
merely produces a plausible picture is not: the first attempt on this file
decoded the raw bytes at the big-data offset as a 256-wide image, got a
recognisable flag silhouette, and concluded it had found the military flag. It
had found `BuilderRecommendation`, the improvement-recommendation banner, which
shares the flag family's shape. None of the eight real flags is at that offset.

## What has been cut so far

| asset | tool | source |
|---|---|---|
| tile yield signs | `tools/civ6_yield_icons.py` | `TEXTURE_YieldOverlayAtlas` |
| unit flag plates | `tools/civ6_unit_flag_plates.py` | `BLPs/UI/InWorld.blp` |
| district colours | inline in `web/assets/app.js` | `Base/Assets/UI/Civ6_ColorAtlas.xml` |

⚠ `web/assets/civ6-unit-flags.png`, the unit **glyph** atlas built by
`tools/civ6_unit_flags.swift`, is the odd one out: it is scraped from
Civilopedia card images archived by the Civilization Wiki rather than read off
the install. `UnitFlagAtlasWhite` inside `InWorld.blp` is the game's own
in-world unit glyph sheet, 512×256, and re-cutting the atlas from it would
close that gap and drop the wiki dependency. It is a separate task from this
one and nobody has taken it.

## When measuring, measure the set

Two lessons, learned twice:

- **A per-icon crop destroys a set's relative sizes.** The unit glyph atlas is
  cut from Civilopedia cards whose margins are per-icon, so its cells *must* be
  measured individually or the same counter is drawn at sizes 1.6× apart
  (#2298). The unit flags are the opposite case: all eight are authored in one
  64px cell at one scale, horizontally centred and deliberately seated at
  different heights, so they must be cut on **one shared window** or a Trade
  arrow ends up the size of a Support diamond.
- **A rim you can see is often not in the icon.** The dark ring around a yield
  sign is the plate underneath showing through, not part of the art; cutting it
  in would have drawn the ring twice. The unit flags are the other way round —
  the rim is authored into the flag — which is why the plate sheet keeps the
  top-left cell of each `_Combo` sheet and not the unrimmed body below it.
