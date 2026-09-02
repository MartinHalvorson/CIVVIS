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

    /// An amenity crisis repair is bought with Gold when the treasury covers
    /// it, so the Science plan's repeatable project keeps its queue; only a
    /// district or an unaffordable building still pauses the project.
    ///
    /// Version 2 of `amenity_project_preemption`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `amenity-project-preemption-2`. See `AdvancedAi::amenity_project_preemption_2`.
    pub fn enable_amenity_project_preemption_2(&mut self) {
        self.amenity_project_preemption = false;
        self.amenity_project_preemption_2 = true;
    }

    pub fn disable_amenity_project_preemption_2(&mut self) {
        self.amenity_project_preemption_2 = false;
    }

    /// A settle site with a plot in its first three rings that could host a
    /// Campus at raw Science adjacency 4 is worth 15% more, so the
    /// multiplier's threshold is bought where it is decided — at city siting
    /// — and not only priced at the district.
    ///
    /// Version 2 of `campus_adjacency_threshold`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `campus-adjacency-threshold-2`. See `AdvancedAi::campus_adjacency_threshold_2`.
    pub fn enable_campus_adjacency_threshold_2(&mut self) {
        self.campus_adjacency_threshold = false;
        self.campus_adjacency_threshold_2 = true;
    }

    pub fn disable_campus_adjacency_threshold_2(&mut self) {
        self.campus_adjacency_threshold_2 = false;
    }

    /// The district coverage term falls to a quarter of the bred weight for a
    /// family every city already holds, not half, pushing the lever further
    /// in the direction that paid score share.
    ///
    /// Version 2 of `district_coverage`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `district-coverage-2`. See `BasicAi::district_coverage_2`.
    pub fn enable_district_coverage_2(&mut self) {
        self.base.district_coverage = false;
        self.base.district_coverage_2 = true;
    }

    pub fn disable_district_coverage_2(&mut self) {
        self.base.district_coverage_2 = false;
    }

    /// A founder may hold one Guru whenever any of its religious units is
    /// damaged, not only while the home is under conversion pressure, so the
    /// corps out spreading gets the heal too.
    ///
    /// Opt-in gene `guru-heals-the-corps-2`. See
    /// [`AdvancedAi::guru_heals_the_corps_2`].
    pub fn enable_guru_heals_the_corps_2(&mut self) {
        self.guru_heals_the_corps_2 = true;
    }

    pub fn disable_guru_heals_the_corps_2(&mut self) {
        self.guru_heals_the_corps_2 = false;
    }

    /// A slipping city with no Holy Site claims one only when Gold can buy
    /// it; the district is never put at the front of the city's own queue.
    ///
    /// Version 2 of `holy_site_where_the_threat_is`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `holy-site-where-the-threat-is-2`. See `AdvancedAi::holy_site_where_the_threat_is_2`.
    pub fn enable_holy_site_where_the_threat_is_2(&mut self) {
        self.holy_site_where_the_threat_is = false;
        self.holy_site_where_the_threat_is_2 = true;
    }

    pub fn disable_holy_site_where_the_threat_is_2(&mut self) {
        self.holy_site_where_the_threat_is_2 = false;
    }

    /// Two peacetime naval eyes instead of one while unseen water remains, so
    /// the second coast is charted while the first Galley is still out.
    ///
    /// Version 2 of `naval_recon`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `naval-recon-2`. See `BasicAi::naval_recon_2`.
    pub fn enable_naval_recon_2(&mut self) {
        self.base.naval_recon = false;
        self.base.naval_recon_2 = true;
    }

    pub fn disable_naval_recon_2(&mut self) {
        self.base.naval_recon_2 = false;
    }

    /// Version 3 of `naval-recon`: let a simultaneously missing land scout
    /// take the idle queue before the one peacetime sea scout. One version per
    /// family is active in a screen.
    pub fn enable_naval_recon_3(&mut self) {
        self.base.naval_recon = false;
        self.base.naval_recon_2 = false;
        self.base.naval_recon_3 = true;
    }

    /// The twin of `enable_naval_recon_3`.
    pub fn disable_naval_recon_3(&mut self) {
        self.base.naval_recon_3 = false;
    }

    /// A building whose powered half would be switched on the day it stands —
    /// the city is powered and stays powered with the building's own demand —
    /// is priced with that half, so the Lab, Stock Exchange and Factory in
    /// already-powered cities stop being bought without it.
    ///
    /// Version 2 of `power_the_laboratory`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `power-the-laboratory-2`. See `AdvancedAi::power_the_laboratory_2`.
    pub fn enable_power_the_laboratory_2(&mut self) {
        self.power_the_laboratory = false;
        self.power_the_laboratory_2 = true;
    }

    pub fn disable_power_the_laboratory_2(&mut self) {
        self.power_the_laboratory_2 = false;
    }

    /// A settler's bound guard is no protection when two visible hostiles
    /// that can reach the tile each match its strength, not only when one is
    /// 1.5× it.
    ///
    /// Version 2 of `settler_guard_holds`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `settler-guard-holds-2`. See `AdvancedAi::settler_guard_holds_2`.
    pub fn enable_settler_guard_holds_2(&mut self) {
        self.settler_guard_holds = false;
        self.settler_guard_holds_2 = true;
    }

    pub fn disable_settler_guard_holds_2(&mut self) {
        self.settler_guard_holds_2 = false;
    }

    /// A settle site one settler drops for an empire-wide invalidation is set
    /// aside for every own settler for the same window. Route safety remains
    /// local, so an escorted Settler can still use a site an unguarded one
    /// cannot approach.
    ///
    /// Version 2 of `settler_target_hysteresis`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `settler-target-hysteresis-2`. See `AdvancedAi::settler_target_hysteresis_2`.
    pub fn enable_settler_target_hysteresis_2(&mut self) {
        self.settler_target_hysteresis = false;
        self.settler_target_hysteresis_2 = true;
    }

    pub fn disable_settler_target_hysteresis_2(&mut self) {
        self.settler_target_hysteresis_2 = false;
    }

    /// The first turn an own land unit stands within two tiles of an at-war
    /// city resets the war-fatigue clock once for that city, so a campaign
    /// still walking to its target is not offered away as stalled.
    ///
    /// Opt-in gene `siege-is-progress-2`. See `AdvancedAi::siege_is_progress_2`.
    pub fn enable_siege_is_progress_2(&mut self) {
        self.siege_is_progress_3 = false;
        self.siege_is_progress_2 = true;
    }

    pub fn disable_siege_is_progress_2(&mut self) {
        self.siege_is_progress_2 = false;
    }

    /// Reset war fatigue once per enemy city only after a nearby own land
    /// force is observed reducing its wall or city health. Version 3 replaces
    /// v2's proximity-only reset and remains independently reversible.
    pub fn enable_siege_is_progress_3(&mut self) {
        self.siege_is_progress_2 = false;
        self.siege_is_progress_3 = true;
    }

    /// The twin of `enable_siege_is_progress_3`.
    pub fn disable_siege_is_progress_3(&mut self) {
        self.siege_is_progress_3 = false;
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
        self.settler_sea_guards.clear();
        self.settler_escort_journeys.clear();
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
    /// [`super::SUZERAIN_PRIZE`]. Off on the anchor, so a comparison against it
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

    /// Keep land units out of the water: no water exploration goals, and
    /// disembark units already at sea. Native tournament games leave this
    /// disabled so their recorded ladders stay comparable.
    pub fn enable_come_ashore(&mut self) {
        self.base.come_ashore = true;
    }

    pub fn disable_come_ashore(&mut self) {
        self.base.come_ashore = false;
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

    /// Count a stacked guard as protection only when it can hold, and make it
    /// stay with its settler. See [`Self::settler_guard_holds`].
    pub fn enable_settler_guard_holds(&mut self) {
        self.settler_guard_holds = true;
    }

    pub fn disable_settler_guard_holds(&mut self) {
        self.settler_guard_holds = false;
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
        self.diplomatic_lane_forecast_2 = false;
        self.diplomatic_lane_forecast = true;
    }

    pub fn disable_diplomatic_lane_forecast(&mut self) {
        self.diplomatic_lane_forecast = false;
    }

    /// Version 2 waits until a current suzerainty or already-earned Diplomatic
    /// Victory Point proves a real foothold before projecting Congress turns.
    /// One version per family is active in a screen.
    pub fn enable_diplomatic_lane_forecast_2(&mut self) {
        self.diplomatic_lane_forecast = false;
        self.diplomatic_lane_forecast_2 = true;
    }

    /// The twin of `enable_diplomatic_lane_forecast_2`.
    pub fn disable_diplomatic_lane_forecast_2(&mut self) {
        self.diplomatic_lane_forecast_2 = false;
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
        self.conversion_majority_alarm_2 = false;
    }

    pub fn disable_conversion_majority_alarm(&mut self) {
        self.conversion_majority_alarm = false;
    }

    /// Read each rival civilization's progress toward its own majority, then
    /// use the least-converted holdout as the conjunctive victory clock.
    /// Version 2 of `conversion-majority-alarm`; enabling it selects this
    /// family version.
    pub fn enable_conversion_majority_alarm_2(&mut self) {
        self.conversion_majority_alarm = false;
        self.conversion_majority_alarm_2 = true;
    }

    /// The twin of `enable_conversion_majority_alarm_2`.
    pub fn disable_conversion_majority_alarm_2(&mut self) {
        self.conversion_majority_alarm_2 = false;
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

    /// Count a rival's city-state suzerainties toward the Diplomatic threat it
    /// presents. See [`Self::rival_suzerainty_alarm`].
    pub fn enable_rival_suzerainty_alarm(&mut self) {
        self.rival_suzerainty_alarm = true;
    }

    pub fn disable_rival_suzerainty_alarm(&mut self) {
        self.rival_suzerainty_alarm = false;
    }

    /// Point the three targeted World Congress penalties at the empire the
    /// denial layer names. See [`Self::congress_counter_leader`].
    pub fn enable_congress_counter_leader(&mut self) {
        self.congress_counter_leader = true;
    }

    pub fn disable_congress_counter_leader(&mut self) {
        self.congress_counter_leader = false;
    }

    /// Read a rival's conquests from the cities it has taken rather than only
    /// from the capitals. See [`Self::domination_city_count`].
    pub fn enable_domination_city_count(&mut self) {
        self.domination_city_count = true;
    }

    pub fn disable_domination_city_count(&mut self) {
        self.domination_city_count = false;
    }

    /// Stop a war we did not declare from taking the grand strategy while our
    /// own victory lane is live. See [`Self::unchosen_war_keeps_the_lane`].
    pub fn enable_unchosen_war_keeps_the_lane(&mut self) {
        self.unchosen_war_keeps_the_lane = true;
    }

    pub fn disable_unchosen_war_keeps_the_lane(&mut self) {
        self.unchosen_war_keeps_the_lane = false;
    }

    /// Measure the elective war against the weakest rival we can reach rather
    /// than the weakest on the board. See [`Self::elective_war_in_reach`].
    pub fn enable_elective_war_in_reach(&mut self) {
        self.elective_war_in_reach = true;
    }

    pub fn disable_elective_war_in_reach(&mut self) {
        self.elective_war_in_reach = false;
    }

    /// Shut the settler window on whether the city would pay the settler back
    /// before the game ends, rather than on a deadline. See
    /// [`Self::expansion_pays_back`].
    pub fn enable_expansion_pays_back(&mut self) {
        self.expansion_pays_back = true;
    }

    pub fn disable_expansion_pays_back(&mut self) {
        self.expansion_pays_back = false;
    }

    /// Stop a war we choose from taking the grand strategy while our own
    /// victory lane is live. See [`Self::elective_war_yields_to_a_lane`].
    pub fn enable_elective_war_yields_to_a_lane(&mut self) {
        self.elective_war_yields_to_a_lane = true;
    }

    pub fn disable_elective_war_yields_to_a_lane(&mut self) {
        self.elective_war_yields_to_a_lane = false;
    }

    /// Weigh whether a settle site can be held — barbarian exposure and
    /// distance from our own cities — not only what it yields. See
    /// [`Self::defensible_sites`].
    pub fn enable_defensible_sites(&mut self) {
        self.defensible_sites = true;
    }

    pub fn disable_defensible_sites(&mut self) {
        self.defensible_sites = false;
    }

    /// Measure the Recovery power gap against the war we are actually
    /// fighting. See [`Self::recovery_reads_the_war`].
    pub fn enable_recovery_reads_the_war(&mut self) {
        self.recovery_reads_the_war = true;
    }

    pub fn disable_recovery_reads_the_war(&mut self) {
        self.recovery_reads_the_war = false;
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

    pub fn disable_solvent_faith_army(&mut self) {
        self.solvent_faith_army = false;
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

    /// Deliberate camp clearing as a peacetime errand. See
    /// `BasicAi::camp_bounty`; entrant `advanced_camp_bounty`.
    pub fn enable_camp_bounty(&mut self) {
        self.base.camp_bounty = true;
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

    /// Run the screenable native expansion curve: rapid safe settlement first,
    /// then a conquest posture only after the practical frontier is exhausted.
    /// See [`AdvancedAi::rapid_city_expansion`].
    pub fn enable_rapid_city_expansion(&mut self) {
        self.rapid_city_expansion_2 = false;
        self.rapid_city_expansion = true;
        self.base.enable_rapid_city_expansion();
    }

    /// The twin of [`AdvancedAi::enable_rapid_city_expansion`].
    pub fn disable_rapid_city_expansion(&mut self) {
        self.rapid_city_expansion = false;
        self.base.disable_rapid_city_expansion();
    }

    /// After its current production completes, let a population-two capital
    /// start the next legal Settler before ordinary production ranking. The
    /// baseline governor retains the city-target, site, and emergency gates.
    pub fn enable_capital_settler_after_completion(&mut self) {
        self.capital_settler_after_completion = true;
        self.base.enable_capital_settler_after_completion();
    }

    /// The twin of [`AdvancedAi::enable_capital_settler_after_completion`].
    pub fn disable_capital_settler_after_completion(&mut self) {
        self.capital_settler_after_completion = false;
        self.base.disable_capital_settler_after_completion();
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

    pub fn disable_blind_objective_strength(&mut self) {
        self.blind_objective_strength = false;
    }

    pub fn disable_war_economy(&mut self) {
        self.war_economy = false;
    }

    pub fn disable_war_reinforcement(&mut self) {
        self.war_reinforcement = false;
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
    /// reopening the closed war-half bundle. See
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
        self.district_planning_2 = false;
        self.district_planning_3 = false;
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

    /// Widen the science lane's land grab, deferring its city cap, while a
    /// founded city can still mature; then revert and grow.
    /// Opt-in gene `science-expansion-phase`; the timing argument is on
    /// `SCIENCE_EXPANSION_CITY_CEILING`. Filed above the markers: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_science_expansion_phase(&mut self) {
        self.science_expansion_phase = true;
    }

    /// Keep the Science lane expanding first until five cities or 100 standard
    /// turns, the band live wins open from. Opt-in gene `science-opening-band`;
    /// the live-versus-screen disagreement is argued on
    /// `SCIENCE_OPENING_BAND_CITY_TARGET`. Filed above the markers: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_science_opening_band(&mut self) {
        self.science_opening_band = true;
    }

    /// The twin of `enable_science_opening_band`.
    pub fn disable_science_opening_band(&mut self) {
        self.science_opening_band = false;
    }

    /// The twin of `enable_science_expansion_phase`.
    pub fn disable_science_expansion_phase(&mut self) {
        self.science_expansion_phase = false;
    }

    /// Open the settler pipeline by the shortfall while the opening is behind
    /// the four-cities-by-turn-sixty pace every recorded win came from.
    /// Opt-in gene `expansion-schedule`; see
    /// `advanced/expansion_schedule.rs` for the corpus table. Filed above the
    /// markers: the append-point check reads a method line's first identifier.
    pub fn enable_expansion_schedule(&mut self) {
        self.expansion_schedule = true;
    }

    /// The twin of `enable_expansion_schedule`.
    pub fn disable_expansion_schedule(&mut self) {
        self.expansion_schedule = false;
    }

    /// A Settler out of a city past its deadline founds the best legal site
    /// within reach instead of chasing the ranked one it has not reached.
    /// Opt-in gene `settler-walk-deadline`; see
    /// `advanced/settler_walk_deadline.rs` for the live forensic. Filed above
    /// the markers: the append-point check reads a method line's first
    /// identifier.
    pub fn enable_settler_walk_deadline(&mut self) {
        self.settler_walk_deadline = true;
    }

    /// The twin of `enable_settler_walk_deadline`.
    pub fn disable_settler_walk_deadline(&mut self) {
        self.settler_walk_deadline = false;
    }

    /// Work food while the opening is behind the pace and no city has reached
    /// the population a Settler needs. Opt-in gene `growth-to-settle`; see
    /// `advanced/growth_to_settle.rs`.
    pub fn enable_growth_to_settle(&mut self) {
        self.growth_to_settle = true;
    }

    /// The twin of `enable_growth_to_settle`.
    pub fn disable_growth_to_settle(&mut self) {
        self.growth_to_settle = false;
    }

    /// Once the Palace landmass has at most two independent city sites left,
    /// rebuild a naval eye and favor water that can reveal a known foreign
    /// landfall. Opt-in gene `island-exploration`; see `BasicAi`'s
    /// `island_exploration_active` and `exploration_goal`.
    pub fn enable_island_exploration(&mut self) {
        self.base.island_exploration = true;
    }

    /// The twin of `enable_island_exploration`.
    pub fn disable_island_exploration(&mut self) {
        self.base.island_exploration = false;
    }

    /// When the main landmass has little room left, route a Settler to the
    /// nearest viable foreign site the player has discovered. Opt-in gene
    /// `overseas-settlement`; see `advanced/island_expansion.rs`.
    pub fn enable_overseas_settlement(&mut self) {
        self.overseas_settlement = true;
    }

    /// The twin of `enable_overseas_settlement`.
    pub fn disable_overseas_settlement(&mut self) {
        self.overseas_settlement = false;
    }

    /// Fall through to the next-best candidate the planner already ranked when
    /// an order is refused, instead of losing the turn. Opt-in gene
    /// `order-retry`; see `advanced/order_retry.rs`.
    pub fn enable_order_retry(&mut self) {
        self.order_retry = true;
    }

    /// The twin of `enable_order_retry`.
    pub fn disable_order_retry(&mut self) {
        self.order_retry = false;
    }

    /// Before the first city, move a nearby Warrior before the Settler and
    /// choose the city site from the terrain the Warrior has now revealed.
    /// Opt-in gene `opening-warrior-recon`; see
    /// `advanced/opening_settlement.rs`. Filed above the markers: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_opening_warrior_recon(&mut self) {
        self.opening_warrior_recon = true;
        self.opening_warrior_recon_2 = false;
    }

    /// The twin of `enable_opening_warrior_recon`.
    pub fn disable_opening_warrior_recon(&mut self) {
        self.opening_warrior_recon = false;
    }

    /// Before the first city, let only a Warrior directly escorting the
    /// Settler reveal terrain first, then reconsider from the ordinary
    /// settlement candidates. Opt-in gene `opening-warrior-recon-2`; see
    /// `advanced/opening_settlement.rs`. Filed above the markers: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_opening_warrior_recon_2(&mut self) {
        self.opening_warrior_recon = false;
        self.opening_warrior_recon_2 = true;
    }

    /// The twin of `enable_opening_warrior_recon_2`.
    pub fn disable_opening_warrior_recon_2(&mut self) {
        self.opening_warrior_recon_2 = false;
    }

    /// After a Settler's first move, discard only its disposable cached site
    /// while movement remains, so the next leg can use its new sight. Opt-in
    /// gene `settler-second-look`; see `advanced/opening_settlement.rs`.
    /// Filed above the markers: the append-point check reads a method line's
    /// first identifier.
    pub fn enable_settler_second_look(&mut self) {
        self.settler_second_look = true;
    }

    /// The twin of `enable_settler_second_look`.
    pub fn disable_settler_second_look(&mut self) {
        self.settler_second_look = false;
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

    /// Version 2 of `science_victory_drive`: the original planner remains a
    /// separately measurable family member, while this continuation uses a
    /// meaningful lead, legal launch sites, a research funnel, and an estimate
    /// that can keep a live chain moving. One version of the family plays, so
    /// this turns version 1 off. Opt-in gene `science-victory-drive-2`.
    pub fn enable_science_victory_drive_2(&mut self) {
        self.science_victory_drive = false;
        self.science_victory_drive_2 = true;
    }

    /// The twin of `enable_science_victory_drive_2`.
    pub fn disable_science_victory_drive_2(&mut self) {
        self.science_victory_drive_2 = false;
    }

    /// In the opening, ordinary repeatable Great-Person projects wait for
    /// city development; an open Prophet race and a clutch for an exceptional
    /// Scientist remain forcing. Opt-in gene `early-project-restraint`.
    pub fn enable_early_project_restraint(&mut self) {
        self.early_project_restraint_2 = false;
        self.early_project_restraint = true;
    }

    /// The twin of `enable_early_project_restraint`.
    pub fn disable_early_project_restraint(&mut self) {
        self.early_project_restraint = false;
    }

    /// Let a Builder whose nearest improvable tile cannot be routed to try the
    /// next-nearest instead of standing still for the rest of the game. See
    /// `BasicAi::builder_tries_the_next_tile`; opt-in gene
    /// `builder-tries-the-next-tile`. Filed here rather than under a marker:
    /// the append-point check reads a method line's first identifier.
    pub fn enable_builder_tries_the_next_tile(&mut self) {
        self.base.builder_tries_the_next_tile = true;
    }

    /// The twin of `enable_builder_tries_the_next_tile`.
    pub fn disable_builder_tries_the_next_tile(&mut self) {
        self.base.builder_tries_the_next_tile = false;
    }

    /// Settlers and builders stay out of a barbarian's one-turn reach: flee
    /// it, never step into it alone, and summon a guard onto the settler's
    /// tile when they must cross it. See
    /// [`AdvancedAi::civilian_out_of_reach`]. Opt-in gene
    /// `civilian-out-of-reach`.
    pub fn enable_civilian_out_of_reach(&mut self) {
        self.civilian_out_of_reach = true;
    }

    /// The twin of `enable_civilian_out_of_reach`.
    pub fn disable_civilian_out_of_reach(&mut self) {
        self.civilian_out_of_reach = false;
    }

    /// A unit the next blow could remove leaves the reach of whatever can
    /// strike it — the roll-top total of visible attackers, or a raider
    /// remembered in the fog — and a shooter or scout does not end the
    /// turn inside a raider's reach without a melee unit beside it. See
    /// `advanced/wounded_out_of_reach.rs`. Opt-in gene `wounded-out-of-reach`.
    pub fn enable_wounded_out_of_reach(&mut self) {
        self.wounded_out_of_reach = true;
    }

    /// The twin of `enable_wounded_out_of_reach`.
    pub fn disable_wounded_out_of_reach(&mut self) {
        self.wounded_out_of_reach = false;
    }

    /// A Builder chops woods, rainforest or marsh into the Settler, district
    /// or wonder at the front of the owning city's queue, priced as a one-off
    /// lump against the per-turn jobs. See
    /// [`AdvancedAi::chop_into_the_queue_value`]. Opt-in gene
    /// `chop-into-the-queue`.
    pub fn enable_chop_into_the_queue(&mut self) {
        self.chop_into_the_queue = true;
    }

    /// The twin of `enable_chop_into_the_queue`.
    pub fn disable_chop_into_the_queue(&mut self) {
        self.chop_into_the_queue = false;
    }

    /// An improvement that completes an unresearched technology's or civic's
    /// boost is worth the research the boost grants, spread over the steps
    /// the trigger still needs. See [`AdvancedAi::eureka_builder_premium`].
    /// Opt-in gene `eureka-chasing-builder`.
    pub fn enable_eureka_chasing_builder(&mut self) {
        self.eureka_chasing_builder_2 = false;
        self.eureka_chasing_builder = true;
    }

    /// The twin of `enable_eureka_chasing_builder`.
    pub fn disable_eureka_chasing_builder(&mut self) {
        self.eureka_chasing_builder = false;
    }

    /// Version two rewards only a final, immediately usable Builder eureka
    /// step for the technology or civic currently in progress. It replaces
    /// the broad v1 premium for this seat. Opt-in gene
    /// `eureka-chasing-builder-2`.
    pub fn enable_eureka_chasing_builder_2(&mut self) {
        self.eureka_chasing_builder = false;
        self.eureka_chasing_builder_2 = true;
    }

    /// The twin of `enable_eureka_chasing_builder_2`.
    pub fn disable_eureka_chasing_builder_2(&mut self) {
        self.eureka_chasing_builder_2 = false;
    }

    /// A unit, building or district that completes an unresearched
    /// technology's or civic's boost is worth the research the boost grants,
    /// spread over the steps the trigger still needs. See
    /// [`AdvancedAi::eureka_production_premium`]. Opt-in gene
    /// `eureka-chasing-production`.
    pub fn enable_eureka_chasing_production(&mut self) {
        self.eureka_chasing_production = true;
    }

    /// The twin of `enable_eureka_chasing_production`.
    pub fn disable_eureka_chasing_production(&mut self) {
        self.eureka_chasing_production = false;
    }

    /// Recruit the target's neighbours before an elective war: alliances,
    /// envoys to its city-states and joint-war invitations at the strike. From
    /// the turn the war desk holds a target we are at peace with, one alliance
    /// a turn is proposed to the target's best neighbour (`military` when free
    /// on both sides), the envoy scorer gains a term for a city-state near the
    /// target — most for one the target holds — and the turn the desk would
    /// declare, every eligible neighbour is sent `ProposeJointWar` and the
    /// declaration is held while an answer is due. See
    /// [`AdvancedAi::coalition_observe`]. Opt-in gene `coalition-before-war`.
    /// Filed above the markers: the append-point check reads a method line's
    /// first identifier.
    pub fn enable_coalition_before_war(&mut self) {
        self.coalition_before_war = true;
        self.coalition_before_war_2 = false;
        self.coalition_before_war_3 = false;
    }

    /// The twin of `enable_coalition_before_war`.
    pub fn disable_coalition_before_war(&mut self) {
        self.coalition_before_war = false;
    }

    /// Scale a node whose boost is already in hand by what the discount buys
    /// under the value function's own cost divisor. `tech_value` and
    /// `civic_value` both end `(value + k) / cost.sqrt()`, so a node costing
    /// `1 - frac` of its printed price scores `1 / (1 - frac).sqrt()` times as
    /// much — 1.29 at the shipped 40%, capped. See
    /// [`AdvancedAi::boost_in_hand_scale`]. Opt-in gene
    /// `boost-first-research`. Filed above the markers: the append-point check
    /// reads a method line's first identifier.
    pub fn enable_boost_first_research(&mut self) {
        self.boost_first_research = true;
    }

    /// The twin of `enable_boost_first_research`.
    pub fn disable_boost_first_research(&mut self) {
        self.boost_first_research = false;
    }

    /// Version one hunts every Eureka and Inspiration through a global union
    /// of research, production, Builder, and combat hooks. It stays intact as
    /// the broad family control. See `advanced/chase_every_boost.rs`. Opt-in
    /// gene `chase-every-boost`.
    /// Filed above the markers: the append-point check reads a method line's
    /// first identifier.
    pub fn enable_chase_every_boost(&mut self) {
        self.chase_every_boost_2 = false;
        self.chase_every_boost = true;
    }

    /// The twin of `enable_chase_every_boost`.
    pub fn disable_chase_every_boost(&mut self) {
        self.chase_every_boost = false;
    }

    /// Version two only lets a city finish the final production trigger for
    /// the technology or civic already in progress, before that study ends.
    /// Enabling the successor turns the broad version one off, so one family
    /// version plays. See `advanced/chase_every_boost.rs`. Opt-in gene
    /// `chase-every-boost-2`.
    pub fn enable_chase_every_boost_2(&mut self) {
        self.chase_every_boost = false;
        self.chase_every_boost_2 = true;
    }

    /// The twin of `enable_chase_every_boost_2`.
    pub fn disable_chase_every_boost_2(&mut self) {
        self.chase_every_boost_2 = false;
    }

    /// Take a node the empire would finish before its own eureka lands after
    /// that eureka, not before it. The engine credits a boost mid-research
    /// onto a node still being worked, and never onto one already finished, so
    /// only a short node loses its discount outright. See [`AdvancedAi::boost_research_value`]. Opt-in gene
    /// `boost-wait-research`. Filed above the markers: the append-point check
    /// reads a method line's first identifier.
    pub fn enable_boost_wait_research(&mut self) {
        self.boost_wait_research = true;
        self.boost_wait_research_2 = false;
    }

    /// The twin of `enable_boost_wait_research`.
    pub fn disable_boost_wait_research(&mut self) {
        self.boost_wait_research = false;
    }

    /// Wait only on a two-turn node whose final boost trigger is already at
    /// the front of an owned city queue, using the boost as a light tie-break.
    /// Opt-in gene `boost-wait-research-2`.
    pub fn enable_boost_wait_research_2(&mut self) {
        self.boost_wait_research_2 = true;
        self.boost_wait_research = false;
    }

    /// The twin of `enable_boost_wait_research_2`.
    pub fn disable_boost_wait_research_2(&mut self) {
        self.boost_wait_research_2 = false;
    }

    /// Credit a technology or civic with the boosts it makes chaseable by
    /// unlocking what their triggers need. The quarry Masonry's eureka wants
    /// needs Mining first, Machinery's three Archers need Archery, and nothing
    /// ever bought the permission. See [`AdvancedAi::boost_research_value`]. Opt-in gene
    /// `boost-unlock-research`. Filed above the markers: the append-point check
    /// reads a method line's first identifier.
    pub fn enable_boost_unlock_research(&mut self) {
        self.boost_unlock_research = true;
    }

    /// The twin of `enable_boost_unlock_research`.
    pub fn disable_boost_unlock_research(&mut self) {
        self.boost_unlock_research = false;
    }

    /// Pays the Envoy a city-state's quest promises on the unit or district family it asked for.
    /// The production queue reads the outstanding `train_unit_type` and
    /// `zone_district_type` quests and prices the item the city-state named
    /// at the seat's own Envoy value, scaled by what that city-state's next
    /// Envoy buys. A reorder of the queue, never an addition to it. Opt-in
    /// gene `quest-production`; see `advanced/city_state_quests.rs`.
    /// Operator request, 2026-08-26.
    pub fn enable_quest_production(&mut self) {
        self.quest_production = true;
    }
    /// The twin of `enable_quest_production`.
    pub fn disable_quest_production(&mut self) {
        self.quest_production = false;
    }

    /// Sends the Trader to the city-state that is asking us for a trade route, for the Envoy.
    /// The destination score carries the Envoy an outstanding
    /// `send_trade_route` quest pays, on any of that city-state's cities, so
    /// the ordinary yield terms still choose between them. Opt-in gene
    /// `quest-trade-route`; see `advanced/city_state_quests.rs`. Operator
    /// request, 2026-08-26.
    pub fn enable_quest_trade_route(&mut self) {
        self.quest_trade_route = true;
    }
    /// The twin of `enable_quest_trade_route`.
    pub fn disable_quest_trade_route(&mut self) {
        self.quest_trade_route = false;
    }

    /// Runs the camp errand against the outpost a city-state named, from further out than the usual ring.
    /// A `clear_barbarian_camp` quest names one outpost within five tiles of
    /// the city-state, and clearing any other one does not pay; the errand
    /// prefers the named camp over a nearer unnamed one and admits it from
    /// beyond the home radius it normally stops at. Opt-in gene
    /// `quest-camp-errand`; see `advanced/city_state_quests.rs`. Operator
    /// request, 2026-08-26.
    pub fn enable_quest_camp_errand(&mut self) {
        self.base.quest_camp_errand = true;
    }
    /// The twin of `enable_quest_camp_errand`.
    pub fn disable_quest_camp_errand(&mut self) {
        self.base.quest_camp_errand = false;
    }

    /// Prices the Envoy on whatever completes the Eureka or Inspiration a city-state asked for.
    /// A `trigger_tech_boost` or `trigger_civic_boost` quest is paid by the
    /// trigger, so the Envoy rides on the same boost table
    /// `eureka-chasing-production` reads — as the Envoy, beside that gene's
    /// research and independent of it. Opt-in gene `quest-boost`; see
    /// `advanced/city_state_quests.rs`. Operator request, 2026-08-26.
    pub fn enable_quest_boost(&mut self) {
        self.quest_boost = true;
    }
    /// The twin of `enable_quest_boost`.
    pub fn disable_quest_boost(&mut self) {
        self.quest_boost = false;
    }

    /// Leaves barbarian camps that raid a rival rather than us, and courts the city-states and majors beyond that rival.
    /// A camp is the rival's when a major we are not allied with has a
    /// city strictly nearer it than any of ours and it sits outside our
    /// worked ring; the envoy scorer, the alliance partner score and the
    /// joint-war answer read the far side of every rival. Opt-in gene
    /// `enemy-of-my-enemy`; see `advanced/enemy_of_my_enemy.rs`. Operator
    /// request, 2026-08-25.
    pub fn enable_enemy_of_my_enemy(&mut self) {
        self.base.enemy_of_my_enemy = true;
    }
    /// The twin of `enable_enemy_of_my_enemy`.
    pub fn disable_enemy_of_my_enemy(&mut self) {
        self.base.enemy_of_my_enemy = false;
    }

    /// Buy the plot a Barbarian Outpost stands on for the city inside whose
    /// three rings it sits, when being rid of the outpost is worth more than
    /// the plot's quote. The outpost must be one we can see and one a soldier
    /// of ours can reach and disperse — Civilization VI removes an outpost on
    /// entry, never because the ground changed hands. See
    /// [`AdvancedAi::camp_buyout_clearance`]. Opt-in gene `camp-tile-buyout`.
    /// Filed above the markers: the append-point check reads a method line's
    /// first identifier.
    pub fn enable_camp_tile_buyout(&mut self) {
        self.camp_tile_buyout = true;
    }

    /// The twin of `enable_camp_tile_buyout`.
    pub fn disable_camp_tile_buyout(&mut self) {
        self.camp_tile_buyout = false;
    }

    /// Price a Gold purchase at the build's card-boosted rate, so items a
    /// slotted card discounts lose purchase priority to items no card touches.
    /// Opt-in gene `buy-what-cards-cannot-boost`; see
    /// `advanced/gold_and_cards.rs`. Filed above the markers: the append-point
    /// check reads a method line's first identifier.
    pub fn enable_buy_what_cards_cannot_boost(&mut self) {
        self.buy_what_cards_cannot_boost = true;
    }

    /// The twin of `enable_buy_what_cards_cannot_boost`.
    pub fn disable_buy_what_cards_cannot_boost(&mut self) {
        self.buy_what_cards_cannot_boost = false;
    }

    /// Lean the city production governor toward items the slotted policy deck
    /// makes cheap, by half the card's bonus. Opt-in gene
    /// `build-what-cards-boost`; see `advanced/gold_and_cards.rs`. Filed above
    /// the markers: the append-point check reads a method line's first
    /// identifier.
    pub fn enable_build_what_cards_boost(&mut self) {
        self.build_what_cards_boost = true;
    }

    /// The twin of `enable_build_what_cards_boost`.
    pub fn disable_build_what_cards_boost(&mut self) {
        self.build_what_cards_boost = false;
    }

    /// Pay a Gold purchase premium in a city producing less than the empire's
    /// best city, proportional to the deficit. Opt-in gene
    /// `gold-for-the-young-city`; see `advanced/gold_and_cards.rs`. Filed
    /// above the markers: the append-point check reads a method line's first
    /// identifier.
    pub fn enable_gold_for_the_young_city(&mut self) {
        self.gold_for_the_young_city = true;
    }

    /// The twin of `enable_gold_for_the_young_city`.
    pub fn disable_gold_for_the_young_city(&mut self) {
        self.gold_for_the_young_city = false;
    }

    /// Version one buys Walls or a land defender for a city struck within four
    /// turns, retaining its broad damage-memory signal for comparison. Opt-in
    /// gene `native-emergency-purchase`; see
    /// `advanced/gold_and_cards.rs`. Filed above the markers: the append-point
    /// check reads a method line's first identifier.
    pub fn enable_native_emergency_purchase(&mut self) {
        self.native_emergency_purchase_2 = false;
        self.native_emergency_purchase = true;
    }

    /// The twin of `enable_native_emergency_purchase`.
    pub fn disable_native_emergency_purchase(&mut self) {
        self.native_emergency_purchase = false;
    }

    /// Version two buys the same local answer only for damage this turn or
    /// last that a visible at-war military unit can legally follow with a City
    /// Center attack. One family version plays, so enabling this successor
    /// turns version one off. Opt-in gene `native-emergency-purchase-2`; see
    /// `advanced/gold_and_cards.rs`.
    pub fn enable_native_emergency_purchase_2(&mut self) {
        self.native_emergency_purchase = false;
        self.native_emergency_purchase_2 = true;
    }

    /// The twin of `enable_native_emergency_purchase_2`.
    pub fn disable_native_emergency_purchase_2(&mut self) {
        self.native_emergency_purchase_2 = false;
    }

    // The five chokepoint toggles are filed ABOVE the markers: the
    // append-point check reads a method line's first identifier, so a
    // `pub fn` under a marker is read as the entry `pub`.
    /// Value a settle site by the passes and straits its own borders would
    /// cover. A border refuses entry to anyone without Open Borders, so the
    /// ground a city claims is ground a rival cannot cross. Off in
    /// production; opted into by name. See
    /// [`AdvancedAi::chokepoint_site_bonus`].
    pub fn enable_chokepoint_siting(&mut self) {
        self.chokepoint_siting = true;
    }

    /// The twin of `enable_chokepoint_siting`.
    pub fn disable_chokepoint_siting(&mut self) {
        self.chokepoint_siting = false;
    }

    /// Value a settle site on a one-tile land bridge by the sea detour its
    /// city center would save. A city center is a naval passage no foreign
    /// hull may enter, at peace or at war. Off in production; opted into by
    /// name. See [`AdvancedAi::canal_city_bonus`].
    pub fn enable_canal_city(&mut self) {
        self.canal_city = true;
    }

    /// The twin of `enable_canal_city`.
    pub fn disable_canal_city(&mut self) {
        self.canal_city = false;
    }

    /// Buy the plot that closes a passage a rival could walk or sail through.
    /// `expand_borders` is the engine's own influence picker and takes no
    /// advice, so the buy is the whole lever. Off in production; opted into by
    /// name. See [`AdvancedAi::chokepoint_plot_bonus`].
    pub fn enable_chokepoint_claim(&mut self) {
        self.chokepoint_claim = true;
    }

    /// The twin of `enable_chokepoint_claim`.
    pub fn disable_chokepoint_claim(&mut self) {
        self.chokepoint_claim = false;
    }

    /// Site the Encampment on the pass, a tile no foreign unit may ever
    /// enter. `can_enter_past` refuses an unpillaged foreign Encampment with
    /// no war, alliance or Open Borders exception. Off in production; opted
    /// into by name. See [`AdvancedAi::encampment_seal_bonus`].
    pub fn enable_encampment_seals_the_pass(&mut self) {
        self.encampment_seals_the_pass = true;
    }

    /// The twin of `enable_encampment_seals_the_pass`.
    pub fn disable_encampment_seals_the_pass(&mut self) {
        self.encampment_seals_the_pass = false;
    }

    /// Hold the gate on the approach to one of our cities with a surplus
    /// soldier or hull. Nothing foreign may enter a tile one of our military
    /// units stands on. Off in production; opted into by name. See
    /// [`AdvancedAi::chokepoint_garrison_step`].
    pub fn enable_chokepoint_garrison(&mut self) {
        self.chokepoint_garrison = true;
    }

    /// The twin of `enable_chokepoint_garrison`.
    pub fn disable_chokepoint_garrison(&mut self) {
        self.chokepoint_garrison = false;
    }

    // Append points, one per name range: a new treatment goes under the range
    // its own name falls in, so that two of them do not append to one line.
    // The rule, the measurement behind it and the check that enforces it are
    // on `pub struct AdvancedAi` in `src/ai/advanced.rs`.

    /// Price a settle site the way the engine pays it beside a natural
    /// wonder: the wonder's projected yields on every neighbouring work tile
    /// and a capped credit for the amenity, appeal, Holy Site adjacency and
    /// era score no yield table shows. Opt-in gene `wonder-adjacent-sites`;
    /// see `advanced/wonder_sites.rs`.
    pub fn enable_wonder_adjacent_sites(&mut self) {
        self.wonder_adjacent_sites = true;
    }

    /// The twin of `enable_wonder_adjacent_sites`.
    pub fn disable_wonder_adjacent_sites(&mut self) {
        self.wonder_adjacent_sites = false;
    }

    /// The projection plus a small flat credit per wonder tile in the
    /// footprint, capped at a river's worth.
    ///
    /// Version 2 of `wonder_adjacent_sites`; one version of a family plays, so
    /// this turns version 1 off. Opt-in gene `wonder-adjacent-sites-2`. See
    /// `AdvancedAi::wonder_adjacent_sites_2`.
    pub fn enable_wonder_adjacent_sites_2(&mut self) {
        self.wonder_adjacent_sites = false;
        self.wonder_adjacent_sites_2 = true;
    }

    pub fn disable_wonder_adjacent_sites_2(&mut self) {
        self.wonder_adjacent_sites_2 = false;
    }

    /// Send an explorer to the unseen ring of a natural wonder within
    /// settling range of an own city before it picks a frontier, so a site
    /// beside the wonder exists to be priced. Opt-in gene
    /// `wonder-ring-recon`; see `advanced/wonder_sites.rs` and
    /// `BasicAi::wonder_ring_recon`.
    pub fn enable_wonder_ring_recon(&mut self) {
        self.base.wonder_ring_recon = true;
    }

    /// The twin of `enable_wonder_ring_recon`.
    pub fn disable_wonder_ring_recon(&mut self) {
        self.base.wonder_ring_recon = false;
    }

    /// Keep one Builder per city while there is still land to improve, priced
    /// where it can win the queue. See `AdvancedAi::builder_supply_floor`;
    /// opt-in gene `builder-supply-floor`. Filed here rather than under a
    /// marker: the append-point check reads a method line's first identifier.
    pub fn enable_builder_supply_floor(&mut self) {
        self.builder_supply_floor = true;
    }

    /// The twin of `enable_builder_supply_floor`.
    pub fn disable_builder_supply_floor(&mut self) {
        self.builder_supply_floor = false;
    }
    /// Price the cheap rung of a district chain by payback and leave the whole
    /// district on the clock. See `AdvancedAi::chain_payback_window_2`; opt-in
    /// gene `chain-payback-window-2`, version two of `chain-payback-window`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_chain_payback_window_2(&mut self) {
        self.chain_payback_window_2 = true;
    }

    /// The twin of `enable_chain_payback_window_2`.
    pub fn disable_chain_payback_window_2(&mut self) {
        self.chain_payback_window_2 = false;
    }
    /// Price the science and culture chain debts by whether the building can
    /// still repay, not by how much of the clock is left. See
    /// `AdvancedAi::chain_payback_window`; opt-in gene `chain-payback-window`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_chain_payback_window(&mut self) {
        self.chain_payback_window = true;
    }

    /// The twin of `enable_chain_payback_window`.
    pub fn disable_chain_payback_window(&mut self) {
        self.chain_payback_window = false;
    }
    /// Reserve the first Builder ahead of ordinary production, the way
    /// `solvency-first-trade-slot` reserves the first trade slot. See
    /// `AdvancedAi::first_builder_reserve`; opt-in gene `first-builder-reserve`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_first_builder_reserve(&mut self) {
        self.first_builder_reserve = true;
        self.first_builder_reserve_2 = false;
    }

    /// The twin of `enable_first_builder_reserve`.
    pub fn disable_first_builder_reserve(&mut self) {
        self.first_builder_reserve = false;
    }

    /// Reserve one Builder for an immediately connectable first-copy luxury
    /// when Amenities are short and expansion is covered, retaining a
    /// lifetime receipt. Opt-in gene `first-builder-reserve-2`.
    pub fn enable_first_builder_reserve_2(&mut self) {
        self.first_builder_reserve_2 = true;
        self.first_builder_reserve = false;
    }

    /// The twin of `enable_first_builder_reserve_2`.
    pub fn disable_first_builder_reserve_2(&mut self) {
        self.first_builder_reserve_2 = false;
    }
    /// Reserve the cheapest Campus building a city owes ahead of ordinary
    /// production. See `AdvancedAi::first_research_building_reserve`; opt-in
    /// gene `first-research-building-reserve`. Filed here rather than under a
    /// marker: the append-point check reads a method line's first identifier.
    pub fn enable_first_research_building_reserve(&mut self) {
        self.first_research_building_reserve = true;
    }

    /// The twin of `enable_first_research_building_reserve`.
    pub fn disable_first_research_building_reserve(&mut self) {
        self.first_research_building_reserve = false;
    }
    /// Let the Builder see the Housing an improvement carries, the way the
    /// baseline chooser already does. See
    /// `AdvancedAi::improvement_housing_value`; opt-in gene
    /// `improvement-housing-value`. Filed here rather than under a marker: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_improvement_housing_value(&mut self) {
        self.improvement_housing_value = true;
    }

    /// The twin of `enable_improvement_housing_value`.
    pub fn disable_improvement_housing_value(&mut self) {
        self.improvement_housing_value = false;
    }
    /// Climb to a tier-2 government once Political Philosophy lands, instead of
    /// playing the whole game on four policy slots. See
    /// `AdvancedAi::government_ladder`; opt-in gene `government-ladder`. Filed
    /// here rather than under a marker: the append-point check reads a method
    /// line's first identifier.
    pub fn enable_government_ladder(&mut self) {
        self.government_ladder = true;
    }

    /// The twin of `enable_government_ladder`.
    pub fn disable_government_ladder(&mut self) {
        self.government_ladder = false;
    }
    /// Fill an idle turn with something that is not a soldier, or leave it
    /// idle. See `AdvancedAi::never_an_empty_queue_2`; opt-in gene
    /// `never-an-empty-queue-2`, version two of `never-an-empty-queue`. Filed
    /// here rather than under a marker: the append-point check reads a method
    /// line's first identifier.
    pub fn enable_never_an_empty_queue_2(&mut self) {
        self.never_an_empty_queue = false;
        self.never_an_empty_queue_2 = true;
        self.never_an_empty_queue_3 = false;
    }

    /// The twin of `enable_never_an_empty_queue_2`.
    pub fn disable_never_an_empty_queue_2(&mut self) {
        self.never_an_empty_queue_2 = false;
    }
    /// Build the best real candidate instead of standing idle when nothing
    /// clears the ordinary production bar. See
    /// `AdvancedAi::never_an_empty_queue`; opt-in gene `never-an-empty-queue`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_never_an_empty_queue(&mut self) {
        self.never_an_empty_queue = true;
        self.never_an_empty_queue_2 = false;
        self.never_an_empty_queue_3 = false;
    }

    /// The twin of `enable_never_an_empty_queue`.
    pub fn disable_never_an_empty_queue(&mut self) {
        self.never_an_empty_queue = false;
    }
    /// Modernize the standing army before the discretionary purchase pass
    /// spends the treasury, while a major war is being fought. See
    /// `AdvancedAi::upgrade_the_garrison`; opt-in gene `upgrade-the-garrison`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_upgrade_the_garrison(&mut self) {
        self.upgrade_the_garrison = true;
    }

    /// The twin of `enable_upgrade_the_garrison`.
    pub fn disable_upgrade_the_garrison(&mut self) {
        self.upgrade_the_garrison = false;
    }
    /// Claim the ground between us and the nearest neighbours first while
    /// the army can hold it, waive the border provocation there, and wall
    /// and garrison the frontier. See `advanced/contested_land.rs`; opt-in
    /// gene `contested-land-first`. The flag lives on `BasicAi`, which owns
    /// the peacetime garrison. Filed here rather than under a marker: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_contested_land_first(&mut self) {
        self.base.contested_land_first = true;
    }

    /// The twin of `enable_contested_land_first`.
    pub fn disable_contested_land_first(&mut self) {
        self.base.contested_land_first = false;
    }

    /// An adaptive seat stops racing for a Great Prophet: the race costs more
    /// science than the religion returns. See
    /// `AdvancedAi::science_building_first`; opt-in gene
    /// `science-building-first`.
    pub fn enable_science_building_first(&mut self) {
        self.science_building_first = true;
    }

    /// The twin of `enable_science_building_first`.
    pub fn disable_science_building_first(&mut self) {
        self.science_building_first = false;
    }

    /// `AdvancedAi::skip_the_prophet_race`; opt-in gene
    /// `skip-the-prophet-race`. Filed above the markers with the other
    /// toggles, because the append-point check reads a line's first
    /// identifier and every one of these starts `pub`.
    pub fn enable_skip_the_prophet_race(&mut self) {
        self.skip_the_prophet_race = true;
    }

    /// The twin of `enable_skip_the_prophet_race`.
    pub fn disable_skip_the_prophet_race(&mut self) {
        self.skip_the_prophet_race = false;
    }

    /// Cap the city target at the sites the map can actually seat. See
    /// `city_target_meets_the_map`.
    pub fn enable_city_target_meets_the_map(&mut self) {
        self.city_target_meets_the_map = true;
    }

    /// The twin of `enable_city_target_meets_the_map`.
    pub fn disable_city_target_meets_the_map(&mut self) {
        self.city_target_meets_the_map = false;
    }

    /// Climb past tier two, and keep climbing while the field out-slots us.
    /// See `government_ladder_rung`.
    pub fn enable_government_ladder_2(&mut self) {
        self.government_ladder_2 = true;
    }

    /// The twin of `enable_government_ladder_2`.
    pub fn disable_government_ladder_2(&mut self) {
        self.government_ladder_2 = false;
    }

    /// Let an unlocked government the lane's list never names take the seat
    /// when it carries more policy slots. See `government_capacity_fallback`.
    pub fn enable_government_capacity_fallback(&mut self) {
        self.government_capacity_fallback = true;
    }

    /// The twin of `enable_government_capacity_fallback`.
    pub fn disable_government_capacity_fallback(&mut self) {
        self.government_capacity_fallback = false;
    }

    /// Stop paying a lost victory lane's premiums. See
    /// `lane_release_when_hopeless`.
    pub fn enable_lane_release_when_hopeless(&mut self) {
        self.lane_release_when_hopeless = true;
    }

    /// The twin of `enable_lane_release_when_hopeless`.
    pub fn disable_lane_release_when_hopeless(&mut self) {
        self.lane_release_when_hopeless = false;
    }

    /// Sue for peace when a war has taken nothing and cannot be paid for. See
    /// `peace_when_war_does_not_pay`.
    pub fn enable_peace_when_war_does_not_pay(&mut self) {
        self.peace_when_war_does_not_pay = true;
    }

    /// The twin of `enable_peace_when_war_does_not_pay`.
    pub fn disable_peace_when_war_does_not_pay(&mut self) {
        self.peace_when_war_does_not_pay = false;
    }

    /// Refuse a war the treasury cannot pay for. See `war_needs_a_treasury`.
    pub fn enable_war_needs_a_treasury(&mut self) {
        self.war_needs_a_treasury = true;
    }

    /// The twin of `enable_war_needs_a_treasury`.
    pub fn disable_war_needs_a_treasury(&mut self) {
        self.war_needs_a_treasury = false;
    }

    /// Let the assigned diplomatic lane size its own Congress ballot. See
    /// `lane_votes_its_favor`.
    pub fn enable_lane_votes_its_favor(&mut self) {
        self.lane_votes_its_favor = true;
    }

    /// The twin of `enable_lane_votes_its_favor`.
    pub fn disable_lane_votes_its_favor(&mut self) {
        self.lane_votes_its_favor = false;
    }

    /// Stop paying for a religion race the world has already closed. See
    /// `religion_race_is_closed`.
    pub fn enable_religion_race_is_closed(&mut self) {
        self.religion_race_is_closed = true;
    }

    /// The twin of `enable_religion_race_is_closed`.
    pub fn disable_religion_race_is_closed(&mut self) {
        self.religion_race_is_closed = false;
    }

    /// Put Moksha first for a religionless empire under conversion. See
    /// `moksha_defends_the_faithless`.
    pub fn enable_moksha_defends_the_faithless(&mut self) {
        self.moksha_defends_the_faithless = true;
    }

    /// The twin of `enable_moksha_defends_the_faithless`.
    pub fn disable_moksha_defends_the_faithless(&mut self) {
        self.moksha_defends_the_faithless = false;
    }

    /// Put a ceiling on how long a settler waits for an escort. See
    /// `escort_patience_runs_out`.
    pub fn enable_escort_patience_runs_out(&mut self) {
        self.escort_patience_runs_out = true;
    }

    /// The twin of `enable_escort_patience_runs_out`.
    pub fn disable_escort_patience_runs_out(&mut self) {
        self.escort_patience_runs_out = false;
    }

    /// Keep one emergency defender and ten turns of deficit in the bank,
    /// not 250 + 75 Gold a city. See `treasury_at_work`.
    pub fn enable_treasury_at_work(&mut self) {
        self.treasury_at_work = true;
    }

    /// The twin of `enable_treasury_at_work`.
    pub fn disable_treasury_at_work(&mut self) {
        self.treasury_at_work = false;
    }

    /// The working reserve, and the first Builder or a missing Monument
    /// bought ahead of the purchase argmax. See `treasury_at_work_2`.
    pub fn enable_treasury_at_work_2(&mut self) {
        self.treasury_at_work_2 = true;
    }

    /// The twin of `enable_treasury_at_work_2`.
    pub fn disable_treasury_at_work_2(&mut self) {
        self.treasury_at_work_2 = false;
    }

    // Filed here rather than under a marker: `test_treatment_append_points`
    // reads every line under a marker as an entry and takes its first
    // identifier, so a whole function files as `pub` and `self`.

    /// The turn's fire is planned once from the engine's own arithmetic —
    /// the kills that can be finished, their shooters first in the unit
    /// order, ranged before the melee finisher, each biased toward its
    /// planned target — without a clone. See `fire_plan`.
    pub fn enable_fire_plan(&mut self) {
        self.fire_plan = true;
    }

    /// The twin of `enable_fire_plan`.
    pub fn disable_fire_plan(&mut self) {
        self.fire_plan = false;
    }

    /// On an advance, no unit ends the turn more than the body's pace plus
    /// one tile closer to the objective than the force's anchor stood, so a
    /// horseman does not meet the enemy four tiles before the line does.
    /// See `close_as_a_body`.
    pub fn enable_close_as_a_body(&mut self) {
        self.close_as_a_body = true;
    }

    /// The twin of `enable_close_as_a_body`.
    pub fn disable_close_as_a_body(&mut self) {
        self.close_as_a_body = false;
    }

    /// A shooter's tile beside a melee friend that stands nearer the enemy
    /// earns two screen weights — the arena's own definition of screened,
    /// paid to the archer that stays behind the line. See
    /// `screen_the_shooters`.
    pub fn enable_screen_the_shooters(&mut self) {
        self.screen_the_shooters = true;
    }

    /// The twin of `enable_screen_the_shooters`.
    pub fn disable_screen_the_shooters(&mut self) {
        self.screen_the_shooters = false;
    }

    /// An Archer for every city, the frontier city first, while the world
    /// is Ancient and Classical, and Archery chased until a city can train
    /// one. Filed here rather than under a marker: a whole function under
    /// one reads as an entry. See `early_archers`.
    pub fn enable_early_archers(&mut self) {
        self.early_archers = true;
    }

    /// The twin of `enable_early_archers`.
    pub fn disable_early_archers(&mut self) {
        self.early_archers = false;
    }

    /// A declared war's objective nobody of ours has been near for
    /// `CAPTURE_GO_TURNS` consecutive turns is stood down explicitly and the
    /// strategy re-assessed, instead of being held unprosecuted. Filed here
    /// rather than under a marker: a whole function under one reads as an
    /// entry. See `capture_go_or_stand_down` and `advanced/commitments.rs`.
    pub fn enable_capture_go_or_stand_down(&mut self) {
        self.capture_go_or_stand_down = true;
    }

    /// The twin of `enable_capture_go_or_stand_down`.
    pub fn disable_capture_go_or_stand_down(&mut self) {
        self.capture_go_or_stand_down = false;
        self.capture_stood_down.clear();
    }

    /// Version 2 of `capture_go_or_stand_down`: also stands down a siege
    /// whose bodies are at the objective but have not pushed the city to a
    /// new low for `CAPTURE_STALL_TURNS` stalled readings. One version of a
    /// family plays, so this turns version 1 off. Opt-in gene
    /// `capture-go-or-stand-down-2`. Filed here rather than under a marker.
    pub fn enable_capture_go_or_stand_down_2(&mut self) {
        self.capture_go_or_stand_down = false;
        self.capture_go_or_stand_down_2 = true;
    }

    /// The twin of `enable_capture_go_or_stand_down_2`.
    pub fn disable_capture_go_or_stand_down_2(&mut self) {
        self.capture_go_or_stand_down_2 = false;
        self.capture_stood_down.clear();
    }

    /// A settle or improve target survives a passing threat, and the
    /// commitment ledger retires it after `COMMITMENT_PATIENCE` consecutive
    /// forgotten turns, parking the site. Filed here rather than under a
    /// marker: a whole function under one reads as an entry. See
    /// `commitment_patience` and `advanced/commitments.rs`.
    pub fn enable_commitment_patience(&mut self) {
        self.commitment_patience = true;
    }

    /// The twin of `enable_commitment_patience`.
    pub fn disable_commitment_patience(&mut self) {
        self.commitment_patience = false;
        self.builder_avoid.clear();
    }

    /// Every open settle or improve commitment whose owner the unit pass left
    /// with movement and no order acts on it after the pass — found, improve,
    /// or the safe route step — or is released with a recorded reason. Filed
    /// here rather than under a marker: a whole function under one reads as
    /// an entry. See `commitment_owner_acts` and `advanced/commitments.rs`.
    pub fn enable_commitment_owner_acts(&mut self) {
        self.commitment_owner_acts = true;
    }

    /// The twin of `enable_commitment_owner_acts`.
    pub fn disable_commitment_owner_acts(&mut self) {
        self.commitment_owner_acts = false;
    }

    /// `culture-floor`: a culture-yielding building is exempt from the Great
    /// Work veto and the Theatre Square is priced while the empire's culture
    /// a turn trails the strongest major's by the floor ratio. See
    /// `advanced/yield_floors.rs`.
    pub fn enable_culture_floor(&mut self) {
        self.culture_floor = true;
    }

    /// The twin of `enable_culture_floor`.
    pub fn disable_culture_floor(&mut self) {
        self.culture_floor = false;
    }

    /// `gold-income-floor`: Markets, Lighthouses, gold-yielding buildings and
    /// the Commercial Hub or Harbor are priced by the income the empire is
    /// short of two Gold a turn per city. See `advanced/yield_floors.rs`.
    pub fn enable_gold_income_floor(&mut self) {
        self.gold_income_floor = true;
    }

    /// The twin of `enable_gold_income_floor`.
    pub fn disable_gold_income_floor(&mut self) {
        self.gold_income_floor = false;
    }

    /// A settler in the barbarians' hands is taken back: exempt from the
    /// duplicate-settler guard, first among adjacent captures, pursued out to
    /// `BARBARIAN_SETTLER_PURSUIT_RADIUS`. Gene `barbarian-settler-capture`;
    /// the flag lives on `BasicAi`, whose `military_step` and
    /// `capture_adjacent_civilian` read it.
    pub fn enable_barbarian_settler_capture(&mut self) {
        self.base.barbarian_settler_capture = true;
    }

    /// The twin of `enable_barbarian_settler_capture`.
    pub fn disable_barbarian_settler_capture(&mut self) {
        self.base.barbarian_settler_capture = false;
    }

    /// Price a city site's Harbor-eligible coast in the final settlement
    /// score. See `advanced/coastal_sites.rs`.
    pub fn enable_coastal_city_sites(&mut self) {
        self.coastal_city_sites = true;
    }

    /// The twin of `enable_coastal_city_sites`.
    pub fn disable_coastal_city_sites(&mut self) {
        self.coastal_city_sites = false;
    }

    /// Version 2 keeps the coast baseline and additionally prices the best
    /// water-resource adjacency around a prospective Harbor.
    pub fn enable_coastal_city_sites_2(&mut self) {
        self.coastal_city_sites = false;
        self.coastal_city_sites_2 = true;
    }

    /// The twin of `enable_coastal_city_sites_2`.
    pub fn disable_coastal_city_sites_2(&mut self) {
        self.coastal_city_sites_2 = false;
    }

    // Filed here rather than under a marker: `test_treatment_append_points`
    // reads every line under a marker as an entry and takes its first
    // identifier, so a whole function files as `pub` and `self`.

    /// Price a strike with the two strengths the engine will resolve it
    /// with, so matchup, flanking, adjacent support, ground and the river
    /// reach the exchange evaluation. See `exchange_is_the_engines`.
    pub fn enable_exchange_is_the_engines(&mut self) {
        self.base.exchange_is_the_engines = true;
    }

    /// The twin of `enable_exchange_is_the_engines`.
    pub fn disable_exchange_is_the_engines(&mut self) {
        self.base.exchange_is_the_engines = false;
    }

    /// Price the defender on the tile it is being asked about, with that
    /// tile's own defence, so a unit weighing a hill is credited for the
    /// hill. See `defend_where_you_stand`.
    pub fn enable_defend_where_you_stand(&mut self) {
        self.base.defend_where_you_stand = true;
    }

    /// The twin of `enable_defend_where_you_stand`.
    pub fn disable_defend_where_you_stand(&mut self) {
        self.base.defend_where_you_stand = false;
    }

    /// A wounded unit holding a front trades places with the fresh unit
    /// behind it — the engine's own `Action::Swap`, which no controller has
    /// ever chosen — so the line does not open when it leaves. See
    /// `swap_rotation`.
    pub fn enable_swap_rotation(&mut self) {
        self.swap_rotation = true;
    }

    /// The twin of `enable_swap_rotation`.
    pub fn disable_swap_rotation(&mut self) {
        self.swap_rotation = false;
    }

    /// A force group beyond `THREAT_RELIEF_RADIUS` of the threatened city is
    /// a relief column: it advances on the siege when locally superior,
    /// holds at the city when too weak, and while a city is threatened
    /// focuses only on combatants and cities. See `relief_column_marches`.
    pub fn enable_relief_column_marches(&mut self) {
        self.relief_column_marches = true;
    }

    /// The twin of `enable_relief_column_marches`.
    pub fn disable_relief_column_marches(&mut self) {
        self.relief_column_marches = false;
    }

    /// While a city of ours is threatened or bleeding, every ordinary Gold
    /// purchase leaves one emergency defender's price in the treasury. See
    /// `threatened_city_reserve`.
    pub fn enable_threatened_city_reserve(&mut self) {
        self.threatened_city_reserve = true;
    }

    /// The twin of `enable_threatened_city_reserve`.
    pub fn disable_threatened_city_reserve(&mut self) {
        self.threatened_city_reserve = false;
    }

    /// A Settler always has somewhere to go: exhaustion asks wider questions
    /// instead of holding, and a watchdog bounds every other hold. Filed
    /// above the marker run like `enable_early_archers`: a whole function
    /// under a marker reads as an entry. See `settler_never_idles` and
    /// `advanced/settler_never_idles.rs`.
    pub fn enable_settler_never_idles(&mut self) {
        self.settler_never_idles = true;
    }

    /// The twin of `enable_settler_never_idles`.
    pub fn disable_settler_never_idles(&mut self) {
        self.settler_never_idles = false;
    }

    /// Enter the secondary Great Prophet race for eligible lanes: Astrology
    /// after the opening techs, the empire's first Holy Site at the front of
    /// the district order, the Prophet priced as a lane great person, and
    /// `pursue_religion` for the prize. An explicit Science lane stays on its
    /// pure beeline. See `enter_the_prophet_race`.
    pub fn enable_enter_the_prophet_race(&mut self) {
        self.enter_the_prophet_race_2 = false;
        self.enter_the_prophet_race = true;
    }

    /// The twin of `enable_enter_the_prophet_race`.
    pub fn disable_enter_the_prophet_race(&mut self) {
        self.enter_the_prophet_race = false;
        self.base.enter_prophet_race = false;
    }

    /// The host's barbarian scouts capture civilians, so a barbarian recon unit counts in every capture-reach model.
    ///
    /// Host-only `live-barbarian-scouts-capture`. Run civvis-20260828T122324Z
    /// lost four settlers to one scout that `barbarian_scouts_are_scouts`
    /// exempted from every capture model; the native barbarian seat's recon
    /// really cannot capture, so the treatment is inert on a native board.
    /// See `advanced/civilian_safety.rs`.
    pub fn enable_live_barbarian_scouts_capture(&mut self) {
        self.live_barbarian_scouts_capture = true;
    }

    /// Withholding twin for `enable_live_barbarian_scouts_capture`.
    pub fn disable_live_barbarian_scouts_capture(&mut self) {
        self.live_barbarian_scouts_capture = false;
    }

    /// A lost settler retires its ground for every settler; fleeing prefers a friendly stack; guards stay near known camps; early settlers stay within a six-tile capital corridor.
    ///
    /// Host-only `live-settler-capture-lessons`: what twenty-four live captures
    /// on 2026-08-28 taught. A settler that leaves the board with no city
    /// within two tiles of where it stood was taken, and every site within
    /// three tiles of that ground is dead for every settler for thirty
    /// standard turns; a civilian fleeing with no safe tile prefers a tile one
    /// of our units holds to the least exposed bare one and never holds beside
    /// a raider while a farther tile exists; the strongest guard that can reach
    /// the settler this turn is summoned, not the nearest; a guard is not
    /// released while a known barbarian camp is within eight tiles; a Settler
    /// first seen during the one-city opening stays within six tiles of its
    /// capital and returns if an emergency flee pushes it farther. See
    /// `advanced/civilian_safety.rs`.
    pub fn enable_live_settler_capture_lessons(&mut self) {
        self.live_settler_capture_lessons = true;
    }

    /// Withholding twin for `enable_live_settler_capture_lessons`.
    pub fn disable_live_settler_capture_lessons(&mut self) {
        self.live_settler_capture_lessons = false;
    }

    /// A stranded Settler's exhaustion search sets aside a site beside an
    /// unresolved rival border — the one fog guess the preferred search made
    /// that names a city the forecast cannot see — and its nearest-legal
    /// tier runs the same concrete-revolt forecast the ranked tier does. See
    /// `AdvancedAi::exhaustion_site_unpriceable`.
    pub fn enable_exhaustion_loyalty_guard(&mut self) {
        self.exhaustion_loyalty_guard = true;
    }

    /// The twin of `enable_exhaustion_loyalty_guard`.
    pub fn disable_exhaustion_loyalty_guard(&mut self) {
        self.exhaustion_loyalty_guard = false;
    }

    /// Once the empire holds three cities, no Settler starts while an owned
    /// Settler has stood on one tile six turns; the same brake on the
    /// `BasicAi` pipeline. See `AdvancedAi::settler_in_flight_allowed`.
    pub fn enable_settler_backlog_brake(&mut self) {
        self.settler_backlog_brake = true;
        self.base.settler_backlog_brake = true;
    }

    /// The twin of `enable_settler_backlog_brake`.
    pub fn disable_settler_backlog_brake(&mut self) {
        self.settler_backlog_brake = false;
        self.base.settler_backlog_brake = false;
    }

    /// A city grown to its housing (pop + 1 ≥ housing) with no Granary starts
    /// one ahead of the argmax. See `AdvancedAi::first_granary_reserve`.
    pub fn enable_first_granary_reserve(&mut self) {
        self.first_granary_reserve = true;
    }

    /// The twin of `enable_first_granary_reserve`.
    pub fn disable_first_granary_reserve(&mut self) {
        self.first_granary_reserve = false;
    }

    /// Research the cheapest technology that connects an owned, unimproved
    /// luxury (Irrigation, Sailing) ahead of the lane's beeline, once the
    /// opening techs are in. See `AdvancedAi::unconnected_luxury_tech`.
    pub fn enable_connect_the_luxury(&mut self) {
        self.connect_the_luxury = true;
    }

    /// The twin of `enable_connect_the_luxury`.
    pub fn disable_connect_the_luxury(&mut self) {
        self.connect_the_luxury = false;
    }

    /// In peacetime, keep the seat's military power at four fifths of the
    /// strongest bordering major's by buying the contact city's ranged
    /// defender with Gold above a reserve. See
    /// `AdvancedAi::border_parity_purchase`.
    pub fn enable_border_parity(&mut self) {
        self.border_parity = true;
        self.border_parity_2 = false;
        self.border_parity_3 = false;
    }

    /// The twin of `enable_border_parity`.
    pub fn disable_border_parity(&mut self) {
        self.border_parity = false;
    }

    /// One to four era points short of a Normal Age, patronize any Great
    /// Person the bank can carry for its Historic Moment. See
    /// `AdvancedAi::era_points_short`.
    pub fn enable_age_closer(&mut self) {
        self.age_closer = true;
    }

    /// The twin of `enable_age_closer`.
    pub fn disable_age_closer(&mut self) {
        self.age_closer = false;
    }

    /// Research a prerequisite-met technology whose Eureka is in hand and
    /// whose remaining cost is at most two turns of science before the lane's
    /// beeline resumes. See `AdvancedAi::boosted_bargain_tech`.
    pub fn enable_boosted_bargain_first(&mut self) {
        self.boosted_bargain_first = true;
        self.boosted_bargain_first_2 = false;
    }

    /// The twin of `enable_boosted_bargain_first`.
    pub fn disable_boosted_bargain_first(&mut self) {
        self.boosted_bargain_first = false;
    }

    /// Let a one-turn boosted technology break a close ordinary fallback
    /// decision after every forced lane goal stands down. See
    /// `AdvancedAi::boosted_bargain_tech_2`.
    pub fn enable_boosted_bargain_first_2(&mut self) {
        self.boosted_bargain_first_2 = true;
        self.boosted_bargain_first = false;
    }

    /// The twin of `enable_boosted_bargain_first_2`.
    pub fn disable_boosted_bargain_first_2(&mut self) {
        self.boosted_bargain_first_2 = false;
    }

    /// A wonder within twelve turns of done in one of the empire's strongest
    /// cities opens the live wonder race without the three-city and
    /// three-building guards, with a bonus that scales with how quickly it
    /// finishes; a wonder further than twenty-five turns from done never
    /// opens the ordinary race. See `AdvancedAi::wonder_bargain_city`.
    pub fn enable_cheapest_wonder_first(&mut self) {
        self.cheapest_wonder_first = true;
    }

    /// The twin of `enable_cheapest_wonder_first`.
    pub fn disable_cheapest_wonder_first(&mut self) {
        self.cheapest_wonder_first = false;
    }

    /// Version two of `border-parity`: the same target and Gold purchase,
    /// and when the treasury cannot pay, the contact city's idle queue starts
    /// the defender. See `AdvancedAi::border_parity_target`.
    pub fn enable_border_parity_2(&mut self) {
        self.border_parity = false;
        self.border_parity_2 = true;
        self.border_parity_3 = false;
    }

    /// The twin of `enable_border_parity_2`.
    pub fn disable_border_parity_2(&mut self) {
        self.border_parity_2 = false;
    }

    pub fn enable_first_district_first(&mut self) {
        self.first_district_first = true;
    }

    pub fn disable_first_district_first(&mut self) {
        self.first_district_first = false;
    }

    pub fn enable_walls_after_districts(&mut self) {
        self.walls_after_districts = true;
    }

    pub fn disable_walls_after_districts(&mut self) {
        self.walls_after_districts = false;
    }

    /// The two-turn escort cap releases the settler on schedule instead of
    /// being suspended by a predicate that reads only the visible frame, and
    /// a settler already outside its own city with no guard on its tile
    /// marches on a zero risk reading rather than fortifying bare. See
    /// `AdvancedAi::stacked_escort_pace`.
    pub fn enable_escort_cap_holds(&mut self) {
        self.escort_cap_holds = true;
    }

    /// The twin of `enable_escort_cap_holds`.
    pub fn disable_escort_cap_holds(&mut self) {
        self.escort_cap_holds = false;
    }

    /// The civilian capture envelope counts every at-war owner and keeps
    /// pricing a hostile the seat has seen for a few turns after it walks
    /// back into the fog. See `AdvancedAi::barbarian_reach`.
    pub fn enable_hostile_memory(&mut self) {
        self.hostile_memory = true;
    }

    /// The twin of `enable_hostile_memory`.
    pub fn disable_hostile_memory(&mut self) {
        self.hostile_memory = false;
    }

    pub fn enable_first_luxury_first(&mut self) {
        self.first_luxury_first = true;
    }

    pub fn disable_first_luxury_first(&mut self) {
        self.first_luxury_first = false;
    }

    /// `live-move-refusal-break` (HostOnly): stop re-issuing a move the host
    /// keeps refusing. `BasicAi` records the first pathed step issued to each
    /// unit per turn; a unit seen on the same tile two judged turns running
    /// with the same issued step gets that step barred for eight standard
    /// turns (`judge_move_refusals`), and a frozen Settler additionally has
    /// its destination retired through the dead-site machinery. Measured
    /// motive: 13.9% of all orders were `did_not_move`, with one settler
    /// re-ordered to the identical tile eleven straight turns before capture.
    pub fn enable_live_move_refusal_break(&mut self) {
        self.live_move_refusal_break = true;
        self.base.move_refusal_break = true;
    }

    /// Withholding twin for `enable_live_move_refusal_break`.
    pub fn disable_live_move_refusal_break(&mut self) {
        self.live_move_refusal_break = false;
        self.base.move_refusal_break = false;
    }

    /// `spaceport-surplus-veto` (OptIn): the Science strategy's flat per-pad
    /// district bonus stops paying once the empire already holds as many
    /// Spaceports as the current race stage can use
    /// (`science_drive_desired_pads`). Measured motive: live Emperor game
    /// 20260901T132005Z built nine pads and was still ordering more at t213
    /// with the race already unwinnable.
    pub fn enable_spaceport_surplus_veto(&mut self) {
        self.spaceport_surplus_veto = true;
    }

    /// The twin of `enable_spaceport_surplus_veto`.
    pub fn disable_spaceport_surplus_veto(&mut self) {
        self.spaceport_surplus_veto = false;
    }

    /// `district-planning-2`: the district plan's tile buy competes out of
    /// the treasury reserve (never spending below half of it) instead of
    /// needing 200 Gold of surplus headroom, and the purchase bars drop to
    /// adjacency 2 with an edge of 1 over owned ground. A Science lane also
    /// promotes a workable tile worth at least 5 Science, or the connector
    /// that immediately opens it, into that strategic competition; it may
    /// draw through the general reserve but preserves the war-package and
    /// immediate-defender floors. Measured motive: zero `buy_plot` orders in
    /// every recorded live game — replaying Emperor
    /// game 20260901T132005Z, the plan priced the adjacency-4 Campus plot at
    /// 905 against a floor of 120 on every probed turn and only the headroom
    /// rule refused it, while three cities placed campuses at adjacency ≤ 1
    /// beside that ground.
    pub fn enable_district_planning_2(&mut self) {
        self.district_planning = false;
        self.district_planning_3 = false;
        self.district_planning_2 = true;
    }

    /// The twin of `enable_district_planning_2`.
    pub fn disable_district_planning_2(&mut self) {
        self.district_planning_2 = false;
    }

    /// `district-planning-3`: retain the joint district-site plan, but make
    /// a Gold purchase only for its highest-value unowned site when that city
    /// is idle and can start the district next. The full working reserve stays
    /// intact, and version 2's speculative high-Science and bridge purchases
    /// are deliberately absent. One family version plays at a time.
    pub fn enable_district_planning_3(&mut self) {
        self.district_planning = false;
        self.district_planning_2 = false;
        self.district_planning_3 = true;
    }

    /// The twin of `enable_district_planning_3`.
    pub fn disable_district_planning_3(&mut self) {
        self.district_planning_3 = false;
    }

    /// Version 2 of `air_surge`: the science–domination loop. The original
    /// one-appointment surge remains a separately measurable family member;
    /// this continuation lets the Formal-War clock run through the buildout,
    /// prices a follow-up surge by the package still missing so the loop
    /// repeats, and keeps the Campus priced under Conquest. One version of
    /// the family plays, so this turns version 1 off. Opt-in gene
    /// `air-surge-2`. (Filed above the letter markers like every toggle pair
    /// here — the append-point guard reads a method line's first identifier.)
    pub fn enable_air_surge_2(&mut self) {
        self.air_surge = false;
        self.air_surge_2 = true;
    }

    /// The twin of `enable_air_surge_2`.
    pub fn disable_air_surge_2(&mut self) {
        self.air_surge_2 = false;
    }

    /// `settler-site-gate`: a city starts a Settler only while an acceptable,
    /// unclaimed site worth founding exists. See `advanced/settler_site_gate.rs`.
    pub fn enable_settler_site_gate(&mut self) {
        self.settler_site_gate = true;
    }

    /// The twin of `enable_settler_site_gate`.
    pub fn disable_settler_site_gate(&mut self) {
        self.settler_site_gate = false;
    }

    /// `campus-through-expansion` (OptIn): a Science seat prices the Campus
    /// and its buildings in the Science lane while its plan still reads
    /// Expansion, and no city's first specialty district may be an
    /// Entertainment Complex. Measured motive: live Emperor game
    /// 20260901T175154Z held four cities and zero districts at turn 64 —
    /// under Expansion the Campus had no strategic arm and a beaker weighed
    /// 1.2, so the Plaza, walls, the repair project and an Arena-credited
    /// Entertainment Complex took every queue.
    pub fn enable_campus_through_expansion(&mut self) {
        self.campus_through_expansion = true;
    }

    /// The twin of `enable_campus_through_expansion`.
    pub fn disable_campus_through_expansion(&mut self) {
        self.campus_through_expansion = false;
    }

    /// `trade-route-network` (OptIn): a Commercial Hub (or a Harbor where no
    /// Hub stands) beside a standing Campus escapes the Science contract and
    /// is priced as trade capacity; a Market or Lighthouse is worth the
    /// route it adds. Measured motive: zero Commercial Hubs in the last 22
    /// live runs and a trade capacity of one for the whole game — thirteen
    /// cities on one Trade Route for 216 turns in 20260901T132005Z.
    pub fn enable_trade_route_network(&mut self) {
        self.trade_route_network = true;
    }

    /// The twin of `enable_trade_route_network`.
    pub fn disable_trade_route_network(&mut self) {
        self.trade_route_network = false;
    }

    /// `industrial-chain-debt` (OptIn): an Industrial Zone owes its
    /// Workshop, Factory and plant the same flat debt a Campus owes its
    /// Library, a regional building is worth the production it reaches, and
    /// the Factory and plants join the buildings a repeatable project waits
    /// behind. Measured motive: nine Industrial Zones, four Workshops, one
    /// Factory and no plant at turn 216 of 20260901T132005Z.
    pub fn enable_industrial_chain_debt(&mut self) {
        self.industrial_chain_debt = true;
    }

    /// The twin of `enable_industrial_chain_debt`.
    pub fn disable_industrial_chain_debt(&mut self) {
        self.industrial_chain_debt = false;
    }

    /// Version 2 of `skip-the-prophet-race`: retain version 1's published
    /// behavior, but screen the narrower last-call decision independently.
    /// One version per family is active in a screen.
    pub fn enable_skip_the_prophet_race_2(&mut self) {
        self.skip_the_prophet_race = false;
        self.skip_the_prophet_race_2 = true;
    }

    /// The twin of `enable_skip_the_prophet_race_2`.
    pub fn disable_skip_the_prophet_race_2(&mut self) {
        self.skip_the_prophet_race_2 = false;
    }

    /// `siege-preempts-the-queue`: a raider on a city's doorstep is answered
    /// with a body before anything else is built, bought when no defender
    /// exists, and a recon unit is not a defender. A `BasicAi` flag; see
    /// `advanced/siege_response.rs`.
    pub fn enable_siege_preempts_the_queue(&mut self) {
        self.base.enable_siege_preempts_the_queue();
    }

    /// The twin of `enable_siege_preempts_the_queue`.
    pub fn disable_siege_preempts_the_queue(&mut self) {
        self.base.disable_siege_preempts_the_queue();
    }

    /// `guard-breaks-the-pin`: a Settler's stacked guard strikes the raider
    /// whose zone of control pins the pair when the trade is worth it. See
    /// `advanced/siege_response.rs`.
    pub fn enable_guard_breaks_the_pin(&mut self) {
        self.guard_breaks_the_pin = true;
    }

    /// The twin of `enable_guard_breaks_the_pin`.
    pub fn disable_guard_breaks_the_pin(&mut self) {
        self.guard_breaks_the_pin = false;
    }

    /// `settler-target-floor`: a Settler is never sent to a site not worth the
    /// walk. See `advanced/settler_target_floor.rs`.
    pub fn enable_settler_target_floor(&mut self) {
        self.settler_target_floor = true;
    }

    /// The twin of `enable_settler_target_floor`.
    pub fn disable_settler_target_floor(&mut self) {
        self.settler_target_floor = false;
    }

    /// Version two of `rapid-city-expansion`: aim at the measured five-city
    /// opening band without version one's immediate fifteen-city order,
    /// non-empty queue preemption, closest-site override, founding-pantheon
    /// override, or automatic conquest pivot. One family member plays, so
    /// enabling this version turns version one off.
    pub fn enable_rapid_city_expansion_2(&mut self) {
        self.rapid_city_expansion = false;
        self.rapid_city_expansion_2 = true;
        self.base.enable_rapid_city_expansion_2();
    }

    /// The twin of `enable_rapid_city_expansion_2`.
    pub fn disable_rapid_city_expansion_2(&mut self) {
        self.rapid_city_expansion_2 = false;
        self.base.disable_rapid_city_expansion_2();
    }

    /// Version three of `never-an-empty-queue`: tolerate one transient empty
    /// turn, then recover a persistent stall with a civilian candidate above
    /// the hard veto. Enabling it selects this family version exclusively.
    pub fn enable_never_an_empty_queue_3(&mut self) {
        self.never_an_empty_queue = false;
        self.never_an_empty_queue_2 = false;
        self.never_an_empty_queue_3 = true;
    }

    /// The twin of `enable_never_an_empty_queue_3`.
    pub fn disable_never_an_empty_queue_3(&mut self) {
        self.never_an_empty_queue_3 = false;
    }

    /// At the ready strike, invite one credible neighbour to a joint war and
    /// wait no more than one turn. Credible means the target is close to a
    /// victory or the partner has the grievance and combined power that make
    /// the Basic controller accept. Before the strike it asks that partner
    /// only for a military alliance and spends Envoy score only to unseat the
    /// target from a nearby client; it never retries. Opt-in gene
    /// `coalition-before-war-2`.
    pub fn enable_coalition_before_war_2(&mut self) {
        self.coalition_before_war_2 = true;
        self.coalition_before_war = false;
        self.coalition_before_war_3 = false;
    }

    /// The twin of `enable_coalition_before_war_2`.
    pub fn disable_coalition_before_war_2(&mut self) {
        self.coalition_before_war_2 = false;
    }

    /// Recruit only a target neighbour already fighting it. The accepted
    /// military alliance makes that real second front an immediate combat
    /// bonus when we declare; this version neither diverts Envoys nor holds
    /// a ready declaration for a speculative joint-war answer. Opt-in gene
    /// `coalition-before-war-3`.
    pub fn enable_coalition_before_war_3(&mut self) {
        self.coalition_before_war_3 = true;
        self.coalition_before_war = false;
        self.coalition_before_war_2 = false;
    }

    /// The twin of `enable_coalition_before_war_3`.
    pub fn disable_coalition_before_war_3(&mut self) {
        self.coalition_before_war_3 = false;
    }

    /// The force's turn is planned jointly — the danger field, a
    /// beam-searched kill sequence verified on one clone, and a heal
    /// rotation — ahead of the per-unit ladder, which leaves the planned
    /// units alone. See `battle_planner`.
    pub fn enable_battle_planner(&mut self) {
        self.battle_planner = true;
    }

    /// The twin of `enable_battle_planner`.
    pub fn disable_battle_planner(&mut self) {
        self.battle_planner = false;
    }

    /// Restrain an off-lane repeatable Great-Person project only while its
    /// district owes a first building this city can start now. Lane projects
    /// and immediate race swings remain available. Version 2 of
    /// `early-project-restraint`; one family member plays at a time.
    pub fn enable_early_project_restraint_2(&mut self) {
        self.early_project_restraint = false;
        self.early_project_restraint_2 = true;
    }

    /// The twin of `enable_early_project_restraint_2`.
    pub fn disable_early_project_restraint_2(&mut self) {
        self.early_project_restraint_2 = false;
    }

    /// Version three of `border-parity`: fill one local garrison debt only
    /// when two visible non-recon land bodies from the same peaceful major
    /// are staged beside the city. Buy one answer without touching production.
    /// Enabling it selects this family version.
    pub fn enable_border_parity_3(&mut self) {
        self.border_parity = false;
        self.border_parity_2 = false;
        self.border_parity_3 = true;
    }

    /// The twin of `enable_border_parity_3`.
    pub fn disable_border_parity_3(&mut self) {
        self.border_parity_3 = false;
    }

    /// Version two of `battle_planner`: the positions plan — slots laid
    /// against the enemy contact and the objective, a minimum-cost
    /// assignment with reservations, moves front to rear, pacing on the
    /// approach — joins the kill plan and the heal rotation. One version of
    /// a family plays, so this turns version one off. Opt-in gene
    /// `battle-planner-2`. See `battle_planner`.
    pub fn enable_battle_planner_2(&mut self) {
        self.battle_planner = false;
        self.battle_planner_2 = true;
    }

    /// The twin of `enable_battle_planner_2`.
    pub fn disable_battle_planner_2(&mut self) {
        self.battle_planner_2 = false;
    }

    /// `battle-planner-3`: version three of `battle_planner` — the siege's
    /// taker left to the siege, the host's strike preview over the closed
    /// form, and the previews asked for. One version of a family plays, so
    /// this turns versions one and two off. See `battle_planner`.
    pub fn enable_battle_planner_3(&mut self) {
        self.battle_planner = false;
        self.battle_planner_2 = false;
        self.battle_planner_3 = true;
    }

    /// The twin of `enable_battle_planner_3`.
    pub fn disable_battle_planner_3(&mut self) {
        self.battle_planner_3 = false;
    }

    /// A force whose objective is an enemy city plays the siege as a state
    /// machine — stage on the ring out of the city's reach until the bill is
    /// met, seal the ring spread-first, reduce walls before the garrison,
    /// reserve one taker and walk in when the city is within its blow. See
    /// `siege_train`.
    pub fn enable_siege_train(&mut self) {
        self.siege_train = true;
    }

    /// The twin of `enable_siege_train`.
    pub fn disable_siege_train(&mut self) {
        self.siege_train = false;
    }

    /// The land group nearest a threatened city of ours holds it as a
    /// formation — a shooter on the centre, melee on the front tiles that
    /// face the enemy, the rest within two, the wounded rotated into the
    /// city — in place of the relief hold point. See `siege_train`.
    pub fn enable_anvil(&mut self) {
        self.anvil = true;
    }

    /// The twin of `enable_anvil`.
    pub fn disable_anvil(&mut self) {
        self.anvil = false;
    }

    /// Enter the secondary Great Prophet race only when the board-aware
    /// religious-opening rank admits this seat: two cities, a real Holy Site
    /// path, and one of the remaining global slots. The entry fee and prize
    /// still move together. Version 2 of `enter-the-prophet-race`; enabling it
    /// selects this family version.
    pub fn enable_enter_the_prophet_race_2(&mut self) {
        self.enter_the_prophet_race = false;
        self.enter_the_prophet_race_2 = true;
    }

    /// The twin of `enable_enter_the_prophet_race_2`.
    pub fn disable_enter_the_prophet_race_2(&mut self) {
        self.enter_the_prophet_race_2 = false;
        self.base.enter_prophet_race = false;
    }

    /// Upgrade the standing army before the purchase pass, at strategic
    /// moments. See `AdvancedAi::modernize_before_spending`; default-on gene
    /// `modernize-before-spending`.
    pub fn enable_modernize_before_spending(&mut self) {
        self.modernize_before_spending = true;
    }
    /// The twin of `enable_modernize_before_spending`.
    pub fn disable_modernize_before_spending(&mut self) {
        self.modernize_before_spending = false;
    }
    /// A promoted unit keeps hit points in hand before it withdraws. See
    /// `BasicAi::veteran_retreat_margin`; default-on gene
    /// `veterans-withdraw-early`.
    pub fn enable_veteran_retreat_margin(&mut self) {
        self.base.veteran_retreat_margin = true;
    }
    /// The twin of `enable_veteran_retreat_margin`.
    pub fn disable_veteran_retreat_margin(&mut self) {
        self.base.veteran_retreat_margin = false;
    }

    /// The army's turn planned from a ranked Objective Board — rows valued in
    /// hammers with a requirement and a deadline — and served by persistent
    /// task forces, in place of proximity force groups and the posture
    /// ladder; `force_groups` is built from the forces. See `objective_board`.
    pub fn enable_objective_board(&mut self) {
        self.objective_board = true;
    }

    /// The twin of `enable_objective_board`.
    pub fn disable_objective_board(&mut self) {
        self.objective_board = false;
    }

    /// `requisitions`: the Objective Board's shortfall reaches production
    /// and the treasury. See `advanced/requisitions.rs`.
    pub fn enable_requisitions(&mut self) {
        self.requisitions = true;
    }

    /// The twin of `enable_requisitions`.
    pub fn disable_requisitions(&mut self) {
        self.requisitions = false;
    }

    /// `war-policy-via-board`: target feasibility, the declaration and the
    /// peace term read off the board. See `advanced/war_policy.rs`.
    pub fn enable_war_policy_via_board(&mut self) {
        self.war_policy_via_board = true;
    }

    /// The twin of `enable_war_policy_via_board`.
    pub fn disable_war_policy_via_board(&mut self) {
        self.war_policy_via_board = false;
    }

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
