# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 228 |
| Live-bridge treatments | 79 |
| Firaxis-only treatments | 30 |
| Native engine-repair treatments | 49 |
| Withholdable live treatments | 49 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **49**
- Named somewhere in the evidence: **49**
- **Never named in any round: 0**

⚠ This is deliberately the weaker half of the question. Whether a
treatment was *priced* is a judgement about what a round concluded and
no string search can make it; whether it has ever been *named* is
mechanical. So the middle number over-counts coverage and the last one
under-counts the debt — act on the last one, which cannot be flattered.

`docs/ROADMAP.md` objective 3 asks for this bundle to be priced by
withholding, *before the next effect hides inside a composite the way
`city_target_floor` did*. The inventory above counts the arms that
exist; this counts the ones that have been used, and the gap between
them is what stayed invisible.

Never named:

_None — every withholdable treatment has been named._

## Genome coverage

How much of the controller the genome instrument can vary at all.
`docs/GENE_SCREEN.md` names the growth direction as "hundreds of
genes"; this is the denominator that direction is measured against.

- Capability toggles on the controller: **165**
- Reachable as a gene `gene_screen` can vary: **100**
- Measured by at least one screen: **65**
- Resolved by the ledger (helps or hurts): **21**
- **Unreachable by any screen: 65**

⚠ This is the mirror of the section above and it errs the other way.
`Never named` under-counts the live-bundle debt; this OVER-counts the
genome debt, because some toggles are host-only or bundle plumbing
that no native screen could price and nothing here tries to tell them
apart. So the last number is a ceiling on the work, not a floor — and
a count that can only be wrong in the direction of more work cannot be
flattered either.

Why it is published: `precise_evacuation` shipped in #2059 ON for
every major, city-state and barbarian, holding roughly half of the
simulator's main thread, with no gene row, no evaluator arm and no
mention in any recorded round. Neither gate could address it and
nothing said so.

Unreachable:

`adjacent_camp_clear`, `amenity_districts`, `bank_envoys`, `battlefront_observation`, `builder_reward_survey`, `camp_bounty`, `counter_in_lane`, `coupled_expansion`, `deny_while_targeted`, `engine_faith_price`, `engine_repairs`, `engine_repairs_economy`, `engine_repairs_universe`, `engine_repairs_war`, `envelope_cache_across_own_moves`, `era_paced_expansion`, `expansion_before_prophet`, `expansion_hall`, `expansion_pantheon`, `explore_commit`, `explore_dead_targets`, `fog_honest`, `fog_land_capacity`, `fortify_idle_units`, `frontier_loyalty`, `governor_in_recovery`, `great_work_veto_by_district`, `host_settler_pop`, `hut_collection`, `joint_reach_lines`, `land_grab`, `legal_tactical_candidates`, `live_bridge`, `live_bridge_universe`, `live_formationless_settler_shadow`, `live_motion_turn_accounting`, `live_religious_purchase_guard`, `live_trader_route_adapter`, `live_wonder_race`, `maintenance_aware_deck`, `naval_production_policy`, `no_elective_war`, `open_water_navy`, `opening_settler_waits`, `pantheon_board`, `parallel_settlers`, `price_the_suzerainty`, `production_builder_floor`, `production_settler_deadline`, `projected_stock_denial`, `promote_when_wounded`, `sea_answers`, `settlement_gap_target`, `settlement_safety`, `settler_founds_when_stalled`, `solvent_faith_army`, `spy_mission_patience`, `step_and_reassess`, `stock_denial_lead_time`, `tactical_strategy`, `tally_culture`, `tally_great_people`, `unit_cost_efficiency`, `unit_objective_memory`, `village_seeking`

## Live ladder

- Attempts recorded: **349**
- Configured attempts: **342**
- Terminal outcomes: **232**
- Configured wins: **7**
- Latest ledger entry: **2026-08-19T11:21:36Z**

- Attempts that ran the full clock: **147**, median score **461**, best **1588**
- Graded against the best rival: **48** of 147 finished attempts; rival bar median **1077**, our lead median **-207**, best **+759**, ahead in **12**
- Lost to a rival's victory before the clock: **83** (diplomatic 47, culture 27, religious 5, technology 3, conquest 1), of which **4** while our own score was the highest on the board
- The turns those landed on: conquest 27–27 (median 27), culture 145–245 (median 221), diplomatic 202–247 (median 234), religious 75–233 (median 170), technology 242–244 (median 242)

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
