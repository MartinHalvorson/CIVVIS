//! ⭐ THE GENE REGISTRY — the **gene pool**: every gene there is, on or off,
//! declared once.
//!
//! Vocabulary (operator, 2026-08-23): the **gene pool** is the collection of
//! all genes, whether on or off — this table is it. A **genome** is one
//! player's set of on genes, a subset of the pool: the deployment genome is
//! the set the ledger ships on, and a screen seat draws its own.
//!
//! A gene is one boolean flag on `AdvancedAi` with a published tag. This table
//! is the one place a gene is declared — its tag, its flag, what kind of gene
//! it is, and the two toggles that set it — and everything that needs the
//! list reads it from here:
//!
//! - `AdvancedAi::enable_live_bridge_universe` turns on every `live` gene
//!   (what the Civilization VI seat ships), `enable_engine_repairs_universe`
//!   every `Repair` (the native half of that bundle), and the war/economy
//!   halves filter by `Axis`; `apply_gene_ledger` then sets each gene to the
//!   deployment default the ledger derived (`gene_ledger.rs`).
//! - `gene_screen` screens every `screenable()` gene; `civvis_orders --without`
//!   withholds any `live()` gene on the live seat.
//! - `tools/genes.py` (the ledger, the ranking) and
//!   `tools/eval_manifest.py` scrape the rows below, so a row added here is
//!   priced, ranked and counted without touching them.
//!
//! Until 2026-08-23 the same facts were spread over three tuple tables
//! (`LIVE_TREATMENTS`, `PRODUCTION_TREATMENTS`, `PRODUCTION_OPT_INS`) and
//! five tag lists in `src/elo.rs` (`LIVE_BRIDGE_TREATMENTS`,
//! `FIRAXIS_ONLY_TREATMENTS`, `ENGINE_REPAIR_*`), held in step by tests.
//! They are columns of one row now. The `treatment` vocabulary went with
//! them: a treatment was a gene seen from the evaluator's side, and the
//! evaluator is gone.
//!
//! ⚠ The tags are published: `docs/gene_ledger.json`, every
//! `docs/gene_screens/*.json` header and every round under `docs/eval/` file
//! results under them, so renaming a row silently unfinds a recorded result.
//! Add rows; do not rewrite them. A tag `<base>-<n>` (`war-economy-2`) is
//! version `n` of `<base>` — see `docs/GENE_SCREEN.md`, *Versioning a gene*.
//!
//! ⚠ Existing rows are never re-ordered: the screen writes genes in this
//! order, and `tools/genes.py` re-derives a batch's gene set from this file
//! at the commit the batch names. Add a row below the tail append marker for
//! its tag's first-letter range. Every marker follows the existing rows, so
//! the distinct append points let concurrent gene PRs merge without moving a
//! positional bit that a running screen already wrote.
//!
//! Every row's comment is the note its old table row or its old bundle line
//! carried — the reason the gene exists and the measurement that motivated
//! it — moved here with the row.

use super::AdvancedAi;
#[allow(unused_imports)]
use super::{GeneVerdict, Measure, Verdict};

/// Which half of the native repair bundle a `Repair` belongs to. Split so the
/// bundle's interaction stays measurable: if the whole bundle beats stock by
/// more than the halves do separately, the repairs compound.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    War,
    Economy,
}

/// What kind of gene a row is — the one fact the old tables encoded by which
/// table a row sat in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// An engine repair the Civilization VI seat ships and a native board can
    /// play: on in the genome's universe, screenable, withholdable live.
    Repair(Axis),
    /// Shipped by the Civilization VI seat but reading host state a native
    /// board does not have: inert in a headless game, so never screened;
    /// withholdable live.
    HostOnly,
    /// Production ships it on before the ledger says anything (the stock
    /// agent carries it).
    Production,
    /// Off everywhere until the ledger turns it on; screenable.
    OptIn,
}

/// One gene, declared once.
pub struct Gene {
    /// The published name: the ledger, the screen and every recorded round
    /// file results under it.
    pub tag: &'static str,
    /// The flag on `AdvancedAi` it sets.
    pub field: &'static str,
    pub kind: Kind,
    pub enable: fn(&mut AdvancedAi),
    pub disable: fn(&mut AdvancedAi),
}

impl Gene {
    /// Shipped by the Civilization VI seat (`enable_live_bridge_universe`).
    pub const fn live(&self) -> bool {
        matches!(self.kind, Kind::Repair(_) | Kind::HostOnly)
    }
    /// Reads host state a native board does not have.
    pub const fn host_only(&self) -> bool {
        matches!(self.kind, Kind::HostOnly)
    }
    /// A native engine repair in the live bundle.
    pub const fn repair(&self) -> bool {
        matches!(self.kind, Kind::Repair(_))
    }
    pub const fn repair_axis(&self) -> Option<Axis> {
        match self.kind {
            Kind::Repair(axis) => Some(axis),
            _ => None,
        }
    }
    /// Production ships it on before the ledger.
    pub const fn production(&self) -> bool {
        matches!(self.kind, Kind::Production)
    }
    /// Off until the ledger turns it on.
    pub const fn opt_in(&self) -> bool {
        matches!(self.kind, Kind::OptIn)
    }
    /// On after `enable_engine_repairs_universe`: the genome's universe.
    pub const fn universe_on(&self) -> bool {
        self.repair() || self.production()
    }
    /// On in the production agent alone, before any bundle.
    pub const fn stock_on(&self) -> bool {
        self.production()
    }
    /// Whether `gene_screen` can price it on a native board.
    pub const fn screenable(&self) -> bool {
        self.universe_on() || self.opt_in()
    }
}

/// The gene by its published tag.
pub fn gene(tag: &str) -> Option<&'static Gene> {
    GENES.iter().find(|gene| gene.tag == tag)
}

/// Every `live()` tag, in registry order — what the live bundle stamps on a
/// run so an old binary's list reads as shorter.
pub fn live_tags() -> Vec<&'static str> {
    GENES.iter().filter(|g| g.live()).map(|g| g.tag).collect()
}

/// Every `host_only()` tag, in registry order.
pub fn host_only_tags() -> Vec<&'static str> {
    GENES
        .iter()
        .filter(|g| g.host_only())
        .map(|g| g.tag)
        .collect()
}

/// Every `repair()` tag, in registry order — the native half of the bundle.
pub fn repair_tags() -> Vec<&'static str> {
    GENES.iter().filter(|g| g.repair()).map(|g| g.tag).collect()
}

/// The repairs of one half, in registry order.
pub fn repair_tags_on(axis: Axis) -> Vec<&'static str> {
    GENES
        .iter()
        .filter(|g| g.repair_axis() == Some(axis))
        .map(|g| g.tag)
        .collect()
}

/// Every `screenable()` gene, in registry order — the genome `gene_screen`
/// varies and `tools/genes.py` fingerprints.
pub fn screenable_genes() -> Vec<&'static Gene> {
    GENES.iter().filter(|g| g.screenable()).collect()
}

#[rustfmt::skip]
pub const GENES: &[Gene] = &[
    // ── Repairs: the native half of the live bundle, war then economy ──
    // ⚠ Reinforcements never reach the front. Force groups are cliques at
    // `command_radius`, so the trickle of fresh and released units forms
    // one- and two-body groups at home that can never clear
    // `LOCAL_SUPERIORITY_FLOOR` at the objective. Measured on run
    // `civvis-20260808T033223Z`, t217-t225: land forces of one, two and
    // three against the same objective, every one "too weak locally to
    // advance", while the empire fielded 10-14 units.
    // Force assembly and movement.
    Gene { tag: "war-reinforcement", field: "war_reinforcement", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_war_reinforcement, disable: AdvancedAi::disable_war_reinforcement },
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
    Gene { tag: "come-ashore", field: "come_ashore", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_come_ashore, disable: AdvancedAi::disable_come_ashore },
    Gene { tag: "recorded-tactical-step", field: "recorded_tactical_step", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_recorded_tactical_step, disable: AdvancedAi::disable_recorded_tactical_step },
    // And a three-hop loop back to the start is no better than a two-hop
    // one. See `whole_turn_backtrack_guard`.
    // And a three-hop loop back to the start is no better than a two-hop
    // one. See `whole_turn_backtrack_guard`.
    Gene { tag: "whole-turn-backtrack-guard", field: "whole_turn_backtrack_guard", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_whole_turn_backtrack_guard, disable: AdvancedAi::disable_whole_turn_backtrack_guard },
    // Reading the enemy. The `3.0` "we dominate here" sentinel fires on
    // 53.3% of force decisions, and two thirds of those are objectives
    // that are not cities.
    Gene { tag: "blind-objective-strength", field: "blind_objective_strength", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_blind_objective_strength, disable: AdvancedAi::disable_blind_objective_strength },
    Gene { tag: "blind-objective-units", field: "blind_objective_units", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_blind_objective_units, disable: AdvancedAi::disable_blind_objective_units },
    Gene { tag: "relief-targets-the-siege", field: "relief_targets_the_siege", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_relief_targets_the_siege, disable: AdvancedAi::disable_relief_targets_the_siege },
    // ⚠ `desired_military` is `2 * city_count` at war — a headcount keyed to
    // OUR empire that never asks how strong the rival is. Once it is met,
    // `force_gap` hits zero and the military arm of `production_value` drops
    // from a 4.0 multiplier to 0.65, so units lose to buildings. Measured on
    // run `civvis-20260803T005930Z`: 94 of 188 war turns had the target
    // already satisfied, CIVVIS ordered 17 military units in the whole war
    // (8 land combat, 2 siege) against Korea's five walled cities, and at t240
    // the rival fielded 1050 military against our 658 while the target still
    // read satisfied at 11 against a wanted 10.
    // Sizing the army against the rival rather than against our own city
    // count, before as well as during the war.
    Gene { tag: "army-target-weighs-enemy", field: "army_target_weighs_enemy", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_army_target_weighs_the_enemy, disable: AdvancedAi::disable_army_target_weighs_the_enemy },
    // ⚠ And the wartime repair above still wakes up only once the war has
    // started. Measured on run `civvis-20260803T220954Z` (Rome, 250 turns):
    // seven cities founded by t123, Mali declared at t157 holding **894
    // military against our 481**, and six of the seven were taken at
    // loyalty 100 — sieges, not revolts — including Rome itself at t225.
    // We issued zero war orders and sixteen refused peace requests. A
    // target that asks "who could kill me" only after the declaration is
    // asking too late; this floor asks it of the strongest MET major in
    // peacetime, under its own far smaller ceiling.
    Gene { tag: "peacetime-deterrence", field: "peacetime_deterrence", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_peacetime_deterrence, disable: AdvancedAi::disable_peacetime_deterrence },
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
    Gene { tag: "war-economy", field: "war_economy", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_war_economy, disable: AdvancedAi::disable_war_economy },
    Gene { tag: "bounded-recovery", field: "bounded_recovery", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_bounded_recovery, disable: AdvancedAi::disable_bounded_recovery },
    // ⚠ And the war it keeps prosecuting still has to end on a captured
    // city. The campaign re-picks its objective from scratch every turn and
    // prices fifteen turns of siege at ~37 points, less than the distance
    // terms swing; the army walks off a city at 25 hp with its walls down
    // and Civ 6 heals it back at 20 hp a turn. Live run
    // `civvis-20260808T142724Z` dealt 338 hp of city damage over t73-t105,
    // handed 200 of it back, and took nothing — the shape behind 25 live
    // games and 0 captures on 7.7x the field's military.
    Gene { tag: "siege-commitment", field: "siege_commitment", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_siege_commitment, disable: AdvancedAi::disable_siege_commitment },
    // ⚠ Barbarians are excluded from `at_major_war` by design, so every defensive
    // escalation in the production picker reads a barbarian siege as no threat at
    // all: a one-city empire's standing-army floor stays at `mil_per_city` (1.0)
    // and it cannot want a third defender while horsemen stand on its doorstep.
    // Measured on run `civvis-20260802T202501Z` — four settlers built into that
    // siege and captured, two on the capital tile without ever moving, one city
    // until t80, score 140 against a best rival's 416. The tournament controller
    // stays frozen so its recorded ladders remain comparable.
    // The other half of the same three-defeat measurement: the capital that
    // fell bleeding with an empty hostile list. See garrison_under_fire.
    Gene { tag: "garrison-under-fire", field: "garrison_under_fire", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_garrison_under_fire, disable: AdvancedAi::disable_garrison_under_fire },
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
    // Tactical quality on the tile the unit actually stands on.
    Gene { tag: "strike-opening", field: "strike_opening", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_strike_opening, disable: AdvancedAi::disable_strike_opening },
    // ⚠ The empire goes blind and the build order never notices. Recon is
    // not among the counts `pick_item` receives, and `OPENING_MENU` is the
    // only place a scout is named, so once the openers die nothing replaces
    // them. Live run `civvis-20260808T142724Z`: zero recon units from turn
    // ~100 to 251 while the army grew to 22, 77% of the map never seen, and
    // the eventual winner first met on turn 215 already holding 927 points.
    Gene { tag: "recon-replacement", field: "recon_replacement", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_recon_replacement, disable: AdvancedAi::disable_recon_replacement },
    // And a barbarian scout does not pin the opening. See
    // `barbarian_scouts_are_scouts`.
    // And a barbarian scout is a scout in both regimes — it can neither
    // attack nor capture, so nothing retreats from one. See
    // `barbarian_scouts_are_scouts`.
    Gene { tag: "barbarian-scouts-are-scouts", field: "barbarian_scouts_are_scouts", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_barbarian_scouts_are_scouts, disable: AdvancedAi::disable_barbarian_scouts_are_scouts },
    // And a raider is cheaper to kill than a major: over the repaired
    // bridge our 225 melee attacks killed 119 and cost 6 attackers, while
    // the barbarians attacked us 867 times to our 290. See
    // `BasicAi::barbarian_bargain`.
    // And a raider is cheaper to kill than a major: 225 melee attacks over
    // the repaired bridge killed 119 and cost 6 attackers, while the
    // barbarians attacked us 867 times to our 290. See
    // `BasicAi::barbarian_bargain`.
    Gene { tag: "barbarian-bargain", field: "barbarian_bargain", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_barbarian_bargain, disable: AdvancedAi::disable_barbarian_bargain },
    // And a ring of shooters is answered by a shooter: they field 45 % of
    // their attacks ranged to our 22 %, and our ranged attacks have never
    // lost the attacker. See `BasicAi::barbarian_ranged_answer`.
    // And a ring of shooters is answered by a shooter: they field 45 % of
    // their attacks ranged to our 22 %, and our ranged attacks have never
    // lost the attacker. See `BasicAi::barbarian_ranged_answer`.
    Gene { tag: "barbarian-ranged-answer", field: "barbarian_ranged_answer", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_barbarian_ranged_answer, disable: AdvancedAi::disable_barbarian_ranged_answer },
    // And the sea gets one eye of its own. See `BasicAi::naval_recon`.
    // And the sea gets one eye of its own. See `BasicAi::naval_recon`.
    Gene { tag: "naval-recon", field: "naval_recon", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_naval_recon, disable: AdvancedAi::disable_naval_recon },
    // And in peacetime the whole field army clears it. See
    // `BasicAi::camp_party`.
    // And in peacetime the whole field army clears it. See
    // `BasicAi::camp_party`.
    Gene { tag: "camp-party", field: "camp_party", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_camp_party, disable: AdvancedAi::disable_camp_party },
    // And the settler decides on the real board, prices capture as
    // capture and trusts only a guard on its tile; see
    // settler_stack_discipline.
    // The religion lane was structurally blocked by its own wars; see
    // religion_sues_peace.
    // A Religion plan that keeps its wars blockades its own lane.
    Gene { tag: "religion-sues-peace", field: "religion_sues_peace", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_religion_sues_peace, disable: AdvancedAi::disable_religion_sues_peace },
    // The other half of that same capital's diagnosis: garrison_under_fire
    // reacts to a city already bleeding, but the capital that fell had
    // NEVER ORDERED WALLS — max_wall_damage 0 at t115 with production on
    // the culture lane and the fog hiding every attacker until adjacency.
    // See BasicAi::garrison_walls_item.
    // Settler conversion is the score frontier the first seven live games
    // isolated; see escort_unstick.
    // Getting a settler to a site it can keep.
    Gene { tag: "escort-unstick", field: "escort_unstick", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_escort_unstick, disable: AdvancedAi::disable_escort_unstick },
    // And the Library and University come before the Great Merchant race.
    // See `buildings_before_projects`.
    // The cheap half of a research city before the race in it. See
    // `buildings_before_projects`.
    Gene { tag: "buildings-before-projects", field: "buildings_before_projects", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_buildings_before_projects, disable: AdvancedAi::disable_buildings_before_projects },
    // And the guard on the settler's tile holds there, and only a guard
    // that can hold counts — both settlers of civvis-20260819T025840Z were
    // taken one tile outside Rome from a tile a warrior had just left.
    // See `settler_guard_holds`.
    // And a stacked guard holds, and only one that can hold counts. See
    // `settler_guard_holds`.
    Gene { tag: "settler-guard-holds", field: "settler_guard_holds", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_settler_guard_holds, disable: AdvancedAi::disable_settler_guard_holds },
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
    Gene { tag: "stranded-settler-discount", field: "stranded_settler_discount", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_stranded_settler_discount, disable: AdvancedAi::disable_stranded_settler_discount },
    // Three straight Settler losses were an eight-city empire against
    // ten- and eleven-city rivals; the stock nine-ceiling was the binding
    // constant. See `wide_map_capacity`.
    Gene { tag: "wide-map-capacity", field: "wide_map_capacity", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_wide_map_capacity, disable: AdvancedAi::disable_wide_map_capacity },
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
    // ⚠⚠ AND THE TWO HOUSING CARDS THE EMPIRE CAN REACH ARE NEVER PLAYED.
    // `medina_quarter` (+2 Housing at 3+ specialty districts) is slotted in
    // **0 of 107 live runs** and appears nowhere in `src/`; `insulae` (+1 at
    // 2+) in **1**. Housing is the dominant growth cap — 71.7% of 13,214
    // host-exported city-turns sit under it at a mean multiplier of 0.510,
    // against the Amenity band's 0.872 — and 60.3% / 40.0% of city-turns
    // already carry the 2 / 3 specialty districts these cards need.
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
    Gene { tag: "amenity-project-preemption", field: "amenity_project_preemption", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_amenity_project_preemption, disable: AdvancedAi::disable_amenity_project_preemption },
    Gene { tag: "amenity-district-path", field: "amenity_district_path", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_amenity_district_path, disable: AdvancedAi::disable_amenity_district_path },
    // ⚠⚠ AND THE REPAIR IS BEHIND A TECH THE ARGMAX NEVER AIMS AT. Over 94
    // live runs the median empire ends on **30 techs of 77**, `engineering`
    // is reached by only **73%** and at a median turn **116** — which is why
    // the live median Aqueduct order lands at turn 164. Making the district
    // reachable in the build lists cannot beat the tech that gates it.
    Gene { tag: "housing-research", field: "housing_research", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_housing_research, disable: AdvancedAi::disable_housing_research },
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
    Gene { tag: "district-coverage", field: "district_coverage", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_district_coverage, disable: AdvancedAi::disable_district_coverage },
    Gene { tag: "slot-kind-tiebreak", field: "slot_kind_tiebreak", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_slot_kind_tiebreak, disable: AdvancedAi::disable_slot_kind_tiebreak },
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
    // Keeping it loyal.
    Gene { tag: "loyalty-rate-alarm", field: "loyalty_rate_alarm", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_loyalty_rate_alarm, disable: AdvancedAi::disable_loyalty_rate_alarm },
    // And the last fifty turns are a tally, not a launch window. See
    // `score_horizon`.
    // And the last fifty turns are a tally, not a launch window. See
    // `score_horizon`.
    Gene { tag: "score-horizon", field: "score_horizon", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_score_horizon, disable: AdvancedAi::disable_score_horizon },
    // And the race that does fit needs one launch pad, not one per city.
    // See `one_launch_pad`.
    // And the race that does fit needs one launch pad, not one per city.
    // See `one_launch_pad`.
    Gene { tag: "one-launch-pad", field: "one_launch_pad", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_one_launch_pad, disable: AdvancedAi::disable_one_launch_pad },
    // And a settler target dropped for danger stays dropped for a while.
    // See `settler_target_hysteresis`.
    // And a settler target dropped for danger stays dropped for a while.
    // See `settler_target_hysteresis`.
    Gene { tag: "settler-target-hysteresis", field: "settler_target_hysteresis", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_settler_target_hysteresis, disable: AdvancedAi::disable_settler_target_hysteresis },
    // And the buildings that chain hangs off. See `culture_building_debt`.
    // ★★★★ A TAG IN `ENGINE_REPAIR_TREATMENTS` WHOSE ENABLE IS MISSING
    // HERE SCREENS AS EXACTLY INERT, AND SAYS SO IN A WAY THAT IS EASY TO
    // READ AS A RESULT. `gene_screen` builds its treated seat from
    // `enable_engine_repairs_universe` and then flips only the genes whose
    // drawn bit differs from `Gene::after_setup_on` — which the table
    // asserts is `true` for every engine repair. A repair this bundle
    // never turns on is therefore off in BOTH arms of every pair, the two
    // arms play byte-identical games, and the screen reports `Δ +0.0
    // [+0.0, +0.0] z +0.00` for the gene. That is the signature: a
    // zero-width confidence interval is not a null, it is a gene that was
    // never varied. Three tags reached the tables before this line and
    // burned 30 games saying nothing.
    //
    // The research economy's two counterparts on the culture tree, and the
    // chain that fills every specialty district. `enable_engine_repairs`
    // applies the ledger after this, so an unmeasured one is still off at
    // deployment.
    Gene { tag: "culture-building-debt", field: "culture_building_debt", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_culture_building_debt, disable: AdvancedAi::disable_culture_building_debt },
    // ── Host-only: shipped by the Civilization VI seat, inert headless ──
    Gene { tag: "live-trader-route", field: "live_trader_route_adapter", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_trader_route_adapter, disable: AdvancedAi::disable_live_trader_route_adapter },
    Gene { tag: "live-religious-purchase", field: "live_religious_purchase_guard", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_religious_purchase_guard, disable: AdvancedAi::disable_live_religious_purchase_guard },
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
    Gene { tag: "live-motion-turn-accounting", field: "live_motion_turn_accounting", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_motion_turn_accounting, disable: AdvancedAi::disable_live_motion_turn_accounting },
    Gene { tag: "solvent-faith-army", field: "solvent_faith_army", kind: Kind::HostOnly, enable: AdvancedAi::enable_solvent_faith_army, disable: AdvancedAi::disable_solvent_faith_army },
    // And the formation channel that escort depends on went 0-for-7 on
    // the live bridge while two unescorted settlers were captured one
    // turn short of founding; see stacked_escort.
    // The native genome now withholds `stacked_escort`, but that result
    // does not make the host's formation channel work.  Keep live
    // settlers on the already-tested ordinary-unit shadow instead.
    Gene { tag: "live-formationless-settler-shadow", field: "live_formationless_settler_shadow", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_formationless_settler_shadow, disable: AdvancedAi::disable_live_formationless_settler_shadow },
    // Zero wonder orders in twenty live Settler runs that all ended on the
    // host's score tally, 15 points a wonder. See `live_wonder_race`.
    Gene { tag: "live-wonder-race", field: "live_wonder_race", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_wonder_race, disable: AdvancedAi::disable_live_wonder_race },
    Gene { tag: "expansion-before-prophet", field: "expansion_before_prophet", kind: Kind::HostOnly, enable: AdvancedAi::enable_expansion_before_prophet, disable: AdvancedAi::disable_expansion_before_prophet },
    Gene { tag: "no-elective-war", field: "no_elective_war", kind: Kind::HostOnly, enable: AdvancedAi::enable_no_elective_war, disable: AdvancedAi::disable_no_elective_war },
    // And that ceiling was still priced off the revealed quarter of the
    // map. See `fog_land_capacity`.
    Gene { tag: "fog-land-capacity", field: "fog_land_capacity", kind: Kind::HostOnly, enable: AdvancedAi::enable_fog_land_capacity, disable: AdvancedAi::disable_fog_land_capacity },
    // And the last-quarter score-leader alarm asks for a race, not a war.
    // See `counter_in_lane`.
    Gene { tag: "counter-in-lane", field: "counter_in_lane", kind: Kind::HostOnly, enable: AdvancedAi::enable_counter_in_lane, disable: AdvancedAi::disable_counter_in_lane },
    // And the city target climbs at the Settler game's own era pace. See
    // `era_paced_expansion`.
    Gene { tag: "era-paced-expansion", field: "era_paced_expansion", kind: Kind::HostOnly, enable: AdvancedAi::enable_era_paced_expansion, disable: AdvancedAi::disable_era_paced_expansion },
    // And a civic is three points on that tally to a tech's two. See
    // `tally_culture`.
    Gene { tag: "tally-culture", field: "tally_culture", kind: Kind::HostOnly, enable: AdvancedAi::enable_tally_culture, disable: AdvancedAi::disable_tally_culture },
    // And no colony beyond the empire's Loyalty reach on fogged ground.
    // See `frontier_loyalty`.
    Gene { tag: "frontier-loyalty", field: "frontier_loyalty", kind: Kind::HostOnly, enable: AdvancedAi::enable_frontier_loyalty, disable: AdvancedAi::disable_frontier_loyalty },
    // And banked Faith buys the Great People the tally pays five for. See
    // `tally_great_people`.
    Gene { tag: "tally-great-people", field: "tally_great_people", kind: Kind::HostOnly, enable: AdvancedAi::enable_tally_great_people, disable: AdvancedAi::disable_tally_great_people },
    // ⚠ The live seat plays under an assigned lane (`--victory science`),
    // and `victory_denial` stands down entirely for a targeted seat — so
    // five of the twelve runs the seat was LEADING on 2026-08-16/17 ended
    // at t229-245 on a rival's culture, technology or diplomatic victory
    // with the whole counter apparatus gated off. Match point overrides
    // the lane's focus; ordinary pressure still never does.
    Gene { tag: "deny-while-targeted", field: "deny_while_targeted", kind: Kind::HostOnly, enable: AdvancedAi::enable_deny_while_targeted, disable: AdvancedAi::disable_deny_while_targeted },
    // ⚠ And the alarm must reach the two lanes it cannot answer late.
    // Four of the five stolen games above were Culture; the general 90
    // bar had not fired when the game ended. See `STOCK_DENIAL_BAR`.
    Gene { tag: "stock-denial-lead-time", field: "stock_denial_lead_time", kind: Kind::HostOnly, enable: AdvancedAi::enable_stock_denial_lead_time, disable: AdvancedAi::disable_stock_denial_lead_time },
    // ⚠ And the stock bar must fire BEFORE the last Congress, not at it.
    // The first game on the repaired economy (231407Z) crossed the bar at
    // ~t221; the final Congress sat at t222 with nothing reading urgent
    // when its ballot was priced, and Egypt won Culture at t232. The
    // projection carries the recorded slope fifteen turns forward and
    // feeds the same gate. See `projected_stock_denial`.
    Gene { tag: "projected-stock-denial", field: "projected_stock_denial", kind: Kind::HostOnly, enable: AdvancedAi::enable_projected_stock_denial, disable: AdvancedAi::disable_projected_stock_denial },
    // These host-facing controls used to be applied by `civvis_orders`
    // after this bundle. They belong here: `live` and every
    // `live_without_*` arm must construct the controller that deployment
    // actually plays.
    Gene { tag: "parallel-settlers", field: "parallel_settlers", kind: Kind::HostOnly, enable: AdvancedAi::enable_parallel_settlers, disable: AdvancedAi::disable_parallel_settlers },
    Gene { tag: "host-settler-pop", field: "host_settler_pop", kind: Kind::HostOnly, enable: AdvancedAi::enable_host_settler_pop, disable: AdvancedAi::disable_host_settler_pop },
    Gene { tag: "explore-dead-targets", field: "explore_dead_targets", kind: Kind::HostOnly, enable: AdvancedAi::enable_explore_dead_targets, disable: AdvancedAi::disable_explore_dead_targets },
    Gene { tag: "explore-commit", field: "explore_commit", kind: Kind::HostOnly, enable: AdvancedAi::enable_explore_commit, disable: AdvancedAi::disable_explore_commit },
    Gene { tag: "bank-envoys", field: "bank_envoys", kind: Kind::HostOnly, enable: AdvancedAi::enable_bank_envoys, disable: AdvancedAi::disable_bank_envoys },
    // And the rung clock itself: the seat leads the count at t50, loses
    // it by t150 and stops settling at t116 under an assigned lane. See
    // `land_grab`.
    Gene { tag: "land-grab", field: "land_grab", kind: Kind::HostOnly, enable: AdvancedAi::enable_land_grab, disable: AdvancedAi::disable_land_grab },
    // And a Spy order the host is still running is not re-sent every
    // turn: the rebuilt mirror cannot see a running operation, so the
    // first run with a working spy chain (civvis-20260818T155500Z)
    // re-ordered SPY_GAIN_SOURCES 35 times. See `spy_mission_patience`.
    Gene { tag: "spy-mission-patience", field: "spy_mission_patience", kind: Kind::HostOnly, enable: AdvancedAi::enable_spy_mission_patience, disable: AdvancedAi::disable_spy_mission_patience },
    // And the pantheon is the one that founds a city, bought with the
    // Faith card the portfolio used to throw away at the first civic
    // swap: Divine Spark 40 of 40 times at median t22 (t108 at worst)
    // where Religious Settlements is a free Settler at ~t20. See
    // `expansion_pantheon`.
    Gene { tag: "expansion-pantheon", field: "expansion_pantheon", kind: Kind::HostOnly, enable: AdvancedAi::enable_expansion_pantheon, disable: AdvancedAi::disable_expansion_pantheon },
    // And the plaza the seat builds by t29–57 in every game gets the one
    // building the land grab is made of — a free Builder in every new
    // city, +50% Settlers — instead of standing empty to turn 250. See
    // `expansion_hall`.
    Gene { tag: "expansion-hall", field: "expansion_hall", kind: Kind::HostOnly, enable: AdvancedAi::enable_expansion_hall, disable: AdvancedAi::disable_expansion_hall },
    // And the book's own Settler slot survives the host's population
    // floor: half the recorded openings burned it and ordered the first
    // Settler four turns later. See `opening_settler_waits`.
    Gene { tag: "opening-settler-waits", field: "opening_settler_waits", kind: Kind::HostOnly, enable: AdvancedAi::enable_opening_settler_waits, disable: AdvancedAi::disable_opening_settler_waits },
    // ── Production: on before the ledger ──
    Gene { tag: "strategic-wonders", field: "strategic_wonders", kind: Kind::Production, enable: AdvancedAi::enable_strategic_wonders, disable: AdvancedAi::disable_strategic_wonders },
    // ── Opt-ins: off until the ledger turns one on ──
    // A Builder does not walk into a visible Barbarian-capture envelope; its
    // target and retreat both use the same fog-honest risk model as Settlers.
    // `gene_screen` discovers this native opt-in directly from this row.
    Gene { tag: "builder-barbarian-safety", field: "builder_barbarian_safety", kind: Kind::OptIn, enable: AdvancedAi::enable_builder_barbarian_safety, disable: AdvancedAi::disable_builder_barbarian_safety },
    Gene { tag: "apostle-promotion-by-role", field: "apostle_promotion_by_role", kind: Kind::OptIn, enable: AdvancedAi::enable_apostle_promotion_by_role, disable: AdvancedAi::disable_apostle_promotion_by_role },
    Gene { tag: "founder-temple", field: "founder_temple", kind: Kind::OptIn, enable: AdvancedAi::enable_founder_temple, disable: AdvancedAi::disable_founder_temple },
    Gene { tag: "holy-lane-parity", field: "holy_lane_parity", kind: Kind::OptIn, enable: AdvancedAi::enable_holy_lane_parity, disable: AdvancedAi::disable_holy_lane_parity },
    // Half the seats never found a religion and bank ~1,000 Faith they
    // cannot spend; see `AdvancedAi::idle_faith_patronage`.
    Gene { tag: "idle-faith-patronage", field: "idle_faith_patronage", kind: Kind::OptIn, enable: AdvancedAi::enable_idle_faith_patronage, disable: AdvancedAi::disable_idle_faith_patronage },
    // The rest of the same chain: the founder's corps cannot answer more than
    // two cities, cannot heal when it is defending, and spends charges at a
    // fraction of their strength. See `advanced/religion.rs`.
    Gene { tag: "religious-defence-scales", field: "religious_defence_scales", kind: Kind::OptIn, enable: AdvancedAi::enable_religious_defence_scales, disable: AdvancedAi::disable_religious_defence_scales },
    Gene { tag: "religious-units-heal-first", field: "religious_units_heal_first", kind: Kind::OptIn, enable: AdvancedAi::enable_religious_units_heal_first, disable: AdvancedAi::disable_religious_units_heal_first },
    Gene { tag: "holy-site-where-the-threat-is", field: "holy_site_where_the_threat_is", kind: Kind::OptIn, enable: AdvancedAi::enable_holy_site_where_the_threat_is, disable: AdvancedAi::disable_holy_site_where_the_threat_is },
    Gene { tag: "enhancer-for-the-corps", field: "enhancer_for_the_corps", kind: Kind::OptIn, enable: AdvancedAi::enable_enhancer_for_the_corps, disable: AdvancedAi::disable_enhancer_for_the_corps },
    // Every major adopts Early Empire on turns 23-30, and a Scout's sight of 2
    // cannot see a city-state's seat from outside its border. The land route
    // to first contact closes once and never reopens; see
    // `AdvancedAi::early_contact_window`.
    Gene { tag: "early-contact-window", field: "early_contact_window", kind: Kind::OptIn, enable: AdvancedAi::enable_early_contact_window, disable: AdvancedAi::disable_early_contact_window },
    // A Great Person earned and blocked is a race forfeited: build the slot
    // space ahead of the person, sell duplicate works when nothing can be
    // built; see `AdvancedAi::great_person_housing`.
    Gene { tag: "great-person-housing", field: "great_person_housing", kind: Kind::OptIn, enable: AdvancedAi::enable_great_person_housing, disable: AdvancedAi::disable_great_person_housing },
    // A surprise war priced on what the board exposes — an unescorted
    // Settler or Builder, a cluster of unpillaged tiles — taken by movement
    // and closed by peace; see `AdvancedAi::opportunistic_war`.
    Gene { tag: "opportunistic-war", field: "opportunistic_war", kind: Kind::OptIn, enable: AdvancedAi::enable_opportunistic_war, disable: AdvancedAi::disable_opportunistic_war },
    // The pillage half of the raid, priced apart: inert unless the row
    // above is on. See `AdvancedAi::raid_pillage_prizes`.
    Gene { tag: "raid-pillage-prizes", field: "raid_pillage_prizes", kind: Kind::OptIn, enable: AdvancedAi::enable_raid_pillage_prizes, disable: AdvancedAi::disable_raid_pillage_prizes },
    // A target can be excellent while a visible hostile makes its next route
    // step unsafe. This holds that corridor aside briefly and sends the
    // Settler to the best safe runner-up; see `settler_threat_detour`.
    Gene { tag: "settler-threat-detour", field: "settler_threat_detour", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_threat_detour, disable: AdvancedAi::disable_settler_threat_detour },
    // The site ranking is indifferent between founding now and founding the
    // same value later; this prices every turn of the walk, dearer the longer
    // the Settler has been out. See `settle_sooner`.
    Gene { tag: "settle-sooner", field: "settle_sooner", kind: Kind::OptIn, enable: AdvancedAi::enable_settle_sooner, disable: AdvancedAi::disable_settle_sooner },
    // A Library bought after Rationalism earns twice a Library bought before
    // it, and the price never noticed. See
    // `AdvancedAi::science_multiplier_payoff`.
    Gene { tag: "science-multiplier-payoff", field: "science_multiplier_payoff", kind: Kind::OptIn, enable: AdvancedAi::enable_science_multiplier_payoff, disable: AdvancedAi::disable_science_multiplier_payoff },
    // The chain's rungs are 2, 4 and 3-plus-5 and the debt that buys them is
    // flat. See `AdvancedAi::research_tier_premium`.
    Gene { tag: "research-tier-premium", field: "research_tier_premium", kind: Kind::OptIn, enable: AdvancedAi::enable_research_tier_premium, disable: AdvancedAi::disable_research_tier_premium },
    // `withdraw_hp` is a constant and the enemy's damage is not: a unit the
    // strongest thing in reach would kill in one blow recovers whatever its
    // hit points say, and healing ground that comes under a shooter's reach
    // is left. See `BasicAi::one_shot_recovery`.
    Gene { tag: "one-shot-recovery", field: "one_shot_recovery", kind: Kind::OptIn, enable: AdvancedAi::enable_one_shot_recovery, disable: AdvancedAi::disable_one_shot_recovery },
    // The four victory-lane genes (`advanced/victory_lane.rs`,
    // `docs/VICTORY_GENES.md`) and the competition value they enable. A
    // targeted seat spends about a fifth of the game — and an adaptive one
    // 15% — with `Expansion` in its plan, and `take_turn_inner` hands that
    // to every lane-shaped decider. The first two substitute the victory the
    // empire is actually racing at one decider; the next two enable the
    // Culture and Science passes, and the last prices the Diplomatic Victory
    // Points a scored competition pays, which nothing priced.
    Gene { tag: "lane-great-people", field: "lane_great_people", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_great_people, disable: AdvancedAi::disable_lane_great_people },
    Gene { tag: "lane-policy-deck", field: "lane_policy_deck", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_policy_deck, disable: AdvancedAi::disable_lane_policy_deck },
    Gene { tag: "lane-culture-spending", field: "lane_culture_spending", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_culture_spending, disable: AdvancedAi::disable_lane_culture_spending },
    Gene { tag: "lane-space-race", field: "lane_space_race", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_space_race, disable: AdvancedAi::disable_lane_space_race },
    Gene { tag: "competition-victory-points", field: "competition_victory_points", kind: Kind::OptIn, enable: AdvancedAi::enable_competition_victory_points, disable: AdvancedAi::disable_competition_victory_points },
    // ⚠ Barbarians take 7.0 major cities a game -- 65% of everything a major
    // loses -- at a median city age of TEN TURNS, and `settle_value`
    // explicitly filters barbarians out of its proximity penalty, so a site
    // beside a barbarian city scores exactly as well as an empty one. Cities
    // held correlate with final score at r = +0.89; cities FOUNDED at -0.03.
    // The A/B moved cities held +1.09 (p = 0.0001) and barbarian losses -0.48
    // (p < 0.0001), both replicated; its 35-37 win count over 72 games is
    // underpowered, not a null. See `defensible_sites`.
    Gene { tag: "defensible-sites", field: "defensible_sites", kind: Kind::OptIn, enable: AdvancedAi::enable_defensible_sites, disable: AdvancedAi::disable_defensible_sites },
    // ⚠ "Strong enough to take what a neighbour has" never asks WHICH
    // neighbour: `weakest_rival` is the minimum power over every met major
    // with no proximity test, so the branch really asks "am I 1.8x the
    // feeblest empire in the world?" -- true from t55 onward for anyone doing
    // well, next door or on another continent. `city_campaign` already has
    // the reach this wants (`CAMPAIGN_REACH`, "the same reach the
    // declaration's `close_enough` already asks for") and branch nine uses
    // it. See `elective_war_in_reach`.
    Gene { tag: "elective-war-in-reach", field: "elective_war_in_reach", kind: Kind::OptIn, enable: AdvancedAi::enable_elective_war_in_reach, disable: AdvancedAi::disable_elective_war_in_reach },
    // ⚠ The same shape as the row above, on the branch beside it. Recovery's
    // power gap reads `strongest_rival` -- the maximum over every MET major,
    // at peace or not, next door or on another continent -- so an empire at
    // war with a weak neighbour takes the defensive posture because a distant
    // superpower exists. The file already names this ("the power-gap Recovery
    // a strong third party would trigger") and exempts only `raid_only_war`.
    // Recovery is 19% of the board's planner-turns under
    // `audit --genome deployment`. See `recovery_reads_the_war`.
    Gene { tag: "recovery-reads-the-war", field: "recovery_reads_the_war", kind: Kind::OptIn, enable: AdvancedAi::enable_recovery_reads_the_war, disable: AdvancedAi::disable_recovery_reads_the_war },
    // ⚠⚠ THE BIGGEST NUMBER IN THIS REPOSITORY SITS BEHIND A FLAG NOTHING
    // COULD SET. #554: a free settler while short of the city target more than
    // DOUBLES the win rate, 23.0% -> 52.3% at p=0.0000 over 300 games. #559:
    // the shut expansion window is the SOLE blocker on 31.2% of the city-turns
    // an empire spends short of its own target. `settler_expansion_window_open`
    // takes the payback branch only under `land_grab` (HostOnly) or this flag
    // (an orphaned evaluator arm), so the native board has only ever shut the
    // window on a clock. See `expansion_pays_back`.
    Gene { tag: "expansion-pays-back", field: "expansion_pays_back", kind: Kind::OptIn, enable: AdvancedAi::enable_expansion_pays_back, disable: AdvancedAi::disable_expansion_pays_back },
    // ⚠ The last blind lane in the threat model: Domination is read as
    // `100 * foreign CAPITALS held / foreign capitals`, five values twenty
    // points apart, and ZERO for a rival that has taken any number of ordinary
    // cities. Captured cities are production, population, districts and techs
    // -- the currency of every other lane -- so the empire eating the board
    // reads as a pacifist right up to the first palace. See
    // `domination_city_count`.
    Gene { tag: "domination-city-count", field: "domination_city_count", kind: Kind::OptIn, enable: AdvancedAi::enable_domination_city_count, disable: AdvancedAi::disable_domination_city_count },
    // ⚠ The largest branch in the grand-strategy cascade and it has no exit:
    // any war at all pins the plan on Conquest for its whole duration.
    // Conquest takes 40% of the planner-turns while domination finishes 2/16
    // and is 1 of 107 recorded
    // rival victories. Withdrawn once on a -22.2 pp reading that #2452 proved
    // was a degenerate-block artifact; measured here on the repaired
    // instrument. See `unchosen_war_keeps_the_lane`.
    Gene { tag: "unchosen-war-keeps-the-lane", field: "unchosen_war_keeps_the_lane", kind: Kind::OptIn, enable: AdvancedAi::enable_unchosen_war_keeps_the_lane, disable: AdvancedAi::disable_unchosen_war_keeps_the_lane },
    // ⚠ The row above's construction, applied to the two branches below it in
    // the cascade. That gene is the only one of eleven written this week with
    // persistent signal (+4.7 pp, z +2.09 over 271 games) and what it does is
    // refuse to let a war a RIVAL started take the plan from a live lane.
    // These branches take the plan for a war WE choose, and `no_elective_war`
    // records that they never converted in eight live runs -- 0 cities taken,
    // 16 lost -- while being Firaxis-only, so the native board runs them bare.
    // See `elective_war_yields_to_a_lane`.
    Gene { tag: "elective-war-yields-to-a-lane", field: "elective_war_yields_to_a_lane", kind: Kind::OptIn, enable: AdvancedAi::enable_elective_war_yields_to_a_lane, disable: AdvancedAi::disable_elective_war_yields_to_a_lane },
    // ⚠ The planner judges its OWN diplomatic position by `dvp * 5 +
    // suzerain * 6` and a RIVAL's by `dvp * 5`. A rival holding every
    // city-state -- the whole Favor engine that manufactures the points --
    // contributes nothing to its threat reading until the points land, and
    // diplomatic victories are 58 of the 107 recorded losses to a rival. The
    // missing term is the one the file already has. See
    // `rival_suzerainty_alarm`.
    Gene { tag: "rival-suzerainty-alarm", field: "rival_suzerainty_alarm", kind: Kind::OptIn, enable: AdvancedAi::enable_rival_suzerainty_alarm, disable: AdvancedAi::disable_rival_suzerainty_alarm },
    // ⚠ This decides WHO the three targeted penalties are pointed at -- the empire
    // `victory_denial` names, instead of the diplomatic leader. The Congress
    // is the only counter in this game not paid for in development
    // (`resolve_congress` refunds a losing vote in full), which is exactly
    // what the war-shaped counters in `docs/COUNTERING_LEADERS.md` could not
    // say. See `congress_counter_leader`.
    Gene { tag: "congress-counter-leader", field: "congress_counter_leader", kind: Kind::OptIn, enable: AdvancedAi::enable_congress_counter_leader, disable: AdvancedAi::disable_congress_counter_leader },
    // ⚠ Eighteen of 32 screened games today ended on a Science Victory, and
    // the threat model reads that race as a five-step ladder off launches
    // already made: a rival one tech from Rocketry with a Spaceport standing
    // reads ZERO. `rocketry_readiness` is the same chain this planner uses to
    // judge itself, off public tech-screen information. See
    // `science_chain_alarm`.
    Gene { tag: "science-chain-alarm", field: "science_chain_alarm", kind: Kind::OptIn, enable: AdvancedAi::enable_science_chain_alarm, disable: AdvancedAi::disable_science_chain_alarm },
    // ⚠ Two thirds of native games end by religious conversion, and the alarm
    // for it has five values twenty points apart: `religious_conversion_tally`
    // counts whole civilizations already lost, so a rival holding 45% of every
    // rival's cities reads zero. The victory rule is a count of cities and
    // counting them is smooth. See `conversion_majority_alarm`.
    Gene { tag: "conversion-majority-alarm", field: "conversion_majority_alarm", kind: Kind::OptIn, enable: AdvancedAi::enable_conversion_majority_alarm, disable: AdvancedAi::disable_conversion_majority_alarm },
    // ⚠ The same lock `diplomatic-lane-forecast` opened, on the second-best
    // lane this engine has: `victory_eval` finishes culture 12/16 and `audit`
    // gives it 2% of 14,376 planner-turns, below a twentieth of the board's
    // planning in eleven of twelve games. The lane reads a ratio of the
    // finished race, and foreign tourists are near zero until the Renaissance.
    // Unlike Diplomacy, both curves move, so this projects both. See
    // `culture_lane_forecast`.
    Gene { tag: "culture-lane-forecast", field: "culture_lane_forecast", kind: Kind::OptIn, enable: AdvancedAi::enable_culture_lane_forecast, disable: AdvancedAi::disable_culture_lane_forecast },
    // ⚠ The lane this engine finishes most often is the lane the planner never
    // picks. `victory_eval` at the ladder's profile: diplomatic 14/16, culture
    // 12/16, religious 8/16, domination 2/16, science 0/16. `audit` over the
    // same profile: diplomacy 0% of 14,376 planner-turns in 12 of 12 games,
    // culture 2% -- against conquest 43%. Science reads a readiness ramp and
    // Religion a 46-point commitment floor; Diplomacy reads points already
    // banked and so cannot be chosen until it has already been played. See
    // `diplomatic_lane_forecast`.
    Gene { tag: "diplomatic-lane-forecast", field: "diplomatic_lane_forecast", kind: Kind::OptIn, enable: AdvancedAi::enable_diplomatic_lane_forecast, disable: AdvancedAi::disable_diplomatic_lane_forecast },
    // ⚠ `city_pressure_with_visibility` counts hostile strength only from
    // civilizations we are ALREADY AT WAR WITH, so a rival massing on our
    // border at peace reads exactly zero and no city is ever "threatened"
    // before the declaration. (The -22.2 pp this row used to cite from the
    // withdrawn `unchosen-war-keeps-the-lane` probe is retracted: it was a
    // degenerate-block artifact, see `docs/GENE_SCREEN.md`. The filter is
    // still a fact about the code.)
    // See `frontier_massing_alarm`.
    Gene { tag: "frontier-massing-alarm", field: "frontier_massing_alarm", kind: Kind::OptIn, enable: AdvancedAi::enable_frontier_massing_alarm, disable: AdvancedAi::disable_frontier_massing_alarm },
    // Nothing in this controller reaches the air layer: the melee package
    // skips `domain == "air"` and ranks its unlocks by cheapest remaining
    // research, so it can appoint the next technology and never a chain.
    // This beelines Advanced Flight from three technologies out, raises an
    // Aerodrome and a bomber wing, and takes the appointed city with the
    // cavalry behind it. See `advanced/air_surge.rs`.
    Gene { tag: "air-surge", field: "air_surge", kind: Kind::OptIn, enable: AdvancedAi::enable_air_surge, disable: AdvancedAi::disable_air_surge },
    // ⭐ APPENDED AT THE END ON PURPOSE. A gene inserted into the middle of
    // this table renumbers every row after it, and `gene_screen` writes the
    // genome as a POSITIONAL bit string — so a screen already running against
    // an older binary cannot be pooled with one running against this. That
    // cost the 83,000,000 run its analysis; see
    // `tools/gene_quantity_contrast.py`.
    //
    // The Research Lab's larger half is switched off until something generates
    // power, and nothing in the controller buys the switch. See
    // `AdvancedAi::power_the_laboratory`.
    Gene { tag: "power-the-laboratory", field: "power_the_laboratory", kind: Kind::OptIn, enable: AdvancedAi::enable_power_the_laboratory, disable: AdvancedAi::disable_power_the_laboratory },
    // Rationalism is slotted and NOT ONE Campus in the empire clears the
    // adjacency half it pays. Appended at the end, for the reason above. See
    // `AdvancedAi::campus_adjacency_threshold`.
    Gene { tag: "campus-adjacency-threshold", field: "campus_adjacency_threshold", kind: Kind::OptIn, enable: AdvancedAi::enable_campus_adjacency_threshold, disable: AdvancedAi::disable_campus_adjacency_threshold },
    // ★★★★ FOURTEEN BEHAVIOURS THAT NO SCREEN COULD REACH, AND THE COUNT THAT
    // FOUND THEM. `docs/EVAL_STATUS.md`'s "Genome coverage" section read 165
    // capability toggles on the controller, 100 reachable as a gene and **65
    // unreachable by any screen** — a number published because
    // `precise_evacuation` shipped ON for every seat, held roughly half the
    // simulator's main thread, and had no gene row, no evaluator arm and no
    // mention in any round. Neither gate could address it and nothing said so.
    // These fourteen are the part of that 65 a native screen can honestly
    // price. Every one is off in `promoted_policy_envoy`, so an opt-in row is
    // the whole change: `apply_gene_ledger` enables an opt-in only on a
    // `default_on` ledger row and there is none, so each ships off and
    // unmeasured and NOTHING about the deployed agent moves.
    //
    // ⭐ EIGHTEEN ROWS WERE WRITTEN AND FOUR WERE TAKEN BACK OUT, BY THE GATE
    // THIS SAME CHANGE ADDS. `camp_bounty`, `great_work_veto_by_district`,
    // `sea_answers` and `settler_founds_when_stalled` each left both arms of a
    // single-gene probe byte-identical — win Δ, share Δ and both standard
    // errors exactly zero — so they would have consumed a genome bit and
    // returned the zero-width interval `docs/GENE_SCREEN.md` warns about.
    // `tools/gene_fires.py` is what says so; their probes and their reasons are
    // in `docs/gene_screens/fires/` and `docs/genome_reach_debt.json`. The rest
    // of the residual is accounted for one line at a time in
    // `docs/GENE_SCREEN.md` (§"The toggles no screen can reach"); it is
    // examined work, not an unexamined ceiling.
    //
    // ⭐ APPENDED AT THE END, for the reason four rows up: `gene_screen` writes
    // the genome as a POSITIONAL bit string.
    //
    // ⚠⚠ AND THE TABLE ABOVE IS DELIBERATELY THE ONLY DOOR USED HERE. Six more
    // of the 65 — `explore_commit`, `open_water_navy`, `amenity_districts`,
    // `hut_collection`, `village_seeking`, `legal_tactical_candidates` — are
    // native behaviours production ships ON, whose door is `PRODUCTION_TREATMENTS`.
    // That row is NOT neutral: `apply_gene_ledger` disables every production
    // treatment whose `ledger_default_on` is `Some(false)`, and a screenable tag
    // with no ledger row is exactly that. Adding those six would switch six
    // shipped behaviours off at deployment — `open_water_navy` alone was
    // promoted at +61 Elo — which is a genome change wearing an instrument's
    // clothes. They need their first screen row before they can have a gene row.
    //
    // The friendly-volley extension lets a force finish a defender together
    // without reopening the closed war-half bundle.
    Gene { tag: "coordinated-finish", field: "coordinated_finish", kind: Kind::OptIn, enable: AdvancedAi::enable_coordinated_finish, disable: AdvancedAi::disable_coordinated_finish },
    Gene { tag: "unit-objective-memory", field: "unit_objective_memory", kind: Kind::OptIn, enable: AdvancedAi::enable_unit_objective_memory, disable: AdvancedAi::disable_unit_objective_memory },
    // Choose the pantheon from the land the empire actually holds rather than
    // from a fixed order.
    Gene { tag: "pantheon-board", field: "pantheon_board", kind: Kind::OptIn, enable: AdvancedAi::enable_pantheon_board, disable: AdvancedAi::disable_pantheon_board },
    // The policy counterfactual sees the unit-maintenance bill, so the cards
    // that pay it stop scoring zero.
    Gene { tag: "maintenance-aware-deck", field: "maintenance_aware_deck", kind: Kind::OptIn, enable: AdvancedAi::enable_maintenance_aware_deck, disable: AdvancedAi::disable_maintenance_aware_deck },
    // Credit strength-per-production, and the civilization's unique unit, in
    // military production.
    Gene { tag: "unit-cost-efficiency", field: "unit_cost_efficiency", kind: Kind::OptIn, enable: AdvancedAi::enable_unit_cost_efficiency, disable: AdvancedAi::disable_unit_cost_efficiency },
    // The settlement-gap redirect and the Settler ranking read the same city
    // target, instead of two that can disagree.
    Gene { tag: "settlement-gap-target", field: "settlement_gap_target", kind: Kind::OptIn, enable: AdvancedAi::enable_settlement_gap_target, disable: AdvancedAi::disable_settlement_gap_target },
    // Let the envoy scorer see the suzerainty it is walking toward.
    Gene { tag: "price-the-suzerainty", field: "price_the_suzerainty", kind: Kind::OptIn, enable: AdvancedAi::enable_price_the_suzerainty, disable: AdvancedAi::disable_price_the_suzerainty },
    // Read the Faith price from the engine instead of the Standard-speed
    // literal, which overquotes by 2x at Online — the screen's own speed.
    Gene { tag: "engine-faith-price", field: "engine_faith_price", kind: Kind::OptIn, enable: AdvancedAi::enable_engine_faith_price, disable: AdvancedAi::disable_engine_faith_price },
    // A wounded unit may still take its promotion.
    Gene { tag: "promote-when-wounded", field: "promote_when_wounded", kind: Kind::OptIn, enable: AdvancedAi::enable_promote_when_wounded, disable: AdvancedAi::disable_promote_when_wounded },
    // ⚠ The production menu offers each district's top TWO fresh sites
    // ranked by unweighted yield total, districts claim plots independently
    // with nothing reserving ground between them, and no path buys the plot
    // a high-adjacency site sits on — while the best legal owned Campus
    // plot measured ≤2 across three seeds and plots at adjacency ≥4 are
    // under 1% of the map: the ground worth planning for is almost never
    // owned ground. The plan assigns the city's wished districts their
    // plots together over rings 1-3, reserves them, puts its sites on the
    // menu, and buys the tile a very valuable site needs. Appended at the
    // END so a running screen keeps its positional genome. See
    // `AdvancedAi::district_planning` / `advanced/district_planning.rs`.
    Gene { tag: "district-planning", field: "district_planning", kind: Kind::OptIn, enable: AdvancedAi::enable_district_planning, disable: AdvancedAi::disable_district_planning },
    // ⭐ A WONDER LANE ANY CIVILIZATION CAN REACH ON MERIT. The `Item::Wonder`
    // arm refuses every wonder in a 53-wonder roster unless the plan is
    // Culture, the target is Score, or the seat is an UNTARGETED EGYPT OR
    // CHINA. `Game::score_parts` pays 15 points a wonder — the densest line of
    // a tally that decides three quarters of standard-screen games — and no
    // native gate says so: `live-wonder-race` is `Kind::HostOnly` and inert
    // headless, and `strategic-wonders` prices only the lane's own payload,
    // zero for Conquest, Expansion and Recovery. This gene tells the queue: a
    // developed city may take a wonder whose ordinary value plus its fifteen
    // points clears `WONDER_TALLY_MIN_DENSITY` per point of production cost,
    // with no flat lane bonus, so it still loses to a Settler or a district
    // worth more per turn.
    // ⚠ MEASURED AND OFF. The premise that four of six civilizations never
    // build a wonder is false — 91.6% of deployment seats finish one, 6.54 a
    // seat, and the Culture disjunct beside the identity clause is what opens
    // the lane — and the gene's own 462-seat batch is `~` on both axes. The
    // numbers, and the live corpus that says the same lane is open there too,
    // are in
    // `docs/eval/2026-08-24-the-wonder-lane-is-already-open-in-both-regimes.md`;
    // the row is kept so the next standard screen prices it for free.
    // Appended at the END so a running screen keeps its positional genome. See
    // `AdvancedAi::wonder_score_tally`.
    Gene { tag: "wonder-score-tally", field: "wonder_score_tally", kind: Kind::OptIn, enable: AdvancedAi::enable_wonder_score_tally, disable: AdvancedAi::disable_wonder_score_tally },
    // Operator goal 2026-08-24: "a heuristic for exploring with missionaries
    // with 1 charge remaining". The third charge deletes the unit, so a
    // four-move, border-ignoring Missionary explores the fog within ten
    // tiles for up to twelve turns before spending it — unless a city of ours
    // is slipping or an untouched city stands beside it. See
    // `AdvancedAi::missionary_last_charge_explores`.
    Gene { tag: "missionary-last-charge-explores", field: "missionary_last_charge_explores", kind: Kind::OptIn, enable: AdvancedAi::enable_missionary_last_charge_explores, disable: AdvancedAi::disable_missionary_last_charge_explores },
    // Same goal: "missionaries should be smart enough to evade barbarians
    // using their fast movement". Since 2026-08-24 the barbarian seat walks
    // onto religious units and condemns them (`BasicAi::barbarian_heretic_
    // hunt`); with this gene a religious unit steps out of, and never steps
    // into, the exact tiles a visible raider can reach next turn
    // (`Game::threat_reach`). See `AdvancedAi::missionary_evades_raiders`.
    Gene { tag: "missionary-evades-raiders", field: "missionary_evades_raiders", kind: Kind::OptIn, enable: AdvancedAi::enable_missionary_evades_raiders, disable: AdvancedAi::disable_missionary_evades_raiders },
    // Operator goal 2026-08-24: "a gene for defending against religion, to
    // the extent we care about that". A religious victory needs more than
    // half of EVERY living major's cities, so each civilization is a veto;
    // the gene reads how much of a rival's victory is done (civs held +
    // progress toward half of ours), names and targets that faith from half
    // a victory and scales the defensive corps and the reserve by it from
    // match point, never withholding the shipped defence; the Inquisitor
    // goes to the heresy. See `AdvancedAi::religious_veto_defence`.
    Gene { tag: "religious-veto-defence", field: "religious_veto_defence", kind: Kind::OptIn, enable: AdvancedAi::enable_religious_veto_defence, disable: AdvancedAi::disable_religious_veto_defence },
    // Operator 2026-08-24: "very bad deals and just giving stuff away when
    // more optimally we'd get more in exchange". Three leaks, one gene each.
    // The trade objective was FAIRNESS: `bilateral_trade` picked the quote
    // maximising `min(our gain, their gain)`, discarding the ordering
    // `quick_deals` already produced by our gain. See
    // `BasicAi::deals_for_our_gain`.
    Gene { tag: "deals-for-our-gain", field: "deals_for_our_gain", kind: Kind::OptIn, enable: AdvancedAi::enable_deals_for_our_gain, disable: AdvancedAi::disable_deals_for_our_gain },
    // Every peacetime quote split the surplus down the middle
    // (`quote_asset_trade`); the war-eve lane prices at the buyer's ceiling
    // and proves the engine can. The chosen quote's Gold moves to the
    // counterparty's walk-away less two; the midpoint is the fallback. See
    // `BasicAi::deals_at_the_ceiling`.
    Gene { tag: "deals-at-the-ceiling", field: "deals_at_the_ceiling", kind: Kind::OptIn, enable: AdvancedAi::enable_deals_at_the_ceiling, disable: AdvancedAi::disable_deals_at_the_ceiling },
    // Both alliance proposals bundled one-way Open Borders once Early Empire
    // was in, and `do_accept_deal` grants them proposer → recipient: passage
    // through the whole empire for nothing, on every friendship ask. See
    // `BasicAi::no_free_passage`.
    Gene { tag: "no-free-passage", field: "no_free_passage", kind: Kind::OptIn, enable: AdvancedAi::enable_no_free_passage, disable: AdvancedAi::disable_no_free_passage },
    // Operator directive 2026-08-24: "one war at a time — some defences on
    // the home front, the rest concentrated on a single war; fight while
    // benefit > cost, sue for peace when the tide is no longer in our
    // favour consistently". Every offensive opening already refuses a second
    // front; this decides what happens once a second war ARRIVES — peace on
    // every front but one, the plan and the force planner held to that one,
    // the fatigue clause stood down while a city is breaking or tiles are in
    // reach, and peace sued for once the war ledger's exchange has run
    // against us for `ONE_WAR_TIDE_PATIENCE` turns. See
    // `AdvancedAi::one_war_observe` and `advanced/one_war.rs`.
    Gene { tag: "one-war-at-a-time", field: "one_war_at_a_time", kind: Kind::OptIn, enable: AdvancedAi::enable_one_war_at_a_time, disable: AdvancedAi::disable_one_war_at_a_time },
    // The envoy scorer prices where a city-state is and whose it is: a
    // suzerain's land heals as home and its client fights the suzerain's wars.
    Gene { tag: "flip-nearby-city-states", field: "flip_nearby_city_states", kind: Kind::OptIn, enable: AdvancedAi::enable_flip_nearby_city_states, disable: AdvancedAi::disable_flip_nearby_city_states },
    Gene { tag: "settler-screen", field: "settler_screen", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_screen, disable: AdvancedAi::disable_settler_screen },
    Gene { tag: "pass-picket", field: "pass_picket", kind: Kind::OptIn, enable: AdvancedAi::enable_pass_picket, disable: AdvancedAi::disable_pass_picket },
    // Run civvis-20260824T204654Z opened trade capacity around turn 17 and
    // still held zero Traders and zero routes at turn 65, by then at -6 Gold
    // per turn. The safety gate asked whether EVERY city was quiet, so a
    // remote coastal alarm vetoed a safe capital's route. Reserve the first
    // usable empty slot after immediate local defence and test the producing
    // origin itself; ordinary and frozen controllers keep the global veto.
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "solvency-first-trade-slot", field: "solvency_first_trade_slot", kind: Kind::OptIn, enable: AdvancedAi::enable_solvency_first_trade_slot, disable: AdvancedAi::disable_solvency_first_trade_slot },
    // 2026-08-24 operator goal: "much stronger tactical smarts and planning
    // for taking enemy cities, particularly for weaker enemies … analyze
    // neighboring enemies' military strength (public information) and their
    // progress in science … pick a city or two or more that we can likely
    // take and hold … units to spare … quickly pillage the tiles we can if
    // we have time". Two genes, `advanced/city_campaign.rs`.
    // The neighbour appraised on public power and tech count, the holdable
    // city priced with the spare, the launch on the city's own bill.
    Gene { tag: "city-campaign", field: "city_campaign", kind: Kind::OptIn, enable: AdvancedAi::enable_city_campaign, disable: AdvancedAi::disable_city_campaign },
    // A soldier at war pillages with the movement its march does not use.
    Gene { tag: "campaign-pillage", field: "campaign_pillage", kind: Kind::OptIn, enable: AdvancedAi::enable_campaign_pillage, disable: AdvancedAi::disable_campaign_pillage },
    // Run civvis-20260824T204654Z had a useful wide early pipeline but offered
    // its next Settlers from factories finishing in roughly 3, 7 and 29 turns,
    // while walkers repeatedly held or lost a site to another founding. Keep
    // `settler_in_flight_allowed` unchanged; admit only competitive factories
    // and reserve a distinct, lawful route and site for each live or queued
    // Settler. Appended at the END so a running screen keeps its positional
    // genome.
    Gene { tag: "settler-factory-coordination", field: "settler_factory_coordination", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_factory_coordination, disable: AdvancedAi::disable_settler_factory_coordination },
    // Run civvis-20260824T204654Z let offshore barbarian hulls read as the
    // same emergency as land raiders: they preempted city queues and recruited
    // land defenders even where coastline and attack domain gave them no
    // serious next-turn target. Price the engine's terrain-accurate attack
    // envelope against actual assets; harmless hulls create no alarm or chase,
    // but a ranged unit already holding a legal shot gets a bounded XP credit.
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "naval-threat-triage", field: "naval_threat_triage", kind: Kind::OptIn, enable: AdvancedAi::enable_naval_threat_triage, disable: AdvancedAi::disable_naval_threat_triage },
    // A surprise declaration arrives after the aggressor has chosen the
    // timing, while the target still carries peaceful queues, policies and
    // old units. For six Standard-speed turns, buy one frontline land body,
    // modernize, prioritize land-unit production cards, and redirect at most
    // half the empire's unfinished non-Settler queues toward the fastest
    // credible defenders until live plus queued land force reaches two per
    // city. The declaration direction and hard expiry keep this distinct from
    // the removed permanent war-economy routing. See
    // `advanced/surprise_defense.rs`.
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "surprise-war-mobilization", field: "surprise_war_mobilization", kind: Kind::OptIn, enable: AdvancedAi::enable_surprise_war_mobilization, disable: AdvancedAi::disable_surprise_war_mobilization },
    // Operator 2026-08-24: the live seat led the field in science (334 a turn,
    // 71 techs) and never ran a launch project — the stock horizon refused
    // the race from t150 at the city's raw production, ignoring the engine's
    // +100% on Spaceport projects, and nothing built production for it. A
    // seat that dominates science drives the race: the chain beeline, the
    // launch city's zone chain, a horizon priced as the engine runs it, two
    // pads by the Earth Satellite. Pinned on by the operator before its
    // first screen. Appended at the END so a running screen keeps its
    // positional genome.
    Gene { tag: "science-victory-drive", field: "science_victory_drive", kind: Kind::OptIn, enable: AdvancedAi::enable_science_victory_drive, disable: AdvancedAi::disable_science_victory_drive },
    // Over 218 completed live runs every one of the nine wins came from an
    // empire holding four to six cities at turn 60, and none of the 128 runs
    // outside that band won (Fisher p = 2.6e-4); the median loss loses the
    // lead for good at turn 61. The target is not the problem — `assess`
    // already asks for seven cities by then — and neither is the price, which
    // a 96-game census settled by scoring the Settler at four times its value
    // and leaving 97% of games identical. The pipeline is: one walker at a
    // time, and both flags that widen it are host-only. Appended at the END so
    // a running screen keeps its positional genome.
    Gene { tag: "expansion-schedule", field: "expansion_schedule", kind: Kind::OptIn, enable: AdvancedAi::enable_expansion_schedule, disable: AdvancedAi::disable_expansion_schedule },
    // The other half of the same defect, named in `production_value`'s own
    // note: a Settler needs a city at population two, and there is none on
    // 23.8% of seat-turns — "a growth constraint no price can buy past". The
    // citizen governor reads one empire scalar the engine never writes.
    // Scoped to the opening, to being behind the pace, and to an empire that
    // genuinely cannot build, because the unscoped version of this idea
    // (`city_strategy`) measured Elo -53 by building a smaller empire.
    Gene { tag: "growth-to-settle", field: "growth_to_settle", kind: Kind::OptIn, enable: AdvancedAi::enable_growth_to_settle, disable: AdvancedAi::disable_growth_to_settle },
    // 37.8% of recorded turns carry at least one refused order (`produce`
    // 24.9%, `trade` 67.3%) while the seat's judgement at matched states is
    // intact — it loses by not landing orders. Of the ~137 sites that discard
    // an `apply` error, not one tries an alternative. This is the repair
    // `docs/AI_GAPS.md` names after `fog-honest-2` lost with the ambitious
    // one: skip only the refused decision, no re-plan and no early EndTurn.
    Gene { tag: "order-retry", field: "order_retry", kind: Kind::OptIn, enable: AdvancedAi::enable_order_retry, disable: AdvancedAi::disable_order_retry },
    // The same shape one layer down: `builder_step` ranks every improvable
    // owned tile by straight-line `wdist`, then asks `step_toward` for the
    // single nearest one and returns `false` if it refuses. A ridge, a zone of
    // control or a unit in the way is enough, and the sweep re-picks the same
    // unreachable tile every turn after. Census at the deployment genome:
    // 22 Builders stood still 25+ turns across 8 games; at t180, 25 Builders
    // were alive holding 73 charges with NOT ONE of them in an empire that had
    // run out of improvable tiles. One caught directly -- builder 312, seat 5,
    // 30 turns at (10, 29), 3 charges, work 5 tiles away on its own landmass.
    // The gene tries the next-nearest candidate instead of the turn.
    Gene { tag: "builder-tries-the-next-tile", field: "builder_tries_the_next_tile", kind: Kind::OptIn, enable: AdvancedAi::enable_builder_tries_the_next_tile, disable: AdvancedAi::disable_builder_tries_the_next_tile },
    // Before the capital exists, move the starting Warrior before the Settler
    // and score only city footprints that the player's sight has fully
    // observed. The target cache is invalidated after that recon turn, so a
    // later opening turn can improve the choice with new terrain too.
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "opening-warrior-recon", field: "opening_warrior_recon", kind: Kind::OptIn, enable: AdvancedAi::enable_opening_warrior_recon, disable: AdvancedAi::disable_opening_warrior_recon },
    // A Settler normally has two movement points. After its first actual move,
    // throw away its cached destination and choose the remaining leg from the
    // newly current board without discarding long-lived safety history.
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "settler-second-look", field: "settler_second_look", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_second_look, disable: AdvancedAi::disable_settler_second_look },
    // ⚠ Settlers are lost by CAPTURE — a raider stepping onto the tile —
    // and in a native game the settler's capture model is a geometric disk
    // priced as a soft score under the MILITARY model, its retreat block and
    // the stacked-guard system both host-only; the builder's exact flood
    // (`builder-barbarian-safety`) credits no guard and never flees a job.
    // Eight settlers were taken in 104 turns on civvis-20260821T130446Z,
    // both of civvis-20260815T081505Z's within a tile of their site, and a
    // run that loses settlers ends with half the cities. This gene reads
    // the tiles every visible raider could END its next move on
    // (`threat_reach`; barbarians move after us in the same world turn), and
    // a Settler or Builder flees such a tile first, never steps into one
    // alone, and — the settler — summons the nearest healthy land unit onto
    // its own tile and walks with it: a stacked civilian cannot be taken.
    // Appended at the END so a running screen keeps its positional genome.
    // See `AdvancedAi::civilian_out_of_reach` / `advanced/civilian_safety.rs`.
    Gene { tag: "civilian-out-of-reach", field: "civilian_out_of_reach", kind: Kind::OptIn, enable: AdvancedAi::enable_civilian_out_of_reach, disable: AdvancedAi::disable_civilian_out_of_reach },
    // ⭐ THREE DEITY HABITS (operator, 2026-08-24: "study expert level deity
    // civ 6 tips and tricks and implement the best as heuristics"). The engine
    // has offered `chop_woods` / `chop_rainforest` / `clear_marsh` through
    // `Game::builder_operations` since the feature-removal tables shipped,
    // paying the shipped yield scaled by the world era and Magnus; no agent
    // ever asked for one. A Deity player chops into the Settler, the district
    // and the wonder. The chop joins the Builder's job list wherever the
    // owning city's queue front is one of those, priced as a one-off lump.
    // Appended at the END so a running screen keeps its positional genome.
    // See `advanced/deity_habits.rs`.
    Gene { tag: "chop-into-the-queue", field: "chop_into_the_queue", kind: Kind::OptIn, enable: AdvancedAi::enable_chop_into_the_queue, disable: AdvancedAi::disable_chop_into_the_queue },
    // Sixty-two technologies and fifty-three civics carry a boost worth 40%
    // of their cost; `tech_value` pays +28 for a boost in hand and nothing
    // ever earned one. A Deity player builds the quarry for Masonry, the
    // pasture for Horseback Riding, the sixth farm for Feudalism. The
    // improvement that completes a trigger is worth the research it grants,
    // spread over the steps the trigger still needs. Appended at the END so
    // a running screen keeps its positional genome. See
    // `advanced/deity_habits.rs`.
    Gene { tag: "eureka-chasing-builder", field: "eureka_chasing_builder", kind: Kind::OptIn, enable: AdvancedAi::enable_eureka_chasing_builder, disable: AdvancedAi::disable_eureka_chasing_builder },
    // The same boost table read by the production queue: two Galleys for
    // Shipbuilding, three Archers for Machinery, Walls for Engineering, an
    // Aqueduct for Military Engineering, two Markets for Guilds. The unit,
    // building or district that completes a trigger is worth the research it
    // grants on `production_value`'s raw scale. Appended at the END so a
    // running screen keeps its positional genome. See
    // `advanced/deity_habits.rs`.
    Gene { tag: "eureka-chasing-production", field: "eureka_chasing_production", kind: Kind::OptIn, enable: AdvancedAi::enable_eureka_chasing_production, disable: AdvancedAi::disable_eureka_chasing_production },

    // The war desk prepares an army and nothing else: an appointed war spends
    // its phases on a tech, a package and a march, an elective one waits for
    // `ready && staged`, and neither spends a turn of the lead on who else
    // will be fighting the target. `propose_strategic_alliance` picks its
    // partner from OUR strategy on a 12-turn cadence, the envoy scorer reads
    // a city-state's place only against our own cities, and
    // `Action::ProposeJointWar` -- the one action that makes a second empire
    // declare the turn we do -- is issued by no controller at all. The gene
    // opens a coalition window from the turn a target is held: an alliance a
    // turn with the target's neighbours (military first), envoys to the
    // city-states beside the target (a suzerain's clients fight its wars),
    // and joint-war invitations the turn the desk would declare, holding the
    // declaration while an answer is due. Operator request, 2026-08-25.
    Gene { tag: "coalition-before-war", field: "coalition_before_war", kind: Kind::OptIn, enable: AdvancedAi::enable_coalition_before_war, disable: AdvancedAi::disable_coalition_before_war },
    // Version 2 of `amenity-project-preemption` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_amenity_project_preemption_2`.
    Gene { tag: "amenity-project-preemption-2", field: "amenity_project_preemption_2", kind: Kind::OptIn, enable: AdvancedAi::enable_amenity_project_preemption_2, disable: AdvancedAi::disable_amenity_project_preemption_2 },
    // Version 2 of `campus-adjacency-threshold` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_campus_adjacency_threshold_2`.
    Gene { tag: "campus-adjacency-threshold-2", field: "campus_adjacency_threshold_2", kind: Kind::OptIn, enable: AdvancedAi::enable_campus_adjacency_threshold_2, disable: AdvancedAi::disable_campus_adjacency_threshold_2 },
    // Version 2 of `district-coverage` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_district_coverage_2`.
    Gene { tag: "district-coverage-2", field: "district_coverage_2", kind: Kind::OptIn, enable: AdvancedAi::enable_district_coverage_2, disable: AdvancedAi::disable_district_coverage_2 },
    // A damaged religious corps may hold one Guru for its only field heal.
    // See `AdvancedAi::enable_guru_heals_the_corps_2`.
    Gene { tag: "guru-heals-the-corps-2", field: "guru_heals_the_corps_2", kind: Kind::OptIn, enable: AdvancedAi::enable_guru_heals_the_corps_2, disable: AdvancedAi::disable_guru_heals_the_corps_2 },
    // Version 2 of `holy-site-where-the-threat-is` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_holy_site_where_the_threat_is_2`.
    Gene { tag: "holy-site-where-the-threat-is-2", field: "holy_site_where_the_threat_is_2", kind: Kind::OptIn, enable: AdvancedAi::enable_holy_site_where_the_threat_is_2, disable: AdvancedAi::disable_holy_site_where_the_threat_is_2 },
    // Version 2 of `naval-recon` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_naval_recon_2`.
    Gene { tag: "naval-recon-2", field: "naval_recon_2", kind: Kind::OptIn, enable: AdvancedAi::enable_naval_recon_2, disable: AdvancedAi::disable_naval_recon_2 },
    // Version 2 of `power-the-laboratory` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_power_the_laboratory_2`.
    Gene { tag: "power-the-laboratory-2", field: "power_the_laboratory_2", kind: Kind::OptIn, enable: AdvancedAi::enable_power_the_laboratory_2, disable: AdvancedAi::disable_power_the_laboratory_2 },
    // Version 2 of `settler-guard-holds` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_settler_guard_holds_2`.
    Gene { tag: "settler-guard-holds-2", field: "settler_guard_holds_2", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_guard_holds_2, disable: AdvancedAi::disable_settler_guard_holds_2 },
    // Version 2 of `settler-target-hysteresis` (2026-08-24): one gated delta on version 1; the
    // family draw is biased toward the best version and the ledger ships the
    // best by pooled Diff. See `AdvancedAi::enable_settler_target_hysteresis_2`.
    Gene { tag: "settler-target-hysteresis-2", field: "settler_target_hysteresis_2", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_target_hysteresis_2, disable: AdvancedAi::disable_settler_target_hysteresis_2 },
    // The first land unit within two tiles of an at-war city resets the
    // fatigue clock once, so a campaign still walking to its target is not
    // offered away as stalled. See `AdvancedAi::enable_siege_is_progress_2`.
    Gene { tag: "siege-is-progress-2", field: "siege_is_progress_2", kind: Kind::OptIn, enable: AdvancedAi::enable_siege_is_progress_2, disable: AdvancedAi::disable_siege_is_progress_2 },
    // Every tier-2 government is gated on one civic and `strategic_government`
    // already ranks all three above `classical_republic` in every lane. The
    // civic never arrives: past `political_philosophy` the `forced_goal` match
    // falls through to `civic_value`, whose `(value + 32) / sqrt(cost)` pays a
    // tier-2 gate 153 over sqrt(440) against Political Philosophy's three
    // governments plus a flat +70 over sqrt(110). Live King seat, 2026-08-25:
    // the three gate civics were reached in 0 of 59 games and all 24 readable
    // late snapshots were still Classical Republic -- four policy slots for the
    // whole game instead of six, and the one tier-1 government with no military
    // slot at all.
    Gene { tag: "government-ladder", field: "government_ladder", kind: Kind::OptIn, enable: AdvancedAi::enable_government_ladder, disable: AdvancedAi::disable_government_ladder },
    // The repair was written, documented, and half-applied. `research_debt`,
    // `culture_debt` and `research_coverage` are all multiplied by
    // `research_horizon`, a straight line to zero at the turn limit, while
    // `RESEARCH_CAMPUS_PAYBACK` and `campus_payback_horizon` sitting beside them
    // say what the code means to ask -- "whether it can REPAY, not what
    // fraction of the game is left" -- and the comment over `research_coverage`
    // claims outright to BE a payback horizon. Only `adjacency_threshold` was
    // ever migrated. 88% of standard Emperor games end on a science victory at
    // a median turn 193, where the game fraction is already down to 0.23: the
    // Library that decides the race is priced at under a quarter of its debt
    // exactly when the race is decided, against rival terms that never decay.
    Gene { tag: "chain-payback-window", field: "chain_payback_window", kind: Kind::OptIn, enable: AdvancedAi::enable_chain_payback_window, disable: AdvancedAi::disable_chain_payback_window },
    // The order is the bug, not the floor. `upgrade_units` ranks by strength
    // gained per gold and stops at a treasury floor, and `take_turn_inner` calls
    // it LAST -- after `advanced_gold_spending` has bought until the bank hits
    // its reserve, so what survives is by construction about equal to that
    // reserve and only the cheapest upgrades clear the floor. An upgrade costs
    // base plus twice the production difference; buying the same unit fresh
    // costs four times its production, so the inversion spends several times
    // the gold for the same strength. Live King seat, 2026-08-25: about three
    // UPGRADE orders per whole game, Heavy Chariots still the commonest unit at
    // turn 150, and 28 of 40 lost cities lost to conquest.
    Gene { tag: "upgrade-the-garrison", field: "upgrade_the_garrison", kind: Kind::OptIn, enable: AdvancedAi::enable_upgrade_the_garrison, disable: AdvancedAi::disable_upgrade_the_garrison },
    // `advanced_production` ranks the menu, walks it while the score clears
    // -1,000, and then `if let Some(..)` -- with no `else`. A city whose whole
    // menu is priced at a refusal sentinel produces NOTHING, silently, with no
    // journal line. `BasicAi::pick_item` returns `None` the same way out of its
    // economic-recovery arm and its tail, and its caller has no `else` either.
    // Live King seat, 2026-08-25: 3,094 of 36,975 city-turns before turn 104
    // carried no production item at all, 8.4% of the early empire's output.
    Gene { tag: "never-an-empty-queue", field: "never_an_empty_queue", kind: Kind::OptIn, enable: AdvancedAi::enable_never_an_empty_queue, disable: AdvancedAi::disable_never_an_empty_queue },
    // `BasicAi::best_improvement` pays `spec.housing * 2.0`; the advanced
    // chooser the deployed agent uses never reads `spec.housing` at all, so it
    // is strictly blinder than the baseline about the thing that caps a city's
    // growth. Seventeen improvements carry Housing, counted within three tiles
    // of the centre whether or not the tile is worked. Population is the
    // largest single source of a city's science -- 3.5 of 9.3 beakers on the
    // live seat, against the Campus's own 2.1 -- and 88% of standard Emperor
    // games end on science. Live King seat: 58% of cities at their housing
    // ceiling at turn 100, 13% of owned land improved.
    Gene { tag: "improvement-housing-value", field: "improvement_housing_value", kind: Kind::OptIn, enable: AdvancedAi::enable_improvement_housing_value, disable: AdvancedAi::disable_improvement_housing_value },
    // Two Builder quotas exist and the measured one is not the one that binds.
    // `production_builder_floor` raises 0.5 to 0.75 inside `delegated_cities`,
    // the baseline governor the strategic path reaches only for a city it has
    // already left empty; the quota that decides almost every Builder is
    // hardcoded in `production_value`'s own arm as `city_count.div_ceil(2)` and
    // has never been screened. Its 260 also loses to a monument's flat 240 plus
    // yields and to a district's `balanced_core` 130 plus yields x 60. Live
    // King seat, 2026-08-25: Builders 3% of early city production against
    // Settlers' 22%, 13% of owned land improved by turn 100.
    Gene { tag: "builder-supply-floor", field: "builder_supply_floor", kind: Kind::OptIn, enable: AdvancedAi::enable_builder_supply_floor, disable: AdvancedAi::disable_builder_supply_floor },
    // Version two of `chain-payback-window`, which moved all three chain terms
    // to the payback horizon and whose 24-game probe came back negative on both
    // axes. The three are not the same purchase: `research_debt` and
    // `culture_debt` are a Library and an Amphitheater owed to a district
    // already paid for -- the case `RESEARCH_CAMPUS_PAYBACK` was written for --
    // while `research_coverage` is 300 points for a whole Campus in a city with
    // none, and holding THAT at full value to within forty turns of the end
    // lets it outbid the Spaceport that actually ends the game. Version two
    // takes the repaired horizon for the cheap rung only.
    Gene { tag: "chain-payback-window-2", field: "chain_payback_window_2", kind: Kind::OptIn, enable: AdvancedAi::enable_chain_payback_window_2, disable: AdvancedAi::disable_chain_payback_window_2 },
    // Version two of `never-an-empty-queue`, whose 24-game probe read negative
    // on both axes. The preference version one carries cannot bind: the scorer
    // issues -10,000 for a hard veto and -2,000 for a soft one, and every soft
    // refusal it actually issues is a UNIT -- a saturated domain, a second
    // Scout, a body weaker than the best in its role. So an idle city has its
    // infrastructure at -10,000 and soldiers at -2,000, and "prefer
    // infrastructure" never fires: version one answers an idle turn with a
    // surplus soldier the empire owes upkeep on. Version two lets a Builder,
    // Trader, Settler, building, district or project answer the turn and lets a
    // refused soldier not.
    Gene { tag: "never-an-empty-queue-2", field: "never_an_empty_queue_2", kind: Kind::OptIn, enable: AdvancedAi::enable_never_an_empty_queue_2, disable: AdvancedAi::disable_never_an_empty_queue_2 },
    // The measured pattern, applied where it should also hold. The 2026-08-25
    // King-rung screen priced `solvency-first-trade-slot` at +4.04 pp wins
    // (z +3.02) and +1.09 pp share (z +4.65) -- helps on both axes -- while its
    // own version two, which fills EVERY empty slot instead of the first,
    // measured -1.93 pp on the same games. The win is buying ONE compounding
    // asset ahead of an argmax that will never choose it, not buying more of
    // it. A Builder is priced 260 and a Library about 960 against a Settler's
    // 1,560 by that same argmax; live King seat, Builders were 3% of early city
    // production and only 40% of Campus cities held a Library at turn 100.
    Gene { tag: "first-builder-reserve", field: "first_builder_reserve", kind: Kind::OptIn, enable: AdvancedAi::enable_first_builder_reserve, disable: AdvancedAi::disable_first_builder_reserve },
    // The same sentence for the cheap rung of the research chain, in a city
    // that has already paid for the Campus. 91% of standard Emperor games end
    // on a science victory, so this is the win condition itself.
    Gene { tag: "first-research-building-reserve", field: "first_research_building_reserve", kind: Kind::OptIn, enable: AdvancedAi::enable_first_research_building_reserve, disable: AdvancedAi::disable_first_research_building_reserve },
    // A camp's Scout reports the nearest major settlement it sees and the
    // camp raises its party against that report, so a camp nearer a rival's
    // city than ours raids the rival -- and five deliberate clears (the
    // adjacent clear, the camp errand, the barbarian-response threat list, the
    // near-home chase, the presence alarm) took it down anyway, each
    // measuring distance from OUR cities only. The envoy scorer and the
    // alliance partner score likewise read a city-state's or a major's place
    // against our own cities, so the one ACROSS a rival -- the front that
    // rival cannot cover while facing us -- scored nothing for being there.
    // The gene leaves the neighbours' camps standing, pays envoys and
    // alliance partners on the far side of every rival, and refuses a
    // rival's joint war against the major on its far side. Operator
    // request, 2026-08-25.
    Gene { tag: "enemy-of-my-enemy", field: "enemy_of_my_enemy", kind: Kind::OptIn, enable: AdvancedAi::enable_enemy_of_my_enemy, disable: AdvancedAi::disable_enemy_of_my_enemy },
    // A Barbarian Outpost inside a city's own three rings is bought out. The
    // shipped plot appraisal prices a camp hex as its bare terrain, so the
    // one plot in the empire whose ownership is about a threat is the one
    // plot Gold never reaches; and Culture will not reach it either, because
    // `border_influence_cost` charges a camp the shipped +100, a whole ring
    // of distance, and the city grows around the hole instead. The gene buys
    // it when being rid of the outpost is worth more than the quote — the
    // outpost visible, and a soldier of ours in reach to walk in and
    // disperse it, since Civilization VI removes an outpost on entry and
    // never because the deed changed hands. Operator request, 2026-08-25.
    Gene { tag: "camp-tile-buyout", field: "camp_tile_buyout", kind: Kind::OptIn, enable: AdvancedAi::enable_camp_tile_buyout, disable: AdvancedAi::disable_camp_tile_buyout },
    // 2026-08-25, run civvis-20260825T162542Z: Mount Roraima three tiles from
    // Rome, every Settler walked the other way. The site model read the
    // wonder as lost jobs and the ground beside it was never scouted. See
    // `advanced/wonder_sites.rs`.
    Gene { tag: "wonder-adjacent-sites", field: "wonder_adjacent_sites", kind: Kind::OptIn, enable: AdvancedAi::enable_wonder_adjacent_sites, disable: AdvancedAi::disable_wonder_adjacent_sites },
    // Version 2 adds a small flat footprint credit on top of the projection;
    // kept a separate version because #1419's flat wonder credit lost at
    // scale (#2464), so the batch prices the credit apart from the repair.
    Gene { tag: "wonder-adjacent-sites-2", field: "wonder_adjacent_sites_2", kind: Kind::OptIn, enable: AdvancedAi::enable_wonder_adjacent_sites_2, disable: AdvancedAi::disable_wonder_adjacent_sites_2 },
    Gene { tag: "wonder-ring-recon", field: "wonder_ring_recon", kind: Kind::OptIn, enable: AdvancedAi::enable_wonder_ring_recon, disable: AdvancedAi::disable_wonder_ring_recon },
    // ⭐ RAPID CITY EXPANSION (recovered 2026-08-25 from the branch stranded by
    // #2306; the implementation landed 21 minutes after that PR was closed as
    // "empty" and was never reviewed). Settler-first at the legal population
    // floor, a shared multi-Settler pipeline, safe nearby sites before any war,
    // and Conquest only once that practical frontier is full. Registered here
    // because `advanced/treatments.rs`, which the branch wrote its row into,
    // was deleted by the 2026-08-23 registry cleanup. Appended at the END so a
    // running screen keeps its positional genome.
    Gene { tag: "rapid-city-expansion", field: "rapid_city_expansion", kind: Kind::OptIn, enable: AdvancedAi::enable_rapid_city_expansion, disable: AdvancedAi::disable_rapid_city_expansion },
    // Three site terms run from every neighbour (six a tile under six from
    // a foreign city, four a tile for rival ground within three, isolation
    // on top) and nothing pays for the one thing the ground between two
    // empires has that the ground behind us does not: a rival's Settler is
    // walking toward it. While our military power is at least 0.8 of the
    // strongest neighbour's and two land units stand, a site between us and
    // a met neighbour within eight of their city earns 10 plus 3 a tile of
    // the gap it closes (cap 28) and pays no border provocation; an own city
    // within eight of a neighbour's is a frontier city whose first Walls are
    // worth 240 more and which holds a peacetime garrison. When no contested
    // site is left, the shipped ranking takes the uncontested one. Operator
    // request, 2026-08-25.
    Gene { tag: "contested-land-first", field: "contested_land_first", kind: Kind::OptIn, enable: AdvancedAi::enable_contested_land_first, disable: AdvancedAi::disable_contested_land_first },
    // Operator heuristic, 2026-08-25: Production is spent best on what a
    // slotted card boosts; Gold is flexible and immediate, so it belongs on
    // what no card boosts, on the young city that cannot yet build, and on
    // emergencies. The engine prices every card in `Game::item_prod_mult`
    // and NO controller read it: `gold_purchase_score` divides the remaining
    // cost by the raw yield, so a Settler under Colonization looks slower
    // than it is and becomes a BETTER purchase — the shipped purchaser buys
    // exactly what the deck already discounts. The four rows below each own
    // one decision, so the screen can price them apart; all in
    // `advanced/gold_and_cards.rs`. Appended at the END so a running screen
    // keeps its positional genome.
    Gene { tag: "buy-what-cards-cannot-boost", field: "buy_what_cards_cannot_boost", kind: Kind::OptIn, enable: AdvancedAi::enable_buy_what_cards_cannot_boost, disable: AdvancedAi::disable_buy_what_cards_cannot_boost },
    // The production side of the same heuristic: a positive governor value
    // leans toward the item the slotted deck makes cheap, half the card's
    // bonus, capped at a doubling. A reorder, never a spend — a value at or
    // below zero is not raised.
    Gene { tag: "build-what-cards-boost", field: "build_what_cards_boost", kind: Kind::OptIn, enable: AdvancedAi::enable_build_what_cards_boost, disable: AdvancedAi::disable_build_what_cards_boost },
    // "A new city will always have low production but has access to the
    // same money as anywhere else": a purchase in a city below the empire's
    // best producer earns a premium proportional to the deficit, +50% at
    // zero output. The shipped scorer already prices turns at the city's
    // raw yield; this is the explicit lean on top of it.
    Gene { tag: "gold-for-the-young-city", field: "gold_for_the_young_city", kind: Kind::OptIn, enable: AdvancedAi::enable_gold_for_the_young_city, disable: AdvancedAi::disable_gold_for_the_young_city },
    // `emergency_city_defense_purchase` is gated on `garrison_under_fire`,
    // which only the live bridge sets — on a native board it has never fired
    // once. The gene adds the native signal: a city that lost health, was
    // struck within four turns, and has a hostile military unit within three
    // tiles buys Walls or the best land defender through the reserve. Damage,
    // not a hostile in sight: `besieged_city_item` records that reacting to
    // one raider in range cost score.
    Gene { tag: "native-emergency-purchase", field: "native_emergency_purchase", kind: Kind::OptIn, enable: AdvancedAi::enable_native_emergency_purchase, disable: AdvancedAi::disable_native_emergency_purchase },
    // Operator request, 2026-08-26: play the city-state quests for the Envoy
    // they pay. `src/game/quests.rs` has modelled the eight shipped
    // `Quests.xml` rows since #430 — every met city-state asks each
    // civilization for one thing and pays `envoys_free += 1` when it is done
    // — and NO controller has ever read one: `grep city_state_quest src/ai`
    // was empty. The four rows below each own one decision surface that can
    // already satisfy a quest without knowing it is being asked, so the
    // screen can price them apart; every one is a reorder rather than a
    // spend. All in `advanced/city_state_quests.rs`. Appended at the END so a
    // running screen keeps its positional genome.
    Gene { tag: "quest-production", field: "quest_production", kind: Kind::OptIn, enable: AdvancedAi::enable_quest_production, disable: AdvancedAi::disable_quest_production },
    // The Trader's destination score carries the Envoy of a `send_trade_route`
    // quest, on every city of the city-state asking, so the ordinary yield
    // terms still choose between them.
    Gene { tag: "quest-trade-route", field: "quest_trade_route", kind: Kind::OptIn, enable: AdvancedAi::enable_quest_trade_route, disable: AdvancedAi::disable_quest_trade_route },
    // Civilization VI names ONE outpost and pays for that one only, so the
    // camp errand prefers the named camp over a nearer unnamed one and
    // reaches six tiles further out for it.
    Gene { tag: "quest-camp-errand", field: "quest_camp_errand", kind: Kind::OptIn, enable: AdvancedAi::enable_quest_camp_errand, disable: AdvancedAi::disable_quest_camp_errand },
    // A boost quest is paid by the trigger, so the Envoy rides on the same
    // table `eureka-chasing-production` reads — the Envoy beside that gene's
    // research, and independent of it.
    Gene { tag: "quest-boost", field: "quest_boost", kind: Kind::OptIn, enable: AdvancedAi::enable_quest_boost, disable: AdvancedAi::disable_quest_boost },
    // Every adaptive seat pursues a religion unconditionally -- `take_turn_inner`
    // reads `active_victory_target.is_none()`, and a screen seat has no target --
    // and nothing weighs that against the science race it competes with.
    // Measured over a 12,000-seat probe at seeds 95000000.., 89% science
    // endings: founders won 14.5% (n=8,000) against 20.9% (n=4,000) for
    // non-founders, a 6.4 pp gap on a binary two thirds of seats perform. It
    // survives stratification by empire size and WIDENS with it (-2.4 pp at
    // five cities, -16.3 at eight), and founders end five techs behind.
    Gene { tag: "skip-the-prophet-race", field: "skip_the_prophet_race", kind: Kind::OptIn, enable: AdvancedAi::enable_skip_the_prophet_race, disable: AdvancedAi::disable_skip_the_prophet_race },
    // Buildings are picked cheapest-first, so the Library — the win condition in
    // a regime that ends 89% science — queues behind every cheaper building.
    Gene { tag: "science-building-first", field: "science_building_first", kind: Kind::OptIn, enable: AdvancedAi::enable_science_building_first, disable: AdvancedAi::disable_science_building_first },

    // Sixty-two technologies and fifty-three civics carry a boost worth 40% of
    // their cost, and the whole of the agent's opinion about the order they
    // are taken in was one flat `+28` in `tech_value` and `civic_value`. Both
    // functions end `(value + k) / cost.sqrt()` -- they rank value per root
    // beaker -- and a boost makes the node cost `1 - frac` of its printed
    // price, so the score it buys is the old one times `1 / (1 - frac).sqrt()`,
    // 1.29 at the shipped 40%, with no free parameter. Operator request,
    // 2026-08-25. ⚠ The first cut ADDED the turns of research the boost saves
    // and its own probe resolved a score-share loss of -3.36 pp (z -3.21, run
    // resolves ±2.93): above a `sqrt(cost)` divisor of 7 for an ancient node,
    // an opening empire's two beakers a turn made every boost worth the
    // twelve-turn cap, and the gene took whatever was boosted rather than the
    // boosted one among comparable nodes. See
    // `docs/gene_screens/fires/boost-first-research-v1.json`. Appended at the
    // END so a running screen keeps its positional genome. See
    // `advanced/boost_research.rs`.
    Gene { tag: "boost-first-research", field: "boost_first_research", kind: Kind::OptIn, enable: AdvancedAi::enable_boost_first_research, disable: AdvancedAi::disable_boost_first_research },
    // The other half of the same fact: `Game`'s boost loop credits a boost
    // mid-research onto a node already being worked, and never onto one
    // already finished -- so a long node collects its eureka late and a short
    // one loses it outright. A node the empire would finish inside
    // `BOOST_WAIT_HORIZON_TURNS`, whose boost is still earnable by something
    // buildable, is docked the boost at risk scaled by how likely it is to
    // beat its own trigger home. Appended at the END so a running screen keeps
    // its positional genome. See `advanced/boost_research.rs`.
    Gene { tag: "boost-wait-research", field: "boost_wait_research", kind: Kind::OptIn, enable: AdvancedAi::enable_boost_wait_research, disable: AdvancedAi::disable_boost_wait_research },
    // Being intentional about earning the rest: `eureka-chasing-builder` and
    // `eureka-chasing-production` can only chase a trigger whose thing the
    // empire is already allowed to build, and nothing ever bought the
    // permission. Masonry's quarry wants Mining, Machinery's three Archers
    // want Archery, Guilds' two Markets want Currency. A node is credited the
    // boosts it makes chaseable, capped at `BOOST_UNLOCK_TURNS_CAP` turns.
    // Appended at the END so a running screen keeps its positional genome. See
    // `advanced/boost_research.rs`.
    Gene { tag: "boost-unlock-research", field: "boost_unlock_research", kind: Kind::OptIn, enable: AdvancedAi::enable_boost_unlock_research, disable: AdvancedAi::disable_boost_unlock_research },
    // ⭐ FIVE GENES FOR HARD POWER OVER THE MAP (operator, 2026-08-25:
    // "control over chokepoints — controlling with territory an important
    // narrow water passageway. placing a city on a single wide strip of land
    // to connect 2 bodies of water (through the city, for only me),
    // controlling important mountain passageways"). Nothing in this
    // controller has ever read the SHAPE of the map: a site is a bundle of
    // yields, a plot a bundle of yields, a district an adjacency. The four
    // engine facts these spend are quoted in the header of
    // `advanced/chokepoints.rs` — a city center is a naval passage only its
    // owner may enter (`class_can_traverse`, then `can_enter_past`'s city
    // arm); a border refuses entry without Open Borders and asks nothing
    // about the terrain, so owning the water of a strait closes it; an
    // unpillaged Encampment refuses a foreign unit with no exception at all,
    // war included; and one military body in a one-tile pass is a wall the
    // column has to break. Appended at the END so a running screen keeps its
    // positional genome.
    // A site is worth the passes and straits its own borders would cover.
    Gene { tag: "chokepoint-siting", field: "chokepoint_siting", kind: Kind::OptIn, enable: AdvancedAi::enable_chokepoint_siting, disable: AdvancedAi::disable_chokepoint_siting },
    // A city center on the strip of land between two seas joins them for our
    // fleet and for nobody else's, priced in the sea detour it saves.
    Gene { tag: "canal-city", field: "canal_city", kind: Kind::OptIn, enable: AdvancedAi::enable_canal_city, disable: AdvancedAi::disable_canal_city },
    // Gold buys the plot that closes a passage somebody else could use:
    // `expand_borders` is the engine's own influence picker and takes no
    // advice, so `BuyPlot` is the whole lever the seat has over a border.
    Gene { tag: "chokepoint-claim", field: "chokepoint_claim", kind: Kind::OptIn, enable: AdvancedAi::enable_chokepoint_claim, disable: AdvancedAi::disable_chokepoint_claim },
    // The Encampment lands on the pass, where it is a permanent wall.
    Gene { tag: "encampment-seals-the-pass", field: "encampment_seals_the_pass", kind: Kind::OptIn, enable: AdvancedAi::enable_encampment_seals_the_pass, disable: AdvancedAi::disable_encampment_seals_the_pass },
    // A surplus soldier — or a hull, for a strait — holds the gate on the
    // approach to one of our cities and fortifies. On the peacetime tail
    // alone: a stand-still posture in a major war screened NEGATIVELY at
    // 38,160 seats, which `advanced/field_craft.rs` records in its header.
    Gene { tag: "chokepoint-garrison", field: "chokepoint_garrison", kind: Kind::OptIn, enable: AdvancedAi::enable_chokepoint_garrison, disable: AdvancedAi::disable_chokepoint_garrison },
    // `strategic_government` chooses from a hand-written priority list per
    // lane, and a government missing from that list is invisible even when the
    // empire already owns its civic. Live King seat 2026-08-26 (run
    // `civvis-20260826T112920Z`): Corporate Libertarianism -- ten policy slots
    // -- was held from turn 222 and the seat played Classical Republic's four
    // to the end, because no lane list names it. Six of the thirteen
    // governments in `data/governments.json` appear in no Diplomacy or Culture
    // list at all. The lists rank comparable governments; they were never
    // meant to cap capacity.
    Gene { tag: "government-capacity-fallback", field: "government_capacity_fallback", kind: Kind::OptIn, enable: AdvancedAi::enable_government_capacity_fallback, disable: AdvancedAi::disable_government_capacity_fallback },
    // Version one of the ladder retires the moment any tier-2 gate civic
    // lands and refuses to climb past half the clock. The live King seat
    // (`civvis-20260826T112920Z`, 2026-08-26) shows which half of that is
    // binding: it reached NO tier-2 gate in 248 turns, so the retirement
    // clause never fired, while the gate civics that carry real policy
    // capacity all sit past turn 125 at Online speed. Version two ranks every
    // government whose gate it does not own and whose slots beat the one it is
    // playing, takes the cheapest such gate, and holds the window open to
    // three quarters of the clock for as long as a living major plays a
    // government with more slots than ours.
    Gene { tag: "government-ladder-2", field: "government_ladder_2", kind: Kind::OptIn, enable: AdvancedAi::enable_government_ladder_2, disable: AdvancedAi::disable_government_ladder_2 },
    // An explicit `victory_target` is a set of hard refusals, not a
    // preference: Great Work buildings score `-10_000` off the Culture lane,
    // space-race projects `-10_000` off the Science lane, the wonder gate
    // opens for Culture and Score alone, and `culture_spending` -- the only
    // Faith sink a religion-less empire has -- is dispatched on the Culture
    // lane only. Live King seat `civvis-20260826T112920Z`, 2026-08-26: 248
    // turns of `--victory diplomatic` at 2 diplomatic points against the
    // leader's 19, ending with no Museum in twelve cities, a Spaceport
    // finished at turn 225 with zero launches, 6,329 unspendable Faith, seven
    // wonders to the leader's thirteen, and a 448-point loss on score.
    Gene { tag: "lane-release-when-hopeless", field: "lane_release_when_hopeless", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_release_when_hopeless, disable: AdvancedAi::disable_lane_release_when_hopeless },
    // Every solvency guard in the agent runs AFTER the declaration:
    // `live_war_economy_requires_recovery` reprices production, the
    // `maintenance_emergency` arm swaps a policy card, `war_treasury_floor`
    // reserves upgrade gold. Live King seat `civvis-20260826T112920Z`,
    // 2026-08-26: 25 Gold at -11 a turn at turn 100, an elective declaration
    // on the Maori at turn 110 (`source: civvis`, twenty-two turns after
    // accepting peace with them), fifty consecutive turns at a zero treasury,
    // military power 218 -> 30 as unpaid units disbanded, and a bankruptcy
    // amenity penalty of 18 across nine cities. This asks the same reserve
    // question one turn earlier, where the answer can still prevent the war.
    Gene { tag: "war-needs-a-treasury", field: "war_needs_a_treasury", kind: Kind::OptIn, enable: AdvancedAi::enable_war_needs_a_treasury, disable: AdvancedAi::disable_war_needs_a_treasury },
    // The peace desk's fatigue clause is written `!appointed_objective &&
    // fatigued && ...`, so an appointed campaign that never lands is a
    // permanent exemption from peace. Live King seat
    // `civvis-20260826T112920Z`, 2026-08-26: 172 of 248 turns at war with the
    // Maori behind one objective appointed at turn 100 and never taken, while
    // an air surge stood appointed against a third empire at a zero treasury.
    // Every peace the seat did ask for -- four of four -- was accepted.
    Gene { tag: "peace-when-the-war-does-not-pay", field: "peace_when_war_does_not_pay", kind: Kind::OptIn, enable: AdvancedAi::enable_peace_when_war_does_not_pay, disable: AdvancedAi::disable_peace_when_war_does_not_pay },
    // `assess` sizes the city target from a land census and never asks
    // whether that many sites exist; every settler gate downstream reads the
    // target. Live King seat `civvis-20260826T112920Z`, 2026-08-26: target
    // sixteen, twelve cities, **168 idle settler-turns**, five Settlers parked
    // from turn 180 to turn 230 with nowhere to walk, and the two sites that
    // were founded late scored 39.1 and 47.8 against the capital's 186.2.
    Gene { tag: "city-target-meets-the-map", field: "city_target_meets_the_map", kind: Kind::OptIn, enable: AdvancedAi::enable_city_target_meets_the_map, disable: AdvancedAi::disable_city_target_meets_the_map },
    // The Congress ballot is sized from `plan.strategy`, the posture `assess`
    // chose this turn, while the lane the seat plays lives in
    // `victory_target`. They disagree most of the time: live King seat
    // `civvis-20260826T112920Z` was assigned Diplomacy for all 248 turns and
    // held a Diplomacy posture on 66 of them against Expansion's 96 and
    // Recovery's 80, so it cast ONE vote on thirteen of twenty-five ballots
    // while holding 74-223 Favor at `spent: 0`, and ended with 238 Favor
    // banked and 2 Diplomatic Victory Points against the leader's 19.
    Gene { tag: "lane-votes-its-favor", field: "lane_votes_its_favor", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_votes_its_favor, disable: AdvancedAi::disable_lane_votes_its_favor },
    // `pursue_religion` already discards the prize for a non-Religion lane,
    // but `skip_prophet_race` -- the reservation that stops the empire paying
    // for the race -- needs `skip-the-prophet-race`, which a stock seat does
    // not carry. Unlike that gene, which is a prior and measured -12 pp when
    // it forced non-founding, this reads the board: `religions_founded()` has
    // reached `max_religions()` and none of them is ours, so no prophet this
    // empire recruits can found anything. Live King seat
    // `civvis-20260826T112920Z`: four religions on a Small map capped at
    // four, none of them Rome's, and the seat still finished holding a Holy
    // Site, 228 Prophet points, two Missionaries and 6,329 unspendable Faith.
    Gene { tag: "religion-race-is-closed", field: "religion_race_is_closed", kind: Kind::OptIn, enable: AdvancedAi::enable_religion_race_is_closed, disable: AdvancedAi::disable_religion_race_is_closed },
    // `governor_priority` is keyed on the grand strategy alone and ranks
    // Moksha sixth of seven under Expansion and Recovery and seventh under
    // Diplomacy. For an empire that founded no religion that is backwards:
    // Moksha's `citadel_of_god` is the ONLY thing in the ruleset that stops a
    // foreign faith taking its cities -- `RemoveHeresy` needs an Inquisitor
    // that a religionless empire can never launch, a defensive Missionary
    // needs a faith to spread, `CondemnHeretic` needs a war, and `do_spread`
    // has no target-side gate at all. Live King seat
    // `civvis-20260826T112920Z`: no religion, eleven of twelve cities ended
    // under the score leader's faith paying that rival two points each, and
    // the fifteen governor titles it spent went to Amani, Victor, Pingala and
    // Reyna -- the top four of those same lists. The native screen carries the
    // same regime: own cities under a foreign faith at the end, 1.9 of 5.9.
    Gene { tag: "moksha-defends-the-faithless", field: "moksha_defends_the_faithless", kind: Kind::OptIn, enable: AdvancedAi::enable_moksha_defends_the_faithless, disable: AdvancedAi::disable_moksha_defends_the_faithless },
    // The ladder abandons a King game without three cities by turn 32 and that
    // rule fires on 39 of 87 King games -- the rung's dominant failure, ahead
    // of anything in the score gap. It is not production and it is not sites:
    // the second city lands at a median turn 18 whether the game lives or
    // dies, and across the 33 opening deaths of 2026-08-26 THIRTY still had a
    // settler alive at the abandon, TWENTY of them waiting for a guard in the
    // final five turns. `stacked_escort_pace` bounds that wait at
    // `STACKED_ESCORT_PATIENCE`, but writes the bound as `waited > PATIENCE &&
    // !unstacked_settler_step_is_capturable(..)`, so any visible hostile near
    // the next step suspends the ceiling -- and in the opening that is the
    // weather, not an exception: 1,083 waits against 458 guard arrivals across
    // those 46 games, one run waiting on 34 distinct turns.
    // HostOnly rather than OptIn because the whole path it sits in is gated by
    // `formationless_settler_escort()` -> `live_formationless_settler_shadow`,
    // itself HostOnly: a native board cannot reach this branch, so the screen
    // cannot price it and an OptIn row here would be dead code. The live
    // ladder measures it directly and at high frequency -- the abandon rule it
    // targets fires on 45% of King games, ~46 of which run a day.
    Gene { tag: "escort-patience-runs-out", field: "escort_patience_runs_out", kind: Kind::HostOnly, enable: AdvancedAi::enable_escort_patience_runs_out, disable: AdvancedAi::disable_escort_patience_runs_out },
    // `treasury-at-work` (2026-08-26): the live King seat banked 286 Gold by
    // t36 at +7 a turn and bought nothing in 36 turns, as every live game of
    // the last three days did — 250–330 Gold by t50 and 0–3 purchases in the
    // first 100 turns across 130 runs. `advanced_gold_spending` keeps back
    // 250 + 75 Gold per city under Expansion (325 with one city, 625 with
    // five) and buys only what leaves that much behind, so a 160-Gold
    // Settler or a 100-Gold Builder never clears; every Gold purchase on
    // record came under Recovery's 75 + 25. The working reserve is one
    // emergency defender plus ten turns of any deficit, never below an
    // appointed war's bill. See `advanced/gold_and_cards.rs`.
    Gene { tag: "treasury-at-work", field: "treasury_at_work", kind: Kind::OptIn, enable: AdvancedAi::enable_treasury_at_work, disable: AdvancedAi::disable_treasury_at_work },
    // Version two also buys one under-bought compounding asset ahead of the
    // argmax — the empire's first Builder, then a Monument where a city has
    // none — on `solvency-first-trade-slot`'s measured pattern (+4.65 pp for
    // ONE reserved slot, −2.80 for every slot).
    Gene { tag: "treasury-at-work-2", field: "treasury_at_work_2", kind: Kind::OptIn, enable: AdvancedAi::enable_treasury_at_work_2, disable: AdvancedAi::disable_treasury_at_work_2 },
    // The removed joint search measured its value mostly in ordering: its
    // static seed lost 700 kills on identical damage. The turn's kills are
    // planned once from the engine's arithmetic (no clone), their shooters
    // go first, ranged before the melee finisher, each biased toward its
    // planned target; the exact attack decision is unchanged.
    Gene { tag: "fire-plan", field: "fire_plan", kind: Kind::OptIn, enable: AdvancedAi::enable_fire_plan, disable: AdvancedAi::disable_fire_plan },
    // The doctrine arena's replicated finding: the share of the force up at
    // first contact predicts the swing (r +0.30 on central_position), and
    // the deployed controller arrives over twice the span `basic` does. On
    // an advance no unit ends the turn more than the body's pace plus one
    // closer to the objective than the anchor stood — no stand, every unit
    // spends its movement; the tile beside the line is the line.
    Gene { tag: "close-as-a-body", field: "close_as_a_body", kind: Kind::OptIn, enable: AdvancedAi::enable_close_as_a_body, disable: AdvancedAi::disable_close_as_a_body },
    // The same arena reads the deployed controller's shooters unscreened
    // 25–32 % of their turns against basic's 39–50 %. The arena's own
    // definition of screened — a friend beside the shooter and nearer the
    // enemy — paid to the shooter's tile; the melee side measured −19/seed.
    Gene { tag: "screen-the-shooters", field: "screen_the_shooters", kind: Kind::OptIn, enable: AdvancedAi::enable_screen_the_shooters, disable: AdvancedAi::disable_screen_the_shooters },
    // A settler in the barbarians' hands is plunder to take back — usually
    // our own — not a duplicate to refuse. The second writing: #2075's
    // `civilian-rescue` left in #2509, and the live seat went back to
    // fortifying beside a free settler for twenty turns
    // (civvis-20260826T194422Z). Exempt from the duplicate-settler guard,
    // first among adjacent captures, pursued within four tiles, and the
    // capture crosses to the host as `CAPTURE` — the attack-modifier move
    // #2075's bare `MOVE_TO` never was (65 sent, 0 captures).
    Gene { tag: "barbarian-settler-capture", field: "barbarian_settler_capture", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_barbarian_settler_capture, disable: AdvancedAi::disable_barbarian_settler_capture },
    // `exchange_score` — move ordering, the `military_step` accept, and the
    // force's focus target — reads bare combat strength, so a spearman is
    // priced against a horseman as if the anti-cavalry bonus did not exist
    // and a river crossing costs nothing. `melee_exchange_strengths` and
    // `ranged_strike_strengths` are the engine's own pair, and `do_attack`
    // calls them, so this cannot drift from the fight it will get.
    Gene { tag: "exchange-is-the-engines", field: "exchange_is_the_engines", kind: Kind::OptIn, enable: AdvancedAi::enable_exchange_is_the_engines, disable: AdvancedAi::disable_exchange_is_the_engines },
    // Both threat terms ask what the enemy would do to us on a candidate
    // tile and then price the defender where it is standing NOW, with no
    // tile defence at all — so a unit weighing a hill gets nothing for the
    // hill. `incoming_damage` has always cloned the unit onto the tile
    // first; the movers never learned.
    Gene { tag: "defend-where-you-stand", field: "defend_where_you_stand", kind: Kind::OptIn, enable: AdvancedAi::enable_defend_where_you_stand, disable: AdvancedAi::disable_defend_where_you_stand },
    // `Action::Swap` has been legal, tested and refused correctly in the
    // engine since `do_swap`, and no controller has ever chosen one —
    // `docs/MOVEMENT.md` names this gene as its obvious first use. Today a
    // wounded front-liner is walked away by recovery and the tile it held
    // goes with it; this trades it for the fresh unit behind, in one action.
    Gene { tag: "swap-rotation", field: "swap_rotation", kind: Kind::OptIn, enable: AdvancedAi::enable_swap_rotation, disable: AdvancedAi::disable_swap_rotation },
    // The full modern settlement score pays coastal Housing and a small,
    // half-weighted Harbor adjacency, but never the usable coast itself. The
    // global prefilter and legacy scorer retain a six-point coast credit;
    // this gene restores that scale where a neighbouring `coast` tile can
    // host the Harbor the rules define. Operator request, 2026-08-26. See
    // `advanced/coastal_sites.rs`.
    Gene { tag: "coastal-city-sites", field: "coastal_city_sites", kind: Kind::OptIn, enable: AdvancedAi::enable_coastal_city_sites, disable: AdvancedAi::disable_coastal_city_sites },
    // Version 2 keeps the port-coast baseline and adds the strongest local
    // `coast_resource` adjacency available to a prospective Harbor, so the
    // screen can price the generic coast repair separately from a resource
    // layout refinement.
    Gene { tag: "coastal-city-sites-2", field: "coastal_city_sites_2", kind: Kind::OptIn, enable: AdvancedAi::enable_coastal_city_sites_2, disable: AdvancedAi::disable_coastal_city_sites_2 },
    // `culture-floor` / `gold-income-floor` (2026-08-26): the live King seat
    // `civvis-20260826T184456Z` reached t150 at 46% of the leader with 57
    // culture to the Aztec 183 (Gaul: 180 from ONE city), one wonder to
    // seven, 34 techs to 45, still in Classical Republic, after 44 turns at
    // a treasury of 0 while the engine disbanded the army. Every one of
    // the day's fifteen King runs that reached t100 shows the same shape:
    // culture 16–62 against the best rival's 71–133, ZERO Amphitheatres
    // and ZERO Markets or Lighthouses, one trade route on a capacity of
    // one, six of them bankrupt for 31–73 turns. Three mechanisms in
    // `production_value`: the Great Work veto returns −10,000 for the
    // Amphitheatre (+2 culture, two slots) on every non-Culture seat; the
    // Theatre Square's lane bonus is 0 under Expansion and Recovery, where
    // the seat spends 165 of 184 readings; and nothing prices a gold
    // building or district by the income it is missing — a Market is worth
    // ≈76 under Expansion with the treasury at 0. See
    // `advanced/yield_floors.rs`.
    Gene { tag: "culture-floor", field: "culture_floor", kind: Kind::OptIn, enable: AdvancedAi::enable_culture_floor, disable: AdvancedAi::disable_culture_floor },
    // The gold half: Markets and Lighthouses (a trade route each), any
    // gold-yielding building, and the Commercial Hub or Harbor while fewer
    // than half the cities hold one, priced by the shortfall under two
    // Gold a turn per city.
    Gene { tag: "gold-income-floor", field: "gold_income_floor", kind: Kind::OptIn, enable: AdvancedAi::enable_gold_income_floor, disable: AdvancedAi::disable_gold_income_floor },
    // `early-archers` (operator, 2026-08-26): an Archer for every city, the
    // frontier city first, while the world is Ancient and Classical. The
    // shipped army is sized by bodies (one land unit per city, the Scout
    // counted) and balanced one shooter per melee unit empire-wide, so a
    // one-city empire at its ceiling is refused a third body outright and
    // no city asks for a shooter of its own; Archery itself scores
    // `≈ 8.8` in `tech_value` against a Pottery at 13–17. The gene wants one
    // range-two land shooter per city, lifts the ceiling veto for a missing
    // one the way the contact window does for a Scout, pays a frontier city
    // with no shooter on or beside it the Walls credit on top, and chases
    // Archery until a city can train one. Closes with the first Medieval
    // technology. See `advanced/early_archers.rs`.
    Gene { tag: "early-archers", field: "early_archers", kind: Kind::OptIn, enable: AdvancedAi::enable_early_archers, disable: AdvancedAi::disable_early_archers },
    // `capture-go-or-stand-down` (operator, 2026-08-27, "decisions are slow to
    // be carried out or forgotten"): the commitment ledger read 263 conquest
    // decisions → 10 captures over 8 deployment maps, with nobody of ours
    // within three hexes of the objective on a third of the turns it was
    // held. A declared objective nobody has gone to for six consecutive
    // turns is stood down explicitly — excluded from the ranking for twenty
    // turns, strategy re-assessed now. See `docs/COMMITMENTS.md`.
    Gene { tag: "capture-go-or-stand-down", field: "capture_go_or_stand_down", kind: Kind::OptIn, enable: AdvancedAi::enable_capture_go_or_stand_down, disable: AdvancedAi::disable_capture_go_or_stand_down },
    // Version 2 (2026-08-28): the ledger's split of the 182 failed conquest
    // decisions on eight maps read 137 with bodies at the objective throughout
    // — the army was there and never took the city. A siege that has not
    // pushed the city to a new low for six stalled readings is stood down the
    // same way. One version of the family plays.
    Gene { tag: "capture-go-or-stand-down-2", field: "capture_go_or_stand_down_2", kind: Kind::OptIn, enable: AdvancedAi::enable_capture_go_or_stand_down_2, disable: AdvancedAi::disable_capture_go_or_stand_down_2 },
    // `commitment-patience` (operator, 2026-08-27, the follow-up): the ledger
    // read 2,684 civilian-turns frozen with a hostile within two, 1,713
    // Builder-turns holding a pin never walked to, and 150 of 309 settler
    // retargets after the walk had started — the threat drop reasons turn a
    // passing raider into a whole new walk. A target now survives a threat
    // (the unit holds or retreats as before) and is retired only after three
    // consecutive forgotten turns, parked for the hysteresis window. See
    // `docs/COMMITMENTS.md` §8.
    Gene { tag: "commitment-patience", field: "commitment_patience", kind: Kind::OptIn, enable: AdvancedAi::enable_commitment_patience, disable: AdvancedAi::disable_commitment_patience },
    // `relief-column-marches` / `threatened-city-reserve` (operator,
    // 2026-08-27): the live King seat `civvis-20260827T113726Z` lost
    // Aquileia (pop 15, walled) on t164 to a Chinese stack of seven while
    // holding 2.5× China's military. Its main army — seven units, 1.58 local
    // strength against the besiegers, objective the besieger's own hex —
    // logged "will hold — held back to cover a threat" on every frame from
    // t161 and stood nine hexes west, because `relieving` was
    // `plan.threatened_city.is_some()` for every group in the empire and a
    // Hold's mover target is the group's own centroid; the weak southern
    // group of three collapsed onto ITS centroid, away from the city. The
    // first gene restores #354's reach test (pruned by #1194) and adds the
    // march: a group beyond `THREAT_RELIEF_RADIUS` advances on the siege when
    // locally superior, holds at the city when not, and while a city is
    // threatened focuses only on combatants (the t160 army sat in "Engage"
    // against an Apostle it declined to strike).
    Gene { tag: "relief-column-marches", field: "relief_column_marches", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_relief_column_marches, disable: AdvancedAi::disable_relief_column_marches },
    // The treasury half: the same seat bought a Water Mill in Rome at t160
    // (399 Gold, Aquileia named under threat, a Line Infantry at 360) and a
    // Market in Aquileia itself at t162 with the walls half down, and had 29
    // Gold on t163 when the emergency purchase first became legal. While a
    // city is threatened or bleeding, both Gold buyers keep one emergency
    // defender's price back.
    Gene { tag: "threatened-city-reserve", field: "threatened_city_reserve", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_threatened_city_reserve, disable: AdvancedAi::disable_threatened_city_reserve },
    // A nearly full capital landmass makes unexplored water an expansion
    // frontier: keep one sea scout and favor water that opens a known foreign
    // landfall, while the ordinary naval-recon policy stays unchanged.
    Gene { tag: "island-exploration", field: "island_exploration", kind: Kind::OptIn, enable: AdvancedAi::enable_island_exploration, disable: AdvancedAi::disable_island_exploration },
    // A discovered foreign landfall is time-sensitive. Once the capital's
    // connected land has two or fewer independent city sites remaining, send
    // a Settler to the nearest viable one before spending its last local room.
    Gene { tag: "overseas-settlement", field: "overseas_settlement", kind: Kind::OptIn, enable: AdvancedAi::enable_overseas_settlement, disable: AdvancedAi::disable_overseas_settlement },    // The live seat never researched Astrology (1 of 130 games, at t244):
    // every explicit lane goal is a far-era tech Astrology is not an
    // ancestor of, so the beeline skips it for the whole game and no Holy
    // Site, Shrine, Prophet or Missionary is ever reachable. A secondary
    // Prophet race remains available to eligible lanes, but an explicit
    // Science lane must keep the beeline rather than pay that dead-end cost.
    Gene { tag: "enter-the-prophet-race", field: "enter_the_prophet_race", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_enter_the_prophet_race, disable: AdvancedAi::disable_enter_the_prophet_race },
    // `settler-never-idles` (operator, 2026-08-27): a Settler always has
    // somewhere to go. `advanced_settler_step` held a Settler on more than a
    // dozen branches — a Loyalty forecast, a fog guess, a safe-step guard, a
    // guard that has not come — and when every preferred site was refused it
    // held outright, with each refusal retired for thirty turns so the
    // refusals compounded. On the live seat's genome a native census read
    // 87% of settler-turns idle and 33% idle on an own city tile. The gene
    // asks two wider questions on exhaustion (the ranking with the guesses
    // set aside, then any legal site nearest first), founds where it stands
    // when nothing is reachable, and after `SETTLER_IDLE_PATIENCE` idle turns
    // marches on the exact-reach rule alone. See
    // `advanced/settler_never_idles.rs`.
    Gene { tag: "settler-never-idles", field: "settler_never_idles", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_never_idles, disable: AdvancedAi::disable_settler_never_idles },
    // ⚠ 2026-08-28: the live seat lost twenty-four settlers in ten runs. Six
    // went to barbarian SCOUTS, which every capture model exempted on the
    // claim that Firaxis' scouts neither attack nor capture — run
    // civvis-20260828T122324Z shows one scout standing on our captured
    // settler and taking three more. Host-only: the native barbarian seat's
    // recon really cannot capture, so no screen can price this.
    Gene { tag: "live-barbarian-scouts-capture", field: "live_barbarian_scouts_capture", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_barbarian_scouts_capture, disable: AdvancedAi::disable_live_barbarian_scouts_capture },
    // The other eighteen: eleven walked to a site beside a nest that had
    // already taken a settler (dead sites were per settler and six turns
    // long), six marched alone after "no visible hostile within 8 tiles"
    // beside a known camp, one held still beside a skirmisher two tiles from
    // a full-health archer. Every settler lost on the live seat is now a
    // reconstructed case and a fix (operator rule, 2026-08-28). See
    // `advanced/civilian_safety.rs`.
    Gene { tag: "live-settler-capture-lessons", field: "live_settler_capture_lessons", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_settler_capture_lessons, disable: AdvancedAi::disable_live_settler_capture_lessons },
    // The host refuses ~14% of MOVE_TO orders as `did_not_move` with no
    // destination on the refusal event, so the same refused move was re-issued
    // for up to eleven straight turns (a settler frozen four turns at
    // (19,10) in run civvis-20260830T095742Z was captured on t47). The proof
    // is an issued step the next turn's board shows untaken — impossible in
    // the simulator, where every move applies, so no screen can price this.
    Gene { tag: "live-move-refusal-break", field: "live_move_refusal_break", kind: Kind::HostOnly, enable: AdvancedAi::enable_live_move_refusal_break, disable: AdvancedAi::disable_live_move_refusal_break },
    // A stranded Settler's wider search refuses a site beside an unresolved
    // rival border and forecasts its nearest-legal tier; see
    // `advanced/settler_never_idles.rs`.
    Gene { tag: "exhaustion-loyalty-guard", field: "exhaustion_loyalty_guard", kind: Kind::OptIn, enable: AdvancedAi::enable_exhaustion_loyalty_guard, disable: AdvancedAi::disable_exhaustion_loyalty_guard },
    // A parked Settler holds the Settler pipeline instead of opening it for a
    // replacement; see `BasicAi::settler_backlog_brake`.
    Gene { tag: "settler-backlog-brake", field: "settler_backlog_brake", kind: Kind::OptIn, enable: AdvancedAi::enable_settler_backlog_brake, disable: AdvancedAi::disable_settler_backlog_brake },
    // A housing-bound city builds its Granary ahead of the argmax; see
    // `advanced_production`.
    Gene { tag: "first-granary-reserve", field: "first_granary_reserve", kind: Kind::OptIn, enable: AdvancedAi::enable_first_granary_reserve, disable: AdvancedAi::disable_first_granary_reserve },
    // Research the tech that connects an owned, unimproved luxury before the
    // lane's beeline resumes; see `unconnected_luxury_tech`.
    Gene { tag: "connect-the-luxury", field: "connect_the_luxury", kind: Kind::OptIn, enable: AdvancedAi::enable_connect_the_luxury, disable: AdvancedAi::disable_connect_the_luxury },
    // Hold four fifths of the strongest bordering major's military power in
    // peacetime by buying the contact city's defender; see
    // `border_parity_purchase`.
    Gene { tag: "border-parity", field: "border_parity", kind: Kind::OptIn, enable: AdvancedAi::enable_border_parity, disable: AdvancedAi::disable_border_parity },
    // A few era points short of a Normal Age, any affordable Great Person is
    // worth its moment; see `era_points_short`.
    Gene { tag: "age-closer", field: "age_closer", kind: Kind::OptIn, enable: AdvancedAi::enable_age_closer, disable: AdvancedAi::disable_age_closer },
    // A boosted, prerequisite-met technology within two turns of science is
    // researched before the lane's beeline resumes; see `boosted_bargain_tech`.
    Gene { tag: "boosted-bargain-first", field: "boosted_bargain_first", kind: Kind::OptIn, enable: AdvancedAi::enable_boosted_bargain_first, disable: AdvancedAi::disable_boosted_bargain_first },
    // A wonder twelve turns from done in a strong city opens the live race
    // without the ordinary guards; see `wonder_bargain_city`.
    Gene { tag: "cheapest-wonder-first", field: "cheapest_wonder_first", kind: Kind::OptIn, enable: AdvancedAi::enable_cheapest_wonder_first, disable: AdvancedAi::disable_cheapest_wonder_first },
    // Version two of border-parity: produce the defender when it cannot be
    // bought; see `border_parity_target`.
    Gene { tag: "border-parity-2", field: "border_parity_2", kind: Kind::OptIn, enable: AdvancedAi::enable_border_parity_2, disable: AdvancedAi::disable_border_parity_2 },
    Gene { tag: "first-district-first", field: "first_district_first", kind: Kind::OptIn, enable: AdvancedAi::enable_first_district_first, disable: AdvancedAi::disable_first_district_first },
    Gene { tag: "walls-after-districts", field: "walls_after_districts", kind: Kind::OptIn, enable: AdvancedAi::enable_walls_after_districts, disable: AdvancedAi::disable_walls_after_districts },
    // ⚠ THE HOSTILE LIST IS VISIBLE-ONLY AND THE SETTLERS DIE TO WHAT IS NOT
    // ON IT. Twenty-eight real settler losses over eleven live King runs
    // (2026-08-29): TEN walked into a hostile with none visible within three
    // tiles on the previous turn's state, eight were captured while parked
    // waiting for a guard, six moved inside a visible hostile's reach
    // unescorted, two were zone-of-control pinned, and TWO were taken by Free
    // Cities units (player 62), which `barbarian_reach` ignored outright
    // because it early-returned on `g.barb_pid` and skipped every unit whose
    // owner was not the Barbarian player. The envelope now counts every
    // at-war owner, and keeps pricing a hostile it has actually seen for
    // `HOSTILE_MEMORY_TURNS` after it walks back into the fog — projected
    // with `wdisk` from the last seen tile, padded by the turns since, never
    // from where the unit really is. See `advanced/civilian_safety.rs`.
    Gene { tag: "hostile-memory", field: "hostile_memory", kind: Kind::OptIn, enable: AdvancedAi::enable_hostile_memory, disable: AdvancedAi::disable_hostile_memory },
    // The other half of the same twenty-eight: eight settlers were captured
    // while PARKED waiting for a guard. `stacked_escort_pace` bounds that
    // wait at `STACKED_ESCORT_PATIENCE` = 2 turns and then suspends the bound
    // whenever `unstacked_settler_step_is_capturable` is true — the one
    // predicate in the wait that reads the visible frame alone, which in the
    // opening is the weather rather than the exception. Its fallback then
    // fortifies a settler bare on open ground on `risk > 0.0`, so a zero
    // reading (nothing VISIBLE can reach it) is treated as quiet ground when
    // it is just as often an empty vision frame — the march itself prices the
    // same step against `SETTLER_STEP_RISK_LIMIT` = 30. With the gene on the
    // cap releases on schedule and an unstacked settler already outside its
    // own city marches instead of standing still.
    // ⚠ OptIn rather than HostOnly, unlike `escort-patience-runs-out` above,
    // because the operator arms it as a labelled live arm (`--with`, which
    // only a held-off opt-in or a ledger-held live treatment can seat) rather
    // than shipping it on with the live universe. The path it sits in is
    // still gated by `formationless_settler_escort()` ->
    // `live_formationless_settler_shadow`, so a native screen cannot price it
    // and it carries a fire waiver saying exactly that; the live ladder
    // measures it directly, and the abandon rule it targets fires on 45% of
    // King games.
    Gene { tag: "escort-cap-holds", field: "escort_cap_holds", kind: Kind::OptIn, enable: AdvancedAi::enable_escort_cap_holds, disable: AdvancedAi::disable_escort_cap_holds },
    // The score gap on the live seat is a RESEARCH gap, and at FIXED city
    // count science goes as pop^1.21 — so a city only helps the science lane
    // if it is founded early enough to mature into that super-linear term.
    // Under the gene, until standard turn `SCIENCE_EXPANSION_UNTIL_TURN` =
    // 188 (~t125 Online), the science lane's land grab widens from
    // `LAND_GRAB_CITY_CEILING` = 16 to `SCIENCE_EXPANSION_CITY_CEILING` = 24
    // AND the Science contract's `SCIENCE_CITY_TARGET_CAP` = 8 — shipped
    // after this idea was written — is deferred; when the window closes both
    // revert and the empire grows what it holds. Reaches the board only
    // through `desired_cities` on the Science target — which #2824 made the
    // deployed lane — so it is the standing counter-hypothesis to the cap:
    // cap early and grow, versus widen early then grow.
    // Written for the live seat on 2026-08-19 (three patched games, one
    // stock: too small a read to ship on, so it ships OFF for the screen to
    // price). Carries a fire waiver: its single-gene probe is queued behind
    // the 08-28 operator halt on tournaments while this machine runs live
    // verification. Appended at the END so a running screen keeps its
    // positional genome.
    Gene { tag: "science-expansion-phase", field: "science_expansion_phase", kind: Kind::OptIn, enable: AdvancedAi::enable_science_expansion_phase, disable: AdvancedAi::disable_science_expansion_phase },
    Gene { tag: "first-luxury-first", field: "first_luxury_first", kind: Kind::OptIn, enable: AdvancedAi::enable_first_luxury_first, disable: AdvancedAi::disable_first_luxury_first },
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "science-opening-band", field: "science_opening_band", kind: Kind::OptIn, enable: AdvancedAi::enable_science_opening_band, disable: AdvancedAi::disable_science_opening_band },
    // Version 2 keeps the original science-drive phenotype and tournament
    // evidence intact, while pricing the continuation planner as its own
    // family member: meaningful adaptive leads, legal pads, research funnel,
    // and a live launch-chain estimate. Appended at the END so a running
    // screen keeps its positional genome.
    Gene { tag: "science-victory-drive-2", field: "science_victory_drive_2", kind: Kind::OptIn, enable: AdvancedAi::enable_science_victory_drive_2, disable: AdvancedAi::disable_science_victory_drive_2 },
    // A capital at population two can legally begin a Settler, yet the
    // ordinary production ranking may spend its newly empty queue on an
    // Archer or filler instead. Once its whole queue drains, reserve that
    // one next build for a Settler; repairs, emergencies, war, the city
    // target, the normal deadline, and the practical-site gate still decide
    // whether the reservation is allowed. See `BasicAi::pick_item`.
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "capital-settler-after-completion", field: "capital_settler_after_completion", kind: Kind::OptIn, enable: AdvancedAi::enable_capital_settler_after_completion, disable: AdvancedAi::disable_capital_settler_after_completion },
    // Opening project restraint is intentionally an opt-in while the
    // single-gene screen determines whether its production reallocation
    // improves the live universe. Appended at the END to preserve any
    // running screen's positional genome.
    Gene { tag: "early-project-restraint", field: "early_project_restraint", kind: Kind::OptIn, enable: AdvancedAi::enable_early_project_restraint, disable: AdvancedAi::disable_early_project_restraint },

    // ★★★ APPEND POINTS, SO THAT TWO GENE PRS DO NOT APPEND TO ONE LINE.
    //
    // The registry order is positional, so these all stay after the existing
    // rows: a new tag goes below the marker for its first-letter range. That
    // preserves every existing bit while giving independent new tags distinct
    // insertion lines. `tools/test_treatment_append_points.py` proves the
    // claim by constructing and merging two synthetic Gene-row pull requests.
    // ---- append: a-b ------------------------------------------------
    // ---- append: c-d ------------------------------------------------
    // ---- append: e-f ------------------------------------------------
    // ---- append: g-k ------------------------------------------------
    // ---- append: l-o ------------------------------------------------
    // ---- append: p-r ------------------------------------------------
    // ---- append: s-s ------------------------------------------------
    Gene { tag: "spaceport-surplus-veto", field: "spaceport_surplus_veto", kind: Kind::OptIn, enable: AdvancedAi::enable_spaceport_surplus_veto, disable: AdvancedAi::disable_spaceport_surplus_veto },
    // ---- append: t-z ------------------------------------------------
];

// ═══ GENERATED BY tools/genes.py — THE VERDICTS. Do not edit below: `python3 tools/genes.py write` ═══
//
// Source: docs/gene_ledger.json (the same tool writes both); `genes.py check` holds them
// together; this reporting-only publication retains `DEPLOYMENT_GENOME`.
// Each selected tag averages at least +5 wins/10k total seats across its
// available latest-three `BATCH_COLUMNS`; one higher-average version ships per family.

/// The policy that supplies every `default_on` below.
pub(super) const DEPLOYMENT_POLICY: &str = "operator-retained-selection";

/// The screenable genes averaging at least +5 wins per 10,000 total seats
/// over their available latest-three batch readings; every other screenable
/// gene is off. One higher-average version ships per family.
#[rustfmt::skip]
pub(super) const DEPLOYMENT_GENOME: &[&str] = &[
    "air-surge",
    "amenity-district-path",
    "amenity-project-preemption",
    "army-target-weighs-enemy",
    "barbarian-settler-capture",
    "blind-objective-units",
    "bounded-recovery",
    "buy-what-cards-cannot-boost",
    "campus-adjacency-threshold",
    "canal-city",
    "capture-go-or-stand-down-2",
    "chokepoint-siting",
    "civilian-out-of-reach",
    "commitment-patience",
    "competition-victory-points",
    "connect-the-luxury",
    "contested-land-first",
    "culture-lane-forecast",
    "deals-at-the-ceiling",
    "defensible-sites",
    "district-planning",
    "domination-city-count",
    "early-archers",
    "elective-war-yields-to-a-lane",
    "encampment-seals-the-pass",
    "enemy-of-my-enemy",
    "engine-faith-price",
    "enter-the-prophet-race",
    "escort-cap-holds",
    "expansion-schedule",
    "first-luxury-first",
    "gold-for-the-young-city",
    "government-ladder-2",
    "great-person-housing",
    "guru-heals-the-corps-2",
    "holy-site-where-the-threat-is-2",
    "idle-faith-patronage",
    "island-exploration",
    "lane-culture-spending",
    "loyalty-rate-alarm",
    "maintenance-aware-deck",
    "missionary-evades-raiders",
    "missionary-last-charge-explores",
    "naval-recon",
    "naval-threat-triage",
    "one-shot-recovery",
    "opportunistic-war",
    "pantheon-board",
    "price-the-suzerainty",
    "quest-trade-route",
    "recon-replacement",
    "recorded-tactical-step",
    "religion-race-is-closed",
    "religious-defence-scales",
    "religious-veto-defence",
    "science-building-first",
    "science-chain-alarm",
    "science-opening-band",
    "science-victory-drive",
    "settle-sooner",
    "settler-second-look",
    "slot-kind-tiebreak",
    "solvency-first-trade-slot",
    "stranded-settler-discount",
    "surprise-war-mobilization",
    "treasury-at-work-2",
    "unit-objective-memory",
    "war-reinforcement",
    "wide-map-capacity",
    "wonder-adjacent-sites",
];

/// No manual on overrides: every deployed gene meets the published
/// available-latest-three-batches average-at-least-+5 criterion.
#[rustfmt::skip]
pub(super) const OPERATOR_DEFAULT_ON: &[&str] = &[
];

/// No manual off overrides: every gene outside `DEPLOYMENT_GENOME` is off
/// because it did not meet the explicit selection criterion.
#[rustfmt::skip]
pub(super) const OPERATOR_DEFAULT_OFF: &[&str] = &[
];

/// Every screenable gene a reporting batch priced, with its three batch
/// columns newest first — wins ± per 10,000 total seats in the ranking's
/// *Last*, *Prior* and *Third Batch* columns. They remain evidence while
/// this reporting-only publication retains the selected defaults.
#[rustfmt::skip]
pub(super) const BATCH_COLUMNS: &[(&str, [Option<i32>; 3])] = &[
    ("age-closer", [Some(-11), Some(0), Some(21)]),
    ("air-surge", [Some(18), Some(30), Some(16)]),
    ("amenity-district-path", [Some(35), Some(9), Some(4)]),
    ("amenity-project-preemption", [Some(11), Some(24), Some(7)]),
    ("amenity-project-preemption-2", [Some(-17), Some(-14), Some(-2)]),
    ("apostle-promotion-by-role", [Some(-1), Some(7), Some(6)]),
    ("army-target-weighs-enemy", [Some(19), Some(1), Some(20)]),
    ("barbarian-bargain", [Some(14), Some(14), Some(-16)]),
    ("barbarian-ranged-answer", [Some(-14), Some(20), Some(5)]),
    ("barbarian-scouts-are-scouts", [Some(-23), Some(11), Some(-2)]),
    ("barbarian-settler-capture", [Some(35), Some(34), Some(34)]),
    ("blind-objective-strength", [Some(-19), Some(9), Some(16)]),
    ("blind-objective-units", [Some(20), Some(0), Some(4)]),
    ("boost-first-research", [Some(4), Some(-12), Some(-3)]),
    ("boost-unlock-research", [Some(-23), Some(1), Some(-38)]),
    ("boost-wait-research", [Some(-8), Some(-23), Some(-19)]),
    ("boosted-bargain-first", [Some(-39), Some(9), Some(-21)]),
    ("border-parity", [Some(9), Some(-5), Some(-10)]),
    ("border-parity-2", [Some(-10), Some(-13), Some(-12)]),
    ("bounded-recovery", [Some(9), Some(10), Some(4)]),
    ("build-what-cards-boost", [Some(-9), Some(-9), Some(31)]),
    ("builder-barbarian-safety", [Some(-5), Some(1), Some(9)]),
    ("builder-supply-floor", [Some(-17), Some(-26), Some(-9)]),
    ("builder-tries-the-next-tile", [Some(-23), Some(-3), Some(-7)]),
    ("buildings-before-projects", [Some(-26), Some(19), Some(0)]),
    ("buy-what-cards-cannot-boost", [Some(24), Some(-7), Some(21)]),
    ("camp-party", [Some(-3), Some(-14), Some(-4)]),
    ("camp-tile-buyout", [Some(-22), Some(19), Some(-5)]),
    ("campaign-pillage", [Some(-6), Some(-14), Some(-1)]),
    ("campus-adjacency-threshold", [Some(22), Some(-25), Some(19)]),
    ("campus-adjacency-threshold-2", [Some(-20), Some(13), Some(-5)]),
    ("canal-city", [Some(19), Some(5), Some(9)]),
    ("capital-settler-after-completion", [Some(23), Some(-16), None]),
    ("capture-go-or-stand-down", [Some(-25), Some(-1), Some(-8)]),
    ("capture-go-or-stand-down-2", [Some(25), Some(8), Some(1)]),
    ("chain-payback-window", [Some(-6), Some(-5), Some(-1)]),
    ("chain-payback-window-2", [Some(-26), Some(-19), Some(6)]),
    ("cheapest-wonder-first", [Some(-10), Some(-11), Some(-4)]),
    ("chokepoint-claim", [Some(-19), Some(2), Some(0)]),
    ("chokepoint-garrison", [Some(-1), Some(-6), Some(-7)]),
    ("chokepoint-siting", [Some(22), Some(-5), Some(24)]),
    ("chop-into-the-queue", [Some(-17), Some(-22), Some(-24)]),
    ("city-campaign", [Some(-19), Some(-16), Some(-7)]),
    ("city-target-meets-the-map", [Some(-14), Some(6), Some(2)]),
    ("civilian-out-of-reach", [Some(14), Some(2), Some(4)]),
    ("close-as-a-body", [Some(-10), Some(-5), Some(12)]),
    ("coalition-before-war", [Some(-20), Some(-5), Some(13)]),
    ("coastal-city-sites", [Some(-2), Some(9), Some(-6)]),
    ("coastal-city-sites-2", [Some(-8), Some(-1), Some(-3)]),
    ("come-ashore", [Some(-42), Some(10), Some(1)]),
    ("commitment-patience", [Some(26), Some(1), Some(2)]),
    ("competition-victory-points", [Some(12), Some(36), Some(7)]),
    ("congress-counter-leader", [Some(-23), Some(-11), Some(27)]),
    ("connect-the-luxury", [Some(66), Some(29), Some(27)]),
    ("contested-land-first", [Some(41), Some(4), Some(-2)]),
    ("conversion-majority-alarm", [Some(6), Some(-9), Some(-2)]),
    ("coordinated-finish", [Some(-2), Some(-7), Some(0)]),
    ("culture-building-debt", [Some(24), Some(-22), Some(4)]),
    ("culture-floor", [Some(8), Some(4), Some(-15)]),
    ("culture-lane-forecast", [Some(29), Some(24), Some(-3)]),
    ("deals-at-the-ceiling", [Some(7), Some(23), Some(8)]),
    ("deals-for-our-gain", [Some(-2), Some(-12), Some(8)]),
    ("defend-where-you-stand", [Some(-44), Some(-4), Some(7)]),
    ("defensible-sites", [Some(3), Some(24), Some(26)]),
    ("diplomatic-lane-forecast", [Some(-30), Some(-14), Some(-13)]),
    ("district-coverage", [Some(-18), Some(-7), Some(13)]),
    ("district-coverage-2", [Some(17), Some(-2), Some(-6)]),
    ("district-planning", [Some(1), Some(0), Some(14)]),
    ("domination-city-count", [Some(26), Some(20), Some(17)]),
    ("early-archers", [Some(-15), Some(18), Some(29)]),
    ("early-contact-window", [Some(8), Some(17), Some(-18)]),
    ("early-project-restraint", [Some(-9), Some(-10), None]),
    ("elective-war-in-reach", [Some(-5), Some(6), Some(3)]),
    ("elective-war-yields-to-a-lane", [Some(24), Some(32), Some(27)]),
    ("encampment-seals-the-pass", [Some(18), Some(13), Some(10)]),
    ("enemy-of-my-enemy", [Some(6), Some(15), Some(-5)]),
    ("engine-faith-price", [Some(5), Some(17), Some(41)]),
    ("enhancer-for-the-corps", [Some(14), Some(-15), Some(-5)]),
    ("enter-the-prophet-race", [Some(-30), Some(35), Some(16)]),
    ("escort-cap-holds", [Some(46), Some(-9), Some(-13)]),
    ("escort-unstick", [Some(-28), Some(19), Some(6)]),
    ("eureka-chasing-builder", [Some(-20), Some(-16), Some(-32)]),
    ("eureka-chasing-production", [Some(-19), Some(19), Some(-8)]),
    ("exchange-is-the-engines", [Some(2), Some(13), Some(-1)]),
    ("exhaustion-loyalty-guard", [Some(-7), Some(2), Some(-11)]),
    ("expansion-pays-back", [Some(21), Some(-10), Some(-5)]),
    ("expansion-schedule", [Some(17), Some(8), Some(3)]),
    ("fire-plan", [Some(-6), Some(-10), Some(-10)]),
    ("first-builder-reserve", [Some(9), Some(-18), Some(-6)]),
    ("first-district-first", [Some(-5), Some(-8), Some(7)]),
    ("first-granary-reserve", [Some(-8), Some(-5), Some(13)]),
    ("first-luxury-first", [Some(15), Some(1), Some(5)]),
    ("first-research-building-reserve", [Some(-5), Some(7), Some(-3)]),
    ("flip-nearby-city-states", [Some(3), Some(-6), Some(-15)]),
    ("founder-temple", [Some(17), Some(18), Some(-24)]),
    ("frontier-massing-alarm", [Some(0), Some(-12), Some(0)]),
    ("garrison-under-fire", [Some(10), Some(-8), Some(-8)]),
    ("gold-for-the-young-city", [Some(30), Some(8), Some(-6)]),
    ("gold-income-floor", [Some(18), Some(-15), Some(-1)]),
    ("government-capacity-fallback", [Some(-23), Some(-13), Some(3)]),
    ("government-ladder", [Some(-9), Some(-25), Some(-16)]),
    ("government-ladder-2", [Some(-9), Some(36), Some(27)]),
    ("great-person-housing", [Some(26), Some(22), Some(20)]),
    ("growth-to-settle", [Some(-8), Some(0), Some(20)]),
    ("guru-heals-the-corps-2", [Some(5), Some(12), Some(-2)]),
    ("holy-lane-parity", [Some(4), Some(3), Some(0)]),
    ("holy-site-where-the-threat-is", [Some(-6), Some(12), Some(-7)]),
    ("holy-site-where-the-threat-is-2", [Some(37), Some(-10), Some(8)]),
    ("hostile-memory", [Some(-23), Some(-5), Some(-7)]),
    ("housing-research", [Some(8), Some(2), Some(-7)]),
    ("idle-faith-patronage", [Some(26), Some(9), Some(1)]),
    ("improvement-housing-value", [Some(18), Some(-12), Some(-5)]),
    ("island-exploration", [Some(46), Some(0), Some(-8)]),
    ("lane-culture-spending", [Some(-11), Some(22), Some(9)]),
    ("lane-great-people", [Some(-24), Some(-7), Some(-11)]),
    ("lane-policy-deck", [Some(-13), Some(-1), Some(-20)]),
    ("lane-release-when-hopeless", [Some(3), Some(-28), Some(6)]),
    ("lane-space-race", [Some(-13), Some(-12), Some(11)]),
    ("lane-votes-its-favor", [Some(6), Some(1), Some(5)]),
    ("loyalty-rate-alarm", [Some(8), Some(54), Some(42)]),
    ("maintenance-aware-deck", [Some(25), Some(35), Some(12)]),
    ("missionary-evades-raiders", [Some(10), Some(21), Some(32)]),
    ("missionary-last-charge-explores", [Some(42), Some(16), Some(-10)]),
    ("moksha-defends-the-faithless", [Some(18), Some(-24), Some(-11)]),
    ("native-emergency-purchase", [Some(-33), Some(2), Some(9)]),
    ("naval-recon", [Some(29), Some(9), Some(-1)]),
    ("naval-recon-2", [Some(-17), Some(-10), Some(13)]),
    ("naval-threat-triage", [Some(14), Some(1), Some(5)]),
    ("never-an-empty-queue", [Some(-19), Some(-18), Some(-8)]),
    ("never-an-empty-queue-2", [Some(-21), Some(-11), Some(0)]),
    ("no-free-passage", [Some(-2), Some(-2), Some(14)]),
    ("one-launch-pad", [Some(-2), Some(-7), Some(-4)]),
    ("one-shot-recovery", [Some(17), Some(-14), Some(14)]),
    ("one-war-at-a-time", [Some(-1), Some(7), Some(-1)]),
    ("opening-warrior-recon", [Some(4), Some(-27), Some(-17)]),
    ("opportunistic-war", [Some(18), Some(0), Some(8)]),
    ("order-retry", [Some(-11), Some(-14), Some(-17)]),
    ("overseas-settlement", [Some(-20), Some(-7), Some(12)]),
    ("pantheon-board", [Some(36), Some(6), Some(31)]),
    ("pass-picket", [Some(14), Some(0), Some(0)]),
    ("peace-when-the-war-does-not-pay", [Some(-27), Some(25), Some(-7)]),
    ("peacetime-deterrence", [Some(-37), Some(10), Some(0)]),
    ("power-the-laboratory", [Some(15), Some(-8), Some(-8)]),
    ("power-the-laboratory-2", [Some(-23), Some(-7), Some(-3)]),
    ("price-the-suzerainty", [Some(51), Some(1), Some(20)]),
    ("promote-when-wounded", [Some(1), Some(0), Some(7)]),
    ("quest-boost", [Some(25), Some(-13), Some(-8)]),
    ("quest-camp-errand", [Some(-16), Some(11), Some(-18)]),
    ("quest-production", [Some(-40), Some(-25), Some(-2)]),
    ("quest-trade-route", [Some(10), Some(19), Some(-4)]),
    ("raid-pillage-prizes", [Some(7), Some(-7), Some(0)]),
    ("rapid-city-expansion", [Some(-118), Some(-104), Some(-83)]),
    ("recon-replacement", [Some(46), Some(-4), Some(2)]),
    ("recorded-tactical-step", [Some(45), Some(25), Some(-5)]),
    ("recovery-reads-the-war", [Some(36), Some(-26), Some(3)]),
    ("relief-column-marches", [Some(-12), Some(12), Some(10)]),
    ("relief-targets-the-siege", [Some(-2), Some(13), Some(-2)]),
    ("religion-race-is-closed", [Some(37), Some(21), Some(-15)]),
    ("religion-sues-peace", [Some(17), Some(-8), Some(5)]),
    ("religious-defence-scales", [Some(23), Some(-5), Some(18)]),
    ("religious-units-heal-first", [Some(18), Some(-26), Some(-4)]),
    ("religious-veto-defence", [Some(-4), Some(12), Some(42)]),
    ("research-tier-premium", [Some(-33), Some(3), Some(-6)]),
    ("rival-suzerainty-alarm", [Some(11), Some(-4), Some(-10)]),
    ("science-building-first", [Some(25), Some(28), Some(-2)]),
    ("science-chain-alarm", [Some(10), Some(12), Some(1)]),
    ("science-expansion-phase", [Some(-22), Some(-1), Some(-3)]),
    ("science-multiplier-payoff", [Some(5), Some(1), Some(-10)]),
    ("science-opening-band", [Some(40), Some(12), Some(-5)]),
    ("science-victory-drive", [Some(20), Some(16), Some(18)]),
    ("science-victory-drive-2", [Some(-13), Some(12), None]),
    ("score-horizon", [Some(-32), Some(-27), Some(2)]),
    ("screen-the-shooters", [Some(0), Some(18), Some(-13)]),
    ("settle-sooner", [Some(29), Some(18), Some(-5)]),
    ("settlement-gap-target", [Some(-17), Some(12), Some(9)]),
    ("settler-backlog-brake", [Some(-12), Some(0), Some(5)]),
    ("settler-factory-coordination", [Some(7), Some(-21), Some(-26)]),
    ("settler-guard-holds", [Some(-16), Some(-31), Some(4)]),
    ("settler-guard-holds-2", [Some(13), Some(6), Some(-8)]),
    ("settler-never-idles", [Some(-27), Some(27), Some(3)]),
    ("settler-screen", [Some(-29), Some(7), Some(0)]),
    ("settler-second-look", [Some(28), Some(4), Some(-4)]),
    ("settler-target-hysteresis", [Some(-5), Some(-2), Some(-2)]),
    ("settler-target-hysteresis-2", [Some(-1), Some(0), Some(2)]),
    ("settler-threat-detour", [Some(10), Some(1), Some(-11)]),
    ("siege-commitment", [Some(-21), Some(-26), Some(-6)]),
    ("siege-is-progress-2", [Some(-19), Some(-16), Some(-3)]),
    ("skip-the-prophet-race", [Some(2), Some(-115), Some(-118)]),
    ("slot-kind-tiebreak", [Some(16), Some(-7), Some(7)]),
    ("solvency-first-trade-slot", [Some(108), Some(111), Some(136)]),
    ("stranded-settler-discount", [Some(-18), Some(31), Some(5)]),
    ("strategic-wonders", [Some(0), Some(6), Some(-11)]),
    ("strike-opening", [Some(-15), Some(-5), Some(-2)]),
    ("surprise-war-mobilization", [Some(18), Some(-4), Some(7)]),
    ("swap-rotation", [Some(-15), Some(-2), Some(-3)]),
    ("threatened-city-reserve", [Some(-3), Some(10), Some(3)]),
    ("treasury-at-work", [Some(10), Some(-1), Some(8)]),
    ("treasury-at-work-2", [Some(29), Some(4), Some(17)]),
    ("unchosen-war-keeps-the-lane", [Some(1), Some(8), Some(-9)]),
    ("unit-cost-efficiency", [Some(-9), Some(-17), Some(9)]),
    ("unit-objective-memory", [Some(-5), Some(14), Some(16)]),
    ("upgrade-the-garrison", [Some(-33), Some(7), Some(-5)]),
    ("walls-after-districts", [Some(-14), Some(-9), Some(-1)]),
    ("war-economy", [Some(-1), Some(-8), Some(21)]),
    ("war-needs-a-treasury", [Some(-27), Some(15), Some(19)]),
    ("war-reinforcement", [Some(15), Some(8), Some(26)]),
    ("whole-turn-backtrack-guard", [Some(-3), Some(0), Some(-7)]),
    ("wide-map-capacity", [Some(21), Some(30), Some(52)]),
    ("wonder-adjacent-sites", [Some(32), Some(15), Some(29)]),
    ("wonder-adjacent-sites-2", [Some(-6), Some(-12), Some(-17)]),
    ("wonder-ring-recon", [Some(5), Some(-2), Some(-8)]),
    ("wonder-score-tally", [Some(1), Some(3), Some(-5)]),
];

#[rustfmt::skip]
pub(super) const VERDICTS: &[GeneVerdict] = &[
    GeneVerdict { tag: "air-surge", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(44), wins_prior_10k: Some(108), win_diff_pp: Some(1.868162), posterior_pp: Some(100.730365), posterior_se_pp: Some(12.512909), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.748, win_z: 4.063, share_delta_pp: 0.523, share_z: 5.915, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "amenity-district-path", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(9), wins_prior_10k: Some(17), win_diff_pp: Some(0.220736), posterior_pp: Some(11.500915), posterior_se_pp: Some(8.515459), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.343, win_z: 0.778, share_delta_pp: 0.086, share_z: 0.944, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "amenity-project-preemption", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(26), wins_prior_10k: Some(-34), win_diff_pp: Some(-0.037946), posterior_pp: Some(-0.579866), posterior_se_pp: Some(12.814024), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.515, win_z: 1.347, share_delta_pp: 0.062, share_z: 0.787, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "apostle-promotion-by-role", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(12), wins_prior_10k: Some(16), win_diff_pp: Some(0.133468), posterior_pp: Some(6.102099), posterior_se_pp: Some(10.821961), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.488, win_z: 1.11, share_delta_pp: 0.009, share_z: 0.1, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "army-target-weighs-enemy", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(-1), wins_prior_10k: Some(18), win_diff_pp: Some(0.098436), posterior_pp: Some(4.708189), posterior_se_pp: Some(10.494744), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.026, win_z: -0.058, share_delta_pp: -0.071, share_z: -0.77, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "barbarian-bargain", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(22), wins_prior_10k: Some(10), win_diff_pp: Some(0.322761), posterior_pp: Some(16.41034), posterior_se_pp: Some(10.942339), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.879, win_z: 2.013, share_delta_pp: 0.131, share_z: 1.474, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "barbarian-ranged-answer", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-2), wins_prior_10k: Some(17), win_diff_pp: Some(0.194468), posterior_pp: Some(10.931114), posterior_se_pp: Some(10.506421), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.095, win_z: -0.216, share_delta_pp: -0.008, share_z: -0.093, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "barbarian-scouts-are-scouts", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(1), wins_prior_10k: Some(12), win_diff_pp: Some(0.542033), posterior_pp: Some(29.535753), posterior_se_pp: Some(11.143814), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.037, win_z: 0.085, share_delta_pp: 0.131, share_z: 1.443, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "blind-objective-strength", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-23), wins_prior_10k: Some(0), win_diff_pp: Some(0.185599), posterior_pp: Some(10.786338), posterior_se_pp: Some(10.431471), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.458, win_z: -1.197, share_delta_pp: 0.042, share_z: 0.546, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "blind-objective-units", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(2), wins_prior_10k: Some(0), win_diff_pp: Some(0.009017), posterior_pp: Some(0.293538), posterior_se_pp: Some(8.178949), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.031, win_z: 0.083, share_delta_pp: -0.095, share_z: -1.209, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "bounded-recovery", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(23), wins_prior_10k: Some(29), win_diff_pp: Some(0.602307), posterior_pp: Some(31.029126), posterior_se_pp: Some(8.476105), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.904, win_z: 2.094, share_delta_pp: 0.296, share_z: 3.353, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "builder-barbarian-safety", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(13), wins_prior_10k: Some(-10), win_diff_pp: Some(0.077422), posterior_pp: Some(3.322829), posterior_se_pp: Some(10.068339), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.255, win_z: 0.67, share_delta_pp: 0.023, share_z: 0.295, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "buildings-before-projects", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(13), wins_prior_10k: Some(61), win_diff_pp: Some(0.567693), posterior_pp: Some(28.304902), posterior_se_pp: Some(11.64753), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.534, win_z: 1.214, share_delta_pp: 0.059, share_z: 0.654, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "camp-party", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(28), wins_prior_10k: Some(-15), win_diff_pp: Some(0.398999), posterior_pp: Some(22.420326), posterior_se_pp: Some(12.741517), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.566, win_z: 1.481, share_delta_pp: 0.059, share_z: 0.746, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "campus-adjacency-threshold", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(21), wins_prior_10k: Some(-18), win_diff_pp: Some(-0.007025), posterior_pp: Some(0.159571), posterior_se_pp: Some(19.440709), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.425, win_z: 1.115, share_delta_pp: 0.016, share_z: 0.208, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "come-ashore", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-3), wins_prior_10k: Some(5), win_diff_pp: Some(0.124668), posterior_pp: Some(7.026062), posterior_se_pp: Some(8.597676), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.102, win_z: -0.232, share_delta_pp: -0.035, share_z: -0.381, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "competition-victory-points", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(35), wins_prior_10k: Some(0), win_diff_pp: Some(0.309119), posterior_pp: Some(15.760591), posterior_se_pp: Some(17.698531), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.702, win_z: 1.841, share_delta_pp: 0.037, share_z: 0.463, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "coordinated-finish", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(0), wins_prior_10k: None, win_diff_pp: Some(-0.008735), posterior_pp: Some(-0.436761), posterior_se_pp: Some(19.047677), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.009, win_z: -0.023, share_delta_pp: 0.074, share_z: 0.943, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "culture-building-debt", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(15), wins_prior_10k: Some(24), win_diff_pp: Some(0.486116), posterior_pp: Some(25.813208), posterior_se_pp: Some(12.82409), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.58, win_z: 1.328, share_delta_pp: 0.087, share_z: 0.957, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "district-coverage", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(14), wins_prior_10k: Some(-22), win_diff_pp: Some(0.008266), posterior_pp: Some(0.19452), posterior_se_pp: Some(10.3006), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.28, win_z: 0.736, share_delta_pp: 0.166, share_z: 2.158, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "district-planning", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(12), wins_prior_10k: None, win_diff_pp: Some(0.244584), posterior_pp: Some(12.229214), posterior_se_pp: Some(18.957561), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.245, win_z: 0.645, share_delta_pp: 0.145, share_z: 1.843, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "early-contact-window", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(17), wins_prior_10k: Some(2), win_diff_pp: Some(0.169393), posterior_pp: Some(7.701146), posterior_se_pp: Some(12.097581), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.337, win_z: 0.877, share_delta_pp: 0.091, share_z: 1.164, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "engine-faith-price", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(63), wins_prior_10k: None, win_diff_pp: Some(1.259609), posterior_pp: Some(62.980433), posterior_se_pp: Some(19.232589), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.26, win_z: 3.275, share_delta_pp: 0.089, share_z: 1.125, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "enhancer-for-the-corps", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-5), wins_prior_10k: Some(8), win_diff_pp: Some(0.043714), posterior_pp: Some(2.646744), posterior_se_pp: Some(12.161783), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.091, win_z: -0.239, share_delta_pp: 0.096, share_z: 1.239, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "escort-unstick", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(38), wins_prior_10k: Some(36), win_diff_pp: Some(0.665374), posterior_pp: Some(32.078711), posterior_se_pp: Some(12.520561), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.767, win_z: 2.017, share_delta_pp: 0.197, share_z: 2.463, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "founder-temple", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(11), wins_prior_10k: Some(7), win_diff_pp: Some(0.317732), posterior_pp: Some(19.361508), posterior_se_pp: Some(9.58433), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.455, win_z: 1.028, share_delta_pp: -0.108, share_z: -1.196, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "garrison-under-fire", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(31), wins_prior_10k: Some(-27), win_diff_pp: Some(0.234065), posterior_pp: Some(15.397499), posterior_se_pp: Some(16.420664), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.617, win_z: 1.615, share_delta_pp: 0.118, share_z: 1.499, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "great-person-housing", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(38), wins_prior_10k: Some(94), win_diff_pp: Some(1.583612), posterior_pp: Some(84.173107), posterior_se_pp: Some(10.509873), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.499, win_z: 3.464, share_delta_pp: 0.375, share_z: 4.121, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "holy-lane-parity", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(5), wins_prior_10k: Some(19), win_diff_pp: Some(0.623541), posterior_pp: Some(32.349037), posterior_se_pp: Some(19.66726), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.185, win_z: 0.421, share_delta_pp: -0.184, share_z: -1.997, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "holy-site-where-the-threat-is", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(19), wins_prior_10k: Some(-19), win_diff_pp: Some(-0.031224), posterior_pp: Some(-1.100691), posterior_se_pp: Some(19.032158), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.391, win_z: 1.034, share_delta_pp: -0.088, share_z: -1.141, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "housing-research", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-5), wins_prior_10k: Some(-17), win_diff_pp: Some(-0.009393), posterior_pp: Some(0.465459), posterior_se_pp: Some(11.264612), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.107, win_z: -0.282, share_delta_pp: -0.004, share_z: -0.048, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "idle-faith-patronage", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(-1), wins_prior_10k: Some(39), win_diff_pp: Some(0.505304), posterior_pp: Some(25.778142), posterior_se_pp: Some(7.31643), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.056, win_z: -0.127, share_delta_pp: 0.148, share_z: 1.622, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "lane-culture-spending", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(9), wins_prior_10k: Some(8), win_diff_pp: Some(0.172513), posterior_pp: Some(8.564803), posterior_se_pp: Some(12.159394), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.187, win_z: 0.486, share_delta_pp: -0.01, share_z: -0.13, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "lane-great-people", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(33), wins_prior_10k: Some(-3), win_diff_pp: Some(0.262286), posterior_pp: Some(13.24308), posterior_se_pp: Some(17.90329), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.66, win_z: 1.717, share_delta_pp: 0.1, share_z: 1.261, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "lane-policy-deck", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(29), wins_prior_10k: Some(0), win_diff_pp: Some(0.264627), posterior_pp: Some(12.799771), posterior_se_pp: Some(14.670107), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.592, win_z: 1.554, share_delta_pp: 0.083, share_z: 1.067, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "lane-space-race", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-12), wins_prior_10k: Some(11), win_diff_pp: Some(0.010148), posterior_pp: Some(1.30902), posterior_se_pp: Some(12.141457), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.239, win_z: -0.632, share_delta_pp: -0.104, share_z: -1.317, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "loyalty-rate-alarm", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(5), wins_prior_10k: Some(38), win_diff_pp: Some(0.752269), posterior_pp: Some(39.929305), posterior_se_pp: Some(9.323774), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.207, win_z: 0.473, share_delta_pp: 0.188, share_z: 2.083, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "maintenance-aware-deck", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(61), wins_prior_10k: None, win_diff_pp: Some(1.229909), posterior_pp: Some(61.49546), posterior_se_pp: Some(19.045342), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.23, win_z: 3.229, share_delta_pp: 0.341, share_z: 4.381, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "naval-recon", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(17), wins_prior_10k: Some(-20), win_diff_pp: Some(-0.045836), posterior_pp: Some(-3.061334), posterior_se_pp: Some(8.280916), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.353, win_z: 0.917, share_delta_pp: 0.066, share_z: 0.83, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "one-launch-pad", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(19), wins_prior_10k: Some(-11), win_diff_pp: Some(0.235943), posterior_pp: Some(11.406528), posterior_se_pp: Some(8.259198), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.374, win_z: 0.97, share_delta_pp: 0.031, share_z: 0.395, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "one-shot-recovery", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(8), wins_prior_10k: Some(-8), win_diff_pp: Some(-0.022638), posterior_pp: Some(-1.894908), posterior_se_pp: Some(12.019236), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.159, win_z: 0.418, share_delta_pp: 0.053, share_z: 0.671, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "opportunistic-war", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(39), wins_prior_10k: Some(49), win_diff_pp: Some(0.904992), posterior_pp: Some(48.055552), posterior_se_pp: Some(14.27792), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.531, win_z: 3.564, share_delta_pp: 0.482, share_z: 5.452, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "pantheon-board", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(5), wins_prior_10k: None, win_diff_pp: Some(0.106576), posterior_pp: Some(5.328802), posterior_se_pp: Some(19.058715), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.107, win_z: 0.28, share_delta_pp: 0.013, share_z: 0.169, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "peacetime-deterrence", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(16), wins_prior_10k: Some(21), win_diff_pp: Some(0.349649), posterior_pp: Some(18.012148), posterior_se_pp: Some(8.565639), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.617, win_z: 1.413, share_delta_pp: 0.039, share_z: 0.427, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "power-the-laboratory", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(14), wins_prior_10k: Some(-8), win_diff_pp: Some(0.035908), posterior_pp: Some(0.603865), posterior_se_pp: Some(11.964935), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.29, win_z: 0.762, share_delta_pp: 0.054, share_z: 0.695, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "price-the-suzerainty", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(55), wins_prior_10k: None, win_diff_pp: Some(1.11297), posterior_pp: Some(55.648484), posterior_se_pp: Some(19.144988), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.113, win_z: 2.907, share_delta_pp: 0.337, share_z: 4.293, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "promote-when-wounded", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(7), wins_prior_10k: None, win_diff_pp: Some(0.143269), posterior_pp: Some(7.163433), posterior_se_pp: Some(19.219865), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.143, win_z: 0.373, share_delta_pp: 0.025, share_z: 0.317, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "raid-pillage-prizes", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(43), wins_prior_10k: Some(53), win_diff_pp: Some(1.016339), posterior_pp: Some(53.890257), posterior_se_pp: Some(14.608934), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.698, win_z: 3.94, share_delta_pp: 0.407, share_z: 4.569, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "recon-replacement", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(36), wins_prior_10k: Some(30), win_diff_pp: Some(0.956553), posterior_pp: Some(50.640837), posterior_se_pp: Some(10.654421), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.454, win_z: 3.362, share_delta_pp: 0.418, share_z: 4.632, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "recorded-tactical-step", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(11), wins_prior_10k: Some(18), win_diff_pp: Some(0.321794), posterior_pp: Some(16.717609), posterior_se_pp: Some(8.50066), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.444, win_z: 1.021, share_delta_pp: 0.084, share_z: 0.936, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "relief-targets-the-siege", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(18), wins_prior_10k: Some(6), win_diff_pp: Some(0.191188), posterior_pp: Some(9.598962), posterior_se_pp: Some(8.511735), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.71, win_z: 1.628, share_delta_pp: 0.013, share_z: 0.138, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "religion-sues-peace", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(5), wins_prior_10k: Some(-18), win_diff_pp: Some(0.120977), posterior_pp: Some(6.548389), posterior_se_pp: Some(8.778382), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.091, win_z: 0.238, share_delta_pp: -0.006, share_z: -0.074, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "religious-defence-scales", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(12), wins_prior_10k: Some(-8), win_diff_pp: Some(0.01249), posterior_pp: Some(-0.269738), posterior_se_pp: Some(12.17978), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.238, win_z: 0.619, share_delta_pp: -0.063, share_z: -0.802, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "religious-units-heal-first", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(5), wins_prior_10k: Some(12), win_diff_pp: Some(0.177982), posterior_pp: Some(9.256103), posterior_se_pp: Some(11.985282), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.105, win_z: 0.274, share_delta_pp: 0.009, share_z: 0.109, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "research-tier-premium", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(29), wins_prior_10k: Some(-6), win_diff_pp: Some(0.191249), posterior_pp: Some(9.651653), posterior_se_pp: Some(17.243946), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.575, win_z: 1.503, share_delta_pp: 0.079, share_z: 1.01, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "science-multiplier-payoff", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(23), wins_prior_10k: Some(-8), win_diff_pp: Some(0.120995), posterior_pp: Some(5.644704), posterior_se_pp: Some(15.161652), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.459, win_z: 1.206, share_delta_pp: 0.076, share_z: 0.964, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "score-horizon", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(5), wins_prior_10k: Some(18), win_diff_pp: Some(0.322285), posterior_pp: Some(16.89195), posterior_se_pp: Some(8.532299), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.2, win_z: 0.451, share_delta_pp: 0.139, share_z: 1.524, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "settle-sooner", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(19), wins_prior_10k: Some(28), win_diff_pp: Some(0.654906), posterior_pp: Some(34.563276), posterior_se_pp: Some(10.399355), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.764, win_z: 1.748, share_delta_pp: 0.123, share_z: 1.352, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "settlement-gap-target", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(14), wins_prior_10k: None, win_diff_pp: Some(0.286513), posterior_pp: Some(14.32565), posterior_se_pp: Some(18.969806), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.287, win_z: 0.755, share_delta_pp: 0.055, share_z: 0.707, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "settler-guard-holds", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(25), wins_prior_10k: Some(-8), win_diff_pp: Some(0.080401), posterior_pp: Some(3.289281), posterior_se_pp: Some(8.232809), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.489, win_z: 1.285, share_delta_pp: 0.061, share_z: 0.783, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "settler-target-hysteresis", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(17), wins_prior_10k: Some(-18), win_diff_pp: Some(0.011271), posterior_pp: Some(0.142789), posterior_se_pp: Some(8.235122), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.335, win_z: 0.873, share_delta_pp: 0.12, share_z: 1.532, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "settler-threat-detour", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(1), wins_prior_10k: Some(18), win_diff_pp: Some(0.450991), posterior_pp: Some(24.316588), posterior_se_pp: Some(13.516949), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.021, win_z: 0.047, share_delta_pp: 0.044, share_z: 0.493, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "siege-commitment", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(9), wins_prior_10k: Some(6), win_diff_pp: Some(-0.039073), posterior_pp: Some(-2.000626), posterior_se_pp: Some(8.322987), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.185, win_z: 0.485, share_delta_pp: -0.036, share_z: -0.457, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "slot-kind-tiebreak", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(25), wins_prior_10k: Some(-1), win_diff_pp: Some(0.203632), posterior_pp: Some(9.448489), posterior_se_pp: Some(8.258141), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.507, win_z: 1.329, share_delta_pp: 0.12, share_z: 1.526, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "stranded-settler-discount", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(33), wins_prior_10k: Some(-8), win_diff_pp: Some(0.234064), posterior_pp: Some(11.031107), posterior_se_pp: Some(8.29178), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.659, win_z: 1.743, share_delta_pp: 0.007, share_z: 0.085, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "strategic-wonders", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(0), wins_prior_10k: Some(11), win_diff_pp: Some(0.172073), posterior_pp: Some(8.794727), posterior_se_pp: Some(8.277293), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.007, win_z: -0.018, share_delta_pp: -0.044, share_z: -0.559, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "strike-opening", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(1), wins_prior_10k: Some(4), win_diff_pp: Some(0.229561), posterior_pp: Some(12.164275), posterior_se_pp: Some(8.437152), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.051, win_z: 0.116, share_delta_pp: 0.107, share_z: 1.162, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "unit-cost-efficiency", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(17), wins_prior_10k: None, win_diff_pp: Some(0.347661), posterior_pp: Some(17.383064), posterior_se_pp: Some(19.225877), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.348, win_z: 0.904, share_delta_pp: 0.177, share_z: 2.235, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "unit-objective-memory", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(10), wins_prior_10k: None, win_diff_pp: Some(0.197428), posterior_pp: Some(9.871423), posterior_se_pp: Some(19.11049), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.197, win_z: 0.517, share_delta_pp: 0.138, share_z: 1.788, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "war-economy", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(46), wins_prior_10k: Some(118), win_diff_pp: Some(0.527307), posterior_pp: Some(13.370014), posterior_se_pp: Some(51.998198), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 1.872, win_z: 4.325, share_delta_pp: 1.141, share_z: 12.467, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "war-reinforcement", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(0), wins_prior_10k: Some(34), win_diff_pp: Some(0.326817), posterior_pp: Some(16.756373), posterior_se_pp: Some(10.360862), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: 0.005, win_z: 0.011, share_delta_pp: -0.048, share_z: -0.536, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "whole-turn-backtrack-guard", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-14), wins_prior_10k: Some(6), win_diff_pp: Some(0.20946), posterior_pp: Some(11.765564), posterior_se_pp: Some(9.909905), family_wise: false, screen: Some(Measure { pairs: 19080, win_delta_pp: -0.563, win_z: -1.269, share_delta_pp: -0.124, share_z: -1.37, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
    GeneVerdict { tag: "wide-map-capacity", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(55), wins_prior_10k: Some(91), win_diff_pp: Some(1.198264), posterior_pp: Some(60.530019), posterior_se_pp: Some(16.598622), family_wise: true, screen: Some(Measure { pairs: 19080, win_delta_pp: 2.223, win_z: 5.21, share_delta_pp: 0.577, share_z: 6.468, source: "2026-08-24-standard-continuous-38160-total-seats.json" }) },
];
// ═══ END GENERATED ═══
