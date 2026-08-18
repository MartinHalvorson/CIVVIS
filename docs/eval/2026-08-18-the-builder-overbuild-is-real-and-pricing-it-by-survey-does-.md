# The builder overbuild is real and pricing it by survey does not win games

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`advanced_builder_survey` has been implemented and off by default since it was
written. Its own doc comment states the defect it exists to fix: the Builder
production arm is `ceil(cities/2)` and a flat 260, it never looks at a tile, so
a fully-improved empire and a virgin one buy Builders identically — and the
floor withhold had already measured that overbuild costing terminal score
(225/175, p=0.0142).

An unscreened arm with a documented defect behind it is a decision nobody has
made. Make it.

## How it was measured

First the defect, to check it is still there. A probe linking the crate,
following every major Builder for its whole life over three 150-turn
six-player games at 74x46, 9 city-states, Online, seeds 21000000–02.

Then the arm: `ai_eval advanced_builder_survey advanced`, **200 pairs / 400
games** at 6p 74x46, 6 city-states, Online, 150 turns, seed 8500000. Two
hundred and not forty, because at the ~26% break rate these screens produce a
40-map run cannot promote anything under about +97 Elo-equivalent — see
`2026-08-18-a-forty-map-screen-cannot-see-a-forty-elo-change.md`.

## What it measured

**The overbuild is real and larger than the headline suggests.** Across three
games majors fielded 330 Builders, and:

| | |
|---|---:|
| Builder unit-turns | 4616 |
| stationary | 1429 (31.0%) |
| of those, spent a charge that turn (productive) | 372 (26.0%) |
| **of those, did nothing at all** | **1057** |
| stationary turns with **no** charges left | **0** |
| **charges granted / spent** | **1019 / 614 (60.3%)** |
| Builders alive at the end still holding charges | 49 (120 charges) |

**Two in five Builder charges are never spent**, and every idle Builder-turn is
an idle Builder that still had work in hand. Nothing is stranded empty; the
empire simply buys more capacity than it consumes.

**The arm does not convert any of that into wins.** 200 maps: **49.8%,
Elo-equivalent −2, betting CI 43.4%..55.4%**, 63 of 200 maps broke,
**INCONCLUSIVE**. At this map count and break rate the gate resolves about +47,
so an interval of −46..+37 centred on zero is a real null and not an unasked
question.

## What was decided

**Withheld, and the question is closed rather than left open.** Repricing the
Builder by a survey of the work it would do fixes the overbuild it was aimed at
and does not make the controller stronger. The arm stays for the record; the
defect it targets should not be re-attacked from the pricing side without new
reasoning.

⚠ The interesting part is what this rules *out*. The overbuild is measured, it
costs terminal score, and removing it is worth nothing in wins. That is the
third time this log has separated the two: score is not the endpoint, and a
defect being real does not make fixing it valuable.

One thread stays open and is not this one: **1057 Builder-turns of a unit
standing in the field with charges in hand**. Pricing the *purchase* is not the
same question as why the Builder already bought is not working.
