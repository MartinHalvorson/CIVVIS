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
/// ★★★ NO COUNT, BECAUSE THE COUNT KEPT BEING WRONG.
///
/// This was `[LiveTreatment; N]`, and N is a number every appending PR has to
/// raise while every other appending PR raises it too. On 2026-08-19 `main`
/// stopped compiling with `expected an array with a size of 80, found one with
/// a size of 83`: two treatments had landed within minutes of each other, each
/// correctly bumping the count it saw. Two repair PRs were opened, both saying
/// "81 rows" — and by the time either could merge the table held 83, so both
/// were stale before review. That is not a mistake anyone made; it is what a
/// hand-maintained length does on a list this contended.
///
/// A slice has no length to maintain. Appending a row is now a one-line diff
/// that cannot break the build, and the only thing two concurrent PRs can
/// collide on is the row itself — which is a real conflict, resolved by keeping
/// both lines.
///
/// Every consumer already used `.iter()`, `.len()`, or `for … in`, all of which
/// a slice supports unchanged.
pub const LIVE_TREATMENTS: &[LiveTreatment] = &[
    ("joint_tactics", "joint-tactics", AdvancedAi::disable_joint_tactics),
    ("joint_reach_lines", "joint-reach-lines", AdvancedAi::disable_joint_reach_lines),
    ("live_trader_route_adapter", "live-trader-route", AdvancedAi::disable_live_trader_route_adapter),
    ("live_religious_purchase_guard", "live-religious-purchase", AdvancedAi::disable_live_religious_purchase_guard),
    ("home_defense", "home-defense", AdvancedAi::disable_home_defense),
    ("recorded_tactical_step", "recorded-tactical-step", AdvancedAi::disable_recorded_tactical_step),
    ("live_motion_turn_accounting", "live-motion-turn-accounting", AdvancedAi::disable_live_motion_turn_accounting),
    ("whole_turn_backtrack_guard", "whole-turn-backtrack-guard", AdvancedAi::disable_whole_turn_backtrack_guard),
    ("step_and_reassess", "step-and-reassess", AdvancedAi::disable_step_and_reassess),
    ("strike_opening", "strike-opening", AdvancedAi::disable_strike_opening),
    ("bounded_recovery", "bounded-recovery", AdvancedAi::disable_bounded_recovery),
    ("army_target_weighs_enemy", "army-target-weighs-enemy", AdvancedAi::disable_army_target_weighs_the_enemy),
    ("peacetime_deterrence", "peacetime-deterrence", AdvancedAi::disable_peacetime_deterrence),
    ("siege_tracks_wall", "siege-tracks-wall", AdvancedAi::disable_siege_tracks_the_wall),
    ("blind_objective_strength", "blind-objective-strength", AdvancedAi::disable_blind_objective_strength),
    ("solvent_faith_army", "solvent-faith-army", AdvancedAi::disable_solvent_faith_army),
    ("loyalty_rate_alarm", "loyalty-rate-alarm", AdvancedAi::disable_loyalty_rate_alarm),
    ("district_coverage", "district-coverage", AdvancedAi::disable_district_coverage),
    ("slot_kind_tiebreak", "slot-kind-tiebreak", AdvancedAi::disable_slot_kind_tiebreak),
    ("come_ashore", "come-ashore", AdvancedAi::disable_come_ashore),
    ("relief_targets_the_siege", "relief-targets-the-siege", AdvancedAi::disable_relief_targets_the_siege),
    ("blind_objective_units", "blind-objective-units", AdvancedAi::disable_blind_objective_units),
    ("housing_districts", "housing-districts", AdvancedAi::disable_housing_districts),
    ("housing_research", "housing-research", AdvancedAi::disable_housing_research),
    ("war_economy", "war-economy", AdvancedAi::disable_war_economy),
    ("war_reinforcement", "war-reinforcement", AdvancedAi::disable_war_reinforcement),
    ("war_patience", "war-patience", AdvancedAi::disable_war_patience),
    ("endgame_war_runway", "endgame-war-runway", AdvancedAi::disable_endgame_war_runway),
    ("wide_map_capacity", "wide-map-capacity", AdvancedAi::disable_wide_map_capacity),
    ("garrison_under_fire", "garrison-under-fire", AdvancedAi::disable_garrison_under_fire),
    ("escort_unstick", "escort-unstick", AdvancedAi::disable_escort_unstick),
    ("live_formationless_settler_shadow", "live-formationless-settler-shadow", AdvancedAi::disable_live_formationless_settler_shadow),
    ("religion_sues_peace", "religion-sues-peace", AdvancedAi::disable_religion_sues_peace),
    ("recon_replacement", "recon-replacement", AdvancedAi::disable_recon_replacement),
    ("stranded_settler_discount", "stranded-settler-discount", AdvancedAi::disable_stranded_settler_discount),
    ("siege_commitment", "siege-commitment", AdvancedAi::disable_siege_commitment),
    ("wonder_ring_settle_value", "wonder-ring-settle-value", AdvancedAi::disable_wonder_ring_settle_value),
    ("amenity_project_preemption", "amenity-project-preemption", AdvancedAi::disable_amenity_project_preemption),
    ("amenity_district_path", "amenity-district-path", AdvancedAi::disable_amenity_district_path),
    ("governor_every_lane", "governor-every-lane", AdvancedAi::disable_governor_every_lane),
    ("live_wonder_race", "live-wonder-race", AdvancedAi::disable_live_wonder_race),
    ("expansion_before_prophet", "expansion-before-prophet", AdvancedAi::disable_expansion_before_prophet),
    ("no_elective_war", "no-elective-war", AdvancedAi::disable_no_elective_war),
    ("fog_land_capacity", "fog-land-capacity", AdvancedAi::disable_fog_land_capacity),
    ("score_horizon", "score-horizon", AdvancedAi::disable_score_horizon),
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
    ("barbarian_hunt", "barbarian-hunt", AdvancedAi::disable_barbarian_hunt),
    ("barbarian_bargain", "barbarian-bargain", AdvancedAi::disable_barbarian_bargain),
    ("barbarian_ranged_answer", "barbarian-ranged-answer", AdvancedAi::disable_barbarian_ranged_answer),
    ("camp_party", "camp-party", AdvancedAi::disable_camp_party),
    ("buildings_before_projects", "buildings-before-projects", AdvancedAi::disable_buildings_before_projects),
    ("deny_while_targeted", "deny-while-targeted", AdvancedAi::disable_deny_while_targeted),
    ("stock_denial_lead_time", "stock-denial-lead-time", AdvancedAi::disable_stock_denial_lead_time),
    ("projected_stock_denial", "projected-stock-denial", AdvancedAi::disable_projected_stock_denial),
    ("parallel_settlers", "parallel-settlers", AdvancedAi::disable_parallel_settlers),
    ("host_settler_pop", "host-settler-pop", AdvancedAi::disable_host_settler_pop),
    ("explore_dead_targets", "explore-dead-targets", AdvancedAi::disable_explore_dead_targets),
    ("explore_commit", "explore-commit", AdvancedAi::disable_explore_commit),
    ("bank_envoys", "bank-envoys", AdvancedAi::disable_bank_envoys),
    ("land_grab", "land-grab", AdvancedAi::disable_land_grab),
    ("siege_is_progress", "siege-is-progress", AdvancedAi::disable_siege_is_progress),
    ("spy_mission_patience", "spy-mission-patience", AdvancedAi::disable_spy_mission_patience),
    ("settler_site_agreement", "settler-site-agreement", AdvancedAi::disable_settler_site_agreement),
    ("civilian_rescue", "civilian-rescue", AdvancedAi::disable_civilian_rescue),
    ("district_building_chain", "district-building-chain", AdvancedAi::disable_district_building_chain),
    ("settler_guard_holds", "settler-guard-holds", AdvancedAi::disable_settler_guard_holds),
    ("expansion_pantheon", "expansion-pantheon", AdvancedAi::disable_expansion_pantheon),
    ("expansion_hall", "expansion-hall", AdvancedAi::disable_expansion_hall),
    ("opening_settler_waits", "opening-settler-waits", AdvancedAi::disable_opening_settler_waits),
];

/// ★★★★ THE MIRROR OF THE TABLE ABOVE, AND IT DID NOT EXIST.
///
/// `LIVE_TREATMENTS` names everything the live bridge ADDS to production, so a
/// live behaviour can be withheld and priced. Nothing named what production
/// itself adds — `promoted_policy_envoy` turns on a dozen behaviours that
/// `configured` leaves off, and each has a `disable_*` twin and an
/// `advanced_without_*` arm in `src/elo.rs`, but no table joined the three. The
/// consequence is small and real: a tool that wants to run "the shipped agent,
/// minus one promoted behaviour" has to be taught each name by hand, which is
/// how `civvis_orders` came to carry 57 hand-written arms against 68 rows.
///
/// ⚠ This table is CORRECT, not COMPLETE: `production_bundle_rows_are_real`
/// checks that every row names a behaviour `promoted_policy_envoy` actually
/// turns on and that its withholding twin exists, and it does not require that
/// every promoted behaviour appear. Claiming completeness would be a claim
/// nothing checks. Add a row when a promotion needs to be withheld by name.
#[rustfmt::skip]
pub const PRODUCTION_TREATMENTS: &[LiveTreatment] = &[
    ("strategic_wonders", "strategic-wonders", AdvancedAi::disable_strategic_wonders),
];

/// Opt-in treatments production ships OFF: the complement of
/// `PRODUCTION_TREATMENTS`. A row here lets an evaluator seat the treated
/// agent by name (`victory_eval --with <tag>`) the way `--without` withholds
/// a shipped behaviour, so an off-by-default arm can be measured in the
/// targeted regime it exists for before any promotion question is asked.
#[rustfmt::skip]
pub const PRODUCTION_OPT_INS: &[LiveTreatment] = &[
    // A Builder does not walk into a visible Barbarian-capture envelope; its
    // target and retreat both use the same fog-honest risk model as Settlers.
    // `gene_screen` discovers this native opt-in directly from this row.
    ("builder_barbarian_safety", "builder-barbarian-safety", AdvancedAi::enable_builder_barbarian_safety),
    // A Builder's finite charges first pay today's worked yields, except for
    // luxury and strategic connections that pay empire-wide either way.
    // `gene_screen` discovers this native opt-in directly from this row.
    ("builder_worked_tile_priority", "builder-worked-tile-priority", AdvancedAi::enable_builder_worked_tile_priority),
    ("apostle_promotion_by_role", "apostle-promotion-by-role", AdvancedAi::enable_apostle_promotion_by_role),
    // The joint engagement search (`docs/TACTICS.md`): production ships it
    // off, the bridge turns it on, and `advanced_joint_tactics` is
    // `AdvancedAi::new()` plus this one flag — so it is a native opt-in like
    // any other. Listed here so `gene_screen` can price it in whole native
    // games beside the engine repairs and `victory_eval --with joint-tactics`
    // can seat it by name. (`joint-reach-lines` rides with it — the enable
    // turns both on; the lines alone are priced on `battle_bench`, §17.)
    ("joint_tactics", "joint-tactics", AdvancedAi::enable_joint_tactics),
    // Item 4 of `docs/LIVE_TACTICS.md`: rear reinforcements arrive at an
    // engaged front as a wave rather than one at a time. Off everywhere
    // until the screen says otherwise; see `AdvancedAi::enable_arrival_waves`.
    // The religion race decides two thirds of native games, and a founder
    // that loses its own cities wins as rarely as a seat that never founded
    // (3.0% v 3.0%, 13,446 seat-pairs). These two are priced against the
    // deployment genome before either ships; see the fields' docs.
    ("inquisition_on_threat", "inquisition-on-threat", AdvancedAi::enable_inquisition_on_threat),
    ("founder_temple", "founder-temple", AdvancedAi::enable_founder_temple),
    ("theology_for_founders", "theology-for-founders", AdvancedAi::enable_theology_for_founders),
    // Half the seats never found a religion and bank ~1,000 Faith they
    // cannot spend; see `AdvancedAi::idle_faith_patronage`.
    ("idle_faith_patronage", "idle-faith-patronage", AdvancedAi::enable_idle_faith_patronage),
    // The rest of the same chain: the founder's corps cannot answer more than
    // two cities, cannot heal when it is defending, spends charges at a
    // fraction of their strength, and never uses the World Congress licence to
    // remove a carrier at peace. See `advanced/religion.rs`.
    ("religious_defence_scales", "religious-defence-scales", AdvancedAi::enable_religious_defence_scales),
    ("guru_heals_the_corps", "guru-heals-the-corps", AdvancedAi::enable_guru_heals_the_corps),
    ("religious_units_heal_first", "religious-units-heal-first", AdvancedAi::enable_religious_units_heal_first),
    ("condemn_under_congress", "condemn-under-congress", AdvancedAi::enable_condemn_under_congress),
    ("spread_campaign_persists", "spread-campaign-persists", AdvancedAi::enable_spread_campaign_persists),
    ("holy_site_where_the_threat_is", "holy-site-where-the-threat-is", AdvancedAi::enable_holy_site_where_the_threat_is),
    ("enhancer_for_the_corps", "enhancer-for-the-corps", AdvancedAi::enable_enhancer_for_the_corps),
    // Every major adopts Early Empire on turns 23-30, and a Scout's sight of 2
    // cannot see a city-state's seat from outside its border. The land route
    // to first contact closes once and never reopens; see
    // `AdvancedAi::early_contact_window`.
    ("early_contact_window", "early-contact-window", AdvancedAi::enable_early_contact_window),
    // A Great Person earned and blocked is a race forfeited: build the slot
    // space ahead of the person, sell duplicate works when nothing can be
    // built; see `AdvancedAi::great_person_housing`.
    ("great_person_housing", "great-person-housing", AdvancedAi::enable_great_person_housing),
    // A surprise war priced on what the board exposes — an unescorted
    // Settler or Builder, a cluster of unpillaged tiles — taken by movement
    // and closed by peace; see `AdvancedAi::opportunistic_war`.
    ("opportunistic_war", "opportunistic-war", AdvancedAi::enable_opportunistic_war),
    // The pillage half of the raid, priced apart: inert unless the row
    // above is on. See `AdvancedAi::raid_pillage_prizes`.
    ("raid_pillage_prizes", "raid-pillage-prizes", AdvancedAi::enable_raid_pillage_prizes),
    // A target can be excellent while a visible hostile makes its next route
    // step unsafe. This holds that corridor aside briefly and sends the
    // Settler to the best safe runner-up; see `settler_threat_detour`.
    ("settler_threat_detour", "settler-threat-detour", AdvancedAi::enable_settler_threat_detour),
    // The site ranking is indifferent between founding now and founding the
    // same value later; this prices every turn of the walk, dearer the longer
    // the Settler has been out. See `settle_sooner`.
    ("settle_sooner", "settle-sooner", AdvancedAi::enable_settle_sooner),
    // A settler prices a site by the districts the plan would build there,
    // and a treasury buys a border plot only when it pays for itself. See
    // `advanced/site_lookahead.rs`.
    // A unit in contact prices the exchange it is standing in, not only the
    // attacks it could make: stand and heal against melee that has to come to
    // it, close on or leave an unanswerable shooter. See
    // `advanced/contact_posture.rs`.
    ("contact_posture", "contact-posture", AdvancedAi::enable_contact_posture),
    ("district_lookahead_settle", "district-lookahead-settle", AdvancedAi::enable_district_lookahead_settle),
    ("priced_tile_purchase", "priced-tile-purchase", AdvancedAi::enable_priced_tile_purchase),
    // `governor-every-lane` is a losing composite: the deployment screen's
    // score-share drag is large enough that the two pre-existing halves need
    // their own randomised comparisons before either can be retained. The
    // composite remains the live-bridge compatibility switch; these opt-ins
    // let a native seat turn on exactly one of its established predicates.
    ("governor_victory_lanes", "governor-victory-lanes", AdvancedAi::enable_governor_victory_lanes),
    ("governor_expansion_lane", "governor-expansion-lane", AdvancedAi::enable_governor_expansion_lane),
    // `withdraw_hp` is a constant and the enemy's damage is not: a unit the
    // strongest thing in reach would kill in one blow recovers whatever its
    // hit points say, and healing ground that comes under a shooter's reach
    // is left. See `BasicAi::one_shot_recovery`.
    ("one_shot_recovery", "one-shot-recovery", AdvancedAi::enable_one_shot_recovery),
    // The six victory-lane genes (`advanced/victory_lane.rs`,
    // `docs/VICTORY_GENES.md`). A targeted seat spends about a fifth of the
    // game — and an adaptive one 15% — with `Expansion` in its plan, and
    // `take_turn_inner` hands that to every lane-shaped decider. Each of the
    // first five substitutes the victory the empire is actually racing at ONE
    // of them; the sixth prices the Diplomatic Victory Points a scored
    // competition pays, which nothing priced.
    ("lane_congress_ballot", "lane-congress-ballot", AdvancedAi::enable_lane_congress_ballot),
    ("lane_congress_favor", "lane-congress-favor", AdvancedAi::enable_lane_congress_favor),
    ("lane_great_people", "lane-great-people", AdvancedAi::enable_lane_great_people),
    ("lane_policy_deck", "lane-policy-deck", AdvancedAi::enable_lane_policy_deck),
    ("lane_culture_spending", "lane-culture-spending", AdvancedAi::enable_lane_culture_spending),
    ("lane_space_race", "lane-space-race", AdvancedAi::enable_lane_space_race),
    ("competition_victory_points", "competition-victory-points", AdvancedAi::enable_competition_victory_points),
    // Three behaviours that already existed and could not be screened: off in
    // production, reachable only as named `elo.rs` arms, so `gene_screen`
    // never saw them. `docs/VICTORY_GENES.md` §9 counts these fields; these
    // are the three the Diplomacy lane needs — and the first has a measured
    // number sitting unused in its own field doc (26 of 192 ballot decisions
    // already settled, ~1.4 free Diplomatic Victory Points a seat a game).
    ("congress_banks_a_decided_vote", "congress-banks-decided", AdvancedAi::enable_congress_banks_a_decided_vote),
    ("congress_counter_votes", "congress-counter-votes", AdvancedAi::enable_congress_counter_votes),
    ("envoy_infrastructure", "envoy-infrastructure", AdvancedAi::enable_envoy_infrastructure),
    // Nothing in this controller reaches the air layer: the melee package
    // skips `domain == "air"` and ranks its unlocks by cheapest remaining
    // research, so it can appoint the next technology and never a chain.
    // This beelines Advanced Flight from three technologies out, raises an
    // Aerodrome and a bomber wing, and takes the appointed city with the
    // cavalry behind it. See `advanced/air_surge.rs`.
    ("air_surge", "air-surge", AdvancedAi::enable_air_surge),
];
