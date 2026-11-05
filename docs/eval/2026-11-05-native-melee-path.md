# Observe native melee movement paths

The qualified native control `civvis-20261105T155735Z-deterrence-matched-01-on`
used source `a4884eab8765382ef1c928c6d9e4f41e9bd599b1`. Its actual readbacks show:

- Turn 40/frame 0: Warrior 720902 is in Yangsan at (7,23), 60 HP, two moves
  and one attack remaining. The emitted order is `ATTACK (8,23)`, targeting
  Archer 5308428. The native combat preview predicts 29 damage to the Warrior
  and 31 to the Archer.
- Turn 40/frame 1: the Warrior is at (8,24), still 60 HP with one move and one
  attack remaining. The Archer remains at (8,23), still 100 HP. The next
  emitted order moves the Warrior to (7,25).
- The native ledger later reports the original attack as `target_unharmed`.
  On turn 42, a different hostile Warrior destroys Yangsan's city center; the
  city disappears in turn 43. This does not establish a sufficient prevention.

An emitted melee `ATTACK` can therefore resolve as an approach rather than the
modeled strike. The old `strike` event contains damage preview and target but
not the native movement path or exact attack modifiers, leaving the reason
unresolved.

The event now records the exact modifier argument, origin/movement/attack
readbacks, and a bounded summary of `UnitManager.GetMoveToPathEx(unit, target)`:
length, first step, destination/turn, whether that destination matches the order,
and the last path plot available this turn. It reads the UI movement path; it
does not call that path a guarantee of melee combat.

Native reference: shipped `Base/Assets/UI/WorldInput.lua:961` requests the same
movement path; lines 1052-1054 distinguish a later-turn attack destination,
and 1067-1080 display aligned plot/turn entries. The existing attack modifier
contract follows `Base/Assets/UI/Civ6Common.lua:151-163`.

Missing APIs, exceptions, malformed or sparse arrays, unreadable plot positions,
nonfinite numbers and oversized paths remain explicitly unknown. A valid empty
answer reports length zero without inventing a destination. Ranged strikes do
not query this path. The request target, modifiers, verdict and number of combat
simulations remain unchanged; the observer adds one path query to a melee
request. It introduces no gene, rating or tactical-policy change.

The Lua 5.1 regression fixture executes the production melee handler and checks
its actual request arguments, observed approaches, same-turn paths, mismatched
endpoints and unknown cases. Validation results are recorded in PR #4605.
