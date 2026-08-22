#!/usr/bin/env python3
"""Paired A/B timing for the simulator, with the traps already paid for.

Runs in CI on every pull request (`.github/workflows/speed.yml`), and by hand.
The long version of why is under "It runs in CI" below; keep this sentence
here, near the top, because `tools/test_ci_wiring.py` reads only the first
forty lines and a claim that drifts out of that window stops being checked
without anything going red.

## Why this is a file and not a paragraph

`docs/SIMULATOR_PERFORMANCE.md` prescribes this method in detail and in several
places — alternate the arms seed by seed, check the game reports are
byte-identical, judge against a ±0.2% noise floor — and the harness itself was
never in the tree. Every session rebuilt it from the prose, and the same
document records what that cost:

* a real **−11%** change was read as **+8%** from running A then B instead of
  interleaving them; host load alone swung sequential measurements by ±15%;
* one seed read **+26.7%** purely from another session's games sharing the CPU;
* a hoisted allocation measured as an improvement was a **10x pessimization**
  once counted properly.

The method is not the hard part. Remembering all of it, every time, is.

## What it does

1. **Pairs.** Runs baseline and candidate alternately, seed by seed, so host
   load falls on both arms equally. Never all of A then all of B.
2. **Proves the arms agree.** Strips the timing line and hashes each report. A
   timing difference only means overhead if both arms played the same game; if
   the reports differ the change altered behaviour and the numbers below are
   meaningless, so it says so and refuses a verdict.
3. **Watches the host.** Counts other CIVVIS processes before and after. A
   busy host is the one failure mode that makes a good change look bad and a
   bad one look good, and it is invisible in the output it corrupts.
4. **Judges against the floor.** A delta inside ±0.2% is reported as noise, not
   as a win. Run `--baseline X --candidate X` to re-measure the floor on this
   host before trusting a small number.

    tools/speed_ab.py --baseline target/ci/civvis --candidate /tmp/civvis-new
    tools/speed_ab.py --baseline a --candidate b --seeds 7311000 --games 8
    tools/speed_ab.py --baseline a --candidate a      # what is the noise floor here?

## It runs in CI, because reading this file was never the failure

`.github/workflows/speed.yml` builds the merge base and the head and runs this
harness with `--budget`, on every pull request. The rule it enforces is one
this document already wrote, in a section titled *"one promoted feature made
every simulation six times slower"*:

> a promoted feature is a performance event, and this one was a six-fold event
> that no strength gate could see. `speed_ab.py` costs four minutes; a strength
> gate on a feature that multiplies game cost by six costs a day.

Four minutes, and nothing spent them, because nothing was obliged to. #2059
passed `cargo test` and a twelve-game crash soak and made every simulation in
the fleet six times slower; four PRs and four days of envelope-cache work
(#2148, #2151, #2155, #2163) went into paying it back. A gate is the only form
of that sentence that survives a busy evening.

⚠ The budget is deliberately loose (`--budget 50`). This does not exist to
find five percent — on a shared runner five percent is not there to be found.
It exists so nothing multiplies the cost of every evaluation in the fleet
without a human seeing the number first.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import resource
import subprocess
import sys
from pathlib import Path

#: Measured repeatedly on this fleet by running one binary against itself.
#: Anything inside it is host noise wearing a result's clothes.
NOISE_FLOOR_PCT = 0.2

#: The engine prints its own wall time; it is the one line that legitimately
#: differs between two runs of the same game.
TIMING_LINE = re.compile(r"^\[.*s\]")

#: Enough to be a game rather than an opening, at the shape the deployment
#: evaluator uses. Overridable, because the right shape is the one the change
#: you are measuring actually touches.
DEFAULTS = dict(players=6, turns=150, width=74, height=46, city_states=9,
                speed="online")


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


def civvis_processes() -> int:
    """Other CIVVIS games on this host, which is the trap that ruins a run."""
    out = subprocess.run(["ps", "-Ao", "args="], capture_output=True,
                         text=True, check=False).stdout
    return sum(1 for line in out.splitlines()
               if re.search(r"civvis\s+(simulate|soak|tournament)|ai_eval", line))


def run_once(binary: Path, seed: int, opts: dict) -> tuple[float, str]:
    """One game. Returns (user CPU seconds, report digest).

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
         "--city-states", str(opts["city_states"]), "--speed", opts["speed"]],
        capture_output=True, text=True, check=False)
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if done.returncode != 0:
        raise SystemExit(f"{binary} exited {done.returncode} on seed {seed}:\n"
                         f"{done.stderr[-2000:]}")
    return after.ru_utime - before.ru_utime, report_digest(done.stdout)


def compare(baseline: Path, candidate: Path, seeds: list[int],
            opts: dict) -> dict:
    """Alternate the arms seed by seed. Never all of A and then all of B."""
    totals = {"baseline": 0.0, "candidate": 0.0}
    mismatched: list[int] = []
    for seed in seeds:
        # Order flipped per seed so a drifting host does not land on one arm.
        first, second = ("baseline", "candidate") if seed % 2 == 0 else \
                        ("candidate", "baseline")
        binaries = {"baseline": baseline, "candidate": candidate}
        digests = {}
        for arm in (first, second):
            elapsed, digest = run_once(binaries[arm], seed, opts)
            totals[arm] += elapsed
            digests[arm] = digest
        if digests["baseline"] != digests["candidate"]:
            mismatched.append(seed)
    change = (100.0 * (totals["candidate"] - totals["baseline"]) / totals["baseline"]
              if totals["baseline"] else 0.0)
    return {"totals": totals, "change_pct": change, "mismatched": mismatched,
            "seeds": seeds}


def verdict(result: dict) -> str:
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
    change = result["change_pct"]
    if result["mismatched"]:
        return (f"{change:+.2f}% FEATURE COST — the reports differ on "
                f"{len(result['mismatched'])} seed(s): {result['mismatched']}. "
                "This is a different game, so it is NOT a measure of overhead "
                "and no optimization claim can be made from it. It is what the "
                "changed behaviour costs, which is the number a promotion has "
                "to answer for")
    if abs(change) <= NOISE_FLOOR_PCT:
        return (f"NOISE — {change:+.2f}% is inside the ±{NOISE_FLOOR_PCT}% floor. "
                "Not a result in either direction")
    return f"{change:+.2f}% overhead — same game on every seed, done slower"


def over_budget(result: dict, budget: float | None) -> bool:
    """Whether the candidate costs more than the run was allowed to spend.

    Deliberately blind to whether the arms agreed. A regression that also
    changes play is still a regression, and it is the kind this repository has
    actually shipped.
    """
    return budget is not None and result["change_pct"] > budget


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--seeds", type=int, default=7_311_000,
                        help="first seed; games run on consecutive seeds")
    parser.add_argument("--games", type=int, default=8)
    parser.add_argument("--budget", type=float, default=None, metavar="PCT",
                        help="fail (exit 1) when the candidate costs more than "
                             "PCT%% over the baseline, whether or not the arms "
                             "played the same game. Without it the exit code "
                             "keeps its old meaning: non-zero when the reports "
                             "disagree.")
    for name, value in DEFAULTS.items():
        parser.add_argument(f"--{name.replace('_', '-')}",
                            type=type(value), default=value)
    args = parser.parse_args(argv)

    for binary in (args.baseline, args.candidate):
        if not binary.is_file():
            raise SystemExit(f"not a binary: {binary}")

    busy_before = civvis_processes()
    if busy_before:
        print(f"⚠ {busy_before} other CIVVIS process(es) are running. One seed "
              f"has read +26.7% from contention alone; stop them or expect a "
              f"number that means nothing.", file=sys.stderr)

    opts = {name: getattr(args, name) for name in DEFAULTS}
    seeds = list(range(args.seeds, args.seeds + max(1, args.games)))
    if args.baseline.resolve() == args.candidate.resolve():
        print("measuring one binary against itself: this reports the noise "
              "floor on this host, not a change")
    result = compare(args.baseline, args.candidate, seeds, opts)

    busy_after = civvis_processes()
    print(f"seeds {seeds[0]}..{seeds[-1]} ({len(seeds)} paired games), "
          f"{opts['players']}p {opts['width']}x{opts['height']} "
          f"{opts['turns']}t {opts['speed']}, --jobs 1")
    print(f"  baseline  {result['totals']['baseline']:8.2f}s user CPU")
    print(f"  candidate {result['totals']['candidate']:8.2f}s user CPU")
    print(f"  {verdict(result)}")
    if busy_before or busy_after:
        print(f"  ⚠ host was not quiet: {busy_before} CIVVIS process(es) before, "
              f"{busy_after} after")
    if args.budget is not None:
        if over_budget(result, args.budget):
            print(f"  OVER BUDGET — {result['change_pct']:+.2f}% is past the "
                  f"{args.budget:+.2f}% this run was allowed to spend. Either "
                  f"the cost is not intended, or it is and the number belongs "
                  f"in the PR body with the reason it is worth paying.")
            return 1
        print(f"  within the {args.budget:+.2f}% budget")
        return 0
    return 1 if result["mismatched"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
