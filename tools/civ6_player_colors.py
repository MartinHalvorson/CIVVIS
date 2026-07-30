#!/usr/bin/env python3
"""Ground CIVVIS empire colours in Civilization VI's own player-colour tables.

Civ 6 dresses an empire in a *jersey*: a primary colour that fills its
territory and a secondary that trims it, plus three alternate pairs the game
falls back to when two players in one lobby would otherwise wear the same
primary. CIVVIS draws its frontiers the same way, so the colours should be the
same colours -- an empire that is purple-on-gold in Civ 6 has no business being
pastel yellow here.

Two facts about the shipped data shape everything below.

**Civ 6 keys a jersey by leader, not by civilization.** ``PlayerColors`` has 79
``Usage=Unique`` rows and every one of them is a ``LEADER_*`` type; there is not
a single ``CIVILIZATION_*`` major row. Rome is purple-on-gold because *Trajan*
is, and Rome under Julius Caesar is dark-red-on-gold instead. CIVVIS seats
civilizations rather than leaders, so each civilization inherits the jersey of
its flagship leader -- Trajan over Caesar, Cleopatra over Ramses, Victoria over
Eleanor, Qin over Wu Zetian -- named explicitly in ``FLAGSHIP_LEADER`` because
file load order cannot be trusted to pick it (see that table).

**Colours are a two-level indirection with an optional alpha override.** A
``PlayerColors`` row names a ``COLOR_*`` type; ``Colors.xml`` maps that to
another ``COLOR_*`` type or to ``r,g,b,a``; and either level may append an alpha
(``COLOR_STANDARD_RED_MD,166``). Only ``PlayerStandardColors.xml`` holds literal
channel values. Resolving one level, or splitting on comma before checking for a
``COLOR`` prefix, silently yields black.

Usage::

    python3 tools/civ6_player_colors.py                 # report, with coverage
    python3 tools/civ6_player_colors.py --json          # write data/civ6_player_colors.json
    python3 tools/civ6_player_colors.py --js            # print the generated JS region
    python3 tools/civ6_player_colors.py --write --json  # patch web/index.html and the JSON

Requires a Civ 6 install (see ``tools/civ6_env.py``). The generated JSON is
checked in so CI and machines without the game can still verify the table --
``tests/civ6_jerseys.rs`` asserts web/index.html agrees with it.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env  # noqa: E402

REPO = Path(__file__).resolve().parent.parent

# --------------------------------------------------------------------------
# CIVVIS civilization -> Civ 6 civilization type.
#
# Only the civilizations Civ 6 actually ships. The other 55 in data/civs.json
# have no counterpart to copy, so they are dressed from the same palette
# instead -- see PALETTE_JERSEYS.
# --------------------------------------------------------------------------
CIV6_CIVILIZATION = {
    "America": "CIVILIZATION_AMERICA",
    "Arabia": "CIVILIZATION_ARABIA",
    "Australia": "CIVILIZATION_AUSTRALIA",
    "Aztec": "CIVILIZATION_AZTEC",
    "Babylon": "CIVILIZATION_BABYLON_STK",
    "Brazil": "CIVILIZATION_BRAZIL",
    "Byzantium": "CIVILIZATION_BYZANTIUM",
    "Canada": "CIVILIZATION_CANADA",
    "China": "CIVILIZATION_CHINA",
    "Cree": "CIVILIZATION_CREE",
    "Egypt": "CIVILIZATION_EGYPT",
    "England": "CIVILIZATION_ENGLAND",
    "Ethiopia": "CIVILIZATION_ETHIOPIA",
    "France": "CIVILIZATION_FRANCE",
    "Gaul": "CIVILIZATION_GAUL",
    "Georgia": "CIVILIZATION_GEORGIA",
    "Germany": "CIVILIZATION_GERMANY",
    "Gran Colombia": "CIVILIZATION_GRAN_COLOMBIA",
    "Greece": "CIVILIZATION_GREECE",
    "Hungary": "CIVILIZATION_HUNGARY",
    "Inca": "CIVILIZATION_INCA",
    "India": "CIVILIZATION_INDIA",
    "Indonesia": "CIVILIZATION_INDONESIA",
    "Japan": "CIVILIZATION_JAPAN",
    "Khmer": "CIVILIZATION_KHMER",
    "Kongo": "CIVILIZATION_KONGO",
    "Korea": "CIVILIZATION_KOREA",
    "Macedon": "CIVILIZATION_MACEDON",
    "Mali": "CIVILIZATION_MALI",
    "Maori": "CIVILIZATION_MAORI",
    "Mapuche": "CIVILIZATION_MAPUCHE",
    "Maya": "CIVILIZATION_MAYA",
    "Mongolia": "CIVILIZATION_MONGOLIA",
    "Netherlands": "CIVILIZATION_NETHERLANDS",
    "Norway": "CIVILIZATION_NORWAY",
    "Nubia": "CIVILIZATION_NUBIA",
    "Ottomans": "CIVILIZATION_OTTOMAN",
    "Persia": "CIVILIZATION_PERSIA",
    "Phoenicia": "CIVILIZATION_PHOENICIA",
    "Poland": "CIVILIZATION_POLAND",
    "Portugal": "CIVILIZATION_PORTUGAL",
    "Rome": "CIVILIZATION_ROME",
    "Russia": "CIVILIZATION_RUSSIA",
    "Scotland": "CIVILIZATION_SCOTLAND",
    "Scythia": "CIVILIZATION_SCYTHIA",
    "Spain": "CIVILIZATION_SPAIN",
    "Sumeria": "CIVILIZATION_SUMERIA",
    "Sweden": "CIVILIZATION_SWEDEN",
    "Vietnam": "CIVILIZATION_VIETNAM",
    "Zulu": "CIVILIZATION_ZULU",
}

# --------------------------------------------------------------------------
# Which leader's jersey a civilization wears, for the 20 that have more than
# one. File load order gets most of these right on its own, but it cannot be
# trusted: the DLC packs load in an order this tool has no way to read, so
# Persia came out as Nader Shah (a 2023 pack) rather than Cyrus, and Greece --
# which has *two* base-game leaders in the same file -- came out as Gorgo's
# Spartan dark red rather than the blue-and-white that reads as Greece.
#
# So the choice is written down. `build` refuses to guess: any multi-leader
# civilization missing from this table is reported rather than resolved.
# --------------------------------------------------------------------------
FLAGSHIP_LEADER = {
    "CIVILIZATION_AMERICA": "LEADER_T_ROOSEVELT",
    "CIVILIZATION_ARABIA": "LEADER_SALADIN",
    "CIVILIZATION_BYZANTIUM": "LEADER_BASIL",
    "CIVILIZATION_CHINA": "LEADER_QIN",
    "CIVILIZATION_EGYPT": "LEADER_CLEOPATRA",
    "CIVILIZATION_ENGLAND": "LEADER_VICTORIA",
    "CIVILIZATION_FRANCE": "LEADER_CATHERINE_DE_MEDICI",
    "CIVILIZATION_GERMANY": "LEADER_BARBAROSSA",
    "CIVILIZATION_GREECE": "LEADER_PERICLES",
    "CIVILIZATION_INDIA": "LEADER_GANDHI",
    "CIVILIZATION_JAPAN": "LEADER_HOJO",
    "CIVILIZATION_KONGO": "LEADER_MVEMBA",
    "CIVILIZATION_KOREA": "LEADER_SEONDEOK",
    "CIVILIZATION_MALI": "LEADER_MANSA_MUSA",
    "CIVILIZATION_MONGOLIA": "LEADER_GENGHIS_KHAN",
    "CIVILIZATION_NORWAY": "LEADER_HARDRADA",
    "CIVILIZATION_OTTOMAN": "LEADER_SULEIMAN",
    "CIVILIZATION_PERSIA": "LEADER_CYRUS",
    "CIVILIZATION_ROME": "LEADER_TRAJAN",
    "CIVILIZATION_VIETNAM": "LEADER_LADY_TRIEU",
}

# --------------------------------------------------------------------------
# The civilizations Civ 6 never shipped, dressed from Civ 6's own 28-colour
# player palette (PlayerStandardColors.xml). Named by palette token rather than
# hex so the values stay the game's values -- if Firaxis re-tints the palette in
# a patch, re-running this tool moves these with it.
#
# Picked from the civilization's own heraldry where it has one (Denmark's
# Dannebrog red, Ireland's green-and-orange, Prussia's black-and-silver) and for
# hue separation from its neighbours on the Earth map otherwise, because two
# adjacent empires are the pair a reader actually has to tell apart.
# --------------------------------------------------------------------------
PALETTE_JERSEYS = {
    # Europe
    "Denmark": ("RED_DK", "WHITE_LT"),
    "Austria": ("RED_MD", "WHITE_MD2"),
    "Bohemia": ("WHITE_MD2", "RED_MD"),
    "Ireland": ("GREEN_MD", "ORANGE_MD"),
    "Switzerland": ("RED_MD", "WHITE_DK"),
    "Venice": ("RED_DK", "YELLOW_MD"),
    "Serbia": ("BLUE_DK", "RED_LT"),
    "Bulgaria": ("GREEN_MD", "WHITE_LT"),
    "Lithuania": ("YELLOW_DK", "RED_MD"),
    "Ukraine": ("BLUE_MD", "YELLOW_MD"),
    "Finland": ("BLUE_MD", "WHITE_LT"),
    "Romania": ("BLUE_MD", "RED_MD"),
    "Novgorod": ("AQUA_DK", "YELLOW_MD"),
    "Prussia": ("WHITE_DK", "WHITE_MD2"),
    "Catalonia": ("YELLOW_LT", "RED_MD"),
    # Near East and Central Asia
    "Assyria": ("ORANGE_DK", "AQUA_LT"),
    "Media": ("PURPLE_MD", "AQUA_LT"),
    "Lydia": ("YELLOW_LT", "RED_DK"),
    "Parthia": ("AQUA_DK", "ORANGE_LT"),
    "Sogdiana": ("AQUA_LT", "PURPLE_DK"),
    "Israel": ("WHITE_LT", "BLUE_LT"),
    "Armenia": ("RED_DK", "ORANGE_LT"),
    "Timurids": ("AQUA_MD", "WHITE_DK"),
    "Kazakh": ("AQUA_MD", "YELLOW_MD"),
    "Bactria": ("GREEN_DK", "ORANGE_LT"),
    "Manchuria": ("YELLOW_DK", "WHITE_DK"),
    # Africa
    "Axum": ("GREEN_DK", "YELLOW_LT"),
    "Morocco": ("RED_DK", "GREEN_MD"),
    "Numidia": ("ORANGE_DK", "YELLOW_LT"),
    "Songhai": ("YELLOW_DK", "AQUA_LT"),
    "Ghana": ("YELLOW_MD", "RED_DK"),
    "Benin": ("MAGENTA_DK", "ORANGE_LT"),
    "Ashanti": ("YELLOW_MD", "ORANGE_DK"),
    "Swahili": ("AQUA_MD", "RED_DK"),
    "Great Zimbabwe": ("GREEN_LT", "WHITE_DK"),
    "Buganda": ("MAGENTA_LT", "GREEN_DK"),
    "Oyo": ("PURPLE_LT", "YELLOW_DK"),
    "Tuareg": ("BLUE_LT", "ORANGE_DK"),
    "Madagascar": ("MAGENTA_MD", "GREEN_DK"),
    # South and South-East Asia
    "Gujarat": ("ORANGE_MD", "AQUA_MD"),
    "Tibet": ("RED_MD", "YELLOW_LT"),
    "Nepal": ("RED_MD", "BLUE_DK"),
    "Kalinga": ("ORANGE_LT", "PURPLE_DK"),
    "Chola": ("RED_LT", "YELLOW_DK"),
    "Bengal": ("GREEN_DK", "RED_MD"),
    "Maratha": ("ORANGE_MD", "PURPLE_DK"),
    "Siam": ("RED_MD", "BLUE_MD"),
    "Burma": ("YELLOW_MD", "GREEN_MD"),
    "Majapahit": ("RED_LT", "WHITE_LT"),
    "Champa": ("MAGENTA_DK", "YELLOW_LT"),
    # The Americas
    "Pueblo": ("ORANGE_LT", "AQUA_DK"),
    "Comanche": ("RED_LT", "WHITE_DK"),
    "Sioux": ("PURPLE_DK", "WHITE_MD2"),
    "Muisca": ("YELLOW_LT", "GREEN_DK"),
    "Argentina": ("BLUE_LT", "YELLOW_LT"),
}

# Civ 6 builds an alternate jersey by moving within the pair it already has --
# LEADER_CLEOPATRA_ALT is Cleopatra swapped, LEADER_QIN_ALT is Qin swapped -- so
# the alternates for a palette jersey are generated the same way: swap the pair,
# then re-shade each half within its own hue family. Every alternate is a real
# palette value with a primary distinct from the canonical one, which is all the
# runtime collision fallback needs.
SHADE_CYCLE = {"LT": "DK", "MD": "DK", "MD2": "DK", "DK": "LT"}


def field(row, name):
    """One column of a ``<Row>``, whichever of the two shipped forms it uses.

    The colour tables spell a column as a child element
    (``<Row><Type>…</Type></Row>``) and the gameplay tables spell it as an
    attribute (``<Row CivilizationType="…"/>``). Reading only children finds
    every colour and no leader; reading only attributes does the reverse. Both
    forms appear inside the same install, sometimes in the same pack.
    """
    value = row.get(name)
    if value is None:
        value = row.findtext(name)
    return value.strip() if value else None


def parse_colors(assets: Path):
    """Return (colors, player, provenance) from every shipped colour table."""
    files = []
    base = assets / "Base/Assets/UI/Colors"
    for name in ("PlayerStandardColors.xml", "Colors.xml", "PlayerColors.xml"):
        if (base / name).exists():
            files.append(base / name)
    dlc = assets / "DLC"
    packs = sorted(p.name for p in dlc.iterdir() if p.is_dir()) if dlc.is_dir() else []
    ordered = [p for p in ("Expansion1", "Expansion2") if p in packs]
    ordered += [p for p in packs if p not in ordered]
    for pack in ordered:
        # A scenario pack re-skins players for one scripted setup; letting it
        # overwrite the standard palette is how Lisbon ends up called Venice.
        if "Scenario" in pack:
            continue
        data = dlc / pack / "Data"
        if data.is_dir():
            files += [f for f in sorted(data.iterdir())
                      if f.suffix == ".xml" and re.search("colors", f.name, re.I)]

    colors, player, provenance = {}, {}, {}
    for path in files:
        try:
            root = ET.parse(path).getroot()
        except ET.ParseError as exc:
            print(f"warning: {path.name}: {exc}", file=sys.stderr)
            continue
        for table in root.iter("Colors"):
            for row in table:
                kind, value = field(row, "Type"), field(row, "Color")
                if kind and value:
                    colors[kind] = value
        for table in root.iter("PlayerColors"):
            for row in table:
                kind = field(row, "Type")
                if not kind:
                    continue
                rec = player.setdefault(kind, {})
                for column in ("Usage", "PrimaryColor", "SecondaryColor",
                               "Alt1PrimaryColor", "Alt1SecondaryColor",
                               "Alt2PrimaryColor", "Alt2SecondaryColor",
                               "Alt3PrimaryColor", "Alt3SecondaryColor"):
                    value = field(row, column)
                    if value:
                        rec[column] = value
                provenance.setdefault(kind, path)
    return colors, player, provenance


def make_resolver(colors):
    def resolve(ref, depth=0):
        """A ``COLOR_*`` type or ``r,g,b[,a]`` literal -> ``#rrggbb``."""
        if ref is None or depth > 8:
            return None
        parts = [p.strip() for p in ref.strip().split(",")]
        if parts[0].startswith("COLOR"):
            # The alpha suffix rides on the reference, not on the definition,
            # so it must be read before recursing and then discarded -- the
            # jersey is an opaque colour wherever CIVVIS paints it.
            return resolve(colors.get(parts[0]), depth + 1)
        try:
            channels = [int(float(p)) for p in parts if p]
        except ValueError:
            return None
        if len(channels) < 3:
            return None
        return "#%02x%02x%02x" % tuple(max(0, min(255, c)) for c in channels[:3])
    return resolve


def base_leader(civ_type, leaders_of, provenance):
    """The leader whose jersey the civilization wears, or a complaint.

    Returns ``(leader, None)`` or ``(None, reason)``. A civilization with
    exactly one leader that has a colour row needs no judgement; anything else
    must be named in FLAGSHIP_LEADER.
    """
    named = FLAGSHIP_LEADER.get(civ_type)
    if named:
        if named in provenance:
            return named, None
        return None, f"FLAGSHIP_LEADER names {named}, which has no colour row"
    candidates = sorted({l for l in leaders_of.get(civ_type, []) if l in provenance})
    if not candidates:
        return None, "no leader of this civilization has a colour row"
    if len(candidates) > 1:
        return None, ("needs a FLAGSHIP_LEADER entry; candidates "
                      + ", ".join(candidates))
    return candidates[0], None


def leader_map(assets):
    """civilization type -> [leader types], from CivilizationLeaders XML.

    The gameplay tables nest one level deeper than the colour tables --
    ``<Database><GameInfo><CivilizationLeaders>`` against
    ``<Database><PlayerColors>`` -- so this iterates the whole tree by tag name
    rather than looking at the root's direct children. Walking only the root's
    children finds every colour and not one leader, which reads as a game that
    ships no leaders at all.
    """
    out = {}
    roots = [assets / "Base/Assets/Gameplay/Data"]
    dlc = assets / "DLC"
    if dlc.is_dir():
        roots += [p / "Data" for p in sorted(dlc.iterdir())
                  if p.is_dir() and "Scenario" not in p.name]
    for folder in roots:
        if not folder.is_dir():
            continue
        for path in sorted(folder.glob("*.xml")):
            try:
                root = ET.parse(path).getroot()
            except (ET.ParseError, OSError):
                continue
            for table in root.iter("CivilizationLeaders"):
                for row in table:
                    civ = field(row, "CivilizationType")
                    leader = field(row, "LeaderType")
                    if civ and leader:
                        out.setdefault(civ, []).append(leader)
    return out


def build(assets: Path):
    colors, player, provenance = parse_colors(assets)
    resolve = make_resolver(colors)
    leaders_of = leader_map(assets)

    palette = {name[len("COLOR_STANDARD_"):]: resolve(value)
               for name, value in colors.items()
               if name.startswith("COLOR_STANDARD_")}

    def shade(token):
        stem, _, level = token.rpartition("_")
        return f"{stem}_{SHADE_CYCLE.get(level, level)}" if stem else token

    jerseys, report, complaints = {}, [], []
    for civ, civ_type in CIV6_CIVILIZATION.items():
        leader, reason = base_leader(civ_type, leaders_of, provenance)
        if leader is None:
            complaints.append(f"{civ} ({civ_type}): {reason}")
            report.append((civ, civ_type, f"UNRESOLVED -- {reason}"))
            continue
        row = player[leader]
        pairs = []
        for prefix in ("", "Alt1", "Alt2", "Alt3"):
            primary = resolve(row.get(f"{prefix}PrimaryColor"))
            secondary = resolve(row.get(f"{prefix}SecondaryColor"))
            if primary and secondary:
                pairs.append([primary, secondary])
        jerseys[civ] = {"source": "civ6", "leader": leader,
                        "civilization": civ_type, "pairs": pairs}
        report.append((civ, leader, " ".join(p[0] + "/" + p[1] for p in pairs)))

    for civ, (primary_token, secondary_token) in PALETTE_JERSEYS.items():
        tokens = [(primary_token, secondary_token),
                  (secondary_token, primary_token),
                  (shade(primary_token), secondary_token),
                  (shade(secondary_token), primary_token)]
        pairs, seen = [], set()
        for primary_token_, secondary_token_ in tokens:
            primary, secondary = palette.get(primary_token_), palette.get(secondary_token_)
            if not primary or not secondary or primary == secondary or primary in seen:
                continue
            seen.add(primary)
            pairs.append([primary, secondary])
        jerseys[civ] = {"source": "palette",
                        "tokens": [primary_token, secondary_token], "pairs": pairs}
        report.append((civ, f"{primary_token}/{secondary_token}",
                       " ".join(p[0] + "/" + p[1] for p in pairs)))

    # Only 27 of the palette's 28 colours ever serve as a primary, so with 105
    # civilizations a shared *primary* is structural and the renderer's
    # collision fallback exists to absorb it -- Civ 6 has the same problem and
    # puts Saladin and Menelik in the identical gold-on-green. An exact
    # duplicate *pair* is different: it costs the later civilization its
    # intended jersey every time the two are seated together, for nothing. The
    # 50 copied from the game are beyond reach, so this only holds the 55
    # authored picks to the standard.
    canonical = {}
    for civ, entry in jerseys.items():
        if entry["pairs"]:
            canonical.setdefault(tuple(entry["pairs"][0]), []).append(civ)
    for pair, wearers in sorted(canonical.items()):
        if len(wearers) < 2:
            continue
        authored = [c for c in wearers if jerseys[c]["source"] == "palette"]
        if authored and len(wearers) > len(authored) or len(authored) > 1:
            complaints.append(
                f"{'/'.join(pair)} is the canonical jersey of "
                + ", ".join(f"{c} [{jerseys[c]['source']}]" for c in sorted(wearers))
                + " -- retune the PALETTE_JERSEYS entry"
            )

    generic = generic_pairs(player, resolve)
    return jerseys, palette, generic, report, complaints


def channels(hex_color):
    return tuple(int(hex_color[i:i + 2], 16) / 255 for i in (1, 3, 5))


def hue(hex_color):
    r, g, b = channels(hex_color)
    high, low = max(r, g, b), min(r, g, b)
    if high == low:
        return 360.0  # the greys have no hue; park them past every one that does
    span = high - low
    if high == r:
        return (60 * ((g - b) / span) + 360) % 360
    if high == g:
        return 60 * ((b - r) / span) + 120
    return 60 * ((r - g) / span) + 240


def luminance(hex_color):
    r, g, b = channels(hex_color)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def generic_pairs(player, resolve):
    """Civ 6's generic ``Usage=Major`` jerseys, one per primary, spread by hue.

    The last resort for a civilization with no entry of its own -- a custom or
    future one. Two corrections to the raw table are needed to make it usable as
    a by-player-id fallback:

    * **One row per primary.** The 43 shipped rows draw on only 25 distinct
      primaries -- ``PLAYERCOLOR_GREEN`` and ``PLAYERCOLOR_MIDDLE_GREEN`` are
      both ``GREEN_MD``, and three separate rows are ``GREEN_DK`` -- so indexing
      the raw list by id hands different seats the same territory colour, which
      is the one thing the fallback has to avoid. Where rows collide, keep the
      partner furthest from the primary in luminance: that pair is the one whose
      border still reads when it is drawn over terrain sharing its hue.
    * **Hue-strided order.** Ids are assigned in sequence, so a hue-sorted list
      gives seats 0 and 1 two neighbouring reds. Walking the sorted list with a
      step coprime to its length keeps consecutive ids far apart in hue and
      still uses every colour exactly once.
    """
    best = {}
    for kind, rec in sorted(player.items()):
        if rec.get("Usage") != "Major":
            continue
        primary = resolve(rec.get("PrimaryColor"))
        secondary = resolve(rec.get("SecondaryColor"))
        if not primary or not secondary or primary == secondary:
            continue
        contrast = abs(luminance(primary) - luminance(secondary))
        if primary not in best or contrast > best[primary][0]:
            best[primary] = (contrast, secondary, kind)
    rows = [(primary, secondary, kind)
            for primary, (_, secondary, kind) in best.items()]
    if not rows:
        return []
    rows.sort(key=lambda row: (hue(row[0]), luminance(row[0]), row[0]))
    step = next((s for s in (7, 5, 3, 2, 1) if math.gcd(s, len(rows)) == 1), 1)
    return [rows[(i * step) % len(rows)] for i in range(len(rows))]


def roster():
    return list(json.loads((REPO / "data/civs.json").read_text()).keys())


BEGIN = "// >>> generated by tools/civ6_player_colors.py -- do not edit by hand"
END = "// <<< generated by tools/civ6_player_colors.py"


def wrap(values, indent, width=96):
    """Break a JS array literal across lines at the given indent."""
    lines, current = [], ""
    for index, value in enumerate(values):
        piece = value + ("," if index < len(values) - 1 else "")
        if current and len(indent) + len(current) + 1 + len(piece) > width:
            lines.append(indent + current)
            current = piece
        else:
            current = f"{current} {piece}".strip()
    if current:
        lines.append(indent + current)
    return "\n".join(lines).lstrip()


def js_region(jerseys, names, generic):
    out = [
        BEGIN,
        "// Civ 6 dresses an empire in a jersey: a primary that fills its territory and a",
        "// secondary that trims it, plus three alternates for when two players in one lobby",
        "// would wear the same primary. These are those colours, read out of the game's own",
        "// PlayerColors tables -- 50 civilizations copy their base leader exactly, and the 55",
        "// Civ 6 never shipped are dressed from the same 28-colour palette. Regenerate with",
        "//   python3 tools/civ6_player_colors.py --write --json",
        "// and keep data/civ6_player_colors.json in step; tests/civ6_jerseys.rs compares them.",
        "",
        "// The last resort, for a civilization with no entry below: Civ 6's own generic",
        "// Usage=Major jerseys, ordered so consecutive player ids land far apart in hue.",
        "const PCOLORS = [" + wrap([f'"{p}"' for p, _, _ in generic], " " * 17) + "];",
        "// Indexed in PCOLORS order -- a jersey is the pair, so these must stay aligned.",
        "const PCOLORS2 = [" + wrap([f'"{s}"' for _, s, _ in generic], " " * 18) + "];",
        "const CIV6_JERSEYS = {",
    ]
    for civ in names:
        entry = jerseys[civ]
        key = civ if re.fullmatch(r"[A-Za-z_$][\w$]*", civ) else f'"{civ}"'
        note = (f"leader {entry['leader'][len('LEADER_'):].title().replace('_', ' ')}"
                if entry["source"] == "civ6"
                else "palette " + "/".join(entry["tokens"]).lower())
        pairs = ", ".join(f'["{a}","{b}"]' for a, b in entry["pairs"])
        out.append(f"  {key}: [{pairs}], // {note}")
    out += ["};", END]
    return "\n".join(out)


def patch_index(region):
    path = REPO / "web/index.html"
    text = path.read_text()
    if BEGIN in text:
        start = text.index(BEGIN)
        stop = text.index(END, start) + len(END)
    else:
        # First run: swap out the hand-maintained tables, PCOLORS through the
        # close of CIV6_JERSEYS, and leave the markers behind for next time.
        start = text.index("const PCOLORS = [")
        stop = text.index("\n};", text.index("const CIV6_JERSEYS = {")) + len("\n};")
    path.write_text(text[:start] + region + text[stop:])
    return path


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--civ6", help="Civ 6 install root or Assets directory")
    ap.add_argument("--json", action="store_true", help="write data/civ6_player_colors.json")
    ap.add_argument("--js", action="store_true", help="print the generated JS region")
    ap.add_argument("--write", action="store_true", help="patch web/index.html in place")
    args = ap.parse_args()

    assets = civ6_env.assets_dir(args.civ6)
    jerseys, palette, generic, report, complaints = build(assets)
    names = roster()

    missing = [c for c in names if c not in jerseys]
    extra = [c for c in jerseys if c not in names]

    # Emitting a half-resolved table would quietly drop civilizations back onto
    # the generic fallback, which is the exact failure this tool exists to end.
    if (args.json or args.js or args.write) and (complaints or missing or extra):
        for line in complaints:
            print(f"error: {line}", file=sys.stderr)
        if missing:
            print(f"error: no jersey for {missing}", file=sys.stderr)
        if extra:
            print(f"error: not in data/civs.json: {extra}", file=sys.stderr)
        print("refusing to emit an incomplete table", file=sys.stderr)
        return 1

    if args.json:
        out = {"palette": palette,
               "generic": [[p, s, kind] for p, s, kind in generic],
               "jerseys": {c: jerseys[c] for c in names if c in jerseys}}
        path = REPO / "data/civ6_player_colors.json"
        path.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
        print(f"wrote {path.relative_to(REPO)}")
    if args.js or args.write:
        region = js_region(jerseys, [c for c in names if c in jerseys], generic)
        if args.write:
            print(f"patched {patch_index(region).relative_to(REPO)}")
        else:
            print(region)
    if not (args.json or args.js or args.write):
        for civ, source, pairs in report:
            print(f"{civ:16s} {source:34s} {pairs}")

    print(f"\n{len(palette)} palette colours, {len(jerseys)} jerseys "
          f"({sum(1 for j in jerseys.values() if j['source'] == 'civ6')} from Civ 6, "
          f"{sum(1 for j in jerseys.values() if j['source'] == 'palette')} from its palette)",
          file=sys.stderr)
    for line in complaints:
        print(f"UNRESOLVED: {line}", file=sys.stderr)
    if missing:
        print(f"MISSING from the roster: {missing}", file=sys.stderr)
    if extra:
        print(f"NOT IN data/civs.json: {extra}", file=sys.stderr)
    return 1 if complaints or missing or extra else 0


if __name__ == "__main__":
    sys.exit(main())
