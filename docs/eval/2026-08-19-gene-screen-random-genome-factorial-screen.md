# Every native treatment flag priced from one batch of 640 games

_2026-08-19 · `agent/martbot-mbp-m5-max-128/claude-fable-a7710669`_

## What was asked

Fifty-five live-bridge engine repairs, one production treatment and one opt-in
are boolean flags with named withholding twins. The lane prices them one arm at
a time — `live` against `live_without_<flag>`, forty to two hundred maps each —
so pricing all fifty-seven costs upwards of eleven thousand games, and each is
priced against a background in which every *other* repair is on.
`AdvancedAi::enable_engine_repairs` says in its own doc comment why that is
unsatisfying: the repairs are serially coupled, so ablating one from a bundle
that keeps the other fifty-six prices a link inside an otherwise-whole chain.

**Can all of them be priced from one batch, and what does that batch say?**

## How it was measured

`gene_screen` (new; `docs/GENE_SCREEN.md`) runs the classical random-balance
screen with a foldover. Every game seats one treated major carrying a genome
drawn at random — each screened gene on with probability ½ — against a field of
production `advanced`. Games come in **pairs**: the second replays the same map
seed and the same seat with the **complement** genome, so every gene is on in
exactly one arm of every pair and the map's own difficulty cancels out of every
per-gene difference. Every game then informs every gene.

- **300 foldover pairs (600 games)**, seeds 26081900–26082199, 4 majors,
  60×38 Pangaea, 6 city-states, **Online speed to its own 250-turn clock**
  (567 tiles per player — the deployment density; the deployment *seat count*
  is a separate run, below).
- **20 anchor pairs (40 games)**, seeds 26082200–26082219: all screened genes
  ON against all screened genes OFF, same seeds and seats. Excluded from the
  per-gene table.
- Genes discovered from `ENGINE_REPAIR_TREATMENTS` ∪ `PRODUCTION_TREATMENTS` ∪
  `PRODUCTION_OPT_INS`. `FIRAXIS_ONLY_TREATMENTS` are excluded by construction:
  they read host state and are inert on a native board, so screening them would
  measure noise and report it as noise.
- Two outcomes per gene: win rate, and **score share** (treated score ÷ all
  majors' scores). Both as mean paired differences with 95% intervals.
- ~2 CPU-minutes per game; the batch took about 3 hours at `--jobs 8`.

## What it measured

**The regime first, because it governs everything below.** 65% of the games
ended in a **religious** victory at a median of turn 148; 27% ran to the
turn-limit score, 6% culture, 1% science, 1% diplomatic. A third of all games
were over before turn 150.

**Resolution.** At 300 pairs this run resolves a win Δ of **±7.0 pp** and a
score-share Δ of **±1.50 pp** at 80% power. Fifty-seven genes throw ~2.6 flags
at |z| ≥ 2 by chance; the family-wise 5% bar is |z| ≥ 3.33. Everything below is
read against those numbers.

The treated seat — carrying a random half of the repairs — won **12.7%** of its
games against a 25% chance baseline, at a score share of 22.2% against 25%.

### Genes past the family-wise bar (all on the score-share axis)

| gene | win Δ | share Δ | verdict |
|---|---|---|---|
| `governor-every-lane` | −1.3 pp (z −0.53) | **−4.02 pp (z −8.34)** | HURTS |
| `war-economy` | **−6.0 pp (z −2.42)** | **−2.57 pp (z −5.00)** | HURTS on both axes |
| `wide-map-capacity` | −0.7 pp (z −0.27) | **+2.89 pp (z +5.69)** | more score, **no more wins** |

`governor-every-lane` reproduces an existing recorded result from a new
instrument: `advanced_every_lane` measured −62/−95 Elo in `docs/EVAL.md`. That
agreement is the best evidence available that this screen measures what it
claims to.

### Screen flags below the family-wise bar (candidates, not findings)

`district-coverage` +6.0 pp (z +2.42) and `score-horizon` +6.0 pp (z +2.42).
Two flags is what 57 genes produce by chance, so these are where to point an
arm, not results.

### Everything else

Fifty-two genes are `~` — **unresolved at this size, which is not the same as
no effect.** In particular the thirty-one war and siege genes cluster at ~0,
and the regime census says why: they were being asked what they contribute to a
game that ended by conversion at turn 148. That is a statement about the
experiment, not about the repairs.

### The anchors: the bundle buys cities and does not convert them

All screened genes ON (the native repair bundle plus the opt-in) against all
OFF, 20 pairs on the same maps and seats:

| | all-on | all-off | paired Δ (95% CI) |
|---|---|---|---|
| wins | 2/20 | 6/20 | −20.0 pp (−47.0, +7.0) · z −1.45 |
| score share | 19.3% | 21.9% | −2.60 pp (−6.51, +1.31) · z −1.30 |
| **cities** | — | — | **+3.45 (+2.48, +4.42) · z +6.98** |

Twenty pairs cannot resolve a 20-point win difference and this one does not:
both outcome intervals span zero. What twenty pairs *does* resolve is that the
bundle reliably ends with **three and a half more cities** and turns none of it
into wins — the same shape as `wide-map-capacity`'s own row, and consistent in
direction with `advanced_synergy`'s recorded −108 Elo without independently
establishing it.

⚠ "All-off" here is production `advanced` **minus `strategic_wonders`**, which
is a screened gene like any other. It is not exactly the shipped agent.

### Interactions: nothing at this size, and the tool says so

A foldover splits the evidence in two. The pair **difference** keeps every main
effect and cancels every two-factor term; the pair **sum** does the reverse. So
the interactions were already in these 300 pairs, in the half the main-effect
table throws away, and reading them needed no game replayed.

Of **1,596** gene pairs tested: **72 at |z| ≥ 2 against 73 expected by chance**,
and **0 past the family-wise bar** (|z| ≥ 4.16) on the win axis; 70 against 73,
1 past the bar, on score share. **The interaction layer is indistinguishable
from noise at this size**, and `--interactions` prints that verdict in those
words above the table — because a top-twenty list printed without it would read
as twenty findings every time it ran.

At this power, then, no pairwise coupling among the repairs is visible. That
does not refute the serial-coupling argument in `enable_engine_repairs`; it
bounds it.

## What was decided

**Shipped: the instrument.** `gene_screen`, `docs/GENE_SCREEN.md`, and this
round. Per-game rows are written to JSONL, so every re-analysis above —
interactions, anchors, the regime census — was computed without replaying a
game, and `--analyze` merges runs.

**No controller change on this evidence.** Two genes are implicated and both go
to a confirmation run at the deployment seat count and map (6 majors, 74×46,
9 city-states) with only five genes screened, so the residual background
variance is a fraction of this run's. Recorded separately when it lands.

**Three things this run changed about how the next one is set up**, all now
flags on the tool:

1. **The regime decides which genes can act.** `--victories domination,score`
   gives the war half a game that does not end at turn 148.
2. **Score share, not win rate, is the axis that resolves.** ±1.50 pp against
   ±7.0 pp from identical games. Every result past the family-wise bar here was
   on the share axis, and two of the three were invisible on the win axis — so
   the `read` column now names both.
3. **Fixed seating is a confound for the field.** Stock order seats Rome,
   Egypt, Greece and China in seats 0–3 every game, and seats 0 and 2 won twice
   as often as seat 3 whoever sat there. The foldover cancels this for every
   per-gene contrast — both arms share the seat — but the *field* is the same
   three civs in every game unless `--randomize-civs` is on.

**And one thing it changed about what to work on.** Two thirds of native games
being decided by conversion at turn 148, against a controller whose defensive
religion `docs/EVAL.md` has flagged as thin since its first baseline, makes the
religion lane the largest failure mode on the board. Rows now carry
`founded_religion`, `foreign_faith_cities`, banked `faith` and `inquisition`,
and the table prints a religion census from them, so the next screen diagnoses
the loss instead of only counting it. The first repair off that finding —
Apostle promotions chosen for the job the empire has rather than by the biggest
number on the card — is PR #2114 and is screenable by name the day it lands.
