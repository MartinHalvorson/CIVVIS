#!/usr/bin/env python3
"""Draws every map mark the spectator ships, so none of it is cut from a game.

    python3 tools/own_art.py           # redraw the sheets, their index and the manifest
    python3 tools/own_art.py --check   # redraw in memory; fail if a committed byte differs
    python3 tools/own_art.py --fetch   # refresh tools/own_art_icons.json (network)

Four sheets in `web/assets/`, each keeping the cell layout the viewer already
measures:

| sheet | layout | what is on it |
|---|---|---|
| `unit-glyphs.png` (+ `.json`) | 12 x 12 cells of 64 px, one per unit in sorted roster order | a white silhouette from game-icons.net (CC BY 3.0) |
| `unit-plates.png` | 8 cells of 60 px in one row | the counter shapes, drawn here |
| `yield-icons.png` | 6 cells of 80 px in one row | a coloured disc drawn here, carrying a game-icons.net pictograph |
| `city-wall-mark.png` | one 21 x 18 sprite | a battlement, drawn here |

Nothing here reads the Civilization VI install. The cutters in `tools/civ6_*`
still can, for somebody who owns the game, and they write into
`local/civ6-art/`, which git ignores and which only the local server reads
(docs/CIV6_ART.md). `tools/art_provenance.py` is the check that keeps it that
way: every image in the tree and in a published bundle is either listed in
`web/assets/art-manifest.json` with its hash, or refused.

The icons are not fetched at draw time. `tools/own_art_icons.json` holds the
path data of exactly the icons used, with each one's author and licence, at a
pinned revision of https://github.com/game-icons/icons, so a redraw needs no
network and gives the same bytes. Like the cutters, this is stdlib only: the
rasteriser below is an SVG path parser and a scanline filler with exact
horizontal and 5x vertical coverage, and the PNG writer is `zlib`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import struct
import sys
import urllib.request
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "web/assets"
ICON_DATA = ROOT / "tools/own_art_icons.json"
MANIFEST = ASSETS / "art-manifest.json"
CREDITS = ASSETS / "ART_CREDITS.md"

ICONS_REPOSITORY = "https://github.com/game-icons/icons"
ICONS_COMMIT = "82d948812bfe3f269ef8f731dcdb07b08160edc4"
ICONS_LICENCE = "CC BY 3.0"
ICONS_LICENCE_URL = "https://creativecommons.org/licenses/by/3.0/"
AUTHORS = {
    "lorc": "Lorc", "delapouite": "Delapouite", "skoll": "Skoll", "sbed": "Sbed",
    "cathelineau": "Cathelineau", "caro-asercion": "Caro Asercion",
    "darkzaitzev": "DarkZaitzev",
}

# One silhouette per unit. Several units share one, as a unit and the unit it
# replaces often do; the counter's shape and colour still tell them apart.
UNIT_ICONS = {
    "aircraft_carrier": "cathelineau/carrier",
    "anti_air_gun": "sbed/turret",
    "apostle": "delapouite/sun-priest",
    "archaeologist": "delapouite/trowel",
    "archer": "lorc/bowman",
    "artillery": "lorc/cannon",
    "at_crew": "lorc/rocket",
    "barbarian_horse_archer": "delapouite/bow-arrow",
    "barbarian_horseman": "delapouite/horseshoe",
    "barbary_corsair": "delapouite/pirate-captain",
    "battering_ram": "skoll/siege-ram",
    "battleship": "cathelineau/battleship",
    "berserker": "lorc/battle-axe",
    "biplane": "skoll/airplane",
    "bireme": "delapouite/galley",
    "black_army": "delapouite/black-knight-helm",
    "bombard": "lorc/cannon-shot",
    "bomber": "delapouite/stealth-bomber",
    "builder": "delapouite/wheelbarrow",
    "caravel": "delapouite/caravel",
    "carolean": "skoll/musket",
    "catapult": "skoll/ballista",
    "cavalry": "delapouite/cavalry",
    "conquistador": "lorc/crested-helmet",
    "cossack": "lorc/crossed-sabres",
    "courser": "delapouite/horse-head",
    "crossbowman": "lorc/heavy-arrow",
    "crouching_tiger": "delapouite/tiger",
    "cuirassier": "lorc/breastplate",
    "de_zeven_provincien": "lorc/galleon",
    "destroyer": "delapouite/torpedo",
    "digger": "skoll/brodie-helmet",
    "domrey": "delapouite/elephant",
    "dromon": "delapouite/trireme",
    "drone": "delapouite/delivery-drone",
    "eagle_warrior": "delapouite/eagle-head",
    "field_cannon": "delapouite/pirate-cannon",
    "fighter": "delapouite/plane-pilot",
    "frigate": "lorc/galleon",
    "gaesatae": "lorc/broadsword",
    "galley": "delapouite/galley",
    "garde_imperiale": "lorc/visored-helm",
    "giant_death_robot": "delapouite/battle-mech",
    "guru": "lorc/lotus",
    "heavy_chariot": "cathelineau/chariot",
    "helicopter": "delapouite/helicopter",
    "hetairoi": "delapouite/centurion-helmet",
    "highlander": "skoll/winchester-rifle",
    "hoplite": "delapouite/spartan-helmet",
    "horseman": "lorc/horse-head",
    "hulche": "lorc/stone-spear",
    "huszar": "delapouite/saber-and-pistol",
    "hwacha": "lorc/missile-swarm",
    "hypaspist": "delapouite/spiked-shield",
    "immortal": "lorc/arrows-shield",
    "impi": "delapouite/tribal-shield",
    "infantry": "skoll/stahlhelm",
    "inquisitor": "lorc/candle-flame",
    "ironclad": "cathelineau/iron-hulled-warship",
    "janissary": "delapouite/turban",
    "jet_bomber": "delapouite/stealth-bomber",
    "jet_fighter": "delapouite/jet-fighter",
    "jong": "delapouite/sail",
    "keshig": "lorc/pocket-bow",
    "khevsureti": "lorc/crossed-swords",
    "knight": "skoll/mounted-knight",
    "kongo_shield_bearer": "delapouite/shield-bash",
    "legion": "delapouite/roman-shield",
    "line_infantry": "skoll/musket",
    "llanero": "delapouite/spear-feather",
    "machine_gun": "skoll/machine-gun",
    "malon_raider": "lorc/bolas",
    "mamluk": "caro-asercion/cloaked-figure-on-horseback",
    "man_at_arms": "delapouite/black-knight-helm",
    "mandekalu_cavalry": "delapouite/cavalry",
    "maryannu_chariot_archer": "cathelineau/chariot",
    "mechanized_infantry": "skoll/apc",
    "medic": "sbed/medical-pack",
    "military_engineer": "delapouite/toolbox",
    "minas_geraes": "cathelineau/battleship",
    "missile_cruiser": "cathelineau/cruiser",
    "missionary": "delapouite/pilgrim-hat",
    "mobile_sam": "delapouite/missile-launcher",
    "modern_armor": "lorc/battle-tank",
    "modern_at": "lorc/missile-pod",
    "mountie": "skoll/winchester-rifle",
    "musketman": "skoll/musket",
    "naturalist": "delapouite/oak-leaf",
    "nau": "delapouite/caravel",
    "nihang": "delapouite/chakram",
    "nuclear_submarine": "delapouite/submarine-missile",
    "observation_balloon": "delapouite/air-balloon",
    "okihtcitaw": "lorc/feather",
    "oromo_cavalry": "delapouite/horse-head",
    "p51_mustang": "delapouite/plane-pilot",
    "pike_and_shot": "delapouite/sharp-halberd",
    "pikeman": "delapouite/pikeman",
    "pitati_archer": "delapouite/archer",
    "privateer": "delapouite/pirate-flag",
    "quadrireme": "delapouite/trireme",
    "ranger": "sbed/rifle",
    "redcoat": "skoll/musket",
    "rock_band": "lorc/guitar",
    "rocket_artillery": "lorc/missile-swarm",
    "rough_rider": "delapouite/cowboy-holster",
    "sabum_kibittum": "lorc/spear-hook",
    "saka_horse_archer": "delapouite/bow-arrow",
    "samurai": "delapouite/samurai-helmet",
    "scout": "delapouite/binoculars",
    "sea_dog": "delapouite/pirate-hat",
    "settler": "delapouite/old-wagon",
    "siege_tower": "delapouite/siege-tower",
    "skirmisher": "delapouite/sling",
    "slinger": "delapouite/sling",
    "spearman": "lorc/spears",
    "spec_ops": "lorc/parachute",
    "spy": "delapouite/spy",
    "submarine": "delapouite/submarine",
    "supply_convoy": "delapouite/truck",
    "swordsman": "cathelineau/swordman",
    "tagma": "skoll/mounted-knight",
    "tank": "lorc/tank",
    "toa": "delapouite/wood-club",
    "trader": "delapouite/caravan",
    "trebuchet": "delapouite/trebuchet",
    "u_boat": "delapouite/periscope",
    "varu": "delapouite/elephant-head",
    "viking_longship": "delapouite/drakkar",
    "voi_chien": "lorc/pocket-bow",
    "war_cart": "lorc/cartwheel",
    "warakaq": "delapouite/slingshot",
    "warrior": "lorc/stone-axe",
    "warrior_monk": "lorc/fist",
    "winged_hussar": "lorc/feathered-wing",
}

# The viewer's yield order (`CIV6_YIELD_ICON_KINDS` in app.js), the disc's
# colour, and the pictograph's ink and icon. The colours are the families the
# viewer's own drawn fallback already uses (`YPIP`), a little deeper so a
# white pictograph reads on them.
YIELDS = [
    ("food", "#4fbf3a", "#ffffff", "lorc/wheat"),
    ("production", "#e3762b", "#ffffff", "lorc/gear-hammer"),
    ("gold", "#f0c02e", "#4a3410", "delapouite/two-coins"),
    ("science", "#25a8dd", "#ffffff", "lorc/fizzing-flask"),
    ("culture", "#a457dc", "#ffffff", "lorc/drama-masks"),
    ("faith", "#e8d595", "#4a3410", "lorc/dove"),
]

PLATE_STYLES = ["base", "civilian", "naval", "support",
                "trade", "religion", "fortify", "embark"]

GLYPH_CELL, GLYPH_COLUMNS = 64, 12
PLATE_CELL = 60
YIELD_CELL = 80
WALL_SIZE = (21, 18)
# The viewer measures a cell's silhouette on alpha >= 12 (CIV6_UNIT_ICON_ALPHA_FLOOR).
ALPHA_FLOOR = 12
SUBSAMPLES = 5


# ----------------------------------------------------------------- SVG paths

_TOKEN = re.compile(r"[MmLlHhVvCcSsQqTtAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")


def _tokens(d: str):
    """Commands and numbers. Arc flags may be written without separators
    (`a1 1 0 011 1`), so they are read one character at a time below."""
    return _TOKEN.findall(d)


def path_polygons(d: str, transform, tolerance: float = .04) -> list[list[tuple[float, float]]]:
    """Every subpath of `d` as a closed polyline in output pixels.

    `transform` maps a path point to the output. Curves are flattened until
    every chord is within `tolerance` pixels of its curve.
    """
    tokens = _tokens(d)
    at = 0
    polys: list[list[tuple[float, float]]] = []
    current: list[tuple[float, float]] = []
    x = y = sx = sy = 0.0
    last_ctrl = None
    command = None

    def number():
        nonlocal at
        value = float(tokens[at])
        at += 1
        return value

    def flag():
        # A flag is one character; "011" is three numbers' worth of digits.
        nonlocal at
        token = tokens[at]
        if token in ("0", "1"):
            at += 1
            return token == "1"
        head, rest = token[0], token[1:]
        tokens[at] = rest
        return head == "1"

    def emit(px, py):
        current.append(transform(px, py))

    def cubic(x0, y0, x1, y1, x2, y2, x3, y3):
        p0, p1, p2, p3 = (transform(x0, y0), transform(x1, y1),
                          transform(x2, y2), transform(x3, y3))
        span = (math.dist(p0, p1) + math.dist(p1, p2) + math.dist(p2, p3))
        steps = max(1, min(64, int(math.ceil(math.sqrt(span / tolerance) * .5))))
        for i in range(1, steps + 1):
            t = i / steps
            u = 1 - t
            a, b, c, e = u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t
            current.append((a * p0[0] + b * p1[0] + c * p2[0] + e * p3[0],
                            a * p0[1] + b * p1[1] + c * p2[1] + e * p3[1]))

    def arc(x0, y0, rx, ry, phi, large, sweep, x1, y1):
        # SVG implementation notes F.6.5: endpoint to centre parameterisation.
        if rx == 0 or ry == 0 or (x0 == x1 and y0 == y1):
            emit(x1, y1)
            return
        rx, ry = abs(rx), abs(ry)
        cos_p, sin_p = math.cos(math.radians(phi)), math.sin(math.radians(phi))
        dx, dy = (x0 - x1) / 2, (y0 - y1) / 2
        x1p = cos_p * dx + sin_p * dy
        y1p = -sin_p * dx + cos_p * dy
        scale = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
        if scale > 1:
            rx, ry = rx * math.sqrt(scale), ry * math.sqrt(scale)
        num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
        den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
        coef = math.sqrt(max(0.0, num / den)) if den else 0.0
        if large == sweep:
            coef = -coef
        cxp, cyp = coef * rx * y1p / ry, -coef * ry * x1p / rx
        cx = cos_p * cxp - sin_p * cyp + (x0 + x1) / 2
        cy = sin_p * cxp + cos_p * cyp + (y0 + y1) / 2

        def angle(ux, uy, vx, vy):
            a = math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)
            return a

        theta = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
        delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry,
                      (-x1p - cxp) / rx, (-y1p - cyp) / ry)
        if not sweep and delta > 0:
            delta -= 2 * math.pi
        elif sweep and delta < 0:
            delta += 2 * math.pi
        radius = max(rx, ry) * abs(transform(1, 0)[0] - transform(0, 0)[0])
        steps = max(2, min(96, int(math.ceil(abs(delta) * math.sqrt(max(radius, 1) / tolerance) * .35))))
        for i in range(1, steps + 1):
            t = theta + delta * i / steps
            px, py = rx * math.cos(t), ry * math.sin(t)
            emit(cos_p * px - sin_p * py + cx, sin_p * px + cos_p * py + cy)

    while at < len(tokens):
        token = tokens[at]
        if token.isalpha():
            command = token
            at += 1
            if command in "Zz":
                if current:
                    polys.append(current)
                current = []
                x, y = sx, sy
                last_ctrl = None
                continue
        elif command is None:
            raise ValueError(f"path data starts with a number: {d[:40]!r}")
        relative = command.islower()
        op = command.upper()
        ox, oy = (x, y) if relative else (0.0, 0.0)
        if op == "M":
            if current:
                polys.append(current)
            x, y = number() + ox, number() + oy
            sx, sy = x, y
            current = [transform(x, y)]
            command = "l" if relative else "L"   # later pairs are line-tos
            last_ctrl = None
        elif op == "L":
            x, y = number() + ox, number() + oy
            emit(x, y)
            last_ctrl = None
        elif op == "H":
            x = number() + ox
            emit(x, y)
            last_ctrl = None
        elif op == "V":
            y = number() + oy
            emit(x, y)
            last_ctrl = None
        elif op == "C":
            x1, y1 = number() + ox, number() + oy
            x2, y2 = number() + ox, number() + oy
            x3, y3 = number() + ox, number() + oy
            cubic(x, y, x1, y1, x2, y2, x3, y3)
            last_ctrl = ("C", x2, y2)
            x, y = x3, y3
        elif op == "S":
            if last_ctrl and last_ctrl[0] == "C":
                x1, y1 = 2 * x - last_ctrl[1], 2 * y - last_ctrl[2]
            else:
                x1, y1 = x, y
            x2, y2 = number() + ox, number() + oy
            x3, y3 = number() + ox, number() + oy
            cubic(x, y, x1, y1, x2, y2, x3, y3)
            last_ctrl = ("C", x2, y2)
            x, y = x3, y3
        elif op in "QT":
            if op == "Q":
                qx, qy = number() + ox, number() + oy
            elif last_ctrl and last_ctrl[0] == "Q":
                qx, qy = 2 * x - last_ctrl[1], 2 * y - last_ctrl[2]
            else:
                qx, qy = x, y
            x3, y3 = number() + ox, number() + oy
            cubic(x, y, x + 2 / 3 * (qx - x), y + 2 / 3 * (qy - y),
                  x3 + 2 / 3 * (qx - x3), y3 + 2 / 3 * (qy - y3), x3, y3)
            last_ctrl = ("Q", qx, qy)
            x, y = x3, y3
        elif op == "A":
            rx, ry, phi = number(), number(), number()
            large, sweep = flag(), flag()
            x1, y1 = number() + ox, number() + oy
            arc(x, y, rx, ry, phi, large, sweep, x1, y1)
            x, y = x1, y1
            last_ctrl = None
        else:
            raise ValueError(f"unsupported path command {command!r}")
    if current:
        polys.append(current)
    return [poly for poly in polys if len(poly) > 2]


# ----------------------------------------------------------------- coverage

def coverage(polys, width: int, height: int, even_odd: bool = False) -> list[float]:
    """Fraction of each pixel the polygons cover, under the non-zero rule.

    Horizontal coverage is exact per sub-scanline; vertically each pixel row
    is sampled `SUBSAMPLES` times. Row major, `width * height` floats.
    """
    edges = []
    for poly in polys:
        for i in range(len(poly)):
            (x0, y0), (x1, y1) = poly[i - 1], poly[i]
            if y0 == y1:
                continue
            direction = 1
            if y0 > y1:
                x0, y0, x1, y1 = x1, y1, x0, y0
                direction = -1
            edges.append((y0, y1, x0, (x1 - x0) / (y1 - y0), direction))
    edges.sort()
    out = [0.0] * (width * height)
    share = 1.0 / SUBSAMPLES
    active: list = []
    next_edge = 0
    for row in range(height):
        acc = [0.0] * (width + 1)
        for sub in range(SUBSAMPLES):
            sy = row + (sub + .5) * share
            while next_edge < len(edges) and edges[next_edge][0] <= sy:
                active.append(edges[next_edge])
                next_edge += 1
            active = [edge for edge in active if edge[1] > sy]
            crossings = sorted((edge[2] + (sy - edge[0]) * edge[3], edge[4])
                               for edge in active if edge[0] <= sy)
            winding = 0
            for i, (cx, direction) in enumerate(crossings):
                before = winding
                winding += direction
                inside_before = (before % 2 == 1) if even_odd else before != 0
                if not inside_before or i == 0:
                    continue
                start = crossings[i - 1][0]
                _span(acc, start, cx, width, share)
            # Spans are added when the walk leaves them, above.
        base = row * width
        for x in range(width):
            out[base + x] = min(1.0, acc[x])
    return out


def _span(acc, a: float, b: float, width: int, weight: float) -> None:
    a = max(0.0, min(float(width), a))
    b = max(0.0, min(float(width), b))
    if b <= a:
        return
    ia, ib = int(a), int(b)
    if ia == ib:
        acc[ia] += (b - a) * weight
        return
    acc[ia] += (ia + 1 - a) * weight
    for x in range(ia + 1, ib):
        acc[x] += weight
    if ib < width:
        acc[ib] += (b - ib) * weight


def icon_polygons(name: str, icons: dict, box: tuple[float, float, float, float]):
    """An icon's white paths fitted into `box` = (x, y, w, h), viewBox 512."""
    entry = icons[name]
    x0, y0, w, h = box
    sx, sy = w / 512.0, h / 512.0
    polys = []
    for d in entry["paths"]:
        polys += path_polygons(d, lambda px, py: (x0 + px * sx, y0 + py * sy))
    return polys


# ----------------------------------------------------------------- images

class Image:
    """Straight-alpha RGBA in floats 0..1, row major."""

    def __init__(self, width: int, height: int):
        self.width, self.height = width, height
        self.px = [(0.0, 0.0, 0.0, 0.0)] * (width * height)

    def over(self, x: int, y: int, rgb, alpha: float) -> None:
        if alpha <= 0:
            return
        i = y * self.width + x
        r0, g0, b0, a0 = self.px[i]
        a = alpha + a0 * (1 - alpha)
        if a <= 0:
            return
        mix = lambda c, c0: (c * alpha + c0 * a0 * (1 - alpha)) / a  # noqa: E731
        self.px[i] = (mix(rgb[0], r0), mix(rgb[1], g0), mix(rgb[2], b0), a)

    def paint(self, cover, ox: int, oy: int, w: int, h: int, colour) -> None:
        """Composite a coverage block of `w * h` at (ox, oy). `colour` is an
        RGB tuple or a function (x, y) -> RGB in block coordinates."""
        for y in range(h):
            for x in range(w):
                c = cover[y * w + x]
                if c > 0:
                    rgb = colour(x, y) if callable(colour) else colour
                    self.over(ox + x, oy + y, rgb, c)

    def png(self, text: dict | None = None) -> bytes:
        raw = bytearray()
        stride = self.width * 4
        previous = bytes(stride)
        for y in range(self.height):
            row = bytearray()
            for x in range(self.width):
                r, g, b, a = self.px[y * self.width + x]
                q = lambda v: max(0, min(255, int(v * 255 + .5)))  # noqa: E731
                alpha = q(a)
                if alpha == 0:
                    row += b"\0\0\0\0"
                else:
                    row += bytes((q(r), q(g), q(b), alpha))
            raw += _filtered(bytes(row), previous)
            previous = bytes(row)
        return _png_bytes(self.width, self.height, bytes(raw), text)


def _filtered(row: bytes, previous: bytes) -> bytes:
    """The PNG filter libpng's heuristic would pick: least sum of |byte|."""
    best = None
    n = len(row)
    for kind in range(5):
        out = bytearray(n)
        for i in range(n):
            a = row[i - 4] if i >= 4 else 0
            b = previous[i]
            c = previous[i - 4] if i >= 4 else 0
            if kind == 0:
                p = 0
            elif kind == 1:
                p = a
            elif kind == 2:
                p = b
            elif kind == 3:
                p = (a + b) // 2
            else:
                e = a + b - c
                pa, pb, pc = abs(e - a), abs(e - b), abs(e - c)
                p = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
            out[i] = (row[i] - p) & 255
        score = sum(v if v < 128 else 256 - v for v in out)
        if best is None or score < best[0]:
            best = (score, kind, out)
    return bytes((best[1],)) + bytes(best[2])


def _chunk(kind: bytes, data: bytes) -> bytes:
    return (struct.pack(">I", len(data)) + kind + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF))


def _png_bytes(width: int, height: int, raw: bytes, text: dict | None) -> bytes:
    out = b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    for key, value in (text or {}).items():
        out += _chunk(b"tEXt", key.encode("latin-1") + b"\0" + value.encode("latin-1"))
    out += _chunk(b"IDAT", zlib.compress(raw, 9)) + _chunk(b"IEND", b"")
    return out


def hex_rgb(colour: str) -> tuple[float, float, float]:
    colour = colour.lstrip("#")
    return tuple(int(colour[i:i + 2], 16) / 255 for i in (0, 2, 4))


def mix(a, b, t: float):
    return tuple(x + (y - x) * t for x, y in zip(a, b))


WHITE, BLACK = (1.0, 1.0, 1.0), (0.0, 0.0, 0.0)
PROVENANCE = {"civvis-provenance": "drawn by tools/own_art.py; see web/assets/ART_CREDITS.md"}


# ----------------------------------------------------------------- unit glyphs

def roster() -> list[str]:
    """The ruleset's units, sorted: a cell is a unit's place in this list."""
    units = json.loads((ROOT / "data/units.json").read_text(encoding="utf-8"))
    return sorted(units)


def draw_unit_glyphs(icons: dict):
    units = roster()
    missing = sorted(set(units) - set(UNIT_ICONS))
    extra = sorted(set(UNIT_ICONS) - set(units))
    if missing or extra:
        raise SystemExit(f"UNIT_ICONS is not the roster: missing {missing}, unknown {extra}")
    rows = -(-len(units) // GLYPH_COLUMNS)
    rows = max(rows, GLYPH_COLUMNS)          # keep the square 12 x 12 sheet
    sheet = Image(GLYPH_COLUMNS * GLYPH_CELL, rows * GLYPH_CELL)
    index_rows = []
    inset = 2.0
    for index, unit in enumerate(units):
        ox = (index % GLYPH_COLUMNS) * GLYPH_CELL
        oy = (index // GLYPH_COLUMNS) * GLYPH_CELL
        name = UNIT_ICONS[unit]
        polys = icon_polygons(name, icons, (inset, inset, GLYPH_CELL - 2 * inset, GLYPH_CELL - 2 * inset))
        cover = coverage(polys, GLYPH_CELL, GLYPH_CELL)
        sheet.paint(cover, ox, oy, GLYPH_CELL, GLYPH_CELL, WHITE)
        xs = [i % GLYPH_CELL for i, c in enumerate(cover) if c * 255 >= ALPHA_FLOOR]
        ys = [i // GLYPH_CELL for i, c in enumerate(cover) if c * 255 >= ALPHA_FLOOR]
        author = name.split("/")[0]
        index_rows.append({
            "index": index,
            "type": unit,
            "icon": name,
            "author": AUTHORS[author],
            "licence": ICONS_LICENCE,
            "url": f"https://game-icons.net/1x1/{name}.html",
            "box": [min(xs), min(ys), max(xs) - min(xs) + 1, max(ys) - min(ys) + 1] if xs else None,
            "ink": len(xs),
        })
    return sheet, rows, index_rows


# ----------------------------------------------------------------- unit plates

def _circle(cx, cy, r, steps=96, start=0.0, end=2 * math.pi):
    return [(cx + r * math.cos(start + (end - start) * i / steps),
             cy + r * math.sin(start + (end - start) * i / steps)) for i in range(steps + 1)]


def _rounded(points, radius, steps=12):
    """A convex polygon with each corner replaced by an arc of `radius`."""
    out = []
    n = len(points)
    for i in range(n):
        p0, p1, p2 = points[i - 1], points[i], points[(i + 1) % n]
        v0 = (p0[0] - p1[0], p0[1] - p1[1])
        v2 = (p2[0] - p1[0], p2[1] - p1[1])
        l0, l2 = math.hypot(*v0), math.hypot(*v2)
        u0, u2 = (v0[0] / l0, v0[1] / l0), (v2[0] / l2, v2[1] / l2)
        half = math.acos(max(-1.0, min(1.0, u0[0] * u2[0] + u0[1] * u2[1]))) / 2
        cut = min(radius / math.tan(half), l0 / 2, l2 / 2)
        r = cut * math.tan(half)
        a = (p1[0] + u0[0] * cut, p1[1] + u0[1] * cut)
        b = (p1[0] + u2[0] * cut, p1[1] + u2[1] * cut)
        bis = (u0[0] + u2[0], u0[1] + u2[1])
        bl = math.hypot(*bis)
        centre = (p1[0] + bis[0] / bl * r / math.sin(half), p1[1] + bis[1] / bl * r / math.sin(half))
        a0 = math.atan2(a[1] - centre[1], a[0] - centre[0])
        a1 = math.atan2(b[1] - centre[1], b[0] - centre[0])
        sweep = a1 - a0
        while sweep > math.pi:
            sweep -= 2 * math.pi
        while sweep < -math.pi:
            sweep += 2 * math.pi
        for k in range(steps + 1):
            t = a0 + sweep * k / steps
            out.append((centre[0] + r * math.cos(t), centre[1] + r * math.sin(t)))
    return out


def plate_outlines() -> dict[str, list[list[tuple[float, float]]]]:
    """The eight counter shapes in unit coordinates (y down, reach <= 1).

    These are CIVVIS's own: a roundel for anything that fights, the viewer's
    long-standing rounded triangle (point down) for civilians, a hull for
    ships, a rounded square for support, a hexagon for trade, a quatrefoil
    for religion, a heater shield for a fortified unit and a boat for an
    embarked one.
    """
    tri = [(math.cos(math.pi / 2 + k * 2 * math.pi / 3), math.sin(math.pi / 2 + k * 2 * math.pi / 3))
           for k in range(3)]
    tri = [(x * 1.06, y * 1.06 - .04) for x, y in tri]
    corner = .2
    hull = (_circle(-.92 + corner, -.70 + corner, corner, 8, math.pi, 1.5 * math.pi)
            + _circle(.92 - corner, -.70 + corner, corner, 8, 1.5 * math.pi, 2 * math.pi)
            + [(.92 * math.cos(t), -.05 + .93 * math.sin(t))
               for t in [k * math.pi / 48 for k in range(49)]])
    hexagon = [(math.cos(k * math.pi / 3), .93 * math.sin(k * math.pi / 3)) for k in range(6)]
    shield = ([(-.86, -.86), (.86, -.86), (.86, -.05)]
              + [(.86 * math.cos(t), -.05 + 1.03 * math.sin(t)) for t in [k * math.pi / 64 for k in range(1, 32)]]
              + [(0.0, .98)]
              + [(-.86 * math.cos(t), -.05 + 1.03 * math.sin(t)) for t in [k * math.pi / 64 for k in range(31, 0, -1)]]
              + [(-.86, -.05)])
    boat = [(-1.0, -.62), (1.0, -.62), (.56, .74), (-.56, .74)]
    lobes = []
    for cx, cy in ((-.47, 0), (.47, 0), (0, -.47), (0, .47)):
        lobes.append(_circle(cx, cy, .5, 72))
    lobes.append([(-.5, -.5), (.5, -.5), (.5, .5), (-.5, .5)])
    return {
        "base": [_circle(0, 0, .95)],
        "civilian": [_rounded(tri, .30)],
        "naval": [hull],
        "support": [_rounded([(-.8, -.8), (.8, -.8), (.8, .8), (-.8, .8)], .24)],
        "trade": [_rounded(hexagon, .14)],
        "religion": lobes,
        "fortify": [shield],
        "embark": [_rounded(boat, .16)],
    }


def draw_unit_plates():
    """Grey plates the viewer tints by `multiply`: a white body shaded down
    to light grey, a darker rim, and a soft highlight inside the rim."""
    cell = PLATE_CELL
    scale = 27.0                 # unit radius in pixels; reach 1 -> 27 px of 30
    sheet = Image(cell * len(PLATE_STYLES), cell)
    shapes = plate_outlines()
    hi = 4
    for index, style in enumerate(PLATE_STYLES):
        ox = index * cell
        to_px = lambda pts, k=1: [((cell / 2 + x * scale) * k, (cell / 2 + y * scale) * k) for x, y in pts]  # noqa: E731
        polys = [to_px(poly) for poly in shapes[style]]
        alpha = coverage(polys, cell, cell)
        # Rim and highlight come from the distance to the edge, measured on a
        # 4x grid and averaged back down.
        big = coverage([to_px(poly, hi) for poly in shapes[style]], cell * hi, cell * hi)
        inside = [c >= .5 for c in big]
        dist = _distance_inside(inside, cell * hi, cell * hi)
        rim, glow = 1.7 * hi, 3.2 * hi
        top, bottom = 3.0, cell - 3.0
        for y in range(cell):
            for x in range(cell):
                a = alpha[y * cell + x]
                if a <= 0:
                    continue
                acc = [0.0, 0.0, 0.0]
                n = 0
                for sy in range(hi):
                    for sx in range(hi):
                        i = (y * hi + sy) * cell * hi + x * hi + sx
                        if not inside[i]:
                            continue
                        n += 1
                        d = dist[i]
                        t = min(1.0, max(0.0, ((y + (sy + .5) / hi) - top) / (bottom - top)))
                        body = mix((1.0, 1.0, 1.0), (.80, .80, .80), t)
                        if d < rim:
                            rgb = (.46, .46, .46)
                        elif d < glow:
                            rgb = mix(WHITE, body, (d - rim) / (glow - rim) * .7 + .3)
                        else:
                            rgb = body
                        acc[0] += rgb[0]; acc[1] += rgb[1]; acc[2] += rgb[2]  # noqa: E702
                if n == 0:
                    rgb = (.46, .46, .46)
                else:
                    rgb = (acc[0] / n, acc[1] / n, acc[2] / n)
                sheet.over(ox + x, y, rgb, a)
    return sheet


def _distance_inside(inside: list[bool], width: int, height: int) -> list[float]:
    """Chamfer (3-4) distance from each inside pixel to the nearest outside
    one, in pixels. Good to a few percent, which is all a rim needs."""
    big = 10 ** 9
    d = [0 if not v else big for v in inside]
    for y in range(height):
        for x in range(width):
            i = y * width + x
            if d[i] == 0:
                continue
            best = d[i]
            if x > 0:
                best = min(best, d[i - 1] + 3)
            if y > 0:
                best = min(best, d[i - width] + 3)
                if x > 0:
                    best = min(best, d[i - width - 1] + 4)
                if x < width - 1:
                    best = min(best, d[i - width + 1] + 4)
            if x == 0 or y == 0 or x == width - 1 or y == height - 1:
                best = min(best, 3)
            d[i] = best
    for y in range(height - 1, -1, -1):
        for x in range(width - 1, -1, -1):
            i = y * width + x
            if d[i] == 0:
                continue
            best = d[i]
            if x < width - 1:
                best = min(best, d[i + 1] + 3)
            if y < height - 1:
                best = min(best, d[i + width] + 3)
                if x < width - 1:
                    best = min(best, d[i + width + 1] + 4)
                if x > 0:
                    best = min(best, d[i + width - 1] + 4)
            d[i] = best
    return [v / 3.0 for v in d]


# ----------------------------------------------------------------- yield icons

def draw_yield_icons(icons: dict):
    """One finished circular sign per yield: a disc lit from the upper left,
    a darker rim, and the pictograph with a soft shadow under it."""
    cell = YIELD_CELL
    sheet = Image(cell * len(YIELDS), cell)
    radius = cell / 2 - .5
    for index, (_, disc, ink, name) in enumerate(YIELDS):
        ox = index * cell
        base, ink_rgb = hex_rgb(disc), hex_rgb(ink)
        light, dark, edge = mix(base, WHITE, .38), mix(base, BLACK, .28), mix(base, BLACK, .5)
        cover = coverage([_circle(cell / 2, cell / 2, radius, 160)], cell, cell)
        inner = coverage([_circle(cell / 2, cell / 2, radius - 3.2, 160)], cell, cell)

        def shade(x, y, light=light, dark=dark):
            t = ((x + .5 - cell * .25) + (y + .5 - cell * .2)) / (cell * 1.3)
            return mix(light, dark, min(1.0, max(0.0, t)))

        sheet.paint(cover, ox, 0, cell, cell, edge)
        sheet.paint(inner, ox, 0, cell, cell, shade)
        size = cell * .62
        box = (cell / 2 - size / 2, cell / 2 - size / 2 + .5, size, size)
        shadow = coverage(icon_polygons(name, icons, (box[0] + .9, box[1] + 1.6, size, size)), cell, cell)
        sheet.paint([c * .38 for c in shadow], ox, 0, cell, cell, BLACK)
        glyph = coverage(icon_polygons(name, icons, box), cell, cell)
        sheet.paint(glyph, ox, 0, cell, cell, ink_rgb)
    return sheet


# ----------------------------------------------------------------- city walls

def draw_city_wall_mark():
    """A battlement: three merlons on a wall, light grey, on a dark halo."""
    width, height = WALL_SIZE
    sheet = Image(width, height)
    wall = [(4.0, 7.2), (17.0, 7.2), (17.0, 15.2), (4.0, 15.2)]
    merlons = [[(x0, 2.8), (x0 + 2.8, 2.8), (x0 + 2.8, 7.4), (x0, 7.4)] for x0 in (4.0, 9.1, 14.2)]
    gate = _circle(10.5, 12.4, 1.9, 24, math.pi, 2 * math.pi) + [(12.4, 15.3), (8.6, 15.3)]
    body = [wall] + merlons

    def grown(poly, by):
        cx = sum(p[0] for p in poly) / len(poly)
        cy = sum(p[1] for p in poly) / len(poly)
        out = []
        for x, y in poly:
            out.append((x + (by if x > cx else -by), y + (by if y > cy else -by)))
        return out

    halo = coverage([grown(p, 1.5) for p in body], width, height)
    sheet.paint([c * .55 for c in halo], 0, 0, width, height, BLACK)
    fill = coverage(body, width, height)
    sheet.paint(fill, 0, 0, width, height,
                lambda x, y: mix((.97, .97, .97), (.72, .72, .72), min(1.0, max(0.0, (y - 3) / 12))))
    door = coverage([gate], width, height)
    sheet.paint([c * .9 for c in door], 0, 0, width, height, (.25, .25, .25))
    return sheet


# ----------------------------------------------------------------- outputs

def load_icons() -> dict:
    return json.loads(ICON_DATA.read_text(encoding="utf-8"))["icons"]


def fnv1a64(data: bytes) -> str:
    h = 0xcbf29ce484222325
    for byte in data:
        h = ((h ^ byte) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return f"{h:#018x}"


def build() -> dict[str, bytes]:
    """Every output file's bytes, keyed by its path under web/assets."""
    icons = load_icons()
    glyphs, rows, index_rows = draw_unit_glyphs(icons)
    glyph_png = glyphs.png(PROVENANCE)
    plates_png = draw_unit_plates().png(PROVENANCE)
    yields_png = draw_yield_icons(icons).png(PROVENANCE)
    wall_png = draw_city_wall_mark().png(PROVENANCE)
    index = {
        "description": "Unit glyphs of the CIVVIS command map: one white silhouette per unit",
        "source": f"game-icons.net icons ({ICONS_REPOSITORY} at {ICONS_COMMIT}), drawn by tools/own_art.py",
        "licence": f"{ICONS_LICENCE} ({ICONS_LICENCE_URL}); authors per unit below and in ART_CREDITS.md",
        "cell_size": GLYPH_CELL,
        "columns": GLYPH_COLUMNS,
        "rows": rows,
        "png_bytes": len(glyph_png),
        "png_fnv1a64": fnv1a64(glyph_png),
        "units": index_rows,
    }
    files = {
        "unit-glyphs.png": glyph_png,
        "unit-glyphs.json": (json.dumps(index, indent=2, sort_keys=True) + "\n").encode(),
        "unit-plates.png": plates_png,
        "yield-icons.png": yields_png,
        "city-wall-mark.png": wall_png,
    }
    files["ART_CREDITS.md"] = credits(index_rows).encode()
    files["art-manifest.json"] = manifest(files).encode()
    return files


def credits(index_rows) -> str:
    icons_used: dict[str, list[str]] = {}
    for row in index_rows:
        icons_used.setdefault(row["icon"], []).append(f"unit `{row['type']}`")
    for kind, _, _, name in YIELDS:
        icons_used.setdefault(name, []).append(f"yield `{kind}`")
    lines = [
        "# Art credits",
        "",
        "Every image the CIVVIS viewer ships, and where it came from. None of it",
        "is taken from Civilization VI or any other game. This file is written by",
        "`tools/own_art.py`; `tools/art_provenance.py` refuses any image in the",
        "repository or in a published build that `art-manifest.json` beside it",
        "does not list with its hash.",
        "",
        "| file | made by | licence |",
        "|---|---|---|",
        "| `unit-plates.png` | drawn by `tools/own_art.py` (CIVVIS's own shapes) | the repository's own |",
        "| `city-wall-mark.png` | drawn by `tools/own_art.py` | the repository's own |",
        "| `yield-icons.png` | discs drawn by `tools/own_art.py`; pictographs from game-icons.net, below | "
        f"[{ICONS_LICENCE}]({ICONS_LICENCE_URL}) for the pictographs |",
        "| `unit-glyphs.png` | game-icons.net silhouettes, below, rasterised by `tools/own_art.py` | "
        f"[{ICONS_LICENCE}]({ICONS_LICENCE_URL}) |",
        "| `hidden-map-monsters.png` | added in #391, redrawn in #947, #953, #2222; how it was made is not "
        "recorded (THIRD_PARTY_NOTICES.md, Unresolved); not Civilization VI's art | unresolved |",
        "",
        "## Icons from game-icons.net",
        "",
        f"Icons made by the authors named below. Available on https://game-icons.net "
        f"under [{ICONS_LICENCE}]({ICONS_LICENCE_URL}), taken unmodified in shape from "
        f"{ICONS_REPOSITORY} at `{ICONS_COMMIT[:12]}`; CIVVIS only rasterises and tints them.",
        "",
        "| icon | author | licence | used for |",
        "|---|---|---|---|",
    ]
    for name in sorted(icons_used):
        author = AUTHORS[name.split("/")[0]]
        lines.append(f"| [{name.split('/')[1]}](https://game-icons.net/1x1/{name}.html) | {author} | "
                     f"{ICONS_LICENCE} | {', '.join(icons_used[name])} |")
    return "\n".join(lines) + "\n"


def manifest(files: dict[str, bytes]) -> str:
    """Every image the viewer may ship, with its hash and maker."""
    entries = {
        name: {"sha256": hashlib.sha256(data).hexdigest(), "made_by": "tools/own_art.py",
               "source": ("game-icons.net, CC BY 3.0 (ART_CREDITS.md)" if name in ("unit-glyphs.png", "yield-icons.png")
                          else "drawn by tools/own_art.py")}
        for name, data in sorted(files.items()) if name.endswith(".png")
    }
    monsters = ASSETS / "hidden-map-monsters.png"
    entries["hidden-map-monsters.png"] = {
        "sha256": hashlib.sha256(monsters.read_bytes()).hexdigest(),
        "made_by": "added in #391, redrawn in #947, #953 and #2222",
        "source": "origin not recorded (THIRD_PARTY_NOTICES.md, Unresolved); not Civilization VI's art",
    }
    body = {
        "_doc": ("Every image web/assets may ship, by sha256. tools/art_provenance.py refuses a tracked "
                 "image or a published one that is not listed here with these bytes, and any image "
                 "carrying the cutters' local-only marker. Written by tools/own_art.py."),
        "files": dict(sorted(entries.items())),
    }
    return json.dumps(body, indent=2) + "\n"


def fetch() -> None:
    """Refresh tools/own_art_icons.json from game-icons at ICONS_COMMIT."""
    wanted = sorted(set(UNIT_ICONS.values()) | {name for *_, name in YIELDS})
    icons = {}
    for name in wanted:
        url = f"https://raw.githubusercontent.com/game-icons/icons/{ICONS_COMMIT}/{name}.svg"
        with urllib.request.urlopen(url, timeout=120) as response:
            svg = response.read().decode("utf-8")
        icons[name] = parse_icon_svg(name, svg)
        print(f"  {name}")
    write_icon_data(icons)


def parse_icon_svg(name: str, svg: str) -> dict:
    """The white paths of a game-icons SVG; the black backdrop square is not art."""
    paths = re.findall(r'<path fill="#fff" d="([^"]+)"', svg)
    if not paths:
        raise SystemExit(f"{name}: no white path in the SVG")
    return {"author": AUTHORS[name.split("/")[0]], "licence": ICONS_LICENCE,
            "url": f"https://game-icons.net/1x1/{name}.html", "paths": paths}


def write_icon_data(icons: dict) -> None:
    body = {
        "_doc": ("Path data of the game-icons.net icons tools/own_art.py draws, unmodified, "
                 f"from {ICONS_REPOSITORY} at {ICONS_COMMIT}. Each icon is {ICONS_LICENCE} "
                 f"({ICONS_LICENCE_URL}) by the author named on it."),
        "repository": ICONS_REPOSITORY,
        "commit": ICONS_COMMIT,
        "icons": dict(sorted(icons.items())),
    }
    ICON_DATA.write_text(json.dumps(body, indent=1) + "\n", encoding="utf-8")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true", help="fail if a committed file differs")
    parser.add_argument("--fetch", action="store_true", help="refresh the icon path data (network)")
    args = parser.parse_args(argv)
    if args.fetch:
        fetch()
    files = build()
    stale = []
    for name, data in files.items():
        path = ASSETS / name
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                stale.append(name)
        else:
            path.write_bytes(data)
            print(f"wrote web/assets/{name} ({len(data):,} bytes)")
    if stale:
        print("redraw differs from the committed file: " + ", ".join(stale), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
