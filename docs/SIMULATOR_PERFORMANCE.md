# Simulator performance

This note records the July 2026 simulator profile, the changes kept from that
work, the production-catalog follow-up, and the next optimization targets.

## 2026-08-23: the 9% BTreeSet row was never the attack envelopes

The section below ranks *"the next target is `AttackEnvelopes = Vec<(u32,
Arc<BTreeSet<Pos>>)>`"* on the strength of `BTreeSet<Pos>` holding **9.0% of
the main thread**. The row is real. The attribution was not.

`sample`'s collapsed self-time list gives a symbol, not a caller, and
`BTreeMap<(i64,i64), SetVal>` is every `BTreeSet<Pos>` in the binary at once.
Walking the call tree instead of reading the symbol name puts **92% of those
samples in one line**: the `seen` set of the naval-recon flood fill in
`BasicAi::naval_recon_can_chart_from`. Same command as that section, `ci`
profile, `--map continents`:

| symbol, main-thread share | |
| --- | ---: |
| `BTreeSet<Pos>::insert` | 9.02% |
| ├ `naval_recon_ship_can_chart` (via `AdvancedAi::naval_explorer`) | 8.3% |
| └ everything else, envelopes included | 0.7% |
| `naval_recon_ship_can_chart`, inclusive | **13.19%** |
| `evacuation_tile`, inclusive | 1.31% |
| `incoming_damage`, inclusive | 1.31% |

So the two changes that section ranked first and third were aimed at 0.7% and
2.6% of the run, and the 13% sitting beside them had no entry in this file at
all. #2033 gave that walk an early exit and its docstring still says *"building
it was 13% of the simulator"* — the early exit is exact and it works, but its
worst case is a fully-charted body with no frontier, which is what a settled
game mostly has, and that case still has to exhaust the ocean to answer "no".
What was left after the early exit was the price of the *set*.

**⚠ Read a `sample` self-time row as a symbol, never as a subsystem.** This
file has now mis-ranked a target that way once; the collapsed list is a lead,
and the call tree is the finding.

### What was done

Four changes, all exact, all verified by `tools/speed_ab.py` reporting the
same report digest on every paired seed.

1. **The naval-recon visited set is a dense table.** `TileGrid::index_of` is
   documented for exactly this — *"callers that keep their own per-tile table
   index it by this"* — and `seen` is only ever tested and set, never iterated,
   so the frontier order, the tile count and the early-exit point are the ones
   a `BTreeSet` gave. `naval_recon_early_exit_agrees_with_the_full_walk` still
   checks the walk against the reference `naval_recon_waterway`, which keeps
   its tree.

2. **`incoming_damage` tests its sources before pricing its defender.** It
   cloned the unit, ran `unit_strength` and read `tile_defense_bonus` first,
   then discovered on most tiles that no envelope covered them and no walled
   district could see them — and a garrisoned tile threw away three answers it
   had already paid for. The empty folds it replaces returned exactly
   `IncomingDamage::default()`, and `default().merge(default())` is
   `default()`.

3. **`retreat_step` stops pricing the escape from a tile nothing can reach.**
   It runs for every military unit of every seat every turn — 87% of that bill
   is on city-states and barbarians, per the 2026-08-22 section below — and on
   a quiet tile it priced a heal rate, a City Center's ownership and a suzerain
   only to read `0.0`, conclude "not lethal" and return `None`. The pre-test is
   `anything_can_reach`, the same source scan `incoming_damage` performs
   anyway. `a_tile_nothing_can_reach_prices_at_exactly_zero` asserts the
   implication the skip rests on over a real board rather than arguing it in a
   comment.

4. **`AttackEnvelopes` holds a sorted slice.** Decided on counted work; see
   the section below for the numbers and for why the clock could not decide
   it. `Game::attack_reach_from_flood` also built the answer twice — a
   `BTreeSet` that its caller re-collected into a second one — and now builds
   one `Vec` the envelope adopts.

`safe_healing_step`'s whole-map scan now takes its covered-tile union from a
cache keyed on the envelope table's own `Arc` — pointer identity is sound as
the key because the entry holds that `Arc`, so the allocation cannot be freed
and reused underneath the comparison, and `enemy_attack_envelopes` builds a
fresh one whenever the envelopes change. ⚠ That one is an argument, not a
measurement: the set is identical and the cache can only turn several builds
of it into one — `military_step` reaches `safe_healing_step` up to three times
for one unit on one board — so it cannot cost more than a mutex and a pointer
comparison. It is not separately priced and should not be quoted as a number.

### Rejected, with the number: the envelope union as a membership test

The first version of change 3 answered it from that cached union: one binary
search instead of one per envelope. `sample` priced the union build at **1.8%
of the main thread against the 1.2% the skip saves**. Building an 800-tile
union to answer a single membership question is a loss, and the union earns
its keep only across `safe_healing_step`'s 3,400-tile scan. Recorded here
because the reasoning for it was good and the measurement was not.

### The envelope representation: decided by counting, not by the clock

The change that section ranked first was built, measured, reverted and then
restored, and the round trip is the useful part.

Measured on the clock it reads **+1.17%** and **+0.20%** — paired,
`tools/speed_ab.py`, 4 seeds each, 6p 60×38 6CS 120t online, reports agreeing
on every seed, host load 6.5 and 5.6:

| seeds | baseline | candidate | |
| --- | ---: | ---: | ---: |
| 7311001–04 | 42.87 s | 43.37 s | +1.17% |
| 7311010–13 | 38.90 s | 38.98 s | +0.20% (inside the noise floor) |

Both are inside the band #2339 has since shown this fleet cannot resolve the
*sign* of, so neither is evidence of anything, and no quiet window would have
made them evidence. Counted instead, with a counting `GlobalAlloc` over one
250-turn 74×46/9CS continents game at seed 7311001 (deterministic: two runs of
the same arm differed by 3 allocations in 375 million):

| arm | allocations | bytes |
| --- | ---: | ---: |
| `Arc<BTreeSet<Pos>>` | 375,799,768 | 45.13 GB |
| sorted `Vec<Pos>` | 374,642,802 | 44.87 GB |
| **difference** | **−1,156,966 (−0.31%)** | −253 MB (−0.56%) |

And the shape of the workload, counted in the same run: **271,484 envelope
builds against 12,123,375 `contains` calls**, on sets averaging 17.3 tiles —
44 queries per build. The failure mode a sorted `Vec` has here is a set built
once and queried rarely, where the sort costs more than incremental inserts;
this is forty-four times the opposite. Per query, two separately allocated
nodes and ~7.5 linear comparisons (Rust's `BTreeMap` searches a node linearly)
become ~4.1 comparisons over 136 contiguous bytes.

At ~45 ns an allocation that is ~0.05 s, and the query side is worth perhaps
as much again, on a ~43 s game: **an estimated ~0.3%, which no clock on this
machine can confirm.** It is kept because the counted work is strictly lower
on both axes that can be counted exactly, not because a percentage said so —
and **~0.3% is an estimate from counts and must never be quoted as a
measurement.**

⚠ The counting build is not in the tree. It is a `GlobalAlloc` wrapper in
`src/main.rs` incrementing two `AtomicU64`s in `src/lib.rs`, printed at the
end of `ai::run_game`; roughly thirty throwaway lines, and worth rebuilding
the next time a sub-1% change needs deciding.

### What Part 2's target was actually worth

The same brief expected 5–10% from a zero-incoming pre-test in
retreat/healing. `evacuation_tile` and `incoming_damage` are **1.31% of the
main thread each**, so the whole target was ~2.6% before any of it was
skipped, and the changes above take it to 0.68% and 0.68%. The large
inclusive shares `healing_step` and `retreat_step` carry — 27.7% and 24.8% on
this profile, 41.0% and 35.9% on the 2026-08-21 one — are almost entirely
`enemy_attack_envelopes` (17.6%) and the `flow_past` beneath it, not the
evacuation pricing. **That is where the next real win in this subsystem is,
and it is the same problem #2155 left open: the envelope table is rebuilt
whenever one of the viewer's own units moves.**

### ⚠ The paired harness measures a map no screen runs

`tools/speed_ab.py` does not pass `--map`, so it measures `civvis simulate`'s
default, `tennis_ball`. `gene_screen`'s `SCREEN_MAP` is `MapScript::Continents`
(#2308). The same profile, same binary, same 74×46/9CS/250t shape:

| | `tennis_ball` (what the harness times) | `continents` (what every batch runs) |
| --- | ---: | ---: |
| `BTreeSet<Pos>::insert` | 1.29% | **9.02%** |
| `naval_recon_ship_can_chart` | 1.42% | **13.19%** |

A twelve-point hotspot is invisible to the gate on every pull request. This is
the same trap the 2026-08-22 section below records for map *size* — *"every row
above it was taken at 60×38 / 6 and measures a shape no screen runs"* — one
axis further in. `tools/speed_ab.py` is not touched here because another pull
request owns it as this is written; **adding `--map` to it is the follow-up
this section is asking for.**

TIMING_PLACEHOLDER

## 2026-08-22: the movement flood stopped rescanning the world (−10.0%)

Two whole-board facts were being recomputed inside the innermost movement
loop. `flow_past` calls `can_enter_past` once per neighbour of every tile it
expands, and each call:

1. **walked every unit in the world** looking for an enemy air patrol over the
   tile being stepped onto — the code said so in a comment and it was still
   there; and
2. reached `class_can_traverse`, which asked
   `improvements[name].effects["passage"]` — a `BTreeMap<String, f64>` descent,
   `memcmp` per level — for every tile.

Neither answer can change inside a `&self` query, so both moved into the
existing `QueryMemo` scope: `air_patrols` (almost always empty — a patrol needs
a fighter) and `passage_improvements` (a `Vec<bool>` indexed by `Name::id`, one
array read). No invalidation question arises, because the memo is created and
dropped around the query and the board is immutable inside it.

**Measured with `tools/speed_ab.py`, which refuses a speed claim unless both
arms played the same game:**

| seed window | baseline | candidate | |
|---|---:|---:|---|
| 7311001..7311008 | 269.78s | 245.01s | **−9.18%** (air patrols only) |
| 7311001..7311008 | 263.85s | 237.16s | −10.11% (both) |
| 7311020..7311027 | 332.28s | 298.98s | −10.02% (both, quiet host) |
| 7311040..7311047 | 255.64s | 224.53s | −12.17% (both, against merged `main`) |
| 7311060..7311067 | 326.39s | 285.51s | −12.52% (both arms rebuilt at `5f2699fd`, after #2308) |
| **7311080..7311083** | **171.49s** | **152.07s** | **−11.32%** — at the **standard screen shape**, 74×46 / 9 city-states |

⚠ **The last row is the one worth quoting.** #2308 made 74×46 / 9 city-states
`gene_screen`'s bare default, so that is the shape a batch actually pays; every
row above it was taken at 60×38 / 6 and measures a shape no screen runs.
⚠ Rebuild **both** arms at the base the change lands on — `main` moves hourly
here, and a stale baseline flatters or punishes the reading.

*same game on every seed* in all six. The air-patrol hoist is ~9 points of
the 10; the passage table is the last one.

⚠ This is an optimization, not a feature, so whole-game wall clock is the right
measure here — both arms play the identical game, which is exactly the
condition the 2026-08-21 retraction below says makes it valid.

### The profile that found it, and what is left

`/usr/bin/sample` at 1 ms over `civvis simulate --seed 7311001 --jobs 1
--players 6 --turns 250 --width 60 --height 38 --city-states 6 --speed online
--map continents`, self time:

| | before | after |
|---|---:|---:|
| `BTreeMap<(i64,i64), SetVal>` — ~~the `BTreeSet<Pos>` envelopes~~ **the naval-recon visited set, corrected 2026-08-23** | 7.6% | **9.0%** |
| `memcmp` | 5.0% | 6.0% |
| `free` / `malloc` | 7.3% | ~7% |
| `memmove` | 5.4% | 5.7% |
| `tile_has_visibility_line` | 5.1% | 5.5% |
| `BTreeMap<String, f64>::get` | 3.6% | 4.2% |
| `can_enter_past` | 3.9% | **gone from the top 16** |

~~**The next target is `AttackEnvelopes = Vec<(u32, Arc<BTreeSet<Pos>>)>`.** Its
`BTreeSet<Pos>` is queried with `contains` and iterated in sorted order; a
sorted `Vec<Pos>` with `binary_search` gives the identical iteration order and
identical answers with no node allocation and contiguous memory, which should
take a large share of the BTree, malloc and memmove rows at once. Not attempted
here — it touches ~40 sites and deserves its own change.~~

⚠⚠ **WRONG, AND CORRECTED BY THE 2026-08-23 SECTION AT THE TOP OF THIS FILE.**
`BTreeMap<(i64,i64), SetVal>` is *every* `BTreeSet<Pos>` in the binary
collapsed into one symbol, and 92% of that 9.0% is the visited set of the
naval-recon flood fill in `BasicAi::naval_recon_can_chart_from`, not these
envelopes. The paragraph above was written from the collapsed self-time list;
the call tree says otherwise. The envelope conversion was subsequently built and
kept, but on counted allocations rather than on the clock, and it is worth
about 0.3% rather than the large share this paragraph expected. The row itself
is real and is now 0.13%.

⚠ **Read the last section first.** Each profile here superseded the one above
it, and the most recent — 2026-08-22 — corrects the 2026-08-21 section directly
above it, whose whole-game cost figures conflated the feature's cost with the
game-length change it causes. The 2026-08-21 profile supersedes **two** of the
tables below:
the "the profile is now flat" conclusion from 2026-08-17, and the "Largest
remaining opportunities" table under it, whose top row is now 1.1% of the run.
(This line itself said "2026-08-18" while four 2026-08-19 sections sat under
it. A pointer to the newest section is only worth having if it moves.)
Percentages below are diagnostic signals, not an additive decomposition:
library routines such as `memcmp` and `memmove` are costs incurred by several
higher-level systems.

## The harness

⚠⚠ **`tools/speed_ab.py` is the method this file has been describing in prose
since July, and it was not in the tree until 2026-08-18.** Every session rebuilt
it from these paragraphs, and this document already records what that cost: a
real **−11%** change read as **+8%** because the arms ran sequentially instead
of interleaved; one seed reading **+26.7%** purely from another session's games
sharing the host; a hoisted allocation that measured as an improvement and was a
**10x pessimization**.

    tools/speed_ab.py --baseline target/ci/civvis --candidate /tmp/civvis-new
    tools/speed_ab.py --baseline X --candidate X    # the noise floor, here, today

It pairs the arms seed by seed and flips their order between seeds; strips the
timing line and hashes each game report, so a timing difference only counts as
overhead when both arms played the same game; counts other CIVVIS processes
before and after; and reports anything inside ±0.2% as noise rather than as a
result. If the reports disagree it refuses a speed claim entirely, whatever the
timing said — which is the one case where a large clean number is worth least.

Everything below is a reading taken with that method, by hand, before it was a
file. The traps are described in place because they were found in place.

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

⚠⚠ **SUPERSEDED 2026-08-21 — see the re-profile at the end of this file.** The
first two rows below are the ones this table exists for, and they are now
**1.1%** and **0.6%** of the run. Read them as history: what they record about
*why* a scan is expensive is still true, and what they say about *where the
time is* has not been true since the envelope work landed.

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

## 2026-08-17 the profile after the settlement work, and three rejections

#1911 and #1917 both cut the settlement scan. This is what the profile looks
like afterwards, and three further changes that were built, measured and
**rejected**. All four measurements used the same harness: byte-identical
game reports between arms (so the agent made every same decision and any time
difference is pure overhead), then interleaved runs taking the best of two per
arm per seed. Baseline against itself reads ±0.2%, which is the noise floor
every claim below is judged against.

⚠ **A second session was running CIVVIS games on the same host for part of this
work.** One seed read +26.7% purely from CPU contention. Where a result mattered
it was re-measured with the host quiet, or established with a deterministic
counter instead of a clock.

### The profile is now flat

| leaf | % of working samples |
| --- | ---: |
| `memmove` / `memcmp` / `free` / `malloc` together | **~18%** |
| `vision_input_stamp_with_suzerains` | 2.3% |
| `Sphere::disk` + `Sphere::distance` | 3.8% |
| `settlement_growth_forecast_from_positions` | 2.1% |
| `district_adjacency_assuming_with_family…` | 1.9% |
| `Name::new` | 1.9% |
| `build_reverse_flow_field` | 1.8% |

Nothing named is over 2.5%. The settlement scan no longer dominates, and the
largest remaining block is not a function but the allocator.

### Rejected: hoisting the interned family name out of the neighbour filter

The `district_family` adjacency arm called `Name::new(district_family)` inside
its per-neighbour filter, in both branches — an obvious hoist.

**It is a 10× pessimization, and only a counter shows it.** The intern sat
behind `t.district.is_some_and(…)`, which short-circuits on the common case of
a neighbour with no district. Instrumented over one 150-turn game: the arm is
entered **11,053,403** times while the intern inside the filter ran **1,025,006**
times. Hoisting moves 1.03M interns to 11.05M. Wall-clock could not resolve it;
the counter is unambiguous.

(#1917 later restructured this arm so the family is resolved once and compared
as a family rather than re-derived per neighbour, which is the shape that
actually pays.)

### Rejected: a plot-scoped memo of neighbouring district families

Which families sit beside a plot does not depend on which district is being
priced, and the scan prices about 4.4 districts per plot, so a plot-scoped memo
should remove repeated six-neighbour scans. It does: instrumented at
**11,053,403 arm entries → 7,381,112 scans**, 3.67M scans removed per game.

Measured **−4.1% wall on six seeds** against the pre-#1917 baseline, with
identical play. Then #1917 landed, and re-measured on top of it the same change
is **−1.2% on four seeds and −0.1% on four more** — inside the noise floor.
#1917 removed the same work more directly by not valuing candidates the scan
discards, so the repeats were largely already gone. Closed unmerged: an extra
parameter, a scratch buffer and a branch in the hottest adjacency arm are not
worth 0.1%.

**The lesson is about overlap, not about the idea.** Two sessions optimizing one
subsystem in parallel will each measure a real win against a baseline that the
other is about to remove. Re-measure against current `main` immediately before
shipping a performance change, not against the tree you started from.

### Rejected: reserving the sphere disk and ring result size

`hex::disk` reserves its exact `1 + 3r(r + 1)` size, and the July pass records
that as one of four changes worth keeping. `Sphere::disk` and `Sphere::ring`
were never given the same treatment: both `collect` a filtered iterator, which
has no size hint, so a nineteen-tile radius-2 disk grows 1→2→4→8→16→32. The
globe is what `--shape` defaults to, so this is the path the shipped
configuration takes.

Reserved both. **−0.2% over six seeds — exactly the noise floor.** The
allocations are small and short-lived and this platform's allocator handles them
cheaply; the growth copies are a `memmove` of at most 152 bytes. Reverted rather
than shipped, because a correct-looking change that buys nothing is still a
change to read.

### What this says about the ~18% in the allocator

Three separate attempts to remove allocation and copying from named hot spots
bought nothing measurable, which agrees with the July finding that a wholesale
borrowed API also showed no win. The allocator cost is real but it is **spread
thin** — no single call site owns enough of it to repay a targeted fix. Anything
that moves this number will have to be structural (arena or reuse across a whole
turn), and that is a correctness and determinism risk rather than a local edit.

## 2026-08-18 the flat profile lasted one day, and a promoted feature owns 13%

⚠⚠ **THE SECTION ABOVE IS WHAT A PROFILE LOOKS LIKE BEFORE THE NEXT FEATURE
LANDS.** It concluded that the profile was flat, that nothing named was over
2.5%, and that the only remaining lever was structural. Twenty-four hours and
about forty merges later, a fresh `sample` of head — same method, same shape,
`ci` binary, one 6-player 74×46 nine-city-state Online game at `--jobs 1` —
reads:

| subsystem | % of working samples |
| --- | ---: |
| **`naval_recon`** | **13.3%** |
| `forcing_reply_*` | 6.1% |
| `Game::do_end_turn` | 5.1% |
| `refresh_all_visibility` | 4.2% |
| `Game::speculative_clone` | 1.6% |
| `advanced_settler_step` | 1.1% |

The settlement scan, which owned a fifth of the game on 2026-08-17 and has two
sections of this document to itself, is now **1.1%** — #1911 and #1917 did what
they claimed. And the largest cost in the simulator is a subsystem that did not
appear in any earlier profile in this file, because it was promoted the same
week (#1989, #1997: the open-water navy).

`BTreeSet<(i64, i64)>::insert` was the single hottest leaf in the whole
program: 710 of ~8,140 working samples, all of them under
`BasicAi::naval_recon_ship_can_chart`.

### The component was never the question

`naval_recon_waterway` flood-filled the entire connected navigable-water
component into a `BTreeSet<Pos>`. Both of its callers passed that set straight
to `naval_recon_waterway_can_chart` and kept one boolean. On a 74×46 map the
open sea is thousands of tiles, and the arm ran the fill once per naval unit
and twice per city, inside a check that production calls per city.

The irony is recorded three functions higher in the same file, in
`city_has_open_water`: *"Deliberately the six neighbours and not a flood fill:
the exact question is the size of the connected body, and `production_value`
cannot afford a flood fill per candidate per city per turn."* The naval-recon
arm was doing exactly that.

The predicate is `size >= 4` — monotone — and two existential quantifiers over
the same set. All three are settled by a **prefix** of the walk, so
`naval_recon_can_chart_from` evaluates them as it walks and stops when the
answer is fixed.

**The early exit is exact, not a heuristic.** Every tile it counts is a tile the
reference set contains, so a `true` from a prefix is a `true` from the whole;
and a walk that exhausts its queue has dequeued exactly the reference set, so a
`false` is the reference's `false`. `naval_recon_early_exit_agrees_with_the_full_walk`
asserts that against the retained full-walk implementation over four maps at
three exploration stages — including the fully-charted stage, where every walk
must run to exhaustion and every answer is `false`, which is the case the early
exit cannot short-circuit.

### What it bought

Counted first, because the clock is the thing this document keeps catching out.
One 150-turn six-player game at seed 7,310,002:

| | walks | tiles dequeued |
| --- | ---: | ---: |
| before | 23,943 | **8,681,806** |
| after | 23,943 | **1,870,273** |

**78.5% of the flood-fill work removed**, 6.8M fewer dequeues per game, with
the same answer every time.

Then the clock, `tools/speed_ab.py`, three independent seed blocks, byte-identical
game reports in every pair:

| seeds | games | result |
| --- | ---: | ---: |
| 7311000–7311007 | 8 | **−3.57%** |
| 4200000–4200007 | 8 | **−4.50%** |
| 5550000–5550009 | 10 | **−5.61%** |

Noise floor on the same host the same hour, baseline against itself: **+0.04%**.

### Rejected: bounding the *number* of walks as well as their length

`naval_recon_is_the_missing_arm` counts naval eyes across the whole empire and
then compares the total against an `arm_target` of **one or two**; and it runs
`city_has_naval_recon_launch` over the city list twice, where the second pass
only asks whether *any* city passed — which the first pass already knows. So:
hoist `major_naval_war` (it reads no waterway) to fix `arm_target` first,
`take(arm_target)` on the eye count, and fold the two city passes into one.

Exact, and it works: walks **23,943 → 18,062** (−24.6%), tiles **1,870,273 →
1,679,322** (−10.2%). Wall clock, same eight seeds: **−3.57% → −3.60%**.

It removes 191k dequeues out of the 8.68M the early exit was up against — about
0.3% of the run, under half the noise floor. Two declarative iterator chains
become an imperative loop with two more early returns, in a function that
already has several. Recorded and reverted, on the same rule the sphere-reserve
change was reverted under: a correct-looking change that buys nothing is still a
change to read.

**What that says about the rest of this subsystem.** After the early exit the
average walk is 78 tiles, and what remains is dominated by the walks that must
exhaust to answer `false`. The `BTreeSet<Pos>` those walks probe is now ~1.7M
operations a game — under a percent of the run at any plausible cost per
operation. This subsystem is finished at the local level; the next reader should
profile before assuming otherwise.

### The standing rule this round adds

**A promoted feature is a performance event, and the profile is stale the day
after it lands.** `naval_recon` went from absent to the largest cost in the
simulator in one week, in code that passed a strength gate — which measures
Elo, not tiles. Nothing in CI would ever have reported it. Re-profile after a
batch of merges, not only after a performance change.

## 2026-08-19 one promoted feature made every simulation six times slower

⚠⚠ **THE STANDING RULE ABOVE FIRED WITHIN A DAY, AND NOTHING IN CI SAW IT.**
`speed_ab.py`'s own shape — one 6-player 74×46 nine-city-state 150-turn Online
game at `--jobs 1`, `ci` profile, seed 7311001 — measured on this host:

| build | seconds per game |
| --- | ---: |
| `d3f624da` (2026-08-18 18:21Z, the commit before #2059) | **16.7** |
| `b70c689b` (#2059 "Evacuate threatened units to safe healing ground") | **102.7** |
| head `be44fa63` (2026-08-19 01:00Z) | 119.1 |
| head with `precise_evacuation` forced off | 24.8 |

So the flag #2059 turned on for `BasicAi::new()` — every city-state,
barbarian and current `advanced` seat — owns ~95 of head's 119 seconds, and
the forty other merges of the same evening own the remaining ~8. A `sample` of
head, main thread, working samples: `retreat_step → enemy_attack_envelopes →
Game::attack_reach` **33%**, `healing_step → safe_healing_step →
evacuation_tile → evacuation_incoming_damage` **52%** (summed over every
call site; the two overlap under `healing_step`).

### What it was doing

- `retreat_step` runs for **every military unit** of the seat on every turn,
  before any withdrawal threshold, and starts by recomputing
  `enemy_attack_envelopes` — one `attack_reach` flow field per visible enemy
  military unit — for that unit alone. `unit_visible_to` answers `true` for
  every non-stealth unit, so a barbarian seat priced the reach of every army
  on the map, once per barbarian, every turn.
- `safe_healing_step`, for every recovering unit, walked **all 3,404 tiles of
  the map** and priced each with `evacuation_tile`: a defender strength on
  that tile, the attack strength of every enemy whose envelope covers it, and
  a scan of every hostile city with `is_at_war` per city per tile.
  `suzerain_of` — a full envoy count per minor — was asked twice per tile
  outside any memo scope. That is why `is_at_war`, `suzerain_of`,
  `envoys_at` and `established_governor_at` were the top self-time leaves.

### The repairs, both exact

1. **Envelopes once per board.** `enemy_attack_envelopes` is memoized behind
   `(turn, viewer, board fingerprint)`, where the fingerprint is FNV-1a over
   every unit's identity, owner, place, health, movement and fighting state,
   every city's identity, owner and place, the map epoch and the war ledger —
   everything `attack_reach` reads, own units included because an own unit's
   zone of control ends an enemy's move. A hit therefore returns what a
   recompute would have, byte for byte. The cache is shared across controller
   clones (`Arc<Mutex<…>>`), which is the whole win: `plan_general_unit_turn`
   plans a batch of units on clones of one board, and the first per-clone
   version of this cache — the obvious `RefCell` — was computed once per unit
   and dropped with the clone, i.e. it did nothing (profiled, then replaced).
2. **`safe_healing_step` decides safety by membership.** A tile's incoming
   damage is zero exactly when it is a garrison district, or lies outside
   every envelope and out of strike range of every hostile walled city and
   encampment (each covering source contributes at least the clamp's floor of
   one) — so the scan tests set membership computed once per call instead of
   pricing 3,404 tiles, and reads its heal rates under a `query_memo` scope so
   `suzerain_of` is answered once per minor.

| seed | `main` (913b85d5) | with both repairs | reports |
| --- | ---: | ---: | --- |
| 7311000 | 136.2 s | 67.3 s | identical |
| 7311001 | 147.0 s | 86.9 s | identical |

And on the paired harness itself, `tools/speed_ab.py --seeds 7311000 --games 4`
against the same `main` binary: **461.75 s → 306.26 s user CPU, −33.7%, reports
agree on every seed** (the harness interleaves the arms and runs the pair
concurrently, which is why its per-game figure sits under the serial ones
above). Between 1.5× and 2×, byte-identical. Measured but **not shipped**: keying the envelope
cache on enemy state only (own units left out) makes the same games 34 s and
39 s — the serial paths (city-state and barbarian `military_step`,
`advance_unit_serial`) then hit as well — but every report differs, because
an own unit that steps out of an enemy's path widens that enemy's reach and
the stale envelope does not see it. That is a behaviour change to #2059's
policy and needs #2059's own gate, which the PR did not run; it is recorded
here as the next 2× if the owner wants it.

**Gated the same night, and the answer is no — as a shortcut.** The enemy-only
key is now an evaluator arm, `advanced_envelope_own_moves`
(`BasicAi::enable_envelope_cache_across_own_moves`, off in production), so
the trade can be priced instead of argued: `ai_eval
advanced_envelope_own_moves advanced --matrix --pairs 40 --stop-when-decisive`
returned **RETAIN advanced**. Compact-standard 50.6 % (INCONCLUSIVE, no
regression); deployment-online 50.0 %, 9 sweeps each way; deployment-contested
— the profile with `live_target_*` seats in the field, the one that looks most
like the ladder — **43.8 %, 11 of 80 games won against 21, twelve maps to two
by direction, exact sign p = 0.013**. A stale envelope is not free: in a
contested game the own unit that just moved changes the enemy's reach that
the next unit's retreat, healing and settling decisions read, and reading the
old reach loses ground. The 2× stays on the table only through the exact
algorithmic route above (envelopes for enemies within reach of own units,
recomputed when *those* enemies or *those* own units move), never through
staleness. The arm remains so that route can be measured against it.

### What is left, and whose it is

Even exact, the simulator is **four to five times slower than the day before
#2059**: one full envelope computation per seat-turn is inherent to a design
that prices the reach of *every* enemy on the map for *every* seat, and
`retreat_step` runs its evacuation pricing for every unit before any
threshold. The bounded fix is algorithmic — envelopes only for enemies within
their maximum reach of any own unit or candidate tile, or the retreat check
behind a cheap "is any envelope near this unit" test — and it changes which
tiles a unit reads as threatened only where the answer was already zero, so
it can be made exact too. It belongs to #2059's lane.

The standing rule stands, sharpened: **a promoted feature is a performance
event, and this one was a six-fold event that no strength gate could see.**
`speed_ab.py` costs four minutes; a strength gate on a feature that
multiplies game cost by six costs a day. Run the paired speed harness before
promoting anything that adds a per-unit or per-tile pass.

## 2026-08-19 — the envelope key hashed three fields no reach rule reads

`sample` on head, `civvis simulate --seed 7311001 --jobs 1 --players 6 --turns
150 --width 74 --height 46 --city-states 9 --speed online`, `ci` profile,
9,053 main-thread samples:

| symbol | samples | share of main thread |
| --- | ---: | ---: |
| `healing_step` | 7,034 | 77.7% |
| `retreat_step` | 6,917 | 76.4% |
| `enemy_attack_envelopes` | 6,778 | **74.9%** |
| `attack_reach` (the leaf) | 6,629 | 73.2% |
| `safe_healing_step` | 130 | 1.4% |

The envelope cache from #2059's repair is exact and does work — `safe_healing_step`,
which that repair rewrote, is down to noise. What remained was the *key*.
`attack_envelope_fingerprint` hashed every unit's `hp`, `moves_left`,
`attacks_left` and `fortified`, and no reach rule reads any of them:
`attack_reach` flows from `unit_max_moves`, never `moves_left`, and `flow_past`
relaxes the stacking layer so other units reach it only through
`in_enemy_zoc_for` — owner, kind, religion, promotions. So every attack, every
fortify and every spent movement point was a cache miss that recomputed one
flow field per visible enemy.

The key now hashes id, owner, place, **kind**, formation and `zoc_stopped`.
`kind` is new and closes a hole rather than opening one: an upgraded unit
changes the spec `exerts_zoc` reads, at the same tile, and the old key saw that
only because an upgrade also happens to spend movement.

Paired, `tools/speed_ab.py`, `ci` profile, 6p 74×46 150t online, `--jobs 1`:

| seeds | baseline | candidate | |
| --- | ---: | ---: | ---: |
| 7311000–03 | 173.09 s ← noise floor, one binary against itself | 172.95 s | −0.08% |
| 7311000–03 | 169.77 s | 160.60 s | **−5.40%** |
| 7311010–13 | 161.71 s | 154.89 s | **−4.21%** |

Reports agree on every paired seed — the harness refuses a speed claim
otherwise — and `advanced_v1_plays_the_same_game_it_always_did` passes
unchanged at 17,482 decisions and `0x8162_c919_b83c_40df`, so not one decision
moved across the five anchor profiles. `the_envelope_key_ignores_state_no_reach_rule_reads`
pins both halves of the claim.

### What is left, and it is most of it

**This takes 5% of a 75% hotspot.** The key still hashes every unit's *place*,
correctly — an own unit's zone of control really does end an enemy's move — so
a serial path that moves a unit between two asks still recomputes every enemy's
`attack_reach`. Movement is the common case, which is why the remaining cost is
what it is.

The bounded fix is per-enemy locality, and it has to be built more carefully
than the note above this section suggested. That note proposed computing
"envelopes only for enemies within maximum reach of any own unit"; that is
**not exact**, because `safe_healing_step` unions *every* envelope and scans
the whole map, so a distant enemy's envelope legitimately excludes a distant
healing tile. The exact route is to keep computing every enemy's envelope but
cache each one on a key covering only what can change it — that enemy's own
state plus the units and cities within its maximum reach — so a move forty
tiles away is a hit rather than a recompute. Bounding "maximum reach" needs the
minimum step cost the route rules allow, not one tile per movement point.

⚠ Promotions are still absent from the key and `exerts_zoc` reads
`promotion_effect(u, "zone_of_control")`. A promotion granting ZOC to a unit
standing still is invisible to it. Pre-existing, named here rather than
silently carried.

## 2026-08-19 (later) — an envelope is recomputed only when its own neighbourhood moved

#2148 tightened the board-wide envelope key to what reach reads and took 5%.
The remaining 70% was the key's *granularity*: it moves whenever any unit
moves, so a serial path that steps one own unit recomputed an `attack_reach`
flow field for every visible enemy on the map.

Each envelope now carries its own key, over the units and cities within
`envelope_reach_bound` of that enemy. The board key stays as the fast path; on
a board-key miss, each enemy is asked separately and only those whose
neighbourhood changed are recomputed.

| seeds | baseline | candidate | |
| --- | ---: | ---: | ---: |
| 7311000–03 | 166.23 s | 98.02 s | **−41.04%** |
| 7311010–13 | 156.47 s | 92.07 s | **−41.16%** |
| 7311020–23 | 148.39 s | 87.95 s | **−40.73%** |

Reports agree on every paired seed, and
`advanced_v1_plays_the_same_game_it_always_did` passes unchanged at 17,482
decisions and `0x8162_c919_b83c_40df`.

### The radius, and why it is what it is

`envelope_reach_bound` is `4 × max_moves + 1 + max(1, attack_range)`. The 4 is
`1 / 0.25`: terrain defaults to 1 MP and every feature that declares a cost
adds 1, so an off-route step never costs less than 1, and the only discounts
are the route ladder's — 0.75 Industrial, 0.5 Modern, 0.25 Railroad, the floor.
`envelope_reach_bound_matches_the_shipped_route_ladder` pins that floor against
the shipped data, and `the_reach_bound_covers_every_tile_attack_reach_returns`
runs the real `attack_reach` over every military kind and checks no tile it
returns lies outside the radius.

⚠ Air units are excluded and always recompute: `attack_reach` centres their
disk on `air_operation_origin`, not the unit's tile, so a key built around the
tile would watch the wrong neighbourhood. They are a disk, not a flow field.

### ⚠ What the attempt to test this the obvious way found

A fixture that searches for a tile where moving an own unit changes an enemy's
envelope **finds none, at any distance, for any shipped unit kind**. A
two-movement unit's flood is spent by the time zone of control could bite, and
cavalry ignore incoming zone of control outright. That is a large part of why
this cache wins as much as it does — and it means no fixture can tell a
one-tile radius from a ten-tile one, so the radius is defended by the bound
test above rather than by a placement search. A first draft of the warm-versus-cold
test passed against a planted radius of 1; it is recorded in the test's own
comment so the next reader does not repeat it.

### What is left

`enemy_attack_envelopes` should now be well under the 74.9% of main thread it
held before #2148. **It has not been re-profiled** — the standing rule in this
file is to re-profile after every landed win, and that is the next step here,
not a claim this one makes.

## 2026-08-19 (re-profile) — the key that saved the recomputes became the cost

#2151 landed and, per the standing rule at the top of this file, was
re-profiled. Same workload, `ci` profile, 6,423 main-thread samples:

| symbol | before #2148 | after #2151 |
| --- | ---: | ---: |
| `enemy_attack_envelopes` | 74.9% | 55.6% |
| `attack_reach` | 73.2% | 42.0% |
| `enemy_envelope_key` | — | **7.1%** |
| `safe_healing_step` | 1.4% | 2.6% |

The win landed. But hashing each enemy's neighbourhood costs a sweep of every
unit and city *per enemy, per ask*, and that sweep had become 7.1% of the main
thread on its own.

Two changes, measured separately against the same merge-base:

1. **Stop sorting a table that is already in order.** `Units` wraps a
   `BTreeMap` keyed by id and `Game::cities` is one, so `values()` already
   arrives in id order; the first cut collected each into a `Vec` and sorted it
   anyway. Hashing in place: **−2.53%**.
2. **Replace the hash with a board delta.** An enemy's envelope depends on the
   map, the wars, the cities and the units within its radius. If none of those
   changed since the previous ask, an envelope that was right then is right
   now — so the reuse test becomes "did any changed tile land inside my
   radius", over a change list that is usually one unit long because the caller
   has stepped one unit and asked again. This removes `enemy_envelope_key`
   entirely: **−11.03%, −12.01%, −10.39%** on three paired batches.

Reports agree on every paired seed and the anchor passes unchanged at 17,482
decisions and `0x8162_c919_b83c_40df`.

⚠ The induction only holds while the delta is refreshed on **every** ask, and
`None` — "assume everything changed" — is what a first ask, a map edit, a war
or any city change returns.

### Where the tests had to move, and why

`a_warm_envelope_cache_answers_what_a_cold_one_computes` catches a reuse gate
that always reuses, and **nothing else**. Planted defects that dropped the tile
a moved unit *left*, that skipped the wholesale fallback, and that ignored a
removed unit all survived it — because with shipped units an own unit's move
never changes an enemy's envelope at all, so a whole-board fixture cannot
reach those paths. `the_board_delta_reports_every_tile_a_change_touched` tests
`envelope_board_delta` directly and refuses all three.

That is the general lesson from this pair of changes: when the observable
behaviour is provably unchanged, the test has to move down to the invariant
that makes it unchanged, not stay at the behaviour.

## 2026-08-19 (third re-profile) — a tighter invalidation set, refused twice

Re-profiled after #2155, as the rule at the top requires. 5,614 main-thread
samples:

| symbol | after #2151 | after #2155 |
| --- | ---: | ---: |
| `enemy_attack_envelopes` | 55.6% | 51.7% |
| `attack_reach` | 42.0% | 47.9% |
| `envelope_board_delta` | — | under 1% |

The delta is cheap, so what remains is genuine recomputation. The obvious next
step is a tighter invalidation set, and the obvious tighter set is exact on
paper: other units enter `attack_reach` only through `in_enemy_zoc_for`, and
zone of control only bites on a tile the unit can step onto, so an envelope
should be sensitive to its own **movement flood dilated by one** — forty-odd
tiles for a two-movement unit rather than the three hundred in a
`4 × max_moves` disk.

Implemented, it measured **−26.0% and −21.0%**. It is not shipped, because
`tools/speed_ab.py` refused it: **ARMS DISAGREE on two of four seeds, in both
batches.** The flood set is not a complete account of what an envelope reads.

⚠ **The anchor passed throughout.** `advanced_v1_plays_the_same_game_it_always_did`
reported 17,482 decisions and the same fingerprint at every stage of this
work, including both refused versions. Its five profiles do not reach the
divergent path; a 6-player 150-turn game does. The paired report hash is the
stronger check of the two and neither substitutes for the other.

Two leaks were found and fixed on the way, and both are holes in the **shipped**
delta rather than in the experiment:

- `can_enter_past` refuses a step onto a tile a hostile fighter is patrolling,
  and finds that fighter by scanning **every unit in the world** — so the
  blocker can sit arbitrarily far from the tile it blocks. The delta tracked
  place, kind and owner, so starting or moving a patrol produced an *empty*
  delta and every envelope was reused across it.
- `is_at_war` consults `at_war` and, for a city-state, `suzerain_of`, which is
  derived from every major's envoys. The stamp carried `wars.len()`, which an
  envoy changing hands does not move.

Neither fixed the disagreement, so a third leak remains unidentified. What the
sequence establishes is the shape of the problem: **the delta's field set has
been incomplete all along, and the generous radius was masking it** — almost
any nearby unit moving invalidated anyway. That is now bounded rather than
guessed: the delta tracks exactly the unit fields `attack_envelope_fingerprint`
hashes, plus the patrol, and
`the_delta_tracks_every_field_the_board_key_hashes` mutates each field and
fails if the delta stops noticing one.

### If you pick this up

Do not re-derive the flood argument; it is correct as far as it goes and still
insufficient. Find the third leak first, and the way to find it is a diverging
seed: run seed 7311000 under both arms and diff the reports, rather than
reasoning about what `attack_reach` reads. Two rounds of inspection found two
real leaks and still missed one.

## 2026-08-19 (fourth) — the leak, found by audit rather than by inspection

The section above records a tighter invalidation set that measured −26%/−21%
and was refused twice by `tools/speed_ab.py`, with the leak unfound after two
rounds of reading the code. It said to find it by evidence. That worked, in one
run.

⚠ **Correction to the record:** commit `d474fa16`'s subject line reads "An
envelope is invalidated by its own flood, not by a disk around it". That
describes the approach that PR *refused to ship*; its title was set when the
worktree opened and never revised when the experiment failed. The body and the
section above are accurate. The flood approach ships **here**, in #2163.

### The instrument

`CIVVIS_ENVELOPE_AUDIT=1` makes every cache reuse also compute the envelope
fresh and describe the first six disagreements: the enemy, its state, the tiles
gained and lost, and every unit within three hexes of them. Reading it took
seconds. Every report had the same shape — **tiles gained, none lost, around a
unit whose cached envelope was empty.**

### The leak

`flow_past` returns nothing at all when `formation_movement_locked_by_zoc`
holds. A sensitivity set built from the flood was therefore **empty**, and an
empty set is touched by no board change ever — so that envelope froze at empty
for the rest of the game, long after the lock lifted. The unit kept reading as
harmless while it stood there able to attack.

Two fixes, and the second came out of reading what the lock depends on:

1. The sensitive set is always seeded with the unit's own tile and its
   neighbours, and its linked peer's, flood or no flood. The lock is read off
   that ground.
2. `started_turn_in_zoc`, `acted` and `moved` are now tracked by the delta.
   All three move without the unit moving — a unit that acts where it stands
   loses its whole flood — and none is hashed by the board key, so nothing else
   would have noticed.

| seeds | baseline | candidate | |
| --- | ---: | ---: | ---: |
| 7311000–03 | 93.03 s | 69.05 s | **−25.77%** |
| 7311010–13 | 80.53 s | 62.80 s | **−22.02%** |
| 7311020–23 | 79.15 s | 63.04 s | **−20.35%** |

Reports agree on every paired seed — the same harness that refused this twice —
and the anchor is unchanged at 17,482 decisions.

Cumulative over #2148, #2151, #2155 and this: the same four-game workload has
gone from **173 s to about 65 s**.

### What this cost, and what it is worth

Three iterations of inspection found two real leaks and missed the one that
mattered; one audit run found it. The lesson is not that inspection is useless —
the two leaks it found were genuine holes in shipped code — but that **a cache
whose invariant spans a large surface should ship with a way to check itself.**
`CIVVIS_ENVELOPE_AUDIT` stays for that reason, read once through a `OnceLock`
so it costs nothing when unset.

## 2026-08-21 (fifth) — the sight frame: who asks, and what the asking costs

Re-profiled at HEAD `0f1b04e0`, as the rule at the top requires — the fourth
profile predates ten landed PRs. The reading was taken by sampling a **live p7
`gene_screen`** for 40 s rather than a purpose-built run: that is the workload
that actually burns the core-hours, and sampling a process already running
costs the host nothing.

The top named CIVVIS symbol is no longer anything in the envelope family:

| symbol | self-samples |
| --- | ---: |
| `vision_input_stamp_with_suzerains` | **8,784** |
| `tile_has_visibility_line` | 3,425 |
| `hex::disk` | 1,662 |
| `can_enter_past` | 1,609 |
| `BasicAi::enemy_attack_envelopes` | 504 |

`enemy_attack_envelopes` — 74.9% of the main thread two profiles ago — is now
504 samples. That work is finished. Sight replaced it.

### What one headless game spends on sight

A one-off instrumented build counted the asks and what each one walks. The
measurement code was removed before submission so it adds no branch to this
hot path. Seed 7311001, 6p 60x38 pangaea online, all six victories, decided
turn 232:

| | per game |
| --- | ---: |
| `vision_frame` asks | 128,826 |
| …answered from cache | 76,641 (**59.5%**) |
| world units scanned by the stamp | **25,221,623** |
| units the scan kept (owner match) | 2,881,751 — **89% discarded** |
| city owned-tile positions hashed | **8,592,020** |

⚠ **The stamp is computed on every one of the 128,826 asks, hit or miss.** It is
a content hash by design, and that design is right: hashing only the fields
`base_player_visibility` reads is what stops a hit-point change from evicting a
map-sized bitset. The defect is not that it hashes; it is that hashing costs a
walk of the whole world. `self.units.values().filter(|u| u.owner == viewer)`
visits every unit on the board to keep roughly one in nine.

### Who asks 128,826 times

Attributed from the sample call tree, by samples in stack:

| caller | share |
| --- | ---: |
| `combat_target_visible` | **22,575 (92%)** |
| `player_can_see` | 1,337 |
| `refresh_player_visibility_via` | 373 |
| `player_vision_now` | 190 |

⚠ **This is a trap this document already named and only half-fixed.** The
2026-08-18 entry records that `combat_target_visible(pid, pos)` recomputes both
frames per call, and that the fix is `combat_target_visible_at(pid, pos,
&visible, &viewers)` with the frames hoisted. The tactical picker was converted.
**The engine's own five call sites never were** — `game.rs` 35375, 37362, 37492,
39170, 39264, inside `do_ranged`, `do_attack` and `do_city_strike`. Every shot
the engine applies still asks the unhoisted wrapper.

### Why that is so expensive: the branch started cold

`impl Clone for VisionFrameCache` returned `Self::default()`. Every `Game` clone
therefore began with **no sight frames at all**. The tactical picker scores a
candidate by cloning the world and applying the order, and applying a shot calls
`combat_target_visible` — so each candidate paid a full ray-cast derivation on a
world discarded microseconds later.

The comment defending it said a branch "is allowed to move its sources before
its first read, and copying a parent's bitsets would retain work that belongs to
the parent position." The stamp already refutes that: a frame is reused only
when `vision_input_stamp_with_suzerains` still matches, and that stamp folds
every input the derivation reads. A branch that moves a sight source restamps
and recomputes; a branch that does not would have derived the bitset it
inherited. The four fields `speculative_clone` changes — `track_fog_memory`,
`track_war_ledger`, `visibility_suppressed`, `visibility_batch` — are read by
neither the stamp nor the derivation.

A clone now inherits the frames (an `Arc` bump per seat).

| workload | baseline | candidate | |
| --- | ---: | ---: | ---: |
| 6p 74x46 150t online, 6 seeds | 165.32 s | 163.55 s | **−1.07%** |
| 6p 60x38 250t online, 4 seeds | 183.57 s | 180.17 s | **−1.85%** |

Reports agree on every paired seed. Cache hit rate 59.5% → 63.9%.

⚠ **Read that number as small on purpose.** It converts misses into hits, and a
hit still pays the stamp. The 25.2 M-visit scan is untouched, which is why a
change that removed a full ray-cast from most speculative clones is worth under
two percent. Wall-clock on the shared host said −21% for the same change; the
paired harness said −1.85%. That gap is the whole reason the harness exists.

### Next, and it is most of the win

Two independent targets remain, in order of expected value:

1. **The 25.2 M-visit unit scan.** `unit_vision_input_stamps()` already walks
   the units once and stamps *every* player, and `base_vision_input_stamp`
   already accepts the result as `unit_stamps`. `vision_frame` passes `None`.
   Memoize the vector on the game behind a units-mutation epoch: `Units::get_mut`
   and `values_mut` already insert into a branch write-set, so the choke point
   for the bump exists. ⚠ Over-invalidation is **safe here** in a way it was not
   for the envelopes — a stale epoch only recomputes the *stamp*, while frame
   validity still rests on the content hash. Worst case is one pass where there
   is one pass today; it cannot regress correctness.
2. **The 8.6 M owned-tile hashes.** Every ask hashes every owned tile of every
   city of every viewer. Borders move rarely; a per-city tiles counter bumped on
   mutation would replace the inner loop with one number.

And separately, convert the five engine call sites to `combat_target_visible_at`
with the frames hoisted once per order, which is what the 2026-08-18 entry
already told the next reader to do.


## 2026-08-21 (re-profile) — the settlement scan is 1.1%, and half the run is threat

The rule at the top of this file says re-profile after every landed win. Four
landed (#2148, #2151, #2155, #2163) and the last section said the re-profile
"is the next step here, not a claim this one makes". This is that step, taken
after 133 further merges, and it moves the target.

`/usr/bin/sample` at 1 ms, `ci` profile, head `cefe73b8`, this file's own
reference workload — `civvis simulate --seed 7311001 --jobs 1 --players 6
--turns 150 --width 74 --height 46 --city-states 9 --speed online` — 14,141
main-thread samples. Inclusive shares, summed over every call site, with a
symbol never counted inside itself.

| subsystem | % of main thread |
| --- | ---: |
| `AdvancedAi::take_turn` — all deliberation | 100% |
| `BasicAi::military_step` | **50.6%** |
| ├ `healing_step` | 41.0% |
| ├ `retreat_step` | 35.9% |
| ├ `enemy_attack_envelopes` | 29.0% |
| ├ `attack_reach` | 23.4% |
| ├ `flow_past` | 21.0% |
| └ `can_enter_past` | 15.3% |
| `advanced_units` → `plan_general_unit_turn` | 38.3% → 36.4% |
| `advance_unit_serial` | 16.9% |
| `forcing_reply_*` | 11.0% |
| `explorer_turn` / `explore_step` | 6.1% / 5.7% |
| `city_yields` | 4.9% |
| `legal_purchase_actions` | 4.3% |
| `research_with_government` | 3.3% |
| **`Game::do_end_turn` — the engine's own turn** | **3.2%** |
| `policy_card_score` | 2.9% |
| `refresh_all_visibility` | 2.5% |
| `production_value` | 1.7% |
| **`advanced_settler_step`** | **1.1%** |
| └ `settlement_atlas_values` | 0.5% |

### The table this supersedes

"Largest remaining opportunities, superseding the July table" (2026-08-17)
ranks *narrow the settlement candidate set* first, on the strength of the
settler search being **20% of the run**, and *memoize district adjacency per
plot* second at ~7%. Those two rows are now **1.1%** and **0.6%**. The work
that moved them is in this file — the four skipped questions (2026-08-17) and
`settle_site_exists` (#1911) — plus the fact that everything around them got
more expensive. Neither is worth doing for speed any more, and the
cost-versus-value paragraph under that table, which asked whether the
settlement search is worth its breadth, is now a question about **1%**.

⚠ That paragraph is still worth reading for its argument, which was never
about the percentage: *the single most expensive subsystem in the simulator is
the one whose decision treatments have most consistently failed to demonstrate
strength.* It was written about expansion. It is now true of the threat
machinery, which is why the next section exists.

### The regression is paid off, and the shape changed underneath it

The same command that measured **16.7 s** on `d3f624da` (the commit before
#2059) and **119.1 s** at the 2026-08-19 head now measures **21.4 s**
(`real 21.40 / user 20.82`, host load 3.4 before and 2.8 after). Four exact
caches took a 6x event down to about 1.3x. ⚠ The 16.7 s reading carries no
recorded host load, so the residual 1.3x is a comparison across days and is
worth re-taking paired before anyone spends a week on it.

But the profile is not the shape it was before #2059 either. `military_step`
now holds **half the main thread**, and the engine's own rules — everything
`do_end_turn` does to advance a turn — hold **3.2%**. The rules are not the
cost and have not been since July; what changed is which part of the
controller is.

## 2026-08-21 — the most expensive default in the simulator had never been priced

`precise_evacuation` (#2059) is `true` in `BasicAi::new()` and
`BasicAi::with_weights()` — every major, every city-state, every barbarian.
Only `AdvancedAi::legacy()` withholds it, behind the frozen `advanced_v1`
anchor. `retreat_step` returns `None` immediately when it is false, so one
flag gates the whole block that owns half the profile above.

`tools/speed_ab.py`, four paired games per row:

| shape | on (head) | off | |
| --- | ---: | ---: | ---: |
| 6p 74x46 150t online | 78.22 s | 46.19 s | -41.0% |
| 6p 60x38 6CS 250t online (the `gene_screen` native shape) | 156.13 s | 80.03 s | -48.7% |

⚠⚠ **THOSE TWO NUMBERS ARE WHOLE-GAME WALL CLOCK AND THEY ARE NOT THE
FEATURE'S COST. Corrected below, in the section this one is superseded by** —
they are retained because the retraction is the useful part. The harness
prints ARMS DISAGREE on every seed, correctly: this is a behaviour flag. What
was missed is the consequence of that — a different game is a different
*length*, and this feature changes length. The same change measured **-14.2%**
on `20bce807` a day later at the identical shape and seeds. A number that
moves by a factor of three across one day of merges was never measuring what
the sentence around it claimed.

The profile is unaffected and stands: `healing_step` 41.0% and `retreat_step`
35.9% of main thread are shares of running time, which no change in game
length can move.

**What it was worth: nobody knew.** As shipped it had no `LIVE_TREATMENTS` row,
no `elo.rs` arm, no row in `docs/gene_ledger.json`, and no mention in
`docs/EVAL.md` or `docs/AI_GAPS.md` — `grep -n 'evacuation|retreat_step|safe_healing'`
over `src/elo.rs`, `src/bin/ai_eval.rs`, `src/bin/gene_screen.rs`,
`src/bin/civvis_orders.rs` and `src/ai/advanced/treatment_flags.rs` returned
nothing. #2059's Validation section lists `cargo test` and a twelve-game crash
soak. So the single largest consumer of every evaluation batch in the fleet was
also the one behaviour neither gate could address: not unpriced by oversight,
**unpriceable**, because a flag that is not an arm is invisible to both
instruments.

### What was done about it

- A `PRODUCTION_TREATMENTS` row and an `enable_/disable_` pair make it a
  screenable gene, so `gene_screen` and `ai_eval --without` can both reach it.
- ⚠ Adding that row also makes `gene_ledger::ledger_default_on` answer
  `Some(false)` for it — "unmeasured means off at deployment" — so the row and
  its measurement have to land together or the wiring change silently
  withholds a shipped behaviour. That is why it is one change and not two.
- `.github/workflows/speed.yml` runs `tools/speed_ab.py --budget` on every
  pull request. The standing rule above has now fired twice with nothing in CI
  watching; four minutes of paired timing per PR is what it costs to stop
  writing it down again.

### The standing rule, third statement

**A promoted feature is a performance event.** It was written after #2059 and
the sentence was already there before that. What was missing every time is not
the sentence: it is that no gate ran it. Prose in this file has now failed to
prevent the same class of event twice, so the next reader should treat a
change to this section as a change to `speed.yml` first and a paragraph
second.


## 2026-08-22 — the evacuation cost, per completed turn, and whose seats it is on

Two independent measurements of the same feature disagreed, and the
disagreement is what located the cost. `docs/EVAL.md`'s own rule — compute the
headline fact twice and read both — earned itself again here.

`gene_screen --genes precise-evacuation`, 9,000 seat pairs (3,000 games, seeds
61000000.., the native regime's own shape) reports the compute column as
**+2.90 ± 0.42% per completed turn, per enabled major seat**. That cannot sit
beside the section above's "-48.7% when withheld". Measured directly instead,
four seeds interleaved on one revision, dividing by turns so game length cannot
confound it:

| arm | CPU s | turns | s/turn | vs on |
| --- | ---: | ---: | ---: | ---: |
| on (`main`, `20bce807`) | 160.07 | 745 | 0.2149 | — |
| off for minors and barbarians only | 110.98 | 729 | 0.1522 | **-29.1%** |
| off everywhere | 136.28 | 951 | 0.1433 | **-33.3%** |

### 87% of it is on seats no evaluation is measuring

Minors and barbarians are 29.1 of the 33.3 points. The six major seats — the
only ones any evaluation reads — are the remaining four. `BasicAi::new()` is
city-states, barbarians and the `basic` entrant, and every one of them runs the
same per-unit envelope pricing as a major.

⚠ A gene screen structurally cannot see this. It varies major genomes; every
city-state and barbarian carries the flag ON in both arms of every pair. The
screen's +2.90% is per *major seat* and is correct; it is simply not the whole
bill, and nothing in the instrument says so.

### Why the whole-game numbers moved

The turn column is the other half. Withholding the feature makes games run
**longer** — 745 turns to 951 over the same four seeds, fewer early religious
victories — so whole-game wall clock measures the length change and the cost
change together, with opposite signs. That is how the identical change read
-48.7% on `cefe73b8` and -14.2% on `20bce807`.

**Per completed turn is the metric.** `tools/speed_ab.py` reports whole-game
user CPU and is right to: for an *optimization*, where the arms play the same
game by construction, length is held constant and wall clock is exactly the
answer. The moment the reports disagree — which is every promoted feature —
whole-game time stops being a cost and starts being a mixture. `gene_screen`
already separates the two columns for this reason; the paired harness does not,
and a reader who quotes its number for a behaviour change is quoting a mixture.
