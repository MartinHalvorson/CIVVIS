# The district a city will build, and the plot it should buy

_2026-08-21 · `agent/mbp-m5-pro-64/claude-fable-lookahead` · PR #2253_

## What was asked

Operator request, 2026-08-21: add a gene (or modify an existing one behind a
flag) that **looks ahead at the likely district a city will build** and uses
that when choosing where to settle; and add logic for **if and when to
purchase tiles**, where a purchase has a cost and so the benefit must be
fairly positive — more value than the Gold gives up.

## What the code did before

**Settlement.** `settlement_static_value` prices a site's district potential
through `Game::settlement_adjacency_summary` (gene `adjacency_site_planning`,
on in production): for *every* adjacency-bearing family the ruleset knows,
the adjacency its best plot within two rings would pay, summed at the settle
scorer's flat yield weights and capped at 24. Two things are wrong with that
as a look-ahead. It is the potential of a city that builds a Campus, a Holy
Site, a Commercial Hub, a Theater Square, a Harbor and an Industrial Zone at
once — a Science seat is paid for the Holy Site it will never build. And the
families are scored independently, so the one river-mountain hex is paid for
as every district's best plot at the same time.

**Tile purchase.** `advanced_gold_spending` buys a plot only when no unit,
building or district clears its own bar (a plot is a "surplus purchase"),
scoring it as `24 × lane-weighted yields + resource class + natural wonder −
0.70 × price`, plus 0.35 of a speculative district site found by cloning the
game, against a constant floor of 120. Nothing asks whether a citizen would
*work* the plot — a size-three town with eleven tiles works three of them —
or whether the border takes it for free next turn anyway. With a victory
target the baseline `BasicAi::buy_gold_plot` runs as well on a similar flat
score. The price, meanwhile, quadruples over the game
(`Game::tile_purchase_cost`: `base × (1 + 4 × progress)`), so the same flat
floor means something different at turn 20 and turn 200.

## What was built

Two opt-in genes in `PRODUCTION_OPT_INS`, off everywhere until a screen
prices them (`docs/GENE_SCREEN.md`; the ledger leaves an unmeasured
screenable gene off). Both live in `src/ai/advanced/site_lookahead.rs`.

**`district-lookahead-settle`** replaces the summary term while on. The lane's
*wishlist* — the first two or three adjacency-bearing families
`production_value`'s `strategic_family` table and coverage terms would ask a
new city for (Science: Campus, Commercial Hub, Industrial Zone; Culture:
Theater Square first; Religion: Holy Site first; Diplomacy: Commercial Hub,
Harbor; Conquest and Recovery: Industrial Zone first; Expansion, the lane
every seat opens in: Campus, Commercial Hub, Harbor) — goes to a new
game-side calculator, `Game::settlement_district_lookahead_from_positions`,
which hands each family **its own plot, first family first** (same placement
and adjacency rules as the summary; uniques resolved to the civ's variant;
a family with no legal plot comes back `None`). Each family's adjacency is
priced at the lane's own `yield_value` (never below the settle scorer's flat
weights), weighted by its place on the wishlist, scaled ×1.5 and capped at
the summary's 24. A site where the lane's *first* district cannot be placed
at all loses 6 × its weight. The growth forecast, housing, safety and wonder
terms are untouched; a test pins that the gene swaps exactly the adjacency
term and nothing else.

**`priced-tile-purchase`** replaces the plot scorer in `advanced_gold_spending`
while on, and stands the baseline buyer down
(`BasicAi::plot_purchase_delegated`). Every legal `BuyPlot` is priced as an
investment in yield-points at the lane's weights:

- **the job** — what a citizen gains by moving to the plot now (its value less
  the weakest worked tile's, from `city_citizen_plan`), or half of what the
  next citizen gains by having it instead of the best idle owned tile;
- **the connection** — a luxury (6/turn) or strategic (4/turn) the empire owns
  nowhere yet; 1.5 / 1 for a second copy; bonus resources pay through yields;
- **the site** — the district this city would build next (first wished family
  it lacks and has not queued), if the plot beats every owned plot's
  adjacency for it, at 0.6 (the district arrives later);
- a natural wonder at 8/turn;

summed and multiplied by a **payback horizon**: 30 standard turns or the game
left, whichever is shorter — **cut to the border's own schedule when culture
would claim that very plot**. That needs two engine facts the AI could not
read: `Game::border_growth_front` (the tied-minimum influence plots the next
expansion draws from) and `Game::border_growth_turns` (the shipped
`10 + 6 × plots^1.3` curve less the culture banked, over per-turn border
culture), added as a child module `src/game/border_forecast.rs` so
`expand_borders` and its influence costs stay private. A plot the border takes
within two turns is never bought. The Gold is priced at what the lane thinks
a Gold is worth (`yield_value` of one Gold: 0.9 on Expansion, 2.2 on
Diplomacy), and the purchase is made only when the benefit clears that by
**×1.5 and by 40 points outright**. The plot stays a surplus purchase behind
units, buildings and Governor districts, behind the same `reserve + 200`.

## How it was measured

Not yet. This machine is running two fleet screens at load 80+
(`governor-expansion-lane` at seeds 57M, the culture-economy trio at 71M),
and an unmeasured opt-in ships off, so the screens are queued rather than
run here. What each test pins:

| what | test |
|---|---|
| the look-ahead hands families distinct plots, first first; no coast, no Harbor (`None`, not zero) | `adjacency::tests::lookahead_assigns_distinct_plots_first_district_first` |
| a grove pays the Religion lane's Holy Site and nothing to the Science lane's Campus, where the summary paid both | `site_lookahead::tests::the_lookahead_prices_only_the_districts_the_lane_would_build` |
| the gene swaps exactly the adjacency term of the settle score | `…::the_lookahead_enters_the_settle_score_only_when_the_gene_is_on` |
| the front is where `expand_borders` actually expands; turns follow the shipped curve and the bank | `border_forecast::tests::*` |
| a 2/2 plot beside a plains capital that the border will not reach clears ×1.5 + 40 | `…::a_worked_plot_the_border_will_not_reach_soon_pays_for_itself` |
| the border's next plot, one turn out, is not bought | `…::a_plot_the_border_takes_next_turn_is_not_bought` |
| a plot no citizen would take does not clear its price | `…::a_plot_no_citizen_would_work_does_not_clear_its_price` |
| the gold pass buys the priced plot and leaves bare plains | `…::the_gold_pass_buys_the_priced_plot_and_leaves_the_worthless_one` |
| both genes are native opt-ins; the purchase gene takes over the base buyer | the two `*_is_a_native_opt_in*` tests |

## What to run next

Price each gene against the best genome on disjoint seeds, native regime
first (the seeds below are past every window in the queue logs at 64M and
the two live runs at 57M/71M):

```
gene_screen --players 6 --all-seats --randomize-civs --baseline best \
  --pairs 1000 --start-seed 66000000 --genes district-lookahead-settle
gene_screen --players 6 --all-seats --randomize-civs --baseline best \
  --pairs 1000 --start-seed 67000000 --genes priced-tile-purchase
```

then the war regime (`--victories domination,score`) before any drop
decision, per the doctrine. If either helps past the bar, export `--json`
under `docs/gene_screens/`, add it as a `--source`, `gene_ledger.py --write`,
and the deployment genome turns it on. The two are priced apart on purpose:
the look-ahead moves where cities stand; the purchase moves where Gold goes,
and a joint screen could hide one behind the other.
