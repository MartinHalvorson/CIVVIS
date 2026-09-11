# Culture and domination conversion genes

Ten independently switchable, default-off experiments for the Emperor
(level 6) culture and domination lanes. These are hypotheses about converting
an economy and army into a finish. No 10× uplift has been demonstrated.

| Gene | Executed behavior |
|---|---|
| `victory-deadline-budget` | Samples known rivals' victory progress and our assigned lane once per turn. At least five turns of rising progress in the same lane are required for an ETA. When a rival is projected to finish first, discounts investments arriving too late and favors timely finishing assets. Recovery and emergency city defense retain priority. |
| `culture-tourism-payback` | Previews cultural buildings using the engine's tourism calculation, including usable works and themes; rewards incremental tourism across contacted markets over the remaining useful lifetime. |
| `siege-positive-damage-budget` | Adds a wall/city damage-versus-health budget to the existing siege train's Stage → Invest gate, and requests missing breach/capture units through production. Enabling it makes the siege doctrine available without mutating the separate `siege-train` flag. |
| `culture-faith-reservation` | Compares reachable park and concert opportunities per Faith. Reserves their quoted cost during the unlock window, chooses the usable purchase city, and releases unusable reserves. The spending pass still calls the engine's normal Buy action. |
| `capital-campaign-router` | Adds travel, wall reduction, retention and onward-capital estimates to campaign routing. Uses actual routes for the first leg and a distance lower bound for the next leg; preserves forced opponents, denial targets and existing capture safety. |
| `great-work-completion-value` | Rewards buildings that change usable Great Work housing, and evaluates offered work purchases with a hypothetical legal trade so actual creator/era identities can affect themes. It does not invent an independent live rearrangement command. |
| `upgrade-window-campaign` | Finds groups of two or more healthy units whose successor technology is near, reserves formation-aware upgrade quotes, prioritizes that research below immediate defense, upgrades before elective purchases, and withholds elective declaration until the package is upgraded. Existing campaign staging remains in force. A bounded wait releases an unaffordable package. |
| `tourism-land-reservation` | Reserves up to one prospective park per city, capped at three cities, and a strong resort site per city from Humanism onward. Prices completing plot purchases, plants supporting woods with the existing builder operation, and protects the park's woods from the expansion chop gene. |
| `reinforce-before-stall` | Compares frontline health, missing roles, construction time and marching time. Raises production of timely reinforcements before the current train fails, while limiting duplicate queues. |
| `capture-hold-chain` | Estimates post-capture loyalty before choosing a city; preserves the final-capital exception. After capture, orders garrison priority by turns to revolt and raises Victor, repair and loyalty-building value. |

Implementation: `src/ai/advanced/victory_conversion.rs`; focused behavior
tests live in `src/ai/advanced/victory_conversion/tests.rs`. Normal action
legality remains authoritative. Travel, future production, enemy reactions
and victory dates are estimates, not a simulated guarantee of success.

The native siege arithmetic uses `src/game.rs::city_take_damage` and the
existing `src/ai/advanced/siege_train.rs` doctrine: wall multipliers are
0.15 melee, 0.5 ranged, 1.0 siege. This change does not alter the underlying
combat rules or the live control mod.

## Reproduction

Focused behavior checks:

```sh
cargo test --profile ci --locked --lib victory_conversion
```

Paired native Emperor bundle pilot (default two seeds per lane, eight games):

```sh
CIVVIS_CONVERSION_SEEDS=2 CIVVIS_CONVERSION_OUT=/tmp/victory-conversion.json \
  cargo test --profile ci --locked --lib emperor_conversion_paired_census \
  -- --ignored --nocapture
```

The measured seat rotates across four chairs as seeds advance, has no AI
handicap, and faces three fixed deployment opponents targeting a reproducible
mix of science, culture and domination. Only its ten switches differ between
paired arms. Map: Pangaea, 40×24, Online, four city-states, 250-turn cap,
native competitions enabled. The primary outcome is the measured seat winning
by the requested victory type; score and other wins are reported separately.
This instrument is a native controller comparison, not a Firaxis live win rate.

For individual pricing on the established random-genome instrument, use
`gene_screen` with `--difficulty emperor --handicap rivals --rivals firaxis-mix
--rival-chairs 3 --players 4 --target-mix culture,domination` and these ten
tags in `--genes`. Increase game count and use disjoint held-out seeds before
deciding deployment defaults; the small paired pilot is a behavior and runtime
check, not statistical evidence for promotion.

## Validation record

Validation results and the paired pilot artifact are recorded here after the
final implementation has been checked. No deployment defaults are changed.
