#!/usr/bin/env python3
"""Where, exactly, does CIVVIS's economy disagree with the Civilization VI it mirrors?

`mirror::economy_drift` says BY HOW MUCH the model is off, per turn, as one line
of totals. This tool says WHERE: per city, per yield, per tile, and for how
long — and what changed in the host on the turn a gap opened.

    python3 tools/civ6_yield_drift.py [run-dir] [--turns LO:HI] [--step N]
                                      [--bin civvis_orders] [--min-episode 3]
                                      [--city NAME] [--json out.json]

The newest run under ~/civvis-civ6-runs/control is used when none is given.
`--bin` names the `civvis_orders` binary; without it the tool takes the
worktree's `target/{release,ci,debug}/civvis_orders`, else the newest
PUBLISHED decider, the one the live seat actually runs:
`~/.cache/civvis/live-game-runtime/published/<sha>/civvis_orders`. The dump
is asked for with `--victory civvis` and no `--strategy`: the deployed binary
refuses `--strategy auto` ("names a league genome, and the league is retired",
#2357), which is what silently emptied every run of this tool after that
change — the dump call raised on the empty stdout instead of measuring.
For every state record the mod exported, the mirror is rebuilt exactly as the
decider rebuilds it (`civvis_orders --dump-mirror --turn N`) and its per-city
MODEL yields — `city_yields_model`, the number BEFORE the host-to-model
correction — are compared with the host's own `city:GetYield` figures. The
board a viewer sees carries the correction and agrees by construction; this
compares the thing the correction hides.

## What each block reports

- TOTALS   per-yield sum of |model − host| × turns, split into episodes that
           last ≥ `--min-episode` turns (a rule the model gets wrong) and
           transients (the host publishes a change one turn before or after
           the model does — a policy swap, a tech, a repair — which is timing,
           not a rule). Where the export carries plot yields the model figure
           already includes the mirror's per-plot corrections, so TOTALS is
           the city-level residual (buildings, routes, policies, bands) and
           TILES is where the tile model itself is judged.
- EPISODES every persistent per-city, per-yield gap: turns, sign, size, and
           the host-side changes on the turn it opened (buildings, districts,
           worked plots, specialists, amenity band, policies, government,
           routes, techs, civics, governors), so a reader can name the rule.
- TILES    when the export carries per-plot yields (`worked[].yields`, mod
           builds after this tool shipped), every worked tile where CIVVIS's
           tile model and `Plot:GetYield` disagree, with the terrain, feature,
           resource and improvement the mirror has on that plot — the exact
           row of the ruleset to look at.
- SOURCES  when the export carries `yield_sources` (the host's own per-yield
           tooltip), the host's ledger for each city in a persistent gap.
- HOUSING / AMENITIES  host count against the model's own derivation (the
           board applies a correction; the model does not). Housing is also
           broken down BY SOURCE — the host has always exported
           `housing_from_water`, `_buildings`, `_districts`, `_improvements`,
           `_civics`, `_great_people`, `_great_works` and `_starting_era`, and
           `Game::city_housing_sources` now produces the same categories, so a
           gap names the rule to read instead of only its size. A category off
           by less than one Housing is reported apart from the rest: the model
           carries the half-Housing of Farms and Pastures and the host
           publishes whole numbers, which is presentation, not a rule.

           Open residuals as of 2026-08-17, on run civvis-20260817T030352Z:
           `improvements` reads HIGH (Ostia +1.5, Cumae +2, Aquileia +1) and
           Rome's `districts` reads 2 LOW on 15 of 24 sampled turns while Rome
           holds no housing-granting district in this ruleset at all — Campus,
           Wonder, City Center, Commercial Hub, Industrial Zone. `other` is
           CIVVIS-only (resource industries, wall levels) and has no host
           counterpart by construction.
- PLAYER   Faith PER TURN at the empire level: the host's top-bar figure
           (`faith_per_turn`, `GetFaithYield`) against the model's — the
           cities plus what the empire collects beside them: founder-belief
           income and the Faith Civilization VI pays for every Great Person
           point of a class the empire can no longer earn (the last Great
           Scientist anywhere claimed; a Holy Site's Prophet points once the
           empire has a religion or the map has run out of them). On an
           export older than `faith_per_turn` the host income is read from
           the balance's next-turn change where no purchase intervened —
           the one place this tool reads across turns, and it says so. With
           `great_person_points_per_turn` in the export the model's per-class
           rate is checked against the host's, and the host's own ledger
           (`faith_sources`) is printed for the last turn.

## What it deliberately does not do

It does not judge the board. The board is corrected to the host per turn and
`civ6_mirror_check.py` guards that. It judges the MODEL, because the model is
what CIVVIS plans with when it asks "what is a Library worth here?" — a plan
priced in a currency 10% off is a plan that finishes late.

Every comparison is same-turn: the model at turn N is rebuilt from the state
record of turn N. Reading the model against a later export invents an income's
worth of drift, which is the first lesson `civ6_mirror_check.py` records.
"""

from __future__ import annotations

import argparse
import collections
import glob
import json
import math
import os
import re
import subprocess
import sys
from pathlib import Path

YIELDS = ("food", "production", "gold", "science", "culture", "faith")
REPO = Path(__file__).resolve().parent.parent
RUNS = str(Path.home() / "civvis-civ6-runs" / "control")
# A model-host difference below this is float noise from the amenity
# multiplier (host figures arrive as single-precision floats).
TOLERANCE = 0.05


# --------------------------------------------------------------- run inputs

def newest_run() -> str:
    dirs = [d for d in glob.glob(os.path.join(RUNS, "*")) if os.path.isdir(d)]
    live = [d for d in dirs if os.path.exists(os.path.join(d, "events.jsonl"))]
    if not live:
        raise SystemExit(f"no run with events.jsonl under {RUNS}")
    return max(live, key=lambda d: os.path.getmtime(os.path.join(d, "events.jsonl")))


def load_states(run: str) -> dict:
    """Every `state` record by turn. A later record for the same turn wins,
    which is what the decider itself sees."""
    states = {}
    with open(os.path.join(run, "events.jsonl"), encoding="utf-8") as handle:
        for line in handle:
            try:
                record = json.loads(line)
            except ValueError:
                continue
            if record.get("kind") == "state" and isinstance(record.get("turn"), int):
                states[record["turn"]] = record
    return states


def default_binary() -> str | None:
    for candidate in (
        REPO / "target" / "release" / "civvis_orders",
        REPO / "target" / "ci" / "civvis_orders",
        REPO / "target" / "debug" / "civvis_orders",
    ):
        if candidate.is_file():
            return str(candidate)
    published = sorted(
        glob.glob(str(Path.home() / ".cache" / "civvis" / "live-game-runtime"
                      / "published" / "*" / "civvis_orders")),
        key=os.path.getmtime,
    )
    return published[-1] if published else None


def dump_mirror(binary: str, run: str, turn: int, civ: str = "Rome") -> dict:
    """The decider's own reconstruction at `turn`, as `--dump-mirror` prints it."""
    result = subprocess.run(
        [binary, "--mirror", run, "--dump-mirror", "--turn", str(turn),
         "--victory", "civvis", "--civ", civ],
        capture_output=True, text=True, timeout=300, check=False,
    )
    return json.loads(result.stdout)


# ------------------------------------------------------------- comparisons

def yield_delta(host: dict, model: dict) -> dict:
    """model − host per yield, rounded past float noise."""
    return {key: round(float(model.get(key, 0.0)) - float(host.get(key, 0.0)), 2)
            for key in YIELDS}


# The host's own housing categories, in the order Civilization VI reports them.
# Every `state` record has carried these since the mod shipped and the totals it
# publishes are exactly their sum; the model grew the same names in
# `Game::city_housing_sources` so the two can be read against each other without
# a mapping anyone has to remember.
HOUSING_SOURCES = (
    "water", "buildings", "districts", "improvements",
    "civics", "great_people", "great_works", "starting_era",
)


def housing_source_delta(host: dict, city: dict) -> dict:
    """`model - host` per housing category, for the categories both report.

    ⚠ THE TOTAL WAS NEVER ENOUGH TO NAME A RULE. A city persistently two
    Housing short says only that something is wrong; the same city short by two
    in `buildings` and right everywhere else says which line of the ruleset to
    read. `other` is CIVVIS-only — resource industries, wall levels — so it is
    reported as a model-side figure with no host counterpart rather than
    silently compared against zero.
    """
    model = city.get("model_housing_sources")
    if not isinstance(model, dict):
        return {}
    out = {}
    for source in HOUSING_SOURCES:
        host_value = host.get(f"housing_from_{source}")
        if not isinstance(host_value, (int, float)):
            continue
        out[source] = round(float(model.get(source, 0.0)) - float(host_value), 2)
    if model.get("other"):
        out["other"] = round(float(model["other"]), 2)
    return out


def city_comparisons(state: dict, dump: dict) -> list:
    """One record per city present on both sides, keyed by host coordinates.

    Only cities whose export carries `yields` are compared; an older mod that
    sent none is unknown, not agreement.
    """
    host_by_pos = {(c.get("x"), c.get("y")): c for c in state.get("cities") or []}
    out = []
    for city in dump.get("cities") or []:
        host = host_by_pos.get((city.get("x"), city.get("y")))
        if not host or not isinstance(host.get("yields"), dict):
            continue
        model = city.get("model_yields") or {}
        record = {
            "name": host.get("name") or city.get("name"),
            "x": city.get("x"),
            "y": city.get("y"),
            "host": host,
            "model": city,
            "delta": yield_delta(host["yields"], model),
        }
        housing = host.get("housing")
        if isinstance(housing, (int, float)) and housing >= 0 \
                and isinstance(city.get("model_housing"), (int, float)):
            # The host reports Housing as a whole number (never a fraction in
            # any export), while the model carries the half-Housing of Farms
            # and Pastures; compare what the host would show.
            record["housing_delta"] = round(math.floor(city["model_housing"] + 1e-9) - housing, 2)
            record["housing_source_delta"] = housing_source_delta(host, city)
        amenities = host.get("amenities")
        if isinstance(amenities, (int, float)) and amenities >= 0 \
                and isinstance(city.get("model_amenities"), (int, float)):
            record["amenities_delta"] = int(city["model_amenities"] - amenities)
        out.append(record)
    return out


def tile_diffs(host_city: dict, model_city: dict, plots: dict | None = None) -> list:
    """Worked tiles where the host's per-plot yields and CIVVIS's tile model
    disagree. Empty when the export carries no per-plot yields (older mod).

    The city centre is compared separately from the citizens' tiles: Firaxis
    lists it as worked, CIVVIS floors it to 2 Food / 1 Production, and the
    host's `center_yields` (when exported) is the raw plot.
    """
    ledger = (model_city or {}).get("ledger") or {}
    model_tiles = {(t.get("x"), t.get("y")): t.get("yields") or {}
                   for t in ledger.get("tiles") or []}
    center = (host_city.get("x"), host_city.get("y"))
    diffs = []
    for plot in host_city.get("worked") or []:
        pos = (plot.get("x"), plot.get("y"))
        host_yields = plot.get("yields")
        if not isinstance(host_yields, dict) or pos == center:
            continue
        model_yields = model_tiles.get(pos)
        if model_yields is None:
            # A worked plot the model does not work: a district plot (a
            # specialist, imported separately) or a plot the mirror lacks.
            continue
        delta = yield_delta(host_yields, model_yields)
        if any(abs(v) > TOLERANCE for v in delta.values()):
            entry = {"x": pos[0], "y": pos[1], "host": host_yields,
                     "model": model_yields, "delta": delta}
            if plots and pos in plots:
                entry["plot"] = plots[pos]
            diffs.append(entry)
    return diffs


def episodes(series: dict, min_len: int = 3) -> list:
    """Runs of consecutive turns on which one (city, yield) delta stays put.

    `series` maps (city, yield) -> {turn: delta}. A run shorter than `min_len`
    is a transient — the host and the model publishing the same change a turn
    apart — and is not a rule the model gets wrong.
    """
    out = []
    for (city, key), by_turn in series.items():
        turns = sorted(by_turn)
        i = 0
        while i < len(turns):
            turn = turns[i]
            delta = by_turn[turn]
            if abs(delta) <= TOLERANCE:
                i += 1
                continue
            j = i
            while j + 1 < len(turns) and turns[j + 1] == turns[j] + 1 \
                    and abs(by_turn[turns[j + 1]] - delta) <= TOLERANCE:
                j += 1
            out.append({
                "city": city, "yield": key, "start": turns[i], "end": turns[j],
                "delta": delta, "turns": j - i + 1,
                "persistent": (j - i + 1) >= min_len,
            })
            i = j + 1
    out.sort(key=lambda e: (e["start"], e["city"], e["yield"]))
    return out


def state_changes(before: dict | None, after: dict, city_name: str) -> list:
    """What moved in the host between two consecutive states, for one city
    and for the empire, in the order a reader wants to see it."""
    if before is None:
        return ["(first turn compared)"]
    lines = []
    prev = next((c for c in before.get("cities") or [] if c.get("name") == city_name), None)
    cur = next((c for c in after.get("cities") or [] if c.get("name") == city_name), None)
    if prev and cur:
        for key in ("pop", "capital", "buildings", "pillaged_buildings", "wonders",
                    "specialists", "happiness_yield_mult", "amenities", "amenities_needed",
                    "housing", "religion", "pantheon_active", "great_works",
                    "incoming_routes"):
            if prev.get(key) != cur.get(key):
                lines.append(f"city.{key}: {prev.get(key)!r} -> {cur.get(key)!r}")
        pd = [(d.get("type"), d.get("x"), d.get("y"), d.get("complete"), d.get("pillaged"))
              for d in prev.get("districts") or []]
        cd = [(d.get("type"), d.get("x"), d.get("y"), d.get("complete"), d.get("pillaged"))
              for d in cur.get("districts") or []]
        if pd != cd:
            lines.append(f"districts: {pd} -> {cd}")
        pw = {(w.get("x"), w.get("y")) for w in prev.get("worked") or []}
        cw = {(w.get("x"), w.get("y")) for w in cur.get("worked") or []}
        if pw != cw:
            lines.append(f"worked: -{sorted(pw - cw)} +{sorted(cw - pw)}")
    for key in ("policies", "government", "pantheon", "religion_beliefs", "governors",
                "trade_routes", "techs", "civics", "golden_age", "dark_age", "trade_capacity",
                "dedications", "resolutions"):
        a, b = before.get(key), after.get(key)
        if a == b:
            continue
        if isinstance(a, list) and isinstance(b, list):
            sa = {json.dumps(x, sort_keys=True) for x in a}
            sb = {json.dumps(x, sort_keys=True) for x in b}
            lines.append(f"state.{key}: -{[json.loads(x) for x in sorted(sa - sb)]} "
                         f"+{[json.loads(x) for x in sorted(sb - sa)]}")
        else:
            lines.append(f"state.{key}: {a!r} -> {b!r}")
    return lines


SOURCE_LINE = re.compile(r"([+-]?\d+(?:\.\d+)?)\s*(?:%\s*)?(.*)")


def parse_yield_sources(text: str) -> list:
    """The host's per-yield tooltip as (amount, label) rows.

    The mod exports `City:GetYieldToolTip` with icon markup stripped and
    `[NEWLINE]` turned into a real newline. Every informative line starts with
    a signed amount ("+2 from Buildings", "-10% from Amenities"); the header
    and blank lines carry no amount and are dropped. Localised wording is
    passed through untouched — the reader, not this parser, knows the words.
    """
    rows = []
    for raw in (text or "").splitlines():
        line = raw.strip()
        if not line:
            continue
        match = SOURCE_LINE.match(line)
        if not match:
            continue
        amount, label = match.groups()
        label = label.strip()
        if not label:
            continue
        try:
            value = float(amount)
        except ValueError:
            continue
        rows.append((value, label))
    return rows


# ------------------------------------------------------------ player level

def host_faith_income(states: dict, turn: int) -> tuple:
    """The host's Faith per turn at `turn`, and where the figure came from.

    `faith_per_turn` is the top bar's own number and is used whenever the
    export carries it. Older exports have only the balance; there the income
    is the balance's change to the NEXT turn's record when nothing was bought
    in between (a drop, or a change far below the model, is a purchase and
    yields no reading) — the one cross-turn read in this tool, labelled so.
    Returns `(income, source)` with source `"faith_per_turn"`, `"balance"` or
    `None` when nothing can be said.
    """
    state = states[turn]
    rate = state.get("faith_per_turn")
    if isinstance(rate, (int, float)) and rate >= 0:
        return float(rate), "faith_per_turn"
    nxt = states.get(turn + 1)
    balance, later = state.get("faith"), (nxt or {}).get("faith")
    if isinstance(balance, (int, float)) and isinstance(later, (int, float)) \
            and balance >= 0 and later >= 0 and later >= balance:
        return float(later - balance), "balance"
    return None, None


def player_comparison(states: dict, turn: int, dump: dict) -> dict | None:
    """Model versus host Faith per turn for the empire, with the model's own
    split (cities / unused Great Person points / founder beliefs), the
    per-class Great Person rates on both sides where the host exports its
    own, and the host's ledger. `None` when the host income cannot be read.
    """
    model = dump.get("model_empire_yields") or {}
    if "faith" not in model:
        return None
    host, source = host_faith_income(states, turn)
    if host is None:
        return None
    extras = dump.get("player_extras") or {}
    unused = float(dump.get("unused_great_person_faith") or 0.0)
    beliefs = float(extras.get("faith", 0.0)) - unused
    model_faith = float(model["faith"])
    delta = round(model_faith - host, 2)
    # The balance is an integer (`math.floor` in the mod), so a next-turn
    # change cannot resolve below one Faith: a sub-point gap there is the
    # floor, not the model.
    if source == "balance" and abs(delta) < 1.0:
        delta = 0.0
    record = {
        "host": host, "source": source, "model": model_faith,
        "delta": delta,
        "cities": round(model_faith - float(extras.get("faith", 0.0)), 2),
        "unused_gpp": round(unused, 2), "beliefs": round(beliefs, 2),
        "unused_classes": list(dump.get("unused_great_person_classes") or []),
        "gpp_model": {k: round(float(v), 2)
                      for k, v in (dump.get("great_person_points_per_turn") or {}).items()},
    }
    host_gpp = states[turn].get("great_person_points_per_turn")
    if isinstance(host_gpp, dict):
        record["gpp_host"] = {
            k.replace("GREAT_PERSON_CLASS_", "").lower(): round(float(v), 2)
            for k, v in host_gpp.items() if isinstance(v, (int, float))
        }
    sources = states[turn].get("faith_sources")
    if isinstance(sources, str) and sources.strip():
        record["sources"] = sources
    return record



def report_housing_sources(series: dict) -> None:
    """Name the category, not just the total.

    A city persistently two Housing short says only that something is wrong.
    The same city short by two in `buildings` and right everywhere else says
    which line of the ruleset to read, which is the difference between an open
    question and a piece of work.
    """
    if not series:
        print("         (export carries no per-source breakdown; rebuild "
              "civvis_orders for model_housing_sources)")
        return
    # ⚠ THE MODEL CARRIES HALF-HOUSING AND THE HOST PUBLISHES INTEGERS. A Farm
    # is worth +0.5 Housing in this ruleset and Civilization VI reports every
    # `housing_from_*` as a whole number, so a category off by less than one is
    # a presentation difference, not a rule CIVVIS gets wrong. Reporting those
    # beside real gaps would make every farming city look broken — six of seven
    # cities showed a spurious `improvements +0.5` before this split.
    by_source: dict = collections.defaultdict(list)
    rounding: dict = collections.defaultdict(set)
    for (city, source), by_turn in series.items():
        off = [(t, d) for t, d in sorted(by_turn.items()) if abs(d) > TOLERANCE]
        whole = [(t, d) for t, d in off if abs(d) >= 1.0 - TOLERANCE]
        if whole:
            by_source[source].append((city, len(whole), len(by_turn), whole[-1]))
        elif off:
            rounding[source].add(city)
    if not by_source and not rounding:
        print("         every category agrees")
        return
    # Loudest category first: the one to read is the one that is wrong most.
    order = sorted(by_source, key=lambda k: -sum(n for _, n, _, _ in by_source[k]))
    for source in order:
        cities = sorted(by_source[source], key=lambda row: -row[1])
        detail = "; ".join(
            f"{city} {off}/{total} turns (last t{last[0]}: {last[1]:+g})"
            for city, off, total, last in cities[:6]
        )
        more = len(cities) - 6
        print(f"  from {source:13} {detail}" + (f"; +{more} more" if more > 0 else ""))
    for source, cities in sorted(rounding.items()):
        if source in by_source:
            continue
        print(f"  from {source:13} under 1 Housing in {len(cities)} city(ies) "
              f"({', '.join(sorted(cities)[:4])}) — the model's half-Housing "
              f"against the host's whole numbers, not a rule")


def report_player_faith(player_turns: dict, min_len: int) -> list:
    """Print the PLAYER block and return its persistent episodes."""
    rows = {t: r for t, r in player_turns.items() if r}
    if not rows:
        print("PLAYER   no host Faith income readable (no `faith_per_turn`, no next-turn balance)")
        return []
    sources = collections.Counter(r["source"] for r in rows.values())
    series = {("empire", "faith"): {t: r["delta"] for t, r in rows.items()}}
    all_episodes = episodes(series, min_len)
    persistent = [e for e in all_episodes if e["persistent"]]
    transient = [e for e in all_episodes if not e["persistent"]]
    weight = lambda items: round(sum(abs(e["delta"]) * e["turns"] for e in items), 1)
    print("PLAYER   Faith per turn, empire level: model (cities + unused Great Person points "
          "+ founder beliefs) against the host")
    if sources.get("balance"):
        print(f"  host income read from the balance's next-turn change on {sources['balance']} "
              f"turn(s) (export older than `faith_per_turn`; purchases hide the reading)"
              + (f", from `faith_per_turn` on {sources['faith_per_turn']}"
                 if sources.get("faith_per_turn") else ""))
    else:
        print(f"  host income from `faith_per_turn` on {len(rows)} turn(s)")
    print(f"  |model-host| x turns: persistent (>= {min_len} turns) {weight(persistent)}, "
          f"transient {weight(transient)}")
    for episode in persistent[:12]:
        first = rows[episode["start"]]
        print(f"  t{episode['start']}-{episode['end']} model-host {episode['delta']:+.2f} "
              f"(host {first['host']:.2f} model {first['model']:.2f} = cities {first['cities']:.2f}"
              f" + unused GPP {first['unused_gpp']:.2f} {first['unused_classes']}"
              f" + beliefs {first['beliefs']:.2f})")
    if len(persistent) > 12:
        print(f"  ({len(persistent) - 12} more persistent episodes)")
    last = max(rows)
    final = rows[last]
    print(f"  at turn {last}: host {final['host']:.2f} / model {final['model']:.2f} "
          f"(cities {final['cities']:.2f} + unused GPP {final['unused_gpp']:.2f} "
          f"{final['unused_classes']} + beliefs {final['beliefs']:.2f})")
    if final.get("gpp_host"):
        pairs = sorted(set(final["gpp_model"]) | set(final["gpp_host"]))
        print("  Great Person points per turn, model/host: "
              + "  ".join(f"{kind} {final['gpp_model'].get(kind, 0.0):g}/"
                          f"{final['gpp_host'].get(kind, 0.0):g}" for kind in pairs))
    if final.get("sources"):
        ledger = parse_yield_sources(final["sources"])
        if ledger:
            print("  host ledger: " + "; ".join(f"{v:+g} {label}" for v, label in ledger))
    return persistent


# ------------------------------------------------------------------ report

def fmt_yields(values: dict) -> str:
    return " ".join(f"{k[:4]}{float(values.get(k, 0.0)):+.2f}" if k in values else ""
                    for k in YIELDS).strip()


def run_report(run: str, binary: str, lo: int, hi: int, step: int, min_len: int,
               only_city: str | None, json_out: str | None) -> int:
    states = load_states(run)
    turns = [t for t in sorted(states) if lo <= t <= hi][::max(1, step)]
    if not turns:
        print(f"no state records in {run} within {lo}:{hi}")
        return 2
    series: dict = collections.defaultdict(dict)
    housing_series: dict = collections.defaultdict(dict)
    housing_source_series: dict = collections.defaultdict(dict)
    amenity_series: dict = collections.defaultdict(dict)
    per_turn = {}
    player_turns = {}
    tile_reports = []
    failed = []
    for turn in turns:
        try:
            dump = dump_mirror(binary, run, turn)
        except (ValueError, subprocess.SubprocessError, OSError) as exc:
            failed.append((turn, str(exc)[:80]))
            continue
        plots = {(p.get("x"), p.get("y")): p for p in dump.get("plots") or []}
        comparisons = city_comparisons(states[turn], dump)
        per_turn[turn] = comparisons
        player_turns[turn] = player_comparison(states, turn, dump)
        for record in comparisons:
            name = record["name"]
            if only_city and name != only_city:
                continue
            for key in YIELDS:
                series[(name, key)][turn] = record["delta"][key]
            if "housing_delta" in record:
                housing_series[name][turn] = record["housing_delta"]
                for source, delta in (record.get("housing_source_delta") or {}).items():
                    housing_source_series[(name, source)][turn] = delta
            if "amenities_delta" in record:
                amenity_series[name][turn] = record["amenities_delta"]
            for diff in tile_diffs(record["host"], record["model"], plots):
                tile_reports.append((turn, name, diff))

    print(f"run {run}")
    print(f"decider {binary}")
    print(f"turns compared {len(per_turn)} of {len(turns)} requested"
          + (f"; {len(failed)} failed to rebuild: {failed[:3]}" if failed else ""))
    if not per_turn:
        return 2

    # --- TOTALS ---------------------------------------------------------
    all_episodes = episodes(series, min_len)
    persistent = [e for e in all_episodes if e["persistent"]]
    transient = [e for e in all_episodes if not e["persistent"]]
    weight = lambda rows: {k: round(sum(abs(e["delta"]) * e["turns"] for e in rows
                                       if e["yield"] == k), 1) for k in YIELDS}
    print()
    print("TOTALS   |model-host| x turns")
    print("  (the model figure includes the per-plot corrections the mirror derives where the "
          "export carries plot yields; the tile model itself is judged in TILES below)")
    print(f"  persistent (>= {min_len} turns): {weight(persistent)}")
    print(f"  transient  (<  {min_len} turns): {weight(transient)}")
    last = max(per_turn)
    host_tot = collections.Counter()
    model_tot = collections.Counter()
    for record in per_turn[last]:
        for key in YIELDS:
            host_tot[key] += float(record["host"]["yields"].get(key, 0.0))
            model_tot[key] += float((record["model"].get("model_yields") or {}).get(key, 0.0))
    print(f"  at turn {last}: "
          + "  ".join(f"{k[:4]} {host_tot[k]:.1f}/{model_tot[k]:.1f}" for k in YIELDS)
          + "   (host/model)")

    # --- EPISODES -------------------------------------------------------
    print()
    print(f"EPISODES persistent gaps ({len(persistent)}); each with what the host changed "
          "on the turn it opened")
    for episode in persistent:
        turn = episode["start"]
        record = next((r for r in per_turn.get(turn, []) if r["name"] == episode["city"]), None)
        host = record["host"] if record else {}
        model = record["model"] if record else {}
        host_value = float((host.get("yields") or {}).get(episode["yield"], 0.0))
        model_value = float((model.get("model_yields") or {}).get(episode["yield"], 0.0))
        print(f"  t{episode['start']}-{episode['end']} {episode['city']:14} "
              f"{episode['yield']:10} model-host {episode['delta']:+.2f} "
              f"(host {host_value:.2f} model {model_value:.2f}) pop {host.get('pop')} "
              f"mult {host.get('happiness_yield_mult')}")
        before = states.get(turn - 1)
        for line in state_changes(before, states[turn], episode["city"])[:12]:
            print(f"        {line}")
        sources = (host.get("yield_sources") or {}).get(episode["yield"])
        if sources:
            rows = parse_yield_sources(sources)
            print(f"        host ledger: " + "; ".join(f"{v:+g} {label}" for v, label in rows))

    # --- TILES ----------------------------------------------------------
    print()
    if tile_reports:
        print(f"TILES    {len(tile_reports)} worked-tile disagreements (turn city plot: "
              "host / model / delta; mirror's plot)")
        seen = collections.Counter()
        for turn, name, diff in tile_reports:
            plot = diff.get("plot") or {}
            signature = (name, diff["x"], diff["y"], json.dumps(diff["delta"], sort_keys=True))
            seen[signature] += 1
            if seen[signature] > 1:
                continue
            print(f"  t{turn} {name} ({diff['x']},{diff['y']}): host {fmt_yields(diff['host'])} | "
                  f"model {fmt_yields(diff['model'])} | delta {fmt_yields(diff['delta'])} | "
                  f"{plot.get('t')} hills={plot.get('h')} f={plot.get('f')} r={plot.get('r')} "
                  f"im={plot.get('im')} pillaged={plot.get('p')}")
        repeats = sum(1 for count in seen.values() if count > 1)
        if repeats:
            print(f"  ({repeats} of those signatures repeat on later turns; shown once)")
    else:
        any_plot_yields = any(
            isinstance(w.get("yields"), dict)
            for state in states.values() for c in state.get("cities") or []
            for w in c.get("worked") or []
        )
        print("TILES    " + ("no worked-tile disagreements" if any_plot_yields
                             else "export carries no per-plot yields (mod older than "
                                  "this tool); tile attribution unavailable"))

    # --- HOUSING / AMENITIES --------------------------------------------
    print()
    for label, table in (("HOUSING", housing_series), ("AMENITIES", amenity_series)):
        if not table:
            print(f"{label:9}export carries no host figure")
            continue
        rows = []
        for name, by_turn in sorted(table.items()):
            off = [(t, d) for t, d in sorted(by_turn.items()) if abs(d) > TOLERANCE]
            if off:
                rows.append(f"{name} off on {len(off)}/{len(by_turn)} turns "
                            f"(last {off[-1][0]}: model-host {off[-1][1]:+g})")
        print(f"{label:9}" + ("; ".join(rows) if rows else "model agrees with host on every turn"))
        if label == "HOUSING" and rows:
            report_housing_sources(housing_source_series)

    # --- PLAYER ---------------------------------------------------------
    print()
    player_episodes = report_player_faith(player_turns, min_len)

    if json_out:
        payload = {
            "run": run, "binary": binary, "turns": sorted(per_turn),
            "episodes": all_episodes,
            "tiles": [{"turn": t, "city": n, **d} for t, n, d in tile_reports],
            "housing": {n: v for n, v in housing_series.items()},
            "housing_sources": {f"{n}/{k}": v
                                for (n, k), v in housing_source_series.items()},
            "amenities": {n: v for n, v in amenity_series.items()},
            "player": {str(t): v for t, v in player_turns.items() if v},
            "player_episodes": player_episodes,
        }
        with open(json_out, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=1, sort_keys=True)
        print(f"\nwrote {json_out}")
    return 1 if persistent else 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", nargs="?", help="run directory (newest if omitted)")
    parser.add_argument("--turns", default="1:100000", help="LO:HI turn window")
    parser.add_argument("--step", type=int, default=1, help="compare every Nth state")
    parser.add_argument("--bin", default=None, help="civvis_orders binary")
    parser.add_argument("--min-episode", type=int, default=3,
                        help="turns a gap must persist to count as a rule, not timing")
    parser.add_argument("--city", default=None, help="restrict to one city name")
    parser.add_argument("--json", default=None, help="write the full comparison here")
    args = parser.parse_args(argv)
    run = args.run or newest_run()
    binary = args.bin or default_binary()
    if not binary or not os.path.isfile(binary):
        print("no civvis_orders binary; build one (cargo build --release --bin civvis_orders) "
              "or pass --bin", file=sys.stderr)
        return 2
    lo, _, hi = args.turns.partition(":")
    return run_report(run, binary, int(lo or 1), int(hi or 100000), args.step,
                      args.min_episode, args.city, args.json)


if __name__ == "__main__":
    sys.exit(main())
