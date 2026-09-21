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

## Validation and limits

All 4,017 Rust tests pass (53 ignored), including six focused tests for native
request logging and unchanged balances, simulator settlement, invalid offers,
purchase affordability before/after observed funds, other simulated seats,
and unconfirmed Open Borders. The affordability fixture starts one Gold below
the real purchase price and offers a sale worth more than that gap. Fourteen
treatment-append tests, formatting, and diff checks pass. Eight four-player
simulator games completed through victory or turn 180 (seeds 0–7, four workers).

The matched native replay uses 482 frozen decision frames through 184/0,
identical runtime flags, persistent AI memory, and a fresh observed board at
each frame. The baseline is the regional-production candidate binary whose
Rust sources, rules, and Cargo files are identical to main `480216769`;
only Python save-recovery changes differ in that source checkout. The candidate
adds this trade-settlement guard to that merged tree.

Twenty frames change exported actionable orders; 26 differ including synthetic
verification receipts, and 23 differ in internal planned actions. Purchases
fall from eight to seven, upgrade requests nine to seven, levies four to zero,
and plot purchases six to zero. All 21 sale and six buy requests remain
identical, as do research, civic, policy, and peace orders. Other differences
include unit movement and production choices with the original money and
resources retained. These are budget-dependent decisions on historical
boards, not executed outcomes or a measured win-rate improvement.

At 108/0 the candidate keeps the iron-sale request and removes the Monument,
upgrade, and levy requests. It does not substitute an immediate upgrade: the
existing modernization reserve still applies. Subsequent frozen boards retain
the historical Monument purchase and depleted treasury, so this replay cannot
measure how actual cash preservation would change later turns. At 120/0 the
candidate retains its iron and requests a Man-at-Arms instead of a Pikeman;
this is an order difference, not a completed unit.

Replay times were 154.03 seconds baseline and 132.21 seconds candidate under
different background load, not a paired performance result. Required CI
measures runtime cost separately. This does not repair native deal sessions,
establish that a pending offer will succeed, or prove a native Domination
victory.
