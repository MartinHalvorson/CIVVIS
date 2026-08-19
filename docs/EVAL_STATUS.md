# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 232 |
| Live-bridge treatments | 85 |
| Firaxis-only treatments | 29 |
| Native engine-repair treatments | 56 |
| Withholdable live treatments | 56 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **56**
- Named somewhere in the evidence: **33**
- **Never named in any round: 23**

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

`amenity-district-path` (`live_without_amenity_district_path`), `amenity-project-preemption` (`live_without_amenity_project_preemption`), `blind-objective-strength` (`live_without_blind_objective_strength`), `blind-objective-units` (`live_without_blind_objective_units`), `campus-every-city` (`live_without_campus_every_city`), `endgame-war-runway` (`live_without_endgame_war_runway`), `garrison-under-fire` (`live_without_garrison_under_fire`), `garrison-walls` (`live_without_garrison_walls`), `housing-cards` (`live_without_housing_cards`), `housing-research` (`live_without_housing_research`), `loyalty-rate-alarm` (`live_without_loyalty_rate_alarm`), `muster-at-command-radius` (`live_without_muster_at_command_radius`), `relief-targets-the-siege` (`live_without_relief_targets_the_siege`), `settler-guard-holds` (`live_without_settler_guard_holds`), `settler-site-agreement` (`live_without_settler_site_agreement`), `siege-commitment` (`live_without_siege_commitment`), `siege-role` (`live_without_siege_role`), `siege-tracks-wall` (`live_without_siege_tracks_wall`), `stacked-escort` (`live_without_stacked_escort`), `stranded-settler-discount` (`live_without_stranded_settler_discount`), `suzerain-cards` (`live_without_suzerain_cards`), `war-reinforcement` (`live_without_war_reinforcement`), `wonder-ring-settle-value` (`live_without_wonder_ring_settle_value`)

## Live ladder

- Attempts recorded: **317**
- Configured attempts: **310**
- Terminal outcomes: **209**
- Configured wins: **3**
- Latest ledger entry: **2026-08-18T06:17:27Z**

- Attempts that ran the full clock: **133**, median score **449**, best **1191**
- Graded against the best rival: **17** of 133 finished attempts; rival bar median **1035**, our lead median **-83**, best **+409**, ahead in **6**
- Lost to a rival's victory before the clock: **74** (diplomatic 41, culture 24, religious 5, technology 3, conquest 1), of which **3** while our own score was the highest on the board

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
