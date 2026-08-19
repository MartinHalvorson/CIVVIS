# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 221 |
| Live-bridge treatments | 79 |
| Firaxis-only treatments | 24 |
| Native engine-repair treatments | 55 |
| Withholdable live treatments | 55 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **55**
- Named somewhere in the evidence: **22**
- **Never named in any round: 33**

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

`amenity-district-path`, `amenity-project-preemption`, `army-target-weighs-enemy`, `blind-objective-strength`, `blind-objective-units`, `campus-every-city`, `endgame-war-runway`, `escort-unstick`, `garrison-under-fire`, `garrison-walls`, `housing-buildings`, `housing-cards`, `housing-districts`, `housing-research`, `loyalty-rate-alarm`, `muster-at-command-radius`, `ranged-line-of-sight`, `relief-targets-the-siege`, `religion-sues-peace`, `score-horizon`, `settler-site-agreement`, `siege-commitment`, `siege-role`, `siege-tracks-wall`, `slot-kind-tiebreak`, `stacked-escort`, `stranded-settler-discount`, `suzerain-cards`, `war-economy`, `war-patience`, `war-reinforcement`, `wide-map-capacity`, `wonder-ring-settle-value`

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
