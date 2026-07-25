# Rating: measuring which strategy is actually better

`civvis rating` is the measurement layer under the strategy league. It exists
because a rating that nobody audits is a number, not a measurement — and when
the exhibition's ratings were audited, they turned out to carry no information
at all.

```bash
civvis rating --dir league/                  # rate a history, print the table
civvis rating --dir league/ --backtest       # score this system against the others
civvis rating --dir league/ --stages         # what each placement stage is worth
civvis rating --dir league/ --sweep          # tune the stage weighting
civvis rating --dir league/ --seats 6        # restrict to one table size
```

Every number `--backtest` prints is out of sample. Each system forecasts a
game, is scored, and only then is told what happened, exactly as it would run
live. Nothing below asks to be taken on trust.

## What the audit found

Replaying the live exhibition's own match history through the rating system
the league runs today:

| rating system | winner LL | info/game | pair LL |
| --- | --- | --- | --- |
| glicko-2 (league today) | 1.8168 | **−0.025** | 0.6913 |
| random guessing | 1.7918 | 0.0000 | 0.6931 |

Negative information means the ratings were *worse than knowing nothing*. The
league's own `calibration.csv` had been reporting the same thing for a
thousand rounds — cumulative Brier 0.2515 against 0.2500 for a coin flip — but
a pairwise Brier score sitting a thousandth above the floor does not look like
a failure until you put the floor next to it.

Two separate causes, which need two separate fixes.

### Most of a finishing order is noise

A six-player game is scored by decomposing its finishing order into all
fifteen pairwise comparisons, each carrying equal weight. But only the first
of those is decided by winning; the rest are decided by an engine score that
barely separates strategies.

`--stages` measures how much each placement decision is worth, in nats:

```
stage 1 (who won)       +0.5607   ############################
stage 2                 +0.1685   ########
stage 3                 +0.2456   ############
stage 4                 -0.0064
stage 5                 -0.0024
```

The last two stages are at or below zero: they are coin flips. Weighting them
like the first buries one real observation under four fake ones. This system
weights stage `k` by a geometric `--stage-decay` (0.5 by default, so the
winner carries about half the update), and `--sweep` re-tunes it against a
real history.

### A seat is not just a strategy

Which civilization a seat drew moves the result at least as much as which
strategy plays it. In the observed history Rome won 37.7% of the games it
appeared in and Sumeria 14.6% — a spread wider than the one between the
strategies being rated.

That alone would be survivable, because the league's mirrored batch rounds
rotate civs. What is not survivable is how the live exhibition seated players:
each civ went to whichever strategy had the best rating *on that civ*, which
is a feedback loop. In the last 200 games of the audited history, one strategy
played Rome **200 times out of 200**. Its rating led the table at 1873 while
its actual winrate on Rome was 25%, against 56% for the strategy rated 204
points below it.

Strategy and civilization had become the same variable. No estimator can undo
that, which is why the fix has two halves:

- **In the model**: a seat's strength is `skill[player] + edge[civ]`, so a
  rating means "how strong is this player, net of what it drew". The civ edge
  is one shared quantity every game helps estimate, not a sparse per-strategy
  side table.
- **In the seating**: `rating::rotate_seating` is a Latin square — over a full
  cycle every player draws every civ the same number of times. Balanced
  seating is a *precondition* for the civ term helping rather than hurting,
  and the backtest shows exactly that: on the confounded history the civ term
  makes forecasts slightly worse, because it splits one confounded signal into
  two noisier halves.

## How the model works

Each player and each civilization holds a Gaussian belief over its strength in
logits; `ELO_PER_LOGIT` is `400 / ln 10`, so the numbers print on the familiar
1500-centred scale and stay comparable with `league.rs`.

A finished game is read as a Plackett–Luce ranking: stage `k` asks which of
the seats not yet placed finished next. Each stage's likelihood is turned into
a Gaussian observation of the seat's strength by a Laplace approximation, and
folded in with an **exact Kalman gain** for an observation of a sum:

```
S      = var(player) + var(civ) + noise
player += (var(player) / S) · innovation
civ    += (var(civ)    / S) · innovation
```

Each side moves in proportion to how uncertain it already is. A settled civ
edge barely budges while an unrated newcomer absorbs the surprise — the
behaviour you want, and one a separate per-civ rating table cannot produce.

Everything a game contributes is accumulated as a mean shift and a precision
gain *against the beliefs held before the game*, then committed at once. So
the update does not depend on the order seats are visited in, a tie moves
nobody, and two games in the same period commute.

Other properties worth knowing:

- **Uncertainty is attenuated, not ignored.** Strength differences are shrunk
  by Glicko's `g(φ)` using the field's variance, so a forecast between two
  barely-rated players stays near even money instead of confidently wrong.
- **A rating never becomes certain.** `beta` is the irreducible per-game luck
  and `min_rd` a hard floor, so a long history cannot drive deviation to zero
  and freeze the table.
- **A rating can follow a player that changes.** `drift` adds uncertainty per
  game played, which matters here because the thing being rated is a genome
  that gets bred and an engine that ships under a live exhibition.
- **Anchors pin the scale.** `--anchors advanced,basic` holds those players'
  mean rating fixed, so a leader's margin stays comparable across hundreds of
  rounds even after every founder has been replaced.

## Reading a backtest honestly

The report prints a `(random guess)` row on every table. Read it first.

- **`info/game` is the only line that matters.** It is `ln(seats) −
  winner LL`: how many nats the system knew that guessing did not. At or below
  zero the ratings are decoration.
- **The uniform baseline is not a constant.** It is `ln(seats)`, and a league
  directory can hold 2-, 4-, and 6-seat games at once. Use `--seats N` before
  comparing anything.
- **A model can only be as good as the games.** On the audited 6-seat
  exhibition history *every* system scores at or below guessing — and so does
  a batch fit with full hindsight (−0.006 nats). That is not an estimator
  failing. It is six near-identical bred genomes, seated confoundedly, playing
  games that 62% of the time hit the turn cap and resolve on score. There is
  nothing there to measure. On the same league's 4-seat games, where the field
  is more varied, this system extracts +0.050 nats against glicko-2's +0.043.

The lesson generalises: **when ratings stop separating, fix the experiment
before the estimator.** Balanced seating, a roster with real spread, and
games that end decisively are what make a rating mean something.

## Where the numbers live

`--backtest` and `--stages` read `matches.csv` from a league directory, which
records every finished game as `round,seed,turns,victory,placements` with
placements in finishing order as `name@Civ|name@Civ|...`. Anything the rest of
the pipeline adds after the civ is preserved, so richer histories still load.

See [LEAGUE.md](LEAGUE.md) for the league that produces those games and the
Glicko-2 system this one is measured against.
