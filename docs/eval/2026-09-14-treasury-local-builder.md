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

## Activation results

All six games and all 36 intended seats completed on clean source
`24ae04a3cde44a61479ad1dd51a3526801032b23`, with binary SHA-256
`37a2a3e0fba51e5644be01c268313f1aa432abe45f70ba934a111051c07ef3de`
and gene fingerprint
`ec6ed09ee9a04c6928878900dcefe53e9f624ba9694164523198a43acb6a4383`.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes treasury-at-work-2,treasury-at-work-2-2 \
  --games 6 --start-seed 914357400 --jobs 2 --difficulty emperor --out treasury.jsonl
target/ci/gene_screen --analyze treasury.jsonl \
  --json docs/gene_screens/fires/2026-09-14-treasury-local-builder.json
```

The successor appeared on 13 seats and won twice; the other 23 seats won
four times. Its marginal win difference is -2.01 pp
(SE 16.51) and score-share difference is -2.03 pp
(SE 2.07). Both are unresolved. Other seats include the older
version, so this marginal row is not a family-off-only comparison. Four games
ended in culture, one in diplomacy and one in science.

Eight focused regressions cover locality, repair-only work, the four-turn
attack boundary, barbarian alarm, reserves and fallback, stale ownership,
exclusive defaults and matching live identity. The complete-game probe adds
execution evidence; its size does not justify a strength or default change.
