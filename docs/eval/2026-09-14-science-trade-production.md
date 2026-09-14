# Research-informed trade valuation

## Strategy audit

Sources accessed 2026-09-14. These are experienced players' strategy guides,
not controlled comparisons proving a universally best build order. Advice
must match the map, civilization, ruleset and victory objective.

| Transferable strategy | Source | Current implementation and decision |
| --- | --- | --- |
| Expand into useful land, improve productive tiles, and fund development with trade | [Comrade Kaine's early-development guide](https://comradekaine.com/civilization6/14-essential-early-game-development-and-expansion-tips-for-civ-6/) | Already represented by expansion scheduling, settlement scoring, builder tasks, and trade-network genes. A rigid universal opening would discard their situational gates. |
| Coordinate district adjacency, placement and discounts | [Victoria's district-discount analysis](https://forums.civfanatics.com/resources/civ-vi-district-discounts.27783/) | `district_planning` coordinates sites and industrial support; cost-lock/research scheduling is a distinct, more invasive mechanism. A flat preference for cheap districts would not implement the source's strategy. |
| Use production modifiers, regional industry and trade to finish science | [Comrade Kaine's production guide](https://comradekaine.com/civilization6/per-turn-production/) | Industrial investment and launch scheduling exist. Route scoring still values production primarily through the empire's strategy weights. This is the gap implemented here. |
| Send production-bearing routes from cities building space projects | [Zigzagzigal's Australia guide, Ecommerce discussion](https://steamcommunity.com/sharedfiles/filedetails/?id=2542999669) | The generalizable part is valuing production at its origin; Australia's particular multiplier is not hardcoded. The new gene evaluates whether the chosen route actually advances a queued project. |

## Changes

`trade-production-to-launch` is an independently screenable opt-in, off by
default. A route gets a bounded premium for whole turns it saves on a legal
Spaceport project at its origin. This applies to domestic and international
routes, including the host's observed route options. The project must be at
the front of the queue, science victory must be enabled, and the city must
be loyal, solvent and not recently attacked. Ordinary research projects and
empty or pillaged Spaceports do not qualify.

The timing estimate uses remaining production, current city production and
the engine's project production multiplier. Existing routes are already in
the city's production, reducing the marginal value of another route. An
investment must finish within thirty Standard turns and before the game
clock. The hypothesis is four route-score points per Standard turn saved,
capped at 32. Equal completion turns earn no premium. These weights are not
claimed to be optimal.

When available, the host's `BuildQueue:GetTurnsLeft` quote anchors the
timing instead of the native estimate. The route's proportional increase in
origin production estimates the new duration; rounding that quoted duration
is conservative, not an exact reconstruction of hidden production progress.

This reaches the real destination chooser and its existing origin search.
It does not cancel active routes or relocate an idle Trader that already has
a legal route from its current city. Route legality, travel cost, quests,
alliance development and tourism preferences remain part of that chooser.
The timing is a marginal estimate, not a simulation of future governors,
growth or research. Native route previews
retain the existing fallback's limited accounting of policy/city modifiers.

The audit also found a deployed valuation error: observed potential-route
yields already include alliance modifiers, but the scorer added alliance
yield again. The scorer now adds that yield only to the native fallback.
It retains the first-connection alliance-development premium. This correction
is independent of the experimental gene and has no native-screen effect.

Engine authorities: `Game::item_remaining_cost_for_city`, `item_prod_mult`,
`city_yields`, `can_produce`, and the Spaceport district requirement in
`data/projects.json`. The host authority is the shipped
`Base/Assets/UI/Choosers/TradeRouteChooser.lua:864`:
`partialValue = tradeManager:CalculateOriginYieldFromPotentialRoute(m_originCityOwner, m_originCityID, cityOwner, cityID, yieldIndex);`
The bridge's `route_options` sums the chooser's route, path and modifier
values under the international multiplier; `observed_route_options` is
that complete yield vector, not an unmodified district-yield subtotal.

## Evaluation plan

Focused tests exercise actual destination ranking, route application,
remaining production, turn rounding, speed scaling, existing routes,
legality, economic distress and equivalent host/native alliance yields.
A six-game, 36-seat single-gene smoke probe uses the standard screen shape:
Emperor majors, six players, 74x46 Continents, nine city-states, Online speed,
all victory conditions, seeds 914356500 through 914356505. This is a smoke
probe, not enough evidence to promote a default or claim a win-rate gain.

## Results

The eight focused tests pass, including a complete route choice followed by
ordinary game turns that finishes the satellite launch earlier. They also
cover the host's quoted project duration and all four alliance-yield kinds.
The registry append-point tests pass without changing any existing gene bit.

The six-game probe completed all 36 seats. Eight seats drew the gene on and
28 drew it off: 1/8 versus 5/28 wins, a difference of **-5.36 percentage
points**, with a clustered standard error of **15.25 points**. Score share
differed by +3.83 points. This small randomized sample cannot establish a
win-rate improvement; its reported resolution is 42.71 win points. Early
science differences also reflect the sparse seat mix, not proof that a
late-game trade rule improved early research. The gene remains default off.

The artifact is
`docs/gene_screens/fires/2026-09-14-science-trade-production.json`; raw rows
are retained at
`/Users/martbot/civvis-runs/2026-09-14-science-trade-production/rows.jsonl`.
The binary was built from `a72b8bc8f6dd7a14d9b0eff12e187b039af70f6e`, with
SHA-256 `1931f46ebcd5cf8bc8a891ca2e7a0425256d8dbf20e0ba9e44237291c3823b93`.
Its source is explicitly stamped because a later test-only commit made the
automatic timestamp check decline to infer a revision. Subsequent changes
fix the test's native production-multiplier assumption, preserve registry
append markers without moving gene bits, and add the host-duration branch;
the native route-scoring behavior measured by this probe is unchanged.

The firing-evidence gate passes with the committed probe. That gate's
nonzero on/off statistics are a repository requirement, not an independent
causal proof; the controlled route-choice and normal-turn tests establish
that the implemented behavior can actually change a launch.

After merging `origin/main`, `cargo test --profile ci --locked` passed
3,694 tests (53 ignored). The 237 relevant Python tests passed: 185 gene,
21 manifest, 17 firing-evidence and 14 append-point tests. Formatting,
diff whitespace, gene generation, evaluation manifest, firing-evidence and
deployment-cost checks passed. No game rules were changed; the six completed
screen games provide the controller smoke run.
