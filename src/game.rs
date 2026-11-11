//! Core turn engine (mirrors civvis/game.py — same mechanics and action protocol).
use serde::ser::SerializeMap;
mod host_policy_windows;
#[cfg(test)]
mod host_trade_tests;
mod native_settlement_loyalty;
mod player_view;
pub use host_policy_windows::HostPolicyWindow;
pub use native_settlement_loyalty::NativeSettlementPressure;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::Arc;

use crate::leader_roster;
use crate::name::{AsName, Name};
use crate::parallel::WorkPool;
use crate::pedia::title as pretty;
use crate::rng::Rng;
use crate::rules::{
    building_yield_effect_key, grant_ability_effect_key, unit_purchase_discount_effect_key,
    volcanic_soil_valid_terrain, AgendaSpec, BuildingSpec, DifficultySpec, DisasterSpec,
    FutureTreeLayout, ModifierCollection, ModifierContext, ResourceIndustryEffects, Rules,
    SpeedSpec, Yields, ERA_NAMES,
};
use crate::specmap::SpecMap;

use crate::setup::{
    BaseRuleset, FutureEra, GameSpeed, MapPoles, MapScript, MapSize, MapTopology, TacticsRules,
    TurnStructure,
};
use crate::world::{DistrictFoundation, RememberedTile, Tile, TileBits, TileMemory, WorldMap};
use crate::{hex, mapgen, Pos};

pub use crate::leader_roster::LeaderPool;

/// The civilizations this ruleset can seat, in seating order. A stock lobby
/// seats majors straight down this list, so it must be at least as long as the
/// largest major count any map size offers — Huge tops out at twenty — or two
/// seats end up sharing a leader, an ability and a unique.
///
/// The order is fixed history: seat *i* of a game nobody customised has been
/// `CIV_NAMES[i]` since the first seed, so new civilizations are appended and
/// never inserted.
pub const CIV_NAMES: [&str; 105] = [
    "Rome",
    "Egypt",
    "Greece",
    "China",
    "Sumeria",
    "Aztec",
    "Nubia",
    "Scythia",
    "England",
    "Germany",
    "Russia",
    "Korea",
    "Maya",
    "Mali",
    "Phoenicia",
    "Byzantium",
    "Zulu",
    "Gaul",
    "Kongo",
    "Vietnam",
    "Brazil",
    "France",
    "Spain",
    "Portugal",
    "Netherlands",
    "Sweden",
    "Norway",
    "Denmark",
    "Poland",
    "Hungary",
    "Austria",
    "Bohemia",
    "Scotland",
    "Ireland",
    "Switzerland",
    "Venice",
    "Serbia",
    "Bulgaria",
    "Lithuania",
    "Ukraine",
    "Finland",
    "Romania",
    "Novgorod",
    "Prussia",
    "Catalonia",
    "Gujarat",
    "Assyria",
    "Persia",
    "Media",
    "Manchuria",
    "Lydia",
    "Parthia",
    "Sogdiana",
    "Ottomans",
    "Arabia",
    "Israel",
    "Armenia",
    "Georgia",
    "Timurids",
    "Kazakh",
    "Bactria",
    "Ethiopia",
    "Axum",
    "Morocco",
    "Numidia",
    "Songhai",
    "Ghana",
    "Benin",
    "Ashanti",
    "Swahili",
    "Great Zimbabwe",
    "Buganda",
    "Oyo",
    "Tuareg",
    "Madagascar",
    "India",
    "Japan",
    "Mongolia",
    "Tibet",
    "Nepal",
    "Kalinga",
    "Chola",
    "Bengal",
    "Maratha",
    "Khmer",
    "Siam",
    "Burma",
    "Majapahit",
    "Champa",
    "America",
    "Canada",
    "Pueblo",
    "Comanche",
    "Sioux",
    "Inca",
    "Muisca",
    "Mapuche",
    "Argentina",
    "Australia",
    "Maori",
    "Babylon",
    "Cree",
    "Gran Colombia",
    "Indonesia",
    "Macedon",
];

/// All fifty civilizations shipped by Civilization VI and its official DLC.
/// Keep the names in [`CIV_NAMES`] order: deterministic non-random callers
/// have always relied on the head of that list.
pub const CIV6_LEADER_POOL: [&str; 50] = [
    "Rome",
    "Egypt",
    "Greece",
    "China",
    "Sumeria",
    "Aztec",
    "Nubia",
    "Scythia",
    "England",
    "Germany",
    "Russia",
    "Korea",
    "Maya",
    "Mali",
    "Phoenicia",
    "Byzantium",
    "Zulu",
    "Gaul",
    "Kongo",
    "Vietnam",
    "Brazil",
    "France",
    "Spain",
    "Portugal",
    "Netherlands",
    "Sweden",
    "Norway",
    "Poland",
    "Hungary",
    "Scotland",
    "Persia",
    "Ottomans",
    "Arabia",
    "Georgia",
    "Ethiopia",
    "India",
    "Japan",
    "Mongolia",
    "Khmer",
    "America",
    "Canada",
    "Inca",
    "Mapuche",
    "Australia",
    "Maori",
    "Babylon",
    "Cree",
    "Gran Colombia",
    "Indonesia",
    "Macedon",
];
/// Every city-state identity this ruleset knows. Every seat needs its own
/// entry here — two city-states sharing a name would share an identity, and so
/// would a city-state sharing one with a civilization or with a city that
/// civilization can found. Append only: a city-state's index is its identity
/// in a saved game.
///
/// This list is *identity*, not placement. Seating order lives in
/// `data/city_states.json`, which puts Civilization VI's own forty-eight
/// city-states first so an ordinary game draws only from the shipped roster
/// and the extra names are reached only by the largest maps. Four entries here
/// (Carthage, Stockholm, Seoul, Amsterdam) were city-states in earlier
/// versions of the game and became playable capitals; they keep their indices
/// so old saves still resolve, and seat after the shipped forty-eight.
pub const CITY_STATE_NAMES: [&str; 182] = [
    "Kabul",
    "Geneva",
    "Carthage",
    "Hattusa",
    "Mohenjo-Daro",
    "Yerevan",
    "Zanzibar",
    "Auckland",
    "Valletta",
    "Vilnius",
    "Stockholm",
    "Kandy",
    "Jerusalem",
    "Brussels",
    "Preslav",
    "Antananarivo",
    "Seoul",
    "Amsterdam",
    "Akragas",
    "Gela",
    "Tarentum",
    "Croton",
    "Kos",
    "Knidos",
    "Pergamon",
    "Sinope",
    "Massalia",
    "Emporion",
    "Cyrene",
    "Leptis Magna",
    "Thapsus",
    "Malaca",
    "Palmyra",
    "Petra",
    "Ebla",
    "Kadesh",
    "Alalakh",
    "Qatna",
    "Emar",
    "Nuzi",
    "Sicyon",
    "Orchomenus",
    "Tiryns",
    "Megara",
    "Chalcis",
    "Eretria",
    "Dodona",
    "Elis",
    "Capua",
    "Tarquinia",
    "Veii",
    "Perusia",
    "Ancona",
    "Ragusa",
    "Amalfi",
    "Pisa",
    "Lubeck",
    "Rostock",
    "Wismar",
    "Riga",
    "Reval",
    "Visby",
    "Bruges",
    "Ghent",
    "Antwerp",
    "Soest",
    "Bamberg",
    "Goslar",
    "Ulm",
    "Basel City",
    "Sankt Gallen City",
    "Zurich City",
    "Bergen City",
    "Trondheim",
    "Wisby",
    "Elsinore",
    "Malmo",
    "Turku City",
    "Abo",
    "Narva",
    "Derbent",
    "Tanais",
    "Chersonesos",
    "Phanagoria",
    "Sarkel",
    "Itil",
    "Bolghar",
    "Suzdal",
    "Merv City",
    "Balasagun",
    "Talas",
    "Osh",
    "Kashgar",
    "Khotan",
    "Turfan",
    "Dunhuang",
    "Pagan City",
    "Pegu City",
    "Malacca",
    "Brunei",
    "Aceh",
    "Palembang",
    "Makassar",
    "Ternate",
    "Tidore",
    "Hoi An City",
    "Luang Prabang",
    "Vientiane",
    "Ayutthaya City",
    "Pattani",
    "Kedah",
    "Manila",
    "Cebu",
    "Ryukyu",
    "Naha",
    "Hakodate",
    "Sakai City",
    "Hakata City",
    "Kamakura City",
    "Dazaifu",
    "Gao City",
    "Kano",
    "Katsina",
    "Zaria",
    "Sokoto",
    "Ife",
    "Nri",
    "Bonny",
    "Cabinda",
    "Ndongo",
    "Adal",
    "Zeila",
    "Berbera",
    "Mogadishu",
    "Barawa",
    "Merca",
    "Cahokia",
    "Poverty Point",
    "Chaco",
    "Mesa Verde",
    "Taos",
    "Zuni",
    "Hopi",
    "Acoma",
    "Kabah",
    "Monte Alban",
    "El Tajin",
    "Cholula",
    "Chan Chan",
    "Chavin",
    "Akkad",
    "Anshan",
    "Armagh",
    "Ayutthaya",
    "Bandar Brunei",
    "Bologna",
    "Buenos Aires",
    "Caguana",
    "Cardiff",
    "Chinguetti",
    "Fez",
    "Granada",
    "Hong Kong",
    "Hunza",
    "Johannesburg",
    "Kumasi",
    "La Venta",
    "Lahore",
    "Mexico City",
    "Mitla",
    "Muscat",
    "Nalanda",
    "Nan Madol",
    "Nazca",
    "Ngazargamu",
    "Rapa Nui",
    "Samarkand",
    "Singapore",
    "Taruga",
    "Vatican City",
    "Venice",
    "Wolin",
];

// The city-name tables live in `src/game/city_names.rs`: 1,895 lines of
// content with no rules in it, which had been adding a conflict surface to
// every edit near the top of this file. See that file's header for the
// commit that claimed to move them and did not.
mod city_names;
use city_names::city_names;

#[cfg(test)]
mod city_name_tests;

#[cfg(test)]
mod seating_tests;

#[cfg(test)]
mod space_project_cost_tests;

#[cfg(test)]
mod modifier_attachment_tests;

#[cfg(test)]
mod domestic_tourist_tests;

/// Put `district` on a free tile of a test city and register it.
///
/// One copy for the whole repository: `src/ai/advanced/tests.rs` carried a
/// byte-identical `install_ai_test_district` until 2026-08-27.
#[cfg(test)]
pub(crate) fn install_test_district(game: &mut Game, city: u32, district: &str) -> Pos {
    let center = game.cities[&city].pos;
    let position = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != center
                && game.map.tiles[position].district.is_none()
                && game.map.tiles[position].wonder.is_none()
                && game.map.tiles[position].improvement.is_none()
        })
        .expect("test city has an unused district tile");
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(Name::new(district));
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
    position
}

#[cfg(test)]
pub(crate) fn vacate_land_combat_purchase_slot(game: &mut Game, player: usize, city: u32) {
    let center = game.cities[&city].pos;
    let blocker = game.unit_ids_at(center).iter().find(|unit| {
        let unit = &game.units[unit];
        let spec = &game.rules.units[unit.kind];
        unit.owner == player
            && spec.class == "military"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
    });
    if let Some(unit) = blocker {
        let destination =
            game.nbrs(center)
                .into_iter()
                .find(|position| {
                    game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    }) && game.unit_ids_at(*position).is_empty()
                })
                .expect("test city has an open adjacent land-combat slot");
        game.relocate(*unit, destination);
    }
}

pub mod adjacency;
pub mod quests;

mod actions;
mod city;
/// The World Congress: resolutions, emergencies, votes and favor. See
/// `game/congress.rs`.
mod congress;
mod defending_district;
pub use defending_district::DefendingDistrict;
#[cfg(test)]
pub(crate) mod defending_district_tests;
/// Espionage: spies, missions, promotions and counterspies. See
/// `game/espionage.rs`.
mod espionage;
/// Great People: points, offers, activation, patronage and effects. See
/// `game/great_people.rs`.
mod great_people;
mod growth;
mod policy_ledger;
mod research_quotes;
mod route_avoidance;

/// Amenities: luxuries, required and local amenities, war weariness. See
/// `game/amenities.rs`.
mod amenities;
/// Barbarians: camps, raids, scouts, and clearing a camp. See
/// `game/barbarians.rs`.
mod barbarians;
/// Battle board: the engine reads a local battle solver takes once per fight.
/// See `game/battle_board.rs`.
mod battle_board;
/// Boosts: eurekas and inspirations, their triggers and progress, and tree
/// completion. See `game/boosts.rs`.
mod boosts;
/// Borders: border growth, its culture cost, and culture bombs. See
/// `game/borders.rs`.
mod borders;
/// City capture: capturing, transferring, keeping, razing and liberating a
/// city, and the grievances each fate incurs. See `game/city_capture.rs`.
mod city_capture;
/// City-states: envoys, suzerainty, levies, and city-state bonuses. See
/// `game/city_states.rs`.
mod city_states;
/// Combat: attack range and reach, strength and bonuses, city strikes, XP
/// and promotions. See `game/combat.rs`.
mod combat;
mod combat_forecast;
pub use combat_forecast::CombatForecast;
/// Completion: finishing a city's item, wonder effects, and placing new
/// units. See `game/completion.rs`.
mod completion;
/// Disasters and climate: natural disasters and their damage, climate
/// change and sea level, reactor accidents. See `game/disasters.rs`.
mod disasters;
/// Embarkation: embarked units and their strength. See
/// `game/embarkation.rs`.
mod embarkation;
/// Eras: historic moments, era score, dedications and golden ages. See
/// `game/eras.rs`.
mod eras;
/// Goody huts: tribal villages, meteor sites, and their rewards. See
/// `game/goody_huts.rs`.
mod goody_huts;
/// Healing: heal rates, healing locations and Holy Waters. See
/// `game/healing.rs`.
mod healing;
/// Housing: a city's housing, its sources, and its effect on growth. See
/// `game/housing.rs`.
mod housing;
/// Loyalty: city loyalty, its pressures and effects, and Free Cities. See
/// `game/loyalty.rs`.
mod loyalty;
/// Modifiers: government and policy effects, and attached modifiers. See
/// `game/modifiers.rs`.
mod modifiers;
/// Monopolies and Corporations: resource counts, industries, corporations,
/// products and monopoly bonuses. See `game/monopolies.rs`.
mod monopolies;
/// The Moon: deposits, ores and mass drivers. See `game/moon.rs`.
mod moon;
/// Movement: step costs, movement points, traversal, entering and stopping,
/// and borders. See `game/movement.rs`.
mod movement;
/// Pathfinding: unit routes, route steps, movement relaxation, reach and
/// flow fields. See `game/pathfinding.rs`.
mod pathfinding;
/// Policy cards: government slots, fitting and pruning a deck, and the cards
/// available. See `game/policy_cards.rs`.
mod policy_cards;
/// City power: demand, supply, renewables, and powered yields. See
/// `game/power.rs`.
mod power;
/// Religion: pantheons, founding, beliefs, followers, theological combat.
/// See `game/religion.rs`.
mod religion;
pub use religion::ObservedCityFaith;
/// Strategic resources: connections, per-turn income, stockpiles. See
/// `game/strategic_resources.rs`.
mod strategic_resources;
/// Tourism and Great Works: tourism, tourists, slots and theming. See
/// `game/tourism.rs`.
mod tourism;
/// Trade routes: establishing, pricing, running and cancelling them. See
/// `game/trade_routes.rs`.
mod trade_routes;
/// The turn: beginning a turn, a city's turn and upkeep, modernizing unit
/// queues, and fallout damage. See `game/turn.rs`.
mod turn;
/// Unit upgrades: the upgrade graph, prices, host refusals, and upgrading.
/// See `game/unit_upgrades.rs`.
mod unit_upgrades;
/// Units: creating, removing and transferring units, and placing them off a
/// move. See `game/units.rs`.
mod units;
/// Victory: which victories are open, the races, the winner, and how a
/// game ends. See `game/victory.rs`.
mod victory;
/// Visibility: line of sight, unit and player vision, and fog memory. See
/// `game/visibility.rs`.
mod visibility;
/// Vision memo: the stamps and frames that let vision be reused. See
/// `game/vision_memo.rs`.
mod vision_memo;
/// The war log: war records, fronts, participants, losses and moments.
/// See `game/war_log.rs`.
mod war_log;
/// Zones of control for units and escorted formations. See
/// `game/zone_of_control.rs`.
mod zone_of_control;

pub use policy_ledger::{EmpireReading, PolicyLedger, PolicyWorth};
pub use research_quotes::HostResearchQuote;

#[cfg(test)]
mod area_damage_unit_removal_tests;

#[cfg(test)]
mod city_transfer_unit_removal_tests;

#[cfg(test)]
mod housing_source_tests;

#[cfg(test)]
mod city_state_unique_tests;

#[cfg(test)]
mod modifier_tests;

#[cfg(test)]
mod age_tests;

#[cfg(test)]
mod disaster_tests;

#[cfg(test)]
mod city_trade_tests;

#[cfg(test)]
mod great_person_runtime_tests;

#[cfg(test)]
mod mass_driver_tests;

#[cfg(test)]
mod religious_purchase_tests;

#[cfg(test)]
mod score_tests;

#[cfg(test)]
mod city_state_rule_tests;

#[cfg(test)]
mod gold_building_purchase_tests;

#[cfg(test)]
mod corporation_tests;

/// A natural wonder is permanent terrain in Civ VI. The generator draws a
/// fixed number of them per map size, and nothing a player does afterwards
/// removes one — so the count a map starts with is the count it ends with.
#[cfg(test)]
mod natural_wonder_permanence_tests;

#[cfg(test)]
mod national_park_tests;

#[cfg(test)]
mod belief_runtime_tests;

#[cfg(test)]
mod religion_belief_count_tests;

#[cfg(test)]
mod governor_runtime_tests;

#[cfg(test)]
mod action_family_tests;

#[cfg(test)]
mod arena_only_the_fighting_tests;

#[cfg(test)]
mod envoy_contact_tests;

#[cfg(test)]
mod agenda_cache_tests;

#[cfg(test)]
mod trade_deal_tests;

#[cfg(test)]
mod strategic_resource_tests;

#[cfg(test)]
mod project_runtime_tests;

#[cfg(test)]
mod espionage_runtime_tests;

#[cfg(test)]
mod specialist_tests;

#[cfg(test)]
mod power_tests;

#[cfg(test)]
mod maintenance_tests;

#[cfg(test)]
mod tourism_clamp_tests;

#[cfg(test)]
mod government_runtime_tests;

#[cfg(test)]
mod district_building_wonder_runtime_tests;

pub fn growth_threshold(pop: i32) -> f64 {
    15.0 + 8.0 * (pop - 1) as f64 + ((pop - 1) as f64).powf(1.5).trunc()
}

/// Gathering Storm victory thresholds on standard speed.
/// The seat a live mirror reconstructs. `mirror.rs` addresses it as a bare `0`
/// throughout — `plant_unit(&mut game, 0, …)`, `is_at_war(0, seat)`,
/// `player_unit_ids(0)` — and this names it where the engine has to agree.
pub const MIRRORED_SEAT: usize = 0;

pub const DIPLOMATIC_VICTORY_POINTS: i64 = 20;

/// The New Frontier game modes CIVVIS models. Each is a lobby checkbox that
/// is off in a stock Gathering Storm game and in every tournament lobby.
pub const GAME_MODES: [&str; 2] = ["apocalypse", "secret_societies"];

/// The Exoplanet Expedition's trip on Standard speed, in light-years:
/// `SCIENCE_VICTORY_POINTS_REQUIRED` in Expansion2_GlobalParameters.xml.
/// It is private because it is no game's total on its own: the game speed
/// scales it, and real Online games read 25. Read
/// [`Game::science_victory_points_needed`] instead.
const EXOPLANET_DESTINATION: f64 = 50.0;

/// A world the expedition can be sent to.
///
/// These are the real ones: every temperate planet inside fifty light-years
/// that is actually argued about as a target, at the distance the trip really
/// is, with the thing that is really wrong with it. There is a genuine
/// trade-off in the list and it is the trade-off the field has — the nearest
/// worlds are around flare stars and tidally locked, and the calmest star with
/// a temperate planet is nearly five times further away.
pub struct ExoplanetTarget {
    pub id: &'static str,
    pub name: &'static str,
    /// The trip, in light-years.
    pub light_years: f64,
    /// How habitable it is thought to be. This is what the expedition aims
    /// for, because the distance costs nothing (see [`Game::advance_exoplanet`]).
    pub grade: u8,
}

/// Ordered by distance, nearest first. **Append, never insert** — a saved
/// target is stored by id, but the survey draws from this list by index and
/// re-ordering it re-rolls which worlds a given seed reveals.
pub const EXOPLANET_TARGETS: [ExoplanetTarget; 10] = [
    ExoplanetTarget {
        id: "proxima_b",
        name: "Proxima Centauri b",
        light_years: 4.24,
        grade: 1,
    },
    ExoplanetTarget {
        id: "tau_ceti_e",
        name: "Tau Ceti e",
        light_years: 11.91,
        grade: 1,
    },
    ExoplanetTarget {
        id: "gj273_b",
        name: "Luyten's Star b",
        light_years: 12.35,
        grade: 2,
    },
    ExoplanetTarget {
        id: "teegarden_b",
        name: "Teegarden's Star b",
        light_years: 12.50,
        grade: 2,
    },
    ExoplanetTarget {
        id: "wolf1061_c",
        name: "Wolf 1061 c",
        light_years: 14.05,
        grade: 1,
    },
    ExoplanetTarget {
        id: "hd20794_d",
        name: "82 Eridani d",
        light_years: 19.71,
        grade: 2,
    },
    ExoplanetTarget {
        id: "gj667_cc",
        name: "Gliese 667 Cc",
        light_years: 23.62,
        grade: 2,
    },
    ExoplanetTarget {
        id: "gliese12_b",
        name: "Gliese 12 b",
        light_years: 39.70,
        grade: 1,
    },
    ExoplanetTarget {
        id: "trappist1_e",
        name: "TRAPPIST-1 e",
        light_years: 40.66,
        grade: 2,
    },
    ExoplanetTarget {
        id: "lhs1140_b",
        name: "LHS 1140 b",
        light_years: 48.93,
        grade: 3,
    },
];

/// Where the expedition goes if nothing has been surveyed at all: the world
/// anybody would already have heard of, at the far end of the list and very
/// nearly the fifty light-years this engine has always quoted.
pub const EXOPLANET_DEFAULT_TARGET: &str = "lhs1140_b";

/// How many of the roster a civilization has found, per thing it has put above
/// the air. Detection is not a matter of distance — TRAPPIST-1 is forty
/// light-years out and was found early because its star is small enough for a
/// planet to blot out a real fraction of it, while plenty of nearer worlds are
/// still argued over — so what a survey buys is *more of the list*, not the
/// near end of it.
///
/// This is the payoff the Moon landing and the Mars colony did not have. Both
/// were dead ends: the Moon paid a one-off Culture bonus and Mars paid nothing
/// at all, so a civilization racing the science victory correctly skipped
/// straight past both. Now they are the survey, and what they buy is which
/// world humanity actually reaches.
pub const EXOPLANET_SURVEY: [(&str, usize); 3] = [
    ("launch_earth_satellite", 3),
    ("launch_moon_landing", 2),
    ("launch_mars_colony", 2),
];

/// Every civilization's standing in every victory race, each as a percentage of
/// what that race requires. Produced by [`Game::victory_races`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VictoryRaces {
    pub science: f64,
    pub science_projects: usize,
    pub science_project_target: usize,
    /// Current progress of an active Exoplanet Expedition. On a mirrored
    /// Civilization VI board this is the host's Science Victory Points, which
    /// are the real distance shown by World Rankings rather than a simulation.
    pub exoplanet_distance: f64,
    /// Science Victory Points required on this game's speed: the host's own
    /// total on a mirrored board, otherwise fifty light-years scaled by the
    /// speed ([`Game::exoplanet_destination`]).
    pub exoplanet_distance_target: f64,
    /// The host-reported Science Victory Points gained per turn when available,
    /// including the current effects of Lagrange and Terrestrial Laser Stations.
    pub exoplanet_speed: f64,
    pub techs: usize,
    pub tech_total: usize,
    pub culture: f64,
    pub foreign_tourists: i64,
    pub culture_target: i64,
    pub civics: usize,
    pub civic_total: usize,
    pub domestic_tourists: i64,
    pub rival_domestic: i64,
    pub leading_domestic: i64,
    pub religious: f64,
    pub converted_civs: usize,
    pub religious_target: usize,
    /// Cities anywhere following this seat's religion, the religion lane's
    /// own number on the shipped overview ([`Game::cities_following_religion`]).
    pub cities_following_religion: usize,
    pub diplomatic: f64,
    pub diplomatic_points: i64,
    pub domination: f64,
    pub controlled_capitals: usize,
    pub capital_target: usize,
    pub score: f64,
    pub score_points: i64,
}
pub const TOURISM_PER_VISITOR: f64 = 200.0;

/// Faith for a pantheon at STANDARD speed. Read it through
/// `Game::pantheon_faith_cost`, never directly: the number that matters is this one
/// scaled by the game's speed, and using the constant raw is the defect that hid
/// here for the whole project.
pub const PANTHEON_FAITH_STANDARD: f64 = 25.0;

/// The pantheons that pay a Holy Site for the ground around it, and the plot
/// each one is paid for.
///
/// Every row is the same shipped modifier — `DISTRICT_HOLY_SITE`,
/// `YIELD_FAITH`, `Amount 1`, subject `CITY_FOLLOWS_PANTHEON_REQUIREMENTS` —
/// differing only in its `TerrainType`/`FeatureType`, which is why the engine
/// carries one predicate and three rows rather than three branches. The plot
/// name is matched against a terrain OR a feature so both shipped modifier
/// types (`MODIFIER_ALL_CITIES_TERRAIN_ADJACENCY` and
/// `MODIFIER_ALL_CITIES_FEATURE_ADJACENCY`) read the same way.
const PANTHEON_HOLY_SITE_ADJACENCY: [(&str, &str); 3] = [
    // DESERT_FOLKLORE_FAITHDESERT{,HILLS}ADJACENCY
    ("holy_site_desert_faith", "desert"),
    // DANCE_OF_THE_AURORA_FAITHTUNDRA{,HILLS}ADJACENCY
    ("holy_site_tundra_faith", "tundra"),
    // SACRED_PATH_FAITHFEATUREADJACENCY, FEATURE_JUNGLE
    ("holy_site_jungle_faith", "jungle"),
];

/// How far from a Holy Site a kill still pays God of War.
/// `PLOT_EIGHT_INCLUDE_HOLY_SITE`: `REQUIREMENT_PLOT_ADJACENT_DISTRICT_TYPE_MATCHES`
/// with `MinRange 0` and `MaxRange 8`, and the shipped text says the same —
/// "within 8 tiles of a Holy Site district". The requirement names no owner,
/// so any Holy Site on the map answers it.
const GOD_OF_WAR_HOLY_SITE_RANGE: i32 = 8;

const STANDARD_DEAL_TURNS: u32 = 30;
/// The one-off token costs published for individual diplomatic missions.
const DELEGATION_GOLD: f64 = 10.0;
const EMBASSY_GOLD: f64 = 25.0;
/// Civ VI's published grievance caps and one-off values. City occupation and
/// razing begin from a population-scaled share of the capture cap; the
/// casus-belli profile below applies the published proportional modifiers.
const FORMAL_WAR_GRIEVANCES: f64 = 100.0;
const CITY_CAPTURE_GRIEVANCES: f64 = 50.0;
const CITY_RAZE_GRIEVANCES: f64 = 150.0;
const CITY_STATE_SUZERAIN_WAR_GRIEVANCES: f64 = 100.0;
const CITY_STATE_ENVOY_WAR_GRIEVANCES: f64 = 50.0;
const CITY_STATE_CONQUEST_GRIEVANCES: f64 = 50.0;
const CITY_STATE_RAZE_GRIEVANCES: f64 = 100.0;
const REQUEST_REFUSAL_FIRST_GRIEVANCES: f64 = 25.0;
const REQUEST_REFUSAL_REPEAT_GRIEVANCES: f64 = 25.0;
const PROMISE_BROKEN_FIRST_GRIEVANCES: f64 = 100.0;
const PROMISE_BROKEN_REPEAT_GRIEVANCES: f64 = 25.0;
/// Gathering Storm's `WORLD_CONGRESS_REQUEST_FOR_MILITARY_AID_GRIEVANCES_MIN`.
const MILITARY_AID_REQUEST_GRIEVANCES_MIN: f64 = 200.0;

/// The grievance share a third party acquires from its relationship with the
/// directly aggrieved civilization.  These are intentionally applied once at
/// the event boundary rather than recursively through `add_grievances`.
const ALLIED_GRIEVANCE_SHARE: f64 = 0.50;
const FRIEND_GRIEVANCE_SHARE: f64 = 0.25;

/// One named declaration framework.  Values are expressed against the
/// ordinary Formal War declaration, capture, and raze bases above.  Razing
/// starts at three times the capture base, so the published 600% Liberation
/// penalty is a 2.0 multiplier here rather than a misleading 6.0.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CasusBelliProfile {
    id: &'static str,
    declaration_multiplier: f64,
    capture_multiplier: f64,
    raze_multiplier: f64,
}

impl CasusBelliProfile {
    fn declaration_grievances(self) -> f64 {
        FORMAL_WAR_GRIEVANCES * self.declaration_multiplier
    }
}

/// Every declaration form uses one source of truth for both admission and
/// city-fate accounting.  Aliases keep older saved action logs readable while
/// new observations always emit the explicit `_war` IDs.
/// Native unilateral declaration name, shared by permission checks and orders.
/// Joint wars need a partner/deal and are not unilateral native sessions.
pub fn native_war_statement(casus_belli: &str) -> Option<&'static str> {
    Some(match casus_belli_profile(casus_belli)?.id {
        "surprise_war" => "DECLARE_SURPRISE_WAR",
        "formal_war" => "DECLARE_FORMAL_WAR",
        "holy_war" => "DECLARE_HOLY_WAR",
        "liberation_war" => "DECLARE_LIBERATION_WAR",
        "reconquest_war" => "DECLARE_RECONQUEST_WAR",
        "protectorate_war" => "DECLARE_PROTECTORATE_WAR",
        "colonial_war" => "DECLARE_COLONIAL_WAR",
        "territorial_war" => "DECLARE_TERRITORIAL_WAR",
        "golden_age_war" => "DECLARE_GOLDEN_AGE_WAR",
        "retribution_war" => "DECLARE_WAR_OF_RETRIBUTION",
        "ideological_war" => "DECLARE_IDEOLOGICAL_WAR",
        _ => return None,
    })
}

fn casus_belli_profile(id: &str) -> Option<CasusBelliProfile> {
    let profile = match id {
        "surprise_war" | "surprise" => CasusBelliProfile {
            id: "surprise_war",
            declaration_multiplier: 1.5,
            capture_multiplier: 1.5,
            raze_multiplier: 1.5,
        },
        "formal_war" | "formal" => CasusBelliProfile {
            id: "formal_war",
            declaration_multiplier: 1.0,
            capture_multiplier: 1.0,
            raze_multiplier: 1.0,
        },
        "holy_war" | "holy" => CasusBelliProfile {
            id: "holy_war",
            declaration_multiplier: 0.5,
            capture_multiplier: 0.5,
            raze_multiplier: 0.5,
        },
        "joint_war" | "joint" => CasusBelliProfile {
            id: "joint_war",
            declaration_multiplier: 1.0,
            capture_multiplier: 1.0,
            raze_multiplier: 1.0,
        },
        "reconquest_war" | "reconquest" => CasusBelliProfile {
            id: "reconquest_war",
            declaration_multiplier: 0.0,
            capture_multiplier: 1.0,
            raze_multiplier: 1.0,
        },
        "protectorate_war" | "protectorate" => CasusBelliProfile {
            id: "protectorate_war",
            declaration_multiplier: 0.0,
            capture_multiplier: 1.0,
            raze_multiplier: 1.0,
        },
        "liberation_war" | "liberation" => CasusBelliProfile {
            id: "liberation_war",
            declaration_multiplier: 0.0,
            capture_multiplier: 1.0,
            raze_multiplier: 2.0,
        },
        "colonial_war" | "colonial" => CasusBelliProfile {
            id: "colonial_war",
            declaration_multiplier: 0.5,
            capture_multiplier: 0.5,
            raze_multiplier: 1.0,
        },
        "territorial_war" | "territorial" => CasusBelliProfile {
            id: "territorial_war",
            declaration_multiplier: 0.75,
            capture_multiplier: 0.75,
            raze_multiplier: 0.5,
        },
        "golden_age_war" | "golden_age" => CasusBelliProfile {
            id: "golden_age_war",
            declaration_multiplier: 0.25,
            capture_multiplier: 0.25,
            raze_multiplier: 1.0,
        },
        "retribution_war" | "retribution" => CasusBelliProfile {
            id: "retribution_war",
            declaration_multiplier: 0.5,
            capture_multiplier: 0.5,
            raze_multiplier: 2.0 / 3.0,
        },
        "ideological_war" | "ideological" => CasusBelliProfile {
            id: "ideological_war",
            declaration_multiplier: 0.5,
            capture_multiplier: 0.5,
            raze_multiplier: 0.5,
        },
        _ => return None,
    };
    Some(profile)
}
/// Shipped `GOVERNMENT_BASE_ANARCHY_TURNS`: returning to a government the
/// civilization has already run costs this many turns of Anarchy.
pub const GOVERNMENT_BASE_ANARCHY_TURNS: u32 = 2;
/// Shipped `DIPLOMACY_PEACE_MIN_TURNS`: a peace treaty holds for ten turns on
/// standard speed before either signatory may declare war again.
const PEACE_TREATY_TURNS: u32 = 10;

/// Extra damage a defender takes when a `force_retreat` attacker beats it and
/// it has nowhere to withdraw to. Civilization VI's Winged Hussar deals its
/// retreat damage in place of the move; cornering the loser is the punishment,
/// not a reprieve.
const FORCED_RETREAT_BLOCKED_DAMAGE: i32 = 10;
/// Shipped `Eras_XP1.GameEraMinimumTurns`: every era is held open this many
/// standard turns before the next one may begin, whatever the field has
/// researched. It is the same 40 for every era in the table.
const ERA_MINIMUM_TURNS: u32 = 40;
/// Shipped `Eras_XP1.GameEraMaximumTurns`: every standard-speed world era
/// closes by its sixtieth turn.
const ERA_MAXIMUM_TURNS: u32 = 60;
/// Shipped `NEXT_ERA_TURN_COUNTDOWN` warning after the field median advances.
const NEXT_ERA_COUNTDOWN_TURNS: u32 = 10;
/// Shipped `DIPLOMACY_WAR_MIN_TURNS`: a war runs ten turns before either side
/// may sue for peace. Declaring is a commitment, not a gesture.
const WAR_MIN_TURNS: u32 = 10;
/// Shipped `BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY`: no camp is seated within
/// four tiles of a city, anybody's.
const BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY: i32 = 4;
/// Shipped `BARBARIAN_CAMP_MINIMUM_DISTANCE_ANOTHER_CAMP`: camps stand seven
/// tiles apart, so a region that already has one is done.
const BARBARIAN_CAMP_MINIMUM_DISTANCE_ANOTHER_CAMP: i32 = 7;
/// Shipped `BARBARIAN_CAMP_ODDS_OF_NEW_CAMP_SPAWNING`: every turn the world is
/// under its camp target it rolls one in this many to seat one more. It is a
/// per-turn roll, not a schedule — the first camps a civilization has not seen
/// yet appear over the opening handful of turns, not once a decade.
const BARBARIAN_CAMP_ODDS_OF_NEW_CAMP_SPAWNING: usize = 2;
/// Shipped `BARBARIAN_CAMP_COASTAL_SPAWN_ROLL`: one in six eligible coastal
/// camps is a naval outpost. The classification persists for the camp's life.
const BARBARIAN_CAMP_COASTAL_SPAWN_ROLL: usize = 6;
/// A Scout that successfully returns home opens a finite raid window. During
/// it the outpost raises the difficulty-shaped party it was told to send;
/// after it expires the Scout goes back to looking for a new target.
const BARBARIAN_SCOUT_ALERT_TURNS: u32 = 15;

pub fn effective_strength(base: f64, hp: i32) -> f64 {
    let wounded_penalty = (10.0 - hp.clamp(0, 100) as f64 / 10.0).round();
    (base - wounded_penalty).max(0.0)
}

pub fn damage(att: f64, def: f64, rng: &mut Rng) -> i32 {
    let d = 30.0 * ((att - def) / 25.0).exp() * rng.uniform(0.8, 1.2);
    (d.round() as i32).clamp(1, 100)
}

/// [`damage`] with its uniform factor taken at its mean: the blow a
/// controller should expect without consuming the game RNG or rounding to a
/// particular roll. Stated here, beside the roll it is the centre of, so the
/// two cannot drift.
pub fn expected_damage(att: f64, def: f64) -> f64 {
    (30.0 * ((att - def) / 25.0).exp()).clamp(1.0, 100.0)
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct GreatWorkPiece {
    pub kind: String,
    /// The era the work was created in (for artifacts, the era dug up).
    pub era: usize,
    /// The creating Great Person, or a source tag for relics and artifacts.
    pub creator: String,
}

/// The Great Person a kill by this unit feeds under Wolin, by the class lists
/// of `ABILITY_WOLIN_LAND_UNITS` and `ABILITY_WOLIN_NAVAL_UNITS`.
///
/// The killer decides; the victim's domain does not enter into it, so a
/// Crossbowman that sinks a Galley earns General points. Air units, carriers
/// and the Giant Death Robot (`CLASS_GIANT_DEATH_ROBOT` alone) are on neither
/// list, and CIVVIS files the Robot under `melee`, so it is named here.
fn wolin_great_person_class(kind: &str, spec: &crate::rules::UnitSpec) -> Option<&'static str> {
    if kind == "giant_death_robot" {
        return None;
    }
    match spec.promotion_class.as_str() {
        "melee" | "ranged" | "anti_cavalry" | "heavy_cavalry" | "light_cavalry" | "recon"
        | "siege" | "warrior_monk" | "nihang" => Some("general"),
        "naval_melee" | "naval_raider" | "naval_ranged" => Some("admiral"),
        _ => None,
    }
}

fn pair(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn cube_round(q: f64, r: f64, s: f64) -> Pos {
    let mut rq = q.round();
    let mut rr = r.round();
    let rs = s.round();
    let q_delta = (rq - q).abs();
    let r_delta = (rr - r).abs();
    let s_delta = (rs - s).abs();
    if q_delta > r_delta && q_delta > s_delta {
        rq = -rr - rs;
    } else if r_delta > s_delta {
        rr = -rq - rs;
    }
    (rq as i32, rr as i32)
}

impl Game {
    /// Stock setup profile governing this world. Exact stock dimensions win;
    /// custom maps fall back to the profile for their major-player count.
    pub fn map_size(&self) -> &'static MapSize {
        MapSize::from_dimensions(self.map.width, self.map.height).unwrap_or_else(|| {
            let majors = self
                .players
                .iter()
                .filter(|player| !player.is_minor && !player.is_barbarian)
                .count();
            MapSize::for_players(majors)
        })
    }

    pub fn max_religions(&self) -> usize {
        self.map_size().max_religions
    }

    /// Whether the lobby switched on one of the New Frontier game modes.
    /// Nothing outside a mode's own rules may consult this: a mode is a
    /// pre-game setting, not a mid-game state.
    pub fn game_mode(&self, mode: &str) -> bool {
        self.game_modes.contains(mode)
    }

    /// Replace native observations without modifying the shared constructor
    /// rules or retaining knowledge from a newer export after a rewind.
    pub(crate) fn set_host_research_prerequisites(
        &mut self,
        observations: BTreeMap<Name, Vec<Name>>,
    ) -> Result<(), String> {
        if let Some(base) = self.host_research_base_rules.take() {
            self.rules = base;
        }
        self.host_revealed_research = Arc::new(BTreeSet::new());
        self.host_research_eligibility = Arc::new(BTreeMap::new());
        let revealed = observations.keys().copied().collect();
        let mut changes = BTreeMap::new();
        for (node, mut parents) in observations {
            let Some(spec) = self.rules.techs.get(&node) else {
                return Err(format!("unknown node {node}"));
            };
            if parents
                .iter()
                .any(|parent| !self.rules.techs.contains_key(parent))
            {
                return Err(format!("unknown prerequisite for {node}"));
            }
            parents.sort();
            parents.dedup();
            let mut generated = spec.requires.clone();
            generated.sort();
            generated.dedup();
            if parents != generated {
                changes.insert(node, parents);
            }
        }
        if changes.is_empty() {
            self.host_revealed_research = Arc::new(revealed);
            return Ok(());
        }
        fn visit(
            node: Name,
            rules: &Rules,
            changes: &BTreeMap<Name, Vec<Name>>,
            marks: &mut BTreeMap<Name, u8>,
        ) -> bool {
            match marks.get(&node) {
                Some(1) => return false,
                Some(2) => return true,
                _ => {}
            }
            marks.insert(node, 1);
            let parents = changes.get(&node).unwrap_or(&rules.techs[node].requires);
            if parents
                .iter()
                .any(|parent| !visit(*parent, rules, changes, marks))
            {
                return false;
            }
            marks.insert(node, 2);
            true
        }
        let mut marks = BTreeMap::new();
        if self
            .rules
            .techs
            .keys()
            .any(|node| !visit(*node, &self.rules, &changes, &mut marks))
        {
            return Err("cyclic prerequisites".into());
        }
        let base = self.rules.clone();
        let mut observed = (*base).clone();
        for (node, parents) in changes {
            observed.techs.get_mut(&node).unwrap().requires = parents;
        }
        // AdvancedAi routes research through this compiled transitive index,
        // not the direct lists alone. Keep every descendant's index coherent.
        observed.tech_ancestors = crate::rules::ancestry(&observed.techs);
        self.host_research_base_rules = Some(base);
        self.host_revealed_research = Arc::new(revealed);
        self.rules = Arc::new(observed);
        Ok(())
    }

    fn host_research_prerequisites(&self) -> BTreeMap<Name, Vec<Name>> {
        let Some(base) = &self.host_research_base_rules else {
            return BTreeMap::new();
        };
        self.rules
            .techs
            .iter()
            .filter(|(node, spec)| spec.requires != base.techs[*node].requires)
            .map(|(node, spec)| (*node, spec.requires.clone()))
            .collect()
    }
    pub fn tech_cost(&self, technology: &str) -> f64 {
        self.game_speed.scale(self.rules.techs[technology].cost)
    }

    pub fn civic_cost(&self, civic: &str) -> f64 {
        self.game_speed.scale(self.rules.civics[civic].cost)
    }

    pub fn growth_cost(&self, population: i32) -> f64 {
        self.game_speed.scale(growth_threshold(population))
    }

    /// Faith needed to found a pantheon, scaled by game speed like every other cost.
    ///
    /// ⚠⚠⚠ THIS WAS A BARE `25.0` IN TWO PLACES, AND IT COST CIVVIS THE DECISION
    /// ENTIRELY IN LIVE GAMES. Every neighbouring cost above already scales — techs,
    /// civics, growth — and this one did not, so on any speed but Standard CIVVIS
    /// was asking for a different game's price.
    ///
    /// Measured on live run `civvis-20260803T191900Z` (Online speed, Rome/Trajan):
    /// Civilization VI raised `ENDTURN_BLOCKING_PANTHEON` on **turn 19 at faith 11**,
    /// and faith went 11 -> 0 when the pantheon was taken. CIVVIS's gate wanted 25,
    /// so `ChoosePantheon` was never a legal action, `civvis_orders` emitted nothing
    /// for turns 18/19/20 on replay, and the mod's fallback answered instead — by
    /// walking `GameInfo.Beliefs()` and taking the FIRST UNTAKEN ROW. The empire's
    /// one permanent pantheon was chosen by database order, in every live game.
    ///
    /// `Online.scale(25.0)` is 12.5, which is the price the host was charging.
    /// Standard is unchanged at 25.0, so self-play at the default speed is
    /// untouched; only the speeds that were already wrong move.
    pub fn pantheon_faith_cost(&self) -> f64 {
        self.game_speed.scale(PANTHEON_FAITH_STANDARD)
    }

    pub fn standard_duration(&self, turns: u32) -> u32 {
        self.game_speed.scale_turns(turns)
    }

    /// Distance along the world's own shape: wrapped hex distance on the
    /// cylindrical scripts, steps around the globe on Planet.
    pub fn wdist(&self, a: Pos, b: Pos) -> i32 {
        self.map.distance(a, b)
    }

    /// [`Self::wdist`] that stops measuring at `cap`: the true distance up to
    /// it and `cap + 1` beyond. For a rule that only compares against `cap`.
    #[inline]
    pub fn wdist_capped(&self, a: Pos, b: Pos, cap: i32) -> i32 {
        self.map.distance_capped(a, b, cap)
    }

    /// In-map neighbors, across the wrap seam or around the globe.
    #[inline]
    pub fn nbrs(&self, p: Pos) -> hex::Neighbors {
        self.map.neighbors(p)
    }

    /// Every in-map tile within `r` steps of `c`.
    pub fn wdisk(&self, c: Pos, r: i32) -> Vec<Pos> {
        self.map.disk(c, r)
    }

    /// The world's tiles exactly `r` steps out. See [`WorldMap::ring`].
    pub fn wring(&self, c: Pos, r: i32) -> Vec<Pos> {
        self.map.ring(c, r)
    }
}

/// Terrain heights for one visibility sweep, indexed by tile.
///
/// Working out how tall a hex stands means asking whether it is a mountain,
/// whether its feature is a Natural Wonder, and whether a City Center or
/// Encampment looks out from it. Every ray that crosses a hex asks the same
/// question, and a player's sweep runs a ray from every unit to every tile in
/// sight, so the same handful of answers were being recomputed thousands of
/// times a turn. They are pure functions of the map, so the sweep keeps them.
///
/// An empty field memoizes nothing, which is what a one-off ray — a combat
/// line of sight, say — wants: filling a per-tile table costs more than the
/// two lookups it would save.
struct HeightField {
    /// The state of the world this sweep is reading. Worked out once when
    /// the sweep starts rather than per unit.
    stamp: u64,
    /// How many tiles a table would need, or zero for a sweep that memoizes
    /// nothing. The tables themselves are only built if something asks for a
    /// height — a sweep whose every unit is already cached never does.
    tiles: usize,
    sight: Vec<i16>,
    /// Blocker heights for a viewer who sees through woods, and for one who
    /// does not. Woods are the only thing the two disagree about.
    open: Vec<i16>,
    wooded: Vec<i16>,
}

/// One immutable answer to the question "what can this seat see right now?".
///
/// The expensive part of a visibility refresh is the terrain-aware ray walk,
/// not publishing the answer into fog memory.  The sight inputs are stamped
/// independently of the world turn, so a seat whose units, cities, diplomacy,
/// or host-provided sight did not change can reuse this compact bit frame even
/// when the turn counter (and therefore remembered-tile stamps) advances.
#[derive(Clone)]
struct VisionFrame {
    input_stamp: u64,
    visible: Arc<TileBits>,
}

/// The folded suzerain-of-every-minor map, beside the diplomacy epoch (see
/// `Game::diplomacy_epoch`) it was built from.
type SuzerainMapCache = std::cell::RefCell<Option<(u64, BTreeMap<usize, Option<usize>>)>>;
/// Everything `suzerain_of` reads: the players' envoys and governors, the
/// cities their governors sit in, and the turn their establishment counts
/// from. Players and cities bump their counters at every mutable access.
type SuzerainStamp = (u64, u64, u32);
type SuzerainStable = std::cell::RefCell<Option<(SuzerainStamp, BTreeMap<usize, Option<usize>>)>>;
/// Per-pid shared-vision viewer sets, beside the diplomacy epoch they were
/// built from. Indexed by pid; a pid nobody has asked about this epoch keeps
/// its slot `None` rather than being folded speculatively.
type ViewersCache = std::cell::RefCell<Option<(u64, Vec<Option<BTreeSet<usize>>>)>>;

/// Inputs whose changes can make a cached final visibility signature stale.
/// They are intentionally broader than that signature itself: an exposed
/// mutable unit or city is cheap to recognize here, while the slower exact
/// fold still decides whether the final `TileBits` frame can be reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VisionInputMemoKey {
    geometry: u64,
    units: u64,
    diplomacy: u64,
    spies: u64,
}

/// Per-pid final visibility-input signatures, beside the conservative inputs
/// that produced them. The final signature is what validates a `VisionFrame`;
/// this cache only avoids rebuilding it on an unchanged world.
type InputStampCache = std::cell::RefCell<Option<(VisionInputMemoKey, Vec<Option<u64>>)>>;

/// Runtime-only per-seat visibility frames.  A cloned game inherits its
/// parent's, because each frame carries the stamp of every input the
/// derivation reads: a branch that moves its sight sources before its first
/// read restamps and recomputes, and one that does not would have derived the
/// same bitset.  The cache is deliberately separate from [`VisionCache`],
/// whose entries are per-unit ray answers and are merged from worker worlds.
#[derive(Default)]
struct VisionFrameCache {
    frames: std::cell::RefCell<Vec<Option<VisionFrame>>>,
    /// Frame hits used to rebuild their full per-viewer signature before they
    /// could discover that the `TileBits` was reusable. Keep that signature
    /// per seat behind broad, mutation-safe inputs instead.
    input_stamps: InputStampCache,
    /// Map writes also cover roads, improvements, and ownership, none of
    /// which alter a sight ray.  Re-fold the small sight-relevant tile view
    /// once per map epoch so those unrelated writes do not evict every seat's
    /// frame.
    map_geometry: std::cell::RefCell<Option<(u64, u64)>>,
    /// One full roster walk yields every viewer's unit-source stamp.  Frame
    /// lookups are far more frequent than unit writes, so retain that fan-out
    /// until a mutable unit access can have changed one of its inputs.
    unit_stamps: std::cell::RefCell<Option<(u64, Vec<u64>)>>,
    /// Every minor's current suzerain, folded once per diplomacy epoch
    /// instead of once per single-seat vision ask.  `suzerain_of` is cheap
    /// once the relationship is known, but `suzerain_input_map` used to
    /// allocate this `BTreeMap` fresh on every `player_vision_frame` call.
    suzerain_map: SuzerainMapCache,
    /// Per-pid shared-vision viewer sets — team, military-alliance, and live
    /// emergency partners — folded lazily per pid the first time a
    /// diplomacy epoch asks for it. `visibility_viewers` used to allocate a
    /// fresh `BTreeSet` on every ask even when nothing diplomatic had moved.
    viewers: ViewersCache,
}

impl Clone for VisionFrameCache {
    /// A branch inherits its parent's sight frames rather than starting cold.
    ///
    /// This is exact by construction, and the reason is the stamp: a frame is
    /// reused only when `vision_input_stamp_with_suzerains` still matches, and
    /// that stamp folds every input `player_vision` reads — map geometry, the
    /// viewer set, and each viewer's units, cities, suzerained city-states and
    /// spies. A branch that moves a sight source restamps and recomputes; one
    /// that does not would have derived the bitset it inherited. The fields
    /// `speculative_clone` changes — `track_fog_memory`, `track_war_ledger`,
    /// `visibility_suppressed`, `visibility_batch` — are read by neither the
    /// stamp nor the derivation.
    ///
    /// ⚠ Returning `Self::default()` here, as this did, made every speculative
    /// clone pay a cold cache. The tactical picker scores a candidate by
    /// cloning and applying, and `do_ranged`/`do_attack`/`do_city_strike` each
    /// ask `combat_target_visible` — so one full ray-cast per candidate, on a
    /// world discarded immediately afterwards.
    fn clone(&self) -> Self {
        Self {
            frames: std::cell::RefCell::new(self.frames.borrow().clone()),
            input_stamps: std::cell::RefCell::new(self.input_stamps.borrow().clone()),
            map_geometry: std::cell::RefCell::new(*self.map_geometry.borrow()),
            unit_stamps: std::cell::RefCell::new(self.unit_stamps.borrow().clone()),
            suzerain_map: std::cell::RefCell::new(self.suzerain_map.borrow().clone()),
            viewers: std::cell::RefCell::new(self.viewers.borrow().clone()),
        }
    }
}

impl HeightField {
    const UNKNOWN: i16 = i16::MIN;

    fn none() -> HeightField {
        HeightField {
            stamp: 0,
            tiles: 0,
            sight: Vec::new(),
            open: Vec::new(),
            wooded: Vec::new(),
        }
    }

    fn for_map(stamp: u64, tiles: usize) -> HeightField {
        HeightField {
            stamp,
            tiles,
            sight: Vec::new(),
            open: Vec::new(),
            wooded: Vec::new(),
        }
    }

    /// The table for one kind of height, built on first use.
    fn table(tiles: usize, cache: &mut Vec<i16>) -> Option<&mut Vec<i16>> {
        if tiles == 0 {
            return None;
        }
        if cache.is_empty() {
            cache.resize(tiles, HeightField::UNKNOWN);
        }
        Some(cache)
    }
}

/// Mix a few numbers into one, for cache keys that must change whenever any
/// of their parts does.
fn vision_key(parts: &[u64]) -> u64 {
    const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut key = SEED;
    for part in parts {
        key = (key ^ part).wrapping_mul(SEED);
        key ^= key >> 29;
    }
    key
}

/// The immutable entries of a [`VisionCache`].
///
/// A speculative branch normally changes one actor and then asks what the
/// whole seat can see.  The other units' rays remain valid, so keeping their
/// dense bitsets behind an `Arc` turns cloning the cache into a refcount bump;
/// the first miss in a branch clones only these small index vectors, not every
/// ray's bitset.  Entries themselves are immutable after installation.
#[derive(Clone, Default)]
struct VisionCacheEntries {
    units: Vec<u32>,
    keys: Vec<u64>,
    seen: Vec<Arc<TileBits>>,
}

/// What each unit could see, the last time anybody asked.
///
/// The cache key validates both the world's geometry (`stamp`) and the
/// individual sight source (`key`).  A copied cache is therefore safe in a
/// search branch: a moved source misses its own entry, while every unchanged
/// source reuses the ray it would derive again.  Entries stay in unit order so
/// the authoritative world can merge worker answers without walking a tree.
#[derive(Default)]
pub struct VisionCache {
    stamp: u64,
    entries: Arc<VisionCacheEntries>,
    /// Which wonders exist anywhere in the world. Deciding whether a city may
    /// start one asks this of every wonder in the ruleset, and answering it
    /// from scratch meant walking every city and every tile each time. This is
    /// deliberately branch-local: it is production state, not a unit ray.
    built_wonders: Option<BTreeSet<Name>>,
}

impl Clone for VisionCache {
    /// A visibility answer is immutable and lookup validates its exact map
    /// stamp plus the source's position, sight and elevation key.  A branch
    /// can therefore share the parent table until its first newly-derived ray
    /// needs copy-on-write storage; clearing it here made tactical clones
    /// re-cast every stationary unit's sight ray after one actor moved.
    fn clone(&self) -> Self {
        Self {
            stamp: self.stamp,
            entries: Arc::clone(&self.entries),
            built_wonders: None,
        }
    }
}

/// The player-independent half of a monopoly answer.
///
/// This owns interned resource names so a context can be retained across the
/// many ordinary unit actions in one turn. The context is derived state, not
/// game state: `QueryCache` drops it before a mutation that can change a
/// resource's world count, connected ownership, or city-state patronage.
#[derive(Clone)]
struct MonopolyContext {
    world_counts: BTreeMap<Name, i32>,
    /// Each player's suzerained city-states, whose holdings count as theirs.
    minors: Vec<Vec<usize>>,
    /// Each player's connected holdings, by resource.
    connected: Vec<BTreeMap<Name, i32>>,
}

/// Memoized answers to expensive read-only queries.
///
/// Most entries live only while a [`QueryMemo`] guard is held. The production
/// catalog and monopoly census are deliberately retained between those short
/// read-only scopes and are invalidated at their respective mutation
/// boundaries.
///
/// For the guard-scoped entries, the guard borrows the game immutably, so the
/// borrow checker — not a hand-written stamp over the citizen plan, worked
/// tiles, districts, buildings, techs, policies, religion and wonders — is
/// what guarantees the cache cannot go stale. Nothing can reach a `&mut Game`
/// while one of those entries is live.
type GreatWorksByCity = BTreeMap<u32, BTreeMap<String, usize>>;
const GREAT_WORK_COUNTERS: [(&str, &str); 6] = [
    ("writing", "great_work:writing"),
    ("art", "great_work:art"),
    ("religious_art", "great_work:religious_art"),
    ("artifact", "great_work:artifact"),
    ("music", "great_work:music"),
    ("relic", "great_work:relic"),
];
type HousedWorksByPlayer = BTreeMap<usize, Arc<GreatWorksByCity>>;
type GreatWorkPiecesByCity = BTreeMap<u32, Vec<GreatWorkPiece>>;
type HousedPiecesByPlayer = BTreeMap<usize, Arc<GreatWorkPiecesByCity>>;
type GreatWorkSlotsByPlayer = BTreeMap<usize, Vec<(u32, String)>>;
type GreatWorkHousing = BTreeMap<(usize, String, usize), bool>;
type WonderEffectsByPlayer = BTreeMap<usize, BTreeMap<String, f64>>;
type NationalParksByPlayer = BTreeMap<usize, Vec<(u32, [Pos; 4])>>;
type RegionalSlotsByPlayer = BTreeMap<usize, Arc<Vec<RegionalSlot>>>;

/// One active regional building of an empire and the ground it reaches, in
/// the empire's city then building order. What it sends is still read per
/// receiving city, from the ruleset and the source's Power.
#[derive(Clone, Copy, Debug)]
struct RegionalSlot {
    source: u32,
    building: Name,
    origin: Pos,
    range: i32,
}

/// Memoized `unit_purchase_cost_for_formation` answers for one `QueryMemo`
/// guard: keyed on (player, city, unit kind, formation, gold?) → price.
/// Only keyed lookups read this cache; action order comes from the ruleset
/// sweep. Hashing the interned name avoids ordering it by ruleset text.
type PurchasePriceMemo = HashMap<(usize, u32, Name, u8, bool), Option<f64>>;

#[derive(Default)]
pub struct QueryCache {
    yields: std::cell::RefCell<Option<BTreeMap<u32, Yields>>>,
    // Appeal is read far more often than anything else in this cache: the
    // settle and district planners sweep whole regions of the map, and every
    // sweep asks the same tiles again. Measured over a 150-turn six-player
    // game, `tile_appeal` was called 1,240,472 times and 97.1% of those calls
    // recomputed an answer the engine had already produced for that tile that
    // turn -- 34 evaluations per tile per turn, of which one was new.
    appeal: std::cell::RefCell<Option<BTreeMap<Pos, i32>>>,
    /// `modeled_tile_yields` per plot.
    ///
    /// ★★★★ THE SAME PLOT PRICED TWICE PER CITY, EVERY TIME. One city-yield
    /// derivation asks every owned plot for its yields to plan the citizens,
    /// then asks the worked plots again to sum them; the 2026-09-26 profile
    /// put `modeled_tile_yields` at 7.6% of a ladder game's CPU, 2.2% of it
    /// that second ask. A plot's modeled yields read the map, its owner's
    /// techs, civics and policies and its neighbours — nothing a `&self`
    /// query can change — so the answer is kept for the memo scope, exactly
    /// like `appeal`.
    tile_yields: std::cell::RefCell<Option<BTreeMap<Pos, Yields>>>,
    /// `district_yields` per standing district. One city-yield derivation
    /// asks each of its districts three times — the citizen strategy, the
    /// citizen plan's fixed food, then the sum — and a district's yields
    /// read only the board, so the answer is kept for the memo scope like
    /// `tile_yields`.
    district_yields: std::cell::RefCell<Option<BTreeMap<(Name, Pos), Yields>>>,
    traversal: std::cell::RefCell<Option<BTreeMap<u32, TraversalClass>>>,
    /// Every unit currently flying a patrol, as `(owner, tile)`.
    ///
    /// ★★★★ A WHOLE-BOARD FACT ASKED ONCE PER NEIGHBOUR. `can_enter_past`
    /// walked `units` in full to find out whether an enemy fighter was
    /// patrolling the tile being stepped onto — and `flow_past` asks it for
    /// every neighbour of every tile it expands, so a single unit's movement
    /// flood scanned the world hundreds of times. The answer cannot change
    /// inside a `&self` query, so it is computed once per memo scope. Almost
    /// always empty: a patrol needs a fighter, and Flight is deep enough
    /// that most games never see one.
    air_patrols: std::cell::RefCell<Option<AirPatrols>>,
    /// Which improvements let a unit pass, indexed by [`Name::id`].
    ///
    /// ★★★★ A RULESET FACT ASKED PER TILE, BY STRING. `class_can_traverse`
    /// read `improvements[name].effects["passage"]` — a `BTreeMap<String,
    /// f64>` descent, so `memcmp` per level — for every tile of every
    /// movement flood. The rules do not change inside a game, let alone
    /// inside a query, so the answer is a table built once per memo scope
    /// and read with one array index.
    passage_improvements: std::cell::RefCell<Option<PassageTable>>,
    /// What each unit's movement costs before the tile is even named.
    ///
    /// ★★★★ A UNIT-WIDE FACT ASKED PER NEIGHBOUR, BY STRING. `flow_past`
    /// reaches `unit_step_cost`, `unit_max_moves_at` and
    /// `formation_enters_enemy_zoc` once for every neighbour of every tile it
    /// expands, and each of those descended a `BTreeMap<String, f64>` — one
    /// `memcmp` per level — for `woods_move_cost`, `hills_move_cost`,
    /// `amphibious`, `movement` and the rest, plus a seven-tile occupancy
    /// sweep for the support aura. None of it depends on the tile, so a flood
    /// over three hundred tiles asked the same questions three hundred times.
    /// The answers are computed once per unit per memo scope instead.
    movement: std::cell::RefCell<Option<BTreeMap<u32, MovementProfile>>>,
    amenities: std::cell::RefCell<Option<BTreeMap<u32, i64>>>,
    // `legal_purchase_actions_for_city` asks `unit_purchase_cost_for_formation`
    // for every unit kind in the ruleset, at all three formations and both
    // currencies, and `legal_actions_within`'s purchase family repeats the
    // same sweep, none of the asks sharing an answer. The price reads live
    // unit occupancy (`land_combat_purchase_slot_open`) and the production
    // queue head, both of which a caller can change without going through
    // `Game::apply` — `relocate` and a direct queue edit are `pub(crate)`,
    // and `land_combat_purchase_requires_an_unreserved_city_center_combat_layer`
    // exercises exactly that from a test. So this is scoped like `appeal` and
    // `movement`, not like `producible`: cached only for the life of one
    // `QueryMemo` guard, which is also the only span the codebase already
    // guarantees nothing mutates the world out from under a `&self` query.
    purchase_price: std::cell::RefCell<Option<PurchasePriceMemo>>,
    // Ownership-filtered ids are requested throughout AI evaluation. A
    // 100-seat game otherwise rescans every world entity for each request,
    // even when a read-only phase asks for the same empire repeatedly.
    unit_ids: std::cell::RefCell<Option<BTreeMap<usize, Arc<Vec<u32>>>>>,
    // Territory access is a unit-level route predicate. Cache it by unit id
    // while a route call runs, because the same access gates both local and
    // cached planning steps.
    unit_territory_access: std::cell::RefCell<Option<BTreeMap<u32, Arc<Vec<bool>>>>>,
    city_ids: std::cell::RefCell<Option<BTreeMap<usize, Vec<u32>>>>,
    // Four empire-wide derivations that callers reach for one city at a
    // time. Each is keyed by player, because that is the scope it is
    // computed over, not the scope it is read at.
    lux_alloc: std::cell::RefCell<Option<BTreeMap<usize, BTreeMap<u32, i64>>>>,
    lux_names: std::cell::RefCell<Option<BTreeMap<usize, BTreeSet<Name>>>>,
    housed_works: std::cell::RefCell<Option<HousedWorksByPlayer>>,
    housed_pieces: std::cell::RefCell<Option<HousedPiecesByPlayer>>,
    // A city-state's patron, which is a poll of every major's effective envoy
    // count. Deciding whether a step crosses a hostile border asks it, so a
    // route search asks it of the same city-state at every tile it considers.
    suzerain: std::cell::RefCell<Option<BTreeMap<usize, Option<usize>>>>,
    // Every Great Work slot the empire owns, and whether one more work of a
    // kind would find a home. Deciding what a Builder or an Archaeologist may
    // do on a tile asks the second of these, and the answer is the same at
    // every tile.
    gw_slots: std::cell::RefCell<Option<GreatWorkSlotsByPlayer>>,
    gw_housing: std::cell::RefCell<Option<GreatWorkHousing>>,
    // What every regional building in the empire sends this city. Both its
    // yields and its Amenities are read through it, so a single valuation of
    // one city walks the whole empire's buildings twice.
    regional: std::cell::RefCell<Option<BTreeMap<u32, (Yields, f64)>>>,
    // Two facts the luxury allocation asks of every city while it values
    // the whole empire's Amenities. Each city's regional sources ask whether
    // every source city is powered, and each city's own Amenities walk every
    // owned tile of the empire looking for National Parks, so one allocation
    // repeated both empire-wide answers once per city.
    powered: std::cell::RefCell<Option<BTreeMap<u32, bool>>>,
    national_parks: std::cell::RefCell<Option<NationalParksByPlayer>>,
    // Where each empire's regional buildings stand and how far they reach.
    // Every city's regional effects walked every building of every city of
    // its owner to find them, so one luxury allocation walked them once per
    // city.
    regional_slots: std::cell::RefCell<Option<RegionalSlotsByPlayer>>,
    // Wonders are an empire-wide source, but city yields and rule gates ask
    // for one named effect at a time. Aggregate every effect for a player on
    // the first lookup in a memo scope so the same wonder set is not walked
    // once per effect key. The scope is what makes this safe: callers cannot
    // mutate a city or transfer a wonder while the guard is alive.
    wonder_effects: std::cell::RefCell<Option<WonderEffectsByPlayer>>,
    /// `tourism_components_per_turn` per player. A Culture seat's Trader prices
    /// every origin and destination pair, and each pair asks for the whole
    /// empire's Tourism.
    tourism: std::cell::RefCell<Option<BTreeMap<usize, (f64, f64)>>>,
    /// The air-patrol list from the last unit-roster epoch it was built on.
    /// Every route search is its own memo scope, and on a 250-seat world each
    /// one walked every unit in the game to learn that nobody is patrolling.
    air_patrols_stable: std::cell::RefCell<Option<(u64, AirPatrols)>>,
    /// `suzerain_of` per city-state from the last moment nothing it reads
    /// changed. Most scopes follow an action that touched neither envoys nor
    /// cities, and the answer walks every major.
    suzerain_stable: SuzerainStable,
    /// `domestic_tourists` per player. Each seat's victory tracker reads every
    /// rival's count and each count walks every player, which made one frame
    /// cubic in the number of civilizations.
    domestic_tourists: std::cell::RefCell<Option<BTreeMap<usize, i64>>>,
    // `note_first_monopoly_moments` runs after successful actions. Its answer
    // is player-independent until a connected resource, city owner, world
    // resource, or suzerainty changes, so retain the expensive census across
    // ordinary movement/combat-free actions instead of walking a Ludicrous
    // map once per action.
    monopoly_context: std::cell::RefCell<Option<Arc<MonopolyContext>>>,
    // A city production menu walks every unit, building, wonder, project and
    // district in the ruleset. Several city governors ask for that same menu
    // more than once before they commit an action (repairs, products, and
    // wonder fallbacks are separate passes), so retain the complete legal
    // catalog across those read-only decisions. A successful `Game::apply`
    // clears it: that normal mutation boundary can change any prerequisite,
    // queue, resource, or placement fact the catalog reads.
    //
    // Unlike the other entries this intentionally outlives `QueryMemo`. A
    // helper that opens and closes its own short memo must still share the
    // catalog with the next helper in the same decision; retaining it only
    // inside one guard would miss the duplicate scans this cache exists for.
    producible: std::cell::RefCell<BTreeMap<(usize, u32), Vec<Item>>>,
    // `city_yields` of every city the acting seat does not own, kept from one
    // read-only pass to the next while a driver has asked for it. See
    // [`Game::keep_rival_yields`], which is the whole contract; like
    // `producible` this intentionally outlives `QueryMemo`.
    rival_yields: std::cell::RefCell<BTreeMap<u32, Yields>>,
    // The acting seat's own cities, when `Game::keep_own_yields` asks. A
    // separate map because more drops it: see `ActionReach`.
    own_yields: std::cell::RefCell<BTreeMap<u32, Yields>>,
    // What the acting seat's governor settings were when `own_yields` was
    // derived: see `Game::own_settings_stamp`.
    own_yields_stamp: std::cell::Cell<u64>,
    // The tile-grid epoch `rival_yields` was derived under. A tile that
    // changed hands, was improved or was pillaged moves it, whoever wrote it.
    rival_yields_epoch: std::cell::Cell<u64>,
    keeps_rival_yields: std::cell::Cell<bool>,
    // The acting seat's own cities as well; see `Game::keep_own_yields`.
    keeps_own_yields: std::cell::Cell<bool>,
    // `Game::apply` calls in flight. The rules read yields while they are
    // half-way through changing the board, so nothing is kept or reused
    // inside an action.
    acting: std::cell::Cell<u32>,
    // The instrument: derive every reused answer again and compare.
    audits_rival_yields: std::cell::Cell<bool>,
    auditing_rival_yields: std::cell::Cell<bool>,
    rival_yields_reused: std::cell::Cell<u64>,
    rival_yields_derived: std::cell::Cell<u64>,
    // What a read-only sweep asked one seat's policy deck, while a
    // `Game::trace_policy_reads` guard is alive. Nothing records unless a
    // caller opens a trace, so the ordinary path pays one predicted `Cell`
    // load per policy question and nothing else. It lives here rather than on
    // `Game` for the same two reasons the rest of this struct does: it is
    // derived, and a clone must start empty.
    policy_reads_on: std::cell::Cell<bool>,
    policy_reads: std::cell::RefCell<PolicyReadSet>,
}

/// What one read-only sweep asked a seat's policy deck — see
/// [`Game::trace_policy_reads`].
///
/// Every route from a slotted card to an answer runs through
/// [`Game::policy_effect`], [`Game::policy_effect_for_unit`] or
/// [`Game::has_policy`]; outside those three the deck is touched only by the
/// mutators that seat and drop cards, and by [`Game::modifier_context`], whose
/// callers are gated below. So a card naming none of the effect keys and none
/// of the card names recorded here cannot move a single number the sweep
/// produced: every lookup returns the value it already returned, the two runs
/// stay in lockstep, and the result is bit-for-bit the same `f64`.
#[derive(Clone, Debug, Default)]
pub struct PolicyReadSet {
    /// Effect keys the sweep asked for, restricted to keys some card in the
    /// ruleset grants — a key no card grants cannot be a card's key.
    effects: BTreeSet<String>,
    /// Cards the sweep named to [`Game::has_policy`].
    cards: BTreeSet<Name>,
    /// The traced seat carries a runtime modifier attachment. Those are
    /// matched against the whole deck by name in
    /// [`crate::rules::ModifierRequirements`] rather than by effect key, so a
    /// trace taken on such a seat can rule nothing inert.
    opaque: bool,
}

impl PolicyReadSet {
    /// Whether slotting or unslotting `card` is guaranteed to leave every
    /// number the traced sweep produced bit-for-bit identical.
    ///
    /// Conservative in one direction only. An unknown card, an effect key the
    /// sweep did ask for, or a seat carrying an attachment all answer `false`,
    /// which costs the caller exactly the sweep it would have run anyway.
    pub fn card_is_inert(&self, rules: &Rules, card: Name) -> bool {
        if self.opaque || self.cards.contains(&card) {
            return false;
        }
        match rules.policies.get_interned(card) {
            Some(spec) => !spec
                .effects
                .keys()
                .any(|key| self.effects.contains(key.as_str())),
            None => false,
        }
    }

    /// Whether this trace can rule anything inert at all.
    pub fn is_opaque(&self) -> bool {
        self.opaque
    }
}

/// Records what the sweep running under it asks the policy deck. Recording
/// stops when the guard is dropped, whether or not the caller took the answer.
pub struct PolicyReadTrace<'a> {
    game: &'a Game,
}

impl PolicyReadTrace<'_> {
    /// Stop recording and take what was recorded.
    pub fn finish(&self) -> PolicyReadSet {
        self.game.query_memo.policy_reads_on.set(false);
        std::mem::take(&mut *self.game.query_memo.policy_reads.borrow_mut())
    }
}

impl Drop for PolicyReadTrace<'_> {
    fn drop(&mut self) {
        self.game.query_memo.policy_reads_on.set(false);
        *self.game.query_memo.policy_reads.borrow_mut() = PolicyReadSet::default();
    }
}

/// How far an action reaches into the yields `Game::keep_rival_yields` holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionReach {
    /// Only the acting seat's own cities can read differently afterwards.
    OwnCities,
    /// Anything can.
    Board,
}

impl QueryCache {
    /// An action is about to change the board. Nothing kept before it may
    /// answer for the board after it.
    fn action_begins(&self, reach: ActionReach) {
        self.acting.set(self.acting.get() + 1);
        self.drop_kept_yields(reach);
    }

    /// The action is over, successful or not. Whatever the rules derived
    /// while it was half-applied goes too.
    fn action_ends(&self, reach: ActionReach) {
        self.acting.set(self.acting.get().saturating_sub(1));
        self.drop_kept_yields(reach);
    }

    fn drop_kept_yields(&self, reach: ActionReach) {
        if !self.keeps_rival_yields.get() {
            return;
        }
        self.own_yields.borrow_mut().clear();
        if reach == ActionReach::Board {
            self.rival_yields.borrow_mut().clear();
        }
    }
}

impl Clone for QueryCache {
    /// A clone starts with no memo: a search branch is about to be played
    /// forward, and inheriting a parent's answers would be exactly the
    /// staleness this design rules out.
    fn clone(&self) -> Self {
        QueryCache::default()
    }
}

/// Scope over which `Game::city_yields`, `Game::traversal_class`, and the
/// empire-wide read aggregates answer from a cache. Dropping the outermost
/// guard clears it.
/// Every unit flying a patrol, as `(owner, tile)` — see `Game::air_patrols`.
type AirPatrols = Arc<Vec<(usize, Pos)>>;
/// `true` at `Name::id` for improvements that grant passage — see
/// `Game::passage_improvements`.
type PassageTable = Arc<Vec<bool>>;

/// What a movement flood asks about one unit and gets the same answer for at
/// every tile it considers — see [`Game::step_terms`], [`Game::move_terms`]
/// and [`Game::naval_move_terms`].
///
/// The three groups are filled independently, because the code they come from
/// asks for them independently: a step cost never needs the support aura, and
/// a land unit standing on land never needs the naval unlocks. Deriving a
/// group its caller did not ask for would charge a one-off caller for work the
/// open-coded version skipped, which is how a memo becomes a pessimization.
///
/// Each field is the *value* the open-coded expression produced, not a
/// simplification of it, so the arithmetic that reads them adds the same terms
/// in the same order and lands on the same bits.
#[derive(Clone, Copy, Default)]
struct MovementProfile {
    step: Option<StepTerms>,
    moves: Option<MoveTerms>,
    naval: Option<NavalMoveTerms>,
}

/// The unit's half of `Game::unit_step_cost`.
#[derive(Clone, Copy)]
struct StepTerms {
    /// A religious unit whose religion flattens every step to 1 MP.
    religious_flat_movement: bool,
    /// `unit_effect(unit, "woods_move_cost")`.
    woods_move_cost: f64,
    /// `unit_effect(unit, "hills_move_cost")`.
    hills_move_cost: f64,
    /// `promotion_effect(unit, "amphibious")`.
    amphibious: f64,
}

/// The terms of `Game::unit_base_max_moves_at` that do not read the tile.
#[derive(Clone, Copy)]
struct MoveTerms {
    /// `promotion_effect(unit, "movement")`.
    promotion_movement: f64,
    civilization_movement: f64,
    /// A religious unit standing beside a Guru of its own religion. The wonder
    /// effect that pays for it stays at the call site: it is this seven-tile
    /// occupancy scan, not the lookup, that costs anything.
    guru_adjacent: bool,
    /// Monumentality is active and this unit is a Builder.
    ///
    /// Firaxis's `COMMEMORATION_INFRASTRUCTURE_GA_MOVEMENT` modifier is
    /// gated by `REQUIREMENT_UNIT_IS_BUILDER`; Settlers receive the faith
    /// purchase discount, but not the two movement points. Keeping those
    /// effects separate matters to the live mirror: a Settler with the
    /// fictional four-point allowance can be judged unable to cross a river
    /// even though the host correctly lets its real two-point unit cross.
    monumentality_builder: bool,
    /// Exodus of the Evangelists is active and this unit is religious.
    exodus_religious: bool,
    /// `adjacent_support_effect(unit, "adjacent_movement")`.
    adjacent_movement: f64,
}

/// The movement unlocks only a naval or embarked unit is charged for.
#[derive(Clone, Copy)]
struct NavalMoveTerms {
    /// `tree_effect(unit.owner, "naval_movement")`.
    naval_movement_tree: f64,
    /// `tree_effect(unit.owner, "embarked_movement")`.
    embarked_movement_tree: f64,
    /// `has_ability(unit.owner, "mana")`.
    mana: bool,
    /// `empire_wonder_effect(unit.owner, "naval_movement")`.
    naval_movement_wonder: f64,
}

pub struct QueryMemo<'a> {
    game: &'a Game,
    outermost: bool,
}

impl Drop for QueryMemo<'_> {
    fn drop(&mut self) {
        if self.outermost {
            *self.game.query_memo.yields.borrow_mut() = None;
            *self.game.query_memo.appeal.borrow_mut() = None;
            *self.game.query_memo.tile_yields.borrow_mut() = None;
            *self.game.query_memo.district_yields.borrow_mut() = None;
            *self.game.query_memo.traversal.borrow_mut() = None;
            *self.game.query_memo.air_patrols.borrow_mut() = None;
            *self.game.query_memo.passage_improvements.borrow_mut() = None;
            *self.game.query_memo.movement.borrow_mut() = None;
            *self.game.query_memo.amenities.borrow_mut() = None;
            *self.game.query_memo.purchase_price.borrow_mut() = None;
            *self.game.query_memo.unit_ids.borrow_mut() = None;
            *self.game.query_memo.unit_territory_access.borrow_mut() = None;
            *self.game.query_memo.city_ids.borrow_mut() = None;
            *self.game.query_memo.lux_alloc.borrow_mut() = None;
            *self.game.query_memo.lux_names.borrow_mut() = None;
            *self.game.query_memo.housed_works.borrow_mut() = None;
            *self.game.query_memo.housed_pieces.borrow_mut() = None;
            *self.game.query_memo.suzerain.borrow_mut() = None;
            *self.game.query_memo.gw_slots.borrow_mut() = None;
            *self.game.query_memo.gw_housing.borrow_mut() = None;
            *self.game.query_memo.regional.borrow_mut() = None;
            *self.game.query_memo.powered.borrow_mut() = None;
            *self.game.query_memo.national_parks.borrow_mut() = None;
            *self.game.query_memo.regional_slots.borrow_mut() = None;
            *self.game.query_memo.wonder_effects.borrow_mut() = None;
            *self.game.query_memo.tourism.borrow_mut() = None;
            *self.game.query_memo.domestic_tourists.borrow_mut() = None;
        }
    }
}

impl VisionCache {
    fn reset(&mut self, stamp: u64) {
        self.stamp = stamp;
        self.entries = Arc::new(VisionCacheEntries::default());
        self.built_wonders = None;
    }

    fn wonders(&mut self, stamp: u64, build: impl FnOnce() -> BTreeSet<Name>) -> &BTreeSet<Name> {
        if self.stamp != stamp {
            self.reset(stamp);
        }
        self.built_wonders.get_or_insert_with(build)
    }

    fn lookup(&mut self, stamp: u64, uid: u32, key: u64) -> Option<&TileBits> {
        if self.stamp != stamp {
            self.reset(stamp);
            return None;
        }
        let entries = self.entries.as_ref();
        let slot = entries.units.binary_search(&uid).ok()?;
        (entries.keys[slot] == key).then(|| entries.seen[slot].as_ref())
    }

    fn put(&mut self, stamp: u64, uid: u32, key: u64, seen: TileBits) {
        self.put_shared(stamp, uid, key, Arc::new(seen));
    }

    fn put_shared(&mut self, stamp: u64, uid: u32, key: u64, seen: Arc<TileBits>) {
        if self.stamp != stamp {
            self.reset(stamp);
        }
        let entries = Arc::make_mut(&mut self.entries);
        match entries.units.binary_search(&uid) {
            Ok(slot) => {
                entries.keys[slot] = key;
                entries.seen[slot] = seen;
            }
            Err(slot) => {
                entries.units.insert(slot, uid);
                entries.keys.insert(slot, key);
                entries.seen.insert(slot, seen);
            }
        }
    }

    /// Fold worker-local sight answers back into the authoritative cache.
    ///
    /// Every worker starts from a clone of the same final position. Ignore a
    /// cache that never advanced to that position's stamp (a seat with no
    /// sight sources can legitimately leave it untouched). An unchanged worker
    /// still points at the authoritative table, so it needs no merge at all;
    /// a worker that derived a missing ray has its own copy-on-write table,
    /// which is folded in stable ID order through `put`.
    fn merge_current(&mut self, other: VisionCache, stamp: u64) {
        if other.stamp != stamp {
            return;
        }
        if self.stamp != stamp {
            self.reset(stamp);
        }
        if !Arc::ptr_eq(&self.entries, &other.entries) {
            for ((unit, key), seen) in other
                .entries
                .units
                .iter()
                .zip(&other.entries.keys)
                .zip(&other.entries.seen)
            {
                self.put_shared(stamp, *unit, *key, Arc::clone(seen));
            }
        }
        if let Some(wonders) = other.built_wonders {
            self.built_wonders = Some(wonders);
        }
    }
}

/// The expensive, viewer-independent part of one unit's attack envelope.
///
/// An AI still decides whether this unit is hostile and visible to its own
/// seat. Once those filters have selected a unit, though,
/// [`Game::attack_reach_from_flood`] reads only the board and that unit. Keep
/// both products together: the target list is the answer callers need and the
/// flood is the exact sensitivity input the envelope cache needs to decide
/// whether that answer survives a later board change.
#[derive(Debug)]
pub(crate) struct AttackReachFromFlood {
    /// The target list is immutable once the flood has built it. Keeping it
    /// behind an `Arc` lets the AI's `EnvelopeReach` retain precisely this
    /// sorted list instead of allocating and copying a second one per
    /// envelope rebuild.
    targets: Arc<[Pos]>,
    flood: Vec<Pos>,
}

impl AttackReachFromFlood {
    #[inline]
    pub(crate) fn shared_targets(&self) -> Arc<[Pos]> {
        Arc::clone(&self.targets)
    }

    #[inline]
    pub(crate) fn flood(&self) -> &[Pos] {
        &self.flood
    }
}

/// Reach answers for one complete envelope-board fingerprint.
///
/// `BasicAi` already computes this fingerprint before it asks for an envelope.
/// Storing only one board at a time is deliberate: a speculative branch that
/// changes a unit must never be served its parent's reach, and retaining a
/// table for every short-lived tactical branch would trade flood work for an
/// unbounded cache. A matching board, however, is common when separate
/// controllers inspect the same position, and a game clone shares this cache
/// through its [`Arc`].
#[derive(Default)]
struct AttackReachCache {
    key: Option<(u32, u64)>,
    reaches: HashMap<u32, Arc<AttackReachFromFlood>>,
    #[cfg(test)]
    computations: u64,
}

/// Everything about a mover that decides which tiles it could ever stand
/// on, boiled down to four flags. Two units with equal classes traverse
/// exactly the same set of tiles, so one connectivity map computed for the
/// class serves every such unit. Per-edge and per-moment rules — cliffs,
/// stacking, closed borders, zone of control — are deliberately absent:
/// they gate individual steps, not which regions exist.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TraversalClass {
    /// Naval movement: water, city centers, and canal-like districts.
    sea: bool,
    /// A land mover that may enter water (embarkation, or a wading giant).
    embark: bool,
    /// May enter deep ocean (ocean navigation, or a wading giant).
    ocean: bool,
    /// A worker whose unlocked improvement lets it climb mountains.
    mountain: bool,
}

/// Connected regions of the map, one labeling per way of moving.
///
/// `zones[i]` labels tile `i` (in map order) with a region id, 0 for tiles
/// the class can never stand on. Two tiles with the same non-zero label are
/// linked by a chain of statically-traversable tiles, so the router can
/// reject a target on a foreign landmass in one lookup instead of flooding
/// the world to discover the disconnect. Labels over-approximate true
/// reachability — dynamic blockers like stacking, borders, and foreign
/// cities are ignored — which is the safe direction: a zone mismatch is
/// proof no route exists, while a match is still verified by the search.
///
/// Region maps are shared (`Arc`) so the AI's per-branch game clones copy a
/// pointer, not the world.
#[derive(Default)]
pub struct RoutingCache {
    stamp: u64,
    zones: Vec<(TraversalClass, std::sync::Arc<Vec<u32>>)>,
    paths: Vec<PlannedRoute>,
    /// Reverse distance fields shared by units with the same traversal and
    /// border access. A large goal set (unexplored tiles, landing sites, or a
    /// staging ring) is otherwise flooded once from every unit that asks for
    /// its nearest member. The field is derived state and is discarded on a
    /// map/turn epoch change or when a game is cloned for a branch.
    reverse_fields: Vec<ReverseFlowField>,
    /// Folded city ids and owners used by route access checks. City ownership
    /// changes invalidate routing paths, so compute this once per routing
    /// epoch instead of walking every city for every unit route query.
    city_owner_key: Option<u64>,
}

impl Clone for RoutingCache {
    /// Connected-region labels are map-only and can safely be shared across a
    /// branch, while a planned path also depends on unit positions and
    /// diplomacy that the branch is about to change. Keep the cheap `Arc`
    /// labels but discard those path answers and their owner key.
    fn clone(&self) -> Self {
        Self {
            stamp: self.stamp,
            zones: self.zones.clone(),
            paths: Vec::new(),
            reverse_fields: Vec::new(),
            city_owner_key: None,
        }
    }
}

/// A multi-source reverse breadth-first field for one route-access context.
///
/// Future route segments intentionally have unit occupancy removed, and their
/// legality depends only on the unit's traversal class, owner, and territory
/// access. Those are exactly the values folded into `access_key`, so every
/// unit sharing that context can read one field and only validate its own
/// immediate step with `can_enter`.
#[derive(Clone)]
struct ReverseFlowField {
    class: TraversalClass,
    access_key: u64,
    goals: Arc<Vec<Pos>>,
    distance: Arc<Vec<i32>>,
}

const MAX_REVERSE_FLOW_FIELDS: usize = 8;

/// What one tile of a movement flood remembers.
///
/// The three floods in `game.rs` — [`Game::flow_past`], [`Game::path_to`] and
/// [`Game::approach_reach`] — were three copies of one relaxation loop and
/// differed in exactly two places: which step test they use, and what entering
/// a zone of control does to the movement recorded on arrival. The step test is
/// a closure on [`Game::relax_movement`]; this trait is the second difference.
///
/// `flow_past` and `path_to` write 0 there, which is the right answer to "can
/// it move on". `approach_reach` keeps the movement and marks the tile as one
/// nothing expands from, which is the right answer to "can it still strike": a
/// unit stopped by a zone of control keeps its unused movement for the blow
/// (`do_attack`, `do_ranged`, `can_pay_melee_entry`).
trait FloodArrival: Copy {
    /// The unit's own tile, before it has spent anything.
    fn start(moves: f64) -> Self;
    /// A neighbour reached with `rem` movement left, `zoc` telling whether
    /// arriving there entered an enemy zone of control.
    fn arrival(rem: f64, zoc: bool) -> Self;
    /// Rebuild the answer from the score and stop bit kept by the dense flood
    /// scratch. This is separate from [`Self::arrival`] because the `f64`
    /// representation folds ZOC into a zero score while the approach flood
    /// retains the score and stores ZOC as its stop bit.
    fn recorded(remaining: f64, stopped: bool) -> Self;
    /// Movement kept here — the value the relaxation compares.
    fn remaining(self) -> f64;
    /// Whether the flood must not expand past this tile.
    fn stopped(self) -> bool;
}

/// Receives the reached tiles from one movement relaxation.
///
/// Most callers need the historical ordered map. `attack_reach_from_flood`
/// only folds the reached tiles into a separately sorted target list, though,
/// so it can retain the dense flood's discovery order in one contiguous vector
/// instead of materializing a temporary tree first.
trait MovementFloodOutput<V> {
    fn reserve_reached(&mut self, count: usize);
    fn push_reached(&mut self, pos: Pos, value: V);
}

impl<V> MovementFloodOutput<V> for BTreeMap<Pos, V> {
    #[inline]
    fn reserve_reached(&mut self, _count: usize) {}

    #[inline]
    fn push_reached(&mut self, pos: Pos, value: V) {
        self.insert(pos, value);
    }
}

impl<V> MovementFloodOutput<V> for Vec<(Pos, V)> {
    #[inline]
    fn reserve_reached(&mut self, count: usize) {
        self.reserve(count);
    }

    #[inline]
    fn push_reached(&mut self, pos: Pos, value: V) {
        self.push((pos, value));
    }
}

impl FloodArrival for f64 {
    fn start(moves: f64) -> Self {
        moves
    }

    fn arrival(rem: f64, zoc: bool) -> Self {
        if zoc {
            0.0 // entering enemy ZOC ends movement
        } else {
            rem
        }
    }

    fn recorded(remaining: f64, _stopped: bool) -> Self {
        remaining
    }

    fn remaining(self) -> f64 {
        self
    }

    fn stopped(self) -> bool {
        false // zero movement already stops the expansion
    }
}

impl FloodArrival for (f64, bool) {
    fn start(moves: f64) -> Self {
        (moves, false)
    }

    fn arrival(rem: f64, zoc: bool) -> Self {
        (rem, zoc)
    }

    fn recorded(remaining: f64, stopped: bool) -> Self {
        (remaining, stopped)
    }

    fn remaining(self) -> f64 {
        self.0
    }

    fn stopped(self) -> bool {
        self.1
    }
}

/// The terms of [`Game::unit_max_moves_at`] which name the unit rather than
/// the tile: which unit's allowance a tile is read for, and whether a linked
/// pair takes the slower of the two. A movement flood asks that question once
/// per neighbour of every tile it expands and the answer cannot change between
/// them, so [`Game::relax_movement`] settles it once and then only reads the
/// tile.
#[derive(Clone, Copy)]
enum MovesCap {
    /// The unit's own allowance.
    Own,
    /// A linked escort's allowance, in place of the unit's own.
    Peer(u32),
    /// The slower of the unit's own and the linked peer's.
    Min(u32),
}

/// The terms of [`Game::formation_enters_enemy_zoc`] which name the formation
/// rather than the tile: which units of a linked pair are subject to zone of
/// control at all. Hoisted for the same reason as [`MovesCap`] — `unit_ignores_zoc`
/// is a ruleset lookup and a handful of string comparisons, and the flood was
/// paying for it once per neighbour.
#[derive(Clone, Copy)]
struct ZocTest {
    /// The moving unit, unless it ignores zone of control.
    mover: Option<u32>,
    /// Its linked peer, when this unit leads the pair and the peer does not
    /// ignore zone of control.
    peer: Option<u32>,
}

type AStarEntry = Reverse<(i32, Reverse<i32>, Pos, usize)>;

#[derive(Default)]
struct RouteScratch {
    astar_frontier: BinaryHeap<AStarEntry>,
    bfs_frontier: VecDeque<(Pos, usize)>,
    path: Vec<Pos>,
    parent: Vec<u32>,
    seen: Vec<bool>,
    distance: Vec<i32>,
    astar_touched: Vec<usize>,
    bfs_touched: Vec<usize>,
    /// Dense movement-flood state. The old flood used a `BTreeMap<Pos, V>`
    /// for every relaxation; these vectors keep the same strict-improvement
    /// and LIFO visitation order while replacing tree descent with array
    /// reads. `movement_touched` is the only part reset between floods.
    movement_frontier: Vec<usize>,
    movement_touched: Vec<usize>,
    movement_seen: Vec<bool>,
    movement_score: Vec<f64>,
    movement_stopped: Vec<bool>,
}

impl Clone for RouteScratch {
    /// A route search mutates every scratch vector. Reusing a parent's
    /// frontier would copy the largest transient allocation in a branch for
    /// no semantic benefit; a fresh scratch area grows only if the branch
    /// actually asks for a route.
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[derive(Clone)]
struct PlannedRoute {
    unit: u32,
    target: Pos,
    range: i32,
    /// Diplomatic and city-ownership state which made the path legal. The
    /// routing cache already expires at every new turn and map mutation; this
    /// compact key catches same-turn deals, wars, civics, and city transfers
    /// without rescanning the complete remaining path before every step.
    access_key: u64,
    /// Complete path including the position where it was planned and the
    /// stopping tile. Looking up the unit's live position makes repeated
    /// calls before a move harmless and advances naturally after one.
    path: Vec<Pos>,
}

/// Everything a player's luxury access is counted from, taken once rather
/// than once per luxury: their own connected resources, those of the
/// city-states they are suzerain of, and those of an Amani host.
struct LuxuryHoldings<'a> {
    own: BTreeMap<&'a str, i32>,
    clients: Vec<BTreeMap<&'a str, i32>>,
    amani: Option<BTreeMap<&'a str, i32>>,
}

/// Everything one Loyalty tick needs to apply atomically. Free Cities keep
/// the per-civilization pressure as a ledger until their second revolt, when
/// the civilization that exerted the most total pressure receives the city.
#[derive(Default)]
struct LoyaltyChange {
    delta: f64,
    foreign_pressure: BTreeMap<usize, f64>,
    locked: bool,
}

/// The seats in a game, each shared until it is written to.
///
/// A game is cloned to look ahead, and a seat carries everything that player
/// has ever learned — its explored map, its counters, its trees. A branch
/// that moves one unit changes at most one seat, so copying all of them per
/// branch was most of what cloning a game cost.
///
/// Indexing mutably is what copies a seat, and every write already goes
/// through that, so nothing can write through the sharing by accident.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(from = "Vec<Player>", into = "Vec<Player>")]
pub struct Players(
    Vec<Arc<Player>>,
    /// Bumped by every route that can reach a `&mut Player`: `get_mut`,
    /// `IndexMut::index_mut`, `iter_mut`, and `push`. That is the whole
    /// mutation surface — the wrapped `Vec` is private and nothing else in
    /// this file reaches inside it — so a caller can tell "no seat changed"
    /// from one cheap integer compare instead of re-deriving an answer that
    /// reads the roster. `Game::suzerain_input_map` and
    /// `Game::visibility_viewers` key their per-ask caches off this.
    u64,
);

impl Players {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push(&mut self, player: Player) {
        self.0.push(Arc::new(player));
        self.1 = self.1.wrapping_add(1);
    }

    pub fn get(&self, index: usize) -> Option<&Player> {
        self.0.get(index).map(|seat| &**seat)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Player> {
        self.1 = self.1.wrapping_add(1);
        self.0.get_mut(index).map(Arc::make_mut)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Player> + ExactSizeIterator {
        self.0.iter().map(|seat| &**seat)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Player> {
        self.1 = self.1.wrapping_add(1);
        self.0.iter_mut().map(Arc::make_mut)
    }

    /// See the tuple field's doc comment.
    #[inline]
    fn epoch(&self) -> u64 {
        self.1
    }
}

impl std::ops::Index<usize> for Players {
    type Output = Player;

    #[inline]
    fn index(&self, index: usize) -> &Player {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for Players {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Player {
        self.1 = self.1.wrapping_add(1);
        Arc::make_mut(&mut self.0[index])
    }
}

impl<'a> IntoIterator for &'a Players {
    type Item = &'a Player;
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, Arc<Player>>, fn(&'a Arc<Player>) -> &'a Player>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|seat| &**seat)
    }
}

impl From<Vec<Player>> for Players {
    fn from(players: Vec<Player>) -> Players {
        Players(players.into_iter().map(Arc::new).collect(), 0)
    }
}

impl From<Players> for Vec<Player> {
    fn from(players: Players) -> Vec<Player> {
        players
            .0
            .into_iter()
            .map(|seat| Arc::try_unwrap(seat).unwrap_or_else(|shared| (*shared).clone()))
            .collect()
    }
}

impl FromIterator<Player> for Players {
    fn from_iter<I: IntoIterator<Item = Player>>(players: I) -> Players {
        Players(players.into_iter().map(Arc::new).collect(), 0)
    }
}

/// The historic moments a game has recorded, shared until written to.
///
/// Events only accumulate, and a game cloned to look ahead almost never adds
/// one — but it was copying every event ever recorded, each with its own
/// category and prose, and that grew with the length of the game.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(from = "Vec<Event>", into = "Vec<Event>")]
pub struct EventLog(Arc<Vec<Event>>);

impl std::ops::Deref for EventLog {
    type Target = Vec<Event>;

    fn deref(&self) -> &Vec<Event> {
        &self.0
    }
}

/// Taking the log mutably is what copies it.
impl std::ops::DerefMut for EventLog {
    fn deref_mut(&mut self) -> &mut Vec<Event> {
        Arc::make_mut(&mut self.0)
    }
}

impl From<Vec<Event>> for EventLog {
    fn from(events: Vec<Event>) -> EventLog {
        EventLog(Arc::new(events))
    }
}

impl From<EventLog> for Vec<Event> {
    fn from(log: EventLog) -> Vec<Event> {
        Arc::try_unwrap(log.0).unwrap_or_else(|shared| (*shared).clone())
    }
}

// `track_fog_memory` is `#[serde(skip, default = "yes")]`, so this is the
// deserialization default that keeps fog tracking on for every save and the
// normal runtime path. `skip` elides the call site the compiler can see, so it
// reports the function as unused; it is not.
#[allow(dead_code)]
fn yes() -> bool {
    true
}

/// Runtime-only coalescing state for observation maintenance.
///
/// A game clone is a new simulation branch, not a continuation of the
/// caller's stack frame, so it deliberately starts outside any surrounding
/// batch. The ordinary `Game` clone can therefore stay derived while tactical
/// lookahead never inherits an unfinished refresh.
#[derive(Default)]
struct VisibilityBatch {
    depth: usize,
    refresh_all: bool,
    refresh_teams: BTreeSet<usize>,
}

/// A runtime vector shared between a game and its disposable branches.
/// Visibility snapshot coverage is derived state; branches that never publish
/// an observation can keep the parent allocation without copying it, while a
/// real refresh gets an owned vector through `Arc::make_mut`.
#[derive(Default)]
struct SharedVec<T>(Arc<Vec<T>>);

impl<T> Clone for SharedVec<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> std::ops::Deref for SharedVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Clone> std::ops::DerefMut for SharedVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

/// Below this many total seats, cloning the final position for worker-local
/// visibility caches costs more than the ray sweeps it replaces. Paired
/// release runs were neutral at 20 seats and positive at 50; keep ordinary
/// games on the already cache-efficient serial path.
const PARALLEL_VISIBILITY_MIN_PLAYERS: usize = 32;

impl Clone for VisibilityBatch {
    fn clone(&self) -> Self {
        Self::default()
    }
}

fn bump(p: &mut Player, key: &str) {
    *p.counters.entry(key.to_string()).or_insert(0) += 1;
}

// ------------------------------------------------------------------ entities

fn lvl1() -> i32 {
    1
}

fn one_attack() -> i32 {
    1
}

fn one_u32() -> u32 {
    1
}

/// Aggregate combat output and Production investment for one unit kind.
///
/// Completed lives are folded into a player's saved ledger when a unit leaves
/// play. [`Game::unit_lifetime_stats`] adds the units still on the field, so a
/// finished battle has one observation for every unit it fielded rather than
/// silently dropping the survivors.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct UnitLifetimeStats {
    pub units: u64,
    pub damage_dealt: u64,
    pub production_cost: f64,
}

impl UnitLifetimeStats {
    fn observe(&mut self, unit: &Unit, production_cost: f64) {
        self.units = self.units.saturating_add(1);
        self.damage_dealt = self.damage_dealt.saturating_add(unit.damage_dealt);
        self.production_cost += production_cost;
    }

    /// Mean enemy-unit HP removed over one observed life.
    pub fn average_damage(&self) -> Option<f64> {
        (self.units > 0).then(|| self.damage_dealt as f64 / self.units as f64)
    }

    /// Mean Production invested in one observed unit.
    pub fn average_production_cost(&self) -> Option<f64> {
        (self.units > 0).then(|| self.production_cost / self.units as f64)
    }

    /// Enemy-unit HP removed per point of Production invested.
    pub fn damage_per_production(&self) -> Option<f64> {
        (self.production_cost > 0.0).then(|| self.damage_dealt as f64 / self.production_cost)
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Unit {
    pub id: u32,
    #[serde(rename = "type")]
    pub kind: Name,
    pub owner: usize,
    pub pos: Pos,
    pub hp: i32,
    /// Enemy-unit hit points this unit has actually removed over its life.
    /// Killing blows are capped at the victim's remaining health, so overkill
    /// cannot make one unit look more productive than another. The counter
    /// survives saves, promotions, and upgrades with the unit.
    #[serde(default)]
    pub damage_dealt: u64,
    /// Production represented by this unit when it entered play. Directly
    /// trained formations use their 1.5x/2x price and combined formations add
    /// their constituents, so the lifetime ledger measures the investment
    /// that produced the combat output rather than today's upgrade target.
    #[serde(default)]
    pub production_cost: f64,
    pub moves_left: f64,
    pub charges: i32,
    #[serde(default)]
    pub xp: i64,
    /// Cumulative album sales from successful Rock Band concerts. This is
    /// added to the next concert's Tourism multiplier.
    #[serde(default)]
    pub album_sales: i64,
    #[serde(default = "lvl1")]
    pub level: i32,
    /// Chosen nodes from this unit class's promotion tree.
    #[serde(default)]
    pub promotions: BTreeSet<Name>,
    /// Patron Saint grants a second choice after this unit's first promotion.
    #[serde(default)]
    pub extra_first_promotion: bool,
    /// Exempt from strategic-resource maintenance (the meteor goody's
    /// GOODY_METEOR_UNIT_REFUND_COST travels with the granted unit).
    #[serde(default)]
    pub free_upkeep: bool,
    /// 0 = unit, 1 = Corps/Fleet, 2 = Army/Armada.
    #[serde(default)]
    pub formation: u8,
    /// Symmetric escort/support link. Linked units occupy and move on one tile.
    #[serde(default)]
    pub linked_to: Option<u32>,
    /// Religious units retain the majority religion of their purchase city.
    #[serde(default)]
    pub religion: Option<String>,
    #[serde(default = "one_attack")]
    pub attacks_left: i32,
    #[serde(default)]
    pub moved: bool,
    #[serde(default)]
    pub fortified: bool,
    /// Consecutive inactive/fortified turns, capped at two for +3/+6 CS.
    #[serde(default)]
    pub fortify_turns: i32,
    /// Whether the unit moved or acted since its last turn began. Healing and
    /// siege setup both depend on this rather than on remaining movement.
    #[serde(default)]
    pub acted: bool,
    #[serde(default)]
    pub zoc_stopped: bool,
    /// The host's own `Unit:IsEmbarked` reading, pinned to the position it was
    /// observed on. While the unit stands there it wins over the "its tile is
    /// water" derivation; a unit the board has moved, and an older export that
    /// carries no flag, derive from the tile. See `unit_is_embarked_at`.
    #[serde(default)]
    pub host_embarked: Option<(Pos, bool)>,
    /// Snapshot taken when this unit's turn begins. A unit may leave a ZOC
    /// as its first action, but attacking before leaving forfeits that move.
    #[serde(default)]
    pub started_turn_in_zoc: bool,
    /// Fighters on patrol can intercept hostile air missions adjacent to the
    /// selected deployment hex. Their base remains in `pos`.
    #[serde(default)]
    pub air_patrol: bool,
    #[serde(default)]
    pub air_patrol_pos: Option<Pos>,
    /// Permanent movement granted when the unit is trained (for example by
    /// the Royal Navy Dockyard).
    #[serde(default)]
    pub bonus_moves: f64,
    /// Permanent combat-experience modifier inherited from the training
    /// city's military infrastructure.
    #[serde(default)]
    pub xp_bonus_pct: f64,
    /// Original city-state owner and expiry for a military levy. These fields
    /// are save-compatible and remain empty for ordinary units.
    #[serde(default)]
    pub levied_from: Option<usize>,
    #[serde(default)]
    pub levied_until: u32,
}

/// The unit roster shared by game clones and search branches.
///
/// A branch normally reads many units and writes only the handful named by its
/// action.  Keeping the roster as an immutable `Arc` snapshot means cloning a
/// position copies neither the map nor any unit.  The first branch write makes
/// a shallow map copy (its values are still `Arc`s), then `get_mut` clones only
/// the unit being edited.  Insertions and removals are likewise local to the
/// branch.  This is the unit equivalent of [`Players`]' copy-on-write seats,
/// with the extra per-unit indirection making combat lookahead proportional to
/// the units it actually touches rather than to the whole army.
#[derive(Default)]
pub struct Units(Arc<BTreeMap<u32, Arc<Unit>>>, BTreeSet<u32>, u64);

impl Clone for Units {
    fn clone(&self) -> Self {
        // A game clone starts a new branch. The immutable map is shared, but
        // its write set belongs only to that branch and therefore starts empty.
        Self(Arc::clone(&self.0), BTreeSet::new(), self.2)
    }
}

impl Units {
    /// Bump the cache epoch before exposing a mutable unit.  The vision stamp
    /// itself still folds only sight-relevant fields, so a combat-only write
    /// recomputes the small stamp but keeps an otherwise matching frame.
    #[inline]
    fn mark_mutated(&mut self) {
        self.2 = self.2.wrapping_add(1);
    }

    #[inline]
    fn vision_epoch(&self) -> u64 {
        self.2
    }

    /// Borrow one unit from the immutable snapshot.
    #[inline]
    pub fn get(&self, id: &u32) -> Option<&Unit> {
        self.0.get(id).map(Arc::as_ref)
    }

    /// Enter the branch delta for one unit.  The map is copied at most once,
    /// and the unit value is copied only when this branch is the first writer
    /// for that id.
    #[inline]
    pub fn get_mut(&mut self, id: &u32) -> Option<&mut Unit> {
        if self.0.contains_key(id) {
            self.1.insert(*id);
            self.mark_mutated();
        }
        Arc::make_mut(&mut self.0).get_mut(id).map(Arc::make_mut)
    }

    #[inline]
    pub fn contains_key(&self, id: &u32) -> bool {
        self.0.contains_key(id)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, u32, Arc<Unit>> {
        self.0.keys()
    }

    /// Iterate over units without exposing the internal `Arc` values.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Unit> + ExactSizeIterator {
        self.0.values().map(Arc::as_ref)
    }

    /// Iterate over `(id, unit)` pairs without exposing the internal `Arc`
    /// values.  The map remains ordered by id, as the old `BTreeMap` field was.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&u32, &Unit)> + ExactSizeIterator {
        self.0.iter().map(|(id, unit)| (id, unit.as_ref()))
    }

    /// Mutable iteration is a branch delta for every unit visited.  Callers
    /// already asking for `iter_mut` intend to rewrite the roster, so this is
    /// deliberately explicit rather than making ordinary reads pay for it.
    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Unit> + ExactSizeIterator {
        self.1.extend(self.0.keys().copied());
        self.mark_mutated();
        Arc::make_mut(&mut self.0).values_mut().map(Arc::make_mut)
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (&u32, &mut Unit)> + ExactSizeIterator {
        self.1.extend(self.0.keys().copied());
        self.mark_mutated();
        Arc::make_mut(&mut self.0)
            .iter_mut()
            .map(|(id, unit)| (id, Arc::make_mut(unit)))
    }

    /// Insert a unit, returning the previous value just like `BTreeMap`.
    pub fn insert(&mut self, id: u32, unit: Unit) -> Option<Unit> {
        self.1.insert(id);
        self.mark_mutated();
        Arc::make_mut(&mut self.0)
            .insert(id, Arc::new(unit))
            .map(arc_into_unit)
    }

    /// Remove a unit, returning the owned value just like `BTreeMap`.
    pub fn remove(&mut self, id: &u32) -> Option<Unit> {
        self.1.insert(*id);
        self.mark_mutated();
        Arc::make_mut(&mut self.0).remove(id).map(arc_into_unit)
    }

    pub fn clear(&mut self) {
        self.1.extend(self.0.keys().copied());
        self.mark_mutated();
        Arc::make_mut(&mut self.0).clear();
    }

    /// Retain units by their immutable value.  No unit clone is needed for a
    /// predicate, which keeps cleanup passes cheap even on a shared snapshot.
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&u32, &Unit) -> bool,
    {
        self.1.extend(self.0.keys().copied());
        self.mark_mutated();
        Arc::make_mut(&mut self.0).retain(|id, unit| keep(id, unit));
    }

    /// Consume the roster in stable id order.  A unique snapshot unwraps all
    /// arcs; a shared snapshot clones only the values the caller requested.
    pub fn into_values(self) -> std::vec::IntoIter<Unit> {
        let values = match Arc::try_unwrap(self.0) {
            Ok(map) => map.into_values().map(arc_into_unit).collect::<Vec<_>>(),
            Err(map) => map
                .values()
                .map(|unit| (**unit).clone())
                .collect::<Vec<_>>(),
        };
        values.into_iter()
    }

    /// Take a cheap immutable snapshot for a speculative branch.  The delta
    /// returned by [`Self::delta_since`] later identifies only ids whose Arc
    /// value changed, was inserted, or was removed.
    pub fn snapshot(&self) -> UnitSnapshot {
        UnitSnapshot(Arc::clone(&self.0))
    }

    /// Return the localized unit changes since `snapshot`.
    pub fn delta_since(&self, snapshot: &UnitSnapshot) -> UnitDelta {
        if Arc::ptr_eq(&self.0, &snapshot.0) {
            return UnitDelta {
                base: snapshot.clone(),
                changed: BTreeMap::new(),
            };
        }
        let mut changed = BTreeMap::new();
        // Every mutating entry point records its id. Comparing this write set
        // keeps extraction proportional to the branch's edits instead of
        // walking the complete army after every AI turn.
        for id in &self.1 {
            let before = snapshot.0.get(id);
            let after = self.0.get(id);
            match (before, after) {
                (Some(before), Some(after)) if same_unit_arc(before, after) => {}
                (_, Some(after)) => {
                    changed.insert(*id, Some(Arc::clone(after)));
                }
                (_, None) => {
                    changed.insert(*id, None);
                }
            }
        }
        UnitDelta {
            base: snapshot.clone(),
            changed,
        }
    }
}

fn arc_into_unit(unit: Arc<Unit>) -> Unit {
    Arc::try_unwrap(unit).unwrap_or_else(|unit| (*unit).clone())
}

fn same_unit_arc(left: &Arc<Unit>, right: &Arc<Unit>) -> bool {
    Arc::ptr_eq(left, right) || left.as_ref() == right.as_ref()
}

impl std::ops::Index<&u32> for Units {
    type Output = Unit;

    fn index(&self, id: &u32) -> &Unit {
        self.get(id)
            .unwrap_or_else(|| panic!("unit {id} is not present"))
    }
}

impl std::ops::IndexMut<&u32> for Units {
    fn index_mut(&mut self, id: &u32) -> &mut Unit {
        self.get_mut(id)
            .unwrap_or_else(|| panic!("unit {id} is not present"))
    }
}

impl<'a> IntoIterator for &'a Units {
    type Item = (&'a u32, &'a Unit);
    type IntoIter = std::iter::Map<
        std::collections::btree_map::Iter<'a, u32, Arc<Unit>>,
        fn((&'a u32, &'a Arc<Unit>)) -> (&'a u32, &'a Unit),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(id, unit)| (id, unit.as_ref()))
    }
}

impl<'a> IntoIterator for &'a mut Units {
    type Item = (&'a u32, &'a mut Unit);
    type IntoIter = std::iter::Map<
        std::collections::btree_map::IterMut<'a, u32, Arc<Unit>>,
        fn((&'a u32, &'a mut Arc<Unit>)) -> (&'a u32, &'a mut Unit),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.mark_mutated();
        self.1.extend(self.0.keys().copied());
        Arc::make_mut(&mut self.0)
            .iter_mut()
            .map(|(id, unit)| (id, Arc::make_mut(unit)))
    }
}

impl IntoIterator for Units {
    type Item = (u32, Unit);
    type IntoIter = std::vec::IntoIter<(u32, Unit)>;

    fn into_iter(self) -> Self::IntoIter {
        let values = match Arc::try_unwrap(self.0) {
            Ok(map) => map
                .into_iter()
                .map(|(id, unit)| (id, arc_into_unit(unit)))
                .collect::<Vec<_>>(),
            Err(map) => map
                .iter()
                .map(|(id, unit)| (*id, (**unit).clone()))
                .collect::<Vec<_>>(),
        };
        values.into_iter()
    }
}

impl FromIterator<(u32, Unit)> for Units {
    fn from_iter<I: IntoIterator<Item = (u32, Unit)>>(items: I) -> Self {
        Self(
            Arc::new(
                items
                    .into_iter()
                    .map(|(id, unit)| (id, Arc::new(unit)))
                    .collect(),
            ),
            BTreeSet::new(),
            0,
        )
    }
}

impl From<BTreeMap<u32, Unit>> for Units {
    fn from(map: BTreeMap<u32, Unit>) -> Self {
        map.into_iter().collect()
    }
}

/// The world's cities, and the sight inputs folded out of them.
///
/// This is a module rather than a bare type so that `Cities::map` is private
/// to *these* lines and not merely to `game`. A private field is readable by
/// every descendant module of the one that declares it, and `game` has three
/// dozen of them; declaring the roster here means no other file in the crate
/// can reach past the accessors, whatever it is a child of.
mod city_roster {
    use super::{vision_key, Arc, BTreeMap, City};

    /// Every city fact the visibility derivation reads, folded once per
    /// roster state instead of once per sight ask.
    ///
    /// Three folds, because sight asks the roster three different questions.
    /// `Game::see_from_level` reads the whole world's city geometry; a seat's
    /// own borders are an unconditional sight source; and a suzerain is shown
    /// a fixed ring around each of its city-states' centres. Every chain walks
    /// the roster in city-id order, exactly as the `BTreeMap::values()` loops
    /// it replaces did, so a fold is a pure function of the roster.
    #[derive(Default)]
    pub(super) struct CityVisionStamps {
        /// The city half of `Game::vision_geometry_stamp`: identity, centre,
        /// encampment damage and every district placement, for every city in
        /// the world regardless of owner.
        pub(super) geometry: u64,
        /// One compact signature of every city fact that any visibility
        /// derivation reads.  Unlike a mutation generation this also travels
        /// with a whole roster assignment, so a cache outside `Cities` can
        /// distinguish a replacement whose generation happens to match.
        pub(super) input: u64,
        /// Per owner: identity, centre and every owned tile — the border
        /// reveal `Game::base_player_visibility` unions for a seat's own
        /// cities.
        owned: BTreeMap<usize, u64>,
        /// Per owner: identity and centre only — all that a suzerain's
        /// three-tile reveal around a city-state's cities can depend on.
        centres: BTreeMap<usize, u64>,
    }

    impl CityVisionStamps {
        const GEOMETRY_BASE: u64 = 0x67;
        const OWNED_BASE: u64 = 0x68;
        const CENTRE_BASE: u64 = 0x69;
        const INPUT_BASE: u64 = 0x6a;

        /// The empty chain for one owner. An owner with no cities and an owner
        /// whose cities were all folded in must not share a value, and this is
        /// also what that owner's chain starts from.
        fn owner_seed(base: u64, owner: usize) -> u64 {
            vision_key(&[base, owner as u64])
        }

        pub(super) fn owned(&self, owner: usize) -> u64 {
            self.owned
                .get(&owner)
                .copied()
                .unwrap_or_else(|| Self::owner_seed(Self::OWNED_BASE, owner))
        }

        pub(super) fn centres(&self, owner: usize) -> u64 {
            self.centres
                .get(&owner)
                .copied()
                .unwrap_or_else(|| Self::owner_seed(Self::CENTRE_BASE, owner))
        }

        fn build(map: &BTreeMap<u32, City>) -> Self {
            let mut stamps = Self {
                geometry: vision_key(&[Self::GEOMETRY_BASE, map.len() as u64]),
                input: 0,
                owned: BTreeMap::new(),
                centres: BTreeMap::new(),
            };
            for city in map.values() {
                stamps.geometry = vision_key(&[
                    stamps.geometry,
                    city.id as u64,
                    city.pos.0 as i64 as u64,
                    city.pos.1 as i64 as u64,
                    city.encampment_pillaged as u64,
                    city.districts.len() as u64,
                    city.wonders.len() as u64,
                ]);
                for state in &city.defending_districts {
                    stamps.geometry = vision_key(&[
                        stamps.geometry,
                        state.pos.0 as u64,
                        state.pos.1 as u64,
                        state.pillaged as u64,
                    ]);
                }
                for (district, position) in city.districts.iter() {
                    stamps.geometry = vision_key(&[
                        stamps.geometry,
                        district.id() as u64,
                        position.0 as i64 as u64,
                        position.1 as i64 as u64,
                    ]);
                }
                let owned = stamps
                    .owned
                    .entry(city.owner)
                    .or_insert_with(|| Self::owner_seed(Self::OWNED_BASE, city.owner));
                *owned = vision_key(&[
                    *owned,
                    3,
                    city.id as u64,
                    city.pos.0 as i64 as u64,
                    city.pos.1 as i64 as u64,
                    city.owned_tiles.len() as u64,
                ]);
                for position in &city.owned_tiles {
                    *owned =
                        vision_key(&[*owned, position.0 as i64 as u64, position.1 as i64 as u64]);
                }
                let centres = stamps
                    .centres
                    .entry(city.owner)
                    .or_insert_with(|| Self::owner_seed(Self::CENTRE_BASE, city.owner));
                *centres = vision_key(&[
                    *centres,
                    city.id as u64,
                    city.pos.0 as i64 as u64,
                    city.pos.1 as i64 as u64,
                ]);
            }
            let mut input = vision_key(&[
                Self::INPUT_BASE,
                stamps.geometry,
                stamps.owned.len() as u64,
                stamps.centres.len() as u64,
            ]);
            for (&owner, &owned) in &stamps.owned {
                input = vision_key(&[input, owner as u64, owned]);
            }
            for (&owner, &centres) in &stamps.centres {
                input = vision_key(&[input, owner as u64, centres]);
            }
            stamps.input = input;
            stamps
        }
    }

    /// The world's cities, wrapped so that every route which can change one is
    /// a method on this type.
    ///
    /// Reads reach the whole `BTreeMap` API through `Deref`, so borrowing,
    /// iterating, indexing and looking a city up are unchanged — 589 existing
    /// `cities.get_mut` call sites and every read compiled against this
    /// without an edit. **`DerefMut` is deliberately absent, and adding one
    /// would undo the whole guarantee.** Without it nothing can obtain
    /// `&mut BTreeMap<u32, City>`, so the only ways to reach a `&mut City`, or
    /// to add, drop or replace one, are the inherent methods below — and each
    /// clears the memo before it hands the caller anything.
    ///
    /// A mutating map method that is not reimplemented here does not compile
    /// at all. That is what makes the invalidation exhaustive rather than
    /// remembered: `cargo check` re-enumerates the whole mutation surface of
    /// every file in the crate on every build, and a mutation route added
    /// years from now is a build error rather than a stale sight frame. Only
    /// the routes the crate actually uses are here, deliberately — `iter_mut`,
    /// `entry`, `retain`, `append`, `drain`, `split_off`, `pop_first` and
    /// `pop_last` are all absent because nothing calls them, and an untested
    /// accessor is a worse guard than a compile error. If you need one, add it
    /// here with an `invalidate()` first; never reach for `DerefMut`.
    ///
    /// The memo is a field of the roster rather than an epoch counter beside
    /// it in `VisionFrameCache`, because a counter can be desynchronized by
    /// assigning or swapping a whole roster: the counter would travel with the
    /// new cities while the memo stayed with the old. A memo that *is* part of
    /// the value cannot come apart from it, and `mem::take`, `mem::swap` and
    /// whole-field assignment are all safe for the same reason.
    ///
    /// Reading the memo needs `&Cities` and every mutable handle borrows
    /// `&mut Cities`, so the borrow checker already forbids a sight ask from
    /// observing the memo while a write to any city is outstanding.
    /// Invalidation is therefore eager — it happens when the handle is issued,
    /// before the caller has written anything — and no ask can fall between
    /// the two.
    #[derive(Default)]
    pub struct Cities {
        /// Speculative unit branches usually never change a city. Sharing the
        /// roster across those clones removes a full B-tree walk and all of
        /// each City's nested allocations; the mutable accessors below detach
        /// it only for a branch that really writes a city.
        map: Arc<BTreeMap<u32, City>>,
        vision: std::cell::RefCell<Option<Arc<CityVisionStamps>>>,
        /// Bumped alongside every `invalidate()`, so a caller outside this
        /// module can tell "some city was exposed mutably since I last
        /// looked" without paying for a content fold. `Game::suzerain_of`
        /// reads a governor's established city, so the per-ask suzerain-map
        /// and shared-vision-viewer caches key off this in addition to the
        /// roster epoch.
        generation: u64,
    }

    impl Cities {
        /// Drop the folded sight inputs. Every mutable accessor calls this
        /// before it returns, so a caller cannot forget it.
        #[inline]
        fn invalidate(&mut self) {
            *self.vision.get_mut() = None;
            self.generation = self.generation.wrapping_add(1);
        }

        /// See the `generation` field doc: a cheap "did any city change"
        /// signal for caches this module does not otherwise know about.
        #[inline]
        pub(super) fn generation(&self) -> u64 {
            self.generation
        }

        /// The folded city inputs to sight, rebuilt only after a mutation.
        ///
        /// The cached `Arc` is cloned out before the build so the shared
        /// borrow is released first; the fold reads only `self.map` and can
        /// therefore never re-enter this cell.
        pub(super) fn vision_stamps(&self) -> Arc<CityVisionStamps> {
            let cached = self.vision.borrow().clone();
            cached.unwrap_or_else(|| {
                let stamps = Arc::new(CityVisionStamps::build(&self.map));
                *self.vision.borrow_mut() = Some(Arc::clone(&stamps));
                stamps
            })
        }

        /// Whether the folded sight inputs are currently installed. Only the
        /// visibility tests read this; it is how they assert that a border
        /// change actually evicted the memo rather than merely producing a
        /// different answer.
        #[cfg(test)]
        pub(super) fn vision_stamps_cached(&self) -> bool {
            self.vision.borrow().is_some()
        }

        #[inline]
        pub fn get_mut(&mut self, id: &u32) -> Option<&mut City> {
            self.invalidate();
            Arc::make_mut(&mut self.map).get_mut(id)
        }

        pub fn values_mut(&mut self) -> std::collections::btree_map::ValuesMut<'_, u32, City> {
            self.invalidate();
            Arc::make_mut(&mut self.map).values_mut()
        }

        pub fn insert(&mut self, id: u32, city: City) -> Option<City> {
            self.invalidate();
            Arc::make_mut(&mut self.map).insert(id, city)
        }

        pub fn remove(&mut self, id: &u32) -> Option<City> {
            self.invalidate();
            Arc::make_mut(&mut self.map).remove(id)
        }

        pub fn clear(&mut self) {
            self.invalidate();
            Arc::make_mut(&mut self.map).clear();
        }

        pub fn into_values(self) -> std::collections::btree_map::IntoValues<u32, City> {
            match Arc::try_unwrap(self.map) {
                Ok(map) => map.into_values(),
                Err(shared) => shared.as_ref().clone().into_values(),
            }
        }
    }

    impl Clone for Cities {
        /// A clone inherits the memo, which describes exactly the roster being
        /// cloned with it. The two cells are independent from then on, so a
        /// branch that moves a border restamps only its own copy — the same
        /// reasoning `VisionFrameCache::clone` records for the sight frames.
        fn clone(&self) -> Self {
            Self {
                map: Arc::clone(&self.map),
                vision: std::cell::RefCell::new(self.vision.borrow().clone()),
                generation: self.generation,
            }
        }
    }

    impl std::ops::Deref for Cities {
        type Target = BTreeMap<u32, City>;

        fn deref(&self) -> &Self::Target {
            &self.map
        }
    }

    impl<'a> IntoIterator for &'a Cities {
        type Item = (&'a u32, &'a City);
        type IntoIter = std::collections::btree_map::Iter<'a, u32, City>;

        fn into_iter(self) -> Self::IntoIter {
            self.map.iter()
        }
    }

    impl FromIterator<(u32, City)> for Cities {
        fn from_iter<I: IntoIterator<Item = (u32, City)>>(items: I) -> Self {
            Self {
                map: Arc::new(items.into_iter().collect()),
                vision: std::cell::RefCell::new(None),
                generation: 0,
            }
        }
    }
}

pub use city_roster::Cities;

/// An immutable unit roster captured before a speculative branch runs.
#[derive(Clone)]
pub struct UnitSnapshot(Arc<BTreeMap<u32, Arc<Unit>>>);

impl Default for UnitSnapshot {
    fn default() -> Self {
        Self(Arc::new(BTreeMap::new()))
    }
}

impl UnitSnapshot {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, id: &u32) -> Option<&Unit> {
        self.0.get(id).map(Arc::as_ref)
    }
}

/// Unit-only changes made by one speculative branch.  It is intentionally
/// separate from the action log: the log is the authoritative replay, while
/// this compact side channel tells the commit phase which unit ids can be in
/// conflict without diffing the whole world.
#[derive(Clone, Default)]
pub struct UnitDelta {
    base: UnitSnapshot,
    changed: BTreeMap<u32, Option<Arc<Unit>>>,
}

impl UnitDelta {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty()
    }

    pub fn len(&self) -> usize {
        self.changed.len()
    }

    pub fn changed_ids(&self) -> impl Iterator<Item = &u32> {
        self.changed.keys()
    }

    /// Unit ids whose planning snapshot no longer matches the live world.
    /// Only the changed ids are visited, so a city-only or diplomacy-only
    /// action never turns conflict detection into a full-army scan.
    pub fn conflicts_with(&self, current: &Units) -> Vec<u32> {
        self.changed
            .keys()
            .filter(|id| {
                let before = self.base.0.get(id);
                let now = current.0.get(id);
                match (before, now) {
                    (Some(before), Some(now)) => !same_unit_arc(before, now),
                    (None, None) => false,
                    _ => true,
                }
            })
            .copied()
            .collect()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct SpyMission {
    pub kind: String,
    pub city: u32,
    pub target: Pos,
    pub started: u32,
    pub ends: u32,
}

/// Spies are city-based agents rather than map units. `ready_turn` covers
/// travel, establishment, and temporary captivity; active operations live in
/// `mission`, while Gain Sources remains tied to one foreign city.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Spy {
    pub id: u32,
    pub owner: usize,
    pub level: i64,
    #[serde(default)]
    pub promotions: BTreeSet<String>,
    #[serde(default)]
    pub city: Option<u32>,
    #[serde(default)]
    pub ready_turn: u32,
    #[serde(default)]
    pub mission: Option<SpyMission>,
    #[serde(default)]
    pub sources_city: Option<u32>,
    #[serde(default)]
    pub sources_until: u32,
    #[serde(default)]
    pub captured_by: Option<usize>,
}

/// The host's own verdict on a mirrored unit's upgrade — the loose and strict
/// `UnitManager.CanStartCommand(unit, UNITCOMMAND_UPGRADE, …)` reads the
/// shipped `Base/Assets/UI/Panels/UnitPanel.lua:468-483` makes, with
/// `Unit:GetUpgradeCost()` as the bill. Simulator games never fill one.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Default)]
pub struct HostUnitUpgrade {
    /// The successor the host names (`UnitCommandResults.UNIT_TYPE`), as the
    /// CIVVIS unit; `None` when it named a type CIVVIS does not model.
    #[serde(default)]
    pub to: Option<Name>,
    /// The Gold the host would charge, as `GetUpgradeCost()` quotes it.
    #[serde(default)]
    pub cost: Option<f64>,
    /// Complete strategic-resource bill from `GetUpgradeResourceCost()`.
    #[serde(default)]
    pub resources: Option<HostUnitUpgradeResource>,
    /// The first `FAILURE_REASONS` entry when a successor exists and the
    /// command cannot start this turn. `Some` is final: the order would be
    /// refused on arrival.
    #[serde(default)]
    pub blocked: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct HostUnitUpgradeResource {
    /// None only when the host positively reports no material and a zero bill.
    pub resource: Option<Name>,
    pub cost: f64,
}

/// What an authoritative host said about one of the mirrored seat's units
/// that the board used to derive from its own rules (docs/FIDELITY.md, "The
/// one-to-one map", item 9). Keyed by CIVVIS unit id; empty in a native game,
/// and every reader falls back to the board's own rule when its key is absent.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Default)]
pub struct HostUnitFacts {
    /// Stable Civilization VI unit id, when these facts came from the live
    /// mirror. The board id is reassigned when the live bridge rebuilds its
    /// planning board, so bounded host observations must key themselves by
    /// this identity instead of by the transient CIVVIS id.
    #[serde(default)]
    pub civ6_id: Option<i64>,
    /// Firaxis Rock Band activation highlights, in axial coordinates. An empty
    /// set means no host-legal concert destination; None keeps model behavior
    /// for headless games and older or unreadable exports.
    #[serde(default)]
    pub concert_plots: Option<BTreeSet<Pos>>,
    /// `GetRockBand():GetRockBandLevel()` from UnitPanel_Expansion2.lua:42.
    /// Generic experience level remains the promotion-entitlement reading.
    #[serde(default)]
    pub rock_band_level: Option<i32>,
    #[serde(default)]
    pub upgrade: Option<HostUnitUpgrade>,
    /// `UnitManager.GetUnitMaintenance` (or the Corps/Army accessor) for the
    /// unit's type, before the player's per-unit discount —
    /// `Screens/ReportScreen.lua:314-338`.
    #[serde(default)]
    pub maintenance: Option<f64>,
    /// `Unit:GetReligiousStrength()`, the panel's own stat (`UnitPanel.lua:2257`).
    /// Carried for diagnostics: like the exported `combat`, it is the unit's
    /// displayed figure, and the board's pressure model stacks its own
    /// situational terms on the ruleset base rather than on this.
    #[serde(default)]
    pub religious_strength: Option<f64>,
    /// `Unit:GetMaxMoves()` (`UnitPanel.lua:2242`): the fresh-turn allowance
    /// with every host effect applied.
    #[serde(default)]
    pub max_moves: Option<f64>,
    /// `UnitManager.GetActivityType` as the shipped `WorldTracker.lua:544`
    /// names it: `sleep`, `hold`, `operation` or `awake`.
    #[serde(default)]
    pub activity: Option<String>,
    /// A Spy's running operation (`Unit:GetSpyOperation()`,
    /// `EspionageOverview.lua:659`) as the CIVVIS mission kind, and the turn
    /// it ends (`GetSpyOperationEndTurn`).
    #[serde(default)]
    pub spy_operation: Option<String>,
    #[serde(default)]
    pub spy_operation_ends: Option<u32>,
    /// The operations the host would let an idle Spy start from the city it
    /// stands in (`EspionageChooser.lua:196-213`), as CIVVIS mission kinds.
    /// `Some` only with entries; a Spy on an operation or outside a city has
    /// none and the board's menu stands.
    #[serde(default)]
    pub spy_missions: Option<BTreeSet<String>>,
    /// Beside `spy_missions`: each offered mission's target plot (the
    /// district the host aims it at, else the city) and the host's odds,
    /// keyed by CIVVIS mission kind. `Some` only with entries.
    #[serde(default)]
    pub spy_targets: Option<BTreeMap<String, HostSpyTarget>>,
    /// A Spy's promotion menu as the host offers it: the stock promotion
    /// popup's `CanStartCommand(unit, PROMOTE, true, true)` list
    /// (`UnitPromotionPopup.lua:81-82`), as CIVVIS promotion names. `Some`
    /// and empty is an observed lack of choices; `None` is no reading. A
    /// levelled Spy reads host level 1 until its choice is made, so this menu
    /// is the only witness that a promotion is owed.
    #[serde(default)]
    pub spy_promotions: Option<BTreeSet<Name>>,
}

/// One host-offered Spy mission: the plot it aims at and the host's odds
/// (`UnitManager.GetResultProbability`: `success` = the two
/// `ESPIONAGE_SUCCESS_*` rows, `fail` = the two `ESPIONAGE_FAIL_*` rows; the
/// rest is capture or death). `None` odds when the host gave none.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug, Default)]
pub struct HostSpyTarget {
    pub pos: Pos,
    #[serde(default)]
    pub success: Option<f64>,
    #[serde(default)]
    pub fail: Option<f64>,
}

/// The treasury's Gold bill by source — `PlayerTreasury:GetUnitMaintenance`,
/// `GetBuildingMaintenance` and `GetDistrictMaintenance`, the top panel's own
/// breakdown (`ToolTipHelper_PlayerYields.lua:22-26`). Keyed by seat on
/// [`Game::host_maintenance`]; a native game has none and bills itself.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug, Default)]
pub struct HostMaintenance {
    #[serde(default)]
    pub units: Option<f64>,
    #[serde(default)]
    pub buildings: Option<f64>,
    #[serde(default)]
    pub districts: Option<f64>,
}

/// The four location classes used for passive unit healing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealingLocation {
    District,
    FriendlyTerritory,
    NeutralTerritory,
    EnemyTerritory,
}

impl HealingLocation {
    pub fn rate(self) -> i32 {
        match self {
            HealingLocation::District => 20,
            HealingLocation::FriendlyTerritory => 15,
            HealingLocation::NeutralTerritory => 10,
            HealingLocation::EnemyTerritory => 5,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum Item {
    Formation {
        unit: Name,
        /// 1 = Corps/Fleet, 2 = Army/Armada.
        formation: u8,
    },
    Unit {
        unit: Name,
    },
    Building {
        building: Name,
    },
    District {
        district: Name,
        pos: Pos,
    },
    Wonder {
        wonder: Name,
        pos: Pos,
    },
    /// Restores either a pillaged district base (`repair = "district"`) or
    /// one pillaged building on that district tile.
    Repair {
        repair: Name,
        pos: Pos,
    },
    Project {
        project: Name,
    },
    /// Monopolies & Corporations city project. The resource identifies the
    /// Corporation brand; each Corporation can manufacture five Products.
    Product {
        product: Name,
    },
}

/// [`Item`], reduced to exactly what [`Game::production_block_key`] hashes
/// into a string — the `pos` an `Item::District`, `Item::Wonder` or
/// `Item::Repair` carries never enters the key. `Name` is `Copy`, so this
/// type is too: building one, comparing two, or hashing one touches no
/// allocator, unlike the `String` `production_block_key` used to be the only
/// way to ask any of those questions.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) enum ProductionKey {
    Formation(Name, u8),
    Unit(Name),
    Building(Name),
    District(Name),
    Wonder(Name),
    Repair(Name),
    Project(Name),
    Product(Name),
}

impl ProductionKey {
    /// The `String` form external maps fed by the live mirror are keyed by
    /// (`blocked_production`, `blocked_purchases`, `host_buildable`,
    /// `host_purchasable`) — byte-identical to the old `production_block_key`
    /// output. Allocate only at that boundary; an internal comparison should
    /// use the key itself instead of calling this.
    fn to_block_string(self) -> String {
        match self {
            ProductionKey::Formation(unit, formation) => format!("formation:{unit}:{formation}"),
            ProductionKey::Unit(unit) => format!("unit:{unit}"),
            ProductionKey::Building(building) => format!("building:{building}"),
            ProductionKey::District(district) => format!("district:{district}"),
            ProductionKey::Wonder(wonder) => format!("wonder:{wonder}"),
            ProductionKey::Repair(repair) => format!("repair:{repair}"),
            ProductionKey::Project(project) => format!("project:{project}"),
            ProductionKey::Product(product) => format!("product:{product}"),
        }
    }
}

/// One row of the host's production menu for a city: what the engine says the
/// item costs here and how many turns it takes, from
/// `BuildQueue:GetXCost(row.Index)` and `GetTurnsLeft(hash)`. `None` where the
/// accessor did not answer. See [`Game::host_buildable`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostMenuEntry {
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub turns: Option<f64>,
}

/// One row of the host's purchase menu: the engine's own price in each
/// currency, `None` where it will not sell for that currency. See
/// [`Game::host_purchasable`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostPurchaseEntry {
    #[serde(default)]
    pub gold: Option<f64>,
    #[serde(default)]
    pub faith: Option<f64>,
}

/// A native, currently legal plot purchase and the host's Acropolis-site test.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostPlotPurchaseEntry {
    pub pos: Pos,
    pub gold: f64,
    #[serde(default)]
    pub can_have_acropolis: Option<bool>,
}

/// District placements owned by a city.
///
/// Older saves encoded this as `{ "campus": [q, r] }`, which made it
/// impossible to represent Civ VI's repeatable Neighborhood and Canal
/// districts.  The in-memory representation stores every placement while the
/// custom deserializer accepts both that legacy shape and the new
/// `{ "neighborhood": [[q1, r1], [q2, r2]] }` shape.
#[derive(Clone, Default)]
pub struct Districts(BTreeMap<Name, Vec<Pos>>);

impl Districts {
    pub fn contains_key(&self, name: impl AsName) -> bool {
        self.0
            .get(&name.as_name())
            .is_some_and(|positions| !positions.is_empty())
    }

    /// Total district instances, rather than the number of distinct types.
    pub fn len(&self) -> usize {
        self.0.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn remove(&mut self, name: impl AsName) -> Option<Vec<Pos>> {
        self.0.remove(&name.as_name())
    }

    pub fn get(&self, name: impl AsName) -> Option<&Pos> {
        self.0
            .get(&name.as_name())
            .and_then(|positions| positions.first())
    }

    pub fn positions(&self, name: impl AsName) -> &[Pos] {
        self.0
            .get(&name.as_name())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn insert(&mut self, name: Name, pos: Pos) {
        self.0.entry(name).or_default().push(pos);
    }

    pub fn keys(&self) -> impl Iterator<Item = &Name> {
        self.0
            .iter()
            .flat_map(|(name, positions)| std::iter::repeat_n(name, positions.len()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Name, &Pos)> {
        self.0
            .iter()
            .flat_map(|(name, positions)| positions.iter().map(move |pos| (name, pos)))
    }
}

impl<'a> IntoIterator for &'a Districts {
    type Item = (&'a Name, &'a Pos);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter().collect::<Vec<_>>().into_iter()
    }
}

impl Serialize for Districts {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, positions) in &self.0 {
            if positions.len() == 1 {
                map.serialize_entry(name, &positions[0])?;
            } else {
                map.serialize_entry(name, positions)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Districts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum OneOrMany {
            One(Pos),
            Many(Vec<Pos>),
        }

        let encoded = BTreeMap::<String, OneOrMany>::deserialize(deserializer)?;
        Ok(Self(
            encoded
                .into_iter()
                .map(|(name, positions)| {
                    let positions = match positions {
                        OneOrMany::One(pos) => vec![pos],
                        OneOrMany::Many(positions) => positions,
                    };
                    (Name::new(&name), positions)
                })
                .collect(),
        ))
    }
}

/// A city's housing, split into the categories Civilization VI itself reports.
///
/// The field names are the host's, minus the `housing_from_` prefix, so a
/// comparison against a `state` export is a field-by-field read rather than a
/// mapping somebody has to keep in their head. `great_people`, `great_works`
/// and `starting_era` exist here with no CIVVIS term behind them on purpose:
/// the host fills them, and a category we cannot produce at all should read as
/// a zero we can see rather than as an absence nobody notices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HousingSources {
    pub water: f64,
    pub buildings: f64,
    pub districts: f64,
    pub improvements: f64,
    pub civics: f64,
    pub great_people: f64,
    pub great_works: f64,
    pub starting_era: f64,
    /// CIVVIS terms with no host counterpart: resource industries, wall levels.
    pub other: f64,
}

impl HousingSources {
    pub fn total(&self) -> f64 {
        self.water
            + self.buildings
            + self.districts
            + self.improvements
            + self.civics
            + self.great_people
            + self.great_works
            + self.starting_era
            + self.other
    }

    /// `(name, value)` for every category, in the host's own reporting order.
    pub fn named(&self) -> [(&'static str, f64); 9] {
        [
            ("water", self.water),
            ("buildings", self.buildings),
            ("districts", self.districts),
            ("improvements", self.improvements),
            ("civics", self.civics),
            ("great_people", self.great_people),
            ("great_works", self.great_works),
            ("starting_era", self.starting_era),
            ("other", self.other),
        ]
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct City {
    pub id: u32,
    pub name: String,
    pub owner: usize,
    pub pos: Pos,
    pub pop: i32,
    pub food: f64,
    pub production: f64,
    /// Banked progress for paused builds. `production` is the active build's
    /// progress (or unassigned overflow when the queue is empty).
    #[serde(default)]
    pub production_progress: BTreeMap<String, f64>,
    /// Strategic material already committed to paused or active unit builds,
    /// keyed by `item_progress_key`. This prevents production switching from
    /// charging twice while still charging when a build begins, as Civ VI does.
    #[serde(default)]
    pub strategic_resource_commitments: BTreeMap<String, f64>,
    pub border_culture: f64,
    pub hp: i32,
    pub buildings: Vec<Name>,
    /// Economic Great Works housed in Stock Exchange/Seaport Product slots.
    /// The resource ID is also the Product's brand/effect identity.
    #[serde(default)]
    pub products: Vec<String>,
    /// World era in which each building was completed. Era-scaling effects
    /// such as the Dar-e Mehr use this while old saves safely default to no
    /// retroactive age bonus.
    #[serde(default)]
    pub building_eras: BTreeMap<Name, usize>,
    #[serde(default)]
    pub pillaged_buildings: BTreeSet<Name>,
    #[serde(default)]
    pub districts: Districts,
    #[serde(default)]
    pub wonders: BTreeMap<Name, Pos>,
    pub owned_tiles: Vec<Pos>,
    pub queue: Vec<Item>,
    pub original_owner: usize,
    pub is_capital: bool,
    #[serde(default)]
    pub struck: bool,
    /// Additional city-center strikes consumed after the normal strike.
    #[serde(default)]
    pub extra_strikes_used: i32,
    /// Outer-defense pool from walls (Civ 6); -1 in old saves = derive on load.
    #[serde(default = "wall_unset")]
    pub wall_hp: i32,
    /// Encampments are independent defensible districts with their own 100 HP,
    /// wall pool, ranged strike, pillage state, and attack timer.
    #[serde(default)]
    pub encampment_hp: i32,
    #[serde(default = "wall_unset")]
    pub encampment_wall_hp: i32,
    #[serde(default)]
    pub encampment_struck: bool,
    /// Additional Encampment strikes consumed after the normal strike.
    #[serde(default)]
    pub encampment_extra_strikes_used: i32,
    #[serde(default)]
    pub encampment_last_attacked: u32,
    #[serde(default)]
    pub encampment_pillaged: bool,
    /// Independent state for defending districts outside the Encampment family.
    #[serde(default)]
    pub defending_districts: Vec<DefendingDistrict>,
    #[serde(default)]
    pub last_attacked: u32,
    #[serde(default)]
    pub pressure: BTreeMap<String, f64>, // religious pressure by religion
    /// Atheist/pantheon pressure. Civ VI counts it alongside the religions
    /// when dividing Citizens, so it is the reason a city with a handful of
    /// foreign Pressure does not immediately adopt that faith. It gains 50 per
    /// growth while the city has no majority religion and is never removed.
    #[serde(default = "starting_atheist_pressure")]
    pub atheist_pressure: f64,
    #[serde(default = "full_loyalty")]
    pub loyalty: f64,
    /// Loyalty pressure accumulated while this settlement is a Free City.
    /// The ledger is cleared whenever it joins a major civilization.
    #[serde(default)]
    pub free_city_pressure: BTreeMap<usize, f64>,
    /// The owner that lost this city in the unresolved conquest which just
    /// occurred. While set, the conqueror must keep, raze, or (when legal)
    /// liberate the city before taking another action.
    #[serde(default)]
    pub captured_from: Option<usize>,
    /// The faith the host names as this city's next majority, whatever its
    /// turn estimate (`religion_next`). The mirror keeps it raw so a founder
    /// can see a rival faith closing on its Holy City long before the host's
    /// conversion clock reads short. `None` on a native board.
    #[serde(default)]
    pub religion_next_seen: Option<String>,
    /// The civilization from which this city was taken in war. Unlike
    /// `captured_from`, this survives the mandatory keep decision: an
    /// occupied city keeps its -5 Loyalty pressure until the former owner is
    /// eliminated or the city is ceded/returned.
    #[serde(default)]
    pub occupied_from: Option<usize>,
    /// The population- and casus-adjusted grievance incurred when this city
    /// was occupied. A peace settlement charges the same amount again if the
    /// current owner keeps it, while returning it clears that first charge.
    /// Older saves predate the split accounting and safely omit this value.
    #[serde(default)]
    pub occupation_grievance: Option<f64>,
    /// Turns elapsed since this city's active Nuclear Power Plant was built or
    /// last recommissioned.
    #[serde(default)]
    pub reactor_age: u32,
    /// Gold granted to both ends of each foreign route terminating here by
    /// city-bound Great Person effects such as Marco Polo and Zheng He.
    #[serde(default)]
    pub great_person_foreign_route_gold: f64,
}

/// Public city information frozen at the moment a player last had sight of
/// the City Center. Private production and citizen state are never copied.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct RememberedCity {
    pub id: u32,
    pub name: String,
    pub owner: usize,
    pub pos: Pos,
    pub pop: i32,
    pub hp: i32,
    pub is_capital: bool,
    pub original_owner: usize,
    pub captured_from: Option<usize>,
    pub occupied_from: Option<usize>,
    pub wall_hp: i32,
    pub wall_max: i32,
    pub encampment_hp: i32,
    pub encampment_wall_hp: i32,
    pub encampment_pillaged: bool,
    /// Independent state for defending districts outside the Encampment family.
    #[serde(default)]
    pub defending_districts: Vec<DefendingDistrict>,
    pub religion: Option<String>,
    #[serde(default)]
    pub seen_turn: u32,
}

/// What one city is locally best used for, independent of what the empire is
/// trying to win. The two axes are deliberately separate: `CityRole` is the
/// local optimization and [`CityDirective::emphasis`] is the empire's
/// objective, and a directive carries both so neither silently overrides the
/// other.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CityRole {
    /// No local specialization worth naming — the governor's own evidence
    /// (districts, buildings, the item in the queue) decides alone.
    #[default]
    Balanced,
    /// The empire's workshop: the city that pays for armies, wonders and
    /// districts. Wants hammers over its own growth.
    Forge,
    /// The settler pump. Wants food while the empire is still short of its
    /// city target, because capital growth is what gates a settler.
    Breadbasket,
    /// A city whose specialty districts already earn more than its tiles do.
    /// Leans further into the yield it is already best at rather than
    /// spreading thin.
    Specialist,
    /// A frontline city under real local pressure. Wants defenses now and
    /// will not grow into a siege it cannot feed.
    Bastion,
}

impl CityRole {
    pub fn as_str(self) -> &'static str {
        match self {
            CityRole::Balanced => "balanced",
            CityRole::Forge => "forge",
            CityRole::Breadbasket => "breadbasket",
            CityRole::Specialist => "specialist",
            CityRole::Bastion => "bastion",
        }
    }
}

/// What an empire's plan asks of one of its cities this turn.
///
/// [`Player::citizen_food_bias`] is one scalar for a whole seat, so every city
/// hears the same sentence: it cannot say *this* city feeds the war and *that*
/// one races to Mars. A directive is the per-city channel. It is written by an
/// agent and never by the engine, and it is folded into
/// [`Game::citizen_strategy`] **additively** — the local evidence already
/// there keeps its full weight and the directive tilts the margin, so a plan
/// can never talk a city into starving itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CityDirective {
    /// The empire's objective expressed in the governor's own vocabulary:
    /// extra appetite per yield, added to the shipped weights.
    pub emphasis: Yields,
    /// What this city is locally for.
    pub role: CityRole,
    /// Local military pressure — hostile military strength within six tiles
    /// over the friendly strength answering it, so 0.0 is a safe rear city and
    /// 1.0 is one whose besiegers already match its defenders. Empire-wide
    /// `at_war` cannot tell those apart; this is what makes the governor
    /// militarily aware of *where* the city stands.
    pub pressure: f64,
    /// Stop asking for a growth surplus and cut the food appetite.
    ///
    /// ⚠ **Measured harmful, and it is the whole reason the first three
    /// city-strategy arms lost.** It is kept as an expressible stance rather
    /// than deleted because the engine's job is to obey a directive, not to
    /// hold an opinion about it — but nothing should set it. Isolated over 120
    /// paired maps it scores 43.8%, Elo-equivalent −44 at sign p=0.0107, and
    /// it costs the empire a fifth of its cities (2.17 against 2.74). Wanting
    /// hammers under threat is free-to-positive; *trading away growth* to get
    /// them is not, and a besieged city that stops growing does not become
    /// safer — it becomes permanently smaller.
    pub halt_growth: bool,
}

/// The priorities used by a city's automatic citizen governor.  These are
/// deliberately observable: agents and the browser can explain why a tile is
/// being worked instead of treating city yields as a hidden heuristic.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CitizenStrategy {
    pub focus: String,
    pub weights: Yields,
    /// Total food the governor tries to collect, including the city center.
    /// It always covers consumption when the owned tiles make that possible;
    /// surplus is requested only when housing and amenities support growth.
    pub food_target: f64,
}

/// The tile-level ledger behind a city's modelled yields; see
/// [`Game::city_yield_ledger`].
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CityYieldLedger {
    /// The city centre as CIVVIS floors it before assigning citizens.
    pub center: Yields,
    /// Every worked tile and what CIVVIS's own tile model pays this city for
    /// it, before any host correction.
    pub tiles: Vec<(Pos, Yields)>,
    /// The host-to-model correction on the centre and each worked tile where
    /// the mirror derived one (`observed_tile_yield_adjustments`); empty on a
    /// native game.
    pub tile_adjustments: Vec<(Pos, Yields)>,
    /// Every assigned specialist, by district family, and what its slot pays.
    pub specialists: Vec<(String, Yields)>,
    /// Every standing district's own yields (adjacency and base), by name and
    /// position.
    pub districts: Vec<(String, Pos, Yields)>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CitizenPlan {
    pub strategy: CitizenStrategy,
    /// Tiles worked by population. The free city-center tile is not included.
    pub worked_tiles: Vec<Pos>,
    /// One district-family ID per citizen assigned to a specialist slot.
    pub specialists: Vec<String>,
}

/// One line of a district's adjacency ledger.  A district's total is the sum
/// of these, so the interface can show a player *why* a Campus is worth four
/// science instead of only that it is.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AdjacencySource {
    /// The ruleset key that paid — `mountain`, `district`, `government_plaza`
    /// — or a modifier's own name (`gaul_mine`, `adjacency_bonus`).
    pub source: String,
    /// Neighboring tiles of this kind.  Zero for a percentage modifier, which
    /// scales the rest rather than counting anything.
    pub count: usize,
    /// Percentage a modifier line adds to everything above it; 0 otherwise.
    pub percent: f64,
    /// What the line pays after Civ VI's per-source rounding.
    pub yields: Yields,
    /// The same figure before rounding.  Half-point sources bank their
    /// remainder, and a player deciding where to build wants to see it.
    pub raw: Yields,
}

fn full_loyalty() -> f64 {
    100.0
}

/// A city's first Citizen contributes 50 atheist/pantheon Pressure.
fn starting_atheist_pressure() -> f64 {
    50.0
}

fn wall_unset() -> i32 {
    -1
}

fn normal_age() -> String {
    "normal".to_string()
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernorState {
    pub city: Option<u32>,
    pub assigned_turn: u32,
    /// Neutralize Governor suppresses every local effect through this turn.
    #[serde(default)]
    pub disabled_until: u32,
    #[serde(default)]
    pub promotions: BTreeSet<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct AllianceState {
    pub kind: String,
    pub points: f64,
    pub level: i32,
    pub ends: u32,
}

/// A bilateral diplomatic presence sent by its owner to one other leader.
///
/// Delegations and Resident Embassies are deliberately relationship state,
/// rather than a one-turn event: their visibility and attitude effects remain
/// until the relationship is broken by war.  An Embassy replaces a Delegation
/// for the same recipient, so there is at most one entry per counterpart.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct DiplomaticMission {
    /// `delegation` or `embassy`.
    pub kind: String,
    /// Lets an observation distinguish a newly established mission from one
    /// restored out of an older save without treating it as a timer.
    pub sent: u32,
}

/// One actionable conduct incident between two major civilizations.
///
/// Civ VI's Discuss panel only exposes a promise after the recipient has
/// already performed the matching action. `requestable_at` also gives a
/// refused or expired request its normal thirty-turn cadence without losing
/// the fact that the underlying conduct occurred.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct DiplomaticIncident {
    pub occurred: u32,
    #[serde(default)]
    pub requestable_at: u32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct DiplomaticDeal {
    pub id: u32,
    pub from: usize,
    pub to: usize,
    pub give_gold: f64,
    pub request_gold: f64,
    pub open_borders: bool,
    pub friendship: bool,
    pub peace: bool,
    pub alliance: Option<String>,
    /// A Mobilization-era defensive pact proposal.  It uses the ordinary
    /// proposal/accept/reject lifecycle so neither party is silently bound.
    #[serde(default)]
    pub defensive_pact: bool,
    /// The third party a Foreign Trade joint-war offer would immediately
    /// declare against when the invited partner accepts.
    #[serde(default)]
    pub joint_war_target: Option<usize>,
    /// A requested diplomatic promise, normalized to one of the stable
    /// engine IDs (`no_settling`, `no_conversion`, `no_spying`, or
    /// `no_city_state_attack`).
    #[serde(default)]
    pub promise: Option<String>,
    /// Unlike an ordinary trade, a demand need not be mutually valuable.
    /// Refusal is a diplomatic event and therefore carries a grievance.
    #[serde(default)]
    pub demand: bool,
    pub expires: u32,
}

/// One civilization's side of a Civ VI-style trade. Lump Gold, Favor, and
/// strategic stockpile quantities, Great Works, captured Spies, and cities
/// transfer immediately; Gold per turn, luxury copies, and Open Borders last
/// for the standard 30-turn agreement.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug)]
pub struct DealItems {
    #[serde(default)]
    pub gold: f64,
    #[serde(default)]
    pub gold_per_turn: f64,
    #[serde(default)]
    pub diplomatic_favor: f64,
    #[serde(default)]
    pub resources: BTreeMap<String, i32>,
    #[serde(default)]
    pub great_works: BTreeMap<String, i32>,
    /// Captured enemy agents returned to their original civilization.
    /// Captives remain in custody until a deal explicitly transfers them.
    #[serde(default)]
    pub captured_spies: Vec<u32>,
    /// Non-capital cities transferred permanently when the trade closes.
    #[serde(default)]
    pub cities: Vec<u32>,
    #[serde(default)]
    pub open_borders: bool,
}

impl DealItems {
    pub fn is_empty(&self) -> bool {
        self.gold == 0.0
            && self.gold_per_turn == 0.0
            && self.diplomatic_favor == 0.0
            && self.resources.values().all(|amount| *amount == 0)
            && self.great_works.values().all(|amount| *amount == 0)
            && self.captured_spies.is_empty()
            && self.cities.is_empty()
            && !self.open_borders
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct ActiveTradeDeal {
    pub id: u32,
    pub from: usize,
    pub to: usize,
    pub offer: DealItems,
    pub request: DealItems,
    pub started: u32,
    pub ends: u32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct CongressResolution {
    pub id: String,
    pub title: String,
    pub choices: Vec<String>,
    #[serde(default)]
    pub ballots: BTreeMap<usize, (String, u32)>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct CongressSession {
    pub convened: u32,
    pub closes: u32,
    pub resolutions: Vec<CongressResolution>,
}

/// A passed regular-session resolution remains binding until the next World
/// Congress convenes. `outcome` is `A` or `B`; `target` is the selected
/// civilization, resource, district, unit class, or other proposal target.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct CongressEffect {
    pub resolution: String,
    pub outcome: String,
    pub target: String,
    pub expires: u32,
}

/// A city-capture Emergency eligible for a World Congress Special Session.
/// The proposal remains queued while another session is voting, so a valid
/// trigger cannot silently disappear merely because it happened on the same
/// turn as the regular Congress.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct EmergencyProposal {
    pub id: u32,
    pub kind: String,
    pub target: usize,
    pub city: u32,
    pub original_owner: usize,
    pub eligible: BTreeSet<usize>,
    pub requested: u32,
}

/// A passed Military or City-State Emergency. Contributions are deterministic
/// participation points earned by maintaining military units in the target's
/// territory, attacking or killing its units, and liberating the objective.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Emergency {
    pub id: u32,
    pub kind: String,
    pub target: usize,
    pub city: u32,
    pub original_owner: usize,
    pub members: BTreeSet<usize>,
    pub contributions: BTreeMap<usize, i64>,
    pub started: u32,
    pub ends: u32,
}

/// A scored World Congress competition observed from the live Civilization VI
/// host. It remains separate from [`Emergency`], which models CIVVIS's native
/// city-capture emergencies rather than a host-granted production opportunity.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct HostCompetition {
    /// Firaxis emergency type, for example `EMERGENCY_WORLD_GAMES`.
    pub kind: String,
    /// First CIVVIS turn on which the host opportunity is no longer current.
    pub ends: u32,
    /// The mirrored seat's current competition score.
    pub ours: f64,
    /// The highest score any member currently holds, including ours.
    pub leader: f64,
}

/// One shipped `EmergencyScoreSources` row, in the vocabulary CIVVIS can
/// evaluate.
///
/// Each variant is a row rather than a category: the number is that row's
/// `ScoreAmount`, and the cadence is the one its `Description` states.
/// `LOC_EMERGENCY_SCORE_GPP_DESC` is "Generating Great People Points **Per
/// Turn**" and `LOC_EMERGENCY_SCORE_SPACEPORTS_DESC` is "**Maintaining**
/// Spaceport Districts", so both accrue every turn; a `FromProject` row pays
/// once, when the project completes.
///
/// ⚠ Three shipped rows stay unmodelled because the data does not say what they
/// measure. `CLIMATE_ACCORDS_SCORE_CO2` pays for "emissions much lower than the
/// biggest CO2 polluter" and never says how much lower; and the two
/// `FROM_AT_WAR` penalties do not say whether -30
/// and -200 are charged once or every turn. Choosing a number for any of them
/// would be inventing a rule, which is the #2049 mistake.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CompetitionScoreSource {
    /// Expansion2_Emergencies.xml:131,166: one point per Gold gifted to
    /// the Aid Request's target. Only completed lump-sum gifts are modeled;
    /// recurring deal payments have no verified scoring cadence here.
    GoldGift,
    /// `FromProject`: a project completed in a city, worth the
    /// `competition_score` it declares.
    Project,
    /// The eight `WORLDS_FAIR_SCORE_GPP_*` rows, `ScoreAmount="1"` for every
    /// class: one point per Great Person Point generated, each turn.
    GreatPersonPointsPerTurn,
    /// `NOBEL_PRIZE_PEACE_SCORE_FROM_FAVOR`, `FromFavor="true"`
    /// `ScoreAmount="1"`: one point per Diplomatic Favor generated, each turn.
    DiplomaticFavorPerTurn,
    /// `FromDistrict`: `amount` each turn for every city holding the district.
    DistrictPerTurn { district: &'static str, amount: f64 },
    /// `FromBuilding`: `amount` each turn for every city holding the building.
    BuildingPerTurn { building: &'static str, amount: f64 },
}

/// The event that seats a native scored competition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NativeCompetitionTrigger {
    /// The regular World Congress selects a scored competition.
    Congress,
    /// A player lost population to a random disaster.
    RandomDisasterPopulationLoss,
    /// A civilization the target already has 200 grievances against declares war.
    WarWithGrievances,
}

/// The portion of a scored competition CIVVIS can faithfully seat itself.
///
/// The World Congress trigger is shared, but each emergency's requirements
/// constrain its era. Keeping that source data beside its DVP award prevents a
/// project prerequisite from accidentally deciding when the congress may offer
/// the competition.
#[derive(Clone, Copy, PartialEq, Debug)]
struct NativeCompetitionSpec {
    kind: &'static str,
    diplomatic_victory_points: i64,
    /// The Diplomatic Favor first place takes beside the point.
    ///
    /// ⚠ This is the emergency's **top tier** award, not a separate first-place
    /// one, and first place is read as being in that tier: the shipped strings
    /// name the bands "Gold Tier Rewards (for highest score)", "Silver Tier
    /// Rewards (for the top 25% of scores)" and "Bronze Tier Rewards (for the
    /// next 25% of scores)", and `LOC_SCORED_COMPETITION_SILVER_TIER_CHILD`
    /// exists to say "All Silver Tier Rewards". The highest score is in the top
    /// 25% of scores, so the winner takes both.
    ///
    /// ⚠ That is a **reading**: the three band predicates
    /// (`REQUIREMENT_PLAYER_GOT_FIRST_PLACE_IN_EMERGENCY`, `..._HIGH_TIER_...`,
    /// `..._LOW_TIER_...`) are compiled into the engine, and the child string is
    /// referenced by no shipped Lua. The two lower bands are not paid at all,
    /// because where a 25% cut falls among five or six scoring empires is a
    /// rounding rule the data does not state.
    first_place_favor: f64,
    scoring: &'static [CompetitionScoreSource],
    trigger: NativeCompetitionTrigger,
    duration: u32,
    lockout: u32,
    minimum_world_era: usize,
    maximum_world_era: Option<usize>,
    /// A civilization ability the game must contain before this competition
    /// exists at all — the emergency's
    /// `REQUIREMENT_GAME_HAS_CIVILIZATION_OR_LEADER_TRAIT`.
    required_civilization_ability: Option<&'static str>,
}

impl NativeCompetitionSpec {
    fn offered_in_world_era(self, era: usize) -> bool {
        era >= self.minimum_world_era && self.maximum_world_era.is_none_or(|maximum| era <= maximum)
    }

    fn counts(self, source: CompetitionScoreSource) -> bool {
        self.scoring.contains(&source)
    }
}

/// A scored competition CIVVIS runs itself, rather than observing on a host.
///
/// ★★★★★ THE HALF OF THE DIPLOMATIC LANE A NATIVE GAME NEVER HAD. Gathering
/// Storm pays a Diplomatic Victory Point to the first-place finisher of a
/// scored competition, and they recur for the whole second half of a game.
/// Live, 19.6% of terminal games end in a rival's diplomatic victory; on the
/// contested native screen CIVVIS produced 1.7%.
///
/// ⚠ **"Without them a native empire has no route to twenty" is what this
/// comment said, and #2379's census says otherwise.** At the standard screen's
/// shape — 6 majors, 74×46 continents, nine city-states, Online, 250 turns,
/// 150 paired seeds an arm from 41000 — the leading empire finishes on a
/// **median of 13** of the 20 with competitions off, and 3 of 150 games crossed
/// the line without one; the congress resolution at ±2 from the Modern era does
/// most of that. Turning all seven competitions on is worth **+0.35 points to
/// the leader per game (95% CI +0.05 to +0.64)** and moved the diplomatic
/// ending share from 3/150 to 0/150. The lane is short by about a third, not
/// unreachable, and this closes a fraction of the gap rather than the whole of
/// it — which is why the flag ships off. See `docs/FIDELITY.md`.
///
/// Unlike [`HostCompetition`], which is one mirrored seat's view of a host's
/// competition, this holds every seat's score because CIVVIS is running it.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Competition {
    /// Firaxis emergency type, so a seat cannot tell this from a host one.
    pub kind: String,
    /// First turn on which the competition is no longer current.
    pub ends: u32,
    /// The player a targeted aid request is helping, if it has one.
    ///
    /// A target is not an emergency member and therefore cannot access the
    /// request's scoring project. Older saves held only no-target congress
    /// competitions, so they deserialize as no target.
    #[serde(default)]
    pub target: Option<usize>,
    /// Score by player id. A seat that has scored nothing is absent.
    pub scores: BTreeMap<usize, f64>,
}

/// Every blow that has landed on a city this game.
///
/// A war ledger says who declared and who lost units; it cannot say whether an
/// army ever reached a city at all. Measured over 12 full-length six-player
/// games the AI chooses Conquest about 26% of its turns and its forces engage
/// about 22% of theirs, yet roughly 0.3 cities fall per game and no capital has
/// ever fallen — so the question is which link between engaging and capturing
/// is missing, and only the city-damage funnel can answer it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiegeCensus {
    /// Times a city took damage from anyone.
    pub blows: u64,
    /// Total health struck off cities, walls excluded.
    pub damage: i64,
    /// Times a wall was knocked from standing to flat.
    pub walls_breached: u64,
    /// Times damage took a city's garrison to zero *inside*
    /// [`Game::city_take_damage`]. This is not the same as the city being left
    /// depleted: only a Bombard-class shot may leave a garrison at zero, and an
    /// ordinary ranged attack has its caller restore it to 1 immediately
    /// afterwards, so this overcounts by every reverted crossing. Use
    /// [`SiegeCensus::left_depleted`] for cities that were actually left open.
    pub cities_reduced: u64,
    /// Times a city was actually left standing at zero garrison, which is the
    /// state a melee unit can walk into. A barbarian never captures and leaves
    /// the city at 1 instead.
    pub left_depleted: u64,
    /// Of those, how many had a melee-capable unit of the depleting side already
    /// adjacent **with movement remaining** — that is, a capture that was
    /// available that same turn and simply was not taken.
    ///
    /// This is the count that separates the two remaining explanations. Doubling
    /// the number of cities left depleted cut captures by 61%, so opening cities
    /// is not what is missing; either the AI declines captures it could make, or
    /// the piece that could make them is always already spent.
    pub depleted_with_a_taker_ready: u64,
    /// Of those reductions, how many happened with a melee-capable unit of the
    /// attacking side already standing next to the city, so occupying it was
    /// available that turn.
    ///
    /// This is the discriminating count. Only melee can walk into a city, and
    /// an unoccupied city at zero health heals, so 663 reductions against 23
    /// captures has exactly two explanations: the AI declines a capture it
    /// could make, or it never has anyone there to make it. Attack valuation
    /// already pays 520+ for a capture and only vetoes one that loyalty would
    /// flip back within four turns, which points at the second — but pointing
    /// is not measuring.
    pub reduced_with_melee_adjacent: u64,
}

/// What one belligerent has had taken from it in a war — never what it
/// inflicted, so the two sides of a war read as two columns of the same
/// ledger rather than two versions of it.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Default)]
pub struct WarLosses {
    pub units: u32,
    pub cities: u32,
    /// Unit names include their formation (for example, `knight corps`) so
    /// the infobox can name the force that was actually destroyed instead of
    /// reducing every casualty to an anonymous counter.
    #[serde(default)]
    pub unit_kinds: BTreeMap<String, u32>,
    /// A city can change hands more than once. Keep every loss in order rather
    /// than a set so the ledger still accounts for each capture.
    #[serde(default)]
    pub city_names: Vec<String>,
    /// The same losses with the facts that rank them. Older saves carry only
    /// `city_names`, so this is additive rather than a replacement.
    #[serde(default)]
    pub city_losses: Vec<WarCityLoss>,
}

/// One city taken in a war, recorded as it falls.
///
/// A war log that lists lost cities has to rank them, and neither fact needed
/// for that survives the event: a razed city has no population left to read,
/// and a captured one keeps growing under its new owner. A capital is the
/// largest loss a civilization can take; after that, the population that
/// changed hands is the readable stand-in for how much city was lost.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WarCityLoss {
    pub name: String,
    pub turn: u32,
    /// Population at the moment it was taken.
    pub pop: i32,
    /// Whether it was the loser's own capital, not merely a capital it held.
    pub capital: bool,
}

/// One civilization's interval in a war. City-states are real belligerents
/// even though their diplomatic relation is derived from their Suzerain, so
/// participation is captured while the war is live rather than reconstructed
/// from whatever the envoy table happens to say later.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WarParticipation {
    pub player: usize,
    /// `true` is the declaring coalition (the left column in the infobox).
    pub declarer_side: bool,
    /// A city-state's Suzerain when it entered; `None` for a principal civ.
    pub suzerain: Option<usize>,
    pub entered: u32,
    pub exited: Option<u32>,
    /// Public military score at entry, which is the start-of-war strength for
    /// original belligerents and the entry strength for a late participant.
    pub strength: i64,
    /// Highest unit-only military score observed while this interval was active.
    /// `None` identifies a save written before war-strength history existed.
    #[serde(default)]
    pub peak_strength: Option<i64>,
    /// Each unit that attacked, defended, intercepted, or pillaged during this
    /// participation interval, valued with the same health-adjusted base
    /// strength used by `military_power`. Keeping the per-unit high-water mark
    /// makes the total durable after casualties and prevents repeat attacks
    /// from inflating a belligerent's effort.
    #[serde(default)]
    pub saw_action_units: BTreeMap<u32, i64>,
}

/// Terms attached to one bilateral exit from a potentially larger conflict.
/// Multiple entries let an ally peace out before the principal war ends.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WarPeace {
    pub turn: u32,
    pub first: usize,
    pub second: usize,
    pub terms: Vec<String>,
}

/// A turning point in a war: its declaration, a city changing hands, the
/// peace. Unit casualties are itemized separately from this chronology so a
/// hundred skirmishes cannot bury the moments that decided the conflict.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WarHighlight {
    pub turn: u32,
    /// `declared`, `city_captured`, `capital_captured`, `city_razed`,
    /// `nuclear_strike`, `peace`, `coalition` or `conquest`.
    pub kind: String,
    /// The player who acted, and the one it happened to.
    pub actor: usize,
    pub subject: usize,
    pub city: Option<String>,
}

/// A place where a war was fought.  Unlike the units and borders on the live
/// map, these sites survive peace, conquest, later rebuilding and a reload, so
/// a client can return to the battlefield and show what became of it.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WarTheaterSite {
    pub turn: u32,
    pub pos: Pos,
}

/// One way a device can reach its target this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WmdPlatform {
    /// `missile_silo`, or the kind of the unit carrying the device.
    pub kind: Name,
    /// Where the device leaves from.
    pub from: Pos,
    /// The carrier, which spends its attack on the launch. A silo has none.
    pub unit: Option<u32>,
    pub distance: i32,
}

/// One detonation, recorded on the game rather than reconstructed from its
/// consequences.
///
/// A nuclear strike is the loudest single event a game contains, and three
/// separate surfaces need the *same* account of it: the notification log has
/// to name it, the war ledger has to place it in the war it was used in, and a
/// client has to put a fireball on the tile it landed on. Diffing the board
/// answers none of those — a halved city and a ring of fallout do not say who
/// fired, from what, or with which device — so the engine writes the account
/// once and everybody reads it.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct NuclearStrike {
    /// Unique for the life of the game, so a client can tell a fresh
    /// detonation from one it has already animated without comparing fields.
    pub id: u32,
    pub turn: u32,
    pub attacker: usize,
    /// Ground zero.
    pub target: Pos,
    pub thermonuclear: bool,
    /// What carried the shot — `city`, `missile_silo` or `nuclear_submarine` —
    /// and the tile it was fired from.
    pub platform: String,
    pub launched_from: Pos,
    pub blast_radius: i32,
    pub fallout_until: u32,
    /// Everyone whose city, unit or territory the blast touched.
    pub victims: BTreeSet<usize>,
    /// Cities inside the radius, named as they stood before the blast halved
    /// them.
    pub cities: Vec<String>,
    pub units_destroyed: u32,
}

/// The tile-by-tile route one unit walked, recorded as it stepped rather than
/// reconstructed from a board diff.
///
/// A spectator frame is one seat's whole turn, so the board alone says only
/// where a unit ended up — not which tiles carried it there. The client draws
/// these as movement tails and walks its move animation along them, so the
/// route has to be the executed truth: a ZOC stop or an exhausted unit ends
/// the record where the unit actually stood.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct UnitMoveTrail {
    pub unit: u32,
    pub owner: usize,
    pub turn: u32,
    /// The tile the unit stood on, then every tile it stepped to, in order.
    /// Always at least two entries, and every hop is adjacent: teleport-style
    /// relocations (airlift) never chain into a walked route.
    pub path: Vec<Pos>,
}

/// The engine's account of one bilateral front, so every client and every
/// reloaded save tells the same story about it. All fronts opened by one
/// declaration share `conflict`; city-states are participants in their
/// Suzerain's front rather than synthetic declarations of their own.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WarRecord {
    /// Every front opened by the same declaration shares this id. This is the
    /// durable join key that turns a defensive-pact or team war into one
    /// conflict summary instead of one card per pair.
    #[serde(default)]
    pub conflict: u32,
    /// The declaration's named principals stay fixed even when allies join or
    /// leave, and determine the left/right orientation of every front.
    #[serde(default)]
    pub declarer: usize,
    #[serde(default)]
    pub target: usize,
    /// The declaration framework that governs warmonger/grievance outcomes
    /// for this entire conflict. Missing on old saves keeps the legacy Formal
    /// War capture baseline, preserving the accounting those saves used.
    #[serde(default)]
    pub casus_belli: Option<String>,
    /// A Joint War is a 30-turn deal, not merely a differently-labelled
    /// declaration.  This is the earliest turn this particular front may be
    /// settled; ordinary wars leave it absent and use the standard ten-turn
    /// minimum instead.
    #[serde(default)]
    pub joint_war_until: Option<u32>,
    pub aggressor: usize,
    pub defender: usize,
    pub started: u32,
    pub ended: Option<u32>,
    pub losses: BTreeMap<usize, WarLosses>,
    #[serde(default)]
    pub participants: Vec<WarParticipation>,
    #[serde(default)]
    pub peace_terms: Vec<WarPeace>,
    pub highlights: Vec<WarHighlight>,
    /// Recent places where this front saw action, oldest first.  The bounded
    /// tail describes the current theater while a war is active and becomes a
    /// durable aftermath target once it is over.
    #[serde(default)]
    pub theater: Vec<WarTheaterSite>,
}

/// Metadata shared by every front opened from one declaration.
struct WarDeclaration {
    conflict: u32,
    declarer: usize,
    target: usize,
    declared_front: bool,
    casus_belli: Option<String>,
    joint_war_until: Option<u32>,
}

impl WarRecord {
    fn new(aggressor: usize, defender: usize, turn: u32, declaration: WarDeclaration) -> Self {
        let WarDeclaration {
            conflict,
            declarer,
            target,
            declared_front,
            casus_belli,
            joint_war_until,
        } = declaration;
        Self {
            conflict,
            declarer,
            target,
            casus_belli,
            joint_war_until,
            aggressor,
            defender,
            started: turn,
            ended: None,
            losses: BTreeMap::new(),
            participants: Vec::new(),
            peace_terms: Vec::new(),
            highlights: declared_front
                .then(|| WarHighlight {
                    turn,
                    kind: "declared".into(),
                    actor: declarer,
                    subject: target,
                    city: None,
                })
                .into_iter()
                .collect(),
            theater: Vec::new(),
        }
    }

    fn record_theater_site(&mut self, turn: u32, pos: Pos) {
        if self
            .theater
            .iter()
            .rev()
            .take_while(|site| site.turn == turn)
            .any(|site| site.pos == pos)
        {
            return;
        }
        self.theater.push(WarTheaterSite { turn, pos });
        const THEATER_SITES_KEPT: usize = 64;
        if self.theater.len() > THEATER_SITES_KEPT {
            self.theater
                .drain(..self.theater.len() - THEATER_SITES_KEPT);
        }
    }

    pub fn losses_for(&self, player: usize) -> WarLosses {
        self.losses.get(&player).cloned().unwrap_or_default()
    }
}

/// A pre-negotiated, one-click offer for the human/AI Quick Deals surface.
/// Values are expressed in equivalent lump Gold and are positive for both
/// parties by construction.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct QuickDeal {
    pub partner: usize,
    pub category: String,
    pub item: String,
    pub direction: String,
    pub offer: DealItems,
    pub request: DealItems,
    pub my_value: f64,
    pub partner_value: f64,
}

/// A physical Firaxis Great Person that the live bridge cannot currently use.
///
/// Headless CIVVIS games retire Great People immediately and therefore leave
/// this empty. The live mirror fills it only for a person whose host-validated
/// activation-plot list is empty, so an AI can create the missing district,
/// Great Work capacity, Wonder, or eligible unit instead of treating the
/// person standing in the capital as already spent.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LiveGreatPersonActivationNeed {
    /// CIVVIS class spelling, such as `scientist` or `writer`.
    pub kind: String,
    /// Firaxis individual spelling with its prefix removed and lowercased.
    /// This is optional because older control-mod exports did not include it.
    #[serde(default)]
    pub individual: Option<String>,
    /// CIVVIS district-family spelling for an individual's authoritative hard
    /// prerequisite. `None` means Firaxis named no completed-district gate.
    #[serde(default)]
    pub required_district: Option<String>,
    /// CIVVIS building-family spelling for Firaxis's
    /// `ActionRequiresMissingBuildingType` gate. This is a negative
    /// prerequisite: the building must be absent because the activation may
    /// supply it (James of St. George is the stock example).
    #[serde(default)]
    pub required_missing_building: Option<String>,
    /// CIVVIS Great Work object kind for Firaxis's
    /// `ActionRequiresCityGreatWorkObjectType` gate, such as `artifact`.
    #[serde(default)]
    pub required_great_work: Option<String>,
}

/// Aggregate public standings reported by an authoritative host for one
/// civilization.
///
/// A live mirror deliberately keeps fogged cities and units out of the board,
/// but Civilization VI's standings expose these empire-wide totals. Keeping
/// them separate from the reconstructed objects lets the HUD be exact without
/// manufacturing hidden locations, infrastructure, or armies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObservedPublicEmpireStats {
    pub city_count: Option<usize>,
    pub population: Option<i32>,
    pub wonder_count: Option<usize>,
    pub suzerain_count: Option<usize>,
    pub nuclear_devices: Option<i64>,
    pub thermonuclear_devices: Option<i64>,
    pub techs: Option<usize>,
    /// Highest research era, independent of the private technology identities.
    /// Colonial War must not read a redacted tree as an Ancient-era rival.
    #[serde(default)]
    pub tech_era: Option<usize>,
    pub civics: Option<usize>,
    pub tourism_per_turn: Option<f64>,
    /// The culture victory's two public counters as the host's World Rankings
    /// screen shows them for every major: tourists visiting this civilization,
    /// and its own staycationers (the bar every rival must clear).
    /// `#[serde(default)]` keeps saves from before these fields loadable.
    #[serde(default)]
    pub foreign_tourists: Option<usize>,
    #[serde(default)]
    pub domestic_tourists: Option<usize>,
    /// The engine's own turns until this major wins on Culture
    /// (`GetTurnsUntilVictory`); a negative reading means it reports none.
    #[serde(default)]
    pub culture_turns_to_victory: Option<f64>,
    /// Two more of that screen's own lane numbers: the religion lane's cities
    /// following this seat's religion (`GetNumCitiesFollowingReligion`, which
    /// counts cities the seat has never seen) and the domination lane's
    /// military strength without the treasury
    /// (`GetMilitaryStrengthWithoutTreasury` — the army alone, where
    /// `observed_military_power` is the ribbon figure with Gold folded in).
    #[serde(default)]
    pub cities_following_religion: Option<usize>,
    #[serde(default)]
    pub military_no_treasury: Option<f64>,
    /// Civilization VI's authoritative Science Victory Points from World
    /// Rankings. This is intentionally separate from `Player`'s native-game
    /// travel simulation: a live host can use a nonstandard victory target.
    #[serde(default)]
    pub science_victory_points: Option<f64>,
    /// Current science-victory movement per turn as Firaxis counts it. In
    /// particular this reflects which repeatable laser stations are active,
    /// including whether a Terrestrial Laser Station is powered.
    #[serde(default)]
    pub science_victory_points_per_turn: Option<f64>,
    /// The host's total Science Victory Points required for its game speed.
    #[serde(default)]
    pub science_victory_points_needed: Option<f64>,
    /// Every unit the host says this seat fields, and how many of them are
    /// military, counted off its whole roster. The board itself holds only
    /// the rival units the seat can see, so the standings' UNIT column reads
    /// these rather than counting the board. `None` on an export from before
    /// the host sent them.
    #[serde(default)]
    pub unit_count: Option<usize>,
    #[serde(default)]
    pub military_unit_count: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: usize,
    pub civ: String,
    /// Pre-game multiplayer team. Missing in old saves means free-for-all.
    #[serde(default)]
    pub team: Option<usize>,
    pub techs: BTreeSet<Name>,
    pub research: Option<String>,
    pub research_progress: f64,
    pub research_overflow: f64,
    pub civics: BTreeSet<Name>,
    pub civic: Option<String>,
    pub civic_progress: f64,
    pub civic_overflow: f64,
    pub gold: f64,
    /// Net recurring Gold budget from the most recently processed turn,
    /// including active Gold-per-turn deals and all modeled maintenance.
    #[serde(default)]
    pub gold_per_turn: f64,
    /// Empire-wide Amenity loss while the treasury is empty and the recurring
    /// budget is negative: one per complete 10 Gold of deficit.
    #[serde(default)]
    pub bankruptcy_amenity_penalty: i64,
    /// Extra weight this empire's governors put on food when assigning
    /// citizens, added to `citizen_strategy`'s food appetite.
    ///
    /// Per **player**, not per game, because `ai_eval` is seat-mirrored: both
    /// arms play the same map, so anything global cannot be a treatment arm.
    /// Zero is the shipped behaviour and old saves default to it. Nothing in
    /// the engine sets this — an agent does, which is the point: citizen
    /// assignment is the one city-level decision no player, human or AI, can
    /// currently express.
    #[serde(default)]
    pub citizen_food_bias: f64,
    /// What this empire's plan asks of each of its cities, keyed by city id.
    ///
    /// The per-city counterpart of `citizen_food_bias`, and for the same
    /// reason: it is per **player** so `ai_eval`'s seat-mirrored arms can
    /// differ on one map. An empty map is the shipped behaviour and old saves
    /// default to it. Nothing in the engine writes this — an agent does, once
    /// per turn, and stale entries for cities it no longer owns are harmless
    /// because the governor only ever looks up cities that exist.
    #[serde(default)]
    pub city_directives: BTreeMap<u32, CityDirective>,
    pub faith: f64,
    /// Gathering Storm strategic-resource stockpiles. Luxuries remain copy
    /// based and are intentionally not represented here.
    #[serde(default)]
    pub strategic_resources: BTreeMap<Name, f64>,
    /// Unpaid per-turn fuel demand by resource, capped at Civ VI's -20
    /// combat-strength shortage penalty.
    #[serde(default)]
    pub strategic_resource_shortages: BTreeMap<Name, i32>,
    /// Power actually allocated to each owned city at the start of this
    /// player's current turn, after renewable generation and fuel burning.
    #[serde(default)]
    pub city_power: BTreeMap<u32, f64>,
    /// Fuel consumed by power plants on the current turn, for observation and
    /// deterministic save/replay diagnostics.
    #[serde(default)]
    pub power_fuel_consumed: BTreeMap<String, f64>,
    /// Cumulative CO2 emitted by this civilization's power generation.
    #[serde(default)]
    pub co2_emissions: f64,
    pub explored: BTreeSet<Pos>,
    /// Tiles this civilization has seen at any point during its active player
    /// turn. Current sight remains authoritative for rules and targeting; this
    /// short-lived union exists so the map can keep the just-seen frame lit
    /// until this player's End Turn, then fall back to ordinary fog memory.
    #[serde(default)]
    pub turn_visible: BTreeSet<Pos>,
    /// Rival units last observed during this active player turn. Unlike the
    /// long-term map memory, these contacts survive only until End Turn.
    #[serde(default)]
    pub turn_units: BTreeMap<u32, Unit>,
    /// Whether this civilization's own knowledge has ever run the whole way
    /// round the world — see [`Game::update_world_lap`]. Once a people have
    /// been around they know the shape they live on; until then they do not,
    /// and their chart says so.
    #[serde(default)]
    pub went_around: bool,
    /// Longitude bands already covered by this civilization's explored ground —
    /// see [`Game::update_world_lap`]. A running accumulation over `explored`,
    /// so each tile is classified once as it leaves the fog instead of the
    /// whole explored set being rescanned on every visibility refresh. Empty
    /// means not yet accumulated; the next lap check rebuilds it from
    /// `explored`, which also migrates saves from before the cache existed.
    /// Cleared once the lap closes, because the answer can never change back.
    #[serde(default)]
    pub lap_bands: BTreeSet<u32>,
    /// Per-tile last-known state. Unlike `explored`, this does not update
    /// while a tile is under fog, so improvements, ownership, disasters, and
    /// other changes cannot leak through a player observation.
    #[serde(default)]
    pub remembered_tiles: TileMemory,
    /// Last public City Center state observed by this player.
    #[serde(default)]
    pub remembered_cities: BTreeMap<u32, RememberedCity>,
    /// Every seat this civilization has made contact with, and never forgets.
    /// A world is full of empires nobody has met yet; until a unit or a city
    /// of theirs has actually been seen, they are not on anybody's ledger,
    /// which is why this is knowledge held per seat rather than a fact about
    /// the world. Barbarians are nobody's acquaintance and are left out.
    #[serde(default)]
    pub met: BTreeSet<usize>,
    pub alive: bool,
    pub is_minor: bool,
    #[serde(default)]
    pub is_barbarian: bool,
    /// The one non-playable Free Cities seat. It is minor/barbarian for
    /// victory and diplomacy filtering, but follows its own Loyalty rules.
    #[serde(default)]
    pub is_free_city: bool,
    #[serde(default)]
    pub government: Option<String>,
    /// Every form of government this civilization has already run. Trying one
    /// for the first time is free; the people remember the rest, and going
    /// back to one of them costs Anarchy.
    #[serde(default)]
    pub past_governments: BTreeSet<String>,
    /// The government that takes power when Anarchy ends.
    #[serde(default)]
    pub pending_government: Option<String>,
    /// Turns of Anarchy still to serve. Zero when the empire is governed.
    #[serde(default)]
    pub anarchy_turns: u32,
    #[serde(default)]
    pub policies: BTreeSet<Name>,
    /// Runtime modifier bundles attached directly to this player. World
    /// Congress and mod-defined systems use this for Civ VI's
    /// `ATTACH_MODIFIER_TO_PLAYERTYPE`; the IDs resolve through
    /// `rules.modifiers` and survive save/load.
    #[serde(default)]
    pub attached_modifiers: BTreeSet<String>,
    #[serde(default)]
    pub influence: f64,
    #[serde(default)]
    pub envoys_free: i64,
    #[serde(default)]
    pub gpp: BTreeMap<String, f64>, // great person points by type
    /// The exact Great Person classes Firaxis is offering right now, when a
    /// live mirror supplied that screen. `None` preserves CIVVIS's native
    /// global roster for headless games, older exports, and saved games;
    /// `Some(empty)` is the host's authoritative answer that no class can be
    /// recruited this turn.
    #[serde(default)]
    pub live_great_person_offers: Option<BTreeSet<String>>,
    /// Open native Great Work slot kinds after retaining each work in its
    /// observed building. None means occupancy was not exported; this is a
    /// spending input, not a restriction on recruiting a physical Great Person.
    #[serde(default)]
    pub live_open_great_work_slots: Option<BTreeSet<String>>,
    /// The exact named individual Firaxis is offering for each class. A
    /// mirrored controller uses this to distinguish a Space Race Engineer
    /// from an otherwise-valid Engineer whose effect belongs to another
    /// victory plan. Empty on native boards and older exports.
    #[serde(default)]
    pub live_great_person_offer_individuals: BTreeMap<String, String>,
    /// Hard activation prerequisites of the *live* named offer, keyed by
    /// Great Person class. Headless games leave this empty; the Firaxis mirror
    /// fills it only when an offered individual names a completed district the
    /// empire does not have.
    #[serde(default)]
    pub live_great_person_offer_blockers: BTreeMap<String, String>,
    /// Physical Great People in a mirrored Civilization VI game for whom the
    /// host currently exposes no legal activation plot. This is mirror input,
    /// never persistent policy; an empty/default value preserves ordinary
    /// engine and old-save behavior.
    #[serde(default)]
    pub live_great_person_activation_needs: Vec<LiveGreatPersonActivationNeed>,
    /// The Great Person classes the mirrored Civilization VI game has nobody
    /// left to recruit in, when the host says. `None` in headless games and
    /// on older exports: the engine then falls back to its own roster
    /// (`current_great_person`). `Some(set)` is the host's answer — a class in
    /// it has no unclaimed individual anywhere on the timeline, and its
    /// points are the ones Firaxis turns into Faith
    /// (`great_person_class_earnable`, `unused_great_person_faith`).
    #[serde(default)]
    pub live_great_person_exhausted: Option<BTreeSet<String>>,
    /// How many majors the host says are alive, the seat included: the
    /// World Congress standing (`congress_dvp`, one entry per alive major,
    /// met or not). `None` headless and before the first standing. The
    /// mirror plants a seat for every major at setup and never retires one,
    /// so an eliminated major stays `alive` on the board; see
    /// `AdvancedAi::retire_defeated_majors`.
    #[serde(default)]
    pub live_living_majors: Option<usize>,
    #[serde(default)]
    pub gp_claimed: BTreeMap<String, i64>,
    /// IDs of named Great People recruited from the global market.
    #[serde(default)]
    pub great_people: Vec<String>,
    #[serde(default)]
    pub pantheon: Option<String>,
    #[serde(default)]
    pub religion: Option<String>,
    #[serde(default)]
    pub holy_city: Option<u32>,
    #[serde(default)]
    pub religion_beliefs: Vec<String>,
    /// Optional Secret Societies game-mode membership. The three society
    /// replacement buildings in this ruleset are gated by this choice.
    #[serde(default)]
    pub secret_society: Option<String>,
    #[serde(default)]
    pub prophet_pending: bool,
    #[serde(default)]
    pub era_score: i64,
    #[serde(default)]
    pub era_score_baseline: i64,
    #[serde(default)]
    pub normal_age_threshold: i64,
    #[serde(default)]
    pub golden_age_threshold: i64,
    #[serde(default)]
    pub dedications: BTreeSet<String>,
    #[serde(default)]
    pub dedication_choices: usize,
    /// How often each Dedication trigger has fired for this civilization since
    /// the current age began, counted whether or not it was dedicated. A
    /// Dedication is a bet on your own behaviour, so the only honest estimate
    /// of what one would pay is what the behaviour it names actually did.
    #[serde(default)]
    pub era_triggers: BTreeMap<String, i64>,
    /// `era_triggers` as it stood when the last age ended — the evidence a
    /// civilization has in hand at the moment it must dedicate the next one.
    #[serde(default)]
    pub last_era_triggers: BTreeMap<String, i64>,
    /// Cities this civilization's religion has taken at least once, so
    /// Exodus of the Evangelists pays only for the first conversion of each.
    #[serde(default)]
    pub converted_cities: BTreeSet<u32>,
    #[serde(default)]
    pub discovered_natural_wonders: BTreeSet<Name>,
    #[serde(default)]
    pub governors: Vec<u32>, // city ids with an established governor
    #[serde(default)]
    pub governor_roster: SpecMap<GovernorState>,
    #[serde(default)]
    pub governor_titles_spent: usize,
    #[serde(default)]
    pub grievances: BTreeMap<usize, f64>,
    /// Last agenda stance this leader announced about each rival: -1 disdain,
    /// 0 indifference, 1 approval. Only changes are worth reporting.
    #[serde(default)]
    pub agenda_view: BTreeMap<usize, i8>,
    #[serde(default)]
    pub denounced_until: BTreeMap<usize, u32>,
    /// Start turns are kept separately from expiry so the five-turn Formal
    /// War wait is exact at every game speed and not inferred from a
    /// remaining-duration heuristic.  Older saves use the old safe fallback.
    #[serde(default)]
    pub denounced_since: BTreeMap<usize, u32>,
    #[serde(default)]
    pub friends_until: BTreeMap<usize, u32>,
    #[serde(default)]
    pub open_borders_until: BTreeMap<usize, u32>,
    /// Whether this seat's border is ENFORCED at all, when a live host has
    /// said so. Civilization VI shuts a civilization's territory to foreign
    /// units only once that civilization holds Early Empire
    /// (`CIVIC_ENFORCE_BORDERS` hangs off `CIVIC_EARLY_EMPIRE`), and a
    /// native game reads that off the seat's own civics through
    /// [`Game::enforces_borders`]. A mirrored board does not model a rival's
    /// or a city-state's civics, so without this it answered "no border" for
    /// every foreign seat and walked military units into ground the host
    /// refuses: run civvis-20260826T184456Z sent 122 military steps into a
    /// non-suzerain city-state's land and 4 % arrived, 37 into a rival's
    /// closed border and 0 arrived. `None` = derive from civics, which is
    /// every native game.
    #[serde(default)]
    pub borders_enforced: Option<bool>,
    /// Diplomatic Visibility this seat holds on each rival AS THE HOST REPORTS
    /// IT (`GetVisibilityOn`, the `GameInfo.Visibilities` index), when a live
    /// mirror has said so. A native game derives the level in
    /// [`Game::diplomatic_visibility`] from trees, missions, routes, alliances
    /// and listening posts; a mirrored board cannot see most of those sources,
    /// so without this every rival read as level 0 both ways and the combat
    /// modifier that visibility buys was never priced. Assigned from every
    /// export; empty on every native board.
    #[serde(default)]
    pub observed_visibility: BTreeMap<usize, f64>,
    /// This leader's standing toward each seat AS THE HOST REPORTS IT
    /// (`GetDiplomaticStateIndex`, `DiplomacyActionView.lua:870`), without the
    /// `DIPLO_STATE_` prefix: `FRIENDLY`, `NEUTRAL`, `UNFRIENDLY`,
    /// `DENOUNCED`, `DECLARED_FRIEND`, `ALLIED`, `WAR`. The treaty states are
    /// already applied as treaties; the attitudes are what a live seat needs
    /// before it asks for one, since the host's leaders value a declared
    /// friendship only from `FRIENDLY`. Assigned from every export; empty on
    /// every native board.
    #[serde(default)]
    pub observed_diplomatic_state: BTreeMap<usize, String>,
    #[serde(default)]
    pub alliances: BTreeMap<usize, AllianceState>,
    /// Outgoing Delegations or Resident Embassies, keyed by the leader that
    /// received them.
    #[serde(default)]
    pub diplomatic_missions: BTreeMap<usize, DiplomaticMission>,
    /// Bilateral Defensive Pact expiry turns.  The same state is written on
    /// both signatories so save/load and unilateral expiry stay deterministic.
    #[serde(default)]
    pub defensive_pacts: BTreeMap<usize, u32>,
    /// Promises this civilization made, grouped by the civilization that
    /// asked for them and then by promise kind.
    #[serde(default)]
    pub promises: BTreeMap<usize, BTreeMap<String, u32>>,
    /// Conduct by other leaders that makes one of the Discuss-promise
    /// requests available to this civilization.  The outer key is the
    /// offending leader and the inner key is the promise kind.
    #[serde(default)]
    pub diplomatic_incidents: BTreeMap<usize, BTreeMap<String, DiplomaticIncident>>,
    /// A promise broken against each requester remains usable as the trigger
    /// for a War of Retribution for the standard 30-turn window.
    #[serde(default)]
    pub broken_promises_until: BTreeMap<usize, u32>,
    #[serde(default)]
    pub diplomatic_favor: f64,
    /// Accumulated War Weariness points. Every
    /// `WAR_WEARINESS_POINTS_FOR_AMENITY_LOSS` (400) of them costs one
    /// Amenity in *every* city this civilization owns.
    #[serde(default)]
    pub war_weariness: f64,
    /// Ages already lived through. `THRESHOLD_SHIFT_PER_PAST_DARK_AGE` is -5
    /// and `_PER_PAST_GOLDEN_AGE` is +5, so a civilization that has struggled
    /// finds the next Normal Age easier to reach and one that has prospered
    /// finds it harder.
    #[serde(default)]
    pub past_dark_ages: i64,
    #[serde(default)]
    pub past_golden_ages: i64,
    #[serde(default)]
    pub dvp: i64, // diplomatic victory points
    #[serde(default = "normal_age")]
    pub age: String,
    #[serde(default)]
    pub culture_lifetime: f64,
    #[serde(default)]
    pub tourism_lifetime: f64,
    /// One-time Tourism pressure aimed at a specific civilization, chiefly
    /// from Rock Band concerts. `tourism_lifetime` remains the public total;
    /// this ledger prevents directed bursts from affecting every rival.
    #[serde(default)]
    pub targeted_tourism: BTreeMap<usize, f64>,
    /// Lifetime Tourism pressure after per-civilization international
    /// modifiers. Entries are accumulated when Tourism is generated, so a
    /// later border, route, religion, or government change is not retroactive.
    /// Missing entries use the legacy lifetime totals for save compatibility.
    #[serde(default)]
    pub tourism_pressure: BTreeMap<usize, f64>,
    /// Portion of lifetime Tourism produced by Relics and controlled Holy
    /// Cities. Enlightenment and Cristo Redentor apply only to this portion.
    #[serde(default)]
    pub religious_tourism_lifetime: f64,
    /// Completed one-time space-race projects.
    #[serde(default)]
    pub science_projects: BTreeSet<String>,
    /// Light-years travelled after launching the Exoplanet Expedition.
    #[serde(default)]
    pub exoplanet_distance: f64,
    /// Which world the expedition was aimed at, by id in [`EXOPLANET_TARGETS`].
    /// Set once, when the expedition launches, from what had been surveyed by
    /// then; `None` until then, and on a save written before targets existed.
    #[serde(default)]
    pub exoplanet_target: Option<String>,
    /// Where this civilization's mass drivers put their slugs down: a tile in
    /// its own territory, standing until it names another. `None` means the
    /// drivers are built but nothing has been aimed, and nothing falls.
    #[serde(default)]
    pub mass_driver_site: Option<Pos>,
    /// Which of the Moon's ores those drivers are mining. Set with the site,
    /// by the same order, because a driver aimed at nothing in particular is
    /// not a state worth having.
    #[serde(default)]
    pub mass_driver_ore: Option<Name>,
    /// Drivers that threw their slug at something this turn instead of
    /// delivering it. Reset at the start of every turn.
    #[serde(default)]
    pub mass_driver_shots: i64,
    #[serde(default)]
    pub envoys: Vec<(usize, i64)>, // (city-state pid, envoys placed)
    #[serde(default)]
    pub counters: BTreeMap<String, i64>,
    /// Completed military-unit lives by the kind the unit carried when it
    /// left play. Live units are composed with this by
    /// [`Game::unit_lifetime_stats`].
    #[serde(default)]
    pub unit_lifetimes: BTreeMap<Name, UnitLifetimeStats>,
    /// Every great work as an object with its creating era and creator, kept
    /// in lockstep with the ``great_work:*`` counters. Theming reads these.
    #[serde(default)]
    pub great_work_pieces: Vec<GreatWorkPiece>,
    /// The quest each met city-state is currently asking this civilization
    /// for, keyed by the city-state's seat. Per pair, not per city-state.
    #[serde(default)]
    pub quests: BTreeMap<usize, crate::game::quests::CityStateQuest>,
    #[serde(default)]
    pub boosted_techs: BTreeSet<Name>,
    #[serde(default)]
    pub boosted_civics: BTreeSet<Name>,
}

impl Player {
    fn new(id: usize, civ: &str, is_minor: bool) -> Player {
        Player {
            id,
            civ: civ.to_string(),
            team: None,
            techs: BTreeSet::new(),
            research: None,
            research_progress: 0.0,
            research_overflow: 0.0,
            civics: BTreeSet::new(),
            civic: None,
            civic_progress: 0.0,
            civic_overflow: 0.0,
            gold: 0.0,
            gold_per_turn: 0.0,
            bankruptcy_amenity_penalty: 0,
            citizen_food_bias: 0.0,
            city_directives: BTreeMap::new(),
            faith: 0.0,
            strategic_resources: BTreeMap::new(),
            strategic_resource_shortages: BTreeMap::new(),
            city_power: BTreeMap::new(),
            power_fuel_consumed: BTreeMap::new(),
            co2_emissions: 0.0,
            explored: BTreeSet::new(),
            turn_visible: BTreeSet::new(),
            turn_units: BTreeMap::new(),
            went_around: false,
            lap_bands: BTreeSet::new(),
            remembered_tiles: TileMemory::default(),
            remembered_cities: BTreeMap::new(),
            met: BTreeSet::new(),
            alive: true,
            is_minor,
            is_barbarian: false,
            is_free_city: false,
            government: None,
            past_governments: BTreeSet::new(),
            pending_government: None,
            anarchy_turns: 0,
            policies: BTreeSet::new(),
            attached_modifiers: BTreeSet::new(),
            influence: 0.0,
            envoys_free: 0,
            gpp: BTreeMap::new(),
            live_great_person_offers: None,
            live_open_great_work_slots: None,
            live_great_person_offer_individuals: BTreeMap::new(),
            live_great_person_offer_blockers: BTreeMap::new(),
            live_great_person_activation_needs: Vec::new(),
            live_great_person_exhausted: None,
            live_living_majors: None,
            gp_claimed: BTreeMap::new(),
            great_people: Vec::new(),
            pantheon: None,
            religion: None,
            holy_city: None,
            religion_beliefs: Vec::new(),
            secret_society: None,
            prophet_pending: false,
            era_score: 0,
            era_score_baseline: 0,
            normal_age_threshold: 12,
            golden_age_threshold: 26,
            dedications: BTreeSet::new(),
            dedication_choices: 0,
            era_triggers: BTreeMap::new(),
            last_era_triggers: BTreeMap::new(),
            converted_cities: BTreeSet::new(),
            discovered_natural_wonders: BTreeSet::new(),
            governors: Vec::new(),
            governor_roster: SpecMap::new(),
            governor_titles_spent: 0,
            grievances: BTreeMap::new(),
            agenda_view: BTreeMap::new(),
            denounced_until: BTreeMap::new(),
            denounced_since: BTreeMap::new(),
            friends_until: BTreeMap::new(),
            open_borders_until: BTreeMap::new(),
            borders_enforced: None,
            observed_visibility: BTreeMap::new(),
            observed_diplomatic_state: BTreeMap::new(),
            alliances: BTreeMap::new(),
            diplomatic_missions: BTreeMap::new(),
            defensive_pacts: BTreeMap::new(),
            promises: BTreeMap::new(),
            diplomatic_incidents: BTreeMap::new(),
            broken_promises_until: BTreeMap::new(),
            diplomatic_favor: 0.0,
            war_weariness: 0.0,
            past_dark_ages: 0,
            past_golden_ages: 0,
            dvp: 0,
            age: "normal".to_string(),
            culture_lifetime: 0.0,
            tourism_lifetime: 0.0,
            targeted_tourism: BTreeMap::new(),
            tourism_pressure: BTreeMap::new(),
            religious_tourism_lifetime: 0.0,
            science_projects: BTreeSet::new(),
            exoplanet_distance: 0.0,
            exoplanet_target: None,
            mass_driver_site: None,
            mass_driver_ore: None,
            mass_driver_shots: 0,
            envoys: Vec::new(),
            counters: BTreeMap::new(),
            unit_lifetimes: BTreeMap::new(),
            great_work_pieces: Vec::new(),
            quests: BTreeMap::new(),
            boosted_techs: BTreeSet::new(),
            boosted_civics: BTreeSet::new(),
        }
    }
}

// ------------------------------------------------------------------- actions

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Move {
        unit: u32,
        to: Pos,
    },
    MoveTo {
        unit: u32,
        to: Pos,
    },
    /// Exchange two adjacent units of one player. Civilization VI offers this
    /// wherever a unit's move would end on a friendly unit of its own layer:
    /// the pair trades hexes and each pays the entry cost of the tile it takes.
    /// It is how a damaged front-liner is pulled out of contact for the healthy
    /// unit behind it without either of them leaving the line.
    Swap {
        unit: u32,
        other: u32,
    },
    Attack {
        unit: u32,
        target: Pos,
    },
    Ranged {
        unit: u32,
        target: Pos,
    },
    FoundCity {
        unit: u32,
    },
    Improve {
        unit: u32,
        improvement: Name,
    },
    /// Retire the currently available Great Merchant to upgrade an Industry.
    FoundCorporation {
        pos: Pos,
    },
    /// Products are movable economic Great Works. Equal-resource Products are
    /// interchangeable, so the resource ID is a stable save/replay handle.
    MoveProduct {
        from: u32,
        to: u32,
        product: Name,
    },
    ContributeProject {
        unit: u32,
        city: u32,
    },
    /// Spend one Military Engineer charge for 20% of the current Aqueduct,
    /// Dam, or Canal cost (40% for England's Workshop of the World).
    ContributeDistrict {
        unit: u32,
        city: u32,
    },
    PerformConcert {
        unit: u32,
    },
    /// Pay Gold to turn an obsolete unit into its successor, keeping its
    /// experience and same-class promotions.
    UpgradeUnit {
        unit: u32,
    },
    /// Disband one of our own units. The bridge sends the host
    /// `UNITCOMMAND_DELETE`; `bankruptcy-exits-the-drain` uses it to choose
    /// which units go before the host's bankruptcy takes the costliest.
    DisbandUnit {
        unit: u32,
    },
    Pillage {
        unit: u32,
    },
    RepairImprovement {
        unit: u32,
    },
    CoastalRaid {
        unit: u32,
        target: Pos,
    },
    AirRebase {
        unit: u32,
        to: Pos,
    },
    AirStrike {
        unit: u32,
        target: Pos,
    },
    /// Bomber mission that pillages one improvement, district, or building
    /// layer without collecting the normal ground-unit plunder reward.
    AirPillage {
        unit: u32,
        target: Pos,
    },
    /// Spec Ops and jet aircraft bypass a military escort to deal the stock
    /// fixed 65 damage to a support unit in the target formation.
    PriorityTarget {
        unit: u32,
        target: Pos,
    },
    AirPatrol {
        unit: u32,
        to: Pos,
    },
    Produce {
        city: u32,
        item: Item,
    },
    Buy {
        city: u32,
        unit: Name,
        /// Zero buys a normal unit; one and two buy a Corps/Fleet or
        /// Army/Armada directly. Missing fields in older saves remain normal.
        #[serde(default)]
        formation: u8,
        #[serde(default = "gold_s")]
        currency: String,
    },
    BuyBuilding {
        city: u32,
        building: Name,
        #[serde(default = "gold_s")]
        currency: String,
    },
    BuyDistrict {
        city: u32,
        district: Name,
        pos: Pos,
        #[serde(default = "gold_s")]
        currency: String,
    },
    /// Purchase one neutral plot for a city. `cost` is the quoted price sent
    /// to clients; applying the action always recomputes it from live state.
    BuyPlot {
        city: u32,
        pos: Pos,
        #[serde(default)]
        cost: f64,
    },
    Research {
        tech: Name,
    },
    Civic {
        civic: Name,
    },
    DeclareWar {
        player: usize,
    },
    DeclareWarWithCasusBelli {
        player: usize,
        casus_belli: String,
    },
    MakePeace {
        player: usize,
    },
    Denounce {
        player: usize,
    },
    /// Pay 10 Gold to establish an individual Delegation, gaining one level
    /// of diplomatic visibility and a small relationship bonus.
    SendDelegation {
        player: usize,
    },
    /// Diplomatic Service replaces an existing Delegation with a 25-Gold
    /// Resident Embassy; the two visibility sources never stack.
    SendEmbassy {
        player: usize,
    },
    /// Ask an existing ally to sign the Mobilization-era Defensive Pact.
    ProposeDefensivePact {
        player: usize,
    },
    /// Invite one civilization to declare a Foreign Trade joint war against a
    /// third party if it accepts the ordinary timed proposal.
    ProposeJointWar {
        player: usize,
        target: usize,
    },
    /// Ask a leader to promise not to repeat one of the actionable diplomatic
    /// offences.  The recipient answers with the normal deal accept/reject
    /// actions, preserving one response surface for people and agents.
    RequestPromise {
        player: usize,
        promise: String,
    },
    /// Demand an immediate lump-sum Gold payment.  As in Civ VI, refusing a
    /// demand is itself a relationship event rather than an invalid trade.
    DemandGold {
        player: usize,
        gold: f64,
    },
    ProposeDeal {
        player: usize,
        #[serde(default)]
        give_gold: f64,
        #[serde(default)]
        request_gold: f64,
        #[serde(default)]
        open_borders: bool,
        #[serde(default)]
        friendship: bool,
        #[serde(default)]
        peace: bool,
        #[serde(default)]
        alliance: Option<String>,
    },
    AcceptDeal {
        deal: u32,
    },
    RejectDeal {
        deal: u32,
    },
    Trade {
        player: usize,
        offer: Box<DealItems>,
        request: Box<DealItems>,
    },
    CongressVote {
        resolution: Name,
        choice: String,
        votes: u32,
    },
    AssignSpy {
        spy: u32,
        city: u32,
    },
    SpyMission {
        spy: u32,
        mission: String,
        target: Pos,
    },
    PromoteSpy {
        spy: u32,
        promotion: Name,
    },
    ChooseDedication {
        dedication: Name,
    },
    Fortify {
        unit: u32,
    },
    Promote {
        unit: u32,
        promotion: Name,
    },
    Upgrade {
        unit: u32,
        to: String,
    },
    CombineUnits {
        unit: u32,
        with: u32,
    },
    LinkUnits {
        unit: u32,
        with: u32,
    },
    UnlinkUnits {
        unit: u32,
    },
    Government {
        government: Name,
    },
    SlotPolicy {
        policy: Name,
    },
    UnslotPolicy {
        policy: Name,
    },
    TradeRoute {
        unit: u32,
        city: u32,
    },
    SendEnvoy {
        player: usize,
    },
    LevyMilitary {
        player: usize,
    },
    RecruitGreatPerson {
        kind: String,
    },
    PatronizeGreatPerson {
        kind: String,
        #[serde(default = "gold_s")]
        currency: String,
    },
    ChoosePantheon {
        belief: Name,
    },
    ChooseSecretSociety {
        society: Name,
    },
    AssignGovernor {
        city: u32,
    },
    AppointGovernor {
        governor: Name,
        city: u32,
    },
    ReassignGovernor {
        governor: Name,
        city: u32,
    },
    PromoteGovernor {
        governor: Name,
        promotion: Name,
    },
    FoundReligion {
        follower: Name,
        founder: Name,
    },
    Spread {
        unit: u32,
    },
    TheologicalAttack {
        unit: u32,
        target: Pos,
    },
    CondemnHeretic {
        unit: u32,
        target_unit: u32,
    },
    HealReligious {
        unit: u32,
    },
    RemoveHeresy {
        unit: u32,
    },
    LaunchInquisition {
        unit: u32,
    },
    EvangelizeBelief {
        unit: u32,
        belief: Name,
    },
    ConvertBarbarians {
        unit: u32,
    },
    CityStrike {
        city: u32,
        target: Pos,
    },
    /// Launch a stockpiled nuclear or thermonuclear device at a revealed tile,
    /// from a working Missile Silo of the named city or from any `wmd_capable`
    /// unit in reach. See [`Game::wmd_platforms`] for what reaches how far.
    WmdStrike {
        city: u32,
        target: Pos,
        thermonuclear: bool,
    },
    /// Point every mass driver on the Moon at one ore and one landing site in
    /// your own territory. A standing order: it holds until it is given again,
    /// and until it is given nothing falls.
    AimMassDriver {
        site: Pos,
        ore: Name,
    },
    /// Throw one slug at a revealed tile instead of landing it as cargo. It
    /// costs one unit of the aimed ore out of the stockpile — the slug is the
    /// metal — and one driver's turn.
    MassDriverStrike {
        target: Pos,
    },
    /// A Military Engineer lays a Railroad on its tile: engineer-built
    /// only, no build charge, 1 Iron and 1 Coal from the stockpile per
    /// tile (Routes_XP2 and Route_ResourceCosts).
    BuildRailroad {
        unit: u32,
    },
    EncampmentStrike {
        city: u32,
        target: Pos,
    },
    DistrictStrike {
        city: u32,
        source: Pos,
        target: Pos,
    },
    KeepCity {
        city: u32,
    },
    RazeCity {
        city: u32,
    },
    LiberateCity {
        city: u32,
    },
    EndTurn,
}

impl Action {
    /// Whether this is a decision of the empire game rather than of the
    /// fighting — a technology or civic to study, a government to form, a
    /// deal to strike, a ballot to cast, an envoy, a governor, a religion.
    ///
    /// A Tactics arena is decided by the fighting and by nothing else, so
    /// none of these exist there: they are not enumerated for any seat and
    /// [`Game::apply`] refuses them, so a player on an arena is never asked
    /// for anything but orders and an AI that would choose its own research
    /// or sue for its own peace gets the same answer. Research on an arena
    /// is a pace the engine keeps for both sides alike
    /// (`Game::arena_auto_research`), and a captured city is simply kept
    /// (`Game::capture_city`) rather than put to its captor.
    ///
    /// Orders — moving, attacking, fortifying, promoting, upgrading, forming
    /// a corps, striking from a city — stay, as does what a city may build
    /// when the arena grants the Production to build it with.
    pub fn off_the_battlefield(&self) -> bool {
        matches!(
            self,
            Action::Research { .. }
                | Action::Civic { .. }
                | Action::DeclareWar { .. }
                | Action::DeclareWarWithCasusBelli { .. }
                | Action::MakePeace { .. }
                | Action::Denounce { .. }
                | Action::SendDelegation { .. }
                | Action::SendEmbassy { .. }
                | Action::ProposeDefensivePact { .. }
                | Action::ProposeJointWar { .. }
                | Action::RequestPromise { .. }
                | Action::DemandGold { .. }
                | Action::ProposeDeal { .. }
                | Action::AcceptDeal { .. }
                | Action::RejectDeal { .. }
                | Action::Trade { .. }
                | Action::CongressVote { .. }
                | Action::AssignSpy { .. }
                | Action::SpyMission { .. }
                | Action::PromoteSpy { .. }
                | Action::ChooseDedication { .. }
                | Action::Government { .. }
                | Action::SlotPolicy { .. }
                | Action::UnslotPolicy { .. }
                | Action::TradeRoute { .. }
                | Action::SendEnvoy { .. }
                | Action::LevyMilitary { .. }
                | Action::RecruitGreatPerson { .. }
                | Action::PatronizeGreatPerson { .. }
                | Action::ChoosePantheon { .. }
                | Action::ChooseSecretSociety { .. }
                | Action::AssignGovernor { .. }
                | Action::AppointGovernor { .. }
                | Action::ReassignGovernor { .. }
                | Action::PromoteGovernor { .. }
                | Action::FoundReligion { .. }
                | Action::FoundCorporation { .. }
                | Action::MoveProduct { .. }
        )
    }
}

fn gold_s() -> String {
    "gold".to_string()
}

// --------------------------------------------------------------------- game

/// One thing that happened to one civilization, in the order it happened.
///
/// Unciv keeps a categorized notification list per civilization, with each
/// entry able to point at the tile it concerns; it turns out to be the piece
/// that makes both a spectator and an agent able to say *why* the board
/// changed rather than only *that* it changed. Ours is the same idea reduced
/// to data: the GUI renders it as a log, and `obs` hands it to agents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub turn: u32,
    /// The civilization this happened to. Events are visible only to them.
    pub player: usize,
    /// One of [`EVENT_CATEGORIES`]. `Nuclear` is deliberately its own category
    /// rather than a flavour of `War`: it is the only one a client is expected
    /// to give a distinct icon and a place at the top of the log.
    pub category: String,
    pub text: String,
    /// Where to look, when there is somewhere to look.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<Pos>,
    /// Too consequential to let scroll past. Set for the handful of events
    /// whose category does not already say so — a detonation is `War`, like a
    /// unit upgrade.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub important: bool,
}

/// Every category [`Event`] can carry.
///
/// This is a vocabulary, not a hint: the browser's event log offers a filter
/// per kind, and a kind gathers categories, so a category nothing on the
/// client claims is an entry a reader can see in the combined log and never
/// reach by asking a narrower question. `browser_lets_an_observer_narrow_the_
/// two_chronicles` in `server.rs` checks this list against the client's
/// grouping, so adding one here without grouping it there fails the build.
/// `Game::note` asserts membership in debug builds, which is what catches a
/// category invented at a call site and declared nowhere.
///
/// `Faith` is currently written only by the browser, which files a religion
/// being founded under it. It stays declared because the classification is
/// shared: the log the reader sees is fed from both sides.
pub const EVENT_CATEGORIES: [&str; 11] = [
    "General",
    "World",
    "Cities",
    "War",
    "Nuclear",
    "Science",
    "Culture",
    "Faith",
    "People",
    "Diplomacy",
    "CityState",
];

/// Events older than this are dropped, oldest first. Long games are long.
const EVENT_LIMIT: usize = 2_000;

/// How many longitudes a globe is cut into when asking whether a people have
/// been round it — ten degrees apiece. Coarse enough that a ship crossing a
/// band cannot pass through without seeing it, fine enough that a lap is a
/// lap. A cylinder is asked about its own columns instead, since it has a
/// countable number of them and each one is real ground.
const GLOBE_LAP_BANDS: usize = 36;

/// The latitude past which going round proves nothing, in radians: the Arctic
/// Circle. See [`Game::update_world_lap`].
const POLAR_LAP_LIMIT: f64 = 66.5 * std::f64::consts::PI / 180.0;

pub fn default_difficulty() -> String {
    "prince".to_string()
}

/// ★★★ THE BARBARIANS PLAY AT THEIR OWN DIFFICULTY — EMPEROR, NOT THE SEAT'S.
///
/// Operator directive 2026-08-24: *"We need to make the barbarians in civvis
/// more aggressive. Should still roughly match the Civ 6 behavior. But weak
/// barbarians in civvis leads us to weak training and favoring the slightly
/// wrong genes … we are playing on level 5 and higher in Civ 6 verification
/// games. Let's make the barbarians level 6 barbarians in civvis for now."*
///
/// The sixth named rung on the ladder is Emperor. Emperor is the last rung in
/// the game's standard `BarbarianAttackForces` band: it assembles two melee
/// and one ranged unit at a `SpawnRate` of 2, while Immortal and Deity switch
/// to the high band. Until now the barbarian seat read its band from the
/// *seat's* difficulty, and every native screen ran at the Prince default, so
/// the setting was not stable for public games. This key is what the
/// barbarian seat plays by, whatever the majors' rung is; the seat difficulty
/// still governs the human's camp Gold and the AI handicaps.
pub fn default_barbarian_difficulty() -> String {
    "emperor".to_string()
}

pub fn default_speed() -> String {
    "standard".to_string()
}

/// The standard turn from which `GameOptions::ai_late_science_pct` applies:
/// turn 100 at Online speed.
pub const AI_LATE_SCIENCE_FROM_STANDARD: u32 = 150;

/// Everything a setup screen gets to choose about a game.
#[derive(Clone, Debug)]
pub struct GameOptions {
    pub players: usize,
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub max_turns: u32,
    pub city_states: usize,
    pub barbarians: bool,
    /// Which published game's rules this world is played by. Civilization VI
    /// is the only one modeled, and the only one this can be.
    pub base_ruleset: BaseRuleset,
    /// How far into history the game opens, as the era of
    /// [`crate::rules::ERA_NAMES`] every civilization starts in: zero is the
    /// stock Ancient start, and anything above it is Civilization VI's
    /// Advanced Start — the earlier eras are already researched. The rungs of
    /// the ladder this is chosen from live in [`crate::setup::START_ERAS`].
    pub start_era: usize,
    /// Which rules the far end of the game is played by. The classic era is
    /// Gathering Storm's own and the default; the modified one puts ore on the
    /// Moon and a mass driver over it. See [`crate::setup::FUTURE_ERAS`].
    pub future_era: FutureEra,
    pub map_script: MapScript,
    /// What shape the world is, asked separately from what fills it: every
    /// world type can be laid out flat or closed into a globe.
    pub map_topology: MapTopology,
    /// Whether the world has cold ends. With poles, latitude runs the climate
    /// and the extremes end in tundra, snow and sea ice; without them the
    /// world is warm from edge to edge.
    pub map_poles: MapPoles,
    pub difficulty: String,
    /// The difficulty the BARBARIAN seat plays by — its raid band and spawn
    /// cadence — independent of `difficulty`. See
    /// [`default_barbarian_difficulty`].
    pub barbarian_difficulty: String,
    pub speed: String,
    pub human_seats: BTreeSet<usize>,
    /// ⭐ MAJOR SEATS THAT PLAY THE RUNG WITHOUT ITS HANDICAP (2026-09-08):
    /// no AI yields, Combat Strength, XP, bonus start units or era boosts,
    /// and none of the below-Prince human bonuses either. The gene screen's
    /// `--handicap rivals` puts every measured seat here so only the rival
    /// chairs play with Emperor's bonuses — the shape the live Civ 6 seat
    /// meets, where WE get nothing and every rival AI gets the rung. A
    /// separate set rather than `human_seats` because a human seat is
    /// also a person's chair everywhere else (`server.rs` hands it to the
    /// browser, `odds.rs` prices it as a human, below Prince it is PAID).
    /// Empty in every save written before this field existed.
    pub handicap_exempt: BTreeSet<usize>,
    /// ⭐ The rung whose YIELD bonus (`ai_yield_pct`) every handicapped AI
    /// seat plays, in place of `difficulty`'s own. `None` — the default — is
    /// the game's rung. Only the percentage moves: the bonus start units, the
    /// era boosts, combat strength and experience stay on the game's rung, and
    /// so does everything else it decides (city-state walls, the AI behaviour
    /// that reads the rung). See `Game::handicap_yield_spec`.
    pub ai_yield_rung: Option<String>,
    /// ⭐ The percentage every handicapped AI seat pays for the four space
    /// projects (Earth Satellite, Moon Landing, Mars Colony, Exoplanet
    /// Expedition), over their ordinary cost. `None` — the default, and every
    /// live game — is 100. A calibration knob for `gene_screen
    /// --rival-victory-pace`: the native rival converts its science into a
    /// launch faster than the live Firaxis AI does. See
    /// `Game::space_project_cost_scale`.
    pub ai_space_project_cost_pct: Option<u32>,
    /// ⭐ The percentage of their ordinary Science every handicapped AI seat
    /// keeps from [`AI_LATE_SCIENCE_FROM_STANDARD`] standard turns on, applied
    /// to the rung's yield bonus. `None` — the default, and every live game —
    /// is 100. A calibration knob for `gene_screen --rival-late-science`: the
    /// native rival out-researches the live Firaxis AI from about turn 100.
    pub ai_late_science_pct: Option<u32>,
    /// ⭐ The techs a handicapped AI major must know before it may start one
    /// of the four space projects. `None` — the default, and every live game —
    /// is no floor. A calibration knob for `gene_screen --rival-space-floor`:
    /// the native rival beelines the space branch and launches its first
    /// satellite knowing ~50 techs, the live Firaxis AI knowing ~59.
    pub ai_space_tech_floor: Option<u32>,
    /// Optional pre-game team assignment for each major seat. An empty vector
    /// means free-for-all; otherwise it must contain exactly `players`
    /// entries. Equal non-`None` values place those seats on one team.
    pub teams: Vec<Option<usize>>,
    /// Gathering Storm's disaster intensity, 0 (none) to 4 (hyperreal).
    pub disaster_intensity: u8,
    /// New Frontier game modes to switch on. Empty — the stock Gathering
    /// Storm ruleset every tournament lobby plays — is the default.
    pub game_modes: BTreeSet<String>,
    /// Civilizations for the leading major seats, in seat order. A setup
    /// screen picks the person's own civilization here; every seat it does
    /// not name is filled from [`Self::leader_pool`], skipping anything
    /// already chosen so no two majors share an identity by accident. Empty
    /// uses that pool's stock order.
    pub civs: Vec<String>,
    /// Roster from which every unnamed seat is filled. Civilization VI leaders
    /// are the default and the only tier with modeled leader/civilization
    /// abilities or unique units; expanded historical and future Today entries
    /// are neutral identities.
    pub leader_pool: LeaderPool,
    /// Shuffle every civilization seat the lobby did not name. Kept opt-in so
    /// simulations and old saves retain their seed-for-seed stock seating;
    /// the live server enables it for newly created games.
    pub randomize_civs: bool,
    /// Whether the seats of one game turn act one after another or plan
    /// against a shared snapshot and commit together. Sequential is the stock
    /// regime and the default everywhere; see [`crate::setup::TurnStructure`]
    /// and [`crate::simultaneous`] for what the alternative changes.
    pub turn_structure: TurnStructure,
    /// The Mercy Rule: end the game the moment one seat's live win odds —
    /// the same calibrated share the spectator ribbon shows, on flat priors —
    /// reach this threshold. `None` plays every game to its natural end.
    /// The setup ladder and the measured winner-agreement behind it live in
    /// docs/ADJUDICATION.md.
    pub mercy_rule: Option<f64>,
    /// Require this many distinct victory types before a winner is declared.
    /// 1 is the stock game. Above it, each achieved victory type banks a
    /// milestone and play continues; the first civilization holding the
    /// required number of different types wins, and the turn limit falls back
    /// to whoever banked the most. Clamped to the number of enabled victory
    /// conditions.
    pub required_victory_types: usize,
    /// Which victory conditions this world plays with, the lobby's own set of
    /// checkboxes.
    ///
    /// ⚠⚠ THIS FIELD DID NOT EXIST AND `Game::new_with` HARDCODED THE DEFAULT,
    /// so `--victories` reached exactly one command. `main.rs` parses and
    /// validates the flag — a bad name is `exit(2)` — and then only the `play`
    /// server's setup carried it through; `simulate`, `soak`, `odds-audit`,
    /// `benchmark`, `rollouts` and `selfplay` all played with all six on
    /// whatever was asked for. A run refused for a typo and then silently given
    /// the wrong game is the expensive shape of that mistake.
    pub victory_conditions: VictoryConditions,
    /// What a Tactics arena grants its two sides. Ignored on a world, which
    /// earns its yields rather than being handed them.
    pub tactics: TacticsRules,
}

impl GameOptions {
    pub fn new(
        players: usize,
        width: i32,
        height: i32,
        seed: u64,
        max_turns: u32,
        city_states: usize,
    ) -> GameOptions {
        GameOptions {
            players,
            width,
            height,
            seed,
            max_turns,
            city_states,
            barbarians: true,
            base_ruleset: BaseRuleset::default(),
            start_era: 0,
            future_era: FutureEra::default(),
            map_script: MapScript::Pangaea,
            map_topology: MapTopology::default(),
            map_poles: MapPoles::default(),
            difficulty: default_difficulty(),
            barbarian_difficulty: default_barbarian_difficulty(),
            speed: default_speed(),
            human_seats: BTreeSet::new(),
            handicap_exempt: BTreeSet::new(),
            ai_yield_rung: None,
            ai_space_project_cost_pct: None,
            ai_late_science_pct: None,
            ai_space_tech_floor: None,
            teams: Vec::new(),
            disaster_intensity: DEFAULT_DISASTER_INTENSITY,
            game_modes: BTreeSet::new(),
            civs: Vec::new(),
            leader_pool: LeaderPool::default(),
            randomize_civs: false,
            turn_structure: TurnStructure::default(),
            mercy_rule: None,
            required_victory_types: 1,
            victory_conditions: VictoryConditions::default(),
            tactics: TacticsRules::default(),
        }
    }
}

/// Seat the majors: the civilizations a setup screen named, in seat order,
/// then the stock roster for everything it left open.
///
/// The seats nobody asked for never collide — with each other or with a
/// chosen name — because two majors sharing a civilization by accident share
/// its unique unit, ability, agenda and city names, which reads as a bug. A
/// lobby that deliberately names the same civilization twice gets it twice:
/// a mirror match is a legitimate thing to set up, and the request is
/// unambiguous in a way an automatic fill never is. Past the end of the
/// selected roster — more majors than there are available leaders — the stock
/// fill starts again at the top.
///
/// A name outside the selected roster is ignored rather than fatal: a stale
/// client or a save naming a retired historical figure gets a valid leader in
/// the current pool instead of taking setup down.
pub fn seat_civs(
    num_players: usize,
    chosen: &[String],
    known: &BTreeSet<Name>,
    leader_pool: LeaderPool,
) -> Vec<String> {
    let leader_pool = leader_pool.available_or_default();
    let roster: Vec<&str> = leader_pool
        .entries()
        .map(|entry| entry.civ.as_str())
        .collect();
    let mut seats: Vec<Option<String>> = vec![None; num_players];
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for (seat, civ) in chosen.iter().enumerate().take(num_players) {
        if known.contains(&Name::new(civ)) && roster.contains(&civ.as_str()) {
            taken.insert(civ.clone());
            seats[seat] = Some(civ.clone());
        }
    }
    let stock: Vec<&str> = roster
        .into_iter()
        .filter(|civ| !taken.contains(*civ))
        .collect();
    assert!(
        !stock.is_empty(),
        "selected leader pool has no known leaders"
    );
    let mut stock = stock.into_iter().cycle();
    seats
        .into_iter()
        .map(|seat| seat.unwrap_or_else(|| stock.next().unwrap().to_string()))
        .collect()
}

/// Seat explicitly chosen civilizations in place, then uniformly shuffle the
/// known stock roster for every open seat. The independent roster RNG keeps
/// map generation stable when random seating is enabled.
pub fn seat_civs_randomized(
    num_players: usize,
    chosen: &[String],
    known: &BTreeSet<Name>,
    leader_pool: LeaderPool,
    rng: &mut Rng,
) -> Vec<String> {
    let leader_pool = leader_pool.available_or_default();
    let roster: Vec<&str> = leader_pool
        .entries()
        .map(|entry| entry.civ.as_str())
        .collect();
    let mut seats: Vec<Option<String>> = vec![None; num_players];
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for (seat, civ) in chosen.iter().enumerate().take(num_players) {
        if known.contains(&Name::new(civ)) && roster.contains(&civ.as_str()) {
            taken.insert(civ.clone());
            seats[seat] = Some(civ.clone());
        }
    }
    let mut stock: Vec<String> = roster
        .into_iter()
        .filter(|civ| known.contains(&Name::new(civ)) && !taken.contains(*civ))
        .map(str::to_string)
        .collect();
    for i in (1..stock.len()).rev() {
        stock.swap(i, rng.below(i + 1));
    }
    // `available_or_default` ensures a usable data pool; this assertion makes
    // a malformed rules/roster integration fail at its source rather than
    // silently inventing a civilization identity.
    assert!(
        !stock.is_empty(),
        "selected leader pool has no known leaders"
    );
    let mut stock = stock.into_iter().cycle();
    seats
        .into_iter()
        .map(|civ| civ.unwrap_or_else(|| stock.next().unwrap()))
        .collect()
}

/// Which families of legal action to enumerate.
///
/// Action families are additive. `CORE` is explicit rather than implicit, so
/// a caller looking only for unit orders or purchases does not also enumerate
/// every upgrade, spy mission, research choice and city strike. Products,
/// corporations and formations are individually small in a new game but
/// scale with every city or pair of units, so repeatedly treating them as
/// free made otherwise narrowed AI queries quadratic in a developed empire.
///
/// `Buy` and `BuyBuilding` are produced by both `PURCHASES` and `EMPIRE` —
/// the latter sells Naturalists, Rock Bands and Great Person patronage — so
/// an agent shopping with a treasury wants both. `BuyPlot` is a city purchase
/// and lives in `PURCHASES` alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ActionFamilies(u16);

impl ActionFamilies {
    /// Unit upgrades, spy missions, fortifying, levies, research and civic
    /// choices, and city strikes.
    pub const CORE: ActionFamilies = ActionFamilies(1);
    /// Everything a unit can be ordered to do: movement, attacks, promotions,
    /// improvements, religious missions, air operations, settling.
    pub const UNITS: ActionFamilies = ActionFamilies(2);
    /// `Produce`, `Buy`, `BuyBuilding`, `BuyDistrict` and `BuyPlot` in each city.
    pub const PURCHASES: ActionFamilies = ActionFamilies(4);
    /// Governments, policies, governors, religion, great people, envoys,
    /// trade routes and WMD strikes — the major-only management block.
    pub const EMPIRE: ActionFamilies = ActionFamilies(8);
    /// Incoming deals, quick offers to make, dedications and congress ballots.
    /// Valuing offers is the single most expensive block of the enumeration.
    pub const DEALS: ActionFamilies = ActionFamilies(16);
    /// Founding an industry-backed corporation on an owned tile.
    pub const CORPORATIONS: ActionFamilies = ActionFamilies(32);
    /// Moving products between slots in the empire's cities.
    pub const PRODUCTS: ActionFamilies = ActionFamilies(64);
    /// Combining Corps/Armies and forming or releasing support or naval escorts.
    pub const FORMATIONS: ActionFamilies = ActionFamilies(128);
    /// Wars, peace treaties, denunciations, missions, demands, promises and
    /// proposed diplomatic agreements against every known rival.
    pub const DIPLOMACY: ActionFamilies = ActionFamilies(256);
    /// The historical "cheap" query: preserve its old core-plus-diplomacy
    /// result while omitting unit orders, purchases and empire management.
    pub const CHEAP: ActionFamilies = ActionFamilies(497);
    pub const ALL: ActionFamilies = ActionFamilies(511);
    /// The empire game: everything a Tactics arena switches off. A
    /// battlefield seats no government, religion, Great People, envoys,
    /// trade or corporations, holds no Congress, and has nothing to
    /// negotiate — its two sides are at war from the first turn and stay
    /// there — so `legal_actions` never enumerates these on an arena and
    /// [`Game::apply`] refuses them there. See [`Action::off_the_battlefield`].
    pub const OFF_THE_BATTLEFIELD: ActionFamilies = ActionFamilies(8 | 16 | 32 | 64 | 256);

    pub fn has(self, family: ActionFamilies) -> bool {
        self.0 & family.0 == family.0
    }

    pub fn without(self, family: ActionFamilies) -> ActionFamilies {
        ActionFamilies(self.0 & !family.0)
    }
}

impl std::ops::BitOr for ActionFamilies {
    type Output = ActionFamilies;

    fn bitor(self, other: ActionFamilies) -> ActionFamilies {
        ActionFamilies(self.0 | other.0)
    }
}

/// What should end a world after somebody chooses "one more turn".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayOnMode {
    /// Keep playing until somebody earns a result other than the exact result
    /// that just stopped the game.
    #[default]
    UntilNextVictory,
    /// Keep the world live no matter which later victory requirements are met.
    Indefinite,
    /// Every civilization left plays on until only one remains. Nothing else
    /// can end the world: every other result is refused and no two
    /// civilizations can be at peace, so the survivor is the only verdict.
    LastCivStanding,
}

impl PlayOnMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "until_next_victory" => Some(Self::UntilNextVictory),
            "indefinite" => Some(Self::Indefinite),
            "last_civ_standing" => Some(Self::LastCivStanding),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::UntilNextVictory => "until_next_victory",
            Self::Indefinite => "indefinite",
            Self::LastCivStanding => "last_civ_standing",
        }
    }
}

/// The result a game most recently played on past. Keeping its exact winner
/// and path prevents a persistent victory condition from immediately stopping
/// the resumed world on the same verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decided {
    pub winner: usize,
    pub victory_type: String,
    /// The lanes a Mercy Rule verdict named, carried off the live game so the
    /// extension can still say what the result it is playing past was decided
    /// on. Empty for every other victory type, and in saves written before the
    /// notation existed.
    #[serde(default)]
    pub mercy_lanes: Vec<String>,
    /// The turn the victory was declared on, not the turn play resumed.
    pub turn: u32,
    /// What ends this continuation: the next distinct result, nothing, or
    /// the last civilization left standing.
    #[serde(default)]
    pub mode: PlayOnMode,
}

impl Decided {
    /// How this past result is written down and shown. See
    /// [`Game::victory_label`].
    pub fn victory_label(&self) -> String {
        if self.victory_type == MERCY_VICTORY {
            mercy_label(&self.mercy_lanes)
        } else {
            self.victory_type.clone()
        }
    }
}

/// The victory type a Mercy Rule ending is recorded under. It answers to the
/// setup option rather than the victory checkboxes, so it is not one of
/// [`VictoryConditions::NAMES`].
pub const MERCY_VICTORY: &str = "mercy";
/// Terminal result recorded when a Tactics deadline expires with both sides
/// still alive. It is a result label, not a victory lane: `winner` remains
/// `None`, and match consumers record a draw.
pub const DRAW_RESULT: &str = "draw";
/// The victory type a capture-the-flag battle is recorded under. Like the
/// Mercy Rule it answers to its own setup option rather than the victory
/// checkboxes, so it is not one of [`VictoryConditions::NAMES`]: the flag is
/// an arena objective, and only a battle set up around one can end this way.
pub const FLAG_VICTORY: &str = "flag";
/// The result of a world played on until one civilization was left. It is not
/// one of [`VictoryConditions::NAMES`] either: no lobby checkbox asks for it,
/// and only "one more turn" can lead here.
pub const LAST_CIV_VICTORY: &str = "last_civ_standing";

/// The notation a Mercy Rule verdict is written and displayed under.
///
/// The rule stops a game the victory conditions were still deciding, so the
/// bare word "mercy" says only that it ended early — never what it ended on.
/// The notation names the lane the winner led when the odds crossed:
/// `Mercy Rule - Science`, and both lanes joined when a seat led two at once.
/// A seat that crossed on standing alone, with no progress in any open lane,
/// keeps the bare rule.
///
/// The joiner is deliberately not a comma. This exact string was written into
/// the `victory` column of the retired league's `matches.csv`, whose readers
/// split rows on commas; the recorded histories still parse only while no
/// comma ever appears here.
pub fn mercy_label(lanes: &[String]) -> String {
    if lanes.is_empty() {
        return "Mercy Rule".to_string();
    }
    let named: Vec<String> = lanes.iter().map(|lane| pretty(lane)).collect();
    format!("Mercy Rule - {}", named.join(" + "))
}

/// Victory paths that can end the game. All paths are enabled by default so
/// existing callers and saves retain the traditional rules unless a new-game
/// setup explicitly disables one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VictoryConditions {
    pub science: bool,
    pub culture: bool,
    pub religious: bool,
    pub diplomatic: bool,
    pub domination: bool,
    pub score: bool,
}

impl VictoryConditions {
    /// The lobby's victory checkboxes, in the order the setup screen lists them.
    pub const NAMES: [&'static str; 6] = [
        "science",
        "culture",
        "religious",
        "diplomatic",
        "domination",
        "score",
    ];

    /// Read a comma-separated list of the paths a lobby leaves enabled. An
    /// unrecognised name is refused rather than quietly switching off the path
    /// it was meant to keep: a misspelt setting that disables every victory
    /// produces a game nobody can win, which is indistinguishable from a
    /// correctly configured one until the turn limit.
    pub fn parse(list: &str) -> Result<VictoryConditions, String> {
        let mut conditions = VictoryConditions {
            science: false,
            culture: false,
            religious: false,
            diplomatic: false,
            domination: false,
            score: false,
        };
        let mut named = 0;
        for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            match name {
                "science" => conditions.science = true,
                "culture" => conditions.culture = true,
                "religious" => conditions.religious = true,
                "diplomatic" => conditions.diplomatic = true,
                "domination" => conditions.domination = true,
                "score" => conditions.score = true,
                other => return Err(format!("unknown victory {other:?}")),
            }
            named += 1;
        }
        if named == 0 {
            return Err("no victory named".to_string());
        }
        Ok(conditions)
    }

    pub fn is_enabled(self, victory_type: &str) -> bool {
        match victory_type {
            "science" => self.science,
            "culture" => self.culture,
            "religious" => self.religious,
            "diplomatic" => self.diplomatic,
            "domination" => self.domination,
            "score" => self.score,
            _ => false,
        }
    }
}

impl Default for VictoryConditions {
    fn default() -> Self {
        Self {
            science: true,
            culture: true,
            religious: true,
            diplomatic: true,
            domination: true,
            score: true,
        }
    }
}

/// The scaffolding roles a simultaneous planning world can play. Not part of
/// the game state: never serialized, never set on a committed game.
///
/// - `Off`: an ordinary, authoritative world. Every rule runs in full.
/// - `Rolling`: the prepare walk's rolling world. Seats take empty turns so
///   each later seat plans with upkeep applied; when the walk wraps out of
///   the cycle the world is discarded, so the wrap's world systems and the
///   next turn's upkeep are skipped — provably unread state.
/// - `Seat`: one seat's private planning copy. The world is discarded the
///   moment its plan is harvested, and the harvest reads only actions logged
///   *before* the closing `EndTurn`, so that close does the minimum needed
///   to move the turn cursor and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanningRole {
    #[default]
    Off,
    Rolling,
    Seat,
}

/// One host combat simulation, as the mod's `preview` event reports it:
/// `CombatManager.SimulateAttackInto`'s `COMBAT_STRENGTH` and `DAMAGE_TO` for
/// both sides, and the defender's `DEFENSE_DAMAGE_TO` (wall damage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostStrikePreview {
    pub attacker_strength: f64,
    pub defender_strength: f64,
    pub damage_to_attacker: i32,
    pub damage_to_defender: i32,
    pub defender_wall_damage: i32,
}

/// A native menu is valid only until the unit consumes a promotion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostBandPromotions {
    pub held: BTreeSet<Name>,
    pub offered: BTreeSet<Name>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "GameSer", into = "GameSer")]
pub struct Game {
    /// Shared with every clone/search branch of this game. A match gets one
    /// immutable snapshot because Gathering Storm randomizes its Future trees
    /// at setup. Native mirrors retain that baseline while replacing revealed
    /// research prerequisites with observations from the host's actual tree.
    pub rules: Arc<Rules>,
    /// Derived from the rest of the game, never saved, and safe to drop at
    /// any moment — it is only ever a shortcut to an answer the sweep would
    /// otherwise recompute.
    #[serde(skip)]
    vision: std::cell::RefCell<VisionCache>,
    /// Compact per-seat current-sight frames, keyed by the inputs that can
    /// change the answer.  Unlike the remembered-map cache this is only a
    /// read shortcut, so it is never serialized.
    #[serde(skip)]
    vision_frames: VisionFrameCache,
    /// Derived connectivity regions, same contract as `vision`: never saved,
    /// rebuilt on demand whenever the world moves under it.
    #[serde(skip)]
    routing: std::cell::RefCell<RoutingCache>,
    #[serde(skip)]
    route_scratch: std::cell::RefCell<RouteScratch>,
    /// Memoized read-only answers; see [`QueryCache`]. Never saved, with
    /// short-lived entries cleared by their guard, production catalogs cleared
    /// by a successful action, and the monopoly census cleared by actions that
    /// can change its inputs; empty in any clone.
    #[serde(skip)]
    query_memo: QueryCache,
    /// `rules` plus the synthetic unknown terrain, shared by every decision
    /// view, beside the ruleset it was built from. Never saved.
    #[serde(skip)]
    view_rules: std::cell::RefCell<Option<(Arc<Rules>, Arc<Rules>)>>,
    /// Raw next-turn attack reaches, shared by controller instances that read
    /// this exact board and by their speculative clones. The cache is keyed
    /// by the same complete board fingerprint that gates AI attack envelopes;
    /// it is never serialized and can always be discarded.
    #[serde(skip)]
    attack_reach_cache: Arc<std::sync::Mutex<AttackReachCache>>,
    /// The state of the world each seat's remembered map was last taken
    /// under — the whole of it, then the narrower part the tile memory is
    /// drawn from — and the tiles it was taken over. Empty means "assume
    /// nothing".
    #[serde(skip)]
    remembered_under: SharedVec<(u64, u64, TileBits)>,
    /// Whether to keep each player's remembered map of fogged tiles and
    /// cities up to date.
    ///
    /// That memory feeds observations and the shared sight allies pool; no
    /// rule and no built-in agent reads it, so a search rolling positions
    /// forward can leave it untouched and reach the same outcome. Explored
    /// ground and Natural-Wonder discovery are kept regardless, because those
    /// do change the game. On by default; a caller that never reads fogged
    /// memory turns it off for a faster clone-and-step.
    #[serde(skip, default = "yes")]
    track_fog_memory: bool,
    /// Whether the narrated war ledger is re-synced after every successful
    /// action, so a spectator's war infobox stays live inside a turn. The
    /// ledger feeds observations and post-game reports, never a rule or a
    /// built-in agent, and every declaration, peace, and turn boundary still
    /// syncs it unconditionally — switching this off only trades mid-turn
    /// freshness for skipping an at_war clone, an uncached suzerain poll of
    /// every minor, and a military_power walk per participant, once per
    /// action. On by default; headless rollouts turn it off.
    #[serde(skip, default = "yes")]
    track_war_ledger: bool,
    /// What this world is *for*. The authoritative game and every sequential
    /// game run `Off`. The simultaneous driver marks its scaffolding worlds —
    /// the rolling prepare world and each seat's private planning copy — so
    /// their `EndTurn` closes can skip work whose results nothing will ever
    /// read. The committed game never carries a role, so nothing here can
    /// reach an authoritative byte; see the `do_end_turn` branches.
    #[serde(skip)]
    planning_role: PlanningRole,
    /// AI turns apply many actions synchronously, with no observer able to
    /// read the half-finished position. Coalesce their full-empire visibility
    /// refreshes and publish the same final observation once at the boundary.
    #[serde(skip)]
    visibility_batch: VisibilityBatch,
    /// A disposable search branch never publishes a player observation. Its
    /// actions still run the full rules, but skipping reveal/contact/snapshot
    /// maintenance keeps those branches from copying fog memory and doing
    /// visibility sweeps that no caller can read.
    #[serde(skip)]
    visibility_suppressed: bool,
    pub rng: Rng,
    pub seed: u64,
    /// Key into `rules.difficulties`. Prince is the unhandicapped reference.
    pub difficulty: String,
    /// Key into `rules.difficulties` for the barbarian seat's raid band and
    /// spawn cadence, independent of the seat's rung; a save without it
    /// plays Emperor barbarians. See [`default_barbarian_difficulty`].
    #[serde(default = "default_barbarian_difficulty")]
    pub barbarian_difficulty: String,
    /// Key into `rules.speeds`. Scales everything bought with a yield.
    pub speed: String,
    /// Mods this game was created under. A save carries them so a mismatched
    /// ruleset can be reported rather than silently changing the rules.
    pub mods: Vec<String>,
    /// Seats played by a person rather than by an agent. Difficulty hands its
    /// bonuses to the AI seats above Prince and to these seats below it; an
    /// all-agent game leaves this empty, which is why headless simulation is
    /// unaffected by the setting unless a seat is declared human.
    pub human_seats: BTreeSet<usize>,
    /// Major seats that take no handicap at all at this rung — neither the
    /// AI's bonuses nor a human's. See `Game::handicap_exempt`. Must be set
    /// here rather than on the built world because the bonus start units
    /// are placed during setup.
    #[serde(default)]
    pub handicap_exempt: BTreeSet<usize>,
    /// The rung whose yield bonus the AI seats play, when it is not the
    /// game's own. See `GameOptions::ai_yield_rung`. `None` in every save
    /// written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_yield_rung: Option<String>,
    /// The space-project cost the handicapped AI seats pay, in percent. See
    /// `GameOptions::ai_space_project_cost_pct`. `None` in every save written
    /// before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_space_project_cost_pct: Option<u32>,
    /// The late Science the handicapped AI seats keep, in percent. See
    /// `GameOptions::ai_late_science_pct`. `None` in older saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_late_science_pct: Option<u32>,
    /// The techs a handicapped AI major needs before a space project. See
    /// `GameOptions::ai_space_tech_floor`. `None` in older saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_space_tech_floor: Option<u32>,
    /// The published game whose rules this world is played by. Kept on the
    /// save alongside the rest of the setup so a restart offers what was
    /// actually played, rather than today's default.
    #[serde(default)]
    pub base_ruleset: BaseRuleset,
    /// How far into history this world opened, as an era of
    /// [`crate::rules::ERA_NAMES`]. Kept on the save alongside the rest of the
    /// setup so a restart offers what was actually played.
    #[serde(default)]
    pub start_era: usize,
    /// Which rules the far end of this game is played by. Kept on the save
    /// because it selects the ruleset itself: a match loaded on a later build
    /// must be rebuilt on the rules it was started under.
    #[serde(default)]
    pub future_era: FutureEra,
    /// Whether this game's seats act sequentially or plan simultaneously.
    /// Kept on the save so a resumed game keeps the regime it was played
    /// under, and so a finished run's record says which game it was. It does
    /// not change `Game::apply` semantics — only how a driver consults the
    /// seats — which is why nothing else in this struct reads it.
    #[serde(default)]
    pub turn_structure: TurnStructure,
    /// The Moon's ore, by resource: one body, one set of piles, shared by
    /// everybody who gets a mass driver over it. Rolled at setup only when the
    /// ruleset gives a resource a lunar deposit, so a classic game carries an
    /// empty map here and never looks at the Moon at all.
    #[serde(default)]
    pub moon_deposits: BTreeMap<Name, f64>,
    pub map_script: MapScript,
    /// Random-leader roster chosen when this world was created. It remains on
    /// the save so a restart can faithfully offer the same setup.
    pub leader_pool: LeaderPool,
    /// Whether the world was rolled with cold ends. The shape it was rolled on
    /// is on the map itself (`map.topology`), and the climate this chose is
    /// baked into the terrain — this is kept so that a loaded game can still
    /// say what it was set up with, and offer the same world again.
    pub map_poles: MapPoles,
    pub game_speed: GameSpeed,
    pub max_turns: u32,
    pub turn: u32,
    pub current: usize,
    pub winner: Option<usize>,
    pub victory_type: Option<String>,
    /// The result of the game that was already decided, kept when somebody
    /// chose to play on past it. History still has its victor even though the
    /// world is live again; `winner` is `None` while the extension runs.
    #[serde(default)]
    pub decided: Option<Decided>,
    pub victory_conditions: VictoryConditions,
    /// The Mercy Rule threshold this world was set up with, if any. Absent
    /// in saves written before the rule existed, all of which play to the
    /// natural end.
    #[serde(default)]
    pub mercy_rule: Option<f64>,
    /// The open victory lanes the Mercy Rule's winner led when the odds
    /// crossed, read at the crossing because the board that produced them
    /// stops changing the moment the game does. Empty for every other ending
    /// and for a mercy crossing on a seat with no race progress at all; see
    /// [`mercy_label`] for what it is written as.
    #[serde(default)]
    pub mercy_lanes: Vec<String>,
    /// How many distinct victory types a winner must hold. Zero or one — and
    /// every save written before the option existed — is the stock game; see
    /// [`Self::effective_required_victories`].
    #[serde(default)]
    pub required_victory_types: usize,
    /// What this arena grants its two sides each turn. Meaningless on a
    /// world, and absent from saves written before the arena had an economy —
    /// those load as the stock grant, which no world ever reads.
    #[serde(default)]
    pub tactics: TacticsRules,
    /// Where each side's flag stands on a capture-the-flag arena, by the
    /// seat that owns it.
    ///
    /// A flag stands where that side's city would have stood, because it is
    /// what the city was replaced by: the thing the side holds and the other
    /// side is trying to take. Taking an **enemy** flag's tile wins the
    /// battle outright; standing on your own does nothing, which is what
    /// makes defending one meaningful. Empty on every world and on every
    /// arena whose match did not ask for flags, and on saves written before
    /// the shape existed.
    #[serde(default)]
    pub arena_flags: BTreeMap<usize, Pos>,
    /// The victory types each seat has banked under Require-N, by player id.
    #[serde(default)]
    pub victories_won: BTreeMap<usize, BTreeSet<String>>,
    pub next_id: u32,
    pub map: WorldMap,
    pub players: Players,
    pub units: Units,
    pub spies: BTreeMap<u32, Spy>,
    pub cities: Cities,
    pub at_war: BTreeSet<(usize, usize)>,
    /// Military scores reported by an authoritative host, keyed by CIVVIS seat.
    ///
    /// Ordinary games leave this empty and derive strength from their units. A
    /// live mirror cannot do that for a met rival whose army is currently under
    /// fog, even though Civilization VI exposes that rival's aggregate military
    /// score to the player. Keeping the host score here lets the ordinary AI use
    /// the public information without inventing hidden unit positions.
    #[serde(default)]
    pub observed_military_power: Arc<BTreeMap<usize, f64>>,
    /// Per-unit facts an authoritative host exported about units in a live
    /// mirror — the upgrade verdict and bill, the per-type upkeep, the movement
    /// allowance, a Spy's operation and menu. Own units and visible foreign
    /// units may carry different subsets. See [`HostUnitFacts`] for which
    /// decision reads each. Empty in native games.
    #[serde(default)]
    pub host_unit_facts: Arc<BTreeMap<u32, HostUnitFacts>>,
    /// The host treasury's bill by source for mirrored seats, read by
    /// [`Game::unit_gold_maintenance`] and
    /// [`Game::infrastructure_gold_maintenance`] in place of the board's own
    /// sums. Empty in native games.
    #[serde(default)]
    pub host_maintenance: Arc<BTreeMap<usize, HostMaintenance>>,
    /// Public host score for mirrored seats. Empty in native CIVVIS games.
    #[serde(default)]
    pub observed_score: Arc<BTreeMap<usize, i64>>,
    /// Wars between two met rivals as the live host reports them (each
    /// rival's `at_war_with`), as `(low, high)` seat pairs. Kept apart from
    /// `at_war`, which every war reader of the planner consults, so the host
    /// fact reaches only the readers that ask for it
    /// ([`Game::rivals_at_war`]). Empty in native CIVVIS games.
    #[serde(default)]
    pub observed_rival_wars: Arc<BTreeSet<(usize, usize)>>,
    /// Current host Palace locations, distinct from original capitals required
    /// for Domination. Empty in simulated games and older host snapshots.
    #[serde(default)]
    pub observed_city_palaces: Arc<BTreeMap<u32, bool>>,
    /// Cities whose imported pressure values encode majority and conversion
    /// warnings rather than measured accumulated pressure. Spreading and
    /// religious combat cannot forecast a conversion from these markers.
    #[serde(default)]
    pub observed_city_religion_markers: Arc<BTreeSet<u32>>,
    /// The religion rows a live host's city banner showed for each mirrored
    /// city: followers, pressure per turn and lifetime pressure. Empty in
    /// simulated games, whose `City::pressure` is already the real figure.
    #[serde(default)]
    pub observed_city_faiths: Arc<BTreeMap<u32, Vec<ObservedCityFaith>>>,
    /// Host gross strategic income minus the reconstructed board's income.
    /// Corrections preserve policy and improvement counterfactuals on clones.
    /// Empty in simulated games; refreshed on each host snapshot.
    #[serde(default)]
    pub observed_strategic_income_adjustments: Arc<BTreeMap<usize, BTreeMap<Name, f64>>>,
    /// Exact host trade-route capacity for mirrored seats. Native games derive
    /// this from their own infrastructure and leave the map empty.
    #[serde(default)]
    pub observed_trade_capacity: Arc<BTreeMap<usize, i64>>,
    /// Exact Firaxis leader type for mirrored seats. CIVVIS models one leader
    /// per civilization, while Civ VI can seat alternates and personas; keeping
    /// the host type separate preserves identity without claiming their rules
    /// have been folded into the modeled civilization.
    #[serde(default)]
    pub observed_leader_types: Arc<BTreeMap<usize, String>>,
    /// Host-to-model yield corrections for mirrored empires. Native games leave
    /// this empty. A correction, rather than an absolute replacement, preserves
    /// counterfactual deltas when the AI evaluates a policy or build on a clone.
    #[serde(default)]
    pub observed_yield_adjustments: Arc<BTreeMap<usize, crate::rules::Yields>>,
    /// Public empire-wide figures from an authoritative host which cannot be
    /// reconstructed from fog-limited city and unit records. Native games leave
    /// this empty and derive every standing from their full board.
    #[serde(default)]
    pub observed_public_empire_stats: Arc<BTreeMap<usize, ObservedPublicEmpireStats>>,
    /// Seats known to own cities omitted from this partial board. Existence
    /// alone prevents speculative casualties from falsely eliminating them.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unseen_city_owners: BTreeSet<usize>,
    /// The religion a majority of a seat's cities follow, as the host reports
    /// it for every met major (`GetReligionInMajorityOfCities`, the test the
    /// shipped religion tab runs to call a civilization converted), keyed by
    /// seat. [`Game::majority_religion_of`] prefers it to counting the cities
    /// the board holds, which for a rival are only the ones in view. Empty on
    /// a native game.
    #[serde(default)]
    pub observed_majority_religion: Arc<BTreeMap<usize, String>>,
    /// Tourists one seat's culture draws from another as the host counts them
    /// (`GetTouristsFrom` on the source's culture), keyed (tourism source,
    /// where the tourists come from) — the order `visiting_tourists_from`
    /// takes. The mirror records our seat's draw from each met rival, the
    /// per-rival term of `foreign_tourists`. Empty on a native game.
    #[serde(default)]
    pub observed_visiting_tourists: Arc<BTreeMap<(usize, usize), i64>>,
    /// Per-city host-to-model corrections and exact citizen assignments for a
    /// mirrored Firaxis turn. Native games leave these empty. Corrections are
    /// additive so counterfactual buildings, policies, and assignments still
    /// contribute their modeled delta instead of being hidden by an override.
    #[serde(default)]
    pub observed_city_yield_adjustments: Arc<BTreeMap<u32, crate::rules::Yields>>,
    /// Per-city host-to-model Amenity-surplus corrections for a mirrored
    /// Firaxis turn. Native games leave this empty. Like yield corrections,
    /// these are additive: a planned Arena, Luxury, or policy still changes
    /// the projected surplus instead of being masked by a frozen host value.
    #[serde(default)]
    pub observed_city_amenity_adjustments: Arc<BTreeMap<u32, i64>>,
    /// Per-city host-to-model Housing corrections for a mirrored Firaxis
    /// turn, the Amenity map's twin: `host - model`, added inside
    /// [`Game::city_housing`] so the board reads the host's ceiling while a
    /// planned Granary or Aqueduct still moves the projection by its modeled
    /// amount. Native games leave this empty.
    #[serde(default)]
    pub observed_city_housing_adjustments: Arc<BTreeMap<u32, f64>>,
    /// Per-plot host-to-model yield corrections for a mirrored Firaxis turn:
    /// `Plot:GetYield` minus CIVVIS's own tile model, on the plots the host
    /// reports worked. Added inside [`Game::workable_tile_yields`], so the
    /// board pays a tile what the host pays it — including the terms only the
    /// host can know, such as the fertility an eruption or a flood left on the
    /// ground (Rome on run civvis-20260816T003229Z read **+12 Food** over the
    /// model for forty turns on volcanic soil) — while a counterfactual
    /// improvement still moves the tile by its modeled amount. Native games
    /// leave this empty.
    #[serde(default)]
    pub observed_tile_yield_adjustments: Arc<BTreeMap<Pos, crate::rules::Yields>>,
    /// Trading Posts on the host's own path for one of our routes, keyed by
    /// (origin, destination) city and filed (own cities, foreign cities). The
    /// host's pathfinder follows roads the model's straight-line walk does
    /// not; where the mirror knows the host's answer it is the one that pays
    /// (`trading_post_route_gold`). Empty on a native game.
    #[serde(default)]
    pub observed_route_posts: Arc<BTreeMap<(u32, u32), (i64, i64)>>,
    /// Where the host houses this seat's Great Works, by city and kind, when
    /// the mirror has read it: the host's own placement is the one that
    /// pays, not the model's best-slot heuristic (a Relic the host kept in
    /// Rome's Palace read "+6 from GreatWorks" there while the model housed
    /// it in Mediolanum's St. Basil's, run civvis-20260816T233226Z t154+).
    /// `None` on a native game or an export without the works.
    #[serde(default)]
    pub observed_great_work_housing: Option<Arc<BTreeMap<u32, BTreeMap<String, usize>>>>,
    /// What the host says one of our routes pays its ORIGIN, keyed by
    /// (origin, destination) city — the Trade Overview's own sum of route,
    /// path and modifier yields under the international multiplier. Stands
    /// in for the model's route yield where present; empty on a native game.
    #[serde(default)]
    pub observed_route_yields: Arc<BTreeMap<(u32, u32), crate::rules::Yields>>,
    /// Host-to-model incoming-route count deltas, keyed by destination city
    /// and stored as `(foreign, domestic)`. A foreign route whose origin city
    /// is still behind fog cannot be represented in `Game::routes`; this
    /// correction preserves the host's destination-side count while keeping
    /// counterfactual route additions/removals modeled. Empty on a native game.
    #[serde(default)]
    pub observed_incoming_route_deltas: Arc<BTreeMap<u32, (i64, i64)>>,
    #[serde(default)]
    pub observed_city_worked_tiles: Arc<BTreeMap<u32, Vec<Pos>>>,
    #[serde(default)]
    pub observed_city_specialists: Arc<BTreeMap<u32, Vec<String>>>,
    /// Observed loyalty rates for owned cities, keyed by CIVVIS city id.
    /// Populated by the live mirror and native player decision views.
    #[serde(default)]
    pub observed_city_loyalty_per_turn: Arc<BTreeMap<u32, f64>>,
    /// Current local-player settler-lens nearby-city pressure. Used only for
    /// prospective settlements; an absent plot is not a zero-pressure reading.
    #[serde(default)]
    pub observed_settlement_loyalty: Option<Arc<NativeSettlementPressure>>,
    /// Observed banner defense strengths for reconstructed cities.
    #[serde(default)]
    pub observed_city_strength: Arc<BTreeMap<u32, f64>>,
    /// Visible host city ranged strength, already including native modifiers.
    #[serde(default)]
    pub observed_city_ranged_strength: Arc<BTreeMap<u32, f64>>,
    /// Host-reported outer-defense capacity for mirrored cities. Native games
    /// derive this from wall buildings and leave the override empty.
    #[serde(default)]
    pub observed_city_max_wall_hp: Arc<BTreeMap<u32, i32>>,
    /// `breach-support-reads-the-wall-tier`: read a mirrored city's wall tier
    /// from its observed wall pool when deciding whether a Battering Ram or a
    /// Siege Tower works on it. A rival city's record carries its wall bars,
    /// not its buildings, so `city_building_effect` read no immunity from
    /// Medieval or Renaissance Walls it could not see. Set by the seat each
    /// turn; native games leave the observed pool empty, so it changes nothing
    /// there.
    #[serde(default)]
    pub observed_wall_tier_rules: bool,
    /// `trade-quotes-pay-the-lump`: the seat whose own quick-deal quotes are
    /// paid as one rounded-up Gold lump whenever the payer's treasury covers
    /// it, instead of the floored lump plus a per-turn sliver that
    /// `quoted_payment` checks against `empire_gold_per_turn`. That proxy (4
    /// Gold a city less unit and building upkeep, floored at zero) reads zero
    /// from about turn 100 for a built-up empire whose real income is 30-200,
    /// so every fractional quote it is party to vanishes. Set by the seat each
    /// turn; `None` changes nothing.
    #[serde(default)]
    pub lump_quote_viewer: Option<usize>,
    /// `nusantara-reads-the-coast`: the Indonesian seat whose district site
    /// pickers price Great Nusantara's Coast adjacency
    /// ([`Game::district_site_yields`]). Planning only: real yields never read
    /// it. Set by the seat each turn; `None` changes nothing.
    #[serde(default)]
    pub nusantara_site_planner: Option<usize>,
    /// `builders-route-around-mountains`: a Builder never enters a mountain,
    /// whatever the Ski Resort unlock. The host's move pathing put none of
    /// ours on a mountain in 984 Builder-turns after Professional Sports over
    /// two 10-13 games and refused every move that ended on one, while the
    /// board routed Builders across them. Set by the seat each turn.
    #[serde(default)]
    pub observed_builders_avoid_mountains: bool,
    /// `culture-denial-heist`: a Great Work heist the HOST lists on a spy's
    /// operation menu is offered, though the board cannot see the works. The
    /// mirror carries only our own Great Works, so a rival city's
    /// `spy_city_has_stealable_work` always read false and the heist was
    /// never legal live: the host offered it for 8-14 turns in the culture
    /// winner's city in three of today's eight culture losses
    /// (civvis-20261008T062856Z St. Petersburg, T083432Z Tokyo, T090439Z
    /// Mistahi-Sipihk) and it was never sent. Set by the seat each turn;
    /// native games have no host menu, so it changes nothing there.
    #[serde(default)]
    pub heist_reads_the_host_menu: bool,
    /// `spies-steal-the-lead`: a district mission the host offers at a plot
    /// (`HostUnitFacts::spy_targets`) is offered there though the board has
    /// not seen the district. The board learns a rival Campus only once the
    /// tile is seen, so a host-offered Steal Tech Boost went unsent (live
    /// 080206Z t145 and 073507Z t150: steal on the host menu, Listening Post
    /// sent). Set by the seat each spy pass; native games have no host
    /// targets, so it changes nothing there.
    #[serde(default)]
    pub spy_targets_read_the_host: bool,
    /// Sites a HOST ruleset forbids for a reason CIVVIS's own rules cannot see.
    ///
    /// ★★★★ Empty in an ordinary game, and load-bearing when CIVVIS is driving a
    /// foreign engine. Civilization VI refused 141 `FOUND_CITY` orders in one run and
    /// 18 in another while the site picker re-chose the SAME tile at turns 20, 33 and
    /// 79 — because nothing carried the refusal back, so every turn re-derived the
    /// same answer from the same board and re-sent the same rejected order. Peak city
    /// count was 2 in every run of the ladder.
    ///
    /// The fix is the one this whole bridge keeps needing: stop lying to CIVVIS. A
    /// site the host rejected is recorded here, `can_found_city` honours it, and the
    /// planner routes around it on its own. Filtering the order at the bridge instead
    /// would only have made the turn do nothing — CIVVIS would never learn to pick
    /// somewhere else.
    #[serde(default)]
    pub blocked_city_sites: Arc<BTreeSet<Pos>>,
    /// Tiles a HOST ruleset will not let a builder improve, for the same reasons and
    /// with the same emptiness in an ordinary game as [`Game::blocked_city_sites`].
    ///
    /// ★★★★ The largest refusal category measured: **311 `IMPROVEMENT_MINE`
    /// refusals in one run**, 51 and 31 in others. CIVVIS names improvements from ITS
    /// terrain model and the two rulesets do not agree tile for tile, so a tile it
    /// believes is a mine site simply stays bare — and because the mirror then keeps
    /// reporting an undeveloped empire, CIVVIS orders another builder. One run ended
    /// with seven builders alive against an army of one.
    #[serde(default)]
    pub blocked_improvement_sites: Arc<BTreeSet<Pos>>,
    /// Improvements a HOST ruleset refused on one tile, by name. A refusal that
    /// names its improvement condemns that improvement there, not the ground:
    /// only a refusal without a name lands in
    /// [`Game::blocked_improvement_sites`].
    ///
    /// ★★★ ONE REFUSED ALCÁZAR HID THE EMPIRE'S ONLY ALUMINUM. Live King
    /// 20261001T042554Z: a Builder asked for an Alcázar on the Farm at
    /// (31,29) on turn 112 and the host refused it. The tile was blocked for
    /// every improvement for the rest of the game, so when Radio revealed
    /// Aluminum under that Farm and Advanced Flight landed on turn 160, the
    /// Mine was never valid: eight Builders stood idle, the Bomber wing was
    /// stood down "no Aluminum" on turn 174, and the Mine went in on turn
    /// 203 only because a restarted bridge had lost the old refusal.
    #[serde(default)]
    pub blocked_improvements: Arc<BTreeMap<Pos, BTreeSet<Name>>>,
    /// ★★★★ GROUND A GREAT PERSON STANDS ON, keyed to its owner. Empty in an
    /// ordinary game; the live mirror fills it because CIVVIS does not model
    /// Great People as units and drops them from the board (see
    /// `mirror::is_great_person`), so a plot the host has occupied read as EMPTY.
    ///
    /// Civilization VI stacks one civilian per plot and a Great Person is a
    /// civilian, so a Builder or Settler cannot end a move on one — exactly as
    /// this engine treats a plot holding a friendly unit of the same class — and
    /// a foreign Great Person blocks every unit, at war or not (Firaxis
    /// captures no Great Person; live King civvis-20261004T160213Z). On
    /// run `civvis-20260816T003229Z` the founded, zero-charge Prophet stood on
    /// (7,25) beside the capital from turn 73 to the end; a Builder with two
    /// charges left was ordered `MOVE_TO (7,25)` on 25 consecutive turns
    /// (t175–t199) and never moved, because CIVVIS chose that tile as its
    /// improvement target and Firaxis refused the step every time. Nothing on
    /// the board could tell it otherwise.
    ///
    /// Honoured in the two places the board answers "can a unit be here":
    /// [`Game::valid_improvements`] offers nothing on such a plot (the same
    /// contract as `blocked_improvement_sites` — a tile a builder cannot reach
    /// must offer nothing, or the builder loops on it), and `can_enter_past`
    /// refuses the step for the layers Firaxis refuses it for.
    #[serde(default)]
    pub great_person_plots: BTreeMap<Pos, usize>,
    /// ★★★★★ PROMOTIONS THE HOST REFUSED, so CIVVIS stops asking for them again.
    ///
    /// Every other host refusal already had a block set — `blocked_city_sites`,
    /// `blocked_improvement_sites`, `blocked_trade_routes`, `blocked_policies`,
    /// `blocked_districts`, `blocked_wonders`, `blocked_production` — and
    /// promotions had NONE, so a refused promotion was re-requested every turn
    /// for the rest of the game.
    ///
    /// Measured over every run recorded on 2026-08-03: **411 `promotion_refused`
    /// events from only 19 distinct (unit, promotion) pairs — a median of 13
    /// retries each and a maximum of 71.** Only 11% were asked once. The two
    /// populations:
    ///
    /// - **318 on UNIT_APOSTLE** (Translator, Chaplain, Debater, Indulgence
    ///   Vendor, Proselytizer). A Civilization VI Apostle takes ONE promotion when
    ///   it is created and can never be promoted again, so every repeat was
    ///   guaranteed to fail from the first refusal onward.
    /// - **93 `PROMOTION_ECHELON` on UNIT_PIKE_AND_SHOT.**
    ///
    /// Keyed by unit id and promotion, because the refusal is specific to both:
    /// another unit of the same kind may legitimately take the promotion this one
    /// cannot.
    #[serde(default)]
    pub blocked_promotions: Arc<BTreeMap<u32, BTreeSet<Name>>>,
    /// Current native Rock Band promotion offers; absent means unobserved.
    #[serde(default)]
    pub host_band_promotions: Arc<BTreeMap<u32, HostBandPromotions>>,
    /// ★★★ STRIKES THE HOST REFUSED THIS TURN, so a later frame of the same
    /// turn does not propose the identical shot again.
    ///
    /// `(attacker, target)` in CIVVIS unit ids and axial tiles. A live mirror
    /// fills it per turn from the mod's `range_attack_refused` (line of sight,
    /// range, spent attacks — the host's own `why`) and `war_refused` (a strike
    /// the engine would answer with a war the agent never declared) events;
    /// native games leave it empty. Gated where `blocked_promotions` is: at
    /// the one legality each chooser and the applier itself pass through
    /// (`legal_actions_within`'s strike loops, `ranged_order_is_legal`,
    /// `melee_order_is_legal`, `do_attack`, `do_ranged`), so a pre-pass that
    /// proves a strike on a private board (`live_finishing_candidates`) is
    /// refused the same way the picker is.
    #[serde(default)]
    pub blocked_strikes: Arc<BTreeSet<(u32, Pos)>>,
    /// Explicit host declaration refusals, keyed by (actor, target, observed
    /// turn). They mask voluntary war actions only on that observed turn;
    /// hypothetical later turns and games without host facts use normal rules.
    #[serde(default)]
    pub host_war_blocks: Arc<BTreeSet<(usize, usize, u32)>>,
    /// Exact native permissions for unilateral war types, refreshed per board.
    /// Present observations require an explicit true for the selected type;
    /// missing observations and later simulated turns use ordinary rules.
    #[serde(skip)]
    pub host_war_type_permissions: Arc<BTreeMap<(usize, usize), ObservedWarTypes>>,
    /// Native envoy permissions for (actor, city-state), with observed turn.
    /// Only that turn uses the host answer; simulations without it retain
    /// their own rules. Contact, a living minor and an available envoy remain
    /// mandatory even when the host permits placement.
    #[serde(default)]
    pub host_envoy_permissions: Arc<BTreeMap<(usize, usize), (u32, bool)>>,
    /// ★★★ THE HOST'S OWN PRICE OF A STRIKE, asked for this turn and answered
    /// without fighting it. Keyed `(attacker, target, ranged)` in CIVVIS unit
    /// ids and axial tiles; the value is Civilization VI's
    /// `CombatManager.SimulateAttackInto` result as the mod's `preview` event
    /// carries it (`CivvisLedger.preview`, answering a `preview` order). Per
    /// turn, from the run's events; native games leave it empty. Read through
    /// [`Game::host_preview`]; nothing else consumes it yet.
    #[serde(default)]
    pub host_previews: Arc<BTreeMap<(u32, Pos, bool), HostStrikePreview>>,
    /// Origin/destination pairs a host engine rejected for a trade route.
    /// Native games leave this empty; a live mirror learns it from Firaxis so an
    /// unreachable city is not offered again every turn on geometric range alone.
    #[serde(default)]
    pub blocked_trade_routes: Arc<BTreeSet<(Pos, Pos)>>,
    /// Policy cards a HOST ruleset has retired, for the same reasons and with the
    /// same emptiness in an ordinary game as [`Game::blocked_city_sites`].
    ///
    /// ★★★★ Measured on live run `civvis-20260801T012454Z`: `POLICY_ILKUM` was chosen
    /// and refused **105 times**, `DISCIPLINE` 6 and `AGOGE` 1 — **112 of 813
    /// refusals**, third behind movement and `no_params`. Civilization VI knew every
    /// time (`culture:IsPolicyObsolete`) and said so in the refusal reason; nothing
    /// carried it back, so CIVVIS re-derived the same card from the same board on
    /// almost every turn of the game.
    ///
    /// [`Game::available_policies`] already computes an obsolete set from each card's
    /// `replaces` list. That is the right mechanism and it is under-populated — 22 of
    /// 41 shipped rows carry no successor — but **filling it in is what PR #703 is
    /// blocked on**: the rows are ruleset data, so changing them moves
    /// `Rules::source_fingerprint` and the Elo ledger rejects new games at bind time.
    ///
    /// ⚠ This set deliberately does NOT go near that. It is per-game reconstruction
    /// state carried beside the board, exactly like the two above, so the fingerprint
    /// does not move and no rated game is invalidated. It fixes the mirrored game
    /// without pretending to fix the ruleset — and an ordinary CIVVIS game leaves it
    /// empty, so nothing about simulated play changes.
    #[serde(default)]
    pub blocked_policies: Arc<BTreeSet<Name>>,
    /// Current policy eligibility observed from a host, scoped to its mirrored
    /// seat. Unlike historical refusals this is replaced on each observation:
    /// government changes and Congress bans can make a card available again.
    /// An absent seat entry preserves ordinary simulation rules.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub host_policy_choices: BTreeMap<usize, BTreeSet<Name>>,
    /// A native policy transaction window, scoped to its observed seat and
    /// turn. Absent in ordinary games and legacy host snapshots.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub host_policy_windows: BTreeMap<usize, HostPolicyWindow>,
    /// Pantheon beliefs a HOST has already granted to another player, for the same
    /// reasons and with the same emptiness in an ordinary game as
    /// [`Game::blocked_city_sites`].
    ///
    /// The mirror seats no rival pantheons — the export lists founded religions
    /// only (`Game.GetReligion():GetReligions()` minus the pantheon rows), so
    /// `do_choose_pantheon`'s "belief already taken" check sees only our own. A
    /// first choice a rival holds is refused by the mod as `taken_BELIEF_<X>`, and
    /// nothing carried that back: the same belief would be re-derived from the same
    /// board next turn, and after two sightings the mod's own blocker fallback
    /// picks the FIRST untaken pantheon in database order (Dance of the Aurora on
    /// the recorded runs). Harvested from the refusal stream by the mirror
    /// (`refused_pantheons`); an ordinary CIVVIS game leaves it empty. See
    /// `AdvancedAi::expansion_pantheon` for the choice this protects.
    #[serde(default)]
    pub blocked_pantheons: Arc<BTreeSet<Name>>,
    /// Districts a HOST ruleset will not place, per city, for the same reasons and
    /// with the same emptiness in an ordinary game as [`Game::blocked_city_sites`].
    ///
    /// ★★★★ Measured on live run `civvis-20260801T024428Z`: `DISTRICT_GOVERNMENT`
    /// refused **24** times by turn 115 and `build_no_plot` fired **39** times, every
    /// one of them the same district. Civilization VI offers no plot for a Government
    /// Plaza once one exists — it is one per civilization — and CIVVIS re-chose it
    /// from the same board turn after turn. Each discard leaves the city with nothing
    /// queued, so `ENDTURN_BLOCKING_PRODUCTION` fires and the hand-written ladder
    /// picks instead; on that run only 11 of 55 builds were CIVVIS's.
    ///
    /// ⚠⚠ **KEYED BY CITY, AND THAT IS THE WHOLE CARE IN THIS FIX.** The host refuses
    /// a district for two opposite reasons: it is impossible *anywhere* (Government
    /// Plaza), or *this* city has no room. A global set would stop CIVVIS building
    /// Campuses across the empire the first time one city ran out of space — trading a
    /// small waste for a large one. The mod now sends the city id with the refusal so
    /// the block can be scoped; before this it sent only a bare hash.
    #[serde(default)]
    pub blocked_districts: Arc<BTreeMap<u32, BTreeSet<Name>>>,
    /// Fresh district plots Firaxis explicitly approved after rejecting a different
    /// CIVVIS coordinate. Native games leave this empty. A positive host candidate
    /// wins over the accompanying temporary refusal, so the next planner decision
    /// can select a real placement instead of waiting out the cooldown.
    #[serde(default)]
    pub host_district_sites: Arc<BTreeMap<u32, BTreeMap<Name, BTreeSet<Pos>>>>,
    /// Fresh wonder plots Firaxis explicitly approved after rejecting a different
    /// CIVVIS coordinate. Like [`Game::host_district_sites`], this is bridge-only
    /// feedback: a positive host answer wins over its paired temporary refusal so
    /// the next decision can start the same wonder on legal ground.
    #[serde(default)]
    pub host_wonder_sites: Arc<BTreeMap<u32, BTreeMap<Name, BTreeSet<Pos>>>>,
    /// Wonders a HOST engine says have no legal plot IN THIS CITY, scoped for the
    /// same reason as [`Game::blocked_districts`] — Hanging Gardens needs a river
    /// and Great Bath floodplains, so one city having no ground says nothing about
    /// the next.
    ///
    /// ★★★★ The wonder half of `build_no_plot` was being DROPPED. That parser reads
    /// only the event's `district` key, and the mod emits a refused wonder under
    /// `building`, so every wonder refusal fell through it silently. Measured over
    /// 20 live runs: **370 wonder refusals against 55 district ones**, from only 29
    /// distinct (run, city, wonder) combinations — a mean of 12.8 re-asks each and
    /// **53 consecutive turns** at worst of one city ordering one wonder Firaxis had
    /// no ground for. `HANGING_GARDENS` 159, `GREAT_BATH` 129, `TEMPLE_ARTEMIS` 45.
    #[serde(default)]
    pub blocked_wonders: Arc<BTreeMap<u32, BTreeSet<Name>>>,
    /// World-unique wonders the live host has ruled out everywhere.
    ///
    /// `blocked_wonders` is deliberately city-local: an ordinary zero-site answer
    /// can mean this city lacks a river or floodplain. But the control bridge also
    /// records the host's number of offered locations. An explicit `offered: 0` for
    /// a model-legal wonder means Firaxis has no target anywhere, most commonly
    /// because an unseen rival already completed that world unique. Carry that
    /// stronger, permanent live-only fact separately so the next developed city
    /// does not spend a production decision rediscovering it. Native games leave
    /// the set empty.
    #[serde(default)]
    pub host_unavailable_wonders: Arc<BTreeSet<Name>>,
    /// Production choices a HOST ruleset has refused in a particular city.
    ///
    /// The bridge keeps these blocks on a short cooldown: a missing prerequisite or
    /// temporary host rule must be allowed to become legal later, while an item the
    /// host has just rejected must not be selected from the same reconstructed board
    /// on every turn. Keys are typed (`building:library`, `unit:warrior`, ...), so a
    /// name shared by two production tables cannot suppress the wrong kind of item.
    #[serde(default)]
    pub blocked_production: Arc<BTreeMap<u32, BTreeSet<String>>>,
    /// Purchases a HOST ruleset has recently refused in a particular city.
    ///
    /// This is deliberately separate from [`Game::blocked_production`]. A host may
    /// reject buying a unit while still allowing the city to build it, and merging
    /// the two would turn an actuation mismatch into the exact production starvation
    /// the feedback is meant to repair. Ordinary CIVVIS games leave this empty.
    #[serde(default)]
    pub blocked_purchases: Arc<BTreeMap<u32, BTreeSet<String>>>,
    /// The HOST's production menu per city, keyed like `blocked_production`
    /// (`unit:warrior`, `formation:warrior:1`, `building:library`,
    /// `wonder:pyramids`, `district:campus`, `project:...`): every item the
    /// engine's `BuildQueue:CanProduce(hash, false, true)` says this city can
    /// START now, with its cost and turns (`StateCity::buildable`). Empty in
    /// an ordinary game and on an export without the key, so neither changes.
    /// When a city has an entry, [`Game::can_produce`] refuses any unit,
    /// formation, building, wonder, district or project the menu does not
    /// list — the positive gate beside the refusal cooldown — and
    /// [`Game::item_cost_for_city`] prices from it.
    #[serde(default)]
    pub host_buildable: Arc<BTreeMap<u32, BTreeMap<String, HostMenuEntry>>>,
    /// The HOST's saved work per city on PAUSED units, buildings and projects
    /// (`StateCity::buildable[].pr`, from `BuildQueue:GetUnitProgress`,
    /// `GetBuildingProgress` and `GetProjectProgress`), keyed like
    /// [`City::production_progress`] (`unit:builder`, `building:library`,
    /// `project:...`), positive readings only. Districts are bound to their
    /// foundation by the mirror instead; Wonders and formations need a plot or
    /// tier this key does not carry and are left out. Read only by the opt-in
    /// gene `saved-work-is-seen`. Empty in an ordinary game.
    #[serde(default)]
    pub host_saved_work: Arc<BTreeMap<u32, BTreeMap<String, f64>>>,
    /// Observed native per-technology costs and saved research progress, by
    /// player. Like production menus, these are refreshed host facts, not rules.
    #[serde(default)]
    pub host_research_quotes: Arc<BTreeMap<usize, BTreeMap<Name, HostResearchQuote>>>,
    /// Generated rules before the native mirror applied revealed prerequisite
    /// observations. Restored on every refresh, including older/missing exports.
    /// Saves keep that valid generated layout plus the observed differences.
    #[serde(skip)]
    pub(crate) host_research_base_rules: Option<Arc<Rules>>,
    /// Nodes whose complete prerequisite list was revealed in this host frame.
    /// Transient observation metadata; refreshed after a native save is loaded.
    #[serde(skip)]
    pub(crate) host_revealed_research: Arc<BTreeSet<Name>>,
    /// Instantaneous native CanResearch readings, by mirrored player and node.
    /// Hypothetical prerequisite searches continue to use the rules graph.
    #[serde(skip)]
    pub(crate) host_research_eligibility: Arc<BTreeMap<usize, BTreeMap<Name, bool>>>,
    /// Native strategic-resource prices for units and formations in each city,
    /// from `BuildQueue:GetUnitResourceCost(index, formation)`. These already
    /// include the host's modifiers; absent prices use the ordinary rules.
    #[serde(default)]
    pub host_unit_resource_prices: Arc<BTreeMap<u32, BTreeMap<String, f64>>>,
    /// The HOST's purchase menu per city, same keys: what
    /// `CityManager.CanStartCommand(city, PURCHASE, ...)` says this city can
    /// BUY now and what `CityGold:GetPurchaseCost` charges
    /// (`StateCity::purchasable`). When a city has an entry the three purchase
    /// pricers answer from it and nothing else; an item off the menu is not
    /// for sale.
    #[serde(default)]
    pub host_purchasable: Arc<BTreeMap<u32, BTreeMap<String, HostPurchaseEntry>>>,
    /// Native plot-purchase prices and prospective Acropolis sites. A present
    /// empty city menu supplies no purchase; absent observations retain the
    /// ordinary model price. Refreshed independently of buildable districts.
    #[serde(default)]
    pub host_plot_purchases: Arc<BTreeMap<u32, Vec<HostPlotPurchaseEntry>>>,
    /// The plots the host's `GetOperationTargets(BUILD)` offers a district in
    /// a city, kept only when the export carried the complete offer. A site
    /// outside it is not producible and `district_sites` drops it.
    #[serde(default)]
    pub host_district_plots: Arc<BTreeMap<u32, BTreeMap<Name, BTreeSet<Pos>>>>,
    /// Actual native specialty-district limit, including Great Person bonuses.
    /// Missing entries retain the ordinary population-derived limit.
    #[serde(default)]
    pub host_district_capacity: Arc<BTreeMap<u32, usize>>,
    /// Tiles a HOST says the mirrored seat can see RIGHT NOW, over and above what
    /// this engine's own sight model derives. Empty in an ordinary CIVVIS game.
    ///
    /// ★★★★★ THE EXPORT IS THE HOST'S VISIBILITY ANSWER, AND WE WERE THROWING IT
    /// AWAY. Civilization VI exports a rival's or a minor's units *only under
    /// current visibility* — `StateMinor` says so in its own doc — and the bridge
    /// plants exactly those units on the board. Then
    /// [`Game::player_vision_now`] RE-DERIVES what the seat can see, from this
    /// engine's sight radii and height field on a reconstructed map, and the
    /// tactical layer believes the derivation instead of the export. Every place
    /// the two disagree, an enemy Civilization VI is showing us is invisible to
    /// the agent deciding whether to shoot it.
    ///
    /// ⚠⚠⚠ Measured on live run `civvis-20260803T191900Z`, the 49 turns of
    /// Kongo's war (t203-250) that cost Arpinum and Arretium and ended the game
    /// at **479 against the winner's 1214**: an enemy was in the export on **49
    /// of 49 turns**, our units stood **adjacent to one on 95 unit-turns** and
    /// within range 2 — the range of the Field Cannons and Trebuchets that were
    /// most of the army — on **197**. Attacks issued: **37**. The army declined
    /// **81%** of the shots the host was showing it.
    ///
    /// `ForcePosture` only reaches `Engage` through
    /// `g.sees(&visible, enemy.pos) && battlefront_unit_visible(..)`, so with the
    /// derivation disagreeing it fell through to `Muster` — "still gathering" —
    /// turn after turn while the cities burned. The same disagreement is what
    /// `blind_ranged_tiles_charged` counts: 3 to 13 tiles on 45 separate turns.
    ///
    /// ⚠ Deliberately a stored set rather than "foreign units imply visibility".
    /// That inference is only sound for a MIRRORED board, where every foreign unit
    /// arrived from an export gated on sight. In ordinary self-play the board holds
    /// the full simulation, and reading it the same way would hand every AI player
    /// perfect vision of the entire world.
    #[serde(default)]
    pub host_observed: Arc<BTreeSet<Pos>>,
    /// Appeal as the host counts it, per plot (`Plot:GetAppeal` off the tile
    /// export). [`Game::tile_appeal`] prefers it to the derivation from the
    /// six neighbours the board can see; empty on a native game.
    #[serde(default)]
    pub observed_appeal: Arc<BTreeMap<Pos, i32>>,
    /// The host's own `Plot:IsFreshWater()` per revealed plot, which
    /// `city_water` prefers to the river/lake/oasis derivation for a city
    /// centre. Exported as `fw` since the tiles export began and read by
    /// nothing until 2026-08-27.
    #[serde(default)]
    pub observed_fresh_water: Arc<BTreeMap<Pos, bool>>,
    /// A seat's tourism per turn as the host reports it — ours from the
    /// state's `tourism_per_turn`, a rival's from `rivals[].tourism`.
    /// `tourism_per_turn` prefers it; `tourism_per_turn_model` is the
    /// engine's own figure for the instrument that measures the gap.
    #[serde(default)]
    pub observed_tourism_per_turn: Arc<BTreeMap<usize, f64>>,
    /// What the host says a route a Trader could START would pay its origin,
    /// keyed by (origin, destination) — the shipped TradeRouteChooser's own
    /// `CalculateOriginYield…` sum, exported while a route slot is open.
    /// Distinct from [`Game::observed_route_yields`], which is for ACTIVE
    /// routes and stands in for the model inside `city_yields`; this one is
    /// read by the trader-destination chooser. Empty on a native game.
    #[serde(default)]
    pub observed_route_options: Arc<BTreeMap<(u32, u32), crate::rules::Yields>>,
    /// The host's climate readings beyond the phase itself: the world's CO2,
    /// the temperature, the sea-level and disaster forecasts
    /// (`GameClimate`, the shipped ClimateScreen's reads). `None` on a native
    /// game or an export without it.
    #[serde(default)]
    pub observed_climate: Option<ObservedClimate>,
    /// Ground a HOST holds for somebody else and will not let this seat's units
    /// enter, for tiles whose owner [`Game::territory_owner_at`] cannot name.
    ///
    /// ★★★★★ THE MISSING MEMBER OF THE `blocked_*` FAMILY, AND THE COMMENT ON
    /// [`Game::blocked_policies`] already says which one it is: policies were
    /// "third behind **movement** and `no_params`". Movement is the largest
    /// refusal category in the ledger and it was the only one with no block set.
    ///
    /// The ordinary gate in `can_enter` reads `territory_owner_at`, which resolves
    /// a tile's owner through `owner_city -> cities -> owner`. A rival whose cities
    /// this seat has never SEEN has no city on the mirrored board, so their border
    /// resolves to `None` and reads as free ground. That is a closed loop: their
    /// border is invisible precisely because we cannot get past it to see the city
    /// that makes it.
    ///
    /// ⚠⚠⚠ Measured on live run `civvis-20260803T191900Z` (Rome, SETTLER, small).
    /// Scout `196608` reached offset (12,24) on **turn 42** and was ordered
    /// `MOVE_TO (11,24)` — one hex — on **74 separate turns** without ever moving;
    /// at t91 it was upgraded in place and the Skirmisher kept re-sending the same
    /// step. (11,24) is exported `o: 4`, Kongo's territory, and we were not at war
    /// and held no open borders. **81 of 670 `MOVE_TO` orders targeted foreign
    /// ground, every one of them Kongo's, and all 81 were counted `applied` —
    /// a blocked move is a silent no-op, not a refusal.**
    ///
    /// What it cost, in order: exploration flatlined at **283 of 3404 tiles (8.3%)**
    /// from turn 42; **zero rival cities were observed in 96 snapshots**; with no
    /// known enemy city `plan.target_city` stayed `None`, so `campaign` fell through
    /// to `peacetime_step` and all seven combat units garrisoned within three hexes
    /// of home; and `strategy=conquest` ran for forty turns with `war_legal=9` and
    /// **no war ever declared**. Of 35 walkable frontier tiles, 21 were sealed
    /// behind that border — including all six nearest the scout — while 14 neutral
    /// ones waited at distance 7, leading east into two thirds of an unseen map.
    ///
    /// Written fresh from the export every turn rather than accumulated, and never
    /// written for an owner this seat has access to, so declaring war opens the
    /// ground on the following turn instead of sealing our own invasion out.
    #[serde(default)]
    pub closed_borders: BTreeSet<Pos>,
    /// Ground a MET major civilization owns whose city attribution is unresolved
    /// to this seat. That means either no city of theirs is visible, or the plot
    /// sits on the fifth ownership ring of the nearest visible city, where the
    /// host cannot distinguish that city from a nearer one still in fog. Written
    /// fresh from the export every turn beside `closed_borders`, and empty on any
    /// board that is not a live mirror. A settle site beside such a border can sit
    /// under Loyalty pressure the forecast cannot count: run
    /// civvis-20260826T030045Z founded Lugdunum five tiles from a German border
    /// attributed to a city ten tiles away, read −22 Loyalty on its first export,
    /// and lost the undamaged city at t70. See `AdvancedAi::frontier_loyalty`.
    #[serde(default)]
    pub unseen_major_borders: BTreeSet<Pos>,
    /// How much sealed ground each MAJOR seat accounts for — the mirrored seat
    /// id against its tile count in [`Game::closed_borders`]. The set alone
    /// cannot answer the one question the passage-purchase lane asks: WHO to
    /// buy Open Borders from. Written fresh beside `closed_borders` on a live
    /// mirror and empty everywhere else; minors seal ground too but are never
    /// recorded here, because their passage is suzerainty, not a deal.
    #[serde(default)]
    pub sealed_border_owners: BTreeMap<usize, u32>,
    /// `(band owner, host)` pairs: the host keeps that seat's Rock Bands out
    /// of its land. A live mirror's reading of a rival's Music Censorship,
    /// which the export never shows: the host's own concert highlights name
    /// no stage in a rival's land although a finished World Wonder (a stage
    /// every band may play) stands there. Live 20261110T164254Z: Ethiopia
    /// slotted it the turn after our first concerts in its land, every band
    /// was then refused at its border for the rest of the game, and five
    /// bands played twice. Written fresh every turn by
    /// `mirror::band_closed_hosts`, empty on any other board; read through
    /// [`Game::bars_foreign_rock_bands`].
    #[serde(default)]
    pub rock_band_barred_hosts: BTreeSet<(usize, usize)>,
    /// The turn each peace treaty runs until, keyed by signatory pair. War
    /// cannot be declared again before it expires — the shipped
    /// `DIPLOMACY_PEACE_MIN_TURNS`.
    pub peace_treaties: BTreeMap<(usize, usize), u32>,
    /// Running chronicle of every war in progress, keyed by belligerent pair.
    pub wars: BTreeMap<(usize, usize), WarRecord>,
    /// What has actually landed on cities this game, for evaluators.
    #[serde(default)]
    pub siege: SiegeCensus,
    /// The wars that ended, oldest first and bounded — long enough for a
    /// client to show what a peace cost, short enough not to grow forever.
    pub concluded_wars: Vec<WarRecord>,
    /// Every nuclear detonation, oldest first and bounded. The tail is what a
    /// client animates and what a log describes; the whole history is not
    /// worth carrying in every frame of a five-hundred-turn game.
    #[serde(default)]
    pub nuclear_strikes: Vec<NuclearStrike>,
    /// Where each unit has walked lately, oldest first and bounded like the
    /// strike ledger: a client draws the recent tail, and the full walking
    /// history of a long game is not worth carrying in every frame.
    #[serde(default)]
    pub unit_move_trails: Vec<UnitMoveTrail>,
    pub barb_pid: Option<usize>,
    pub barb_camps: BTreeMap<Pos, u32>,
    /// Coastal camps are naval outposts; the set is persisted so a camp does
    /// not change type merely because a save was loaded on another turn.
    #[serde(default)]
    pub barb_naval_camps: BTreeSet<Pos>,
    /// The fortified anti-cavalry unit initially assigned to each camp.
    /// Keeping its id lets camp cleanup remove stale ownership cleanly.
    #[serde(default)]
    pub barb_camp_guards: BTreeMap<Pos, u32>,
    pub barb_scout_homes: BTreeMap<u32, Pos>,
    /// Raiders remember the outpost that raised them. A nearby second camp
    /// must not silently redirect a party that was sent somewhere else.
    #[serde(default)]
    pub barb_raider_homes: BTreeMap<u32, Pos>,
    pub barb_scout_targets: BTreeMap<u32, Pos>,
    pub barb_camp_targets: BTreeMap<Pos, Pos>,
    pub barb_alerted_until: BTreeMap<Pos, u32>,
    pub routes: Vec<TradeRoute>,
    pub world_era: usize,
    /// Turn `world_era` last changed. Shipped `Eras_XP1.GameEraMinimumTurns`
    /// holds an era open for 40 standard turns before the next one may start.
    pub world_era_since: u32,
    /// Turn on which the active next-era countdown expires.
    #[serde(default)]
    pub world_era_countdown_end: Option<u32>,
    /// Irreversible Gathering Storm climate phase, from 0 (pre-warming) to 7.
    pub climate_phase: u8,
    /// Meteor showers landed so far - the Apocalypse pack budgets a fixed
    /// number per game.
    #[serde(default)]
    pub meteor_strikes: u32,
    /// The lobby's disaster-intensity setting, 0 (none) to 4 (hyperreal).
    #[serde(default = "default_disaster_intensity")]
    pub disaster_intensity: u8,
    /// The New Frontier game modes this lobby switched on. Empty is the stock
    /// Gathering Storm ruleset, which is what a tournament lobby plays.
    #[serde(default)]
    pub game_modes: BTreeSet<String>,
    /// Storm systems currently crossing the map.
    #[serde(default)]
    pub storms: Vec<Storm>,
    /// Droughts in progress, with the tiles they hold and when they lift.
    #[serde(default)]
    pub droughts: Vec<Drought>,
    /// Retired named Great People leave the global market permanently.
    pub retired_great_people: BTreeSet<String>,
    /// Recruitment price fixed when each named person enters the market.
    /// Ahead-of-world-era surcharges therefore do not fall while an offer is
    /// waiting, and save/load preserves the quoted price.
    pub great_person_offer_costs: BTreeMap<String, f64>,
    pub pending_deals: Vec<DiplomaticDeal>,
    pub active_trade_deals: Vec<ActiveTradeDeal>,
    pub next_deal_id: u32,
    pub congress: Option<CongressSession>,
    pub active_congress_effects: Vec<CongressEffect>,
    /// Active scored competitions supplied by a live Civilization VI host.
    /// Native games leave this empty, so host-only projects cannot leak into
    /// offline production menus.
    #[serde(default)]
    pub host_competitions: Arc<Vec<HostCompetition>>,
    /// The scored competition CIVVIS is running itself, if any. Off unless
    /// `native_competitions` is set. See [`Competition`].
    #[serde(default)]
    pub competition: Option<Competition>,
    /// First turn each competition kind may be seated again.
    ///
    /// ⚠ Per kind, because the shipped `LockoutTime` is a column on the
    /// emergency row rather than a global clock. A single lockout let one
    /// competition block every other for sixty turns, and a 250-turn game
    /// seated two.
    #[serde(default)]
    pub competition_lockout_until: BTreeMap<String, u32>,
    /// Whether this game runs its own scored competitions.
    ///
    /// ⚠ Off by default and deliberately so. Turning it on changes what every
    /// participant faces, which moves the frozen rating anchor — so it ships
    /// as an arm to be priced first, not as a silent rules change. See
    /// `docs/ELO_REPINS.md` for what promoting it costs.
    #[serde(default)]
    pub native_competitions: bool,
    /// Turn the last Special Session was seated. Shipped
    /// `WORLD_CONGRESS_MIN_TIME_BETWEEN_SPECIAL_SESSIONS` holds the next one
    /// off for 15 standard-scaled turns. Saves from before this field start
    /// at zero, which only ever lets one extra early Session through.
    #[serde(default)]
    pub last_special_session: u32,
    pub pending_emergencies: Vec<EmergencyProposal>,
    pub active_emergencies: Vec<Emergency>,
    occ: BTreeMap<Pos, Vec<u32>>,
    city_by_pos: BTreeMap<Pos, u32>,
    /// Every successfully applied action, in order — the game is exactly
    /// f(seed+params, log), so this is the replay/desync-detection record.
    /// Runtime-only (not in saves yet).
    pub log: crate::actionlog::ActionLog,
    /// Per-civilization event stream; see [`Event`].
    pub events: EventLog,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TradeRoute {
    pub origin: u32,
    pub dest: u32,
    pub owner: usize,
    pub ends: u32,
}

/// One of Gathering Storm's four terrain-bound storm systems. Unlike every
/// other disaster a storm is not over when it lands: it drifts across the map
/// for three turns, so it is state rather than an event.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Storm {
    /// The disaster class, and so the key into `rules.disasters`.
    pub kind: String,
    pub pos: Pos,
    /// Index into [`hex::DIRS`]; a storm holds its course.
    pub heading: usize,
    pub severity: u8,
    /// The turn the system dissipates.
    pub ends: u32,
}

/// One target's current native declaration observations. Unknown types are not
/// permissions; a snapshot without this record retains legacy model semantics.
#[derive(Clone, Debug, Default)]
pub struct ObservedWarTypes {
    pub turn: u32,
    pub permissions: BTreeMap<String, bool>,
}

/// The host's climate readings a mirrored board carries beside its phase —
/// `GameClimate` as the shipped ClimateScreen reads it. Each field is `None`
/// where the accessor was missing on the host's ruleset.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ObservedClimate {
    /// Degrees above the starting baseline (`GetTemperatureChange`).
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Every civilization's CO2 together (`GetTotalCO2Footprint`), which
    /// [`Game::global_co2_emissions`] prefers to the board's own sum.
    #[serde(default)]
    pub co2_total: Option<f64>,
    /// Turns until the next sea-level rise (`GetNextSeaLevelRiseTurns`).
    #[serde(default)]
    pub sea_level_turns: Option<i64>,
    /// Tiles the sea has flooded so far (`GetTilesFlooded`).
    #[serde(default)]
    pub tiles_flooded: Option<i64>,
    /// Per-turn percent chance of a storm, a river flood and a drought.
    #[serde(default)]
    pub storm_pct: Option<f64>,
    #[serde(default)]
    pub flood_pct: Option<f64>,
    #[serde(default)]
    pub drought_pct: Option<f64>,
}

/// A drought in progress. Droughts are the longest-lived disaster, so the
/// tiles they cover are remembered until the rain returns.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Drought {
    pub tiles: Vec<Pos>,
    pub severity: u8,
    pub ends: u32,
}

/// The five Gathering Storm disaster-intensity settings, as a multiplier on
/// every class's per-game rate. Zero is the lobby's "no disasters"; the rest
/// climb the way the shipped intensity ladder does.
const DISASTER_INTENSITY_SCALE: [f64; 5] = [0.0, 0.5, 1.0, 1.75, 2.75];

/// Which half of a tile's permanent disaster fertility a roll landed on.
/// `RandomEvent_Yields` rates Food and Production separately, and the tile
/// holds them separately (`Tile::disaster_food`, `Tile::disaster_production`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fertility {
    Food,
    Production,
}

/// The share of the map's volcanoes that are active at each intensity — the
/// shipped 45%-to-95% band.
const ACTIVE_VOLCANO_SHARE: [f64; 5] = [0.0, 0.45, 0.62, 0.79, 0.95];

/// How much each irreversible climate phase raises disaster frequency, and how
/// much it pushes severity toward the top tier.
const CLIMATE_DISASTER_SCALE: f64 = 0.125;
const CLIMATE_SEVERITY_BIAS: f64 = 0.15;

/// The most permanent Food repeated disasters can leave on one tile.
const DISASTER_FERTILITY_CAP: f64 = 3.0;

/// The world era Heartbeat of Steam starts paying for buildings in.
const INDUSTRIAL_ERA: usize = 4;

/// Gathering Storm's default disaster intensity: the middle of the five.
pub const DEFAULT_DISASTER_INTENSITY: u8 = 2;

fn default_disaster_intensity() -> u8 {
    DEFAULT_DISASTER_INTENSITY
}

#[derive(Clone, Serialize, Deserialize)]
struct GameSer {
    seed: u64,
    /// The concrete Future-era graph rolled at setup. Old saves did not carry
    /// it and deterministically regenerate from their seed; new saves keep it
    /// so an engine update cannot reroll a match in progress.
    #[serde(default)]
    future_tree_layout: Option<FutureTreeLayout>,
    /// Native revealed prerequisites are observations, not a new generated
    /// two-column layout. Keep them separate so partial revelations round-trip.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    host_research_prerequisites: BTreeMap<Name, Vec<Name>>,
    /// Zero means the save predates persistent last-seen snapshots. Current
    /// saves must load their memories verbatim so deserialization is pure.
    #[serde(default)]
    visibility_memory_version: u8,
    #[serde(default = "default_difficulty")]
    difficulty: String,
    #[serde(default = "default_barbarian_difficulty")]
    barbarian_difficulty: String,
    #[serde(default = "default_speed")]
    speed: String,
    #[serde(default)]
    human_seats: BTreeSet<usize>,
    #[serde(default)]
    handicap_exempt: BTreeSet<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ai_yield_rung: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ai_space_project_cost_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ai_late_science_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ai_space_tech_floor: Option<u32>,
    #[serde(default)]
    events: Vec<Event>,
    #[serde(default)]
    base_ruleset: BaseRuleset,
    /// Absent in saves written before a game could open anywhere but the
    /// Ancient era, which is exactly what the default is.
    #[serde(default)]
    start_era: usize,
    /// Absent in saves written before the Future Era was a choice, all of
    /// which were played on the classic one.
    #[serde(default)]
    future_era: FutureEra,
    /// Absent in saves written before turn structure was a choice, all of
    /// which were sequential.
    #[serde(default)]
    turn_structure: TurnStructure,
    /// Absent in every classic save, and in a modified one that has not been
    /// rolled yet.
    #[serde(default)]
    moon_deposits: BTreeMap<Name, f64>,
    #[serde(default)]
    map_script: MapScript,
    #[serde(default)]
    leader_pool: LeaderPool,
    /// Absent in saves written before the poles became a choice, which were
    /// all rolled with them.
    #[serde(default)]
    map_poles: MapPoles,
    #[serde(default)]
    game_speed: GameSpeed,
    #[serde(default)]
    mods: Vec<String>,
    max_turns: u32,
    turn: u32,
    current: usize,
    winner: Option<usize>,
    victory_type: Option<String>,
    #[serde(default)]
    decided: Option<Decided>,
    #[serde(default)]
    victory_conditions: VictoryConditions,
    #[serde(default)]
    mercy_rule: Option<f64>,
    #[serde(default)]
    mercy_lanes: Vec<String>,
    #[serde(default)]
    required_victory_types: usize,
    #[serde(default)]
    tactics: TacticsRules,
    #[serde(default)]
    arena_flags: BTreeMap<usize, Pos>,
    #[serde(default)]
    victories_won: BTreeMap<usize, BTreeSet<String>>,
    next_id: u32,
    rng: Rng,
    at_war: Vec<(usize, usize)>,
    #[serde(default)]
    observed_military_power: BTreeMap<usize, f64>,
    #[serde(default)]
    host_unit_facts: BTreeMap<u32, HostUnitFacts>,
    #[serde(default)]
    host_maintenance: BTreeMap<usize, HostMaintenance>,
    #[serde(default)]
    observed_score: BTreeMap<usize, i64>,
    #[serde(default)]
    observed_rival_wars: BTreeSet<(usize, usize)>,
    #[serde(default)]
    observed_city_palaces: BTreeMap<u32, bool>,
    #[serde(default)]
    observed_city_religion_markers: BTreeSet<u32>,
    #[serde(default)]
    observed_city_faiths: BTreeMap<u32, Vec<ObservedCityFaith>>,
    #[serde(default)]
    observed_strategic_income_adjustments: BTreeMap<usize, BTreeMap<Name, f64>>,
    #[serde(default)]
    observed_trade_capacity: BTreeMap<usize, i64>,
    #[serde(default)]
    observed_leader_types: BTreeMap<usize, String>,
    #[serde(default)]
    observed_yield_adjustments: BTreeMap<usize, crate::rules::Yields>,
    #[serde(default)]
    observed_public_empire_stats: BTreeMap<usize, ObservedPublicEmpireStats>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    unseen_city_owners: BTreeSet<usize>,
    #[serde(default)]
    observed_majority_religion: BTreeMap<usize, String>,
    /// A list, not a map: JSON cannot key an object by a tuple.
    #[serde(default)]
    observed_visiting_tourists: Vec<((usize, usize), i64)>,
    #[serde(default)]
    observed_city_yield_adjustments: BTreeMap<u32, crate::rules::Yields>,
    #[serde(default)]
    observed_city_amenity_adjustments: BTreeMap<u32, i64>,
    #[serde(default)]
    observed_city_housing_adjustments: BTreeMap<u32, f64>,
    /// A list, not a map: JSON cannot key an object by a tuple.
    #[serde(default)]
    observed_tile_yield_adjustments: Vec<(Pos, crate::rules::Yields)>,
    #[serde(default)]
    observed_route_posts: Vec<((u32, u32), (i64, i64))>,
    #[serde(default)]
    observed_great_work_housing: Option<BTreeMap<u32, BTreeMap<String, usize>>>,
    #[serde(default)]
    observed_route_yields: Vec<((u32, u32), crate::rules::Yields)>,
    #[serde(default)]
    observed_incoming_route_deltas: BTreeMap<u32, (i64, i64)>,
    #[serde(default)]
    observed_route_options: Vec<((u32, u32), crate::rules::Yields)>,
    #[serde(default)]
    observed_appeal: Vec<(Pos, i32)>,
    #[serde(default)]
    observed_fresh_water: Vec<(Pos, bool)>,
    #[serde(default)]
    observed_tourism_per_turn: Vec<(usize, f64)>,
    #[serde(default)]
    observed_climate: Option<ObservedClimate>,
    #[serde(default)]
    observed_city_worked_tiles: BTreeMap<u32, Vec<Pos>>,
    #[serde(default)]
    observed_city_specialists: BTreeMap<u32, Vec<String>>,
    #[serde(default)]
    observed_city_loyalty_per_turn: BTreeMap<u32, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observed_settlement_loyalty: Option<NativeSettlementPressure>,
    #[serde(default)]
    observed_city_strength: BTreeMap<u32, f64>,
    #[serde(default)]
    observed_city_ranged_strength: BTreeMap<u32, f64>,
    #[serde(default)]
    observed_city_max_wall_hp: BTreeMap<u32, i32>,
    #[serde(default)]
    peace_treaties: Vec<((usize, usize), u32)>,
    #[serde(default)]
    wars: Vec<((usize, usize), WarRecord)>,
    #[serde(default)]
    concluded_wars: Vec<WarRecord>,
    #[serde(default)]
    nuclear_strikes: Vec<NuclearStrike>,
    #[serde(default)]
    unit_move_trails: Vec<UnitMoveTrail>,
    #[serde(default)]
    barb_pid: Option<usize>,
    #[serde(default)]
    barb_camps: Vec<(Pos, u32)>,
    #[serde(default)]
    barb_naval_camps: Vec<Pos>,
    #[serde(default)]
    barb_camp_guards: Vec<(Pos, u32)>,
    #[serde(default)]
    barb_scout_homes: BTreeMap<u32, Pos>,
    #[serde(default)]
    barb_raider_homes: BTreeMap<u32, Pos>,
    #[serde(default)]
    barb_scout_targets: BTreeMap<u32, Pos>,
    #[serde(default)]
    barb_camp_targets: Vec<(Pos, Pos)>,
    #[serde(default)]
    barb_alerted_until: Vec<(Pos, u32)>,
    #[serde(default)]
    routes: Vec<TradeRoute>,
    #[serde(default)]
    world_era: usize,
    #[serde(default)]
    world_era_since: u32,
    #[serde(default)]
    world_era_countdown_end: Option<u32>,
    #[serde(default)]
    climate_phase: u8,
    #[serde(default)]
    meteor_strikes: u32,
    #[serde(default = "default_disaster_intensity")]
    disaster_intensity: u8,
    #[serde(default)]
    game_modes: BTreeSet<String>,
    #[serde(default)]
    storms: Vec<Storm>,
    #[serde(default)]
    droughts: Vec<Drought>,
    #[serde(default)]
    retired_great_people: BTreeSet<String>,
    #[serde(default)]
    great_person_offer_costs: BTreeMap<String, f64>,
    #[serde(default)]
    pending_deals: Vec<DiplomaticDeal>,
    #[serde(default)]
    active_trade_deals: Vec<ActiveTradeDeal>,
    #[serde(default = "one_u32")]
    next_deal_id: u32,
    #[serde(default)]
    congress: Option<CongressSession>,
    #[serde(default)]
    active_congress_effects: Vec<CongressEffect>,
    #[serde(default)]
    host_competitions: Vec<HostCompetition>,
    #[serde(default)]
    competition: Option<Competition>,
    #[serde(default)]
    competition_lockout_until: BTreeMap<String, u32>,
    #[serde(default)]
    native_competitions: bool,
    #[serde(default)]
    last_special_session: u32,
    #[serde(default)]
    pending_emergencies: Vec<EmergencyProposal>,
    #[serde(default)]
    active_emergencies: Vec<Emergency>,
    map: WorldMap,
    players: Vec<Player>,
    units: Vec<Unit>,
    #[serde(default)]
    spies: Vec<Spy>,
    cities: Vec<City>,
}

impl From<GameSer> for Game {
    fn from(s: GameSer) -> Game {
        let needs_visibility_backfill = s.visibility_memory_version == 0;
        let has_unknown_terrain = s.map.tiles.values().any(|tile| tile.terrain == "unknown");
        let seated_identities: Vec<String> = s
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.civ.clone())
            .collect();
        let mut rules = Rules::for_game(s.seed, s.future_tree_layout.as_ref(), s.future_era);
        if has_unknown_terrain {
            // Mirror-only terrain is injected after normal ruleset loading so it
            // survives a save round trip without changing the audited fingerprint.
            Arc::make_mut(&mut rules).enable_unknown_terrain();
        }
        if seated_identities
            .iter()
            .any(|civ| !leader_roster::uses_civ6_mechanics(civ))
        {
            leader_roster::install_neutral_identities(
                Arc::make_mut(&mut rules),
                &seated_identities,
            );
        }
        // Both speed representations exist for save compatibility. Prefer an
        // explicit ruleset speed, except when loading an older map-script save
        // whose absent string field defaulted to Standard.
        let (speed, game_speed) = if s.speed == "standard" && s.game_speed != GameSpeed::Standard {
            (s.game_speed.id().to_string(), s.game_speed)
        } else {
            let game_speed = GameSpeed::from_id(&s.speed).unwrap_or(s.game_speed);
            (game_speed.id().to_string(), game_speed)
        };
        let mut g = Game {
            rules,
            vision: std::cell::RefCell::new(VisionCache::default()),
            vision_frames: VisionFrameCache::default(),
            routing: std::cell::RefCell::new(RoutingCache::default()),
            route_scratch: std::cell::RefCell::new(RouteScratch::default()),
            query_memo: QueryCache::default(),
            view_rules: Default::default(),
            attack_reach_cache: Arc::new(std::sync::Mutex::new(AttackReachCache::default())),
            remembered_under: SharedVec::default(),
            track_fog_memory: true,
            track_war_ledger: true,
            planning_role: PlanningRole::Off,
            visibility_batch: VisibilityBatch::default(),
            visibility_suppressed: false,
            rng: s.rng,
            seed: s.seed,
            difficulty: s.difficulty,
            barbarian_difficulty: s.barbarian_difficulty,
            speed,
            human_seats: s.human_seats,
            handicap_exempt: s.handicap_exempt,
            ai_yield_rung: s.ai_yield_rung,
            ai_space_project_cost_pct: s.ai_space_project_cost_pct,
            ai_late_science_pct: s.ai_late_science_pct,
            ai_space_tech_floor: s.ai_space_tech_floor,
            mods: s.mods,
            events: s.events.into(),
            base_ruleset: s.base_ruleset,
            start_era: s.start_era,
            future_era: s.future_era,
            turn_structure: s.turn_structure,
            moon_deposits: s.moon_deposits,
            map_script: s.map_script,
            leader_pool: s.leader_pool.available_or_default(),
            map_poles: s.map_poles,
            game_speed,
            max_turns: s.max_turns,
            turn: s.turn,
            current: s.current,
            winner: s.winner,
            victory_type: s.victory_type,
            decided: s.decided,
            victory_conditions: s.victory_conditions,
            mercy_rule: s.mercy_rule,
            mercy_lanes: s.mercy_lanes,
            required_victory_types: s.required_victory_types,
            tactics: s.tactics,
            arena_flags: s.arena_flags,
            victories_won: s.victories_won,
            next_id: s.next_id,
            map: s.map,
            players: s.players.into(),
            units: s.units.into_iter().map(|u| (u.id, u)).collect(),
            spies: s.spies.into_iter().map(|spy| (spy.id, spy)).collect(),
            cities: s.cities.into_iter().map(|c| (c.id, c)).collect(),
            at_war: s.at_war.into_iter().collect(),
            observed_military_power: Arc::new(s.observed_military_power),
            host_unit_facts: Arc::new(s.host_unit_facts),
            host_maintenance: Arc::new(s.host_maintenance),
            observed_score: Arc::new(s.observed_score),
            observed_rival_wars: Arc::new(s.observed_rival_wars),
            observed_city_palaces: Arc::new(s.observed_city_palaces),
            observed_city_religion_markers: Arc::new(s.observed_city_religion_markers),
            observed_city_faiths: Arc::new(s.observed_city_faiths),
            observed_strategic_income_adjustments: Arc::new(
                s.observed_strategic_income_adjustments,
            ),
            observed_trade_capacity: Arc::new(s.observed_trade_capacity),
            observed_leader_types: Arc::new(s.observed_leader_types),
            observed_yield_adjustments: Arc::new(s.observed_yield_adjustments),
            observed_public_empire_stats: Arc::new(s.observed_public_empire_stats),
            unseen_city_owners: s.unseen_city_owners,
            observed_majority_religion: Arc::new(s.observed_majority_religion),
            observed_visiting_tourists: Arc::new(
                s.observed_visiting_tourists.into_iter().collect(),
            ),
            observed_city_yield_adjustments: Arc::new(s.observed_city_yield_adjustments),
            observed_city_amenity_adjustments: Arc::new(s.observed_city_amenity_adjustments),
            observed_city_housing_adjustments: Arc::new(s.observed_city_housing_adjustments),
            observed_tile_yield_adjustments: Arc::new(
                s.observed_tile_yield_adjustments.into_iter().collect(),
            ),
            observed_route_posts: Arc::new(s.observed_route_posts.into_iter().collect()),
            observed_great_work_housing: s.observed_great_work_housing.map(Arc::new),
            observed_route_yields: Arc::new(s.observed_route_yields.into_iter().collect()),
            observed_incoming_route_deltas: Arc::new(s.observed_incoming_route_deltas),
            observed_route_options: Arc::new(s.observed_route_options.into_iter().collect()),
            observed_appeal: Arc::new(s.observed_appeal.into_iter().collect()),
            observed_fresh_water: Arc::new(s.observed_fresh_water.into_iter().collect()),
            observed_tourism_per_turn: Arc::new(s.observed_tourism_per_turn.into_iter().collect()),
            observed_climate: s.observed_climate,
            observed_city_worked_tiles: Arc::new(s.observed_city_worked_tiles),
            observed_city_specialists: Arc::new(s.observed_city_specialists),
            observed_city_loyalty_per_turn: Arc::new(s.observed_city_loyalty_per_turn),
            observed_settlement_loyalty: s.observed_settlement_loyalty.map(Arc::new),
            observed_city_strength: Arc::new(s.observed_city_strength),
            observed_city_ranged_strength: Arc::new(s.observed_city_ranged_strength),
            observed_city_max_wall_hp: Arc::new(s.observed_city_max_wall_hp),
            observed_wall_tier_rules: false,
            lump_quote_viewer: None,
            nusantara_site_planner: None,
            observed_builders_avoid_mountains: false,
            heist_reads_the_host_menu: false,
            spy_targets_read_the_host: false,
            // Not carried in a save: host refusals are rebuilt from the run's event
            // log on every reconstruction, so a stale copy would only mislead.
            blocked_city_sites: Arc::new(BTreeSet::new()),
            host_observed: Arc::new(BTreeSet::new()),
            closed_borders: BTreeSet::new(),
            unseen_major_borders: BTreeSet::new(),
            sealed_border_owners: BTreeMap::new(),
            rock_band_barred_hosts: BTreeSet::new(),
            blocked_improvement_sites: Arc::new(BTreeSet::new()),
            blocked_improvements: Arc::new(BTreeMap::new()),
            great_person_plots: BTreeMap::new(),
            blocked_promotions: Arc::new(BTreeMap::new()),
            host_band_promotions: Arc::new(BTreeMap::new()),
            blocked_strikes: Arc::new(BTreeSet::new()),
            host_war_blocks: Arc::new(BTreeSet::new()),
            host_war_type_permissions: Arc::new(BTreeMap::new()),
            host_envoy_permissions: Arc::new(BTreeMap::new()),
            host_previews: Arc::new(BTreeMap::new()),
            blocked_trade_routes: Arc::new(BTreeSet::new()),
            blocked_policies: Arc::new(BTreeSet::new()),
            host_policy_choices: BTreeMap::new(),
            host_policy_windows: BTreeMap::new(),
            blocked_pantheons: Arc::new(BTreeSet::new()),
            blocked_districts: Arc::new(BTreeMap::new()),
            host_district_sites: Arc::new(BTreeMap::new()),
            host_wonder_sites: Arc::new(BTreeMap::new()),
            blocked_wonders: Arc::new(BTreeMap::new()),
            host_unavailable_wonders: Arc::new(BTreeSet::new()),
            blocked_production: Arc::new(BTreeMap::new()),
            blocked_purchases: Arc::new(BTreeMap::new()),
            host_buildable: Arc::new(BTreeMap::new()),
            host_saved_work: Arc::new(BTreeMap::new()),
            host_research_quotes: Arc::new(BTreeMap::new()),
            host_research_base_rules: None,
            host_revealed_research: Arc::new(BTreeSet::new()),
            host_research_eligibility: Arc::new(BTreeMap::new()),
            host_unit_resource_prices: Arc::new(BTreeMap::new()),
            host_purchasable: Arc::new(BTreeMap::new()),
            host_plot_purchases: Arc::new(BTreeMap::new()),
            host_district_plots: Arc::new(BTreeMap::new()),
            host_district_capacity: Arc::new(BTreeMap::new()),
            peace_treaties: s.peace_treaties.into_iter().collect(),
            wars: s.wars.into_iter().collect(),
            siege: SiegeCensus::default(),
            concluded_wars: s.concluded_wars,
            nuclear_strikes: s.nuclear_strikes,
            unit_move_trails: s.unit_move_trails,
            barb_pid: s.barb_pid,
            barb_camps: s.barb_camps.into_iter().collect(),
            barb_naval_camps: s.barb_naval_camps.into_iter().collect(),
            barb_camp_guards: s.barb_camp_guards.into_iter().collect(),
            barb_scout_homes: s.barb_scout_homes,
            barb_raider_homes: s.barb_raider_homes,
            barb_scout_targets: s.barb_scout_targets,
            barb_camp_targets: s.barb_camp_targets.into_iter().collect(),
            barb_alerted_until: s.barb_alerted_until.into_iter().collect(),
            routes: s.routes,
            world_era: s.world_era,
            world_era_since: s.world_era_since,
            world_era_countdown_end: s.world_era_countdown_end,
            climate_phase: s.climate_phase,
            meteor_strikes: s.meteor_strikes,
            disaster_intensity: s.disaster_intensity,
            game_modes: s.game_modes,
            storms: s.storms,
            droughts: s.droughts,
            retired_great_people: s.retired_great_people,
            great_person_offer_costs: s.great_person_offer_costs,
            pending_deals: s.pending_deals,
            active_trade_deals: s.active_trade_deals,
            next_deal_id: s.next_deal_id,
            congress: s.congress,
            active_congress_effects: s.active_congress_effects,
            host_competitions: Arc::new(s.host_competitions),
            competition: s.competition,
            competition_lockout_until: s.competition_lockout_until,
            native_competitions: s.native_competitions,
            last_special_session: s.last_special_session,
            pending_emergencies: s.pending_emergencies,
            active_emergencies: s.active_emergencies,
            occ: BTreeMap::new(),
            city_by_pos: BTreeMap::new(),
            log: crate::actionlog::ActionLog::new(),
        };
        g.set_host_research_prerequisites(s.host_research_prerequisites)
            .unwrap_or_else(|error| panic!("invalid saved native research prerequisites: {error}"));
        // Saves from before the two-stage Loyalty model have no Free Cities
        // seat. Appending it preserves every existing player id.
        g.ensure_free_city_player();
        for u in g.units.values() {
            g.occ.entry(u.pos).or_default().push(u.id);
        }
        for c in g.cities.values() {
            g.city_by_pos.insert(c.pos, c.id);
        }
        let legacy: Vec<u32> = g
            .cities
            .values()
            .filter(|c| c.wall_hp < 0)
            .map(|c| c.id)
            .collect();
        for cid in legacy {
            let max = g.city_max_wall_hp(&g.cities[&cid]);
            g.cities.get_mut(&cid).unwrap().wall_hp = max;
        }
        let legacy_encampments: Vec<u32> = g
            .cities
            .values()
            .filter(|c| g.city_has_district_family(c, crate::name!("encampment")))
            .map(|c| c.id)
            .collect();
        for cid in legacy_encampments {
            let max = g.city_max_wall_hp(&g.cities[&cid]);
            let city = g.cities.get_mut(&cid).unwrap();
            if city.encampment_hp <= 0 && !city.encampment_pillaged {
                city.encampment_hp = 100;
            }
            if city.encampment_wall_hp < 0 {
                city.encampment_wall_hp = max;
            }
        }
        for cid in g.cities.keys().copied().collect::<Vec<_>>() {
            g.restore_other_defending_districts(cid);
        }
        let unit_ids: Vec<u32> = g.units.keys().copied().collect();
        for uid in unit_ids {
            let max_attacks = g.unit_max_attacks(uid);
            let unit = g.units.get_mut(&uid).unwrap();
            // Missing legacy fields already deserialize through `one_attack`.
            // Preserve a real mid-turn zero so save/load cannot restore an
            // attack that the unit has already spent.
            unit.attacks_left = unit.attacks_left.clamp(0, max_attacks);
            if unit.religion.is_none() && g.rules.units[unit.kind].class == "religious" {
                unit.religion = g.players[unit.owner].religion.clone();
            }
        }
        let governor_ids: Vec<Name> = g.rules.governors.keys().cloned().collect();
        for player in g.players.iter_mut() {
            // Favor lived in the generic counter map in pre-trade saves.
            // Migrate it once into the typed balance used by Congress and
            // diplomatic trades without changing the save format version.
            if player.diplomatic_favor == 0.0 {
                player.diplomatic_favor = player
                    .counters
                    .get("diplomatic_favor")
                    .copied()
                    .unwrap_or(0)
                    .max(0) as f64;
            }
            if player.governor_roster.is_empty() && !player.governors.is_empty() {
                for (governor, city) in governor_ids.iter().zip(player.governors.iter().copied()) {
                    player.governor_roster.insert(
                        governor.to_string(),
                        GovernorState {
                            city: Some(city),
                            assigned_turn: 0,
                            disabled_until: 0,
                            promotions: BTreeSet::new(),
                        },
                    );
                }
                player.governor_titles_spent = player.governor_roster.len();
            }
        }
        g.refresh_great_person_offers();
        if needs_visibility_backfill {
            // Legacy saves know which tiles were explored but predate
            // last-seen snapshots. Seed those once from the loaded world.
            g.backfill_visibility_memory();
            g.refresh_all_visibility();
        }
        g.repair_legacy_war_log();
        g
    }
}

impl From<Game> for GameSer {
    fn from(g: Game) -> GameSer {
        let host_research_prerequisites = g.host_research_prerequisites();
        let future_tree_layout = g
            .host_research_base_rules
            .as_ref()
            .unwrap_or(&g.rules)
            .future_tree_layout();
        GameSer {
            seed: g.seed,
            future_tree_layout: Some(future_tree_layout),
            host_research_prerequisites,
            visibility_memory_version: 1,
            difficulty: g.difficulty,
            barbarian_difficulty: g.barbarian_difficulty,
            // `game_speed` is the live, typed setting used by every rules
            // calculation.  Keep the compatibility string in lockstep so a
            // save cannot preserve two conflicting speeds.
            speed: g.game_speed.id().to_string(),
            human_seats: g.human_seats,
            handicap_exempt: g.handicap_exempt,
            ai_yield_rung: g.ai_yield_rung,
            ai_space_project_cost_pct: g.ai_space_project_cost_pct,
            ai_late_science_pct: g.ai_late_science_pct,
            ai_space_tech_floor: g.ai_space_tech_floor,
            mods: g.mods,
            events: g.events.into(),
            base_ruleset: g.base_ruleset,
            start_era: g.start_era,
            future_era: g.future_era,
            turn_structure: g.turn_structure,
            moon_deposits: g.moon_deposits,
            map_script: g.map_script,
            leader_pool: g.leader_pool,
            map_poles: g.map_poles,
            game_speed: g.game_speed,
            max_turns: g.max_turns,
            turn: g.turn,
            current: g.current,
            winner: g.winner,
            victory_type: g.victory_type,
            decided: g.decided,
            victory_conditions: g.victory_conditions,
            mercy_rule: g.mercy_rule,
            mercy_lanes: g.mercy_lanes,
            required_victory_types: g.required_victory_types,
            tactics: g.tactics,
            arena_flags: g.arena_flags,
            victories_won: g.victories_won,
            next_id: g.next_id,
            rng: g.rng,
            at_war: g.at_war.into_iter().collect(),
            observed_military_power: Arc::unwrap_or_clone(g.observed_military_power),
            host_unit_facts: Arc::unwrap_or_clone(g.host_unit_facts),
            host_maintenance: Arc::unwrap_or_clone(g.host_maintenance),
            observed_score: Arc::unwrap_or_clone(g.observed_score),
            observed_rival_wars: Arc::unwrap_or_clone(g.observed_rival_wars),
            observed_city_palaces: Arc::unwrap_or_clone(g.observed_city_palaces),
            observed_city_religion_markers: Arc::unwrap_or_clone(g.observed_city_religion_markers),
            observed_city_faiths: Arc::unwrap_or_clone(g.observed_city_faiths),
            observed_strategic_income_adjustments: Arc::unwrap_or_clone(
                g.observed_strategic_income_adjustments,
            ),
            observed_trade_capacity: Arc::unwrap_or_clone(g.observed_trade_capacity),
            observed_leader_types: Arc::unwrap_or_clone(g.observed_leader_types),
            observed_yield_adjustments: Arc::unwrap_or_clone(g.observed_yield_adjustments),
            observed_public_empire_stats: Arc::unwrap_or_clone(g.observed_public_empire_stats),
            unseen_city_owners: g.unseen_city_owners,
            observed_majority_religion: Arc::unwrap_or_clone(g.observed_majority_religion),
            observed_visiting_tourists: Arc::unwrap_or_clone(g.observed_visiting_tourists)
                .into_iter()
                .collect(),
            observed_city_yield_adjustments: Arc::unwrap_or_clone(
                g.observed_city_yield_adjustments,
            ),
            observed_city_amenity_adjustments: Arc::unwrap_or_clone(
                g.observed_city_amenity_adjustments,
            ),
            observed_city_housing_adjustments: Arc::unwrap_or_clone(
                g.observed_city_housing_adjustments,
            ),
            observed_tile_yield_adjustments: Arc::unwrap_or_clone(
                g.observed_tile_yield_adjustments,
            )
            .into_iter()
            .collect(),
            observed_route_posts: Arc::unwrap_or_clone(g.observed_route_posts)
                .into_iter()
                .collect(),
            observed_great_work_housing: g.observed_great_work_housing.map(Arc::unwrap_or_clone),
            observed_route_yields: Arc::unwrap_or_clone(g.observed_route_yields)
                .into_iter()
                .collect(),
            observed_incoming_route_deltas: Arc::unwrap_or_clone(g.observed_incoming_route_deltas),
            observed_route_options: Arc::unwrap_or_clone(g.observed_route_options)
                .into_iter()
                .collect(),
            observed_appeal: Arc::unwrap_or_clone(g.observed_appeal)
                .into_iter()
                .collect(),
            observed_fresh_water: Arc::unwrap_or_clone(g.observed_fresh_water)
                .into_iter()
                .collect(),
            observed_tourism_per_turn: Arc::unwrap_or_clone(g.observed_tourism_per_turn)
                .into_iter()
                .collect(),
            observed_climate: g.observed_climate,
            observed_city_worked_tiles: Arc::unwrap_or_clone(g.observed_city_worked_tiles),
            observed_city_specialists: Arc::unwrap_or_clone(g.observed_city_specialists),
            observed_city_loyalty_per_turn: Arc::unwrap_or_clone(g.observed_city_loyalty_per_turn),
            observed_settlement_loyalty: g.observed_settlement_loyalty.map(Arc::unwrap_or_clone),
            observed_city_strength: Arc::unwrap_or_clone(g.observed_city_strength),
            observed_city_ranged_strength: Arc::unwrap_or_clone(g.observed_city_ranged_strength),
            observed_city_max_wall_hp: Arc::unwrap_or_clone(g.observed_city_max_wall_hp),
            peace_treaties: g.peace_treaties.into_iter().collect(),
            wars: g.wars.into_iter().collect(),
            concluded_wars: g.concluded_wars,
            nuclear_strikes: g.nuclear_strikes,
            unit_move_trails: g.unit_move_trails,
            barb_pid: g.barb_pid,
            barb_camps: g.barb_camps.into_iter().collect(),
            barb_naval_camps: g.barb_naval_camps.into_iter().collect(),
            barb_camp_guards: g.barb_camp_guards.into_iter().collect(),
            barb_scout_homes: g.barb_scout_homes,
            barb_raider_homes: g.barb_raider_homes,
            barb_scout_targets: g.barb_scout_targets,
            barb_camp_targets: g.barb_camp_targets.into_iter().collect(),
            barb_alerted_until: g.barb_alerted_until.into_iter().collect(),
            routes: g.routes,
            world_era: g.world_era,
            world_era_since: g.world_era_since,
            world_era_countdown_end: g.world_era_countdown_end,
            climate_phase: g.climate_phase,
            meteor_strikes: g.meteor_strikes,
            disaster_intensity: g.disaster_intensity,
            game_modes: g.game_modes,
            storms: g.storms,
            droughts: g.droughts,
            retired_great_people: g.retired_great_people,
            great_person_offer_costs: g.great_person_offer_costs,
            pending_deals: g.pending_deals,
            active_trade_deals: g.active_trade_deals,
            next_deal_id: g.next_deal_id,
            congress: g.congress,
            active_congress_effects: g.active_congress_effects,
            host_competitions: Arc::unwrap_or_clone(g.host_competitions),
            competition: g.competition,
            competition_lockout_until: g.competition_lockout_until,
            native_competitions: g.native_competitions,
            last_special_session: g.last_special_session,
            pending_emergencies: g.pending_emergencies,
            active_emergencies: g.active_emergencies,
            map: g.map,
            players: g.players.into(),
            units: g.units.into_values().collect(),
            spies: g.spies.into_values().collect(),
            cities: g.cities.into_values().collect(),
        }
    }
}

/// What a one-hex step onto a tile is allowed to do.
///
/// Civilization VI checks the stacking layer at the **end** of a move, not at
/// every step, so "may this unit enter that hex" and "may this unit finish
/// there" are two different questions and only the second one asks about a
/// friendly unit standing in the way. See [`Game::entry_at`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// The step is illegal: terrain, a cliff, a border, a hostile city or a
    /// foreign unit. Nothing about waiting or spending more Movement helps.
    Blocked,
    /// The unit may cross this tile but may not end its movement on it,
    /// because one of its own units already holds the same stacking layer.
    Pass,
    /// The unit may cross this tile and may end its movement on it.
    Stop,
}

impl Game {
    /// Remove the generated settlements before a foreign authoritative board is
    /// planted. Map generation seats city-states immediately; a mirror needs their
    /// player slots but must not retain their invented cities or borders.
    pub(crate) fn clear_mirror_cities(&mut self) {
        self.invalidate_routing_paths();
        self.cities.clear();
        self.city_by_pos.clear();
        self.routes.clear();
        Arc::make_mut(&mut self.observed_city_loyalty_per_turn).clear();
        self.observed_settlement_loyalty = None;
        Arc::make_mut(&mut self.observed_city_strength).clear();
        Arc::make_mut(&mut self.observed_city_ranged_strength).clear();
        Arc::make_mut(&mut self.observed_city_palaces).clear();
        Arc::make_mut(&mut self.observed_city_religion_markers).clear();
        Arc::make_mut(&mut self.observed_city_faiths).clear();
        Arc::make_mut(&mut self.observed_city_max_wall_hp).clear();
        Arc::make_mut(&mut self.observed_city_yield_adjustments).clear();
        Arc::make_mut(&mut self.observed_city_amenity_adjustments).clear();
        Arc::make_mut(&mut self.observed_city_housing_adjustments).clear();
        Arc::make_mut(&mut self.observed_tile_yield_adjustments).clear();
        Arc::make_mut(&mut self.observed_route_posts).clear();
        self.observed_great_work_housing = None;
        Arc::make_mut(&mut self.observed_route_yields).clear();
        Arc::make_mut(&mut self.observed_incoming_route_deltas).clear();
        Arc::make_mut(&mut self.observed_route_options).clear();
        Arc::make_mut(&mut self.observed_city_worked_tiles).clear();
        Arc::make_mut(&mut self.observed_city_specialists).clear();
        for tile in self.map.tiles.values_mut() {
            tile.owner_city = None;
            let had_infrastructure = tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some();
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
            if had_infrastructure {
                tile.pillaged = false;
            }
        }
        for player in self.players.iter_mut() {
            player.remembered_cities.clear();
            player.city_directives.clear();
        }
    }

    /// Reconcile an authoritative external city roster without firing conquest
    /// rewards, grievances, population loss, or any other simulated transition.
    pub(crate) fn mirror_set_city_owner(&mut self, cid: u32, owner: usize) {
        let changed = self
            .cities
            .get(&cid)
            .is_some_and(|city| city.owner != owner);
        if let Some(city) = self.cities.get_mut(&cid) {
            city.owner = owner;
        }
        if changed {
            self.invalidate_routing_paths();
        }
        *self.query_memo.city_ids.borrow_mut() = None;
    }

    /// Remove a city that the authoritative mirror says this seat no longer owns.
    /// A later observed owner can place it again in the same synchronization pass.
    pub(crate) fn mirror_remove_city(&mut self, cid: u32) {
        let Some(city) = self.cities.remove(&cid) else {
            return;
        };
        self.invalidate_routing_paths();
        let mut infrastructure: BTreeSet<Pos> = city
            .districts
            .iter()
            .map(|(_, position)| *position)
            .chain(city.wonders.values().copied())
            .collect();
        infrastructure.extend(city.queue.iter().filter_map(|item| match item {
            Item::District { pos, .. } | Item::Wonder { pos, .. } => Some(*pos),
            _ => None,
        }));
        self.city_by_pos.remove(&city.pos);
        for tile in self.map.tiles.values_mut() {
            if tile.owner_city == Some(cid) {
                tile.owner_city = None;
            }
        }
        for position in infrastructure {
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = None;
                tile.pillaged = false;
            }
        }
        Arc::make_mut(&mut self.observed_city_loyalty_per_turn).remove(&cid);
        Arc::make_mut(&mut self.observed_city_strength).remove(&cid);
        Arc::make_mut(&mut self.observed_city_ranged_strength).remove(&cid);
        Arc::make_mut(&mut self.observed_city_palaces).remove(&cid);
        Arc::make_mut(&mut self.observed_city_religion_markers).remove(&cid);
        Arc::make_mut(&mut self.observed_city_faiths).remove(&cid);
        Arc::make_mut(&mut self.observed_city_max_wall_hp).remove(&cid);
        Arc::make_mut(&mut self.observed_city_yield_adjustments).remove(&cid);
        Arc::make_mut(&mut self.observed_city_amenity_adjustments).remove(&cid);
        Arc::make_mut(&mut self.observed_city_housing_adjustments).remove(&cid);
        Arc::make_mut(&mut self.observed_city_worked_tiles).remove(&cid);
        Arc::make_mut(&mut self.observed_city_specialists).remove(&cid);
        for player in self.players.iter_mut() {
            player.city_directives.remove(&cid);
        }
        *self.query_memo.city_ids.borrow_mut() = None;
    }

    /// Copy a constructor template for a newly observed Native board.
    /// Normal speculative clones share reach answers for the same board;
    /// a fresh reconstruction must leave its template's read cache untouched.
    pub(crate) fn clone_for_native_rebuild(&self) -> Self {
        let mut game = self.clone();
        game.attack_reach_cache = Arc::new(std::sync::Mutex::new(AttackReachCache::default()));
        game
    }

    pub fn new(
        num_players: usize,
        width: i32,
        height: i32,
        seed: u64,
        max_turns: u32,
        num_city_states: usize,
    ) -> Game {
        Game::new_full(
            num_players,
            width,
            height,
            seed,
            max_turns,
            num_city_states,
            true,
        )
    }

    pub fn new_full(
        num_players: usize,
        width: i32,
        height: i32,
        seed: u64,
        max_turns: u32,
        num_city_states: usize,
        barbarians: bool,
    ) -> Game {
        Game::new_with(GameOptions {
            barbarians,
            ..GameOptions::new(num_players, width, height, seed, max_turns, num_city_states)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_setup(
        num_players: usize,
        width: i32,
        height: i32,
        seed: u64,
        max_turns: u32,
        num_city_states: usize,
        map_script: MapScript,
        game_speed: GameSpeed,
        barbarians: bool,
    ) -> Game {
        Game::new_with(GameOptions {
            barbarians,
            map_script,
            speed: game_speed.id().to_string(),
            ..GameOptions::new(num_players, width, height, seed, max_turns, num_city_states)
        })
    }

    /// The one real constructor. Everything the setup screen can choose lives
    /// in [`GameOptions`]; the older positional constructors delegate here.
    pub fn new_with(options: GameOptions) -> Game {
        let GameOptions {
            players: num_players,
            width,
            height,
            seed,
            max_turns,
            city_states: num_city_states,
            barbarians,
            base_ruleset,
            start_era,
            future_era,
            map_script,
            map_topology,
            map_poles,
            difficulty,
            barbarian_difficulty,
            speed,
            human_seats,
            handicap_exempt,
            ai_yield_rung,
            ai_space_project_cost_pct,
            ai_late_science_pct,
            ai_space_tech_floor,
            teams,
            disaster_intensity,
            game_modes,
            civs,
            leader_pool,
            randomize_civs,
            turn_structure,
            mercy_rule,
            required_victory_types,
            victory_conditions,
            tactics,
        } = options;
        // Only an arena is handed an economy, and only a sane one. A scenario
        // is handed the one its battle was fought under, whatever was asked
        // for — see `TacticsRules::for_script`.
        let tactics = tactics.for_script(map_script);
        // An arena keeps a battle's clock, not a civilization's. Five hundred
        // turns is the length of a history; a battle on a ten-hex field should
        // end much sooner. The selected deadline is a backstop rather than the
        // usual ending: most battles reach the last army standing first, and
        // one where both sides survive the clock is a draw.

        // A rung past the end of the ladder is clamped rather than fatal: a
        // client from a later build must not be able to construct a world that
        // reads as a later era than the rules have a tree for.
        let start_era = start_era.min(crate::setup::last_start_era());
        // A custom arena's era is the arena's own setting: one rung, a fresh
        // roll per battle, or a pool's latest — see `TacticsEra`. A named
        // scenario keeps the era its battle was fought in, and every other
        // world keeps the start era it was asked for.
        let start_era = if map_script.is_battlefield() && !map_script.is_scenario() {
            tactics.era.start_era(start_era, seed)
        } else {
            start_era
        };
        assert!(
            teams.is_empty() || teams.len() == num_players,
            "team assignments must be empty or contain one entry per major player"
        );
        let leader_pool = leader_pool.available_or_default();
        let mut rules = Rules::for_game(seed, None, future_era);
        assert!(
            rules.difficulties.contains_key(&difficulty),
            "unknown difficulty {difficulty}"
        );
        assert!(
            rules.difficulties.contains_key(&barbarian_difficulty),
            "unknown barbarian difficulty {barbarian_difficulty}"
        );
        assert!(
            rules.speeds.contains_key(&speed),
            "unknown game speed {speed}"
        );
        let mut known_civs = rules.civs.keys().cloned().collect::<BTreeSet<_>>();
        known_civs.extend(
            leader_roster::all()
                .iter()
                .filter(|record| record.available)
                .map(|record| Name::new(&record.civ)),
        );
        // A scenario names both sides. Its seats are the two fleets rather
        // than two civilizations picked from a pool, and seat 0 is the side
        // that historically moved first, so neither the lobby's roster nor a
        // randomizer gets a say — a Trafalgar fought by Nubia and Scythia
        // would be a map with a misleading name on it.
        let seated = match map_script.scenario_civs().filter(|_| num_players == 2) {
            Some(sides) => sides.iter().map(|civ| (*civ).to_string()).collect(),
            None if randomize_civs => {
                let mut roster_rng = Rng::new(seed ^ 0x4349_5656_4953_4349);
                seat_civs_randomized(
                    num_players,
                    &civs,
                    &known_civs,
                    leader_pool,
                    &mut roster_rng,
                )
            }
            None => seat_civs(num_players, &civs, &known_civs, leader_pool),
        };
        if seated
            .iter()
            .any(|civ| !leader_roster::uses_civ6_mechanics(civ))
        {
            leader_roster::install_neutral_identities(Arc::make_mut(&mut rules), &seated);
        }
        let leader_starts: Vec<_> = seated
            .iter()
            .map(|civ| {
                leader_roster::true_start(civ)
                    .unwrap_or_else(|| panic!("selected leader {civ:?} has no true-start point"))
            })
            .collect();
        // Which city-states this world seats, decided before the map so their
        // own sites can steer where they land on a true-start Earth. Its own
        // stream, like the civilization draw above: a world's city-states
        // changing must not shift a single number the generator rolls, or
        // every existing seed would generate a different map.
        let mut minor_rng = Rng::new(seed ^ 0x4d49_4e4f_5253_4954);
        let city_state_seats = crate::city_states::seat_selection(
            &rules.city_states.roster,
            num_city_states,
            &mut minor_rng,
        );
        let minor_starts: Vec<_> = city_state_seats
            .iter()
            .map(|index| rules.city_states.roster[*index].site())
            .collect();
        let mut rng = Rng::new(seed);
        let map_size = MapSize::from_dimensions(width, height)
            .unwrap_or_else(|| MapSize::for_players(num_players));
        let (map, spawns) = mapgen::generate_with_script_and_leader_starts(
            &rules,
            width,
            height,
            num_players,
            num_city_states,
            map_size.natural_wonders,
            map_size.continents,
            map_script,
            map_topology,
            map_poles,
            &leader_starts,
            &minor_starts,
            &mut rng,
        );
        // After the world, deliberately: a ruleset with nothing on the Moon
        // draws no numbers here, so a classic game rolls exactly the map it
        // always did from a given seed.
        let moon_deposits = Self::roll_moon_deposits(&rules, &mut rng);
        let game_speed = GameSpeed::from_id(&speed).unwrap_or(GameSpeed::Standard);
        let mut g = Game {
            rules,
            vision: std::cell::RefCell::new(VisionCache::default()),
            vision_frames: VisionFrameCache::default(),
            routing: std::cell::RefCell::new(RoutingCache::default()),
            route_scratch: std::cell::RefCell::new(RouteScratch::default()),
            query_memo: QueryCache::default(),
            view_rules: Default::default(),
            attack_reach_cache: Arc::new(std::sync::Mutex::new(AttackReachCache::default())),
            remembered_under: SharedVec::default(),
            track_fog_memory: true,
            track_war_ledger: true,
            planning_role: PlanningRole::Off,
            visibility_batch: VisibilityBatch::default(),
            visibility_suppressed: false,
            rng,
            seed,
            difficulty,
            barbarian_difficulty,
            speed,
            human_seats,
            handicap_exempt,
            ai_yield_rung,
            ai_space_project_cost_pct,
            ai_late_science_pct,
            ai_space_tech_floor,
            base_ruleset,
            start_era,
            future_era,
            turn_structure,
            moon_deposits,
            map_script,
            leader_pool,
            map_poles,
            game_speed,
            mods: crate::mods::active_names(),
            max_turns,
            turn: 1,
            current: 0,
            winner: None,
            victory_type: None,
            decided: None,
            victory_conditions,
            mercy_rule,
            mercy_lanes: Vec::new(),
            required_victory_types,
            tactics,
            arena_flags: BTreeMap::new(),
            victories_won: BTreeMap::new(),
            next_id: 1,
            map,
            players: Players::default(),
            units: Units::default(),
            spies: BTreeMap::new(),
            cities: Cities::default(),
            at_war: BTreeSet::new(),
            observed_military_power: Arc::new(BTreeMap::new()),
            host_unit_facts: Arc::new(BTreeMap::new()),
            host_maintenance: Arc::new(BTreeMap::new()),
            observed_score: Arc::new(BTreeMap::new()),
            observed_rival_wars: Arc::new(BTreeSet::new()),
            observed_city_palaces: Arc::new(BTreeMap::new()),
            observed_city_religion_markers: Arc::new(BTreeSet::new()),
            observed_city_faiths: Arc::new(BTreeMap::new()),
            observed_strategic_income_adjustments: Arc::new(BTreeMap::new()),
            observed_trade_capacity: Arc::new(BTreeMap::new()),
            observed_leader_types: Arc::new(BTreeMap::new()),
            observed_yield_adjustments: Arc::new(BTreeMap::new()),
            observed_public_empire_stats: Arc::new(BTreeMap::new()),
            unseen_city_owners: BTreeSet::new(),
            observed_majority_religion: Arc::new(BTreeMap::new()),
            observed_visiting_tourists: Arc::new(BTreeMap::new()),
            observed_city_yield_adjustments: Arc::new(BTreeMap::new()),
            observed_city_amenity_adjustments: Arc::new(BTreeMap::new()),
            observed_city_housing_adjustments: Arc::new(BTreeMap::new()),
            observed_tile_yield_adjustments: Arc::new(BTreeMap::new()),
            observed_route_posts: Arc::new(BTreeMap::new()),
            observed_great_work_housing: None.map(Arc::new),
            observed_route_yields: Arc::new(BTreeMap::new()),
            observed_incoming_route_deltas: Arc::new(BTreeMap::new()),
            observed_city_worked_tiles: Arc::new(BTreeMap::new()),
            observed_city_specialists: Arc::new(BTreeMap::new()),
            observed_city_loyalty_per_turn: Arc::new(BTreeMap::new()),
            observed_settlement_loyalty: None,
            observed_city_strength: Arc::new(BTreeMap::new()),
            observed_city_ranged_strength: Arc::new(BTreeMap::new()),
            observed_city_max_wall_hp: Arc::new(BTreeMap::new()),
            observed_wall_tier_rules: false,
            lump_quote_viewer: None,
            nusantara_site_planner: None,
            observed_builders_avoid_mountains: false,
            heist_reads_the_host_menu: false,
            spy_targets_read_the_host: false,
            blocked_city_sites: Arc::new(BTreeSet::new()),
            host_observed: Arc::new(BTreeSet::new()),
            observed_appeal: Arc::new(BTreeMap::new()),
            observed_fresh_water: Arc::new(BTreeMap::new()),
            observed_tourism_per_turn: Arc::new(BTreeMap::new()),
            observed_route_options: Arc::new(BTreeMap::new()),
            observed_climate: None,
            closed_borders: BTreeSet::new(),
            unseen_major_borders: BTreeSet::new(),
            sealed_border_owners: BTreeMap::new(),
            rock_band_barred_hosts: BTreeSet::new(),
            blocked_improvement_sites: Arc::new(BTreeSet::new()),
            blocked_improvements: Arc::new(BTreeMap::new()),
            great_person_plots: BTreeMap::new(),
            blocked_promotions: Arc::new(BTreeMap::new()),
            host_band_promotions: Arc::new(BTreeMap::new()),
            blocked_strikes: Arc::new(BTreeSet::new()),
            host_war_blocks: Arc::new(BTreeSet::new()),
            host_war_type_permissions: Arc::new(BTreeMap::new()),
            host_envoy_permissions: Arc::new(BTreeMap::new()),
            host_previews: Arc::new(BTreeMap::new()),
            blocked_trade_routes: Arc::new(BTreeSet::new()),
            blocked_policies: Arc::new(BTreeSet::new()),
            host_policy_choices: BTreeMap::new(),
            host_policy_windows: BTreeMap::new(),
            blocked_pantheons: Arc::new(BTreeSet::new()),
            blocked_districts: Arc::new(BTreeMap::new()),
            host_district_sites: Arc::new(BTreeMap::new()),
            host_wonder_sites: Arc::new(BTreeMap::new()),
            blocked_wonders: Arc::new(BTreeMap::new()),
            host_unavailable_wonders: Arc::new(BTreeSet::new()),
            blocked_production: Arc::new(BTreeMap::new()),
            blocked_purchases: Arc::new(BTreeMap::new()),
            host_buildable: Arc::new(BTreeMap::new()),
            host_saved_work: Arc::new(BTreeMap::new()),
            host_research_quotes: Arc::new(BTreeMap::new()),
            host_research_base_rules: None,
            host_revealed_research: Arc::new(BTreeSet::new()),
            host_research_eligibility: Arc::new(BTreeMap::new()),
            host_unit_resource_prices: Arc::new(BTreeMap::new()),
            host_purchasable: Arc::new(BTreeMap::new()),
            host_plot_purchases: Arc::new(BTreeMap::new()),
            host_district_plots: Arc::new(BTreeMap::new()),
            host_district_capacity: Arc::new(BTreeMap::new()),
            peace_treaties: BTreeMap::new(),
            wars: BTreeMap::new(),
            siege: SiegeCensus::default(),
            concluded_wars: Vec::new(),
            nuclear_strikes: Vec::new(),
            unit_move_trails: Vec::new(),
            barb_pid: None,
            barb_camps: BTreeMap::new(),
            barb_naval_camps: BTreeSet::new(),
            barb_camp_guards: BTreeMap::new(),
            barb_scout_homes: BTreeMap::new(),
            barb_raider_homes: BTreeMap::new(),
            barb_scout_targets: BTreeMap::new(),
            barb_camp_targets: BTreeMap::new(),
            barb_alerted_until: BTreeMap::new(),
            routes: Vec::new(),
            world_era: start_era,
            world_era_since: 0,
            world_era_countdown_end: None,
            climate_phase: 0,
            meteor_strikes: 0,
            disaster_intensity,
            game_modes,
            storms: Vec::new(),
            droughts: Vec::new(),
            retired_great_people: BTreeSet::new(),
            great_person_offer_costs: BTreeMap::new(),
            pending_deals: Vec::new(),
            active_trade_deals: Vec::new(),
            next_deal_id: 1,
            congress: None,
            active_congress_effects: Vec::new(),
            host_competitions: Arc::new(Vec::new()),
            competition: None,
            competition_lockout_until: BTreeMap::new(),
            native_competitions: false,
            last_special_session: 0,
            pending_emergencies: Vec::new(),
            active_emergencies: Vec::new(),
            occ: BTreeMap::new(),
            city_by_pos: BTreeMap::new(),
            log: crate::actionlog::ActionLog::new(),
            events: EventLog::default(),
        };
        // Civilization VI decides *which* start a civilization is given from
        // its shipped StartBias rows. Reorder the major sites to honour them
        // before any seat is handed one; True Start Earth is already ordered
        // by the selected roster’s exact coordinates and must not be moved by
        // a generic bias afterward. Neither is a scenario, for the same
        // reason twice over: its seats are the two commanders' own, and a
        // fleet has no terrain preference to bias toward anyway.
        let mut spawns = spawns;
        if map_script != MapScript::TrueStartEarth && !map_script.is_scenario() {
            mapgen::assign_starts_by_bias(&g.rules, &g.map, &mut spawns[..num_players], &seated);
        }
        for (i, civ) in seated.iter().take(num_players).enumerate() {
            let mut player = Player::new(i, civ, false);
            player.team = teams.get(i).copied().flatten();
            g.players.push(player);
        }
        for (i, pos) in spawns.iter().take(num_players).enumerate() {
            if map_script.is_battlefield() {
                // A Tactics seat is dropped in as an army, plus at most the
                // one city its economy is granted through. Never a Settler:
                // the arena's single city is placed here at setup or the side
                // has none for the whole battle, so there is no decision
                // about where or whether to found and no early game spent
                // walking to a better spot. No handicap units either, because
                // the whole claim of the arena is that the two sides are even
                // and a difficulty bonus would decide the battle at setup.
                // Both seats receive the identical roster, laid out over
                // their own end of the field.
                if tactics.cities > 0 {
                    g.found_city_for(i, *pos, None);
                }
                // A flag stands exactly where this side's city would have:
                // the two are alternative answers to the same question, what
                // this side holds and the other side is coming for. Planted
                // before the army is laid out so the deployment can gather
                // around it.
                //
                // At sea it stands off the shore instead. A seat on the naval
                // globe is an island, and no fleet can sail onto one, so a
                // flag planted on the seat itself would be a flag the enemy
                // could never take — a capture-the-flag battle that can only
                // end on the clock. The anchorage is the honest equivalent:
                // the nearest open water to the seat, which is exactly the
                // tile an attacking fleet has to fight its way onto and the
                // defending one has to hold.
                if tactics.flag {
                    let site = if map_script.is_naval_battlefield() {
                        g.nearest_open_water(*pos)
                    } else {
                        Some(*pos)
                    };
                    if let Some(site) = site {
                        g.arena_flags.insert(i, site);
                    }
                }
                // A scenario deals each side its own historical order of
                // battle instead, on the ground that side actually held.
                if map_script.is_scenario() {
                    if map_script == MapScript::Trafalgar {
                        g.deploy_trafalgar_fleet(i);
                    } else {
                        g.deploy_historical_scenario(i, *pos);
                    }
                } else {
                    g.deploy_battlefield_army(i, *pos, map_script);
                }
                g.reveal(i, *pos, 3);
                continue;
            }
            g.spawn_unit("settler", i, *pos);
            g.spawn_unit("warrior", i, *pos);
            // Above Prince the AI seats open with extra units, exactly as the
            // shipped `Eras.xml` bonus start table describes. An exempt seat
            // opens like a human one: Settler and Warrior only.
            if !g.is_human_seat(i) && !g.is_handicap_exempt(i) {
                let bonus = g.difficulty_spec().ai_bonus_units.clone();
                for (kind, count) in bonus {
                    for _ in 0..count {
                        g.spawn_unit(&kind, i, *pos);
                    }
                }
            }
            g.reveal(i, *pos, 3);
        }
        if map_script.is_battlefield() {
            // The two sides of an arena know perfectly well who they are
            // facing: they were set down at opposite ends of the same field
            // to fight each other. Making the introduction explicit keeps the
            // battle from waiting on somebody's line of sight to reach the
            // far wall before either army will act on the other.
            for first in 0..num_players {
                for second in (first + 1)..num_players {
                    g.record_contact(first, second);
                }
            }
        }
        let major_spawns: Vec<Pos> = spawns.iter().take(num_players).cloned().collect();
        // Seating order is the draw made above, not `CITY_STATE_NAMES`' and no
        // longer the roster's own order either. `city_states::seat_selection`
        // deals the six Suzerain types round-robin and, within a type, takes
        // the identity whose real site is farthest from the ones already
        // taken — so a world's city-states come from six types and six
        // continents rather than from the top of a list that runs through
        // Magna Graecia and the Levant in blocks. It still empties the shipped
        // forty-eight before reaching any other identity, so an ordinary game
        // seats only city-states the real game could have seated.
        //
        // `seats[i]` matches `minor_starts[i]`, which is what let the map
        // generator put each of them on its own ground.
        let seating: Vec<String> = city_state_seats
            .iter()
            .map(|index| g.rules.city_states.roster[*index].name.clone())
            .collect();
        // Only as many city-states as the ruleset has distinct identities for:
        // every modeled city-state carries its own Suzerain bonus, and two
        // seats sharing a name would share that bonus.
        let wanted = num_city_states.min(seating.len());
        for pos in spawns.iter().skip(num_players) {
            if g.players.len() - num_players >= wanted {
                break;
            }
            // A spawn the generator had to pack in beside a neighbour is
            // relocated rather than abandoned; dropping it silently produced
            // games with a third of the city-states that were asked for.
            let Some(pos) = g.city_state_site(*pos, &major_spawns) else {
                continue;
            };
            let pid = g.players.len();
            let name = seating[pid - num_players].clone();
            g.players.push(Player::new(pid, &name, true));
            let city = g.found_city_for(pid, pos, Some(name.clone()));
            // Gathering Storm's Ancient-era minor start is two Warriors, with
            // one more per difficulty step from Emperor onward. City-states do
            // not receive the major AI's Builders or Settlers from the general
            // handicap table.
            let warriors = 2 + g.difficulty_spec().order.saturating_sub(4);
            for _ in 0..warriors {
                g.place_city_state_starting_unit("warrior", pid, pos);
            }
            // The shipped `StartingBuildings` table grants completed Ancient
            // Walls to every city-state from Immortal upward. Which rungs and
            // which building is `difficulties.json`'s to say now, rather than
            // a rung number and a wall pool written into the setup code.
            g.grant_starting_buildings(city);
        }
        // Rise & Fall always reserves a non-playable Free Cities seat. It is
        // dormant until the first Loyalty revolt, so it neither receives a
        // start nor consumes a turn in games where every city stays loyal.
        g.ensure_free_city_player();
        // No third force on a battlefield: barbarian camps on an arena a
        // dozen tiles across would decide more battles than the two sides do.
        let barbarians = barbarians && !map_script.is_battlefield();
        if barbarians {
            let pid = g.players.len();
            let mut barb = Player::new(pid, "Barbarians", true);
            barb.is_barbarian = true;
            g.players.push(barb);
            g.barb_pid = Some(pid);
            // BARBARIAN_CAMP_FIRST_TURN_PERCENT_OF_TARGET_TO_ADD: a third of
            // the world's camp target is already on the map when the game
            // opens, rather than a flat pair regardless of player count. Every
            // one of them lands in the dark beyond the starting positions —
            // nothing has moved yet, so the world's sight is worked out once
            // and the whole opening is placed against it.
            let opening = g.barbarian_camp_target() * 33 / 100;
            let watched = g.watched_by_the_living();
            for _ in 0..opening.max(1) {
                g.spawn_camp_unwatched(&watched);
            }
        }
        g.open_in_start_era();
        // Each side of an arena is studying something from its first turn:
        // the arena chooses its own research, identically for both — after
        // the start era has dealt the techs the choice is made from.
        if g.is_arena() {
            for seat in 0..num_players {
                g.arena_auto_research(seat);
            }
        }
        g.refresh_great_person_offers();
        g.refresh_all_visibility();
        g
    }

    /// Give a city the completed buildings the shipped `StartingBuildings`
    /// table grants a city of its owner's kind at this difficulty.
    ///
    /// *Completed* is the whole of it. These are not a production head start
    /// and not a discount: they are standing when the game opens, with no
    /// technology behind them and nothing left to build. So the building goes
    /// into `buildings`, which is the list every yield, adjacency, tourism
    /// and defence rule in the engine reads, rather than into a counter of
    /// its own — a city-state at Deity is one the challenger has to breach,
    /// not one carrying a note that says it has walls.
    ///
    /// Anything with an outer defence tops the wall pool up exactly the way
    /// finishing it in a city does, through `city_max_wall_hp`, so a second
    /// tier or a mod's own walls need no arithmetic here.
    ///
    /// ⚠ `MinorOnly` is a partition, not a permission: a row is granted to
    /// city-states or to majors, never to both, which is why one flag decides
    /// each side. Every major-side row of the shipped table is keyed on the
    /// start era and carries no `MinDifficulty` at all, so with the shipped
    /// ladder this reaches only city-states.
    fn grant_starting_buildings(&mut self, cid: u32) {
        let minor = self.players[self.cities[&cid].owner].is_minor;
        let granted: Vec<Name> = self
            .difficulty_spec()
            .starting_buildings
            .iter()
            .filter(|row| row.minor_only == minor)
            .map(|row| Name::new(&row.building))
            .collect();
        for building in granted {
            if self.cities[&cid].buildings.contains(&building) {
                continue;
            }
            let outer_defense = self.rules.buildings[&building].outer_defense;
            // Setup runs before `open_in_start_era` moves the world era, so a
            // start-era grant is stamped with the Ancient era the shipped row
            // itself names.
            let era = self.world_era;
            let city = self.cities.get_mut(&cid).unwrap();
            city.buildings.push(building);
            city.building_eras.insert(building, era);
            if outer_defense > 0 {
                let pool = self.city_max_wall_hp(&self.cities[&cid]);
                let city = self.cities.get_mut(&cid).unwrap();
                city.wall_hp = (city.wall_hp + outer_defense).min(pool);
            }
        }
    }

    /// Where a city-state assigned to `preferred` should actually settle.
    /// That tile is used when it clears every major start and existing city by
    /// the stock four-tile minimum; otherwise the roomiest legal tile on the
    /// map stands in, and `None` means the map has no room left at all.
    fn city_state_site(&self, preferred: Pos, major_spawns: &[Pos]) -> Option<Pos> {
        let clear = |pos: Pos| {
            major_spawns
                .iter()
                .all(|start| self.wdist(pos, *start) >= 4)
                && self.cities.values().all(|c| self.wdist(pos, c.pos) >= 4)
        };
        if clear(preferred) {
            return Some(preferred);
        }
        self.map
            .tiles
            .values()
            .filter(|tile| {
                !self.rules.is_water(tile)
                    && self.rules.is_passable(tile)
                    && tile.improvement.is_none()
                    && tile.owner_city.is_none()
                    && clear(tile.pos)
            })
            .max_by_key(|tile| {
                let room = major_spawns
                    .iter()
                    .map(|start| self.wdist(tile.pos, *start))
                    .chain(self.cities.values().map(|c| self.wdist(tile.pos, c.pos)))
                    .min()
                    .unwrap_or(i32::MAX);
                (room, tile.pos)
            })
            .map(|tile| tile.pos)
    }

    /// Setup can grant more same-class units than fit on a coastal city's
    /// center and first ring. Ordinary production rightly fails when that local
    /// placement area is full; starting forces instead spill into the nearest
    /// legal empty tile inside the city-state's local defense area.
    /// The army one side of a Tactics battlefield is dropped in with, on an
    /// arena of `tiles` hexes of ground.
    ///
    /// A company is the Ancient counter cycle entire, so that no unit on the
    /// field is without something that beats it and something it beats:
    /// melee, anti-cavalry that punishes a charge, ranged that punishes
    /// standing still, and the light and heavy horse that punish an
    /// unsupported flank. Both sides receive the same list, so what separates
    /// them is where they put it.
    ///
    /// Bigger fields take more companies, at a constant density of roughly
    /// one unit per twelve hexes of ground — the same battle on more room
    /// rather than the same army wandering a larger map looking for it.
    pub fn battlefield_army(tiles: usize, script: MapScript) -> Vec<&'static str> {
        const COMPANY: [&str; 8] = [
            "warrior",
            "warrior",
            "spearman",
            "spearman",
            "archer",
            "archer",
            "horseman",
            "heavy_chariot",
        ];
        // A squadron is the Ancient naval cycle entire, which is a shorter
        // cycle than the land one: the ruleset gives the age exactly two
        // warships, and they answer each other. The galley closes and boards,
        // which is what punishes a quadrireme for keeping its distance; the
        // quadrireme shoots, which is what punishes a galley for crossing the
        // open water between them. There is no third arm at sea in this era —
        // no anti-cavalry to hold a line and no horse to turn a flank — so
        // the squadron is an even split rather than the land company's spread
        // across five roles.
        const SQUADRON: [&str; 8] = [
            "galley",
            "quadrireme",
            "galley",
            "quadrireme",
            "galley",
            "quadrireme",
            "galley",
            "quadrireme",
        ];
        // Bigger fields take more companies, but only up to two. Density is
        // not the point past that: a Tactics battle is decided by what a
        // manageable force does with the room it has, and a diameter-20 globe
        // filled to a constant one-unit-per-twelve-hexes would be a different
        // mode — hundreds of units a side, and a turn spent issuing orders
        // rather than a battle. The larger globes are deliberately the same
        // battle given more room to be fought over.
        let companies = tiles.div_ceil(100).clamp(1, 2);
        let order = if script.is_naval_battlefield() {
            &SQUADRON
        } else {
            &COMPANY
        };
        order
            .iter()
            .cycle()
            .take(order.len() * companies)
            .copied()
            .collect()
    }

    /// The closest water a ship could be on to `anchor`, or `None` if there is
    /// none within reach of it.
    ///
    /// Ties break on position so the same world always answers the same tile:
    /// a naval arena plants its flag with this, and a flag that moved between
    /// two runs of the same seed would make the battle unreproducible.
    fn nearest_open_water(&self, anchor: Pos) -> Option<Pos> {
        // Four rings out. A seat on the naval globe is an islet a handful of
        // tiles across with open sea around it, so the answer is normally one
        // or two steps away; the radius is a bound on the search rather than a
        // distance the water is expected to be at.
        let mut candidates = self.wdisk(anchor, 4);
        candidates.sort_by_key(|pos| (self.wdist(anchor, *pos), *pos));
        candidates.into_iter().find(|pos| {
            self.map
                .get(*pos)
                .is_some_and(|tile| self.rules.is_passable(tile) && self.rules.is_water(tile))
        })
    }

    /// Set out one side's opening army on a Tactics battlefield.
    ///
    /// The army is laid out over the side's own end of the field, working
    /// outward from its start, rather than stacked on the start itself: a
    /// column of units on one hex is not a formation, it is a queue, and the
    /// first turns of the battle would be spent unstacking it. Only one
    /// military unit stands on a hex, so "the next free ground outward" is
    /// the whole rule.
    ///
    /// Which tiles count as "outward" is the one thing the element changes. A
    /// column forms on dry ground and a squadron forms on open water, so the
    /// search is filtered to the element the roster can actually stand on —
    /// `is_passable` alone would seat a galley on a hill, since a land tile is
    /// passable to something. On the naval globe the anchor is the seat's
    /// island, so the squadron rings the shore it sails from.
    fn deploy_battlefield_army(&mut self, pid: usize, anchor: Pos, script: MapScript) {
        let afloat = script.is_naval_battlefield();
        let mut ground: Vec<Pos> = self
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                let tile = &self.map.tiles[pos];
                self.rules.is_passable(tile) && self.rules.is_water(tile) == afloat
            })
            .collect();
        ground.sort_by_key(|pos| (self.wdist(anchor, *pos), *pos));
        let mut cursor = 0;
        for kind in Self::battlefield_army(self.map.tiles.len(), script) {
            while ground
                .get(cursor)
                .is_some_and(|pos| !self.unit_ids_at(*pos).is_empty())
            {
                cursor += 1;
            }
            let Some(pos) = ground.get(cursor).copied() else {
                // The arena ran out of ground. Every offered size has room for
                // its own roster several times over, so this is a floor under
                // a hand-built field rather than a case the mode reaches.
                break;
            };
            cursor += 1;
            // Whatever this civilization fields in that unit's place, which
            // with unique units switched off is the stock unit itself. Only
            // the replacements reach the opening roster: a unique that
            // replaces nothing — Sumeria's War Cart, Egypt's Maryannu Chariot
            // Archer — is an addition to a build menu in the shipped rules
            // rather than a substitution, and stays one here.
            let kind = self.player_unit_replacement(pid, Name::new(kind));
            self.spawn_unit(kind.as_str(), pid, pos);
        }
    }

    /// Set out one side's fleet for the Trafalgar scenario.
    ///
    /// Nothing is searched for and nothing is scaled: every ship goes on the
    /// tile [`crate::trafalgar`] records her on, because the position is the
    /// thing the scenario is claiming and a deployment that drifted to the
    /// nearest free water would quietly stop being Trafalgar. The tables are
    /// held to that promise by their own tests — sixty distinct tiles, all of
    /// them open water on the chart — so a tile that is somehow occupied here
    /// is a bug in the caller rather than a case to place around, and the ship
    /// is left off rather than moved somewhere she was not.
    ///
    /// No unique-unit substitution: `TacticsRules::for_script` switches uniques
    /// off for every scenario, and a Sea Dog in Nelson's line would be a claim
    /// about England that this battle is not making.
    ///
    /// Each ship is granted the promotions her rate is worth — see
    /// `trafalgar::rate_promotions`, which is where that ladder is argued.
    /// Granted at setup rather than earned, which is the only difference
    /// between this and a veteran unit; the engine reads a promotion off the
    /// unit and never asks how it got there.
    fn deploy_trafalgar_fleet(&mut self, pid: usize) {
        for ship in crate::trafalgar::fleet(pid) {
            let pos = crate::hex::offset_to_axial(ship.at.0, ship.at.1);
            debug_assert!(
                self.map
                    .get(pos)
                    .is_some_and(|tile| self.rules.is_water(tile)),
                "{} was ordered onto ({}, {}), which is not water",
                ship.name,
                ship.at.0,
                ship.at.1
            );
            if self.map.get(pos).is_none() || !self.unit_ids_at(pos).is_empty() {
                continue;
            }
            let uid = self.spawn_unit(crate::trafalgar::SHIP_OF_THE_LINE, pid, pos);
            let promotions = crate::trafalgar::rate_promotions(ship.guns);
            let unit = self.units.get_mut(&uid).unwrap();
            unit.promotions
                .extend(promotions.iter().map(|name| Name::new(name)));
            // A unit's level is what its promotion count has bought, so the
            // two are set together rather than left to disagree on the HUD.
            unit.level = 1 + promotions.len() as i32;
            // The flag officer aboard, if any: every flagship is better
            // handled, and a good enough admiral is worth a fighting tier as
            // well. `trafalgar::admiral_formation` argues where that line sits.
            if ship.stars > 0 {
                unit.bonus_moves += crate::trafalgar::ADMIRAL_MOVEMENT_BONUS;
                unit.formation = crate::trafalgar::admiral_formation(ship.stars);
            }
        }
    }

    /// Deploy a catalogue scenario's compact order of battle around its fixed
    /// chart anchor.  The scenario data names real ruleset units, so the
    /// briefing and the opening board agree even though the engine compresses
    /// a regiment, squadron, or carrier group into one Civ-style unit.
    fn deploy_historical_scenario(&mut self, pid: usize, anchor: Pos) {
        let Some(units) = crate::historical_scenarios::force_units(self.map_script, pid) else {
            return;
        };
        // Along the line this side actually formed on, where the battle's own
        // chart draws one: an army that stood in a line should open in a line,
        // not in a clump around one tile. `front_tiles` orders the ground
        // along the front first and away from it second, and the plan names
        // the historically decisive wing first, so the order of battle fills
        // from that wing outward. A force with both feet and keels — an
        // amphibious landing — needs both elements ordered, so each is asked
        // for separately and the unit's own domain picks between them.
        let plan = crate::historical_scenarios::by_script(self.map_script)
            .and_then(|scenario| crate::historical_terrain::by_id(scenario.id));
        let line = |water: bool| {
            plan.map(|plan| crate::historical_terrain::front_tiles(&self.map, plan, pid, water))
        };
        let (ashore, afloat) = (line(false), line(true));
        let mut ground: Vec<Pos> = self.map.tiles.keys().copied().collect();
        ground.sort_by_key(|position| (self.wdist(anchor, *position), *position));
        for kind in units {
            let Some(spec) = self.rules.units.get(kind) else {
                continue;
            };
            let wants_water = spec.domain.as_deref() == Some("sea");
            let standing = if wants_water { &afloat } else { &ashore };
            let open = |position: &Pos| {
                self.map.get(*position).is_some_and(|tile| {
                    self.rules.is_passable(tile)
                        && self.rules.is_water(tile) == wants_water
                        && self.unit_ids_at(*position).is_empty()
                })
            };
            // The front first; the anchor's own neighbourhood is the fallback
            // for a piece the drawn line has no room for, and for the charts
            // that carry their own deployment (Trafalgar).
            let position = standing
                .as_ref()
                .and_then(|line| line.iter().copied().find(open))
                .or_else(|| ground.iter().copied().find(open));
            if let Some(position) = position {
                self.spawn_unit(kind, pid, position);
            }
        }
    }

    pub fn difficulty_spec(&self) -> &DifficultySpec {
        &self.rules.difficulties[&self.difficulty]
    }

    /// ⭐ The rung whose yield bonus (`ai_yield_pct`) a handicapped AI seat
    /// plays: the game's own rung unless `ai_yield_rung` names another (a key
    /// this ruleset lacks falls back to the game's rung). Every discrete grant
    /// — bonus start units, era boosts, combat strength, experience — always
    /// reads [`Game::difficulty_spec`]: a calibrated rival field keeps the
    /// rung's start and fight and converts a lower rung's percentage.
    pub fn handicap_yield_spec(&self) -> &DifficultySpec {
        self.ai_yield_rung
            .as_ref()
            .and_then(|rung| self.rules.difficulties.get(rung))
            .unwrap_or_else(|| self.difficulty_spec())
    }

    pub fn speed_spec(&self) -> &SpeedSpec {
        &self.rules.speeds[&self.speed]
    }

    pub fn is_human_seat(&self, pid: usize) -> bool {
        self.human_seats.contains(&pid)
    }

    /// Whether this seat was declared to play the rung without its handicap
    /// (`Game::handicap_exempt`). Read by every handicap the rung applies to
    /// a major, so one set covers yields, strength, XP, start units, era
    /// boosts and the below-Prince human bonuses alike.
    pub fn is_handicap_exempt(&self, pid: usize) -> bool {
        self.handicap_exempt.contains(&pid)
    }

    /// One of the four space projects a Science victory is built from.
    pub fn is_space_project(project: &str) -> bool {
        matches!(
            project,
            "launch_earth_satellite"
                | "launch_moon_landing"
                | "launch_mars_colony"
                | "exoplanet_expedition"
        )
    }

    /// A handicapped AI major: the seats the calibration knobs reach.
    fn calibrated_rival_seat(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .is_some_and(|player| !player.is_minor && !player.is_barbarian && !player.is_free_city)
            && !self.is_handicap_exempt(pid)
            && !self.is_human_seat(pid)
    }

    /// The factor on `project`'s cost for `pid`: `ai_space_project_cost_pct`
    /// over 100 for one of the four space projects built by a handicapped AI
    /// major, else 1. Always 1 when the option is unset, which every live game
    /// and every save written before it is.
    pub fn space_project_cost_scale(&self, pid: usize, project: &str) -> f64 {
        match self.ai_space_project_cost_pct {
            Some(pct) if Self::is_space_project(project) && self.calibrated_rival_seat(pid) => {
                f64::from(pct) / 100.0
            }
            _ => 1.0,
        }
    }

    /// Whether `pid` may start `project` under `ai_space_tech_floor`: a
    /// handicapped AI major needs that many techs for a space project. Always
    /// true when the option is unset.
    pub fn space_project_tech_floor_met(&self, pid: usize, project: &str) -> bool {
        match self.ai_space_tech_floor {
            Some(floor) if Self::is_space_project(project) && self.calibrated_rival_seat(pid) => {
                self.players[pid].techs.len() >= floor as usize
            }
            _ => true,
        }
    }

    /// Whether this world is a Tactics arena rather than a Civ world.
    ///
    /// The map script is the mode's only marker — there is no separate mode
    /// field on `Params` or on a save — so everything the arena decides
    /// differently asks this. What it decides: the two sides are dropped in
    /// as armies and nothing else, no city is ever founded, an army costs
    /// nothing to keep because there is no empire behind it to earn the gold,
    /// and a side is alive exactly as long as it still has a unit standing.
    pub fn is_arena(&self) -> bool {
        self.map_script.is_battlefield()
    }

    /// The Science an arena hands one side this turn.
    ///
    /// A pace rather than a figure, because a technology's cost climbs some
    /// seventeenfold from the Ancient era to the Information one: any flat
    /// grant either floods the opening or stalls halfway up the tree. Dividing
    /// what the current research actually costs by the configured pace lands
    /// an unlock every `turns_per_tech` turns in every era and at every game
    /// speed, identically for both sides, with no table to keep current.
    fn arena_science(&self, pid: usize) -> f64 {
        let pace = self.tactics.turns_per_tech;
        if pace == 0 {
            return 0.0;
        }
        // Before a side has chosen what to research, price the pace against
        // the cheapest thing it could choose: the overflow it banks is then
        // worth about one turn of the research it is about to start, rather
        // than of some Information-era tech it cannot reach.
        let cost = match self.players[pid].research.clone() {
            Some(tech) => self.tech_cost(tech.as_str()),
            None => self
                .rules
                .techs
                .keys()
                .filter(|name| !self.players[pid].techs.contains(*name))
                .map(|name| self.tech_cost(name.as_str()))
                .fold(f64::INFINITY, f64::min),
        };
        // The whole tree is researched: there is nothing left to pace toward.
        if !cost.is_finite() {
            return 0.0;
        }
        cost / f64::from(pace)
    }

    /// Pick an arena side's next technology for it.
    ///
    /// Research on a battlefield is a pace, not a decision. `turns_per_tech`
    /// says how often the tree moves, and it moves the same way for both
    /// sides — the cheapest technology either can reach, ties broken by
    /// name — so the two armies stay armed alike and nobody is ever asked
    /// to choose. Runs at the top of a side's turn before the pace is paid,
    /// so the Science lands on a technology rather than in overflow, and
    /// once when the arena is built, so a side is studying something from
    /// its first turn. A pace of 0 freezes the tree and picks nothing.
    fn arena_auto_research(&mut self, pid: usize) {
        if self.tactics.turns_per_tech == 0 || self.players[pid].research.is_some() {
            return;
        }
        let pick = self.available_techs(pid).into_iter().min_by(|a, b| {
            self.tech_cost(a.as_str())
                .total_cmp(&self.tech_cost(b.as_str()))
                .then_with(|| a.cmp(b))
        });
        if let Some(tech) = pick {
            self.begin_research(pid, tech.as_str());
        }
    }

    /// What an arena grants one side each turn, over and above whatever its
    /// city produces. Empty on a world, which earns its yields instead.
    ///
    /// Per side rather than per city on purpose: a side set up with no city
    /// still techs and still banks upgrade money, which is what keeps the
    /// zero-city arena a real option. Both the turn and the observer read
    /// this, so the panel cannot drift from what is actually collected.
    pub fn arena_side_yields(&self, pid: usize) -> Yields {
        if !self.is_arena() {
            return Yields::default();
        }
        Yields {
            gold: f64::from(self.tactics.gold),
            science: self.arena_science(pid),
            ..Yields::default()
        }
    }

    /// Whether an arena will let a city build this item.
    ///
    /// The mode is a tactical testbed, so the only thing worth spending
    /// Production on is something that fights. `military` covers the 69 units
    /// that do; `support` covers the 9 that make them better at it — the
    /// battering ram and siege tower that get them through walls, the medic,
    /// the observation balloon. Everything else a city could queue —
    /// buildings, districts, wonders, projects, Settlers, Builders, Traders —
    /// is either meaningless on a bounded arena or would turn the battle into
    /// the empire game the mode exists to strip away.
    pub fn arena_allows_production(&self, item: &Item) -> bool {
        let unit = match item {
            Item::Unit { unit } => *unit,
            Item::Formation { unit, .. } => *unit,
            _ => return false,
        };
        self.rules.units.get_interned(unit).is_some_and(|spec| {
            matches!(spec.class.as_str(), "military" | "support")
                // And a civilization's own units only when the match asked
                // for them. Off, the two build menus are the same menu, so
                // whatever separates the two sides is play rather than
                // roster.
                && (self.tactics.unique_units || spec.unique_to.is_none())
                // At sea, only what can put to sea. A naval battlefield's
                // city stands on an islet a few tiles across, and every enemy
                // in the battle is afloat: a chariot built there has nowhere
                // to march and nothing it can reach. Measured on a diameter-8
                // ocean, an ungated side spent most of a sixty-turn battle's
                // production on land units that never left their own island —
                // which is not a handicap, since both sides do it, but it is
                // half the arena's economy going nowhere.
                && (!self.map_script.is_naval_battlefield()
                    || spec.domain.as_deref() == Some("sea"))
        })
    }

    /// Handicaps reach the major civilizations only: city-states and
    /// barbarians are not on either side of the difficulty bargain, and a
    /// seat in `handicap_exempt` has stepped out of it on purpose.
    fn takes_handicap(&self, pid: usize) -> bool {
        !self.is_handicap_exempt(pid)
            && self
                .players
                .get(pid)
                .is_some_and(|player| !player.is_minor && !player.is_barbarian)
    }

    /// Whether this seat may take a new tile with its own Culture or Gold.
    ///
    /// `CivilizationLevels` grants both to `CIVILIZATION_LEVEL_FULL_CIV`
    /// alone: `CanAnnexTilesWithCulture` and `CanAnnexTilesWithGold` are 0 for
    /// `CITY_STATE`, `FREE_CITIES` and `TRIBE`. A city-state's territory grows
    /// only through `CanAnnexTilesWithReceivedInfluence` — one tile per Envoy
    /// it receives from its Suzerain — which `do_send_envoy` and liberation
    /// already pay. Barbarian camps and Free Cities never annex at all.
    pub(crate) fn annexes_tiles_with_own_yields(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .is_some_and(|player| !player.is_minor && !player.is_barbarian)
    }

    /// Percentage added to each of this seat's city yields. Food is never
    /// handicapped — the shipped scaling covers the four spendable yields and
    /// Faith, and growth stays on the honest curve.
    pub fn handicap_yield_pct(&self, pid: usize) -> Yields {
        if !self.takes_handicap(pid) || self.is_human_seat(pid) {
            return Yields::default();
        }
        let mut pct = self.handicap_yield_spec().ai_yield_pct;
        // `GameOptions::ai_late_science_pct`: from the late mark the rival's
        // Science, bonus included, is that share of what it was.
        if let Some(keep) = self.ai_late_science_pct {
            if self.turn >= self.standard_duration(AI_LATE_SCIENCE_FROM_STANDARD) {
                pct.science = (100.0 + pct.science) * f64::from(keep) / 100.0 - 100.0;
            }
        }
        pct
    }

    /// Everything bought with a stockpiled yield scales by the game speed:
    /// production, research, and civic costs alike.
    pub fn speed_cost_mult(&self) -> f64 {
        self.speed_spec().cost_pct / 100.0
    }

    // -------------------------------------------------------------- agendas

    /// The historical agenda this civilization's leader follows, if any.
    pub fn agenda_of(&self, pid: usize) -> Option<&AgendaSpec> {
        let player = self.players.get(pid)?;
        if player.is_minor || player.is_barbarian {
            return None;
        }
        let civ = self.rules.civs.get(player.civ.as_str())?;
        self.rules.agendas.get(civ.agenda.as_ref()?)
    }

    /// Whether this leader carries a named preference trait, as the shipped
    /// leader data names them (`expansionist`, `aggressive_military`, …).
    pub fn leader_trait(&self, pid: usize, name: &str) -> bool {
        self.players
            .get(pid)
            .and_then(|player| self.rules.civs.get(player.civ.as_str()))
            .is_some_and(|civ| civ.traits.iter().any(|trait_name| trait_name == name))
    }

    /// What `observer`'s agenda makes of `subject`, from -30 (this leader
    /// holds them in contempt) to +30 (this leader approves).
    ///
    /// Agendas come in two shapes. Most weigh the subject against how the
    /// rest of the world is doing — Trajan compares your territory to
    /// everyone's, so being the smallest empire among giants reads worse than
    /// being small in a small world. The rest are inherently relational:
    /// Pericles cares about your envoys against his, and nothing about
    /// anyone else's.
    pub fn agenda_opinion(&self, observer: usize, subject: usize) -> f64 {
        self.agenda_opinion_reading(observer, subject, &mut |pid, measure| {
            self.agenda_measure(pid, measure)
        })
    }

    /// `agenda_opinion` with the comparative measures supplied by `read`, so
    /// that a caller scoring every pair need work each one out only once.
    fn agenda_opinion_reading(
        &self,
        observer: usize,
        subject: usize,
        read: &mut dyn FnMut(usize, &str) -> f64,
    ) -> f64 {
        if observer == subject {
            return 0.0;
        }
        let Some(agenda) = self.agenda_of(observer) else {
            return 0.0;
        };
        if self
            .players
            .get(subject)
            .is_none_or(|player| player.is_minor || player.is_barbarian || !player.alive)
        {
            return 0.0;
        }
        let direction = if agenda.approves_of == "less" {
            -1.0
        } else {
            1.0
        };
        let raw = match agenda.measure.as_str() {
            // Relational measures already carry their own sign and scale.
            "city_state_rivalry" => self.city_state_rivalry(observer, subject),
            "loyalty_to_friends" => self.loyalty_to_friends(observer, subject),
            "shared_luxuries" => self.shared_luxuries(observer, subject),
            "trustworthiness" => self.trustworthiness(observer, subject),
            // Comparative measures are scored against the rest of the world.
            other => {
                let value = read(subject, other);
                let peers: Vec<f64> = self
                    .players
                    .iter()
                    .filter(|player| {
                        player.alive
                            && !player.is_minor
                            && !player.is_barbarian
                            && player.id != subject
                    })
                    .map(|player| read(player.id, other))
                    .collect();
                if peers.is_empty() {
                    return 0.0;
                }
                let reference = peers.iter().sum::<f64>() / peers.len() as f64;
                (value - reference) / reference.max(1.0)
            }
        };
        (direction * raw * 30.0).clamp(-30.0, 30.0)
    }

    /// The comparative measures, each a plain count for one civilization.
    fn agenda_measure(&self, pid: usize, measure: &str) -> f64 {
        match measure {
            "territory" => self
                .player_city_ids(pid)
                .iter()
                .map(|cid| self.cities[cid].owned_tiles.len() as f64)
                .sum(),
            "military" => self.military_power(pid),
            "wonders" => self
                .player_city_ids(pid)
                .iter()
                .map(|cid| self.cities[cid].wonders.len() as f64)
                .sum(),
            "districts_per_city" => {
                let cities = self.player_city_ids(pid);
                if cities.is_empty() {
                    return 0.0;
                }
                cities
                    .iter()
                    .map(|cid| self.cities[cid].districts.len() as f64)
                    .sum::<f64>()
                    / cities.len() as f64
            }
            _ => 0.0,
        }
    }

    /// Pericles: how directly this civilization contests the city-states he
    /// courts. Only city-states he has actually invested in count.
    fn city_state_rivalry(&self, observer: usize, subject: usize) -> f64 {
        let minors: Vec<usize> = self
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian && player.alive)
            .map(|player| player.id)
            .collect();
        let mut contested: f64 = 0.0;
        let mut courted: f64 = 0.0;
        for minor in minors {
            if self.envoys_at(observer, minor) <= 0 {
                continue;
            }
            courted += 1.0;
            if self.envoys_at(subject, minor) > 0 {
                contested += 1.0;
            }
            if self.suzerain_of(minor) == Some(subject) {
                contested += 1.0;
            }
        }
        if courted == 0.0 {
            return 0.0;
        }
        (contested / courted).min(1.0)
    }

    /// Tomyris: friendship counts for a great deal, and a reputation for
    /// treachery — grievances held against them by anyone at all — counts
    /// heavily against.
    fn trustworthiness(&self, observer: usize, subject: usize) -> f64 {
        let mut score: f64 = 0.0;
        if self.are_friends(observer, subject) || self.are_allied(observer, subject) {
            score += 1.0;
        }
        let held_against_them: f64 = self
            .players
            .iter()
            .filter(|player| !player.is_barbarian && player.id != subject)
            .map(|player| player.grievances.get(&subject).copied().unwrap_or(0.0))
            .sum();
        score -= (held_against_them / 200.0).min(2.0);
        score.clamp(-1.0, 1.0)
    }

    fn relational_agenda_measure(measure: &str) -> bool {
        matches!(
            measure,
            "city_state_rivalry" | "loyalty_to_friends" | "shared_luxuries" | "trustworthiness"
        )
    }

    /// Announce agenda stances that have changed since last turn. A leader
    /// says nothing while merely indifferent, which keeps the log to the
    /// moments a relationship actually turned.
    fn process_agendas(&mut self) {
        const THRESHOLD: f64 = 15.0;
        let majors: Vec<usize> = self
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        // Comparative agendas read the same empire metric for every
        // observer/subject pair. Snapshot each distinct metric once, then
        // reproduce the original peer-order sum for each possible subject.
        // Without this, 100 civilizations turn four cheap empire scans into
        // roughly a million full-world scans at every round boundary.
        let comparative_measures: BTreeSet<String> = majors
            .iter()
            .filter_map(|observer| self.agenda_of(*observer))
            .map(|agenda| agenda.measure.clone())
            .filter(|measure| !Self::relational_agenda_measure(measure))
            .collect();
        let mut comparative_raw: BTreeMap<(String, usize), f64> = BTreeMap::new();
        for measure in comparative_measures {
            let values: Vec<(usize, f64)> = majors
                .iter()
                .map(|subject| (*subject, self.agenda_measure(*subject, &measure)))
                .collect();
            for (subject, value) in &values {
                let peers: Vec<f64> = values
                    .iter()
                    .filter(|(other, _)| other != subject)
                    .map(|(_, value)| *value)
                    .collect();
                if peers.is_empty() {
                    continue;
                }
                let reference = peers.iter().sum::<f64>() / peers.len() as f64;
                comparative_raw.insert(
                    (measure.clone(), *subject),
                    (*value - reference) / reference.max(1.0),
                );
            }
        }

        for observer in majors.iter().copied() {
            let Some(agenda) = self.agenda_of(observer) else {
                continue;
            };
            let agenda_name = agenda.name.clone();
            let measure = agenda.measure.clone();
            let direction = if agenda.approves_of == "less" {
                -1.0
            } else {
                1.0
            };
            for subject in majors.iter().copied() {
                if subject == observer {
                    continue;
                }
                let raw = match measure.as_str() {
                    "city_state_rivalry" => self.city_state_rivalry(observer, subject),
                    "loyalty_to_friends" => self.loyalty_to_friends(observer, subject),
                    "shared_luxuries" => self.shared_luxuries(observer, subject),
                    "trustworthiness" => self.trustworthiness(observer, subject),
                    _ => comparative_raw
                        .get(&(measure.clone(), subject))
                        .copied()
                        .unwrap_or(0.0),
                };
                let opinion = (direction * raw * 30.0).clamp(-30.0, 30.0);
                let stance = if opinion >= THRESHOLD {
                    1_i8
                } else if opinion <= -THRESHOLD {
                    -1
                } else {
                    0
                };
                let previous = self.players[observer]
                    .agenda_view
                    .get(&subject)
                    .copied()
                    .unwrap_or(0);
                if stance == previous {
                    continue;
                }
                self.players[observer].agenda_view.insert(subject, stance);
                let observer_civ = self.civ_name(observer);
                let subject_civ = self.civ_name(subject);
                let message = match stance {
                    1 => format!("{observer_civ} approves of {subject_civ} ({agenda_name})"),
                    -1 => format!("{observer_civ} disapproves of {subject_civ} ({agenda_name})"),
                    _ => format!("{observer_civ}'s view of {subject_civ} has cooled"),
                };
                self.note(subject, "Diplomacy", message.clone(), None);
                self.note(observer, "Diplomacy", message, None);
            }
        }
    }

    // --------------------------------------------------------------- events

    /// Record something that happened to `pid`. Silently ignores minor and
    /// barbarian seats, which nobody observes.
    pub(crate) fn note(
        &mut self,
        pid: usize,
        category: &str,
        text: impl Into<String>,
        pos: Option<Pos>,
    ) {
        debug_assert!(
            EVENT_CATEGORIES.contains(&category),
            "{category} is not one of the event categories the browser's log filter groups"
        );
        if self
            .players
            .get(pid)
            .is_none_or(|player| player.is_minor || player.is_barbarian)
        {
            return;
        }
        let text = self.attributed(pid, text.into());
        // The chronicle dates an entry by the turn the world is reported on,
        // which is the turn being played for all but the victory a score
        // tiebreak declares on the wrap out of the final turn.
        let turn = self.reported_turn();
        self.events.push(Event {
            turn,
            player: pid,
            category: category.to_string(),
            text,
            pos,
            important: false,
        });
        if self.events.len() > EVENT_LIMIT {
            let excess = self.events.len() - EVENT_LIMIT;
            self.events.drain(..excess);
        }
    }

    /// Record something that must not be missed.
    ///
    /// The log is a stream, and a stream at Lightning pace scrolls a routine
    /// notice past a reader in well under a second. Category alone cannot say
    /// which entries deserve to be held: a nuclear detonation and a unit
    /// upgrade are both `War`. This marks the handful that are, and every
    /// surface reading the log — a browser panel, an agent's observation —
    /// gets the same answer about which those are.
    pub(crate) fn note_important(
        &mut self,
        pid: usize,
        category: &str,
        text: impl Into<String>,
        pos: Option<Pos>,
    ) {
        // `note` drops minor and barbarian seats, so marking the last entry
        // blind would promote whatever unrelated event happened to precede it.
        let before = self.events.len();
        self.note(pid, category, text, pos);
        if self.events.len() != before {
            if let Some(event) = self.events.last_mut() {
                event.important = true;
            }
        }
    }

    /// A log entry names its civilization first. Read on its own — in a
    /// spectator's mixed stream, or by an agent reasoning over its tail — an
    /// entry that opens with a verb is an event without an author, and the
    /// spectate view rotates the observed seat between frames, so "Founded
    /// Antium" belongs to nobody in particular. Messages a call site already
    /// wrote from the actor's side (war, peace, diplomacy, a captured city)
    /// name a civilization themselves and reach both parties unchanged.
    fn attributed(&self, pid: usize, text: String) -> String {
        if self
            .players
            .iter()
            .any(|player| !player.civ.is_empty() && text.starts_with(&player.civ))
        {
            return text;
        }
        format!("{} {}", self.civ_name(pid), text)
    }

    fn civ_name(&self, pid: usize) -> String {
        self.players
            .get(pid)
            .map(|player| player.civ.clone())
            .unwrap_or_else(|| "Someone".to_string())
    }

    /// The events one civilization can see, oldest first.
    pub fn events_for(&self, pid: usize) -> Vec<&Event> {
        self.events.iter().filter(|e| e.player == pid).collect()
    }

    pub fn city_at(&self, pos: Pos) -> Option<u32> {
        self.city_by_pos.get(&pos).copied()
    }

    #[inline]
    pub fn unit_ids_at(&self, pos: Pos) -> &[u32] {
        const EMPTY: &[u32] = &[];
        self.occ.get(&pos).map_or(EMPTY, Vec::as_slice)
    }

    /// Prefer [`unit_ids_at`](Self::unit_ids_at) at a read-only call site — it
    /// borrows the occupancy list instead of copying it into a fresh `Vec`.
    pub fn units_at(&self, pos: Pos) -> Vec<u32> {
        self.unit_ids_at(pos).to_vec()
    }

    /// This player's unit ids in stable ascending order.
    ///
    /// The roster is a `BTreeMap` keyed by id.  Keep that order through the
    /// read memo too: callers use the first few qualifying units as a
    /// deterministic bounded roster.
    pub fn player_unit_ids(&self, pid: usize) -> Vec<u32> {
        if let Some(ids) = self
            .query_memo
            .unit_ids
            .borrow()
            .as_ref()
            .and_then(|memo| memo.get(&pid))
        {
            return ids.as_ref().clone();
        }
        let ids = Arc::new(
            self.units
                .values()
                .filter(|u| u.owner == pid)
                .map(|u| u.id)
                .collect::<Vec<u32>>(),
        );
        let owned = ids.as_ref().clone();
        if let Some(memo) = self.query_memo.unit_ids.borrow_mut().as_mut() {
            memo.insert(pid, ids.clone());
        }
        owned
    }

    fn unit_territory_access(&self, unit: &Unit) -> Arc<Vec<bool>> {
        if let Some(access) = self
            .query_memo
            .unit_territory_access
            .borrow()
            .as_ref()
            .and_then(|memo| memo.get(&unit.id))
        {
            return access.clone();
        }
        let access: Arc<Vec<bool>> = Arc::new(
            (0..self.players.len())
                .map(|owner| self.unit_has_territory_access(unit, owner))
                .collect(),
        );
        if let Some(memo) = self.query_memo.unit_territory_access.borrow_mut().as_mut() {
            memo.insert(unit.id, access.clone());
        }
        access
    }

    pub fn player_city_ids(&self, pid: usize) -> Vec<u32> {
        if let Some(ids) = self
            .query_memo
            .city_ids
            .borrow()
            .as_ref()
            .and_then(|memo| memo.get(&pid))
        {
            return ids.clone();
        }
        let ids: Vec<u32> = self
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .map(|c| c.id)
            .collect();
        if let Some(memo) = self.query_memo.city_ids.borrow_mut().as_mut() {
            memo.insert(pid, ids.clone());
        }
        ids
    }

    /// Whether `viewer` has made contact with `other` and may therefore be
    /// told anything at all about them.
    ///
    /// Civilization VI keeps an unmet empire off every panel it would
    /// otherwise appear on — the diplomacy ribbon, the victory tracker, the
    /// score list — and this is the one question all of those ask. A seat
    /// always knows itself and its pre-game team; barbarians and Free Cities
    /// are not diplomatic powers and are never withheld.
    pub fn has_met(&self, viewer: usize, other: usize) -> bool {
        if viewer == other || self.same_team(viewer, other) {
            return true;
        }
        match self.players.get(other) {
            None => true,
            Some(player) if player.is_barbarian || player.is_free_city => true,
            Some(_) => self
                .players
                .get(viewer)
                .is_some_and(|player| player.met.contains(&other)),
        }
    }

    /// Pre-game teams are permanent. `None` means an ordinary free-for-all
    /// seat, not an implicit one-player team.
    pub fn same_team(&self, first: usize, second: usize) -> bool {
        first != second
            && self
                .players
                .get(first)
                .and_then(|player| player.team)
                .is_some_and(|team| {
                    self.players
                        .get(second)
                        .is_some_and(|player| player.team == Some(team))
                })
    }

    /// Every major seat which shares `pid`'s explicit team. Free-for-all
    /// players form a singleton for calculations that aggregate by side.
    pub fn team_members(&self, pid: usize) -> Vec<usize> {
        let Some(player) = self.players.get(pid) else {
            return Vec::new();
        };
        let Some(team) = player.team else {
            return vec![pid];
        };
        self.players
            .iter()
            .filter(|member| !member.is_minor && !member.is_barbarian && member.team == Some(team))
            .map(|member| member.id)
            .collect()
    }

    /// All seats credited with the terminal result. The triggering
    /// civilization remains `winner` for save compatibility and narration.
    pub fn winning_players(&self) -> Vec<usize> {
        self.winner
            .map(|winner| self.team_members(winner))
            .unwrap_or_default()
    }

    pub fn is_at_war(&self, a: usize, b: usize) -> bool {
        if a == b || self.same_team(a, b) {
            return false;
        }
        // Two armies are dropped onto an arena to fight each other. There is
        // nothing else on a battlefield to do and nothing to negotiate over —
        // no borders, no cities, no trade — so a Tactics game is at war from
        // its first turn and stays there. Nobody has to declare it and nobody
        // can sign it away, which is why this is the war state itself rather
        // than a declaration written into `at_war` at setup.
        if self.is_arena() {
            return true;
        }
        // The same is true of a world played on to its last civilization:
        // the war is the rule of the extension, so it cannot be signed away.
        if self.plays_to_the_last_civ() && self.major_diplomatic_counterpart(a, b) {
            return true;
        }
        if self
            .players
            .get(a)
            .is_some_and(|player| player.is_free_city)
            || self
                .players
                .get(b)
                .is_some_and(|player| player.is_free_city)
        {
            return true;
        }
        if let Some(bp) = self.barb_pid {
            if bp == a || bp == b {
                return true;
            }
        }
        if self.at_war.contains(&pair(a, b)) {
            return true;
        }
        // What follows can only find a war that one of these two was dragged
        // into by a patron, so it needs a city-state to drag and a war to
        // drag it into. Deciding a city-state's patron is expensive, and this
        // is asked tens of millions of times in a long game.
        let is_client = |player: usize| {
            self.players
                .get(player)
                .is_some_and(|seat| seat.is_minor && !seat.is_barbarian)
        };
        if self.at_war.is_empty() || (!is_client(a) && !is_client(b)) {
            return false;
        }
        // A city-state follows its Suzerain into wars and peace. Keep the
        // explicit relation set between the declaring civilizations, then
        // derive city-state participation so it changes immediately when
        // suzerainty changes without leaving stale war pairs behind.
        let controller = |player: usize| {
            self.players
                .get(player)
                .filter(|candidate| candidate.is_minor && !candidate.is_barbarian)
                .and_then(|_| self.suzerain_of(player))
                .unwrap_or(player)
        };
        let a_controller = controller(a);
        let b_controller = controller(b);
        a_controller != b_controller
            && (a_controller != a || b_controller != b)
            && self.at_war.contains(&pair(a_controller, b_controller))
    }

    /// Whether two empires are at war: the engine's own relation, or — on a
    /// mirrored seat, whose `at_war` carries only our own wars — the host's
    /// report of a war between two met rivals ([`Game::observed_rival_wars`]).
    pub fn rivals_at_war(&self, a: usize, b: usize) -> bool {
        self.is_at_war(a, b) || (a != b && self.observed_rival_wars.contains(&(a.min(b), a.max(b))))
    }

    /// Is this tile contaminated right now?
    pub fn fallout_at(&self, pos: Pos) -> bool {
        self.map
            .get(pos)
            .is_some_and(|tile| tile.fallout_until > self.turn)
    }

    /// Stock Gold price of a plot whose ring base is `base` (50 through ring
    /// two, 75 in ring three). Civ VI uses the farther-advanced research tree,
    /// truncates it to a whole percent, applies its 1x-5x progression, scales
    /// for game speed, rounds down to 5 Gold, and only then applies Land
    /// Surveyors/Expropriation. That order is observable on Marathon, where a
    /// 20% discount leaves prices divisible by four rather than five.
    pub fn tile_purchase_cost(&self, pid: usize, base: f64) -> f64 {
        let progressed = base * (1.0 + 4.0 * self.game_progress_ratio(pid));
        let speed_scaled = self.game_speed.scale(progressed);
        let rounded = (speed_scaled / 5.0 + 1e-9).floor() * 5.0;
        let discount = self
            .policy_effect(pid, "tile_purchase_discount_pct")
            .clamp(0.0, 100.0);
        rounded * (1.0 - discount / 100.0)
    }

    /// The observed native price for annexing `pos` to `cid`, or the ordinary
    /// connected, three-ring model price when no native menu was supplied.
    /// A present native menu authorizes only its quoted plots. Ownership and
    /// exploration checks apply in both cases.
    pub fn plot_purchase_cost(&self, pid: usize, cid: u32, pos: Pos) -> Option<f64> {
        let city = self.cities.get(&cid)?;
        let tile = self.map.get(pos)?;
        if city.owner != pid
            // `CanAnnexTilesWithGold` is a full-civilization privilege.
            || !self.annexes_tiles_with_own_yields(pid)
            || tile.owner_city.is_some()
            || !self.players.get(pid)?.explored.contains(&pos)
        {
            return None;
        }
        if let Some(quotes) = self.host_plot_purchases.get(&cid) {
            let mut matches = quotes.iter().filter(|quote| quote.pos == pos);
            let quote = matches.next()?;
            return (matches.next().is_none() && quote.gold.is_finite() && quote.gold >= 0.0)
                .then_some(quote.gold);
        }
        if !self
            .nbrs(pos)
            .into_iter()
            .any(|neighbor| self.map.tiles[&neighbor].owner_city == Some(cid))
        {
            return None;
        }
        let base = match self.wdist(city.pos, pos) {
            // Ring one is normally granted on foundation. It can become
            // neutral after a neighboring city is razed, and then uses the
            // same base price as ring two.
            1 | 2 => 50.0,
            3 => 75.0,
            _ => return None,
        };
        Some(self.tile_purchase_cost(pid, base))
    }

    fn add_war_weariness(&mut self, pid: usize, points: f64) {
        self.players[pid].war_weariness = (self.players[pid].war_weariness + points).max(0.0);
    }

    /// `WAR_WEARINESS_DECAY_TURN_AT_WAR` 50 while any war is running, and
    /// `_AT_PEACE` 200 once they are all over.
    fn decay_war_weariness(&mut self, pid: usize) {
        let decay = if self.at_war_with_any_civilization(pid) {
            50.0
        } else {
            200.0
        };
        self.add_war_weariness(pid, -decay);
    }

    pub fn war_weariness_multiplier(&self, pid: usize, home_territory: bool) -> f64 {
        let reduction = self.policy_effect(pid, "war_weariness_reduction_pct")
            + self.gov_effects(pid).war_weariness_reduction_pct
            + if home_territory {
                self.policy_effect(pid, "home_war_weariness_reduction_pct")
            } else {
                0.0
            };
        (1.0 - reduction / 100.0).max(0.0)
    }

    /// Sum global abilities from every researched technology and civic.
    pub fn tree_effect(&self, pid: usize, effect: &str) -> f64 {
        let player = &self.players[pid];
        let sources = self.rules.tree_effects.get(effect);
        let researched = sources
            .into_iter()
            .flat_map(|sources| sources.techs.iter())
            .filter(|(node, _)| player.techs.contains(node));
        let adopted = sources
            .into_iter()
            .flat_map(|sources| sources.civics.iter())
            .filter(|(node, _)| player.civics.contains(node));
        // Node lists are in tree order, so this adds the same values in the
        // same sequence as walking the player's own trees did.
        researched.chain(adopted).map(|(_, value)| *value).sum()
    }

    fn city_has_established_governor(&self, pid: usize, cid: u32) -> bool {
        self.players[pid]
            .governor_roster
            .keys()
            .any(|governor| self.established_governor_city(pid, governor) == Some(cid))
    }

    pub(crate) fn process_spies(&mut self, pid: usize) {
        let ids: Vec<u32> = self
            .spies
            .values()
            .filter(|spy| spy.owner == pid)
            .map(|spy| spy.id)
            .collect();
        for spy_id in ids {
            let Some(spy) = self.spies.get(&spy_id).cloned() else {
                continue;
            };
            if spy.sources_until <= self.turn {
                let spy = self.spies.get_mut(&spy_id).unwrap();
                spy.sources_city = None;
                spy.sources_until = 0;
            }
            if spy.captured_by.is_none()
                && spy
                    .mission
                    .as_ref()
                    .is_some_and(|mission| mission.ends <= self.turn)
            {
                self.resolve_spy_mission(spy_id);
            }
        }
    }

    /// Whether this seat is one of the official Civilization VI identities
    /// entitled to the ruleset's unique content.  Historical and Today roster
    /// entries are deliberately neutral even where an older rules row happens
    /// to share their civilization name.
    pub fn uses_civ6_content(&self, pid: usize) -> bool {
        self.players.get(pid).is_some_and(|player| {
            !player.is_minor
                && !player.is_barbarian
                && leader_roster::uses_civ6_mechanics(&player.civ)
        })
    }

    /// Whether this player may use content marked unique to `civilization`.
    #[inline]
    pub fn owns_civ_unique(&self, pid: usize, civilization: &str) -> bool {
        self.uses_civ6_content(pid) && self.players[pid].civ == civilization
    }

    /// Leader/civ ability check (data in civs.json, effects keyed by name).
    pub fn has_ability(&self, pid: usize, ability: &str) -> bool {
        (self.uses_civ6_content(pid)
            && self
                .rules
                .civs
                .get(&self.players[pid].civ)
                .is_some_and(|c| c.ability == ability))
            || self.modifier_grants_ability(pid, ability)
    }

    /// What this player's signature ability is worth under a named modifier,
    /// for the abilities whose whole effect is a number the engine already
    /// applies from somewhere else. Zero for a civilization that declares no
    /// such effect, and for city-states and barbarians, which are not seated
    /// from the civilization roster at all.
    ///
    /// See `CivSpec::effects` for the vocabulary. Abilities the engine cannot
    /// express as a modifier stay keyed by name in [`Game::has_ability`].
    pub fn civ_effect(&self, pid: usize, effect: &str) -> f64 {
        if !self.uses_civ6_content(pid) {
            return 0.0;
        }
        self.players
            .get(pid)
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .and_then(|player| self.rules.civs.get(player.civ.as_str()))
            .and_then(|civ| civ.effects.get(effect))
            .copied()
            .unwrap_or(0.0)
    }

    /// A city that has just grown gains 50 Pressure for its majority religion,
    /// or 50 atheist/pantheon Pressure when it follows none.
    fn apply_growth_pressure(&mut self, cid: u32) {
        let majority = self.city_religion(&self.cities[&cid]).map(str::to_string);
        let city = self.cities.get_mut(&cid).unwrap();
        match majority {
            Some(religion) => *city.pressure.entry(religion).or_insert(0.0) += 50.0,
            None => city.atheist_pressure += 50.0,
        }
    }

    fn do_choose_secret_society(&mut self, pid: usize, society: &str) -> Result<(), String> {
        if !self.game_mode("secret_societies")
            || self.players[pid].is_minor
            || self.players[pid].is_barbarian
            || self.players[pid].secret_society.is_some()
            || !self.players[pid]
                .civics
                .contains(&crate::name!("code_of_laws"))
            || !matches!(
                society,
                "hermetic_order" | "owls_of_minerva" | "voidsingers"
            )
        {
            return Err("secret society cannot be chosen".into());
        }
        self.players[pid].secret_society = Some(society.to_string());
        Ok(())
    }

    fn do_spread(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        let spread = self.rules.units[u.kind].religious_spread;
        if spread <= 0.0 || u.charges <= 0 || u.moves_left <= 0.0 {
            return Err("religious unit has no spread charges".into());
        }
        let kind = u.kind;
        let religion = u
            .religion
            .clone()
            .ok_or_else(|| "unit has no religion to spread".to_string())?;
        let cid = self
            .city_at(u.pos)
            .or_else(|| self.nbrs(u.pos).into_iter().find_map(|n| self.city_at(n)))
            .ok_or_else(|| "no city in range".to_string())?;
        let previous_religion = self.city_religion(&self.cities[&cid]).map(str::to_string);
        let target_owner = self.cities[&cid].owner;
        let foreign = target_owner != pid;
        let pressure_ignored = self.city_ignores_foreign_religion(&self.cities[&cid], &religion);
        let mut eviction: f64 = match kind.as_str() {
            "apostle" => 0.25,
            "missionary" => 0.10,
            _ => 0.0,
        };
        eviction = eviction.max(self.promotion_effect(&u, "rival_pressure_removed_pct") / 100.0);
        let foreign_multiplier = if foreign {
            1.0 + self.promotion_effect(&u, "foreign_spread_pct") / 100.0
        } else {
            1.0
        };
        // Host majority/warning markers are not accumulated pressure. The
        // next snapshot can report a conversion; arithmetic on 100/60/50
        // markers cannot predict how many native charges it takes.
        if !pressure_ignored && !self.observed_city_religion_markers.contains(&cid) {
            let city = self.cities.get_mut(&cid).unwrap();
            for (faith, pressure) in city.pressure.iter_mut() {
                if *faith != religion {
                    *pressure *= 1.0 - eviction;
                }
            }
            *city.pressure.entry(religion.clone()).or_insert(0.0) +=
                spread * foreign_multiplier * u.hp.clamp(0, 100) as f64 / 100.0;
        }
        let converted = previous_religion.as_deref() != Some(religion.as_str())
            && self.city_religion(&self.cities[&cid]) == Some(religion.as_str());
        if converted {
            self.players[pid].gold += self.promotion_effect(&u, "gold_per_conversion");
            self.break_promises_on_conversion(pid, target_owner);
            if self.grants_city_state_unique_bonus(pid, "Fez")
                && self.players[pid].religion.as_deref() == Some(religion.as_str())
            {
                let key = format!("fez_first_conversion:{cid}");
                if self.players[pid].counters.insert(key, 1).is_none() {
                    let population = self.cities[&cid].pop as f64;
                    self.players[pid].research_overflow += self.game_speed.scale(20.0 * population);
                }
            }
        }
        let mu = self.units.get_mut(&uid).unwrap();
        mu.charges -= 1;
        mu.moves_left = 0.0;
        mu.acted = true;
        if self.units[&uid].charges <= 0 {
            self.remove_unit(uid);
        }
        self.check_religious_victory();
        Ok(())
    }

    fn do_condemn_heretic(&mut self, pid: usize, uid: u32, target_id: u32) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let target = self
            .units
            .get(&target_id)
            .cloned()
            .ok_or_else(|| "no such religious unit".to_string())?;
        let congress_condemnation = target
            .religion
            .as_deref()
            .is_some_and(|religion| self.congress_effect_active("world_religion", "B", religion));
        if self.rules.units[unit.kind].class != "military"
            || self.rules.units[target.kind].class != "religious"
            || (!self.is_at_war(pid, target.owner) && !congress_condemnation)
            || unit.pos != target.pos
            || unit.moves_left <= 0.0
        {
            return Err("cannot condemn that unit".into());
        }
        let religion = target
            .religion
            .clone()
            .ok_or_else(|| "target has no religion".to_string())?;
        self.remove_unit(target_id);
        // The loss side of the raid ledger. A condemned Missionary is removed
        // here rather than through `record_kill` or `resolve_entered_units`,
        // so until 2026-08-24 the barbarians could wipe a whole religious
        // corps and no counter said so. Free Cities also carry
        // `is_barbarian`; only the true barbarian seat's blows count.
        if self.players[pid].is_barbarian && !self.players[pid].is_free_city {
            bump(
                &mut self.players[target.owner],
                "religious_lost_to_barbarians",
            );
        }
        bump(
            &mut self.players[pid],
            &format!("condemned:{}", target.kind),
        );
        self.religious_combat_pressure(None, &religion, target.pos, 6, 125.0);
        if self.congress_effect_active("world_religion", "B", &religion) {
            self.players[pid].diplomatic_favor += 25.0;
        }
        let unit = self.units.get_mut(&uid).unwrap();
        unit.moves_left = 0.0;
        unit.acted = true;
        self.check_religious_victory();
        Ok(())
    }

    fn do_remove_heresy(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let inquisitor = self.own_unit(pid, uid)?;
        if inquisitor.kind != "inquisitor"
            || inquisitor.charges <= 0
            || inquisitor.moves_left <= 0.0
        {
            return Err("not an Inquisitor with charges".into());
        }
        let religion = inquisitor
            .religion
            .clone()
            .ok_or_else(|| "Inquisitor has no religion".to_string())?;
        let cid = self
            .city_at(inquisitor.pos)
            .filter(|cid| self.cities[cid].owner == pid)
            .ok_or_else(|| "Remove Heresy requires a friendly City Center".to_string())?;
        for (name, pressure) in self.cities.get_mut(&cid).unwrap().pressure.iter_mut() {
            if *name != religion {
                *pressure *= 0.25;
            }
        }
        let unit = self.units.get_mut(&uid).unwrap();
        unit.charges -= 1;
        unit.moves_left = 0.0;
        unit.acted = true;
        if unit.charges <= 0 {
            self.remove_unit(uid);
        }
        self.check_religious_victory();
        Ok(())
    }

    fn do_launch_inquisition(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let apostle = self.own_unit(pid, uid)?;
        if apostle.kind != "apostle"
            || apostle.moves_left <= 0.0
            || apostle.religion != self.players[pid].religion
            || self.players[pid]
                .counters
                .get("inquisition")
                .copied()
                .unwrap_or(0)
                > 0
        {
            return Err("cannot launch an inquisition".into());
        }
        let holy = self.players[pid]
            .holy_city
            .ok_or_else(|| "religion has no holy city".to_string())?;
        let holy_pos = self
            .cities
            .get(&holy)
            .map(|city| city.pos)
            .ok_or_else(|| "holy city is missing".to_string())?;
        if self.wdist(apostle.pos, holy_pos) > 1 {
            return Err("Apostle must be at its Holy Site".into());
        }
        self.remove_unit(uid);
        bump(&mut self.players[pid], "inquisition");
        self.first_historic_moment(
            pid,
            "inquisition_launched",
            Some("MOMENT_INQUISITION_LAUNCHED"),
            Some("MOMENT_INQUISITION_LAUNCHED_FIRST_IN_WORLD"),
        );
        Ok(())
    }

    /// Passive spread: cities pressure other cities within ten tiles, while
    /// active Trade Routes exchange additional pressure per turn — 1 to the
    /// route's destination and 0.5 back to its origin.
    fn process_pressure(&mut self, pid: usize) {
        // Exodus of the Evangelists pays for the first time each city takes
        // the founder's religion, so the majorities are read before pressure
        // moves and compared afterwards.
        let before: Vec<(u32, Option<String>)> = self
            .cities
            .values()
            .map(|city| (city.id, self.city_religion(city).map(str::to_string)))
            .collect();
        let sources: Vec<(Pos, String, f64, i32)> = self
            .cities
            .values()
            .filter_map(|c| {
                self.city_religion(c).map(|r| {
                    // `RELIGION_SPREAD_HOLY_SITE_PRESSURE_MULTIPLIER` 2 and
                    // `RELIGION_SPREAD_HOLY_CITY_PRESSURE_MULTIPLIER` 4. Jerusalem's
                    // Suzerain has every city with a Holy Site count as a Holy
                    // City (`TREAT_HOLY_SITES_AS_HOLY_CITIES`).
                    let mut boost = 1.0;
                    let holy_site =
                        self.city_has_active_district_family(c, crate::name!("holy_site"));
                    if holy_site {
                        boost *= 2.0;
                    }
                    let holy_city = self
                        .religion_founder(r)
                        .and_then(|founder| self.players[founder].holy_city)
                        == Some(c.id);
                    if holy_city
                        || (holy_site && self.grants_city_state_unique_bonus(c.owner, "Jerusalem"))
                    {
                        boost *= 4.0;
                    }
                    let strength = boost
                        * (1.0 + self.religion_belief_effect(r, "pressure_strength_pct") / 100.0)
                        * (1.0
                            + self.governor_effect(c.owner, c.id, "religious_pressure_pct")
                                / 100.0);
                    let range = (10.0
                        * (1.0 + self.religion_belief_effect(r, "pressure_range_pct") / 100.0))
                        .floor() as i32;
                    (c.pos, r.to_string(), strength, range)
                })
            })
            .collect();
        let targets: Vec<u32> = self.player_city_ids(pid);
        for cid in targets {
            let cpos = self.cities[&cid].pos;
            for (spos, r, amt, range) in &sources {
                if *spos != cpos
                    && self.wdist(*spos, cpos) <= *range
                    && !self.city_ignores_foreign_religion(&self.cities[&cid], r)
                    && !self.religious_alliance_blocks_pressure(&self.cities[&cid], r)
                {
                    *self
                        .cities
                        .get_mut(&cid)
                        .unwrap()
                        .pressure
                        .entry(r.clone())
                        .or_insert(0.0) += amt;
                }
            }
            let route_religions: Vec<(String, f64)> = self
                .routes
                .iter()
                .filter_map(|route| {
                    // A route is directional: the shipped
                    // RELIGION_SPREAD_TRADE_ROUTE_PRESSURE_FOR_DESTINATION is
                    // 1.0 and _FOR_ORIGIN is 0.5, so the city the trader
                    // travels *to* takes twice the pressure the origin does.
                    let (other, share) = if route.origin == cid {
                        (route.dest, 0.5)
                    } else if route.dest == cid {
                        (route.origin, 1.0)
                    } else {
                        return None;
                    };
                    self.cities.get(&other).and_then(|city| {
                        self.city_religion(city).map(|religion| {
                            (
                                religion.to_string(),
                                share
                                    * (1.0
                                        + self.governor_effect(
                                            city.owner,
                                            city.id,
                                            "religious_pressure_pct",
                                        ) / 100.0),
                            )
                        })
                    })
                })
                .collect();
            for (religion, amount) in route_religions {
                if self.city_ignores_foreign_religion(&self.cities[&cid], &religion) {
                    continue;
                }
                if self.religious_alliance_blocks_pressure(&self.cities[&cid], &religion) {
                    continue;
                }
                *self
                    .cities
                    .get_mut(&cid)
                    .unwrap()
                    .pressure
                    .entry(religion)
                    .or_insert(0.0) += amount;
            }
        }
        self.award_conversion_era_score(&before);
        self.check_religious_victory();
    }

    /// Religious victory: "make your Religion the predominant Religion for
    /// every other major civilization in the game" — a majority of each
    /// rival's cities. The founder's own cities are not part of the test, and
    /// neither are city-states.
    fn check_religious_victory(&mut self) {
        self.award_religious_unity_envoys();
        // The sweep below can only ever crown a religious winner, and
        // `set_winner` refuses that crown whenever the lobby has Religious
        // Victory switched off — so with the condition disabled the whole
        // O(majors × rivals × cities) scan is a no-op that was still paid once
        // per player turn. The envoy award above is an ordinary envoy
        // mechanic, not a victory check, and keeps running either way.
        if !self.victory_conditions.religious {
            return;
        }
        if self.is_finished() {
            return;
        }
        for p in 0..self.players.len() {
            if !self.victory_eligible(p) {
                continue;
            }
            if self.players[p].team.is_some() {
                let team_religions: BTreeSet<String> = self
                    .team_members(p)
                    .into_iter()
                    .filter_map(|member| self.players[member].religion.clone())
                    .collect();
                if team_religions.is_empty() {
                    continue;
                }
                let opponents: Vec<&Player> = self
                    .players
                    .iter()
                    .filter(|other| {
                        other.alive
                            && !other.is_minor
                            && !other.is_barbarian
                            && other.id != p
                            && !self.same_team(p, other.id)
                    })
                    .collect();
                let all_opponents_converted = !opponents.is_empty()
                    && opponents.iter().all(|other| {
                        let cities: Vec<&City> = self
                            .cities
                            .values()
                            .filter(|city| city.owner == other.id)
                            .collect();
                        !cities.is_empty()
                            && team_religions.iter().any(|religion| {
                                cities
                                    .iter()
                                    .filter(|city| {
                                        self.city_religion(city) == Some(religion.as_str())
                                    })
                                    .count()
                                    * 2
                                    > cities.len()
                            })
                    });
                if all_opponents_converted && self.set_winner(p, "religious") {
                    return;
                }
                continue;
            }
            let religion = match &self.players[p].religion {
                Some(r) => r.clone(),
                None => continue,
            };
            let rivals: Vec<usize> = self
                .players
                .iter()
                .filter(|o| o.alive && !o.is_minor && !o.is_barbarian && o.id != p)
                .map(|o| o.id)
                .collect();
            // Nobody left to convert is a conquest, not a conversion: an empty
            // list would otherwise satisfy "every other civilization" and hand
            // the last survivor a religious win in a lobby with domination off.
            if rivals.is_empty() {
                continue;
            }
            let mut all = true;
            for o in rivals {
                let cities: Vec<&City> = self.cities.values().filter(|c| c.owner == o).collect();
                if cities.is_empty() {
                    all = false; // a civ without cities cannot be converted
                    break;
                }
                let following = cities
                    .iter()
                    .filter(|c| self.city_religion(c) == Some(religion.as_str()))
                    .count();
                if following * 2 <= cities.len() {
                    all = false;
                    break;
                }
            }
            if all && self.set_winner(p, "religious") {
                return;
            }
        }
    }

    // -------------------------------------------------- great people

    /// Player-level yields the empire collects each turn beside its cities:
    /// founder-belief income, the Faith from unused Great Person points, and
    /// the policy income that pays per Envoy or per tributary. All sit on
    /// Civilization VI's top bar next to the city sum, so every reader of
    /// "yields per turn" adds this to the cities.
    pub fn player_yield_extras(&self, pid: usize) -> Yields {
        let mut extras = self.founder_belief_yields(pid);
        extras.faith += self.unused_great_person_faith(pid);
        extras.add(self.player_policy_yields(pid));
        extras
    }

    /// Policy income paid to the PLAYER rather than to any city: Merchant
    /// Confederation's and an Emergency's Gold per placed Envoy
    /// (`MODIFIER_PLAYER_ADJUST_YIELD_CHANGE_PER_USED_INFLUENCE_TOKEN`) and
    /// Raj's Science, Culture, Faith and Gold per tributary
    /// (`MODIFIER_PLAYER_ADJUST_YIELD_CHANGE_PER_TRIBUTARY`). None of it is a
    /// line of a city's ledger — the host's capital showed no trace of 27
    /// Envoys' Gold — so it is banked with the founder-belief income at turn
    /// end and reported beside the city sum, not inside the Palace city.
    pub fn player_policy_yields(&self, pid: usize) -> Yields {
        let mut yields = Yields::default();
        let Some(player) = self.players.get(pid) else {
            return yields;
        };
        if player.is_minor || player.is_barbarian {
            return yields;
        }
        let envoys: i64 = player.envoys.iter().map(|(_, count)| *count).sum();
        yields.gold += envoys as f64 * self.policy_effect(pid, "gold_per_envoy");
        yields.gold += envoys as f64
            * player
                .counters
                .get("emergency_gold_per_envoy")
                .copied()
                .unwrap_or(0) as f64;
        let suzerains = self
            .players
            .iter()
            .filter(|minor| {
                minor.is_minor && !minor.is_barbarian && self.suzerain_of(minor.id) == Some(pid)
            })
            .count() as f64;
        let raj = suzerains * self.policy_effect(pid, "suzerain_all_yields");
        yields.gold += raj;
        yields.science += raj;
        yields.culture += raj;
        yields.faith += raj;
        yields
    }

    fn at_war_with_any_civilization(&self, pid: usize) -> bool {
        self.players.iter().any(|other| {
            other.id != pid && other.alive && !other.is_barbarian && self.is_at_war(pid, other.id)
        })
    }

    fn highest_active_alliance_level(&self, pid: usize) -> i32 {
        self.players[pid]
            .alliances
            .values()
            .filter(|alliance| alliance.ends > self.turn)
            .map(|alliance| alliance.level)
            .max()
            .unwrap_or(0)
    }

    /// +% production toward the item at the head of a city's queue from
    /// slotted policy cards (Agoge, Maneuver, Maritime Industries, Ilkum,
    /// Colonization, Feudal Contract, Limes).
    pub fn item_prod_mult(&self, pid: usize, cid: u32, item: Option<&Item>) -> f64 {
        let mut bonus: f64 = 0.0;
        let economic = self.city_resource_industry_effects(&self.cities[&cid]);
        match item {
            Some(Item::Unit { unit }) | Some(Item::Formation { unit, .. }) => {
                let spec = &self.rules.units[unit];
                bonus += self.gov_effects(pid).unit_production_pct / 100.0;
                // Flower Power's ADJUST_UNIT_PRODUCTION_MODIFIER (-100).
                bonus += self.policy_effect(pid, "unit_production_pct") / 100.0;
                if spec.class == "military" {
                    bonus += economic.military_unit_production_pct / 100.0;
                }
                if matches!(unit.as_str(), "builder" | "settler" | "trader") {
                    bonus += economic.civilian_unit_production_pct / 100.0;
                }
                // `MINOR_CIV_DEFAULT_TRAIT`: a city-state trains Builders 200%
                // faster, and military units too while it holds fewer than 10
                // of them (`PLAYER_HAS_SMALL_MILITARY`).
                if self.players[pid].is_minor
                    && !self.players[pid].is_barbarian
                    && (unit == "builder"
                        || (spec.class == "military"
                            && self
                                .units
                                .values()
                                .filter(|held| {
                                    held.owner == pid
                                        && self.rules.units[held.kind].class == "military"
                                })
                                .count()
                                < 10))
                {
                    bonus += 2.0;
                }
                if spec.class == "military"
                    && self
                        .alliance_partner(pid, "military", 2)
                        .is_some_and(|partner| {
                            self.at_war_with_any_major(pid) || self.at_war_with_any_major(partner)
                        })
                {
                    bonus += 0.15;
                }
                if spec.class == "military" && self.dedication_active(pid, "to_arms") {
                    // POLICY_TO_ARMS_GA_PRODUCTION ships 25, not 15.
                    bonus += 0.25;
                }
                if unit == "builder" {
                    bonus += self.policy_effect(pid, "builder_production_pct") / 100.0;
                }
                if unit == "settler" {
                    bonus += self.policy_effect(pid, "settler_production_pct") / 100.0;
                    bonus += self
                        .city_building_effect(&self.cities[&cid], "settler_production_pct")
                        / 100.0;
                }
                if spec.domain.as_deref() == Some("sea") {
                    bonus += self.policy_effect_for_unit(pid, "naval_production_pct", unit) / 100.0;
                    bonus += self
                        .city_building_effect(&self.cities[&cid], "naval_unit_production_pct")
                        / 100.0;
                }
                if spec.promotion_class == "naval_ranged" {
                    bonus += self.players[pid]
                        .counters
                        .get("great_person:naval_ranged_production_pct")
                        .copied()
                        .unwrap_or(0) as f64
                        / 100.0;
                }
                if spec.domain.as_deref() == Some("sea") || unit == "settler" {
                    bonus += self
                        .city_district_effect(&self.cities[&cid], "naval_settler_production_pct")
                        / 100.0;
                }
                if spec.domain.as_deref() == Some("air") {
                    bonus += self.policy_effect_for_unit(pid, "air_production_pct", unit) / 100.0;
                }
                if spec.cavalry {
                    bonus +=
                        self.policy_effect_for_unit(pid, "cavalry_production_pct", unit) / 100.0;
                }
                if spec.class == "support" {
                    bonus +=
                        self.policy_effect_for_unit(pid, "support_production_pct", unit) / 100.0;
                }
                if matches!(
                    spec.promotion_class.as_str(),
                    "melee" | "anti_cavalry" | "ranged"
                ) {
                    bonus +=
                        self.policy_effect_for_unit(pid, "infantry_production_pct", unit) / 100.0;
                }
                if spec.promotion_class == "anti_cavalry" {
                    bonus += self.empire_wonder_effect(pid, "anti_cavalry_production_pct") / 100.0;
                }
                if unit == "giant_death_robot" {
                    bonus += self.policy_effect(pid, "gdr_production_pct") / 100.0;
                    if self.dedication_active(pid, "automaton_warfare") {
                        bonus += 0.50;
                    }
                }
                if unit == "aircraft_carrier" {
                    bonus += self.policy_effect(pid, "carrier_production_pct") / 100.0;
                }
                if unit == "spy" {
                    bonus += self.policy_effect(pid, "spy_production_pct") / 100.0;
                }
                if spec.ranged_strength > 0.0
                    && spec.class == "military"
                    && self.has_ability(pid, "ta_seti")
                {
                    bonus += 0.5; // Nubia: Ta-Seti
                }
                let unit_era = spec
                    .tech
                    .as_ref()
                    .and_then(|technology| self.rules.techs.get(technology))
                    .map(|technology| technology.era)
                    .or_else(|| {
                        spec.civic
                            .as_ref()
                            .and_then(|civic| self.rules.civics.get(civic))
                            .map(|civic| civic.era)
                    })
                    .unwrap_or(0);
                if spec.class == "military" && unit_era <= 1 {
                    bonus += self.pantheon_effect(pid, "early_military_production_pct") / 100.0;
                }
                if matches!(item, Some(Item::Formation { .. })) {
                    let effect = if spec.domain.as_deref() == Some("sea") {
                        "fleet_armada_production_pct"
                    } else {
                        "corps_army_production_pct"
                    };
                    bonus += self.city_building_effect(&self.cities[&cid], effect) / 100.0;
                    let discount = self
                        .city_district_effect(&self.cities[&cid], "corps_army_discount_pct")
                        .clamp(0.0, 99.0);
                    bonus += discount / (100.0 - discount);
                }
            }
            Some(Item::Building { building }) => {
                let spec = &self.rules.buildings[building];
                bonus += economic.building_production_pct / 100.0;
                // `MINOR_CIV_PRODUCTION_WALLS`: 200% on all three tiers.
                if self.players[pid].is_minor
                    && !self.players[pid].is_barbarian
                    && matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
                {
                    bonus += 2.0;
                }
                let district = spec
                    .district
                    .map(|district| self.district_family(district))
                    .unwrap_or(crate::name!("city_center"));
                if self.congress_effect_active("urban_development_treaty", "B", district.as_str()) {
                    return 0.0;
                }
                if self.congress_effect_active("urban_development_treaty", "A", district.as_str()) {
                    bonus += 1.0;
                }
                if self.congress_effect_active("global_energy_treaty", "B", building) {
                    return 0.0;
                }
                if spec.outer_defense > 0 {
                    bonus += self.policy_effect(pid, "wall_production_pct") / 100.0;
                }
                if spec.district.is_some_and(|district| {
                    matches!(
                        self.district_family(district).as_str(),
                        "encampment" | "harbor"
                    )
                }) {
                    bonus += self.policy_effect(pid, "military_port_production_pct") / 100.0;
                }
                // Several Governor promotions speed the buildings of one
                // district family alongside their headline effect.
                bonus +=
                    self.governor_effect(pid, cid, &format!("{district}_building_production_pct"))
                        / 100.0;
            }
            Some(Item::District { district, .. }) => {
                let family = self.district_family(*district);
                bonus += self.governor_effect(pid, cid, "district_production_pct") / 100.0;
                bonus += self.gov_effects(pid).district_production_pct / 100.0;
                // City Patron Goddess: CITY_PATRON_GODDESS_DISTRICT_PRODUCTION_MODIFIER,
                // `EFFECT_ADJUST_ALL_DISTRICT_PRODUCTION_MODIFIER` Amount 25,
                // subject `CITY_HAS_0_SPECIALTY_DISTRICTS_REQUIREMENTS` — which
                // is `REQUIREMENT_CITY_HAS_X_SPECIALTY_DISTRICTS` Amount 1 with
                // Inverse set, i.e. the city has none. `MustBeFunctioning 0`, so
                // a district that stands but is pillaged still ends the bonus;
                // one still under construction has not been placed yet and does
                // not. No id in this belief appears in `Expansion2_RemoveData.xml`.
                if self.city_specialty_district_count(&self.cities[&cid]) == 0 {
                    bonus += self.pantheon_effect(pid, "first_district_production_pct") / 100.0;
                }
                if matches!(family.as_str(), "encampment" | "harbor") {
                    bonus += self.policy_effect(pid, "military_port_production_pct") / 100.0;
                }
                bonus += self.civ_effect(pid, &format!("{family}_district_production_pct")) / 100.0;
                bonus += self.city_state_district_production_pct(pid, family.as_str()) / 100.0;
            }
            Some(Item::Wonder { wonder, pos }) => {
                bonus += self.policy_effect(pid, "wonder_production_pct") / 100.0;
                bonus += self.gov_effects(pid).wonder_production_pct / 100.0;
                // ADJUST_WONDER_ERA_PRODUCTION carries a StartEra/EndEra window,
                // and two of the three cards using it are named after a window
                // they do not have: GOTHICARCHITECTURE_MEDIEVALRENAISSANCEWONDER
                // starts at ANCIENT, and SKYSCRAPERS_INDUSTRIALINFORMATION runs
                // ANCIENT to FUTURE, which is every wonder in the game. Every
                // shipped row starts at ANCIENT, so only the end ever varies.
                let era = self.wonder_era(wonder);
                if era <= 1 {
                    bonus += self.policy_effect(pid, "classical_wonder_production_pct") / 100.0;
                    // Monument to the Gods rides the same window:
                    // MONUMENT_TO_THE_GODS_ANCIENTCLASSICALWONDER_MODIFIER is
                    // `MODIFIER_PLAYER_CITIES_ADJUST_WONDER_ERA_PRODUCTION`,
                    // Amount 15, StartEra ANCIENT, EndEra CLASSICAL, IsWonder 1
                    // — the identical StartEra/EndEra pair the policy cards
                    // above already reduce to `era <= 1`. Not in
                    // `Expansion2_RemoveData.xml`.
                    bonus += self.pantheon_effect(pid, "ancient_classical_wonder_production_pct")
                        / 100.0;
                }
                if era <= 3 {
                    bonus += self.policy_effect(pid, "renaissance_wonder_production_pct") / 100.0;
                }
                if self.has_ability(pid, "iteru") && self.map.tiles[pos].has_river() {
                    bonus += 0.15; // Egypt: Iteru (river cities)
                }
                if self.grants_city_state_unique_bonus(pid, "Brussels") {
                    bonus += 0.15;
                }
            }
            Some(Item::Project { project }) => {
                // Model the late-game factory, power and specialist stack that
                // the compact city layer does not represent explicitly.
                if self
                    .rules
                    .projects
                    .get(project)
                    .and_then(|spec| spec.district)
                    == Some(crate::name!("spaceport"))
                {
                    bonus += 1.0;
                }
                bonus += self.gov_effects(pid).project_production_pct / 100.0;
                if self.grants_city_state_unique_bonus(pid, "Hong Kong") {
                    bonus += 0.20;
                }
                if matches!(
                    project.as_str(),
                    "build_nuclear_device" | "build_thermonuclear_device"
                ) {
                    bonus +=
                        self.governor_effect(pid, cid, "nuclear_armament_production_pct") / 100.0;
                }
                // Rogue State: the four ROGUESTATE_*_PRODUCTION rows.
                if matches!(
                    project.as_str(),
                    "manhattan_project"
                        | "operation_ivy"
                        | "build_nuclear_device"
                        | "build_thermonuclear_device"
                ) {
                    bonus += self.policy_effect(pid, "nuclear_project_production_pct") / 100.0;
                }
                // Automated Workforce: ADJUST_ALL_PROJECTS_PRODUCTION.
                bonus += self.policy_effect(pid, "project_production_pct") / 100.0;
                if self.congress_effect_active("public_works_program", "A", project) {
                    bonus += 1.0;
                } else if self.congress_effect_active("public_works_program", "B", project) {
                    bonus -= 0.5;
                }
                if self.rules.projects[project].district == Some(crate::name!("spaceport")) {
                    bonus += self.governor_effect(pid, cid, "space_project_production_pct") / 100.0;
                }
                if self.rules.projects[project].district == Some(crate::name!("spaceport"))
                    && (self.city_has_building_family(
                        &self.cities[&cid],
                        crate::name!("military_academy"),
                    ) || self
                        .city_has_building_family(&self.cities[&cid], crate::name!("seaport")))
                {
                    bonus += self.policy_effect(pid, "space_project_production_pct") / 100.0;
                }
                let completions = self.players[pid]
                    .counters
                    .get("tree_completions:future_tech")
                    .copied()
                    .unwrap_or(0) as f64;
                bonus += completions
                    * self.rules.techs["future_tech"]
                        .effects
                        .get("project_production_pct_per_completion")
                        .copied()
                        .unwrap_or(0.0)
                    / 100.0;
            }
            Some(Item::Product { .. }) => {
                // Create New Product is a city project, so project modifiers
                // apply even though its resource identity is dynamic.
                bonus += self.gov_effects(pid).project_production_pct / 100.0;
            }
            _ => {}
        }
        // A civilization whose signature ability speeds one kind of build
        // declares the percentage in civs.json rather than earning a branch of
        // its own here.
        bonus += match item {
            Some(Item::Unit { unit }) | Some(Item::Formation { unit, .. }) => {
                let mut pct = self.civ_effect(pid, "unit_production_pct");
                if unit == "settler" {
                    pct += self.civ_effect(pid, "settler_production_pct");
                }
                pct
            }
            Some(Item::Building { .. }) => self.civ_effect(pid, "building_production_pct"),
            Some(Item::District { .. }) => self.civ_effect(pid, "district_production_pct"),
            Some(Item::Wonder { .. }) => self.civ_effect(pid, "wonder_production_pct"),
            _ => 0.0,
        } / 100.0;
        let kilwa_kind = match item {
            Some(Item::Unit { .. } | Item::Formation { .. }) => Some("militaristic"),
            Some(Item::Building { .. } | Item::District { .. } | Item::Wonder { .. }) => {
                Some("industrial")
            }
            _ => None,
        };
        if let Some(kind) = kilwa_kind {
            bonus += self.kilwa_type_bonus_pct(pid, &self.cities[&cid], kind) / 100.0;
        }
        1.0 + bonus
    }

    pub fn unit_strength(&self, u: &Unit, defending: bool) -> f64 {
        if self.is_embarked(u) {
            // Embarked strength climbs the shipped era ladder: 10 in the
            // Ancient era up to 55 from the Atomic era on.
            let era = crate::rules::ERA_NAMES[self.player_era(u.owner).min(8)];
            return self.rules.eras[era].embarked_strength
                + self.unit_formation_bonus(u)
                + if defending {
                    self.governor_unit_defense_bonus(u)
                } else {
                    0.0
                };
        }
        let mut s = self.unit_unembarked_strength(u);
        if defending {
            // `unit_unembarked_strength` is the attack-side value, so the
            // attacking-only half of the government bonus comes back off.
            // FASCISM_ATTACK_BUFF carries FASCISM_REQUIREMENTS, a single
            // REQUIREMENT_PLAYER_IS_ATTACKING.
            s -= self.government_combat_bonus(u, true) - self.government_combat_bonus(u, false);
            // GDR_ARMOR_DEFENSES requires the defender and a non-air
            // opponent (Expansion2_UnitPromotions.xml). Ground melee,
            // ranged and city shots use this defensive value; air strikes
            // against GDRs use their dedicated Anti-Air Strength instead.
            if u.kind == "giant_death_robot" {
                s += self.tree_effect(u.owner, "gdr_armor");
            }
            s += self.governor_unit_defense_bonus(u);
            s += 3.0 * u.fortify_turns.clamp(0, 2) as f64;
            s += self.promotion_effect(u, "defend_all");
            let tile = &self.map.tiles[&u.pos];
            if self.city_at(u.pos).is_some() || tile.district.is_some() {
                s += self.promotion_effect(u, "district_defense");
            }
            if tile.hills || matches!(tile.feature.as_deref(), Some("forest" | "jungle" | "marsh"))
            {
                s += self.promotion_effect(u, "rough_defense");
            }
            if u.linked_to.is_some() {
                s += self.promotion_effect(u, "formation_combat");
            }
        }
        s
    }

    /// Whether `Action::Fortify` will be accepted for `u`. `pub(crate)` so the
    /// controllers can ask the engine instead of guessing: `fortify_or_stop`
    /// is the fleet's universal "nothing better to do" ending and used to
    /// issue the order for embarked units, Settlers, Builders and ships,
    /// which the engine refuses — 1,549 refused orders in one 150-turn
    /// six-player game, the largest single refusal category measured.
    pub(crate) fn unit_can_fortify(&self, u: &Unit) -> bool {
        let spec = &self.rules.units[u.kind];
        spec.class == "military" && spec.domain.as_deref() != Some("sea") && !self.is_embarked(u)
    }

    /// The model-side Gold quote for buying a Granary in a newly founded city.
    ///
    /// Before a city exists the live host has no city-specific purchase menu to
    /// query. A fresh city has no district discount, buildings, queue, or
    /// production block, however, so its standard quote is exact once Pottery
    /// and the empire-wide purchase rules permit it. `None` is deliberately
    /// conservative when a future city could not have the Granary immediately.
    pub(crate) fn new_city_granary_gold_purchase_cost(&self, pid: usize) -> Option<f64> {
        let granary = crate::name!("granary");
        let spec = self.rules.buildings.get(&granary)?;
        let item = Item::Building { building: granary };
        let city_center = spec
            .district
            .is_none_or(|district| self.district_family(district) == "city_center");
        let unavailable = self.is_arena()
            || !self.unlocked(pid, &spec.tech, &spec.civic)
            || !spec.buildable
            || spec.purchase_only
            || !city_center
            || !spec.requires.is_empty()
            || !spec.requires_any.is_empty()
            || spec.wonder
            || spec.outer_defense > 0
            || spec.coastal
            || spec
                .effects
                .get("requires_river_city")
                .copied()
                .unwrap_or(0.0)
                > 0.0
            || spec
                .effects
                .get("requires_fresh_water_city")
                .copied()
                .unwrap_or(0.0)
                > 0.0
            || spec
                .unique_to
                .as_deref()
                .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
            || self.rules.buildings.values().any(|candidate| {
                candidate.replaces == Some(granary)
                    && candidate
                        .unique_to
                        .as_deref()
                        .is_some_and(|owner| self.owns_civ_unique(pid, owner))
            })
            || self.congress_effect_active("urban_development_treaty", "B", "city_center")
            || self.congress_effect_active("global_energy_treaty", "B", "granary");
        if unavailable {
            return None;
        }
        let discount = self
            .gov_effects(pid)
            .gold_purchase_discount_pct
            .clamp(0.0, 100.0);
        Some(self.item_cost(&item) * 4.0 * (1.0 - discount / 100.0))
    }

    pub fn wonder_built(&self, name: &str) -> bool {
        let stamp = self.world_stamp();
        let mut cache = self.vision.borrow_mut();
        cache
            .wonders(stamp, || {
                let mut built: BTreeSet<Name> = BTreeSet::new();
                for city in self.cities.values() {
                    built.extend(city.wonders.keys().cloned());
                }
                for tile in self.map.tiles.values() {
                    if let Some(wonder) = &tile.wonder {
                        built.insert(*wonder);
                    }
                }
                built
            })
            .iter()
            .any(|built| *built == name)
    }

    fn empire_building_sum(
        &self,
        pid: usize,
        f: impl Fn(&crate::rules::BuildingSpec) -> f64,
    ) -> f64 {
        self.cities
            .values()
            .filter(|c| c.owner == pid)
            .flat_map(|c| {
                c.buildings.iter().filter(|building| {
                    !c.pillaged_buildings.contains(*building)
                        && self.building_district_is_active(c, building)
                })
            })
            .map(|b| f(&self.rules.buildings[b]))
            .sum()
    }

    fn city_building_effect(&self, city: &City, effect: &str) -> f64 {
        if !self.rules.effect_index.buildings(effect) {
            return 0.0;
        }
        city.buildings
            .iter()
            .filter_map(|building| {
                // Ask what a building grants before asking whether it is
                // working. Most buildings grant nothing towards a given
                // effect, and deciding whether one's district is standing
                // means walking the city's districts — far dearer than the
                // table lookup that rules it out.
                let value = self.rules.buildings[building]
                    .effects
                    .get(effect)
                    .copied()?;
                (!city.pillaged_buildings.contains(building)
                    && self.building_district_is_active(city, building))
                .then_some(value)
            })
            .sum()
    }

    fn empire_wonder_effect(&self, pid: usize, effect: &str) -> f64 {
        // No wonder in the ruleset grants anything towards this, so no
        // arrangement of built wonders can either — and the alternative is
        // walking every city in the world to reach that same zero.
        if !self.rules.effect_index.wonders(effect) {
            return 0.0;
        }
        // The memo is deliberately guard-scoped. `QueryMemo` borrows the
        // world immutably, so a cached aggregate cannot survive a wonder
        // completion, capture, or raze and become stale. Outside a memo
        // scope, retain the old scalar path: one-off callers should not pay
        // to allocate a table for an answer they will read once.
        if self.query_memo.wonder_effects.borrow().is_none() {
            return self
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .flat_map(|city| city.wonders.keys())
                .map(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get(effect)
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum();
        }
        if let Some(aggregate) = self
            .query_memo
            .wonder_effects
            .borrow()
            .as_ref()
            .and_then(|all| all.get(&pid))
        {
            return aggregate.get(effect).copied().unwrap_or(0.0);
        }

        let mut aggregate = BTreeMap::<String, f64>::new();
        for city in self.cities.values().filter(|city| city.owner == pid) {
            for wonder in city.wonders.keys() {
                for (name, value) in &self.rules.wonders[wonder].effects {
                    *aggregate.entry(name.clone()).or_default() += *value;
                }
            }
        }
        let value = aggregate.get(effect).copied().unwrap_or(0.0);
        self.query_memo
            .wonder_effects
            .borrow_mut()
            .as_mut()
            .expect("a wonder aggregate is built only inside a memo scope")
            .insert(pid, aggregate);
        value
    }

    /// Stock fuel conversion and emissions read from the power-plant data.
    /// `*_per_power` is fuel consumed per point of Power, while
    /// `co2_per_power` is the Gathering Storm emissions intensity.
    fn fuel_resource_profile(&self, resource: &str) -> Option<(&'static str, f64, f64, i32)> {
        let (plant, consumption_key, priority) = match resource {
            "coal" => ("coal_power_plant", "coal_per_power", 1),
            "oil" => ("oil_power_plant", "oil_per_power", 2),
            "uranium" => ("nuclear_power_plant", "uranium_per_power", 3),
            _ => return None,
        };
        let effects = &self.rules.buildings[plant].effects;
        let fuel_per_power = effects.get(consumption_key).copied().unwrap_or(0.0);
        let power_per_resource = effects.get("power_generated").copied().unwrap_or(0.0);
        let co2_per_power = effects.get("co2_per_power").copied().unwrap_or(0.0);
        if fuel_per_power <= 0.0 || power_per_resource <= 0.0 || co2_per_power < 0.0 {
            return None;
        }
        debug_assert!(
            (fuel_per_power * power_per_resource - 1.0).abs() <= 1e-9,
            "{plant} declares inconsistent fuel conversion"
        );
        Some((plant, power_per_resource, co2_per_power, priority))
    }

    fn city_wonder_effect(&self, city: &City, effect: &str) -> f64 {
        city.wonders
            .keys()
            .map(|wonder| {
                self.rules.wonders[wonder]
                    .effects
                    .get(effect)
                    .copied()
                    .unwrap_or(0.0)
            })
            .sum()
    }

    pub(crate) fn encampment_can_strike(&self, city: &City) -> bool {
        city.encampment_hp > 0
            && city.encampment_wall_hp > 0
            && !city.encampment_pillaged
            && (!city.encampment_struck
                || city.encampment_extra_strikes_used
                    < self.governor_effect(city.owner, city.id, "city_extra_strike") as i32)
    }

    pub fn available_techs(&self, pid: usize) -> Vec<Name> {
        let p = &self.players[pid];
        self.rules
            .techs
            .iter()
            .filter(|(t, s)| {
                (!p.techs.contains(*t) || s.repeatable)
                    && s.requires.iter().all(|r| p.techs.contains(&Name::new(r)))
            })
            .map(|(t, _)| *t)
            .collect()
    }

    pub fn available_civics(&self, pid: usize) -> Vec<Name> {
        let p = &self.players[pid];
        self.rules
            .civics
            .iter()
            .filter(|(c, s)| {
                (!p.civics.contains(*c) || s.repeatable)
                    && s.requires.iter().all(|r| p.civics.contains(&Name::new(r)))
            })
            .map(|(c, _)| *c)
            .collect()
    }

    /// Gathering Storm scoring, per the Civilopedia Score/Time rules:
    /// 3/civic, 5/city, 2/district (4 if unique), 1/building, 1/Citizen,
    /// 5/Great Person, 10 for founding a religion + 2 per foreign city
    /// following it, 2/technology, 15/wonder, plus Era Score. Units and
    /// population multipliers are deliberately absent — they score nothing
    /// in the shipped game.
    pub fn score(&self, pid: usize) -> i64 {
        if let Some(score) = self.observed_score.get(&pid) {
            return (*score).max(0);
        }
        self.score_parts(pid).iter().sum()
    }

    /// Civ 6 score ranking: total first, then the shipped tiebreaker chain
    /// (civics, cities, districts, Population, Great People, religion,
    /// technologies, wonders). Higher is better on every key.
    pub fn score_rank_key(&self, pid: usize) -> (i64, [i64; 9]) {
        let parts = self.score_parts(pid);
        (self.score(pid), parts)
    }

    /// The stock team score is the sum of every member civilization's score;
    /// the category vector is summed as well so the ordinary tiebreak chain
    /// remains deterministic between tied teams.
    pub fn team_score_rank_key(&self, pid: usize) -> (i64, [i64; 9]) {
        let mut parts = [0_i64; 9];
        let mut score = 0_i64;
        for member in self.team_members(pid) {
            score += self.score(member);
            let member_parts = self.score_parts(member);
            for (total, value) in parts.iter_mut().zip(member_parts) {
                *total += value;
            }
        }
        (score, parts)
    }

    pub fn military_power(&self, pid: usize) -> f64 {
        let represented = self
            .units
            .values()
            .filter(|u| u.owner == pid)
            .map(|u| {
                (self.rules.units[u.kind].strength + self.unit_formation_bonus(u)) * u.hp as f64
                    / 100.0
            })
            .sum::<f64>();
        self.observed_military_power
            .get(&pid)
            .copied()
            .unwrap_or(0.0)
            .max(0.0)
            .max(represented)
    }

    /// Fielded ground strength. The host reports only one military total, so
    /// a navy cannot be removed reliably from `observed_military_power`.
    /// Use our complete unit roster when sizing a land campaign instead.
    pub fn land_military_power(&self, pid: usize) -> f64 {
        self.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && !matches!(
                        self.rules.units[unit.kind].domain.as_deref(),
                        Some("sea" | "air")
                    )
            })
            .map(|unit| {
                (self.rules.units[unit.kind].strength + self.unit_formation_bonus(unit))
                    * unit.hp as f64
                    / 100.0
            })
            .sum()
    }

    fn unlocked(&self, pid: usize, tech: &Option<Name>, civic: &Option<Name>) -> bool {
        let p = &self.players[pid];
        if let Some(t) = tech {
            if !p.techs.contains(t) {
                return false;
            }
        }
        if let Some(c) = civic {
            if !p.civics.contains(c) {
                return false;
            }
        }
        true
    }

    /// Whose borders a tile is inside, if anybody's.
    fn tile_owner(&self, position: Pos) -> Option<usize> {
        self.map
            .get(position)?
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner)
    }

    /// The city-states this player is suzerain of. Suzerainty does not depend
    /// on which resource is being counted, so a caller sweeping every resource
    /// works it out once rather than re-deciding every city-state's patron for
    /// each luxury in the ruleset.
    fn suzerained_minors(&self, pid: usize) -> Vec<usize> {
        self.players
            .iter()
            .filter(|player| {
                player.is_minor && !player.is_barbarian && self.suzerain_of(player.id) == Some(pid)
            })
            .map(|player| player.id)
            .collect()
    }

    /// The seat whose flag stands on this tile, if one does.
    pub fn arena_flag_holder(&self, pos: Pos) -> Option<usize> {
        self.arena_flags
            .iter()
            .find(|(_, at)| **at == pos)
            .map(|(owner, _)| *owner)
    }

    /// Whether these two seats are the same side of a battle — the same
    /// seat, or two seats sharing a team. Teammates cannot take each other's
    /// flag any more than a side can take its own.
    fn same_side(&self, first: usize, second: usize) -> bool {
        first == second || self.same_team(first, second)
    }

    /// The flag this seat is trying to take: the nearest one that is not its
    /// own side's, measured from `from`. `None` on any battle without flags,
    /// and once every enemy flag is gone.
    pub fn arena_enemy_flag(&self, pid: usize, from: Pos) -> Option<Pos> {
        self.arena_flags
            .iter()
            .filter(|(owner, _)| !self.same_side(**owner, pid))
            .map(|(_, at)| *at)
            .min_by_key(|at| (self.wdist(from, *at), *at))
    }

    fn improvement_fortification_at(&self, pos: Pos) -> i32 {
        let tile = &self.map.tiles[&pos];
        if tile.pillaged {
            return 0;
        }
        tile.improvement
            .as_deref()
            .and_then(|improvement| self.rules.improvements.get(improvement))
            .and_then(|improvement| improvement.effects.get("grant_fortification"))
            .copied()
            .unwrap_or(0.0)
            .round()
            .clamp(0.0, 2.0) as i32
    }

    /// A height table sized to this map, for one visibility sweep.
    fn height_field(&self) -> HeightField {
        HeightField::for_map(self.world_stamp(), self.map.tiles.len())
    }

    /// Strongest same-owner support aura on this unit's tile or an adjacent
    /// tile. Effects of the same family do not stack, so a maximum also
    /// handles a Medic beside a Supply Convoy or a Balloon beside a Drone.
    fn adjacent_support_effect(&self, unit: &Unit, effect: &str) -> f64 {
        self.wdisk(unit.pos, 1)
            .into_iter()
            .flat_map(|position| self.unit_ids_at(position))
            .filter(|other| self.units[other].owner == unit.owner)
            .filter_map(|other| {
                let support = &self.rules.units[self.units[other].kind];
                (support.class == "support")
                    .then(|| support.effects.get(effect).copied().unwrap_or(0.0))
            })
            .fold(0.0, f64::max)
    }

    /// Strongest aura supplied by any adjacent same-owner unit. Unlike the
    /// support-only family above, unique-unit formation bonuses such as the
    /// Tagma's apply from an ordinary military unit and never to itself.
    fn adjacent_friendly_unit_effect(&self, unit: &Unit, effect: &str) -> f64 {
        self.wdisk(unit.pos, 1)
            .into_iter()
            .flat_map(|position| self.unit_ids_at(position))
            .filter(|other| **other != unit.id && self.units[other].owner == unit.owner)
            .map(|other| {
                self.rules.units[self.units[other].kind]
                    .effects
                    .get(effect)
                    .copied()
                    .unwrap_or(0.0)
            })
            .fold(0.0, f64::max)
    }

    /// Taxis scales with converted Holy Cities, including an enemy founder's
    /// Holy City after it adopts Byzantium's founded religion.
    fn taxis_holy_city_strength(&self, pid: usize) -> f64 {
        if !self.has_ability(pid, "taxis") {
            return 0.0;
        }
        let Some(religion) = self.players[pid].religion.as_deref() else {
            return 0.0;
        };
        let converted = self
            .players
            .iter()
            .filter_map(|player| player.holy_city)
            .filter_map(|city| self.cities.get(&city))
            .filter(|city| self.city_religion(city) == Some(religion))
            .count() as f64;
        converted * self.civ_effect(pid, "holy_city_combat_strength")
    }

    /// Aircraft rebase up to twice their printed Movement. This deliberately
    /// differs from operational attack range (and from range promotions).
    pub fn air_rebase_range(&self, uid: u32) -> i32 {
        self.units
            .get(&uid)
            .filter(|unit| self.rules.units[unit.kind].domain.as_deref() == Some("air"))
            .map(|unit| (self.rules.units[unit.kind].moves * 2.0).floor() as i32)
            .unwrap_or(0)
    }

    fn can_air_patrol_at(&self, pid: usize, pos: Pos) -> bool {
        let territory_owner = self
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| self.cities.get(&city).map(|city| city.owner));
        territory_owner.is_none_or(|owner| owner == pid) && !self.enemy_combat_target_at(pid, pos)
    }

    /// An owned copy of the current frame, for a caller that genuinely needs
    /// one to mutate or move independently of the engine's cache. Every
    /// production call site turned out to be a pure read and now takes
    /// [`Self::player_vision_frame`] instead; this stays for tests that want
    /// an owned snapshot without depending on the `Arc`.
    #[cfg(test)]
    pub(crate) fn player_vision_now(&self, pid: usize) -> TileBits {
        self.player_vision_frame(pid).as_ref().clone()
    }

    #[inline]
    pub(crate) fn sees(&self, visible: &TileBits, pos: Pos) -> bool {
        self.map
            .tiles
            .index_of(pos)
            .is_some_and(|index| visible.contains(index))
    }

    /// Whether one tile is currently in a player's sight.
    pub fn player_can_see(&self, pid: usize, pos: Pos) -> bool {
        self.sees(&self.player_vision_frame(pid), pos)
    }

    fn player_vision(&self, heights: &mut HeightField, pid: usize) -> TileBits {
        // An arena is a field, and by default both commanders can see all of
        // it.
        //
        // A Tactics battle is meant to be a test of what each side does with
        // what is in front of it, not of who finds the other first — and a
        // fogged arena is not even that. Both armies are set down out of each
        // other's sight, neither has a city to march on or a border to
        // trespass, and the objective each side would advance toward is an
        // enemy it cannot see; measured, both sides simply stood in their own
        // deployment bands for five hundred turns and the clock decided it.
        // Lifting the fog is symmetric — it is the rule of the arena, not a
        // handicap given to one side.
        //
        // That is the default rather than the rule: `tactics.fog` is the
        // deliberate mode the comment above used to leave for later, and it
        // simply declines the shortcut so the field is scouted the way a world
        // is. It is symmetric in the same way — both sides are fogged or
        // neither is.
        if self.is_arena() && !self.tactics.fog {
            return TileBits::all(self.map.tiles.len());
        }
        let mut visible = TileBits::with_capacity(self.map.tiles.len());
        for viewer in self.visibility_viewers(pid) {
            visible.union_with(&self.base_player_visibility(heights, viewer));
        }
        // What the HOST says this seat can see, added to what we derive. See
        // [`Game::host_observed`]: the derivation runs on a reconstructed map and
        // loses ground the export was explicit about, and the tactical layer reads
        // this answer to decide whether an enemy may be engaged at all.
        //
        // Seat 0 only, because that is the one seat a mirrored game reconstructs
        // and the only one the set is ever populated for. An ordinary CIVVIS game
        // leaves it empty, so this loop does not run.
        if pid == MIRRORED_SEAT && !self.host_observed.is_empty() {
            for pos in self.host_observed.iter() {
                if let Some(index) = self.map.tiles.index_of(*pos) {
                    visible.insert(index);
                }
            }
        }
        visible
    }

    /// Make a branch for a read-and-discard simulation.
    ///
    /// Search branches never publish an observation, and nobody reads their
    /// explored ground, contacts, fog memory, or mid-turn war narration. The
    /// branch therefore keeps the complete rule state but suppresses those
    /// observer-only updates in addition to disabling their retained ledgers.
    /// Its derived caches are reset or shared cheaply by `Game::clone`, so a
    /// disposable branch starts paying only for work its hypothetical actions
    /// actually ask for.
    pub fn speculative_clone(&self) -> Self {
        let mut branch = self.clone();
        branch.track_fog_memory = false;
        branch.track_war_ledger = false;
        branch.visibility_suppressed = true;
        branch.visibility_batch = VisibilityBatch::default();
        branch
    }

    /// Mark this world as simultaneous-planning scaffolding (or ordinary,
    /// with [`PlanningRole::Off`]). Only the simultaneous driver calls this,
    /// and only on worlds it will discard; a committed game never carries a
    /// role. See [`PlanningRole`] for what each role elides.
    pub fn set_planning_role(&mut self, role: PlanningRole) {
        self.planning_role = role;
    }

    /// A standing Encampment remains a combat target at zero HP until a
    /// melee unit enters and pillages it, like a depleted City Center.
    /// Every fort's combat presence is `defending_district_at`; this reads the
    /// Encampment alone, for the siege readers keyed to its legacy fields.
    pub(crate) fn encampment_at(&self, pos: Pos) -> Option<u32> {
        let tile = self.map.get(pos)?;
        if !tile
            .district
            .is_some_and(|district| self.district_is_family(district, crate::name!("encampment")))
        {
            return None;
        }
        let cid = tile.owner_city?;
        let city = self.cities.get(&cid)?;
        (!city.encampment_pillaged).then_some(cid)
    }

    /// Exchange two adjacent units of one player.
    ///
    /// Civilization VI offers this wherever a unit's move would end on a
    /// friendly unit of its own stacking layer, and it is the reason that rule
    /// costs a front line nothing: a damaged unit trades places with the
    /// healthy one behind it without either leaving the line. Both units move,
    /// so both pay the entry cost of the tile they take and both lose
    /// fortification — a swap is two steps, never a free reposition. See
    /// [`Game::entry_at`] for the stacking rule itself and `docs/MOVEMENT.md`
    /// for the whole of it.
    fn do_swap(&mut self, pid: usize, uid: u32, other: u32) -> Result<(), String> {
        if uid == other {
            return Err("a unit cannot swap with itself".into());
        }
        let from = self.own_unit(pid, uid)?.pos;
        let to = self.own_unit(pid, other)?.pos;
        if self.wdist(from, to) != 1 {
            return Err("units are not adjacent".into());
        }
        if self.units[&uid].linked_to.is_some() || self.units[&other].linked_to.is_some() {
            // A linked escort moves as one unit with its charge; exchanging
            // half of a formation is a different question from this one.
            return Err("a linked unit cannot swap".into());
        }
        let (mover_kind, peer_kind) = (self.units[&uid].kind, self.units[&other].kind);
        if !self.shares_stacking_layer(&self.rules.units[mover_kind], &self.rules.units[peer_kind])
        {
            // Nothing to exchange: units on different layers already share a
            // tile, so the mover may simply move.
            return Err("those units do not share a stacking layer".into());
        }
        if [from, to].iter().any(|pos| {
            self.unit_ids_at(*pos)
                .iter()
                .any(|id| self.rules.units[self.units[id].kind].domain.as_deref() == Some("air"))
        }) {
            // Based aircraft belong to the tile, not to the unit carrying
            // them, and a swap would leave them under somebody else.
            return Err("a tile with based aircraft cannot swap".into());
        }
        // Each half is an ordinary crossing step onto the other's tile: the
        // layer it would contend for is the one being vacated in the same
        // action, and everything else — cost, zone of control, cliffs,
        // borders, a hostile city — binds exactly as it does for a move.
        if !self.can_move_step(uid, to, false) || !self.can_move_step(other, from, false) {
            return Err("invalid swap".into());
        }
        let mover_cost = self.unit_step_cost(uid, from, to);
        let peer_cost = self.unit_step_cost(other, to, from);
        for id in [uid, other] {
            let unit = self.units.get_mut(&id).unwrap();
            unit.fortified = false;
            unit.fortify_turns = 0;
            unit.acted = true;
            unit.moved = true;
        }
        self.relocate(uid, to);
        self.relocate(other, from);
        for (id, cost, arrived) in [(uid, mover_cost, to), (other, peer_cost, from)] {
            let cap = self.unit_max_moves_at(id, arrived);
            let unit = self.units.get_mut(&id).unwrap();
            unit.moves_left = (unit.moves_left - cost).max(0.0).min(cap);
        }
        for (id, arrived) in [(uid, to), (other, from)] {
            if self.formation_enters_enemy_zoc(id, arrived) {
                self.stop_unit_by_zoc(id);
            }
        }
        self.record_move_step(pid, uid, from, to);
        self.record_move_step(pid, other, to, from);
        Ok(())
    }

    fn do_move_to(&mut self, pid: usize, uid: u32, to: Pos) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        if u.moves_left <= 0.0 {
            return Err("no moves left".into());
        }
        let path = self
            .path_to(uid, to)
            .ok_or_else(|| "unreachable".to_string())?;
        if path.is_empty() {
            return Err("already there".into());
        }
        // ★★★★★ THE ONE ACTION THAT MAY CROSS OUR OWN UNITS, AND THE ONE
        // PLACE THE "NEVER END STACKED" RULE HAS TO BE ENFORCED RATHER THAN
        // CHECKED. `path_to` proves the destination is stoppable, but a walk
        // can still stop short: it floods on this unit alone, so a linked
        // escort's own legality can refuse a step, and a fresh unit's
        // one-free-step allowance can run the budget out. Anchoring on the
        // last hex the unit could legally be *left* on turns a truncated walk
        // into a legal position instead of an illegal stack.
        let last = path.len() - 1;
        let mut anchor = self.units[&uid].pos;
        for (index, step) in path.into_iter().enumerate() {
            if self
                .units
                .get(&uid)
                .map(|x| x.moves_left <= 0.0)
                .unwrap_or(true)
            {
                break;
            }
            if self.do_move_step(pid, uid, step, index == last).is_err() {
                break; // out of MP or stopped by ZOC mid-path
            }
            if self.can_stop(uid, step) {
                anchor = step;
            }
        }
        if let Some(at) = self.units.get(&uid).map(|unit| unit.pos) {
            if at != anchor && !self.can_stop(uid, at) {
                self.relocate(uid, anchor);
                if let Some(unit) = self.units.get_mut(&uid) {
                    // It spent this turn's movement on the attempt either way,
                    // and a walk that reaches here has already paid for the
                    // tiles it crossed. Zero rather than restored, so no
                    // truncated crossing can hand movement back.
                    unit.moves_left = 0.0;
                }
            }
        }
        Ok(())
    }

    // -------------------------------------------------- loyalty & governors

    /// Governor titles come from civic milestones and completed districts
    /// such as the Government Plaza.
    pub fn governor_titles(&self, pid: usize) -> usize {
        let civics: usize = self.players[pid]
            .civics
            .iter()
            .filter_map(|civic| self.rules.civics.get(civic))
            .map(|spec| spec.governor_title)
            .sum();
        civics
            + self.players[pid]
                .counters
                .get("district_governor_titles")
                .copied()
                .unwrap_or(0)
                .max(0) as usize
    }

    pub fn governor_titles_available(&self, pid: usize) -> usize {
        self.governor_titles(pid)
            .saturating_sub(self.players[pid].governor_titles_spent)
    }

    pub fn available_governor_promotions(&self, pid: usize, governor: &str) -> Vec<Name> {
        if pid >= self.players.len() || self.governor_titles_available(pid) == 0 {
            return Vec::new();
        }
        let Some(state) = self.players[pid].governor_roster.get(governor) else {
            return Vec::new();
        };
        let Some(spec) = self.rules.governors.get(governor) else {
            return Vec::new();
        };
        spec.promotions
            .iter()
            .filter(|(name, promotion)| {
                !state.promotions.contains(*name)
                    && (promotion.requires.is_empty()
                        || promotion
                            .requires
                            .iter()
                            .any(|required| state.promotions.iter().any(|held| *held == *required)))
            })
            .map(|(name, _)| Name::new(name))
            .collect()
    }

    /// The city an established Governor holds, if that Governor is established
    /// at all.
    ///
    /// Deciding a city-state's patron asks this of every major, and war checks
    /// go through that, so it runs hundreds of millions of times in a long
    /// game. Both questions — whether, and where — are answered from one
    /// lookup rather than from two of the same one.
    fn established_governor_at(&self, pid: usize, governor: &str) -> Option<u32> {
        let state = self.players.get(pid)?.governor_roster.get(governor)?;
        let spec = self.rules.governors.get(governor)?;
        let city_id = state.city?;
        let city = self.cities.get(&city_id)?;
        // Capturing or razing a Governor's city displaces that Governor. Amani
        // is the one exception to domestic ownership because she may be
        // assigned to a living city-state. Validate this here as well as at
        // assignment time so old saves with a stale city id cannot reactivate
        // an aura or panic a turn after the city is gone.
        let valid_assignment = city.owner == pid
            || (governor == "amani"
                && self.players[city.owner].is_minor
                && !self.players[city.owner].is_barbarian);
        (valid_assignment
            && self.turn >= state.assigned_turn + self.standard_duration(spec.establish_turns)
            && self.turn >= state.disabled_until)
            .then_some(city_id)
    }

    fn governor_established(&self, pid: usize, governor: &str) -> bool {
        self.established_governor_at(pid, governor).is_some()
    }

    fn established_governor_city(&self, pid: usize, governor: &str) -> Option<u32> {
        self.established_governor_at(pid, governor)
    }

    fn amani_city_state_for_effect(&self, pid: usize, effect: &str) -> Option<(u32, usize)> {
        let city_id = self.established_governor_city(pid, "amani")?;
        let city = self.cities.get(&city_id)?;
        (self.players[city.owner].is_minor
            && !self.players[city.owner].is_barbarian
            && self.governor_effect(pid, city_id, effect) > 0.0)
            .then_some((city_id, city.owner))
    }

    pub(crate) fn governor_effect(&self, pid: usize, cid: u32, effect: &str) -> f64 {
        // Deciding whether a Governor is established walks the roster and the
        // city; no title or promotion granting this makes that moot.
        if !self.rules.effect_index.governors(effect) {
            return 0.0;
        }
        self.players[pid]
            .governor_roster
            .iter()
            .filter(|(governor, state)| {
                state.city == Some(cid) && self.governor_established(pid, governor)
            })
            .map(|(governor, state)| {
                let spec = &self.rules.governors[governor];
                spec.effects.get(effect).copied().unwrap_or(0.0)
                    + state
                        .promotions
                        .iter()
                        .filter_map(|promotion| spec.promotions.get(promotion))
                        .filter_map(|promotion| promotion.effects.get(effect))
                        .sum::<f64>()
            })
            .sum()
    }

    /// Whether completing a Settler in this city consumes one population.
    ///
    /// Production and purchase completion must use the same governor-aware
    /// rule, and AI estimators need that contract when pricing a Settler as a
    /// coupled investment. Keeping the decision here prevents those paths
    /// from drifting apart as governor promotions evolve.
    pub fn settler_consumes_population(&self, pid: usize, cid: u32) -> bool {
        self.governor_effect(pid, cid, "settler_no_population") <= 0.0
    }

    fn sync_governor_cities(&mut self, pid: usize) {
        let mut cities: Vec<u32> = self.players[pid]
            .governor_roster
            .values()
            .filter_map(|state| state.city)
            .filter(|cid| self.cities.get(cid).is_some_and(|city| city.owner == pid))
            .collect();
        cities.sort_unstable();
        cities.dedup();
        self.players[pid].governors = cities;
    }

    fn do_appoint_governor(&mut self, pid: usize, governor: &str, cid: u32) -> Result<(), String> {
        if !self.rules.governors.contains_key(governor)
            || self.players[pid].governor_roster.contains_key(governor)
            || self.governor_titles_available(pid) == 0
        {
            return Err("governor cannot be appointed".into());
        }
        let city = self
            .cities
            .get(&cid)
            .ok_or_else(|| "no such city".to_string())?;
        let foreign_amani = governor == "amani"
            && self.players[city.owner].is_minor
            && !self.is_at_war(pid, city.owner);
        if city.owner != pid && !foreign_amani {
            return Err("governor cannot be assigned to that city".into());
        }
        for state in self.players[pid].governor_roster.values_mut() {
            if state.city == Some(cid) {
                state.city = None;
            }
        }
        self.players[pid].governor_roster.insert(
            governor.to_string(),
            GovernorState {
                city: Some(cid),
                assigned_turn: self.turn,
                disabled_until: 0,
                promotions: BTreeSet::new(),
            },
        );
        self.players[pid].governor_titles_spent += 1;
        self.sync_governor_cities(pid);
        if self.players[pid].governor_roster.len() == self.rules.governors.len() {
            self.first_historic_moment(
                pid,
                "all_governors_appointed",
                Some("MOMENT_GOVERNOR_ALL_APPOINTED_FIRST"),
                None,
            );
        }
        Ok(())
    }

    fn do_reassign_governor(&mut self, pid: usize, governor: &str, cid: u32) -> Result<(), String> {
        let city = self
            .cities
            .get(&cid)
            .ok_or_else(|| "no such city".to_string())?;
        let foreign_amani = governor == "amani"
            && self.players[city.owner].is_minor
            && !self.is_at_war(pid, city.owner);
        if city.owner != pid && !foreign_amani {
            return Err("governor cannot be assigned to that city".into());
        }
        if !self.players[pid].governor_roster.contains_key(governor) {
            return Err("governor has not been appointed".into());
        }
        for (name, state) in self.players[pid].governor_roster.iter_mut() {
            if name != governor && state.city == Some(cid) {
                state.city = None;
            }
        }
        let state = self.players[pid].governor_roster.get_mut(governor).unwrap();
        if state.city == Some(cid) {
            return Err("governor is already assigned there".into());
        }
        state.city = Some(cid);
        state.assigned_turn = self.turn;
        state.disabled_until = 0;
        self.sync_governor_cities(pid);
        Ok(())
    }

    fn do_promote_governor(
        &mut self,
        pid: usize,
        governor: &str,
        promotion: &str,
    ) -> Result<(), String> {
        if self.governor_titles_available(pid) == 0 {
            return Err("no free Governor Titles".into());
        }
        let spec = self
            .rules
            .governors
            .get(governor)
            .and_then(|governor| governor.promotions.get(promotion))
            .ok_or_else(|| "no such governor promotion".to_string())?;
        let state = self.players[pid]
            .governor_roster
            .get(governor)
            .ok_or_else(|| "governor has not been appointed".to_string())?;
        if state.promotions.contains(promotion)
            || (!spec.requires.is_empty()
                && !spec
                    .requires
                    .iter()
                    .any(|required| state.promotions.iter().any(|held| *held == *required)))
        {
            return Err("governor promotion unavailable".into());
        }
        self.players[pid]
            .governor_roster
            .get_mut(governor)
            .unwrap()
            .promotions
            .insert(promotion.to_string());
        self.players[pid].governor_titles_spent += 1;
        let fully_promoted = self.players[pid]
            .governor_roster
            .get(governor)
            .is_some_and(|state| {
                self.rules.governors[governor]
                    .promotions
                    .keys()
                    .all(|promotion| state.promotions.contains(promotion))
            });
        if fully_promoted {
            self.first_historic_moment(
                pid,
                "governor_fully_promoted",
                Some("MOMENT_GOVERNOR_FULLY_PROMOTED_FIRST"),
                None,
            );
        }
        Ok(())
    }

    fn do_assign_governor(&mut self, pid: usize, cid: u32) -> Result<(), String> {
        let governor = self
            .rules
            .governors
            .keys()
            .find(|governor| !self.players[pid].governor_roster.contains_key(governor))
            .cloned()
            .ok_or_else(|| "all governors have been appointed".to_string())?;
        self.do_appoint_governor(pid, governor.as_str(), cid)
    }

    pub(crate) fn emergency_war_pair(&self, first: usize, second: usize) -> bool {
        self.active_emergencies.iter().any(|emergency| {
            emergency.ends > self.turn
                && ((emergency.target == first && emergency.members.contains(&second))
                    || (emergency.target == second && emergency.members.contains(&first)))
        })
    }

    fn emergency_coalition_pair(&self, first: usize, second: usize) -> bool {
        self.active_emergencies.iter().any(|emergency| {
            emergency.ends > self.turn
                && emergency.members.contains(&first)
                && emergency.members.contains(&second)
        })
    }

    pub(crate) fn emergency_objective(&self, member: usize) -> Option<&Emergency> {
        self.active_emergencies
            .iter()
            .filter(|emergency| {
                emergency.ends > self.turn
                    && emergency.members.contains(&member)
                    && self
                        .cities
                        .get(&emergency.city)
                        .is_some_and(|city| city.owner == emergency.target)
            })
            .min_by_key(|emergency| (emergency.ends, emergency.id))
    }

    /// Seat the oldest still-valid queued Emergency, if a Special Session is
    /// allowed to open at all. Shipped
    /// `WORLD_CONGRESS_MIN_TIME_BETWEEN_SPECIAL_SESSIONS` is 15, so Sessions
    /// cannot chain: a queued proposal simply waits its turn rather than
    /// being dropped, because the crisis has not gone away.
    fn convene_pending_emergency(&mut self) {
        if self.last_special_session > 0
            && self.turn < self.last_special_session + self.standard_duration(15)
        {
            return;
        }
        while self.congress.is_none() && !self.pending_emergencies.is_empty() {
            let mut proposal = self.pending_emergencies[0].clone();
            proposal.eligible.retain(|player| {
                self.players.get(*player).is_some_and(|candidate| {
                    candidate.alive && !candidate.is_minor && !candidate.is_barbarian
                })
            });
            let valid = !proposal.eligible.is_empty()
                && self
                    .players
                    .get(proposal.target)
                    .is_some_and(|target| target.alive && !target.is_minor && !target.is_barbarian)
                && self
                    .cities
                    .get(&proposal.city)
                    .is_some_and(|city| city.owner == proposal.target);
            if !valid {
                self.pending_emergencies.remove(0);
                continue;
            }
            self.pending_emergencies[0].eligible = proposal.eligible;
            self.last_special_session = self.turn;
            let title = if proposal.kind == "city_state" {
                "City-State Emergency"
            } else {
                "Military Emergency"
            };
            self.congress = Some(CongressSession {
                convened: self.turn,
                closes: self.turn + self.standard_duration(5),
                resolutions: vec![CongressResolution {
                    id: Self::emergency_resolution_id(proposal.id),
                    title: title.to_string(),
                    choices: vec!["A:support".to_string(), "B:oppose".to_string()],
                    ballots: BTreeMap::new(),
                }],
            });
        }
    }

    fn request_city_capture_emergency(&mut self, target: usize, city: u32, defeated: usize) {
        if self.is_arena()
            || self.world_era < 2
            || self.players[target].is_minor
            || self.players[target].is_barbarian
            || self
                .pending_emergencies
                .iter()
                .any(|proposal| proposal.city == city)
            || self
                .active_emergencies
                .iter()
                .any(|emergency| emergency.city == city)
        {
            return;
        }

        let city_state = self.players[defeated].is_minor && !self.players[defeated].is_barbarian;
        if !city_state {
            let leading_score = self
                .players
                .iter()
                .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
                .map(|player| self.score(player.id))
                .max();
            if leading_score != Some(self.score(target)) {
                return;
            }
        }

        let eligible: BTreeSet<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.id != target
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && !self.are_friends(player.id, target)
                    && !self.are_allied(player.id, target)
            })
            .filter(|player| {
                !city_state
                    || player
                        .envoys
                        .iter()
                        .any(|(minor, count)| *minor == defeated && *count > 0)
            })
            .map(|player| player.id)
            .collect();
        if eligible.is_empty() {
            return;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.pending_emergencies.push(EmergencyProposal {
            id,
            kind: if city_state { "city_state" } else { "military" }.to_string(),
            target,
            city,
            original_owner: self.cities[&city].original_owner,
            eligible,
            requested: self.turn,
        });
        if self.congress.is_none() {
            self.convene_pending_emergency();
        }
    }

    fn record_emergency_presence(&mut self, member: usize) {
        let points: Vec<(u32, i64)> = self
            .active_emergencies
            .iter()
            .filter(|emergency| emergency.members.contains(&member))
            .map(|emergency| {
                let count = self
                    .units
                    .values()
                    .filter(|unit| {
                        unit.owner == member
                            && self.rules.units[unit.kind].class == "military"
                            && self.map.tiles[&unit.pos]
                                .owner_city
                                .and_then(|city| self.cities.get(&city))
                                .is_some_and(|city| city.owner == emergency.target)
                    })
                    .count() as i64;
                (emergency.id, count)
            })
            .collect();
        for (id, points) in points {
            if points > 0 {
                let emergency = self
                    .active_emergencies
                    .iter_mut()
                    .find(|emergency| emergency.id == id)
                    .unwrap();
                *emergency.contributions.entry(member).or_insert(0) += points;
            }
        }
    }

    fn resolve_emergency(&mut self, id: u32, success: bool) {
        let Some(index) = self
            .active_emergencies
            .iter()
            .position(|emergency| emergency.id == id)
        else {
            return;
        };
        let emergency = self.active_emergencies.remove(index);
        if success {
            for member in emergency.members.iter().filter(|member| {
                emergency
                    .contributions
                    .get(member)
                    .is_some_and(|points| *points > 0)
            }) {
                self.players[*member].diplomatic_favor += 100.0;
                let key = if emergency.kind == "city_state" {
                    "emergency_gold_per_envoy".to_string()
                } else {
                    format!("emergency_heal_in:{}", emergency.target)
                };
                *self.players[*member].counters.entry(key).or_insert(0) +=
                    if emergency.kind == "city_state" { 1 } else { 5 };
                self.add_historic_moment(*member, "MOMENT_EMERGENCY_WON_AS_MEMBER");
            }
        } else {
            self.players[emergency.target].diplomatic_favor += 200.0;
            self.add_historic_moment(emergency.target, "MOMENT_EMERGENCY_WON_AS_TARGET");
            if emergency.kind == "city_state" {
                *self.players[emergency.target]
                    .counters
                    .entry("emergency_city_state_route_gold".to_string())
                    .or_insert(0) += 2;
            } else {
                for member in emergency.members {
                    *self.players[emergency.target]
                        .counters
                        .entry(format!("emergency_city_strike_vs:{member}"))
                        .or_insert(0) += 2;
                }
            }
        }
    }

    fn process_emergencies(&mut self) {
        // No Congress, no Emergencies: a battlefield has nobody to convene.
        if self.is_arena() {
            return;
        }
        self.share_emergency_visibility();
        let expired: Vec<(u32, bool)> = self
            .active_emergencies
            .iter()
            .filter_map(|emergency| {
                (self.turn >= emergency.ends || !self.cities.contains_key(&emergency.city))
                    .then_some((emergency.id, false))
            })
            .collect();
        for (id, success) in expired {
            self.resolve_emergency(id, success);
        }
    }

    // -------------------------------------------------- eras & tourism

    /// The `MOMENT_CITY_SIZE_*_FIRST` ladder: the first time any city of this
    /// civilization reaches 10, 15, 20 and 25 Population, +1 each, or +2 as the
    /// `_FIRST_IN_WORLD` variant. The four thresholds are the ones the shipped
    /// moment descriptions name.
    fn note_city_size_moment(&mut self, pid: usize, pop: i64) {
        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return;
        }
        let Some(size) = [
            (10, "small"),
            (15, "medium"),
            (20, "large"),
            (25, "extra_large"),
        ]
        .into_iter()
        .find(|(threshold, _)| *threshold == pop)
        .map(|(_, name)| name) else {
            return;
        };
        let key = format!("city_size:{size}");
        let (ordinary, world_first) = match size {
            "small" => (
                "MOMENT_CITY_SIZE_SMALL_FIRST",
                "MOMENT_CITY_SIZE_SMALL_FIRST_IN_WORLD",
            ),
            "medium" => (
                "MOMENT_CITY_SIZE_MEDIUM_FIRST",
                "MOMENT_CITY_SIZE_MEDIUM_FIRST_IN_WORLD",
            ),
            "large" => (
                "MOMENT_CITY_SIZE_LARGE_FIRST",
                "MOMENT_CITY_SIZE_LARGE_FIRST_IN_WORLD",
            ),
            "extra_large" => (
                "MOMENT_CITY_SIZE_EXTRA_LARGE_FIRST",
                "MOMENT_CITY_SIZE_EXTRA_LARGE_FIRST_IN_WORLD",
            ),
            _ => unreachable!(),
        };
        self.first_historic_moment(pid, &key, Some(ordinary), Some(world_first));
    }

    /// All non-growth population grants come through this boundary so a hut
    /// or Angkor Wat cannot skip a city-size threshold.
    fn increase_city_population(&mut self, cid: u32, amount: i32) {
        if amount <= 0 || !self.cities.contains_key(&cid) {
            return;
        }
        let pid = self.cities[&cid].owner;
        for _ in 0..amount {
            let pop = {
                let city = self.cities.get_mut(&cid).unwrap();
                city.pop += 1;
                city.pop as i64
            };
            self.apply_growth_pressure(cid);
            self.note_city_size_moment(pid, pop);
        }
    }

    /// `MOMENT_GOVERNMENT_ENACTED_TIER_N_FIRST`, +2, or +3 as
    /// `_FIRST_IN_WORLD`. Tiers follow the shipped unlock civics: Political
    /// Philosophy opens tier 1, Divine Right / Exploration / Reformed Church
    /// tier 2, the Modern trio tier 3, and the Information trio tier 4.
    fn note_government_tier_moment(&mut self, pid: usize, government: &str) {
        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return;
        }
        let tier = match government {
            "autocracy" | "oligarchy" | "classical_republic" => 1,
            "monarchy" | "merchant_republic" | "theocracy" => 2,
            "communism" | "democracy" | "fascism" => 3,
            "corporate_libertarianism" | "digital_democracy" | "synthetic_technocracy" => 4,
            // Chiefdom is the starting government and names no moment.
            _ => return,
        };
        let key = format!("government_tier:{tier}");
        let (ordinary, world_first) = match tier {
            1 => (
                "MOMENT_GOVERNMENT_ENACTED_TIER_1_FIRST",
                "MOMENT_GOVERNMENT_ENACTED_TIER_1_FIRST_IN_WORLD",
            ),
            2 => (
                "MOMENT_GOVERNMENT_ENACTED_TIER_2_FIRST",
                "MOMENT_GOVERNMENT_ENACTED_TIER_2_FIRST_IN_WORLD",
            ),
            3 => (
                "MOMENT_GOVERNMENT_ENACTED_TIER_3_FIRST",
                "MOMENT_GOVERNMENT_ENACTED_TIER_3_FIRST_IN_WORLD",
            ),
            4 => (
                "MOMENT_GOVERNMENT_ENACTED_TIER_4_FIRST",
                "MOMENT_GOVERNMENT_ENACTED_TIER_4_FIRST_IN_WORLD",
            ),
            _ => unreachable!(),
        };
        self.first_historic_moment(pid, &key, Some(ordinary), Some(world_first));
    }

    /// `MOMENT_TECH_RESEARCHED_IN_ERA_FIRST` (+1) and
    /// `MOMENT_CIVIC_CULTURVATED_IN_ERA_FIRST` (+1), each doubled to +2 by their
    /// `_IN_WORLD` variant: the first node a civilization finishes from each era
    /// of each tree, once per era per tree.
    ///
    /// These two are among the highest-frequency Historic Moments in the
    /// shipped game — every civilization earns up to eight of each over a full
    /// game — and CIVVIS awarded neither, which is a large part of why 79% of
    /// its age transitions were Dark.
    fn note_era_first_tree_node(&mut self, pid: usize, technology: bool, node: &str) {
        // Only a major banks Era Score, and only a major may hold the
        // first-in-world claim — otherwise a city-state researching ahead
        // silently downgrades every major's moment from +2 to +1.
        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return;
        }
        let era = if technology {
            self.rules.techs.get(node).map(|spec| spec.era)
        } else {
            self.rules.civics.get(node).map(|spec| spec.era)
        };
        let Some(era) = era else {
            return;
        };
        let tree = if technology { "tech" } else { "civic" };
        let key = format!("era_first:{tree}:{era}");
        let (ordinary, world_first) = if technology {
            (
                "MOMENT_TECH_RESEARCHED_IN_ERA_FIRST",
                "MOMENT_TECH_RESEARCHED_IN_ERA_FIRST_IN_WORLD",
            )
        } else {
            (
                "MOMENT_CIVIC_CULTURVATED_IN_ERA_FIRST",
                "MOMENT_CIVIC_CULTURVATED_IN_ERA_FIRST_IN_WORLD",
            )
        };
        self.first_historic_moment(pid, &key, Some(ordinary), Some(world_first));
    }

    /// Which Normal-Age Dedication triggers a finished building pays out.
    /// Civ VI names three building shapes, and the first two are named the
    /// same way as each other: PER_CULTURE_BUILDING_CONSTRUCTED and
    /// PER_SCIENCE_BUILDING_CONSTRUCTED, which is a building that yields that
    /// yield. The third is any building of the Industrial era or later.
    fn note_dedicated_building(&mut self, pid: usize, building: &str, spec: &BuildingSpec) {
        if spec.wonder {
            return;
        }
        if spec.yields.culture > 0.0 {
            self.dedication_trigger(pid, "culture_building", 1);
        }
        if spec.yields.science > 0.0 {
            self.dedication_trigger(pid, "science_building", 1);
        }
        let era = spec
            .tech
            .as_deref()
            .and_then(|tech| self.rules.techs.get(tech))
            .map(|tech| tech.era)
            .or_else(|| {
                spec.civic
                    .as_deref()
                    .and_then(|civic| self.rules.civics.get(civic))
                    .map(|civic| civic.era)
            })
            .unwrap_or(0);
        if era >= INDUSTRIAL_ERA {
            self.dedication_trigger(pid, "industrial_building", 1);
        }
        let _ = building;
    }

    /// On a world-era transition, each civilization receives dynamic Normal
    /// and Golden thresholds based on era and empire size. Climbing directly
    /// from a Dark Age to the Golden threshold produces a Heroic Age and
    /// three simultaneous dedication choices.
    /// Era Score needed to avoid a Dark Age, from the shipped
    /// `THRESHOLD_SHIFT_*` parameters: `_PER_CITY` 1, including the first
    /// city, `_PER_PAST_DARK_AGE` -5 and
    /// `_PER_PAST_GOLDEN_AGE` +5. The last two make ages self-correcting — a
    /// civilization that has struggled finds Normal easier to reach next time,
    /// one that has prospered finds it harder.
    ///
    /// `_PER_ANARCHY`, `_PER_INCOMPLETE_ERA_TECH`/`_CIVIC`, `_PER_INCOMPLETE_OLD_TECH`/`_CIVIC`
    /// and `_PER_MISSING_AMENITY` all ship as 0, so there is nothing to model
    /// for them.
    fn normal_age_threshold(era: usize, cities: i64, past_dark: i64, past_golden: i64) -> i64 {
        14 + if era == 0 { -3 } else { 0 } + cities - 5 * past_dark + 5 * past_golden
    }

    fn process_eras(&mut self) {
        if self.world_era + 1 >= ERA_NAMES.len() {
            return;
        }
        let progress_era = self.era_from_progress();
        let minimum = self.game_speed.scale_turns(ERA_MINIMUM_TURNS);
        let maximum = self.game_speed.scale_turns(ERA_MAXIMUM_TURNS);
        let countdown = self.game_speed.scale_turns(NEXT_ERA_COUNTDOWN_TURNS);
        if self.world_era_countdown_end.is_none() {
            let minimum_end = self.world_era_since.saturating_add(minimum);
            let maximum_end = self.world_era_since.saturating_add(maximum);
            if progress_era > self.world_era {
                self.world_era_countdown_end = Some(
                    self.turn
                        .saturating_add(countdown)
                        .clamp(minimum_end, maximum_end),
                );
            } else if self.turn.saturating_add(countdown) >= maximum_end {
                self.world_era_countdown_end = Some(maximum_end);
            }
        }
        let Some(end) = self.world_era_countdown_end else {
            return;
        };
        if self.turn < end {
            return;
        }
        let previous_world_era = self.world_era;
        let era = self.world_era + 1;
        self.world_era = era;
        self.world_era_since = self.turn;
        self.world_era_countdown_end = None;
        let majors: Vec<usize> = self
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let era_name = ERA_NAMES.get(era).copied().unwrap_or("a new");
        for pid in majors.clone() {
            self.note(pid, "General", format!("entered the {era_name} era"), None);
        }
        // Above Prince every AI civilization enters the new era holding a
        // handful of free Eurekas and Inspirations.
        let era_boosts = self.difficulty_spec().ai_era_boosts;
        if era_boosts > 0 {
            for pid in majors.clone() {
                if self.is_human_seat(pid) || self.is_handicap_exempt(pid) {
                    continue;
                }
                self.grant_random_boosts(pid, era_boosts, true);
                self.grant_random_boosts(pid, era_boosts, false);
            }
        }
        for pid in majors.iter().copied() {
            let cities = self.player_city_ids(pid).len().max(1) as i64;
            let player = &mut self.players[pid];
            let normal = if player.normal_age_threshold > 0 {
                player.normal_age_threshold
            } else {
                Self::normal_age_threshold(
                    previous_world_era,
                    cities,
                    player.past_dark_ages,
                    player.past_golden_ages,
                )
            };
            let golden = if player.golden_age_threshold > normal {
                player.golden_age_threshold
            } else {
                Self::golden_age_threshold(
                    previous_world_era,
                    cities,
                    player.past_dark_ages,
                    player.past_golden_ages,
                )
            };
            let previous = player.age.clone();
            player.age = if player.era_score >= golden && previous == "dark" {
                "heroic".to_string()
            } else if player.era_score >= golden {
                "golden".to_string()
            } else if player.era_score < normal {
                "dark".to_string()
            } else {
                "normal".to_string()
            };
            player.dedications.clear();
            player.dedication_choices = if player.age == "heroic" { 3 } else { 1 };
            player.era_score = 0;
            player.era_score_baseline = 0;
            // The era that just ended becomes the evidence for the dedication
            // about to be chosen, and the new one starts its own tally.
            player.last_era_triggers = std::mem::take(&mut player.era_triggers);
            // The age just entered counts toward the next threshold: shipped
            // THRESHOLD_SHIFT_PER_PAST_DARK_AGE -5 and
            // _PER_PAST_GOLDEN_AGE +5 make ages self-correcting, so a
            // civilization that has struggled finds Normal easier to reach
            // next time and one that has prospered finds it harder. Heroic
            // counts as the Golden Age it is.
            match player.age.as_str() {
                "dark" => player.past_dark_ages += 1,
                "golden" | "heroic" => player.past_golden_ages += 1,
                _ => {}
            }
            player.normal_age_threshold = Self::normal_age_threshold(
                era,
                cities,
                player.past_dark_ages,
                player.past_golden_ages,
            );
            player.golden_age_threshold = Self::golden_age_threshold(
                era,
                cities,
                player.past_dark_ages,
                player.past_golden_ages,
            );
        }
        // A Dark Age card is a loan against the age that offered it. Climbing
        // out of the Dark Age — or simply leaving the eras that card belongs
        // to — takes it back out of its Wildcard slot.
        let era = self.world_era;
        for pid in 0..self.players.len() {
            let age = self.players[pid].age.clone();
            let expired: Vec<Name> = self.players[pid]
                .policies
                .iter()
                .filter(|policy| {
                    self.rules
                        .policies
                        .get(policy.as_str())
                        .is_some_and(|spec| !spec.offered(&age, era))
                })
                .cloned()
                .collect();
            for policy in expired {
                self.players[pid]
                    .policies
                    .retain(|slotted| *slotted != policy);
            }
        }
    }

    /// Which worlds this civilization has found, as indices into
    /// [`EXOPLANET_TARGETS`].
    ///
    /// The neighbourhood is the same for everybody — it is a fact about the
    /// sky, not about a civilization — so the *order* worlds are found in is
    /// one shuffle drawn from the game's own seed and shared by every player.
    /// What differs is how far down that order each of them has got, which is
    /// what their instruments above the air have bought them.
    pub fn exoplanet_survey(&self, pid: usize) -> Vec<usize> {
        let depth = EXOPLANET_SURVEY
            .iter()
            .filter(|(project, _)| self.players[pid].science_projects.contains(*project))
            .map(|(_, found)| *found)
            .sum::<usize>()
            + self.exoplanet_laser_stations(pid);
        if depth == 0 {
            return Vec::new();
        }
        let mut order: Vec<usize> = (0..EXOPLANET_TARGETS.len()).collect();
        // A Fisher-Yates shuffle off the map seed, so a given world is found in
        // a given order every time that world is played and in a different one
        // in the next. Drawn with the same splitmix the rest of the engine
        // uses rather than the live RNG, because reading the live stream here
        // would make every later roll in the game depend on how many space
        // projects happen to have finished.
        let mut state = self.seed ^ 0x9E37_79B9_7F4A_7C15;
        for index in (1..order.len()).rev() {
            state = state
                .wrapping_add(0x9E37_79B9_7F4A_7C15)
                .rotate_left(31)
                .wrapping_mul(0xBF58_476D_1CE4_E5B9);
            let pick = (state >> 33) as usize % (index + 1);
            order.swap(index, pick);
        }
        order.truncate(depth.min(EXOPLANET_TARGETS.len()));
        order
    }

    fn exoplanet_laser_stations(&self, pid: usize) -> usize {
        let counters = &self.players[pid].counters;
        (counters
            .get("project:lagrange_laser_station")
            .copied()
            .unwrap_or(0)
            + counters
                .get("project:terrestrial_laser_station")
                .copied()
                .unwrap_or(0))
        .max(0) as usize
    }

    /// The world the expedition sets out for, decided the moment it launches.
    ///
    /// The best world found, and the nearest of those if two are graded alike.
    /// It aims for the *best* rather than the nearest because a light-year
    /// currently costs nothing — see [`Game::exoplanet_destination`], which is the
    /// whole trip whichever world it is — so there is nothing to trade a worse
    /// world for. A civilization that surveyed nothing takes the one anybody
    /// would already have heard of.
    pub fn exoplanet_choice(&self, pid: usize) -> &'static str {
        self.exoplanet_survey(pid)
            .into_iter()
            .map(|index| &EXOPLANET_TARGETS[index])
            .max_by(|a, b| {
                a.grade
                    .cmp(&b.grade)
                    .then(b.light_years.total_cmp(&a.light_years))
            })
            .map(|target| target.id)
            .unwrap_or(EXOPLANET_DEFAULT_TARGET)
    }

    /// The world the expedition is going to, or would go to if it left now.
    pub fn exoplanet_target(&self, pid: usize) -> &'static ExoplanetTarget {
        let chosen = self.players[pid]
            .exoplanet_target
            .as_deref()
            .unwrap_or_else(|| self.exoplanet_choice(pid));
        EXOPLANET_TARGETS
            .iter()
            .find(|target| target.id == chosen)
            // A save naming a world this build does not have is a save from a
            // future roster, not a corrupt one. Fall back rather than panic.
            .unwrap_or_else(|| {
                EXOPLANET_TARGETS
                    .iter()
                    .find(|target| target.id == EXOPLANET_DEFAULT_TARGET)
                    .expect("the default target is in the roster")
            })
    }

    /// The native simulator's laser-station model. It remains separate from
    /// the host reading below because advancing a speculative game must never
    /// mutate it using a snapshot's real-world measurement.
    fn modeled_exoplanet_speed(&self, pid: usize) -> f64 {
        let p = &self.players[pid];
        if !p.science_projects.contains("exoplanet_expedition") {
            return 0.0;
        }
        1.0 + p
            .counters
            .get("project:lagrange_laser_station")
            .copied()
            .unwrap_or(0) as f64
            + p.counters
                .get("project:terrestrial_laser_station")
                .copied()
                .unwrap_or(0) as f64
    }

    fn advance_exoplanet(&mut self, pid: usize) {
        if !self.victory_eligible(pid)
            || !self.players[pid]
                .science_projects
                .contains("exoplanet_expedition")
        {
            return;
        }
        let destination = self.science_victory_points_needed(pid);
        self.players[pid].exoplanet_distance = (self.players[pid].exoplanet_distance
            + self.modeled_exoplanet_speed(pid))
        .min(destination);
        if self.players[pid].exoplanet_distance >= destination {
            self.set_winner(pid, "science");
        }
    }

    /// Reconstruct pressure for an older save that predates the per-rival
    /// ledger. The value is frozen into `tourism_pressure` the next time this
    /// source generates Tourism against the target.
    fn legacy_tourism_pressure(&self, source: usize, target: usize) -> f64 {
        let player = &self.players[source];
        let religious = player
            .religious_tourism_lifetime
            .clamp(0.0, player.tourism_lifetime.max(0.0));
        let targeted_total = player.targeted_tourism.values().copied().sum::<f64>();
        let secular = (player.tourism_lifetime - religious - targeted_total).max(0.0);
        let targeted = player.targeted_tourism.get(&target).copied().unwrap_or(0.0);
        (secular + targeted) * self.international_tourism_multiplier(source, target, false)
            + religious * self.international_tourism_multiplier(source, target, true)
    }

    fn add_passive_tourism(
        &mut self,
        pid: usize,
        tourism: f64,
        religious: f64,
        film_studio_tourism: f64,
        film_studio_religious: f64,
    ) {
        // A per-turn total is a sum of percentage-scaled sources and can come
        // out a hair below zero (-7e-5 on a mirrored board), and `clamp`
        // panics whenever its upper bound is below its lower one.
        let tourism = tourism.max(0.0);
        let religious = religious.clamp(0.0, tourism);
        let pressure: Vec<(usize, f64)> = self
            .players
            .iter()
            .filter(|target| {
                target.id != pid
                    && !self.same_team(pid, target.id)
                    && target.alive
                    && !target.is_minor
                    && !target.is_barbarian
            })
            .map(|target| {
                let modern_target = self.player_era(target.id) >= 5;
                let target_tourism = tourism
                    + if modern_target {
                        film_studio_tourism
                    } else {
                        0.0
                    };
                let target_religious = (religious
                    + if modern_target {
                        film_studio_religious
                    } else {
                        0.0
                    })
                .clamp(0.0, target_tourism.max(0.0));
                let target_secular = (target_tourism - target_religious).max(0.0);
                let added = target_secular
                    * self.international_tourism_multiplier(pid, target.id, false)
                    + target_religious
                        * self.international_tourism_multiplier(pid, target.id, true);
                (
                    target.id,
                    self.tourism_pressure_against(pid, target.id) + added,
                )
            })
            .collect();
        let player = &mut self.players[pid];
        player.tourism_lifetime += tourism;
        player.religious_tourism_lifetime += religious;
        for (target, total) in pressure {
            player.tourism_pressure.insert(target, total);
        }
    }

    fn do_end_turn(&mut self) {
        if self.planning_role == PlanningRole::Seat {
            // A seat's private planning world is discarded the moment its
            // plan is harvested, and the harvest reads only the actions
            // logged before this close. All that must still happen is the
            // cursor movement the planning agent's own turn loop observes.
            let n = self.players.len();
            for i in 1..=n {
                let cand = (self.current + i) % n;
                if self.players[cand].alive {
                    if cand != self.current {
                        let wrapped = cand <= self.current;
                        self.current = cand;
                        if wrapped {
                            self.turn += 1;
                        }
                    }
                    return;
                }
            }
            return;
        }
        // Strength is part of the live war ledger, not just its opening and
        // closing snapshots. Refresh after every civilization has completed
        // its actions so the next observation sees the latest total and peak.
        self.sync_war_log();
        let ending = self.current;
        let n = self.players.len();
        let mut nxt = None;
        for i in 1..=n {
            let cand = (self.current + i) % n;
            if self.players[cand].alive {
                nxt = Some(cand);
                break;
            }
        }
        let nxt = match nxt {
            Some(x) if x != self.current => x,
            _ => return,
        };
        let wrapped = nxt <= self.current;
        // This is a player-turn boundary, not a world-turn boundary. Ground
        // that fell out of sight while `ending` was acting now becomes normal
        // remembered terrain; `nxt` starts with a fresh union that the
        // post-action visibility refresh will seed from its current sight.
        self.players[ending].turn_visible.clear();
        self.players[ending].turn_units.clear();
        self.players[nxt].turn_visible.clear();
        self.players[nxt].turn_units.clear();
        self.current = nxt;
        if wrapped {
            if self.planning_role == PlanningRole::Rolling {
                // The prepare walk's loop condition is `turn == cycle_turn`:
                // wrapping out of the cycle is the walk's exit, and the
                // rolling world is discarded on it. The world systems, the
                // next turn's upkeep, and the score check below would be
                // computed for a world nothing will ever read again.
                self.turn += 1;
                return;
            }
            self.turn += 1;
            self.process_climate();
            self.process_disasters();
            self.process_meteors();
            self.barbarian_phase();
            self.process_eras();
            self.process_agendas();
            self.process_emergencies();
            self.process_congress();
            // A second refresh catches world-turn systems such as Emergencies
            // and city-state suzerainty changes that run between player turns.
            self.sync_war_log();
            self.check_culture_victory();
            // A Civ world's clock awards its score tiebreak. A Tactics clock
            // is deliberately different: if both sides are still alive at
            // the selected deadline, neither side won the battle.
            if self.turn_limit().is_some_and(|limit| self.turn > limit) && !self.is_finished() {
                if self.is_arena() {
                    self.declare_draw();
                } else {
                    // Ties resolve through the shipped chain (civics, cities,
                    // districts, Population, Great People, religion,
                    // technologies, wonders) before falling back to seat order.
                    let mut best: Option<((i64, [i64; 9]), i64)> = None;
                    let mut best_pid = 0;
                    let mut seen_teams = BTreeSet::new();
                    for pl in &self.players {
                        if pl.alive && !pl.is_minor && !pl.is_barbarian {
                            if pl.team.is_some_and(|team| !seen_teams.insert(team)) {
                                continue;
                            }
                            let representative = self
                                .team_members(pl.id)
                                .into_iter()
                                .filter(|member| self.players[*member].alive)
                                .min()
                                .unwrap_or(pl.id);
                            let key = (
                                self.team_score_rank_key(representative),
                                -(representative as i64),
                            );
                            if best.is_none() || key > best.unwrap() {
                                best = Some(key);
                                best_pid = representative;
                            }
                        }
                    }
                    if best.is_none() {
                        for pl in &self.players {
                            let key = (self.score_rank_key(pl.id), -(pl.id as i64));
                            if best.is_none() || key > best.unwrap() {
                                best = Some(key);
                                best_pid = pl.id;
                            }
                        }
                    }
                    // The count is taken on the wrap out of the final turn;
                    // `reported_turn` dates the result on the turn the limit
                    // names rather than on that wrap.
                    self.set_winner(best_pid, "score");
                    // Require-N at the limit: the banked score type may not have
                    // completed anybody's set, but the clock still ends the
                    // world. The seat holding the most banked types wins — the
                    // cap scorer breaks ties, having just banked score through
                    // the shipped chain — and the award keeps the turn limit's
                    // own victory type.
                    if !self.is_finished() && self.effective_required_victories() > 1 {
                        let most_banked = self
                            .players
                            .iter()
                            .filter(|pl| pl.alive && !pl.is_minor && !pl.is_barbarian)
                            .max_by_key(|pl| {
                                (
                                    self.victories_won.get(&pl.id).map_or(0, BTreeSet::len),
                                    pl.id == best_pid,
                                    std::cmp::Reverse(pl.id),
                                )
                            })
                            .map(|pl| pl.id);
                        if let Some(leader) = most_banked {
                            self.crown(leader, "score");
                        }
                    }
                }
            }
            // The Mercy Rule: end the game the moment the board is beyond
            // reasonable doubt. The threshold came from the setup screen; the
            // model is the calibrated live win share the spectator ribbon
            // shows, on flat priors and soft-float transcendentals so every
            // platform crosses on the same turn (docs/ADJUDICATION.md holds
            // the measured agreement ladder). Checked after the real victory
            // sweeps and the turn-limit award, inside the same wrap, so mercy
            // can never outrank a victory the rules just recognised.
            if !self.is_finished() {
                if let Some(threshold) = self.mercy_rule {
                    if let Some(leader) = crate::odds::mercy_leader(self, threshold) {
                        // The lanes go on before the crown, not after it: the
                        // verdict note and the finish screen read the
                        // composed label, and the board
                        // they are describing is the one standing at this
                        // crossing. Cleared again if the crown is refused —
                        // a play-on extension can decline this very result —
                        // so no unrelated later ending inherits them.
                        self.mercy_lanes = self.leading_victory_lanes(leader);
                        if !self.set_winner(leader, MERCY_VICTORY) {
                            self.mercy_lanes.clear();
                        }
                    }
                }
            }
        }
        if !self.is_finished() {
            self.begin_turn(self.current);
            // Production, healing, levies, policy effects, and governors can
            // alter a live total at the beginning of the next seat's turn.
            self.sync_war_log();
        }
    }

    // ------------------------------------------------------- turn engine

    /// One owned, unpillaged tile per city satisfying the predicate.
    fn owned_tiles_where(&self, pid: usize, want: impl Fn(&crate::world::Tile) -> bool) -> i64 {
        self.cities
            .values()
            .filter(|c| c.owner == pid)
            .flat_map(|c| c.owned_tiles.iter())
            .filter(|pos| {
                self.map
                    .get(**pos)
                    .is_some_and(|tile| !tile.pillaged && want(tile))
            })
            .count() as i64
    }

    fn record_kill(&mut self, pid: usize, weapon: Option<&str>, victim: &Unit) {
        bump(&mut self.players[pid], "kills");
        if let Some(kind) = weapon {
            *self.players[pid]
                .counters
                .entry(format!("kill_with:{kind}"))
                .or_insert(0) += 1;
        }
        *self.players[pid]
            .counters
            .entry(format!("kill_kind:{}", victim.kind))
            .or_insert(0) += 1;
        // The loss side of the same ledger. Only `kills` and `barbs_killed`
        // existed, so a game could say what an empire took from the
        // barbarians but never what the barbarians took from it. Free Cities
        // also carry `is_barbarian`; a revolt's garrison is not a raider, so
        // it is excluded here.
        if self.players[pid].is_barbarian && !self.players[pid].is_free_city {
            bump(&mut self.players[victim.owner], "lost_to_barbarians");
        }
        if self.players[victim.owner].is_barbarian {
            bump(&mut self.players[pid], "barbs_killed");
        } else {
            // Every Dedication that pays for kills excludes Barbarians.
            if self.rules.units[victim.kind].domain.as_deref() == Some("sea") {
                self.dedication_trigger(pid, "naval_kill", 1);
            }
            match victim.formation {
                1 => self.dedication_trigger(pid, "corps_kill", 1),
                2 => self.dedication_trigger(pid, "army_kill", 1),
                _ => {}
            }
            if weapon == Some("giant_death_robot") {
                self.dedication_trigger(pid, "robot_kill", 1);
            }
        }
        if !self.players[victim.owner].is_barbarian
            && self.grants_city_state_unique_bonus(pid, "Wolin")
        {
            let point_kind = weapon.and_then(|kind| {
                self.rules
                    .units
                    .get(kind)
                    .and_then(|spec| wolin_great_person_class(kind, spec))
            });
            if let Some(point_kind) = point_kind {
                *self.players[pid]
                    .gpp
                    .entry(point_kind.to_string())
                    .or_insert(0.0) += self
                    .game_speed
                    .scale(self.rules.units[victim.kind].strength * 0.25);
            }
        }
        if !self.players[victim.owner].is_barbarian && self.has_ability(pid, "taxis") {
            if let Some(religion) = self.players[pid].religion.clone() {
                let pressure = self.civ_effect(pid, "unit_kill_religious_pressure");
                let targets: Vec<u32> = self
                    .cities
                    .values()
                    .filter(|city| self.wdist(city.pos, victim.pos) <= 10)
                    .map(|city| city.id)
                    .collect();
                for city_id in targets {
                    if !self.city_ignores_foreign_religion(&self.cities[&city_id], &religion) {
                        *self
                            .cities
                            .get_mut(&city_id)
                            .unwrap()
                            .pressure
                            .entry(religion.clone())
                            .or_insert(0.0) += pressure;
                    }
                }
                self.check_religious_victory();
            }
        }
        self.record_war_unit_loss(
            pid,
            victim.owner,
            &victim.kind,
            victim.formation,
            victim.pos,
        );
    }
}

#[cfg(test)]
mod production_catalog_tests;

#[cfg(test)]
mod yield_carry_tests;

#[cfg(test)]
mod dedication_era_tests;

#[cfg(test)]
mod team_tests;

#[cfg(test)]
mod border_growth_tests;

#[cfg(test)]
mod visibility_tests;

#[cfg(test)]
mod combat_scenarios;

#[cfg(test)]
mod gdr_armor_tests;

#[cfg(test)]
mod approach_arrival_tests;

#[cfg(test)]
mod movement_rule_tests;

#[cfg(test)]
mod victory_conditions;

#[cfg(test)]
mod great_work_tests;

#[cfg(test)]
mod government_deck;

#[cfg(test)]
mod district_mechanics;

#[cfg(test)]
mod diplomatic_relations_tests;

#[cfg(test)]
mod world_lap_tests;

#[cfg(test)]
mod purchase_gate_tests;

#[cfg(test)]
mod purchase_price_memo_tests;

#[cfg(test)]
mod unit_upgrade_price_tests;

#[cfg(test)]
mod host_resource_price_tests;

#[cfg(test)]
mod host_upgrade_resource_tests;

#[cfg(test)]
mod regional_building_tests;

#[cfg(test)]
mod wonder_effect_cache_tests;

#[cfg(test)]
mod building_activity_cache_tests;

#[cfg(test)]
mod attack_reach_flood_tests;

#[cfg(test)]
mod luxury_allocation_tests;

#[cfg(test)]
mod fogged_elimination_tests;

#[cfg(test)]
mod host_war_permission_tests;

#[cfg(test)]
mod siege_support_eligibility_tests;

#[cfg(test)]
mod air_pillage_eligibility_tests;

#[cfg(test)]
mod native_great_person_recruitment_tests;

#[cfg(test)]
mod gran_colombia_traits_tests;

#[cfg(test)]
mod mongolia_combat_tests;

#[cfg(test)]
mod ranged_handicap_tests;
