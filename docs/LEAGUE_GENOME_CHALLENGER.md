# Outcome-selected league genome transfer

The genetic search in `civvis evolve` uses dense score and combat signals to
propose candidates, then an independent win gate decides deployment. The
committed multiplayer league supplies a complementary experiment: its genomes
are rated directly from completed table outcomes over many games.

`strategic_deep_league` transfers the conservatively strongest active,
untargeted `Advanced` genome from `data/league/league.json` into the strongest
measured macro-search budget (`review_every = 20`, `horizon = 80`). The
selection statistic is `rating - 1.96 * rd`; fixed-lane specialists are
excluded so the test changes the general policy, not the victory target.

In the current committed snapshot the selected genome is `g20-21`:

| strategy | rating | RD | games | wins | lower-confidence rating |
| --- | ---: | ---: | ---: | ---: | ---: |
| `g20-21` | 1790.8 | 31.0 | 216 | 82 | 1730.1 |
| `advanced` anchor | 1702.7 | 30.5 | 331 | 91 | 1642.8 |

The league snapshot is definitional provenance. If it is absent or contains
no eligible generalist, the entrant explicitly degrades to `strategic_deep`
instead of reporting a result under an agent that did not play. A focused test
pins the selected snapshot row so a future league update must update this
evidence rather than silently changing the treatment.

## Fresh mirrored screen

The transfer candidate was preselected by its league lower-confidence rating,
then evaluated once on a fresh seed:

```sh
cargo run --profile ci --bin ai_eval -- \
  strategic_deep_league strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 103000 --jobs 12
```

Result:

| metric | `strategic_deep_league` | `strategic_deep` |
| --- | ---: | ---: |
| game wins | 24 | 36 |
| directional maps | 1 | 7 |
| paired score | 40.0% | 60.0% |
| terminal-score diagnostic | 49.0% | 51.0% |
| religious wins | 14 | 26 |

The directional sign test was `p = 0.0703`; the point estimate was -70 Elo.
This does not cross the symmetric promotion gate, but it is plainly not a
positive screen, so no disjoint 120-map gate was spent and the player is not
promoted.

## Interpretation

The transferred genome accumulated more gold, military strength, faith,
production, culture, units, and trade routes than its control. It nevertheless
lost twelve more games, almost entirely through religious conversion: 14
religious wins against 26.

The league evidence was real for `AdvancedAi`; it did not transfer through the
strategic planner. A genome changes both the scripted governor inside each
rollout and the live governor executing the selected plan. A policy that is
strong when it owns victory routing can interfere with a macro search that
already owns that routing. This is evidence against assuming policy rankings
are invariant across agent architectures.

The evaluator-only entrant remains useful as the integration test between the
two evolutionary systems. It is excluded from persistent ratings and from the
production default. When the committed league produces a new conservative
generalist champion, the pinned test and this fresh mirrored comparison make
the transfer question explicit again.
