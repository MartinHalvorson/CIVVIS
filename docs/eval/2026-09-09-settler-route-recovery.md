# Settler route recovery, 2026-09-09

Run `civvis-20260909T013436Z`, host Settler `720902` (journal unit `58`),
reached offset `(7,26)` on turn 43. The host recorded `did_not_move` for its
MOVE_TO orders on turns 44 and 45. On turn 46 the controller barred the
issued axial step `(-5,26)` for eight standard turns.

That bar was effective in `BasicAi::path_move`, but the ordinary pathfinder
continued to return the barred step. `settler_step_toward` immediately returned
the failed move rather than routing around it. Independently,
`retire_frozen_settler_targets` retired whichever city destination was currently
cached on every replan while the bar was active. Thus turns 46–51 discarded
successive high-ranked sites without new host evidence against those sites.
The journal misleadingly described the resulting hold as every neighbour
being unsafe.

The repair keeps a reachable city's ranking and searches for the shortest
route in tiles that excludes the refused tile. It uses the normal terrain,
territory, city-entry and first-step occupancy rules, and passes the resulting
waypoint through the existing civilian and escort safety checks. If no route
remains, the site's deferral ends with the movement refusal instead of lasting
thirty standard turns. With the host-refusal treatment off, ordinary routing
is unchanged.

## Evidence

The saved turn-48 export reproduces the stationary old movement call at
axial `(-6,26)`, targeting `(-4,25)`. With the known refusal of `(-5,26)`
injected, the repaired call retains that target and moves to `(-6,27)`
(offset `(7,27)`). This is an offline reconstruction of the host board and
known refusal memory, not execution of the new order in Civilization VI or
a replay of all persistent AI memory.

The event prefix is preserved on the host at
`/Users/martbot-mbp-m5-max-128/civvis-settler-route-evidence-20260909/events-through-turn48.jsonl`,
SHA-256 `4a96e50bf080592093f062dc7bbe3c7e22ed902ecc21ab8bd3a46a633be200ff`.
The sibling receipt and journal extract identify the original run and unit.

Reproduce with:

```sh
CIVVIS_SETTLER_REPLAY=/path/to/events-through-turn48.jsonl cargo test --profile ci --locked --lib replay_turn48_refused_settler_route -- --ignored --nocapture
cargo test --profile ci --locked --lib settler_route_recovery
```

The controlled regressions cover repeated replans preserving the best site,
a detour whose first step increases geometric distance followed by founding,
retirement expiry, inactive treatment, and refusal to detour into a barbarian's
capture reach. The saved-export replay is explicitly ignored in CI because the
external run artifact is required; the controlled regressions run by default.

Full repository validation is pending. The new game routing query is called
only by the existing host-only move-refusal policy; simulator movement rules
and the native evaluation genome are unchanged, so a native policy soak does
not exercise this host-refusal repair.
