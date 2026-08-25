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
    /// Size the defensive Missionary corps by cities actually under conversion
    /// pressure, up to four, instead of two. Off in production; opted into by
    /// name. See [`AdvancedAi::religious_defence_scales`].
    pub fn enable_religious_defence_scales(&mut self) {
        self.religious_defence_scales = true;
    }

    /// The twin of `enable_religious_defence_scales`.
    pub fn disable_religious_defence_scales(&mut self) {
        self.religious_defence_scales = false;
    }

    /// Let a founder defending its own cities buy one Guru, the only unit that
    /// heals religious units. Off in production; opted into by name. See
    /// [`AdvancedAi::guru_heals_the_corps`].
    pub fn enable_guru_heals_the_corps(&mut self) {
        self.guru_heals_the_corps = true;
    }

    /// The twin of `enable_guru_heals_the_corps`.
    pub fn disable_guru_heals_the_corps(&mut self) {
        self.guru_heals_the_corps = false;
    }

    /// Let a wounded spreader in its own Holy Site's heal ring hold and heal
    /// instead of spending a weak charge. Off in production; opted into by
    /// name. See [`AdvancedAi::religious_units_heal_first`].
    pub fn enable_religious_units_heal_first(&mut self) {
        self.religious_units_heal_first = true;
    }

    /// The twin of `enable_religious_units_heal_first`.
    pub fn disable_religious_units_heal_first(&mut self) {
        self.religious_units_heal_first = false;
    }

    /// Condemn a heretic whose religion the World Congress condemned, not only
    /// one belonging to a war enemy. Off in production; opted into by name. See
    /// [`AdvancedAi::condemn_under_congress`].
    pub fn enable_condemn_under_congress(&mut self) {
        self.condemn_under_congress = true;
    }

    /// The twin of `enable_condemn_under_congress`.
    pub fn disable_condemn_under_congress(&mut self) {
        self.condemn_under_congress = false;
    }

    /// Keep a spread campaign on the offensive between waves once it has
    /// converted a foreign city. Keep a spread campaign that has already
    /// converted a foreign city on the offensive between waves, instead of
    /// dropping the posture the turn its last charge is spent. Off in
    /// production; opted into by name. See
    /// [`AdvancedAi::spread_campaign_persists`].
    pub fn enable_spread_campaign_persists(&mut self) {
        self.spread_campaign_persists = true;
    }

    /// The twin of `enable_spread_campaign_persists`.
    pub fn disable_spread_campaign_persists(&mut self) {
        self.spread_campaign_persists = false;
    }

    /// Build a Holy Site in the city losing its religious majority so defenders
    /// can be bought there directly. Put a Holy Site in the city that is
    /// actually losing its majority, so its defender can be bought there
    /// instead of walking from the Holy City. Off in production; opted into by
    /// name. See [`AdvancedAi::holy_site_where_the_threat_is`].
    pub fn enable_holy_site_where_the_threat_is(&mut self) {
        self.holy_site_where_the_threat_is = true;
    }

    /// The twin of `enable_holy_site_where_the_threat_is`.
    pub fn disable_holy_site_where_the_threat_is(&mut self) {
        self.holy_site_where_the_threat_is = false;
    }

    /// Choose the enhancer beliefs that multiply religious spread while the
    /// corps has a job to do. Off in production; opted into by name. See
    /// [`AdvancedAi::enhancer_for_the_corps`].
    pub fn enable_enhancer_for_the_corps(&mut self) {
        self.enhancer_for_the_corps = true;
    }

    /// The twin of `enable_enhancer_for_the_corps`.
    pub fn disable_enhancer_for_the_corps(&mut self) {
        self.enhancer_for_the_corps = false;
    }

    /// Promote an Apostle for the job the empire needs rather than the largest
    /// number on the card. Off in production; opted into by name (`victory_eval
    /// --with apostle-promotion-by-role`, `gene_screen`). See
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

    /// Adapt a live Civilization VI Trader, which cannot walk, to its distinct
    /// route-start action. Native tournament games leave this disabled.
    pub fn enable_live_trader_route_adapter(&mut self) {
        self.live_trader_route_adapter = true;
    }

    /// Withholding twin for `enable_live_trader_route_adapter`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_live_trader_route_adapter(&mut self) {
        self.live_trader_route_adapter = false;
    }
    /// Let a founder under conversion pressure buy one Apostle to launch the
    /// Inquisition after its Missionaries. See
    /// [`AdvancedAi::inquisition_on_threat`]. Off everywhere by default; opted
    /// into by name (`gene_screen`, `victory_eval --with
    /// inquisition-on-threat`) and listed in `PRODUCTION_OPT_INS`.
    pub fn enable_inquisition_on_threat(&mut self) {
        self.inquisition_on_threat = true;
    }

    /// The twin of `enable_inquisition_on_threat`.
    pub fn disable_inquisition_on_threat(&mut self) {
        self.inquisition_on_threat = false;
    }

    /// Have a founder outside the Religion lane still build the Shrine and
    /// Temple an Apostle needs. See [`AdvancedAi::founder_temple`]. Opt-in
    /// gene.
    pub fn enable_founder_temple(&mut self) {
        self.founder_temple = true;
    }

    /// The twin of `enable_founder_temple`.
    pub fn disable_founder_temple(&mut self) {
        self.founder_temple = false;
    }

    /// Have a founder research Theology next, after its first government, so it
    /// can build a Temple. See [`AdvancedAi::theology_for_founders`]. Opt-in
    /// gene.
    pub fn enable_theology_for_founders(&mut self) {
        self.theology_for_founders = true;
    }

    /// The twin of `enable_theology_for_founders`.
    pub fn disable_theology_for_founders(&mut self) {
        self.theology_for_founders = false;
    }

    /// Let a unit inside enemy reach choose to hold and heal, close on a
    /// shooter, or step out of range. A unit already inside a hostile's
    /// next-turn reach picks a posture: stand and heal where the melee exchange
    /// favours holding, close on a shooter it cannot answer, or step out of
    /// that shooter's envelope. See [`AdvancedAi::contact_posture`]. Opt-in
    /// gene `contact-posture`.
    pub fn enable_contact_posture(&mut self) {
        self.contact_posture = true;
    }

    /// The twin of `enable_contact_posture`.
    pub fn disable_contact_posture(&mut self) {
        self.contact_posture = false;
    }

    /// Score a settle site by the districts the lane would actually build
    /// there, each on its own plot. See
    /// [`AdvancedAi::district_lookahead_settle`]. Opt-in gene
    /// `district-lookahead-settle`.
    pub fn enable_district_lookahead_settle(&mut self) {
        self.district_lookahead_settle = true;
    }

    /// The twin of `enable_district_lookahead_settle`.
    pub fn disable_district_lookahead_settle(&mut self) {
        self.district_lookahead_settle = false;
    }

    /// Buy a border plot only when its priced benefit clears its Gold cost by a
    /// margin. See [`AdvancedAi::priced_tile_purchase`]. Opt-in gene
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

    /// Credit a Campus building the science its city's multipliers will
    /// actually pay, not its raw spec yield. See
    /// [`AdvancedAi::science_multiplier_payoff`]. Opt-in gene
    /// `science-multiplier-payoff`.
    pub fn enable_science_multiplier_payoff(&mut self) {
        self.science_multiplier_payoff = true;
    }

    /// The twin of `enable_science_multiplier_payoff`.
    pub fn disable_science_multiplier_payoff(&mut self) {
        self.science_multiplier_payoff = false;
    }

    /// Scale a missing Campus building's debt by its own science yield, so
    /// Universities and Labs outrank Libraries. See
    /// [`AdvancedAi::research_tier_premium`]. Opt-in gene
    /// `research-tier-premium`.
    pub fn enable_research_tier_premium(&mut self) {
        self.research_tier_premium = true;
    }

    /// The twin of `enable_research_tier_premium`.
    pub fn disable_research_tier_premium(&mut self) {
        self.research_tier_premium = false;
    }

    /// Scale the Campus coverage bonus by how complete the empire's existing
    /// Campuses are, so Labs come before new Campuses. See
    /// [`AdvancedAi::campus_finishes_first`]. Opt-in gene
    /// `campus-finishes-first`.
    pub fn enable_campus_finishes_first(&mut self) {
        self.campus_finishes_first = true;
    }

    /// The twin of `enable_campus_finishes_first`.
    pub fn disable_campus_finishes_first(&mut self) {
        self.campus_finishes_first = false;
    }

    /// Credit a power plant the powered yields it switches on, above all the
    /// Research Lab's extra science. See [`AdvancedAi::power_the_laboratory`].
    /// Opt-in gene `power-the-laboratory`.
    pub fn enable_power_the_laboratory(&mut self) {
        self.power_the_laboratory = true;
    }

    /// The twin of `enable_power_the_laboratory`.
    pub fn disable_power_the_laboratory(&mut self) {
        self.power_the_laboratory = false;
    }

    /// Credit a Campus plot that reaches the Rationalism adjacency threshold
    /// with the science bonus crossing it unlocks. See
    /// [`AdvancedAi::campus_adjacency_threshold`]. Opt-in gene
    /// `campus-adjacency-threshold`.
    pub fn enable_campus_adjacency_threshold(&mut self) {
        self.campus_adjacency_threshold = true;
    }

    /// The twin of `enable_campus_adjacency_threshold`.
    pub fn disable_campus_adjacency_threshold(&mut self) {
        self.campus_adjacency_threshold = false;
    }

    /// Credit growth in a Campus city near the Rationalism population gate with
    /// the science bonus crossing it unlocks. See
    /// [`AdvancedAi::fifteenth_citizen`]. Opt-in gene `fifteenth-citizen`.
    pub fn enable_fifteenth_citizen(&mut self) {
        self.fifteenth_citizen = true;
    }

    /// The twin of `enable_fifteenth_citizen`.
    pub fn disable_fifteenth_citizen(&mut self) {
        self.fifteenth_citizen = false;
    }

    /// Let a seat with no religion and 600+ banked Faith patronize Great People
    /// whatever the points shortfall. See [`AdvancedAi::idle_faith_patronage`].
    /// Opt-in gene.
    pub fn enable_idle_faith_patronage(&mut self) {
        self.idle_faith_patronage = true;
    }

    /// The twin of `enable_idle_faith_patronage`.
    pub fn disable_idle_faith_patronage(&mut self) {
        self.idle_faith_patronage = false;
    }

    /// Buy the second and third Scout early, before Early Empire closes borders
    /// and city-states become unreachable. Buy the second and third Scout while
    /// the world's borders are still open — after Early Empire a city-state
    /// cannot be met by land at all. See [`AdvancedAi::early_contact_window`].
    pub fn enable_early_contact_window(&mut self) {
        self.early_contact_window = true;
    }

    /// The twin of `enable_early_contact_window`.
    pub fn disable_early_contact_window(&mut self) {
        self.early_contact_window = false;
    }

    /// Reserve a city to build whatever unblocks an earned Great Person,
    /// selling duplicate works to make room. A class earned and blocked
    /// reserves a city for the slot building, district, wonder or soldier that
    /// lifts the block, and a due cultural person sells duplicate works to make
    /// room. See [`AdvancedAi::great_person_housing`]. Opt-in gene.
    pub fn enable_great_person_housing(&mut self) {
        self.great_person_housing = true;
    }

    /// The twin of `enable_great_person_housing`.
    pub fn disable_great_person_housing(&mut self) {
        self.great_person_housing = false;
    }

    /// Open a surprise war on a neighbour whose Settlers, Builders or tiles lie
    /// exposed nearby, then sue for peace. Open a surprise war on a neighbour
    /// whose unescorted Settlers, Builders or unpillaged tiles lie within a
    /// short march of our soldiers, take them, and sue for peace. See
    /// [`AdvancedAi::opportunistic_war`]. Opt-in gene.
    pub fn enable_opportunistic_war(&mut self) {
        self.opportunistic_war = true;
    }

    /// The twin of `enable_opportunistic_war`.
    pub fn disable_opportunistic_war(&mut self) {
        self.opportunistic_war = false;
        self.raid_war = None;
    }

    /// Count a neighbour's unpillaged improvements within reach as raid prizes
    /// and send raiders to pillage them. See
    /// [`AdvancedAi::raid_pillage_prizes`]. Opt-in gene; inert unless
    /// `opportunistic_war` is on.
    pub fn enable_raid_pillage_prizes(&mut self) {
        self.raid_pillage_prizes = true;
    }

    /// The twin of `enable_raid_pillage_prizes`.
    pub fn disable_raid_pillage_prizes(&mut self) {
        self.raid_pillage_prizes = false;
    }

    /// Price the Religion lane's Holy Site at what the Culture lane pays for
    /// its Theater Square. See [`AdvancedAi::holy_lane_parity`]; the evaluator
    /// arm `advanced_holy_lane` sets the field directly, this pair makes it a
    /// native opt-in gene (`PRODUCTION_OPT_INS`).
    pub fn enable_holy_lane_parity(&mut self) {
        self.holy_lane_parity = true;
    }

    /// The twin of `enable_holy_lane_parity`.
    pub fn disable_holy_lane_parity(&mut self) {
        self.holy_lane_parity = false;
    }
    /// Enforce Civilization VI's rule that a purchased religious unit inherits
    /// its city's majority religion. Native tournament games leave this
    /// disabled.
    pub fn enable_live_religious_purchase_guard(&mut self) {
        self.base.live_religious_purchase_guard = true;
    }

    /// Withholding twin for `enable_live_religious_purchase_guard`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_live_religious_purchase_guard(&mut self) {
        self.base.live_religious_purchase_guard = false;
    }
    /// Let hostile units inside our own territory claim defenders before the
    /// offensive campaign takes them. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_home_defense(&mut self) {
        self.base.home_defense = true;
    }

    /// Discount a Settler that has stopped walking from the expansion gate, and
    /// found where it stands when stalled. Native tournament games leave this
    /// disabled so their recorded ladders replay the historical controller move
    /// for move.
    pub fn enable_stranded_settler_discount(&mut self) {
        self.base.settler_strand_discount = true;
        // Discounting the stuck body lets another Settler enter the pipeline,
        // but does not convert the production already standing on legal, safe
        // ground. Once the bounded stall counter expires, finish that city
        // instead of beginning another target cycle.
        self.settler_founds_when_stalled = true;
    }

    /// Record each tactical step so a unit moved twice in one turn cannot
    /// return to the tile it just left. Native tournament games leave this
    /// disabled so their recorded ladders replay move-for-move.
    pub fn enable_recorded_tactical_step(&mut self) {
        self.base.recorded_tactical_step = true;
    }

    /// Count a unit's livelock history once per Civilization VI turn, not once
    /// per live replan frame. Keep that bridge-only accounting behind a
    /// separately withholdable treatment.
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

    /// Refuse any step onto a tile the unit already stood on this turn, closing
    /// three-hop loops too.
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

    /// Stop a blind-planned unit at the first step that reveals new ground and
    /// finish its movement sighted. A blind-planned unit stops at the first
    /// step that revealed new ground and finishes its movement sighted; on the
    /// bridge its walk is cut at the first unrevealed hex so the replan frame
    /// sees what it uncovered. See [`AdvancedAi::step_and_reassess`].
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

    /// Price the city ceiling off the passable land actually visible, at one
    /// city per 45 tiles, capped at twelve. Native tournament games leave this
    /// off so recorded ladders stay comparable; see `wide_map_capacity` for the
    /// live Settler measurement.
    pub fn enable_wide_map_capacity(&mut self) {
        self.wide_map_capacity = true;
    }

    pub fn disable_wide_map_capacity(&mut self) {
        self.wide_map_capacity = false;
    }

    /// Price the live city ceiling off the whole map's land, counting ground
    /// still hidden by fog. See `fog_land_capacity`.
    pub fn enable_fog_land_capacity(&mut self) {
        self.fog_land_capacity = true;
    }

    pub fn disable_fog_land_capacity(&mut self) {
        self.fog_land_capacity = false;
    }

    /// Let more than one Settler exist at a time, up to the shortfall against
    /// the city target. Set by the Civilization VI bridge only; native
    /// constructors and the frozen anchor keep the one-at-a-time gate in both
    /// settler routes.
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

    /// Price a Settler as an investment, subtracting production, population,
    /// escort, route and safety costs from the site's payback. It also routes
    /// the adaptive Expansion plan through `advanced_production`; otherwise the
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

    /// Start a Settler at Civilization VI's own population floor of 2 instead
    /// of the genome's higher figure. Set by the Civilization VI bridge only;
    /// native constructors and the frozen anchor keep the genome's figure.
    pub fn enable_host_settler_pop(&mut self) {
        self.base.enable_host_settler_pop();
    }

    /// Withhold the host population floor from one live evaluator arm.
    pub fn disable_host_settler_pop(&mut self) {
        self.base.host_settler_pop = false;
    }

    /// Give up an exploration target the host accepts but never actually moves
    /// the unit toward. Set by the Civilization VI bridge only.
    pub fn enable_explore_dead_targets(&mut self) {
        self.base.enable_explore_dead_targets();
    }

    /// Withhold target retirement from one live evaluator arm.
    pub fn disable_explore_dead_targets(&mut self) {
        self.base.explore_dead_targets = false;
    }

    /// Hold an exploration goal until reached and sweep outward from home
    /// instead of pacing the same box. Set by the Civilization VI bridge only.
    pub fn enable_explore_commit(&mut self) {
        self.base.enable_explore_commit();
    }

    /// Treat a city that is losing hitpoints as besieged even when fog hides
    /// every attacker. See `BasicAi::garrison_under_fire` for the t115
    /// measurement.
    pub fn enable_garrison_under_fire(&mut self) {
        self.base.garrison_under_fire = true;
    }

    pub fn disable_garrison_under_fire(&mut self) {
        self.base.garrison_under_fire = false;
    }
    /// Release a settler's linked escort after two turns without progress so
    /// the settler marches on by itself. See `escort_unstick`.
    pub fn enable_escort_unstick(&mut self) {
        self.escort_unstick = true;
    }

    pub fn disable_escort_unstick(&mut self) {
        self.escort_unstick = false;
    }
    /// Version 2 of escort-unstick: release a stalled settler's escort after
    /// two turns unless a visible barbarian raider can reach it. One version of
    /// a family plays, so this turns version 1 off. Opt-in gene
    /// `escort-unstick-2`. See [`AdvancedAi::escort_unstick_2`].
    pub fn enable_escort_unstick_2(&mut self) {
        self.escort_unstick = false;
        self.escort_unstick_2 = true;
    }

    pub fn disable_escort_unstick_2(&mut self) {
        self.escort_unstick_2 = false;
    }
    /// Escort live settlers by shadowing with ordinary moves instead of
    /// Civilization VI's formation channel, which stalls.
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
    /// In peacetime let the whole army answer home threats, ranking a nearby
    /// barbarian camp above countryside raiders. See `BasicAi::camp_party`.
    pub fn enable_camp_party(&mut self) {
        self.base.enable_camp_party();
    }

    pub fn disable_camp_party(&mut self) {
        self.base.disable_camp_party();
    }

    /// Have a Religion strategy offer peace to every at-war major so its
    /// missionaries can reach their cities. See `religion_sues_peace` for the
    /// t200 measurement.
    pub fn enable_religion_sues_peace(&mut self) {
        self.religion_sues_peace = true;
    }

    pub fn disable_religion_sues_peace(&mut self) {
        self.religion_sues_peace = false;
    }

    /// Assign explicit battlefield roles: counter cycle, ranged standoff, siege
    /// against walls, escorts and cavalry jobs. Production Advanced enabled
    /// this at construction until 2026-08-14, when the war-half withhold passed
    /// the promotion matrix (+38, CI +10..+66, seed stream 18000000; see
    /// `promoted_policy_envoy`); now only the `advanced_war_half` re-addition
    /// arm and focused evaluator controls set it.
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

    /// Let a unit remember its campaign objective, dangerous approaches and a
    /// short retreat commitment across turns. Production Advanced enabled this
    /// by default until 2026-08-14 (the war-half removal; see
    /// `promoted_policy_envoy`); the explicit method keeps focused evaluators
    /// able to opt in deliberately.
    pub fn enable_unit_objective_memory(&mut self) {
        self.base.unit_objective_memory = true;
    }

    /// Let the defensive Recovery posture expire after a turn limit instead of
    /// trapping the empire in it permanently.
    ///
    /// Production leaves this measured-null repair off. The live bridge and
    /// explicit evaluator bundles call this method when they need the repair;
    /// keeping the switch here makes that opt-in auditable.
    pub fn enable_bounded_recovery(&mut self) {
        self.bounded_recovery = true;
    }
    /// Defer a unit's promotion until it is wounded enough to use the
    /// promotion's heal instead of wasting it.
    pub fn enable_promote_when_wounded(&mut self) {
        self.promote_when_wounded = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_promote_when_wounded(&mut self) {
        self.promote_when_wounded = false;
    }

    /// Credit a movement tile for the attack it opens next, not only charge it
    /// for the threat it accepts. Native tournament games leave this disabled
    /// so their recorded ladders stay comparable.
    pub fn enable_strike_opening(&mut self) {
        self.strike_opening = true;
    }

    /// Withholding twin for `enable_strike_opening`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_strike_opening(&mut self) {
        self.strike_opening = false;
    }

    /// Read Faith purchase prices from the engine at the game's speed and
    /// discounts, instead of a Standard-speed literal.
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

    /// Raise the wartime army target when the enemy outweighs us, instead of
    /// counting only our own cities. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_army_target_weighs_the_enemy(&mut self) {
        self.army_target_weighs_the_enemy = true;
    }

    /// Let the strongest met major raise the army target in peacetime, so
    /// deterrence exists before any declaration. Native tournament games leave
    /// this disabled so their recorded ladders stay comparable.
    pub fn enable_peacetime_deterrence(&mut self) {
        self.peacetime_deterrence = true;
    }

    /// Credit the envoy scorer with the resources, bonuses and points a
    /// suzerainty pays, amortised over envoys still needed. See
    /// [`SUZERAIN_PRIZE`]. Off on the anchor, so a comparison against it
    /// measures the term rather than a rename.
    pub fn enable_price_the_suzerainty(&mut self) {
        self.price_the_suzerainty = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_price_the_suzerainty(&mut self) {
        self.price_the_suzerainty = false;
    }

    /// Rebuild the recon arm when every scout is gone and unexplored ground
    /// remains to chart. Native tournament games leave this disabled so their
    /// recorded ladders stay comparable.
    pub fn enable_recon_replacement(&mut self) {
        self.base.recon_replacement = true;
    }

    pub fn disable_recon_replacement(&mut self) {
        self.base.recon_replacement = false;
    }

    /// Buy one ship for a fleetless empire with unexplored water off its coast
    /// and send it exploring. See `BasicAi::naval_recon` and `naval_explorer`.
    pub fn enable_naval_recon(&mut self) {
        self.base.naval_recon = true;
    }

    pub fn disable_naval_recon(&mut self) {
        self.base.naval_recon = false;
    }

    /// Credit a settle site for the modeled appeal and yields of a revealed
    /// natural wonder's neighbouring tiles. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_wonder_ring_settle_value(&mut self) {
        self.base.wonder_ring_settle_value = true;
    }

    pub fn disable_wonder_ring_settle_value(&mut self) {
        self.base.wonder_ring_settle_value = false;
    }

    /// Keep land units out of the water: no water exploration goals, and
    /// disembark units already at sea. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_come_ashore(&mut self) {
        self.base.come_ashore = true;
    }

    pub fn disable_come_ashore(&mut self) {
        self.base.come_ashore = false;
    }

    /// Size the siege train by the wall at the target city instead of always
    /// asking for one siege unit.
    pub fn enable_siege_tracks_the_wall(&mut self) {
        self.siege_tracks_the_wall = true;
    }

    /// Price a fogged objective city from its last sighting instead of treating
    /// unseen ground as empty. Native tournament games leave this disabled so
    /// their recorded ladders stay comparable.
    pub fn enable_blind_objective_strength(&mut self) {
        self.blind_objective_strength = true;
    }

    /// Route an adaptive plan that switched to Conquest through the war
    /// production path instead of the basic governor. Native tournament games
    /// leave this disabled so their recorded ladders stay comparable.
    pub fn enable_war_economy(&mut self) {
        self.war_economy = true;
    }

    /// Keep marching newly built rear units to the campaign objective after war
    /// is declared, not only before. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_war_reinforcement(&mut self) {
        self.war_reinforcement = true;
    }

    /// Keep fighting a war the empire overwhelmingly outweighs instead of
    /// offering peace because it reads as stalled. Native tournament games
    /// leave this disabled so their recorded ladders stay comparable.
    pub fn enable_war_patience(&mut self) {
        self.war_patience = true;
    }

    /// Let a seat with an assigned victory target still counter a rival at
    /// match point. See [`Self::deny_while_targeted`].
    pub fn enable_deny_while_targeted(&mut self) {
        self.deny_while_targeted = true;
    }

    pub fn disable_deny_while_targeted(&mut self) {
        self.deny_while_targeted = false;
    }

    /// Leave an ordered Spy alone for the mission's duration instead of
    /// re-sending the order every turn. See [`Self::spy_mission_patience`].
    pub fn enable_spy_mission_patience(&mut self) {
        self.spy_mission_patience = true;
    }

    pub fn disable_spy_mission_patience(&mut self) {
        self.spy_mission_patience = false;
    }

    /// Ask the walker's own loyalty verdict on the chosen site before building
    /// a Settler for it. See [`Self::settler_site_agreement`].
    pub fn enable_settler_site_agreement(&mut self) {
        self.settler_site_agreement = true;
    }

    pub fn disable_settler_site_agreement(&mut self) {
        self.settler_site_agreement = false;
    }

    /// Count a stacked guard as protection only when it can hold, and make it
    /// stay with its settler. See [`Self::settler_guard_holds`].
    pub fn enable_settler_guard_holds(&mut self) {
        self.settler_guard_holds = true;
    }

    pub fn disable_settler_guard_holds(&mut self) {
        self.settler_guard_holds = false;
    }

    /// Count damage dealt to an enemy city or its walls as campaign progress,
    /// so a winning siege is never stalled. See [`Self::siege_is_progress`].
    pub fn enable_siege_is_progress(&mut self) {
        self.siege_is_progress = true;
    }

    pub fn disable_siege_is_progress(&mut self) {
        self.siege_is_progress = false;
    }

    /// Read the Culture and Diplomacy denial alarms off projected pressure,
    /// current value plus recent slope carried forward. See
    /// [`Self::projected_stock_denial`].
    pub fn enable_projected_stock_denial(&mut self) {
        self.projected_stock_denial = true;
    }

    pub fn disable_projected_stock_denial(&mut self) {
        self.projected_stock_denial = false;
    }

    /// Score the Diplomacy lane by when twenty Diplomatic Victory Points
    /// arrive along the Congress calendar, not by how many are banked. See
    /// [`Self::diplomatic_lane_forecast`].
    pub fn enable_diplomatic_lane_forecast(&mut self) {
        self.diplomatic_lane_forecast = true;
    }

    pub fn disable_diplomatic_lane_forecast(&mut self) {
        self.diplomatic_lane_forecast = false;
    }

    /// Count a peacetime major's army massed near one of our cities toward
    /// that city's danger. See [`Self::frontier_massing_alarm`].
    pub fn enable_frontier_massing_alarm(&mut self) {
        self.frontier_massing_alarm = true;
    }

    pub fn disable_frontier_massing_alarm(&mut self) {
        self.frontier_massing_alarm = false;
    }

    /// Read a rival's religious clock from the cities it has converted rather
    /// than from whole civilizations already lost. See
    /// [`Self::conversion_majority_alarm`].
    pub fn enable_conversion_majority_alarm(&mut self) {
        self.conversion_majority_alarm = true;
    }

    pub fn disable_conversion_majority_alarm(&mut self) {
        self.conversion_majority_alarm = false;
    }

    /// Score the Culture lane by where the two tourist curves are when the
    /// clock stops. See [`Self::culture_lane_forecast`].
    pub fn enable_culture_lane_forecast(&mut self) {
        self.culture_lane_forecast = true;
    }

    pub fn disable_culture_lane_forecast(&mut self) {
        self.culture_lane_forecast = false;
    }

    /// Read a rival's Science clock from the prerequisite chain it has
    /// climbed, not only from the launches it has made. See
    /// [`Self::science_chain_alarm`].
    pub fn enable_science_chain_alarm(&mut self) {
        self.science_chain_alarm = true;
    }

    pub fn disable_science_chain_alarm(&mut self) {
        self.science_chain_alarm = false;
    }

    /// Point the three targeted World Congress penalties at the empire the
    /// denial layer names. See [`Self::congress_counter_leader`].
    pub fn enable_congress_counter_leader(&mut self) {
        self.congress_counter_leader = true;
    }

    pub fn disable_congress_counter_leader(&mut self) {
        self.congress_counter_leader = false;
    }

    /// Raise the Culture and Diplomacy denial alarms early, since countering an
    /// accumulated stock takes many turns. See
    /// [`Self::stock_denial_lead_time`].
    pub fn enable_stock_denial_lead_time(&mut self) {
        self.stock_denial_lead_time = true;
    }

    pub fn disable_stock_denial_lead_time(&mut self) {
        self.stock_denial_lead_time = false;
    }

    /// Refuse a fresh direct war declaration once the endgame reserve leaves
    /// too few turns to capture a city. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_endgame_war_runway(&mut self) {
        self.endgame_war_runway = true;
    }

    /// Keep the campaign aimed at a city it has already damaged instead of
    /// re-targeting a fresh one each turn. A breach gets an additional value
    /// credit, but even an intact objective needs several turns of marching and
    /// bombardment before that credit exists; changing the objective every
    /// assessment strands the whole army between cities. An emergency, a
    /// changed war target, or capture immediately releases the commitment.
    /// Native tournament games leave this disabled so their recorded ladders
    /// stay comparable.
    pub fn enable_siege_commitment(&mut self) {
        self.siege_commitment = true;
    }

    /// Send a relief force at the besiegers actually damaging the city, not the
    /// enemy nearest the force. Native tournament games leave this disabled so
    /// their recorded ladders stay comparable.
    pub fn enable_relief_targets_the_siege(&mut self) {
        self.relief_targets_the_siege = true;
    }

    /// Price the enemy units remembered near an unseen objective instead of
    /// reading a fogged approach as empty. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
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

    /// Plan an engagement's attacks jointly across all units by search instead
    /// of one unit at a time in class order. Measured on `battle_bench` (1000
    /// paired fresh seeds a cell, seats swapped): combined arms +275,
    /// ranged-heavy +363, siege +206, melee-only within noise, all against the
    /// production controller the bridge extends. The whole-game gate stays
    /// inconclusive (`docs/TACTICS.md` §6), so the tournament `advanced`
    /// entrant keeps the greedy commitment rule and the deployed bridge — where
    /// the operator asked for the strongest battlefield play, not a rating —
    /// takes the search.
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

    /// Give the joint tactics search approach lines from the engine's exact
    /// reach flood, not only adjacent steps. On by default wherever the joint
    /// search runs; this pair exists so the live bridge can price the lines by
    /// taking them out (`live_without_joint_reach_lines`) and the bench can
    /// seat the geometric portfolio by name
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

    /// Price a Builder by surveying the improvement jobs it would actually do,
    /// not by a city-count quota. Evaluator arm `advanced_builder_survey`; off
    /// in production.
    pub fn enable_builder_reward_survey(&mut self) {
        self.builder_reward_survey = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_builder_reward_survey(&mut self) {
        self.builder_reward_survey = false;
    }

    /// Prefer Builder jobs on tiles citizens currently work, keeping luxury and
    /// strategic resource connections at full priority. Native opt-in gene
    /// `builder-worked-tile-priority`; off in production.
    pub fn enable_builder_worked_tile_priority(&mut self) {
        self.builder_worked_tile_priority = true;
    }

    /// The twin of `enable_builder_worked_tile_priority`.
    pub fn disable_builder_worked_tile_priority(&mut self) {
        self.builder_worked_tile_priority = false;
    }

    /// Make a Builder retreat from, and never step into, a tile a visible
    /// barbarian can capture next turn. Native opt-in gene
    /// `builder-barbarian-safety`; off in production until its targeted
    /// barbarian screen has priced the safety/tempo trade.
    pub fn enable_builder_barbarian_safety(&mut self) {
        self.builder_barbarian_safety = true;
    }

    /// The twin of `enable_builder_barbarian_safety`.
    pub fn disable_builder_barbarian_safety(&mut self) {
        self.builder_barbarian_safety = false;
    }
    /// Credit strength per production and the civilization's own unique unit
    /// when pricing military production. Evaluator arm
    /// `advanced_unit_efficiency`; off in production.
    pub fn enable_unit_cost_efficiency(&mut self) {
        self.unit_cost_efficiency = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_unit_cost_efficiency(&mut self) {
        self.unit_cost_efficiency = false;
    }

    /// Fortify any unit the planner gave nothing to do, not only one in a
    /// stand-down window. Evaluator arm `advanced_fortify_idle_units`; off in
    /// production.
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

    /// Slot the naval-production policy card while a coastal empire wants hulls
    /// it does not have. See `naval_production_policy`; entrant
    /// `advanced_maritime_splice`.
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

    /// Build a ranged defender, not a melee one, when the barbarian ring around
    /// a city is mostly shooters. See `BasicAi::barbarian_ranged_answer`;
    /// withheld by `barbarian-ranged-answer`.
    pub fn enable_barbarian_ranged_answer(&mut self) {
        self.base.enable_barbarian_ranged_answer();
    }

    pub fn disable_barbarian_ranged_answer(&mut self) {
        self.base.disable_barbarian_ranged_answer();
    }

    /// Price a fight against a barbarian below a fight against a major, since
    /// barbarians carry no war costs. See `BasicAi::barbarian_bargain`;
    /// withheld by `barbarian-bargain`.
    pub fn enable_barbarian_bargain(&mut self) {
        self.base.enable_barbarian_bargain();
    }

    pub fn disable_barbarian_bargain(&mut self) {
        self.base.disable_barbarian_bargain();
    }

    /// Walk onto a visible, undefended barbarian camp one step away and clear
    /// it for the gold bounty. See `BasicAi::barbarian_hunt`; withheld by the
    /// `barbarian-hunt` treatment.
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

    /// The barbarian seat walks onto religious units and condemns them with
    /// the movement it arrives with. See `BasicAi::barbarian_heretic_hunt`;
    /// default-ON, off on the frozen anchor, a controller treatment of the
    /// world rather than a gene of one seat.
    pub fn enable_barbarian_heretic_hunt(&mut self) {
        self.base.barbarian_heretic_hunt = true;
    }

    pub fn disable_barbarian_heretic_hunt(&mut self) {
        self.base.barbarian_heretic_hunt = false;
    }

    /// Subtract the unit-maintenance bill inside the policy counterfactual so
    /// maintenance-discount cards score above zero. See
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

    /// Plan the whole turn against a fog-redacted world and replay only the
    /// resulting orders on the real game. Belief pressure and conservative
    /// objective floors are enabled together so a hidden contact is represented
    /// as stale uncertainty rather than as an empty tile or a live omniscient
    /// unit.
    pub fn enable_fog_honest(&mut self) {
        self.fog_honest = true;
        self.fog_honest_2 = false;
        self.enable_fog_honest_belief();
    }

    /// The information contract both versions of the fog-honest major share.
    fn enable_fog_honest_belief(&mut self) {
        self.battlefront_observation = true;
        self.belief_pressure = true;
        self.blind_objective_strength = true;
        self.blind_objective_units = true;
    }

    pub fn disable_fog_honest(&mut self) {
        self.fog_honest = false;
    }

    /// Version 2 of fog-honest: the same redacted planning plus one re-plan
    /// from the real board when an order is refused. One version of a family
    /// plays, so this turns version 1 off (`docs/GENE_SCREEN.md`, *Versioning a
    /// gene*: "write it so the newer version's enable turns the older one
    /// off"); the turn entry in `AdvancedAi::take_turn` admits either flag.
    /// Opt-in gene `fog-honest-2`. See [`AdvancedAi::fog_honest_2`].
    pub fn enable_fog_honest_2(&mut self) {
        self.fog_honest = false;
        self.fog_honest_2 = true;
        self.enable_fog_honest_belief();
    }

    /// The twin of `enable_fog_honest_2`. It leaves version 1's mode where it
    /// found it, exactly as `disable_fog_honest` leaves the four flags
    /// `enable_fog_honest` sets.
    pub fn disable_fog_honest_2(&mut self) {
        self.fog_honest_2 = false;
    }

    /// Rank each district family by how much of the empire still lacks it, so
    /// Theater Squares get built.
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

    /// Break a production cost tie between museums by which great-work slots
    /// the empire can actually fill.
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

    /// Let the baseline governor build the Aqueduct and Neighborhood districts
    /// that raise the housing ceiling. See `BasicAi::housing_districts`: 78.4%
    /// of live city-turns are growth- throttled by housing, the median headroom
    /// is 0, and the Aqueduct and Neighborhood together take 1.6% of district
    /// orders.
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
    /// In a severe empire-wide Amenity crisis, pause one repeatable project for
    /// the amenity repair chain and slot Liberalism. When host-observed Amenity
    /// deficits have crossed a severe empire-wide threshold, pause one
    /// repeatable project for the concrete repair chain and let the policy deck
    /// use its direct empire-wide repair. Frozen controllers leave both
    /// ordinary orderings untouched.
    pub fn enable_amenity_project_preemption(&mut self) {
        self.amenity_project_preemption = true;
    }

    pub fn disable_amenity_project_preemption(&mut self) {
        self.amenity_project_preemption = false;
    }

    /// Price an amenity district by the building it will hold, and a regional
    /// amenity building by every city it reaches. See `amenity_district_path`.
    pub fn enable_amenity_district_path(&mut self) {
        self.amenity_district_path = true;
    }

    pub fn disable_amenity_district_path(&mut self) {
        self.amenity_district_path = false;
    }

    /// Make the settlement-gap redirect and the Settler ranking read the same
    /// city target as the baseline cascade. See `settlement_target`.
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

    /// Let a developed live city build the cheapest legal wonder whatever the
    /// grand strategy says. See `live_wonder_race`: the live seat had never
    /// ordered one, and its games end on the host's score tally.
    pub fn enable_live_wonder_race(&mut self) {
        self.live_wonder_race = true;
    }

    pub fn disable_live_wonder_race(&mut self) {
        self.live_wonder_race = false;
    }

    /// Found the third city before racing for a Great Prophet on the live seat.
    /// See `expansion_before_prophet`.
    pub fn enable_expansion_before_prophet(&mut self) {
        self.expansion_before_prophet = true;
    }

    pub fn disable_expansion_before_prophet(&mut self) {
        self.expansion_before_prophet = false;
    }

    /// Never open an elective war on the live seat; it has never converted one
    /// into a capture. See `no_elective_war`.
    pub fn enable_no_elective_war(&mut self) {
        self.no_elective_war = true;
    }

    pub fn disable_no_elective_war(&mut self) {
        self.no_elective_war = false;
    }

    /// Answer a Science or score leader by racing them in that lane instead of
    /// declaring war on them. See `counter_in_lane`.
    pub fn enable_counter_in_lane(&mut self) {
        self.counter_in_lane = true;
    }

    pub fn disable_counter_in_lane(&mut self) {
        self.counter_in_lane = false;
    }

    /// Raise the wanted city count one rung per game era instead of every
    /// ninety standard turns. See `era_paced_expansion`.
    pub fn enable_era_paced_expansion(&mut self) {
        self.era_paced_expansion = true;
    }

    pub fn disable_era_paced_expansion(&mut self) {
        self.era_paced_expansion = false;
    }

    /// Expand to all the land the empire can hold at pipeline pace, instead of
    /// on an era clock. Sets both halves: the strategic governor's target,
    /// window and pipeline, and `BasicAi::pick_item`'s pipeline and window.
    pub fn enable_land_grab(&mut self) {
        self.land_grab = true;
        self.base.land_grab = true;
    }

    pub fn disable_land_grab(&mut self) {
        self.land_grab = false;
        self.base.land_grab = false;
    }

    /// Take the pantheon that founds cities and keep the God-King card until
    /// its Faith is paid. Sets both halves: the strategic portfolio's God-King
    /// want, and `BasicAi`'s pantheon prefix.
    pub fn enable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = true;
        self.base.expansion_pantheon = true;
    }

    pub fn disable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = false;
        self.base.expansion_pantheon = false;
    }

    /// Price the Ancestral Hall for its Settler production bonus and the free
    /// Builder in every new city.
    pub fn enable_expansion_hall(&mut self) {
        self.expansion_hall = true;
    }

    pub fn disable_expansion_hall(&mut self) {
        self.expansion_hall = false;
    }

    /// Hold the opening book's Settler slot until the host's population floor
    /// instead of burning it.
    pub fn enable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = true;
        self.base.opening_settler_waits = true;
    }

    pub fn disable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = false;
        self.base.opening_settler_waits = false;
    }

    /// Price a point of culture at what the Science lane pays for a point of
    /// science. See `tally_culture`.
    pub fn enable_tally_culture(&mut self) {
        self.tally_culture = true;
    }

    pub fn disable_tally_culture(&mut self) {
        self.tally_culture = false;
    }

    /// Make a Theater Square owe its Amphitheater, Museum and Broadcast Center
    /// the way a Campus owes its buildings. See `culture_building_debt`.
    pub fn enable_culture_building_debt(&mut self) {
        self.culture_building_debt = true;
    }

    pub fn disable_culture_building_debt(&mut self) {
        self.culture_building_debt = false;
    }

    /// Make every specialty district owe the buildings inside it, whatever
    /// victory lane the empire is playing. See `district_building_chain`.
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

    /// Pay a coverage bonus for a Theater Square in every city that lacks one,
    /// as the Campus already gets. See `culture_coverage`.
    pub fn enable_culture_coverage(&mut self) {
        self.culture_coverage = true;
    }

    pub fn disable_culture_coverage(&mut self) {
        self.culture_coverage = false;
    }

    /// Bank an envoy instead of spending it on a placement whose score is
    /// negative. See `bank_envoys`.
    pub fn enable_bank_envoys(&mut self) {
        self.bank_envoys = true;
        self.base.enable_bank_envoys();
    }

    pub fn disable_bank_envoys(&mut self) {
        self.bank_envoys = false;
        self.base.disable_bank_envoys();
    }

    /// Refuse to found a city beyond the empire's Loyalty reach on ground the
    /// seat has not explored. See `frontier_loyalty`.
    pub fn enable_frontier_loyalty(&mut self) {
        self.frontier_loyalty = true;
    }

    pub fn disable_frontier_loyalty(&mut self) {
        self.frontier_loyalty = false;
    }

    /// Keep a settler target dropped for danger out of the ranking for several
    /// turns instead of re-picking it immediately. See
    /// `settler_target_hysteresis`.
    pub fn enable_settler_target_hysteresis(&mut self) {
        self.settler_target_hysteresis = true;
    }

    pub fn disable_settler_target_hysteresis(&mut self) {
        self.settler_target_hysteresis = false;
    }

    /// Send a Settler to the best safe alternative site when a visible threat
    /// blocks its route. See `settler_threat_detour`.
    pub fn enable_settler_threat_detour(&mut self) {
        self.settler_threat_detour = true;
    }

    pub fn disable_settler_threat_detour(&mut self) {
        self.settler_threat_detour = false;
    }

    /// Price a Settler's walk per turn, rising the longer it has walked, so it
    /// settles sooner. Price a Settler's walk in turns, each turn dearer the
    /// longer the Settler has already been walking, so expansion founds sooner
    /// without giving up a site good enough to pay for its walk. See
    /// `settle_sooner`.
    pub fn enable_settle_sooner(&mut self) {
        self.settle_sooner = true;
    }

    pub fn disable_settle_sooner(&mut self) {
        self.settle_sooner = false;
    }

    /// Rank a settle site by the future city sites it leaves room for as well
    /// as its own ground. Rank a settle site by the cities it leaves room for
    /// as well as its own ground, so a Settler stops taking the one plot in a
    /// pocket that would have held two. See `settle_plan_ahead`.
    pub fn enable_settle_plan_ahead(&mut self) {
        self.settle_plan_ahead = true;
    }

    pub fn disable_settle_plan_ahead(&mut self) {
        self.settle_plan_ahead = false;
    }

    /// Let any civilization build wonders on merit by pricing the fifteen score
    /// points a finished wonder pays. A wonder lane any civilization can reach
    /// on merit: the `Item::Wonder` arm learns the fifteen points
    /// `Game::score_parts` pays for a finished wonder, under a density bar and
    /// the live race's own development guards. See
    /// `AdvancedAi::wonder_score_tally`.
    pub fn enable_wonder_score_tally(&mut self) {
        self.wonder_score_tally = true;
    }

    /// Withhold `enable_wonder_score_tally`.
    pub fn disable_wonder_score_tally(&mut self) {
        self.wonder_score_tally = false;
    }

    /// Let banked Faith or Gold patronize any affordable Great Person on the
    /// score-tally seat, not only nearly-earned ones. See `tally_great_people`.
    pub fn enable_tally_great_people(&mut self) {
        self.tally_great_people = true;
    }

    pub fn disable_tally_great_people(&mut self) {
        self.tally_great_people = false;
    }

    /// Make a repeatable district project wait behind the science and
    /// production buildings the city can already build. See
    /// `buildings_before_projects`.
    pub fn enable_buildings_before_projects(&mut self) {
        self.buildings_before_projects = true;
    }

    pub fn disable_buildings_before_projects(&mut self) {
        self.buildings_before_projects = false;
    }

    /// Walk onto any capturable civilian within reach, and always take back a
    /// Settler the barbarians hold. See `BasicAi::civilian_rescue`.
    pub fn enable_civilian_rescue(&mut self) {
        self.base.civilian_rescue = true;
    }

    pub fn disable_civilian_rescue(&mut self) {
        self.base.civilian_rescue = false;
    }

    /// Capture a visible barbarian-held Settler or Scout within one-turn reach
    /// before healing, retreating or any other move. This native opt-in is
    /// screened as `barbarian-capture-priority`; see
    /// [`BasicAi::barbarian_capture_priority`](crate::ai::BasicAi::barbarian_capture_priority).
    pub fn enable_barbarian_capture_priority(&mut self) {
        self.base.barbarian_capture_priority = true;
    }

    /// The twin of `enable_barbarian_capture_priority`.
    pub fn disable_barbarian_capture_priority(&mut self) {
        self.base.barbarian_capture_priority = false;
    }

    /// Withdraw a unit that one enemy blow could kill to safe healing ground,
    /// and leave when threatened again. See `BasicAi::one_shot_recovery`.
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

    /// Stop pricing a barbarian Scout as a threat, since it never attacks or
    /// captures; settlers and scouts ignore it. See
    /// `barbarian_scouts_are_scouts`.
    pub fn enable_barbarian_scouts_are_scouts(&mut self) {
        self.barbarian_scouts_are_scouts = true;
    }

    pub fn disable_barbarian_scouts_are_scouts(&mut self) {
        self.barbarian_scouts_are_scouts = false;
    }

    /// Skip a space race or Manhattan Project that cannot finish before the
    /// turn limit ends the game. See `score_horizon`.
    pub fn enable_score_horizon(&mut self) {
        self.score_horizon = true;
    }

    pub fn disable_score_horizon(&mut self) {
        self.score_horizon = false;
    }

    /// Price a wonder's effects in the victory lane's currency and build the
    /// ones that lane needs. See `AdvancedAi::strategic_wonder_value`.
    pub fn enable_strategic_wonders(&mut self) {
        self.strategic_wonders = true;
    }

    /// Withholding twin for `enable_strategic_wonders`, so the arm can be
    /// priced by taking this one treatment out. See `LIVE_TREATMENTS`.
    pub fn disable_strategic_wonders(&mut self) {
        self.strategic_wonders = false;
    }

    /// Let only one city at a time claim the 3,000-point first Spaceport bonus,
    /// instead of every city at once. See `one_launch_pad`.
    pub fn enable_one_launch_pad(&mut self) {
        self.one_launch_pad = true;
    }

    pub fn disable_one_launch_pad(&mut self) {
        self.one_launch_pad = false;
    }
    /// Aim research at the technology that raises the housing ceiling while
    /// housing is throttling growth.
    pub fn enable_housing_research(&mut self) {
        self.housing_research = true;
    }

    pub fn disable_housing_research(&mut self) {
        self.housing_research = false;
    }

    /// Rank loyalty emergencies by turns until the city flips rather than by
    /// its current loyalty level. Native tournament games leave this disabled.
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

    /// Buy a soldier with Faith only when the treasury can also pay its Gold
    /// upkeep.
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
    /// Choose the pantheon by what it would pay on the tiles the empire owns,
    /// not from a fixed order. Reachable as `advanced_pantheon_board`; see
    /// `BasicAi::pantheon_reads_the_board`.
    pub fn enable_pantheon_board(&mut self) {
        self.base.pantheon_reads_the_board = true;
    }

    /// The off toggle, so the registry row has both directions.
    pub fn disable_pantheon_board(&mut self) {
        self.base.pantheon_reads_the_board = false;
    }

    /// Score the World Congress ballot for the victory the empire is racing
    /// while its plan is still Expansion. Score the World Congress ballot —
    /// which outcome and target this seat names — for the victory the empire is
    /// actually racing rather than for an expansion posture that has no lane.
    /// See `advanced/victory_lane.rs`. The Favor stake behind the ballot is
    /// `lane-congress-favor`, a separate gene. Off everywhere by default;
    /// opt-in gene `lane-congress-ballot`.
    pub fn enable_lane_congress_ballot(&mut self) {
        self.lane_congress_ballot = true;
    }

    /// The twin of `enable_lane_congress_ballot`.
    pub fn disable_lane_congress_ballot(&mut self) {
        self.lane_congress_ballot = false;
    }

    /// Stake Favor behind a World Congress ballot for the victory the empire is
    /// racing while its plan is Expansion. The other half of what
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

    /// Rank Great Person classes and project points by the victory the empire
    /// is racing, even during a war. Rank Great Person classes, and the Great
    /// Person points a project earns, by the victory the empire is actually
    /// racing rather than by a war it is fighting. See
    /// `advanced/victory_lane.rs` — this is the one gene there that overrides a
    /// Conquest plan, and the fires-check that chose that scope is in
    /// `docs/VICTORY_GENES.md` §7. `Recovery` still keeps its own strategy. Off
    /// everywhere by default; opt-in gene `lane-great-people`.
    pub fn enable_lane_great_people(&mut self) {
        self.lane_great_people = true;
    }

    /// The twin of `enable_lane_great_people`.
    pub fn disable_lane_great_people(&mut self) {
        self.lane_great_people = false;
    }

    /// Choose policy cards for the victory the empire is racing while its plan
    /// is still Expansion. See `advanced/victory_lane.rs`. Off everywhere by
    /// default; opt-in gene `lane-policy-deck`.
    pub fn enable_lane_policy_deck(&mut self) {
        self.lane_policy_deck = true;
    }

    /// The twin of `enable_lane_policy_deck`.
    pub fn disable_lane_policy_deck(&mut self) {
        self.lane_policy_deck = false;
    }

    /// Run the Culture lane's Faith purchases, Naturalists and Rock Bands, for
    /// an empire racing Culture under an Expansion plan. Run the Culture lane's
    /// Faith pass — the Naturalist that founds a National Park, the touring
    /// Rock Bands — and size its reserve, for an empire racing Culture whose
    /// plan has not named the lane. See `advanced/victory_lane.rs`; `Recovery`
    /// still refuses. Off everywhere by default; opt-in gene
    /// `lane-culture-spending`.
    pub fn enable_lane_culture_spending(&mut self) {
        self.lane_culture_spending = true;
    }

    /// The twin of `enable_lane_culture_spending`.
    pub fn disable_lane_culture_spending(&mut self) {
        self.lane_culture_spending = false;
    }

    /// Open the Spaceport and launch pass for an empire racing Science even
    /// while its plan is still Expansion. Treat an empire racing Science as a
    /// Science seat throughout the space race: the pad count, the city a launch
    /// project may claim and the city a pad may be sited in all read the race
    /// rather than an explicitly assigned target, and the pass opens at all.
    /// `score_horizon` still refuses a race that cannot finish. See
    /// `advanced/victory_lane.rs`. Off everywhere by default; opt-in gene
    /// `lane-space-race`.
    pub fn enable_lane_space_race(&mut self) {
        self.lane_space_race = true;
    }

    /// The twin of `enable_lane_space_race`.
    pub fn disable_lane_space_race(&mut self) {
        self.lane_space_race = false;
    }

    /// Price first place in a scored competition by the Diplomatic Victory
    /// Points it pays. See `advanced/victory_lane.rs`. Off everywhere by
    /// default; opt-in gene `competition-victory-points`.
    pub fn enable_competition_victory_points(&mut self) {
        self.competition_victory_points = true;
    }

    /// The twin of `enable_competition_victory_points`.
    pub fn disable_competition_victory_points(&mut self) {
        self.competition_victory_points = false;
    }

    /// Cast the free vote on an already-decided resolution's winner to bank the
    /// Diplomatic Victory Point for predicting it. Answer a World Congress
    /// resolution that is already decided with the one free vote on its settled
    /// winner, taking the Diplomatic Victory Point for an exact prediction and
    /// staking nothing.
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

    /// Back the counter-victory ballot with every Favor the treasury can spare,
    /// since a losing vote is refunded. Back a ballot aimed at the empire
    /// closest to a victory with everything the treasury can spare — a losing
    /// vote is refunded in full, so an opposition that fails costs no Favor.
    /// Gene `congress-counter-votes`.
    pub fn enable_congress_counter_votes(&mut self) {
        self.congress_counter_votes = true;
    }

    /// The twin of `enable_congress_counter_votes`.
    pub fn disable_congress_counter_votes(&mut self) {
        self.congress_counter_votes = false;
    }

    /// Value the Consulate, Chancery and Diplomatic Quarter by the envoys their
    /// influence can produce before the turn limit. Value the infrastructure
    /// that produces city-state influence: the Consulate and Chancery's
    /// per-turn influence becomes the envoys it can produce before the turn
    /// limit, and a first Diplomatic Quarter sees part of the Consulate stream
    /// it unlocks. Gene `envoy-infrastructure`.
    pub fn enable_envoy_infrastructure(&mut self) {
        self.envoy_infrastructure = true;
    }

    /// The twin of `enable_envoy_infrastructure`.
    pub fn disable_envoy_infrastructure(&mut self) {
        self.envoy_infrastructure = false;
    }

    /// Beeline Advanced Flight, build an Aerodrome and bombers, and take the
    /// appointed city with cavalry behind them. See
    /// [`AdvancedAi::maintain_air_surge`] and `advanced/air_surge.rs`. Off
    /// everywhere by default; opt-in gene `air-surge`.
    pub fn enable_air_surge(&mut self) {
        self.air_surge = true;
    }

    /// The twin of `enable_air_surge`.
    pub fn disable_air_surge(&mut self) {
        self.air_surge = false;
    }

    /// Let a force finish a defender together with a friendly volley, without
    /// the rest of the tactical-strategy bundle. See
    /// [`AdvancedAi::coordinated_finish`].
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

    /// Plan a city's districts, their plots and tile purchases jointly,
    /// reserving plots in rings one to three. The city plans its districts,
    /// sites and tile buys together: wished districts get jointly assigned,
    /// reserved plots over rings 1-3, and the tile a very valuable site needs
    /// is bought. See [`AdvancedAi::district_planning`]. Opt-in gene
    /// `district-planning`. (Filed here rather than under a marker: the
    /// append-point check reads a line's first identifier, which for any `pub
    /// fn` is `pub`.)
    pub fn enable_district_planning(&mut self) {
        self.district_planning = true;
    }

    /// The twin of `enable_district_planning`.
    pub fn disable_district_planning(&mut self) {
        self.district_planning = false;
    }

    /// Let a Missionary on its last charge explore nearby fog for a few turns
    /// before spending it. A Missionary on its last charge explores the fog
    /// within ten tiles for up to twelve turns before spending it, unless a
    /// city of ours is slipping or an untouched city stands beside it. See
    /// [`AdvancedAi::missionary_last_charge_explores`]. Opt-in gene
    /// `missionary-last-charge-explores`. (Filed here rather than under a
    /// marker: the append-point check reads a line's first identifier.)
    pub fn enable_missionary_last_charge_explores(&mut self) {
        self.missionary_last_charge_explores = true;
    }

    /// The twin of `enable_missionary_last_charge_explores`.
    pub fn disable_missionary_last_charge_explores(&mut self) {
        self.missionary_last_charge_explores = false;
    }

    /// Keep religious units out of every tile a visible barbarian raider can
    /// reach next turn. A religious unit steps out of the tiles a visible
    /// barbarian raider can reach next turn, and never steps into them on the
    /// way to anything, holding when no safe step makes progress. See
    /// [`AdvancedAi::missionary_evades_raiders`]. Opt-in gene
    /// `missionary-evades-raiders`.
    pub fn enable_missionary_evades_raiders(&mut self) {
        self.missionary_evades_raiders = true;
    }

    /// The twin of `enable_missionary_evades_raiders`.
    pub fn disable_missionary_evades_raiders(&mut self) {
        self.missionary_evades_raiders = false;
    }

    /// Scale religious defence with a rival's progress toward religious
    /// victory, and walk the Inquisitor to the heresy. The religious defence
    /// grows with how much of a rival's religious victory is already done —
    /// every civilization is a veto on it — naming and targeting the stakes
    /// faith from half a victory and spending on it from match point; the
    /// Inquisitor walks to the heresy instead of spending its charges where it
    /// was bought. See [`AdvancedAi::religious_veto_defence`]. Opt-in gene
    /// `religious-veto-defence`.
    pub fn enable_religious_veto_defence(&mut self) {
        self.religious_veto_defence = true;
    }

    /// The twin of `enable_religious_veto_defence`.
    pub fn disable_religious_veto_defence(&mut self) {
        self.religious_veto_defence = false;
    }

    /// Pick the trade quote with the best net value to us instead of the most
    /// balanced exchange. A quote is chosen by our own net value instead of the
    /// most balanced exchange on the board (`min(our gain, their gain)`), which
    /// threw away the ordering `Game::quick_deals` already produced by our
    /// gain. See `BasicAi::deals_for_our_gain`. Opt-in gene
    /// `deals-for-our-gain`.
    pub fn enable_deals_for_our_gain(&mut self) {
        self.base.deals_for_our_gain = true;
    }

    /// The twin of `enable_deals_for_our_gain`.
    pub fn disable_deals_for_our_gain(&mut self) {
        self.base.deals_for_our_gain = false;
    }

    /// Price a trade quote at the counterparty's walk-away point less two Gold,
    /// falling back to the midpoint if refused. The chosen quote's Gold is
    /// moved to the counterparty's walk-away less two Gold — a sale asks for
    /// more, a purchase pays less — where the shipped quote split the surplus
    /// down the middle; the midpoint quote stays the fallback. See
    /// `BasicAi::deals_at_the_ceiling`. Opt-in gene `deals-at-the-ceiling`.
    pub fn enable_deals_at_the_ceiling(&mut self) {
        self.base.deals_at_the_ceiling = true;
    }

    /// The twin of `enable_deals_at_the_ceiling`.
    pub fn disable_deals_at_the_ceiling(&mut self) {
        self.base.deals_at_the_ceiling = false;
    }

    /// Stop bundling free one-way Open Borders into friendship and alliance
    /// proposals; sell passage through the quote lane. Friendship and alliance
    /// proposals no longer bundle one-way Open Borders, which every ask handed
    /// out for nothing once Early Empire was in; passage is sold through the
    /// quote lane. See `BasicAi::no_free_passage`. Opt-in gene
    /// `no-free-passage`.
    pub fn enable_no_free_passage(&mut self) {
        self.base.no_free_passage = true;
    }

    /// The twin of `enable_no_free_passage`.
    pub fn disable_no_free_passage(&mut self) {
        self.base.no_free_passage = false;
    }
    /// Fight one campaign front at a time, seeking peace with every other major
    /// and once the front turns against us. Fight one war at a time: keep one
    /// campaign front and sue every other major for peace, hold a fresh
    /// declaration while a war is on, press the front while a city is breaking
    /// or tiles are in reach to pillage, and offer peace once the exchange has
    /// run against us for long enough with nothing left to take. See
    /// [`AdvancedAi::one_war_observe`]. Opt-in gene `one-war-at-a-time`.
    pub fn enable_one_war_at_a_time(&mut self) {
        self.one_war_at_a_time = true;
    }

    /// The twin of `enable_one_war_at_a_time`.
    pub fn disable_one_war_at_a_time(&mut self) {
        self.one_war_at_a_time = false;
    }

    /// Add a city-state's proximity and hostile suzerain to the envoy score,
    /// amortised over envoys the flip needs. A city-state's place enters the
    /// envoy score: up to ninety for one on our border, two hundred more when
    /// its sitting suzerain is at war with us, amortised over the envoys the
    /// flip still needs. [`AdvancedAi::flip_nearby_city_states`]. Opt-in gene
    /// `flip-nearby-city-states`; see `advanced/field_craft.rs`.
    pub fn enable_flip_nearby_city_states(&mut self) {
        self.flip_nearby_city_states = true;
    }

    /// The twin of `enable_flip_nearby_city_states`.
    pub fn disable_flip_nearby_city_states(&mut self) {
        self.flip_nearby_city_states = false;
    }

    /// Let a unit at or below 65 health pillage a healing improvement on or
    /// beside its tile before retreating. A unit at or below 65 health pillages
    /// a heal-type improvement it stands on, or steps one tile onto one and
    /// pillages it, before the recovery path walks it home.
    /// [`AdvancedAi::pillage_to_heal`]. Opt-in gene `pillage-to-heal`; see
    /// `advanced/field_craft.rs`.
    pub fn enable_pillage_to_heal(&mut self) {
        self.pillage_to_heal = true;
    }

    /// The twin of `enable_pillage_to_heal`.
    pub fn disable_pillage_to_heal(&mut self) {
        self.pillage_to_heal = false;
    }

    /// Let a ranged unit inside melee reach step to a safer firing tile and
    /// shoot the threatening body. A ranged unit inside a hostile melee body's
    /// reach steps to a firing tile inside strictly fewer hostile envelopes and
    /// fires at that body, in war and against barbarians. Shooters exert no
    /// zone of control, so the step fires behind a melee friend's zone, across
    /// a river or onto ground the body cannot enter and swing from — never in
    /// the open. [`AdvancedAi::shoot_and_scoot`]. Opt-in gene
    /// `shoot-and-scoot`; see `advanced/field_craft.rs`.
    pub fn enable_shoot_and_scoot(&mut self) {
        self.shoot_and_scoot = true;
    }

    /// The twin of `enable_shoot_and_scoot`.
    pub fn disable_shoot_and_scoot(&mut self) {
        self.shoot_and_scoot = false;
    }

    /// Stand an idle melee unit where its zone of control shields our shooters
    /// and wounded from the most enemy reaches. A melee unit the attack scan
    /// found nothing for stands where its zone of control takes the most enemy
    /// reaches off our shooters and wounded, read exactly off `attack_reach`,
    /// and holds only while the stand is load-bearing.
    /// [`AdvancedAi::zoc_screen`]. Opt-in gene `zoc-screen`; see
    /// `advanced/field_craft.rs`.
    pub fn enable_zoc_screen(&mut self) {
        self.zoc_screen = true;
    }

    /// The twin of `enable_zoc_screen`.
    pub fn disable_zoc_screen(&mut self) {
        self.zoc_screen = false;
    }

    /// Appraise weaker neighbours, plan to take one to three holdable cities
    /// the army can afford, and launch when staged. Appraise the neighbours on
    /// public military power and tech count, plan the holdable city — or two,
    /// or three — of a weaker one that the field army can take with units to
    /// spare, and launch when the staging ring carries that bill. The plan aims
    /// the campaign, replaces the empire ratio at the declaration with the
    /// city's own requirement, and offers peace once every planned city is
    /// taken. [`AdvancedAi::city_campaign`]. Opt-in gene `city-campaign`; see
    /// `advanced/city_campaign.rs`.
    pub fn enable_city_campaign(&mut self) {
        self.city_campaign = true;
    }

    /// The twin of `enable_city_campaign`.
    pub fn disable_city_campaign(&mut self) {
        self.city_campaign = false;
    }

    /// Let a soldier at war pillage the tile it stands on with movement its
    /// march does not use. A soldier at war standing on a tile it may pillage
    /// spends the movement its march does not use on the pillage — waiting with
    /// its force, unable to move on, or in the siege ring with its blow
    /// declined — and never a tile of advance.
    /// [`AdvancedAi::campaign_pillage`]. Opt-in gene `campaign-pillage`; see
    /// `advanced/city_campaign.rs`.
    pub fn enable_campaign_pillage(&mut self) {
        self.campaign_pillage = true;
    }

    /// The twin of `enable_campaign_pillage`.
    pub fn disable_campaign_pillage(&mut self) {
        self.campaign_pillage = false;
    }

    /// From mid-game commit an adaptive seat to the victory lane it leads the
    /// field in, instead of re-picking each turn. From the midpoint of the game
    /// an adaptive seat commits to the victory lane it leads the field in and
    /// holds that plan, in place of the per-turn best-progress pick. See
    /// [`AdvancedAi::maintain_lane_commit`]. Opt-in gene `lane-commit`. (Filed
    /// here rather than under a marker: the append-point check reads a line's
    /// first identifier.)
    pub fn enable_lane_commit(&mut self) {
        self.lane_commit = true;
    }

    /// The twin of `enable_lane_commit`.
    pub fn disable_lane_commit(&mut self) {
        self.lane_commit = false;
    }

    /// Reserve the first empty trade route slot ahead of ordinary production in
    /// any city that can start a safe route. A barbarian alarm at a remote city
    /// no longer vetoes the whole empire. See
    /// `BasicAi::solvency_first_trade_slot`; opt-in gene
    /// `solvency-first-trade-slot`. Filed here rather than under a marker: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_solvency_first_trade_slot(&mut self) {
        self.base.solvency_first_trade_slot = true;
    }

    /// The twin of `enable_solvency_first_trade_slot`.
    pub fn disable_solvency_first_trade_slot(&mut self) {
        self.base.solvency_first_trade_slot = false;
    }

    /// Give early Settler pipeline slots to cities that finish fastest and hold
    /// distinct reachable claim sites. Opt-in gene
    /// `settler-factory-coordination`. Filed here rather than under a marker:
    /// the append-point check reads a method line's first identifier.
    pub fn enable_settler_factory_coordination(&mut self) {
        self.settler_factory_coordination = true;
    }

    /// The twin of `enable_settler_factory_coordination`.
    pub fn disable_settler_factory_coordination(&mut self) {
        self.settler_factory_coordination = false;
    }

    /// Ignore nearby barbarian ships that cannot land a meaningful blow, while
    /// still allowing ranged shots at them. Opt-in gene `naval-threat-triage`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_naval_threat_triage(&mut self) {
        self.base.naval_threat_triage = true;
    }

    /// The twin of `enable_naval_threat_triage`.
    pub fn disable_naval_threat_triage(&mut self) {
        self.base.naval_threat_triage = false;
    }

    /// Block a seen rival Settler with up to four nearby units standing on its
    /// likeliest paths to slow its founding. A seen rival Settler near our
    /// cities is screened: up to four of our nearby land units, recon first,
    /// take the stands that add the most expected steps to its likeliest walks
    /// — a tile a foreign unit holds cannot be entered at peace — and hold them
    /// while the plan names them. [`AdvancedAi::settler_screen`]. Opt-in gene
    /// `settler-screen`; see `advanced/recon_disruption.rs`.
    pub fn enable_settler_screen(&mut self) {
        self.settler_screen = true;
    }

    /// The twin of `enable_settler_screen`.
    pub fn disable_settler_screen(&mut self) {
        self.settler_screen = false;
    }

    /// Station an idle recon unit on the chokepoint tile of the land route
    /// toward a neighbour, or watch their border. A recon unit with nothing
    /// left to explore holds the pass toward a neighbour — the first tile
    /// outside their borders whose removal cuts the land walk between the two
    /// capitals — or, when no single tile cuts it, watches the border tile that
    /// walk leaves their ground by. [`AdvancedAi::pass_picket`]. Opt-in gene
    /// `pass-picket`; see `advanced/recon_disruption.rs`.
    pub fn enable_pass_picket(&mut self) {
        self.pass_picket = true;
    }

    /// The twin of `enable_pass_picket`.
    pub fn disable_pass_picket(&mut self) {
        self.pass_picket = false;
    }

    /// Convert the first six Standard-speed turns after a surprise war is
    /// declared against us into a bounded defensive mobilization. Off
    /// everywhere by default; opt-in gene `surprise-war-mobilization`. Filed
    /// here rather than under a marker: the append-point check reads a method
    /// line's first identifier.
    pub fn enable_surprise_war_mobilization(&mut self) {
        self.surprise_war_mobilization = true;
    }

    /// The twin of `enable_surprise_war_mobilization`.
    pub fn disable_surprise_war_mobilization(&mut self) {
        self.surprise_war_mobilization = false;
    }

    /// When the empire leads the field in science, beeline the space-race chain,
    /// build launch-city production and race two pads early.
    /// Opt-in gene `science-victory-drive`; see `advanced/science_victory_drive.rs`
    /// for the live runs that led science and never launched, and what each
    /// lever does. Filed above the markers: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_science_victory_drive(&mut self) {
        self.science_victory_drive = true;
    }

    /// The twin of `enable_science_victory_drive`.
    pub fn disable_science_victory_drive(&mut self) {
        self.science_victory_drive = false;
    }

    // Append points, one per name range: a new treatment goes under the range
    // its own name falls in, so that two of them do not append to one line.
    // The rule, the measurement behind it and the check that enforces it are
    // on `pub struct AdvancedAi` in `src/ai/advanced.rs`.

    // ---- append: a-b ------------------------------------------------

    // ---- append: c-d ------------------------------------------------

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
