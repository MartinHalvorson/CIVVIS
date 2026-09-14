# Builder recovery counts repair jobs

The workforce recovery family can recognize empire-wide repair work, then
reject every local production candidate because its local checks accept only
new improvements. V2 additionally requires three distinct new-improvement
tiles. Several pillaged Farms therefore cannot justify a replacement Builder.

`builder-workforce-recovery-3` counts new improvements and repairs toward the
same three-tile threshold. A job counts only when the tile still belongs to
the candidate city. The gene retains the absence-of-builders requirement,
one reserved idle queue, empire expansion guard, military reservations and
twenty-standard-turn payoff window. Existing versions keep their decisions.
The live controller's reported identity follows its mutually exclusive flags.

This is a local workload heuristic, not a forecast of route availability or
the economic value of each repair. Repairs consume no Builder charges; the
three-job floor is deliberately retained from v2 rather than claimed optimal
for free repairs. The new version defaults off and changes no deployment
selection.

## Predeclared activation

Before any results are collected, run six complete Emperor games with seeds
914357700–914357705 and two workers, screening all three workforce-recovery
versions because v2 defaults on. Keep the standard six-player, 74×46
Continents, nine-city-state, Online-speed 250-turn shape, all victories and
standard genome probabilities. The activation sample establishes execution
and records uncertainty; it cannot establish a win-rate improvement or
justify a default change.

## Activation results

All six games and all 36 intended seats completed on clean source
`39ed02f2249b1bdd8e3bcd2bb575c50dc6fbe437`, with binary SHA-256
`d2f15a6da559b02f6b19b0ec5f38b15df92f5b1c189c52cf524ae91e72d02df3`
and gene fingerprint
`198e42dcab3a8999b749eb750082fa612ad34e45201eb8fe5bf329942c8b5223`.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes builder-workforce-recovery,builder-workforce-recovery-2,builder-workforce-recovery-3 \
  --games 6 --start-seed 914357700 --jobs 2 --difficulty emperor --out recovery.jsonl
target/ci/gene_screen --analyze recovery.jsonl \
  --json docs/gene_screens/fires/2026-09-14-builder-repair-recovery.json
```

V3 appeared on nine seats and won twice; the other 27 seats won four times.
Its marginal win difference is +7.41 pp (SE 13.30),
and score-share difference is +0.39 pp (SE 0.59).
Both measurements are unresolved. Other seats include older family versions;
this row is not a comparison against only the family-off level. Three games
ended in science, two in culture and one by score.

Seven focused regressions cover repair-only work, mixed workloads, the retained
three-job threshold, ordinary improvement work, military and clock protections,
stale ownership, exclusive defaults and runtime identity. These decision tests
and the complete-game probe do not establish a reliable win-rate improvement.
