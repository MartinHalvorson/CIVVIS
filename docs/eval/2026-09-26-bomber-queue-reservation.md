# A launch Bomber keeps an idle queue during deterrence

Native run `civvis-20260926T194755Z` completed Advanced Flight on turn 201.
Cumaná already had an active Aerodrome. Aluminum income became 2 on turn
208, and the host offered a Bomber from turn 209. Nevertheless, the city
started a Ranger on turn 212 and remained occupied with it through turn 227.

An immutable prefix ending at the first turn-212 board records an empty
Cumaná queue, Aluminum stock 8/income 2, 639 Gold, and a 21-turn Bomber.
The mirrored game independently offers and accepts that Bomber. A fresh
late-start controller instead queues a Cuirassier. Withholding only
`peacetime-deterrence` changes that choice to a Bomber. The native controller
had persistent memories; the replay's different land-unit choice is not
claimed to reproduce that history.

The cause is a queue ordering conflict. The peacetime branch of
`redirect_repeatable_projects_for_force_gap` claims all idle cities for land
units before `air_surge_production` runs. Its candidate list excludes aircraft.
The normal air production pass cannot use a queue already occupied by that
earlier pass.

The fix is restricted to an idle city that can legally start a launch Bomber
under the existing Domination air-readiness policy. Its technology, wing
count, home safety, fuel, treasury and completion-time gates remain in force.
Deterrence continues through other cities. Active queues, project redirection,
wartime force gaps and emergency defense retain their existing behavior.
No host bridge, engine rule or deployment defaults change.

## Validation protocol

The regression runs deterrence before air production, matching the live
policy order. It failed before the fix because deterrence consumed the
airfield's queue. Controls exercise the cases where a launch Bomber cannot
be reserved, plus the existing readiness safety tests.

Before implementation, freeze a baseline CLI and evaluator from production
source `1155d4ee6` (test-only checkpoint records the reproducer). Compare the
candidate against that baseline on the immutable turn-212 prefix using
`--serve --fresh-board`, one stdin request and immediate EOF. This offline
process never opens a native orders database or controls the running game.

Run two complete simulator pairs per source with the existing
`--domination-pair air-surge-2` profile, seeds 37820000 and 37820001. The fixed
profile is King, Gran Colombia, four majors, six city-states, 60×38 Tiny
Pangaea, Online, all victories, barbarians and the natural 250-turn clock.
Rivals are CIVVIS controllers, not Firaxis AI. Keep every outcome, including
the off arm, and do not substitute seeds or stop for results. This small
diagnostic does not establish a win-rate improvement or justify promotion.

Artifacts are retained under
`~/civvis-tactics-results/2026-09-26/bomber-queue-reservation/`, with native
prefixes and exploratory evidence in the neighboring `air-readiness/` directory.
Native execution and stronger Domination outcomes remain to be measured at
the verification agent's normal completed-game boundaries.
