# Reserved capture-unit pressure: investigation

Not yet validated for shipping. Native run `civvis-20260921T212727Z`, pinned
`82741ed94`, left Man-at-Arms `3080208` at `(33,16)` with 100HP and three moves
at the start of turns 92–94. Teayo `(34,16)` had no walls and healed most ranged
damage; walls appeared at turn 95. The siege doctrine reserves a capture unit
until the city's health fits one blow.

The experiment permits a Domination capture unit with at least 80HP to attack
an unwalled city before the final blow only when speculative damage exceeds
20HP and the immediate loss, and at least 60HP remains after the strike-reach
reply estimate. Reservation stays in place. Standing walls, embarked units,
spent attacks, wounded units, and other victory lanes retain the hold.

Frozen native input through turn100 is at
`/tmp/civvis-taker-pressure-replay/native-events.jsonl`, SHA256
`48cb9cf8bb677a09841391d729e3ee95cffefd1f68c2fdc5e88f2b135551f2df`.
Baseline replay completes283frames. No candidate replay or native capture
benefit has yet been demonstrated. The first focused case fails on old code
and passes after the change; further guards and validation remain in progress.
