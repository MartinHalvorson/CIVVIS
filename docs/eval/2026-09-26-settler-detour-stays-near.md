# Settler detour stays near — ladder-proxy evaluation (2026-09-26)

Gene: `settler-detour-stays-near` (opt-in, PR #3785). Instrument: the in-engine
ladder proxy (`examples/ladder_proxy.rs`, private bench; 1 unhandicapped
focal seat on the live Science lane vs 3 handicapped adaptive rivals, Tiny
Pangaea, 6 city-states), built from `main` be1492357. Paired by seed;
`share` = focal's end-score share of all four seats.

## The defect

Settler build→found time on the proxy is 10.6–14 turns at King and Immortal
(median 8, p90 22). A per-settler timeline over `TRACE_DETAIL=1` traces
(`settler_timeline.py`) shows the settlers are *not* holding: they stand
still for only 9–13% of their lives. They are walking.

The walk is `detour_settler_around_visible_threat`, on for the live seat
through `live_settler_capture_lessons && settlement_safety`. When a visible
hostile makes the next route step unsafe it defers the approach for six
standard turns empire-wide and hands the Settler the best SAFE site anywhere
within `best_settler_target`'s radius 8. Over 18 detours in two traced
Immortal games (seeds 61000000 and 61000004) the fallback was **farther**
than the deferred site in 11 — 4 tiles became 9, 2 became 9, 3 became 8,
8 became 13 — and usually on the other side of the empire. Each far walk then
met its own blocker and detoured again. Settler #353 (seed 61000004) was
born four tiles from its site and walked 27 turns criss-crossing the empire;
#553 walked 27, #440 20, #178 17. In the full-length base trace of seed
61000002 four settlers averaged 37 turns each (one walked 80).

## The gene

With `settler-detour-stays-near` on, the detour's fallback search is bounded
to `clamp(wdist(settler, deferred site) + 1, 3, 8)` hexes
(`best_reachable_settle_site_except_cached` at that radius, deferral scratch
retries unchanged). With no safe site inside that radius the original
target is kept and the deferral rolled back — exactly the existing
no-safe-alternate path — so the Settler holds with its guard for the
blocker's short clock instead of walking. The value floor stays with
`detour-keeps-the-site-worth`, which stacks. Gene off is byte-identical to
`main` on 3 seeds (all fields but `secs`).

## Immortal, Science lane, seeds 61000000–31, 32 paired

| | base | gene | Δ |
|---|---|---|---|
| focal share | 0.112 | 0.113 | **+0.19 pp ± 0.36 (z +0.52)** |
| focal alive at end | 30 | 32 | +2 |
| wins | 0 | 0 | — |
| settlers founded (total) | 234 | 241 | +7 |
| settlers lost (total) | 4 | 3 | −1 |
| settler life mean / p75 / p90 / max (turns) | 11.5 / 15 / 23 / 102 | 10.2 / 13 / 19 / 46 | −1.3 mean |
| excess settler turns (life−3) per game | 62.7 | 55.3 | −7.4 |
| cities t30 / t60 / t100 / end | 2.41 / 4.06 / 6.28 / 7.34 | 2.47 / 4.22 / 6.44 / 7.66 | +0.06 / +0.16 / +0.16 / +0.32 |
| pop t100 | 36.0 | 36.2 | +0.2 |
| science t100 | 56.3 | 56.4 | +0.1 |

Rivals (never given the gene) are unchanged within noise: settler life
11.1 → 11.4, lost 4 → 6 over 96 seat-games.

The mechanism does what it says — the tail of settler lives is gone and
more settlers found — but at Immortal the score effect is inside the noise,
as every single gene has been on this rung. Settlers lost is shown per the
operator rule (every live capture became a lesson): 4 → 3.

## King, Science lane, seeds 37140000–63, 64 paired

| | base | gene | Δ |
|---|---|---|---|
| focal share | 0.180 | 0.177 | **−0.28 pp ± 0.51 (z −0.54)** |
| focal wins | 2 | 3 | +1 (discordant +1/−0) |
| focal alive at end | 61 | 64 | +3 |
| settlers founded (total) | 475 | 506 | +31 |
| settlers lost (total) | 4 | 6 | +2 |
| settler life mean / p90 / max (turns) | 10.9 / 24 / 94 | 10.1 / 19 / 74 | −0.8 mean |
| cities t30 / t60 / t100 / end | 2.14 / 4.09 / 6.14 / 8.36 | 2.20 / 4.28 / 6.44 / 8.52 | +0.06 / +0.19 / +0.30 / +0.16 |
| pop t100 | 37.3 | 38.7 | +1.3 |
| science t100 | 64.0 | 67.2 | +3.2 |
| war turns | 27.2 | 27.1 | — |

The first 32 seeds read −0.66 pp (z −0.98) and the second 32 read +0.10 pp
(z +0.13): the early game moves the right way on both halves (more settlers
founded, more cities and science at t100) and the end-score share does not
follow, because the paired differences are dominated by late-game conquest
swings (single seeds move ±8 pp when the focal ends with 1 city instead of
9, or 7 instead of 0), which the settler walk does not decide.

## A second rule tried and not shipped: `settler-detour-limit`

Under the near rule a site whose approach stays blocked (a camp on the
road) still comes back the moment its six-turn deferral lapses; seed
61000011's settler #468 oscillated 43 turns between a 174-worth site ten
tiles south-west and 60–90-worth sites north-east (17 target switches). A
bench-only gene that retires a site after two detours away from it, the way
`SETTLER_RETREAT_LIMIT` retires a site after three retreats, cut that
settler to 28 turns — and over the same 32 Immortal seeds it changed exactly
one game (61000011), where the focal then ended with 0 cities instead of 6
(share 0.075 → 0.041). One game is not a measurement, but a rule that fires
once in 32 games and loses that game is not worth a registry row. Not
included in this PR; the code and traces are in the author's bench.

⚠ For the next person: the proxy's `--on` takes ONE comma-separated value;
a second `--on` is silently ignored. The first arm of this rule ran with the
rule off for an hour.

## Verdict

The defect is real and the rule removes it: over 96 paired games on two
rungs the settler tail is gone (max life 102 → 46 at Immortal, 94 → 74 at
King, p90 23 → 19 and 24 → 19), 38 more settlers found cities, and the focal
survives to the end in 96/96 games instead of 91/96. The end-score share is
neutral within the noise on both rungs (+0.19 pp z +0.5 at Immortal, −0.28
pp z −0.5 at King), so this ships as what it is — an opt-in, off by default,
for the fleet screen to price at its own scale — and is NOT forced on the
live seat. Settlers lost, shown per the operator rule: 4 → 3 at Immortal,
4 → 6 at King (the JSONL records the loss, not its cause; not attributed
to the hold or otherwise).

Files: `~/civvis-proxy-runs/fable-c5/` (`imm-base/near`, `king-base/near`,
`king2-base/near` JSONL, traces, `settler_timeline.py`, `settler_paths.py`,
`settler_targets.py`).
