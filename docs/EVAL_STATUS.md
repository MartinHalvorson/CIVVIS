# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 205 |
| Live-bridge treatments | 74 |
| Firaxis-only treatments | 23 |
| Native engine-repair treatments | 51 |
| Withholdable live treatments | 51 |

## Bundle coverage

How much of the shipped live-bridge bundle the evaluation evidence has
ever *named* — `docs/EVAL.md` plus every round under `docs/eval/`.

- Withholdable live treatments: **51**
- Named somewhere in the evidence: **17**
- **Never named in any round: 34**

⚠ This is deliberately the weaker half of the question. Whether a
treatment was *priced* is a judgement about what a round concluded and
no string search can make it; whether it has ever been *named* is
mechanical. So the middle number over-counts coverage and the last one
under-counts the debt — act on the last one, which cannot be flattered.

`docs/ROADMAP.md` objective 3 asks for this bundle to be priced by
withholding. The inventory above counts the arms that exist; this counts
the ones that have been used. The gap is what stayed invisible: the
bundle applied natively measures −76/−108 Elo with the cause still open,
and `city_target_floor` once measured −41 while hidden in a composite.

Never named:

`amenity-district-path`, `amenity-project-preemption`, `army-target-weighs-enemy`, `blind-objective-strength`, `blind-objective-units`, `campus-every-city`, `district-coverage`, `endgame-war-runway`, `escort-unstick`, `garrison-under-fire`, `garrison-walls`, `governor-every-lane`, `housing-buildings`, `housing-cards`, `housing-districts`, `housing-research`, `loyalty-rate-alarm`, `muster-at-command-radius`, `ranged-line-of-sight`, `relief-targets-the-siege`, `religion-sues-peace`, `score-horizon`, `siege-commitment`, `siege-role`, `siege-tracks-wall`, `slot-kind-tiebreak`, `stacked-escort`, `stranded-settler-discount`, `suzerain-cards`, `war-economy`, `war-patience`, `war-reinforcement`, `wide-map-capacity`, `wonder-ring-settle-value`

## Live ladder

- Attempts recorded: **313**
- Configured attempts: **309**
- Terminal outcomes: **205**
- Configured wins: **3**
- Latest ledger entry: **2026-08-18T03:16:49Z**

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
