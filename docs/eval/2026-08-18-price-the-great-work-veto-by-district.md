# Price the great-work veto by district

_2026-08-18 · `bc381abc`_

## What was asked

Can the non-Culture Great Work building veto be measured as the district policy
it is intended to be, rather than as a proxy based on slot presence? And can
the existing Theater Square building-debt treatment be reached on a targeted
Culture seat without changing the target or importing unrelated live settings?

## How it was measured

This is a structural pre-registration, not a game-performance batch. The new
`advanced_great_work_veto_by_district` arm is Science-targeted and differs from
`advanced_target_science` only by `great-work-veto-by-district`. The new
`advanced_target_culture_with_culture_building_debt` arm differs from
`advanced_target_culture` only by `culture-building-debt`.

Focused unit tests exercise the three classifier boundaries (Amphitheater,
Marae, and National History Museum) and prove that the Culture-targeted debt
changes the Theater-building value. Typed-arm tests require each comparison to
remain exactly one semantic axis, and factory/provenance coverage prevents a
selectable name from falling through to an unrelated controller. No games,
seeds, tournament profile, win-rate calculation, or Elo calculation ran in
this round.

## What it measured

The historical slot key refuses both Amphitheater and the Government Plaza's
National History Museum, while allowing the slotless Theater Square building
Marae. The district key still refuses Amphitheater, refuses Marae, and allows
National History Museum to be valued on its own merits. On a Culture-targeted
seat, enabling the existing building debt makes Amphitheater strictly more
valuable than the otherwise identical control.

Those are mechanism checks, not performance estimates. There is therefore no
win rate, score delta, Elo delta, confidence interval, or significance value to
report here.

## What was decided

Ship the two evaluator-only arms and keep the production slot-keyed veto
unchanged. A deployment-shaped paired batch must compare
`advanced_great_work_veto_by_district` with `advanced_target_science` before
the classifier can be promoted. The Culture debt arm is a separate reachability
comparison against `advanced_target_culture`; it does not license the district
veto, nor does either structural result claim a gameplay improvement.
