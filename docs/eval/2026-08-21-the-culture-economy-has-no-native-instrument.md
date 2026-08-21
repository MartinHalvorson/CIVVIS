# The culture economy has no native instrument

_2026-08-21 · `agent/mbp-m5-pro-64/claude-opus-science` · PR #2245_

## What was asked

Operator, verbatim intent: study the heuristic genes around scaling science and
culture. Scale culture at least as far as the four-slot government, then scale
science. The cheap science is filling out Campus buildings, building more
Campuses, and finding science city-states to send Envoys to the 1- and 3-Envoy
tiers. **Calculate roughly the cost and the benefit and target the high-benefit,
low-cost opportunities.**

## How it was measured

A census (`examples/sci_cult_census.rs`, kept out of the tree) plays the
deployment shape — 6 majors, 74×46 Pangaea, Online, 250 turns, nine
city-states — with `enable_engine_repairs()` on every major, and reports each
seat at t40/t80/t120/t180 and at the end. Four windows, 108 seats:
seeds 90001000.., 90002000.., 90003000.., 90004000...

⚠ `AdvancedAi::fleet()` is `AdvancedAi::new()`, **not** the deployed genome. The
first pass used it and every number below moved when it was corrected — the
Political Philosophy date alone went from t52 to t39.

## What the board actually looks like

| | research tree | culture tree |
|---|---|---|
| district | Campus **79–82%** of cities | Theater Square **33–37%** |
| first building | Library **76–81%** | Amphitheater **32–35%** |
| second | University **71–74%** | Museum **29%** |
| third | Research Lab 40–41% | Broadcast Center ~0% |
| yield | science **148–172** a turn | culture **121–129** |
| tree | 47.7–50.0 techs | 36.8–38.3 civics |

Two of the operator's three science levers are already close to saturated, and
the third is saturated outright:

- **Filling out Campus buildings.** The Library follows the Campus within three
  percentage points. There is no missing-building gap left to close.
- **More Campuses.** 79–82% of cities hold one; the specialty capacity rule
  (`1 + (pop-1)/3`) never blocks a first district, so the remaining fifth is a
  city that chose a Harbor or a Commercial Hub, not a city that was refused.
- **Envoys at the 1- and 3-Envoy tiers of science city-states.** Of the
  scientific city-states the seat has met, it holds **≥1 Envoy at 100%, ≥3 at
  96%, and ≥6 at 66%.** This lever is finished.

## The cost and the benefit, in production

Production per +1 yield a turn, at the census's measured building counts (4.5
Libraries, 4.2 Universities, 2.4 Research Labs, 2.0 Amphitheaters, 1.6 Museums):

| item | cost | yield | production per +1 |
|---|---|---|---|
| Campus / Theater Square district | 54 | +2 citizen + adjacency | **~18–27** |
| Library | 90 | +2 science | 45 |
| Monument | 60 | +1 culture | 60 |
| University | 250 | +4 science | 62.5 |
| Amphitheater | 150 | +2 culture | 75 |
| Art / Archaeological Museum | 290 | +2 culture | 145 |
| Research Lab | 440 | +3 science | **147** |
| Broadcast Center | 440 | +2 culture | 220 |

And the Envoy tiers, which are the same arithmetic in a different currency
(`envoy_type_yields_for_count`: tier 1 pays +1 in the Palace city and +1 per
city holding the tier-1 building; tier 3 pays +2 per tier-2 building; tier 6
pays +3 per tier-3 building):

| Envoy | empire-wide gain | Libraries it is worth | production-equivalent |
|---|---|---|---|
| 1st at a scientific city-state | **+5.5 science** | 2.75 | **~248** |
| each of the two to reach 3 | +4.2 science | 2.1 | ~189 |
| each of the three to reach 6 | +2.4 science | 1.2 | ~108 |
| 1st at a cultural city-state | +3.0 culture | — | — |
| each of the two to reach 3 | +1.6 culture | — | — |

The operator's ranking is exactly right, and it explains a choice the agent
already makes correctly: **a 6-tier Envoy at 108 production-equivalent beats a
Research Lab at 147**, which is why Envoy coverage (66% at the 6-tier) runs
ahead of Research Lab coverage (40%) and should.

It also shows why the culture city-states are funded half as well: an Envoy
there is worth +3.0 against +5.5, because the empire holds 2.0 Amphitheaters
against 4.5 Libraries. **The culture lane is circular — the seat does not build
the buildings, so the Envoys that scale off them are cheap, so it does not fund
the Envoys either.**

## Where the asymmetry comes from

`research_economy` is set in `promoted_policy_envoy`, so every native seat pays
`RESEARCH_CAMPUS_COVERAGE` for a city with no Campus and `RESEARCH_BUILDING_DEBT`
for a Campus with no Library. The three heuristics that say the same sentence on
the other tree — `culture-coverage`, `culture-building-debt` and the
`district-building-chain` that fills every specialty district — were in
`FIRAXIS_ONLY_TREATMENTS`, on the reasoning that "the native lanes keep their
bred district coverage". That is an assumption about the bred `Weights`, and the
table above is what it is worth.

The three tags are now native repairs, screenable and (being unmeasured) off at
deployment. The screen is 6p all-seats, foldover, `--baseline best --field
repairs`, seeds 71000000...

## Two measurements that did not become code

**More Scouts do not buy more contacts.** The Envoy economy before Political
Philosophy really is first contact — a Chiefdom makes 1 Influence a turn against
a threshold of 100, `Game::record_meeting` gives the first major to discover a
city-state one Envoy standing at it, and Political Philosophy's own Eureka is
`met_city_states` ≥ 3. The census confirms the identity: **2.7 city-states met
and 2.1 Envoys held at t40.** But the empire does not hold the single Scout the
production scorer's `-2_000` veto implies — it holds **2.33 at t40 and 3.67 at
t120**, from the `grant_scout` goody hut and `reserve_idle_land_recon`. Lifting
both vetoes (the Scout line, and the army-composition ceiling that counts a
Scout as land military) moved t40 from 2.33 Scouts / 2.67 met to 2.50 / 2.50. At
900 per unmet city-state with a cap of six the mechanism fired and the answer
got worse: seed 90002001 at t120 held **5.50 Scouts and had met 4.67
city-states**, against the baseline's 3.83 and 6.00. Exploration is saturated at
two or three eyes; the constraint is walking distance.

**The Envoy allocation is close to its ceiling.** Against a greedy-optimal
spread of the same Envoys over the same city-states met, the seat's actual Envoy
yield is **85% of the ceiling** (66.8 of 78.3 points over 24.8 Envoys), and of
the 7.0 Envoys sitting above the last tier they paid for, only **0.29** face a
tier that pays nothing at all — the rest are suzerainty bids or the last Envoy
or two of the game. There is no large, simple leak here.

## A trap worth naming

A gene that the screen reports as `Δ +0.0 [+0.0, +0.0] z +0.00` was **not
varied**. `gene_screen` builds its treated seat from
`enable_engine_repairs_universe` and flips only the genes whose drawn bit
differs from `Gene::after_setup_on`, which the gene table asserts is `true` for
every `ENGINE_REPAIR_TREATMENTS` tag. A repair the universe never enables is off
in both arms, the arms play byte-identical games, and the result reads exactly
like a clean null. The first version of this change moved the three tags into
the tables without adding their enables and burned 30 games saying nothing.
`the_culture_economy_is_in_the_native_universe_and_out_of_the_deployment` pins
both halves of the contract. **A zero-width confidence interval is the
signature.**
