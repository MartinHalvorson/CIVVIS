# `--rival-war-pace`: a harness that goes to war like the live game

`gene_screen --rival-war-pace PCT` makes the rival chairs open wars on the
measured seats with the declaration hazard the live Firaxis AI shows against
ours. PCT scales the hazard's odds; 100 is the fitted model unchanged. The flag
needs `--rivals`. Without it nothing is wrapped and every game plays exactly as
before.

## Why

At Standard speed the calibrated harness (`--rivals firaxis-faithful
--handicap rivals --rival-chairs 5 --rival-rung -2 --rival-pace --difficulty
immortal --speed standard`) is close to a peaceful world:

| measure | harness, Standard, t220-250 (11-04 paired reads, 6 arms × 16) | live Gran Colombia, Standard Immortal (26 games, 11-04 10:44Z-18:30Z) |
|---|---|---|
| cities taken per game | 0.00-0.06 | 0.15 by t100, 0.69 by t250 |
| a war in the game | 3 of 16 trajectories, the first at t175 | 19% by t60, 59% by t100, 70% by t150, 96% by t250 |
| war starts per game | — | 0.67 by t100, 2.56 by t250 |
| cities lost per game | ~0.5 at the end | 0.46 by t100, 1.54 by t250 |

A Standard paired read therefore prices walls, deterrence, garrisons,
generals and siege genes in a game where nobody attacks: defence looks like
pure cost and the war genes barely act. Online reads (0.3-0.9 cities taken)
were closer to live.

## Model

The rival chair rolls once a turn for each measured seat it has met and is at
peace with, before its own AI moves. The roll uses -5f's 10-turn declaration
logistic, fitted on 68 live games and 119 wars (AUC 0.83, PR #4344):

```text
logit = −4.96
      + 1.53 · [one of its cities within 7 tiles of one of ours]  (8-10 tiles: +0.24)
      + 1.15 · min(n, 4) / 4    n = its land soldiers within 4 tiles of our cities
      + 0.24 · [a denouncement running either way]
      − 1.41 · [friends]
      + 0.13 · ln(its military power / ours)
      + 0.79 · [Standard turn 320 or later, on this speed's clock]
      + ln(PCT / 100)
p(turn) = 1 − (1 − sigmoid(logit))^(1/10)
```

Proximity and soldiers carry the model; the plain power ratio barely predicted
a declaration live. The hazard counts RAW turns, because live wars on our seat
landed on the same raw turn at Online and Standard (t42.5 against t43).

The draw is a hash of (seed, turn, rival, measured seat), never the game's own
RNG. Paired arms on one seed therefore see the same draws wherever their
states agree. A declaration is a plain `Action::DeclareWar`, which the engine
may refuse (for example while a peace treaty runs). After that the rival
chair's own AI prosecutes the war.

## Rows

With the flag, or with `--trace`, each measured seat's row carries:

- `war_starts`: the turns the seat entered a war with another major, whoever
  declared it. This matches the live ladder's war census.
- `rival_war_turns`: the turns a rival chair declared on it through the knob.
- `cities_lost_100` and `cities_taken_100`: the live ladder's raw turn-100
  marks.

## Calibration

See the PR for the PCT that matches the live curve above.
