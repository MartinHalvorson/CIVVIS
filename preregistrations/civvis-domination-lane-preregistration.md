# Pre-registration: is the domination lane worth assigning on the live seat?

2026-08-18, session claude-2703990e (goal: push domination victories).

## Context

The live launcher has never assigned the conquest lane (22 lane-assigned runs
since 08-10: science 10, diplomatic 12, domination 0), so the seat never aims
at a domination victory by construction. #1959 made `live_target_domination`
evaluable; #1960 selected Diplomacy as the central default on its own
evidence. With #2031 (siege-is-progress) landed, wars now end in conquest
natively 20% of games (prereg `~/civvis-siege-progress-preregistration.md`).

## Question

Under the deployment victory set (all six), does pinning the conquest lane
(`live_target_domination`) cost or gain against adaptive `live` — and does it
actually convert domination victories?

## Run

- Binary: dedicated eval worktree at current origin/main (post-#2037).
- Command: `ai_eval live_target_domination live --pairs 120 --seed 819000
  --players 4 --width 24 --height 16 --turns 500
  --victories science,culture,religious,diplomatic,domination,score`
- One run, one fresh seed. No sweeps.

## Decision rule (before the run)

- Read the victory-type census FIRST: domination wins by the pinned arm is
  the conversion question.
- If the pinned arm is not significantly WORSE (paired sign p — if it loses
  with p<0.05, record the lane as not livable and stop), AND it converts
  domination victories at a material rate (≥10% of its games), then propose
  adding `domination` to the live rotation as a minority arm (e.g. alongside
  science/diplomatic) — as a PR to the launcher rotation, priced by the
  ladder itself thereafter.
- If it wins or draws but converts ~zero dominations, the lane pin is inert
  live — record and stop.
- Known limits: native 4p/24×16, no city-states, adaptive rivals — regime
  caveats as ever; this prices the LANE PIN, not the ladder outcome.

## OUTCOME (2026-08-18, run as registered)

`/Users/martin/domination-lane-eval-819000.log`:

- **live_target_domination 33/240 (13.8%) vs live 207/240 (86.2%)** — paired
  directions 0–87, p=0.0000, Elo −319 (discovery estimate). RETAIN live.
- **BUT the pin converts: 22 of its 33 wins are DOMINATION victories** (plus
  6 culture, 5 score) vs adaptive's 1 domination — conquest is mechanically
  reachable when aimed at (post-#2031); forcing the aim forfeits the rest.
- Regime caveat: adaptive live's native wins are 191/207 RELIGIOUS — a lane
  dead on the live seat — so the −319 is regime-colored. The rule still
  says stop: no rotation change proposed. Domination on the live seat should
  arrive organically (armies + siege patience making conquest the best lane
  in games where it is), not by pin.
