# Gran Colombia conquest milestones — 2026-09-26

The fixed King / four-player / Gran Colombia / Tiny Pangaea paired evaluator
now distinguishes declarations against majors from city-state wars, defensive
war exposure from focal declarations, and original-capital control from other
foreign cities. Schema 2 adds `conquest` to each arm without removing the old
outcome fields. This changes measurement only, not the controller.

The observer runs at turn boundaries and once after the game ends. Every
`*_observed_turn` is the first boundary that saw the event, not its exact action
turn. Declaration counts consume the applied-action log once, including wars
that ended before the next observation. City sets record unique cities held
at observations; captures and losses between observations are not counted.
The final ownership fields distinguish taking a capital from keeping it and
also report whether the focal seat still controls its own original capital.
City-state capitals do not count as foreign major capitals.

## Diagnostic pilot plan

Before running: compare `early-conquest-opening` off/on over four complete
pairs, seeds 37140000–37140003. Preserve every pair, including losses and
identical action histories. All other focal policies use the compiled live
bundle. The outcome of interest is major declarations followed by foreign
capital control and domination wins; score alone cannot qualify improvement.
This small pilot diagnoses exposure and chooses the next experiment. It is
not sufficient to promote or remove a policy. Rivals are CIVVIS controllers
with King bonuses, not Firaxis AI; native verification remains separate.

Results and validation pending.
