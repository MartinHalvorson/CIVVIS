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
//! ⚠ Rows are appended at the end of their block and never re-ordered: the
//! screen writes genes in this order, and `tools/genes.py` re-derives a
//! batch's gene set from this file at the commit the batch names.
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
    /// Host-only in the live bundle AND an explicit native opt-in — the one
    /// such row is `joint-tactics`, whose search runs on a native board and is
    /// screened as an opt-in.
    HostOnlyOptIn,
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
        matches!(
            self.kind,
            Kind::Repair(_) | Kind::HostOnly | Kind::HostOnlyOptIn
        )
    }
    /// Reads host state a native board does not have.
    pub const fn host_only(&self) -> bool {
        matches!(self.kind, Kind::HostOnly | Kind::HostOnlyOptIn)
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
        matches!(self.kind, Kind::OptIn | Kind::HostOnlyOptIn)
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
    // Taking a city, and finishing the one already broken open.
    Gene { tag: "siege-tracks-wall", field: "siege_tracks_wall", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_siege_tracks_the_wall, disable: AdvancedAi::disable_siege_tracks_the_wall },
    // ⚠ And the war it keeps prosecuting still has to end on a captured
    // city. The campaign re-picks its objective from scratch every turn and
    // prices fifteen turns of siege at ~37 points, less than the distance
    // terms swing; the army walks off a city at 25 hp with its walls down
    // and Civ 6 heals it back at 20 hp a turn. Live run
    // `civvis-20260808T142724Z` dealt 338 hp of city damage over t73-t105,
    // handed 200 of it back, and took nothing — the shape behind 25 live
    // games and 0 captures on 7.7x the field's military.
    Gene { tag: "siege-commitment", field: "siege_commitment", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_siege_commitment, disable: AdvancedAi::disable_siege_commitment },
    // ⚠ A war that grinds a wall for 12 turns reads as stalled, and
    // fatigue then offers peace AND accepts any white peace at +320 —
    // followed by a 30-turn re-declaration lockout. The stall clause is
    // right when the sides are close and self-defeating when the attacker
    // holds `OVERWHELMING_WAR_RATIO` over the defender: the measured live
    // pattern is one declaration per game and no second attempt.
    Gene { tag: "war-patience", field: "war_patience", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_war_patience, disable: AdvancedAi::disable_war_patience },
    // ⚠ And the fatigue clock must not offer away a siege that is landing
    // net damage — Chennai at 190/200, "the war has stalled: 1180 power
    // against their 82". See `siege_is_progress`.
    // And a siege landing net damage resets the fatigue clock, so the
    // peace desk cannot offer away a war one hit from a capture. See
    // `siege_is_progress`.
    Gene { tag: "siege-is-progress", field: "siege_is_progress", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_siege_is_progress, disable: AdvancedAi::disable_siege_is_progress },
    // ⚠ A counter-leader emergency declaration reached France at t235 with
    // only sixteen turns left, captured nothing, and Zulu won at t251.
    // Timed attacks already reserve their scaled campaign window; the
    // direct denial fallback must not spend a war on less runway.
    Gene { tag: "endgame-war-runway", field: "endgame_war_runway", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_endgame_war_runway, disable: AdvancedAi::disable_endgame_war_runway },
    // ⚠ Barbarians are excluded from `at_major_war` by design, so every defensive
    // escalation in the production picker reads a barbarian siege as no threat at
    // all: a one-city empire's standing-army floor stays at `mil_per_city` (1.0)
    // and it cannot want a third defender while horsemen stand on its doorstep.
    // Measured on run `civvis-20260802T202501Z` — four settlers built into that
    // siege and captured, two on the capital tile without ever moving, one city
    // until t80, score 140 against a best rival's 416. The tournament controller
    // stays frozen so its recorded ladders remain comparable.
    // ⚠ And once it CAN want the defenders, something has to send them. Measured
    // on run `civvis-20260803T005930Z` (Kongo, 154 turns): **116 of 154 turns had
    // a hostile standing inside or beside our own territory**, including a
    // full-health Crossbowman parked four tiles from two cities, unmoved and
    // unengaged, for 21 consecutive turns, while the whole seven-unit army stood
    // eight tiles away on a war front that had taken nothing in 75 turns. The
    // cause is `nearest_enemy` ranking targets by distance FROM THE ASKING UNIT,
    // which for a deployed army is always the enemy's cities. The tournament
    // controller stays frozen so its recorded ladders remain comparable.
    // Holding one. Barbarians take 7.0 major cities a game, 65% of
    // everything a major loses.
    Gene { tag: "home-defense", field: "home_defense", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_home_defense, disable: AdvancedAi::disable_home_defense },
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
    // And a raider standing over a Settler ten tiles out is somebody's
    // job, which it was not while the admission test measured only the
    // distance to our cities: eight Settlers taken in 104 turns on run
    // civvis-20260821T130446Z. See `barbarian_hunt`.
    // And a raider standing over a Settler on the road is home ground
    // too — the admission test that admits the barbarian seat at all
    // measures distance from our CITIES, so the walk to a site ten tiles
    // out is unguarded ground by construction. See `BasicAi::barbarian_hunt`.
    Gene { tag: "barbarian-hunt", field: "barbarian_hunt", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_barbarian_hunt, disable: AdvancedAi::disable_barbarian_hunt },
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
    // And a capturable civilian within reach is walked onto, never
    // watched from adjacency — and a settler in the barbarians' hands is
    // never declined. Run `civvis-20260818T222844Z` t27–t33: our own
    // captured settler passed four units unguarded and was lost to a
    // camp. See `BasicAi::civilian_rescue`.
    // And a capturable civilian within reach is walked onto — a settler
    // taken back from the barbarians repays its whole production. See
    // `BasicAi::civilian_rescue`.
    Gene { tag: "civilian-rescue", field: "civilian_rescue", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_civilian_rescue, disable: AdvancedAi::disable_civilian_rescue },
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
    Gene { tag: "wonder-ring-settle-value", field: "wonder_ring_settle_value", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_wonder_ring_settle_value, disable: AdvancedAi::disable_wonder_ring_settle_value },
    // And never paying for a Settler the march will refuse to land —
    // 19 starts became 8 foundings on the first hostile map after the
    // land-grab pipeline. See `settler_site_agreement`.
    // And never paying for a Settler the march will refuse to land. See
    // `settler_site_agreement`.
    Gene { tag: "settler-site-agreement", field: "settler_site_agreement", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_settler_site_agreement, disable: AdvancedAi::disable_settler_site_agreement },
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
    // Growing what was founded. Housing is gated by a tech the argmax
    // never aims at, so the district, the buildings, the cards and the
    // research order have to move together or none of them binds.
    Gene { tag: "housing-districts", field: "housing_districts", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_housing_districts, disable: AdvancedAi::disable_housing_districts },
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
    Gene { tag: "governor-every-lane", field: "governor_every_lane", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_governor_every_lane, disable: AdvancedAi::disable_governor_every_lane },
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
    // And the coverage that price alone never bought. See
    // `culture_coverage`.
    Gene { tag: "culture-coverage", field: "culture_coverage", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_culture_coverage, disable: AdvancedAi::disable_culture_coverage },
    // And every specialty district's own buildings, whatever the lane —
    // eight Campuses and six Theater Squares stood empty to turn 205 on
    // run civvis-20260819T000800Z while the queue bought Builders, Baths
    // and Harbors over a Library priced at 23. See
    // `district_building_chain`.
    Gene { tag: "district-building-chain", field: "district_building_chain", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_district_building_chain, disable: AdvancedAi::disable_district_building_chain },
    // ── Host-only: shipped by the Civilization VI seat, inert headless ──
    Gene { tag: "joint-reach-lines", field: "joint_reach_lines", kind: Kind::HostOnly, enable: AdvancedAi::enable_joint_reach_lines, disable: AdvancedAi::disable_joint_reach_lines },
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
    // A unit that uncovers new ground re-decides the rest of its movement
    // on what it saw, instead of finishing a path planned blind. See
    // `step_and_reassess`; on the bridge it is the brain half of the
    // mid-turn replan frame.
    Gene { tag: "step-and-reassess", field: "step_and_reassess", kind: Kind::HostOnly, enable: AdvancedAi::enable_step_and_reassess, disable: AdvancedAi::disable_step_and_reassess },
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
    // A Builder's finite charges first pay today's worked yields, except for
    // luxury and strategic connections that pay empire-wide either way.
    // `gene_screen` discovers this native opt-in directly from this row.
    Gene { tag: "builder-worked-tile-priority", field: "builder_worked_tile_priority", kind: Kind::OptIn, enable: AdvancedAi::enable_builder_worked_tile_priority, disable: AdvancedAi::disable_builder_worked_tile_priority },
    Gene { tag: "apostle-promotion-by-role", field: "apostle_promotion_by_role", kind: Kind::OptIn, enable: AdvancedAi::enable_apostle_promotion_by_role, disable: AdvancedAi::disable_apostle_promotion_by_role },
    // The joint engagement search (`docs/TACTICS.md`): production ships it
    // off, the bridge turns it on, and `advanced_joint_tactics` is
    // `AdvancedAi::new()` plus this one flag — so it is a native opt-in like
    // any other. Listed here so `gene_screen` can price it in whole native
    // games beside the engine repairs and `victory_eval --with joint-tactics`
    // can seat it by name. (`joint-reach-lines` rides with it — the enable
    // turns both on; the lines alone are priced on `battle_bench`, §17.)
    Gene { tag: "joint-tactics", field: "joint_tactics", kind: Kind::HostOnlyOptIn, enable: AdvancedAi::enable_joint_tactics, disable: AdvancedAi::disable_joint_tactics },
    // Item 4 of `docs/LIVE_TACTICS.md`: rear reinforcements arrive at an
    // engaged front as a wave rather than one at a time. Off everywhere
    // until the screen says otherwise; see `AdvancedAi::enable_arrival_waves`.
    // The religion race decides two thirds of native games, and a founder
    // that loses its own cities wins as rarely as a seat that never founded
    // (3.0% v 3.0%, 13,446 seat-pairs). These two are priced against the
    // deployment genome before either ships; see the fields' docs.
    Gene { tag: "inquisition-on-threat", field: "inquisition_on_threat", kind: Kind::OptIn, enable: AdvancedAi::enable_inquisition_on_threat, disable: AdvancedAi::disable_inquisition_on_threat },
    Gene { tag: "founder-temple", field: "founder_temple", kind: Kind::OptIn, enable: AdvancedAi::enable_founder_temple, disable: AdvancedAi::disable_founder_temple },
    Gene { tag: "theology-for-founders", field: "theology_for_founders", kind: Kind::OptIn, enable: AdvancedAi::enable_theology_for_founders, disable: AdvancedAi::disable_theology_for_founders },
    Gene { tag: "holy-lane-parity", field: "holy_lane_parity", kind: Kind::OptIn, enable: AdvancedAi::enable_holy_lane_parity, disable: AdvancedAi::disable_holy_lane_parity },
    // Half the seats never found a religion and bank ~1,000 Faith they
    // cannot spend; see `AdvancedAi::idle_faith_patronage`.
    Gene { tag: "idle-faith-patronage", field: "idle_faith_patronage", kind: Kind::OptIn, enable: AdvancedAi::enable_idle_faith_patronage, disable: AdvancedAi::disable_idle_faith_patronage },
    // The rest of the same chain: the founder's corps cannot answer more than
    // two cities, cannot heal when it is defending, spends charges at a
    // fraction of their strength, and never uses the World Congress licence to
    // remove a carrier at peace. See `advanced/religion.rs`.
    Gene { tag: "religious-defence-scales", field: "religious_defence_scales", kind: Kind::OptIn, enable: AdvancedAi::enable_religious_defence_scales, disable: AdvancedAi::disable_religious_defence_scales },
    Gene { tag: "guru-heals-the-corps", field: "guru_heals_the_corps", kind: Kind::OptIn, enable: AdvancedAi::enable_guru_heals_the_corps, disable: AdvancedAi::disable_guru_heals_the_corps },
    Gene { tag: "religious-units-heal-first", field: "religious_units_heal_first", kind: Kind::OptIn, enable: AdvancedAi::enable_religious_units_heal_first, disable: AdvancedAi::disable_religious_units_heal_first },
    Gene { tag: "condemn-under-congress", field: "condemn_under_congress", kind: Kind::OptIn, enable: AdvancedAi::enable_condemn_under_congress, disable: AdvancedAi::disable_condemn_under_congress },
    Gene { tag: "spread-campaign-persists", field: "spread_campaign_persists", kind: Kind::OptIn, enable: AdvancedAi::enable_spread_campaign_persists, disable: AdvancedAi::disable_spread_campaign_persists },
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
    // A settler prices a site by the districts the plan would build there,
    // and a treasury buys a border plot only when it pays for itself. See
    // `advanced/site_lookahead.rs`.
    // A unit in contact prices the exchange it is standing in, not only the
    // attacks it could make: stand and heal against melee that has to come to
    // it, close on or leave an unanswerable shooter. See
    // `advanced/contact_posture.rs`.
    Gene { tag: "contact-posture", field: "contact_posture", kind: Kind::OptIn, enable: AdvancedAi::enable_contact_posture, disable: AdvancedAi::disable_contact_posture },
    Gene { tag: "district-lookahead-settle", field: "district_lookahead_settle", kind: Kind::OptIn, enable: AdvancedAi::enable_district_lookahead_settle, disable: AdvancedAi::disable_district_lookahead_settle },
    Gene { tag: "priced-tile-purchase", field: "priced_tile_purchase", kind: Kind::OptIn, enable: AdvancedAi::enable_priced_tile_purchase, disable: AdvancedAi::disable_priced_tile_purchase },
    // Every science term in the controller tapers to zero while the flat
    // constants it competes with never do; this asks whether the investment
    // can still repay instead. See `AdvancedAi::science_payback_horizon`.
    Gene { tag: "science-payback-horizon", field: "science_payback_horizon", kind: Kind::OptIn, enable: AdvancedAi::enable_science_payback_horizon, disable: AdvancedAi::disable_science_payback_horizon },
    // A Library bought after Rationalism earns twice a Library bought before
    // it, and the price never noticed. See
    // `AdvancedAi::science_multiplier_payoff`.
    Gene { tag: "science-multiplier-payoff", field: "science_multiplier_payoff", kind: Kind::OptIn, enable: AdvancedAi::enable_science_multiplier_payoff, disable: AdvancedAi::disable_science_multiplier_payoff },
    // The chain's rungs are 2, 4 and 3-plus-5 and the debt that buys them is
    // flat. See `AdvancedAi::research_tier_premium`.
    Gene { tag: "research-tier-premium", field: "research_tier_premium", kind: Kind::OptIn, enable: AdvancedAi::enable_research_tier_premium, disable: AdvancedAi::disable_research_tier_premium },
    // Measured: every extra Campus bought late came out of a Research Lab
    // that then never got built. See `AdvancedAi::campus_finishes_first`.
    Gene { tag: "campus-finishes-first", field: "campus_finishes_first", kind: Kind::OptIn, enable: AdvancedAi::enable_campus_finishes_first, disable: AdvancedAi::disable_campus_finishes_first },
    // The empire builds the laboratory and then declines to staff it. See
    // `AdvancedAi::research_floor_holds`.
    Gene { tag: "research-floor-holds", field: "research_floor_holds", kind: Kind::OptIn, enable: AdvancedAi::enable_research_floor_holds, disable: AdvancedAi::disable_research_floor_holds },
    // `governor-every-lane` is a losing composite: the deployment screen's
    // score-share drag is large enough that the two pre-existing halves need
    // their own randomised comparisons before either can be retained. The
    // composite remains the live-bridge compatibility switch; these opt-ins
    // let a native seat turn on exactly one of its established predicates.
    Gene { tag: "governor-victory-lanes", field: "governor_victory_lanes", kind: Kind::OptIn, enable: AdvancedAi::enable_governor_victory_lanes, disable: AdvancedAi::disable_governor_victory_lanes },
    Gene { tag: "governor-expansion-lane", field: "governor_expansion_lane", kind: Kind::OptIn, enable: AdvancedAi::enable_governor_expansion_lane, disable: AdvancedAi::disable_governor_expansion_lane },
    // `withdraw_hp` is a constant and the enemy's damage is not: a unit the
    // strongest thing in reach would kill in one blow recovers whatever its
    // hit points say, and healing ground that comes under a shooter's reach
    // is left. See `BasicAi::one_shot_recovery`.
    Gene { tag: "one-shot-recovery", field: "one_shot_recovery", kind: Kind::OptIn, enable: AdvancedAi::enable_one_shot_recovery, disable: AdvancedAi::disable_one_shot_recovery },
    // The six victory-lane genes (`advanced/victory_lane.rs`,
    // `docs/VICTORY_GENES.md`). A targeted seat spends about a fifth of the
    // game — and an adaptive one 15% — with `Expansion` in its plan, and
    // `take_turn_inner` hands that to every lane-shaped decider. Each of the
    // first five substitutes the victory the empire is actually racing at ONE
    // of them; the sixth prices the Diplomatic Victory Points a scored
    // competition pays, which nothing priced.
    Gene { tag: "lane-congress-ballot", field: "lane_congress_ballot", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_congress_ballot, disable: AdvancedAi::disable_lane_congress_ballot },
    Gene { tag: "lane-congress-favor", field: "lane_congress_favor", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_congress_favor, disable: AdvancedAi::disable_lane_congress_favor },
    Gene { tag: "lane-great-people", field: "lane_great_people", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_great_people, disable: AdvancedAi::disable_lane_great_people },
    Gene { tag: "lane-policy-deck", field: "lane_policy_deck", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_policy_deck, disable: AdvancedAi::disable_lane_policy_deck },
    Gene { tag: "lane-culture-spending", field: "lane_culture_spending", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_culture_spending, disable: AdvancedAi::disable_lane_culture_spending },
    Gene { tag: "lane-space-race", field: "lane_space_race", kind: Kind::OptIn, enable: AdvancedAi::enable_lane_space_race, disable: AdvancedAi::disable_lane_space_race },
    Gene { tag: "competition-victory-points", field: "competition_victory_points", kind: Kind::OptIn, enable: AdvancedAi::enable_competition_victory_points, disable: AdvancedAi::disable_competition_victory_points },
    // Three behaviours that already existed and could not be screened: off in
    // production, reachable only as named `elo.rs` arms, so `gene_screen`
    // never saw them. `docs/VICTORY_GENES.md` §9 counts these fields; these
    // are the three the Diplomacy lane needs — and the first has a measured
    // number sitting unused in its own field doc (26 of 192 ballot decisions
    // already settled, ~1.4 free Diplomatic Victory Points a seat a game).
    Gene { tag: "congress-banks-decided", field: "congress_banks_a_decided_vote", kind: Kind::OptIn, enable: AdvancedAi::enable_congress_banks_a_decided_vote, disable: AdvancedAi::disable_congress_banks_a_decided_vote },
    Gene { tag: "congress-counter-votes", field: "congress_counter_votes", kind: Kind::OptIn, enable: AdvancedAi::enable_congress_counter_votes, disable: AdvancedAi::disable_congress_counter_votes },
    Gene { tag: "envoy-infrastructure", field: "envoy_infrastructure", kind: Kind::OptIn, enable: AdvancedAi::enable_envoy_infrastructure, disable: AdvancedAi::disable_envoy_infrastructure },
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
    // The one multiplier gate this empire can actually reach: five Campus
    // cities growing, five citizens short each. Appended at the END so a
    // running screen keeps its positional genome. See
    // `AdvancedAi::fifteenth_citizen`.
    Gene { tag: "fifteenth-citizen", field: "fifteenth_citizen", kind: Kind::OptIn, enable: AdvancedAi::enable_fifteenth_citizen, disable: AdvancedAi::disable_fifteenth_citizen },
    // The chain is clock-bound and the clock is serialized: Chemistry follows
    // the University STANDING by 36-73 turns. Appended at the END so a running
    // screen keeps its positional genome. See
    // `AdvancedAi::chain_tech_lookahead`.
    Gene { tag: "chain-tech-lookahead", field: "chain_tech_lookahead", kind: Kind::OptIn, enable: AdvancedAi::enable_chain_tech_lookahead, disable: AdvancedAi::disable_chain_tech_lookahead },
    // Priced second over 1,113 finished-city turns and run zero times, while
    // the nuclear projects took 14% of them. Appended at the END so a running
    // screen keeps its positional genome. See
    // `AdvancedAi::research_grants_first`.
    Gene { tag: "research-grants-first", field: "research_grants_first", kind: Kind::OptIn, enable: AdvancedAi::enable_research_grants_first, disable: AdvancedAi::disable_research_grants_first },
    // A visible Barbarian Settler or Scout that a military unit can take this
    // turn outranks healing and every tactical alternative. Appended at the
    // END so a running screen keeps its positional genome. See
    // `BasicAi::barbarian_capture_priority`.
    Gene { tag: "barbarian-capture-priority", field: "barbarian_capture_priority", kind: Kind::OptIn, enable: AdvancedAi::enable_barbarian_capture_priority, disable: AdvancedAi::disable_barbarian_capture_priority },
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
    // Price a Settler as the coupled investment it is — production, population,
    // escort, route, safety and founding lag together — instead of a unit cost.
    Gene { tag: "coupled-expansion", field: "coupled_expansion", kind: Kind::OptIn, enable: AdvancedAi::enable_coupled_expansion, disable: AdvancedAi::disable_coupled_expansion },
    // The friendly-volley extension without the rest of the closed war-half
    // bundle: a force finishes a defender together. The volley shipped inside
    // `tactical_strategy` and left with that bundle's removal (#1589, +38 for
    // the composite), and a composite gate never prices its parts.
    Gene { tag: "coordinated-finish", field: "coordinated_finish", kind: Kind::OptIn, enable: AdvancedAi::enable_coordinated_finish, disable: AdvancedAi::disable_coordinated_finish },
    // The other two survivors of the same 2026-08-14 withhold, which measured
    // +32/+34 as a bundle of four and has never been priced one flag at a
    // time. `advanced_war_half` re-adds them together; these price them apart.
    Gene { tag: "tactical-strategy", field: "tactical_strategy", kind: Kind::OptIn, enable: AdvancedAi::enable_tactical_strategy, disable: AdvancedAi::disable_tactical_strategy },
    Gene { tag: "unit-objective-memory", field: "unit_objective_memory", kind: Kind::OptIn, enable: AdvancedAi::enable_unit_objective_memory, disable: AdvancedAi::disable_unit_objective_memory },
    // Choose the pantheon from the land the empire actually holds rather than
    // from a fixed order.
    Gene { tag: "pantheon-board", field: "pantheon_board", kind: Kind::OptIn, enable: AdvancedAi::enable_pantheon_board, disable: AdvancedAi::disable_pantheon_board },
    // The policy counterfactual sees the unit-maintenance bill, so the cards
    // that pay it stop scoring zero.
    Gene { tag: "maintenance-aware-deck", field: "maintenance_aware_deck", kind: Kind::OptIn, enable: AdvancedAi::enable_maintenance_aware_deck, disable: AdvancedAi::disable_maintenance_aware_deck },
    // A unit the planner gave nothing to do fortifies instead of standing.
    Gene { tag: "fortify-idle-units", field: "fortify_idle_units", kind: Kind::OptIn, enable: AdvancedAi::enable_fortify_idle_units, disable: AdvancedAi::disable_fortify_idle_units },
    // Splice the +100% naval-production card family in while hulls are wanted.
    Gene { tag: "naval-production-policy", field: "naval_production_policy", kind: Kind::OptIn, enable: AdvancedAi::enable_naval_production_policy, disable: AdvancedAi::disable_naval_production_policy },
    // Credit strength-per-production, and the civilization's unique unit, in
    // military production.
    Gene { tag: "unit-cost-efficiency", field: "unit_cost_efficiency", kind: Kind::OptIn, enable: AdvancedAi::enable_unit_cost_efficiency, disable: AdvancedAi::disable_unit_cost_efficiency },
    // Price Builder production by a survey of the work it would actually do
    // rather than by `ceil(cities / 2)`.
    Gene { tag: "builder-reward-survey", field: "builder_reward_survey", kind: Kind::OptIn, enable: AdvancedAi::enable_builder_reward_survey, disable: AdvancedAi::disable_builder_reward_survey },
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
    // A Settler ranks a site by the cities it leaves room for as well as its
    // own ground: the best twelve candidates each add a share of the best two
    // sites that stay settleable once they are founded, so the one plot in a
    // pocket that would have held two cities loses to the edge that keeps
    // both. Operator, 2026-08-23: "be mindful of our likely future settling
    // spots when settling cities." See `AdvancedAi::settle_plan_ahead`.
    Gene { tag: "settle-plan-ahead", field: "settle_plan_ahead", kind: Kind::OptIn, enable: AdvancedAi::enable_settle_plan_ahead, disable: AdvancedAi::disable_settle_plan_ahead },
    // ⭐ THE FIRST VERSIONED GENE. `escort-unstick` (a Repair, ships on)
    // releases a linked escort after two turns without closing on the site —
    // blind to WHY the pair stopped. Watched live 2026-08-23: the formation
    // stalled beside a barbarian raider, the release handed the warrior back
    // to the army, and the raider took the settler the next turn. Version 2
    // is the same release refused while a visible raider can reach the
    // settler's tile, at both release points (the settler's unstick and the
    // escort's route abandonment). One version of the family plays and the
    // ledger ships the best (`docs/GENE_SCREEN.md`, *Versioning a gene*).
    // Appended at the END so a running screen keeps its positional genome.
    Gene { tag: "escort-unstick-2", field: "escort_unstick_2", kind: Kind::OptIn, enable: AdvancedAi::enable_escort_unstick_2, disable: AdvancedAi::disable_escort_unstick_2 },
    // The end-to-end fair-play major: plan the whole turn inside a
    // fog-redacted clone, replay only the resulting actions on the real
    // board. Its one recorded screen was 20 paired maps on the retired
    // `ai_eval` (15.0%, 95% Wilson 5.2%..36.0%), and `ai_eval` was deleted in
    // #2351 — so until this row the arm the repository most wants re-screened
    // had no instrument at all. `docs/genome_reach_debt.json` held it out as
    // an aggregate over `blind-objective-strength` and `blind-objective-units`;
    // it still is one, and this column prices the arm AS CONSTRUCTED
    // (`AdvancedAi::fog_honest()`), which is exactly what the 15.0% priced.
    // Appended at the END so a running screen keeps its positional genome.
    // See `AdvancedAi::fog_honest`.
    Gene { tag: "fog-honest", field: "fog_honest", kind: Kind::OptIn, enable: AdvancedAi::enable_fog_honest, disable: AdvancedAi::disable_fog_honest },
    // Version 2 of `fog-honest`. Version 1 plans one turn against a world with
    // no hidden units in it and replays that tape whatever the board says:
    // when an order is refused, every later order on the tape assumed it had
    // happened, and version 1 applies them anyway and ends the turn on the
    // tape's own EndTurn. Version 2 drops the stale remainder and crosses the
    // fog boundary again from the board as it now stands, at most
    // `FOG_REPLAN_LIMIT` times a turn. Being a version of that mode, enabling
    // it enables the mode. See `AdvancedAi::fog_honest_2`.
    Gene { tag: "fog-honest-2", field: "fog_honest_2", kind: Kind::OptIn, enable: AdvancedAi::enable_fog_honest_2, disable: AdvancedAi::disable_fog_honest_2 },
];

// ═══ GENERATED BY tools/genes.py — THE VERDICTS. Do not edit below: `python3 tools/genes.py write` ═══
//
// Source: docs/gene_ledger.json (the same tool writes both); `genes.py check` holds them
// together, and `the_default_follows_the_ledgers_authority` re-derives every `default_on`
// below from the figures beside it under `LEDGER_AUTHORITY`.

/// Which rule decided every `default_on` below: `AUTHORITY` in `tools/genes.py`.
pub(super) const LEDGER_AUTHORITY: &str = "columns";

#[rustfmt::skip]
pub(super) const VERDICTS: &[GeneVerdict] = &[
    GeneVerdict { tag: "air-surge", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(108), wins_prior_10k: None, win_diff_pp: Some(2.150538), posterior_pp: Some(107.526882), posterior_se_pp: Some(15.380193), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 2.151, win_z: 6.991, share_delta_pp: 0.45, share_z: 7.737, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "amenity-district-path", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(17), wins_prior_10k: Some(12), win_diff_pp: Some(0.206772), posterior_pp: Some(10.511592), posterior_se_pp: Some(9.233796), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.339, win_z: 1.073, share_delta_pp: 0.059, share_z: 1.026, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "amenity-project-preemption", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-34), wins_prior_10k: Some(-14), win_diff_pp: Some(-0.189541), posterior_pp: Some(-7.259857), posterior_se_pp: Some(13.707692), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.677, win_z: -2.166, share_delta_pp: -0.115, share_z: -2.003, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "apostle-promotion-by-role", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(16), wins_prior_10k: Some(14), win_diff_pp: Some(0.068924), posterior_pp: Some(2.117182), posterior_se_pp: Some(12.73794), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.322, win_z: 1.017, share_delta_pp: -0.05, share_z: -0.865, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "army-target-weighs-enemy", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(18), wins_prior_10k: Some(30), win_diff_pp: Some(0.129232), posterior_pp: Some(5.247431), posterior_se_pp: Some(13.027793), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.364, win_z: 1.152, share_delta_pp: 0.01, share_z: 0.176, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "barbarian-bargain", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(10), wins_prior_10k: Some(5), win_diff_pp: Some(0.155355), posterior_pp: Some(7.845452), posterior_se_pp: Some(11.982251), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.203, win_z: 0.65, share_delta_pp: 0.026, share_z: 0.45, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "barbarian-capture-priority", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(1), wins_prior_10k: None, win_diff_pp: Some(0.0254), posterior_pp: Some(1.270003), posterior_se_pp: Some(15.767991), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.025, win_z: 0.081, share_delta_pp: -0.124, share_z: -2.099, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "barbarian-hunt", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(10), wins_prior_10k: Some(-86), win_diff_pp: Some(-0.62142), posterior_pp: Some(-37.655923), posterior_se_pp: Some(48.323025), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.203, win_z: 0.645, share_delta_pp: -0.134, share_z: -2.286, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "barbarian-ranged-answer", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(17), wins_prior_10k: Some(14), win_diff_pp: Some(0.31071), posterior_pp: Some(15.552532), posterior_se_pp: Some(11.955023), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.33, win_z: 1.053, share_delta_pp: -0.011, share_z: -0.203, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "barbarian-scouts-are-scouts", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(12), wins_prior_10k: Some(23), win_diff_pp: Some(0.67488), posterior_pp: Some(35.073709), posterior_se_pp: Some(12.030762), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.246, win_z: 0.788, share_delta_pp: 0.022, share_z: 0.389, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "blind-objective-strength", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(0), wins_prior_10k: Some(30), win_diff_pp: Some(0.361851), posterior_pp: Some(17.659739), posterior_se_pp: Some(9.164527), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.008, win_z: -0.027, share_delta_pp: 0.003, share_z: 0.054, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "blind-objective-units", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(0), wins_prior_10k: Some(-7), win_diff_pp: Some(0.002872), posterior_pp: Some(0.003378), posterior_se_pp: Some(9.059427), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.008, win_z: -0.028, share_delta_pp: -0.057, share_z: -0.985, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "bounded-recovery", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(29), wins_prior_10k: Some(19), win_diff_pp: Some(0.571494), posterior_pp: Some(28.446104), posterior_se_pp: Some(9.215507), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.576, win_z: 1.842, share_delta_pp: 0.187, share_z: 3.158, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "builder-barbarian-safety", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-10), wins_prior_10k: Some(13), win_diff_pp: Some(-0.004855), posterior_pp: Some(-0.343822), posterior_se_pp: Some(11.865083), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.195, win_z: -0.624, share_delta_pp: 0.004, share_z: 0.075, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "builder-worked-tile-priority", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: Some(24), win_diff_pp: Some(0.111661), posterior_pp: Some(6.840167), posterior_se_pp: Some(16.426383), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.169, win_z: -0.54, share_delta_pp: -0.044, share_z: -0.747, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "buildings-before-projects", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(61), wins_prior_10k: Some(-2), win_diff_pp: Some(0.605956), posterior_pp: Some(28.233682), posterior_se_pp: Some(14.547491), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 1.228, win_z: 3.949, share_delta_pp: 0.282, share_z: 4.886, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "camp-party", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-15), wins_prior_10k: Some(13), win_diff_pp: Some(0.353235), posterior_pp: Some(21.505307), posterior_se_pp: Some(16.177401), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.305, win_z: -0.959, share_delta_pp: -0.012, share_z: -0.208, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "campus-adjacency-threshold", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-18), wins_prior_10k: None, win_diff_pp: Some(-0.355601), posterior_pp: Some(-17.780036), posterior_se_pp: Some(15.495953), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.356, win_z: -1.147, share_delta_pp: -0.059, share_z: -1.018, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "campus-finishes-first", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(4), wins_prior_10k: None, win_diff_pp: Some(0.0762), posterior_pp: Some(3.810008), posterior_se_pp: Some(15.550386), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.076, win_z: 0.245, share_delta_pp: 0.096, share_z: 1.643, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "chain-tech-lookahead", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-25), wins_prior_10k: None, win_diff_pp: Some(-0.491068), posterior_pp: Some(-24.553382), posterior_se_pp: Some(15.791551), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.491, win_z: -1.555, share_delta_pp: -0.034, share_z: -0.586, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "civilian-rescue", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-1), wins_prior_10k: Some(-6), win_diff_pp: Some(-0.080411), posterior_pp: Some(-4.012919), posterior_se_pp: Some(9.232314), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.025, win_z: -0.08, share_delta_pp: 0.096, share_z: 1.647, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "come-ashore", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(5), wins_prior_10k: Some(11), win_diff_pp: Some(0.178053), posterior_pp: Some(9.201381), posterior_se_pp: Some(9.912531), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.093, win_z: 0.293, share_delta_pp: 0.015, share_z: 0.249, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "competition-victory-points", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(0), wins_prior_10k: None, win_diff_pp: Some(-0.008467), posterior_pp: Some(-0.423334), posterior_se_pp: Some(15.847382), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.008, win_z: -0.027, share_delta_pp: -0.069, share_z: -1.169, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "condemn-under-congress", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: None, win_diff_pp: Some(-0.1524), posterior_pp: Some(-7.620015), posterior_se_pp: Some(15.532728), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.152, win_z: -0.491, share_delta_pp: 0.037, share_z: 0.639, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "congress-banks-decided", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: None, win_diff_pp: Some(-0.169334), posterior_pp: Some(-8.466684), posterior_se_pp: Some(15.497956), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.169, win_z: -0.546, share_delta_pp: -0.043, share_z: -0.732, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "congress-counter-votes", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-13), wins_prior_10k: None, win_diff_pp: Some(-0.254001), posterior_pp: Some(-12.700025), posterior_se_pp: Some(15.381116), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.254, win_z: -0.826, share_delta_pp: 0.064, share_z: 1.113, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "contact-posture", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-55), wins_prior_10k: None, win_diff_pp: Some(-1.092202), posterior_pp: Some(-54.610109), posterior_se_pp: Some(15.398598), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -1.092, win_z: -3.546, share_delta_pp: -0.358, share_z: -6.148, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "culture-building-debt", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(24), wins_prior_10k: None, win_diff_pp: Some(0.482601), posterior_pp: Some(24.130048), posterior_se_pp: Some(15.842715), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.483, win_z: 1.523, share_delta_pp: 0.05, share_z: 0.833, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "culture-coverage", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-14), wins_prior_10k: None, win_diff_pp: Some(-0.279401), posterior_pp: Some(-13.970028), posterior_se_pp: Some(15.409945), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.279, win_z: -0.907, share_delta_pp: 0.083, share_z: 1.435, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "district-building-chain", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-1), wins_prior_10k: None, win_diff_pp: Some(-0.016933), posterior_pp: Some(-0.846668), posterior_se_pp: Some(15.705348), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.017, win_z: -0.054, share_delta_pp: -0.183, share_z: -3.154, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "district-coverage", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-22), wins_prior_10k: Some(32), win_diff_pp: Some(-0.066052), posterior_pp: Some(-2.972191), posterior_se_pp: Some(12.462119), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.44, win_z: -1.44, share_delta_pp: -0.021, share_z: -0.358, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "district-lookahead-settle", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-41), wins_prior_10k: Some(-22), win_diff_pp: Some(-0.655403), posterior_pp: Some(-32.809202), posterior_se_pp: Some(11.759247), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.821, win_z: -2.649, share_delta_pp: -0.117, share_z: -1.991, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "early-contact-window", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(2), wins_prior_10k: None, win_diff_pp: Some(0.033867), posterior_pp: Some(1.693337), posterior_se_pp: Some(15.567762), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.034, win_z: 0.109, share_delta_pp: 0.022, share_z: 0.382, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "endgame-war-runway", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-17), wins_prior_10k: Some(-5), win_diff_pp: Some(-0.169438), posterior_pp: Some(-8.646684), posterior_se_pp: Some(9.215306), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.33, win_z: -1.059, share_delta_pp: -0.057, share_z: -0.979, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "enhancer-for-the-corps", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(8), wins_prior_10k: None, win_diff_pp: Some(0.1524), posterior_pp: Some(7.620015), posterior_se_pp: Some(15.818613), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.152, win_z: 0.482, share_delta_pp: -0.03, share_z: -0.512, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "envoy-infrastructure", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-6), wins_prior_10k: None, win_diff_pp: Some(-0.118534), posterior_pp: Some(-5.926679), posterior_se_pp: Some(15.705069), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.119, win_z: -0.377, share_delta_pp: 0.003, share_z: 0.051, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "escort-unstick", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(36), wins_prior_10k: Some(72), win_diff_pp: Some(0.637546), posterior_pp: Some(30.124123), posterior_se_pp: Some(16.042415), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.72, win_z: 2.287, share_delta_pp: 0.166, share_z: 2.853, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "fifteenth-citizen", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-12), wins_prior_10k: None, win_diff_pp: Some(-0.237067), posterior_pp: Some(-11.853357), posterior_se_pp: Some(15.612633), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.237, win_z: -0.759, share_delta_pp: -0.067, share_z: -1.136, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "fog-honest", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-1006), wins_prior_10k: None, win_diff_pp: Some(-13.526097), posterior_pp: Some(-676.304827), posterior_se_pp: Some(144.664113), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 207, win_delta_pp: -13.526, win_z: -4.675, share_delta_pp: -3.436, share_z: -3.996, source: "2026-08-24-fog-honest-family-direct-6p-allseats-414-seats.json" }) },
    GeneVerdict { tag: "fog-honest-2", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-1574), wins_prior_10k: None, win_diff_pp: Some(-21.296296), posterior_pp: Some(-1064.814815), posterior_se_pp: Some(74.175396), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 207, win_delta_pp: -21.296, win_z: -14.355, share_delta_pp: -6.762, share_z: -11.482, source: "2026-08-24-fog-honest-family-direct-6p-allseats-414-seats.json" }) },
    GeneVerdict { tag: "founder-temple", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(7), wins_prior_10k: Some(14), win_diff_pp: Some(0.300873), posterior_pp: Some(19.135575), posterior_se_pp: Some(11.49542), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.144, win_z: 0.455, share_delta_pp: -0.092, share_z: -1.566, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "garrison-under-fire", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-27), wins_prior_10k: Some(-5), win_diff_pp: Some(0.129232), posterior_pp: Some(11.896268), posterior_se_pp: Some(20.146449), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.542, win_z: -1.75, share_delta_pp: -0.046, share_z: -0.791, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "governor-every-lane", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-234), wins_prior_10k: Some(13), win_diff_pp: Some(-1.771919), posterior_pp: Some(-71.558363), posterior_se_pp: Some(63.511046), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -4.682, win_z: -15.12, share_delta_pp: -1.998, share_z: -35.456, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "governor-expansion-lane", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-28), wins_prior_10k: Some(-30), win_diff_pp: Some(-0.572871), posterior_pp: Some(-28.617241), posterior_se_pp: Some(11.960276), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.55, win_z: -1.757, share_delta_pp: -0.233, share_z: -4.026, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "governor-victory-lanes", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-239), wins_prior_10k: Some(-237), win_diff_pp: Some(-2.522547), posterior_pp: Some(-142.32886), posterior_se_pp: Some(105.929107), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 3600, win_delta_pp: -4.778, win_z: -6.112, share_delta_pp: -2.732, share_z: -23.757, source: "2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json" }) },
    GeneVerdict { tag: "great-person-housing", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(94), wins_prior_10k: Some(78), win_diff_pp: Some(1.738033), posterior_pp: Some(87.016416), posterior_se_pp: Some(12.023346), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 1.871, win_z: 5.93, share_delta_pp: 0.631, share_z: 10.827, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "guru-heals-the-corps", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-29), wins_prior_10k: None, win_diff_pp: Some(-0.584201), posterior_pp: Some(-29.210058), posterior_se_pp: Some(15.975762), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.584, win_z: -1.828, share_delta_pp: -0.026, share_z: -0.455, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "holy-lane-parity", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(19), wins_prior_10k: Some(99), win_diff_pp: Some(0.779469), posterior_pp: Some(38.032015), posterior_se_pp: Some(24.120908), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.389, win_z: 1.228, share_delta_pp: -0.012, share_z: -0.214, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "holy-site-where-the-threat-is", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-19), wins_prior_10k: None, win_diff_pp: Some(-0.372534), posterior_pp: Some(-18.626704), posterior_se_pp: Some(15.426134), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.373, win_z: -1.207, share_delta_pp: -0.286, share_z: -4.964, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "home-defense", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-18), wins_prior_10k: Some(-15), win_diff_pp: Some(-0.295799), posterior_pp: Some(-14.694182), posterior_se_pp: Some(9.240066), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.364, win_z: -1.156, share_delta_pp: 0.055, share_z: 0.927, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "housing-districts", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-37), wins_prior_10k: Some(5), win_diff_pp: Some(-0.341748), posterior_pp: Some(-17.09006), posterior_se_pp: Some(9.449019), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.737, win_z: -2.346, share_delta_pp: -0.145, share_z: -2.469, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "housing-research", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-17), wins_prior_10k: Some(13), win_diff_pp: Some(0.017231), posterior_pp: Some(2.038047), posterior_se_pp: Some(14.437946), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.347, win_z: -1.103, share_delta_pp: -0.095, share_z: -1.628, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "idle-faith-patronage", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(39), wins_prior_10k: Some(36), win_diff_pp: Some(0.711925), posterior_pp: Some(29.353784), posterior_se_pp: Some(7.760813), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.77, win_z: 2.496, share_delta_pp: 0.221, share_z: 3.829, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "inquisition-on-threat", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(3), wins_prior_10k: Some(2), win_diff_pp: Some(0.131367), posterior_pp: Some(9.374131), posterior_se_pp: Some(10.575053), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.051, win_z: 0.161, share_delta_pp: -0.041, share_z: -0.697, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "joint-tactics", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(3), wins_prior_10k: Some(-4), win_diff_pp: Some(-0.104302), posterior_pp: Some(-5.145492), posterior_se_pp: Some(11.41702), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 17574, win_delta_pp: 0.068, win_z: 0.186, share_delta_pp: 0.248, share_z: 3.838, source: "2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json" }) },
    GeneVerdict { tag: "lane-congress-ballot", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(8), wins_prior_10k: None, win_diff_pp: Some(0.160867), posterior_pp: Some(8.043349), posterior_se_pp: Some(15.66483), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.161, win_z: 0.513, share_delta_pp: -0.012, share_z: -0.214, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "lane-congress-favor", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-14), wins_prior_10k: None, win_diff_pp: Some(-0.279401), posterior_pp: Some(-13.970028), posterior_se_pp: Some(15.54891), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.279, win_z: -0.898, share_delta_pp: -0.093, share_z: -1.607, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "lane-culture-spending", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(8), wins_prior_10k: None, win_diff_pp: Some(0.160867), posterior_pp: Some(8.043349), posterior_se_pp: Some(15.699122), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.161, win_z: 0.512, share_delta_pp: -0.073, share_z: -1.258, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "lane-great-people", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-3), wins_prior_10k: None, win_diff_pp: Some(-0.059267), posterior_pp: Some(-2.963339), posterior_se_pp: Some(15.538901), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.059, win_z: -0.191, share_delta_pp: -0.039, share_z: -0.661, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "lane-policy-deck", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(0), wins_prior_10k: None, win_diff_pp: Some(0.0), posterior_pp: Some(0.0), posterior_se_pp: Some(15.636721), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.0, win_z: 0.0, share_delta_pp: -0.043, share_z: -0.745, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "lane-space-race", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(11), wins_prior_10k: None, win_diff_pp: Some(0.211667), posterior_pp: Some(10.583355), posterior_se_pp: Some(15.823845), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.212, win_z: 0.669, share_delta_pp: -0.013, share_z: -0.225, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "loyalty-rate-alarm", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(38), wins_prior_10k: Some(40), win_diff_pp: Some(0.904627), posterior_pp: Some(45.253303), posterior_se_pp: Some(9.162528), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.762, win_z: 2.422, share_delta_pp: 0.288, share_z: 5.025, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "naval-recon", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-20), wins_prior_10k: Some(16), win_diff_pp: Some(-0.155079), posterior_pp: Some(-7.76717), posterior_se_pp: Some(9.173696), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.406, win_z: -1.307, share_delta_pp: 0.029, share_z: 0.506, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "one-launch-pad", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-11), wins_prior_10k: Some(15), win_diff_pp: Some(0.198156), posterior_pp: Some(9.767815), posterior_se_pp: Some(9.140807), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.212, win_z: -0.677, share_delta_pp: 0.013, share_z: 0.223, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "one-shot-recovery", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: None, win_diff_pp: Some(-0.169334), posterior_pp: Some(-8.466684), posterior_se_pp: Some(15.521072), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.169, win_z: -0.545, share_delta_pp: 0.017, share_z: 0.288, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "opportunistic-war", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(49), wins_prior_10k: Some(23), win_diff_pp: Some(0.757355), posterior_pp: Some(37.851865), posterior_se_pp: Some(13.032549), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.982, win_z: 3.135, share_delta_pp: 0.386, share_z: 6.646, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "peacetime-deterrence", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(21), wins_prior_10k: Some(1), win_diff_pp: Some(0.313029), posterior_pp: Some(15.675434), posterior_se_pp: Some(9.311329), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.415, win_z: 1.323, share_delta_pp: 0.009, share_z: 0.149, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "power-the-laboratory", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: None, win_diff_pp: Some(-0.169334), posterior_pp: Some(-8.466684), posterior_se_pp: Some(15.381856), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.169, win_z: -0.55, share_delta_pp: -0.044, share_z: -0.765, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "priced-tile-purchase", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-11), wins_prior_10k: Some(-31), win_diff_pp: Some(-0.393242), posterior_pp: Some(-19.626455), posterior_se_pp: Some(11.723375), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.229, win_z: -0.739, share_delta_pp: 0.019, share_z: 0.337, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "raid-pillage-prizes", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(53), wins_prior_10k: Some(30), win_diff_pp: Some(0.859307), posterior_pp: Some(43.013614), posterior_se_pp: Some(11.744172), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 1.058, win_z: 3.418, share_delta_pp: 0.356, share_z: 6.11, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "recon-replacement", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(30), wins_prior_10k: Some(48), win_diff_pp: Some(0.907498), posterior_pp: Some(46.179037), posterior_se_pp: Some(11.827045), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.593, win_z: 1.896, share_delta_pp: 0.181, share_z: 3.111, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "recorded-tactical-step", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(18), wins_prior_10k: Some(30), win_diff_pp: Some(0.313029), posterior_pp: Some(15.729503), posterior_se_pp: Some(9.237274), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.356, win_z: 1.144, share_delta_pp: 0.044, share_z: 0.768, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "relief-targets-the-siege", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(6), wins_prior_10k: Some(14), win_diff_pp: Some(0.09477), posterior_pp: Some(4.945), posterior_se_pp: Some(9.244581), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.119, win_z: 0.38, share_delta_pp: 0.028, share_z: 0.486, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "religion-sues-peace", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-18), wins_prior_10k: Some(6), win_diff_pp: Some(0.129232), posterior_pp: Some(7.907618), posterior_se_pp: Some(11.267787), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.356, win_z: -1.143, share_delta_pp: 0.034, share_z: 0.579, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "religious-defence-scales", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: None, win_diff_pp: Some(-0.169334), posterior_pp: Some(-8.466684), posterior_se_pp: Some(15.761741), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.169, win_z: -0.537, share_delta_pp: -0.059, share_z: -0.995, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "religious-units-heal-first", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(12), wins_prior_10k: None, win_diff_pp: Some(0.237067), posterior_pp: Some(11.853357), posterior_se_pp: Some(15.381288), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.237, win_z: 0.771, share_delta_pp: 0.102, share_z: 1.735, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "research-floor-holds", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-19), wins_prior_10k: None, win_diff_pp: Some(-0.372534), posterior_pp: Some(-18.626704), posterior_se_pp: Some(15.691127), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.373, win_z: -1.187, share_delta_pp: -0.258, share_z: -4.415, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "research-grants-first", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-20), wins_prior_10k: None, win_diff_pp: Some(-0.406401), posterior_pp: Some(-20.320041), posterior_se_pp: Some(15.413966), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.406, win_z: -1.318, share_delta_pp: -0.035, share_z: -0.606, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "research-tier-premium", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-6), wins_prior_10k: None, win_diff_pp: Some(-0.118534), posterior_pp: Some(-5.926679), posterior_se_pp: Some(15.624967), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.119, win_z: -0.379, share_delta_pp: -0.104, share_z: -1.8, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "science-multiplier-payoff", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: None, win_diff_pp: Some(-0.1524), posterior_pp: Some(-7.620015), posterior_se_pp: Some(15.451733), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.152, win_z: -0.493, share_delta_pp: -0.008, share_z: -0.131, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "science-payback-horizon", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-11), wins_prior_10k: None, win_diff_pp: Some(-0.211667), posterior_pp: Some(-10.583355), posterior_se_pp: Some(15.503408), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.212, win_z: -0.683, share_delta_pp: -0.161, share_z: -2.79, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "score-horizon", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(18), wins_prior_10k: Some(24), win_diff_pp: Some(0.364722), posterior_pp: Some(18.098408), posterior_se_pp: Some(9.247158), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.356, win_z: 1.14, share_delta_pp: 0.132, share_z: 2.28, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "settle-sooner", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(28), wins_prior_10k: Some(41), win_diff_pp: Some(0.669968), posterior_pp: Some(33.494158), posterior_se_pp: Some(11.821844), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.559, win_z: 1.79, share_delta_pp: 0.136, share_z: 2.351, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "settler-guard-holds", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: Some(-3), win_diff_pp: Some(-0.03159), posterior_pp: Some(-1.583607), posterior_se_pp: Some(9.131295), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.161, win_z: -0.515, share_delta_pp: -0.046, share_z: -0.782, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "settler-site-agreement", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-23), wins_prior_10k: Some(24), win_diff_pp: Some(-0.014359), posterior_pp: Some(0.337885), posterior_se_pp: Some(13.367959), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.457, win_z: -1.472, share_delta_pp: -0.026, share_z: -0.454, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "settler-target-hysteresis", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-18), wins_prior_10k: Some(15), win_diff_pp: Some(-0.077539), posterior_pp: Some(-3.605405), posterior_se_pp: Some(9.116102), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.364, win_z: -1.156, share_delta_pp: -0.07, share_z: -1.206, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "settler-threat-detour", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(18), wins_prior_10k: Some(50), win_diff_pp: Some(0.635984), posterior_pp: Some(32.616627), posterior_se_pp: Some(15.862447), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.364, win_z: 1.159, share_delta_pp: 0.099, share_z: 1.74, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "siege-commitment", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(6), wins_prior_10k: Some(1), win_diff_pp: Some(-0.100514), posterior_pp: Some(-5.000474), posterior_se_pp: Some(9.893066), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.119, win_z: 0.379, share_delta_pp: -0.005, share_z: -0.081, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "siege-is-progress", verdict: Verdict::Helps, default_on: false, wins_last_10k: Some(-6), wins_prior_10k: Some(-16), win_diff_pp: Some(-0.307286), posterior_pp: Some(-16.866239), posterior_se_pp: Some(14.967804), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.119, win_z: -0.38, share_delta_pp: 0.191, share_z: 3.263, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "siege-tracks-wall", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(8), wins_prior_10k: Some(-3), win_diff_pp: Some(0.318773), posterior_pp: Some(16.628943), posterior_se_pp: Some(10.819428), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.152, win_z: 0.492, share_delta_pp: -0.021, share_z: -0.354, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "slot-kind-tiebreak", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-1), wins_prior_10k: Some(-12), win_diff_pp: Some(0.120617), posterior_pp: Some(5.778034), posterior_se_pp: Some(9.162766), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.017, win_z: -0.054, share_delta_pp: -0.008, share_z: -0.147, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "spread-campaign-persists", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-16), wins_prior_10k: None, win_diff_pp: Some(-0.313267), posterior_pp: Some(-15.663365), posterior_se_pp: Some(15.743271), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.313, win_z: -0.995, share_delta_pp: -0.044, share_z: -0.748, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "step-and-reassess", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-27), wins_prior_10k: Some(-860), win_diff_pp: Some(-0.4875), posterior_pp: Some(-18.224413), posterior_se_pp: Some(17.30875), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 15000, win_delta_pp: -0.533, win_z: -1.352, share_delta_pp: -0.15, share_z: -2.315, source: "2026-08-21-p7-native-6p-allseats-15000-pairs.json" }) },
    GeneVerdict { tag: "stranded-settler-discount", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(-8), wins_prior_10k: Some(13), win_diff_pp: Some(0.117745), posterior_pp: Some(5.809664), posterior_se_pp: Some(9.227437), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.169, win_z: -0.54, share_delta_pp: -0.032, share_z: -0.548, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "strategic-wonders", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(11), wins_prior_10k: Some(-5), win_diff_pp: Some(0.221131), posterior_pp: Some(10.938151), posterior_se_pp: Some(9.196383), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.229, win_z: 0.73, share_delta_pp: 0.066, share_z: 1.139, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "strike-opening", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(4), wins_prior_10k: Some(17), win_diff_pp: Some(0.278568), posterior_pp: Some(13.802139), posterior_se_pp: Some(9.128001), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.085, win_z: 0.273, share_delta_pp: 0.066, share_z: 1.125, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "theology-for-founders", verdict: Verdict::Unresolved, default_on: false, wins_last_10k: Some(22), wins_prior_10k: Some(-16), win_diff_pp: Some(0.093228), posterior_pp: Some(2.813855), posterior_se_pp: Some(12.606021), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.449, win_z: 1.435, share_delta_pp: -0.002, share_z: -0.032, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "war-economy", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(118), wins_prior_10k: Some(38), win_diff_pp: Some(0.281439), posterior_pp: Some(-6.622122), posterior_se_pp: Some(62.967136), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 2.354, win_z: 7.498, share_delta_pp: 1.426, share_z: 24.582, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "war-patience", verdict: Verdict::Hurts, default_on: false, wins_last_10k: Some(-28), wins_prior_10k: Some(-19), win_diff_pp: Some(-0.186669), posterior_pp: Some(-8.921559), posterior_se_pp: Some(10.894315), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: -0.55, win_z: -1.769, share_delta_pp: -0.205, share_z: -3.455, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "war-reinforcement", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(34), wins_prior_10k: Some(3), win_diff_pp: Some(0.410672), posterior_pp: Some(19.988646), posterior_se_pp: Some(12.174707), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.677, win_z: 2.169, share_delta_pp: 0.154, share_z: 2.636, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "whole-turn-backtrack-guard", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(6), wins_prior_10k: Some(18), win_diff_pp: Some(0.379082), posterior_pp: Some(18.703707), posterior_se_pp: Some(9.251633), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.127, win_z: 0.401, share_delta_pp: -0.101, share_z: -1.718, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "wide-map-capacity", verdict: Verdict::Helps, default_on: true, wins_last_10k: Some(91), wins_prior_10k: Some(35), win_diff_pp: Some(1.053962), posterior_pp: Some(49.209132), posterior_se_pp: Some(16.035688), family_wise: true, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 1.812, win_z: 5.836, share_delta_pp: 0.443, share_z: 7.609, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
    GeneVerdict { tag: "wonder-ring-settle-value", verdict: Verdict::Unresolved, default_on: true, wins_last_10k: Some(22), wins_prior_10k: Some(7), win_diff_pp: Some(0.212515), posterior_pp: Some(10.599093), posterior_se_pp: Some(9.27529), family_wise: false, family_runner_up: false, screen: Some(Measure { pairs: 23622, win_delta_pp: 0.432, win_z: 1.366, share_delta_pp: -0.03, share_z: -0.523, source: "2026-08-22-standard-10k-6p-allseats-23622-pairs.json" }) },
];
// ═══ END GENERATED ═══
