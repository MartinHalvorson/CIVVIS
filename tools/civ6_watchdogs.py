"""Detectors for the two failures that ran for a whole night without being noticed.

    python3 tools/civ6_watchdogs.py                 # newest run
    python3 tools/civ6_watchdogs.py --run TAG
    python3 tools/civ6_watchdogs.py --all --json out.jsonl

⚠ THIS FILE EXISTS BECAUSE BOTH FAILURES BELOW ARE INVISIBLE IN EVERY EXISTING
REPORT. `civ6_civvis_status.py` reads green on both: `orders_source: civvis` on
every turn, `applied` in the 90s, `residual: none` — while the army stands in the
capital and the mirror describes a different map than the one on screen. A summary
that cannot go red for a failure is not a check for it.

Both detectors report a NUMERATOR AND A DENOMINATOR, never a boolean, for the reason
this project has now learned four times: a mechanism that works and a mechanism that
destroys itself both read "connected".

## 1. idle stack — units piling up in a city and never being used

The shape: a unit is built, is given no order it can act on, and stands on the city
centre. Civilization VI permits UNLIMITED stacking on a city centre, so the engine
never pushes back and nothing in the event stream complains. Measured as unit-turns,
because a snapshot of one turn cannot tell a garrison from a traffic jam:

  stuck_unit_turns / unit_turns   a unit-turn is stuck when the unit did not change
                                  plot since the previous state AND is standing on
                                  one of our own city centres.
  worst_stack                     most own units sharing a single city-centre plot.
  frozen_units                    units that never moved at all, over their lifetime,
                                  and lived at least `--frozen-turns` turns.

⚠ Standing still is not by itself a fault — a garrison is supposed to stand still, and
one defender per city is a floor this project deliberately added. That is why the
report separates the first unit on a plot from the surplus, and why `worst_stack` is
printed beside the fraction. Read them together.

## 2. mirror disagreement — CIVVIS's board vs the one Civilization VI exported

The shape: the two sides use different coordinate systems (Civ 6 OFFSET, CIVVIS
AXIAL) and both are pairs of small integers, so a mix-up silently lands on a
different hex. It has already cost this project a capital that had NO TILE in the
reconstruction, and it is the standing explanation for improvement orders that are
re-issued and refused forever.

This half needs CIVVIS's own view, which `civvis_orders --dump-mirror` prints back in
OFFSET coordinates. Given both, the check is tile for tile:

  agree / compared      plots present on both sides that carry the same terrain.
  missing_in_mirror     plots Civ 6 exported that the mirror does not hold AT ALL.
                        ⚠ This is the coordinate bug's signature: not a wrong value,
                        an absent tile.
  disagree              per-field disagreement counts (terrain, hills, water, ...).

A run with `missing_in_mirror` above zero is not a valid measurement of CIVVIS's
judgement: it answered a different map.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
HERE = Path(__file__).resolve().parent


def newest_run() -> Path | None:
    runs = [p for p in RUN_ROOT.iterdir() if (p / "events.jsonl").exists()]
    return max(runs, key=lambda p: (p / "events.jsonl").stat().st_mtime) if runs else None


def read_events(run: Path) -> list[dict]:
    out = []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            out.append(json.loads(line))
        except ValueError:
            continue
    return out


# --------------------------------------------------------------------------- 1


def idle_stack(events: list[dict], frozen_turns: int = 20) -> dict:
    """Unit-turns spent motionless on our own city centres.

    Walks `state` events, which carry every own unit with an id and a plot, and every
    own city with its plot. Keyed by unit id across turns.

    ⚠ Unit ids are Civilization VI's, not the mirror's — the mirror reassigns them
    every turn, which is why this reads the EXPORT and not anything CIVVIS produced.
    An id that disappears is a dead unit, not a moved one.
    """
    states = [e for e in events if e.get("kind") == "state"]
    prev: dict[int, tuple[int, int]] = {}
    first_seen: dict[int, int] = {}
    last_seen: dict[int, int] = {}
    # ⚠ NAME THE KIND. "2 of 36 units never moved" is a number nobody can act on;
    # "a TRADER stood still for 113 turns" names a whole capability that does not
    # cross the bridge — Civ 6 trade routes are not actuated, so every trader CIVVIS
    # builds is dead weight for the rest of the game. The count alone read as noise.
    kinds: dict[int, str] = {}
    ever_moved: set[int] = set()
    unit_turns = 0
    stuck_turns = 0
    surplus_turns = 0
    worst_stack = 0
    worst_at: tuple | None = None
    per_turn: list[tuple[int, int, int, int]] = []

    for state in states:
        turn = state.get("turn")
        centres = {(c["x"], c["y"]) for c in (state.get("cities") or [])}
        units = state.get("units") or []
        occupancy: dict[tuple[int, int], list[int]] = defaultdict(list)
        turn_units = 0
        turn_stuck = 0
        for unit in units:
            uid = unit.get("id")
            if uid is None:
                continue
            plot = (unit.get("x"), unit.get("y"))
            kinds[uid] = unit.get("kind", "?")
            first_seen.setdefault(uid, turn)
            last_seen[uid] = turn
            occupancy[plot].append(uid)
            if uid in prev:
                turn_units += 1
                unit_turns += 1
                if prev[uid] != plot:
                    ever_moved.add(uid)
                elif plot in centres:
                    stuck_turns += 1
                    turn_stuck += 1
            prev_plot = prev.get(uid)
            if prev_plot is not None and prev_plot != plot:
                ever_moved.add(uid)
        # Surplus is everything beyond the first unit on a city centre: the first is a
        # garrison, the rest are the pile-up this detector is named for.
        turn_surplus = 0
        for plot, ids in occupancy.items():
            if plot in centres:
                turn_surplus += max(0, len(ids) - 1)
                if len(ids) > worst_stack:
                    worst_stack, worst_at = len(ids), (turn, plot)
        surplus_turns += turn_surplus
        per_turn.append((turn, turn_units, turn_stuck, turn_surplus))
        prev = {u["id"]: (u["x"], u["y"]) for u in units if u.get("id") is not None}

    # ★★★★ HOW FAR THE EMPIRE REACHES. A stack on the city centre is one way an army
    # goes unused; the other is an army that never leaves the neighbourhood, and the
    # idle-stack fraction cannot see it — every unit moves a tile each turn and none of
    # them goes anywhere. It is the failure behind `met` stalling at 1-2 of 3 rivals,
    # which makes domination unreachable no matter how good the siege code is: no
    # contact, no visible rival city, nothing for the army to attack.
    #
    # Measured on run civvis-20260731T075743Z at turn 77: TWELVE units alive, five of
    # them archers, and the FURTHEST from any of our cities was 2 tiles (mean 1.2).
    # ★★★★ CITIES LOST, WHICH CAPS THE EMPIRE EXACTLY AS HARD AS SETTLERS DYING.
    # A ladder that reads "peak 4, final 3" has founded four cities and been unable to
    # HOLD four, and those are different repairs. Recorded with the turn and the plot
    # so the event stream around it can be read.
    lost: list = []
    previous: set | None = None
    for state in states:
        held = {(c["x"], c["y"]) for c in (state.get("cities") or [])}
        if previous is not None:
            for plot in sorted(previous - held):
                lost.append({"turn": state.get("turn"), "plot": plot})
        previous = held

    # ⚠⚠ OVER THE WHOLE RUN, NOT THE LAST TURN — and reading the last turn alone
    # nearly bought me a wrong conclusion tonight. Sampled at the final state the four
    # attempts of 2026-07-31 read 13, 56, 4 and 2 tiles, which looks like a collapse
    # caused by whatever changed between them. It is not: the last turn of a run that
    # is losing units has three units left, all of them at home defending, and a run
    # that ended on a scout abroad reads 56. The number that means something is the
    # furthest ANY unit got at ANY point, beside the last-turn picture.
    def offset_distance(a, b):
        def axial(p):
            col, row = p
            return col - (row - (row & 1)) // 2, row
        (aq, ar), (bq, br) = axial(a), axial(b)
        return max(abs(aq - bq), abs(ar - br), abs((-aq - ar) - (-bq - br)))

    reach = {"furthest": None, "mean": None, "units": 0, "furthest_ever": None,
             "furthest_ever_turn": None}
    ever, ever_turn = None, None
    for state in states:
        centres = [(c["x"], c["y"]) for c in (state.get("cities") or [])]
        units = state.get("units") or []
        if not centres or not units:
            continue
        spread = [
            min(offset_distance((u["x"], u["y"]), c) for c in centres) for u in units
        ]
        if ever is None or max(spread) > ever:
            ever, ever_turn = max(spread), state.get("turn")
        if state is states[-1]:
            reach.update({
                "furthest": max(spread),
                "mean": round(sum(spread) / len(spread), 1),
                "units": len(spread),
            })
    reach["furthest_ever"] = ever
    reach["furthest_ever_turn"] = ever_turn
    # How far into the game this judgement is being made. A scout has not had time to
    # get anywhere by turn 26, and a detector that cries wolf on every young run is one
    # people learn to scroll past.
    reach["last_turn"] = states[-1].get("turn") if states else None

    frozen = [
        (uid, kinds.get(uid, "?"), last_seen[uid] - first)
        for uid, first in first_seen.items()
        if uid not in ever_moved and (last_seen[uid] - first) >= frozen_turns
    ]
    return {
        "unit_turns": unit_turns,
        "stuck_unit_turns": stuck_turns,
        "stuck_fraction": round(stuck_turns / unit_turns, 3) if unit_turns else None,
        "surplus_on_centres_unit_turns": surplus_turns,
        "worst_stack": worst_stack,
        "worst_stack_at": worst_at,
        "reach": reach,
        "cities_lost": lost,
        "frozen_units": len(frozen),
        "frozen_by_kind": dict(Counter(kind for _, kind, _ in frozen).most_common()),
        "frozen_worst": sorted(frozen, key=lambda row: -row[2])[:4],
        "units_seen": len(first_seen),
        "per_turn_tail": per_turn[-8:],
    }


# --------------------------------------------------------------------------- 2

# The improvements CIVVIS models. Anything else the mirror stores as None, so the
# tile reads UNIMPROVED — honest for a name that cannot be translated, and also the
# exact condition that made CIVVIS order 19 builders for one city. Counted apart from
# a disagreement, because it is a known gap rather than a wrong hex.
MODELLED_IMPROVEMENTS = {
    "farm", "mine", "quarry", "pasture", "plantation", "camp", "fishing_boats",
    "lumber_mill", "oil_well", "offshore_oil_rig", "fort", "airstrip",
    "national_park", "industry", "seaside_resort", "ski_resort",
}


def load_vocab() -> dict:
    """The SAME table the mirror builds from, so the diff compares like with like.

    ⚠ Terrain, feature and resource names are written into the mirror THROUGH this
    table, so those three columns are only a check on the translation being applied
    at all — they cannot catch a wrong table. What they do catch, and what has
    actually bitten: a name absent from the table, which does not error and leaves
    the tile carrying whatever `Game::new` generated.

    The columns that are genuinely independent are `w` (Civ 6 answers `IsWater()`;
    CIVVIS derives water from its own ruleset) and the presence of the tile at all.
    """
    return json.loads((HERE / "civ6_control" / "vocab.json").read_text())


def expected(plot: dict, vocab: dict) -> dict:
    """What the mirror SHOULD hold for a plot Civilization VI exported."""
    terrain = vocab["terrains"].get(plot.get("t") or "")
    feature = vocab["features"].get(plot.get("f") or "") if plot.get("f") else None
    resource = vocab["resources"].get(plot.get("r") or "") if plot.get("r") else None
    improvement = None
    if plot.get("im"):
        short = plot["im"].removeprefix("IMPROVEMENT_").lower()
        improvement = short if short in MODELLED_IMPROVEMENTS else None
    return {
        "t": terrain["terrain"] if terrain else None,
        "h": terrain["hills"] if terrain else None,
        "w": plot.get("w"),
        "f": feature,
        "r": resource,
        "im": improvement,
        "own": plot.get("o") == 0,
    }


# Fields compared after translation. `t`/`h`/`f`/`r` go through the vocabulary; `w`
# and `own` are answered independently by each side.
COMPARED = ("t", "h", "w", "f", "r", "im", "own")


def latest_tiles(events: list[dict]) -> dict[tuple[int, int], dict]:
    """The most recent export of every plot, merged across chunks and re-exports.

    ⚠ `tiles` arrives in chunks and the map is re-exported every few turns, so a
    single event is a fragment. Later exports win: a plot's terrain can legitimately
    change (a chop, an improvement), and the mirror is rebuilt from the same stream.
    """
    plots: dict[tuple[int, int], dict] = {}
    for event in events:
        if event.get("kind") != "tiles":
            continue
        for plot in event.get("plots") or []:
            plots[(plot.get("x"), plot.get("y"))] = plot
    return plots


def mirror_agreement(run: Path, events: list[dict], orders_bin: Path) -> dict:
    """Ask CIVVIS for its board in OFFSET coordinates and diff it against the export."""
    exported = latest_tiles(events)
    if not exported:
        return {"error": "run exported no tiles"}
    try:
        proc = subprocess.run(
            [str(orders_bin), "--mirror", str(run), "--dump-mirror"],
            capture_output=True, text=True, timeout=300,
        )
    except (subprocess.SubprocessError, OSError) as exc:
        return {"error": f"could not run {orders_bin}: {exc}"}
    if proc.returncode != 0:
        return {"error": f"dump-mirror exited {proc.returncode}: {proc.stderr[-400:]}"}
    try:
        dump = json.loads(proc.stdout)
    except ValueError:
        return {"error": f"dump-mirror printed no JSON: {proc.stdout[:200]!r}"}

    vocab = load_vocab()
    mirrored = {(p["x"], p["y"]): p for p in dump.get("plots", [])}
    missing = [k for k in exported if k not in mirrored]
    # Tiles the mirror holds that Civ 6 never exported are the deliberate land
    # frontier — the ring of invented land at the edge of what the seat has revealed,
    # which exists because unrevealed ground otherwise reads as ocean and the empire
    # looks like an island. Reported, never counted as a disagreement.
    extra = [k for k in mirrored if k not in exported]
    disagree: Counter = Counter()
    examples: dict[str, list] = defaultdict(list)
    unmodelled_improvements: Counter = Counter()
    compared = 0
    agree = 0
    for key, plot in exported.items():
        got = mirrored.get(key)
        if got is None:
            continue
        compared += 1
        want = expected(plot, vocab)
        if plot.get("im") and want["im"] is None:
            unmodelled_improvements[plot["im"]] += 1
        bad = []
        for field in COMPARED:
            a, b = want.get(field), got.get(field)
            if a != b:
                bad.append(field)
                disagree[field] += 1
                if len(examples[field]) < 3:
                    examples[field].append(
                        {"xy": key, "civ6": plot.get(field if field != "own" else "o"),
                         "expected": a, "mirror": b})
        if not bad:
            agree += 1
    return {
        "exported_plots": len(exported),
        "mirrored_plots": len(mirrored),
        "compared": compared,
        "agree": agree,
        "agree_fraction": round(agree / compared, 4) if compared else None,
        "missing_in_mirror": len(missing),
        "missing_examples": missing[:6],
        "extra_in_mirror_frontier": len(extra),
        "unresolved_terrain": dump.get("unresolved_terrain") or {},
        "unmodelled_improvements": dict(unmodelled_improvements.most_common(8)),
        "disagree_by_field": dict(disagree.most_common()),
        "examples": {k: v for k, v in examples.items()},
    }


# ---------------------------------------------------------------------------


def verdicts(report: dict, stuck_max: float, agree_min: float) -> list[str]:
    """The loud half. A detector nobody reads is the failure it was written for."""
    out = []
    idle = report.get("idle_stack") or {}
    frac = idle.get("stuck_fraction")
    if frac is not None and frac > stuck_max:
        out.append(
            f"IDLE STACK: {idle['stuck_unit_turns']}/{idle['unit_turns']} unit-turns "
            f"({frac:.0%}) motionless on a city centre, worst stack "
            f"{idle['worst_stack']} at {idle['worst_stack_at']}")
    if idle.get("cities_lost"):
        out.append(
            f"CITIES LOST: {len(idle['cities_lost'])} — {idle['cities_lost']}. "
            f"Founding a city and holding it are different problems and the peak "
            f"count hides the second one.")
    reach = idle.get("reach") or {}
    if (reach.get("furthest_ever") is not None
            and reach["furthest_ever"] <= 8
            and (reach.get("last_turn") or 0) >= 60):
        out.append(
            f"THE EMPIRE NEVER REACHED: the furthest any unit ever got from one of our "
            f"cities was {reach['furthest_ever']} tiles, at turn "
            f"{reach['furthest_ever_turn']}. Nothing went looking for anybody — this is "
            f"what makes `met` stall and domination unreachable.")
    if idle.get("frozen_units"):
        out.append(
            f"FROZEN UNITS: {idle['frozen_units']} of {idle['units_seen']} units never "
            f"moved once in their whole life — {idle.get('frozen_by_kind')} "
            f"(longest: {idle.get('frozen_worst')})")
    mirror = report.get("mirror") or {}
    if mirror.get("error"):
        out.append(f"MIRROR NOT CHECKED: {mirror['error']}")
    else:
        if mirror.get("missing_in_mirror"):
            out.append(
                f"MIRROR MISSING TILES: {mirror['missing_in_mirror']} plots Civ 6 "
                f"exported have no tile in CIVVIS's board "
                f"(e.g. {mirror.get('missing_examples')}) — the coordinate bug's signature")
        af = mirror.get("agree_fraction")
        if af is not None and af < agree_min:
            out.append(
                f"MIRROR DISAGREES: only {mirror['agree']}/{mirror['compared']} "
                f"({af:.1%}) of plots match tile for tile; "
                f"{mirror.get('disagree_by_field')}")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=None)
    ap.add_argument("--all", action="store_true", help="every run that has events")
    ap.add_argument("--json", default=None, help="append one JSON object per run here")
    ap.add_argument("--no-mirror", action="store_true",
                    help="skip the tile diff (it rebuilds the board and costs seconds)")
    ap.add_argument("--frozen-turns", type=int, default=20)
    ap.add_argument("--stuck-max", type=float, default=0.35,
                    help="fraction of unit-turns motionless on a city centre before it is loud")
    ap.add_argument("--agree-min", type=float, default=0.98)
    ap.add_argument("--orders-bin",
                    default=str(HERE.parent / "target" / "release" / "civvis_orders"))
    args = ap.parse_args()

    if args.all:
        runs = sorted((p for p in RUN_ROOT.iterdir() if (p / "events.jsonl").exists()),
                      key=lambda p: p.name)
    elif args.run:
        runs = [RUN_ROOT / args.run]
    else:
        one = newest_run()
        if one is None:
            print("no run with events found")
            return 2
        runs = [one]

    handle = open(args.json, "a") if args.json else None
    bad = 0
    for run in runs:
        if not (run / "events.jsonl").exists():
            print(f"{run.name}: no events")
            continue
        events = read_events(run)
        report = {
            "run": run.name,
            # How much stream this verdict was formed on, so a check taken while the
            # run was still playing can be told apart from the final one.
            "events_bytes": (run / "events.jsonl").stat().st_size,
            "idle_stack": idle_stack(events, args.frozen_turns),
        }
        if not args.no_mirror:
            report["mirror"] = mirror_agreement(run, events, Path(args.orders_bin))
        found = verdicts(report, args.stuck_max, args.agree_min)
        report["verdicts"] = found
        if handle:
            handle.write(json.dumps(report, sort_keys=True, default=str) + "\n")
        idle = report["idle_stack"]
        print(f"{run.name}")
        reach = idle.get("reach") or {}
        print(f"  reach: furthest EVER {reach.get('furthest_ever')} tiles "
              f"(at t{reach.get('furthest_ever_turn')}); at the last turn "
              f"{reach.get('furthest')} furthest / {reach.get('mean')} mean "
              f"over {reach.get('units')} units")
        print(f"  idle: stuck {idle['stuck_unit_turns']}/{idle['unit_turns']} unit-turns"
              f" ({idle['stuck_fraction']})  surplus-on-centres "
              f"{idle['surplus_on_centres_unit_turns']}  worst stack {idle['worst_stack']}"
              f" at {idle['worst_stack_at']}  frozen {idle['frozen_units']}/{idle['units_seen']}")
        if "mirror" in report:
            mirror = report["mirror"]
            if mirror.get("error"):
                print(f"  mirror: {mirror['error']}")
            else:
                print(f"  mirror: agree {mirror['agree']}/{mirror['compared']} "
                      f"({mirror['agree_fraction']})  missing {mirror['missing_in_mirror']}"
                      f"  frontier {mirror['extra_in_mirror_frontier']}  "
                      f"{mirror['disagree_by_field']}")
                if mirror.get("unresolved_terrain"):
                    print(f"    unresolved terrain names: {mirror['unresolved_terrain']}")
                if mirror.get("unmodelled_improvements"):
                    print(f"    improvements CIVVIS does not model (tile reads "
                          f"unimproved): {mirror['unmodelled_improvements']}")
                for field, cases in (mirror.get("examples") or {}).items():
                    print(f"    {field}: {cases}")
        for line in found:
            print(f"  ⚠ {line}")
            bad += 1
    if handle:
        handle.close()
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
