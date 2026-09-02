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
| Live-bridge treatments | 72 |
| Firaxis-only treatments | 32 |
| Native engine-repair treatments | 40 |
| Withholdable live treatments | 40 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **40**
- Named somewhere in the evidence: **37**
- **Never named in any round: 3**

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

`enter-the-prophet-race`, `relief-column-marches`, `threatened-city-reserve`

## Genome coverage

How much of the controller the genome instrument can vary at all.
`docs/GENE_SCREEN.md` names the growth direction as "hundreds of
genes"; this is the denominator that direction is measured against.

- Capability toggles on the controller: **274**
- Reachable as a gene `gene_screen` can vary: **233**
- Measured by at least one screen: **73**
- Resolved by the ledger (helps or hurts): **16**
- **Unreachable by any screen: 41**

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

`bank_envoys`, `camp_bounty`, `counter_in_lane`, `deny_while_targeted`, `engine_repairs`, `engine_repairs_economy`, `engine_repairs_universe`, `engine_repairs_war`, `envelope_cache_across_own_moves`, `era_paced_expansion`, `escort_patience_runs_out`, `expansion_before_prophet`, `expansion_hall`, `expansion_pantheon`, `explore_commit`, `explore_dead_targets`, `fog_land_capacity`, `frontier_loyalty`, `guard_breaks_the_pin`, `host_settler_pop`, `land_grab`, `live_barbarian_scouts_capture`, `live_bridge`, `live_bridge_universe`, `live_formationless_settler_shadow`, `live_motion_turn_accounting`, `live_move_refusal_break`, `live_religious_purchase_guard`, `live_settler_capture_lessons`, `live_trader_route_adapter`, `live_wonder_race`, `no_elective_war`, `opening_settler_waits`, `parallel_settlers`, `projected_stock_denial`, `settler_founds_when_stalled`, `solvent_faith_army`, `spy_mission_patience`, `stock_denial_lead_time`, `tally_culture`, `tally_great_people`

## Live ladder

- Attempts recorded: **650**
- Configured attempts: **642**
- Terminal outcomes: **326**
- Configured wins: **23**
- Latest ledger entry: **2026-08-30T11:23:20Z**

- Attempts that ran the full clock: **207**, median score **544**, best **1606**
- Graded against the best rival: **337** of 207 finished attempts; rival bar median **597**, our lead median **-182**, best **+759**, ahead in **46**
- Lost to a rival's victory before the clock: **117** (diplomatic 58, culture 42, technology 11, religious 5, conquest 1), of which **17** while our own score was the highest on the board
- The turns those landed on: conquest 27–27 (median 27), culture 145–247 (median 229), diplomatic 202–247 (median 241), religious 75–233 (median 170), technology 227–246 (median 242)

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
