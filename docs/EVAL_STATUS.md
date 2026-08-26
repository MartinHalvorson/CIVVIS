# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from the gene registry (`src/ai/advanced/genes.rs`),
`src/elo.rs` (the built-in agents) and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Live-bridge treatments | 63 |
| Firaxis-only treatments | 27 |
| Native engine-repair treatments | 36 |
| Withholdable live treatments | 36 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **36**
- Named somewhere in the evidence: **36**
- **Never named in any round: 0**

⚠ This is deliberately the weaker half of the question. Whether a
treatment was *priced* is a judgement about what a round concluded and
no string search can make it; whether it has ever been *named* is
mechanical. So the middle number over-counts coverage and the last one
under-counts the debt — act on the last one, which cannot be flattered.

The native half of this bundle is priced by the gene screen
(`docs/GENE_SCREEN.md`, `GENE_HEURISTIC_RANKING.md`); the host-only
half can only be priced on the live seat, by `civvis_orders --without`
over ladder games. This list is the debt neither has touched.

Never named:

_None — every withholdable treatment has been named._

## Genome coverage

How much of the controller the genome instrument can vary at all.
`docs/GENE_SCREEN.md` names the growth direction as "hundreds of
genes"; this is the denominator that direction is measured against.

- Capability toggles on the controller: **205**
- Reachable as a gene `gene_screen` can vary: **155**
- Measured by at least one screen: **73**
- Resolved by the ledger (helps or hurts): **16**
- **Unreachable by any screen: 50**

⚠ This is the mirror of the section above and it errs the other way.
`Never named` under-counts the live-bundle debt; this OVER-counts the
genome debt, because some toggles are host-only or bundle plumbing
that no native screen could price and nothing here tries to tell them
apart. So the last number is a ceiling on the work, not a floor — and
a count that can only be wrong in the direction of more work cannot be
flattered either.

Why it is published: `precise_evacuation` shipped in #2059 ON for
every major, city-state and barbarian, holding roughly half of the
simulator's main thread, with no gene row and no
mention in any recorded round. Neither gate could address it and
nothing said so.

Unreachable:

`adjacent_camp_clear`, `amenity_districts`, `bank_envoys`, `barbarian_heretic_hunt`, `battlefront_observation`, `camp_bounty`, `counter_in_lane`, `deny_while_targeted`, `engine_repairs`, `engine_repairs_economy`, `engine_repairs_universe`, `engine_repairs_war`, `envelope_cache_across_own_moves`, `era_paced_expansion`, `expansion_before_prophet`, `expansion_hall`, `expansion_pantheon`, `explore_commit`, `explore_dead_targets`, `fog_land_capacity`, `frontier_loyalty`, `governor_in_recovery`, `great_work_veto_by_district`, `host_settler_pop`, `hut_collection`, `land_grab`, `legal_tactical_candidates`, `live_bridge`, `live_bridge_universe`, `live_formationless_settler_shadow`, `live_motion_turn_accounting`, `live_religious_purchase_guard`, `live_trader_route_adapter`, `live_wonder_race`, `no_elective_war`, `open_water_navy`, `opening_settler_waits`, `parallel_settlers`, `production_builder_floor`, `production_settler_deadline`, `projected_stock_denial`, `sea_answers`, `settlement_safety`, `settler_founds_when_stalled`, `solvent_faith_army`, `spy_mission_patience`, `stock_denial_lead_time`, `tally_culture`, `tally_great_people`, `village_seeking`

## Live ladder

- Attempts recorded: **425**
- Configured attempts: **418**
- Terminal outcomes: **299**
- Configured wins: **16**
- Latest ledger entry: **2026-08-19T11:21:36Z**

- Attempts that ran the full clock: **190**, median score **514**, best **1606**
- Graded against the best rival: **120** of 190 finished attempts; rival bar median **1108**, our lead median **-218**, best **+759**, ahead in **34**
- Lost to a rival's victory before the clock: **107** (diplomatic 58, culture 38, religious 5, technology 5, conquest 1), of which **15** while our own score was the highest on the board
- The turns those landed on: conquest 27–27 (median 27), culture 145–247 (median 230), diplomatic 202–247 (median 241), religious 75–233 (median 170), technology 242–246 (median 244)

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
