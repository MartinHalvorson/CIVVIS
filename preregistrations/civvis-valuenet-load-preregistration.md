# Preregistration: what the learned evaluator is worth when it actually loads

Fixed 2026-07-30T00:0xZ, before any powered run was read.
Agent `claude-frontier`, machine `martin-mbp`.

## Background (established, not under test)

`ValueNet::load(dir)` reads `<dir>/valuenet.json` with a **cwd-relative** path
and has no fallback. `evolve::load_champion` — one file away — resolves
local → `data/<dir>` → embedded (`evolve.rs:124`). No `evolved/valuenet.json`
exists in any checkout; `data/evolved/` holds only `best.json`. So no game
played anywhere on this machine has ever loaded the learned evaluator.

`docs/EVAL.md` records the no-model signature explicitly: with no artifact,
`strategic` vs `strategic_score` is *"digit-for-digit identical"*. The
2026-07-26 entry then reports 10 neutral splits on 10 maps and concludes
**"the evaluator is good and inert."** That is the same signature.

**Reproduced today, seed 37000, 4 pairs, 4 players, 200 turns:**

| arm | provenance | result |
|---|---|---|
| no artifact in cwd | `strategic: plays as strategic_score (missing valuenet.json)` + self-comparison warning | 4/8, **4 neutral maps, every diagnostic identical to the digit** |
| 25-wide net staged in cwd | `strategic: loaded best.json, valuenet.json` | 5/8, 1 sweep + 3 neutral, **every diagnostic differs** |

So the inertness claim rests on a run in which the treatment and the control
were the same agent. What the net is worth is **unmeasured**, not inert.

## The artifact

`/Users/martin/civvis-valuenet-corpus/valuenet.json`, sizes `[25,64,32,1]`,
matching `evolve::FEATURE_WIDTH = 25`. Test BCE **0.4058 vs 0.5636** constant,
accuracy 0.809 vs 0.749, ECE 0.035, `beats_baseline: true`. This is the exact
metric `docs/AI_GAPS.md` quotes when it calls the evaluator calibrated.

`evolve::features()` is **byte-identical** since the corpus was trained
(`622924b`): no diff hunk since then falls inside the function, so the net is
feature-aligned and this is not a stale-artifact experiment.

Trained on: `ai advanced, players 4, 60x38, max_turns 400, every 20, seed 41000`.

## Hypotheses

H1. With the net loaded, `strategic` differs from `strategic_score`.
    **Already established above** — recorded so it is not re-tested.
H2. The blended objective raises the branch spread the commitment margin
    gates on. `position_value` returns
    `score_share + 0.25*(learned - score_share)`; median branch spread at
    horizon 40 is 0.0045 against `TARGET_COMMITMENT_MARGIN = 0.01`, so an
    ordinary review cannot clear its own threshold. A calibrated
    win-probability differing ~0.1 across lanes moves blended values ~0.025.
H3. The net wins games at the profile it was trained on.
H4. The net wins games at the deployment profile.

H3 and H4 are separate because the net is **out of distribution** at
deployment: `features()` normalizes by absolute constants (`score/400`,
`pop/60`, `military/200`), and 6 players on 74x46 build larger empires than
the 4p 60x38 games the corpus came from. A null at deployment with a win
in-distribution is a distribution-shift result, not a verdict on learned value.

## Fixed runs

Both are `strategic` (net loaded, cwd staged with `evolved/valuenet.json`)
against `strategic_score` (same genome, same horizon, same seat schedule,
net forcibly disabled). Paired, mirrored, seat-swapped.

1. **In-distribution** — `--pairs 120 --players 4 --width 60 --height 38
   --turns 400 --seed 640000` (the corpus config).
2. **Deployment profile** — `--pairs 120 --players 6 --width 74 --height 46
   --city-states 9 --speed online --turns 250 --seed 650000`.

Disjoint seeds. 120 pairs is the repo's power convention; 40 is a screen.

## Decision rule, fixed before reading

| outcome | action |
|---|---|
| `promotion gate: PASS` at the deployment profile | ship the net: commit `data/evolved/valuenet.json` + fixture, give `ValueNet::load` the three-tier resolution `load_champion` already has |
| PASS in-distribution, null at deployment | ship the **loader fix** only, leave the artifact unshipped, and record the shift; the loader defect is real regardless of this net's strength |
| null in both | record the evaluator as inert **with the artifact loaded** — the honest version of the existing claim — and keep the loader fix so the next trained net is reachable |
| net loses significantly | record it, keep the loader fix, and do not ship the artifact |

**The loader fix is justified by the defect, not by this net's win rate.** A
path that silently resolves to a fallback is the same bug `#469`/`#471` fixed
for the genome and `#490` fixed for the league roster; this is the third
instance. What the runs decide is only whether *this* artifact ships with it.

## Read before the win rate

- the provenance line, on both arms (a run where `strategic` reports
  `plays as strategic_score` measures nothing)
- `Macro search exposure` — a run that never reaches the rollouts cannot
  exercise the evaluator at all
- direction resolution (how many maps actually broke)
