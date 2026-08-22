# Pre-registration — what is a victory lane worth as a function of when you commit?

Written **before** the run, 2026-07-28. Branch `agent/martin-mbp/loop-lanes/…-2464`, PR #519.

## Question

PR #366's oracle ablation measured committed-Religion-from-turn-one winning
29/50 matched cells against the shipped adaptive agent's 14/50 (McNemar
p=0.0000). `plan_churn` (PR #519) measured the adaptive agent switching lane
14.2 times a game and spending 34.9% of it on lanes that won 0/50. But
`refuse_unreachable_lanes`, the filter built to stop exactly that, measured
**null** at 120 maps — and its note records why: *103 of 120 `advanced` wins
were religious anyway.*

So the agent is probably not picking the wrong lane. It plausibly reaches the
right lane **late**. This measures the value of a lane against the turn it is
entered.

## Design

`commit_curve --lane religion --maps 40 --players 4 --width 60 --height 38
--city-states 6 --turns 500 --seed 420000 --commits 0,60,120,180 --genome champion`

One focal seat per map (rotating with map index), committed via
`AdvancedAi::retarget` at turn T; all other seats stock. Same map replayed for
each T plus an adaptive control. Deterministic engine, so cells differ only by
the treatment. 40 maps × 5 conditions = 200 games.

Genome is the **embedded gen-14 champion**, not `Weights::default()` — the
fallback is not the agent that ships.

## Calibration, fixed in advance

`T=0` is the oracle's own condition on the oracle's own profile and seed. It
should land near **58%** against a control near **28%**.

**If `T=0` is not at least 10 points above the control, the oracle result does
not reproduce on this harness and the rest of the curve is void.** In that case
the deliverable is the harness reconciliation, not a routing conclusion.

## Prediction (mine, recorded before the run)

**I predict timing is the lever: `T=0` − `T=180` ≥ 10 points.**

Mechanism: a religion is a *finite global race*. Holy Site → Great Prophet is a
first-come-first-served slot, and a seat entering the lane at turn 180 finds
the prophets already claimed, so the commitment cannot convert however sincere
it is. `assess()` already half-knows this — it has a dedicated arm reading "a
Prophet is a finite race worth entering now" — but that arm sits *below* the
opportunistic power-ratio Conquest arm in the cascade.

## What refutes it

- **A flat curve** (all commitments within 10 points of each other, and above
  the control) refutes timing and says the fix is lane *selection*: any
  mechanism that ends in the right lane suffices, and early commitment and
  hysteresis are both the wrong build.
- **A flat curve at the control's level** refutes the whole routing premise on
  this harness — see calibration above.

## What I will do with each outcome

| outcome | next build |
|---|---|
| decaying curve | raise the religious-opening arm above the opportunistic Conquest arm in `assess()`, A/B as a flag |
| flat and high | leave the cascade alone; work on lane selection (a viability filter that is not just Science) |
| flat at control | reconcile against `ablate` before building anything |

No result here ships anything on its own. Whatever it says, the intervention
goes through `ai_eval` with its own pre-registration and the unmodified
promotion gate.
