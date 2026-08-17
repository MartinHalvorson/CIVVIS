#!/usr/bin/env python3
"""Run the Tactics arena benchmark and compare it against the committed baseline.

The Tactics mode exists to develop AI tactical combat, which means every change
to that AI has to be answerable with a number rather than an impression. This
runs the standard battery — the same opponents in the same regimes, every time —
and prints a table beside `docs/TACTICS_BASELINE.md`.

Two things it exists to stop.

**Reading the null as a result.** `advanced_v1` is a frozen copy of the live
controller, and nearly everything separating them is empire-level machinery
that an arena never exercises. Rating one against the other on a battlefield
therefore lands near 50% whatever the tactical AI does, and that number means
"these two share a tactical core", not "there is no headroom". The battery pairs
every controller against `basic` as well, which is a genuinely different
opponent, and it is the `basic` column that moves when tactical play changes.

**Rating two experiments into one ledger.** Each run writes to its own scratch
ratings file under a temporary directory, never to the committed Tactics ledger.
The engine also records the arena's economy in the rating profile, so a mixed
ledger would be refused rather than silently averaged — this simply keeps the
question from arising.

Usage:

    tools/tactics_bench.py                      # run the battery, compare to baseline
    tools/tactics_bench.py --games 80           # more games for a tighter interval
    tools/tactics_bench.py --write-baseline     # record a new baseline
    tools/tactics_bench.py --only attrition     # one regime while iterating
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BASELINE = REPO / "docs" / "TACTICS_BASELINE.md"

#: Seat-mirrored games per matchup, and the floor a baseline may be written at.
#:
#: ⚠⚠⚠ FORTY WAS THE DEFAULT AND FORTY MANUFACTURED A FALSE ALARM. On the same
#: binary, `1 city per side` measured 97.5% at 40 games and 81.2% at 480 — the
#: 40-game interval (87.1-99.6) does not overlap the 480-game one (77.5-84.5).
#: A baseline written from that reading made two days of ordinary merges look
#: like a 21.7-point regression in a shipped product. At 120 the interval is
#: about +-7 points, which is narrow enough for the comparison this battery
#: exists to make.
DEFAULT_GAMES = 120

#: A baseline is what every later run is compared against, so it may not be
#: written from a spot check. `--games` below this is fine for iterating; it is
#: not fine for `--write-baseline`.
MIN_BASELINE_GAMES = 120

# The arena the battery is fought on. Fixed rather than configurable: a
# benchmark whose board moves is not a benchmark, and the numbers in
# `docs/TACTICS_BASELINE.md` are only comparable because this does not change.
WIDTH, HEIGHT, PLAYERS = 20, 20, 2

# The economy the baseline was recorded under, pinned for the same reason. The
# stock arena moved on 2026-08-15 to two standing armies with no reinforcements
# (0 Production, 0 Gold, a 250-turn clock); the battery keeps the arena its
# recorded figures were fought on — one city producing 30 a turn, 30 Gold a turn
# for upgrades, a technology every five turns and a 100-turn clock — so a
# regime's row still means what it did. To re-baseline on the new stock arena,
# change these and `--write-baseline` in the same pull request.
ECONOMY = (
    "--tactics-production", "30",
    "--tactics-gold", "30",
    "--tactics-turns-per-tech", "5",
    "--tactics-turn-limit", "100",
)


@dataclass(frozen=True)
class Regime:
    """One arena setup the battery is fought under."""

    key: str
    title: str
    why: str
    flags: tuple[str, ...]


# Two regimes because the measured difference between them is the whole point:
# with a city the objective stands still, without one it walks away, and the
# advanced controller's results invert completely between the two.
REGIMES = (
    Regime(
        key="capture",
        title="1 city per side",
        why="a static objective: the battle is decided by taking the enemy city",
        flags=(),
    ),
    Regime(
        key="attrition",
        title="no cities",
        why="pure combat: the objective is the enemy army, and it moves",
        flags=("--tactics-cities", "0"),
    ),
    Regime(
        key="attrition-eras",
        title="no cities, random era",
        why="pure combat across the whole unit roster rather than one era's",
        flags=("--tactics-cities", "0", "--start-era", "random"),
    ),
)

# `basic` first: it is the informative opponent, and the one whose column is
# expected to move when tactical play changes.
MATCHUPS = (("advanced", "basic"), ("advanced", "advanced_v1"))

# The tournament prints one standardized line per rated controller. The
# pair-score is the order-independent figure with a Wilson interval, which is
# what a comparison should quote — the online Elo above it is order-sensitive.
PAIR_LINE = re.compile(
    r"^\s{2}(?P<name>\S+)\s+[\d.]+\s+\(95%.*?\)\s+pair-score=\s*(?P<score>[\d.]+)/(?P<games>\d+)\s+"
    r"\((?P<pct>[\d.]+)%,\s*95%\s*(?P<lo>[\d.]+)\.\.(?P<hi>[\d.]+)%\)"
)
# When neither controller is the ledger anchor the standardized block is absent,
# so fall back to the leaderboard's own win counts.
WIN_LINE = re.compile(r"^\s{2}(?P<name>\S+)\s+[\d.]+\s+games=(?P<games>\d+)\s+wins=(?P<wins>\d+)")


@dataclass
class Result:
    regime: str
    left: str
    right: str
    wins: float
    games: int
    pct: float
    lo: float | None
    hi: float | None

    @property
    def label(self) -> str:
        return f"{self.left} vs {self.right}"

    def cell(self) -> str:
        band = f" ({self.lo:.1f}–{self.hi:.1f})" if self.lo is not None else ""
        return f"{self.pct:.1f}%{band}"


def binary(explicit: str | None) -> str:
    """The civvis binary to benchmark with.

    A benchmark run against a stale build measures the wrong code, and the
    profile the engine prints cannot catch that — so this refuses to guess
    between builds and says which it wants.
    """
    if explicit:
        return explicit
    for profile in ("ci", "release", "debug"):
        candidate = REPO / "target" / profile / "civvis"
        if candidate.exists():
            return str(candidate)
    sys.exit(
        "no civvis binary found; build one first:\n"
        "    cargo build --profile ci --locked\n"
        "or name it with --binary"
    )


def run_match(exe: str, regime: Regime, left: str, right: str, games: int, ratings: Path) -> Result:
    command = [
        exe, "tournament",
        "--map", "battlefield", "--shape", "flat",
        "--players", str(PLAYERS), "--width", str(WIDTH), "--height", str(HEIGHT),
        "--games", str(games),
        "--ais", f"{left},{right}",
        "--ratings", str(ratings),
        *ECONOMY,
        *regime.flags,
    ]
    finished = subprocess.run(command, capture_output=True, text=True, cwd=REPO)
    if finished.returncode != 0:
        sys.exit(f"tournament failed for {regime.key} {left} vs {right}:\n{finished.stderr[-2000:]}")

    for line in finished.stdout.splitlines():
        found = PAIR_LINE.match(line)
        if found and found["name"] == left:
            return Result(
                regime.key, left, right,
                float(found["score"]), int(found["games"]), float(found["pct"]),
                float(found["lo"]), float(found["hi"]),
            )
    # No standardized block: read the leaderboard instead. Reported without an
    # interval, because an order-sensitive win count does not carry one.
    for line in finished.stdout.splitlines():
        found = WIN_LINE.match(line)
        if found and found["name"] == left:
            wins, total = int(found["wins"]), int(found["games"])
            pct = 100.0 * wins / total if total else 0.0
            return Result(regime.key, left, right, wins, total, pct, None, None)
    sys.exit(f"could not read a result for {left} from:\n{finished.stdout[-2000:]}")


def table(results: list[Result]) -> str:
    matchups = [f"{left} vs {right}" for left, right in MATCHUPS]
    lines = ["| regime | " + " | ".join(matchups) + " |",
             "| --- | " + " | ".join("---" for _ in matchups) + " |"]
    for regime in REGIMES:
        cells = []
        for left, right in MATCHUPS:
            hit = next(
                (r for r in results if r.regime == regime.key and r.left == left and r.right == right),
                None,
            )
            cells.append(hit.cell() if hit else "—")
        lines.append(f"| {regime.title} | " + " | ".join(cells) + " |")
    return "\n".join(lines)


def parse_baseline(text: str) -> dict[tuple[str, str, str], float]:
    """The recorded figures, keyed by regime and matchup.

    Read back out of the committed document rather than a sidecar file, so
    there is one copy of the numbers and it is the one a person reads.
    """
    recorded: dict[tuple[str, str, str], float] = {}
    for line in text.splitlines():
        marker = "<!-- bench:"
        if not line.startswith(marker):
            continue
        payload = json.loads(line[len(marker):line.rindex("-->")].strip())
        recorded[(payload["regime"], payload["left"], payload["right"])] = payload["pct"]
    return recorded


def revision() -> tuple[str, str]:
    """The commit this benchmark is measuring, and its date. `("", "")` if unknown."""
    try:
        sha = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"],
                             capture_output=True, text=True, check=True).stdout.strip()
        when = subprocess.run(["git", "-C", str(REPO), "show", "-s", "--format=%cI", sha],
                              capture_output=True, text=True, check=True).stdout.strip()
        return sha, when
    except (subprocess.SubprocessError, OSError):
        return "", ""


def commits_since(sha: str) -> int | None:
    """How many commits have landed since `sha`. `None` when it cannot be told."""
    if not sha:
        return None
    try:
        out = subprocess.run(["git", "-C", str(REPO), "rev-list", "--count", f"{sha}..HEAD"],
                             capture_output=True, text=True, check=True)
        return int(out.stdout.strip())
    except (subprocess.SubprocessError, OSError, ValueError):
        return None


def baseline_provenance(text: str) -> dict:
    """The `<!-- measured: {..} -->` stamp, or `{}` on a baseline written before it."""
    for line in text.splitlines():
        marker = "<!-- measured: "
        if line.startswith(marker) and line.endswith(" -->"):
            try:
                return json.loads(line[len(marker):-len(" -->")])
            except ValueError:
                return {}
    return {}


def staleness_note(text: str) -> str:
    """One line saying how far the code has moved since the baseline was taken.

    ⚠⚠⚠ A BASELINE WITH NO AGE ON IT READS AS CURRENT, AND THIS ONE WAS NOT.
    The table committed on 2026-08-15 sat unrefreshed while the controller
    moved under it, and nothing in the file said so.

    ⚠⚠⚠ AND THEN SAMPLE SIZE ATE THE FIRST CONCLUSION. The re-measurement on
    2026-08-17 called `1 city per side` a **21.7-point regression** — 97.5%
    recorded against 75.8% re-measured — and said it "holds at 120 games, so it
    is not sample noise". That was wrong in exactly the way this file warns
    about: the two numbers came from DIFFERENT SAMPLE SIZES. Only the new one
    was 120 games; the recorded 97.5% was 40.

    Rebuilding the 2026-08-15 commit and measuring it properly settles it:

        capture, advanced vs basic          n     pct    95% CI
        recorded at 7cd011bb               40   97.5%   87.1-99.6
        re-measured at 7cd011bb           120   81.7%   73.8-87.6
        re-measured at 7cd011bb           480   81.2%   77.5-84.5
        measured at 04d9c805              480   77.3%   73.3-80.8

    **The same binary measures 81.2% at 480 games and 97.5% at 40**, and the
    recorded figure's interval does not overlap its own binary's 480-game
    interval. About sixteen of the twenty-two points were never there. What is
    left, 81.2% -> 77.3%, is **p = 0.136 — no established difference** — and
    the same pair against the FROZEN anchor moved the other way, 58.8% ->
    64.4%, p = 0.074. Two columns pointing in opposite directions with neither
    significant is the signature of noise. **There was no capture regression.**

    ★★★ WHAT THE STALE BASELINE WAS ACTUALLY HIDING WAS A LARGE IMPROVEMENT.
    #1858 routed the bounded joint-tactics search through the arena movement
    seam. Measured across its own parent, 240 seat-mirrored games per matchup:

        no cities, advanced vs basic          60.4% -> 87.9%   +27.5  p=6e-12
        no cities, advanced vs advanced_v1    92.9% -> 99.6%   + 6.7  p=1e-4

    That is ROADMAP objective 4 delivering, and it went uncredited for two days
    for the same reason the phantom regression went unchallenged: nothing said
    how old the table was.

    Running 1,440 rated games in CI is still not the answer. Saying how old the
    number is, is — and refusing to write a baseline from a sample too small to
    support one. See `MIN_BASELINE_GAMES`.
    """
    stamp = baseline_provenance(text)
    sha, when = stamp.get("commit", ""), stamp.get("date", "")
    if not sha:
        return ("this baseline predates revision stamping, so how far the code "
                "has moved since is unknown — re-run with --write-baseline")
    behind = commits_since(sha)
    where = f"{sha[:9]}" + (f" ({when[:10]})" if when else "")
    if behind is None:
        return f"baseline measured at {where}; this checkout cannot count commits since"
    if behind == 0:
        return f"baseline measured at {where}, which is this revision"
    return f"baseline measured at {where} — {behind} commit(s) ago"


def render_baseline(results: list[Result], games: int) -> str:
    sha, when = revision()
    stamp = "<!-- measured: " + json.dumps({
        "commit": sha, "date": when, "games": games,
    }) + " -->"
    measured = (f"Measured on `{sha[:9]}`" + (f" ({when[:10]})" if when else "")
                if sha else "Measured on an unknown revision")
    machine = "\n".join(
        "<!-- bench: " + json.dumps({
            "regime": r.regime, "left": r.left, "right": r.right, "pct": r.pct,
        }) + " -->"
        for r in results
    )
    regimes = "\n".join(f"- **{r.title}** — {r.why}" for r in REGIMES)
    return f"""# Tactics arena baseline

What the shipped controllers do on the arena, so a change to tactical AI can be
answered with a number. Regenerate with `tools/tactics_bench.py --write-baseline`
and quote the diff in the pull request that moves it.

**{measured}, {games} seat-mirrored games per matchup.** These figures describe
that revision and no other. `tactics_bench.py` prints how many commits have
landed since, because a table with no age on it reads as current.

⚠ **Compare like with like.** A row here is only comparable to a row measured
at the same sample size. On 2026-08-17 a 40-game reading of `1 city per side`
(97.5%) was compared against a 120-game one (75.8%) and reported as a
21.7-point regression; rebuilding the older commit and measuring it at 480
games gave **81.2%**, so about sixteen of those points were never there and the
remainder is not statistically distinguishable (p = 0.136). `--write-baseline`
now refuses fewer than 120 games for exactly that reason.

Every figure is the left controller's share of {games} seat-mirrored games on a
{WIDTH}x{HEIGHT} arena, with a 95% Wilson interval. Seat-mirrored means each
controller plays both ends of every draw, so a starting-corner advantage cannot
read as a controller advantage. The arena economy is pinned by the battery
(`ECONOMY` in `tools/tactics_bench.py`) rather than taken from the stock arena,
so the rows stay comparable when the stock arena moves.

## Regimes

{regimes}

## Opponents

`basic` is the informative opponent. `advanced_v1` is a frozen copy of the live
controller, and nearly everything separating them is empire-level machinery an
arena never exercises — so that column sits near 50% whatever the tactical AI
does. **A near-50% result against `advanced_v1` is the expected null, not a
finding.** Read the `basic` column.

## Results

{table(results)}

{stamp}
{machine}
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--games", type=int, default=DEFAULT_GAMES,
                        help=f"seat-mirrored games per matchup (default {DEFAULT_GAMES})")
    parser.add_argument("--binary", help="civvis binary to benchmark (default: newest built)")
    parser.add_argument("--only", choices=[r.key for r in REGIMES],
                        help="run one regime while iterating")
    parser.add_argument("--write-baseline", action="store_true",
                        help="record these results as the committed baseline")
    args = parser.parse_args()

    # ⚠ REFUSE BEFORE PLAYING ANYTHING. The first version of this guard sat
    # beside the write, so a `--write-baseline --games 40` spent a full battery
    # and then declined to keep it. Argument errors belong before the work.
    if args.write_baseline and args.games < MIN_BASELINE_GAMES:
        sys.exit(
            f"refusing to write a baseline from {args.games} games. A baseline "
            f"is what every later run is compared against, and {args.games} "
            f"games is roughly a +-{int(98 / (args.games ** 0.5))} point "
            f"instrument: at 40, `1 city per side` read 97.5% on a binary that "
            f"measures 81.2% at 480 games, and the gap was reported as a "
            f"regression that did not exist. Use --games {MIN_BASELINE_GAMES} "
            f"or more."
        )

    exe = binary(args.binary)
    regimes = [r for r in REGIMES if not args.only or r.key == args.only]

    results: list[Result] = []
    # One scratch directory for the whole run, removed after: rated evidence
    # from a benchmark is not evidence about the shipped ladder.
    scratch = Path(tempfile.mkdtemp(prefix="tactics-bench-"))
    try:
        for regime in regimes:
            for left, right in MATCHUPS:
                ratings = scratch / f"{regime.key}-{left}-{right}.json"
                print(f"… {regime.title}: {left} vs {right}", file=sys.stderr, flush=True)
                results.append(run_match(exe, regime, left, right, args.games, ratings))
    finally:
        shutil.rmtree(scratch, ignore_errors=True)

    print(table(results))

    if args.write_baseline:
        BASELINE.parent.mkdir(parents=True, exist_ok=True)
        BASELINE.write_text(render_baseline(results, args.games))
        print(f"\nbaseline written to {BASELINE.relative_to(REPO)}")
        return 0

    if not BASELINE.exists():
        print("\nno baseline recorded yet; --write-baseline records one")
        return 0

    committed = BASELINE.read_text()
    recorded = parse_baseline(committed)
    # ⚠ THE AGE FIRST, BEFORE ANY DELTA. A delta against a baseline of unknown
    # age is not a comparison between two versions of the AI; it is a
    # comparison between now and whenever somebody last remembered to run this.
    print(f"\n{staleness_note(committed)}")
    print("\nagainst the baseline:")
    worst = 0.0
    for result in results:
        was = recorded.get((result.regime, result.left, result.right))
        if was is None:
            print(f"  {result.regime:16} {result.label:26} {result.pct:5.1f}%  (not in the baseline)")
            continue
        delta = result.pct - was
        worst = min(worst, delta)
        arrow = "+" if delta >= 0 else ""
        print(f"  {result.regime:16} {result.label:26} {result.pct:5.1f}%  {arrow}{delta:.1f} vs {was:.1f}%")
    # Reported, never enforced: these intervals are wide at forty games, so a
    # threshold here would fail honest runs and teach people to ignore it.
    print(f"\nlargest regression: {worst:.1f} points")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
