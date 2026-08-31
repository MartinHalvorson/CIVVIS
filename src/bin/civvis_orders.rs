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
        if state.seat.max_turns > 0 {
            state.seat.max_turns as u32
        } else {
            fallback_turns
        },
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
        "toa" => "MAORI_TOA",
        "crouching_tiger" => "CHINESE_CROUCHING_TIGER",
        "nihang" => "LAHORE_NIHANG",
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

/// Firaxis blocks outer-defense repairs for three turns after a city takes damage.
///
/// `civvis_orders --serve --fresh-board` deliberately reconstructs the board for
/// every decision, so `City::last_attacked` otherwise returns to its default zero
/// each turn. That made a recently hit city look eligible for
/// `PROJECT_REPAIR_OUTER_DEFENSES`; Firaxis then rejected the order and left an
/// endangered city on its offensive queue. Keep this host-only cooldown beside the
/// other persistent bridge facts, then apply it to the newly reconstructed board
/// before the AI evaluates production.
#[derive(Clone, Copy)]
struct HostCityDamage {
    garrison: Option<f64>,
    outer_defenses: Option<f64>,
}

impl HostCityDamage {
    fn from_city(city: &civvis::mirror::StateCity) -> Self {
        let bar = |damage: f64, maximum: f64| {
            (damage.is_finite() && maximum.is_finite() && damage >= 0.0 && maximum > 0.0)
                .then_some((damage / maximum).clamp(0.0, 1.0))
        };
        Self {
            garrison: bar(city.damage, city.max_damage),
            outer_defenses: bar(city.wall_damage, city.max_wall_damage),
        }
    }

    fn increased_since(self, previous: Self) -> bool {
        let increased = |now: Option<f64>, was: Option<f64>| {
            now.zip(was)
                .is_some_and(|(now, was)| now > was + f64::EPSILON)
        };
        increased(self.garrison, previous.garrison)
            || increased(self.outer_defenses, previous.outer_defenses)
    }
}

#[derive(Default)]
struct HostCityAttackCooldowns {
    observed_turn: Option<u32>,
    health: std::collections::BTreeMap<i64, HostCityDamage>,
    last_attack: std::collections::BTreeMap<i64, u32>,
}

impl HostCityAttackCooldowns {
    /// Retain a hit only when adjacent state frames prove it. A skipped/replayed
    /// turn is unknown rather than evidence of an attack, which avoids inventing a
    /// host cooldown from an arbitrary long gap.
    fn observe(&mut self, state: &civvis::mirror::StateSnapshot) {
        let health: std::collections::BTreeMap<i64, HostCityDamage> = state
            .cities
            .iter()
            .map(|city| (city.id, HostCityDamage::from_city(city)))
            .collect();
        let consecutive = self
            .observed_turn
            .is_some_and(|previous| state.turn == previous.saturating_add(1));
        if consecutive {
            for (&city, &current) in &health {
                if self
                    .health
                    .get(&city)
                    .is_some_and(|previous| current.increased_since(*previous))
                {
                    self.last_attack.insert(city, state.turn);
                }
            }
        } else if self
            .observed_turn
            .is_some_and(|previous| state.turn < previous)
        {
            // A replay starts a new timeline. Retaining a future timestamp would
            // make `saturating_sub` claim every repair is still cooling down.
            self.last_attack.clear();
        }
        self.last_attack
            .retain(|city, attack_turn| health.contains_key(city) && *attack_turn <= state.turn);
        self.observed_turn = Some(state.turn);
        self.health = health;
    }

    /// Restore the host's recent-hit timestamp after a fresh reconstruction.
    fn apply(&self, mirror: &mut civvis::mirror::LiveMirror) {
        for (&host_city, &city) in &mirror.cid_of {
            let Some(&attack_turn) = self.last_attack.get(&host_city) else {
                continue;
            };
            if let Some(live) = mirror.game.cities.get_mut(&city) {
                live.last_attacked = attack_turn;
            }
        }
    }
}

/// The ranged strikes the decider has already ordered this turn, per host
/// city, replayed onto every board built for the turn.
///
/// ★★★★ THE EXPORT CARRIES NO "ATTACKS REMAINING" FOR A CITY. A unit's
/// `attacks_remaining` (`Unit:GetAttacksRemaining`) crosses on every frame and
/// the board plans no second shot for it; a city's strike is answered only by
/// `CityManager.CanStartCommand(city, CityCommandTypes.RANGE_ATTACK)`
/// (`Base/Assets/UI/WorldView/CityBannerManager.lua:1555`, `CanRangeAttack`;
/// `WorldInput.lua:2545` asks the same before `RequestCommand`), which the mod
/// asks at order time and which refuses a second strike in the same turn. The
/// decider plans every frame on a throwaway clone of the mirror, so the
/// authoritative board's `struck` stayed false across the turn's replan
/// frames and the same city was ordered to fire again on each one. Measured
/// on run civvis-20260826T184456Z: 66 `city_strike` orders, 31 refused
/// (`city_strike_refused`), every one of the 31 a re-issue on frame 1 or 2 by
/// a city whose frame-0 strike had landed (the target's hit points fell
/// between the frames); walls, range (≤ 2), visibility and war state were in
/// order on all 31, and no frame-0 strike was refused. Over the last five
/// control runs the refusal was 75 of 161 strikes. Remembered here exactly
/// like the host's repair cooldown above, because it belongs to the host's
/// turn and not to the reconstructed board.
///
/// Victor's Embrasure grants one extra strike; the count of strikes ordered
/// is kept so `extra_strikes_used` carries it and `city_can_strike` reads the
/// same rule it reads for a strike applied on the board itself.
#[derive(Default)]
struct HostCityStrikes {
    turn: Option<u32>,
    city: std::collections::BTreeMap<i64, i32>,
    encampment: std::collections::BTreeMap<i64, i32>,
}

impl HostCityStrikes {
    /// Count the strikes that left for the host on this frame.
    fn observe(&mut self, turn: u32, orders: &[IssuedOrder]) {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.city.clear();
            self.encampment.clear();
        }
        for order in orders {
            let Some(subject) = order.subject else {
                continue;
            };
            match order.kind.as_str() {
                "city_strike" => *self.city.entry(subject).or_insert(0) += 1,
                "encampment_strike" => *self.encampment.entry(subject).or_insert(0) += 1,
                _ => {}
            }
        }
    }

    /// Spend, on a board built for `turn`, the strikes already ordered on it.
    fn apply(&self, mirror: &mut civvis::mirror::LiveMirror, turn: u32) {
        if self.turn != Some(turn) {
            return;
        }
        for (&host_city, &city) in &mirror.cid_of {
            let Some(live) = mirror.game.cities.get_mut(&city) else {
                continue;
            };
            if let Some(&count) = self.city.get(&host_city) {
                live.struck = true;
                live.extra_strikes_used = live.extra_strikes_used.max(count - 1);
            }
            if let Some(&count) = self.encampment.get(&host_city) {
                live.encampment_struck = true;
                live.encampment_extra_strikes_used =
                    live.encampment_extra_strikes_used.max(count - 1);
            }
        }
    }
}

/// Plots the host will not walk a unit onto, learned from moves that went
/// nowhere.
///
/// ★★★★ AN ACCEPTED MOVE THAT NEVER MOVES IS THE MIRROR'S BLIND SPOT. The
/// mirror marks unrevealed plots within the frontier depth as traversable so
/// exploration and expansion can aim outward, and Civilization VI accepts a
/// `MOVE_TO` toward one without a `move_refused` — then, if the plot is really
/// ice, ocean or a mountain behind a hill, walks the unit nowhere. Run
/// civvis-20260816T093036Z: the only Scout stood on (12,12) from t13 to t49
/// ordered `MOVE_TO (9,11)` on 28 consecutive turns; 193 plots revealed by t50.
/// #1735 retires an exploration GOAL the unit stands still for, but the goal
/// varied while the coalesced first step through (9,11) did not, so nothing
/// fired. The host receives a coalesced destination, but CIVVIS still knows
/// each local step that led there. A unit that never starts a long route first
/// retries that exact speculative step; only if Firaxis refuses the one-step
/// probe too does it become unwalkable. The mirror then stops assuming it is
/// traversable, and every path — exploration, settlers, the army — routes
/// around it. Revealed terrain is never overridden: once the plot is seen, its
/// real terrain governs.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct HostUnitOrderKey {
    verb: String,
    pos: Option<(i32, i32)>,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct HostDealOrderKey {
    kind: &'static str,
    subject: Option<i64>,
    verb: Option<String>,
    pos: Option<(i32, i32)>,
}

#[derive(Default)]
struct HostMoveRefusals {
    /// The last `MOVE_TO` sent per host unit: where it stood, the long
    /// destination sent to Firaxis, the first unknown local step, and the turn.
    sent: std::collections::BTreeMap<i64, HostMoveAttempt>,
    /// A first unknown hop from a long route the host did not begin. It must
    /// become the next exact `MOVE_TO` before terrain is declared dead.
    pending_probes: std::collections::BTreeMap<i64, HostFrontierProbe>,
    /// Speculative plots proved unwalkable, in host offset coordinates.
    dead: std::collections::BTreeSet<(i32, i32)>,
    /// Unit orders already emitted during the current turn, keyed by host unit.
    /// Same-turn frames can still carry the previous frame's positions, so an
    /// identical movement command is a replay of a request the host has already
    /// consumed rather than a new route decision.
    same_turn_orders: std::collections::BTreeMap<i64, std::collections::BTreeSet<HostUnitOrderKey>>,
    /// The diplomacy arm also has one working deal per rival and keeps an ask
    /// pending or cooling down across same-turn frames. An identical
    /// `sell`/`buy` in a later frame therefore cannot be a new deal request.
    same_turn_deal_orders: std::collections::BTreeSet<HostDealOrderKey>,
    same_turn: Option<u32>,
}

#[derive(Clone, Copy)]
struct HostMoveAttempt {
    from: (i32, i32),
    destination: (i32, i32),
    /// The first unrevealed step CIVVIS simulated before coalescing the whole
    /// path into `destination`, or that destination for a fully revealed route.
    /// It is the actionable feedback target when the host never starts the
    /// route at all.
    frontier_step: (i32, i32),
    turn: u32,
}

#[derive(Clone, Copy)]
struct HostFrontierProbe {
    from: (i32, i32),
    target: (i32, i32),
}

impl HostMoveRefusals {
    /// Drop exact, non-tactical unit and deal replays from later frames of one
    /// turn.
    ///
    /// The host may apply a queued move before the next exported frame catches
    /// up. CIVVIS then plans from the old tile and emits the same `MOVE_TO`
    /// again; the host records that as `from == destination` / `move_refused`.
    /// Waiting for the next turn is the only useful retry because this frame has
    /// no newer authoritative position. FORTIFY and civilian CAPTURE have the
    /// same stale-state failure mode. The diplomacy arm has the same shape:
    /// its pending/cooldown guards mean a repeated deal order can only be
    /// refused again. Attack orders are deliberately excluded: a fresh combat
    /// frame may legitimately finish the same target.
    fn suppress_same_turn_replays(
        &mut self,
        orders: &mut Vec<Order>,
        state: &civvis::mirror::StateSnapshot,
    ) -> usize {
        if self.same_turn != Some(state.turn) {
            self.same_turn = Some(state.turn);
            self.same_turn_orders.clear();
            self.same_turn_deal_orders.clear();
        }
        let frame = state.frame;
        let mut dropped = 0;
        orders.retain(|order| {
            let Some(unit) = order.subject else {
                return true;
            };
            let Some(verb) = order.verb.as_deref() else {
                return true;
            };
            if order.kind == "unit" && matches!(verb, "MOVE_TO" | "CAPTURE" | "FORTIFY") {
                let key = HostUnitOrderKey {
                    verb: verb.to_string(),
                    pos: order.pos,
                };
                let seen = self.same_turn_orders.entry(unit).or_default();
                let duplicate = !seen.insert(key);
                if frame > 0 && duplicate {
                    dropped += 1;
                    return false;
                }
            }

            if matches!(order.kind, "sell" | "buy") {
                let key = HostDealOrderKey {
                    kind: order.kind,
                    subject: order.subject,
                    verb: order.verb.clone(),
                    pos: order.pos,
                };
                let duplicate = !self.same_turn_deal_orders.insert(key);
                if frame > 0 && duplicate {
                    dropped += 1;
                    return false;
                }
            }
            true
        });
        dropped
    }

    /// Compare this turn's positions with last turn's orders.
    fn observe(&mut self, state: &civvis::mirror::StateSnapshot) {
        let sent = std::mem::take(&mut self.sent);
        for (unit, attempt) in sent {
            // The ladder asks several times per turn; a same-turn frame cannot
            // judge last frame's move yet, so keep it for the turn that can.
            if state.turn <= attempt.turn {
                self.sent.insert(unit, attempt);
                continue;
            }
            if state.turn != attempt.turn.saturating_add(1) {
                self.pending_probes.remove(&unit);
                continue;
            }
            let Some(now) = state.units.iter().find(|u| u.id == unit) else {
                self.pending_probes.remove(&unit);
                continue;
            };
            if (now.x, now.y) != attempt.from || attempt.from == attempt.destination {
                self.pending_probes.remove(&unit);
            } else if attempt.destination == attempt.frontier_step {
                // This was already the exact local step, rather than a long
                // route containing it. Now the host's non-movement is proof.
                self.pending_probes.remove(&unit);
                self.dead.insert(attempt.frontier_step);
            } else {
                self.pending_probes.insert(
                    unit,
                    HostFrontierProbe {
                        from: attempt.from,
                        target: attempt.frontier_step,
                    },
                );
            }
        }
    }

    /// Replace a repeated long move with the one unknown hop the host just
    /// failed to begin. This keeps healthy paths coalesced at full speed and
    /// gathers exact terrain evidence only on a failed route.
    fn cap_pending_frontier_moves(
        &mut self,
        orders: &mut [Order],
        state: &civvis::mirror::StateSnapshot,
        first_unknown_steps: &std::collections::BTreeMap<i64, (i32, i32)>,
    ) -> usize {
        let pending = std::mem::take(&mut self.pending_probes);
        let mut capped = 0;
        for (unit, probe) in pending {
            let still_at_from = state
                .units
                .iter()
                .find(|candidate| candidate.id == unit)
                .is_some_and(|candidate| (candidate.x, candidate.y) == probe.from);
            if !still_at_from {
                continue;
            }
            // A replan that chose a different opening is already escaping the
            // failed route. Do not overwrite its tactical judgment with stale
            // exploration feedback.
            if first_unknown_steps.get(&unit) != Some(&probe.target) {
                continue;
            }
            if let Some(order) = orders.iter_mut().find(|order| {
                order.kind == "unit"
                    && order.subject == Some(unit)
                    && order.verb.as_deref() == Some("MOVE_TO")
                    && order.pos.is_some()
            }) {
                if order.pos != Some(probe.target) {
                    order.pos = Some(probe.target);
                    capped += 1;
                }
                self.pending_probes.insert(unit, probe);
            }
        }
        capped
    }

    /// Remember where each `MOVE_TO` in `orders` sends which host unit from.
    ///
    /// `frontier_steps` comes from the uncoalesced action log. It identifies the
    /// first unknown hop along an otherwise long host route; a one-step or fully
    /// revealed route falls back to its final destination, preserving the ordinary
    /// refusal behaviour.
    fn record(
        &mut self,
        orders: &[Order],
        state: &civvis::mirror::StateSnapshot,
        frontier_steps: &std::collections::BTreeMap<i64, (i32, i32)>,
    ) {
        // A sequenced turn can carry a second MOVE_TO for the same unit, planned
        // from where an earlier act leaves it. Only the FIRST is judged against
        // the unit's exported position next turn; the second would read a unit
        // that walked, struck and walked on as "never moved".
        let mut recorded = std::collections::BTreeSet::new();
        for order in orders {
            if order.kind != "unit" || order.verb.as_deref() != Some("MOVE_TO") {
                continue;
            }
            let (Some(unit), Some(dest)) = (order.subject, order.pos) else {
                continue;
            };
            if !recorded.insert(unit) {
                continue;
            }
            let Some(from) = state
                .units
                .iter()
                .find(|u| u.id == unit)
                .map(|u| (u.x, u.y))
            else {
                continue;
            };
            let frontier_step = self
                .pending_probes
                .get(&unit)
                .filter(|probe| probe.from == from && probe.target == dest)
                .map(|probe| probe.target)
                .or_else(|| frontier_steps.get(&unit).copied())
                .unwrap_or(dest);
            self.sent.insert(
                unit,
                HostMoveAttempt {
                    from,
                    destination: dest,
                    frontier_step,
                    turn: state.turn,
                },
            );
        }
    }

    /// Take the proved-unwalkable plots off the mirror's speculative frontier —
    /// both domains' priors, since the refusal came from whichever unit was sent
    /// and the plot is dead for the sea's scout as much as the land's.
    fn apply(&self, mirror: &mut civvis::mirror::LiveMirror) {
        for &(x, y) in &self.dead {
            let pos = civvis::hex::offset_to_axial(x, y);
            if let Some(tile) = mirror.game.map.tiles.get_mut(&pos) {
                if tile.terrain == "unknown" {
                    tile.assumed_traversable = false;
                    tile.assumed_navigable = false;
                }
            }
        }
    }
}

/// Withhold only peace orders whose known Firaxis cooldown has not expired.
///
/// This is deliberately below translation, so native `MakePeace`, negotiated
/// peace deals, and the report-only mirrored peace path all obey the same host
/// contract. Other diplomacy stays untouched.
fn defer_host_peace_retries(
    orders: Vec<Order>,
    state: &civvis::mirror::StateSnapshot,
    retries: &mut HostPeaceRetries,
) -> (Vec<Order>, Vec<i64>) {
    retries.observe(state);
    let mut allowed = Vec::with_capacity(orders.len());
    let mut deferred = Vec::new();
    for order in orders {
        let Some(target) = (order.kind == "peace").then_some(order.subject).flatten() else {
            allowed.push(order);
            continue;
        };
        if retries.permits(target, state.turn) {
            retries.record(target, state.turn);
            allowed.push(order);
        } else {
            deferred.push(target);
        }
    }
    (allowed, deferred)
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
fn coalesce_unit_paths(orders: Vec<Order>, sequenced: bool) -> (Vec<Order>, usize, usize) {
    // Per unit: where its kept order sits in `out`, and whether that order is still
    // an open walk (every order for the unit so far has been a MOVE_TO).
    let mut kept: std::collections::BTreeMap<i64, (usize, bool)> =
        std::collections::BTreeMap::new();
    let mut out: Vec<Order> = Vec::with_capacity(orders.len());
    let mut deferred = 0;
    let mut coalesced = 0;
    for order in orders {
        if order.kind != "unit" {
            out.push(order);
            continue;
        }
        let Some(subject) = order.subject else {
            out.push(order);
            continue;
        };
        let is_step = order.verb.as_deref() == Some("MOVE_TO") && order.pos.is_some();
        match kept.get_mut(&subject) {
            None => {
                kept.insert(subject, (out.len(), is_step));
                out.push(order);
            }
            Some((index, open)) => {
                if *open && is_step {
                    // The walk continues: the host only needs its last hex.
                    out[*index].pos = order.pos;
                    coalesced += 1;
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
            "" => {}
            other => {
                return Err(format!(
                    "--assume-seat {other}: unknown capability (order_queue, \
                     moves_at_turn_start, replan_frames, tile_delta)"
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
/// Major civilizations occupy compact seats in the exact order of `state.rivals`,
/// but city-states live in the mirror's later minor seats and are only exported
/// through `state.minors`.  Resolve those through an exact city plot rather than
/// assuming a generated city-state roster has the host's player numbering.
fn host_player_target(
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    player: usize,
) -> Option<i64> {
    state
        .rivals
        .get(player.saturating_sub(1))
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
// One ask per cadence window, aimed at the seat sealing the most ground; the
// mod's own cooldowns (`TradeRetryTurns`) meter re-asks after a refusal, and
// the export's `open_borders` flag retires the trigger the moment the grant
// exists — the mirror stops sealing that rival, `sealed_border_owners` drops
// them, and this lane moves to the next-worst seal or goes quiet.
const BORDER_BUY_MIN_SEALED: u32 = 6;
const BORDER_BUY_GOLD_RESERVE: i64 = 60;
const BORDER_BUY_CEILING_MIN: i32 = 30;
const BORDER_BUY_CEILING_MAX: i32 = 180;
const BORDER_BUY_CADENCE: u32 = 6;
const BORDER_BUY_PHASE: u32 = 1;

/// Why no passage-purchase order was appended this turn, for the note;
/// `None` when one was. `sealed_by` is the mirrored board's
/// `sealed_border_owners`; rival records are looked up by the mirror's own
/// seating rule (majors take seats 1.. in export order, the same mapping
/// `host_player_target` uses).
fn append_border_buy_order(
    sealed_by: &std::collections::BTreeMap<usize, u32>,
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<Order>,
    passage_value: &dyn Fn(usize) -> f64,
) -> Option<&'static str> {
    let worst = sealed_by
        .iter()
        .filter(|(_, count)| **count >= BORDER_BUY_MIN_SEALED)
        .filter_map(|(seat, count)| {
            state
                .rivals
                .get(seat.saturating_sub(1))
                .map(|rival| (*seat, rival, *count))
        })
        .filter(|(_, rival, _)| !rival.at_war && rival.open_borders != Some(true))
        .max_by_key(|(_, rival, count)| (*count, std::cmp::Reverse(rival.player)));
    let Some((seat, rival, _)) = worst else {
        return Some("border_buy_hold:no_seal");
    };
    if state.turn % BORDER_BUY_CADENCE != BORDER_BUY_PHASE {
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
// but the mirror carries no rival's tradeable stock and `border_buy_ceiling`
// rightly refuses the shape, so — like the favor sale and the passage
// purchase — the order is composed here from the export's own facts: the
// amenity ledger the host reports per city, the net income, the treasury,
// the rivals' war state and gold. The Lua arm reads the RIVAL's own
// tradeable list, takes the first luxury the host says the seat holds none
// of, asks the rival's own price with EQUALIZE and closes only at or under
// the ceiling carried in `x`.
//
// Cadence-gated like its neighbours (phase 4, the last free slot of the
// six-turn deal week), aimed at the richest met major at peace with no deal
// already heading its way this turn; the mod's own cooldowns
// (`TradeRetryTurns`) meter re-asks after a `buy_no_luxury`. The ceiling is
// what a copy is worth to the empire — a base and a share per city, since
// the four cities it reaches each recover the 10% — bounded by what the
// income carries at the 25× book with `LUXURY_BUY_INCOME_FLOOR` Gold a turn
// kept clear (every recorded ask was per-turn), and by the cap. A ceiling
// under `LUXURY_BUY_CEILING_MIN` — two Gold a turn, the cheapest ask on
// record — is not worth a deal window.
const LUXURY_BUY_MIN_DEFICIT: f64 = 2.0;
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
    if amenity_deficit(state) < LUXURY_BUY_MIN_DEFICIT {
        return Some("luxury_buy_hold:content");
    }
    if state.turn % LUXURY_BUY_CADENCE != LUXURY_BUY_PHASE {
        return Some("luxury_buy_hold:cadence");
    }
    let Some(income) = state.gold_per_turn.filter(|income| income.is_finite()) else {
        return Some("luxury_buy_hold:income_unknown");
    };
    let carried = (25.0 * (income - LUXURY_BUY_INCOME_FLOOR))
        .floor()
        .clamp(0.0, LUXURY_BUY_CEILING_MAX as f64) as i32;
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
        return Some("luxury_buy_hold:no_seller");
    };
    orders.push(Order {
        kind: "buy",
        subject: Some(seller.player as i64),
        verb: Some("LUXURY_ANY".to_string()),
        pos: Some((ceiling, 0)),
    });
    None
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

/// Drop the planner's own favor sales when the plan means to cast that favor.
/// `Game::quick_deals` quotes a ten-favor block to any partner two DVP richer,
/// plan-blind; on a Diplomacy plan those ten points are two votes at the next
/// session. Returns how many orders were held back.
fn hold_planner_favor_sales(plan: Option<(&str, Option<&str>)>, orders: &mut Vec<Order>) -> usize {
    if !plan_keeps_favor(plan) {
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
    if state.turn % FAVOR_SALE_CADENCE != FAVOR_SALE_PHASE {
        return Some("favor_hold:cadence");
    }
    let Some(favor) = state.favor.filter(|favor| favor.is_finite()) else {
        return Some("favor_hold:unknown");
    };
    let surplus = favor - FAVOR_RESERVE;
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
const AID_GIFT_FINISH_WINDOW: i64 = 3;
const AID_GIFT_GOLD_RESERVE: i64 = 100;
const AID_GIFT_SCORE_MAX: i64 = 200;

fn aid_request_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "EMERGENCY_SEND_AID" | "EMERGENCY_SEND_MILITARY_AID"
    )
}

/// The share of the treasury a ROUTED peace offer may carry as tribute on
/// its retry, and the most it may ever carry. ★ 2026-08-24: over 42 live
/// runs the seat paid 142 tributes at a median 116 Gold (max 783, ~21,000
/// Gold in all) because the control mod put three quarters of the treasury
/// on every retried offer, whatever the planner's reason for offering — a
/// second front, war fatigue, a religious lane and an envoy reclaim all
/// paid like a rout. Civilization VI's own database carries no diplomatic
/// modifier for a gift, so Gold that does not buy the peace buys nothing.
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

/// The smallest integral gift that puts our tracker score strictly above every
/// finite score Firaxis currently reports.  `None` says the host has not given
/// an authoritative enough board, or the gap exceeds the bounded fallback.
fn aid_gift_needed_score(emergency: &civvis::mirror::StateEmergency) -> Option<i64> {
    let ours = emergency.ours.score.filter(|score| score.is_finite())?;
    let leader = emergency
        .scores
        .iter()
        .map(|entry| entry.score)
        .filter(|score| score.is_finite())
        .fold(ours, f64::max);
    let needed = (leader - ours).floor() + 1.0;
    if !needed.is_finite() || needed < 1.0 || needed > AID_GIFT_SCORE_MAX as f64 {
        return None;
    }
    Some(needed as i64)
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
    let spendable = state.gold.saturating_sub(AID_GIFT_GOLD_RESERVE);
    let candidate = emergencies
        .iter()
        .filter(|emergency| {
            aid_request_kind(&emergency.kind)
                && emergency.begun
                && emergency.turns_left >= 0
                && emergency.turns_left <= AID_GIFT_FINISH_WINDOW
                && emergency.ours.member
                && emergency.target >= 0
                && !aid_project_finishes_by_deadline(state, emergency)
        })
        .filter_map(|emergency| {
            let amount = aid_gift_needed_score(emergency)?;
            if amount > spendable {
                return None;
            }
            let target_is_peaceful_met_major = state
                .rivals
                .iter()
                .any(|rival| rival.player as i64 == emergency.target && !rival.at_war);
            target_is_peaceful_met_major.then_some((emergency, amount))
        })
        // If rare overlapping Aid Requests are both finishable, bank the
        // earliest deadline; the lower exact spend is the deterministic tie
        // breaker, then the actual host recipient id.
        .min_by_key(|(emergency, amount)| (emergency.turns_left, *amount, emergency.target));
    let Some((emergency, amount)) = candidate else {
        return Some("aid_gift_hold:no_finishable_lead");
    };
    let target = emergency.target;
    if orders.iter().any(|order| {
        order.subject == Some(target)
            && matches!(order.kind, "war" | "peace" | "delegation" | "denounce")
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
}

impl GreatPersonStall {
    fn total(self) -> usize {
        self.on_cooldown + self.no_activation_plot + self.no_empty_slot
    }
}

fn great_person_orders(state: &civvis::mirror::StateSnapshot) -> (Vec<Order>, GreatPersonStall) {
    let mut orders = Vec::new();
    let mut stall = GreatPersonStall::default();
    for unit in &state.units {
        let Some(person) = &unit.great_person else {
            continue;
        };
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

/// Keep every physical Great Person hex clear for the current Lua batch.
///
/// Firaxis queues unit operations. `CanStartCommand` can therefore approve a Great
/// Person retirement and a later move onto that person's hex even though the move
/// will make the retirement illegal by the time the engine resolves it. Hanno the
/// Navigator hit this exact race three times while a Galley oscillated through his
/// Harbor. An inactive Great Person also blocks another civilian from entering its
/// plot: Hildegard spent forty turns trying to enter a Holy Site occupied by an
/// unspent Prophet. Reserve every observed Great Person hex and reconsider those
/// moves after the next authoritative export.
fn defer_great_person_plot_conflicts(
    orders: Vec<Order>,
    state: &civvis::mirror::StateSnapshot,
) -> (Vec<Order>, usize) {
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
        .map(|unit| (unit.id, (unit.x, unit.y)))
        .collect::<Vec<_>>();
    let mut deferred = 0;
    let orders = orders
        .into_iter()
        .filter(|order| {
            // A Trade Route uses `pos` to name its destination city, not a
            // physical hex the Trader will enter in this Lua batch.
            let conflicts = order.kind == "unit"
                && order.verb.as_deref() != Some("TRADE_ROUTE")
                && people
                    .iter()
                    .any(|(person, pos)| order.subject != Some(*person) && order.pos == Some(*pos));
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
        {
            continue;
        }
        let Some(cid) = cid_of.get(&city.id).copied() else {
            continue;
        };
        if let Some(built) = planned_game.cities.get_mut(&cid) {
            if !built.queue.is_empty() {
                built.queue.clear();
                released += 1;
            }
        }
    }
    released
}

#[derive(Default)]
struct WarFinishingVolley {
    /// Wounded military enemies at war that the exact planning model removed
    /// before ordinary movement.
    targets: usize,
    /// Direct attacks to send from the exported frame, including one reserve
    /// attacker when Firaxis may leave a modelled kill alive.
    actions: Vec<Action>,
    reserves: usize,
}

#[derive(Clone)]
struct FinishingCandidate {
    unit: u32,
    /// Exact CIVVIS actions used to prove and score the line.
    simulation: Vec<Action>,
    /// One order the host can execute from its exported frame. A melee order
    /// may collapse approach moves plus the final attack into MOVE_TO(enemy).
    order: Action,
    kills: bool,
    ranged: bool,
    damage: i32,
    attacker_loss: i32,
}

/// Legal attacks a mapped unit can make on `target` from the exported frame.
///
/// Geometry alone is not enough here: line of sight, siege setup, remaining
/// attacks and promotions can all make an apparently direct shot illegal. Each
/// candidate therefore has to survive the engine's own `apply` on a private
/// board before it can pre-empt the unit's ordinary movement order.
fn live_finishing_candidates(
    game: &civvis::game::Game,
    pid: usize,
    target: u32,
    mapped: &std::collections::BTreeMap<u32, i64>,
    committed: &std::collections::BTreeSet<u32>,
) -> Vec<FinishingCandidate> {
    let Some(defender) = game.units.get(&target) else {
        return Vec::new();
    };
    let target_pos = defender.pos;
    let target_hp = defender.hp;
    let mut candidates = Vec::new();

    for unit in game.player_unit_ids(pid) {
        if committed.contains(&unit) || !mapped.contains_key(&unit) {
            continue;
        }
        let friendly = &game.units[&unit];
        let spec = &game.rules.units[friendly.kind];
        if spec.class != "military"
            || friendly.moves_left <= 0.0
            || friendly.attacks_left <= 0
            || friendly
                .linked_to
                .and_then(|peer| game.units.get(&peer))
                .is_some_and(|peer| game.rules.units[peer.kind].class != "military")
        {
            continue;
        }
        let distance = game.wdist(friendly.pos, target_pos);
        let mut modes = Vec::with_capacity(3);
        if spec.has_ranged_attack() && distance <= game.unit_attack_range(unit) {
            let order = Action::Ranged {
                unit,
                target: target_pos,
            };
            modes.push((true, vec![order.clone()], order));
        }
        if spec.is_melee_capable() && distance == 1 {
            let order = Action::Attack {
                unit,
                target: target_pos,
            };
            modes.push((false, vec![order.clone()], order));
        } else if spec.is_melee_capable() && distance > 1 {
            // Civ VI resolves melee through MOVE_TO on the occupied tile, so
            // one host order can cover an approach and its blow. Prove the
            // corresponding line with CIVVIS's own pathfinder first; a route
            // stopped by terrain, stacking, ZOC or exhausted movement never
            // becomes a host order.
            let mut approach = game.clone();
            let mut simulation = Vec::new();
            for _ in 0..game.map.tiles.len() {
                if approach.wdist(approach.units[&unit].pos, target_pos) <= 1 {
                    break;
                }
                let Some(next) = approach.route_step(unit, target_pos, 1) else {
                    break;
                };
                let step = Action::Move { unit, to: next };
                if approach.apply(pid, &step).is_err() {
                    break;
                }
                simulation.push(step);
            }
            if approach.wdist(approach.units[&unit].pos, target_pos) == 1 {
                let attack = Action::Attack {
                    unit,
                    target: target_pos,
                };
                if approach.apply(pid, &attack).is_ok() {
                    simulation.push(attack);
                    modes.push((
                        false,
                        simulation,
                        Action::Attack {
                            unit,
                            target: target_pos,
                        },
                    ));
                }
            }
        }

        for (ranged, simulation, order) in modes {
            let attacker_hp = friendly.hp;
            let mut after = game.clone();
            let mut legal = true;
            for action in &simulation {
                if after.apply(pid, action).is_err() {
                    legal = false;
                    break;
                }
            }
            if !legal || !after.units.contains_key(&unit) {
                continue;
            }
            let remaining = after.units.get(&target).map(|unit| unit.hp).unwrap_or(0);
            let damage = (target_hp - remaining).max(0);
            if damage == 0 {
                continue;
            }
            let attacker_loss = attacker_hp - after.units[&unit].hp;
            candidates.push(FinishingCandidate {
                unit,
                simulation,
                order,
                kills: !after.units.contains_key(&target),
                ranged,
                damage,
                attacker_loss,
            });
        }
    }

    // A certain kill first; otherwise make the shortest volley. Ranged fire
    // breaks a tie without risking a melee body, then damage and retaliation
    // distinguish otherwise equivalent blows.
    candidates.sort_by(|left, right| {
        right
            .kills
            .cmp(&left.kills)
            .then_with(|| right.ranged.cmp(&left.ranged))
            .then_with(|| right.damage.cmp(&left.damage))
            .then_with(|| left.attacker_loss.cmp(&right.attacker_loss))
            .then_with(|| left.unit.cmp(&right.unit))
    });
    // A unit with both attack modes still contributes only one seat to the
    // volley. The preferred mode is first after the ordering above.
    let mut seen = std::collections::BTreeSet::new();
    candidates.retain(|candidate| seen.insert(candidate.unit));
    candidates
}

/// Finish exposed wounded enemies before their attackers receive campaign moves.
///
/// The old live-only repair rewrote every wounded barbarian to 100 HP on the
/// planning board. That kept a second defender from being released after a
/// simulated kill, but it also erased the decisive tactical fact: this unit is
/// one blow from death. The active run `civvis-20260815T095258Z` then showed
/// barbarians survive successive exported frames on 1, 3, 6, 8, 16 and 20 HP
/// while nearby units promoted or marched. The reserve was firing throughout.
///
/// Keep the exported HP. For every wounded military unit belonging to a player
/// we are currently at war with, prove a kill using only attacks legal *right
/// now*, apply that shortest volley before the normal AI turn, and reserve one
/// additional direct attacker when one is available. This covers barbarians,
/// rivals, city-states, and Free Cities without treating a visible peacetime
/// unit as a target. Damage-only opportunities remain the normal tactical
/// planner's decision.
///
/// The extra order preserves the host/model safety margin: if Firaxis leaves a
/// predicted kill alive it gets the second blow; if the first blow killed it, a
/// ranged order is refused and a melee order can only enter the cleared tile or
/// be blocked by the first attacker.
#[cfg(test)]
fn finish_live_war_units(
    planned_game: &mut civvis::game::Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
) -> WarFinishingVolley {
    finish_live_war_units_excluding(
        planned_game,
        pid,
        mapped,
        &std::collections::BTreeSet::new(),
    )
}

/// [`finish_live_war_units`] with `excluded` units left out of the volley
/// entirely — a settler's bound guard under
/// `AdvancedAi::settler_stack_discipline`, which stays out of this pre-pass
/// for the same reason.
fn finish_live_war_units_excluding(
    planned_game: &mut civvis::game::Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
    excluded: &std::collections::BTreeSet<u32>,
) -> WarFinishingVolley {
    let mapped: std::collections::BTreeMap<u32, i64> = mapped
        .iter()
        .filter(|(uid, _)| !excluded.contains(uid))
        .map(|(uid, civ6)| (*uid, *civ6))
        .collect();
    let mapped = &mapped;
    let mut targets = planned_game
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && planned_game.is_at_war(pid, unit.owner)
                && planned_game.rules.units[unit.kind].class == "military"
                && unit.hp < 100
        })
        .map(|unit| (unit.hp, unit.pos, unit.id))
        .collect::<Vec<_>>();
    targets.sort_unstable();

    let mut result = WarFinishingVolley::default();
    let mut committed = std::collections::BTreeSet::new();
    for (_, _, target) in targets {
        if !planned_game.units.contains_key(&target) {
            continue;
        }
        let initial = live_finishing_candidates(planned_game, pid, target, mapped, &committed);
        if initial.is_empty() {
            continue;
        }

        // Prove that a direct volley actually removes the target before taking
        // any unit away from the ordinary AI. Damage-only opportunities remain
        // the ordinary tactical path's decision.
        let mut proof = planned_game.clone();
        let mut proof_committed = committed.clone();
        let mut chosen = Vec::new();
        while proof.units.contains_key(&target) {
            let Some(candidate) =
                live_finishing_candidates(&proof, pid, target, mapped, &proof_committed)
                    .into_iter()
                    .next()
            else {
                break;
            };
            let mut legal = true;
            for action in &candidate.simulation {
                if proof.apply(pid, action).is_err() {
                    legal = false;
                    break;
                }
            }
            if !legal {
                break;
            }
            proof_committed.insert(candidate.unit);
            chosen.push(candidate);
        }
        if proof.units.contains_key(&target) {
            continue;
        }

        for candidate in &chosen {
            let mut legal = true;
            for action in &candidate.simulation {
                if planned_game.apply(pid, action).is_err() {
                    legal = false;
                    break;
                }
            }
            if !legal {
                break;
            }
            committed.insert(candidate.unit);
            result.actions.push(candidate.order.clone());
        }
        if planned_game.units.contains_key(&target) {
            continue;
        }
        result.targets += 1;

        // One predicted attack plus one available attacker is the exact shape
        // the old durability rewrite was trying to preserve. Keep the reserve
        // explicit instead of making the target look healthy to every scorer.
        if chosen.len() == 1 {
            if let Some(backup) = initial
                .into_iter()
                .find(|candidate| !committed.contains(&candidate.unit))
            {
                if let Some(unit) = planned_game.units.get_mut(&backup.unit) {
                    unit.moves_left = 0.0;
                    unit.attacks_left = 0;
                }
                committed.insert(backup.unit);
                result.actions.push(backup.order);
                result.reserves += 1;
            }
        }
    }
    result
}

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
        .find(|gene| gene.live() && gene.tag == treatment)
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

/// Every treatment `--without` accepts, in table order, for the usage line.
fn withholdable_treatments() -> String {
    civvis::ai::GENES
        .iter()
        .filter(|gene| gene.live())
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
fn forced_live_treatments(forced: &[String]) -> Result<Vec<&'static str>, String> {
    let mut selected = Vec::new();
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
                return Err(format!(
                    "--with treatment {treatment:?} already ships in the deployment genome \
                     (tag {tag:?}); only a ledger-held live treatment or a held-off opt-in \
                     forms a distinct arm"
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
    Ok(selected)
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
    let (production_hints_consumed, production_hints_expired) =
        settle_deferred_production_hints(ours, state);
    // `Ai::take_turn` is a full CIVVIS turn simulation: it changes queues, spends
    // resources, ends the turn, and can complete a queued unit.  None of those
    // mutations happened in Firaxis merely because we asked for a recommendation.
    // Keep the authoritative mirror as the last exported Civ VI state and plan on a
    // throwaway clone instead.  Apart from preventing phantom units, this means the
    // board shown to the next decision is never a mixture of one real game and one
    // speculative CIVVIS turn.
    let mut planned_game = mirror_state.game.clone();
    // Firaxis keeps a Trader visible while it is travelling an active route;
    // CIVVIS's native model consumes it into `game.routes`.  The authoritative
    // mirror carries both so the map remains faithful.  Remove only the busy
    // trader from this throwaway planning board, otherwise a spare-capacity turn
    // can send the same real unit a second `TRADE_ROUTE` request.
    remove_active_route_traders_from_plan(&mut planned_game, mirror_state);
    let released = release_foreign_production(&mut planned_game, &mirror_state.cid_of, state, ours);
    let before = planned_game.log.len();
    // ★★★★ BEFORE THE VOLLEY. See `AdvancedAi::observe_turn_start_hostiles`:
    // the volley below removes wounded raiders from the planning board, and
    // a settler stepping afterwards must keep pricing them — the host has
    // been measured leaving such kills alive. And a settler's bound guard is
    // not the volley's to spend one tile away from the civilian it shields.
    ai.observe_turn_start_hostiles(&planned_game, 0);
    let bound_guards = ai.bound_settler_guards(&planned_game, 0);
    let war_finishers =
        finish_live_war_units_excluding(&mut planned_game, 0, &mirror_state.civ6_of, &bound_guards);
    // Finishing attacks are translated explicitly below, including the reserve
    // order that was intentionally not applied to the planning board. Ordinary
    // AI actions start after the attacks that were applied there.
    let ai_actions_begin = planned_game.log.len();
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
    ai.take_turn(&mut planned_game, 0);

    let mut orders: Vec<Order> = war_finishers
        .actions
        .iter()
        .filter_map(|action| translate(action, mirror_state, state))
        .collect();
    let mut skipped: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut skipped_examples: Vec<String> = Vec::new();
    let mut note_bits: Vec<String> = Vec::new();
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
                    | Action::FoundCity { unit }
                    | Action::Fortify { unit }
                    | Action::Improve { unit, .. }
                    | Action::UpgradeUnit { unit } => {
                        if rebuilt_unit_missing(mirror_state, *unit) {
                            "unit_not_mapped"
                        } else {
                            "unit_action_untranslated"
                        }
                    }
                    Action::Produce { .. } => "produce_not_mapped",
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
        orders.push(policy_deck_order(&planned_game, 0));
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
    let held_favor_sales = hold_planner_favor_sales(plan_facts, &mut orders);
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
            if why == "favor_hold:no_buyer" {
                note_bits.push(why.to_string());
            }
        }
    }
    let passage_value = |_seat: usize| mirror_state.game.passage_gold_value(0);
    match append_border_buy_order(
        &mirror_state.game.sealed_border_owners,
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
    match append_luxury_buy_order(state, &mut orders) {
        None => note_bits.push("luxury_buy=1".to_string()),
        Some(why) => {
            // The holds worth a glance: an amenity deficit standing on the
            // lane's own turn while the income or the treasury cannot carry
            // a copy, or with nobody at peace to buy one from.
            if why == "luxury_buy_hold:income"
                || why == "luxury_buy_hold:treasury"
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
    let sales = orders.iter().filter(|order| order.kind == "sell").count();
    if sales > 0 {
        note_bits.push(format!("sales={sales}"));
    }

    let (person_orders, great_person_stall) = great_person_orders(state);
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
                if let Some(rival) = state.rivals.get(seat.saturating_sub(1)) {
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
            if let Some(rival) = state.rivals.get(seat.saturating_sub(1)) {
                let subject = rival.player as i64;
                let already = orders
                    .iter()
                    .any(|o| o.kind == "peace" && o.subject == Some(subject));
                if rival.at_war && !already {
                    let routed = report.peace_routed.contains(seat);
                    let cap = peace_tribute_cap(state, routed);
                    orders.push(Order {
                        kind: "peace",
                        subject: Some(subject),
                        verb: Some("MAKE_PEACE".to_string()),
                        pos: Some((cap, 0)),
                    });
                    note_bits.push(format!("peace_from_plan={}", rival.player));
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

    let (host_legal, deferred_peace_retries) =
        defer_host_peace_retries(orders, state, host_peace_retries);
    orders = host_legal;
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

    let (activation_safe, deferred_activation_plot_conflicts) =
        defer_great_person_plot_conflicts(orders, state);
    orders = activation_safe;
    if deferred_activation_plot_conflicts > 0 {
        note_bits.push(format!(
            "deferred_activation_plot_conflicts={deferred_activation_plot_conflicts}"
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
    let (causally_safe, deferred_unit_followups, coalesced_path_steps) =
        coalesce_unit_paths(orders, sequenced);
    orders = causally_safe;
    if coalesced_path_steps > 0 {
        note_bits.push(format!("coalesced_path_steps={coalesced_path_steps}"));
    }
    if deferred_unit_followups > 0 {
        note_bits.push(format!("deferred_unit_followups={deferred_unit_followups}"));
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
    // real change, while an identical stale MOVE_TO/CAPTURE/FORTIFY is not sent
    // twice to the host. Strike retries stay enabled; see the method comment.
    let same_turn_replays = host_move_refusals.suppress_same_turn_replays(&mut orders, state);
    if same_turn_replays > 0 {
        note_bits.push(format!("same_turn_replays={same_turn_replays}"));
    }

    // Remember where each move sends which host unit, so next turn's positions
    // can prove a destination unwalkable. See `HostMoveRefusals`.
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

    let body = orders
        .iter()
        .map(|o| o.to_json())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"turn\":{},\"orders\":[{}],\"note\":{}}}",
        state.turn,
        body,
        quote(&note_bits.join("; "))
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
        Action::BuyPlot { city, pos, .. } => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: "buy_plot",
                subject: Some(*civ6),
                verb: None,
                pos: Some(civvis::hex::axial_to_offset(pos.0, pos.1)),
            }),
        Action::Improve { unit, improvement } => civ6_of
            .get(unit)
            .map(|civ6| Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("IMPROVE".to_string()),
                pos: None,
            })
            .map(|mut order| {
                // The improvement name rides in `verb` alongside the operation, because the
                // order row has no spare column; the mod splits them.
                order.verb = Some(format!("IMPROVE:{}", civ6_improvement_type(improvement)));
                order
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
        Action::UpgradeUnit { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("UPGRADE".to_string()),
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
        // ⚠⚠ A CASUS-BELLI WAR IS STILL A WAR, AND THIS DROPPED IT ON THE FLOOR.
        // CIVVIS prefers `DeclareWarWithCasusBelli` for a major rival and keeps
        // surprise war for minors, so this variant is the one it would actually emit
        // against the civilizations domination needs — and it was falling through to
        // the `other` tally, counted as untranslatable. Civilization VI has one war
        // declaration; the grievance bookkeeping is a CIVVIS rule with no counterpart,
        // so the casus belli is dropped and the war is kept.
        Action::DeclareWarWithCasusBelli { player, .. } | Action::DeclareWar { player, .. } => {
            Some(Order {
                kind: "war",
                subject: host_player_target(mirror_state, state, *player),
                verb: Some("DECLARE".to_string()),
                pos: None,
            })
        }
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
        // a non-peace deal (open borders, friendship, gold) still has no
        // counterpart and stays skipped.
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
        _ => None,
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
];

/// Unit verbs the next frame cannot answer, each with the reason. `SPY_*`
/// covers every espionage mission: they run for many turns and the frame
/// carries no mission state.
const UNVERIFIABLE_UNIT_VERBS: &[(&str, &str)] = &[
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
];

fn unverifiable_kind(kind: &str) -> bool {
    UNVERIFIABLE_ORDER_KINDS.iter().any(|(k, _)| *k == kind)
}

fn unverifiable_unit_verb(op: &str) -> bool {
    op.starts_with("SPY_") || UNVERIFIABLE_UNIT_VERBS.iter().any(|(v, _)| *v == op)
}

/// Ledger event kinds the checks read as evidence between two frames.
const EVIDENCE_KINDS: &[&str] = &[
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (Vec::new(), Vec::new());
    };
    let mut evidence = Vec::new();
    let mut states = Vec::new();
    for line in raw.lines() {
        if !line.contains("\"state\"")
            && !EVIDENCE_KINDS
                .iter()
                .any(|kind| line.contains(&format!("\"{kind}\"")))
        {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let turn = event.get("turn").and_then(|turn| turn.as_u64());
        if !turn.is_some_and(|turn| turns.iter().any(|want| u64::from(*want) == turn)) {
            continue;
        }
        let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind == "state" {
            if let Ok(state) = serde_json::from_value(event) {
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

/// Whether whatever stood on `pos` before the order is damaged, gone, or ours.
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
    let event = evidence.iter().find(|event| {
        event.get("kind").and_then(|k| k.as_str()) == Some("move_refused")
            && event.get("turn").and_then(|t| t.as_u64()) == Some(u64::from(turn))
            && event.get("unit").and_then(|u| u.as_i64()) == Some(unit)
    })?;
    let named = |key: &str| event.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    Some(if named("dest_impassable") {
        "host_refused_impassable".to_string()
    } else if named("dest_water") {
        "host_refused_water".to_string()
    } else {
        "host_refused_move".to_string()
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

fn combat_evidence(evidence: &[serde_json::Value], turn: u32, attacker: i64) -> bool {
    evidence.iter().any(|event| {
        event.get("kind").and_then(|k| k.as_str()) == Some("combat")
            && event.get("turn").and_then(|t| t.as_u64()) == Some(u64::from(turn))
            && event
                .get("attacker")
                .and_then(|a| a.get("id"))
                .and_then(|id| id.as_i64())
                == Some(attacker)
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
    city.buildings.iter().any(|b| b == item)
        || city.districts.iter().any(|d| d.kind == item)
        || city.wonders.iter().any(|w| w.kind == item)
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
        "ATTACK" | "RANGE_ATTACK" => {
            let harmed = order.pos.is_some_and(|p| target_harmed(before, after, p));
            if harmed || combat_evidence(evidence, turn, id) {
                Verdict::Verified
            } else {
                Verdict::Failed("target_unharmed".to_string())
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
        "IMPROVE" | "REPAIR" => {
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
            if now.is_none() || charges_spent || tile_shows || improved_event {
                Verdict::Verified
            } else if op == "REPAIR" {
                // A repair spends no charge and raises no `improved` event: the
                // tile's pillage bit is its only witness.
                Verdict::Failed("not_repaired".to_string())
            } else {
                Verdict::Failed("no_improvement".to_string())
            }
        }
        "PILLAGE" => {
            let pillaged = was.is_some_and(|u| tiles.plot((u.x, u.y)).is_some_and(|plot| plot.p));
            if pillaged {
                Verdict::Verified
            } else {
                Verdict::Failed("not_pillaged".to_string())
            }
        }
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
        "SPREAD_RELIGION" | "RELIGIOUS_HEAL" | "REMOVE_HERESY" | "LAUNCH_INQUISITION"
        | "CONDEMN_HERETIC" | "CONVERT_BARBARIANS" => {
            let spent = matches!(
                (was.and_then(|u| u.spread_charges), now.and_then(|u| u.spread_charges)),
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

struct VerificationContext<'a> {
    same_turn_orders: &'a [IssuedOrder],
    later_fortified: bool,
    /// The decider moved this unit in a later frame of the same turn, so any
    /// FORTIFY it issued earlier was overridden by its own next decision.
    later_moved: bool,
    /// A later policy-deck replacement in this turn matches the final state,
    /// so this order was superseded by the decider's own re-plan.
    later_policy_deck: bool,
}

/// The next frame's verdict on one order issued on `turn`.
fn verify_order_with_context(
    order: &IssuedOrder,
    turn: u32,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    context: VerificationContext<'_>,
) -> Verdict {
    if unverifiable_kind(&order.kind) {
        return Verdict::Unverifiable;
    }
    let verb = order.verb.as_deref().unwrap_or("");
    let failed = |why: String| Verdict::Failed(why);
    match order.kind.as_str() {
        "unit" => verify_unit_order(
            order,
            turn,
            before,
            after,
            tiles,
            evidence,
            LaterFrames {
                fortified: context.later_fortified,
                moved: context.later_moved,
            },
        ),
        "produce" => {
            let Some(city) = order.subject.and_then(|id| own_city(after, id)) else {
                return failed("city_gone".to_string());
            };
            let unit_appeared = unit_appeared_at_city(before, after, city, verb);
            let purchase_conflict =
                production_unit_appearance_conflicts_with_purchase(order, context.same_turn_orders);
            if city.producing.as_deref() == Some(verb)
                || city_holds(city, verb)
                || (unit_appeared && !purchase_conflict)
            {
                Verdict::Verified
            } else if unit_appeared && purchase_conflict {
                failed("unit_appearance_ambiguous_purchase".to_string())
            } else {
                failed(format!(
                    "producing={}",
                    city.producing.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "purchase" | "purchase_faith" => {
            let Some(city) = order.subject.and_then(|id| own_city(after, id)) else {
                return failed("city_gone".to_string());
            };
            let present =
                city_holds(city, verb) || unit_appeared_at_city(before, after, city, verb);
            let treasury_fell = if order.kind == "purchase_faith" {
                after.faith < before.faith
            } else {
                after.gold < before.gold
            };
            if present || treasury_fell {
                Verdict::Verified
            } else {
                failed("not_bought".to_string())
            }
        }
        "buy_plot" => {
            let Some(pos) = order.pos else {
                return failed("no_plot".to_string());
            };
            match tiles.plot(pos) {
                Some(plot) if plot.o == after.seat.local_player => Verdict::Verified,
                Some(_) => failed("plot_not_owned".to_string()),
                None => failed("plot_unseen".to_string()),
            }
        }
        "envoy" => {
            let envoys = |state: &civvis::mirror::StateSnapshot| {
                state
                    .minors
                    .iter()
                    .find(|m| Some(m.player as i64) == order.subject)
                    .map(|m| m.envoys)
            };
            let rose = matches!((envoys(before), envoys(after)), (Some(b), Some(a)) if a > b);
            let spent =
                matches!((before.envoys_free, after.envoys_free), (Some(b), Some(a)) if a < b);
            if rose || spent {
                Verdict::Verified
            } else {
                failed("envoys_unchanged".to_string())
            }
        }
        "sell" | "buy" => {
            if deal_answered(evidence, order.subject) {
                Verdict::Verified
            } else {
                failed("no_deal_response".to_string())
            }
        }
        "peace" => {
            let war_now = order.subject.and_then(|p| at_war_with(after, p));
            let war_before = order.subject.and_then(|p| at_war_with(before, p));
            match peace_response(evidence, order.subject) {
                Some(true) if war_now == Some(true) => failed("still_at_war".to_string()),
                Some(_) => Verdict::Verified,
                None if war_before == Some(true) && war_now == Some(false) => Verdict::Verified,
                None => failed("no_peace_response".to_string()),
            }
        }
        "war" => match order.subject.and_then(|p| at_war_with(after, p)) {
            Some(true) => Verdict::Verified,
            Some(false) => failed("not_at_war".to_string()),
            None => failed("player_unseen".to_string()),
        },
        // The host's own record of OUR denouncement (`GetDenounceTurn`, the
        // turn it was made, -1 for none) crosses on every rival since #2590;
        // a session asked this turn lands on the next frame with that turn.
        "denounce" => match order.subject.and_then(|p| our_denounce_turn_of(after, p)) {
            Some(Some(since)) if since >= turn as i64 => Verdict::Verified,
            Some(Some(_)) => failed("not_denounced".to_string()),
            Some(None) => Verdict::Unverifiable,
            None => failed("player_unseen".to_string()),
        },
        "governor_appoint" | "governor_assign" => {
            let governors =
                |state: &civvis::mirror::StateSnapshot| state.governors.clone().unwrap_or_default();
            let seated = governors(after)
                .iter()
                .any(|g| g.kind == verb && Some(g.city) == order.subject);
            let appointed = order.kind == "governor_appoint"
                && governors(after).iter().any(|g| g.kind == verb)
                && !governors(before).iter().any(|g| g.kind == verb);
            if seated || appointed {
                Verdict::Verified
            } else if order.kind == "governor_appoint" {
                failed("not_appointed".to_string())
            } else {
                failed("not_assigned".to_string())
            }
        }
        "governor_promote" => {
            let (governor, promotion) = verb.split_once(',').unwrap_or((verb, ""));
            let promoted = after.governors.as_ref().is_some_and(|gs| {
                gs.iter()
                    .any(|g| g.kind == governor && g.promotions.iter().any(|p| p == promotion))
            });
            if promoted {
                Verdict::Verified
            } else {
                failed("not_promoted".to_string())
            }
        }
        "research" => {
            if after.research.as_deref() == Some(verb) || after.techs.iter().any(|t| t == verb) {
                Verdict::Verified
            } else {
                failed(format!(
                    "research={}",
                    after.research.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "civic" => {
            if after.civic.as_deref() == Some(verb) || after.civics.iter().any(|c| c == verb) {
                Verdict::Verified
            } else {
                failed(format!(
                    "civic={}",
                    after.civic.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "policy_deck" => {
            let missing: Vec<&str> = verb
                .split(',')
                .filter(|p| !p.is_empty() && !after.policies.iter().any(|have| have == p))
                .collect();
            if missing.is_empty() {
                Verdict::Verified
            } else if context.later_policy_deck {
                failed("superseded_by_policy_deck".to_string())
            } else {
                failed(format!("missing={}", missing.join("+")))
            }
        }
        "government" => {
            if after.government.as_deref() == Some(verb) {
                Verdict::Verified
            } else {
                failed(format!(
                    "government={}",
                    after.government.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "pantheon" => {
            if after.pantheon.as_deref() == Some(verb) {
                Verdict::Verified
            } else {
                failed("no_pantheon".to_string())
            }
        }
        "dedication" => match after.dedications.as_ref() {
            Some(dedications) if dedications.iter().any(|dedication| dedication == verb) => {
                Verdict::Verified
            }
            Some(_) => failed("not_dedicated".to_string()),
            None => Verdict::Unverifiable,
        },
        "religion" => {
            let beliefs_held = verb
                .split(',')
                .filter(|b| !b.is_empty())
                .all(|b| after.religion_beliefs.iter().any(|have| have == b));
            let founded = after.founded_religion.is_some() && before.founded_religion.is_none();
            if founded || beliefs_held {
                Verdict::Verified
            } else {
                failed("not_founded".to_string())
            }
        }
        "gp_recruit" | "gp_patronize" | "gp_patronize_faith" => {
            let arrived = after.units.iter().any(|u| {
                u.great_person.as_ref().and_then(|gp| gp.class.as_deref()) == Some(verb)
                    && own_unit(before, u.id).is_none()
            });
            if arrived {
                Verdict::Verified
            } else {
                failed("no_great_person".to_string())
            }
        }
        "city_strike" | "encampment_strike" => {
            let harmed = order.pos.is_some_and(|p| target_harmed(before, after, p));
            if harmed
                || order
                    .subject
                    .is_some_and(|id| combat_evidence(evidence, turn, id))
            {
                Verdict::Verified
            } else {
                failed("target_unharmed".to_string())
            }
        }
        _ => Verdict::Unverifiable,
    }
}

/// Verify one order without sibling-order context. The single-order helper
/// remains useful for focused postcondition tests; live settlement supplies
/// the full turn context through `verify_orders` below.
#[cfg(test)]
fn verify_order(
    order: &IssuedOrder,
    turn: u32,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
) -> Verdict {
    verify_order_with_context(
        order,
        turn,
        before,
        after,
        tiles,
        evidence,
        VerificationContext {
            same_turn_orders: &[],
            later_fortified: false,
            later_moved: false,
            later_policy_deck: false,
        },
    )
}

/// Every order in `pending` against the frame that followed it.
#[cfg(test)]
fn verify_orders(
    pending: &PendingOrders,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    same_turn_orders: &[IssuedOrder],
) -> Vec<OrderCheck> {
    verify_orders_with_later_fortifications(
        pending,
        after,
        tiles,
        evidence,
        same_turn_orders,
        LaterOrderEvidence {
            fortified: &std::collections::BTreeSet::new(),
            moved: &std::collections::BTreeSet::new(),
            policy_deck: false,
        },
    )
}

/// Verify pending orders with positive evidence from later same-turn frames.
struct LaterOrderEvidence<'a> {
    fortified: &'a std::collections::BTreeSet<i64>,
    moved: &'a std::collections::BTreeSet<i64>,
    policy_deck: bool,
}

fn verify_orders_with_later_fortifications(
    pending: &PendingOrders,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    same_turn_orders: &[IssuedOrder],
    later: LaterOrderEvidence<'_>,
) -> Vec<OrderCheck> {
    pending
        .orders
        .iter()
        .map(|order| OrderCheck {
            verdict: verify_order_with_context(
                order,
                pending.turn,
                &pending.before,
                after,
                tiles,
                evidence,
                VerificationContext {
                    same_turn_orders,
                    later_fortified: order
                        .subject
                        .is_some_and(|id| later.fortified.contains(&id)),
                    later_moved: order.subject.is_some_and(|id| later.moved.contains(&id)),
                    later_policy_deck: later.policy_deck,
                },
            ),
            order: order.clone(),
        })
        .collect()
}

/// The rows that carry one turn's verdicts to the mod: one per checked order
/// and one `turn_verified` tally. `x` carries the verified turn because the
/// order row has no spare column; the mod reads it back as `turn`.
fn verdict_rows(turn: u32, checks: &[OrderCheck]) -> Vec<Order> {
    let mut rows = Vec::with_capacity(checks.len() + 1);
    let (mut verified, mut failed, mut unverifiable) = (0usize, 0usize, 0usize);
    for check in checks {
        match &check.verdict {
            Verdict::Verified => {
                verified += 1;
                rows.push(Order {
                    kind: "order_verified",
                    subject: check.order.subject,
                    verb: Some(check.order.label()),
                    pos: Some((turn as i32, -1)),
                });
            }
            Verdict::Failed(why) => {
                failed += 1;
                rows.push(Order {
                    kind: "order_failed",
                    subject: check.order.subject,
                    verb: Some(format!("{} {}", check.order.label(), why)),
                    pos: Some((turn as i32, -1)),
                });
            }
            Verdict::Unverifiable => unverifiable += 1,
        }
    }
    rows.push(Order {
        kind: "turn_verified",
        subject: Some(i64::from(turn)),
        verb: Some(format!(
            "issued={} verified={} failed={} unverifiable={}",
            checks.len(),
            verified,
            failed,
            unverifiable
        )),
        pos: None,
    });
    rows
}

/// Append rows to a reply's `orders` array without disturbing the rest.
fn append_rows_to_reply(reply: &str, rows: &[Order]) -> String {
    if rows.is_empty() {
        return reply.to_string();
    }
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(reply) else {
        return reply.to_string();
    };
    let Some(orders) = value.get_mut("orders").and_then(|o| o.as_array_mut()) else {
        return reply.to_string();
    };
    for row in rows {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&row.to_json()) {
            orders.push(parsed);
        }
    }
    value.to_string()
}

/// The verdict rows for every pending frame the new frame answers, and the
/// pending list trimmed to what is still waiting.
fn settle_pending_orders(
    pending: &mut Vec<PendingOrders>,
    events: &Path,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
) -> Vec<Order> {
    let answered: Vec<PendingOrders> = {
        let (done, waiting): (Vec<_>, Vec<_>) = std::mem::take(pending)
            .into_iter()
            .partition(|p| p.turn < after.turn);
        *pending = waiting;
        done
    };
    if answered.is_empty() {
        return Vec::new();
    }
    let turns: Vec<u32> = answered.iter().flat_map(|p| [p.turn, p.turn + 1]).collect();
    let (evidence, states) = ledger_evidence_and_states(events, &turns);
    let mut rows = Vec::new();
    let mut same_turn_orders: std::collections::BTreeMap<u32, Vec<IssuedOrder>> =
        Default::default();
    for frame in &answered {
        same_turn_orders
            .entry(frame.turn)
            .or_default()
            .extend(frame.orders.iter().cloned());
    }
    let mut by_turn: std::collections::BTreeMap<u32, Vec<OrderCheck>> = Default::default();
    for frame in &answered {
        let context = same_turn_orders
            .get(&frame.turn)
            .map_or(&[][..], Vec::as_slice);
        let later_fortified = later_fortified_units(&states, frame.turn, frame.frame);
        let later_moved = later_moved_units(&answered, frame.turn, frame.frame);
        let later_policy_replan =
            later_policy_deck_matches(&answered, frame.turn, frame.frame, after);
        by_turn
            .entry(frame.turn)
            .or_default()
            .extend(verify_orders_with_later_fortifications(
                frame,
                after,
                tiles,
                &evidence,
                context,
                LaterOrderEvidence {
                    fortified: &later_fortified,
                    moved: &later_moved,
                    policy_deck: later_policy_replan,
                },
            ));
    }
    for (turn, checks) in by_turn {
        rows.extend(verdict_rows(turn, &checks));
    }
    rows
}

/// Replay a recorded run's orders against its own frames.
///
/// `orders_path` holds the rows the brain wrote to `orders.sqlite`, one JSON
/// object per line (`turn`, `kind`, `subject`, `verb`, `x`, `y`, and optionally
/// `frame`); the frames
/// and the evidence come from the run's `events.jsonl`. Prints one JSON line:
/// what the checks verified, what failed and why, and the mod's own
/// `orders_reported` for the same turns, so the gap can be measured on a run
/// that was played before the checks existed. The board is not replayed, so
/// the plot-bound checks (`buy_plot`, `PILLAGE`, `REPAIR`) are counted apart
/// as `plot_bound` rather than as failures.
fn audit_orders(events: &Path, orders_path: &Path) {
    let mut by_frame: std::collections::BTreeMap<(u32, u32), Vec<IssuedOrder>> = Default::default();
    for line in std::fs::read_to_string(orders_path)
        .unwrap_or_default()
        .lines()
    {
        let Ok(row) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(turn) = row.get("turn").and_then(|t| t.as_u64()) else {
            continue;
        };
        let frame = row
            .get("frame")
            .and_then(|frame| frame.as_u64())
            .unwrap_or(0);
        let kind = row
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("")
            .to_string();
        if kind.is_empty() || VERDICT_ROW_KINDS.contains(&kind.as_str()) {
            continue;
        }
        let x = row.get("x").and_then(|v| v.as_i64());
        let y = row.get("y").and_then(|v| v.as_i64());
        by_frame
            .entry((turn as u32, frame as u32))
            .or_default()
            .push(IssuedOrder {
                kind,
                subject: row.get("subject").and_then(|v| v.as_i64()),
                verb: row.get("verb").and_then(|v| v.as_str()).map(str::to_string),
                pos: match (x, y) {
                    (Some(x), Some(y)) => Some((x as i32, y as i32)),
                    _ => None,
                },
            });
    }
    let raw = std::fs::read_to_string(events).unwrap_or_default();
    let mut frames: std::collections::BTreeMap<u32, civvis::mirror::StateSnapshot> =
        Default::default();
    let mut frame_states: std::collections::BTreeMap<(u32, u32), civvis::mirror::StateSnapshot> =
        Default::default();
    let mut all_states: Vec<civvis::mirror::StateSnapshot> = Vec::new();
    let mut reported: std::collections::BTreeMap<u32, (i64, i64)> = Default::default();
    let mut evidence: Vec<serde_json::Value> = Vec::new();
    for line in raw.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind == "state" {
            // The opening board of each turn: the one the orders were decided on.
            match serde_json::from_value::<civvis::mirror::StateSnapshot>(event) {
                Ok(state) => {
                    frame_states
                        .entry((state.turn, state.frame))
                        .or_insert_with(|| state.clone());
                    frames.entry(state.turn).or_insert_with(|| state.clone());
                    all_states.push(state);
                }
                Err(why) => eprintln!("audit: state frame skipped: {why}"),
            }
        } else if kind == "turn" && event.get("ctx").and_then(|c| c.as_str()) == Some("agent") {
            if let (Some(turn), Some(seen)) = (
                event.get("turn").and_then(|t| t.as_u64()),
                event.get("orders_seen").and_then(|n| n.as_i64()),
            ) {
                let ok = event
                    .get("orders_reported")
                    .or_else(|| event.get("orders_applied"))
                    .and_then(|n| n.as_i64())
                    .unwrap_or(0);
                let entry = reported.entry(turn as u32).or_insert((0, 0));
                entry.0 += seen;
                entry.1 += ok;
            }
        } else if EVIDENCE_KINDS.contains(&kind) {
            evidence.push(event);
        }
    }
    let tiles = civvis::mirror::Snapshot::from_chunks(&[]);
    let (mut verified, mut failed, mut unverifiable, mut plot_bound, mut unframed) =
        (0, 0, 0, 0, 0);
    let (mut seen, mut ok) = (0i64, 0i64);
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    let mut by_kind: std::collections::BTreeMap<String, [usize; 3]> = Default::default();
    let mut counted_reported_turns = std::collections::BTreeSet::new();
    let mut same_turn_orders: std::collections::BTreeMap<u32, Vec<IssuedOrder>> =
        Default::default();
    for ((turn, _frame), orders) in &by_frame {
        same_turn_orders
            .entry(*turn)
            .or_default()
            .extend(orders.iter().cloned());
    }
    // Precomputed because the loop below consumes `by_frame`: for each frame,
    // the units the decider moved in a LATER frame of the same turn. See
    // `later_moved_units` for why a FORTIFY they overrode is not a bridge
    // failure.
    let later_moved_by_frame: std::collections::BTreeMap<
        (u32, u32),
        std::collections::BTreeSet<i64>,
    > = by_frame
        .keys()
        .map(|&(turn, frame)| {
            let moved = by_frame
                .iter()
                .filter(|((t, f), _)| *t == turn && *f > frame)
                .flat_map(|(_, orders)| orders.iter())
                .filter(|order| order.verb.as_deref() == Some("MOVE_TO"))
                .filter_map(|order| order.subject)
                .collect();
            ((turn, frame), moved)
        })
        .collect();
    // A later policy deck is only useful as supersession evidence when the
    // final answering frame contains its requested cards. This mirrors the
    // live settlement path while keeping the replay loop free to consume
    // `by_frame` below.
    let later_policy_deck_by_frame: std::collections::BTreeMap<(u32, u32), bool> = by_frame
        .keys()
        .map(|&(turn, frame)| {
            let matched = frames.get(&(turn + 1)).is_some_and(|after| {
                by_frame
                    .iter()
                    .filter(|((t, f), _)| *t == turn && *f > frame)
                    .flat_map(|(_, orders)| orders.iter())
                    .any(|order| policy_deck_cards_present(order, after))
            });
            ((turn, frame), matched)
        })
        .collect();
    for ((turn, frame), orders) in by_frame {
        let Some(before) = frame_states
            .get(&(turn, frame))
            .or_else(|| frames.get(&turn))
        else {
            unframed += orders.len();
            continue;
        };
        let Some(after) = frames.get(&(turn + 1)) else {
            unframed += orders.len();
            continue;
        };
        if counted_reported_turns.insert(turn) {
            if let Some((s, a)) = reported.get(&turn) {
                seen += s;
                ok += a;
            }
        }
        let window: Vec<serde_json::Value> = evidence
            .iter()
            .filter(|e| {
                e.get("turn")
                    .and_then(|t| t.as_u64())
                    .is_some_and(|t| t == u64::from(turn) || t == u64::from(turn) + 1)
            })
            .cloned()
            .collect();
        let pending = PendingOrders {
            turn,
            frame,
            orders,
            before: before.clone(),
        };
        let context = same_turn_orders.get(&turn).map_or(&[][..], Vec::as_slice);
        let later_fortified = later_fortified_units(&all_states, turn, frame);
        let later_moved = later_moved_by_frame
            .get(&(turn, frame))
            .cloned()
            .unwrap_or_default();
        let later_policy_replan = later_policy_deck_by_frame
            .get(&(turn, frame))
            .copied()
            .unwrap_or(false);
        for check in verify_orders_with_later_fortifications(
            &pending,
            after,
            &tiles,
            &window,
            context,
            LaterOrderEvidence {
                fortified: &later_fortified,
                moved: &later_moved,
                policy_deck: later_policy_replan,
            },
        ) {
            let label = match check.order.kind.as_str() {
                "unit" => check
                    .order
                    .label()
                    .split(':')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(":"),
                kind => kind.to_string(),
            };
            let slot = by_kind.entry(label).or_insert([0, 0, 0]);
            match check.verdict {
                Verdict::Verified => {
                    verified += 1;
                    slot[0] += 1;
                }
                Verdict::Failed(why)
                    if why == "plot_unseen" || why == "not_pillaged" || why == "not_repaired" =>
                {
                    plot_bound += 1;
                }
                Verdict::Failed(why) => {
                    failed += 1;
                    slot[1] += 1;
                    let key = why.split('=').next().unwrap_or(&why).to_string();
                    *reasons.entry(key).or_default() += 1;
                }
                Verdict::Unverifiable => {
                    unverifiable += 1;
                    slot[2] += 1;
                }
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "issued": verified + failed + unverifiable + plot_bound + unframed,
            "verified": verified,
            "failed": failed,
            "unverifiable": unverifiable,
            "plot_bound": plot_bound,
            "unframed": unframed,
            "reported_seen": seen,
            "reported_ok": ok,
            "reasons": reasons,
            "by_kind": by_kind,
        })
    );
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = arg_text(&args, "--mirror") else {
        eprintln!("usage: civvis-orders --mirror <run-dir> [--turn N] [--serve] [--audit-orders <orders.jsonl>]");
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
            "treatments": civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&forced_on),
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

    // ★★★★★ PRINT THE BOARD CIVVIS IS ACTUALLY ANSWERING, so it can be diffed against
    // the one Civilization VI exported.
    //
    // ⚠ THIS EXISTS BECAUSE "the mirror is 1:1" HAS ALREADY BEEN CLAIMED AND BEEN
    // FALSE. It was rendering the right terrain at the wrong hexes: Civ 6 speaks
    // OFFSET, CIVVIS stores AXIAL, both are pairs of small integers, and nothing
    // complains when they are mixed. A capital at offset (56,28) had NO TILE in the
    // reconstruction and the only symptom was CIVVIS reporting "no legal revealed
    // site" on a map with 323 revealed plots — it blamed the map.
    //
    // The dump is keyed back in OFFSET (`hex::axial_to_offset`) precisely so the
    // round trip is exercised: a plot the export named and this dump cannot produce
    // is the coordinate bug's signature, and it shows as an ABSENT tile rather than a
    // wrong value.
    //
    // Not everything here is an independent check — terrain names are written from
    // this same export through this same vocabulary, so they must agree. What IS
    // independent, and is where the defects have been:
    //
    //   w    Civ 6 answers `IsWater()`; CIVVIS derives water from its own ruleset via
    //        the translated terrain. This is the "unrevealed ground reads as OCEAN"
    //        family, which cost a seat its whole continent.
    //   h    the export encodes hills in the terrain NAME (`TERRAIN_*_HILLS`) and
    //        CIVVIS carries a separate flag. Disagreement here is the standing
    //        explanation for improvement orders refused and re-issued forever.
    //   res  whether the name resolved at all. An unresolved name does not error: the
    //        tile silently keeps whatever `Game::new` generated, which is a wrong
    //        terrain wearing a right one's clothes.
    // ★★★★★ WHY THERE IS NO SETTLER, ANSWERED FOR A RECORDED RUN.
    //
    // The empire-level failure this project keeps measuring is EXPANSION: runs
    // end with one city while the plan asks for seven or eight. The Strategy
    // journal says "short of cities with land still open" every turn, and the
    // Cities journal says "starts heavy chariot" every turn, and NOTHING says
    // which of the settler gate's five conditions refused.
    //
    // Live on `civvis-20260804T041525Z`: one settler in 130 turns (built t9,
    // lost t28), then 90 consecutive turns of military while `desired_cities`
    // stood at 7. Reading the code cannot distinguish "the site search found
    // nothing" from "the branch was never reached", because the gate is a
    // single `&&` chain with no journal line of its own.
    //
    // This evaluates each condition separately on the reconstructed board and
    // prints them, so the answer is a measurement rather than an inference.
    if args.iter().any(|a| a == "--explain-settler") {
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            println!("no revealed terrain or no state yet");
            return;
        };
        let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
        let live = civvis::mirror::LiveMirror::new(
            &snapshot,
            &state,
            mirror_players,
            1,
            mirror_turns,
            frontier,
        );
        let g = &live.game;
        // ⚠⚠ THE PROBE MUST PLAY THE GENOME THE LIVE RUN PLAYED.
        //
        // The first version of this diagnostic used stock `AdvancedAi::new()`
        // weights and reported `mil_per_city` 1.0, while the live journal for the
        // same run printed **1.4** — a league genome the live seat had been handed.
        // The league is retired (#2357) and the live seat plays `AdvancedAi::new`
        // itself, so the probe and the deployment are the same agent again; the
        // header line still names it so nobody has to take that on trust.
        let advanced = civvis::ai::AdvancedAi::new();
        refuse_retired_strategy_flag(&args);
        println!("genome: stock AdvancedAi::new — the deployment genome");
        let w = advanced.weights().clone();
        let mut ai = civvis::ai::BasicAi::with_weights(w.clone());
        // The live decider plays with the second pipeline slot open, the
        // host's Settler population floor, and the land grab's wider pipeline
        // and later window (see `AdvancedAi::land_grab`).
        ai.enable_parallel_settlers();
        ai.enable_host_settler_pop();
        ai.enable_land_grab();
        let pid = 0usize;
        let n_cities = g.player_city_ids(pid).len();
        let settlers = g
            .units
            .values()
            .filter(|u| u.owner == pid && u.kind == "settler")
            .count();
        let city_pop = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pop)
            .max()
            .unwrap_or(0);
        println!(
            "turn {}  cities {n_cities}  settlers {settlers}  best city pop {city_pop}",
            g.turn
        );
        println!("settler gate, condition by condition:");
        println!(
            "  (cities+settlers) < city_target      {:>5}   {} < {:.1}",
            ((n_cities + settlers) as f64) < w.city_target,
            n_cities + settlers,
            w.city_target
        );
        // The live seat's pipeline width (see `AdvancedAi::land_grab`, and
        // `AdvancedAi::parallel_settlers` beneath it): two walkers from the
        // first city, one more per three cities, never more than the seats
        // still short of the target.
        let seats_short = (w.city_target.ceil().max(0.0) as usize).saturating_sub(n_cities);
        let pipeline = if seats_short > 0 {
            (civvis::ai::LAND_GRAB_PIPELINE_BASE + n_cities / 3).min(seats_short)
        } else if n_cities >= 2 && ((n_cities + settlers + 1) as f64) < w.city_target {
            2
        } else {
            1
        };
        println!(
            "  settlers < pipeline ({pipeline})            {:>5}   {settlers}",
            settlers < pipeline
        );
        // The live seat's floor is the host's (see `BasicAi::host_settler_pop`).
        let settler_min_pop = w.settler_min_pop.min(2.0);
        println!(
            "  city_pop >= settler_min_pop          {:>5}   {city_pop} >= {:.1} (genome {:.3}, host floor 2)",
            (city_pop as f64) >= settler_min_pop,
            settler_min_pop,
            w.settler_min_pop
        );
        // The land grab's window: a settler must still repay before the turn
        // limit (see `BasicAi::land_grab`); the genome's stop turn alone no
        // longer closes it.
        println!(
            "  turn < settler_stop_turn             {:>5}   {} < {:.0} (land grab keeps the window open while a settler still repays: {})",
            (g.turn as f64) < w.settler_stop_turn,
            g.turn,
            w.settler_stop_turn,
            g.turn + g.standard_duration(18) < g.max_turns
        );
        // The military branch sits ABOVE the settler gate in `pick_item` and
        // returns first when it fires, so the gate passing means nothing on its
        // own. Recompute its floor from outside: `mil_per_city * cities`, raised
        // by visible besiegers within 3 of a city, capped at +3.
        let military = g
            .units
            .values()
            .filter(|u| u.owner == pid && g.rules.units[u.kind].class == "military")
            .count();
        let besiegers = {
            let homes: Vec<_> = g
                .player_city_ids(pid)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .collect();
            g.units
                .values()
                .filter(|u| u.owner != pid && g.rules.units[u.kind].class == "military")
                .filter(|u| homes.iter().any(|h| g.wdist(*h, u.pos) <= 3))
                .count()
        };
        let floor = (w.mil_per_city * n_cities as f64).max(if besiegers == 0 {
            0.0
        } else {
            w.mil_per_city * n_cities as f64 + besiegers.min(3) as f64
        });
        println!(
            "military branch: hold {military}, floor {floor:.1} (besiegers within 3: {besiegers}) -> fires {}",
            (military as f64) < floor
        );
        // End the inference: ask the chooser itself.
        {
            let cid = g.player_city_ids(pid).into_iter().next();
            if let Some(cid) = cid {
                let probe = g.clone();
                let melee = g
                    .units
                    .values()
                    .filter(|u| u.owner == pid && g.rules.units[u.kind].class == "military")
                    .count();
                let chosen = ai.pick_item(
                    &probe, pid, cid, n_cities, settlers, 0, 0, 0, military, melee, 0,
                );
                println!("pick_item returns: {chosen:?}");
            }
        }
        let practical = ai.has_practical_settle_site(g, pid);
        println!("  has_practical_settle_site            {practical:>5}");
        // When the site search refuses, say WHICH rule refused each candidate:
        // "no site" and "every site too close to a city we already own" are
        // different defects and only one of them is about the map.
        if !practical {
            let mut water = 0;
            let mut impassable = 0;
            let mut too_close = 0;
            let mut foreign = 0;
            let mut wonder = 0;
            let mut ok = 0;
            for (pos, tile) in g.map.tiles.iter() {
                if g.rules.is_water(tile) {
                    water += 1;
                } else if !g.rules.is_passable(tile) {
                    impassable += 1;
                } else if g.tile_is_natural_wonder(tile) {
                    wonder += 1;
                } else if g
                    .cities
                    .values()
                    .any(|city| (g.wdist(city.pos, *pos) as f64) < w.min_city_dist)
                {
                    too_close += 1;
                } else if tile
                    .owner_city
                    .is_some_and(|cid| g.cities[&cid].owner != pid)
                {
                    foreign += 1;
                } else {
                    ok += 1;
                }
            }
            println!(
                "  known tiles {}: water {water}, impassable {impassable}, natural wonder {wonder}, within min_city_dist {:.0} of a city {too_close}, foreign-owned {foreign}, VALID {ok}",
                g.map.tiles.len(),
                w.min_city_dist
            );
            println!("  ⚠ VALID > 0 with practical=false means the site exists but the 8-step walk cannot REACH it");
        }
        return;
    }
    if args.iter().any(|a| a == "--dump-mirror") {
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            println!("{{\"plots\":[],\"note\":\"no revealed terrain or no state yet\"}}");
            return;
        };
        let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
        let live = civvis::mirror::LiveMirror::new(
            &snapshot,
            &state,
            mirror_players,
            1,
            mirror_turns,
            frontier,
        );
        let game = &live.game;
        let vocab = civvis::mirror::Vocabulary::embedded();
        let width = snapshot.width.max(1);
        let height = snapshot.height.max(1);
        let mut plots: Vec<String> = Vec::new();
        let mut unresolved: std::collections::BTreeMap<String, usize> = Default::default();
        for y in 0..height {
            for x in 0..width {
                let pos = civvis::hex::offset_to_axial(x, y);
                let Some(tile) = game.map.get(pos) else {
                    // Deliberately NOT skipped silently: a plot with no tile is the
                    // whole reason this dump exists. It is reported as absent by
                    // simply not appearing, and the diff counts it.
                    continue;
                };
                let exported = snapshot.plot((x, y));
                // Only dump ground either side has an opinion about. The far unknown
                // is ocean filler on both and would drown the diff in agreement.
                if exported.is_none() && !snapshot.is_revealed((x, y)) {
                    continue;
                }
                let mut resolved = true;
                if let Some(plot) = exported {
                    if let Some(name) = &plot.t {
                        match vocab.terrain(name) {
                            civvis::mirror::Resolved::Known(_) => {}
                            _ => {
                                resolved = false;
                                *unresolved.entry(name.clone()).or_default() += 1;
                            }
                        }
                    }
                }
                let field = |value: &Option<civvis::name::Name>| match value {
                    Some(name) => format!("\"{}\"", name.as_str()),
                    None => "null".to_string(),
                };
                // Whose ground the mirror thinks this is, as a CIVVIS seat index.
                // The export gives a Civ 6 player id and our seat is always its
                // local player 0, so "ours" is the part that compares cleanly; a
                // rival's id does not, because rivals are remapped on the way in.
                let owner = tile
                    .owner_city
                    .and_then(|cid| game.cities.get(&cid))
                    .map(|city| city.owner as i64);
                let foundation = tile
                    .district_foundation
                    .as_ref()
                    .map(|placed| format!("\"{}\"", placed.district.as_str()))
                    .unwrap_or_else(|| "null".to_string());
                // What the board PAYS this plot, and what the host said it
                // pays. The dump named every row of the ruleset either side
                // reads and not the one number they are read for, so the
                // mapping could be verified tile by tile while the yields
                // derived from it went unchecked — which is how disaster
                // fertility stayed invisible on the board for as long as it
                // did. `ty` is `Game::workable_tile_yields`; `hy` is the
                // export's own `Plot:GetYield` tuple where the sweep carried
                // one, so `ty` minus `hy` is the residual to explain.
                //
                // ⚠ `ty`, not `y`: `y` is this plot's row coordinate three
                // fields to the left, and a duplicate key in one JSON object
                // silently keeps whichever a reader saw last.
                let paid = serde_json::to_string(&game.workable_tile_yields(pos))
                    .unwrap_or_else(|_| "null".to_string());
                let host = exported
                    .and_then(civvis::mirror::Plot::host_yields)
                    .map(|yields| serde_json::to_string(&yields).unwrap_or_else(|_| "null".into()))
                    .unwrap_or_else(|| "null".to_string());
                plots.push(format!(
                    "{{\"x\":{},\"y\":{},\"t\":\"{}\",\"h\":{},\"w\":{},\"f\":{},\"r\":{},\
                     \"im\":{},\"d\":{},\"df\":{},\"wo\":{},\"p\":{},\"own\":{},\"res\":{},\
                     \"ty\":{},\"hy\":{}}}",
                    x,
                    y,
                    tile.terrain.as_str(),
                    tile.hills,
                    game.rules.is_water(tile),
                    field(&tile.feature),
                    field(&tile.resource),
                    field(&tile.improvement),
                    field(&tile.district),
                    foundation,
                    field(&tile.wonder),
                    tile.pillaged,
                    owner.map(|o| o == 0).unwrap_or(false),
                    resolved,
                    paid,
                    host,
                ));
            }
        }
        let unresolved_json: Vec<String> = unresolved
            .iter()
            .map(|(name, count)| format!("\"{name}\":{count}"))
            .collect();
        let mut cities = Vec::new();
        for city in game.cities.values().filter(|city| city.owner == 0) {
            let (x, y) = civvis::hex::axial_to_offset(city.pos.0, city.pos.1);
            let plan = game.city_citizen_plan(city.id);
            let worked = plan
                .worked_tiles
                .iter()
                .map(|pos| {
                    let (wx, wy) = civvis::hex::axial_to_offset(pos.0, pos.1);
                    serde_json::json!({"x": wx, "y": wy})
                })
                .collect::<Vec<_>>();
            // The tile-level ledger behind the model total, in the host's
            // offset coordinates so `tools/civ6_yield_drift.py` can diff it
            // against the export's per-plot yields tile for tile.
            let ledger = game.city_yield_ledger(city.id);
            let offset = |pos: civvis::Pos| {
                let (px, py) = civvis::hex::axial_to_offset(pos.0, pos.1);
                serde_json::json!({"x": px, "y": py})
            };
            let ledger_json = serde_json::json!({
                "center": ledger.center,
                "tiles": ledger
                    .tiles
                    .iter()
                    .map(|(pos, yields)| {
                        let mut entry = offset(*pos);
                        entry["yields"] = serde_json::json!(yields);
                        entry
                    })
                    .collect::<Vec<_>>(),
                "tile_adjustments": ledger
                    .tile_adjustments
                    .iter()
                    .map(|(pos, yields)| {
                        let mut entry = offset(*pos);
                        entry["yields"] = serde_json::json!(yields);
                        entry
                    })
                    .collect::<Vec<_>>(),
                "specialists": ledger
                    .specialists
                    .iter()
                    .map(|(family, yields)| serde_json::json!({"district": family, "yields": yields}))
                    .collect::<Vec<_>>(),
                "districts": ledger
                    .districts
                    .iter()
                    .map(|(name, pos, yields)| {
                        let mut entry = offset(*pos);
                        entry["district"] = serde_json::json!(name);
                        entry["yields"] = serde_json::json!(yields);
                        entry
                    })
                    .collect::<Vec<_>>(),
            });
            // Board values carry the mirror's host-to-model correction; the
            // `model_*` twins are what CIVVIS derives on its own, which is the
            // number the fidelity instrument scores.
            let housing = game.city_housing(city);
            let housing_adjustment = game
                .observed_city_housing_adjustments
                .get(&city.id)
                .copied()
                .unwrap_or(0.0);
            let housing_sources = serde_json::json!(game
                .city_housing_sources(city)
                .named()
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>());
            let amenities = game.city_amenities(city);
            let amenity_adjustment = game
                .observed_city_amenity_adjustments
                .get(&city.id)
                .copied()
                .unwrap_or(0);
            cities.push(serde_json::json!({
                "x": x,
                "y": y,
                "name": city.name,
                "pop": city.pop,
                "production_progress": city.production,
                "worked": worked,
                "specialists": plan.specialists,
                "yields": game.city_yields(city.id),
                "model_yields": game.city_yields_model(city.id),
                "ledger": ledger_json,
                "housing": housing,
                "model_housing": housing - housing_adjustment,
                // The host exports `housing_from_water`, `_buildings`,
                // `_districts` and the rest on every city of every state. Until
                // this twin existed the instrument could only compare the two
                // totals, so a persistent gap named no rule and nobody could
                // act on it. Same categories, same names.
                "model_housing_sources": housing_sources,
                "amenities": amenities,
                "model_amenities": amenities - amenity_adjustment,
                "amenities_required": civvis::game::Game::city_amenities_required(city),
                "amenity_surplus": game.city_amenity_surplus(city),
            }));
        }
        let great_works = [
            "writing",
            "art",
            "religious_art",
            "artifact",
            "music",
            "relic",
        ]
        .into_iter()
        .map(|kind| {
            (
                kind,
                game.players[0]
                    .counters
                    .get(&format!("great_work:{kind}"))
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
        let governors = game.players[0]
            .governor_roster
            .iter()
            .filter_map(|(name, governor)| {
                let host_type = mirror::civ6_governor_name(name.as_str())?;
                let city = governor.city.and_then(|cid| game.cities.get(&cid));
                let (x, y) = city
                    .map(|city| civvis::hex::axial_to_offset(city.pos.0, city.pos.1))
                    .unwrap_or((-1, -1));
                let establish_turns = game
                    .rules
                    .governors
                    .get(name)
                    .map(|spec| game.standard_duration(spec.establish_turns))
                    .unwrap_or(0);
                let mut promotions = governor
                    .promotions
                    .iter()
                    .filter_map(|promotion| mirror::civ6_governor_promotion(promotion))
                    .collect::<Vec<_>>();
                promotions.sort_unstable();
                Some(serde_json::json!({
                    "type": host_type,
                    "x": x,
                    "y": y,
                    "established": city.is_some()
                        && game.turn >= governor.assigned_turn.saturating_add(establish_turns),
                    "neutralized": game.turn < governor.disabled_until,
                    "promotions": promotions,
                }))
            })
            .collect::<Vec<_>>();
        let mut empire_yields = civvis::rules::Yields::default();
        let mut model_empire_yields = civvis::rules::Yields::default();
        for city in game.cities.values().filter(|city| city.owner == 0) {
            empire_yields.add(game.city_yields(city.id));
            model_empire_yields.add(game.city_yields_model(city.id));
        }
        // What the empire collects beside its cities — founder beliefs and
        // the Faith Firaxis pays for Great Person points nobody can spend —
        // is part of the model's per-turn figure and of the board's, and the
        // player-level correction (`observed_yield_adjustments`) is measured
        // against the same sum, so the board reads the host's number.
        let player_extras = game.player_yield_extras(0);
        empire_yields.add(player_extras);
        model_empire_yields.add(player_extras);
        if let Some(adjustment) = game.observed_yield_adjustments.get(&0) {
            empire_yields.add(*adjustment);
        }
        let great_person_points_per_turn = game.great_person_points_per_turn(0);
        let unused_great_person_classes = great_person_points_per_turn
            .keys()
            .filter(|kind| !game.great_person_class_earnable(0, kind))
            .cloned()
            .collect::<Vec<_>>();
        // What crossed for the first time on 2026-08-26, read back from the
        // board it landed on: the host's climate beside the phase it set, the
        // request each city-state is making, and the routes the host priced.
        let quests: Vec<serde_json::Value> = game.players[0]
            .quests
            .iter()
            .map(|(minor, quest)| {
                serde_json::json!({
                    "minor": game.players.get(*minor).map(|player| player.civ.clone()),
                    "kind": quest.kind,
                    "target": quest.target,
                })
            })
            .collect();
        let route_options: Vec<serde_json::Value> = game
            .observed_route_options
            .iter()
            .map(|((origin, destination), yields)| {
                serde_json::json!({
                    "origin": game.cities.get(origin).map(|city| city.name.to_string()),
                    "dest": game.cities.get(destination).map(|city| city.name.to_string()),
                    "yields": yields,
                })
            })
            .collect();
        println!(
            "{{\"turn\":{},\"width\":{},\"height\":{},\"revealed\":{},\
             \"unresolved_terrain\":{{{}}},\"plots\":[{}],\"cities\":{},\
             \"great_works\":{},\"governors\":{},\"governor_points\":{},\
             \"governor_points_spent\":{},\"governor_points_available\":{},\
             \"empire_yields\":{},\"model_empire_yields\":{},\
             \"player_extras\":{},\"unused_great_person_faith\":{},\
             \"great_person_points_per_turn\":{},\"unused_great_person_classes\":{},\
             \"host_faith_per_turn\":{},\"climate_phase\":{},\"climate\":{},\
             \"quests\":{},\"route_options\":{},\"observed_appeal\":{},\
             \"host_observed\":{}}}",
            state.turn,
            width,
            height,
            snapshot.revealed_count(),
            unresolved_json.join(","),
            plots.join(","),
            serde_json::to_string(&cities).unwrap(),
            serde_json::to_string(&great_works).unwrap(),
            serde_json::to_string(&governors).unwrap(),
            game.governor_titles(0),
            game.players[0].governor_titles_spent,
            game.governor_titles_available(0),
            serde_json::to_string(&empire_yields).unwrap(),
            serde_json::to_string(&model_empire_yields).unwrap(),
            serde_json::to_string(&player_extras).unwrap(),
            game.unused_great_person_faith(0),
            serde_json::to_string(&great_person_points_per_turn).unwrap(),
            serde_json::to_string(&unused_great_person_classes).unwrap(),
            serde_json::to_string(&state.faith_per_turn).unwrap(),
            game.climate_phase,
            serde_json::to_string(&game.observed_climate).unwrap(),
            serde_json::to_string(&quests).unwrap(),
            serde_json::to_string(&route_options).unwrap(),
            game.observed_appeal.len(),
            game.host_observed.len(),
        );
        return;
    }

    if !serve {
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            println!(
                "{{\"turn\":0,\"orders\":[],\"note\":\"no revealed terrain or no state yet\"}}"
            );
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
        // One-shot: there is no next turn to be self-limiting against, so this starts
        // empty and every foreign choice is released once.
        let mut ours = std::collections::BTreeMap::new();
        let mut host_peace_retries = HostPeaceRetries::default();
        let mut host_move_refusals = HostMoveRefusals::default();
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
    // The Firaxis repair cooldown belongs to the host, not the reconstructed
    // board. It must therefore survive `--fresh-board` just like the peace and
    // treasury handoffs above.
    let mut host_city_attack_cooldowns = HostCityAttackCooldowns::default();
    // A city's strike is once per host turn and the export never says it was
    // spent; the decider's own earlier frames do. See `HostCityStrikes`.
    let mut host_city_strikes = HostCityStrikes::default();
    let mut explain_cursor: u64 = 0;
    // What left for the host on each frame, until the next turn's frame answers
    // for it. See "order postconditions" above.
    let mut pending_orders: Vec<PendingOrders> = Vec::new();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let want: Option<u32> = line.trim().parse().ok();
        let reply = match load(want) {
            None => format!(
                "{{\"turn\":{},\"orders\":[],\"note\":\"no revealed terrain or no state yet\"}}",
                want.unwrap_or(0)
            ),
            Some((snapshot, state)) => {
                let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
                host_city_attack_cooldowns.observe(&state);
                host_move_refusals.observe(&state);
                // The verdicts on the previous turn's orders ride at the end of
                // this turn's reply; the checks read the frame the decision is
                // about to read, before anything is decided on it.
                let verdicts =
                    settle_pending_orders(&mut pending_orders, &events, &state, &snapshot);
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
                            let carried: std::collections::BTreeMap<u32, u32> = old
                                .iter()
                                .filter_map(|(old_uid, civ6)| {
                                    board.uid_of.get(civ6).map(|new| (*old_uid, *new))
                                })
                                .collect();
                            ai.remap_unit_memory(&carried);
                        }
                        None => ai.forget_unit_memory(),
                    }
                    board.carry_treasury_baseline(carried_treasury);
                    host_city_attack_cooldowns.apply(&mut board);
                    host_city_strikes.apply(&mut board, state.turn);
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
                            host_city_attack_cooldowns.apply(&mut fresh);
                            host_city_strikes.apply(&mut fresh, state.turn);
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
                                },
                            );
                            live = Some(fresh);
                            reply
                        }
                        Some(existing) => {
                            existing.sync(&snapshot, &state, frontier);
                            host_city_attack_cooldowns.apply(existing);
                            host_city_strikes.apply(existing, state.turn);
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
                                    },
                                )
                            }
                        }
                    }
                };
                let orders = IssuedOrder::from_reply(&reply);
                host_city_strikes.observe(state.turn, &orders);
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
mod tests {
    /// ★★★★★ EVERY ITEM THE BOARD CAN ORDER COMES BACK FROM ITS HOST SPELLING
    /// TO THE SAME KEY. `Game::can_produce` gates on the host's exported menu,
    /// translated through `mirror::host_production_key`; an orderable item
    /// whose outbound name did not translate back would be gated off every
    /// live board forever. Checked for the names the shipped game has
    /// (`data/civ6_type_names.json`): a CIVVIS-only item can be on no host
    /// menu and can be ordered nowhere either.
    #[test]
    fn every_orderable_item_round_trips_through_the_host_menu_key() {
        use civvis::game::Item;
        let rules = civvis::rules::Rules::embedded();
        let shipped: std::collections::BTreeSet<String> =
            serde_json::from_str(include_str!("../../data/civ6_type_names.json"))
                .expect("the type-name snapshot is a JSON list");
        let mut items: Vec<(Item, String)> = Vec::new();
        for unit in rules.units.keys() {
            items.push((Item::Unit { unit: *unit }, format!("unit:{unit}")));
        }
        for building in rules.buildings.keys() {
            items.push((
                Item::Building {
                    building: *building,
                },
                format!("building:{building}"),
            ));
        }
        for wonder in rules.wonders.keys() {
            items.push((
                Item::Wonder {
                    wonder: *wonder,
                    pos: (0, 0),
                },
                format!("wonder:{wonder}"),
            ));
        }
        for district in rules.districts.keys() {
            items.push((
                Item::District {
                    district: *district,
                    pos: (0, 0),
                },
                format!("district:{district}"),
            ));
        }
        for project in rules.projects.keys() {
            items.push((
                Item::Project { project: *project },
                format!("project:{project}"),
            ));
        }
        let mut broken = Vec::new();
        let mut checked = 0;
        for (item, key) in items {
            let Some(host) = super::civ6_build_name(&item) else {
                continue;
            };
            if !shipped.contains(&host) {
                continue;
            }
            checked += 1;
            let back = civvis::mirror::host_production_key(&rules, &host, None);
            if back.as_deref() != Some(key.as_str()) {
                broken.push(format!("{host} -> {back:?}, the gate wants {key}"));
            }
        }
        assert!(
            checked > 200,
            "only {checked} orderable names are in the snapshot"
        );
        assert!(
            broken.is_empty(),
            "host spellings that do not come back to the board's key:\n{}",
            broken.join("\n")
        );
    }

    /// The lane list is the enum's, in the enum's order, with `civvis` in front.
    ///
    /// ⚠ The usage line and the Python launchers' `choices` are both generated
    /// from this const, so it is the one place the six names are written. If a
    /// seventh `VictoryTarget` is added this fails until the const names it, and
    /// `test_civ6_play.py` then fails until the launchers do.
    #[test]
    fn the_usage_line_offers_exactly_the_targets_the_engine_implements() {
        let expected: Vec<&str> = std::iter::once("civvis")
            .chain(
                civvis::ai::VictoryTarget::ALL
                    .iter()
                    .map(|target| target.as_str()),
            )
            .collect();
        assert_eq!(
            super::VICTORY_LANES.split('|').collect::<Vec<_>>(),
            expected
        );
    }

    /// A bare `civvis_orders` must resolve the launcher chain's lane to a real
    /// target — not merely name it. Three of the six spellings resolved to
    /// nothing until 2026-08-17, which is why this asserts the resolution and
    /// not just the string.
    ///
    #[test]
    fn direct_default_uses_the_safe_launcher_lane() {
        assert_eq!(super::DEFAULT_VICTORY, "science");
        assert!(super::VICTORY_LANES
            .split('|')
            .any(|lane| lane == super::DEFAULT_VICTORY));
        assert_eq!(
            super::victory_lane(super::DEFAULT_VICTORY).and_then(|ai| ai.victory_target()),
            Some(civvis::ai::VictoryTarget::Science)
        );
    }

    /// ⚠⚠ THREE OF THESE SIX RESOLVED TO NOTHING UNTIL 2026-08-17. The live seat
    /// matched four names by hand while `VictoryTarget` had six, so Culture,
    /// Religion and Diplomacy exited with code 2 — and because `advanced.rs`
    /// gates each lane's own machinery on being targeted at it, no other setting
    /// could reach them either.
    #[test]
    fn every_named_lane_builds_the_agent_it_names() {
        for target in civvis::ai::VictoryTarget::ALL {
            let ai = super::victory_lane(target.as_str())
                .unwrap_or_else(|| panic!("{} is in the usage line", target.as_str()));
            assert_eq!(ai.victory_target(), Some(target));
        }
        // The aliases `VictoryTarget::from_str` already accepted now work on the
        // command line as well, so a run asking for `religion` is not refused.
        for (spelling, target) in [
            ("religion", civvis::ai::VictoryTarget::Religion),
            ("diplomacy", civvis::ai::VictoryTarget::Diplomacy),
            ("conquest", civvis::ai::VictoryTarget::Domination),
        ] {
            assert_eq!(
                super::victory_lane(spelling).and_then(|ai| ai.victory_target()),
                Some(target),
                "{spelling}"
            );
        }
        // `civvis` is the absence of a target, not one of them.
        assert_eq!(
            super::victory_lane("civvis").and_then(|ai| ai.victory_target()),
            None
        );
        // And a name that is not a lane stays an error rather than quietly
        // becoming Domination, which is what the `--fresh-ai` copy of this match
        // used to do.
        assert!(super::victory_lane("religous").is_none());
    }

    /// ★★★★★ A CONTROL ARM THE LIVE HARNESS CAN ACTUALLY RUN.
    ///
    /// Every live-bridge repair has a `live_without_*` arm in `src/elo.rs`, but
    /// `ai_eval` plays headless CIVVIS where several of them cannot act at all —
    /// `closed_borders` and `host_observed` are populated only by `mirror.rs`,
    /// and the 20-axis composite measures +9 Elo (CI −53..+71) against plain
    /// `advanced`. The live harness, meanwhile, had no way to hold a treatment
    /// off: `civ6_play.py` starts `--serve` and takes whatever the binary does.
    /// So the one regime where these mechanisms fire had no control arm.
    ///
    /// ⚠ The unknown-name case is asserted deliberately. A typo that silently
    /// produced a control identical to the treatment would report a null that
    /// looks exactly like a real one.
    #[test]
    fn a_live_treatment_can_be_withheld_by_name() {
        let mut ai = civvis::ai::AdvancedAi::new();
        // The universe, not the deployment genome: the ledger already holds
        // unproven genes off, and this test is about the withholding path.
        ai.enable_live_bridge_universe();
        assert!(
            ai.blind_objective_strength,
            "the composite must set the flag before we can meaningfully hold it off"
        );

        withhold_live_treatment(&mut ai, "blind-objective-strength")
            .expect("a registered treatment can be withheld");
        assert!(
            !ai.blind_objective_strength,
            "--without must actually clear the flag, or the control arm is the treatment"
        );

        // Holding one off must not disturb its neighbours.
        assert!(
            ai.blind_objective_units,
            "withholding one treatment must leave the rest of the composite intact"
        );

        assert!(ai.amenity_project_preemption);
        withhold_live_treatment(&mut ai, "amenity-project-preemption")
            .expect("the amenity repair's control arm is registered");
        assert!(
            !ai.amenity_project_preemption,
            "the named Amenity control must hold the queue handoff off"
        );

        assert!(ai.live_wonder_race);
        withhold_live_treatment(&mut ai, "live-wonder-race")
            .expect("the wonder race's control arm is registered");
        assert!(
            !ai.live_wonder_race,
            "the named wonder-race control must hold it off"
        );

        assert!(ai.amenity_district_path);
        withhold_live_treatment(&mut ai, "amenity-district-path")
            .expect("the amenity path's control arm is registered");
        assert!(
            !ai.amenity_district_path,
            "the named amenity-path control must hold it off"
        );

        assert!(ai.expansion_before_prophet);
        withhold_live_treatment(&mut ai, "expansion-before-prophet")
            .expect("the Prophet deferral's control arm is registered");
        assert!(
            !ai.expansion_before_prophet,
            "the named Prophet-deferral control must hold it off"
        );

        assert!(ai.no_elective_war);
        withhold_live_treatment(&mut ai, "no-elective-war")
            .expect("the elective-war stand-down's control arm is registered");
        assert!(
            !ai.no_elective_war,
            "the named elective-war control must hold it off"
        );

        assert!(ai.fog_land_capacity);
        withhold_live_treatment(&mut ai, "fog-land-capacity")
            .expect("the fogged-capacity control arm is registered");
        assert!(
            !ai.fog_land_capacity,
            "the named fogged-capacity control must hold it off"
        );

        assert!(ai.score_horizon);
        withhold_live_treatment(&mut ai, "score-horizon")
            .expect("the score-horizon control arm is registered");
        assert!(
            !ai.score_horizon,
            "the named score-horizon control must hold it off"
        );
        assert!(ai.one_launch_pad);
        withhold_live_treatment(&mut ai, "one-launch-pad")
            .expect("the one-launch-pad control arm is registered");
        assert!(
            !ai.one_launch_pad,
            "the named one-launch-pad control must hold it off"
        );
        assert!(ai.naval_recon());
        withhold_live_treatment(&mut ai, "naval-recon")
            .expect("the naval-recon control arm is registered");
        assert!(
            !ai.naval_recon(),
            "the named naval-recon control must hold it off"
        );

        assert!(ai.counter_in_lane);
        withhold_live_treatment(&mut ai, "counter-in-lane")
            .expect("the counter-in-lane control arm is registered");
        assert!(
            !ai.counter_in_lane,
            "the named counter-in-lane control must hold it off"
        );
        assert!(ai.era_paced_expansion);
        withhold_live_treatment(&mut ai, "era-paced-expansion")
            .expect("the era-paced-expansion control arm is registered");
        assert!(
            !ai.era_paced_expansion,
            "the named era-pace control must hold it off"
        );

        assert!(ai.tally_culture);
        withhold_live_treatment(&mut ai, "tally-culture")
            .expect("the tally-culture control arm is registered");
        assert!(
            !ai.tally_culture,
            "the named tally-culture control must hold it off"
        );
        assert!(ai.culture_building_debt);
        withhold_live_treatment(&mut ai, "culture-building-debt")
            .expect("the culture-building-debt control arm is registered");
        assert!(
            !ai.culture_building_debt,
            "the named culture-building-debt control must hold it off"
        );
        assert!(ai.frontier_loyalty);
        withhold_live_treatment(&mut ai, "frontier-loyalty")
            .expect("the frontier-loyalty control arm is registered");
        assert!(
            !ai.frontier_loyalty,
            "the named frontier-loyalty control must hold it off"
        );

        assert!(ai.settler_target_hysteresis);
        withhold_live_treatment(&mut ai, "settler-target-hysteresis")
            .expect("the settler-target-hysteresis control arm is registered");
        assert!(
            !ai.settler_target_hysteresis,
            "the named settler-target-hysteresis control must hold it off"
        );

        assert!(ai.tally_great_people);
        withhold_live_treatment(&mut ai, "tally-great-people")
            .expect("the tally-great-people control arm is registered");
        assert!(
            !ai.tally_great_people,
            "the named tally-great-people control must hold it off"
        );

        assert!(ai.barbarian_scouts_are_scouts);
        withhold_live_treatment(&mut ai, "barbarian-scouts-are-scouts")
            .expect("the barbarian-scout control arm is registered");
        assert!(
            !ai.barbarian_scouts_are_scouts,
            "the named barbarian-scout control must hold it off"
        );

        assert!(ai.camp_party());
        withhold_live_treatment(&mut ai, "camp-party")
            .expect("the camp-party control arm is registered");
        assert!(
            !ai.camp_party(),
            "the named camp-party control must hold it off"
        );

        assert!(ai.buildings_before_projects);
        withhold_live_treatment(&mut ai, "buildings-before-projects")
            .expect("the buildings-before-projects control arm is registered");
        assert!(
            !ai.buildings_before_projects,
            "the named buildings-before-projects control must hold it off"
        );

        let bad = withhold_live_treatment(&mut ai, "no-such-treatment");
        assert!(
            bad.is_err(),
            "an unknown name must be a hard error; a silent no-op would report a \
             null that is indistinguishable from a real one"
        );
        assert!(
            bad.unwrap_err().contains("come-ashore"),
            "the error must name what this binary can actually withhold"
        );
    }

    /// ⚠⚠ ELEVEN SHIPPED TREATMENTS HAD NO CONTROL ARM ON THE ONLY HARNESS
    /// WHERE THEY FIRE, and nothing said so: this binary matched 57
    /// hand-written names against a 68-row table, and its usage string was a
    /// third, shorter copy again. `deny_while_targeted`,
    /// `live_religious_purchase`, `live_trader_route`,
    /// `loyalty_policy_defence`, `peacetime_deterrence`, `ranged_line_of_sight`,
    /// `recorded_tactical_step`, `slot_kind_tiebreak` and `strike_opening` were
    /// unwithholdable — including the religious-purchase repair, on the lane
    /// this engine finishes fastest.
    ///
    /// The list cannot be short again: it is the table.
    #[test]
    fn every_registered_live_treatment_can_be_withheld() {
        for gene in civvis::ai::GENES.iter().filter(|gene| gene.live()) {
            let mut ai = civvis::ai::AdvancedAi::new();
            ai.enable_live_bridge();
            withhold_live_treatment(&mut ai, gene.tag).unwrap_or_else(|error| {
                panic!(
                    "{} is a live gene but not withholdable: {error}",
                    gene.field
                )
            });
        }
    }

    /// And the usage line is the same list, so a name the binary accepts is
    /// never one the error message hides.
    #[test]
    fn the_usage_line_names_every_treatment_the_binary_accepts() {
        let listed: Vec<String> = super::withholdable_treatments()
            .split(", ")
            .map(str::to_string)
            .collect();
        let registered: Vec<String> = civvis::ai::GENES
            .iter()
            .filter(|gene| gene.live())
            .map(|gene| gene.tag.to_string())
            .collect();
        assert_eq!(listed, registered);
    }

    /// The ledger's best genome remains the default, but a verification arm
    /// needs a precise way to restore one held row and record a real contrast.
    /// The host says why a step did not happen; the checks now say it back.
    #[test]
    fn a_refused_step_carries_the_hosts_own_reason() {
        let evidence = vec![
            serde_json::json!({
                "kind": "move_refused", "turn": 42, "unit": 7,
                "x": 3, "y": 4, "dest_impassable": true
            }),
            serde_json::json!({
                "kind": "move_refused", "turn": 42, "unit": 9,
                "x": 5, "y": 6, "dest_water": true
            }),
            serde_json::json!({
                "kind": "move_refused", "turn": 42, "unit": 11, "x": 1, "y": 1
            }),
        ];
        assert_eq!(
            super::move_refusal_reason(&evidence, 42, 7).as_deref(),
            Some("host_refused_impassable")
        );
        assert_eq!(
            super::move_refusal_reason(&evidence, 42, 9).as_deref(),
            Some("host_refused_water")
        );
        assert_eq!(
            super::move_refusal_reason(&evidence, 42, 11).as_deref(),
            Some("host_refused_move")
        );
        assert_eq!(
            super::move_refusal_reason(&evidence, 43, 7),
            None,
            "a refusal belongs to the turn it happened on"
        );
        assert_eq!(super::move_refusal_reason(&evidence, 42, 8), None);
        assert_eq!(
            super::move_refusal_reason(&[], 42, 7),
            None,
            "and a run whose mod says nothing keeps the position diff"
        );
        assert!(
            super::EVIDENCE_KINDS.contains(&"move_refused"),
            "the reader has to be asked for the kind before it can read it"
        );
        assert!(
            super::EVIDENCE_KINDS.contains(&"settler_capture_escape"),
            "a safety rewrite must reach the postcondition verifier"
        );
    }

    #[test]
    fn a_live_arm_can_force_only_a_ledger_held_live_treatment() {
        // The exact held live genes move with the deployment selection.
        let held_live = civvis::ai::gene_ledger::ledger_held_live_treatments()
            .first()
            .copied()
            .expect("the explicit genome leaves live treatments off");
        let forced = super::forced_live_treatments(&[held_live.to_string(), held_live.to_string()])
            .expect("a named ledger-held gene is a valid live arm");
        assert_eq!(forced, vec![held_live], "the arm is deduplicated");

        let other_held_live = civvis::ai::gene_ledger::ledger_held_live_treatments()
            .iter()
            .copied()
            .find(|tag| *tag != held_live)
            .expect("the explicit genome leaves more than one live treatment off");

        let mut ai = civvis::ai::AdvancedAi::new();
        super::configure_live_bridge(&mut ai, &forced, &[])
            .expect("the validated arm configures the live controller");
        assert!(
            civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&forced)
                .contains(&held_live),
            "the requested gene is restored in the arm's genome"
        );
        assert!(
            !civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&forced)
                .contains(&other_held_live),
            "another held gene stays off until the experiment names it"
        );

        let host_only = super::forced_live_treatments(&["parallel-settlers".to_string()])
            .expect_err("a treatment already in the live genome cannot form a force-on arm");
        assert!(host_only.contains("already ships"), "{host_only}");

        // A held-off opt-in is the other thing an arm may name: it is the
        // only route a gene priced on the arena has to the live board
        // before the whole-game screen answers.
        let opt_in = civvis::ai::gene_ledger::ledger_held_opt_ins()
            .first()
            .copied()
            .expect("the registry holds opt-ins the ledger has not turned on");
        let seated = super::forced_live_treatments(&[opt_in.to_string()])
            .expect("a held-off opt-in is a valid live arm");
        assert_eq!(seated, vec![opt_in]);
        let mut seat = civvis::ai::AdvancedAi::new();
        super::configure_live_bridge(&mut seat, &seated, &[])
            .expect("the opt-in arm configures the live controller");
        assert!(
            civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&seated)
                .contains(&opt_in),
            "the arm's genome names the opt-in it seated"
        );
        let mut ordinary = civvis::ai::AdvancedAi::new();
        super::configure_live_bridge(&mut ordinary, &[], &[])
            .expect("the ordinary live seat configures");
        assert!(
            !civvis::ai::gene_ledger::deployment_treatments().contains(&opt_in),
            "and seating it changes nothing about the deployment genome"
        );
        let unknown = super::forced_live_treatments(&["no-such-treatment".to_string()])
            .expect_err("a typo cannot silently become the deployment arm");
        assert!(unknown.contains(held_live), "{unknown}");
    }

    use super::*;
    use civvis::mirror::{
        Plot, Snapshot, StateActivationPlot, StateCity, StateDistrict, StateEmergency,
        StateEmergencyOurs, StateEmergencyScore, StateGovernor, StateGreatPerson, StateGreatWork,
        StateMinor, StateRival, StateSnapshot, StateTradeRoute, StateUnit, TilesChunk,
    };

    fn default_decision_arm() -> DecisionArm<'static> {
        DecisionArm {
            war_from_plan: false,
            forced_on: &[],
            withheld: &[],
        }
    }

    #[test]
    fn host_peace_backoff_matches_firaxis_retry_window() {
        let mut retries = HostPeaceRetries::default();
        let mut state = StateSnapshot {
            turn: 58,
            rivals: vec![StateRival {
                player: 2,
                at_war: true,
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let peace = || Order {
            kind: "peace",
            subject: Some(2),
            verb: Some("MAKE_PEACE".to_string()),
            pos: None,
        };

        let (orders, deferred) = defer_host_peace_retries(vec![peace()], &state, &mut retries);
        assert_eq!(orders.len(), 1, "the first host request is legal");
        assert!(deferred.is_empty());

        for turn in 59..63 {
            state.turn = turn;
            let (orders, deferred) = defer_host_peace_retries(vec![peace()], &state, &mut retries);
            assert!(
                orders.is_empty(),
                "turn {turn} remains inside host cooldown"
            );
            assert_eq!(deferred, vec![2]);
        }

        state.turn = 63;
        let (orders, deferred) = defer_host_peace_retries(vec![peace()], &state, &mut retries);
        assert_eq!(orders.len(), 1, "the fifth turn is legal again");
        assert!(deferred.is_empty());
    }

    #[test]
    fn an_affordable_direct_city_state_war_peaces_before_envoys() {
        let mut state = StateSnapshot {
            turn: 58,
            envoys_free: Some(4),
            // This is the current Suzerain, but we are at peace with it.  The
            // city-state war is therefore direct and a minor peace can release it.
            rivals: vec![StateRival {
                player: 2,
                at_war: false,
                ..StateRival::default()
            }],
            minors: vec![StateMinor {
                player: 9,
                civ: "CIVILIZATION_GENEVA".to_string(),
                at_war: true,
                suzerain: 2,
                envoys: 1,
                most_envoys: 4,
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let mut proposed = Vec::new();
        assert_eq!(
            queue_city_state_envoy_reclaim(&mut proposed, &state),
            Some((9, 4)),
            "four held envoys raise our one to five and take Geneva"
        );
        assert_eq!(proposed.len(), 1);
        assert_eq!(proposed[0].kind, "peace");
        assert_eq!(proposed[0].subject, Some(9));
        assert!(
            proposed
                .iter()
                .all(|order| !(order.kind == "envoy" && order.subject == Some(9))),
            "Firaxis cannot accept an envoy to the city-state until a later state frame"
        );

        // A direct minor peace uses the same host cooldown as a major.  Without
        // this the next bridge frame would re-submit an operation Firaxis has
        // already accepted but not yet reflected.
        let mut retries = HostPeaceRetries::default();
        let (sent, deferred) = defer_host_peace_retries(proposed, &state, &mut retries);
        assert_eq!(sent.len(), 1);
        assert!(deferred.is_empty());

        state.turn = 59;
        let mut retry = Vec::new();
        assert_eq!(
            queue_city_state_envoy_reclaim(&mut retry, &state),
            Some((9, 4))
        );
        let (sent, deferred) = defer_host_peace_retries(retry, &state, &mut retries);
        assert!(sent.is_empty(), "the host has not confirmed peace yet");
        assert_eq!(deferred, vec![9]);

        // Once a fresh export says the war ended, retry memory clears and the
        // normal live planner may legally choose its envoy placements.
        state.turn = 60;
        state.minors[0].at_war = false;
        let _ = defer_host_peace_retries(Vec::new(), &state, &mut retries);
        assert!(retries.permits(9, state.turn));
    }

    #[test]
    fn submitted_city_state_reclaim_reserves_only_the_suzerainty_cost() {
        let envoy = |subject| Order {
            kind: "envoy",
            subject: Some(subject),
            verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
            pos: None,
        };
        let mut orders = vec![
            Order {
                kind: "peace",
                subject: Some(9),
                verb: Some("MAKE_PEACE".to_string()),
                pos: None,
            },
            envoy(7),
            // The planner should never make this during war, but preserving it
            // would defeat the host-boundary guarantee the reserve provides.
            envoy(9),
            envoy(8),
            envoy(10),
        ];
        assert_eq!(
            reserve_envoys_for_submitted_reclaim(&mut orders, 9, 5, 3),
            2,
            "one invalid target and one generic surplus order wait for the next frame"
        );
        let envoys: Vec<i64> = orders
            .iter()
            .filter(|order| order.kind == "envoy")
            .filter_map(|order| order.subject)
            .collect();
        assert_eq!(envoys, vec![7, 8], "two surplus envoys still invest now");
        assert!(
            orders
                .iter()
                .any(|order| order.kind == "peace" && order.subject == Some(9)),
            "the submitted peace itself remains intact"
        );
    }

    #[test]
    fn city_state_envoy_reclaim_skips_derived_wars_and_short_pools() {
        let mut state = StateSnapshot {
            turn: 58,
            envoys_free: Some(3),
            rivals: vec![StateRival {
                player: 2,
                at_war: false,
                ..StateRival::default()
            }],
            minors: vec![StateMinor {
                player: 9,
                civ: "CIVILIZATION_GENEVA".to_string(),
                at_war: true,
                suzerain: 2,
                envoys: 1,
                most_envoys: 4,
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let mut proposed = Vec::new();
        assert_eq!(
            queue_city_state_envoy_reclaim(&mut proposed, &state),
            None,
            "do not end a war when the pool still cannot win the city-state"
        );
        assert!(proposed.is_empty());

        state.envoys_free = Some(4);
        state.rivals[0].at_war = true;
        assert_eq!(
            queue_city_state_envoy_reclaim(&mut proposed, &state),
            None,
            "a city-state follows its hostile Suzerain; direct peace would not unlock envoys"
        );
        assert!(proposed.is_empty());
    }

    #[test]
    fn planned_major_peace_reserves_envoys_to_reclaim_its_derived_city_state() {
        let state = StateSnapshot {
            turn: 58,
            envoys_free: Some(5),
            rivals: vec![StateRival {
                player: 2,
                at_war: true,
                ..StateRival::default()
            }],
            minors: vec![StateMinor {
                player: 9,
                civ: "CIVILIZATION_GENEVA".to_string(),
                at_war: true,
                suzerain: 2,
                envoys: 1,
                most_envoys: 4,
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let peace = Order {
            kind: "peace",
            subject: Some(2),
            verb: Some("MAKE_PEACE".to_string()),
            pos: None,
        };
        let envoy = |subject| Order {
            kind: "envoy",
            subject: Some(subject),
            verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
            pos: None,
        };
        let mut proposed = vec![peace, envoy(7), envoy(9)];

        assert_eq!(
            planned_suzerain_peace_envoy_reclaim(&proposed, &state),
            Some((2, 9, 4)),
            "the existing peace offer releases Geneva only after it succeeds"
        );
        let mut retries = HostPeaceRetries::default();
        let (submitted, deferred_peace) = defer_host_peace_retries(proposed, &state, &mut retries);
        assert!(deferred_peace.is_empty());
        proposed = submitted;
        assert_eq!(
            reserve_envoys_for_submitted_reclaim(&mut proposed, 9, 5, 4),
            1,
            "the invalid envoy to the still-hostile minor waits with the four-token claim"
        );
        let retained: Vec<i64> = proposed
            .iter()
            .filter(|order| order.kind == "envoy")
            .filter_map(|order| order.subject)
            .collect();
        assert_eq!(
            retained,
            vec![7],
            "the one surplus envoy may still invest now"
        );
        assert!(
            proposed
                .iter()
                .any(|order| order.kind == "peace" && order.subject == Some(2)),
            "the planner's major peace, rather than a fabricated city-state peace, crosses the host boundary"
        );

        let no_peace = vec![envoy(7)];
        assert_eq!(
            planned_suzerain_peace_envoy_reclaim(&no_peace, &state),
            None,
            "a valuable city-state alone never asks a major to end a campaign"
        );
        let conflicting_orders = vec![
            Order {
                kind: "peace",
                subject: Some(2),
                verb: Some("MAKE_PEACE".to_string()),
                pos: None,
            },
            Order {
                kind: "war",
                subject: Some(2),
                verb: Some("DECLARE".to_string()),
                pos: None,
            },
        ];
        assert_eq!(
            planned_suzerain_peace_envoy_reclaim(&conflicting_orders, &state),
            None,
            "a contradictory order set cannot turn an Envoy reserve into a war-ending decision"
        );
    }

    #[test]
    fn direct_city_state_reclaim_crosses_the_host_boundary_as_peace_only() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 58,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 58,
            military: 100.0,
            envoys_free: Some(4),
            cities: vec![StateCity {
                id: 65_536,
                name: "Rome".to_string(),
                x: 2,
                y: 2,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            rivals: vec![StateRival {
                player: 2,
                at_war: false,
                ..StateRival::default()
            }],
            minors: vec![StateMinor {
                player: 9,
                civ: "CIVILIZATION_GENEVA".to_string(),
                at_war: true,
                military: 1.0,
                suzerain: 2,
                envoys: 1,
                most_envoys: 4,
                cities: vec![StateCity {
                    id: 65_537,
                    name: "Geneva".to_string(),
                    x: 8,
                    y: 8,
                    pop: 3,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
        let mut ai = civvis::ai::AdvancedAi::new();
        let mut ours = std::collections::BTreeMap::new();
        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut ours,
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .expect("the decision is JSON");
        let orders = reply["orders"].as_array().expect("orders are an array");
        assert!(
            orders
                .iter()
                .any(|order| { order["kind"] == "peace" && order["subject"] == 9 }),
            "an affordable direct city-state war must request peace: {reply}"
        );
        assert!(
            orders
                .iter()
                .all(|order| { !(order["kind"] == "envoy" && order["subject"] == 9) }),
            "the same host batch must not try to envoy a still-warring city-state: {reply}"
        );
        assert!(
            reply["note"]
                .as_str()
                .is_some_and(|note| note.contains("envoy_reclaim_peace=9 needed=4")),
            "the bridge must identify this intentional peace-to-envoy handoff: {reply}"
        );

        // The host resolves the peace asynchronously.  The first later state
        // that says it succeeded is where the ordinary envoy planner regains
        // legal access to Geneva and spends the prepared pool.
        let mut confirmed_state = state.clone();
        confirmed_state.turn = 59;
        confirmed_state.minors[0].at_war = false;
        let mut confirmed_mirror =
            civvis::mirror::LiveMirror::new(&snapshot, &confirmed_state, 3, 1, 250, 0);
        let mut confirmed_ai = civvis::ai::AdvancedAi::new();
        let mut confirmed_ours = std::collections::BTreeMap::new();
        let confirmed: serde_json::Value = serde_json::from_str(&decide(
            &mut confirmed_mirror,
            &mut confirmed_ai,
            &snapshot,
            &confirmed_state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut confirmed_ours,
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .expect("the confirmed decision is JSON");
        let envoys = confirmed["orders"]
            .as_array()
            .expect("confirmed orders are an array")
            .iter()
            .filter(|order| order["kind"] == "envoy" && order["subject"] == 9)
            .count();
        assert_eq!(
            envoys, 4,
            "the confirmed peace frame spends the full suzerainty pool: {confirmed}"
        );
    }

    /// Run civvis-20260826T184456Z: every one of the 31 refused city strikes
    /// was a replan-frame re-issue by a city whose frame-0 strike had landed.
    /// The export carries no per-city "attacks remaining", so the decider's
    /// own earlier frames are the record, and every board built for the turn
    /// must read it.
    #[test]
    fn a_city_that_fired_on_an_earlier_frame_holds_its_strike_for_the_turn() {
        let (snapshot, mut state) = production_board();
        state.turn = 40;
        let city = &mut state.cities[0];
        city.producing = None;
        city.buildings = vec!["BUILDING_WALLS".to_string()];
        city.damage = 0.0;
        city.max_damage = 200.0;
        city.wall_damage = 0.0;
        city.max_wall_damage = 100.0;
        let host_city = state.cities[0].id;
        let strike = |kind: &str| IssuedOrder {
            kind: kind.to_string(),
            subject: Some(host_city),
            verb: None,
            pos: Some((5, 6)),
        };
        let can_strike = |mirror: &civvis::mirror::LiveMirror| {
            let city = mirror.cid_of[&host_city];
            mirror.game.city_can_strike(&mirror.game.cities[&city])
        };

        let mut strikes = HostCityStrikes::default();
        strikes.observe(40, &[strike("city_strike")]);

        let mut replan = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        assert!(
            can_strike(&replan),
            "precondition: a walled city on a fresh board may strike"
        );
        strikes.apply(&mut replan, 40);
        assert!(
            !can_strike(&replan),
            "the frame-0 strike is spent on the replan frame's board"
        );
        let city = replan.cid_of[&host_city];
        assert_eq!(replan.game.cities[&city].extra_strikes_used, 0);
        assert!(
            !replan.game.cities[&city].encampment_struck,
            "a city strike does not spend the encampment's"
        );

        // A second strike on the same turn (Victor's Embrasure) is counted so
        // `city_can_strike` reads the extra-strike rule exactly as it would
        // for strikes applied on the board itself.
        strikes.observe(40, &[strike("city_strike"), strike("encampment_strike")]);
        let mut third = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        strikes.apply(&mut third, 40);
        let city = third.cid_of[&host_city];
        assert_eq!(third.game.cities[&city].extra_strikes_used, 1);
        assert!(third.game.cities[&city].encampment_struck);

        // The next turn's board starts with the strike available again, and
        // observing the new turn forgets the old one.
        let mut next = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        strikes.apply(&mut next, 41);
        assert!(
            can_strike(&next),
            "a board built for the next turn is untouched"
        );
        strikes.observe(41, &[]);
        let mut fresh = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        strikes.apply(&mut fresh, 40);
        assert!(can_strike(&fresh), "the old turn's strikes are forgotten");
    }

    #[test]
    fn fresh_board_retains_firaxis_city_repair_cooldown_after_damage() {
        let (snapshot, mut state) = production_board();
        state.turn = 40;
        let city = &mut state.cities[0];
        city.producing = None;
        city.buildings = vec!["BUILDING_WALLS".to_string()];
        city.damage = 0.0;
        city.max_damage = 200.0;
        city.wall_damage = 0.0;
        city.max_wall_damage = 100.0;

        let mut cooldowns = HostCityAttackCooldowns::default();
        cooldowns.observe(&state);

        // This mirrors Ravenna on the measured t125 frame: its wall and
        // garrison both took a fresh hit, so the host's repair project cannot
        // start even though a fresh model board normally permits it.
        state.turn = 41;
        state.cities[0].damage = 18.0;
        state.cities[0].wall_damage = 68.0;
        cooldowns.observe(&state);

        let repair = civvis::game::Item::Project {
            project: civvis::name::Name::new("repair_outer_defenses"),
        };
        let unchecked = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let city = unchecked.cid_of[&7];
        assert!(
            unchecked.game.can_produce(0, city, &repair),
            "precondition: a fresh board alone loses the host hit timestamp"
        );

        let mut tracked = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        cooldowns.apply(&mut tracked);
        let city = tracked.cid_of[&7];
        assert_eq!(tracked.game.cities[&city].last_attacked, 41);
        assert!(
            !tracked.game.can_produce(0, city, &repair),
            "the host cooldown must prevent the repair Civ 6 would reject"
        );

        // No further damage leaves the last-hit turn intact. Firaxis permits
        // the repair again on the third later turn, matching `Game::can_produce`.
        for turn in 42..=44 {
            state.turn = turn;
            cooldowns.observe(&state);
        }
        let mut cooled = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        cooldowns.apply(&mut cooled);
        let city = cooled.cid_of[&7];
        assert_eq!(cooled.game.cities[&city].last_attacked, 41);
        assert!(
            cooled.game.can_produce(0, city, &repair),
            "the exact three-turn Firaxis cooldown must eventually expire"
        );
    }

    /// One land patch is enough: the override reads cities, not terrain.
    /// ★★★★ A move the host accepts and never performs is a fact about the
    /// ground, and it must reach the mirror's speculative frontier.
    ///
    /// Run civvis-20260816T093036Z: the only Scout stood on (12,12) for 28
    /// consecutive turns of `MOVE_TO (9,11)` with no `move_refused` and 193 plots
    /// revealed by t50. The mirror kept assuming the unrevealed (9,11) traversable
    /// on every rebuild, so every replan chose the same dead line. Here a scout is
    /// ordered from (4,5) onto the unknown ring beyond the revealed 8×8, is still
    /// standing on (4,5) next turn, and the destination stops being assumed
    /// traversable — while a same-turn frame keeps the memory, a unit that DID
    /// move proves nothing, and revealed terrain is never overridden.
    #[test]
    fn a_frontier_plot_the_host_will_not_walk_onto_stops_being_traversable() {
        let (snapshot, mut state) = production_board();
        state.turn = 13;
        state.units = vec![StateUnit {
            id: 900,
            kind: "UNIT_SCOUT".to_string(),
            x: 4,
            y: 5,
            hp: 100.0,
            moves: 3.0,
            ..StateUnit::default()
        }];
        // (8,5) is the first unrevealed column past the 8×8 board: frontier ground.
        let dead_dest = (8, 5);
        let dead_pos = civvis::hex::offset_to_axial(dead_dest.0, dead_dest.1);
        let seen_pos = civvis::hex::offset_to_axial(6, 5);

        let mut refusals = HostMoveRefusals::default();
        let orders = vec![
            Order {
                kind: "unit",
                subject: Some(900),
                verb: Some("MOVE_TO".to_string()),
                pos: Some(dead_dest),
            },
            // A non-move for the same unit and a move for an unknown unit are noise.
            Order {
                kind: "unit",
                subject: Some(900),
                verb: Some("FORTIFY".to_string()),
                pos: None,
            },
            Order {
                kind: "unit",
                subject: Some(901),
                verb: Some("MOVE_TO".to_string()),
                pos: Some((7, 7)),
            },
        ];
        refusals.record(&orders, &state, &std::collections::BTreeMap::new());
        assert_eq!(
            refusals.sent.len(),
            1,
            "only the scout's move is remembered"
        );

        // A second frame on the SAME turn cannot judge the move yet and must not
        // forget it.
        refusals.observe(&state);
        assert_eq!(refusals.sent.len(), 1, "a same-turn frame keeps the memory");
        assert!(refusals.dead.is_empty());

        // Next turn, still on the tile it was ordered from: the destination is dead.
        state.turn = 14;
        refusals.observe(&state);
        assert!(refusals.sent.is_empty(), "a judged move is consumed");
        assert_eq!(
            refusals.dead.iter().copied().collect::<Vec<_>>(),
            vec![dead_dest],
            "a unit that never left its tile proves the destination unwalkable"
        );

        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
        assert_eq!(mirror.game.map.tiles[&dead_pos].terrain.as_str(), "unknown");
        assert!(
            mirror.game.map.tiles[&dead_pos].assumed_traversable,
            "fixture precondition: the frontier plot starts out assumed traversable"
        );
        refusals.apply(&mut mirror);
        assert!(
            !mirror.game.map.tiles[&dead_pos].assumed_traversable,
            "the proved-dead frontier plot stops being assumed traversable"
        );
        assert!(
            !mirror
                .game
                .rules
                .is_passable(&mirror.game.map.tiles[&dead_pos]),
            "and the planner's paths route around it"
        );

        // A move that DID progress proves nothing, and a revealed plot is never
        // overridden even if it once sat in the dead set.
        refusals.record(
            &[Order {
                kind: "unit",
                subject: Some(900),
                verb: Some("MOVE_TO".to_string()),
                pos: Some((6, 5)),
            }],
            &state,
            &std::collections::BTreeMap::new(),
        );
        state.turn = 15;
        state.units[0].x = 5;
        refusals.observe(&state);
        assert_eq!(
            refusals.dead.len(),
            1,
            "a unit that moved marks nothing dead"
        );
        refusals.dead.insert((6, 5));
        let before = mirror.game.map.tiles[&seen_pos].assumed_traversable;
        refusals.apply(&mut mirror);
        assert_eq!(
            mirror.game.map.tiles[&seen_pos].terrain.as_str(),
            "grassland"
        );
        assert_eq!(
            mirror.game.map.tiles[&seen_pos].assumed_traversable, before,
            "revealed terrain keeps its own truth"
        );
        assert!(mirror
            .game
            .rules
            .is_passable(&mirror.game.map.tiles[&seen_pos]));
    }

    /// A coalesced `MOVE_TO` keeps the host fast, but the failure feedback must
    /// verify the first speculative hop that was hidden inside it. Otherwise a
    /// rejected three-hex Scout route either retires an unproven valid tile or
    /// keeps choosing the same blocked opening again.
    #[test]
    fn a_stalled_coalesced_walk_probes_then_retires_its_first_unknown_step() {
        let (snapshot, mut state) = production_board();
        state.turn = 13;
        state.units = vec![StateUnit {
            id: 900,
            kind: "UNIT_SCOUT".to_string(),
            x: 6,
            y: 5,
            hp: 100.0,
            moves: 3.0,
            ..StateUnit::default()
        }];
        // The first local step is known ground. The second is the first unknown
        // frontier plot, then the coalescer carries the Scout a further hex.
        let planned = vec![
            unit_order(900, "MOVE_TO", Some((7, 5))),
            unit_order(900, "MOVE_TO", Some((8, 5))),
            unit_order(900, "MOVE_TO", Some((9, 5))),
        ];
        let probes = first_unknown_coalesced_steps(&planned, &snapshot);
        assert_eq!(probes.get(&900), Some(&(8, 5)));

        let (orders, deferred, coalesced) = coalesce_unit_paths(planned, false);
        assert_eq!(deferred, 0);
        assert_eq!(coalesced, 2);
        assert_eq!(
            orders[0].pos,
            Some((9, 5)),
            "the host still gets the full walk"
        );

        let mut refusals = HostMoveRefusals::default();
        refusals.record(&orders, &state, &probes);
        state.turn = 14;
        refusals.observe(&state);
        assert!(
            refusals.dead.is_empty(),
            "a long route has not proved which unknown hop blocked it"
        );
        assert_eq!(
            refusals
                .pending_probes
                .get(&900)
                .map(|probe| (probe.from, probe.target)),
            Some(((6, 5), (8, 5))),
            "the next outgoing move must test the first unknown hop exactly"
        );

        // The next plan may again want the distant destination, but the bridge
        // reduces just this known-failed route to its exact frontier probe.
        let mut retry = vec![unit_order(900, "MOVE_TO", Some((9, 5)))];
        assert_eq!(
            refusals.cap_pending_frontier_moves(&mut retry, &state, &probes),
            1
        );
        assert_eq!(retry[0].pos, Some((8, 5)));
        refusals.record(&retry, &state, &std::collections::BTreeMap::new());
        state.turn = 15;
        refusals.observe(&state);
        assert_eq!(
            refusals.dead.iter().copied().collect::<Vec<_>>(),
            vec![(8, 5)],
            "only a failed exact probe retires the hidden unknown hop"
        );

        // A later replan with another frontier opening must remain its own
        // decision; stale feedback may narrow a repeated route, never replace
        // a new tactical direction.
        refusals.pending_probes.insert(
            900,
            HostFrontierProbe {
                from: (6, 5),
                target: (8, 5),
            },
        );
        let mut different_route = vec![unit_order(900, "MOVE_TO", Some((7, 6)))];
        let different_steps = std::collections::BTreeMap::from([(900, (7, 6))]);
        assert_eq!(
            refusals.cap_pending_frontier_moves(&mut different_route, &state, &different_steps),
            0
        );
        assert_eq!(different_route[0].pos, Some((7, 6)));
        assert!(
            refusals.pending_probes.is_empty(),
            "a changed route clears the stale probe instead of being overwritten"
        );

        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
        let first_unknown = civvis::hex::offset_to_axial(8, 5);
        let host_destination = civvis::hex::offset_to_axial(9, 5);
        assert!(mirror.game.map.tiles[&first_unknown].assumed_traversable);
        assert!(mirror.game.map.tiles[&host_destination].assumed_traversable);
        refusals.apply(&mut mirror);
        assert!(
            !mirror.game.map.tiles[&first_unknown].assumed_traversable,
            "the next fresh mirror routes around the exact failed frontier hop"
        );
        assert!(
            mirror.game.map.tiles[&host_destination].assumed_traversable,
            "unproven downstream frontier remains available to future exploration"
        );
    }

    #[test]
    fn a_same_turn_replan_drops_stale_unit_replays_but_keeps_new_tactics() {
        let (_snapshot, mut state) = production_board();
        state.turn = 42;
        state.frame = 0;
        let mut refusals = HostMoveRefusals::default();
        let mut first = vec![
            unit_order(7, "MOVE_TO", Some((3, 3))),
            unit_order(7, "FORTIFY", None),
            unit_order(8, "CAPTURE", Some((4, 4))),
        ];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut first, &state),
            0,
            "the first frame is allowed to issue its complete plan"
        );
        assert_eq!(first.len(), 3);

        state.frame = 1;
        let mut replan = vec![
            unit_order(7, "MOVE_TO", Some((3, 3))),
            unit_order(7, "FORTIFY", None),
            unit_order(8, "CAPTURE", Some((4, 4))),
            // A combat frame may legitimately need another strike at the same
            // target, so attack verbs are intentionally outside the filter.
            unit_order(9, "ATTACK", Some((5, 5))),
            // A changed destination is a new route decision and must survive.
            unit_order(7, "MOVE_TO", Some((3, 4))),
        ];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut replan, &state),
            3,
            "only exact stale MOVE_TO/CAPTURE/FORTIFY replays are removed"
        );
        assert_eq!(replan.len(), 2);
        assert!(replan.iter().any(|order| {
            order.subject == Some(9)
                && order.verb.as_deref() == Some("ATTACK")
                && order.pos == Some((5, 5))
        }));
        assert!(replan.iter().any(|order| {
            order.subject == Some(7)
                && order.verb.as_deref() == Some("MOVE_TO")
                && order.pos == Some((3, 4))
        }));

        // The suppression scope is one turn. A fresh turn may retry the old
        // destination using the newly authoritative host state.
        state.turn = 43;
        state.frame = 0;
        let mut next_turn = vec![unit_order(7, "MOVE_TO", Some((3, 3)))];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut next_turn, &state),
            0
        );
        assert_eq!(next_turn.len(), 1);
    }

    #[test]
    fn a_same_turn_replan_drops_exact_deal_replays_but_keeps_changed_offers() {
        let (_snapshot, mut state) = production_board();
        state.turn = 44;
        state.frame = 0;
        let mut refusals = HostMoveRefusals::default();
        let mut first = vec![
            Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("RESOURCE_HORSES=10".to_string()),
                pos: Some((200, 0)),
            },
            Order {
                kind: "buy",
                subject: Some(3),
                verb: Some("OPEN_BORDERS".to_string()),
                pos: Some((60, 0)),
            },
        ];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut first, &state),
            0,
            "the first frame is allowed to issue its complete deal plan"
        );

        state.frame = 1;
        let mut replan = vec![
            Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("RESOURCE_HORSES=10".to_string()),
                pos: Some((200, 0)),
            },
            Order {
                kind: "buy",
                subject: Some(3),
                verb: Some("OPEN_BORDERS".to_string()),
                pos: Some((60, 0)),
            },
            Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("RESOURCE_HORSES=10".to_string()),
                pos: Some((220, 0)),
            },
            // Non-deal orders retain their existing same-turn semantics.
            Order {
                kind: "research",
                subject: None,
                verb: Some("TECH_WRITING".to_string()),
                pos: None,
            },
        ];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut replan, &state),
            2,
            "only exact sell/buy replays are removed"
        );
        assert_eq!(replan.len(), 2);
        assert!(replan.iter().any(|order| {
            order.kind == "sell" && order.subject == Some(2) && order.pos == Some((220, 0))
        }));
        assert!(replan.iter().any(|order| {
            order.kind == "research" && order.verb.as_deref() == Some("TECH_WRITING")
        }));

        state.turn = 45;
        state.frame = 0;
        let mut next_turn = vec![Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("RESOURCE_HORSES=10".to_string()),
            pos: Some((200, 0)),
        }];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut next_turn, &state),
            0,
            "a fresh turn may retry the old offer after the host's cooldown"
        );
    }

    fn production_board() -> (Snapshot, StateSnapshot) {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1,
            width: 20,
            height: 20,
            chunk: 1,
            // `Plot` carries serde defaults for everything but the coordinates, so
            // deserializing is cheaper than naming a dozen fields the test does not
            // care about — and it cannot drift as new ones are added.
            plots: (0..8)
                .flat_map(|x| (0..8).map(move |y| (x, y)))
                .map(|(x, y)| {
                    serde_json::from_value::<Plot>(serde_json::json!({
                        "x": x, "y": y, "t": "TERRAIN_GRASS"
                    }))
                    .expect("a plot with serde defaults deserializes")
                })
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 1,
            cities: vec![StateCity {
                id: 7,
                name: "Ottawa".to_string(),
                x: 4,
                y: 4,
                pop: 4,
                producing: Some("UNIT_WARRIOR".to_string()),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        (snapshot, state)
    }

    #[test]
    fn host_production_finish_gate_requires_authoritative_metrics() {
        let mut city = StateCity {
            producing: Some("UNIT_WARRIOR".to_string()),
            production: 8.0,
            production_cost: 40.0,
            production_progress: 32.0,
            production_turns: 1.0,
            ..StateCity::default()
        };
        assert!(host_production_finishes_this_turn(&city));

        city.production_progress = 10.0;
        city.production_turns = 4.0;
        assert!(!host_production_finishes_this_turn(&city));

        city.production_progress = -1.0;
        city.production_turns = -1.0;
        assert!(!host_production_finishes_this_turn(&city));
    }

    /// ⚠⚠ The defect: the mod ladder answers `ENDTURN_BLOCKING_PRODUCTION`, the next
    /// rebuild seeds that choice into CIVVIS's queue as work in progress, and CIVVIS —
    /// which only produces for a city whose queue is empty — says nothing. Measured at
    /// 13% of turns carrying a produce order against 100% carrying research and civic.
    /// The largest waste class on the ladder must say WHICH unit it is.
    ///
    /// `self_tile_move` totalled 25,387 across 43 live runs as one anonymous
    /// number. Named, five replayed runs answered it in one pass: 249 events,
    /// **100% military**, zero settlers — which is the opposite of what the
    /// expansion-tempo reading predicted and sends the repair somewhere else.
    ///
    /// ⚠ Two-sided: the key must carry the kind AND keep the `self_tile_move`
    /// prefix, because existing readers match on that prefix and a bare total
    /// has to stay recoverable.
    #[test]
    fn a_self_tile_move_names_the_unit_that_asked_for_it() {
        let (snapshot, state) = local_barbarian_defense_board();
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        // ⚠ DISCOVERED, not hardcoded: the point is that whatever unit the board
        // carries, its kind reaches the key. Naming one would make this fail on a
        // fixture change rather than on anything to do with the instrument.
        let unit = *mirror
            .game
            .units
            .keys()
            .next()
            .expect("this board carries units");
        let kind = mirror.game.units[&unit].kind.to_string();
        assert!(
            !kind.is_empty(),
            "fixture precondition: the unit has a kind"
        );

        let key = self_tile_move_key(&mirror, unit);
        assert!(
            key.starts_with("self_tile_move"),
            "the prefix is load-bearing for every existing reader, got {key}"
        );
        assert_eq!(key, format!("self_tile_move:{kind}"));

        // A unit the bridge cannot map back still has to be counted, not dropped:
        // an unnamed refusal is the exact failure this key exists to end.
        let missing = mirror.game.units.keys().copied().max().unwrap_or(0) + 1_000;
        assert_eq!(self_tile_move_key(&mirror, missing), "self_tile_move");
    }

    /// ★★★★★ THE DEFERRED PRODUCTION HINT NEVER FIRED IN ANY RECORDED GAME.
    /// `append_next_production_hints` previews the next build on the board the
    /// agent has already taken its turn on — and `take_turn` ends with
    /// `EndTurn`, so `current` has moved to the next seat and every `Produce`
    /// the preview tries is refused "not your turn". Zero `produce_next` orders
    /// across the 2026-08-16..19 ladder; 70–174 mod-ladder `build` picks per
    /// game answered the production prompt instead, ~85% displaced by CIVVIS's
    /// own order one turn later. The preview must run on a board where it is
    /// still this seat's turn.
    #[test]
    fn the_next_production_hint_survives_the_seat_having_ended_its_turn() {
        let (snapshot, mut state) = production_board();
        // The exported city finishes its Warrior this turn.
        state.cities[0].production = 8.0;
        state.cities[0].production_cost = 40.0;
        state.cities[0].production_progress = 36.0;
        state.cities[0].production_turns = 1.0;
        assert!(host_production_finishes_this_turn(&state.cities[0]));
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let mut planned = mirror.game.clone();
        let mut ai = civvis::ai::AdvancedAi::new();
        ai.enable_live_bridge();
        // The agent takes its turn on the planning board, exactly as `decide`
        // does, and the board's seat moves on.
        ai.take_turn(&mut planned, 0);
        assert_ne!(
            planned.current, 0,
            "precondition: the seat has ended its turn"
        );

        let mut orders = Vec::new();
        let mut ours = std::collections::BTreeMap::new();
        let hinted =
            append_next_production_hints(&ai, &planned, &mirror, &state, &mut orders, &mut ours);
        assert_eq!(
            hinted,
            1,
            "the finishing city gets a hint: {:?}",
            orders.iter().map(Order::to_json).collect::<Vec<_>>()
        );
        let hint = orders
            .iter()
            .find(|order| order.kind == "produce_next")
            .expect("a produce_next order");
        assert_eq!(hint.subject, Some(7));
        assert!(hint.verb.as_deref().is_some_and(|v| !v.is_empty()));
        assert!(
            ours.get(&7)
                .is_some_and(|value| value.starts_with(DEFERRED_PRODUCTION_PREFIX)),
            "the lease is remembered until the host consumes it: {ours:?}"
        );
        // And the preview never touched the caller's board.
        assert_ne!(planned.current, 0);
    }

    #[test]
    fn a_build_civvis_did_not_choose_is_handed_back_to_it() {
        let (snapshot, state) = production_board();
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let cid = *mirror
            .cid_of
            .get(&7)
            .expect("the exported city is mirrored");
        let mut planned = mirror.game.clone();
        assert!(
            !planned.cities[&cid].queue.is_empty(),
            "precondition: the rebuild seeds the queue from `producing`"
        );

        // Nothing recorded for this city, so the warrior is the ladder's pick.
        let ours = std::collections::BTreeMap::new();
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            1
        );
        assert!(
            planned.cities[&cid].queue.is_empty(),
            "a choice CIVVIS did not make must not read as its own plan"
        );
        assert!(
            !mirror.game.cities[&cid].queue.is_empty(),
            "and the authoritative mirror must be left as the last exported state"
        );
    }

    /// The other half, and what keeps this from re-creating the thrash the queue
    /// seeding was added to fix: once CIVVIS's own choice is what the city is
    /// building, the seed stands and it is left alone.
    #[test]
    fn civvis_own_choice_is_left_alone_so_the_override_is_self_limiting() {
        let (snapshot, state) = production_board();
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let cid = *mirror
            .cid_of
            .get(&7)
            .expect("the exported city is mirrored");
        let mut planned = mirror.game.clone();

        let mut ours = std::collections::BTreeMap::new();
        ours.insert(7_i64, "UNIT_WARRIOR".to_string());
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            0
        );
        assert!(
            !planned.cities[&cid].queue.is_empty(),
            "work in progress CIVVIS asked for must survive, or it re-chooses every turn"
        );
    }

    #[test]
    fn a_deferred_production_hint_preserves_a_slow_host_queue() {
        let (snapshot, mut state) = production_board();
        state.cities[0].production = 8.0;
        state.cities[0].production_cost = 40.0;
        state.cities[0].production_progress = 32.0;
        state.cities[0].production_turns = 1.0;
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let cid = *mirror
            .cid_of
            .get(&7)
            .expect("the exported city is mirrored");
        let mut planned = mirror.game.clone();
        let mut ours = std::collections::BTreeMap::from([(
            7_i64,
            format!("{DEFERRED_PRODUCTION_PREFIX}UNIT_SETTLER"),
        )]);

        assert_eq!(settle_deferred_production_hints(&mut ours, &state), (0, 0));
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            0,
            "a near-finished old queue stays intact until its blocker consumes the lease"
        );
        assert!(!planned.cities[&cid].queue.is_empty());

        state.cities[0].production_progress = 10.0;
        state.cities[0].production_turns = 4.0;
        assert_eq!(settle_deferred_production_hints(&mut ours, &state), (0, 1));
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            1,
            "a stale lease is released once the host is no longer approaching its blocker"
        );
    }

    /// ★★★★ A DECIDER RESTARTED MID-GAME MUST NOT HAND BACK EVERY CITY. A fresh
    /// process holds an empty `ours`, so on its first turn all work in progress
    /// read as foreign and was re-decided (run civvis-20260816T213447Z, t139 and
    /// t157: the Taj Mahal at 188/460 dropped, seven Rangers ordered). Adopting
    /// the host's current item as ours on that first turn leaves the queue alone,
    /// keeps a plan this process already ordered, and skips idle cities.
    #[test]
    fn a_restarted_decider_adopts_the_hosts_production_as_its_own() {
        let (snapshot, mut state) = production_board();
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let cid = *mirror
            .cid_of
            .get(&7)
            .expect("the exported city is mirrored");
        let mut planned = mirror.game.clone();

        let mut ours = std::collections::BTreeMap::new();
        assert_eq!(
            adopt_host_production(&mut ours, &state),
            1,
            "one city building"
        );
        assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_WARRIOR"));
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            0,
            "the adopted item is not released"
        );
        assert!(!planned.cities[&cid].queue.is_empty());

        // An entry this process already ordered wins over the host's reading.
        let mut ours = std::collections::BTreeMap::new();
        ours.insert(7_i64, "UNIT_SETTLER".to_string());
        assert_eq!(adopt_host_production(&mut ours, &state), 0);
        assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_SETTLER"));

        // An idle city adopts nothing.
        state.cities[0].producing = None;
        let mut ours = std::collections::BTreeMap::new();
        assert_eq!(adopt_host_production(&mut ours, &state), 0);
        assert!(ours.is_empty());
    }

    /// A city building nothing has nothing to hand back, and must not be counted.
    #[test]
    fn an_idle_city_is_not_reported_as_released() {
        let (snapshot, mut state) = production_board();
        state.cities[0].producing = None;
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let mut planned = mirror.game.clone();
        let ours = std::collections::BTreeMap::new();
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            0
        );
    }

    fn local_barbarian_defense_board() -> (Snapshot, StateSnapshot) {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| (x, y)))
                .map(|(x, y)| {
                    serde_json::from_value::<Plot>(serde_json::json!({
                        "x": x, "y": y, "t": "TERRAIN_GRASS"
                    }))
                    .expect("a plot with serde defaults deserializes")
                })
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 30,
            cities: vec![StateCity {
                id: 7,
                name: "Rome".to_string(),
                x: 4,
                y: 4,
                pop: 4,
                ..StateCity::default()
            }],
            units: vec![
                StateUnit {
                    id: 100,
                    kind: "UNIT_SLINGER".to_string(),
                    x: 4,
                    y: 5,
                    hp: 100.0,
                    moves: 2.0,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 101,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 5,
                    y: 5,
                    hp: 100.0,
                    moves: 2.0,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 102,
                    kind: "UNIT_SPEARMAN".to_string(),
                    x: 6,
                    y: 4,
                    hp: 100.0,
                    moves: 2.0,
                    ..StateUnit::default()
                },
            ],
            hostiles: vec![StateUnit {
                kind: "UNIT_WARRIOR".to_string(),
                x: 5,
                y: 4,
                hp: 64.0,
                combat: 20.0,
                moves: 0.0,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        (snapshot, state)
    }

    /// The same exposed-defender geometry, but a current war against a rival
    /// supplies the defender rather than the aggregate barbarian export.
    fn local_war_enemy_board() -> (Snapshot, StateSnapshot) {
        let (snapshot, mut state) = local_barbarian_defense_board();
        let hostile = state
            .hostiles
            .pop()
            .expect("the local defense fixture carries one barbarian");
        state.rivals.push(StateRival {
            player: 3,
            at_war: true,
            units: vec![hostile],
            ..StateRival::default()
        });
        (snapshot, state)
    }

    #[test]
    fn local_barbarian_finishing_volley_keeps_exported_hp_and_proves_the_kill() {
        let (snapshot, state) = local_barbarian_defense_board();
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let barbarian = mirror.game.barb_pid.unwrap();
        let hostile = mirror
            .game
            .units
            .values()
            .find(|unit| unit.owner == barbarian)
            .unwrap()
            .id;
        let mut planned = mirror.game.clone();

        let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
        assert_eq!(volley.targets, 1);
        assert!(
            volley.actions.len() >= 2,
            "the host/model safety margin keeps at least two direct attackers: {:?}",
            volley.actions
        );
        assert!(
            !planned.units.contains_key(&hostile),
            "the exact planning model must prove the target dies before pre-empting movement"
        );
        assert_eq!(
            mirror.game.units[&hostile].hp, 64,
            "the exported board must retain Firaxis's actual damage"
        );
    }

    #[test]
    fn dying_barbarian_gets_a_direct_attack_and_a_reserve_before_any_movement() {
        let (snapshot, mut state) = local_barbarian_defense_board();
        state.hostiles[0].hp = 8.0;
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let mut ai = civvis::ai::AdvancedAi::new();
        let mut ours = std::collections::BTreeMap::new();

        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut ours,
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .unwrap();
        let attacks = reply["orders"]
            .as_array()
            .unwrap()
            .iter()
            .take_while(|order| {
                matches!(order["verb"].as_str(), Some("ATTACK" | "RANGE_ATTACK"))
                    && order["x"] == 5
                    && order["y"] == 4
            })
            .collect::<Vec<_>>();
        assert_eq!(
            attacks.len(),
            2,
            "one modelled kill and one host-side reserve must lead the order list: {}",
            reply["orders"]
        );
        assert!(reply["note"]
            .as_str()
            .unwrap()
            .contains("war_unit_finishing_volleys=1 attacks=2 reserves=1"));
    }

    #[test]
    fn dying_war_enemy_gets_a_direct_attack_and_a_reserve_before_any_movement() {
        let (snapshot, mut state) = local_war_enemy_board();
        state.rivals[0].units[0].hp = 8.0;
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let (target, owner, target_pos) = mirror
            .game
            .units
            .values()
            .find(|unit| unit.owner != 0)
            .map(|unit| (unit.id, unit.owner, unit.pos))
            .expect("the rival warrior is mirrored");
        assert_ne!(Some(owner), mirror.game.barb_pid);
        assert!(mirror.game.is_at_war(0, owner));

        let mut planned = mirror.game.clone();
        let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
        assert_eq!(volley.targets, 1);
        assert!(
            volley.actions.len() >= 2,
            "one modelled kill and one host-side reserve must be retained: {:?}",
            volley.actions
        );
        assert!(
            !planned.units.contains_key(&target),
            "a current-war unit on 8 health must be removed on the private board before movement"
        );

        let mut ai = civvis::ai::AdvancedAi::new();
        let mut ours = std::collections::BTreeMap::new();
        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut ours,
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .unwrap();
        let (x, y) = civvis::hex::axial_to_offset(target_pos.0, target_pos.1);
        let attacks = reply["orders"]
            .as_array()
            .unwrap()
            .iter()
            .take_while(|order| {
                matches!(order["verb"].as_str(), Some("ATTACK" | "RANGE_ATTACK"))
                    && order["x"] == x
                    && order["y"] == y
            })
            .collect::<Vec<_>>();
        assert_eq!(
            attacks.len(),
            2,
            "the finishing volley must lead every ordinary movement order: {}",
            reply["orders"]
        );
        assert!(reply["note"]
            .as_str()
            .unwrap()
            .contains("war_unit_finishing_volleys=1 attacks=2 reserves=1"));
    }

    #[test]
    fn a_wounded_peacetime_enemy_does_not_preempt_the_tactical_planner() {
        let (snapshot, mut state) = local_war_enemy_board();
        state.rivals[0].at_war = false;
        state.rivals[0].units[0].hp = 8.0;
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let target = mirror
            .game
            .units
            .values()
            .find(|unit| unit.owner != 0)
            .expect("the rival warrior is mirrored");
        assert!(!mirror.game.is_at_war(0, target.owner));
        let target = target.id;
        let mut planned = mirror.game.clone();

        let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
        assert_eq!(volley.targets, 0);
        assert!(volley.actions.is_empty());
        assert!(
            planned.units.contains_key(&target),
            "an enemy outside a current war must stay with the ordinary planner"
        );
    }

    #[test]
    fn healthy_barbarian_does_not_preempt_the_ordinary_tactical_planner() {
        let (snapshot, state) = local_barbarian_defense_board();
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let barbarian = mirror.game.barb_pid.unwrap();
        let hostile = mirror
            .game
            .units
            .values()
            .find(|unit| unit.owner == barbarian)
            .unwrap()
            .id;
        let mut planned = mirror.game.clone();
        planned.units.get_mut(&hostile).unwrap().hp = 100;

        let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
        assert_eq!(volley.targets, 0);
        assert!(volley.actions.is_empty());
        assert_eq!(planned.units[&hostile].hp, 100);
    }

    #[test]
    fn reachable_melee_finish_collapses_approach_and_attack_into_one_host_order() {
        let (snapshot, mut state) = local_barbarian_defense_board();
        state.hostiles[0].hp = 1.0;
        state.units.retain(|unit| unit.id == 102);
        let chariot = state.units.iter_mut().find(|unit| unit.id == 102).unwrap();
        chariot.kind = "UNIT_HEAVY_CHARIOT".to_string();
        chariot.x = 5;
        chariot.y = 6;
        chariot.moves = 4.0;

        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let chariot = mirror
            .civ6_of
            .iter()
            .find_map(|(unit, civ6)| (*civ6 == 102).then_some(*unit))
            .unwrap();
        let target = civvis::hex::offset_to_axial(5, 4);
        assert!(
            mirror.game.wdist(mirror.game.units[&chariot].pos, target) > 1,
            "the host-order collapse is only meaningful when an approach is required"
        );
        let mut planned = mirror.game.clone();

        let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
        assert_eq!(volley.targets, 1);
        assert_eq!(
            volley.actions,
            vec![Action::Attack {
                unit: chariot,
                target
            }],
            "the live host receives its native move-to-enemy operation, not a \
             deferred MOVE_TO followed by an attack next turn"
        );
        assert_eq!(
            planned.units[&chariot].pos, target,
            "the private CIVVIS board still simulates and scores the full approach"
        );
    }

    /// A move onto an enemy civilian crosses as `CAPTURE`, the attack-modifier
    /// MOVE_TO the host needs to enter an occupied plot; a move onto open
    /// ground stays `MOVE_TO`. Measured reason: 65 bare `MOVE_TO` orders at a
    /// barbarian-held settler's tile across 273 live runs, zero captures.
    #[test]
    fn a_move_onto_an_enemy_civilian_crosses_as_a_capture() {
        let (snapshot, mut state) = local_barbarian_defense_board();
        let settler = &mut state.hostiles[0];
        settler.kind = "UNIT_SETTLER".to_string();
        settler.combat = 0.0;
        settler.hp = 100.0;
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let barbarian = mirror.game.barb_pid.unwrap();
        assert!(
            mirror
                .game
                .units
                .values()
                .any(|unit| unit.owner == barbarian && unit.kind == "settler"),
            "the barbarian-held settler must reach the board"
        );
        let warrior = mirror
            .civ6_of
            .iter()
            .find_map(|(unit, civ6)| (*civ6 == 101).then_some(*unit))
            .unwrap();

        let capture = translate(
            &Action::Move {
                unit: warrior,
                to: civvis::hex::offset_to_axial(5, 4),
            },
            &mirror,
            &state,
        )
        .expect("the capture crosses");
        assert_eq!(capture.verb.as_deref(), Some("CAPTURE"));
        assert_eq!(capture.pos, Some((5, 4)));
        assert_eq!(capture.subject, Some(101));

        let walk = translate(
            &Action::Move {
                unit: warrior,
                to: civvis::hex::offset_to_axial(3, 3),
            },
            &mirror,
            &state,
        )
        .expect("the walk crosses");
        assert_eq!(walk.verb.as_deref(), Some("MOVE_TO"));
    }

    /// A capture is verified by the unit standing where the civilian stood,
    /// or by the civilian gone from there; a unit still beside it failed.
    #[test]
    fn a_capture_is_verified_by_arrival_or_by_the_civilian_gone() {
        let (tiles, mut before) = local_barbarian_defense_board();
        let settler = &mut before.hostiles[0];
        settler.kind = "UNIT_SETTLER".to_string();
        settler.combat = 0.0;
        let order = IssuedOrder {
            kind: "unit".to_string(),
            subject: Some(101),
            verb: Some("CAPTURE".to_string()),
            pos: Some((5, 4)),
        };

        let mut arrived = before.clone();
        arrived.hostiles.clear();
        let warrior = arrived
            .units
            .iter_mut()
            .find(|unit| unit.id == 101)
            .unwrap();
        warrior.x = 5;
        warrior.y = 4;
        assert!(matches!(
            verify_unit_order(
                &order,
                30,
                &before,
                &arrived,
                &tiles,
                &[],
                LaterFrames::default()
            ),
            Verdict::Verified
        ));

        let mut gone = before.clone();
        gone.hostiles.clear();
        assert!(matches!(
            verify_unit_order(
                &order,
                30,
                &before,
                &gone,
                &tiles,
                &[],
                LaterFrames::default()
            ),
            Verdict::Verified
        ));

        let stood = before.clone();
        assert!(matches!(
            verify_unit_order(&order, 30, &before, &stood, &tiles, &[], LaterFrames::default()),
            Verdict::Failed(reason) if reason == "not_captured"
        ));
    }

    #[test]
    fn great_people_activate_or_move_to_the_nearest_firaxis_valid_plot() {
        let state = StateSnapshot {
            units: vec![
                StateUnit {
                    id: 70,
                    kind: "UNIT_GREAT_SCIENTIST".to_string(),
                    great_person: Some(StateGreatPerson {
                        charges: 0,
                        can_activate: true,
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 71,
                    kind: "UNIT_GREAT_MERCHANT".to_string(),
                    great_person: Some(StateGreatPerson {
                        charges: 1,
                        activation_plots: vec![
                            StateActivationPlot {
                                x: 30,
                                y: 20,
                                distance: 7,
                                ..StateActivationPlot::default()
                            },
                            StateActivationPlot {
                                x: 11,
                                y: 12,
                                distance: 2,
                                ..StateActivationPlot::default()
                            },
                        ],
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };

        let (orders, stall) = great_person_orders(&state);

        assert_eq!(stall.total(), 0);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].subject, Some(70));
        assert_eq!(orders[0].verb.as_deref(), Some("ACTIVATE_GREAT_PERSON"));
        assert_eq!(orders[1].subject, Some(71));
        assert_eq!(orders[1].verb.as_deref(), Some("MOVE_TO"));
        assert_eq!(orders[1].pos, Some((11, 12)));
    }

    /// The nine Great People of live run `civvis-20260822T020434Z`, replayed.
    ///
    /// Three Artists, three Writers, three Musicians and a Scientist stood in
    /// Rome at turn 231 and `orders.sqlite` holds NOT ONE ORDER for any of
    /// them, ever. Their exports are reproduced here field for field: every
    /// activation plot at `slot_open: false`, and `empty_slots` at 24, 4 and
    /// 2 — never the zero that every escape hatch was gated on.
    #[test]
    fn the_rome_stack_is_starved_even_though_the_empire_owns_empty_slots() {
        // As exported. The counts are what made this invisible: an empire-wide
        // tally of slots this person cannot reach.
        let cultural = |empty: u32, plots: Vec<(i32, i32, i32)>| StateGreatPerson {
            can_activate: false,
            charges: 0,
            empty_slots: Some(empty),
            activation_plots: plots
                .into_iter()
                .map(|(x, y, distance)| StateActivationPlot {
                    x,
                    y,
                    distance,
                    slot_open: Some(false),
                })
                .collect(),
            ..StateGreatPerson::default()
        };
        let writer = cultural(24, vec![(67, 14, 12), (65, 25, 1), (64, 27, 2)]);
        let musician = cultural(4, vec![(65, 26, 1), (67, 32, 6)]);
        let artist = cultural(2, vec![(65, 26, 1)]);
        for (label, person) in [
            ("writer", &writer),
            ("musician", &musician),
            ("artist", &artist),
        ] {
            assert!(
                person.slot_starved(),
                "{label} has 24/4/2 slots the empire owns and none it can reach"
            );
        }

        // The Great Scientist of the same stack: no plots at all, waiting on
        // the Spaceport its `required_district` names. That is a missing
        // DISTRICT, not a missing slot, and it must not read as starved here —
        // the needs machinery has its own branch for it, and a work sale
        // cannot help a Scientist.
        let scientist = StateGreatPerson {
            can_activate: false,
            charges: 1,
            class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
            required_district: Some("DISTRICT_SPACEPORT".to_string()),
            ..StateGreatPerson::default()
        };
        assert!(!scientist.slot_starved(), "no plot offered is not no slot");

        // The three cases the predicate must NOT change, or it trades one
        // wedge for another:
        let mut can_go = writer.clone();
        can_go.can_activate = true;
        assert!(!can_go.slot_starved(), "a person the host will activate");

        let mut one_open = writer.clone();
        one_open.activation_plots[1].slot_open = Some(true);
        assert!(
            !one_open.slot_starved(),
            "one reachable slot is not starved"
        );

        // ⚠ An older control mod sends no `slot_open` at all. `None` is an
        // absence, never a claim — it kept the benefit of the doubt before
        // this predicate existed and it still does.
        let mut older_mod = writer.clone();
        for plot in &mut older_mod.activation_plots {
            plot.slot_open = None;
        }
        older_mod.empty_slots = None;
        assert!(
            !older_mod.slot_starved(),
            "an unknowing export must not be read as starved"
        );

        // And the whole point: the driver now counts them, and the work-sale
        // arm now sees them, where before both walked past.
        let state = StateSnapshot {
            units: vec![
                StateUnit {
                    id: 10092559,
                    kind: "UNIT_GREAT_WRITER".to_string(),
                    great_person: Some(writer),
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 9175056,
                    kind: "UNIT_GREAT_MUSICIAN".to_string(),
                    great_person: Some(musician),
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 6684677,
                    kind: "UNIT_GREAT_ARTIST".to_string(),
                    great_person: Some(artist),
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let (orders, stall) = great_person_orders(&state);
        assert!(
            orders.is_empty(),
            "marching to a known-full district is still never the answer"
        );
        assert_eq!(
            stall.no_empty_slot, 3,
            "all three report the real reason, not `no_activation_plot`"
        );
        assert_eq!(stall.no_activation_plot, 0);
    }

    /// Seven cultural people stood on one Theater plot for thirty-plus turns
    /// on run civvis-20260817T010950Z: `GetActivationHighlightPlots` lists the
    /// district whether or not a compatible Great Work slot is free, so the
    /// cooldown branch swallowed them forever and no one built the slots.
    /// With the host's own empty-slot count, a slot-starved person stands
    /// still under an explicit counter — and never marches to a full building.
    #[test]
    fn a_person_with_no_empty_slot_anywhere_stalls_explicitly_not_as_cooldown() {
        let on_plot = StateActivationPlot {
            x: 25,
            y: 23,
            distance: 0,
            ..StateActivationPlot::default()
        };
        let far_plot = StateActivationPlot {
            x: 30,
            y: 20,
            distance: 7,
            ..StateActivationPlot::default()
        };
        let state = StateSnapshot {
            units: vec![
                // Standing on the highlighted Theater, zero writing slots free.
                StateUnit {
                    id: 80,
                    kind: "UNIT_GREAT_WRITER".to_string(),
                    great_person: Some(StateGreatPerson {
                        empty_slots: Some(0),
                        activation_plots: vec![on_plot.clone(), far_plot.clone()],
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
                // Slot-starved and NOT on a plot: marching cannot help either.
                StateUnit {
                    id: 81,
                    kind: "UNIT_GREAT_ARTIST".to_string(),
                    great_person: Some(StateGreatPerson {
                        empty_slots: Some(0),
                        activation_plots: vec![far_plot],
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
                // Slots exist: standing on the plot is the benign cooldown
                // frame, exactly as before this field existed.
                StateUnit {
                    id: 82,
                    kind: "UNIT_GREAT_MUSICIAN".to_string(),
                    great_person: Some(StateGreatPerson {
                        empty_slots: Some(2),
                        activation_plots: vec![on_plot],
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };

        let (orders, stall) = great_person_orders(&state);

        assert!(
            orders.is_empty(),
            "no order helps a slot-starved person, and a cooldown is a wait \
             ({} issued)",
            orders.len()
        );
        assert_eq!(stall.no_empty_slot, 2);
        assert_eq!(stall.on_cooldown, 1);
        assert_eq!(stall.no_activation_plot, 0);
        assert_eq!(stall.total(), 3);
    }

    /// The host answering `can_activate` outranks its slot arithmetic: if the
    /// engine will take Activate here and now, press it.
    #[test]
    fn can_activate_outranks_a_zero_slot_count() {
        let state = StateSnapshot {
            units: vec![StateUnit {
                id: 83,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(StateGreatPerson {
                    can_activate: true,
                    empty_slots: Some(0),
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let (orders, stall) = great_person_orders(&state);
        assert_eq!(stall.total(), 0);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].verb.as_deref(), Some("ACTIVATE_GREAT_PERSON"));
    }

    #[test]
    fn great_person_already_on_an_unusable_activation_plot_waits() {
        let state = StateSnapshot {
            units: vec![StateUnit {
                id: 72,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(StateGreatPerson {
                    charges: 0,
                    can_activate: false,
                    activation_plots: vec![
                        StateActivationPlot {
                            x: 11,
                            y: 12,
                            distance: 0,
                            ..StateActivationPlot::default()
                        },
                        StateActivationPlot {
                            x: 20,
                            y: 18,
                            distance: 7,
                            ..StateActivationPlot::default()
                        },
                    ],
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };

        let (orders, stall) = great_person_orders(&state);

        assert!(orders.is_empty());
        assert_eq!(
            stall.on_cooldown, 1,
            "standing on a plot is the benign wait"
        );
        assert_eq!(
            stall.no_activation_plot, 0,
            "and must not be reported as an unusable Great Person"
        );
    }

    /// The (25,23) wedge from run civvis-20260817T010950Z: eleven cultural
    /// people stood a whole game on one highlighted tile the host KNEW held no
    /// compatible empty slot, while highlighted tiles with empty Amphitheater
    /// slots waited 2-10 tiles away, and the run ended with zero Great Works.
    /// A known-full tile is neither a cooldown to wait out nor a destination
    /// to march to; a known-open tile beats an unknown one even when farther.
    #[test]
    fn a_person_on_a_known_full_tile_marches_to_the_known_open_slot() {
        let full_here = StateActivationPlot {
            x: 25,
            y: 23,
            distance: 0,
            slot_open: Some(false),
        };
        let unknown_near = StateActivationPlot {
            x: 26,
            y: 23,
            distance: 1,
            slot_open: None,
        };
        let open_far = StateActivationPlot {
            x: 30,
            y: 20,
            distance: 7,
            slot_open: Some(true),
        };
        let state = StateSnapshot {
            units: vec![StateUnit {
                id: 84,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(StateGreatPerson {
                    charges: 0,
                    can_activate: false,
                    empty_slots: Some(2),
                    activation_plots: vec![full_here, unknown_near, open_far],
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };

        let (orders, stall) = great_person_orders(&state);

        assert_eq!(stall.on_cooldown, 0, "a known-full tile is not a cooldown");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
        assert_eq!(
            orders[0].pos,
            Some((30, 20)),
            "the known-open slot outranks the nearer unknown tile"
        );
    }

    /// The case the merged counter hid. A Great Person the host offers nowhere
    /// to activate is not waiting a frame — they are stranded until the empire
    /// builds the district their class needs, which for a Great Scientist is a
    /// Campus. Live brain logs reached 3 and 6 under the old key with no way to
    /// tell this apart from a Writer pausing for one export.
    #[test]
    fn a_great_person_with_nowhere_to_activate_is_named_separately() {
        let state = StateSnapshot {
            units: vec![StateUnit {
                id: 73,
                kind: "UNIT_GREAT_SCIENTIST".to_string(),
                great_person: Some(StateGreatPerson {
                    charges: 1,
                    can_activate: false,
                    activation_plots: Vec::new(),
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };

        let (orders, stall) = great_person_orders(&state);

        assert!(orders.is_empty(), "there is nowhere to send them");
        assert_eq!(
            stall.no_activation_plot, 1,
            "a Great Scientist with no Campus to use is a loss, not a wait"
        );
        assert_eq!(stall.on_cooldown, 0);
        assert_eq!(stall.total(), 1, "the old key keeps its meaning");
    }

    #[test]
    fn religious_unit_actions_reach_firaxis_unit_orders() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 93,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![grass(5, 5)],
        }]);
        let state = StateSnapshot {
            turn: 93,
            units: vec![StateUnit {
                id: 91,
                kind: "UNIT_APOSTLE".to_string(),
                x: 5,
                y: 5,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let unit = *mirror.uid_of.get(&91).expect("the Apostle is mirrored");

        let promotion = translate(
            &Action::Promote {
                unit,
                promotion: civvis::name!("translator"),
            },
            &mirror,
            &state,
        )
        .expect("the promotion crosses");
        assert_eq!(promotion.kind, "unit");
        assert_eq!(promotion.subject, Some(91));
        assert_eq!(
            promotion.verb.as_deref(),
            Some("PROMOTE:PROMOTION_TRANSLATOR")
        );

        let spread =
            translate(&Action::Spread { unit }, &mirror, &state).expect("the spread crosses");
        assert_eq!(spread.kind, "unit");
        assert_eq!(spread.subject, Some(91));
        assert_eq!(spread.verb.as_deref(), Some("SPREAD_RELIGION"));

        let evangelize = translate(
            &Action::EvangelizeBelief {
                unit,
                belief: civvis::name!("holy_order"),
            },
            &mirror,
            &state,
        )
        .expect("the Apostle belief choice crosses");
        assert_eq!(evangelize.kind, "unit");
        assert_eq!(evangelize.subject, Some(91));
        assert_eq!(
            evangelize.verb.as_deref(),
            Some("EVANGELIZE_BELIEF:BELIEF_HOLY_ORDER")
        );
        assert_eq!(evangelize.pos, None);

        let theological = translate(
            &Action::TheologicalAttack {
                unit,
                target: civvis::hex::offset_to_axial(6, 5),
            },
            &mirror,
            &state,
        )
        .expect("theological combat crosses");
        assert_eq!(theological.kind, "unit");
        assert_eq!(theological.subject, Some(91));
        assert_eq!(theological.verb.as_deref(), Some("ATTACK"));
        assert_eq!(theological.pos, Some((6, 5)));

        for (action, verb) in [
            (Action::HealReligious { unit }, "RELIGIOUS_HEAL"),
            (Action::RemoveHeresy { unit }, "REMOVE_HERESY"),
            (Action::LaunchInquisition { unit }, "LAUNCH_INQUISITION"),
            (Action::ConvertBarbarians { unit }, "CONVERT_BARBARIANS"),
        ] {
            let operation =
                translate(&action, &mirror, &state).expect("a direct religious operation crosses");
            assert_eq!(operation.kind, "unit");
            assert_eq!(operation.subject, Some(91));
            assert_eq!(operation.verb.as_deref(), Some(verb));
            assert_eq!(operation.pos, None);
        }

        // ★★★ And the one order that touches an ENEMY religious unit. It used
        // to fall through this match's `_ => None` and be counted
        // untranslatable, so a rival Apostle standing in our territory —
        // untouchable by ordinary combat, by design — could not be answered by
        // anything the bridge could send. `target_unit` deliberately does not
        // cross: the command is parameterless in Firaxis' own UnitPanel (its
        // `UnitCommands` row carries no `InterfaceMode`) and the engine
        // requires the mover to be standing on the target anyway, so the
        // co-location IS the target.
        let condemn = translate(
            &Action::CondemnHeretic {
                unit,
                target_unit: 4242,
            },
            &mirror,
            &state,
        )
        .expect("the condemnation crosses");
        assert_eq!(condemn.kind, "unit");
        assert_eq!(condemn.subject, Some(91));
        assert_eq!(condemn.verb.as_deref(), Some("CONDEMN_HERETIC"));
        assert_eq!(
            condemn.pos, None,
            "a parameterless command must not invent a destination"
        );
    }

    /// ★★★ Corps and Army: decided 10,015 times across the live archive and
    /// sent zero times, because `CombineUnits` had no arm and fell through
    /// `translate`'s `_ => None`. It is two Firaxis commands rather than one —
    /// `UNITCOMMAND_FORM_CORPS` and `UNITCOMMAND_FORM_ARMY`,
    /// `Gameplay/Data/UnitCommands.xml:20-21` and `:44-45` — and the tier is
    /// the caller's to choose, so the verb has to carry which merge was decided
    /// rather than leaving the mod to guess it from a board it cannot see.
    #[test]
    fn combining_units_reaches_the_form_corps_and_form_army_commands() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 118,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![grass(4, 4), grass(5, 4)],
        }]);
        let state = StateSnapshot {
            turn: 118,
            units: vec![
                StateUnit {
                    id: 41,
                    kind: "UNIT_SWORDSMAN".to_string(),
                    x: 4,
                    y: 4,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 42,
                    kind: "UNIT_SWORDSMAN".to_string(),
                    x: 5,
                    y: 4,
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let first = *mirror.uid_of.get(&41).expect("the first swordsman mirrors");
        let second = *mirror
            .uid_of
            .get(&42)
            .expect("the second swordsman mirrors");

        let corps = translate(
            &Action::CombineUnits {
                unit: first,
                with: second,
            },
            &mirror,
            &state,
        )
        .expect("two standard units form a Corps on the live seat");
        assert_eq!(corps.kind, "unit");
        assert_eq!(corps.subject, Some(41));
        assert_eq!(corps.verb.as_deref(), Some("FORM_CORPS"));
        assert_eq!(
            corps.pos,
            Some((state.seat.local_player, 42)),
            "x/y are the partner's OWNER and UNIT ID, the pair Firaxis's own \
             WorldInput.lua:2879-2880 puts in PARAM_UNIT_PLAYER/PARAM_UNIT_ID — \
             never a map position"
        );

        // A standard unit joining a Corps is an Army, which is a DIFFERENT
        // command with a different prerequisite civic (Mobilization, not
        // Nationalism). Sending FORM_CORPS here would be refused by the host
        // for a unit that is already one.
        mirror
            .game
            .units
            .get_mut(&first)
            .expect("the mirrored unit is addressable")
            .formation = 1;
        let army = translate(
            &Action::CombineUnits {
                unit: first,
                with: second,
            },
            &mirror,
            &state,
        )
        .expect("a Corps absorbing a standard unit forms an Army");
        assert_eq!(army.verb.as_deref(), Some("FORM_ARMY"));
        assert_eq!(army.subject, Some(41));

        // The partner is half the order. Without a Civilization VI id for it
        // there is nothing to put in PARAM_UNIT_ID, so this must be counted as
        // untranslatable rather than sent as a merge with no one to merge with.
        assert!(
            translate(
                &Action::CombineUnits {
                    unit: first,
                    with: 9_999,
                },
                &mirror,
                &state,
            )
            .is_none(),
            "an unmapped partner cannot be silently dropped from the command"
        );
    }

    /// ★★★★★ THE HOST'S OWN CORPS REACHES `FORM_ARMY`, END TO END.
    ///
    /// The test above proves the tier decides the command; it sets the tier by
    /// hand. That is exactly the half that was missing in #2373: the live seat
    /// runs `--fresh-board`, the mirror is rebuilt from the host export every
    /// turn, and the export said nothing about Corps or Army — so every unit was
    /// reconstructed at tier 0 and this arm could only ever emit FORM_CORPS. The
    /// Army half of the unit-consolidation layer was unreachable live for that
    /// one reason.
    ///
    /// This drives the whole chain instead: a `StateUnit` carrying the tier the
    /// mod now exports from `Unit:GetMilitaryFormation()`, through
    /// `LiveMirror::new`, into the command actually sent. Nothing is set by hand.
    #[test]
    fn a_corps_the_host_exported_reaches_form_army_from_a_fresh_board() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 152,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![grass(4, 4), grass(5, 4)],
        }]);
        // Exactly what the host export now says: 1 = Corps, and the escort stack
        // size stays 1 because a Corps is ONE unit. The two fields share a word
        // and nothing else.
        let state = StateSnapshot {
            turn: 152,
            units: vec![
                StateUnit {
                    id: 71,
                    kind: "UNIT_SWORDSMAN".to_string(),
                    x: 4,
                    y: 4,
                    formation: Some(1),
                    formation_count: 1,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 72,
                    kind: "UNIT_SWORDSMAN".to_string(),
                    x: 5,
                    y: 4,
                    formation: Some(0),
                    formation_count: 1,
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let corps = *mirror.uid_of.get(&71).expect("the Corps mirrors");
        let standard = *mirror.uid_of.get(&72).expect("the swordsman mirrors");
        assert_eq!(
            mirror.game.units[&corps].formation, 1,
            "the host said Corps, so the mirror must hold a Corps"
        );
        assert_eq!(mirror.game.units[&standard].formation, 0);

        let order = translate(
            &Action::CombineUnits {
                unit: corps,
                with: standard,
            },
            &mirror,
            &state,
        )
        .expect("a host Corps absorbing a standard unit is an Army");
        assert_eq!(
            order.verb.as_deref(),
            Some("FORM_ARMY"),
            "a Corps the host itself reported must not be sent FORM_CORPS again — \
             they are different commands behind different civics (Mobilization, \
             not Nationalism), so the host refuses the wrong one"
        );
        assert_eq!(order.subject, Some(71));
        assert_eq!(order.pos, Some((state.seat.local_player, 72)));

        // ⚠ And the same pair with the tier UNREAD stays a Corps request. The
        // mod's −1 means "asked, could not answer"; the mirror keeps the board's
        // own value rather than claiming standard, and a board that knows nothing
        // still asks for the merge it can justify.
        let unknown = StateSnapshot {
            turn: 152,
            units: state
                .units
                .iter()
                .map(|unit| StateUnit {
                    formation: Some(-1),
                    ..unit.clone()
                })
                .collect(),
            ..StateSnapshot::default()
        };
        let blind = civvis::mirror::LiveMirror::new(&snapshot, &unknown, 4, 1, 250, 0);
        let first = *blind.uid_of.get(&71).expect("the first swordsman mirrors");
        let second = *blind.uid_of.get(&72).expect("the second swordsman mirrors");
        assert_eq!(
            translate(
                &Action::CombineUnits {
                    unit: first,
                    with: second,
                },
                &blind,
                &unknown,
            )
            .expect("an unread tier still translates")
            .verb
            .as_deref(),
            Some("FORM_CORPS")
        );
    }

    /// ★★★ Espionage had no route to a live game: all three actions fell
    /// through `translate`'s `_ => None`, on top of a `Game::spies` the mirror
    /// left empty so nothing was ever chosen to drop.
    #[test]
    fn spy_missions_reach_firaxis_unit_operations() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![grass(4, 4), grass(6, 5)],
        }]);
        let mut state = StateSnapshot {
            turn: 120,
            spy_capacity: Some(2),
            ..StateSnapshot::default()
        };
        state.units.push(StateUnit {
            id: 77,
            kind: "UNIT_SPY".to_string(),
            x: 4,
            y: 4,
            ..StateUnit::default()
        });
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let spy = *mirror.uid_of.get(&77).expect("the Spy is mirrored");

        // The mission's own name becomes the host's operation, aimed at the
        // target plot the way Firaxis' EspionagePopup aims it.
        let heist = translate(
            &Action::SpyMission {
                spy,
                mission: "great_work_heist".to_string(),
                target: (6, 5),
            },
            &mirror,
            &state,
        )
        .expect("the mission crosses");
        assert_eq!(heist.kind, "unit");
        assert_eq!(heist.subject, Some(77));
        assert_eq!(heist.verb.as_deref(), Some("SPY_GREAT_WORK_HEIST"));
        assert!(
            heist.pos.is_some(),
            "a spy operation without a destination silently does nothing"
        );

        let promote = translate(
            &Action::PromoteSpy {
                spy,
                promotion: civvis::name!("technologist"),
            },
            &mirror,
            &state,
        )
        .expect("the promotion crosses");
        assert!(promote
            .verb
            .as_deref()
            .is_some_and(|verb| verb.starts_with("PROMOTE:")));
    }

    #[test]
    fn builder_repairs_reach_firaxis_unit_orders() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 94,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![grass(5, 5)],
        }]);
        let state = StateSnapshot {
            turn: 94,
            units: vec![StateUnit {
                id: 92,
                kind: "UNIT_BUILDER".to_string(),
                x: 5,
                y: 5,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let unit = *mirror.uid_of.get(&92).expect("the Builder is mirrored");

        let repair = translate(&Action::RepairImprovement { unit }, &mirror, &state)
            .expect("the builder repair crosses");
        assert_eq!(repair.kind, "unit");
        assert_eq!(repair.subject, Some(92));
        assert_eq!(repair.verb.as_deref(), Some("REPAIR"));
        assert_eq!(repair.pos, None);
    }

    /// `Action::Pillage` was decided by the doctrine layer and dropped by the
    /// bridge; it now crosses as the parameterless host operation.
    #[test]
    fn a_pillage_crosses_as_the_parameterless_host_operation() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![grass(5, 5)],
        }]);
        let state = StateSnapshot {
            turn: 60,
            units: vec![StateUnit {
                id: 77,
                kind: "UNIT_HORSEMAN".to_string(),
                x: 5,
                y: 5,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let unit = *mirror.uid_of.get(&77).expect("the Horseman is mirrored");

        let pillage =
            translate(&Action::Pillage { unit }, &mirror, &state).expect("the pillage crosses");
        assert_eq!(pillage.kind, "unit");
        assert_eq!(pillage.subject, Some(77));
        assert_eq!(pillage.verb.as_deref(), Some("PILLAGE"));
        assert_eq!(pillage.pos, None);
    }

    #[test]
    fn contracted_promotion_names_expand_to_the_firaxis_database_ids() {
        assert_eq!(
            civ6_unit_promotion_name("cobra_strike"),
            "PROMOTION_MONK_COBRA_STRIKE"
        );
        assert_eq!(
            civ6_unit_promotion_name("supercarrier"),
            "PROMOTION_SUPER_CARRIER"
        );
        assert_eq!(civ6_unit_promotion_name("surf_band"), "PROMOTION_SURF_ROCK");
    }

    /// The zero-charge Prophet left on the map after its religion was founded
    /// is retired from the bridge, and its hex stops swallowing other units'
    /// orders; a Prophet whose seat has NOT founded yet keeps every protection.
    #[test]
    fn a_founded_zero_charge_prophet_is_retired_and_stops_reserving_its_hex() {
        let ghost = StateUnit {
            id: 851_977,
            kind: "UNIT_GREAT_PROPHET".to_string(),
            x: 59,
            y: 16,
            great_person: Some(StateGreatPerson {
                charges: 0,
                can_activate: false,
                activation_plots: Vec::new(),
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        };
        let founded = StateSnapshot {
            founded_religion: Some("RELIGION_BUDDHISM".to_string()),
            units: vec![ghost.clone()],
            ..StateSnapshot::default()
        };

        let (orders, stall) = great_person_orders(&founded);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].subject, Some(851_977));
        assert_eq!(orders[0].verb.as_deref(), Some("DELETE"));
        assert_eq!(stall.retired_prophets, 1);
        assert_eq!(stall.total(), 0, "a retirement is not a stall");

        // Its hex no longer black-holes the settler's exit from the capital.
        let settler_move = || Order {
            kind: "unit",
            subject: Some(2_031_627),
            verb: Some("MOVE_TO".to_string()),
            pos: Some((59, 16)),
        };
        let (kept, deferred) = defer_great_person_plot_conflicts(vec![settler_move()], &founded);
        assert_eq!(deferred, 0);
        assert_eq!(kept.len(), 1);

        // Before the religion exists the same unit is a Prophet still to found:
        // no retirement, still a named stall, hex still reserved.
        let unfounded = StateSnapshot {
            founded_religion: None,
            units: vec![ghost],
            ..StateSnapshot::default()
        };
        let (orders, stall) = great_person_orders(&unfounded);
        assert!(orders.is_empty());
        assert_eq!(stall.retired_prophets, 0);
        assert_eq!(stall.no_activation_plot, 1);
        let (kept, deferred) = defer_great_person_plot_conflicts(vec![settler_move()], &unfounded);
        assert_eq!(deferred, 1);
        assert!(kept.is_empty());
    }

    #[test]
    fn great_person_reservation_defers_physical_moves_but_not_trade_routes() {
        let state = StateSnapshot {
            units: vec![
                StateUnit {
                    id: 72,
                    kind: "UNIT_GREAT_ADMIRAL".to_string(),
                    x: 68,
                    y: 27,
                    great_person: Some(StateGreatPerson {
                        charges: 1,
                        can_activate: true,
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 73,
                    kind: "UNIT_GREAT_PROPHET".to_string(),
                    x: 68,
                    y: 17,
                    great_person: Some(StateGreatPerson {
                        can_activate: false,
                        ..StateGreatPerson::default()
                    }),
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let (orders, deferred) = defer_great_person_plot_conflicts(
            vec![
                Order {
                    kind: "unit",
                    subject: Some(10),
                    verb: Some("MOVE_TO".to_string()),
                    pos: Some((68, 27)),
                },
                Order {
                    kind: "unit",
                    subject: Some(72),
                    verb: Some("ACTIVATE_GREAT_PERSON".to_string()),
                    pos: None,
                },
                Order {
                    kind: "unit",
                    subject: Some(11),
                    verb: Some("MOVE_TO".to_string()),
                    pos: Some((68, 17)),
                },
                Order {
                    kind: "unit",
                    subject: Some(12),
                    verb: Some("MOVE_TO".to_string()),
                    pos: Some((69, 27)),
                },
                Order {
                    kind: "unit",
                    subject: Some(14),
                    verb: Some("TRADE_ROUTE".to_string()),
                    pos: Some((68, 27)),
                },
            ],
            &state,
        );

        assert_eq!(deferred, 2);
        assert_eq!(orders.len(), 3);
        assert_eq!(orders[0].subject, Some(72));
        assert_eq!(orders[1].subject, Some(12));
        assert_eq!(orders[2].verb.as_deref(), Some("TRADE_ROUTE"));
    }

    #[test]
    fn firaxis_religion_state_reaches_a_found_religion_order() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 89,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 89,
            pantheon: Some("BELIEF_DIVINE_SPARK".to_string()),
            prophet_pending: true,
            founded_religions: vec![
                "RELIGION_ORTHODOXY".to_string(),
                "RELIGION_CATHOLICISM".to_string(),
                "RELIGION_ISLAM".to_string(),
            ],
            taken_religion_beliefs: vec![
                "BELIEF_WORK_ETHIC".to_string(),
                "BELIEF_TITHE".to_string(),
            ],
            cities: vec![StateCity {
                id: 7,
                name: "Krakow".to_string(),
                x: 6,
                y: 6,
                pop: 3,
                capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_HOLY_SITE".to_string(),
                    x: 7,
                    y: 6,
                    pillaged: false,
                    complete: true,
                    ..StateDistrict::default()
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);

        assert!(mirror.game.players[0].prophet_pending);
        assert_eq!(mirror.game.religions_founded(), 3);
        assert_eq!(mirror.game.max_religions(), 4);
        assert!(mirror.game.players[1]
            .religion_beliefs
            .contains(&"work_ethic".to_string()));
        assert!(mirror.game.players[1]
            .religion_beliefs
            .contains(&"tithe".to_string()));
        let action = mirror
            .game
            .legal_actions(0)
            .into_iter()
            .find(|action| matches!(action, Action::FoundReligion { .. }))
            .expect("a pending Firaxis Prophet must expose a religion choice");
        let order = translate(&action, &mirror, &state)
            .expect("a CIVVIS religion choice must reach Firaxis");

        assert_eq!(order.kind, "religion");
        assert_eq!(order.subject, None);
        assert!(order
            .verb
            .as_deref()
            .is_some_and(|verb| verb.starts_with("BELIEF_") && verb.contains(",BELIEF_")));
    }

    #[test]
    fn purchases_preserve_currency_formation_and_district_plot() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 92,
            cities: vec![StateCity {
                id: 77,
                name: "Wroclaw".to_string(),
                x: 6,
                y: 6,
                pop: 4,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
        let city = mirror.game.player_city_ids(0)[0];
        let unit = translate(
            &Action::Buy {
                city,
                unit: civvis::name!("missionary"),
                formation: 2,
                currency: "faith".to_string(),
            },
            &mirror,
            &state,
        )
        .expect("faith unit purchase crosses");
        assert_eq!(unit.kind, "purchase_faith");
        assert_eq!(unit.subject, Some(77));
        assert_eq!(unit.verb.as_deref(), Some("UNIT_MISSIONARY"));
        assert_eq!(unit.pos, Some((2, -1)));

        let walls = translate(
            &Action::BuyBuilding {
                city,
                building: civvis::name!("medieval_walls"),
                currency: "gold".to_string(),
            },
            &mirror,
            &state,
        )
        .expect("a wall purchase crosses through the same building vocabulary");
        assert_eq!(walls.kind, "purchase");
        assert_eq!(walls.subject, Some(77));
        assert_eq!(walls.verb.as_deref(), Some("BUILDING_CASTLE"));

        let pos = (7, 6);
        let district = translate(
            &Action::BuyDistrict {
                city,
                district: civvis::name!("campus"),
                pos,
                currency: "gold".to_string(),
            },
            &mirror,
            &state,
        )
        .expect("placed district purchase crosses");
        assert_eq!(district.kind, "purchase");
        assert_eq!(district.verb.as_deref(), Some("DISTRICT_CAMPUS"));
        assert_eq!(
            district.pos,
            Some(civvis::hex::axial_to_offset(pos.0, pos.1))
        );
    }

    #[test]
    fn pillaged_district_repair_reuses_the_firaxis_district_type() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 112,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 112,
            cities: vec![StateCity {
                id: 77,
                name: "Ostia".to_string(),
                x: 6,
                y: 6,
                pop: 4,
                capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_CAMPUS".to_string(),
                    x: 7,
                    y: 6,
                    pillaged: true,
                    complete: true,
                    ..StateDistrict::default()
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
        let city = mirror.game.player_city_ids(0)[0];
        let pos = civvis::hex::offset_to_axial(7, 6);
        assert!(mirror.game.map.tiles[&pos].pillaged);

        let order = translate(
            &Action::Produce {
                city,
                item: civvis::game::Item::Repair {
                    repair: civvis::name!("district"),
                    pos,
                },
            },
            &mirror,
            &state,
        )
        .expect("a CIVVIS district repair must reach Firaxis");

        assert_eq!(order.kind, "produce");
        assert_eq!(order.subject, Some(77));
        assert_eq!(order.verb.as_deref(), Some("DISTRICT_CAMPUS"));
        assert_eq!(order.pos, None);
    }

    #[test]
    fn link_and_unlink_orders_preserve_both_firaxis_unit_ids() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 92,
            seat: civvis::mirror::Seat {
                local_player: 7,
                ..civvis::mirror::Seat::default()
            },
            units: vec![
                StateUnit {
                    id: 501,
                    kind: "UNIT_SETTLER".to_string(),
                    x: 6,
                    y: 6,
                    hp: 100.0,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 502,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 6,
                    y: 6,
                    hp: 100.0,
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
        let settler = mirror.uid_of[&501];
        let escort = mirror.uid_of[&502];

        let link = translate(
            &Action::LinkUnits {
                unit: settler,
                with: escort,
            },
            &mirror,
            &state,
        )
        .expect("formation order crosses");
        assert_eq!(link.kind, "unit");
        assert_eq!(link.subject, Some(501));
        assert_eq!(link.verb.as_deref(), Some("ENTER_FORMATION"));
        assert_eq!(link.pos, Some((7, 502)));

        let unlink = translate(&Action::UnlinkUnits { unit: settler }, &mirror, &state)
            .expect("formation exit crosses");
        assert_eq!(unlink.subject, Some(501));
        assert_eq!(unlink.verb.as_deref(), Some("EXIT_FORMATION"));
        assert_eq!(unlink.pos, None);
    }

    fn unit_order(subject: i64, verb: &str, pos: Option<(i32, i32)>) -> Order {
        Order {
            kind: "unit",
            subject: Some(subject),
            verb: Some(verb.to_string()),
            pos,
        }
    }

    #[test]
    fn unit_followups_wait_for_the_next_observed_firaxis_frame() {
        let (orders, deferred, coalesced) = coalesce_unit_paths(
            vec![
                unit_order(42, "MOVE_TO", Some((4, 5))),
                unit_order(42, "IMPROVE:IMPROVEMENT_FARM", None),
                unit_order(99, "FORTIFY", None),
                Order {
                    kind: "research",
                    subject: None,
                    verb: Some("TECH_WRITING".to_string()),
                    pos: None,
                },
            ],
            false,
        );

        assert_eq!(deferred, 1);
        assert_eq!(coalesced, 0);
        assert_eq!(orders.len(), 3);
        assert_eq!(orders[0].subject, Some(42));
        assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
        assert_eq!(orders[0].pos, Some((4, 5)));
        assert_eq!(orders[1].subject, Some(99));
        assert_eq!(orders[2].kind, "research");
    }

    /// The found carries the hex the planned walk leaves the settler on —
    /// the last step of its contiguous walk — or, founding without moving,
    /// where the host says it stands. A found that already has a site and a
    /// unit the export does not know are left alone.
    #[test]
    fn a_found_city_row_carries_the_site_the_walk_ends_on() {
        let state = civvis::mirror::StateSnapshot {
            units: vec![
                civvis::mirror::StateUnit {
                    id: 7,
                    x: 10,
                    y: 10,
                    ..Default::default()
                },
                civvis::mirror::StateUnit {
                    id: 8,
                    x: 30,
                    y: 31,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut orders = vec![
            unit_order(7, "MOVE_TO", Some((11, 10))),
            unit_order(7, "MOVE_TO", Some((12, 11))),
            unit_order(7, "FOUND_CITY", None),
            unit_order(8, "FOUND_CITY", None),
            unit_order(9, "FOUND_CITY", None),
            unit_order(10, "FOUND_CITY", Some((1, 1))),
        ];
        assert_eq!(stamp_found_sites(&mut orders, &state), 2);
        assert_eq!(orders[2].pos, Some((12, 11)), "the walk's last hex");
        assert_eq!(orders[3].pos, Some((30, 31)), "founds where it stands");
        assert_eq!(orders[4].pos, None, "a unit the export does not know");
        assert_eq!(orders[5].pos, Some((1, 1)), "an existing site is kept");
        // Stamped before the fold, the site survives coalescing and the
        // walk-with-a-follow-up is never cut.
        let (orders, _, coalesced) = coalesce_unit_paths(orders, true);
        assert_eq!(coalesced, 1);
        assert_eq!(orders[0].pos, Some((12, 11)));
        assert_eq!(orders[1].verb.as_deref(), Some("FOUND_CITY"));
        assert_eq!(orders[1].pos, Some((12, 11)));
    }

    /// `--assume-seat` names the capabilities a replay should pretend the
    /// mod advertised; a typo or non-boolean value is an error.
    #[test]
    fn assumed_seat_capabilities_are_named_and_checked() {
        let mut seat = civvis::mirror::Seat::default();
        let names: Vec<String> = ["order_queue", "replan_frames", " tile_delta "]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assume_seat_capabilities(&mut seat, &names).unwrap();
        assert!(seat.order_queue);
        assert!(seat.replan_frames);
        assert!(seat.tile_delta);
        assert!(!seat.moves_at_turn_start, "not named: not assumed");
        assert!(assume_seat_capabilities(&mut seat, &["order_que".to_string()]).is_err());
        assert!(assume_seat_capabilities(&mut seat, &["replan_frames=2".to_string()]).is_err());
    }

    /// A settler with two movement points logs two hex steps; the host must be
    /// asked for the WHOLE walk, or it spends every turn on the first hex.
    #[test]
    fn a_units_planned_walk_becomes_one_move_to_its_furthest_hex() {
        let (orders, deferred, coalesced) = coalesce_unit_paths(
            vec![
                unit_order(7, "MOVE_TO", Some((10, 10))),
                unit_order(7, "MOVE_TO", Some((11, 10))),
                unit_order(7, "MOVE_TO", Some((12, 11))),
                unit_order(7, "FOUND_CITY", None),
            ],
            false,
        );

        assert_eq!(coalesced, 2, "two later steps folded into the first");
        assert_eq!(
            deferred, 1,
            "the founding still waits for the arrival frame"
        );
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].subject, Some(7));
        assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
        assert_eq!(orders[0].pos, Some((12, 11)));
    }

    /// Only the CONTIGUOUS prefix of moves is a walk. A move logged after an act
    /// was planned from where the act leaves the unit, so it waits like the act.
    #[test]
    fn a_move_after_an_act_is_not_folded_into_the_walk() {
        let (orders, deferred, coalesced) = coalesce_unit_paths(
            vec![
                unit_order(3, "MOVE_TO", Some((1, 1))),
                unit_order(3, "MOVE_TO", Some((2, 1))),
                unit_order(3, "RANGE_ATTACK", Some((4, 1))),
                unit_order(3, "MOVE_TO", Some((1, 1))),
            ],
            false,
        );

        assert_eq!(coalesced, 1);
        assert_eq!(deferred, 2);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].pos, Some((2, 1)));
    }

    /// Melee stays conservative: the strike is not turned into a MOVE_TO onto the
    /// defender, and a unit whose first order is the strike sends only the strike.
    #[test]
    fn a_melee_strike_terminates_the_walk_and_leads_alone_when_first() {
        let (orders, deferred, coalesced) = coalesce_unit_paths(
            vec![
                unit_order(5, "MOVE_TO", Some((8, 8))),
                unit_order(5, "ATTACK", Some((9, 8))),
                unit_order(6, "ATTACK", Some((9, 8))),
                unit_order(6, "MOVE_TO", Some((9, 8))),
            ],
            false,
        );

        assert_eq!(coalesced, 0);
        assert_eq!(deferred, 2);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].subject, Some(5));
        assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
        assert_eq!(orders[0].pos, Some((8, 8)));
        assert_eq!(orders[1].subject, Some(6));
        assert_eq!(orders[1].verb.as_deref(), Some("ATTACK"));
    }

    /// Units are folded independently even when their steps interleave, and each
    /// keeps the slot of its FIRST order so the batch's relative order — which the
    /// host executes sequentially — is preserved.
    #[test]
    fn interleaved_units_fold_independently_and_keep_their_batch_slots() {
        let (orders, deferred, coalesced) = coalesce_unit_paths(
            vec![
                unit_order(1, "MOVE_TO", Some((0, 1))),
                unit_order(2, "MOVE_TO", Some((5, 5))),
                unit_order(1, "MOVE_TO", Some((0, 2))),
                Order {
                    kind: "produce",
                    subject: Some(65_536),
                    verb: Some("UNIT_SETTLER".to_string()),
                    pos: None,
                },
                unit_order(2, "MOVE_TO", Some((6, 5))),
                unit_order(2, "MOVE_TO", Some((7, 5))),
                unit_order(1, "IMPROVE:IMPROVEMENT_MINE", None),
            ],
            false,
        );

        assert_eq!(coalesced, 3);
        assert_eq!(deferred, 1);
        assert_eq!(orders.len(), 3);
        assert_eq!((orders[0].subject, orders[0].pos), (Some(1), Some((0, 2))));
        assert_eq!((orders[1].subject, orders[1].pos), (Some(2), Some((7, 5))));
        assert_eq!(orders[2].kind, "produce");
    }

    /// (subject, verb, position) of one host order, for asserting a sequence.
    type Sequenced<'a> = (Option<i64>, &'a str, Option<(i32, i32)>);

    /// With a queueing mod (`seat.order_queue`), the walk still folds into one
    /// MOVE_TO and every later order for that unit rides along in sequence —
    /// the strike after the step, the fortify after the strike, a move after an
    /// act as its own order — instead of waiting a turn.
    #[test]
    fn a_sequenced_turn_keeps_every_followup_behind_the_folded_walk() {
        let (orders, deferred, coalesced) = coalesce_unit_paths(
            vec![
                unit_order(3, "MOVE_TO", Some((1, 1))),
                unit_order(3, "MOVE_TO", Some((2, 1))),
                unit_order(3, "RANGE_ATTACK", Some((4, 1))),
                unit_order(3, "FORTIFY", None),
                unit_order(5, "MOVE_TO", Some((8, 8))),
                unit_order(5, "ATTACK", Some((9, 8))),
                unit_order(5, "MOVE_TO", Some((8, 7))),
                unit_order(6, "ATTACK", Some((9, 8))),
            ],
            true,
        );

        assert_eq!(deferred, 0, "nothing waits for the next frame");
        assert_eq!(coalesced, 1, "only the contiguous walk folds");
        let sequence: Vec<Sequenced<'_>> = orders
            .iter()
            .map(|order| {
                (
                    order.subject,
                    order.verb.as_deref().unwrap_or(""),
                    order.pos,
                )
            })
            .collect();
        assert_eq!(
            sequence,
            vec![
                (Some(3), "MOVE_TO", Some((2, 1))),
                (Some(3), "RANGE_ATTACK", Some((4, 1))),
                (Some(3), "FORTIFY", None),
                (Some(5), "MOVE_TO", Some((8, 8))),
                (Some(5), "ATTACK", Some((9, 8))),
                (Some(5), "MOVE_TO", Some((8, 7))),
                (Some(6), "ATTACK", Some((9, 8))),
            ]
        );
        assert_eq!(sequenced_unit_followups(&orders), 4);
    }

    /// The same list against a mod without the queue is exactly the old
    /// behaviour: the capability decides, not the wish.
    #[test]
    fn without_the_capability_the_followups_still_wait() {
        let list = || {
            vec![
                unit_order(3, "MOVE_TO", Some((1, 1))),
                unit_order(3, "RANGE_ATTACK", Some((4, 1))),
            ]
        };
        let (orders, deferred, _) = coalesce_unit_paths(list(), false);
        assert_eq!((orders.len(), deferred), (1, 1));
        let (orders, deferred, _) = coalesce_unit_paths(list(), true);
        assert_eq!((orders.len(), deferred), (2, 0));
        assert_eq!(sequenced_unit_followups(&orders), 1);
    }

    #[test]
    fn governor_followups_wait_for_the_next_observed_firaxis_frame() {
        let (orders, deferred) = defer_governor_followups(vec![
            Order {
                kind: "governor_appoint",
                subject: Some(65_536),
                verb: Some("GOVERNOR_THE_DEFENDER".to_string()),
                pos: Some((0, -1)),
            },
            Order {
                kind: "governor_promote",
                subject: None,
                verb: Some(
                    "GOVERNOR_THE_DEFENDER,GOVERNOR_PROMOTION_GARRISON_COMMANDER".to_string(),
                ),
                pos: None,
            },
            Order {
                kind: "research",
                subject: None,
                verb: Some("TECH_WRITING".to_string()),
                pos: None,
            },
        ]);

        assert_eq!(deferred, 1);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].kind, "governor_appoint");
        assert_eq!(orders[1].kind, "research");
    }

    #[test]
    fn governor_actions_use_firaxis_types_and_city_owner_ids() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 40,
            seat: civvis::mirror::Seat {
                local_player: 7,
                ..civvis::mirror::Seat::default()
            },
            cities: vec![StateCity {
                id: 65_536,
                name: "Capital".to_string(),
                x: 6,
                y: 6,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let city = mirror.game.player_city_ids(0)[0];

        let appoint = translate(
            &Action::AppointGovernor {
                governor: civvis::name!("victor"),
                city,
            },
            &mirror,
            &state,
        )
        .expect("a known Governor and city translate");
        assert_eq!(appoint.kind, "governor_appoint");
        assert_eq!(appoint.subject, Some(65_536));
        assert_eq!(appoint.verb.as_deref(), Some("GOVERNOR_THE_DEFENDER"));
        assert_eq!(appoint.pos, Some((7, -1)));

        let promote = translate(
            &Action::PromoteGovernor {
                governor: civvis::name!("victor"),
                promotion: civvis::name!("garrison_commander"),
            },
            &mirror,
            &state,
        )
        .expect("a known promotion translates");
        assert_eq!(promote.kind, "governor_promote");
        assert_eq!(
            promote.verb.as_deref(),
            Some("GOVERNOR_THE_DEFENDER,GOVERNOR_PROMOTION_GARRISON_COMMANDER")
        );
    }

    #[test]
    fn great_person_claims_cross_the_bridge() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 40,
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

        let recruit = translate(
            &Action::RecruitGreatPerson {
                kind: "scientist".to_string(),
            },
            &mirror,
            &state,
        )
        .expect("a recruit decision translates");
        assert_eq!(recruit.kind, "gp_recruit");
        assert_eq!(recruit.subject, None);
        assert_eq!(
            recruit.verb.as_deref(),
            Some("GREAT_PERSON_CLASS_SCIENTIST")
        );

        let gold = translate(
            &Action::PatronizeGreatPerson {
                kind: "merchant".to_string(),
                currency: "gold".to_string(),
            },
            &mirror,
            &state,
        )
        .expect("a gold patronage translates");
        assert_eq!(gold.kind, "gp_patronize");
        assert_eq!(gold.verb.as_deref(), Some("GREAT_PERSON_CLASS_MERCHANT"));

        let faith = translate(
            &Action::PatronizeGreatPerson {
                kind: "general".to_string(),
                currency: "faith".to_string(),
            },
            &mirror,
            &state,
        )
        .expect("a faith patronage translates");
        assert_eq!(faith.kind, "gp_patronize_faith");
        assert_eq!(faith.verb.as_deref(), Some("GREAT_PERSON_CLASS_GENERAL"));
    }

    #[test]
    fn dedication_choices_cross_with_firaxis_commemoration_names() {
        use civvis::name::Name;

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 40,
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

        for (civvis, firaxis) in [
            ("monumentality", "COMMEMORATION_INFRASTRUCTURE"),
            ("pen_brush_and_voice", "COMMEMORATION_CULTURAL"),
            ("free_inquiry", "COMMEMORATION_SCIENTIFIC"),
            ("exodus_of_the_evangelists", "COMMEMORATION_RELIGIOUS"),
            ("hic_sunt_dracones", "COMMEMORATION_EXPLORATION"),
            ("reform_the_coinage", "COMMEMORATION_ECONOMIC"),
            ("to_arms", "COMMEMORATION_MILITARY"),
            ("heartbeat_of_steam", "COMMEMORATION_INDUSTRIAL"),
            ("bodyguard_of_lies", "COMMEMORATION_ESPIONAGE"),
            ("sky_and_stars", "COMMEMORATION_AERONAUTICAL"),
            ("wish_you_were_here", "COMMEMORATION_TOURISM"),
            ("automaton_warfare", "COMMEMORATION_AUTOMATON"),
        ] {
            let order = translate(
                &Action::ChooseDedication {
                    dedication: Name::new(civvis),
                },
                &mirror,
                &state,
            )
            .expect("a known dedication translates");
            assert_eq!(order.kind, "dedication");
            assert_eq!(order.subject, None);
            assert_eq!(order.verb.as_deref(), Some(firaxis));
            assert_eq!(order.pos, None);
        }
        assert!(translate(
            &Action::ChooseDedication {
                dedication: Name::new("not_a_dedication"),
            },
            &mirror,
            &state,
        )
        .is_none());
    }

    #[test]
    fn a_city_strike_names_its_city_and_target() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 40,
            cities: vec![StateCity {
                id: 65_536,
                name: "Capital".to_string(),
                x: 6,
                y: 6,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let city = mirror.game.player_city_ids(0)[0];

        let strike = translate(
            &Action::CityStrike {
                city,
                target: (5, 6),
            },
            &mirror,
            &state,
        )
        .expect("a strike from a mapped city translates");
        assert_eq!(strike.kind, "city_strike");
        assert_eq!(strike.subject, Some(65_536));
        assert_eq!(strike.pos, Some(civvis::hex::axial_to_offset(5, 6)));

        let encampment = translate(
            &Action::EncampmentStrike {
                city,
                target: (7, 5),
            },
            &mirror,
            &state,
        )
        .expect("an encampment strike from a mapped city translates");
        assert_eq!(encampment.kind, "encampment_strike");
        assert_eq!(encampment.subject, Some(65_536));
        assert_eq!(encampment.pos, Some(civvis::hex::axial_to_offset(7, 5)));
    }

    #[test]
    fn delegations_and_embassies_name_the_firaxis_seat() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 40,
            rivals: vec![StateRival {
                player: 4,
                civ: "CIVILIZATION_PERSIA".to_string(),
                leader: "LEADER_NADER_SHAH".to_string(),
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

        let delegation = translate(&Action::SendDelegation { player: 1 }, &mirror, &state)
            .expect("a delegation to a mapped rival translates");
        assert_eq!(delegation.kind, "delegation");
        assert_eq!(delegation.subject, Some(4));
        assert_eq!(delegation.verb.as_deref(), Some("DIPLOMATIC_DELEGATION"));

        let denounce = translate(&Action::Denounce { player: 1 }, &mirror, &state)
            .expect("a denouncement of a mapped rival translates");
        assert_eq!(denounce.kind, "denounce");
        assert_eq!(denounce.subject, Some(4));
        assert_eq!(denounce.verb.as_deref(), Some("DENOUNCE"));

        let embassy = translate(&Action::SendEmbassy { player: 1 }, &mirror, &state)
            .expect("an embassy to a mapped rival translates");
        assert_eq!(embassy.kind, "delegation");
        assert_eq!(embassy.subject, Some(4));
        assert_eq!(embassy.verb.as_deref(), Some("RESIDENT_EMBASSY"));

        // An unmet seat still crosses, with no subject: the Lua side names it
        // `delegation_target_unmapped` instead of the bridge dropping it here.
        let unmet = translate(&Action::SendDelegation { player: 3 }, &mirror, &state)
            .expect("an unmapped rival still crosses for the ledger");
        assert_eq!(unmet.subject, None);
    }

    #[test]
    fn planner_sales_cross_as_sell_orders_with_a_floor() {
        use civvis::game::DealItems;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 144,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 144,
            rivals: vec![StateRival {
                player: 4,
                civ: "CIVILIZATION_PERSIA".to_string(),
                leader: "LEADER_NADER_SHAH".to_string(),
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

        // The exact quote run civvis-20260818T083142Z dropped at t156: one
        // surplus dyes copy for 84 gold and 1.1 a turn.
        let mut offer = DealItems::default();
        offer.resources.insert("dyes".to_string(), 1);
        let request = DealItems {
            gold: 84.0,
            gold_per_turn: 1.1,
            ..DealItems::default()
        };
        let (offer_copy, request_copy) = (offer.clone(), request.clone());
        let sale = translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(offer),
                request: Box::new(request),
            },
            &mirror,
            &state,
        )
        .expect("a luxury sale to a mapped rival translates");
        assert_eq!(sale.kind, "sell");
        assert_eq!(sale.subject, Some(4));
        assert_eq!(sale.verb.as_deref(), Some("RESOURCE_DYES=1"));
        // The floor is our own walk-away by the mirrored board's book: the
        // ask (84 + 25 × 1.1) less our net gain on the quote — not half the
        // ask, which is what it was until 2026-08-24.
        let (my_value, _) = mirror
            .game
            .trade_utilities(0, 1, &offer_copy, &request_copy);
        let asked = 84.0 + 25.0 * 1.1;
        // A mirrored board that cannot price the copy answers non-finite,
        // and the floor is then the whole ask.
        let net = if my_value.is_finite() { my_value } else { 0.0 };
        let walk_away = ((asked - net).ceil() as i32).max(SALE_FLOOR_MIN);
        assert_eq!(sale.pos, Some((walk_away, 0)));
        assert!(
            walk_away > 56,
            "the old half-ask floor of 56 undercut our own book: {walk_away}"
        );

        // A favor block and a strategic block ride the same verb, favor last,
        // and a tiny ask still carries the minimum floor.
        let mut mixed = DealItems::default();
        mixed.resources.insert("iron".to_string(), 10);
        mixed.resources.insert("dyes".to_string(), 1);
        mixed.diplomatic_favor = 10.0;
        let sale = translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(mixed),
                request: Box::new(DealItems {
                    gold: 18.0,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .expect("a mixed sale translates");
        assert_eq!(
            sale.verb.as_deref(),
            Some("RESOURCE_DYES=1,RESOURCE_IRON=10,FAVOR=10")
        );
        // A tiny ask carries our walk-away, never less than the minimum.
        let floor = sale.pos.unwrap().0;
        assert!((SALE_FLOOR_MIN..=18).contains(&floor), "floor {floor}");

        // An unmapped seat still crosses for the ledger, subject-less, like
        // the delegation arm: the Lua side names it `sell_target_unmapped`.
        let favor = DealItems {
            diplomatic_favor: 10.0,
            ..DealItems::default()
        };
        let unmapped = translate(
            &Action::Trade {
                player: 3,
                offer: Box::new(favor),
                request: Box::new(DealItems {
                    gold: 30.0,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .expect("an unmapped buyer still crosses");
        assert_eq!(unmapped.subject, None);
    }

    #[test]
    fn a_passage_purchase_crosses_as_a_buy_order_with_a_ceiling() {
        use civvis::game::DealItems;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 90,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 90,
            rivals: vec![StateRival {
                player: 4,
                civ: "CIVILIZATION_KONGO".to_string(),
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

        // Their Open Borders for our gold: the one purchase with an arm.
        let buy = translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(DealItems {
                    gold: 60.0,
                    ..DealItems::default()
                }),
                request: Box::new(DealItems {
                    open_borders: true,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .expect("a passage purchase from a mapped rival translates");
        assert_eq!(buy.kind, "buy");
        assert_eq!(buy.subject, Some(4));
        assert_eq!(buy.verb.as_deref(), Some("OPEN_BORDERS"));
        // The ceiling is what the passage is worth to us by the mirrored
        // board's book: the offer plus our net gain on the quote — not the
        // offer and half again, which is what it was until 2026-08-24.
        let worth =
            (mirror.game.passage_gold_value(0).ceil() as i32).clamp(1, BORDER_BUY_CEILING_MAX);
        assert_eq!(buy.pos, Some((worth, 0)));
        assert!(
            (28..90).contains(&worth),
            "the book, not the offer and half again: {worth}"
        );

        // Per-turn gold rides the same 25× book, and the ceiling never
        // crosses the cap however rich the offer.
        let rich = translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(DealItems {
                    gold: 200.0,
                    gold_per_turn: 4.0,
                    ..DealItems::default()
                }),
                request: Box::new(DealItems {
                    open_borders: true,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .expect("a rich passage purchase still translates");
        // A rich offer does not raise the ceiling: passage is worth what the
        // book says, and never more than the cap.
        assert_eq!(rich.pos, buy.pos);
        assert!(rich.pos.unwrap().0 <= BORDER_BUY_CEILING_MAX);

        // A mutual swap is a different agreement and stays skipped, named.
        assert!(translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(DealItems {
                    open_borders: true,
                    ..DealItems::default()
                }),
                request: Box::new(DealItems {
                    open_borders: true,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .is_none());

        // Passage plus a luxury on either side is not this arm's shape.
        let mut sweetened = DealItems {
            gold: 60.0,
            ..DealItems::default()
        };
        sweetened.resources.insert("dyes".to_string(), 1);
        assert!(translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(sweetened),
                request: Box::new(DealItems {
                    open_borders: true,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .is_none());

        // A free ask carries no ceiling and does not cross.
        assert!(translate(
            &Action::Trade {
                player: 1,
                offer: Box::new(DealItems::default()),
                request: Box::new(DealItems {
                    open_borders: true,
                    ..DealItems::default()
                }),
            },
            &mirror,
            &state,
        )
        .is_none());
    }

    #[test]
    fn purchases_and_other_deal_shapes_stay_untranslated() {
        use civvis::game::DealItems;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 84,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 84,
            rivals: vec![StateRival {
                player: 2,
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let trade = |offer: DealItems, request: DealItems| Action::Trade {
            player: 1,
            offer: Box::new(offer),
            request: Box::new(request),
        };

        // Buying tea for gold (t84 of the same run) is not a sale.
        let mut tea = DealItems::default();
        tea.resources.insert("tea".to_string(), 1);
        let purchase = trade(
            DealItems {
                gold: 75.0,
                gold_per_turn: 1.4,
                ..DealItems::default()
            },
            tea,
        );
        assert!(translate(&purchase, &mirror, &state).is_none());

        // A Great Work sale is a different validation and stays skipped.
        let mut work = DealItems::default();
        work.great_works.insert("religious_art".to_string(), 1);
        let art = trade(
            work,
            DealItems {
                gold: 103.0,
                gold_per_turn: 2.7,
                ..DealItems::default()
            },
        );
        assert!(translate(&art, &mirror, &state).is_none());

        // A resource-for-resource swap asks for something other than gold.
        let mut dyes = DealItems::default();
        dyes.resources.insert("dyes".to_string(), 1);
        let mut silk = DealItems::default();
        silk.resources.insert("silk".to_string(), 1);
        assert!(translate(&trade(dyes, silk), &mirror, &state).is_none());

        // Open Borders for gold is an agreement, not a sale.
        let borders = trade(
            DealItems {
                open_borders: true,
                ..DealItems::default()
            },
            DealItems {
                gold: 40.0,
                ..DealItems::default()
            },
        );
        assert!(translate(&borders, &mirror, &state).is_none());

        // Nothing offered is nothing sold.
        assert!(translate(
            &trade(
                DealItems::default(),
                DealItems {
                    gold: 40.0,
                    ..DealItems::default()
                }
            ),
            &mirror,
            &state
        )
        .is_none());
    }

    #[test]
    fn a_peace_tribute_rides_only_a_routed_offer_and_is_capped() {
        let mut state = StateSnapshot {
            turn: 120,
            gold: 1_000,
            ..StateSnapshot::default()
        };
        assert_eq!(
            peace_tribute_cap(&state, false),
            0,
            "a second front, fatigue, a religious lane or an envoy reclaim is white peace"
        );
        assert_eq!(
            peace_tribute_cap(&state, true),
            250,
            "a quarter of the treasury"
        );
        state.gold = 5_000;
        assert_eq!(
            peace_tribute_cap(&state, true),
            PEACE_TRIBUTE_MAX as i32,
            "and never more than the maximum"
        );
        state.gold = 50;
        assert_eq!(peace_tribute_cap(&state, true), 12);
        state.gold = 0;
        assert_eq!(peace_tribute_cap(&state, true), 0);
    }

    #[test]
    fn idle_favor_sells_on_every_plan_that_reports_one() {
        // The bank the live seat actually holds: 300 favor at t141 with three
        // met rivals — one rich, one richer but two points short of the
        // twenty-point win, one at war with us.
        let state = StateSnapshot {
            turn: 141,
            favor: Some(300.0),
            rivals: vec![
                StateRival {
                    player: 2,
                    gold: 250.0,
                    dvp: Some(3),
                    ..StateRival::default()
                },
                StateRival {
                    player: 4,
                    gold: 1300.0,
                    dvp: Some(17),
                    ..StateRival::default()
                },
                StateRival {
                    player: 5,
                    gold: 900.0,
                    dvp: Some(2),
                    at_war: true,
                    ..StateRival::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let science = Some(("science", None));

        let mut orders = Vec::new();
        assert_eq!(
            append_favor_sale_order(science, &state, &mut orders, 2.0),
            None
        );
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].kind, "sell");
        // Not the richest (Persia at 17 DVP would spend it on the win), not
        // the one at war: the rich rival at 3 DVP.
        assert_eq!(orders[0].subject, Some(2));
        // 300 − 120 reserve = 180 surplus, capped at one block of 150, at a
        // floor of a gold a point.
        assert_eq!(orders[0].verb.as_deref(), Some("FAVOR=150"));
        // ... at the engine's own book per point (2.0 here), not a flat Gold.
        assert_eq!(orders[0].pos, Some((300, 0)));

        // A Diplomacy plan sells its surplus too: the favor it was holding
        // buys only EXTRA votes, and those never register. Run
        // civvis-20260819T054901Z banked 566 points to t222 and cast none of
        // them, losing to the rival diplomatic victory it was saving for.
        for diplomatic in [
            Some(("diplomacy", None)),
            Some(("expansion", Some("diplomatic"))),
        ] {
            let mut sold = Vec::new();
            assert_eq!(
                append_favor_sale_order(diplomatic, &state, &mut sold, 2.0),
                None,
                "the diplomatic lane no longer banks what it cannot cast"
            );
            assert_eq!(sold.len(), 1, "and the surplus reaches a buyer");
            assert_eq!(sold[0].verb.as_deref(), Some("FAVOR=150"));
        }

        // No plan report still holds: an unknown intent is not licence to sell.
        let mut held = Vec::new();
        assert_eq!(
            append_favor_sale_order(None, &state, &mut held, 2.0),
            Some("favor_hold:no_plan")
        );
        assert!(held.is_empty());

        // Off the cadence, nothing; inside the reserve, nothing.
        let mut off_cadence = state.clone();
        off_cadence.turn = 142;
        assert_eq!(
            append_favor_sale_order(science, &off_cadence, &mut held, 2.0),
            Some("favor_hold:cadence")
        );
        let mut small = state.clone();
        small.favor = Some(139.0);
        assert_eq!(
            append_favor_sale_order(science, &small, &mut held, 2.0),
            Some("favor_hold:reserve")
        );
        // The last twenty above the reserve still sell, in a smaller block.
        let mut tail = state.clone();
        tail.favor = Some(140.0);
        assert_eq!(
            append_favor_sale_order(science, &tail, &mut held, 2.0),
            None
        );
        assert_eq!(
            held.pop().and_then(|order| order.verb),
            Some("FAVOR=20".to_string())
        );

        // The planner's own favor quote this turn takes precedence.
        let mut planned = vec![Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("FAVOR=10".to_string()),
            pos: Some((10, 0)),
        }];
        assert_eq!(
            append_favor_sale_order(science, &state, &mut planned, 2.0),
            Some("favor_hold:planner_sale")
        );
        assert_eq!(planned.len(), 1);

        // The planner's own ten-favor block is held on a Diplomacy plan and
        // passes on any other; a resource sale is never touched.
        let mut planner = vec![
            Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("FAVOR=10".to_string()),
                pos: Some((10, 0)),
            },
            Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("RESOURCE_DYES=1".to_string()),
                pos: Some((56, 0)),
            },
        ];
        assert_eq!(hold_planner_favor_sales(science, &mut planner), 0);
        assert_eq!(planner.len(), 2);
        // The diplomacy plan lets the planner's own quote stand now: those ten
        // points were held as "two votes at the next session", and the next
        // session cannot take them.
        assert_eq!(
            hold_planner_favor_sales(Some(("diplomacy", None)), &mut planner),
            0
        );
        assert_eq!(planner.len(), 2);
        // A seat with no plan report still holds: unknown intent is not licence.
        assert_eq!(hold_planner_favor_sales(None, &mut planner), 1);
        assert_eq!(planner.len(), 1);
        assert_eq!(planner[0].verb.as_deref(), Some("RESOURCE_DYES=1"));

        // Nobody eligible: every met rival at war or already near the win.
        let mut nobody = state.clone();
        nobody.rivals.remove(0);
        assert_eq!(
            append_favor_sale_order(science, &nobody, &mut held, 2.0),
            Some("favor_hold:no_buyer")
        );
        assert!(held.is_empty());
    }

    /// The out-of-space fallback: a Writer the host counts ZERO compatible
    /// empty slots for triggers the sale of one placed writing work — to the
    /// richest peaceful rival that is not the culture front-runner — so the
    /// freed slot seats the idle person and the treasury gets the price.
    #[test]
    fn a_slot_starved_writer_sells_a_placed_writing_work() {
        let starved_writer = StateUnit {
            id: 90,
            kind: "UNIT_GREAT_WRITER".to_string(),
            great_person: Some(StateGreatPerson {
                class: Some("GREAT_PERSON_CLASS_WRITER".to_string()),
                can_activate: false,
                empty_slots: Some(0),
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        };
        let works_city = StateCity {
            great_works: Some(vec![
                StateGreatWork {
                    kind: "GREATWORK_YING_1".to_string(),
                    object: "GREATWORKOBJECT_LANDSCAPE".to_string(),
                    ..StateGreatWork::default()
                },
                StateGreatWork {
                    kind: "GREATWORK_POE_1".to_string(),
                    object: "GREATWORKOBJECT_WRITING".to_string(),
                    ..StateGreatWork::default()
                },
            ]),
            ..StateCity::default()
        };
        let state = StateSnapshot {
            turn: 143, // 143 % 6 == 5, the lane's phase
            units: vec![starved_writer],
            cities: vec![works_city],
            rivals: vec![
                // The culture front-runner is also the richest: skipped, our
                // losses are culture steals and a work in their museum is
                // their tourism twice over.
                StateRival {
                    player: 2,
                    gold: 1500.0,
                    culture: 90.0,
                    ..StateRival::default()
                },
                StateRival {
                    player: 4,
                    gold: 800.0,
                    culture: 30.0,
                    ..StateRival::default()
                },
                StateRival {
                    player: 5,
                    gold: 900.0,
                    culture: 20.0,
                    at_war: true,
                    ..StateRival::default()
                },
            ],
            ..StateSnapshot::default()
        };

        let mut orders = Vec::new();
        assert_eq!(append_work_sale_order(&state, &mut orders), None);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].kind, "sell");
        assert_eq!(
            orders[0].subject,
            Some(4),
            "not the culture leader, not the war"
        );
        assert_eq!(
            orders[0].verb.as_deref(),
            Some("GREATWORK_POE_1=1"),
            "the writing work, not the landscape the writer cannot replace"
        );
        assert_eq!(orders[0].pos, Some((WORK_SALE_FLOOR, 0)));

        // Off the cadence: hold, whoever is starved.
        let mut off = state.clone();
        off.turn = 144;
        let mut held = Vec::new();
        assert_eq!(
            append_work_sale_order(&off, &mut held),
            Some("work_sale_hold:cadence")
        );

        // `None` slots — an older mod that cannot count — is not starvation;
        // neither is a person the host says can activate right now.
        let mut unknown = state.clone();
        unknown.units[0].great_person.as_mut().unwrap().empty_slots = None;
        assert_eq!(
            append_work_sale_order(&unknown, &mut held),
            Some("work_sale_hold:no_starved_person")
        );
        let mut activatable = state.clone();
        activatable.units[0]
            .great_person
            .as_mut()
            .unwrap()
            .can_activate = true;
        assert_eq!(
            append_work_sale_order(&activatable, &mut held),
            Some("work_sale_hold:no_starved_person")
        );

        // A starved Merchant is not a slot consumer and never triggers.
        let mut merchant = state.clone();
        merchant.units[0].great_person.as_mut().unwrap().class =
            Some("GREAT_PERSON_CLASS_MERCHANT".to_string());
        assert_eq!(
            append_work_sale_order(&merchant, &mut held),
            Some("work_sale_hold:no_starved_person")
        );

        // No placed work of the starved kind: only building capacity helps.
        let mut bare = state.clone();
        bare.cities[0].great_works = Some(vec![StateGreatWork {
            kind: "GREATWORK_YING_1".to_string(),
            object: "GREATWORKOBJECT_LANDSCAPE".to_string(),
            ..StateGreatWork::default()
        }]);
        assert_eq!(
            append_work_sale_order(&bare, &mut held),
            Some("work_sale_hold:no_matching_work")
        );

        // A deal already heading to the buyer's seat yields the turn.
        let mut busy = vec![Order {
            kind: "sell",
            subject: Some(4),
            verb: Some("RESOURCE_DYES=1".to_string()),
            pos: Some((30, 0)),
        }];
        assert_eq!(
            append_work_sale_order(&state, &mut busy),
            Some("work_sale_hold:deal_in_flight")
        );
        assert_eq!(busy.len(), 1);

        // The culture leader is still a buyer when they are the ONLY buyer.
        let mut lone = state.clone();
        lone.rivals = vec![StateRival {
            player: 2,
            gold: 1500.0,
            culture: 90.0,
            ..StateRival::default()
        }];
        let mut only = Vec::new();
        assert_eq!(append_work_sale_order(&lone, &mut only), None);
        assert_eq!(only[0].subject, Some(2));
        assert!(held.is_empty());
    }

    #[test]
    fn final_aid_request_gold_gift_takes_an_affordable_first_place_without_raiding_the_bank() {
        let aid = StateEmergency {
            kind: "EMERGENCY_SEND_AID".to_string(),
            target: 4,
            turns_left: AID_GIFT_FINISH_WINDOW,
            begun: true,
            scores: vec![StateEmergencyScore {
                player: 2,
                score: 200.0,
                tier: Some(1),
            }],
            ours: StateEmergencyOurs {
                member: true,
                score: Some(50.0),
                tier: Some(2),
            },
        };
        let state = StateSnapshot {
            // 251 - the 100 reserve is exactly the 151 score needed to pass
            // the 200-point leader from our 50.
            gold: 251,
            emergencies: Some(vec![aid]),
            rivals: vec![StateRival {
                player: 4,
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        // An ordinary deal for the emergency target would consume the sole
        // working-deal slot. The DVP attempt gets it instead; a different
        // rival's passage request stays intact.
        let mut orders = vec![
            Order {
                kind: "sell",
                subject: Some(4),
                verb: Some("RESOURCE_DYES=1".to_string()),
                pos: Some((30, 0)),
            },
            Order {
                kind: "buy",
                subject: Some(8),
                verb: Some("OPEN_BORDERS".to_string()),
                pos: Some((90, 0)),
            },
        ];
        assert_eq!(append_aid_gift_order(&state, &mut orders), None);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].kind, "aid_gift", "the gift goes first");
        assert_eq!(orders[0].subject, Some(4));
        assert_eq!(orders[0].verb.as_deref(), Some("EMERGENCY_SEND_AID"));
        assert_eq!(orders[0].pos, Some((151, 0)), "one point past first");
        assert_eq!(orders[1].subject, Some(8), "other rivals keep their deal");
    }

    #[test]
    fn aid_gifts_require_a_live_authoritative_finish_line_and_safe_recipient() {
        let emergency = || StateEmergency {
            kind: "EMERGENCY_SEND_MILITARY_AID".to_string(),
            target: 4,
            turns_left: 2,
            begun: true,
            scores: vec![StateEmergencyScore {
                player: 2,
                score: 100.0,
                tier: Some(1),
            }],
            ours: StateEmergencyOurs {
                member: true,
                score: Some(50.0),
                tier: Some(2),
            },
        };
        let state = StateSnapshot {
            gold: 300,
            emergencies: Some(vec![emergency()]),
            rivals: vec![StateRival {
                player: 4,
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mut orders = Vec::new();
        assert_eq!(append_aid_gift_order(&state, &mut orders), None);
        assert_eq!(orders[0].pos, Some((51, 0)));

        // A real host queue already delivering the same 200-point project by
        // the deadline is worth more than a redundant treasury spend. Unknown
        // or late estimates do not suppress the finish-line fallback.
        let mut queued = state.clone();
        queued.cities = vec![StateCity {
            producing: Some("PROJECT_SEND_AID".to_string()),
            production_turns: 2.0,
            ..StateCity::default()
        }];
        let mut no_order = Vec::new();
        assert_eq!(
            append_aid_gift_order(&queued, &mut no_order),
            Some("aid_gift_hold:no_finishable_lead")
        );
        assert!(no_order.is_empty());
        queued.cities[0].production_turns = 3.0;
        assert_eq!(append_aid_gift_order(&queued, &mut no_order), None);
        assert_eq!(
            no_order[0].kind, "aid_gift",
            "a project that misses the deadline cannot suppress a gift"
        );

        // The tracker is the authorization. A same-shaped World Games score,
        // a pre-begin event, a non-member, an old/finished event, or an event
        // with more than the official 200-project gap can never spend gold.
        let mutations: [fn(&mut StateEmergency); 7] = [
            |event: &mut StateEmergency| event.kind = "EMERGENCY_WORLD_GAMES".to_string(),
            |event: &mut StateEmergency| event.begun = false,
            |event: &mut StateEmergency| event.ours.member = false,
            |event: &mut StateEmergency| event.turns_left = -1,
            |event: &mut StateEmergency| event.turns_left = AID_GIFT_FINISH_WINDOW + 1,
            |event: &mut StateEmergency| event.scores[0].score = 251.0,
            |event: &mut StateEmergency| event.ours.score = None,
        ];
        for mutate in mutations {
            let mut blocked = state.clone();
            mutate(blocked.emergencies.as_mut().unwrap().first_mut().unwrap());
            let mut no_order = Vec::new();
            assert_eq!(
                append_aid_gift_order(&blocked, &mut no_order),
                Some("aid_gift_hold:no_finishable_lead")
            );
            assert!(no_order.is_empty());
        }

        // The 100-Gold reserve and a known peaceful major recipient are both
        // hard guards.  No amount inferred from the tracker overrides either.
        let mut poor = state.clone();
        poor.gold = 150; // only 50 spendable, one short of the 51 gift
        let mut no_order = Vec::new();
        assert_eq!(
            append_aid_gift_order(&poor, &mut no_order),
            Some("aid_gift_hold:no_finishable_lead")
        );
        let mut unknown_target = state.clone();
        unknown_target.rivals.clear();
        assert_eq!(
            append_aid_gift_order(&unknown_target, &mut no_order),
            Some("aid_gift_hold:no_finishable_lead")
        );
        let mut war_target = state.clone();
        war_target.rivals[0].at_war = true;
        assert_eq!(
            append_aid_gift_order(&war_target, &mut no_order),
            Some("aid_gift_hold:no_finishable_lead")
        );

        // A peace, delegation, or war instruction names a different
        // diplomatic transition and has priority over a one-way Gold deal;
        // retaining it proves the fallback never silently changes that plan.
        for kind in ["peace", "delegation", "war"] {
            let planned = Order {
                kind,
                subject: Some(4),
                verb: Some("OTHER_DIPLOMACY".to_string()),
                pos: None,
            };
            let mut conflict = vec![planned];
            assert_eq!(
                append_aid_gift_order(&state, &mut conflict),
                Some("aid_gift_hold:diplomacy_conflict")
            );
            assert_eq!(conflict.len(), 1);
            assert_eq!(conflict[0].kind, kind);
        }
    }

    #[test]
    fn border_buys_aim_at_the_worst_seal() {
        // The Kongo run's shape: two majors sealing ground, one of them worse,
        // a third at war whose ground war already opens.
        let state = StateSnapshot {
            turn: 91, // 91 % 6 == 1, the lane's phase
            gold: 200,
            civics: vec![
                "CIVIC_CODE_OF_LAWS".to_string(),
                "CIVIC_EARLY_EMPIRE".to_string(),
            ],
            rivals: vec![
                StateRival {
                    player: 2,
                    ..StateRival::default()
                },
                StateRival {
                    player: 4,
                    ..StateRival::default()
                },
                StateRival {
                    player: 5,
                    at_war: true,
                    ..StateRival::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let sealed_by: std::collections::BTreeMap<usize, u32> =
            [(1, 7), (2, 21), (3, 40)].into_iter().collect();

        let mut orders = Vec::new();
        assert_eq!(
            append_border_buy_order(&sealed_by, &state, &mut orders, &|_| 40.0),
            None
        );
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].kind, "buy");
        // Seat 3 seals the most but is at war — war opens that ground by
        // itself. Seat 2 (host player 4) is the worst peaceful seal.
        assert_eq!(orders[0].subject, Some(4));
        assert_eq!(orders[0].verb.as_deref(), Some("OPEN_BORDERS"));
        // The engine's book for this passage (40 here) under the treasury's
        // 200 − 60 = 140 and the cap: the price is what passage is worth.
        assert_eq!(orders[0].pos, Some((40, 0)));

        // A grant already in hand retires the trigger; the next-worst seal
        // takes over.
        let mut granted = state.clone();
        granted.rivals[1].open_borders = Some(true);
        let mut next = Vec::new();
        assert_eq!(
            append_border_buy_order(&sealed_by, &granted, &mut next, &|_| 40.0),
            None
        );
        assert_eq!(next[0].subject, Some(2));

        // Small seals are not worth a deal window; no majors sealing, no ask.
        let trivial: std::collections::BTreeMap<usize, u32> =
            [(1, BORDER_BUY_MIN_SEALED - 1)].into_iter().collect();
        let mut held = Vec::new();
        assert_eq!(
            append_border_buy_order(&trivial, &state, &mut held, &|_| 40.0),
            Some("border_buy_hold:no_seal")
        );
        assert_eq!(
            append_border_buy_order(&Default::default(), &state, &mut held, &|_| 40.0),
            Some("border_buy_hold:no_seal")
        );

        // Off the cadence, without the civic, or too poor to meet a minimum
        // ask: held, with the seal still standing.
        let mut off_cadence = state.clone();
        off_cadence.turn = 92;
        assert_eq!(
            append_border_buy_order(&sealed_by, &off_cadence, &mut held, &|_| 40.0),
            Some("border_buy_hold:cadence")
        );
        let mut no_civic = state.clone();
        no_civic.civics = vec!["CIVIC_CODE_OF_LAWS".to_string()];
        assert_eq!(
            append_border_buy_order(&sealed_by, &no_civic, &mut held, &|_| 40.0),
            Some("border_buy_hold:no_civic")
        );
        let mut poor = state.clone();
        poor.gold = 89; // 89 − 60 reserve = 29, one short of the minimum
        assert_eq!(
            append_border_buy_order(&sealed_by, &poor, &mut held, &|_| 40.0),
            Some("border_buy_hold:treasury")
        );
        assert!(held.is_empty());

        // A deal already heading to the same rival this turn holds the buy —
        // the mod runs one working deal per rival at a time.
        let mut in_flight = vec![Order {
            kind: "sell",
            subject: Some(4),
            verb: Some("FAVOR=10".to_string()),
            pos: Some((10, 0)),
        }];
        assert_eq!(
            append_border_buy_order(&sealed_by, &state, &mut in_flight, &|_| 40.0),
            Some("border_buy_hold:deal_in_flight")
        );
        assert_eq!(in_flight.len(), 1);
    }

    #[test]
    fn luxury_buys_fill_the_amenity_gap() {
        // Two cities one Amenity short and one with a surplus: a deficit of
        // two by the host's own ledger, on the lane's turn, with income to
        // carry a copy and three majors met — one at war, one broke.
        let short = |needed: f64, have: f64| StateCity {
            amenities: have,
            amenities_needed: needed,
            ..StateCity::default()
        };
        let state = StateSnapshot {
            turn: 94, // 94 % 6 == 4, the lane's phase
            gold: 200,
            gold_per_turn: Some(12.0),
            cities: vec![short(5.0, 4.0), short(6.0, 5.0), short(3.0, 4.0)],
            rivals: vec![
                StateRival {
                    player: 2,
                    gold: 300.0,
                    ..StateRival::default()
                },
                StateRival {
                    player: 4,
                    gold: 500.0,
                    ..StateRival::default()
                },
                StateRival {
                    player: 5,
                    gold: 900.0,
                    at_war: true,
                    ..StateRival::default()
                },
                StateRival {
                    player: 6,
                    ..StateRival::default()
                },
            ],
            ..StateSnapshot::default()
        };
        assert_eq!(amenity_deficit(&state), 2.0);

        let mut orders = Vec::new();
        assert_eq!(append_luxury_buy_order(&state, &mut orders), None);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].kind, "buy");
        // The richest major at peace; seat 5 is richer and at war, seat 6's
        // treasury is unknown and reads as nothing.
        assert_eq!(orders[0].subject, Some(4));
        assert_eq!(orders[0].verb.as_deref(), Some("LUXURY_ANY"));
        // 135 + 10·3 = 165, under what 12 a turn carries (25·(12−4) = 200)
        // and the cap.
        assert_eq!(orders[0].pos, Some((165, 0)));

        // The income binds: 8 a turn carries 100 at the 25× book with 4 kept
        // clear; the cap binds a wide empire on a fat income.
        let mut lean = state.clone();
        lean.gold_per_turn = Some(8.0);
        let mut lean_orders = Vec::new();
        assert_eq!(append_luxury_buy_order(&lean, &mut lean_orders), None);
        assert_eq!(lean_orders[0].pos, Some((100, 0)));
        let mut wide = state.clone();
        wide.gold_per_turn = Some(40.0);
        wide.cities = (0..20).map(|_| short(6.0, 5.0)).collect();
        let mut wide_orders = Vec::new();
        assert_eq!(append_luxury_buy_order(&wide, &mut wide_orders), None);
        assert_eq!(wide_orders[0].pos, Some((300, 0)));

        // Content, or with only a ledger the host never sent: no ask.
        let mut held = Vec::new();
        let mut content = state.clone();
        content.cities = vec![short(5.0, 4.0), short(3.0, 4.0)];
        assert_eq!(
            append_luxury_buy_order(&content, &mut held),
            Some("luxury_buy_hold:content")
        );
        let mut unknown = state.clone();
        unknown.cities = vec![
            short(f64::NAN, f64::NAN),
            short(-1.0, -1.0),
            short(9.0, 4.0),
        ];
        assert_eq!(amenity_deficit(&unknown), 5.0);
        unknown.cities.pop();
        assert_eq!(
            append_luxury_buy_order(&unknown, &mut held),
            Some("luxury_buy_hold:content")
        );

        // Off the cadence, income unknown, income too thin for the cheapest
        // ask on record, or the treasury under the reserve: held.
        let mut off_cadence = state.clone();
        off_cadence.turn = 95;
        assert_eq!(
            append_luxury_buy_order(&off_cadence, &mut held),
            Some("luxury_buy_hold:cadence")
        );
        let mut blind = state.clone();
        blind.gold_per_turn = None;
        assert_eq!(
            append_luxury_buy_order(&blind, &mut held),
            Some("luxury_buy_hold:income_unknown")
        );
        let mut thin = state.clone();
        thin.gold_per_turn = Some(5.0); // 25·(5−4) = 25, under the 50 minimum
        assert_eq!(
            append_luxury_buy_order(&thin, &mut held),
            Some("luxury_buy_hold:income")
        );
        let mut poor = state.clone();
        poor.gold = 59;
        assert_eq!(
            append_luxury_buy_order(&poor, &mut held),
            Some("luxury_buy_hold:treasury")
        );
        assert!(held.is_empty());

        // A deal already heading to the richest seat this turn passes the
        // ask to the next-richest; with every peaceful seat busy, or nobody
        // met, there is nobody to buy from.
        let mut in_flight = vec![Order {
            kind: "sell",
            subject: Some(4),
            verb: Some("FAVOR=10".to_string()),
            pos: Some((10, 0)),
        }];
        assert_eq!(append_luxury_buy_order(&state, &mut in_flight), None);
        assert_eq!(in_flight.len(), 2);
        assert_eq!(in_flight[1].subject, Some(2));
        let mut all_busy: Vec<Order> = [2, 4, 6]
            .into_iter()
            .map(|seat| Order {
                kind: "buy",
                subject: Some(seat),
                verb: Some("OPEN_BORDERS".to_string()),
                pos: Some((40, 0)),
            })
            .collect();
        assert_eq!(
            append_luxury_buy_order(&state, &mut all_busy),
            Some("luxury_buy_hold:no_seller")
        );
        assert_eq!(all_busy.len(), 3);
        let mut alone = state.clone();
        alone.rivals.clear();
        assert_eq!(
            append_luxury_buy_order(&alone, &mut held),
            Some("luxury_buy_hold:no_seller")
        );
    }

    #[test]
    fn city_state_war_and_peace_orders_name_the_firaxis_seat() {
        // Live run civvis-20260816T070212Z reached this shape at t92: CIVVIS
        // chose a staged war on Nazca (host player 13), but translating only
        // through `state.rivals` emitted subject -1 and the control mod refused
        // it as `war_target_unmapped` three times.
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 92,
            minors: vec![StateMinor {
                player: 13,
                civ: "CIVILIZATION_NAZCA".to_string(),
                cities: vec![StateCity {
                    id: 65_536,
                    name: "Nazca".to_string(),
                    x: 6,
                    y: 6,
                    pop: 2,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
        let nazca = mirror
            .game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
            .expect("Nazca must occupy a mirrored city-state seat")
            .id;

        let war = translate(&Action::DeclareWar { player: nazca }, &mirror, &state)
            .expect("a city-state war crosses the order boundary");
        assert_eq!(war.kind, "war");
        assert_eq!(war.subject, Some(13));

        let peace = translate(&Action::MakePeace { player: nazca }, &mirror, &state)
            .expect("a city-state peace crosses the same boundary");
        assert_eq!(peace.kind, "peace");
        assert_eq!(peace.subject, Some(13));
    }

    /// ★★★★★ The whole envoy chain inside the bridge: the held count reaches
    /// the board, the deployed controller spends it on the planning clone, and
    /// each `SendEnvoy` crosses as an `envoy` order naming Firaxis's minor
    /// player id — including a city-state met before its centre is in view,
    /// which has no mirrored city to resolve through.
    #[test]
    fn held_envoys_are_spent_by_the_plan_and_cross_as_envoy_orders() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 60,
            envoys_free: Some(3),
            cities: vec![StateCity {
                id: 65_536,
                name: "Rome".to_string(),
                x: 2,
                y: 2,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            minors: vec![
                StateMinor {
                    player: 9,
                    civ: "CIVILIZATION_GENEVA".to_string(),
                    envoys: 1,
                    suzerain: -1,
                    cities: vec![StateCity {
                        id: 65_536,
                        name: "Geneva".to_string(),
                        x: 8,
                        y: 8,
                        pop: 3,
                        capital: true,
                        ..StateCity::default()
                    }],
                    ..StateMinor::default()
                },
                // Met by contact, centre still in the fog: no city exported.
                StateMinor {
                    player: 12,
                    civ: "CIVILIZATION_KABUL".to_string(),
                    envoys: 0,
                    suzerain: -1,
                    ..StateMinor::default()
                },
            ],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 2, 250, 0);
        assert_eq!(
            mirror.game.players[0].envoys_free, 3,
            "the held count is mirrored"
        );
        let seats: Vec<usize> = mirror
            .game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
            .map(|player| player.id)
            .collect();
        let (geneva, kabul) = (seats[0], seats[1]);
        assert_eq!(mirror.game.players[geneva].civ, "Geneva");
        assert_eq!(mirror.game.players[kabul].civ, "Kabul");

        // Both cross by the seating rule, the unseen one included.
        let order = translate(&Action::SendEnvoy { player: geneva }, &mirror, &state)
            .expect("an envoy to a met city-state crosses");
        assert_eq!(order.kind, "envoy");
        assert_eq!(order.subject, Some(9));
        assert_eq!(order.verb.as_deref(), Some("GIVE_INFLUENCE_TOKEN"));
        let unseen = translate(&Action::SendEnvoy { player: kabul }, &mirror, &state)
            .expect("a city-state met before its centre is in view still gets its envoy");
        assert_eq!(unseen.subject, Some(12));
        // A major seat is not a city-state.
        assert!(translate(&Action::SendEnvoy { player: 1 }, &mirror, &state).is_none());

        // The deployed controller spends what the board holds.
        let mut ai = civvis::ai::AdvancedAi::new();
        ai.enable_live_bridge();
        let mut planned = mirror.game.clone();
        let begin = planned.log.len();
        ai.take_turn(&mut planned, 0);
        let sent: Vec<i64> = planned
            .log
            .since(begin)
            .filter(|(seat, _)| *seat == 0)
            .filter_map(|(_, action)| match action {
                Action::SendEnvoy { .. } => translate(action, &mirror, &state),
                _ => None,
            })
            .filter_map(|order| order.subject)
            .collect();
        assert_eq!(
            sent.len(),
            3,
            "every held envoy is spent on the planning board: {sent:?}"
        );
        assert!(
            sent.iter().all(|host| *host == 9 || *host == 12),
            "{sent:?}"
        );
        assert_eq!(planned.players[0].envoys_free, 0);
    }

    /// The live planner may intentionally retain envoys after it has secured
    /// every met city-state past its final yield tier.  That decision has to
    /// survive the whole order boundary: `BasicAi` runs later in the same
    /// simulated turn, so its historical "highest count" fallback must not
    /// turn the bank straight back into `GIVE_INFLUENCE_TOKEN` orders.
    #[test]
    fn banked_secure_envoys_do_not_cross_the_live_order_boundary() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 60,
            envoys_free: Some(2),
            cities: vec![StateCity {
                id: 65_536,
                name: "Rome".to_string(),
                x: 2,
                y: 2,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            minors: vec![StateMinor {
                player: 9,
                civ: "CIVILIZATION_AUCKLAND".to_string(),
                envoys: 8,
                most_envoys: 8,
                suzerain: 0,
                cities: vec![StateCity {
                    id: 65_537,
                    name: "Auckland".to_string(),
                    x: 8,
                    y: 8,
                    pop: 3,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
        let auckland = mirror
            .game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
            .map(|player| player.id)
            .expect("Auckland must occupy a mirrored city-state seat");
        assert_eq!(mirror.game.envoys_at(0, auckland), 8);
        assert_eq!(mirror.game.suzerain_of(auckland), Some(0));
        assert_eq!(mirror.game.players[0].envoys_free, 2);

        let mut ai = civvis::ai::AdvancedAi::new();
        ai.enable_bank_envoys();
        let mut ours = std::collections::BTreeMap::new();
        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut ours,
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .expect("the decision is JSON");

        assert!(
            reply["orders"]
                .as_array()
                .expect("orders are an array")
                .iter()
                .all(|order| order["kind"] != "envoy"),
            "a safely owned eight-envoy city-state must retain the two-envoy bank: {reply}"
        );
    }

    /// A suzerain at war levies the city-state's army through the same seat
    /// resolution as the envoy; a major seat is not a city-state.
    #[test]
    fn a_levy_names_the_firaxis_city_state() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 120,
            minors: vec![StateMinor {
                player: 11,
                civ: "CIVILIZATION_KABUL".to_string(),
                envoys: 4,
                suzerain: 0,
                cities: vec![StateCity {
                    id: 65_536,
                    name: "Kabul".to_string(),
                    x: 8,
                    y: 8,
                    pop: 5,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
        let kabul = mirror
            .game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
            .expect("Kabul occupies the mirrored city-state seat")
            .id;
        assert_eq!(mirror.game.suzerain_of(kabul), Some(0));
        let levy = translate(&Action::LevyMilitary { player: kabul }, &mirror, &state)
            .expect("a levy of a suzerained city-state crosses");
        assert_eq!(levy.kind, "levy");
        assert_eq!(levy.subject, Some(11));
        assert_eq!(levy.verb.as_deref(), Some("LEVY_MILITARY"));
        assert!(translate(&Action::LevyMilitary { player: 1 }, &mirror, &state).is_none());
    }

    /// The first turn the lane opens on a seat holding fifty envoys must not
    /// issue fifty operations: eight cross in plan order, the rest wait for
    /// next turn's plan, and nothing else is touched.
    #[test]
    fn envoy_orders_are_bounded_per_turn_in_plan_order() {
        let envoy = |minor: i64| Order {
            kind: "envoy",
            subject: Some(minor),
            verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
            pos: None,
        };
        let mut orders: Vec<Order> = (0..12).map(|i| envoy(20 + i)).collect();
        orders.insert(
            0,
            Order {
                kind: "research",
                subject: None,
                verb: Some("TECH_WRITING".to_string()),
                pos: None,
            },
        );
        orders.push(Order {
            kind: "produce",
            subject: Some(65_536),
            verb: Some("BUILDING_LIBRARY".to_string()),
            pos: None,
        });
        let (kept, deferred) = bound_envoy_orders(orders);
        assert_eq!(deferred, 12 - ENVOY_ORDERS_PER_TURN);
        let envoys: Vec<i64> = kept
            .iter()
            .filter(|o| o.kind == "envoy")
            .filter_map(|o| o.subject)
            .collect();
        assert_eq!(
            envoys,
            (20..20 + ENVOY_ORDERS_PER_TURN as i64).collect::<Vec<_>>(),
            "plan order, first eight"
        );
        assert_eq!(
            kept.iter().filter(|o| o.kind != "envoy").count(),
            2,
            "other kinds pass untouched"
        );
        assert_eq!(kept.first().map(|o| o.kind), Some("research"));
        assert_eq!(kept.last().map(|o| o.kind), Some("produce"));
    }

    #[test]
    fn exact_spent_firaxis_titles_suppress_duplicate_governor_orders() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 92,
            governor_points: Some(4),
            governor_points_spent: Some(4),
            governors: Some(vec![
                StateGovernor {
                    kind: "GOVERNOR_THE_DEFENDER".to_string(),
                    city: 65_536,
                    x: 6,
                    y: 6,
                    established: true,
                    promotions: vec![
                        "GOVERNOR_PROMOTION_GARRISON_COMMANDER".to_string(),
                        "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS".to_string(),
                    ],
                    ..StateGovernor::default()
                },
                StateGovernor {
                    kind: "GOVERNOR_THE_RESOURCE_MANAGER".to_string(),
                    city: -1,
                    ..StateGovernor::default()
                },
            ]),
            cities: vec![StateCity {
                id: 65_536,
                name: "Capital".to_string(),
                x: 6,
                y: 6,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let mut ai = civvis::ai::AdvancedAi::new();

        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut Default::default(),
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .expect("the decision is JSON");

        assert!(reply["orders"].as_array().unwrap().iter().all(|order| {
            !order["kind"]
                .as_str()
                .is_some_and(|kind| kind.starts_with("governor_"))
        }));
    }

    #[test]
    fn policy_replacements_are_one_complete_host_deck() {
        let mut game = civvis::game::Game::new(2, 12, 12, 1, 50, 0);
        game.players[0]
            .policies
            .insert(civvis::name!("urban_planning"));
        game.players[0].policies.insert(civvis::name!("agoge"));

        let order = policy_deck_order(&game, 0);
        assert_eq!(order.kind, "policy_deck");
        assert_eq!(
            order.verb.as_deref(),
            Some("POLICY_AGOGE,POLICY_URBAN_PLANNING")
        );
    }

    #[test]
    fn renamed_units_and_improvements_use_firaxis_type_ids() {
        let winged_hussar = civvis::game::Item::Unit {
            unit: civvis::name!("winged_hussar"),
        };
        let keshig = civvis::game::Item::Unit {
            unit: civvis::name!("keshig"),
        };
        let oromo = civvis::game::Item::Unit {
            unit: civvis::name!("oromo_cavalry"),
        };
        let nau = civvis::game::Item::Unit {
            unit: civvis::name!("nau"),
        };
        let toa = civvis::game::Item::Unit {
            unit: civvis::name!("toa"),
        };
        assert_eq!(
            civ6_build_name(&winged_hussar).as_deref(),
            Some("UNIT_POLISH_HUSSAR")
        );
        assert_eq!(
            civ6_build_name(&keshig).as_deref(),
            Some("UNIT_MONGOLIAN_KESHIG")
        );
        assert_eq!(
            civ6_build_name(&oromo).as_deref(),
            Some("UNIT_ETHIOPIAN_OROMO_CAVALRY")
        );
        assert_eq!(
            civ6_build_name(&nau).as_deref(),
            Some("UNIT_PORTUGUESE_NAU")
        );
        assert_eq!(civ6_build_name(&toa).as_deref(), Some("UNIT_MAORI_TOA"));
        assert_eq!(
            civ6_improvement_type(&civvis::name!("seaside_resort")),
            "IMPROVEMENT_BEACH_RESORT"
        );
        assert_eq!(
            civ6_improvement_type(&civvis::name!("qhapaq_nan")),
            "IMPROVEMENT_MOUNTAIN_ROAD"
        );

        let district = civvis::game::Item::District {
            district: civvis::name!("campus"),
            pos: (7, 4),
        };
        let wonder = civvis::game::Item::Wonder {
            wonder: civvis::name!("pyramids"),
            pos: (3, 8),
        };
        assert_eq!(civ6_build_pos(&district), Some((9, 4)));
        assert_eq!(civ6_build_pos(&wonder), Some((7, 8)));
    }

    #[test]
    fn wall_production_and_repairs_use_firaxis_internal_type_ids() {
        for (civvis, firaxis) in [
            ("ancient_walls", "BUILDING_WALLS"),
            ("medieval_walls", "BUILDING_CASTLE"),
            ("renaissance_walls", "BUILDING_STAR_FORT"),
        ] {
            let building = civvis::game::Item::Building {
                building: civvis::name::Name::new(civvis),
            };
            assert_eq!(civ6_build_name(&building).as_deref(), Some(firaxis));

            let repair = civvis::game::Item::Repair {
                repair: civvis::name::Name::new(civvis),
                pos: (4, 4),
            };
            let game = civvis::game::Game::new(2, 12, 12, 1, 50, 0);
            assert_eq!(
                civ6_live_build_name(&repair, &game).as_deref(),
                Some(firaxis)
            );
        }
    }

    #[test]
    fn exported_lobby_size_and_horizon_override_cli_fallbacks() {
        let state = StateSnapshot {
            seat: civvis::mirror::Seat {
                players: 6,
                max_turns: 250,
                ..civvis::mirror::Seat::default()
            },
            ..StateSnapshot::default()
        };
        assert_eq!(mirror_setup(&state, 4, 500), (6, 250));
        assert_eq!(mirror_setup(&StateSnapshot::default(), 4, 500), (4, 500));
    }

    fn grass(x: i32, y: i32) -> Plot {
        Plot {
            x,
            y,
            t: Some("TERRAIN_GRASS".to_string()),
            f: None,
            r: None,
            o: -1,
            w: false,
            i: false,
            fw: None,
            im: None,
            rv: 0,
            ri: false,
            ct: None,
            cl: -1,
            p: false,
            d: None,
            dc: None,
            wo: None,
            rt: None,
            rp: false,
            yl: None,
            ap: None,
            np: false,
            vis: false,
        }
    }

    #[test]
    fn deciding_does_not_mutate_the_authoritative_live_mirror() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 7,
            name: "Roma".to_string(),
            x: 6,
            y: 6,
            pop: 3,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 42,
            kind: "UNIT_WARRIOR".to_string(),
            x: 6,
            y: 7,
            ..StateUnit::default()
        });

        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let before = serde_json::to_value(&mirror.game).expect("mirror game serializes");
        let mut ai = civvis::ai::AdvancedAi::new();

        let reply = decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut Default::default(),
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        );

        assert!(reply.contains("\"turn\":4"));
        assert_eq!(
            serde_json::to_value(&mirror.game).expect("mirror game serializes"),
            before,
            "planning must not leave an imagined end turn, queue, or produced unit on the live mirror"
        );
        assert_eq!(mirror.civ6_of.len(), 1);
        assert_eq!(mirror.uid_of.len(), 1);
    }

    #[test]
    fn converted_holy_site_never_emits_a_rival_missionary_purchase() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 151,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 151,
            techs: vec!["TECH_ASTROLOGY".to_string()],
            founded_religion: Some("RELIGION_CATHOLICISM".to_string()),
            founded_religions: vec![
                "RELIGION_CATHOLICISM".to_string(),
                "RELIGION_ISLAM".to_string(),
            ],
            faith: 1_000,
            cities: vec![StateCity {
                id: 65_536,
                name: "Rome".to_string(),
                x: 8,
                y: 8,
                pop: 7,
                capital: true,
                religion: Some("RELIGION_ISLAM".to_string()),
                buildings: vec!["BUILDING_SHRINE".to_string()],
                districts: vec![StateDistrict {
                    kind: "DISTRICT_HOLY_SITE".to_string(),
                    x: 9,
                    y: 8,
                    pillaged: false,
                    complete: true,
                    ..StateDistrict::default()
                }],
                producing: Some("PROJECT_HOLY_SITE_PRAYERS".to_string()),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let city = mirror.game.player_city_ids(0)[0];
        assert_eq!(
            mirror.game.players[0].religion.as_deref(),
            Some("Catholicism")
        );
        assert_eq!(
            mirror.game.city_religion(&mirror.game.cities[&city]),
            Some("Islam")
        );

        let mut ai = civvis::ai::AdvancedAi::new();
        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut Default::default(),
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        ))
        .expect("the decision is JSON");

        assert!(
            reply["orders"]
                .as_array()
                .unwrap()
                .iter()
                .all(|order| order["verb"].as_str() != Some("UNIT_MISSIONARY")),
            "a live recommendation must not buy the converted city's rival-faith Missionary"
        );
    }

    #[test]
    fn fresh_mirror_translates_a_zero_movement_trader_to_a_trade_route() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 20,
            civics: vec!["CIVIC_FOREIGN_TRADE".to_string()],
            cities: vec![
                StateCity {
                    id: 7,
                    name: "Roma".to_string(),
                    x: 6,
                    y: 6,
                    pop: 3,
                    capital: true,
                    ..StateCity::default()
                },
                StateCity {
                    id: 8,
                    name: "Antium".to_string(),
                    x: 7,
                    y: 7,
                    pop: 3,
                    ..StateCity::default()
                },
            ],
            units: vec![StateUnit {
                id: 42,
                kind: "UNIT_TRADER".to_string(),
                x: 7,
                y: 7,
                moves: 0.0,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let trader = mirror.uid_of[&42];

        assert_eq!(mirror.game.trade_capacity(0), 1);
        assert!(mirror.game.legal_actions(0).iter().any(|action| {
            matches!(action, Action::TradeRoute { unit, .. } if *unit == trader)
        }));

        let mut ai = civvis::ai::AdvancedAi::new();
        let reply = decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut Default::default(),
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
            },
        );

        assert!(
            reply.contains("\"verb\":\"TRADE_ROUTE\"") && reply.contains("\"subject\":42"),
            "a live trader cannot walk, but its legal route must still reach Civ VI: {reply}"
        );
    }

    #[test]
    fn active_firaxis_trade_route_keeps_the_trader_visible_but_out_of_the_plan() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 20,
            civics: vec!["CIVIC_FOREIGN_TRADE".to_string()],
            cities: vec![
                StateCity {
                    id: 7,
                    name: "Roma".to_string(),
                    x: 6,
                    y: 6,
                    pop: 3,
                    capital: true,
                    ..StateCity::default()
                },
                StateCity {
                    id: 8,
                    name: "Antium".to_string(),
                    x: 7,
                    y: 7,
                    pop: 3,
                    ..StateCity::default()
                },
            ],
            units: vec![StateUnit {
                id: 42,
                kind: "UNIT_TRADER".to_string(),
                x: 7,
                y: 7,
                moves: 0.0,
                ..StateUnit::default()
            }],
            trade_routes: vec![StateTradeRoute {
                trader: 42,
                origin: 8,
                destination: 7,
                origin_x: 7,
                origin_y: 7,
                destination_x: 6,
                destination_y: 6,
                ..StateTradeRoute::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let trader = mirror.uid_of[&42];

        assert!(
            mirror.game.units.contains_key(&trader),
            "the mirrored map must retain Firaxis's moving trader unit"
        );
        assert_eq!(
            mirror.game.active_routes(0),
            1,
            "the active route must occupy CIVVIS trade capacity and pay its yields"
        );
        assert!(mirror.active_trade_route_traders.contains(&42));

        let mut planning = mirror.game.clone();
        remove_active_route_traders_from_plan(&mut planning, &mirror);
        assert!(
            !planning.units.contains_key(&trader),
            "only the planning clone may consume a trader that Firaxis reports as busy"
        );
        assert_eq!(
            planning.active_routes(0),
            1,
            "removing the visual stand-in must not erase the real route's economic state"
        );
    }

    /// Every name here was refused by the live mod, and the count is from the ledger.
    ///
    /// The right-hand side is the row in `Cache/DebugGameplay.sqlite`, not a guess from
    /// the display name — that distinction is the whole bug. `BUILDING_ART_MUSEUM` reads
    /// perfectly and does not exist.
    #[test]
    fn a_build_order_names_the_row_civilization_vi_actually_ships() {
        use civvis::game::Item;
        use civvis::name::Name;

        let building = |n: &str| civ6_building_type(&Name::new(n));
        let district = |n: &str| {
            civ6_build_name(&Item::District {
                district: Name::new(n),
                pos: (0, 0),
            })
            .expect("a district always names something")
        };
        let project = |n: &str| {
            civ6_build_name(&Item::Project {
                project: Name::new(n),
            })
            .expect("a project always names something")
        };

        // The culture chain. 507 Museum orders across 45 runs, every one refused, and
        // not one city in any of them ever finished a Museum or held a Great Work.
        assert_eq!(building("art_museum"), "BUILDING_MUSEUM_ART");
        assert_eq!(
            building("archaeological_museum"),
            "BUILDING_MUSEUM_ARTIFACT"
        );
        assert_eq!(district("theater_square"), "DISTRICT_THEATER");
        // 310 refusals, and the inbound reader already had to grow a unique-prefix
        // rule to recover this one coming the other way.
        assert_eq!(district("government_plaza"), "DISTRICT_GOVERNMENT");
        assert_eq!(building("national_history_museum"), "BUILDING_GOV_CULTURE");
        // 110 refusals: an entire technology the seat could never select.
        assert_eq!(civ6_tech_name("wheel"), "TECH_THE_WHEEL");

        // The rest of the Government Plaza tier, named for the slot not the subject.
        assert_eq!(building("audience_chamber"), "BUILDING_GOV_TALL");
        assert_eq!(building("ancestral_hall"), "BUILDING_GOV_WIDE");
        assert_eq!(building("warlords_throne"), "BUILDING_GOV_CONQUEST");
        assert_eq!(building("foreign_ministry"), "BUILDING_GOV_CITYSTATES");
        assert_eq!(building("grand_masters_chapel"), "BUILDING_GOV_FAITH");
        assert_eq!(building("intelligence_agency"), "BUILDING_GOV_SPIES");
        assert_eq!(building("royal_society"), "BUILDING_GOV_SCIENCE");
        assert_eq!(building("war_department"), "BUILDING_GOV_MILITARY");

        assert_eq!(
            building("oil_power_plant"),
            "BUILDING_FOSSIL_FUEL_POWER_PLANT"
        );
        assert_eq!(building("nuclear_power_plant"), "BUILDING_POWER_PLANT");
        assert_eq!(
            building("mausoleum_at_halicarnassus"),
            "BUILDING_HALICARNASSUS_MAUSOLEUM"
        );
        assert_eq!(building("statue_of_liberty"), "BUILDING_STATUE_LIBERTY");
        assert_eq!(
            building("university_of_sankore"),
            "BUILDING_UNIVERSITY_SANKORE"
        );

        assert_eq!(
            district("water_park"),
            "DISTRICT_WATER_ENTERTAINMENT_COMPLEX"
        );
        assert_eq!(district("copacabana"), "DISTRICT_WATER_STREET_CARNIVAL");

        assert_eq!(
            project("exoplanet_expedition"),
            "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"
        );
        assert_eq!(project("launch_mars_colony"), "PROJECT_LAUNCH_MARS_BASE");
        assert_eq!(
            project("terrestrial_laser_station"),
            "PROJECT_TERRESTRIAL_LASER"
        );
        assert_eq!(project("lagrange_laser_station"), "PROJECT_ORBITAL_LASER");

        // ⚠ The mechanical path still has to carry the other ~230 names untouched. A
        // translation table that starts rewriting things it was not asked to rewrite is
        // the same defect pointed the other way.
        assert_eq!(building("monument"), "BUILDING_MONUMENT");
        assert_eq!(building("amphitheater"), "BUILDING_AMPHITHEATER");
        assert_eq!(district("campus"), "DISTRICT_CAMPUS");
        assert_eq!(civ6_tech_name("mining"), "TECH_MINING");
        assert_eq!(civ6_civic_name("drama_poetry"), "CIVIC_DRAMA_POETRY");
        assert_eq!(
            project("build_nuclear_device"),
            "PROJECT_BUILD_NUCLEAR_DEVICE"
        );
        // Already fixed before this change; keep it pinned so the table stays whole.
        assert_eq!(building("medieval_walls"), "BUILDING_CASTLE");
        assert_eq!(
            project("campus_research_grants"),
            "PROJECT_ENHANCE_DISTRICT_CAMPUS"
        );
    }

    /// ⚠ A wonder reaches Civilization VI as a BUILDING, so it must use the building
    /// table — #959 added the divergent spellings but left `Item::Wonder` formatting its
    /// own name one line away, so all three kept going out wrong. This asserts the two
    /// arms agree, which is the property that was actually missing.
    #[test]
    fn a_wonder_and_a_building_translate_through_the_same_table() {
        use civvis::game::Item;
        use civvis::name::Name;

        for name in [
            "mausoleum_at_halicarnassus",
            "statue_of_liberty",
            "university_of_sankore",
            "stonehenge",
            "great_bath",
        ] {
            let as_wonder = civ6_build_name(&Item::Wonder {
                wonder: Name::new(name),
                pos: (0, 0),
            })
            .expect("a wonder always names something");
            assert_eq!(
                as_wonder,
                civ6_building_type(&Name::new(name)),
                "{name} must translate the same way whichever Item variant carries it"
            );
        }

        assert_eq!(
            civ6_build_name(&Item::Wonder {
                wonder: Name::new("mausoleum_at_halicarnassus"),
                pos: (0, 0)
            })
            .as_deref(),
            Some("BUILDING_HALICARNASSUS_MAUSOLEUM")
        );
        // The plot is part of the decision and must survive the shared table.
        assert_eq!(
            civ6_build_pos(&Item::Wonder {
                wonder: Name::new("stonehenge"),
                pos: (3, 4)
            }),
            Some(civvis::hex::axial_to_offset(3, 4))
        );
    }

    /// The outbound table must not silently shrink relative to CIVVIS's own ruleset.
    ///
    /// This is the check that would have caught the original defect: it walks every
    /// building and district CIVVIS can order and fails on any whose translated name is
    /// still the mechanical uppercase when the shipped database has no such row. The
    /// known-divergent list is spelled out because CI has no Civilization VI install —
    /// if a ruleset entry joins that list, this test is where it gets recorded.
    #[test]
    fn every_name_civilization_vi_spells_differently_is_translated() {
        use civvis::name::Name;

        // Read out of `Cache/DebugGameplay.sqlite` on 2026-08-02 by comparing every
        // `data/*.json` key against Buildings/Districts/Projects/Technologies.
        const DIVERGENT_BUILDINGS: &[&str] = &[
            "ancient_walls",
            "medieval_walls",
            "renaissance_walls",
            "art_museum",
            "archaeological_museum",
            "oil_power_plant",
            "nuclear_power_plant",
            "mausoleum_at_halicarnassus",
            "statue_of_liberty",
            "university_of_sankore",
            "audience_chamber",
            "ancestral_hall",
            "warlords_throne",
            "foreign_ministry",
            "grand_masters_chapel",
            "intelligence_agency",
            "national_history_museum",
            "royal_society",
            "war_department",
        ];
        const DIVERGENT_DISTRICTS: &[&str] = &[
            "theater_square",
            "government_plaza",
            "water_park",
            "copacabana",
        ];

        for name in DIVERGENT_BUILDINGS {
            let mapped = civ6_building_type(&Name::new(name));
            assert_ne!(
                mapped,
                format!("BUILDING_{}", name.to_ascii_uppercase()),
                "{name} is known to be spelled differently in Civilization VI, so the \
                 mechanical uppercase is exactly the name the mod refuses"
            );
        }
        for name in DIVERGENT_DISTRICTS {
            let mapped = civ6_district_type(&Name::new(name));
            assert_ne!(
                mapped,
                format!("DISTRICT_{}", name.to_ascii_uppercase()),
                "{name} is known to be spelled differently in Civilization VI"
            );
        }
    }
}

/// The receiving side of the bridge: every check reads two consecutive frames,
/// never a return code.
#[cfg(test)]
mod order_postcondition_tests {
    use super::*;
    use civvis::mirror::{
        Snapshot, StateCity, StateGovernor, StateGreatPerson, StateMinor, StateRival,
        StateSnapshot, StateTradeRoute, StateUnit, TilesChunk,
    };

    fn order(
        kind: &str,
        subject: Option<i64>,
        verb: Option<&str>,
        pos: Option<(i32, i32)>,
    ) -> IssuedOrder {
        IssuedOrder {
            kind: kind.to_string(),
            subject,
            verb: verb.map(str::to_string),
            pos,
        }
    }

    fn unit(id: i64, kind: &str, x: i32, y: i32) -> StateUnit {
        StateUnit {
            id,
            kind: kind.to_string(),
            x,
            y,
            hp: 100.0,
            ..StateUnit::default()
        }
    }

    fn city(id: i64, x: i32, y: i32, producing: Option<&str>) -> StateCity {
        StateCity {
            id,
            name: format!("city-{id}"),
            x,
            y,
            producing: producing.map(str::to_string),
            ..StateCity::default()
        }
    }

    fn frame(turn: u32) -> StateSnapshot {
        StateSnapshot {
            turn,
            ..StateSnapshot::default()
        }
    }

    fn no_tiles() -> Snapshot {
        Snapshot::from_chunks(&[])
    }

    fn check(
        order: &IssuedOrder,
        before: &StateSnapshot,
        after: &StateSnapshot,
        evidence: &[serde_json::Value],
    ) -> Verdict {
        verify_order(order, before.turn, before, after, &no_tiles(), evidence)
    }

    fn failed(why: &str) -> Verdict {
        Verdict::Failed(why.to_string())
    }

    fn event(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("test event is JSON")
    }

    #[test]
    fn a_move_is_verified_by_the_unit_standing_closer_not_by_the_return_code() {
        let mut before = frame(42);
        before.units = vec![unit(7, "UNIT_SETTLER", 13, 11)];
        let walk = order("unit", Some(7), Some("MOVE_TO"), Some((17, 11)));

        let mut closer = frame(43);
        closer.units = vec![unit(7, "UNIT_SETTLER", 15, 11)];
        assert_eq!(check(&walk, &before, &closer, &[]), Verdict::Verified);

        let mut arrived = frame(43);
        arrived.units = vec![unit(7, "UNIT_SETTLER", 17, 11)];
        assert_eq!(check(&walk, &before, &arrived, &[]), Verdict::Verified);

        // The measured case: `applied = true`, no refusal, and the settler at
        // (12,9) — farther from (14,11) than it started.
        let mut away = frame(43);
        away.units = vec![unit(7, "UNIT_SETTLER", 9, 11)];
        assert_eq!(check(&walk, &before, &away, &[]), failed("moved_away"));

        let mut stood = frame(43);
        stood.units = vec![unit(7, "UNIT_SETTLER", 13, 11)];
        assert_eq!(check(&walk, &before, &stood, &[]), failed("did_not_move"));
    }

    #[test]
    fn a_settler_safety_rewrite_verifies_the_host_sent_leg() {
        let mut before = frame(50);
        before.units = vec![unit(7, "UNIT_SETTLER", 21, 21)];
        let walk = order("unit", Some(7), Some("MOVE_TO"), Some((21, 20)));

        let mut after = frame(51);
        after.units = vec![unit(7, "UNIT_SETTLER", 22, 22)];
        let evidence = vec![event(
            r#"{"kind":"settler_capture_escape","turn":50,"settler":7,"want":[21,20],"sent":[22,22]}"#,
        )];
        assert_eq!(
            check(&walk, &before, &after, &evidence),
            Verdict::Verified,
            "the bridge's safety rewrite is successful actuation even when it retreats"
        );
    }

    #[test]
    fn a_settler_consumed_by_its_own_founding_verifies_both_orders() {
        let mut before = frame(30);
        before.units = vec![unit(7, "UNIT_SETTLER", 12, 19)];
        let mut after = frame(31);
        after.cities = vec![city(65_536, 12, 19, Some("UNIT_WARRIOR"))];

        let walk = order("unit", Some(7), Some("MOVE_TO"), Some((12, 19)));
        let found = order("unit", Some(7), Some("FOUND_CITY"), None);
        assert_eq!(check(&walk, &before, &after, &[]), Verdict::Verified);
        assert_eq!(check(&found, &before, &after, &[]), Verdict::Verified);

        let mut still_no_city = frame(31);
        still_no_city.units = vec![unit(7, "UNIT_SETTLER", 12, 19)];
        assert_eq!(
            check(&found, &before, &still_no_city, &[]),
            failed("no_city")
        );
    }

    #[test]
    fn an_attack_is_verified_by_the_target_or_the_combat_record() {
        let mut before = frame(50);
        before.units = vec![unit(7, "UNIT_ARCHER", 10, 10)];
        let mut enemy = unit(900, "UNIT_WARRIOR", 11, 10);
        enemy.player = 3;
        before.hostiles = vec![enemy.clone()];
        let strike = order("unit", Some(7), Some("RANGE_ATTACK"), Some((11, 10)));

        let mut wounded = frame(51);
        wounded.units = before.units.clone();
        let mut hurt = enemy.clone();
        hurt.hp = 70.0;
        wounded.hostiles = vec![hurt];
        assert_eq!(check(&strike, &before, &wounded, &[]), Verdict::Verified);

        let mut killed = frame(51);
        killed.units = before.units.clone();
        assert_eq!(check(&strike, &before, &killed, &[]), Verdict::Verified);

        let mut untouched = frame(51);
        untouched.units = before.units.clone();
        untouched.hostiles = vec![enemy.clone()];
        assert_eq!(
            check(&strike, &before, &untouched, &[]),
            failed("target_unharmed")
        );
        let combat = event(r#"{"kind":"combat","turn":50,"attacker":{"id":7,"player":0}}"#);
        assert_eq!(
            check(&strike, &before, &untouched, &[combat]),
            Verdict::Verified
        );
    }

    #[test]
    fn production_and_purchases_are_read_off_the_city() {
        let mut before = frame(10);
        before.cities = vec![city(65_536, 20, 20, Some("UNIT_WARRIOR"))];
        before.gold = 200;
        let produce = order("produce", Some(65_536), Some("BUILDING_MONUMENT"), None);
        let purchase = order(
            "purchase",
            Some(65_536),
            Some("UNIT_BUILDER"),
            Some((0, -1)),
        );

        let mut queued = frame(11);
        queued.cities = vec![city(65_536, 20, 20, Some("BUILDING_MONUMENT"))];
        queued.gold = 210;
        assert_eq!(check(&produce, &before, &queued, &[]), Verdict::Verified);
        assert_eq!(
            check(&purchase, &before, &queued, &[]),
            failed("not_bought")
        );

        let mut ignored = frame(11);
        ignored.cities = vec![city(65_536, 20, 20, Some("UNIT_WARRIOR"))];
        ignored.gold = 210;
        assert_eq!(
            check(&produce, &before, &ignored, &[]),
            failed("producing=UNIT_WARRIOR")
        );

        let mut bought = frame(11);
        bought.cities = vec![city(65_536, 20, 20, Some("UNIT_WARRIOR"))];
        bought.units = vec![unit(8, "UNIT_BUILDER", 20, 20)];
        bought.gold = 210;
        assert_eq!(check(&purchase, &before, &bought, &[]), Verdict::Verified);

        // A completed item counts as much as a queued one.
        let mut finished = frame(11);
        let mut done = city(65_536, 20, 20, Some("UNIT_WARRIOR"));
        done.buildings = vec!["BUILDING_MONUMENT".to_string()];
        finished.cities = vec![done];
        assert_eq!(check(&produce, &before, &finished, &[]), Verdict::Verified);
    }

    #[test]
    fn a_purchased_unit_cannot_verify_a_same_turn_production_order() {
        let mut before = frame(30);
        before.cities = vec![city(65_536, 20, 20, Some("UNIT_SETTLER"))];

        let mut after = frame(31);
        after.cities = before.cities.clone();
        after.units = vec![unit(8, "UNIT_WARRIOR", 20, 20)];

        let produce = order("produce", Some(65_536), Some("UNIT_WARRIOR"), None);
        let purchase = order(
            "purchase",
            Some(65_536),
            Some("UNIT_WARRIOR"),
            Some((0, -1)),
        );
        let pending = PendingOrders {
            turn: 30,
            frame: 0,
            orders: vec![produce.clone(), purchase.clone()],
            before: before.clone(),
        };

        // Without a sibling purchase, the new unit remains valid completion
        // evidence for the production postcondition.
        assert_eq!(check(&produce, &before, &after, &[]), Verdict::Verified);

        let checks = verify_orders(&pending, &after, &no_tiles(), &[], &pending.orders);
        assert_eq!(checks[0].order, produce);
        assert_eq!(
            checks[0].verdict,
            failed("unit_appearance_ambiguous_purchase")
        );
        assert_eq!(checks[1].order, purchase);
        assert_eq!(checks[1].verdict, Verdict::Verified);
    }

    #[test]
    fn a_later_same_turn_fortify_frame_proves_a_transient_fortify() {
        let mut before = frame(30);
        before.units = vec![unit(7, "UNIT_WARRIOR", 20, 20)];

        // The next turn has already cleared the transient fortified state by
        // moving this unit, so the ordinary next-turn postcondition alone
        // would report `not_fortified`.
        let mut after = frame(31);
        after.units = vec![unit(7, "UNIT_WARRIOR", 21, 20)];

        // Frame 1 is emitted after frame 0's FORTIFY has reached the host.
        let mut same_turn = frame(30);
        same_turn.frame = 1;
        let mut fortified = unit(7, "UNIT_WARRIOR", 20, 20);
        fortified.fortified = true;
        fortified.fortify_turns = 1;
        same_turn.units = vec![fortified];

        let pending = PendingOrders {
            turn: 30,
            frame: 0,
            orders: vec![order("unit", Some(7), Some("FORTIFY"), None)],
            before,
        };
        let later = later_fortified_units(&[same_turn.clone()], 30, pending.frame);
        let checks = verify_orders_with_later_fortifications(
            &pending,
            &after,
            &no_tiles(),
            &[],
            &pending.orders,
            LaterOrderEvidence {
                fortified: &later,
                moved: &std::collections::BTreeSet::new(),
                policy_deck: false,
            },
        );
        assert_eq!(checks[0].verdict, Verdict::Verified);

        // Evidence from the frame on which the order was issued is not enough
        // to prove that the host accepted it.
        assert!(later_fortified_units(&[same_turn], 30, 1).is_empty());

        // A repeat FORTIFY issued after the unit was already fortified remains
        // a failure even if that old state is visible on a later frame.
        let mut already_fortified = frame(30);
        let mut old = unit(7, "UNIT_WARRIOR", 20, 20);
        old.fortified = true;
        old.fortify_turns = 1;
        already_fortified.units = vec![old];
        let repeated = PendingOrders {
            turn: 30,
            frame: 0,
            orders: vec![order("unit", Some(7), Some("FORTIFY"), None)],
            before: already_fortified,
        };
        let checks = verify_orders_with_later_fortifications(
            &repeated,
            &after,
            &no_tiles(),
            &[],
            &repeated.orders,
            LaterOrderEvidence {
                fortified: &later,
                moved: &std::collections::BTreeSet::new(),
                policy_deck: false,
            },
        );
        assert_eq!(checks[0].verdict, failed("not_fortified"));

        // ⚠⚠ A FORTIFY THE DECIDER ITSELF OVERRODE IS NOT A BRIDGE FAILURE.
        // `not_fortified` is the second-largest failure reason on the ledger —
        // 3124 of 5193 FORTIFY orders over the runs of 2026-08-30 — and 95% of
        // them had a MOVE_TO for the same unit in the same turn, 61% of those
        // issued AFTER the fortify. Reading that as `not_fortified` sends an
        // auditor after an actuation bug that is not there.
        let mut movable = frame(30);
        movable.units = vec![unit(7, "UNIT_WARRIOR", 20, 20)];
        let overridden = PendingOrders {
            turn: 30,
            frame: 0,
            orders: vec![order("unit", Some(7), Some("FORTIFY"), None)],
            before: movable,
        };
        let mut walked_away = std::collections::BTreeSet::new();
        walked_away.insert(7);
        let checks = verify_orders_with_later_fortifications(
            &overridden,
            &after,
            &no_tiles(),
            &[],
            &overridden.orders,
            LaterOrderEvidence {
                fortified: &std::collections::BTreeSet::new(),
                moved: &walked_away,
                policy_deck: false,
            },
        );
        assert_eq!(
            checks[0].verdict,
            failed("superseded_by_move"),
            "the decider walked this unit away after asking it to dig in"
        );
    }

    #[test]
    fn a_later_same_turn_policy_replan_is_not_a_missing_card_failure() {
        let before = frame(64);
        let mut after = frame(65);
        after.policies = vec![
            "POLICY_CHARISMATIC_LEADER".to_string(),
            "POLICY_COLONIZATION".to_string(),
            "POLICY_FEUDAL_CONTRACT".to_string(),
            "POLICY_URBAN_PLANNING".to_string(),
        ];
        let earlier = order(
            "policy_deck",
            None,
            Some(
                "POLICY_CHARISMATIC_LEADER,POLICY_COLONIZATION,POLICY_GOD_KING,POLICY_URBAN_PLANNING",
            ),
            None,
        );
        let later = order(
            "policy_deck",
            None,
            Some(
                "POLICY_CHARISMATIC_LEADER,POLICY_COLONIZATION,POLICY_FEUDAL_CONTRACT,POLICY_URBAN_PLANNING",
            ),
            None,
        );
        let pending = PendingOrders {
            turn: 64,
            frame: 0,
            orders: vec![earlier.clone()],
            before: before.clone(),
        };
        let later_pending = PendingOrders {
            turn: 64,
            frame: 2,
            orders: vec![later.clone()],
            before,
        };

        assert_eq!(
            check(&earlier, &pending.before, &after, &[]),
            failed("missing=POLICY_GOD_KING"),
            "without same-turn context, the final state correctly lacks the old card"
        );
        assert!(later_policy_deck_matches(
            &[later_pending],
            64,
            pending.frame,
            &after
        ));
        let checks = verify_orders_with_later_fortifications(
            &pending,
            &after,
            &no_tiles(),
            &[],
            &pending.orders,
            LaterOrderEvidence {
                fortified: &std::collections::BTreeSet::new(),
                moved: &std::collections::BTreeSet::new(),
                policy_deck: true,
            },
        );
        assert_eq!(checks[0].verdict, failed("superseded_by_policy_deck"));

        // A later deck that did not land must not hide a genuine missing card.
        let mut unchanged = after.clone();
        unchanged.policies = vec![
            "POLICY_CHARISMATIC_LEADER".to_string(),
            "POLICY_COLONIZATION".to_string(),
            "POLICY_URBAN_PLANNING".to_string(),
        ];
        assert!(!later_policy_deck_matches(
            &[PendingOrders {
                turn: 64,
                frame: 2,
                orders: vec![later],
                before: pending.before.clone(),
            }],
            64,
            pending.frame,
            &unchanged
        ));
    }

    #[test]
    fn a_bought_plot_is_verified_by_its_owner_on_the_board() {
        let mut before = frame(20);
        before.seat.local_player = 4;
        let mut after = frame(21);
        after.seat.local_player = 4;
        let buy = order("buy_plot", Some(65_536), None, Some((5, 6)));
        let tiles = |owner: i32| {
            let chunk: TilesChunk = serde_json::from_str(&format!(
                r#"{{"turn":21,"width":10,"height":10,"plots":[{{"x":5,"y":6,"o":{owner}}}]}}"#
            ))
            .expect("a tiles chunk parses");
            Snapshot::from_chunks(&[chunk])
        };
        assert_eq!(
            verify_order(&buy, 20, &before, &after, &tiles(4), &[]),
            Verdict::Verified
        );
        assert_eq!(
            verify_order(&buy, 20, &before, &after, &tiles(-1), &[]),
            failed("plot_not_owned")
        );
        assert_eq!(
            verify_order(&buy, 20, &before, &after, &no_tiles(), &[]),
            failed("plot_unseen")
        );
    }

    #[test]
    fn an_envoy_is_verified_by_the_city_state_count() {
        let minor = |envoys: i64| StateMinor {
            player: 9,
            envoys,
            ..StateMinor::default()
        };
        let mut before = frame(60);
        before.minors = vec![minor(1)];
        let envoy = order("envoy", Some(9), Some("GIVE_INFLUENCE_TOKEN"), None);
        let mut placed = frame(61);
        placed.minors = vec![minor(2)];
        assert_eq!(check(&envoy, &before, &placed, &[]), Verdict::Verified);
        let mut held = frame(61);
        held.minors = vec![minor(1)];
        assert_eq!(
            check(&envoy, &before, &held, &[]),
            failed("envoys_unchanged")
        );
    }

    /// A denouncement is verified by the host's own `GetDenounceTurn` on the
    /// next frame (#2590 exports it as `our_denounce_turn`), never by the
    /// session having been asked.
    #[test]
    fn a_denouncement_is_verified_by_the_hosts_own_denounce_turn() {
        let rival = |since: Option<i64>| StateRival {
            player: 2,
            our_denounce_turn: since,
            ..StateRival::default()
        };
        let mut before = frame(80);
        before.rivals = vec![rival(Some(-1))];
        let denounce = order("denounce", Some(2), Some("DENOUNCE"), None);

        let mut after = frame(81);
        after.rivals = vec![rival(Some(80))];
        assert_eq!(check(&denounce, &before, &after, &[]), Verdict::Verified);

        after.rivals = vec![rival(Some(-1))];
        assert_eq!(
            check(&denounce, &before, &after, &[]),
            failed("not_denounced")
        );

        after.rivals = vec![rival(None)];
        assert_eq!(
            check(&denounce, &before, &after, &[]),
            Verdict::Unverifiable,
            "an older mod that does not export the turn cannot answer"
        );

        after.rivals = Vec::new();
        assert_eq!(
            check(&denounce, &before, &after, &[]),
            failed("player_unseen")
        );
    }

    #[test]
    fn diplomacy_is_verified_by_the_answer_and_the_war_flag() {
        let rival = |at_war: bool| StateRival {
            player: 2,
            at_war,
            ..StateRival::default()
        };
        let mut before = frame(80);
        before.rivals = vec![rival(true)];
        let sale = order("sell", Some(2), Some("FAVOR=10"), Some((60, 0)));
        let peace = order("peace", Some(2), Some("MAKE_PEACE"), None);
        let war = order("war", Some(2), Some("DECLARE"), None);

        let mut after = frame(81);
        after.rivals = vec![rival(true)];
        assert_eq!(
            check(&sale, &before, &after, &[]),
            failed("no_deal_response")
        );
        let closed = event(r#"{"kind":"deal_closed","turn":80,"from":2,"gold":60}"#);
        assert_eq!(check(&sale, &before, &after, &[closed]), Verdict::Verified);
        let declined = event(r#"{"kind":"deal_declined","turn":80,"from":2}"#);
        assert_eq!(
            check(&sale, &before, &after, &[declined]),
            Verdict::Verified
        );
        let other_party = event(r#"{"kind":"deal_closed","turn":80,"from":5}"#);
        assert_eq!(
            check(&sale, &before, &after, &[other_party]),
            failed("no_deal_response")
        );

        assert_eq!(
            check(&peace, &before, &after, &[]),
            failed("no_peace_response")
        );
        let accepted = event(r#"{"kind":"peace_response","turn":80,"target":2,"accepted":true}"#);
        assert_eq!(
            check(&peace, &before, &after, std::slice::from_ref(&accepted)),
            failed("still_at_war")
        );
        let mut at_peace = frame(81);
        at_peace.rivals = vec![rival(false)];
        assert_eq!(
            check(&peace, &before, &at_peace, &[accepted]),
            Verdict::Verified
        );
        assert_eq!(check(&peace, &before, &at_peace, &[]), Verdict::Verified);
        let refused = event(r#"{"kind":"peace_response","turn":80,"target":2,"accepted":false}"#);
        assert_eq!(
            check(&peace, &before, &after, &[refused]),
            Verdict::Verified
        );

        let mut calm = frame(80);
        calm.rivals = vec![rival(false)];
        assert_eq!(check(&war, &calm, &after, &[]), Verdict::Verified);
        assert_eq!(check(&war, &calm, &at_peace, &[]), failed("not_at_war"));
    }

    #[test]
    fn governors_are_verified_by_the_seat_they_hold() {
        let governor = |city: i64| StateGovernor {
            kind: "GOVERNOR_THE_DEFENDER".to_string(),
            city,
            ..StateGovernor::default()
        };
        let mut before = frame(25);
        before.governors = Some(vec![]);
        let appoint = order(
            "governor_appoint",
            Some(65_536),
            Some("GOVERNOR_THE_DEFENDER"),
            Some((0, -1)),
        );
        let assign = order(
            "governor_assign",
            Some(65_536),
            Some("GOVERNOR_THE_DEFENDER"),
            Some((0, -1)),
        );

        let mut seated = frame(26);
        seated.governors = Some(vec![governor(65_536)]);
        assert_eq!(check(&appoint, &before, &seated, &[]), Verdict::Verified);
        assert_eq!(check(&assign, &before, &seated, &[]), Verdict::Verified);

        // The asynchronous appointment that never persisted its assignment.
        let mut unseated = frame(26);
        unseated.governors = Some(vec![governor(-1)]);
        assert_eq!(check(&appoint, &before, &unseated, &[]), Verdict::Verified);
        assert_eq!(
            check(&assign, &before, &unseated, &[]),
            failed("not_assigned")
        );

        let mut none = frame(26);
        none.governors = Some(vec![]);
        assert_eq!(
            check(&appoint, &before, &none, &[]),
            failed("not_appointed")
        );
    }

    #[test]
    fn empire_choices_are_read_back_from_the_frame() {
        let before = frame(5);
        let mut after = frame(6);
        after.research = Some("TECH_WRITING".to_string());
        after.civics = vec!["CIVIC_CODE_OF_LAWS".to_string()];
        after.government = Some("GOVERNMENT_CHIEFDOM".to_string());
        after.pantheon = Some("BELIEF_GOD_OF_THE_SEA".to_string());
        after.dedications = Some(vec!["COMMEMORATION_SCIENTIFIC".to_string()]);
        after.policies = vec!["POLICY_DISCIPLINE".to_string(), "POLICY_SURVEY".to_string()];
        after.units = vec![StateUnit {
            id: 55,
            great_person: Some(StateGreatPerson {
                class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
                ..StateGreatPerson::default()
            }),
            ..unit(55, "UNIT_GREAT_SCIENTIST", 1, 1)
        }];

        let cases = [
            (
                order("research", None, Some("TECH_WRITING"), None),
                Verdict::Verified,
            ),
            (
                order("research", None, Some("TECH_MINING"), None),
                failed("research=TECH_WRITING"),
            ),
            (
                order("civic", None, Some("CIVIC_CODE_OF_LAWS"), None),
                Verdict::Verified,
            ),
            (
                order("government", None, Some("GOVERNMENT_CHIEFDOM"), None),
                Verdict::Verified,
            ),
            (
                order("pantheon", None, Some("BELIEF_GOD_OF_THE_SEA"), None),
                Verdict::Verified,
            ),
            (
                order("dedication", None, Some("COMMEMORATION_SCIENTIFIC"), None),
                Verdict::Verified,
            ),
            (
                order(
                    "dedication",
                    None,
                    Some("COMMEMORATION_INFRASTRUCTURE"),
                    None,
                ),
                failed("not_dedicated"),
            ),
            (
                order(
                    "policy_deck",
                    None,
                    Some("POLICY_DISCIPLINE,POLICY_SURVEY"),
                    None,
                ),
                Verdict::Verified,
            ),
            (
                order(
                    "policy_deck",
                    None,
                    Some("POLICY_DISCIPLINE,POLICY_URBAN_PLANNING"),
                    None,
                ),
                failed("missing=POLICY_URBAN_PLANNING"),
            ),
            (
                order(
                    "gp_recruit",
                    None,
                    Some("GREAT_PERSON_CLASS_SCIENTIST"),
                    None,
                ),
                Verdict::Verified,
            ),
            (
                order(
                    "gp_recruit",
                    None,
                    Some("GREAT_PERSON_CLASS_MERCHANT"),
                    None,
                ),
                failed("no_great_person"),
            ),
            (
                order(
                    "gp_patronize_faith",
                    None,
                    Some("GREAT_PERSON_CLASS_SCIENTIST"),
                    None,
                ),
                Verdict::Verified,
            ),
        ];
        for (order, want) in cases {
            assert_eq!(
                check(&order, &before, &after, &[]),
                want,
                "{}",
                order.label()
            );
        }
    }

    #[test]
    fn unit_states_are_read_back_from_the_unit() {
        let mut before = frame(70);
        let mut builder = unit(3, "UNIT_BUILDER", 4, 4);
        builder.build_charges = Some(3);
        let mut trader = unit(5, "UNIT_TRADER", 6, 6);
        trader.moves = 2.0;
        before.units = vec![
            unit(1, "UNIT_WARRIOR", 2, 2),
            unit(2, "UNIT_WARRIOR", 3, 3),
            builder.clone(),
            trader,
        ];

        let mut after = frame(71);
        let mut fortified = unit(1, "UNIT_WARRIOR", 2, 2);
        fortified.fortify_turns = 1;
        let mut spent = builder.clone();
        spent.build_charges = Some(2);
        after.units = vec![
            fortified,
            unit(2, "UNIT_SWORDSMAN", 3, 3),
            spent,
            unit(5, "UNIT_TRADER", 6, 6),
        ];
        after.trade_routes = vec![StateTradeRoute {
            trader: 5,
            ..StateTradeRoute::default()
        }];

        let cases = [
            (
                order("unit", Some(1), Some("FORTIFY"), None),
                Verdict::Verified,
            ),
            (
                order("unit", Some(2), Some("FORTIFY"), None),
                failed("not_fortified"),
            ),
            (
                order("unit", Some(2), Some("UPGRADE"), None),
                Verdict::Verified,
            ),
            (
                order("unit", Some(1), Some("UPGRADE"), None),
                failed("same_unit_type"),
            ),
            (
                order("unit", Some(3), Some("IMPROVE:IMPROVEMENT_FARM"), None),
                Verdict::Verified,
            ),
            (
                order("unit", Some(5), Some("TRADE_ROUTE"), Some((9, 9))),
                Verdict::Verified,
            ),
            (
                order("unit", Some(1), Some("DELETE"), None),
                failed("still_exists"),
            ),
            (
                order("unit", Some(99), Some("DELETE"), None),
                Verdict::Verified,
            ),
            (
                order("unit", Some(2), Some("PROMOTE:PROMOTION_BATTLECRY"), None),
                failed("not_promoted"),
            ),
            (
                order("unit", Some(1), Some("SKIP_TURN"), None),
                Verdict::Unverifiable,
            ),
            (
                order("unit", Some(1), Some("SPY_TRAVEL_NEW_CITY"), Some((1, 1))),
                Verdict::Unverifiable,
            ),
        ];
        for (order, want) in cases {
            assert_eq!(
                check(&order, &before, &after, &[]),
                want,
                "{}",
                order.label()
            );
        }
    }

    #[test]
    fn every_issued_order_kind_is_checked_or_declared_unverifiable() {
        // Discover, never list: every `kind: "..."` literal this file can emit.
        let source = include_str!("civvis_orders.rs");
        let mut kinds: std::collections::BTreeSet<&str> = Default::default();
        // An `Order` literal is `kind: <expr>, subject: ...`; the expression is
        // a string literal or an `if` choosing between literals (`purchase` /
        // `purchase_faith`). Every quoted identifier in it that is not a
        // comparison operand (`== "faith"`) names a kind.
        for (index, _) in source.match_indices("kind: ") {
            let rest = &source[index + 6..];
            let Some(end) = rest.find("subject:") else {
                continue;
            };
            let expr = &rest[..end];
            if !(expr.starts_with('"') || expr.starts_with("if "))
                || expr.len() > 200
                || expr.contains(';')
            {
                continue;
            }
            for (at, _) in expr.match_indices('"') {
                let literal = &expr[at + 1..];
                let Some(close) = literal.find('"') else {
                    continue;
                };
                let kind = &literal[..close];
                let operand = expr[..at].trim_end().ends_with("==");
                if !kind.is_empty()
                    && kind.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                    && !operand
                    && !expr[..at].trim_end().ends_with(kind)
                {
                    kinds.insert(kind);
                }
            }
        }
        assert!(kinds.len() > 20, "the scan found only {kinds:?}");
        let mut before = frame(1);
        before.units = vec![unit(1, "UNIT_WARRIOR", 1, 1)];
        let mut after = frame(2);
        after.units = before.units.clone();
        for kind in kinds {
            if VERDICT_ROW_KINDS.contains(&kind) {
                continue;
            }
            let probe = if kind == "unit" {
                order("unit", Some(1), Some("MOVE_TO"), Some((1, 1)))
            } else if kind == "dedication" {
                order(
                    kind,
                    Some(1),
                    Some("COMMEMORATION_SCIENTIFIC"),
                    Some((1, 1)),
                )
            } else {
                order(kind, Some(1), Some(""), Some((1, 1)))
            };
            let verdict = if kind == "dedication" {
                let mut dedication_after = after.clone();
                dedication_after.dedications = Some(vec!["COMMEMORATION_SCIENTIFIC".to_string()]);
                check(&probe, &before, &dedication_after, &[])
            } else {
                check(&probe, &before, &after, &[])
            };
            assert!(
                (verdict != Verdict::Unverifiable) != unverifiable_kind(kind),
                "order kind {kind:?} is neither checked by `verify_order` nor listed in \
                 UNVERIFIABLE_ORDER_KINDS with a reason (got {verdict:?})"
            );
        }
        for (kind, why) in UNVERIFIABLE_ORDER_KINDS {
            assert!(!why.is_empty(), "{kind} needs a reason");
        }
        for (verb, why) in UNVERIFIABLE_UNIT_VERBS {
            assert!(!why.is_empty(), "{verb} needs a reason");
        }
    }

    #[test]
    fn the_verdict_rows_carry_the_turn_and_a_tally_the_mod_can_re_emit() {
        let checks = vec![
            OrderCheck {
                order: order("unit", Some(7), Some("MOVE_TO"), Some((3, 3))),
                verdict: Verdict::Verified,
            },
            OrderCheck {
                order: order("produce", Some(65_536), Some("BUILDING_MONUMENT"), None),
                verdict: failed("producing=UNIT_WARRIOR"),
            },
            OrderCheck {
                order: order("delegation", Some(2), Some("DIPLOMATIC_DELEGATION"), None),
                verdict: Verdict::Unverifiable,
            },
        ];
        let rows: Vec<String> = verdict_rows(42, &checks)
            .iter()
            .map(Order::to_json)
            .collect();
        assert_eq!(
            rows,
            vec![
                r#"{"kind":"order_verified","subject":7,"verb":"unit:MOVE_TO","x":42,"y":-1}"#,
                r#"{"kind":"order_failed","subject":65536,"verb":"produce:BUILDING_MONUMENT producing=UNIT_WARRIOR","x":42,"y":-1}"#,
                r#"{"kind":"turn_verified","subject":42,"verb":"issued=3 verified=1 failed=1 unverifiable=1","x":null,"y":null}"#,
            ]
        );
    }

    #[test]
    fn the_reply_keeps_its_orders_and_note_when_verdicts_are_appended() {
        let reply = r#"{"turn":43,"orders":[{"kind":"research","subject":null,"verb":"TECH_WRITING","x":null,"y":null}],"note":"roster=1"}"#;
        let rows = verdict_rows(42, &[]);
        let joined = append_rows_to_reply(reply, &rows);
        let value: serde_json::Value = serde_json::from_str(&joined).unwrap();
        assert_eq!(value["note"], "roster=1");
        assert_eq!(value["turn"], 43);
        assert_eq!(value["orders"].as_array().unwrap().len(), 2);
        assert_eq!(value["orders"][1]["kind"], "turn_verified");
        // The verdict rows are not orders: the next turn's pending set excludes them.
        let issued = IssuedOrder::from_reply(&joined);
        assert_eq!(
            issued,
            vec![order("research", None, Some("TECH_WRITING"), None)]
        );
        assert_eq!(append_rows_to_reply(reply, &[]), reply);
    }

    #[test]
    fn pending_orders_are_settled_by_the_next_turn_and_not_by_a_frame_of_the_same_turn() {
        let dir = std::env::temp_dir().join(format!("civvis-postcond-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let events = dir.join("events.jsonl");
        // The relay's own encoding, `": "` spacing included.
        std::fs::write(
            &events,
            "{\"kind\": \"turn\", \"turn\": 42, \"ctx\": \"agent\"}\n\
             {\"kind\": \"deal_closed\", \"turn\": 42, \"from\": 2, \"ctx\": \"agent\"}\n",
        )
        .unwrap();

        let mut before = frame(42);
        before.units = vec![unit(7, "UNIT_WARRIOR", 1, 1)];
        let mut pending = vec![PendingOrders {
            turn: 42,
            frame: 0,
            orders: vec![
                order("unit", Some(7), Some("MOVE_TO"), Some((3, 1))),
                order("sell", Some(2), Some("FAVOR=10"), Some((60, 0))),
            ],
            before,
        }];

        // A combat frame of the same turn answers nothing.
        let mut same_turn = frame(42);
        same_turn.frame = 1;
        assert!(settle_pending_orders(&mut pending, &events, &same_turn, &no_tiles()).is_empty());
        assert_eq!(pending.len(), 1);

        let mut next = frame(43);
        next.units = vec![unit(7, "UNIT_WARRIOR", 2, 1)];
        let rows: Vec<String> = settle_pending_orders(&mut pending, &events, &next, &no_tiles())
            .iter()
            .map(Order::to_json)
            .collect();
        assert!(pending.is_empty());
        assert_eq!(
            rows,
            vec![
                r#"{"kind":"order_verified","subject":7,"verb":"unit:MOVE_TO","x":42,"y":-1}"#,
                r#"{"kind":"order_verified","subject":2,"verb":"sell:FAVOR=10","x":42,"y":-1}"#,
                r#"{"kind":"turn_verified","subject":42,"verb":"issued=2 verified=2 failed=0 unverifiable=0","x":null,"y":null}"#,
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
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
mod civ6_name_audit {
    use super::*;
    use civvis::game::Item;
    use civvis::rules::Rules;
    use std::collections::BTreeSet;

    fn shipped() -> BTreeSet<String> {
        let raw = include_str!("../../data/civ6_type_names.json");
        serde_json::from_str::<Vec<String>>(raw)
            .expect("the type-name snapshot parses")
            .into_iter()
            .collect()
    }

    /// CIVVIS concepts Civilization VI has no build order for at all.
    ///
    /// A National Park is not an Improvement row in the shipped ruleset, an
    /// Antiquity Site and a Shipwreck are Resources an Archaeologist consumes,
    /// and a Rock Concert is a unit ability. Naming them here says "checked, and
    /// there is deliberately nothing to map" rather than leaving them to the
    /// uppercase fallthrough, which would emit a name and have it dropped.
    /// `PROJECT_REPAIR_ENCAMPMENT` is the one entry here that is a *deferral*
    /// rather than an absence: Civilization VI repairs a pillaged Encampment by
    /// ordering the district again, which needs a plot the `Item::Project` arm
    /// cannot see. `civ6_build_name` records why it is left untranslated and
    /// where the repair belongs. Listing it keeps that decision declared instead
    /// of indistinguishable from a name nobody has checked.
    /// Promotions that belong to a unit Civilization VI does not have.
    ///
    /// The Nihang is CIVVIS content: `data/promotions.json` gives it its own
    /// promotion class and the shipped ruleset has no row for any of them. They
    /// can never reach the wire on a live seat — the mirror cannot build a unit
    /// the host never fielded — so this is an absence, not an unfixed mapping.
    /// Naming them keeps that judgement declared and re-checked on every run.
    const CIVVIS_ONLY_PROMOTIONS: [&str; 7] = [
        "PROMOTION_CHAKRAM",
        "PROMOTION_DUMALLA",
        "PROMOTION_JANGI_KARA",
        "PROMOTION_JANGI_MOJEH",
        "PROMOTION_SANJO",
        "PROMOTION_TEGH",
        "PROMOTION_TREHSOOL_MUKH",
    ];

    const NO_CIV6_EQUIVALENT: [&str; 5] = [
        "IMPROVEMENT_NATIONAL_PARK",
        "IMPROVEMENT_ARCHAEOLOGICAL_DIG",
        "IMPROVEMENT_SHIPWRECK_EXCAVATION",
        "IMPROVEMENT_ROCK_CONCERT",
        "PROJECT_REPAIR_ENCAMPMENT",
    ];

    /// The exact regression, pinned: repairing a pillaged Government Plaza
    /// must not re-introduce CIVVIS's own spelling.
    ///
    /// Live run `civvis-20260803T094641Z` emitted `DISTRICT_GOVERNMENT_PLAZA`
    /// 26 times while `civ6_district_type` had already mapped it to
    /// `DISTRICT_GOVERNMENT` — because the repair arm formatted its own name.
    #[test]
    fn repairing_a_district_uses_the_same_spelling_as_building_one() {
        use civvis::game::{Game, Item};
        let mut board = Game::new(2, 8, 8, 7, 100, 0);
        let pos = (3, 3);
        board.map.tiles.get_mut(&pos).expect("a tile").district =
            Some(civvis::name::Name::new("government_plaza"));

        let repaired = civ6_live_build_name(
            &Item::Repair {
                repair: civvis::name::Name::new("district"),
                pos,
            },
            &board,
        )
        .expect("a district repair names a district");

        assert_eq!(
            repaired, "DISTRICT_GOVERNMENT",
            "the repair path must use civ6_district_type, not its own uppercase"
        );
        assert_eq!(
            repaired,
            civ6_district_type(&civvis::name::Name::new("government_plaza")),
            "one table, one formatter"
        );
        assert!(
            shipped().contains(&repaired),
            "and the result must be a name Civilization VI actually ships"
        );
    }

    #[test]
    fn the_snapshot_is_the_shipped_ruleset_and_not_our_own_mod() {
        let shipped = shipped();
        assert!(
            shipped.len() > 400,
            "the snapshot has {} names, too few to be Civilization VI's ruleset",
            shipped.len()
        );
        assert!(
            shipped.contains("BUILDING_MUSEUM_ARTIFACT"),
            "the real name of the building 248 live orders never built"
        );
        assert!(
            !shipped.contains("BUILDING_ARCHAEOLOGICAL_MUSEUM"),
            "CIVVIS's own spelling is in the snapshot, so the harvest read our \
             control mod back as evidence about the game — rerun \
             tools/civ6_type_names.py, which excludes DLC/CivvisControl"
        );
    }

    #[test]
    fn every_name_the_order_channel_can_emit_exists_in_civilization_vi() {
        let rules = Rules::shipped();
        let shipped = shipped();
        let mut missing: Vec<(&str, String, String)> = Vec::new();

        let mut check = |kind: &'static str, name: &str, emitted: String| {
            if !shipped.contains(&emitted)
                && !NO_CIV6_EQUIVALENT.contains(&emitted.as_str())
                && !CIVVIS_ONLY_PROMOTIONS.contains(&emitted.as_str())
            {
                missing.push((kind, name.to_string(), emitted));
            }
        };

        // ⚠ Drive `civ6_build_name`, the function the order channel actually
        // calls, rather than the per-kind helpers. Dispatch is where this has
        // gone wrong before: #959 put three divergent wonder spellings into
        // `civ6_building_type` while the `Item::Wonder` arm still formatted its
        // own name one line away, so the repair never reached the path that
        // emitted them. A test that reproduces the mapping instead of invoking
        // it reproduces that mistake too — this one wrote out the same stale
        // uppercase and reported three false failures against correct code.
        let anywhere: civvis::Pos = (0, 0);
        for name in rules.units.keys() {
            let item = Item::Unit { unit: *name };
            check(
                "unit",
                name.as_str(),
                civ6_build_name(&item).expect("a unit name"),
            );
        }
        for name in rules.districts.keys() {
            let item = Item::District {
                district: *name,
                pos: anywhere,
            };
            check(
                "district",
                name.as_str(),
                civ6_build_name(&item).expect("a district name"),
            );
        }
        for name in rules.buildings.keys() {
            let item = Item::Building { building: *name };
            check(
                "building",
                name.as_str(),
                civ6_build_name(&item).expect("a building name"),
            );
        }
        for name in rules.wonders.keys() {
            let item = Item::Wonder {
                wonder: *name,
                pos: anywhere,
            };
            check(
                "wonder",
                name.as_str(),
                civ6_build_name(&item).expect("a wonder name"),
            );
        }
        for name in rules.projects.keys() {
            let item = Item::Project { project: *name };
            check(
                "project",
                name.as_str(),
                civ6_build_name(&item).expect("a project name"),
            );
        }
        // Improvements are ordered by builders, not produced in a city, so they
        // reach the wire through their own helper rather than `civ6_build_name`.
        for name in rules.improvements.keys() {
            check("improvement", name.as_str(), civ6_improvement_type(name));
        }
        // ⚠ AND THE REPAIR PATH, which is a DIFFERENT FUNCTION.
        //
        // The first version of this audit drove `civ6_build_name` only. That
        // missed `civ6_live_build_name`'s `Item::Repair` arm, which formatted
        // its own uppercase — and live run `civvis-20260803T094641Z` then
        // emitted `DISTRICT_GOVERNMENT_PLAZA` 26 times, all discarded, on a
        // revision where every other path spelled it `DISTRICT_GOVERNMENT`.
        //
        // A name audit is only worth what it covers. Drive every function that
        // can put a name on the wire, not the one that is easiest to reach.
        let board = civvis::game::Game::new(2, 8, 8, 1, 100, 0);
        for name in rules.districts.keys() {
            let repaired = civ6_district_type(name);
            check("district-repair", name.as_str(), repaired);
        }
        for name in rules.buildings.keys() {
            let item = Item::Repair {
                repair: *name,
                pos: (0, 0),
            };
            if let Some(emitted) = civ6_live_build_name(&item, &board) {
                check("building-repair", name.as_str(), emitted);
            }
        }
        // ★★★ AND PROMOTIONS, WHICH THIS AUDIT DID NOT COVER UNTIL A WHOLE
        // FAMILY OF THEM WAS WRONG.
        //
        // The comment above says a name audit is only worth what it covers, and
        // then the audit covered five families out of six. Every spy promotion
        // went out as `PROMOTION_<NAME>` when the host ships
        // `PROMOTION_SPY_<NAME>` — 259-341 refusals a game, the seat's largest
        // refusal category, and a bridge-health drop from 96% to 87% that no
        // test noticed. Both sources are driven here: the ruleset's own table
        // and `Game::SPY_PROMOTIONS`, which is not in it.
        for name in rules.promotions.keys() {
            check(
                "promotion",
                name.as_str(),
                civ6_unit_promotion_name(name.as_str()),
            );
        }
        for name in civvis::game::Game::SPY_PROMOTIONS {
            check("spy-promotion", name, civ6_unit_promotion_name(name));
        }

        assert!(
            missing.is_empty(),
            "these names would be written to the order channel and silently \
             discarded by the host — add an arm to the matching civ6_*_type \
             function, or to NO_CIV6_EQUIVALENT if the game genuinely has no \
             build order for it:\n{}",
            missing
                .iter()
                .map(|(kind, name, emitted)| format!("  {kind:11} {name:32} -> {emitted}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
