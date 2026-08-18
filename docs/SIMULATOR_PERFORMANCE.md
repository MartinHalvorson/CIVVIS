# Simulator performance

This note records the July 2026 simulator profile, the changes kept from that
work, the production-catalog follow-up, and the next optimization targets.
Percentages below are diagnostic signals, not an additive decomposition:
library routines such as `memcmp` and `memmove` are costs incurred by several
higher-level systems.

## Representative workloads

Measurements used the `ci` Cargo profile and an otherwise idle serial CIVVIS
process as far as the shared development host allowed. The full-game comparison
used ten adjacent baseline/treatment pairs with four major civilizations, a
44-by-28 map, no city-states, a 200-turn limit, and seeds 7,310,000 through
7,310,009. CPU time is reported because unrelated jobs made wall time noisy.

The rollout comparison prepared the same four-player, 44-by-28 game through
turn 100 at seed 7,310,002, then took the median of four adjacent 5,000-sample
runs for each operation. Headless games disable fog memory, so the no-fog rows
most closely represent simulator and search use.

| Rollout operation | Baseline | Optimized | Latency change |
| --- | ---: | ---: | ---: |
| Clone only | 8.55 us | 8.50 us | -0.6% |
| Clone + move | 50.10 us | 32.15 us | -35.8% (1.56x throughput) |
| Clone + end turn | 301.90 us | 277.30 us | -8.1% |
| Clone + move, no fog memory | 38.60 us | 22.45 us | -41.8% (1.72x throughput) |
| Clone + end turn, no fog memory | 305.35 us | 286.35 us | -6.2% |

Across the ten paired full games, aggregate user CPU time fell from 16.96 to
15.76 seconds, a 7.1% reduction. Every seed was faster. After removing the
elapsed-time line, all ten baseline and optimized simulator reports were
identical. A separate optimized soak completed all 20 requested games.

## Changes retained

Four deliberately small changes produced that result:

1. Monopoly calculation now builds one connected-resource census per player
   and reuses it across every luxury and foreign player. Previously each query
   walked all owned city tiles again.
2. An ordinary adjacent `Move` no longer runs the first-monopoly transition
   scan after success. Moves onto tribal villages, meteor sites, or Barbarian
   Outposts still run it because their rewards can complete a technology and
   reveal a luxury resource. Other action kinds remain conservative.
3. `hex::disk` reserves its exact `1 + 3r(r + 1)` result size, avoiding growth
   copies in a primitive used by movement, combat, yields, and visibility.
4. A per-player/city production catalog retains the complete legal `Item` list
   across the separate read-only passes a city governor makes before acting.
   A successful `Game::apply` clears it, and game clones and saves start empty,
   so normal mutation and search branches cannot reuse a stale menu.

## Live policy-deck counterfactuals (2026-08-01)

The live policy deck was a newly isolated serial island inside an otherwise
parallel single simulation. On a review, it removes and restores every legal
or held card, reading the whole empire before and after each change. The
sampling evidence put this work under `BasicAi::research_with_government`, but
the expensive child was the counterfactual `empire_reading`, not technology
selection.

Two bounded changes keep its semantics intact:

1. Each read-only empire valuation holds one `QueryMemo` scope, so the city
   yield and ownership derivations shared by its cities are reused only while
   the policy slate is unchanged.
2. A fleet with its existing persistent `WorkPool` scores independent cards on
   worker-private game snapshots. Results return in the original candidate
   order, and the authoritative game's existing sort and serial deck commit
   remain the only mutation path. Interactive and baseline `BasicAi` runs
   retain their serial path.

Release `simulate` was measured on two fixed six-player, 74-by-46 maps with
nine city-states, 150 turns, online speed, and seeds 7,311,002 and 7,311,003.
Baseline and candidate order alternated across the two seeds. These are
shared-host elapsed times, so they establish a reproducible directional result
rather than a universal machine-level throughput promise.

| Internal workers | Baseline total | Candidate total | Change |
| --- | ---: | ---: | ---: |
| 1 | 15.150s | 14.067s | -7.1% |
| 4 | 13.968s | 11.404s | -18.4% |
| 18 | 14.183s | 12.057s | -15.0% |

After removing the simulator's elapsed-time line, every baseline/candidate
report had the same SHA-256 hash for each seed and worker count. A focused
regression also compares a full live-deck review with one versus four workers
and requires identical action logs and serialized game state. Four workers
were the best observed count for this limited workload; that is evidence for a
separate default-worker calibration experiment, not grounds to silently cap
all hosts or workload shapes.

### Bounded policy-score fan-out (2026-08-01)

The fleet pool is intentionally still sized by `--jobs`: unit, tactical,
purchase, and visibility frontiers can use every worker. A policy-deck review
has a much smaller independent batch, however, and each active scorer owns a
full `Game` snapshot. The scorer therefore caps only that batch at four active
workers and leaves the rest of the persistent pool available to later work.

This compared the uncapped #761 build with the cap on four fixed six-player,
74-by-46, nine-city-state, 150-turn online games (seeds 7,311,002 through
7,311,005), at `--jobs 18`. Baseline/candidate order alternated by seed. These
are shared-host elapsed times, so they show a directional single-machine win,
not a universal hardware default.

| Policy-score workers | Four-game total | Change |
| --- | ---: | ---: |
| Pool-wide (18) | 23.335s | baseline |
| Capped (4) | 22.764s | -2.4% |

Every normalized report had the same SHA-256 hash for its seed. A fixed
`--jobs 4` report was also identical before and after the cap, as expected
because the limit is already met. The evidence is specific to this clone-heavy
frontier; it does not change the global worker default.

### Single-simulation worker default (2026-08-01)

The remaining global choice is different from a batch default. `simulate`
uses one persistent pool for clone-heavy inner frontiers, whereas `soak`,
evaluation, and other batch commands assign whole independent games to their
workers. On this 18-core host, an omitted `simulate --jobs` therefore now uses
`min(available_parallelism, 4)`; an explicit `--jobs` remains authoritative
and all batch defaults remain one worker per available core.

Four fixed six-player, 74-by-46, nine-city-state, 150-turn online games
(seeds 7,311,030 through 7,311,033) compared four workers with the old
18-worker implicit setting. Run order alternated by seed. As with the other
single-host measurements, this supports the selected default here rather than
a universal hardware claim.

| `simulate` workers | Four-game total | Change |
| --- | ---: | ---: |
| 18 (former implicit setting) | 19.030s | baseline |
| 4 (new implicit maximum) | 18.649s | -2.0% |

Every normalized report had the same SHA-256 hash across worker counts. The
full observed curve also rose from 4 to 6, 8, 12, and 18 workers on the first
two seeds, while explicit user choices remain available for hosts or workloads
with a different knee.

### Bounded purchase-menu fan-out (2026-08-01)

`AdvancedAi::legal_purchase_actions` distributes independent city menus, but
every active worker owns a complete `Game` snapshot. A developed empire often
has only a few cities, so using every persistent-pool worker creates more
snapshots than the menu work can repay. The frontier now has its own cap of
three workers; results are still collected by city index and flattened in the
stock purchase-then-empire order before the authoritative AI chooses an
action.

The three-way calibration used four fixed six-player, 74-by-46,
nine-city-state, 150-turn online games at `--jobs 4` (seeds 7,311,055 through
7,311,058), rotating the run order. It selected three workers rather than a
generic low cap:

| Purchase-menu workers | Four-game total | Change |
| --- | ---: | ---: |
| Pool-wide (4) | 21.404s | baseline |
| Capped (2) | 21.232s | -0.8% |
| Capped (3) | 21.057s | -1.6% |

A separate four-seed default-path confirmation (seeds 7,311,063 through
7,311,066) improved from 19.997s to 19.477s. Together, the eight `--jobs 4`
games improved from 41.401s to 40.534s (**-2.1%**). Four additional explicit
`--jobs 18` games (seeds 7,311,067 through 7,311,070) improved from 23.643s
to 23.138s (**-2.1%**). Every normalized baseline/candidate report had the
same SHA-256 hash. The cap is local to this clone-heavy, bounded frontier; it
does not resize the fleet pool or alter the single-simulation worker default.

### Advanced-unit planner fan-out: rejected (2026-08-01)

`AdvancedAi::advanced_units` is also clone-heavy, but unlike the policy and
purchase frontiers it is a broad, expensive unit-intent planner. Its existing
dynamic batch gives every worker a private game and planner state, then plans
many general units from it. A local cap would preserve intent order and replay,
but did not repay the lost planning throughput.

Three alternating release comparisons used the same six-player, 74-by-46,
nine-city-state, 150-turn online workload. Each baseline/candidate pair had
an identical normalized report hash.

| Pool / cap experiment | Seeds | Baseline total | Capped total | Change |
| --- | --- | ---: | ---: | ---: |
| `--jobs 4`, pool-wide 4 vs cap 2 | 7,311,081–084 | 21.71s | 22.59s | +4.1% |
| `--jobs 4`, pool-wide 4 vs cap 3 | 7,311,085–088 | 22.62s | 23.47s | +3.8% |
| `--jobs 18`, pool-wide 18 vs cap 4 | 7,311,089–092 | 18.50s | 18.94s | +2.4% |

No unit-planner cap is retained. This is a useful boundary on the earlier
worker caps: a complete snapshot is not alone evidence that fewer workers
help. The next experiment in this hotspot should reduce work *inside* a unit
intent or reuse a measured immutable derivation, rather than suppress a
planner worker.

## Production-catalog follow-up

The production catalog was the narrow first experiment from the profile. Its
target is a real multi-city decision path: repairs, products, and wonder
fallbacks can each ask for the same city's legal production menu before a
single action is committed. Caching only the fully derived menu preserves the
existing item order and legality checks.

On a release paired measurement of 20 fixed-seed games (four major
civilizations, 44-by-28 map, no city-states, 200-turn limit, seeds 7,310,200
through 7,310,219), aggregate serial wall time fell from 44.57 to 41.34
seconds: **7.2%**, with a 6.8% median per-seed reduction. Baseline ran first
for ten seeds and the catalog build first for the other ten; the two subsets
improved 7.1% and 7.4% respectively. After removing the elapsed-time field,
all 20 paired terminal reports were identical. A separate 40-game,
eight-worker soak on the same map shape also had identical normalized reports
and reduced accumulated per-game time from 230.54 to 221.51 seconds (3.9%).

This is intentionally a targeted result, not a claim that every tiny
production query is faster. A short two-player throughput run was not
consistently improved; the retained cache earns its cost where a developed,
multi-city controller repeats the catalog scan.

Two broader prototypes were rejected rather than kept without evidence. A
borrowed replacement for `units_at` did not materially improve adjacent rollout
runs because most consumers still need a stable snapshot while mutating the
game. Filling `WorldMap::disk`'s final buffer directly was neutral to slightly
slower. An older sphere-cache experiment also remains rejected: admitted-row
lookup improved substantially but cold local lookup exceeded the project's
regression gate.

## Largest remaining opportunities

A post-change main-thread sample contained 8,282 active samples. The profile
was taken immediately before the small `hex::disk` reserve change, which does
not materially affect this ranking.

| Opportunity | Current signal | Promising next experiment | Main constraint |
| --- | --- | --- | --- |
| Intern effect and rules keys | `memcmp` 9.2%, `Name::new` 1.4%, and `SpecMap::position` 1.2% as exclusive leaves | The full effect-map conversion was exact but a 0.3% CPU loss; retry only after isolating a direct typed lookup in a measured hot path | Broad representation change; conversion alone leaves dynamic `&str` callers doing the same comparisons |
| Reuse movement and routing state | Traversal class, neighbors, passability, path checks, entry checks, and routing zones form roughly a 6-7% family | Identify one repeated rule derivation before changing it; generic search buffers and route-local traversal profiles were measured and rejected | Movement rules have many conditional abilities and diplomatic dependencies |
| Remove targeted allocation and copying | `memmove` 5.9% and allocator leaves are prominent; `units_at` alone is 1.4% | Add narrow `tile_occupied` and callback/iterator queries, then migrate read-only and `is_empty` callers individually | These costs overlap the systems above; a wholesale borrowed API showed no win |
| Reduce rollout clone cost | Clone alone is 8.5 us, 38% of the optimized no-fog clone-plus-move latency | Share more immutable state or prototype reversible branch deltas/undo for search | High correctness and determinism risk; cloning is currently the isolation boundary |
| Broaden visibility and yield memoization | Line-of-sight/world-stamp leaves are about 2-3%; yield and adjacency work is fragmented across roughly 3-5% | Extend epoch-keyed memoization and precompute stable adjacency facts | Smaller expected return and potentially high cache-invalidation complexity |

The string/key work is cross-cutting but invasive. With the production catalog
now retained, no generic routing-state reuse is a standing target: a future
movement experiment needs one measured repeated derivation first. These should
be measured independently because removing one source of comparison or
allocation cost will also shrink the apparent library leaves.

Operationally, independent games should continue to use the existing outer
`--jobs` parallelism. Production runs should use `--release`; its thin LTO and
single codegen unit are intentionally more optimized than the faster-building
`ci` profile used for the comparisons above.

## Where the allocations are (2026-07-31)

Every earlier profile here is a *time* profile — `sample`'s leaf ranking, or
inclusive timers around suspect functions. Allocation had only ever been
inferred from `memmove`/`malloc` leaves, which say how much it costs but not
who is doing it. This is the direct measurement.

**Method, and it is cheap to repeat.** A counting `GlobalAlloc` plus a global
slot index and an RAII `Tag` that swaps the slot and restores it on drop, so a
tagged function is charged exclusively and nested tags are charged to the
innermost. It needs no thread-local — the probe is run under `--jobs 1`, and a
lazily-initialised thread-local risks allocating inside the allocator. Roughly
sixty lines in a scratch module plus one tag line per suspect, applied and
stripped by a script; it must not reach a commit.

**One 6p 74×46 150-turn game allocates 13,728,753 times for 3.09 GB.** Game
setup is 142k of that, so essentially all of it is play. That is about 5,700
allocations per seat-turn, and at a plausible 30 ns per malloc/free pair it is
roughly a tenth of the game's runtime — consistent with the allocator and
`memmove` leaves in the time profile.

| site | allocations | share | bytes |
| --- | ---: | ---: | ---: |
| `advanced_units` | 5,842,083 | **42.6%** | 1.70 GB |
| the rest of `AdvancedAi::take_turn` | 3,038,944 | 22.1% | 597 MB |
| `city_yields` | 1,256,361 | 9.2% | 129 MB |
| `units_at` | 1,076,368 | 7.8% | 4.5 MB |
| `wdisk` | 817,304 | 6.0% | 128 MB |
| `begin_turn` | 606,103 | 4.4% | 69 MB |
| `legal_actions_within` | 325,139 | 2.4% | 51 MB |
| `player_unit_ids` | 213,378 | 1.6% | 9 MB |
| `player_city_ids` | 182,495 | 1.3% | 7 MB |
| `route_step` | 89,702 | 0.7% | 292 MB |

**Two thirds of all allocation is the AI's own decision code**, and
`advanced_units` alone is 42.6% of it. That agrees with the long-standing
finding that roughly two thirds of runtime is `AdvancedAi`'s deliberation
rather than the engine's rules, and it says the largest remaining allocation
work is in `src/ai/advanced.rs`, not in `src/game.rs`.

**Read the count column, not the byte column.** `route_step` is the warning:
292 MB — the largest byte figure outside the AI — from only 89,702
allocations, because each search allocates two dense per-tile vectors. Its
leaf time is about 1%. Large, short-lived, lazily-faulted buffers are cheap;
volume of small allocations is what shows up in the allocator leaves.
`units_at` is the mirror image: 1.08M allocations for 4.5 MB, an average of
four bytes, because it clones a one- or two-element `Vec<u32>` out of the
occupancy map.

### Route-search scratch reuse: rejected

The `route_step` byte total also suggested a generation-stamped, worker-local
scratch buffer for A* and breadth-first routing. The prototype preserved the
same search ordering and route rules, then compared release serial games with
six major civilizations, a 74-by-46 map, nine city-states, 150 turns, and
fixed seeds 7,310,500 through 7,310,514. Removing the per-game elapsed field,
all fifteen baseline/prototype reports were byte-identical.

| Five-game block | Run order | baseline | scratch prototype |
| --- | --- | ---: | ---: |
| 7,310,500–504 | baseline then prototype | 10.15s | 10.91s |
| 7,310,505–509 | baseline then prototype | 11.42s | 14.93s |
| 7,310,510–514 | prototype then baseline | 11.03s | 14.75s |

The prototype was slower in every block (40.59s versus 32.60s in aggregate),
so it was removed. It replaces cheap dense initialization with an extra
per-tile generation array and a thread-local borrow on every search; for this
map size, that additional cache traffic loses to the allocator. Future routing
work should isolate traversal or entry-rule derivation instead of retrying
generic scratch-buffer reuse.

### Route-local traversal profile: rejected

A follow-up passed the already memoized `TraversalClass` directly into A* and
breadth-first entry checks, avoiding the memo-map lookup at each neighbor. It
used the same release workload and three fresh five-game blocks (seeds
7,310,520 through 7,310,534). Again, every normalized report was
byte-identical.

| Five-game block | Run order | baseline | direct profile |
| --- | --- | ---: | ---: |
| 7,310,520–524 | prototype then baseline | 12.48s | 13.44s |
| 7,310,525–529 | baseline then prototype | 13.65s | 13.58s |
| 7,310,530–534 | prototype then baseline | 11.33s | 12.23s |

Two losses and one near-tie (39.25s versus 37.46s aggregate) do not clear the
regression gate, so this plumbing was also removed. The profile lookup is too
small relative to the rule checks that remain; future movement work needs a
measured expensive derivation, not a generic memo lookup.

**What this does and does not license.** It ranks allocation, which is one of
the two things this codebase has repeatedly found to be worth removing (the
other being an expensive derivation that is recomputed). It does not by itself
predict a win: the record already contains a neutral result for removing ~30
`String` allocations per citizen plan, and a neutral result for a wholesale
borrowed `units_at`. Take the count column as a ranking of *candidates* and
still A/B each one interleaved.

### The first thing this profile suggested, measured and rejected

The obvious read of the table is that `units_at` is worth attacking: 1.08M
allocations, and 82 of its 183 call sites are an immediate `.is_empty()`, so a
borrow-free `tile_occupied(pos)` removes the allocation entirely at every one
of them. That is also the experiment the opportunity table above proposes.

It was implemented — `tile_occupied` plus 50 converted sites, leaving
`src/ai/advanced.rs` alone to avoid a conflicting branch — verified
byte-identical on nine seed/player combinations, and measured against
`origin/main` over ten interleaved pairs:

| | main | `tile_occupied` |
| --- | ---: | ---: |
| best of 10 | 2.70s | 2.71s |
| median | 3.69s | 3.71s |

**0.996x best, 0.993x median, 52 of 100 pairs, p = 0.88 — a clean null.**
Reverted.

**This is the calibration the count column needed, and it should be read
alongside the table above.** `units_at` allocations average *four bytes*: they
are one-element `Vec<u32>` clones served straight from the tiny-allocation free
list, and a million of them cost no measurable time. So allocation *count* is
not by itself a predictor either — what the record's two paying allocation
fixes had in common was that the allocation also did work (a `format!` builds
and copies a string; a deep clone copies a structure). A malloc/free pair on
its own is nearly free here.

That makes this the eighth consecutive null for "remove cheap per-call work",
and it narrows the standing rule usefully: **the payer is an expensive
derivation that is recomputed, not a cheap operation that is frequent — and
allocation only counts as expensive when something is built.** Applied to the
table above, the interesting rows are the ones with a high byte-per-allocation
ratio inside the AI, not `units_at`.

### Interned effect-map keys: rejected

The remaining string-key opportunity was tested directly. The candidate
changed each numeric rules `effects` table from `BTreeMap<String, f64>` to
`BTreeMap<Name, f64>`, keeping the same lexical `Ord` and JSON string shape.
`Name` also exposed a borrowed `str` lookup so every dynamic caller continued
to compile unchanged. The source was then replayed against the unchanged
baseline over fifteen release headless games: six major civilizations, a
74-by-46 map, nine city-states, 150 turns, and seeds 7,310,700 through
7,310,714.

After removing the per-game elapsed field, all fifteen baseline/candidate
reports had identical SHA-256 hashes. The CPU result did not earn the added
representation:

| run order | seeds | baseline user CPU | interned effect maps |
| --- | --- | ---: | ---: |
| baseline then candidate | 7,310,700–705; 7,310,711–714 | 46.77s | 46.99s |
| candidate then baseline | 7,310,706–710 | 27.60s | 27.57s |
| aggregate | 15 pairs | 74.37s | 74.56s |

The median was a noisy 4.92s versus 4.90s, but the aggregate is a **0.3%
loss**, not an optimization. The mechanical conversion removes duplicate keys
at rules-load time, yet nearly all runtime effect queries still begin with a
borrowed string and therefore retain the original string comparison. It was
removed. A future retry needs one profiled high-frequency caller that can carry
a `Name` all the way to the map lookup; broad key conversion on its own is not
a standing target.

## 2026-08-17 profile — one settler owns a fifth of the game

The ranking table above is from July and its top row (intern effect and rules
keys) is no longer where the time is. This is a fresh `sample` profile of the
current head, taken the same way: `ci`-built `release` binary, one 6-player
74×46 nine-city-state online game at `--jobs 1`, `sample` at 1 ms.

⚠ **Read the `--jobs 1` caveat first.** `sample` counts blocked threads, so the
persistent pool's idle workers show up as `semaphore_wait_trap` — 5,505 of
10,542 samples in the first run. Every percentage below is against the working
main thread, not that total.

### Where the time is

| subsystem | % of main thread |
| --- | ---: |
| `AdvancedAi::take_turn` (all deliberation) | **95.2%** |
| ├ `advanced_units` | 24.4% |
| ├ `advance_unit_serial` | 20.4% |
| ├ `settle_sites_with_limit_cached` | 13.4% |
| ├ `production_value` | 7.8% |
| ├ `research_with_government` | 6.1% |
| ├ `policy_card_score` | 5.3% |
| └ `legal_purchase_actions` | 3.1% |
| `Game::do_end_turn` (the engine's own turn) | 4.6% |
| `refresh_all_visibility` | 4.2% |

**The rules are not the cost.** Everything the engine does to advance a turn is
under ten percent; the rest is the controller deciding what to do. That agrees
with the 2026-07-31 allocation census (two thirds of allocations inside
`AdvancedAi`) and sharpens it.

### One settler owns a fifth of the game

`advance_unit_serial` is 20.4% of the run, and it decomposes almost entirely
into a single unit's target search:

| step | % of the `advance_unit_serial` block |
| --- | ---: |
| `advanced_settler_step` | **98.6%** |
| └ `best_settler_target` | 85.5% |
| &nbsp;&nbsp;└ `best_reachable_settle_site_except_cached` | 79.7% |
| &nbsp;&nbsp;&nbsp;&nbsp;├ `settlement_atlas_values` | 65.4% |
| &nbsp;&nbsp;&nbsp;&nbsp;├ `settlement_static_value_uncached` | 38.3% |
| &nbsp;&nbsp;&nbsp;&nbsp;├ `settlement_adjacency_summary_from_positions` | 23.7% |
| &nbsp;&nbsp;&nbsp;&nbsp;└ `district_adjacency_assuming_…_cached` | 21.9% + 12.4% |

Every other unit in the empire together costs less than the settler's choice of
destination. The shape is `sites × disk × districts`: a scan considers ~154
candidate sites, each valued over its radius-2 disk of 19 plots, and each plot
is tested against every plannable district for adjacency yield. That is roughly
35,000 adjacency evaluations per scan, and a 150-turn game runs 965 scans.

### What was measured and rejected

Two plausible causes were tested and are **not** the problem. Recording them so
the next reader does not re-derive them:

- **Atlas thrashing.** `SettlementAtlas` invalidates on `(turn, map_epoch, pid)`
  and `WorldMap::get_mut` bumps the epoch on *any* mutable tile access, so the
  cache looked certain to be destroyed by ordinary unit movement. Instrumented:
  **428 rebuilds across 906 player-turns**, a 68% hit rate, and forcing the
  atlas to persist across turns and epochs changed misses by only 3% (96,052 →
  92,884) and did not reduce runtime. The turn/player key, not the epoch, is
  what bounds it, and it is behaving.
- **Measured-null flags.** `battlefront_observation`, `deny_leaders` and
  `plan_city_target` are recorded null or near-inert in
  `docs/AI_GAPS.md`, so they looked like free savings. They do not appear in the
  profile at all — under 0.1% each. They are null in value *and* in cost;
  removing them is hygiene, not speed. `plannable_districts`, recomputed once
  per candidate site, is likewise 0.12%.

### Largest remaining opportunities, superseding the July table

| opportunity | current signal | main constraint |
| --- | --- | --- |
| Narrow the settlement candidate set | the settler search is 20% of the run and scores ~154 sites to move one unit | breadth is the score's quality; a prefilter that changes the chosen site changes play |
| Memoize district adjacency per plot | `district_adjacency_assuming_…` is ~7% of the run, re-derived per candidate site over overlapping disks | the "assuming" argument is a counterfactual city center, so the memo key must carry it |
| Allocation volume in the controller | allocator and `memmove`/`memcmp` leaves are ~18% of working samples | unchanged from 2026-07-31; two thirds of it is `AdvancedAi` |
| Existence vs ranking queries | **done** — `settle_site_exists` (#1911) took −4.0% wall with byte-identical play | none; the pattern generalises to any `.is_some()` on a ranked scan |

### The cost-versus-value question

`docs/AI_GAPS.md` records that of the twenty-three production behaviours priced
by withholding, **two were worth keeping and one was actively harmful**; and
that expansion specifically is "the second replicated oracle ceiling" where
**seven decision treatments failed**. Set beside this profile, that is the
uncomfortable pair worth stating plainly: the single most expensive subsystem in
the simulator is the one whose decision treatments have most consistently failed
to demonstrate strength.

That is an argument for measuring the settlement search's *value* at its current
breadth — not for cutting it blind. A narrower scan is a play change and has to
clear the same gate as any other.
## The expansion scan: four skipped questions (2026-08-17)

The July profile above ranked *leaves*. The 2026-08-17 settler profile ranked
*subsystems* and found the settlement scan holding a fifth of the run, so this
round asked a different question — not "which operation is expensive" but
"which expensive derivation is computed and then discarded". Four were, and
removing them is the largest single-round CPU result recorded in this file.

Every change below is answer-identical by construction: it changes *whether* a
value is computed, never how. That claim is checked, not asserted — the four
150-turn, six-player, 74-by-46, nine-city-state, online-speed reports at seeds
7,320,000–7,320,003 are byte-identical to the baseline after the elapsed-time
line is removed, and `advanced_v1_plays_the_same_game_it_always_did` — the
anchor fingerprint over every action the frozen controller applies across five
profiles — is unchanged.

### 1. The Settler arm valued a site it had already decided not to build

`production_value`'s Settler arm opened with `best_settle_site(g, pid,
city.pos, 11)`, plus a whole-map scan once Shipbuilding is in, and then read
that site only inside a gate made of three integer comparisons and a window
test. Every city that was not in the market for a Settler — most cities, most
reviews, and `production_value` runs the arm for each of them — paid a valued
sweep of a 331-plot disk and threw the answer away. The gate now runs first.

### 2. The global prefilter pre-valued the candidates it had just excluded

`settle_sites_with_limit_cached` splits candidates at
`SETTLEMENT_GLOBAL_PREFILTER_LIMIT` so the expensive site value is paid for the
top 512 alone — and then handed `candidates.chain(overflow)` to the atlas,
which valued the overflow anyway. The prefilter saved nothing it did not
immediately give back. The overflow is read only by the `sites.is_empty()`
fallback, which reaches `settlement_atlas_value` per plot on demand and
computes the identical number there.

### 3. The founding-exclusion union was built for the radius that never runs

The union of `wdisk(city.pos, 3)` was materialized only for `radius > 12`,
leaving the two radii the expansion path actually walks — the radius-8 local
pass and the radius-11 scan in the Settler arm — running an O(cities) `wdist`
scan for every plot of the disk. `wdisk(pos, 3)` is exactly the set of plots at
`wdist <= 3`, so the union answers the same question at every radius.

### 4. `adjacent_land` was computed for six placement rules that never read it

`plot_fits_placement_with_neighbors` runs once per (plot, district) — six times
per plot for a stock civilization, across nineteen plots per site. It computed
`adjacent_land` (six map lookups and six water tests) eagerly, and only the
`coast`/`water_park` arm reads it. Moved into that arm. In the same table,
three `Name`-versus-literal comparisons in the district adjacency count
(`terrain == "mountain"`, `feature == Some("forest")`, `Some("jungle")`) became
interned `name!` comparisons: a `Name`/`&str` compare loads the entry out of
the 32,768-slot registry and then compares text, while `Name`/`Name` is one
integer compare, and interning is injective so the answer cannot differ. The
key's family name in the district-name arm is now interned once per key instead
of once per neighbour.

### Result

Paired A/B, alternating baseline and candidate seed by seed so host load falls
on both equally; `ci` profile, `--jobs 1`, eight games per arm.

| comparison | baseline user CPU | candidate user CPU | change |
| --- | ---: | ---: | ---: |
| changes 1–3 vs the branch point | 101.49s | 90.03s | −11.3% |
| change 4 vs changes 1–3 | 83.01s | 78.81s | −5.1% |
| all four vs `origin/main` after #1911 | 91.61s | 75.19s | **−17.9%** |

The last row is the one that matters and it is larger than the first two
compounded, because #1911's `stop_at_first` landed in between: an existence
query now stops at the first qualifying site *and* no longer pre-values the
overflow behind it. The two changes multiply rather than overlap.

### What this round says about the standing rule

The file's standing rule after eight consecutive nulls was: *the payer is an
expensive derivation that is recomputed, not a cheap operation that is
frequent.* All four wins above are the same shape and sharpen it by one word —
**recomputed or discarded**. Nothing here made an operation faster. Three of
the four removed a derivation whose result was never read, and the fourth moved
one behind the branch that reads it. The rejected experiments recorded earlier
in this file — interned effect maps, route scratch reuse, traversal profiles —
all tried to make frequent cheap work cheaper, and all measured null or worse.
That asymmetry is now eight nulls against four wins, and it has never gone the
other way.
