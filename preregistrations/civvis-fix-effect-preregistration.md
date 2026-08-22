# Pre-registration — did the 2026-08-02 fixes change the real Civ 6 games?

Written **before any run existed on the new binary**. Agent `b0edd12a`, session
`/loop` on the operator's standing question "our player score typically lags by a
lot, stats too — why?".

## Why pre-register

Five fixes merged in one session, all diagnosed from the same corpus they will now
be measured on. That is the exact shape that produces a flattering read
(`civvis-effect-sizes-are-winners-curse`, `civvis-measurement-discipline`). Writing
the baseline and the predictions down first is the cheapest guard available.

## ⚠⚠ BOUNDARY RESET 2026-08-02 08:41 local — READ THIS FIRST

The 02:08 split below was **never the full stack** and its "after" group is void.
The batch tree sat at #868 for hours, briefly reached #880, and was reverted to #868
and rebuilt — so **#731, #867 and #877 were in no game measured under it**. Those runs
are re-grouped with BEFORE, which is the honest place for them.

The new cut is the start of batch `civvis-20260802T124143Z`, pinned to `df475b9`:
the first with all of #731 / #867 / #877 / #882 in the binary **and** `--victory
civvis` instead of the dominated `score` lane. Operator approved the restart.

**New baseline, 8 partial-stack runs:**

```
MEAN   peak cities 3.6   end 1.9   score 190   turns broke 32.0   impMax 71.1
```

Predictions for the full stack, in the order I expect them to be readable:

| column | prediction | why |
|---|---|---|
| `impMax` | falls to **1–2** | #731's deferral stops the wrong-tile improve loop |
| `broke` | falls | #867 lets CIVVIS's trader/economy response actually reach the game |
| `end` cities | rises toward `cit` | fewer bankruptcies means fewer disbanded armies |
| `cit`, `score` | **do not read** | between-run SD swamps any effect at this n |

⚠ The `rec` column is still meaningless — see the correction below.

## The split (SUPERSEDED, kept for the record)

Runs are grouped by whether their `events.jsonl` was written after the mtime of the
deployed decider, `/Users/martin/civvis-batch-runner/target/release/civvis_orders`,
built **2026-08-02T02:08 local** — which postdates every merge below. Runs with
fewer than 20 turn records are dropped (they are launcher failures, not games).

Re-run with `scratchpad/fix_effect.py`.

## The fixes and what each should move

| PR | fix | column | prediction |
|---|---|---|---|
| #848 | settler dodge livelock — stall counter measured motion, not progress | `life` | **no change expected**; already measured NULL on peak cities at 576 sim seats |
| #851 | `gold_per_turn` never written by the bridge | `rec`, `broke` | `rec` **> 0** at all; `broke` should fall |
| #853 | improve refusal named the builder's tile, not the refused one | `impMax` | should fall toward 1–2 |
| #840 | settler escorts (another agent's PR) | `cap` | should fall |

## The baseline — 10 runs, all before the binary

```
run                           T cit end score rival broke rec setl  life cap impMax
civvis-20260801T232456Z     241   5   5   334   992    89   0   11  10.8   1      1
civvis-20260802T014139Z     250   9   6   394  1248    71   0   16  10.9   3      2
civvis-20260802T021923Z     250   8   5   557  1010    25   0    8  14.6   0      2
civvis-20260802T030105Z      62   1   1    62   160     0   0    4   5.5   1      1
civvis-20260802T030910Z     222   3   2   144   873     0   0   13   3.8   5      2
civvis-20260802T033552Z     209   6   5   402  1041    96   0    7   7.1   0      2
civvis-20260802T041527Z     250   2   2   163  1095     0   0   14   5.4   5    118
civvis-20260802T044726Z     250   3   1   177  1059   119   0    6  17.3   2     35
civvis-20260802T052223Z      45   2   2    66   102     0   0    3   7.7   0      0
civvis-20260802T053109Z     182   3   1   165   772    71   0    7  29.6   2     85
MEAN                            4.2 3.0   246        47.1 0.0  8.9  11.3 1.9   24.8
```

`cit` peak cities · `end` cities at the end · `broke` turns at zero gold ·
`rec` recovery-reason builds · `setl` settlers built · `life` mean settler sightings ·
`cap` settlers whose last sighting had a hostile **adjacent** · `impMax` most repeats
of a single refused (unit, improvement, tile).

## ⚠⚠ CORRECTION 2026-08-02 — the `rec` column is BROKEN, ignore it

`rec` counted `build` events whose `reason` is `recovery`/`economic_recovery`/`upkeep`.
**No such reason exists.** The `reason` vocabulary belongs to the mod's own Lua ladder
— `civvis, floor, army, ranged, expand, develop, grow, siege, improve, scout, defend`
— and a build CIVVIS chose is tagged simply `civvis`. So `rec` could only ever read 0
and its "conclusive" reading below is worthless.

The recovery plan **is** running. `why.log` on the first after-run shows
`Grand strategy: recovery` and `Researching banking | worth 14 to the recovery plan`.

⚠ The right check for #851 is the decider's `why.log`, not the `build` reason field.

## ★★★★ The one number that is already conclusive

**`rec = 0` in all ten runs.** `economic_recovery` never fired once, across ~2000
turns and 471 turns of zero treasury. That is not a weak signal about #851's
diagnosis — it is the diagnosis, measured. If `rec` stays at 0 after the fix, the
fix did not reach the deployment and something else is wrong.

## What would count as a null

- `rec` still 0 → #851 did not reach the game
- `impMax` still above ~10 on any run → #853 did not reach the game (it is a **mod**
  change and needs a relaunch, so check the installed Lua too)
- `cap` unchanged → #840's escorts are not firing

## ⚠ What this design cannot do

- **n is tiny and grows slowly** — one run is ~30 minutes. Peak cities has a
  between-run SD of about 2.5 against a mean of 4.2, so `cit` and `score` will not
  separate at any n this loop will reach. **Do not read them.** The mechanism columns
  (`rec`, `impMax`, `cap`) are binary-ish and will separate; the outcome columns
  will not.
- The arms are **not** contemporaneous. Machine load, other agents' merges, and the
  operator's own changes all vary between the groups.
- Five fixes landed together, so nothing here attributes an outcome change to one of
  them. Only the per-fix mechanism columns are attributable.
- ⚠ If **PR #859** (the `--victory` flip) merges, the groups stop being comparable at
  all and this pre-registration is void from that point. Note the boundary run.
