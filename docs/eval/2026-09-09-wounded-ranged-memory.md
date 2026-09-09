# Remembered shore guns remain threats to wounded ships

In `civvis-20260909T025914Z`, Galley 851973 saw Field Cannon 9109537 at
(59,7) on turn 134 frame 0. After losing sight of it, the Galley held at
(61,7), 31 HP, on turn 136. The same cannon, still at the recorded (59,7),
then dealt the lethal 31 damage. Earlier, a Ranger at (62,9) had reduced the
Galley from 88 to 31 HP while it stood at (62,8).

The selected `wounded-out-of-reach` policy reused a civilian capture envelope
for remembered threats. A land unit's capture envelope contains land tiles,
so a remembered land gun did not make the ship's water tile dangerous. A
persistent replay through the preceding prepass repair still produced no
Galley order on turn 136. A synthetic regression with a genuinely observed,
then hidden, field cannon likewise returned no withdrawal. The gene-disabled,
unseen, and expired-memory controls passed before the repair.

This change adds a separate remembered firing projection inside the military
withdrawal policy. It keeps the existing four-turn memory window and the
capture memory's one extra hex of uncertainty per elapsed turn, adds the
unit's base firing range to its base movement allowance, and allows that
firing reach to cross a shoreline. Exact visible enemies remain priced by
the existing attack envelopes. The projection reads recorded positions and
static unit rules, never an unseen unit's actual current position or health.

If every reachable tile remains inside a remembered firing projection, the
withdrawal can improve separation from its edge instead of considering all
such tiles equally dangerous and holding. Exact visible incoming damage,
garrison and screening priorities retain their place ahead of this comparison.
A kill forecast excludes only the units that the simulated shot removed from
this firing projection. Their actual observation history stays intact, and a
different remembered gun still counts. The final control initially caught a
regression here: the projection retained the simulated casualty and withdrew a
ship that could finish its only threat. The corrected forecast permits that shot.
No deployment selection changes, and civilian capture calculations are unchanged.

The native observation also lacked a tile-visibility check: `unit_visible_to`
checks stealth detection and returns true for an ordinary non-stealth unit even
outside current sight. Both hostile observation lists now require the current
player vision frame as well. The hidden-position control invokes the observer
before withdrawal and verifies that neither an unseen position nor its timestamp
replaces the last actual sighting; a never-seen gun enters neither observation
list. This corrects the shared observation used by hostile memory.

This is a bounded uncertainty projection, not an exact prediction of a hidden
unit's move, promotions, or line of sight. It repairs remembered firing coverage
for the existing withdrawal triggers; it does not add remembered damage totals
to the roll-top trigger for otherwise healthy melee units.

The initial combined full suite passed 3,378 tests with 50 ignored, and the
changed-line formatting/clippy gate passed. Six targeted controls now cover
wounded ship withdrawal, both memory versions, off/unseen/expired/future/peace
and melee-only controls, hidden-position independence through the observer,
never-seen units, and the only-threat shot. Final validation follows the shot
forecast correction.

A same-binary persistent replay of the recorded encounter leaves the Galley
holding when withdrawal is explicitly withheld; the repaired deployed policy
moves through (62,7), (63,7), (64,7), and (65,7). Evidence and exact source/binary
hashes are retained in `025914-shore-cannon-repaired/provenance.json` under the
local tactical evidence directory. This pre-final-control binary was built from
`c6fba0ba0`; the final revision is replayed again before shipping.

A fixed-observation replay is not a counterfactual host game or proof of
improved wins.
