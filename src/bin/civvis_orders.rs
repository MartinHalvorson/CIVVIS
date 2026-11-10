//! Ask CIVVIS what to do with a real Civilization VI turn, and say it in orders.
//!
//! # The architecture this completes
//!
//! "CIVVIS is the logic engine and the harness actuates its decisions" was the
//! stated design, and it was blocked on a measured fact: the control mod had no
//! inbound channel. That fact is now false — `DB.Query("ATTACH DATABASE …")` works
//! from the mod's gameplay context, so a decision written to a SQLite file the
//! harness owns reaches a running game on the same turn.
//!
//! So this is the decider. It rebuilds the real board as a CIVVIS `Game`, runs
//! `AdvancedAi` on it, and reads back the actions that agent chose from the action
//! log. Nothing here decides anything itself; the translation layer is deliberately
//! dumb, because the moment it starts preferring one action to another it becomes
//! another hand-written heuristic wearing CIVVIS's name.
//!
//!     civvis-orders --mirror ~/civvis-civ6-runs/control/<run> --turn 42
//!
//! # What it is honest about
//!
//! ⚠ THE RECONSTRUCTION IS PARTIAL, and the partiality has a direction. Terrain,
//! both empires' remembered cities, own units, visible hostile units, research,
//! government, development, treasury and public aggregate strength cross over.
//! Unit promotions, religious spread charges, and the religion a unit carries cross
//! through the live mirror. Firaxis's physical Great Person units are bridged through
//! its own activation verdict and legal activation plots, matching CIVVIS's
//! immediate-effect semantics without reproducing those rules here. An
//! untranslatable order or entity is refused or counted rather than guessed, so
//! partiality stays visible in the run ledger.
//!
//! ⚠ `unmapped` is reported, not swallowed. A Civ 6 unit type with no CIVVIS
//! counterpart is a unit CIVVIS cannot see, and a half-visible army produces
//! confident orders about the wrong battle.
//!
//! ⚠ Coordinates are converted at the boundary and only there. Civilization VI
//! speaks OFFSET; CIVVIS stores AXIAL. Mixing them is silent — it once put a
//! capital on no tile at all — so every position in the emitted orders goes back
//! through `hex::axial_to_offset`.

use std::path::Path;

use civvis::ai::Ai;
use civvis::game::Action;
use civvis::mirror;
use host_ledgers::*;
use host_move_audit::*;
use host_refusals::*;
use verification::*;

#[path = "civvis_orders/builder_diagnostics.rs"]
mod builder_diagnostics;
#[path = "civvis_orders/builder_operation_refusals.rs"]
mod builder_operation_refusals;

#[path = "civvis_orders/trader_origin_coverage.rs"]
mod trader_origin_coverage;

#[path = "civvis_orders/air_assault.rs"]
mod air_assault;
#[path = "civvis_orders/air_assault_continuation.rs"]
mod air_assault_continuation;
#[path = "civvis_orders/campus_above_sea.rs"]
mod campus_above_sea;
#[path = "civvis_orders/citizen_growth.rs"]
mod citizen_growth;
#[path = "civvis_orders/culture_ski_resorts.rs"]
mod culture_ski_resorts;
#[path = "civvis_orders/diagnostics.rs"]
mod diagnostics;
#[path = "civvis_orders/formation_refusals.rs"]
mod formation_refusals;
#[path = "civvis_orders/generals_lead_the_army.rs"]
mod generals_lead_the_army;
#[path = "civvis_orders/governor_exchange.rs"]
mod governor_exchange;
#[path = "civvis_orders/host_ledgers.rs"]
mod host_ledgers;
#[path = "civvis_orders/host_move_audit.rs"]
mod host_move_audit;
#[path = "civvis_orders/host_move_postconditions.rs"]
mod host_move_postconditions;
#[path = "civvis_orders/host_ranged_history.rs"]
mod host_ranged_history;
#[path = "civvis_orders/host_refusals.rs"]
mod host_refusals;
#[path = "civvis_orders/spaceport_above_sea.rs"]
mod spaceport_above_sea;
#[path = "civvis_orders/unit_identity.rs"]
mod unit_identity;
#[path = "civvis_orders/verification.rs"]
mod verification;

#[cfg(test)]
#[path = "civvis_orders/great_person_reservation_tests.rs"]
mod great_person_reservation_tests;

fn arg_text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|value| value == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

/// Every value `--victory` accepts, for the usage line and for the launchers
/// that mirror this list in Python.
const VICTORY_LANES: &str = "civvis|science|culture|religious|diplomatic|domination|score";

/// Direct invocations without `--victory` must agree with the high-level
/// launchers' one central default. The launch chain aims at Science on the
/// operator's 2026-08-30 instruction (see `civ6_play.DEFAULT_CIVVIS_VICTORY`
/// for the evidence trail); keeping a named mirror here prevents a bare
/// recovery or manual invocation from silently reviving an old lane.
const DEFAULT_VICTORY: &str = "science";

/// Build the agent for a `--victory` lane, or `None` if the name is not one.
///
/// ⚠ THE SIX NAMES ARE NOT A SEPARATE LIST. They come from
/// [`civvis::ai::VictoryTarget`]'s own `FromStr`, so a target added to the enum
/// reaches the live seat without a second edit here, and the aliases the enum
/// already accepts (`religion`/`religious`, `diplomacy`/`diplomatic`,
/// `conquest`/`domination`) work on the command line for free. The previous
/// hand-written match listed four of the six, which is why Culture, Religion
/// and Diplomacy — all three implemented in `advanced.rs` — could not be played
/// in the live seat at all.
///
/// ★ NAMING THE OBJECTIVE IS NOT MAKING THE DECISIONS. `targeting` pins which
/// victory CIVVIS plays for and leaves every choice about how to reach it —
/// war target, army size, what each city builds, where each unit goes — to
/// CIVVIS. Left to itself on a reconstruction carrying no wonders or tech
/// history it picked `religion` with `victory=None`, unreachable in 250 turns.
/// `--victory civvis` restores letting it choose, so the two are comparable.
fn victory_lane(victory: &str) -> Option<civvis::ai::AdvancedAi> {
    if victory == "civvis" {
        return Some(civvis::ai::AdvancedAi::new());
    }
    victory
        .parse::<civvis::ai::VictoryTarget>()
        .ok()
        .map(civvis::ai::AdvancedAi::targeting)
}

fn mirror_setup(
    state: &civvis::mirror::StateSnapshot,
    fallback_players: usize,
    fallback_turns: u32,
) -> (usize, u32) {
    (
        if state.seat.players > 0 {
            state.seat.players
        } else {
            fallback_players
        },
        // The engine's readback, else the speed's own limit, else the CLI's. The
        // CLI fallback is a fixed 250 (`civ6_brain` passes no `--max-turns`), so
        // a Standard seat whose readback failed used to plan to a 250-turn game:
        // every clock keyed on min(max_turns, turn_limit) -- the halfway
        // specialization clock, Culture's third -- fired at half its turn.
        civvis::mirror::host_turn_limit(state).unwrap_or(fallback_turns),
    )
}

/// JSON-escape the little that needs it. Order verbs are type names from the two
/// rulesets, so this is a guard rather than a general encoder.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The Civilization VI type name for whatever CIVVIS asked a city to build.
///
/// ⚠ ONLY UNITS USED TO TRANSLATE, so every district, building and wonder CIVVIS
/// chose was dropped — its whole economic half silently became the built-in
/// ladder's decision while telemetry still read `orders_source: civvis`. On the
/// turn-190 board that was 6 of 6 city orders reduced to units.
///
/// A wonder is a BUILDING in Civilization VI, not a category of its own. Formations
/// (Corps/Armies) need a different operation than BUILD, so they stay untranslated
/// and counted rather than guessed at.
///
/// The mod resolves the final name against `GameInfo.Units`/`Buildings`/`Districts`/
/// `Projects` and refuses what it cannot find, so a wrong guess here is reported as
/// a refusal rather than acted on.
fn civ6_unit_type(name: &civvis::name::Name) -> String {
    let id = match name.as_str() {
        "bireme" => "PHOENICIA_BIREME",
        "tagma" => "BYZANTINE_TAGMA",
        "legion" => "ROMAN_LEGION",
        "nau" => "PORTUGUESE_NAU",
        "hoplite" => "GREEK_HOPLITE",
        "eagle_warrior" => "AZTEC_EAGLE_WARRIOR",
        "war_cart" => "SUMERIAN_WAR_CART",
        "pitati_archer" => "NUBIAN_PITATI",
        "maryannu_chariot_archer" => "EGYPTIAN_CHARIOT_ARCHER",
        "saka_horse_archer" => "SCYTHIAN_HORSE_ARCHER",
        "keshig" => "MONGOLIAN_KESHIG",
        "winged_hussar" => "POLISH_HUSSAR",
        "oromo_cavalry" => "ETHIOPIAN_OROMO_CAVALRY",
        "llanero" => "COLOMBIAN_LLANERO",
        "toa" => "MAORI_TOA",
        "crouching_tiger" => "CHINESE_CROUCHING_TIGER",
        "gaesatae" => "GAUL_GAESATAE",
        "samurai" => "JAPANESE_SAMURAI",
        "hypaspist" => "MACEDONIAN_HYPASPIST",
        "varu" => "INDIAN_VARU",
        "mandekalu_cavalry" => "MALI_MANDEKALU_CAVALRY",
        "cossack" => "RUSSIAN_COSSACK",
        "rough_rider" => "AMERICAN_ROUGH_RIDER",
        "voi_chien" => "VIETNAMESE_VOI_CHIEN",
        "nihang" => "LAHORE_NIHANG",
        "mamluk" => "ARABIAN_MAMLUK",
        "sabum_kibittum" => "BABYLONIAN_SABUM_KIBITTUM",
        "minas_geraes" => "BRAZILIAN_MINAS_GERAES",
        "dromon" => "BYZANTINE_DROMON",
        "mountie" => "CANADA_MOUNTIE",
        "okihtcitaw" => "CREE_OKIHTCITAW",
        "viking_longship" => "NORWEGIAN_LONGSHIP",
        "sea_dog" => "ENGLISH_SEADOG",
        "redcoat" => "ENGLISH_REDCOAT",
        "garde_imperiale" => "FRENCH_GARDE_IMPERIALE",
        "khevsureti" => "GEORGIAN_KHEVSURETI",
        "u_boat" => "GERMAN_UBOAT",
        "huszar" => "HUNGARY_HUSZAR",
        "black_army" => "HUNGARY_BLACK_ARMY",
        "warakaq" => "INCA_WARAKAQ",
        "jong" => "INDONESIAN_JONG",
        "domrey" => "KHMER_DOMREY",
        "hwacha" => "KOREAN_HWACHA",
        "hetairoi" => "MACEDONIAN_HETAIROI",
        "malon_raider" => "MAPUCHE_MALON_RAIDER",
        "hulche" => "MAYAN_HULCHE",
        "berserker" => "NORWEGIAN_BERSERKER",
        "barbary_corsair" => "OTTOMAN_BARBARY_CORSAIR",
        "janissary" => "SULEIMAN_JANISSARY",
        "immortal" => "PERSIAN_IMMORTAL",
        "highlander" => "SCOTTISH_HIGHLANDER",
        "conquistador" => "SPANISH_CONQUISTADOR",
        "carolean" => "SWEDEN_CAROLEAN",
        "impi" => "ZULU_IMPI",
        "p51_mustang" => "AMERICAN_P51",
        "anti_air_gun" => "ANTIAIR_GUN",
        other => return format!("UNIT_{}", other.to_ascii_uppercase()),
    };
    format!("UNIT_{id}")
}

fn civ6_improvement_type(name: &civvis::name::Name) -> String {
    let id = match name.as_str() {
        "seaside_resort" => "BEACH_RESORT",
        "qhapaq_nan" => "MOUNTAIN_ROAD",
        "nubian_pyramid" => "PYRAMID",
        other => return format!("IMPROVEMENT_{}", other.to_ascii_uppercase()),
    };
    format!("IMPROVEMENT_{id}")
}

/// Civilization VI keeps the three wall tiers under internal implementation names.
///
/// The mirror already translates these names in the other direction.  Keep the
/// outbound boundary symmetric: live turns 165-219 refused 200 production orders
/// for the nonexistent `BUILDING_MEDIEVAL_WALLS`, leaving three or four queues idle
/// per turn.  The shipped Buildings.xml names the tiers WALLS, CASTLE, and STAR_FORT.
/// ⚠⚠ THE WALL TIERS WERE NOT THE ONLY THREE. `mirror::civvis_node_name` carries a
/// nineteen-entry inbound alias table for exactly this vocabulary gap, and only the
/// walls half of it was ever mirrored back out. Every other entry was still being
/// uppercased into a name the shipped database does not contain, and the mod that
/// resolves against `GameInfo.Buildings` refused each one. Counted over the runs under
/// `civvis-civ6-runs/control` on 2026-08-02:
///
/// ```text
/// unknown_BUILDING_ARCHAEOLOGICAL_MUSEUM  449
/// unknown_DISTRICT_GOVERNMENT_PLAZA       310
/// unknown_BUILDING_MEDIEVAL_WALLS         275   (already fixed; runs predate it)
/// unknown_TECH_WHEEL                      110
/// unknown_BUILDING_ART_MUSEUM             108
/// unknown_DISTRICT_THEATER_SQUARE          24
/// unknown_BUILDING_OIL_POWER_PLANT         10
/// ```
///
/// The culture cost is direct and it is the whole reason this was found. A Museum is
/// the third step of the only culture chain in the game — Theater Square →
/// Amphitheater → Art/Archaeological Museum → Broadcast Center — and it carries +2
/// Culture and **three Great Work slots**, the largest late-game culture source there
/// is. Across 45 live runs that reached turn 150, **not one city ever finished a
/// Museum**, and the empire held a median of **zero Great Works**. The order went out
/// 507 times and was refused every time.
///
/// ⚠ Every replacement here was read out of the shipped
/// `Cache/DebugGameplay.sqlite`, not inferred from the display name: there is no
/// `BUILDING_ART_MUSEUM` in Civilization VI, the row is `BUILDING_MUSEUM_ART`, and the
/// nine Government Plaza buildings are named for their *tier* (`BUILDING_GOV_TALL`)
/// rather than their subject. Keep this table the exact inverse of
/// `mirror::civvis_node_name`'s — a name that only translates one way is the defect
/// this pair of tables exists to prevent.
fn civ6_building_type(name: &civvis::name::Name) -> String {
    let id = match name.as_str() {
        "ancient_walls" => "WALLS",
        "medieval_walls" => "CASTLE",
        "renaissance_walls" => "STAR_FORT",
        "art_museum" => "MUSEUM_ART",
        "archaeological_museum" => "MUSEUM_ARTIFACT",
        "oil_power_plant" => "FOSSIL_FUEL_POWER_PLANT",
        "nuclear_power_plant" => "POWER_PLANT",
        "mausoleum_at_halicarnassus" => "HALICARNASSUS_MAUSOLEUM",
        "statue_of_liberty" => "STATUE_LIBERTY",
        "university_of_sankore" => "UNIVERSITY_SANKORE",
        // The Government Plaza tier buildings. Firaxis names the slot, not the
        // building: `gov_culture` is the National History Museum.
        "audience_chamber" => "GOV_TALL",
        "ancestral_hall" => "GOV_WIDE",
        "warlords_throne" => "GOV_CONQUEST",
        "foreign_ministry" => "GOV_CITYSTATES",
        "grand_masters_chapel" => "GOV_FAITH",
        "intelligence_agency" => "GOV_SPIES",
        "national_history_museum" => "GOV_CULTURE",
        "royal_society" => "GOV_SCIENCE",
        "war_department" => "GOV_MILITARY",
        other => return format!("BUILDING_{}", other.to_ascii_uppercase()),
    };
    format!("BUILDING_{id}")
}

/// Civilization VI truncates two district names and spells a third differently.
///
/// `DISTRICT_THEATER_SQUARE` and `DISTRICT_GOVERNMENT_PLAZA` are CIVVIS spellings; the
/// shipped rows are `DISTRICT_THEATER` and `DISTRICT_GOVERNMENT`. The inbound reader
/// already recovers both — `civvis_node_name`'s unique-prefix rule was added for
/// `DISTRICT_GOVERNMENT` specifically — so this is the missing outbound half.
fn civ6_district_type(name: &civvis::name::Name) -> String {
    let id = match name.as_str() {
        "theater_square" => "THEATER",
        "government_plaza" => "GOVERNMENT",
        "water_park" => "WATER_ENTERTAINMENT_COMPLEX",
        // Brazil's Water Park replacement; `data/districts.json` records
        // `"replaces": "water_park"`, so it takes the water spelling.
        "copacabana" => "WATER_STREET_CARNIVAL",
        other => return format!("DISTRICT_{}", other.to_ascii_uppercase()),
    };
    format!("DISTRICT_{id}")
}

fn civ6_build_name(item: &civvis::game::Item) -> Option<String> {
    use civvis::game::Item;
    let upper = |name: &civvis::name::Name| name.as_str().to_ascii_uppercase();
    match item {
        Item::Unit { unit } => Some(civ6_unit_type(unit)),
        Item::Building { building } => Some(civ6_building_type(building)),
        Item::District { district, .. } => Some(civ6_district_type(district)),
        // ⚠ A WONDER IS A BUILDING AND MUST USE THE BUILDING TABLE. #959 put the
        // divergent spellings in `civ6_building_type`, but this arm still formatted its
        // own name, so the three wonders Civilization VI spells differently
        // — `BUILDING_HALICARNASSUS_MAUSOLEUM`, `BUILDING_STATUE_LIBERTY`,
        // `BUILDING_UNIVERSITY_SANKORE` — kept going out mechanically uppercased and
        // kept being refused. Two arms formatting the same table one line apart is the
        // shape that made this a bug twice.
        Item::Wonder { wonder, .. } => Some(civ6_building_type(wonder)),
        // ⚠ CIVVIS'S DISTRICT PROJECTS ARE NOT NAMED LIKE CIVILIZATION VI'S, and the
        // mechanical uppercase below produced names the game has never heard of.
        //
        // `campus_research_grants` became `PROJECT_CAMPUS_RESEARCH_GRANTS`, which
        // appears in ZERO shipped Assets — live run `civvis-20260801T145302Z`
        // refused it as `unknown_PROJECT_CAMPUS_RESEARCH_GRANTS` ten times. Firaxis
        // names every district project `PROJECT_ENHANCE_DISTRICT_<DISTRICT>`; CIVVIS
        // names each one after what it DOES. All seven were silently unbuildable.
        Item::Project { project } => Some(match project.as_str() {
            "campus_research_grants" => "PROJECT_ENHANCE_DISTRICT_CAMPUS".to_string(),
            "commercial_hub_investment" => "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB".to_string(),
            "encampment_training" => "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT".to_string(),
            "harbor_shipping" => "PROJECT_ENHANCE_DISTRICT_HARBOR".to_string(),
            "holy_site_prayers" => "PROJECT_ENHANCE_DISTRICT_HOLY_SITE".to_string(),
            "industrial_zone_logistics" => "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE".to_string(),
            "theater_square_festival" => "PROJECT_ENHANCE_DISTRICT_THEATER".to_string(),
            // ⚠ The space-race projects diverge too, and in four different ways: the
            // shipped rows are LAUNCH_EXOPLANET_EXPEDITION (CIVVIS drops the verb),
            // LAUNCH_MARS_BASE (CIVVIS says colony), and the two lasers are
            // TERRESTRIAL_LASER / ORBITAL_LASER with no "station" and no Lagrange.
            "exoplanet_expedition" => "PROJECT_LAUNCH_EXOPLANET_EXPEDITION".to_string(),
            "launch_mars_colony" => "PROJECT_LAUNCH_MARS_BASE".to_string(),
            "terrestrial_laser_station" => "PROJECT_TERRESTRIAL_LASER".to_string(),
            "lagrange_laser_station" => "PROJECT_ORBITAL_LASER".to_string(),
            // ⚠ `repair_encampment` is DELIBERATELY not translated, though it is the
            // second-largest refusal class at 342. Civilization VI has no encampment
            // repair project — a pillaged Encampment is repaired by ordering the
            // district again, which needs the plot this arm cannot see. Mapping it to
            // `DISTRICT_ENCAMPMENT` here would only trade `unknown_` refusals for
            // `build_no_plot` ones. It belongs on the `Item::Repair` path in
            // `civ6_live_build_name`, which already recovers the plot from the mirror.
            // The rest ARE mechanical: `build_nuclear_device` really is
            // `PROJECT_BUILD_NUCLEAR_DEVICE`.
            _ => format!("PROJECT_{}", upper(project)),
        }),
        _ => None,
    }
}

/// Resolve a live production item that needs board context.
///
/// Firaxis repairs a pillaged district by building that already-placed district
/// type again. CIVVIS names the repair operation and its plot, so recover the
/// district type from the authoritative mirrored tile instead of inventing a
/// `PROJECT_REPAIR_*` id that does not exist in the shipped database.
fn civ6_live_build_name(item: &civvis::game::Item, game: &civvis::game::Game) -> Option<String> {
    use civvis::game::Item;
    match item {
        // ⚠ THIS ARM MUST USE `civ6_district_type`, NOT ITS OWN UPPERCASE.
        //
        // It formatted its own name and so re-introduced every divergent
        // district spelling the rest of this file had already repaired. Live run
        // `civvis-20260803T094641Z` emitted `DISTRICT_GOVERNMENT_PLAZA` **26
        // times** on a revision where `civ6_district_type` had mapped it to
        // `DISTRICT_GOVERNMENT` for hours; the host discarded all 26.
        //
        // This is the third time the same shape has cost a live game: #959 put
        // the wonder spellings in `civ6_building_type` while `Item::Wonder`
        // still formatted its own, and #983's map export was rejected wholesale
        // for a related reason. **One table, one formatter.**
        Item::Repair { repair, pos } if repair == "district" => game
            .map
            .get(*pos)
            .and_then(|tile| tile.district.as_ref())
            .map(civ6_district_type),
        Item::Repair { repair, .. } => Some(civ6_building_type(repair)),
        _ => civ6_build_name(item),
    }
}

/// Preserve the plot CIVVIS selected for placeable production.
///
/// District and wonder placement is part of the decision, not an implementation
/// detail. Firaxis expects offset coordinates while CIVVIS stores axial ones.
fn civ6_build_pos(item: &civvis::game::Item) -> Option<(i32, i32)> {
    use civvis::game::Item;
    match item {
        Item::District { pos, .. } | Item::Wonder { pos, .. } => {
            Some(civvis::hex::axial_to_offset(pos.0, pos.1))
        }
        _ => None,
    }
}

/// The host's alliance type for a planner alliance kind (`propose_strategic_alliance`
/// picks one of these five), the `DB.MakeHash` value the shipped screen sets on the
/// ALLIANCE agreement item (DiplomacyActionView_Expansion1.lua:149-217). `None` for
/// anything else, so an unknown kind is skipped rather than asked blind.
fn civ6_alliance_type(kind: &str) -> Option<&'static str> {
    match kind {
        "research" => Some("ALLIANCE_RESEARCH"),
        "cultural" => Some("ALLIANCE_CULTURAL"),
        "religious" => Some("ALLIANCE_RELIGIOUS"),
        "military" => Some("ALLIANCE_MILITARY"),
        "economic" => Some("ALLIANCE_ECONOMIC"),
        _ => None,
    }
}

/// ⚠ THE TWO RULESETS DISAGREE ON ARTICLES, AND IT COSTS A WHOLE TECHNOLOGY.
///
/// `mirror::civvis_node_name` documents the inbound half: Civ 6's `TECH_THE_WHEEL` is
/// CIVVIS's `wheel`, and it strips the leading article to cross. Outbound never put it
/// back, so every research order for the Wheel went out as `TECH_WHEEL` and was
/// refused — **110 times** across the runs under `civvis-civ6-runs/control`. The Wheel
/// gates Horseback Riding, Bronze Working's siege line and the Water Mill, so a seat
/// that could never select it was steering around a hole in its own tech tree.
///
/// The article is the only divergence in the 77-row technology table (audited against
/// `Cache/DebugGameplay.sqlite` on 2026-08-02) and civics diverge nowhere, so this
/// stays an exact list rather than a prefix rule.
fn civ6_tech_name(civvis: &str) -> String {
    match civvis {
        "wheel" => "TECH_THE_WHEEL".to_string(),
        other => format!("TECH_{}", other.to_ascii_uppercase()),
    }
}

fn civ6_civic_name(civvis: &str) -> String {
    format!("CIVIC_{}", civvis.to_ascii_uppercase())
}

/// The CIVVIS dedication ids are the stable names used by the rules, while
/// Civilization VI exposes the same choices as `COMMEMORATION_*` operation
/// values. Keep the inverse of `mirror::civvis_dedication_name` explicit: the
/// two vocabularies are not a prefix transform (`free_inquiry` is
/// `COMMEMORATION_SCIENTIFIC`, for example).
fn civ6_dedication_name(civvis: &str) -> Option<&'static str> {
    Some(match civvis {
        "monumentality" => "COMMEMORATION_INFRASTRUCTURE",
        "pen_brush_and_voice" => "COMMEMORATION_CULTURAL",
        "free_inquiry" => "COMMEMORATION_SCIENTIFIC",
        "exodus_of_the_evangelists" => "COMMEMORATION_RELIGIOUS",
        "hic_sunt_dracones" => "COMMEMORATION_EXPLORATION",
        "reform_the_coinage" => "COMMEMORATION_ECONOMIC",
        "to_arms" => "COMMEMORATION_MILITARY",
        "heartbeat_of_steam" => "COMMEMORATION_INDUSTRIAL",
        "bodyguard_of_lies" => "COMMEMORATION_ESPIONAGE",
        "sky_and_stars" => "COMMEMORATION_AERONAUTICAL",
        "wish_you_were_here" => "COMMEMORATION_TOURISM",
        "automaton_warfare" => "COMMEMORATION_AUTOMATON",
        _ => return None,
    })
}

/// CIVVIS mostly uses Firaxis promotion identifiers without their prefix. Keep the
/// few deliberate vocabulary contractions explicit in both directions rather than
/// asking Lua to guess from localized display names.
///
/// ★★★ A SPY PROMOTION IS NOT SPELLED LIKE A SOLDIER'S. Civilization VI puts the
/// promotion class in the identifier — `PROMOTION_SPY_SMEAR_CAMPAIGN`, not
/// `PROMOTION_SMEAR_CAMPAIGN` — and the uppercase fallthrough below produced the
/// second one for all seventeen. Nothing failed loudly: the mod answered
/// `unknown_promotion_PROMOTION_SMEAR_CAMPAIGN`, the order was counted refused,
/// and the spy kept its level.
///
/// It arrived with espionage (#1929) and is measurable in the ledger's own
/// bridge-health column: `orders_applied / orders_seen` ran 95–98.4% for every
/// run through `civvis-20260818T003523Z` and 84.0–90.6% for every run after it,
/// with 259–341 `unknown_promotion_PROMOTION_*` refusals per game — the largest
/// single refusal category on the seat.
///
/// `Game::SPY_PROMOTIONS` is the engine's own list, read here rather than copied,
/// so a promotion added there cannot go out under the wrong spelling. Sixteen of
/// the seventeen take the prefix unchanged; Firaxis spells the last one
/// `GUERILLA_LEADER`, with one `r`.
fn civ6_unit_promotion_name(civvis: &str) -> String {
    if civvis == "guerrilla_leader" {
        // Firaxis' own spelling, in `Expansion1_UnitPromotions.xml`.
        return "PROMOTION_SPY_GUERILLA_LEADER".to_string();
    }
    if civvis::game::Game::SPY_PROMOTIONS.contains(&civvis) {
        return format!("PROMOTION_SPY_{}", civvis.to_ascii_uppercase());
    }
    let suffix = match civvis {
        "cobra_strike" | "dancing_crane" | "disciples" | "exploding_palms" | "shadow_strike"
        | "sweeping_wind" | "twilight_veil" => {
            format!("MONK_{}", civvis.to_ascii_uppercase())
        }
        "supercarrier" => "SUPER_CARRIER".to_string(),
        "goes_to_11" => "GOES_TO".to_string(),
        "pop_star" => "POP".to_string(),
        "surf_band" => "SURF_ROCK".to_string(),
        other => other.to_ascii_uppercase(),
    };
    format!("PROMOTION_{suffix}")
}

struct Order {
    kind: &'static str,
    subject: Option<i64>,
    verb: Option<String>,
    pos: Option<(i32, i32)>,
}

fn append_citizen_growth_orders(
    snapshot: &mirror::Snapshot,
    state: &mirror::StateSnapshot,
    widened: bool,
    orders: &mut Vec<Order>,
) -> usize {
    let choices = citizen_growth::choices(snapshot, state, widened);
    let count = choices.len();
    orders.extend(choices.into_iter().map(|choice| Order {
        kind: "city_focus",
        subject: Some(choice.city),
        verb: Some(choice.verb.to_string()),
        pos: None,
    }));
    count
}

/// Marker kept in `ours` while a `produce_next` lease is waiting for the host
/// queue to finish.  It prevents the next fresh planning board from releasing
/// the still-running item as foreign before the blocker consumes the lease.
const DEFERRED_PRODUCTION_PREFIX: &str = "__civvis_next__:";

/// Whether a host-reported queue is expected to finish during this turn.
///
/// A next-build hint is only safe when the current item is already paid for by
/// the city's own production forecast.  Unknown host metrics deliberately do
/// not qualify: guessing would turn a deferred handoff into an early queue
/// replacement, which is the exact actuation bug this path is meant to avoid.
fn host_production_finishes_this_turn(city: &mirror::StateCity) -> bool {
    city.producing.is_some()
        && city.production >= 0.0
        && city.production_cost > 0.0
        && city.production_progress >= 0.0
        && (city.production_turns >= 0.0 && city.production_turns <= 1.0 + f64::EPSILON
            || city.production_progress + city.production + f64::EPSILON >= city.production_cost)
}

/// The control mod may replace CIVVIS's requested queue with Walls when an
/// unwalled city is damaged or has a visible major-enemy unit within its
/// emergency radius.  That replacement is authoritative host state, but the
/// original request remains in `ours` unless the bridge reconciles it.  Without
/// this lease, the next fresh-board decision clears the wall queue and repeats
/// the same Campus/Archer request on every frame while the city is under threat.
/// Keep this predicate identical to the Lua emergency-wall gate: no walls,
/// actual damage or a visible unit from a rival we are at war with nearby.
const EMERGENCY_WALL_RADIUS: i32 = 3;

fn host_emergency_wall_is_active(
    state: &civvis::mirror::StateSnapshot,
    city: &civvis::mirror::StateCity,
) -> bool {
    if city.producing.as_deref() != Some("BUILDING_WALLS")
        || !city.max_wall_damage.is_finite()
        || city.max_wall_damage > 0.0
    {
        return false;
    }
    let damaged = city.damage.is_finite() && city.damage > 0.0;
    let nearby_enemy = state
        .rivals
        .iter()
        .filter(|rival| rival.at_war)
        .flat_map(|rival| rival.units.iter())
        .any(|unit| offset_distance((city.x, city.y), (unit.x, unit.y)) <= EMERGENCY_WALL_RADIUS);
    damaged || nearby_enemy
}

/// Adopt only the host's bounded emergency-wall replacement on every decision.
/// Ordinary host queues remain self-limiting: they are adopted on a decider
/// restart, while a CIVVIS choice already in `ours` is allowed to be replaced
/// normally.  The emergency response is different because releasing it during
/// the same threat window removes the host's safety action before the next
/// production callback can complete it.
fn adopt_emergency_wall_production(
    ours: &mut std::collections::BTreeMap<i64, String>,
    state: &civvis::mirror::StateSnapshot,
) -> usize {
    let mut adopted = 0;
    for city in &state.cities {
        if !host_emergency_wall_is_active(state, city) {
            continue;
        }
        if ours.get(&city.id).map(String::as_str) != Some("BUILDING_WALLS") {
            ours.insert(city.id, "BUILDING_WALLS".to_string());
            adopted += 1;
        }
    }
    adopted
}

/// Firaxis remembers a submitted peace offer per target for five turns.
///
/// `CivvisControlAgent.lua` rejects another offer while
/// `turn - asked < (cfg.PeaceRetryTurns or 5)`. Keep that host-only fact at the
/// persistent outbound boundary: the planner can still decide to seek peace on
/// every turn, but the bridge emits only the first request and the next legal
/// retry. It also survives a fresh mirror or fresh AI, because the cooldown
/// belongs to Firaxis rather than either reconstructed object.
const HOST_PEACE_RETRY_TURNS: u32 = 5;

#[derive(Default)]
struct HostPeaceRetries {
    last_request: std::collections::BTreeMap<i64, u32>,
}

impl HostPeaceRetries {
    fn observe(&mut self, state: &civvis::mirror::StateSnapshot) {
        self.last_request.retain(|target, asked| {
            *asked <= state.turn
                && (state
                    .rivals
                    .iter()
                    .any(|rival| rival.at_war && rival.player as i64 == *target)
                    || state
                        .minors
                        .iter()
                        .any(|minor| minor.at_war && minor.player as i64 == *target))
        });
    }

    fn permits(&self, target: i64, turn: u32) -> bool {
        self.last_request
            .get(&target)
            .is_none_or(|asked| turn.saturating_sub(*asked) >= HOST_PEACE_RETRY_TURNS)
    }

    fn record(&mut self, target: i64, turn: u32) {
        self.last_request.insert(target, turn);
    }
}

/// The minimum delegation Civilization VI requires for any suzerainty, before a
/// rival's standing raises that floor.
const SUZERAIN_ENVOY_FLOOR: i64 = 3;

/// How many of our held Envoys would immediately take this city-state.
fn envoys_to_take_suzerainty(minor: &civvis::mirror::StateMinor) -> i64 {
    (SUZERAIN_ENVOY_FLOOR.max(minor.most_envoys.saturating_add(1)) - minor.envoys.max(0)).max(1)
}

/// Whether this city-state is only at war because its current Suzerain is.
///
/// A city-state follows its Suzerain into that war, so a direct peace operation
/// cannot make Envoys legal.  The relevant major must be met to have a row in
/// `rivals`; an unrepresented Suzerain is deliberately not guessed to be at war.
fn city_state_war_is_derived(
    state: &civvis::mirror::StateSnapshot,
    minor: &civvis::mirror::StateMinor,
) -> bool {
    minor.suzerain >= 0
        && state
            .rivals
            .iter()
            .any(|rival| rival.at_war && rival.player as i32 == minor.suzerain)
}

/// Convert one affordable, direct city-state war into the first half of an
/// Envoy investment.
///
/// Civilization VI rejects `GIVE_INFLUENCE_TOKEN` while a war is active, and its
/// diplomacy operation lands asynchronously.  Submit exactly one peace order
/// now; the next exported frame is the authority for whether normal envoy
/// planning may spend the pool.  The caller retains that exact cost only on an
/// accepted peace frame.  Picking the lowest threshold puts a limited pool
/// behind the soonest available Suzerain bonus.
fn queue_city_state_envoy_reclaim(
    orders: &mut Vec<Order>,
    state: &civvis::mirror::StateSnapshot,
) -> Option<(i64, i64)> {
    let held = state.envoys_free.filter(|held| *held > 0)?;
    let (needed, target) = state
        .minors
        .iter()
        .filter(|minor| minor.is_city_state() && minor.at_war)
        .filter(|minor| !city_state_war_is_derived(state, minor))
        .map(|minor| (envoys_to_take_suzerainty(minor), minor.player as i64))
        .filter(|(needed, _)| held >= *needed)
        .filter(|(_, target)| {
            !orders.iter().any(|order| {
                order.subject == Some(*target) && matches!(order.kind, "war" | "peace")
            })
        })
        .min_by_key(|(needed, target)| (*needed, *target))?;
    orders.push(Order {
        kind: "peace",
        subject: Some(target),
        verb: Some("MAKE_PEACE".to_string()),
        pos: None,
    });
    Some((target, needed))
}

/// Pair an already-planned major peace with the city-state suzerainty it frees.
///
/// A city-state at war through its hostile Suzerain cannot accept a direct
/// peace operation or an Envoy.  The major's peace, unlike a direct minor
/// peace, is a bilateral offer that can be refused and later ask for Gold.  Do
/// not manufacture that strategic trade merely to spend Envoys: wait until the
/// planner has already chosen the peace, then retain the exact takeover cost
/// for the authoritative post-peace frame.  A successful deal is the only
/// event that makes the minor legal again.
///
/// Returns `(suzerain, minor, needed)`, choosing the cheapest immediately
/// affordable reclaim.  The caller deliberately keeps the direct-city-state
/// reclaim ahead of this one: a minor peace is a unilateral, lower-risk path.
fn planned_suzerain_peace_envoy_reclaim(
    orders: &[Order],
    state: &civvis::mirror::StateSnapshot,
) -> Option<(i64, i64, i64)> {
    let held = state.envoys_free.filter(|held| *held > 0)?;
    state
        .minors
        .iter()
        .filter(|minor| minor.is_city_state() && minor.at_war)
        .filter(|minor| city_state_war_is_derived(state, minor))
        .filter_map(|minor| {
            let suzerain = i64::from(minor.suzerain);
            let peace_is_planned = orders
                .iter()
                .any(|order| order.kind == "peace" && order.subject == Some(suzerain));
            let war_is_planned = orders
                .iter()
                .any(|order| order.kind == "war" && order.subject == Some(suzerain));
            (peace_is_planned && !war_is_planned).then_some((
                suzerain,
                minor.player as i64,
                envoys_to_take_suzerainty(minor),
            ))
        })
        .filter(|(_, _, needed)| held >= *needed)
        .min_by_key(|(suzerain, minor, needed)| (*needed, *minor, *suzerain))
}

/// Keep the exact cost of a submitted city-state peace available for the next
/// authoritative frame, while still investing any surplus Envoys immediately.
///
/// The normal per-turn envoy cap means no more than `ENVOY_ORDERS_PER_TURN`
/// existing orders can land in this host batch.  Removing the target itself is
/// defensive: the planning board cannot emit it during war, but the host state
/// is authoritative and must never receive peace and envoy for one minor at once.
fn reserve_envoys_for_submitted_reclaim(
    orders: &mut Vec<Order>,
    target: i64,
    held: i64,
    needed: i64,
) -> usize {
    let mut remaining_surplus = held
        .saturating_sub(needed)
        .clamp(0, ENVOY_ORDERS_PER_TURN as i64) as usize;
    let mut deferred = 0;
    orders.retain(|order| {
        if order.kind != "envoy" {
            return true;
        }
        if order.subject == Some(target) || remaining_surplus == 0 {
            deferred += 1;
            false
        } else {
            remaining_surplus -= 1;
            true
        }
    });
    deferred
}

/// A later frame can discover a fortification need after the first queue
/// request has already reached the host. Require its actual readback before
/// handing the lease to Walls, then keep that fortified queue for the turn.
/// Ordinary changes and repeated proposals retain the existing lease.
fn same_turn_wall_handoff(
    state: &civvis::mirror::StateSnapshot,
    city: i64,
    first: &HostProductionLease,
    requested: &str,
) -> bool {
    if state.frame <= first.frame
        || first.verb == requested
        || !state.cities.iter().any(|observed| {
            observed.id == city && observed.producing.as_deref() == Some(first.verb.as_str())
        })
    {
        return false;
    }
    let rules = civvis::rules::Rules::shared();
    let fortification = |host: &str| {
        civvis::mirror::host_production_key(&rules, host, None).is_some_and(|key| {
            key.strip_prefix("building:")
                .and_then(|name| rules.buildings.get(name))
                .is_some_and(|building| building.outer_defense > 0)
        })
    };
    fortification(requested) && !fortification(&first.verb)
}

#[cfg(test)]
#[path = "civvis_orders/same_turn_wall_handoff_tests.rs"]
mod same_turn_wall_handoff_tests;

#[cfg(test)]
#[path = "civvis_orders/first_pad_observed_commitment_tests.rs"]
mod first_pad_observed_commitment_tests;

/// Apply the producer rule to the observed frame, before acquiring a native
/// production lease. A discarded simulated district is not a placed native pad.
fn keep_observed_first_pad_floor(
    orders: &mut Vec<Order>,
    allowed: &std::collections::BTreeSet<i64>,
    previous_ours: &std::collections::BTreeMap<i64, String>,
    ours: &mut std::collections::BTreeMap<i64, String>,
) -> usize {
    let before = orders.len();
    let mut rejected = std::collections::BTreeSet::new();
    orders.retain(|order| {
        let held = order.kind == "produce"
            && order.verb.as_deref() == Some("DISTRICT_SPACEPORT")
            && order.pos.is_some()
            && order.subject.is_some_and(|city| !allowed.contains(&city));
        if held {
            rejected.insert(order.subject.unwrap());
        }
        !held
    });
    for city in rejected {
        if !orders
            .iter()
            .any(|order| order.kind == "produce" && order.subject == Some(city))
        {
            if let Some(previous) = previous_ours.get(&city) {
                ours.insert(city, previous.clone());
            } else {
                ours.remove(&city);
            }
        }
    }
    before - orders.len()
}

/// Keep only commands whose preconditions belong to the observed Firaxis frame —
/// but send a unit's whole planned WALK, not just its first step.
///
/// CIVVIS applies a unit's whole turn synchronously to its planning clone, so a
/// builder can log MOVE_TO followed by IMPROVE and a military unit can log several
/// path steps. Firaxis operations are asynchronous: when Lua submits that list in
/// one callback, every later command still sees the unit at its original position.
/// The next exported frame is the first point where another command for that unit
/// can be evaluated honestly. Different units and non-unit orders remain independent.
///
/// ★★★★★ ONE HEX PER TURN WAS THE PRICE OF THAT RULE, FOR EVERY UNIT, FOR TWO
/// WEEKS. CIVVIS's own movement is one `Move` action per hex, so a settler with two
/// movement points logs `MOVE_TO a, MOVE_TO b` and a scout logs three steps.
/// Keeping the FIRST order per unit therefore sent exactly one adjacent hex per turn
/// and deferred the rest — measured on run civvis-20260815T190904Z: **442 of 442**
/// MOVE_TO orders were the only MOVE_TO their unit received that turn, every one
/// aimed at an adjacent hex, and scouts (3 MP), settlers (2 MP) and builders (2 MP)
/// all crawled one tile per turn. That is the whole "live settlers cross 0.78
/// tiles/turn on 2 movement points" finding, and it compounds: a guard walks to its
/// settler at 1 hex/turn, the settler waits, the founding lands 7 turns after the
/// settler was built for a 4-hex trip, and `settlers == 0` blocks the next one
/// meanwhile.
///
/// The causal-safety rule only requires that a later command not be evaluated
/// against a position the unit has not yet reached. A path of moves is different
/// from a move followed by an act: `UNITOPERATION_MOVE_TO` takes a DESTINATION and
/// Firaxis paths to it with the unit's real movement, spending as much of this turn
/// as it can and continuing next turn if the ground was further than CIVVIS priced
/// (the mod already treats stopping short as ordinary — see `applyOrders`). So the
/// contiguous prefix of a unit's MOVE_TO steps collapses into ONE MOVE_TO at the
/// furthest planned hex, and only the first NON-move command (IMPROVE, FOUND_CITY,
/// ATTACK, RANGE_ATTACK, FORTIFY, ...) and anything after it still wait for the
/// next frame, exactly as before. Melee attacks stay conservative on purpose: CIVVIS
/// chose the tile it strikes from, and a MOVE_TO onto the defender would let the
/// host pathfinder pick another; the finishing-volley code above already collapses
/// approach+blow where it has proved the line on a private board.
///
/// ★★★★★ AND THAT DEFERRAL WAS THE PRICE OF EVERY STRIKE THAT FOLLOWS A STEP.
/// A friendly volley is move-then-shoot, and a mover's own step onto a firing
/// tile is followed by the shot it opened — and every one of those arrived here
/// as a walk plus a follow-up, and left as a walk. Measured on
/// run civvis-20260803T005930Z: 7 melee ATTACK orders against 1,546 MOVE_TO
/// in 188 turns of war; 622 of 1,787 military unit-turns hovering 2–4 hexes
/// from a target. The unit stepped into contact and stood there, unstruck,
/// for the enemy's whole turn.
///
/// With `sequenced` — the run's `seat` event says `order_queue: true`, i.e.
/// the mod that will apply these rows keeps a per-unit queue (`CivvisQueue`)
/// and issues each later order once the earlier one has arrived — the walk
/// still folds into one `MOVE_TO`, and every order after it now rides along
/// in sequence instead of waiting a turn. Against an older mod the behaviour
/// is byte-identical to before: a capability the sender assumes and the
/// receiver lacks is how an accepted order becomes a silent no-op.
#[cfg(test)]
fn coalesce_unit_paths(orders: Vec<Order>, sequenced: bool) -> (Vec<Order>, usize, usize) {
    coalesce_unit_paths_except(orders, sequenced, &Default::default(), &Default::default())
}

/// The centre tiles of every city we are at war with, in Civilization VI
/// offset coordinates (the coordinates `Order::pos` carries).
///
/// ★★★★ A WALK THAT TAKES A CITY ENDS THERE. Live King 20261004T100903Z
/// (G52), turn 191: the Siege of Mashhad left the city at 0 health behind
/// fallen walls, CIVVIS moved its Helicopter in ("taken by the helicopter"),
/// then the battle planner rotated the wounded Helicopter (77 hp) out to heal.
/// `coalesce_unit_paths_except` folded the step INTO the city and the step OUT
/// of it into one `MOVE_TO` to the healing tile, so the host flew the
/// Helicopter past Mashhad and the city healed. Persia's capital stayed
/// Persian.
fn capture_tiles(state: &civvis::mirror::StateSnapshot) -> std::collections::BTreeSet<(i32, i32)> {
    state
        .rivals
        .iter()
        .filter(|rival| rival.at_war)
        .flat_map(|rival| rival.cities.iter())
        .chain(
            state
                .minors
                .iter()
                .filter(|minor| minor.at_war)
                .flat_map(|minor| minor.cities.iter()),
        )
        .map(|city| (city.x, city.y))
        .collect()
}

/// A wounded military unit near a known enemy must follow the planner's safe
/// steps. Sending only the destination lets Firaxis choose a different path,
/// including a shorter route through the threat the planner walked around.
/// The host queue executes each step after the preceding move arrives; older
/// hosts get only the first step and replan from the next observed position.
/// Known hostile cities and intact Encampments matter even when no enemy army
/// is in sight. Their positions already belong to the observed board; preserving
/// a route around them requires no inference about a fogged city's strength.
fn wounded_local_routes(state: &civvis::mirror::StateSnapshot) -> std::collections::BTreeSet<i64> {
    let military = state
        .hostiles
        .iter()
        .chain(
            state
                .rivals
                .iter()
                .filter(|rival| rival.at_war)
                .flat_map(|rival| rival.units.iter()),
        )
        .chain(
            state
                .minors
                .iter()
                .filter(|minor| minor.at_war)
                .flat_map(|minor| minor.units.iter()),
        )
        .filter(|enemy| enemy.combat > 0.0 || enemy.ranged > 0.0)
        .map(|enemy| (enemy.x, enemy.y));
    let cities = state
        .rivals
        .iter()
        .filter(|rival| rival.at_war)
        .flat_map(|rival| rival.cities.iter())
        .chain(
            state
                .minors
                .iter()
                .filter(|minor| minor.at_war)
                .flat_map(|minor| minor.cities.iter()),
        );
    let threats: Vec<_> = military
        .chain(cities.flat_map(|city| {
            std::iter::once((city.x, city.y)).chain(
                city.districts
                    .iter()
                    .filter(|district| {
                        district.kind == "DISTRICT_ENCAMPMENT"
                            && district.complete
                            && !district.pillaged
                    })
                    .map(|district| (district.x, district.y)),
            )
        }))
        .collect();
    state
        .units
        .iter()
        .filter(|unit| {
            unit.hp > 0.0
                && unit.hp < 100.0
                && (unit.combat > 0.0 || unit.ranged > 0.0)
                && threats
                    .iter()
                    .any(|&pos| offset_distance((unit.x, unit.y), pos) <= 3)
        })
        .map(|unit| unit.id)
        .collect()
}

#[cfg(test)]
#[path = "civvis_orders/city_route_tests.rs"]
mod city_route_tests;

/// `own-column-is-not-a-refusal`: an order that moves its unit off its tile.
fn moves_its_unit(order: &Order) -> bool {
    order.kind == "unit" && matches!(order.verb.as_deref(), Some("MOVE_TO" | "CAPTURE" | "SWAP"))
}

/// `own-column-is-not-a-refusal`: which of the units this frame's plan moved
/// had a move order cross to Firaxis (`sent`), and which were withheld by an
/// outbound filter (`unsent`). Mirror ids, the planner's own.
fn split_planned_movers<'a>(
    orders: &[Order],
    planned: impl Iterator<Item = (usize, &'a Action)>,
    civ6_of: &std::collections::BTreeMap<u32, i64>,
) -> (
    std::collections::BTreeSet<u32>,
    std::collections::BTreeSet<u32>,
) {
    let moved: std::collections::BTreeSet<i64> = orders
        .iter()
        .filter(|order| moves_its_unit(order))
        .filter_map(|order| order.subject)
        .collect();
    planned
        .filter(|(seat, _)| *seat == 0)
        .filter_map(|(_, action)| match action {
            Action::Move { unit, .. } | Action::MoveTo { unit, .. } | Action::Swap { unit, .. } => {
                Some(*unit)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<u32>>()
        .into_iter()
        .partition(|uid| civ6_of.get(uid).is_some_and(|civ6| moved.contains(civ6)))
}

/// `own-column-is-not-a-refusal`: each unit's opening walk as the planner
/// stepped it, before `coalesce_unit_paths_except` folds it into its last
/// hex -- every `MOVE_TO` from the unit's first order until anything else.
fn open_walk_steps(orders: &[Order]) -> std::collections::BTreeMap<i64, Vec<(i32, i32)>> {
    let mut walks: std::collections::BTreeMap<i64, (bool, Vec<(i32, i32)>)> =
        std::collections::BTreeMap::new();
    for order in orders.iter().filter(|order| order.kind == "unit") {
        let Some(subject) = order.subject else {
            continue;
        };
        let step = (order.verb.as_deref() == Some("MOVE_TO"))
            .then_some(order.pos)
            .flatten();
        let (open, steps) = walks.entry(subject).or_insert((true, Vec::new()));
        match step {
            Some(pos) if *open => steps.push(pos),
            _ => *open = false,
        }
    }
    walks
        .into_iter()
        .filter(|(_, (_, steps))| steps.len() >= 2)
        .map(|(subject, (_, steps))| (subject, steps))
        .collect()
}

/// `own-column-is-not-a-refusal`: a folded walk that ends on a hex another of
/// our units holds is cut back to the last hex of the same walk that is free.
///
/// Firaxis checks stacking where a move ENDS. The planning board walked the
/// column mate off that hex first, but the host may take the two orders the
/// other way round, and then the whole walk is refused (`move_no_path`) and
/// the unit loses the turn -- a column marching onto a staging ring stalls
/// on its own back. `held(subject, hex, index)` answers whether the hex holds,
/// at the frame's start, another of our units on the subject's stacking layer
/// that does not move off before the order at `index`. Returns how many walks
/// were cut back.
fn retarget_friendly_held_walks(
    orders: &mut [Order],
    walk_steps: &std::collections::BTreeMap<i64, Vec<(i32, i32)>>,
    held: impl Fn(i64, (i32, i32), usize) -> bool,
) -> usize {
    let mut retargeted = 0;
    for (index, order) in orders.iter_mut().enumerate() {
        if order.kind != "unit" || order.verb.as_deref() != Some("MOVE_TO") {
            continue;
        }
        let (Some(subject), Some(end)) = (order.subject, order.pos) else {
            continue;
        };
        let Some(steps) = walk_steps.get(&subject) else {
            continue;
        };
        if steps.last() != Some(&end) || !held(subject, end, index) {
            continue;
        }
        let free = steps[..steps.len() - 1]
            .iter()
            .rev()
            .find(|hex| !held(subject, **hex, index));
        if let Some(free) = free {
            order.pos = Some(*free);
            retargeted += 1;
        }
    }
    retargeted
}

/// [`retarget_friendly_held_walks`] against the frame's authoritative board:
/// our units where the host last exported them, and the order index at which
/// each first moves off its hex.
fn retarget_friendly_held_walks_on_board(
    orders: &mut [Order],
    walk_steps: &std::collections::BTreeMap<i64, Vec<(i32, i32)>>,
    mirror_state: &civvis::mirror::LiveMirror,
) -> usize {
    let game = &mirror_state.game;
    let mirror_of: std::collections::HashMap<i64, u32> = mirror_state
        .civ6_of
        .iter()
        .map(|(mirror, civ6)| (*civ6, *mirror))
        .collect();
    let mut standing: std::collections::HashMap<(i32, i32), Vec<u32>> =
        std::collections::HashMap::new();
    for (uid, unit) in game.units.iter().filter(|(_, unit)| unit.owner == 0) {
        standing
            .entry(civvis::hex::axial_to_offset(unit.pos.0, unit.pos.1))
            .or_default()
            .push(*uid);
    }
    let mut leaves_at: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for (index, order) in orders.iter().enumerate() {
        if let (true, Some(subject)) = (moves_its_unit(order), order.subject) {
            leaves_at.entry(subject).or_insert(index);
        }
    }
    let held = |subject: i64, hex: (i32, i32), index: usize| {
        let Some(me) = mirror_of.get(&subject) else {
            return false;
        };
        let here = game
            .units
            .get(me)
            .map(|unit| civvis::hex::axial_to_offset(unit.pos.0, unit.pos.1));
        if here == Some(hex) {
            // Its own hex: cutting a walk back to it is no walk at all.
            return true;
        }
        standing.get(&hex).is_some_and(|ids| {
            ids.iter().any(|other| {
                other != me
                    && game.units_share_stacking_layer(*me, *other)
                    && !mirror_state
                        .civ6_of
                        .get(other)
                        .and_then(|civ6| leaves_at.get(civ6))
                        .is_some_and(|at| *at < index)
            })
        })
    };
    retarget_friendly_held_walks(orders, walk_steps, held)
}

#[cfg(test)]
#[path = "civvis_orders/own_column_tests.rs"]
mod own_column_tests;

#[cfg(test)]
#[path = "civvis_orders/ranged_before_melee_tests.rs"]
mod ranged_before_melee_tests;

/// `ranged-before-melee`: within one frame's orders, a unit's RANGE_ATTACK on
/// a tile moves ahead of the first melee ATTACK on that tile that precedes it,
/// so the shot softens the target and the blow finishes it. 20 of 23
/// strike_frame_timeouts measured on October 5 were shots at a target a melee
/// blow earlier in the frame had already killed: live King
/// civvis-20261005T091120Z, turn 20, a Warrior killed the Horse Archer and the
/// Slinger's queued shot at it was dropped. A shot moves only when every
/// earlier order of its own unit already stands before the melee blow, so no
/// unit's own sequence changes. Returns how many shots moved.
fn ranged_before_melee(orders: &mut Vec<Order>) -> usize {
    let mut moved = 0;
    let mut index = 0;
    while index < orders.len() {
        let is_shot = orders[index].kind == "unit"
            && orders[index].verb.as_deref() == Some("RANGE_ATTACK")
            && orders[index].pos.is_some();
        if !is_shot {
            index += 1;
            continue;
        }
        let target = orders[index].pos;
        let shooter = orders[index].subject;
        let first_blow = orders[..index].iter().position(|order| {
            order.kind == "unit"
                && order.verb.as_deref() == Some("ATTACK")
                && order.pos == target
                && order.subject != shooter
        });
        let Some(blow) = first_blow else {
            index += 1;
            continue;
        };
        let own_after_blow = orders[blow..index]
            .iter()
            .any(|order| order.kind == "unit" && order.subject == shooter);
        if own_after_blow {
            index += 1;
            continue;
        }
        let shot = orders.remove(index);
        orders.insert(blow, shot);
        moved += 1;
        index += 1;
    }
    moved
}

fn coalesce_unit_paths_except(
    orders: Vec<Order>,
    sequenced: bool,
    local_routes: &std::collections::BTreeSet<i64>,
    capture_tiles: &std::collections::BTreeSet<(i32, i32)>,
) -> (Vec<Order>, usize, usize) {
    // Per unit: where its kept order sits in `out`, and whether that order is still
    // an open walk (every order for the unit so far has been a MOVE_TO).
    let mut kept: std::collections::BTreeMap<i64, (usize, bool)> =
        std::collections::BTreeMap::new();
    let mut out: Vec<Order> = Vec::with_capacity(orders.len());
    let mut deferred = 0;
    let mut coalesced = 0;
    for order in orders {
        if order.kind == "unit" && order.verb.as_deref() == Some("AIR_ATTACK") {
            // A preceding cavalry move can supply the aircraft's visibility.
            // Its later capture approach or retreat is a separate segment:
            // folding it into the opening MOVE_TO would erase the spotting
            // position before the sortie. Preserve that cross-unit boundary
            // even though each unit has its own native follow-up queue.
            for (_, open) in kept.values_mut() {
                *open = false;
            }
        }
        if order.kind != "unit" {
            out.push(order);
            continue;
        }
        let Some(subject) = order.subject else {
            out.push(order);
            continue;
        };
        let is_step = order.verb.as_deref() == Some("MOVE_TO") && order.pos.is_some();
        // A step onto an enemy city takes it; whatever the unit does next
        // must happen after the host has moved it in. See `capture_tiles`.
        let takes_city = is_step && order.pos.is_some_and(|pos| capture_tiles.contains(&pos));
        match kept.get_mut(&subject) {
            None => {
                kept.insert(subject, (out.len(), is_step && !takes_city));
                out.push(order);
            }
            Some((index, open)) => {
                if *open && is_step && !local_routes.contains(&subject) {
                    // The walk continues: the host only needs its last hex.
                    out[*index].pos = order.pos;
                    coalesced += 1;
                    if takes_city {
                        *open = false;
                    }
                } else if sequenced {
                    // The mod queues this behind the unit's earlier orders and
                    // issues it once they have settled. A move after an act is
                    // its own host order, not part of the opening walk.
                    *open = false;
                    out.push(order);
                } else {
                    // A command that must see the unit where the walk ends, or a
                    // move after such a command. Next frame.
                    *open = false;
                    deferred += 1;
                }
            }
        }
    }
    (out, deferred, coalesced)
}

/// Put the settler's SITE on every `FOUND_CITY` row: the hex the planned walk
/// leaves it on, or where it stands when it founds without moving.
///
/// ★★★★★ `UNITOPERATION_FOUND_CITY` FOUNDS WHERE THE SETTLER STANDS, and the
/// row carried no position at all. The mod runs every `FOUND_CITY` row
/// before the settler's own `MOVE_TO` (so a settler already on its site
/// founds without waiting) and then re-queues a refused found behind the
/// walk. Both are right when the found is refused off-site — but Civilization
/// VI refuses a found only where founding is ILLEGAL, and the hex one step
/// short of a chosen site is legal far more often than not (`CITY_MIN_RANGE`
/// is 3). Since the walk and the found ride the same turn (`order_queue`),
/// a planned "step, then settle" founded on the hex BEFORE the step whenever
/// that hex was legal, and a walk the host capped short of the site founded
/// on the capped hex once the settler arrived there with movement to spare.
/// With the site on the row the mod founds only when the settler stands on
/// it, and names the miss (`found_off_site`) instead of settling it.
///
/// Stamped BEFORE the walk is folded, so the last `MOVE_TO` of the unit's
/// contiguous walk is still in the list; a found with no preceding step this
/// frame takes the unit's exported position. Returns how many rows were
/// stamped.
fn stamp_found_sites(orders: &mut [Order], state: &civvis::mirror::StateSnapshot) -> usize {
    let mut standing: std::collections::BTreeMap<i64, (i32, i32)> = state
        .units
        .iter()
        .map(|unit| (unit.id, (unit.x, unit.y)))
        .collect();
    let mut stamped = 0;
    for order in orders.iter_mut() {
        if order.kind != "unit" {
            continue;
        }
        let Some(unit) = order.subject else {
            continue;
        };
        match order.verb.as_deref() {
            Some("MOVE_TO") => {
                if let Some(pos) = order.pos {
                    standing.insert(unit, pos);
                }
            }
            Some("FOUND_CITY") if order.pos.is_none() => {
                if let Some(&site) = standing.get(&unit) {
                    order.pos = Some(site);
                    stamped += 1;
                }
            }
            _ => {}
        }
    }
    stamped
}

/// Replay only: decide a recorded journal as a NEWER mod would present it.
/// `--assume-seat order_queue,moves_at_turn_start,replan_frames,tile_delta`
/// sets the named capabilities on every board read from the journal, so the
/// orders a turn would get from today's bridge can be censused on a run that
/// was played before those capabilities existed. Never used by the live
/// brain, which reads the seat the mod actually advertised.
fn assume_seat_capabilities(
    seat: &mut civvis::mirror::Seat,
    names: &[String],
) -> Result<(), String> {
    for name in names {
        let (key, value) = match name.split_once('=') {
            Some((key, value)) => (key.trim(), Some(value.trim())),
            None => (name.trim(), None),
        };
        match key {
            "order_queue" => seat.order_queue = true,
            "moves_at_turn_start" => seat.moves_at_turn_start = true,
            "tile_delta" => seat.tile_delta = true,
            "replan_frames" if value.is_none() => seat.replan_frames = true,
            "replan_frame_limit" => {
                seat.replan_frame_limit = Some(
                    value
                        .and_then(|text| text.parse().ok())
                        .ok_or("--assume-seat replan_frame_limit needs a nonnegative integer")?,
                );
            }
            "" => {}
            other => {
                return Err(format!(
                    "--assume-seat {other}: unknown capability (order_queue, \
                     moves_at_turn_start, replan_frames, replan_frame_limit=N, tile_delta)"
                ))
            }
        }
    }
    Ok(())
}

/// Unit orders that follow an earlier order for the same unit in this turn's
/// list — the ones the mod's per-unit queue will hold until that earlier order
/// has settled. Zero on an unsequenced turn by construction.
fn sequenced_unit_followups(orders: &[Order]) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    orders
        .iter()
        .filter(|order| order.kind == "unit")
        .filter_map(|order| order.subject)
        .filter(|unit| !seen.insert(*unit))
        .count()
}

/// The first unrevealed step in each walk that reaches the host boundary.
///
/// `coalesce_unit_paths` deliberately sends a unit's furthest planned hex so
/// Scouts, settlers and builders spend all of their movement. When the host
/// refuses that long route without moving at all, retaining only its terminal
/// hex gives the next mirror no useful terrain fact: the obstacle can be an
/// intermediate speculative step. Capture that first unknown step before the
/// command is compressed, but only for a contiguous opening walk — exactly the
/// prefix that coalescing will actually emit.
fn first_unknown_coalesced_steps(
    orders: &[Order],
    snapshot: &civvis::mirror::Snapshot,
) -> std::collections::BTreeMap<i64, (i32, i32)> {
    let mut open_walks: std::collections::BTreeMap<i64, bool> = std::collections::BTreeMap::new();
    let mut steps = std::collections::BTreeMap::new();
    for order in orders {
        if order.kind != "unit" {
            continue;
        }
        let Some(unit) = order.subject else {
            continue;
        };
        let step = (order.verb.as_deref() == Some("MOVE_TO"))
            .then_some(order.pos)
            .flatten();
        match open_walks.get_mut(&unit) {
            None => {
                open_walks.insert(unit, step.is_some());
                if let Some(pos) = step.filter(|pos| !snapshot.is_revealed(*pos)) {
                    steps.insert(unit, pos);
                }
            }
            Some(open) if *open => {
                if let Some(pos) = step.filter(|pos| !snapshot.is_revealed(*pos)) {
                    steps.entry(unit).or_insert(pos);
                }
                if step.is_none() {
                    *open = false;
                }
            }
            Some(_) => {}
        }
    }
    steps
}

/// Firaxis resolves Governor operations asynchronously. CIVVIS can spend several
/// titles in one simulated planning pass, but each later appointment/promotion was
/// chosen against the result of the prior one. Send only the first and re-plan from
/// the next authoritative host frame.
fn defer_governor_followups(orders: Vec<Order>) -> (Vec<Order>, usize) {
    let mut sent = false;
    let mut deferred = 0;
    let orders = orders
        .into_iter()
        .filter(|order| {
            if !order.kind.starts_with("governor_") {
                return true;
            }
            if sent {
                deferred += 1;
                false
            } else {
                sent = true;
                true
            }
        })
        .collect();
    (orders, deferred)
}

/// Resolve a CIVVIS city back to Firaxis's owner/id pair through its exact plot.
/// Firaxis city ids are only unique within a player, so id-only matching is unsafe.
fn host_city_target(
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    city: u32,
) -> Option<(i64, i32)> {
    let city = mirror_state.game.cities.get(&city)?;
    let (x, y) = civvis::hex::axial_to_offset(city.pos.0, city.pos.1);
    if let Some(host) = state.cities.iter().find(|host| (host.x, host.y) == (x, y)) {
        let owner = if state.seat.local_player >= 0 {
            state.seat.local_player
        } else {
            0
        };
        return Some((host.id, owner));
    }
    for rival in &state.rivals {
        if let Some(host) = rival.cities.iter().find(|host| (host.x, host.y) == (x, y)) {
            return Some((host.id, rival.player as i32));
        }
    }
    for minor in &state.minors {
        if let Some(host) = minor.cities.iter().find(|host| (host.x, host.y) == (x, y)) {
            return Some((host.id, minor.player as i32));
        }
    }
    None
}

/// Resolve a CIVVIS player seat back to the Firaxis player id the order channel uses.
///
/// Major civilizations occupy the model's stable major seats, while city-states
/// live in the mirror's later minor seats and are only exported through
/// `state.minors`. Resolve those through an exact city plot rather than assuming
/// a generated city-state roster has the host's player numbering.
fn host_player_target(
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    player: usize,
) -> Option<i64> {
    civvis::mirror::host_rival_for_seat(
        state,
        player,
        civvis::mirror::modeled_major_player_count(&mirror_state.game),
    )
    .map(|rival| rival.player as i64)
    .or_else(|| {
        mirror_state
            .game
            .cities
            .values()
            .filter(|city| city.owner == player)
            .find_map(|city| {
                host_city_target(mirror_state, state, city.id).map(|(_, owner)| owner as i64)
            })
    })
}

/// Resolve a mirrored city-state SEAT back to Firaxis's minor player id.
///
/// By the mirror's own seating rule (`minor_actor_assignments`: met city-states
/// take the board's minor seats in export order), not through a city plot —
/// a city-state met by a scout's contact whose centre the fog has not shown
/// yet has no mirrored city to look up, and an envoy to it is exactly the
/// order that must still cross (its first-meet envoy is already standing
/// there).
fn host_minor_target(
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    player: usize,
) -> Option<i64> {
    civvis::mirror::minor_actor_assignments(&mirror_state.game, state)
        .into_iter()
        .find(|(minor, seat)| *seat == player && minor.is_city_state())
        .map(|(minor, _)| minor.player as i64)
}

// ★★★★★ SURPLUS SOLD, NOT HOARDED. CIVVIS's planner already decides these
// sales — `Game::quick_deals` quotes a duplicate luxury copy to a rival that
// lacks it, a strategic block, a Great Work, a favor block — and every one of
// them fell through `translate`'s `_ => None`: **17 `Trade` skips in run
// civvis-20260818T083142Z alone** (dyes ×5, turtles ×2, a favor block, and the
// buys), while the seat ended that game holding 423 unspent favor and a
// stockpile pinned at the 25 cap on four strategics for a hundred turns. A
// sale is one host deal: the Lua side puts exactly these items in the outgoing
// working deal, asks the rival's own valuation for the gold with
// `DealProposalAction.EQUALIZE`, and accepts the answer only at or above the
// floor carried here. Nothing on this side prices what the rival will pay; it
// says what CIVVIS is willing to let go and what it would have taken.
//
// Only SALES cross: our side resources and/or favor, theirs gold. A purchase
// (their luxury for our gold), a Great Work, a city, a captive or Open Borders
// is a different risk class with a different validation and stays in the
// skipped tally, named, until it earns its own arm.
/// The verb a `sell` order carries: `RESOURCE_DYES=1,FAVOR=10` — Firaxis's own
/// resource type names (the mirror's `strategic_resource` import is the exact
/// inverse) with the favor block last. `None` when the offer holds anything
/// the arm does not sell, or nothing at all.
fn sale_verb(offer: &civvis::game::DealItems) -> Option<String> {
    if offer.gold != 0.0
        || offer.gold_per_turn != 0.0
        || !offer.great_works.is_empty()
        || !offer.captured_spies.is_empty()
        || !offer.cities.is_empty()
        || offer.open_borders
        || !offer.diplomatic_favor.is_finite()
        || offer.diplomatic_favor < 0.0
    {
        return None;
    }
    let mut parts: Vec<String> = offer
        .resources
        .iter()
        .filter(|(_, amount)| **amount > 0)
        .map(|(resource, amount)| format!("RESOURCE_{}={amount}", resource.to_ascii_uppercase()))
        .collect();
    let favor = offer.diplomatic_favor.floor() as i64;
    if favor > 0 {
        parts.push(format!("FAVOR={favor}"));
    }
    (!parts.is_empty()).then(|| parts.join(","))
}

/// The gold-equivalent below which the host must NOT close the sale: what
/// the asset is worth to US by the engine's own book — the ask (lump gold
/// plus per-turn gold at the 25× factor of `Game::receive_items_value`) less
/// our net gain on the quote, i.e. `give_items_cost` — never below
/// `SALE_FLOOR_MIN`. ★ Until 2026-08-24 this was HALF the ask: a surplus copy
/// the book priced at 90 was let go for 45, on the argument that the rival
/// prices by its own book. It does — and the floor is still the price below
/// which the sale loses us value, which no discount changes. (Whether a
/// rival ever meets it is what the ledger's `deal_response` now records.)
const SALE_FLOOR_MIN: i32 = 10;

fn sale_floor(request: &civvis::game::DealItems, my_value: f64) -> Option<i32> {
    if !request.resources.is_empty()
        || request.diplomatic_favor != 0.0
        || !request.great_works.is_empty()
        || !request.captured_spies.is_empty()
        || !request.cities.is_empty()
        || request.open_borders
        || !request.gold.is_finite()
        || !request.gold_per_turn.is_finite()
        || request.gold < 0.0
        || request.gold_per_turn < 0.0
    {
        return None;
    }
    let asked = request.gold + 25.0 * request.gold_per_turn;
    let walk_away = if my_value.is_finite() {
        asked - my_value
    } else {
        asked
    };
    Some((walk_away.ceil() as i32).max(SALE_FLOOR_MIN))
}

/// The gold-equivalent ABOVE which the host must NOT close a passage
/// purchase: what the passage is worth to US by the engine's own book
/// (`Game::passage_gold_value`; the offer when the book is silent) — never
/// above `BORDER_BUY_CEILING_MAX`. ★ Until 2026-08-24 this was the offer and half
/// again, "headroom because the rival prices by its own book"; a purchase
/// above what the passage is worth to us is a loss whatever the rival's
/// book says. `None` unless the trade is exactly the shape this arm buys:
/// their Open Borders, our gold, nothing else on either side.
fn border_buy_ceiling(
    offer: &civvis::game::DealItems,
    request: &civvis::game::DealItems,
    worth: f64,
) -> Option<i32> {
    if !request.open_borders
        || request.gold != 0.0
        || request.gold_per_turn != 0.0
        || !request.resources.is_empty()
        || request.diplomatic_favor != 0.0
        || !request.great_works.is_empty()
        || !request.captured_spies.is_empty()
        || !request.cities.is_empty()
        || offer.open_borders
        || !offer.resources.is_empty()
        || offer.diplomatic_favor != 0.0
        || !offer.great_works.is_empty()
        || !offer.captured_spies.is_empty()
        || !offer.cities.is_empty()
        || !offer.gold.is_finite()
        || !offer.gold_per_turn.is_finite()
        || offer.gold < 0.0
        || offer.gold_per_turn < 0.0
    {
        return None;
    }
    let offered = offer.gold + 25.0 * offer.gold_per_turn;
    if offered <= 0.0 {
        return None;
    }
    let worth = if worth.is_finite() && worth > 0.0 {
        worth
    } else {
        offered
    };
    Some((worth.ceil() as i32).clamp(1, BORDER_BUY_CEILING_MAX))
}

// ★★★★★ PASSAGE BOUGHT WHERE THE MAP IS SEALED. `Game::sealed_border_owners`
// names the major seats whose fogged border the mirror must seal
// (`closed_borders`) and how much ground each one accounts for. That seal is
// the measured killer of exploration: one live run held a scout against
// Kongo's invisible border for 74 turns, exploration flatlined at 8.3% of the
// map, no rival city was ever observed, and a conquest plan ran forty turns
// with no one to attack. Sealed ground is also the self-fulfilling half of
// the settler veto — the ground stays unexplored because it is never entered.
//
// Open Borders is the peacetime key to that door, and the host sells it as an
// ordinary deal. The engine's own quote lane (`quick_deals`) cannot decide
// this on a mirrored board — it validates the PARTNER's Early Empire civic,
// which the mirror does not model — so, like the favor sale above, the order
// is composed here from the export's own facts: our civic list, our treasury,
// the rival's exported war/borders state. The Lua arm asks the rival's own
// price with EQUALIZE and closes only at or under the ceiling carried in `x`.
//
// One ask per cadence window, aimed at the border an actual Scout or naval
// explorer can use to push into remaining fog. A large, distant border is not
// a reason to spend Gold while our explorers are working another frontier.
// The mod's own cooldowns (`TradeRetryTurns`) meter re-asks after a refusal,
// and the export's `open_borders` flag retires the trigger the moment the grant
// exists — the mirror stops sealing that rival, `sealed_border_owners` drops
// them, and this lane moves to the next useful passage or goes quiet.
const BORDER_BUY_MIN_SEALED: u32 = 6;
const BORDER_BUY_GOLD_RESERVE: i64 = 60;
const BORDER_BUY_CEILING_MIN: i32 = 30;
const BORDER_BUY_CEILING_MAX: i32 = 180;
const BORDER_BUY_CADENCE: u32 = 6;
const BORDER_BUY_PHASE: u32 = 1;
const BORDER_BUY_EXPLORER_APPROACH: i32 = 4;
/// `siege-buys-the-passage`: the Gold a siege's passage is worth to the
/// purchase, under `BORDER_BUY_CEILING_MAX`; a train held short of closed
/// borders for 4-37 turns costs more than this in lost turns.
const SIEGE_PASSAGE_GOLD: f64 = 150.0;
/// `siege-buys-the-passage`: the explorer count a siege's blocker is ranked
/// at, so its passage comes before any exploration gate.
const SIEGE_PASSAGE_PRIORITY: usize = 1_000;

/// `siege-buys-the-passage`: rank each major whose closed borders hold a
/// siege march ahead of every exploration gate, whether or not an explorer
/// stands by it.
fn add_siege_passage_targets(
    targets: &mut std::collections::BTreeMap<usize, BorderExplorationPriority>,
    blockers: &std::collections::BTreeSet<usize>,
) {
    for seat in blockers {
        let priority = targets.entry(*seat).or_default();
        priority.explorers = priority.explorers.max(SIEGE_PASSAGE_PRIORITY);
    }
}
const BORDER_BUY_FRONTIER_RADIUS: i32 = 5;

/// The evidence that one rival's closed border is a useful exploration gate.
/// `explorers` wins first: a passage an existing eye can use now is more useful
/// than a larger but remote seal. `frontier_tiles` then distinguishes the two
/// routes available to that eye without inspecting hidden terrain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BorderExplorationPriority {
    explorers: usize,
    frontier_tiles: usize,
}

/// Whether the host's known board still has a discovery job. The map reading
/// is enough for older exports; current seat metadata additionally names the
/// two things a Scout is trying to find: uncontacted major civilizations and
/// city-states.
fn exploration_still_matters(
    snapshot: &civvis::mirror::Snapshot,
    state: &civvis::mirror::StateSnapshot,
) -> bool {
    let map_tiles = i64::from(snapshot.width.max(0)) * i64::from(snapshot.height.max(0));
    let world_unexplored = map_tiles > 0 && (snapshot.revealed_count() as i64) < map_tiles;
    let unmet_major =
        state.seat.players > 0 && state.rivals.len().saturating_add(1) < state.seat.players;
    let met_city_states = state
        .minors
        .iter()
        .filter(|minor| minor.is_city_state())
        .count();
    let unmet_city_state = state.seat.city_states > met_city_states;
    world_unexplored || unmet_major || unmet_city_state
}

/// Scouts and naval military units are the bodies that turn a bought passage
/// into map knowledge. A warrior marching with an army is deliberately not an
/// exploration signal: buying its way through a neutral rival's land belongs
/// to a military plan, not this peacetime scouting lane.
fn is_border_explorer(game: &civvis::game::Game, uid: u32) -> bool {
    let Some(unit) = game.units.get(&uid) else {
        return false;
    };
    let spec = &game.rules.units[unit.kind];
    unit.owner == 0
        && spec.class == "military"
        && spec.domain.as_deref() != Some("air")
        && (spec.promotion_class == "recon" || spec.domain.as_deref() == Some("sea"))
}

/// Is this revealed border close enough to a still-unknown part of the actual
/// host map to be a credible route of discovery? This consults only the
/// snapshot's revealed set, never the reconstruction's invented fog terrain.
fn border_leads_to_fog(
    snapshot: &civvis::mirror::Snapshot,
    game: &civvis::game::Game,
    border: civvis::Pos,
) -> bool {
    game.wdisk(border, BORDER_BUY_FRONTIER_RADIUS)
        .into_iter()
        .any(|pos| {
            let (x, y) = civvis::hex::axial_to_offset(pos.0, pos.1);
            x >= 0
                && x < snapshot.width
                && y >= 0
                && y < snapshot.height
                && !snapshot.is_revealed((x, y))
        })
}

/// Score the sealed major borders an actual explorer can reach. The snapshot
/// owns player ids, while `sealed_by` owns mirror seats, so resolve each once
/// through the same rival ordering used by the live mirror.
fn border_buy_exploration_targets(
    snapshot: &civvis::mirror::Snapshot,
    state: &civvis::mirror::StateSnapshot,
    game: &civvis::game::Game,
    sealed_by: &std::collections::BTreeMap<usize, u32>,
) -> std::collections::BTreeMap<usize, BorderExplorationPriority> {
    if !exploration_still_matters(snapshot, state) {
        return Default::default();
    }
    let explorers: Vec<u32> = game
        .units
        .iter()
        .filter_map(|(uid, _)| is_border_explorer(game, *uid).then_some(*uid))
        .collect();
    if explorers.is_empty() {
        return Default::default();
    }
    let seats_by_host = civvis::mirror::host_major_seat_map(
        state,
        civvis::mirror::modeled_major_player_count(game),
    );
    let mut targets: std::collections::BTreeMap<
        usize,
        (
            std::collections::BTreeSet<u32>,
            std::collections::BTreeSet<civvis::Pos>,
        ),
    > = Default::default();
    for (x, y) in snapshot.revealed_positions() {
        let Some(plot) = snapshot.plot((x, y)) else {
            continue;
        };
        let Ok(owner) = usize::try_from(plot.o) else {
            continue;
        };
        let Some(&seat) = seats_by_host.get(&owner) else {
            continue;
        };
        if sealed_by.get(&seat).copied().unwrap_or_default() < BORDER_BUY_MIN_SEALED {
            continue;
        }
        let border = civvis::hex::offset_to_axial(x, y);
        if !border_leads_to_fog(snapshot, game, border) {
            continue;
        }
        for uid in &explorers {
            let unit = &game.units[uid];
            if game.wdist(unit.pos, border) <= BORDER_BUY_EXPLORER_APPROACH
                && game.unit_can_traverse(*uid, border)
            {
                let (eyes, frontier) = targets.entry(seat).or_default();
                eyes.insert(*uid);
                frontier.insert(border);
            }
        }
    }
    targets
        .into_iter()
        .map(|(seat, (eyes, frontier))| {
            (
                seat,
                BorderExplorationPriority {
                    explorers: eyes.len(),
                    frontier_tiles: frontier.len(),
                },
            )
        })
        .collect()
}

/// Why no passage-purchase order was appended this turn, for the note;
/// `None` when one was. `sealed_by` is the mirrored board's
/// `sealed_border_owners`; rival records are looked up by the mirror's own
/// stable host-id seating rule (the same mapping `host_player_target` uses).
fn append_border_buy_order(
    sealed_by: &std::collections::BTreeMap<usize, u32>,
    exploration: &std::collections::BTreeMap<usize, BorderExplorationPriority>,
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    passage_value: &dyn Fn(usize) -> f64,
) -> Option<&'static str> {
    let has_sealable_border = sealed_by.iter().any(|(seat, count)| {
        *count >= BORDER_BUY_MIN_SEALED
            && civvis::mirror::host_rival_for_seat(state, *seat, 0)
                .is_some_and(|rival| !rival.at_war && rival.open_borders != Some(true))
    });
    let worst = sealed_by
        .iter()
        .filter(|(_, count)| **count >= BORDER_BUY_MIN_SEALED)
        .filter_map(|(seat, count)| {
            civvis::mirror::host_rival_for_seat(state, *seat, 0).and_then(|rival| {
                exploration
                    .get(seat)
                    .map(|priority| (*seat, rival, *count, *priority))
            })
        })
        .filter(|(_, rival, _, _)| !rival.at_war && rival.open_borders != Some(true))
        .max_by_key(|(_, rival, count, priority)| {
            (
                priority.explorers,
                priority.frontier_tiles,
                *count,
                std::cmp::Reverse(rival.player),
            )
        });
    let Some((seat, rival, _, priority)) = worst else {
        return Some(if has_sealable_border {
            "border_buy_hold:no_exploration_need"
        } else {
            "border_buy_hold:no_seal"
        });
    };
    // `siege-buys-the-passage`: a siege's passage is asked on any turn; the
    // mod's own retry cooldown still meters re-asks. A train holds short of
    // closed borders a median 4.5 turns, under the six-turn cadence: live
    // Emperor civvis-20261007T074538Z held short of Macedon's borders turns
    // 177-180, and the one phase turn in that run came when the treasury
    // read 60.
    if priority.explorers < SIEGE_PASSAGE_PRIORITY
        && state.turn % BORDER_BUY_CADENCE != BORDER_BUY_PHASE
    {
        return Some("border_buy_hold:cadence");
    }
    // The host refuses the agreement item itself without Early Empire on both
    // sides; ours is in the export, theirs only the deal validation knows.
    // Asking early costs a named `agreement_invalid` refusal, asking without
    // our own civic is a guaranteed one — skip only the guaranteed case.
    if !state
        .civics
        .iter()
        .any(|civic| civic == "CIVIC_EARLY_EMPIRE")
    {
        return Some("border_buy_hold:no_civic");
    }
    // ★ Until 2026-08-24 the ceiling was the treasury less the reserve — the
    // price was what we could afford, not what passage is worth. It is now
    // the engine's own book for this passage (`Game::passage_gold_value`),
    // still bounded by the treasury and the cap.
    let book = passage_value(seat);
    let affordable =
        (state.gold - BORDER_BUY_GOLD_RESERVE).min(BORDER_BUY_CEILING_MAX as i64) as i32;
    let ceiling = if book.is_finite() && book > 0.0 {
        (book.ceil() as i32).min(affordable)
    } else {
        affordable
    };
    if ceiling < BORDER_BUY_CEILING_MIN {
        return Some("border_buy_hold:treasury");
    }
    // One working deal per rival at a time is the mod's rule; a sale already
    // heading to the same seat this turn would turn the buy into a named
    // `buy_pending` refusal. Yield the turn — the cadence retries.
    if orders.iter().any(|order| {
        (order.kind == "sell" || order.kind == "buy") && order.subject == Some(rival.player as i64)
    }) {
        return Some("border_buy_hold:deal_in_flight");
    }
    orders.push(Order {
        kind: "buy",
        subject: Some(rival.player as i64),
        verb: Some("OPEN_BORDERS".to_string()),
        pos: Some((ceiling, 0)),
    });
    None
}

// ★★★★★ THE LUXURY THE SEAT LACKS, BOUGHT FOR GOLD. The live seat runs
// Displeased on 48% of its city-turns — Firaxis takes 10% off every non-food
// yield there, ≈1,150 yield points over a game — and luxuries are 60% of the
// amenities it does have. Rivals visibly hold 2–8 improved luxuries the seat
// holds none of, the host sells a copy as an ordinary deal, and in the nine
// recorded sales the AI asked 2–14 Gold a turn per copy; one copy is +1
// Amenity in four cities. `Game::quick_deals` does quote a luxury PURCHASE,
// but its result is not enough to choose a seller; the mirror now carries the
// host's met-gated tradeable luxury catalogue and `border_buy_ceiling` rightly
// refuses the shape, so — like the favor sale and the passage purchase — the
// order is composed here from the export's own facts: the
// amenity ledger the host reports per city, the net income, the treasury,
// the rivals' war state and gold. The Lua arm reads the RIVAL's own
// tradeable list, takes the first luxury the host says the seat holds none
// of, asks the rival's own price with EQUALIZE and closes only at or under
// the ceiling carried in `x`.
//
// Cadence-gated like its neighbours (phase 4, the last free slot of the
// six-turn deal week), aimed at the richest met major at peace that the host
// says has at least one tradeable luxury, with no deal already heading its
// way this turn; the mod's own cooldowns
// (`TradeRetryTurns`) meter re-asks after a `buy_no_luxury`. The ceiling is
// what a copy is worth to the empire — a base and a share per city, since
// the four cities it reaches each recover the 10% — bounded by what the
// income carries at the 25× book with `LUXURY_BUY_INCOME_FLOOR` Gold a turn
// kept clear (every recorded ask was per-turn), and by the cap. A ceiling
// under `LUXURY_BUY_CEILING_MIN` — two Gold a turn, the cheapest ask on
// record — is not worth a deal window.
const LUXURY_BUY_MIN_DEFICIT: f64 = 2.0;
const LUXURY_BUY_EMERGENCY_DEFICIT: f64 = 4.0;
const LUXURY_BUY_GOLD_RESERVE: i64 = 60;
const LUXURY_BUY_INCOME_FLOOR: f64 = 4.0;
const LUXURY_BUY_CEILING_BASE: i32 = 135;
const LUXURY_BUY_CEILING_PER_CITY: i32 = 10;
const LUXURY_BUY_CEILING_MIN: i32 = 50;
const LUXURY_BUY_CEILING_MAX: i32 = 300;
const LUXURY_BUY_CADENCE: u32 = 6;
const LUXURY_BUY_PHASE: u32 = 4;

/// How many amenities the empire is short by the host's own ledger: the sum
/// over cities of `needed − have` where positive. A city whose ledger the
/// host did not report (`NaN` or `-1`, see `StateCity::amenities`) is left
/// out rather than read as content or as starving.
/// `captives-come-home`: Gold the treasury keeps through a ransom: upkeep and the purchase lanes
/// (luxury top-ups, the upgrade floor) are not spent on a Spy.
const RANSOM_GOLD_RESERVE: i64 = 150;
/// The most a Spy is worth back: under a new one's Production at the turns
/// the network empties (225-262), and its experience comes with it.
const RANSOM_CEILING_MAX: i32 = 200;
/// Below this the treasury cannot meet any likely price; no ask goes out.
const RANSOM_CEILING_MIN: i32 = 40;
/// One ask per captor every this many turns, staggered by the captor's seat,
/// so an unanswered deal session cannot stand the trade lane down.
const RANSOM_CADENCE: u32 = 5;
/// `captives-come-home-2`: Favor the seat keeps through a ransom.
const RANSOM_FAVOR_RESERVE: i64 = 30;
/// The most Favor one captive is offered. The AI took 38 Favor for 9 Gold a
/// turn (225 on the book) in live sales, about 6 Gold a point, and named 474
/// Gold for a captive: some 80 points.
const RANSOM_FAVOR_CAP: i64 = 150;
/// Below this no Favor is offered (the ask falls back to Gold alone).
const RANSOM_FAVOR_MIN: i64 = 20;
/// While the bank is held for Congress (`favor_banked_for_votes`: a rival at
/// [`FAVOR_HOLD_DVP`] or more, which on the live Deity lane is every turn
/// past 100), this much Favor stays for the votes.
const RANSOM_FAVOR_VOTE_RESERVE: i64 = 120;
/// A rival this near a Diplomatic Victory keeps the whole bank for Congress.
const RANSOM_FAVOR_DVP_CEILING: i64 = 15;

/// `captives-come-home`: ask one captor at peace with us for one of our
/// captured Spies (`buy CAPTIVE`), Gold up to the ceiling. The captor is the
/// one holding most of ours whose staggered [`RANSOM_CADENCE`] turn this is
/// (`captives_of_ours`, the mod's read of the shipped deal screen); the
/// ceiling is the treasury over the reserve, capped. `None` when an ask was
/// pushed, else the hold. The caller decides whether the network wants its
/// slots (`AdvancedAi::captives_come_home_wanted`). See
/// `advanced/captives_come_home.rs`.
///
/// `captives-come-home-2`: `favor` puts that much Diplomatic Favor on our side
/// too (`CAPTIVE;FAVOR=n`), so the price can be met where the Gold alone
/// cannot; with Favor offered, a treasury under the Gold floor asks anyway
/// at whatever Gold it can add.
fn append_captive_ransom_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    favor: Option<u32>,
) -> Option<&'static str> {
    let held = |rival: &civvis::mirror::StateRival| rival.captives_of_ours.unwrap_or(0);
    let captor = state
        .rivals
        .iter()
        .filter(|rival| held(rival) > 0 && !rival.at_war)
        .filter(|rival| state.turn % RANSOM_CADENCE == rival.player as u32 % RANSOM_CADENCE)
        .max_by_key(|rival| (held(rival), std::cmp::Reverse(rival.player)));
    let Some(captor) = captor else {
        return Some("ransom_hold:no_captor");
    };
    let ceiling = (state.gold - RANSOM_GOLD_RESERVE)
        .min(RANSOM_CEILING_MAX as i64)
        .max(0) as i32;
    if ceiling < RANSOM_CEILING_MIN && favor.is_none() {
        return Some("ransom_hold:treasury");
    }
    // One working deal per rival at a time is the mod's rule.
    if orders.iter().any(|order| {
        (order.kind == "sell" || order.kind == "buy") && order.subject == Some(captor.player as i64)
    }) {
        return Some("ransom_hold:deal_in_flight");
    }
    orders.push(Order {
        kind: "buy",
        subject: Some(captor.player as i64),
        verb: Some(match favor {
            Some(points) => format!("CAPTIVE;FAVOR={points}"),
            None => "CAPTIVE".to_string(),
        }),
        pos: Some((ceiling, 0)),
    });
    None
}

/// `tribute-buys-the-friendship`: Gold the treasury keeps through a tribute.
const TRIBUTE_GOLD_RESERVE: i64 = 100;
/// The smallest tribute sent: a rival a point short still gets a gift it
/// notices.
const TRIBUTE_MIN_GOLD: i64 = 30;
/// The visible opinion a tribute aims for: the median total of a rival the
/// host reads as Friendly on the live Deity lane (+4), plus a point.
const TRIBUTE_TARGET_OPINION: f64 = 5.0;
/// Gold per point of a favorable deal (`TradeValuePerModifierPoint`, shipped
/// `Leaders.xml` `STANDARD_DIPLOMATIC_ONE_SIDED_TRADES`).
const TRIBUTE_GOLD_PER_POINT: f64 = 10.0;
/// The most one gift has been seen to buy. The row's `MaxValue` is 20, but on
/// the first live carriers 80, 101 and 139 Gold bought about +9, +9 and +10,
/// and a further 80 or 120 Gold to the same rival about +2. A gap wider than
/// this is out of reach, and paying toward it bought nothing: seven such gifts
/// (789 Gold) warmed no rival.
const TRIBUTE_ONE_GIFT_POINTS: f64 = 10.0;
/// One tribute per rival every this many turns, staggered by its seat. The
/// mod holds a rival to one gift per `TributeRetryTurns`; see `aid_gift`.
const TRIBUTE_CADENCE: u32 = 5;
/// The Intel tab's row for that modifier, as the English host reports it. A
/// standing row means a gift already paid. Below the visibility that names it
/// the row reads "Unknown Reason"; the mod's retry bar covers that case.
const TRIBUTE_FAVORABLE_TRADE_ROW: &str = "Favorable trade deals";

/// `tribute-buys-the-friendship`: a one-way Gold gift (`aid_gift` verb
/// `TRIBUTE`) to the strongest met major at peace with us, the rival
/// `befriend-the-strongest` courts, while it is stronger than us and reads
/// Unfriendly or Neutral. The amount is the opinion gap to
/// [`TRIBUTE_TARGET_OPINION`] at [`TRIBUTE_GOLD_PER_POINT`], sent whole or not
/// at all: only a gap one gift can close ([`TRIBUTE_ONE_GIFT_POINTS`]), only
/// to a rival not already holding one, and only from the treasury over
/// [`TRIBUTE_GOLD_RESERVE`]. `None` when a gift was pushed, else the hold.
/// See `advanced/tribute_buys_the_friendship.rs`.
fn append_tribute_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
) -> Option<&'static str> {
    let Some(rival) = state
        .rivals
        .iter()
        .filter(|rival| !rival.at_war)
        .max_by(|a, b| {
            a.military
                .total_cmp(&b.military)
                .then(b.player.cmp(&a.player))
        })
    else {
        return Some("tribute_hold:no_rival");
    };
    let mood = rival.diplomatic_state.as_deref().unwrap_or("");
    if !matches!(mood, "DIPLO_STATE_UNFRIENDLY" | "DIPLO_STATE_NEUTRAL") {
        return Some(if mood == "DIPLO_STATE_DENOUNCED" {
            "tribute_hold:denounced"
        } else {
            "tribute_hold:courted"
        });
    }
    if rival.military <= state.military {
        return Some("tribute_hold:not_stronger");
    }
    if state.turn % TRIBUTE_CADENCE != rival.player as u32 % TRIBUTE_CADENCE {
        return Some("tribute_hold:cadence");
    }
    let Some(reasons) = rival.opinion_modifiers.as_ref() else {
        return Some("tribute_hold:opinion_unknown");
    };
    let total: f64 = reasons.iter().map(|reason| reason.score).sum();
    let favorable: f64 = reasons
        .iter()
        .filter(|reason| reason.text.starts_with(TRIBUTE_FAVORABLE_TRADE_ROW))
        .map(|reason| reason.score.max(0.0))
        .sum();
    if favorable > 0.0 {
        return Some("tribute_hold:already_paid");
    }
    let points = TRIBUTE_TARGET_OPINION - total;
    if points <= 0.0 {
        return Some("tribute_hold:no_gap");
    }
    if points > TRIBUTE_ONE_GIFT_POINTS {
        return Some("tribute_hold:out_of_reach");
    }
    let amount = ((points.ceil() * TRIBUTE_GOLD_PER_POINT) as i64).max(TRIBUTE_MIN_GOLD);
    if amount > state.gold - TRIBUTE_GOLD_RESERVE {
        return Some("tribute_hold:treasury");
    }
    // One working deal per rival, and a tribute beside a war, peace,
    // denunciation, friendship or alliance order to the same rival would be
    // read against the wrong diplomatic state.
    let subject = rival.player as i64;
    if orders.iter().any(|order| {
        order.subject == Some(subject)
            && matches!(
                order.kind,
                "sell"
                    | "buy"
                    | "aid_gift"
                    | "war"
                    | "peace"
                    | "delegation"
                    | "denounce"
                    | "friendship"
                    | "alliance"
            )
    }) {
        return Some("tribute_hold:deal_in_flight");
    }
    orders.push(Order {
        kind: "aid_gift",
        subject: Some(subject),
        verb: Some("TRIBUTE".to_string()),
        pos: Some((amount as i32, 0)),
    });
    None
}

fn amenity_deficit(state: &civvis::mirror::StateSnapshot) -> f64 {
    state
        .cities
        .iter()
        .filter(|city| city.amenities >= 0.0 && city.amenities_needed >= 0.0)
        .map(|city| (city.amenities_needed - city.amenities).max(0.0))
        .sum()
}

/// Why no luxury-purchase order was appended this turn, for the note; `None`
/// when one was.
fn append_luxury_buy_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
) -> Option<&'static str> {
    let deficit = amenity_deficit(state);
    if deficit < LUXURY_BUY_MIN_DEFICIT {
        return Some("luxury_buy_hold:content");
    }
    if state.turn % LUXURY_BUY_CADENCE != LUXURY_BUY_PHASE {
        return Some("luxury_buy_hold:cadence");
    }
    let Some(income) = state.gold_per_turn.filter(|income| income.is_finite()) else {
        return Some("luxury_buy_hold:income_unknown");
    };
    let income_carried = (25.0 * (income - LUXURY_BUY_INCOME_FLOOR))
        .floor()
        .clamp(0.0, LUXURY_BUY_CEILING_MAX as f64) as i32;
    // A severe amenity deficit can itself push income below the ordinary
    // safety floor through the host's yield penalty. Preserve that floor for
    // ordinary shortages, but let the treasury carry one emergency luxury
    // ask when four or more amenities are missing and the reserve can survive
    // it. The ask remains bounded by the normal ceiling.
    let carried = if deficit >= LUXURY_BUY_EMERGENCY_DEFICIT {
        income_carried.max(
            (state.gold - LUXURY_BUY_GOLD_RESERVE)
                .max(0)
                .min(LUXURY_BUY_CEILING_MAX as i64) as i32,
        )
    } else {
        income_carried
    };
    let worth = LUXURY_BUY_CEILING_BASE
        + LUXURY_BUY_CEILING_PER_CITY * state.cities.len().min(i32::MAX as usize) as i32;
    let ceiling = worth.min(LUXURY_BUY_CEILING_MAX).min(carried);
    if ceiling < LUXURY_BUY_CEILING_MIN {
        return Some("luxury_buy_hold:income");
    }
    if state.gold < LUXURY_BUY_GOLD_RESERVE {
        return Some("luxury_buy_hold:treasury");
    }
    // One working deal per rival at a time is the mod's rule; a rival with a
    // sale or purchase already heading its way this turn is passed over for
    // the next-richest, not held against.
    let seller = state
        .rivals
        .iter()
        .filter(|rival| !rival.at_war)
        .filter(|rival| {
            rival
                .tradeable_luxuries
                .as_ref()
                .is_some_and(|luxuries| !luxuries.is_empty())
        })
        .filter(|rival| {
            !orders.iter().any(|order| {
                (order.kind == "sell" || order.kind == "buy")
                    && order.subject == Some(rival.player as i64)
            })
        })
        .max_by(|left, right| {
            let gold = |rival: &civvis::mirror::StateRival| {
                if rival.gold.is_finite() {
                    rival.gold
                } else {
                    0.0
                }
            };
            gold(left)
                .partial_cmp(&gold(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.player.cmp(&left.player))
        });
    let Some(seller) = seller else {
        let mut peaceful_rivals = state.rivals.iter().filter(|rival| !rival.at_war);
        let has_peaceful_rival = peaceful_rivals.clone().next().is_some();
        let all_catalogues_known = has_peaceful_rival
            && peaceful_rivals
                .clone()
                .all(|rival| rival.tradeable_luxuries.is_some());
        let known_stock = peaceful_rivals.any(|rival| {
            rival
                .tradeable_luxuries
                .as_ref()
                .is_some_and(|luxuries| !luxuries.is_empty())
        });
        return Some(if all_catalogues_known && !known_stock {
            "luxury_buy_hold:no_stock"
        } else {
            "luxury_buy_hold:no_seller"
        });
    };
    orders.push(Order {
        kind: "buy",
        subject: Some(seller.player as i64),
        verb: Some("LUXURY_ANY".to_string()),
        pos: Some((ceiling, 0)),
    });
    None
}

// ★★★★ THE SPARE COPY, SWAPPED FOR A LUXURY THE SEAT LACKS
// (`luxury-swap-asks`). From turn 75 on, 56% of the live seat's city-turns
// ran Displeased (31 Emperor games of 2026-10-08, 20,432 city-turns) — the
// host's own 10% off every non-food yield and 15% off growth, 6.2% of all
// production from turn 75 to 150 — and 84% of those cities were exactly ONE
// Amenity short, which one more luxury type (+1 in four cities) closes. At
// turn 100 the median empire held 4 types and 2 spare copies, its peaceful
// rivals offered 4 types it lacked, and 26 of 32 games had both a spare and
// an offer. Gold alone never bought a copy (`luxury_buy_asks`: 644 asks, every
// one refused), while the rivals bought our spares for Gold and took luxury
// spares in barter for Oil (games 351 and 61202Z); so the ask here is copy for
// copy: the luxury the rival holds most of against one spare copy of ours
// (two when four or more spares are held), through the strategic barter's
// shape on the agent's buy arm (`LUXURY_ANY;RESOURCE_A=a,...`), Gold topping
// up to what a copy is worth and the treasury carries. In the luxury slot of
// the deal week, at the rival the turn rotates to among those at peace that
// offer a type we lack, with no deal already heading their way and no
// denunciation either way inside `LUXURY_SWAP_DENOUNCE_TURNS`.
//
// Version two (`luxury-swap-asks-2`), from 22 live Immortal games on
// 455f438ca: version one closed 36 of 119 asks and every close added a type,
// but a rival whose land holds the spare we put up closed 2 of 32 against 34
// of 87 otherwise (the rival's own tradeable list never names it, so only
// the board can tell), and 25 of the 32 had another partner or spare that
// fit; 28 luxury slots with a spare and an offer sat one Amenity short,
// under the two-short gate, while 55 of the 87 short cities at turn 150 were
// exactly one short. So version two asks from one Amenity short, and
// only of a partner whose mirrored board (`lacks`) holds none of one of our
// spares, offering those spares; with no such partner it holds.
const LUXURY_SWAP_MIN_DEFICIT: f64 = 2.0;
const LUXURY_SWAP_2_MIN_DEFICIT: f64 = 1.0;
const LUXURY_SWAP_SECOND_COPY_SPARES: f64 = 4.0;
const LUXURY_SWAP_DENOUNCE_TURNS: i64 = 30;

/// The swap's fit test: whether the seller's seat holds none of a luxury
/// (`None` when the board cannot read it). `luxury-swap-asks-2` only.
type LuxuryLacks<'a> = Option<&'a dyn Fn(i64, &str) -> Option<bool>>;

/// `lacks` (`luxury-swap-asks-2`, `None` for version one): whether the rival
/// with this host player id holds no copy of the luxury, as the host names
/// it, on its mirrored board; `None` when the board cannot say.
#[allow(clippy::type_complexity)]
fn append_luxury_swap_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    lacks: LuxuryLacks<'_>,
) -> Option<&'static str> {
    luxury_swap_order(state, orders, lacks, &|_| false)
}

/// `luxury-asks-pay-in-strategics-2`: the swap, with every luxury in `stale`
/// (one whose last swap the buy arm answered `barter_short`: the host would
/// not let that copy go, to any partner) off our side. Live 11-05: 085437Z
/// put one Marble up eight times from turn 94 to 142, to three partners,
/// and every ask shorted; 040135Z put one Truffles up eight times. Each held
/// the luxury slot, so the strategic ask never ran.
fn append_luxury_swap_order_2(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    lacks: LuxuryLacks<'_>,
    stale: &std::collections::BTreeSet<String>,
) -> Option<&'static str> {
    luxury_swap_order(state, orders, lacks, &|name| stale.contains(name))
}

/// Both swaps: `stale` names a luxury never put up.
fn luxury_swap_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    lacks: LuxuryLacks<'_>,
    stale: &dyn Fn(&str) -> bool,
) -> Option<&'static str> {
    let min_deficit = if lacks.is_some() {
        LUXURY_SWAP_2_MIN_DEFICIT
    } else {
        LUXURY_SWAP_MIN_DEFICIT
    };
    if amenity_deficit(state) < min_deficit {
        return Some("luxury_swap_hold:content");
    }
    if state.turn % LUXURY_BUY_CADENCE != LUXURY_BUY_PHASE {
        return Some("luxury_swap_hold:cadence");
    }
    let Some(counts) = state.luxury_counts.as_ref() else {
        return Some("luxury_swap_hold:counts_unknown");
    };
    // What another sale already gives this turn stays off our side.
    let selling: std::collections::BTreeSet<&str> = orders
        .iter()
        .filter(|order| order.kind == "sell")
        .filter_map(|order| order.verb.as_deref())
        .flat_map(|verb| verb.split([',', ';']))
        .filter_map(|part| part.split_once('=').map(|(name, _)| name))
        .collect();
    let mut spares: Vec<(&str, f64)> = counts
        .iter()
        .filter(|(name, _)| !selling.contains(name.as_str()) && !stale(name.as_str()))
        .map(|(name, count)| (name.as_str(), (count.floor() - 1.0).max(0.0)))
        .filter(|(_, spare)| *spare >= 1.0)
        .collect();
    if spares.is_empty() {
        return Some("luxury_swap_hold:no_spare");
    }
    spares.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(right.0))
    });
    let held: f64 = spares.iter().map(|(_, spare)| spare).sum();
    // One copy of the first spare offered; two copies once four or more are
    // held: one each of the first two, or two of a lone type that has them.
    let give = |offered: &[(&str, f64)]| {
        if held < LUXURY_SWAP_SECOND_COPY_SPARES {
            format!("{}=1", offered[0].0)
        } else if offered.len() > 1 {
            format!("{}=1,{}=1", offered[0].0, offered[1].0)
        } else if offered[0].1 >= 2.0 {
            format!("{}=2", offered[0].0)
        } else {
            format!("{}=1", offered[0].0)
        }
    };
    let recent = |turn: Option<i64>| {
        turn.is_some_and(|at| at >= 0 && i64::from(state.turn) - at < LUXURY_SWAP_DENOUNCE_TURNS)
    };
    let mut partners: Vec<&civvis::mirror::StateRival> = state
        .rivals
        .iter()
        .filter(|rival| !rival.at_war)
        .filter(|rival| {
            rival
                .tradeable_luxuries
                .as_ref()
                .is_some_and(|luxuries| !luxuries.is_empty())
        })
        .filter(|rival| !recent(rival.our_denounce_turn) && !recent(rival.their_denounce_turn))
        .filter(|rival| {
            !orders.iter().any(|order| {
                (order.kind == "sell" || order.kind == "buy")
                    && order.subject == Some(rival.player as i64)
            })
        })
        .collect();
    if partners.is_empty() {
        return Some("luxury_swap_hold:no_partner");
    }
    partners.sort_by_key(|rival| rival.player);
    let week = (state.turn / LUXURY_BUY_CADENCE) as usize;
    let (partner, given) = match lacks {
        None => (partners[week % partners.len()], give(&spares)),
        Some(lacks) => {
            // Each partner with the spares its board holds none of, a known
            // lack before one the board cannot read, most copies first.
            let fitted: Vec<(&civvis::mirror::StateRival, Vec<(&str, f64)>)> = partners
                .iter()
                .filter_map(|rival| {
                    let mut fit: Vec<(&str, f64, bool)> = spares
                        .iter()
                        .filter_map(|(name, spare)| match lacks(rival.player as i64, name) {
                            Some(false) => None,
                            known => Some((*name, *spare, known.is_some())),
                        })
                        .collect();
                    fit.sort_by_key(|(_, _, known)| !*known);
                    (!fit.is_empty()).then(|| {
                        (
                            *rival,
                            fit.into_iter()
                                .map(|(name, spare, _)| (name, spare))
                                .collect(),
                        )
                    })
                })
                .collect();
            if fitted.is_empty() {
                return Some("luxury_swap_hold:no_fit");
            }
            let (partner, fit) = &fitted[week % fitted.len()];
            (*partner, give(fit))
        }
    };
    let income = state
        .gold_per_turn
        .filter(|income| income.is_finite())
        .unwrap_or(0.0);
    let carried = (state.gold - LUXURY_BUY_GOLD_RESERVE).max(0)
        + (25.0 * (income - LUXURY_BUY_INCOME_FLOOR)).floor().max(0.0) as i64;
    let worth = LUXURY_BUY_CEILING_BASE
        + LUXURY_BUY_CEILING_PER_CITY * state.cities.len().min(i32::MAX as usize) as i32;
    let ceiling = carried
        .min(i64::from(worth.min(LUXURY_BUY_CEILING_MAX)))
        .max(0) as i32;
    orders.push(Order {
        kind: "buy",
        subject: Some(partner.player as i64),
        verb: Some(format!("LUXURY_ANY;{given}")),
        pos: Some((ceiling, 0)),
    });
    None
}

// ★★★★ A LUXURY WE LACK, PAID IN STRATEGICS OUR ARMY DOES NOT NEED
// (`luxury-asks-pay-in-strategics`). Live Lv 8 Deity Korea Online (28 games,
// 2026-11-04 21Z-11-05 00Z): host-reported "Amenities" lines cost 6.7% of all
// production at turns 90-110 (-79's census of `yield_sources`), the margin by
// which the near-miss games fell under the quarter-of-the-leader retire gate
// (0.236 and 0.245). On the luxury slot's turns t50-120 with a city short,
// the swap held 74 times: `no_spare` 51 (221505Z held ONE luxury type with 7-8
// cities short; most seats hold every type once), `no_partner` 13, `no_fit`
// 10. Gold alone closed 0 of 16 luxury asks at Deity, while barter closed 22
// (a spare copy, or strategics such as `RESOURCE_IRON=4;RESOURCE_COTTON=1`),
// and in 56 of the 74 holds the seat stockpiled 10 or more of a strategic
// that the surplus sale sells for Gold anyway. So when the swap finds no
// spare copy that fits, the same slot asks a partner at peace that offers a
// type we hold none of, and pays from the strategics held above what our
// units, their upgrades, their fuel and the barter reserve need
// (`AdvancedAi::siege_barter_spares`, strategics only): the spare the
// partner's seat books highest, the smallest lot its book prices at a
// copy's worth (the whole spare when none does), Gold topping up as the swap
// does, to a copy's worth. The Lua arm picks a luxury the seat holds none of.
const LUXURY_STRATEGIC_ASK_UNPRICED_LOT: u32 = 10;

/// `luxury-asks-pay-in-strategics`: see above. `spares` is
/// `AdvancedAi::siege_barter_spares` (only its strategics are read);
/// `give_worth(seller, resource_id, amount)` is what the lot is worth to the
/// seller's mirrored seat, 0 when the seat cannot be named. Returns why it
/// held, or `None` with the order pushed.
fn append_luxury_strategic_ask(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    spares: &[civvis::ai::BarterSpare],
    give_worth: &dyn Fn(i64, &str, u32) -> f64,
) -> Option<&'static str> {
    luxury_strategic_ask(state, orders, spares, give_worth, &|_| false, false)
}

/// `luxury-asks-pay-in-strategics-2`: version one's ask, but never of a
/// partner in `shorted` (the host's buy arm answered our last barter to it
/// `barter_short`: it could not take what we put up), and with Horses put up
/// only when no other strategic is spare. Live Deity games of 11-05 after
/// version one's arming: 29 of 61 strategic asks died in the Lua arm as
/// `barter_short` before reaching the partner, 24 of the 27 partners asked
/// more than once answered the same way every time, and Horses shorted 22
/// times in 42 asks against 2 in 10 for Niter (the rivals hold their Horses
/// at the stockpile cap).
fn append_luxury_strategic_ask_2(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    spares: &[civvis::ai::BarterSpare],
    give_worth: &dyn Fn(i64, &str, u32) -> f64,
    shorted: &std::collections::BTreeSet<i64>,
) -> Option<&'static str> {
    luxury_strategic_ask(
        state,
        orders,
        spares,
        give_worth,
        &|partner| shorted.contains(&partner),
        true,
    )
}

/// Both versions' ask: `skip` names a partner never asked, and
/// `horses_last` keeps Horses back while another strategic is spare.
fn luxury_strategic_ask(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    spares: &[civvis::ai::BarterSpare],
    give_worth: &dyn Fn(i64, &str, u32) -> f64,
    skip: &dyn Fn(i64) -> bool,
    horses_last: bool,
) -> Option<&'static str> {
    if amenity_deficit(state) < LUXURY_SWAP_2_MIN_DEFICIT {
        return Some("luxury_strategic_hold:content");
    }
    if state.turn % LUXURY_BUY_CADENCE != LUXURY_BUY_PHASE {
        return Some("luxury_strategic_hold:cadence");
    }
    let Some(counts) = state.luxury_counts.as_ref() else {
        return Some("luxury_strategic_hold:counts_unknown");
    };
    // The slot is the swap's: a luxury ask already out this turn keeps it.
    if orders.iter().any(|order| {
        order.kind == "buy"
            && order
                .verb
                .as_deref()
                .is_some_and(|verb| verb.starts_with("LUXURY_ANY"))
    }) {
        return Some("luxury_strategic_hold:slot_taken");
    }
    let selling: std::collections::BTreeSet<&str> = orders
        .iter()
        .filter(|order| order.kind == "sell")
        .filter_map(|order| order.verb.as_deref())
        .flat_map(|verb| verb.split([',', ';']))
        .filter_map(|part| part.split_once('=').map(|(name, _)| name))
        .collect();
    let mut payable: Vec<&civvis::ai::BarterSpare> = spares
        .iter()
        .filter(|spare| !spare.luxury && spare.amount > 0)
        .filter(|spare| !selling.contains(spare.resource.as_str()))
        .collect();
    if horses_last
        && payable
            .iter()
            .any(|spare| spare.resource != "RESOURCE_HORSES")
    {
        payable.retain(|spare| spare.resource != "RESOURCE_HORSES");
    }
    if payable.is_empty() {
        return Some("luxury_strategic_hold:no_strategic");
    }
    let recent = |turn: Option<i64>| {
        turn.is_some_and(|at| at >= 0 && i64::from(state.turn) - at < LUXURY_SWAP_DENOUNCE_TURNS)
    };
    let lacked = |name: &String| counts.get(name).copied().unwrap_or(0.0) < 1.0;
    let mut partners: Vec<&civvis::mirror::StateRival> = state
        .rivals
        .iter()
        .filter(|rival| !rival.at_war)
        .filter(|rival| {
            rival
                .tradeable_luxuries
                .as_ref()
                .is_some_and(|luxuries| luxuries.iter().any(lacked))
        })
        .filter(|rival| !recent(rival.our_denounce_turn) && !recent(rival.their_denounce_turn))
        .filter(|rival| {
            !orders.iter().any(|order| {
                (order.kind == "sell" || order.kind == "buy")
                    && order.subject == Some(rival.player as i64)
            })
        })
        .filter(|rival| !skip(rival.player as i64))
        .collect();
    if partners.is_empty() {
        return Some("luxury_strategic_hold:no_partner");
    }
    partners.sort_by_key(|rival| rival.player);
    let week = (state.turn / LUXURY_BUY_CADENCE) as usize;
    let partner = partners[week % partners.len()];
    let seller = partner.player as i64;
    let worth = f64::from(
        (LUXURY_BUY_CEILING_BASE
            + LUXURY_BUY_CEILING_PER_CITY * state.cities.len().min(i32::MAX as usize) as i32)
            .min(LUXURY_BUY_CEILING_MAX),
    );
    // (lot, its worth to the seller) for each spare: the smallest lot that
    // meets a copy's worth, else the whole spare; unpriced, a fixed lot.
    let lot = |spare: &civvis::ai::BarterSpare| -> (u32, f64) {
        if give_worth(seller, &spare.resource_id, spare.amount) <= 0.0 {
            return (spare.amount.min(LUXURY_STRATEGIC_ASK_UNPRICED_LOT), 0.0);
        }
        (1..=spare.amount)
            .map(|count| (count, give_worth(seller, &spare.resource_id, count)))
            .find(|(_, value)| *value >= worth)
            .unwrap_or_else(|| {
                (
                    spare.amount,
                    give_worth(seller, &spare.resource_id, spare.amount),
                )
            })
    };
    // The spare whose lot covers most of a copy, then the smaller lot, then
    // the name: a priced lot that meets the worth beats one that does not.
    let Some((spare, (count, _))) = payable.iter().map(|spare| (*spare, lot(spare))).max_by(
        |(left, (left_count, left_value)), (right, (right_count, right_value))| {
            left_value
                .min(worth)
                .partial_cmp(&right_value.min(worth))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right_count.cmp(left_count))
                .then_with(|| right.resource.cmp(&left.resource))
        },
    ) else {
        return Some("luxury_strategic_hold:no_strategic");
    };
    let income = state
        .gold_per_turn
        .filter(|income| income.is_finite())
        .unwrap_or(0.0);
    let carried = (state.gold - LUXURY_BUY_GOLD_RESERVE).max(0)
        + (25.0 * (income - LUXURY_BUY_INCOME_FLOOR)).floor().max(0.0) as i64;
    // The swap's top-up: Gold up to a copy's worth, which the treasury and
    // income carry. The host's counter asks what it wants on top of the lot,
    // and the Lua arm closes only at or under this.
    let ceiling = carried.min(worth.floor() as i64).max(0) as i32;
    orders.push(Order {
        kind: "buy",
        subject: Some(seller),
        verb: Some(format!("LUXURY_ANY;{}={count}", spare.resource)),
        pos: Some((ceiling, 0)),
    });
    None
}

// ★★★★★ THE GUN'S RESOURCE, BOUGHT WHERE A RIVAL OFFERS IT
// (`siege-buys-the-gun-resource`). Live Emperor civvis-20261007T111017Z (game
// 343) held Angkor Thom's 400 walls at turns 176-177 at a Trebuchet's 2.3 a
// shot, Steel and Military Science researched and no Niter or Oil tile owned,
// while the Zulu offered 82 Niter on their trade screen and Arabia 70. At turn
// 150 of 101 Emperor runs of October 6-7, 92 lacked Niter or Oil and 53 had a
// met rival offering one; -d8 read 22% of 1,000 walled infinite siege-turns
// as resource-blocked (Oil 13%, Niter 9%), the tech-best gun hitting a median
// 9.4 a shot against 4.1 for the buildable best. The rival's offer is the
// export's `tradeable_strategics`; what the campaign's guns want, and how
// much, is `AdvancedAi::siege_gun_resource_wants` (the Bombard's Niter sized
// to the guns its blow needs, 20 a gun; the Artillery's Oil with ten turns of
// upkeep, renewed when the fielded guns' fuel runs low).
//
// The order asks the seller's own price with EQUALIZE for `RESOURCE_X=N`, a
// lump from their side, and the Lua arm closes only at or under the ceiling
// carried in `x`. The price is the engine's own book for the block
// (`Game::strategic_gold_value`). Gold alone rarely meets it: -d8 read a
// median 115 Gold in the treasury on the Niter-blocked turns of 10-06/07 and
// 118 on the Oil-blocked, none at the 641 a gun's 20 Niter books at and 3-10%
// at 300. So our side BARTERS first (`RESOURCE_X=N;RESOURCE_A=a,...`): spare
// luxury copies, then strategics held above our units' needs
// (`AdvancedAi::siege_barter_spares`), each valued at what it is worth to the
// seller by the same book (`Game::resource_gold_value`), until the block's
// price is met; the rest is Gold, never more than the treasury and the income
// carry at the 25× book with a reserve and a floor kept clear, and that is the
// ceiling. The full lot goes when our side covers it, else one gun's worth;
// otherwise the ask waits. Nothing the guns hold, nothing another sale gives
// this turn, and never a luxury's last copy goes on our side.
// Cadence-gated in the free slots of the deal week (phase 2 of 3: turns 2 and
// 5 of 6, clear of the passage at 1, the favor sale at 3 and the luxury at 4),
// the largest offer first among the majors at peace with no deal already
// heading their way.
const GUN_RESOURCE_BUY_CADENCE: u32 = 3;
const GUN_RESOURCE_BUY_PHASE: u32 = 2;
const GUN_RESOURCE_BUY_GOLD_RESERVE: i64 = 60;
const GUN_RESOURCE_BUY_INCOME_FLOOR: f64 = 4.0;
const GUN_RESOURCE_BUY_CEILING_MIN: i32 = 30;
const GUN_RESOURCE_BUY_CEILING_MAX: i32 = 900;
/// `aluminum-ask-tops-up-gold`: the Gold a unit of the air wing's Aluminum
/// ask may cost on top of its barter, the most the host's counter is let to
/// ask (`aluminum_ask_ceiling`).
const ALUMINUM_ASK_GOLD_PER_UNIT: i64 = 8;

/// `aluminum-ask-tops-up-gold`: the Gold ceiling an Aluminum buy order
/// carries, or `None` when `order` is not one or its ceiling already stands
/// at or above the top-up. The order's Gold is the ceiling the Lua ask
/// registers: the host's counter (`EQUALIZE`) may add our Gold to the deal up
/// to it, and the answer closes only at or under it. The top-up is
/// [`ALUMINUM_ASK_GOLD_PER_UNIT`] a unit asked, never more than the treasury
/// above `floor`. Over 10-13/14 every Aluminum ask carried a ceiling of 0 and
/// 7 of 91 answered closed, against 38% of luxury swaps.
fn aluminum_ask_ceiling(order: &Order, gold: i64, floor: i64) -> Option<i32> {
    if order.kind != "buy" {
        return None;
    }
    let verb = order.verb.as_deref()?;
    let amount: i64 = verb
        .strip_prefix("RESOURCE_ALUMINUM=")?
        .split(';')
        .next()?
        .parse()
        .ok()?;
    let current = i64::from(order.pos.map_or(0, |(gold, _)| gold));
    let ceiling = (ALUMINUM_ASK_GOLD_PER_UNIT * amount)
        .min((gold - floor).max(0))
        .min(i64::from(GUN_RESOURCE_BUY_CEILING_MAX));
    (ceiling > current).then_some(ceiling as i32)
}

/// `siege-buys-the-gun-resource`: drop every sale that would give away a
/// strategic in `held` (`AdvancedAi::siege_gun_resources_held`, host names)
/// — the planner's surplus sale (`sale_verb`, `RESOURCE_X=N`, alone or in a
/// bundle) reads a bought block as surplus. Returns how many were dropped.
fn drop_held_resource_sales(
    held: &std::collections::BTreeSet<String>,
    orders: &mut Vec<Order>,
) -> usize {
    if held.is_empty() {
        return 0;
    }
    let before = orders.len();
    orders.retain(|order| {
        order.kind != "sell"
            || !order.verb.as_deref().is_some_and(|verb| {
                verb.split(',')
                    .filter_map(|part| part.split_once('='))
                    .any(|(name, _)| held.contains(name))
            })
    });
    before - orders.len()
}

fn protect_army_upgrade_material_sales(
    ai: &civvis::ai::AdvancedAi,
    game: &civvis::game::Game,
    wants: &[civvis::ai::ArmyUpgradeMaterialWant],
    orders: &mut Vec<Order>,
) -> (std::collections::BTreeSet<String>, usize) {
    let mut held = ai.siege_gun_resources_held(game, 0, &[]);
    held.extend(wants.iter().map(|want| want.resource.clone()));
    let removed = drop_held_resource_sales(&held, orders);
    let mut budgets: std::collections::BTreeMap<_, _> = ai
        .army_upgrade_material_reserves(game, 0)
        .into_iter()
        .map(|(resource, reserve)| {
            (
                format!("RESOURCE_{}", resource.as_str().to_ascii_uppercase()),
                (game.strategic_stockpile(0, resource), reserve),
            )
        })
        .collect();
    if budgets.is_empty() {
        return (held, removed);
    }
    let before = orders.len();
    orders.retain(|order| {
        if order.kind != "sell" {
            return true;
        }
        let mut package = std::collections::BTreeMap::<&str, f64>::new();
        for (resource, amount) in order
            .verb
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .filter_map(|part| part.split_once('='))
        {
            if let (Some(_), Ok(amount)) = (budgets.get(resource), amount.parse::<u32>()) {
                *package.entry(resource).or_default() += f64::from(amount);
            }
        }
        if package.iter().any(|(resource, amount)| {
            let (available, reserve) = budgets[*resource];
            available - *amount + f64::EPSILON < reserve
        }) {
            return false;
        }
        for (resource, amount) in package {
            if let Some((available, _)) = budgets.get_mut(resource) {
                *available -= amount;
            }
        }
        true
    });
    (held, removed + before - orders.len())
}

/// Why no strategic-purchase order was appended this turn, for the note;
/// `None` when one was. `wants` is `AdvancedAi::siege_gun_resource_wants`,
/// strongest gun first; `spares` is `AdvancedAi::siege_barter_spares`,
/// luxuries first; `worth` prices a block of a resource (its CIVVIS id) to us
/// by the engine's own book, and `give_worth` what `amount` of one of ours is
/// worth to the host player `seller` by the same book.
#[derive(Clone, Copy)]
struct ResourceBuyBudget {
    reserve: i64,
    gold_limit: Option<i64>,
    credit_income: bool,
    cadence: bool,
}

fn append_gun_resource_buy_order(
    wants: &[civvis::ai::GunResourceWant],
    spares: &[civvis::ai::BarterSpare],
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    worth: &dyn Fn(&str, u32) -> f64,
    give_worth: &dyn Fn(i64, &str, u32) -> f64,
) -> Option<&'static str> {
    append_resource_buy_order(
        wants,
        spares,
        state,
        orders,
        worth,
        give_worth,
        ResourceBuyBudget {
            reserve: GUN_RESOURCE_BUY_GOLD_RESERVE,
            gold_limit: None,
            credit_income: true,
            cadence: true,
        },
    )
}

fn append_army_upgrade_material_buy_order(
    wants: &[civvis::ai::ArmyUpgradeMaterialWant],
    spares: &[civvis::ai::BarterSpare],
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    worth: &dyn Fn(&str, u32) -> f64,
    give_worth: &dyn Fn(i64, &str, u32) -> f64,
    remaining_gold: i64,
) -> Option<&'static str> {
    let Some(want) = wants.first() else {
        return Some("no_need");
    };
    // A previously emitted purchase can still change the real treasury.
    // Finish that market operation before requesting another material.
    if orders.iter().any(|order| order.kind == "buy") {
        return Some("deal_in_flight");
    }
    let reserve = want.gold_reserve.ceil() as i64;
    if state.gold.min(remaining_gold) < reserve {
        return Some("treasury");
    }
    // The shared transport reads resource/amount/minimum; its gun-specific
    // descriptive fields are unused here. The upgrade remains a named want.
    let request = civvis::ai::GunResourceWant {
        resource: want.resource.clone(),
        resource_id: want.resource_id.clone(),
        amount: want.amount,
        minimum: want.amount,
        unit: want.unit.clone(),
        guns: 1,
        hit: want.gain,
        renewal: false,
    };
    append_resource_buy_order(
        &[request],
        spares,
        state,
        orders,
        worth,
        give_worth,
        ResourceBuyBudget {
            reserve,
            gold_limit: Some(remaining_gold),
            credit_income: false,
            cadence: false,
        },
    )
    .map(|why| match why {
        "gun_resource_buy_hold:treasury" => "treasury",
        "gun_resource_buy_hold:deal_in_flight" => "deal_in_flight",
        "gun_resource_buy_hold:no_seller" => "no_seller",
        _ => "no_need",
    })
}

fn append_resource_buy_order(
    wants: &[civvis::ai::GunResourceWant],
    spares: &[civvis::ai::BarterSpare],
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    worth: &dyn Fn(&str, u32) -> f64,
    give_worth: &dyn Fn(i64, &str, u32) -> f64,
    budget: ResourceBuyBudget,
) -> Option<&'static str> {
    if wants.is_empty() {
        return Some("gun_resource_buy_hold:no_need");
    }
    if budget.cadence && state.turn % GUN_RESOURCE_BUY_CADENCE != GUN_RESOURCE_BUY_PHASE {
        return Some("gun_resource_buy_hold:cadence");
    }
    let income = state
        .gold_per_turn
        .filter(|income| income.is_finite())
        .unwrap_or(0.0);
    let gold = budget
        .gold_limit
        .map_or(state.gold, |limit| state.gold.min(limit));
    let credit = if budget.credit_income {
        (25.0 * (income - GUN_RESOURCE_BUY_INCOME_FLOOR))
            .floor()
            .max(0.0) as i64
    } else {
        0
    };
    let carried = (gold - budget.reserve).max(0) + credit;
    let affordable = carried.min(GUN_RESOURCE_BUY_CEILING_MAX as i64) as i32;
    // What another sale already gives this turn stays off our side.
    let selling: std::collections::BTreeSet<&str> = orders
        .iter()
        .filter(|order| order.kind == "sell")
        .filter_map(|order| order.verb.as_deref())
        .flat_map(|verb| verb.split(','))
        .filter_map(|part| part.split_once('=').map(|(name, _)| name))
        .collect();
    let mut busy = false;
    let mut dear = false;
    for want in wants {
        let offered = |rival: &civvis::mirror::StateRival| {
            rival
                .tradeable_strategics
                .as_ref()
                .and_then(|offers| offers.get(&want.resource))
                .copied()
                .filter(|offer| *offer >= i64::from(want.minimum.max(1)))
        };
        let in_flight = |rival: &civvis::mirror::StateRival| {
            orders.iter().any(|order| {
                (order.kind == "sell" || order.kind == "buy")
                    && order.subject == Some(rival.player as i64)
            })
        };
        let sellers: Vec<_> = state
            .rivals
            .iter()
            .filter(|rival| !rival.at_war)
            .filter_map(|rival| offered(rival).map(|offer| (rival, offer)))
            .collect();
        busy |= sellers.iter().any(|(rival, _)| in_flight(rival));
        let Some((seller, offer)) = sellers
            .into_iter()
            .filter(|(rival, _)| !in_flight(rival))
            .max_by_key(|(rival, offer)| (*offer, std::cmp::Reverse(rival.player)))
        else {
            continue;
        };
        let full = want.amount.min(offer.clamp(0, i64::from(u32::MAX)) as u32);
        let mut lots = vec![full];
        if want.minimum < full {
            lots.push(want.minimum);
        }
        for amount in lots {
            let price = worth(&want.resource_id, amount);
            if !(price.is_finite() && price > 0.0) {
                // No book for the block: Gold alone, at what the treasury
                // carries.
                if affordable < GUN_RESOURCE_BUY_CEILING_MIN {
                    continue;
                }
                orders.push(Order {
                    kind: "buy",
                    subject: Some(seller.player as i64),
                    verb: Some(format!("{}={amount}", want.resource)),
                    pos: Some((affordable, 0)),
                });
                return None;
            }
            let seller_id = seller.player as i64;
            let mut covered = 0.0;
            let mut given: Vec<String> = Vec::new();
            for spare in spares {
                if covered >= price {
                    break;
                }
                if spare.resource == want.resource || selling.contains(spare.resource.as_str()) {
                    continue;
                }
                // A luxury goes only above the host's own count of it, which
                // nets out copies a deal has traded away: the board counts
                // the tiles, and live game 354 priced the Turtles and Marble
                // it had sold to Kongo into seven Oil asks the agent's
                // last-copy guard refused unproposed. An older export with
                // no count keeps the board's reading.
                if spare.luxury
                    && state.luxury_counts.as_ref().is_some_and(|counts| {
                        counts.get(&spare.resource).copied().unwrap_or(0.0) < 2.0
                    })
                {
                    continue;
                }
                let remaining = price - covered;
                let count = if spare.luxury {
                    1
                } else {
                    (1..=spare.amount)
                        .find(|count| {
                            give_worth(seller_id, &spare.resource_id, *count) >= remaining
                        })
                        .unwrap_or(spare.amount)
                };
                let value = give_worth(seller_id, &spare.resource_id, count);
                if count == 0 || !(value.is_finite() && value > 0.0) {
                    continue;
                }
                covered += value;
                given.push(format!("{}={count}", spare.resource));
            }
            let gold = (price - covered).max(0.0).ceil();
            if gold > f64::from(affordable)
                || (given.is_empty() && gold < f64::from(GUN_RESOURCE_BUY_CEILING_MIN))
            {
                continue;
            }
            let verb = if given.is_empty() {
                format!("{}={amount}", want.resource)
            } else {
                format!("{}={amount};{}", want.resource, given.join(","))
            };
            orders.push(Order {
                kind: "buy",
                subject: Some(seller_id),
                verb: Some(verb),
                pos: Some((gold as i32, 0)),
            });
            return None;
        }
        dear = true;
    }
    Some(if dear {
        "gun_resource_buy_hold:treasury"
    } else if busy {
        "gun_resource_buy_hold:deal_in_flight"
    } else {
        "gun_resource_buy_hold:no_seller"
    })
}

// ★★★★★ THE FAVOR BANK, SPENT ON THE PLAN THAT IS ACTUALLY RUNNING. Diplomatic
// favor is votes at the World Congress and nothing else; on the live seat the
// bank has read 200–420 at the end of every game measured, and the 2026-08-18
// study found the ballot's favor spend never registers at all. Under a
// Diplomacy plan every point is a vote CIVVIS means to cast, so nothing sells
// (the native `quick_deals` block never fires there either: it wants a partner
// two DVP richer). Under any other plan the points are idle capital, and the
// one rival that must never buy them is one already close to the twenty-point
// win — favor in that treasury is the win. So: not on a Diplomacy plan, hold
// `FAVOR_RESERVE` for the emergency ballot, sell the rest in blocks to the
// richest met major we are at peace with that is below `FAVOR_BUYER_DVP_MAX`,
// on a cadence offset from the planner's own trade turn (`turn % 6 == pid`),
// at a floor of one gold a point — the rival's own valuation sets the price.
const FAVOR_RESERVE: f64 = 120.0;
const FAVOR_SALE_MIN: f64 = 20.0;
const FAVOR_SALE_MAX: f64 = 150.0;
const FAVOR_SALE_CADENCE: u32 = 6;
const FAVOR_SALE_PHASE: u32 = 3;
const FAVOR_BUYER_DVP_MAX: i64 = 12;
/// The Diplomatic Victory points at which any met rival makes the bank votes
/// again: from then on no favor sells, ours or the planner's.
///
/// ★★★ THE VOTES REGISTER NOW, AND THE BANK WAS BEING SOLD. The note on
/// [`plan_keeps_favor`] records a host that never honoured a purchased vote;
/// the congress budget (`CivvisCongressVoteBudget` in the agent) has since
/// cast them. Live King civvis-20261005T030607Z (game 95): "deny" ballots cast
/// 3 votes against the Zulu for 112 favor at turn 181 and 40 at 201, and the
/// Zulu went 16 -> 15 and 17 -> 15 DVP across those sessions. The bank was at
/// 0 by 210 (grievances drain it), the turn 221 ballot could cast only free
/// votes, and the Zulu took +5 at 222 and won. Between turns 87 and 177 this
/// sale had sold sixteen blocks, about 600 favor, some of it to England, who
/// finished on 16 DVP. Four is a fifth of the win, reached around the middle
/// of a game: from then on the favor is the vote against the leader.
const FAVOR_HOLD_DVP: i64 = 4;

/// Whether the bank is held for the World Congress: a met rival has reached
/// [`FAVOR_HOLD_DVP`]. `CIVVIS_SELL_IDLE_FAVOR=1` restores the old sale.
fn favor_banked_for_votes(state: &civvis::mirror::StateSnapshot) -> bool {
    let sell_anyway = std::env::var("CIVVIS_SELL_IDLE_FAVOR")
        .is_ok_and(|value| !value.is_empty() && value != "0");
    !sell_anyway
        && state
            .rivals
            .iter()
            .any(|rival| rival.dvp.unwrap_or(0) >= FAVOR_HOLD_DVP)
}
/// Turns of negative income the treasury must cover before the favor bank is
/// let go for it. Ten: the planner starts disbanding units inside one to
/// three turns of runway (`bankruptcy-exits-the-drain`), and a sale placed
/// this turn pays on the next.
const FAVOR_DRAIN_RUNWAY_TURNS: f64 = 10.0;
/// The favor a drained treasury still keeps: one extra vote's worth.
const FAVOR_DRAIN_RESERVE: f64 = 20.0;
/// The Diplomatic Victory points at which even a drained treasury holds its
/// bank: eight short of the win, where a single session can decide it.
const FAVOR_DRAIN_HOLD_DVP: i64 = 12;

/// Whether the treasury is being drained: income is negative and the Gold in
/// hand covers fewer than [`FAVOR_DRAIN_RUNWAY_TURNS`] turns of it.
///
/// ★ THE BANK WAS HELD WHILE THE ARMY WAS DISBANDED FOR GOLD. Live Deity
/// Russia, 2026-11-10: 19 of 43 games disbanded three or more units "to leave
/// the bankruptcy drain". civvis-20261110T091939Z held 117 favor at t100 on
/// 19 Gold and -1 a turn, 244 at t140 and 332 at t160, with every rival past
/// [`FAVOR_HOLD_DVP`], and disbanded 14 units. civvis-20261110T090948Z
/// deleted 16 soldiers in peace from t66 at 0 Gold and -3 to -8 a turn; its
/// neighbour declared at t103 with 1,190 military strength against 202 and
/// took four of its five cities. A rival at four points of twenty is a
/// threat a session away from mattering; an army deleted this turn is not
/// coming back.
fn treasury_drained(state: &civvis::mirror::StateSnapshot) -> bool {
    state
        .gold_per_turn
        .filter(|income| income.is_finite() && *income < 0.0)
        .is_some_and(|income| (state.gold as f64) < -income * FAVOR_DRAIN_RUNWAY_TURNS)
}

/// Whether a drained treasury may sell past the vote hold: it is drained and
/// no met rival has reached [`FAVOR_DRAIN_HOLD_DVP`].
fn drain_sells_the_bank(state: &civvis::mirror::StateSnapshot) -> bool {
    treasury_drained(state)
        && !state
            .rivals
            .iter()
            .any(|rival| rival.dvp.unwrap_or(0) >= FAVOR_DRAIN_HOLD_DVP)
}

/// ★ Until 2026-08-24 the floor was a flat Gold a point; the engine's own
/// book (`Game::favor_gold_value`, `1.1 + 0.14·era + 0.07·dvp`) says 1.7–3.0
/// over a game, and the caller now passes it. Kept as the floor of floors.
const FAVOR_GOLD_FLOOR_PER_POINT: i32 = 1;

/// Whether the plan in force can still spend its favor. Only a seat with no
/// plan report holds now; every plan sells its surplus above `FAVOR_RESERVE`.
///
/// ⚠ A Diplomacy strategy or an assigned diplomatic lane used to hold every
/// point, on the reading that each one is a vote CIVVIS means to cast. **It
/// cannot cast them.** The first vote on a resolution is free — the cost
/// ladder a ballot reports starts `{index 0, cost 0}`, then 4, 12, 24 — so
/// favor buys only EXTRA votes, and the host has never honoured
/// `PARAM_WORLD_CONGRESS_VOTES > 1` through the mod's path. The comment above
/// `FAVOR_RESERVE` already recorded that the ballot's favor spend never
/// registers; this is the policy catching up with it.
///
/// Measured on run `civvis-20260819T054901Z`, a full Chieftain game to t222:
/// the plan read `victory=Some("diplomatic")` on 119 turns, so nothing sold;
/// **0 of 7** purchased-vote ballots registered; and the seat ended holding
/// **566 favor** with 3 diplomatic victory points against Germany's 20, losing
/// to that rival's diplomatic victory. Every one of those points was banked
/// for a purchase that cannot happen.
///
/// `FAVOR_RESERVE` still stands for the emergency ballot, so this frees the
/// surplus rather than the bank. If the multi-vote purchase is ever fixed in
/// the game core, revisit this: the holding was correct reasoning about a
/// mechanism that does not work, not a mistake about what favor is for.
fn plan_keeps_favor(plan: Option<(&str, Option<&str>)>) -> bool {
    plan.is_none()
}

/// Whether the seat is ASSIGNED the Diplomatic lane: its bank is the
/// agent's racer ballot (`CivvisDiploRacer` in the agent), so none of it
/// sells. The note on [`plan_keeps_favor`] predates purchased votes
/// registering; the ballot casts them now, and a racer's Favor buys its own
/// Congress Participation points at 4-24 Favor a resolution. Live Prince
/// civvis-20261030T043401Z (a Culture seat) banked 570 Favor idle for three
/// sessions; a diplomatic seat that sold that bank before any rival reached
/// [`FAVOR_HOLD_DVP`] would have sold its first claims.
fn plan_races_diplomacy(plan: Option<(&str, Option<&str>)>) -> bool {
    plan.is_some_and(|(_, victory)| victory == Some("diplomatic"))
}

/// Drop the planner's alliance orders unless the seat is assigned the
/// Diplomatic lane ([`plan_races_diplomacy`]): an alliance forbids a war on
/// the ally, and the domination campaign planner does not price that. The
/// agent's `CivvisAllianceStep` refuses them off the lane too; holding them
/// here keeps eight to fifteen refusals a game out of the order ledger.
/// `culture_allies` (`culture-seat-allies` armed on a seat aiming at
/// Culture, or `science-seat-allies` on one aiming at Science) keeps them for
/// that lane as well; the agent takes them under `CultureAllies` /
/// `ScienceAllies`. `shield` is the host player and alliance type
/// `military-alliance-shield` asked this turn
/// ([`AdvancedAi::military_alliance_shield_ask`]): that one order crosses on
/// every lane, since that desk prices the Domination target and prey itself,
/// and the agent takes it under `MilitaryAllies` (`ALLIANCE_MILITARY`) or,
/// for version two's other kinds, `ShieldAllies`. Returns how many were
/// dropped.
fn drop_off_lane_alliances(
    plan: Option<(&str, Option<&str>)>,
    culture_allies: bool,
    shield: Option<(i64, &str)>,
    orders: &mut Vec<Order>,
) -> usize {
    if plan_races_diplomacy(plan)
        || (culture_allies
            && plan.is_some_and(|(_, victory)| matches!(victory, Some("culture" | "science"))))
    {
        return 0;
    }
    let before = orders.len();
    orders.retain(|order| {
        order.kind != "alliance"
            || shield.is_some_and(|(subject, verb)| {
                order.subject == Some(subject) && order.verb.as_deref() == Some(verb)
            })
    });
    before - orders.len()
}

/// Drop the planner's own favor sales when the plan means to cast that favor,
/// or once a rival's DVP makes the bank votes ([`favor_banked_for_votes`]).
/// `Game::quick_deals` quotes a ten-favor block to any partner two DVP richer,
/// plan-blind — a sale to a rival nearer the win than we are. Returns how many
/// orders were held back.
fn hold_planner_favor_sales(
    plan: Option<(&str, Option<&str>)>,
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
) -> usize {
    if !plan_keeps_favor(plan)
        && !plan_races_diplomacy(plan)
        && (!favor_banked_for_votes(state) || drain_sells_the_bank(state))
    {
        return 0;
    }
    let before = orders.len();
    orders.retain(|order| {
        !(order.kind == "sell"
            && order
                .verb
                .as_deref()
                .is_some_and(|verb| verb.contains("FAVOR=")))
    });
    before - orders.len()
}

/// Drop the deal offers aimed at a rival this batch also declares war on or
/// offers peace to. Every deal, war and peace opens the same per-rival
/// diplomacy session, and the host refuses a second request while one is
/// open. Live King civvis-20261003T060034Z, turn 112: `sell
/// RESOURCE_AMBER=1` to Phoenicia went first, the declaration's request found
/// the session busy ("Requested Session but already had one open" in
/// DiplomacyManager.csv), no war happened, and the refusal cooldown held the
/// campaign at peace with the rival it was countering. Returns how many
/// offers were dropped.
fn drop_deals_beside_war_or_peace(orders: &mut Vec<Order>) -> usize {
    let contested: std::collections::BTreeSet<i64> = orders
        .iter()
        .filter(|order| matches!(order.kind, "war" | "peace"))
        .filter_map(|order| order.subject)
        .collect();
    if contested.is_empty() {
        return 0;
    }
    let before = orders.len();
    orders.retain(|order| {
        !(matches!(order.kind, "sell" | "buy" | "alliance")
            && order
                .subject
                .is_some_and(|subject| contested.contains(&subject)))
    });
    before - orders.len()
}

/// Keep a batch's first alliance ask as its only deal session: every other
/// alliance, sale and purchase in the batch waits for a later turn. Each one
/// is a `CivvisTrade.ask`, and the asks of one batch open their sessions in
/// the same frame. A rival that refuses an alliance answers with a statement
/// of its own, which the host leaves "Adding to pending" while another of our
/// sessions is still held open, and the game never advances. Live King, 11-01:
/// every Culture seat froze on the turn of an alliance ask sent beside an
/// EQUALIZE sale or an open-borders buy to a different rival (102116Z t120,
/// four times from the same save; 063953Z t186; 091422Z t120; 081602Z t149).
/// Each time, DiplomacyManager.csv ended on the alliance target's
/// "AI_REFUSE_DEAL" then "MAKE_DEAL, POSITIVE, POSITIVE, Adding to pending".
/// Alliance asks sent alone (081602Z t128 and t144; Science seats 043921Z and
/// 023248Z) did not freeze. Sales and purchases are re-quoted every turn;
/// the alliance ask is rarer and carries the plan. Returns how many orders
/// were held back.
fn hold_deals_beside_an_alliance(orders: &mut Vec<Order>) -> usize {
    let Some(first) = orders.iter().position(|order| order.kind == "alliance") else {
        return 0;
    };
    let before = orders.len();
    let mut index = 0;
    orders.retain(|order| {
        let keep = index == first || !matches!(order.kind, "sell" | "buy" | "alliance");
        index += 1;
        keep
    });
    before - orders.len()
}

fn defer_alliances_for_defensive_material(
    orders: &mut Vec<Order>,
    native_defense_buy: bool,
) -> usize {
    if !native_defense_buy || !orders.iter().any(|order| order.kind == "buy") {
        return 0;
    }
    let before = orders.len();
    orders.retain(|order| order.kind != "alliance");
    before - orders.len()
}

/// Why no favor sale order was appended this turn, for the note; `None` when
/// one was. `plan` is the plan report's `(strategy, victory_target)`.
fn append_favor_sale_order(
    plan: Option<(&str, Option<&str>)>,
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    favor_value: f64,
) -> Option<&'static str> {
    // `plan_keeps_favor` now answers only this, so the diplomacy branch that
    // stood here is gone with it — it could never be reached past this line.
    if plan.is_none() {
        return Some("favor_hold:no_plan");
    }
    if plan_races_diplomacy(plan) {
        return Some("favor_hold:diplomatic_lane");
    }
    // See `treasury_drained`: a drained treasury sells past the vote hold,
    // off the cadence, and down to a smaller reserve.
    let drained = drain_sells_the_bank(state);
    if favor_banked_for_votes(state) && !drained {
        return Some("favor_hold:contender");
    }
    if !drained && state.turn % FAVOR_SALE_CADENCE != FAVOR_SALE_PHASE {
        return Some("favor_hold:cadence");
    }
    let Some(favor) = state.favor.filter(|favor| favor.is_finite()) else {
        return Some("favor_hold:unknown");
    };
    let surplus = favor
        - if drained {
            FAVOR_DRAIN_RESERVE
        } else {
            FAVOR_RESERVE
        };
    if surplus < FAVOR_SALE_MIN {
        return Some("favor_hold:reserve");
    }
    // The planner may already be selling favor this turn through its own
    // quote; one block per turn keeps the two from bidding the same bank.
    if orders.iter().any(|order| {
        order.kind == "sell"
            && order
                .verb
                .as_deref()
                .is_some_and(|verb| verb.contains("FAVOR="))
    }) {
        return Some("favor_hold:planner_sale");
    }
    let buyer = state
        .rivals
        .iter()
        .filter(|rival| !rival.at_war && rival.dvp.unwrap_or(0) < FAVOR_BUYER_DVP_MAX)
        .max_by(|left, right| {
            let gold = |rival: &civvis::mirror::StateRival| {
                if rival.gold.is_finite() {
                    rival.gold
                } else {
                    0.0
                }
            };
            gold(left)
                .partial_cmp(&gold(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.player.cmp(&left.player))
        });
    let Some(buyer) = buyer else {
        return Some("favor_hold:no_buyer");
    };
    let amount = surplus.min(FAVOR_SALE_MAX).floor() as i32;
    let per_point = if favor_value.is_finite() {
        favor_value.max(FAVOR_GOLD_FLOOR_PER_POINT as f64)
    } else {
        FAVOR_GOLD_FLOOR_PER_POINT as f64
    };
    orders.push(Order {
        kind: "sell",
        subject: Some(buyer.player as i64),
        verb: Some(format!("FAVOR={amount}")),
        pos: Some(((amount as f64 * per_point).ceil() as i32, 0)),
    });
    None
}

// ★★★ FAVOR BOUGHT BEFORE THE SESSION THAT DECIDES THE GAME
// (`favor-bought-before-congress`, opt-in). A bought World Congress vote is
// cheap on this host — the ballot's cumulative ladder reads 4, 12, 24, 40, 60
// and 84 Favor for two to seven votes — and the Diplomatic Victory is won and
// denied a single vote at a time: live King civvis-20261005T051413Z (game
// 102) lost the +2 to the Aztecs 6-5, 5-4 and 4-3 at three sessions running.
// Yet about half of the Diplomatic losses of October 2-5 met the deciding
// session at 0-40 Favor while holding 200-1,000 Gold (game 126: 0 Favor from
// turn 212 to the turn-221 session, 185-341 Gold), and rivals had paid us
// about 5.5 Gold a point (median of forty favor sales on record). So, while a
// player stands at `FAVOR_BUY_LEADER_DVP` or more and the next session is at
// most `FAVOR_BUY_SESSION_TURNS` away, a bank under `FAVOR_BUY_TARGET` asks
// one peaceful rival for the shortfall, in a block of `FAVOR_BUY_MIN` to
// `FAVOR_BUY_MAX`, at up to `FAVOR_BUY_PRICE` Gold a point, with
// `FAVOR_BUY_GOLD_RESERVE` kept back. The highest-DVP rival at peace is asked
// first: Favor taken from a contender is a vote it no longer casts. The order
// rides the `buy` arm with `FAVOR=N`; the agent clips the block to the bank
// the engine says that rival holds and closes only at or under the ceiling.
const FAVOR_BUY_LEADER_DVP: i64 = 10;
const FAVOR_BUY_SESSION_TURNS: i64 = 5;
const FAVOR_BUY_TARGET: f64 = 100.0;
const FAVOR_BUY_MIN: i32 = 20;
const FAVOR_BUY_MAX: i32 = 80;
const FAVOR_BUY_PRICE: f64 = 8.0;
const FAVOR_BUY_GOLD_RESERVE: i64 = 100;

/// Why no favor purchase order was appended this turn, for the note; `None`
/// when one was. Nothing with the gene off.
fn append_favor_buy_order(
    enabled: bool,
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
) -> Option<&'static str> {
    if !enabled {
        return Some("favor_buy_hold:gene");
    }
    let top_dvp = state
        .rivals
        .iter()
        .filter_map(|rival| rival.dvp)
        .chain(state.dvp)
        .max()
        .unwrap_or(0);
    if top_dvp < FAVOR_BUY_LEADER_DVP {
        return Some("favor_buy_hold:no_contender");
    }
    let Some(turns_left) = state.congress_turns_left else {
        return Some("favor_buy_hold:session_unknown");
    };
    if !(0..=FAVOR_BUY_SESSION_TURNS).contains(&turns_left) {
        return Some("favor_buy_hold:session_far");
    }
    let Some(favor) = state.favor.filter(|favor| favor.is_finite()) else {
        return Some("favor_buy_hold:unknown");
    };
    let shortfall = (FAVOR_BUY_TARGET - favor).floor() as i32;
    if shortfall < FAVOR_BUY_MIN {
        return Some("favor_buy_hold:banked");
    }
    // A sale of ours this turn (`CIVVIS_SELL_IDLE_FAVOR`) is never bought
    // back in the same batch.
    if orders.iter().any(|order| {
        order.kind == "sell"
            && order
                .verb
                .as_deref()
                .is_some_and(|verb| verb.contains("FAVOR="))
    }) {
        return Some("favor_buy_hold:selling");
    }
    let affordable =
        ((state.gold - FAVOR_BUY_GOLD_RESERVE).max(0) as f64 / FAVOR_BUY_PRICE).floor() as i32;
    let amount = shortfall.min(FAVOR_BUY_MAX).min(affordable);
    if amount < FAVOR_BUY_MIN {
        return Some("favor_buy_hold:treasury");
    }
    let seller = state
        .rivals
        .iter()
        .filter(|rival| !rival.at_war)
        .filter(|rival| {
            !orders.iter().any(|order| {
                matches!(order.kind, "sell" | "buy") && order.subject == Some(rival.player as i64)
            })
        })
        .max_by_key(|rival| (rival.dvp.unwrap_or(0), std::cmp::Reverse(rival.player)));
    let Some(seller) = seller else {
        return Some("favor_buy_hold:no_seller");
    };
    orders.push(Order {
        kind: "buy",
        subject: Some(seller.player as i64),
        verb: Some(format!("FAVOR={amount}")),
        pos: Some(((amount as f64 * FAVOR_BUY_PRICE).floor() as i32, 0)),
    });
    None
}

// ★★★★★ THE WORK SOLD TO SEAT THE PERSON. A cultural Great Person the host
// will not let activate anywhere is out of space by the host's own account —
// see `StateGreatPerson::slot_starved`, which asks whether this person can
// REACH a slot rather than how many the empire owns — and #2086's driver
// rightly stands them still while the needs machinery builds more. But capacity is
// bounded (one Amphitheater per city, one Museum per Theater), the person
// produces NOTHING while parked, and Firaxis trades placed Great Works as
// ordinary deal items. So sell ONE placed work of the starved class's own
// object kind to a rich rival at the rival's own valuation: the treasury —
// zero for 103 of 245 turns on measured runs — gets the gold, the freed slot
// re-fills with the idle person's fresh work, and the empire ends with the
// same works standing plus the price of one. The order rides the `sell` arm
// exactly as favor does; the mod's cooldowns meter refusals.
const WORK_SALE_CADENCE: u32 = 6;
const WORK_SALE_PHASE: u32 = 5;
/// ★ Until 2026-08-24 this was 50 against an engine book of 150–400
/// (`great_work_receive_value`: 60 + 32·tourism + 18·culture + 12·faith). The
/// sale exists to free a slot, so the floor stays below the book — but not
/// at a third of its bottom.
const WORK_SALE_FLOOR: i32 = 120;

/// The Great Work object types a slot-consuming class produces — the Rust
/// copy of the control mod's `CivvisGreatWorks.CLASS_OBJECTS` (there is no
/// `GREATWORKOBJECT_ART`; Artists create these four). `None` for classes
/// that do not consume slots.
fn class_sale_objects(class: &str) -> Option<&'static [&'static str]> {
    match class {
        "GREAT_PERSON_CLASS_WRITER" => Some(&["GREATWORKOBJECT_WRITING"]),
        "GREAT_PERSON_CLASS_MUSICIAN" => Some(&["GREATWORKOBJECT_MUSIC"]),
        "GREAT_PERSON_CLASS_ARTIST" => Some(&[
            "GREATWORKOBJECT_SCULPTURE",
            "GREATWORKOBJECT_PORTRAIT",
            "GREATWORKOBJECT_LANDSCAPE",
            "GREATWORKOBJECT_RELIGIOUS",
        ]),
        _ => None,
    }
}

/// Why no work-sale order was appended this turn, for the note; `None` when
/// one was.
fn append_work_sale_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
) -> Option<&'static str> {
    // The trigger is the person, not the treasury: a slot consumer the host
    // says cannot activate anywhere. `slot_starved` is that question asked
    // per plot as well as empire-wide; an older mod's `None` still must NOT
    // trigger a sale on a guess, and does not.
    let starved = state
        .units
        .iter()
        .filter_map(|unit| unit.great_person.as_ref())
        .filter(|person| person.slot_starved())
        .filter_map(|person| person.class.as_deref().and_then(class_sale_objects))
        .next();
    let Some(objects) = starved else {
        return Some("work_sale_hold:no_starved_person");
    };
    if state.turn % WORK_SALE_CADENCE != WORK_SALE_PHASE {
        return Some("work_sale_hold:cadence");
    }
    // One placed work the starved person could replace. Any placed work of
    // the class's own object kind frees a slot that accepted it; base works
    // of one kind carry near-identical yields, so the first is as good as
    // any and the choice stays deterministic.
    let work = state
        .cities
        .iter()
        .flat_map(|city| city.great_works.iter().flatten())
        .find(|work| !work.kind.is_empty() && objects.contains(&work.object.as_str()));
    let Some(work) = work else {
        // Starved with nothing of its own kind placed: the slots holding the
        // empire's works are all foreign kinds (or palace-only), and only
        // building capacity can help. The needs machinery's case, not ours.
        return Some("work_sale_hold:no_matching_work");
    };
    // The richest met major at peace — the favor sale's buyer rule — except
    // the culture front-runner when there is any other choice: our losses
    // are 27-of-71 culture steals, and a Great Work in that rival's museum
    // is tourism for them twice over.
    let candidates: Vec<&civvis::mirror::StateRival> =
        state.rivals.iter().filter(|rival| !rival.at_war).collect();
    let top_culture = candidates
        .iter()
        .max_by(|left, right| {
            left.culture
                .partial_cmp(&right.culture)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|rival| rival.player);
    let buyer = candidates
        .iter()
        .filter(|rival| candidates.len() == 1 || Some(rival.player) != top_culture)
        .max_by(|left, right| {
            let gold = |rival: &civvis::mirror::StateRival| {
                if rival.gold.is_finite() {
                    rival.gold
                } else {
                    0.0
                }
            };
            gold(left)
                .partial_cmp(&gold(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.player.cmp(&left.player))
        });
    let Some(buyer) = buyer else {
        return Some("work_sale_hold:no_buyer");
    };
    // One working deal per rival at a time is the mod's rule; see the border
    // buy's identical yield.
    if orders.iter().any(|order| {
        (order.kind == "sell" || order.kind == "buy") && order.subject == Some(buyer.player as i64)
    }) {
        return Some("work_sale_hold:deal_in_flight");
    }
    orders.push(Order {
        kind: "sell",
        subject: Some(buyer.player as i64),
        verb: Some(format!("{}=1", work.kind)),
        pos: Some((WORK_SALE_FLOOR, 0)),
    });
    None
}

// ★★★★★ AN AID LEAD THAT CANNOT WAIT FOR PRODUCTION.  Firaxis's
// `EMERGENCY_SEND_AID` and `EMERGENCY_SEND_MILITARY_AID` score sources award
// one point for each Gold gift to the emergency target; first place pays two
// Diplomatic Victory points.  `PROJECT_SEND_AID` contributes 200 of those
// points at completion. When no currently queued project can finish before the
// final production window closes, a normal one-way Gold deal can close an
// already-visible gap immediately, provided the target accepts it.
//
// This is deliberately a finish-line fallback, not a treasury-to-score
// conversion.  It acts only in the final three host turns, keeps 100 Gold for
// ordinary emergency purchases, and will never spend more score than the
// official project gives.  The current tracker—not a guessed opponent total—
// sets the exact amount required to move strictly ahead.
#[cfg(test)]
use civvis::ai::player::aid::FINISH_WINDOW as AID_GIFT_FINISH_WINDOW;

/// The share of the treasury a ROUTED peace offer may carry as tribute on
/// its retry, and the most it may ever carry. ★ 2026-08-24: over 42 live
/// runs the seat paid 142 tributes at a median 116 Gold (max 783, ~21,000
/// Gold in all) because the control mod put three quarters of the treasury
/// on every retried offer, whatever the planner's reason for offering — a
/// second front, war fatigue, a religious lane and an envoy reclaim all
/// paid like a rout. Beyond the peace itself such Gold buys only the capped
/// favorable-deal modifier, far less than those offers paid.
/// Now the planner's `peace_routed` says whether the front is lost; only
/// then does the order carry a cap (`x`), and the mod offers at most that.
/// A white offer carries `x = 0`.
const PEACE_TRIBUTE_SHARE: f64 = 0.25;
const PEACE_TRIBUTE_MAX: i64 = 300;

/// The tribute cap a peace order carries: nothing unless the planner is
/// routed on that front, else a quarter of the treasury up to the maximum.
fn peace_tribute_cap(state: &civvis::mirror::StateSnapshot, routed: bool) -> i32 {
    if !routed {
        return 0;
    }
    ((state.gold as f64 * PEACE_TRIBUTE_SHARE).floor() as i64).clamp(0, PEACE_TRIBUTE_MAX) as i32
}

/// A host-reported Send Aid project already reaches the current emergency's
/// deadline.  The production estimate is authoritative only when finite and
/// non-negative; an omitted or unknown estimate must not suppress a finish-line
/// gift on a guess.  Both humanitarian emergency kinds use `PROJECT_SEND_AID`.
fn aid_project_finishes_by_deadline(
    state: &civvis::mirror::StateSnapshot,
    emergency: &civvis::mirror::StateEmergency,
) -> bool {
    let deadline = emergency.turns_left as f64;
    state.cities.iter().any(|city| {
        city.producing.as_deref() == Some("PROJECT_SEND_AID")
            && city.production_turns.is_finite()
            && city.production_turns >= 0.0
            && city.production_turns <= deadline + f64::EPSILON
    })
}

/// Append the direct Gold fallback for an Aid Request that can be won before
/// it closes.  The target must be a currently exported peaceful major: that
/// means the Lua side has a met player id it can validate rather than a guessed
/// emergency recipient.  Existing sales and border purchases to that target
/// yield to a point-winning gift; a war, peace, or delegation order does not,
/// because it changes the diplomatic state the gift relies upon.
fn append_aid_gift_order(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
) -> Option<&'static str> {
    let Some(emergencies) = state.emergencies.as_ref() else {
        return Some("aid_gift_hold:no_tracker");
    };
    let opportunities: Vec<_> = emergencies
        .iter()
        .map(|emergency| civvis::ai::player::aid::Opportunity {
            kind: &emergency.kind,
            target: emergency.target,
            turns_left: emergency.turns_left,
            begun: emergency.begun,
            member: emergency.ours.member,
            ours: emergency.ours.score,
            leader: emergency.ours.score.map(|_| {
                emergency
                    .scores
                    .iter()
                    .filter(|entry| entry.player != i64::from(state.seat.local_player))
                    .map(|entry| entry.score)
                    .filter(|score| score.is_finite())
                    .fold(0.0, f64::max)
            }),
            peaceful_met_target: state
                .rivals
                .iter()
                .any(|rival| rival.player as i64 == emergency.target && !rival.at_war),
            project_finishes: aid_project_finishes_by_deadline(state, emergency),
        })
        .collect();
    let Some((emergency, amount)) =
        civvis::ai::player::aid::choose(state.gold as f64, &opportunities)
    else {
        return Some("aid_gift_hold:no_finishable_lead");
    };
    let target = emergency.target;
    if orders.iter().any(|order| {
        order.subject == Some(target)
            && matches!(
                order.kind,
                "war" | "peace" | "delegation" | "denounce" | "friendship" | "alliance"
            )
    }) {
        return Some("aid_gift_hold:diplomacy_conflict");
    }
    // A one-working-deal-per-rival host cannot carry an ordinary trade and the
    // finish-line gift together.  The gift is already affordable and converts
    // directly into a two-DVP first-place attempt, so it has priority over an
    // unrelated sale or passage purchase to the same recipient this frame.
    orders.retain(|order| !(order.subject == Some(target) && matches!(order.kind, "sell" | "buy")));
    orders.insert(
        0,
        Order {
            kind: "aid_gift",
            subject: Some(target),
            verb: Some(emergency.kind.trim().to_string()),
            pos: Some((amount as i32, 0)),
        },
    );
    None
}

/// How many `envoy` orders one turn may carry. The planning board spends every
/// held envoy in one pass (`advanced_envoys` loops until `envoys_free` is 0),
/// and a seat that has banked fifty of them — every Settler game to date —
/// would otherwise issue fifty `GIVE_INFLUENCE_TOKEN` operations the first
/// turn the lane opens. Eight a turn drains that bank in a week of turns, keeps
/// the actuation path's blast radius small while it earns its record, and the
/// remainder is not lost: next turn's board re-plans from the fresh export with
/// the placed ones already standing. The greedy order is preserved — the first
/// eight the plan chose are the eight that cross.
const ENVOY_ORDERS_PER_TURN: usize = 8;

/// Keep the first `ENVOY_ORDERS_PER_TURN` envoy orders in plan order and defer
/// the rest to next turn's plan. Every other order kind passes untouched.
fn bound_envoy_orders(orders: Vec<Order>) -> (Vec<Order>, usize) {
    let mut kept = 0;
    let mut deferred = 0;
    let orders = orders
        .into_iter()
        .filter(|order| {
            if order.kind != "envoy" {
                return true;
            }
            if kept < ENVOY_ORDERS_PER_TURN {
                kept += 1;
                true
            } else {
                deferred += 1;
                false
            }
        })
        .collect();
    (orders, deferred)
}

/// Translate CIVVIS's immediate Great Person semantics into Firaxis's physical
/// unit workflow. The host supplies both the current activation verdict and every
/// legal activation plot; choosing the nearest legal plot is path actuation, not a
/// second strategy layer.
/// Why a Great Person standing on the board produced no order this turn.
///
/// ⚠ These are not the same thing and the single counter they replace could not
/// tell them apart. `on_cooldown` is the deliberate, benign case documented
/// below: the person already occupies an activation plot and Firaxis has not
/// re-offered the command yet, so waiting is correct and the next export will
/// activate them. `no_activation_plot` is a **loss** — the host offers nowhere
/// at all to use this individual, and they will stand there for the rest of the
/// game unless the empire builds the district their class needs.
///
/// The distinction is not cosmetic for the science question. A Great Scientist
/// activates on a Campus; an empire with one Campus and two Scientists has one
/// of them permanently stranded, and the merged counter reported that
/// identically to a Writer waiting a single frame. Live brain logs show this
/// reaching 3 and 6, and nothing said which kind.
#[derive(Default, Clone, Copy)]
struct GreatPersonStall {
    on_cooldown: usize,
    no_activation_plot: usize,
    /// Slot-consuming people (Writers, Artists, Musicians) the host says have
    /// ZERO compatible empty Great Work slots anywhere. Their highlight plots
    /// still list the district, so before this counter existed they sat in
    /// `on_cooldown` forever — seven of them, thirty-plus turns, on run
    /// civvis-20260817T010950Z. This bucket is a build order the mirror's
    /// needs machinery is already placing, not a wait.
    no_empty_slot: usize,
    /// Founded zero-charge Prophets sent for retirement this frame.
    retired_prophets: usize,
    /// Generals kept with a front of the army this frame
    /// (`generals-lead-the-army`); not a stall, so outside `total`.
    generals_leading: usize,
}

impl GreatPersonStall {
    fn total(self) -> usize {
        self.on_cooldown + self.no_activation_plot + self.no_empty_slot
    }
}

#[cfg(test)]
fn great_person_orders(state: &civvis::mirror::StateSnapshot) -> (Vec<Order>, GreatPersonStall) {
    great_person_orders_led(state, false)
}

/// [`great_person_orders`], with `generals-lead-the-army` deciding whether a
/// General walks to a front of our land army instead of retiring. See
/// `civvis_orders/generals_lead_the_army.rs`.
fn great_person_orders_led(
    state: &civvis::mirror::StateSnapshot,
    lead_with_generals: bool,
) -> (Vec<Order>, GreatPersonStall) {
    let mut orders = Vec::new();
    let mut stall = GreatPersonStall::default();
    let led = if lead_with_generals {
        generals_lead_the_army::plan(state)
    } else {
        std::collections::BTreeMap::new()
    };
    for unit in &state.units {
        let Some(person) = &unit.great_person else {
            continue;
        };
        // `generals-lead-the-army`: a General kept for a front walks to its
        // post (or holds beside it) and is never retired while it leads.
        if let Some(post) = led.get(&unit.id) {
            stall.generals_leading += 1;
            // A held General is still named: the control mod's own Great
            // Person pass retires any General no row claims (live 093211Z t55).
            orders.push(match post {
                Some(tile) => Order {
                    kind: "unit",
                    subject: Some(unit.id),
                    verb: Some("MOVE_TO".to_string()),
                    pos: Some(*tile),
                },
                None => Order {
                    kind: "unit",
                    subject: Some(unit.id),
                    verb: Some("KEEP_GREAT_PERSON".to_string()),
                    pos: None,
                },
            });
            continue;
        }
        // ★★★★ A PROPHET WHOSE RELIGION IS ALREADY FOUNDED, WITH NO CHARGE LEFT,
        // IS A GHOST — and a ghost blocks a hex for the rest of the game. This
        // build leaves the zero-charge unit on the map after the religion is
        // created; the mirror does not model Great People, so CIVVIS paths
        // other units through its hex, and `defer_great_person_plot_conflicts`
        // then drops every one of those orders. Measured on run
        // civvis-20260815T210845Z: Confucius (charges 0, Buddhism founded)
        // stood beside Rome from ~t45; the settler built at t83 received no
        // order for 21 turns (`deferred_activation_plot_conflicts=2` every
        // turn) while the journal said "marching". Run T190904Z carried Simon
        // Peter the same way for 100+ turns. The control mod already retires
        // such a Prophet in its own turn routine (`UNITCOMMAND_DELETE` once the
        // religion is created and the charge is spent), but under
        // `--civvis-decides` that routine never runs for a unit CIVVIS owns —
        // so the bridge asks for the same retirement here.
        if is_founded_zero_charge_prophet(unit, person, state) {
            orders.push(Order {
                kind: "unit",
                subject: Some(unit.id),
                verb: Some("DELETE".to_string()),
                pos: None,
            });
            stall.retired_prophets += 1;
            continue;
        }
        // Firaxis's GetActionCharges is not the activation authority for every
        // class. Great Writers report zero while CanStartCommand is true and
        // still have works to create. Trust the host's command verdict first;
        // the old charge gate left Qu Yuan in the capital for 76 turns and let
        // a second Writer stack on top of him.
        if person.can_activate {
            orders.push(Order {
                kind: "unit",
                subject: Some(unit.id),
                verb: Some("ACTIVATE_GREAT_PERSON".to_string()),
                pos: None,
            });
            continue;
        }
        // No slot this person can REACH is not a cooldown and not a marching
        // problem: nothing this unit can do fixes it, and walking to a
        // highlighted-but-full district is motion without progress. Stand
        // still and let the mirror's activation-needs machinery build the
        // capacity — which it now actually does for this case, because the
        // same predicate opens that door. See `StateGreatPerson::slot_starved`
        // and the nine people it found idling a whole game.
        if person.slot_starved() {
            stall.no_empty_slot += 1;
            continue;
        }
        // A command that just resolved can remain unavailable for the rest of
        // this Firaxis frame. If the person already occupies an activation
        // plot THAT CAN STILL TAKE THE WORK, wait for the next export instead
        // of sending them toward a different city. Qu Yuan otherwise bounced
        // Theater -> capital on the cooldown frame after creating his first
        // work. `slot_open == Some(false)` is the one standing-still case that
        // is NOT a cooldown: the engine highlights a cultural person's
        // district whether or not a compatible slot is free, and waiting on a
        // known-full tile is the wedge that stacked eleven people on (25,23)
        // for the whole of run civvis-20260817T010950Z while twelve empty
        // slots stood 2-10 tiles away. Older exports send no `slot_open`;
        // their `None` keeps the old benefit of the doubt.
        if person
            .activation_plots
            .iter()
            .any(|plot| plot.distance == 0 && plot.slot_open != Some(false))
        {
            stall.on_cooldown += 1;
            continue;
        }
        // Prefer the tile a matching empty slot is KNOWN to stand on, then an
        // unknown tile (a wonder's — the host cannot map those), by distance
        // within each. A tile known to hold no matching slot is never a
        // destination: walking there is the wedge this ranking exists to end.
        let target = person
            .activation_plots
            .iter()
            .filter(|plot| plot.slot_open != Some(false))
            .min_by_key(|plot| (plot.slot_open != Some(true), plot.distance, plot.y, plot.x));
        if let Some(target) = target {
            orders.push(Order {
                kind: "unit",
                subject: Some(unit.id),
                verb: Some("MOVE_TO".to_string()),
                pos: Some((target.x, target.y)),
            });
        } else {
            // The host offered no plot anywhere — or every highlighted tile is
            // known-full while the empty slots sit somewhere unmappable (a
            // wonder). Either way marching cannot help; this individual waits
            // for the empire to build the district or slot their class needs.
            stall.no_activation_plot += 1;
        }
    }
    (orders, stall)
}

/// A Great Prophet that can no longer do anything: the seat has founded its
/// religion, the unit reports no charge, cannot activate and has no activation
/// plot. Everything else — a Prophet still to found, a Writer whose charges read
/// zero while `CanStartCommand` says otherwise — is left exactly as before.
fn is_founded_zero_charge_prophet(
    unit: &civvis::mirror::StateUnit,
    person: &civvis::mirror::StateGreatPerson,
    state: &civvis::mirror::StateSnapshot,
) -> bool {
    unit.kind.eq_ignore_ascii_case("UNIT_GREAT_PROPHET")
        && person.charges == 0
        && !person.can_activate
        && person.activation_plots.is_empty()
        && state
            .founded_religion
            .as_deref()
            .is_some_and(|religion| !religion.trim().is_empty())
}

/// Reserve Great Person tiles against civilian conflicts and activation races.
///
/// Firaxis queues unit operations. `CanStartCommand` can therefore approve a Great
/// Person retirement and a later move onto that person's hex even though the move
/// will make the retirement illegal by the time the engine resolves it. Hanno the
/// Navigator hit this exact race three times while a Galley oscillated through his
/// Harbor. An inactive Great Person also blocks another civilian from entering its
/// plot: Hildegard spent forty turns trying to enter a Holy Site occupied by an
/// unspent Prophet. Combat units can share an idle person's tile; reserve their
/// entry only when this batch submits an activation. Preserve the full reservation
/// for civilian, support and unobserved movers.
fn defer_great_person_plot_conflicts(
    orders: Vec<Order>,
    state: &civvis::mirror::StateSnapshot,
) -> (Vec<Order>, usize) {
    let activations = orders
        .iter()
        .filter(|order| {
            order.kind == "unit" && order.verb.as_deref() == Some("ACTIVATE_GREAT_PERSON")
        })
        .filter_map(|order| order.subject)
        .collect::<std::collections::BTreeSet<_>>();
    let people = state
        .units
        .iter()
        .filter(|unit| {
            unit.great_person.as_ref().is_some_and(|person| {
                // A founded zero-charge Prophet is being retired, not activated:
                // there is no retirement race to protect on its hex, and
                // reserving it is exactly what black-holed the settler's exit
                // from Rome for 21 turns on run civvis-20260815T210845Z.
                !is_founded_zero_charge_prophet(unit, person, state)
            })
        })
        .map(|unit| (unit.id, (unit.x, unit.y), activations.contains(&unit.id)))
        .collect::<Vec<_>>();
    let mut deferred = 0;
    let orders = orders
        .into_iter()
        .filter(|order| {
            // The native combat and civilian slots coexist. Resolve military
            // identity from the observed actor, not a translated unit name.
            // Only physical MOVE_TO orders get this exemption; other uses of
            // a destination retain the existing reservation.
            let military_move = order.verb.as_deref() == Some("MOVE_TO")
                && state.units.iter().any(|unit| {
                    order.subject == Some(unit.id) && (unit.combat > 0.0 || unit.ranged > 0.0)
                });
            // A Trade Route uses `pos` to name its destination city, not a
            // physical hex the Trader will enter in this Lua batch.
            let conflicts = order.kind == "unit"
                && order.verb.as_deref() != Some("TRADE_ROUTE")
                && people.iter().any(|(person, pos, activating)| {
                    order.subject != Some(*person)
                        && order.pos == Some(*pos)
                        && (*activating || !military_move)
                });
            if conflicts {
                deferred += 1;
            }
            !conflicts
        })
        .collect();
    (orders, deferred)
}

/// Send the complete post-plan policy set as one host transaction.
///
/// A replacement is two CIVVIS actions (`UnslotPolicy`, then `SlotPolicy`), but
/// Firaxis accepts the government screen's clear/add lists atomically. Sending
/// only the second action asks it to add a card to a deck that is still full.
fn policy_deck_order(game: &civvis::game::Game, pid: usize) -> Order {
    let policies = game.players[pid]
        .policies
        .iter()
        .map(|policy| format!("POLICY_{}", policy.as_str().to_ascii_uppercase()))
        .collect::<Vec<_>>()
        .join(",");
    Order {
        kind: "policy_deck",
        subject: None,
        verb: Some(policies),
        pos: None,
    }
}

/// Cards in a speculative deck which Civilization VI cannot accept yet.
///
/// `Ai::take_turn` advances a throwaway board through a complete CIVVIS turn.
/// That can finish the active civic on `planned_game` and make the next civic's
/// cards look unlocked even though the authoritative mirror is still on the
/// previous host export.  The control mod checks `IsPolicyUnlocked` at the
/// moment it receives the order, so sending that deck creates a refusal loop
/// until the next export catches up.  Cards already slotted, or available on
/// the authoritative board, are legal; anything else must wait for that export.
fn policy_deck_cards_not_currently_legal(
    planned_game: &civvis::game::Game,
    authoritative_game: &civvis::game::Game,
    pid: usize,
) -> Vec<String> {
    let Some(planned_player) = planned_game.players.get(pid) else {
        return Vec::new();
    };
    let Some(authoritative_player) = authoritative_game.players.get(pid) else {
        return Vec::new();
    };
    let available = authoritative_game.available_policies(pid);
    planned_player
        .policies
        .iter()
        .filter(|policy| {
            !authoritative_player.policies.contains(*policy)
                && !available.iter().any(|candidate| candidate == *policy)
        })
        .map(|policy| format!("POLICY_{}", policy.as_str().to_ascii_uppercase()))
        .collect()
}

impl Order {
    fn to_json(&self) -> String {
        let mut parts = vec![format!("\"kind\":{}", quote(self.kind))];
        match self.subject {
            Some(value) => parts.push(format!("\"subject\":{value}")),
            None => parts.push("\"subject\":null".to_string()),
        }
        match &self.verb {
            Some(value) => parts.push(format!("\"verb\":{}", quote(value))),
            None => parts.push("\"verb\":null".to_string()),
        }
        match self.pos {
            Some((x, y)) => {
                parts.push(format!("\"x\":{x}"));
                parts.push(format!("\"y\":{y}"));
            }
            None => {
                parts.push("\"x\":null".to_string());
                parts.push("\"y\":null".to_string());
            }
        }
        format!("{{{}}}", parts.join(","))
    }
}

/// Add deferred production choices for host queues that will finish this turn.
///
/// The ordinary `produce` orders still actuate immediately.  These hints are a
/// separate, non-mutating order kind: the Lua controller stores them and uses
/// one only if the corresponding city later raises its production blocker.  A
/// city with a direct choice is intentionally excluded because replacing its
/// queue would make the hint race the decision it is meant to follow.
fn append_next_production_hints(
    ai: &civvis::ai::AdvancedAi,
    planned_game: &civvis::game::Game,
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    ours: &mut std::collections::BTreeMap<i64, String>,
) -> usize {
    let mut hinted = 0;
    // ★★★★★ THE HINT NEVER FIRED, IN ANY RECORDED GAME. `planned_game` is the
    // board the agent has already TAKEN ITS TURN ON, and `take_turn` ends with
    // `EndTurn`, so `current` is the next seat; `preview_live_production`
    // clones that board and asks `advanced_production` to fill the cleared
    // queue, and every `Produce` it tries comes back "not your turn". The
    // preview answered `None` on every call, and the deferred hint — the one
    // thing that stops the mod's hand-written ladder from answering the
    // production prompt itself — was emitted zero times across the whole
    // 2026-08-16..19 ladder (0 `produce_next` orders, 0 `build_hint` events,
    // 70–174 ladder `build` picks per game, ~85% of them displaced by
    // CIVVIS's own order on the following turn). Give the preview a board on
    // which it is still this seat's turn: a throwaway copy, never applied.
    let mut preview_board = planned_game.clone();
    preview_board.current = 0;
    for city in &state.cities {
        if !host_production_finishes_this_turn(city)
            || orders
                .iter()
                .any(|order| order.kind == "produce" && order.subject == Some(city.id))
        {
            continue;
        }
        let Some(cid) = mirror_state.cid_of.get(&city.id).copied() else {
            continue;
        };
        let Some(item) = ai.preview_live_production(&preview_board, 0, cid) else {
            continue;
        };
        let Some(name) = civ6_live_build_name(&item, planned_game) else {
            continue;
        };
        // The hint is a CIVVIS choice that the host has not consumed yet.  Keep
        // a marker in the persistent production ownership map so the next mirror
        // does not classify the still-running queue as foreign and immediately
        // release it.  `settle_deferred_production_hints` removes the marker once
        // the host reports the hinted item as its current queue.
        ours.insert(city.id, format!("{DEFERRED_PRODUCTION_PREFIX}{name}"));
        orders.push(Order {
            kind: "produce_next",
            subject: Some(city.id),
            verb: Some(name),
            pos: civ6_build_pos(&item),
        });
        hinted += 1;
    }
    hinted
}

/// Reconcile deferred production ownership against the next authoritative host
/// frame.  A matching queue means the hint was consumed and becomes ordinary
/// CIVVIS ownership.  A different queue is released only when it is not itself
/// still approaching its blocker; this keeps a slow or lagging host queue intact
/// for another frame instead of turning a timing miss into an early replacement.
fn settle_deferred_production_hints(
    ours: &mut std::collections::BTreeMap<i64, String>,
    state: &civvis::mirror::StateSnapshot,
) -> (usize, usize) {
    let pending: Vec<(i64, String)> = ours
        .iter()
        .filter_map(|(city, value)| {
            value
                .strip_prefix(DEFERRED_PRODUCTION_PREFIX)
                .map(|name| (*city, name.to_string()))
        })
        .collect();
    let mut consumed = 0;
    let mut expired = 0;
    for (city_id, expected) in pending {
        let Some(city) = state.cities.iter().find(|city| city.id == city_id) else {
            ours.remove(&city_id);
            expired += 1;
            continue;
        };
        match city.producing.as_deref() {
            Some(current) if current == expected => {
                ours.insert(city_id, expected);
                consumed += 1;
            }
            Some(_) if host_production_finishes_this_turn(city) => {
                // The old queue is still near its blocker; leave the lease in
                // place for the host's next production callback.
            }
            Some(_) | None => {
                // A different, non-finishing queue means the host answered the
                // blocker without consuming this lease.  Let normal foreign
                // production release recover the city on this frame.
                ours.remove(&city_id);
                expired += 1;
            }
        }
    }
    (consumed, expired)
}

/// One turn's decision, against a mirror and an agent that PERSIST across turns.
///
/// ★★★★★ WHY PERSISTENCE IS THE WHOLE POINT. A fresh `AdvancedAi` on a fresh board
/// throws away its strategic plan, its force groups and every settler's destination
/// each turn, so only what is locally optimal survives — and standing still is almost
/// always locally optimal. Measured on run civvis-20260730T120107Z: 28 units at turn
/// 108 and the FURTHEST one 7 tiles from the capital, plateaued since turn 74. Nothing
/// ever went looking for the enemy, `met` stopped at 2, no rival city was ever seen,
/// and an army of 23 had nothing to attack. The settler oscillating between two tiles
/// for twenty turns is the same defect in miniature.
fn remove_active_route_traders_from_plan(
    planned_game: &mut civvis::game::Game,
    mirror_state: &civvis::mirror::LiveMirror,
) {
    mirror_state.prune_active_trade_route_traders(planned_game);
}

/// Release the production queue of any city building something CIVVIS did not
/// choose, so it decides afresh instead of adopting the mod's pick as its own.
///
/// ⚠⚠ THIS IS WHY THE LADDER STILL BUILDS 72% OF EVERYTHING. Measured on run
/// `civvis-20260802T064240Z`, applied orders over 144 turns:
///
/// ```text
/// unit 395    civic 144    research 144    produce 22    policy 16
/// ```
///
/// Research and civic go out every single turn. Production goes out on **19 turns
/// of 144 — 13%**. CIVVIS's build orders are not being refused any more (`no_params`
/// is gone from the refusal ledger entirely); it is **not issuing them**.
///
/// The loop: `rebuild_from_state` seeds each city's queue from Civilization VI's
/// `producing`, `--fresh-board` reruns that every turn, and CIVVIS produces only for
/// a city whose queue it believes is empty. So when `driveProduction` answers
/// `ENDTURN_BLOCKING_PRODUCTION` from its own ladder, the next turn hands CIVVIS that
/// choice as work in progress, CIVVIS says nothing, the item completes, the queue
/// empties, and the ladder picks again. CIVVIS never gets a word in.
///
/// ⚠ The seeding itself is correct and must stay — without it CIVVIS re-chooses from
/// scratch every turn and alternates Builder / Monument / Campus, which is the defect
/// the seed was added to fix. What was missing is that it cannot tell CIVVIS's own
/// plan from the ladder's.
///
/// ⚠ Applied to the THROWAWAY planning board, not the authoritative mirror, so the
/// board shown to the next decision stays the last exported Civ VI state.
///
/// **Self-limiting by construction.** The override only fires while the game is
/// building something CIVVIS did not order; once its choice takes, `producing`
/// matches and the queue is seeded normally again. A choice the host keeps refusing
/// is already routed around by `blocked_production`, which `sync` refreshes.
/// Adopt whatever the host is building right now as CIVVIS's own plan. See
/// the persistent server's first turn: a fresh process holds an empty `ours`,
/// so without this every city's work in progress reads as foreign to
/// [`release_foreign_production`] and is re-decided the moment the decider
/// restarts. Existing entries — the plan this process has already ordered —
/// are kept.
fn adopt_host_production(
    ours: &mut std::collections::BTreeMap<i64, String>,
    state: &civvis::mirror::StateSnapshot,
) -> usize {
    let mut adopted = 0;
    for city in &state.cities {
        if let Some(producing) = city.producing.as_deref() {
            if let std::collections::btree_map::Entry::Vacant(e) = ours.entry(city.id) {
                e.insert(producing.to_string());
                adopted += 1;
            }
        }
    }
    adopted
}

/// A host queue can advance along its unit's upgrade chain when research
/// completes. That preserves our military commitment even though its type ID
/// changes (for example Trebuchet -> Bombard at Metal Casting).
fn production_is_unit_upgrade(rules: &civvis::rules::Rules, ordered: &str, observed: &str) -> bool {
    if !ordered.starts_with("UNIT_") || !observed.starts_with("UNIT_") {
        return false;
    }
    let Some((_, spec)) = rules
        .units
        .iter()
        .find(|(name, _)| civ6_unit_type(name) == ordered)
    else {
        return false;
    };
    let mut next = spec.upgrade_to.as_ref();
    // Bound traversal even if a modded ruleset contains a cycle.
    for _ in 0..rules.units.len() {
        let Some(name) = next else { return false };
        if civ6_unit_type(name) == observed {
            return true;
        }
        next = rules
            .units
            .get(name)
            .and_then(|spec| spec.upgrade_to.as_ref());
    }
    false
}

fn release_foreign_production(
    planned_game: &mut civvis::game::Game,
    cid_of: &std::collections::BTreeMap<i64, u32>,
    state: &civvis::mirror::StateSnapshot,
    ours: &std::collections::BTreeMap<i64, String>,
) -> usize {
    let mut released = 0;
    for city in &state.cities {
        let Some(producing) = city.producing.as_deref() else {
            continue;
        };
        if ours
            .get(&city.id)
            .is_some_and(|owned| owned.starts_with(DEFERRED_PRODUCTION_PREFIX))
            || ours.get(&city.id).map(String::as_str) == Some(producing)
            || ours.get(&city.id).is_some_and(|owned| {
                production_is_unit_upgrade(&planned_game.rules, owned, producing)
            })
        {
            continue;
        }
        let Some(cid) = cid_of.get(&city.id).copied() else {
            continue;
        };
        if planned_game.pause_city_production(cid) {
            released += 1;
        }
    }
    released
}

#[cfg(test)]
use civvis::ai::finishing::finish_live_war_units_excluding;
#[cfg(test)]
use civvis::ai::finishing::{finish_live_war_units, live_finishing_candidates};

/// The skipped-reason key for a move whose destination is the unit's own tile.
///
/// ★★★★ NAME THE ANONYMOUS COUNT. `self_tile_move` is the largest single waste
/// class on the ladder — **25,387 dropped orders across 43 live runs, on 3,803
/// turns with at least one** — and as a bare total it cannot say whether that is
/// one stuck Settler, the whole army, or a Trader with nowhere to go. Those are
/// completely different repairs, and this project has three times found the
/// standing hypothesis about an anonymous count to be wrong: `no_params` x221
/// was one district, `move_refused` x33 was one trader, and 146 blocked upgrades
/// were two resource gaps rather than gold.
///
/// It answered immediately. Replaying five finished runs through the fixed
/// binary: **249 self-tile moves, 100% military** — warrior 59, heavy_chariot
/// 53, field_cannon 52, pike_and_shot 32, archer 23, and a tail of galley,
/// crossbowman, cuirassier, trebuchet, spec_ops and slinger. **Zero settlers,
/// zero builders, zero traders**, against a prediction of ~2% settlers.
///
/// ⚠ Keep the `self_tile_move` PREFIX. Existing readers match on it, and the
/// unit kind is a suffix so a total is still recoverable by prefix.
fn self_tile_move_key(mirror_state: &civvis::mirror::LiveMirror, unit: u32) -> String {
    mirror_state
        .game
        .units
        .get(&unit)
        .map(|unit| format!("self_tile_move:{}", unit.kind))
        .unwrap_or_else(|| "self_tile_move".to_string())
}

/// Turn one live-bridge treatment back off, by the same kebab name the Elo
/// registry uses for its `live_without_*` arms.
///
/// ★★★★★ THE LIVE BRIDGE HAS NEVER BEEN MEASURED, AND THIS IS WHY. Every repair
/// in `AdvancedAi::enable_live_bridge` is registered in `src/elo.rs` with a
/// matching `live_without_<flag>` arm, so `ai_eval` can isolate it — but
/// `ai_eval` plays headless CIVVIS, where several of those mechanisms cannot act
/// at all. `closed_borders` and `host_observed` are populated only by
/// `mirror.rs` and are empty by construction in self-play; the whole 20-axis
/// composite scores **+9 Elo (CI −53..+71)** against plain `advanced`, and
/// individual arms come back at ±0 with 40 of 40 paired maps neutral because the
/// branch under test fires roughly **18 times per 10,000** tactical steps there.
///
/// Meanwhile the LIVE harness had no way to hold a treatment off at all:
/// `civ6_play.py` starts `civvis_orders --serve` and takes whatever the binary
/// does. So the one regime where these mechanisms actually fire was also the one
/// regime in which no control arm could be run. Every live claim in this lane is
/// therefore an instrumented mechanism check — foreign-tile move orders
/// 115/432 → 0/349, embarkation leaving the refusal census — and never a paired
/// outcome.
///
/// `--without <treatment>` closes that. `--with <treatment>` is its deliberate
/// complement for a treatment the new genome's ledger holds off: it restores
/// one live-universe row for a named experiment without changing deployment.
/// Repeat either flag to select several rows. An unknown name is a hard error
/// rather than a warning: a typo that silently produced an identical arm would
/// report a null and look exactly like a real one.
fn withhold_live_treatment(ai: &mut civvis::ai::AdvancedAi, treatment: &str) -> Result<(), String> {
    // ⚠⚠ THIS WAS A SECOND LIST, AND IT WAS SHORTER THAN THE FIRST.
    //
    // The gene registry (`src/ai/advanced/genes.rs`) carries the disabler for
    // every `live()` gene. This binary once wrote the names out again: 57
    // hand-written arms against 68 rows, so ELEVEN SHIPPED LIVE TREATMENTS HAD
    // NO CONTROL on the only harness where they fire. The usage string was a
    // THIRD copy and shorter still. A lookup cannot drift from the registry by
    // construction: a live gene added there reaches this binary and its usage
    // line at the same moment.
    match civvis::ai::GENES
        .iter()
        .find(|gene| withholdable(gene) && gene.tag == treatment)
    {
        Some(gene) => {
            (gene.disable)(ai);
            Ok(())
        }
        // An unknown name stays a hard error rather than a warning: a typo that
        // silently produced a control identical to the treatment would report a
        // null that looks exactly like a real one.
        None => Err(format!(
            "unknown --without treatment {treatment:?}; this binary can withhold: {}",
            withholdable_treatments()
        )),
    }
}

/// Whether `--without` can form an off arm for this gene: every live gene
/// (the repairs and the host-only adapters, which ship on), and every opt-in
/// the ledger turns on. An opt-in the ledger leaves off is already off and a
/// production gene has no live control at all, so neither is an arm; naming
/// one stays the hard error above rather than a silent no-op control.
/// `tools/genes.py arm` mirrors this rule in Python so the ladder can assign a
/// screen's arm before a build (`docs/LIVE_SCREEN.md`).
fn withholdable(gene: &civvis::ai::Gene) -> bool {
    gene.live()
        || (gene.opt_in() && civvis::ai::gene_ledger::ledger_default_on(gene.tag) == Some(true))
}

/// Every treatment `--without` accepts, in table order, for the usage line.
fn withholdable_treatments() -> String {
    civvis::ai::GENES
        .iter()
        .filter(|gene| withholdable(gene))
        .map(|gene| gene.tag)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resolve the genes an explicit `--with` arm may seat: a ledger-held live
/// treatment to restore, or a held-off production opt-in to add.
///
/// The live universe already set the live flags before the ledger withheld
/// them, so skipping just that withholding restores the exact named gene
/// without a second hand-written enable table. An opt-in was never in that
/// universe — seating one is an addition, and it is the only route a gene
/// priced on the arena has to the live board before the whole-game screen
/// answers (`docs/DOCTRINE_ARENA.md`, "The gate for a tactical gene").
///
/// A caller can also name a treatment that already ships in deployment. That
/// is an idempotent part of a wider profile, not a reason to kill the live
/// decider: retain only the rows that actually change the genome. A list made
/// entirely of deployed rows remains an error, since it is a mislabeled
/// no-op arm rather than a force-on experiment.
fn forced_live_treatments(forced: &[String]) -> Result<Vec<&'static str>, String> {
    let mut selected = Vec::new();
    let mut already_deployed = Vec::new();
    for treatment in forced {
        let tag = match civvis::ai::GENES
            .iter()
            .find(|gene| gene.tag == treatment)
            .map(|gene| gene.tag)
        {
            Some(tag)
                if civvis::ai::gene_ledger::ledger_held_live_treatment(tag)
                    || civvis::ai::gene_ledger::ledger_held_opt_in(tag) =>
            {
                tag
            }
            Some(tag) => {
                if civvis::ai::gene_ledger::deployment_treatments().contains(&tag) {
                    if !already_deployed.contains(&tag) {
                        already_deployed.push(tag);
                    }
                    continue;
                }
                return Err(format!(
                    "--with treatment {treatment:?} is not a ledger-held live treatment or \
                     held-off opt-in (tag {tag:?}); this binary can force: {}",
                    forceable_live_treatments()
                ));
            }
            None => {
                return Err(format!(
                    "unknown --with treatment {treatment:?}; this binary can force: {}",
                    forceable_live_treatments()
                ));
            }
        };
        if !selected.contains(&tag) {
            selected.push(tag);
        }
    }
    if selected.is_empty() && !already_deployed.is_empty() {
        return Err(format!(
            "--with treatment(s) {} already ship in the deployment genome; name at least one \
             ledger-held live treatment or held-off opt-in to form a distinct arm{}",
            already_deployed
                .iter()
                .map(|tag| format!("{tag:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            host_only_already_on(&already_deployed)
        ));
    }
    Ok(selected)
}

/// The clause a refused `--with` carries when a named row is host-only.
///
/// A `Kind::HostOnly` gene is on in every live seat: `enable_live_bridge_universe`
/// turns on every `live()` gene, and `apply_gene_ledger` withholds only a tag
/// the ledger reads as off, which an unscreened row never is. `genes.py list`
/// printed such rows as `off` (the deployment genome holds only screenable
/// tags), and that reading put host-only tags into `~/.civvis-live-force-on`
/// twice (2026-08-30, 09-01). Say what the seat already plays and which flag
/// moves it. Empty when no named row is host-only.
fn host_only_already_on(tags: &[&str]) -> String {
    let host_only: Vec<String> = tags
        .iter()
        .filter(|tag| civvis::ai::gene(tag).is_some_and(|gene| gene.host_only()))
        .map(|tag| format!("{tag:?}"))
        .collect();
    if host_only.is_empty() {
        return String::new();
    }
    format!(
        "; {} {} host-only and already on in every live seat (the live bridge turns on \
         every host-only gene and no screen row can hold one off) — `--without <tag>` \
         is what withholds one",
        host_only.join(", "),
        if host_only.len() == 1 { "is" } else { "are" }
    )
}

/// Every name a live `--with` arm can seat, in canonical registry order.
fn forceable_live_treatments() -> String {
    civvis::ai::gene_ledger::forceable_treatments().join(", ")
}

/// Reconstruct the exact live genome for each decision. The default retains
/// `enable_live_bridge` verbatim; an explicit arm begins from the same
/// universe and asks the ledger to leave only validated named rows on.
fn configure_live_bridge(
    ai: &mut civvis::ai::AdvancedAi,
    forced_on: &[&str],
    withheld: &[String],
) -> Result<(), String> {
    if forced_on.is_empty() {
        ai.enable_live_bridge();
    } else {
        ai.enable_live_bridge_universe();
        ai.apply_gene_ledger_with_forced_live(forced_on);
    }
    for treatment in withheld {
        withhold_live_treatment(ai, treatment)?;
    }
    Ok(())
}

/// The configured arm, after validated explicit withholds, recorded at startup.
fn configured_live_treatments(forced_on: &[&str], withheld: &[String]) -> Vec<&'static str> {
    let mut treatments = civvis::ai::gene_ledger::deployment_treatments_with_forced_live(forced_on);
    treatments.retain(|tag| !withheld.iter().any(|treatment| treatment.as_str() == *tag));
    treatments
}

/// The immutable live-arm identity. Keeping it as one value prevents a new
/// experimental gene from growing the already busy turn-decider signature.
#[derive(Clone, Copy)]
struct DecisionArm<'a> {
    war_from_plan: bool,
    forced_on: &'a [&'a str],
    withheld: &'a [String],
}

/// Per-run state the decider carries between host turns.
struct DecisionMemory<'a> {
    ours: &'a mut std::collections::BTreeMap<i64, String>,
    host_peace_retries: &'a mut HostPeaceRetries,
    host_move_refusals: &'a mut HostMoveRefusals,
    /// The actuation contract's memory: orders the host has provably refused.
    host_order_refusals: &'a mut HostOrderRefusals,
    air_assault_continuation: &'a mut air_assault_continuation::Continuation,
    governor_exchange: &'a mut governor_exchange::Continuation,
}

fn decide(
    mirror_state: &mut civvis::mirror::LiveMirror,
    ai: &mut civvis::ai::AdvancedAi,
    snapshot: &civvis::mirror::Snapshot,
    state: &civvis::mirror::StateSnapshot,
    arm: DecisionArm<'_>,
    memory: DecisionMemory<'_>,
) -> String {
    let DecisionMemory {
        ours,
        host_peace_retries,
        host_move_refusals,
        host_order_refusals,
        air_assault_continuation,
        governor_exchange,
    } = memory;
    // Only the live bridge has Firaxis's non-walking Trader representation and
    // host-city religious purchase rule. Enable those narrow adapters before
    // the AI simulates its turn; the tournament controller stays frozen.
    // Every live-bridge adapter and repair, in one place so the headless
    // measurement arms can play the SAME controller the bridge deploys.
    // See `AdvancedAi::enable_live_bridge`.
    if let Err(why) = configure_live_bridge(ai, arm.forced_on, arm.withheld) {
        // Names were validated before the first board is read. Keep a hard
        // error here too: a registry drift must not silently downgrade a live
        // treatment arm into the deployment arm mid-game.
        eprintln!("civvis-orders: {why}");
        std::process::exit(2);
    }
    // `unseen-capital-is-unseen`: the board's capital and founder facts are
    // the host's by now; the seat's city memory, taken earlier in the
    // rebuild, still crowns each rival's first planted city. Re-take it
    // before anything plans on it. See `mirror::refresh_city_memory`.
    if ai.unseen_capital_is_unseen() {
        civvis::mirror::refresh_city_memory(&mut mirror_state.game);
    }
    let emergency_wall_adoptions = adopt_emergency_wall_production(ours, state);
    let (production_hints_consumed, production_hints_expired) =
        settle_deferred_production_hints(ours, state);
    let observed_first_pad = ai
        .native_first_pad_allowed_cities(&mirror_state.game, 0)
        .map(|allowed| {
            let native_cities = mirror_state
                .cid_of
                .iter()
                .filter_map(|(native, model)| allowed.contains(model).then_some(*native))
                .collect::<std::collections::BTreeSet<_>>();
            (native_cities, ours.clone())
        });
    // `Ai::take_turn` is a full CIVVIS turn simulation: it changes queues, spends
    // resources, ends the turn, and can complete a queued unit.  None of those
    // mutations happened in Firaxis merely because we asked for a recommendation.
    // Keep the authoritative mirror as the last exported Civ VI state and plan on a
    // throwaway clone instead.  Apart from preventing phantom units, this means the
    // board shown to the next decision is never a mixture of one real game and one
    // speculative CIVVIS turn.
    let mut planned_game = mirror_state.game.clone();
    ai.observe_air_surge_city_owners(&mirror_state.game);
    // `phantom-capture-keeps-the-siege`: the host's cities as this frame opens.
    ai.observe_frame_cities(&mirror_state.game);
    // Firaxis keeps a Trader visible while it is travelling an active route;
    // CIVVIS's native model consumes it into `game.routes`.  The authoritative
    // mirror carries both so the map remains faithful.  Remove only the busy
    // trader from this throwaway planning board, otherwise a spare-capacity turn
    // can send the same real unit a second `TRADE_ROUTE` request.
    remove_active_route_traders_from_plan(&mut planned_game, mirror_state);
    let released = release_foreign_production(&mut planned_game, &mirror_state.cid_of, state, ours);
    let before = planned_game.log.len();
    // `own-column-is-not-a-refusal`: the movement memory before this frame is
    // planned, handed back once its orders are final. `None` with the gene off.
    let own_column_before = ai.host_frame_movement();
    // ★★★★ BEFORE THE VOLLEY. See `AdvancedAi::observe_turn_start_hostiles`:
    // the volley below removes wounded raiders from the planning board, and
    // a settler stepping afterwards must keep pricing them — the host has
    // been measured leaving such kills alive. And a settler's bound guard is
    // not the volley's to spend one tile away from the civilian it shields.
    ai.observe_confirmed_host_deaths(&planned_game, state);
    let refused_builder_operations = builder_operation_refusals::refused_operations(
        &planned_game,
        &mirror_state.civ6_of,
        state.turn,
        host_order_refusals,
    );
    let builder_operations_held: usize = refused_builder_operations
        .values()
        .map(|ops| ops.len())
        .sum();
    ai.set_refused_builder_operations(refused_builder_operations);
    air_assault::apply_cooldowns(
        &mut planned_game,
        &mirror_state.civ6_of,
        state.turn,
        host_order_refusals,
    );
    air_assault::observe(ai, snapshot, state);
    ai.set_refused_combinations(formation_refusals::refused_pairs(
        &mirror_state.civ6_of,
        state.turn,
        host_order_refusals,
    ));
    // The planner must not buy passage with placements the outbound cap
    // will discard. Reapplied for every disposable decision board.
    ai.set_envoy_order_budget(Some(ENVOY_ORDERS_PER_TURN));
    let (war_finishers, ai_actions_begin, air_assault_resumed) =
        air_assault_continuation.plan_frame(ai, &mut planned_game, state, &mirror_state.civ6_of);
    // Finishing attacks are translated explicitly below, including the reserve
    // order that was intentionally not applied to the planning board. Ordinary
    // AI actions start after the attacks that were applied there.
    // ⚠ MEASURE LEGALITY BEFORE THE TURN IS TAKEN. Asking afterwards reported
    // `all_legal = 0` — the enumeration short-circuits once the seat has acted — which
    // would have been read as "CIVVIS cannot declare war" when it only meant "I asked
    // at the wrong moment".
    let (pre_all_legal, pre_war_legal, pre_traders) = {
        let legal = mirror_state.game.legal_actions(0);
        let wars = legal
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
                )
            })
            .count();
        let traders = mirror_state
            .game
            .units
            .iter()
            .filter(|(_, unit)| unit.owner == 0 && unit.kind == "trader")
            .map(|(uid, unit)| {
                let routes = legal
                    .iter()
                    .filter(|action| matches!(action, Action::TradeRoute { unit: route, .. } if route == uid))
                    .count();
                let city = mirror_state
                    .game
                    .city_at(unit.pos)
                    .and_then(|cid| mirror_state.game.cities.get(&cid))
                    .map(|city| city.name.as_str())
                    .unwrap_or("none");
                let civ6 = mirror_state.civ6_of.get(uid).copied().unwrap_or_default();
                let active = mirror_state.active_trade_route_traders.contains(&civ6);
                let routes = if active { 0 } else { routes };
                format!(
                    "civ6={civ6} city={city} moves={:.1} active={active} routes={routes}",
                    unit.moves_left
                )
            })
            .collect::<Vec<_>>();
        (legal.len(), wars, traders)
    };
    // ⚠ MEASURE MOVEMENT BEFORE THE TURN IS TAKEN, for the same reason as legality
    // above. Counted afterwards, `movable` reports what is left AFTER CIVVIS has
    // moved everything -- so a perfectly healthy turn reads `movable=0/8`, which
    // looks exactly like an army that cannot move. It nearly cost a wrong conclusion
    // about units parked for 171 turns.
    let pre_movable = mirror_state
        .game
        .units
        .values()
        .filter(|u| u.owner == 0 && u.moves_left > 0.0)
        .count();
    let mut orders: Vec<Order> = war_finishers
        .actions
        .iter()
        .filter_map(|action| translate(action, mirror_state, state))
        .collect();
    // `battle-planner-3`: the pairs the kill plan wants the host to price,
    // ahead of the frame's strikes. A `preview` order is a question — the
    // mod answers it with a `preview` event at issue time and nothing moves
    // — and the answers reach `Game::host_previews` on the turn's next
    // frame, where the plan reads them in place of its closed form. Empty
    // with the version off.
    for (unit, target, ranged) in ai.wanted_previews() {
        if let Some(civ6) = mirror_state.civ6_of.get(&unit) {
            orders.push(Order {
                kind: "preview",
                subject: Some(*civ6),
                verb: Some(if ranged { "RANGE_ATTACK" } else { "ATTACK" }.to_string()),
                pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
            });
        }
    }
    let mut skipped: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut skipped_examples: Vec<String> = Vec::new();
    let mut note_bits: Vec<String> = Vec::new();
    if builder_operations_held > 0 {
        note_bits.push(format!("builder_operations_held={builder_operations_held}"));
    }
    // `food-focus-grows-the-small-city`: see `citizen_growth::choices`.
    let citizen_growth_orders = append_citizen_growth_orders(
        snapshot,
        state,
        ai.food_focus_grows_the_small_city(),
        &mut orders,
    );
    if citizen_growth_orders > 0 {
        note_bits.push(format!("citizen_growth_orders={citizen_growth_orders}"));
    }
    if air_assault_resumed {
        note_bits.push("air_assault_continuation=1".into());
    }
    // ⚠ Which rule is refusing the army its attacks. 45 of 87 declined attacks
    // on a replay of run `civvis-20260803T005930Z` were the forward model
    // rejecting the action outright rather than judging it bad, 27 of them a
    // Field Cannon, and `do_ranged` alone refuses for seven distinct reasons.
    // Naming them is the difference between "the army does not attack" and a
    // fixable rule.
    {
        let census = civvis::ai::AdvancedAi::illegal_attack_census();
        if !census.is_empty() {
            let named: Vec<String> = census
                .iter()
                .take(6)
                .map(|(why, count)| format!("{why}={count}"))
                .collect();
            note_bits.push(format!("illegal_attacks [{}]", named.join("; ")));
        }
    }
    let mut policy_changed = false;
    if !arm.withheld.is_empty() {
        // ⚠ A control arm that does not say so in its own run log is a control
        // arm nobody can trust afterwards. Wall-clock is not provenance.
        note_bits.push(format!("withheld=[{}]", arm.withheld.join(",")));
    }
    if !pre_traders.is_empty() {
        note_bits.push(format!(
            "traders capacity={} active={} [{}]",
            mirror_state.game.trade_capacity(0),
            mirror_state.game.active_routes(0),
            pre_traders.join("; ")
        ));
    }
    if war_finishers.roll_rejections > 0 {
        note_bits.push(format!(
            "war_unit_finishing_roll_rejections={}",
            war_finishers.roll_rejections
        ));
    }
    if war_finishers.survival_rejections > 0 {
        note_bits.push(format!(
            "war_unit_finishing_survival_rejections={}",
            war_finishers.survival_rejections
        ));
    }
    if war_finishers.targets > 0 {
        note_bits.push(format!(
            "war_unit_finishing_volleys={} attacks={} reserves={}",
            war_finishers.targets,
            war_finishers.actions.len(),
            war_finishers.reserves,
        ));
    }

    for (seat, action) in planned_game.log.since(ai_actions_begin) {
        if *seat != 0 {
            continue;
        }
        if matches!(
            action,
            Action::SlotPolicy { .. } | Action::UnslotPolicy { .. }
        ) {
            policy_changed = true;
            continue;
        }
        let order = translate(action, mirror_state, state);
        match order {
            Some(value) => {
                // Remember what WE asked each city for, so next turn's override can
                // tell our own plan from the ladder's. Keyed on the Civilization VI
                // city id, which `translate` has already resolved.
                if value.kind == "produce" {
                    if let (Some(civ6), Some(name)) = (value.subject, value.verb.clone()) {
                        ours.insert(civ6, name);
                    }
                }
                orders.push(value)
            }
            None => {
                if skipped_examples.len() < 12 {
                    skipped_examples.push(format!("{action:?}"));
                }
                // Which half failed: the action had no counterpart, or it named a unit
                // or city this bridge could not map back to Civilization VI. Those are
                // completely different repairs.
                let why = match action {
                    Action::MoveTo { unit, to } | Action::Move { unit, to } => {
                        if mirror_state
                            .game
                            .units
                            .get(unit)
                            .is_some_and(|unit| unit.pos == *to)
                        {
                            "self_tile_move"
                        } else if rebuilt_unit_missing(mirror_state, *unit) {
                            "unit_not_mapped"
                        } else {
                            "unit_action_untranslated"
                        }
                    }
                    Action::Attack { unit, .. }
                    | Action::Ranged { unit, .. }
                    | Action::Swap { unit, .. }
                    | Action::AirStrike { unit, .. }
                    | Action::AirPillage { unit, .. }
                    | Action::AirRebase { unit, .. }
                    | Action::AirPatrol { unit, .. }
                    | Action::FoundCity { unit }
                    | Action::Fortify { unit }
                    | Action::Improve { unit, .. }
                    | Action::UpgradeUnit { unit }
                    | Action::DisbandUnit { unit } => {
                        if rebuilt_unit_missing(mirror_state, *unit) {
                            "unit_not_mapped"
                        } else {
                            "unit_action_untranslated"
                        }
                    }
                    Action::Produce { .. } => "produce_not_mapped",
                    Action::KeepCity { .. }
                    | Action::RazeCity { .. }
                    | Action::LiberateCity { .. } => "city_not_mapped",
                    _ => "",
                };
                // ★★★★ NAME THE ANONYMOUS COUNT. `self_tile_move` is the largest
                // single waste class on the ladder — **25,387 dropped orders across
                // 43 live runs, on 3,803 turns with at least one** — and as a bare
                // total it cannot say whether that is one stuck Settler, the whole
                // army, or a Trader with nowhere to go. Those are completely
                // different repairs, and this project has three times found that
                // the standing hypothesis about an anonymous count was wrong:
                // `no_params` x221 was one district, `move_refused` x33 was one
                // trader, and 146 blocked upgrades were two resource gaps, not gold.
                //
                // The expansion half is already measured and it is worth naming:
                // live settlers cross 0.78 tiles/turn on 2 movement points, stand
                // still on 45% of their turn-transitions, and **0 of 1455** of those
                // standstills carry a `move_refused` while 91% fall on a turn whose
                // journal says the settler is marching. Whether the settler's own
                // step is computing its own tile is exactly what this key answers.
                let why = if why.is_empty() {
                    action_variant(action)
                } else if why == "self_tile_move" {
                    match action {
                        Action::MoveTo { unit, .. } | Action::Move { unit, .. } => {
                            self_tile_move_key(mirror_state, *unit)
                        }
                        _ => why.to_string(),
                    }
                } else {
                    why.to_string()
                };
                *skipped.entry(why).or_insert(0) += 1;
            }
        }
    }
    if policy_changed {
        let deferred = policy_deck_cards_not_currently_legal(&planned_game, &mirror_state.game, 0);
        if deferred.is_empty() {
            orders.push(policy_deck_order(&planned_game, 0));
        } else {
            note_bits.push(format!(
                "policy_deck_deferred_future_unlocks={}",
                deferred.join(",")
            ));
        }
    }

    let production_hints =
        append_next_production_hints(ai, &planned_game, mirror_state, state, &mut orders, ours);
    if production_hints > 0 {
        note_bits.push(format!("production_next_hints={production_hints}"));
    }
    if production_hints_consumed > 0 {
        note_bits.push(format!(
            "production_next_consumed={production_hints_consumed}"
        ));
    }
    if production_hints_expired > 0 {
        note_bits.push(format!(
            "production_next_expired={production_hints_expired}"
        ));
    }

    let plan = ai.plan_report();
    let plan_facts = plan
        .as_ref()
        .map(|report| (report.strategy, report.victory_target));
    // `military-alliance-shield`: the host player and type its ask went to.
    let shield_subject = ai
        .military_alliance_shield_ask()
        .and_then(|(partner, kind)| {
            Some((
                host_player_target(mirror_state, state, partner)?,
                civ6_alliance_type(kind)?,
            ))
        });
    let held_alliances = drop_off_lane_alliances(
        plan_facts,
        ai.seat_allies_armed(),
        shield_subject,
        &mut orders,
    );
    if held_alliances > 0 {
        note_bits.push(format!("alliance_hold:off_lane={held_alliances}"));
    }
    let held_favor_sales = hold_planner_favor_sales(plan_facts, state, &mut orders);
    if held_favor_sales > 0 {
        note_bits.push(format!(
            "favor_hold:planner_sales_dropped={held_favor_sales}"
        ));
    }
    let favor_value = mirror_state.game.favor_gold_value(0);
    match append_favor_sale_order(plan_facts, state, &mut orders, favor_value) {
        None => note_bits.push("favor_sale=1".to_string()),
        Some(why) => {
            // Only the holds worth a glance in the ledger: a bank sitting on
            // a plan that would sell it, with nobody to sell to.
            if why == "favor_hold:no_buyer" || why == "favor_hold:contender" {
                note_bits.push(why.to_string());
            }
        }
    }
    match append_favor_buy_order(
        ai.favor_bought_before_congress_enabled(),
        state,
        &mut orders,
    ) {
        None => note_bits.push("favor_buy=1".to_string()),
        Some(why) => {
            // The holds worth a glance: a session at hand and a bank short of
            // it, with the treasury or the sellers missing.
            if why == "favor_buy_hold:treasury" || why == "favor_buy_hold:no_seller" {
                note_bits.push(why.to_string());
            }
        }
    }
    // `find-the-capital`: passage to a rival whose needed original capital
    // we have never seen is worth more than the tourism book (28 Gold for a
    // seat with no tourism, under the lane's 30 Gold minimum ask). See
    // `AdvancedAi::find_the_capital_passage_gold`.
    // `siege-buys-the-passage`: the majors whose closed borders hold a
    // siege march this turn or last. See `AdvancedAi::siege_passage_blockers`.
    let siege_blockers = ai.siege_passage_blockers(&mirror_state.game, 0);
    let passage_value = |seat: usize| {
        let book = mirror_state.game.passage_gold_value(0);
        let book = if siege_blockers.contains(&seat) {
            book.max(SIEGE_PASSAGE_GOLD)
        } else {
            book
        };
        ai.find_the_capital_passage_gold(&mirror_state.game, 0, seat)
            .map_or(book, |floor| book.max(floor))
    };
    let mut exploration_targets = border_buy_exploration_targets(
        snapshot,
        state,
        &mirror_state.game,
        &mirror_state.game.sealed_border_owners,
    );
    add_siege_passage_targets(&mut exploration_targets, &siege_blockers);
    // `unseen-capital-is-unseen`: the lane asks one rival a cadence, the one
    // with the most explorers at its border, and a rival the book prices
    // under the 30 Gold minimum ask can only ever hold. Keep the rivals it
    // would ask, so a rival whose capital we have never seen is not starved
    // by one whose capital we know: G120134Z t121 held on Babylon's 28 Gold
    // book with 314 Gold in the bank while France, the culture winner, was
    // never seen.
    if ai.unseen_capital_is_unseen() {
        exploration_targets.retain(|seat, _| passage_value(*seat) >= BORDER_BUY_CEILING_MIN as f64);
    }
    match append_border_buy_order(
        &mirror_state.game.sealed_border_owners,
        &exploration_targets,
        state,
        &mut orders,
        &passage_value,
    ) {
        None => note_bits.push("border_buy=1".to_string()),
        Some(why) => {
            // Only the holds that mean something is wrong on the ground: a
            // seal standing while the treasury cannot meet the minimum ask,
            // or while another deal holds the same rival's working deal.
            if why == "border_buy_hold:treasury" || why == "border_buy_hold:deal_in_flight" {
                note_bits.push(why.to_string());
            }
        }
    }
    // `captives-come-home`: our captured Spies bought back while the network
    // is short of its slots. See `append_captive_ransom_order`.
    if ai.captives_come_home_enabled() {
        let held = state
            .units
            .iter()
            .filter(|unit| unit.kind == "UNIT_SPY")
            .count();
        let capacity = state.spy_capacity.unwrap_or(0).max(0) as usize;
        if ai.captives_come_home_wanted(&mirror_state.game, 0, held, capacity) {
            // `captives-come-home-2`: Favor beside the Gold. A bank held for
            // Congress keeps a vote reserve; a rival near a Diplomatic Victory,
            // or a seat racing Diplomacy itself, keeps all of it.
            let top_dvp = state
                .rivals
                .iter()
                .map(|rival| rival.dvp.unwrap_or(0))
                .max()
                .unwrap_or(0);
            let reserve = if favor_banked_for_votes(state) {
                RANSOM_FAVOR_VOTE_RESERVE
            } else {
                RANSOM_FAVOR_RESERVE
            };
            let favor = (ai.captives_come_home_pays_favor()
                && top_dvp < RANSOM_FAVOR_DVP_CEILING
                && !plan_races_diplomacy(plan_facts))
            .then(|| state.favor.unwrap_or(0.0).floor() as i64 - reserve)
            .filter(|points| *points >= RANSOM_FAVOR_MIN)
            .map(|points| points.min(RANSOM_FAVOR_CAP) as u32);
            match append_captive_ransom_order(state, &mut orders, favor) {
                None => note_bits.push("captive_ransom=1".to_string()),
                Some(why) if why != "ransom_hold:no_captor" => note_bits.push(why.to_string()),
                Some(_) => {}
            }
        }
    }
    // `tribute-buys-the-friendship`: Gold to the strongest neighbour at peace
    // while it is too cold for the friendship ask. See `append_tribute_order`.
    if ai.tribute_buys_the_friendship_enabled() {
        match append_tribute_order(state, &mut orders) {
            None => note_bits.push("tribute=1".to_string()),
            Some(why @ ("tribute_hold:treasury" | "tribute_hold:deal_in_flight")) => {
                note_bits.push(why.to_string())
            }
            Some(_) => {}
        }
    }
    // `siege-buys-the-gun-resource`: the Niter or Oil the campaign's better
    // siege gun waits on, bought from a met major's trade screen. See
    // `append_gun_resource_buy_order`.
    if ai.siege_buys_the_gun_resource_enabled() {
        let wants = ai.siege_gun_resource_wants(&mirror_state.game, 0);
        let held = ai.siege_gun_resources_held(&mirror_state.game, 0, &wants);
        let kept = drop_held_resource_sales(&held, &mut orders);
        if kept > 0 {
            note_bits.push(format!("gun_resource_sale_held={kept}"));
        }
        let spares = ai.siege_barter_spares(&mirror_state.game, 0, &held);
        let worth = |resource: &str, amount: u32| {
            mirror_state.game.strategic_gold_value(0, resource, amount)
        };
        // A barter item at what it is worth to the seller's mirrored seat;
        // nothing when the seat cannot be named.
        let give_worth = |seller: i64, resource: &str, amount: u32| {
            (1..mirror_state.game.players.len())
                .find(|seat| host_player_target(mirror_state, state, *seat) == Some(seller))
                .map_or(0.0, |seat| {
                    mirror_state
                        .game
                        .resource_gold_value(seat, resource, amount)
                })
        };
        match append_gun_resource_buy_order(
            &wants,
            &spares,
            state,
            &mut orders,
            &worth,
            &give_worth,
        ) {
            None => {
                note_bits.push("gun_resource_buy=1".to_string());
                // `aluminum-ask-tops-up-gold`: let the host's counter price
                // the Aluminum block in Gold, up to the ceiling journalled
                // here; the deal events carry the ceiling and the outcome.
                if ai.aluminum_ask_tops_up_gold_enabled() {
                    let floor = (ai
                        .purchase_upkeep_floor_reading(&mirror_state.game, 0)
                        .ceil() as i64)
                        .max(GUN_RESOURCE_BUY_GOLD_RESERVE);
                    if let Some(order) = orders.last_mut() {
                        if let Some(ceiling) = aluminum_ask_ceiling(order, state.gold, floor) {
                            order.pos = Some((ceiling, 0));
                            note_bits.push(format!("aluminum_ask_ceiling={ceiling}"));
                        }
                    }
                }
            }
            Some(why) => {
                // The holds worth a glance: a gun waiting on its resource
                // while the treasury, the sellers or their working deals
                // stand in the way.
                if why != "gun_resource_buy_hold:no_need" && why != "gun_resource_buy_hold:cadence"
                {
                    note_bits.push(why.to_string());
                }
            }
        }
    }
    // A power bridge uses the existing strategic market transport. It never
    // competes with another purchase already proposed in this frame.
    if !orders.iter().any(|order| order.kind == "buy") {
        let wants = ai.science_power_fuel_wants(&mirror_state.game, 0);
        if !wants.is_empty() {
            let reserve = (ai
                .purchase_upkeep_floor_reading(&mirror_state.game, 0)
                .ceil() as i64)
                .max(100);
            let worth = |resource: &str, amount: u32| {
                mirror_state.game.strategic_gold_value(0, resource, amount)
            };
            let give_worth = |_: i64, _: &str, _: u32| 0.0;
            let result = append_resource_buy_order(
                &wants,
                &[],
                state,
                &mut orders,
                &worth,
                &give_worth,
                ResourceBuyBudget {
                    reserve,
                    gold_limit: Some(mirror_state.game.players[0].gold.floor() as i64),
                    credit_income: true,
                    cadence: true,
                },
            );
            match result {
                None => note_bits.push("science_power_fuel_buy=1".to_string()),
                Some(why) => note_bits.push(format!("science_power_fuel:{why}")),
            }
        }
    }
    // `luxury-buy-asks`: no rival has ever priced a luxury for us -- 644
    // EQUALIZE asks of October 4-5, every answer a refusal with nothing on our
    // side (364 of them to rivals we had not denounced) -- while each ask held
    // the rival's one working deal and the trade cooldown the sales share.
    // `luxury-swap-asks`: a spare copy for a luxury the seat lacks, ahead of
    // the Gold-only ask on the same slot.
    // `luxury-swap-asks-2` fits the spare to the partner by the board: the
    // rival's seat holds no copy of it.
    if ai.luxury_swap_asks_enabled() || ai.luxury_swap_asks_2_enabled() {
        let lacks = |seller: i64, resource: &str| {
            let id = resource.strip_prefix("RESOURCE_")?.to_ascii_lowercase();
            mirror_state.game.rules.resources.get(id.as_str())?;
            (1..mirror_state.game.players.len())
                .find(|seat| host_player_target(mirror_state, state, *seat) == Some(seller))
                .map(|seat| mirror_state.game.resource_access_count(seat, &id) == 0)
        };
        let fit = ai
            .luxury_swap_asks_2_enabled()
            .then_some(&lacks as &dyn Fn(i64, &str) -> Option<bool>);
        let swapped = if ai.luxury_asks_pay_in_strategics_2_enabled() {
            append_luxury_swap_order_2(
                state,
                &mut orders,
                fit,
                &ai.luxury_swap_stale_luxuries(&mirror_state.game),
            )
        } else {
            append_luxury_swap_order(state, &mut orders, fit)
        };
        match swapped {
            None => note_bits.push("luxury_swap=1".to_string()),
            Some(why) => {
                if why == "luxury_swap_hold:no_spare"
                    || why == "luxury_swap_hold:no_partner"
                    || why == "luxury_swap_hold:no_fit"
                {
                    note_bits.push(why.to_string());
                }
                // `luxury-asks-pay-in-strategics`: no spare copy fits, so
                // the slot pays in strategics the army does not need. See
                // `append_luxury_strategic_ask`.
                if ai.luxury_asks_pay_in_strategics_enabled()
                    && (why == "luxury_swap_hold:no_spare" || why == "luxury_swap_hold:no_fit")
                {
                    let game = &mirror_state.game;
                    let mut held =
                        ai.siege_gun_resources_held(game, 0, &ai.siege_gun_resource_wants(game, 0));
                    if ai.army_upgrade_materials_enabled() {
                        held.extend(
                            ai.army_upgrade_material_wants(game, 0)
                                .into_iter()
                                .map(|want| want.resource),
                        );
                    }
                    let spares = ai.luxury_ask_strategic_spares(game, 0, &held);
                    let give_worth = |seller: i64, resource: &str, amount: u32| {
                        (1..game.players.len())
                            .find(|seat| {
                                host_player_target(mirror_state, state, *seat) == Some(seller)
                            })
                            .map_or(0.0, |seat| game.resource_gold_value(seat, resource, amount))
                    };
                    let asked = if ai.luxury_asks_pay_in_strategics_2_enabled() {
                        append_luxury_strategic_ask_2(
                            state,
                            &mut orders,
                            &spares,
                            &give_worth,
                            &ai.luxury_barter_shorted_partners(game),
                        )
                    } else {
                        append_luxury_strategic_ask(state, &mut orders, &spares, &give_worth)
                    };
                    match asked {
                        None => note_bits.push("luxury_strategic_ask=1".to_string()),
                        Some(why) => note_bits.push(why.to_string()),
                    }
                }
            }
        }
    }
    let luxury_buy = if ai.luxury_buy_asks_enabled() {
        append_luxury_buy_order(state, &mut orders)
    } else {
        Some("luxury_buy_hold:gene")
    };
    match luxury_buy {
        None => note_bits.push("luxury_buy=1".to_string()),
        Some(why) => {
            // The holds worth a glance: an amenity deficit standing on the
            // lane's own turn while the income or the treasury cannot carry
            // a copy, or with nobody at peace to buy one from.
            if why == "luxury_buy_hold:income"
                || why == "luxury_buy_hold:treasury"
                || why == "luxury_buy_hold:no_stock"
                || why == "luxury_buy_hold:no_seller"
            {
                note_bits.push(why.to_string());
            }
        }
    }
    match append_work_sale_order(state, &mut orders) {
        None => note_bits.push("work_sale=1".to_string()),
        Some(why) => {
            // The holds worth a glance: a starved person whose kind has no
            // placed work to free (only building capacity can seat them), or
            // nobody at peace to sell to.
            if why == "work_sale_hold:no_matching_work" || why == "work_sale_hold:no_buyer" {
                note_bits.push(why.to_string());
            }
        }
    }
    let mut native_defense_material_buy = false;
    if ai.army_upgrade_materials_enabled() {
        let gold = planned_game.players[0].gold.floor() as i64;
        let (wants, native_defense) =
            ai.army_upgrade_material_wants_after_spending(&mirror_state.game, 0, gold as f64);
        let (held, kept) =
            protect_army_upgrade_material_sales(ai, &mirror_state.game, &wants, &mut orders);
        if kept > 0 {
            note_bits.push(format!("army_material_sale_held={kept}"));
        }
        // A model spare is not proof that the host can put that resource on
        // our side of this rival's deal. Native 084123-cont1 t169 refused
        // the whole Oil request as barter_short. Use the already bounded
        // cash budget for native defensive materials; keep the upgrade bill.
        let spares = if native_defense {
            Vec::new()
        } else {
            ai.siege_barter_spares(&mirror_state.game, 0, &held)
        };
        let worth = |resource: &str, amount: u32| {
            if native_defense {
                // Ask the host for its price within the carried cash ceiling.
                // The model valued native t169's 30 Oil at 891 Gold; no
                // native quote existed. Zero selects the transport's existing
                // unpriced-book path, retaining all native reply checks.
                0.0
            } else {
                mirror_state.game.strategic_gold_value(0, resource, amount)
            }
        };
        let give_worth = |seller: i64, resource: &str, amount: u32| {
            (1..mirror_state.game.players.len())
                .find(|seat| host_player_target(mirror_state, state, *seat) == Some(seller))
                .map_or(0.0, |seat| {
                    mirror_state
                        .game
                        .resource_gold_value(seat, resource, amount)
                })
        };
        match append_army_upgrade_material_buy_order(
            &wants,
            &spares,
            state,
            &mut orders,
            &worth,
            &give_worth,
            gold,
        ) {
            None => {
                native_defense_material_buy = native_defense;
                if let Some(want) = wants.first() {
                    note_bits.push(format!(
                        "army_material_buy=1 upgrade={} unit={} cash_kept={:.0}",
                        want.unit, want.uid, want.gold_reserve
                    ));
                }
            }
            Some("no_need") => {}
            Some(why) => note_bits.push(format!("army_material_buy_hold:{why}")),
        }
    }
    let sales = orders.iter().filter(|order| order.kind == "sell").count();
    if sales > 0 {
        note_bits.push(format!("sales={sales}"));
    }

    let (person_orders, great_person_stall) =
        great_person_orders_led(state, ai.generals_lead_the_army_enabled());
    if great_person_stall.generals_leading > 0 {
        note_bits.push(format!(
            "generals_leading={}",
            great_person_stall.generals_leading
        ));
    }
    if !person_orders.is_empty() {
        note_bits.push(format!("great_people_orders={}", person_orders.len()));
        // Retirements go FIRST: the ghost's hex is what other units in this
        // very batch want to walk through, and the host executes in order.
        let (retirements, others): (Vec<Order>, Vec<Order>) = person_orders
            .into_iter()
            .partition(|order| order.verb.as_deref() == Some("DELETE"));
        orders.splice(0..0, retirements);
        orders.extend(others);
    }
    if great_person_stall.retired_prophets > 0 {
        note_bits.push(format!(
            "retired_founded_prophets={}",
            great_person_stall.retired_prophets
        ));
    }
    if great_person_stall.total() > 0 {
        // Kept under the old key so existing log readers still find it, with the
        // split alongside: one of these two numbers is a cooldown frame and the
        // other is a Great Person the empire cannot use at all.
        note_bits.push(format!(
            "great_people_without_activation_target={} (cooldown={} no_plot={} no_empty_slot={})",
            great_person_stall.total(),
            great_person_stall.on_cooldown,
            great_person_stall.no_activation_plot,
            great_person_stall.no_empty_slot,
        ));
    }

    // An override nothing reports does not exist. `released` is how many cities were
    // handed back to CIVVIS this turn; if it stays high all game, CIVVIS's choices are
    // not taking and the refusal ledger is the place to look.
    if released > 0 {
        note_bits.push(format!("released={released}"));
    }
    if emergency_wall_adoptions > 0 {
        note_bits.push(format!(
            "emergency_wall_production_adopted={emergency_wall_adoptions}"
        ));
    }

    if let Some(report) = &plan {
        note_bits.push(format!(
            "plan strategy={} victory={:?} target_player={:?} desired_cities={}",
            report.strategy, report.victory_target, report.target_player, report.desired_cities
        ));
        // ⚠⚠ RETRACTED AS A DEFAULT, AND THE REASON IS A LOST GAME.
        //
        // This used to declare war whenever CIVVIS's PLAN named a target, on the
        // reasoning that a plan rebuilt every turn never gets far enough to log a
        // `DeclareWar` of its own. But `plan_report().target_player` is who CIVVIS
        // would PREFER to fight, not "declare now" — CIVVIS's own gating had declined,
        // and overriding a decline is me making the decision, which is exactly what
        // this architecture exists to stop.
        //
        // Measured cost on run civvis-20260730T120107Z: three forced declarations
        // (t48, t144, t217) with an army of 2-6 units, the empire ground from 3 cities
        // to 2 to none, and the run ended on the DEFEAT screen at ~t220 with score 161
        // against 892. Being conquered on SETTLER is the strongest possible evidence
        // that the wars were not CIVVIS's idea.
        //
        // Kept behind `--war-from-plan` because it is the right diagnostic when plan
        // continuity is broken — but with the persistent mirror, CIVVIS should reach
        // its own declaration, and if it does not that is information, not a gap to fill.
        let already = orders.iter().any(|o| o.kind == "war");
        if arm.war_from_plan && !already {
            if let Some(seat) = report.target_player {
                if let Some(rival) = civvis::mirror::host_rival_for_seat(
                    state,
                    seat,
                    civvis::mirror::modeled_major_player_count(&mirror_state.game),
                ) {
                    if !rival.at_war {
                        orders.push(Order {
                            kind: "war",
                            subject: Some(rival.player as i64),
                            verb: Some("DECLARE".to_string()),
                            pos: None,
                        });
                        note_bits.push(format!("war_from_plan={}", rival.player));
                    }
                }
            }
        }
        // ★ PEACE RIDES THE REPORT, NOT THE LOG — and unlike the retracted
        // war-from-plan above this is NOT the bridge upgrading a preference:
        // the planner DECIDED to offer (it journals "Offering peace" and
        // applies `ProposeDeal { peace: true }`). What declines is CIVVIS's
        // internal MODEL of the rival — winning, so the deal never reaches the
        // applied-action log — but on a mirrored game that answer belongs to
        // the real Civilization VI rival. Run civvis-20260801T221459Z: 106
        // offer decisions from t118, zero orders, the losing war unexitable.
        for seat in &report.peace_offers {
            if let Some(rival) = civvis::mirror::host_rival_for_seat(
                state,
                *seat,
                civvis::mirror::modeled_major_player_count(&mirror_state.game),
            ) {
                let subject = rival.player as i64;
                let already = orders
                    .iter()
                    .any(|o| o.kind == "peace" && o.subject == Some(subject));
                if rival.at_war && !already {
                    let routed = report.peace_routed.contains(seat);
                    let cap = peace_tribute_cap(state, routed);
                    // `peace-asks-a-city`: `y` 1 asks the agent to add the
                    // rival's nearest cedable town to the first ask.
                    let ask_city = !routed && report.peace_asks_city.contains(seat);
                    orders.push(Order {
                        kind: "peace",
                        subject: Some(subject),
                        verb: Some("MAKE_PEACE".to_string()),
                        pos: Some((cap, i32::from(ask_city))),
                    });
                    note_bits.push(format!("peace_from_plan={}", rival.player));
                    if ask_city {
                        note_bits.push(format!("peace_city_ask={}", rival.player));
                    }
                    if cap > 0 {
                        note_bits.push(format!("peace_tribute_cap={cap}"));
                    }
                }
            }
        }
    } else {
        note_bits.push("plan=none".to_string());
    }

    match append_aid_gift_order(state, &mut orders) {
        None => note_bits.push("aid_gift=1".to_string()),
        // This is the one active hold worth keeping beside the decision: an
        // emergency was finishable, but another order in this exact batch
        // changes the target's diplomatic state first.
        Some("aid_gift_hold:diplomacy_conflict") => {
            note_bits.push("aid_gift_hold:diplomacy_conflict".to_string());
        }
        Some(_) => {}
    }

    let envoy_reclaim = queue_city_state_envoy_reclaim(&mut orders, state);
    // A direct city-state peace remains the preferred reclaim: it cannot cost
    // a major-war concession.  If none is available, turn a peace the planner
    // already chose against the hostile Suzerain into an Envoy reservation.
    let suzerain_envoy_reclaim = envoy_reclaim
        .is_none()
        .then(|| planned_suzerain_peace_envoy_reclaim(&orders, state))
        .flatten();

    let (activation_safe, deferred_activation_plot_conflicts) =
        defer_great_person_plot_conflicts(orders, state);
    orders = activation_safe;
    if deferred_activation_plot_conflicts > 0 {
        note_bits.push(format!(
            "deferred_activation_plot_conflicts={deferred_activation_plot_conflicts}"
        ));
    }

    let air_phase_deferred = air_assault::defer_followups(
        &mut orders,
        ai.planned_air_city_assault(),
        snapshot,
        &mirror_state.civ6_of,
    );
    if air_phase_deferred > 0 {
        note_bits.push(format!("air_assault_phase_deferred={air_phase_deferred}"));
    }

    for (source, work, destination, slot) in
        ai.native_writing_evacuations(&mirror_state.game, 0, state)
    {
        if let Ok(destination) = i32::try_from(destination) {
            orders.push(Order {
                kind: "writing_evacuation",
                subject: Some(source),
                verb: Some(work),
                pos: Some((destination, slot)),
            });
        }
    }

    for (source, work, destination, slot) in
        ai.native_art_museum_swaps(&mirror_state.game, 0, state)
    {
        if let Ok(destination) = i32::try_from(destination) {
            orders.push(Order {
                kind: "museum_swap",
                subject: Some(source),
                verb: Some(work),
                pos: Some((destination, slot)),
            });
        }
    }

    let relocated_traders = trader_origin_coverage::apply(
        ai,
        &mirror_state.game,
        &mirror_state.uid_of,
        &mirror_state.active_trade_route_traders,
        &mut orders,
    );
    if relocated_traders > 0 {
        note_bits.push(format!(
            "culture_trade_origin_transfers={relocated_traders}"
        ));
    }

    // Keep the first speculative local step before a whole walk is compressed
    // into one host operation. `HostMoveRefusals` uses it only if Firaxis then
    // leaves the unit exactly where it started.
    let first_unknown_steps = first_unknown_coalesced_steps(&orders, snapshot);
    // The site rides on the found, so the mod founds only where the walk was
    // meant to end. See `stamp_found_sites`.
    let found_sites = stamp_found_sites(&mut orders, state);
    if found_sites > 0 {
        note_bits.push(format!("found_sites={found_sites}"));
    }
    let sequenced = state.seat.order_queue;
    let mut local_routes = wounded_local_routes(state);
    local_routes.extend(
        host_move_refusals
            .local_retry
            .iter()
            .filter_map(|(uid, (turn, from))| {
                (*turn == state.turn
                    && state
                        .units
                        .iter()
                        .any(|unit| unit.id == *uid && (unit.x, unit.y) == *from))
                .then_some(*uid)
            }),
    );
    if !local_routes.is_empty() {
        note_bits.push(format!("wounded_local_routes={}", local_routes.len()));
    }
    let walk_steps = if ai.own_column_is_not_a_refusal_enabled() {
        open_walk_steps(&orders)
    } else {
        std::collections::BTreeMap::new()
    };
    let (causally_safe, deferred_unit_followups, coalesced_path_steps) =
        coalesce_unit_paths_except(orders, sequenced, &local_routes, &capture_tiles(state));
    orders = causally_safe;
    if coalesced_path_steps > 0 {
        note_bits.push(format!("coalesced_path_steps={coalesced_path_steps}"));
    }
    if !walk_steps.is_empty() {
        let retargeted =
            retarget_friendly_held_walks_on_board(&mut orders, &walk_steps, mirror_state);
        if retargeted > 0 {
            note_bits.push(format!("own_column_retargeted={retargeted}"));
        }
    }
    if deferred_unit_followups > 0 {
        note_bits.push(format!("deferred_unit_followups={deferred_unit_followups}"));
    }
    if ai.ranged_before_melee_enabled() {
        let moved = ranged_before_melee(&mut orders);
        if moved > 0 {
            note_bits.push(format!("ranged_before_melee={moved}"));
        }
    }
    if sequenced {
        // How many orders now ride the mod's per-unit queue instead of waiting
        // a turn — the number the queue exists to move off zero.
        let followups = sequenced_unit_followups(&orders);
        if followups > 0 {
            note_bits.push(format!("sequenced_unit_followups={followups}"));
        }
    }
    if state.seat.moves_at_turn_start {
        // Units the host had already walked before this frame — planned with
        // what they have left, not with a fresh turn. Zero is the healthy
        // reading once every MOVE_TO is capped to the turn's reach.
        let short = mirror_state.units_short_of_movement();
        if short > 0 {
            note_bits.push(format!("moves_short={short}"));
        }
    }

    let host_frontier_probes =
        host_move_refusals.cap_pending_frontier_moves(&mut orders, state, &first_unknown_steps);
    if host_frontier_probes > 0 {
        note_bits.push(format!("host_frontier_probes={host_frontier_probes}"));
    }

    // A same-turn replan can still be based on the previous frame's unit tile.
    // Do this after frontier capping so a newly requested exact probe remains a
    // real change, while stale unit/deal replays and conflicting city production
    // requests are not sent twice to the host. Strike retries stay enabled; see
    // the method comment.
    let ski_jobs =
        culture_ski_resorts::append(ai, mirror_state, &host_move_refusals.dead, &mut orders);
    if ski_jobs > 0 {
        note_bits.push(format!("culture_ski_jobs={ski_jobs}"));
    }
    let safer_campuses = campus_above_sea::retarget(ai, mirror_state, &mut orders);
    if safer_campuses > 0 {
        note_bits.push(format!("campus_above_the_sea={safer_campuses}"));
    }
    let safer_pads = spaceport_above_sea::retarget(ai, mirror_state, &mut orders);
    if safer_pads > 0 {
        note_bits.push(format!("spaceport_above_the_sea={safer_pads}"));
    }
    if let Some((allowed, previous_ours)) = &observed_first_pad {
        let held = keep_observed_first_pad_floor(&mut orders, allowed, previous_ours, ours);
        if held > 0 {
            note_bits.push(format!("observed_first_pad_held={held}"));
        }
    }
    let same_turn_replays = host_move_refusals.suppress_same_turn_replays(&mut orders, state, ours);
    if same_turn_replays > 0 {
        note_bits.push(format!("same_turn_replays={same_turn_replays}"));
    }
    let chained_moves_resent = host_move_refusals.resend_cleared_chained_moves(&mut orders, state);
    if chained_moves_resent > 0 {
        note_bits.push(format!("chained_moves_resent={chained_moves_resent}"));
    }

    // Retained assignments must pass the same refusal filter and outbound
    // ledger as a newly planned order. The Governor operation budget follows.
    if let Some(note) = governor_exchange.prepare(state, &mut orders) {
        note_bits.push(format!("governor_exchange={note}"));
    }

    // ⭐ THE ACTUATION CONTRACT. An order that has come back Failed with the
    // same reason `ORDER_REFUSAL_STRIKES` times running is not sent again for
    // `ORDER_REFUSAL_COOLDOWN_TURNS`. Placed after every other suppressor, so
    // the strike list is applied to the orders that would really have gone,
    // and before `record`, which is the ledger of what was actually sent.
    let (mut orders, refused_before) = withhold_refused_orders(orders, state, host_order_refusals);
    if !refused_before.is_empty() {
        note_bits.push(format!(
            "withheld_refused={} [{}]",
            refused_before.len(),
            refused_before.join("; ")
        ));
    }

    // Remember where each move sends which host unit, so next turn's positions
    // can prove a destination unwalkable. See `HostMoveRefusals`.
    air_assault_continuation.record(state, ai.planned_air_city_assault(), &orders);
    host_move_refusals.record(&orders, state, &first_unknown_steps);
    if !host_move_refusals.dead.is_empty() {
        note_bits.push(format!("host_dead_plots={}", host_move_refusals.dead.len()));
    }
    let (governor_safe, deferred_governor_followups) = defer_governor_followups(orders);
    orders = governor_safe;
    if deferred_governor_followups > 0 {
        note_bits.push(format!(
            "deferred_governor_followups={deferred_governor_followups}"
        ));
    }
    let (envoy_bounded, deferred_envoys) = bound_envoy_orders(orders);
    orders = envoy_bounded;
    {
        // The instrument for the lane: how many crossed this turn and how many
        // wait, against how many the host says we hold. `envoys unspent N` in
        // the economy note is the same fact from Firaxis's side.
        let sent = orders.iter().filter(|order| order.kind == "envoy").count();
        if sent > 0 || deferred_envoys > 0 {
            note_bits.push(format!(
                "envoy_orders={sent} deferred={deferred_envoys} held={}",
                mirror_state.game.players[0].envoys_free
            ));
        }
    }

    // Record a host peace retry only after the air-assault observation barrier
    // and the other outbound filters have settled. A planned offer deferred
    // before a sortie was never submitted, so it cannot start Firaxis's
    // five-turn retry clock or reserve Envoys for a peace that did not happen.
    let (host_legal, deferred_peace_retries) =
        defer_host_peace_retries(orders, state, host_peace_retries);
    orders = host_legal;
    // The AI chooses whether/where to buy; the host quotes a tradeable work.
    // Keep higher-priority resource, passage and amenity purchases first.
    if !orders.iter().any(|order| order.kind == "buy") {
        let cultural_activation_pending = state.units.iter().any(|unit| {
            unit.great_person.as_ref().is_some_and(|person| {
                (person.can_activate
                    || orders.iter().any(|order| {
                        order.kind == "unit"
                            && order.subject == Some(unit.id)
                            && order.verb.as_deref() == Some("ACTIVATE_GREAT_PERSON")
                    }))
                    && matches!(
                        person.class.as_deref(),
                        Some(
                            "GREAT_PERSON_CLASS_WRITER"
                                | "GREAT_PERSON_CLASS_ARTIST"
                                | "GREAT_PERSON_CLASS_MUSICIAN"
                        )
                    )
            })
        });
        if let Some((seller, ceiling, max_gold, max_gpt)) = ai.culture_work_purchase_ask(
            &mirror_state.game,
            0,
            planned_game.players[0].gold,
            state.gold_per_turn,
            cultural_activation_pending,
        ) {
            if let Some(rival) = civvis::mirror::host_rival_for_seat(state, seller, 0) {
                orders.push(Order {
                    kind: "buy",
                    subject: Some(rival.player as i64),
                    verb: Some(format!("GREATWORK_ANY;GPT_CAP={max_gpt}")),
                    pos: Some((ceiling, max_gold)),
                });
                note_bits.push("culture_work_buy=1".to_string());
            }
        }
    }
    let session_conflicts = drop_deals_beside_war_or_peace(&mut orders);
    if session_conflicts > 0 {
        note_bits.push(format!(
            "deals_dropped_for_war_or_peace={session_conflicts}"
        ));
    }
    let deferred_alliances =
        defer_alliances_for_defensive_material(&mut orders, native_defense_material_buy);
    if deferred_alliances > 0 {
        note_bits.push(format!(
            "alliances_deferred_for_upgrade_material={deferred_alliances}"
        ));
    }
    let held_beside_alliance = hold_deals_beside_an_alliance(&mut orders);
    if held_beside_alliance > 0 {
        note_bits.push(format!("deals_held_beside_alliance={held_beside_alliance}"));
    }
    if let Some((target, needed)) = envoy_reclaim {
        let submitted = orders
            .iter()
            .any(|order| order.kind == "peace" && order.subject == Some(target));
        if submitted {
            let deferred = reserve_envoys_for_submitted_reclaim(
                &mut orders,
                target,
                state.envoys_free.unwrap_or_default(),
                needed,
            );
            note_bits.push(format!(
                "envoy_reclaim_peace={target} needed={needed} deferred_envoys={deferred}"
            ));
        }
    } else if let Some((suzerain, minor, needed)) = suzerain_envoy_reclaim {
        let submitted = orders
            .iter()
            .any(|order| order.kind == "peace" && order.subject == Some(suzerain));
        if submitted {
            let deferred = reserve_envoys_for_submitted_reclaim(
                &mut orders,
                minor,
                state.envoys_free.unwrap_or_default(),
                needed,
            );
            note_bits.push(format!(
                "envoy_suzerain_reclaim_peace={suzerain} minor={minor} needed={needed} \
                 deferred_envoys={deferred}"
            ));
        }
    }
    if !deferred_peace_retries.is_empty() {
        let targets = deferred_peace_retries
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        note_bits.push(format!("peace_host_cooldown=[{targets}]"));
    }

    if !mirror_state.unmapped.is_empty() {
        note_bits.push(format!("unmapped: {}", mirror_state.unmapped.join(",")));
    }
    // ★★★★ HOW FAR OFF THE RECONSTRUCTED ECONOMY IS. The board is openly partial and
    // nothing has ever said by how much; research valuations are spent in these units,
    // so a rate half again too fast makes an unaffordable plan look affordable.
    // Reported, never injected — see `mirror::economy_drift`.
    if let Some(drift) = civvis::mirror::economy_drift(&mirror_state.game, state) {
        note_bits.push(drift);
    }
    // ★★★★★ UNITS THE EXPORT NAMED THAT NEVER REACHED THE BOARD. A unit CIVVIS cannot
    // see gets no order and stands where it was built for the rest of the game — the
    // "units stacking up in the capital" the operator reported, arriving by a route
    // nobody had looked at. `unmapped` cannot show these: they are not translation
    // failures, they are units the reconstruction refused for a REASON, and the reason
    // is what says whether it is fog, water, an untranslatable type, or a tile CIVVIS
    // will not stack the way Civilization VI does.
    if !mirror_state.dropped_units.is_empty() {
        note_bits.push(format!(
            "dropped_units={} [{}]",
            mirror_state.dropped_units.len(),
            mirror_state.dropped_units.join(" ")
        ));
    }
    if !skipped.is_empty() {
        let text = skipped
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        note_bits.push(format!("skipped {text}"));
        note_bits.push(format!(
            "skipped_examples [{}]",
            skipped_examples.join(" | ")
        ));
    }
    // ⚠ Diagnostics for the "CIVVIS returned nothing" case, which is otherwise
    // indistinguishable from "CIVVIS chose to do nothing". Both a stopped game
    // (`winner` set) and an army with no movement produce an empty order list.
    // What is left after the turn. Reported beside the pre-turn count, because the
    // pair is what distinguishes "could not move" from "already moved".
    let movable = mirror_state
        .game
        .units
        .values()
        .filter(|u| u.owner == 0 && u.moves_left > 0.0)
        .count();
    let ours = mirror_state
        .game
        .units
        .values()
        .filter(|u| u.owner == 0)
        .count();
    // Does the engine still SEE our roster? `player_unit_ids` answers from a memo,
    // and on a persistent game a stale memo would hand CIVVIS an army that no longer
    // matches the one in `units` — a mismatch that produces no error anywhere.
    // ⚠ Is war even LEGAL in CIVVIS's model? Its journal says "campaign aimed at Egypt,
    // not yet at war" while our power is 156 against 20 — so either it is choosing not
    // to, or the action is not on the table. Those are opposite problems.
    {
        let has_met = mirror_state.game.has_met(0, 1);
        // ⚠ Which units does CIVVIS think it has that this bridge cannot name? A unit
        // with no Civ 6 counterpart takes orders that vanish, so CIVVIS believes a
        // settler is marching to a site while nothing moves in the real game.
        let phantom: Vec<String> = mirror_state
            .game
            .units
            .values()
            .filter(|u| u.owner == 0 && !mirror_state.civ6_of.contains_key(&u.id))
            .map(|u| format!("{}:{}", u.id, u.kind.as_str()))
            .collect();
        note_bits.push(format!(
            "pre_all_legal={pre_all_legal} pre_war_legal={pre_war_legal} has_met01={has_met} \
             phantom=[{}]",
            phantom.join(",")
        ));
        let legal = mirror_state
            .game
            .legal_actions(0)
            .into_iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
                )
            })
            .count();
        let minors: Vec<String> = mirror_state
            .game
            .players
            .iter()
            .map(|p| format!("{}:{}", p.id, if p.is_minor { "minor" } else { "major" }))
            .collect();
        let g = &mirror_state.game;
        note_bits.push(format!(
            "p1 alive={} at_war={} friends={} allied={} treaty={:?} denounced={:?}",
            g.players.get(1).map(|p| p.alive).unwrap_or(false),
            g.is_at_war(0, 1),
            g.are_friends(0, 1),
            g.are_allied(0, 1),
            g.peace_treaty_until(0, 1),
            g.players[0].denounced_until.get(&1),
        ));
        note_bits.push(format!(
            "war_legal={} met={:?} players=[{}]",
            legal,
            mirror_state.game.players[0].met,
            minors.join(",")
        ));
    }
    note_bits.push(format!(
        "roster={} ",
        mirror_state.game.player_unit_ids(0).len()
    ));
    note_bits.push(format!(
        "movable_before={}/{} movable_after={} winner={:?} logged={}",
        pre_movable,
        ours,
        movable,
        mirror_state.game.winner,
        mirror_state.game.log.len() - before
    ));
    note_bits.push(format!(
        "synced={} units={} cities={} revealed={}",
        mirror_state.turns_synced,
        mirror_state
            .game
            .units
            .values()
            .filter(|u| u.owner == 0)
            .count(),
        mirror_state
            .game
            .cities
            .values()
            .filter(|c| c.owner == 0)
            .count(),
        snapshot.revealed_count()
    ));

    // Batch metadata travels through the existing five-column order transport.
    // The controller attaches it to each queued strike; no persistent host
    // setting can leak the selected gene into a later frame or experiment.
    if ai.live_strike_survival_enabled()
        && orders.iter().any(|order| {
            order.kind == "unit" && matches!(order.verb.as_deref(), Some("ATTACK" | "RANGE_ATTACK"))
        })
    {
        orders.push(Order {
            kind: "combat_policy",
            subject: None,
            verb: Some("DOOMED_BLOW_VETO".to_string()),
            pos: None,
        });
    }
    // A leader statement is answered by the AutoClose layer, often before this
    // frame's orders land, so every batch renews a one-turn lease there.
    if ai.dialogue_never_declares_war_enabled() {
        orders.push(Order {
            kind: "combat_policy",
            subject: None,
            verb: Some("DIALOGUE_NEVER_DECLARES_WAR".to_string()),
            pos: None,
        });
    }
    // `congress-guards-the-leader`: the agent's ballot reads the lease when
    // a World Congress session opens, often before this frame's orders land,
    // so every batch renews it.
    if ai.congress_guards_the_leader_enabled() {
        orders.push(Order {
            kind: "combat_policy",
            subject: None,
            verb: Some("CONGRESS_GUARDS_THE_LEADER".to_string()),
            pos: None,
        });
    }
    // `world-religion-condemns-the-faith`: the same lease for the World
    // Religion ballot.
    if ai.world_religion_condemns_the_faith_enabled() {
        orders.push(Order {
            kind: "combat_policy",
            subject: None,
            verb: Some("WORLD_RELIGION_CONDEMNS".to_string()),
            pos: None,
        });
    }
    if own_column_before.is_some() {
        let (sent, unsent) = split_planned_movers(
            &orders,
            planned_game
                .log
                .since(before)
                .map(|(seat, action)| (*seat, action)),
            &mirror_state.civ6_of,
        );
        let rolled_back = ai.note_host_moves(&mirror_state.game, own_column_before, &sent, &unsent);
        if rolled_back > 0 {
            note_bits.push(format!("own_column_rolled_back={rolled_back}"));
        }
    }
    let body = orders
        .iter()
        .map(|o| o.to_json())
        .collect::<Vec<_>>()
        .join(",");
    let decision = serde_json::json!({
        "schema": 1,
        "player_contract": civvis::ai::player::CONTRACT,
        "turn": state.turn,
        "victory_portfolio": ai.victory_portfolio_snapshot(),
        "frame": state.frame,
        "native_actions": planned_game.log.since(before)
            .filter(|(seat,_)| *seat == 0)
            .map(|(_,action)| action).collect::<Vec<_>>(),
        "finishing_lines": war_finishers.execution,
        "unit_ids": mirror_state.civ6_of,
        "city_ids": mirror_state.cid_of,
        "unmapped": mirror_state.unmapped,
    });
    format!(
        "{{\"turn\":{},\"orders\":[{}],\"note\":{},\"decision\":{}}}",
        state.turn,
        body,
        quote(&note_bits.join("; ")),
        decision,
    )
}

/// Whether this CIVVIS unit has no Civilization VI counterpart in the id map.
fn rebuilt_unit_missing(mirror_state: &civvis::mirror::LiveMirror, uid: u32) -> bool {
    !mirror_state.civ6_of.contains_key(&uid)
}

/// Whether `unit` moving onto `to` is a civilian capture on CIVVIS's board: an
/// enemy civilian or support unit stands there, at war with the mover, with
/// no military unit of theirs on the tile (that tile is an attack problem) and
/// none of ours (a stack, not a capture). Religious units are neither — walking
/// onto one neither captures nor kills it.
fn move_captures(game: &civvis::game::Game, unit: u32, to: (i32, i32)) -> bool {
    let Some(mover) = game.units.get(&unit) else {
        return false;
    };
    let mut civilian = false;
    for other in game.units.values().filter(|other| other.pos == to) {
        if other.owner == mover.owner || !game.is_at_war(mover.owner, other.owner) {
            return false;
        }
        match game.rules.units[other.kind].class.as_str() {
            "military" => return false,
            "civilian" | "support" => civilian = true,
            _ => {}
        }
    }
    civilian
}

/// One CIVVIS action -> one Civilization VI order, or None with a counted reason.
fn translate(
    action: &Action,
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
) -> Option<Order> {
    let civ6_of = &mirror_state.civ6_of;
    match action {
        Action::MoveTo { unit, to } | Action::Move { unit, to } => {
            // ★★★★ A MOVE ONTO THE TILE THE UNIT IS ALREADY STANDING ON IS A NO-OP
            // THE ENGINE ALWAYS REFUSES, and it costs the unit its turn.
            //
            // Measured on run civvis-20260802T030910Z, 84 turns: **18 of 21
            // `move_refused` events had `from_x,from_y` identical to `x,y`** — the
            // destination WAS the origin. Seven different units, so it is not one
            // stuck unit; the mod dutifully recorded each refusal with
            // `dest_impassable: false` on ordinary grass, plains and tundra, which
            // is why the terrain fields never explained them.
            //
            // ⚠ COUNTED, NOT SILENTLY DROPPED. If the AI meant "hold", emitting
            // nothing is exactly right and the unit stays put either way. If it
            // meant to go somewhere and computed its own tile, that is a real
            // planning defect and swallowing the order would hide it — so this
            // lands in `skipped`, the same map every other refusal reason uses,
            // and shows up in the run's notes as `self_tile_move=N`.
            //
            // Compared in AXIAL, on CIVVIS's own board, so no coordinate conversion
            // sits between the two values being tested. `axial_to_offset` still runs
            // afterwards for the orders that survive.
            if let Some(here) = mirror_state.game.units.get(unit).map(|u| u.pos) {
                if (here.0, here.1) == (to.0, to.1) {
                    return None;
                }
            }
            // ★★★★★ A MOVE ONTO AN ENEMY CIVILIAN IS A CAPTURE, AND A BARE
            // MOVE_TO NEVER MAKES IT. CIVVIS captures a civilian by moving onto
            // its tile (`capture_adjacent_civilian`: "civilian capture is
            // movement, not combat"), and this arm sent that move as a plain
            // `MOVE_TO`. The mod's own note on `attackModifiers` says what the
            // host does with one: without `UnitOperationMoveModifiers.ATTACK`
            // "the pathfinder will not enter a plot an enemy is standing on,
            // and the request resolves to 'walk next to it and stop'" —
            // Firaxis's `Civ6Common.lua:152` sets that flag on every melee
            // move a human makes, capture included.
            //
            // Measured over the 273 live runs that carried #2075's rescue
            // (2026-08-18 → 08-25): 278 unit-turns adjacent to an unguarded
            // barbarian-held settler with movement left, 65 `MOVE_TO` orders
            // aimed at its tile, ZERO captures. Every "capture" in that window
            // was a kill that advanced onto a GUARDED settler. So the move
            // crosses as `CAPTURE`: the mod requests the same MOVE_TO with the
            // attack modifier and no strike ledger, and `verify_unit_order`
            // reads the unit standing on the tile, or the civilian gone from
            // it, as the capture.
            let verb = if move_captures(&mirror_state.game, *unit, *to) {
                "CAPTURE"
            } else {
                "MOVE_TO"
            };
            civ6_of.get(unit).map(|civ6| Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some(verb.to_string()),
                pos: Some(civvis::hex::axial_to_offset(to.0, to.1)),
            })
        }
        // ★★★ THE SWAP VERB. `Action::Swap` — legal, tested and correctly refused
        // in the engine since `do_swap`, and chosen by `swap-rotation`
        // (`src/ai/advanced/swap_rotation.rs`) — fell through this match's
        // `_ => None` and was counted `unit_action_untranslated`, so a rotation
        // the arena priced positive on every instrument (docs/LIVE_TACTICS.md
        // §18) could never reach the live board.
        //
        // Civilization VI has a dedicated operation for it, read off the
        // installed game: `Base/Assets/Gameplay/Data/UnitOperations.xml:56`
        // (`UNITOPERATION_SWAP_UNITS`, `VisibleInUI="false"`). The shipped UI
        // requests it with the PARTNER'S plot as the ordinary positional pair —
        // `Base/Assets/UI/Civ6Common.lua:160-161`, inside `RequestMoveOperation`:
        //
        //   if (UnitManager.CanStartOperation( kUnit, UnitOperationTypes.SWAP_UNITS, nil, tParameters) ) then
        //       UnitManager.RequestOperation(kUnit, UnitOperationTypes.SWAP_UNITS, tParameters);
        //
        // so the order is `SWAP` with `pos` = where the partner stands. The
        // partner is read off the mirrored board, not the action: the mod needs
        // a plot, and a partner this bridge cannot see has no plot to name.
        Action::Swap { unit, other } => civ6_of.get(unit).and_then(|civ6| {
            let partner = mirror_state.game.units.get(other)?.pos;
            Some(Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("SWAP".to_string()),
                pos: Some(civvis::hex::axial_to_offset(partner.0, partner.1)),
            })
        }),
        // ★★★ THE AIR VERBS. `Action::AirStrike`, `Action::AirRebase` and
        // `Action::AirPatrol` — legal and tested in the engine since
        // `do_air_strike` / `do_air_rebase` / `do_air_patrol` — fell through
        // this match's `_ => None` and were counted `unit_action_untranslated`,
        // so no aircraft the live seat ever built could fly a sortie, change
        // base or intercept. Civilization VI has one operation for each, read
        // off the installed game (`Base/Assets/Gameplay/Data/UnitOperations.xml`
        // rows `UNITOPERATION_AIR_ATTACK`, `UNITOPERATION_REBASE`,
        // `UNITOPERATION_DEPLOY`; there is no `AIR_PATROL` row — a fighter's
        // patrol is the shipped "Deploy": it flies to a plot within range and
        // intercepts from there). The shipped UI requests all three with the
        // plot as the ordinary positional pair, `WorldInput.lua:2077-2078`
        // (`UnitAirAttack`), `:2418-2419` (`AirUnitDeploy`), `:2486-2487`
        // (`AirUnitReBase`):
        //
        //   if (UnitManager.CanStartOperation( pSelectedUnit, UnitOperationTypes.AIR_ATTACK, nil, tParameters)) then
        //       UnitManager.RequestOperation( pSelectedUnit, UnitOperationTypes.AIR_ATTACK, tParameters);
        //
        // so `pos` is the target plot for `AIR_ATTACK`, the base plot for
        // `REBASE` and the patrol plot for `PATROL`.
        // Strategic bombing uses the same operation: shipped
        // Base/Assets/UI/Panels/UnitPanel.lua:3629 explicitly includes
        // air-pillage plots in the air-attack interface. Omitting this action
        // silently stranded the native bomber on t153–156 of run 161428.
        Action::AirStrike { unit, target } | Action::AirPillage { unit, target } => {
            civ6_of.get(unit).map(|civ6| Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("AIR_ATTACK".to_string()),
                pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
            })
        }
        Action::AirRebase { unit, to } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("REBASE".to_string()),
            pos: Some(civvis::hex::axial_to_offset(to.0, to.1)),
        }),
        Action::AirPatrol { unit, to } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("PATROL".to_string()),
            pos: Some(civvis::hex::axial_to_offset(to.0, to.1)),
        }),
        // ⚠ There is NO attack operation on this build; the resolved list is only
        // MOVE_TO and RANGE_ATTACK, so a melee strike IS a move onto the defended
        // plot. That is how Civilization VI resolves it, not a hack.
        Action::Attack { unit, target } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("ATTACK".to_string()),
            pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
        }),
        // Religious combat uses the same native move-to-attacker route as a
        // melee strike. The target is adjacent by the model's legal-action
        // gate, while the host's MOVE_TO resolves the theological combat.
        //
        // ⚠ READ OFF THE INSTALLED GAME, NOT GUESSED: there is NO
        // `UNITOPERATION_THEOLOGICAL_ATTACK` and no `UNITCOMMAND_` for it
        // either. `Base/Assets/Gameplay/Data/UnitOperations.xml` lists 57
        // operations and none of them is a theological strike, and the shipped
        // Civilopedia says why —
        // `Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:636`:
        //
        //   "Theological combat works just like combat with military units,
        //    just attack one Religious unit with another."
        //
        // So the verb is the ordinary `ATTACK` above, and the mod turns it into
        // the same MOVE_TO-with-the-ATTACK-modifier that Firaxis's own
        // `Civ6Common.lua:152-163` requests for every melee strike a human
        // makes. A religious unit has no ranged combat, so it takes exactly
        // that branch of `RequestMoveOperation`. Inventing a type literal here
        // would have produced an order that could never fire.
        Action::TheologicalAttack { unit, target } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("ATTACK".to_string()),
            pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
        }),
        Action::Ranged { unit, target } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("RANGE_ATTACK".to_string()),
            pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
        }),
        Action::FoundCity { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("FOUND_CITY".to_string()),
            pos: None,
        }),
        Action::LinkUnits { unit, with } => civ6_of.get(unit).and_then(|civ6| {
            civ6_of.get(with).map(|target| Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("ENTER_FORMATION".to_string()),
                // This command is not positional: x/y are target owner/unit id.
                pos: Some((state.seat.local_player, *target as i32)),
            })
        }),
        // ★★★ CORPS AND ARMY: 10,015 DECIDED ACROSS THE LIVE ARCHIVE, ZERO SENT.
        // `CombineUnits` had no arm at all, fell through this match's
        // `_ => None`, and was counted untranslatable — the largest named class
        // left in the live skip tally once the religious, policy and formation
        // repairs had landed, and it is the whole of CIVVIS's unit-consolidation
        // layer. It is still decided today: 5 skips in the newest recorded run,
        // `civvis-20260819T115423Z`.
        //
        // Firaxis models this as TWO commands and the caller picks the tier.
        // Both names were read off the installed game, not recalled:
        //
        //   UNITCOMMAND_FORM_CORPS  Gameplay/Data/UnitCommands.xml:20 and :44
        //   UNITCOMMAND_FORM_ARMY   Gameplay/Data/UnitCommands.xml:21 and :45
        //
        // and the shipped `WorldInput.lua` builds each one from the OTHER unit's
        // owner and id — `FormCorps` at WorldInput.lua:2879-2882 and `FormArmy`
        // at WorldInput.lua:2949-2952:
        //
        //   tParameters[UnitCommandTypes.PARAM_UNIT_PLAYER] = pUnit:GetOwner();
        //   tParameters[UnitCommandTypes.PARAM_UNIT_ID]     = pUnit:GetID();
        //   UnitManager.RequestCommand(pSelectedUnit,
        //                              UnitCommandTypes.FORM_CORPS, tParameters);
        //
        // That is the same non-positional owner/id pair `LinkUnits` already
        // rides in `pos`, so the existing convention carries it unchanged.
        //
        // The tier is `Game::can_combine_units`'s own rule, restated: two
        // standard units make a Corps, a standard joining a Corps makes an Army.
        //
        // ⚠ THE TIER IS READ OFF THE HOST NOW, WHICH IS WHAT MAKES FORM_ARMY
        // REACHABLE AT ALL. When #2373 landed this arm, the export carried no
        // military formation — only `formation_count`, Firaxis's ESCORT stack
        // size, which is a different mechanism and belongs to `LinkUnits` — and
        // `mirror.rs` never wrote `Unit::formation`. The live seat runs
        // `--fresh-board`, so every unit was reconstructed as STANDARD and this
        // could only ever emit FORM_CORPS; the Army half was unreachable outside
        // a persistent mirror carrying a tier CIVVIS itself had raised.
        //
        // The mod now exports `formation` from Firaxis's own
        // `Unit:GetMilitaryFormation()` and `mirror.rs` writes it, so this reads
        // Civilization VI's tier rather than a guess. An unreadable tier crosses
        // as the mod's −1 and the mirror keeps the board's own value rather than
        // flattening it to standard — unknown must never read as STANDARD, which
        // is how `GetDefenseStrength` went unnoticed for the project's life.
        // A host that still disagrees answers with a NAMED refusal
        // (`cannot_form_corps` / `cannot_form_army`) rather than a silent no-op,
        // which is what makes any remaining divergence measurable.
        Action::CombineUnits { unit, with } => civ6_of.get(unit).and_then(|civ6| {
            civ6_of.get(with).map(|target| {
                let tier = |uid: &u32| {
                    mirror_state
                        .game
                        .units
                        .get(uid)
                        .map_or(0, |unit| unit.formation)
                };
                let verb = if tier(unit) == 0 && tier(with) == 0 {
                    "FORM_CORPS"
                } else {
                    "FORM_ARMY"
                };
                Order {
                    kind: "unit",
                    subject: Some(*civ6),
                    verb: Some(verb.to_string()),
                    // Not positional: x/y are the target's owner and unit id,
                    // exactly as `LinkUnits` sends them.
                    pos: Some((state.seat.local_player, *target as i32)),
                }
            })
        }),
        Action::UnlinkUnits { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("EXIT_FORMATION".to_string()),
            pos: None,
        }),
        // ★★★ A TRADER THAT CANNOT BE GIVEN A ROUTE IS PRODUCTION AND GOLD SPENT ON A
        // UNIT THAT WILL NEVER ACT. `Action::TradeRoute` was on the untranslatable
        // list, so every trader CIVVIS built stood where it was made for the rest of
        // the game — `civ6_watchdogs.py` names one in every run, motionless for 114
        // turns in the longest case.
        //
        // Civilization VI takes the DESTINATION CITY's plot, so the order carries the
        // target city's position rather than a unit id the bridge would have to map
        // twice. The mod resolves the operation and reports a refusal if the engine
        // will not take it, which is the honest failure for something untested against
        // a live game.
        Action::TradeRoute { unit, city } => civ6_of.get(unit).and_then(|civ6| {
            mirror_state.game.cities.get(city).map(|destination| Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("TRADE_ROUTE".to_string()),
                pos: Some(civvis::hex::axial_to_offset(
                    destination.pos.0,
                    destination.pos.1,
                )),
            })
        }),
        // A builder improving the tile it stands on. Dropping this is what kept the
        // mirror looking undeveloped and made CIVVIS order builder after builder.
        // Preserve the currency and formation CIVVIS chose. The old bridge accepted
        // only Gold and silently dropped every Faith purchase; two were lost on
        // recorded turn 92 (the old diagnostic reported four because it counted each
        // skip twice). For unit purchases `x` is the CIVVIS formation tier (0/1/2),
        // while district purchases use x/y as their actual OFFSET placement. The item
        // kind makes those meanings unambiguous.
        Action::Buy {
            city,
            unit,
            formation,
            currency,
        } if matches!(currency.as_str(), "gold" | "faith") => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: if currency == "faith" {
                    "purchase_faith"
                } else {
                    "purchase"
                },
                subject: Some(*civ6),
                verb: Some(civ6_unit_type(unit)),
                pos: Some((*formation as i32, -1)),
            }),
        Action::BuyBuilding {
            city,
            building,
            currency,
        } if matches!(currency.as_str(), "gold" | "faith") => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: if currency == "faith" {
                    "purchase_faith"
                } else {
                    "purchase"
                },
                subject: Some(*civ6),
                verb: Some(civ6_building_type(building)),
                pos: None,
            }),
        Action::BuyDistrict {
            city,
            district,
            pos,
            currency,
        } if matches!(currency.as_str(), "gold" | "faith") => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: if currency == "faith" {
                    "purchase_faith"
                } else {
                    "purchase"
                },
                subject: Some(*civ6),
                verb: Some(format!(
                    "DISTRICT_{}",
                    district.as_str().to_ascii_uppercase()
                )),
                pos: Some(civvis::hex::axial_to_offset(pos.0, pos.1)),
            }),
        // ★★★★★ BUYING GROUND. Discarded for the life of this bridge: `BuyPlot`
        // reached `translate` only to be counted in the `skipped` tally, 25 of them
        // across the runs of 2026-07-31, while cities finished games on the tiles
        // they happened to grow into.
        //
        // A bought plot is how a city reaches the resource, the river or the hill it
        // needs, and a treasury that ends a game unspent (1459 gold at t182 of run
        // civvis-clean-20260731T191337Z) is a treasury that bought no ground.
        //
        // ⚠ AXIAL IN, OFFSET OUT, like every other position this file sends —
        // Civilization VI reads offsets and CIVVIS keeps axial, and nothing complains
        // when they are mixed because both are pairs of small integers.
        Action::BuyPlot { city, pos, cost } => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: "buy_plot",
                subject: Some(*civ6),
                verb: Some(format!("GOLD_CAP={cost:.6}")),
                pos: Some(civvis::hex::axial_to_offset(pos.0, pos.1)),
            }),
        Action::Improve { unit, improvement } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            // These simulator improvements are native operations, not rows in
            // Firaxis' Improvements table. Both land and sea artifacts use EXCAVATE.
            verb: Some(match improvement.as_str() {
                "archaeological_dig" | "shipwreck_excavation" => "EXCAVATE".to_string(),
                "national_park" => "DESIGNATE_PARK".to_string(),
                // The engine's builder operations (`Game::builder_operations`)
                // are native unit operations too, not Improvements rows:
                // `IMPROVE:IMPROVEMENT_CHOP_WOODS` found no row and the host
                // picked an improvement of its own. Base/Assets/Gameplay/Data/
                // UnitOperations.xml:35 REMOVE_FEATURE, :23 HARVEST_RESOURCE,
                // :32 PLANT_FOREST, none with an InterfaceMode.
                "chop_woods" | "chop_rainforest" | "clear_marsh" => "REMOVE_FEATURE".to_string(),
                "harvest_resource" => "HARVEST_RESOURCE".to_string(),
                "plant_woods" => "PLANT_FOREST".to_string(),
                _ => format!("IMPROVE:{}", civ6_improvement_type(improvement)),
            }),
            pos: None,
        }),
        Action::PerformConcert { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("TOURISM_BOMB".to_string()),
            pos: None,
        }),
        // A builder repair is a distinct Firaxis unit operation.  Dropping it
        // strands pillaged improvements even though CIVVIS already chose the
        // repair and the builder is standing on the target tile.
        Action::RepairImprovement { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("REPAIR".to_string()),
            pos: None,
        }),
        Action::Fortify { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("FORTIFY".to_string()),
            pos: None,
        }),
        // ★★★ PILLAGE WAS DECIDED AND DROPPED. `doctrine_action` has light
        // cavalry pillage before routine combat and the role basics let heavy
        // cavalry pillage as a fallback (`docs/TACTICS.md` §8); the bridge had
        // no arm for it, so on the live seat those units did nothing instead.
        // The host operation is parameterless — the unit pillages the tile it
        // stands on — and the mod resolves `UNITOPERATION_PILLAGE` by name.
        Action::Pillage { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("PILLAGE".to_string()),
            pos: None,
        }),
        Action::ContributeProject { unit, city } => {
            // Expansion2/Data/Expansion1_UnitCommands.xml:11 defines the
            // parameterless PROJECT_PRODUCTION command. Preserve the intended
            // district as a position guard: the native command acts where the
            // Builder stands, never on a city id supplied as a parameter.
            let actor = own_unit(state, *civ6_of.get(unit)?)?;
            let native_city = mirror_state
                .cid_of
                .iter()
                .find(|(_, cid)| **cid == *city)
                .and_then(|(id, _)| state.cities.iter().find(|c| c.id == *id))?;
            if actor.kind != "UNIT_BUILDER"
                || !actor.build_charges.is_some_and(|charges| charges > 0)
                || !native_city
                    .producing
                    .as_deref()
                    .is_some_and(|head| head.starts_with("PROJECT_"))
                || !native_city.districts.iter().any(|district| {
                    district.complete
                        && !district.pillaged
                        && (district.x, district.y) == (actor.x, actor.y)
                })
            {
                return None;
            }
            Some(Order {
                kind: "unit",
                subject: Some(actor.id),
                verb: Some("CONTRIBUTE_PROJECT".to_string()),
                pos: Some((actor.x, actor.y)),
            })
        }
        Action::UpgradeUnit { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("UPGRADE".to_string()),
            pos: None,
        }),
        // `bankruptcy-exits-the-drain`: the same `UNITCOMMAND_DELETE` the
        // founded Prophet's retirement sends, verified by the unit's absence.
        Action::DisbandUnit { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("DELETE".to_string()),
            pos: None,
        }),
        Action::Promote { unit, promotion } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some(format!(
                "PROMOTE:{}",
                civ6_unit_promotion_name(promotion.as_str())
            )),
            pos: None,
        }),
        // ★★★ ESPIONAGE, WHICH HAD NO ROUTE TO A LIVE GAME AT ALL. The engine
        // models twelve missions and the AI aims them at the denial target
        // (`great_work_heist` 340 against a Culture leader, `disrupt_rocketry`
        // 290 against a Science one) — and all three actions fell through this
        // match's `_ => None` and were counted untranslatable, on top of a
        // `Game::spies` the live mirror left empty, so nothing was ever chosen
        // to drop in the first place.
        //
        // A spy's id IS its unit id on the live seat (`seat_live_spies`), so
        // the same `civ6_of` mapping every other unit order uses applies here.
        // Firaxis' own EspionagePopup issues these as PARAM_X/PARAM_Y plus
        // `RequestOperation`, and travelling to a new city takes the identical
        // shape — which is why the destination is required rather than
        // optional: an empty-parameter spy operation silently does nothing.
        Action::AssignSpy { spy, city } => {
            let civ6 = civ6_of.get(spy)?;
            let target = mirror_state.game.cities.get(city)?.pos;
            Some(Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("SPY_TRAVEL_NEW_CITY".to_string()),
                pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
            })
        }
        Action::SpyMission {
            spy,
            mission,
            target,
        } => {
            let civ6 = civ6_of.get(spy)?;
            // The engine's own mission names are the host's, lowercased. Keep
            // the mapping a plain uppercase rather than a table: a name the
            // host does not know is refused by `CanStartOperation` and named
            // in the refusal ledger, which is a better failure than a silent
            // table miss.
            Some(Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some(format!("SPY_{}", mission.to_uppercase())),
                pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
            })
        }
        Action::PromoteSpy { spy, promotion } => civ6_of.get(spy).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some(format!(
                "PROMOTE:{}",
                civ6_unit_promotion_name(promotion.as_str())
            )),
            pos: None,
        }),
        // ★★★ THE ONE ORDER THAT TOUCHES AN ENEMY MISSIONARY. Religious units
        // are excluded from ordinary combat by design — `enemy_combat_target_at`,
        // `do_attack` and `do_ranged` all require `class == "military"`, and
        // walking onto one neither captures nor kills it — so Condemn Heretic
        // and theological combat are the only removals in the model. CIVVIS has
        // been deciding this action all along (`condemn_step`, gated on
        // `victory_planning`, war, and standing on the target) and it fell
        // straight through this match's `_ => None` and was counted
        // untranslatable. The unit's co-location IS the target, which is why
        // nothing but the subject crosses: Firaxis gives the command row no
        // `InterfaceMode`, so its own UnitPanel requests it parameterless too.
        Action::CondemnHeretic { unit, .. } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("CONDEMN_HERETIC".to_string()),
            pos: None,
        }),
        Action::Spread { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("SPREAD_RELIGION".to_string()),
            pos: None,
        }),
        // Evangelization opens Firaxis's native belief prompt. Carry the
        // exact belief CIVVIS selected with the Apostle so the control mod can
        // start that operation and answer the resulting prompt without
        // substituting a heuristic choice.
        Action::EvangelizeBelief { unit, belief } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some(format!(
                "EVANGELIZE_BELIEF:BELIEF_{}",
                belief.as_str().to_ascii_uppercase()
            )),
            pos: None,
        }),
        // These are direct, parameterless entries in Firaxis' UnitOperations
        // table, just like SPREAD_RELIGION. Evangelization remains separate:
        // it opens an asynchronous belief-selection flow rather than applying
        // an operation immediately.
        Action::HealReligious { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("RELIGIOUS_HEAL".to_string()),
            pos: None,
        }),
        // `UNITOPERATION_REMOVE_HERESY`, read off the installed game at
        // `Base/Assets/Gameplay/Data/UnitOperations.xml:36` (the type) and `:95`
        // (the row). That row carries **no `InterfaceMode`**, and the shipped
        // `Panels/UnitPanel.lua:2518-2535` shows exactly what that means: with no
        // mode it takes the "No mode needed, just do the operation" branch and
        // calls `UnitManager.RequestOperation(pSelectedUnit, actionHash)` with no
        // parameter table. So nothing but the subject crosses — an Inquisitor
        // purges the city it is standing in, and the co-location IS the target.
        Action::RemoveHeresy { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("REMOVE_HERESY".to_string()),
            pos: None,
        }),
        Action::LaunchInquisition { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("LAUNCH_INQUISITION".to_string()),
            pos: None,
        }),
        Action::ConvertBarbarians { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("CONVERT_BARBARIANS".to_string()),
            pos: None,
        }),
        // The host validates and dispatches the exact chosen declaration.
        // Dropping the casus belli here turned Formal War into the generic
        // operation Canada refuses (native run 20260926T185025Z).
        Action::DeclareWarWithCasusBelli {
            player,
            casus_belli,
        } => {
            let statement = civvis::game::native_war_statement(casus_belli)?;
            Some(Order {
                kind: "war",
                subject: host_player_target(mirror_state, state, *player),
                verb: Some(statement.to_string()),
                pos: None,
            })
        }
        Action::DeclareWar { player } => Some(Order {
            kind: "war",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("DECLARE".to_string()),
            pos: None,
        }),
        // ★★★★★ PEACE, WHICH HAD NO ARM AT ALL. A losing war could never be
        // exited: run civvis-20260801T221459Z spent 93 turns emitting MakePeace
        // — "Offering peace" in why.log every turn from t118 to the end — while
        // every one fell through to the skipped tally and the harness fought on.
        // Same city-state-safe seat mapping as the war arm above; the Lua side gates on
        // IsAtWarWith and answers with the shipped deal shape.
        Action::MakePeace { player } => Some(Order {
            kind: "peace",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("MAKE_PEACE".to_string()),
            pos: None,
        }),
        // ⚠ THE PLANNER'S ACTUAL PEACE VEHICLE IS A DEAL, NOT MakePeace.
        // `advanced.rs` deliberately proposes `ProposeDeal { peace: true, .. }`
        // so the recipient's valuation answers, instead of the engine's direct
        // MakePeace (which let a defender end a war the conqueror valued) —
        // the replay of civvis-20260801T221459Z journals "Offering peace" 106
        // times and every one was a ProposeDeal falling into the `deal` skip
        // tally. Both variants funnel to the one Civilization VI peace order;
        // a friendship-only deal has its own arm below, and every other deal
        // (open borders, gold, an alliance bundle) has no counterpart and
        // stays skipped.
        Action::ProposeDeal {
            player,
            peace: true,
            ..
        } => Some(Order {
            kind: "peace",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("MAKE_PEACE".to_string()),
            pos: None,
        }),
        // ★★ THE PLANNER'S ALLIANCE, WHICH HAD NO ARM. `propose_strategic_alliance`
        // proposes `ProposeDeal { friendship: true, alliance: Some(kind), .. }`
        // 8 to 15 times a game on the live seat ("Proposing a Research Alliance
        // to Poland", why.log of civvis-20261030T043401Z at t132 and t142), and
        // every one fell into the skip tally: the friendship arm below requires
        // `alliance: None`, and the host has no bundled friendship-and-alliance
        // deal (DIPLOACTION_ALLIANCE needs a declared friendship and Civil
        // Service on both sides, Base/Assets/Gameplay/Data/DiplomaticActions.xml:69).
        // The agent's `alliance` arm asks the friendship while there is none and
        // the alliance once there is, as the shipped screen builds it
        // (DLC/Expansion1/UI/Replacements/DiplomacyActionView_Expansion1.lua:
        // 141-217: an AGREEMENTS item, subtype ALLIANCE, value type
        // ALLIANCE_<KIND>, in a MAKE_DEAL session). An alliance pays its level in
        // Favor every turn (`WORLD_CONGRESS_ALLIANCE_FAVOR_PER_TURN`).
        Action::ProposeDeal {
            player,
            give_gold,
            request_gold,
            peace: false,
            alliance: Some(kind),
            ..
        } if *give_gold <= 0.0 && *request_gold <= 0.0 => {
            let verb = civ6_alliance_type(kind)?;
            Some(Order {
                kind: "alliance",
                subject: host_player_target(mirror_state, state, *player),
                verb: Some(verb.to_string()),
                pos: None,
            })
        }
        // A friendship and nothing else — no gold, passage or alliance
        // bundled — is the host's DIPLOACTION_DECLARE_FRIENDSHIP, asked as a
        // session like a denouncement: DiplomacyActionView.lua:472-473 answers
        // CHOICE_DECLARE_FRIENDSHIP with `RequestSession(local, other,
        // "DECLARE_FRIEND")`. On the advanced turn only the opt-in
        // `befriend-the-strongest` proposes this shape, so the arm carries
        // nothing to the host until that gene is forced on.
        Action::ProposeDeal {
            player,
            give_gold,
            request_gold,
            open_borders: false,
            friendship: true,
            peace: false,
            alliance: None,
        } if *give_gold <= 0.0 && *request_gold <= 0.0 => Some(Order {
            kind: "friendship",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("DECLARE_FRIEND".to_string()),
            pos: None,
        }),
        Action::Research { tech, .. } => Some(Order {
            kind: "research",
            subject: None,
            verb: Some(civ6_tech_name(tech.as_str())),
            pos: None,
        }),
        Action::Civic { civic, .. } => Some(Order {
            kind: "civic",
            subject: None,
            verb: Some(civ6_civic_name(civic.as_str())),
            pos: None,
        }),
        Action::Government { government } => Some(Order {
            kind: "government",
            subject: None,
            verb: Some(format!(
                "GOVERNMENT_{}",
                government.as_str().to_ascii_uppercase()
            )),
            pos: None,
        }),
        Action::ChoosePantheon { belief } => Some(Order {
            kind: "pantheon",
            subject: None,
            verb: Some(format!("BELIEF_{}", belief.as_str().to_ascii_uppercase())),
            pos: None,
        }),
        Action::ChooseDedication { dedication } => Some(Order {
            kind: "dedication",
            subject: None,
            verb: Some(civ6_dedication_name(dedication.as_str())?.to_string()),
            pos: None,
        }),
        Action::AppointGovernor { governor, city } => {
            let governor = mirror::civ6_governor_name(governor.as_str())?;
            host_city_target(mirror_state, state, *city).map(|(city, owner)| Order {
                kind: "governor_appoint",
                subject: Some(city),
                verb: Some(governor.to_string()),
                pos: Some((owner, -1)),
            })
        }
        Action::ReassignGovernor { governor, city } => {
            let governor = mirror::civ6_governor_name(governor.as_str())?;
            host_city_target(mirror_state, state, *city).map(|(city, owner)| Order {
                kind: "governor_assign",
                subject: Some(city),
                verb: Some(governor.to_string()),
                pos: Some((owner, -1)),
            })
        }
        Action::PromoteGovernor {
            governor,
            promotion,
        } => Some(Order {
            kind: "governor_promote",
            subject: None,
            verb: Some(format!(
                "{},{}",
                mirror::civ6_governor_name(governor.as_str())?,
                mirror::civ6_governor_promotion(promotion.as_str())?
            )),
            pos: None,
        }),
        Action::FoundReligion { follower, founder } => Some(Order {
            kind: "religion",
            subject: None,
            verb: Some(format!(
                "BELIEF_{},BELIEF_{}",
                follower.as_str().to_ascii_uppercase(),
                founder.as_str().to_ascii_uppercase()
            )),
            pos: None,
        }),
        Action::Produce { city, item } => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .and_then(|(civ6, _)| {
                civ6_live_build_name(item, &mirror_state.game).map(|name| Order {
                    kind: "produce",
                    subject: Some(*civ6),
                    verb: Some(name),
                    pos: civ6_build_pos(item),
                })
            }),
        // ★★★ GREAT PERSON POINTS AT THE THRESHOLD ARE YIELD ALREADY PAID FOR.
        // These two actions were on the untranslatable list, so every claim CIVVIS
        // decided on fell into the skipped tally — 260 RecruitGreatPerson and 25
        // PatronizeGreatPerson skips in run civvis-20260815T020330Z alone, while
        // four classes sat above 370 banked points. The class crosses as the
        // Firaxis `GREAT_PERSON_CLASS_*` name (the exact inverse of the mirror's
        // suffix-lowercase import); the mod resolves WHICH individual that class
        // currently offers, because only the live timeline knows.
        Action::RecruitGreatPerson { kind } => Some(Order {
            kind: "gp_recruit",
            subject: None,
            verb: Some(format!("GREAT_PERSON_CLASS_{}", kind.to_ascii_uppercase())),
            pos: None,
        }),
        Action::PatronizeGreatPerson { kind, currency } => Some(Order {
            kind: if currency == "faith" {
                "gp_patronize_faith"
            } else {
                "gp_patronize"
            },
            subject: None,
            verb: Some(format!("GREAT_PERSON_CLASS_{}", kind.to_ascii_uppercase())),
            pos: None,
        }),
        // A city under attack that never fires back was the other big skip:
        // 98 CityStrike decisions dropped in the same run, all of them while at
        // war. The strike is free damage the defender is already owed, and
        // holding a city is worth double the score of taking one.
        Action::CityStrike { city, target } => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: "city_strike",
                subject: Some(*civ6),
                verb: None,
                pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
            }),
        // The encampment's own gun, silent for the same reason the city's was:
        // 21 EncampmentStrike skips by turn 181 of run civvis-20260815T024518Z,
        // all at war. Keyed by the owning city exactly like CityStrike; the mod
        // walks that city's districts to find the encampment, because only the
        // live game knows where it stands.
        Action::EncampmentStrike { city, target } => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: "encampment_strike",
                subject: Some(*civ6),
                verb: None,
                pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
            }),
        Action::DistrictStrike {
            city,
            source,
            target,
        } => {
            let live = mirror_state.game.cities.get(city)?;
            let district = live.defending_districts.iter().find(|d| d.pos == *source)?;
            if !live
                .districts
                .iter()
                .any(|(kind, pos)| *kind == district.kind && *pos == *source)
            {
                return None;
            }
            mirror_state
                .cid_of
                .iter()
                .find(|(_, cid)| **cid == *city)
                .map(|(native, _)| {
                    let (x, y) = civvis::hex::axial_to_offset(source.0, source.1);
                    Order {
                        kind: "district_strike",
                        subject: Some(*native),
                        verb: Some(format!("{x}:{y}")),
                        pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
                    }
                })
        }
        // ★★★ THE CAPTURED CITY'S DISPOSITION. `KeepCity` / `RazeCity` /
        // `LiberateCity` are mandatory and exclusive on the board
        // (`pending_city_capture_actions`: nothing else is legal until one is
        // chosen) and fell through this match's `_ => None`, so the host's own
        // default — the `CONSIDER_RAZE_CITY` blocker the mod lists as soft,
        // i.e. keep — decided every capture. Civilization VI has ONE command
        // for all three, `CityCommandTypes.DESTROY`, told apart by a directive
        // flag; the shipped `Base/Assets/UI/Popups/RazeCity.lua` buttons:
        //
        //   :39  tParameters[UnitOperationTypes.PARAM_FLAGS] = CityDestroyDirectives.KEEP;
        //   :48  tParameters[UnitOperationTypes.PARAM_FLAGS] = CityDestroyDirectives.RAZE;
        //   :17  tParameters[UnitOperationTypes.PARAM_FLAGS] = CityDestroyDirectives.LIBERATE_FOUNDER;
        //   :18  if (CityManager.CanStartCommand( g_pSelectedCity, CityCommandTypes.DESTROY, tParameters)) then
        //   :20      CityManager.RequestCommand( g_pSelectedCity, CityCommandTypes.DESTROY, tParameters);
        //
        // so the order is kind `city`, `subject` = the host city id, the verb
        // names the directive, and `pos` carries the city plot so the verdict
        // can ask who holds it next frame. The engine's `do_liberate_city`
        // returns the city to `original_owner`, the founder — the
        // LIBERATE_FOUNDER button, not the owner-before-occupation one.
        Action::KeepCity { city } | Action::RazeCity { city } | Action::LiberateCity { city } => {
            let verb = match action {
                Action::KeepCity { .. } => "KEEP",
                Action::RazeCity { .. } => "RAZE",
                _ => "LIBERATE",
            };
            let pos = mirror_state.game.cities.get(city)?.pos;
            mirror_state
                .cid_of
                .iter()
                .find(|(_, cid)| **cid == *city)
                .map(|(civ6, _)| Order {
                    kind: "city",
                    subject: Some(*civ6),
                    verb: Some(verb.to_string()),
                    pos: Some(civvis::hex::axial_to_offset(pos.0, pos.1)),
                })
        }
        // Delegations and embassies buy diplomatic visibility (a combat bonus
        // against that rival) and a relationship modifier for pocket change,
        // and both were untranslatable: 16 SendDelegation and 7 SendEmbassy
        // skips in run civvis-20260815T020330Z, 8 more delegations by turn 70
        // of the next game. Same seat-mapping as the war arm above; the verb
        // is the session name the Lua side hands to DiplomacyManager.
        Action::SendDelegation { player } => Some(Order {
            kind: "delegation",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("DIPLOMATIC_DELEGATION".to_string()),
            pos: None,
        }),
        Action::SendEmbassy { player } => Some(Order {
            kind: "delegation",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("RESIDENT_EMBASSY".to_string()),
            pos: None,
        }),
        // A denouncement is a diplomatic SESSION on the host, not a player
        // operation: the shipped view opens it with
        // `DiplomacyManager.RequestSession(local, other, "DENOUNCE")`
        // (DiplomacyActionView.lua:469), the same call the delegation arm
        // above makes with its own session name. Until 2026-08-27 the action
        // fell through `_ => None` and the host's grievances the board now
        // reads (#2590) could raise a denouncement it had no way to deliver.
        Action::Denounce { player } => Some(Order {
            kind: "denounce",
            subject: host_player_target(mirror_state, state, *player),
            verb: Some("DENOUNCE".to_string()),
            pos: None,
        }),
        // ★★★★★ THE ENVOYS THE SEAT IS HOLDING, SPENT. `SendEnvoy` never reached
        // this match — not because it had no counterpart, but because the
        // mirror never carried `envoys_free`, so the planning board never
        // enumerated it (see `mirror::apply_mirrored_envoys_free`). Measured on
        // the twelve Settler games of 2026-08-15/16: 40–70 envoys held unspent
        // at the end, 0 suzerainties in 11 of 12, while CIVVIS's own
        // `advanced_envoys` prices the 1/3/6 type tiers, the suzerainty and
        // denial exactly. One order per envoy; the mod issues one
        // `GIVE_INFLUENCE_TOKEN` per order through a handle it fetches fresh
        // for each — the stale-handle write is the one concrete defect the old
        // Lua lane's crash was ever pinned on. The subject is Firaxis's minor
        // player id, by the mirror's own seating rule rather than a city plot,
        // so a city-state met before its centre is in view still gets its
        // envoy.
        Action::SendEnvoy { player } => {
            host_minor_target(mirror_state, state, *player).map(|minor| Order {
                kind: "envoy",
                subject: Some(minor),
                verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
                pos: None,
            })
        }
        // ★★★★ THE LEVY, WHICH THE PLAN ASKED FOR 44 TIMES A GAME AND NEVER GOT.
        // `levy_city_state_military` fires at war (and urgently under
        // Conquest/Recovery) for the suzerained city-state whose visible army
        // is the most strength per gold, and `LevyMilitary` was the single
        // most-skipped action in the pre-#1765 tally — moot while the seat held
        // no suzerainty, live now that it spends its envoys. A levied army is
        // the one force that does not have to be built first, on a seat whose
        // dominant loss mode is a tech-superior rival's siege. The mod pays
        // Firaxis's own quote (`GetLevyMilitaryCost`) against the treasury and
        // refuses on `CanLevyMilitary`, so a mirror-priced levy the host will
        // not sell is a named refusal, not a phantom army.
        Action::LevyMilitary { player } => {
            host_minor_target(mirror_state, state, *player).map(|minor| Order {
                kind: "levy",
                subject: Some(minor),
                verb: Some("LEVY_MILITARY".to_string()),
                pos: None,
            })
        }
        // A sale the planner decided — surplus luxury copies, a strategic
        // block, a favor block — for the rival's gold. See `sale_verb`; the
        // floor rides in `x`, the Lua arm asks the rival's own valuation with
        // EQUALIZE and closes only at or above it. The one purchase with an
        // arm is Open Borders (`border_buy_ceiling`); anything else in a
        // `Trade` (another purchase, a Great Work, a city, a mutual
        // borders swap) stays skipped and named.
        Action::Trade {
            player,
            offer,
            request,
        } => {
            // Our own book value of the quote, read off the mirrored board:
            // the seat is 0, majors take seats 1.. in export order.
            let my_value = mirror_state
                .game
                .trade_utilities(0, *player, offer, request)
                .0;
            if let Some(verb) = sale_verb(offer) {
                let floor = sale_floor(request, my_value)?;
                Some(Order {
                    kind: "sell",
                    subject: host_player_target(mirror_state, state, *player),
                    verb: Some(verb),
                    pos: Some((floor, 0)),
                })
            } else {
                let worth = mirror_state.game.passage_gold_value(0);
                let ceiling = border_buy_ceiling(offer, request, worth)?;
                Some(Order {
                    kind: "buy",
                    subject: host_player_target(mirror_state, state, *player),
                    verb: Some("OPEN_BORDERS".to_string()),
                    pos: Some((ceiling, 0)),
                })
            }
        }
        // A naval raider lands on the coast it stands beside. The shipped UI
        // requests it with the plot as the ordinary pair,
        // `Base/Assets/UI/WorldInput.lua:2331-2353` (`CoastalRaid`).
        Action::CoastalRaid { unit, target } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("COASTAL_RAID".to_string()),
            pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
        }),
        // A device leaves one of two ways, and the host has a different call
        // for each: a carrier is a unit operation
        // (`WorldInput.lua:2162-2168`, `WMDStrike`), a silo is a command of
        // the city that owns it, aimed from the silo's plot
        // (`WorldInput.lua:2264-2272`, `ICBMStrike`). The engine picks the
        // platform; `mirror_state.game` is still the board it was picked on.
        Action::WmdStrike {
            city,
            target,
            thermonuclear,
        } => {
            let weapon = if *thermonuclear {
                "WMD_THERMONUCLEAR_DEVICE"
            } else {
                "WMD_NUCLEAR_DEVICE"
            };
            let platform = *mirror_state
                .game
                .wmd_platforms(
                    civvis::game::MIRRORED_SEAT,
                    Some(*city),
                    *target,
                    *thermonuclear,
                )
                .first()?;
            let pos = Some(civvis::hex::axial_to_offset(target.0, target.1));
            match platform.unit {
                Some(carrier) => civ6_of.get(&carrier).map(|civ6| Order {
                    kind: "unit",
                    subject: Some(*civ6),
                    verb: Some(weapon.to_string()),
                    pos,
                }),
                None => {
                    let (x, y) = civvis::hex::axial_to_offset(platform.from.0, platform.from.1);
                    mirror_state
                        .cid_of
                        .iter()
                        .find(|(_, cid)| **cid == *city)
                        .map(|(civ6, _)| Order {
                            kind: "wmd_strike",
                            subject: Some(*civ6),
                            verb: Some(format!("{x}:{y}:{weapon}")),
                            pos,
                        })
                }
            }
        }
        // ⚠ NO WILDCARD HERE, ON PURPOSE. This match used to end in `_ => None`,
        // so every new `Action` reached the live board as nothing at all and
        // the compiler said nothing: the air verbs, swaps, coastal raids and
        // nuclear strikes each sat there until somebody happened to notice.
        // A new variant now fails to compile until it is given an arm above
        // or a reason below. These are the orders the live seat does not send.
        //
        // The host's own flow answers these: the mod's deal and congress
        // handlers respond to the popups, the mod appoints and posts its own
        // Governors, and the harness ends the turn.
        Action::AcceptDeal { .. }
        | Action::RejectDeal { .. }
        | Action::CongressVote { .. }
        | Action::EndTurn
        | Action::AssignGovernor { .. }
        // The caller folds every policy change into one deck order.
        | Action::SlotPolicy { .. }
        | Action::UnslotPolicy { .. }
        // Game modes the live seat does not play, or CIVVIS's own Future Era.
        | Action::ChooseSecretSociety { .. }
        | Action::FoundCorporation { .. }
        | Action::MoveProduct { .. }
        | Action::AimMassDriver { .. }
        | Action::MassDriverStrike { .. }
        // Real host orders with no bridge arm yet. Any the planner chooses
        // lands in the run's `skipped` notes by variant name.
        | Action::BuildRailroad { .. }
        | Action::ContributeDistrict { .. }
        | Action::DemandGold { .. }
        | Action::PriorityTarget { .. }
        | Action::ProposeDefensivePact { .. }
        | Action::ProposeJointWar { .. }
        | Action::RequestPromise { .. }
        | Action::Upgrade { .. }
        // The arms above are guarded, so these catch what the guards refuse:
        // a purchase in anything but gold or faith, and any deal that is not
        // peace, an alliance or a bare friendship (open borders, gold).
        | Action::Buy { .. }
        | Action::BuyBuilding { .. }
        | Action::BuyDistrict { .. }
        | Action::ProposeDeal { .. } => None,
    }
}

// ---- order postconditions -----------------------------------------------------
//
// ★★★★★ AN ORDER IS APPLIED WHEN THE NEXT FRAME SHOWS IT, NOT WHEN THE MOD'S
// ARM RETURNED `ok`. Every accounting the bridge had was on the ISSUING side:
// `orders_applied` counted arms whose `pcall` returned true, and that number ran
// 95–98% while a Settler was requested on 83 consecutive turns with nothing
// built, a purchase bought nothing, and a `MOVE_TO (14,11)` from (13,11) ended
// at (12,9). The receiving side is the next `state` frame, and this is the one
// place that sees both frames: what left for the host on turn T, and what the
// host exported at the start of turn T+1.
//
// The verdicts travel back through the orders channel as rows of kind
// `order_verified` / `order_failed` / `turn_verified`; the mod is the ledger's
// only writer (a second appender interleaves partial lines into
// `events.jsonl`), so it re-emits them as events. `civ6_ladder.orders_totals`
// then counts VERIFIED orders as `orders_applied` and keeps the mod's own
// return-code count beside it as `orders_reported`, so the gap is a number on
// the ledger instead of a memory note.

/// One order as it left for the host: exactly the shape the brain forwards.
#[derive(Clone, Debug, PartialEq)]
struct IssuedOrder {
    kind: String,
    subject: Option<i64>,
    verb: Option<String>,
    pos: Option<(i32, i32)>,
}

impl IssuedOrder {
    /// Read the orders out of a `decide` reply, verdict rows excluded.
    fn from_reply(reply: &str) -> Vec<IssuedOrder> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(reply) else {
            return Vec::new();
        };
        value
            .get("orders")
            .and_then(|orders| orders.as_array())
            .map(|orders| {
                orders
                    .iter()
                    .filter_map(|o| {
                        let kind = o.get("kind")?.as_str()?.to_string();
                        if VERDICT_ROW_KINDS.contains(&kind.as_str()) {
                            return None;
                        }
                        let x = o.get("x").and_then(|v| v.as_i64());
                        let y = o.get("y").and_then(|v| v.as_i64());
                        Some(IssuedOrder {
                            kind,
                            subject: o.get("subject").and_then(|v| v.as_i64()),
                            verb: o.get("verb").and_then(|v| v.as_str()).map(str::to_string),
                            pos: match (x, y) {
                                (Some(x), Some(y)) => Some((x as i32, y as i32)),
                                _ => None,
                            },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn label(&self) -> String {
        match &self.verb {
            Some(verb) => format!("{}:{}", self.kind, verb),
            None => self.kind.clone(),
        }
    }
}

/// What the next frame said about one order.
#[derive(Clone, Debug, PartialEq)]
enum Verdict {
    Verified,
    Failed(String),
    Unverifiable,
}

#[derive(Clone, Debug, PartialEq)]
struct OrderCheck {
    order: IssuedOrder,
    verdict: Verdict,
}

/// Orders sent on one frame, held until a later turn's frame answers them.
struct PendingOrders {
    turn: u32,
    /// The mid-turn frame that supplied the board on which these orders were
    /// decided. Frame 0 is the opening board; later frames are re-plans after
    /// the host has drained some of the unit queue.
    frame: u32,
    orders: Vec<IssuedOrder>,
    before: civvis::mirror::StateSnapshot,
}

/// Row kinds that carry verdicts back to the mod. They are not orders: the mod
/// emits each one as a ledger event and keeps them out of `orders_seen`.
const VERDICT_ROW_KINDS: &[&str] = &["order_verified", "order_failed", "turn_verified"];

/// Order kinds the next frame cannot answer, each with the reason. Every kind
/// `translate` can emit is either checked in `verify_order` or listed here;
/// `every_issued_order_kind_is_checked_or_declared_unverifiable` enforces it.
const UNVERIFIABLE_ORDER_KINDS: &[(&str, &str)] = &[
    // A deferred lease the mod holds until the host queue finishes; the later
    // `build` event is its actuation proof, and the mod already keeps it out of
    // the applied count as `orders_deferred`.
    (
        "produce_next",
        "deferred lease; the `build` event is its proof",
    ),
    // The frame exports no diplomatic-mission table (delegations, embassies),
    // and the 25-gold fee is indistinguishable from ordinary spending.
    ("delegation", "no mission table in the frame"),
    // Aid is a contribution to an emergency score the host reveals late and
    // rounds; the frame has no per-turn contribution field to compare.
    ("aid_gift", "no contribution field in the frame"),
    // Levied units keep the city-state's ids and the frame does not mark them
    // as levied, so a levy cannot be told from the minor's own army.
    ("levy", "levied units are not marked in the frame"),
    // A question, not an act: the mod answers it at issue time with a
    // `preview` event (the host's `SimulateAttackInto` reading, filed on the
    // board as `Game::host_previews`) and changes nothing the next frame
    // could show.
    (
        "preview",
        "answered by the host's `preview` event at issue time; nothing changes on the frame",
    ),
];

/// Unit verbs the next frame cannot answer, each with the reason. `SPY_*`
/// covers every espionage mission: they run for many turns and the frame
/// carries no mission state.
const UNVERIFIABLE_UNIT_VERBS: &[(&str, &str)] = &[
    (
        "CONTRIBUTE_PROJECT",
        "no per-command project-production attribution; builder disappearance alone is not proof of contribution",
    ),
    (
        "SKIP_TURN",
        "a skipped turn changes nothing the frame exports",
    ),
    ("SLEEP", "sleep is not exported"),
    ("ALERT", "alert is not exported"),
    ("HEAL", "healing is indistinguishable from resting"),
    (
        "AUTOMATE_EXPLORE",
        "the host owns the unit's route from here on",
    ),
    (
        "SPY_*",
        "espionage missions run for many turns and the frame carries none",
    ),
    // A fighter's patrol is the shipped `UNITOPERATION_DEPLOY`: the aircraft
    // flies to the plot and intercepts from there, and the frame exports no
    // patrol state — neither the plot it watches nor whether the host has
    // recalled it to base by the next turn's opening frame.
    (
        "PATROL",
        "the frame carries no patrol state; where a deployed fighter stands next frame is the host's",
    ),
    // A Rock Band's concert is the shipped `UNITOPERATION_TOURISM_BOMB`
    // (`DLC/Expansion2/Data/Expansion2_UnitOperations.xml:11`). The frame
    // exports no band level and no per-unit tourism, so the only consequence
    // it can show is the player's whole-turn tourism — which the passive rate
    // moves every turn anyway. Same shape as HEAL: real, and not separable
    // from doing nothing. Declared rather than checked, so that a verb which
    // genuinely cannot be verified says so instead of vanishing through the
    // fallthrough.
    (
        "TOURISM_BOMB",
        "the frame carries no band level and a whole-turn tourism delta cannot separate a concert from the passive rate",
    ),
];

fn unverifiable_kind(kind: &str) -> bool {
    UNVERIFIABLE_ORDER_KINDS.iter().any(|(k, _)| *k == kind)
}

fn unverifiable_unit_verb(op: &str) -> bool {
    op.starts_with("SPY_") || UNVERIFIABLE_UNIT_VERBS.iter().any(|(v, _)| *v == op)
}

/// Ledger event kinds the checks read as evidence between two frames.
const EVIDENCE_KINDS: &[&str] = &[
    "artifact_extraction_pending",
    "condemn_removed",
    "combat_policy_applied",
    "strike_survival_refused",
    "combat",
    "deal_closed",
    "deal_declined",
    "deal_session",
    "peace_response",
    "improved",
    // The host safety pass may replace the planner's Settler destination. Its
    // `sent` plot is the postcondition the ordinary distance check cannot see.
    "settler_capture_escape",
    // ⭐ THE HOST ALREADY SAID WHY THE STEP DID NOT HAPPEN. The mod has
    // emitted `move_refused` — with the destination, whether it is water,
    // whether it is impassable and who owns it — since the order channel
    // existed, and its own comment says the point is that "the Rust side
    // can feed a NAMED refusal back so CIVVIS stops re-deriving the same
    // impossible step". Nothing in Rust has ever read it: a failed
    // `MOVE_TO` was diagnosed by comparing two frames' positions, which
    // cannot tell a refusal from a unit that simply had no movement left.
    "move_refused",
    // The queue's same-pass answer to a leg the host accepted and never
    // walked (`CivvisBoard.moveNoop`): the host's own reason, and the
    // neighbour step sent in its place. A `did_not_move` with one of these
    // on the ledger is a named refusal, not a position diff.
    "move_noop",
    "move_fallback",
    // A policy change can be accepted by the host's queue but deferred when
    // another policy transaction is still in flight on the same turn. This
    // is the host's attribution for an otherwise indistinguishable missing
    // deck postcondition.
    "policy_deck_deferred",
    // ⭐ THE SAME FOR A STRIKE. `range_attack_refused` is the host declining a
    // shot the simulator previewed (line of sight, range, a spent attack — its
    // `why` is the host's own reason), and `war_refused` is `refuseWarStarter`
    // holding back an order the engine would answer with a war the agent never
    // declared. Both were emitted for months and read by nothing in Rust: a
    // refused ATTACK/RANGE_ATTACK was failed as `target_unharmed`, which
    // reads the same as a shot that landed for zero. The mirror also reads
    // both into `Game::blocked_strikes` so the next frame of the same turn
    // does not propose the identical pair again.
    "range_attack_refused",
    "war_refused",
];

/// Ledger evidence and state frames stamped with one of `turns`.
///
/// A rival answers a deal during its own part of the game turn, so the answer
/// carries our turn; a callback that lands at the start of the next turn
/// carries that one. Both are accepted, restricted by subject in the checks.
///
/// ⚠ The relay re-encodes every line with `": "` spacing, so the cheap
/// pre-filter looks for the quoted kind alone (as `state_from_events` does)
/// and the parsed `kind` decides.
fn ledger_evidence_and_states(
    path: &Path,
    turns: &[u32],
) -> (Vec<serde_json::Value>, Vec<civvis::mirror::StateSnapshot>) {
    let Ok(raw) = civvis::mirror::read_events(path) else {
        return (Vec::new(), Vec::new());
    };
    let mut evidence = Vec::new();
    let mut states = Vec::new();
    let mut seat: Option<civvis::mirror::Seat> = None;
    // ★★★★ ONLY THE LINES THAT NAME A KIND WE KEEP, AND ONLY THEIR TURNS.
    // This ran on the first frame of every turn and built a full
    // `serde_json::Value` of EVERY state record of the run (hundreds of
    // 100-300 KB records by turn 200) to keep the one or two turns asked for:
    // the largest single cost of a late decider frame (2,080 of ~7,700 samples
    // on a growing-log replay of civvis-20261003T164758Z t230-245). The lines
    // are the same ones the old `contains` test kept, in file order; a record
    // that spells only other integer turns cannot pass the `as_u64` check below.
    let mut lines = civvis::mirror::line_ranges_containing(&raw, "\"state\"");
    // State events omit identity; retain the preceding seat even though its
    // metadata event has no requested turn. Do not parse unrelated state rows.
    lines.extend(civvis::mirror::line_ranges_containing(&raw, "\"seat\""));
    for kind in EVIDENCE_KINDS {
        lines.extend(civvis::mirror::line_ranges_containing(
            &raw,
            &format!("\"{kind}\""),
        ));
    }
    lines.sort_unstable();
    lines.dedup();
    for (start, end) in lines {
        let line = &raw[start..end];
        if !civvis::mirror::turn_may_be_any(line, turns) && !line.contains("\"seat\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|kind| kind.as_str()) == Some("seat") {
            seat = serde_json::from_value(event).ok();
            continue;
        }
        let turn = event.get("turn").and_then(|turn| turn.as_u64());
        if !turn.is_some_and(|turn| turns.iter().any(|want| u64::from(*want) == turn)) {
            continue;
        }
        let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind == "state" {
            if let Ok(mut state) = serde_json::from_value::<civvis::mirror::StateSnapshot>(event) {
                if let Some(seat) = &seat {
                    state.seat = seat.clone();
                }
                states.push(state);
            }
        } else if EVIDENCE_KINDS.contains(&kind) {
            evidence.push(event);
        }
    }
    (evidence, states)
}

/// Unit ids that were explicitly fortified on a later frame of this turn.
fn later_fortified_units(
    states: &[civvis::mirror::StateSnapshot],
    turn: u32,
    frame: u32,
) -> std::collections::BTreeSet<i64> {
    states
        .iter()
        .filter(|state| state.turn == turn && state.frame > frame)
        .flat_map(|state| state.units.iter())
        .filter(|unit| unit.fortified || unit.fortify_turns > 0)
        .map(|unit| unit.id)
        .collect()
}

/// Subjects the decider gave a MOVE_TO in a LATER frame of the same turn.
/// This identifies both a FORTIFY that the decider later cleared and a MOVE_TO
/// whose final position is ambiguous because the decider replanned it again.
///
/// ⚠⚠⚠ A FORTIFY THE DECIDER ITSELF OVERRODE IS NOT A BRIDGE FAILURE.
/// `not_fortified` is the second-largest failure reason on the ledger — 3124 of
/// 5193 FORTIFY orders over the runs of 2026-08-30, 60% — and **95% of them had
/// a MOVE_TO for the same unit in the same turn**. Splitting by order:
///
///     42%  FORTIFY then MOVE   (the later move clears the fortification)
///     19%  moves both before and after the fortify
///     39%  MOVE then FORTIFY   (should have stuck; a real question)
///
/// So roughly three fifths of the second-biggest "failure" is CIVVIS asking a
/// unit to dig in and then walking it away. The verdict is mechanically right —
/// the unit did not end up fortified — but reading it as `not_fortified` sends
/// anyone auditing bridge health after an actuation bug that is not there.
///
/// Named separately, it stays out of the applied count (it genuinely did not
/// apply) while saying whose decision it was.
///
/// ## 🔴 2026-09-11: THIS SPLIT WORKED, AND WHAT IT LEFT BEHIND IS REAL
///
/// Re-measured over the 42 recorded live runs of 2026-09-10/11. The
/// classification above now carries its own weight: `superseded_by_move` takes
/// **2,050** verdicts, and of the **5,442** that remain `not_fortified`:
///
///     95.0%  MOVE then FORTIFY   (5,169 — the "real question" above)
///      5.0%  no move that turn     (273)
///      0.0%  FORTIFY then MOVE   — the split now catches every one
///
/// The mix moved from 39% to 95% because the decider's own overrides were named
/// and removed. **What is left is the case the note flagged and nobody
/// answered**, and it is now essentially all of it.
///
/// ⚠ And it is not a checking-window artefact for most of them. Following each
/// failed subject into the next three turns' state frames (n = 1,770 with a
/// later frame):
///
///     72.0%  still not fortified
///     28.0%  fortified on a later turn
///
/// So roughly seven in ten genuinely never dig in, and the remaining three in
/// ten are a verdict read too early — widening `checked_on` for FORTIFY would
/// reclaim those without changing any behaviour.
///
/// ⚠⚠ The obvious mechanism — the unit spent its movement and the host refuses
/// FORTIFY at zero moves — is NOT established, and the tempting reading of the
/// data is wrong. The `moves` field in a turn's first state frame is the
/// PREVIOUS turn's value (see the frame-0 caution on the tactics ledger), so
/// the `moves = 3` those records show is the allowance before the move, not
/// after it. Answering this needs the host's own refusal reason on the FORTIFY
/// call, which the event does not carry today.
///
/// ⭐ For scale before anyone prices the work: **82.5% of all FORTIFY orders
/// fail** (7,711 against 1,637 verified), and FORTIFY is 36% of every failed
/// order on the bridge, second only to MOVE_TO at 51%.
///
/// ## ✅ ANSWERED, AND ALMOST NONE OF IT IS THE BRIDGE
///
/// `operation_refused` (the mod's `firstOperation`, added for this) shipped and
/// ran. In the first run carrying it, **2 of 181** `not_fortified` verdicts had
/// a host refusal recorded — **1%**. So `CanStartOperation` says yes and the
/// order is issued; the mod declining is not the cause. (The 67 refusals it did
/// record are civilians: 41 Traders, 9 Builders, 6 Settlers, asked to skip or
/// dig in and legitimately unable to.)
///
/// Following every `not_fortified` subject into the NEXT turn (n = 2,507):
///
///     64.6%  the decider MOVED IT AGAIN next turn
///     29.2%  fortified on a later frame after all
///      6.2%  neither
///
/// ⭐⭐ So this note's own opening sentence holds one turn further out than it
/// was written: **a FORTIFY the decider itself overrode is not a bridge
/// failure**, and `superseded_by_move` catches only the same-turn override.
/// Two thirds of what is left is the identical thing on the following turn.
/// Roughly 1% is a host refusal, and about three in ten are a verdict read
/// before the state settled. The genuinely unexplained residue is ~6%.
///
/// ### The two pieces of work this names, in order of size
///
/// 1. **Extend the supersession to the next turn.** It needs turn+1 frames,
///    which are not in hand at verification time the way same-turn frames are,
///    so it is a real change rather than a wider filter. Name it separately —
///    `superseded_next_turn` — so the two stay countable apart.
/// 2. **Widen `checked_on` for FORTIFY.** Reclaims the 29.2% that do fortify,
///    with no behaviour change at all.
///
/// ### ⚠ And the strategic question underneath, which is the bigger one
///
/// Two thirds of the time CIVVIS asks a unit to dig in and then walks it away
/// the next turn, so **the unit never collects the fortification it was told to
/// take**. That is a decider question, not a bridge one, and it is worth asking
/// against the standing puzzle that 234 of 304 unit deaths happen with no
/// visible threat. Unproven as a link — but the accounting above means the
/// defence bonus is almost never actually held, which is the first thing to
/// check.
fn later_moved_units(
    frames: &[PendingOrders],
    turn: u32,
    frame: u32,
) -> std::collections::BTreeSet<i64> {
    frames
        .iter()
        .filter(|pending| pending.turn == turn && pending.frame > frame)
        .flat_map(|pending| pending.orders.iter())
        .filter(|order| order.verb.as_deref() == Some("MOVE_TO"))
        .filter_map(|order| order.subject)
        .collect()
}

/// The host deferred this exact policy deck on the requested turn because a
/// same-turn policy transaction was still in flight. The event has no frame
/// number, so callers use later-policy supersession first when an earlier
/// frame has a matching replacement that did land.
fn policy_deck_deferred_reason(
    evidence: &[serde_json::Value],
    turn: u32,
    order: &IssuedOrder,
) -> Option<String> {
    if order.kind != "policy_deck" {
        return None;
    }
    let requested: Vec<&str> = order
        .verb
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|policy| !policy.is_empty())
        .collect();
    evidence.iter().find_map(|event| {
        if event.get("kind").and_then(|kind| kind.as_str()) != Some("policy_deck_deferred")
            || event.get("turn").and_then(|value| value.as_u64()) != Some(u64::from(turn))
        {
            return None;
        }
        let desired = event.get("desired").and_then(|value| value.as_array())?;
        let exact_deck = desired.len() == requested.len()
            && requested
                .iter()
                .zip(desired)
                .all(|(want, got)| got.as_str() == Some(*want));
        exact_deck.then(|| {
            format!(
                "deferred_{}",
                event
                    .get("why")
                    .and_then(|why| why.as_str())
                    .unwrap_or("same_turn")
            )
        })
    })
}

/// Whether a policy-deck order's requested cards are present in the state.
///
/// The planner emits the complete active deck, but the host may expose an
/// additional slot on the answering frame after a civic completes. The
/// postcondition therefore keeps the existing subset semantics: every card
/// requested by the order must be present, while unrelated extra cards do not
/// make an otherwise successful replacement fail.
fn policy_deck_cards_present(order: &IssuedOrder, state: &civvis::mirror::StateSnapshot) -> bool {
    if order.kind != "policy_deck" {
        return false;
    }
    let requested: Vec<&str> = order
        .verb
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|policy| !policy.is_empty())
        .collect();
    !requested.is_empty()
        && requested
            .iter()
            .all(|policy| state.policies.iter().any(|have| have == policy))
}

/// A later same-turn policy replacement landed in the final answering state.
///
/// In that case an earlier deck can be absent for the healthy reason that the
/// decider replaced it before the turn ended. Keep that case distinct from a
/// host refusing the only policy transaction, just as later MOVE_TO orders are
/// distinct from a failed FORTIFY.
fn later_policy_deck_matches(
    frames: &[PendingOrders],
    turn: u32,
    frame: u32,
    after: &civvis::mirror::StateSnapshot,
) -> bool {
    frames
        .iter()
        .filter(|pending| pending.turn == turn && pending.frame > frame)
        .flat_map(|pending| pending.orders.iter())
        .any(|order| policy_deck_cards_present(order, after))
}

fn offset_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    civvis::hex::distance(
        civvis::hex::offset_to_axial(a.0, a.1),
        civvis::hex::offset_to_axial(b.0, b.1),
    )
}

fn own_unit(state: &civvis::mirror::StateSnapshot, id: i64) -> Option<&civvis::mirror::StateUnit> {
    state.units.iter().find(|u| u.id == id)
}

fn own_city(state: &civvis::mirror::StateSnapshot, id: i64) -> Option<&civvis::mirror::StateCity> {
    state.cities.iter().find(|c| c.id == id)
}

fn own_city_at(
    state: &civvis::mirror::StateSnapshot,
    pos: (i32, i32),
) -> Option<&civvis::mirror::StateCity> {
    state.cities.iter().find(|c| (c.x, c.y) == pos)
}

fn foreign_units(
    state: &civvis::mirror::StateSnapshot,
) -> impl Iterator<Item = &civvis::mirror::StateUnit> {
    state
        .rivals
        .iter()
        .flat_map(|r| r.units.iter())
        .chain(state.minors.iter().flat_map(|m| m.units.iter()))
        .chain(state.hostiles.iter())
}

fn foreign_city_at(
    state: &civvis::mirror::StateSnapshot,
    pos: (i32, i32),
) -> Option<&civvis::mirror::StateCity> {
    state
        .rivals
        .iter()
        .flat_map(|r| r.cities.iter())
        .chain(state.minors.iter().flat_map(|m| m.cities.iter()))
        .find(|c| (c.x, c.y) == pos)
}

/// Who holds a foreign city on `pos`: the Firaxis player id of the rival or
/// city-state whose `cities[]` lists it.
fn foreign_city_owner_at(state: &civvis::mirror::StateSnapshot, pos: (i32, i32)) -> Option<i64> {
    let holds = |cities: &[civvis::mirror::StateCity]| cities.iter().any(|c| (c.x, c.y) == pos);
    state
        .rivals
        .iter()
        .find(|r| holds(&r.cities))
        .map(|r| r.player as i64)
        .or_else(|| {
            state
                .minors
                .iter()
                .find(|m| holds(&m.cities))
                .map(|m| m.player as i64)
        })
}

/// The captured city's disposition, judged by who holds the city on the next
/// frame. KEEP: still ours and the host no longer waits on a decision for it
/// (`captured_from` clear), else `not_kept`. RAZE: Civilization VI burns a
/// razed city down one citizen a turn and it stays ours while it burns, so
/// the next turn shows it gone or smaller; on a same-turn replan frame the
/// only trace is the decision consumed — the flag clear before any citizen
/// has left — which is accepted there and only there; else `not_razed`.
/// LIBERATE: not ours, and the founder the decision frame named
/// (`original_owner`) holds the plot, else `liberated_to_another` /
/// `not_liberated`.
fn verify_city_disposition(
    order: &IssuedOrder,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
) -> Verdict {
    let failed = |why: &str| Verdict::Failed(why.to_string());
    let Some(id) = order.subject else {
        return failed("no_subject");
    };
    let verb = order.verb.as_deref().unwrap_or("");
    let was = own_city(before, id);
    let plot = order.pos.or_else(|| was.map(|c| (c.x, c.y)));
    let now = own_city(after, id).or_else(|| plot.and_then(|p| own_city_at(after, p)));
    match verb {
        "KEEP" => match now {
            Some(city) if city.captured_from.is_none() => Verdict::Verified,
            Some(_) => failed("not_kept"),
            None => failed("city_gone"),
        },
        "RAZE" => match now {
            None => Verdict::Verified,
            Some(city) if was.is_some_and(|w| city.pop < w.pop) => Verdict::Verified,
            Some(city)
                if after.turn == before.turn
                    && city.captured_from.is_none()
                    && was.is_some_and(|w| w.captured_from.is_some()) =>
            {
                Verdict::Verified
            }
            Some(_) => failed("not_razed"),
        },
        "LIBERATE" => {
            let Some(plot) = plot else {
                return failed("no_plot");
            };
            if now.is_some() {
                return failed("not_liberated");
            }
            let founder = was.and_then(|w| w.original_owner);
            match foreign_city_owner_at(after, plot) {
                Some(owner) if founder.is_none_or(|f| f == owner) => Verdict::Verified,
                Some(_) => failed("liberated_to_another"),
                None => failed("not_liberated"),
            }
        }
        _ => Verdict::Failed(format!("unknown_city_verb_{verb}")),
    }
}

fn at_war_with(state: &civvis::mirror::StateSnapshot, player: i64) -> Option<bool> {
    state
        .rivals
        .iter()
        .find(|r| r.player as i64 == player)
        .map(|r| r.at_war)
        .or_else(|| {
            state
                .minors
                .iter()
                .find(|m| m.player as i64 == player)
                .map(|m| m.at_war)
        })
}

/// The turn we denounced `player` as the host records it (`-1` for never):
/// `Some(None)` when the rival is in the frame but an older mod did not
/// export the field, `None` when the rival is not in the frame at all.
fn our_denounce_turn_of(state: &civvis::mirror::StateSnapshot, player: i64) -> Option<Option<i64>> {
    state
        .rivals
        .iter()
        .find(|r| r.player as i64 == player)
        .map(|r| r.our_denounce_turn)
}

/// The turn the host records our declared friendship with `player`
/// (`GetDeclaredFriendshipTurn`, `<= 0` for none), shaped like
/// [`our_denounce_turn_of`].
fn friendship_turn_of(state: &civvis::mirror::StateSnapshot, player: i64) -> Option<Option<i64>> {
    state
        .rivals
        .iter()
        .find(|r| r.player as i64 == player)
        .map(|r| r.friendship_turn)
}

/// Whether whatever stood on `pos` before the order is damaged, gone, or ours.
/// A launch leaves two marks the next frame can show: one device fewer in the
/// stockpile, and a hurt target. Either proves it. A device finished the same
/// turn can hide the first and an empty target hides the second, so a launch
/// fails only when neither shows.
fn launch_landed(
    weapon: &str,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    target: Option<(i32, i32)>,
) -> bool {
    let held = |frame: &civvis::mirror::StateSnapshot| {
        let stats = &frame.public_stats;
        let count = if weapon == "WMD_THERMONUCLEAR_DEVICE" {
            stats.thermonuclear_devices
        } else {
            stats.nuclear_devices
        };
        count.filter(|held| *held >= 0)
    };
    let spent = matches!((held(before), held(after)), (Some(was), Some(now)) if now < was);
    spent || target.is_some_and(|pos| target_harmed(before, after, pos))
}

fn target_harmed(
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    pos: (i32, i32),
) -> bool {
    let stood: Vec<&civvis::mirror::StateUnit> = foreign_units(before)
        .filter(|u| (u.x, u.y) == pos)
        .collect();
    let unit_harmed = stood.iter().any(|was| {
        match foreign_units(after).find(|now| now.id == was.id && now.player == was.player) {
            None => true,
            Some(now) => now.hp < was.hp,
        }
    });
    if unit_harmed {
        return true;
    }
    match foreign_city_at(before, pos) {
        Some(was) => match foreign_city_at(after, pos) {
            None => own_city_at(after, pos).is_some(),
            Some(now) => now.damage > was.damage || now.wall_damage > was.wall_damage,
        },
        None => false,
    }
}

/// The host's own reason a unit did not step, if it gave one for this unit
/// and turn. Prefers the named refusal over the position diff: a host that
/// says `dest_impassable` is a fact, and "the unit is where it was" is an
/// inference that reads the same for a refusal, an exhausted allowance and
/// an order the mod never issued.
fn move_refusal_reason(evidence: &[serde_json::Value], turn: u32, unit: i64) -> Option<String> {
    let of_kind = |kind: &str| {
        evidence.iter().find(|event| {
            event.get("kind").and_then(|k| k.as_str()) == Some(kind)
                && event.get("turn").and_then(|t| t.as_u64()) == Some(u64::from(turn))
                && event.get("unit").and_then(|u| u.as_i64()) == Some(unit)
        })
    };
    // A leg the host accepted and never walked is the more specific answer:
    // the mod probed the host for the reason after the watch ran out
    // (`cannot_start`, `no_path`, `zoc`, `occupied`, …), where `move_refused`
    // only says the operation would not start.
    if let Some(noop) = of_kind("move_noop") {
        let why = noop
            .get("why")
            .and_then(|w| w.as_str())
            .filter(|w| !w.is_empty())
            .unwrap_or("unknown");
        return Some(format!("host_noop_{why}"));
    }
    let event = of_kind("move_refused")?;
    let named = |key: &str| event.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    Some(if named("dest_impassable") {
        "host_refused_impassable".to_string()
    } else if named("dest_water") {
        "host_refused_water".to_string()
    } else {
        "host_refused_move".to_string()
    })
}

/// The host's own reason a strike did not happen, when it gave one this turn
/// for exactly this attacker and target.
///
/// `war_refused` (from `CivvisLedger.refuseWarStarter`) verifies as
/// `would_declare_war`: the order was never sent, because the engine would
/// have answered it with a war the agent never declared. `range_attack_refused`
/// carries the host's `why` — `refusalReason`'s localized failure reasons,
/// `"Target is out of range | No line of sight [p4r]"` — and crosses as
/// `host_refused_<why>` with that text reduced to a token; a refusal without
/// words is `host_refused_strike`.
fn strike_refusal_reason(
    evidence: &[serde_json::Value],
    turn: u32,
    unit: i64,
    target: Option<(i32, i32)>,
) -> Option<String> {
    let aimed_here = |event: &serde_json::Value| {
        event.get("turn").and_then(|t| t.as_u64()) == Some(u64::from(turn))
            && event.get("unit").and_then(|u| u.as_i64()) == Some(unit)
            && target.is_none_or(|(x, y)| {
                event.get("x").and_then(|v| v.as_i64()) == Some(i64::from(x))
                    && event.get("y").and_then(|v| v.as_i64()) == Some(i64::from(y))
            })
    };
    let of_kind = |event: &serde_json::Value, kind: &str| {
        event.get("kind").and_then(|k| k.as_str()) == Some(kind)
    };
    if evidence
        .iter()
        .any(|event| of_kind(event, "war_refused") && aimed_here(event))
    {
        return Some("would_declare_war".to_string());
    }
    if evidence
        .iter()
        .any(|event| of_kind(event, "strike_survival_refused") && aimed_here(event))
    {
        return Some("lethal_host_preview".to_string());
    }
    let refused = evidence
        .iter()
        .find(|event| of_kind(event, "range_attack_refused") && aimed_here(event))?;
    let why = refused
        .get("why")
        .and_then(|w| w.as_str())
        .unwrap_or("")
        // `refusalReason` suffixes the call form it got the words from.
        .split(" [")
        .next()
        .unwrap_or("");
    let token: String = why
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    Some(if token.is_empty() || token.starts_with("can_start") {
        "host_refused_strike".to_string()
    } else {
        format!("host_refused_{token}")
    })
}

/// Whether the host deliberately rewrote this Settler leg to an escape plot.
/// The safety pass can replace the planner's destination after the order has
/// already entered the ledger; the event is the bridge's proof of the actual
/// destination, so a farther final position is not a failed MOVE_TO.
fn settler_capture_escape_reached(
    evidence: &[serde_json::Value],
    turn: u32,
    unit: i64,
    want: (i32, i32),
    reached: (i32, i32),
) -> bool {
    let point = |event: &serde_json::Value, key: &str| {
        let values = event.get(key)?.as_array()?;
        Some((
            values.first()?.as_i64()? as i32,
            values.get(1)?.as_i64()? as i32,
        ))
    };
    evidence.iter().any(|event| {
        event.get("kind").and_then(|kind| kind.as_str()) == Some("settler_capture_escape")
            && event.get("turn").and_then(|value| value.as_u64()) == Some(u64::from(turn))
            && event.get("settler").and_then(|value| value.as_i64()) == Some(unit)
            && point(event, "want") == Some(want)
            && point(event, "sent") == Some(reached)
    })
}

fn combat_evidence(
    evidence: &[serde_json::Value],
    turn: u32,
    attacker: i64,
    target: Option<(i32, i32)>,
) -> bool {
    evidence.iter().any(|event| {
        event.get("kind").and_then(|k| k.as_str()) == Some("combat")
            && event.get("turn").and_then(|t| t.as_u64()) == Some(u64::from(turn))
            && event
                .get("attacker")
                .and_then(|a| a.get("id"))
                .and_then(|id| id.as_i64())
                == Some(attacker)
            && target.is_none_or(|(x, y)| {
                event
                    .get("defender")
                    .and_then(|defender| defender.get("x"))
                    .and_then(|value| value.as_i64())
                    == Some(i64::from(x))
                    && event
                        .get("defender")
                        .and_then(|defender| defender.get("y"))
                        .and_then(|value| value.as_i64())
                        == Some(i64::from(y))
            })
    })
}

/// Firaxis can finish an aircraft operation with an anonymous plot defender
/// (-1/-1), without any defender position, health or infrastructure result.
/// That own-attacker callback cannot locate this order's effect. Do not turn
/// missing attribution into either a verified strike or a refusal cooldown.
fn anonymous_air_combat(evidence: &[serde_json::Value], turn: u32, attacker: i64) -> bool {
    evidence.iter().any(|event| {
        event.get("kind").and_then(|v| v.as_str()) == Some("combat")
            && event.get("turn").and_then(|v| v.as_u64()) == Some(u64::from(turn))
            && event.get("ours").and_then(|v| v.as_bool()) == Some(true)
            && event
                .get("attacker")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_i64())
                == Some(attacker)
            && event.get("defender").is_some_and(|defender| {
                ["player", "id"].iter().any(|key| {
                    defender
                        .get(key)
                        .and_then(|v| v.as_i64())
                        .is_some_and(|id| id < 0)
                })
            })
    })
}

fn deal_answered(evidence: &[serde_json::Value], subject: Option<i64>) -> bool {
    evidence.iter().any(|event| {
        let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        let party = event
            .get("from")
            .or_else(|| event.get("target"))
            .and_then(|p| p.as_i64());
        let same_party = subject.is_none() || party.is_none() || party == subject;
        same_party
            && match kind {
                "deal_closed" | "deal_declined" => true,
                "deal_session" => event.get("phase").and_then(|p| p.as_str()) == Some("answered"),
                _ => false,
            }
    })
}

fn peace_response(evidence: &[serde_json::Value], subject: Option<i64>) -> Option<bool> {
    evidence.iter().find_map(|event| {
        if event.get("kind").and_then(|k| k.as_str()) != Some("peace_response") {
            return None;
        }
        let party = event.get("target").and_then(|p| p.as_i64());
        if subject.is_some() && party.is_some() && party != subject {
            return None;
        }
        Some(
            event
                .get("accepted")
                .and_then(|a| a.as_bool())
                .unwrap_or(false),
        )
    })
}

/// Whether a city holds `item` as a finished building, district, or wonder.
fn city_holds(city: &civvis::mirror::StateCity, item: &str) -> bool {
    // A pillaged predecessor is not a completed repair. A queued repair is
    // verified separately by the production arm's current-item check.
    (city.buildings.iter().any(|b| b == item)
        && !city
            .pillaged_buildings
            .as_ref()
            .is_some_and(|items| items.iter().any(|b| b == item)))
        || city.districts.iter().any(|d| d.kind == item && !d.pillaged)
        || city.wonders.iter().any(|w| w.kind == item)
}

fn building_repair_status_unknown(
    before: &civvis::mirror::StateSnapshot,
    city: &civvis::mirror::StateCity,
    item: &str,
) -> bool {
    city.pillaged_buildings.is_none()
        && own_city(before, city.id)
            .and_then(|prior| prior.pillaged_buildings.as_ref())
            .is_some_and(|items| items.iter().any(|b| b == item))
}

/// A unit of `kind` within one tile of the city that was not there before.
fn unit_appeared_at_city(
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    city: &civvis::mirror::StateCity,
    kind: &str,
) -> bool {
    after.units.iter().any(|u| {
        u.kind == kind
            && offset_distance((u.x, u.y), (city.x, city.y)) <= 1
            && own_unit(before, u.id).is_none()
    })
}

/// A newly visible unit can prove either a completed production order or a
/// purchase made in the same turn. When both orders ask the same city for the
/// same unit, the next frame cannot attribute the appearance to production.
/// Keep that ambiguity from turning a refused queue replacement into a false
/// `order_verified` row.
fn production_unit_appearance_conflicts_with_purchase(
    order: &IssuedOrder,
    same_turn_orders: &[IssuedOrder],
) -> bool {
    order.kind == "produce"
        && order
            .verb
            .as_deref()
            .is_some_and(|verb| verb.starts_with("UNIT_"))
        && same_turn_orders.iter().any(|other| {
            matches!(other.kind.as_str(), "purchase" | "purchase_faith")
                && other.subject == order.subject
                && other.verb == order.verb
        })
}

/// What later frames of the same turn say about a unit, for verdicts that a
/// single frame cannot settle. Bundled rather than passed as loose flags so
/// `verify_unit_order` stays inside clippy's argument limit.
#[derive(Clone, Copy, Default)]
struct LaterFrames {
    /// The unit was fortified on a later frame, so a FORTIFY did land.
    fortified: bool,
    /// The decider moved it on a later frame, overriding its own FORTIFY.
    moved: bool,
    /// The actor actually occupied this MOVE_TO's endpoint on a later frame.
    move_reached: bool,
}

fn verify_unit_order(
    order: &IssuedOrder,
    turn: u32,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    later: LaterFrames,
) -> Verdict {
    let Some(id) = order.subject else {
        return Verdict::Failed("no_subject".to_string());
    };
    let verb = order.verb.as_deref().unwrap_or("");
    let (op, arg) = verb.split_once(':').unwrap_or((verb, ""));
    if unverifiable_unit_verb(op) {
        return Verdict::Unverifiable;
    }
    let was = own_unit(before, id);
    let now = own_unit(after, id);
    let gone = || Verdict::Failed("unit_gone".to_string());
    match op {
        "MOVE_TO" => {
            let Some(want) = order.pos else {
                return Verdict::Failed("no_destination".to_string());
            };
            // A completed move is not a refusal merely because a subsequent
            // replan moved the actor away, or combat later removed it. Survival
            // and operation causality are separate from this postcondition.
            if was.is_some() && later.move_reached {
                return Verdict::Verified;
            }
            match (was, now) {
                (_, Some(now)) if (now.x, now.y) == want => Verdict::Verified,
                // A unit first seen on a combat frame has no baseline to move from.
                (None, Some(_)) => Verdict::Unverifiable,
                (Some(was), Some(now)) => {
                    let reached = (now.x, now.y);
                    let safety_rewrite = was.kind == "UNIT_SETTLER"
                        && settler_capture_escape_reached(evidence, turn, id, want, reached);
                    let start = offset_distance((was.x, was.y), want);
                    let end = offset_distance(reached, want);
                    if safety_rewrite || start == 0 || end < start {
                        Verdict::Verified
                    } else if end == start && later.moved {
                        // A later same-turn MOVE_TO superseded this request;
                        // the final position alone cannot distinguish that
                        // replan from a host no-op.
                        Verdict::Failed("superseded_by_move".to_string())
                    } else if end == start {
                        // The host's own reason when it gave one; see
                        // `move_refusal_reason`.
                        Verdict::Failed(
                            move_refusal_reason(evidence, turn, id)
                                .unwrap_or_else(|| "did_not_move".to_string()),
                        )
                    } else {
                        Verdict::Failed("moved_away".to_string())
                    }
                }
                (Some(_), None) => {
                    // A settler that walked onto its site and founded is consumed
                    // by the order it was walking for.
                    if own_city_at(after, want).is_some() && own_city_at(before, want).is_none() {
                        Verdict::Verified
                    } else {
                        gone()
                    }
                }
                (None, None) => Verdict::Unverifiable,
            }
        }
        // The capture is the unit standing where the civilian stood, or the
        // civilian gone from there (it is ours now, so `foreign_units` no
        // longer lists it).
        "CAPTURE" => {
            let Some(want) = order.pos else {
                return Verdict::Failed("no_destination".to_string());
            };
            match now {
                Some(now) if (now.x, now.y) == want => Verdict::Verified,
                _ if target_harmed(before, after, want) => Verdict::Verified,
                None => gone(),
                Some(_) => Verdict::Failed("not_captured".to_string()),
            }
        }
        // A swap is two units trading hexes, so either half proves it: this
        // unit standing on the partner's former tile, or the partner standing
        // on ours. Only a unit that was on the decision frame can be judged
        // (there is no "our tile" for a unit first seen afterwards).
        "SWAP" => {
            let Some(want) = order.pos else {
                return Verdict::Failed("no_destination".to_string());
            };
            let partner_took_our_tile = was.is_some_and(|was| {
                after.units.iter().any(|u| {
                    u.id != id
                        && (u.x, u.y) == (was.x, was.y)
                        && own_unit(before, u.id).is_some_and(|p| (p.x, p.y) == want)
                })
            });
            match now {
                Some(now) if (now.x, now.y) == want => Verdict::Verified,
                _ if partner_took_our_tile => Verdict::Verified,
                None => gone(),
                Some(_) if was.is_none() => Verdict::Unverifiable,
                Some(_) => Verdict::Failed("not_swapped".to_string()),
            }
        }
        "FOUND_CITY" => {
            let site = order.pos.or_else(|| was.map(|u| (u.x, u.y)));
            let founded_here = site.is_some_and(|s| {
                own_city_at(after, s).is_some() && own_city_at(before, s).is_none()
            });
            if founded_here || (now.is_none() && after.cities.len() > before.cities.len()) {
                Verdict::Verified
            } else {
                Verdict::Failed("no_city".to_string())
            }
        }
        // An ordinary strike must be attributed to this attacker and this
        // target. A foreign unit can harm the same plot between frames, which
        // is not proof that this request landed. AIR_ATTACK retains the target
        // harm fallback because aircraft can resolve through a host/UI path
        // that does not emit the same combat record; an exact host refusal
        // still wins over collateral damage.
        "ATTACK" | "RANGE_ATTACK" | "AIR_ATTACK" => {
            let exact_combat = combat_evidence(evidence, turn, id, order.pos);
            let refusal = strike_refusal_reason(evidence, turn, id, order.pos);
            let target_damage_observed = order.pos.is_some_and(|p| target_harmed(before, after, p));
            let air_target_harmed = verb == "AIR_ATTACK" && target_damage_observed;
            if exact_combat || (air_target_harmed && refusal.is_none()) {
                Verdict::Verified
            } else if verb == "AIR_ATTACK"
                && refusal.is_none()
                && anonymous_air_combat(evidence, turn, id)
            {
                // No target location or pillage-layer postcondition came back.
                // This callback proves neither this request's success nor its
                // failure, even when an older mod labeled (-1/-1) as killed.
                Verdict::Unverifiable
            } else {
                // The host's own reason when it gave one; see
                // `strike_refusal_reason`.
                Verdict::Failed(refusal.unwrap_or_else(|| {
                    if air_target_harmed {
                        "target_unharmed".to_string()
                    } else if target_damage_observed && (verb == "ATTACK" || verb == "RANGE_ATTACK")
                    {
                        "attacker_unproven".to_string()
                    } else {
                        "target_unharmed".to_string()
                    }
                }))
            }
        }
        // A rebase lands the aircraft on the new base plot at once (the
        // shipped `UNITOPERATION_REBASE` is a teleport that spends the turn),
        // so the aircraft standing there on the next frame is the whole
        // postcondition.
        "TELEPORT_TO_CITY" => {
            let Some(want) = order.pos else {
                return Verdict::Failed("no_destination".to_string());
            };
            match now {
                Some(now)
                    if now.kind == "UNIT_TRADER"
                        && (now.x, now.y) == want
                        && own_city_at(after, want).is_some() =>
                {
                    Verdict::Verified
                }
                None => gone(),
                Some(_) => Verdict::Failed("trade_origin_not_changed".to_string()),
            }
        }
        "REBASE" => {
            let Some(want) = order.pos else {
                return Verdict::Failed("no_destination".to_string());
            };
            match now {
                Some(now) if (now.x, now.y) == want => Verdict::Verified,
                None => gone(),
                Some(_) => Verdict::Failed("not_rebased".to_string()),
            }
        }
        "FORTIFY" => {
            // Fortify is a transient host state: a later same-turn queued move
            // can clear it before the next turn's opening frame, even though
            // the FORTIFY operation did land. Require the unit to have been
            // eligible on the decision frame so a newly appearing id or a
            // refused repeat on an already-fortified unit cannot be credited.
            let was_eligible = was.is_some_and(|unit| !unit.fortified && unit.fortify_turns <= 0);
            if later.fortified && was_eligible {
                Verdict::Verified
            } else {
                match now {
                    None => gone(),
                    Some(u) if u.fortified || u.fortify_turns > 0 => Verdict::Verified,
                    // See `later_moved_units`: the decider walked this unit away
                    // after asking it to dig in. Not the bridge's doing.
                    Some(_) if later.moved => Verdict::Failed("superseded_by_move".to_string()),
                    Some(_) => Verdict::Failed("not_fortified".to_string()),
                }
            }
        }
        "UPGRADE" => match (was, now) {
            // The host may hand the upgraded unit a new id: a different kind
            // standing where this one stood, unseen before, is the upgrade.
            (Some(was), None) => {
                let replaced = after.units.iter().any(|u| {
                    (u.x, u.y) == (was.x, was.y)
                        && u.kind != was.kind
                        && own_unit(before, u.id).is_none()
                });
                if replaced {
                    Verdict::Verified
                } else {
                    gone()
                }
            }
            (None, None) => gone(),
            (None, Some(_)) => Verdict::Unverifiable,
            (Some(was), Some(now)) if was.kind != now.kind => Verdict::Verified,
            _ => Verdict::Failed("same_unit_type".to_string()),
        },
        "DELETE" => {
            if now.is_none() {
                Verdict::Verified
            } else {
                Verdict::Failed("still_exists".to_string())
            }
        }
        // ⚠⚠ FOUND BY `every_issued_unit_verb_is_checked_or_declared_unverifiable`.
        // `Action::Improve` does not always translate to `IMPROVE:<type>`: a
        // National Park is `DESIGNATE_PARK` and both artifact digs are
        // `EXCAVATE`, and neither had an arm — both fell through to
        // `Unverifiable`, so a Naturalist's park and an Archaeologist's dig
        // were excluded from every actuation rate. They share this arm because
        // they share its evidence: the operation spends a charge, and a unit
        // spending its last one is consumed.
        //
        // The tile test differs by verb. `IMPROVE` names the improvement it
        // wants in `arg`; `DESIGNATE_PARK` names none, and what the plot must
        // show is the park itself. `EXCAVATE` leaves no improvement at all —
        // it lifts the artifact and clears the site — so it rests on the
        // charge, the unit, an `improved` event, or the native pending-artifact
        // witness identifying the extracting Archaeologist and its artifact.
        "IMPROVE" | "REPAIR" | "DESIGNATE_PARK" | "EXCAVATE" | "REMOVE_FEATURE"
        | "HARVEST_RESOURCE" | "PLANT_FOREST" => {
            let charges_spent = match (
                was.and_then(|u| u.build_charges),
                now.and_then(|u| u.build_charges),
            ) {
                (Some(before), Some(after)) => after < before,
                _ => false,
            };
            let tile_shows = was.is_some_and(|u| {
                tiles.plot((u.x, u.y)).is_some_and(|plot| {
                    (op == "IMPROVE" && !arg.is_empty() && plot.im.as_deref() == Some(arg))
                        || (op == "REPAIR" && !plot.p)
                        || (op == "DESIGNATE_PARK"
                            && plot.im.as_deref() == Some("IMPROVEMENT_NATIONAL_PARK"))
                })
            });
            let improved_event = was.is_some_and(|u| {
                evidence.iter().any(|event| {
                    event.get("kind").and_then(|k| k.as_str()) == Some("improved")
                        && event.get("turn").and_then(|t| t.as_u64()) == Some(u64::from(turn))
                        && event.get("x").and_then(|x| x.as_i64()) == Some(i64::from(u.x))
                        && event.get("y").and_then(|y| y.as_i64()) == Some(i64::from(u.y))
                })
            });
            // Native Archaeologists report zero Builder charges and leave no
            // improvement. The artifact-choice API identifies the extracting
            // unit and its actual artifact, without relying on unrelated work
            // trades or another Archaeologist's inventory change.
            let extraction_pending = op == "EXCAVATE"
                && was.is_some_and(|u| u.kind == "UNIT_ARCHAEOLOGIST")
                && before.seat.local_player >= 0
                && evidence.iter().any(|event| {
                    event["kind"] == "artifact_extraction_pending"
                        && event["player"].as_i64() == Some(i64::from(before.seat.local_player))
                        && event["unit"].as_i64() == Some(id)
                        && event["artifact"].as_i64().is_some_and(|index| index >= 0)
                        && event["turn"].as_u64() == Some(u64::from(turn))
                });
            if now.is_none() || charges_spent || tile_shows || improved_event || extraction_pending
            {
                Verdict::Verified
            } else if op == "REPAIR" {
                // A repair spends no charge and raises no `improved` event: the
                // tile's pillage bit is its only witness.
                Verdict::Failed("not_repaired".to_string())
            } else {
                Verdict::Failed("no_improvement".to_string())
            }
        }
        // The host pillages the plot the unit stands on when the order runs.
        // A raider walks first (`air_surge/raids.rs`: MOVE_TO, then PILLAGE),
        // so the plot it ends on is as much a witness as the one it started
        // on; reading only the start plot failed every raid, and three
        // failures withhold the unit's PILLAGE for ten turns.
        // The raider stays at sea, so the proof is the shore tile it named.
        "COASTAL_RAID" => {
            if order
                .pos
                .and_then(|pos| tiles.plot(pos))
                .is_some_and(|plot| plot.p)
            {
                Verdict::Verified
            } else {
                Verdict::Failed("not_pillaged".to_string())
            }
        }
        "WMD_NUCLEAR_DEVICE" | "WMD_THERMONUCLEAR_DEVICE" => {
            if launch_landed(op, before, after, order.pos) {
                Verdict::Verified
            } else {
                Verdict::Failed("device_not_spent".to_string())
            }
        }
        "PILLAGE" => {
            let pillaged_under =
                |u: &civvis::mirror::StateUnit| tiles.plot((u.x, u.y)).is_some_and(|plot| plot.p);
            let pillaged = was.is_some_and(pillaged_under) || now.is_some_and(pillaged_under);
            if pillaged {
                Verdict::Verified
            } else {
                Verdict::Failed("not_pillaged".to_string())
            }
        }
        // Corps and Armies are single units. The host's military tier, not
        // its escort member count, proves this command's postcondition.
        // Missing or invalid tiers cannot prove either success or failure.
        "FORM_CORPS" | "FORM_ARMY" => match now {
            None => gone(),
            Some(u) => {
                let required = if op == "FORM_CORPS" { 1 } else { 2 };
                match u.formation {
                    Some(tier @ 0..=2) if tier >= required => Verdict::Verified,
                    Some(0..=2) => Verdict::Failed("military_formation_tier_too_low".to_string()),
                    _ => Verdict::Unverifiable,
                }
            }
        },
        "ENTER_FORMATION" => match now {
            None => gone(),
            Some(u) if u.formation_count > 1 => Verdict::Verified,
            Some(_) => Verdict::Failed("not_in_formation".to_string()),
        },
        "EXIT_FORMATION" => match now {
            None => gone(),
            Some(u) if u.formation_count <= 1 => Verdict::Verified,
            Some(_) => Verdict::Failed("still_in_formation".to_string()),
        },
        "TRADE_ROUTE" => {
            if after.trade_routes.iter().any(|route| route.trader == id) {
                Verdict::Verified
            } else {
                Verdict::Failed("no_route".to_string())
            }
        }
        // `generals-lead-the-army`: the General is still ours after the frame.
        "KEEP_GREAT_PERSON" => {
            if now.is_some() {
                Verdict::Verified
            } else {
                Verdict::Failed("general_gone".to_string())
            }
        }
        "ACTIVATE_GREAT_PERSON" => {
            let charges = |u: Option<&civvis::mirror::StateUnit>| {
                u.and_then(|u| u.great_person.as_ref()).map(|gp| gp.charges)
            };
            let spent = matches!((charges(was), charges(now)), (Some(b), Some(a)) if a < b);
            if now.is_none() || spent {
                Verdict::Verified
            } else {
                Verdict::Failed("not_activated".to_string())
            }
        }
        "CONDEMN_HERETIC" => {
            // A military actor has no religious charge. The host correlates
            // this command with removal of a hostile religious unit under it;
            // losing sight of a target or losing the actor is not that proof.
            if evidence.iter().any(|event| {
                event["kind"].as_str() == Some("condemn_removed")
                    && event["turn"].as_u64() == Some(u64::from(turn))
                    && event["unit"].as_i64() == Some(id)
                    && event["target_player"].as_u64().is_some()
                    && event["target"].as_u64().is_some()
            }) {
                Verdict::Verified
            } else {
                Verdict::Failed("no_condemn_removal".to_string())
            }
        }
        "SPREAD_RELIGION" | "RELIGIOUS_HEAL" | "REMOVE_HERESY" | "LAUNCH_INQUISITION"
        | "CONVERT_BARBARIANS" => {
            // A Guru's heals are their own pool; its spread charges read 0.
            let pool = |unit: &civvis::mirror::StateUnit| {
                if op == "RELIGIOUS_HEAL" {
                    unit.heal_charges
                } else {
                    unit.spread_charges
                }
            };
            let spent = matches!(
                (was.and_then(pool), now.and_then(pool)),
                (Some(b), Some(a)) if a < b
            );
            if now.is_none() || spent {
                Verdict::Verified
            } else {
                Verdict::Failed("charge_not_spent".to_string())
            }
        }
        "PROMOTE" => match now {
            None => gone(),
            Some(u)
                if u.promotions
                    .as_ref()
                    .is_some_and(|p| p.iter().any(|have| have == arg)) =>
            {
                Verdict::Verified
            }
            Some(_) => Verdict::Failed("not_promoted".to_string()),
        },
        _ => Verdict::Unverifiable,
    }
}

fn great_person_arrived(
    class: &str,
    before: &civvis::mirror::StateSnapshot,
    observed: &civvis::mirror::StateSnapshot,
) -> bool {
    observed.units.iter().any(|unit| {
        unit.great_person
            .as_ref()
            .and_then(|gp| gp.class.as_deref())
            == Some(class)
            && own_unit(before, unit.id).is_none()
    })
}

/// `--strategy` named a league genome for this seat to play. The league is
/// retired (#2357): the live seat plays the deployment genome — `AdvancedAi`
/// with the gene ledger applied — and the gene screen prices everything else.
/// A launcher that still passes the flag is told so rather than silently
/// played stock, which is what the old resolver did when its roster was
/// missing.
fn refuse_retired_strategy_flag(args: &[String]) {
    if let Some(want) = arg_text(args, "--strategy") {
        eprintln!(
            "civvis-orders: --strategy {want} names a league genome, and the league is \
             retired (#2357); the live seat plays the deployment genome. Drop the flag."
        );
        std::process::exit(2);
    }
}

fn attach_input_capture(
    reply: String,
    capture: Option<mirror::input_capture::RequestCapture>,
) -> String {
    let Some(capture) = capture else {
        return reply;
    };
    let mut payload: serde_json::Value =
        serde_json::from_str(&reply).expect("decision reply is JSON");
    payload["input_capture"] = capture.finish();
    payload.to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = arg_text(&args, "--mirror") else {
        eprintln!(
            "usage: civvis-orders --mirror <run-dir> [--turn N] [--serve] \\
             [--audit-orders <orders.jsonl>] [--audit-host-moves] \\
             [--capture-inputs <new-directory>]"
        );
        std::process::exit(2);
    };
    let players: usize = arg_text(&args, "--players")
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let max_turns: u32 = arg_text(&args, "--max-turns")
        .and_then(|v| v.parse().ok())
        .unwrap_or(250);
    let frontier: u32 = arg_text(&args, "--frontier")
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    let victory = arg_text(&args, "--victory").unwrap_or_else(|| DEFAULT_VICTORY.to_string());
    refuse_retired_strategy_flag(&args);
    let mut ai = match victory_lane(&victory) {
        Some(lane) => lane,
        None => {
            eprintln!("unknown --victory {victory}; use {VICTORY_LANES}");
            std::process::exit(2);
        }
    };
    // Repeatable: `--with stacked-escort --with settler-stack-discipline`.
    // A `--with` arm restores only a ledger-held live treatment; it never
    // changes the deployment genome when the flag is absent.
    let forced: Vec<String> = args
        .windows(2)
        .filter(|pair| pair[0] == "--with")
        .map(|pair| pair[1].clone())
        .collect();
    // Repeatable: `--without come-ashore --without camp-party`.
    let withheld: Vec<String> = args
        .windows(2)
        .filter(|pair| pair[0] == "--without")
        .map(|pair| pair[1].clone())
        .collect();
    let forced_on = match forced_live_treatments(&forced) {
        Ok(selected) => selected,
        Err(why) => {
            eprintln!("civvis-orders: {why}");
            std::process::exit(2);
        }
    };
    if let Some(tag) = forced_on
        .iter()
        .find(|tag| withheld.iter().any(|treatment| treatment.as_str() == **tag))
    {
        eprintln!("civvis-orders: {tag:?} cannot be both --with and --without in one live arm");
        std::process::exit(2);
    }
    // Validate every requested row before a single turn is driven. Discovering
    // a typo on turn 1 of a four-hour live run is discovering it too late.
    {
        let mut probe = civvis::ai::AdvancedAi::new();
        if let Err(why) = configure_live_bridge(&mut probe, &forced_on, &withheld) {
            eprintln!("civvis-orders: {why}");
            std::process::exit(2);
        }
    }
    // Configure before recording the startup identity, so the metadata names
    // the same controller that will answer the first turn. `decide` repeats
    // this idempotently for fresh agents and both kinds of live arm.
    if let Err(why) = configure_live_bridge(&mut ai, &forced_on, &withheld) {
        eprintln!("civvis-orders: {why}");
        std::process::exit(2);
    }
    // ★★★ SAY WHICH GENOME IS PLAYING, ALWAYS — INCLUDING "the stock one".
    //
    // An axis nothing reports does not exist, and this project has already shipped a
    // learned evaluator that never once loaded while its documentation called it
    // good and inert. A run that does not name its genome cannot be told apart from
    // a run whose league file failed to resolve.
    //
    // ⚠⚠ STDERR, NOT STDOUT — AND THAT IS PROTOCOL, NOT STYLE.
    //
    // `--serve` speaks a strict one-line-per-request protocol: `civ6_brain.py` writes
    // a turn number and does exactly ONE `readline()`, then reads `payload["orders"]`.
    // Printing this to stdout put a line in front of the first response, and it is
    // valid JSON with no `orders` key — so it parsed cleanly, yielded an empty list,
    // and shifted every later turn by one. No error was raised anywhere.
    //
    // Measured after this line shipped: `turn 1: 0 orders in 0.33s` on a fresh run,
    // and a live run that had been 236 turns of `orders_source: civvis` flipped to
    // `fallback` the moment a binary carrying it was swapped in — the hand-written
    // ladder playing while CIVVIS decided correctly into a pipe nobody read. The
    // decider was never wrong; `why.log` showed it founding the capital on the very
    // turn the brain recorded zero orders.
    //
    // Anything this binary emits that is not a response belongs on stderr.
    eprintln!(
        "{}",
        serde_json::json!({
            "kind": "genome",
            // The deployment genome, always: the league's named genomes are
            // retired (#2357). The keys stay so the ladder ledger's reader
            // (`civ6_ladder.decider_genome`) keeps its shape.
            "strategy": "stock",
            "source": "AdvancedAi::new",
            "civ": serde_json::Value::Null,
            "strength_bound": serde_json::Value::Null,
            "per_civ": serde_json::Value::Null,
            "lane": serde_json::Value::Null,
            "victory": victory.clone(),
            "parallel_settlers": ai.parallel_settlers,
            // ⚠⚠ SAY WHAT THIS BINARY ACTUALLY CARRIES, EVERY RUN.
            //
            // A stale binary is invisible: `summary.json` records no revision, and
            // three of thirty-two recent live runs were executing a pre-fix build,
            // each losing most of a city's production to a defect already fixed on
            // `main`. They were identifiable only because they emitted a build name
            // a later commit had corrected — an accident that will not repeat for
            // the next stale build.
            //
            // The registry's `live()` genes are what `enable_live_bridge`
            // turns on. So a binary that predates a repair emits a SHORTER list,
            // and the difference names exactly which repairs were missing. That
            // also gives any A/B the one thing it needs and has never had: which
            // genes were actually live in the arm it measured.
            //
            // `revision` beside it is the supervisor's label when there is one —
            // `CIVVIS_COMMIT`, or a promoted `civvis-<sha>` executable name. It is
            // `null` for an ordinary development build, which is honest: the
            // treatment list is the identity that always reports.
            "revision": civvis::server::runtime_commit_or_none(),
            // ★★★★ THE LIST IS WHAT THE LEDGER LEFT ON, NOT THE UNIVERSE. Since
            // the gene ledger (`docs/gene_ledger.json`) decides the deployment
            // genome, the registry's live genes name what COULD be on; this
            // names what IS — the helpers the screens proved, the opt-ins they
            // proved, and the host-only flags no screen can price.
            "treatments": configured_live_treatments(&forced_on, &withheld),
            "ledger_withheld": civvis::ai::GENES
                .iter()
                .filter(|gene| gene.live())
                .map(|gene| gene.tag)
                .filter(|tag| {
                    civvis::ai::ledger_default_on(tag) == Some(false)
                        && !forced_on.contains(tag)
                })
                .collect::<Vec<_>>(),
            "forced": &forced_on,
        })
    );

    // ★★★★ HOLD THE SITE ACROSS A TURN THE SETTLER COULD NOT MOVE.
    //
    // Off by default in CIVVIS's own games; on here, and the reason is specific to
    // this bridge rather than a preference. Without it `advanced_settler_step` drops
    // the target on ANY turn the unit fails to move — a friendly unit in the way, a
    // zone of control, a barbarian standing on the route — and this bridge fails to
    // move settlers far more often than an ordinary game does, because it is also
    // refusing steps that would end inside a captor's reach. Dropping the target on
    // each of those turns would undo the unit memory that is now carried across the
    // rebuild, which is the whole point of carrying it.
    //
    // ⚠ Bounded, and the bound is what makes it safe: `SETTLER_STALL_LIMIT`
    // consecutive turns without getting closer releases the site, so an unreachable
    // target cannot hold a settler hostage — which is the livelock #492 was merged
    // to fix. This must include a linked escort: the completed Rome comparator's
    // escort moved legally while oscillating for 101 turns, and motion alone reset
    // the old counter forever.
    ai.settler_commit = true;
    ai.linked_settler_progress = true;
    // A Firaxis appointment is asynchronous: if its same-callback assignment
    // did not persist, the next mirrored turn must retry the posting without
    // waiting for another Governor Title that may be dozens of turns away.
    ai.live_governor_assignment_adapter = true;

    let events = Path::new(&dir).join("events.jsonl");
    // Offline: replay a recorded run's orders through the postcondition checks.
    if let Some(orders_path) = arg_text(&args, "--audit-orders") {
        audit_orders(&events, Path::new(&orders_path));
        return;
    }
    // Offline behavioural conformance: every host-observed unit step is checked
    // against the preceding reconstructed board, including moves not proposed by
    // CIVVIS.  This is the reverse direction the outbound order verifier cannot
    // see on its own.
    if args.iter().any(|arg| arg == "--audit-host-moves") {
        audit_host_moves(&events, players, max_turns, frontier);
        return;
    }
    let serve = args.iter().any(|a| a == "--serve");
    // ★ CIVVIS's own account of WHY. `--explain` attaches a recording journal — the
    // same one the spectator HUD reads — and dumps it to stderr. When the agent
    // returns no orders, "it chose nothing" and "it never reached the question" are
    // indistinguishable from the outside, and this is the difference.
    let explain = args.iter().any(|a| a == "--explain");
    let journal = if explain {
        let j = civvis::reasoning::Journal::recording();
        ai.attach_journal(j.handle());
        Some(j)
    } else {
        None
    };
    let fresh_ai = args.iter().any(|a| a == "--fresh-ai");
    let war_from_plan = args.iter().any(|a| a == "--war-from-plan");
    let fresh_board = args.iter().any(|a| a == "--fresh-board");
    // `CityFireMemory`'s kill switch: the flag or `CIVVIS_NO_CITY_FIRE_MEMORY=1`.
    let city_fire_memory = !args.iter().any(|a| a == "--no-city-fire-memory")
        && !civvis::mirror::CityFireMemory::disabled_by_env();
    let arm = DecisionArm {
        war_from_plan,
        forced_on: &forced_on,
        withheld: &withheld,
    };

    // Read the board fresh each time: the mod appends to this file every turn.
    // Replay only — see `assume_seat_capabilities`. Validated here so a typo
    // fails before any board is read.
    let assumed_seat: Vec<String> = arg_text(&args, "--assume-seat")
        .map(|list| list.split(',').map(str::to_string).collect())
        .unwrap_or_default();
    if let Err(why) = assume_seat_capabilities(&mut civvis::mirror::Seat::default(), &assumed_seat)
    {
        eprintln!("civvis-orders: {why}");
        std::process::exit(2);
    }
    let load =
        |want: Option<u32>| -> Option<(civvis::mirror::Snapshot, civvis::mirror::StateSnapshot)> {
            let snapshot = mirror::snapshot_from_events_at(&events, want).ok()?;
            let mut state = mirror::state_from_events(&events, want)?;
            if snapshot.revealed_count() == 0 {
                return None;
            }
            let _ = assume_seat_capabilities(&mut state.seat, &assumed_seat);
            Some((snapshot, state))
        };

    if args.iter().any(|a| a == "--explain-builders") {
        match builder_diagnostics::explain(&args, &ai, players, max_turns, frontier, &load) {
            Ok(report) => println!("{report}"),
            Err(why) => {
                eprintln!("civvis-orders: {why}");
                std::process::exit(2);
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--explain-settler") {
        diagnostics::explain_settler(args, players, max_turns, frontier, &load);
        return;
    }
    if args.iter().any(|a| a == "--dump-mirror") {
        diagnostics::dump_mirror(args, players, max_turns, frontier, &load);
        return;
    }

    // Opt-in only: record the bytes each reader actually receives, rather than
    // guessing one input frontier for a decision against a growing host log.
    let input_capture = arg_text(&args, "--capture-inputs").map(|directory| {
        mirror::input_capture::InputCapture::create(Path::new(&directory), &events).unwrap_or_else(
            |error| {
                eprintln!("civvis-orders: cannot create input capture: {error}");
                std::process::exit(2);
            },
        )
    });
    if args.iter().any(|arg| arg == "--capture-inputs") && input_capture.is_none() {
        eprintln!("civvis-orders: --capture-inputs requires a new directory");
        std::process::exit(2);
    }
    let begin_capture = || {
        input_capture
            .as_ref()
            .map(|capture| capture.begin().expect("one decision at a time"))
    };

    if !serve {
        let capture = begin_capture();
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            let reply = attach_input_capture(
                r#"{"turn":0,"orders":[],"note":"no revealed terrain or no state yet"}"#
                    .to_string(),
                capture,
            );
            println!("{reply}");
            return;
        };
        let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
        let mut live = civvis::mirror::LiveMirror::new(
            &snapshot,
            &state,
            mirror_players,
            1,
            mirror_turns,
            frontier,
        );
        // One frame has no earlier reading of a fogged city's fire; its
        // seat's research still floors it. See `CityFireMemory`.
        if city_fire_memory {
            civvis::mirror::CityFireMemory::default().apply(&mut live.game);
        }
        // One-shot: there is no next turn to be self-limiting against, so this starts
        // empty and every foreign choice is released once.
        let mut ours = std::collections::BTreeMap::new();
        // `--resumed`: replay the turn the way the live bridge's agent plays
        // a board it first sees past turn one (the `--serve` fresh-board
        // branch): the opening book is already played and the host's current
        // items are our plan. Without it a fresh one-shot agent has
        // `book_pos = 0`, so every mid-game replay plays the opening book and
        // never reaches the strategic production claims at all (no
        // `early-settler-floor` Settler on a turn the live seat queued one).
        if args.iter().any(|a| a == "--resumed") && state.turn > 1 {
            ai.skip_opening_book();
            adopt_host_production(&mut ours, &state);
        }
        let mut host_peace_retries = HostPeaceRetries::default();
        let mut host_move_refusals = HostMoveRefusals::default();
        let mut host_order_refusals = HostOrderRefusals::default();
        let mut air_assault_continuation = air_assault_continuation::Continuation::default();
        let mut governor_exchange = governor_exchange::Continuation::default();
        // `denial-spies-count-their-losses`: the one frame reads every loss the
        // log holds up to its turn. See `SpyLossLedger`.
        if ai.denial_spies_count_their_losses_enabled() {
            let mut spy_losses = SpyLossLedger::default();
            spy_losses.refresh(&events);
            ai.observe_spy_losses(spy_losses.through(state.turn));
        }
        // `luxury-asks-pay-in-strategics-2`: the one frame reads every short
        // the log holds up to its turn. See `BarterShortLedger`.
        if ai.luxury_asks_pay_in_strategics_2_enabled() {
            let mut barter_shorts = BarterShortLedger::default();
            barter_shorts.refresh(&events);
            ai.observe_luxury_barter_shorts(barter_shorts.through(state.turn));
        }
        let reply = decide(
            &mut live,
            &mut ai,
            &snapshot,
            &state,
            arm,
            DecisionMemory {
                ours: &mut ours,
                host_peace_retries: &mut host_peace_retries,
                host_move_refusals: &mut host_move_refusals,
                host_order_refusals: &mut host_order_refusals,
                air_assault_continuation: &mut air_assault_continuation,
                governor_exchange: &mut governor_exchange,
            },
        );
        // ⚠ `--explain` USED TO WORK ONLY UNDER `--serve`, which is the mode you cannot
        // debug in. Replaying one recorded turn is the fast loop — seconds, no game,
        // no lock — and it was the one path that could not say why it chose anything.
        if let Some(j) = &journal {
            for thought in &j.since(0).thoughts {
                eprintln!("{}", explain_line(thought));
            }
        }
        let reply = attach_input_capture(reply, capture);
        println!("{reply}");
        return;
    }

    // ---- persistent mode -------------------------------------------------------
    //
    // One line of input per turn (the turn number, or blank for "newest"), one line of
    // orders JSON out. The mirror and the agent live for the whole game, which is what
    // gives CIVVIS a plan that spans turns.
    //
    // ⚠ Errors answer with an EMPTY order list rather than dying, so a bad turn costs
    // one turn's decisions. The mod then records `fallback` and the game keeps moving —
    // a brain that takes the run down with it is worse than one that misses a turn.
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut live: Option<civvis::mirror::LiveMirror> = None;
    // What CIVVIS itself last asked each city to build, by Civilization VI city id.
    // Lives out here rather than on the mirror because `--fresh-board` rebuilds the
    // mirror every turn and this must survive that, exactly like the unit memory
    // carried through `remap_unit_memory` below. See `release_foreign_production`.
    let mut ours: std::collections::BTreeMap<i64, String> = std::collections::BTreeMap::new();
    // This records an operation Firaxis already accepted, rather than a simulated
    // choice. Keep it through all bridge reconstruction modes so a fresh board or
    // diagnostic fresh AI cannot repeat a host-cooldown peace request.
    let mut host_peace_retries = HostPeaceRetries::default();
    let mut host_move_refusals = HostMoveRefusals::default();
    let mut host_order_refusals = HostOrderRefusals::default();
    let mut air_assault_continuation = air_assault_continuation::Continuation::default();
    let mut governor_exchange = governor_exchange::Continuation::default();
    // The Firaxis repair cooldown belongs to the host, not the reconstructed
    // board. It must therefore survive `--fresh-board` just like the peace and
    // treasury handoffs above.
    let mut host_city_attack_cooldowns = HostCityAttackCooldowns::default();
    // The export reads a rival city's fire only while it is in sight; the
    // board rebuilt each turn keeps the last reading. See `CityFireMemory`.
    let mut host_city_fire = civvis::mirror::CityFireMemory::default();
    // A city's strike is once per host turn and the export never says it was
    // spent; the decider's own earlier frames do. See `HostCityStrikes`.
    let mut host_city_strikes = HostCityStrikes::default();
    let mut host_air_strikes = HostAirStrikes::default();
    let mut host_ranged_history = host_ranged_history::History::default();
    // `denial-spies-count-their-losses`: our Spies the host reported lost,
    // read from the run's own log as it grows. See `SpyLossLedger`.
    let mut spy_losses = SpyLossLedger::default();
    // `luxury-asks-pay-in-strategics-2`: partners our barter could not reach.
    let mut barter_shorts = BarterShortLedger::default();
    let mut explain_cursor: u64 = 0;
    // What left for the host on each frame, until the next turn's frame answers
    // for it. See "order postconditions" above.
    let mut pending_orders: Vec<PendingOrders> = Vec::new();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let capture = begin_capture();
        let want: Option<u32> = line.trim().parse().ok();
        let reply = match load(want) {
            None => format!(
                "{{\"turn\":{},\"orders\":[],\"note\":\"no revealed terrain or no state yet\"}}",
                want.unwrap_or(0)
            ),
            Some((snapshot, state)) => {
                let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
                host_city_attack_cooldowns.observe(&state);
                // Read replacements before settlement consumes the prior turn's
                // orders. Native upgrades can replace an ID, without replacing
                // the soldier whose siege/escort memory the AI must carry.
                let upgraded_units = unit_identity::replacement_ids(&pending_orders, &state);
                // The verdicts on the previous turn's orders ride at the end of
                // this turn's reply; the checks read the frame the decision is
                // about to read, before anything is decided on it.
                let verdicts = settle_pending_orders(
                    &mut pending_orders,
                    &events,
                    &state,
                    &snapshot,
                    &mut host_order_refusals,
                );
                // The verdicts include Firaxis's named answer for an exact
                // frontier probe. Read it before rebuilding the mirror so a
                // transient failure cannot be mistaken for dead terrain.
                host_move_refusals.observe(&state, &verdicts);
                // ★★★★★ FRESH BOARD, PERSISTENT AGENT — the one combination that works.
                //
                // Three of the four quadrants were measured: fresh board + fresh agent
                // gives 17-30 real orders; persistent board + persistent agent gives 0;
                // persistent board + fresh agent gives 0. The board is what cannot be
                // reused, because `Ai::take_turn` needs a turn that has advanced through
                // the engine's own `begin_turn`, which is private and would simulate a
                // second game.
                //
                // The AGENT can be reused, and it is the half that carries the plan:
                // `StrategicPlan` holds the grand strategy, the war target and the city
                // target, and none of those are keyed to a unit id, so rebuilding the
                // board does not invalidate them. That is the continuity a domination
                // win needs — a target chosen once and built toward, instead of
                // re-derived every turn by an agent that has never seen this world
                // before.
                //
                // ⚠ Unit-keyed memory (`settler_targets`) CAN attach to the wrong unit,
                // because ids are reassigned when the board is rebuilt. Bounded: it
                // misdirects one settler, and CIVVIS re-targets next turn. Worth
                // watching if settlers start wandering.
                if ai.denial_spies_count_their_losses_enabled() {
                    spy_losses.refresh(&events);
                    ai.observe_spy_losses(spy_losses.through(state.turn));
                }
                if ai.luxury_asks_pay_in_strategics_2_enabled() {
                    barter_shorts.refresh(&events);
                    ai.observe_luxury_barter_shorts(barter_shorts.through(state.turn));
                }
                let reply = if fresh_board {
                    // ★★★★ A DECIDER THAT FIRST SEES THE GAME PAST TURN ONE HAS
                    // NO OPENING TO PLAY. The bridge restarts this process whenever
                    // `origin/main` advances, and a fresh agent's opening book
                    // handed every city to the baseline governor for ten turns
                    // per restart (Skirmishers, then Rangers, in all seven cities
                    // of run civvis-20260816T213447Z at t139; Artillery at
                    // t157). See `BasicAi::skip_opening_book`.
                    if live.is_none() && state.turn > 1 {
                        ai.skip_opening_book();
                        // ★★★★ AND WHAT THE HOST IS BUILDING RIGHT NOW IS OURS. `ours`
                        // is what tells `release_foreign_production` a queue is
                        // CIVVIS's own plan rather than the ladder's; a fresh process
                        // holds none, so on its first turn every city's work in
                        // progress read as foreign, was released, and was re-decided
                        // — the Taj Mahal at 188/460 in Ravenna dropped for a
                        // University, the wonder re-ordered from Rome. On a seat that
                        // has been CIVVIS's for a hundred turns, the host's current
                        // item is the plan; adopt it and let the ordinary rule take
                        // over from the next order on.
                        adopt_host_production(&mut ours, &state);
                    }
                    // ★★★★★ CARRY THE UNIT MEMORY ACROSS THE REBUILD INSTEAD OF
                    // DROPPING IT. This used to call `forget_unit_memory`, on the
                    // sound reasoning that rebuilding the board reassigns unit ids so
                    // every unit-keyed map describes the wrong unit. The reasoning was
                    // right and the conclusion was too strong: the mirror knows each
                    // board's Civ 6 id for every unit, so old id -> Civ 6 id -> new id
                    // recovers the mapping exactly, and units that died just drop out.
                    //
                    // What forgetting cost, measured on run civvis-20260731T055749Z:
                    // the settler's DESTINATION was re-derived from scratch every turn
                    // and flipped — a site 23 tiles away on t14, t18 and t20, a
                    // different one 7 tiles away on t16 — so it never committed to
                    // anything and never arrived. The livelock detector is unit-keyed
                    // too, so the one mechanism that exists to catch a unit going in
                    // circles could never fire in this bridge at all.
                    let previous: Option<std::collections::BTreeMap<u32, i64>> =
                        live.as_ref().map(|board| board.civ6_of.clone());
                    // The treasury baseline dies with the old board unless it is
                    // handed over: see `LiveMirror::carry_treasury_baseline`. Without
                    // this, `gold_per_turn` is 0 for the whole game and every
                    // bankruptcy response that reads it is dead.
                    let carried_treasury =
                        live.as_ref().and_then(|board| board.treasury_baseline());
                    let mut board = civvis::mirror::LiveMirror::new(
                        &snapshot,
                        &state,
                        mirror_players,
                        1,
                        mirror_turns,
                        frontier,
                    );
                    match previous {
                        Some(old) => {
                            let carried =
                                unit_identity::carried_units(&old, &board.uid_of, &upgraded_units);
                            if let Some(previous_board) = live.as_ref() {
                                ai.remap_campaign_city_memory(&previous_board.game, &board.game);
                                ai.remap_siege_memory(&previous_board.game, &board.game, &carried);
                                ai.remap_conquest_memory(
                                    &previous_board.game,
                                    &board.game,
                                    &carried,
                                );
                                ai.remap_objective_board_memory(
                                    &previous_board.game,
                                    &board.game,
                                    &carried,
                                );
                                ai.remap_city_memory(&previous_board.game, &board.game, &carried);
                            }
                            ai.remap_unit_memory(&carried);
                        }
                        None => ai.forget_unit_memory(),
                    }
                    board.carry_treasury_baseline(carried_treasury);
                    host_ranged_history.observe_and_apply(&mut board, &state);
                    host_city_attack_cooldowns.apply(&mut board);
                    if city_fire_memory {
                        host_city_fire.apply(&mut board.game);
                    }
                    host_city_strikes.apply(&mut board, state.turn);
                    host_air_strikes.apply(&mut board, state.turn);
                    host_move_refusals.apply(&mut board);
                    let reply = decide(
                        &mut board,
                        &mut ai,
                        &snapshot,
                        &state,
                        arm,
                        DecisionMemory {
                            ours: &mut ours,
                            host_peace_retries: &mut host_peace_retries,
                            host_move_refusals: &mut host_move_refusals,
                            host_order_refusals: &mut host_order_refusals,
                            air_assault_continuation: &mut air_assault_continuation,
                            governor_exchange: &mut governor_exchange,
                        },
                    );
                    live = Some(board);
                    reply
                } else {
                    match live.as_mut() {
                        None => {
                            let mut fresh = civvis::mirror::LiveMirror::new(
                                &snapshot,
                                &state,
                                mirror_players,
                                1,
                                mirror_turns,
                                frontier,
                            );
                            host_ranged_history.observe_and_apply(&mut fresh, &state);
                            host_city_attack_cooldowns.apply(&mut fresh);
                            if city_fire_memory {
                                host_city_fire.apply(&mut fresh.game);
                            }
                            host_city_strikes.apply(&mut fresh, state.turn);
                            host_air_strikes.apply(&mut fresh, state.turn);
                            host_move_refusals.apply(&mut fresh);
                            let reply = decide(
                                &mut fresh,
                                &mut ai,
                                &snapshot,
                                &state,
                                arm,
                                DecisionMemory {
                                    ours: &mut ours,
                                    host_peace_retries: &mut host_peace_retries,
                                    host_move_refusals: &mut host_move_refusals,
                                    host_order_refusals: &mut host_order_refusals,
                                    air_assault_continuation: &mut air_assault_continuation,
                                    governor_exchange: &mut governor_exchange,
                                },
                            );
                            live = Some(fresh);
                            reply
                        }
                        Some(existing) => {
                            existing.sync(&snapshot, &state, frontier);
                            host_ranged_history.observe_and_apply(existing, &state);
                            host_city_attack_cooldowns.apply(existing);
                            if city_fire_memory {
                                host_city_fire.apply(&mut existing.game);
                            }
                            host_city_strikes.apply(existing, state.turn);
                            host_air_strikes.apply(existing, state.turn);
                            host_move_refusals.apply(existing);
                            // `--fresh-ai` isolates the two halves of persistence: keep the
                            // mirror, throw away the agent. If orders come back, the empty
                            // turns are the AGENT's carried state; if they stay empty, they
                            // are the MIRROR's. Guessing between the two cost several
                            // rebuilds, so it is worth a flag.
                            if fresh_ai {
                                // ⚠ This was a SECOND hand-written match, and it did not
                                // agree with the one that built `ai`: it listed three
                                // lanes and sent everything else to Domination, so a
                                // `--fresh-ai` run silently swapped the objective it was
                                // asked for. `victory` was already validated at startup,
                                // so the lane always resolves here.
                                let mut throwaway = victory_lane(&victory)
                                    .expect("--victory was validated at startup");
                                decide(
                                    existing,
                                    &mut throwaway,
                                    &snapshot,
                                    &state,
                                    arm,
                                    DecisionMemory {
                                        ours: &mut ours,
                                        host_peace_retries: &mut host_peace_retries,
                                        host_move_refusals: &mut host_move_refusals,
                                        host_order_refusals: &mut host_order_refusals,
                                        air_assault_continuation: &mut air_assault_continuation,
                                        governor_exchange: &mut governor_exchange,
                                    },
                                )
                            } else {
                                decide(
                                    existing,
                                    &mut ai,
                                    &snapshot,
                                    &state,
                                    arm,
                                    DecisionMemory {
                                        ours: &mut ours,
                                        host_peace_retries: &mut host_peace_retries,
                                        host_move_refusals: &mut host_move_refusals,
                                        host_order_refusals: &mut host_order_refusals,
                                        air_assault_continuation: &mut air_assault_continuation,
                                        governor_exchange: &mut governor_exchange,
                                    },
                                )
                            }
                        }
                    }
                };
                let orders = IssuedOrder::from_reply(&reply);
                host_city_strikes.observe(state.turn, &orders);
                host_air_strikes.observe(state.turn, &orders);
                pending_orders.push(PendingOrders {
                    turn: state.turn,
                    frame: state.frame,
                    orders,
                    before: state.clone(),
                });
                append_rows_to_reply(&reply, &verdicts)
            }
        };
        if let Some(j) = &journal {
            let delta = j.since(explain_cursor);
            explain_cursor = delta.cursor;
            for thought in &delta.thoughts {
                eprintln!("{}", explain_line(thought));
            }
        }
        let reply = attach_input_capture(reply, capture);
        if writeln!(out, "{reply}").is_err() {
            break;
        }
        if out.flush().is_err() {
            break;
        }
    }
}

/// One line of CIVVIS's reasoning, with the coordinates the OPERATOR can check.
///
/// ★★★★★ EVERY POSITION IN THE JOURNAL IS AXIAL AND EVERY POSITION ON THE SCREEN IS
/// OFFSET, and the two are both pairs of small integers. Reading "Settler marching to
/// (10, 11)" against a game window showing the settler at (15, 11) reads as CIVVIS
/// ordering nonsense; they are the SAME TILE. I lost most of an hour to it tonight,
/// chasing a coordinate bug that did not exist, on the very run the operator asked me
/// to watch for exactly this.
///
/// The headline text is CIVVIS's own and stays in CIVVIS's own coordinates — rewriting
/// another module's prose would be worse — but the thought carries its focus position
/// separately, and that is appended here in OFFSET, tagged, so the number beside the
/// line is the number on the screen. See [[civvis-civ6-bridge]]: Civ 6 exports OFFSET,
/// CIVVIS stores AXIAL, and nothing complains when they are mixed.
fn explain_line(thought: &civvis::reasoning::Thought) -> String {
    let focus = match thought.focus {
        Some(pos) => {
            let (x, y) = civvis::hex::axial_to_offset(pos.0, pos.1);
            format!("  [civ6 ({x},{y}) = axial ({},{})]", pos.0, pos.1)
        }
        None => String::new(),
    };
    format!(
        "[why] t{} {:?}/{:?} {} | {}{}",
        thought.turn, thought.topic, thought.level, thought.headline, thought.detail, focus
    )
}

/// A short, stable label per action kind, for the skipped tally.
///
/// ⚠ NAME EVERY BUCKET. A tally whose biggest entry is `other` cannot be acted on —
/// over 81 replayed turns it read `untranslatable 849, other 466, buy 122,
/// government 81`, and the two largest said nothing at all about what was lost.
/// `Action::Improve` hid in `other` for the whole project and was the reason CIVVIS
/// ordered builder after builder.
/// The variant's own name, taken from its Debug form.
///
/// ⚠ A HAND-WRITTEN LIST OF LABELS GOES STALE AND LIES. A curated `action_label`
/// list (since removed) still reported `other = 932` over 81 turns — 11 a turn of
/// something nobody could name, because the list did not cover every variant and
/// there is no compiler error for a missing arm behind `_`. Reading the name off
/// Debug cannot drift.
fn action_variant(action: &Action) -> String {
    let text = format!("{action:?}");
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("other")
        .to_string()
}

#[cfg(test)]
#[path = "civvis_orders/tests.rs"]
mod tests;

/// Every Civilization VI type name this binary can emit must be one the game
/// actually ships.
///
/// ## Why this is a test and not a review note
///
/// The outbound mapping is a `match` with a fallthrough that uppercases CIVVIS's
/// own name. When the two rulesets happen to agree that is correct and free;
/// when they disagree the host **silently discards the order**. Nothing in the
/// telemetry says so — the order is written, `orders_source` still reads
/// `civvis`, and the city simply builds nothing.
///
/// That is not hypothetical. Three separate rounds of it are recorded in the
/// doc comments above:
///
/// - `BUILDING_MEDIEVAL_WALLS`, 200 refused orders over live turns 165-219;
/// - `PROJECT_CAMPUS_RESEARCH_GRANTS` and its six siblings, all seven silently
///   unbuildable;
/// - `BUILDING_ARCHAEOLOGICAL_MUSEUM`, **248 orders across turns 118-250 of run
///   `civvis-20260803T014330Z`, in all three of that empire's main cities**,
///   which finished the game holding the building in none of them. Civilization
///   VI calls it `BUILDING_MUSEUM_ARTIFACT`.
///
/// Each was repaired by adding one more arm. None of them added a way to find
/// the next one, and each cost most of a game's production to notice. This
/// closes the class: `data/civ6_type_names.json` is harvested from the shipped
/// rule files by `tools/civ6_type_names.py`, and every name the mapping can
/// produce is checked against it.
///
/// ⚠ The snapshot deliberately excludes `DLC/CivvisControl`. Our control mod is
/// installed *into* the game's Assets tree, so a scan that includes it reads our
/// own invented names back as proof the game has them — which is how
/// `BUILDING_ARCHAEOLOGICAL_MUSEUM` looked legitimate on first inspection.
#[cfg(test)]
#[path = "civvis_orders/civ6_name_audit.rs"]
mod civ6_name_audit;
/// The receiving side of the bridge: every check reads two consecutive frames,
/// never a return code.
#[cfg(test)]
#[path = "civvis_orders/order_postcondition_tests.rs"]
mod order_postcondition_tests;

#[cfg(test)]
#[path = "civvis_orders/air_pillage_tests.rs"]
mod air_pillage_tests;

#[cfg(test)]
#[path = "civvis_orders/air_sequence_tests.rs"]
mod air_sequence_tests;

#[cfg(test)]
#[path = "civvis_orders/air_receipt_tests.rs"]
mod air_receipt_tests;

#[cfg(test)]
#[path = "civvis_orders/input_capture_tests.rs"]
mod input_capture_tests;

#[cfg(test)]
#[path = "civvis_orders/defending_district_tests.rs"]
mod defending_district_tests;

#[cfg(test)]
#[path = "civvis_orders/army_upgrade_materials_tests.rs"]
mod army_upgrade_materials_tests;

#[cfg(test)]
#[path = "civvis_orders/repair_postconditions_tests.rs"]
mod repair_postconditions_tests;

#[cfg(test)]
#[path = "civvis_orders/culture_war_materials_tests.rs"]
mod culture_war_materials_tests;
