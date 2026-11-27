#!/usr/bin/env python3
r"""Extract Civilization VI's city-banner walls mark, for a local server only.

The game does not invent a generic wall pictograph for a city with outer
defenses.  Its CityBannerManager swaps ``Banner_StrengthIcon`` for the exact
``Banner_StrengthIcon_Shields`` texture in ``InWorld.blp``.  Somebody who owns
the game can have the local server draw that mark beside the blue
outer-defense meter instead of CIVVIS's own battlement (``tools/own_art.py``):

    python3 tools/civ6_city_banner_art.py
    python3 tools/civ6_city_banner_art.py --verify

The shared ``civ6_unit_flag_plates`` parser resolves the Civilization VI
install through ``civ6_env.py``, validates every package sprite, and can
round-trip this sprite back to the exact bytes in the game package.  The first
command writes ``local/civ6-art/city-wall-mark.png``, which git ignores and the
local server prefers; ``--verify`` regenerates it in a temporary directory and
compares. The PNG carries ``art_provenance.LOCAL_MARK``, so neither the
repository nor a published build will take it.
"""

from __future__ import annotations

import argparse
import tempfile
from pathlib import Path

import art_provenance
import civ6_env as env
import civ6_unit_flag_plates as blp


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = art_provenance.LOCAL_ART / "city-wall-mark.png"
SPRITE = "Banner_StrengthIcon_Shields"
DIMENSIONS = (21, 18)


def source_sprite() -> tuple[int, int, bytearray]:
    """The source RGBA pixels, after proving the package decode is exact."""
    package = blp.Package((env.assets_dir() / blp.IN_WORLD).read_bytes())
    sprites = package.assets()
    blp._check(package, sprites)
    if SPRITE not in sprites:
        raise SystemExit(f"{blp.IN_WORLD} does not contain {SPRITE}")
    sprite = sprites[SPRITE]
    width, height, pixels = blp.decode_sprite(package, sprite)
    if (width, height) != DIMENSIONS:
        raise SystemExit(f"{SPRITE} is {width}x{height}, not {DIMENSIONS[0]}x{DIMENSIONS[1]}")
    blp.roundtrip(package, sprite, pixels)
    if not any(pixels[3::4]):
        raise SystemExit(f"{SPRITE} is blank")
    return width, height, pixels


def write(destination: Path, image: tuple[int, int, bytearray] | None = None) -> None:
    """Write the game's one finished city-wall marker as an RGBA PNG."""
    width, height, pixels = image if image is not None else source_sprite()
    blp.write_png(destination, width, height, pixels)


def verify(destination: Path = OUTPUT) -> None:
    """Ensure the local PNG is the source sprite cut by this tool."""
    if not destination.is_file():
        raise SystemExit(f"missing local city-banner art: {destination}")
    with tempfile.TemporaryDirectory() as temporary:
        generated = Path(temporary) / destination.name
        write(generated)
        if destination.read_bytes() != generated.read_bytes():
            raise SystemExit(
                f"{destination} is not the current {SPRITE} extract; rerun "
                "tools/civ6_city_banner_art.py"
            )
    print(f"{destination.name}: {DIMENSIONS[0]}x{DIMENSIONS[1]} exact {SPRITE} extract")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", nargs="?", type=Path, default=OUTPUT)
    parser.add_argument("--verify", action="store_true",
                        help="compare the local output to a fresh game extract")
    args = parser.parse_args()
    if args.verify:
        verify(args.destination)
    else:
        args.destination.parent.mkdir(parents=True, exist_ok=True)
        write(args.destination)
        print(f"wrote {args.destination} ({DIMENSIONS[0]}x{DIMENSIONS[1]} {SPRITE})")


if __name__ == "__main__":
    main()
