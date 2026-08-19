# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 238 |
| Live-bridge treatments | 89 |
| Firaxis-only treatments | 30 |
| Native engine-repair treatments | 59 |
| Withholdable live treatments | 59 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **59**
- Named somewhere in the evidence: **35**
- **Never named in any round: 24**

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

`amenity-district-path` (`live_without_amenity_district_path`), `amenity-project-preemption` (`live_without_amenity_project_preemption`), `barbarian-walls-one-tier` (`live_without_barbarian_walls_one_tier`), `blind-objective-strength` (`live_without_blind_objective_strength`), `blind-objective-units` (`live_without_blind_objective_units`), `campus-every-city` (`live_without_campus_every_city`), `endgame-war-runway` (`live_without_endgame_war_runway`), `garrison-under-fire` (`live_without_garrison_under_fire`), `garrison-walls` (`live_without_garrison_walls`), `housing-cards` (`live_without_housing_cards`), `housing-research` (`live_without_housing_research`), `idle-walkers-close-the-pipeline` (`live_without_idle_walkers_close_the_pipeline`), `muster-at-command-radius` (`live_without_muster_at_command_radius`), `relief-targets-the-siege` (`live_without_relief_targets_the_siege`), `settler-guard-holds` (`live_without_settler_guard_holds`), `settler-site-agreement` (`live_without_settler_site_agreement`), `siege-commitment` (`live_without_siege_commitment`), `siege-role` (`live_without_siege_role`), `siege-tracks-wall` (`live_without_siege_tracks_wall`), `stacked-escort` (`live_without_stacked_escort`), `stranded-settler-discount` (`live_without_stranded_settler_discount`), `suzerain-cards` (`live_without_suzerain_cards`), `war-reinforcement` (`live_without_war_reinforcement`), `wonder-ring-settle-value` (`live_without_wonder_ring_settle_value`)

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
