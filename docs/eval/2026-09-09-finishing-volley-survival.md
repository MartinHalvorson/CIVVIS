# Survival policy before live finishing volleys

## Observed failure and bypass

Verification run `civvis-20260909T003711Z`, turn 30: warrior 196608 began at 56 HP on (19,30). Healthy warrior 131073 attacked first and weakened barbarian warrior 4784130 from its exported 50 HP to 16 HP at (18,31). Our warrior killed it, took 23 retaliation, and ended on that tile with 33 HP. Visible archer 5308443 at (18,33) then killed the warrior. Coordinates here are host offset coordinates.

A cold replay of the exported frame, including its following map chunks, still ordered the fatal attack even with `doomed-blow-veto-2` forced on. Its note identified `war_unit_finishing_volleys=1 attacks=2 reserves=0`. The cause was a separate live bridge pre-pass: `finish_live_war_units_excluding` applies finishing attacks before `AdvancedAi::take_turn`. The battle planner's survival checks cannot reconsider strikes already applied by that pre-pass. Its candidate proof required survival of immediate retaliation, but did not price the enemy reply.

## Change

Expose the controller's selected survival rule to the live finishing pre-pass. With either `doomed-blow-veto` version enabled, replay the complete proposed volley on a speculative board and evaluate each striker's remaining HP and reply danger there. The same battle-planner danger field, reach flag, and wounded-striker threshold are used. A later friendly kill can remove the threat to an earlier striker. A rejected volley leaves units available to the ordinary tactical planner.

The one optional reserve also needs this check. Its intended contingency is a host target surviving the primary shot, so evaluate the reserve's own candidate on the original board where that target exists. An unsafe reserve does not consume the guard's moves or attack, and the first safe alternative can still be retained.

The note `war_unit_finishing_survival_rejections=N` records rejections. With both survival genes off, the helper returns immediately without simulating actions. This fixes a policy bypass; it does not promote either gene or claim that all finishing attacks should be forbidden.

## Controls

The exposed-melee control failed before the check was implemented; its no-archer control passed. The first six focused controls then passed: unsafe finisher, safe finisher, version-one coverage, unsafe melee reserve, safe reserve, and a complete volley whose final kill rescues the wounded first shooter's otherwise unsafe prefix.

## Recorded frame replay

The same new binary was run on turn 30/frame 0 with the six recorded live tags, then with each survival version. Default still produced finishing attacks in order **131073 → 196608**. Both survival versions recorded one pre-pass rejection; the ordinary planner instead ordered **196608 → 131073**. Comparing only whether unit 196608 still attacked would miss the effect: the *last* melee striker takes the defeated enemy's tile.

A separate native simulation of those two action sequences confirmed the modeled consequence:

| Sequence | Warrior 196608 (started 56 HP) | Warrior 131073 (started 100 HP) | Survival check |
|---|---|---|---|
| Original, healthy first | 30 HP at exposed (18,31) | 75 HP at (18,30) | rejects |
| Revised, wounded first | 27 HP at city (19,30) | 77 HP at exposed (18,31) | accepts |

These are native simulated HP values, not host counterfactual measurements. The recorded original host result was 33 HP for the exposed wounded warrior, subsequently killed by the archer. The replay demonstrates policy routing and a changed assignment of the exposed finishing blow; it does not prove the revised game's outcome.

Replay binary SHA-256: `4b6582cc36964d2ee3632321c4d24fb686127a26fdf508f79c8a365df2da9e00`.

A second recorded failure, `civvis-20260909T002110Z` turn 82/frame 0, had archer 983047 shoot bombard 3342344, then warrior 131073 die attacking it. The revised default replay still sends those two attacks. With version 2 enabled, the replay rejects the pre-pass, sends the city strike at (54,23), and fortifies warrior 131073 instead. The archer takes a promotion. This removes the recorded fatal melee order; whether the city shot kills the bombard in the real host remains unproven.

Final validation will be recorded after completion.

## Limits

This uses the existing native damage and reach model. It does not repair host/native damage discrepancies, predict unseen enemies, or establish a counterfactual game outcome. Broader performance remains subject to the earlier gene evaluation; no default change is proposed.
