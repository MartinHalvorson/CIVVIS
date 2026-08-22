# Pre-registration: siege_is_progress on the domination axis

2026-08-18, session claude-2703990e (goal: push score/science/domination).

## Hypothesis

The war-fatigue clock offering away winning sieges (`last_campaign_progress`
blind to city damage) suppresses domination conversion. With
`siege_is_progress` (PR #2031: an at-war rival city whose city or wall health
dropped since the previous observation counts as campaign progress), wars that
are landing net damage continue, and more of them convert to captures and
domination victories.

## Run

- Binary: worktree `civvis-siege-is-progress-0453` at the PR head (main +
  #2031 only).
- Command: `ai_eval live live_without_siege_is_progress --pairs 120
  --seed 818000 --victories domination --players 4 --width 24 --height 16
  --turns 500`
- One run, one seed, chosen before execution; no sweeps. Follow-up on a
  disjoint seed only if the decision rule below asks for it.

## Decision rule (written before the run)

- Primary: paired map wins for `live` (arm ON) vs the withhold control, sign
  test on map directions; and the count of games decided by a domination
  victory in each arm (the conversion metric this axis is about).
- PROMOTE-shaped result: win share ≥ 55% with sign p < 0.05, or domination
  conversions at least doubled with direction p < 0.05 → confirm on a
  disjoint seed before claiming.
- NULL: neither moves — record it in the PR / docs, the arm stays (it is a
  correctness repair priced at zero cost; a null here bounds its native war
  effect, not its live steal-lane effect).
- NEGATIVE (control wins ≥ 55%, p < 0.05): reopen the arm's default-on
  status before the next ladder cycle.

## OUTCOME (2026-08-18, run as registered; corrected from the full readout)

Head-to-head, 120 mirrored pairs / 240 games
(`/Users/martin/siege-progress-eval-818000.log`):

- **Game wins: live 48/240 (20.0%) vs control 38/240 (15.8%)**; paired
  directions **16–6** for live, exact sign p=0.0525; paired score 52.1%,
  Elo-equivalent **+14** (CI −10..+38).
- **Gate: INCONCLUSIVE** — n=120 resolves a true +48 Elo at 80% power.
- Terminal score dead even (50.4%, 48–46): the arm changes how wars END,
  not the economy — the expected signature. 86/240 games ended by
  domination; combat census identical between arms.
- Per the rule: positive direction below the promote bar → recorded, the
  arm stands as a zero-cost correctness repair, NO sweeps or extra seeds.
- ⚠ Process note: the first capture piped through `tail -40`, lost the
  header, and I briefly published a self-play misreading (48/120 = 40%
  conversions). The identical deterministic re-run recovered the honest
  numbers; both PR comments stand, the correction second.

## Known limits, stated up front

- Native regime, 4p/24×16, no city-states (`ai_eval` direct mode seats none)
  — this prices the NATIVE half of the repair. The live seat's defensive-war
  shape (assigned lanes, no_elective_war) is NOT reproduced here; the live
  answer comes from the ladder arms (`--without siege-is-progress`).
- `live` vs `live_without_*` differs by exactly one flag, so the comparison
  is paired and single-mechanism by construction.
