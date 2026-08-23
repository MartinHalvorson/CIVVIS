# Three merged live repairs, checked against the recorded corpus

_2026-08-23 · `tools/live_repair_census.py` over 560 recorded runs in
`~/civvis-civ6-runs/control`, plus three paired `civvis_orders` replays of 768
recorded turns each_

## What was asked

Three repairs merged on 2026-08-22, and every one of them shipped with **one
live run** as its evidence:

| PR | commit | what it claimed |
|---|---|---|
| #2278 | `c8a90523` | two wrong predicates in the live bridge left nine Great People and four Traders idle |
| #2316 | `d55b03f3` | a stalled Settler could found in a loyalty trap |
| #2319 | `90bc9a09` | a lost position should be restarted rather than played out |

`docs/EVAL.md`'s doctrine says one seed is never a result. The question here is
narrow and it is the one none of the three answered: **did each repair change
what the agent does, at all, on evidence bigger than its own anecdote** — and
where it did, by how much.

A second question is answered on the way, because the task that commissioned
this carried four numeric premises about the live seat. Each is audited below
against the corpus. Three of the four do not survive.

## Why `ai_eval` is the wrong instrument, for all three

None of these repairs runs in a headless game, so no evaluator arm can see any
of them:

- **#2278** lives in `StateGreatPerson::slot_starved` (`src/mirror.rs`) and in
  `src/bin/civvis_orders.rs`. A headless `Game` never constructs a
  `StateGreatPerson`; that type exists only where Civilization VI's export
  does.
- **#2316** is inside `founds_where_it_stands`, behind
  `settler_founds_when_stalled` — off in `AdvancedAi::new()`, on in the live
  bridge because `enable_stranded_settler_discount` bundles it — and behind
  `loyalty_rate_alarm`, which `src/ai/advanced.rs:24422` documents as a
  live-bridge-only flag. Both are off in the deployment genome for native play.
- **#2319** is `behind_all_metrics_reading` in `tools/civ6_play.py`, the
  ladder's own restart policy. There is no game in it at all.

So the instrument is the **recorded corpus**: 560 finished live runs, each with
the exact `events.jsonl` the live harness consumed, 356 of them with the
decider's own `why.log` journal beside it.

## How it was measured

### Instrument A — the census

`tools/live_repair_census.py`, reading and printing only. It never starts a
game, never writes into a run directory, and is safe under the operator halt.

```bash
python3 tools/live_repair_census.py                    # all 560 runs
python3 tools/live_repair_census.py --since 20260819   # the 43 most recent
```

#2319's verdict is a **check**, not a transcription: the recorded events are
fed to `civ6_play.behind_all_metrics_reading` itself, in file order, which is
what `_play`'s `finished()` does with the live stream. #2278's Great Person
predicate is transcribed from `src/mirror.rs` and its truth table is pinned in
`tools/test_live_repair_census.py` against the same case table the Rust unit
test `the_rome_stack_is_starved_even_though_the_empire_owns_empty_slots`
asserts — the three cases it must change and the four it must not.

### Instrument B — the paired replay

Two `civvis_orders` binaries built from the **same** tree at `4747a6d2`, in
throwaway detached worktrees, differing only by a revert:

```bash
git worktree add --detach <scratch>/revert-arm HEAD
git -C <scratch>/revert-arm revert --no-commit d55b03f3 c8a90523   # both applied cleanly
cargo build --profile ci --locked --bin civvis_orders -j4

python3 tools/live_repair_census.py --corpus <8-run-subset> \
  --replay before=<scratch>/revert-arm/target/ci/civvis_orders \
  --replay after=target/ci/civvis_orders \
  --replay-turns 60:250:2 --jobs 4
```

The subset is the **eight recorded runs whose control mod exports
`slot_open`** — the only runs in the whole corpus where #2278's predicate can
read anything. Turns 60 to 250 in steps of two gives **768 recorded turns
answered by both arms, 0 failed invocations.**

Fidelity and its limits, stated because they bound the reading:

- The argument list is `civ6_brain.civvis_orders`'s own, plus `--explain`. In
  particular it does **not** pass `--players`; neither does the live brain, so
  a replay that did would be answering a board the seat never saw.
- Each run is replayed on **the victory lane its own `why.log` genome line
  records**, not on today's default.
- The mirror directory handed to each binary is a temporary directory holding
  a *symlink* to the archived `events.jsonl`. Nothing writes to the archive.
- ⚠ `--turn N` builds a fresh agent per turn. The live loop runs one long-lived
  `--serve --fresh-board` process and keeps plan continuity across turns. A
  per-turn replay therefore cannot see an effect that only accumulates.

## What it measured

### #2278, half one — the Great Person slot predicate: **VERIFIED**

Census, all 560 runs. 237 export a Great Person; **8** export the `slot_open`
field the repair reads (the rest ran an older control mod, whose `None` is
correctly never read as either claim). Restricted to those eight:

| | count | rate |
|---|---:|---:|
| Great-Person unit-frames | 3,341 | |
| starved under the OLD predicate `!can_activate && empty_slots == Some(0)` | **0** | 0.0% |
| starved under `slot_starved` | 2,522 | 75.5% |
| verdict flips | 2,522 | 75.5% |
| cultural (Writer/Artist/Musician) unit-frames | 2,525 | |
| …of which flipped | **2,522** | 99.9% |

The old predicate did not fire **once** in 3,341 observations. #2278's account
of why — `empty_slots` counting empire-wide slots the person cannot reach while
every offered plot reads `slot_open: false` — reproduces in eight independent
games, not one.

Paired replay, 768 turns:

| reading | before | after |
|---|---:|---:|
| turns whose **order set differs** | — | **199 (25.9%)** |
| `no_empty_slot` (the `slot_starved` branch) | 0 | **1,255** |
| `no_plot` | 1,424 | 169 |
| `great_people_orders` | 188 | 188 |
| journal *"activation path for an idle Great Person"* | 17 | **308** |

Two things in that table matter and pull in opposite directions.

**The Great Person driver itself did not change.** `great_people_orders` is 188
in both arms, and the 1,255 person-turns simply move from the `no_plot` bucket
to the `no_empty_slot` bucket. Those people stood still before and stand still
now. The repair's value is not in the unit orders.

**The mirror's needs machinery changed by 18×**, and that is where the whole
effect lands. What the empire built instead, over the same 768 turns:

| production order | before | after | Δ |
|---|---:|---:|---:|
| `BUILDING_AMPHITHEATER` | 52 | 169 | **+117** |
| `BUILDING_MUSEUM_ART` | 54 | 119 | **+65** |
| `DISTRICT_THEATER` | 49 | 84 | +35 |
| `BUILDING_BROADCAST_CENTER` | 0 | 17 | +17 |
| `UNIT_BUILDER` | 1,068 | 1,023 | −45 |
| `UNIT_IRONCLAD` | 497 | 457 | −40 |
| `UNIT_SETTLER` | 385 | 361 | −24 |
| `DISTRICT_CAMPUS` | 189 | 176 | −13 |
| `PROJECT_MANHATTAN_PROJECT` | 88 | 75 | −13 |
| `BUILDING_LIBRARY` | 75 | 63 | −12 |

+234 city-turns of Great-Work capacity, bought mostly out of Builders,
Ironclads and Settlers. That is exactly the actuation the repair claimed, and
it is now measured on 8 games rather than argued from 1.

⚠ **One of the four escape hatches #2278 opened still never fires.** The
work-sale arm produced **zero** orders in either arm across 768 turns: every
`sell` order in both arms is a resource or Favor sale (`RESOURCE_TRUFFLES=1`,
`FAVOR=10`, …), never a `GREAT_WORK_*`. `WORK_SALE_CADENCE` and the need for a
placed work of the starved class's own kind keep it shut. It is not wrong; it
is untested, and it should not be counted as part of what was verified.

### #2278, half two — trade-route corroboration: **VERIFIED IN THE LEDGER, UNOBSERVED IN THE ORDERS**

Census, all 560 runs: **113 distinct origin/destination pairings** were refused
across 72 runs, and **all 113 were refused exactly once. None was ever refused
twice.** Under the old rule each of those 113 was retired for the rest of its
game on that single reading.

⚠ **That 100% is mechanically forced and must not be read as evidence the
refusals were transient.** The old rule put a pairing into
`blocked_trade_routes` at its first refusal, so it was never ordered again and
could never be refused a second time. What the corpus can say is how many
pairings were retired on one reading — 113, or 1.57 per affected run. What it
cannot say, and what no recorded artefact can, is how many of them would have
succeeded on the retry that corroboration now grants.

The replay saw no order change at all: `TRADE_ROUTE` is 103 in both arms. The
eight replayed runs carry four refusals between them, so the rule had almost
nothing to hand back.

⚠ **The corroboration buys one retry, not a recovery.**
`refused_trade_routes_through` (`src/mirror.rs:13291`) counts refusals
anywhere in the game, with no window and no expiry, and retires a pairing at
`TRADE_ROUTE_REFUSALS_BEFORE_BLOCK = 2`. So a transient condition that lasts
more than one turn — a war not yet ended, a border that opens later, a route
slot filled on two consecutive turns — still condemns that pairing for the
rest of the game, exactly as before. The corpus cannot show this yet, because
under the old rule a condemned pairing was never re-ordered and so never
reached a second refusal; the first live run on the new rule will. **The named
next repair is a window or an expiry** — two refusals within N turns, or
re-open after N — which is the shape `settler_dead_sites` already uses with
`SETTLER_DEAD_SITE_AVOID_TURNS`, in the very function #2316 edited.

⚠ **Scale.** The motivating run reported 23 pairings in one game. The corpus
rate is **0.20 pairings per run**. That run is a hundredfold outlier, and the
typical benefit of this half is correspondingly smaller than its PR body
implies. Final unused trade capacity across the corpus is 0.89 routes, mean.

### #2316 — the loyalty-doomed fallback guard: **STILL UNVERIFIED**

The guard never fired.

| reading | before | after |
|---|---:|---:|
| journal *"loyalty-doomed fallback"* | 0 | **0** |
| `FOUND_CITY` orders | 46 | **46** |

768 recorded turns of eight games, and the settle behaviour of the two arms is
identical. This is **not** a refutation — nothing here says the guard is wrong,
and it is cheap and locally correct. It is an absence of occasion. Reaching it
needs all of: `settler_founds_when_stalled` (on at the live seat),
`loyalty_rate_alarm` (on), a settler that has run its full stall counter,
`g.can_found_city(uid)` true on the tile it happens to be standing on, and the
loyalty verdict then refusing that tile. `treatment_flags.rs:1648` already
records that `can_found_city` refusing the tile is what usually ends that path,
and this measurement is consistent with it.

For context on how much room there is: 2,949 Settlers appear in the corpus,
**1,204 (40.8%) never found a city**, median 5 turns alive, p90 52, p99 146,
max 203 — and 127 of them stand 50 turns or more. The stalled-settler
population is large. The specific tile-and-loyalty coincidence this guard
answers is not.

### #2319 — the three-signal restart: **VERIFIED as a behaviour, with a tight risk bound — and NOT ARMED**

Replayed through the harness's own function on all 560 runs at the operator's
requested ratio of 0.70:

| | value |
|---|---:|
| runs the rule would have restarted | **36 of 560 (6.4%)** |
| median turn it fires | 104 |
| turns of play it would have skipped | 3,713 (mean 103 per stopped run) |
| stopped runs that later regained the 0.70 score ratio | 9 (25.0%) |
| stopped runs that were **won** | **0** |
| median FINAL score ratio of a stopped run | 0.47 |
| wins anywhere in the corpus, for scale | 9 of 560 |

⚠ This is a **risk** bound, not a value reading. A restart replaces the rest of
that game with a different game whose result is unrecorded, so nothing here can
say the policy is worth its cost. What it can say exactly is what the policy
would have thrown away, and the answer is: of 36 games it would have stopped,
none was won, and the median one finished at 47% of the leader's score. All
nine corpus wins lie outside the stopped set. A quarter of stopped games did
climb back over the score threshold — but none of them converted it.

⚠⚠ **The policy is not switched on.** Every layer defaults it off:
`civ6_play.py --restart-below-leader-ratio` defaults to `0.0` (disabled);
`civ6_civvis_climb.py` defaults to `None` and forwards nothing;
`tools/ops/civvis-game-supervisor.sh:168` reads
`RESTART_BELOW_LEADER_RATIO=${CIVVIS_RESTART_BELOW_LEADER_RATIO:-}` and its own
comment says the operator's 70% "lives in the inherited login shell". That
variable is **not set** in `~/.zshrc`, `~/.zprofile`, `~/.bash_profile`,
`~/.profile`, any `~/civvis-*.sh`, any LaunchAgent plist, or
`~/.civvis-climb-extra-args` (which holds `--campus-specialist
--refresh-seconds 0`). So #2319 is merged, correct, and inert on this machine
until somebody exports it.

## The premises the commissioning task carried, audited

Three of four do not survive contact with the corpus. They are recorded here
because acting on any of them would have produced a repair against a number
that is not there.

**"`docs/AI_GAPS.md` records that the war gates read POWER, which is why war
freezes expansion."** Not in that file. Zero case-insensitive matches for
`POWER` in 1,085 lines; no section on war freezing expansion. Nor is there a
passage about the economy starving while the army doubles — the file's own
deployment table at lines 766-783 records the opposite shape, an army that
*shrinks* (749.7 against 1,030.0) while gold collapses (216.0 against 727.6).
The POWER claim traces to a memory note, not to this repository. The nearest
thing in tracked source is `src/ai/advanced.rs:18982-18985`, which names the
`civvis-civ6-all-army-no-economy` failure as the reason
`WARTIME_ARMY_CEILING` exists.

**"Roughly 46% of city-turns go to settlers."** The corpus says **6.8%**
(19,014 of 280,026 city-turns), and 13.6% in the most recent window. Military
units take 18.0% and civilian units 18.4%.

**"About 213 settle-veto refusals per run."** The host-side `found_refused`
event runs at **0.5 per run**. The number that matches the claim is the
decider's own journal veto, at **61 per run** corpus-wide and **135 per run**
in the 08-19 window — a different counter with a different meaning, and the one
worth acting on (see below).

**"The army is 2-5× larger than any rival's while at peace."** True as a
*strength* ratio and misleading as a spending claim. `state.military` is the
host's strength aggregate. At peace it reads mean 1.58, median 1.23, p90 2.58
against the best rival; 19.9% of runs average 2× or more, and the peak within a
run is 2.67 at the median. By **unit count against the agent's own target**, the
decider's own arithmetic — parsed out of 592 explained turns of the replay,
*"the empire holds N military for M cities against a target of T each"* — reads
**1.04 held per city against a target of 1.00 per city**, median 0.89. The
empire is at its target, slightly under it at the median. The strength ratio is
rivals being weak, not us being fat.

**"Walls escalate through Castle to Star Fort."** Walls are held on 41.6% of
city-turns, a Castle on 6.6%, a Star Fort on 1.8%; 2.3% of city-turns are spent
building any wall. The escalation exists and is rare.

## The peacetime army target and the wall trigger

Named for the next reader, since the task's name for the first one does not
exist:

- **`peacetime_war_floors` is not a gene in this repository.** The rows that
  size the peacetime army are `army-target-weighs-enemy` (ledger **off**),
  `peacetime-deterrence` (ledger **on**) and `war-economy` (ledger **off**,
  vetoed by its pooled `win_diff_pp` of −0.78 despite reading +2.35 pp at
  z +7.50 on the new standard screen).
- The peacetime target is `city_count`, multiplied by
  `clamp(strongest_met_major_power / ours, 1.0, 1.5)` in
  `enemy_weighted_army_target` (`src/ai/advanced.rs:18998`), with
  `PEACETIME_DETERRENCE_CEILING = 1.5`. **Against an observed 1.04 held per
  city, a cap on this target would bind on nothing.** The reading does not
  support a fix and none was made.

The wall trigger is a different story, and it produced two defects that are
recorded here rather than fixed, because both sit behind a ledger-off gene and
belong in a PR that owns `src/ai.rs`:

1. **`src/ai.rs:9355` is unreachable.** `besieged_city_item` computes
   `bleeding = self.garrison_under_fire && city.hp < 200` and returns `None`
   three lines later when `!bleeding`. The next guard is
   `if self.garrison_under_fire && !bleeding {`, whose condition is therefore
   provably false. The garrison-hold early-out inside it — the one its own
   ★★★★ doc block says answers the run where Rome held five units and this path
   built a Warrior on t15 and again on t21 — **never runs.**
2. **The same doc block's claim about the frozen controllers is false.** It
   says "the frozen controllers keep the raid test as it was". With
   `garrison_under_fire` off, `bleeding` is false for every city and the
   function returns `None` before reaching any raid test, for every caller. On
   the deployment genome, where `garrison-under-fire` is ledger-off, this means
   the besieged-city wall-and-defender path is **entirely dead**, and
   `redirect_unsafe_city_queue_for_defense` returns immediately as well.

Neither changes deployment behaviour today — that is the point of them being
behind an off gene — but (1) makes the gene untestable as documented, so a
future screen of `garrison-under-fire` would be pricing something other than
what the comment describes.

## `governor-victory-lanes` is not the common cause: measured

The gene reads **−4.73 pp at win z −15.37** on the standard 23,622-seat screen
(`docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`) while
shipping `default_on: true`, and it is the only gene in `docs/gene_ledger.json`
carrying `conflict: true` (win z +2.46 against share z −15.92). Because it
routes the four victory lanes through `advanced_production` — where the
`+320 walls if threatened` and `+210 threatened` terms live — it is a plausible
upstream cause of exactly the misallocation this round is about.

A third paired replay, same eight runs and same 768 turns, two binaries from
the same HEAD differing only in `enable_governor_victory_lanes` setting the
flag `false`:

> **0 of 768 turns differ. Every order in both arms is identical.**

The flag was **probed, not assumed**: a temporary
`probe_governor_victory_lanes` field added to the genome line printed `false`
in the patched arm, and `true` after the patch was reverted in that same
worktree and rebuilt. The binaries differ (distinct MD5s) and the flag is what
differs.

The mechanism explains it. The routing gate at `src/ai/advanced.rs:31550` is

```rust
if (self.governor_in_recovery && plan.strategy == GrandStrategy::Recovery)
    || active_victory_target.is_some()
    || adaptive_expansion_dispatch
    || self.war_plan.is_some()
    || every_lane
```

and the live bridge always passes `--victory <lane>`, so
`active_victory_target.is_some()` is already true and `every_lane` is
redundant. The gene's second site — the Trader reservation at `:18750` —
produced no difference either.

Two consequences:

- **It cannot be the common cause of the live economy starvation**, because at
  the live seat it does nothing at all.
- **Flipping its default off costs the live seat nothing.** Its measured harm,
  if it is real, is a native-screen effect where the agent chooses its own
  lane. That makes the default flip a cheaper decision than it looked.

⚠ Bounded by the replay's own limit: a fresh agent per turn cannot see an
effect that only accumulates across a `--serve` session's plan continuity.

## The settle veto is fog, not loyalty — with the number

**356** of the 560 runs carry a decider journal, and **76 of them vetoed at
least one site**. Across those 76, 4,639 settle-site vetoes (13 per journalled
run, 61 per run that vetoed at all):

| reason the site was refused | count | share |
|---|---:|---:|
| *"ground the seat has not explored"* | 3,715 | **80.1%** |
| *"a rival city it has never seen"* | 438 | 9.4% |
| an arithmetic Loyalty forecast on ground it can see | 486 | 10.5% |
| anything else | 0 | 0.0% |
| **fog, both kinds** | **4,153** | **89.5%** |

In the 18 most recent vetoing runs the fog share is **92.2%** and the rate is
135 vetoes per run.

And the ground is taken: of **1,653 distinct sites vetoed**, **979 (59.2%) had a
rival city standing within three hexes at some later turn** than the veto. (The
journal prints axial and Civ 6 offset in one bracket; the census inverts the
odd-r offset with a conversion its tests pin against four of those printed
pairs.)

So the recorded conclusion holds and now has a magnitude. **The binding
constraint is mid-game exploration, and the size of the prize is the 80.1%
bucket: 3,715 of 4,639 vetoes, 49 per run that vetoed at all, resolving to
about 22 distinct sites per such run, of which roughly 13 are ground a rival
subsequently settled.** A softer veto is not the lever — only 10.5% of refusals are the
veto's arithmetic at all.

## What was decided

- **No production code changed.** Two defects in `src/ai.rs::besieged_city_item`
  are recorded above for a PR that owns that file; both are behind a ledger-off
  gene, and fixing them here would put a hotspot edit into a measurement PR.
- **No army cap.** The observed headcount is 1.04 per city against a target of
  1.00; the reading does not support a fix and inventing one would be a
  valuation tune, which this repository has measured as not paying.
- The census and the paired replay ship as `tools/live_repair_census.py`, so
  every number above is re-derivable, and so the next merged live repair can be
  checked against 560 games instead of one.

### The verdicts

| PR | verdict | why |
|---|---|---|
| **#2278** Great Person half | **VERIFIED** | 0 → 2,522 predicate flips over 3,341 GP unit-frames in 8 games; 199 of 768 replayed turns change orders; the needs machinery fires 17 → 308 and buys +234 city-turns of Great-Work capacity. One of its four escape hatches (the work sale) still never fires. |
| **#2278** Trader half | **VERIFIED IN PRINCIPLE, UNOBSERVED IN ORDERS** | 113 pairings across 72 runs were retired on a single refusal and are handed back; but no replayed turn changed a `TRADE_ROUTE`, the corpus rate is 0.20 pairings per run against the motivating run's 23, and the "100% refused once" statistic is forced by the old rule and is not evidence the refusals were transient. |
| **#2316** | **STILL UNVERIFIED** | The guard fired 0 times in 768 recorded turns and `FOUND_CITY` is 46 in both arms. No counter-evidence; no occasion. |
| **#2319** | **VERIFIED as a behaviour; NOT ARMED** | Fires on 36 of 560 runs at median turn 104, skipping 103 turns each; 0 of the 36 was ever won, against 9 wins in the corpus. Its risk of discarding a winnable game is bounded at 0/36. It is switched off at every layer and its env var is unset on this machine. |

### What would settle what is left

Each of these is a live run, and the operator halt is in force, so they are
written down rather than started.

1. **#2316.** One live 250-turn attempt with `--explain`, then
   `grep -c 'loyalty-doomed fallback' why.log`. A single non-zero count
   verifies the guard; a zero count across ten attempts (about 2,500 live
   turns, against the 768 replayed here) should retire it as unreachable rather
   than leave it standing as untested code.
2. **#2278's Trader half.** One live attempt, then
   `python3 tools/live_repair_census.py --since <that day> --section traders`.
   The reading that verifies it is a pairing appearing **twice** in
   `trade_route_refused` — impossible under the old rule and therefore
   diagnostic — or `final_idle_capacity` falling below the corpus mean of 0.89.
3. **#2319.** Export `CIVVIS_RESTART_BELOW_LEADER_RATIO=0.70` in the
   supervisor's login shell first; it is doing nothing until somebody does.
   Then the value question needs the thing the corpus cannot supply: the
   result of the *replacement* games. Twenty ladder attempts with the policy
   armed against twenty without, comparing mean score-at-250 per wall-clock
   hour rather than per attempt, is the smallest honest form of it.
4. **`governor-victory-lanes`.** The default flip is a native-screen decision
   and is already being confirmed elsewhere; this round only removes the live
   seat as a reason not to.
5. **The exploration lane.** The measurement that would size a repair before it
   is written: replay the same 768 turns with a build whose settle-site veto
   treats unexplored ground as neutral instead of refusing it, and count how
   many of the 22 distinct sites per run come back into the candidate list.
   That is instrument B with a one-line arm and costs two builds.

## Power, and what this round cannot resolve

- The Great Person reading rests on **eight** runs, because eight is how many
  in the entire corpus carry the field the repair reads. Within them the effect
  is not marginal — 2,522 of 2,525 cultural observations flip, and 199 of 768
  turns change orders — so the question it answers ("does the predicate change
  what the agent does") is settled. The question it does not touch is whether
  the change wins games.
- The corpus spans 2026-08-02 to 2026-08-19 and the deployment genome moved
  repeatedly across it. Every cross-arm comparison here is a *paired replay of
  the same recorded frames*, which is immune to that drift; every *rate* quoted
  from the census is not, and the 08-19 window is reported beside the whole
  corpus wherever it differs materially.
- **Nothing in this round is a value reading.** Not one number here is a win
  rate, an Elo, or a score delta, and none of them should be quoted as one.
