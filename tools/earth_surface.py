#!/usr/bin/env python3
"""Rebuild `data/earth_surface.txt`, the real surface of Earth on a
half-degree grid, from four public datasets.

    python3 tools/earth_surface.py --out data/earth_surface.txt

Requires `numpy` and `h5py` (the relief grid is netCDF-4/HDF5) and downloads
its inputs on first run into `--cache`. Nothing in the engine needs any of
that: the committed text file is the only thing `src/mapgen.rs` reads.

Sources, all public domain or CC-equivalent:

* Natural Earth 1:50m `land` and `lakes` — coastlines and the lakes big
  enough to read at half a degree.
* SRTM15+ v2.7, resampled to 10 arc-minutes by the Generic Mapping Tools
  data server — elevation, which decides mountains and hills.
* Koeppen-Geiger climate classification (Kottek et al. 2006) at half a
  degree — which decides the terrain family and the vegetation.

Why half a degree: the largest world the engine builds is Ludicrous, whose
57,950 tiles average about 1.6 degrees across on the globe, so a half-degree
source is finer than the finest map by a comfortable margin and coarser than
anything the sampler could resolve is wasted bytes.
"""

from __future__ import annotations

import argparse
import collections
import json
import sys
import urllib.request
from pathlib import Path

import numpy as np

try:
    import h5py
except ImportError:  # pragma: no cover - the tool is not part of the build
    h5py = None

W, H = 720, 360
SUB = 4  # sub-samples per cell edge when rasterising a polygon

SOURCES = {
    "ne_50m_land.geojson": "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_50m_land.geojson",
    "ne_50m_lakes.geojson": "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_50m_lakes.geojson",
    "earth_relief_10m_p.grd": "https://oceania.generic-mapping-tools.org/server/earth/earth_relief/earth_relief_10m_p.grd",
    "koppen.txt": "http://koeppen-geiger.vu-wien.ac.at/data/Koeppen-Geiger-ASCII.txt",
}

# Surface classes, in the order `src/mapgen.rs` decodes them.
SEA, LAKE, GRASS, PLAINS, DESERT, TUNDRA, SNOW, MOUNTAIN = range(8)
NONE, FOREST, JUNGLE, MARSH = range(4)
SURFACE_NAMES = ["sea", "lake", "grassland", "plains", "desert", "tundra", "snow", "mountain"]
VEG_NAMES = ["none", "forest", "jungle", "marsh"]

# Elevation cuts, in metres, measured over one cell. Either test alone makes a
# mountain, because a range and a plateau are high in different ways: the Alps
# average only 1.9 km but climb 1.2 km inside a single cell, while Tibet
# averages 4.8 km and is nearly flat across any one of them. Hills work the
# same way one step down, which is what puts the Appalachians and the Deccan in
# a different class from Iowa and the Ganges plain.
MOUNTAIN_MEAN = 2600.0
MOUNTAIN_RELIEF = 1000.0
HILL_MEAN = 1200.0
HILL_RELIEF = 300.0


def fetch(cache: Path, name: str) -> Path:
    path = cache / name
    if not path.exists():
        cache.mkdir(parents=True, exist_ok=True)
        print(f"downloading {name} ...", file=sys.stderr)
        with urllib.request.urlopen(SOURCES[name], timeout=180) as response:
            path.write_bytes(response.read())
    return path


def rings(path: Path, keep=lambda properties: True):
    out = []
    for feature in json.load(open(path))["features"]:
        if not keep(feature["properties"]):
            continue
        geometry = feature["geometry"]
        if geometry is None:
            continue
        polygons = (
            [geometry["coordinates"]]
            if geometry["type"] == "Polygon"
            else geometry["coordinates"]
        )
        for polygon in polygons:
            for ring in polygon:
                out.append(np.asarray(ring, dtype=float))
    return out


def rasterise(polygon_rings, width: int, height: int) -> np.ndarray:
    """Even-odd scanline fill. Outer rings and holes both toggle, which is
    what even-odd means, so lakes cut out of an island fall out for free."""
    filled = np.zeros((height, width), dtype=np.int8)
    lats = 90.0 - (np.arange(height) + 0.5) * (180.0 / height)
    xs = -180.0 + (np.arange(width) + 0.5) * (360.0 / width)
    for ring in polygon_rings:
        y0, y1 = ring[:-1, 1], ring[1:, 1]
        x0, x1 = ring[:-1, 0], ring[1:, 0]
        low, high = np.minimum(y0, y1), np.maximum(y0, y1)
        first = max(0, int((90.0 - ring[:, 1].max()) / (180.0 / height)) - 1)
        last = min(height, int((90.0 - ring[:, 1].min()) / (180.0 / height)) + 2)
        for row in range(first, last):
            lat = lats[row]
            spans = (low <= lat) & (high > lat)
            if not spans.any():
                continue
            t = (lat - y0[spans]) / (y1[spans] - y0[spans])
            crossings = np.sort(x0[spans] + t * (x1[spans] - x0[spans]))
            for left, right in zip(crossings[0::2], crossings[1::2]):
                a = np.searchsorted(xs, left, "left")
                b = np.searchsorted(xs, right, "left")
                if b > a:
                    filled[row, a:b] ^= 1
    return filled.astype(bool)


def coarsen(fine: np.ndarray) -> np.ndarray:
    return fine.reshape(H, SUB, W, SUB).mean(axis=(1, 3))


def climate_of(code: str, latitude: float):
    """Koeppen-Geiger to (terrain, vegetation).

    B is the dry belt: BW is true desert, BS is steppe, which Civilization
    paints as Plains. A is the tropics: Af and Am carry rainforest, Aw and As
    are savanna. C is temperate and D is continental, both wooded; Cs is the
    dry-summer Mediterranean, which reads as Plains rather than Grassland. The
    third letter c or d is the subarctic taiga, tundra where it reaches far
    enough north and wooded plains where it does not. E is polar: EF is the
    ice caps and ET the tundra proper."""
    if code == "EF":
        return SNOW, NONE
    if code == "ET":
        return TUNDRA, NONE
    family = code[0]
    second = code[1] if len(code) > 1 else ""
    if family == "B":
        return (DESERT, NONE) if second == "W" else (PLAINS, NONE)
    if family == "A":
        return (GRASS, JUNGLE) if code in ("Af", "Am") else (PLAINS, NONE)
    if family == "C":
        return (PLAINS, FOREST) if second == "s" else (GRASS, FOREST)
    if family == "D":
        if code[-1] in ("c", "d"):
            return (TUNDRA if abs(latitude) >= 58.0 else PLAINS), FOREST
        return GRASS, FOREST
    return GRASS, NONE


def build(cache: Path):
    if h5py is None:
        sys.exit("h5py is required to read the relief grid: pip install h5py numpy")

    land = coarsen(rasterise(rings(fetch(cache, "ne_50m_land.geojson")), W * SUB, H * SUB))
    lakes = coarsen(
        rasterise(
            rings(fetch(cache, "ne_50m_lakes.geojson"), lambda p: (p.get("scalerank") or 0) <= 4),
            W * SUB,
            H * SUB,
        )
    )
    is_land = land >= 0.5
    is_lake = (lakes >= 0.5) & is_land
    is_ground = is_land & ~is_lake

    with h5py.File(fetch(cache, "earth_relief_10m_p.grd"), "r") as grid:
        scale = float(grid["z"].attrs["scale_factor"][0])
        elevation = np.asarray(grid["z"][:], dtype=np.float32)[::-1] * scale
    block = elevation.shape[0] // H
    blocks = elevation.reshape(H, block, W, block)
    mean_elevation = blocks.mean(axis=(1, 3))
    local_relief = blocks.max(axis=(1, 3)) - blocks.min(axis=(1, 3))

    koppen = np.full((H, W), "", dtype=object)
    for line in fetch(cache, "koppen.txt").read_text().splitlines()[1:]:
        parts = line.split()
        if len(parts) != 3:
            continue
        latitude, longitude, code = float(parts[0]), float(parts[1]), parts[2]
        row = int(round((90.0 - 0.25 - latitude) / 0.5))
        col = int(round((longitude + 180.0 - 0.25) / 0.5))
        if 0 <= row < H and 0 <= col < W:
            koppen[row, col] = code

    # A cell whose own centre missed the climate grid — most of them coastal —
    # takes the nearest classified cell's climate rather than a default.
    known = koppen != ""
    for _ in range(8):
        missing = is_ground & ~known
        if not missing.any():
            break
        for dj, di in ((0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (1, -1), (-1, 1), (-1, -1)):
            source = np.roll(np.roll(koppen, dj, axis=0), di, axis=1)
            has = np.roll(np.roll(known, dj, axis=0), di, axis=1)
            take = missing & has & ~known
            koppen[take] = source[take]
            known = known | take

    surface = np.full((H, W), SEA, dtype=np.uint8)
    vegetation = np.zeros((H, W), dtype=np.uint8)
    decided: dict = {}
    for row in range(H):
        latitude = 90.0 - 0.5 * row - 0.25
        for col in range(W):
            if not is_ground[row, col]:
                continue
            code = koppen[row, col] or "Cfb"
            key = (code, abs(latitude) >= 58.0)
            if key not in decided:
                decided[key] = climate_of(code, latitude)
            surface[row, col], vegetation[row, col] = decided[key]
    surface[is_lake] = LAKE

    # The ice caps keep their ice. Greenland and Antarctica are three
    # kilometres up, but what is up there is the sheet, not a range.
    ice = surface == SNOW
    mountain = (
        is_ground
        & ~ice
        & ((mean_elevation >= MOUNTAIN_MEAN) | (local_relief >= MOUNTAIN_RELIEF))
    )
    hills = (
        is_ground
        & ~ice
        & ~mountain
        & ((mean_elevation >= HILL_MEAN) | (local_relief >= HILL_RELIEF))
    )
    surface[mountain] = MOUNTAIN
    vegetation[mountain] = NONE

    packed = (surface | (hills.astype(np.uint8) << 3) | (vegetation << 4)).astype(np.uint8)
    return packed, is_ground, surface, hills, vegetation


def encode(packed: np.ndarray) -> list[str]:
    """Run-length encode row by row as `count:value` in base 36, which holds
    a full 720-cell run of ocean in two characters."""
    lines = []
    for row in range(H):
        tokens = []
        values = packed[row]
        start = 0
        for col in range(1, W + 1):
            if col == W or values[col] != values[start]:
                tokens.append(f"{np.base_repr(col - start, 36)}:{np.base_repr(int(values[start]), 36)}".lower())
                start = col
        lines.append(" ".join(tokens))
    return lines


def report(is_ground, surface, hills, vegetation):
    """Shares by area, not by cell: half-degree cells shrink towards the poles,
    so counting them would report Antarctica as a third of the world's land."""
    weight = np.cos(np.radians(np.repeat(
        (90.0 - (np.arange(H) + 0.5) * 0.5)[:, None], W, axis=1)))
    ground = (weight * is_ground).sum()
    print(f"land {100 * ground / weight.sum():.1f}% of the globe", file=sys.stderr)
    for value, name in enumerate(SURFACE_NAMES):
        share = (weight * is_ground * (surface == value)).sum() / ground
        if share:
            print(f"  {name:<10}{100 * share:5.1f}%", file=sys.stderr)
    print(f"  {'hills':<10}{100 * (weight * hills).sum() / ground:5.1f}%", file=sys.stderr)
    for value, name in enumerate(VEG_NAMES):
        share = (weight * is_ground * (vegetation == value)).sum() / ground
        if share:
            print(f"  {name:<10}{100 * share:5.1f}%", file=sys.stderr)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default="data/earth_surface.txt", type=Path)
    parser.add_argument("--cache", default=Path.home() / ".cache" / "civvis-earth", type=Path)
    parser.add_argument("--probe", action="store_true", help="print named places instead of writing")
    args = parser.parse_args()

    packed, is_ground, surface, hills, vegetation = build(args.cache)
    report(is_ground, surface, hills, vegetation)

    if args.probe:
        for name, latitude, longitude in PROBES:
            row = min(max(int((90.0 - latitude) / 0.5), 0), H - 1)
            col = int((longitude + 180.0) / 0.5) % W
            print(
                f"  {name:<14}{SURFACE_NAMES[surface[row, col]]:<10}"
                f"{'hills' if hills[row, col] else 'flat':<7}{VEG_NAMES[vegetation[row, col]]}"
            )
        return

    body = "\n".join(encode(packed))
    header = (
        "# Earth's real surface on a half-degree grid, 720 columns by 360 rows.\n"
        "# Row 0 is 90N..89.5N and column 0 is 180W..179.5W; each cell is read at\n"
        "# its own centre. Rebuild with tools/earth_surface.py; do not hand-edit.\n"
        "#\n"
        "# Sources: Natural Earth 1:50m land and lakes; SRTM15+ v2.7 at 10 arc-\n"
        "# minutes via the GMT data server; Koeppen-Geiger (Kottek et al. 2006).\n"
        "#\n"
        "# Each row is run-length encoded as `count:value` pairs in base 36, west\n"
        "# to east. A value packs three fields: bits 0-2 the surface (0 sea,\n"
        "# 1 lake, 2 grassland, 3 plains, 4 desert, 5 tundra, 6 snow,\n"
        "# 7 mountain), bit 3 hills, bits 4-5 the vegetation (0 none, 1 forest,\n"
        "# 2 jungle, 3 marsh).\n"
    )
    args.out.write_text(header + body + "\n")
    print(f"wrote {args.out} ({args.out.stat().st_size} bytes)", file=sys.stderr)


PROBES = [
    ("Everest", 27.99, 86.93), ("Tibet", 32.0, 88.0), ("Sahara", 23.0, 10.0),
    ("Amazon", -3.0, -62.0), ("Alps", 46.5, 9.5), ("Andes", -13.0, -72.0),
    ("Rockies", 39.5, -106.0), ("Gobi", 43.0, 105.0), ("Congo", -1.0, 21.0),
    ("Ganges", 25.5, 83.0), ("Nile", 30.0, 31.0), ("Rome", 41.9, 12.5),
    ("Britain", 52.5, -1.5), ("Siberia", 65.0, 100.0), ("Ukraine", 49.5, 31.5),
    ("Kalahari", -23.0, 22.0), ("Outback", -25.0, 131.0), ("Iowa", 42.0, -94.0),
    ("Yucatan", 20.0, -89.0), ("Java", -7.0, 110.0), ("Caspian", 42.0, 51.0),
    ("Greenland", 72.0, -40.0), ("Atacama", -23.5, -69.0), ("Iran", 32.0, 54.0),
    ("Mississippi", 38.0, -90.0), ("Anatolia", 39.0, 33.0), ("Scandinavia", 62.0, 14.0),
    ("Ethiopia", 9.5, 38.7), ("Deccan", 18.0, 76.0), ("NorthChina", 39.9, 116.4),
]

if __name__ == "__main__":
    main()
