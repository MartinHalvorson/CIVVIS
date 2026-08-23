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
    /// Size the defensive Missionary corps by the number of cities actually
    /// under conversion pressure instead of the shipped constant 2. Off in
    /// production; opted into by name. See
    /// [`AdvancedAi::religious_defence_scales`].
    pub fn enable_religious_defence_scales(&mut self) {
        self.religious_defence_scales = true;
    }

    /// The twin of `enable_religious_defence_scales`.
    pub fn disable_religious_defence_scales(&mut self) {
        self.religious_defence_scales = false;
    }

    /// Let a founder that is defending its own cities hold one Guru, the only
    /// field heal a religious corps has. Off in production; opted into by
    /// name. See [`AdvancedAi::guru_heals_the_corps`].
    pub fn enable_guru_heals_the_corps(&mut self) {
        self.guru_heals_the_corps = true;
    }

    /// The twin of `enable_guru_heals_the_corps`.
    pub fn disable_guru_heals_the_corps(&mut self) {
        self.guru_heals_the_corps = false;
    }

    /// Let a wounded spreader standing in its own Holy Site's heal ring hold
    /// instead of spending a charge at a fraction of its strength. Off in
    /// production; opted into by name. See
    /// [`AdvancedAi::religious_units_heal_first`].
    pub fn enable_religious_units_heal_first(&mut self) {
        self.religious_units_heal_first = true;
    }

    /// The twin of `enable_religious_units_heal_first`.
    pub fn disable_religious_units_heal_first(&mut self) {
        self.religious_units_heal_first = false;
    }

    /// Condemn a heretic the World Congress has condemned, not only one this
    /// seat is at war with. Off in production; opted into by name. See
    /// [`AdvancedAi::condemn_under_congress`].
    pub fn enable_condemn_under_congress(&mut self) {
        self.condemn_under_congress = true;
    }

    /// The twin of `enable_condemn_under_congress`.
    pub fn disable_condemn_under_congress(&mut self) {
        self.condemn_under_congress = false;
    }

    /// Keep a spread campaign that has already converted a foreign city on the
    /// offensive between waves, instead of dropping the posture the turn its
    /// last charge is spent. Off in production; opted into by name. See
    /// [`AdvancedAi::spread_campaign_persists`].
    pub fn enable_spread_campaign_persists(&mut self) {
        self.spread_campaign_persists = true;
    }

    /// The twin of `enable_spread_campaign_persists`.
    pub fn disable_spread_campaign_persists(&mut self) {
        self.spread_campaign_persists = false;
    }

    /// Put a Holy Site in the city that is actually losing its majority, so
    /// its defender can be bought there instead of walking from the Holy City.
    /// Off in production; opted into by name. See
    /// [`AdvancedAi::holy_site_where_the_threat_is`].
    pub fn enable_holy_site_where_the_threat_is(&mut self) {
        self.holy_site_where_the_threat_is = true;
    }

    /// The twin of `enable_holy_site_where_the_threat_is`.
    pub fn disable_holy_site_where_the_threat_is(&mut self) {
        self.holy_site_where_the_threat_is = false;
    }

    /// Evangelize the beliefs that multiply a religious corps while the corps
    /// has a job, instead of the victory lane's worship building. Off in
    /// production; opted into by name. See
    /// [`AdvancedAi::enhancer_for_the_corps`].
    pub fn enable_enhancer_for_the_corps(&mut self) {
        self.enhancer_for_the_corps = true;
    }

    /// The twin of `enable_enhancer_for_the_corps`.
    pub fn disable_enhancer_for_the_corps(&mut self) {
        self.enhancer_for_the_corps = false;
    }

    /// Promote an Apostle for the job the empire has rather than for the
    /// largest number on the card. Off in production; opted into by name
    /// (`victory_eval --with apostle-promotion-by-role`, `gene_screen`). See
    /// [`crate::ai::BasicAi::apostle_promotion_by_role`] for the units mismatch
    /// that makes the shipped ranking a constant.
    pub fn enable_apostle_promotion_by_role(&mut self) {
        self.base.apostle_promotion_by_role = true;
    }

    /// The twin of `enable_apostle_promotion_by_role`, so an arm that opted in
    /// can put it back.
    pub fn disable_apostle_promotion_by_role(&mut self) {
        self.base.apostle_promotion_by_role = false;
    }

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
    /// A founder under conversion pressure may hold one Apostle for the
    /// Inquisition, bought after its Missionaries when the bank covers it.
    /// See [`AdvancedAi::inquisition_on_threat`]. Off everywhere by default;
    /// opted into by name (`gene_screen`, `victory_eval --with
    /// inquisition-on-threat`) and listed in `PRODUCTION_OPT_INS`.
    pub fn enable_inquisition_on_threat(&mut self) {
        self.inquisition_on_threat = true;
    }

    /// The twin of `enable_inquisition_on_threat`.
    pub fn disable_inquisition_on_threat(&mut self) {
        self.inquisition_on_threat = false;
    }

    /// A founder outside the Religion lane still builds its Shrine and
    /// Temple. See [`AdvancedAi::founder_temple`]. Opt-in gene.
    pub fn enable_founder_temple(&mut self) {
        self.founder_temple = true;
    }

    /// The twin of `enable_founder_temple`.
    pub fn disable_founder_temple(&mut self) {
        self.founder_temple = false;
    }

    /// A founder researches Theology next. See
    /// [`AdvancedAi::theology_for_founders`]. Opt-in gene.
    pub fn enable_theology_for_founders(&mut self) {
        self.theology_for_founders = true;
    }

    /// The twin of `enable_theology_for_founders`.
    pub fn disable_theology_for_founders(&mut self) {
        self.theology_for_founders = false;
    }

    /// A unit already inside a hostile's next-turn reach picks a posture:
    /// stand and heal where the melee exchange favours holding, close on a
    /// shooter it cannot answer, or step out of that shooter's envelope. See
    /// [`AdvancedAi::contact_posture`]. Opt-in gene `contact-posture`.
    pub fn enable_contact_posture(&mut self) {
        self.contact_posture = true;
    }

    /// The twin of `enable_contact_posture`.
    pub fn disable_contact_posture(&mut self) {
        self.contact_posture = false;
    }

    /// A settler scores a site by the districts the plan would build there,
    /// each on its own plot. See [`AdvancedAi::district_lookahead_settle`].
    /// Opt-in gene `district-lookahead-settle`.
    pub fn enable_district_lookahead_settle(&mut self) {
        self.district_lookahead_settle = true;
    }

    /// The twin of `enable_district_lookahead_settle`.
    pub fn disable_district_lookahead_settle(&mut self) {
        self.district_lookahead_settle = false;
    }

    /// A border plot is bought only when its priced benefit clears its Gold
    /// by a margin. See [`AdvancedAi::priced_tile_purchase`]. Opt-in gene
    /// `priced-tile-purchase`.
    pub fn enable_priced_tile_purchase(&mut self) {
        self.priced_tile_purchase = true;
        self.base.plot_purchase_delegated = true;
    }

    /// The twin of `enable_priced_tile_purchase`.
    pub fn disable_priced_tile_purchase(&mut self) {
        self.priced_tile_purchase = false;
        self.base.plot_purchase_delegated = false;
    }

    /// Price the science economy on whether it can still repay rather than on
    /// how much of the game is left. See
    /// [`AdvancedAi::science_payback_horizon`]. Opt-in gene
    /// `science-payback-horizon`.
    pub fn enable_science_payback_horizon(&mut self) {
        self.science_payback_horizon = true;
    }

    /// The twin of `enable_science_payback_horizon`.
    pub fn disable_science_payback_horizon(&mut self) {
        self.science_payback_horizon = false;
    }

    /// Credit a Campus building the beakers its city's multipliers will
    /// actually pay it. See [`AdvancedAi::science_multiplier_payoff`]. Opt-in
    /// gene `science-multiplier-payoff`.
    pub fn enable_science_multiplier_payoff(&mut self) {
        self.science_multiplier_payoff = true;
    }

    /// The twin of `enable_science_multiplier_payoff`.
    pub fn disable_science_multiplier_payoff(&mut self) {
        self.science_multiplier_payoff = false;
    }

    /// A Campus building's debt is scaled by its own Science against the
    /// chain's first rung. See [`AdvancedAi::research_tier_premium`]. Opt-in
    /// gene `research-tier-premium`.
    pub fn enable_research_tier_premium(&mut self) {
        self.research_tier_premium = true;
    }

    /// The twin of `enable_research_tier_premium`.
    pub fn disable_research_tier_premium(&mut self) {
        self.research_tier_premium = false;
    }

    /// The Campus coverage term is scaled by how finished the empire's
    /// standing Campuses are. See [`AdvancedAi::campus_finishes_first`].
    /// Opt-in gene `campus-finishes-first`.
    pub fn enable_campus_finishes_first(&mut self) {
        self.campus_finishes_first = true;
    }

    /// The twin of `enable_campus_finishes_first`.
    pub fn disable_campus_finishes_first(&mut self) {
        self.campus_finishes_first = false;
    }

    /// A power plant is credited the yields it switches on in its city. See
    /// [`AdvancedAi::power_the_laboratory`]. Opt-in gene
    /// `power-the-laboratory`.
    pub fn enable_power_the_laboratory(&mut self) {
        self.power_the_laboratory = true;
    }

    /// The twin of `enable_power_the_laboratory`.
    pub fn disable_power_the_laboratory(&mut self) {
        self.power_the_laboratory = false;
    }

    /// A Campus plot that clears the multiplier's adjacency threshold is
    /// credited what crossing it unlocks. See
    /// [`AdvancedAi::campus_adjacency_threshold`]. Opt-in gene
    /// `campus-adjacency-threshold`.
    pub fn enable_campus_adjacency_threshold(&mut self) {
        self.campus_adjacency_threshold = true;
    }

    /// The twin of `enable_campus_adjacency_threshold`.
    pub fn disable_campus_adjacency_threshold(&mut self) {
        self.campus_adjacency_threshold = false;
    }

    /// A Campus city within reach of the Population gate credits growth with
    /// what crossing it unlocks. See [`AdvancedAi::fifteenth_citizen`].
    /// Opt-in gene `fifteenth-citizen`.
    pub fn enable_fifteenth_citizen(&mut self) {
        self.fifteenth_citizen = true;
    }

    /// The twin of `enable_fifteenth_citizen`.
    pub fn disable_fifteenth_citizen(&mut self) {
        self.fifteenth_citizen = false;
    }

    /// The research goal aims at a Campus rung the empire can BUILD, not only
    /// one it has already built. See [`AdvancedAi::chain_tech_lookahead`].
    /// Opt-in gene `chain-tech-lookahead`.
    pub fn enable_chain_tech_lookahead(&mut self) {
        self.chain_tech_lookahead = true;
    }

    /// The twin of `enable_chain_tech_lookahead`.
    pub fn disable_chain_tech_lookahead(&mut self) {
        self.chain_tech_lookahead = false;
    }

    /// A finished research city pays more for its own district's project. See
    /// [`AdvancedAi::research_grants_first`]. Opt-in gene
    /// `research-grants-first`.
    pub fn enable_research_grants_first(&mut self) {
        self.research_grants_first = true;
    }

    /// The twin of `enable_research_grants_first`.
    pub fn disable_research_grants_first(&mut self) {
        self.research_grants_first = false;
    }

    /// The citizen tilt and the beaker floor hold while the research can
    /// still pay. See [`AdvancedAi::research_floor_holds`]. Opt-in gene
    /// `research-floor-holds`.
    pub fn enable_research_floor_holds(&mut self) {
        self.research_floor_holds = true;
    }

    /// The twin of `enable_research_floor_holds`.
    pub fn disable_research_floor_holds(&mut self) {
        self.research_floor_holds = false;
    }

    /// A seat with no religion and 600+ Faith patronizes Great People with it
    /// whatever the shortfall. See [`AdvancedAi::idle_faith_patronage`].
    /// Opt-in gene.
    pub fn enable_idle_faith_patronage(&mut self) {
        self.idle_faith_patronage = true;
    }

    /// The twin of `enable_idle_faith_patronage`.
    pub fn disable_idle_faith_patronage(&mut self) {
        self.idle_faith_patronage = false;
    }

    /// Buy the second and third Scout while the world's borders are still
    /// open — after Early Empire a city-state cannot be met by land at all.
    /// See [`AdvancedAi::early_contact_window`].
    pub fn enable_early_contact_window(&mut self) {
        self.early_contact_window = true;
    }

    /// The twin of `enable_early_contact_window`.
    pub fn disable_early_contact_window(&mut self) {
        self.early_contact_window = false;
    }

    /// A class earned and blocked reserves a city for the slot building,
    /// district, wonder or soldier that lifts the block, and a due cultural
    /// person sells duplicate works to make room. See
    /// [`AdvancedAi::great_person_housing`]. Opt-in gene.
    pub fn enable_great_person_housing(&mut self) {
        self.great_person_housing = true;
    }

    /// The twin of `enable_great_person_housing`.
    pub fn disable_great_person_housing(&mut self) {
        self.great_person_housing = false;
    }

    /// Open a surprise war on a neighbour whose unescorted Settlers, Builders
    /// or unpillaged tiles lie within a short march of our soldiers, take
    /// them, and sue for peace. See [`AdvancedAi::opportunistic_war`].
    /// Opt-in gene.
    pub fn enable_opportunistic_war(&mut self) {
        self.opportunistic_war = true;
    }

    /// The twin of `enable_opportunistic_war`.
    pub fn disable_opportunistic_war(&mut self) {
        self.opportunistic_war = false;
        self.raid_war = None;
    }

    /// Count a neighbour's unpillaged tiles within reach as raid prizes and
    /// send raiding soldiers to them. See [`AdvancedAi::raid_pillage_prizes`].
    /// Opt-in gene; inert unless `opportunistic_war` is on.
    pub fn enable_raid_pillage_prizes(&mut self) {
        self.raid_pillage_prizes = true;
    }

    /// The twin of `enable_raid_pillage_prizes`.
    pub fn disable_raid_pillage_prizes(&mut self) {
        self.raid_pillage_prizes = false;
    }

    /// The Religion lane pays for its Holy Site what the Culture lane pays
    /// for its Theater Square. See [`AdvancedAi::holy_lane_parity`]; the
    /// evaluator arm `advanced_holy_lane` sets the field directly, this pair
    /// makes it a native opt-in gene (`PRODUCTION_OPT_INS`).
    pub fn enable_holy_lane_parity(&mut self) {
        self.holy_lane_parity = true;
    }

    /// The twin of `enable_holy_lane_parity`.
    pub fn disable_holy_lane_parity(&mut self) {
        self.holy_lane_parity = false;
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

    /// A Civ6 replan frame is another view of the current host turn, not a
    /// second turn of a unit's livelock history. Keep that bridge-only
    /// accounting behind a separately withholdable treatment.
    pub fn enable_live_motion_turn_accounting(&mut self) {
        self.base.enable_live_motion_turn_accounting();
    }

    /// Withholding twin for `enable_live_motion_turn_accounting`.
    pub fn disable_live_motion_turn_accounting(&mut self) {
        self.base.disable_live_motion_turn_accounting();
    }

    /// Withholding twin for the base military picker's engine-legality
    /// candidate screen (`BasicAi::legal_tactical_candidates`), which
    /// production enables in `promoted_policy_envoy`. Only the
    /// `advanced_without_legal_candidates` arm calls this, so the axis stays
    /// measurable after shipping.
    pub fn disable_legal_tactical_candidates(&mut self) {
        self.base.legal_tactical_candidates = false;
    }

    /// Refuse a step onto any tile this unit has already stood on this turn.
    ///
    /// `enable_recorded_tactical_step` remembers one tile, which refuses
    /// `A -> B -> A` and lets `A -> B -> C -> A` through. See
    /// `BasicAi::whole_turn_backtrack_guard` for what that costs.
    pub fn enable_whole_turn_backtrack_guard(&mut self) {
        self.base.whole_turn_backtrack_guard = true;
    }

    /// Withholding twin for `enable_whole_turn_backtrack_guard`. See
    /// `LIVE_TREATMENTS`.
    pub fn disable_whole_turn_backtrack_guard(&mut self) {
        self.base.whole_turn_backtrack_guard = false;
    }

    /// A blind-planned unit stops at the first step that revealed new ground
    /// and finishes its movement sighted; on the bridge its walk is cut at
    /// the first unrevealed hex so the replan frame sees what it uncovered.
    /// See [`AdvancedAi::step_and_reassess`].
    pub fn enable_step_and_reassess(&mut self) {
        self.step_and_reassess = true;
    }

    /// Withholding twin for `enable_step_and_reassess`. See `LIVE_TREATMENTS`.
    pub fn disable_step_and_reassess(&mut self) {
        self.step_and_reassess = false;
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
    /// Release an escort that is not walking its settler. See `escort_unstick`.
    pub fn enable_escort_unstick(&mut self) {
        self.escort_unstick = true;
    }

    pub fn disable_escort_unstick(&mut self) {
        self.escort_unstick = false;
    }
    /// Keep live settlers out of Civilization VI's formation channel while
    /// leaving the native `stacked_escort` gene independently screenable.
    ///
    /// The live bridge recorded formations failing to advance; a guard that
    /// shadows with normal movement is the already-tested host-safe path.
    /// This is therefore a live fidelity gate, not a re-promotion of the
    /// native gene that the ledger correctly holds off.
    pub fn enable_live_formationless_settler_shadow(&mut self) {
        self.live_formationless_settler_shadow = true;
    }

    /// The twin of [`Self::enable_live_formationless_settler_shadow`].
    pub fn disable_live_formationless_settler_shadow(&mut self) {
        self.live_formationless_settler_shadow = false;
        self.settler_guards.clear();
        self.guard_wait.clear();
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
    pub fn enable_promote_when_wounded(&mut self) {
        self.promote_when_wounded = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_promote_when_wounded(&mut self) {
        self.promote_when_wounded = false;
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

    /// The off toggle, so the registry row has both directions.
    pub fn disable_engine_faith_price(&mut self) {
        self.engine_faith_price = false;
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

    /// The off toggle, so the registry row has both directions.
    pub fn disable_price_the_suzerainty(&mut self) {
        self.price_the_suzerainty = false;
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

    /// Price a revealed natural wonder's ring into the settle scorer. Native
    /// tournament games leave this disabled so their recorded ladders stay
    /// comparable.
    pub fn enable_wonder_ring_settle_value(&mut self) {
        self.base.wonder_ring_settle_value = true;
    }

    pub fn disable_wonder_ring_settle_value(&mut self) {
        self.base.wonder_ring_settle_value = false;
    }

    /// Keep the land army out of the water. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_come_ashore(&mut self) {
        self.base.come_ashore = true;
    }

    pub fn disable_come_ashore(&mut self) {
        self.base.come_ashore = false;
    }

    /// Size the siege train by the wall it has to breach.
    pub fn enable_siege_tracks_the_wall(&mut self) {
        self.siege_tracks_the_wall = true;
    }

    /// Stop a fogged objective city from reading as an empty tile when the
    /// army decides whether it is strong enough to engage. Native tournament
    /// games leave this disabled so their recorded ladders stay comparable.
    pub fn enable_blind_objective_strength(&mut self) {
        self.blind_objective_strength = true;
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

    /// See [`Self::spy_mission_patience`].
    pub fn enable_spy_mission_patience(&mut self) {
        self.spy_mission_patience = true;
    }

    pub fn disable_spy_mission_patience(&mut self) {
        self.spy_mission_patience = false;
    }

    /// See [`Self::settler_site_agreement`].
    pub fn enable_settler_site_agreement(&mut self) {
        self.settler_site_agreement = true;
    }

    pub fn disable_settler_site_agreement(&mut self) {
        self.settler_site_agreement = false;
    }

    /// See [`Self::settler_guard_holds`].
    pub fn enable_settler_guard_holds(&mut self) {
        self.settler_guard_holds = true;
    }

    pub fn disable_settler_guard_holds(&mut self) {
        self.settler_guard_holds = false;
    }

    /// See [`Self::siege_is_progress`].
    pub fn enable_siege_is_progress(&mut self) {
        self.siege_is_progress = true;
    }

    pub fn disable_siege_is_progress(&mut self) {
        self.siege_is_progress = false;
    }

    /// See [`Self::projected_stock_denial`].
    pub fn enable_projected_stock_denial(&mut self) {
        self.projected_stock_denial = true;
    }

    pub fn disable_projected_stock_denial(&mut self) {
        self.projected_stock_denial = false;
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
    /// ⚠ ADD NEW BRIDGE FLAGS TO `enable_live_bridge_universe`, not in the
    /// binary, or the arm silently stops matching the deployment — the exact
    /// shape of `civvis-the-runner-tree-was-the-broken-link`.
    ///
    /// ★★★★ AND WHAT SHIPS IS THE UNIVERSE MINUS WHAT THE LEDGER HOLDS OFF.
    /// Operator directive 2026-08-20: the defaults reflect the best genome.
    /// Since the directive of 2026-08-22 the ledger reads that off the
    /// ranking's two win columns — a gene is on when both its last and prior
    /// native screens are positive, or when their average clears +15 with
    /// neither below −10. `apply_gene_ledger` (`advanced/gene_ledger.rs`)
    /// ends this helper: a treatment the ledger does not default on is
    /// withheld, an opt-in it defaults on is enabled, a flag no native screen
    /// can price (Firaxis-only) stays as the universe set it. A new treatment
    /// therefore ships OFF until two screens agree; `gene_screen --list` shows
    /// each gene's verdict and default.
    pub fn enable_live_bridge(&mut self) {
        self.enable_live_bridge_universe();
        self.apply_gene_ledger();
    }

    /// Every live treatment on, the ledger NOT applied: the genome's universe.
    /// `gene_screen` starts here and sets each gene to its drawn state; the
    /// membership tests read this body. Deployment is `enable_live_bridge`.
    pub fn enable_live_bridge_universe(&mut self) {
        // Every `live()` gene in the registry: the repairs and the host-only
        // adapters. The reason each exists is on its row in `genes.rs`.
        for gene in super::GENES.iter().filter(|gene| gene.live()) {
            (gene.enable)(self);
        }
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
    /// `enable_live_bridge` is therefore this function plus the host-only
    /// genes (`Kind::HostOnly` in `genes.rs`). Both bundles are loops over the
    /// one registry, so they cannot drift apart.
    pub fn enable_engine_repairs(&mut self) {
        self.enable_engine_repairs_universe();
        self.apply_gene_ledger();
    }

    /// Every native repair on, the ledger NOT applied — the two halves and
    /// nothing else. See `enable_live_bridge_universe`.
    pub fn enable_engine_repairs_universe(&mut self) {
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
        for gene in super::GENES
            .iter()
            .filter(|gene| gene.repair_axis() == Some(super::Axis::War))
        {
            (gene.enable)(self);
        }
    }

    /// The economic half of [`AdvancedAi::enable_engine_repairs`]: settlement,
    /// growth, districts, and the policy deck.
    pub fn enable_engine_repairs_economy(&mut self) {
        for gene in super::GENES
            .iter()
            .filter(|gene| gene.repair_axis() == Some(super::Axis::Economy))
        {
            (gene.enable)(self);
        }
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
        self.joint_reach_lines = true;
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

    /// The joint search's approach lines from the engine's exact reach flood
    /// (`Game::approach_reach`, `docs/TACTICS.md` §17). On by default wherever
    /// the joint search runs; this pair exists so the live bridge can price
    /// the lines by taking them out (`live_without_joint_reach_lines`) and the
    /// bench can seat the geometric portfolio by name
    /// (`advanced_joint_tactics_geometric`).
    pub fn enable_joint_reach_lines(&mut self) {
        self.joint_reach_lines = true;
    }

    /// See [`AdvancedAi::enable_joint_reach_lines`].
    pub fn disable_joint_reach_lines(&mut self) {
        self.joint_reach_lines = false;
    }

    pub fn disable_solvent_faith_army(&mut self) {
        self.solvent_faith_army = false;
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
    /// `advanced_settler_founds_when_stalled`; off in the native production
    /// controller.
    ///
    /// ⚠ **Not off in the live bridge.** `enable_stranded_settler_discount`
    /// sets the same flag as part of its bundle, and `enable_live_bridge`
    /// calls that — so a live seat DOES found where it stands, and this
    /// explicit enabler is only how an evaluator seats the behaviour on a
    /// native arm. Reading "off in production" here and stopping cost a study
    /// on 2026-08-19 a false conclusion: `founds_where_it_stands` opens with
    /// `if !self.settler_founds_when_stalled`, so the flag looks like the
    /// reason that path fires rarely on the live ladder. It is not — the flag
    /// is on, and `g.can_found_city` refusing the tile a walker happens to be
    /// standing on is what usually ends it.
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

    /// The off toggle, so the registry row has both directions.
    pub fn disable_builder_reward_survey(&mut self) {
        self.builder_reward_survey = false;
    }

    /// Prefer existing Builder work that pays on a tile a citizen currently
    /// works, while preserving luxury and strategic connections. Native
    /// opt-in gene `builder-worked-tile-priority`; off in production.
    pub fn enable_builder_worked_tile_priority(&mut self) {
        self.builder_worked_tile_priority = true;
    }

    /// The twin of `enable_builder_worked_tile_priority`.
    pub fn disable_builder_worked_tile_priority(&mut self) {
        self.builder_worked_tile_priority = false;
    }

    /// Keep Builders from entering a visible Barbarian-capture envelope.
    /// Native opt-in gene `builder-barbarian-safety`; off in production until
    /// its targeted barbarian screen has priced the safety/tempo trade.
    pub fn enable_builder_barbarian_safety(&mut self) {
        self.builder_barbarian_safety = true;
    }

    /// The twin of `enable_builder_barbarian_safety`.
    pub fn disable_builder_barbarian_safety(&mut self) {
        self.builder_barbarian_safety = false;
    }
    /// Credit strength-per-production and the civ's own unique unit in the
    /// military production arm. Evaluator arm `advanced_unit_efficiency`;
    /// off in production.
    pub fn enable_unit_cost_efficiency(&mut self) {
        self.unit_cost_efficiency = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_unit_cost_efficiency(&mut self) {
        self.unit_cost_efficiency = false;
    }

    /// Fortify units the planner gave nothing to do. Evaluator arm
    /// `advanced_fortify_idle_units`; off in production.
    pub fn enable_fortify_idle_units(&mut self) {
        self.base.fortify_idle_units = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_fortify_idle_units(&mut self) {
        self.base.fortify_idle_units = false;
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

    /// The off toggle, so the registry row has both directions.
    pub fn disable_naval_production_policy(&mut self) {
        self.naval_production_policy = false;
    }

    /// Sea threats get sea answers. See `BasicAi::sea_answers`; entrant
    /// `advanced_sea_answers`.
    pub fn enable_sea_answers(&mut self) {
        self.base.sea_answers = true;
    }

    /// Answer a ring of shooters with a shooter. See
    /// `BasicAi::barbarian_ranged_answer`; withheld by `barbarian-ranged-answer`.
    pub fn enable_barbarian_ranged_answer(&mut self) {
        self.base.enable_barbarian_ranged_answer();
    }

    pub fn disable_barbarian_ranged_answer(&mut self) {
        self.base.disable_barbarian_ranged_answer();
    }

    /// Price a raider's life below a major's. See `BasicAi::barbarian_bargain`;
    /// withheld by `barbarian-bargain`.
    pub fn enable_barbarian_bargain(&mut self) {
        self.base.enable_barbarian_bargain();
    }

    pub fn disable_barbarian_bargain(&mut self) {
        self.base.disable_barbarian_bargain();
    }

    /// See `BasicAi::barbarian_hunt`; withheld by the `barbarian-hunt`
    /// treatment.
    pub fn enable_barbarian_hunt(&mut self) {
        self.base.enable_barbarian_hunt();
    }

    pub fn disable_barbarian_hunt(&mut self) {
        self.base.disable_barbarian_hunt();
    }

    /// Deliberate camp clearing as a peacetime errand. See
    /// `BasicAi::camp_bounty`; entrant `advanced_camp_bounty`.
    pub fn enable_camp_bounty(&mut self) {
        self.base.camp_bounty = true;
    }

    /// Walk onto a visible, undefended barbarian camp one legal step away.
    /// See `BasicAi::adjacent_camp_clear`; withheld by
    /// `advanced_without_adjacent_camp_clear`.
    pub fn enable_adjacent_camp_clear(&mut self) {
        self.base.adjacent_camp_clear = true;
    }

    pub fn disable_adjacent_camp_clear(&mut self) {
        self.base.adjacent_camp_clear = false;
    }

    /// Let the deck counterfactual see the unit-maintenance bill. See
    /// `BasicAi::maintenance_aware_deck`; entrant `advanced_maintenance_deck`.
    pub fn enable_maintenance_aware_deck(&mut self) {
        self.base.maintenance_aware_deck = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_maintenance_aware_deck(&mut self) {
        self.base.maintenance_aware_deck = false;
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
        self.governor_victory_lanes = true;
        self.governor_expansion_lane = true;
    }

    pub fn disable_governor_every_lane(&mut self) {
        self.governor_every_lane = false;
        self.governor_victory_lanes = false;
        self.governor_expansion_lane = false;
    }

    /// Half the composite: the governor under the four victory lanes only.
    /// See `governor_victory_lanes`.
    pub fn enable_governor_victory_lanes(&mut self) {
        self.governor_victory_lanes = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_governor_victory_lanes(&mut self) {
        self.governor_victory_lanes = false;
    }

    /// The other half: the governor under Expansion only. See
    /// `governor_expansion_lane`.
    pub fn enable_governor_expansion_lane(&mut self) {
        self.governor_expansion_lane = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_governor_expansion_lane(&mut self) {
        self.governor_expansion_lane = false;
    }

    /// Make the settlement-gap redirect and the Settler ranking honour the same
    /// city target the cascade settles toward. See `settlement_target`.
    pub fn enable_settlement_gap_target(&mut self) {
        self.settlement_gap_reads_city_target = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_settlement_gap_target(&mut self) {
        self.settlement_gap_reads_city_target = false;
    }

    /// Withhold the strategic governor from the Recovery lane, so the baseline
    /// cascade runs those cities too. See `governor_in_recovery`.
    pub fn disable_governor_in_recovery(&mut self) {
        self.governor_in_recovery = false;
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

    /// Expand to the land the empire can hold, at pipeline pace (see
    /// `land_grab`). Sets both halves: the strategic governor's target,
    /// window and pipeline, and `BasicAi::pick_item`'s pipeline and window.
    pub fn enable_land_grab(&mut self) {
        self.land_grab = true;
        self.base.land_grab = true;
    }

    pub fn disable_land_grab(&mut self) {
        self.land_grab = false;
        self.base.land_grab = false;
    }

    /// Take the pantheon that founds a city and keep the Faith card that buys
    /// it (see `expansion_pantheon`). Sets both halves: the strategic
    /// portfolio's God-King want, and `BasicAi`'s pantheon prefix.
    pub fn enable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = true;
        self.base.expansion_pantheon = true;
    }

    pub fn disable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = false;
        self.base.expansion_pantheon = false;
    }

    /// Price the Ancestral Hall for the land grab (see `expansion_hall`).
    pub fn enable_expansion_hall(&mut self) {
        self.expansion_hall = true;
    }

    pub fn disable_expansion_hall(&mut self) {
        self.expansion_hall = false;
    }

    /// Hold the opening book's Settler slot for the host's population floor
    /// (see `opening_settler_waits`).
    pub fn enable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = true;
        self.base.opening_settler_waits = true;
    }

    pub fn disable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = false;
        self.base.opening_settler_waits = false;
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

    /// Make every specialty district owe its own buildings, whatever the
    /// lane. See `district_building_chain`.
    pub fn enable_district_building_chain(&mut self) {
        self.district_building_chain = true;
    }

    pub fn disable_district_building_chain(&mut self) {
        self.district_building_chain = false;
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

    /// Let a Settler switch to the best safe alternate when a visible threat
    /// blocks the next step toward an otherwise sound settlement site. See
    /// `settler_threat_detour`.
    pub fn enable_settler_threat_detour(&mut self) {
        self.settler_threat_detour = true;
    }

    pub fn disable_settler_threat_detour(&mut self) {
        self.settler_threat_detour = false;
    }

    /// Price a Settler's walk in turns, each turn dearer the longer the
    /// Settler has already been walking, so expansion founds sooner without
    /// giving up a site good enough to pay for its walk. See `settle_sooner`.
    pub fn enable_settle_sooner(&mut self) {
        self.settle_sooner = true;
    }

    pub fn disable_settle_sooner(&mut self) {
        self.settle_sooner = false;
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

    /// Walk onto a capturable civilian within reach, and never decline a
    /// settler held by the barbarians. See `BasicAi::civilian_rescue`.
    pub fn enable_civilian_rescue(&mut self) {
        self.base.civilian_rescue = true;
    }

    pub fn disable_civilian_rescue(&mut self) {
        self.base.civilian_rescue = false;
    }

    /// Take a visible Barbarian Settler or Scout in exact one-turn reach
    /// before healing, retreat, or any ordinary tactical choice.  This native
    /// opt-in is screened as `barbarian-capture-priority`; see
    /// [`BasicAi::barbarian_capture_priority`](crate::ai::BasicAi::barbarian_capture_priority).
    pub fn enable_barbarian_capture_priority(&mut self) {
        self.base.barbarian_capture_priority = true;
    }

    /// The twin of `enable_barbarian_capture_priority`.
    pub fn disable_barbarian_capture_priority(&mut self) {
        self.base.barbarian_capture_priority = false;
    }

    /// A unit one enemy blow from death withdraws to safe healing ground, and
    /// leaves that ground again the moment an enemy can strike it. See
    /// `BasicAi::one_shot_recovery`.
    pub fn enable_one_shot_recovery(&mut self) {
        self.base.one_shot_recovery = true;
    }

    pub fn disable_one_shot_recovery(&mut self) {
        self.base.one_shot_recovery = false;
    }

    /// Keep the hostile-envelope table across this seat's own unit moves —
    /// evaluator arm `advanced_envelope_own_moves`. See
    /// `BasicAi::envelope_cache_across_own_moves`.
    pub fn enable_envelope_cache_across_own_moves(&mut self) {
        self.base.enable_envelope_cache_across_own_moves();
    }

    /// Stop pricing a Firaxis barbarian scout as a threat. See
    /// `barbarian_scouts_are_scouts`.
    pub fn enable_barbarian_scouts_are_scouts(&mut self) {
        self.barbarian_scouts_are_scouts = true;
    }

    pub fn disable_barbarian_scouts_are_scouts(&mut self) {
        self.barbarian_scouts_are_scouts = false;
    }

    /// Skip a space race or a bomb that cannot finish before the turn limit.
    /// See `score_horizon`.
    pub fn enable_score_horizon(&mut self) {
        self.score_horizon = true;
    }

    pub fn disable_score_horizon(&mut self) {
        self.score_horizon = false;
    }

    /// Build the wonders the chosen victory actually needs. See
    /// `AdvancedAi::strategic_wonder_value`.
    pub fn enable_strategic_wonders(&mut self) {
        self.strategic_wonders = true;
    }

    /// Withholding twin for `enable_strategic_wonders`, so the arm can be
    /// priced by taking this one treatment out. See `LIVE_TREATMENTS`.
    pub fn disable_strategic_wonders(&mut self) {
        self.strategic_wonders = false;
    }

    /// Give the 3,000-point first-pad rung to one city at a time. See
    /// `one_launch_pad`.
    pub fn enable_one_launch_pad(&mut self) {
        self.one_launch_pad = true;
    }

    pub fn disable_one_launch_pad(&mut self) {
        self.one_launch_pad = false;
    }
    /// Aim research at the housing ceiling when the empire is paying it.
    pub fn enable_housing_research(&mut self) {
        self.housing_research = true;
    }

    pub fn disable_housing_research(&mut self) {
        self.housing_research = false;
    }

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

    pub fn disable_siege_tracks_the_wall(&mut self) {
        self.siege_tracks_the_wall = false;
    }

    pub fn disable_blind_objective_strength(&mut self) {
        self.blind_objective_strength = false;
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
    /// Choose the pantheon from the land the empire holds rather than from a
    /// fixed order. Reachable as `advanced_pantheon_board`; see
    /// `BasicAi::pantheon_reads_the_board`.
    pub fn enable_pantheon_board(&mut self) {
        self.base.pantheon_reads_the_board = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_pantheon_board(&mut self) {
        self.base.pantheon_reads_the_board = false;
    }

    /// Score the World Congress ballot — which outcome and target this seat
    /// names — for the victory the empire is actually racing rather than for
    /// an expansion posture that has no lane. See `advanced/victory_lane.rs`.
    /// The Favor stake behind the ballot is `lane-congress-favor`, a separate
    /// gene. Off everywhere by default; opt-in gene `lane-congress-ballot`.
    pub fn enable_lane_congress_ballot(&mut self) {
        self.lane_congress_ballot = true;
    }

    /// The twin of `enable_lane_congress_ballot`.
    pub fn disable_lane_congress_ballot(&mut self) {
        self.lane_congress_ballot = false;
    }

    /// Stake the Favor behind a World Congress ballot for the victory the
    /// empire is actually racing. The other half of what
    /// `lane-congress-ballot` used to be, split from it after the lane's own
    /// regime flagged the composite at −0.61 pp of score share (z −2.33).
    ///
    /// ★★★ And priced apart it is the **better** half, not the worse one the
    /// split predicted: +1.4 pp at 570 pairs against the naming half's
    /// −1.8, positive in all four windows. See `advanced/victory_lane.rs` and
    /// `docs/VICTORY_GENES.md` §8.5. Off everywhere by default; opt-in gene
    /// `lane-congress-favor`.
    pub fn enable_lane_congress_favor(&mut self) {
        self.lane_congress_favor = true;
    }

    /// The twin of `enable_lane_congress_favor`.
    pub fn disable_lane_congress_favor(&mut self) {
        self.lane_congress_favor = false;
    }

    /// Rank Great Person classes, and the Great Person points a project earns,
    /// by the victory the empire is actually racing rather than by a war it is
    /// fighting. See `advanced/victory_lane.rs` — this is the one gene there
    /// that overrides a Conquest plan, and the fires-check that chose that
    /// scope is in `docs/VICTORY_GENES.md` §7. `Recovery` still keeps its own
    /// strategy. Off everywhere by default; opt-in gene `lane-great-people`.
    pub fn enable_lane_great_people(&mut self) {
        self.lane_great_people = true;
    }

    /// The twin of `enable_lane_great_people`.
    pub fn disable_lane_great_people(&mut self) {
        self.lane_great_people = false;
    }

    /// Choose the policy cards for the victory the empire is actually racing
    /// while its plan is still Expansion. See `advanced/victory_lane.rs`. Off everywhere by
    /// default; opt-in gene `lane-policy-deck`.
    pub fn enable_lane_policy_deck(&mut self) {
        self.lane_policy_deck = true;
    }

    /// The twin of `enable_lane_policy_deck`.
    pub fn disable_lane_policy_deck(&mut self) {
        self.lane_policy_deck = false;
    }

    /// Run the Culture lane's Faith pass — the Naturalist that founds a
    /// National Park, the touring Rock Bands — and size its reserve, for an
    /// empire racing Culture whose plan has not named the lane. See
    /// `advanced/victory_lane.rs`; `Recovery` still refuses. Off everywhere by
    /// default; opt-in gene `lane-culture-spending`.
    pub fn enable_lane_culture_spending(&mut self) {
        self.lane_culture_spending = true;
    }

    /// The twin of `enable_lane_culture_spending`.
    pub fn disable_lane_culture_spending(&mut self) {
        self.lane_culture_spending = false;
    }

    /// Treat an empire racing Science as a Science seat throughout the space
    /// race: the pad count, the city a launch project may claim and the city a
    /// pad may be sited in all read the race rather than an explicitly
    /// assigned target, and the pass opens at all. `score_horizon` still
    /// refuses a race that cannot finish. See `advanced/victory_lane.rs`. Off
    /// everywhere by default; opt-in gene `lane-space-race`.
    pub fn enable_lane_space_race(&mut self) {
        self.lane_space_race = true;
    }

    /// The twin of `enable_lane_space_race`.
    pub fn disable_lane_space_race(&mut self) {
        self.lane_space_race = false;
    }

    /// Price a scored competition's first place by the Diplomatic Victory
    /// Points it pays, at the rate `strategic_wonder_value` already pays a
    /// wonder's. See `advanced/victory_lane.rs`. Off everywhere by
    /// default; opt-in gene `competition-victory-points`.
    pub fn enable_competition_victory_points(&mut self) {
        self.competition_victory_points = true;
    }

    /// The twin of `enable_competition_victory_points`.
    pub fn disable_competition_victory_points(&mut self) {
        self.competition_victory_points = false;
    }

    /// Answer a World Congress resolution that is already decided with the one
    /// free vote on its settled winner, taking the Diplomatic Victory Point
    /// for an exact prediction and staking nothing.
    ///
    /// Its own field doc records the measurement and it has sat unused: **26
    /// of 192 ballot decisions already settled, ~1.4 free points a seat a game
    /// against the twenty a diplomatic victory needs**. Reachable as
    /// `advanced_congress_banks_decided`; now also as a gene.
    ///
    /// ★★★ THIS AND THE TWO BELOW ARE NOT NEW BEHAVIOUR. They already existed,
    /// off in production and reachable only as named `elo.rs` arms — which
    /// means `gene_screen` could not see them and the genome instrument has
    /// never priced any of them. `docs/VICTORY_GENES.md` §9 counts these
    /// behaviours; these are the three the Diplomacy lane needs, and a toggle
    /// pair plus a `PRODUCTION_OPT_INS` row is the whole of making one
    /// screenable.
    pub fn enable_congress_banks_a_decided_vote(&mut self) {
        self.congress_banks_a_decided_vote = true;
    }

    /// The twin of `enable_congress_banks_a_decided_vote`.
    pub fn disable_congress_banks_a_decided_vote(&mut self) {
        self.congress_banks_a_decided_vote = false;
    }

    /// Back a ballot aimed at the empire closest to a victory with everything
    /// the treasury can spare — a losing vote is refunded in full, so an
    /// opposition that fails costs no Favor. Gene `congress-counter-votes`.
    pub fn enable_congress_counter_votes(&mut self) {
        self.congress_counter_votes = true;
    }

    /// The twin of `enable_congress_counter_votes`.
    pub fn disable_congress_counter_votes(&mut self) {
        self.congress_counter_votes = false;
    }

    /// Value the infrastructure that produces city-state influence: the
    /// Consulate and Chancery's per-turn influence becomes the envoys it can
    /// produce before the turn limit, and a first Diplomatic Quarter sees
    /// part of the Consulate stream it unlocks. Gene `envoy-infrastructure`.
    pub fn enable_envoy_infrastructure(&mut self) {
        self.envoy_infrastructure = true;
    }

    /// The twin of `enable_envoy_infrastructure`.
    pub fn disable_envoy_infrastructure(&mut self) {
        self.envoy_infrastructure = false;
    }

    /// Beeline Advanced Flight from three technologies out, raise an
    /// Aerodrome and a bomber wing, and take the appointed city with the
    /// cavalry behind it. See [`AdvancedAi::maintain_air_surge`] and
    /// `advanced/air_surge.rs`. Off everywhere by default; opt-in gene
    /// `air-surge`.
    pub fn enable_air_surge(&mut self) {
        self.air_surge = true;
    }

    /// The twin of `enable_air_surge`.
    pub fn disable_air_surge(&mut self) {
        self.air_surge = false;
    }

    /// Admit the friendly-volley extension without the rest of the closed
    /// war-half bundle. See [`AdvancedAi::coordinated_finish`].
    ///
    /// ⭐ THIS PAIR EXISTS SO THE FLAG IS COUNTED AT ALL. `docs/EVAL_STATUS.md`
    /// counts a capability toggle by its `enable_*`/`disable_*` pair here, so a
    /// field only ever set directly — `ai.coordinated_finish = true` in
    /// `src/elo.rs`'s `advanced_coordinated_finish` arm — is not merely
    /// unreachable by a screen, it is invisible to the count that measures the
    /// debt. `docs/VICTORY_GENES.md` §9 names it first among the fields in
    /// that position. Off everywhere by default; opt-in gene
    /// `coordinated-finish`.
    pub fn enable_coordinated_finish(&mut self) {
        self.coordinated_finish = true;
    }

    /// The twin of `enable_coordinated_finish`.
    pub fn disable_coordinated_finish(&mut self) {
        self.coordinated_finish = false;
    }
    // Append points, one per name range: a new treatment goes under the range
    // its own name falls in, so that two of them do not append to one line.
    // The rule, the measurement behind it and the check that enforces it are
    // on `pub struct AdvancedAi` in `src/ai/advanced.rs`.

    // ---- append: a-b ------------------------------------------------

    // ---- append: c-d ------------------------------------------------

    /// The city plans its districts, sites and tile buys together: wished
    /// districts get jointly assigned, reserved plots over rings 1-3, and
    /// the tile a very valuable site needs is bought. See
    /// [`AdvancedAi::district_planning`]. Opt-in gene `district-planning`.
    pub fn enable_district_planning(&mut self) {
        self.district_planning = true;
    }

    /// The twin of `enable_district_planning`.
    pub fn disable_district_planning(&mut self) {
        self.district_planning = false;
    }

    // ---- append: e-f ------------------------------------------------

    // ---- append: g-k ------------------------------------------------

    // ---- append: l-o ------------------------------------------------

    // ---- append: p-r ------------------------------------------------

    // ---- append: s-s ------------------------------------------------

    // ---- append: t-z ------------------------------------------------
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
        &source[..source
            .find(marker)
            .expect("the guard module moved or was renamed")]
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
