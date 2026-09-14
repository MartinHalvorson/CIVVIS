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

This reaches the real destination chooser and its existing origin search.
It does not cancel active routes or relocate an idle Trader that already has
a legal route from its current city. Route legality, travel cost, quests,
alliance development and tourism preferences remain part of that chooser.
The timing is a marginal estimate, not a simulation of future governors,
growth, research or host-only production modifiers. Native route previews
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

Results are recorded after execution below.
