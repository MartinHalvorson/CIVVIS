# Existing trade-route growth coverage

The passive observer reproduced all eight previously consumed controls' action
histories and final saves byte for byte. Most early routes were already domestic;
this diagnostic does not identify an early route preference gap. Do not design
a route intervention or use fresh strength samples on this evidence alone.

The observer changes no controller or route decision. It records active focal
routes and their destination Food/Production payload, plus engine-legal route
options for existing idle Traders at world-turn start. Origin population, raw
Food surplus, housing, amenities, income and bank are recorded. Raw Food surplus
is before housing/growth modifiers and is not a citizen-arrival forecast.

The option enumeration uses the full native world. Foreign destinations may
be unknown to the focal AI; this instrumentation does not actuate them. The
prospective domestic destinations are owned cities. A Trader that moves into
an origin later in the turn can start a route without appearing as ready in the
turn-start options, so these observations do not exhaust every route choice.

## Completed consumed coverage

Four Emperor seeds 61008600–603 and four Deity seeds 61008700–703 use the public
named Domination live bridge controller, unchanged weights, stock adaptive
rivals, four majors, six city states, 60×38 Pangaea, Online speed, Gran Colombia
focal and a 150-turn cap. Only the focal seat is handicap-exempt. All eight
assigned executions completed with exit zero before analysis. Source, protocol,
compiler, optimized binary and library/dependency hashes were frozen before play.
Setup readbacks confirm identical public weights and the rejected escort off.
Journals retain contiguous thought IDs with no resets or truncated turns.

| Difficulty | Ready Trader-turns, all played turns | Exploratory eligible Trader-turns through T75 | Foreign starts with eligible domestic alternative | Domestic / foreign route-turns through T75 |
| --- | ---: | ---: | ---: | ---: |
| Emperor | 27 | 1 | 0 | 134 / 18 |
| Deity | 23 | 0 | 0 | 108 / 13 |

A route-turn counts one active route at one observed world-turn start. The
exploratory eligibility filter requires T≤75, origin population below six,
housing headroom at least two, nonnegative amenities, income at least three,
raw Food surplus at most two, and a legal owned destination supplying at least
one Food and one Production. These are opportunity counts, not measured returns.
The only qualifying frame was Emperor 61008601; its actual route was domestic.
No early foreign start met this filter with a domestic alternative.

No candidate decision was implemented, no fresh map was played, and this is
neither a causal production-gain experiment nor Firaxis verification. The next
investigation should explain scarce growth/production opportunities instead of
forcing a route that the existing controller already chooses.

## Frozen runtime and validation scope

The standalone probe links the normal optimized library frozen for #3953 at
`37f669c9157b8185ab77e965cdb9e5aa65422597`, explicitly asserting its escort flag
false. That frozen native control had already reproduced the preceding pure
baseline on all eight maps; this additional observer reproduces it again.
The library contains disabled historical code and is not represented as a
new current-main library build. Normal opt-level-three probe compilation passed.
After one current-main merge, the native/Cargo/data tree matches main
`a640e8737ee8765095e6bb8e817244930b787e2f` exactly. No new full Cargo invocation is required for
this standalone diagnostic archive; the frozen optimized probe compilation,
eight complete equivalence replays and standalone formatting passed. Required
repository CI runs separately. Reproduction commands and hashes are in the adjacent
manifest, with complete outcomes and equivalence checks in the results JSON.
