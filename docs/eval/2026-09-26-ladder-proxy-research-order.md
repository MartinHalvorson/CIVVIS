# The Science seat walks its research by price and by name, and never asks what a node is worth

_2026-09-26 · `claude-opus55-0926-5d` on `martins-m4-air` · simulator rounds, no native games_

Follow-up to `docs/eval/2026-09-26-ladder-proxy-immortal.md`, same instrument
(`examples/ladder_proxy.rs`, private, not committed) and shape: 4 majors, Tiny
60×38 Pangaea, Online, 6 city-states, the focal seat the live controller with
`handicap_exempt` and the live forced genes, three rival seats
`AdvancedAi::new()` + `enable_live_bridge()` carrying the rung's full AI
bonuses. Paired by seed; "share" is the focal seat's score over the four
majors' total at the last turn played.

## What the boost audit found

The instrument now records every seat's completed and boosted technologies and
civics at turns 60 and 100, and the turn each node completed. Immortal, Science
lane, 16 games (61000000+, 150-turn clock), at turn 100:

| | focal | rival mean |
|---|---:|---:|
| technologies | 24.3 | 38.8 |
| … completed with their Eureka | **8.5** (35%) | 21.3 (55%) |
| civics | 17.9 | 28.3 |
| … completed with their Inspiration | **5.4** (30%) | 14.4 (51%) |

Rivals are handed three free Eurekas and three free Inspirations each era, so
part of that gap is the rung. The candidate this round tested is the order the
focal seat takes the tree in. Its research journal (seed 61000004, turn by
turn):

```
t1  Researching animal husbandry — the cheapest step toward rocketry, which science needs
t6  Researching mining — the cheapest step toward rocketry, which science needs
t10 Researching archery — the first range-two defender is needed against nearby barbarians
t17 Researching pottery — the cheapest step toward irrigation, which science needs
t20 Researching irrigation — the cheapest step toward irrigation, which science needs
t24 Researching sailing — the cheapest step toward rocketry, which science needs
t27 Researching writing — the cheapest step toward rocketry, which science needs
```

A Science seat takes Writing — its Campus and its Library — at turn 27, behind
Sailing. Education lands at a median turn 82 against the rivals' 52 (six
full-length Immortal games, measured independently by `claude-opus55-0926`).

## Why: the beeline pre-empts every opinion the seat has about the tree

`advanced_research` asks a chain of forced goals first and only falls back to
the `tech_value` argmax when none is set. An assigned Science seat always has
one: `science_victory_tech_goal` names Rocketry from turn 1, and Rocketry's
ancestors are 7 of the 11 Ancient technologies, 5 of 8 Classical, 6 of 8
Medieval. `goal_pick` then takes, among the era window's ancestors of the goal,
the **cheapest printed price, ties by name**: Animal Husbandry, Mining and
Pottery at 25; then Archery, Sailing, Writing at 50 in alphabetical order;
Military Tactics (300) before Education (390). Civics do the same toward
Political Philosophy, the government ladder and Space Race.

The order among a goal's prerequisites cannot change when the goal lands — every
one of them is researched first — so the cheapest-first rule buys nothing. What
the order does decide is which unlock arrives first, and which nodes are still
open when their boost trigger fires. It is also why every boost-ordering gene
reads **byte-identical** on the Science seat (`boost-first-research-2`,
`boost-wait-research-2`, `boost-planner`, `boost-planner-builds`, three seeds
each): they live in the argmax the beeline never lets run.

## The gene: `beeline-orders-by-value`

A forced research or civic goal walks its remaining prerequisites by
`tech_value` / `civic_value` — the same score the unforced argmax ranks by,
the flat credit for a boost in hand included — instead of by printed price.
The goal, the forced-goal chain and the era window are untouched; only the
choice among the goal's own prerequisites changes. Off, every path is
byte-identical. Opt-in.

Under the gene, over the same 16 seeds (150-turn clock), mean completion turn:
Writing 28.5 → 27.2, Currency 41.8 → 39.1 (with its Eureka in 10 → 13 games),
Education 86.8 → 78.4; Craftsmanship 17.3 → 28.4 (Inspired in 0 → 2 games),
Early Empire 27.9 → 24.2 (Inspired in 10 → 5 — taken sooner, it is open for
less of its population trigger). Coastal seats still take Sailing early
(25.4 → 20.9): `tech_value` pays the water goal 190 on a coastal start, and
Sailing's own Eureka (a coastal city) was in hand in 9 of the 16 games.

## Measured

Paired by seed, gene on the focal seat only. Settler counts are summed over
the block: "founded" is Settlers that reached a site and settled it, "lost" is
Settlers that left the map without a new city within three tiles of their last
position (captured or killed).

| block | games | Δ share | z | Science t100 | cities t100 | Settlers that founded | Settlers lost | alive at end |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Immortal, Science, 150-turn clock, block A (61000000+) | 16 | +0.66 pp | +1.00 | 59.2 → 60.2 | 5.81 → 6.00 | 117 → 110 | 0 → 4 | 16 → 16 |
| Immortal, Science, 150-turn clock, block B (61000016+) | 16 | +0.58 pp | +0.83 | 56.0 → 56.8 | 5.88 → 5.62 | 105 → 99 | 3 → 5 | 14 → 16 |
| Immortal, Science, **250-turn clock** (61000000+) | 16 | -0.95 pp | -1.40 | 57.1 → 59.1 | 6.47 → 5.80 | 116 → 101 | 0 → 6 | 16 → 15 |
| Emperor, Science, 150-turn clock (51000000+) | 16 | +1.37 pp | +2.09 | 53.6 → 55.1 | 6.31 → 6.44 | 104 → 120 | 3 → 2 | 15 → 16 |
| King, Domination, 150-turn clock (37140000+) | 16 | +1.04 pp | +1.20 | 58.1 → 62.9 | 6.38 → 6.12 | 118 → 114 | 4 → 2 | 16 → 16 |

The two 150-turn Immortal blocks pool to **+0.62 pp (z +1.31)** over 32 games,
and King and Emperor read positive. The configuration the live seat actually
plays — Immortal, Science, the full 250-turn clock — reads the other way, and
its leading indicators say why: Science at turn 100 rises, but the seat builds
fewer Settlers and loses more of them, and holds fewer cities at turn 100.
⚠ **A 150-turn clock is not the first 150 turns of a 250-turn game**: the lane's
development half is keyed to the game's length, so the seat specializes at
turn 75 instead of 125, and the shortened games hid the cost.
An independent 250-turn arm by `claude-opus55-0926` (8 games, the value order
on the Rocketry beeline only, `builders-work-through-raiders` on) read
−0.35 pp (z −0.37), Education 82.5 → 73, technologies at turn 150 +0.5.

The Settler losses are an Immortal effect — over the three Immortal blocks
the seat lost 15 Settlers with the gene against 3 without it, at King and
Emperor 4 against 7 — and Immortal is where barbarians march at 1.5× force.
Most of it is the civic half: value
order takes Foreign Trade and Early Empire ahead of Craftsmanship, which lands
about ten turns later (17.3 → 28.4 in block A). A techs-only prototype (the
same order for technologies, civics left on the cheapest step; bench build,
not in this PR), on the same sixteen 250-turn Immortal seeds, reads
**−0.23 pp (z −0.29)**: Science at turn 100 57.1 → 63.7 (z +1.88), cities at
turn 100 6.47 → 6.00, Settlers lost 0 → 1 against the full gene's 0 → 6. The
tech half buys Science with a little expansion (buildings take Production the
Settlers had: through turn 100 Buildings +6%, Settlers −7%, military −14% in
the full-length block) and nets to nothing at this rung.

## What was tried and dropped

- **`beeline-orders-by-value-2`** — the same order priced at the lane's own
  yield weights (Science) rather than the plan's current posture (Expansion in
  the development half). Block B, 12 seeds: share +0.57 pp but Science at turn
  100 −5.2 and cashed boosts −1.1 against the baseline, where version one read
  +0.86 pp and +1.9 Science. Not shipped.
- **`boost-wait-research-2` on top.** With the beeline now reading
  `tech_value`, the wait penalty can reach the Science seat — and still barely
  does: 12 of 13 paired games byte-identical to the gene alone.
- **`eureka-chasing-builder`** (existing, off), 15 paired games: +0.35 pp, but
  Science at turn 100 −8.3 (z −1.83). Not taken further.

## What was decided

- **Shipped, off**: `beeline-orders-by-value`, an opt-in gene with its fires
  probe (`docs/gene_screens/fires/beeline-orders-by-value.json`). It is **not**
  forced on the live seat: at the rung the ladder is trying to claim, full
  length, it reads negative and costs Settlers.
- **Recorded**: an assigned Science seat's research is decided by the
  forced-goal chain from turn 1, so every research-ordering gene that lives in
  the `tech_value` argmax is inert there. A research experiment for the live
  seat has to act on `goal_pick`, or it measures nothing.
- **Not pursued**: the techs-only order — neutral at the rung (−0.23 pp,
  z −0.29) for +6.5 Science. What the Immortal seat is short of is cities and
  the Production to defend them, not the order of its tree.

The fires probe (6 standard screen games, seeds 26092661–66) was played on the
bench build, which still registered the dropped `beeline-orders-by-value-2`
toggle; no seat drew it, so its rows in the probe are empty.
