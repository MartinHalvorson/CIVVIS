# The `advanced_v1` re-pin log

Every change that reached `src/ai.rs` or `src/ai/advanced.rs` under the frozen
rating anchor, and the argument for why it did not change what the anchor
plays. It was the paper trail behind `ELO_PROTOCOL_VERSION` and the ledger in
`docs/EVAL.md`; the ledgers and the tournament harness were removed on
2026-08-23 (#2357), and the anchor pin itself stays — `ANCHOR_BEHAVIOUR_FNV`
in `src/main.rs` still hashes what `AdvancedAi::legacy()` plays, because a
change that moves it is a change that reached production without a gene in
front of it. New re-pins still belong here.

## Why it lives here and not in `src/main.rs`

It lived there — 1,371 lines of doc comment on one constant — until 2026-08-17.
That constant was an FNV hash of every byte of both AI files, so any edit to
either, **including a typo fix in a comment**, moved it and had to be re-pinned
with a paragraph explaining why the change was free.

Measured over the thirty days to 2026-08-17: **248 of about 1,669 merged pull
requests rewrote that constant**, at a rising rate (14% on 08-04, 40% on 08-15,
48% on 08-16, 6 of 6 on 08-17), and **173 of the 359 commits that touched
`main.rs` at all touched nothing but this comment**. Both edits landed at a
fixed point, so concurrent pull requests conflicted structurally rather than
occasionally — a serialisation point in the middle of a fleet merging a hundred
changes a day.

The byte hash is gone. `ANCHOR_BEHAVIOUR_FNV` now pins what `advanced_v1`
**does**: every action it applies across five profiles, from a two-player 20x14
duel to the six-player 54x34 deployment shape, plus an archipelago for the
embarkation paths a land map never reaches. A default-off addition cannot move
it, because the anchor never reads one — which is the claim every entry below
makes in prose, now checked by playing the games.

Entries are kept because they are the record of what was argued at the time. New
entries belong here, appended, and no longer need to accompany a constant edit:
**if `advanced_v1` still plays the same game, the test stays green on its own.**
An entry is still worth writing when a change reaches the shared AI files and
you want the reasoning on record.

---

`advanced_v1` freezes the planning configuration, but deliberately shares
the production Basic/Advanced implementation. Pin those sources so a code
edit cannot silently change the longitudinal anchor. If an edit reaches
the legacy path, bump the Elo protocol and start a new ledger; if it is
provably gated away, review that fact before updating this guard.

Recomputed after removing default-off experiments that had no whole-game
proof. The fingerprint covers the whole of both files, so any future edit
must either pass the fixed-prefix compatibility check or advance the Elo
protocol and start a new ledger.

#660 subsequently adds only default-off evaluator fields and a disabled
production prepass. `AdvancedAi::legacy()` leaves those gates off; the merged
source contract was re-pinned only after its fixed-prefix behavior check.

#672 adds two more default-off adaptive-Expansion flags and observer-only
action telemetry. With all flags false, a matched release-mode `ai_eval
advanced basic --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1`
comparison against pre-#672 `374e0f0` had identical scores, sweeps, seat
metrics, victory mix, and strategy-transition counts across 40 Advanced
seat-games and 4,022 observed Advanced turns. The only added report is a
zero-valued telemetry block, so this fingerprint is deliberately re-pinned
rather than changing the Elo protocol.

#673 similarly adds an empty `BeliefState` and a default-off pressure arm.
A clean `2f3dcb7` release build and this branch both produced the identical
19/20 Advanced game wins, scores, sweeps, seat metrics, victory mix, and
strategy-transition counts across 40 Advanced seat-games and 4,022 observed
Advanced turns on that same fixed prefix. The re-pin is justified because
the arm never observes or contributes a nonzero term while its flag is off.

#686 repairs a legacy settlement path: a passable natural wonder must not
remain a settler target when `Game::can_found_city` will always reject it.
Because that behavior is live for `advanced_v1`, the Elo protocol advances
to v3 and the source contract is recomputed with the new ledger rather than
being treated as a default-off compatibility re-pin.

#697 lands the Civilization VI bridge. It adds `forget_unit_memory` and
`remap_unit_memory` to both agents and a `settle_ranking` wrapper, none of
which the play path calls — only `civvis_orders --fresh-board` and
`civvis-advise` do — and one guard inside `settle_sites` that skips a site in
`Game::blocked_city_sites`. That set has exactly one production writer,
`mirror.rs`, which fills it from a host engine's refusals; `Game::new` and
`From<GameSer>` both leave it empty and nothing in an ordinary game ever
inserts into it, so the guard cannot fire outside the bridge. Measured on the
fixed prefix the re-pins above use — `ai_eval advanced basic --pairs 10
--players 4 --turns 200 --seed 31337 --jobs 1`, release, this branch against
`main` at `81636d9` — the two reports are **byte-identical**: 19/20 game wins,
95.0% paired-map score, 9 sweeps and 1 neutral, and every seat metric equal
across 20 games and 2,310 turns. Default-off compatibility re-pin; the Elo
protocol does not move.

#704 widens `PolicySpec::replaces` from `Option<Name>` to a list, because
Civilization VI's `ObsoletePolicies` lets one card retire several. The only
edit inside this anchor is the obsolete-card scan adapting to the new type:

```text
- .filter_map(|policy| policy.replaces.clone())
+ .flat_map(|policy| policy.replaces.iter().copied())
```

**`data/policies.json` is untouched by that PR**, so every `replaces` still
deserializes to exactly one name, and a `filter_map` over `Option` and a
`flat_map` over a one-element `Vec` collect the identical set. The obsolete
set, and therefore every policy decision downstream, cannot differ — this is
a type change, not a behaviour change. Confirmed on the same fixed prefix,
release, this branch against `main` at `1d8567b`: the two `ai_eval` reports
are **byte-identical**. Compatibility re-pin; the Elo protocol does not move.

#682 adds fog-aware campaign and battlefront observation to the live
controller, but `AdvancedAi::legacy()` explicitly disables every new
branch. A clean `41c02c0` release build and this branch produced identical
output from `ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337
--players 4 --turns 200 --deployment-comparison`: all 20 game results,
scores, sweeps, seat metrics, victory mix, and strategy-transition counts
matched across 40 Advanced-v1 seat-games and 4,300 observed Advanced-v1
turns. The source contract is deliberately re-pinned after that direct
compatibility check rather than changing the Elo protocol.

#684 adds one default-off evaluator field, `AdvancedAi::plan_city_target`,
and the delegated-call substitution it gates. With the flag false the
substitution is a `bool::then` that returns `None` and touches nothing, so
this is a compatibility re-pin and not a protocol change. It is earned the
way the entries above are: a matched `ai_eval advanced basic --pairs 10
--players 4 --turns 200 --seed 31337 --jobs 1` on a clean `origin/main`
build and on this branch, compared in full.

#719 freezes that live battlefront observation at the start of a major
turn, including camouflage detection. `advanced_v1` still disables the
observation path. Clean before/after release builds produced byte-identical
output from the same 10-map deployment comparison as #682: 16/20
`advanced_v1` game wins, 80.0% paired-map score, six sweeps, and 4,264
observed Advanced-v1 player-turns. The contract is re-pinned because the
gated legacy path did not move; the Elo protocol does not change.

The Civ VI bridge also needs to begin a route for an idle Firaxis Trader
whose normal walking movement is zero. `start_zero_movement_trader_route`
sits behind a default-off bridge flag enabled only by `civvis_orders` before
`Ai::take_turn`; `advance_unit_serial`, which is the native tournament loop,
is unchanged. The new code cannot run in an `advanced_v1` tournament game,
so its historical agent and Elo protocol remain unchanged. Re-pin the source
contract to make that reviewed exception explicit.
#746 promotes the confirmed policy/envoy composite only through the public
production constructors. `AdvancedAi::legacy()` still calls `configured`
directly and cannot reach that wrapper. A release build of `e46d1b7` and a
separately targeted release build of this change produced byte-identical
`ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 --players 4
--turns 200 --deployment-comparison` reports. This is a compatibility
re-pin, not an Elo protocol change.

#757 filters only the production controller's coordinated tactical threat
score behind `battlefront_observation`; `AdvancedAi::legacy()` keeps that
gate off. Clean `1c93908` and candidate release reports from the same
10-map deployment comparison were byte-identical after Cargo's build
prelude (SHA-256
`e37ae6f3014c6f13c75ef964027e7b57f5e57e9289f0fdb36cae80f5bb863341`).
This is a compatibility re-pin, not an Elo protocol change.

#761 parallelizes only live policy-card counterfactual scoring. The pool
exists only in `AdvancedAi::fleet_parallel`; `AdvancedAi::legacy()` keeps
`work_pool` at `None`, so its ancillary pass selects the literal serial
scorer. The new `QueryMemo` is confined to an unchanged read-only policy
valuation and drops before a card is changed. Clean `cb7969d` and candidate
release builds produced byte-identical SHA-256 reports from `ai_eval
advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 --players 4 --turns 200
--deployment-comparison` (20 games and 4,264 observed Advanced-v1 turns).
This is a compatibility re-pin, not an Elo protocol change.

#762 bounds that same production-only card scorer to four worker-private
snapshots even when its persistent fleet pool is wider. The `None` branch
used by `AdvancedAi::legacy()` still chooses the literal serial scorer.
Clean `0b04a59` and candidate release builds produced byte-identical
reports from the same 20-game deployment comparison (SHA-256
`932cfabf125e729a5264ce43d2fd8b05d013d3fe84939b1dcd366ff122ddc84a`).
This is a compatibility re-pin, not an Elo protocol change.

#766 bounds only the live controller's clone-heavy purchase-menu batch to
three workers. `AdvancedAi::legacy()` has no `work_pool` and continues to
select the literal serial action enumeration. Clean `8812d36` and candidate
release builds produced byte-identical reports from the same 20-game
deployment comparison (SHA-256
`932cfabf125e729a5264ce43d2fd8b05d013d3fe84939b1dcd366ff122ddc84a`).
This is a compatibility re-pin, not an Elo protocol change.

#782 repairs runaway military production and wartime science/culture
neglect only for the live controller. Every new strategic branch is gated
by `victory_planning`; `AdvancedAi::legacy()` sets that flag false. The
regression test checks the legacy yield weights and production choice as
well as the live behavior. The source contract is deliberately re-pinned;
the Elo protocol does not move.

#786 adds live Delegation/Embassy, Defensive Pact, Joint War, promise, and
demand decisions to the shared Basic and Advanced diplomacy paths. The
frozen `advanced_v1` controller invokes that shared path, so the combined
source contract intentionally moves to protocol v4 with a fresh ledger.

#801 makes compiler-equivalent `BasicAi` cleanup only: redundant clones of
`Copy` values and needless references become direct values, a periodic
modulo test becomes `is_multiple_of`, a candidate tuple gains a name, and
unused mutability goes away. It changes no choice condition, score, or
iteration/action ordering. This was checked rather than inferred: clean
`e3481e4` and candidate release builds produced byte-identical reports from
`ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 --players 4
--turns 200 --deployment-comparison` (SHA-256
`f6d9e17ee19fe298e14a573f97a896280a75a767306dca6ef0d80d2020384b2c`).
This is a compatibility re-pin, not an Elo protocol change.

#799 adds the live settlement-site intelligence and visible transit-risk
gates to the shared Advanced source. `AdvancedAi::legacy()` explicitly
keeps the historical settlement scorer and disables those gates, so the
source contract is deliberately re-pinned without moving the Elo protocol.
#802 adds a settle-scoring adjacency term gated behind
`AdvancedAi::adjacency_site_planning` — on in `promoted_policy_envoy`,
off in `configured()`, so `AdvancedAi::legacy()` never evaluates it.
Checked the same way: baseline and branch builds produced byte-identical
`ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed
31337` reports. Another compatibility re-pin over the merged sources,
not an Elo protocol change.

#808 records the planner's peace-offer decisions (`peace_offers`, a
BTreeSet written at the offer site) and exposes them on `PlanReport`,
which is observer-only by contract — nothing in play reads the field.
Checked the same way: baseline and branch builds produced byte-identical
`ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed
31337 --jobs 1` reports. Another compatibility re-pin over the merged
sources, not an Elo protocol change.

#838 bundles already-shared purchase-scoring inputs and replaces only
`Copy`/iterator idioms in the Advanced source. The control on `main` and
this branch produced byte-identical `ai_eval advanced_v1 basic --pairs 10
--players 4 --turns 200 --seed 31337 --jobs 1 --deployment-comparison`
reports. This is therefore a compatibility re-pin, not an Elo protocol
change.

#840 adds population-four settlement forecasting, bounded travel,
stalled-route recovery, and land escorts to the shared Advanced source.
Every decision path is gated by `settlement_safety`, which
`AdvancedAi::legacy()` disables. Clean `bc58acb` and candidate builds
produced identical `ai_eval advanced_v1 basic --pairs 10 --players 4
--turns 200 --seed 31337 --jobs 1 --deployment-comparison` reports:
18/20 Advanced wins, 119.2 average turns, and identical terminal tables.
#848's progress-versus-motion tracker is now merged into the same path, but
it returns through the prior code whenever `settler_commit` is disabled, as
it is in `AdvancedAi::legacy()`. This is another compatibility re-pin, not
an Elo protocol change.

The live Civ VI mirror's purchase-placement regression moves only a unit in
a `cfg(test)` fixture. The compiled AdvancedAi implementation is unchanged;
this is therefore another reviewed compatibility re-pin.

#896 keeps repeatable economic projects out of the live controller's
wartime land-force gap. The new branch is gated by `victory_planning`,
which `AdvancedAi::legacy()` disables. Clean `f114601` and candidate
release builds produced byte-identical stdout from `ai_eval advanced_v1
basic --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1
--deployment-comparison`: 16/20 Advanced-v1 wins, 125.9 average turns, and
identical terminal and strategy-transition tables. Compatibility re-pin;
the Elo protocol does not change.

#879 replaces `filter(...).next()` with `find(...)` in one `cfg(test)`
fixture and removes an unnecessary `mut` from another. Neither change is
part of a compiled controller, so Advanced-v1 behavior and the Elo protocol
remain unchanged. The source contract is deliberately re-pinned for this
reviewed test-only diff.

#927 adds escort progress accounting behind `linked_settler_progress`,
which is false for configured and legacy engine agents and enabled only by
the live `civvis_orders` bridge. Engine and Elo trajectories therefore keep
their prior behavior; this is a compatibility re-pin, not a protocol bump.

#929 retries asynchronous Firaxis governor postings behind
`live_governor_assignment_adapter`. Configured and legacy engine agents keep
it false; only `civvis_orders` enables it. Compatibility re-pin, not an Elo
protocol change.

#911's escorted-settler correction remains behind `settlement_safety`,
which `AdvancedAi::legacy()` disables. The live religious-purchase guard
added afterward is likewise default-off in `BasicAi` and enabled only by
`civvis_orders`; its focused test preserves the historical rival-faith
purchase with the flag off. A matched 10-map, 20-game deployment comparison
had identical substantive output across 3,801 observed Advanced-v1 turns.
These are compatibility re-pins, not an Elo protocol change.

#930 adds `besieged_military_floor`, which lets a city under visible siege
raise its standing-army floor against hostiles the empire has no diplomatic
state with (Barbarians are excluded from `at_major_war` by design, so every
defensive escalation in `pick_item` previously read a barbarian siege as no
threat at all). It sits behind `siege_muster`, a `BasicAi` flag that is
false in both constructors and is enabled only by the live `civvis_orders`
bridge, so configured, legacy and Elo agents keep their prior behavior —
`besieged_military_floor` returns 0.0 before reading the board when the flag
is off. Unconditional, the same change perturbed
`oracle::tests::the_modernity_grant_actually_fires`; gated, the full suite
is unchanged. A compatibility re-pin, not a protocol bump. The same flag
also gates `besieged_city_item`, which lets a city with a raiding party at
its gates build walls or a defender ahead of the ordinary build order; both
entry points return early on `!siege_muster` before reading the board.

#933 bounds the defensive-war Recovery posture behind `bounded_recovery`, a
field that is `false` in `AdvancedAi::new()` and set only by
`civvis_orders`. `recovery_is_stale` short-circuits on that flag before it
reads the plan or the clock, so `assess` returns the identical strategy for
configured, legacy and Elo agents. Headless confirmation: 24 paired seeds
with the flag on and off are byte-identical on score, cities and the
strategy census, because the sim reaches Recovery on only 8% of
strategy-turns against the live ladder's 86%. A compatibility re-pin.

#934 re-keys that same bound from the plan's age to the WAR's age
(`major_war_since`). Still behind `bounded_recovery`, still short-circuiting
on the flag before anything is read, so every configured, legacy and Elo
agent is byte-identical. Another compatibility re-pin.

#955 adds `home_defense_objective`, which lets a raider standing in our own
territory claim a unit before the offensive does. Gated behind the new
`home_defense` flag, which is `false` in BOTH `BasicAi` constructors and is
turned on only by the Civilization VI bridge — exactly the contract
`siege_muster` runs under. `home_defense_objective` short-circuits on that
flag before it reads anything, so every configured, legacy and Elo agent is
byte-identical and `the_default_controller_keeps_home_defense_off` asserts
it on a board that DOES yield an objective once enabled. A compatibility
re-pin.

#955 also adds `garrison_assignments`/`garrison_step`, which put a unit on a
threatened city's own tile. Behind the SAME `home_defense` flag, short-
circuiting on it first, so this too is a compatibility re-pin.

#962 gates faith military purchases on whether the empire can pay the GOLD
upkeep the soldier incurs. Behind the new `solvent_faith_army` flag, which is
`false` in `AdvancedAi::configured` (so `new()` and `legacy()` both have it
off) and is turned on only by the Civilization VI bridge —
`faith_military_is_affordable` returns `true` immediately on that flag, so
every configured, legacy and Elo agent buys exactly what it always did.
`the_default_controller_keeps_the_faith_army_ungated` asserts it on a board
that DOES refuse once enabled. A compatibility re-pin.

#957 prices a fogged objective city from this controller's last sighting
inside `local_strength_ratio`, behind `blind_objective_strength` — again
`false` in `AdvancedAi::new()` and set only by `civvis_orders`.
`remembered_objective_strength` returns `None` on that flag before it
touches the belief state, and the fallback is only reachable at all once
`battlefront_observation` is on. `advanced_v1` sets that to `false`, so the
legacy anchor takes the same `Some(g.city_strength(city))` arm it always
took and is byte-identical twice over. A compatibility re-pin.

#963 sizes the siege train against the target city's standing wall, behind
`siege_tracks_the_wall` — `false` in `AdvancedAi::new()` and set only by
`civvis_orders`. `siege_units_wanted` returns the shipped
`usize::from(plan.target_city.is_some())` on that flag before it reads the
board, so the legacy anchor's production value is bit-for-bit what it was.
A compatibility re-pin.
#819 routes `BasicAi::tactical_step` and the Advanced force mover through
`path_move` so a unit stepped twice in one turn cannot reverse its own
first step, behind `recorded_tactical_step` — `false` at both `BasicAi`
construction sites and set only by `civvis_orders`. Unlike the flags
above, this one guards a call that can *refuse*: `path_move` rejects a
reversal, a retread, or a minor leaving its defense area where the raw
`g.apply(Move)` would have moved. `tactical_apply_move` therefore returns
the historical raw apply on the flag before it reaches `path_move` at all,
so `advanced_v1` takes byte-for-byte the arm it always took. A
compatibility re-pin.
#1727 lets a coordinated Advance/Engage unit that has proven a multi-turn
livelock use its A* route through one recorded square. The exception still
keeps same-turn reversal and all normal movement guards, and requires
`recorded_tactical_step`, which the frozen anchor never enables. A
compatibility re-pin.

#974 adds a `Cities/Decision` journal line to `advanced_production`, which
had none. It is inside `if self.journal().wants(Decision)` and writes only
to the reasoning journal — no board state is read or changed, and the
legacy anchor's chosen item is bit-for-bit what it was. A compatibility
re-pin.

#965 promotes wide, developed, defended expansion only in the production
constructor: it enables call-local city/Builder floors, plan delegation plus
the three existing defense flags, and lets that flagged plan consume the
land-aware nine-city ceiling. Stored genomes, `configured`, and
`AdvancedAi::legacy()` retain the historical weights; the controls also keep
their three-city floor, six-city ceiling, flat delegation, and default-off
defense fields. The focused production/control contract test asserts each
side of that boundary. This is therefore a compatibility re-pin for
`advanced_v1`, not an Elo protocol change.

#976 adds `AdvancedAi::enable_live_bridge` (the eight bridge flags in one
place, so a headless arm can play the deployed agent) and three
`disable_*` methods that hold one flag off for a measurement arm. Nothing
calls either from `new()` or `legacy()`, so every configured, legacy and Elo
agent is byte-identical. A compatibility re-pin.

#958 prices research outside the victory lane, behind `research_economy`.
`advanced_v1` is `AdvancedAi::legacy()`, which goes through
`AdvancedAi::configured` and therefore has that field `false`; only
`promoted_policy_envoy` turns it on. The identity is exact rather than
sampled, and it holds in two ways at once:

- the Campus coverage bonus, the peacetime Campus-building debt and the
  policy-deck insertion are each guarded by `if self.research_economy`, so
  for the anchor they are not evaluated at all;
- the three weight terms are floors — `science.max(self.research_weight)` in
  `yield_value`, `yield_weights.science.max(..)` in the search evaluator, and
  `ys.science.max(research_tilt)` in `lane_emphasis`. For an agent without
  the flag, `refresh_research_weight` writes `0.0` and the tilt argument is
  `0.0`, and every value being floored is already non-negative (the lane
  science weights run 1.0-4.2, the evaluator's 0.5-2.8, the emphasis 0.0 or
  0.50). A floor at zero over a non-negative quantity is the identity, so
  this is provably byte-identical and not merely measured to be.

A compatibility re-pin.


#977 raises the wartime army target when the enemy outweighs us, behind
`army_target_weighs_the_enemy` — `false` in `AdvancedAi::new()` and set only
by `civvis_orders`. `wartime_army_target` returns its `shipped` argument
unchanged on that flag before it reads a single player, so every configured,
legacy and Elo agent wants exactly the army it always wanted. A
compatibility re-pin.

#1056 skips policy cards that multiply a suzerainty count of zero, behind
`suzerain_cards_need_a_suzerainty`, `false` in `AdvancedAi::new()` and set
only by `enable_live_bridge`. `strategic_policies` reorders nothing on that
flag before it counts a single city-state, so every configured, legacy and
Elo agent picks exactly the deck it always picked. A compatibility re-pin.

#981 adds `BasicAi::loyalty_emergency`, which ranks loyalty trouble by TURNS
TO FLIP rather than by level, behind the new `loyalty_rate_alarm` flag. The
flag is `false` in both `BasicAi` constructors and `loyalty_emergency`
returns the old level-only answer on it before reading any rate, so every
configured, legacy and Elo agent behaves identically. A compatibility re-pin.

#984 credits a movement tile for the attack it opens, behind
`strike_opening` — `false` in `AdvancedAi::new()` and set only by
`enable_live_bridge`. `strike_opening_value` returns 0.0 on that flag
before it reads the board, so every configured, legacy and Elo agent scores
every tile exactly as it did. A compatibility re-pin.

#990 adds four `disable_*` methods so every flag in `enable_live_bridge` has a
measurement arm. They are called only by `builtin_ai`'s `live_without_*`
factories, never in play, so every configured, legacy and Elo agent is
byte-identical. A compatibility re-pin.

#989 adds a journal line for a DECLINED attack and a diagnostic tally of the
reasons the forward model refuses one. Both are behind
`journal().wants(Detail)` or write only to a process-local census; no board
state is read and no decision changes, so every configured, legacy and Elo
agent attacks exactly what it always did. A compatibility re-pin.

#991 makes a ranged unit prefer a movement tile it can actually see the
target from, behind `ranged_needs_line_of_sight` — `false` in
`AdvancedAi::new()` and set only by `enable_live_bridge`.
`ranged_tile_is_blind` returns `false` on that flag before it reads the
board, so every configured, legacy and Elo agent scores every tile exactly
as it did. A compatibility re-pin.

#999 gives the research chooser a goal for a Campus building the empire is
already equipped for but cannot reach, behind `research_economy`. That field
is `false` in `AdvancedAi::configured`, and `unreachable_science_building_tech`
returns `None` on it before reading the board at all, so `advanced_v1` picks
the technology it always picked. A compatibility re-pin.

#1003 lets the baseline governor build an Entertainment Complex when the
host reports the city paying the Amenity band, behind
`BasicAi::amenity_districts`. That field is `false` in both `BasicAi`
constructors and is set only by `AdvancedAi::promoted_policy_envoy`; the
added block short-circuits on it before reading the board, so `advanced_v1`
ranks the same four district families in the same order. A compatibility

The siege-role branch adds `best_military_role`, `siege_is_the_missing_arm`
and a `missing_siege_arm` term on the army floor, all behind the new
`siege_role` flag. It is `false` in both `BasicAi` constructors and every
new path short-circuits on it before reading anything, so every configured,
legacy and Elo agent picks exactly what it always picked. A compatibility
re-pin.

#1011 holds a promotion until its healing would land, behind
`promote_when_wounded` — `false` in `AdvancedAi::new()` and set by nothing
on the shipped paths (it is native/eval only and deliberately absent from
`enable_live_bridge`). `promotion_heal_is_wasted` returns `false` on that
flag before it reads a unit, so every configured, legacy and Elo agent
promotes exactly when it always did. A compatibility re-pin.

#954 says why a settler was held instead of only that it was marching. The
added block is inside `if !moved && self.journal().wants(Detail)` and every
call it makes is a read: `Game::route_step` and `route_step_to_any` take
`&self`, as do `can_move`, `units_at` and `wdist`, and `think!` writes to the
reasoning journal, which is observer-only by contract. No RNG is drawn and no
board state is touched, so the anchor plays the identical game and only its
journal differs. A compatibility re-pin.
#1026 keeps the land army out of the water, behind `come_ashore` — `false`
in both `BasicAi` constructors and set only by `enable_live_bridge`. Every
one of its paths short-circuits on the flag before reading anything:
`explore_step`'s `dry_only` and `step_toward_range`'s and
`coordinated_tactical_step`'s `prefer_dry` are each `come_ashore && …`, both
`disembark_step` call sites are guarded by `if …come_ashore`, and
`peacetime_step`'s new `at_war` parameter is folded through
`at_war && self.come_ashore`, which reproduces the historical hardcoded
`false` exactly. So every configured, legacy and Elo agent explores and
moves exactly as it always did. A compatibility re-pin.

#1087 lets the baseline governor raise the housing ceiling — the Aqueduct
and the Neighborhood — behind `BasicAi::housing_districts`. That field is
`false` in both `BasicAi` constructors and is set only by
`enable_live_bridge`; the added block in `pick_item` short-circuits on it
before it reads a city, so `advanced_v1` ranks the same district families in
the same order. `Game::city_housing` is refactored onto `city_water` and
`city_housing_floor` without changing a single band, so the housing it
returns is unchanged for every caller. A compatibility re-pin.

#1095 keeps asking for a Campus in every city that can still repay one,
behind `AdvancedAi::campus_every_city` — `false` in the constructor and set
only by `enable_live_bridge`. Both of its paths short-circuit on the flag:
`balanced_core`'s exemption is `campus_every_city && family == "campus"`,
which is `false` for every legacy agent and reproduces the half-empire cliff
exactly, and the coverage term keeps `research_horizon` unless the flag is
set. So `advanced_v1` prices every district exactly as it did. A
compatibility re-pin.

#1099 puts `medina_quarter` and `insulae` in the deck when a city is short
of housing, behind `AdvancedAi::housing_cards` — `false` in the constructor
and set only by `enable_live_bridge`. The block short-circuits on the flag
before it reads a city, so every legacy and Elo agent slots exactly the cards
it always slotted. `Game::city_specialty_district_count` only widens from
private to `pub(crate)`. A compatibility re-pin.

⚠ Re-pinned twice in this PR. The first version was inert — it patched
`BasicAi::tactical_step`, which a live probe showed the deployed controller
never calls; the working change is in
`AdvancedAi::coordinated_tactical_step`. Both edits touch anchored source,
so both moved this hash.

⚠ And again for `blind_objective_units`, which is `false` in both `BasicAi`
constructors' downstream `AdvancedAi` defaults and set only by
`enable_live_bridge`. `local_strength_ratio`'s new term is
`if self.blind_objective_units { … } else { 0.0 }`, so with the flag off the
sum is arithmetically identical to before. A compatibility re-pin.

⚠ And a third time on merging `origin/main`, which had re-pinned the same
constant for the `tactical_strategy` branch documented below. Neither hash
survives a merge of the two — the anchored source is now different from
both — so the value here is the one the test computes over the merged tree.
Both gating arguments still hold independently, which is what makes the
re-pin a compatibility one rather than a ledger break.
The tactical-role branch adds class assignments, projected return-fire,
wall/support coordination, and cavalry action priority behind
`BasicAi::tactical_strategy`. Both Basic constructors leave it `false`, and
`AdvancedAi::promoted_policy_envoy` alone enables it for the production
controller, so frozen Basic, configured, legacy and `advanced_v1` entrants
retain their old branches. A compatibility re-pin.

⚠ And again for a warning fix. `science_goal_for_campus` bound a building's
name it never read; the loop now iterates the map's values. The anchor
hashes whole files, so a change that cannot alter behaviour still moves it.
That this one cannot was checked rather than argued: the same
`BTreeMap<String, BuildingSpec>` in the same key order, with the binding the
compiler proved unused removed. Seed 1002 was then played to completion on
both revisions through the same routes — turn 206, player 4, religious, all
six scores equal (Arabia 994, Aztec 592, Ethiopia 651, Georgia 1012, Khmer
706, Maya 464), and the same 254 requests to get there. A compatibility
re-pin.
⚠ And again for `relief_targets_the_siege`, which is `false` in every
`AdvancedAi` default and set only by `enable_live_bridge`. Its whole effect is
the leading component of one `min_by_key` in `domain_objective`, and with the
flag off that component is the constant `0` — so the ordering, and therefore
every objective any legacy or Elo entrant receives, is bit-for-bit what it was.
A compatibility re-pin.

⚠ And again for the pantheon price, which now reads `Game::pantheon_faith_cost()`
instead of a bare `25.0` in the `ai.rs` gate. `pantheon_faith_cost` is
`game_speed.scale(PANTHEON_FAITH_STANDARD)`, and `GameSpeed::default()` is
`Standard`, whose `cost_percent` is 100 — so at the speed every legacy and Elo
entrant plays, the expression evaluates to exactly `25.0` and the gate is
bit-for-bit what it was. Only Online, Quick, Epic and Marathon move, and those
were charging a price the game does not.
⚠ The value below is recomputed over the MERGED sources: main re-pinned this
constant for its own change while this branch was open, so neither side's
number is right after the merge — only a fresh fingerprint is.
`elo_anchor_speed_is_standard_so_the_pantheon_repin_is_free` checks the
Standard-speed claim rather than asserting it.
⚠ Re-pinned for the unified timed-war appointment. The behavior is behind
`AdvancedAi::timed_war`, initialized `false` by `configured` and enabled
only by the evaluator-only `AdvancedAi::timing_attack` constructor. Every
shared call site short-circuits on an absent `war_plan`; frozen legacy and
`advanced_v1` therefore retain the same research, spending, production,
diplomacy, movement, and upgrade decisions. Focused construction tests
additionally assert that `advanced` reports the treatment off.
⚠ Re-pinned for selective timing v2. Its additional chooser and launch
gates require both `timed_war` and `selective_timed_war`; both initialize
`false`, and only the evaluator-only `selective_timing_attack` constructor
enables them. The typed-arm test checks production `advanced`, v1, and v2
independently, while focused tests cover the selective-only branches.
⚠ Re-pinned again for ready-force v3. `rapid_timed_war` also initializes
`false`, is enabled only by the evaluator constructor, and only narrows the
already-gated chooser before a `WarPlan` exists.
⚠ And again for `settler_blocked_turns` surviving a retarget. That reset lives
AFTER `advanced_settler_step`'s `if !self.settler_commit { return moved; }`
early return, and `settler_commit` is `false` in every default constructor —
only `civvis_orders` turns it on for the live bridge. So the legacy and Elo
entrants return before the changed line is ever reached and the anchor's
behaviour is bit-for-bit what it was. A compatibility re-pin;
`elo_anchor_never_reaches_the_settler_commit_path` checks the claim.
⚠ Re-pinned for test-only seeded-map fixture hardening after Natural
Wonder silhouettes changed. Both edits are inside `#[cfg(test)]` modules;
no controller path is compiled into an Elo game.
⚠ Re-pinned for production unit-objective memory. The full objective,
danger, and retreat path is behind `BasicAi::unit_objective_memory`, which
initializes false in Basic and `AdvancedAi::legacy()` and true only in the
production Advanced constructor. The focused regression test asserts that
split and the production assignment; the frozen anchor never takes either
new movement branch.
⚠ #1162 routes the charged Toa, Legion, and Nau through shared improvement
planning. `AdvancedAi::legacy()` and `BasicAi` can now select real new
improvement actions, so this is deliberately a protocol-v6 change rather
than a compatibility re-pin; the fresh source fingerprint documents that
the new ledger starts from this exact shared controller.
⚠ 2026-08-04 prunes default-off experiments whose measured effects were
negative, inert, or inconclusive. A fixed-prefix `advanced_v1`/`basic`
comparison remains the compatibility check; this is a deliberate re-pin.
#1034 pulls the loyalty policy cards when a city is bleeding loyalty, behind
`loyalty_policy_defence` — `false` in `AdvancedAi::new()` and set only by
`enable_live_bridge`. `strategic_policies` reads the flag before it counts a
single city, so with it off the wishlist is byte-for-byte the old one and
every configured, legacy and Elo agent slots exactly the cards it always did.
A compatibility re-pin.

#1195 bounds only the live controller's global settlement-site search.
`AdvancedAi::legacy()` keeps `settlement_safety` disabled, so it returns
through the historical full-search path and the frozen `advanced_v1`
controller remains byte-identical. Compatibility re-pin; the Elo protocol
does not move.

#1204 makes action-family queries skip unrelated enumeration and removes
duplicate production-catalog work from the purchase-only projection.
`AdvancedAi::legacy()` retains the same action ordering and the BasicAI
purchase helper is outside the frozen controller's path. Clean `origin/main`
and candidate release builds produced byte-identical output from
`ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
--jobs 1 --deployment-comparison`. Compatibility re-pin; the Elo protocol
does not move.

#1206 keeps the live settlement-growth beam's at-most-four selected plots
inline instead of allocating a `Vec` for each candidate branch.
`AdvancedAi::legacy()` keeps `settlement_safety` disabled, so the changed
forecast is outside the frozen controller's path. Clean `origin/main` and
candidate release builds again produced byte-identical output from
`ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
--jobs 1 --deployment-comparison`. Compatibility re-pin; the Elo protocol
does not move.

#1209 partitions settlement-growth forecast layers at the existing beam
width before sorting the survivors. `AdvancedAi::legacy()` keeps
`settlement_safety` disabled, so the changed forecast is outside the frozen
controller's path. Clean `origin/main` and candidate release builds again
produced byte-identical output from `ai_eval advanced_v1 basic --pairs 10
--players 4 --turns 200 --seed 31337 --jobs 1 --deployment-comparison`.
Compatibility re-pin; the Elo protocol does not move.

#1217 reuses exact raw settlement-site values across the live controller's
local and global radius scans. The radius-specific penalties are still
applied at each call site, and a fixed-prefix `ai_eval advanced basic`
comparison produced byte-identical output on clean main and the candidate.
Compatibility re-pin; the Elo protocol does not move.

#1225 reuses the tile appeal computed by one `worthwhile_improvements` call
across that tile's candidate improvements. A fixed-prefix `ai_eval
advanced basic --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1
--deployment-comparison` comparison produced byte-identical output on
clean main and the candidate (SHA-256
`34c8ccea34d4bf3a8b60ae1b713f82bffbce77a5f1614f07d69db591d6287b24`).
#1227 stops the live religious buyer purchasing a Missionary into a tile that
already holds one of our religious units — the host refuses it outright with
"Too many units of the same class in this location.", 799 times across the
08-04/08-05 runs. The guard is gated on `live_religious_purchase_guard`, like
the majority-religion check beside it, so the frozen controller is untouched:
`ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
--jobs 1 --deployment-comparison` produced BYTE-IDENTICAL output from this
worktree with the change stashed and applied (same base, same build profile).
Compatibility re-pin; the Elo protocol does not move.

#1232 shares the radius-two position disk between settlement growth
forecasting and adjacency scoring inside one visible site valuation. The
legacy controller leaves the settlement-safety path disabled. A fixed-prefix
`ai_eval advanced basic --pairs 10 --players 4 --turns 200 --seed 31337
--jobs 1 --deployment-comparison` comparison produced byte-identical output
on clean main and the candidate (SHA-256
`34c8ccea34d4bf3a8b60ae1b713f82bffbce77a5f1614f07d69db591d6287b24`).
Compatibility re-pin; the Elo protocol does not move.

#1241 moves the friendly-city ownership check ahead of the static movement
predicate in `BasicAi::patrol_tile`. The predicate is unchanged; the new
order rejects the overwhelmingly common unowned map tile before asking the
traversal cache, so the frozen controller's source contract is re-pinned
after the fixed-prefix comparison below.
#1259 guards the special-improver helper at its call site. The guard repeats
the helper's existing eligibility checks, so the advanced_v1 legacy path is
unchanged; the source contract is re-pinned after the fixed-prefix
comparison above.
The Advanced parallel unit planner now primes frontier-post scans inside the
immutable batch snapshot, keyed by traversal class, so each worker reuses
the same read-only map scan without publishing it across a world mutation.
The fixed-prefix output remains byte-identical; compatibility is re-pinned.
`run_game` now also switches the narrated war ledger's per-action re-sync
off beside fog memory. The ledger is observation-only — no rule and no
built-in agent reads it, and declarations, peaces, and turn boundaries
still sync unconditionally — so the frozen controller's decisions are
unchanged and the source contract is re-pinned.
`faith_building_spending` now skips building the purchase menu whenever
the faith bank is below the reserve — the state in which the existing
filter provably rejects every candidate. Identical purchases in every
reachable state, so the frozen controller's decisions are unchanged and
the source contract is re-pinned.
`culture_focus` is removed from `BasicAi`: both constructors pinned it
`false` and nothing else ever set it, so its production blocks and the
`project_matches_focus` helper were unreachable — dead weight, not
behaviour. No reachable decision changes; the source contract is
re-pinned.
#1297 lets the strongest MET major weigh on the army target in PEACETIME,
behind `peacetime_deterrence` — `false` in `AdvancedAi::new()` and set only
by `enable_live_bridge`. `enemy_weighted_army_target` (the renamed
`wartime_army_target`; the wartime term is untouched) multiplies `shipped`
by 1.0 on that flag before it reads a single player, so every configured,
legacy and Elo agent wants exactly the army it always wanted. A
compatibility re-pin.
The `BasicAi` doc comment claiming CIVVIS "ordered an Entertainment
Complex zero times" is corrected — the census's name filter missed the
unique replacements (7 stood across 33 cities, as Hippodromes and a
Street Carnival) — and a test pins that a unique replacement belongs to
the family it replaces. A doc comment and one test; no executable path
changes, so advanced_v1 is byte-identical by construction. Compatibility
re-pin.
#1360 adds a bounded friendly-volley extension only under
`BasicAi::tactical_strategy`. `AdvancedAi::legacy()` leaves that flag false,
so its unit loop never asks for a paired friendly finisher or replaces a
reply price; this is a reviewed compatibility re-pin, not an Elo-protocol
change.
#1363 restores the joint tactical planner (`joint_tactics`, off everywhere
but the `advanced_joint_tactics` arm and the live bridge) and admits the
barbarian seat to the Advanced military step's enemy list behind
`home_defense`. `AdvancedAi::legacy()` leaves `home_defense` false and
`joint_tactics` false, so the anchor's path gains only inert fields and a
set-membership test against an empty set; the STOCK `advanced` entrant
(which ships `home_defense = true`) now answers barbarian raiders at home,
recorded here honestly. Compatibility re-pin for the anchor.
The Tactics arena adds an arena doctrine to both files, every part of it
behind `Game::is_arena()` — which is false for every world a rated game is
ever played on, because the Battlefield script is not a world and no
rating instrument accepts one. With the flag false the three touched
expressions reduce to the shipped ones by construction (`arena || x` is
`x`, `!arena && y` is `y`, and the weight pair is returned unchanged), so
the anchor's decisions are byte-identical. Reviewed compatibility re-pin,
not an Elo-protocol change.
#1386 makes production Scouts collect a tribal village they can currently
see and reach before another unseen exploration tile. The shared branch is
behind `BasicAi::tactical_strategy`: it is false for Basic and
`AdvancedAi::legacy()`, and `promoted_policy_envoy` enables it for the
production controller. The condition tests that flag before reading player
sight, reachability, or village state; the frozen path therefore proceeds
directly into its historical fog-target selection. The focused regression
asserts that split on the same staged board. A matched release
`ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
--jobs 1 --deployment-comparison` report was byte-identical to the
then-current `origin/main` (SHA-256
`1bebbaa15ee7388b3d9427c1d49726d8e29b2328113c9b9409cb60bb7ae813e0`).
Compatibility re-pin, not an Elo-protocol change.
#1384 teaches the joint planner withdrawals and handoff steps and keeps the
per-unit movers off units the plan moved without a blow
(`tactics_withdrawn`). `AdvancedAi::legacy()` leaves `joint_tactics` false,
so the plan never runs, the set stays empty, and the anchor's only new
executable is a set-membership test against an empty set — the same shape
#1363 re-pinned. Compatibility re-pin over the #1382 merge.
#1393 adds the war-conversion trio (`war_economy`, `war_reinforcement`,
`war_patience`), off by default and set only by `enable_live_bridge`.
`AdvancedAi::legacy()` leaves all three false, so its new executable is
three short-circuiting flag tests: the production routing keeps its
historical arms (`false && _` adds nothing), both fatigue sites reduce to
the shipped expression (`!false &&` is identity), and
`wartime_reinforcement_step` returns `None` on its first line. The anchor's
decisions are byte-identical by construction. Compatibility re-pin, not an
Elo-protocol change.
#1399 breaks the Tactics-arena standoff by switching two pieces of
world-preservation logic off on a battlefield: the per-tile
local-superiority brake on closing moves, and the dangerous-approach
memory whose retreat floor assumes healing that an arena does not have.
Both sit behind a `!g.is_arena()` test, and `is_arena()` is false for
every world a rated game is played on, so the anchor's decision stream on
the rated profile is identical by construction — the same shape as the
flag re-pins above, with the map script as the flag. Compatibility
re-pin, not an Elo-protocol change.
The recon-replacement arm adds one disjunct to `pick_item`'s military-floor
condition and one new chooser, behind `BasicAi::recon_replacement`. Both
constructors leave the flag false and only `enable_live_bridge` sets it, so
under the anchor `recon_is_the_missing_arm` returns on its first line, the
added disjunct is a constant `false` that cannot change the `||`, and
`best_recon` is never reached. The anchor's build order is byte-identical by
construction. Compatibility re-pin, not an Elo-protocol change.
#1401 discounts a motionless Settler from the expansion gate's in-flight
test, so one stuck settler stops costing every future one. It sits behind
`BasicAi::settler_strand_discount`, which `AdvancedAi::enable_live_bridge`
sets and nothing else does — `BasicAi::new` and `with_weights` both leave
it false, and both new tests assert the off path is unchanged, so the
anchor's decision stream is identical by construction. Compatibility
re-pin, not an Elo-protocol change.
#1402 counts mirrored `UNIT_SPY` units as espionage agents in the spy
capacity test. A native CIVVIS Spy is a `Game::spies` entry and never a
unit — the production arm returns before `place_new_unit` — so the unit
census contributes 0 to `spy_agents` in every native game and the anchor
sees the same number it always did. Identical by construction, on a rated
profile and off it. Compatibility re-pin, not an Elo-protocol change.
#1404 adds the missing `disable_stranded_settler_discount` counterpart so
the treatment can be held off for a controlled arm. It only writes `false`
into a field the anchor already reads as `false`. Compatibility re-pin, not
an Elo-protocol change.
The siege-commitment term adds one summand to `campaign_city_value`, behind
`AdvancedAi::siege_commitment`. Both constructors leave the flag false and
only `enable_live_bridge` sets it, so under the anchor the `&&` chain
short-circuits on its first test, the term is `0.0`, and the returned score
is the shipped expression minus zero — the anchor's campaign ordering is
byte-identical by construction. Compatibility re-pin, not an Elo-protocol
change.
The wonder-ring settle credit (#1378) adds one term to
`settle_value_visible`, behind `BasicAi::wonder_ring_settle_value`. Both
constructors leave the flag false and only `enable_live_bridge` sets it,
so under the anchor `natural_wonder_ring_value` returns 0.0 on its first
line and the added `value += 0.0` cannot move any site score — and the
anchor reaches `settle_value_visible` only through constructors that keep
`settlement_safety` true, which `legacy()` does not. The anchor's settle
ordering is byte-identical by construction. Compatibility re-pin, not an
Elo-protocol change.
#1401 discounts a motionless Settler from the expansion gate's in-flight
test, so one stuck settler stops costing every future one. It sits behind
`BasicAi::settler_strand_discount`, which `AdvancedAi::enable_live_bridge`
sets and nothing else does — `BasicAi::new` and `with_weights` both leave
it false, and both new tests assert the off path is unchanged, so the
anchor's decision stream is identical by construction. Compatibility
re-pin, not an Elo-protocol change.
#1402 counts mirrored `UNIT_SPY` units as espionage agents in the spy
capacity test. A native CIVVIS Spy is a `Game::spies` entry and never a
unit — the production arm returns before `place_new_unit` — so the unit
census contributes 0 to `spy_agents` in every native game and the anchor
sees the same number it always did. Identical by construction, on a rated
profile and off it. Compatibility re-pin, not an Elo-protocol change.
#1404 adds the missing `disable_stranded_settler_discount` counterpart so
the treatment can be held off for a controlled arm. It only writes `false`
into a field the anchor already reads as `false`. Compatibility re-pin, not
an Elo-protocol change.
#1405 gives the baseline governor's building sort a housing term, behind
`BasicAi::housing_buildings`. The field is `false` in both `BasicAi`
constructors and set only by `enable_live_bridge`, and `housing_lift`
returns 0.0 whenever it is off, so the comparator is the identity it always
was on the anchor. Compatibility re-pin, not an Elo-protocol change.

Merging `origin/main` into this branch brings both sides' live-bridge
treatments into one `BasicAi`/`AdvancedAi`. Every one of them is off in both
constructors and set only by `enable_live_bridge`, so the anchor's decision
stream is unchanged by the union. Compatibility re-pin, not an Elo-protocol
change.
The capture-the-flag objective gives both controllers one new march: land
columns aim at the enemy's flag, via `Game::arena_enemy_flag`. It returns
`None` unless the battle was set up with flags — a shape that exists only
on a Tactics arena that asked for it, and on no world any rated game has
ever been played on — so both guards reduce to a `None` test and the
anchor's decision stream is identical by construction. The same shape as
the #1399 re-pin, with the objective as the flag. Re-pinned when the
objective became a flag per side taken from the enemy, rather than one
neutral flag raced for; the guard's shape did not change, only what it
returns on the arena that has flags at all. Pinned over the merge with
main's own re-pins, every one off in both constructors as their entries
above record. Compatibility re-pin, not an Elo-protocol change.
The garrison-walls arm adds one guarded branch to `pick_item` and its
chooser, behind `BasicAi::garrison_walls`. Both constructors leave the flag
false and only `enable_live_bridge` sets it, so under the anchor
`garrison_walls_item` returns `None` on its first line and the branch can
never take the build. The anchor's build order is byte-identical by
construction. Compatibility re-pin, not an Elo-protocol change.
⚠ **The war-eve liquidation is NOT a free re-pin.** Every entry above ends
with a flag both constructors leave false; this one has no flag at all.
`BasicAi::war_eve_liquidation` runs from the shared `diplomacy` pass and
from `AdvancedAi`'s ordinary declaration, so the `advanced_v1` anchor really
does sell its cancellable promises before it declares, and its Gold, army,
and the victim's treasury all move. `ELO_PROTOCOL_VERSION` is bumped to 7
with this pin; see its own note for what stops comparing.
The settlement atlas reuses static site terms only while an active
battlefront frame and the live settlement-safety controller are present.
`AdvancedAi::legacy()` disables both `battlefront_observation` and
`settlement_safety`, so it stays on the historical uncached settlement
path. The production `advanced` controller does use the atlas, but the
frozen `advanced_v1` anchor cannot observe it. Compatibility re-pin, not
an additional Elo-protocol change.
The disposable speculative branch likewise changes only hypothetical
worlds: the fixed `advanced_v1`/`basic` prefix (`ai_eval advanced_v1 basic
--pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1
--deployment-comparison`) remains byte-identical. Its source contract is
re-pinned below; the Elo protocol does not move.
The Holy Site figure moved onto `Weights::advanced()`, which only
`AdvancedAi::new()` reads. `AdvancedAi::legacy()` builds from
`BasicAi::new()` and therefore from `Weights::default()`, which still pays
`d_holy` 2.0 — the anchor keeps the exact weights it always had, and
`holy_lane_parity` is a flag both constructors leave false. Compatibility
re-pin, not an Elo-protocol change.
The district-weight guard and the roster-composite notes are a test and
doc comments in the hashed sources; no constructor moved and
`AdvancedAi::legacy()` is untouched. Compatibility re-pin.
`diplomatic_opening` is a flag both constructors leave false and
`diplomatic_opening_score` returns 0 without it, so the anchor never
reaches the new lane term. Compatibility re-pin.
`AdvancedAi::new()` is back on `Weights::default()`, which is the
weights `AdvancedAi::legacy()` always used, so the anchor is where it
has always been. Compatibility re-pin.
Doc comments only in the hashed sources: two of them claimed native
games leave `bounded_recovery` disabled when `promoted_policy_envoy`
enables it. No constructor moved and `legacy()` is untouched.
Compatibility re-pin.
A test doc comment only: the evidence ledger for what
`promoted_policy_envoy` enables. No constructor moved and
`legacy()` is untouched. Compatibility re-pin.
The production city-target floor was removed from
`promoted_policy_envoy`. `AdvancedAi::legacy()` builds through
`configured`, never that constructor, and its floor was and remains
3 — pinned by the same test that caught this. Compatibility re-pin.
Three new `disable_*` withholds so every production flag can be
priced. They are evaluator entry points that no constructor calls;
`AdvancedAi::legacy()` never had these flags on. Compatibility
re-pin.
`enable_engine_repairs` and its war/economy halves, so the live-bridge
repair bundle can be priced natively. They are evaluator entry points
reached only by the three `advanced_synergy*` arms in `builtin_ai`;
no constructor calls them, `AdvancedAi::legacy()` builds through
`configured` and never had one of these flags on, and the addition is
purely additive — 115 lines, no deletions. Compatibility re-pin,
asserted rather than asserted-by-comment in
`the_repair_bundle_cannot_reach_the_frozen_anchor`.
Seven production category genes, and `production_value` multiplied by the
one that matches each candidate. Every gene defaults to 1.0 and the
multiply is applied only to a positive score, so a default genome — which
is what `AdvancedAi::legacy()` and `BasicAi::new()` both carry — ranks
builds bit-identically.

⚠ This re-pin is **measured, not argued**. The list above contains one
justified by a comment that was wrong, so this one was checked against the
tree it claims to preserve: `ai_eval advanced advanced_v1
--deployment-comparison --players 4 --pairs 12 --turns 150 --seed
91000000` was run on a build of `a2c8c7f` and on this branch, and the two
outputs are byte-identical across 24 games and 5,712 bytes of diagnostics —
including `advanced_v1`'s own per-seat cities, score, military and victory
types. Compatibility re-pin.
Two further `disable_*` withholds for base-constructor defaults
(`settlement_safety`, `battlefront_observation`). Evaluator entry points no
constructor calls; `legacy()` already turns both off. Compatibility re-pin
recomputed over the merged tree — neither side's value applies.
A test only: `the_withholdable_defaults_are_off_on_the_anchor_and_on_in_production`,
which asserts the claim the re-pins above made in prose — that no
withhold arm for a production default can reach `AdvancedAi::legacy()`.
Compatibility re-pin, and the last one on this branch that needs the
argument, because the assertion now carries it.
`settler_founds_when_stalled` and its `founds_where_it_stands`
branch. The flag defaults false in the struct init both `legacy()`
and `new()` build from, and the branch returns immediately without
it — asserted, not argued, in
`the_repair_bundle_cannot_reach_the_frozen_anchor`, which this
change extends. Compatibility re-pin.
`fortify_idle_units` and the `hold_stood_down_unit` branch that reads
it. Evaluator-only: the flag defaults false in the `BasicAi` init
both `legacy()` and `new()` build from, and the branch keeps its
original stand-down condition without it — asserted in
`the_repair_bundle_cannot_reach_the_frozen_anchor`, which this
change extends. Compatibility re-pin.
⚠ **First city-state discovery is NOT a free re-pin.** The production
Scout's high-information frontier chooser is guarded by `tactical_strategy`,
which `AdvancedAi::legacy()` leaves off, but the corresponding first-contact
Envoy is a `Game` rule. Any controller can earn it by seeing a city-state,
so its influence thresholds and downstream choices differ in a native game.
`ELO_PROTOCOL_VERSION` is bumped to 8; the source contract is re-pinned for
the separately reviewed, legacy-gated Scout source edit.
`with_legacy_policy_deck` plus two comment corrections. The new
constructor is an evaluator entry point no other constructor calls,
and `AdvancedAi::legacy()` never routed through `production_weights`
so its deck was and remains `Legacy` — pinned by
`the_policy_deck_is_live_in_production_and_legacy_on_the_anchor`.
Compatibility re-pin.
`production_builder_floor` and the `delegated_cities` branch reading
it. The whole block is already behind `if !self.plan_city_target`,
which `AdvancedAi::legacy()` leaves false, so the anchor never
reaches it — and the flag defaults true so production is unchanged.
Compatibility re-pin.
`production_settler_deadline` and its `delegated_cities` branch, the
last production-only override to get a withhold. The whole block is
behind `if !self.plan_city_target`, which `AdvancedAi::legacy()`
leaves false, and the flag defaults true so production is unchanged.
Compatibility re-pin.
#1522 gates the Conquest wartime economy on a concrete objective:
`offensive_conquest` (a target city, a threatened city, or an active
major war) now decides the 2x-cities military target, the production
ceiling buffer, and the +160/+120 Conquest production bonuses; an
objective-less Conquest plan keeps the ordinary garrison. Measured on
the fixed prefix — `ai_eval advanced_v1 basic --pairs 10 --jobs 1
--seed 31337 --players 4 --turns 200 --deployment-comparison`, ci
profile, this branch against `main` at `5df102c4` — the two reports
are **byte-identical**: 85.0% paired-map score, 7 sweeps / 3 neutral,
17/40 vs 3/40 seat wins, every metric equal across 20 games averaging
131.9 turns, with conquest plans live on 14.1% of anchored all-game
seat-turns — so wherever the anchor's planner went Conquest, the gate
resolved the same wartime package as before. Compatibility re-pin;
the Elo protocol does not move.
The 2026-08-14 war-half removal: `promoted_policy_envoy` stops setting
`siege_muster`, `home_defense`, `tactical_strategy` and
`unit_objective_memory`, plus the alias declarations and doc updates
that ride with it. `AdvancedAi::legacy()` never routed through
`promoted_policy_envoy` and `BasicAi::new()` constructs all four flags
false, so the anchor's behaviour is unchanged; the four flags are now
false in production too and set only by `enable_live_bridge` (two of
them, as the `siege-muster`/`home-defense` treatments) and the
`advanced_war_half` re-addition arm. Compatibility re-pin; the Elo
protocol does not move.
Live strategic targeting now excludes unintroduced mirror seats behind
`battlefront_observation`. `AdvancedAi::legacy()` holds that flag false, so
both new predicates short-circuit to their historical forms; the focused
regression asserts that boundary. Compatibility re-pin; the Elo protocol
does not move.
The same live-only observation gate keeps a stale major-war defense from
becoming a counter-campaign at less than half the rival's power. The
frozen anchor's false gate preserves its historical denial path, asserted
in the regression. Compatibility re-pin; the Elo protocol does not move.
The live-only peacetime-deterrence gate now converts its raised defender
target into city queues before adaptive Science can refill them with
projects. `AdvancedAi::legacy()` leaves that gate false; the regression
asserts both the frozen project and the live defender. Compatibility re-pin;
the Elo protocol does not move.
The peaceful city-plan handoff is likewise unreachable to the frozen
anchor: its call site requires `victory_planning`, and its own gate is
`plan_city_target`, both false in `AdvancedAi::legacy()`. The regression
holds the anchor's research grant while the live plan replaces it with one
Settler. Compatibility re-pin; the Elo protocol does not move.
The named live-Great-Person gate is a host-only fact in
`Player::live_great_person_offer_blockers` and
`Player::live_great_person_offers`. `Game::new` and old saves leave the
latter `None`, making `great_person_class_offered_now` accept the native
roster; only `mirror.rs` writes Firaxis's current offer set. The assertion
below locks that boundary, so the source-contract re-pin does not silently
alter headless `advanced_v1` tournament rows. Compatibility re-pin; the Elo
protocol does not move.
The severe-Amenity project handoff is false for `AdvancedAi::legacy()` and
becomes live only through `enable_live_bridge` (or an explicit engine-repair
evaluation arm). The frozen `advanced_v1` controller retains its project
queues, so this is a compatibility re-pin rather than an Elo protocol move.
The related Liberalism relief uses that same false-by-default gate before it
reads a city Amenity or policy deck: only a live controller with two
developed, host-observed deficit cities can trade Aesthetics for the
immediately paying card. `AdvancedAi::legacy()` cannot enter the branch, so
this is also a compatibility re-pin rather than an Elo protocol move.
The opening Scout, six-city fog floor, civilian policy timing, government
prerequisite, major-war zero-damage siege handoff, stalled-Settler founding,
and first-Campus Writing handoff changes are all behind live-bridge treatment
flags. `first_campus_tech` short-circuits on `campus_every_city` before it
reads the board, and `AdvancedAi::legacy()` leaves that flag false; the
focused ablation tests lock the boundary. Compatibility re-pin; the Elo
protocol does not move.
Physical Great People that have no host-valid activation plot now add
mirror-only production and research needs. An unfinished host activation
district is also a map foundation, which reserves that family before a
second Spaceport can be ordered. `Game::new` leaves the need list empty, old
saves default it empty, and only `mirror.rs` populates it from a Firaxis unit
export. The assertion below locks that boundary, so the frozen headless
anchor cannot enter any of the new planning branches. Compatibility re-pin;
the Elo protocol does not move.
A named live Great Engineer can ask for a wonder only while the host has not
already refused a wonder in that city. This circuit breaker reads the same
mirror-only activation need and host-refusal map, both empty in ordinary and
frozen games; other cities remain eligible. Compatibility re-pin; the Elo
protocol does not move.
The wartime maintenance-card handoff requires `war_economy`, a zero
treasury, and an active major war. `AdvancedAi::legacy()` leaves
`war_economy` false, so it cannot enter the new policy branch. Compatibility
re-pin; the Elo protocol does not move.
The live war-production solvency handoff is gated by that same
`war_economy` flag. `AdvancedAi::legacy()` leaves it false, so its recovery
chooser and every production queue remain unchanged. Compatibility re-pin;
the Elo protocol does not move.
The local-defense handoff is likewise live-only: `garrison_under_fire`
changes the emergency chooser from a generic military pick to a
melee-capable land defender, lets the queue release replace a siege piece,
lets it start a defender after clearing a host-owned queue, and spends Gold
on that immediate defense before upgrades, patronage, or the ordinary
purchaser can choose a Builder or preserve its strategic reserve.
`AdvancedAi::legacy()` also leaves `amenity_project_preemption` false, so
it never reads the host-calibrated Amenity ledger or reserves an idle Arena
queue. The stricter broad-wartime reservation uses that same gate before it
can inspect an idle or repeatable queue; every frozen constructor returns
before reading a city. Compatibility re-pin; the Elo protocol does not
move.
A fresh direct declaration likewise observes the timed-war endgame reserve
only when `endgame_war_runway` is enabled through the live bridge. The
frozen anchor leaves that flag false, retaining its historical late-war
behavior. Compatibility re-pin; the Elo protocol does not move.
A home barbarian now lets only unclaimed live-bridge units retain their
pre-war campaign staging after the bounded garrison/defense responders get
first claim. `AdvancedAi::legacy()` leaves `home_defense` false, so it
never inserts the barbarian seat into this path. The fixed 10-pair
`advanced_v1`/`basic` seed prefix (31337 through 31346) matched the prior
17/20 wins, 131.9 average turns, score, and per-seat metrics exactly.
Compatibility re-pin; the Elo protocol does not move.
The live wonder race is likewise gated by `live_wonder_race`, which only
`enable_live_bridge` sets: `AdvancedAi::legacy()` and every rated arm keep
the `Item::Wonder` refusal exactly as it was, so no headless anchor can enter
the new valuation branch. Compatibility re-pin; the Elo protocol does not
move.
The same live-only wonder race now closes during the empire-wide `Recovery`
posture, even if the individual building city is not yet threatened. The
frozen anchor leaves `live_wonder_race` false and cannot enter either
branch. Compatibility re-pin; the Elo protocol does not move.
A settler standing on a cached target that `can_found_city` now refuses
retires that target with the bounded avoidance the stall counter uses — behind
`settler_commit`, which `AdvancedAi::legacy()` leaves off
(`elo_anchor_never_reaches_the_settler_commit_path`); the re-validation of a
cached target also refuses `blocked_city_sites`, a set that is empty in every
ordinary and frozen game. Compatibility re-pin; the Elo protocol does not
move.
A new Settler target is forecast through the engine's Loyalty model only
while `loyalty_rate_alarm` is on. Both default constructors and the frozen
anchor leave that treatment flag false; the live bridge enables it with the
live Loyalty emergency handling. If every inspected target is immediately
doomed, the live controller holds rather than falling through to the
unaware baseline picker. Compatibility re-pin; the Elo protocol does not
move.
A missing siege/recon arm now owns a city queue only when that city can
actually build the requested role. Both `siege_role` and
`recon_replacement` remain disabled by `AdvancedAi::legacy()`, so the
frozen anchor retains its prior production path. Compatibility re-pin; the
Elo protocol does not move.
The second settler pipeline slot is behind `parallel_settlers`, which only
the Civilization VI bridge sets (`AdvancedAi::enable_parallel_settlers`); every
native constructor and `AdvancedAi::legacy()` keep the one-at-a-time gate in
both settler routes (asserted). Compatibility re-pin; the Elo protocol does not
move.
`war_patience` is now bounded by `WAR_PATIENCE_LIMIT_TURNS`; the flag is set
only by the live bridge and the native repair bundle, never by
`AdvancedAi::legacy()`, so the anchor's peace rules are unchanged.
Compatibility re-pin; the Elo protocol does not move.
The threat-aware guard wait lives inside `stacked_escort_pace`, behind
`stacked_escort`, which only the live bridge and the native repair bundle set;
`AdvancedAi::legacy()` never reaches it. Compatibility re-pin; the Elo
protocol does not move.
Naval units now count in `settlement_tile_risk` on coastal tiles, and a
threatened settler retreats before any hold; both live under
`settlement_safety`/`stacked_escort`, which `AdvancedAi::legacy()` leaves off.
Compatibility re-pin; the Elo protocol does not move.
The stalemate posture is behind `war_patience`, which `AdvancedAi::legacy()`
never sets; the anchor's grand-strategy selection is unchanged (asserted).
Compatibility re-pin; the Elo protocol does not move.
The generic Wonder fallback reads mirror-only `blocked_wonders`, which
ordinary/headless games never populate. Compatibility re-pin; the Elo
protocol does not move.
Live war patience now recognizes only an observed foreign city changing
hands, so a fresh settlement cannot prolong a stale war; the frozen anchor
never enables `war_patience`. Compatibility re-pin; the Elo protocol does
not move.
The hosted-amenity and regional-reach pricing is behind
`amenity_district_path`, which only the live bridge and the native repair
bundle set; `AdvancedAi::new()` and `AdvancedAi::legacy()` price the
Entertainment Complex exactly as before (asserted). Compatibility re-pin;
the Elo protocol does not move.
A live wonder race now rejects a data-marked religion-founding site after
its civilization has already founded a religion. `live_wonder_race` remains
false for the frozen anchor, so its historical wonder choices cannot enter
the new guard. Compatibility re-pin; the Elo protocol does not move.
The Prophet deferral is behind `expansion_before_prophet`, which only the
live bridge sets (Firaxis-only); `AdvancedAi::new()` and `legacy()` enter
the Prophet race with two cities exactly as before (asserted).
A battlefront-observing controller now requires a legal, known enemy city
before a Conquest denial may replace a stalled war's economic plan; raw
leader pressure remains available to Congress and in-lane counters, while
`advanced_v1` retains its historical all-information path (asserted).
The wonder lanes are behind `live_wonder_race`, which only the live bridge
sets; `AdvancedAi::new()` and `legacy()` refuse wonders exactly as before
(asserted).
Compatibility re-pin; the Elo protocol does not move.
(asserted). Compatibility re-pin; the Elo protocol does not move.
The host Settler population floor is behind `BasicAi::host_settler_pop`,
set only by the Civilization VI bridge; every native constructor and
`AdvancedAi::legacy()` keep the genome's `settler_min_pop` (asserted).
Compatibility re-pin; the Elo protocol does not move.
The elective-war stand-down is behind `no_elective_war`, which only the
live bridge sets (Firaxis-only); `AdvancedAi::new()` and `legacy()` take
the "strong enough" branch exactly as before (asserted). Compatibility
re-pin; the Elo protocol does not move.
The war-patience reference is read only under `war_patience`, which the
frozen anchor never sets. Compatibility re-pin; the Elo protocol does not
move.
A catastrophic multi-front Recovery peace proposal is also behind that
same live-only `war_patience` gate: with it false, the anchor keeps
protecting its active campaign target exactly as before. Compatibility
re-pin; the Elo protocol does not move.
The wonder-race scale is read only under `live_wonder_race`, which the
frozen anchor never sets. Compatibility re-pin; the Elo protocol does not
move.
The wall-tech research goal is behind `garrison_walls`, the live walls
doctrine, which the frozen anchor never sets (asserted). Compatibility
re-pin; the Elo protocol does not move.
A stalled Settler's known-hostile-frontier rejection is read only under the
live `loyalty_rate_alarm`; the frozen anchor cannot enter the guard, so its
historical fallback remains intact (asserted). Compatibility re-pin; the
Elo protocol does not move.
A cached settlement target's arrival forecast reads the same live-only
`loyalty_rate_alarm`; normal and frozen controllers retain their historical
cached-target founding behavior (asserted). Compatibility re-pin; the Elo
protocol does not move.
The exploration dead-target memory is behind `BasicAi::explore_dead_targets`,
set only by the Civilization VI bridge; native constructors and
`AdvancedAi::legacy()` keep the plain goal (asserted). Compatibility re-pin;
the Elo protocol does not move.
The foreign-border settle penalty is behind `settlement_safety`, which
`AdvancedAi::legacy()` leaves off (asserted). Compatibility re-pin; the Elo
protocol does not move.
The Amenity-repair band gate sits inside `amenity_districts`, which every
native constructor and the frozen anchor leave off. Compatibility re-pin;
the Elo protocol does not move.
The every-lane governor routing is behind `governor_every_lane`, which only
the live bridge and the native repair bundle set; `AdvancedAi::new()` and
`legacy()` keep the historical routing (asserted). Compatibility re-pin; the
Elo protocol does not move.
A plan-confirmed pre-damage barbarian siege reuses the live-only
`garrison_under_fire` gate; the frozen anchor never enables it and therefore
keeps its historical queue commitments (asserted). Compatibility re-pin; the
Elo protocol does not move.
The settler retreat limit lives inside the retreat step, behind
`stacked_escort`/`settlement_safety`, which `AdvancedAi::legacy()` leaves
off; carrying retired sites across rebuilds touches only unit-keyed memory.
Compatibility re-pin; the Elo protocol does not move.
The pre-declaration maintenance reserve is guarded by the live-only
`war_economy` flag, which the frozen anchor never enables. Its Conquest
portfolio therefore keeps the historical order until the live bridge opts
into the named-campaign recovery guard (asserted). Compatibility re-pin; the
Elo protocol does not move.
The value below is recomputed after both live-only changes are combined.
The garrisoned-city raid gate is behind `garrison_under_fire`, the live
doctrine that owns the besieged-city path; frozen controllers keep the raid
test as it was (asserted). Compatibility re-pin; the Elo protocol does not
move.
The adjacent-guard march is behind `stacked_escort`/`settlement_safety`,
which `AdvancedAi::legacy()` leaves off. Compatibility re-pin; the Elo
protocol does not move.
The fog-read city ceiling is behind `fog_land_capacity` under
`wide_map_capacity`, both live-only and off for `AdvancedAi::legacy()`
(asserted); a native board carries no unknown terrain, so the estimate
equals the count there. Compatibility re-pin; the Elo protocol does not
move.
The recon flight step is behind `recon_flight`, off for
`AdvancedAi::legacy()` (asserted); the frozen anchor's scouts explore
exactly as before. Compatibility re-pin; the Elo protocol does not move.
The embarked-settler sea link is skipped only under `stacked_escort`,
which `AdvancedAi::legacy()` leaves off (asserted); the frozen anchor
still links a ship to a settler at sea. Compatibility re-pin; the Elo
protocol does not move.
The turn-limit horizon on the space race and the nuclear lane is behind
`score_horizon`, off for `AdvancedAi::legacy()` (asserted); the frozen
anchor races and arms exactly as before. Compatibility re-pin; the Elo
protocol does not move.
The sea's recon arm — the one-ship purchase and the naval explorer — is
behind `naval_recon`, off for `AdvancedAi::legacy()` (asserted); the
frozen anchor's ships and production are unchanged. Its viable-waterway
and lake-bound-hull refinements remain behind that same gate. Compatibility
re-pin; the Elo protocol does not move.
The in-lane answer to a Science or score leader is behind
`counter_in_lane`, which the live bridge now enables and
`AdvancedAi::legacy()` leaves off (asserted); the frozen anchor still
declares. Compatibility re-pin; the Elo protocol does not move.
The era-paced city cadence is behind `era_paced_expansion`, off for
`AdvancedAi::legacy()` (asserted); the frozen anchor still adds a city
per ninety standard turns. Compatibility re-pin; the Elo protocol does
not move.
The tally price of culture is behind `tally_culture`, off for
`AdvancedAi::legacy()` (asserted); the frozen anchor's lanes keep their
bred yield weights and district table. Compatibility re-pin; the Elo
protocol does not move.
The frontier-loyalty settle rule is behind `frontier_loyalty`, off for
`AdvancedAi::legacy()` (asserted); the frozen anchor's settle forecast is
unchanged. Compatibility re-pin; the Elo protocol does not move.
The banked envoy and its final-tier, secure-suzerain marginal-return cap
are behind `bank_envoys`, and the committed outward exploration goal
behind `BasicAi::explore_commit`, all set only by the Civilization VI
bridge and off for `AdvancedAi::new()` and `AdvancedAi::legacy()`
(asserted); the frozen anchor spends every envoy and re-derives its
scout's goal each turn as before. Compatibility re-pin; the Elo protocol
does not move.
The settler-target hysteresis is behind `settler_target_hysteresis`, off
for `AdvancedAi::legacy()` (asserted); the frozen anchor's settler
re-picks exactly as before. Compatibility re-pin; the Elo protocol does
not move.
The tally price of a Great Person is behind `tally_great_people`, off for
`AdvancedAi::legacy()` (asserted); the frozen anchor's patronage keeps its
closeness limit. Compatibility re-pin; the Elo protocol does not move.
The frontier-loyalty rule is now a distance test (own city within nine
tiles), still behind `frontier_loyalty` and off for `AdvancedAi::legacy()`
(asserted). Compatibility re-pin; the Elo protocol does not move.
The barbarian-scout exemption in the settlement risk model is behind
`barbarian_scouts_are_scouts`, off for `AdvancedAi::legacy()` (asserted);
the frozen anchor prices every hostile as before. Compatibility re-pin;
the Elo protocol does not move.
The nine-tile camp reach is behind `BasicAi::camp_reach`, off for
`AdvancedAi::legacy()` (asserted); the frozen anchor's home guard keeps
the six-tile radius for camps and raiders alike. Compatibility re-pin;
the Elo protocol does not move.
The frontier-loyalty reach moves from nine to seven tiles, still behind
`frontier_loyalty` and off for `AdvancedAi::legacy()` (asserted).
Compatibility re-pin; the Elo protocol does not move.
The strategic governor's Expansion routing is behind `governor_every_lane`,
off for `AdvancedAi::legacy()` (asserted); the frozen anchor's baseline
still governs its Expansion lane. Compatibility re-pin; the Elo protocol
does not move.
The settler stack discipline (settlers decide before the engagement,
capture priced as capture, only a guard on the tile counts, bound guards
kept out of the joint plan) is behind `settler_stack_discipline`, and the
peacetime camp party (the whole field army answers home threats, a camp in
reach outranks the countryside, the party sized to the camp's defender)
behind `BasicAi::camp_party`; both off for `AdvancedAi::legacy()`
(asserted). Compatibility re-pin; the Elo protocol does not move.
`recon_is_the_missing_arm` counts a recon unit already in a city queue as
the arm being rebuilt (still behind `recon_replacement`, off for
`AdvancedAi::legacy()`), and `BasicAi::skip_opening_book` lets a decider
restarted mid-game leave the four-build book behind it — the frozen
anchor's opening is unchanged. Compatibility re-pin; the Elo protocol does
not move.
The live envoy bank gates both the plan-aware scorer and the later
`BasicAi` fallback, while `AdvancedAi::legacy()` keeps both historical
paths enabled. Compatibility re-pin; the Elo protocol does not move.
A looped reconnaissance target is retired only behind
`explore_dead_targets`, which the Firaxis order bridge explicitly enables;
`AdvancedAi::legacy()` keeps that flag off. Compatibility re-pin; the Elo
protocol does not move.
The idle Entertainment Complex reservation is behind
`amenity_project_preemption`, which both `AdvancedAi::legacy()` and the
stock constructor keep off (asserted in
`the_repair_bundle_cannot_reach_the_frozen_anchor`). Compatibility re-pin;
the Elo protocol does not move.
A repeatable district project waits behind the Library, University,
Research Lab or Workshop its city can already build, behind
`buildings_before_projects`, off for `AdvancedAi::legacy()` (asserted).
Compatibility re-pin; the Elo protocol does not move.
The live recon arm keeps a second Scout only after city two, still behind
`recon_replacement`, which `AdvancedAi::legacy()` leaves off. Its missing-arm
predicate therefore returns on the same first-line flag check in every
frozen game; the anchor's production decisions remain byte-identical.
Compatibility re-pin; the Elo protocol does not move.
A second already-built sea hull may explore only behind `naval_recon`, which
`AdvancedAi::legacy()` leaves off. The frozen anchor still gets an empty
explorer set before inspecting units, so its movement decisions are
byte-identical. Compatibility re-pin; the Elo protocol does not move.
The recon-flight loop escape is reached only from `recon_flight`; that
live-only flag is false in `AdvancedAi::legacy()`, so a frozen Scout keeps
its historical flight and exploration behavior. Compatibility re-pin; the
Elo protocol does not move.
The hostile-Suzerain peace path is reached only through `bank_envoys`,
which the Firaxis order bridge enables after profitable Envoy placements
have already run. `AdvancedAi::legacy()` keeps that gate false, so its
diplomacy remains historical. Compatibility re-pin; the Elo protocol does
not move.
The wartime second naval eye and its idle-city reservation are both reached
only through `naval_recon`, which `AdvancedAi::legacy()` leaves false.
Compatibility re-pin; the Elo protocol does not move.
The bounded Envoy liquidity reserve is reached only through `bank_envoys`,
false in `AdvancedAi::legacy()`. Compatibility re-pin; the Elo protocol
does not move.
A campaign-target Suzerain cannot make the peace needed for an Envoy
reclaim, so it no longer inflates that live-only liquidity reserve.
Compatibility re-pin; the Elo protocol does not move.
A major-war campaign now keeps its already chosen enemy city until capture,
a target change, or an emergency. That condition is inside
`siege_commitment`, which `AdvancedAi::legacy()` leaves false; the frozen
anchor therefore continues to refresh the city ranking as before.
Compatibility re-pin; the Elo protocol does not move.
The developed-city-state contact sweep's third Scout stays behind
`recon_replacement`, false in `AdvancedAi::legacy()`. Compatibility re-pin;
the Elo protocol does not move.
Patronage skips both a Great Person class the mirrored host reports
exhausted (`live_great_person_exhausted`, read through
`great_person_class_earnable`) and one absent from its current
`live_great_person_offers` screen; native boards carry neither list and are
unchanged. Compatibility re-pin; the Elo protocol does not move.

The 2026-08-17 measured-null production cleanup changes the shared source
file but leaves `AdvancedAi::legacy()` gated away from both retired arms;
compatibility re-pin, not a new rating protocol.

---

## v13 (2026-08-18) — WITHDRAWN, and v14 puts the v12 ruleset back

v13 argued that a rules correction is meant to reach every seat, so moving the
frozen anchor was correct and the ledger should restart. The argument was fine.
**The correction was not a correction.**

#2049 read four Founder-belief modifiers out of the compiled gameplay cache and
changed `beliefs.json` to match them. The cache holds whatever ruleset the game
last ran, and it held the base game. Gathering Storm's
`Expansion2_RemoveData.xml` deletes all four of those modifiers and replaces
them with the per-city and per-follower forms `beliefs.json` already had, so
the change replaced correct expansion values with base-game ones.

#2050 reverted it. The anchor returned to **18,572 decisions** and
`0x3bda_c2f2_b84d_30fc` and the ruleset fingerprint to
`fnv1a64:585ff2655ffd3a6d` — all three the values from before v13, which is how
the revert was verified instead of trusted.

⚠ The version advances to **14** rather than returning to 12. Rows written
under v13 were played on the base game's beliefs and have to stay
identifiable: **v14 rows are comparable to v12 rows, and v13 rows to neither.**

⚠ The lesson is not "be careful with the cache". It is that
`civ6_fidelity.py` had refused a non-Gathering-Storm reference since #1946 and
the refusal sat in `main`, so three lines of `sqlite3` walked past it. It is in
`load_cache_database` now.

---

#2078 makes `BasicAi::military_step` drop attack candidates the engine will
refuse — invisible target, blocked line of sight, wrong melee domain,
unpayable entry — before scoring, behind `legal_tactical_candidates`: `false`
in both `BasicAi` constructors, set only by `AdvancedAi::promoted_policy_envoy`
(production) and withheld by the `advanced_without_legal_candidates` arm. The
anchor's `legacy()` builds its base from `BasicAi::new()` and never sets the
flag, and the two new `Game` predicates (`ranged_order_is_legal`,
`melee_order_is_legal`) are dead code on every frozen path, so `advanced_v1`
keeps proposing — and having refused — the historical candidates.
**Verified by playing, not argued**: `advanced_v1_plays_the_same_game_it_always_did`
passed on this branch at the pre-v15 pin (18,572 decisions,
`0x3bda_c2f2_b84d_30fc`), and again after merging v15 below at its new pin.
Production play does change where a refused order previously won the argmax —
519 authoritative combat-order refusals per censused deployment game, all from
this loop — see `docs/eval/2026-08-18-the-base-picker-stops-proposing-attacks-the-engine-will-refu.md`.

---

## v15 (2026-08-18) — the Builder can reach the pillaged tile

`has_builder_work` decides whether to *train* a Builder and counts a pillaged
improvement anywhere in the empire. The Builder's own target sweep tested only
`valid_improvements`, which a pillaged-but-improved tile fails, and handled
repair for the tile the Builder already stood on and nowhere else.

Two definitions of "work" that disagreed — the wider one spending the
production, the narrower one choosing the destination. The empire trained
Builders for work its Builders could not walk to, and a razed farm went on
earning nothing until one wandered onto it.

Counted over three 250-turn six-player games: Builders reached a decision with
no target 3,704 times, and `has_builder_work` said there was work on **508** of
them.

Unlike almost every entry above, this one is *not* an argument that the change
was free. It reaches every seat: **18,572 decisions became 18,586** across the
five anchor profiles.

⚠ Its strength effect is **not measured**. The justification is that two
definitions of the same thing disagreed and one of them was reachable only by
accident — a defect repair, not a demonstrated gain. Rows before and after v15
are not comparable in any game where an improvement was pillaged.

---

## v16 (2026-08-18) — a scout's report raises its own finite raid

Barbarian Scouts now report only cities they can actually see. The reported
camp then raises one finite, difficulty-shaped raiding party; idle and
unrelated camps cannot consume that party's slots. Each raider retains the
camp that raised it, and the report expires after the party has formed.

This is a native-world rule, not an Advanced-AI controller treatment: every
participant can face the party it creates. The v15 anchor was 18,586 decisions
and `0x2076_c0d8_5213_9238`; with this correction it is **17,494 decisions**
and `0x6cf9_b1fa_a854_dcd6` across the five anchor profiles.

Rows before and after v16 are not comparable. This is a rules correction, not
a compatibility re-pin.

---

## v17 (2026-08-19) — the Great Person roster reaches the Information era

The shipped roster held **29 of Gathering Storm's 213 Great People** and
stopped at the Atomic era. Every class ran out mid-game, and
`Game::unused_great_person_faith` — which correctly models Civilization VI's
`GetFaithFromUnusedGreatPeoplePoints` — then paid the whole Campus, Theatre
Square and Harbour output out as Faith instead of Great People. The engine was
right; the content it was reading from was not there.

Measured over eight 6-player 200-turn games, 48 empire-games, paired on the
same seeds: **26.6% of all non-prophet Great Person points were converted to
Faith**, and seven of the eight non-prophet classes ran dry — writer by median
turn 106, artist 139, scientist 159, admiral 160. With 36 individuals added the
same measurement reports **5.4%**, three classes running dry and none before
turn 172, and **229 Great People recruited against 125**.

Every addition takes its class, era, cost and charges from the shipped
`GreatPersonIndividuals` and `Eras` tables, so `tools/civ6_fidelity.py` still
reports zero divergent fields; the roster row goes from 29/0/0/184 to
65/0/0/148. Effects use only keys the engine already prices —
`tools/civvis_inert.py` still reports zero effect keys with no consumer — and
are class-typical rather than a per-individual translation of each Firaxis
ability, which would need new engine keys.

This is a shared native-world rule with no controller gate: the recruitment
market is the same one every participant draws from. The v16 anchor was 17,494
decisions and `0x6cf9_b1fa_a854_dcd6`; with this correction it is **17,482
decisions** and `0x8162_c919_b83c_40df` across the five anchor profiles.

Rows before and after v17 are not comparable. This is a rules correction, not a
compatibility re-pin.

---

## v18 (2026-08-21) — a Barbarian Scout reports the walkers it sees

`Game::barbarian_phase` gates every raid behind a Scout's report, and v16 gave
each reported outpost a finite, difficulty-shaped party. The report itself
still accepted **a city alone**, so a camp's whole raid throughput was one
Scout's round trip to a settlement — and an empire's Settlers, which is what a
Civilization VI barbarian actually takes, could not start a raid at all. A
Settler walking past a camp was not a sighting.

MEASURED before the change, `ai_eval live live_without_camp_reach`, 12 pairs /
72 seat-games an arm at 6p/150t/online on identical seeds: **0.22 civilians
lost to barbarians per game**. The live Civilization VI seat on the same shape
(run `civvis-20260821T130446Z`) lost **4 of the 8 Settlers that ever walked,
in 104 turns**, at a matching city count — 2 cities at t60 against the
simulation's 2.53. Not an exposure difference; the opponent.

⚠ **CORRECTED 2026-08-21.** This entry first read "8 of 12". The host emits
`unit_lost` when a Settler **founds a city** — the operation consumes the unit,
so `CivvisLedger.onUnitRemoved` reports a founding exactly like a capture, and
counting those events naively doubles the loss. Always subtract the `unit_lost`
events whose `unit` id also carries a `found` event. Across 89 live runs the
real settler-loss rate to turn 100 is **23 %**, not the 56 % a naive count
gives. The conclusion this version rests on is unchanged — the simulation was
an order of magnitude gentler than the host — but the figure was wrong and is
corrected here rather than left to propagate.

With the sighting extended, and with the raider pursuit of #2227 that this
version follows, the same measurement reads **0.61**.

Like v16 this is a **shared native-world rule with no controller gate**: the
Scout phase runs in the engine and every participant faces the same camps, so
`AdvancedAi::legacy()` cannot hold it away from the anchor the way
`barbarian_tactics = false` holds #2227's raider pursuit away. The anchor moves
from v17's 17,482 decisions and `0x8162_c919_b83c_40df` to **18,596 and
`0xf78a_2b10_c0e3_5945`** across its five profiles.

⚠ Rows from v17 and earlier are not comparable with v18 — and that includes
every settler-safety and barbarian-response verdict in `docs/gene_ledger.json`,
each priced at 13,446 pairs against a barbarian that did not hunt civilians.
That family needs re-pricing under v18: `home-defense`, `camp-reach`,
`camp-party`, `civilian-rescue`, `settler-guard-holds`, `stacked-escort`,
`escort-unstick`, `stranded-settler-discount`.

---

## v19 (2026-08-23) — the Great Person roster completes four classes

v17 took the roster from 29 of Gathering Storm's 213 individuals to 65, which
stopped every class running dry mid-game. It did not close the content gap:
`tools/civ6_fidelity.py` still reported **148 individuals the game has and
CIVVIS does not model at all** — by a wide margin the largest *only in Civ VI*
row of the 27 audited tables, ahead of Units at 58 and Promotions at 26.

82 more are added here, taking the roster to **147 of 213**. Four classes are
now complete against the shipped game: every Writer (29), Artist (23), Musician
(18) and Prophet (16). Those four complete because their whole shipped effect
is the Great Works they create, and the per-individual count comes from the
`GreatWorks` table rather than a class constant — which is why Dimitrie
Cantemir and Scott Joplin leave three works where the other sixteen Musicians
leave two.

Class, era, cost and charges again come from `GreatPersonIndividuals`, `Eras`
and `GreatWorks` in the installed Gathering Storm load order, so
`tools/civ6_fidelity.py` still reports zero divergent fields; the roster row
goes from 65/0/0/148 to **147/0/0/66**. `tools/civvis_inert.py` still reports
zero effect keys with no consumer, and
`every_effect_key_in_the_roster_is_read_by_the_engine` now pins that from the
data side as well: every key in all 147 rows is checked against the 34 the
engine branches on, and no row may be effect-less. A Great Person who grants
nothing is worse than an absent one — `current_great_person` offers one
individual per class at a time, so an unread key does not merely grant nothing,
it holds the class at its price.

⚠ The roster is drawn by `min_by_key((era, id))` — era first, then
alphabetically by id — so a new individual can take an opening draw. Two of the
nine move: **Aryabhata replaces Hypatia** as the first Great Scientist and
**Andrei Rublev replaces Donatello** as the first Great Artist, both because
Gathering Storm's Classical Scientists and Renaissance Artists sort ahead of
CIVVIS's incumbent. `the_opening_offer_of_every_class_is_pinned` now names all
nine so the next roster change reports this rather than a recorded game
discovering it.

This is a shared native-world rule with no controller gate: the recruitment
market is the same one every participant draws from. The v18 anchor was 18,596
decisions and `0xf78a_2b10_c0e3_5945`; with this content it is **18,599
decisions** and `0xf412_82b3_5376_9723` across the five anchor profiles.

⚠ Rows before and after v19 are not comparable for the three genes keyed on
which individual is offered — `great-person-housing` (ranked 4, on),
`idle-faith-patronage` (10, on) and `tally-great-people`. Every column in those
`GENE_HEURISTIC_RANKING.md` rows was measured against 65 individuals; a batch
whose `batch.source_commit` predates this must not be pooled with one after it
for them. `docs/eval/2026-08-21-great-people-never-pile-up.md` and
`docs/eval/2026-08-21-idle-faith-buys-great-people.md` are likewise measured on
the old roster: the first counts Engineer blocked seat-turns when five of six
Engineers were wonder-gated (now seven of eight), the second reasons from three
Great Prophets existing (now sixteen).

## v20 (2026-08-24) — the modifier catalog is imported and four rows change the ruleset

Not an AI change at all: `data/modifiers.json` stopped being `{}`. The catalog
is now generated by `tools/civ6_modifiers.py --emit-catalog` from the shipped
`Modifiers`, `DynamicModifiers` and `ModifierArguments` tables, and each CIVVIS
ruleset object that the game says owns a row attaches its bundle by name, so
the loader folds the game's own number into that object's effect map. Most of
the fold restores exactly what CIVVIS already carried and cannot move anything.

Four rows do change the ruleset, and they are why the anchor moves:

- eleven of the thirteen civics that `GRANT_INFLUENCE_TOKEN` pays now award
  their Envoys. CIVVIS carried Colonialism and Conservation and nothing else,
  so twenty-one Envoys per game were never handed out. Envoys buy suzerainty,
  which the AI spends on and plans around, so this is the row that moves the
  decision stream (#2378);
- Jakob Fugger awards the two Envoys his row grants. The other four
  individuals `GRANT_INFLUENCE_TOKEN` reaches — John Jacob Astor, Piero de'
  Bardi, Raja Todar Mal and Ana Nzinga — arrived with v19's roster already
  carrying the right amount, so importing them moves the source of the number
  and not the number;
- Sweeping Wind gains `MOD_MOVE_AFTER_ATTACKING`, which it shares with Elite
  Guard and Breakthrough — CIVVIS had given it to the other two only;
- Computers multiplies Tourism by the +25% `COMPUTERS_BOOST_ALL_TOURISM`
  states, where CIVVIS carried +100%.

There is no controller gate for any of these: they are ruleset data every
participant plays under, so `AdvancedAi::legacy()` cannot hold them away from
the anchor. The anchor moves from v19's 18,599 decisions and
`0xf412_82b3_5376_9723` to **18,368 and `0x02f8_e0dd_ae0a_d0f2`** across its
five profiles. Both numbers were re-derived on the merged tree rather than
carried over: v19 landed while this branch was open, and a constant taken from
either side alone would match neither ruleset.

## v21 (2026-08-24) — the barbarian seat plays at Immortal whatever the majors' rung

A world rule, by operator directive (2026-08-24): *"We need to make the
barbarians in civvis more aggressive. Should still roughly match the Civ 6
behavior. But weak barbarians in civvis leads us to weak training and
favoring the slightly wrong genes … we are playing on level 5 and higher in
Civ 6 verification games. Let's make the barbarians level 6 barbarians in
civvis for now."*

Level 6 on the ladder is Immortal, and Immortal is exactly where the game's
own `BarbarianAttackForces` changes band: `HighDifficultyStandardRaid` (and
its cavalry and attack siblings) assembles three melee and two ranged units
against two and one, at a `SpawnRate` of 1 against 2 — twice as often. Those
are the rows `data/difficulties.json` has carried since the ladder audit as
`barb_force_scale 1.5` / `barb_spawn_scale 0.5` (bands exact, sizes
approximated, `docs/FIDELITY.md`). Until now the barbarian seat read them
from the *majors'* rung, and every native board — the anchor's five
profiles, every gene screen, every fires probe — runs at the Prince default,
so every gene ever priced was priced against the Standard band (two melee
and one ranged, every other turn). `Game::barbarian_difficulty` is now the
rung the barbarian seat plays by, `immortal` by default
(`default_barbarian_difficulty`), settable per game
(`--barbarian-difficulty`, `Game::set_barbarian_difficulty`); the seat's
own rung still governs the human's camp Gold and the AI handicaps.

There is no controller gate for this: it is what the world does, and every
seat in a game meets the same raiders. The anchor moves from v20's 18,368
decisions and `0x02f8_e0dd_ae0a_d0f2` to **18,790 and `0x32d6_ac78_9161_017f`** across its
five profiles — more decisions, as more raiders arrive and are answered.
Two tests that pinned "the majors' rung sizes the raid" now set the
barbarian rung explicitly and additionally pin that a Settler seat meets the
Immortal band; the recon-disruption real-game pin holds its board at the
Standard band, since the barbarians are not what it tests.

What this does to the record: every screen in `docs/gene_screens/` before
this date, and the ledger's verdicts, measured genes against barbarians
that raided at half the cadence with three-fifths of the force. The next
standard screen reprices the pool in the new world; read a gene's newest
column, not its pooled Diff, until one has landed. Raising further ("we can
raise more later") is a one-word change to `default_barbarian_difficulty`
— the data carries Deity at the same band, so a further step is a new
band in the data or a model of the game's `BARBARIAN_LOWER_THROTTLE_PER_
DIFFICULTY` and boldness, which CIVVIS does not carry.

## v22 (2026-08-25) — the data ratchet runs, and seven shipped numbers replace seven remembered ones

`tools/civ6_fidelity.py` documented `--max-divergences 0 # CI gate` from the
day it was written and no workflow ran it. Wired in as
`civ6_fidelity.py --check --max 0` (#2441), its first run against the
compiled Gathering Storm database reported ten divergences that every
gate had been green over. Seven are plain transcriptions and are now the
shipped values: the Tagma costs 220 Production and 4 Gold upkeep and
upgrades to the Cuirassier (it was 180, 3, and a Tank — the Knight's own
successor, `UnitUpgrades`); the Pike and Shot pays 4 upkeep (3); the Prasat
yields +6 Faith and holds one Relic (+4, two); the Sukiennice +2 Gold (+3);
the Tlachtli +2 Culture (+1); Eyjafjallajökull's neighbours take +1 Food
(+2); and the human's camp Gold above Prince, which
`BARBARIAN_CAMP_GOLD_SCALING` runs −5 per rung to −20 at Deity and the data
had stopped transcribing at Warlord's +5. The Vampire Castle's mode-only
terrain, Prince's AI at −1 Combat Strength and Warlord's human bonuses
under an inverse requirement are waived with their reasons in
`tools/fidelity_waivers.json`, which a test now caps at thirteen.

There is no controller gate for a ruleset transcription: every seat reads
the same `data/*.json`. The anchor moves from v21's 18,790 decisions and
`0x32d6_ac78_9161_017f` to **18,796 and `0x0b7b_b89a_2d86_c831`** across its
five profiles — a handful more decisions, as Byzantium's Tagma and the
Aztec and Polish buildings are priced and produced at their shipped values.
The ratchet is a real number only on a fleet Mac (a hosted runner has no
database and the step skips by name), so the number to keep at zero is the
one `ship` validation prints.

## v23 (2026-08-25) — Monumentality gives its movement to Builders, not Settlers

The live verifier caught a Settler repeatedly held at a river crossing even
though the host reported two movement points and no blocking unit. CIVVIS had
modelled Monumentality as +2 movement for both Builders and Settlers, so its
route planner treated that host Settler as a four-move unit that had already
spent part of its allowance; it refused to emit the legal final move. The
shipped database is explicit: `COMMEMORATION_INFRASTRUCTURE_GA_MOVEMENT` uses
`MODIFIER_PLAYER_UNITS_ADJUST_MOVEMENT` with
`UNIT_IS_GOLDEN_AGE_BUILDER`, whose requirement is
`REQUIREMENT_UNIT_IS_BUILDER`. Settlers share Monumentality's faith-purchase
discount, not its movement modifier.

This is a shared rules correction, not a controller treatment that can be
hidden behind a gene: every simulated player must use the same movement
allowance as the host. The anchor therefore moves from v22's 18,796 decisions
and `0x0b7b_b89a_2d86_c831` to **18,797 and
`0x728b_8a78_d941_5de3`** across its five profiles. The dedicated age test
pins both sides of the rule: a golden-age Builder gains two movement and a
golden-age Settler remains at its shipped base allowance.

## v24 (2026-08-26) — a tile keeps what a disaster left on it, and nobody has to own it first

Two rules corrections in one place, both found from the live board: an
operator reported a tile Civilization VI was paying **3 Food 3 Production**
that CIVVIS drew as 2 Food 2 Production, after a volcano went off beside it.

**Fertility was one Food roll; the shipped table rates each yield apart.**
`RandomEvent_Yields` in `DLC/Expansion2/Data/Expansion2_RandomEvents.xml`
carries a row per `YieldType`: a Catastrophic eruption leaves Food on 50% of
the plots it reaches and Production on 25% of them, independently, and a
Megacolossal one 75% and 35%. CIVVIS had a single `fertility_chance` and spent
it on Food. `Tile::disaster_production` existed, was summed into every tile's
yields, and **nothing ever wrote it**. The odds themselves were wrong in four
more places: `river_flood` and `blizzard` sat at zero although the table gives
them 15–60% and 10–20% Food, `volcanic_eruption`'s middle tier read 55%
against the shipped 50%, `hurricane` read 20/40 against 30/45, and
`dust_storm` 25/45 against 10/20. `docs/FIDELITY.md` said the table "cannot be
read"; it was on disk. The one approximation left is `river_flood`, whose
three Floodplains features are rated apart in the file and are averaged into
the single per-severity rate `DisasterSpec` carries.

**And an unowned plot kept none of it.** `Game::modeled_tile_yields` handed a
tile nobody owned straight to `Rules::worked_tile_yields`, which reads terrain,
feature, resource and improvement and knows nothing about the ground's own
history — so disaster fertility, a neighbouring feature's adjacency, drought
and fallout were invisible until a border reached the plot. That is exactly
backwards: unclaimed ground is what a Settler and a Builder choose *between*,
and Volcanic Soil ships with no `Feature_YieldChanges` row at all, so the
fertility is the whole of what such a plot is worth. `ground_tile_yields` now
carries the player-independent half and both the owned and unowned paths go
through it.

Neither half is a controller treatment that could hide behind a gene: every
seat reads the same `data/*.json`, and a tile has to pay every player what the
game pays it. The extra per-tile fertility roll also moves the disaster RNG
stream, so the anchor's games diverge wholesale rather than at the margin. It
moves from v23's 18,797 decisions and `0x728b_8a78_d941_5de3` to **18,557 and
`0x441a_fc9a_dfdb_76ba`** across its five profiles. The shipped ruleset
fingerprint moves with `data/disasters.json`, to
`fnv1a64:fbd69339f1ba4e2b`.

## v25 (2026-08-26) — a unit may walk through its own units

Civilization VI checks the stacking layer at the **end** of a move, not at
every step. A unit crosses a tile held by its own unit of the same layer when
it has the Movement to leave again, and may not finish there. CIVVIS asked that
question at every hex, through a single boolean — `Game::can_enter` — that
answered both "may this unit step onto that tile" and "may it be left standing
there". Every flood, path and route in the engine consumed it, so a friendly
unit was a wall: no column filed through a defile, no front line rotated, no
route was planned through the army occupying it, and the reachability overlay
stopped at our own units.

Reference basis: the in-game
[Civilopedia "Movement" entry](https://www.civilopedia.net/en-US/standard-rules/concepts/movement_3/),
and the shipped end-turn blocker `ENDTURN_BLOCKING_STACKED_UNITS`, which this
repository's own bridge already handles (`docs/CIV6_COMPUTER_CONTROL.md`) and
which exists precisely because units are allowed to be stacked mid-turn. The
whole rule, and what is deliberately not modelled, is written down in
`docs/MOVEMENT.md` and pinned test by test in
`src/game/movement_rule_tests.rs`.

This is a shared-engine rules correction and not a controller treatment: no
gene can keep it away from `legacy()`, because what is *legal* changed. Every
seat plays by it.

**It moves the anchor by four decisions.** `Game::entry_at` now answers
`Blocked` / `Pass` / `Stop`; floods and paths expand on the crossing question
and offer only the stopping one; `Action::MoveTo` executes a walk that crosses
and anchors on the last tile the unit could legally be left on. `BasicAi`
reaches its pass-through walk **last**, after every ordinary step has been
refused, so it replaces a hold rather than a march — which is why five profiles
and roughly eighteen thousand decisions move so little. It goes from v24's
18,557 decisions and `0x441a_fc9a_dfdb_76ba` to **18,553 and
`0x14b8_c800_4f26_5e5b`**. The shipped ruleset fingerprint does not move: no
`data/*.json` changed.

⚠ Worth recording beside the re-pin, because it is the reason this entry is
dated 2026-08-26 and not two years earlier: **no instrument in this repository
could have found it.** Gene screens, the Elo ladder, the Tactics bench and the
doctrine arena all play both arms under the same rules, so a missing rule
cancels out of the control and the treatment identically. It was found by a
person playing the Tactics arena. See `docs/AI_GAPS.md` and `docs/FIDELITY.md`.
