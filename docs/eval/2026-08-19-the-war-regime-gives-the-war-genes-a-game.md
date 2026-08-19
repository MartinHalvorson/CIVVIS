# Turning off four victory lanes does not produce a war

_2026-08-19 · `agent/martbot-mbp-m5-max-128/claude-opus5-a7710669`_

## What was asked

Round one's regime census
(`docs/eval/2026-08-19-gene-screen-random-genome-factorial-screen.md`) found 65%
of native 4-player games ending by **religious conversion at a median of turn
148**, and drew the obvious conclusion: the thirty-one war and siege genes were
being asked what they contribute to a game that was over before a siege could
matter. It closed by naming the fix — restrict the victory lanes — and
`gene_screen --victories` was built for it.

**Does removing the other four lanes give the war half a game?**

## How it was measured

- **150 foldover pairs (300 games)**, seeds 28000000–28000149, disjoint from
  round one (26081900+) and round two (27000000+).
- 4 majors, 60×38 Pangaea, 6 city-states, Online speed to its own 250-turn
  clock, against a field of production `advanced`.
- **`--victories domination,score`** — conquest, or the score at the clock, and
  nothing else. All 57 genes screened.
- **`--randomize-civs`**, on round one's own recommendation: stock seating puts
  the same four civilizations in the same four seats every game.

```sh
gene_screen --pairs 150 --players 4 --width 60 --height 38 --city-states 6 \
  --victories domination,score --randomize-civs \
  --start-seed 28000000 --jobs 8 --out screen-war-regime.jsonl
```

⚠ This run differs from round one's 4p baseline in **two** ways — the lanes and
the seating — so it is not a clean single-factor contrast with it. It is a
well-specified regime in its own right, and the within-run gene contrasts are
unaffected: both arms of every pair share the map, the seat and the civilization.

## What it measured

### The regime answer is no

| | |
|---|---|
| games ending by **score at the clock** | **299 of 300 (100%)**, median t250 |
| games ending by **domination** | **1**, at t219 |
| mean cities at the end | 8.0 (against 6.6 in round one's all-lanes 4p) |
| mean military power at the end | 1,342 |
| majors eliminated | 4 of 300 |

**Armies get built and cities do not change hands.** Taking away religion,
culture, science and diplomacy does not make this AI conquer; it makes it play a
longer, larger score game. One domination in three hundred games, with a mean
military power of thirteen hundred on the board, is the finding.

So the war genes still cannot be priced on *did the war work* — but the regime
is well-conditioned in a way the all-lanes one is not: every game reaches turn
250, so nothing is truncated by an early conversion, and score share becomes a
clean axis. Resolution here: win Δ **±11.8 pp**, share Δ **±2.87 pp** at 80%
power; family-wise bar |z| ≥ 3.33 for 57 genes, with ~2.6 flags expected at
|z| ≥ 2 by chance.

### `war-economy` hurts for the third time, in the third regime

| run | regime | win Δ | share Δ |
|---|---|---|---|
| one | 4p all lanes, 300 pairs | −6.0 pp (z −2.42) | −2.57 pp (z −5.00) |
| two | 6p deployment shape, 80 pairs | −12.5 pp (z −3.03) | −2.62 pp (z −4.15) |
| **three** | **4p war regime, 150 pairs** | **−12.0 pp (z −2.92)** | **−4.76 pp (z −5.01)** |

Three disjoint seed ranges, three profiles, both axes every time, and the
adjusted coefficient over the whole sign matrix (−10.4 ± 5.2) agrees with the
marginal. This is as much as a screen can say.

### `wide-map-capacity` converts here, and that explains its earlier shape

**+10.7 pp win (z +2.58) and +5.22 pp share (z +5.59)** — where in both
all-lanes runs it bought score and **exactly zero** extra wins.

That is a coherent story rather than a contradiction: it is a **score-lane
gene**. It buys cities, cities buy score, and score only decides the game in the
regime where the clock does the deciding. Round two's anchors said the bundle as
a whole ends +3.45 cities and converts none of it; this says where the
conversion goes when the win condition is score.

### Screen flags, which are flags

`loyalty-rate-alarm` +10.7 pp win / +2.70 pp share, `escort-unstick` +9.3 pp,
`siege-muster` −10.7 pp — all at |z| between 2.2 and 2.6, against ~2.6 expected
by chance. Candidates for an arm, not results.

### And the faith bank in its purest form

With **religious victory switched off entirely**, the treated seat still founded
a religion in **68%** of games, launched an Inquisition in **2%**, and ended
with **3,799 Faith in the bank** — nearly double the 2,014 measured at the
deployment shape with religion live.

Round two traced the defensive half of this to `AdvancedAi::religious_defense`.
This run isolates the other half: the Faith arrives whether or not there is
anything to do with it — `Game::unused_great_person_faith` pays out the Prophet
points of an empire that has no Prophet race to run — and a non-founder has
almost no sink for it. `AdvancedAi::advanced_great_people` already carries a
comment about a live run ending on 1,999 Faith. At 3,799 with the lane disabled,
this is not an edge case.

## What was decided

**The war genes do not belong in a lane-restricted full game, and this run is
the evidence.** The repository already has the instrument built for the
question: `docs/TACTICS.md`'s arena, where combat is the only thing on the
board, and where `docs/TACTICS_BASELINE.md` has already measured the shape of
the problem — the advanced controller is overwhelmingly better than `basic` with
a city to take (39/40) and overwhelmingly **worse** in pure attrition (1/40).
A full game with four lanes removed produces one conquest in three hundred
games; the arena produces a fight every time. That is where the thirty-one
genes should be screened, and `gene_screen` cannot do it — this is a note for
whoever picks up the arena lane, not a claim that the flag was a mistake.
`--victories` remains the right way to *condition* a run; it is simply not a way
to *cause* a war.

**`war-economy` has as much screen evidence as this instrument can produce.**
Round two routed it to `ai_eval live live_without_war_economy --matrix`; nothing
here changes that, and a third agreeing regime raises the prior rather than
substituting for the gate. **`advanced_synergy`'s membership must still not be
edited to act on it** — a confirmed change is a new arm beside the old one.

**`wide-map-capacity` is understood, not promoted.** Its win contribution is
conditional on the score lane deciding the game, which is a property of the
regime and not of the deployment. Worth stating in its own doc comment; not
worth a promotion argument on a screen.

**Faith with no sink is now the largest measured hole**, and it is measured
twice from opposite directions: 752 Faith unspent in the games *lost to*
religion at the deployment shape, and 3,799 banked with the religious lane
*switched off*. The next repair in this lane is a spending one, not a promotion
one — PR #2114 already established that a promotion cannot be a religious
defence, because the Apostle is the only promotable religious unit and a
defending empire buys Missionaries and Inquisitors.
