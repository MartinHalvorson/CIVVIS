# Strategy-evolver session synthesis

## The role's question, and the answer it turned out to have
"Take the best strategy and improve it" presumes there is one. There is not.

| candidate | strong on | weak on |
|---|---|---|
| shipped gen-14 champion | compact 4p 24×16 (+51 recorded / +48 replicated) | deployment 6p 74×46 (−30 recorded / −53 replicated) |
| live-league leader `g28-28` | the league's all-victories 6p table (199/1011 v stock 142/866) | the gate's deployment profile — 40% at only 10 maps, `INSUFFICIENT`; ⚠ `g44-41` read 51.9% at 40 maps on an 8p profile, so "league genomes do not carry" is **not** established |
| stock `advanced` | the deployment gate profile | compact, where the champion is +51 |

**There is a best strategy per reachability regime, and the repository ships
one artifact for both.**

## The mechanism (new)
- 96 tiles/player, 4 seats: the neighbour is adjacent. The champion takes
  **domination 21 to stock's 7** and its combat genes cash out.
- 567 tiles/player, 6 seats: nobody is reachable in 250 Online turns; the game
  is a science race. The champion sits at **16.1% midgame Science against
  stock's 31.4%** with **twice the city-deficit strategy boundaries**, because
  `settler_min_pop` 4.457 and `builder_per_city` 0.200 keep it short of a city
  target it *shares* with stock. It never leaves the expansion loop.

## The attribution (new), and the candidate it produced
Four arms, same forty deployment maps, seed 66,000,000:

| arm | champion genes | score | Elo |
|---|---:|---:|---:|
| `r0` deck-only calibration | 0 | 53.1% | +22 |
| `r2` yield half | 11 | 44.4% | −39 |
| `r3` the other 29 | 29 | **57.5%** | **+53** |
| champion (seed 61M) | 40 | 42.5% | −53 |

The halves oppose and the whole tracks the worse one. `r3` is `r2`'s mirror on
the deciding diagnostic (Science 33.9% v 15.9%, Expansion 18.7% v 35.6%, 21
science victories to 13) and holds a larger empire on every column.

And it is **not** that the eleven moved further: normalised displacement is
0.194 for the yield block against 0.185 for the rest. Equally perturbed, only
one consequential. The largest displacement in the genome, `war_ratio` at 0.65
of its range, belongs to a block `docs/GENOME.md` measured as reached only by
`BasicAi` — drift under no selection.

## The instrument correction (new)
`city_target` has **two** consumers; the recorded sweep moved the dead one. The
live one is `BasicAi::cities`, the Settler gate for every adaptive empire, and
`ai_eval` reports stock `advanced` at 100% adaptive. Expansion has two regimes:
deployment is **target**-limited (4.83 cities of a 5.00 target), the compact
eval profile is **execution**-limited (2.17 of 3.83).

## What was built and refuted
`advanced_plan_city_target` — hand the governor the empire's own land-aware
target. Killed by its own fires-check before an outcome seed: cities fall in
both profiles because the ramp opens at 3 and the stock gene is 4.

## The candidate, and where it stands

`r3` = the gen-14 champion with those eleven genes reverted to
`Weights::default()`, nothing else changed (verified gene-for-gene). An
explicit forty-gene form is at `r3-shippable-best.json` in this directory and
was verified **byte-identical** to the gated form by running both on the same
prefix and diffing the evaluator output.

| profile | shipped champion | `r3` |
|---|---|---|
| compact 4p 24×16, seed 61,000,000, same maps | 56.9%, +48 | 55.6%, +39 |
| deployment 6p 74×46 | 42.5%, −53 | 57.5%, +53 |

It cleared both pre-registered conditions and went to the unmodified
`ai_eval --matrix --pairs 120 --seed 67000000`. **Only that gate decides**, and
a champion replacement touches 38 evaluator arms plus the league seeding and
the embedded fallback, so every strength number measured against
`advanced_evolved` before such a change is measured against a different agent
after it.

## Method notes that earned themselves this session
- A two-minute fires-check killed a treatment that would have cost a 480-game
  gate.
- Reading the repo's record turned a claimed discovery into a replication plus
  a mechanism. `grep` the arm name in `docs/EVAL.md` before writing a headline.
- A binary relinked mid-screen threatens a multi-arm comparison; re-run the
  calibration arm on the final binary rather than reasoning about it.
- Ask what *else* differs: every artifact arm silently carries
  `PolicyDeck::Live`. The deck-only rung exists to difference that out, and
  `dedication_choice` was checked too rather than assumed.
- Search the record **before writing the headline**, twice over. It turned a
  claimed discovery into a replication (the 120-map matrix already had the
  +51/−30 split) and stopped me claiming an experiment the 2026-07-28 entry had
  already named and recorded as unlaunched.
- Do not extend a correction past the gene it was measured on: the settle-site
  block was checked for the same two-consumer pattern and does **not** have it.
