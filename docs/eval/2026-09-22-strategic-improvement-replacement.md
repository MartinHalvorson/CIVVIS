# Connect strategic resources beneath existing improvements

Native match `civvis-20260922T004732Z` used source `a8c02a2468cd47a64583e127fe4e44171bc4d7d5`. It owned Aluminum at (23,32) and (21,31), both covered by Haciendas. The planner requested mines at turns 183 and 187. The host refused both requests while the Builders had movement and charges: respectively one move/two charges and two moves/four charges. The refusal events identify the existing Hacienda and our ownership. At turn 198 Aluminum income remained zero and there were no Bombers. Raising the Builder score would repeat a request that already failed in execution.

The actuator now tries the named improvement first. If the host refuses, it may request removal of an existing non-extracting improvement on the Builder's own tile, then queue the original named build. This requires an owned, revealed strategic deposit; the requested improvement must connect it according to the shipped database and the existing improvement must not. Movement, charges, the order queue and the native removal legality check are required. Hidden resources, foreign tiles, already connected deposits and unrelated improvements do not enter this path.

The retry is inserted before later planned Builder orders and waits through the queue's existing native-operation settlement barrier. Removing a Hacienda does not emit an `improved` event for a mine. If the later build cannot start, the retry does not invoke an arbitrary-improvement fallback or blacklist the deposit; a later observed board can replan it. This matters if removal uses the remaining movement.

## Shipped API evidence

- `Base/Assets/Gameplay/Data/UnitOperations.xml:37` registers `UNITOPERATION_REMOVE_IMPROVEMENT`; line 96 makes it a visible BUILD operation without an interface mode.
- `Base/Assets/UI/Panels/UnitPanel.lua:2518-2535` requests operations without an interface mode using `UnitManager.RequestOperation(unit, actionHash)` with exactly two arguments.
- `Base/Assets/UI/Panels/UnitPanel.lua:615` checks parameterless operations with `UnitManager.CanStartOperation(unit, actionHash, nil, false, false)`. The existing actuator wrapper uses these forms.
- `Improvement_ValidResources` supplies the requested/existing improvement-to-resource relation. The existing `visibleResourceName` additionally checks the resource's prerequisite technology/civic, because the raw map API can expose unrevealed deposits.

## Validation and limits

The new Lua behavior test runs the shipped actuator and its queue against a simulated host. It covers asynchronous removal, mine ordering before later Builder actions, movement exhaustion without site blacklisting, direct-build preference, refusal/throw handling, and ownership/resource/charge/visibility guards. The original source fails the replacement assertion. All 57 discovered Lua suites pass; 1,461 controller Python tests complete successfully with one skip. Lua 5.1 compilation and whitespace checks pass.

These checks prove the requested call sequence under the simulated API contract, not native removal acceptance or Aluminum income. A subsequent native match must verify removal, the completed mine, resource income, aircraft construction and conquest results. No native victory is claimed. Raw evidence is preserved locally in `/tmp/civvis-004732-aluminum-audit.json`; validation logs use `/tmp/civvis-3711-*`.
