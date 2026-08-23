#!/usr/bin/env python3
"""Paired A/B timing for the simulator, per completed turn, at the screen's shape.

Runs in CI on every pull request (`.github/workflows/speed.yml`), and by hand.
The long version of why is under "It runs in CI" below; keep this sentence
here, near the top, because `tools/test_ci_wiring.py` reads only the first
forty lines and a claim that drifts out of that window stops being checked
without anything going red.

## Why this is a file and not a paragraph

`docs/SIMULATOR_PERFORMANCE.md` prescribes this method in detail and in several
places — alternate the arms seed by seed, check the game reports are
byte-identical, judge against a noise floor — and the harness itself was never
in the tree. Every session rebuilt it from the prose, and the same document
records what that cost:

* a real **-11%** change was read as **+8%** from running A then B instead of
  interleaving them; host load alone swung sequential measurements by ±15%;
* one seed read **+26.7%** purely from another session's games sharing the CPU;
* a hoisted allocation measured as an improvement was a **10x pessimization**
  once counted properly;
* and the same behaviour flag read **-48.7%** one day and **-14.2%** the next,
  because whole-game time is cost *times length* and that change moved both.

The method is not the hard part. Remembering all of it, every time, is.

## What it does

1. **Pairs.** Runs baseline and candidate alternately, seed by seed, so host
   load falls on both arms equally. Never all of A then all of B.
2. **Divides by completed turns.** The primary number is seconds of user CPU
   per completed turn. Whole-game CPU is reported beside it, and when the two
   disagree the disagreement is printed, because that gap *is* the length
   change (see "Per completed turn" below).
3. **Judges the median pair, and prints the spread.** One contended pair
   cannot move a median, and a run too noisy to resolve its own budget says so
   instead of returning a confident number (see "Load" below).
4. **Proves the arms agree.** Strips the timing line and hashes each report. A
   timing difference only means overhead if both arms played the same game; if
   the reports differ the change altered behaviour and no *overhead* claim
   survives — though the cost of the new behaviour is still real and printed.
5. **Records its own conditions.** Load average at the start, the peak during,
   and at the end, in the output and in every ledger row.
6. **Confirms before failing.** Over budget, it re-measures on a disjoint block
   of seeds and fails only if the second pass agrees.
7. **Takes yes for an answer.** A `paired-cost: allow <reason>` line in the
   pull request body accepts an intended cost, exactly as `overwrite-guard:
   allow` does. A promoted feature is a performance event *by definition*, so a
   blocking gate needs a way to say "yes, and here is the number" — and the
   reason is mandatory, because that sentence is the one #2059 never wrote.
8. **Records the absolute.** `--record-ledger` writes today's cost per turn
   into `docs/speed_ledger.json`, deliberately, the way `census_report.py
   --write` records a census. A relative gate cannot see 5% a month; a
   committed absolute can.

    tools/speed_ab.py --baseline target/ci/civvis --candidate /tmp/civvis-new
    tools/speed_ab.py --baseline a --candidate b --seeds 7311000 --games 8
    tools/speed_ab.py --baseline a --candidate a      # what is the noise floor here?
    tools/speed_ab.py --baseline a --candidate a --record-ledger \\
        --ledger-machine mbp-m5-max-128 --note "after #2309"

## Per completed turn, because whole-game time is a mixture

`docs/SIMULATOR_PERFORMANCE.md` (2026-08-22) retracts a -48.7% reading of
`precise_evacuation` for exactly this reason. Withholding that feature made
games run **longer** — 745 turns to 951 over the same four seeds — so
whole-game CPU measured the length change and the cost change together, with
opposite signs, and the identical change read -48.7% on one revision and
-14.2% on another a day later. Per completed turn it is -33.3% on both.

For a byte-identical optimization the arms play the same game, the turn totals
are equal, and the two metrics are *algebraically the same number*. They can
only diverge when play changed — which is every promoted feature — and there
the per-turn column is the cost and the whole-game column is the bill.

## Load: the conditions are part of the reading

Measured on `mbp-m5-max-128` on 2026-08-23, one binary against itself, while
twelve sibling agents built concurrently. Every row is the SAME BINARY in both
arms, so the honest paired delta is zero:

| shape | 1-min load | absolute s/turn | paired delta |
| --- | ---: | ---: | ---: |
| the gate's 5x120t | ~6 (4 of the 5 seeds) | 0.0878 | — |
| the gate's 5x120t | 61 → 86 | 0.1436 (+63%) | +0.33% |
| 3x150t | ~6 | 0.1134 | — |
| 3x150t | 49 | 0.1737 (+53%) | -0.56% |
| 3x150t | 94 | 0.1761 (+55%) | +0.11% |

Two things follow, and the design turns on both.

* **The absolute number is worthless without its load.** Every ledger row
  therefore carries one, and a row taken on a busy machine is a different
  measurement from one taken quiet, not a drifted version of it.
* **The paired delta is not.** Interleaving the arms seed by seed cancels
  slowly-varying load, which is why a ±0.6% reading survives a load average of
  94 on eighteen cores. That is what makes tightening the budget defensible
  where tightening a bare stopwatch would not be.

What interleaving cannot cancel is a *burst* landing on one arm of one pair.
So the gate statistic is the **median** of the per-pair deltas rather than a
pooled ratio, and the run prints the spread of those pairs and the smallest
change that spread could have resolved. A run whose own resolution is wider
than its budget says so in the log rather than returning a confident green.

## The shape is the screen's shape

`src/bin/gene_screen.rs`'s `SCREEN_*` constants and `docs/GENE_SCREEN.md` fix
the deployment measurement at six majors on 74x46 with **nine city-states**,
Continents, Online. That last number is not decoration: #2301 measured 87% of
`precise_evacuation`'s bill on minor seats, so a gate at six city-states
systematically under-weights the cost that dominates. The map row here is
therefore the screen's, exactly.

⚠ The one leg traded for the runner's clock is the turn count: the screen plays
Online's own 250-turn limit and this gate stops at 120. That is a real
concession and it is one-directional — cost per turn rises steeply with the
turn number (measured on `mbp-m5-max-128`, ci profile, quiet: 0.088 s/turn
through turn 120, 0.115 through 150, 0.183 over full games), so the gate reads
about half the screen's per-turn density and under-weights the late game where
a per-unit pass hurts most. It is a smoke alarm sized to the runner, not a
promotion figure; `docs/EVAL.md` and the screen own that.

⚠⚠ **State it plainly: this gate is blind to a regression that only appears
after turn 120.** Six of the seven legs match the screen; the clock does not,
and it is the one leg whose difference is not proportional — turn 200 has
bigger empires, more units per seat, more cities, aircraft and a congress, so
its profile is not turn 100's scaled up. When a change plausibly touches
late-game code, run a full-clock reading by hand before believing the gate:

    tools/speed_ab.py --baseline B --candidate C --turns 250 --games 4

## It runs in CI, because reading this file was never the failure

`.github/workflows/speed.yml` builds the merge base and the head and runs this
harness with `--budget`, on every pull request. The rule it enforces is one
`docs/SIMULATOR_PERFORMANCE.md` already wrote, in a section titled *"one
promoted feature made every simulation six times slower"*:

> a promoted feature is a performance event, and this one was a six-fold event
> that no strength gate could see. `speed_ab.py` costs four minutes; a strength
> gate on a feature that multiplies game cost by six costs a day.

Four minutes, and nothing spent them, because nothing was obliged to. #2059
passed `cargo test` and a twelve-game crash soak and made every simulation in
the fleet six times slower; four PRs and four days of envelope-cache work
(#2148, #2151, #2155, #2163) went into paying it back. A gate is the only form
of that sentence that survives a busy evening.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import resource
import statistics
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

#: The committed absolute-cost ledger. Written deliberately by `--record-ledger`,
#: never by CI: a runner cannot commit, and a number nobody chose to record is
#: not a baseline.
LEDGER = REPO / "docs" / "speed_ledger.json"

#: Measured repeatedly on this fleet by running one binary against itself.
#: Anything inside it is host noise wearing a result's clothes. A hosted runner
#: is looser than a quiet desktop; `--noise-floor` is how the workflow says so.
NOISE_FLOOR_PCT = 0.2

#: The engine prints its own wall time; it is the one line that legitimately
#: differs between two runs of the same game.
TIMING_LINE = re.compile(r"^\[.*s\]")

#: How a game report says how far it got. `standings()` in `src/main.rs` prints
#: exactly one of these three lines, always, and nothing else in the report
#: contains the word "turn".
#:
#: ⚠ A report this cannot read is a hard error, not a zero. The whole point of
#: the primary metric is the divisor; silently guessing it would reproduce the
#: mixture this file exists to stop reporting.
TURNS_PLAYED = re.compile(
    r"^(?:Draw: turn limit reached on turn (\d+)"
    r"|Winner: .* on turn (\d+)"
    r"|No winner: turn (\d+) of \d+)",
    re.MULTILINE,
)

#: The escape hatch, and the same one `tools/overwrite_guard.py` uses: a line
#: in the pull request body. A required gate with no way to say "yes, and here
#: is why" is a gate that gets deleted the first Friday it is inconvenient —
#: and this one CAN legitimately fire on an intended cost, because a promoted
#: feature is a performance event by definition. The reason is mandatory: the
#: hatch costs a sentence, which is exactly the sentence #2059 never wrote.
ACKNOWLEDGEMENT = re.compile(r"^[ \t]*paired-cost:[ \t]*allow\b[ \t]*(?P<reason>.*)$",
                             re.MULTILINE | re.IGNORECASE)

#: The gate's shape: `gene_screen`'s `SCREEN_*` map row exactly, with the turn
#: clock traded down for the runner (see the module docstring). `speed.yml`
#: passes every one of these explicitly and `test_speed_ab.py` asserts the two
#: agree, so the workflow and this default cannot drift apart in silence.
GATE_SHAPE = dict(players=6, turns=120, width=74, height=46, city_states=9,
                  speed="online", map="continents")

#: Five pairs rather than three, because the gate statistic is a median and a
#: median of three has no spread to report. One interleave, because a second
#: doubles a job that already sits beside the ~10.5-minute `cargo-test` on the
#: merge path; raise it by hand on a busy desktop, where it buys more than the
#: turn clock does.
GATE_SEEDS = 900_001
GATE_GAMES = 5
GATE_INTERLEAVES = 1

#: `--baseline`/`--candidate` shape arguments, and their defaults.
DEFAULTS = dict(GATE_SHAPE)


def report_digest(text: str) -> str:
    """The game report with its timing line removed, hashed.

    ⚠ Strips rather than ignores: two reports that differ only in elapsed time
    are the same game, and two that differ anywhere else are not comparable at
    all. This is what makes a time difference mean "overhead" instead of
    "the agent made different decisions".
    """
    body = "\n".join(line for line in text.splitlines()
                     if not TIMING_LINE.match(line))
    return hashlib.sha256(body.encode("utf-8", "replace")).hexdigest()[:16]


def turns_played(text: str) -> int:
    """How many turns the game actually completed, from its own report.

    Raises rather than defaults. A missing divisor is the failure mode that
    would turn the primary metric back into whole-game time without saying so.
    """
    found = TURNS_PLAYED.search(text)
    if not found:
        raise SystemExit(
            "no turn count in the game report: `standings()` in src/main.rs "
            "prints one of 'Winner: … on turn N', 'Draw: turn limit reached on "
            "turn N' or 'No winner: turn N of M', and this report has none. If "
            "the engine's wording changed, TURNS_PLAYED here has to change with "
            "it — the per-completed-turn metric has no divisor otherwise.\n"
            f"report was:\n{text[-2000:]}")
    return int(next(group for group in found.groups() if group is not None))


def civvis_processes() -> int:
    """Other CIVVIS games on this host, which is the trap that ruins a run."""
    out = subprocess.run(["ps", "-Ao", "args="], capture_output=True,
                         text=True, check=False).stdout
    return sum(1 for line in out.splitlines()
               if re.search(r"civvis\s+(simulate|soak|tournament)|ai_eval", line))


def load_average() -> float:
    """One-minute load average, or 0.0 where the platform has none."""
    try:
        return os.getloadavg()[0]
    except (OSError, AttributeError):  # pragma: no cover - POSIX everywhere here
        return 0.0


def run_once(binary: Path, seed: int, opts: dict) -> tuple[float, str, int]:
    """One game. Returns (user CPU seconds, report digest, completed turns).

    User CPU rather than wall clock: it is what the doc's own tables record, and
    it is the half of wall clock that a competing process does not inflate as
    badly. `--jobs 1` because a thread pool turns the measurement into a
    scheduling experiment.
    """
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    done = subprocess.run(
        [str(binary), "simulate",
         "--seed", str(seed), "--jobs", "1",
         "--players", str(opts["players"]), "--turns", str(opts["turns"]),
         "--width", str(opts["width"]), "--height", str(opts["height"]),
         "--city-states", str(opts["city_states"]), "--speed", opts["speed"],
         "--map", opts["map"]],
        capture_output=True, text=True, check=False)
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if done.returncode != 0:
        raise SystemExit(f"{binary} exited {done.returncode} on seed {seed}:\n"
                         f"{done.stderr[-2000:]}")
    return (after.ru_utime - before.ru_utime,
            report_digest(done.stdout),
            turns_played(done.stdout))


def pct(before: float, after: float) -> float:
    return 100.0 * (after - before) / before if before else 0.0


def quartiles(deltas: list[float]) -> tuple[float, float]:
    """First and third quartile, with a definition that works below n=4."""
    if len(deltas) >= 4:
        cuts = statistics.quantiles(deltas, n=4)
        return cuts[0], cuts[2]
    return min(deltas), max(deltas)


def robust_sigma(deltas: list[float]) -> float:
    """How scattered the pairs were, by whichever robust estimator says worse.

    Two of them, because each has a blind spot and the blind spots are not the
    same. 1.4826 x the median absolute deviation ignores a minority of wild
    values entirely — which is right for the verdict and wrong for the
    confidence, because at five pairs one wild value genuinely does mean this
    run could not resolve much. The interquartile range does not collapse
    there. Taking the larger over-reports the noise, and over-reporting noise
    is the safe direction for a number whose only job is to say when a green
    verdict means "not seen" rather than "not there".
    """
    if len(deltas) < 2:
        return 0.0
    middle = statistics.median(deltas)
    from_mad = 1.4826 * statistics.median([abs(d - middle) for d in deltas])
    low, high = quartiles(deltas)
    return max(from_mad, (high - low) / 1.349)


def summarise(pairs: list[dict], seeds: list[int], interleaves: int,
              load: dict) -> dict:
    """The gate statistic, its spread, and the two pooled metrics beside it.

    * `median_pct` is the **gate statistic**: the median per-pair change in cost
      per completed turn. A median because the failure mode this has to survive
      is one pair landing under a burst of host load, not a uniform slowdown.
    * `resolution_pct` is the smallest change this run could have told apart
      from its own scatter — two robust standard errors of that median. When it
      is wider than the budget, the run did not have the resolution to enforce
      the budget and the output says so.
    * `per_turn_change_pct` pools every game instead, weighting long games more;
      it is what the perf ledger's tables record.
    * `change_pct` keeps its old name and its old meaning — whole-game user CPU
      — so nothing reading this dict starts quietly reading a different number.
    """
    cpu = {arm: sum(p[arm]["cpu"] for p in pairs) for arm in ("baseline", "candidate")}
    turns = {arm: sum(p[arm]["turns"] for p in pairs) for arm in ("baseline", "candidate")}
    per_turn = {arm: (cpu[arm] / turns[arm] if turns[arm] else 0.0) for arm in cpu}
    deltas = [p["delta_pct"] for p in pairs]
    sigma = robust_sigma(deltas)
    low, high = quartiles(deltas)
    return {
        "pairs": pairs,
        "deltas": deltas,
        "median_pct": statistics.median(deltas),
        "spread_pct": high - low,
        "range_pct": (min(deltas), max(deltas)),
        "resolution_pct": 2.0 * sigma / (len(deltas) ** 0.5) if sigma else 0.0,
        "totals": cpu,
        "turns": turns,
        "per_turn": per_turn,
        "per_turn_change_pct": pct(per_turn["baseline"], per_turn["candidate"]),
        "change_pct": pct(cpu["baseline"], cpu["candidate"]),
        "length_change_pct": pct(turns["baseline"], turns["candidate"]),
        "mismatched": sorted({p["seed"] for p in pairs if not p["agree"]}),
        "seeds": seeds,
        "interleaves": interleaves,
        "load": load,
    }


def compare(baseline: Path, candidate: Path, seeds: list[int], opts: dict,
            interleaves: int = 1) -> dict:
    """Alternate the arms seed by seed. Never all of A and then all of B."""
    binaries = {"baseline": baseline, "candidate": candidate}
    pairs: list[dict] = []
    load = {"start": load_average(), "peak": load_average(), "end": 0.0}
    for interleave in range(max(1, interleaves)):
        for seed in seeds:
            # Order flipped per seed, and again per interleave, so a drifting
            # host does not land on one arm.
            forward = (seed + interleave) % 2 == 0
            order = ("baseline", "candidate") if forward else ("candidate", "baseline")
            load["peak"] = max(load["peak"], load_average())
            measured, digests = {}, {}
            for arm in order:
                elapsed, digest, played = run_once(binaries[arm], seed, opts)
                measured[arm] = {"cpu": elapsed, "turns": played}
                digests[arm] = digest
            pairs.append({
                "seed": seed,
                "interleave": interleave,
                "baseline": measured["baseline"],
                "candidate": measured["candidate"],
                "agree": digests["baseline"] == digests["candidate"],
                "delta_pct": pct(
                    measured["baseline"]["cpu"] / measured["baseline"]["turns"],
                    measured["candidate"]["cpu"] / measured["candidate"]["turns"]),
            })
    load["end"] = load_average()
    load["peak"] = max(load["peak"], load["end"])
    return summarise(pairs, seeds, max(1, interleaves), load)


def verdict(result: dict, floor: float = NOISE_FLOOR_PCT) -> str:
    """What the measurement says, in the vocabulary the case deserves.

    ⚠⚠ THE DISAGREEING CASE USED TO PRINT NO NUMBER AT ALL, AND THAT IS THE
    CASE THIS FILE EXISTS FOR. `docs/SIMULATOR_PERFORMANCE.md` records #2059
    making every simulation **six times slower** and names the rule it broke:
    *a promoted feature is a performance event, and no strength gate can see
    one.* A promoted feature changes play by construction, so the reports
    differ by construction — and the old wording answered that by withholding
    the percentage and saying no claim could be made. Run against #2059 it
    would have printed the refusal and not the 6x.

    Both statements are true and they are different statements:

    * reports agree  → the same game, done slower. That is **overhead**, and a
      tenth of a percent of it is a result.
    * reports differ → a different game. That is not overhead and no
      optimization claim survives it — but it is still exactly what the new
      behaviour **costs**, which is the number a promotion needs to see.
    """
    change = result["median_pct"]
    pairs = len(result["deltas"])
    if result["mismatched"]:
        return (f"{change:+.2f}% per completed turn (median of {pairs} pairs) — "
                f"the reports differ on {len(result['mismatched'])} seed(s): "
                f"{result['mismatched']}. This is a different game, so it is NOT "
                "a measure of overhead and no optimization claim can be made "
                "from it. It is what the changed behaviour costs, which is the "
                "number a promotion has to answer for")
    if abs(change) <= floor:
        return (f"NOISE — {change:+.2f}% per completed turn (median of {pairs} "
                f"pairs) is inside the ±{floor}% floor. Not a result in either "
                "direction")
    direction = "slower" if change > 0 else "faster"
    return (f"{change:+.2f}% per completed turn (median of {pairs} pairs) — "
            f"same game on every seed, done {direction}")


def dispersion(result: dict, budget: float | None = None) -> str:
    """How scattered the pairs were, and what that run could therefore resolve.

    A green verdict from a run whose own pairs disagree by more than the budget
    is not evidence of anything, and the fleet routinely runs this on a machine
    carrying a dozen concurrent builds. Saying so is cheaper than pretending.
    """
    low, high = result["range_pct"]
    said = (f"spread: IQR {result['spread_pct']:.2f}pp over "
            f"[{low:+.2f}%, {high:+.2f}%]; this run resolves "
            f"±{result['resolution_pct']:.2f}%")
    if budget is not None and result["resolution_pct"] > abs(budget):
        said += (f" — ⚠ WIDER THAN THE {budget:+.2f}% BUDGET, so a green verdict "
                 "here is 'not seen', not 'not there'. Re-run on a quiet host, "
                 "or with more --interleaves")
    return said


def secondary(result: dict) -> str:
    """The pooled numbers, and whether they are telling the same story.

    When the arms complete the same number of turns the per-turn and
    whole-game metrics are the same number by construction, and saying so is
    worth a line: it is the property that makes a byte-identical optimization
    readable at all. When they differ, the gap is game length and nothing else,
    and quoting the whole-game figure as a cost is the mistake #2301 retracted.
    """
    pooled = result["per_turn_change_pct"]
    whole = result["change_pct"]
    base, cand = result["turns"]["baseline"], result["turns"]["candidate"]
    if base == cand:
        return (f"pooled {pooled:+.2f}% per turn; whole game {whole:+.2f}% over "
                f"the same {base} turns — the two metrics agree, as they must "
                "when both arms play the same game")
    return (f"pooled {pooled:+.2f}% per turn; whole game {whole:+.2f}% — ⚠ THE "
            f"TWO METRICS DISAGREE, and the gap is game LENGTH: {base} turns → "
            f"{cand} ({result['length_change_pct']:+.2f}%). Whole-game time is "
            "cost times length; per completed turn is the cost. "
            "docs/SIMULATOR_PERFORMANCE.md (2026-08-22) retracted a -48.7% "
            "reading of exactly this shape")


def over_budget(result: dict, budget: float | None) -> bool:
    """Whether the median pair costs more per completed turn than allowed.

    Judges the primary metric, on the median pair. A change that makes games
    *longer* raises the fleet's bill and does not raise this number —
    correctly: that is a play outcome, priced by the screen, and not overhead
    this gate can attribute. What it does catch is the case #2059 was, where
    every turn costs more.

    Deliberately blind to whether the arms agreed. A regression that also
    changes play is still a regression, and it is the kind this repository has
    actually shipped.
    """
    return budget is not None and result["median_pct"] > budget


# --------------------------------------------------------------------------
# The absolute ledger
# --------------------------------------------------------------------------
#
# A relative gate is blind to drift by construction: every pull request is
# measured against the commit before it, so a fleet can lose 5% a month and
# every single run reads green. `docs/census.json` already solved this shape of
# problem for the crate's censuses — record the reading, let the number move
# freely, make the *diff* the signal — and this is the same device for cost.
#
# ⚠ An absolute here is a machine-and-load reading, not a property of the code:
# the same binary at the same shape measured 0.1134 s/turn at load 6 and 0.1761
# at load 94 on the same Mac within the hour. Rows therefore carry their
# machine and their load average, and two rows are comparable only when both
# match.


def load_ledger(path: Path = LEDGER) -> dict:
    if not path.is_file():
        return {"shape": ledger_shape(GATE_SEEDS, GATE_GAMES, GATE_INTERLEAVES,
                                      dict(GATE_SHAPE)),
                "readings": []}
    return json.loads(path.read_text(encoding="utf-8"))


def ledger_shape(seeds: int, games: int, interleaves: int, opts: dict) -> dict:
    """The measurement conditions a reading is only comparable within.

    `profile` is here because it is the leg that silently invalidates a
    comparison: `Cargo.toml`'s `ci` profile turns debug assertions back **on**,
    so a `ci` number and a `release` number are different measurements of the
    same code.
    """
    return dict(opts, seeds=seeds, games=games, interleaves=interleaves,
                profile="ci", jobs=1)


def newest_reading(ledger: dict, machine: str) -> dict | None:
    rows = [row for row in ledger.get("readings", [])
            if row.get("machine") == machine]
    return rows[-1] if rows else None


def record_reading(ledger: dict, machine: str, cpu: float, turns: int, *,
                   commit: str, note: str, load: dict,
                   spread_pct: float | None = None,
                   date: str | None = None) -> dict:
    """Append one deliberate reading. Never called by CI; a runner cannot commit."""
    # ⚠ Divided from the ROUNDED total, not the raw one, so a reader with a
    # calculator gets the row's own third column back. A row that fails its own
    # arithmetic is a row nobody trusts the rest of.
    seconds = round(cpu, 2)
    reading = {
        "date": date or dt.date.today().isoformat(),
        "machine": machine,
        "commit": commit,
        "cpu_seconds": seconds,
        "turns": turns,
        "seconds_per_turn": round(seconds / turns, 6) if turns else 0.0,
        "load_start": round(load.get("start", 0.0), 2),
        "load_peak": round(load.get("peak", 0.0), 2),
        "load_end": round(load.get("end", 0.0), 2),
        "pair_spread_pct": round(spread_pct, 3) if spread_pct is not None else None,
        "note": note,
    }
    ledger.setdefault("readings", []).append(reading)
    return reading


def write_ledger(ledger: dict, path: Path = LEDGER) -> None:
    path.write_text(
        json.dumps(ledger, indent=2, sort_keys=False, ensure_ascii=False) + "\n",
        encoding="utf-8")


def ledger_line(ledger: dict, machine: str, measured: float, shape: dict,
                load: dict, exists: bool = True) -> str:
    """One line of absolute-cost drift, or the reason there is none yet."""
    if not exists:
        return (f"ledger: {LEDGER.name} does not exist yet; record the first "
                f"reading with --record-ledger --ledger-machine {machine}")
    if ledger.get("shape") != shape:
        return (f"ledger: NOT COMPARABLE — {LEDGER.name} records a different "
                "shape than this run measured, so no absolute drift is "
                "reported. Re-record it (--record-ledger) or fix the shape")
    recorded = newest_reading(ledger, machine)
    if recorded is None:
        return (f"ledger: no reading for {machine!r} yet — record one with "
                f"--record-ledger --ledger-machine {machine}")
    drift = pct(recorded["seconds_per_turn"], measured)
    return (f"ledger: {measured:.6f} s/turn at load {load.get('peak', 0.0):.1f} vs "
            f"{recorded['seconds_per_turn']:.6f} recorded {recorded['date']} on "
            f"{machine} at load {recorded.get('load_peak')} "
            f"({recorded['commit']}) → {drift:+.1f}%")


def acknowledged(text: str | None) -> str | None:
    """The reason a pull request body gives for accepting this cost, if any.

    `paired-cost: allow <reason>`. A bare `allow` is not an acknowledgement —
    the number and the reason are the whole point, and a marker anybody can
    paste without thinking is a gate nobody has to answer to.
    """
    found = ACKNOWLEDGEMENT.search(text or "")
    if not found:
        return None
    reason = found.group("reason").strip()
    return reason or None


def head_commit() -> str:
    done = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--short", "HEAD"],
                          capture_output=True, text=True, check=False)
    return done.stdout.strip() or "unknown"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--seeds", type=int, default=GATE_SEEDS,
                        help="first seed; games run on consecutive seeds")
    parser.add_argument("--games", type=int, default=GATE_GAMES)
    parser.add_argument("--interleaves", type=int, default=GATE_INTERLEAVES,
                        help="how many times each seed's pair is repeated. More "
                             "pairs is the answer to a loaded host, and costs "
                             "linearly.")
    parser.add_argument("--budget", type=float, default=None, metavar="PCT",
                        help="fail (exit 1) when the MEDIAN pair costs more than "
                             "PCT%% more PER COMPLETED TURN than the baseline, "
                             "whether or not the arms played the same game. "
                             "Without it the exit code keeps its old meaning: "
                             "non-zero when the reports disagree.")
    parser.add_argument("--noise-floor", type=float, default=NOISE_FLOOR_PCT,
                        metavar="PCT",
                        help="below this the verdict reads NOISE rather than a "
                             "result. A hosted runner is looser than a quiet "
                             f"desktop; the default {NOISE_FLOOR_PCT}%% is the "
                             "quiet-desktop figure.")
    parser.add_argument("--acknowledge-env", default=None, metavar="VAR",
                        help="environment variable holding the pull request "
                             "body. A line reading `paired-cost: allow "
                             "<reason>` accepts a cost this run would "
                             "otherwise fail, the way `overwrite-guard: allow` "
                             "does. The reason is required.")
    parser.add_argument("--no-confirm", action="store_true",
                        help="fail on the first over-budget reading instead of "
                             "re-measuring on a disjoint block of seeds first. "
                             "One noisy sample can then block a merge.")
    parser.add_argument("--ledger", type=Path, default=LEDGER)
    parser.add_argument("--ledger-machine", default=None, metavar="NAME",
                        help="compare the baseline arm's absolute cost per turn "
                             "against this machine's newest recorded reading")
    parser.add_argument("--record-ledger", action="store_true",
                        help="append today's reading for --ledger-machine and "
                             "write the ledger. Deliberate, like "
                             "`census_report.py --write`; CI never does this.")
    parser.add_argument("--ledger-cpu", type=float, default=None,
                        help="record this CPU total instead of running games — "
                             "for transcribing a reading taken on a machine "
                             "this process is not running on, such as a CI "
                             "runner, which cannot commit its own ledger row")
    parser.add_argument("--ledger-turns", type=int, default=None,
                        help="completed turns for --ledger-cpu")
    parser.add_argument("--ledger-load", type=float, default=None,
                        help="load average that reading was taken under")
    parser.add_argument("--ledger-spread", type=float, default=None,
                        help="pair-to-pair IQR, in percentage points, of the "
                             "run being transcribed. Without it the row records "
                             "an absolute with no scatter beside it, which is "
                             "the half of the reading that says how much to "
                             "believe the other half.")
    parser.add_argument("--ledger-commit", default=None,
                        help="commit the recorded reading describes "
                             "(default: this worktree's HEAD)")
    parser.add_argument("--note", default="",
                        help="why this reading was taken; it is the part of a "
                             "ledger row a later reader actually needs")
    for name, value in DEFAULTS.items():
        parser.add_argument(f"--{name.replace('_', '-')}",
                            type=type(value), default=value)
    return parser


def transcribe(args: argparse.Namespace, shape: dict) -> int:
    """Record a reading taken somewhere this process is not running.

    A CI runner measures the trunk's absolute cost on every pull request and
    cannot commit the row; this is how that number reaches the ledger without
    anybody hand-editing JSON. The note is required because an absolute with no
    provenance is worse than no absolute at all.
    """
    if not (args.record_ledger and args.ledger_machine and args.ledger_cpu
            and args.ledger_turns and args.note):
        raise SystemExit(
            "--ledger-cpu/--ledger-turns transcribe a reading from another "
            "machine and need --record-ledger, --ledger-machine and a --note "
            "saying where the number came from")
    ledger = load_ledger(args.ledger)
    ledger["shape"] = shape
    load = {"start": args.ledger_load or 0.0, "peak": args.ledger_load or 0.0,
            "end": args.ledger_load or 0.0}
    row = record_reading(ledger, args.ledger_machine, args.ledger_cpu,
                         args.ledger_turns, commit=args.ledger_commit or "unknown",
                         note=args.note, load=load, spread_pct=args.ledger_spread)
    write_ledger(ledger, args.ledger)
    print(f"recorded {json.dumps(row)}")
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    opts = {name: getattr(args, name) for name in DEFAULTS}
    shape = ledger_shape(args.seeds, args.games, args.interleaves, opts)

    if args.ledger_cpu is not None or args.ledger_turns is not None:
        return transcribe(args, shape)

    for binary in (args.baseline, args.candidate):
        if not binary.is_file():
            raise SystemExit(f"not a binary: {binary}")
    if args.record_ledger and not args.ledger_machine:
        raise SystemExit("--record-ledger needs --ledger-machine: a reading "
                         "with no machine on it cannot be compared to anything")

    busy_before = civvis_processes()
    if busy_before:
        print(f"⚠ {busy_before} other CIVVIS process(es) are running. One seed "
              f"has read +26.7% from contention alone; the median pair and the "
              f"spread below are what survive that, not the absolute.",
              file=sys.stderr)

    seeds = list(range(args.seeds, args.seeds + max(1, args.games)))
    if args.baseline.resolve() == args.candidate.resolve():
        print("measuring one binary against itself: this reports the noise "
              "floor on this host, not a change")
    result = compare(args.baseline, args.candidate, seeds, opts, args.interleaves)

    busy_after = civvis_processes()
    print(f"seeds {seeds[0]}..{seeds[-1]} ({len(seeds)} games x "
          f"{result['interleaves']} interleave(s) = {len(result['deltas'])} "
          f"pairs), {opts['players']}p {opts['width']}x{opts['height']} "
          f"{opts['city_states']}CS {opts['turns']}t {opts['speed']} "
          f"{opts['map']}, --jobs 1")
    print(f"  load average {result['load']['start']:.2f} at start, "
          f"{result['load']['peak']:.2f} peak, {result['load']['end']:.2f} at end"
          + (f"; {busy_before}/{busy_after} other CIVVIS process(es) before/after"
             if busy_before or busy_after else ""))
    for arm in ("baseline", "candidate"):
        print(f"  {arm:<9} {result['totals'][arm]:8.2f}s user CPU / "
              f"{result['turns'][arm]:5d} turns = "
              f"{result['per_turn'][arm]:.6f} s/turn")
    print(f"  {verdict(result, args.noise_floor)}")
    print(f"  {dispersion(result, args.budget)}")
    print(f"  {secondary(result)}")

    if args.ledger_machine:
        ledger = load_ledger(args.ledger)
        print("  " + ledger_line(ledger, args.ledger_machine,
                                 result["per_turn"]["baseline"], shape,
                                 result["load"], args.ledger.is_file()))
        if args.record_ledger:
            row = record_reading(
                ledger, args.ledger_machine, result["totals"]["baseline"],
                result["turns"]["baseline"],
                commit=args.ledger_commit or head_commit(),
                note=args.note, load=result["load"],
                spread_pct=result["spread_pct"])
            ledger["shape"] = shape
            write_ledger(ledger, args.ledger)
            print(f"  recorded {json.dumps(row)} → {args.ledger}")

    if args.budget is None:
        return 1 if result["mismatched"] else 0

    if not over_budget(result, args.budget):
        print(f"  within the {args.budget:+.2f}% budget, per completed turn")
        return 0

    print(f"  OVER BUDGET — the median pair is {result['median_pct']:+.2f}% per "
          f"completed turn, past the {args.budget:+.2f}% this run was allowed "
          f"to spend.")
    reason = acknowledged(os.environ.get(args.acknowledge_env or "", ""))
    if reason:
        print(f"  ACKNOWLEDGED in the pull request body: {reason}\n"
              f"  The cost stands and it is now written down, which is the "
              f"whole ask. Passing.")
        return 0
    if args.no_confirm:
        return 1

    # ⚠ The confirmation is what lets this be a blocking gate at all. A budget
    # tight enough to see a real 10% regression is loose enough that a burst of
    # host load can trip it, and a required check that fails at random is worse
    # than an advisory one — the fleet learns to ignore it, which is precisely
    # the state #2289 was written to leave behind. A second pass on a DISJOINT
    # block of seeds costs nothing on the ordinary run, because the ordinary
    # run never reaches this line.
    confirm_from = seeds[-1] + 1
    print(f"  confirming on a disjoint block, seeds {confirm_from}.."
          f"{confirm_from + len(seeds) - 1}, before failing the run")
    second = compare(args.baseline, args.candidate,
                     list(range(confirm_from, confirm_from + len(seeds))), opts,
                     args.interleaves)
    print(f"  confirmation: {verdict(second, args.noise_floor)}")
    print(f"  confirmation {dispersion(second, args.budget)}")
    if over_budget(second, args.budget):
        print(f"  CONFIRMED OVER BUDGET on both blocks "
              f"({result['median_pct']:+.2f}% then {second['median_pct']:+.2f}%). "
              f"Either the cost is not intended, or it is and the number belongs "
              f"in the PR body with the reason it is worth paying.")
        return 1
    print(f"  UNCONFIRMED — the second block read {second['median_pct']:+.2f}%, "
          f"inside budget. One block over and one under is a host, not a "
          f"regression; passing. Both numbers are above and both are in this log.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
