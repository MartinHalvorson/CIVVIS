# Modern siege reserve

## Observed failure

Native run `civvis-20260921T073539Z`, pinned to `aab45a2c3`, reached turn
132 with military strength 580 against the Netherlands’ 22. Utrecht still
had 238/300 wall HP and 192/200 city HP. Two Catapults and four Crossbowmen
were attacking; a Catapult hit dealt only 9 wall damage and 2 city damage.
Metal Casting and Ballistics were known, Niter stock was 25, but treasury
was zero and the observed Catapult upgrade cost 165 Gold. All nine city
queues contained buildings, districts, or projects. Air production was
still seven technologies away when the surge was appointed on turn 124.

`missing_domination_siege` previously rejected every additional siege
weapon once any non-air siege unit existed. Consequently, the composition
exception to the army ceiling treated old Catapults as sufficient forever.

## Change

Track the strongest fielded or queued land siege weapon. A Domination
campaign against a known walled city at war may reserve one replacement
whose base attack strength exceeds that value by at least 10. This is a
meaningful technology step rather than a general increase to the army cap.
The existing production score, legal production/resource checks, and
emergency priorities remain in control.

Fielded formation and support bonuses count. Base strength also counts so
wounds cannot create replacement demand. Queued formations include their
formation strength. A queued modern weapon closes the exception in other
cities; the existing queue-excluded governor census keeps its own
replacement eligible, including a targetless defensive replan while hostile
walls remain known. Started useful investments retain the governor's normal
commitment protection; zero-progress orders still compete with other priorities. Peace, unwalled targets, and non-Domination goals do not gain
this reserve.

The change reserves one modern weapon; it does not claim that one weapon
is enough to capture every city. Comparing candidate base strength to a
fielded unit’s effective strength is conservative when bonuses are active.

## Validation

The matched replay uses all 367 decision frames from the same frozen native
event stream and identical forced treatments. Baseline has zero immediate
Trebuchet production orders; candidate has two, at 118/0 (city 196610) and
119/0 (city 65536). Trebuchet `produce_next` proposals rise from 3 to 14.
There are 13 changed exported frames and 19 changed internal action frames;
other changes include competing city production assignments and one policy
deck. Next-item proposals are independently rescored, so multiple such
proposals are not evidence of multiple active siege queues.

Neither version issues a Bombard order. By turn 131 the planning journal
reports 0 Gold at -9.6/turn and chooses upkeep-free recovery projects. The
host reports -9.58594 Gold/turn in that same 131/0 decision frame. Its
slightly positive 0.917969 reading comes later, at 132/1, after the last
recorded decision. Comparing matching frames confirms that the mirror imports
income correctly; this is a financing constraint, not an income-import defect.
This patch preserves the recovery gate; raising military priority cannot
bypass that gate.

These are proposed orders against recorded states, not executed production,
city captures, or a counterfactual victory. The source run ends at 132 with
no recorded outcome, and the frozen board does not execute candidate orders.

Eight simulator soak games complete (four players, seeds 364300–364307,
180-turn limit, four workers). This checks stability, not native win rate.
All six focused regressions pass within `cargo test --profile ci --locked`:
3,781 library tests plus 206 other tests pass (49 library and four doc tests
ignored). Formatting, whitespace checks, and all 14 treatment append-point
policy tests pass. Initial fixture assertions incorrectly required a new
Bombard to supersede basic infrastructure; the final tests instead verify
the composition exception, its ordinary-ceiling control, queue exclusion,
started-investment retention, formations, wounds, and scope exclusions.
