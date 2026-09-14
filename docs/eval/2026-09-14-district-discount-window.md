# Lock an expiring district discount

## Strategy and engine audit

Victoria's [district-discount analysis](https://forums.civfanatics.com/resources/civ-vi-district-discounts.27783/)
(accessed 2026-09-14) explains why district unlock order matters: completing
research can increase the number of available district families and remove
an underbuilt family's discount. Placement locks the cost. This is expert
strategy advice, not a controlled estimate of the resulting win-rate gain.

CIVVIS already models those mechanisms in `district_underbuilt_discount`,
`district_cost_for_placement` and `item_cost_for_city`. Its ordinary production
scorer uses today's cost but does not value the possibility that a discount
will expire. Existing district planning coordinates sites and construction
order. The new rule handles the price deadline without suspending research
or placing speculative foundations by switching queues twice.

## Independently screenable hypothesis

`lock-expiring-district-discount` is a default-off opt-in. The ordinary
production shortlist calls it after scoring items and applying policy-card
preferences. It operates only in an idle, loyal, peaceful city without a
recent attack or a threatened-city, Recovery or Conquest posture.

A currently selected technology or civic must be at least 90% complete and
unlock a specialty district. This is a progress window, not a claim that
research must complete on the next turn. Only legal, unplaced districts
scoring at least 85% of the ordinary best item are considered. Existing
foundations have already locked their costs and receive no premium.

One forecast per qualifying menu grants the pending unlocks on a private
copy. Both current and future discounts come from the engine's existing
function; its visibility expands to the crate, with no game-rule change.
The forecast must show an actual discount reduction and a higher placement
cost. Growth in the ordinary progress-based price alone does not qualify.
The district score increases by the future/current price ratio, capped at
18%. This can decide a close build choice but cannot rescue low-value filler.
The thresholds and cap are hypotheses, not fitted optimum values.

The forecast removes observed host menu prices because those quote today's
trees. Before trusting it, the native model must reproduce the current
actual quote within one production point. A discrepancy leaves the scores
unchanged. Agreement today cannot prove every future host modifier is modeled;
the rule remains an opt-in for that reason as well as unmeasured strength.

Cost and discount queries retain unique-district pricing, the Government
Plaza's smaller discount, completed districts and existing foundations. A
normal `Produce` action locks the chosen district's price. Research and
existing emergency/expansion reservations retain their own authority.

## Verification plan

Tests cover a close decision, rejected filler, the real production-shortlist
hook, technology and civic unlocks, stable discounts, active queues,
defensive priorities, Government Plaza pricing, and matching/mismatched host
quotes. A controlled normal-turn scenario places the district before research
completes and compares its locked price with a clone that waits to place it.

A preregistered six-game smoke probe uses seeds 914358000–914358005 and
`--genes lock-expiring-district-discount --difficulty emperor`: six majors,
74x46 Continents, nine city-states, Online speed, all victory conditions,
250-turn clock. The probe is an execution check and is excluded from the
promotion ledger. It is not sized to establish a win-rate improvement.

Results will be recorded after execution.
