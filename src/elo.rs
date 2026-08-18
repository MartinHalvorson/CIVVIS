//! Elo tournament harness: evaluate AI strategies against each other.
//!
//! The primary rating belongs to a player (a human account or named AI
//! strategy) and follows it across every leader/civilization draw. Separate
//! `(player, leader, civilization)` rows retain matchup diagnostics; leader
//! and civilization are both needed because they are not one-to-one (Eleanor,
//! for example, can lead either England or France). Multiplayer games are
//! scored as simultaneous pairwise results with `K/(n-1)` scaling.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ai::{AdvancedAi, Ai, BasicAi, RandomAi, VictoryTarget, Weights};
use crate::game::{default_speed, Action, Game, GameOptions, VictoryConditions};
use crate::rng::Rng;
use crate::rules::Rules;
use crate::setup::{GameMode, MapPoles, MapScript, MapSize, MapTopology};

pub const BUILTIN_AIS: &[&str] = &[
    "advanced",
    "advanced_evolved",
    "advanced_v1",
    "basic",
    "random",
    "evolved",
    "strategic",
    "strategic_deep",
];

/// Controls intended for paired evaluator experiments, not persistent
/// tournament ratings. Keeping them out of `BUILTIN_AIS` prevents a control
/// factory from being pooled into the same player/leader rating key as
/// its treatment.
pub const EVAL_ONLY_AIS: &[&str] = &[
    // Complete fair-play major controller; paired screening owns the
    // promotion decision before the default controller changes.
    "fog_honest",
    // One pre-registered point on the production genes #1520 opened.
    "advanced_build_first",
    // The native-safe half of the live-bridge bundle, applied to the stock
    // production controller. `live` has only ever been measured against its
    // own ablations, so these are what prices the whole repair set against
    // the `advanced` incumbent it was never run against. War and economy
    // halves exist so the composite's interaction is measurable rather than
    // assumed: if the whole beats `advanced` by more than the halves do, the
    // repairs compound.
    "advanced_synergy",
    "advanced_synergy_war",
    "advanced_synergy_economy",
    // The deployed Civilization VI agent, its six explicit victory-lane
    // configurations, and one arm per live-bridge flag held off. Eval-only by
    // construction: they move whenever the bridge moves, which is exactly
    // what a rating anchor must not do.
    "live",
    "live_target_science",
    "live_target_culture",
    "live_target_religious",
    "live_target_diplomatic",
    "live_target_domination",
    "live_target_score",
    "live_without_amenity_project_preemption",
    "live_without_amenity_district_path",
    "live_without_governor_every_lane",
    "live_without_live_wonder_race",
    "live_without_expansion_before_prophet",
    "live_without_no_elective_war",
    "live_without_fog_land_capacity",
    "live_without_home_defense",
    "live_without_joint_tactics",
    "live_without_loyalty_policy_defence",
    "live_without_solvent_faith_army",
    "live_without_siege_muster",
    "live_without_district_coverage",
    "live_without_slot_kind_tiebreak",
    "live_without_bounded_recovery",
    "live_without_army_target_weighs_enemy",
    "live_without_peacetime_deterrence",
    "live_without_siege_tracks_wall",
    "live_without_siege_role",
    "live_without_come_ashore",
    "live_without_suzerain_cards",
    "live_without_blind_objective_strength",
    "live_without_muster_at_command_radius",
    "live_without_relief_targets_the_siege",
    "live_without_blind_objective_units",
    "live_without_loyalty_rate_alarm",
    "live_without_housing_districts",
    "live_without_housing_buildings",
    "live_without_campus_every_city",
    "live_without_housing_cards",
    "live_without_housing_research",
    "live_without_war_economy",
    "live_without_war_reinforcement",
    "live_without_war_patience",
    "live_without_deny_while_targeted",
    "live_without_stock_denial_lead_time",
    "live_without_endgame_war_runway",
    "live_without_stacked_escort",
    "live_without_counter_in_lane",
    "live_without_era_paced_expansion",
    "live_without_escort_unstick",
    "live_without_frontier_loyalty",
    "live_without_garrison_under_fire",
    "live_without_garrison_walls",
    "live_without_naval_recon",
    "live_without_recon_flight",
    "live_without_recon_replacement",
    "live_without_religion_sues_peace",
    "live_without_score_horizon",
    "live_without_siege_commitment",
    "live_without_stranded_settler_discount",
    "live_without_tally_culture",
    "live_without_wide_map_capacity",
    "live_without_wonder_ring_settle_value",
    "live_without_live_trader_route_adapter",
    "live_without_live_religious_purchase_guard",
    "live_without_recorded_tactical_step",
    "live_without_strike_opening",
    "live_without_ranged_needs_line_of_sight",
    "live_without_one_launch_pad",
    "live_without_culture_building_debt",
    "live_without_culture_coverage",
    "live_without_settler_target_hysteresis",
    "live_without_tally_great_people",
    "live_without_barbarian_scouts_are_scouts",
    "live_without_camp_reach",
    "live_without_settler_stack_discipline",
    "live_without_camp_party",
    "live_without_buildings_before_projects",
    "live_without_parallel_settlers",
    "live_without_host_settler_pop",
    "live_without_explore_dead_targets",
    "live_without_explore_commit",
    "live_without_bank_envoys",
    "basic_evolved",
    "advanced_policy_live_control",
    "advanced_policy_envoy_priority",
    "advanced_envoy_policy",
    "advanced_envoy_infrastructure",
    "advanced_envoy_priority",
    "advanced_envoy_economy",
    "advanced_congress_counter",
    "advanced_congress_votes",
    "advanced_congress_counter_hard",
    "advanced_banking_dedication",
    "advanced_blind_to_leaders",
    "advanced_rush",
    "advanced_rush_connected",
    "advanced_timing_attack",
    "advanced_timing_attack_selective",
    "advanced_timing_attack_rapid",
    "advanced_city_strategy",
    "advanced_city_strategy_emphasis",
    "advanced_city_strategy_roles",
    "advanced_city_strategy_roles_raw",
    "advanced_city_strategy_raw",
    "advanced_city_strategy_bastion_only",
    "advanced_city_strategy_breadbasket_only",
    "advanced_city_strategy_comparative_only",
    "advanced_city_strategy_pressure_only",
    "advanced_belief_pressure",
    "advanced_civ_blind",
    "advanced_counter_in_lane",
    "advanced_counter_stand_down",
    "advanced_early_score_alarm",
    "advanced_early_score_build",
    "advanced_evolved_blind",
    "advanced_settler_commit",
    "advanced_wide_opening",
    "advanced_war_half",
    "advanced_plan_city_target",
    "advanced_expansion_payback",
    "advanced_late_expansion",
    "advanced_expansion_dispatch",
    "advanced_expansion_complete",
    "advanced_coupled_expansion",
    // The victory lane the deployed Civ 6 decider is actually given. Named so
    // `ai_eval` can measure the choice `civ6_civvis_climb.py --victory` makes
    // for every real run; see the constructor arms below.
    "advanced_target_domination",
    "advanced_target_score",
    // ⚠⚠ AND THE OTHER FOUR, which had no arm at all. `VictoryTarget` has six
    // variants; two were registered here, so the lane the deployed decider is
    // handed could be measured only if that lane happened to be Domination or
    // Score. `civ6_civvis_climb.py --victory` can now select all six (#1871),
    // and `victory_eval` at the ladder's own profile finishes four of the five
    // named conditions inside its clock while Science — the deployed default —
    // finishes none. Which lane is STRONGEST is a different question from which
    // completes, and these are the arms that can answer it.
    "advanced_target_science",
    // Hold the target fixed and price whether the non-Culture building veto is
    // keyed to a Great Work slot or the Theater Square district itself.
    "advanced_great_work_veto_by_district",
    "advanced_target_culture",
    // The Culture lane is the only targeted lane where its Theater-building
    // debt can pass the non-Culture Great Work veto and actually be reached.
    "advanced_target_culture_with_culture_building_debt",
    "advanced_target_religious",
    "advanced_target_diplomatic",
    "advanced_measured_dedication",
    "advanced_garrison_loyalty",
    "advanced_settler_first",
    "advanced_holy_priority",
    "advanced_holy_lane",
    "advanced_holy_v0",
    "advanced_settle_food",
    "advanced_holy_lane_v0",
    "advanced_roster_live",
    "advanced_roster_live_keep_districts",
    "advanced_diplomatic_opening",
    "advanced_without_bounded_recovery",
    "advanced_without_city_target_floor",
    "advanced_without_plan_city_target",
    "advanced_without_settler_commit",
    "advanced_without_unpriced_bundle",
    "advanced_without_settlement_safety",
    "advanced_without_battlefront_observation",
    "advanced_lower_city_target",
    "advanced_settler_founds_when_stalled",
    "advanced_fortify_idle_units",
    "advanced_open_water_navy",
    "advanced_maritime_splice",
    "advanced_sea_answers",
    "advanced_without_barbarian_scouts_are_scouts",
    "advanced_engine_faith_price",
    "advanced_maintenance_deck",
    "advanced_recon_fleet",
    "advanced_without_recon_fleet",
    // The two production value/cost treatments: the Builder priced by a
    // survey of the work it would do, and military units credited for
    // strength-per-production and the civ's own unique window. Reserved
    // matrix seeds 25000000 (builder survey) and 26000000 (unit
    // efficiency); one pre-registered run each, not swept.
    // The strategic governor under every lane, applied natively. The flag
    // ships on the live bridge (#1742/#1793) and has a live withhold arm,
    // but under Science/Culture/Religion/Diplomacy the NATIVE production
    // controller still routes through `BasicAi::pick_item`, so every
    // valuation in advanced.rs is absent exactly where strong games spend
    // their midgame. Reserved matrix seed 27000000; one pre-registered run.
    "advanced_every_lane",
    "advanced_builder_survey",
    "advanced_unit_efficiency",
    "advanced_without_unpriced_economy",
    "advanced_without_unpriced_war",
    "advanced_without_city_defence",
    "advanced_legacy_policy_deck",
    "advanced_without_builder_floor",
    "advanced_without_settler_deadline",
    "advanced_without_hut_collection",
    "advanced_without_explore_commit",
    "advanced_without_village_seeking",
    "advanced_price_suzerainty",
    "advanced_without_unit_tactics",
    "advanced_league_top",
    "advanced_joint_tactics",
    "strategic_cheap",
    "strategic_score",
    "strategic_doctrine",
    "strategic_joint",
    "strategic_r20",
    "strategic_r10",
    "strategic_nodefer",
    "strategic_r20h20",
    "strategic_h80",
    "strategic_rot20",
    "strategic_rot10",
    "strategic_deep",
    "strategic_ultra",
    "strategic_deep_default",
    "strategic_deep_tempo",
    "strategic_deep_conversion",
    "strategic_deep_checkmate",
    "strategic_deep_expand",
    "strategic_deep_consolidate",
    "strategic_deep_militarize",
    "strategic_deep_league",
    "strategic_warm",
    "strategic_cold",
    "strategic_noprophet",
    "strategic_deep_adaptive",
    "strategic_rivals",
    "strategic_deep_rivals",
];

/// Every mechanism the Civilization VI bridge turns on, as evaluator treatment
/// tags. `live` carries all of them; each `live_without_*` carries all but one,
/// so `differing_axes` between them names exactly the mechanism under test.
///
/// ⚠ Keep in step with `AdvancedAi::enable_live_bridge`. A flag added there and
/// not here makes the arms claim a controlled comparison they are not running.
/// ⚠⚠ PUBLIC SO A RUN CAN SAY WHAT ITS BINARY CARRIES. A stale binary is
/// invisible today: `summary.json` records no revision at all, and three of
/// thirty-two recent live runs were executing a pre-fix build — detectable only
/// because they emitted a build name a later commit had already corrected, a
/// trick that will not work for the next one. Emitting this list per run makes
/// staleness self-describing (an old binary emits a shorter list) and tells any
/// A/B exactly which repairs were live in the arm it measured.
pub const LIVE_BRIDGE_TREATMENTS: &[&str] = &[
    "joint-tactics",
    "live-trader-route",
    "live-religious-purchase",
    "siege-muster",
    "home-defense",
    "loyalty-policy-defence",
    "recorded-tactical-step",
    "strike-opening",
    "bounded-recovery",
    "army-target-weighs-enemy",
    "peacetime-deterrence",
    "siege-tracks-wall",
    "blind-objective-strength",
    "solvent-faith-army",
    "loyalty-rate-alarm",
    "ranged-line-of-sight",
    "district-coverage",
    "slot-kind-tiebreak",
    "siege-role",
    "come-ashore",
    "relief-targets-the-siege",
    "blind-objective-units",
    "suzerain-cards",
    "muster-at-command-radius",
    "housing-districts",
    "campus-every-city",
    "housing-cards",
    "housing-research",
    "war-economy",
    "war-reinforcement",
    "war-patience",
    "endgame-war-runway",
    "wide-map-capacity",
    "garrison-under-fire",
    "escort-unstick",
    "stacked-escort",
    "religion-sues-peace",
    "recon-replacement",
    "stranded-settler-discount",
    "siege-commitment",
    "wonder-ring-settle-value",
    "garrison-walls",
    "housing-buildings",
    "amenity-project-preemption",
    "amenity-district-path",
    "governor-every-lane",
    "live-wonder-race",
    "expansion-before-prophet",
    "no-elective-war",
    "fog-land-capacity",
    "recon-flight",
    "score-horizon",
    "one-launch-pad",
    "naval-recon",
    "counter-in-lane",
    "era-paced-expansion",
    "tally-culture",
    "culture-building-debt",
    "culture-coverage",
    "frontier-loyalty",
    "settler-target-hysteresis",
    "tally-great-people",
    "barbarian-scouts-are-scouts",
    "camp-reach",
    "settler-stack-discipline",
    "camp-party",
    "buildings-before-projects",
    "deny-while-targeted",
    "stock-denial-lead-time",
    "parallel-settlers",
    "host-settler-pop",
    "explore-dead-targets",
    "explore-commit",
    "bank-envoys",
];

/// Every explicit `civvis_orders --victory` configuration which is both
/// target-pinned and carries the deployed bridge. `live` deliberately stays
/// adaptive (`--victory civvis`); these arms model the other six command-line
/// choices without making a second, hand-maintained target or tag table.
const LIVE_TARGET_LANES: &[(&str, VictoryTarget, &str)] = &[
    ("science", VictoryTarget::Science, "victory-lane-science"),
    ("culture", VictoryTarget::Culture, "victory-lane-culture"),
    (
        "religious",
        VictoryTarget::Religion,
        "victory-lane-religious",
    ),
    (
        "diplomatic",
        VictoryTarget::Diplomacy,
        "victory-lane-diplomatic",
    ),
    (
        "domination",
        VictoryTarget::Domination,
        "victory-lane-domination",
    ),
    ("score", VictoryTarget::Score, "victory-lane-score"),
];

/// Each target-pinned live arm adds exactly its victory-lane tag ahead of the
/// deployed bridge's tags. Deriving the lists makes a bridge repair appear in
/// every target configuration at once rather than silently changing only
/// adaptive `live`.
static LIVE_TARGET_TREATMENTS: std::sync::LazyLock<BTreeMap<&'static str, Vec<&'static str>>> =
    std::sync::LazyLock::new(|| {
        LIVE_TARGET_LANES
            .iter()
            .map(|(lane, _, axis)| {
                (
                    *lane,
                    std::iter::once(*axis)
                        .chain(LIVE_BRIDGE_TREATMENTS.iter().copied())
                        .collect(),
                )
            })
            .collect()
    });

fn live_targeted(lane: &'static str) -> AdvancedAi {
    let target = LIVE_TARGET_LANES
        .iter()
        .find(|(known, _, _)| *known == lane)
        .map(|(_, target, _)| *target)
        .unwrap_or_else(|| panic!("{lane} is not an explicit live victory lane"));
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_live_bridge();
    ai
}

fn live_target_treatments(lane: &'static str) -> &'static [&'static str] {
    LIVE_TARGET_TREATMENTS
        .get(lane)
        .unwrap_or_else(|| panic!("{lane} is not an explicit live victory lane"))
}

/// Every `live_without_*` control's tag list: the bridge list minus the one
/// withheld tag, in bridge order — exactly the lists the arms carried when
/// they were written out by hand, now derived so a new bridge treatment
/// updates all of them at once.
static LIVE_WITHOUT_TREATMENTS: std::sync::LazyLock<BTreeMap<&'static str, Vec<&'static str>>> =
    std::sync::LazyLock::new(|| {
        LIVE_BRIDGE_TREATMENTS
            .iter()
            .map(|withheld| {
                (
                    *withheld,
                    LIVE_BRIDGE_TREATMENTS
                        .iter()
                        .copied()
                        .filter(|tag| tag != withheld)
                        .collect(),
                )
            })
            .collect()
    });

fn live_without(withheld: &'static str) -> &'static [&'static str] {
    LIVE_WITHOUT_TREATMENTS
        .get(withheld)
        .unwrap_or_else(|| panic!("{withheld} is not a live-bridge treatment"))
}

/// The deployment-profile treatments that stay out of the native bundle, as
/// tags. Some encode a rule of Firaxis' game, while others price host-only
/// conditions or are already present in the native production baseline. See
/// `AdvancedAi::enable_engine_repairs`.
pub const FIRAXIS_ONLY_TREATMENTS: &[&str] = &[
    "joint-tactics",
    "live-trader-route",
    "live-religious-purchase",
    "solvent-faith-army",
    // Prices a Firaxis-specific opportunity — an uncontested wonder catalogue
    // on the Settler seat, and a score tally at the host's turn limit — that
    // CIVVIS-vs-CIVVIS games do not offer.
    "live-wonder-race",
    // Prices the Settler seat's slow Prophet race: the third city comes before
    // the Holy Site. CIVVIS-vs-CIVVIS contenders are the real race the stock
    // order was written for.
    "expansion-before-prophet",
    // Prices the Settler seat's measured record: eight elective wars, no city
    // ever taken. CIVVIS-vs-CIVVIS wars are the ones the branch was written for.
    "no-elective-war",
    // Reads the live mirror's fog: a native board carries no unknown terrain,
    // so the estimate equals the count there and the flag is a no-op.
    "fog-land-capacity",
    // Prices the Settler seat's last-quarter score-leader war record; the
    // native response shape is measured by its own `advanced_counter_*` arms.
    "counter-in-lane",
    // Prices the Settler seat's uncontested land at its own era pace; the
    // league cadence was bred against CIVVIS rivals who contest the ground.
    "era-paced-expansion",
    // Prices the Settler seat's tally (three a civic, two a tech); the native
    // lanes keep their bred yield weights.
    "tally-culture",
    // Pays the Settler seat's tally for the Theater Square's own chain; the
    // native lanes keep their bred building debts.
    "culture-building-debt",
    // Pays the Settler seat's tally for the Theater Square the empire has not
    // got; the native lanes keep their bred district coverage.
    "culture-coverage",
    // Reads the live mirror's fog around a settle site; the native forecast
    // sees every rival city.
    "frontier-loyalty",
    // Prices the Settler seat's tally (five a Great Person); the native lanes
    // keep the bred closeness limit.
    "tally-great-people",
    // Only a seat playing under an assigned lane (`--victory science`, the
    // Settler seat's standing order) has a target gate to override; the
    // native gate agents are adaptive, so the flag cannot fire there.
    "deny-while-targeted",
    // Priced on the Settler seat's own steal record (five led games taken at
    // t229-245); the native lanes end on their own clock and keep the
    // measured 90 bar until a native run says otherwise.
    "stock-denial-lead-time",
    // These four react to host movement or host production semantics; native
    // CIVVIS has neither distinction, so the repair bundle must not imply
    // they are native engine changes.
    "parallel-settlers",
    "host-settler-pop",
    "explore-dead-targets",
    "bank-envoys",
    // Production Advanced already carries committed exploration. It remains
    // a live treatment so the deployment bundle and its ablation registry are
    // complete, not because `advanced_synergy` needs to turn it on again.
    "explore-commit",
];

/// The military half of the native repair bundle: force assembly, marching,
/// siege, threat reading, and the war/peace decision.
pub const ENGINE_REPAIR_WAR_TREATMENTS: &[&str] = &[
    "muster-at-command-radius",
    "war-reinforcement",
    "come-ashore",
    "recorded-tactical-step",
    "blind-objective-strength",
    "blind-objective-units",
    "relief-targets-the-siege",
    "army-target-weighs-enemy",
    "peacetime-deterrence",
    "war-economy",
    "bounded-recovery",
    "siege-muster",
    "siege-role",
    "siege-tracks-wall",
    "siege-commitment",
    "war-patience",
    "endgame-war-runway",
    "home-defense",
    "garrison-under-fire",
    "garrison-walls",
    "strike-opening",
    "ranged-line-of-sight",
    "recon-replacement",
    "recon-flight",
    "barbarian-scouts-are-scouts",
    "naval-recon",
    "camp-reach",
    "camp-party",
    "religion-sues-peace",
];

/// The economic half: settlement, growth, districts, and the policy deck.
pub const ENGINE_REPAIR_ECONOMY_TREATMENTS: &[&str] = &[
    "escort-unstick",
    "stacked-escort",
    "settler-stack-discipline",
    "buildings-before-projects",
    "wonder-ring-settle-value",
    "stranded-settler-discount",
    "wide-map-capacity",
    "housing-districts",
    "housing-buildings",
    "amenity-project-preemption",
    "amenity-district-path",
    "governor-every-lane",
    "housing-cards",
    "housing-research",
    "campus-every-city",
    "district-coverage",
    "slot-kind-tiebreak",
    "loyalty-policy-defence",
    "loyalty-rate-alarm",
    "suzerain-cards",
    "score-horizon",
    "one-launch-pad",
    "settler-target-hysteresis",
];

/// Every live-bridge repair that fixes a CIVVIS engine defect, as evaluator
/// tags — `LIVE_BRIDGE_TREATMENTS` minus `FIRAXIS_ONLY_TREATMENTS`, and the
/// union of the two halves above. `engine_repair_tags_partition_the_bridge`
/// fails if any of those three relationships stops holding.
pub const ENGINE_REPAIR_TREATMENTS: &[&str] = &[
    "muster-at-command-radius",
    "war-reinforcement",
    "come-ashore",
    "recorded-tactical-step",
    "blind-objective-strength",
    "blind-objective-units",
    "relief-targets-the-siege",
    "army-target-weighs-enemy",
    "peacetime-deterrence",
    "war-economy",
    "bounded-recovery",
    "siege-muster",
    "siege-role",
    "siege-tracks-wall",
    "siege-commitment",
    "war-patience",
    "endgame-war-runway",
    "home-defense",
    "garrison-under-fire",
    "garrison-walls",
    "strike-opening",
    "ranged-line-of-sight",
    "recon-replacement",
    "recon-flight",
    "barbarian-scouts-are-scouts",
    "naval-recon",
    "camp-reach",
    "camp-party",
    "religion-sues-peace",
    "escort-unstick",
    "stacked-escort",
    "settler-stack-discipline",
    "buildings-before-projects",
    "wonder-ring-settle-value",
    "stranded-settler-discount",
    "wide-map-capacity",
    "housing-districts",
    "housing-buildings",
    "amenity-project-preemption",
    "amenity-district-path",
    "governor-every-lane",
    "housing-cards",
    "housing-research",
    "campus-every-city",
    "district-coverage",
    "slot-kind-tiebreak",
    "loyalty-policy-defence",
    "loyalty-rate-alarm",
    "suzerain-cards",
    "score-horizon",
    "one-launch-pad",
    "settler-target-hysteresis",
];

/// Register a selectable arm once, under a typed identity.  The factory,
/// artifact resolver, provenance report, and evaluator-collapse guard all use
/// this identity rather than maintaining separate string matches.
macro_rules! define_arm_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        enum ArmKind {
            $($variant),+
        }

        impl ArmKind {
            fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }

            const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }
        }
    };
}

define_arm_kinds! {
    // The deployed Civilization VI agent, its explicit target configurations,
    // and one arm per live-bridge flag held off, so each can be priced in
    // cities and score.
    Live => "live",
    LiveTargetScience => "live_target_science",
    LiveTargetCulture => "live_target_culture",
    LiveTargetReligious => "live_target_religious",
    LiveTargetDiplomatic => "live_target_diplomatic",
    LiveTargetDomination => "live_target_domination",
    LiveTargetScore => "live_target_score",
    LiveWithoutAmenityProjectPreemption => "live_without_amenity_project_preemption",
    LiveWithoutAmenityDistrictPath => "live_without_amenity_district_path",
    LiveWithoutGovernorEveryLane => "live_without_governor_every_lane",
    LiveWithoutLiveWonderRace => "live_without_live_wonder_race",
    LiveWithoutExpansionBeforeProphet => "live_without_expansion_before_prophet",
    LiveWithoutNoElectiveWar => "live_without_no_elective_war",
    LiveWithoutFogLandCapacity => "live_without_fog_land_capacity",
    LiveWithoutHomeDefense => "live_without_home_defense",
    LiveWithoutJointTactics => "live_without_joint_tactics",
    LiveWithoutLoyaltyPolicyDefence => "live_without_loyalty_policy_defence",
    LiveWithoutSolventFaithArmy => "live_without_solvent_faith_army",
    LiveWithoutSiegeMuster => "live_without_siege_muster",
    LiveWithoutDistrictCoverage => "live_without_district_coverage",
    LiveWithoutSlotKindTiebreak => "live_without_slot_kind_tiebreak",
    LiveWithoutBoundedRecovery => "live_without_bounded_recovery",
    LiveWithoutArmyTargetWeighsEnemy => "live_without_army_target_weighs_enemy",
    LiveWithoutPeacetimeDeterrence => "live_without_peacetime_deterrence",
    LiveWithoutSiegeTracksWall => "live_without_siege_tracks_wall",
    LiveWithoutSiegeRole => "live_without_siege_role",
    LiveWithoutComeAshore => "live_without_come_ashore",
    LiveWithoutSuzerainCards => "live_without_suzerain_cards",
    LiveWithoutBlindObjectiveStrength => "live_without_blind_objective_strength",
    LiveWithoutMusterAtCommandRadius => "live_without_muster_at_command_radius",
    LiveWithoutReliefTargetsTheSiege => "live_without_relief_targets_the_siege",
    LiveWithoutBlindObjectiveUnits => "live_without_blind_objective_units",
    LiveWithoutLoyaltyRateAlarm => "live_without_loyalty_rate_alarm",
    LiveWithoutHousingDistricts => "live_without_housing_districts",
    LiveWithoutHousingBuildings => "live_without_housing_buildings",
    LiveWithoutCampusEveryCity => "live_without_campus_every_city",
    LiveWithoutHousingCards => "live_without_housing_cards",
    LiveWithoutHousingResearch => "live_without_housing_research",
    LiveWithoutWarEconomy => "live_without_war_economy",
    LiveWithoutWarReinforcement => "live_without_war_reinforcement",
    LiveWithoutWarPatience => "live_without_war_patience",
    LiveWithoutDenyWhileTargeted => "live_without_deny_while_targeted",
    LiveWithoutStockDenialLeadTime => "live_without_stock_denial_lead_time",
    LiveWithoutEndgameWarRunway => "live_without_endgame_war_runway",
    LiveWithoutCounterInLane => "live_without_counter_in_lane",
    LiveWithoutEraPacedExpansion => "live_without_era_paced_expansion",
    LiveWithoutEscortUnstick => "live_without_escort_unstick",
    LiveWithoutFrontierLoyalty => "live_without_frontier_loyalty",
    LiveWithoutGarrisonUnderFire => "live_without_garrison_under_fire",
    LiveWithoutGarrisonWalls => "live_without_garrison_walls",
    LiveWithoutNavalRecon => "live_without_naval_recon",
    LiveWithoutReconFlight => "live_without_recon_flight",
    LiveWithoutReconReplacement => "live_without_recon_replacement",
    LiveWithoutReligionSuesPeace => "live_without_religion_sues_peace",
    LiveWithoutScoreHorizon => "live_without_score_horizon",
    LiveWithoutSiegeCommitment => "live_without_siege_commitment",
    LiveWithoutStrandedSettlerDiscount => "live_without_stranded_settler_discount",
    LiveWithoutTallyCulture => "live_without_tally_culture",
    LiveWithoutWideMapCapacity => "live_without_wide_map_capacity",
    LiveWithoutWonderRingSettleValue => "live_without_wonder_ring_settle_value",
    LiveWithoutStackedEscort => "live_without_stacked_escort",
    LiveWithoutLiveTraderRouteAdapter => "live_without_live_trader_route_adapter",
    LiveWithoutLiveReligiousPurchaseGuard => "live_without_live_religious_purchase_guard",
    LiveWithoutRecordedTacticalStep => "live_without_recorded_tactical_step",
    LiveWithoutStrikeOpening => "live_without_strike_opening",
    LiveWithoutRangedNeedsLineOfSight => "live_without_ranged_needs_line_of_sight",
    LiveWithoutOneLaunchPad => "live_without_one_launch_pad",
    LiveWithoutCultureBuildingDebt => "live_without_culture_building_debt",
    LiveWithoutCultureCoverage => "live_without_culture_coverage",
    LiveWithoutSettlerTargetHysteresis => "live_without_settler_target_hysteresis",
    LiveWithoutTallyGreatPeople => "live_without_tally_great_people",
    LiveWithoutBarbarianScoutsAreScouts => "live_without_barbarian_scouts_are_scouts",
    LiveWithoutCampReach => "live_without_camp_reach",
    LiveWithoutSettlerStackDiscipline => "live_without_settler_stack_discipline",
    LiveWithoutCampParty => "live_without_camp_party",
    LiveWithoutBuildingsBeforeProjects => "live_without_buildings_before_projects",
    LiveWithoutParallelSettlers => "live_without_parallel_settlers",
    LiveWithoutHostSettlerPop => "live_without_host_settler_pop",
    LiveWithoutExploreDeadTargets => "live_without_explore_dead_targets",
    LiveWithoutExploreCommit => "live_without_explore_commit",
    LiveWithoutBankEnvoys => "live_without_bank_envoys",
    Advanced => "advanced",
    FogHonest => "fog_honest",
    AdvancedBankingDedication => "advanced_banking_dedication",
    AdvancedBuildFirst => "advanced_build_first",
    AdvancedSynergy => "advanced_synergy",
    AdvancedSynergyWar => "advanced_synergy_war",
    AdvancedSynergyEconomy => "advanced_synergy_economy",
    AdvancedBeliefPressure => "advanced_belief_pressure",
    AdvancedBlindToLeaders => "advanced_blind_to_leaders",
    AdvancedCityStrategy => "advanced_city_strategy",
    AdvancedCityStrategyBastionOnly => "advanced_city_strategy_bastion_only",
    AdvancedCityStrategyBreadbasketOnly => "advanced_city_strategy_breadbasket_only",
    AdvancedCityStrategyComparativeOnly => "advanced_city_strategy_comparative_only",
    AdvancedCityStrategyEmphasis => "advanced_city_strategy_emphasis",
    AdvancedCityStrategyPressureOnly => "advanced_city_strategy_pressure_only",
    AdvancedCityStrategyRaw => "advanced_city_strategy_raw",
    AdvancedCityStrategyRoles => "advanced_city_strategy_roles",
    AdvancedCityStrategyRolesRaw => "advanced_city_strategy_roles_raw",
    AdvancedCivBlind => "advanced_civ_blind",
    AdvancedCongressCounter => "advanced_congress_counter",
    AdvancedCongressCounterHard => "advanced_congress_counter_hard",
    AdvancedCongressVotes => "advanced_congress_votes",
    AdvancedCounterInLane => "advanced_counter_in_lane",
    AdvancedCounterStandDown => "advanced_counter_stand_down",
    AdvancedEarlyScoreAlarm => "advanced_early_score_alarm",
    AdvancedEarlyScoreBuild => "advanced_early_score_build",
    AdvancedEnvoyEconomy => "advanced_envoy_economy",
    AdvancedEnvoyInfrastructure => "advanced_envoy_infrastructure",
    AdvancedEnvoyPolicy => "advanced_envoy_policy",
    AdvancedEnvoyPriority => "advanced_envoy_priority",
    AdvancedEvolved => "advanced_evolved",
    AdvancedEvolvedBlind => "advanced_evolved_blind",
    AdvancedExpansionComplete => "advanced_expansion_complete",
    AdvancedExpansionDispatch => "advanced_expansion_dispatch",
    AdvancedExpansionPayback => "advanced_expansion_payback",
    AdvancedCoupledExpansion => "advanced_coupled_expansion",
    AdvancedJointTactics => "advanced_joint_tactics",
    AdvancedLateExpansion => "advanced_late_expansion",
    AdvancedLeagueTop => "advanced_league_top",
    AdvancedMeasuredDedication => "advanced_measured_dedication",
    AdvancedPlanCityTarget => "advanced_plan_city_target",
    AdvancedPolicyLiveControl => "advanced_policy_live_control",
    AdvancedPolicyEnvoyPriority => "advanced_policy_envoy_priority",
    AdvancedRush => "advanced_rush",
    AdvancedRushConnected => "advanced_rush_connected",
    AdvancedGarrisonLoyalty => "advanced_garrison_loyalty",
    AdvancedTimingAttack => "advanced_timing_attack",
    AdvancedTimingAttackSelective => "advanced_timing_attack_selective",
    AdvancedTimingAttackRapid => "advanced_timing_attack_rapid",
    AdvancedSettlerCommit => "advanced_settler_commit",
    AdvancedSettlerFirst => "advanced_settler_first",
    AdvancedHolyPriority => "advanced_holy_priority",
    AdvancedHolyLane => "advanced_holy_lane",
    AdvancedHolyV0 => "advanced_holy_v0",
    AdvancedSettleFood => "advanced_settle_food",
    AdvancedHolyLaneV0 => "advanced_holy_lane_v0",
    AdvancedRosterLive => "advanced_roster_live",
    AdvancedRosterLiveKeepDistricts => "advanced_roster_live_keep_districts",
    AdvancedDiplomaticOpening => "advanced_diplomatic_opening",
    AdvancedWithoutBoundedRecovery => "advanced_without_bounded_recovery",
    AdvancedWithoutCityTargetFloor => "advanced_without_city_target_floor",
    AdvancedWithoutPlanCityTarget => "advanced_without_plan_city_target",
    AdvancedWithoutSettlerCommit => "advanced_without_settler_commit",
    AdvancedWithoutUnpricedBundle => "advanced_without_unpriced_bundle",
    AdvancedWithoutSettlementSafety => "advanced_without_settlement_safety",
    AdvancedWithoutBattlefrontObservation => "advanced_without_battlefront_observation",
    AdvancedLowerCityTarget => "advanced_lower_city_target",
    AdvancedSettlerFoundsWhenStalled => "advanced_settler_founds_when_stalled",
    AdvancedFortifyIdleUnits => "advanced_fortify_idle_units",
    AdvancedOpenWaterNavy => "advanced_open_water_navy",
    AdvancedMaritimeSplice => "advanced_maritime_splice",
    AdvancedSeaAnswers => "advanced_sea_answers",
    AdvancedWithoutBarbarianScoutExemption => "advanced_without_barbarian_scouts_are_scouts",
    AdvancedEngineFaithPrice => "advanced_engine_faith_price",
    AdvancedMaintenanceDeck => "advanced_maintenance_deck",
    AdvancedReconFleet => "advanced_recon_fleet",
    AdvancedWithoutReconFleet => "advanced_without_recon_fleet",
    AdvancedEveryLane => "advanced_every_lane",
    AdvancedBuilderSurvey => "advanced_builder_survey",
    AdvancedUnitEfficiency => "advanced_unit_efficiency",
    AdvancedWithoutUnpricedEconomy => "advanced_without_unpriced_economy",
    AdvancedWithoutUnpricedWar => "advanced_without_unpriced_war",
    AdvancedWithoutCityDefence => "advanced_without_city_defence",
    AdvancedLegacyPolicyDeck => "advanced_legacy_policy_deck",
    AdvancedWithoutBuilderFloor => "advanced_without_builder_floor",
    AdvancedWithoutSettlerDeadline => "advanced_without_settler_deadline",
    AdvancedWithoutHutCollection => "advanced_without_hut_collection",
    AdvancedWithoutExploreCommit => "advanced_without_explore_commit",
    AdvancedWithoutVillageSeeking => "advanced_without_village_seeking",
    AdvancedPriceSuzerainty => "advanced_price_suzerainty",
    AdvancedWithoutUnitTactics => "advanced_without_unit_tactics",
    AdvancedTargetDomination => "advanced_target_domination",
    AdvancedTargetScore => "advanced_target_score",
    AdvancedTargetScience => "advanced_target_science",
    AdvancedGreatWorkVetoByDistrict => "advanced_great_work_veto_by_district",
    AdvancedTargetCulture => "advanced_target_culture",
    AdvancedTargetCultureWithCultureBuildingDebt => "advanced_target_culture_with_culture_building_debt",
    AdvancedTargetReligious => "advanced_target_religious",
    AdvancedTargetDiplomatic => "advanced_target_diplomatic",
    AdvancedV1 => "advanced_v1",
    AdvancedWarHalf => "advanced_war_half",
    AdvancedWideOpening => "advanced_wide_opening",
    Basic => "basic",
    BasicEvolved => "basic_evolved",
    Evolved => "evolved",
    Random => "random",
    Strategic => "strategic",
    StrategicCheap => "strategic_cheap",
    StrategicCold => "strategic_cold",
    StrategicDeep => "strategic_deep",
    StrategicDeepAdaptive => "strategic_deep_adaptive",
    StrategicDeepCheckmate => "strategic_deep_checkmate",
    StrategicDeepConsolidate => "strategic_deep_consolidate",
    StrategicDeepConversion => "strategic_deep_conversion",
    StrategicDeepDefault => "strategic_deep_default",
    StrategicDeepExpand => "strategic_deep_expand",
    StrategicDeepLeague => "strategic_deep_league",
    StrategicDeepMilitarize => "strategic_deep_militarize",
    StrategicDeepRivals => "strategic_deep_rivals",
    StrategicDeepTempo => "strategic_deep_tempo",
    StrategicDoctrine => "strategic_doctrine",
    StrategicH80 => "strategic_h80",
    StrategicJoint => "strategic_joint",
    StrategicNodefer => "strategic_nodefer",
    StrategicNoprophet => "strategic_noprophet",
    StrategicR10 => "strategic_r10",
    StrategicR20 => "strategic_r20",
    StrategicR20H20 => "strategic_r20h20",
    StrategicRivals => "strategic_rivals",
    StrategicRot10 => "strategic_rot10",
    StrategicRot20 => "strategic_rot20",
    StrategicScore => "strategic_score",
    StrategicUltra => "strategic_ultra",
    StrategicWarm => "strategic_warm",
}

/// On-disk schema for the shared player/leader/civilization rating ledger.
/// What the scripted major paid for a Holy Site before 2026-08-10, kept so
/// `advanced_holy_v0` can still construct that agent. The live value is
/// [`crate::ai::ADVANCED_D_HOLY`].
pub const PRE_2026_08_10_D_HOLY: f64 = 2.0;

/// The city-target floor the frozen and pre-promotion controllers use, so
/// `advanced_without_city_target_floor` withholds to a value the repository
/// already plays rather than a number invented for the arm.
pub const PRE_PROMOTION_CITY_TARGET_FLOOR: usize = 3;

/// What `advanced_lower_city_target` asks the flat gene for: the same 3 the
/// plan's ramp now starts at, so the two "how many cities" knobs agree instead
/// of the gene sitting a city above the plan.
pub const LOWERED_CITY_TARGET: f64 = 3.0;

/// The floor `advanced_wide_opening` tests, read from the one place that
/// defines it so the arm and the history cannot drift apart.
fn civvis_production_city_target_floor() -> usize {
    crate::ai::PRODUCTION_CITY_TARGET_FLOOR
}

/// Games-weighted `settle_food` of the top third of the shipped league roster
/// by outright 8-player win rate, against the bottom third's 1.19 and the
/// shipped 1.2. Read by `advanced_settle_food`; see that arm.
pub const LEAGUE_WINNER_SETTLE_FOOD: f64 = 0.78;

pub const ELO_SCHEMA_VERSION: u32 = 3;
/// Version of the game/rating contract, independent of the JSON shape. Bump
/// this when rules, default setup, or scoring semantics change enough that an
/// Elo point no longer measures the same experiment.
///
/// **v11 (2026-08-18) — the remaining Gathering Storm rule rows use the
/// installed source values and placement semantics.** Pike-and-Shot, Tagma,
/// Prasat, Sukiennice, Tlachtli, and Eyjafjallajökull now carry their effective
/// Gathering Storm values; Monastery, Mine, Terrace Farm, and Rock-Hewn Church
/// also distinguish Hills, resources, and Volcanic Soil as the source does.
///
/// These are shared native-world rules, not live-adapter treatments. They can
/// change openings, economics, military upgrades, and legal Builder actions
/// before or during any controller's turn. The frozen anchor therefore changes
/// from 18,502 actions and `0x1645_2073_bb4b_2b2b` to 18,503 and
/// `0x70c7_8503_3e29_380f` across its five profiles. This is a rules
/// correction, not a compatibility re-pin: v10 and v11 rows are not
/// comparable.
///
/// **v10 (2026-08-18) — map resources use the shipped placement weights.**
/// Civilization VI's `Resources.Frequency` and `SeaFrequency` make, for
/// example, Fish (23) far more common than Whales (1), and Stone (10) more
/// common than Copper (4). CIVVIS instead selected uniformly from all valid
/// resources for each tile. The map generator now draws by the appropriate
/// shipped land or sea weight, while zero-weight artifacts remain owned by
/// their dedicated quota pass.
///
/// Resources contribute to start scoring, so this changes every native-world
/// opening before an agent makes its first decision. The frozen anchor changes
/// from 20,482 actions and `0xd49c_c225_990c_4e66` to 18,502 and
/// `0x1645_2073_bb4b_2b2b` across its five profiles. That is an intentional
/// rules correction, not a compatibility re-pin: v9 and v10 rows are not
/// comparable.
///
/// **v8 (2026-08-11) — first city-state discovery earns an Envoy.** The first
/// living major civilization to make contact with a city-state now receives one
/// Envoy already placed there; later discoverers do not. This is a world rule,
/// not an opt-in controller treatment, so it changes the influence thresholds
/// and available bonuses for Basic, `advanced_v1`, and production Advanced
/// alike. The production Scout's higher-information frontier choice is gated
/// away from `advanced_v1`, but the reward is deliberately not: the new rule
/// changes the experiment whenever anyone reaches a city-state. Ratings from
/// v7 and v8 must not be compared.
///
/// **v7 (2026-08-10) — the AI sells what a declaration cancels.** Immediately
/// before it declares, a civilization now offers its victim the terms the
/// declaration is about to void — spare Luxury copies, Open Borders, Gold per
/// turn — for lump Gold priced at the victim's own walk-away, and the war then
/// returns the Luxury and stops the instalments. Unlike the compatibility
/// re-pins recorded in `docs/ELO_REPINS.md`, this one is behind
/// no constructor flag: `BasicAi::war_eve_liquidation` runs from the shared
/// `diplomacy` pass and from `AdvancedAi`'s ordinary declaration, so the
/// `advanced_v1` anchor plays it too and its treasuries genuinely differ from
/// v6. `game::trade_deal_tests::the_ai_sells_the_cancellable_promises_only_into_a_real_declaration`
/// is the check on that claim, which is why this is a bump rather than a pin.
/// A campaign target under a peace treaty also trades normally again, so the
/// same seats hold different Gold and resources through a treaty.
///
/// **v6 (2026-08-04) — military unique improvements enter AI planning.** Charged
/// Toa, Legions, and Nau now spend their unique improvement actions when a legal
/// site exists, with the advanced controller valuing defensive frontier works and
/// Feitoria trade yields. This is live in both the shared Basic path and the
/// `advanced_v1` serial path, so results can no longer share a ledger with v5.
///
/// **v5 (2026-08-03) — the pantheon price follows game speed.** The faith cost had
/// three spellings; the legality gate, the spend in `do_choose_pantheon` and the
/// AI's own gate in `ai.rs` now all read `Game::pantheon_faith_cost()`.
///
/// ⚠ At `GameSpeed::default()` — Standard — the scaled price is exactly the `25.0`
/// literal it replaced, so the anchor's behaviour is bit-for-bit unchanged and a
/// compatibility re-pin of the old byte-hash source contract would have sufficed.
/// Bumped anyway, on the operator's call: the ledger is cheap to restart and the
/// alternative is a ledger that silently mixes two rule sets if the Standard-speed
/// argument is ever wrong. Rows before and after v5 are not comparable at Online,
/// Quick, Epic or Marathon, where the price genuinely moved (12.5 / 16.75 / 37.5 /
/// 75 against a flat 25).
/// **v9 (2026-08-18) — a pillaged improvement stops granting Housing.**
/// `city_housing_sources` skipped `tile.pillaged` entirely while the building
/// loop beside it had always honoured `city.pillaged_buildings`, so a razed
/// farm went on feeding a city's growth ceiling until somebody repaired it.
/// Housing is what caps growth in Civilization VI, so this changes when cities
/// grow and therefore what every agent decides — the frozen anchor included:
/// 20,464 decisions became 20,482 across the five anchor profiles.
///
/// ⚠ This is the case the ledger version exists for, and it is NOT the case
/// `ai.rs`'s live-adapter gates cover. Those gate a fix behind the live bridge
/// when the bug "only bites the live bridge"; this is the opposite. `mirror.rs`
/// overwrites housing with the host's own figure every turn, and `game.rs` says
/// the correction is "Empty on a native game" — so the live seat was already
/// right and every OFFLINE game was wrong. Gating this behind the live adapter
/// would have preserved the bug in exactly the games this ledger rates.
///
/// Rows before and after v9 are not comparable wherever an improvement was ever
/// pillaged, which on any map with barbarians is most of them.
pub const ELO_PROTOCOL_VERSION: u32 = 11;
pub const ELO_BASE_RATING: f64 = 1500.0;
pub const DEFAULT_RATINGS_PATH: &str = "data/elo_ratings.json";
/// The Tactics ladder. Pure unit tactics is a different skill from the grand
/// strategy game, so it is a different rating.
pub const TACTICS_RATINGS_PATH: &str = "data/elo_ratings_tactics.json";
/// The Sim City ladder, for the mode's own rating when it arrives.
pub const SIMCITY_RATINGS_PATH: &str = "data/elo_ratings_simcity.json";

/// Where a mode's ladder lives.
///
/// One ledger per mode, so a player carries a Civ rating, a Tactics rating
/// and — when the mode arrives — a Sim City rating, each earned against the
/// opponents that mode was played against. The separation is not merely
/// tidiness: a ledger already refuses a game whose setup does not match its
/// own profile, and a battlefield differs from a world in the map script the
/// profile records, so a Tactics result offered to the Civ ladder is rejected
/// outright rather than quietly averaged in. This names the file that result
/// belongs in instead.
pub const fn ratings_path_for(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Civ => DEFAULT_RATINGS_PATH,
        GameMode::Tactics => TACTICS_RATINGS_PATH,
        GameMode::SimCity => SIMCITY_RATINGS_PATH,
    }
}

/// One player's rating across every mode they have played.
///
/// `overall` is the games-weighted mean of the per-mode ratings, which is the
/// honest summary and not a rating in its own right: the ladders are separate
/// experiments against different opponents on different ground, so a Tactics
/// 1600 and a Civ 1600 are not the same 1600 and combining them cannot make
/// them so. What it does say is what a player has actually demonstrated,
/// weighted by how much of it they have demonstrated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerRatings {
    pub player: String,
    pub overall: f64,
    pub games: u32,
    /// Per mode, for the modes this player has games in.
    pub by_mode: BTreeMap<String, ModeRating>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModeRating {
    pub elo: f64,
    pub games: u32,
    pub wins: u32,
}

/// Read every mode's ladder from `dir` and collect each player's ratings.
///
/// A missing ladder is a mode nobody has played yet, not an error: Sim City
/// has no file until Sim City exists, and a fresh checkout has no Tactics
/// file until the first Tactics tournament is run.
pub fn player_ratings(dir: &std::path::Path) -> BTreeMap<String, PlayerRatings> {
    let mut out: BTreeMap<String, PlayerRatings> = BTreeMap::new();
    for mode in GameMode::ALL {
        let path = dir.join(
            std::path::Path::new(ratings_path_for(mode))
                .file_name()
                .expect("every ladder path names a file"),
        );
        let pool = match EloPool::load(&path) {
            Ok(pool) => pool,
            Err(error) => {
                debug_assert!(
                    error.kind() == std::io::ErrorKind::NotFound,
                    "unreadable ladder {}: {error}",
                    path.display()
                );
                continue;
            }
        };
        for (player, rating) in &pool.overall {
            let entry = out.entry(player.clone()).or_insert_with(|| PlayerRatings {
                player: player.clone(),
                overall: 0.0,
                games: 0,
                by_mode: BTreeMap::new(),
            });
            entry.by_mode.insert(
                mode.id().to_string(),
                ModeRating { elo: rating.elo, games: rating.games, wins: rating.wins },
            );
        }
    }
    for ratings in out.values_mut() {
        let played: u32 = ratings.by_mode.values().map(|mode| mode.games).sum();
        ratings.games = played;
        ratings.overall = if played == 0 {
            ELO_BASE_RATING
        } else {
            ratings
                .by_mode
                .values()
                .map(|mode| mode.elo * f64::from(mode.games))
                .sum::<f64>()
                / f64::from(played)
        };
    }
    out
}
/// Immutable protocol-v1 baseline retained for historical comparison after
/// the fog-honest city-pressure repair changed the shared legacy controller.
pub const HISTORICAL_V1_RATINGS_PATH: &str = "data/elo_ratings_v1.json";
/// Immutable protocol-v2 baseline retained after the island-settlement repair
/// changed the shared legacy controller again.
pub const HISTORICAL_V2_RATINGS_PATH: &str = "data/elo_ratings_v2.json";
/// Immutable protocol-v3 baseline retained after the intergovernment
/// diplomacy pass changed both shared scripted-controller paths.
pub const HISTORICAL_V3_RATINGS_PATH: &str = "data/elo_ratings_v3.json";
const LEAGUE_SNAPSHOT_DIR: &str = "data/league";
const LEAGUE_SNAPSHOT_FILE: &str = "data/league/league.json";

/// Schema 3 existed before `setup_contract` was serialized. Those files were
/// all created under this exact lobby contract, so their migration value must
/// remain historical even after a future protocol deliberately changes the
/// live defaults.
const SCHEMA3_LEGACY_SETUP_CONTRACT: &str = "base=civ6;era=0;difficulty=prince;barbarians=true;disasters=2;modes=none;leader-pool=civ6;civilizations=stock-fill;randomize-civs=false;human-seats=none;teams=free-for-all;victories=science+culture+religious+diplomatic+domination+score";

fn schema3_legacy_setup_contract() -> String {
    SCHEMA3_LEGACY_SETUP_CONTRACT.to_string()
}

/// Outcome-affecting tournament settings that are fixed by the harness rather
/// than exposed in [`TourneyCfg`]. Derive the string from the same defaults
/// [`play_tournament`] constructs so changing one cannot silently append a
/// different experiment to an existing ledger.
fn tournament_setup_contract(cfg: &TourneyCfg) -> String {
    let options = GameOptions::new(
        cfg.players_per_game,
        cfg.width,
        cfg.height,
        0,
        cfg.max_turns,
        cfg.num_city_states,
    );
    let modes = if options.game_modes.is_empty() {
        "none".to_string()
    } else {
        options
            .game_modes
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("+")
    };
    let civilizations = if options.civs.is_empty() {
        "stock-fill".to_string()
    } else {
        options.civs.join("+")
    };
    let human_seats = if options.human_seats.is_empty() {
        "none".to_string()
    } else {
        options
            .human_seats
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("+")
    };
    let teams = if options.teams.is_empty() {
        "free-for-all".to_string()
    } else {
        options
            .teams
            .iter()
            .map(|team| team.map_or_else(|| "none".to_string(), |team| team.to_string()))
            .collect::<Vec<_>>()
            .join("+")
    };
    let victories = VictoryConditions::NAMES
        .into_iter()
        .filter(|victory| VictoryConditions::default().is_enabled(victory))
        .collect::<Vec<_>>()
        .join("+");
    // An arena's economy decides what the battle is: at one city per side a
    // 20x20 field is settled in tens of turns by taking the city, at zero it
    // is an attrition duel running to the clock. Two ladders rated across
    // those would be measuring different games, so the arena's grants join
    // the profile — and only for an arena, so every Civ ledger written before
    // the mode had an economy still matches its own profile.
    let arena = if cfg.map_script.is_battlefield() {
        format!(
            ";arena=cities:{},production:{},gold:{},turns-per-tech:{}{}",
            cfg.tactics.cities,
            cfg.tactics.production,
            cfg.tactics.gold,
            cfg.tactics.turns_per_tech,
            // A flag battle is a different game from an attrition duel — a
            // race can be won by a side that would have lost the fight — so
            // the objective joins the profile. Only when it is the flag:
            // every arena ledger written before the shape existed stays
            // matching its own profile.
            if cfg.tactics.flag { ",objective:flag" } else { "" },
        )
    } else {
        String::new()
    };
    format!(
        "base={};era={};difficulty={};barbarians={};disasters={};modes={};leader-pool={};civilizations={};randomize-civs={};human-seats={};teams={};victories={}{arena}",
        options.base_ruleset.id(),
        cfg.start_era.profile_id(),
        options.difficulty,
        options.barbarians,
        options.disaster_intensity,
        modes,
        options.leader_pool.id(),
        civilizations,
        options.randomize_civs,
        human_seats,
        teams,
        victories,
    )
}

/// Conservatively strongest *winning* active, untargeted genome in the
/// committed outcome-rated league. Lane specialists answer a different
/// question; this challenger isolates whether the same win-selected generalist
/// the evolutionary loop prefers transfers to the strongest macro-search
/// budget.
fn league_generalist() -> Option<(String, Weights)> {
    crate::league::load_league(LEAGUE_SNAPSHOT_DIR)?
        .strategies
        .into_iter()
        .filter(|strategy| !strategy.retired && !strategy.human)
        .filter_map(|strategy| {
            let win = crate::league::win_lower_confidence(&strategy);
            let rating = strategy.rating - 1.96 * strategy.rd;
            match strategy.kind {
                crate::league::StrategyKind::Advanced {
                    weights,
                    target: None,
                } => Some((win, rating, strategy.name, weights)),
                _ => None,
            }
        })
        .max_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| right.2.cmp(&left.2))
        })
        .map(|(_, _, name, weights)| (name, weights))
}

/// The weights used by `advanced_league_top`.  Keep this extraction shared
/// with the evaluator spec so the reported source cannot diverge from the
/// controller the factory constructs.
fn shipped_league_top_advanced() -> Option<Weights> {
    let league = crate::league::shipped_league()?;
    let mut best: Option<(f64, Weights)> = None;
    for index in league.active() {
        let strategy = &league.strategies[index];
        if let crate::league::StrategyKind::Advanced { weights, .. } = &strategy.kind {
            if best
                .as_ref()
                .map(|(rating, _)| strategy.rating > *rating)
                .unwrap_or(true)
            {
                best = Some((strategy.rating, weights.clone()));
            }
        }
    }
    best.map(|(_, weights)| weights)
}

/// Resolve the leader supplied by the active ruleset. Keeping this beside the
/// ledger migration also gives old civilization-only rows an unambiguous home.
pub fn leader_for_civilization(civilization: &str) -> String {
    Rules::embedded()
        .civs
        .get(civilization)
        .map(|spec| spec.leader.clone())
        .unwrap_or_else(|| civilization.to_string())
}

pub fn expected(ra: f64, rb: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((rb - ra) / 400.0))
}

/// Each rating's chance of *winning outright* against the rest of the table,
/// summing to 1.
pub fn win_shares(ratings: &[f64]) -> Vec<f64> {
    let top = ratings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = ratings
        .iter()
        .map(|rating| 10f64.powf((rating - top) / 400.0))
        .collect();
    let total: f64 = weights.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return vec![1.0 / ratings.len().max(1) as f64; ratings.len()];
    }
    weights.iter().map(|weight| weight / total).collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct RatingKey {
    pub player: String,
    pub leader: String,
    pub civilization: String,
}

impl RatingKey {
    pub fn new(
        player: impl Into<String>,
        leader: impl Into<String>,
        civilization: impl Into<String>,
    ) -> Self {
        Self {
            player: player.into(),
            leader: leader.into(),
            civilization: civilization.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub elo: f64,
    pub games: u32,
    pub wins: u32,
}

/// The game contract one persistent Elo ledger measures.
///
/// Ratings from different table sizes, maps, speeds, turn limits, or K factors
/// are different experiments. Older ledgers silently mixed them. Schema 3
/// binds the first run to its complete tournament profile and rejects later
/// incompatible runs, so a number can serve as a longitudinal baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TournamentProfile {
    pub protocol_version: u32,
    /// Fingerprint of the fully merged stock + mod rules JSON. Mod names are
    /// retained below for readability; this value binds their actual content.
    pub rules_fingerprint: String,
    /// Lobby settings fixed by the tournament harness rather than exposed
    /// in `TourneyCfg`. Older schema-3 ledgers deserialize to the historical
    /// defaults, then write this contract explicitly on their next checkpoint.
    #[serde(default = "schema3_legacy_setup_contract")]
    pub setup_contract: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_anchor: Option<String>,
    /// Ordered controller roles in the tournament. Immutable player identities
    /// may version between runs, but changing a role changes the multiplayer
    /// environment and therefore requires a different ledger.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controller_roster: Vec<String>,
    pub players_per_game: usize,
    pub width: i32,
    pub height: i32,
    pub max_turns: u32,
    pub num_city_states: usize,
    pub speed: String,
    pub map_script: String,
    pub map_topology: String,
    pub map_poles: String,
    pub mods: Vec<String>,
    pub k: f64,
}

impl TournamentProfile {
    fn from_cfg(cfg: &TourneyCfg) -> Self {
        Self {
            protocol_version: ELO_PROTOCOL_VERSION,
            rules_fingerprint: Rules::embedded().source_fingerprint().to_string(),
            setup_contract: tournament_setup_contract(cfg),
            rating_anchor: cfg.rating_anchor.clone(),
            controller_roster: cfg.controller_roster.clone(),
            players_per_game: cfg.players_per_game,
            width: cfg.width,
            height: cfg.height,
            max_turns: cfg.max_turns,
            num_city_states: cfg.num_city_states,
            speed: cfg.speed.clone(),
            map_script: cfg.map_script.id().to_string(),
            map_topology: cfg.map_topology.id().to_string(),
            map_poles: cfg.map_poles.id().to_string(),
            mods: crate::mods::active_names(),
            k: cfg.k,
        }
    }

    fn validate(&self) -> bool {
        self.protocol_version > 0
            && self.rules_fingerprint.starts_with("fnv1a64:")
            && self.rules_fingerprint.len() == "fnv1a64:".len() + 16
            && self.rules_fingerprint["fnv1a64:".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && !self.setup_contract.trim().is_empty()
            && self
                .rating_anchor
                .as_ref()
                .is_none_or(|anchor| !anchor.trim().is_empty())
            && self.controller_roster.iter().all(|name| !name.trim().is_empty())
            && (self.controller_roster.is_empty()
                || (self.controller_roster.len() >= self.players_per_game
                    && self
                        .controller_roster
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        == self.controller_roster.len()))
            && (2..=100).contains(&self.players_per_game)
            && self.width >= 8
            && self.height >= 8
            && self.max_turns > 0
            && Rules::embedded().speeds.contains_key(&self.speed)
            && MapScript::from_id(&self.map_script).is_some()
            && MapTopology::from_id(&self.map_topology).is_some()
            && MapPoles::from_id(&self.map_poles).is_some()
            && self.mods.iter().all(|name| !name.trim().is_empty())
            && self.k.is_finite()
            && self.k > 0.0
    }

    pub fn label(&self) -> String {
        let mods = if self.mods.is_empty() {
            "stock".to_string()
        } else {
            self.mods.join("+")
        };
        let anchor = self.rating_anchor.as_deref().unwrap_or("floating");
        let controllers = if self.controller_roster.is_empty() {
            "unbound".to_string()
        } else {
            self.controller_roster.join(",")
        };
        format!(
            "protocol v{}, rules={}, setup={}, {}p {}x{}, {} turns, {} city-states, {}, {}/{}/{}, mods={}, K={}, anchor={}, controllers={}",
            self.protocol_version,
            self.rules_fingerprint,
            self.setup_contract,
            self.players_per_game,
            self.width,
            self.height,
            self.max_turns,
            self.num_city_states,
            self.speed,
            self.map_script,
            self.map_topology,
            self.map_poles,
            mods,
            self.k,
            anchor,
            controllers,
        )
    }
}

impl Rating {
    fn new(base: f64) -> Self {
        Self {
            elo: base,
            games: 0,
            wins: 0,
        }
    }
}

fn rating_maps_match<K: Ord>(left: &BTreeMap<K, Rating>, right: &BTreeMap<K, Rating>) -> bool {
    const ROUND_TRIP_TOLERANCE: f64 = 1e-9;
    left.len() == right.len()
        && left.iter().all(|(key, rating)| {
            right.get(key).is_some_and(|other| {
                rating.games == other.games
                    && rating.wins == other.wins
                    && (rating.elo - other.elo).abs() <= ROUND_TRIP_TOLERANCE
            })
        })
}

#[derive(Clone, Debug, PartialEq)]
pub struct EloPool {
    pub base_rating: f64,
    /// Profile-independent player summaries. These accumulate across every
    /// leader/civilization draw and provide the stable longitudinal baseline;
    /// exact combination rows below remain independently queryable.
    pub overall: BTreeMap<String, Rating>,
    /// The rating identity is deliberately structured, not a display string:
    /// player, leader, and civilization can be queried independently.
    pub ratings: BTreeMap<RatingKey, Rating>,
    /// Once present, every future tournament written to this ledger must match
    /// it exactly. `None` exists only for in-memory/manual pools and migrated
    /// schema-1/2 files until their first schema-3 tournament run.
    pub profile: Option<TournamentProfile>,
    /// Ordered raw game evidence. Fresh schema-3 ledgers can rebuild every
    /// aggregate from this log; migrated ledgers retain their old aggregate
    /// as an unauditable prior and mark the history incomplete.
    pub history: Vec<RatedGame>,
    pub history_complete: bool,
}

#[derive(Serialize, Deserialize)]
struct StoredPool {
    schema_version: u32,
    base_rating: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile: Option<TournamentProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    players: Vec<StoredPlayerRating>,
    ratings: Vec<StoredRating>,
    #[serde(default)]
    history_complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    games: Vec<RatedGame>,
}

#[derive(Serialize, Deserialize)]
struct StoredPlayerRating {
    player: String,
    elo: f64,
    games: u32,
    wins: u32,
}

#[derive(Serialize, Deserialize)]
struct StoredRating {
    #[serde(default)]
    player: String,
    #[serde(default)]
    leader: String,
    civilization: String,
    /// Schema-1 migration source. A legacy strategy becomes the player only
    /// when the row does not identify exactly one contributing AI factory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strategy: Option<String>,
    elo: f64,
    games: u32,
    wins: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    agents: Vec<String>,
}

/// Everything needed to score one rated major at the end of a game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RatedPlayer {
    pub key: RatingKey,
    pub score: i64,
    pub won: bool,
}

/// One immutable rating event. Persistent tournament events carry a stable
/// id derived from the map seed and ordered entrant identities; manual library
/// callers leave it absent and retain insertion order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RatedGame {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub players: Vec<RatedPlayer>,
    pub k: f64,
}

impl RatedPlayer {
    pub fn new(
        player: impl Into<String>,
        leader: impl Into<String>,
        civilization: impl Into<String>,
        score: i64,
        won: bool,
    ) -> Self {
        Self {
            key: RatingKey::new(player, leader, civilization),
            score,
            won,
        }
    }
}

fn head_to_head_score(a: &RatedPlayer, b: &RatedPlayer) -> f64 {
    if a.won != b.won {
        f64::from(a.won)
    } else if a.score > b.score {
        1.0
    } else if a.score < b.score {
        0.0
    } else {
        0.5
    }
}

fn valid_rated_players(players: &[RatedPlayer], distinct_identities: bool) -> bool {
    players.len() >= 2
        && players.iter().all(|player| {
            !player.key.player.trim().is_empty()
                && !player.key.leader.trim().is_empty()
                && !player.key.civilization.trim().is_empty()
        })
        && (!distinct_identities
            || players
                .iter()
                .map(|player| player.key.player.as_str())
                .collect::<BTreeSet<_>>()
                .len()
                == players.len())
}

impl EloPool {
    /// Keep the historical constructor shape for library callers. Entrants no
    /// longer create rating rows up front because their leader/civilization
    pub fn new(_names: &[String], base: f64) -> EloPool {
        EloPool {
            base_rating: base,
            overall: BTreeMap::new(),
            ratings: BTreeMap::new(),
            profile: None,
            history: Vec::new(),
            history_complete: true,
        }
    }

    pub fn with_base(base: f64) -> EloPool {
        Self::new(&[], base)
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<EloPool> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)?;
        let stored: StoredPool = serde_json::from_str(&raw).map_err(|error| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("invalid Elo ledger {}: {error}", path.display()),
            )
        })?;
        if !matches!(stored.schema_version, 1 | 2 | ELO_SCHEMA_VERSION) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "unsupported Elo schema {} in {}; expected {}",
                    stored.schema_version,
                    path.display(),
                    ELO_SCHEMA_VERSION
                ),
            ));
        }
        if !stored.base_rating.is_finite() {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("non-finite base rating in {}", path.display()),
            ));
        }
        if stored
            .profile
            .as_ref()
            .is_some_and(|profile| !profile.validate())
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("invalid tournament profile in {}", path.display()),
            ));
        }
        let mut ratings: BTreeMap<RatingKey, Rating> = BTreeMap::new();
        for row in stored.ratings {
            let player = if stored.schema_version == 1 {
                if row.agents.len() == 1 {
                    row.agents[0].clone()
                } else {
                    row.strategy.clone().unwrap_or_default()
                }
            } else {
                row.player
            };
            let leader = if stored.schema_version == 1 {
                leader_for_civilization(&row.civilization)
            } else {
                row.leader
            };
            if player.trim().is_empty()
                || leader.trim().is_empty()
                || row.civilization.trim().is_empty()
                || !row.elo.is_finite()
                || row.wins > row.games
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid rating row in {}", path.display()),
                ));
            }
            let key = RatingKey::new(player, leader, row.civilization);
            let rating = Rating {
                elo: row.elo,
                games: row.games,
                wins: row.wins,
            };
            if let Some(existing) = ratings.get_mut(&key) {
                if stored.schema_version >= 2 {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "duplicate player/leader/civilization row {key:?} in {}",
                            path.display()
                        ),
                    ));
                }
                let total = existing.games.saturating_add(rating.games);
                if total > 0 {
                    existing.elo = (existing.elo * existing.games as f64
                        + rating.elo * rating.games as f64)
                        / total as f64;
                }
                existing.games = total;
                existing.wins = existing.wins.saturating_add(rating.wins);
            } else {
                ratings.insert(key, rating);
            }
        }
        let mut overall = BTreeMap::new();
        for row in stored.players {
            if row.player.trim().is_empty()
                || !row.elo.is_finite()
                || row.wins > row.games
                || overall.contains_key(&row.player)
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid overall player rating in {}", path.display()),
                ));
            }
            overall.insert(
                row.player,
                Rating {
                    elo: row.elo,
                    games: row.games,
                    wins: row.wins,
                },
            );
        }
        // Schema 1/2 had only combination rows. Preserve their scale and give
        // each player the games-weighted centre of those rows as a migration
        // prior. The old files cannot recover how many distinct worlds those
        // seats came from, so the global game/win counters start at zero;
        // exact combination rows retain all of the legacy counts.
        let mut accumulated = BTreeMap::<String, (f64, u32)>::new();
        for (key, rating) in &ratings {
            let entry = accumulated.entry(key.player.clone()).or_default();
            entry.0 += rating.elo * f64::from(rating.games);
            entry.1 = entry.1.saturating_add(rating.games);
        }
        for (player, (weighted, games)) in accumulated {
            overall.entry(player).or_insert_with(|| Rating {
                elo: if games > 0 {
                    weighted / f64::from(games)
                } else {
                    stored.base_rating
                },
                games: 0,
                wins: 0,
            });
        }
        let games = if stored.schema_version == ELO_SCHEMA_VERSION {
            stored.games
        } else {
            Vec::new()
        };
        let mut event_ids = BTreeSet::new();
        let mut keyed_games = 0usize;
        for game in &games {
            let valid_id = match &game.id {
                Some(id) => {
                    keyed_games += 1;
                    !id.trim().is_empty() && event_ids.insert(id.clone())
                }
                None => true,
            };
            let valid_players = valid_rated_players(&game.players, game.id.is_some())
                && stored.profile.as_ref().is_none_or(|profile| {
                    game.players.len() == profile.players_per_game
                });
            if !valid_id || !valid_players || !game.k.is_finite() || game.k < 0.0 {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid raw game evidence in {}", path.display()),
                ));
            }
        }
        let mixed_keying = keyed_games != 0 && keyed_games != games.len();
        let unordered_keys =
            keyed_games != 0 && !games.windows(2).all(|pair| pair[0].id < pair[1].id);
        let profile_k_mismatch = stored.profile.as_ref().is_some_and(|profile| {
            games
                .iter()
                .any(|game| (game.k - profile.k).abs() > f64::EPSILON)
        });
        if mixed_keying || unordered_keys || profile_k_mismatch {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "non-canonical raw game history in {} (mixed keyed/unkeyed: {}, ordered: {}, profile K matches: {})",
                    path.display(),
                    mixed_keying,
                    !unordered_keys,
                    !profile_k_mismatch,
                ),
            ));
        }
        let history_complete =
            stored.schema_version == ELO_SCHEMA_VERSION && stored.history_complete;
        if history_complete {
            let mut replay = EloPool::with_base(stored.base_rating);
            replay.profile = stored.profile.clone();
            for game in &games {
                replay.apply_game(&game.players, game.k);
            }
            let players_match = rating_maps_match(&replay.overall, &overall);
            let combinations_match = rating_maps_match(&replay.ratings, &ratings);
            if !players_match || !combinations_match {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Elo aggregates do not match raw game evidence in {} (players match: {}, combinations match: {})",
                        path.display(),
                        players_match,
                        combinations_match,
                    ),
                ));
            }
        }
        Ok(EloPool {
            base_rating: stored.base_rating,
            overall,
            ratings,
            profile: stored.profile,
            history: games,
            history_complete,
        })
    }

    pub fn load_or_new(path: impl AsRef<Path>, base: f64) -> io::Result<EloPool> {
        match Self::load(path) {
            Ok(pool) => Ok(pool),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::with_base(base)),
            Err(error) => Err(error),
        }
    }

    /// Bind a persistent ledger to one reproducible tournament contract.
    ///
    /// A migrated schema-1/2 file has no profile, so its first schema-3 run
    /// records one. Once bound, mixing evidence from another profile is an
    /// error rather than a silent change in what one Elo point means.
    pub fn bind_profile(&mut self, profile: TournamentProfile) -> io::Result<()> {
        if !profile.validate() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "invalid tournament rating profile",
            ));
        }
        if self
            .history
            .iter()
            .any(|game| {
                (game.k - profile.k).abs() > f64::EPSILON
                    || game.players.len() != profile.players_per_game
            })
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "raw game evidence does not match the tournament rating profile",
            ));
        }
        match &self.profile {
            Some(existing) if existing != &profile => Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "rating profile mismatch: ledger is [{}], requested [{}]; use a different --ratings path for a different experiment",
                    existing.label(),
                    profile.label(),
                ),
            )),
            Some(_) => Ok(()),
            None => {
                self.profile = Some(profile);
                Ok(())
            }
        }
    }

    /// Atomically replace a ledger, so interruption cannot leave partial JSON.
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let stored = StoredPool {
            schema_version: ELO_SCHEMA_VERSION,
            base_rating: self.base_rating,
            profile: self.profile.clone(),
            players: self
                .overall
                .iter()
                .map(|(player, rating)| StoredPlayerRating {
                    player: player.clone(),
                    elo: rating.elo,
                    games: rating.games,
                    wins: rating.wins,
                })
                .collect(),
            ratings: self
                .ratings
                .iter()
                .map(|(key, rating)| StoredRating {
                    player: key.player.clone(),
                    leader: key.leader.clone(),
                    civilization: key.civilization.clone(),
                    strategy: None,
                    elo: rating.elo,
                    games: rating.games,
                    wins: rating.wins,
                    agents: Vec::new(),
                })
                .collect(),
            history_complete: self.history_complete,
            games: self.history.clone(),
        };
        let mut raw = serde_json::to_vec_pretty(&stored).map_err(io::Error::other)?;
        raw.push(b'\n');

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("elo_ratings.json");
        let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(&raw)?;
            file.sync_all()?;
            fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    /// Pairwise, simultaneous Elo update from the pre-game ratings. Equal
    /// scores are draws unless one player is the engine-declared winner.
    pub fn record_game(&mut self, players: &[RatedPlayer], k: f64) {
        if players.len() < 2 {
            return;
        }
        assert!(
            k.is_finite() && k >= 0.0,
            "Elo K must be finite and non-negative"
        );
        assert!(
            self.profile
                .as_ref()
                .is_none_or(|profile| (profile.k - k).abs() <= f64::EPSILON),
            "Elo K must match the bound tournament profile"
        );
        assert!(
            self.profile
                .as_ref()
                .is_none_or(|profile| players.len() == profile.players_per_game),
            "Elo table size must match the bound tournament profile"
        );
        self.apply_game(players, k);
        self.history.push(RatedGame {
            id: None,
            players: players.to_vec(),
            k,
        });
    }

    /// Insert a reproducibly identified tournament game exactly once.
    ///
    /// Keyed evidence is sorted before a full replay, so two concurrent
    /// tournament processes produce the same table regardless of which lock
    /// they acquire first. Repeating an identical run is idempotent; the same
    /// identity producing different evidence is rejected as a reproducibility
    /// failure instead of double-counted.
    pub fn record_game_once(
        &mut self,
        id: impl Into<String>,
        players: &[RatedPlayer],
        k: f64,
    ) -> io::Result<bool> {
        let id = id.into();
        if id.trim().is_empty()
            || !valid_rated_players(players, true)
            || !k.is_finite()
            || k < 0.0
            || self
                .profile
                .as_ref()
                .is_some_and(|profile| players.len() != profile.players_per_game)
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "invalid keyed Elo game (table size and player identities must be distinct and match the profile)",
            ));
        }
        if self
            .profile
            .as_ref()
            .is_some_and(|profile| (profile.k - k).abs() > f64::EPSILON)
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Elo K does not match the bound tournament profile",
            ));
        }
        let candidate = RatedGame {
            id: Some(id.clone()),
            players: players.to_vec(),
            k,
        };
        if let Some(existing) = self
            .history
            .iter()
            .find(|game| game.id.as_deref() == Some(id.as_str()))
        {
            return if existing == &candidate {
                Ok(false)
            } else {
                Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Elo event {id:?} was replayed with different results; use a new versioned rating identity when a controller changes"
                    ),
                ))
            };
        }
        if self.history.iter().any(|game| game.id.is_none()) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "cannot mix reproducibly keyed tournament evidence with unkeyed manual games; use a different --ratings path",
            ));
        }
        let appends_in_order = self
            .history
            .last()
            .and_then(|game| game.id.as_deref())
            .is_none_or(|last| last < id.as_str());
        self.history.push(candidate);
        if self.history_complete {
            if appends_in_order {
                self.apply_game(players, k);
            } else {
                self.history.sort_by(|a, b| a.id.cmp(&b.id));
                let history = self.history.clone();
                self.overall.clear();
                self.ratings.clear();
                for game in &history {
                    self.apply_game(&game.players, game.k);
                }
                self.history = history;
            }
        } else {
            // A schema-1/2 aggregate has no recoverable raw starting evidence.
            // Its new events are still retained and deduplicated, but cannot
            // be reordered ahead of that imported prior.
            self.apply_game(players, k);
        }
        Ok(true)
    }

    fn apply_game(&mut self, players: &[RatedPlayer], k: f64) {
        let mut by_player = BTreeMap::<String, Vec<&RatedPlayer>>::new();
        for player in players {
            by_player
                .entry(player.key.player.clone())
                .or_default()
                .push(player);
            self.overall
                .entry(player.key.player.clone())
                .or_insert_with(|| Rating::new(self.base_rating));
        }
        for player in players {
            let prior = self.overall[&player.key.player].elo;
            self.ratings
                .entry(player.key.clone())
                .or_insert_with(|| Rating::new(prior));
        }

        // One global player identity accumulates across every civilization it
        // draws. When a tournament has fewer entrants than seats, average all
        // cross-seat comparisons for one player pair and count that pair once;
        // cloned seats are correlated and must not manufacture four games of
        // rating evidence from one world.
        let identities: Vec<String> = by_player.keys().cloned().collect();
        if identities.len() >= 2 {
            let scale = k / (identities.len() as f64 - 1.0);
            let mut overall_delta = BTreeMap::<String, f64>::new();
            for i in 0..identities.len() {
                for j in (i + 1)..identities.len() {
                    let a_name = &identities[i];
                    let b_name = &identities[j];
                    let mut actual = 0.0;
                    let mut comparisons = 0usize;
                    for a in &by_player[a_name] {
                        for b in &by_player[b_name] {
                            actual += head_to_head_score(a, b);
                            comparisons += 1;
                        }
                    }
                    actual /= comparisons.max(1) as f64;
                    let expectation =
                        expected(self.overall[a_name].elo, self.overall[b_name].elo);
                    let change = scale * (actual - expectation);
                    *overall_delta.entry(a_name.clone()).or_default() += change;
                    *overall_delta.entry(b_name.clone()).or_default() -= change;
                }
            }
            for (name, change) in overall_delta {
                self.overall.get_mut(&name).unwrap().elo += change;
            }
        }
        for (name, seats) in &by_player {
            let rating = self.overall.get_mut(name).unwrap();
            rating.games = rating.games.saturating_add(1);
            rating.wins = rating
                .wins
                .saturating_add(u32::from(seats.iter().any(|seat| seat.won)));
        }

        let scale = k / (players.len() as f64 - 1.0);
        let mut delta: BTreeMap<RatingKey, f64> = BTreeMap::new();
        for i in 0..players.len() {
            for j in (i + 1)..players.len() {
                let a = &players[i];
                let b = &players[j];
                if a.key.player == b.key.player {
                    // A tournament may reuse one AI player when there are
                    // fewer entrants than seats. Its leader ratings must not
                    // manufacture evidence by competing against themselves.
                    continue;
                }
                let actual_a = head_to_head_score(a, b);
                let elo_a = self.ratings[&a.key].elo;
                let elo_b = self.ratings[&b.key].elo;
                let change = scale * (actual_a - expected(elo_a, elo_b));
                *delta.entry(a.key.clone()).or_insert(0.0) += change;
                *delta.entry(b.key.clone()).or_insert(0.0) -= change;
            }
        }
        for (key, change) in delta {
            self.ratings.get_mut(&key).unwrap().elo += change;
        }
        for player in players {
            let rating = self.ratings.get_mut(&player.key).unwrap();
            rating.games = rating.games.saturating_add(1);
            rating.wins = rating.wins.saturating_add(u32::from(player.won));
        }
        self.recenter_to_anchor();
    }

    /// Keep one contract-pinned control at the base rating. Elo expectations depend
    /// only on differences, so translating every row preserves every update
    /// while preventing repeated introductions of fresh 1500-rated identities
    /// from inflating later generations relative to old, inactive ones.
    fn recenter_to_anchor(&mut self) {
        let Some(anchor) = self
            .profile
            .as_ref()
            .and_then(|profile| profile.rating_anchor.as_ref())
        else {
            return;
        };
        let Some(anchor_rating) = self.overall.get(anchor).map(|rating| rating.elo) else {
            return;
        };
        let shift = self.base_rating - anchor_rating;
        for rating in self.overall.values_mut() {
            rating.elo += shift;
        }
        for rating in self.ratings.values_mut() {
            rating.elo += shift;
        }
    }

    /// Compatibility helper for callers with only a strict placement list.
    /// New evaluation code should use [`EloPool::record_game`] so it can retain
    /// civilization identity and score ties correctly.
    pub fn record(&mut self, placements: &[String], k: f64) {
        let players: Vec<RatedPlayer> = placements
            .iter()
            .enumerate()
            .map(|(place, name)| RatedPlayer {
                key: RatingKey::new(name, "unknown", "unknown"),
                score: (placements.len() - place) as i64,
                won: place == 0,
            })
            .collect();
        self.record_game(&players, k);
    }
}

/// Canonical controller identity for names whose artifacts can make them an
/// exact alias of another selectable agent. The resolved [`ArmKind`] is what
/// construction, provenance, and comparison all receive; a label cannot
/// independently describe a controller that the factory did not build.
fn artifact_effective_alias_from(
    kind: ArmKind,
    champion: bool,
    net: bool,
    league: bool,
) -> ArmKind {
    let basic_fallback = if champion {
        ArmKind::BasicEvolved
    } else {
        ArmKind::Basic
    };
    let advanced_fallback = if champion {
        ArmKind::AdvancedEvolved
    } else {
        ArmKind::Advanced
    };
    match kind {
        ArmKind::Evolved | ArmKind::AdvancedEvolved => advanced_fallback,
        ArmKind::BasicEvolved => basic_fallback,
        ArmKind::Strategic | ArmKind::StrategicWarm => {
            if net {
                ArmKind::Strategic
            } else {
                ArmKind::StrategicScore
            }
        }
        ArmKind::StrategicDeepLeague => {
            if league {
                ArmKind::StrategicDeepLeague
            } else {
                ArmKind::StrategicDeep
            }
        }
        ArmKind::AdvancedEvolvedBlind => {
            if champion {
                ArmKind::AdvancedEvolvedBlind
            } else {
                ArmKind::AdvancedBlindToLeaders
            }
        }
        // The independently confirmed composite became the production
        // controller on 2026-08-01. Retain its pre-registration name as a
        // historical evaluator alias so old commands fail closed as self-play
        // instead of rebuilding a second implementation of `advanced`.
        ArmKind::AdvancedPolicyEnvoyPriority => ArmKind::Advanced,
        // Same story on 2026-08-10: the Holy Site figure this arm carried
        // cleared the 1200-pair gate and became what `advanced` plays, so the
        // arm now builds the production controller. `advanced_holy_v0` is the
        // agent it used to be measured against.
        // `advanced_holy_v0` was the pre-shipment agent; the shipment was
        // reverted, so it is `advanced` under another name and its comparisons
        // must fail closed as self-play.
        ArmKind::AdvancedHolyV0 => ArmKind::Advanced,
        // `bounded_recovery` was removed from production on 2026-08-17 after
        // a 600-map, two-seed null. Keep the historical withhold name for old
        // commands, but resolve it to the stock controller so it cannot report
        // a new comparison against an arm that is already off.
        ArmKind::AdvancedWithoutBoundedRecovery => ArmKind::Advanced,
        // The production floor was removed on 2026-08-10, so withholding it now
        // builds the production controller. Retained under its own name because
        // `docs/EVAL.md` reports five runs against it; declared effectively
        // `advanced` so the pair fails closed as self-play.
        ArmKind::AdvancedWithoutCityTargetFloor => ArmKind::Advanced,
        // Promoted 2026-08-17 on the corrected-gate matrix (see
        // `promoted_policy_envoy`): the quartet is production now, so the
        // treatment arm is a re-labelling of `advanced`.
        ArmKind::AdvancedReconFleet => ArmKind::Advanced,
        // Same story on 2026-08-14 for the war half: the withhold passed the
        // corrected-gate promotion matrix (+38, CI +10..+66, seed stream
        // 18000000) and `promoted_policy_envoy` stopped setting the four
        // flags, so these three withhold arms now build the production
        // controller. Retained under their own names because `docs/EVAL.md`
        // reports eight runs across the family; declared effectively
        // `advanced` so the pairs fail closed as self-play. The inverse
        // treatment is `AdvancedWarHalf`.
        ArmKind::AdvancedWithoutUnpricedWar => ArmKind::Advanced,
        ArmKind::AdvancedWithoutCityDefence => ArmKind::Advanced,
        ArmKind::AdvancedWithoutUnitTactics => ArmKind::Advanced,
        // Production sets `plan_city_target`, so this "add" arm builds the
        // production controller and measures nothing. Fail it closed.
        ArmKind::AdvancedPlanCityTarget => ArmKind::Advanced,
        ArmKind::AdvancedBankingDedication => advanced_fallback,
        _ => kind,
    }
}

fn artifact_effective_alias(kind: ArmKind, dir: &str) -> ArmKind {
    // The hot factory path resolves only artifacts which can change its
    // canonical controller.  In particular, scripted `advanced` used to pay
    // a champion parse solely because evaluator metadata was bolted onto
    // construction; ordinary games must not do evaluator work per seat.
    let champion = matches!(
        kind,
        ArmKind::Evolved
            | ArmKind::AdvancedEvolved
            | ArmKind::BasicEvolved
            | ArmKind::AdvancedEvolvedBlind
            | ArmKind::AdvancedBankingDedication
    ) && crate::evolve::load_champion(dir).is_some();
    let net = matches!(kind, ArmKind::Strategic | ArmKind::StrategicWarm)
        && crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
    let league = kind == ArmKind::StrategicDeepLeague && league_generalist().is_some();
    artifact_effective_alias_from(kind, champion, net, league)
}

/// Construct a canonical, already-resolved arm. Public callers enter through
/// [`builtin_arm`] or [`builtin_ai`], never by selecting a raw string here.
fn build_arm(kind: ArmKind, seed: u64) -> Box<dyn Ai> {
    match kind.name() {
        "advanced" => Box::new(AdvancedAi::new()),
        // ONE pre-registered point on the production category genes, chosen
        // before the run and not swept. `advanced_synergy` lost 108 Elo at
        // deployment while ending on MORE districts (27.0 vs 24.1), FEWER
        // buildings (75.9 vs 94.1), 3.4x less gold (216 vs 728) and a smaller
        // army. That is one arm's correlation and 37 confounded changes, so it
        // is a hypothesis and not a finding: that this engine's build order
        // over-buys districts against buildings. #1516 reached something
        // adjacent from the audit side -- the production bundle's components
        // were selected on development, and development is not the objective.
        //
        // `settler_price` states the procedure this follows: "Pick one,
        // register it, run it once -- do not sweep several against the same
        // maps, which is the selection bias that dissolved this repository's
        // coordinate-descent result on resampling." Registered: p_building
        // 1.5, p_district 0.7, matrix seed 95000000. Whatever it returns is
        // the result.
        "advanced_build_first" => {
            let mut w = Weights::default();
            w.p_building = 1.5;
            w.p_district = 0.7;
            Box::new(AdvancedAi::with_weights(w))
        }
        // Stock production weights and stock policy, with every live-bridge
        // repair that fixes a CIVVIS engine defect rather than a Firaxis one.
        // See `AdvancedAi::enable_engine_repairs` for the four exclusions and
        // for why ablating the bundle cannot answer what the bundle is worth.
        "advanced_synergy" => {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            Box::new(ai)
        }
        "advanced_synergy_war" => {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs_war();
            Box::new(ai)
        }
        "advanced_synergy_economy" => {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs_economy();
            Box::new(ai)
        }
        // The first bounded use of the fog-safe belief surface. It retains
        // only last-seen military strength for the already fog-filtered
        // city-pressure/recovery path; every other Advanced policy remains
        // unchanged, so this is an evaluator arm rather than a fog-honesty
        // claim for the whole controller.
        "advanced_belief_pressure" => {
            let mut ai = AdvancedAi::new();
            ai.belief_pressure = true;
            Box::new(ai)
        }
        // These are the exact pre-promotion factorial controls. Production
        // `advanced` now has Live policy selection plus the retained direct
        // envoy-priority mechanism; the measured-null infrastructure arm is
        // deliberately off. Using ordinary constructors here would collapse
        // a control into the treatment it is meant to diagnose.
        "advanced_policy_live_control" => {
            let weights = Weights {
                policy_deck: crate::ai::PolicyDeck::Live,
                ..Weights::default()
            };
            Box::new(AdvancedAi::pre_policy_envoy_with_weights(weights))
        }
        "advanced_envoy_policy" => {
            let weights = Weights {
                policy_deck: crate::ai::PolicyDeck::Live,
                pol_influence: 4.0,
                ..Weights::default()
            };
            Box::new(AdvancedAi::pre_policy_envoy_with_weights(weights))
        }
        "advanced_envoy_infrastructure" => {
            let mut ai = AdvancedAi::pre_policy_envoy();
            ai.envoy_infrastructure = true;
            Box::new(ai)
        }
        // The valuation-only treatment above is routed around by ordinary
        // adaptive production. This arm keeps that valuation and additionally
        // reserves one empty city for the first legal, horizon-positive stage
        // of the Diplomatic Quarter -> Consulate -> Chancery chain.
        "advanced_envoy_priority" => {
            let mut ai = AdvancedAi::pre_policy_envoy();
            ai.envoy_infrastructure = true;
            ai.envoy_priority = true;
            Box::new(ai)
        }
        "advanced_envoy_economy" => {
            let weights = Weights {
                policy_deck: crate::ai::PolicyDeck::Live,
                pol_influence: 4.0,
                ..Weights::default()
            };
            let mut ai = AdvancedAi::pre_policy_envoy_with_weights(weights);
            ai.envoy_infrastructure = true;
            Box::new(ai)
        }
        // Treatment for the lane-reachability axis: identical to `advanced`
        // except that it refuses to route toward a victory lane it cannot
        // finish inside the turn budget. Paired against `advanced` this
        // isolates the filter and nothing else. Measured no stronger at 120
        // mirrored maps -- 49.6% paired score, Elo-equivalent -3, sign
        // p=1.0000, gate INCONCLUSIVE -- which is why it is an entrant and
        // not the default.
        // Ablation for the civilization-aware decision layer: identical to
        // `advanced` except that it ignores every by-name civilization signal
        // (the Greece and China lane floors, the unique-unit tech bonus, the
        // Egypt/China wonder exemption). It still builds whatever uniques it
        // has -- that is mechanics. Paired against `advanced` this bounds what
        // the existing per-civilization code is worth, which is the ceiling
        // any better per-civilization play has to beat. See `docs/OPENINGS.md`.
        // Treatment for the settler-commitment axis: identical to `advanced`
        // except that a settler holds its chosen site across a turn it could
        // not move, for up to three such turns. See `docs/OPENINGS.md` §15.
        "advanced_settler_commit" => {
            let mut ai = AdvancedAi::new();
            ai.settler_commit = true;
            Box::new(ai)
        }
        // Ablation for the counter-leader axis: identical to `advanced`
        // except that it never reacts to a rival closing on a victory --
        // `victory_denial` is silent and `urgent_victory_threat` never waives
        // the ordinary war-readiness checks. It still fights, expands and
        // races; it just never does any of it *because* somebody else is
        // about to win. Paired against `advanced` this is what the whole
        // denial response is worth. See `docs/COUNTERING_LEADERS.md`, which
        // measures the layer as a near-perfect predictor of the winner, no
        // deterrent, and a real cost in development on its recorded large
        // profile.
        "advanced_blind_to_leaders" => {
            let mut ai = AdvancedAi::new();
            ai.deny_leaders = false;
            Box::new(ai)
        }
        // Treatment for the early-aggression axis: identical to `advanced`
        // except that it will open an ancient rush on the nearest weak
        // neighbour before anybody can build walls. `rush_census` measures the
        // window this plays in — 0% of capitals walled through turn 60, mean
        // garrison 0.7, nearest rival capital a median 13 tiles — and a Monte
        // Carlo over the engine's own combat formulas sizes the stack at four
        // melee units. `advanced` cannot reach any of it: `assess` withholds
        // Conquest until turn 55, and the declaration carries a turn-35 floor.
        // Paired against `advanced` this is what early aggression is worth.
        "advanced_rush" => {
            let mut ai = AdvancedAi::new();
            ai.early_rush = true;
            Box::new(ai)
        }
        // Frozen route selector for the ancient-rush mechanism. It changes no
        // rush rule after target eligibility: only rivals a starting land
        // melee unit could route to the existing staging ring may trigger it.
        "advanced_rush_connected" => {
            let mut ai = AdvancedAi::new();
            ai.early_rush = true;
            ai.route_connected_rush = true;
            Box::new(ai)
        }
        // Preregistered unified midgame appointment: target, breakthrough,
        // bodies, upgrade bill, staging, declaration, and objective lifecycle
        // are one controller-owned plan. The production arm remains off until
        // the frozen paired screens establish that this capability wins.
        "advanced_timing_attack" => {
            Box::new(AdvancedAi::timing_attack())
        }
        // Selective v2 preserves the same controller-owned executor but may
        // appoint only an organically chosen Conquest target with a prebuilt
        // army, once, and launches from a stronger fully staged position.
        "advanced_timing_attack_selective" => {
            Box::new(AdvancedAi::selective_timing_attack())
        }
        // Ready-force v3 only narrows when the same unified campaign may be
        // appointed; every downstream consumer remains the v1/v2 executor.
        "advanced_timing_attack_rapid" => {
            Box::new(AdvancedAi::rapid_timing_attack())
        }
        // Treatment for the response-shape axis: identical to `advanced`
        // except that a Science or Expansion threat is answered by racing the
        // leader in that lane rather than by declaring on them. The alarm is
        // unchanged; only what it asks for changes. See
        // `docs/COUNTERING_LEADERS.md`: on its recorded large profile, one or
        // two belligerents wins 4.4% and 10.7% of seats against a 16.7% base.
        "advanced_counter_in_lane" => {
            let mut ai = AdvancedAi::new();
            ai.counter_in_lane = true;
            Box::new(ai)
        }
        // Decomposition arm for the response-shape axis: reacts to the other
        // four races exactly as `advanced` does and to a Science or Expansion
        // threat not at all. Read against `advanced_counter_in_lane` it says
        // whether that treatment's effect is "stop declaring" or "race them".
        "advanced_counter_stand_down" => {
            let mut ai = AdvancedAi::new();
            ai.counter_stand_down = true;
            Box::new(ai)
        }
        // Instrument treatment: reads the score race as a margin over the
        // field instead of as a last-quarter clock. The response is unchanged.
        "advanced_early_score_alarm" => {
            let mut ai = AdvancedAi::new();
            ai.early_score_alarm = true;
            Box::new(ai)
        }
        // Treatment for the free-counter axis: identical to `advanced` except
        // that the World Congress resolutions carrying a targeted penalty --
        // `trade_policy` B (trade embargo), `migration_treaty` B (-20% growth),
        // `border_control_treaty` B (no border-growth annexation) -- aim at the
        // empire `victory_denial` names instead of at the empire holding the
        // most Diplomatic Victory Points. `congress_census` measures that
        // shipped target as the eventual winner 24.8% of the time at 4p
        // (base 25.0%) and 14.4% at the recorded 6p profile (base 16.7%),
        // against 61% for the score leader on both. Unlike every arm in
        // `docs/COUNTERING_LEADERS.md`, this response is not paid for in
        // development: `resolve_congress` refunds a losing vote in full.
        "advanced_congress_counter" => {
            let mut ai = AdvancedAi::new();
            ai.congress_counter_leader = true;
            Box::new(ai)
        }
        // Decomposition arm: aims exactly where `advanced` aims and only
        // changes how hard it pushes, buying the second and third vote behind
        // a ballot that opposes the empire closest to a victory. Read against
        // `advanced_congress_counter` it separates the target from the weight.
        "advanced_congress_votes" => {
            let mut ai = AdvancedAi::new();
            ai.congress_counter_votes = true;
            Box::new(ai)
        }
        // Both halves at once. Only informative once the two arms above have
        // been read separately.
        "advanced_congress_counter_hard" => {
            let mut ai = AdvancedAi::new();
            ai.congress_counter_leader = true;
            ai.congress_counter_votes = true;
            Box::new(ai)
        }
        // The earlier alarm asking for a build instead of a war -- the only
        // combination `docs/COUNTERING_LEADERS.md` leaves untested, since every
        // response-side change measured null on the shipped instrument.
        "advanced_early_score_build" => {
            let mut ai = AdvancedAi::new();
            ai.early_score_alarm = true;
            ai.counter_in_lane = true;
            Box::new(ai)
        }
        "advanced_civ_blind" => {
            let mut ai = AdvancedAi::new();
            ai.civ_blind = true;
            Box::new(ai)
        }
        // Treatment for the city-decision axis: identical to `advanced`
        // except that it stamps a `CityDirective` on every city each turn, so
        // the citizen governor can see the empire's lane, the city's own role
        // and the hostile strength standing next to it. Paired against
        // `advanced` this isolates the plan-to-tile channel and nothing else.
        // See `AdvancedAi::city_strategy`.
        "advanced_city_strategy" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            Box::new(ai)
        }
        // The two ablation halves of `advanced_city_strategy`, which lost its
        // first screen at 42.5% over 120 maps. Emphasis-only carries the
        // empire's lane into the tiles and nothing else; roles-only carries
        // the local role ladder and per-city military pressure and nothing
        // else. Paired against `advanced` they attribute that loss instead of
        // leaving it to be guessed at.
        "advanced_city_strategy_emphasis" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_roles = false;
            Box::new(ai)
        }
        "advanced_city_strategy_roles" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            Box::new(ai)
        }
        // The frozen pre-repair controls: the role ladder allowed to type a
        // city comparatively while the empire is still expanding, which is
        // what the 42.1% and 42.5% results were measured on. Kept so those
        // numbers stay reproducible now that the repair is the default.
        // Isolates only the Bastion rung: a locally outmatched city halts growth and wants hammers.
        "advanced_city_strategy_bastion_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_halt_growth = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_breadbasket = false;
            ai.city_strategy_comparative = false;
            ai.city_strategy_pressure = false;
            Box::new(ai)
        }
        // Isolates only the Breadbasket rung: the best food city feeds settlers while the empire is short.
        "advanced_city_strategy_breadbasket_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_bastion = false;
            ai.city_strategy_comparative = false;
            ai.city_strategy_pressure = false;
            Box::new(ai)
        }
        // Isolates only the comparative rungs, Forge and Specialist.
        "advanced_city_strategy_comparative_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_bastion = false;
            ai.city_strategy_breadbasket = false;
            ai.city_strategy_pressure = false;
            Box::new(ai)
        }
        // Isolates only per-city military pressure, with no role ever assigned.
        "advanced_city_strategy_pressure_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_bastion = false;
            ai.city_strategy_breadbasket = false;
            ai.city_strategy_comparative = false;
            Box::new(ai)
        }
        "advanced_city_strategy_roles_raw" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_halt_growth = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_expansion_first = false;
            Box::new(ai)
        }
        "advanced_city_strategy_raw" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_halt_growth = true;
            ai.city_strategy_expansion_first = false;
            Box::new(ai)
        }
        // Treatment for the expansion-window axis: identical to `advanced`
        // except the settler gate closes on whether a settler built here and
        // now would pay for itself, rather than on a flat end-of-game reserve.
        // See `AdvancedAi::expansion_pays_back` and #554/#559.
        "advanced_expansion_payback" => {
            let mut ai = AdvancedAi::new();
            ai.expansion_pays_back = true;
            Box::new(ai)
        }
        // The two frozen adaptive-expansion axes and their factorial
        // composition. They are evaluator-only: `advanced` itself leaves both
        // flags false, while the mechanism census proves actual production
        // actions before any outcome seed is read.
        "advanced_late_expansion" => {
            let mut ai = AdvancedAi::new();
            ai.late_expansion = true;
            Box::new(ai)
        }
        "advanced_expansion_dispatch" => {
            let mut ai = AdvancedAi::new();
            ai.expansion_dispatch = true;
            Box::new(ai)
        }
        "advanced_expansion_complete" => {
            let mut ai = AdvancedAi::new();
            ai.late_expansion = true;
            ai.expansion_dispatch = true;
            Box::new(ai)
        }
        // Treatment for the full paid expansion sequence. It routes the
        // adaptive Expansion lane through strategic production and charges
        // production, population, escort, travel, safety, and payback costs
        // before a Settler can outbid another build.
        "advanced_coupled_expansion" => Box::new(AdvancedAi::coupled_expansion()),
        // Treatment for the city-target axis: identical to `advanced` except
        // that the target ramp starts at six rather than three. See
        // `AdvancedAi::city_target_floor`, #554 and #569.
        // ⚠ Since 2026-08-10 this is a treatment again rather than a
        // re-labelling of production: `promoted_policy_envoy` no longer sets
        // the floor, because withholding it passed the promotion matrix at
        // +41 Elo on deployment-online.
        "advanced_wide_opening" => {
            let mut ai = AdvancedAi::new();
            ai.city_target_floor = civvis_production_city_target_floor();
            Box::new(ai)
        }
        // Upper bound for the expansion-valuation axis: a settler outbids every
        // other item whenever the five gates permit one at all. This is not a
        // shippable policy, it is the oracle-ablation question — is there ANY
        // headroom in what a settler is worth? The gates still bind (one in
        // flight, stop at the planned target, pop 2, window open), so it means
        // "beeline to the city target", not "settlers forever".
        "advanced_settler_first" => {
            let mut ai = AdvancedAi::new();
            ai.settler_price = 100.0;
            Box::new(ai)
        }
        // Treatment for the district-lane axis: identical to `advanced` except
        // that the Holy Site outranks the Campus instead of trailing both the
        // Campus and the Commercial Hub.
        //
        // The value is not chosen, it is read off the shipped roster. Of the
        // bred `Advanced` genomes in `data/league/league.json` carrying real
        // 8-player evidence, the top third by *outright win rate* sit at a
        // games-weighted `d_holy` of 5.6 while the bottom third sit at 2.0 --
        // the shipped default, and the largest top-versus-bottom separation of
        // any of the forty genes (0.41 of its legal range; weighted r = +0.62
        // against win rate). The bottom-third contrast is the control that
        // matters: a gene under no selection pressure drifts in both tails, and
        // this one does not move in the losing tail at all.
        //
        // ⚠ That is a correlation over roughly fifty survivors related by
        // descent, which is a hypothesis and not a result -- and the same
        // roster's *Glicko* ordering is on record ranking two agents backwards
        // by 230 Elo (see 2026-07-28 in `docs/EVAL.md`). This entrant exists so
        // the axis can be put to the paired evaluator instead of argued from
        // the roster. Note the win-rate ordering used here independently places
        // `g56-50` -- the genome that measured -108 Elo against the champion --
        // last of the eight, which is the ordering that result implies.
        //
        // Mechanistically it is a plausible miscalibration rather than a lucky
        // draw: `docs/EVAL.md` records that religious victory dominates
        // self-play in this engine, and the shipped default ranks the district
        // that lane runs through *below* two others.
        // ⚠ A treatment again. It shipped into `advanced` on a +20 Elo gate
        // taken at `ai_eval`'s 4p 24x16 defaults, and was reverted the same day
        // when the deployment shape measured it at parity and the promotion
        // matrix at -44. It stays registered because the axis is real and the
        // profile dependence is the finding; see `docs/EVAL.md` 2026-08-10.
        "advanced_holy_priority" => Box::new(AdvancedAi::with_weights(Weights::advanced())),
        // The scripted major exactly as it played before 2026-08-10, retained so
        // the change stays measurable after it ships -- the same role
        // `advanced_v1` fills for the controller as a whole.
        // Treatment for the settle-site yield axis. `settle_site_value` scores
        // a candidate city by its surrounding tiles at
        // `food*settle_food + production*settle_prod + gold*settle_gold`, and
        // the shipped weights are food-dominant: 1.2 against production's 1.0.
        //
        // The roster inverts that. Of the bred genomes carrying real 8-player
        // evidence, the top third by outright win rate sit at a games-weighted
        // `settle_food` of 0.77 against the bottom third's 1.19 -- the second
        // largest top-versus-bottom separation of the forty genes after
        // `d_holy`, and the losing tail again sits on the shipped default.
        // Their `settle_prod` barely moves (0.89), so the winners are not
        // pricing production up, they are pricing food DOWN past it.
        //
        // ⚠ The tempting mechanism is that food is only worth what a city can
        // grow into, and growth is housing-capped -- but the 71.7% housing-cap
        // figure in `AdvancedAi`'s deck notes is measured on **Civ 6
        // host-exported** city-turns, not on this engine, so it motivates the
        // test and does not explain the result. What this arm establishes is
        // whether the axis pays here, not why.
        "advanced_settle_food" => {
            let mut w = Weights::default();
            w.settle_food = LEAGUE_WINNER_SETTLE_FOOD;
            Box::new(AdvancedAi::with_weights(w))
        }
        // The second cell of the 2x2 that reads the lane-table null.
        //
        // `advanced_holy_lane` against `advanced` left 399 of 400 maps
        // untouched, but both arms already pay `d_holy` 5.6, so a ceiling
        // ("BasicAi builds the Holy Site anyway, the lane term is redundant")
        // and an inert path ("the lane term decides nothing") predict the same
        // flat result. This arm carries the lane change on the PRE-shipment
        // weights, so measured against `advanced_holy_v0` it separates them: a
        // gain here means redundancy, another null means the path itself does
        // not bind.
        // Everything the roster's winners agree on, restricted to coordinates
        // that can actually reach a decision.
        //
        // Single-gene mining is close to exhausted: `d_holy` shipped at +20
        // Elo, and after removing the genes `gene_census` proved inert every
        // remaining separation is under a tenth of its gene's legal range. So
        // this composes them, in the same spirit as `Grant::Compound` -- if no
        // single subsystem clears the bar, ask whether several together do.
        //
        // Values are the games-weighted mean of the top four bred genomes by
        // outright 8-player win rate (>=200 games each). Genes within 2% of the
        // shipped value are left alone, the eight inert genes are skipped
        // because by construction they cannot contribute, and `d_holy` is
        // already at the winners' figure.
        //
        // ⚠ This is a SCREEN, not a proposal. 28 genes move at once, so a win
        // says only that something in the set pays and a null says the whole
        // set does not -- neither attributes anything to a gene. The roster's
        // own false-positive rate is measured and high: four of its top
        // fourteen signals, including the second-strongest (`settle_food`,
        // r=-0.73), are genes that provably cannot change a game.
        "advanced_roster_live" => {
            let mut w = Weights::default();
            w.city_target = 6.4714;
            w.settler_stop_turn = 162.6394;
            w.mil_per_city = 0.9332;
            w.builder_per_city = 0.5816;
            w.attack_floor = -2.6485;
            w.kill_bonus = 27.8323;
            w.trade_caution = 1.2266;
            w.min_city_dist = 4.1185;
            w.faith_builder = 260.3021;
            w.d_campus = 4.6378;
            w.d_commercial = 5.5494;
            w.d_theater = 0.4972;
            w.open0 = 0.5105;
            w.open1 = 3.4783;
            w.open2 = 1.7629;
            w.open3 = 4.5988;
            w.mv_support = 2.4320;
            w.mv_threat = 0.4698;
            w.command_radius = 3.2010;
            w.muster_readiness = 0.6351;
            w.cohesion = 6.8153;
            w.focus_fire = 2.0175;
            w.screen = 10.1270;
            w.role_spacing = 0.7614;
            w.objective_progress = 2.9090;
            w.local_superiority = 5.7622;
            w.withdraw_hp = 41.0983;
            w.rejoin_hp = 78.3851;
            Box::new(AdvancedAi::with_weights(w))
        }
        // `advanced_roster_live` minus its district lane, to test why the
        // composite lost.
        //
        // The composite raises `d_commercial` 3.0 -> 5.55, which all but ties
        // the `d_holy` 5.6 that shipped at +20 Elo, and its religious wins fell
        // 470 -> 392 against the control. The reading is that the roster's own
        // district preferences dilute the district priority already measured on
        // this engine. This arm holds `d_campus`, `d_commercial` and
        // `d_theater` at the shipped values and takes the roster's other
        // twenty-five live genes unchanged, so recovery toward parity confirms
        // the districts carried the loss and a second loss acquits them.
        "advanced_roster_live_keep_districts" => {
            let mut w = Weights::default();
            w.city_target = 6.4714;
            w.settler_stop_turn = 162.6394;
            w.mil_per_city = 0.9332;
            w.builder_per_city = 0.5816;
            w.attack_floor = -2.6485;
            w.kill_bonus = 27.8323;
            w.trade_caution = 1.2266;
            w.min_city_dist = 4.1185;
            w.faith_builder = 260.3021;
            w.open0 = 0.5105;
            w.open1 = 3.4783;
            w.open2 = 1.7629;
            w.open3 = 4.5988;
            w.mv_support = 2.4320;
            w.mv_threat = 0.4698;
            w.command_radius = 3.2010;
            w.muster_readiness = 0.6351;
            w.cohesion = 6.8153;
            w.focus_fire = 2.0175;
            w.screen = 10.1270;
            w.role_spacing = 0.7614;
            w.objective_progress = 2.9090;
            w.local_superiority = 5.7622;
            w.withdraw_hp = 41.0983;
            w.rejoin_hp = 78.3851;
            Box::new(AdvancedAi::with_weights(w))
        }
        // Let the Diplomacy lane be entered before it has already succeeded.
        //
        // An actuation treatment, not a valuation one: every other lane in
        // `best_lane` is scored prospectively and Diplomacy alone is scored
        // retrospectively, so its argmax input is a self-fulfilling zero and
        // the lane takes 0.9% of observed player-turns. The prize behind it is
        // the largest the oracle harness has found -- `Grant::Suzerain` at
        // 56.7% against a 22.7% control, p=0.0000 over 400 maps (PR #602).
        //
        // The opening figure is Religion's own, so this loses every tie to
        // Religion and can only win the argmax against Conquest and Expansion,
        // which score zero. See `AdvancedAi::diplomatic_opening_score`.
        // `bounded_recovery` was historically bundled here, then confirmed
        // null over 600 maps on two disjoint deployment seeds. The dated
        // experiment remains in `docs/EVAL.md`; production leaves the arm off,
        // while live-bridge and explicit evaluator constructors opt in.
        // Withhold the production city-target floor, returning it to the 3 the
        // frozen controller uses.
        //
        // The component with the weakest individual case in the whole
        // production bundle. Its solo axis is a **recorded null** — the
        // `city_target_floor` 3 -> 6 ramp measured 49.6%, Elo -3, sign
        // p=0.9007 over 240 pairs on seed 510000, after a 53.3% first reading
        // that did not reproduce, and the entrant was removed. `GENOME.md` puts
        // city expansion "at a local optimum", and every expansion treatment
        // since has been null: the target ramp, parallel settlers, and a
        // settler priced at 100x that moved cities by 0.06.
        //
        // It nevertheless ships, inside the 2026-08-01 composite. A composite
        // may pass while a component is null alone, so this is not an
        // accusation — it is the missing measurement. If withholding it is
        // positive, the bundle is carrying a part that costs.
        // Withhold the plan-driven city target: the production governor goes
        // back to the flat `city_target` gene instead of the empire's own
        // `plan.desired_cities`.
        //
        // The second of the two expansion levers `promoted_policy_envoy` sets.
        // The first, `city_target_floor = 6`, measured **-41 Elo** at the
        // deployment shape and was removed on 2026-08-10 — it bought cities,
        // population and terminal score, and paid wins for them. This is the
        // one still standing, it has no individual number, and it is measured
        // now rather than earlier because the two interact: with the floor gone
        // the plan target is what remains driving expansion.
        //
        // ⚠ `advanced_plan_city_target` cannot measure this. Production already
        // sets the flag, so "new() plus the flag" is byte-identical to the
        // control — the third arm in this file to carry that defect. It is
        // aliased to `advanced` below so the comparison fails closed instead of
        // reporting a confident null.
        // The third expansion-adjacent lever in the production bundle, and the
        // next unpriced one in the #1499 ledger. Registered here so the queue
        // stays visible; its number is pending.
        // Everything in the production bundle that still has no individual
        // number, withheld at once.
        //
        // Pricing nine flags one at a time is nine deployment runs. This asks
        // the prior question in one: **is there anything left in the
        // remainder at all?** A null bounds the whole set and closes the queue;
        // a large effect says keep bisecting, and the bisection is then paid
        // for. The same screen-then-decompose shape found the district genes
        // inside the roster composite (#1486).
        //
        // Retired measured-null production arms are no longer withheld here:
        // `bounded_recovery` (null over 600 maps) and
        // `envoy_infrastructure` (null at 800 games). The remaining retained
        // bundle still needs individual pricing. Other priced components are
        // `city_target_floor` (-41 Elo, removed #1504) and
        // `plan_city_target` (null, #1507),
        // `settler_commit` — which measured **+30 Elo** (withholding it scores
        // 45.6%, 60/95, sign p=0.0061), so it is a component the bundle should
        // keep and including it here would drag the screen negative for the
        // wrong reason.
        //
        // ⚠ A composite says nothing about any single flag. If this moves, the
        // decomposition is the work, not the headline.
        // The two base-constructor defaults that no arm could reach. `configured`
        // sets ten booleans for every non-legacy `AdvancedAi`; `deny_leaders`
        // had `advanced_blind_to_leaders` and these two had nothing at all.
        // The `promoted_policy_envoy` audit found -41 Elo among flags in this
        // exact condition, so "always on and unmeasurable" is where that
        // mistake lived.
        "advanced_without_settlement_safety" => {
            let mut ai = AdvancedAi::new();
            ai.disable_settlement_safety();
            Box::new(ai)
        }
        // ★ A PREDICTION, not a search. Declared before the run.
        //
        // Three withholds at the deployment shape resolved the settling lane
        // into one statement: **ambition costs, execution pays.** Wanting more
        // cities measured **-41 Elo** (`city_target_floor`, removed #1504);
        // committing to a settler already built measured **+30**
        // (`settler_commit`); putting the city somewhere it survives measured
        // **+31** (`settlement_safety`). Two execution mechanisms pay about
        // what the one ambition knob cost.
        //
        // If that is a property of the engine rather than a story fitted to
        // three results, it should predict an axis it was not derived from.
        // `city_target` is the other "how many cities to want" knob — the flat
        // gene the baseline production governor caps on, distinct from the
        // plan's floor and untouched by #1504. **Prediction: lowering it from
        // 4.0 to the 3 the plan now starts at is POSITIVE.**
        //
        // ⚠ The prediction is what makes this worth running, and it is also
        // what makes a null informative: `GENOME.md` puts city expansion "at a
        // local optimum", so a null says the principle does not extend past
        // the three flags that produced it, and it should stop being quoted as
        // though it does. Recorded either way.
        // A settler that cannot reach a better site founds where it stands.
        //
        // An **actuation** repair, the class that produced the only shipped
        // gain this session. Founding is gated on the settler's chosen target
        // equalling its own tile, so one holding out for a site it never
        // reaches never founds. `audit` at the deployment shape names it:
        // "settler sits still 25+ turns … can_found_here=true, legal_sites=644"
        // — a unit that cost a population point and 80-140 production, idle
        // from turn 34 to the end of the game, standing on legal ground. Eight
        // in eight games, plus eight more circling without progress.
        //
        // It is also the settling lane's own lesson applied to a unit rather
        // than a gene: wanting a better site cost 41 Elo as
        // `city_target_floor`; finishing the settlement already begun paid +30
        // (`settler_commit`) and +31 (`settlement_safety`).
        // A unit whose planner gave it nothing to do takes the free defensive
        // stance instead of standing in the open.
        //
        // `hold_stood_down_unit` fortified only inside a stand-down window, so
        // a unit that merely took no turn was left unfortified. `audit`
        // measures the size: **7.73% of major-civ unit-turns** are an
        // unembarked land military unit standing still that could have
        // fortified — 10,477 across eight games — against **3.59%** that are
        // fortified. `unit_strength` pays **+3 per fortified turn capped at
        // two**, so each declines about 30% of a warrior's base strength.
        //
        // ⚠ Chosen by RATE, not by symptom count. The same audit reports 94
        // circling warriors, which is 1.21% of unit-turns; and the previous
        // repair in this file fixed a settler idling two hundred turns and
        // measured ~5 Elo because it touched 14 maps in 400. Frequency is the
        // better guide, and still not a substitute for the paired run.
        // The two halves of `advanced_without_unpriced_bundle`.
        //
        // That composite measured **+9 Elo, CI -25..+43, 94/84, p=0.50** — null
        // on the NET. A net is not a bound on the parts, and in this bundle
        // that is not a formality: `city_target_floor` at **-41** and
        // `settler_commit` at **+30** are demonstrated offsetting components of
        // the same constructor. A +9 across eight flags is perfectly compatible
        // with a -30 and a +40 inside it.
        //
        // Split on the line the flags themselves draw — what the empire builds
        // against how it fights — so a half that moves names a coherent
        // subsystem rather than an arbitrary four.
        "advanced_without_unpriced_economy" => {
            let mut ai = AdvancedAi::new();
            ai.envoy_priority = false;
            ai.adjacency_site_planning = false;
            ai.research_economy = false;
            ai.disable_amenity_districts();
            Box::new(ai)
        }
        // The two quarters of the war half.
        //
        // Withholding all four measured **+32 and +34 Elo** on two disjoint
        // seeds at the exhibition's configuration (e-process crossed at map
        // 134), and **+13, p=0.4671** on the promotion matrix's three-victory
        // `deployment-online`, which rejected it. A group's number is not its
        // members' — `city_target_floor` (-41) and `settler_commit` (+30) sat
        // in the same constructor — so one of these quarters may carry the
        // effect on both profiles where the whole cannot.
        //
        // Split on what the flag governs: the city's own defence against the
        // individual unit's behaviour.
        // The 24th always-on production behaviour, and the one the audit of
        // `promoted_policy_envoy` and `configured` missed: `production_weights`
        // overwrites `policy_deck` with `Live` after the weights are handed
        // over.
        //
        // `Weights::default()`'s comment beside `PolicyDeck::Legacy` says "the
        // agent that plays is the one that always played" and records `Live` as
        // a **measured null** — 18 map directions to 15, p=0.7283 over 120
        // mirrored maps — that "costs an empire valuation per candidate card
        // per review". Production plays `Live` regardless, `docs/EVAL.md` has
        // never mentioned `policy_deck`, and no caller could withhold it
        // because the override happens after construction.
        //
        // 120 maps is well under what this file now treats as resolving
        // anything, so the null it rests on is not one either.
        // The 25th production behaviour, and the second found outside the two
        // constructors the audit swept: `delegated_cities` raises
        // `builder_per_city` from the genome's 0.5 to 0.75 with a call-local
        // `.max()`. Reachable from nowhere else, never mentioned in
        // `docs/EVAL.md`, and justified by reasoning rather than a number —
        // "three active Builders per four cities provide roughly two useful
        // improvements per city".
        //
        // Same profile as `city_target_floor`, which was also a production-only
        // floor justified by argument and measured at **-41 Elo**. That is the
        // reason to look, not a prediction: the two other expansion-adjacent
        // floors beside this one measured null and +30.
        // The last production-only override with no number. `delegated_cities`
        // extends `settler_stop_turn` from the genome's 150 to
        // `min(300 standard, max_turns - 50 standard)`. With this the sweep of
        // everything that separates the shipped controller from its genome is
        // complete.
        "advanced_without_settler_deadline" => {
            let mut ai = AdvancedAi::new();
            ai.disable_production_settler_deadline();
            Box::new(ai)
        }
        "advanced_price_suzerainty" => {
            let mut ai = AdvancedAi::new();
            ai.enable_price_the_suzerainty();
            Box::new(ai)
        }
        "advanced_without_builder_floor" => {
            let mut ai = AdvancedAi::new();
            ai.disable_production_builder_floor();
            Box::new(ai)
        }
        "advanced_without_hut_collection" => {
            let mut ai = AdvancedAi::new();
            ai.disable_hut_collection();
            Box::new(ai)
        }
        "advanced_without_explore_commit" => {
            let mut ai = AdvancedAi::new();
            ai.disable_explore_commit();
            Box::new(ai)
        }
        "advanced_without_village_seeking" => {
            let mut ai = AdvancedAi::new();
            ai.disable_village_seeking();
            Box::new(ai)
        }
        "advanced_legacy_policy_deck" => Box::new(AdvancedAi::with_legacy_policy_deck()),
        // The declared aliases of `advanced` (the war-half withhold trio,
        // `advanced_plan_city_target`, `advanced_without_city_target_floor`,
        // `advanced_holy_v0`, `advanced_policy_envoy_priority`) have no
        // bodies here: `artifact_effective_alias_from` collapses them before
        // construction, and both callers of this factory pass the collapsed
        // kind. The evidence for each aliasing lives on the collapse arms.
        // Treatment for the war-half axis: identical to `advanced` except that
        // the four flags removed from `promoted_policy_envoy` on 2026-08-14
        // are turned back on. Since that removal this is a treatment rather
        // than a re-labelling of production, exactly as `advanced_wide_opening`
        // became for `city_target_floor` after #1504. Withholding the four
        // measured +32/+34 on seeds 10800000/11000000 and +38 (CI +10..+66)
        // on the corrected-gate matrix at seed stream 18000000; this arm asks
        // the inverse question if anyone re-opens the axis.
        "advanced_war_half" => {
            let mut ai = AdvancedAi::new();
            ai.enable_siege_muster();
            ai.enable_home_defense();
            ai.enable_tactical_strategy();
            ai.enable_unit_objective_memory();
            Box::new(ai)
        }
        // The Builder priced by the work it would do rather than a headcount
        // quota: charges from `Game::builder_charges`, jobs from the same
        // valuation Builder movement uses, luxury novelty and strategic
        // saturation modelled, quota sized from the backlog. Reserved matrix
        // seed 25000000.
        "advanced_every_lane" => {
            let mut ai = AdvancedAi::new();
            ai.enable_governor_every_lane();
            Box::new(ai)
        }
        "advanced_builder_survey" => {
            let mut ai = AdvancedAi::new();
            ai.enable_builder_reward_survey();
            Box::new(ai)
        }
        // Military units credited for strength-per-production within their
        // role and for being the civilization's own unique unit while the
        // window is open. Reserved matrix seed 26000000.
        "advanced_unit_efficiency" => {
            let mut ai = AdvancedAi::new();
            ai.enable_unit_cost_efficiency();
            Box::new(ai)
        }
        "advanced_fortify_idle_units" => {
            let mut ai = AdvancedAi::new();
            ai.enable_fortify_idle_units();
            Box::new(ai)
        }
        // Build hulls only where they have open water to sail into. The
        // enqueue path gated on `city_is_coastal`, and a lake is water, so a
        // lakeside city built Galleys that never left the lake: 20 of 53 major
        // hulls never moved once in a three-game audit.
        "advanced_open_water_navy" => {
            let mut ai = AdvancedAi::new();
            ai.enable_open_water_navy();
            Box::new(ai)
        }
        // Reach for the +100% naval-production card while hulls are wanted:
        // the family is invisible to the deck scorer until a sea unit heads a
        // queue and appears in no portfolio, so the Galley-era discount was
        // never slotted. See `AdvancedAi::naval_production_policy`.
        "advanced_maritime_splice" => {
            let mut ai = AdvancedAi::new();
            ai.enable_naval_production_policy();
            Box::new(ai)
        }
        // Sea threats get sea answers: a barbarian raider on water counts
        // toward the wartime second exploration hull, ships join the
        // home-defense pool, and responder domain matches the threat's tile.
        "advanced_sea_answers" => {
            let mut ai = AdvancedAi::new();
            ai.enable_sea_answers();
            Box::new(ai)
        }
        // Withhold the native barbarian-scout exemption so its promotion is
        // priced: the stock controller ships it ON.
        "advanced_without_barbarian_scouts_are_scouts" => {
            let mut ai = AdvancedAi::new();
            ai.disable_barbarian_scouts_are_scouts();
            Box::new(ai)
        }
        // Read the Faith price from the engine rather than the Standard-speed
        // `spec.cost * 2.0` literal. At Online -- the deployment and live-bridge
        // speed -- that literal asks for twice what the engine charges, and it
        // ignores every belief, government and district discount.
        "advanced_engine_faith_price" => {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_faith_price();
            Box::new(ai)
        }
        // Let the deck counterfactual see the unit-maintenance bill, so
        // Conscription and Levee en Masse stop scoring exactly 0.0. Every
        // other card's bill cancels in the with/without difference.
        "advanced_maintenance_deck" => {
            let mut ai = AdvancedAi::new();
            ai.enable_maintenance_aware_deck();
            Box::new(ai)
        }
        // The reconnaissance quartet was PROMOTED into `promoted_policy_envoy`
        // after the corrected-gate matrix PASSED at 400 pairs (deployment
        // 55.0%, Elo +35, CI +1..+69; compact no-regression ACCEPT; seed
        // stream 120000000). `advanced_recon_fleet` collapsed to a declared
        // alias of `advanced` above; withholding is the live question now.
        "advanced_without_recon_fleet" => {
            let mut ai = AdvancedAi::new();
            ai.disable_recon_replacement();
            ai.disable_recon_flight();
            ai.disable_naval_recon();
            ai.disable_come_ashore();
            Box::new(ai)
        }
        "advanced_settler_founds_when_stalled" => {
            let mut ai = AdvancedAi::new();
            ai.enable_settler_founds_when_stalled();
            Box::new(ai)
        }
        "advanced_lower_city_target" => {
            let mut w = Weights::default();
            w.city_target = LOWERED_CITY_TARGET;
            Box::new(AdvancedAi::with_weights(w))
        }
        "advanced_without_battlefront_observation" => {
            let mut ai = AdvancedAi::new();
            ai.disable_battlefront_observation();
            Box::new(ai)
        }
        "advanced_without_unpriced_bundle" => {
            let mut ai = AdvancedAi::new();
            ai.envoy_priority = false;
            ai.adjacency_site_planning = false;
            ai.research_economy = false;
            ai.disable_amenity_districts();
            ai.disable_siege_muster();
            ai.disable_home_defense();
            ai.disable_tactical_strategy();
            ai.disable_unit_objective_memory();
            Box::new(ai)
        }
        "advanced_without_settler_commit" => {
            let mut ai = AdvancedAi::new();
            ai.settler_commit = false;
            Box::new(ai)
        }
        "advanced_without_plan_city_target" => {
            let mut ai = AdvancedAi::new();
            ai.plan_city_target = false;
            Box::new(ai)
        }
        "advanced_diplomatic_opening" => {
            let mut ai = AdvancedAi::new();
            ai.diplomatic_opening = true;
            Box::new(ai)
        }
        "advanced_holy_lane_v0" => {
            let mut w = Weights::default();
            w.d_holy = PRE_2026_08_10_D_HOLY;
            let mut ai = AdvancedAi::with_weights(w);
            ai.holy_lane_parity = true;
            Box::new(ai)
        }
        // Upper bound for the lane-district axis: a Religion empire prices its
        // own Holy Site the way a Culture empire prices its own Theater Square.
        // See `AdvancedAi::holy_lane_parity` for why this is a bound and not a
        // proposal, and why the table it edits has never been measured.
        "advanced_holy_lane" => {
            let mut ai = AdvancedAi::new();
            ai.holy_lane_parity = true;
            Box::new(ai)
        }
        // A high-rated league genome against the genome the repository evolved.
        // The exhibition now samples each civ's top three eligible entries, so
        // this is a reproducible diagnostic from the shipped roster rather than
        // a claim that every live seat uses this genome. Whether a 1790-rated
        // league genome is genuinely stronger than the champion or merely
        // better-rated is the question this entrant exists to answer;
        // `docs/LEAGUE_GENOME_CHALLENGER.md` records the best-rated one losing
        // 98 Elo when transferred into `strategic_deep`.
        //
        // Picks the highest-rated active genome from the SHIPPED roster, so it
        // is reproducible from the tree rather than from a local league.
        // The cost-performance frontier of the macro search, which nothing has
        // measured because nothing was ever cost-bound. Every registered
        // variant moves the budget UP (`review_every` 20 and 10, `horizon` 80,
        // the 4x `strategic_deep`); this is the first that moves it down.
        //
        // At the measured 6p/74x46 profile, one searching seat among five
        // scripted ones cost 76.7 ms per game-turn versus 13.3 for the
        // all-scripted fleet (6.4x). Search strength across the rotating live
        // profiles remains open, so the deployable question is how much gain
        // survives at a justified budget and profile.
        //
        // Three knobs, all multiplicative on branch-rounds:
        //   review_every 40 -> 80   half the reviews
        //   horizon      40 -> 20   half the rollout
        //   rotate_lanes           ~7 branches -> ~3
        // ≈ 9x cheaper. `rotate_lanes` is a recorded NULL for strength, which
        // is exactly what a cost-bound deployment wants from it.
        "strategic_cheap" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 80;
            ai.horizon = 20;
            ai.rotate_lanes = true;
            Box::new(ai)
        }
        "advanced_league_top" => {
            let weights = shipped_league_top_advanced().unwrap_or_default();
            Box::new(AdvancedAi::with_weights(weights))
        }
        // Plan the turn's whole engagement jointly instead of one unit at a time
        // in a fixed class order. Paired against `advanced` this isolates the
        // commitment rule and nothing else — the same per-unit evaluator, the
        // same weights, the same everything above the battlefield.
        "advanced_joint_tactics" => {
            let mut ai = AdvancedAi::new();
            ai.joint_tactics = true;
            Box::new(ai)
        }
        // The denial ablation on the weights the deployment actually plays.
        // Every other arm in `docs/COUNTERING_LEADERS.md` ran on
        // `Weights::default()`, and a genome moves `war_ratio`, `city_target`
        // and the rest -- so a layer that is worth nothing to the default
        // agent is not automatically worth nothing to the shipped one. Paired
        // against `advanced_evolved`, this is the same ablation on the seat
        // the exhibition fills.
        "advanced_evolved_blind" => {
            let mut ai = crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_default();
            ai.deny_leaders = false;
            Box::new(ai)
        }
        "advanced_evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_default(),
        ),
        // The Dedication chooser that ranks the offer by what each Dedication
        // would have paid over the era just ended. **A recorded negative
        // result**, kept as an evaluator arm: over 120 mirrored maps against
        // the shipped alphabetical default it took 41.2% of games, 10 map
        // directions to 31, sign p=0.0015, e-process crossing against it at
        // map 51, and terminal score 46.3% (p=0.0000). See `docs/AGES.md`.
        "advanced_measured_dedication" => {
            let mut w = crate::evolve::load_champion("evolved").unwrap_or_default();
            w.dedication_choice = crate::ai::DedicationChoice::Measured;
            Box::new(AdvancedAi::with_weights(w))
        }
        // The repair for that loss: rank on the projection only in a Normal or
        // Dark Age, where Era Score is the literal objective, and leave the
        // Golden and Heroic choice exactly as the default makes it.
        "advanced_banking_dedication" => {
            let mut w = crate::evolve::load_champion("evolved").unwrap_or_default();
            w.dedication_choice = crate::ai::DedicationChoice::Banking;
            Box::new(AdvancedAi::with_weights(w))
        }
        // ★★★★★ THE VICTORY LANE THE REAL GAMES ARE ACTUALLY GIVEN.
        //
        // `civ6_civvis_climb.py --victory` defaulted to `domination` until #859
        // (2026-08-02; `civ6_brain.py`'s own default followed 2026-08-14), and
        // every one of the 104 ladder rows to that point carries it, with
        // **zero wins**. Nothing in the registry could measure that choice, so
        // the single most consequential setting in the deployment was the one
        // axis never evaluated — until these arms.
        //
        // ⚠ It is not merely unwon, it is out of budget: `victory_eval`'s own
        // per-target turn limits are 650 for Domination and 300 for Score, and
        // the deployment runs **250**. At 6 players / 250 turns, 8 of 8 games
        // targeting domination ended by score at the limit instead.
        "advanced_target_domination" => {
            Box::new(AdvancedAi::targeting(crate::ai::VictoryTarget::Domination))
        }
        "advanced_target_score" => Box::new(AdvancedAi::targeting(crate::ai::VictoryTarget::Score)),
        // The four that had no arm. Each differs from `advanced` only in the
        // lane it is handed, exactly as the two above do.
        "advanced_target_science" => {
            Box::new(AdvancedAi::targeting(crate::ai::VictoryTarget::Science))
        }
        // Preserve the Science target and move only the classifier, so the
        // pre-registered pair has exactly one semantic axis.
        "advanced_great_work_veto_by_district" => {
            let mut ai = AdvancedAi::targeting(crate::ai::VictoryTarget::Science);
            ai.enable_great_work_veto_by_district();
            Box::new(ai)
        }
        "advanced_target_culture" => {
            Box::new(AdvancedAi::targeting(crate::ai::VictoryTarget::Culture))
        }
        // The targeted Culture control has this treatment off. Its paired arm
        // proves the Theater-building debt is reachable without changing the
        // target or importing the rest of the live bridge.
        "advanced_target_culture_with_culture_building_debt" => {
            let mut ai = AdvancedAi::targeting(crate::ai::VictoryTarget::Culture);
            ai.enable_culture_building_debt();
            Box::new(ai)
        }
        "advanced_target_religious" => {
            Box::new(AdvancedAi::targeting(crate::ai::VictoryTarget::Religion))
        }
        "advanced_target_diplomatic" => {
            Box::new(AdvancedAi::targeting(crate::ai::VictoryTarget::Diplomacy))
        }
        // Prices `limitanei` (+2 Loyalty in garrisoned cities), which the hardcoded
        // portfolios in `strategic_policies` never reach. Revolts are 42% of 192
        // observed city losses and holding is worth roughly double the score, but
        // that is a mechanism, not an effect size — this arm exists to find out.
        //
        // It found out (2026-08-14, matrix at 400 pairs, seed 19000000): a
        // clean null. compact-standard 49.6% (CI 44.8..54.5), Elo -3,
        // direction 79/80, p=1.0000; deployment-online 47.9% (CI 43.0..52.8),
        // Elo -15 (CI -49..+19), direction 44/61, p=0.1180;
        // neither e-process moved. RETAIN. The revolt census stands; pricing
        // the card into the slot decision wins nothing measurable — the same
        // lesson as suzerainty (#1575): a mechanism that fires is not a
        // mechanism that matters. See `docs/EVAL.md` 2026-08-14.
        "advanced_garrison_loyalty" => {
            let mut ai = AdvancedAi::new();
            ai.garrison_loyalty_policy = true;
            Box::new(ai)
        }
        "advanced_v1" => Box::new(AdvancedAi::legacy()),
        "fog_honest" => Box::new(AdvancedAi::fog_honest()),
        // ★★★★ THE AGENT THAT ACTUALLY PLAYS CIVILIZATION VI, PLAYABLE HEADLESS.
        //
        // Eight flags separate the frozen controller from the deployed one, and
        // until now they were set inside a binary, so no arm could construct the
        // deployed agent and none of them had ever been priced on an outcome.
        // `live` is that adaptive agent; `live_target_*` carries the same
        // bridge with one explicit `--victory` lane; and each `live_without_*`
        // holds ONE flag off. So `civvis tournament --ais
        // live,live_without_home_defense` measures that flag in cities and
        // score rather than in order counts.
        //
        // ⚠ These are NOT rating anchors. They move whenever the bridge moves,
        // which is exactly what an anchor must not do.
        "live" => {
            let mut ai = AdvancedAi::new();
            ai.enable_live_bridge();
            Box::new(ai)
        }
        "live_target_science" => Box::new(live_targeted("science")),
        "live_target_culture" => Box::new(live_targeted("culture")),
        "live_target_religious" => Box::new(live_targeted("religious")),
        "live_target_diplomatic" => Box::new(live_targeted("diplomatic")),
        "live_target_domination" => Box::new(live_targeted("domination")),
        "live_target_score" => Box::new(live_targeted("score")),
        "random" => Box::new(RandomAi::new(seed)),
        // Named so provenance collapse checks compare controller *and*
        // weights instead of dropping the genome. (Historically also the
        // exact netless fallback the retired `neural` arm played.)
        "basic_evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(BasicAi::with_weights)
                .unwrap_or_default(),
        ),
        "evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_default(),
        ),
        "strategic" => Box::new(crate::strategic::StrategicAi::with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        "strategic_score" => Box::new(crate::strategic::StrategicAi::score_only_with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        // Public-state opponent model. The searching seat, branch set and
        // compute budget are identical to `strategic`; only confidently
        // inferred rival lanes remain fixed through a projection instead of
        // being reconstructed as blank adaptive planners.
        "strategic_rivals" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.model_rival_lanes = true;
            Box::new(ai)
        }
        // Treatment for the doctrine axis: identical to `strategic` in
        // weights, horizon, lane policy and priors, differing only in that
        // a review which reaches the rollouts also projects the four play
        // styles. Paired against `strategic` this isolates the second
        // search axis and nothing else.
        // Search-cadence doses. Everything else — weights, horizon, lane
        // policy, priors — matches `strategic`, so a pair isolates how
        // often the search runs and nothing about how well it runs.
        "strategic_r20" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            Box::new(ai)
        }
        "strategic_r10" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 10;
            Box::new(ai)
        }
        // Compute-matched cadence: twice the reviews at half the horizon,
        // so total simulated rounds per game match `strategic`. Paired
        // against it, this separates "more decisions" from "more compute".
        "strategic_r20h20" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 20;
            Box::new(ai)
        }
        // The other way to spend the doubling `strategic_r20` spends on
        // frequency: same decisions, twice the lookahead. Run on the same
        // maps, the pair asks where a marginal unit of search compute is
        // worth more.
        "strategic_h80" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.horizon = 80;
            Box::new(ai)
        }
        // The macro search with four times the compute, split across both
        // of its axes: reviews every 20 turns instead of 40, projected 80
        // rounds instead of 40. The strongest configuration measured on the
        // 4p, 24x16, Standard source benchmark — promoted on a pre-registered
        // 300-map run at a fresh seed:
        // 56 mirrored maps to 17, sign p=0.0000, e-process 3.14e4 crossing
        // at map 127, Wilson 50.8%..62.0% clearing parity — `promotion
        // gate: PASS` under the unmodified gate. With the two earlier
        // disjoint sets that is 540 independent maps, 109 to 32.
        //
        // `strategic` is deliberately unchanged: it is the frozen control
        // for further search work, the way `advanced_v1` is for
        // `advanced`, and this costs four times the macro-search compute,
        // which batch callers should adopt on purpose rather than inherit.
        "strategic_deep" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // The first strength-first budget above `strategic_deep`: preserve
        // its full 80-round horizon and spend another doubling on the
        // generation-14-favored review cadence. This is deliberately an
        // evaluator-only 8x entrant until an independent promotion gate says
        // that the extra compute buys strength.
        "strategic_ultra" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 10;
            ai.horizon = 80;
            Box::new(ai)
        }
        // Frozen control for testing whether the committed AdvancedAi
        // champion transfers through StrategicAi's 20x80 macro search. It
        // retains the same optional value-net path but deliberately refuses
        // best.json, so the genome is the only policy difference. The first
        // transfer screen favored the champion 33-27 games and 5-2 map
        // directions; retained evaluator-only for future artifact audits.
        "strategic_deep_default" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::ai::Weights::default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // Same promoted 20x80 search budget, but retain the time-to-terminal
        // signal when several deep branches all win or all lose. Outcome
        // classes remain lexicographic, so this cannot prefer an unresolved
        // score proxy over a projected win or prefer a projected loss over an
        // unresolved branch. Measured 28-32 games on 30 fresh mirrored maps;
        // retained evaluator-only because it did not earn a disjoint gate.
        "strategic_deep_tempo" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.terminal_tempo = true;
            Box::new(ai)
        }
        // Exact one- and two-action search over religious conversions before
        // the ordinary controller takes the rest of the turn. Same promoted
        // 20x80 macro budget as its control. Retained evaluator-only after it
        // lost the disjoint gate 114-126 games and religious wins fell 81-65.
        "strategic_deep_conversion" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.religious_finish_search = true;
            Box::new(ai)
        }
        // Outcome-only repair to the conversion treatment above. It searches
        // the same one- and two-action religious space but acts only when the
        // cloned result is an actual religious victory for this civilization.
        // Retained evaluator-only after two exact 30-30 screens -- fallback
        // and evolved genomes -- with all 60 map directions neutral and
        // identical victory types within each pair.
        "strategic_deep_checkmate" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.religious_checkmate_search = true;
            Box::new(ai)
        }
        // Static genome challengers for the strongest search budget measured
        // on that source benchmark. Unlike `strategic_doctrine`, these do not
        // per-review rollout to choose a play style. Each applies one bounded
        // Doctrine perturbation for the whole game, so a paired evaluation
        // measures whether that policy itself is stronger.
        "strategic_deep_expand" => {
            let weights = crate::strategic::Doctrine::Expand
                .apply(&crate::evolve::load_champion("evolved").unwrap_or_default());
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        "strategic_deep_consolidate" => {
            let weights = crate::strategic::Doctrine::Consolidate
                .apply(&crate::evolve::load_champion("evolved").unwrap_or_default());
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        "strategic_deep_militarize" => {
            let weights = crate::strategic::Doctrine::Militarize
                .apply(&crate::evolve::load_champion("evolved").unwrap_or_default());
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // Transfer test for the policy-level evolutionary system: the league
        // rates genomes on completed multiplayer outcomes. Apply its
        // conservatively strongest settled generalist to the promoted search
        // budget, falling back honestly when the committed snapshot is absent.
        "strategic_deep_league" => {
            let weights = league_generalist()
                .map(|(_, weights)| weights)
                .unwrap_or_default();
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // The same opponent-model treatment on the deepest promoted macro
        // search, isolating whether better branch fidelity still helps when
        // each review already spends the promoted 20x80 budget.
        "strategic_deep_rivals" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.model_rival_lanes = true;
            Box::new(ai)
        }
        // The frozen pre-promotion control: branches projected from a newly
        // constructed planner, which is what every `strategic` number
        // published before 2026-07-26 was measured on. Kept so those numbers
        // stay reproducible now that the promoted behaviour is the default.
        "strategic_cold" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.continue_from_plan = false;
            Box::new(ai)
        }
        // Retained as an explicit name for the promoted behaviour, which is
        // now what `strategic` already does. Kept so the pre-registered runs
        // that earned the promotion can be re-run by name.
        "strategic_warm" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.continue_from_plan = true;
            Box::new(ai)
        }
        // The promoted deep budget, spent adaptively: project every branch
        // in lockstep and stop at the first chunk where they separate,
        // rather than always running the full count. Measured WORSE than
        // its control over 120 mirrored maps -- 39.2% paired score,
        // Elo-equivalent -76, sign p=0.0000, gate RETAIN strategic_deep --
        // and kept as an entrant only so the result stays reproducible.
        //
        // There is deliberately no `strategic_adaptive` at the default
        // horizon of 40. The branches there separate by a median 0.0045,
        // under the 0.01 commitment margin, so the search never stops
        // early and the entrant would be bit-identical to its control —
        // an evaluation of it would measure nothing, which is what #380
        // cost a 240-game run to discover.
        "strategic_deep_adaptive" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.adaptive_horizon = true;
            Box::new(ai)
        }
        // The irreversible-Prophet prior removed, so the rollouts answer the
        // reviews it was short-circuiting. It answers about half of all
        // reviews and the search disagrees with it 85% of the time
        // (`search_probe --priors`), which makes it the largest single
        // restriction on this search that has ever been measured -- and an
        // entirely untested one.
        "strategic_noprophet" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.trust_religious_prior = false;
            Box::new(ai)
        }
        "strategic_rot20" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.rotate_lanes = true;
            Box::new(ai)
        }
        "strategic_rot10" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 10;
            ai.rotate_lanes = true;
            Box::new(ai)
        }
        "strategic_nodefer" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.defer_periodic_on_interrupt = false;
            Box::new(ai)
        }
        "strategic_doctrine" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.doctrine_search = true;
            Box::new(ai)
        }
        "strategic_joint" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.joint_axis_search = true;
            Box::new(ai)
        }
        "basic" => Box::new(BasicAi::new()),
        // Every `live_without_*` arm, derived from `LIVE_TREATMENTS` rather
        // than written out.
        //
        // ⚠⚠⚠ FIFTY-TWO IDENTICAL FOUR-LINE CASES STOOD HERE, AND THE
        // FIFTEEN THAT DID NOT ARE WHY THIS IS A LOOKUP NOW. Each was
        // `AdvancedAi::new()`, `enable_live_bridge()`, one `disable_*()`,
        // `Box::new(ai)` — and writing one by hand was six other edits as
        // well, so fifteen of the sixty-seven treatments the live seat ships
        // never got one. A third of the bundle could not be priced by the
        // paired evaluator and nothing said so. Deriving them means a
        // treatment reaches the bundle with its withholding arm or does not
        // compile.
        name => {
            let withheld = name
                .strip_prefix("live_without_")
                .expect("registered arm has no factory row");
            let (_, _, disable) = crate::ai::LIVE_TREATMENTS
                .iter()
                .find(|(treatment, _, _)| *treatment == withheld)
                .unwrap_or_else(|| {
                    panic!("{name} withholds {withheld}, which is not a live treatment")
                });
            let mut ai = AdvancedAi::new();
            ai.enable_live_bridge();
            disable(&mut ai);
            Box::new(ai)
        }
    }
}

/// Build a selectable arm using the production artifact tier, deliberately
/// allowing a missing trained artifact to resolve to its scripted fallback.
///
/// Game-start callers that need to continue without an artifact should use
/// this named escape hatch. Evaluators use [`builtin_ai_strict`] instead, so a
/// result cannot silently be recorded under an unavailable learned name.
pub fn builtin_ai_degraded(name: &str, seed: u64) -> Box<dyn Ai> {
    let requested = ArmKind::from_name(name).unwrap_or(ArmKind::Basic);
    build_arm(artifact_effective_alias(requested, ARTIFACT_DIR), seed)
}

/// Historical compatibility alias for [`builtin_ai_degraded`].
///
/// New callers should make fallback behavior visible by naming
/// `builtin_ai_degraded`; strict evaluator callers use [`builtin_ai_strict`].
pub fn builtin_ai(name: &str, seed: u64) -> Box<dyn Ai> {
    builtin_ai_degraded(name, seed)
}

/// Directory `builtin_ai` resolves trained artifacts from.
pub const ARTIFACT_DIR: &str = "evolved";
/// Evolved strategy genome written by `civvis evolve`.
pub const CHAMPION_FILE: &str = "best.json";
/// Distilled scalar value net written by `tools/train_valuenet.py`.
pub const VALUENET_FILE: &str = "valuenet.json";

/// One trained artifact a builtin name reads, and whether it loaded.
///
/// `definitional` separates the two ways a name depends on an artifact. A
/// definitional artifact *is* the agent: without it `builtin_ai` returns a
/// different agent under the same name. A non-definitional one only tunes
/// the agent, so its absence leaves the name honest but the numbers
/// untrained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStatus {
    pub file: &'static str,
    pub found: bool,
    pub definitional: bool,
}

/// Controller family at the evaluator boundary.  This records the agent that
/// actually receives turns after artifact aliases have resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Advanced,
    LegacyAdvanced,
    Basic,
    Random,
    Strategic,
}

/// Origin of the policy weights actually supplied to an arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightSource {
    Stock,
    Champion,
    League,
    FrozenStock,
    Doctrine(&'static str),
}

/// Terminal evaluator or selection rule that the controller actually uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluatorSource {
    Scripted,
    Random,
    ScoreShare,
    ValueNet { width: usize },
}

/// What an entrant is on every behavior-defining axis the paired evaluator
/// currently understands.  `canonical` is the typed factory target after
/// artifact resolution, so aliases share a spec by construction rather than
/// by a separately maintained display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub canonical: &'static str,
    pub architecture: Architecture,
    pub weights: WeightSource,
    pub evaluator: EvaluatorSource,
    pub treatments: &'static [&'static str],
}

impl AgentSpec {
    /// The independent axes an evaluator would change by replacing `self`
    /// with `other`.  Treatment components are compared as a set so a
    /// composite reports each mechanism rather than hiding several changes
    /// behind one experiment name.
    pub fn differing_axes(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.architecture != other.architecture {
            out.push("architecture");
        }
        if self.weights != other.weights {
            out.push("weights");
        }
        if self.evaluator != other.evaluator {
            out.push("evaluator");
        }
        for treatment in self
            .treatments
            .iter()
            .chain(other.treatments.iter())
        {
            if self.treatments.contains(treatment) != other.treatments.contains(treatment)
                && !out.contains(treatment)
            {
                out.push(treatment);
            }
        }
        // A newly added factory configuration cannot quietly be called a
        // controlled comparison merely because its semantic tags were not
        // filled in.  The canonical target is part of equality, and remains a
        // conservative final axis until an explicit treatment tag is added.
        if out.is_empty() && self.canonical != other.canonical {
            out.push("implementation");
        }
        out
    }
}

impl ArmKind {
    fn architecture(self) -> Architecture {
        match self {
            Self::Basic | Self::BasicEvolved => Architecture::Basic,
            Self::Random => Architecture::Random,
            Self::AdvancedV1 => Architecture::LegacyAdvanced,
            Self::Strategic
            | Self::StrategicCheap
            | Self::StrategicCold
            | Self::StrategicDeep
            | Self::StrategicDeepAdaptive
            | Self::StrategicDeepCheckmate
            | Self::StrategicDeepConsolidate
            | Self::StrategicDeepConversion
            | Self::StrategicDeepDefault
            | Self::StrategicDeepExpand
            | Self::StrategicDeepLeague
            | Self::StrategicDeepMilitarize
            | Self::StrategicDeepRivals
            | Self::StrategicDeepTempo
            | Self::StrategicDoctrine
            | Self::StrategicH80
            | Self::StrategicJoint
            | Self::StrategicNodefer
            | Self::StrategicNoprophet
            | Self::StrategicR10
            | Self::StrategicR20
            | Self::StrategicR20H20
            | Self::StrategicRivals
            | Self::StrategicRot10
            | Self::StrategicRot20
            | Self::StrategicScore
            | Self::StrategicUltra
            | Self::StrategicWarm => Architecture::Strategic,
            _ => Architecture::Advanced,
        }
    }

    fn weights(self, champion: bool, league: bool) -> WeightSource {
        match self {
            Self::StrategicDeepDefault => WeightSource::FrozenStock,
            Self::StrategicDeepLeague | Self::AdvancedLeagueTop if league => WeightSource::League,
            Self::StrategicDeepLeague | Self::AdvancedLeagueTop => WeightSource::Stock,
            Self::StrategicDeepExpand => WeightSource::Doctrine("expand"),
            Self::StrategicDeepConsolidate => WeightSource::Doctrine("consolidate"),
            Self::StrategicDeepMilitarize => WeightSource::Doctrine("militarize"),
            Self::AdvancedEvolved | Self::BasicEvolved => WeightSource::Champion,
            Self::Evolved => WeightSource::Champion,
            Self::Strategic
            | Self::StrategicCheap
            | Self::StrategicCold
            | Self::StrategicDeep
            | Self::StrategicDeepAdaptive
            | Self::StrategicDeepCheckmate
            | Self::StrategicDeepConversion
            | Self::StrategicDeepRivals
            | Self::StrategicDeepTempo
            | Self::StrategicDoctrine
            | Self::StrategicH80
            | Self::StrategicJoint
            | Self::StrategicNodefer
            | Self::StrategicNoprophet
            | Self::StrategicR10
            | Self::StrategicR20
            | Self::StrategicR20H20
            | Self::StrategicRivals
            | Self::StrategicRot10
            | Self::StrategicRot20
            | Self::StrategicScore
            | Self::StrategicUltra
            | Self::StrategicWarm
            | Self::AdvancedMeasuredDedication
            | Self::AdvancedEvolvedBlind => {
                if champion {
                    WeightSource::Champion
                } else {
                    WeightSource::Stock
                }
            }
            _ => WeightSource::Stock,
        }
    }

    fn evaluator(self, net: bool) -> EvaluatorSource {
        match self {
            Self::Random => EvaluatorSource::Random,
            Self::StrategicScore => EvaluatorSource::ScoreShare,
            Self::Strategic
            | Self::StrategicCheap
            | Self::StrategicCold
            | Self::StrategicDeep
            | Self::StrategicDeepAdaptive
            | Self::StrategicDeepCheckmate
            | Self::StrategicDeepConsolidate
            | Self::StrategicDeepConversion
            | Self::StrategicDeepDefault
            | Self::StrategicDeepExpand
            | Self::StrategicDeepLeague
            | Self::StrategicDeepMilitarize
            | Self::StrategicDeepRivals
            | Self::StrategicDeepTempo
            | Self::StrategicDoctrine
            | Self::StrategicH80
            | Self::StrategicJoint
            | Self::StrategicNodefer
            | Self::StrategicNoprophet
            | Self::StrategicR10
            | Self::StrategicR20
            | Self::StrategicR20H20
            | Self::StrategicRivals
            | Self::StrategicRot10
            | Self::StrategicRot20
            | Self::StrategicUltra
            | Self::StrategicWarm => {
                if net {
                    EvaluatorSource::ValueNet { width: crate::evolve::FEATURE_WIDTH }
                } else {
                    EvaluatorSource::ScoreShare
                }
            }
            _ => EvaluatorSource::Scripted,
        }
    }

    fn treatments(self) -> &'static [&'static str] {
        match self {
            Self::FogHonest => &["fog-honest"],
            // The live bridge is a COMPOSITE, and each flag is tagged so a
            // `live` vs `live_without_*` comparison reports exactly the one
            // mechanism that differs instead of the catch-all
            // "implementation" axis, which the evaluator refuses.
            Self::Live => &LIVE_BRIDGE_TREATMENTS,
            // Pinning an explicit target adds exactly its semantic lane tag;
            // the rest remains the deployed bridge. The table is shared with
            // the constructor above so an evaluator cannot label a different
            // target configuration than it builds.
            Self::LiveTargetScience => live_target_treatments("science"),
            Self::LiveTargetCulture => live_target_treatments("culture"),
            Self::LiveTargetReligious => live_target_treatments("religious"),
            Self::LiveTargetDiplomatic => live_target_treatments("diplomatic"),
            Self::LiveTargetDomination => live_target_treatments("domination"),
            Self::LiveTargetScore => live_target_treatments("score"),
            // Every `live_without_*` arm derives its list from the shared
            // bridge table below, so adding a bridge treatment cannot
            // silently miss a control's tag list. The invariant tests pin
            // each arm to bridge-minus-exactly-its-tag.
            Self::LiveWithoutAmenityProjectPreemption => {
                live_without("amenity-project-preemption")
            }
            Self::LiveWithoutAmenityDistrictPath => live_without("amenity-district-path"),
            Self::LiveWithoutGovernorEveryLane => live_without("governor-every-lane"),
            Self::LiveWithoutLiveWonderRace => live_without("live-wonder-race"),
            Self::LiveWithoutExpansionBeforeProphet => {
                live_without("expansion-before-prophet")
            }
            Self::LiveWithoutNoElectiveWar => live_without("no-elective-war"),
            Self::LiveWithoutFogLandCapacity => live_without("fog-land-capacity"),
            Self::LiveWithoutStackedEscort => live_without("stacked-escort"),
            Self::LiveWithoutJointTactics => live_without("joint-tactics"),
            Self::LiveWithoutHomeDefense => live_without("home-defense"),
            Self::LiveWithoutSolventFaithArmy => live_without("solvent-faith-army"),
            Self::LiveWithoutSiegeMuster => live_without("siege-muster"),
            Self::LiveWithoutDistrictCoverage => live_without("district-coverage"),
            Self::LiveWithoutLoyaltyRateAlarm => live_without("loyalty-rate-alarm"),
            Self::LiveWithoutBoundedRecovery => live_without("bounded-recovery"),
            Self::LiveWithoutArmyTargetWeighsEnemy => live_without("army-target-weighs-enemy"),
            Self::LiveWithoutSiegeTracksWall => live_without("siege-tracks-wall"),
            Self::LiveWithoutBlindObjectiveStrength => live_without("blind-objective-strength"),
            Self::LiveWithoutSiegeRole => live_without("siege-role"),
            Self::LiveWithoutComeAshore => live_without("come-ashore"),
            Self::LiveWithoutSuzerainCards => live_without("suzerain-cards"),
            Self::LiveWithoutReliefTargetsTheSiege => live_without("relief-targets-the-siege"),
            Self::LiveWithoutBlindObjectiveUnits => live_without("blind-objective-units"),
            Self::LiveWithoutMusterAtCommandRadius => live_without("muster-at-command-radius"),
            Self::LiveWithoutSlotKindTiebreak => live_without("slot-kind-tiebreak"),
            Self::LiveWithoutHousingDistricts => live_without("housing-districts"),
            Self::LiveWithoutCampusEveryCity => live_without("campus-every-city"),
            Self::LiveWithoutHousingCards => live_without("housing-cards"),
            Self::LiveWithoutHousingResearch => live_without("housing-research"),
            Self::LiveWithoutHousingBuildings => live_without("housing-buildings"),
            Self::LiveWithoutPeacetimeDeterrence => live_without("peacetime-deterrence"),
            Self::LiveWithoutLoyaltyPolicyDefence => live_without("loyalty-policy-defence"),
            Self::LiveWithoutWarEconomy => live_without("war-economy"),
            Self::LiveWithoutWarReinforcement => live_without("war-reinforcement"),
            Self::LiveWithoutWarPatience => live_without("war-patience"),
            Self::LiveWithoutDenyWhileTargeted => live_without("deny-while-targeted"),
            Self::LiveWithoutStockDenialLeadTime => live_without("stock-denial-lead-time"),
            Self::LiveWithoutEndgameWarRunway => live_without("endgame-war-runway"),
            Self::LiveWithoutCounterInLane => live_without("counter-in-lane"),
            Self::LiveWithoutEraPacedExpansion => live_without("era-paced-expansion"),
            Self::LiveWithoutEscortUnstick => live_without("escort-unstick"),
            Self::LiveWithoutFrontierLoyalty => live_without("frontier-loyalty"),
            Self::LiveWithoutGarrisonUnderFire => live_without("garrison-under-fire"),
            Self::LiveWithoutGarrisonWalls => live_without("garrison-walls"),
            Self::LiveWithoutNavalRecon => live_without("naval-recon"),
            Self::LiveWithoutReconFlight => live_without("recon-flight"),
            Self::LiveWithoutReconReplacement => live_without("recon-replacement"),
            Self::LiveWithoutReligionSuesPeace => live_without("religion-sues-peace"),
            Self::LiveWithoutScoreHorizon => live_without("score-horizon"),
            Self::LiveWithoutSiegeCommitment => live_without("siege-commitment"),
            Self::LiveWithoutStrandedSettlerDiscount => live_without("stranded-settler-discount"),
            Self::LiveWithoutTallyCulture => live_without("tally-culture"),
            Self::LiveWithoutWideMapCapacity => live_without("wide-map-capacity"),
            Self::LiveWithoutWonderRingSettleValue => live_without("wonder-ring-settle-value"),
            Self::LiveWithoutLiveTraderRouteAdapter => live_without("live-trader-route"),
            Self::LiveWithoutLiveReligiousPurchaseGuard => live_without("live-religious-purchase"),
            Self::LiveWithoutRecordedTacticalStep => live_without("recorded-tactical-step"),
            Self::LiveWithoutStrikeOpening => live_without("strike-opening"),
            Self::LiveWithoutRangedNeedsLineOfSight => live_without("ranged-line-of-sight"),
            Self::LiveWithoutOneLaunchPad => live_without("one-launch-pad"),
            Self::LiveWithoutCultureBuildingDebt => live_without("culture-building-debt"),
            Self::LiveWithoutCultureCoverage => live_without("culture-coverage"),
            Self::LiveWithoutSettlerTargetHysteresis => live_without("settler-target-hysteresis"),
            Self::LiveWithoutTallyGreatPeople => live_without("tally-great-people"),
            Self::LiveWithoutBarbarianScoutsAreScouts => live_without("barbarian-scouts-are-scouts"),
            Self::LiveWithoutCampReach => live_without("camp-reach"),
            Self::LiveWithoutSettlerStackDiscipline => live_without("settler-stack-discipline"),
            Self::LiveWithoutCampParty => live_without("camp-party"),
            Self::LiveWithoutBuildingsBeforeProjects => live_without("buildings-before-projects"),
            Self::LiveWithoutParallelSettlers => live_without("parallel-settlers"),
            Self::LiveWithoutHostSettlerPop => live_without("host-settler-pop"),
            Self::LiveWithoutExploreDeadTargets => live_without("explore-dead-targets"),
            Self::LiveWithoutExploreCommit => live_without("explore-commit"),
            Self::LiveWithoutBankEnvoys => live_without("bank-envoys"),
            // The native repair bundle is a COMPOSITE for the same reason
            // `live` is, and is tagged the same way: against `advanced` the
            // differing axes name all 38 repairs, and against `live` they name
            // exactly the four Firaxis-semantics flags that separate them.
            Self::AdvancedBuildFirst => &["build-order-tilt"],
            Self::AdvancedSynergy => &ENGINE_REPAIR_TREATMENTS,
            Self::AdvancedSynergyWar => &ENGINE_REPAIR_WAR_TREATMENTS,
            Self::AdvancedSynergyEconomy => &ENGINE_REPAIR_ECONOMY_TREATMENTS,
            Self::AdvancedBeliefPressure => &["belief-pressure"],
            // `advanced` now owns the confirmed Live policy plus the retained
            // priority reservation. The measured-null infrastructure arm is
            // off in production; the retained arms below remain explicit
            // reversion/decomposition controls.
            Self::AdvancedPolicyLiveControl => {
                &["envoy-priority-off"]
            }
            Self::AdvancedPolicyEnvoyPriority => &[],
            Self::AdvancedEnvoyPolicy => &[
                "envoy-influence",
                "envoy-priority-off",
            ],
            Self::AdvancedEnvoyInfrastructure => &[
                "policy-deck-legacy",
                "envoy-infrastructure-on",
                "envoy-priority-off",
            ],
            Self::AdvancedEnvoyPriority => &["policy-deck-legacy", "envoy-infrastructure-on"],
            Self::AdvancedEnvoyEconomy => &[
                "envoy-influence",
                "envoy-infrastructure-on",
                "envoy-priority-off",
            ],
            Self::AdvancedGarrisonLoyalty => &["garrison-loyalty-policy"],
            Self::AdvancedSettlerCommit => &["settler-commitment"],
            // These arms differ only in the lane they are told to win, which is
            // the axis: the deployed Civ 6 decider is handed one of these by
            // `civ6_civvis_climb.py --victory` and nothing could compare them.
            //
            // ⚠ Two of the six used to be here and four did not, so the lane the
            // deployment actually runs could be priced only if it happened to be
            // Domination or Score. `--victory` selects all six since #1871, and
            // Science — the ladder's default — was among the four with no arm.
            Self::AdvancedTargetDomination => &["victory-lane-domination"],
            Self::AdvancedTargetScore => &["victory-lane-score"],
            Self::AdvancedTargetScience => &["victory-lane-science"],
            Self::AdvancedGreatWorkVetoByDistrict => {
                &["victory-lane-science", "great-work-veto-by-district"]
            }
            Self::AdvancedTargetCulture => &["victory-lane-culture"],
            Self::AdvancedTargetCultureWithCultureBuildingDebt => {
                &["victory-lane-culture", "culture-building-debt"]
            }
            Self::AdvancedTargetReligious => &["victory-lane-religious"],
            Self::AdvancedTargetDiplomatic => &["victory-lane-diplomatic"],
            Self::AdvancedBlindToLeaders | Self::AdvancedEvolvedBlind => &["leader-denial-off"],
            Self::AdvancedRush => &["early-rush"],
            Self::AdvancedRushConnected => &["early-rush", "connected-rush"],
            Self::AdvancedTimingAttack => &["timed-war-appointment"],
            Self::AdvancedTimingAttackSelective => &["selective-timed-war-appointment"],
            Self::AdvancedTimingAttackRapid => &["rapid-timed-war-appointment"],
            Self::AdvancedCounterInLane => &["counter-in-lane"],
            Self::AdvancedCounterStandDown => &["counter-stand-down"],
            Self::AdvancedEarlyScoreAlarm => &["early-score-alarm"],
            Self::AdvancedCongressCounter => &["congress-counter-target"],
            Self::AdvancedCongressVotes => &["congress-counter-votes"],
            Self::AdvancedCongressCounterHard => {
                &["congress-counter-target", "congress-counter-votes"]
            }
            Self::AdvancedEarlyScoreBuild => &["early-score-alarm", "counter-in-lane"],
            Self::AdvancedCivBlind => &["civilization-blind"],
            Self::AdvancedCityStrategy => &["city-directives"],
            Self::AdvancedCityStrategyEmphasis => &["city-directives", "city-emphasis-only"],
            Self::AdvancedCityStrategyRoles => &["city-directives", "city-roles-only"],
            Self::AdvancedCityStrategyRolesRaw => &["city-directives", "city-roles-raw"],
            Self::AdvancedCityStrategyRaw => &["city-directives", "city-raw"],
            Self::AdvancedCityStrategyBastionOnly => &["city-directives", "city-bastion-only"],
            Self::AdvancedCityStrategyBreadbasketOnly => {
                &["city-directives", "city-breadbasket-only"]
            }
            Self::AdvancedCityStrategyComparativeOnly => {
                &["city-directives", "city-comparative-only"]
            }
            Self::AdvancedCityStrategyPressureOnly => &["city-directives", "city-pressure-only"],
            Self::AdvancedExpansionPayback => &["expansion-payback"],
            Self::AdvancedCoupledExpansion => &["coupled-expansion"],
            Self::AdvancedJointTactics => &["joint-tactics"],
            Self::AdvancedLateExpansion => &["late-expansion"],
            Self::AdvancedExpansionDispatch => &["expansion-dispatch"],
            Self::AdvancedExpansionComplete => &["late-expansion", "expansion-dispatch"],
            Self::AdvancedWarHalf => &["war-half"],
            Self::AdvancedWideOpening => &["city-target-floor"],
            Self::AdvancedPlanCityTarget => &["plan-city-target"],
            Self::AdvancedSettlerFirst => &["settler-oracle"],
            Self::AdvancedHolyPriority => &["district-holy-priority"],
            Self::AdvancedHolyLane => &["lane-holy-parity"],
            Self::AdvancedHolyV0 => &["district-holy-pre-2026-08-10"],
            Self::AdvancedSettleFood => &["settle-site-food-weight"],
            Self::AdvancedHolyLaneV0 => &["lane-holy-parity", "district-holy-pre-2026-08-10"],
            Self::AdvancedRosterLive => &["roster-winner-live-genes"],
            Self::AdvancedRosterLiveKeepDistricts => &["roster-winner-live-genes-except-districts"],
            Self::AdvancedDiplomaticOpening => &["diplomatic-lane-prospective"],
            // Historical alias: production already leaves this measured-null
            // repair off, so the arm resolves to `advanced` and has no axis.
            Self::AdvancedWithoutBoundedRecovery => &[],
            Self::AdvancedWithoutCityTargetFloor => &["city-target-floor-withheld"],
            Self::AdvancedWithoutPlanCityTarget => &["plan-city-target-withheld"],
            Self::AdvancedWithoutSettlerCommit => &["settler-commit-withheld"],
            Self::AdvancedWithoutUnpricedBundle => &["unpriced-production-bundle-withheld"],
            Self::AdvancedWithoutSettlementSafety => &["settlement-safety-withheld"],
            Self::AdvancedWithoutBattlefrontObservation => &["battlefront-observation-withheld"],
            Self::AdvancedLowerCityTarget => &["city-target-gene-lowered"],
            Self::AdvancedSettlerFoundsWhenStalled => &["settler-founds-when-stalled"],
            Self::AdvancedFortifyIdleUnits => &["fortify-idle-units"],
            Self::AdvancedOpenWaterNavy => &["open-water-navy"],
            Self::AdvancedMaritimeSplice => &["naval-production-card-spliced"],
            Self::AdvancedSeaAnswers => &["sea-answers-sea-threats"],
            Self::AdvancedWithoutBarbarianScoutExemption => &["barbarian-scout-exemption-withheld"],
            Self::AdvancedEngineFaithPrice => &["engine-faith-price"],
            Self::AdvancedMaintenanceDeck => &["maintenance-aware-deck"],
            Self::AdvancedReconFleet => &[
                "recon-replacement",
                "recon-flight",
                "naval-recon",
                "come-ashore",
            ],
            Self::AdvancedWithoutReconFleet => &["recon-fleet-withheld"],
            Self::AdvancedEveryLane => &["governor-under-every-lane"],
            Self::AdvancedBuilderSurvey => &["builder-priced-by-survey"],
            Self::AdvancedUnitEfficiency => &["unit-strength-per-cost"],
            Self::AdvancedWithoutUnpricedEconomy => &["unpriced-economy-half-withheld"],
            Self::AdvancedWithoutUnpricedWar => &["unpriced-war-half-withheld"],
            Self::AdvancedWithoutCityDefence => &["city-defence-quarter-withheld"],
            Self::AdvancedLegacyPolicyDeck => &["live-policy-deck-withheld"],
            Self::AdvancedWithoutBuilderFloor => &["production-builder-floor-withheld"],
            Self::AdvancedWithoutSettlerDeadline => &["production-settler-deadline-withheld"],
            Self::AdvancedWithoutHutCollection => &["hut-collection-withheld"],
            Self::AdvancedWithoutExploreCommit => &["explore-commit-withheld"],
            Self::AdvancedWithoutVillageSeeking => &["village-seeking-withheld"],
            Self::AdvancedPriceSuzerainty => &["suzerainty-priced-into-envoy-placement"],
            Self::AdvancedWithoutUnitTactics => &["unit-tactics-quarter-withheld"],
            Self::AdvancedMeasuredDedication => &["dedication-measured"],
            Self::StrategicCheap => &["search-cheap"],
            Self::StrategicCold => &["search-cold"],
            Self::StrategicDeep => &["search-cadence-20", "search-horizon-80"],
            Self::StrategicDeepAdaptive => {
                &["search-cadence-20", "search-horizon-80", "search-adaptive-horizon"]
            }
            Self::StrategicDeepCheckmate => {
                &["search-cadence-20", "search-horizon-80", "religious-checkmate-search"]
            }
            Self::StrategicDeepConversion => {
                &["search-cadence-20", "search-horizon-80", "religious-conversion-search"]
            }
            Self::StrategicDeepDefault => &["search-cadence-20", "search-horizon-80"],
            Self::StrategicDeepExpand => {
                &["search-cadence-20", "search-horizon-80", "doctrine-expand"]
            }
            Self::StrategicDeepConsolidate => {
                &["search-cadence-20", "search-horizon-80", "doctrine-consolidate"]
            }
            Self::StrategicDeepMilitarize => {
                &["search-cadence-20", "search-horizon-80", "doctrine-militarize"]
            }
            Self::StrategicDeepLeague => &["search-cadence-20", "search-horizon-80"],
            Self::StrategicDeepRivals => {
                &["search-cadence-20", "search-horizon-80", "rival-lane-model"]
            }
            Self::StrategicDeepTempo => {
                &["search-cadence-20", "search-horizon-80", "terminal-tempo"]
            }
            Self::StrategicDoctrine => &["doctrine-search"],
            Self::StrategicH80 => &["search-horizon-80"],
            Self::StrategicJoint => &["joint-axis-search"],
            Self::StrategicNodefer => &["search-no-defer"],
            Self::StrategicNoprophet => &["religious-prior-off"],
            Self::StrategicR10 => &["search-cadence-10"],
            Self::StrategicR20 => &["search-cadence-20"],
            Self::StrategicR20H20 => &["search-cadence-20", "search-horizon-20"],
            Self::StrategicRivals => &["rival-lane-model"],
            Self::StrategicRot10 => &["search-cadence-10", "rotate-lanes"],
            Self::StrategicRot20 => &["search-cadence-20", "rotate-lanes"],
            Self::StrategicUltra => &["search-cadence-10", "search-horizon-80"],
            Self::AdvancedV1 => &["legacy-advanced"],
            _ => &[],
        }
    }

    fn spec(self, dir: &str) -> AgentSpec {
        let champion = crate::evolve::load_champion(dir).is_some();
        let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH)
            .is_some();
        let league = match self {
            Self::StrategicDeepLeague => league_generalist().is_some(),
            Self::AdvancedLeagueTop => shipped_league_top_advanced().is_some(),
            _ => false,
        };
        AgentSpec {
            canonical: self.name(),
            architecture: self.architecture(),
            weights: self.weights(champion, league),
            evaluator: self.evaluator(net),
            treatments: self.treatments(),
        }
    }
}

/// What a builtin name actually plays as once its artifacts are resolved.
///
/// `builtin_ai` falls back silently when a trained artifact is missing —
/// correctly, because a missing file should not stop a game. What it must
/// not do is let an evaluation record the result under the learned name: on
/// a checkout with no value net, `neural` is either `basic_evolved` or
/// `basic`, and `policy` is either `advanced_evolved` or `advanced`, depending
/// on whether the champion genome loaded. Dropping that second artifact from
/// the effective name makes the self-comparison guard wrong in both
/// directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProvenance {
    /// The name the caller asked for.
    pub requested: String,
    /// Every artifact the name reads, in the order it reads them.
    pub artifacts: Vec<ArtifactStatus>,
    /// Canonical identity of the agent that actually plays. Equals
    /// `requested` unless a definitional artifact is missing or the requested
    /// name is a historical alias of an identical controller and weight set.
    pub effective: &'static str,
}

impl AgentProvenance {
    /// True when the name promises more than the loaded artifacts deliver.
    pub fn degraded(&self) -> bool {
        self.artifacts
            .iter()
            .any(|artifact| artifact.definitional && !artifact.found)
    }

    /// True when some artifact the name reads did not load, whether or not
    /// that changed which agent plays.
    pub fn untrained(&self) -> bool {
        self.artifacts.iter().any(|artifact| !artifact.found)
    }

    pub fn missing(&self) -> Vec<&'static str> {
        self.artifacts
            .iter()
            .filter(|artifact| !artifact.found)
            .map(|artifact| artifact.file)
            .collect()
    }

    /// One reportable line, e.g.
    /// `neural: plays as basic (missing valuenet.json, best.json)`.
    pub fn line(&self) -> String {
        let missing = self.missing();
        if missing.is_empty() {
            return if self.artifacts.is_empty() {
                if self.effective == self.requested {
                    format!("{}: scripted, no artifacts required", self.requested)
                } else {
                    format!(
                        "{}: plays as {} (scripted, no artifacts required)",
                        self.requested, self.effective
                    )
                }
            } else if self.effective != self.requested {
                format!(
                    "{}: plays as {} (loaded {})",
                    self.requested,
                    self.effective,
                    self.artifacts_list()
                )
            } else {
                format!("{}: loaded {}", self.requested, self.artifacts_list())
            };
        }
        let plays = match self.degraded() {
            true => format!("plays as {}", self.effective),
            false => format!("plays as {} with untrained defaults", self.requested),
        };
        format!("{}: {} (missing {})", self.requested, plays, missing.join(", "))
    }

    fn artifacts_list(&self) -> String {
        self.artifacts
            .iter()
            .map(|artifact| artifact.file)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Why strict builtin construction refused a requested evaluator arm.
///
/// An artifact can be absent without changing the controller (for example, an
/// optional tuning net). Only [`Self::Degraded`] rejects construction: it
/// means a definitional artifact is absent and the requested name would play
/// as a different controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinAiBuildError {
    /// No selectable builtin or evaluator-only arm has this name.
    UnknownName { requested: String },
    /// A definitional artifact did not load, so construction would substitute
    /// the effective scripted fallback recorded in this provenance.
    Degraded { provenance: AgentProvenance },
}

impl BuiltinAiBuildError {
    /// The name the caller attempted to construct.
    pub fn requested(&self) -> &str {
        match self {
            Self::UnknownName { requested } => requested,
            Self::Degraded { provenance } => &provenance.requested,
        }
    }

    /// Detailed artifact resolution when a known arm degraded.
    pub fn provenance(&self) -> Option<&AgentProvenance> {
        match self {
            Self::UnknownName { .. } => None,
            Self::Degraded { provenance } => Some(provenance),
        }
    }
}

impl std::fmt::Display for BuiltinAiBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownName { requested } => write!(f, "unknown builtin AI {requested:?}"),
            Self::Degraded { provenance } => write!(
                f,
                "{} is unavailable and would play as {} (missing {})",
                provenance.requested,
                provenance.effective,
                provenance.missing().join(", ")
            ),
        }
    }
}

impl std::error::Error for BuiltinAiBuildError {}

/// A resolved evaluator arm. The private `kind` is the exact canonical target
/// sent to the factory; public consumers can compare `spec` without relying on
/// a display alias or a second factory match.
#[derive(Debug, Clone)]
pub struct BuiltinArm {
    pub spec: AgentSpec,
    kind: ArmKind,
}

impl BuiltinArm {
    pub fn build(&self, seed: u64) -> Box<dyn Ai> {
        build_arm(self.kind, seed)
    }
}

fn builtin_arm_in(name: &str, dir: &str) -> Option<BuiltinArm> {
    let requested = ArmKind::from_name(name)?;
    let effective = artifact_effective_alias(requested, dir);
    Some(BuiltinArm {
        spec: effective.spec(dir),
        kind: effective,
    })
}

/// Resolve one selectable arm at the production artifact tier. Returning
/// `None` for an unknown name makes strict evaluator callers fail closed while
/// [`builtin_ai`] retains the explicit game-start fallback for legacy callers.
pub fn builtin_arm(name: &str) -> Option<BuiltinArm> {
    builtin_arm_in(name, ARTIFACT_DIR)
}

/// Resolve a known arm only when its definitional artifacts loaded.
///
/// Keeping the resolver separate makes the invariant testable against a
/// fixture directory; public construction below always uses the production
/// artifact tier that the factories actually read.
fn strict_builtin_arm_in(name: &str, dir: &str) -> Result<BuiltinArm, BuiltinAiBuildError> {
    let arm = builtin_arm_in(name, dir).ok_or_else(|| BuiltinAiBuildError::UnknownName {
        requested: name.to_string(),
    })?;
    let provenance = builtin_provenance(name, dir);
    debug_assert_eq!(
        arm.spec.canonical, provenance.effective,
        "strict arm and provenance disagree for {name}"
    );
    if provenance.degraded() {
        return Err(BuiltinAiBuildError::Degraded { provenance });
    }
    Ok(arm)
}

/// Build a selectable arm only when the requested identity is available.
///
/// This is the fail-closed evaluator boundary. It rejects unknown names and
/// names whose missing definitional artifact would silently substitute a
/// different agent. Call [`builtin_ai_degraded`] only when continuing with
/// that substitution is an intentional, reportable choice.
pub fn builtin_ai_strict(name: &str, seed: u64) -> Result<Box<dyn Ai>, BuiltinAiBuildError> {
    Ok(strict_builtin_arm_in(name, ARTIFACT_DIR)?.build(seed))
}

/// Resolve what `builtin_ai(name, _)` will actually construct from `dir`.
///
/// Presence is decided by the same loaders the agents use, not by a stat:
/// a `valuenet.json` that fails `ValueNet::valid` is rejected at load time,
/// so reporting it as present would restate the bug it is meant to catch.
pub fn builtin_provenance(name: &str, dir: &str) -> AgentProvenance {
    let kind = ArmKind::from_name(name).unwrap_or(ArmKind::Basic);
    let champion = crate::evolve::load_champion(dir).is_some();
    let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
    let basic_fallback = if champion { "basic_evolved" } else { "basic" };
    let advanced_fallback = if champion {
        "advanced_evolved"
    } else {
        "advanced"
    };
    let genome = ArtifactStatus {
        file: CHAMPION_FILE,
        found: champion,
        definitional: false,
    };
    let value = |definitional| ArtifactStatus {
        file: VALUENET_FILE,
        found: net,
        definitional,
    };
    let league = league_generalist().is_some();
    let (artifacts, independently_declared_effective) = match name {
        // The genome *is* these two names; without it they are the stock
        // scripted agent under a name that claims otherwise.
        "evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                // `evolved` and `advanced_evolved` construct the same
                // `AdvancedAi::with_weights(champion)`; retain one canonical
                // identity so their comparison is rejected as self-play.
                "advanced_evolved"
            } else {
                "advanced"
            },
        ),
        "advanced_evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                "advanced_evolved"
            } else {
                "advanced"
            },
        ),
        "basic_evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            basic_fallback,
        ),
        // Strategic keeps its lane rollouts without a net; what it loses is
        // the learned terminal evaluator, which is exactly the published
        // `strategic_score` control.
        "strategic" => (
            vec![genome, value(true)],
            if net { "strategic" } else { "strategic_score" },
        ),
        // The control refuses a net by construction, so it is never
        // degraded — only untrained when the genome is absent.
        "strategic_score" => (vec![genome], "strategic_score"),
        "strategic_rivals" => (vec![genome, value(false)], "strategic_rivals"),
        // Unlike `strategic`, its netless form has no separate published
        // name to degrade *to*: the doctrine axis runs either way. A
        // missing net therefore leaves it untrained rather than renamed,
        // which the provenance line says in those words.
        "strategic_doctrine" => (vec![genome, value(false)], "strategic_doctrine"),
        "strategic_joint" => (vec![genome, value(false)], "strategic_joint"),
        "strategic_r20" => (vec![genome, value(false)], "strategic_r20"),
        "strategic_r10" => (vec![genome, value(false)], "strategic_r10"),
        "strategic_nodefer" => (vec![genome, value(false)], "strategic_nodefer"),
        "strategic_r20h20" => (vec![genome, value(false)], "strategic_r20h20"),
        "strategic_h80" => (vec![genome, value(false)], "strategic_h80"),
        "strategic_rot20" => (vec![genome, value(false)], "strategic_rot20"),
        "strategic_warm" => (
            vec![genome, value(true)],
            if net { "strategic" } else { "strategic_score" },
        ),
        "strategic_cold" => (vec![genome, value(false)], "strategic_cold"),
        "strategic_noprophet" => (vec![genome, value(false)], "strategic_noprophet"),
        "strategic_deep_adaptive" => (vec![genome, value(false)], "strategic_deep_adaptive"),
        // Same artifact dependencies as `strategic`: the genome tunes it,
        // and the net is non-definitional because the search runs without
        // one. There is no separate published netless name to degrade to.
        "strategic_deep" => (vec![genome, value(false)], "strategic_deep"),
        "strategic_ultra" => (vec![genome, value(false)], "strategic_ultra"),
        // The frozen genome is in code; only the same optional value net read
        // by `strategic_deep` remains in its provenance.
        "strategic_deep_default" => (vec![value(false)], "strategic_deep_default"),
        "strategic_deep_tempo" => (
            vec![genome, value(false)],
            "strategic_deep_tempo",
        ),
        "strategic_deep_conversion" => (
            vec![genome, value(false)],
            "strategic_deep_conversion",
        ),
        "strategic_deep_checkmate" => (
            vec![genome, value(false)],
            "strategic_deep_checkmate",
        ),
        "strategic_deep_expand" => (vec![genome, value(false)], "strategic_deep_expand"),
        "strategic_deep_consolidate" => (vec![genome, value(false)], "strategic_deep_consolidate"),
        "strategic_deep_militarize" => (vec![genome, value(false)], "strategic_deep_militarize"),
        "strategic_deep_league" => (
            vec![ArtifactStatus {
                file: LEAGUE_SNAPSHOT_FILE,
                found: league,
                definitional: true,
            }],
            if league {
                "strategic_deep_league"
            } else {
                "strategic_deep"
            },
        ),
        "strategic_deep_rivals" => (vec![genome, value(false)], "strategic_deep_rivals"),
        "strategic_rot10" => (vec![genome, value(false)], "strategic_rot10"),
        // The genome tunes both its rollout policy and its scripted
        // governor; it consults no net.
        // The bridge arms are scripted composites: no genome, no value net, and
        // each is effective as ITSELF. Without an explicit row they inherit the
        // catch-all and the evaluator reports them as plain `basic`, which would
        // silently compare two identical agents.
        "live" => (Vec::new(), "live"),
        "live_target_science" => (Vec::new(), "live_target_science"),
        "live_target_culture" => (Vec::new(), "live_target_culture"),
        "live_target_religious" => (Vec::new(), "live_target_religious"),
        "live_target_diplomatic" => (Vec::new(), "live_target_diplomatic"),
        "live_target_domination" => (Vec::new(), "live_target_domination"),
        "live_target_score" => (Vec::new(), "live_target_score"),
        "live_without_amenity_project_preemption" => {
            (Vec::new(), "live_without_amenity_project_preemption")
        }
        "live_without_amenity_district_path" => {
            (Vec::new(), "live_without_amenity_district_path")
        }
        "live_without_expansion_before_prophet" => {
            (Vec::new(), "live_without_expansion_before_prophet")
        }
        "live_without_loyalty_policy_defence" => {
            (Vec::new(), "live_without_loyalty_policy_defence")
        }
        "advanced" => (Vec::new(), "advanced"),
        "fog_honest" => (Vec::new(), "fog_honest"),
        "advanced_build_first" => (Vec::new(), "advanced_build_first"),
        "advanced_synergy" => (Vec::new(), "advanced_synergy"),
        "advanced_synergy_war" => (Vec::new(), "advanced_synergy_war"),
        "advanced_synergy_economy" => (Vec::new(), "advanced_synergy_economy"),
        "advanced_belief_pressure" => (Vec::new(), "advanced_belief_pressure"),
        "advanced_policy_live_control" => (Vec::new(), "advanced_policy_live_control"),
        "advanced_policy_envoy_priority" => (Vec::new(), "advanced"),
        "advanced_envoy_policy" => (Vec::new(), "advanced_envoy_policy"),
        "advanced_envoy_infrastructure" => (Vec::new(), "advanced_envoy_infrastructure"),
        "advanced_envoy_priority" => (Vec::new(), "advanced_envoy_priority"),
        "advanced_envoy_economy" => (Vec::new(), "advanced_envoy_economy"),
        "advanced_wide_opening" => (Vec::new(), "advanced_wide_opening"),
        "advanced_plan_city_target" => (Vec::new(), "advanced"),
        "advanced_expansion_payback" => (Vec::new(), "advanced_expansion_payback"),
        "advanced_late_expansion" => (Vec::new(), "advanced_late_expansion"),
        "advanced_expansion_dispatch" => (Vec::new(), "advanced_expansion_dispatch"),
        "advanced_expansion_complete" => (Vec::new(), "advanced_expansion_complete"),
        "advanced_coupled_expansion" => (Vec::new(), "advanced_coupled_expansion"),
        "advanced_city_strategy" => (Vec::new(), "advanced_city_strategy"),
        "advanced_city_strategy_emphasis" => (Vec::new(), "advanced_city_strategy_emphasis"),
        "advanced_city_strategy_roles" => (Vec::new(), "advanced_city_strategy_roles"),
        "advanced_city_strategy_roles_raw" => (Vec::new(), "advanced_city_strategy_roles_raw"),
        "advanced_city_strategy_raw" => (Vec::new(), "advanced_city_strategy_raw"),
        "advanced_city_strategy_bastion_only" => (Vec::new(), "advanced_city_strategy_bastion_only"),
        "advanced_city_strategy_breadbasket_only" => (Vec::new(), "advanced_city_strategy_breadbasket_only"),
        "advanced_city_strategy_comparative_only" => (Vec::new(), "advanced_city_strategy_comparative_only"),
        "advanced_city_strategy_pressure_only" => (Vec::new(), "advanced_city_strategy_pressure_only"),
        "advanced_banking_dedication" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            advanced_fallback,
        ),
        "advanced_measured_dedication" => (vec![genome], "advanced_measured_dedication"),
        "advanced_settler_first" => (Vec::new(), "advanced_settler_first"),
        "advanced_holy_priority" => (Vec::new(), "advanced_holy_priority"),
        "advanced_holy_lane" => (Vec::new(), "advanced_holy_lane"),
        "advanced_holy_v0" => (Vec::new(), "advanced"),
        "advanced_settle_food" => (Vec::new(), "advanced_settle_food"),
        "advanced_holy_lane_v0" => (Vec::new(), "advanced_holy_lane_v0"),
        "advanced_roster_live" => (Vec::new(), "advanced_roster_live"),
        "advanced_roster_live_keep_districts" => (Vec::new(), "advanced_roster_live_keep_districts"),
        "advanced_diplomatic_opening" => (Vec::new(), "advanced_diplomatic_opening"),
        // Historical withhold alias: the repair is already off production,
        // so this name now resolves to the stock controller and fails closed.
        "advanced_without_bounded_recovery" => (Vec::new(), "advanced"),
        "advanced_without_city_target_floor" => (Vec::new(), "advanced"),
        "advanced_without_plan_city_target" => (Vec::new(), "advanced_without_plan_city_target"),
        "advanced_without_settler_commit" => (Vec::new(), "advanced_without_settler_commit"),
        "advanced_without_unpriced_bundle" => (Vec::new(), "advanced_without_unpriced_bundle"),
        "advanced_without_settlement_safety" => (Vec::new(), "advanced_without_settlement_safety"),
        "advanced_without_battlefront_observation" => (Vec::new(), "advanced_without_battlefront_observation"),
        "advanced_lower_city_target" => (Vec::new(), "advanced_lower_city_target"),
        "advanced_settler_founds_when_stalled" => (Vec::new(), "advanced_settler_founds_when_stalled"),
        "advanced_fortify_idle_units" => (Vec::new(), "advanced_fortify_idle_units"),
        "advanced_open_water_navy" => (Vec::new(), "advanced_open_water_navy"),
        "advanced_maritime_splice" => (Vec::new(), "advanced_maritime_splice"),
        "advanced_sea_answers" => (Vec::new(), "advanced_sea_answers"),
        "advanced_without_barbarian_scouts_are_scouts" => {
            (Vec::new(), "advanced_without_barbarian_scouts_are_scouts")
        }
        "advanced_engine_faith_price" => (Vec::new(), "advanced_engine_faith_price"),
        "advanced_maintenance_deck" => (Vec::new(), "advanced_maintenance_deck"),
        "advanced_recon_fleet" => (Vec::new(), "advanced"),
        "advanced_without_recon_fleet" => (Vec::new(), "advanced_without_recon_fleet"),
        "advanced_every_lane" => (Vec::new(), "advanced_every_lane"),
        "advanced_builder_survey" => (Vec::new(), "advanced_builder_survey"),
        "advanced_unit_efficiency" => (Vec::new(), "advanced_unit_efficiency"),
        "advanced_without_unpriced_economy" => (Vec::new(), "advanced_without_unpriced_economy"),
        // Aliases since the 2026-08-14 war-half removal: the flags these
        // withheld no longer ship, so the arms construct the control.
        "advanced_without_unpriced_war" => (Vec::new(), "advanced"),
        "advanced_without_city_defence" => (Vec::new(), "advanced"),
        "advanced_legacy_policy_deck" => (Vec::new(), "advanced_legacy_policy_deck"),
        "advanced_without_builder_floor" => (Vec::new(), "advanced_without_builder_floor"),
        "advanced_without_hut_collection" => (Vec::new(), "advanced_without_hut_collection"),
        "advanced_without_explore_commit" => (Vec::new(), "advanced_without_explore_commit"),
        "advanced_without_village_seeking" => (Vec::new(), "advanced_without_village_seeking"),
        "advanced_without_settler_deadline" => (Vec::new(), "advanced_without_settler_deadline"),
        "advanced_price_suzerainty" => (Vec::new(), "advanced_price_suzerainty"),
        "advanced_without_unit_tactics" => (Vec::new(), "advanced"),
        "advanced_war_half" => (Vec::new(), "advanced_war_half"),
        "advanced_joint_tactics" => (Vec::new(), "advanced_joint_tactics"),
        "advanced_league_top" => (Vec::new(), "advanced_league_top"),
        "strategic_cheap" => (vec![genome, value(false)], "strategic_cheap"),
        "advanced_blind_to_leaders" => (Vec::new(), "advanced_blind_to_leaders"),
        "advanced_counter_in_lane" => (Vec::new(), "advanced_counter_in_lane"),
        "advanced_congress_counter" => (Vec::new(), "advanced_congress_counter"),
        "advanced_congress_votes" => (Vec::new(), "advanced_congress_votes"),
        "advanced_congress_counter_hard" => (Vec::new(), "advanced_congress_counter_hard"),
        "advanced_counter_stand_down" => (Vec::new(), "advanced_counter_stand_down"),
        "advanced_early_score_alarm" => (Vec::new(), "advanced_early_score_alarm"),
        "advanced_early_score_build" => (Vec::new(), "advanced_early_score_build"),
        // The genome is definitional here for the same reason it is for
        // `advanced_evolved`: without it this is the stock agent with the
        // denial layer off, which is a different measurement entirely.
        "advanced_evolved_blind" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                "advanced_evolved_blind"
            } else {
                "advanced_blind_to_leaders"
            },
        ),
        "advanced_civ_blind" => (Vec::new(), "advanced_civ_blind"),
        "advanced_rush" => (Vec::new(), "advanced_rush"),
        "advanced_rush_connected" => (Vec::new(), "advanced_rush_connected"),
        "advanced_garrison_loyalty" => (Vec::new(), "advanced_garrison_loyalty"),
        "advanced_timing_attack" => (Vec::new(), "advanced_timing_attack"),
        "advanced_timing_attack_selective" => {
            (Vec::new(), "advanced_timing_attack_selective")
        }
        "advanced_timing_attack_rapid" => (Vec::new(), "advanced_timing_attack_rapid"),
        "advanced_settler_commit" => (Vec::new(), "advanced_settler_commit"),
        "advanced_target_domination" => (Vec::new(), "advanced_target_domination"),
        "advanced_target_score" => (Vec::new(), "advanced_target_score"),
        "advanced_target_science" => (Vec::new(), "advanced_target_science"),
        "advanced_great_work_veto_by_district" => {
            (Vec::new(), "advanced_great_work_veto_by_district")
        }
        "advanced_target_culture" => (Vec::new(), "advanced_target_culture"),
        "advanced_target_culture_with_culture_building_debt" => (
            Vec::new(),
            "advanced_target_culture_with_culture_building_debt",
        ),
        "advanced_target_religious" => (Vec::new(), "advanced_target_religious"),
        "advanced_target_diplomatic" => (Vec::new(), "advanced_target_diplomatic"),
        "advanced_v1" => (Vec::new(), "advanced_v1"),
        "random" => (Vec::new(), "random"),
        // `builtin_ai` answers every other name with the lightweight agent.
        "basic" => (Vec::new(), "basic"),
        // Every `live_without_*` arm is itself: it builds the live bundle with
        // one treatment withheld, which is not an alias of anything.
        //
        // ⚠ THIS ROW USED TO BE FIFTY-TWO ROWS AND THE DEFAULT BELOW WAS THE
        // TRAP. A registered arm with no row here fell to `"basic"`, so the
        // provenance said "this is the lightweight agent" about a live-bundle
        // controller while the alias table said it was itself. The
        // `debug_assert` under this match is the only thing that catches it —
        // and `release` compiles debug assertions out, which is why the `ci`
        // profile turns them back on (see Cargo.toml). Derived, the two
        // cannot disagree.
        live_without if live_without.starts_with("live_without_") => {
            (Vec::new(), live_without)
        }
        _ => (Vec::new(), "basic"),
    };
    let effective = artifact_effective_alias_from(kind, champion, net, league).name();
    debug_assert_eq!(
        effective, independently_declared_effective,
        "artifact alias table and provenance row diverged for {name}"
    );
    AgentProvenance {
        requested: name.to_string(),
        artifacts,
        effective,
    }
}

/// Provenance for a whole entrant list, in the order given.
pub fn builtin_provenances(names: &[&str], dir: &str) -> Vec<AgentProvenance> {
    names
        .iter()
        .map(|name| builtin_provenance(name, dir))
        .collect()
}

/// Distinct requested names that resolve to the same agent, which makes any
/// difference between them noise. Returns `(first, second, shared agent)`.
pub fn collapsed_entrants(names: &[&str], dir: &str) -> Vec<(String, String, &'static str)> {
    let resolved = names
        .iter()
        .filter_map(|name| builtin_arm_in(name, dir).map(|arm| (*name, arm)))
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for (index, left) in resolved.iter().enumerate() {
        for right in resolved.iter().skip(index + 1) {
            if left.0 != right.0 && left.1.spec == right.1.spec {
                out.push((
                    left.0.to_string(),
                    right.0.to_string(),
                    left.1.spec.canonical,
                ));
            }
        }
    }
    out
}

pub struct TourneyCfg {
    pub games: u32,
    pub players_per_game: usize,
    pub width: i32,
    pub height: i32,
    pub speed: String,
    pub map_script: MapScript,
    pub map_topology: MapTopology,
    pub map_poles: MapPoles,
    pub max_turns: u32,
    pub num_city_states: usize,
    /// Which era each game opens in. Part of the experiment, so the profile
    /// records it and a fixed-era ladder can never absorb a random-era one.
    pub start_era: crate::setup::StartEraChoice,
    /// What a Tactics arena grants its two sides. Ignored on a world, and
    /// recorded in the profile only for an arena, so a Civ ledger written
    /// before the arena had an economy still matches.
    pub tactics: crate::setup::TacticsRules,
    pub seed: u64,
    pub k: f64,
    /// Immutable player identity that pins the longitudinal rating scale.
    /// `None` leaves an in-memory or one-off pool floating around its base.
    pub rating_anchor: Option<String>,
    /// Ordered controller roles behind the versioned rating identities.
    /// Persistent CLI tournaments require one role per entrant; in-memory
    /// library experiments may leave this empty.
    pub controller_roster: Vec<String>,
    pub verbose: bool,
    /// How many games to play at once. Results and rating checkpoints remain
    /// in game order, so concurrency does not change the final table.
    pub jobs: usize,
}

impl Default for TourneyCfg {
    fn default() -> Self {
        let size = MapSize::for_players(4);
        let speed = default_speed();
        let max_turns = Rules::embedded()
            .speeds
            .get(&speed)
            .map_or(500, |spec| spec.turns);
        TourneyCfg {
            games: 20,
            players_per_game: 4,
            width: size.width,
            height: size.height,
            speed,
            map_script: MapScript::default(),
            map_topology: MapTopology::default(),
            map_poles: MapPoles::default(),
            max_turns,
            num_city_states: size.default_city_states,
            // The stock ladder is the Ancient-era one every existing ledger
            // was rated on; a sweep asks for anything else explicitly.
            start_era: crate::setup::StartEraChoice::Fixed(0),
            tactics: crate::setup::TacticsRules::default(),
            seed: 0,
            k: 24.0,
            rating_anchor: None,
            controller_roster: Vec::new(),
            verbose: true,
            jobs: crate::parallel::default_jobs(),
        }
    }
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Build a seeded round-robin order. The stride is coprime with the entrant
/// count, so every fixed civilization seat sees every entrant exactly once in
/// each complete cycle. When there are no more entrants than seats, every game
/// also contains every entrant at least once.
fn seat_schedule(names: &[String], players: usize, rng: &mut Rng) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..names.len()).collect();
    for index in (1..order.len()).rev() {
        let other = rng.below(index + 1);
        order.swap(index, other);
    }
    let mut stride = players % names.len();
    if stride == 0 {
        stride = 1;
    }
    while gcd(stride, names.len()) != 1 {
        stride = stride % names.len() + 1;
    }
    (order, stride)
}

fn scheduled_seats(
    names: &[String],
    players: usize,
    game: u32,
    order: &[usize],
    stride: usize,
) -> Vec<String> {
    (0..players)
        .map(|seat| {
            let scheduled = (game as usize * stride + seat) % names.len();
            names[order[scheduled]].clone()
        })
        .collect()
}

fn play_tournament<F, C, E>(
    names: &[String],
    make: &F,
    cfg: &TourneyCfg,
    mut checkpoint: C,
) -> Result<(), E>
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
    C: FnMut(u32, u64, &[RatedPlayer]) -> Result<(), E>,
{
    assert!(!names.is_empty(), "no entrants");
    assert!(cfg.players_per_game >= 2, "Elo needs at least two players");
    let mut rng = Rng::new(cfg.seed.wrapping_add(0x5EED));
    let (entrant_order, entrant_stride) = seat_schedule(names, cfg.players_per_game, &mut rng);
    let draws: Vec<(u64, Vec<String>)> = (0..cfg.games)
        .map(|game| {
            (
                cfg.seed.wrapping_mul(100_000).wrapping_add(game as u64),
                scheduled_seats(
                    names,
                    cfg.players_per_game,
                    game,
                    &entrant_order,
                    entrant_stride,
                ),
            )
        })
        .collect();

    // Games are independent and expensive, while rating mutation and
    // persistence remain serialized below in deterministic game order.
    let played = crate::parallel::map(draws.len(), cfg.jobs, |game_index| {
        let (gseed, seats) = &draws[game_index];
        let mut options = GameOptions::new(
            cfg.players_per_game,
            cfg.width,
            cfg.height,
            *gseed,
            cfg.max_turns,
            cfg.num_city_states,
        );
        options.speed = cfg.speed.clone();
        options.map_script = cfg.map_script;
        options.map_topology = cfg.map_topology;
        options.map_poles = cfg.map_poles;
        // Rolled from this game's own seed, so the draw and the era it is
        // fought in replay together.
        options.start_era = cfg.start_era.for_seed(*gseed);
        options.tactics = cfg.tactics;
        let mut game = Game::new_with(options);
        let mut ais: Vec<Box<dyn Ai>> = game
            .players
            .iter()
            .map(|player| {
                if player.id < cfg.players_per_game {
                    make(&seats[player.id], gseed.wrapping_add(player.id as u64))
                } else {
                    builtin_ai("basic", gseed.wrapping_add(player.id as u64))
                }
            })
            .collect();
        // Until the game is *finished*, not until it has a winner: a Tactics
        // battle that reaches its clock with both armies standing is a
        // terminal draw with `winner: None`, and a loop keyed on the winner
        // would play the drawn arena forever — measured as four hung workers
        // the first time the stock arena stopped granting reinforcements and
        // draws became ordinary. A world always ends with a winner (the clock
        // awards its score tiebreak), so this is the same loop for a world.
        while !game.is_finished() {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if !game.is_finished() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }

        // A game nobody won is a game nobody won: every seat is rated as a
        // non-winner, and the ratings fall back to the score ordering they
        // already carry. A drawn arena reaches this every time it happens; a
        // world reaches it only when its lobby switched the score victory
        // off. Either way it must not take the rating run down with it.
        let winner = game.winner;
        let results: Vec<RatedPlayer> = (0..cfg.players_per_game)
            .map(|pid| {
                let civilization = game.players[pid].civ.clone();
                let leader = game
                    .rules
                    .civs
                    .get(&civilization)
                    .map(|spec| spec.leader.clone())
                    .unwrap_or_else(|| civilization.clone());
                RatedPlayer::new(
                    seats[pid].clone(),
                    leader,
                    civilization,
                    game.score(pid),
                    winner == Some(pid),
                )
            })
            .collect();
        let wname = match winner {
            Some(winner) if winner < cfg.players_per_game => seats[winner].clone(),
            Some(winner) => game.players[winner].civ.clone(),
            None => "-".to_string(),
        };
        (
            *gseed,
            results,
            wname,
            winner.map_or_else(
                || "-".to_string(),
                |winner| game.players[winner].civ.clone(),
            ),
            game.victory_label().unwrap_or_default(),
            game.reported_turn(),
        )
    });

    for (game_index, (gseed, results, winner, civilization, victory, turn)) in
        played.into_iter().enumerate()
    {
        checkpoint(game_index as u32, gseed, &results)?;
        if cfg.verbose {
            let labels: Vec<String> = results
                .iter()
                .map(|result| {
                    format!(
                        "{}:{}:{}",
                        result.key.player, result.key.leader, result.key.civilization
                    )
                })
                .collect();
            println!(
                "game {game_index:3}  winner={winner:<10} \
                 ({civilization}, {victory}, t{turn})  seats={labels:?}",
            );
        }
    }
    Ok(())
}

pub fn run_tournament<F>(names: &[String], make: F, cfg: &TourneyCfg) -> EloPool
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
{
    let mut pool = EloPool::new(names, ELO_BASE_RATING);
    pool.bind_profile(TournamentProfile::from_cfg(cfg))
        .expect("TourneyCfg always produces a valid rating profile");
    let result: Result<(), std::convert::Infallible> =
        play_tournament(names, &make, cfg, |_, _, players| {
            pool.record_game(players, cfg.k);
            Ok(())
        });
    match result {
        Ok(()) => pool,
        Err(never) => match never {},
    }
}

struct LedgerLock {
    path: PathBuf,
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_ledger_lock(path: &Path) -> io::Result<LedgerLock> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("elo_ratings.json");
    let lock_path = path.with_file_name(format!(".{file_name}.lock"));
    if let Some(parent) = lock_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    for _ in 0..400 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                if let Err(error) = writeln!(file, "{}", std::process::id()) {
                    let _ = fs::remove_file(&lock_path);
                    return Err(error);
                }
                return Ok(LedgerLock { path: lock_path });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::WouldBlock,
        format!(
            "timed out waiting for Elo ledger lock {}",
            lock_path.display()
        ),
    ))
}

#[cfg(test)]
fn update_ledger(path: &Path, update: impl FnOnce(&mut EloPool)) -> io::Result<EloPool> {
    let _lock = acquire_ledger_lock(path)?;
    let mut pool = EloPool::load_or_new(path, ELO_BASE_RATING)?;
    update(&mut pool);
    pool.save(path)?;
    Ok(pool)
}

fn update_profiled_ledger(
    path: &Path,
    profile: &TournamentProfile,
    update: impl FnOnce(&mut EloPool) -> io::Result<()>,
) -> io::Result<EloPool> {
    let _lock = acquire_ledger_lock(path)?;
    let mut pool = EloPool::load_or_new(path, ELO_BASE_RATING)?;
    pool.bind_profile(profile.clone())?;
    update(&mut pool)?;
    pool.save(path)?;
    Ok(pool)
}

fn tournament_event_id(
    run_seed: u64,
    game_index: u32,
    map_seed: u64,
    players: &[RatedPlayer],
) -> String {
    let seats = players
        .iter()
        .map(|player| {
            let name = &player.key.player;
            format!("{}:{name}", name.len())
        })
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "v{ELO_PROTOCOL_VERSION}:{run_seed:020}:{game_index:010}:{map_seed:020}:{seats}"
    )
}

/// Run a tournament against the latest shared ledger and atomically checkpoint
/// every completed game. `cfg.controller_roster` must name the ordered,
/// fixed controller role behind each versioned identity in `names`. The
/// short per-game lock prevents concurrent agents from overwriting one
/// another's updates.
pub fn run_persistent_tournament<F>(
    names: &[String],
    make: F,
    cfg: &TourneyCfg,
    path: impl AsRef<Path>,
) -> io::Result<EloPool>
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
{
    if names.len() < cfg.players_per_game {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "persistent Elo needs at least {} distinct entrants for {} seats; cloned seats change the contest, so add anchors or use an in-memory tournament",
                cfg.players_per_game, cfg.players_per_game,
            ),
        ));
    }
    let distinct: BTreeSet<&String> = names.iter().collect();
    if distinct.len() != names.len() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "persistent Elo entrant names must be unique",
        ));
    }
    if let Some(anchor) = &cfg.rating_anchor {
        if !distinct.contains(anchor) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("rating anchor {anchor:?} must be one of the tournament entrants"),
            ));
        }
    }
    if cfg.controller_roster.len() != names.len() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "persistent Elo needs one ordered controller role per entrant ({} identities, {} controllers)",
                names.len(),
                cfg.controller_roster.len(),
            ),
        ));
    }
    let path = path.as_ref();
    let profile = TournamentProfile::from_cfg(cfg);
    let mut pool = update_profiled_ledger(path, &profile, |_| Ok(()))?;
    play_tournament(names, &make, cfg, |game_index, map_seed, players| {
        let event_id = tournament_event_id(cfg.seed, game_index, map_seed, players);
        pool = update_profiled_ledger(path, &profile, |latest| {
            latest
                .record_game_once(event_id, players, cfg.k)
                .map(|_| ())
        })?;
        Ok::<(), io::Error>(())
    })?;
    Ok(pool)
}

pub fn leaderboard(pool: &EloPool) -> String {
    let mut overall: Vec<(&String, &Rating)> = pool.overall.iter().collect();
    overall.sort_by(|(name_a, a), (name_b, b)| {
        b.elo.total_cmp(&a.elo).then(name_a.cmp(name_b))
    });
    let mut out = String::new();
    if let Some(profile) = &pool.profile {
        out.push_str(&format!("rating profile: {}\n", profile.label()));
    } else {
        out.push_str("rating profile: unbound (migrated/manual pool)\n");
    }
    if pool.history_complete {
        out.push_str(&format!(
            "rating evidence: {} raw games (complete and replay-verified)\n",
            pool.history.len()
        ));
    } else {
        out.push_str(&format!(
            "rating evidence: {} raw games after an unreconstructable legacy prior\n",
            pool.history.len()
        ));
    }
    out.push_str(
        "Anchored online Elo leaderboard (order-sensitive K-factor path, player across all draws):\n",
    );
    for (player, rating) in overall {
        out.push_str(&format!(
            "  {:<24} {:7.1}   games={:<4} wins={:<4} winrate={:>3.0}%\n",
            player,
            rating.elo,
            rating.games,
            rating.wins,
            100.0 * rating.wins as f64 / rating.games.max(1) as f64,
        ));
    }
    if let Some(anchor) = pool
        .profile
        .as_ref()
        .and_then(|profile| profile.rating_anchor.as_deref())
    {
        let mut performance = direct_anchor_performance(pool, anchor);
        performance.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        if !performance.is_empty() {
            let evidence = if pool.history_complete {
                "Standardized direct performance"
            } else {
                "Post-migration direct performance (retained raw games only; legacy prior excluded)"
            };
            out.push_str(&format!(
                "\n{evidence} Elo vs {anchor} (order-independent Jeffreys point; 95% Wilson interval transformed to Elo):\n"
            ));
            for (player, elo, score, games, low, high) in performance {
                let elo_low = performance_elo(pool.base_rating, low);
                let elo_high = performance_elo(pool.base_rating, high);
                out.push_str(&format!(
                    "  {:<24} {:7.1} (95% {:7.1}..{:7.1})   pair-score={:>5.1}/{:<4} ({:>4.1}%, 95% {:>4.1}..{:>4.1}%)\n",
                    player,
                    elo,
                    elo_low,
                    elo_high,
                    score,
                    games,
                    100.0 * score / games as f64,
                    100.0 * low,
                    100.0 * high,
                ));
            }
        }
    }
    out.push_str("\nElo by player × leader × civilization:\n");
    let mut rows: Vec<(&RatingKey, &Rating)> = pool.ratings.iter().collect();
    rows.sort_by(|(key_a, a), (key_b, b)| {
        b.elo
            .total_cmp(&a.elo)
            .then(key_a.player.cmp(&key_b.player))
            .then(key_a.leader.cmp(&key_b.leader))
            .then(key_a.civilization.cmp(&key_b.civilization))
    });
    for (key, rating) in rows {
        out.push_str(&format!(
            "  {:<18} {:<18} {:<12} {:7.1}   games={:<4} wins={:<4} winrate={:>3.0}%\n",
            key.player,
            key.leader,
            key.civilization,
            rating.elo,
            rating.games,
            rating.wins,
            100.0 * rating.wins as f64 / rating.games.max(1) as f64,
        ));
    }
    out
}

/// Maximum-likelihood-style performance ratings directly against the fixed
/// control, derived from raw games rather than the order-sensitive K-factor
/// path. A Jeffreys half-result on each side keeps an undefeated or winless
/// finite sample finite without pretending it was observed.
fn wilson_interval(score: f64, games: usize) -> (f64, f64) {
    if games == 0 {
        return (0.0, 1.0);
    }
    let n = games as f64;
    let p = (score / n).clamp(0.0, 1.0);
    let z = 1.96;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denominator;
    let margin = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denominator;
    ((centre - margin).clamp(0.0, 1.0), (centre + margin).clamp(0.0, 1.0))
}

fn performance_elo(base: f64, pair_score: f64) -> f64 {
    let probability = pair_score.clamp(0.0, 1.0);
    base + 400.0 * (probability / (1.0 - probability)).log10()
}

fn direct_anchor_performance(
    pool: &EloPool,
    anchor: &str,
) -> Vec<(String, f64, f64, usize, f64, f64)> {
    let mut evidence = BTreeMap::<String, (f64, usize)>::new();
    for game in &pool.history {
        let anchor_seats: Vec<&RatedPlayer> = game
            .players
            .iter()
            .filter(|seat| seat.key.player == anchor)
            .collect();
        if anchor_seats.is_empty() {
            continue;
        }
        let opponents: BTreeSet<&str> = game
            .players
            .iter()
            .map(|seat| seat.key.player.as_str())
            .filter(|player| *player != anchor)
            .collect();
        for opponent in opponents {
            let opponent_seats: Vec<&RatedPlayer> = game
                .players
                .iter()
                .filter(|seat| seat.key.player == opponent)
                .collect();
            let mut score = 0.0;
            let mut comparisons = 0usize;
            for challenger in &opponent_seats {
                for control in &anchor_seats {
                    score += head_to_head_score(challenger, control);
                    comparisons += 1;
                }
            }
            let result = score / comparisons.max(1) as f64;
            let aggregate = evidence.entry(opponent.to_string()).or_default();
            aggregate.0 += result;
            aggregate.1 += 1;
        }
    }
    evidence
        .into_iter()
        .filter_map(|(player, (score, games))| {
            (games > 0).then(|| {
                let probability = (score + 0.5) / (games as f64 + 1.0);
                let elo = performance_elo(pool.base_rating, probability);
                let (low, high) = wilson_interval(score, games);
                (player, elo, score, games, low, high)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_ai, builtin_ai_degraded, builtin_ai_strict, builtin_arm, builtin_provenance,
        collapsed_entrants, direct_anchor_performance, expected, leaderboard, league_generalist,
        live_targeted, performance_elo, player_ratings, ratings_path_for, scheduled_seats,
        seat_schedule, strict_builtin_arm_in, wilson_interval, win_shares, ArmKind,
        BuiltinAiBuildError, EloPool, RatedPlayer, Rating, RatingKey, TournamentProfile,
        TourneyCfg, WeightSource, ARTIFACT_DIR, BUILTIN_AIS, CHAMPION_FILE, DEFAULT_RATINGS_PATH,
        ELO_BASE_RATING, ELO_PROTOCOL_VERSION, ELO_SCHEMA_VERSION,
        ENGINE_REPAIR_ECONOMY_TREATMENTS, ENGINE_REPAIR_TREATMENTS, ENGINE_REPAIR_WAR_TREATMENTS,
        EVAL_ONLY_AIS, FIRAXIS_ONLY_TREATMENTS, HISTORICAL_V1_RATINGS_PATH,
        HISTORICAL_V2_RATINGS_PATH, HISTORICAL_V3_RATINGS_PATH, LIVE_BRIDGE_TREATMENTS,
        LIVE_TARGET_LANES, VALUENET_FILE,
    };
    use std::collections::BTreeSet;
    use std::path::Path;
    use crate::game::{Action, Game};
    use crate::rng::Rng;
    use crate::rules::Rules;
    use crate::setup::{GameMode, MapScript};

    #[test]
    fn timed_war_arm_is_one_explicit_axis_and_stays_off_for_control_and_minors() {
        let treatment = builtin_arm("advanced_timing_attack").unwrap();
        let selective = builtin_arm("advanced_timing_attack_selective").unwrap();
        let rapid = builtin_arm("advanced_timing_attack_rapid").unwrap();
        let control = builtin_arm("advanced").unwrap();
        assert_eq!(
            treatment.spec.differing_axes(&control.spec),
            vec!["timed-war-appointment"]
        );
        assert_eq!(
            selective.spec.differing_axes(&control.spec),
            vec!["selective-timed-war-appointment"]
        );
        assert_eq!(
            rapid.spec.differing_axes(&control.spec),
            vec!["rapid-timed-war-appointment"]
        );
        assert_eq!(
            builtin_provenance("advanced_timing_attack", ARTIFACT_DIR).effective,
            "advanced_timing_attack"
        );
        assert_eq!(
            builtin_provenance("advanced_timing_attack_selective", ARTIFACT_DIR).effective,
            "advanced_timing_attack_selective"
        );
        assert_eq!(
            builtin_provenance("advanced_timing_attack_rapid", ARTIFACT_DIR).effective,
            "advanced_timing_attack_rapid"
        );

        for (name, enabled, selective, rapid) in [
            ("advanced", false, false, false),
            ("advanced_timing_attack", true, false, false),
            ("advanced_timing_attack_selective", true, true, false),
            ("advanced_timing_attack_rapid", true, true, true),
        ] {
            let mut game = Game::new(2, 20, 14, 940_012, 80, 0);
            let mut ai = builtin_ai_strict(name, 940_012).unwrap();
            ai.take_turn(&mut game, 0);
            let war = ai.plan_report().and_then(|plan| plan.war);
            assert_eq!(
                war.as_ref().is_some_and(|war| war.enabled),
                enabled,
                "{name} must report its actual treatment state"
            );
            assert_eq!(
                war.as_ref().is_some_and(|war| war.selective),
                selective,
                "{name} must report its actual appointment policy"
            );
            assert_eq!(
                war.as_ref().is_some_and(|war| war.rapid),
                rapid,
                "{name} must report its actual rapid policy"
            );
        }

        let mut minor_game = Game::new(2, 20, 14, 940_013, 80, 0);
        minor_game.players[0].is_minor = true;
        let mut treatment = builtin_ai_strict("advanced_timing_attack", 940_013).unwrap();
        treatment.take_turn(&mut minor_game, 0);
        assert!(
            treatment.plan_report().is_none(),
            "city-states remain on the Basic controller and never form elective appointments"
        );
    }
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn short_game_fingerprint(name: &str, seed: u64) -> String {
        let mut game = Game::new_full(2, 24, 16, seed, 30, 0, false);
        let mut ais = (0..game.players.len())
            .map(|pid| builtin_ai(name, seed.wrapping_add(pid as u64)))
            .collect::<Vec<_>>();
        while game.winner.is_none() {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        serde_json::to_string(&game).expect("serialize deterministic game fingerprint")
    }

    /// A checkout with no trained artifacts is the default state of this
    /// repository — `evolved/` is generated and ignored — so every learned
    /// name must report the scripted agent it really is.
    #[test]
    fn a_bare_checkout_reports_the_agent_that_actually_plays() {
        let dir = "target/test-provenance-bare";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for (name, effective) in [
            ("evolved", "advanced"),
            ("advanced_evolved", "advanced"),
            ("strategic", "strategic_score"),
        ] {
            let resolved = builtin_provenance(name, dir);
            assert_eq!(resolved.effective, effective, "{name}");
            assert!(resolved.degraded(), "{name}");
            assert!(resolved.untrained(), "{name}");
            assert!(resolved.line().contains("missing"), "{}", resolved.line());
        }
        fs::remove_dir_all(dir).unwrap();
    }

    /// The scripted names promise nothing they load, so they are never
    /// degraded and never untrained — including on a bare checkout.
    #[test]
    fn scripted_names_are_never_degraded() {
        let dir = "target/test-provenance-scripted";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for name in ["advanced", "advanced_v1", "basic", "random"] {
            let resolved = builtin_provenance(name, dir);
            assert_eq!(resolved.effective, name);
            assert!(!resolved.degraded(), "{name}");
            assert!(!resolved.untrained(), "{name}");
        }
        // The evaluator-only control refuses a net by construction, so a
        // missing net is not a degradation for it — only the genome is.
        let control = builtin_provenance("strategic_score", dir);
        assert_eq!(control.effective, "strategic_score");
        assert!(!control.degraded());
        assert!(control.untrained());
        fs::remove_dir_all(dir).unwrap();
    }

    /// A player carries one rating per mode plus an overall. The per-mode
    /// numbers are the ladders' own; the overall is the games-weighted mean
    /// of them, so a rating earned over forty games counts for more than one
    /// earned over four, and a mode nobody has played yet is simply absent
    /// rather than a 1500 dragging every average toward the middle.
    #[test]
    fn a_player_carries_one_rating_per_mode_and_an_overall() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-mode-ratings-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch ladder directory");
        let write = |mode: GameMode, rows: &[(&str, f64, u32, u32)]| {
            let mut pool = EloPool::new(&[], ELO_BASE_RATING);
            // A hand-written ladder has no raw game log to audit its
            // aggregates against, which is exactly the migrated-ledger shape
            // `history_complete` exists for.
            pool.history_complete = false;
            for (player, elo, games, wins) in rows {
                pool.overall.insert(
                    (*player).to_string(),
                    Rating { elo: *elo, games: *games, wins: *wins },
                );
            }
            let name = Path::new(ratings_path_for(mode))
                .file_name()
                .expect("ladder file name");
            pool.save(&dir.join(name)).expect("write scratch ladder");
        };
        write(
            GameMode::Civ,
            &[("advanced", 1600.0, 40, 28), ("basic", 1400.0, 40, 6)],
        );
        write(GameMode::Tactics, &[("advanced", 1800.0, 10, 9)]);
        // Sim City has no ladder file: the mode is declared and unplayed.

        let ratings = player_ratings(&dir);
        let advanced = &ratings["advanced"];
        assert_eq!(advanced.by_mode["civ"].elo, 1600.0);
        assert_eq!(advanced.by_mode["tactics"].elo, 1800.0);
        assert!(!advanced.by_mode.contains_key("simcity"), "an unplayed mode is absent");
        assert_eq!(advanced.games, 50);
        // Weighted by games played, not a flat mean of the two ladders: forty
        // Civ games at 1600 and ten Tactics games at 1800 is 1640, not 1700.
        assert!((advanced.overall - 1640.0).abs() < 1e-9, "{}", advanced.overall);
        // A player who has only ever played one mode is that mode's rating.
        let basic = &ratings["basic"];
        assert_eq!(basic.by_mode.len(), 1);
        assert!((basic.overall - 1400.0).abs() < 1e-9);

        // A Tactics result cannot be filed on the Civ ladder even by hand:
        // the profile records the map script, and the ledger refuses a game
        // whose setup is not its own.
        let ladder_profile = |script: MapScript| TournamentProfile {
            protocol_version: ELO_PROTOCOL_VERSION,
            rules_fingerprint: Rules::embedded().source_fingerprint().to_string(),
            setup_contract: "test".to_string(),
            rating_anchor: None,
            controller_roster: Vec::new(),
            players_per_game: 2,
            width: 10,
            height: 10,
            max_turns: 250,
            num_city_states: 0,
            speed: "standard".to_string(),
            map_script: script.id().to_string(),
            map_topology: "flat".to_string(),
            map_poles: "poles".to_string(),
            mods: Vec::new(),
            k: 24.0,
        };
        let civ_profile = ladder_profile(MapScript::Pangaea);
        let arena_profile = ladder_profile(MapScript::Battlefield);
        let mut ledger = EloPool::new(&[], ELO_BASE_RATING);
        ledger.bind_profile(civ_profile).expect("first run binds the ladder");
        let refusal = ledger.bind_profile(arena_profile).expect_err("modes must not mix");
        assert!(refusal.to_string().contains("rating profile mismatch"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn strict_construction_refuses_degraded_names_and_unknown_names() {
        let dir = "target/test-strict-builtin-arms";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();

        let error = strict_builtin_arm_in("strategic", dir)
            .expect_err("a bare artifact tier must not construct the learned searcher");
        assert_eq!(error.requested(), "strategic");
        let provenance = error
            .provenance()
            .expect("a known degraded arm reports its provenance");
        assert!(provenance.degraded());
        assert_eq!(provenance.effective, "strategic_score");
        assert_eq!(provenance.missing(), vec![CHAMPION_FILE, VALUENET_FILE]);

        let unknown = strict_builtin_arm_in("not-a-selectable-arm", dir)
            .expect_err("strict construction must reject an unknown name");
        assert!(matches!(unknown, BuiltinAiBuildError::UnknownName { .. }));
        assert!(strict_builtin_arm_in("advanced", dir).is_ok());
        fs::remove_dir_all(dir).unwrap();

        builtin_ai_strict("basic", 78_000_090)
            .expect("a production scripted arm must construct strictly");
        let _ = builtin_ai_degraded("not-a-selectable-arm", 78_000_091);
    }

    /// Presence is decided by the loaders the agents use. A file that exists
    /// but cannot load leaves the agent scripted, so provenance must not
    /// call it found.
    #[test]
    fn an_unloadable_artifact_is_not_a_loaded_one() {
        let dir = "target/test-provenance-corrupt";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(format!("{dir}/{VALUENET_FILE}"), "{\"sizes\":[1,2]}").unwrap();
        fs::write(format!("{dir}/{CHAMPION_FILE}"), "not json").unwrap();
        let resolved = builtin_provenance("strategic", dir);
        assert_eq!(resolved.effective, "strategic_score");
        assert_eq!(resolved.missing(), vec![CHAMPION_FILE, VALUENET_FILE]);
        fs::remove_dir_all(dir).unwrap();
    }

    /// A missing net changes the controller, not its already-loaded genome.
    /// The effective identity must retain both or the self-comparison guard
    /// warns on distinct agents and misses identical ones.
    #[test]
    fn champion_weighted_fallbacks_keep_the_genome_in_their_identity() {
        let dir = "target/test-provenance-champion-fallback";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::copy(
            format!("data/evolved/{CHAMPION_FILE}"),
            format!("{dir}/{CHAMPION_FILE}"),
        )
        .expect("the committed champion fixture must be available");

        let evolved_alias = builtin_provenance("evolved", dir);
        assert_eq!(evolved_alias.effective, "advanced_evolved");
        assert!(!evolved_alias.degraded(), "a loaded alias is not degraded");
        assert_eq!(
            collapsed_entrants(&["evolved", "advanced_evolved"], dir),
            vec![(
                "evolved".to_string(),
                "advanced_evolved".to_string(),
                "advanced_evolved"
            )]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// Production resolves the committed embedded champion even when there is
    /// no generated `evolved/` directory. Pin that real resolution path, not
    /// only the useful-but-impossible bare-directory fixture above.
    #[test]
    fn production_fallback_identity_includes_the_embedded_champion() {
        assert!(
            crate::evolve::load_champion(ARTIFACT_DIR).is_some(),
            "the production artifact tier must resolve the committed champion"
        );
        assert_eq!(
            builtin_provenance("evolved", ARTIFACT_DIR).effective,
            "advanced_evolved",
            "the embedded champion must load for the evolved alias"
        );
        assert_eq!(
            collapsed_entrants(
                &["advanced_banking_dedication", "advanced_evolved"],
                ARTIFACT_DIR
            )[0]
                .2,
            "advanced_evolved"
        );
        assert_eq!(
            collapsed_entrants(&["strategic_warm", "strategic"], ARTIFACT_DIR)[0].2,
            "strategic_score"
        );
    }

    #[test]
    fn typed_specs_keep_aliases_collapsed_and_components_visible() {
        let evolved = builtin_arm("advanced_evolved").expect("control is selectable");
        let stock = builtin_arm("advanced").expect("stock is selectable");
        assert_eq!(
            evolved.spec.differing_axes(&stock.spec),
            vec!["weights"],
            "the champion comparison is a one-axis control"
        );

        let science = builtin_arm("advanced_target_science").expect("control is selectable");
        let district_veto = builtin_arm("advanced_great_work_veto_by_district")
            .expect("district-keyed treatment is selectable");
        assert_eq!(
            district_veto.spec.differing_axes(&science.spec),
            vec!["great-work-veto-by-district"],
            "the veto-key comparison must hold the Science target fixed"
        );

        let culture = builtin_arm("advanced_target_culture").expect("control is selectable");
        let culture_debt = builtin_arm("advanced_target_culture_with_culture_building_debt")
            .expect("Culture debt treatment is selectable");
        assert_eq!(
            culture_debt.spec.differing_axes(&culture.spec),
            vec!["culture-building-debt"],
            "the Culture debt comparison must hold the victory target fixed"
        );

        let economy = builtin_arm("advanced_envoy_economy").expect("arm is selectable");
        assert_eq!(
            economy.spec.differing_axes(&stock.spec),
            vec![
                "envoy-influence",
                "envoy-infrastructure-on",
                "envoy-priority-off",
            ],
            "the economy control must expose every change from the cleaned production default"
        );

        let policy_priority = builtin_arm("advanced_policy_envoy_priority")
            .expect("composite is selectable");
        assert_eq!(policy_priority.spec, stock.spec);
        assert_eq!(
            builtin_provenance("advanced_policy_envoy_priority", ARTIFACT_DIR).line(),
            "advanced_policy_envoy_priority: plays as advanced (scripted, no artifacts required)"
        );
        assert_eq!(
            collapsed_entrants(
                &["advanced_policy_envoy_priority", "advanced"],
                ARTIFACT_DIR
            ),
            vec![(
                "advanced_policy_envoy_priority".to_string(),
                "advanced".to_string(),
                "advanced"
            )],
            "the promoted composite must be a real factory alias, not a duplicate arm"
        );

        let live_control = builtin_arm("advanced_policy_live_control")
            .expect("pre-promotion control is selectable");
        assert_eq!(
            live_control.spec.differing_axes(&stock.spec),
            vec!["envoy-priority-off"],
            "the live-policy control must revert only the retained envoy-priority mechanism"
        );

        let priority = builtin_arm("advanced_envoy_priority")
            .expect("pre-promotion priority control is selectable");
        assert_eq!(
            priority.spec.differing_axes(&stock.spec),
            vec!["policy-deck-legacy", "envoy-infrastructure-on"],
            "the priority control must expose its Legacy deck and restored valuation"
        );

        let bounded = builtin_arm("advanced_without_bounded_recovery")
            .expect("historical bounded-recovery alias is selectable");
        assert_eq!(bounded.spec, stock.spec);
        assert_eq!(
            builtin_provenance("advanced_without_bounded_recovery", ARTIFACT_DIR).effective,
            "advanced"
        );

        let league_top = builtin_arm("advanced_league_top").expect("arm is selectable");
        assert_eq!(
            league_top.spec.weights,
            WeightSource::League,
            "the committed roster has an active Advanced top weight source"
        );
    }

    #[test]
    fn every_selectable_typed_arm_has_a_factory_and_matching_provenance() {
        for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS.iter()) {
            let arm = builtin_arm(name).expect("every selectable name has a typed arm");
            let _ = arm.build(78_000_100);
            assert_eq!(
                builtin_provenance(name, ARTIFACT_DIR).effective,
                arm.spec.canonical,
                "provenance and factory disagree for {name}"
            );
        }
    }

    #[test]
    fn every_distinct_same_family_arm_declares_a_semantic_axis() {
        let arms = BUILTIN_AIS
            .iter()
            .chain(EVAL_ONLY_AIS.iter())
            .map(|name| (*name, builtin_arm(name).expect("every selectable name has a typed arm")))
            .collect::<Vec<_>>();
        for (index, (left_name, left)) in arms.iter().enumerate() {
            for (right_name, right) in arms.iter().skip(index + 1) {
                let axes = left.spec.differing_axes(&right.spec);
                assert!(
                    !axes.contains(&"implementation"),
                    "{left_name} and {right_name} need an explicit treatment or source axis: {axes:?}"
                );
            }
        }
    }

    #[test]
    fn production_aliases_play_as_their_typed_specs_claim() {
        for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS.iter()) {
            let arm = builtin_arm(name).expect("every selectable name has a typed arm");
            let effective = arm.spec.canonical;
            if *name == effective {
                continue;
            }
            for seed in [78_000_101, 78_000_102] {
                assert_eq!(
                    short_game_fingerprint(name, seed),
                    short_game_fingerprint(effective, seed),
                    "{name} does not play as its typed canonical arm {effective} on seed {seed}"
                );
            }
        }
    }

    /// Two entrants that resolve to one agent make their difference noise.
    #[test]
    fn entrants_that_collapse_to_one_agent_are_reported() {
        let dir = "target/test-provenance-collapse";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        let collapsed = collapsed_entrants(&["evolved", "advanced", "basic"], dir);
        assert_eq!(
            collapsed,
            vec![("evolved".to_string(), "advanced".to_string(), "advanced")]
        );
        assert!(collapsed_entrants(&["advanced", "basic", "random"], dir).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    /// Every selectable entrant name must have an explicit provenance row,
    /// so adding a builtin cannot quietly inherit the catch-all. The
    /// catch-all reports "no artifacts required", which for a learned
    /// entrant is a false statement rather than a missing one — exactly
    /// what this module exists to prevent, and it happened once
    /// (`policy_wide`) before this assertion was tightened.
    #[test]
    fn every_selectable_name_resolves_to_itself_or_a_named_fallback() {
        let dir = "target/test-provenance-names";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS.iter()) {
            let resolved = builtin_provenance(name, dir);
            assert!(
                BUILTIN_AIS.contains(&resolved.effective)
                    || EVAL_ONLY_AIS.contains(&resolved.effective),
                "{name} resolved to unknown {}",
                resolved.effective
            );
            // Only genuinely scripted agents and declared historical aliases
            // may report no artifacts.
            // Anything else reaching that state fell through to the
            // catch-all and is claiming to need nothing while quietly
            // needing a net.
            const SCRIPTED: &[&str] = &[
                "advanced_build_first",
                "advanced_synergy",
                "advanced_synergy_war",
                "advanced_synergy_economy",
                "advanced_joint_tactics",
                "advanced",
                "fog_honest",
                "advanced_belief_pressure",
                "advanced_policy_live_control",
                "advanced_policy_envoy_priority",
                "advanced_envoy_policy",
                "advanced_envoy_infrastructure",
                "advanced_envoy_priority",
                "advanced_envoy_economy",
                "advanced_blind_to_leaders",
                "advanced_rush",
                "advanced_rush_connected",
                "advanced_timing_attack",
                "advanced_timing_attack_selective",
                "advanced_timing_attack_rapid",
                "advanced_congress_counter",
                "advanced_congress_votes",
                "advanced_congress_counter_hard",
                "advanced_counter_in_lane",
                "advanced_counter_stand_down",
                "advanced_early_score_alarm",
                "advanced_early_score_build",
                "advanced_garrison_loyalty",
                "advanced_settler_commit",
                "advanced_wide_opening",
                "advanced_plan_city_target",
                "advanced_expansion_payback",
                "advanced_late_expansion",
                "advanced_expansion_dispatch",
                "advanced_expansion_complete",
                "advanced_coupled_expansion",
                "advanced_city_strategy",
                "advanced_city_strategy_emphasis",
                "advanced_city_strategy_roles",
                "advanced_city_strategy_roles_raw",
                "advanced_city_strategy_raw",
                "advanced_city_strategy_bastion_only",
                "advanced_city_strategy_breadbasket_only",
                "advanced_city_strategy_comparative_only",
                "advanced_city_strategy_pressure_only",
                "advanced_civ_blind",
                "advanced_league_top",
                "advanced_war_half",
                "advanced_settler_first",
                "advanced_holy_priority",
                "advanced_holy_lane",
                "advanced_holy_v0",
                "advanced_settle_food",
                "advanced_holy_lane_v0",
                "advanced_roster_live",
                "advanced_roster_live_keep_districts",
                "advanced_diplomatic_opening",
                "advanced_without_bounded_recovery",
                "advanced_without_city_target_floor",
                "advanced_without_plan_city_target",
                "advanced_without_settler_commit",
                "advanced_without_unpriced_bundle",
                "advanced_without_settlement_safety",
                "advanced_without_battlefront_observation",
                "advanced_lower_city_target",
                "advanced_settler_founds_when_stalled",
                "advanced_fortify_idle_units",
                "advanced_open_water_navy",
                "advanced_maritime_splice",
                "advanced_sea_answers",
                "advanced_without_barbarian_scouts_are_scouts",
                "advanced_engine_faith_price",
                "advanced_maintenance_deck",
                "advanced_recon_fleet",
                "advanced_without_recon_fleet",
                "advanced_every_lane",
                "advanced_builder_survey",
                "advanced_unit_efficiency",
                "advanced_without_unpriced_economy",
                "advanced_without_unpriced_war",
                "advanced_without_city_defence",
                "advanced_legacy_policy_deck",
                "advanced_without_builder_floor",
                "advanced_without_settler_deadline",
                "advanced_without_hut_collection",
                "advanced_without_explore_commit",
                "advanced_without_village_seeking",
                "advanced_price_suzerainty",
                "advanced_without_unit_tactics",
                // Built from code, not from a weights artifact: these six differ
                // from `advanced` only in the victory lane they are handed.
                "advanced_target_domination",
                "advanced_target_score",
                "advanced_target_science",
                "advanced_great_work_veto_by_district",
                "advanced_target_culture",
                "advanced_target_culture_with_culture_building_debt",
                "advanced_target_religious",
                "advanced_target_diplomatic",
                "advanced_v1",
                "basic",
                // Scripted composites of bridge flags: built from code, no
                // weights artifact and no value net.
                "live",
                "live_target_science",
                "live_target_culture",
                "live_target_religious",
                "live_target_diplomatic",
                "live_target_domination",
                "live_target_score",
                "random",
            ];
            const SCRIPTED_ALIASES: &[&str] = &[
                "advanced_policy_envoy_priority",
                // Aliases `advanced` since the 2026-08-17 recon-fleet
                // promotion shipped the quartet it used to apply.
                "advanced_recon_fleet",
                "advanced_holy_v0",
                // The measured-null recovery repair is already off in
                // production; retain its old withhold name as a self-play
                // alias so historical commands fail closed.
                "advanced_without_bounded_recovery",
                "advanced_without_city_target_floor",
                "advanced_plan_city_target",
                // The war-half withhold arms alias `advanced` since the
                // 2026-08-14 removal shipped what they used to withhold to.
                "advanced_without_unpriced_war",
                "advanced_without_city_defence",
                "advanced_without_unit_tactics",
            ];
            assert!(
                !resolved.artifacts.is_empty()
                    // Every `live_without_*` arm is scripted by construction:
                    // it is the live bundle with one treatment withheld, and
                    // it loads no artifact. Matched by prefix rather than
                    // listed, because the list this replaces was the eighth
                    // place a new treatment had to be written down and the
                    // comment below already said a name list "stops
                    // discriminating as it grows".
                    || name.starts_with("live_without_")
                    || SCRIPTED.contains(name)
                    || SCRIPTED_ALIASES.contains(name),
                "{name} has no provenance row and inherited the catch-all"
            );
            // The whitelist above is a list of names, so it grows every time
            // a scripted entrant is added and stops discriminating as it
            // does. This does not: the catch-all answers `basic`, so any
            // name that needs no artifacts and still does not resolve to
            // itself reached that arm rather than a row of its own.
            if resolved.artifacts.is_empty() && !SCRIPTED_ALIASES.contains(name) {
                assert_eq!(
                    resolved.effective, *name,
                    "{name} needs no artifacts yet resolves to {}, which only \
                     the catch-all does",
                    resolved.effective
                );
            }
        }
        fs::remove_dir_all(dir).unwrap();
    }

    /// The direct `civvis_orders` binary can combine every explicit victory
    /// target with the deployed bridge. Until this contract, the evaluator
    /// could model either side of that composition (`advanced_target_*` or
    /// adaptive `live`) but not the actual six target-pinned live seats.
    #[test]
    fn live_targeted_arms_cover_every_explicit_victory_lane() {
        let targets: Vec<_> = LIVE_TARGET_LANES
            .iter()
            .map(|(_, target, _)| *target)
            .collect();
        assert_eq!(targets, crate::ai::VictoryTarget::ALL.to_vec());

        let live = builtin_arm("live").expect("adaptive live arm is registered");
        for &(lane, target, axis) in LIVE_TARGET_LANES {
            assert_eq!(lane, target.as_str(), "{lane} must name its target");

            let ai = live_targeted(lane);
            assert_eq!(ai.victory_target(), Some(target), "{lane}");
            assert!(ai.parallel_settlers, "{lane} lost a live-bridge repair");
            assert!(ai.bank_envoys, "{lane} lost a live-bridge repair");

            let name = format!("live_target_{lane}");
            assert!(EVAL_ONLY_AIS.contains(&name.as_str()), "{name}");
            let arm = builtin_arm(&name).unwrap_or_else(|| panic!("{name} is registered"));
            assert_eq!(arm.spec.treatments.first().copied(), Some(axis), "{name}");
            assert_eq!(&arm.spec.treatments[1..], LIVE_BRIDGE_TREATMENTS, "{name}");
            assert_eq!(
                arm.spec.differing_axes(&live.spec),
                vec![axis],
                "{name} must differ from adaptive live only by its target"
            );
        }
    }

    /// `AdvancedAi::enable_live_bridge` is the single place the Civilization VI
    /// bridge turns its repairs on; `LIVE_BRIDGE_TREATMENTS` is how the
    /// evaluator names them. Until now the only thing keeping the two in step
    /// was a comment.
    ///
    /// A flag added to the helper and not to the tag list is silent: both sides
    /// compile, every other test passes, and the `live` vs `live_without_*`
    /// comparison goes on reporting a controlled experiment it is no longer
    /// running. #977 shipped exactly that way — `army_target_weighs_the_enemy`
    /// reached the deployment while the tag list still described ten
    /// mechanisms — and it was caught by reading the merge, not by CI.
    #[test]
    fn live_bridge_treatments_name_every_flag_the_helper_sets() {
        let source = include_str!("ai/advanced.rs");
        let body = source
            .split("pub fn enable_live_bridge(&mut self) {")
            .nth(1)
            .and_then(|tail| tail.split("\n    }\n").next())
            .expect("enable_live_bridge body");
        let enabled = body.matches("self.enable_").count();
        assert_eq!(
            enabled,
            LIVE_BRIDGE_TREATMENTS.len(),
            "enable_live_bridge sets {enabled} flags but LIVE_BRIDGE_TREATMENTS names {}: \
             add the missing tag (and give it a `live_without_*` arm) or the evaluator \
             arms claim a controlled comparison they are not running",
            LIVE_BRIDGE_TREATMENTS.len()
        );
    }

    /// The evaluator's public tag list and the table that supplies each
    /// withholding function are two representations of one deployment
    /// identity. A count alone cannot catch a swapped or renamed treatment.
    #[test]
    fn live_bridge_tags_match_the_withholding_table() {
        let withholding_tags: Vec<&str> = crate::ai::LIVE_TREATMENTS
            .iter()
            .map(|(_, tag, _)| *tag)
            .collect();
        assert_eq!(
            LIVE_BRIDGE_TREATMENTS,
            withholding_tags.as_slice(),
            "the evaluator stamp and live withholding table disagree; a deployment treatment would be unmeasurable or mislabeled"
        );
    }

    /// `AdvancedAi::enable_engine_repairs` claims to be `enable_live_bridge`
    /// minus its deployment-profile treatments. Nothing but this test holds
    /// that claim up.
    ///
    /// It fails in the same silent way as the check above, from the other
    /// side: a repair added to the bridge and not to the native bundle
    /// compiles, passes every other test, and quietly makes `advanced_synergy`
    /// a different treatment than the one its documentation — and whatever
    /// eval record it has by then accumulated — describes.
    /// The two halves of the engine-repair bundle partition the whole.
    ///
    /// ⚠ THIS HELD BY MAINTENANCE, NOT BY CONSTRUCTION. `AdvancedSynergyWar`
    /// and `AdvancedSynergyEconomy` are the two arms that price the repair
    /// bundle by splitting it, and that pricing only means anything if every
    /// repair is in exactly one half. Nothing checked it. A repair added to
    /// `ENGINE_REPAIR_TREATMENTS` and to neither half would be withheld by
    /// neither arm, so the split would report the same bundle twice under two
    /// names — and a repair in both halves would be withheld by both, which is
    /// the opposite error and reads identically in the table.
    #[test]
    fn the_war_and_economy_halves_partition_the_repair_bundle() {
        use std::collections::BTreeSet;
        let war: BTreeSet<_> = ENGINE_REPAIR_WAR_TREATMENTS.iter().collect();
        let economy: BTreeSet<_> = ENGINE_REPAIR_ECONOMY_TREATMENTS.iter().collect();
        let all: BTreeSet<_> = ENGINE_REPAIR_TREATMENTS.iter().collect();

        let both: Vec<_> = war.intersection(&economy).collect();
        assert!(
            both.is_empty(),
            "these repairs are in BOTH halves, so both arms withhold them and \
             neither arm prices what it claims to: {both:?}"
        );

        let halves: BTreeSet<_> = war.union(&economy).copied().collect();
        let unclaimed: Vec<_> = all.difference(&halves).collect();
        assert!(
            unclaimed.is_empty(),
            "these repairs are in the bundle but in NEITHER half, so the split \
             prices the same bundle twice under two names: {unclaimed:?}"
        );

        let stray: Vec<_> = halves.difference(&all).collect();
        assert!(
            stray.is_empty(),
            "these repairs are in a half but not in the bundle it splits: {stray:?}"
        );

        assert_eq!(
            war.len() + economy.len(),
            all.len(),
            "the halves and the whole disagree on size even though the sets match"
        );
    }

    /// No list here carries a hand-typed length any more.
    ///
    /// ⚠ THIS IS THE DEFECT THAT BROKE `main`. Every one of these was a
    /// `[&str; N]` whose N was typed by hand — the largest was 188. Adding an
    /// entry without editing the number is a compile error in a build most
    /// authors never run locally, and on 2026-08-17 #1865 added an arm, missed
    /// the count, and left `main` unable to build for wasm until #1869 fixed
    /// the number. `&[&str]` cannot go stale, so the class is gone rather than
    /// the instance.
    #[test]
    fn no_list_in_this_file_carries_a_hand_typed_length() {
        let source = include_str!("elo.rs");
        let offenders: Vec<&str> = source
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                (line.starts_with("const ") || line.starts_with("pub const "))
                    && line.contains(": [&str; ")
            })
            .collect();
        assert!(
            offenders.is_empty(),
            "these declare a length by hand, which goes stale the next time \
             somebody adds an entry — use `&[&str]`:\n  {}",
            offenders.join("\n  ")
        );
    }

    #[test]
    fn engine_repairs_are_the_live_bridge_minus_the_firaxis_semantics() {
        /// Each of these encodes a rule of Firaxis' game rather than repairing
        /// one of ours, except the last, which is excluded on evidence: the
        /// deployment-profile run split every map at +0 Elo for 2.5x the
        /// rollout branches.
        const EXCLUDED: &[&str] = &[
            "live_trader_route_adapter",
            "live_religious_purchase_guard",
            "solvent_faith_army",
            "joint_tactics",
            // Prices a Firaxis-specific opportunity: an uncontested wonder
            // catalogue on the Settler seat and a score tally at the host's
            // turn limit. CIVVIS-vs-CIVVIS wonders are the contested race the
            // stock gate was written for, so the native bundle keeps it.
            "live_wonder_race",
            // Prices the Settler seat's slow Prophet race; the native
            // contenders are the real race the stock order was written for.
            "expansion_before_prophet",
            // Prices the Settler seat's elective-war record (never converted);
            // native wars are the ones the branch was written for.
            "no_elective_war",
            // Reads the live mirror's fog; a native board has none.
            "fog_land_capacity",
            // The native response shape has its own `advanced_counter_*` arms.
            "counter_in_lane",
            // The Settler seat's era pace; the league cadence stays bred.
            "era_paced_expansion",
            // The Settler seat's tally weights; the native lanes stay bred.
            "tally_culture",
            // The Settler seat's tally price of that chain's buildings.
            "culture_building_debt",
            // The Settler seat's tally price of the coverage those weights
            // never bought.
            "culture_coverage",
            // Reads the live mirror's fog around a settle site.
            "frontier_loyalty",
            // The Settler seat's tally price of a Great Person.
            "tally_great_people",
            // Only a seat playing under an assigned lane (`--victory
            // science`, the Settler seat's standing order) has a target gate
            // to override; the native gate agents are adaptive, so the flag
            // cannot fire there.
            "deny_while_targeted",
            // Same: priced on the live seat's steal record, not native play.
            "stock_denial_lead_time",
            // Host movement and production semantics, not native engine
            // repairs. `explore_commit` is already set by production
            // Advanced, but stays in the live registry for full parity.
            "parallel_settlers",
            "host_settler_pop",
            "explore_dead_targets",
            "explore_commit",
            "bank_envoys",
        ];
        let source = include_str!("ai/advanced.rs");
        let calls = |name: &str| -> BTreeSet<String> {
            let body = source
                .split(&format!("pub fn {name}(&mut self) {{"))
                .nth(1)
                .and_then(|tail| tail.split("\n    }\n").next())
                .unwrap_or_else(|| panic!("no body found for {name}"));
            body.match_indices("self.enable_")
                .map(|(at, _)| {
                    let rest = &body[at + "self.enable_".len()..];
                    rest[..rest.find('(').expect("an enable call")].to_string()
                })
                .collect()
        };

        // The parent must actually delegate, or the halves could agree with
        // the bridge while `advanced_synergy` carried neither of them.
        let parent = calls("enable_engine_repairs");
        assert_eq!(
            parent,
            BTreeSet::from([
                "engine_repairs_economy".to_string(),
                "engine_repairs_war".to_string(),
            ]),
            "enable_engine_repairs must be exactly its two halves"
        );

        let bridge = calls("enable_live_bridge");
        let war = calls("enable_engine_repairs_war");
        let economy = calls("enable_engine_repairs_economy");
        let overlap: Vec<&String> = war.intersection(&economy).collect();
        assert!(
            overlap.is_empty(),
            "a repair is in both halves, so the halves are not a partition \
             and their separate measurements would double-count it: {overlap:?}"
        );

        let native: BTreeSet<String> = war.union(&economy).cloned().collect();
        let excluded: BTreeSet<String> = EXCLUDED.iter().map(|tag| tag.to_string()).collect();
        let smuggled: Vec<&String> = native.intersection(&excluded).collect();
        assert!(
            smuggled.is_empty(),
            "the native bundle carries a Firaxis-semantics flag: {smuggled:?}"
        );

        let expected: BTreeSet<String> = bridge.difference(&excluded).cloned().collect();
        assert_eq!(
            native,
            expected,
            "enable_engine_repairs and enable_live_bridge have drifted. \
             Missing from the native bundle: {:?}. Not in the bridge at all: {:?}. \
             Every bridge repair is a native repair unless it encodes a Firaxis \
             rule — if a new one does, add it to EXCLUDED here with its reason.",
            expected.difference(&native).collect::<Vec<_>>(),
            native.difference(&expected).collect::<Vec<_>>(),
        );
        assert_eq!(
            bridge.len(),
            native.len() + EXCLUDED.len(),
            "the bridge must be the native bundle plus exactly the exclusions"
        );
    }

    /// The flag-level check above proves the two *helpers* agree. This proves
    /// the two *tag lists* do, which is what `differing_axes` actually reports.
    ///
    /// Both are needed: a repair could be correctly added to
    /// `enable_engine_repairs_war` and its tag forgotten here, and then
    /// `advanced_synergy` vs `advanced` would silently under-report its own
    /// axes — the same defect #977 shipped, one level up.
    #[test]
    fn engine_repair_tags_partition_the_bridge() {
        let war: BTreeSet<&str> = ENGINE_REPAIR_WAR_TREATMENTS.iter().copied().collect();
        let economy: BTreeSet<&str> = ENGINE_REPAIR_ECONOMY_TREATMENTS.iter().copied().collect();
        assert_eq!(
            war.len(),
            ENGINE_REPAIR_WAR_TREATMENTS.len(),
            "a duplicate war tag would make the halves overlap silently"
        );
        assert_eq!(
            economy.len(),
            ENGINE_REPAIR_ECONOMY_TREATMENTS.len(),
            "a duplicate economy tag would make the halves overlap silently"
        );
        let both: Vec<&&str> = war.intersection(&economy).collect();
        assert!(
            both.is_empty(),
            "the halves must partition the bundle, or measuring them \
             separately double-counts a repair: {both:?}"
        );

        let whole: BTreeSet<&str> = ENGINE_REPAIR_TREATMENTS.iter().copied().collect();
        let halves: BTreeSet<&str> = war.union(&economy).copied().collect();
        assert_eq!(
            whole, halves,
            "ENGINE_REPAIR_TREATMENTS must be exactly its two halves"
        );

        let bridge: BTreeSet<&str> = LIVE_BRIDGE_TREATMENTS.iter().copied().collect();
        let firaxis: BTreeSet<&str> = FIRAXIS_ONLY_TREATMENTS.iter().copied().collect();
        assert!(
            firaxis.is_subset(&bridge),
            "an exclusion names a treatment the bridge does not carry"
        );
        let expected: BTreeSet<&str> = bridge.difference(&firaxis).copied().collect();
        assert_eq!(
            whole,
            expected,
            "the native tag list has drifted from the bridge. Missing: {:?}. \
             Unknown to the bridge: {:?}.",
            expected.difference(&whole).collect::<Vec<_>>(),
            whole.difference(&expected).collect::<Vec<_>>(),
        );
    }

    /// ⚠ The run log's identity must be the SAME list `enable_live_bridge`
    /// drives, or a stale binary would still look current. That agreement is
    /// already enforced by `live_bridge_treatments_name_every_flag_the_helper_sets`;
    /// this pins that the list is PUBLIC, which is what makes it emittable, and
    /// that it is non-empty so the stamp can never be a silently empty array.
    #[test]
    fn the_treatment_list_is_emittable_as_a_run_stamp() {
        let stamped: Vec<&str> = crate::elo::LIVE_BRIDGE_TREATMENTS.to_vec();
        assert!(
            stamped.len() >= 20,
            "a run stamp of {} treatments is too short to be this build",
            stamped.len()
        );
        assert!(
            stamped.iter().all(|tag| !tag.is_empty()),
            "an empty tag would make the stamp unreadable"
        );
        // The stamp is only useful if a binary predating a repair emits a
        // shorter list, so every tag must be distinct.
        let unique: BTreeSet<&str> = stamped.iter().copied().collect();
        assert_eq!(unique.len(), stamped.len(), "a duplicate tag breaks the diff");
    }

    /// Each `live_without_*` arm exists to hold exactly ONE mechanism off, so
    /// `differing_axes` against `live` names that mechanism and nothing else.
    ///
    /// The failure this catches is a merge artifact rather than a typo: when a
    /// new treatment lands, every arm has to gain it, and an arm that misses
    /// the update silently differs from `live` on two axes instead of one. The
    /// arms are derived from `EVAL_ONLY_AIS`, so a newly added arm is covered
    /// here without touching this test.
    #[test]
    fn each_live_without_arm_holds_exactly_one_treatment_off() {
        let all: BTreeSet<&str> = LIVE_BRIDGE_TREATMENTS.iter().copied().collect();
        assert_eq!(
            all.len(),
            LIVE_BRIDGE_TREATMENTS.len(),
            "LIVE_BRIDGE_TREATMENTS contains a duplicate tag"
        );

        let arms: Vec<&str> = EVAL_ONLY_AIS
            .iter()
            .copied()
            .filter(|name| name.starts_with("live_without_"))
            .collect();
        assert!(!arms.is_empty(), "no live_without_* arms found");

        let mut held_off = BTreeSet::new();
        for name in arms {
            let kind = ArmKind::from_name(name).expect("live_without_* arm is a known ArmKind");
            let have: BTreeSet<&str> = kind.treatments().iter().copied().collect();
            let missing: Vec<&str> = all.difference(&have).copied().collect();
            assert!(
                have.is_subset(&all),
                "{name} carries a tag that is not a live-bridge treatment: {:?}",
                have.difference(&all).collect::<Vec<_>>()
            );
            assert_eq!(
                missing.len(),
                1,
                "{name} must hold exactly one treatment off, but differs from `live` on {:?}",
                missing
            );
            assert!(
                held_off.insert(missing[0]),
                "{name} holds off {}, which another arm already holds off",
                missing[0]
            );
        }

        let amenity_control = ArmKind::from_name("live_without_amenity_project_preemption")
            .expect("amenity control is a known ArmKind");
        let have: BTreeSet<&str> = amenity_control.treatments().iter().copied().collect();
        let expected: BTreeSet<&str> = all
            .iter()
            .copied()
            .filter(|tag| *tag != "amenity-project-preemption")
            .collect();
        assert_eq!(
            have, expected,
            "the amenity control must hold amenity-project-preemption, not a later bridge tag"
        );
        assert_eq!(
            held_off, all,
            "every deployed live treatment needs exactly one live_without_* arm; missing controls: {:?}",
            all.difference(&held_off).collect::<Vec<_>>()
        );
    }

    /// The reactor experiment pins generation 14 inside its dedicated runner.
    /// A public factory once reconstructed this name with default weights,
    /// silently changing the controller under treatment.
    #[test]
    fn reactor_marginal_treatment_stays_private_to_its_pinned_runner() {
        const NAME: &str = "advanced_reactor_marginal";
        assert!(!BUILTIN_AIS.contains(&NAME));
        assert!(!EVAL_ONLY_AIS.contains(&NAME));
        assert_eq!(builtin_provenance(NAME, "unused").effective, "basic");
    }

    #[test]
    fn static_doctrine_challengers_construct_searching_agents() {
        for name in [
            "strategic_deep_expand",
            "strategic_deep_consolidate",
            "strategic_deep_militarize",
        ] {
            let ai = builtin_ai(name, 1);
            assert_eq!(ai.review_census(), Some(Default::default()), "{name}");
        }
    }

    #[test]
    fn joint_axis_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_joint", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_joint", "unused");
        assert_eq!(provenance.effective, "strategic_joint");
        assert!(!provenance.degraded());
    }

    #[test]
    fn terminal_tempo_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_deep_tempo", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_tempo", "unused");
        assert_eq!(provenance.effective, "strategic_deep_tempo");
        assert!(!provenance.degraded());
    }

    #[test]
    fn ultra_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_ultra", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_ultra", "unused");
        assert_eq!(provenance.effective, "strategic_ultra");
        assert!(!provenance.degraded());
    }

    #[test]
    fn deep_default_control_refuses_the_champion_artifact() {
        let ai = builtin_ai("strategic_deep_default", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_default", "unused");
        assert_eq!(provenance.effective, "strategic_deep_default");
        assert!(!provenance.degraded());
        assert!(
            provenance
                .artifacts
                .iter()
                .all(|artifact| artifact.file != CHAMPION_FILE),
            "the control must never resolve best.json"
        );
    }

    #[test]
    fn religious_conversion_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_deep_conversion", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_conversion", "unused");
        assert_eq!(provenance.effective, "strategic_deep_conversion");
        assert!(!provenance.degraded());
    }

    #[test]
    fn religious_checkmate_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_deep_checkmate", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_checkmate", "unused");
        assert_eq!(provenance.effective, "strategic_deep_checkmate");
        assert!(!provenance.degraded());
    }

    #[test]
    fn league_genome_challenger_loads_a_win_selected_searching_agent() {
        let (name, _) = league_generalist().expect("committed league has a generalist genome");
        assert_eq!(name, "g4-10", "update the documented transfer candidate");
        let ai = builtin_ai("strategic_deep_league", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_league", "unused");
        assert_eq!(provenance.effective, "strategic_deep_league");
        assert!(!provenance.degraded());
        assert!(!provenance.untrained());
    }

    fn player(name: &str, leader: &str, civ: &str, score: i64, won: bool) -> RatedPlayer {
        RatedPlayer::new(name, leader, civ, score, won)
    }

    #[test]
    fn win_shares_are_a_distribution_over_the_table() {
        let table = [1914.0, 1865.0, 1836.0, 1847.0, 1766.0, 1755.0];
        let shares = win_shares(&table);
        assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(shares[0] > shares[5]);
        let pair = win_shares(&[1600.0, 1400.0]);
        assert!((pair[0] - expected(1600.0, 1400.0)).abs() < 1e-12);
        let wide = win_shares(&[40_000.0, 0.0]);
        assert!((wide[0] + wide[1] - 1.0).abs() < 1e-9 && wide[0] > 0.999);
    }

    #[test]
    fn direct_pair_score_interval_is_bounded_and_symmetric() {
        let (low, high) = wilson_interval(31.0, 40);
        let (reverse_low, reverse_high) = wilson_interval(9.0, 40);
        assert!(low < 31.0 / 40.0 && 31.0 / 40.0 < high);
        assert!((low - (1.0 - reverse_high)).abs() < 1e-12);
        assert!((high - (1.0 - reverse_low)).abs() < 1e-12);
        assert_eq!(wilson_interval(0.0, 0), (0.0, 1.0));
        assert_eq!(performance_elo(1500.0, 0.5), 1500.0);
        assert!(performance_elo(1500.0, 0.0).is_infinite());
        assert!(performance_elo(1500.0, 1.0).is_infinite());
        assert!(
            (performance_elo(1500.0, low) + performance_elo(1500.0, reverse_high)
                - 3000.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn result_updates_player_leader_civilization_rows() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("TechPriest", "Trajan", "Rome", 200, true),
                player("LabRat", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        let rome = &pool.ratings[&RatingKey::new("TechPriest", "Trajan", "Rome")];
        let egypt = &pool.ratings[&RatingKey::new("LabRat", "Cleopatra", "Egypt")];
        assert_eq!(rome.elo, 1012.0);
        assert_eq!(egypt.elo, 988.0);
        assert_eq!((rome.games, rome.wins), (1, 1));
        assert_eq!(pool.overall["TechPriest"], *rome);
        assert_eq!(pool.overall["LabRat"], *egypt);
    }

    #[test]
    fn immutable_control_pins_the_longitudinal_scale() {
        let mut cfg = TourneyCfg {
            players_per_game: 2,
            rating_anchor: Some("Control".to_string()),
            ..TourneyCfg::default()
        };
        cfg.num_city_states = 0;
        let mut anchored = EloPool::with_base(1500.0);
        anchored
            .bind_profile(TournamentProfile::from_cfg(&cfg))
            .unwrap();
        anchored.record_game(
            &[
                player("Challenger", "Trajan", "Rome", 200, true),
                player("Control", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );

        assert!((anchored.overall["Control"].elo - 1500.0).abs() < 1e-12);
        assert!((anchored.overall["Challenger"].elo - 1524.0).abs() < 1e-12);
        assert!(anchored
            .ratings
            .values()
            .any(|rating| (rating.elo - 1524.0).abs() < 1e-12));

        // Evidence about the fixed control translates every older row. It
        // cannot move the anchor or inflate only the newest generation.
        anchored.record_game(
            &[
                player("Control", "Cleopatra", "Egypt", 200, true),
                player("Novice", "Pericles", "Greece", 100, false),
            ],
            24.0,
        );
        assert!((anchored.overall["Control"].elo - 1500.0).abs() < 1e-12);
        assert!((anchored.overall["Challenger"].elo - 1512.0).abs() < 1e-12);
        assert!((anchored.overall["Novice"].elo - 1476.0).abs() < 1e-12);

        let direct = direct_anchor_performance(&anchored, "Control");
        let challenger = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "Challenger")
            .unwrap();
        let novice = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "Novice")
            .unwrap();
        assert_eq!((challenger.2, challenger.3), (1.0, 1));
        assert_eq!((novice.2, novice.3), (0.0, 1));
        assert!(challenger.1 > 1500.0 && novice.1 < 1500.0);

        let complete_report = leaderboard(&anchored);
        assert!(complete_report.contains("Standardized direct performance Elo"));
        let mut migrated = anchored.clone();
        migrated.history_complete = false;
        let migrated_report = leaderboard(&migrated);
        assert!(migrated_report.contains(
            "Post-migration direct performance (retained raw games only; legacy prior excluded)"
        ));
        assert!(!migrated_report.contains("Standardized direct performance Elo"));
    }

    #[test]
    fn overall_rating_accumulates_across_civilizations() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "France", 100, false),
                player("Bob", "Cleopatra", "Egypt", 200, true),
            ],
            24.0,
        );

        let alice = &pool.overall["Alice"];
        assert_eq!((alice.games, alice.wins), (2, 1));
        assert!(alice.elo < 1000.0, "the upset must erase more than the first win added");
        assert_eq!(
            alice.elo,
            pool.ratings[&RatingKey::new("Alice", "Eleanor", "France")].elo
        );
        assert_eq!(
            pool.ratings[&RatingKey::new("Alice", "Trajan", "Rome")].games,
            1
        );
    }

    #[test]
    fn a_new_combination_inherits_its_players_global_rating() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "France", 200, true),
                player("Bob", "Pericles", "Greece", 100, false),
            ],
            0.0,
        );

        assert_eq!(
            pool.ratings[&RatingKey::new("Alice", "Eleanor", "France")].elo,
            1012.0
        );
        assert_eq!(
            pool.ratings[&RatingKey::new("Bob", "Pericles", "Greece")].elo,
            988.0
        );
    }

    #[test]
    fn cloned_seats_count_as_one_overall_game_and_one_player_pair() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 400, true),
                player("Alice", "Pericles", "Greece", 300, false),
                player("Bob", "Cleopatra", "Egypt", 200, false),
                player("Bob", "Qin Shi Huang", "China", 100, false),
            ],
            24.0,
        );

        assert_eq!(pool.overall["Alice"].elo, 1012.0);
        assert_eq!(pool.overall["Bob"].elo, 988.0);
        assert_eq!((pool.overall["Alice"].games, pool.overall["Alice"].wins), (1, 1));
        assert_eq!((pool.overall["Bob"].games, pool.overall["Bob"].wins), (1, 0));
    }

    #[test]
    fn a_ledger_rejects_a_different_tournament_profile() {
        let cfg = TourneyCfg::default();
        let original = TournamentProfile::from_cfg(&cfg);
        let mut changed_cfg = TourneyCfg::default();
        changed_cfg.width += 2;
        let changed = TournamentProfile::from_cfg(&changed_cfg);
        let mut pool = EloPool::with_base(1000.0);

        pool.bind_profile(original.clone()).unwrap();
        let error = pool.bind_profile(changed).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("rating profile mismatch"));
        assert_eq!(pool.profile, Some(original));

        let mut controller_changed = pool.profile.clone().unwrap();
        controller_changed.controller_roster =
            ["advanced", "advanced_v1", "basic", "strategic"]
                .into_iter()
                .map(str::to_string)
                .collect();
        let error = pool.bind_profile(controller_changed).unwrap_err();
        assert!(error.to_string().contains("rating profile mismatch"));

        let mut rules_changed = pool.profile.clone().unwrap();
        rules_changed.rules_fingerprint = "fnv1a64:0000000000000000".to_string();
        let error = pool.bind_profile(rules_changed).unwrap_err();
        assert!(error.to_string().contains("rating profile mismatch"));

        let mut setup_changed = pool.profile.clone().unwrap();
        setup_changed.setup_contract = "difficulty=deity".to_string();
        let error = pool.bind_profile(setup_changed).unwrap_err();
        assert!(error.to_string().contains("rating profile mismatch"));
    }

    /// An arena's economy and its era choice are part of the experiment, so
    /// the rating profile carries both — and a world's profile carries
    /// neither, so every Civ ledger written before the arena had an economy
    /// still matches its own profile rather than being refused.
    #[test]
    fn an_arena_profile_records_the_economy_and_the_era_choice() {
        let world = TourneyCfg::default();
        assert!(
            !super::tournament_setup_contract(&world).contains("arena="),
            "a world has no arena to describe"
        );
        assert!(super::tournament_setup_contract(&world).contains("era=0"));

        let arena = TourneyCfg {
            map_script: MapScript::Battlefield,
            ..TourneyCfg::default()
        };
        let stock = super::tournament_setup_contract(&arena);
        assert!(
            stock.contains("arena=cities:1,production:0,gold:0,turns-per-tech:5"),
            "the arena grants belong in the profile: {stock}"
        );
        // The stock grants moved (30/30 → 0/0 on 2026-08-15), and the ledger
        // written under the old grants must stay matched to its own arena
        // rather than being read as the new one — which is what carrying the
        // grants in the profile is for.
        let reinforced = TourneyCfg {
            map_script: MapScript::Battlefield,
            tactics: crate::setup::TacticsRules {
                production: 30,
                gold: 30,
                ..crate::setup::TacticsRules::default()
            },
            ..TourneyCfg::default()
        };
        let reinforced_contract = super::tournament_setup_contract(&reinforced);
        assert!(
            reinforced_contract.contains("arena=cities:1,production:30,gold:30,turns-per-tech:5"),
            "{reinforced_contract}"
        );
        assert_ne!(stock, reinforced_contract);

        // The two settings that change what the battle *is* must not share a
        // ledger: one city is decided by taking it, none is an attrition duel.
        let duel = TourneyCfg {
            map_script: MapScript::Battlefield,
            tactics: crate::setup::TacticsRules {
                cities: 0,
                ..crate::setup::TacticsRules::default()
            },
            ..TourneyCfg::default()
        };
        assert_ne!(stock, super::tournament_setup_contract(&duel));

        // The flag objective is a third game again — a race, not a siege or
        // a duel — so it splits the ledger too, and only by *adding* to the
        // profile, so every arena ledger written before the shape existed
        // still matches its own.
        let flagged = TourneyCfg {
            map_script: MapScript::Battlefield,
            tactics: crate::setup::TacticsRules {
                flag: true,
                ..crate::setup::TacticsRules::default()
            }
            .sanitized(),
            ..TourneyCfg::default()
        };
        let race = super::tournament_setup_contract(&flagged);
        assert!(race.contains("objective:flag"), "{race}");
        assert!(!stock.contains("objective:flag"), "{stock}");

        let spread = TourneyCfg {
            map_script: MapScript::Battlefield,
            start_era: crate::setup::StartEraChoice::RandomPerGame,
            ..TourneyCfg::default()
        };
        let spread_contract = super::tournament_setup_contract(&spread);
        assert!(spread_contract.contains("era=random"), "{spread_contract}");
        assert_ne!(stock, spread_contract);
    }

    /// A drawn arena battle ends the tournament game rather than hanging it.
    /// The stock arena grants no reinforcements, so a battle that reaches
    /// its clock with both armies standing is ordinary — and it is a
    /// terminal draw with no winner, which the game loop has to recognise as
    /// finished. A twelve-turn clock on the bounded field is a certain draw:
    /// two Basic companies cannot close and eliminate each other in twelve
    /// turns. Both games must complete, be rated as games nobody won, and
    /// leave the pool with the right count.
    #[test]
    fn a_drawn_arena_battle_ends_the_tournament_game() {
        let cfg = TourneyCfg {
            games: 2,
            players_per_game: 2,
            width: 20,
            height: 20,
            map_script: MapScript::Battlefield,
            max_turns: 12,
            num_city_states: 0,
            verbose: false,
            jobs: 1,
            ..TourneyCfg::default()
        };
        let names = vec!["basic".to_string(), "basic_b".to_string()];
        let pool = super::run_tournament(
            &names,
            |_, seed| builtin_ai("basic", seed),
            &cfg,
        );
        assert_eq!(pool.history.len(), 2, "both drawn battles were rated");
        for game in &pool.history {
            assert!(
                game.players.iter().all(|player| !player.won),
                "a drawn arena battle has no winner: {:?}",
                game.players
            );
        }
        for name in &names {
            assert_eq!(pool.overall[name].games, 2, "{name} played both battles");
            assert_eq!(pool.overall[name].wins, 0, "{name} won neither");
        }
    }

    /// The era choice resolves per game, so a random-era ladder fights a
    /// spread rather than one era, and replays it exactly.
    #[test]
    fn a_random_era_choice_is_per_game_and_reproducible() {
        use crate::setup::StartEraChoice;
        let fixed = StartEraChoice::Fixed(3);
        assert_eq!(fixed.for_seed(1), 3);
        assert_eq!(fixed.for_seed(999), 3);

        let rolled: Vec<usize> = (0..48).map(|seed| StartEraChoice::RandomPerGame.for_seed(seed)).collect();
        let replay: Vec<usize> = (0..48).map(|seed| StartEraChoice::RandomPerGame.for_seed(seed)).collect();
        assert_eq!(rolled, replay, "the same seed must replay the same era");
        let distinct: std::collections::BTreeSet<usize> = rolled.iter().copied().collect();
        assert!(distinct.len() > 1, "a random era choice must actually vary: {distinct:?}");
    }

    #[test]
    fn persistent_elo_pins_every_implicit_lobby_default() {
        assert_eq!(
            super::tournament_setup_contract(&TourneyCfg::default()),
            super::SCHEMA3_LEGACY_SETUP_CONTRACT
        );
    }

    #[test]
    fn old_schema_three_profiles_migrate_to_the_historical_lobby() {
        let mut encoded =
            serde_json::to_value(TournamentProfile::from_cfg(&TourneyCfg::default())).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .remove("setup_contract");

        let migrated: TournamentProfile = serde_json::from_value(encoded).unwrap();

        assert_eq!(
            migrated.setup_contract,
            super::SCHEMA3_LEGACY_SETUP_CONTRACT
        );
    }

    #[test]
    fn persistent_ratings_reject_cloned_or_duplicate_entrants() {
        let cfg = TourneyCfg::default();
        let make = |_: &str, _: u64| {
            Box::new(crate::ai::BasicAi::new()) as Box<dyn crate::ai::Ai>
        };
        let too_few = vec!["advanced".to_string(), "basic".to_string()];
        let error = super::run_persistent_tournament(
            &too_few,
            make,
            &cfg,
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error.to_string().contains("cloned seats change the contest"));

        let duplicate = vec![
            "advanced".to_string(),
            "basic".to_string(),
            "basic".to_string(),
            "random".to_string(),
        ];
        let error = super::run_persistent_tournament(
            &duplicate,
            make,
            &cfg,
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error.to_string().contains("must be unique"));

        let anchored_cfg = TourneyCfg {
            rating_anchor: Some("missing-control".to_string()),
            ..TourneyCfg::default()
        };
        let distinct = vec![
            "advanced".to_string(),
            "advanced_v1".to_string(),
            "basic".to_string(),
            "random".to_string(),
        ];
        let error = super::run_persistent_tournament(
            &distinct,
            make,
            &anchored_cfg,
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error.to_string().contains("must be one of the tournament entrants"));

        let error = super::run_persistent_tournament(
            &distinct,
            make,
            &TourneyCfg::default(),
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("one ordered controller role per entrant"));
    }

    #[test]
    fn score_ties_are_draws_and_still_count_as_games() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 150, false),
                player("Bob", "Cleopatra", "Egypt", 150, false),
            ],
            24.0,
        );
        for rating in pool.ratings.values() {
            assert_eq!(rating.elo, 1000.0);
            assert_eq!(rating.games, 1);
            assert_eq!(rating.wins, 0);
        }
    }

    #[test]
    fn a_player_has_independent_ratings_for_different_leaders() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "England", 100, false),
                player("Bob", "Cleopatra", "Egypt", 200, true),
            ],
            24.0,
        );
        let trajan = &pool.ratings[&RatingKey::new("Alice", "Trajan", "Rome")];
        let eleanor = &pool.ratings[&RatingKey::new("Alice", "Eleanor", "England")];
        assert_eq!(trajan.games, 1);
        assert_eq!(eleanor.games, 1);
        assert!(trajan.elo > 1000.0);
        assert!(eleanor.elo < 1000.0);
    }

    #[test]
    fn declared_winner_outranks_a_higher_score() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 80, true),
                player("Bob", "Cleopatra", "Egypt", 200, false),
            ],
            24.0,
        );
        assert!(pool.ratings[&RatingKey::new("Alice", "Trajan", "Rome")].elo > 1000.0);
    }

    #[test]
    fn eleanor_leading_two_civilizations_has_two_ratings() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Eleanor", "England", 200, true),
                player("Bob", "Victoria", "England", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "France", 100, false),
                player("Bob", "Catherine de Medici", "France", 200, true),
            ],
            24.0,
        );
        assert!(pool
            .ratings
            .contains_key(&RatingKey::new("Alice", "Eleanor", "England")));
        assert!(pool
            .ratings
            .contains_key(&RatingKey::new("Alice", "Eleanor", "France")));
        assert!(pool.ratings[&RatingKey::new("Alice", "Eleanor", "England")].elo > 1000.0);
        assert!(pool.ratings[&RatingKey::new("Alice", "Eleanor", "France")].elo < 1000.0);
    }

    #[test]
    fn one_player_cannot_rate_their_leaders_against_each_other() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Eleanor", "England", 200, true),
                player("Alice", "Eleanor", "France", 100, false),
            ],
            24.0,
        );
        assert!(pool.ratings.values().all(|rating| rating.elo == 1000.0));
        assert!(pool.ratings.values().all(|rating| rating.games == 1));
    }

    #[test]
    fn round_robin_scheduler_balances_every_entrant_across_civilization_seats() {
        let names: Vec<String> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let mut rng = Rng::new(9);
        let (order, stride) = seat_schedule(&names, 4, &mut rng);
        let mut appearances = BTreeMap::<String, u32>::new();
        let mut by_seat = vec![BTreeMap::<String, u32>::new(); 4];
        for game in 0..25 {
            let seats = scheduled_seats(&names, 4, game, &order, stride);
            assert_eq!(
                seats
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                4
            );
            for (seat, entrant) in seats.into_iter().enumerate() {
                *appearances.entry(entrant.clone()).or_insert(0) += 1;
                *by_seat[seat].entry(entrant).or_insert(0) += 1;
            }
        }
        assert_eq!(appearances.values().sum::<u32>(), 100);
        assert!(appearances.values().all(|count| *count == 20));
        for seat in by_seat {
            assert_eq!(seat.len(), names.len());
            assert!(seat.values().all(|count| *count == 5));
        }
    }

    #[test]
    fn ledger_round_trips_structured_keys() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("civvis-elo-{}-{nonce}", std::process::id()));
        let path = dir.join("ratings.json");
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("TechPriest", "Trajan", "Rome", 2, true),
                player("CultureVulture", "Cleopatra", "Egypt", 1, false),
            ],
            24.0,
        );
        pool.save(&path).unwrap();
        assert_eq!(EloPool::load(&path).unwrap(), pool);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(&format!("\"schema_version\": {ELO_SCHEMA_VERSION}")));
        assert!(raw.contains("\"civilization\": \"Rome\""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn keyed_games_are_idempotent_and_independent_of_lock_order() {
        let first_game = [
            player("Alice", "Trajan", "Rome", 200, true),
            player("Bob", "Cleopatra", "Egypt", 100, false),
        ];
        let second_game = [
            player("Alice", "Eleanor", "France", 100, false),
            player("Bob", "Pericles", "Greece", 200, true),
        ];
        let mut reverse_arrival = EloPool::with_base(1500.0);
        reverse_arrival
            .record_game_once("event-b", &second_game, 24.0)
            .unwrap();
        reverse_arrival
            .record_game_once("event-a", &first_game, 24.0)
            .unwrap();
        let mut forward_arrival = EloPool::with_base(1500.0);
        forward_arrival
            .record_game_once("event-a", &first_game, 24.0)
            .unwrap();
        forward_arrival
            .record_game_once("event-b", &second_game, 24.0)
            .unwrap();

        assert_eq!(reverse_arrival, forward_arrival);
        assert!(!forward_arrival
            .record_game_once("event-a", &first_game, 24.0)
            .unwrap());
        let mut changed = first_game.to_vec();
        changed[0].score += 1;
        let error = forward_arrival
            .record_game_once("event-a", &changed, 24.0)
            .unwrap_err();
        assert!(error.to_string().contains("different results"));
    }

    #[test]
    fn keyed_games_refuse_cloned_identities_and_profile_shape_drift() {
        let clones = [
            player("Alice", "Trajan", "Rome", 200, true),
            player("Alice", "Pericles", "Greece", 100, false),
        ];
        let mut unbound = EloPool::with_base(1500.0);
        let error = unbound
            .record_game_once("cloned-table", &clones, 24.0)
            .unwrap_err();
        assert!(error.to_string().contains("identities must be distinct"));

        let cfg = TourneyCfg::default();
        let mut profiled = EloPool::with_base(1500.0);
        profiled
            .bind_profile(TournamentProfile::from_cfg(&cfg))
            .unwrap();
        let duel = [
            player("Alice", "Trajan", "Rome", 200, true),
            player("Bob", "Cleopatra", "Egypt", 100, false),
        ];
        let error = profiled
            .record_game_once("wrong-table-size", &duel, 24.0)
            .unwrap_err();
        assert!(error.to_string().contains("match the profile"));
    }

    #[test]
    fn complete_history_detects_a_tampered_aggregate() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "civvis-elo-tamper-{}-{nonce}",
            std::process::id()
        ));
        let path = dir.join("ratings.json");
        let mut pool = EloPool::with_base(1500.0);
        pool.record_game_once(
            "event-a",
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        )
        .unwrap();
        pool.save(&path).unwrap();

        let mut stored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let elo = stored["players"][0]["elo"].as_f64().unwrap();
        stored["players"][0]["elo"] = serde_json::Value::from(elo + 5.0);
        fs::write(&path, serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
        let error = EloPool::load(&path).unwrap_err();
        assert!(error
            .to_string()
            .contains("aggregates do not match raw game evidence"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn historical_protocol_v1_ledger_is_preserved() {
        let pool = EloPool::load(HISTORICAL_V1_RATINGS_PATH).unwrap();
        let expected_cfg = TourneyCfg {
            rating_anchor: Some("advanced_v1".to_string()),
            controller_roster: ["advanced", "advanced_v1", "basic", "random"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ..TourneyCfg::default()
        };
        assert_eq!(pool.base_rating, ELO_BASE_RATING);
        let mut historical_profile = TournamentProfile::from_cfg(&expected_cfg);
        historical_profile.protocol_version = 1;
        historical_profile.rules_fingerprint = "fnv1a64:3423bd46da2b8cd7".to_string();
        assert_eq!(
            pool.profile,
            Some(historical_profile)
        );
        assert!(pool.history_complete);
        assert_eq!(pool.history.len(), 40);
        assert_eq!(
            pool.history.len(),
            pool.overall
                .values()
                .map(|rating| rating.games)
                .max()
                .unwrap_or(0) as usize
        );
        assert_eq!(
            pool.overall.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "advanced-20260730",
                "advanced_v1",
                "basic-20260730",
                "random-20260730",
            ]
        );
        assert_eq!(pool.ratings.len(), 16);

        let direct = direct_anchor_performance(&pool, "advanced_v1");
        let advanced = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "advanced-20260730")
            .unwrap();
        assert_eq!((advanced.2, advanced.3), (31.0, 40));
        assert!((advanced.1 - 1708.2).abs() < 0.1);
        assert!((100.0 * advanced.4 - 62.5).abs() < 0.1);
        assert!((100.0 * advanced.5 - 87.7).abs() < 0.1);
    }

    #[test]
    fn historical_protocol_v2_ledger_is_preserved() {
        let pool = EloPool::load(HISTORICAL_V2_RATINGS_PATH).unwrap();
        let expected_cfg = TourneyCfg {
            rating_anchor: Some("advanced_v1".to_string()),
            controller_roster: ["advanced", "advanced_v1", "basic", "random"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ..TourneyCfg::default()
        };
        assert_eq!(pool.base_rating, ELO_BASE_RATING);
        let mut historical_profile = TournamentProfile::from_cfg(&expected_cfg);
        historical_profile.protocol_version = 2;
        historical_profile.rules_fingerprint = "fnv1a64:3423bd46da2b8cd7".to_string();
        assert_eq!(
            pool.profile,
            Some(historical_profile)
        );
        assert!(pool.history_complete);
        assert_eq!(pool.history.len(), 40);
        assert!(pool.history.iter().all(|game| {
            game.id
                .as_deref()
                .is_some_and(|id| id.starts_with("v2:"))
        }));
        assert_eq!(
            pool.history.len(),
            pool.overall
                .values()
                .map(|rating| rating.games)
                .max()
                .unwrap_or(0) as usize
        );
        assert_eq!(
            pool.overall.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "advanced-20260731",
                "advanced_v1",
                "basic-20260730",
                "random-20260730",
            ]
        );
        assert_eq!(pool.ratings.len(), 16);

        let direct = direct_anchor_performance(&pool, "advanced_v1");
        let advanced = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "advanced-20260731")
            .unwrap();
        assert_eq!((advanced.2, advanced.3), (27.0, 40));
        assert!((advanced.1 - 1623.6).abs() < 0.1);
        assert!((100.0 * advanced.4 - 52.0).abs() < 0.1);
        assert!((100.0 * advanced.5 - 79.9).abs() < 0.1);
    }

    #[test]
    fn historical_protocol_v3_ledger_is_preserved() {
        let pool = EloPool::load(HISTORICAL_V3_RATINGS_PATH).unwrap();
        let expected_cfg = TourneyCfg {
            rating_anchor: Some("advanced_v1".to_string()),
            controller_roster: ["advanced", "advanced_v1", "basic", "random"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ..TourneyCfg::default()
        };
        assert_eq!(pool.base_rating, ELO_BASE_RATING);
        let mut historical_profile = TournamentProfile::from_cfg(&expected_cfg);
        historical_profile.protocol_version = 3;
        historical_profile.rules_fingerprint = "fnv1a64:3423bd46da2b8cd7".to_string();
        assert_eq!(pool.profile, Some(historical_profile));
        assert!(pool.history_complete);
        assert_eq!(pool.history.len(), 40);
        assert!(pool.history.iter().all(|game| {
            game.id
                .as_deref()
                .is_some_and(|id| id.starts_with("v3:"))
        }));
        assert_eq!(
            pool.history.len(),
            pool.overall
                .values()
                .map(|rating| rating.games)
                .max()
                .unwrap_or(0) as usize
        );
        assert_eq!(
            pool.overall.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "advanced-20260731-settlement",
                "advanced_v1",
                "basic-20260731-settlement",
                "random-20260730",
            ]
        );
        assert_eq!(pool.ratings.len(), 16);

        let direct = direct_anchor_performance(&pool, "advanced_v1");
        let advanced = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "advanced-20260731-settlement")
            .unwrap();
        assert_eq!((advanced.2, advanced.3), (28.0, 40));
        assert!((advanced.1 - 1643.2).abs() < 0.1);
        assert!((100.0 * advanced.4 - 54.6).abs() < 0.1);
        assert!((100.0 * advanced.5 - 81.9).abs() < 0.1);
    }

    #[test]
    fn shipped_protocol_v4_ledger_is_a_canonical_fresh_baseline() {
        let pool = EloPool::load(DEFAULT_RATINGS_PATH).unwrap();
        let expected_cfg = TourneyCfg {
            rating_anchor: Some("advanced_v1".to_string()),
            controller_roster: ["advanced", "advanced_v1", "basic", "random"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ..TourneyCfg::default()
        };
        assert_eq!(pool.base_rating, ELO_BASE_RATING);
        let mut historical_profile = TournamentProfile::from_cfg(&expected_cfg);
        // The checked-in v4 games predate the Firaxis-exact unique-unit and
        // improvement rows. Keep their measured rules binding honest instead of
        // relabeling old evidence with the current rules fingerprint.
        historical_profile.rules_fingerprint = "fnv1a64:3423bd46da2b8cd7".to_string();
        // ⚠ Same reasoning, same rule, for the protocol. These 40 games were PLAYED
        // under protocol 4; the pantheon price bump made 5 current. Letting
        // `from_cfg` stamp them 5 would relabel old evidence as having been
        // measured under rules it never saw — which is the precise thing the line
        // above exists to prevent. The ledger is a record, not a live rating.
        historical_profile.protocol_version = 4;
        assert_eq!(pool.profile, Some(historical_profile));
        assert!(pool.history_complete);
        assert_eq!(pool.history.len(), 40);
        assert!(pool.history.iter().all(|game| {
            game.id
                .as_deref()
                .is_some_and(|id| id.starts_with("v4:"))
        }));
        assert_eq!(
            pool.history.len(),
            pool.overall
                .values()
                .map(|rating| rating.games)
                .max()
                .unwrap_or(0) as usize
        );
        assert_eq!(
            pool.overall.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "advanced-20260801-diplomacy",
                "advanced_v1",
                "basic-20260801-diplomacy",
                "random-20260730",
            ]
        );
        assert_eq!(pool.ratings.len(), 16);

        let direct = direct_anchor_performance(&pool, "advanced_v1");
        let advanced = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "advanced-20260801-diplomacy")
            .unwrap();
        assert_eq!((advanced.2, advanced.3), (29.0, 40));
        assert!((advanced.1 - 1663.6).abs() < 0.1);
        assert!((100.0 * advanced.4 - 57.2).abs() < 0.1);
        assert!((100.0 * advanced.5 - 83.9).abs() < 0.1);
    }

    #[test]
    fn schema_one_rows_migrate_to_player_leader_civilization() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("civvis-elo-migrate-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ratings.json");
        fs::write(
            &path,
            r#"{"schema_version":1,"base_rating":1000.0,"ratings":[{"civilization":"Rome","strategy":"science","elo":1111.0,"games":3,"wins":2,"agents":["advanced"]}]}"#,
        )
        .unwrap();
        let pool = EloPool::load(&path).unwrap();
        let rating = &pool.ratings[&RatingKey::new("advanced", "Trajan", "Rome")];
        assert_eq!((rating.elo, rating.games, rating.wins), (1111.0, 3, 2));
        assert_eq!(pool.overall["advanced"].elo, rating.elo);
        assert_eq!((pool.overall["advanced"].games, pool.overall["advanced"].wins), (0, 0));
        assert!(!pool.history_complete);
        pool.save(&path).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(&format!("\"schema_version\": {ELO_SCHEMA_VERSION}")));
        assert!(raw.contains("\"players\""));
        assert!(!raw.contains("\"strategy\""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn locked_ledger_updates_from_concurrent_workers_are_merged() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "civvis-elo-concurrent-{}-{nonce}",
            std::process::id()
        ));
        let path = dir.join("ratings.json");
        let barrier = Arc::new(Barrier::new(2));
        let workers: Vec<_> = [
            (
                "event-b",
                player("TechPriest", "Trajan", "Rome", 2, true),
                player("LabRat", "Cleopatra", "Egypt", 1, false),
            ),
            (
                "event-a",
                player("CultureVulture", "Pericles", "Greece", 2, true),
                player("OperaGhost", "Qin Shi Huang", "China", 1, false),
            ),
        ]
        .into_iter()
        .map(|results| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                super::update_ledger(&path, |pool| {
                    pool.record_game_once(results.0, &[results.1, results.2], 24.0)
                        .unwrap();
                })
                .unwrap();
            })
        })
        .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let pool = EloPool::load(&path).unwrap();
        assert!(pool.history_complete);
        assert_eq!(
            pool.history
                .iter()
                .map(|game| game.id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["event-a", "event-b"]
        );
        assert_eq!(pool.ratings.len(), 4);
        assert_eq!(
            pool.ratings
                .values()
                .map(|rating| rating.games)
                .sum::<u32>(),
            4
        );
        assert!(!dir.join(".ratings.json.lock").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
