# Full-cost expansion investment

The free-Settler oracle has replicated large expansion headroom, but it cannot
say whether a production policy should build a Settler. It removes production
and population cost, and lets the unit appear immediately. Existing treatments
mostly moved the city target or an abstract price; none measured the complete
investment an agent would actually make.

`expansion_investment` is a counterfactual evaluator for that missing question.
At a live major-civilization turn it accepts a candidate only when all of these
are true:

- the empire is short of its current `assess()` city target and has no Settler
  walking;
- the expansion window is still open and `AdvancedAi` can see a viable site;
- the city is owned, population two or higher, has an empty queue, and can
  legally produce a Settler; and
- the candidate is one of the highest-current-production eligible cities.

The treatment applies the ordinary legal `Action::Produce` for a Settler before
the focal AI turn. The unmodified AI then controls every later turn. The engine
therefore charges production and population, and the AI must route, escort,
found, and exploit the city itself. A control branch begins from exactly the
same cloned game and agent memory. Opponent doctrines are rotated across
matched replicas; each branch runs through the normal terminal game result.

This is an evaluator, not a gameplay change. It is deliberately stricter than
the free-unit oracle, but it remains an oracle over the small set of sampled
city choices. Its best-candidate statistic is a mechanism ceiling, never a
license to install a policy.

## Invariants

Every written corpus must satisfy all of the following:

- each forced `Produce(settler)` is legal in the exact pre-turn game state;
- the forced queue survives the focal `AdvancedAi` turn, so it is a real
  production commitment rather than a one-line mutation the AI immediately
  overwrote;
- the repeated control replica is byte-identical in its terminal outcome;
- every continuation reaches an engine winner, including an ordinary score
  victory at the configured turn cap; and
- any rejected action, overwritten queue, repeat mismatch, or no-op corpus
  prevents output from being written.

The emitted CSV preserves every replica's terminal win, terminal score share,
maximum owned founded-city count, final founded-city count, and end turn. This
lets later work distinguish an extra city that pays back from one that merely
exists.

## Fixed screen

The following Standard screen is frozen before data collection. It tests the
two highest-production legal cities at dense, nonduplicating observation points
on 24 independent four-player 44x28 games, seeds `996000` through `996023`.
The output is a mechanism result only; no model, threshold, or gameplay arm is
selected from it.

```text
expansion_investment --games 24 --players 4 --width 44 --height 28 \
  --turns 200 --city-states 0 --warmup 1 --spacing 5 \
  --decisions-per-game 20 --alternatives 2 --replicas 4 \
  --seed 996000 --jobs 8 --out /tmp/expansion-investment-996000.csv
```

The screen is worth a deployment-profile evaluation only if it has at least 20
eligible decisions from at least 12 independent games, passes every integrity
check, and its game-macro **best forced city or control** terminal score-share
delta is positive. That gate is intentionally weak: it only says that the
full-cost mechanism is not already contradicted on the Standard profile. An
external pass would still require a new six-player 74x46 Online corpus, a
positive terminal-score delta, and no terminal-win regression before any
separate policy design could be considered.

## Result

The frozen screen completed with 15 eligible decisions from 11 games, 15 paid
Settler alternatives, and 120 terminal continuations. All 461 other scheduled
observations correctly declined to manufacture a treatment because no legal
live expansion opportunity existed. There were zero rejected branches and zero
repeated-control mismatches.

Across every forced city alternative, terminal score share changed by
`-0.0100 ± 0.0113` (standard error), terminal win rate by `-0.0333 ± 0.0538`,
and peak owned founded cities by `+0.117 ± 0.091`. Three of 15 treatments ever
produced an extra founded city, while two improved terminal score share. The
control-retaining, game-macro best-choice ceiling was `+0.0018 ± 0.0013` score
share and `+0.0076 ± 0.0076` win rate; it is descriptive only, since it keeps
the control result whenever no forced city is better.

This cohort missed both minimum evidence counts (20 decisions and 12 games),
so it does **not** clear the deployment-profile gate. In particular, it does
not justify a production policy change. The artifact and corpus establish that
the full-cost mechanism is measurable without an oracle shortcut; a separately
predeclared, larger cohort is needed before acting on it.
