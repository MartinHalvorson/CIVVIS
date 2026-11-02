# Science power buys fuel: work in progress

Hypothesis: a Science seat with a ready Coal plant and multiple unpowered
Research Labs can benefit from buying a short fuel bridge while its own
supply is absent. This opt-in proposes a purchase; it does not assert host
acceptance or a strength improvement.

## Native observation and implementation scope

In `civvis-20261102T165603Z` at t150, own Coal/Oil stocks and exported gross
income were zero. India offered 49 Coal and 40 Oil while at peace. Ternate
and Tuban offered Coal plants. A reconstruction from the actual captured
input read found three unpowered Labs within six tiles of each ready plant.
There were no Coal/Oil buy or sell proposals recorded at t140–174.

This is not a free purchase: the mirror valued 20 Coal at 587 Gold against a
258 Gold treasury and approximately 96 Gold/turn. The nearby unowned Coal at
(21,6) was absent from the modeled legal purchase menu, and the three Builders
had modeled routes of 9–12 steps. Those prices/routes are model estimates,
not native quotes or guaranteed legal host actions.

The candidate requires a Science raced target, enabled Science victory,
at least 100 Gold, nonnegative income, no own Coal income and fewer than ten
Coal banked. A Coal plant must be buildable or already stand, with at least
two unpowered Labs within six tiles of its Industrial Zone. Oil/Nuclear
plant cities do not request Coal. Immediate home threats, an excluded fossil
power lane, and fewer than ten turns to the turn cap veto the request.

The request is 20 Coal, minimum 10, using the existing strategic-buy
transport. It preserves the larger of 100 Gold and the upkeep reserve,
retains that transport's cadence, seller availability, finite price ceiling
and income-credit limits, and emits no second buy when another buy already
occupies the frame. No luxuries or strategics are bartered by this request.
The stock gate releases the request after delivery. It is a zero-supply
bootstrap, not a model of every partially fuelled network or a guarantee of
ten powered turns.

The existing native wire contract is documented in
`tools/civ6_control/mod/CivvisControlAgent.lua:14200–14224`: a
`RESOURCE_X=N` buy asks the seller for a clipped strategic block through
`CivvisStrategicAsk`, closing only at or under the ceiling. The candidate
uses that unchanged interface. Actual paid terms and resource delivery must
be read back from native events.

## Validation and promotion gates

Implementation is in progress. Required validation includes:

- Focused tests for ready demand, no demand/plant, another lane, late deadline,
  low/negative finances, and existing Coal stock or income; ensure the gene
  remains off in both controllers.
- A paired query on identical captured native reads, varying only this flag,
  demonstrating the proposed Coal buy, reserve/cadence behavior and controls.
- Native execution evidence: accepted terms, delivered Coal, actual plant
  construction, power/yield changes and ongoing cost, followed by complete
  game outcomes with the rest of the deployed profile held fixed.

This transport acts only on the native bridge. A simulator screen that never
executes the resource purchase cannot establish its strength. Do not credit
the separate first-launch research screen to this candidate. No default
promotion until native execution and win/survival evidence support it.
