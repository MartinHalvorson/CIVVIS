# The wonder lane is already open, in both regimes

_2026-08-24 · `b2e9c8bc4d02aedbd7c7b83f3d5c1eace1a5676b`_

## What was asked

The `Item::Wonder` arm of `AdvancedAi::production_value` returns the `-10_000`
refusal sentinel unless the plan is Culture, the assigned victory target is
Score, or the seat is an **untargeted Egypt or China**. That last clause is an
identity check, not a merit check, and the brief that opened this round read it
as an ACTUATION gap: *four of six stock civilizations structurally never build a
wonder out of a 53-wonder roster, and native games finish with 0–3 wonders
total.* The ask was a reachability repair — a wonder reachable on merit for any
civilization, still losing to a district or a Settler when it should — built as
an opt-in gene and priced on the standard screen.

Two of the three gates that could already open that lane cannot on a native
board. `live-wonder-race` is `Kind::HostOnly`, so it is inert in a headless
game. `strategic-wonders` prices `spec.effects` **in the lane's own currency**
and returns exactly zero for `Conquest`, `Expansion` and `Recovery` by
construction, which are the plans a seat with no assigned target spends most of
its game in; both ship off in `docs/gene_ledger.json`.

⚠ `docs/eval/2026-08-19-the-wonder-reachability-gap-is-closing-on-its-own.md`
had already warned that the census behind this brief was stale — a five-fold
move in sixteen merges — and said it *"should be re-taken before any of it is
funded"*. This round is that re-take, the repair beside it, and the same
question asked of the live corpus.

## How it was measured

**The gene.** `wonder-score-tally` (`AdvancedAi::wonder_score_tally`,
`Kind::OptIn`, off in `new()` and `legacy()`) adds a **fourth** gate to the arm,
keyed to the one fact about a wonder that is true for every seat: `Game::score_parts`
awards **15 points a wonder**, the densest line of the tally against 1 for a
building, 2 for a district and 5 for a city. The gate opens when

* the empire is developed — three cities, three buildings in this city, at most
  one concurrent wonder per six cities, the guards the live race earned;
* no other gate already paid for this wonder (`!lane_opens && !live_race_opens`),
  so the two prices never stack;
* the plan is not `Recovery`; and
* the wonder's **ordinary value** (its yields, housing, Amenities, Great Work
  slots and Great Person points, lifted verbatim out of the arm so the bar and
  the score are the same number) **plus** the tally price clears
  `WONDER_TALLY_MIN_DENSITY` = 3.0 per point of production cost.

The tally price is `WONDER_TALLY_SCORE_POINTS` × `WONDER_TALLY_POINT_VALUE` =
15 × 100. Neither constant is new: `LIVE_WONDER_RACE_BONUS` is already 1 500 for
the same fifteen points, and its comment states the derivation. The gene adds
**no flat lane bonus** — the wonder enters the ranking at exactly the value it
was measured at, so `raw / (7 + turns)` still lets a Settler or a district
out-bid it. `WONDER_TALLY_SCORE_POINTS` is pinned to the engine by
`the_tally_pays_what_the_wonder_gene_prices`, which reads the score off a real
game rather than trusting the literal.

⚠ The live race's `wonder_era + 2 >= world_era` staleness guard is deliberately
**not** carried over. It prices a Firaxis catalogue the rivals have already
eaten; on a native board the engine's own `built_wonders` menu is the only
contest, so an unbuilt ancient wonder is still standing late.

**The screen.** The standard shape and nothing else: six majors, 74×46
Continents, nine city-states, Online speed to its own 250-turn clock, all six
victory lanes, shuffled civilizations, best-genome baseline, every seat drawing
its own genome independently. `--genes wonder-score-tally`, so **every other
gene is held at the deployment default and the off arm is literally the shipped
agent** — the contrast is causal and the off arm doubles as a census of what
production plays today. Seeds from 26100000, build `b2e9c8bc4d02` (clean,
stamped). Errors are clustered by game throughout, because the six seats of one
game share a winner.

`gene_screen` rows gained a `wonders` census field and the analyzer a wonder
census line, so the actuation question is answered from the batch rather than
asserted.

## What it measured

### 1. The premise is false: the wonder lane is already open to everybody

The off arm **is** the deployment genome, so this is a census of what production
plays today, not of a treatment.

| deployment genome, 237 seats | |
|---|---|
| seats that finished at least one wonder | **91.6%** |
| wonders a seat | **6.54** |
| tally points a seat from wonders alone | **98** |
| civilizations that never finished one | Zulu (n=2), Sumeria (n=1) |

The brief held that four of six stock civilizations structurally never build a
wonder and that a whole game finishes with 0–3. Neither survives. Fifty
civilizations appear in the batch and forty-eight of them average between 3.5
and 12.2 wonders a seat; the two that read zero have three seats between them.

⭐ **The `wonder_civ` clause is not what is doing the work — it is doing less
than nothing.** Egypt and China, the two civilizations `lane_opens` names,
average **4.88 wonders a seat (n=8)** against **6.60 for the other forty-eight
(n=229)**. The disjunct that actually opens the lane is
`plan.strategy == GrandStrategy::Culture`, and `assess` awards that lane on
progress, so any empire can enter it — pinned as a unit test in
`wonder_score_tally_never_stacks_and_never_moves_a_gate_it_does_not_own`: the
same Roman seat that takes the `-10_000` refusal under an Expansion plan is
offered the same wonder under a Culture plan, with no gene and no Egypt.

### 2. The gene fires, and it does not resolve

77 complete games, **462 seats** (225 on / 237 off), seeds 26100000–26100076,
build `b2e9c8bc4d02`, errors clustered by game. ⚠ A **partial** batch: 462 of
2,400 intended seats, stopped deliberately rather than extended.

| axis | on | off | Δ | 95% CI | z |
|---|---:|---:|---:|---|---:|
| win | 16.89% | 16.46% | **+0.43 pp** | [−6.8, +7.6] | +0.12 |
| score share | — | — | **−0.23 pp** | [−1.34, +0.88] | −0.41 |
| wonders a seat | 6.22 | 6.54 | −0.32 | — | — |
| cities a seat | 6.94 | 7.02 | −0.08 | — | — |

The analyzer resolves a win Δ of ±10.3 pp and a share Δ of ±1.58 pp at 80%
power at this size, so the win axis is not a reading at all. The **share** axis
is the one this gene's whole thesis lives on — a wonder is 15 tally points and
58% of these games end on the tally — and it is inside its own resolution at
−0.23 pp. The gene is `~`: it fires, and it does not move what it was built to
move.

⚠ An earlier 12-game pilot read −5.7 pp wins and −3.53 pp share and looked like
a clear harm. It was noise, and it is recorded here because *"one seed is never
a result"* is this repository's rule and this is what violating it looks like.

Where the gene does act is visible in the census: within the 9+ city stratum the
treated arm holds **+1.57** more wonders a seat (22 vs 24 seats), and within
6–8 cities **−0.58** (139 vs 153). It moves wonders around; it does not move the
outcome.

### 3. The live seat builds wonders too — the brief's figure is inverted

Re-measured from `~/civvis-civ6-runs/control`, taking each run's final
`public_stats.wonder_count` (cross-checked against the summed per-city `wonders`
arrays, which agree exactly):

| recorded live runs | |
|---|---|
| runs reaching turn 200+ | **64** |
| finished with **zero** wonders | **6 / 64 = 9%** |
| finished with at least one | **58 / 64 = 91%** |
| wonders a run | mean **7.05**, median **7**, max 15 |
| tally points a run from wonders | **106** |
| our score ÷ best rival's | mean **79%**, median 81% (874 against 1,152) |

The brief quoted *"91% of runs end with ZERO wonders."* The recorded figure is
its exact complement: **91% end with at least one.** The 9% that end with none
are the collapsed runs — 4 to 6 cities and scores of 220–653 — where nothing was
built, not where wonders were refused.

⭐ **So the two regimes agree, and there is no wonder gap to close in either.**
The native agent finishes 6.54 wonders a seat; the live agent finishes 7.05 a
run. A headless-versus-live regime gap was the natural reframing once the native
census came in, and it is not what the live corpus says.

⚠ The live score deficit is real but much smaller than quoted: we score **79%**
of the leading rival, not 26%. And the live correlate is the same trap as the
native one — wonders track score at **r = +0.90** across those 64 runs, while
cities track it at +0.44, which is exactly the correlation this round's own
controlled arm declines to convert into a lever.

## What was decided

**`wonder-score-tally` ships, and it ships OFF.** A gene no screen has priced
defaults off, and this batch is deliberately not entered as one. The row stays
in the registry and the ranking so the next standard screen prices it for free.

⚠⚠ **The batch is not a ledger source, and the arithmetic for why is worth
recording.** With exactly one populated win column the deployment rule promotes
a gene reading above **+20** wins per 10,000 on-arm seats (operator directive
2026-08-22). This batch would produce **+22.2** — and it does so on **38 on-arm
wins against a chance expectation of 37.5**. *Half of one win* clears the
promotion bar, on an estimate whose 95% interval is [−6.8, +7.6] pp and whose z
is +0.12. The clause has no power floor; it was written for a ten-thousand-seat
screen and it is unsafe at 462 seats. This is not hypothetical:
`governor-victory-lanes` was promoted by that clause on **+46** and the first
standard-shape screen then read it at **−3.29 pp, z −4.35 over 10,002 seats**,
with a 600-map-pair direct arm confirming **−4.78 pp, z −6.11**
(`docs/gene_ranking_notes.md` calls it the genome's own worst row). Feeding this
batch to the rule would repeat that exactly, so the numbers are published here
and the default is left where the absence of a screen puts it.

**The brief's two motivating figures were both wrong, in opposite directions,
and correcting them dissolves the task.** *Four of six civilizations never build
a wonder* — no: 91.6% of deployment seats finish one, and the two civilizations
the identity clause names build **fewer** than everyone else. *91% of live runs
end with zero wonders* — no: 91% end with **at least one**, mean 7.05 a run.
Neither regime has a wonder gap, and the native and live counts (6.54 a seat,
7.05 a run) agree closely enough that the obvious reframing — that this is
another headless-versus-live regime gap — is itself refuted by the corpus.

**What the round actually establishes.** The `Item::Wonder` sentinel is real
code and its civilization clause really is an identity check, but it has never
been the binding constraint, because the Culture disjunct beside it is dynamic.
Widening the lane on merit is therefore a repair to something that was not
broken, and the controlled arm says so: the gene fires, moves wonders between
empire-size strata, and does not move score share (−0.23 pp, inside its own
±1.58 pp resolution) on an axis where 58% of games end on the tally.

**The correlation that makes this look like headroom is empire size, in both
regimes.** Native: wonders track score share at r = +0.87 while cities track it
at +0.74 and wonders track cities at +0.59. Live: wonders track score at
r = +0.90 while cities track it at +0.44. `docs/EVAL.md`'s own rule — a
score-share correlate is economy evidence, never a lever — arriving twice from
two different instruments, with a controlled arm in between declining to convert
it.

**Where the live score deficit actually is, is still open, and this round moves
it.** We score 79% of the leading rival across 64 full-length runs, not the 26%
the brief quoted, and wonders are not the missing component — we finish seven of
them, worth 106 tally points a run. Whatever the remaining 21% is, it is not the
wonder lane, and the next round should decompose the live tally by component
rather than assume one. That needs the live corpus, not another native screen;
the ladder is halted, but the 852 recorded runs under `~/civvis-civ6-runs/control`
already carry `public_stats` and per-city state at every turn, which is what this
round mined.
