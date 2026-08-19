# The treatment lottery

`treatment_lottery` prices many flags from the same games: every game draws an
independent random subset of treatments to withhold, plays the drawn agent
against a same-seed full-bundle control, and averages the outcome deltas per
treatment across the batch. Randomization makes the other factors cancel in
expectation, so one game contributes an observation to **every** factor — with
`n` games each factor gets an ~`n/2` vs `n/2` comparison, where one-at-a-time
withholding would give it `n/(2·55)`.

```bash
# The default screen: 55 engine-repair factors, density 0.5
treatment_lottery --games 400 --jobs 8

# At the deployment shape, on a disjoint seed range
treatment_lottery --games 400 --players 6 --width 74 --height 46 \
  --city-states 9 --turns 250 --start-seed 7300000 --draw-seed 71

# A focused lottery over one lane
treatment_lottery --factors siege-muster,siege-role,siege-tracks-wall,siege-commitment,siege-is-progress
```

Every game writes a JSONL ledger row (`--out`, default
`lottery-s<seed>-d<draw>-g<games>.jsonl`) holding the withheld vector, the draw
provenance (`draw_seed`, `density`), and both outcomes — the artifact nothing
else in the repository carries, so an analysis can be redone or extended
without replaying a single game.

## Where it sits in the doctrine

`docs/EVAL.md` prices by withholding **one behaviour at a time**, and that
remains the confirmation standard. The lottery is the screening tier in front
of it, built for the situation the ledger actually shows: 81 registered live
treatments, 55 measurable headless, and 24 of those never named in any eval
round. The two tiers answer different objections:

- *"A composite gate licenses the composite, never its parts"* — true of a
  **fixed** composite, which bounds only the net of its members. A composite
  **randomized per game** is different: the marginal contrast is an unbiased
  estimate of each member's average main effect, because every other member is
  on and off equally often on both sides of the comparison.
- What the composite objection still gets: the lottery's estimand is the
  factor's effect averaged over the drawn mixture — at density 0.5, over
  agents missing a random half of the bundle — **not** its effect at the
  deployment point. Interactions are real here (one constructor held a −41
  and a +30 Elo component at the same time), so a lottery signal licenses a
  single-flag confirmation arm (`live_without_<tag>`, or a withholding round
  at the deployment shape); it never licenses a ship decision by itself.

## The genetic-algorithm connection

This is gene activation without breeding. `docs/GENOME.md` records why the
selection half of a GA has not worked in this repository: the deployed rating
carried no signal (−0.025 nats/game against guessing) and about a thousand
rounds of whole-genome evolution returned null. The lottery keeps the useful
half — random activation of discrete genes — and replaces selection with
estimation, which is unbiased, uses every game it plays, and produces a
standard error per gene instead of a champion per run.

## Reading the table

- `contrast` = mean(score-share delta | withheld) − mean(| kept). Negative
  means withholding hurt: the treatment is an asset at this mixture. `win` is
  the same contrast on paired wins.
- `t` = contrast / se. |t| ≥ 2 is a screening signal, not a result. Confirm on
  a **disjoint** `--start-seed` and then on the single-flag arm; one seed
  range is never a result.
- A flat zero is bounded by the fires-check: a factor whose branch never
  executes at this profile prices at exactly zero, indistinguishable from a
  real null. The headline `moved` count (draws that changed the outcome at
  all) is a ceiling on how much any factor could have fired, not an
  attribution.
- Contrasts share games, so neighbouring rows are correlated — rank them,
  don't difference them.

## Choosing `--density`

Density 0.5 maximizes per-factor power and distance from deployment at the
same time. A small density (say 2–4 expected withhelds per game) prices close
to the deployment point but populates each factor's withheld half at only
~`games × density` observations — compute the standard error you can afford
before spending the games, per `docs/GENOME.md`'s method rules. A two-pass
shape that has both properties: screen wide at 0.5, then re-run the survivors
as a small `--factors` lottery at low density.

## The live-ladder half (design, not yet built)

The same idea runs on the real Civ 6 seat with no decider changes: the climb
already threads `--without` end to end
(`civ6_civvis_climb.py --without` → `civ6_play.py --civvis-without` →
`civ6_brain.py` → `civvis_orders --without`), and since #1902 every ladder row
records `withheld` and `mod_arms`. Drawing a small random subset per attempt
turns the ladder into a slow lottery — the only instrument that can ever
price the 26 Firaxis-only treatments headless play cannot reach. At ~24
completed games a day the density must be low (the seat still has to play
well) and the horizon is weeks, but every game already played contributes to
every factor, which serial A/B batches cannot do. As of 2026-08-19 the ladder
ledger has 308 rows with `withheld: null` (pre-#1902), 9 with `[]`, and none
non-empty — the lottery would be the first consumer of that column.
