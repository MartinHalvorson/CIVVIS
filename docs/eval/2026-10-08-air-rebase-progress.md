# Award the air-rebase reach bonus only when it adds reach

The ordinary air selector rewards distance improvement by 18 per tile and
adds 35 when a destination base puts the objective within attack range. The
old calculation adds that bonus even when the starting base already reaches
the objective. With no profitable mission, an equally distant base scores 35
and a base one tile farther away scores 17. Either can spend the aircraft's
turn without adding reach. The same score can displace a Fighter's patrol.

The correction awards the bonus only when a current controller
crosses from outside attack range to inside it. Positive distance improvement
still permits forward staging within range or while still outside range.
The frozen `AdvancedAi::legacy()` controller must retain its original score.
Legal-action generation, base capacity, loyalty filtering, strike and pillage
valuation, and the existing Domination siege forecast retain their roles.
The emergency loyalty evacuation pass runs separately before combat and may
still rebase away from the front.

## Selector evidence

The focused fixtures exercise `advanced_air_action` on an actual `Game`, with
a city base, a second owned Airstrip, an enemy city objective and real engine
action legality. A fogged city objective gives the Bomber no profitable
mission. Regression cases cover equally distant and backward in-range bases.
Controls cover closer in-range staging, newly reaching the objective, forward
staging outside range, a profitable immediate kill, and a full Airstrip.
A Fighter control checks actual patrol execution when a sideways rebase
would previously win its score comparison. Executed rebase controls verify
the destination and the engine's consumed movement and attack allowance.

The unchanged scoring at `dc09134d93778665561fcce80a18e669a69a1e5e`
reproduced the backward Bomber rebase and sideways Fighter rebase in CI run
`37860043925`. All five forward-staging, kill and capacity controls passed.
Fail-fast stopped that run before the equal-distance Bomber case; it is not
credited as executed. The local baseline compilation was intentionally
stopped after that independent reproduction, before editing its source.

Candidate validation adds frozen Bomber and Fighter anchor checks and a
three-turn comparison using remembered objectives on `player_decision_view`:
the legacy Bomber cycles through the two bases using actual rebase and
EndTurn actions, while the current selector keeps its original base.
Candidate results will be recorded after the focused run finishes.
The existing siege-forecast and loyalty-evacuation suites are additional
controls, alongside the full repository regression and required CI cost gate.

## Native boundary

The native Bomber cycle in `civvis-20261008T221617Z` motivated the air audit.
Its separately pinned private source also contains the ordinary scoring
formula, but the observed Spaceport-directed rebases have a separate private
selector absent from public main. This public change is not attributed as a
repair of those native orders. The frozen identity replay failed complete
native reply conformance; see `2026-10-08-native-replay-control-gates.md`.

No native pin, game policy, mod or private branch is changed. A native
candidate adoption and matched game outcomes remain necessary to measure
the requested Gran Colombia four-player Domination win-rate effect.
