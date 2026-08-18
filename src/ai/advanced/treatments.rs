//! The live-bridge treatment table: one row per treatment the seat can withhold.
//!
//! ★★★ THIS TABLE LEFT `advanced.rs` BECAUSE IT IS A SHARED LIST IN THE MOST
//! CONTENDED FILE IN THE REPOSITORY. `tools/conflict_hotspots.py` measures
//! `src/ai/advanced.rs` at 23% of the last 200 merges, first by a wide margin,
//! and `docs/ROADMAP.md` objective 5 separates the two reasons a file is
//! contended: size, which splitting answers, and *one shared line or list*,
//! which it does not — two PRs appending a row to the same list conflict
//! whatever the file's length, and the remedy is to move the data out.
//!
//! Every treatment PR appends here. Now it appends to eighty lines that hold
//! nothing else, rather than to line 3,300 of twenty-eight thousand.
//!
//! This was one of the two anchors a treatment PR touches, and deliberately
//! the small half: the `enable_*`/`disable_*` pair still landed in
//! `advanced.rs` beside the flag it sets, two PRs adding a pair at that anchor
//! had collided there twice on 2026-08-16, and one of those resolutions
//! swallowed the earlier function's closing brace. **The other half followed
//! it out on 2026-08-18**: all 182 toggles now live in
//! `advanced/treatment_flags.rs`, which also guards that they stay there.
//!
//! ⚠ The arm names are published in `docs/EVAL.md` and `docs/eval/`. They are
//! the identity a measured result is filed under, so renaming a row here
//! silently unfinds a recorded experiment. Add rows; do not rewrite them.

use super::AdvancedAi;

/// One live treatment: the published arm identity, the provenance tag, and
/// the call that takes it back out.
pub type LiveTreatment = (&'static str, &'static str, fn(&mut AdvancedAi));

#[rustfmt::skip]
pub const LIVE_TREATMENTS: [LiveTreatment; 79] = [
    ("joint_tactics", "joint-tactics", AdvancedAi::disable_joint_tactics),
    ("live_trader_route_adapter", "live-trader-route", AdvancedAi::disable_live_trader_route_adapter),
    ("live_religious_purchase_guard", "live-religious-purchase", AdvancedAi::disable_live_religious_purchase_guard),
    ("siege_muster", "siege-muster", AdvancedAi::disable_siege_muster),
    ("home_defense", "home-defense", AdvancedAi::disable_home_defense),
    ("loyalty_policy_defence", "loyalty-policy-defence", AdvancedAi::disable_loyalty_policy_defence),
    ("recorded_tactical_step", "recorded-tactical-step", AdvancedAi::disable_recorded_tactical_step),
    ("strike_opening", "strike-opening", AdvancedAi::disable_strike_opening),
    ("bounded_recovery", "bounded-recovery", AdvancedAi::disable_bounded_recovery),
    ("army_target_weighs_enemy", "army-target-weighs-enemy", AdvancedAi::disable_army_target_weighs_the_enemy),
    ("peacetime_deterrence", "peacetime-deterrence", AdvancedAi::disable_peacetime_deterrence),
    ("siege_tracks_wall", "siege-tracks-wall", AdvancedAi::disable_siege_tracks_the_wall),
    ("blind_objective_strength", "blind-objective-strength", AdvancedAi::disable_blind_objective_strength),
    ("solvent_faith_army", "solvent-faith-army", AdvancedAi::disable_solvent_faith_army),
    ("loyalty_rate_alarm", "loyalty-rate-alarm", AdvancedAi::disable_loyalty_rate_alarm),
    ("ranged_needs_line_of_sight", "ranged-line-of-sight", AdvancedAi::disable_ranged_needs_line_of_sight),
    ("district_coverage", "district-coverage", AdvancedAi::disable_district_coverage),
    ("slot_kind_tiebreak", "slot-kind-tiebreak", AdvancedAi::disable_slot_kind_tiebreak),
    ("siege_role", "siege-role", AdvancedAi::disable_siege_role),
    ("come_ashore", "come-ashore", AdvancedAi::disable_come_ashore),
    ("relief_targets_the_siege", "relief-targets-the-siege", AdvancedAi::disable_relief_targets_the_siege),
    ("blind_objective_units", "blind-objective-units", AdvancedAi::disable_blind_objective_units),
    ("suzerain_cards", "suzerain-cards", AdvancedAi::disable_suzerain_cards_need_a_suzerainty),
    ("muster_at_command_radius", "muster-at-command-radius", AdvancedAi::disable_muster_at_command_radius),
    ("housing_districts", "housing-districts", AdvancedAi::disable_housing_districts),
    ("campus_every_city", "campus-every-city", AdvancedAi::disable_campus_every_city),
    ("housing_cards", "housing-cards", AdvancedAi::disable_housing_cards),
    ("housing_research", "housing-research", AdvancedAi::disable_housing_research),
    ("war_economy", "war-economy", AdvancedAi::disable_war_economy),
    ("war_reinforcement", "war-reinforcement", AdvancedAi::disable_war_reinforcement),
    ("war_patience", "war-patience", AdvancedAi::disable_war_patience),
    ("endgame_war_runway", "endgame-war-runway", AdvancedAi::disable_endgame_war_runway),
    ("wide_map_capacity", "wide-map-capacity", AdvancedAi::disable_wide_map_capacity),
    ("garrison_under_fire", "garrison-under-fire", AdvancedAi::disable_garrison_under_fire),
    ("escort_unstick", "escort-unstick", AdvancedAi::disable_escort_unstick),
    ("stacked_escort", "stacked-escort", AdvancedAi::disable_stacked_escort),
    ("religion_sues_peace", "religion-sues-peace", AdvancedAi::disable_religion_sues_peace),
    ("recon_replacement", "recon-replacement", AdvancedAi::disable_recon_replacement),
    ("stranded_settler_discount", "stranded-settler-discount", AdvancedAi::disable_stranded_settler_discount),
    ("siege_commitment", "siege-commitment", AdvancedAi::disable_siege_commitment),
    ("wonder_ring_settle_value", "wonder-ring-settle-value", AdvancedAi::disable_wonder_ring_settle_value),
    ("garrison_walls", "garrison-walls", AdvancedAi::disable_garrison_walls),
    ("housing_buildings", "housing-buildings", AdvancedAi::disable_housing_buildings),
    ("amenity_project_preemption", "amenity-project-preemption", AdvancedAi::disable_amenity_project_preemption),
    ("amenity_district_path", "amenity-district-path", AdvancedAi::disable_amenity_district_path),
    ("governor_every_lane", "governor-every-lane", AdvancedAi::disable_governor_every_lane),
    ("live_wonder_race", "live-wonder-race", AdvancedAi::disable_live_wonder_race),
    ("expansion_before_prophet", "expansion-before-prophet", AdvancedAi::disable_expansion_before_prophet),
    ("no_elective_war", "no-elective-war", AdvancedAi::disable_no_elective_war),
    ("fog_land_capacity", "fog-land-capacity", AdvancedAi::disable_fog_land_capacity),
    ("recon_flight", "recon-flight", AdvancedAi::disable_recon_flight),
    ("score_horizon", "score-horizon", AdvancedAi::disable_score_horizon),
    ("strategic_wonders", "strategic-wonders", AdvancedAi::disable_strategic_wonders),
    ("one_launch_pad", "one-launch-pad", AdvancedAi::disable_one_launch_pad),
    ("naval_recon", "naval-recon", AdvancedAi::disable_naval_recon),
    ("counter_in_lane", "counter-in-lane", AdvancedAi::disable_counter_in_lane),
    ("era_paced_expansion", "era-paced-expansion", AdvancedAi::disable_era_paced_expansion),
    ("tally_culture", "tally-culture", AdvancedAi::disable_tally_culture),
    ("culture_building_debt", "culture-building-debt", AdvancedAi::disable_culture_building_debt),
    ("culture_coverage", "culture-coverage", AdvancedAi::disable_culture_coverage),
    ("frontier_loyalty", "frontier-loyalty", AdvancedAi::disable_frontier_loyalty),
    ("settler_target_hysteresis", "settler-target-hysteresis", AdvancedAi::disable_settler_target_hysteresis),
    ("tally_great_people", "tally-great-people", AdvancedAi::disable_tally_great_people),
    ("barbarian_scouts_are_scouts", "barbarian-scouts-are-scouts", AdvancedAi::disable_barbarian_scouts_are_scouts),
    ("camp_reach", "camp-reach", AdvancedAi::disable_camp_reach),
    ("settler_stack_discipline", "settler-stack-discipline", AdvancedAi::disable_settler_stack_discipline),
    ("camp_party", "camp-party", AdvancedAi::disable_camp_party),
    ("buildings_before_projects", "buildings-before-projects", AdvancedAi::disable_buildings_before_projects),
    ("deny_while_targeted", "deny-while-targeted", AdvancedAi::disable_deny_while_targeted),
    ("stock_denial_lead_time", "stock-denial-lead-time", AdvancedAi::disable_stock_denial_lead_time),
    ("parallel_settlers", "parallel-settlers", AdvancedAi::disable_parallel_settlers),
    ("host_settler_pop", "host-settler-pop", AdvancedAi::disable_host_settler_pop),
    ("explore_dead_targets", "explore-dead-targets", AdvancedAi::disable_explore_dead_targets),
    ("explore_commit", "explore-commit", AdvancedAi::disable_explore_commit),
    ("bank_envoys", "bank-envoys", AdvancedAi::disable_bank_envoys),
    ("land_grab", "land-grab", AdvancedAi::disable_land_grab),
    ("siege_is_progress", "siege-is-progress", AdvancedAi::disable_siege_is_progress),
    ("spy_mission_patience", "spy-mission-patience", AdvancedAi::disable_spy_mission_patience),
    ("settler_site_agreement", "settler-site-agreement", AdvancedAi::disable_settler_site_agreement),
];
