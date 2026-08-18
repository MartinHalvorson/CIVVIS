//! Every `enable_*` / `disable_*` capability toggle on `AdvancedAi`.
//!
//! ★★★ THIS IS THE SECOND OF THE TWO ANCHORS A TREATMENT PR TOUCHES, AND THE
//! ONE THAT HAS ACTUALLY CORRUPTED A MERGE. `treatments.rs` moved the
//! `LIVE_TREATMENTS` table out on 2026-08-18 (#2022) and recorded in its own
//! header that the toggle pair still landed in `advanced.rs`, that two PRs
//! adding a pair at the same anchor had collided there twice on 2026-08-16,
//! and that one of those resolutions swallowed the earlier function's closing
//! brace. It called moving the toggles "its own change with its own risk" and
//! deliberately shipped the small half. This is the other half.
//!
//! `tools/conflict_hotspots.py` measures `src/ai/advanced.rs` at 24% of the
//! last 200 merges, first by a wide margin. `docs/ROADMAP.md` objective 5
//! separates the two reasons a file is contended: **size**, which splitting
//! answers, and **one shared line or list**, which it does not — two PRs
//! appending at the same anchor conflict whatever the file's length. The
//! toggles are the second kind. They are 182 near-identical four-line
//! functions with no readers other than the field they set, so a conflict
//! between two of them is content-free; what made it dangerous was that it
//! happened at line 3,859 of twenty-eight thousand, surrounded by code a
//! careless resolution could take with it.
//!
//! Here, a toggle collision is a collision between two four-line functions in
//! a file that contains nothing else. It is still a conflict. It can no longer
//! swallow anything.
//!
//! ⚠ **The move is only worth anything if it stays moved.** A guard in this
//! file fails when an `enable_*` or `disable_*` method is defined in
//! `advanced.rs` again, and when a function that is *not* a toggle is defined
//! here — so this does not become the next dumping ground. Both run in
//! `cargo test`, which is a required check.
//!
//! ⚠ Adding a treatment still touches more than one place: the flag field on
//! the struct and the `enable_live_bridge` bundle body (both here or in
//! `advanced.rs`), the `LIVE_TREATMENTS` row in `treatments.rs`, and the
//! withholding arm in `elo.rs`. This file removes one of them from the
//! hotspot; it does not claim to have removed the others.

use super::AdvancedAi;
use crate::game::Game;

impl AdvancedAi {
    /// Enable the narrow Trader adaptation required by a live Civilization VI
    /// export.  Native tournament games leave this disabled.
    pub fn enable_live_trader_route_adapter(&mut self) {
        self.live_trader_route_adapter = true;
    }

    /// Withholding twin for `enable_live_trader_route_adapter`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_live_trader_route_adapter(&mut self) {
        self.live_trader_route_adapter = false;
    }

    /// Enforce Firaxis's city-majority rule for live religious purchases.
    /// Native tournament games leave this disabled.
    pub fn enable_live_religious_purchase_guard(&mut self) {
        self.base.live_religious_purchase_guard = true;
    }

    /// Withholding twin for `enable_live_religious_purchase_guard`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_live_religious_purchase_guard(&mut self) {
        self.base.live_religious_purchase_guard = false;
    }

    /// Let a besieged city raise its standing-army floor against hostiles it
    /// has no diplomatic state with. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_siege_muster(&mut self) {
        self.base.siege_muster = true;
    }

    /// Let a raider standing in our own territory claim a unit before the
    /// offensive does. Native tournament games leave this disabled so their
    /// recorded ladders stay comparable.
    pub fn enable_home_defense(&mut self) {
        self.base.home_defense = true;
    }

    /// Stop a Settler that has stopped walking from holding the expansion gate
    /// shut. Native tournament games leave this disabled so their recorded
    /// ladders replay the historical controller move for move.
    pub fn enable_stranded_settler_discount(&mut self) {
        self.base.settler_strand_discount = true;
        // Discounting the stuck body lets another Settler enter the pipeline,
        // but does not convert the production already standing on legal, safe
        // ground. Once the bounded stall counter expires, finish that city
        // instead of beginning another target cycle.
        self.settler_founds_when_stalled = true;
    }

    /// Record tactical steps so a unit stepped twice in one turn cannot walk
    /// back onto the tile it just left. Native tournament games leave this
    /// disabled so their recorded ladders replay move-for-move.
    pub fn enable_recorded_tactical_step(&mut self) {
        self.base.recorded_tactical_step = true;
    }

    /// Withholding twin for `enable_recorded_tactical_step`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_recorded_tactical_step(&mut self) {
        self.base.recorded_tactical_step = false;
    }

    /// Price the city ceiling off uncontested land. Native tournament games
    /// leave this off so recorded ladders stay comparable; see
    /// `wide_map_capacity` for the live Settler measurement.
    pub fn enable_wide_map_capacity(&mut self) {
        self.wide_map_capacity = true;
    }

    pub fn disable_wide_map_capacity(&mut self) {
        self.wide_map_capacity = false;
    }

    /// Price the live city ceiling off the whole map's land, read through the
    /// fog. See `fog_land_capacity`.
    pub fn enable_fog_land_capacity(&mut self) {
        self.fog_land_capacity = true;
    }

    pub fn disable_fog_land_capacity(&mut self) {
        self.fog_land_capacity = false;
    }

    /// Let two Settlers walk at once (see `parallel_settlers`). Set by the
    /// Civilization VI bridge only; native constructors and the frozen anchor
    /// keep the one-at-a-time gate in both settler routes.
    pub fn enable_parallel_settlers(&mut self) {
        self.parallel_settlers = true;
        self.base.parallel_settlers = true;
    }

    /// Withhold the live seat's second Settler pipeline so the evaluator can
    /// price it against the same otherwise-complete deployment bundle.
    pub fn disable_parallel_settlers(&mut self) {
        self.parallel_settlers = false;
        self.base.parallel_settlers = false;
    }

    /// Enable the evaluator-only paid expansion treatment. It also routes the
    /// adaptive Expansion plan through `advanced_production`; otherwise the
    /// ordinary Cities governor would never consult the coupled scorer.
    pub fn enable_coupled_expansion(&mut self) {
        self.coupled_expansion = true;
        self.expansion_dispatch = true;
    }

    /// Withhold the coupled expansion treatment, preserving the stock
    /// production score and the ordinary adaptive dispatcher setting.
    pub fn disable_coupled_expansion(&mut self) {
        self.coupled_expansion = false;
    }

    /// Build a Settler at the host's population floor (see
    /// `BasicAi::host_settler_pop`). Set by the Civilization VI bridge only;
    /// native constructors and the frozen anchor keep the genome's figure.
    pub fn enable_host_settler_pop(&mut self) {
        self.base.enable_host_settler_pop();
    }

    /// Withhold the host population floor from one live evaluator arm.
    pub fn disable_host_settler_pop(&mut self) {
        self.base.host_settler_pop = false;
    }

    /// Give up an exploration target the host will not move the unit toward
    /// (see `BasicAi::explore_dead_targets`). Set by the Civilization VI bridge
    /// only.
    pub fn enable_explore_dead_targets(&mut self) {
        self.base.enable_explore_dead_targets();
    }

    /// Withhold target retirement from one live evaluator arm.
    pub fn disable_explore_dead_targets(&mut self) {
        self.base.explore_dead_targets = false;
    }

    /// Hold an exploration goal and sweep outward from home (see
    /// `BasicAi::explore_commit`). Set by the Civilization VI bridge only.
    pub fn enable_explore_commit(&mut self) {
        self.base.enable_explore_commit();
    }

    /// A city losing hitpoints is besieged, whatever the fog says. See
    /// `BasicAi::garrison_under_fire` for the t115 measurement.
    pub fn enable_garrison_under_fire(&mut self) {
        self.base.garrison_under_fire = true;
    }

    pub fn disable_garrison_under_fire(&mut self) {
        self.base.garrison_under_fire = false;
    }

    /// Order our own ancient walls in the capital and small frontier cities
    /// once Masonry is in. Native tournament games leave this disabled so
    /// their recorded ladders stay comparable; see
    /// `BasicAi::garrison_walls_item` for the t115 measurement.
    pub fn enable_garrison_walls(&mut self) {
        self.base.garrison_walls = true;
    }

    pub fn disable_garrison_walls(&mut self) {
        self.base.garrison_walls = false;
    }

    /// Release an escort that is not walking its settler. See `escort_unstick`.
    pub fn enable_escort_unstick(&mut self) {
        self.escort_unstick = true;
    }

    pub fn disable_escort_unstick(&mut self) {
        self.escort_unstick = false;
    }

    /// Escort settlers by stacked co-movement instead of formations. See
    /// `stacked_escort` for the 0-for-7 live formation record and the two
    /// doorstep captures that motivated it.
    pub fn enable_stacked_escort(&mut self) {
        self.stacked_escort = true;
    }

    pub fn disable_stacked_escort(&mut self) {
        self.stacked_escort = false;
        self.settler_guards.clear();
        self.guard_wait.clear();
    }

    /// Settlers decide before the engagement, price capture as capture and
    /// trust only a guard on their tile. See `settler_stack_discipline`.
    pub fn enable_settler_stack_discipline(&mut self) {
        self.settler_stack_discipline = true;
    }

    pub fn disable_settler_stack_discipline(&mut self) {
        self.settler_stack_discipline = false;
    }

    /// The peacetime camp party. See `BasicAi::camp_party`.
    pub fn enable_camp_party(&mut self) {
        self.base.enable_camp_party();
    }

    pub fn disable_camp_party(&mut self) {
        self.base.disable_camp_party();
    }

    /// A Religion strategy offers peace to unblock its spread lane. See
    /// `religion_sues_peace` for the t200 measurement.
    pub fn enable_religion_sues_peace(&mut self) {
        self.religion_sues_peace = true;
    }

    pub fn disable_religion_sues_peace(&mut self) {
        self.religion_sues_peace = false;
    }

    /// Enable explicit battlefield roles: the land-unit counter cycle, safe
    /// ranged standoff, wall-focused siege/support, and cavalry job priority.
    /// Production Advanced enabled this at construction until 2026-08-14, when
    /// the war-half withhold passed the promotion matrix (+38, CI +10..+66,
    /// seed stream 18000000; see `promoted_policy_envoy`); now only the
    /// `advanced_war_half` re-addition arm and focused evaluator controls set
    /// it.
    pub fn enable_tactical_strategy(&mut self) {
        self.base.tactical_strategy = true;
    }

    /// Withhold the tribal-village pickup that production Advanced carries by
    /// default (see `BasicAi::hut_collection`), so the evaluator arm
    /// `advanced_without_hut_collection` can price it.
    pub fn disable_hut_collection(&mut self) {
        self.base.hut_collection = false;
    }

    /// Withhold the charted-village detour that production Advanced carries
    /// by default (see `BasicAi::village_seeking`), so the evaluator arm
    /// `advanced_without_village_seeking` can price it.
    pub fn disable_village_seeking(&mut self) {
        self.base.village_seeking = false;
    }

    /// Withhold the committed exploration goal that production Advanced
    /// carries by default (see `BasicAi::explore_commit`), so the evaluator
    /// arm `advanced_without_explore_commit` can price it.
    pub fn disable_explore_commit(&mut self) {
        self.base.explore_commit = false;
    }

    /// Let a unit retain its campaign objective and a short, threat-driven
    /// retreat across turns. Production Advanced enabled this by default until
    /// 2026-08-14 (the war-half removal; see `promoted_policy_envoy`); the
    /// explicit method keeps focused evaluators able to opt in deliberately.
    pub fn enable_unit_objective_memory(&mut self) {
        self.base.unit_objective_memory = true;
    }

    /// Stop the defensive-war posture from becoming permanent.
    ///
    /// Production leaves this measured-null repair off. The live bridge and
    /// explicit evaluator bundles call this method when they need the repair;
    /// keeping the switch here makes that opt-in auditable.
    pub fn enable_bounded_recovery(&mut self) {
        self.bounded_recovery = true;
    }

    /// Hold a promotion until its healing would land. Native/eval only; the
    /// live bridge does not set this.
    pub fn enable_loyalty_policy_defence(&mut self) {
        self.loyalty_policy_defence = true;
    }

    pub fn disable_loyalty_policy_defence(&mut self) {
        self.loyalty_policy_defence = false;
    }

    pub fn enable_promote_when_wounded(&mut self) {
        self.promote_when_wounded = true;
    }

    /// Let movement credit the attack a tile opens. Native tournament games
    /// leave this disabled so their recorded ladders stay comparable.
    pub fn enable_strike_opening(&mut self) {
        self.strike_opening = true;
    }

    /// Withholding twin for `enable_strike_opening`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_strike_opening(&mut self) {
        self.strike_opening = false;
    }

    /// Let a ranged unit prefer tiles it can actually shoot from. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_ranged_needs_line_of_sight(&mut self) {
        self.ranged_needs_line_of_sight = true;
    }

    /// Withholding twin for `enable_ranged_needs_line_of_sight`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_ranged_needs_line_of_sight(&mut self) {
        self.ranged_needs_line_of_sight = false;
    }

    /// ★★★★★ THE FAITH PRICE THE AI READS IS THE STANDARD-SPEED ONE.
    ///
    /// `spec.cost * 2.0` is the Faith rate at Standard speed, and `item_cost`
    /// scales every price by `game_speed`. Online — the speed the deployment
    /// profile and the live bridge both play — is 50%, so that literal asks
    /// for **twice** the Faith the engine would take, and the reserve is then
    /// applied on top of a doubled price. Marathon is 300% and it underquotes
    /// by a third, issuing purchases the engine refuses. It also ignores every
    /// discount that moves the number: the founder's belief, Theocracy, the
    /// Holy Site's own purchase discount, a Guru's wonder discount. The same
    /// defect priced Rock Bands, which additionally climb 100 per band already
    /// bought. `unit_purchase_cost` knows all of it and returns None when the
    /// purchase is illegal here, rather than leaving that to a refused order.
    ///
    /// Off by default: it changes play and has not been screened.
    pub fn enable_engine_faith_price(&mut self) {
        self.engine_faith_price = true;
    }

    /// Let the army target account for the enemy it has to beat. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_army_target_weighs_the_enemy(&mut self) {
        self.army_target_weighs_the_enemy = true;
    }

    /// Let the strongest met major weigh on the army target while at peace,
    /// so deterrence exists before a declaration. Native tournament games
    /// leave this disabled so their recorded ladders stay comparable.
    pub fn enable_peacetime_deterrence(&mut self) {
        self.peacetime_deterrence = true;
    }

    /// See [`SUZERAIN_PRIZE`]. Off on the anchor, so a comparison against it
    /// measures the term rather than a rename.
    pub fn enable_price_the_suzerainty(&mut self) {
        self.price_the_suzerainty = true;
    }

    pub fn enable_suzerain_cards_need_a_suzerainty(&mut self) {
        self.suzerain_cards_need_a_suzerainty = true;
    }

    /// Let the siege train be sized by the wall it has to breach. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    /// Let the unit chooser ask for siege as a role. Native tournament games
    /// leave this disabled.
    pub fn enable_siege_role(&mut self) {
        self.base.siege_role = true;
    }

    /// Rebuild the recon arm when it is gone and there is ground left to chart.
    /// Native tournament games leave this disabled so their recorded ladders
    /// stay comparable.
    pub fn enable_recon_replacement(&mut self) {
        self.base.recon_replacement = true;
    }

    pub fn disable_recon_replacement(&mut self) {
        self.base.recon_replacement = false;
    }

    /// Buy one ship for an empire that has none while unexplored water lies
    /// off its coast, and send it exploring. See `BasicAi::naval_recon` and
    /// `naval_explorer`.
    pub fn enable_naval_recon(&mut self) {
        self.base.naval_recon = true;
    }

    pub fn disable_naval_recon(&mut self) {
        self.base.naval_recon = false;
    }

    /// Count a barbarian camp within nine tiles of a city as home ground the
    /// guard clears. See `BasicAi::camp_reach`.
    pub fn enable_camp_reach(&mut self) {
        self.base.enable_camp_reach();
    }

    pub fn disable_camp_reach(&mut self) {
        self.base.disable_camp_reach();
    }

    /// Price a revealed natural wonder's ring into the settle scorer. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_wonder_ring_settle_value(&mut self) {
        self.base.wonder_ring_settle_value = true;
    }

    pub fn disable_wonder_ring_settle_value(&mut self) {
        self.base.wonder_ring_settle_value = false;
    }

    pub fn disable_siege_role(&mut self) {
        self.base.siege_role = false;
    }

    /// Keep the land army out of the water. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_come_ashore(&mut self) {
        self.base.come_ashore = true;
    }

    pub fn disable_come_ashore(&mut self) {
        self.base.come_ashore = false;
    }

    pub fn enable_siege_tracks_the_wall(&mut self) {
        self.siege_tracks_the_wall = true;
    }

    /// Stop a fogged objective city from reading as an empty tile when the
    /// army decides whether it is strong enough to engage. Native tournament
    /// games leave this disabled so their recorded ladders stay comparable.
    pub fn enable_blind_objective_strength(&mut self) {
        self.blind_objective_strength = true;
    }

    /// Judge force readiness at the radius the group was assembled at. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_muster_at_command_radius(&mut self) {
        self.muster_at_command_radius = true;
    }

    /// Send an adaptive Conquest plan through the war production path. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_war_economy(&mut self) {
        self.war_economy = true;
    }

    /// March rear units to the campaign objective while the war is on. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_war_reinforcement(&mut self) {
        self.war_reinforcement = true;
    }

    /// Keep prosecuting a war the empire overwhelmingly outweighs instead of
    /// suing it out as stalled. Native tournament games leave this disabled so
    /// their recorded ladders stay comparable.
    pub fn enable_war_patience(&mut self) {
        self.war_patience = true;
    }

    /// See [`Self::deny_while_targeted`].
    pub fn enable_deny_while_targeted(&mut self) {
        self.deny_while_targeted = true;
    }

    pub fn disable_deny_while_targeted(&mut self) {
        self.deny_while_targeted = false;
    }

    /// See [`Self::stock_denial_lead_time`].
    pub fn enable_stock_denial_lead_time(&mut self) {
        self.stock_denial_lead_time = true;
    }

    pub fn disable_stock_denial_lead_time(&mut self) {
        self.stock_denial_lead_time = false;
    }

    /// Keep a fresh direct declaration out of the final campaign reserve.
    /// Native tournament games leave this disabled so their recorded ladders
    /// stay comparable.
    pub fn enable_endgame_war_runway(&mut self) {
        self.endgame_war_runway = true;
    }

    /// Keep a live campaign pointed at its chosen city. A breach gets an
    /// additional value credit, but even an intact objective needs several
    /// turns of marching and bombardment before that credit exists; changing
    /// the objective every assessment strands the whole army between cities.
    /// An emergency, a changed war target, or capture immediately releases the
    /// commitment. Native tournament games leave this disabled so their
    /// recorded ladders stay comparable.
    pub fn enable_siege_commitment(&mut self) {
        self.siege_commitment = true;
    }

    /// Send a relief force at the units actually besieging the city rather than
    /// the nearest one to itself. Native tournament games leave this disabled so
    /// their recorded ladders stay comparable.
    pub fn enable_relief_targets_the_siege(&mut self) {
        self.relief_targets_the_siege = true;
    }

    /// Let the army price the enemy units it REMEMBERS around an objective it
    /// cannot currently see, instead of reading an unseen approach as empty.
    /// Native tournament games leave this disabled so their recorded ladders
    /// stay comparable.
    pub fn enable_blind_objective_units(&mut self) {
        self.blind_objective_units = true;
    }

    /// ★★★★★ EVERY LIVE-BRIDGE REPAIR, IN ONE PLACE THAT THE MEASUREMENT CAN PLAY.
    ///
    /// These eight flags are the difference between the frozen tournament
    /// controller and the agent that actually plays Civilization VI. They were
    /// set one by one inside `civvis_orders::decide`, which is a binary — so no
    /// headless arm could construct the deployed agent, and **not one of them
    /// has ever been measured on an outcome.** #930, #933, #955, #957 and #962
    /// all shipped on reasoning plus a live anecdote.
    ///
    /// Collecting them here gives `builtin_ai` a `live` controller to play, so
    /// `civvis tournament --ais live,live_without_<flag>` can price any one of
    /// them in cities and score instead of in order counts. The bridge calls
    /// this same function, so the measured agent and the deployed agent cannot
    /// drift apart.
    ///
    /// ⚠ ADD NEW BRIDGE FLAGS HERE, not in the binary, or the arm silently
    /// stops matching the deployment — the exact shape of
    /// `civvis-the-runner-tree-was-the-broken-link`.
    pub fn enable_live_bridge(&mut self) {
        self.enable_live_trader_route_adapter();
        self.enable_live_religious_purchase_guard();
        // ⚠ Barbarians are excluded from `at_major_war` by design, so every defensive
        // escalation in the production picker reads a barbarian siege as no threat at
        // all: a one-city empire's standing-army floor stays at `mil_per_city` (1.0)
        // and it cannot want a third defender while horsemen stand on its doorstep.
        // Measured on run `civvis-20260802T202501Z` — four settlers built into that
        // siege and captured, two on the capital tile without ever moving, one city
        // until t80, score 140 against a best rival's 416. The tournament controller
        // stays frozen so its recorded ladders remain comparable.
        self.enable_siege_muster();
        // ⚠ And once it CAN want the defenders, something has to send them. Measured
        // on run `civvis-20260803T005930Z` (Kongo, 154 turns): **116 of 154 turns had
        // a hostile standing inside or beside our own territory**, including a
        // full-health Crossbowman parked four tiles from two cities, unmoved and
        // unengaged, for 21 consecutive turns, while the whole seven-unit army stood
        // eight tiles away on a war front that had taken nothing in 75 turns. The
        // cause is `nearest_enemy` ranking targets by distance FROM THE ASKING UNIT,
        // which for a deployed army is always the enemy's cities. The tournament
        // controller stays frozen so its recorded ladders remain comparable.
        self.enable_home_defense();
        self.enable_loyalty_policy_defence();
        // ⚠ `assess` drops the empire into Recovery whenever it is at war and
        // `my_power * 1.25 < strongest_rival`, and Recovery does not build an army —
        // so the test stays true because of the choice it caused. Measured on run
        // `civvis-20260802T205959Z`: the journal names that arm 160 times, the
        // posture held from t65 to t229 (72% of the game), and the empire finished
        // with ONE warrior at military 34 against the Mapuche's 1354. The bound
        // releases only the power-gap half, and only after the posture has had
        // `RECOVERY_POSTURE_LIMIT` standard turns to work.
        // ⚠ A tactical step applied raw records nothing, so when a unit with
        // movement left is stepped a second time in the same turn, the reversal
        // guard inside `path_move` cannot see where it came from — and round two
        // walks straight back onto the tile round one just left. Measured on the
        // replay of run `civvis-20260801T224944Z`: 217 of 217 refused moves were
        // exactly that out-and-back pair; self-tile orders fell 43 → 1 with the
        // steps recorded.
        self.enable_recorded_tactical_step();
        self.enable_bounded_recovery();
        // ⚠ `desired_military` is `2 * city_count` at war — a headcount keyed to
        // OUR empire that never asks how strong the rival is. Once it is met,
        // `force_gap` hits zero and the military arm of `production_value` drops
        // from a 4.0 multiplier to 0.65, so units lose to buildings. Measured on
        // run `civvis-20260803T005930Z`: 94 of 188 war turns had the target
        // already satisfied, CIVVIS ordered 17 military units in the whole war
        // (8 land combat, 2 siege) against Korea's five walled cities, and at t240
        // the rival fielded 1050 military against our 658 while the target still
        // read satisfied at 11 against a wanted 10.
        self.enable_army_target_weighs_the_enemy();
        // ⚠ And the wartime repair above still wakes up only once the war has
        // started. Measured on run `civvis-20260803T220954Z` (Rome, 250 turns):
        // seven cities founded by t123, Mali declared at t157 holding **894
        // military against our 481**, and six of the seven were taken at
        // loyalty 100 — sieges, not revolts — including Rome itself at t225.
        // We issued zero war orders and sixteen refused peace requests. A
        // target that asks "who could kill me" only after the declaration is
        // asking too late; this floor asks it of the strongest MET major in
        // peacetime, under its own far smaller ceiling.
        self.enable_peacetime_deterrence();
        // Three straight Settler losses were an eight-city empire against
        // ten- and eleven-city rivals; the stock nine-ceiling was the binding
        // constant. See `wide_map_capacity`.
        self.enable_wide_map_capacity();
        // And that ceiling was still priced off the revealed quarter of the
        // map. See `fog_land_capacity`.
        self.enable_fog_land_capacity();
        // The other half of the same three-defeat measurement: the capital that
        // fell bleeding with an empty hostile list. See garrison_under_fire.
        self.enable_garrison_under_fire();
        // The other half of that same capital's diagnosis: garrison_under_fire
        // reacts to a city already bleeding, but the capital that fell had
        // NEVER ORDERED WALLS — max_wall_damage 0 at t115 with production on
        // the culture lane and the fog hiding every attacker until adjacency.
        // See BasicAi::garrison_walls_item.
        self.enable_garrison_walls();
        // Settler conversion is the score frontier the first seven live games
        // isolated; see escort_unstick.
        self.enable_escort_unstick();
        // And the formation channel that escort depends on went 0-for-7 on
        // the live bridge while two unescorted settlers were captured one
        // turn short of founding; see stacked_escort.
        self.enable_stacked_escort();
        // And the settler decides on the real board, prices capture as
        // capture and trusts only a guard on its tile; see
        // settler_stack_discipline.
        self.enable_settler_stack_discipline();
        // The religion lane was structurally blocked by its own wars; see
        // religion_sues_peace.
        self.enable_religion_sues_peace();
        // Raj, Wisselbanken, Collective Activism and the International Space
        // Agency all scale off SUZERAIN city-states and pay nothing at zero.
        // Live run `civvis-20260803T220954Z` held Raj AND Wisselbanken slotted
        // at turn 208 with 0 suzerainties and 41 unspent envoys — two of six
        // slots returning zero for the whole game.
        self.enable_suzerain_cards_need_a_suzerainty();
        // ⚠ The siege appetite was one unit for any target city at all, walled
        // or not. The engine halves a non-siege unit's wall damage
        // (`mult = if spec.siege { 1.0 } else { 0.5 }`) and docks a non-siege
        // ranged unit a flat 17 attack for shooting a city, so an army without a
        // siege train pays twice. Measured on run `civvis-20260803T005930Z`: four
        // siege units across 251 turns against a Korea holding five walled cities;
        // 27 turns in contact with Jinju and Jeonju removed 12 and 9 points of a
        // 400-point wall, while Korea stripped Kwango's 400 in six.
        // ⚠ And the appetite is useless if the chooser cannot offer a siege
        // unit at all. `best_military` split the world into melee and ranged;
        // every siege unit has a ranged attack, so it competed on raw strength
        // and lost to a Field Cannon. Measured on run `civvis-20260803T082856Z`
        // — a game CIVVIS was WINNING — 151 turns at war at 10:1, ZERO cities
        // taken, zero siege units built in 251 turns with every siege tech in
        // hand. The tournament controller stays frozen.
        self.enable_siege_role();
        // ⚠ The empire goes blind and the build order never notices. Recon is
        // not among the counts `pick_item` receives, and `OPENING_MENU` is the
        // only place a scout is named, so once the openers die nothing replaces
        // them. Live run `civvis-20260808T142724Z`: zero recon units from turn
        // ~100 to 251 while the army grew to 22, 77% of the map never seen, and
        // the eventual winner first met on turn 215 already holding 927 points.
        self.enable_recon_replacement();
        // And the recon it rebuilds must stop walking into barbarians. See
        // `recon_flight`.
        self.enable_recon_flight();
        // And a settler target dropped for danger stays dropped for a while.
        // See `settler_target_hysteresis`.
        self.enable_settler_target_hysteresis();
        // And the last fifty turns are a tally, not a launch window. See
        // `score_horizon`.
        self.enable_score_horizon();
        // And the race that does fit needs one launch pad, not one per city.
        // See `one_launch_pad`.
        self.enable_one_launch_pad();
        // And the sea gets one eye of its own. See `BasicAi::naval_recon`.
        self.enable_naval_recon();
        // And a camp within nine tiles of a city is home ground the guard clears.
        // See `BasicAi::camp_reach`.
        self.enable_camp_reach();
        // And in peacetime the whole field army clears it. See
        // `BasicAi::camp_party`.
        self.enable_camp_party();
        // ⚠ A revealed natural wonder is priced into founding only through the
        // worked tiles the growth forecast can see and a future Holy Site's
        // adjacency — for the Matterhorn about 2-4 points — while everything
        // else a wonder-ring city collects (pantheon faith, appeal districts
        // and parks, late tourism) is invisible to settling, so a breadbasket
        // outbids any wonder ring by construction. Live run
        // `civvis-20260807T202450Z` t93 (issue #1378): the settler founded a
        // 64.6-point site while `FEATURE_MATTERHORN` stood revealed inside the
        // candidate radius. The tournament controller stays frozen so its
        // recorded ladders remain comparable.
        self.enable_wonder_ring_settle_value();
        // ⚠ `unit_can_traverse` says yes to open water for every land unit as
        // soon as embarkation unlocks, so the unexplored ocean becomes a legal
        // exploration goal for the whole army — and the only rule that brought
        // a unit back was welded to `modernization_step`, which does nothing
        // for a unit with no upgrade waiting. Measured across 133 live runs:
        // land combat units spend a mean 15% of their unit-turns embarked, and
        // 21.7% while one of our own cities is taking damage (92.8% in the
        // worst run). An embarked unit cannot attack. On run
        // `civvis-20260803T130831Z` the capital sat at 179/200 damage for 38
        // turns while 11 of 12 land combat units swam. The tournament
        // controller stays frozen.
        self.enable_come_ashore();
        self.enable_siege_tracks_the_wall();
        // ⚠ `local_strength_ratio` prices an objective city only while it is
        // currently in sight, and returns its `hostile <= 0.0` sentinel of 3.0 —
        // the maximum — otherwise. Under live fog that makes a walled enemy
        // capital score identically to an empty meadow, four times over the
        // superiority floor, so the army engages. Measured on run
        // `civvis-20260803T005930Z`: Seoul (walled, 22 pop, defense 101) was the
        // objective of 426 force-group decisions from t65 to t231 and 294 of them
        // read exactly 3.00, 108 with a force of one. No Korean city ever passed
        // 27% damage in 173 turns of war. The repair reads only this controller's
        // own last sighting, which the defensive half already trusts.
        // ⚠ And once the army is sent, something has to make it close. The
        // movement score charges `mv_threat * threat_caution * 30.0` for
        // standing where an enemy can reach — -15.0 at parity for a Vanguard —
        // and credits the attack that position buys with nothing, while
        // closing one hex is worth +2.9. Measured on run
        // `civvis-20260803T005930Z`: **7 melee ATTACK orders in 188 turns of
        // war** against 1546 MOVE_TO, with 622 military unit-turns sitting 2-4
        // hexes from a target and 52% of those under an Engage posture. The
        // tournament controller stays frozen so its recorded ladders remain
        // comparable.
        self.enable_strike_opening();
        // ⚠ And the largest single reason the army does not shoot. Of 87
        // declined attacks on a replay of run `civvis-20260803T005930Z`, 45
        // were the forward model refusing outright and **line of sight blocked
        // was 25 of those** — 27 of the 45 a Field Cannon. Movement picks a
        // ranged unit's tile by distance and preferred depth and never asks
        // whether the target is visible from it, so the unit marches exactly
        // into range and cannot fire.
        self.enable_ranged_needs_line_of_sight();
        self.enable_blind_objective_strength();
        self.enable_muster_at_command_radius();
        self.enable_relief_targets_the_siege();
        self.enable_blind_objective_units();
        // ⚠ THE EXPANSION GATE ASKS `settlers == 0`, so a settler that never
        // founds anything answers "one is already in flight" for the rest of
        // the game. Across the 25 live runs of 2026-08-07/08 that reason is
        // 86% of every refusal to build a settler (1,548 of 1,767), the median
        // game's longest-lived single settler survives 86 turns of 250 without
        // founding, and nine runs carried one alive for 82-171 turns having
        // moved five times or fewer. Those empires finished on a median of 5
        // cities against a `city_target` of 7.8 with the window open to turn
        // 198 — neither was binding, this was — and lost all 21 completed
        // games at a median score 0.46x the leader's, with final score
        // tracking city count at r = 0.81. The mod's fallback ladder already
        // made this repair; under `--civvis-decides` it is not the decider.
        self.enable_stranded_settler_discount();
        // ⚠ Faith buys the soldier; GOLD pays for it every turn forever, and
        // `military_faith_spending` never asks about gold — it gates on the faith
        // bank alone. Measured on run `civvis-20260803T014330Z`: faith military
        // purchases walked down the gold curve (t124 at 60 gold, t141 at 48, t165 at
        // 51), the treasury hit zero on t168, Civilization VI disbanded the army
        // from 29 units to 19 by t173, and on t174 — at FIVE gold, one turn after
        // losing a third of the army — CIVVIS bought another Field Cannon. The
        // tournament controller stays frozen so its recorded ladders stay
        // comparable.
        // ⚠ Loyalty is the LARGEST single cause of city loss and the AI was
        // reading only the level. Classified over every recorded run, 125 city
        // losses: 52 (41.6%) below loyalty 50, 37 (29.6%) loyal-and-damaged, and
        // 36 (28.8%) gone from FULL health and full loyalty in one round. 66 of
        // the 125 were carrying a negative loyalty rate when last seen, and
        // `Game::city_loyalty_per_turn` — mirrored from Civilization VI all along
        // — had zero consumers in the whole AI. The tournament controller stays
        // frozen so its recorded ladders remain comparable.
        self.enable_loyalty_rate_alarm();
        self.enable_solvent_faith_army();
        // ⚠ PENDING MEASUREMENT. First arm at deployment shape (74x46, 9 city-states,
        // 16 games, 200 turns) read +35 Elo (CI -61..+130), direction 7-2, sign
        // p=0.1797 — inconclusive, the promotion gate did not fire, terminal score
        // flat at 23-22. A weak positive is not a result. A 40-game arm is running;
        // if it does not clear, this call and its `live_without_` arm come back out
        // rather than sitting here unpriced.
        //
        // ⚠ It cannot be parked "off but registered": `each_live_without_arm_holds_
        // exactly_one_treatment_off` and `live_bridge_treatments_name_every_flag_
        // the_helper_sets` (#988) require the treatment list, the arms and this
        // helper to agree exactly. A flag the deployment does not set has no
        // `live_without_` arm — which is the invariant working, and the reason a
        // gated-off flag here would be dead code of the `culture_focus` kind.
        self.enable_district_coverage();
        self.enable_slot_kind_tiebreak();
        // ⚠⚠ WARS ARE REACHED BUT NEVER CONVERTED. Across the fifteen live
        // Settler games of 2026-08-07/08 the ledger records four war
        // declarations and ZERO city captures, and the score losses (639-1317,
        // 457 at 3 cities on `civvis-20260808T033223Z`) are the direct result:
        // the games are decided on score at the 250-turn cap, and captures are
        // the one lever never pulled. Three repairs, separately ablatable,
        // one causal story — the agent that reaches its own declaration must
        // also fight the war it declared:
        // ⚠ An adaptive Conquest plan never reaches `advanced_production` —
        // it falls through to the Basic governor and an army target of
        // `mil_per_city * cities` (≈1.4/city on the deployed genome), so the
        // war is fought on a peacetime economy. `docs/RUSH.md` measured the
        // routing trap from the other side: raising the army in
        // `production_value` was twice byte-identical to a no-op.
        self.enable_war_economy();
        // ⚠ Reinforcements never reach the front. Force groups are cliques at
        // `command_radius`, so the trickle of fresh and released units forms
        // one- and two-body groups at home that can never clear
        // `LOCAL_SUPERIORITY_FLOOR` at the objective. Measured on run
        // `civvis-20260808T033223Z`, t217-t225: land forces of one, two and
        // three against the same objective, every one "too weak locally to
        // advance", while the empire fielded 10-14 units.
        self.enable_war_reinforcement();
        // ⚠ A war that grinds a wall for 12 turns reads as stalled, and
        // fatigue then offers peace AND accepts any white peace at +320 —
        // followed by a 30-turn re-declaration lockout. The stall clause is
        // right when the sides are close and self-defeating when the attacker
        // holds `OVERWHELMING_WAR_RATIO` over the defender: the measured live
        // pattern is one declaration per game and no second attempt.
        self.enable_war_patience();
        // ⚠ A counter-leader emergency declaration reached France at t235 with
        // only sixteen turns left, captured nothing, and Zulu won at t251.
        // Timed attacks already reserve their scaled campaign window; the
        // direct denial fallback must not spend a war on less runway.
        self.enable_endgame_war_runway();
        // ⚠ And the war it keeps prosecuting still has to end on a captured
        // city. The campaign re-picks its objective from scratch every turn and
        // prices fifteen turns of siege at ~37 points, less than the distance
        // terms swing; the army walks off a city at 25 hp with its walls down
        // and Civ 6 heals it back at 20 hp a turn. Live run
        // `civvis-20260808T142724Z` dealt 338 hp of city damage over t73-t105,
        // handed 200 of it back, and took nothing — the shape behind 25 live
        // games and 0 captures on 7.7x the field's military.
        self.enable_siege_commitment();
        // ⚠⚠ AND THE POPULATION THE SCIENCE IS COMPUTED FROM IS CAPPED BY HOUSING.
        // The lane above decides which specialty district a city builds; none of
        // them raises the ceiling on the citizens who work them. Measured over
        // 12,969 host-exported city-turns across every one of the 18 live runs
        // carrying `GetHousing()`: median headroom 1 — already inside the
        // half-growth band — **71.2% of city-turns below the break-even 2**,
        // mean growth multiplier 0.515, and at pop >= 8, 87.9% throttled. Over
        // the same runs the Aqueduct family took 8 of 485 district orders and
        // the Neighborhood took none, against 92 Commercial Hubs and 79
        // Campuses; the Aqueduct's median order turn is 164 against a Campus at
        // 131. Science is ~1.16 x population, so this is the same defect shape
        // as #999 and #1003: a repair the governor making most of the builds
        // could not reach.
        self.enable_housing_districts();
        // ⚠ AND THE SAME REPAIR ON THE PRODUCTION PATH. `housing_districts`
        // fixes the two DISTRICTS that raise the ceiling; the buildings that do
        // it — Sewer, Water Mill, Granary — were ranked by price alone, because
        // the baseline governor's building sort has no housing term at all.
        // 44% of our cities end housing-STOPPED against a median food surplus of
        // +6.5 a turn. See the sort in `BasicAi::pick_item`.
        self.enable_housing_buildings();
        // ⚠⚠ AND THE EMPIRE STOPS BUILDING CAMPUSES AT HALF ITS CITIES.
        // `balanced_core` pays a Campus +130 only while `district_count * 2 <
        // city_count`, so the term switches off at half coverage — and measured
        // over 19 live runs, end-of-game Campus coverage is **exactly 50 of 100
        // cities**. The counterweight #958 added is per-city and lane-
        // independent, but it is scaled by a GAME-FRACTION horizon while every
        // term it competes against is a flat constant, so it decays to 120 by
        // turn 150 against a Theatre Square's 850. The cities that never get a
        // Campus are the late-founded ones, and the science funnel cascades
        // from it: 50% Campus, 39% Library, 20% University, 3% Research Lab.
        self.enable_campus_every_city();
        // ⚠⚠ AND THE TWO HOUSING CARDS THE EMPIRE CAN REACH ARE NEVER PLAYED.
        // `medina_quarter` (+2 Housing at 3+ specialty districts) is slotted in
        // **0 of 107 live runs** and appears nowhere in `src/`; `insulae` (+1 at
        // 2+) in **1**. Housing is the dominant growth cap — 71.7% of 13,214
        // host-exported city-turns sit under it at a mean multiplier of 0.510,
        // against the Amenity band's 0.872 — and 60.3% / 40.0% of city-turns
        // already carry the 2 / 3 specialty districts these cards need.
        self.enable_housing_cards();
        // ⚠⚠ THE SCIENCE PROJECT LOOP CAN MAKE THE AMENITY REPAIR UNREACHABLE.
        // On live run `civvis-20260815T051714Z`, every one of five cities sat
        // between -3 and -5 Amenities from t140 onward, costing 10–30% of its
        // yields, while adaptive Science restarted Campus Research Grants 39
        // times from t176 to t233. `BasicAi::pick_item` already ranks an
        // Entertainment Complex above ordinary district lanes, but explicit
        // Science production bypasses that picker whenever it fills a queue.
        // The repair pauses one repeatable project only after at least two
        // cities enter the -3 band; it preserves victory projects, force gaps,
        // and the rest of the Science queue while the district/building chain
        // completes.
        self.enable_amenity_project_preemption();
        self.enable_amenity_district_path();
        self.enable_governor_every_lane();
        // Zero wonder orders in twenty live Settler runs that all ended on the
        // host's score tally, 15 points a wonder. See `live_wonder_race`.
        self.enable_live_wonder_race();
        self.enable_expansion_before_prophet();
        self.enable_no_elective_war();
        // And the last-quarter score-leader alarm asks for a race, not a war.
        // See `counter_in_lane`.
        self.enable_counter_in_lane();
        // And the city target climbs at the Settler game's own era pace. See
        // `era_paced_expansion`.
        self.enable_era_paced_expansion();
        // And a civic is three points on that tally to a tech's two. See
        // `tally_culture`.
        self.enable_tally_culture();
        // And the buildings that chain hangs off. See `culture_building_debt`.
        self.enable_culture_building_debt();
        // And the coverage that price alone never bought. See
        // `culture_coverage`.
        self.enable_culture_coverage();
        // And no colony beyond the empire's Loyalty reach on fogged ground.
        // See `frontier_loyalty`.
        self.enable_frontier_loyalty();
        // And banked Faith buys the Great People the tally pays five for. See
        // `tally_great_people`.
        self.enable_tally_great_people();
        // And the Library and University come before the Great Merchant race.
        // See `buildings_before_projects`.
        self.enable_buildings_before_projects();
        // And a barbarian scout does not pin the opening. See
        // `barbarian_scouts_are_scouts`.
        self.enable_barbarian_scouts_are_scouts();
        // ⚠⚠ AND THE REPAIR IS BEHIND A TECH THE ARGMAX NEVER AIMS AT. Over 94
        // live runs the median empire ends on **30 techs of 77**, `engineering`
        // is reached by only **73%** and at a median turn **116** — which is why
        // the live median Aqueduct order lands at turn 164. Making the district
        // reachable in the build lists cannot beat the tech that gates it.
        self.enable_housing_research();
        self.enable_joint_tactics();
        // ⚠ The live seat plays under an assigned lane (`--victory science`),
        // and `victory_denial` stands down entirely for a targeted seat — so
        // five of the twelve runs the seat was LEADING on 2026-08-16/17 ended
        // at t229-245 on a rival's culture, technology or diplomatic victory
        // with the whole counter apparatus gated off. Match point overrides
        // the lane's focus; ordinary pressure still never does.
        self.enable_deny_while_targeted();
        // ⚠ And the alarm must reach the two lanes it cannot answer late.
        // Four of the five stolen games above were Culture; the general 90
        // bar had not fired when the game ended. See `STOCK_DENIAL_BAR`.
        self.enable_stock_denial_lead_time();
        // These host-facing controls used to be applied by `civvis_orders`
        // after this bundle. They belong here: `live` and every
        // `live_without_*` arm must construct the controller that deployment
        // actually plays.
        self.enable_parallel_settlers();
        self.enable_host_settler_pop();
        self.enable_explore_dead_targets();
        self.enable_explore_commit();
        self.enable_bank_envoys();
    }

    /// Every `enable_live_bridge` repair that fixes a CIVVIS engine defect,
    /// without the deployment-profile treatments that do not apply to native
    /// CIVVIS evaluation.
    ///
    /// ★★★★★ THE WHOLE BUNDLE HAS NEVER BEEN PRICED NATIVELY. The bridge set
    /// grew one measured repair at a time, and each was gated "live-bridge
    /// only" so the frozen `advanced_v1` anchor and the recorded ladders kept
    /// running the controller they were rated with. That is a versioning
    /// decision, not a finding about strength — and the defects themselves are
    /// properties of *this* engine's rules, every one of them measured on
    /// native CIVVIS runs: an army admitted at `command_radius` and judged at
    /// half of it, so it never clears its own muster gate (5/85 turns); a siege
    /// that walks away from a city at 25 hp and is refunded 200 hp of healing;
    /// a relief column that marches at the besieger nearest *itself* rather
    /// than the one killing the city; an army target that never asks how strong
    /// the rival is (94 of 188 war turns already "satisfied").
    ///
    /// `live` has only ever been compared with its own `live_without_*`
    /// ablations, so what the bundle is worth against the production
    /// `advanced` incumbent is simply unmeasured. Ablation cannot answer it
    /// either, because these repairs are *serially coupled*: readiness gates
    /// the march, the march gates the siege, the siege gates the capture, and
    /// the army target decides whether there is anything to march with.
    /// Removing one from a bundle that still contains the other forty prices a
    /// link in a chain that is otherwise whole; it does not price the chain
    /// against no chain at all.
    ///
    /// The core Firaxis adapters are deliberately excluded:
    ///
    /// | excluded | why |
    /// |---|---|
    /// | `live_trader_route_adapter` | adapts a live Trader's zero walking movement to a distinct route-start action; no native game has that action |
    /// | `live_religious_purchase_guard` | enforces Firaxis' city-majority purchase rule, which is not a CIVVIS rule |
    /// | `solvent_faith_army` | prices a faith-bought soldier's GOLD upkeep under Firaxis' economy |
    /// | `joint_tactics` | not a semantics adapter but an evidence exclusion: the whole-game gate is inconclusive, and the deployment-profile run split **every** map at +0 Elo (95% −148..+148) while evaluating 28 branches against the sequential policy's 11 (`docs/AI_GAPS.md` §7) |
    ///
    /// `enable_live_bridge` is therefore this function plus the
    /// deployment-profile treatments tracked in `elo::FIRAXIS_ONLY_TREATMENTS`.
    /// `engine_repairs_match_live_bridge` in `src/elo.rs` fails the build if a
    /// flag is ever added to one and not the other, so the bundles cannot
    /// silently drift apart.
    pub fn enable_engine_repairs(&mut self) {
        self.enable_engine_repairs_war();
        self.enable_engine_repairs_economy();
    }

    /// The military half of [`AdvancedAi::enable_engine_repairs`]: force
    /// assembly, marching, siege, threat reading, and the war/peace decision.
    ///
    /// Split out so the composite's *interaction* is measurable rather than
    /// assumed. If the whole bundle beats `advanced` by more than the war and
    /// economy halves do separately, the repairs compound; if it does not, the
    /// bundle is a sum and should be argued for one term at a time.
    pub fn enable_engine_repairs_war(&mut self) {
        // Force assembly and movement. `muster_at_command_radius` is the
        // keystone: with the shipped radius a real army clears its readiness
        // gate on 6% of turns, so every repair downstream of "the army
        // actually advances" is dead code until it lands.
        self.enable_muster_at_command_radius();
        self.enable_war_reinforcement();
        self.enable_come_ashore();
        self.enable_recorded_tactical_step();
        // Reading the enemy. The `3.0` "we dominate here" sentinel fires on
        // 53.3% of force decisions, and two thirds of those are objectives
        // that are not cities.
        self.enable_blind_objective_strength();
        self.enable_blind_objective_units();
        self.enable_relief_targets_the_siege();
        // Sizing the army against the rival rather than against our own city
        // count, before as well as during the war.
        self.enable_army_target_weighs_the_enemy();
        self.enable_peacetime_deterrence();
        self.enable_war_economy();
        self.enable_bounded_recovery();
        // Taking a city, and finishing the one already broken open.
        self.enable_siege_muster();
        self.enable_siege_role();
        self.enable_siege_tracks_the_wall();
        self.enable_siege_commitment();
        self.enable_war_patience();
        self.enable_endgame_war_runway();
        // Holding one. Barbarians take 7.0 major cities a game, 65% of
        // everything a major loses.
        self.enable_home_defense();
        self.enable_garrison_under_fire();
        self.enable_garrison_walls();
        // Tactical quality on the tile the unit actually stands on.
        self.enable_strike_opening();
        self.enable_ranged_needs_line_of_sight();
        self.enable_recon_replacement();
        // And the recon it rebuilds must stop walking into barbarians. See
        // `recon_flight`.
        self.enable_recon_flight();
        // And a barbarian scout is a scout in both regimes — it can neither
        // attack nor capture, so nothing retreats from one. See
        // `barbarian_scouts_are_scouts`.
        self.enable_barbarian_scouts_are_scouts();
        // And a settler target dropped for danger stays dropped for a while.
        // See `settler_target_hysteresis`.
        self.enable_settler_target_hysteresis();
        // And the last fifty turns are a tally, not a launch window. See
        // `score_horizon`.
        self.enable_score_horizon();
        // And the race that does fit needs one launch pad, not one per city.
        // See `one_launch_pad`.
        self.enable_one_launch_pad();
        // And the sea gets one eye of its own. See `BasicAi::naval_recon`.
        self.enable_naval_recon();
        // And a camp within nine tiles of a city is home ground the guard clears.
        // See `BasicAi::camp_reach`.
        self.enable_camp_reach();
        // And in peacetime the whole field army clears it. See
        // `BasicAi::camp_party`.
        self.enable_camp_party();
        // A Religion plan that keeps its wars blockades its own lane.
        self.enable_religion_sues_peace();
    }

    /// The economic half of [`AdvancedAi::enable_engine_repairs`]: settlement,
    /// growth, districts, and the policy deck.
    pub fn enable_engine_repairs_economy(&mut self) {
        // Getting a settler to a site it can keep.
        self.enable_escort_unstick();
        self.enable_stacked_escort();
        self.enable_settler_stack_discipline();
        self.enable_wonder_ring_settle_value();
        // The cheap half of a research city before the race in it. See
        // `buildings_before_projects`.
        self.enable_buildings_before_projects();
        self.enable_stranded_settler_discount();
        self.enable_wide_map_capacity();
        // Growing what was founded. Housing is gated by a tech the argmax
        // never aims at, so the district, the buildings, the cards and the
        // research order have to move together or none of them binds.
        self.enable_housing_districts();
        self.enable_housing_buildings();
        self.enable_housing_cards();
        self.enable_housing_research();
        self.enable_campus_every_city();
        self.enable_amenity_project_preemption();
        self.enable_amenity_district_path();
        self.enable_governor_every_lane();
        self.enable_district_coverage();
        self.enable_slot_kind_tiebreak();
        // Keeping it loyal, and not slotting cards that multiply zero.
        self.enable_loyalty_policy_defence();
        self.enable_loyalty_rate_alarm();
        self.enable_suzerain_cards_need_a_suzerainty();
    }

    /// Plan each engagement's attacks as one joint problem instead of one
    /// unit at a time in a fixed class order. Measured on `battle_bench`
    /// (1000 paired fresh seeds a cell, seats swapped): combined arms +275,
    /// ranged-heavy +363, siege +206, melee-only within noise, all against
    /// the production controller the bridge extends. The whole-game gate
    /// stays inconclusive (`docs/TACTICS.md` §6), so the tournament
    /// `advanced` entrant keeps the greedy commitment rule and the deployed
    /// bridge — where the operator asked for the strongest battlefield play,
    /// not a rating — takes the search.
    pub fn enable_joint_tactics(&mut self) {
        self.joint_tactics = true;
        self.joint_tactics_forced_off = false;
    }

    /// Hold ONE live-bridge flag off so an arm can price it. These exist for
    /// `live_without_*` in `builtin_ai` and nothing else — the deployment never
    /// turns a repair back off.
    pub fn disable_home_defense(&mut self) {
        self.base.home_defense = false;
    }

    pub fn disable_joint_tactics(&mut self) {
        self.joint_tactics = false;
        self.joint_tactics_forced_off = true;
    }

    pub fn disable_solvent_faith_army(&mut self) {
        self.solvent_faith_army = false;
    }

    pub fn disable_siege_muster(&mut self) {
        self.base.siege_muster = false;
    }

    /// Hold one of the historical production flags off so an evaluator can
    /// price it. The original `promoted_policy_envoy` bundle had thirteen
    /// behaviours and several lacked a `disable_*`; the measured-null cleanup
    /// removed the two confirmed nulls from production, but the explicit
    /// evaluator controls remain available for reproducible decomposition.
    pub fn disable_tactical_strategy(&mut self) {
        self.base.tactical_strategy = false;
    }

    pub fn disable_unit_objective_memory(&mut self) {
        self.base.unit_objective_memory = false;
    }

    /// Let a stalled settler found where it stands. Evaluator arm
    /// `advanced_settler_founds_when_stalled`; off in production.
    pub fn enable_settler_founds_when_stalled(&mut self) {
        self.settler_founds_when_stalled = true;
    }

    /// Withhold the call-local Builder floor. Evaluator-only; production keeps
    /// it until it has a number.
    pub fn disable_production_builder_floor(&mut self) {
        self.production_builder_floor = false;
    }

    /// Withhold the call-local settler-deadline extension. Evaluator-only.
    pub fn disable_production_settler_deadline(&mut self) {
        self.production_settler_deadline = false;
    }

    /// Price Builder production by a survey of the work it would do.
    /// Evaluator arm `advanced_builder_survey`; off in production.
    pub fn enable_builder_reward_survey(&mut self) {
        self.builder_reward_survey = true;
    }

    /// Credit strength-per-production and the civ's own unique unit in the
    /// military production arm. Evaluator arm `advanced_unit_efficiency`;
    /// off in production.
    pub fn enable_unit_cost_efficiency(&mut self) {
        self.unit_cost_efficiency = true;
    }

    /// Fortify units the planner gave nothing to do. Evaluator arm
    /// `advanced_fortify_idle_units`; off in production.
    pub fn enable_fortify_idle_units(&mut self) {
        self.base.fortify_idle_units = true;
    }

    /// ★★★★★ BUILD HULLS ONLY WHERE THEY HAVE OPEN WATER TO SAIL INTO.
    ///
    /// `BasicAi::best_naval_unit` — the path that actually enqueues warships,
    /// not `production_value` — gated on `city_is_coastal`, and a **lake is
    /// water**. A lakeside city therefore built Galleys that spent the whole
    /// game on the lake. Measured over three 150-turn six-player games at the
    /// deployment shape, with the flag off: majors built 53 naval hulls,
    /// **20 of which never moved once**, and only 17.2% of major naval
    /// unit-turns involved any movement.
    ///
    /// With the flag on, on the same three seeds: 26 hulls, **3** that never
    /// moved, Galley movement up from 13.0% to 43.7% of its turns, and the
    /// `audit` major idle-field share down from 21.19% to 17.13%.
    ///
    /// **Promoted to production 2026-08-18** on the matrix at 200 pairs, seed
    /// 8700000. `advanced_without_open_water_navy` prices the withhold.
    pub fn enable_open_water_navy(&mut self) {
        self.base.open_water_navy = true;
    }

    /// Withholding twin for `enable_open_water_navy`, so the promoted rule can
    /// still be priced out of the bundle. See `LIVE_TREATMENTS`.
    pub fn disable_open_water_navy(&mut self) {
        self.base.open_water_navy = false;
    }

    /// Reach for the naval-production discount while hulls are wanted. See
    /// `naval_production_policy`; entrant `advanced_maritime_splice`.
    pub fn enable_naval_production_policy(&mut self) {
        self.naval_production_policy = true;
    }

    /// Sea threats get sea answers. See `BasicAi::sea_answers`; entrant
    /// `advanced_sea_answers`.
    pub fn enable_sea_answers(&mut self) {
        self.base.sea_answers = true;
    }

    /// Deliberate camp clearing as a peacetime errand. See
    /// `BasicAi::camp_bounty`; entrant `advanced_camp_bounty`.
    pub fn enable_camp_bounty(&mut self) {
        self.base.camp_bounty = true;
    }

    /// Let the deck counterfactual see the unit-maintenance bill. See
    /// `BasicAi::maintenance_aware_deck`; entrant `advanced_maintenance_deck`.
    pub fn enable_maintenance_aware_deck(&mut self) {
        self.base.maintenance_aware_deck = true;
    }

    pub fn disable_amenity_districts(&mut self) {
        self.base.amenity_districts = false;
    }

    /// The two base-constructor flags that had no withhold.
    ///
    /// `configured` sets ten booleans true for every `AdvancedAi` that is not
    /// `legacy()`. `deny_leaders` at least had `advanced_blind_to_leaders`;
    /// these two had nothing, so no arm could price them. The
    /// `promoted_policy_envoy` audit found a component costing **41 Elo**
    /// (`city_target_floor`, removed #1504) among flags in exactly this
    /// condition, so an unpriceable default is a real risk rather than a
    /// tidiness complaint.
    ///
    /// ⚠ `AdvancedAi::legacy()` turns both of these off already, so the frozen
    /// anchor is unaffected by anything measured through them.
    pub fn disable_settlement_safety(&mut self) {
        self.settlement_safety = false;
    }

    pub fn disable_battlefront_observation(&mut self) {
        self.battlefront_observation = false;
    }

    /// Put this controller behind the turn-level fog boundary.  Belief
    /// pressure and conservative objective floors are enabled together so a
    /// hidden contact is represented as stale uncertainty rather than as an
    /// empty tile or a live omniscient unit.
    pub fn enable_fog_honest(&mut self) {
        self.fog_honest = true;
        self.battlefront_observation = true;
        self.belief_pressure = true;
        self.blind_objective_strength = true;
        self.blind_objective_units = true;
    }

    pub fn disable_fog_honest(&mut self) {
        self.fog_honest = false;
    }

    /// Rank district families by how much of the empire still lacks them.
    ///
    /// ⚠ `d_theater` is the lowest of the four district weights in all 51 league
    /// genomes and the picker only skips a family THIS CITY holds, so every city
    /// works down the same constant list and the fourth entry is never reached.
    /// Live run `civvis-20260803T090911Z`: 28 `DISTRICT_CAMPUS` orders, **zero**
    /// `DISTRICT_THEATER`, and no Theatre Square anywhere in a 5-city empire — which
    /// makes the whole culture chain unreachable by construction.
    pub fn enable_district_coverage(&mut self) {
        self.base.district_coverage = true;
    }

    pub fn disable_district_coverage(&mut self) {
        self.base.district_coverage = false;
    }

    /// Break a production cost tie by which great-work slots can be filled.
    ///
    /// ⚠ Art and Archaeological Museum are identical in `data/buildings.json` except
    /// for the slot kind and both cost 290, so `sort()` fell through to `Name::cmp`
    /// and the letter 'c' decided it. An Artifact slot needs a 400-production
    /// Archaeologist no live run has ever built; the first run to build a Museum
    /// (`civvis-20260803T082856Z`) took the Artifact one and ended with 0 great works
    /// in 6 slots.
    pub fn enable_slot_kind_tiebreak(&mut self) {
        self.base.slot_kind_tiebreak = true;
    }

    pub fn disable_slot_kind_tiebreak(&mut self) {
        self.base.slot_kind_tiebreak = false;
    }

    /// Let the baseline governor raise the housing ceiling. See
    /// `BasicAi::housing_districts`: 78.4% of live city-turns are growth-
    /// throttled by housing, the median headroom is 0, and the Aqueduct and
    /// Neighborhood together take 1.6% of district orders.
    pub fn enable_housing_districts(&mut self) {
        self.base.housing_districts = true;
    }

    /// Let a housing-short city prefer a building that raises its ceiling.
    pub fn enable_housing_buildings(&mut self) {
        self.base.housing_buildings = true;
    }

    /// Hold the housing-building preference off, for the controlled arm.
    pub fn disable_housing_buildings(&mut self) {
        self.base.housing_buildings = false;
    }

    pub fn disable_housing_districts(&mut self) {
        self.base.housing_districts = false;
    }

    /// Hold the stranded-Settler discount off, for the controlled arm.
    ///
    /// ⚠ Every `enable_*` in `enable_live_bridge` needs this counterpart or the
    /// treatment cannot be ablated: `--without stranded-settler-discount` exits
    /// 2 on an unknown name, so the one arm that would measure the repair
    /// against the deployed configuration does not exist. It shipped without
    /// one; this is that omission.
    pub fn disable_stranded_settler_discount(&mut self) {
        self.base.settler_strand_discount = false;
        self.settler_founds_when_stalled = false;
    }

    /// Keep asking for a Campus in every city that can still repay one. See
    /// `AdvancedAi::campus_every_city`: live end-of-game Campus coverage is
    /// exactly 50 of 100 cities, which is what `balanced_core`'s half-empire
    /// cliff asks for.
    pub fn enable_campus_every_city(&mut self) {
        self.campus_every_city = true;
    }

    pub fn disable_campus_every_city(&mut self) {
        self.campus_every_city = false;
    }

    /// When host-observed Amenity deficits have crossed a severe empire-wide
    /// threshold, pause one repeatable project for the concrete repair chain
    /// and let the policy deck use its direct empire-wide repair. Frozen
    /// controllers leave both ordinary orderings untouched.
    pub fn enable_amenity_project_preemption(&mut self) {
        self.amenity_project_preemption = true;
    }

    pub fn disable_amenity_project_preemption(&mut self) {
        self.amenity_project_preemption = false;
    }

    /// Price an amenity district by the building it will host and a regional
    /// amenity building by every city it reaches. See `amenity_district_path`.
    pub fn enable_amenity_district_path(&mut self) {
        self.amenity_district_path = true;
    }

    pub fn disable_amenity_district_path(&mut self) {
        self.amenity_district_path = false;
    }

    /// Run the strategic governor under every lane. See `governor_every_lane`.
    pub fn enable_governor_every_lane(&mut self) {
        self.governor_every_lane = true;
    }

    pub fn disable_governor_every_lane(&mut self) {
        self.governor_every_lane = false;
    }

    /// Let a developed live city take the cheapest legal wonder whatever the
    /// grand strategy says. See `live_wonder_race`: the live seat had never
    /// ordered one, and its games end on the host's score tally.
    pub fn enable_live_wonder_race(&mut self) {
        self.live_wonder_race = true;
    }

    pub fn disable_live_wonder_race(&mut self) {
        self.live_wonder_race = false;
    }

    /// Let the third city come before the Prophet. See `expansion_before_prophet`.
    pub fn enable_expansion_before_prophet(&mut self) {
        self.expansion_before_prophet = true;
    }

    pub fn disable_expansion_before_prophet(&mut self) {
        self.expansion_before_prophet = false;
    }

    /// Do not open an elective war on the live seat. See `no_elective_war`.
    pub fn enable_no_elective_war(&mut self) {
        self.no_elective_war = true;
    }

    pub fn disable_no_elective_war(&mut self) {
        self.no_elective_war = false;
    }

    /// Answer a Science or score leader by racing in that lane, not by
    /// declaring on them. See `counter_in_lane`.
    pub fn enable_counter_in_lane(&mut self) {
        self.counter_in_lane = true;
    }

    pub fn disable_counter_in_lane(&mut self) {
        self.counter_in_lane = false;
    }

    /// Raise the city target one rung per era of the Settler game. See
    /// `era_paced_expansion`.
    pub fn enable_era_paced_expansion(&mut self) {
        self.era_paced_expansion = true;
    }

    pub fn disable_era_paced_expansion(&mut self) {
        self.era_paced_expansion = false;
    }

    /// Price a point of culture at the lane's price of a point of science.
    /// See `tally_culture`.
    pub fn enable_tally_culture(&mut self) {
        self.tally_culture = true;
    }

    pub fn disable_tally_culture(&mut self) {
        self.tally_culture = false;
    }

    /// Make the Theater Square owe its buildings. See `culture_building_debt`.
    pub fn enable_culture_building_debt(&mut self) {
        self.culture_building_debt = true;
    }

    pub fn disable_culture_building_debt(&mut self) {
        self.culture_building_debt = false;
    }

    /// Treat a Theater Square, rather than any Great Work slot, as the
    /// non-Culture lane's veto boundary. Evaluator-only until the targeted
    /// deployment comparison prices this distinction.
    pub fn enable_great_work_veto_by_district(&mut self) {
        self.great_work_veto_by_district = true;
    }

    /// Pay for the Theater Square the empire has not got. See
    /// `culture_coverage`.
    pub fn enable_culture_coverage(&mut self) {
        self.culture_coverage = true;
    }

    pub fn disable_culture_coverage(&mut self) {
        self.culture_coverage = false;
    }

    /// Bank an envoy the plan has no positive use for. See `bank_envoys`.
    pub fn enable_bank_envoys(&mut self) {
        self.bank_envoys = true;
        self.base.enable_bank_envoys();
    }

    pub fn disable_bank_envoys(&mut self) {
        self.bank_envoys = false;
        self.base.disable_bank_envoys();
    }

    /// Do not found a colony beyond the empire's Loyalty reach on fogged
    /// ground. See `frontier_loyalty`.
    pub fn enable_frontier_loyalty(&mut self) {
        self.frontier_loyalty = true;
    }

    pub fn disable_frontier_loyalty(&mut self) {
        self.frontier_loyalty = false;
    }

    /// Keep a settler target dropped for danger out of the next picks for a
    /// few turns. See `settler_target_hysteresis`.
    pub fn enable_settler_target_hysteresis(&mut self) {
        self.settler_target_hysteresis = true;
    }

    pub fn disable_settler_target_hysteresis(&mut self) {
        self.settler_target_hysteresis = false;
    }

    /// Let banked Faith or gold patronize any Great Person it can pay for on
    /// the tally seat. See `tally_great_people`.
    pub fn enable_tally_great_people(&mut self) {
        self.tally_great_people = true;
    }

    pub fn disable_tally_great_people(&mut self) {
        self.tally_great_people = false;
    }

    /// A district project waits behind the science and production buildings
    /// the city can already build. See `buildings_before_projects`.
    pub fn enable_buildings_before_projects(&mut self) {
        self.buildings_before_projects = true;
    }

    pub fn disable_buildings_before_projects(&mut self) {
        self.buildings_before_projects = false;
    }

    /// Stop pricing a Firaxis barbarian scout as a threat. See
    /// `barbarian_scouts_are_scouts`.
    pub fn enable_barbarian_scouts_are_scouts(&mut self) {
        self.barbarian_scouts_are_scouts = true;
    }

    pub fn disable_barbarian_scouts_are_scouts(&mut self) {
        self.barbarian_scouts_are_scouts = false;
    }

    /// Let a recon unit step out of a visible hostile's reach before it
    /// explores. See `recon_flight`.
    pub fn enable_recon_flight(&mut self) {
        self.recon_flight = true;
    }

    pub fn disable_recon_flight(&mut self) {
        self.recon_flight = false;
    }

    /// Skip a space race or a bomb that cannot finish before the turn limit.
    /// See `score_horizon`.
    pub fn enable_score_horizon(&mut self) {
        self.score_horizon = true;
    }

    pub fn disable_score_horizon(&mut self) {
        self.score_horizon = false;
    }

    /// Give the 3,000-point first-pad rung to one city at a time. See
    /// `one_launch_pad`.
    pub fn enable_one_launch_pad(&mut self) {
        self.one_launch_pad = true;
    }

    pub fn disable_one_launch_pad(&mut self) {
        self.one_launch_pad = false;
    }

    /// Put `medina_quarter` and `insulae` in the deck when a city is short of
    /// housing and already carries the districts they key off.
    pub fn enable_housing_cards(&mut self) {
        self.housing_cards = true;
    }

    pub fn disable_housing_cards(&mut self) {
        self.housing_cards = false;
    }

    /// Aim research at the housing ceiling when the empire is paying it.
    pub fn enable_housing_research(&mut self) {
        self.housing_research = true;
    }

    pub fn disable_housing_research(&mut self) {
        self.housing_research = false;
    }

    /// Require a faith-bought soldier's gold upkeep to be payable. Native
    /// tournament games leave this disabled so their ladders stay comparable.
    /// Rank loyalty emergencies by turns-to-flip instead of by level. Native
    /// tournament games leave this disabled.
    pub fn enable_loyalty_rate_alarm(&mut self) {
        self.base.loyalty_rate_alarm = true;
    }

    /// Hold one more live-bridge flag off so it can be priced. Every flag in
    /// `enable_live_bridge` needs one of these or it ships unmeasured — which is
    /// how five repairs reached deployment without a single outcome number.
    pub fn disable_bounded_recovery(&mut self) {
        self.bounded_recovery = false;
    }

    pub fn disable_army_target_weighs_the_enemy(&mut self) {
        self.army_target_weighs_the_enemy = false;
    }

    pub fn disable_peacetime_deterrence(&mut self) {
        self.peacetime_deterrence = false;
    }

    pub fn disable_suzerain_cards_need_a_suzerainty(&mut self) {
        self.suzerain_cards_need_a_suzerainty = false;
    }

    pub fn disable_siege_tracks_the_wall(&mut self) {
        self.siege_tracks_the_wall = false;
    }

    pub fn disable_blind_objective_strength(&mut self) {
        self.blind_objective_strength = false;
    }

    pub fn disable_muster_at_command_radius(&mut self) {
        self.muster_at_command_radius = false;
    }

    pub fn disable_war_economy(&mut self) {
        self.war_economy = false;
    }

    pub fn disable_war_reinforcement(&mut self) {
        self.war_reinforcement = false;
    }

    pub fn disable_war_patience(&mut self) {
        self.war_patience = false;
    }

    pub fn disable_endgame_war_runway(&mut self) {
        self.endgame_war_runway = false;
    }

    pub fn disable_siege_commitment(&mut self) {
        self.siege_commitment = false;
    }

    pub fn disable_relief_targets_the_siege(&mut self) {
        self.relief_targets_the_siege = false;
    }

    pub fn disable_blind_objective_units(&mut self) {
        self.blind_objective_units = false;
    }

    pub fn disable_loyalty_rate_alarm(&mut self) {
        self.base.loyalty_rate_alarm = false;
    }

    pub fn enable_solvent_faith_army(&mut self) {
        self.solvent_faith_army = true;
    }

    // `pub(super)` rather than private, the one line of this file that is not
    // code motion: this is the only toggle `advanced.rs` itself calls, and a
    // child module's private item is not visible to its parent. Widened by
    // exactly one module, not to the crate.
    /// The Battlefield map is a bounded combat controller, not a Civ-world
    /// deployment profile. Route the already-measured portfolio search there
    /// for promoted controllers, while preserving the frozen anchor and any
    /// explicit evaluator withholding. The flag is set on the controller so
    /// telemetry and `Ai::joint_tactics_census` describe what actually ran.
    pub(super) fn enable_arena_joint_tactics(&mut self, g: &Game) {
        if self.victory_planning && g.is_arena() && !self.joint_tactics_forced_off {
            self.joint_tactics = true;
        }
    }
}

#[cfg(test)]
mod guard {
    /// The name of a method defined at `impl` depth on `line`, if there is one.
    ///
    /// Deliberately textual: the guards below are about where a function is
    /// written, which no reflection over the compiled crate can see.
    fn method_name(line: &str) -> Option<&str> {
        let body = line.strip_prefix("    ")?;
        if body.starts_with(' ') {
            return None; // deeper than `impl` depth: a nested item or a body line
        }
        let body = match body.strip_prefix("pub") {
            Some(rest) => match rest.strip_prefix('(') {
                Some(scoped) => scoped.split_once(") ")?.1,
                None => rest.strip_prefix(' ')?,
            },
            None => body,
        };
        let body = body.strip_prefix("const ").unwrap_or(body);
        let rest = body.strip_prefix("fn ")?;
        Some(&rest[..rest.find(|c: char| !c.is_alphanumeric() && c != '_')?])
    }

    fn is_toggle(name: &str) -> bool {
        name.starts_with("enable_") || name.starts_with("disable_")
    }

    /// This file up to where this module starts.
    ///
    /// The guards below are themselves functions at `impl` depth in this
    /// file, so a scan of the whole text reports them as strays.
    fn toggles_only() -> &'static str {
        let source = include_str!("treatment_flags.rs");
        let marker = "\n#[cfg(test)]\nmod guard {";
        &source[..source.find(marker).expect("the guard module moved or was renamed")]
    }

    /// The toggles stay out of the hotspot.
    ///
    /// Without this, the next treatment PR adds its pair back at the anchor
    /// this change exists to empty, and the move is undone by the following
    /// morning. `AGENTS.md` states the rule this implements: a sentence that
    /// states a fact is enforced by a test, or it is a guess with good
    /// posture.
    #[test]
    fn advanced_rs_defines_no_capability_toggles() {
        let offenders: Vec<&str> = include_str!("../advanced.rs")
            .lines()
            .filter(|line| method_name(line).is_some_and(is_toggle))
            .collect();
        assert!(
            offenders.is_empty(),
            "capability toggles belong in src/ai/advanced/treatment_flags.rs, not in \
             src/ai/advanced.rs — the file tools/conflict_hotspots.py measures as the most \
             contended in the repository. Move these there:\n{}",
            offenders.join("\n")
        );
    }

    /// And nothing else moves in here.
    ///
    /// A file that holds only short setters is safe to conflict in. That
    /// property is the whole point of the move, and it is lost the first time
    /// something with real logic is parked here because the file was quiet.
    #[test]
    fn treatment_flags_holds_only_toggles() {
        let strays: Vec<&str> = toggles_only()
            .lines()
            .filter(|line| method_name(line).is_some_and(|name| !is_toggle(name)))
            .collect();
        assert!(
            strays.is_empty(),
            "src/ai/advanced/treatment_flags.rs holds capability toggles and nothing else; \
             these belong in src/ai/advanced.rs:\n{}",
            strays.join("\n")
        );
    }

    /// The guards can see a toggle at all.
    ///
    /// Both assertions above pass trivially if `method_name` stops matching —
    /// a rustfmt change to how a signature wraps, a visibility spelling it
    /// does not know. This one fails instead of going quiet.
    #[test]
    fn the_guards_can_see_the_toggles_they_guard() {
        let found = toggles_only()
            .lines()
            .filter_map(method_name)
            .filter(|name| is_toggle(name))
            .count();
        assert!(
            found > 150,
            "method_name() found only {found} toggles in a file that holds ~180 of them; \
             the guards in this module are no longer reading the source they claim to check"
        );
    }
}
