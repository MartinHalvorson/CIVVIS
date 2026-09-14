# Government upgrades priced by the remaining civic path

`government-ladder-2` chooses the cheapest printed government-unlock civic.
That can prefer Divine Right at 340 Culture while still owing its branch,
over Exploration at 440 whose prerequisites are already complete. The final
node's price does not measure when the new policy slots arrive.

`government-ladder-3` keeps both prior versions for the normal family screen.
It retains v2's candidate governments, Political Philosophy prerequisite,
and half-clock initiation window (three quarters while behind the field on
slots). It ranks each eligible upgrade by the full remaining Culture bill
per extra policy slot. Shared prerequisites are counted once, known civics
are free, earned Inspirations include China's extra grant, and active
progress is not credited a second time. Unassigned Culture overflow is
deducted once from the whole path. Lower total cost and then civic name
resolve equal per-slot costs.

The complete path must fit within the remaining game clock with twenty
Standard turns left to use the government. The estimate uses current city
Culture and game-speed-adjusted costs and durations. It does not assume
future Culture growth or price individual government effects; those are
limits of this candidate, not measured improvements.

## Fixed evaluation plan

Registered on 2026-09-14 before games run, with fixed sample sizes and seeds:

- A 12-game family probe, seeds 914357300–914357311, naming all three versions
  so the default-on v2 cannot suppress the challenger. All other genes stay at
  the evaluator baseline. This is reach evidence, not strength confirmation.
- A 60-game whole-registry screen on disjoint seeds 914358000–914358059. Each
  seat draws its own genome with the screen's standard probabilities. Read the
  family table against off and against v2, including uncertainty; the sample
  cannot justify silently replacing the deployed version.

Both use the standard six-player, 74×46 Continents, nine-city-state, Online
250-turn Emperor shape. The default target mix and player contract are
recorded by the executable's header. Raw rows stay outside the repository;
the analyzed family artifact is committed for the gene-fires gate.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes government-ladder,government-ladder-2,government-ladder-3 \
  --games 12 --start-seed 914357300 --jobs 3 --difficulty emperor --out FAMILY_ROWS
target/ci/gene_screen --analyze FAMILY_ROWS \
  --json docs/gene_screens/fires/government-ladder-3.json
target/ci/gene_screen --games 60 --start-seed 914358000 --jobs 3 \
  --difficulty emperor --out SCREEN_ROWS
target/ci/gene_screen --analyze SCREEN_ROWS --json SCREEN_SUMMARY
```

## Validation and results

Pending execution. The focused tests include real prerequisite paths that
reverse v2's choice, the actual research-to-government-adoption flow,
Inspiration and overflow accounting, cost per extra slot, affordability,
existing time windows, and reversible version toggles. The complete Rust
suite and repository gene gates run before integration.
