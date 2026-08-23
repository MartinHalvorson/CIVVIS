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

use crate::ai::{AdvancedAi, Ai, BasicAi, RandomAi};
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

pub const LIVE_BRIDGE_TREATMENTS: &[&str] = &[
    "joint-tactics",
    "joint-reach-lines",
    "live-trader-route",
    "live-religious-purchase",
    "home-defense",
    "recorded-tactical-step",
    "live-motion-turn-accounting",
    "whole-turn-backtrack-guard",
    "step-and-reassess",
    "strike-opening",
    "bounded-recovery",
    "army-target-weighs-enemy",
    "peacetime-deterrence",
    "siege-tracks-wall",
    "blind-objective-strength",
    "solvent-faith-army",
    "loyalty-rate-alarm",
    "district-coverage",
    "slot-kind-tiebreak",
    "come-ashore",
    "relief-targets-the-siege",
    "blind-objective-units",
    "housing-districts",
    "housing-research",
    "war-economy",
    "war-reinforcement",
    "war-patience",
    "endgame-war-runway",
    "wide-map-capacity",
    "garrison-under-fire",
    "escort-unstick",
    "live-formationless-settler-shadow",
    "religion-sues-peace",
    "recon-replacement",
    "stranded-settler-discount",
    "siege-commitment",
    "wonder-ring-settle-value",
    "amenity-project-preemption",
    "amenity-district-path",
    "governor-every-lane",
    "live-wonder-race",
    "expansion-before-prophet",
    "no-elective-war",
    "fog-land-capacity",
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
    "barbarian-hunt",
    "barbarian-bargain",
    "barbarian-ranged-answer",
    "camp-party",
    "buildings-before-projects",
    "deny-while-targeted",
    "stock-denial-lead-time",
    "projected-stock-denial",
    "parallel-settlers",
    "host-settler-pop",
    "explore-dead-targets",
    "explore-commit",
    "bank-envoys",
    "land-grab",
    "siege-is-progress",
    "spy-mission-patience",
    "settler-site-agreement",
    "civilian-rescue",
    "district-building-chain",
    "settler-guard-holds",
    "expansion-pantheon",
    "expansion-hall",
    "opening-settler-waits",
];

/// The deployment-profile treatments that stay out of the native bundle, as
/// tags. Some encode a rule of Firaxis' game, while others price host-only
/// conditions or are already present in the native production baseline. See
/// `AdvancedAi::enable_engine_repairs`.
pub const FIRAXIS_ONLY_TREATMENTS: &[&str] = &[
    "joint-tactics",
    "joint-reach-lines",
    // The brain's half of the mid-turn replan frame: a walk cut at its first
    // unrevealed hex because the host executes one coalesced walk per unit.
    // No native meaning; its native stand-in measured harmful and is gone.
    "step-and-reassess",
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
    // Same steal record, same clock: the projection only widens the live
    // seat's lead-time bar; native lanes keep the raw reading.
    "projected-stock-denial",
    // These four react to host movement or host production semantics; native
    // CIVVIS has neither distinction, so the repair bundle must not imply
    // they are native engine changes.
    "parallel-settlers",
    "host-settler-pop",
    "explore-dead-targets",
    // The host invokes the bridge again after some accepted orders. That is a
    // replan frame, not a second game turn for persistent unit motion.
    "live-motion-turn-accounting",
    "bank-envoys",
    // Replaces the host's broken Settler formation channel with ordinary
    // unit movement; native CIVVIS formations have no corresponding defect.
    "live-formationless-settler-shadow",
    // Production Advanced already carries committed exploration. It remains
    // a live treatment so the deployment bundle and its ablation registry are
    // complete, not because `advanced_synergy` needs to turn it on again.
    "explore-commit",
    // Prices the Settler seat's uncontested land against Firaxis rivals who
    // out-settle the seat late; the league cadence was bred against CIVVIS
    // rivals who contest the ground.
    "land-grab",
    // Reacts to the host export's blindness to a running Spy operation;
    // native `do_spy_mission` sets `spy.mission` and legality already
    // debounces, so the repair cannot fire there.
    "spy-mission-patience",
    // Prices the Settler seat's pantheon against a host that grants a Settler
    // for it, bought with the one Faith card the live capital has; the
    // native lanes keep the shipped prefix and the bred policy weights.
    "expansion-pantheon",
    // Prices the Settler seat's plaza building for the land grab; the native
    // lanes keep their bred building prices.
    "expansion-hall",
    // Holds the opening book's Settler for the host's population floor; the
    // native book keeps its bred slot.
    "opening-settler-waits",
];

/// The military half of the native repair bundle: force assembly, marching,
/// siege, threat reading, and the war/peace decision.
pub const ENGINE_REPAIR_WAR_TREATMENTS: &[&str] = &[
    "war-reinforcement",
    "come-ashore",
    "recorded-tactical-step",
    "whole-turn-backtrack-guard",
    "blind-objective-strength",
    "blind-objective-units",
    "relief-targets-the-siege",
    "army-target-weighs-enemy",
    "peacetime-deterrence",
    "war-economy",
    "bounded-recovery",
    "siege-tracks-wall",
    "siege-commitment",
    "war-patience",
    "siege-is-progress",
    "endgame-war-runway",
    "home-defense",
    "garrison-under-fire",
    "strike-opening",
    "recon-replacement",
    "barbarian-scouts-are-scouts",
    "barbarian-hunt",
    "barbarian-bargain",
    "barbarian-ranged-answer",
    "civilian-rescue",
    "naval-recon",
    "camp-party",
    "religion-sues-peace",
];

/// The economic half: settlement, growth, districts, and the policy deck.
pub const ENGINE_REPAIR_ECONOMY_TREATMENTS: &[&str] = &[
    "escort-unstick",
    "buildings-before-projects",
    "wonder-ring-settle-value",
    "settler-site-agreement",
    "settler-guard-holds",
    "stranded-settler-discount",
    "wide-map-capacity",
    "housing-districts",
    "amenity-project-preemption",
    "amenity-district-path",
    "governor-every-lane",
    "housing-research",
    "district-coverage",
    "slot-kind-tiebreak",
    "loyalty-rate-alarm",
    "score-horizon",
    "one-launch-pad",
    "settler-target-hysteresis",
    // ★★★★ THE RESEARCH ECONOMY SHIPS NATIVELY AND THE CULTURE ECONOMY DOES
    // NOT, AND THE BOARD SHOWS IT. `research_economy` is set in
    // `promoted_policy_envoy`, so every native seat already pays
    // `RESEARCH_CAMPUS_COVERAGE` for a city with no Campus and
    // `RESEARCH_BUILDING_DEBT` for a Campus with no Library. Its two
    // counterparts on the other tree were classed host-only on the reasoning
    // that "the native lanes keep their bred district coverage" — an
    // assumption about the bred `Weights`, never a measurement.
    //
    // A six-seat census of the deployed genome at the deployment shape
    // (74x46, nine city-states, 250 turns Online, seeds 90001000..4) puts the
    // two trees side by side and the asymmetry is the live seat's, exactly:
    // Campus in 82% of cities, Library 81%, University 74%; Theater Square
    // **37%**, Amphitheater **35%**, Museum 29%. End-of-game culture ran
    // 128.8 a turn against 153.7 of science, and civics 36.8 against 48.5
    // techs. `culture_coverage`'s own writeup measured 82/27 and 72/21 on the
    // live seat and named a game lost at t197 to a rival's tourism; the
    // native seats lose the same way.
    //
    // These three are the instrument for that question, so they belong where
    // a screen can price them. Screenable means OFF at deployment until a
    // screen says otherwise (`apply_gene_ledger`), so this changes what can
    // be measured and not what ships.
    "culture-building-debt",
    "culture-coverage",
    "district-building-chain",
];

/// Every live-bridge repair that fixes a CIVVIS engine defect, as evaluator
/// tags — `LIVE_BRIDGE_TREATMENTS` minus `FIRAXIS_ONLY_TREATMENTS`, and the
/// union of the two halves above. `engine_repair_tags_partition_the_bridge`
/// fails if any of those three relationships stops holding.
pub const ENGINE_REPAIR_TREATMENTS: &[&str] = &[
    "war-reinforcement",
    "come-ashore",
    "recorded-tactical-step",
    "whole-turn-backtrack-guard",
    "blind-objective-strength",
    "blind-objective-units",
    "relief-targets-the-siege",
    "army-target-weighs-enemy",
    "peacetime-deterrence",
    "war-economy",
    "bounded-recovery",
    "siege-tracks-wall",
    "siege-commitment",
    "war-patience",
    "siege-is-progress",
    "endgame-war-runway",
    "home-defense",
    "garrison-under-fire",
    "strike-opening",
    "recon-replacement",
    "barbarian-scouts-are-scouts",
    "barbarian-hunt",
    "barbarian-bargain",
    "barbarian-ranged-answer",
    "civilian-rescue",
    "naval-recon",
    "camp-party",
    "religion-sues-peace",
    "escort-unstick",
    "buildings-before-projects",
    "wonder-ring-settle-value",
    "settler-site-agreement",
    "settler-guard-holds",
    "stranded-settler-discount",
    "wide-map-capacity",
    "housing-districts",
    "amenity-project-preemption",
    "amenity-district-path",
    "governor-every-lane",
    "housing-research",
    "district-coverage",
    "slot-kind-tiebreak",
    "loyalty-rate-alarm",
    "score-horizon",
    "one-launch-pad",
    "settler-target-hysteresis",
    // The culture economy's two coverage terms and the chain that fills
    // every specialty district: see the census in the economy half above.
    "culture-building-debt",
    "culture-coverage",
    "district-building-chain",
];

/// On-disk schema for the shared player/leader/civilization rating ledger.
pub const ELO_SCHEMA_VERSION: u32 = 3;
/// Version of the game/rating contract, independent of the JSON shape. Bump
/// this when rules, default setup, or scoring semantics change enough that an
/// Elo point no longer measures the same experiment.
///
/// **v12 (2026-08-18) — Gathering Storm district-production rows now execute
/// for their actual owners.** All city-states build Harbors 500% faster and
/// their specialty district 500% faster; Japan builds Encampments, Holy Sites,
/// and Theater Squares 100% faster; and the Netherlands builds Dams 50% faster.
/// CIVVIS had dropped the city-state rows and paid Japan and the Netherlands an
/// invented +1 Production in every city instead. City-state development changes
/// the native game's city queues and map pressure, so the frozen anchor moves
/// from 18,503 actions and `0x70c7_8503_3e29_380f` to 18,572 and
/// `0x3bda_c2f2_b84d_30fc` across its five profiles. This is a rules correction,
/// not a compatibility re-pin: v11 and v12 rows are not comparable.
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
///
/// **v13 (2026-08-18) — WITHDRAWN. Four Founder beliefs were changed to the
/// base game's forms.** #2049 read `TITHE_GOLD_FOLLOWER`,
/// `WORLD_CHURCH_CULTURE_FOREIGN_FOLLOWER`, `PILGRIMAGE_FAITH_FOREIGN_CITY`
/// and `CHURCH_PROPERTY_GOLD_CITY` out of the compiled gameplay cache and
/// rewrote `beliefs.json` to match. The cache held **Vanilla**.
/// `Expansion2_RemoveData.xml` deletes all four of those modifiers, and
/// Gathering Storm — the ruleset CIVVIS models — replaces them with
/// `TITHE_GOLD_CITY` (+3 Gold per following city),
/// `WORLD_CHURCH_CULTURE_FOLLOWER` (+1 Culture per 4 followers) and
/// `PILGRIMAGE_FAITH_CITY` (+2 Faith per following city), and deletes
/// `BELIEF_CHURCH_PROPERTY` outright. `beliefs.json` already had exactly
/// those. The "fix" replaced correct expansion values with base-game ones.
///
/// **v14 (2026-08-18) — v13 reverted; v14 plays the v12 ruleset.** #2050 put
/// the data back, and the frozen anchor returned to 18,572 decisions and
/// `0x3bda_c2f2_b84d_30fc` and the ruleset fingerprint to
/// `fnv1a64:585ff2655ffd3a6d`, all three unchanged from before v13 — which is
/// the proof the revert is exact rather than approximate.
///
/// ⚠ The version still advances rather than returning to 12, because rows
/// written under v13 were played on the base game's beliefs and must stay
/// identifiable. **v14 rows are comparable to v12 rows; v13 rows are
/// comparable to neither.**
/// **v15 (2026-08-18) — a Builder can reach a pillaged tile.**
/// `has_builder_work` — the gate that decides whether to *train* a Builder —
/// counts a pillaged improvement anywhere in the empire. The Builder's own
/// target sweep tested only `valid_improvements`, which a pillaged-but-improved
/// tile fails, and handled repair for the tile it already stood on and nowhere
/// else. Two definitions of "work" that disagreed, with the wider one spending
/// the production and the narrower one choosing the destination: the empire
/// trained Builders for work its Builders could not walk to, and a razed farm
/// earned nothing until one wandered onto it.
///
/// Counted over three 250-turn six-player games before the repair: Builders
/// reached a decision with no target 3,704 times, and `has_builder_work` said
/// there was work on **508** of them.
///
/// This reaches every seat, the frozen anchor included: 18,572 decisions became
/// 18,586 across the five anchor profiles. ⚠ Its strength effect is not
/// measured — the justification is that two definitions of the same thing
/// disagreed, not a demonstrated gain. Rows before and after v15 are not
/// comparable in any game where an improvement was pillaged.
///
/// **v16 (2026-08-18) — Barbarian Scouts now report a sighted city, and each
/// reported outpost raises one finite, difficulty-shaped raiding party.** The
/// old phase let unreported camps add globally capped, unassigned units, so a
/// distant camp could consume the force a successful Scout had earned. The
/// corrected world rule keeps the Scout home while its report is active,
/// retains each raider's source camp, and ends the alert after the party has
/// formed.
///
/// This is a shared native-world rule with no controller gate: it changes what
/// every participant faces before and during their turns. The frozen anchor
/// therefore moved from v15's 18,586 decisions and `0x2076_c0d8_5213_9238` to
/// v16's 17,494 and `0x6cf9_b1fa_a854_dcd6` across its five profiles.
///
/// **v17 (2026-08-19) — the Great Person roster reaches the Information era.**
/// It fills the roster out to the Information era. It held 29 of
/// Gathering Storm's 213 individuals and stopped at the Atomic era, so from the
/// midgame every class ran out and `Game::unused_great_person_faith` paid the
/// whole Campus, Theatre Square and Harbour yield out as Faith --
/// **26.6% of all non-prophet Great Person points**, measured over eight
/// 6-player 200-turn games. This too is a shared world rule with no controller
/// gate: the market every participant recruits from is the same one. The
/// anchor moves again, to **17,482 decisions and `0x8162_c919_b83c_40df`**.
///
/// **v18 (2026-08-21) — a Barbarian Scout reports the WALKERS it sees, not
/// only cities.** v16 gave each reported outpost a finite raiding party, but
/// the report itself still accepted a city alone — so a camp's whole raid
/// throughput was one Scout's round trip to a settlement, and an empire's
/// Settlers, which is what a Civilization VI barbarian actually takes, could
/// not start a raid at all.
///
/// MEASURED, `ai_eval live live_without_camp_reach`, 12 pairs / 72 seat-games
/// an arm at 6p/150t/online on identical seeds: **0.22 civilians lost to
/// barbarians per game**, against the live Civilization VI seat losing **4 of
/// the 8 Settlers that ever walked, in 104 turns** (run
/// civvis-20260821T130446Z) at a matching city count.
///
/// ⚠ CORRECTED 2026-08-21: an earlier reading of this run said 8 of 12. The
/// host emits `unit_lost` when a Settler FOUNDS a city — the operation consumes
/// the unit, so `CivvisLedger.onUnitRemoved` reports it exactly like a capture.
/// Counting those events naively doubles the loss. Subtract every `unit_lost`
/// whose `unit` id also has a `found` event. The gap this version answers is
/// smaller than first stated and still an order of magnitude. Not an exposure
/// difference — the opponent. Every arm this project has priced on settler
/// safety was measured against a barbarian that did not hunt settlers, which
/// is why the whole family reads neutral-to-harmful in `docs/gene_ledger.json`
/// at 13,446 pairs apiece and ships off.
///
/// With the sighting extended, and with the raider pursuit of #2227 that this
/// version follows, the same measurement reads **0.61**.
///
/// Like v16 this is a shared native-world rule with no controller gate — the
/// Scout phase runs in `Game::barbarian_phase` and every participant faces the
/// same camps. The anchor therefore moves again, to **18,596 decisions and
/// `0xf78a_2b10_c0e3_5945`**.
///
/// Each of these is a rules correction, not a compatibility re-pin: rows from
/// v15, v16, v17 and v18 are not comparable.
pub const ELO_PROTOCOL_VERSION: u32 = 18;
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


/// Directory `builtin_ai` resolves trained artifacts from.
pub const ARTIFACT_DIR: &str = "evolved";
/// Evolved strategy genome written by `civvis evolve`.
pub const CHAMPION_FILE: &str = "best.json";
/// Distilled scalar value net written by `tools/train_valuenet.py`.
pub const VALUENET_FILE: &str = "valuenet.json";

/// Build a named built-in agent (`BUILTIN_AIS`), resolving trained artifacts
/// from `ARTIFACT_DIR` and falling back to the scripted controller when one is
/// missing — `evolved` without a champion is `advanced`, `strategic` without a
/// value net is the score-only search. A name that is not a built-in plays as
/// `basic`, as it always has at game start.
///
/// ⚠ The 228 evaluator arms that used to be constructed here — `advanced_<x>`,
/// `advanced_without_<x>`, `live_without_<x>`, `live_target_<lane>` — are gone
/// (2026-08-23). One flag per arm, priced against a fixed background, was the
/// instrument the gene screen replaced: `gene_screen` prices every gene from
/// every seat of a random-genome batch, and the live seat withholds a shipped
/// behaviour through `civvis_orders --without`. See `docs/GENE_SCREEN.md`.
pub fn builtin_ai(name: &str, seed: u64) -> Box<dyn Ai> {
    let champion = || crate::evolve::load_champion(ARTIFACT_DIR);
    match name {
        "advanced" => Box::new(AdvancedAi::new()),
        "advanced_v1" => Box::new(AdvancedAi::legacy()),
        "advanced_evolved" | "evolved" => Box::new(
            champion()
                .map(AdvancedAi::with_weights)
                .unwrap_or_default(),
        ),
        "basic" => Box::new(BasicAi::new()),
        "random" => Box::new(RandomAi::new(seed)),
        "strategic" => {
            let weights = champion().unwrap_or_default();
            if crate::valuenet::ValueNet::load_width(ARTIFACT_DIR, crate::evolve::FEATURE_WIDTH)
                .is_some()
            {
                Box::new(crate::strategic::StrategicAi::with_weights(weights))
            } else {
                Box::new(crate::strategic::StrategicAi::score_only_with_weights(weights))
            }
        }
        "strategic_deep" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(champion().unwrap_or_default());
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        _ => Box::new(BasicAi::new()),
    }
}

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

/// What a built-in name actually loads and plays as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProvenance {
    /// The name the caller asked for.
    pub requested: String,
    /// Every artifact the name reads, in the order it reads them.
    pub artifacts: Vec<ArtifactStatus>,
    /// Canonical identity of the agent that actually plays. Equals
    /// `requested` unless a definitional artifact is missing.
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
    /// `evolved: plays as advanced (missing best.json)`.
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

/// What `builtin_ai(name, _)` will actually construct from the artifacts
/// under `dir`, so a tournament can refuse to rate a name that would silently
/// play as a different controller.
pub fn builtin_provenance(name: &str, dir: &str) -> AgentProvenance {
    let champion = crate::evolve::load_champion(dir).is_some();
    let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
    let genome = |definitional| ArtifactStatus {
        file: CHAMPION_FILE,
        found: champion,
        definitional,
    };
    let value = |definitional| ArtifactStatus {
        file: VALUENET_FILE,
        found: net,
        definitional,
    };
    let (artifacts, effective): (Vec<ArtifactStatus>, &'static str) = match name {
        // The genome *is* these two names; without it they are the stock
        // scripted agent under a name that claims otherwise, and with it they
        // are one agent under two names.
        "evolved" | "advanced_evolved" => (
            vec![genome(true)],
            if champion { "advanced_evolved" } else { "advanced" },
        ),
        "advanced" => (Vec::new(), "advanced"),
        "advanced_v1" => (Vec::new(), "advanced_v1"),
        "basic" => (Vec::new(), "basic"),
        "random" => (Vec::new(), "random"),
        // The value net *is* `strategic`: without it the name plays the
        // score-only search under a name that claims otherwise.
        "strategic" => (
            vec![genome(false), value(true)],
            if net { "strategic" } else { "strategic_score" },
        ),
        "strategic_deep" => (vec![genome(false), value(false)], "strategic_deep"),
        _ => (Vec::new(), "basic"),
    };
    AgentProvenance {
        requested: name.to_string(),
        artifacts,
        effective,
    }
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
    use super::*;

    /// `AdvancedAi::enable_live_bridge` is the single place the Civilization VI
    /// bridge turns its repairs on; `LIVE_BRIDGE_TREATMENTS` is how the
    /// evaluator names them. Until now the only thing keeping the two in step
    /// was a comment.
    ///
    /// A flag added to the helper and not to the tag list is silent: both sides
    /// compile, every other test passes, and the bundle stamp goes on naming a
    /// set of repairs the deployed seat is no longer
    /// running. #977 shipped exactly that way — `army_target_weighs_the_enemy`
    /// reached the deployment while the tag list still described ten
    /// mechanisms — and it was caught by reading the merge, not by CI.
    #[test]
    fn live_bridge_treatments_name_every_flag_the_helper_sets() {
        // The bundle bodies moved to `ai/advanced/treatment_flags.rs`; the
        // scrape reads the controller's whole text so a further split cannot
        // narrow it to a half that no longer holds them.
        let source = concat!(
            include_str!("ai/advanced.rs"),
            include_str!("ai/advanced/treatment_flags.rs")
        );
        let body = source
            .split("pub fn enable_live_bridge_universe(&mut self) {")
            .nth(1)
            .and_then(|tail| tail.split("\n    }\n").next())
            .expect("enable_live_bridge_universe body");
        let enabled = body.matches("self.enable_").count();
        assert_eq!(
            enabled,
            LIVE_BRIDGE_TREATMENTS.len(),
            "enable_live_bridge sets {enabled} flags but LIVE_BRIDGE_TREATMENTS names {}: \
             add the missing tag, or the stamp claims a bundle the bridge is not running",
            LIVE_BRIDGE_TREATMENTS.len()
        );
    }

    /// ⚠⚠ A NAME MAY NOT BE REGISTERED TWICE, AND THE TWO LISTS MAY NOT OVERLAP.
    ///
    /// `EVAL_ONLY_AIS` exists to keep a control out of the persistent rating
    /// keys, which its own doc comment says in as many words. A name in both
    /// lists is therefore not a harmless duplicate: it is evaluator-only *and*
    /// pooled into tournament ratings at the same time, and the two statements
    /// cannot both be acted on.
    ///
    /// `strategic_deep` was in both for three weeks. `3c543665` — "Add
    /// strategic_deep, and decline to promote it" — put it in `EVAL_ONLY_AIS`,
    /// and an hour later `dc6f661e` — "Promote strategic_deep: pre-registered
    /// PASS at 300 maps" — added it to `BUILTIN_AIS` and did not take the first
    /// entry out. Nothing looked, because nothing was looking.
    /// ⚠⚠ TWO CHANGES CLAIMED THE SAME LEDGER VERSION ON THE SAME EVENING.
    ///
    /// #2079 and #2070 both re-pinned the frozen anchor and both set
    /// `ELO_PROTOCOL_VERSION = 15`, for two independent changes to what
    /// `advanced_v1` plays. `collaboration-policy` caught the *line* collision
    /// because they edit the same lines — but only because they do. Nothing
    /// checked the thing that actually matters: that a version number names one
    /// ruleset.
    ///
    /// The anchor pins are self-guarding, because
    /// `advanced_v1_plays_the_same_game_it_always_did` recomputes them from the
    /// merged tree and a stale value fails. The version is not: a merge that
    /// keeps either side's `15` is green, and two different rulesets then share
    /// a ledger identity — which is the one thing the version exists to
    /// prevent.
    ///
    /// So the changelog above this constant is checked instead. Its entries are
    /// the record of what each version means, and three properties make a
    /// duplicate impossible to land quietly: every version is named once, the
    /// numbering has no gaps, and the newest entry is the version the code
    /// actually reports.
    #[test]
    fn every_ledger_version_is_named_exactly_once_and_the_newest_is_current() {
        let source = include_str!("elo.rs");
        let versions: Vec<u32> = source
            .lines()
            .filter_map(|line| {
                line.strip_prefix("/// **v")?
                    .split_once(' ')
                    .map(|(number, _)| number)
                    .and_then(|number| number.parse().ok())
            })
            .collect();
        // A census that reports nothing is a broken census, not an empty
        // changelog: this constant has carried its history since v5.
        assert!(
            versions.len() >= 10,
            "found only {} version entries; the scrape broke rather than \
             finding an undocumented ledger",
            versions.len()
        );

        let mut seen: BTreeMap<u32, usize> = BTreeMap::new();
        for version in &versions {
            *seen.entry(*version).or_default() += 1;
        }
        let twice: Vec<u32> = seen
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(version, _)| *version)
            .collect();
        assert!(
            twice.is_empty(),
            "these ledger versions are documented more than once, so two rulesets share one \
             identity and rows under them cannot be told apart: {twice:?}"
        );

        let lowest = *seen.keys().next().expect("at least one version");
        let highest = *seen.keys().next_back().expect("at least one version");
        let missing: Vec<u32> = (lowest..=highest)
            .filter(|v| !seen.contains_key(v))
            .collect();
        assert!(
            missing.is_empty(),
            "these ledger versions are between the first and last documented one and are \
             described nowhere, so what changed under them is unrecoverable: {missing:?}"
        );

        assert_eq!(
            highest, ELO_PROTOCOL_VERSION,
            "the newest documented version is v{highest} and the code reports \
             v{ELO_PROTOCOL_VERSION}; a bump without an entry leaves a ruleset nobody can \
             describe, and an entry without a bump leaves rows filed under the old one"
        );
    }

    use std::collections::BTreeSet;
    use std::path::Path;
    use crate::rng::Rng;
    use crate::rules::Rules;
    use crate::setup::{GameMode, MapScript};

    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

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
            // The brain's half of the mid-turn replan frame (a bridge fact).
            "step_and_reassess",
            "live_trader_route_adapter",
            "live_religious_purchase_guard",
            "solvent_faith_army",
            "joint_tactics",
            // A sub-feature of the joint search: it has no effect where the
            // search does not run, and the search is excluded above.
            "joint_reach_lines",
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
            // `culture_building_debt`, `culture_coverage` and
            // `district_building_chain` left this list on 2026-08-21 (PR
            // #2245). `tally_culture` above really is the host's score rule;
            // those three are ordinary district-coverage and building-debt
            // terms with no host semantics in them, and the native board
            // shows the gap they were written for — Campus in 79-82% of
            // cities against Theater Square in 33-37%. They are native
            // repairs now, and the ledger withholds them until a screen
            // prices them.
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
            "projected_stock_denial",
            // Host movement and production semantics, not native engine
            // repairs. `explore_commit` is already set by production
            // Advanced, but stays in the live registry for full parity.
            "parallel_settlers",
            "host_settler_pop",
            "explore_dead_targets",
            "explore_commit",
            "bank_envoys",
            // Replaces only the host formation channel with the ordinary
            // movement shadow; native formations have no corresponding bug.
            "live_formationless_settler_shadow",
            // Firaxis can ask the bridge for same-turn replans after an order;
            // native turns have no duplicate motion snapshots to coalesce.
            "live_motion_turn_accounting",
            // The Settler seat's land, at the Settler seat's pace; the league
            // cadence stays bred.
            "land_grab",
            // Reacts to the host export's blindness to a running Spy
            // operation; native `do_spy_mission` sets `spy.mission` and
            // legality already debounces, so the repair cannot fire there.
            "spy_mission_patience",
            // The Settler seat's pantheon, and the Faith card that buys it,
            // against a host that grants a Settler for it; the native lanes
            // keep the shipped prefix and the bred policy weights.
            "expansion_pantheon",
            // The Settler seat's plaza building for the land grab; the
            // native lanes keep their bred building prices.
            "expansion_hall",
            // The host's Settler floor is what the book slot trips over; the
            // native book keeps its bred behaviour.
            "opening_settler_waits",
        ];
        // The bundle bodies moved to `ai/advanced/treatment_flags.rs`; the
        // scrape reads the controller's whole text so a further split cannot
        // narrow it to a half that no longer holds them.
        let source = concat!(
            include_str!("ai/advanced.rs"),
            include_str!("ai/advanced/treatment_flags.rs")
        );
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
        let parent = calls("enable_engine_repairs_universe");
        assert_eq!(
            parent,
            BTreeSet::from([
                "engine_repairs_economy".to_string(),
                "engine_repairs_war".to_string(),
            ]),
            "enable_engine_repairs_universe must be exactly its two halves"
        );
        // And the deployment helpers are each their universe plus the ledger.
        for (deployed, universe) in [
            ("enable_engine_repairs", "engine_repairs_universe"),
            ("enable_live_bridge", "live_bridge_universe"),
        ] {
            assert_eq!(
                calls(deployed),
                BTreeSet::from([universe.to_string()]),
                "{deployed} must be its universe and the ledger, nothing else"
            );
        }

        let bridge = calls("enable_live_bridge_universe");
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
