# Treasury buys the first Builder near local work

`treasury-at-work-2` asks whether the empire has any Builder work, then buys
its first Builder in the city producing the least. A fully improved city can
therefore receive the purchase while all outstanding jobs belong to another
city. `treasury-at-work-2-2` keeps the working reserve, upkeep check and Monument
fallback, and restricts that first Builder to cities with a legal improvement
or pillaged improvement in their own currently claimed tiles. A city attacked
within four turns, or under the existing barbarian alarm, is skipped.

This is a locality heuristic, not a pathfinding or safety guarantee for every
job. It does not forecast roads, embarkation, future borders or unseen threats.
City production is evaluated once per city for the successor's purchase ordering. The
existing version retains its behavior. The successor is exclusive and defaults off; the
deployment selection stays unchanged.

The original published tag is literally `treasury-at-work-2`; the registry
therefore recognizes `treasury-at-work-2-2` as its second version. This keeps
its existing history and guarantees the screen draws at most one of them.

## Predeclared evaluation

Before collecting results, the activation probe is fixed at six complete
Emperor games, seeds 914357400–914357405, two workers, with both treasury
versions in the screen because the existing version defaults on. Use the
standard six-player, 74×46 Continents, nine-city-state, Online-speed 250-turn
shape with every victory condition and standard genome probabilities.

The probe establishes execution and records uncertainty; six games cannot
establish superiority. A follow-up comparison may be scheduled on disjoint
seeds after host capacity becomes available, without selecting its size or
seeds from the direction of this probe's results. No default change is part
of this task.
