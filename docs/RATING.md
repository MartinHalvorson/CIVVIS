# Rating: measuring which strategy is actually better

`civvis rating` is the measurement layer under the strategy league. It exists
because a rating that nobody audits is a number, not a measurement — and when
one confounded historical exhibition slice was audited, its ratings carried no
information at all. Current results are profile-dependent rather than a blanket
failure; the corrected replay and its seat-count breakdown appear below.

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

Winner log loss uses the same categorical target as the rating update. A
unique winner has target mass one; an exact tie divides that mass equally
among all co-winners. Its log score is therefore the mean of the co-winners'
log probabilities. Averaging their forecast probabilities before taking the
log is not equivalent: it would reward a model that confidently selects just
one member of a tie. The uniform reference remains `ln(seats)` for either
case, so a uniform forecast—including a fully tied table—has exactly zero
information. Regression tests pin both properties.

> **Implementation correction (2026-07-30).** The original batched update
> summed per-observation posterior mean shifts and therefore applied the prior
> variance once per placement stage. The implementation now accumulates
> Gaussian natural parameters and applies the final posterior variance once.
> The qualitative audit and raw-history findings below still identify the
> right experiment defects. The historical tables below retain the numbers
> recorded with each experiment; the current corrected replay is reported
> separately here rather than silently rewriting them.

On the live runtime history available when the correction landed (822 games
through round 697), `--backtest` evaluates the last 70% strictly out of sample:

| seats | evaluated games | Glicko-2 info/game | corrected staged + civ |
| ---: | ---: | ---: | ---: |
| 4 | 127 | **+0.1612** | +0.1505 |
| 6 | 37 | +0.2074 | **+0.2477** |
| 8 | 365 | −0.0141 | **+0.0104** |
| mixed (4–8, mean 7.0) | 576 | +0.0521 | **+0.0608** |

The old update scored +0.0632 nats/game on that same mixed replay; the
correction removes 0.0024 nats/game of optimistic movement while preserving
the small measured advantage over Glicko. It does not establish a universal
replacement: Glicko remains better on this four-seat slice, the six-seat slice
is small, and the eight-seat history barely carries signal for either system.
That profile dependence is why `--seats` exists.

## What the audit found

Replaying the recorded exhibition match history through the rating system
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

`--stages` measures how much each placement decision is worth, in nats, on a
576-game 6-player league:

```
stage 1 (who won)       +0.4604   ###########################
stage 2                 +0.5530   #################################
stage 3                 +0.3359   ####################
stage 4                 -0.0055
stage 5                 -0.1120
```

The last two stages are at or below zero: they are coin flips, and the last is
actively misleading. Weighting them like the first buries three real
observations under two fake ones. This system weights stage `k` by a geometric
`--stage-decay`, 0.5 by default, so the winner carries about half the update.

**The best decay is a property of the league, not a constant**, which is why
`--sweep` exists instead of a hard-coded number. On that fresh 6-player league
it peaks near 0.1 — almost "rate only who won" — worth 0.591 nats per game
against 0.551 at the default and 0.459 with flat weights. On the recorded
4-seat exhibition games it peaks near 0.7 and is nearly flat across the range.
0.5 ships because it is close to the better end of both; sweep before trusting
it on a new league.

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

Everything a game contributes is accumulated as a Gaussian natural score
(`innovation / noise`) and a precision gain *against the beliefs held before
the game*. All stage likelihoods for one seat are first collapsed into a single
effective observation of its `player + civ` strength, then the Kalman split is
applied once. The final posterior variance converts the score into one mean
shift. Summing each observation's posterior mean shift would apply the prior
variance repeatedly; marginalizing the same civ uncertainty once per stage
would also pretend the context were newly drawn each time. Both errors make a
game with more placement stages spuriously decisive. Closed-form tests now
cover repeated observations with and without a civ context. The corrected
update does not depend on the order seats are visited in, a tie moves nobody,
and observations have diminishing rather than linear influence.

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

## What it scores on a healthy league

576 games of a freshly seeded 6-player league — mirrored seating, a roster
still spanning victory lanes — with 404 scored out of sample:

| rating system | info/game | winner LL | accuracy | pair Brier |
| --- | --- | --- | --- | --- |
| elo, pairwise placements | 0.4677 | 1.3240 | 43.3% | 0.1593 |
| glicko-2, what the league runs today | 0.4723 | 1.3194 | 43.3% | 0.1594 |
| staged, no civ context | 0.5370 | 1.2548 | 42.3% | 0.1597 |
| **staged + civ context** | **0.5507** | **1.2411** | 43.8% | **0.1571** |
| random guessing | 0.0000 | 1.7918 | 16.7% | 0.2500 |

**+0.078 nats per game over the deployed system, about 17% more information**,
and +0.119 (25%) at the swept decay. Both changes contribute: staging is worth
more than the civ term, and the civ term only helps because this league's
mirrored rounds rotate civilizations. On the confounded exhibition history the
same term makes forecasts slightly *worse* — covariate adjustment cannot
rescue an experiment whose treatment assignment is deterministic, and it is
worth knowing that the tool reports this rather than hiding it.

## Where the numbers live

`--backtest` and `--stages` read `matches.csv` from a league directory, which
records every finished game as `round,seed,turns,victory,placements` with
new placements encoded as `player@leader@civ@rank`. Both batch workers and live
spectator games now write the same exact competition ranks, so score ties
survive either ingestion path. Splitting from the right permits an email-style
player identity to contain `@`. Both historical writers remain readable: batch
`player@civ` rows infer rank from finishing order, while live
`player@leader@civ` rows now separate the leader from the civilization instead
of incorrectly treating `leader@civ` as one unseen civ context.

See [LEAGUE.md](LEAGUE.md) for the league that produces those games and the
Glicko-2 system this one is measured against.
