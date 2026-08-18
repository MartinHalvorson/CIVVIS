# The steal lanes, two starving chains, and the first record win

_2026-08-18 · `agent/mbp-m5-max-128/claude-2703990e`_

## What was asked

Operator goal: study the ongoing live games for where the seat lags its
rivals, and push score, science, and domination. This round covers the halves
this session measured and shipped — the steal-lane chains (#2030, #2037), the
war-fatigue clock (#2031), and the domination-lane pricing — plus the first
game played on the combined binary. The settler-window half is #2027's own
work; this session's fleet evidence for it is in that PR's comments.

## How it was measured

- **Fleet study**: the 54 completed (t≥200) live control runs of
  2026-08-16..18, per-turn `state` events (ours vs `rivals[]`), why-logs, and
  orders databases.
- **Native pricing**: two pre-registered `ai_eval` runs
  (`~/civvis-siege-progress-preregistration.md`,
  `~/civvis-domination-lane-preregistration.md`), 120 mirrored pairs each,
  4p/24×16/500, seeds 818000 / 819000, decision rules written before running.
- **Live acceptance**: run `civvis-20260818T155500Z`, the first game on the
  merged binary, checked against per-mechanism signatures.

## What it measured

**The steal lanes** (diplomatic 41 + culture 24 of 74 pre-cap losses): racing
is structurally too slow late — diplo winners reach 20–21 DVP against our
1–8; culture winners cross a third civ's domestic bar (169/124/221) that our
48–140 cannot hold. The short lever is disruption, and the two missions the
planner already priced highest (`fabricate_scandal` 330, `great_work_heist`
340) had never once crossed the bridge: **zero `SPY_*` orders in every
recorded orders database**, because a fresh live Spy owed a promotion the
host can never grant (native rule: one per level; Civ 6: first at level 2)
and `legal_spy_actions` returns promotions as the ONLY legal actions while
one is owed — spy 2621443 was sent the same impossible order 73 consecutive
turns (run T095712Z), and #2012's naming fix alone did not break the loop
(70 consecutive correctly-named refusals on run T145617Z). **#2030** seats a
live Spy at `host level − 1` and lets `spy.city` match rival cities.

**The war-fatigue clock**: wars ran in 42/54 runs; at-war rival cities were
reduced to 180–190/200 damage four times, walls to 353–400 — zero captures,
because `last_campaign_progress` resets only on a capture and the fatigued
branch then offers peace: run T075857Z journaled *"Offering peace to India |
the war has stalled: 1180 power against their 82"* with Chennai at 190/200.
**#2031** counts falling city or wall health on an at-war rival city as
campaign progress. Native pricing under domination-only victories: game wins
48/240 vs 38/240, paired directions 16–6 (sign p=0.0525), Elo +14 (CI
−10..+38), terminal score dead even — the gate is INCONCLUSIVE at this n
(resolves +48 at 80% power) and the arm stands as a zero-cost correctness
repair; no sweeps.

**The mission re-order seam**: the first run with a working chain re-sent
`SPY_GAIN_SOURCES` 35 times over t107–t141 — the export carries no running
operation, so the rebuilt mirror re-orders every turn. **#2037** trusts an
ordered Spy for the order's own duration (native games are immune; registered
Firaxis-only).

**The domination lane**: the launcher has never assigned it (22 lane-assigned
runs since 08-10: science 10, diplomatic 12, domination 0). Pricing the pin
(`live_target_domination` vs adaptive `live`, all six victories): **13.8% vs
86.2%, directions 0–87, p=0.0000, −319 Elo (discovery estimate) — RETAIN
live**; but the pin converted **22 domination victories to adaptive's 1**, so
conquest is mechanically reachable post-#2031 when aimed at. Per the written
rule, no rotation change: the organic route is converting the defensive wars
the seat already fights. Regime caveat: adaptive's native wins are 191/207
religious, a lane dead on the live seat, so −319 overstates the live cost.

**Live acceptance (run T155500Z, first combined binary)**: first
`SPY_TRAVEL_NEW_CITY` at t103 and first missions at t107 in recorded history,
zero promotion starvation; 16 settler starts by t96 against the old
whole-game median of ~6, 18 cities at t233 against the old t245 median of 8;
a 19-point diplomatic leader denied through the final congress; **won at the
cap, score 1606, lead +472 — both all-time ladder records** (prior bests
1360 / +409). The preceding game on the old binary lost by −531.

## What was decided

#2030, #2031, #2037 shipped (each withholdable; `live_without_*` controls
registered). No domination rotation change. No veto softening on the
loyalty-reach study (52% of vetoed sites were later settled by rivals and 98%
held, but rivals found within their own loyalty reach — confounded, parked
until post-#2027 runs show whether the veto becomes the binding constraint).

⚠ One game is never a result. The record win is the acceptance check firing,
not the effect size; the fleet sample from 2026-08-18T16:47Z onward is the
instrument that prices the day — read `lead` on the control ladder ledger,
and watch for the first live combat capture (`siege-is-progress`'s headline).
