#!/usr/bin/env python3
"""The ledger as a table: what each build did, and where the two disagreed."""

import json
import pathlib
import sys

ledger = (pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else
          pathlib.Path.home() / "civvis-simloop-logs" / "ledger.jsonl")
rows = []
if ledger.exists():
    for line in ledger.read_text(errors="replace").splitlines():
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            pass

if not rows:
    print("no iterations recorded yet")
    raise SystemExit(0)

head = f"{'#':>3} {'arm':<5} {'seed':>5} {'sha':<8} {'build':>6} {'sim':>5} {'turn':>5} {'t/s':>6} {'win':>4} {'victory':<10}"
print(head)
print("-" * len(head))
for row in rows:
    print(
        f"{row['iteration']:>3} {row['arm']:<5} {row['seed']:>5} {row['sha']:<8} "
        f"{row.get('build_seconds', 0):>5}s {row.get('sim_seconds', 0):>4}s "
        f"{str(row.get('turn', '-')):>5} {str(row.get('turns_per_second', '-')):>6} "
        f"{str(row.get('winner', '-')):>4} {str(row.get('victory') or '-'):<10}"
        + ("  FAILED" if not row.get("ok") else "")
    )

# Pair on the same key `record.py` uses — seed *and* revision *and* board.
# Keyed on the seed alone this reported seed 1009 as a divergence, which the
# recorder had correctly declined to pair: its two halves ran either side of a
# config change, so they are two different games and not two builds of one.
pairs = {}
for row in rows:
    if row.get("ok"):
        pairs.setdefault((row["seed"], row.get("sha"), row.get("config", "")), {})[row["arm"]] = row
matched = [k for k, arms in pairs.items() if len(arms) == 2]
# A pair whose divergence claim was explicitly retracted stays retracted. This
# is recomputed from the raw rows on purpose — that independence is what caught
# a bad claim once — but recomputing straight over a written-down retraction
# would just reinstate it every run, and the reason it was retracted is not in
# the scores.
diverged = [
    k for k in matched
    if (pairs[k]["rust"].get("scores") != pairs[k]["wasm"].get("scores")
        or pairs[k]["rust"].get("turn") != pairs[k]["wasm"].get("turn"))
    and not any(pairs[k][arm].get("retracted_divergence") for arm in ("rust", "wasm"))
]
unpaired = [k for k, arms in pairs.items() if len(arms) == 1]
print()
full = [k for k in matched
        if pairs[k]["rust"].get("seat_strategy") == pairs[k]["wasm"].get("seat_strategy")]
maponly = [k for k in matched if k not in full]
fulldiv = [k for k in diverged if k in full]
print(f"parity: {len(full) - len(fulldiv)}/{len(full)} pairs played identically by both builds")
if maponly:
    mapdiv = [k for k in maponly
              if pairs[k]["rust"].get("map_digest") != pairs[k]["wasm"].get("map_digest")]
    print(f"        {len(maponly) - len(mapdiv)}/{len(maponly)} further pairs matched on MAP ONLY "
          f"(different agents seated since #1094 — the game itself is not comparable)")
pub = [r for r in rows if r.get("publish")]
if pub:
    good = [r for r in pub if r["publish"] == "ok"]
    last = pub[-1]
    size = (f", last {last['publish_bytes']:,} bytes "
            f"({last.get('publish_pct_of_budget')}% of budget)" if last.get("publish_bytes") else "")
    print(f"site build: {len(good)}/{len(pub)} revisions still assemble{size}")
sl = [r for r in rows if r.get("saveload")]
if sl:
    line = (f"save/load: {sum(1 for r in sl if r['saveload'] == 'ok')}/{len(sl)} "
            f"saved worlds came back identical")
    cross = [r for r in sl if r.get("cross_build_save")]
    if cross:
        good = sum(1 for r in cross if r["cross_build_save"] == "ok")
        line += f"; {good}/{len(cross)} loaded across builds"
    print(line)
checked = [r for r in rows if "self_consistent" in r]
if checked:
    bad = [r for r in checked if not r["self_consistent"]]
    mem = [r for r in rows if r.get("peak_wasm_mib")]
    if mem:
        print(f"wasm memory: {max(r['peak_wasm_mib'] for r in mem):.0f} MiB peak over "
              f"{len(mem)} runs (wasm32 can address 4096)")
    print(f"repeatability: {len(checked) - len(bad)}/{len(checked)} builds reproduced themselves "
          f"on a replayed seed" + ("" if not bad else "  ⚠ SEE STANDING"))
# By board, not one line per occurrence. The globe's benchmark seed replays
# every lap, so a per-occurrence list grows for ever while saying the same
# thing — and the point of this line is to make a *new* board appearing here
# impossible to miss.
if fulldiv:
    per_board = {}
    for seed, sha, config in fulldiv:
        per_board.setdefault(config or "no config", []).append((seed, sha))
    # Say whether it is CURRENT. This line counts every divergence ever seen on
    # a board, which is the right total — but printed bare it reads as a live
    # problem long after a fix has landed, and the section below already knows
    # better. The two have to agree or the header quietly contradicts the body.
    latest_sha = rows[-1].get("sha")
    for board, hits in sorted(per_board.items()):
        seeds = sorted({s for s, _ in hits})
        shas = {sha for _, sha in hits}
        recent = [k for k in matched
                  if (k[2] or "no config") == board and k[1] == latest_sha]
        still = any(k in diverged for k in recent)
        mark = "⚠ still diverging" if still else "✓ not since the fix"
        print(f"  {mark} — {board}: {len(hits)} divergence(s) over its history, "
              f"{len(seeds)} seed(s), {len(shas)} revision(s)")
if unpaired:
    print(f"  ({len(unpaired)} arm(s) with no matching partner — a config or revision changed mid-pair)")


# Per board, because a turn on ten-major `crowded` is not a turn on six-major
# `baseline` and one best-of-everything number compares neither.
#
# In CPU seconds where the row has them. The wall clock on this box says as much
# about what else was running as about the engine: one seed produced the
# identical game twice and read 4.294 turns/s at load 6 and 2.972 at load 24.
boards = sorted({r.get("config") or "(pre-rotation)" for r in rows if r.get("turns_per_second")})
metric = "turns_per_cpu_second" if any(r.get("turns_per_cpu_second") for r in rows) else "turns_per_second"
unit = "t/cpu-s" if metric == "turns_per_cpu_second" else "t/s (wall)"
print(f"\n{'board':<22} {'rust ' + unit:>18}  {'wasm ' + unit:>18}")
for board in boards:
    cells = []
    for arm in ("rust", "wasm"):
        speeds = [
            r[metric] for r in rows
            if r["arm"] == arm and (r.get("config") or "(pre-rotation)") == board
            and r.get(metric)
        ]
        cells.append(f"{speeds[-1]:>7} (best {max(speeds)})" if speeds else "—")
    print(f"{board:<22} {cells[0]:>18}  {cells[1]:>18}")
measured = sum(1 for r in rows if r.get("turns_per_cpu_second"))
if metric == "turns_per_cpu_second" and measured < len(rows):
    print(f"  ({measured} of {len(rows)} rows carry CPU time; earlier rows predate the change)")

# How the games actually end. This is not a defect report — it is what the
# engine does, and it only became visible once the loop played more than one
# kind of board. Two things stand out and neither is per-run noise: reaching
# the turn limit with nobody winning is a property of the *board* (islands
# three quarters of the time, most boards never), and across every game played
# here domination has not been achieved once.
from collections import Counter  # noqa: E402
finished = [r for r in rows if r.get("ok") and r.get("turn")]
if finished:
    tally = Counter(r.get("victory") or "nobody won" for r in finished)
    print(f"\nhow {len(finished)} games ended")
    for kind, n in tally.most_common():
        print(f"  {kind:<14} {n:>4}  ({100 * n / len(finished):>3.0f}%)")
    never = [v for v in ("science", "culture", "religious", "diplomatic", "domination", "score")
             if v not in tally]
    if never:
        print(f"  never seen:    {', '.join(never)}")
    stalled = sorted(
        ((b, [r for r in finished if (r.get("config") or "(pre)") == b])
         for b in {r.get("config") or "(pre)" for r in finished}),
        key=lambda kv: -sum(1 for r in kv[1] if r.get("winner") is None) / len(kv[1]),
    )
    worst = [(b, g) for b, g in stalled if any(r.get("winner") is None for r in g)]
    if worst:
        print("  reached the limit with no winner:")
        for board, games in worst:
            n = sum(1 for r in games if r.get("winner") is None)
            print(f"    {board:<20} {n:>3}/{len(games):<3} ({100 * n / len(games):>3.0f}%)")

notes = [(r["iteration"], r["arm"], n) for r in rows for n in r.get("notes", [])]
# A civilization eliminated and a warning count are the loop's weather; a
# divergence or a dead module is the loop finding something.
#
# Two tiers, because they age differently. A divergence or a dead module is a
# standing fact about the program and stays until somebody deals with it; a
# throughput reading is about one game and stops being interesting once the
# next few have run. Keeping both forever buried the one real find under six
# lines of noise.
FOREVER = ("DIVERGED", "died", "refused", "DID NOT REPRODUCE",
           "NO LONGER BUILDS", "DID NOT COME BACK", "DID NOT LOAD THE SAME")
UNPAIRED = ("NOT COMPARED",)
PASSING = ("throughput", "nobody won")
recent = max((r["iteration"] for r in rows), default=0) - 8

standing = [n for n in notes if any(k in n[2] for k in FOREVER)
            and not any(k in n[2] for k in UNPAIRED)]
passing = [n for n in notes if any(k in n[2] for k in PASSING) and n[0] > recent]

if standing:
    # Grouped by what the finding *is*, not by how many times it has been seen.
    # A benchmark seed replays a known-bad board every lap, so an ungrouped list
    # gains a line each time the loop re-confirms something already known — and
    # a list that grows without new information is one a new finding disappears
    # into. Re-confirmations are worth a count, not a paragraph each.
    groups = {}
    for iteration, arm, note in standing:
        row = next(r for r in rows if r["iteration"] == iteration)
        key = (row.get("config") or "?", note.split(":", 1)[-1].split(";")[0].strip())
        groups.setdefault(key, []).append((iteration, arm, row.get("seed")))
    # Split on whether it is still happening — and decide that by asking
    # whether the check has since PASSED on the same board, not by how long ago
    # it last failed. A time window is a guess; "this exact check ran again on
    # this exact board and was clean" is the fact. It also reads correctly the
    # moment a fix lands, instead of leaving a resolved finding sitting under
    # "still happening" for another fifty iterations.
    #
    # The window survives only as the fallback for findings with no obvious
    # later check to point at.
    latest = max(r["iteration"] for r in rows)
    QUIET = 50

    def cleared_since(board, since):
        """Did the MOST RECENT run of this check on this board pass?

        Not "has it never failed since" — that is order-sensitive and put two
        findings on one board into different tiers, because one was last seen on
        the rust arm and the other on the wasm arm one iteration later. What a
        person means by "is it fixed" is whether the latest attempt was clean.
        """
        later = [r for r in rows
                 if r["iteration"] > since and (r.get("config") or "?") == board
                 and r.get("saveload")]
        return bool(later) and later[-1]["saveload"] == "ok"

    live, quiet = {}, {}
    for k, v in groups.items():
        board, what = k
        last = v[-1][0]
        is_saveload = "cities changed" in what or "differs (cities)" in what
        resolved = (cleared_since(board, last) if is_saveload
                    else latest - last > QUIET)
        (quiet if resolved else live)[k] = v

    def show(title, chosen, suffix=""):
        if not chosen:
            return
        print(f"\n{title}")
        for (board, what), hits in chosen.items():
            seeds = sorted({h[2] for h in hits})
            first, last = hits[0][0], hits[-1][0]
            when = f"#{first}" if first == last else f"#{first}-#{last}, {len(hits)}x"
            print(f"  [{board}] {what}")
            print(f"      seed(s) {', '.join(str(s) for s in seeds)} · {when}{suffix}")

    show("standing — the loop found these and they are still happening:", live)
    show("resolved — the same check has since run clean on that board:", quiet,
         f" · nothing since, now at #{latest}")

# Not findings, but not nothing either: a row the loop reported and later took
# back. Kept visible so a retraction is a thing that happened rather than a
# thing that quietly stopped being displayed.
for field, label in (("retracted_divergence", "divergence"),
                     ("retracted_throughput_claim", "throughput regression")):
    taken_back = [r for r in rows if r.get(field)]
    if taken_back:
        span = (f"#{taken_back[0]['iteration']}-#{taken_back[-1]['iteration']}"
                if len(taken_back) > 1 else f"#{taken_back[0]['iteration']}")
        print(f"\n{len(taken_back)} earlier {label} claim(s) retracted ({span}) — see ledger for why")
older = len([n for n in notes if any(k in n[2] for k in PASSING)]) - len(passing)
# Printed inside this section, always. Hung on the end it attached itself to
# whichever section happened to print last — and with no recent readings that
# was `standing`, where "13 older not shown" reads as thirteen more findings.
if passing or older:
    print(f"\npassing readings (since #{recent + 1}):")
    for iteration, arm, note in passing:
        print(f"  #{iteration} {arm}: {note}")
    if not passing:
        print("  none")
    if older:
        print(f"  ({older} older reading(s) before this window, not shown)")
