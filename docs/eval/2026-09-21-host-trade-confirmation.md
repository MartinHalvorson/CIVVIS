# Wait for native trade settlement before spending proceeds

Native run `civvis-20260921T095426Z` at revision `bdd4ca620` recorded 22
trade offers and 16 expirations, with no `deal_response` events. At turn 108,
the planner simulated an iron sale for 394 Gold, bought a Monument, then
requested an Archer-to-Crossbowman upgrade whose observed cost was 125 Gold.
The host had 125 Gold before the purchase and 5
afterward; the next turn reported both `no_deal_response` for the sale and
`same_unit_type` for the upgrade. A submitted sale did not fund that upgrade.

The harness deliberately defaults interactive deal sessions off because an
unanswered session previously wedged the native core. This change leaves that
setting intact. The shipped `Base/Assets/UI/DiplomacyActionView.lua:2582`
explains that AI deal actions carry ACCEPT/REJECT verdicts during evaluation.
The local bridge distinguishes its `deal_offer` submission from a later
`deal_response` and `deal_closed`; a planning clone must preserve that boundary.

For the observed local seat, a validated `Action::Trade` remains in the action
log for export but does not settle the exchange in the planning world. Gold,
resources, favors, passage, and completed-trade counters wait for observed
host state. The simulator and other simulated seats still settle validated
trades normally. Invalid requests still fail validation.

Validation is in progress. This does not repair native deal sessions, establish
that a pending offer will succeed, or prove a native Domination victory.
