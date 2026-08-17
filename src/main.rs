//! CLI: simulate / soak / benchmark (mirrors the Python CLI outputs).
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use civvis::ai::{run_game, AdvancedAi, Ai};
use civvis::game::{
    default_difficulty, default_speed, Game, GameOptions, LeaderPool, VictoryConditions,
    WarRecord, DEFAULT_DISASTER_INTENSITY, GAME_MODES,
};
use civvis::leader_roster;
use civvis::rules::Rules;
use civvis::setup::{self, BaseRuleset, GameSpeed, MapPoles, MapScript, MapSize, MapTopology};

/// The mutable controller is deliberately given a dated rating identity.
/// Reusing the bare `advanced` row after its implementation changes would
/// blend two players into one lifetime average and erase the very improvement
/// the longitudinal tournament is supposed to expose.
const DEFAULT_TOURNAMENT_ENTRANTS: &str =
    "advanced-20260801-diplomacy=advanced,advanced_v1,basic-20260801-diplomacy=basic,random-20260730=random";

/// `advanced_v1` freezes the planning configuration, but deliberately shares
/// the production Basic/Advanced implementation. Pin those sources so a code
/// edit cannot silently change the longitudinal anchor. If an edit reaches
/// the legacy path, bump the Elo protocol and start a new ledger; if it is
/// provably gated away, review that fact before updating this guard.
///
/// Recomputed after removing default-off experiments that had no whole-game
/// proof. The fingerprint covers the whole of both files, so any future edit
/// must either pass the fixed-prefix compatibility check or advance the Elo
/// protocol and start a new ledger.
///
/// #660 subsequently adds only default-off evaluator fields and a disabled
/// production prepass. `AdvancedAi::legacy()` leaves those gates off; the merged
/// source contract was re-pinned only after its fixed-prefix behavior check.
///
/// #672 adds two more default-off adaptive-Expansion flags and observer-only
/// action telemetry. With all flags false, a matched release-mode `ai_eval
/// advanced basic --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1`
/// comparison against pre-#672 `374e0f0` had identical scores, sweeps, seat
/// metrics, victory mix, and strategy-transition counts across 40 Advanced
/// seat-games and 4,022 observed Advanced turns. The only added report is a
/// zero-valued telemetry block, so this fingerprint is deliberately re-pinned
/// rather than changing the Elo protocol.
///
/// #673 similarly adds an empty `BeliefState` and a default-off pressure arm.
/// A clean `2f3dcb7` release build and this branch both produced the identical
/// 19/20 Advanced game wins, scores, sweeps, seat metrics, victory mix, and
/// strategy-transition counts across 40 Advanced seat-games and 4,022 observed
/// Advanced turns on that same fixed prefix. The re-pin is justified because
/// the arm never observes or contributes a nonzero term while its flag is off.
///
/// #686 repairs a legacy settlement path: a passable natural wonder must not
/// remain a settler target when `Game::can_found_city` will always reject it.
/// Because that behavior is live for `advanced_v1`, the Elo protocol advances
/// to v3 and the source contract is recomputed with the new ledger rather than
/// being treated as a default-off compatibility re-pin.
///
/// #697 lands the Civilization VI bridge. It adds `forget_unit_memory` and
/// `remap_unit_memory` to both agents and a `settle_ranking` wrapper, none of
/// which the play path calls — only `civvis_orders --fresh-board` and
/// `civvis-advise` do — and one guard inside `settle_sites` that skips a site in
/// `Game::blocked_city_sites`. That set has exactly one production writer,
/// `mirror.rs`, which fills it from a host engine's refusals; `Game::new` and
/// `From<GameSer>` both leave it empty and nothing in an ordinary game ever
/// inserts into it, so the guard cannot fire outside the bridge. Measured on the
/// fixed prefix the re-pins above use — `ai_eval advanced basic --pairs 10
/// --players 4 --turns 200 --seed 31337 --jobs 1`, release, this branch against
/// `main` at `81636d9` — the two reports are **byte-identical**: 19/20 game wins,
/// 95.0% paired-map score, 9 sweeps and 1 neutral, and every seat metric equal
/// across 20 games and 2,310 turns. Default-off compatibility re-pin; the Elo
/// protocol does not move.
///
/// #704 widens `PolicySpec::replaces` from `Option<Name>` to a list, because
/// Civilization VI's `ObsoletePolicies` lets one card retire several. The only
/// edit inside this anchor is the obsolete-card scan adapting to the new type:
///
/// ```text
/// - .filter_map(|policy| policy.replaces.clone())
/// + .flat_map(|policy| policy.replaces.iter().copied())
/// ```
///
/// **`data/policies.json` is untouched by that PR**, so every `replaces` still
/// deserializes to exactly one name, and a `filter_map` over `Option` and a
/// `flat_map` over a one-element `Vec` collect the identical set. The obsolete
/// set, and therefore every policy decision downstream, cannot differ — this is
/// a type change, not a behaviour change. Confirmed on the same fixed prefix,
/// release, this branch against `main` at `1d8567b`: the two `ai_eval` reports
/// are **byte-identical**. Compatibility re-pin; the Elo protocol does not move.
///
/// #682 adds fog-aware campaign and battlefront observation to the live
/// controller, but `AdvancedAi::legacy()` explicitly disables every new
/// branch. A clean `41c02c0` release build and this branch produced identical
/// output from `ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337
/// --players 4 --turns 200 --deployment-comparison`: all 20 game results,
/// scores, sweeps, seat metrics, victory mix, and strategy-transition counts
/// matched across 40 Advanced-v1 seat-games and 4,300 observed Advanced-v1
/// turns. The source contract is deliberately re-pinned after that direct
/// compatibility check rather than changing the Elo protocol.
///
/// #684 adds one default-off evaluator field, `AdvancedAi::plan_city_target`,
/// and the delegated-call substitution it gates. With the flag false the
/// substitution is a `bool::then` that returns `None` and touches nothing, so
/// this is a compatibility re-pin and not a protocol change. It is earned the
/// way the entries above are: a matched `ai_eval advanced basic --pairs 10
/// --players 4 --turns 200 --seed 31337 --jobs 1` on a clean `origin/main`
/// build and on this branch, compared in full.
///
/// #719 freezes that live battlefront observation at the start of a major
/// turn, including camouflage detection. `advanced_v1` still disables the
/// observation path. Clean before/after release builds produced byte-identical
/// output from the same 10-map deployment comparison as #682: 16/20
/// `advanced_v1` game wins, 80.0% paired-map score, six sweeps, and 4,264
/// observed Advanced-v1 player-turns. The contract is re-pinned because the
/// gated legacy path did not move; the Elo protocol does not change.
///
/// The Civ VI bridge also needs to begin a route for an idle Firaxis Trader
/// whose normal walking movement is zero. `start_zero_movement_trader_route`
/// sits behind a default-off bridge flag enabled only by `civvis_orders` before
/// `Ai::take_turn`; `advance_unit_serial`, which is the native tournament loop,
/// is unchanged. The new code cannot run in an `advanced_v1` tournament game,
/// so its historical agent and Elo protocol remain unchanged. Re-pin the source
/// contract to make that reviewed exception explicit.
/// #746 promotes the confirmed policy/envoy composite only through the public
/// production constructors. `AdvancedAi::legacy()` still calls `configured`
/// directly and cannot reach that wrapper. A release build of `e46d1b7` and a
/// separately targeted release build of this change produced byte-identical
/// `ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 --players 4
/// --turns 200 --deployment-comparison` reports. This is a compatibility
/// re-pin, not an Elo protocol change.
///
/// #757 filters only the production controller's coordinated tactical threat
/// score behind `battlefront_observation`; `AdvancedAi::legacy()` keeps that
/// gate off. Clean `1c93908` and candidate release reports from the same
/// 10-map deployment comparison were byte-identical after Cargo's build
/// prelude (SHA-256
/// `e37ae6f3014c6f13c75ef964027e7b57f5e57e9289f0fdb36cae80f5bb863341`).
/// This is a compatibility re-pin, not an Elo protocol change.
///
/// #761 parallelizes only live policy-card counterfactual scoring. The pool
/// exists only in `AdvancedAi::fleet_parallel`; `AdvancedAi::legacy()` keeps
/// `work_pool` at `None`, so its ancillary pass selects the literal serial
/// scorer. The new `QueryMemo` is confined to an unchanged read-only policy
/// valuation and drops before a card is changed. Clean `cb7969d` and candidate
/// release builds produced byte-identical SHA-256 reports from `ai_eval
/// advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 --players 4 --turns 200
/// --deployment-comparison` (20 games and 4,264 observed Advanced-v1 turns).
/// This is a compatibility re-pin, not an Elo protocol change.
///
/// #762 bounds that same production-only card scorer to four worker-private
/// snapshots even when its persistent fleet pool is wider. The `None` branch
/// used by `AdvancedAi::legacy()` still chooses the literal serial scorer.
/// Clean `0b04a59` and candidate release builds produced byte-identical
/// reports from the same 20-game deployment comparison (SHA-256
/// `932cfabf125e729a5264ce43d2fd8b05d013d3fe84939b1dcd366ff122ddc84a`).
/// This is a compatibility re-pin, not an Elo protocol change.
///
/// #766 bounds only the live controller's clone-heavy purchase-menu batch to
/// three workers. `AdvancedAi::legacy()` has no `work_pool` and continues to
/// select the literal serial action enumeration. Clean `8812d36` and candidate
/// release builds produced byte-identical reports from the same 20-game
/// deployment comparison (SHA-256
/// `932cfabf125e729a5264ce43d2fd8b05d013d3fe84939b1dcd366ff122ddc84a`).
/// This is a compatibility re-pin, not an Elo protocol change.
///
/// #782 repairs runaway military production and wartime science/culture
/// neglect only for the live controller. Every new strategic branch is gated
/// by `victory_planning`; `AdvancedAi::legacy()` sets that flag false. The
/// regression test checks the legacy yield weights and production choice as
/// well as the live behavior. The source contract is deliberately re-pinned;
/// the Elo protocol does not move.
///
/// #786 adds live Delegation/Embassy, Defensive Pact, Joint War, promise, and
/// demand decisions to the shared Basic and Advanced diplomacy paths. The
/// frozen `advanced_v1` controller invokes that shared path, so the combined
/// source contract intentionally moves to protocol v4 with a fresh ledger.
///
/// #801 makes compiler-equivalent `BasicAi` cleanup only: redundant clones of
/// `Copy` values and needless references become direct values, a periodic
/// modulo test becomes `is_multiple_of`, a candidate tuple gains a name, and
/// unused mutability goes away. It changes no choice condition, score, or
/// iteration/action ordering. This was checked rather than inferred: clean
/// `e3481e4` and candidate release builds produced byte-identical reports from
/// `ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 --players 4
/// --turns 200 --deployment-comparison` (SHA-256
/// `f6d9e17ee19fe298e14a573f97a896280a75a767306dca6ef0d80d2020384b2c`).
/// This is a compatibility re-pin, not an Elo protocol change.
///
/// #799 adds the live settlement-site intelligence and visible transit-risk
/// gates to the shared Advanced source. `AdvancedAi::legacy()` explicitly
/// keeps the historical settlement scorer and disables those gates, so the
/// source contract is deliberately re-pinned without moving the Elo protocol.
/// #802 adds a settle-scoring adjacency term gated behind
/// `AdvancedAi::adjacency_site_planning` — on in `promoted_policy_envoy`,
/// off in `configured()`, so `AdvancedAi::legacy()` never evaluates it.
/// Checked the same way: baseline and branch builds produced byte-identical
/// `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed
/// 31337` reports. Another compatibility re-pin over the merged sources,
/// not an Elo protocol change.
///
/// #808 records the planner's peace-offer decisions (`peace_offers`, a
/// BTreeSet written at the offer site) and exposes them on `PlanReport`,
/// which is observer-only by contract — nothing in play reads the field.
/// Checked the same way: baseline and branch builds produced byte-identical
/// `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed
/// 31337 --jobs 1` reports. Another compatibility re-pin over the merged
/// sources, not an Elo protocol change.
///
/// #838 bundles already-shared purchase-scoring inputs and replaces only
/// `Copy`/iterator idioms in the Advanced source. The control on `main` and
/// this branch produced byte-identical `ai_eval advanced_v1 basic --pairs 10
/// --players 4 --turns 200 --seed 31337 --jobs 1 --deployment-comparison`
/// reports. This is therefore a compatibility re-pin, not an Elo protocol
/// change.
///
/// #840 adds population-four settlement forecasting, bounded travel,
/// stalled-route recovery, and land escorts to the shared Advanced source.
/// Every decision path is gated by `settlement_safety`, which
/// `AdvancedAi::legacy()` disables. Clean `bc58acb` and candidate builds
/// produced identical `ai_eval advanced_v1 basic --pairs 10 --players 4
/// --turns 200 --seed 31337 --jobs 1 --deployment-comparison` reports:
/// 18/20 Advanced wins, 119.2 average turns, and identical terminal tables.
/// #848's progress-versus-motion tracker is now merged into the same path, but
/// it returns through the prior code whenever `settler_commit` is disabled, as
/// it is in `AdvancedAi::legacy()`. This is another compatibility re-pin, not
/// an Elo protocol change.
///
/// The live Civ VI mirror's purchase-placement regression moves only a unit in
/// a `cfg(test)` fixture. The compiled AdvancedAi implementation is unchanged;
/// this is therefore another reviewed compatibility re-pin.
///
/// #896 keeps repeatable economic projects out of the live controller's
/// wartime land-force gap. The new branch is gated by `victory_planning`,
/// which `AdvancedAi::legacy()` disables. Clean `f114601` and candidate
/// release builds produced byte-identical stdout from `ai_eval advanced_v1
/// basic --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1
/// --deployment-comparison`: 16/20 Advanced-v1 wins, 125.9 average turns, and
/// identical terminal and strategy-transition tables. Compatibility re-pin;
/// the Elo protocol does not change.
///
/// #879 replaces `filter(...).next()` with `find(...)` in one `cfg(test)`
/// fixture and removes an unnecessary `mut` from another. Neither change is
/// part of a compiled controller, so Advanced-v1 behavior and the Elo protocol
/// remain unchanged. The source contract is deliberately re-pinned for this
/// reviewed test-only diff.
///
/// #927 adds escort progress accounting behind `linked_settler_progress`,
/// which is false for configured and legacy engine agents and enabled only by
/// the live `civvis_orders` bridge. Engine and Elo trajectories therefore keep
/// their prior behavior; this is a compatibility re-pin, not a protocol bump.
///
/// #929 retries asynchronous Firaxis governor postings behind
/// `live_governor_assignment_adapter`. Configured and legacy engine agents keep
/// it false; only `civvis_orders` enables it. Compatibility re-pin, not an Elo
/// protocol change.
///
/// #911's escorted-settler correction remains behind `settlement_safety`,
/// which `AdvancedAi::legacy()` disables. The live religious-purchase guard
/// added afterward is likewise default-off in `BasicAi` and enabled only by
/// `civvis_orders`; its focused test preserves the historical rival-faith
/// purchase with the flag off. A matched 10-map, 20-game deployment comparison
/// had identical substantive output across 3,801 observed Advanced-v1 turns.
/// These are compatibility re-pins, not an Elo protocol change.
///
/// #930 adds `besieged_military_floor`, which lets a city under visible siege
/// raise its standing-army floor against hostiles the empire has no diplomatic
/// state with (Barbarians are excluded from `at_major_war` by design, so every
/// defensive escalation in `pick_item` previously read a barbarian siege as no
/// threat at all). It sits behind `siege_muster`, a `BasicAi` flag that is
/// false in both constructors and is enabled only by the live `civvis_orders`
/// bridge, so configured, legacy and Elo agents keep their prior behavior —
/// `besieged_military_floor` returns 0.0 before reading the board when the flag
/// is off. Unconditional, the same change perturbed
/// `oracle::tests::the_modernity_grant_actually_fires`; gated, the full suite
/// is unchanged. A compatibility re-pin, not a protocol bump. The same flag
/// also gates `besieged_city_item`, which lets a city with a raiding party at
/// its gates build walls or a defender ahead of the ordinary build order; both
/// entry points return early on `!siege_muster` before reading the board.
///
/// #933 bounds the defensive-war Recovery posture behind `bounded_recovery`, a
/// field that is `false` in `AdvancedAi::new()` and set only by
/// `civvis_orders`. `recovery_is_stale` short-circuits on that flag before it
/// reads the plan or the clock, so `assess` returns the identical strategy for
/// configured, legacy and Elo agents. Headless confirmation: 24 paired seeds
/// with the flag on and off are byte-identical on score, cities and the
/// strategy census, because the sim reaches Recovery on only 8% of
/// strategy-turns against the live ladder's 86%. A compatibility re-pin.
///
/// #934 re-keys that same bound from the plan's age to the WAR's age
/// (`major_war_since`). Still behind `bounded_recovery`, still short-circuiting
/// on the flag before anything is read, so every configured, legacy and Elo
/// agent is byte-identical. Another compatibility re-pin.
///
/// #955 adds `home_defense_objective`, which lets a raider standing in our own
/// territory claim a unit before the offensive does. Gated behind the new
/// `home_defense` flag, which is `false` in BOTH `BasicAi` constructors and is
/// turned on only by the Civilization VI bridge — exactly the contract
/// `siege_muster` runs under. `home_defense_objective` short-circuits on that
/// flag before it reads anything, so every configured, legacy and Elo agent is
/// byte-identical and `the_default_controller_keeps_home_defense_off` asserts
/// it on a board that DOES yield an objective once enabled. A compatibility
/// re-pin.
///
/// #955 also adds `garrison_assignments`/`garrison_step`, which put a unit on a
/// threatened city's own tile. Behind the SAME `home_defense` flag, short-
/// circuiting on it first, so this too is a compatibility re-pin.
///
/// #962 gates faith military purchases on whether the empire can pay the GOLD
/// upkeep the soldier incurs. Behind the new `solvent_faith_army` flag, which is
/// `false` in `AdvancedAi::configured` (so `new()` and `legacy()` both have it
/// off) and is turned on only by the Civilization VI bridge —
/// `faith_military_is_affordable` returns `true` immediately on that flag, so
/// every configured, legacy and Elo agent buys exactly what it always did.
/// `the_default_controller_keeps_the_faith_army_ungated` asserts it on a board
/// that DOES refuse once enabled. A compatibility re-pin.
///
/// #957 prices a fogged objective city from this controller's last sighting
/// inside `local_strength_ratio`, behind `blind_objective_strength` — again
/// `false` in `AdvancedAi::new()` and set only by `civvis_orders`.
/// `remembered_objective_strength` returns `None` on that flag before it
/// touches the belief state, and the fallback is only reachable at all once
/// `battlefront_observation` is on. `advanced_v1` sets that to `false`, so the
/// legacy anchor takes the same `Some(g.city_strength(city))` arm it always
/// took and is byte-identical twice over. A compatibility re-pin.
///
/// #963 sizes the siege train against the target city's standing wall, behind
/// `siege_tracks_the_wall` — `false` in `AdvancedAi::new()` and set only by
/// `civvis_orders`. `siege_units_wanted` returns the shipped
/// `usize::from(plan.target_city.is_some())` on that flag before it reads the
/// board, so the legacy anchor's production value is bit-for-bit what it was.
/// A compatibility re-pin.
/// #819 routes `BasicAi::tactical_step` and the Advanced force mover through
/// `path_move` so a unit stepped twice in one turn cannot reverse its own
/// first step, behind `recorded_tactical_step` — `false` at both `BasicAi`
/// construction sites and set only by `civvis_orders`. Unlike the flags
/// above, this one guards a call that can *refuse*: `path_move` rejects a
/// reversal, a retread, or a minor leaving its defense area where the raw
/// `g.apply(Move)` would have moved. `tactical_apply_move` therefore returns
/// the historical raw apply on the flag before it reaches `path_move` at all,
/// so `advanced_v1` takes byte-for-byte the arm it always took. A
/// compatibility re-pin.
/// #1727 lets a coordinated Advance/Engage unit that has proven a multi-turn
/// livelock use its A* route through one recorded square. The exception still
/// keeps same-turn reversal and all normal movement guards, and requires
/// `recorded_tactical_step`, which the frozen anchor never enables. A
/// compatibility re-pin.
///
/// #974 adds a `Cities/Decision` journal line to `advanced_production`, which
/// had none. It is inside `if self.journal().wants(Decision)` and writes only
/// to the reasoning journal — no board state is read or changed, and the
/// legacy anchor's chosen item is bit-for-bit what it was. A compatibility
/// re-pin.
///
/// #965 promotes wide, developed, defended expansion only in the production
/// constructor: it enables call-local city/Builder floors, plan delegation plus
/// the three existing defense flags, and lets that flagged plan consume the
/// land-aware nine-city ceiling. Stored genomes, `configured`, and
/// `AdvancedAi::legacy()` retain the historical weights; the controls also keep
/// their three-city floor, six-city ceiling, flat delegation, and default-off
/// defense fields. The focused production/control contract test asserts each
/// side of that boundary. This is therefore a compatibility re-pin for
/// `advanced_v1`, not an Elo protocol change.
///
/// #976 adds `AdvancedAi::enable_live_bridge` (the eight bridge flags in one
/// place, so a headless arm can play the deployed agent) and three
/// `disable_*` methods that hold one flag off for a measurement arm. Nothing
/// calls either from `new()` or `legacy()`, so every configured, legacy and Elo
/// agent is byte-identical. A compatibility re-pin.
///
/// #958 prices research outside the victory lane, behind `research_economy`.
/// `advanced_v1` is `AdvancedAi::legacy()`, which goes through
/// `AdvancedAi::configured` and therefore has that field `false`; only
/// `promoted_policy_envoy` turns it on. The identity is exact rather than
/// sampled, and it holds in two ways at once:
///
/// - the Campus coverage bonus, the peacetime Campus-building debt and the
///   policy-deck insertion are each guarded by `if self.research_economy`, so
///   for the anchor they are not evaluated at all;
/// - the three weight terms are floors — `science.max(self.research_weight)` in
///   `yield_value`, `yield_weights.science.max(..)` in the search evaluator, and
///   `ys.science.max(research_tilt)` in `lane_emphasis`. For an agent without
///   the flag, `refresh_research_weight` writes `0.0` and the tilt argument is
///   `0.0`, and every value being floored is already non-negative (the lane
///   science weights run 1.0-4.2, the evaluator's 0.5-2.8, the emphasis 0.0 or
///   0.50). A floor at zero over a non-negative quantity is the identity, so
///   this is provably byte-identical and not merely measured to be.
///
/// A compatibility re-pin.
///
///
/// #977 raises the wartime army target when the enemy outweighs us, behind
/// `army_target_weighs_the_enemy` — `false` in `AdvancedAi::new()` and set only
/// by `civvis_orders`. `wartime_army_target` returns its `shipped` argument
/// unchanged on that flag before it reads a single player, so every configured,
/// legacy and Elo agent wants exactly the army it always wanted. A
/// compatibility re-pin.
///
/// #1056 skips policy cards that multiply a suzerainty count of zero, behind
/// `suzerain_cards_need_a_suzerainty`, `false` in `AdvancedAi::new()` and set
/// only by `enable_live_bridge`. `strategic_policies` reorders nothing on that
/// flag before it counts a single city-state, so every configured, legacy and
/// Elo agent picks exactly the deck it always picked. A compatibility re-pin.
///
/// #981 adds `BasicAi::loyalty_emergency`, which ranks loyalty trouble by TURNS
/// TO FLIP rather than by level, behind the new `loyalty_rate_alarm` flag. The
/// flag is `false` in both `BasicAi` constructors and `loyalty_emergency`
/// returns the old level-only answer on it before reading any rate, so every
/// configured, legacy and Elo agent behaves identically. A compatibility re-pin.
///
/// #984 credits a movement tile for the attack it opens, behind
/// `strike_opening` — `false` in `AdvancedAi::new()` and set only by
/// `enable_live_bridge`. `strike_opening_value` returns 0.0 on that flag
/// before it reads the board, so every configured, legacy and Elo agent scores
/// every tile exactly as it did. A compatibility re-pin.
///
/// #990 adds four `disable_*` methods so every flag in `enable_live_bridge` has a
/// measurement arm. They are called only by `builtin_ai`'s `live_without_*`
/// factories, never in play, so every configured, legacy and Elo agent is
/// byte-identical. A compatibility re-pin.
///
/// #989 adds a journal line for a DECLINED attack and a diagnostic tally of the
/// reasons the forward model refuses one. Both are behind
/// `journal().wants(Detail)` or write only to a process-local census; no board
/// state is read and no decision changes, so every configured, legacy and Elo
/// agent attacks exactly what it always did. A compatibility re-pin.
///
/// #991 makes a ranged unit prefer a movement tile it can actually see the
/// target from, behind `ranged_needs_line_of_sight` — `false` in
/// `AdvancedAi::new()` and set only by `enable_live_bridge`.
/// `ranged_tile_is_blind` returns `false` on that flag before it reads the
/// board, so every configured, legacy and Elo agent scores every tile exactly
/// as it did. A compatibility re-pin.
///
/// #999 gives the research chooser a goal for a Campus building the empire is
/// already equipped for but cannot reach, behind `research_economy`. That field
/// is `false` in `AdvancedAi::configured`, and `unreachable_science_building_tech`
/// returns `None` on it before reading the board at all, so `advanced_v1` picks
/// the technology it always picked. A compatibility re-pin.
///
/// #1003 lets the baseline governor build an Entertainment Complex when the
/// host reports the city paying the Amenity band, behind
/// `BasicAi::amenity_districts`. That field is `false` in both `BasicAi`
/// constructors and is set only by `AdvancedAi::promoted_policy_envoy`; the
/// added block short-circuits on it before reading the board, so `advanced_v1`
/// ranks the same four district families in the same order. A compatibility
///
/// The siege-role branch adds `best_military_role`, `siege_is_the_missing_arm`
/// and a `missing_siege_arm` term on the army floor, all behind the new
/// `siege_role` flag. It is `false` in both `BasicAi` constructors and every
/// new path short-circuits on it before reading anything, so every configured,
/// legacy and Elo agent picks exactly what it always picked. A compatibility
/// re-pin.
///
/// #1011 holds a promotion until its healing would land, behind
/// `promote_when_wounded` — `false` in `AdvancedAi::new()` and set by nothing
/// on the shipped paths (it is native/eval only and deliberately absent from
/// `enable_live_bridge`). `promotion_heal_is_wasted` returns `false` on that
/// flag before it reads a unit, so every configured, legacy and Elo agent
/// promotes exactly when it always did. A compatibility re-pin.
///
/// #954 says why a settler was held instead of only that it was marching. The
/// added block is inside `if !moved && self.journal().wants(Detail)` and every
/// call it makes is a read: `Game::route_step` and `route_step_to_any` take
/// `&self`, as do `can_move`, `units_at` and `wdist`, and `think!` writes to the
/// reasoning journal, which is observer-only by contract. No RNG is drawn and no
/// board state is touched, so the anchor plays the identical game and only its
/// journal differs. A compatibility re-pin.
/// #1026 keeps the land army out of the water, behind `come_ashore` — `false`
/// in both `BasicAi` constructors and set only by `enable_live_bridge`. Every
/// one of its paths short-circuits on the flag before reading anything:
/// `explore_step`'s `dry_only` and `step_toward_range`'s and
/// `coordinated_tactical_step`'s `prefer_dry` are each `come_ashore && …`, both
/// `disembark_step` call sites are guarded by `if …come_ashore`, and
/// `peacetime_step`'s new `at_war` parameter is folded through
/// `at_war && self.come_ashore`, which reproduces the historical hardcoded
/// `false` exactly. So every configured, legacy and Elo agent explores and
/// moves exactly as it always did. A compatibility re-pin.
///
/// #1087 lets the baseline governor raise the housing ceiling — the Aqueduct
/// and the Neighborhood — behind `BasicAi::housing_districts`. That field is
/// `false` in both `BasicAi` constructors and is set only by
/// `enable_live_bridge`; the added block in `pick_item` short-circuits on it
/// before it reads a city, so `advanced_v1` ranks the same district families in
/// the same order. `Game::city_housing` is refactored onto `city_water` and
/// `city_housing_floor` without changing a single band, so the housing it
/// returns is unchanged for every caller. A compatibility re-pin.
///
/// #1095 keeps asking for a Campus in every city that can still repay one,
/// behind `AdvancedAi::campus_every_city` — `false` in the constructor and set
/// only by `enable_live_bridge`. Both of its paths short-circuit on the flag:
/// `balanced_core`'s exemption is `campus_every_city && family == "campus"`,
/// which is `false` for every legacy agent and reproduces the half-empire cliff
/// exactly, and the coverage term keeps `research_horizon` unless the flag is
/// set. So `advanced_v1` prices every district exactly as it did. A
/// compatibility re-pin.
///
/// #1099 puts `medina_quarter` and `insulae` in the deck when a city is short
/// of housing, behind `AdvancedAi::housing_cards` — `false` in the constructor
/// and set only by `enable_live_bridge`. The block short-circuits on the flag
/// before it reads a city, so every legacy and Elo agent slots exactly the cards
/// it always slotted. `Game::city_specialty_district_count` only widens from
/// private to `pub(crate)`. A compatibility re-pin.
///
/// ⚠ Re-pinned twice in this PR. The first version was inert — it patched
/// `BasicAi::tactical_step`, which a live probe showed the deployed controller
/// never calls; the working change is in
/// `AdvancedAi::coordinated_tactical_step`. Both edits touch anchored source,
/// so both moved this hash.
///
/// ⚠ And again for `blind_objective_units`, which is `false` in both `BasicAi`
/// constructors' downstream `AdvancedAi` defaults and set only by
/// `enable_live_bridge`. `local_strength_ratio`'s new term is
/// `if self.blind_objective_units { … } else { 0.0 }`, so with the flag off the
/// sum is arithmetically identical to before. A compatibility re-pin.
///
/// ⚠ And a third time on merging `origin/main`, which had re-pinned the same
/// constant for the `tactical_strategy` branch documented below. Neither hash
/// survives a merge of the two — the anchored source is now different from
/// both — so the value here is the one the test computes over the merged tree.
/// Both gating arguments still hold independently, which is what makes the
/// re-pin a compatibility one rather than a ledger break.
/// The tactical-role branch adds class assignments, projected return-fire,
/// wall/support coordination, and cavalry action priority behind
/// `BasicAi::tactical_strategy`. Both Basic constructors leave it `false`, and
/// `AdvancedAi::promoted_policy_envoy` alone enables it for the production
/// controller, so frozen Basic, configured, legacy and `advanced_v1` entrants
/// retain their old branches. A compatibility re-pin.
///
/// ⚠ And again for a warning fix. `science_goal_for_campus` bound a building's
/// name it never read; the loop now iterates the map's values. The anchor
/// hashes whole files, so a change that cannot alter behaviour still moves it.
/// That this one cannot was checked rather than argued: the same
/// `BTreeMap<String, BuildingSpec>` in the same key order, with the binding the
/// compiler proved unused removed. Seed 1002 was then played to completion on
/// both revisions through the same routes — turn 206, player 4, religious, all
/// six scores equal (Arabia 994, Aztec 592, Ethiopia 651, Georgia 1012, Khmer
/// 706, Maya 464), and the same 254 requests to get there. A compatibility
/// re-pin.
/// ⚠ And again for `relief_targets_the_siege`, which is `false` in every
/// `AdvancedAi` default and set only by `enable_live_bridge`. Its whole effect is
/// the leading component of one `min_by_key` in `domain_objective`, and with the
/// flag off that component is the constant `0` — so the ordering, and therefore
/// every objective any legacy or Elo entrant receives, is bit-for-bit what it was.
/// A compatibility re-pin.
///
/// ⚠ And again for the pantheon price, which now reads `Game::pantheon_faith_cost()`
/// instead of a bare `25.0` in the `ai.rs` gate. `pantheon_faith_cost` is
/// `game_speed.scale(PANTHEON_FAITH_STANDARD)`, and `GameSpeed::default()` is
/// `Standard`, whose `cost_percent` is 100 — so at the speed every legacy and Elo
/// entrant plays, the expression evaluates to exactly `25.0` and the gate is
/// bit-for-bit what it was. Only Online, Quick, Epic and Marathon move, and those
/// were charging a price the game does not.
/// ⚠ The value below is recomputed over the MERGED sources: main re-pinned this
/// constant for its own change while this branch was open, so neither side's
/// number is right after the merge — only a fresh fingerprint is.
/// `elo_anchor_speed_is_standard_so_the_pantheon_repin_is_free` checks the
/// Standard-speed claim rather than asserting it.
/// ⚠ Re-pinned for the unified timed-war appointment. The behavior is behind
/// `AdvancedAi::timed_war`, initialized `false` by `configured` and enabled
/// only by the evaluator-only `AdvancedAi::timing_attack` constructor. Every
/// shared call site short-circuits on an absent `war_plan`; frozen legacy and
/// `advanced_v1` therefore retain the same research, spending, production,
/// diplomacy, movement, and upgrade decisions. Focused construction tests
/// additionally assert that `advanced` reports the treatment off.
/// ⚠ Re-pinned for selective timing v2. Its additional chooser and launch
/// gates require both `timed_war` and `selective_timed_war`; both initialize
/// `false`, and only the evaluator-only `selective_timing_attack` constructor
/// enables them. The typed-arm test checks production `advanced`, v1, and v2
/// independently, while focused tests cover the selective-only branches.
/// ⚠ Re-pinned again for ready-force v3. `rapid_timed_war` also initializes
/// `false`, is enabled only by the evaluator constructor, and only narrows the
/// already-gated chooser before a `WarPlan` exists.
/// ⚠ And again for `settler_blocked_turns` surviving a retarget. That reset lives
/// AFTER `advanced_settler_step`'s `if !self.settler_commit { return moved; }`
/// early return, and `settler_commit` is `false` in every default constructor —
/// only `civvis_orders` turns it on for the live bridge. So the legacy and Elo
/// entrants return before the changed line is ever reached and the anchor's
/// behaviour is bit-for-bit what it was. A compatibility re-pin;
/// `elo_anchor_never_reaches_the_settler_commit_path` checks the claim.
/// ⚠ Re-pinned for test-only seeded-map fixture hardening after Natural
/// Wonder silhouettes changed. Both edits are inside `#[cfg(test)]` modules;
/// no controller path is compiled into an Elo game.
/// ⚠ Re-pinned for production unit-objective memory. The full objective,
/// danger, and retreat path is behind `BasicAi::unit_objective_memory`, which
/// initializes false in Basic and `AdvancedAi::legacy()` and true only in the
/// production Advanced constructor. The focused regression test asserts that
/// split and the production assignment; the frozen anchor never takes either
/// new movement branch.
/// ⚠ #1162 routes the charged Toa, Legion, and Nau through shared improvement
/// planning. `AdvancedAi::legacy()` and `BasicAi` can now select real new
/// improvement actions, so this is deliberately a protocol-v6 change rather
/// than a compatibility re-pin; the fresh source fingerprint documents that
/// the new ledger starts from this exact shared controller.
/// ⚠ 2026-08-04 prunes default-off experiments whose measured effects were
/// negative, inert, or inconclusive. A fixed-prefix `advanced_v1`/`basic`
/// comparison remains the compatibility check; this is a deliberate re-pin.
/// #1034 pulls the loyalty policy cards when a city is bleeding loyalty, behind
/// `loyalty_policy_defence` — `false` in `AdvancedAi::new()` and set only by
/// `enable_live_bridge`. `strategic_policies` reads the flag before it counts a
/// single city, so with it off the wishlist is byte-for-byte the old one and
/// every configured, legacy and Elo agent slots exactly the cards it always did.
/// A compatibility re-pin.
///
/// #1195 bounds only the live controller's global settlement-site search.
/// `AdvancedAi::legacy()` keeps `settlement_safety` disabled, so it returns
/// through the historical full-search path and the frozen `advanced_v1`
/// controller remains byte-identical. Compatibility re-pin; the Elo protocol
/// does not move.
///
/// #1204 makes action-family queries skip unrelated enumeration and removes
/// duplicate production-catalog work from the purchase-only projection.
/// `AdvancedAi::legacy()` retains the same action ordering and the BasicAI
/// purchase helper is outside the frozen controller's path. Clean `origin/main`
/// and candidate release builds produced byte-identical output from
/// `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
/// --jobs 1 --deployment-comparison`. Compatibility re-pin; the Elo protocol
/// does not move.
///
/// #1206 keeps the live settlement-growth beam's at-most-four selected plots
/// inline instead of allocating a `Vec` for each candidate branch.
/// `AdvancedAi::legacy()` keeps `settlement_safety` disabled, so the changed
/// forecast is outside the frozen controller's path. Clean `origin/main` and
/// candidate release builds again produced byte-identical output from
/// `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
/// --jobs 1 --deployment-comparison`. Compatibility re-pin; the Elo protocol
/// does not move.
///
/// #1209 partitions settlement-growth forecast layers at the existing beam
/// width before sorting the survivors. `AdvancedAi::legacy()` keeps
/// `settlement_safety` disabled, so the changed forecast is outside the frozen
/// controller's path. Clean `origin/main` and candidate release builds again
/// produced byte-identical output from `ai_eval advanced_v1 basic --pairs 10
/// --players 4 --turns 200 --seed 31337 --jobs 1 --deployment-comparison`.
/// Compatibility re-pin; the Elo protocol does not move.
///
/// #1217 reuses exact raw settlement-site values across the live controller's
/// local and global radius scans. The radius-specific penalties are still
/// applied at each call site, and a fixed-prefix `ai_eval advanced basic`
/// comparison produced byte-identical output on clean main and the candidate.
/// Compatibility re-pin; the Elo protocol does not move.
///
/// #1225 reuses the tile appeal computed by one `worthwhile_improvements` call
/// across that tile's candidate improvements. A fixed-prefix `ai_eval
/// advanced basic --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1
/// --deployment-comparison` comparison produced byte-identical output on
/// clean main and the candidate (SHA-256
/// `34c8ccea34d4bf3a8b60ae1b713f82bffbce77a5f1614f07d69db591d6287b24`).
/// #1227 stops the live religious buyer purchasing a Missionary into a tile that
/// already holds one of our religious units — the host refuses it outright with
/// "Too many units of the same class in this location.", 799 times across the
/// 08-04/08-05 runs. The guard is gated on `live_religious_purchase_guard`, like
/// the majority-religion check beside it, so the frozen controller is untouched:
/// `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
/// --jobs 1 --deployment-comparison` produced BYTE-IDENTICAL output from this
/// worktree with the change stashed and applied (same base, same build profile).
/// Compatibility re-pin; the Elo protocol does not move.
///
/// #1232 shares the radius-two position disk between settlement growth
/// forecasting and adjacency scoring inside one visible site valuation. The
/// legacy controller leaves the settlement-safety path disabled. A fixed-prefix
/// `ai_eval advanced basic --pairs 10 --players 4 --turns 200 --seed 31337
/// --jobs 1 --deployment-comparison` comparison produced byte-identical output
/// on clean main and the candidate (SHA-256
/// `34c8ccea34d4bf3a8b60ae1b713f82bffbce77a5f1614f07d69db591d6287b24`).
/// Compatibility re-pin; the Elo protocol does not move.
///
/// #1241 moves the friendly-city ownership check ahead of the static movement
/// predicate in `BasicAi::patrol_tile`. The predicate is unchanged; the new
/// order rejects the overwhelmingly common unowned map tile before asking the
/// traversal cache, so the frozen controller's source contract is re-pinned
/// after the fixed-prefix comparison below.
/// #1259 guards the special-improver helper at its call site. The guard repeats
/// the helper's existing eligibility checks, so the advanced_v1 legacy path is
/// unchanged; the source contract is re-pinned after the fixed-prefix
/// comparison above.
/// The Advanced parallel unit planner now primes frontier-post scans inside the
/// immutable batch snapshot, keyed by traversal class, so each worker reuses
/// the same read-only map scan without publishing it across a world mutation.
/// The fixed-prefix output remains byte-identical; compatibility is re-pinned.
/// `run_game` now also switches the narrated war ledger's per-action re-sync
/// off beside fog memory. The ledger is observation-only — no rule and no
/// built-in agent reads it, and declarations, peaces, and turn boundaries
/// still sync unconditionally — so the frozen controller's decisions are
/// unchanged and the source contract is re-pinned.
/// `faith_building_spending` now skips building the purchase menu whenever
/// the faith bank is below the reserve — the state in which the existing
/// filter provably rejects every candidate. Identical purchases in every
/// reachable state, so the frozen controller's decisions are unchanged and
/// the source contract is re-pinned.
/// `culture_focus` is removed from `BasicAi`: both constructors pinned it
/// `false` and nothing else ever set it, so its production blocks and the
/// `project_matches_focus` helper were unreachable — dead weight, not
/// behaviour. No reachable decision changes; the source contract is
/// re-pinned.
/// #1297 lets the strongest MET major weigh on the army target in PEACETIME,
/// behind `peacetime_deterrence` — `false` in `AdvancedAi::new()` and set only
/// by `enable_live_bridge`. `enemy_weighted_army_target` (the renamed
/// `wartime_army_target`; the wartime term is untouched) multiplies `shipped`
/// by 1.0 on that flag before it reads a single player, so every configured,
/// legacy and Elo agent wants exactly the army it always wanted. A
/// compatibility re-pin.
/// The `BasicAi` doc comment claiming CIVVIS "ordered an Entertainment
/// Complex zero times" is corrected — the census's name filter missed the
/// unique replacements (7 stood across 33 cities, as Hippodromes and a
/// Street Carnival) — and a test pins that a unique replacement belongs to
/// the family it replaces. A doc comment and one test; no executable path
/// changes, so advanced_v1 is byte-identical by construction. Compatibility
/// re-pin.
/// #1360 adds a bounded friendly-volley extension only under
/// `BasicAi::tactical_strategy`. `AdvancedAi::legacy()` leaves that flag false,
/// so its unit loop never asks for a paired friendly finisher or replaces a
/// reply price; this is a reviewed compatibility re-pin, not an Elo-protocol
/// change.
/// #1363 restores the joint tactical planner (`joint_tactics`, off everywhere
/// but the `advanced_joint_tactics` arm and the live bridge) and admits the
/// barbarian seat to the Advanced military step's enemy list behind
/// `home_defense`. `AdvancedAi::legacy()` leaves `home_defense` false and
/// `joint_tactics` false, so the anchor's path gains only inert fields and a
/// set-membership test against an empty set; the STOCK `advanced` entrant
/// (which ships `home_defense = true`) now answers barbarian raiders at home,
/// recorded here honestly. Compatibility re-pin for the anchor.
/// The Tactics arena adds an arena doctrine to both files, every part of it
/// behind `Game::is_arena()` — which is false for every world a rated game is
/// ever played on, because the Battlefield script is not a world and no
/// rating instrument accepts one. With the flag false the three touched
/// expressions reduce to the shipped ones by construction (`arena || x` is
/// `x`, `!arena && y` is `y`, and the weight pair is returned unchanged), so
/// the anchor's decisions are byte-identical. Reviewed compatibility re-pin,
/// not an Elo-protocol change.
/// #1386 makes production Scouts collect a tribal village they can currently
/// see and reach before another unseen exploration tile. The shared branch is
/// behind `BasicAi::tactical_strategy`: it is false for Basic and
/// `AdvancedAi::legacy()`, and `promoted_policy_envoy` enables it for the
/// production controller. The condition tests that flag before reading player
/// sight, reachability, or village state; the frozen path therefore proceeds
/// directly into its historical fog-target selection. The focused regression
/// asserts that split on the same staged board. A matched release
/// `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed 31337
/// --jobs 1 --deployment-comparison` report was byte-identical to the
/// then-current `origin/main` (SHA-256
/// `1bebbaa15ee7388b3d9427c1d49726d8e29b2328113c9b9409cb60bb7ae813e0`).
/// Compatibility re-pin, not an Elo-protocol change.
/// #1384 teaches the joint planner withdrawals and handoff steps and keeps the
/// per-unit movers off units the plan moved without a blow
/// (`tactics_withdrawn`). `AdvancedAi::legacy()` leaves `joint_tactics` false,
/// so the plan never runs, the set stays empty, and the anchor's only new
/// executable is a set-membership test against an empty set — the same shape
/// #1363 re-pinned. Compatibility re-pin over the #1382 merge.
/// #1393 adds the war-conversion trio (`war_economy`, `war_reinforcement`,
/// `war_patience`), off by default and set only by `enable_live_bridge`.
/// `AdvancedAi::legacy()` leaves all three false, so its new executable is
/// three short-circuiting flag tests: the production routing keeps its
/// historical arms (`false && _` adds nothing), both fatigue sites reduce to
/// the shipped expression (`!false &&` is identity), and
/// `wartime_reinforcement_step` returns `None` on its first line. The anchor's
/// decisions are byte-identical by construction. Compatibility re-pin, not an
/// Elo-protocol change.
/// #1399 breaks the Tactics-arena standoff by switching two pieces of
/// world-preservation logic off on a battlefield: the per-tile
/// local-superiority brake on closing moves, and the dangerous-approach
/// memory whose retreat floor assumes healing that an arena does not have.
/// Both sit behind a `!g.is_arena()` test, and `is_arena()` is false for
/// every world a rated game is played on, so the anchor's decision stream on
/// the rated profile is identical by construction — the same shape as the
/// flag re-pins above, with the map script as the flag. Compatibility
/// re-pin, not an Elo-protocol change.
/// The recon-replacement arm adds one disjunct to `pick_item`'s military-floor
/// condition and one new chooser, behind `BasicAi::recon_replacement`. Both
/// constructors leave the flag false and only `enable_live_bridge` sets it, so
/// under the anchor `recon_is_the_missing_arm` returns on its first line, the
/// added disjunct is a constant `false` that cannot change the `||`, and
/// `best_recon` is never reached. The anchor's build order is byte-identical by
/// construction. Compatibility re-pin, not an Elo-protocol change.
/// #1401 discounts a motionless Settler from the expansion gate's in-flight
/// test, so one stuck settler stops costing every future one. It sits behind
/// `BasicAi::settler_strand_discount`, which `AdvancedAi::enable_live_bridge`
/// sets and nothing else does — `BasicAi::new` and `with_weights` both leave
/// it false, and both new tests assert the off path is unchanged, so the
/// anchor's decision stream is identical by construction. Compatibility
/// re-pin, not an Elo-protocol change.
/// #1402 counts mirrored `UNIT_SPY` units as espionage agents in the spy
/// capacity test. A native CIVVIS Spy is a `Game::spies` entry and never a
/// unit — the production arm returns before `place_new_unit` — so the unit
/// census contributes 0 to `spy_agents` in every native game and the anchor
/// sees the same number it always did. Identical by construction, on a rated
/// profile and off it. Compatibility re-pin, not an Elo-protocol change.
/// #1404 adds the missing `disable_stranded_settler_discount` counterpart so
/// the treatment can be held off for a controlled arm. It only writes `false`
/// into a field the anchor already reads as `false`. Compatibility re-pin, not
/// an Elo-protocol change.
/// The siege-commitment term adds one summand to `campaign_city_value`, behind
/// `AdvancedAi::siege_commitment`. Both constructors leave the flag false and
/// only `enable_live_bridge` sets it, so under the anchor the `&&` chain
/// short-circuits on its first test, the term is `0.0`, and the returned score
/// is the shipped expression minus zero — the anchor's campaign ordering is
/// byte-identical by construction. Compatibility re-pin, not an Elo-protocol
/// change.
/// The wonder-ring settle credit (#1378) adds one term to
/// `settle_value_visible`, behind `BasicAi::wonder_ring_settle_value`. Both
/// constructors leave the flag false and only `enable_live_bridge` sets it,
/// so under the anchor `natural_wonder_ring_value` returns 0.0 on its first
/// line and the added `value += 0.0` cannot move any site score — and the
/// anchor reaches `settle_value_visible` only through constructors that keep
/// `settlement_safety` true, which `legacy()` does not. The anchor's settle
/// ordering is byte-identical by construction. Compatibility re-pin, not an
/// Elo-protocol change.
/// #1401 discounts a motionless Settler from the expansion gate's in-flight
/// test, so one stuck settler stops costing every future one. It sits behind
/// `BasicAi::settler_strand_discount`, which `AdvancedAi::enable_live_bridge`
/// sets and nothing else does — `BasicAi::new` and `with_weights` both leave
/// it false, and both new tests assert the off path is unchanged, so the
/// anchor's decision stream is identical by construction. Compatibility
/// re-pin, not an Elo-protocol change.
/// #1402 counts mirrored `UNIT_SPY` units as espionage agents in the spy
/// capacity test. A native CIVVIS Spy is a `Game::spies` entry and never a
/// unit — the production arm returns before `place_new_unit` — so the unit
/// census contributes 0 to `spy_agents` in every native game and the anchor
/// sees the same number it always did. Identical by construction, on a rated
/// profile and off it. Compatibility re-pin, not an Elo-protocol change.
/// #1404 adds the missing `disable_stranded_settler_discount` counterpart so
/// the treatment can be held off for a controlled arm. It only writes `false`
/// into a field the anchor already reads as `false`. Compatibility re-pin, not
/// an Elo-protocol change.
/// #1405 gives the baseline governor's building sort a housing term, behind
/// `BasicAi::housing_buildings`. The field is `false` in both `BasicAi`
/// constructors and set only by `enable_live_bridge`, and `housing_lift`
/// returns 0.0 whenever it is off, so the comparator is the identity it always
/// was on the anchor. Compatibility re-pin, not an Elo-protocol change.
#[cfg(test)]
/// Merging `origin/main` into this branch brings both sides' live-bridge
/// treatments into one `BasicAi`/`AdvancedAi`. Every one of them is off in both
/// constructors and set only by `enable_live_bridge`, so the anchor's decision
/// stream is unchanged by the union. Compatibility re-pin, not an Elo-protocol
/// change.
/// The capture-the-flag objective gives both controllers one new march: land
/// columns aim at the enemy's flag, via `Game::arena_enemy_flag`. It returns
/// `None` unless the battle was set up with flags — a shape that exists only
/// on a Tactics arena that asked for it, and on no world any rated game has
/// ever been played on — so both guards reduce to a `None` test and the
/// anchor's decision stream is identical by construction. The same shape as
/// the #1399 re-pin, with the objective as the flag. Re-pinned when the
/// objective became a flag per side taken from the enemy, rather than one
/// neutral flag raced for; the guard's shape did not change, only what it
/// returns on the arena that has flags at all. Pinned over the merge with
/// main's own re-pins, every one off in both constructors as their entries
/// above record. Compatibility re-pin, not an Elo-protocol change.
/// The garrison-walls arm adds one guarded branch to `pick_item` and its
/// chooser, behind `BasicAi::garrison_walls`. Both constructors leave the flag
/// false and only `enable_live_bridge` sets it, so under the anchor
/// `garrison_walls_item` returns `None` on its first line and the branch can
/// never take the build. The anchor's build order is byte-identical by
/// construction. Compatibility re-pin, not an Elo-protocol change.
/// ⚠ **The war-eve liquidation is NOT a free re-pin.** Every entry above ends
/// with a flag both constructors leave false; this one has no flag at all.
/// `BasicAi::war_eve_liquidation` runs from the shared `diplomacy` pass and
/// from `AdvancedAi`'s ordinary declaration, so the `advanced_v1` anchor really
/// does sell its cancellable promises before it declares, and its Gold, army,
/// and the victim's treasury all move. `ELO_PROTOCOL_VERSION` is bumped to 7
/// with this pin; see its own note for what stops comparing.
/// The settlement atlas reuses static site terms only while an active
/// battlefront frame and the live settlement-safety controller are present.
/// `AdvancedAi::legacy()` disables both `battlefront_observation` and
/// `settlement_safety`, so it stays on the historical uncached settlement
/// path. The production `advanced` controller does use the atlas, but the
/// frozen `advanced_v1` anchor cannot observe it. Compatibility re-pin, not
/// an additional Elo-protocol change.
/// The disposable speculative branch likewise changes only hypothetical
/// worlds: the fixed `advanced_v1`/`basic` prefix (`ai_eval advanced_v1 basic
/// --pairs 10 --players 4 --turns 200 --seed 31337 --jobs 1
/// --deployment-comparison`) remains byte-identical. Its source contract is
/// re-pinned below; the Elo protocol does not move.
/// The Holy Site figure moved onto `Weights::advanced()`, which only
/// `AdvancedAi::new()` reads. `AdvancedAi::legacy()` builds from
/// `BasicAi::new()` and therefore from `Weights::default()`, which still pays
/// `d_holy` 2.0 — the anchor keeps the exact weights it always had, and
/// `holy_lane_parity` is a flag both constructors leave false. Compatibility
/// re-pin, not an Elo-protocol change.
/// The district-weight guard and the roster-composite notes are a test and
/// doc comments in the hashed sources; no constructor moved and
/// `AdvancedAi::legacy()` is untouched. Compatibility re-pin.
/// `diplomatic_opening` is a flag both constructors leave false and
/// `diplomatic_opening_score` returns 0 without it, so the anchor never
/// reaches the new lane term. Compatibility re-pin.
/// `AdvancedAi::new()` is back on `Weights::default()`, which is the
/// weights `AdvancedAi::legacy()` always used, so the anchor is where it
/// has always been. Compatibility re-pin.
/// Doc comments only in the hashed sources: two of them claimed native
/// games leave `bounded_recovery` disabled when `promoted_policy_envoy`
/// enables it. No constructor moved and `legacy()` is untouched.
/// Compatibility re-pin.
/// A test doc comment only: the evidence ledger for what
/// `promoted_policy_envoy` enables. No constructor moved and
/// `legacy()` is untouched. Compatibility re-pin.
/// The production city-target floor was removed from
/// `promoted_policy_envoy`. `AdvancedAi::legacy()` builds through
/// `configured`, never that constructor, and its floor was and remains
/// 3 — pinned by the same test that caught this. Compatibility re-pin.
/// Three new `disable_*` withholds so every production flag can be
/// priced. They are evaluator entry points that no constructor calls;
/// `AdvancedAi::legacy()` never had these flags on. Compatibility
/// re-pin.
/// `enable_engine_repairs` and its war/economy halves, so the live-bridge
/// repair bundle can be priced natively. They are evaluator entry points
/// reached only by the three `advanced_synergy*` arms in `builtin_ai`;
/// no constructor calls them, `AdvancedAi::legacy()` builds through
/// `configured` and never had one of these flags on, and the addition is
/// purely additive — 115 lines, no deletions. Compatibility re-pin,
/// asserted rather than asserted-by-comment in
/// `the_repair_bundle_cannot_reach_the_frozen_anchor`.
/// Seven production category genes, and `production_value` multiplied by the
/// one that matches each candidate. Every gene defaults to 1.0 and the
/// multiply is applied only to a positive score, so a default genome — which
/// is what `AdvancedAi::legacy()` and `BasicAi::new()` both carry — ranks
/// builds bit-identically.
///
/// ⚠ This re-pin is **measured, not argued**. The list above contains one
/// justified by a comment that was wrong, so this one was checked against the
/// tree it claims to preserve: `ai_eval advanced advanced_v1
/// --deployment-comparison --players 4 --pairs 12 --turns 150 --seed
/// 91000000` was run on a build of `a2c8c7f` and on this branch, and the two
/// outputs are byte-identical across 24 games and 5,712 bytes of diagnostics —
/// including `advanced_v1`'s own per-seat cities, score, military and victory
/// types. Compatibility re-pin.
/// Two further `disable_*` withholds for base-constructor defaults
/// (`settlement_safety`, `battlefront_observation`). Evaluator entry points no
/// constructor calls; `legacy()` already turns both off. Compatibility re-pin
/// recomputed over the merged tree — neither side's value applies.
/// A test only: `the_withholdable_defaults_are_off_on_the_anchor_and_on_in_production`,
/// which asserts the claim the re-pins above made in prose — that no
/// withhold arm for a production default can reach `AdvancedAi::legacy()`.
/// Compatibility re-pin, and the last one on this branch that needs the
/// argument, because the assertion now carries it.
/// `settler_founds_when_stalled` and its `founds_where_it_stands`
/// branch. The flag defaults false in the struct init both `legacy()`
/// and `new()` build from, and the branch returns immediately without
/// it — asserted, not argued, in
/// `the_repair_bundle_cannot_reach_the_frozen_anchor`, which this
/// change extends. Compatibility re-pin.
/// `fortify_idle_units` and the `hold_stood_down_unit` branch that reads
/// it. Evaluator-only: the flag defaults false in the `BasicAi` init
/// both `legacy()` and `new()` build from, and the branch keeps its
/// original stand-down condition without it — asserted in
/// `the_repair_bundle_cannot_reach_the_frozen_anchor`, which this
/// change extends. Compatibility re-pin.
/// ⚠ **First city-state discovery is NOT a free re-pin.** The production
/// Scout's high-information frontier chooser is guarded by `tactical_strategy`,
/// which `AdvancedAi::legacy()` leaves off, but the corresponding first-contact
/// Envoy is a `Game` rule. Any controller can earn it by seeing a city-state,
/// so its influence thresholds and downstream choices differ in a native game.
/// `ELO_PROTOCOL_VERSION` is bumped to 8; the source contract is re-pinned for
/// the separately reviewed, legacy-gated Scout source edit.
/// `with_legacy_policy_deck` plus two comment corrections. The new
/// constructor is an evaluator entry point no other constructor calls,
/// and `AdvancedAi::legacy()` never routed through `production_weights`
/// so its deck was and remains `Legacy` — pinned by
/// `the_policy_deck_is_live_in_production_and_legacy_on_the_anchor`.
/// Compatibility re-pin.
/// `production_builder_floor` and the `delegated_cities` branch reading
/// it. The whole block is already behind `if !self.plan_city_target`,
/// which `AdvancedAi::legacy()` leaves false, so the anchor never
/// reaches it — and the flag defaults true so production is unchanged.
/// Compatibility re-pin.
/// `production_settler_deadline` and its `delegated_cities` branch, the
/// last production-only override to get a withhold. The whole block is
/// behind `if !self.plan_city_target`, which `AdvancedAi::legacy()`
/// leaves false, and the flag defaults true so production is unchanged.
/// Compatibility re-pin.
/// #1522 gates the Conquest wartime economy on a concrete objective:
/// `offensive_conquest` (a target city, a threatened city, or an active
/// major war) now decides the 2x-cities military target, the production
/// ceiling buffer, and the +160/+120 Conquest production bonuses; an
/// objective-less Conquest plan keeps the ordinary garrison. Measured on
/// the fixed prefix — `ai_eval advanced_v1 basic --pairs 10 --jobs 1
/// --seed 31337 --players 4 --turns 200 --deployment-comparison`, ci
/// profile, this branch against `main` at `5df102c4` — the two reports
/// are **byte-identical**: 85.0% paired-map score, 7 sweeps / 3 neutral,
/// 17/40 vs 3/40 seat wins, every metric equal across 20 games averaging
/// 131.9 turns, with conquest plans live on 14.1% of anchored all-game
/// seat-turns — so wherever the anchor's planner went Conquest, the gate
/// resolved the same wartime package as before. Compatibility re-pin;
/// the Elo protocol does not move.
/// The 2026-08-14 war-half removal: `promoted_policy_envoy` stops setting
/// `siege_muster`, `home_defense`, `tactical_strategy` and
/// `unit_objective_memory`, plus the alias declarations and doc updates
/// that ride with it. `AdvancedAi::legacy()` never routed through
/// `promoted_policy_envoy` and `BasicAi::new()` constructs all four flags
/// false, so the anchor's behaviour is unchanged; the four flags are now
/// false in production too and set only by `enable_live_bridge` (two of
/// them, as the `siege-muster`/`home-defense` treatments) and the
/// `advanced_war_half` re-addition arm. Compatibility re-pin; the Elo
/// protocol does not move.
/// Live strategic targeting now excludes unintroduced mirror seats behind
/// `battlefront_observation`. `AdvancedAi::legacy()` holds that flag false, so
/// both new predicates short-circuit to their historical forms; the focused
/// regression asserts that boundary. Compatibility re-pin; the Elo protocol
/// does not move.
/// The same live-only observation gate keeps a stale major-war defense from
/// becoming a counter-campaign at less than half the rival's power. The
/// frozen anchor's false gate preserves its historical denial path, asserted
/// in the regression. Compatibility re-pin; the Elo protocol does not move.
/// The live-only peacetime-deterrence gate now converts its raised defender
/// target into city queues before adaptive Science can refill them with
/// projects. `AdvancedAi::legacy()` leaves that gate false; the regression
/// asserts both the frozen project and the live defender. Compatibility re-pin;
/// the Elo protocol does not move.
/// The peaceful city-plan handoff is likewise unreachable to the frozen
/// anchor: its call site requires `victory_planning`, and its own gate is
/// `plan_city_target`, both false in `AdvancedAi::legacy()`. The regression
/// holds the anchor's research grant while the live plan replaces it with one
/// Settler. Compatibility re-pin; the Elo protocol does not move.
/// The named live-Great-Person gate is a host-only fact in
/// `Player::live_great_person_offer_blockers` and
/// `Player::live_great_person_offers`. `Game::new` and old saves leave the
/// latter `None`, making `great_person_class_offered_now` accept the native
/// roster; only `mirror.rs` writes Firaxis's current offer set. The assertion
/// below locks that boundary, so the source-contract re-pin does not silently
/// alter headless `advanced_v1` tournament rows. Compatibility re-pin; the Elo
/// protocol does not move.
/// The severe-Amenity project handoff is false for `AdvancedAi::legacy()` and
/// becomes live only through `enable_live_bridge` (or an explicit engine-repair
/// evaluation arm). The frozen `advanced_v1` controller retains its project
/// queues, so this is a compatibility re-pin rather than an Elo protocol move.
/// The related Liberalism relief uses that same false-by-default gate before it
/// reads a city Amenity or policy deck: only a live controller with two
/// developed, host-observed deficit cities can trade Aesthetics for the
/// immediately paying card. `AdvancedAi::legacy()` cannot enter the branch, so
/// this is also a compatibility re-pin rather than an Elo protocol move.
/// The opening Scout, six-city fog floor, civilian policy timing, government
/// prerequisite, major-war zero-damage siege handoff, stalled-Settler founding,
/// and first-Campus Writing handoff changes are all behind live-bridge treatment
/// flags. `first_campus_tech` short-circuits on `campus_every_city` before it
/// reads the board, and `AdvancedAi::legacy()` leaves that flag false; the
/// focused ablation tests lock the boundary. Compatibility re-pin; the Elo
/// protocol does not move.
/// Physical Great People that have no host-valid activation plot now add
/// mirror-only production and research needs. An unfinished host activation
/// district is also a map foundation, which reserves that family before a
/// second Spaceport can be ordered. `Game::new` leaves the need list empty, old
/// saves default it empty, and only `mirror.rs` populates it from a Firaxis unit
/// export. The assertion below locks that boundary, so the frozen headless
/// anchor cannot enter any of the new planning branches. Compatibility re-pin;
/// the Elo protocol does not move.
/// A named live Great Engineer can ask for a wonder only while the host has not
/// already refused a wonder in that city. This circuit breaker reads the same
/// mirror-only activation need and host-refusal map, both empty in ordinary and
/// frozen games; other cities remain eligible. Compatibility re-pin; the Elo
/// protocol does not move.
/// The wartime maintenance-card handoff requires `war_economy`, a zero
/// treasury, and an active major war. `AdvancedAi::legacy()` leaves
/// `war_economy` false, so it cannot enter the new policy branch. Compatibility
/// re-pin; the Elo protocol does not move.
/// The live war-production solvency handoff is gated by that same
/// `war_economy` flag. `AdvancedAi::legacy()` leaves it false, so its recovery
/// chooser and every production queue remain unchanged. Compatibility re-pin;
/// the Elo protocol does not move.
/// The local-defense handoff is likewise live-only: `garrison_under_fire`
/// changes the emergency chooser from a generic military pick to a
/// melee-capable land defender, lets the queue release replace a siege piece,
/// lets it start a defender after clearing a host-owned queue, and spends Gold
/// on that immediate defense before upgrades, patronage, or the ordinary
/// purchaser can choose a Builder or preserve its strategic reserve.
/// `AdvancedAi::legacy()` also leaves `amenity_project_preemption` false, so
/// it never reads the host-calibrated Amenity ledger or reserves an idle Arena
/// queue. The stricter broad-wartime reservation uses that same gate before it
/// can inspect an idle or repeatable queue; every frozen constructor returns
/// before reading a city. Compatibility re-pin; the Elo protocol does not
/// move.
/// A fresh direct declaration likewise observes the timed-war endgame reserve
/// only when `endgame_war_runway` is enabled through the live bridge. The
/// frozen anchor leaves that flag false, retaining its historical late-war
/// behavior. Compatibility re-pin; the Elo protocol does not move.
/// A home barbarian now lets only unclaimed live-bridge units retain their
/// pre-war campaign staging after the bounded garrison/defense responders get
/// first claim. `AdvancedAi::legacy()` leaves `home_defense` false, so it
/// never inserts the barbarian seat into this path. The fixed 10-pair
/// `advanced_v1`/`basic` seed prefix (31337 through 31346) matched the prior
/// 17/20 wins, 131.9 average turns, score, and per-seat metrics exactly.
/// Compatibility re-pin; the Elo protocol does not move.
/// The live wonder race is likewise gated by `live_wonder_race`, which only
/// `enable_live_bridge` sets: `AdvancedAi::legacy()` and every rated arm keep
/// the `Item::Wonder` refusal exactly as it was, so no headless anchor can enter
/// the new valuation branch. Compatibility re-pin; the Elo protocol does not
/// move.
/// The same live-only wonder race now closes during the empire-wide `Recovery`
/// posture, even if the individual building city is not yet threatened. The
/// frozen anchor leaves `live_wonder_race` false and cannot enter either
/// branch. Compatibility re-pin; the Elo protocol does not move.
/// A settler standing on a cached target that `can_found_city` now refuses
/// retires that target with the bounded avoidance the stall counter uses — behind
/// `settler_commit`, which `AdvancedAi::legacy()` leaves off
/// (`elo_anchor_never_reaches_the_settler_commit_path`); the re-validation of a
/// cached target also refuses `blocked_city_sites`, a set that is empty in every
/// ordinary and frozen game. Compatibility re-pin; the Elo protocol does not
/// move.
/// A new Settler target is forecast through the engine's Loyalty model only
/// while `loyalty_rate_alarm` is on. Both default constructors and the frozen
/// anchor leave that treatment flag false; the live bridge enables it with the
/// live Loyalty emergency handling. If every inspected target is immediately
/// doomed, the live controller holds rather than falling through to the
/// unaware baseline picker. Compatibility re-pin; the Elo protocol does not
/// move.
/// A missing siege/recon arm now owns a city queue only when that city can
/// actually build the requested role. Both `siege_role` and
/// `recon_replacement` remain disabled by `AdvancedAi::legacy()`, so the
/// frozen anchor retains its prior production path. Compatibility re-pin; the
/// Elo protocol does not move.
/// The second settler pipeline slot is behind `parallel_settlers`, which only
/// the Civilization VI bridge sets (`AdvancedAi::enable_parallel_settlers`); every
/// native constructor and `AdvancedAi::legacy()` keep the one-at-a-time gate in
/// both settler routes (asserted). Compatibility re-pin; the Elo protocol does not
/// move.
/// `war_patience` is now bounded by `WAR_PATIENCE_LIMIT_TURNS`; the flag is set
/// only by the live bridge and the native repair bundle, never by
/// `AdvancedAi::legacy()`, so the anchor's peace rules are unchanged.
/// Compatibility re-pin; the Elo protocol does not move.
/// The threat-aware guard wait lives inside `stacked_escort_pace`, behind
/// `stacked_escort`, which only the live bridge and the native repair bundle set;
/// `AdvancedAi::legacy()` never reaches it. Compatibility re-pin; the Elo
/// protocol does not move.
/// Naval units now count in `settlement_tile_risk` on coastal tiles, and a
/// threatened settler retreats before any hold; both live under
/// `settlement_safety`/`stacked_escort`, which `AdvancedAi::legacy()` leaves off.
/// Compatibility re-pin; the Elo protocol does not move.
/// The stalemate posture is behind `war_patience`, which `AdvancedAi::legacy()`
/// never sets; the anchor's grand-strategy selection is unchanged (asserted).
/// Compatibility re-pin; the Elo protocol does not move.
/// The generic Wonder fallback reads mirror-only `blocked_wonders`, which
/// ordinary/headless games never populate. Compatibility re-pin; the Elo
/// protocol does not move.
/// Live war patience now recognizes only an observed foreign city changing
/// hands, so a fresh settlement cannot prolong a stale war; the frozen anchor
/// never enables `war_patience`. Compatibility re-pin; the Elo protocol does
/// not move.
/// The hosted-amenity and regional-reach pricing is behind
/// `amenity_district_path`, which only the live bridge and the native repair
/// bundle set; `AdvancedAi::new()` and `AdvancedAi::legacy()` price the
/// Entertainment Complex exactly as before (asserted). Compatibility re-pin;
/// the Elo protocol does not move.
/// A live wonder race now rejects a data-marked religion-founding site after
/// its civilization has already founded a religion. `live_wonder_race` remains
/// false for the frozen anchor, so its historical wonder choices cannot enter
/// the new guard. Compatibility re-pin; the Elo protocol does not move.
/// The Prophet deferral is behind `expansion_before_prophet`, which only the
/// live bridge sets (Firaxis-only); `AdvancedAi::new()` and `legacy()` enter
/// the Prophet race with two cities exactly as before (asserted).
/// A battlefront-observing controller now requires a legal, known enemy city
/// before a Conquest denial may replace a stalled war's economic plan; raw
/// leader pressure remains available to Congress and in-lane counters, while
/// `advanced_v1` retains its historical all-information path (asserted).
/// The wonder lanes are behind `live_wonder_race`, which only the live bridge
/// sets; `AdvancedAi::new()` and `legacy()` refuse wonders exactly as before
/// (asserted).
/// Compatibility re-pin; the Elo protocol does not move.
/// (asserted). Compatibility re-pin; the Elo protocol does not move.
/// The host Settler population floor is behind `BasicAi::host_settler_pop`,
/// set only by the Civilization VI bridge; every native constructor and
/// `AdvancedAi::legacy()` keep the genome's `settler_min_pop` (asserted).
/// Compatibility re-pin; the Elo protocol does not move.
/// The elective-war stand-down is behind `no_elective_war`, which only the
/// live bridge sets (Firaxis-only); `AdvancedAi::new()` and `legacy()` take
/// the "strong enough" branch exactly as before (asserted). Compatibility
/// re-pin; the Elo protocol does not move.
/// The war-patience reference is read only under `war_patience`, which the
/// frozen anchor never sets. Compatibility re-pin; the Elo protocol does not
/// move.
/// A catastrophic multi-front Recovery peace proposal is also behind that
/// same live-only `war_patience` gate: with it false, the anchor keeps
/// protecting its active campaign target exactly as before. Compatibility
/// re-pin; the Elo protocol does not move.
/// The wonder-race scale is read only under `live_wonder_race`, which the
/// frozen anchor never sets. Compatibility re-pin; the Elo protocol does not
/// move.
/// The wall-tech research goal is behind `garrison_walls`, the live walls
/// doctrine, which the frozen anchor never sets (asserted). Compatibility
/// re-pin; the Elo protocol does not move.
/// A stalled Settler's known-hostile-frontier rejection is read only under the
/// live `loyalty_rate_alarm`; the frozen anchor cannot enter the guard, so its
/// historical fallback remains intact (asserted). Compatibility re-pin; the
/// Elo protocol does not move.
/// A cached settlement target's arrival forecast reads the same live-only
/// `loyalty_rate_alarm`; normal and frozen controllers retain their historical
/// cached-target founding behavior (asserted). Compatibility re-pin; the Elo
/// protocol does not move.
/// The exploration dead-target memory is behind `BasicAi::explore_dead_targets`,
/// set only by the Civilization VI bridge; native constructors and
/// `AdvancedAi::legacy()` keep the plain goal (asserted). Compatibility re-pin;
/// the Elo protocol does not move.
/// The foreign-border settle penalty is behind `settlement_safety`, which
/// `AdvancedAi::legacy()` leaves off (asserted). Compatibility re-pin; the Elo
/// protocol does not move.
/// The Amenity-repair band gate sits inside `amenity_districts`, which every
/// native constructor and the frozen anchor leave off. Compatibility re-pin;
/// the Elo protocol does not move.
/// The every-lane governor routing is behind `governor_every_lane`, which only
/// the live bridge and the native repair bundle set; `AdvancedAi::new()` and
/// `legacy()` keep the historical routing (asserted). Compatibility re-pin; the
/// Elo protocol does not move.
/// A plan-confirmed pre-damage barbarian siege reuses the live-only
/// `garrison_under_fire` gate; the frozen anchor never enables it and therefore
/// keeps its historical queue commitments (asserted). Compatibility re-pin; the
/// Elo protocol does not move.
/// The settler retreat limit lives inside the retreat step, behind
/// `stacked_escort`/`settlement_safety`, which `AdvancedAi::legacy()` leaves
/// off; carrying retired sites across rebuilds touches only unit-keyed memory.
/// Compatibility re-pin; the Elo protocol does not move.
/// The pre-declaration maintenance reserve is guarded by the live-only
/// `war_economy` flag, which the frozen anchor never enables. Its Conquest
/// portfolio therefore keeps the historical order until the live bridge opts
/// into the named-campaign recovery guard (asserted). Compatibility re-pin; the
/// Elo protocol does not move.
/// The value below is recomputed after both live-only changes are combined.
/// The garrisoned-city raid gate is behind `garrison_under_fire`, the live
/// doctrine that owns the besieged-city path; frozen controllers keep the raid
/// test as it was (asserted). Compatibility re-pin; the Elo protocol does not
/// move.
/// The adjacent-guard march is behind `stacked_escort`/`settlement_safety`,
/// which `AdvancedAi::legacy()` leaves off. Compatibility re-pin; the Elo
/// protocol does not move.
/// The fog-read city ceiling is behind `fog_land_capacity` under
/// `wide_map_capacity`, both live-only and off for `AdvancedAi::legacy()`
/// (asserted); a native board carries no unknown terrain, so the estimate
/// equals the count there. Compatibility re-pin; the Elo protocol does not
/// move.
/// The recon flight step is behind `recon_flight`, off for
/// `AdvancedAi::legacy()` (asserted); the frozen anchor's scouts explore
/// exactly as before. Compatibility re-pin; the Elo protocol does not move.
/// The embarked-settler sea link is skipped only under `stacked_escort`,
/// which `AdvancedAi::legacy()` leaves off (asserted); the frozen anchor
/// still links a ship to a settler at sea. Compatibility re-pin; the Elo
/// protocol does not move.
/// The turn-limit horizon on the space race and the nuclear lane is behind
/// `score_horizon`, off for `AdvancedAi::legacy()` (asserted); the frozen
/// anchor races and arms exactly as before. Compatibility re-pin; the Elo
/// protocol does not move.
/// The sea's recon arm — the one-ship purchase and the naval explorer — is
/// behind `naval_recon`, off for `AdvancedAi::legacy()` (asserted); the
/// frozen anchor's ships and production are unchanged. Its viable-waterway
/// and lake-bound-hull refinements remain behind that same gate. Compatibility
/// re-pin; the Elo protocol does not move.
/// The in-lane answer to a Science or score leader is behind
/// `counter_in_lane`, which the live bridge now enables and
/// `AdvancedAi::legacy()` leaves off (asserted); the frozen anchor still
/// declares. Compatibility re-pin; the Elo protocol does not move.
/// The era-paced city cadence is behind `era_paced_expansion`, off for
/// `AdvancedAi::legacy()` (asserted); the frozen anchor still adds a city
/// per ninety standard turns. Compatibility re-pin; the Elo protocol does
/// not move.
/// The tally price of culture is behind `tally_culture`, off for
/// `AdvancedAi::legacy()` (asserted); the frozen anchor's lanes keep their
/// bred yield weights and district table. Compatibility re-pin; the Elo
/// protocol does not move.
/// The frontier-loyalty settle rule is behind `frontier_loyalty`, off for
/// `AdvancedAi::legacy()` (asserted); the frozen anchor's settle forecast is
/// unchanged. Compatibility re-pin; the Elo protocol does not move.
/// The banked envoy and its final-tier, secure-suzerain marginal-return cap
/// are behind `bank_envoys`, and the committed outward exploration goal
/// behind `BasicAi::explore_commit`, all set only by the Civilization VI
/// bridge and off for `AdvancedAi::new()` and `AdvancedAi::legacy()`
/// (asserted); the frozen anchor spends every envoy and re-derives its
/// scout's goal each turn as before. Compatibility re-pin; the Elo protocol
/// does not move.
/// The settler-target hysteresis is behind `settler_target_hysteresis`, off
/// for `AdvancedAi::legacy()` (asserted); the frozen anchor's settler
/// re-picks exactly as before. Compatibility re-pin; the Elo protocol does
/// not move.
/// The tally price of a Great Person is behind `tally_great_people`, off for
/// `AdvancedAi::legacy()` (asserted); the frozen anchor's patronage keeps its
/// closeness limit. Compatibility re-pin; the Elo protocol does not move.
/// The frontier-loyalty rule is now a distance test (own city within nine
/// tiles), still behind `frontier_loyalty` and off for `AdvancedAi::legacy()`
/// (asserted). Compatibility re-pin; the Elo protocol does not move.
/// The barbarian-scout exemption in the settlement risk model is behind
/// `barbarian_scouts_are_scouts`, off for `AdvancedAi::legacy()` (asserted);
/// the frozen anchor prices every hostile as before. Compatibility re-pin;
/// the Elo protocol does not move.
/// The nine-tile camp reach is behind `BasicAi::camp_reach`, off for
/// `AdvancedAi::legacy()` (asserted); the frozen anchor's home guard keeps
/// the six-tile radius for camps and raiders alike. Compatibility re-pin;
/// the Elo protocol does not move.
/// The frontier-loyalty reach moves from nine to seven tiles, still behind
/// `frontier_loyalty` and off for `AdvancedAi::legacy()` (asserted).
/// Compatibility re-pin; the Elo protocol does not move.
/// The strategic governor's Expansion routing is behind `governor_every_lane`,
/// off for `AdvancedAi::legacy()` (asserted); the frozen anchor's baseline
/// still governs its Expansion lane. Compatibility re-pin; the Elo protocol
/// does not move.
/// The settler stack discipline (settlers decide before the engagement,
/// capture priced as capture, only a guard on the tile counts, bound guards
/// kept out of the joint plan) is behind `settler_stack_discipline`, and the
/// peacetime camp party (the whole field army answers home threats, a camp in
/// reach outranks the countryside, the party sized to the camp's defender)
/// behind `BasicAi::camp_party`; both off for `AdvancedAi::legacy()`
/// (asserted). Compatibility re-pin; the Elo protocol does not move.
/// `recon_is_the_missing_arm` counts a recon unit already in a city queue as
/// the arm being rebuilt (still behind `recon_replacement`, off for
/// `AdvancedAi::legacy()`), and `BasicAi::skip_opening_book` lets a decider
/// restarted mid-game leave the four-build book behind it — the frozen
/// anchor's opening is unchanged. Compatibility re-pin; the Elo protocol does
/// not move.
/// The live envoy bank gates both the plan-aware scorer and the later
/// `BasicAi` fallback, while `AdvancedAi::legacy()` keeps both historical
/// paths enabled. Compatibility re-pin; the Elo protocol does not move.
/// A looped reconnaissance target is retired only behind
/// `explore_dead_targets`, which the Firaxis order bridge explicitly enables;
/// `AdvancedAi::legacy()` keeps that flag off. Compatibility re-pin; the Elo
/// protocol does not move.
/// The idle Entertainment Complex reservation is behind
/// `amenity_project_preemption`, which both `AdvancedAi::legacy()` and the
/// stock constructor keep off (asserted in
/// `the_repair_bundle_cannot_reach_the_frozen_anchor`). Compatibility re-pin;
/// the Elo protocol does not move.
/// A repeatable district project waits behind the Library, University,
/// Research Lab or Workshop its city can already build, behind
/// `buildings_before_projects`, off for `AdvancedAi::legacy()` (asserted).
/// Compatibility re-pin; the Elo protocol does not move.
/// The live recon arm keeps a second Scout only after city two, still behind
/// `recon_replacement`, which `AdvancedAi::legacy()` leaves off. Its missing-arm
/// predicate therefore returns on the same first-line flag check in every
/// frozen game; the anchor's production decisions remain byte-identical.
/// Compatibility re-pin; the Elo protocol does not move.
/// A second already-built sea hull may explore only behind `naval_recon`, which
/// `AdvancedAi::legacy()` leaves off. The frozen anchor still gets an empty
/// explorer set before inspecting units, so its movement decisions are
/// byte-identical. Compatibility re-pin; the Elo protocol does not move.
/// The recon-flight loop escape is reached only from `recon_flight`; that
/// live-only flag is false in `AdvancedAi::legacy()`, so a frozen Scout keeps
/// its historical flight and exploration behavior. Compatibility re-pin; the
/// Elo protocol does not move.
/// The hostile-Suzerain peace path is reached only through `bank_envoys`,
/// which the Firaxis order bridge enables after profitable Envoy placements
/// have already run. `AdvancedAi::legacy()` keeps that gate false, so its
/// diplomacy remains historical. Compatibility re-pin; the Elo protocol does
/// not move.
/// The wartime second naval eye and its idle-city reservation are both reached
/// only through `naval_recon`, which `AdvancedAi::legacy()` leaves false.
/// Compatibility re-pin; the Elo protocol does not move.
/// The bounded Envoy liquidity reserve is reached only through `bank_envoys`,
/// false in `AdvancedAi::legacy()`. Compatibility re-pin; the Elo protocol
/// does not move.
/// A campaign-target Suzerain cannot make the peace needed for an Envoy
/// reclaim, so it no longer inflates that live-only liquidity reserve.
/// Compatibility re-pin; the Elo protocol does not move.
/// A major-war campaign now keeps its already chosen enemy city until capture,
/// a target change, or an emergency. That condition is inside
/// `siege_commitment`, which `AdvancedAi::legacy()` leaves false; the frozen
/// anchor therefore continues to refresh the city ranking as before.
/// Compatibility re-pin; the Elo protocol does not move.
/// The developed-city-state contact sweep's third Scout stays behind
/// `recon_replacement`, false in `AdvancedAi::legacy()`. Compatibility re-pin;
/// the Elo protocol does not move.
/// Patronage skips both a Great Person class the mirrored host reports
/// exhausted (`live_great_person_exhausted`, read through
/// `great_person_class_earnable`) and one absent from its current
/// `live_great_person_offers` screen; native boards carry neither list and are
/// unchanged. Compatibility re-pin; the Elo protocol does not move.
const ADVANCED_V1_SOURCE_CONTRACT_FNV: u64 = 0xd215_4140_b41f_04c1;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TournamentEntrant {
    identity: String,
    controller: String,
}

/// Parse `rating-identity=controller`, with a bare name meaning both.
///
/// Separating the two lets a changing builtin enter a persistent ledger under
/// a new immutable identity while still constructing the existing controller.
fn parse_tournament_entrants(spec: &str) -> Result<Vec<TournamentEntrant>, String> {
    let mut entrants = Vec::new();
    for raw in spec.split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("--ais contains an empty entrant".to_string());
        }
        let (identity, controller) = raw.split_once('=').unwrap_or((raw, raw));
        let identity = identity.trim();
        let controller = controller.trim();
        if identity.is_empty() || controller.is_empty() {
            return Err(format!(
                "invalid tournament entrant {raw:?}; use rating-identity=controller"
            ));
        }
        entrants.push(TournamentEntrant {
            identity: identity.to_string(),
            controller: controller.to_string(),
        });
    }
    Ok(entrants)
}

fn arg(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn arg_f64(args: &[String], key: &str, default: f64) -> f64 {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn strict_i64_arg(args: &[String], key: &str, default: i64) -> Result<i64, String> {
    match args.iter().position(|arg| arg == key) {
        Some(index) => {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{key} needs a value"))?;
            value
                .parse::<i64>()
                .map_err(|_| format!("{key} needs an integer, got {value:?}"))
        }
        None => Ok(default),
    }
}

fn strict_f64_arg(args: &[String], key: &str, default: f64) -> Result<f64, String> {
    match args.iter().position(|arg| arg == key) {
        Some(index) => {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{key} needs a value"))?;
            value
                .parse::<f64>()
                .map_err(|_| format!("{key} needs a number, got {value:?}"))
        }
        None => Ok(default),
    }
}

fn arg_text(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// The turn budget the named game speed ships with, which `--turns` overrides.
///
/// Every path that judges which AI is stronger has to play a whole game. A
/// short cap does not just shorten the game, it changes who won: over 9336
/// six-seat league games capped at 250 turns, 81.8% ended on the cap, no game
/// ever ended on a natural score victory, and domination and science never
/// happened at all. Replaying 24 seeds at both budgets, the cap names a
/// different winner in 13 of them.
fn stock_turns(args: &[String]) -> i64 {
    let rules = Rules::embedded();
    let speed = arg_text(args, "--speed", &default_speed());
    rules
        .speeds
        .get(&speed)
        .map(|spec| i64::from(spec.turns))
        .unwrap_or(500)
}

fn victory_conditions(args: &[String]) -> VictoryConditions {
    let Some(enabled) = args
        .iter()
        .position(|value| value == "--victories")
        .and_then(|index| args.get(index + 1))
    else {
        return VictoryConditions::default();
    };
    VictoryConditions::parse(enabled).unwrap_or_else(|why| {
        eprintln!(
            "--victories: {why}; choose from {:?}",
            VictoryConditions::NAMES
        );
        std::process::exit(2);
    })
}

/// A lobby checkbox, spelled the several ways a preset or a script writes one.
fn arg_toggle(args: &[String], key: &str, default: bool) -> bool {
    let Some(value) = args
        .iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
    else {
        return default;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => true,
        "off" | "false" | "no" | "0" => false,
        other => {
            eprintln!("{key} takes on or off, got {other:?}");
            std::process::exit(2);
        }
    }
}

fn auto_cs(args: &[String], players: i64) -> usize {
    let cs = arg(args, "--city-states", -1);
    if cs < 0 {
        return MapSize::for_players(players.max(1) as usize).default_city_states;
    }
    // The engine seats only as many city-states as the ruleset has distinct
    // identities for, because each one owns a unique Suzerain bonus and two
    // seats sharing a name would share it. Asking for more used to be clamped
    // in silence, which turns a pinned lobby setting into a different game
    // than the one that was set up.
    let named = civvis::game::CITY_STATE_NAMES.len();
    if cs as usize > named {
        eprintln!(
            "--city-states {cs} exceeds the {named} city-states the ruleset carries; \
             each owns a unique Suzerain bonus, so the extra seats cannot be filled"
        );
        std::process::exit(2);
    }
    cs as usize
}

fn auto_dimension(args: &[String], key: &str, players: i64, width: bool) -> i32 {
    // A Tactics arena is not sized like a world. Left to the world ladder,
    // `--map battlefield` produced an eighty-hex "arena" that two eight-unit
    // armies could spend a whole battle failing to find each other on; the
    // mode's own smallest field is the honest default, and `--width` and
    // `--height` still name any other.
    let (default_width, default_height) = if map_script(args).is_battlefield() {
        let script = map_script(args);
        let sizes = setup::battlefield_sizes();
        let arena = sizes
            .iter()
            .find(|size| size.script == script)
            .copied()
            .unwrap_or(sizes[0]);
        // A scenario is drawn at the size of its chart and no other: resizing
        // it would read the chart through a window and lose the coastline the
        // battle was fought against. So this one is not a default, and
        // `--width`/`--height` are declined rather than obeyed.
        if script.is_scenario() {
            return if width { arena.width } else { arena.height };
        }
        (arena.width, arena.height)
    } else {
        // A globe stores itself in a rectangle of its own shape, so the size's
        // default dimensions depend on which world shape was asked for.
        MapSize::for_players(players.max(1) as usize).dimensions(map_topology(args))
    };
    arg(
        args,
        key,
        if width { default_width } else { default_height } as i64,
    ) as i32
}

/// The world type asked for, as every command reads it.
fn map_script(args: &[String]) -> MapScript {
    MapScript::from_id(&arg_text(args, "--map", "tennis_ball")).unwrap_or(MapScript::TeninsBall)
}

/// The world's shape, which is asked for separately from what fills it.
/// Fixed geography changes where the land comes from, not which shape it is
/// sampled onto: even True Start Earth can be a flat atlas or a globe.
fn map_topology(args: &[String]) -> MapTopology {
    // New games open on a globe; Flat remains an explicit opt-in shape.
    MapTopology::from_id(&arg_text(args, "--shape", MapTopology::Planet.id()))
        .unwrap_or(MapTopology::Planet)
}

/// Whether the world has cold ends.
fn map_poles(args: &[String]) -> MapPoles {
    MapPoles::from_id(&arg_text(args, "--poles", "poles")).unwrap_or_default()
}

/// Which published game's rules the world is played by.
fn base_ruleset(args: &[String]) -> BaseRuleset {
    let id = arg_text(args, "--ruleset", BaseRuleset::default().id());
    BaseRuleset::from_id(&id).unwrap_or_else(|| {
        eprintln!("unknown ruleset {id:?}; this build models civ6");
        std::process::exit(2);
    })
}

/// How far into history the game opens.
///
/// A rung of the ladder that is declared but not built yet is refused here
/// rather than quietly played as the Ancient era — the whole point of listing
/// it is that it is not the same game.
/// The era every civilization opens in.
///
/// `--start-era random` is the training lane's answer to overfitting. A
/// Tactics sweep fought only from the Ancient era teaches an AI Ancient-era
/// tactics — slingers and warriors on open ground — and nothing about
/// crossbows behind walls or armour in the open. Varying the opening across
/// the ladder spreads a sweep over the whole unit roster instead.
///
/// The roll comes from the game's own seed rather than a fresh source, so a
/// soak stays exactly reproducible: the same `--start-seed` replays the same
/// eras in the same order. The mix is there because consecutive seeds are
/// consecutive integers, and taking those modulo the ladder length directly
/// would march through the eras in lockstep with the seed instead of
/// scattering them.
/// The arena economy a run plays under, read from the `--tactics-*` flags.
///
/// Every launch path needs its own call because they build their world
/// differently — `soak` and `simulate` through `game_options`, `tournament`
/// through its own per-game `GameOptions`, and `play` through
/// `server::Params` — and each one that forgets accepts the flags and
/// silently ignores them. Both of the others did, in turn; this is the single
/// reader they now share.
fn tactics_rules(args: &[String]) -> setup::TacticsRules {
    let stock = setup::TacticsRules::default();
    setup::TacticsRules {
        cities: arg(args, "--tactics-cities", i64::from(stock.cities)).max(0) as u8,
        production: arg(args, "--tactics-production", i64::from(stock.production)).max(0) as u32,
        gold: arg(args, "--tactics-gold", i64::from(stock.gold)).max(0) as u32,
        turns_per_tech: arg(args, "--tactics-turns-per-tech", i64::from(stock.turns_per_tech))
            .max(0) as u32,
        turn_limit: arg(args, "--tactics-turn-limit", i64::from(stock.turn_limit)).max(0) as u32,
        best_of: arg(args, "--tactics-best-of", i64::from(stock.best_of)).max(1) as u32,
        unique_units: flag_or(args, "--tactics-unique-units", stock.unique_units),
        fog: flag_or(args, "--tactics-fog", stock.fog),
        flag: flag_or(args, "--tactics-flag", stock.flag),
        // The command line's era is `--start-era`, which these flags leave
        // alone: a run's era is part of the experiment, not the economy.
        era: stock.era,
    }
    .sanitized()
}

/// The civilizations named on the command line, in seat order.
fn named_civs(args: &[String]) -> Vec<String> {
    arg_text(args, "--civs", &arg_text(args, "--civ", ""))
        .split(',')
        .map(|civ| civ.trim().to_string())
        .filter(|civ| !civ.is_empty())
        .collect()
}

/// Which roster the seats are drawn from.
fn leader_pool(args: &[String]) -> LeaderPool {
    let id = arg_text(args, "--leader-pool", LeaderPool::default().id());
    let pool = LeaderPool::from_id(&id).unwrap_or_else(|| {
        eprintln!("unknown leader pool {id:?}; choose civ6, historical, or today");
        std::process::exit(2);
    });
    if !pool.is_available() {
        eprintln!("leader pool {id:?} has no supplied roster data yet");
        std::process::exit(2);
    }
    pool
}

/// How deep the AI player pool runs: which of the rated strategies may be
/// seated for the game's AI civilizations.
fn ai_player_pool(args: &[String]) -> setup::AiPlayerPool {
    let id = arg_text(args, "--ai-pool", setup::AiPlayerPool::default().id());
    setup::AiPlayerPool::from_id(&id).unwrap_or_else(|| {
        eprintln!("unknown AI player pool {id:?}; choose best1, best2, best3, best5, or all");
        std::process::exit(2);
    })
}

/// The civilizations a Tactics match is between, resolved the same way the
/// engine seats them.
///
/// A match has to know its two contenders *before* the first battle, because
/// it swaps the sides over between battles and keeps the score by
/// civilization. Naming them explicitly for every battle also makes the
/// pairing a property of the match rather than of the seating order the stock
/// fill happened to produce.
fn match_contenders(args: &[String], players: i64, chosen: &[String]) -> Vec<String> {
    let rules = Rules::embedded();
    let mut known: std::collections::BTreeSet<civvis::name::Name> =
        rules.civs.keys().cloned().collect();
    known.extend(
        leader_roster::all()
            .iter()
            .filter(|record| record.available)
            .map(|record| civvis::name::Name::new(&record.civ)),
    );
    civvis::game::seat_civs(players.max(1) as usize, chosen, &known, leader_pool(args))
}

/// An on/off flag that keeps its default when absent, and reads the usual
/// spellings of both answers when present: `--flag`, `--flag on|off`,
/// `true|false`, `yes|no`, `1|0`.
fn flag_or(args: &[String], key: &str, default: bool) -> bool {
    let Some(index) = args.iter().position(|arg| arg == key) else {
        return default;
    };
    match args.get(index + 1).map(String::as_str) {
        Some("on" | "true" | "yes" | "1") | None => true,
        Some("off" | "false" | "no" | "0") => false,
        // A value that is not an answer is the next flag: `--tactics-unique-
        // units --games 8` asks for unique units and eight games.
        Some(_) => true,
    }
}

fn start_era(args: &[String], seed: u64) -> usize {
    let id = arg_text(args, "--start-era", setup::stock_start_era_id());
    if id == "random" {
        return setup::random_start_era(seed);
    }
    setup::start_era_from_id(&id).unwrap_or_else(|| {
        let playable: Vec<&str> = setup::playable_start_eras().map(|spec| spec.id).collect();
        let known = setup::START_ERAS.iter().any(|spec| spec.id == id);
        if known {
            eprintln!("cannot open in the {id:?} era yet; choose one of: {}", playable.join(", "));
        } else {
            eprintln!("unknown start era {id:?}; choose one of: {}", playable.join(", "));
        }
        std::process::exit(2);
    })
}

/// Which rules the far end of the game is played by.
///
/// Same contract as `--start-era`: an era that is declared but not built is
/// refused rather than quietly played as the classic one. The Modified Future
/// Era can also be had as what it is made of — `--mods
/// mods/modified-future-era` loads the same overlay off disk.
fn future_era(args: &[String]) -> setup::FutureEra {
    let id = arg_text(args, "--future-era", setup::FutureEra::default().id());
    setup::future_era_from_id(&id).unwrap_or_else(|| {
        let playable: Vec<&str> = setup::FUTURE_ERAS
            .iter()
            .filter(|spec| spec.is_playable())
            .map(|spec| spec.id)
            .collect();
        eprintln!("unknown Future Era {id:?}; choose one of: {}", playable.join(", "));
        std::process::exit(2);
    })
}

/// Whether the seats of a game turn act one after another or plan against a
/// shared snapshot and commit together. Same contract as the eras: an unknown
/// id is refused rather than quietly played as some stock regime.
///
/// The default is the caller's to name, but today every caller names
/// `Sequential`: the product is hard-committed to sequential turns, and the
/// simultaneous driver is a retained research regime reached only through
/// this explicit flag. `TurnStructure::default()` itself stays `Sequential`
/// as the save-compatibility and setup-contract anchor.
fn turn_structure(args: &[String], default: setup::TurnStructure) -> setup::TurnStructure {
    let id = arg_text(args, "--turn-structure", default.id());
    setup::turn_structure_from_id(&id).unwrap_or_else(|| {
        let known: Vec<&str> = setup::TURN_STRUCTURES.iter().map(|spec| spec.id).collect();
        eprintln!("unknown turn structure {id:?}; choose one of: {}", known.join(", "));
        std::process::exit(2);
    })
}

/// Difficulty and speed are chosen the same way everywhere: by name, against
/// the shipped ruleset, with the stock levels as defaults. The turn-structure
/// default is the command's own (see [`turn_structure`]).
fn game_options(
    args: &[String],
    players: i64,
    seed: u64,
    default_structure: setup::TurnStructure,
) -> GameOptions {
    let rules = Rules::embedded();
    let difficulty = arg_text(args, "--difficulty", &default_difficulty());
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!(
            "unknown difficulty {difficulty:?}; choose one of {:?}",
            ladder(&rules)
        );
        std::process::exit(2);
    }
    let speed = arg_text(args, "--speed", &default_speed());
    let Some(speed_spec) = rules.speeds.get(&speed) else {
        eprintln!("unknown game speed {speed:?}; choose one of {:?}", speeds(&rules));
        std::process::exit(2);
    };
    let tactics = tactics_rules(args);
    // An explicit --turns wins; otherwise every speed brings its own stock
    // budget (Standard is 500 turns / 2050 AD). Short historical defaults
    // ended games at the turn limit before the science, culture, and
    // diplomatic lanes could finish, which handed the win to whoever was
    // ahead on score at an arbitrary cutoff.
    let turns = if args.iter().any(|a| a == "--turns") {
        arg(args, "--turns", speed_spec.turns as i64)
    } else if map_script(args).is_battlefield() {
        // A Tactics battle keeps a battle's clock rather than a game speed's
        // five hundred turns. Its own four-step ladder names the stock
        // deadline; the general `--turns` flag above still wins explicitly.
        i64::from(tactics.turn_limit)
    } else {
        speed_spec.turns as i64
    };
    let player_count = players.max(1) as usize;
    let teams_arg = arg_text(args, "--teams", "");
    let teams = if teams_arg.trim().is_empty() {
        Vec::new()
    } else {
        let parsed: Result<Vec<Option<usize>>, _> = teams_arg
            .split(',')
            .map(|team| {
                let team = team.trim();
                if team.is_empty() || team == "-" {
                    Ok(None)
                } else {
                    team.parse::<usize>().map(Some)
                }
            })
            .collect();
        let teams = parsed.unwrap_or_else(|_| {
            eprintln!("invalid --teams value {teams_arg:?}; use comma-separated team numbers or -");
            std::process::exit(2);
        });
        if teams.len() != player_count {
            eprintln!(
                "--teams needs exactly {player_count} entries (one per major player), got {}",
                teams.len()
            );
            std::process::exit(2);
        }
        teams
    };
    let leader_pool = leader_pool(args);
    GameOptions {
        base_ruleset: base_ruleset(args),
        start_era: start_era(args, seed),
        // The arena's economy, so a headless sweep can vary the thing it is
        // training against without going through a lobby. Ignored on a world.
        tactics,
        future_era: future_era(args),
        map_script: map_script(args),
        map_topology: map_topology(args),
        map_poles: map_poles(args),
        difficulty,
        speed,
        // A headless game has nobody at the keyboard, so the difficulty only
        // reaches the AI side of the ladder unless a seat is named human.
        human_seats: arg_text(args, "--human-seats", "")
            .split(',')
            .filter_map(|seat| seat.trim().parse().ok())
            .collect(),
        teams,
        leader_pool,
        // Who the player is. `--civ Egypt` seats Egypt at seat 0; `--civs
        // Egypt,Rome` names the leading seats in order. Anything unnamed
        // falls back to the selected stock roster, and a name outside that
        // roster is refused here rather than silently ignored downstream.
        civs: {
            let named = arg_text(args, "--civs", &arg_text(args, "--civ", ""));
            let chosen: Vec<String> = named
                .split(',')
                .map(|civ| civ.trim().to_string())
                .filter(|civ| !civ.is_empty())
                .collect();
            for civ in &chosen {
                if !leader_roster::entry(civ).is_some_and(|entry| {
                    entry.available && entry.pool == leader_pool
                }) {
                    let mut known: Vec<&str> = leader_pool
                        .entries()
                        .map(|entry| entry.civ.as_str())
                        .collect();
                    known.sort_unstable();
                    eprintln!(
                        "civilization {civ:?} is not available in {}: choose one of {known:?}",
                        leader_pool.name()
                    );
                    std::process::exit(2);
                }
            }
            chosen
        },
        // Gathering Storm's lobby slider: 0 turns random disasters off,
        // 4 is Hyperreal. Sea-level rise follows CO2 either way.
        disaster_intensity: {
            let intensity = arg(args, "--disasters", i64::from(DEFAULT_DISASTER_INTENSITY));
            if !(0..=4).contains(&intensity) {
                eprintln!("--disasters takes 0 (none) to 4 (hyperreal), got {intensity}");
                std::process::exit(2);
            }
            intensity as u8
        },
        // A lobby checkbox like any other: competitive team events play with
        // barbarians off, so a preset has to be able to say so.
        barbarians: arg_toggle(args, "--barbarians", true),
        // Off is the stock Gathering Storm ruleset and what every tournament
        // lobby plays; naming a mode is an opt-in to New Frontier content.
        game_modes: {
            let requested = arg_text(args, "--game-modes", "");
            let modes: BTreeSet<String> = requested
                .split(',')
                .map(str::trim)
                .filter(|mode| !mode.is_empty())
                .map(str::to_string)
                .collect();
            for mode in &modes {
                if !GAME_MODES.contains(&mode.as_str()) {
                    eprintln!("unknown game mode {mode:?}; choose from {GAME_MODES:?}");
                    std::process::exit(2);
                }
            }
            modes
        },
        turn_structure: turn_structure(args, default_structure),
        ..GameOptions::new(
            player_count,
            auto_dimension(args, "--width", players, true),
            auto_dimension(args, "--height", players, false),
            seed,
            turns as u32,
            auto_cs(args, players),
        )
    }
}

fn ladder(rules: &Rules) -> Vec<&str> {
    let mut names: Vec<&str> = rules.difficulties.keys().map(|k| k.as_str()).collect();
    names.sort_by_key(|name| rules.difficulties[*name].order);
    names
}

fn speeds(rules: &Rules) -> Vec<&str> {
    let mut names: Vec<&str> = rules.speeds.keys().map(|k| k.as_str()).collect();
    names.sort_by_key(|name| rules.speeds[*name].order);
    names
}

fn standings(g: &Game) {
    if g.is_draw() {
        println!("Draw: turn limit reached on turn {}", g.reported_turn());
    }
    match g.winner {
        Some(winner) => {
            let w = &g.players[winner];
            // The label, not the bare type: this line is how a played game
            // announces its result, and a Mercy Rule ending has a lane to
            // name. The fixed-width victory columns in the batch and audit
            // tables below keep the type — they are a tabulation to scan, and
            // one of them is a key things are counted under.
            println!(
                "Winner: {} (player {}) by {} on turn {}",
                w.civ,
                w.id,
                g.victory_label().unwrap_or_default(),
                g.reported_turn()
            );
        }
        None if !g.is_draw() => println!(
            "No winner: turn {} of {}, and no enabled victory was achieved",
            g.turn, g.max_turns
        ),
        None => {}
    }
    let mut majors: Vec<usize> = g
        .players
        .iter()
        .filter(|p| !p.is_minor)
        .map(|p| p.id)
        .collect();
    majors.sort_by_key(|pid| -g.score(*pid));
    for pid in majors {
        let p = &g.players[pid];
        let cities = g.player_city_ids(pid);
        let pop: i32 = cities.iter().map(|c| g.cities[c].pop).sum();
        // The army roster is the one part of an empire the score never shows,
        // and the place where a missing rule hides longest.
        let mut roster: BTreeMap<&str, usize> = BTreeMap::new();
        for unit in g.units.values() {
            if unit.owner == pid && g.rules.units[unit.kind].class == "military" {
                *roster.entry(unit.kind.as_str()).or_default() += 1;
            }
        }
        let mut army: Vec<(&str, usize)> = roster.into_iter().collect();
        army.sort_by_key(|(kind, count)| (std::cmp::Reverse(*count), *kind));
        let army: Vec<String> = army
            .iter()
            .map(|(kind, count)| {
                let stale = if g.unit_is_obsolete(pid, civvis::name::Name::new(kind)) { "*" } else { "" };
                format!("{count}x{kind}{stale}")
            })
            .collect();
        println!(
            "  {:<10} score={:<4} cities={} pop={} techs={} {}",
            p.civ,
            g.score(pid),
            cities.len(),
            pop,
            p.techs.len(),
            if p.alive { "" } else { "(eliminated)" }
        );
        if !army.is_empty() {
            println!("             army: {}", army.join(" "));
        }
    }
    let minors: Vec<&str> = g
        .players
        .iter()
        .filter(|p| p.is_minor && !p.is_barbarian)
        .map(|p| p.civ.as_str())
        .collect();
    if !minors.is_empty() {
        println!("  City-states: {}", minors.join(", "));
    }
}

/// Available batch workers default to one per core. An explicit `--jobs`
/// always wins, while a single simulation has its own bounded default below.
fn jobs_arg(args: &[String]) -> usize {
    let requested = arg(args, "--jobs", 0);
    if requested > 0 {
        requested as usize
    } else {
        civvis::parallel::default_jobs()
    }
}

/// Independent frontiers inside one simulation share cloned worlds, so their
/// useful parallelism reaches its measured knee before every host core. Keep
/// an explicit `--jobs` authoritative and leave outer multi-game workloads on
/// [`jobs_arg`]'s one-core-per-job default.
const SINGLE_SIMULATION_DEFAULT_MAX_JOBS: usize = 4;

fn single_simulation_jobs_arg(args: &[String]) -> usize {
    let requested = arg(args, "--jobs", 0);
    if requested > 0 {
        requested as usize
    } else {
        civvis::parallel::default_jobs().min(SINGLE_SIMULATION_DEFAULT_MAX_JOBS)
    }
}

/// Split one process-wide worker budget across a simultaneous soak's active
/// games. The first `extra` game indices receive one additional seat planner,
/// making the total exact without letting nested game and seat fan-outs
/// oversubscribe the host.
fn simultaneous_soak_job_split(games: usize, jobs: usize) -> (usize, usize, usize) {
    let jobs = jobs.max(1);
    let concurrent_games = games.max(1).min(jobs);
    (
        concurrent_games,
        jobs / concurrent_games,
        jobs % concurrent_games,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    // Mods replace the ruleset for the whole process, so they have to be
    // installed before anything reads it.
    let mod_paths = civvis::mods::parse_arg(&arg_text(&args, "--mods", ""));
    if !mod_paths.is_empty() {
        match civvis::mods::activate(&mod_paths) {
            Ok(loaded) => {
                for info in loaded {
                    let about = if info.description.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", info.description)
                    };
                    println!("mod: {} ({}){about}", info.name, info.files.join(", "));
                }
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    }
    match cmd {
        "simulate" => {
            let players = arg(&args, "--players", 4);
            let g0 = Instant::now();
            // The product is hard-committed to sequential turns, so every
            // command defaults to the regime the shipped game plays;
            // `--turn-structure simultaneous` remains the explicit research
            // escape hatch into the retained driver.
            let mut g = Game::new_with(game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
                setup::TurnStructure::Sequential,
            ));
            // The two regimes want opposite parallelism. Sequential seats
            // cannot deliberate concurrently, so `--jobs` feeds the clone-
            // heavy WorkPool frontiers inside one seat's turn, whose measured
            // knee caps the default at four. Simultaneous seats deliberate
            // independently by construction, so `--jobs` fans whole seats out
            // instead — one clone buys a whole turn of deliberation, past the
            // knee — and the AIs skip the inner pool rather than stack the
            // two layers.
            let census = if g.turn_structure == setup::TurnStructure::Simultaneous {
                let jobs = jobs_arg(&args);
                let mut ais = AdvancedAi::fleet(&g);
                civvis::simultaneous::run_structured_jobs(&mut g, &mut ais, jobs)
            } else {
                let jobs = single_simulation_jobs_arg(&args);
                let mut ais = AdvancedAi::fleet_parallel(&g, jobs);
                civvis::simultaneous::run_structured(&mut g, &mut ais)
            };
            println!("[{:.3}s]", g0.elapsed().as_secs_f64());
            if let Some(census) = census {
                println!("{}", census.summary());
            }
            standings(&g);
        }
        "soak" => {
            let players = arg(&args, "--players", 4);
            let games = arg(&args, "--games", 10);
            let start = arg(&args, "--start-seed", 0);
            let jobs = jobs_arg(&args);
            let simultaneous = turn_structure(&args, setup::TurnStructure::Sequential)
                == setup::TurnStructure::Simultaneous;
            // A sequential soak has only one useful frontier: independent
            // games. Simultaneous games have a second one inside each game —
            // every ready civilization can plan at once. Keep the total
            // worker budget bounded by splitting it across the games that can
            // be live at once, then hand each game's share to its persistent
            // seat-planning fleet. With one large simultaneous game, this
            // therefore reaches every requested core instead of treating the
            // one outer job as a reason to run all of its seats serially.
            let (concurrent_games, jobs_per_game, extra_seat_workers) = if simultaneous {
                simultaneous_soak_job_split(games as usize, jobs)
            } else {
                (jobs, 1, 0)
            };
            // A Tactics match: the same two civilizations over a series of
            // battles, sides swapped between them. `--games` is how many
            // battles are actually played, so a best-of-5 run short of five
            // games simply reports the score it reached.
            let arena = map_script(&args).is_battlefield();
            let match_rules = tactics_rules(&args);
            let contenders = (arena && match_rules.best_of > 1)
                .then(|| match_contenders(&args, players, &named_civs(&args)));
            // Each game is played on an outer worker, then described on the
            // main one, so a soak reads exactly as it did when it was serial.
            let lines = civvis::parallel::map(games as usize, concurrent_games, |index| {
                let seed = start + index as i64;
                let t0 = Instant::now();
                let contenders = contenders.clone();
                let result = std::panic::catch_unwind(|| {
                    let mut options = game_options(
                        &args,
                        players,
                        seed as u64,
                        setup::TurnStructure::Sequential,
                    );
                    if let Some(contenders) = contenders {
                        // The sides change ends at half time, and every
                        // battle after it. Otherwise a series measures the
                        // corner one civilization kept sitting in as much as
                        // it measures the civilization.
                        options.civs = contenders;
                        if index % 2 == 1 {
                            options.civs.reverse();
                        }
                    }
                    let mut g = Game::new_with(options);
                    let mut ais = AdvancedAi::fleet(&g);
                    let simultaneous = if g.turn_structure == setup::TurnStructure::Simultaneous {
                        // Spread a non-divisible budget across the first live
                        // games; later replacements get the base share, so
                        // the running total never exceeds `--jobs`.
                        let seat_jobs = jobs_per_game + usize::from(index < extra_seat_workers);
                        civvis::simultaneous::run_structured_jobs(&mut g, &mut ais, seat_jobs)
                    } else {
                        civvis::simultaneous::run_structured(&mut g, &mut ais)
                    };
                    // Every major's turns, pooled: what the empires in this game
                    // actually spent the game doing.
                    let mut census = civvis::ai::StrategyCensus::default();
                    for ai in ais.iter().take(g.players.iter().filter(|p| !p.is_minor).count()) {
                        census.absorb(&ai.strategy_census());
                    }
                    (g, census, simultaneous)
                });
                match result {
                    Ok((g, census, simultaneous)) => {
                        let majors: Vec<_> = g.players.iter().filter(|p| !p.is_minor).collect();
                        let minors: Vec<_> = g
                            .players
                            .iter()
                            .filter(|p| p.is_minor && !p.is_barbarian)
                            .collect();
                        // A soak line describes a terminal result. Tactics
                        // draws carry no winner but are finished battles.
                        let w = g.winner.map(|winner| &g.players[winner]);
                        let mut flags = String::new();
                        if let Some(simultaneous) = &simultaneous {
                            flags.push_str(&format!(
                                " SIMUL drops={}/{}{}",
                                simultaneous.dropped,
                                simultaneous.planned,
                                if simultaneous.aborted { " ABORTED" } else { "" }
                            ));
                        }
                        if majors.iter().all(|p| p.techs.len() <= 2) {
                            flags.push_str(" NO-TECH-PROGRESS");
                        }
                        if w.is_some_and(|w| w.is_minor) {
                            flags.push_str(" MINOR-WINNER");
                        }
                        if g.is_draw() {
                            flags.push_str(" DRAW");
                        } else if w.is_none() {
                            flags.push_str(" NO-WINNER");
                        }
                        // An army nobody ever modernizes is invisible in the
                        // standings and obvious on the map. Count the units
                        // still fielded after their owner retired them, and
                        // the ones three eras behind the world besides.
                        let unit_era = |kind: &str| -> usize {
                            let spec = &g.rules.units[kind];
                            let tech = spec
                                .tech
                                .as_deref()
                                .and_then(|node| g.rules.techs.get(node))
                                .map(|node| node.era);
                            let civic = spec
                                .civic
                                .as_deref()
                                .and_then(|node| g.rules.civics.get(node))
                                .map(|node| node.era);
                            tech.or(civic).unwrap_or(0)
                        };
                        let (obsolete, ancient, army) = majors
                            .iter()
                            .filter(|p| p.alive)
                            .flat_map(|p| {
                                g.units.values().filter(move |unit| unit.owner == p.id)
                            })
                            .filter(|unit| g.rules.units[unit.kind].class == "military")
                            .fold((0, 0, 0), |(obsolete, ancient, army), unit| {
                                (
                                    obsolete + g.unit_is_obsolete(unit.owner, unit.kind) as i32,
                                    ancient
                                        + (g.world_era.saturating_sub(unit_era(&unit.kind)) >= 3)
                                            as i32,
                                    army + 1,
                                )
                            });
                        flags.push_str(&format!(
                            " ARMY {army} obsolete={obsolete} ancient={ancient} era={}",
                            g.world_era
                        ));
                        // A war nobody ever wins is as invisible as an army
                        // nobody modernizes: the standings only show who was
                        // left standing, so a game where every declaration
                        // ended in a white peace reads exactly like a game of
                        // uninterrupted peace. Count what the declarations
                        // actually achieved.
                        // `Game::wars` holds only the wars still running:
                        // `close_war_record` removes a finished one and pushes
                        // it to `concluded_wars`. Reading the live map alone
                        // therefore hid every war that ended — which is every
                        // war that was *won*, and every white peace this block
                        // was written to make legible. Measured over eight
                        // six-player games it saw 6 of 39 declarations, 96 of
                        // 317 unit losses, and 0 of 13 city captures, while
                        // `ended_in_peace` could not be anything but zero.
                        let all_wars: Vec<&WarRecord> =
                            g.wars.values().chain(g.concluded_wars.iter()).collect();
                        let wars = all_wars.len();
                        let (units_lost, cities_taken) = all_wars.iter().fold(
                            (0u32, 0u32),
                            |(units, cities), war| {
                                (
                                    units + war.losses.values().map(|side| side.units).sum::<u32>(),
                                    cities
                                        + war.losses.values().map(|side| side.cities).sum::<u32>(),
                                )
                            },
                        );
                        let capitals_taken = all_wars
                            .iter()
                            .flat_map(|war| war.highlights.iter())
                            .filter(|highlight| highlight.kind == "capital_captured")
                            .count();
                        // How long the declarations lasted, because a war that
                        // ends in a handful of turns cannot take a walled city
                        // whatever army was pointed at it.
                        let (turns_at_war, ended) = all_wars.iter().fold(
                            (0u32, 0usize),
                            |(turns, ended), war| {
                                let stop = war.ended.unwrap_or(g.turn);
                                (
                                    turns + stop.saturating_sub(war.started),
                                    ended + war.ended.is_some() as usize,
                                )
                            },
                        );
                        let mean_war = if wars > 0 {
                            turns_at_war as f64 / wars as f64
                        } else {
                            0.0
                        };
                        // Every kind of thing the declarations produced, so a war
                        // that only ever produces its own declaration is
                        // legible as exactly that.
                        let mut events: BTreeMap<&str, usize> = BTreeMap::new();
                        for highlight in all_wars.iter().flat_map(|war| war.highlights.iter()) {
                            *events.entry(highlight.kind.as_str()).or_default() += 1;
                        }
                        let events: Vec<String> = events
                            .iter()
                            .map(|(kind, count)| format!("{kind}:{count}"))
                            .collect();
                        flags.push_str(&format!(
                            " WAR {wars} units_lost={units_lost} cities_taken={cities_taken} \
                             capitals_taken={capitals_taken} mean_turns={mean_war:.0} \
                             ended_in_peace={ended} events=[{}]",
                            events.join(" ")
                        ));
                        // A war is only prosecuted if somebody chose to
                        // prosecute it. Recovery is the defensive posture, so
                        // turns spent there are turns nobody was besieging
                        // anything.
                        let total = census.total().max(1);
                        let share = |turns: u32| 100 * turns / total;
                        flags.push_str(&format!(
                            " PLAN conquest={}% recovery={}% expansion={}% science={}% \
                             culture={}% religion={}% diplomacy={}%",
                            share(census.conquest),
                            share(census.recovery),
                            share(census.expansion),
                            share(census.science),
                            share(census.culture),
                            share(census.religion),
                            share(census.diplomacy),
                        ));
                        let posture_total = census.posture_total().max(1);
                        let pshare = |turns: u32| 100 * turns / posture_total;
                        flags.push_str(&format!(
                            " FORCE engage={}% advance={}% hold={}% muster={}% recover={}%",
                            pshare(census.engage),
                            pshare(census.advance),
                            pshare(census.hold),
                            pshare(census.muster),
                            pshare(census.recover),
                        ));
                        flags.push_str(&format!(
                            " SIEGE blows={} damage={} walls_breached={} cities_reduced={} \
                             left_depleted={} taker_ready={} melee_was_there={}",
                            g.siege.blows,
                            g.siege.damage,
                            g.siege.walls_breached,
                            g.siege.cities_reduced,
                            g.siege.left_depleted,
                            g.siege.depleted_with_a_taker_ready,
                            g.siege.reduced_with_melee_adjacent,
                        ));
                        let held = (census.hold_threatened + census.hold_weak).max(1);
                        flags.push_str(&format!(
                            " HELD_BY threatened_city={}% locally_weak={}%",
                            100 * census.hold_threatened / held,
                            100 * census.hold_weak / held,
                        ));
                        Some((w.map(|w| w.civ.clone()), format!(
                            "seed {:3}  t{:<4} {:<10} {:<8} majors_alive={}/{} cities={:<2} cs_alive={}/{} [{:.2}s]{}",
                            seed,
                            g.reported_turn(),
                            g.victory_type.clone().unwrap_or_default(),
                            w.map_or("-", |w| w.civ.as_str()),
                            majors.iter().filter(|p| p.alive).count(),
                            majors.len(),
                            g.cities.len(),
                            minors.iter().filter(|p| p.alive).count(),
                            minors.len(),
                            t0.elapsed().as_secs_f64(),
                            flags
                        )))
                    }
                    Err(_) => None,
                }
            });
            let mut fails = 0;
            let mut series = contenders
                .map(|civs| setup::MatchSeries::new(match_rules.best_of, civs));
            for (index, line) in lines.into_iter().enumerate() {
                match line {
                    Some((winner, line)) => {
                        println!("{line}");
                        if let Some(series) = series.as_mut() {
                            // A match stops at the battle that settles it;
                            // the rest of `--games` is a dead rubber and is
                            // reported as unplayed rather than counted.
                            if !series.decided() {
                                series.record(winner.as_deref());
                            }
                        }
                    }
                    None => {
                        fails += 1;
                        println!("seed {:3}  CRASH (panic)", start + index as i64);
                    }
                }
            }
            println!("\n{}/{} games completed", games - fails, games);
            if let Some(series) = series {
                let verdict = match series.winner() {
                    Some(civ) => format!("{civ} takes the match"),
                    None if series.played() >= series.best_of => "match drawn".to_string(),
                    None => format!(
                        "match unfinished: {} of {} battles played",
                        series.played(),
                        series.best_of
                    ),
                };
                println!("best of {}: {} — {verdict}", series.best_of, series.scoreline());
            }
            if fails > 0 {
                std::process::exit(1);
            }
        }
        // Would ending decided games early change who wins? 62% of audited
        // league games ran to the turn cap, where the winner is whoever is
        // biggest — most of that tail is compute spent on a settled outcome.
        // The spectator ribbon already carries a calibrated live win
        // probability (`odds::table`, Brier- and log-loss-checked at three
        // phases of completed games). This audit plays every game to its real
        // end while recording, per threshold, the first world turn the odds
        // leader crossed it — so agreement between the crossing pick and the
        // played-out winner, and the turns adjudication would have saved, are
        // measured exactly rather than estimated. It changes no outcome and
        // is the pre-registered evidence gate for enabling adjudication in
        // any loop (docs/ADJUDICATION.md).
        "odds-audit" => {
            let players = arg(&args, "--players", 6);
            let games = arg(&args, "--games", 40);
            let start = arg(&args, "--start-seed", 0);
            let jobs = jobs_arg(&args);
            let start_turn = arg(&args, "--adjudicate-start", 100) as u32;
            let every = (arg(&args, "--every", 5).max(1)) as u32;
            let thresholds: Vec<f64> = arg_text(&args, "--thresholds", "0.90,0.95,0.98,0.995")
                .split(',')
                .map(|t| t.trim().parse::<f64>().expect("--thresholds wants numbers"))
                .collect();
            println!(
                "odds-audit: {games} games, {players} players, sampling from turn \
                 {start_turn} every {every}, thresholds {thresholds:?}"
            );
            type GameAudit = (Option<usize>, Option<String>, u32, Vec<Option<(usize, u32)>>);
            let results: Vec<Option<GameAudit>> =
                civvis::parallel::map(games as usize, jobs, |index| {
                    let seed = start + index as i64;
                    std::panic::catch_unwind(|| {
                        let mut g = Game::new_with(game_options(
                            &args,
                            players,
                            seed as u64,
                            setup::TurnStructure::Sequential,
                        ));
                        let mut ais = AdvancedAi::fleet(&g);
                        // Same display-state elisions as `run_game`: this is a
                        // headless rollout and the odds read none of them.
                        g.set_fog_memory(false);
                        g.set_war_ledger(false);
                        let mut crossings: Vec<Option<(usize, u32)>> =
                            vec![None; thresholds.len()];
                        let mut last_turn = g.turn;
                        while g.winner.is_none() && g.turn <= g.max_turns {
                            let pid = g.current;
                            ais[pid].take_turn(&mut g, pid);
                            if g.winner.is_none() && g.current == pid {
                                let _ = g.apply(pid, &civvis::game::Action::EndTurn);
                            }
                            if g.turn == last_turn {
                                continue;
                            }
                            last_turn = g.turn;
                            let due = g.turn >= start_turn && (g.turn - start_turn) % every == 0;
                            if !due || g.winner.is_some() || crossings.iter().all(Option::is_some)
                            {
                                continue;
                            }
                            // A flat 1500 prior for every seat: the audit runs
                            // stock fleets with no roster, so only the board
                            // terms — score, military, cities, victory races,
                            // and the clock — separate the table.
                            let table = civvis::odds::table(&g, |_pid| 1500.0f64);
                            let Some((leader, seat)) =
                                table.iter().max_by(|a, b| a.1.now.total_cmp(&b.1.now))
                            else {
                                continue;
                            };
                            for (slot, threshold) in thresholds.iter().enumerate() {
                                if crossings[slot].is_none() && seat.now >= *threshold {
                                    crossings[slot] = Some((*leader, g.turn));
                                }
                            }
                        }
                        let end_turn = g.turn.min(g.max_turns);
                        (g.winner, g.victory_type.clone(), end_turn, crossings)
                    })
                    .ok()
                });
            let mut crashes = 0;
            let mut finished: Vec<(i64, GameAudit)> = Vec::new();
            for (index, result) in results.into_iter().enumerate() {
                let seed = start + index as i64;
                match result {
                    Some(audit) => finished.push((seed, audit)),
                    None => {
                        crashes += 1;
                        println!("seed {seed:4}  CRASH (panic)");
                    }
                }
            }
            for (seed, (winner, victory, end_turn, crossings)) in &finished {
                let victory = victory.as_deref().unwrap_or("none");
                let verdicts: Vec<String> = crossings
                    .iter()
                    .zip(&thresholds)
                    .map(|(crossing, threshold)| match crossing {
                        Some((pid, turn)) => format!(
                            "{threshold}:t{turn}{}",
                            if Some(*pid) == *winner { "=" } else { "!" }
                        ),
                        None => format!("{threshold}:-"),
                    })
                    .collect();
                println!(
                    "seed {seed:4}  t{end_turn:<4} {victory:<10} {}",
                    verdicts.join(" ")
                );
            }
            // The turn cap awards the score victory, so `score` endings are
            // truncations of an undecided board and every other ending is a
            // game the rules finished. Agreement is reported for both because
            // they answer different questions: natural endings are ground
            // truth, cap endings are agreement with the truncation rule that
            // adjudication would replace.
            println!(
                "\nthreshold  crossed  agree-all      agree-natural  agree-cap      \
                 mean-saved  saved-share"
            );
            for (slot, threshold) in thresholds.iter().enumerate() {
                let (mut crossed, mut agree, mut nat, mut nat_agree, mut cap, mut cap_agree) =
                    (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
                let (mut saved, mut total) = (0u64, 0u64);
                for (_, (winner, victory, end_turn, crossings)) in &finished {
                    total += u64::from(*end_turn);
                    let Some((pid, turn)) = crossings[slot] else {
                        continue;
                    };
                    crossed += 1;
                    saved += u64::from(end_turn.saturating_sub(turn));
                    let hit = *winner == Some(pid);
                    agree += u32::from(hit);
                    if victory.as_deref() == Some("score") {
                        cap += 1;
                        cap_agree += u32::from(hit);
                    } else {
                        nat += 1;
                        nat_agree += u32::from(hit);
                    }
                }
                let pct = |num: u32, den: u32| {
                    if den == 0 {
                        "     -".to_string()
                    } else {
                        format!("{:5.1}%", 100.0 * f64::from(num) / f64::from(den))
                    }
                };
                println!(
                    "{threshold:<9}  {crossed:3}/{:<3}  {}({agree:3}/{crossed:<3})  \
                     {}({nat_agree:2}/{nat:<2})  {}({cap_agree:2}/{cap:<2})  {:8.1}    {:5.1}%",
                    finished.len(),
                    pct(agree, crossed),
                    pct(nat_agree, nat),
                    pct(cap_agree, cap),
                    if crossed == 0 {
                        0.0
                    } else {
                        saved as f64 / f64::from(crossed)
                    },
                    if total == 0 {
                        0.0
                    } else {
                        100.0 * saved as f64 / total as f64
                    },
                );
            }
            if crashes > 0 {
                println!("\n{crashes} of {games} games crashed");
                std::process::exit(1);
            }
        }
        "benchmark" => {
            let games = arg(&args, "--games", 50);
            let turns = arg(&args, "--turns", 100) as u32;
            let jobs = jobs_arg(&args);
            let t0 = Instant::now();
            let played = civvis::parallel::map(games as usize, jobs, |seed| {
                let mut g = Game::new(2, 20, 14, seed as u64, turns, 0);
                let mut ais = AdvancedAi::fleet(&g);
                run_game(&mut g, &mut ais);
                g.turn as u64
            });
            let total_turns: u64 = played.iter().sum();
            let dt = t0.elapsed().as_secs_f64();
            println!(
                "{} games, {} turns in {:.2}s = {:.0} turns/sec \
                 (2 players, 20x14, {jobs} at a time)",
                games,
                total_turns,
                dt,
                total_turns as f64 / dt
            );
        }
        // What an agent that searches actually does: take a position and roll
        // it forward, over and over. Cloning a position dominates that, and
        // nothing else here measured it.
        "rollouts" => {
            let players = arg(&args, "--players", 6);
            let warmup = arg(&args, "--turns", 150) as u32;
            let samples = arg(&args, "--samples", 5000) as usize;
            let mut g = Game::new_with(game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
                setup::TurnStructure::Sequential,
            ));
            let mut ais = AdvancedAi::fleet(&g);
            // Play in to the requested turn first: an empty map clones far
            // faster than a settled one, and a settled one is what an agent
            // searches from.
            while g.turn < warmup && g.winner.is_none() {
                let pid = g.current;
                ais[pid].take_turn(&mut g, pid);
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &civvis::game::Action::EndTurn);
                }
            }
            let clone_start = Instant::now();
            let mut sink = 0usize;
            for _ in 0..samples {
                sink += g.clone().units.len();
            }
            let clone_us = clone_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            let speculative_start = Instant::now();
            for _ in 0..samples {
                sink += g.speculative_clone().units.len();
            }
            let speculative_us =
                speculative_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            // A searching agent mostly applies ordinary moves and only
            // occasionally ends a turn, and the two cost wildly different
            // amounts, so both are reported.
            let seat = g.current;
            let mut mover = None;
            for action in g.legal_actions(seat) {
                if let civvis::game::Action::Move { .. } = action {
                    mover = Some(action);
                    break;
                }
            }
            let move_us = mover.as_ref().map(|action| {
                let start = Instant::now();
                for _ in 0..samples {
                    let mut branch = g.clone();
                    let _ = branch.apply(seat, action);
                    sink += branch.units.len();
                }
                start.elapsed().as_secs_f64() / samples as f64 * 1e6
            });
            let fast = g.speculative_clone();
            let end_start = Instant::now();
            for _ in 0..samples {
                let mut branch = g.clone();
                let _ = branch.apply(seat, &civvis::game::Action::EndTurn);
                sink += branch.units.len();
            }
            let end_us = end_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            let fast_end_start = Instant::now();
            for _ in 0..samples {
                let mut branch = fast.speculative_clone();
                let _ = branch.apply(seat, &civvis::game::Action::EndTurn);
                sink += branch.units.len();
            }
            let fast_end_us = fast_end_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            // The same move on a position that is not maintaining fogged
            // memory — what a search that never observes mid-rollout pays.
            let fast_us = mover.as_ref().map(|action| {
                let start = Instant::now();
                for _ in 0..samples {
                    let mut branch = fast.speculative_clone();
                    let _ = branch.apply(seat, action);
                    sink += branch.units.len();
                }
                start.elapsed().as_secs_f64() / samples as f64 * 1e6
            });
            println!(
                "turn {} · {} seats · {} cities · {} units",
                g.turn,
                g.players.len(),
                g.cities.len(),
                g.units.len(),
            );
            println!("clone            {clone_us:8.1} us  = {:.0}/sec", 1e6 / clone_us);
            println!(
                "speculative clone {speculative_us:7.1} us  = {:.0}/sec",
                1e6 / speculative_us
            );
            match move_us {
                Some(us) => println!("clone + move     {us:8.1} us  = {:.0} rollouts/sec", 1e6 / us),
                None => println!("clone + move          n/a  (no legal move for this seat)"),
            }
            println!("clone + end turn {end_us:8.1} us  = {:.0}/sec", 1e6 / end_us);
            if let Some(us) = fast_us {
                println!(
                    "clone + move (no fog){us:6.1} us  = {:.0} rollouts/sec",
                    1e6 / us
                );
            }
            println!("clone + end (no fog) {fast_end_us:6.1} us  = {:.0}/sec", 1e6 / fast_end_us);
            let _ = sink;
        }
        "tournament" => {
            // Each mode keeps its own ladder: a Tactics rating is earned
            // against Tactics opponents on an arena and says nothing about
            // the grand strategy game, so `--map battlefield` writes to the
            // Tactics ledger unless `--ratings` names another. Offered to the
            // Civ ledger it would be refused anyway — the profile records the
            // map script — so this names the right file rather than making
            // the operator discover the mismatch.
            let ratings_path = arg_text(
                &args,
                "--ratings",
                civvis::elo::ratings_path_for(setup::GameMode::for_script(map_script(&args))),
            );
            if args.iter().any(|arg| arg == "--standings") {
                match civvis::elo::EloPool::load(&ratings_path) {
                    Ok(pool) => print!("{}", civvis::elo::leaderboard(&pool)),
                    Err(error) => {
                        eprintln!("cannot load Elo ledger {ratings_path}: {error}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            let entrant_spec = args
                .iter()
                .position(|a| a == "--ais")
                .and_then(|i| args.get(i + 1))
                .map(String::as_str)
                .unwrap_or(DEFAULT_TOURNAMENT_ENTRANTS);
            let entrants = parse_tournament_entrants(entrant_spec).unwrap_or_else(|error| {
                eprintln!("{error}");
                std::process::exit(2);
            });
            for entrant in &entrants {
                if !civvis::elo::BUILTIN_AIS.contains(&entrant.controller.as_str()) {
                    eprintln!(
                        "unknown AI controller {:?}; builtin: {:?} (custom bots: \
                         use civvis::elo::run_tournament from Rust)",
                        entrant.controller,
                        civvis::elo::BUILTIN_AIS
                    );
                    std::process::exit(1);
                }
            }
            let mut effective = BTreeMap::<&'static str, String>::new();
            for entrant in &entrants {
                let provenance = civvis::elo::builtin_provenance(
                    &entrant.controller,
                    civvis::elo::ARTIFACT_DIR,
                );
                if provenance.degraded() {
                    eprintln!(
                        "cannot rate identity {:?}: {}",
                        entrant.identity,
                        provenance.line()
                    );
                    std::process::exit(2);
                }
                if let Some(other) = effective.insert(provenance.effective, entrant.identity.clone()) {
                    eprintln!(
                        "rating identities {:?} and {:?} both play as {:?}; cloned controllers cannot be rated as separate players",
                        other,
                        entrant.identity,
                        provenance.effective,
                    );
                    std::process::exit(2);
                }
                if entrant.identity != entrant.controller {
                    eprintln!(
                        "rating identity {:?} plays controller {:?}",
                        entrant.identity, entrant.controller
                    );
                }
                if provenance.untrained() {
                    eprintln!("warning: {}", provenance.line());
                }
            }
            let names: Vec<String> = entrants
                .iter()
                .map(|entrant| entrant.identity.clone())
                .collect();
            let controller_roster: Vec<String> = entrants
                .iter()
                .map(|entrant| entrant.controller.clone())
                .collect();
            let controllers: BTreeMap<String, String> = entrants
                .into_iter()
                .map(|entrant| (entrant.identity, entrant.controller))
                .collect();
            let rating_anchor = match args.iter().position(|arg| arg == "--anchor") {
                Some(index) => {
                    let value = args.get(index + 1).unwrap_or_else(|| {
                        eprintln!("--anchor needs an entrant identity or 'none'");
                        std::process::exit(2);
                    });
                    if value.starts_with("--") || value.trim().is_empty() {
                        eprintln!("--anchor needs an entrant identity or 'none'");
                        std::process::exit(2);
                    }
                    (value != "none").then(|| value.clone())
                }
                None => names
                    .iter()
                    .any(|name| name == "advanced_v1")
                    .then(|| "advanced_v1".to_string()),
            };
            let strict = |result: Result<i64, String>| {
                result.unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(2);
                })
            };
            let players = strict(strict_i64_arg(
                &args,
                "--players",
                names.len().max(2) as i64,
            ));
            if !(2..=100).contains(&players) {
                eprintln!("--players must be between 2 and 100");
                std::process::exit(2);
            }
            let rules = Rules::embedded();
            let speed = arg_text(&args, "--speed", &default_speed());
            if !rules.speeds.contains_key(&speed) {
                eprintln!("unknown game speed {speed:?}; choose one of {:?}", speeds(&rules));
                std::process::exit(2);
            }
            let map_id = arg_text(&args, "--map", "pangaea");
            let map_script = MapScript::from_id(&map_id).unwrap_or_else(|| {
                eprintln!("unknown map script {map_id:?}; choose pangaea, continents, or archipelago");
                std::process::exit(2);
            });
            // Ratings are a persistent experiment. Keep its historical flat
            // default unless the operator explicitly selects a globe.
            let topology_default = if map_id == "planet" { "planet" } else { "flat" };
            let topology_id = arg_text(&args, "--shape", topology_default);
            let tournament_topology = MapTopology::from_id(&topology_id).unwrap_or_else(|| {
                eprintln!("unknown map shape {topology_id:?}; choose flat or planet");
                std::process::exit(2);
            });
            let poles_id = arg_text(&args, "--poles", "poles");
            let tournament_poles = MapPoles::from_id(&poles_id).unwrap_or_else(|| {
                eprintln!("unknown pole setting {poles_id:?}; choose poles or randomized");
                std::process::exit(2);
            });
            let size = MapSize::for_players(players as usize);
            let (default_width, default_height) = size.dimensions(tournament_topology);
            let width = strict(strict_i64_arg(&args, "--width", i64::from(default_width)));
            let height = strict(strict_i64_arg(&args, "--height", i64::from(default_height)));
            if width < 8 || height < 8 || width > i64::from(i32::MAX) || height > i64::from(i32::MAX)
            {
                eprintln!("tournament dimensions must each be between 8 and {}", i32::MAX);
                std::process::exit(2);
            }
            let games = strict(strict_i64_arg(&args, "--games", 20));
            let turns = strict(strict_i64_arg(&args, "--turns", stock_turns(&args)));
            let seed = strict(strict_i64_arg(&args, "--seed", 0));
            let city_states = strict(strict_i64_arg(
                &args,
                "--city-states",
                size.default_city_states
                    .min(civvis::game::CITY_STATE_NAMES.len()) as i64,
            ));
            if games <= 0 || games > i64::from(u32::MAX) {
                eprintln!("--games must be between 1 and {}", u32::MAX);
                std::process::exit(2);
            }
            if turns <= 0 || turns > i64::from(u32::MAX) {
                eprintln!("--turns must be between 1 and {}", u32::MAX);
                std::process::exit(2);
            }
            if seed < 0 {
                eprintln!("--seed must be non-negative");
                std::process::exit(2);
            }
            if city_states < 0 || city_states as usize > civvis::game::CITY_STATE_NAMES.len() {
                eprintln!(
                    "--city-states must be between 0 and {}",
                    civvis::game::CITY_STATE_NAMES.len()
                );
                std::process::exit(2);
            }
            let k = strict_f64_arg(&args, "--k", 24.0).unwrap_or_else(|error| {
                eprintln!("{error}");
                std::process::exit(2);
            });
            if !k.is_finite() || k <= 0.0 {
                eprintln!("--k must be finite and greater than zero");
                std::process::exit(2);
            }
            let jobs = strict(strict_i64_arg(&args, "--jobs", 0));
            if jobs < 0 {
                eprintln!("--jobs must be non-negative (zero means one per core)");
                std::process::exit(2);
            }
            let cfg = civvis::elo::TourneyCfg {
                games: games as u32,
                players_per_game: players as usize,
                width: width as i32,
                height: height as i32,
                speed,
                map_script,
                map_topology: tournament_topology,
                map_poles: tournament_poles,
                // A tournament writes the project's persistent Elo, so it has
                // to rank on whole games; see `stock_turns`.
                max_turns: turns as u32,
                num_city_states: city_states as usize,
                // A tournament rolls its own per-game seeds, so the era
                // choice travels rather than one era resolved here.
                start_era: if arg_text(&args, "--start-era", setup::stock_start_era_id())
                    == "random"
                {
                    setup::StartEraChoice::RandomPerGame
                } else {
                    setup::StartEraChoice::Fixed(start_era(&args, seed as u64))
                },
                tactics: tactics_rules(&args),
                seed: seed as u64,
                k,
                rating_anchor,
                controller_roster,
                verbose: !args.iter().any(|a| a == "--quiet"),
                jobs: if jobs == 0 {
                    civvis::parallel::default_jobs()
                } else {
                    jobs as usize
                },
            };
            match civvis::elo::run_persistent_tournament(
                &names,
                |identity, seed| {
                    let controller = controllers
                        .get(identity)
                        .expect("every scheduled identity came from --ais");
                    civvis::elo::builtin_ai(controller, seed)
                },
                &cfg,
                &ratings_path,
            ) {
                Ok(pool) => {
                    println!();
                    print!("{}", civvis::elo::leaderboard(&pool));
                    println!("ratings checkpointed to {ratings_path}");
                }
                Err(error) => {
                    eprintln!("Elo tournament failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(not(feature = "closed-experiments"))]
        "selfplay" => {
            eprintln!(
                "selfplay is part of the closed training-data lane; rebuild with \
                 --features closed-experiments to run the exporter"
            );
            std::process::exit(1);
        }
        #[cfg(feature = "closed-experiments")]
        "selfplay" => {
            let players = arg(&args, "--players", 4).max(2);
            let options = game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
                setup::TurnStructure::Sequential,
            );
            let counterfactual = args.iter().any(|arg| arg == "--counterfactual");
            let cfg = civvis::selfplay::SelfPlayCfg {
                games: arg(&args, "--games", 20) as usize,
                players: players as usize,
                width: options.width,
                height: options.height,
                city_states: options.city_states,
                max_turns: options.max_turns,
                seed: arg(&args, "--seed", 0) as u64,
                every: arg(&args, "--every", if counterfactual { 40 } else { 10 }).max(1) as u32,
                ai: arg_text(
                    &args,
                    "--ai",
                    if counterfactual {
                        "strategic_score"
                    } else {
                        "advanced"
                    },
                ),
                out: arg_text(&args, "--out", "selfplay"),
                scalar_only: args.iter().any(|arg| arg == "--scalar-only"),
                counterfactual,
                counterfactual_roots: arg(&args, "--counterfactual-roots", 0).max(0) as usize,
                decision_features: args.iter().any(|arg| arg == "--decision-features"),
                jobs: jobs_arg(&args),
            };
            match civvis::selfplay::export(&cfg) {
                Ok(stats) => println!(
                    "
{} samples from {} games ({} decisive) -> {}",
                    stats.samples, stats.games, stats.decisive, cfg.out
                ),
                Err(error) => {
                    eprintln!("selfplay export failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        "arena" => {
            // A batch rating event: refit the corrected contextual model over
            // the league's standardized games and publish an anchored table
            // that moves only when an arena runs. `src/arena.rs` says why.
            let dir = arg_text(
                &args,
                "--dir",
                &std::env::var("CIVVIS_LEAGUE_DIR").unwrap_or_else(|_| "league".into()),
            );
            // 0 = the history's modal table size, printed in the report.
            let seats = arg(&args, "--seats", 0).max(0) as usize;
            let anchors: Vec<String> = arg_text(&args, "--anchors", "advanced,basic")
                .split(',')
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect();
            let anchor_elo = arg_f64(&args, "--anchor-elo", 1500.0);
            match civvis::arena::run_dir(
                &dir,
                seats,
                &anchors,
                anchor_elo,
                std::time::SystemTime::now(),
            ) {
                Ok(report) => print!("{report}"),
                Err(error) => {
                    eprintln!("arena failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        "league" => {
            let players = arg(&args, "--players", 4).max(2);
            let defaults = civvis::league::LeagueCfg::default();
            let rules = Rules::embedded();
            let speed = arg_text(&args, "--speed", &defaults.speed);
            let Some(speed_spec) = rules.speeds.get(&speed) else {
                eprintln!(
                    "unknown game speed {speed:?}; choose one of {:?}",
                    speeds(&rules)
                );
                std::process::exit(2);
            };
            let shared_dir =
                std::env::var("CIVVIS_LEAGUE_DIR").unwrap_or_else(|_| defaults.dir.clone());
            let cfg = civvis::league::LeagueCfg {
                rounds: arg(&args, "--rounds", 10).max(0) as u32,
                games_per_round: arg(&args, "--games", 16).max(1) as u32,
                players_per_game: players as usize,
                width: auto_dimension(&args, "--width", players, true),
                height: auto_dimension(&args, "--height", players, false),
                speed,
                max_turns: arg(&args, "--turns", i64::from(speed_spec.turns)).max(1) as u32,
                num_city_states: auto_cs(&args, players),
                seed: arg(&args, "--seed", 1) as u64,
                jobs: jobs_arg(&args),
                dir: arg_text(&args, "--dir", &shared_dir),
                evolve_every: arg(&args, "--evolve-every", 4).max(0) as u32,
                max_pop: arg(&args, "--pop", 12).max(1) as usize,
                verbose: !args.iter().any(|a| a == "--quiet"),
                worker_id: arg_text(&args, "--worker", &defaults.worker_id),
                lease_seconds: arg(&args, "--lease-seconds", defaults.lease_seconds as i64).max(1)
                    as u64,
            };
            let civ = arg_text(&args, "--civ", "");
            if args.iter().any(|a| a == "--standings") || !civ.is_empty() {
                match civvis::league::load_league(&cfg.dir) {
                    Some(league) => {
                        if !civ.is_empty() {
                            print!("{}", civvis::league::civ_standings(&league, &civ));
                        } else if args.iter().any(|a| a == "--civs") {
                            print!("{}", civvis::league::civ_summary(&league));
                        } else {
                            print!("{}", civvis::league::standings(&league));
                        }
                    }
                    None => {
                        eprintln!("no league at {}/league.json", cfg.dir);
                        std::process::exit(1);
                    }
                }
            } else {
                if let Err(error) = civvis::league::try_run_league(&cfg) {
                    eprintln!("league failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        "league-init" => {
            let dir = arg_text(&args, "--league", "");
            let Some(league) = (!dir.is_empty())
                .then(|| civvis::league::initialize_shipped_league(&dir))
                .flatten()
            else {
                eprintln!("league-init needs a writable --league directory");
                std::process::exit(2);
            };
            println!("{}", serde_json::json!({
                "status": "ready",
                "round": league.round,
                "strategies": league.strategies.len(),
            }));
        }
        "rate-game" => {
            let dir = arg_text(&args, "--league", "");
            if dir.is_empty() {
                eprintln!("rate-game needs a writable --league directory");
                std::process::exit(2);
            }
            let report: civvis::league::LiveGameReport =
                match serde_json::from_reader(std::io::stdin().lock()) {
                    Ok(report) => report,
                    Err(error) => {
                        eprintln!("invalid live-game report: {error}");
                        std::process::exit(2);
                    }
                };
            if civvis::league::initialize_shipped_league(&dir).is_none() {
                eprintln!("could not initialize the live league at {dir}");
                std::process::exit(1);
            }
            let Some(record) = civvis::league::record_ranked_game_once(
                &dir,
                &report.result_id,
                &report.seats,
                report.seed,
                report.turn,
                &report.victory,
            ) else {
                eprintln!("the live-game report is invalid or names an unknown strategy");
                std::process::exit(2);
            };
            let league = record.league();
            println!("{}", serde_json::json!({
                "status": record.status(),
                "round": league.round,
                "strategies": report.seats.iter().filter_map(|seat| {
                    league.strategies.iter().find(|strategy| strategy.name == seat.strategy)
                }).map(|strategy| serde_json::json!({
                    "name": strategy.name,
                    "rating": strategy.rating,
                    "rd": strategy.rd,
                    "games": strategy.games,
                    "wins": strategy.wins,
                })).collect::<Vec<_>>(),
            }));
        }
        "evolve" => {
            let players = arg(&args, "--players", 4);
            civvis::evolve::evolve(&civvis::evolve::EvoCfg {
                generations: arg(&args, "--generations", 1_000_000) as u32,
                pop: arg(&args, "--pop", 16) as usize,
                games: arg(&args, "--games", 8) as usize,
                players: players as usize,
                width: auto_dimension(&args, "--width", players, true),
                height: auto_dimension(&args, "--height", players, false),
                // Selection reads continuous score and combat shares, but the
                // separate promotion SPRT still decides on outright wins. At
                // 160 turns almost nothing reaches a victory, so confirmation
                // would judge arbitrary cutoffs rather than completed games.
                // See `stock_turns`.
                max_turns: arg(&args, "--turns", stock_turns(&args)) as u32,
                seed: arg(&args, "--seed", 1) as u64,
                threads: arg(&args, "--threads", 8) as usize,
                dir: arg_text(&args, "--dir", "evolved"),
                // `--turns` already resolves through `stock_turns(&args)`,
                // which reads this flag; until now the game itself did not,
                // so `--speed online` bred truncated Standard games.
                speed: arg_text(&args, "--speed", &default_speed()),
            });
        }
        "play" => {
            // The stock game: four civilizations on a Tiny world, which is
            // `MapSize::for_players(4)`. The map script default lives in
            // `game_options` so the headless arms open the same world.
            let players = arg(&args, "--players", 4);
            // `--mirror <run-dir>`: show the board a Civilization VI seat can
            // actually see, rebuilt as a CIVVIS game, instead of generating one.
            //
            // This is what makes the two windows one game rather than two. The
            // control mod exports only revealed plots, so what appears here is
            // what the seat has earned and nothing more.
            //
            // Unrevealed ground remains explicit `unknown` terrain underneath
            // the fog; see `mirror::rebuild_game` for the separate traversable
            // frontier prior used by the live decider.
            let mirrored: Option<Game> = args
                .iter()
                .position(|value| value == "--mirror")
                .and_then(|index| args.get(index + 1))
                .map(|dir| {
                    let events = std::path::Path::new(dir).join("events.jsonl");
                    let snapshot = civvis::mirror::snapshot_from_events(&events)
                        .unwrap_or_else(|error| {
                            eprintln!("cannot read {}: {error}", events.display());
                            std::process::exit(2);
                        });
                    if snapshot.revealed_count() == 0 {
                        eprintln!(
                            "{} has no tiles to mirror — the run needs --export-state, \
                             and before the PlayersVisibility fix the export emitted nothing",
                            events.display()
                        );
                        std::process::exit(2);
                    }
                    println!(
                        "mirroring {} revealed plots of a {}x{} world at turn {}",
                        snapshot.revealed_count(),
                        snapshot.width,
                        snapshot.height,
                        snapshot.turn
                    );
                    // ★★★★ MIRROR THE EMPIRE, NOT JUST THE GROUND. `rebuild_game`
                    // returns terrain only, so this window read "Ancient Age TURN 1"
                    // with an empty world while Civilization VI sat at turn 7 with a
                    // revealed continent, two cities and an army. Side by side that is
                    // worse than no mirror: the operator is asked to verify the two
                    // match and shown a board that cannot match by construction.
                    //
                    // `rebuild_from_state` places both empires' cities, our units and
                    // every visible rival unit, and sets the turn — the same
                    // reconstruction `civvis-orders` decides from, so what is on screen
                    // is what CIVVIS is actually reasoning about.
                    // ★ MOCK MODE. `--dump-state <file>` writes the observed board out
                    // as JSON; `--state <file>` merges a file back over it before the
                    // reconstruction runs. Together they give a round trip — capture the
                    // real Civilization VI position, edit anything, replay it — which is
                    // what makes a disagreement between the two screens reproducible
                    // instead of a thing you have to catch live.
                    let flag = |name: &str| {
                        args.iter()
                            .position(|value| value == name)
                            .and_then(|at| args.get(at + 1))
                            .cloned()
                    };
                    let mut observed = civvis::mirror::state_value_from_events(&events, None);
                    // ⚠ DUMP BEFORE MERGE. Writing the file after the override records
                    // the mock, not the observation, so using both flags at once would
                    // overwrite the very board you were trying to capture — and the
                    // second run would silently start from the edit.
                    if let (Some(state), Some(path)) = (observed.as_ref(), flag("--dump-state")) {
                        match serde_json::to_string_pretty(state)
                            .map_err(|e| e.to_string())
                            .and_then(|text| std::fs::write(&path, text).map_err(|e| e.to_string()))
                        {
                            Ok(()) => println!("  observed state written to {path}"),
                            Err(why) => println!("  ⚠ could not write --dump-state {path}: {why}"),
                        }
                    }
                    if let (Some(state), Some(path)) = (observed.as_mut(), flag("--state")) {
                        match std::fs::read_to_string(&path)
                            .ok()
                            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                        {
                            Some(patch) => {
                                civvis::mirror::merge_state(state, &patch);
                                println!("  state overridden from {path}");
                            }
                            // Loud, because silently mirroring the real board when the
                            // operator asked for a mocked one is a wrong answer that
                            // looks exactly like a right one.
                            None => println!("  ⚠ could not read --state {path}: using observed board"),
                        }
                    }
                    let from_value = observed
                        .as_ref()
                        .and_then(|v| serde_json::from_value::<civvis::mirror::StateSnapshot>(v.clone()).ok());
                    match from_value.or_else(|| civvis::mirror::state_from_events(&events, None)) {
                        Some(state) => {
                            let rebuilt = civvis::mirror::rebuild_from_state(
                                &snapshot, &state, players as usize, 1, 250, 6,
                            );
                            println!(
                                "  empire: {} cities, {} units, {} rival cities, \
                                 {} rival units at turn {}",
                                rebuilt.placed_cities,
                                rebuilt.placed_units,
                                rebuilt.placed_rival_cities,
                                rebuilt.placed_rival_units,
                                state.turn
                            );
                            if !rebuilt.unmapped.is_empty() {
                                println!("  untranslatable: {}", rebuilt.unmapped.join(","));
                            }
                            rebuilt.game
                        }
                        // No `state` event means the run is not exporting one; terrain
                        // alone is still worth showing, and saying so beats implying
                        // the empty empire is real.
                        None => {
                            println!("  no `state` event: terrain only, no cities or units");
                            civvis::mirror::rebuild_game(&snapshot, players as usize, 1)
                        }
                    }
                });
            let resumed: Option<Game> = args
                .iter()
                .position(|value| value == "--resume")
                .and_then(|index| args.get(index + 1))
                .map(|path| {
                    let raw = std::fs::read_to_string(path).unwrap_or_else(|error| {
                        eprintln!("cannot read checkpoint {path}: {error}");
                        std::process::exit(2);
                    });
                    let game: Game = serde_json::from_str(&raw).unwrap_or_else(|error| {
                        eprintln!("cannot load checkpoint {path}: {error}");
                        std::process::exit(2);
                    });
                    // A save records the mods it was played under. Resuming
                    // under a different set silently changes the rules
                    // mid-game, so say so rather than pretend otherwise.
                    let active = civvis::mods::active_names();
                    if game.mods != active {
                        eprintln!(
                            "warning: {path} was played with mods {:?} but this process has {:?}",
                            game.mods, active
                        );
                    }
                    // Stepping a simultaneous save one seat at a time would
                    // silently change its regime mid-game; say so instead. A
                    // spectated table plays whole planned turns, so only a
                    // resume that seats a human refuses it.
                    if game.turn_structure == setup::TurnStructure::Simultaneous
                        && !args.iter().any(|a| a == "--spectate" || a == "--watch")
                    {
                        eprintln!(
                            "{path} is a simultaneous-turns game; a played game is \
                             sequential by construction — resume it with --spectate"
                        );
                        std::process::exit(2);
                    }
                    game
                });
            let seed = {
                let s = arg(&args, "--seed", -1);
                if s >= 0 {
                    s as u64
                } else {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .subsec_nanos() as u64
                }
            };
            // A played game consults the human seat live, one seat at a
            // time, which is the sequential regime by construction. A
            // spectated table has nobody at the keyboard, so it plays the
            // simultaneous regime as one whole planned turn per pace tick —
            // and defaults to it, like the rest of the automated surfaces.
            // Refuse the combination that cannot be honoured rather than
            // quietly playing a different game than the flag asked for;
            // `simulate` and `soak` play it headless either way.
            let spectate = args.iter().any(|a| a == "--spectate" || a == "--watch");
            let play_options = game_options(
                &args,
                players,
                seed,
                setup::TurnStructure::Sequential,
            );
            if !spectate && play_options.turn_structure == setup::TurnStructure::Simultaneous {
                eprintln!(
                    "a played game is sequential by construction; simultaneous \
                     turns need --spectate, or `civvis simulate --turn-structure \
                     simultaneous`"
                );
                std::process::exit(2);
            }
            let map_script = play_options.map_script;
            let map_topology = play_options.map_topology;
            let map_poles = play_options.map_poles;
            let game_speed = GameSpeed::from_id(&play_options.speed).unwrap_or(GameSpeed::Standard);
            civvis::server::serve_with_game(
                arg(&args, "--port", 8765) as u16,
                !args.iter().any(|a| a == "--no-open"),
                civvis::server::Params {
                    num_players: players as usize,
                    width: auto_dimension(&args, "--width", players, true),
                    height: auto_dimension(&args, "--height", players, false),
                    seed,
                    base_ruleset: play_options.base_ruleset,
                    start_era: play_options.start_era,
                    future_era: play_options.future_era,
                    turn_structure: play_options.turn_structure,
                    map_script,
                    map_topology,
                    map_poles,
                    game_speed,
                    max_turns: play_options.max_turns,
                    victory_conditions: victory_conditions(&args),
                    // The engine's stock setup leaves mercy off. The lobby
                    // can still opt into any listed threshold after launch.
                    mercy_rule: play_options.mercy_rule,
                    required_victory_types: 1,
                    // The lobby can still change these mid-session; this is
                    // what the launch itself asked for.
                    tactics: tactics_rules(&args),
                    num_city_states: auto_cs(&args, players),
                    spectate,
                    difficulty: play_options.difficulty,
                    speed: play_options.speed,
                    teams: play_options.teams,
                    leader_pool: play_options.leader_pool,
                    civs: play_options.civs,
                    supervised: args.iter().any(|a| a == "--supervised"),
                    league_dir: {
                        let dir = arg_text(&args, "--league", "");
                        (!dir.is_empty()).then_some(dir)
                    },
                    league_record: args.iter().any(|a| a == "--league-record"),
                    ai_pool: ai_player_pool(&args),
                    force_strategy: {
                        let name = arg_text(&args, "--force-strategy", "");
                        (!name.is_empty()).then_some(name)
                    },
                },
                mirrored.or(resumed),
                args.iter().any(|a| a == "--paused"),
            );
        }
        "pedia" => {
            // Everything after the command that is not a flag is the query.
            let query = args
                .iter()
                .skip(1)
                .take_while(|arg| !arg.starts_with("--"))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let rules = Rules::embedded();
            let found = civvis::pedia::search(&rules, &query);
            if found.is_empty() {
                println!("nothing in the ruleset matches {query:?}");
                std::process::exit(1);
            }
            print!("{}", civvis::pedia::render(&found));
            println!("
{} entries", found.len());
        }
        "validate" => {
            let findings = civvis::validate::validate(&Rules::embedded());
            let (text, clean) = civvis::validate::report(&findings);
            print!("{text}");
            let strict = args.iter().any(|a| a == "--strict");
            if !clean || (strict && !findings.is_empty()) {
                std::process::exit(1);
            }
        }
        "rating" => {
            let dir = arg_text(
                &args,
                "--dir",
                &std::env::var("CIVVIS_LEAGUE_DIR").unwrap_or_else(|_| "league".into()),
            );
            let mut history = match civvis::rating::load_history(&dir) {
                Ok(history) if history.len() >= 2 => history,
                Ok(_) => {
                    eprintln!("{dir}/matches.csv has no finished games to rate");
                    std::process::exit(1);
                }
                Err(error) => {
                    eprintln!("cannot read {dir}/matches.csv: {error}");
                    std::process::exit(1);
                }
            };
            // A league directory can hold games of several table sizes; a
            // single size is the cleaner slice to reason about.
            let want_seats = arg(&args, "--seats", 0).max(0) as usize;
            if want_seats > 0 {
                history.retain(|m| m.seats.len() == want_seats);
                if history.len() < 2 {
                    eprintln!("{dir}/matches.csv has fewer than 2 games with {want_seats} seats");
                    std::process::exit(1);
                }
            }
            let seats = history.iter().map(|m| m.seats.len()).sum::<usize>() as f64
                / history.len() as f64;
            let burn_in = arg_f64(&args, "--burn-in", 0.3).clamp(0.0, 0.95);
            let mut cfg = civvis::rating::RatingCfg {
                stage_decay: arg_f64(&args, "--stage-decay", 0.5).clamp(0.0, 1.0),
                beta: arg_f64(&args, "--beta", 0.9).max(1e-3),
                ..civvis::rating::RatingCfg::default()
            };
            for anchor in arg_text(&args, "--anchors", "advanced,basic").split(',') {
                let anchor = anchor.trim();
                if !anchor.is_empty() {
                    cfg.anchors.insert(anchor.to_string());
                }
            }
            // Explicit per-stage credit, e.g. `--stage-credit 1,0.5,0.25,0`
            // to keep the geometric shape but silence an anti-informative
            // last stage. Overrides --stage-decay.
            let credit = arg_text(&args, "--stage-credit", "");
            if !credit.is_empty() {
                let parsed: Vec<f64> = credit
                    .split(',')
                    .filter_map(|x| x.trim().parse::<f64>().ok())
                    .collect();
                if parsed.is_empty() {
                    eprintln!("--stage-credit needs comma-separated numbers");
                    std::process::exit(1);
                }
                cfg.stage_credit = Some(parsed);
            }
            println!("{} games from {dir}/matches.csv\n", history.len());
            if args.iter().any(|a| a == "--stages") {
                let info = civvis::rating::fit_stage_weights(&history, burn_in);
                println!("information carried by each placement stage (nats, measured)");
                println!("  a stage at or below zero is noise and should not move a rating\n");
                for (k, nats) in info.iter().enumerate() {
                    let bar = "#".repeat(((nats.max(0.0)) * 60.0) as usize);
                    println!("  stage {:<3} {:+8.4}  {bar}", k + 1, nats);
                }
            } else if args.iter().any(|a| a == "--sweep") {
                println!(
                    "{:<14}{:>12}{:>10}{:>12}",
                    "stage decay", "winner LL", "accuracy", "info/game"
                );
                for step in 0..=10 {
                    let decay = step as f64 / 10.0;
                    let mut model = civvis::rating::ContextualRating::new(
                        civvis::rating::RatingCfg {
                            stage_decay: decay,
                            ..cfg.clone()
                        },
                    );
                    let m = civvis::rating::evaluate(&mut model, &history, burn_in);
                    println!(
                        "{decay:<14.1}{:>12.4}{:>9.1}%{:>12.4}",
                        m.win_log_loss,
                        100.0 * m.win_accuracy,
                        m.information
                    );
                }
            } else if args.iter().any(|a| a == "--backtest") {
                let rows = civvis::rating::backtest(&history, burn_in, &cfg);
                print!("{}", civvis::rating::backtest_report(&rows, seats));
            } else {
                let rating = civvis::rating::rate_history(&history, &cfg);
                print!("{}", rating.standings());
            }
        }
        _ => {
            println!(
                "usage: civvis <simulate|soak|odds-audit|benchmark|tournament|league|league-init|arena|rate-game|rating|play|evolve|validate|pedia> \
                      [--players N] [--seed N] [--turns N] [--width N] [--height N] \
                      [--city-states N] [--games N] [--ais [identity=]controller,...] [--anchor identity|none] [--ratings path] [--standings] [--port N] [--no-open] \
                      [--map land_only|lakes|inland_sea|tenins_ball|grand_canals|grand_canals_2|pangaea|earth|true_start_earth|continents|small_continents|fjords|islands|water_world|battlefield|tactics_planet|tactics_ocean|trafalgar] \
                      [--shape flat|planet] [--poles poles|randomized] \
                      [--difficulty settler|chieftain|warlord|prince|king|emperor|immortal|deity] \
                      [--speed online|quick|standard|epic|marathon] \
                      [--disasters 0|1|2|3|4] [--barbarians on|off] \
                      [--turn-structure sequential|simultaneous (everything defaults to \
                       sequential; simultaneous is a research regime)] \
                      [--game-modes apocalypse,secret_societies] \
                      [--leader-pool civ6|historical|today] \
                      [--human-seats 0,1] [--teams 0,0,1,1] [--mods path/to/mod,path/to/other] \
                      [--victories science,culture,religious,diplomatic,domination,score] \
                      [--spectate] [--supervised] [--force-strategy NAME] [--ai-pool best1|best2|best3|best5|all] [--resume checkpoint.json] [--strict] \
                      [--league dir] [--league-record] [--standings [--civ Rome | --civs]] [--rounds N] \
                      [--evolve-every N] [--pop N] [--worker ID] [--lease-seconds N] \
                      [rating: --dir league/ --backtest|--sweep|--stages --burn-in F --stage-decay F --anchors a,b]"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        game_options, jobs_arg, map_topology, parse_tournament_entrants, start_era, tactics_rules,
        simultaneous_soak_job_split, single_simulation_jobs_arg, strict_f64_arg, strict_i64_arg,
        turn_structure, ADVANCED_V1_SOURCE_CONTRACT_FNV,
        DEFAULT_TOURNAMENT_ENTRANTS, SINGLE_SIMULATION_DEFAULT_MAX_JOBS,
    };
    use civvis::game::{Action, Game};
    use civvis::setup::{MapSize, MapTopology, TurnStructure};

    /// `--start-era random` spreads a sweep over the whole unit roster
    /// instead of teaching one era's matchups, and does it reproducibly: the
    /// roll comes from the game's own seed, so a soak replayed with the same
    /// `--start-seed` opens in the same eras. Consecutive seeds must not walk
    /// the ladder in lockstep, which is what the mix in `start_era` is for.
    /// Every launch path reads the arena flags through one function, because
    /// each path that grew its own copy accepted them and silently ignored
    /// them: `tournament` rated three "different" experiments identically,
    /// and `play` launched the stock arena however it was asked. A shared
    /// reader is the fix, so this pins that it reads what it is given and
    /// clamps what it cannot play.
    #[test]
    fn the_arena_flags_are_read_once_for_every_launch_path() {
        let stock = civvis::setup::TacticsRules::default();
        assert_eq!(tactics_rules(&[]), stock, "no flags is the stock arena");

        let asked = [
            "--map".to_string(), "battlefield".to_string(),
            "--tactics-cities".to_string(), "0".to_string(),
            "--tactics-production".to_string(), "120".to_string(),
            "--tactics-gold".to_string(), "0".to_string(),
            "--tactics-turns-per-tech".to_string(), "0".to_string(),
            "--tactics-turn-limit".to_string(), "150".to_string(),
        ];
        let rules = tactics_rules(&asked);
        assert_eq!(rules.cities, 0);
        assert_eq!(rules.production, 120);
        assert_eq!(rules.gold, 0);
        assert_eq!(rules.turns_per_tech, 0);
        assert_eq!(rules.turn_limit, 150);

        // The flag objective travels the same shared reader, and drags the
        // city out of the battle the way every other surface does.
        let flagged = [
            "--map".to_string(), "battlefield".to_string(),
            "--tactics-flag".to_string(),
            "--tactics-cities".to_string(), "1".to_string(),
        ];
        let rules = tactics_rules(&flagged);
        assert!(rules.flag);
        assert_eq!(rules.cities, 0, "the flag replaces the city objective");

        // Clamped, not trusted: these reach the same sanitiser the server uses.
        let silly = [
            "--tactics-cities".to_string(), "9".to_string(),
            "--tactics-production".to_string(), "100000".to_string(),
            "--tactics-turns-per-tech".to_string(), "100000".to_string(),
        ];
        let rules = tactics_rules(&silly);
        assert_eq!(rules.cities, 1, "an arena seats at most one city a side");
        assert_eq!(rules.production, civvis::setup::TacticsRules::MAX_YIELD);
        assert_eq!(rules.turns_per_tech, civvis::setup::TacticsRules::MAX_TURNS_PER_TECH);

        // And the world path carries the same answer, so a `play` launch and a
        // `soak` launch of the same flags are the same arena.
        let options = game_options(&asked, 2, 7, TurnStructure::Sequential);
        assert_eq!(options.tactics, tactics_rules(&asked));
        assert_eq!(options.max_turns, 150, "the arena uses its selected deadline");

        let explicit = [
            "--map".to_string(), "battlefield".to_string(),
            "--tactics-turn-limit".to_string(), "200".to_string(),
            "--turns".to_string(), "73".to_string(),
        ];
        assert_eq!(
            game_options(&explicit, 2, 8, TurnStructure::Sequential).max_turns,
            73,
            "the general explicit turn flag still overrides the Tactics menu"
        );
    }

    #[test]
    fn a_random_start_era_is_seeded_scattered_and_playable() {
        let args = ["--start-era".to_string(), "random".to_string()];
        let playable: Vec<usize> =
            civvis::setup::playable_start_eras().filter_map(|spec| spec.era).collect();
        let rolled: Vec<usize> = (0..64).map(|seed| start_era(&args, seed)).collect();

        for era in &rolled {
            assert!(playable.contains(era), "rolled an era nobody can open in: {era}");
        }
        let replay: Vec<usize> = (0..64).map(|seed| start_era(&args, seed)).collect();
        assert_eq!(rolled, replay, "the same seed must replay the same era");

        let distinct: std::collections::BTreeSet<usize> = rolled.iter().copied().collect();
        assert!(
            distinct.len() >= playable.len().min(5),
            "64 seeds reached only {} of {} eras: {distinct:?}",
            distinct.len(),
            playable.len()
        );
        // Lockstep would make each seed's era one past its neighbour's.
        let marching = rolled
            .windows(2)
            .filter(|pair| (pair[1] + playable.len() - pair[0]) % playable.len() == 1)
            .count();
        assert!(marching < rolled.len() / 2, "the eras march with the seed");

        // Without the flag the ladder is untouched and the seed is ignored.
        assert_eq!(start_era(&[], 1), start_era(&[], 999_999));
    }

    #[test]
    fn omitted_map_shape_defaults_to_planet() {
        assert_eq!(map_topology(&[]), MapTopology::Planet);

        let options = game_options(&[], 2, 71_004, TurnStructure::Sequential);
        let size = MapSize::for_players(2);
        assert_eq!(options.map_topology, MapTopology::Planet);
        assert_eq!(
            (options.width, options.height),
            size.dimensions(MapTopology::Planet)
        );

        let flat = vec!["--shape".to_string(), "flat".to_string()];
        assert_eq!(map_topology(&flat), MapTopology::Flat);
    }

    /// The turn-structure default is per command: the surfaces that exist
    /// for throughput (simulate, soak, a spectated table) hand this helper
    /// `Simultaneous`, the rating instruments and played games hand it
    /// `Sequential`, and an explicit flag always wins over either. The
    /// anchor `TurnStructure::default()` stays `Sequential` for saves and
    /// the setup contract — that half of the promise is asserted in
    /// `simultaneous.rs`.
    #[test]
    fn the_turn_structure_default_is_the_callers_and_the_flag_still_wins() {
        assert_eq!(
            turn_structure(&[], TurnStructure::Simultaneous),
            TurnStructure::Simultaneous
        );
        assert_eq!(
            turn_structure(&[], TurnStructure::Sequential),
            TurnStructure::Sequential
        );
        let sequential = vec!["--turn-structure".to_string(), "sequential".to_string()];
        assert_eq!(
            turn_structure(&sequential, TurnStructure::Simultaneous),
            TurnStructure::Sequential
        );
        let simultaneous = vec!["--turn-structure".to_string(), "simultaneous".to_string()];
        assert_eq!(
            turn_structure(&simultaneous, TurnStructure::Sequential),
            TurnStructure::Simultaneous
        );
        // The default threads through a whole options build unchanged.
        assert_eq!(
            game_options(&[], 4, 71_006, TurnStructure::Simultaneous).turn_structure,
            TurnStructure::Simultaneous
        );
        assert_eq!(
            game_options(&sequential, 4, 71_006, TurnStructure::Simultaneous).turn_structure,
            TurnStructure::Sequential
        );
    }

    /// The stock game somebody gets by asking for nothing: a Tennis Ball
    /// world. The four-seat half of the promise lives in the `play` arm's
    /// `--players` default, and the serde default stays Pangaea so a client
    /// that has never been taught the setting is unmoved.
    #[test]
    fn omitted_map_defaults_to_the_tennis_ball() {
        use civvis::setup::MapScript;

        let options = game_options(&[], 4, 71_005, TurnStructure::Sequential);
        assert_eq!(options.map_script, MapScript::TeninsBall);
        assert_eq!(
            options.mercy_rule, None,
            "a command-line game starts without mercy"
        );

        // An explicit choice still wins, under either accepted spelling.
        for (asked, chosen) in [
            ("pangaea", MapScript::Pangaea),
            ("tennis_ball", MapScript::TeninsBall),
            ("tenins_ball", MapScript::TeninsBall),
            ("tactics_planet", MapScript::TacticsPlanet),
        ] {
            let args = vec!["--map".to_string(), asked.to_string()];
            assert_eq!(
                game_options(&args, 4, 71_005, TurnStructure::Sequential).map_script,
                chosen,
                "{asked}"
            );
        }
        let planet = vec!["--map".to_string(), "tactics_planet".to_string()];
        let options = game_options(&planet, 2, 71_005, TurnStructure::Sequential);
        assert_eq!((options.width, options.height), (40, 18));
    }

    #[test]
    fn tournament_entrants_separate_immutable_identity_from_controller() {
        let entrants = parse_tournament_entrants(
            "advanced-20260801-policy-envoy=advanced, advanced_v1, basic-20260730=basic, random-20260730=random",
        )
        .unwrap();
        assert_eq!(entrants[0].identity, "advanced-20260801-policy-envoy");
        assert_eq!(entrants[0].controller, "advanced");
        assert_eq!(entrants[1].identity, "advanced_v1");
        assert_eq!(entrants[1].controller, "advanced_v1");
        assert_eq!(entrants[2].identity, "basic-20260730");
        assert_eq!(entrants[2].controller, "basic");
        assert!(parse_tournament_entrants("candidate=").is_err());
        assert!(parse_tournament_entrants("advanced,,basic").is_err());

        let default = parse_tournament_entrants(DEFAULT_TOURNAMENT_ENTRANTS).unwrap();
        assert_eq!(default[0].identity, "advanced-20260801-diplomacy");
        assert_eq!(default[0].controller, "advanced");
    }

    #[test]
    fn implicit_single_simulation_workers_are_bounded_without_changing_batches() {
        let implicit = Vec::new();
        let host_default = civvis::parallel::default_jobs();
        assert_eq!(jobs_arg(&implicit), host_default);
        assert_eq!(
            single_simulation_jobs_arg(&implicit),
            host_default.min(SINGLE_SIMULATION_DEFAULT_MAX_JOBS)
        );

        let explicit = vec!["simulate".to_string(), "--jobs".to_string(), "9".to_string()];
        assert_eq!(jobs_arg(&explicit), 9);
        assert_eq!(single_simulation_jobs_arg(&explicit), 9);
    }

    #[test]
    fn simultaneous_soak_uses_idle_batch_budget_for_seat_planning() {
        assert_eq!(simultaneous_soak_job_split(1, 128), (1, 128, 0));
        assert_eq!(simultaneous_soak_job_split(3, 128), (3, 42, 2));
        assert_eq!(simultaneous_soak_job_split(64, 8), (8, 1, 0));

        for (games, jobs) in [(1, 1), (2, 3), (3, 8), (8, 8), (20, 8)] {
            let (concurrent, per_game, extra) = simultaneous_soak_job_split(games, jobs);
            assert_eq!(concurrent * per_game + extra, jobs);
            assert!(extra < concurrent);
            assert!(concurrent <= games.max(1));
        }
    }

    /// The re-pin above claims the pantheon change is free for the Elo anchor
    /// because every legacy entrant plays at Standard, where the scaled price is
    /// exactly the old literal. ⚠ That is a load-bearing claim guarding a whole
    /// ratings ledger, and prose does not hold — the `_G` incident on 2026-08-03
    /// had TWO prose warnings in the repo and still shipped. Check it.
    #[test]
    fn elo_anchor_speed_is_standard_so_the_pantheon_repin_is_free() {
        use civvis::setup::GameSpeed;
        assert_eq!(
            GameSpeed::default(),
            GameSpeed::Standard,
            "if the default speed ever moves, the re-pin above stops being free and \
             ELO_PROTOCOL_VERSION must be bumped instead"
        );
        assert_eq!(
            GameSpeed::Standard.scale(civvis::game::PANTHEON_FAITH_STANDARD),
            25.0,
            "the scaled price must equal the literal it replaced, or the anchor's \
             behaviour changed and this is not a compatibility re-pin"
        );
    }

    /// The re-pin above claims the `settler_blocked_turns` change is free for the
    /// Elo anchor because the edited line sits behind `settler_commit`, which every
    /// default constructor leaves off. ⚠ That is load-bearing for a ratings ledger,
    /// and prose does not hold — check it.
    #[test]
    fn elo_anchor_never_reaches_the_settler_commit_path() {
        // ⚠ THE ANCHOR IS `legacy()`, NOT `new()` — `league.rs` maps
        // "advanced_v1" => AdvancedAi::legacy(). I first asserted this on `new()`,
        // which sets `settler_commit = true`, and this test failed and corrected me.
        // That is the whole reason the claim is checked rather than written down.
        assert!(
            !civvis::ai::AdvancedAi::legacy().settler_commit,
            "advanced_v1 is legacy(); if it ever reaches the settler_commit path the \
             re-pin above stops being free and ELO_PROTOCOL_VERSION must be bumped"
        );
        // The global Recovery front hold is likewise a production-only branch.
        // If the anchor ever enables it, its campaign movement may change and a
        // source re-pin alone would be invalid.
        assert!(
            !civvis::ai::AdvancedAi::legacy().bounded_recovery,
            "advanced_v1 is legacy(); if it ever carries bounded Recovery the global \
             front-hold branch reaches the anchor and ELO_PROTOCOL_VERSION must be bumped"
        );
        // ⚠ And record the other half honestly: `advanced` DOES set it, so that
        // entrant's settler pipeline genuinely changes. The anchor pins the scale
        // and is untouched, which is what this guard asks about — but v5 rows for
        // `advanced` straddle this change.
        assert!(civvis::ai::AdvancedAi::new().settler_commit);
        // ⚠ SAME QUESTION, ASKED AGAIN FOR THE GARRISON-LOYALTY ARM.
        //
        // The `limitanei` portfolio insert in `strategic_policies` is guarded by
        // `self.garrison_loyalty_policy`, and BOTH the anchor and the stock
        // entrant leave it false — only the eval-only arm
        // `advanced_garrison_loyalty` turns it on. So the source fingerprint
        // moved while the legacy path did not, and the re-pin below is free.
        //
        // Checked rather than asserted in a comment, because the last time this
        // was written down instead of tested the written claim was wrong.
        assert!(
            !civvis::ai::AdvancedAi::legacy().garrison_loyalty_policy,
            "advanced_v1 must not slot limitanei; if it ever does, the re-pin is \
             no longer free and ELO_PROTOCOL_VERSION must be bumped"
        );
        assert!(
            !civvis::ai::AdvancedAi::new().garrison_loyalty_policy,
            "the stock entrant must not slot limitanei either — the arm measured \
             a null and ships OFF"
        );
        // ⚠ SAME QUESTION, ASKED AGAIN FOR THE NUCLEAR LANE.
        //
        // The wmd-strike doctrine's wider gate (Recovery/threatened besides
        // Conquest) lives behind `advanced_command_actions`, and the new
        // nuclear tech beeline in `tech_value` is explicitly gated on
        // `victory_planning` — both paths the anchor never enters, because
        // legacy() constructs with victory_planning = false. So the source
        // fingerprint moved while the legacy path did not, and the re-pin
        // below is free.
        assert!(
            !civvis::ai::AdvancedAi::legacy().coordinates_forces(),
            "advanced_v1 must not victory-plan; if it ever does, the nuclear \
             beeline and strike doctrine reach the anchor and the re-pin is \
             no longer free — bump ELO_PROTOCOL_VERSION instead"
        );
        assert!(
            civvis::ai::AdvancedAi::new().coordinates_forces(),
            "the stock entrant does victory-plan, so `advanced` rows straddle \
             the nuclear-lane change — recorded here honestly"
        );
        let headless = Game::new(2, 24, 16, 71_032, 200, 0);
        assert!(
            headless
                .live_great_person_offer_blocker(0, "scientist")
                .is_none(),
            "the frozen headless anchor has no Firaxis named-offer export; if this \
             becomes populated, the live-only GPP gate can alter its ledger"
        );
        assert!(
            headless.players[0].live_great_person_offers.is_none()
                && headless.great_person_class_offered_now(0, "scientist"),
            "the frozen headless anchor has no Firaxis Great People screen, so its native \
             roster remains available; if this changes, the source re-pin is not free"
        );
        assert!(
            headless.players[0]
                .live_great_person_activation_needs
                .is_empty(),
            "the frozen headless anchor has no physical Firaxis Great Person units; \
             if this becomes populated, live activation infrastructure can alter \
             its ledger"
        );
    }

    #[test]
    fn advanced_v1_shared_sources_cannot_change_silently() {
        let mut fingerprint = 0xcbf29ce484222325u64;
        for source in [
            include_bytes!("ai.rs").as_slice(),
            include_bytes!("ai/advanced.rs").as_slice(),
        ] {
            for byte in source {
                fingerprint ^= u64::from(*byte);
                fingerprint = fingerprint.wrapping_mul(0x100000001b3);
            }
            fingerprint ^= 0xff;
            fingerprint = fingerprint.wrapping_mul(0x100000001b3);
        }
        assert_eq!(
            fingerprint, ADVANCED_V1_SOURCE_CONTRACT_FNV,
            "BasicAi/AdvancedAi changed under the advanced_v1 anchor: if the legacy path changed, bump ELO_PROTOCOL_VERSION and start a new ledger; otherwise review the gating and deliberately re-pin this source contract"
        );
    }

    /// The sea's live-only reconnaissance arm is sourced from the same AI
    /// files as the frozen anchor, so pin its gate independently of the
    /// broader repair bundle.
    #[test]
    fn naval_recon_cannot_reach_the_frozen_anchor() {
        for (name, ai) in [
            ("advanced_v1", civvis::ai::AdvancedAi::legacy()),
            ("advanced", civvis::ai::AdvancedAi::new()),
        ] {
            assert!(
                !ai.naval_recon(),
                "{name} carries live-only naval reconnaissance: the source-contract \
                 re-pin is valid only while this arm stays unreachable from the \
                 frozen rating anchor"
            );
        }
    }

    /// The re-pin above claims the engine-repair bundle cannot reach the
    /// frozen anchor. A comment claiming that is worth exactly as much as the
    /// comment that claimed native games leave `bounded_recovery` disabled —
    /// which was wrong, and is one of the re-pins listed above.
    ///
    /// So assert it on the constructors instead. `advanced_v1` is
    /// `AdvancedAi::legacy()` and the production incumbent is
    /// `AdvancedAi::new()`; neither may carry a repair. Only the three
    /// `advanced_synergy*` evaluator arms turn these on, and if that ever
    /// stops being true this fails before a rating anchor moves under a
    /// ledger that cannot see it.
    #[test]
    fn the_repair_bundle_cannot_reach_the_frozen_anchor() {
        for (name, ai) in [
            ("advanced_v1", civvis::ai::AdvancedAi::legacy()),
            ("advanced", civvis::ai::AdvancedAi::new()),
        ] {
            for (flag, on) in [
                ("muster_at_command_radius", ai.muster_at_command_radius),
                ("war_economy", ai.war_economy),
                ("war_reinforcement", ai.war_reinforcement),
                ("war_patience", ai.war_patience),
                ("endgame_war_runway", ai.endgame_war_runway),
                ("siege_commitment", ai.siege_commitment),
                ("relief_targets_the_siege", ai.relief_targets_the_siege),
                ("blind_objective_units", ai.blind_objective_units),
                ("blind_objective_strength", ai.blind_objective_strength),
                ("siege_tracks_the_wall", ai.siege_tracks_the_wall),
                ("army_target_weighs_the_enemy", ai.army_target_weighs_the_enemy),
                ("peacetime_deterrence", ai.peacetime_deterrence),
                ("strike_opening", ai.strike_opening),
                ("ranged_needs_line_of_sight", ai.ranged_needs_line_of_sight),
                ("loyalty_policy_defence", ai.loyalty_policy_defence),
                // Evaluator-only like the rest of this list: the stalled-settler
                // fallback must reach neither the anchor nor production until it
                // has a number.
                (
                    "settler_founds_when_stalled",
                    ai.settler_founds_when_stalled,
                ),
                ("fortify_idle_units", ai.fortify_idle_units()),
                (
                    "suzerain_cards_need_a_suzerainty",
                    ai.suzerain_cards_need_a_suzerainty,
                ),
                ("amenity_project_preemption", ai.amenity_project_preemption),
            ] {
                assert!(
                    !on,
                    "{name} carries the engine repair {flag}: the bundle measured \
                     a confirmed -108 Elo at deployment, and the re-pin that \
                     let it into the hashed sources was justified by this arm \
                     being unreachable from the anchor"
                );
            }
        }
    }

    #[test]
    fn strict_tournament_numbers_never_fall_back_on_malformed_input() {
        let args = vec![
            "tournament".to_string(),
            "--games".to_string(),
            "forty".to_string(),
            "--k".to_string(),
            "fast".to_string(),
        ];
        assert!(strict_i64_arg(&args, "--games", 20).is_err());
        assert!(strict_f64_arg(&args, "--k", 24.0).is_err());
        assert_eq!(strict_i64_arg(&args, "--players", 4).unwrap(), 4);
        assert!(strict_i64_arg(&["--games".to_string()], "--games", 20).is_err());
    }

    /// The soak's WAR block folds over the wars it can see, and
    /// `close_war_record` moves a finished war out of `Game::wars` into
    /// `Game::concluded_wars`. Reading the live map alone therefore hides
    /// every war that ended — which is every war that was *won*, and every
    /// white peace the block exists to make legible — and makes
    /// `ended_in_peace` structurally impossible to observe.
    #[test]
    fn a_settled_war_leaves_the_live_map_and_must_still_be_counted() {
        let mut game = Game::new(2, 24, 16, 5150, 400, 0);
        game.current = 0;
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
        assert_eq!(game.wars.len(), 1, "the declaration must open a war");
        assert!(game.concluded_wars.is_empty());

        // Peace is gated behind a mandatory war duration; wait it out.
        while let Some(until) = game.peace_available_at(0, 1) {
            assert!(until > game.turn, "the gate must advance the clock");
            game.turn = until;
        }
        game.apply(0, &Action::MakePeace { player: 1 }).unwrap();

        assert!(
            game.wars.is_empty(),
            "a settled war must not remain in the live map"
        );
        assert_eq!(
            game.concluded_wars.len(),
            1,
            "the settled war has to be read from concluded_wars or it is \
             invisible to every count in the soak line"
        );
        assert!(game.concluded_wars[0].ended.is_some());
    }
}
