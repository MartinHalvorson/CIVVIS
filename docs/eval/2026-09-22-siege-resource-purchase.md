# Siege resource purchase experiment

The native match `civvis-20260921T233717Z`, source `48e2388ce`, knew Metal Casting by turn 100 but lacked Niter income until turn 140. Niter at (35,16), within three hexes of Bogotá and adjacent to its territory, remained unowned until turn 146. The modeled purchase price at turn 110 was 95 Gold against a treasury of 141; a charged Builder was nearby.

Prototype: during a Domination conquest, buy a legal, connectable first deposit needed by an existing siege unit's researched upgrade. A nearby charged Builder must have a route to it and the existing civilian destination risk check must pass. Home threats veto the purchase; retain 40 Gold plus six turns of any deficit. This intentionally relaxes the ordinary reserve and 200-Gold surplus requirement to acquire the resource needed to make the army useful.

Validation is in progress. The modeled purchase opportunity is not proof of a host purchase, connection, upgrade or capture. The live bridge retains its CanStartCommand check. Tests and a paired recorded-game replay must establish behavior before this experiment is ready to ship.
