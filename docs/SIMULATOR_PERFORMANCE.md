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

### The paired reading

The tree's own harness gained a `--map` option the same day (#2328); this is
its method at the standard screen shape — 6p 74×46, 9 city-states, 250 turns,
online — 8 paired games:

| seeds | baseline | candidate | | |
| --- | ---: | ---: | ---: | --- |
| 7311001–08, continents | 513.09 s | 492.56 s | **−4.00%** | host load 17–50 |
| 7311001–08, `tennis_ball` | 411.13 s | 405.71 s | **−1.32%** | host load 30–44 |

**Same game on every paired seed**, which is the correctness proof this change
needed: three of the four edits skip work and the fourth changes a container,
and a matching digest on eight 250-turn games says none of them moved a
decision.

⚠⚠ **THE TWO ROWS ARE THE SAME BINARY PAIR AND THE SAME EIGHT SEEDS.** The
only difference is the map, and it is worth **three times the answer** — which
is the point of the subsection below on what the gate could not see. Quote the
continents row: it is the shape every `gene_screen` batch pays.

⚠ Taken while sibling agents held this host at load 17–95. `speed_ab.py`
interleaves the arms seed by seed and flips their order, so the *paired delta*
survives that and the absolute totals do not. The evidence each change was
actually decided on is load-independent: the `sample` share table above and
the allocation counts below.

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

### ⚠ The paired harness measured a map no screen ran — closed by #2328

While this work was in flight `tools/speed_ab.py` did not pass `--map`, so it
timed `civvis simulate`'s default `tennis_ball`, while `gene_screen`'s
`SCREEN_MAP` is `MapScript::Continents`. Same binary, same 74×46/9CS/250t
shape, one `sample` each:

| | `tennis_ball` (what the harness timed) | `continents` (what every batch runs) |
| --- | ---: | ---: |
| `BTreeSet<Pos>::insert` | 1.29% | **9.02%** |
| `naval_recon_ship_can_chart` | 1.42% | **13.19%** |
| **this change, paired, 8 seeds** | **−1.32%** | **−4.00%** |

The hotspot this section is about was **twelve points smaller on the map the
gate timed than on the map the fleet runs** — invisible to `paired-cost` on
every pull request. It is the same trap the 2026-08-22 section below records
for map *size* (*"every row above it was taken at 60×38 / 6 and measures a
shape no screen runs"*), one axis further in.

**#2328 closed it independently and while this was being written**: the gate's
shape is now `GATE_SHAPE`, pinned field-by-field against `gene_screen`'s own
`SCREEN_*` constants, with `map="continents"`. Recorded here anyway because
the two findings arrived from opposite directions — that one from auditing the
gate, this one from a hotspot that hid behind it — and because the pair is the
concrete size of what a default map costs a measurement.

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

## 2026-08-23 — the city half of the sight stamp, and the 11.6 M allocations nobody was counting

The 2026-08-21 (fifth) entry left two vision targets. The first — "the 25.2 M-visit
unit scan" — shipped as #2295. This is the second:

> 2. **The 8.6 M owned-tile hashes.** Every ask hashes every owned tile of every
>    city of every viewer. Borders move rarely; a per-city tiles counter bumped
>    on mutation would replace the inner loop with one number.

It is the right target and the wrong count, because **three** loops walked the
roster on every ask, not one:

- `base_vision_input_stamp` hashed every owned tile of every city the viewer
  holds — the 8.6 M, measured here at 7.5 M;
- `vision_geometry_stamp` hashed every city and every district in the **world**,
  viewer or not;
- `world_stamp` hashed the same world roster a third time, on the hotter path of
  the two: `height_field()` asks for it, and `player_can_see` and
  `player_vision_frame` build a height field per question.

⚠ **And the expensive part of those two was not the hashing.**
`impl IntoIterator for &Districts` is `self.iter().collect::<Vec<_>>().into_iter()`,
so `for (district, position) in &city.districts` **allocates a `Vec` per city per
ask**. Measured below: **11,616,354 allocations in one game**, from a loop whose
own arithmetic is a rounding error. The 2026-08-17 profile's largest single block
was not a function but the allocator at ~18%; this is some of it.

All three now read one number off the roster.

### The measurement that does not need a quiet host

Taken with a deterministic counter, because the host was at load 63–96 with a
dozen sibling agents building, and this file's own history is full of clocks that
read a change backwards under exactly that condition. One game, seed 7311001,
6p 74×46, 9 city-states, 250 turns, online — the shape the newest sections use.

| | per game |
| --- | ---: |
| `vision_stamps()` asks | 452,140 |
| …answered from the fold | 441,886 (**97.7%**) |
| rebuilds | 10,254 |
| owned-tile hashes | 7,532,856 → **5,884,334** (−21.9%) |
| district hashes | 26,083,412 → **790,477** (−97.0%) |
| city visits, each of which allocated a `Vec` | 11,616,354 → **0** |

Total hash work falls from **45.2 M to about 6.7 M** plus one city-header pass
per rebuild. The `Vec` allocations go to zero: the fold iterates with
`Districts::iter()`, which does not collect.

### Read the 21.9% row before copying this shape

The owned-tile row is the weakest of the three and it is worth understanding why:
a **rebuild folds every owner's tiles**, where the old loop folded only the
viewer's. At six majors plus nine city-states that is roughly a sixfold larger
unit of work, and a 97.7% hit rate buys back only 21.9% of it.

The obvious next move is a per-city fold with a dirty set, so that `get_mut(id)`
refolds one city instead of all of them. **Do not spend a day on it.** The
residual is 5.88 M hash operations at a few nanoseconds each — about 15 ms on a
game that costs 54 CPU-seconds, or **0.03%**. The allocations are the whole
number: 11.6 M at roughly 45 ns is about 0.5 s, or **1.0%**, and that is what the
paired harness measured.

### The paired harness, and what it proves

`tools/speed_ab.py --seeds 7311001 --games 8 --players 6 --width 74 --height 46
--city-states 9 --turns 250 --speed online`:

| | user CPU |
| --- | ---: |
| baseline (`5205a424`) | 433.82 s |
| candidate | 429.07 s |
| | **−1.09%** |

⚠ **Provisional on the timing.** Host load averages were 63–96 on 18 cores
throughout, with the harness itself reporting three other CIVVIS processes; a
number taken at that load is not a result, and the agreement with the 1.0%
allocation estimate above should be read as corroboration, not as confirmation.

**What is not provisional is that the reports agree on every paired seed.** That
is the correctness proof this change needs and it does not care about load: all
three stamps changed *value* — they fold the same inputs in a different order —
and a stamp that reached play rather than staying a cache key would surface here
as a divergent report. `docs/FLOAT_DETERMINISM.md`'s same-seed-same-world rule is
the contract, and eight identical paired digests are the evidence.

### Where the invalidation lives, and why it is not an epoch counter

`Units` carries a `u64` epoch bumped inside `get_mut`, and
`with_unit_vision_input_stamps` keys the unit fan-out on it. Copying that shape
for cities has a hole the unit version does not: an epoch and the memo it guards
are two values in two places, and `game.cities = other.cities` moves one without
the other. Nothing in the tree does that today. Nothing had to.

So the memo is a field of the roster instead:

```rust
pub struct Cities {
    map: BTreeMap<u32, City>,
    vision: RefCell<Option<Arc<CityVisionStamps>>>,
}
```

`Deref` — and **deliberately no `DerefMut`** — exposes the whole read API, so
every read, every `values()`, every `cities[&id]` compiled unchanged. Without
`DerefMut` nothing in the crate can obtain `&mut BTreeMap<u32, City>`, so the
only routes to a `&mut City` are six inherent methods, and each drops the memo
before it returns. Three properties follow, and they are the whole argument:

1. **The compiler enumerates the mutation surface, on every build.** A mutating
   map method not reimplemented here does not compile. `iter_mut`, `entry`,
   `retain`, `append`, `drain`, `split_off`, `pop_first` and `pop_last` are
   absent on purpose — nothing calls them, and an untested accessor is a worse
   guard than a build error. This is `AGENTS.md`'s "discover, never list"
   enforced by `rustc` rather than by a grep that was complete the day it was
   written.
2. **The memo cannot come apart from the roster.** Assignment, `mem::take` and
   `mem::swap` move the fold with the cities. `Cities` is declared inside a
   private `mod city_roster` rather than as a bare type, because a private field
   is readable by every *descendant* module and `game` has three dozen of them.
3. **Borrowck forbids reading the memo while a write is outstanding.**
   `vision_stamps()` needs `&Cities`; every mutable handle borrows `&mut Cities`.
   Invalidation is eager — it happens when the handle is *issued*, before the
   caller has written anything — so no ask can fall between the two. The new
   test needed a `let` hoisted out of `push(along(&branch, …))` for exactly this
   reason, which is the guarantee showing up as a compile error.

### What the enumeration found

The claim above is worth what the search behind it is worth. Every `.rs` file in
the repository, single-line **and** line-broken forms — an `rg -U` multiline pass
found 369 chained-across-lines sites a single-line grep misses. **~972
mutable-access sites to `Game::cities`**, 852 of them outside `src/game.rs`: 21
production (14 `mirror.rs`, 3 `ai/advanced.rs`, 2 `bin/civvis_orders.rs`, 2 the
feature-gated `oracle.rs`) and 831 in tests. **None needed an edit.** Confirmed
absent, each of which would have been a hole: any `&mut BTreeMap<u32, City>` in a
signature, any whole-roster assignment in production, any `Game { … }` literal
outside `game.rs`, any `for … in &mut …cities`, any path that moves a `City` out
and puts it back, and any `unsafe` or `UnsafeCell` route into the roster.

`owned_tiles` is written from twelve production sites — `found_city_for`,
`do_buy_plot`, `expand_borders` (twice), `fogged_clone`, six in `mirror.rs` where
the authoritative host reassigns a worked tile, and `oracle.rs`. Ownership moves
in `mirror_set_city_owner`, `fogged_clone`, `transfer_city` (capture, cession, and
the loyalty flip that routes through it) and `do_liberate_city`. Cities appear and
disappear in `found_city_for`, `do_raze_city`, `mirror_remove_city` and
`clear_mirror_cities`. Save and load go through `GameSer`, whose `Vec<City>`
becomes a `Cities` by `FromIterator` with an empty memo. **None of them is named
in the code.** They are covered because they all go through `get_mut`,
`values_mut`, `insert`, `remove` or `clear`, and there is no seventh way.

### What was left alone

`memory_world_stamp` walks the roster too, and folds `pop`, `hp`, `wall_hp`,
`buildings.len()` and every religious pressure. Those change on most turns, so a
memo behind them would miss more often than it hit, and it would need a second
invalidation rule for the fields the sight fold does not read. It keeps its loop.

## 2026-08-23 (follow-up) — the city sight-fold's two clocks disagreed in sign

The section above shipped as #2325 with a paired reading of **−1.09%** and a
warning that host load was 63–96. CI's own `paired-cost` job then read
**+1.01% overhead** on the same change. Opposite signs, and neither run is
allowed to be dismissed just because the other is nicer.

| harness | shape | reading |
| --- | --- | ---: |
| local, load 63–96 on 18 cores | 6p 74×46, 9CS, 250t, 8 seeds | −1.09% |
| `speed.yml`, shared 4-core runner | 6p 60×38, 6CS, 100t, 3 seeds | +1.01% |

Both said **"same game on every seed"**, so the correctness proof is doubled and
only the cost is in question.

### The plausible mechanism, and why it is wrong

The obvious explanation is shape. A 100-turn game is mostly the early game,
where cities are founded and borders grow every few turns, so the memo should
hit far less often than over 250 turns — and a rebuild folds *every* owner's
owned tiles where the old loop folded only the viewer's. A low hit rate at a
small shape would turn the win into a loss, and that would be a real finding.

It is not what happens. The same deterministic counter, run at both shapes:

| | 60×38 / 6CS / 100t | 74×46 / 9CS / 250t |
| --- | ---: | ---: |
| `vision_stamps()` asks | 117,299 | 452,140 |
| rebuilds | 3,035 | 10,254 |
| **hit rate** | **97.41%** | **97.73%** |
| hash operations | 5,113,713 → 973,235 (−81.0%) | 45,232,622 → 7,042,723 (−84.4%) |
| `Vec` allocations | 2,080,373 → **0** | 11,616,354 → **0** |

**The hit rate is shape-insensitive to within a third of a percentage point.**
The small shape removes proportionally almost as much work as the large one, and
2.08 M allocations at roughly 45 ns is about 0.09 s against that job's ~19
CPU-seconds per game — a predicted saving of about **0.5%**, not a 1% cost.

So the CI reading is host noise, which is exactly what `speed.yml`'s own comment
says to expect of it: *"A shared 4-core runner cannot resolve five percent and
this does not try to… It is a smoke alarm."* Three seeds at 100 turns on a
shared runner is a budget check, and quoting its percentage as a cost is reading
an instrument past its resolution.

### The rule this is an instance of

`docs/EVAL.md` says to compute the headline fact twice and read both. Both
clocks here were inside their own noise and they still disagreed in sign,
because ±1% is *below the resolution of every clock available on this fleet
under load*. The counter is not a second opinion about the same quantity — it
measures a different one (work removed, exactly, with no host in the answer),
and it is the only one of the three instruments that could settle the question.

⚠ For the next reader: at ±1% on a busy fleet, **stop planning to resolve it
with a clock.** Neither a quiet window on this Mac nor a rerun of `paired-cost`
would have separated −1.09% from +1.01% with any confidence. Instrument the
work instead, and quote the clock only for its report-digest verdict.

## 2026-08-23 — the gate measured the wrong game, on the wrong clock, and could not block anything

`speed.yml` shipped on 2026-08-22 (#2289) and closed the hole #2059 fell
through: nothing in CI could see a promoted feature multiply the fleet's
compute. It closed it with four measured gaps, all of them named in this file
already, and this section closes those.

| | #2289 | now |
| --- | --- | --- |
| shape | 6p **60x38, 6 city-states**, 100t, the CLI's default **`tennis_ball`** | 6p **74x46, 9 city-states**, 120t, **Continents** — `gene_screen`'s `SCREEN_*` map row |
| metric | whole-game user CPU | **user CPU per completed turn**, whole game reported beside it |
| statistic | pooled ratio of two totals | **median of five paired blocks**, with the spread and the run's own resolution printed |
| budget | +50% | **+8%**, and a second disjoint block before anything fails |
| absolute | none | `docs/speed_ledger.json`, recorded deliberately, per machine, with its load average |
| binding | advisory | **required** (`REQUIRED_CHECKS`), with `paired-cost: allow <reason>` as the escape hatch |

### 1. Nine city-states, because the minor seats are the bill

The section above this one measured **87% of `precise_evacuation`'s cost on
city-states and barbarians** — 29.1 of 33.3 points. Minor-seat cost scales with
the minor-seat count, and #2289's gate ran six of them against the screen's
nine on a map two thirds the area. It systematically under-weighted exactly the
cost that dominates every evaluation batch in the fleet. The map row is now the
screen's, leg for leg: 6 majors, 74x46, 9 city-states, Continents, Online.

⚠⚠ **The map script was the worst leg of the four, and it was invisible.**
`speed_ab.py` never passed `--map` at all, so every reading it has ever
produced was taken on `civvis simulate`'s default `tennis_ball` while the
screen plays Continents. Found by the envelope-representation work (#2324),
which measured **the same hotspot at 1.42% on tennis_ball and 13.19% on
continents**. The other three legs change how much of the code runs; the map
changes *which code is hot*, so the harness could have waved through a
regression in a dense-table path and refused to credit the fix for it, with
the number looking clean either way. `--map` is now a first-class leg of the
shape and defaults to the screen's, and
`test_speed_ab.TheWorkflowMeasuresTheScreensShape` reads `SCREEN_PLAYERS`,
`SCREEN_WIDTH`, `SCREEN_HEIGHT`, `SCREEN_CITY_STATES` and `SCREEN_MAP` out of
`src/bin/gene_screen.rs` and pins the workflow's arguments, the harness's own
defaults and the ledger's shape block to them. That test is the durable part;
`--map` is only today's instance of the divergence it prevents.

⚠ **The turn clock is the one leg still traded**, and the trade is
one-directional. Measured on `mbp-m5-max-128` (ci profile, quiet, four seeds):

| clock | s/turn | what it is |
| ---: | ---: | --- |
| 120 | 0.0878 | the gate |
| 150 | 0.1150 | |
| 250 (full games) | 0.1833 | the screen |

Cost per turn rises steeply with the turn number, so the gate reads about half
the screen's per-turn density and under-weights the late game, where a
per-unit pass hurts most. That is a runner budget, not a claim: the whole
paired step has to sit under `cargo-test`'s ~10.5 minutes or it becomes the
merge path for the entire fleet. Five games at 120 turns costs ~270 s on a
hosted runner; four at 150 costs ~345 s and would.

⚠ Changing `--turns` changes the **game**, not just how much of it is played:
seed 900002 wins a religious victory on turn 134 at a 150-turn clock and runs
to 250 at a 250-turn one. A short-clock run is a different game, not a prefix
of a long one.

⚠⚠ **So say it plainly, because "the gate measures the screen's shape" will
otherwise be read as stronger than it is — which is the exact failure this
whole section is about. THE GATE IS BLIND TO A REGRESSION THAT ONLY APPEARS
AFTER TURN 120.** Six of the seven legs match; the clock does not, and it is
the one leg whose difference is not proportional — turn 200 has bigger
empires, more units per seat, more cities, aircraft and a congress, so its
profile is not turn 100's scaled up. When a change plausibly touches late-game
code, `tools/speed_ab.py --turns 250 --games 4` by hand is the reading to
trust, and a green gate is not a substitute for it.

### 2. Per completed turn, which is what the section above asked for

The 2026-08-22 section ends: *"`gene_screen` already separates the two columns
for this reason; the paired harness does not, and a reader who quotes its
number for a behaviour change is quoting a mixture."* It does now. The turn
count comes from the game's own report — `standings()` prints exactly one of
`Winner: … on turn N`, `Draw: turn limit reached on turn N`, `No winner: turn N
of M` — and a report the harness cannot read is a hard error, never a default,
because a silently-guessed divisor would restore the mixture invisibly.

Whole-game CPU is still printed. When the turn totals match, the two are the
same number algebraically and the output says so; when they do not, the gap is
game length and the output says that instead, with both turn totals. For a
byte-identical optimisation — the four landing beside this section today — the
two agree exactly, and that agreement is itself the evidence the arms played
the same game.

### 3. The conditions are half the reading

Measured while twelve sibling agents built concurrently, one binary against
itself in both arms, so the honest answer is zero:

| shape | 1-min load | absolute s/turn | paired delta | pair IQR |
| --- | ---: | ---: | ---: | ---: |
| 5x120t (the gate) | ~6 (4 of 5 seeds) | 0.0878 | — | — |
| 5x120t (the gate) | 61 → 86 | 0.1436 (**+63%**) | +0.33% | 1.48pp |
| 3x150t | ~6 | 0.1134 | — | — |
| 3x150t | 49 | 0.1737 (**+53%**) | -0.56% | — |
| 3x150t | 94 | 0.1761 (**+55%**) | +0.11% | — |

And the same experiment on the gate's real host, from this change's own CI run
(`32623627991` — the pull request touches no Rust, so both arms built
byte-identical binaries and the run is nothing but runner noise):

| host | 1-min load | absolute s/turn | paired median | pair IQR | resolution |
| --- | ---: | ---: | ---: | ---: | ---: |
| `github-ubuntu-latest` | 3.77 | 0.2389 | +0.84% | **2.37pp** | ±1.57% |
| `mbp-m5-max-128` | 61 → 86 | 0.1436 | +0.33% | 1.48pp | ±0.52% |

⚠ **The hosted runner's pair-to-pair scatter is WIDER than this Mac's under a
twelve-agent load** — 2.37pp against 1.48pp — even though its load average is
3.77 and its absolute is the steadier of the two. Interleaving cancels a
desktop's slowly-varying contention very well; it does nothing about a cloud
VM's frequency and co-tenancy jitter, which lands as a burst inside a single
pair. So **the CI budget is calibrated on the runner row and not on the
desktop row**, and the two hosts keep separate rows in the ledger precisely so
nobody averages them.

+8% is **9.5x the noise median that run measured and 5.1x the resolution it
reported**, and a failure needs two *disjoint* seed blocks to agree. +50% was
not defended by a measurement in either direction; this is.

**The absolute inflates by more than half on a busy fleet host. The
interleaved paired delta does not move at all.** That single pair of facts is
what the rest of the design rests on:

- tightening the budget from +50% to +8% is defensible, because the quantity
  being judged survived a load average of 94 on eighteen cores inside ±0.6%;
- an absolute with no load average beside it is not comparable to anything, so
  every ledger row carries load at start, peak and end, and rows are per
  machine.

What interleaving cannot cancel is a **burst landing on one arm of one pair**.
So the gate statistic is the median of the per-pair per-turn deltas, not a
pooled ratio: four clean pairs and one at +150% leave the median at 0.00% and
move the pooled number past +25%. Beside it the run prints the interquartile
spread and the smallest change that spread could have resolved (two robust
standard errors of the median, taking the larger of the MAD- and IQR-based
sigma so the estimate errs toward *noisier*). When that resolution is wider
than the budget the line says so outright: a green verdict there is **"not
seen", not "not there"**.

### 4. The absolute ledger, because a relative gate cannot see drift

Every pull request is measured against the commit before it, so the fleet can
lose five percent a month and every single run reads green. `docs/census.json`
already solved this shape — record the reading, let the number move, make the
*diff* the signal — and `docs/speed_ledger.json` is that device for cost:

```
tools/speed_ab.py --baseline B --candidate B --record-ledger \
    --ledger-machine mbp-m5-max-128 --note "why this reading was taken"
```

Deliberate, never automatic, and never written by CI — a runner cannot commit.
A reading taken on a runner is transcribed with
`--ledger-cpu/--ledger-turns/--ledger-load`, which require the same `--note`.
The gate prints the trunk's absolute cost per turn on every run either way, so
the number is in the log even before it is in the file.

`tools/test_speed_ab.py` checks the ledger's shape block against the arguments
`speed.yml` actually passes, and both against `gene_screen.rs`'s `SCREEN_*`
constants read out of the source. That is deliberate: an artefact and its
source drifting apart is the defect that put `main` red the same morning this
landed — `17a27004` was pushed with no pull request and left the generated
`GENE_HEURISTIC_RANKING.md` out of step with its generator, failing six tests
every PR inherits (fixed by #2336). An absolute cost recorded at one shape
while the gate runs another is that defect with the drift line calling the
difference a regression.

**Host scaling**, kept in the ledger because it is exactly the constant that
gets re-derived badly later: hosted 4-core `ubuntu-latest` is **2.57x** this
Mac per single-thread core — 56.23 s user CPU for 3 games at the old shape on
run 32601426938, against 21.89 s for the identical command on
`mbp-m5-max-128` at load 5.8 the next day.

### 5. Required, and how to clear one

Advisory, the gate could not do the job it exists for. There is no branch
protection on this repository — `REQUIRED_CHECKS` in `tools/civvis_collab.py`
is the only thing that makes a check binding — and `ship` merges on that tuple
without reading anything else. #2059 would have merged again with a red
advisory X beside it.

Three properties make requiring it safe, and each is pinned by a test:

1. **It always reports.** No `paths:` filter and no job-level `if:`:
   `required_check_state` reads an absent required check as *pending* and a
   skipped one as a *failure*, so either would hang every docs-only PR in the
   fleet. The scope decision is the job's first step, which always succeeds and
   costs ~40 s when there is nothing to measure.
2. **One bad pair cannot fail a merge.** Median statistic, and over budget the
   harness re-measures on a *disjoint* block of seeds and fails only if both
   agree. An ordinary run never reaches that code.
3. **An intended cost can be accepted.** A promoted feature is a performance
   event by definition, so the gate will fire on honest changes:
   `paired-cost: allow <reason>` in the pull request body passes the run, the
   same escape hatch `overwrite-guard: allow` is. The reason is mandatory —
   that sentence, the number and why it is worth paying, is the one #2059 never
   wrote.

A false failure is cleared by re-running the job (`gh run rerun --failed
<run-id>`); `ship` already re-dispatches required checks that end without a
verdict.

**It costs the merge path nothing.** On this change's own pull request
`paired-cost` and `cargo-test` both started at 06:42:35 and finished at
06:53:11 and 06:52:55 — **sixteen seconds apart**. The paired step grew from
~112 s to 288 s and the job from ~7m32s to 10m36s, which is still inside
`cargo-test`'s 10-11 minutes, so the two finish together and nothing waits
longer than it did. That is the whole reason the turn clock is 120 and not
150: four games at 150 turns would have made this gate the trunk's critical
path for every merge in the fleet.

### A note on the other instrument the load moves

Measured the same evening: `python3 -m unittest discover -s tools` on plain
`origin/main` fails **25 tests and errors 6** at load 70 on this host —
`test_ladder_watchdog`, `test_spectator_supervisor`, `test_ops_portability`,
`test_conflict_hotspots`, every one of them timing-sensitive. The identical
suite is green on a hosted runner. That is the same fact this section is
about, applied to a different instrument: a result taken on a fleet host and
not labelled with its load is not a result. It is worth fixing on its own
terms, because a suite that is red on the machine agents actually work on
trains a fleet to stop reading it — the credibility problem `rust-quality`
already cost this repository once.

### What is still owed

The `mbp-m5-max-128` row in `docs/speed_ledger.json` was taken at load 61-86
and is marked PROVISIONAL in its own note. A quiet-host row for that machine,
and the first `github-ubuntu-latest` row, are both still to be recorded — the
file is designed so that adding them is one command and the old rows stay as
history rather than being overwritten.

## 2026-08-23 — the flood stopped re-asking the ruleset what the unit is (−9.9%/turn), and a dense frontier that did not pay

The direct sequel to 2026-08-22 above. That change hoisted the two *whole-board*
facts out of the movement flood — the air-patrol scan and the passage table.
The *per-unit* ones were still being asked once per neighbour of every tile
`flow_past` expands.

### The profile that found it, at the shape a screen actually runs

`/usr/bin/sample` at 1 ms over `civvis simulate --seed 7311001 --jobs 1
--players 6 --turns 250 --width 74 --height 46 --city-states 9 --speed online
--map continents` — the #2308 standard screen shape, not the 60×38/6 the
2026-08-22 profile was taken at. Load average 16 during the sample, so read the
shares, not any wall clock. Self time, over 32,860 non-idle samples:

| leaf | self |
| --- | ---: |
| `BTreeMap<(i64,i64), SetVal>::insert` — the `BTreeSet<Pos>` envelopes | 13.4% |
| `memcmp` | 3.4% |
| `memmove` | 3.2% |
| `BTreeMap<String, f64>::get` | 2.7% |
| `free` | 2.6% |

and, inclusive, where the flood's own work sits:

| | inclusive |
| --- | ---: |
| `BasicAi::enemy_attack_envelopes` | 16.9% |
| `attack_reach_from_flood` | 12.8% |
| `flow_past` | 11.0% |
| `BTreeMap<String, f64>::get` | 4.5% |
| **`units_at`** | **4.1%** |
| `can_enter_past` | 3.3% |
| `formation_enters_enemy_zoc` | 2.3% |
| `unit_step_cost` | 2.1% |

Attributing the two rows that matter to their callers:

| the work | caller | share |
| --- | --- | ---: |
| `units_at` | `unit_max_moves_at` → `adjacent_support_effect` | 1.71% |
| `units_at` | `in_enemy_zoc_for` ← `flow_past` | 1.00% |
| `units_at` | `can_enter_past` ← `flow_past` | 0.40% |
| `BTreeMap<String,f64>::get` | `class_can_traverse` ← **`routing_zones`** | 1.69% |
| `BTreeMap<String,f64>::get` | `unit_effect` ← `unit_step_cost` | 0.10%+ |

### What changed

**A per-unit movement profile in the existing `QueryMemo` scope.** `flow_past`
reaches `unit_step_cost`, `unit_max_moves_at` and `formation_enters_enemy_zoc`
once per neighbour of every tile it expands. Between them they asked, *per
neighbour*: `unit_effect("woods_move_cost")`, `unit_effect("hills_move_cost")`,
`promotion_effect("amphibious")`, `promotion_effect("movement")` — each a
`BTreeMap<String, f64>` descent with a `memcmp` per level — two
`dedication_active` string tests, and `adjacent_support_effect`, which sweeps
the occupancy of seven tiles to find a support aura. None of it depends on the
tile. It is now derived once per unit per memo scope, with the same
no-invalidation argument `traversal_class`, `air_patrols` and
`passage_improvements` already make: the guard borrows the world immutably.

**`units_at` reads the occupancy list in place** at the flood's two sites. This
file already named it — *"`units_at` … 1.08M allocations for 4.5 MB, an average
of four bytes, because it clones a one- or two-element `Vec<u32>` out of the
occupancy map"* — and `unit_ids_at` was already there, returning the slice.

### ⚠ The fix from 2026-08-22 was a pessimization on the path with no memo

The 1.69% row above is not the movement flood. It is `build_routing_zones`,
which maps `class_can_traverse` over **every tile on the map** to label
connected regions — and it runs with no `QueryMemo` scope open.

`Game::passage_improvements`, added by #2309, builds a `Vec<bool>` over the
whole improvement list and keeps it for the scope. Its own comment says that
outside a scope it is rebuilt per call, *"exactly the work the open-coded scan
did, never more"*. That sentence is true of `air_patrols` and false of this
one: the code it replaced was a single `improvements[name].effects["passage"]`
lookup, and rebuilding the table costs one such lookup **per improvement in the
ruleset**. On the one path with no scope, #2309 turned one string lookup per
improved tile into one per improvement.

Opening a memo scope over the sweep is the whole fix — one line, with the same
`&self` argument as everywhere else. **−0.98% measured on its own on a host at
load 73**, which the table below explains is a floor rather than a figure.

The general lesson outlives the line: **a lazily built table is only "never
more work" if its miss path is as cheap as what it replaced.** A memo whose
miss path builds a whole table must not be reachable without a scope.
`air_patrols` is safe because its miss path is the identical scan;
`passage_improvements` was not, and nothing said so.

### The measurement

`tools/speed_ab.py`, which pairs the arms seed by seed, flips their order
between seeds, and **hashes each game report — so a timing difference only
counts as overhead when both arms played the same game.** Every row below
reports *same game on every seed*, which is the correctness claim: the play is
bit-for-bit what it was, the engine simply stopped repeating itself.

Both arms at 6p **74×46, 9 city-states, 250 turns online, Continents** — the
#2308 standard screen shape, on the screen's own map — over eight paired games
on seeds 7311001..7311008, **both arms rebuilt at the merged base**:

    seeds 7311001..7311008 (8 games x 1 interleave(s) = 8 pairs),
    6p 74x46 9CS 250t online continents, --jobs 1
      load average 17.30 at start, 18.33 peak, 10.70 at end
      baseline    450.87s user CPU /  1787 turns = 0.252308 s/turn
      candidate   395.30s user CPU /  1787 turns = 0.221210 s/turn
      -9.94% per completed turn (median of 8 pairs)
          — same game on every seed, done faster
      spread: IQR 11.57pp over [-20.56%, -5.00%]; this run resolves ±6.07%
      pooled -12.33% per turn; whole game -12.33% over the same 1787 turns

**That is the row to quote: −9.94% median, −12.33% pooled**, both arms built
from the base this lands on, at the shape *and the map* a screen actually runs.
The two metrics agreeing exactly is not a coincidence — they must agree when
both arms play the same game, and the harness prints it as a self-check.

⚠ The median's own resolution on this run is ±6.07%, so **−9.94% is a sign and
an order of magnitude, not three significant figures.** The pooled figure is
the tighter of the two here because the arms played identical games.

The earlier readings from this session are kept below because they say
something the headline cannot. All were taken with the pre-#2328 harness, which
never passed `--map` and so measured `tennis_ball`, and they report whole-game
user CPU rather than per turn:

| arm (old harness, `tennis_ball`, whole game) | host load | baseline | candidate | |
| --- | ---: | ---: | ---: | ---: |
| both changes | ~25 | 407.20s | 369.94s | −9.15% |
| per-unit movement profile + in-place occupancy | ~25 | 418.25s | 395.01s | −5.56% |
| per-unit movement profile + in-place occupancy | ~78 | 432.11s | 412.39s | −4.56% |
| memo scope over the connectivity sweep, on top of it | ~73 | 401.80s | 397.87s | −0.98% |
| dense frontier scratch for `flow_past` | ~72 | 399.95s | 403.19s | **+0.81% — rejected** |

⚠⚠ **Those rows do not sum, and that is the reading to take from them, not an
error in them.** The same change — the movement profile, an unchanged binary,
the same seeds and shape — reads −4.56% at load 78 and −5.56% at load 25. A
machine that is already memory- and scheduler-bound returns less of a CPU-work
saving, so **a loaded host understates an optimization**, and a per-part row
taken at load 73 is a floor rather than that part's figure at rest. The split
between the two shipped changes is therefore not known to better than "both are
real and the profile is the larger"; only the total is quoted as a result.

⚠ A dozen agents share this machine. The pairing and the alternating run order
are what make any of these worth quoting at all; a single-arm reading at load 78
would mean nothing. **The digests are not provisional** — 40 paired games across
five windows, two harnesses and two maps, every one of them *same game on every
seed*. That is the claim this change actually rests on.

### Rejected: a dense frontier for `flow_past` (+0.81%)

`flow_past`'s `best` was a `BTreeMap<Pos, f64>` keyed on a hex coordinate,
relaxed once per neighbour of every tile — the classic case for the dense
per-slot scratch `first_route_step` already uses, indexed by
`TileGrid::index_of`. `TileGrid` keeps its tiles sorted by `Pos`, so the
reached set read back in index order reproduces the tree's iteration order
exactly, and the four callers that turn it straight into a result see no
change: *same game on every seed*, all eight.

**It measured +0.81%, and was reverted — but the clock is not what decided
it.** At ±1% on this fleet no available clock resolves a sign, so the verdict
is a counter. Instrumented over one whole standard-shape game (seed 7311001, 6p
74×46, 9 city-states, 250 turns), counting every `flow_past` call, every
relaxation probe, and the size of every reached set:

| | |
| --- | ---: |
| `flow_past` calls in one game | **250,000** |
| relaxation probes | 9,848,274 — **39.4 per flood** |
| tiles reached | 3,364,457 — **13.5 per flood** |

| reached set | share of floods |
| --- | ---: |
| 1 tile | 0.2% |
| 2–5 | 6.3% |
| **6–11** | **52.2%** |
| 12–23 | 30.2% |
| 24–50 | 9.7% |
| 51–132 | 1.3% |
| 133+ | 0.03% |

**Rust's B-tree holds 11 keys in a node, so 58.7% of these floods are a
`BTreeMap` of exactly one node** — a flat, cache-resident array — and 98.7% are
at most two levels. There is no tree descent to remove. What the dense version
adds is real: a `Vec<f64>` sized to all 3,404 tiles (27 KB, written at ~13
scattered slots) in place of one hot node, plus a sort to rebuild the ordering
the tree supplied for free. Even if all 9.8M probes were made free, at a
plausible 20 ns each that is 0.2 s of a ~50 s game — an **0.4% ceiling** against
a cache cost that is certain.

That is this file's ninth null of the same shape, and the second in this exact
spot: *"Route-search scratch reuse: rejected"* above measured the same idea 25%
slower in 2026-07 and diagnosed it identically — *"it replaces cheap dense
initialization with an extra per-tile generation array … for this map size,
that additional cache traffic loses to the allocator."* The standing rule it
produced — *the payer is an expensive derivation that is recomputed, not a
cheap operation that is frequent* — predicted this one before it was written.

The three changes that *did* pay in this area — #2309's two and the profile
above — all removed a derivation that was being recomputed. Nothing that merely
made frequent cheap work cheaper has ever paid here. The count is now nine
nulls against five wins and it has still never gone the other way.

⚠ The next target is unchanged from 2026-08-22 and is now **13.4% of self
time**, up from 9.0%: `AttackEnvelopes = Vec<(u32, Arc<BTreeSet<Pos>>)>`, whose
`BTreeMap<(i64,i64), SetVal>::insert` is the largest single leaf in the
profile. That set is queried with `contains` and iterated in sorted order, so a
sorted `Vec<Pos>` with `binary_search` gives identical answers and identical
order with no node allocation. It touches ~40 sites and deserves its own
change. Note that this is the *opposite* shape to the rejection above and is
not evidence against it: those sets hold hundreds of positions and are built
once and read many times, where the flood's frontier holds tens and is thrown
away.

## 2026-08-26 — a third of every profile in this file had no symbol

Every ranked hotspot list above was drawn from about two thirds of the running
samples. The other third is reported by `/usr/bin/sample` as
`<deduplicated_symbol>` — identical machine code folded to one address, one
name kept — and it is the **largest single entry in the profile**, larger than
any named leaf and larger than the last four shipped optimizations put
together. Nothing in this file has ever mentioned it.

This entry does two things: it closes the instrument gap, and it re-profiles
`4b26eafb` (#2563) with the gap closed. `tools/profile_civvis.py` is the
harness; `tools/test_profile_civvis.py` pins the parsing on every pull request.

### The flag does not work, and that is the finding

The obvious fix is to stop the linker folding. It was tried five ways, one game
each, seed 7311001 at the screen's shape, `ci` profile, folded share of running
samples:

| build | folded |
| --- | ---: |
| the `ci` profile as it ships | **32.54%** |
| `-C link-arg=-Wl,-no_deduplicate` | 31.10% |
| `-Wl,-no_deduplicate` + `-C strip=none` | 32.59% |
| `CARGO_PROFILE_CI_STRIP=none` | 31.59% |
| `CARGO_PROFILE_CI_DEBUG=1` + `CARGO_PROFILE_CI_STRIP=none` | 30.35% |

`-no_deduplicate` is a real option and reaches the link — a deliberately
invented `-Wl,-no_such_option_at_all` fails with `ld: unknown options:` and
this one does not — and it moves the number by noise. Keeping debug info
quadruples the symbol table, 35,810 entries to 114,047, and still leaves 30%.
Whatever merges these bodies is upstream of the linker, and
`-Z merge-functions=disabled` is nightly-only while the fleet is on stable.

⚠ **Do not spend another afternoon on the flag.** The table above is why it is
in this file.

### The answer is a caller, not a name

A folded leaf has lost its own symbol and kept every one of its parents, and
the parent is named. That is the whole fix, and it is what a reader wanted
anyway — *which subsystem is this?*, not *which monomorphization?*.
`profile_civvis.py` credits every placeholder to its nearest **named** ancestor
and prints the section on every run. Seed 7311001, share of the busiest thread:

| the unnamed third | share |
| --- | ---: |
| `tile_has_visibility_line` ← `visible_tiles_from` | 2.67% |
| `in_enemy_zoc_for` ← `formation_enters_enemy_zoc` | 2.17% |
| **`Vec::clone` ← `Game::clone`** | **1.43%** |
| `can_enter_past` ← `flow_past` | 1.21% |
| `spec_from_iter` ← `can_enter_past` | 1.15% |
| `healing_location` ← `safe_healing_step` | 0.98% |
| `build_reverse_flow_field` ← `route_step_to_any` | 0.76% |
| **`Vec::clone` ← `speculative_clone`** | **0.76%** |
| `defensible_district_owner_at` ← `in_enemy_zoc_for` | 0.75% |
| `units_at` ← `unit_max_moves_at`'s `f64::max` fold | 0.69% |
| … 37.15% attributed in total | |

Sight, the movement flood, and **whole-`Game` cloning**. The first two are
already ranked in this file. The third is new: 2.19% of the thread is `Vec`
copies made by `Game::clone` and `speculative_clone`, and no profile in this
document has ever shown it, because it was always inside the placeholder.

### The re-profile, with the third put back

`tools/profile_civvis.py --seed 7311001 --turns 250`, `4b26eafb`, load ~3-7 on
18 cores, 15,770 samples on the busiest thread. Inclusive, entry frames above
99% dropped:

| | inclusive |
| --- | ---: |
| `AdvancedAi::take_turn_inner` | 96.6% |
| `advanced_units` | 53.1% |
| `advanced_military_step_with_decline` | 46.1% |
| `BasicAi::healing_step` | 37.7% |
| `plan_general_unit_turn` | 33.7% |
| `BasicAi::retreat_step` | 31.0% |
| **`BasicAi::take_turn_inner` — the minor seats** | **28.3%** |
| `BasicAi::military_step` | 24.8% |
| `enemy_attack_envelopes` | 21.9% |
| `attack_reach_from_flood` | 14.9% |
| **`forcing_reply_penalty_owned`** | **14.3%** |
| `Game::apply` | 13.2% |
| `flow_past` | 10.4% |
| vision (`vision_frame` 8.7, `player_vision` 7.9) | ~9% |

and the roll-ups, as a share of running samples: **allocator and libc memory
primitives 12.9%**, unnamed-but-now-attributed 32.5%.

### What this re-ranks

1. **`BasicAi::take_turn_inner` at 28.3% is the city-states and barbarians.**
   `advanced.rs` hands a minor or barbarian seat straight to `self.base` and
   returns, so that row is entirely minor-seat deliberation, and 19 points of
   it are `military_step → healing_step`. The 2026-08-22 entry above measured
   87% of `precise_evacuation`'s cost on those seats and −29.1% per turn from
   withholding it there; this is the same finding from the other direction,
   four days and 206 merges later, and **it is still `true` in both
   `BasicAi::new()` and `BasicAi::with_weights()`**. It remains unreachable by
   the gene screen, which varies major genomes only.
2. **`forcing_reply_penalty_owned` at 14.3% has no gene row, no evaluator arm
   and no depth constant.** It clones a whole `Game` per candidate through
   `speculative_clone`, and `Game::apply` at 13.2% and the 2.19% of `Vec`
   copies above are largely it. That is precisely the shape of `#2059`: a
   default nobody can price because it is not an arm. The remedy is the same
   one that was applied to `precise_evacuation` — register it, then measure —
   and it should happen before anything tries to optimize it.
3. **The envelope reach does not depend on who is asking.**
   `compute_enemy_attack_envelopes(g, pid)` uses `pid` only to *filter* which
   units are hostile and visible; the expensive part,
   `g.attack_reach_from_flood(unit.id)`, takes the enemy's id and the board and
   nothing else. `enemy_envelope_cache` is a field of `BasicAi`, so with six
   majors, nine city-states and a barbarian seat, **the same enemy's flow field
   is computed up to sixteen times per board**. Hoisting that memo to `Game`
   beside `vision_frames` — which already carries a content-stamped cache
   across `speculative_clone` and is documented exact — would be
   answer-identical by construction. Unmeasured; the counter that would size it
   is one probe on the call site.
4. `Sphere::distance` is 3.31% of running self time and `slice::binary_search`
   1.61%, both of which are the ring lookup inside `wdist`. `can_enter_past`
   still re-asks `wdist(from, pos) != 1` for a neighbour its caller just
   enumerated (rank 5 of the 2026-08-21 list, never taken).

## 2026-08-26 (later) — the tactical picker cloned the board twice and applied the same action to both

The entry above this one attributes the profile's unnamed third by caller, and
two of the lines it produced had never appeared in a profile in this file:

    1.43%  Vec::clone  <-  Game::clone
    0.76%  Vec::clone  <-  speculative_clone

That is the tactical evaluator, and reading it led straight to the cause.
`plan_tactical_orders` scored every candidate action like this:

```rust
let inputs = candidates.iter()
    .map(|(_, action)| (g.speculative_clone(), g.speculative_clone(), action.clone()))
    .collect();
pool.map_owned(inputs, move |(attack, reply, action)| {
    let attack = Self::tactical_attack_result_owned(attack, pid, uid, &action, &plan);
    (attack.value, attack.eliminates_enemy_unit,
     Self::forcing_reply_penalty_owned(pool, reply, pid, uid, &action))
})
```

**Two whole-`Game` clones per candidate, and both of them apply the same
action.** `tactical_attack_result_owned` calls `after.apply(pid, action)` to
price the attack; `forcing_reply_penalty_owned`'s first statement is
`after.apply(pid, action)` to price the enemy's answer to it. The engine is
deterministic, so the second board was bit-for-bit the first. The serial arm
did the same thing one line further down.

### The fix, and why it is exact

`Game::apply` is the **only** mutation `tactical_attack_result_owned` makes —
everything else it touches is a read (`units_at`, `city_at`, `encampment_at`,
field reads for the before-snapshot) — so the board it finishes with *is* the
board the reply search was rebuilding. It now takes `&mut Game` and reports
what happened to it:

```rust
enum AppliedAttack { Applied, Refused(String), NotScored }
fn tactical_attack_result_in(after: &mut Game, ..) -> (ExactAttackResult, AppliedAttack)
fn forcing_reply_penalty_applied(pool, after: &mut Game, applied, pid, uid, action) -> f64
```

The three arms reproduce the old function case for case: `Refused` carries the
reason `note_illegal_attack` records and returns 1,000.0; `NotScored` — the
action was not this unit's attack, so nothing was applied and nothing was
scored — applies it here, which is the one arm that still pays an apply and is
exactly the arm that used to pay a clone as well; `Applied` goes straight to
`forcing_reply_penalty_from_position`. The 135.0 charged to an attacker that
did not survive its own attack is unchanged and is now charged in one place.

⚠ `forcing_reply_penalty` — the `&Game` entry point eight tests use — stays,
and now routes through `AppliedAttack::NotScored`, so those tests exercise the
new path rather than a retired one.

### The measurement

**The clock could not read it, and the counter could.**

Six paired games, seeds 7311001..7311006, 6p 74x46 9 city-states 250 turns
online Continents, both arms built in one worktree from one base:

    load average 62.64 at start, 62.64 peak, 24.77 at end
    baseline    295.03s user CPU /  1335 turns = 0.220999 s/turn
    candidate   317.53s user CPU /  1335 turns = 0.237847 s/turn
    +7.12% per completed turn (median of 6 pairs)
        — same game on every seed
    spread: IQR 20.82pp over [-12.22%, +24.95%]; this run resolves +/-12.60%

⚠⚠ **THAT +7.12% IS NOT A READING, AND THE RUN SAYS SO ITSELF.** Its own
resolution is +/-12.60% — the verdict is *not seen*, not *not there* — and the
load average was 62 because a `cargo test` of this very branch was running
beside it. The 2026-08-23 entry above already wrote the rule this breaks: at
this scale on a busy fleet, **stop planning to resolve it with a clock;
instrument the work instead, and quote the clock only for its report-digest
verdict.** That verdict is the half worth keeping and it does not care about
load: *same game on every seed*, six paired 250-turn games. The change is exact.

So the result is a counter, taken with `AtomicU64`s on `speculative_clone` and
`Game::apply` over one whole standard-shape game (seed 7311001):

| per game | baseline | candidate | |
| --- | ---: | ---: | ---: |
| `speculative_clone` calls | 36,533 | 33,142 | **-3,391 (-9.28%)** |
| `Game::apply` calls | 144,027 | 140,636 | **-3,391 (-2.35%)** |

**Exactly 3,391 of each**, which is the change's own arithmetic reported back:
one clone and one apply per tactical candidate, and the tactical picker scores
3,391 candidates in a 250-turn game. Nine percent of every speculative clone in
the simulator was a second copy of a board that already existed.

⚠ **Do not read that as a nine-percent speed-up.** The profile above puts
`Vec::clone` under `Game::clone` and `speculative_clone` together at 2.19% of
the thread and `Game::apply` at 13.2% inclusive, so removing 9.28% of the
clones and 2.35% of the applies predicts a few tenths of a percent — real,
permanent, and **below what any clock on this fleet resolves**. The honest
claim is the counter and the digest, not a percentage.

### What this does not do

It halves the clones in the tactical picker; it does not touch the reply
search's own cost. `forcing_reply_penalty_owned` was 14.3% of the main thread
inclusive in the profile above, and `forcing_reply_line` recurses and clones a
`Game` per line inside that. **That machinery still has no gene row, no
evaluator arm and no depth constant** — the shape of #2059, named in the entry
above and unchanged by this. Registering it is blocked today rather than
undone: a `Kind::Production` row would make `ledger_default_on` answer
`Some(false)` and switch a shipped behaviour off at deployment (the trap
`genes.rs` documents for the other six), so the row and either a measurement or
an operator pin have to land together — and `tools/genes.py`,
`docs/gene_ledger.json` and the ranking are being regenerated by two open pull
requests as this is written.

## 2026-08-26 — the minor-seat evacuation saving does not reproduce, and per-completed-turn is confounded when a treatment changes game length

The 2026-08-22 entry above is the origin of a number that has steered this
file's priorities ever since: withholding `precise_evacuation` from city-states
and barbarians measured **−29.1% per completed turn**, and 87% of the feature's
cost was therefore on seats no evaluation varies. The entry two sections up
re-ranks the minor seats first on the strength of it.

**It does not reproduce.** Re-measured on `def819677`, `tools/speed_ab.py`,
four paired 250-turn games at the screen's own shape, the treatment applied at
the one line that decides it — `self.base.precise_evacuation = false` inside
`AdvancedAi::take_turn_inner`'s `minor || barb` branch — with both binaries
built in one worktree from one base and verified to differ:

```
load average 11.05 at start, 13.81 peak, 5.75 at end
baseline    167.39s user CPU /  835 turns = 0.200468 s/turn
candidate   191.42s user CPU /  900 turns = 0.212691 s/turn
-6.41% per completed turn (median of 4 pairs)
   — the reports differ on all 4 seeds, so this is what the changed behaviour
     costs, not an overhead measurement
spread: IQR 82.55pp over [-35.58%, +73.47%]; this run resolves +/-61.19%
pooled +6.10% per turn; whole game +14.36%
```

**The median and the pooled figure have opposite signs**, and the run's own
resolution is ±61.19%. At four pairs of a treatment that changes the game, this
measures nothing. It certainly does not measure −29.1%.

### ⚠⚠ The correction that outlives the number: dividing by turns does not remove a length confound

The 2026-08-22 entry's method paragraph says it measured "four seeds interleaved
on one revision, **dividing by turns so game length cannot confound it**". That
sentence is false in exactly the case it was written for, and this file already
contains the evidence:

| clock | s/turn |
| ---: | ---: |
| 120 | 0.0878 |
| 150 | 0.1150 |
| 250 | 0.1833 |

**Cost per turn rises steeply with the turn NUMBER.** A treatment that makes
games longer therefore shifts the *mix* of turns toward later, more expensive
ones — the withheld arm above played 900 turns against 835, +7.78% — and the
per-turn average moves with the mix even if the per-turn cost of every
individual turn is unchanged. Dividing by turns removes the *count*, not the
*composition*.

So the rule needs its fourth statement:

> **Per completed turn is confound-free only when both arms complete the same
> number of turns.** For a byte-identical optimisation they do, by construction,
> and the metric is exact. For a behaviour change they do not, and per-turn is
> biased in the direction of whichever arm plays longer. That is a *third*
> way this file has now been wrong about the same feature: whole-game time was
> a mixture (2026-08-22 retracting 2026-08-21), and per-turn is a mixture too.

The instrument that answers it is a counter, not a clock — the same conclusion
the 2026-08-23 entry reached at ±1%, arrived at here from the other end.

### What survives, and it is the part worth acting on

The *attribution* is unaffected, because it comes from the call graph rather
than from a clock:

- `BasicAi::take_turn_inner` is **28.3% of the main thread inclusive**, and
  under `AdvancedAi::take_turn_inner`'s `minor || barb` branch that row is
  entirely city-state and barbarian deliberation.
- **19 of those points are `military_step → healing_step`**, the branch
  `precise_evacuation` gates through `retreat_step`.

And a second, independent instrument agrees with it to within a third of a
percentage point. An `AtomicU64` on `BasicAi::compute_enemy_attack_envelopes`,
split on `self.minor || self.barb`, over one whole standard-shape game (seed
7311001, 6p 74×46, 9 city-states, 250 turns, Continents):

| per game | |
| --- | ---: |
| envelope computations for **city-states and barbarians** | **11,310** |
| envelope computations for the six majors | 28,244 |
| **minor-seat share** | **28.6%** |
| `attack_reach_from_flood` calls, all seats | 254,415 |

**28.6% from the counter against 28.3% from the call graph**, measured
different ways on different runs, and neither depends on the host or on how
long the game ran. *That* is what the minor seats cost.

So roughly a fifth of the simulator is minor-seat evacuation machinery, and
that number is solid. What is *not* established — and was believed on the
strength of the retracted figure — is that switching it off recovers any of it.
It changes what city-states and barbarians do, which changes the game the majors
play, which changes how long the game runs and how expensive its turns are.

**For the next reader:** price it with a counter that attributes work to seat
class (`compute_enemy_attack_envelopes` calls split on `BasicAi::minor`/`barb`
is the obvious one), and treat any clock reading of a length-changing treatment
as a mixture until both arms are shown to complete the same number of turns.

### Addendum 2026-08-27: a sound geometric prefilter was still the wrong saving

One exact candidate was built and deliberately **not shipped**. Before
`retreat_step` constructed its hostile-envelope table, it checked whether a
visible hostile could geometrically reach the current tile next turn. The
bound used the ruleset's 0.25-MP route floor only when every terrain cost was
positive and every feature cost non-negative; a custom ruleset that could make
a step free fell straight through to the existing full calculation. City and
Encampment strikes stayed exact, air units fell through because their attack
disk is centred on an operation origin, and every in-range unit still used the
old terrain-accurate envelope. The helper's negative answer was therefore a
proof, not a heuristic.

The proof did not pay for its source-roster scan. Applied to every seat it
read **+3.56% per completed turn** over two 150-turn paired games (reports
identical; resolution ±2.82%). Scoped to the city-state/barbarian seats this
section attributes the work to, it read **+0.64%**, inside that run's ±1.0%
noise floor; again every paired report was identical. The deciding instrument
was a temporary exact trace on seed 7,320,620 at 6p, 74×46, 9 city-states,
Online Continents, 150 turns: the gate avoided only **27** calls to
`enemy_attack_envelopes`. Each would have been one saved table fetch, but the
gate scanned potential sources on every minor-seat retreat and could not repay
that fixed cost.

Do not retry this as a radius tweak. A future version needs a maintained,
cheap local-threat index; without one, the conservative source scan is the
work. The behavioral `precise_evacuation` switch remains a different question
and is still subject to the length-confound warning above.

### ⚠ Addendum, same day: the +7.12% two sections up was load, and the counter was right

The entry on the tactical picker's double clone reports **+7.12% per completed
turn at load 62** with its own resolution at ±12.60%, calls it *not seen rather
than not there*, and rests its claim on a counter instead — 3,391 clones and
3,391 applies removed per game, predicting "a few tenths of a percent".

Re-measured once the fleet went quiet, on **eight fresh seeds** so nothing is
reused from the first run:

```
seeds 7311011..7311018 (8 pairs), 6p 74x46 9CS 250t online continents
load average 2.84 at start, 10.07 peak, 3.77 at end
baseline    306.97s user CPU /  1757 turns = 0.174711 s/turn
candidate   308.24s user CPU /  1757 turns = 0.175436 s/turn
-0.50% per completed turn (median of 8 pairs) — same game on every seed
pooled +0.42% per turn; this run resolves +/-5.71%
```

**Median −0.50%, pooled +0.42%, both inside half a percent of zero and inside
the run's own resolution.** That is the counter's prediction, met. The +7.12%
was the host, and the honest reading of both runs together is: the change is
exact, it removes 9.28% of the simulator's speculative clones, and its effect on
the clock is smaller than this fleet can measure.

Two things worth keeping from the pair of readings:

- **The absolute moved by more than the effect being hunted.** The same
  baseline binary at the same shape read 0.220999 s/turn at load 62 and
  **0.174711 s/turn at load 2.84** — a 21% swing from the host alone, on a
  quantity a third of a percent was being sought in. `docs/speed_ledger.json`'s
  conditions block says exactly this; here it is again, from a change whose
  true size sits under the noise.
- **The exactness claim never wavered.** *Same game on every seed* on both
  runs, fourteen paired 250-turn games in total, at load 62 and at load 2.84.
  A report digest does not care what else the machine is doing, which is why it
  is the half of a paired run worth quoting when the timing is not.


## 2026-08-27 — purchase-price memo (scoped to one `QueryMemo` guard) and a non-allocating production block key

`legal_purchase_actions_for_city` asks `unit_purchase_cost_for_formation`
once per unit kind, formation (0/1/2) and currency (gold/faith) — six full
price derivations per unit kind per city per ask, each one walking policies,
buildings, districts, era, game speed and Great People/religion discounts.
`legal_actions_within`'s purchase family repeats the identical sweep, and
several AI call sites (`ai.rs`, `ai/advanced.rs`, `gold_and_cards.rs`,
`religion.rs`) ask the same price again elsewhere in the same turn's
decision-making.

### ⚠ The first design was wrong, and a pre-existing test caught it before it shipped

The first cut mirrored `producible_items`: a cache keyed on
`(pid, cid, unit, formation, currency)` that outlives `QueryMemo`, cleared
only at explicit sites (a successful `Game::apply`, the mirror-input
`replace_*` calls). `district_building_wonder_runtime_tests::
land_combat_purchase_requires_an_unreserved_city_center_combat_layer` failed
against it: `unit_purchase_cost_for_formation` reads live unit occupancy
through `land_combat_purchase_slot_open` (`self.units_at(city.pos)`) and the
production queue head (`city.queue.first()`), and that test edits the queue
and calls `relocate` directly — both `pub(crate)`, neither going through
`Game::apply` — between two otherwise-identical asks, expecting the second to
differ. The persistent design served the first answer back stale.

**The fix scopes the cache like `Game::tile_appeal`'s `appeal` field instead**
(`QueryCache::purchase_price`, now `RefCell<Option<BTreeMap<...>>>`): armed
only while a `QueryMemo` guard is open, cleared on that guard's outermost
`Drop`, exactly like `appeal`, `movement`, `unit_ids` and the rest of that
family. Outside any guard — a bare call, which is what the caught test and
any direct-mutation caller do — the field stays `None` and every ask derives
fresh, byte-for-byte the function's behaviour before this cache existed. That
also makes the fix airtight against the exact bug that broke the first
design: a `QueryMemo` guard holds `&Game`, so `Game::apply` (`&mut self`)
cannot be called while one is alive, and `relocate`/a queue edit made outside
any guard never had a stale answer to leave behind in the first place.

The benefit this design keeps is real but narrower than the persistent one:
it removes duplication only between calls that share one already-open guard
(nested `query_memo()` calls inside one decision), not across separate
top-level calls. `legal_purchase_actions_for_city` itself has no internal
duplication to remove — each `(unit, formation, currency)` combination is
asked exactly once per city by construction — so the saving is entirely
cross-call-site, and how much of it survives the narrower scope depends on
how much of the AI's purchase-pricing work already runs under one shared
guard. The scratch `AtomicU64` measurement taken against the first (buggy)
design read 4,375,259 asks / 2,993,698 derivations over one 150-turn game;
that number is **retracted** along with the design it measured; a fresh
measurement against the guard-scoped version is the next reader's first
follow-up here, not yet re-taken under fleet time pressure.

### The second half, unaffected by the above: a non-allocating production block key

`production_block_key` built a fresh `String` (`format!("formation:{unit}:{formation}")`
and five siblings) for every candidate item at several call sites, including
unconditionally at the top of `can_produce` and once per queued item in its
duplicate-scan. `ProductionKey` is a `Copy` enum over `Name` (already an
interned `u32`) standing in wherever a key is only compared or hashed;
`production_block_key` becomes a thin `.to_block_string()` wrapper kept at the
three external boundaries that still need a `String` — `blocked_production`,
`blocked_purchases`, `host_buildable`/`host_purchasable` — because those maps
cross the live mirror's serde boundary (`mirror.rs` calls
`Game::production_block_key` directly and was left untouched). `can_produce`
now only builds that `String` when `blocked_production`/`host_buildable`
actually holds an entry for the city — empty on an ordinary, non-mirrored
board — and its queue duplicate-scan compares `ProductionKey` values directly
instead of formatting one `String` per queued item. This half introduces no
caching and carries none of the risk above: it changes only how a key is
represented, never when or whether one is computed fresh.

### Exactness

`purchase_price_memo_tests.rs` covers: a warm ask matching a cold ask under
one shared guard, across a three-city fixture, with every memoized answer
also checked against an uncached re-derivation for every unit/formation/
currency/city; the memo field never arming outside a guard (the shape of the
bug above, pinned directly); and a host-priced answer reflecting a later
purchase refusal immediately. `tools/speed_ab.py` and
`advanced_v1_plays_the_same_game_it_always_did` both agree on report digests
before and after — see the PR body for the exact command and hashes. No
action list changed order or content.
## 2026-08-27: per-ask vision allocations paid for answers that had not moved

Vision sat at roughly 9% of the main thread, and a chunk of that share was
small, high-frequency allocations that answered no differently from a cached
borrow. Two exact changes, both in the single-seat sight-frame path
(`Game::player_vision_frame` / `Game::vision_frame`):

1. **16 non-test call sites cloned a whole `TileBits` for a read-only
   membership check.** `player_vision_now` returns an owned `TileBits`,
   cloned out of the engine's own `Arc<TileBits>` cache
   (`player_vision_frame`). Every production call site only ever read the
   bits afterwards — `Game::sees`, `ranged_order_is_legal`,
   `combat_target_visible_at`, and the rest — so all 16 switch to
   `player_vision_frame`'s `Arc`. `AdvancedAi::BattlefrontFrame` followed the
   same change: it stored a fresh clone of the turn-start frame every turn,
   and now stores the `Arc` instead. `player_vision_now` itself stays,
   `#[cfg_attr(not(test), allow(dead_code))]`, for the handful of test call
   sites that want an owned snapshot on purpose.

2. **`vision_input_stamp_with_suzerains` rebuilt its own inputs on every
   ask, cache hit or not.** Deciding whether the *cached* sight frame was
   still valid required building a fresh `BTreeMap<minor, suzerain>`
   (`suzerain_input_map`) and a fresh `BTreeSet<viewer>`
   (`visibility_viewers`) on every single call — including a repeat ask on a
   board that had not moved at all. Both are now memoized behind a new
   `Game::diplomacy_epoch`, following the same discipline `map_geometry` and
   `unit_stamps` already use above it in `VisionFrameCache`: `Players` gets
   a `Units`-style mutation epoch (bumped in the three routes that reach
   `&mut Player` — `get_mut`, `IndexMut::index_mut`, `iter_mut` — plus
   `push`; the wrapped `Vec` is private, so that is the whole surface).
   `Cities` gets an analogous `generation` counter bumped inside its
   existing `invalidate()`, riding the same eager, exhaustive-by-construction
   guarantee `city_roster`'s sight fold already documents. `active_emergencies`
   is different: it is a plain `pub Vec<Emergency>` a couple of engine paths
   and several tests write in place, not a closed type like the two above, so
   a bumped counter at its two real mutation sites (declare, resolve) could
   silently miss a direct write — an early version of this change did exactly
   that and a test that warmed the cache before pushing straight onto
   `active_emergencies` would have gone stale. `diplomacy_epoch` folds its
   live content instead (`ends`, `members` — the two fields
   `visibility_viewers` reads), which cannot be bypassed and costs nothing
   next to the allocation it is guarding, since the vector holds at most a
   handful of live emergencies.

### Measured

Temporary `AtomicU64` counters on `suzerain_input_map`, `visibility_viewers`,
and the `TileBits` clone inside `player_vision_now` (not committed — added to
a scratch clone of the pre-change commit for "before", and to this branch for
"after"), one 150-turn game, `--jobs 1` (seed 7320000, 6p 74×46, 9 city-states,
online, continents):

| per game | before | after | Δ |
| --- | ---: | ---: | ---: |
| `suzerain_input_map` calls | 69,022 | 26,944 | **-61.0%** |
| `visibility_viewers` calls | 101,585 | 83,435 | **-17.9%** |
| `TileBits` clones (`player_vision_now`) | 19,443 | 0 | **-100%** |

The remaining `visibility_viewers` calls are the roughly-dozen direct
(uncached) call sites elsewhere in `game.rs` this change deliberately left
alone — see "What was left out" below; `suzerain_input_map` has no such
direct callers left, so its whole remaining count is genuine diplomacy-epoch
misses (a suzerainty, envoy, city, or turn change) rather than redundant
re-asks.

### Exactness

`tools/speed_ab.py` (seeds 7320000-7320001, 2 paired 150-turn games, same
shape as the counters above) reports the same game on every seed — identical
report digests, baseline against candidate — and
`advanced_v1_plays_the_same_game_it_always_did` passes. A new test,
`diplomacy_caches_agree_with_an_uncached_derivation_across_every_input` in
`src/game/visibility_tests.rs`, asserts the memoized suzerain map and viewer
set equal a from-scratch derivation across a suzerainty change, an envoy
change that does not flip it, a unit move, and a spy move — and separately
that the last two correctly leave `diplomacy_epoch` untouched, since neither
`suzerain_input_map` nor `visibility_viewers` reads a unit or a spy (the spy
still moves the *overall* stamp, folded fresh on every ask by
`base_vision_input_stamp`, entirely outside this cache).

### What was left out

- The larger per-tile viewer-count redesign this task named was explicitly
  out of scope.
- `battlefront_visibility`'s roughly two dozen other call sites in
  `ai/advanced.rs` needed no edits beyond the `Arc<TileBits>` return-type
  change: the type flows through `let visible = self.battlefront_visibility(...)`
  by inference, since every one of them only ever reads `&visible`. The one
  site that did need a fix was a direct `BattlefrontFrame { visible, .. }`
  construction in `src/ai/advanced/tests.rs`, where a `TileBits` local no
  longer matched the field's new type.
- A handful of direct (uncached) `self.visibility_viewers(pid)` call sites
  remain scattered through `game.rs` outside `vision_input_stamp_with_suzerains`
  (legal-action-enumeration-shaped functions, mostly). Routing them through
  `with_visibility_viewers` would mean restructuring each function's control
  flow into the closure-based API for a caller that, unlike the per-ask sight
  path, is not obviously asked the same question twice — left alone rather
  than widen this pass.

## 2026-08-27 — the envelope table opened one memo scope per flood, and swept a cache that had nothing to sweep

`docs/AI_GAPS.md`'s late-game crawl (#2611) and today's profile both put
`enemy_attack_envelopes → compute_enemy_attack_envelopes →
attack_reach_from_flood → flow_past` at about a fifth of the main thread. The
floods themselves are the answer to a real question and this entry does not
touch one. It is about the four things the machinery *around* them charged on
every ask, none of which can change a decision.

Counted, not clocked — `AtomicU64`s in a scratch build that is not committed,
over one whole 150-turn game at the shape `tools/speed_ab.py` uses (`civvis
simulate --seed 7320000 --jobs 1 --players 6 --turns 150 --width 74 --height 46
--city-states 9 --speed online --map continents`, `ci` profile):

| per game | before | after |
| --- | ---: | ---: |
| envelope asks (`enemy_attack_envelopes`) | 51,500 | 51,500 |
| of those, board-key hits | 16,319 | 16,318 |
| table computations | 35,181 | 35,182 |
| `attack_reach_from_flood` calls | 218,269 | 218,260 |
| **outermost `query_memo` scopes those floods opened** | **218,269** | **35,182** |
| per-enemy store sweeps (`store.retain`) | 35,181 | **2,775** |
| store entries those sweeps walked | 3,076,566 | **227,564** |
| unit records the board diff built per game | 8,977,330 | 8,977,650 |
| …of those, hashed twice into and out of a `HashMap` | 8,977,330 | **0** |
| fingerprint mixing rounds, unit fields only | 734,608,784 | **91,826,098** |

⚠ The *after* column is one run of a build that also carried two candidate
changes to the speculative-attack path which were then **discarded** — see
"What the counter refused" below. They are why `floods` moves by 9 in 218,269
and `hits` by one; no other row is theirs.

### 1. Every flood was the outermost memo scope of its own

`Game::flow_past` opens `self.query_memo()`, and nothing above it did — so each
of a table's floods was an outermost scope, and everything a scope holds was
derived again for the next enemy in the same loop. `air_patrols` is a scan of
every unit in the world, and it exists as a memo entry precisely because
`can_enter_past` asks it once per neighbour of every tile a flood expands;
`passage_improvements` is a table over every improvement in the ruleset;
`traversal` and `movement` are the per-unit terms of the step cost. **218,269
floods over 35,181 tables: 83.9% of those scopes were one flood paying again
for what the previous flood in the same table had just derived.**

One scope now wraps the whole per-enemy loop. It is exact because a memo scope
can only lie if the board moves under it, and this one is taken over `&Game` —
nothing inside can mutate it — and it is dropped before the ask returns, so no
caller inherits it.

It is opened at the call site in `enemy_attack_envelopes` rather than inside
`compute_enemy_attack_envelopes`, which has the same effect and leaves that
function's interior to #2632.

### 2. The board diff rebuilt a `HashMap` of the whole army on every ask

`envelope_board_delta` built a fresh `HashMap<u32, EnvelopeUnit>` of every unit
in the game, hashed each id again to look its predecessor up, and threw the
previous table away — **8,977,330 records a game, each hashed twice**, to
report a list of touched tiles that is usually one unit long.

Both rosters are ordered by id: `Game.units` is a `BTreeMap`, and the stored
board is now a `Vec<(u32, EnvelopeUnit)>` kept in the same order. The diff is a
merge — one pass, one allocation, no hashing at all. The comparison it makes is
the same `EnvelopeUnit` equality over the same fields, and
`the_delta_tracks_every_field_the_board_key_hashes` still holds the field set
to the board key's. `the_merge_diff_reports_what_a_whole_board_diff_would`
writes the table version out and compares the two as sets with a moved unit, a
born unit, a dead unit and a changed patrol all in flight at once, which is
where a merge cursor goes wrong and a one-change-at-a-time test does not look.
Only the order within the returned `Vec` differs, and its single reader asks
whether a sensitive set contains any of them.

### 3. The sweep ran on every ask to find the removals of a board that has none

`store.retain(|id, _| g.units.contains_key(id))` walked the whole per-enemy
cache on every ask, hit or miss, and a board loses a unit on very few of them:
**2,775 asks out of 35,181, and 92.6% of the 3.08 M entries it walked were
walked for nothing.**

The delta already knows. `envelope_board_change` reports whether any id the
previous board held is missing from this one — and reports `true`
unconditionally when it gives up and returns no delta at all, which is the case
where nothing is known.

⚠ The induction, and it is the whole of why the skip is exact: every id
inserted into the store belongs to the board being asked about, and the store
and the stored board move together under the store's own lock, so after a sweep
the store holds only ids of that board. If the next ask removes nothing, every
id the store holds is still on the board and a sweep would find nothing to
drop. That argument deliberately does not care *which* board the previous ask
was about — which matters, because a speculative clone draws new unit ids from
the same `next_id` as the board it was cloned from, so two branches can mint
the same id for different units. The branch that goes away takes its ids with
it, that is a removal, and a removal is a sweep.
`a_removal_still_clears_the_per_enemy_store` pins it.

### 4. FNV-1a is defined a byte at a time, and this is not a hash of bytes

The three envelope fingerprints mixed each `u64` field eight rounds at a time.
`attack_envelope_fingerprint` runs once per ask over every unit on the board —
**13,118,014 units a game, seven fields each, 734.6 M rounds of
xor-and-multiply to answer "is this the same board"**. The mix is now the one
`vision_key` uses in `src/game.rs`, a round per field: **91.8 M**. The
belligerence and city fingerprints, which the board diff pays once per ask,
take the same 8→1.

Nothing reads the number. It is compared for equality with the previous ask's
and discarded, so the only property that matters is that different boards keep
giving different values; the field set and its order are untouched, which is
what the argument for what the key *covers* rests on.

### What the counter refused

Two changes to `can_survive_by_attacking` were written, measured and dropped.

- **The speculative boards do evict the real one's table, and it costs
  nothing.** Each candidate attack clones the game and asks the clone for its
  envelopes, and `enemy_attack_envelopes` keeps exactly one entry — so the last
  speculative board's table is what the caller's next ask finds in the slot.
  Saving and restoring the entry around the loop (exact: the slot is keyed on a
  fingerprint over everything reach reads, and an entry answers for its own key
  or not at all) moved the game's flood count by **9 in 218,269** and its hit
  count by **one**. 1,714 calls and 1,343 speculative asks a game, and the
  pollution is worth four thousandths of a percent of the floods, because the
  caller is still holding the table it asked for and the next ask is about a
  board that has moved anyway.
- **A guard that never fires is not a saving.** `legal_actions_within` is
  enumerated for the whole seat to keep the handful of actions belonging to one
  unit — 205,768 actions thrown away per game — and all three action kinds it
  filters for are gated on `attacks_left > 0`, so reading that field first is
  an exact early-out. The counter says it fired **zero** times: `retreat_step`
  runs before the unit has attacked. A per-unit action generator would be the
  real fix and it belongs to `Game`, not here.

### What is still on the table here

- **The diff still walks every unit.** `Units` has the machinery to skip it
  outright — `snapshot()` holds the roster `Arc` and `Arc::ptr_eq` against it
  is a sound "nothing changed at all", stronger than any field comparison. It
  is not used here because holding that `Arc` forces the game's next unit write
  to copy the whole roster map, and the ask that would benefit is rare: the
  board key is a *miss* on 68% of asks, and a miss usually means the board did
  change. Worth revisiting for the seat-turn's first ask, where the key misses
  because `turn` or `pid` moved rather than because the board did.
- **⚠ There is no reach bound to pre-check with, and the comment that says
  there is describes a function deleted in #2163.** The doc block above
  `envelope_sensitive_tiles` still opens *"the largest number of tiles an
  enemy's next-turn reach can span … `MIN_STEP_COST` 0.25, which is the
  floor"*; that is the key `envelope_sensitive_tiles` replaced, `MIN_STEP_COST`
  does not exist anywhere in the tree, and `Game::unit_step_cost` ends
  `cost.max(0.0)` — **the floor the engine enforces is zero, and 0.25 is a
  claim about the shipped ruleset's data**. 79.1% of `retreat_step`'s 46,731
  calls a game end at `anything_can_reach` saying no, so a pre-check that could
  skip building the table for those is worth real money — but it would rest on
  a movement floor the engine does not guarantee, and on a radius around the
  unit's own tile, which is the wrong centre for an air unit (`attack_reach`
  centres those on `air_operation_origin`). Both are why #2163 replaced the
  radius with the unit's own flood in the first place.

### Exactness

`tools/speed_ab.py`, two paired 150-turn games at the shape above: **"same game
on every seed"** — the two binaries produce byte-identical reports.
`advanced_v1_plays_the_same_game_it_always_did` passes. The timing half of that
run is not quoted: the host was at load 61 with a sibling simulation on it, and
`docs/speed_ledger.json`'s conditions block is right that a number measured
there is a measurement of the machine.

## 2026-08-27 — the forcing-reply tie-break stopped formatting a `Debug` string it usually never needs

`forcing_reply_penalty_applied` / `forcing_reply_line` (7-14% of the main
thread; see the profiles above) hard-coded its search shape and built a
sort-tie-break `String` eagerly for every candidate reply, whether or not the
sort ever looked at it. Three decision-neutral changes, none touching what
the search decides:

- The extension depth (`2`, at the `forcing_reply_line` call inside
  `forcing_reply_penalty_from_position`) and the move-ordering width
  (`.take(4)`) are now `AdvancedAi::FORCING_REPLY_DEPTH` and
  `FORCING_REPLY_WIDTH`, same values, with the reasoning attached at the
  declaration instead of left implicit at the call site.
- `reply_branches`/`ordered` used to store `format!("{reply:?}")` (or
  `format!("{movement:?} -> {followup:?}")`) as the sort key the moment a
  candidate was built. They now store the raw `Action`(s); the `sort_by`'s
  `then_with` only calls the new `forcing_reply_label` helper — the same two
  `format!` bodies, unmoved — when two candidates already tie on the primary
  `f64` score. `total_cmp`/`String::cmp` semantics are unchanged, so the
  final order cannot move; a unit test
  (`ai::advanced::tests::forcing_reply_lazy_key_tests::lazy_tie_break_matches_eager_key_ordering`)
  sorts one synthetic candidate list (including a three-way tie) both the old
  eager way and the new lazy way and asserts the two orders — and a
  hand-checked expected order — are identical.
- **Measured with a temporary, uncommitted counter** (one `AtomicU64` behind
  `forcing_reply_label`, one at the point a candidate is queued — deleted
  before this shipped): one 150-turn, 6-player, 9-city-state game (seed
  7,320,000, online speed, `AdvancedAi::new()` in every seat) queued **5,913**
  reply candidates — what the old code would have formatted unconditionally
  — and the lazy tie-break actually called `forcing_reply_label` **2,752**
  times, a 53.5% cut in that allocation. (The remaining 2,752 is not noise:
  a large share of candidates score exactly `0.0` — a reply that never
  connects with the victim — so ties are common at that one value, and
  `sort_by`, unlike `sort_by_cached_key`, can re-invoke the comparator on the
  same pair more than once during the sort.)

**Not done, and why:** `forcing_reply_penalty_from_position` clones the
position once per enemy seat (`after.speculative_clone()`) before resetting
that seat's move/attack/strike state for the search. Skipping that clone for
a seat that provably has no reachable reply would be exact, but "provably
no reachable reply" has to reproduce two different distance thresholds
exactly — the direct-attack test inside `forcing_attacks_to` (adjacency for
melee, `attack_range` for ranged, plus the city/encampment strike range) and
the wider `attack_range + 2` mobile-attacker test inside `forcing_reply_line`
itself — kept in lockstep with whichever of those two changes next. That
duplication risk, in a file several other agents are editing concurrently
this session, is not a decision-neutral change I could sign off on in this
pass, so the clone stayed. Clone/`speculative_clone` counts are therefore
unchanged by this entry. Separately, `forcing_reply_penalty` (the
`#[allow(dead_code)]`, tests-only sibling of `forcing_reply_penalty_applied`)
already carries its own justification comment from #2578 explaining why it
is kept; its callers live in `src/ai/advanced/tests.rs`, a path this task did
not claim (owned by a concurrent PR), so it was left untouched rather than
edited out from under that PR.

Exactness: `tools/speed_ab.py` on 2 games/seed 7,320,000 (6p, 74x46, 9CS,
150t, online) reported *"the two metrics agree, as they must when both arms
play the same game"* — no digest mismatch across either pair. `cargo test
--profile ci --locked advanced_v1_plays_the_same_game_it_always_did` passed.
Per the note above the 2026-08-23 entries, the timing number from that
`speed_ab` run is not quoted here: the host was carrying another CIVVIS
process at the time (load average 43-49), so only the identity result is
load-bearing.

## 2026-08-27 — three copies of the movement flood became one, and the tie-break inside it was the whole risk

`Game::flow_past`, `Game::path_to` and `Game::approach_reach` were three
hand-copied relaxation loops over a `BTreeMap<Pos, f64>` and a re-pushing `Vec`
stack. `flow_past` is 11.0% of the main thread in the 2026-08-23 profile above
and `path_to` sits behind every AI march, so the same loop had been optimized
once and then not twice: the 2026-08-22 and 2026-08-23 hoists landed on
`flow_past`, and `path_to`/`approach_reach` kept asking the ruleset the same
questions per neighbour that `flow_past` had already stopped asking.

`Game::relax_movement` is that loop once. The two differences between the three
callers are parameters:

- **the step test** — a closure: `entry_at(..) != Entry::Blocked` for
  `flow_past` (which optionally reads through units), `can_pass` for the other
  two;
- **what a zone of control does to the arrival** — the `FloodArrival` value
  type. `f64` writes 0, which is the right answer to *can it move on*.
  `(f64, bool)` keeps the movement and marks the tile as one nothing expands
  from, which is the right answer to *can it still strike* — a unit stopped by a
  zone of control keeps its unused movement for the blow.

Parents are reported through a callback, so `path_to` and `approach_reach` get
the flood's own parents and `flow_past` allocates no parent map at all.

### ⚠ The tie-break is the answer, not an implementation detail

The stack is LIFO and re-pushes a tile every time it improves; the neighbour
order is `nbrs`; the improvement test is a strict `>`. Between them they decide
which parent wins a tie and therefore **which of several equally long walks
`path_to` returns**. Two floods can agree on every distance and still hand back
different paths.

A first draft of the kernel got one detail of that wrong, and it is worth
writing down because the identity check did not catch it. `flow_past` and
`path_to` apply the zone-of-control zeroing **before** the improvement test:

```rust
if self.formation_enters_enemy_zoc(uid, n) { new_rem = 0.0; }
if best.get(&n).map(|b| new_rem > *b).unwrap_or(true) { ... }
```

so a second arrival at a tile inside a zone of control compares `0.0 > 0.0`,
fails, and never re-parents the tile. `approach_reach` stores the un-zeroed
movement and compares that instead. The draft compared the pre-ZOC value in all
three. Every distance stayed identical — the stored value is still 0 either way
— and **the 2-seed report-identity check passed**. What would have changed is
`path_to`'s parent for a destination *inside* a zone of control, i.e. the walk
returned for exactly the move an attack is made from. (Only the final tile can
be affected: a tile with zero movement is never expanded from, so it can never
appear as a parent.)

The shipped kernel compares `arrival.remaining()` — each caller's own rule —
and the counters below say how often that branch is taken.

### Two unit-only derivations hoisted out of the per-neighbour body

The direct sequel to the 2026-08-23 entry above, which hoisted the *per-unit*
movement profile into the `QueryMemo` scope. Two more terms did not depend on
the candidate tile and were still being re-derived per neighbour:

- `unit_max_moves_at`'s linked/escort branch — `units[&uid]`,
  `rules.units[kind]` and up to two `unit_shares_escort_movement` calls — is now
  `MovesCap`, decided once per flood, with `capped_moves_at` doing the tile half.
- `formation_enters_enemy_zoc`'s `unit_ignores_zoc` and `is_linked_leader` — a
  `SpecMap` lookup and four string comparisons — are now `ZocTest`, decided once
  per flood, with `enters_enemy_zoc` doing the tile half.

Both public entry points are the hoisted pair applied at one tile, so there is
still one definition of each rule.

### The counter

Per the standing rule of this file, the verdict is a counter, not a clock.
Instrumented over one whole game (seed 7320000, 6p 74x46, 9 city-states, 150 turns online, Continents, `--jobs 1`), counting every flood and every
relaxation the kernel performs:

| | | per flood |
| --- | ---: | ---: |
| floods (`relax_movement` calls) | **232,000** | |
| neighbours examined | 7,621,833 | 32.9 |
| arrivals relaxed | **5,310,220** | 22.9 |
| tiles reached | 2,704,390 | 11.7 |

(counters printed every 1,000 floods, so the totals are the last print and
undercount the tail of the game by under half a percent)

Each arrival used to evaluate `unit_ignores_zoc` and `unit_max_moves_at`'s
unit half; both are now evaluated once per flood. **5,310,220 evaluations of each became 232,000** — a factor of 22.9, and 5.08 M `SpecMap` lookups plus their string comparisons removed per game, twice over. The 11.7 tiles reached per flood also reproduces the 2026-08-23 entry's 13.5 at a different shape, which is the number that entry used to reject the dense frontier.

And the tie-break above, counted directly — relaxations where the pre-ZOC
movement improves on the stored value but the post-ZOC movement does not, i.e.
exactly the comparisons the draft would have decided differently:

| | |
| --- | ---: |
| divergent comparisons in one game | **10,657** |
| as a share of arrivals | 0.20% |

Two tenths of a percent of relaxations, in one 150-turn game, on the tiles a
unit attacks from. Not a hypothetical.

### Rejected again, without re-measuring: a dense frontier

The 2026-08-23 entry above measured a dense `route_scratch`-style frontier for
`flow_past` at **+0.81%** and rejected it, with the counter that explains why:
58.7% of these floods reach 11 tiles or fewer, which is a single `BTreeMap`
node — a flat, cache-resident array with no tree descent to remove — against a
`Vec<f64>` sized to every tile on the map and a sort to rebuild the ordering the
tree supplies for free. The ceiling on the whole idea was computed there at
0.4%.

This change keeps the `BTreeMap`. Folding three copies into one is orthogonal to
the container, and re-litigating a measured null inside a kernel whose
visitation order is load-bearing is the wrong risk to take for an 0.4% ceiling.
The relevant standing rule from that entry — *the payer is an expensive
derivation that is recomputed, not a cheap operation that is frequent* — is also
why the two hoists above are the part of this change that could pay.

### The measurement

`tools/speed_ab.py` hashes each game report, so the exactness claim is the half
worth quoting on a loaded host:

```
seeds 7320000..7320001 (2 games x 1 interleave(s) = 2 pairs),
6p 74x46 9CS 150t online continents, --jobs 1
  load average 101.24 at start, 101.24 peak, 84.57 at end
  — same game on every seed

seeds 7320000..7320003 (4 games x 1 interleave(s) = 4 pairs),
6p 74x46 9CS 150t online continents, --jobs 1
  load average 84.57 at start, 106.84 peak, 106.84 at end
  — same game on every seed
```

Read the identity line, not the timing: the host carried a load average above 90
for the whole window and a dozen agents share it.

## 2026-08-27 — `units_at` copied the occupancy list at 292 call sites for one that reads it

`Game::units_at(pos)` is `self.unit_ids_at(pos).to_vec()` — a fresh heap `Vec<u32>`
on every call, for a value that lives one statement. `Game::unit_ids_at(pos)`
returns the same ids as a `&[u32]` borrowed straight out of the occupancy
map, no allocation, same order. Both have existed since early on; almost every
call site had kept using the allocating one, including the ones inside a
per-neighbour or per-tile loop — `in_enemy_zoc` over six neighbours,
`unit_heal_rate`'s chaplain search over `wdisk(unit.pos, 1)`, every ZoC and
combat-target scan.

A census at the start of this change found 292 call sites across `game.rs`,
`ai.rs`, `ai/advanced.rs`, and their test/submodule files — roughly twice the
number a line-number sample from an earlier commit had suggested, because the
submodule files (`src/ai/advanced/*.rs`, `src/game/*.rs`) hold as many sites
between them as the three hotspot files do.

### Converting 292 sites by hand does not scale; converting them by compiler does

Reasoning about each site's borrow shape individually — does the surviving
code hold the slice across a `&mut self` call, does a closure need one deref
or two — is exactly what the borrow checker already does per site, correctly,
every time. The conversion here was mechanical instead of manual:

1. Rewrite every `.units_at(` to `.unit_ids_at(` in the claimed files (a
   `sed` pass; the function *definition*, `fn units_at`, doesn't match the
   call-site pattern `.units_at(`, so it is untouched).
2. `cargo check --message-format=json`, and revert to `.units_at(` only the
   specific lines a compile error's spans implicate.
3. `cargo fix --broken-code` picks up everything with a machine-applicable
   suggestion — mostly `for id in unit_ids_at(...)` loops where the bound
   name changed from an owned `u32` to a borrowed `&u32` and downstream
   `*id` / `self.units[&id]` needed a deref adjusted to match.
4. Repeat 2–3 until the error set stops shrinking, then hand-fix what neither
   step reaches: `.collect()` into an owned `Vec<u32>` (or a tuple containing
   one) needs `.copied()` or a `*id` inside the constructing closure — cheap,
   because only the *surviving filtered subset* gets copied, not the whole
   occupancy list up front, which is a net win over the pre-change code, not
   a wash; and the borrow-conflict idiom below, which the compiler correctly
   refuses and this change leaves alone.

The one idiom that has to keep allocating: a site that finds a target unit
and then calls `&mut self` methods against it for the rest of the block
(`apply_unit_damage`, `remove_unit`, `record_war_unit_participation`, …) needs
an *owned* id, because a borrowed `&u32` sourced from `self.occ` would still
be live across those mutations. Religious combat resolution and the
encampment-strike defender search are both this shape; both were left on
`units_at`, exactly as the task that produced this change predicted.

### The result, by file

| file | now borrowing (`unit_ids_at`) | still allocating (`units_at`) |
| --- | ---: | ---: |
| `game.rs` | 65 | 11 |
| `ai.rs` | 56 | 0 |
| `ai/advanced.rs` | 34 | 2 |
| `ai/advanced/*.rs` (submodules) | 97 | 1 |
| `game/*.rs` (submodules) | 37 | 2 |
| **total** | **289** | **16** |

The 16 remaining sites are the mutate-while-holding-the-id shape above, plus
one `.collect()` into an owned `Vec` that a caller elsewhere still consumes by
value — converting it would only move the allocation, not remove it.

### The counter

A temporary `AtomicU64` on `units_at` (not committed — a scratch build only),
one 150-turn, 6-player game, seed 7320000, `--width 74 --height 46
--city-states 9 --speed online --map continents`, `--jobs 1`:

| | calls to `units_at` |
| --- | ---: |
| before | **19,731,233** |
| after | **274,822** |

A 71.8x drop — 98.6% of what used to be a fresh heap allocation per call is
now a slice borrow. The 274,822 that remain are the sites this change
correctly left alone.

### Exactness

`tools/speed_ab.py` hashed both arms' reports on seeds 7320000–7320001
(2 games, 6p) — same game on every seed. The two runs that produced the
counter numbers above are the same comparison with the instrumentation left
in: their stdout (turn timer aside) is byte-identical.
`advanced_v1_plays_the_same_game_it_always_did` passes, and the full suite
(`cargo test --profile ci --locked`) is green: 2,586 lib tests plus every
integration and protocol-parity suite, 0 failures.

## 2026-08-27 — the policy review valued the whole empire to price a Great-Scientist card

`AdvancedAi::production_weights` sets `PolicyDeck::Live`, so every deployment
game runs the counterfactual policy deck. `revise_policy_deck` reviews the deck
every turn while a slot stands empty and every `POLICY_REVIEW_EVERY = 8` turns
once it is full, and `policy_card_score` prices each candidate — every card
`available_policies` offers, plus every card already slotted — by running one
**whole-empire `empire_reading` sweep** with that card added or removed.

Today's profile of a culture-win seed puts the chain at **7% of the main
thread**, with `city_yields_inner` at 11% inclusive:

    research_with_government → revise_policy_deck → policy_card_score
        → empire_reading → city_yields → city_yields_inner
                         → item_prod_mult

### What that sweep actually reads

`empire_reading` is five terms, and this is the whole list:

1. `city_yields(cid)` for every city the seat owns;
2. `item_prod_mult(pid, cid, queued)`, the production multiplier toward the item
   that city is building;
3. `observed_yield_adjustments`, a live-bridge correction no card can move;
4. `unit_strength(unit, false)` and `unit_strength(unit, true)` for every unit;
5. `policy_effect(pid, "influence_per_turn")`, plus `unit_gold_maintenance(pid)`
   behind `maintenance_aware_deck`.

Most of the catalogue reaches none of them. `data/policies.json` ships 129 cards
carrying 157 distinct effect keys between them — Great-Person points, envoys,
espionage, tourism, war weariness, loyalty, upgrade and purchase discounts,
grievances — and one sweep asks for a few dozen. For every other card the sweep
recomputes a number the review already had in hand: it had valued the unchanged
empire one line earlier, as `current_reading`.

### The classifier is a trace, not an allow-list

The obvious implementation is a hand-written allow-list of inert effect keys.
It was rejected. Being right about 157 keys means being right about every branch
of `city_yields_inner`, `player_tile_yields`, `item_prod_mult`, `unit_strength`
and the maintenance bill, in a call graph nobody holds in their head — and an
allow-list that is wrong is wrong *silently*, in the direction that changes
decisions. It also goes stale the first time a card or a yield path is added,
with nothing to say so.

`Game::trace_policy_reads` records instead. While its guard is alive, the three
functions that are the only routes from a slotted card to an answer —
`policy_effect`, `policy_effect_for_unit`, `has_policy` — note the effect key or
card name they were asked for. The review opens the trace over the one sweep it
runs anyway, and gets back the exact set of questions that sweep asked this
seat's deck on this board.

The argument the skip rests on is three sentences. A card whose effect keys all
sit outside the recorded set changes no answer the sweep read. The two runs
therefore return the same value at every lookup, take the same branch at every
branch, and ask the same next question. So the second sweep lands on the same
`f64` bits as the first, and running it is a way of finding out something
already known.

Three holes, each closed rather than argued away:

- **A card read by name.** `has_policy(pid, "x")` names a card rather than a
  key, so the trace records card names too and a named card is never inert.
- **Runtime modifier attachments.** `Game::modifier_context` hands the whole
  deck to `ModifierRequirements::matches`, which can gate a modifier on holding
  one named card — by name, not by key. A trace taken on a seat carrying any
  attachment is therefore marked *opaque* and rules nothing inert. It is the
  same seat class `policy_effect`'s own index short-circuit already excludes.
- **Anything reading `players[pid].policies` directly.** `grep` over `src/`
  finds five non-test sites: `has_policy`, `modifier_context`, `policy_effect`,
  `policy_effect_for_unit`, and the mutators (`trim_policies_to_slots`,
  `prune_policies_to_government`, `policies_fit`, `available_policies`,
  `Game::apply`) — none of which the reading calls. That grep is the
  completeness claim, and it is cheap to re-run.

**It can only be wrong slowly.** An unknown card, a key the sweep did ask for,
an attachment, a seat in anarchy: every unclassifiable case answers "not inert"
and pays exactly the sweep it would have paid anyway. No arm of this is fast
and wrong.

### Counted, not clocked

Three `AtomicU64`s on a scratch build — one on `empire_reading`, one on the
skip, one on `city_yields_inner` — plus an env switch that restores the old
behaviour, so **one binary measures both arms of the same game**. Seed 7320000,
6 players, 74×46, 9 city-states, 150 turns, Continents, `--jobs 1`:

| per 150-turn game | before | after | |
| --- | ---: | ---: | ---: |
| policy reviews | 155 | 155 | — |
| `empire_reading` sweeps | 4,665 | **1,317** | **−71.8%** |
| ↳ candidate sweeps (excl. the 155 baseline readings) | 4,510 | 1,162 | −74.2% |
| `city_yields_inner` calls, **all callers, whole game** | 29,927 | **19,623** | **−34.4%** |

**A third of every city-yield derivation in the simulator was the policy review
rediscovering that a Great-Person card is worth nothing.** 10,304 of the 29,927
went to candidate cards that could not move the answer, and the review as a
whole runs 3,348 fewer whole-empire sweeps per game.

### ⚠⚠ The clock is not quoted, and the control is why

Four paired runs were taken today, every one of them reporting *same game on
every seed*. **One of them compares the baseline binary with a byte-for-byte
copy of itself.**

| run | arms | load, start → end | median s/turn | resolves |
| --- | --- | --- | ---: | ---: |
| A | base vs change | 84 → 60 | +11.17% | ±30.06% |
| B | base vs change | 67 → 13 | +14.32% | ±23.22% |
| **C** | **base vs a copy of base — identical machine code** | 32 → 16 | **−13.18%** | ±12.12% |
| D | base vs change | 16 → 9 | +10.14% | ±19.07% |

**Run C is the whole finding.** The same 27,929,584 bytes against themselves
measured −13.18% per completed turn, pooled −15.19%, with an IQR spanning
[−27.33%, −6.05%]. Nothing was changed and a 15% improvement was reported. On a
host shared with ten sibling agents, this instrument cannot resolve anything
smaller than about ±15% today, and A, B and D all sit inside that.

Note the *signs*. `speed_ab` runs baseline then candidate within each pair, so
whichever way the host's load is drifting during a run biases the second arm.
Run C's load fell throughout and it reported the second arm 13% faster; runs A,
B and D each began under heavier contention than they ended. That is a
mechanism, not a mystery, and it is worth remembering that alternating arms
*within* a pair does not remove a drift that is monotone *across* a pair.

**Take a same-binary control whenever a paired reading has to carry weight on a
loaded host.** It costs two more games and it is the difference between a
number and a story. The counter above does not care what else the machine is
doing, and neither does the report digest.

### The identity, three ways

1. `tools/speed_ab.py`, the merge-base binary against the change, on **14
   paired 150-turn games** across runs A, B and D above (seeds 7320000–7320001,
   7320010–7320015, 7320040–7320043), 6p 74×46 9CS online continents:
   **`same game on every seed`**, every run.
2. The counter run: the same scratch binary with the skip on and off produced
   **the same report digest** (`f7b56b14…`) while running 3,348 fewer sweeps.
   Same code, same seed, one switch — the cleanest form of the claim, because
   it holds the compiler constant too.
3. `a_reading_inert_policy_card_moves_the_empire_reading_by_exactly_zero`:
   1,032 verdicts checked against the sweep itself, on `to_bits`.
4. `advanced_v1_plays_the_same_game_it_always_did`, the frozen-identity anchor.


### The test is the implication, card by card

`a_reading_inert_policy_card_moves_the_empire_reading_by_exactly_zero` plays
four majors and six city-states forward 110 turns under the production
controller, then walks **every card in the ruleset on every major seat**: it
asks the classifier, runs the sweep the classifier called unnecessary, and
asserts `to_bits()` equality — not an epsilon, because the skip reuses a number
and anything short of bit equality is a defect. Both sides of the counterfactual
(removal for a card the seat holds, slotting for one it does not) and both
`net_maintenance` arms, **1,032 verdicts** on one board:

| seat | cities / districts / units | inert, no maintenance | inert, with |
| ---: | --- | ---: | ---: |
| 0 | 3 / 1 / 15 | 104 of 129 | 101 |
| 1 | 3 / 1 / 10 | **109** of 129 | 106 |
| 2 | 2 / 3 / 24 | 105 of 129 | 102 |
| 3 | 1 / 2 / 20 | 105 of 129 | 102 |

**113 cards were inert on some seat and 38 were live on some seat**, and the two
sets overlap: the verdict is a property of the *board*, not of the card. A seat
whose capital is queueing a Settler asks `item_prod_mult` for
`settler_production_pct` and a seat queueing a Granary does not, so Colonization
is live on the first and inert on the second. That overlap is asserted, because
a classifier that answered the same on every board would not have needed the
trace.

`conscription` is the boundary case worth remembering, and the existing
maintenance test now pins it from both sides: its whole effect is
`unit_maintenance_discount`, so with the bill unread it is inert and the review
skips it, and under `maintenance_aware_deck` the same sweep asks for that key
and the same card is live. **The classifier describes the sweep, not the card.**

### What was not done: memoizing city yields across candidates

The other half of the idea was to memoize per-city yields across candidate
cards. `empire_reading` already opens a `query_memo` scope per call, so the
yields are shared *within* one sweep; sharing them *between* sweeps needs a key
on the yield-relevant policy subset, which is the classification above — and
once a card is classified inert there is no second sweep left to memoize. What
remains is a *per-city* trace: a card that can only reach cities with a Campus
leaves every other city's yields untouched, so a sweep that still has to run
could skip most of its cities. That is the next step here, and it needs the same
lockstep argument at city granularity.

## 2026-08-27 — 44 of the board's fields belong to the host, and no search branch has ever written one

The 2026-08-26 entry above found `Vec::clone ← Game::clone` and
`Vec::clone ← speculative_clone` inside the unnamed third of the profile,
**2.19% of the busiest thread between them** — and that is only the `Vec` half
of what a whole-board copy costs. `Game` has 139 fields. **82 of them are a
`BTreeMap`, `BTreeSet` or `Vec`**, and each one was deep-copied, tree node by
tree node, on every clone; there are 40 `speculative_clone` call sites and the
board is cloned outright in hundreds more places.

**45 of those 82 fields are not the simulator's state at all.** They are the
live bridge's import of what Civilization VI told this seat: `observed_*` (25),
`host_*` (10), `blocked_*` (10). Every write of all 45 was enumerated — `x =`,
`.x.insert(`, `.x.clear(`, `.x.entry(`, `.x.extend(`, `&mut g.x`,
`mem::take(&mut g.x)` — and every one lands in one of three places:
`src/mirror.rs`'s host-state steps, `Game`'s own mirror setters
(`replace_host_menus`, `replace_blocked_production`,
`replace_blocked_purchases`, `replace_host_competitions`,
`clear_mirror_cities`, `mirror_remove_city`), or a test fixture building the
board a live import would have produced. **Not one is written by `Game::apply`,
`begin_turn`, `do_end_turn`, or by anything reachable from `BasicAi` or
`AdvancedAi`.** A search branch reads them and never touches them.

44 of the 45 now hold an `Arc`, so cloning one is a refcount increment. Where a
write does happen it goes through `Arc::make_mut` — copy-on-write, so the
semantics are the same even if some future caller writes to a board it shares —
or rebuilds and `Arc::new`s in the mirror setters. Reads deref and did not
change. `GameSer`, the struct `Game` serializes through, was left holding the
plain collections, so no `Arc` reaches the wire and the JSON a save or `/state`
carries is byte-for-byte what it was;
`arc_shared_host_imports_keep_the_save_format_they_had` asserts that from
outside, including that the refusal sets the save format has always omitted are
still omitted.

### The counter, and the thing it says that a clock would have hidden

A scratch zero-sized field whose `Clone` impl bumps an `AtomicU64` counts every
derived `Game::clone`, `speculative_clone` included. One 150-turn game, seed
7320000, at the screen's shape (6 players, 74x46, 9 city-states, `online`,
`continents`), `ci` profile:

| | |
| --- | ---: |
| whole-`Game` clones in the game | **111,458** |
| collections each clone no longer walks | 44 |
| `size_of::<Game>()` after | 3,456 B |
| the 44 fields' own bytes, before → after | 1,064 B → 352 B |
| struct memcpy removed, per clone | **712 B** |
| struct memcpy removed, over the game | ~79 MB |
| **entries those 44 fields held at turn 150** | **0** |

That last row is the finding, and it is the one no A/B could have told us.
**In a game the simulator plays by itself, all 44 fields are empty on every
turn** — nothing but the live bridge ever fills them — so the deep copy this
removes is, in the benchmark, 44 branches on an empty root and 712 bytes of
`memcpy`, repeated 111,458 times. The tree-walking half of the saving is
entirely a *live-bridge* saving, and that is where it is large: `host_observed`
alone is every plot the seat has revealed, `host_buildable` and
`host_purchasable` are a menu row per item per city, and
`host_district_sites` / `host_wonder_sites` / `host_district_plots` are
`BTreeMap<u32, BTreeMap<Name, BTreeSet<Pos>>>`. A mirrored board carries
thousands of entries there and cloned all of them per search branch. There is
no recorded host state in the tree to put a number on that, so this entry does
not invent one.

### What was left, and why

- **`observed_climate`**, the 45th, stays a plain `Option<ObservedClimate>`:
  seven `Option<f64>`/`Option<i64>` and not one heap allocation, so an `Arc`
  would add a pointer chase to a 112-byte memcpy rather than remove one.
- **`occ: BTreeMap<Pos, Vec<u32>>` and `city_by_pos: BTreeMap<Pos, u32>`** were
  considered and rejected. They are derived indexes that the simulation itself
  rewrites — every unit step, every founding, every capture — so they are
  written on the search path by definition. `Arc::make_mut` would deep-copy the
  whole index on a branch's first move and charge the `Arc` on top of that:
  strictly worse than the copy it replaces.

### Exactness

`tools/speed_ab.py`, two paired 150-turn games at the shape above, run three
times — twice against the merge-base and once against the `origin/main` this
branch merged: **"same game on every seed"** every time, byte-identical
reports. `advanced_v1_plays_the_same_game_it_always_did` passes; `mirror` is
224 tests green.

The timing half is not quoted, and here is why in one line: the same two-pair
comparison read **-11.23%** at load 27, **+1.59% (resolves ±1.66%)** at load
4.5, and **-13.40% (resolves ±0.63%)** at load 13 — three runs of the same
question, and one of them is the wrong sign. Two pairs do not resolve a change
of this size on a host shared with a dozen agents, which is the whole reason
the evidence above is a counter.

⚠ **`rustfmt --edition 2021 src/game.rs` reformats every `src/game/*.rs`
submodule too** — 41 files, 1,677 lines here, none of them yours. `game.rs`
declares those modules and rustfmt follows `mod`. The narrower trap beside the
known `cargo fmt -- <file>` one; check `git status`, not just the file you
named.

## 2026-08-27 — movement loops stop measuring edges they just enumerated

The 2026-08-26 opportunity list measured `Sphere::distance` at 3.31% of
running self time and its ring `binary_search` at 1.61%. The generic movement
gate was one of those callers: `Game::entry_at` rejected `wdist(from, pos) !=
1` even when a flood or route loop had just obtained `pos` from `nbrs(from)`.
The predicate is necessary for a direct, arbitrary-coordinate `Move` request;
it is not work a loop needs to repeat for each of its already-adjacent
candidates.

`entry_at` therefore remains the general gate, with its exact distance guard.
Its rule body lives in a private `entry_at_neighbor` helper, used only by loops
whose destination comes directly from `Game::nbrs`: the shared
`relax_movement` kernel behind `flow_past`, `path_to`, and `approach_reach`;
`reach_steps`; the pass-through probe; and the first-edge gates in all three
route finders. The future-route terrain gate has the same private, neighbor-only
form. No public or arbitrary-coordinate caller lost its guard.

The existing movement counter makes the lower bound concrete: one 150-turn
deployment-shaped game reached **7,621,833** neighbor examinations in
`relax_movement` alone. Every one that reaches the gate now skips an exact
world-distance lookup; the route and per-step callers above are additional
savings not included in that flood counter.

### The precondition is tested on both world shapes

`neighbor_fast_path_matches_the_generic_entry_gate_on_both_world_shapes`
enumerates every `nbrs` edge of a wrapped cylinder and a Planet globe. It
asserts `wdist(from, to) == 1` for every edge, then compares the generic and
neighbor entry, stop, and crossing gates on every one. That catches the only
new assumption at the wrap seam and at the globe's pentagons, rather than
assuming ordinary hex-coordinate arithmetic describes either shape.

The focused 16-test movement suite, the frozen `advanced_v1` identity anchor,
and the complete local `cargo test --profile ci --locked` suite passed (2,611
library tests, 44 ignored, plus every binary, integration, protocol, and doc
target). A fresh 2-game 6-player, 74×46, 9-city-state, Online Continents soak
reached turns 212 and 243 without a failure.

### The local clock is deliberately not a speed claim

Two independent, two-pair `tools/speed_ab.py` windows at 6 players, 74×46, 9
city-states, 150 turns, Online Continents each reported **same game on every
seed**. Their clocks point in opposite directions because the host's load did,
and neither window resolves a change this small:

| seeds | load, start → end | median / resolution | pooled |
| --- | ---: | ---: | ---: |
| 7,320,200–201 | 22.81 → 11.73 | −4.45% / ±12.84% | −5.18% |
| 7,320,210–211 | 9.27 → 29.29 | +25.78% / ±18.71% | +23.76% |

The equal report hashes are load-bearing; the opposite timing signs are not.

### The first CI gate saw no measurable regression

GitHub's required five-pair deployment gate, against base `49722445`, also
reported **same game on every seed**: −0.24% median per completed turn, ±1.61%
resolution, −0.39% pooled (600 turns per arm), at load 4.77 → 1.03. That is
inside its 1% noise floor and comfortably inside the +8% regression budget:
the honest conclusion is no measurable change in the runner, not a speed
claim. The gate will run again after this branch's current-main integration;
that final result is the merge guard.
