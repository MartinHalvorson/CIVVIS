//! Civilization VI's stock game-setup presets.
//!
//! Keep these values in one place: browser games, CLI games, map generation,
//! city-state defaults, religion limits, and observation metadata all consume
//! the same profile instead of maintaining subtly different tables.

use std::collections::BTreeMap;

use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

/// Which published game's rules the world is played by.
///
/// This is the outermost setting on the lobby — it is asked before the game
/// mode, because it decides what every other question means. CIVVIS models
/// Civilization VI, so Civilization VI is the only answer today; the setting
/// exists so that a second ruleset can be added without the first one having
/// been an unstated assumption baked through the setup screen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseRuleset {
    #[default]
    Civ6,
}

impl BaseRuleset {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Civ6 => "civ6",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        BASE_RULESETS
            .iter()
            .find(|spec| spec.id == id)
            .map(|spec| spec.ruleset)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BaseRulesetSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip)]
    pub ruleset: BaseRuleset,
}

pub const BASE_RULESETS: [BaseRulesetSpec; 1] = [BaseRulesetSpec {
    id: "civ6",
    name: "Civ 6",
    description:
        "Civilization VI with Rise & Fall and Gathering Storm — the rules every other setting on this screen is expressed in.",
    ruleset: BaseRuleset::Civ6,
}];

/// One rung of the ladder a game can open on.
///
/// The ladder is a timeline, earliest first. A rung that can be played carries
/// [`StartEraSpec::era`] — its index into [`crate::rules::ERA_NAMES`], which is
/// the era the technology and civic trees are cut at. That index is what makes
/// a start era a rule rather than a label.
///
/// A rung that is declared but not built carries `None`. It is listed in the
/// lobby as what is coming, with a `· later` suffix, and is refused by the
/// server and the CLI rather than quietly played as the Ancient era.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartEraSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Which of [`crate::rules::ERA_NAMES`] this rung cuts the trees at, or
    /// `None` for a rung that has no tree behind it yet.
    pub era: Option<usize>,
}

impl StartEraSpec {
    /// Whether a game can actually open on this rung. There is one source for
    /// that answer rather than a flag to keep in step: a rung is playable
    /// exactly when it names an era the ruleset has a tree for.
    pub const fn is_playable(&self) -> bool {
        self.era.is_some()
    }
}

/// The lobby needs to know whether a rung can be chosen, not which era index it
/// cuts the trees at — that index is this crate's business, and publishing it
/// would invite a client to do the resolving itself.
impl Serialize for StartEraSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut spec = serializer.serialize_struct("StartEraSpec", 4)?;
        spec.serialize_field("id", &self.id)?;
        spec.serialize_field("name", &self.name)?;
        spec.serialize_field("description", &self.description)?;
        spec.serialize_field("playable", &self.is_playable())?;
        spec.end()
    }
}

/// The ladder, earliest first: the eight eras Civilization VI is played
/// through, with the age that precedes them named ahead of the first.
///
/// The Stone Age is listed because human history does not in fact begin with a
/// Settler and a Warrior, and a lobby that opens on "Ancient" quietly claims it
/// does. It has no tree behind it yet, so it is offered as a plan rather than a
/// choice — `era: None` is the whole of that statement.
///
/// Future closes the ladder rather than leaving it: a game can open there with
/// every earlier era known, while the Future technologies and civics remain to
/// be played. There is no later era to advance into, which is exactly what a
/// Future-era opening promises.
pub const START_ERAS: [StartEraSpec; 10] = [
    StartEraSpec {
        id: "stone_age",
        name: "Stone Age",
        description: "Before the first city: bands that follow the herd, knap what they carry, and hold no ground for longer than the season is worth staying.",
        era: None,
    },
    StartEraSpec {
        id: "ancient",
        name: "Ancient",
        description: "The stock start: a Settler, a Warrior, an empty tree and the whole of history in front of you.",
        era: Some(0),
    },
    StartEraSpec {
        id: "classical",
        name: "Classical",
        description: "Begin with the Ancient era already learned — writing, bronze, and the first cities worth taking.",
        era: Some(1),
    },
    StartEraSpec {
        id: "medieval",
        name: "Medieval",
        description: "Begin past antiquity: everything up to the Classical era is known, and the world opens on castles and faith.",
        era: Some(2),
    },
    StartEraSpec {
        id: "renaissance",
        name: "Renaissance",
        description: "Begin with the Medieval era behind you, on the edge of gunpowder and the open ocean.",
        era: Some(3),
    },
    StartEraSpec {
        id: "industrial",
        name: "Industrial",
        description: "Begin with the Renaissance learned: rifles, railways and coal are the next thing, not a distant one.",
        era: Some(4),
    },
    StartEraSpec {
        id: "modern",
        name: "Modern",
        description: "Begin with the Industrial era done — flight, radio and the first world war's worth of army.",
        era: Some(5),
    },
    StartEraSpec {
        id: "atomic",
        name: "Atomic",
        description: "Begin with the Modern era known, at the point where a single unit can end a city.",
        era: Some(6),
    },
    StartEraSpec {
        id: "information",
        name: "Information",
        description: "Begin with everything up to the Atomic era researched: a short, sharp game decided by satellites, robots and points.",
        era: Some(7),
    },
    StartEraSpec {
        id: "future",
        name: "Future Era",
        description: "Begin with the Information era behind you: Future technology and civics decide the world, with no later age to wait for.",
        era: Some(8),
    },
];

/// The rungs a game can actually open on, earliest first.
pub fn playable_start_eras() -> impl Iterator<Item = &'static StartEraSpec> {
    START_ERAS.iter().filter(|spec| spec.is_playable())
}

/// Which rules the far end of the game is played by.
///
/// The start era says where a game opens; this says what it opens *onto*. The
/// classic era is Gathering Storm's: the Future tree, the space race, and a
/// Moon that is a milestone rather than a place. The modified one keeps all of
/// that and adds the Moon's ore and the mass driver that throws it down.
///
/// It is a rules choice rather than a world one, which is why it is resolved
/// in [`crate::rules::Rules::for_game`] and recorded on the save: a match runs
/// on the rules it started under, whatever a later build ships.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FutureEra {
    #[default]
    Classic,
    Modified,
}

impl FutureEra {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Modified => "modified",
        }
    }
}

/// One Future Era the lobby can offer. Same contract as [`StartEraSpec`]: an
/// era nobody has built carries `None` and is listed as what is coming rather
/// than hidden.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FutureEraSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// The rules this era resolves to, or `None` for one that is declared but
    /// not built.
    pub era: Option<FutureEra>,
}

impl FutureEraSpec {
    pub const fn is_playable(&self) -> bool {
        self.era.is_some()
    }
}

/// The lobby is told whether an era can be chosen, not which variant of the
/// rules it selects — that is this crate's business.
impl Serialize for FutureEraSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut spec = serializer.serialize_struct("FutureEraSpec", 4)?;
        spec.serialize_field("id", &self.id)?;
        spec.serialize_field("name", &self.name)?;
        spec.serialize_field("description", &self.description)?;
        spec.serialize_field("playable", &self.is_playable())?;
        spec.end()
    }
}

pub const FUTURE_ERAS: [FutureEraSpec; 2] = [
    FutureEraSpec {
        id: "classic",
        name: "Classic Future Era",
        description: "Gathering Storm's own: two random research columns, the space race, and a Moon that is a milestone you pass rather than a place you go back to.",
        era: Some(FutureEra::Classic),
    },
    FutureEraSpec {
        id: "modified",
        name: "Modified Future Era",
        description: "The Moon is a body with ore in it — one set of piles, shared by everybody who gets there — and a mass driver on its surface throws that ore down a gravity well onto a tile you name.",
        era: Some(FutureEra::Modified),
    },
];

/// Resolve a Future Era id to the rules it names. An id that is not on the
/// list — or one naming an era nobody has built — resolves to nothing rather
/// than to the classic rules, so the caller decides what to say about it
/// instead of the player quietly getting a different game.
pub fn future_era_from_id(id: &str) -> Option<FutureEra> {
    FUTURE_ERAS
        .iter()
        .find(|spec| spec.id == id)
        .and_then(|spec| spec.era)
}

/// The Future Era a game is played under, as the lobby names it.
pub fn future_era_id(era: FutureEra) -> &'static str {
    era.id()
}

/// How the seats of one game turn relate to each other in time.
///
/// Sequential is the stock regime: each civilization acts on the world exactly
/// as the previous one left it. Simultaneous freezes the world at the top of
/// the game turn, lets every seat plan its whole turn against that same
/// snapshot, then commits the plans in seat order under the ordinary rules —
/// an order that has become illegal by the time it commits is dropped, not
/// reinterpreted. It is an information choice, not a rules choice: every
/// committed action goes through the same `Game::apply` either way, which is
/// what keeps a simultaneous game's action log an ordinary replayable log.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStructure {
    #[default]
    Sequential,
    Simultaneous,
}

impl TurnStructure {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Simultaneous => "simultaneous",
        }
    }
}

/// One turn structure a setup screen can offer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TurnStructureSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const TURN_STRUCTURES: [TurnStructureSpec; 2] = [
    TurnStructureSpec {
        id: "sequential",
        name: "Sequential turns",
        description: "Civilizations act one after another; each sees the world exactly as the previous one left it.",
    },
    TurnStructureSpec {
        id: "simultaneous",
        name: "Simultaneous turns",
        description: "Every civilization plans the turn against the same snapshot of the world, and the plans are then committed together; a plan the world has outrun is dropped.",
    },
];

/// Resolve a turn-structure id. Same contract as [`future_era_from_id`]: an
/// unknown id resolves to nothing rather than to the stock regime, so the
/// caller decides what to say about it.
pub fn turn_structure_from_id(id: &str) -> Option<TurnStructure> {
    match id {
        "sequential" => Some(TurnStructure::Sequential),
        "simultaneous" => Some(TurnStructure::Simultaneous),
        _ => None,
    }
}

/// The turn structure a game is played under, as the lobby names it.
pub fn turn_structure_id(structure: TurnStructure) -> &'static str {
    structure.id()
}

/// Resolve a start era id to the era its trees are cut at.
///
/// An id that is not on the ladder — or one naming a rung nobody has built yet
/// — resolves to nothing rather than to the stock start, so the caller decides
/// what to say about it instead of the player being quietly handed a different
/// game from the one they asked for.
pub fn start_era_from_id(id: &str) -> Option<usize> {
    START_ERAS
        .iter()
        .find(|spec| spec.id == id)
        .and_then(|spec| spec.era)
}

/// The id of the rung a game opening at this era started on. An era off the end
/// of the ladder names the stock start rather than nothing at all.
pub fn start_era_id(era: usize) -> &'static str {
    START_ERAS
        .iter()
        .find(|spec| spec.era == Some(era))
        .map_or_else(stock_start_era_id, |spec| spec.id)
}

/// The stock start — the first rung with a tree behind it, which is what a game
/// opens on when nobody chooses.
pub fn stock_start_era_id() -> &'static str {
    playable_start_eras()
        .next()
        .expect("the ladder has at least one playable rung")
        .id
}

/// The latest era a game can open on, which is what an era index past the end
/// of the ladder is clamped to.
pub fn last_start_era() -> usize {
    playable_start_eras()
        .filter_map(|spec| spec.era)
        .max()
        .unwrap_or(0)
}

/// What the world is made of, in the order the setup menu offers it.
///
/// The ordinary scripts remain grouped from solid land toward open water, but
/// the two Earth maps sit immediately before Continents so the same coastline
/// can be chosen with either ordinary or historical starts. Fjords follows
/// Small Continents because it is a terrain-first coastal world rather than a
/// point on the land-share dial.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapScript {
    LandOnly,
    Lakes,
    InlandSea,
    TeninsBall,
    GrandCanals,
    GrandCanalsTwo,
    #[default]
    Pangaea,
    Earth,
    TrueStartEarth,
    Continents,
    SmallContinents,
    Fjords,
    Islands,
    WaterWorld,
    Battlefield,
    TacticsPlanet,
    TacticsOcean,
    Trafalgar,
    Kadesh,
    Marathon,
    Thermopylae,
    Gaugamela,
    Cannae,
    Actium,
    Hastings,
    Hattin,
    Agincourt,
    Constantinople1453,
    Lepanto,
    SpanishArmada,
    Waterloo,
    Gettysburg,
    Stalingrad,
    Normandy,
    Midway,
    Inchon,
    DienBienPhu,
    SixDayWar,
    DesertStorm,
    Fallujah,
    Mosul,
}

impl MapScript {
    pub const fn id(self) -> &'static str {
        match self {
            Self::LandOnly => "land_only",
            Self::Lakes => "lakes",
            Self::InlandSea => "inland_sea",
            Self::TeninsBall => "tenins_ball",
            Self::GrandCanals => "grand_canals",
            Self::GrandCanalsTwo => "grand_canals_2",
            Self::Pangaea => "pangaea",
            Self::Earth => "earth",
            Self::TrueStartEarth => "true_start_earth",
            Self::Continents => "continents",
            Self::SmallContinents => "small_continents",
            Self::Fjords => "fjords",
            Self::Islands => "islands",
            Self::WaterWorld => "water_world",
            Self::Battlefield => "battlefield",
            Self::TacticsPlanet => "tactics_planet",
            Self::TacticsOcean => "tactics_ocean",
            Self::Trafalgar => "trafalgar",
            Self::Kadesh => "kadesh",
            Self::Marathon => "marathon",
            Self::Thermopylae => "thermopylae",
            Self::Gaugamela => "gaugamela",
            Self::Cannae => "cannae",
            Self::Actium => "actium",
            Self::Hastings => "hastings",
            Self::Hattin => "hattin",
            Self::Agincourt => "agincourt",
            Self::Constantinople1453 => "constantinople_1453",
            Self::Lepanto => "lepanto",
            Self::SpanishArmada => "spanish_armada",
            Self::Waterloo => "waterloo",
            Self::Gettysburg => "gettysburg",
            Self::Stalingrad => "stalingrad",
            Self::Normandy => "normandy",
            Self::Midway => "midway",
            Self::Inchon => "inchon",
            Self::DienBienPhu => "dien_bien_phu",
            Self::SixDayWar => "six_day_war",
            Self::DesertStorm => "desert_storm",
            Self::Fallujah => "fallujah",
            Self::Mosul => "mosul",
        }
    }

    /// Whether the script draws a Tactics map rather than a Civ world. Tactics
    /// has both a bounded arena and a small globe, but both start with two
    /// cities and armies, skip the development layer, and use the same combat
    /// rules.
    pub const fn is_battlefield(self) -> bool {
        matches!(
            self,
            Self::Battlefield
                | Self::TacticsPlanet
                | Self::TacticsOcean
                | Self::Trafalgar
                | Self::Kadesh
                | Self::Marathon
                | Self::Thermopylae
                | Self::Gaugamela
                | Self::Cannae
                | Self::Actium
                | Self::Hastings
                | Self::Hattin
                | Self::Agincourt
                | Self::Constantinople1453
                | Self::Lepanto
                | Self::SpanishArmada
                | Self::Waterloo
                | Self::Gettysburg
                | Self::Stalingrad
                | Self::Normandy
                | Self::Midway
                | Self::Inchon
                | Self::DienBienPhu
                | Self::SixDayWar
                | Self::DesertStorm
                | Self::Fallujah
                | Self::Mosul
        )
    }

    /// Whether this Tactics map is a scripted historical battle rather than a
    /// rolled arena.
    ///
    /// A scenario owes the player something an arena does not: the same
    /// ground, the same two fleets and the same opening every time it is
    /// launched, because it is a claim about a real battle and not a fresh
    /// roll of one. So it draws its own map from a fixed chart rather than
    /// from the seed, seats each side where history put it rather than at
    /// opposite ends, deals each its own order of battle rather than one
    /// mirrored roster, and fixes the arena economy it is fought under —
    /// see [`TacticsRules::for_script`]. The seed still moves nothing that is
    /// left to move, which on a scenario is nothing at all.
    ///
    /// It also gives up the arena's central promise in exchange: the two
    /// sides are **not** even. Twenty-seven British ships against thirty-three
    /// is the fact the battle is about. A scenario result is therefore a
    /// re-fight of a particular engagement and never a measurement of two
    /// agents against each other — `battle_bench` and the matched skirmish
    /// remain the instruments for that.
    pub const fn is_scenario(self) -> bool {
        matches!(
            self,
            Self::Trafalgar
                | Self::Kadesh
                | Self::Marathon
                | Self::Thermopylae
                | Self::Gaugamela
                | Self::Cannae
                | Self::Actium
                | Self::Hastings
                | Self::Hattin
                | Self::Agincourt
                | Self::Constantinople1453
                | Self::Lepanto
                | Self::SpanishArmada
                | Self::Waterloo
                | Self::Gettysburg
                | Self::Stalingrad
                | Self::Normandy
                | Self::Midway
                | Self::Inchon
                | Self::DienBienPhu
                | Self::SixDayWar
                | Self::DesertStorm
                | Self::Fallujah
                | Self::Mosul
        )
    }

    /// The civilizations a scenario seats, in seat order, or `None` where the
    /// map does not care who fights on it.
    ///
    /// Trafalgar seats Britain first because Britain moved first: Nelson's two
    /// columns were already bearing down when the Combined Fleet was still
    /// dressing its line, and the whole battle is the story of that approach
    /// being pressed home. Seat 0 is also the seat a person plays, so the
    /// scenario hands them the side whose plan there is something to execute.
    ///
    /// Two seats for three navies. Tactics is a two-sided mode throughout —
    /// its starts, its series and its ratings all assume two — and the
    /// Combined Fleet fought as one command under Villeneuve, so France holds
    /// the seat and the Spanish ships are in its line where they stood. The
    /// Spanish flag is missing from the HUD; nothing else about them is.
    pub fn scenario_civs(self) -> Option<[&'static str; 2]> {
        crate::historical_scenarios::by_script(self).map(|scenario| scenario.civs)
    }

    /// Whether this Tactics map is a globe rather than the bounded arena. Its
    /// topology is allowed to remain Planet and its land generator keeps the
    /// ordinary world terrain instead of painting arena ground over it.
    ///
    /// Both globes answer yes: the land planet an army marches over and the
    /// ocean one a fleet sails. What separates them is which element carries
    /// the battle, which is [`Self::is_naval_battlefield`].
    pub const fn is_planet_battlefield(self) -> bool {
        matches!(self, Self::TacticsPlanet | Self::TacticsOcean)
    }

    /// Whether this Tactics map is fought on water rather than on land.
    ///
    /// A naval battlefield inverts every element test the mode makes: the two
    /// sides are seated on open water instead of on shore, their opening
    /// order of battle is ships instead of a marching column, and a flag — if
    /// the battle is fought for one — is planted on water, because a flag on
    /// dry ground is one no fleet can ever reach. The islets it rolls are
    /// cover and chokepoints, and the two cities stand on them; the sea
    /// between is the field.
    pub const fn is_naval_battlefield(self) -> bool {
        matches!(self, Self::TacticsOcean)
    }

    /// Whether the script draws a fixed world instead of rolling a new one.
    /// Earth is the same Earth every game, so the seed moves its resources and
    /// its rivers around but never its coastlines. Fixed geography is still
    /// independent of shape: it can be sampled onto a flat atlas or a globe.
    pub const fn is_fixed_geography(self) -> bool {
        matches!(self, Self::Earth | Self::TrueStartEarth)
    }

    /// Whether a fixed Earth map also assigns each civilization its historic
    /// homeland. [`Self::Earth`] keeps the same coastlines, relief, climate,
    /// and real-world wonders while using the ordinary balanced start picker.
    pub const fn is_true_start(self) -> bool {
        matches!(self, Self::TrueStartEarth)
    }

    /// Roughly what share of the world this script leaves as non-water terrain,
    /// as the generator aims for it. Fjords includes its mountain terrain in
    /// this total; its separate relief profile divides that sixty percent into
    /// forty percent ordinary land and twenty percent mountains.
    pub const fn land_percent(self) -> u32 {
        match self {
            Self::LandOnly => 95,
            Self::Lakes => 81,
            Self::InlandSea => 68,
            // The seam is the world's only ordinary water on a flat map. A
            // globe also holds its pentagons and, with poles, its caps out of
            // the ground, so this is the share both shapes come out near.
            Self::TeninsBall => 79,
            // Ground nearly everywhere, less the six canals cut through it.
            // What the canals take is geometry rather than a share the
            // generator picks, so this is what is left once they have taken
            // it; see `mapgen::grand_canals`.
            Self::GrandCanals => 62,
            // Blocks of ground with a canal around every one of them. What is
            // left is set by how big a block is and how wide the canal around
            // its rim: see `mapgen::CANAL_BLOCK_LAND_TILES`, which is the
            // number this one is measured from rather than chosen against.
            Self::GrandCanalsTwo => 44,
            Self::Pangaea => 42,
            // Both Earth choices sample the same fixed geography. The only
            // difference is where their civilizations begin.
            Self::Earth | Self::TrueStartEarth => 29,
            Self::Continents => 42,
            Self::SmallContinents => 36,
            // Forty percent open water, forty percent ordinary land, and
            // twenty percent mountains. Mountains are land in the generator,
            // so this is sixty percent total non-water terrain.
            Self::Fjords => 60,
            Self::Islands => 22,
            Self::WaterWorld => 5,
            // Ground everywhere except the one-column seam that seals the
            // cylinder's east-west wrap into a bounded arena, plus a pond or
            // two so rivers have somewhere to reach the sea.
            Self::Battlefield => 88,
            // Ground almost everywhere, because Tactics is a fight over
            // ground: both sides open on the same continuous landmass and the
            // sea is scenery between them rather than a wall across the
            // battle. Cut like Land Only rather than laid like Pangaea — see
            // `mapgen::globe_land`, which above half carves the seas out of a
            // solid globe instead of growing a continent inside an ocean.
            // The caps and the twelve pentagons stay wet on top of that, so
            // the world lands a little under the share asked for here; 88
            // leaves it comfortably past the 80% the type promises.
            Self::TacticsPlanet => 88,
            // The mirror of the land planet: sea almost everywhere, because a
            // naval Tactics battle is a fight over water and the ground is
            // what interrupts it. What land there is has to earn its place —
            // islets to work a flank around, channels between them to be held
            // or forced, and a shore apiece for the two cities — so this is
            // set a little above Water World's bare specks. Any higher and
            // the islands start joining into coastline, which would give a
            // fleet a wall to be pinned against rather than a chokepoint to
            // fight over.
            Self::TacticsOcean => 10,
            // Open sea, less the strip of Andalusian shore along the eastern
            // edge and the headland of Cape Trafalgar itself. Read rather
            // than aimed for: the chart in `mapgen::TRAFALGAR_CHART` decides
            // it, and this is what that chart comes out at.
            Self::Trafalgar => 9,
            // Generic historical charts keep enough land for two deployment
            // bands. Their exact water and relief mix is supplied by
            // `historical_scenarios::paint_ground` below.
            _ if self.is_scenario() => 50,
            _ => 50,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        // Spellings the protocol has carried at one time or another. `planet`
        // named a script before the globe became a shape of its own; it now
        // means the world type that script generated, and the shape travels
        // separately in `map_topology`.
        match id {
            "pangea" => return Some(Self::Pangaea),
            "planet" => return Some(Self::SmallContinents),
            "archipelago" => return Some(Self::Islands),
            // Accept the corrected spelling at the protocol boundary while
            // retaining the legacy lobby identifier for this map type.
            "tennis_ball" => return Some(Self::TeninsBall),
            _ => {}
        }
        if let Some(script) = crate::historical_scenarios::script_from_id(id) {
            return Some(script);
        }
        CIV6_MAP_SCRIPTS
            .iter()
            .find(|script| script.id == id)
            .map(|script| script.script)
    }
}

/// What shape the world is, chosen independently of what fills it.
///
/// This is the setting; [`crate::world::Topology`] is the shape the generator
/// actually builds from it, which additionally carries the globe's subdivision
/// frequency. Every world type can be laid out either way, so "Continents" is
/// a question about land and "Planet" a question about the world's shape, and
/// answering one no longer answers the other.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapTopology {
    /// A rectangle that wraps east to west and ends at a northern and a
    /// southern edge.
    #[default]
    Flat,
    /// A closed geodesic globe: hexagons and twelve pentagons, sailable all
    /// the way around in every direction.
    Planet,
}

impl MapTopology {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Planet => "planet",
        }
    }

    pub const fn is_globe(self) -> bool {
        matches!(self, Self::Planet)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        // `globe` and `sphere` are what callers reach for first; `cylinder` is
        // what the engine calls a flat map internally.
        match id {
            "flat" | "cylinder" | "rectangle" => Some(Self::Flat),
            "planet" | "globe" | "sphere" => Some(Self::Planet),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MapTopologySpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip)]
    pub topology: MapTopology,
}

pub const MAP_TOPOLOGIES: [MapTopologySpec; 2] = [
    MapTopologySpec {
        id: "flat",
        name: "Flat",
        description: "A rectangle that wraps east to west, with a northern and a southern edge.",
        topology: MapTopology::Flat,
    },
    MapTopologySpec {
        id: "planet",
        name: "Planet",
        description:
            "A whole world: hexagons and twelve pentagons closing into a globe you can sail all the way around, in any direction.",
        topology: MapTopology::Planet,
    },
];

/// How heat is laid out across the world.
///
/// Two arrangements, and both of them carry the whole range from jungle to
/// snow. With poles, latitude runs the climate: the middle of the world is its
/// hottest ground and every step towards an extreme is colder, ending in
/// tundra, snow and sea ice. Randomized keeps both ends of that range but
/// unhitches them from latitude: a tile is as cold as its own patch of noise
/// says, so snow and jungle are neighbours as readily as antipodes.
///
/// A third setting, `no_poles`, once offered a world with no cold end at all —
/// no ice, no snow, no tundra anywhere. It is retired: a world that cannot grow
/// a third of the terrain table is a narrower game rather than a different one,
/// and the two that remain are the real choice, whether heat follows latitude
/// or ignores it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapPoles {
    /// The alias is for saves, not for lobbies: a checkpoint written while
    /// `no_poles` was still on offer holds a whole generated world, and
    /// refusing to read it back would lose that game rather than merely
    /// mislabel it. Its ground is already painted, so all this decides is the
    /// name the resumed game reports.
    #[default]
    #[serde(alias = "no_poles")]
    Poles,
    Randomized,
}

impl MapPoles {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Poles => "poles",
            Self::Randomized => "randomized",
        }
    }

    /// Whether cold ground sits at the world's extremes. This gates the polar
    /// sea-ice band and the polar cap on start placement, so it is false for
    /// `Randomized`: that world has cold ground, but nowhere in particular.
    pub const fn has_poles(self) -> bool {
        matches!(self, Self::Poles)
    }

    /// The retired `no_poles` spellings — including the `off`/`false` of the
    /// era when this setting was a boolean — name nothing now, so a caller
    /// still asking for that world is left with the default rather than handed
    /// a silent substitute for a world it did not ask for.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "poles" | "on" | "true" => Some(Self::Poles),
            "randomized" | "random" | "scattered" => Some(Self::Randomized),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MapPolesSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip)]
    pub poles: MapPoles,
}

pub const MAP_POLES: [MapPolesSpec; 2] = [
    MapPolesSpec {
        id: "poles",
        name: "Hot equator, cold poles",
        description: "Hottest across the middle of the world, colder towards each extreme, ending in tundra, snow and sea ice.",
        poles: MapPoles::Poles,
    },
    MapPolesSpec {
        id: "randomized",
        name: "Randomized",
        description: "Heat scattered in patches instead of banded by latitude: snow, desert and jungle turn up anywhere, and the poles are no colder than the equator.",
        poles: MapPoles::Randomized,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MapScriptSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip)]
    pub script: MapScript,
}

/// The world types in the order [`MapScript`] declares them. The Tactics maps
/// sit last because they are not Civ worlds: the game mode offers them only
/// when Tactics is chosen.
pub const CIV6_MAP_SCRIPTS: [MapScriptSpec; 18] = [
    MapScriptSpec {
        id: "land_only",
        name: "Land Only",
        description: "Almost nothing but land — one unbroken world with a scatter of inland seas for its water.",
        script: MapScript::LandOnly,
    },
    MapScriptSpec {
        id: "lakes",
        name: "Lakes",
        description: "A world of land, broken up by lakes and inland seas instead of oceans.",
        script: MapScript::Lakes,
    },
    MapScriptSpec {
        id: "inland_sea",
        name: "Inland Sea",
        description: "A broad connected landmass surrounding a central sea.",
        script: MapScript::InlandSea,
    },
    MapScriptSpec {
        id: "tenins_ball",
        name: "Tennis Ball",
        description: "Two land lobes divided by a five-to-six-tile water seam that loops around the whole world like a tennis ball's stitching.",
        script: MapScript::TeninsBall,
    },
    MapScriptSpec {
        id: "grand_canals",
        name: "Grand Canals",
        description: "A world of solid ground cut by six canals that each circle it: two around the poles, two around each of the other two axes, crossing at twenty-four junctions.",
        script: MapScript::GrandCanals,
    },
    MapScriptSpec {
        id: "grand_canals_2",
        name: "Grand Canals II",
        description: "The whole world cut into blocks of ground, each one a few dozen tiles across, and a canal around every one: a shallow shelf off either bank and a channel of deep ocean down the middle.",
        script: MapScript::GrandCanalsTwo,
    },
    MapScriptSpec {
        id: "pangaea",
        name: "Pangaea",
        description: "One connected supercontinent surrounded by ocean.",
        script: MapScript::Pangaea,
    },
    MapScriptSpec {
        id: "earth",
        name: "Earth",
        description: "Earth's real coastlines, relief, climates, and wonders, with ordinary balanced starting positions.",
        script: MapScript::Earth,
    },
    MapScriptSpec {
        id: "true_start_earth",
        name: "True Start Earth",
        description: "Earth itself, with every civilization founded in its own historic homeland.",
        script: MapScript::TrueStartEarth,
    },
    MapScriptSpec {
        id: "continents",
        name: "Continents",
        description: "A few large landmasses separated by open water.",
        script: MapScript::Continents,
    },
    MapScriptSpec {
        id: "small_continents",
        name: "Small Continents",
        description: "Several smaller landmasses with more coastline and sea lanes.",
        script: MapScript::SmallContinents,
    },
    MapScriptSpec {
        id: "fjords",
        name: "Fjords",
        description: "Forty percent open water, forty percent ordinary land, and twenty percent mountain ranges, with winding sea passages and passable breaks through the ridges.",
        script: MapScript::Fjords,
    },
    MapScriptSpec {
        id: "islands",
        name: "Islands",
        description: "An archipelago: many small islands, none of them a continent, every one of them its own shore.",
        script: MapScript::Islands,
    },
    MapScriptSpec {
        id: "water_world",
        name: "Water World",
        description: "Almost nothing but ocean — scattered specks of land, and the sea lanes between them are the map.",
        script: MapScript::WaterWorld,
    },
    // The Tactics maps are named for what fills them, because the lobby asks
    // for the world type first and the map second: a flat world offers Land,
    // a planet offers Land or Ocean, and only one Land is ever on the menu
    // at a time. The ids stay what saves, sweeps and deep links have always
    // named — `battlefield` is still the bounded field.
    MapScriptSpec {
        id: "battlefield",
        name: "Land",
        description: "A small bounded field for tactical unit combat, walled on all four sides: open ground shaped by mountains, rivers, woods and water, with no resources and nothing to develop. Offered at three sizes — Square, March and Field.",
        script: MapScript::Battlefield,
    },
    MapScriptSpec {
        id: "tactics_planet",
        name: "Land",
        description: "A land planet for Tactics: a globe that is more than four-fifths dry ground, with scattered seas rather than an ocean. One city for each side, planted on opposite faces of the world and joined by a march overland. Offered at four diameters, from a world half the size of Duel up to one larger than it.",
        script: MapScript::TacticsPlanet,
    },
    MapScriptSpec {
        id: "tactics_ocean",
        name: "Ocean",
        description: "A water planet for Tactics: open sea over the whole globe, broken by scattered islets and the channels between them. Each side opens with a fleet and a city on its own island, on opposite faces of the world, and every route between them is a sea lane. Offered at the same four diameters as Land.",
        script: MapScript::TacticsOcean,
    },
    MapScriptSpec {
        id: "trafalgar",
        name: "Trafalgar · 21 October 1805",
        description: "The battle as it stood at noon: twenty-seven British ships of the line in Nelson's and Collingwood's two columns, bearing down from the west on the Combined Fleet's thirty-three, strung out in a crescent with Cape Trafalgar and its shoals under their lee. Fixed ground, fixed fleets, and Britain to move.",
        script: MapScript::Trafalgar,
    },
];

/// The world types a lobby offers for the Civ game mode: every script but the
/// battlefield, which is not a world and belongs to the Tactics mode's own
/// menu. Both menus stay data-driven from the one authoritative roster.
pub fn world_map_scripts() -> Vec<&'static MapScriptSpec> {
    CIV6_MAP_SCRIPTS
        .iter()
        .filter(|spec| !spec.script.is_battlefield())
        .collect()
}

/// The Tactics mode's map menu, published separately from the Civ world list
/// so arenas and future tactical worlds can grow without a protocol change.
pub fn battlefield_map_scripts() -> Vec<MapScriptSpec> {
    let mut maps: Vec<MapScriptSpec> = CIV6_MAP_SCRIPTS
        .iter()
        .filter(|spec| spec.script.is_battlefield())
        .copied()
        .collect();
    maps.extend(
        crate::historical_scenarios::generic_scenarios().filter_map(|scenario| {
            Some(MapScriptSpec {
                id: scenario.id,
                name: scenario.name,
                description: scenario.summary,
                script: crate::historical_scenarios::script_from_id(scenario.id)?,
            })
        }),
    );
    maps
}

/// The subset of those that are scripted historical battles. A setup surface
/// needs this to know which of the arena's controls it may still offer: a
/// scenario fixes its own economy, so most of that card is not the player's
/// on one. See [`TacticsRules::for_script`].
pub fn scenario_map_scripts() -> Vec<MapScriptSpec> {
    battlefield_map_scripts()
        .into_iter()
        .filter(|spec| spec.script.is_scenario())
        .collect()
}

/// Which game this is: the whole thing, or one half of it on its own.
///
/// Civ is the complete game — a civilization grown, defended and won with.
/// The other two take one half each and ask whether you can play it: Tactics
/// is the fighting with no city to build, Sim City is the building with no
/// fighting to do. They are separate skills (the retired Elo ledgers rated
/// them separately, and any future rating should again).
///
/// The mode is read from the map script rather than stored beside it. An
/// arena is not a world and a world is not an arena, so the script already
/// answers the question, and keeping it the only marker means no save, no
/// `Params` field and no protocol row can ever disagree with the map about
/// which game is being played.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    /// The complete grand-strategy game: everything, on a world.
    #[default]
    Civ,
    /// Pure unit tactics: two even armies on an arena, no city building.
    Tactics,
    /// Pure development: cities, builders and traders, no fighting.
    ///
    /// Declared but not yet playable — `for_script` never returns it, because
    /// no map script builds one. It is here so the rating layer and the mode
    /// menus have the third slot from the start rather than growing it later.
    SimCity,
}

impl GameMode {
    pub const ALL: [GameMode; 3] = [GameMode::Civ, GameMode::Tactics, GameMode::SimCity];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Civ => "civ",
            Self::Tactics => "tactics",
            Self::SimCity => "simcity",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Civ => "Civ",
            Self::Tactics => "Tactics",
            Self::SimCity => "Sim City",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }

    /// Whether this mode can be played today.
    pub const fn is_playable(self) -> bool {
        !matches!(self, Self::SimCity)
    }

    /// Which game a world of this type is a game of.
    pub const fn for_script(script: MapScript) -> GameMode {
        if script.is_battlefield() {
            GameMode::Tactics
        } else {
            GameMode::Civ
        }
    }
}

/// A Tactics map size. Bounded arenas use their dimensions directly; the
/// Planet entry uses the same width/height fields as the globe storage shape,
/// and carries its script and topology so the lobby can send one complete
/// choice without rebuilding the mapping in JavaScript.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BattlefieldSize {
    pub id: &'static str,
    pub name: &'static str,
    pub width: i32,
    pub height: i32,
    pub script: MapScript,
    pub topology: MapTopology,
}

/// The Tactics maps the setup menu offers, smallest first within each family.
/// The arena entries are the bounded Land field at three sizes — Square, March
/// and Field, the names it shipped with (#1367; briefly renamed in #1626 and
/// put back at the operator's word): a small square, a long ground, and the
/// full field the site's Tactics link opens; then the globes, land and then
/// ocean, each offered at four diameters; then the scenarios, which carry
/// their own chart and so have exactly one size each.
///
/// The lobby offers this table one map at a time: which entries it shows is
/// decided by the scenario, world type and map chosen above it, so a named
/// battle lists only its own chart and a custom battle lists the sizes its map
/// is drawn at. Any map may carry several rows here; a scenario that is ever
/// charted at more than one size needs nothing but a second row.
///
/// A globe's size is named by its **diameter**, which is the subdivision
/// frequency of the icosahedron it is built from — the same number
/// [`crate::sphere`] is indexed by, and the one that decides everything else
/// about the world: frequency `n` holds `10n² + 2` tiles and is stored in a
/// `5n × 2n + 2` rectangle. Diameter 8 is the world Tactics has always
/// shipped, at 642 tiles; 20 is 4002, larger than a Duel world. The four are
/// a ladder rather than a spread of shapes, so a result at one diameter can be
/// read against the same battle fought on more room.
///
/// The land and ocean ladders deliberately use the same four numbers. The two
/// are the same experiment on opposite elements, and a diameter that meant one
/// thing on land and another at sea would make them incomparable.
pub const TACTICS_GLOBE_DIAMETERS: [i32; 4] = [8, 10, 15, 20];

/// See [`TACTICS_GLOBE_DIAMETERS`] for what a diameter means and why the two
/// globe families share one ladder.
pub const BATTLEFIELD_SIZES: [BattlefieldSize; 12] = [
    BattlefieldSize {
        id: "10x10",
        name: "Square · 10×10",
        width: 10,
        height: 10,
        script: MapScript::Battlefield,
        topology: MapTopology::Flat,
    },
    BattlefieldSize {
        id: "10x20",
        name: "March · 10×20",
        width: 10,
        height: 20,
        script: MapScript::Battlefield,
        topology: MapTopology::Flat,
    },
    BattlefieldSize {
        id: "20x20",
        name: "Field · 20×20",
        width: 20,
        height: 20,
        script: MapScript::Battlefield,
        topology: MapTopology::Flat,
    },
    // Diameter 8 keeps the bare id `planet`: it is the globe Tactics shipped
    // as its only one, and saved games, sweep scripts and lobby deep-links
    // name it that. The rest are `planet_<diameter>` and the ladder reads
    // consistently in the menu, where the name carries the number.
    BattlefieldSize {
        id: "planet",
        name: "Planet · diameter 8",
        width: 40,
        height: 18,
        script: MapScript::TacticsPlanet,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "planet_10",
        name: "Planet · diameter 10",
        width: 50,
        height: 22,
        script: MapScript::TacticsPlanet,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "planet_15",
        name: "Planet · diameter 15",
        width: 75,
        height: 32,
        script: MapScript::TacticsPlanet,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "planet_20",
        name: "Planet · diameter 20",
        width: 100,
        height: 42,
        script: MapScript::TacticsPlanet,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "ocean_8",
        name: "Ocean · diameter 8",
        width: 40,
        height: 18,
        script: MapScript::TacticsOcean,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "ocean_10",
        name: "Ocean · diameter 10",
        width: 50,
        height: 22,
        script: MapScript::TacticsOcean,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "ocean_15",
        name: "Ocean · diameter 15",
        width: 75,
        height: 32,
        script: MapScript::TacticsOcean,
        topology: MapTopology::Planet,
    },
    BattlefieldSize {
        id: "ocean_20",
        name: "Ocean · diameter 20",
        width: 100,
        height: 42,
        script: MapScript::TacticsOcean,
        topology: MapTopology::Planet,
    },
    // A scenario's dimensions are its chart's, not a size the player picks:
    // moving them would move the two fleets off the ground they were on.
    BattlefieldSize {
        id: "trafalgar",
        name: "Trafalgar · 21 October 1805",
        width: 30,
        height: 24,
        script: MapScript::Trafalgar,
        topology: MapTopology::Flat,
    },
];

/// The complete Tactics size roster, including one fixed footprint for every
/// named historical scenario. The five original arena sizes remain in the
/// public constant for CLI/save compatibility; this is the browser and
/// server contract that grows with the battle catalogue.
pub fn battlefield_sizes() -> Vec<BattlefieldSize> {
    let mut sizes = BATTLEFIELD_SIZES.to_vec();
    sizes.extend(
        crate::historical_scenarios::generic_scenarios().filter_map(|scenario| {
            Some(BattlefieldSize {
                id: scenario.id,
                name: scenario.name,
                width: scenario.width,
                height: scenario.height,
                script: crate::historical_scenarios::script_from_id(scenario.id)?,
                topology: MapTopology::Flat,
            })
        }),
    );
    sizes
}

/// The era a game opens in when a sweep asked for a random one.
///
/// Seeded off the game's own seed rather than a fresh source, so a run stays
/// exactly reproducible: the same seed always opens in the same era. The mix
/// is splitmix64's finalizer, and it earns its place — consecutive seeds are
/// consecutive integers, so taking them modulo the ladder directly would
/// march through the eras in lockstep with the seed instead of scattering
/// them.
///
/// Lives here rather than in a caller because both the command line and the
/// rating tournament roll it, and two implementations of "which era does seed
/// N open in" would eventually disagree — which would make a replayed
/// tournament a different experiment from the one that was rated.
pub fn random_start_era(seed: u64) -> usize {
    let playable: Vec<usize> = playable_start_eras().filter_map(|spec| spec.era).collect();
    playable[(scatter(seed) % playable.len() as u64) as usize]
}

/// The era a Random-era battle opens in: a seeded, scattered pick over the
/// built rungs short of the Future.
///
/// The Future stays out of the hat deliberately — its battles are meant to be
/// the modified era's drones and missiles, which nobody has built yet — and so
/// does the Moon, which is not on the ladder at all yet. When those rungs are
/// real they join this roll by name rather than the roll changing shape.
pub fn random_battle_era(seed: u64) -> usize {
    let eras: Vec<usize> = START_ERAS
        .iter()
        .filter(|spec| spec.id != "future")
        .filter_map(|spec| spec.era)
        .collect();
    eras[(scatter(seed) % eras.len() as u64) as usize]
}

/// splitmix64's finalizer, shared by both era rolls: consecutive seeds are
/// consecutive integers, and this is what keeps them from marching through
/// the ladder in lockstep.
fn scatter(seed: u64) -> u64 {
    let mut mix = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mix = (mix ^ (mix >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mix = (mix ^ (mix >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mix ^ (mix >> 31)
}

/// Which era arms a custom battle.
///
/// The arena's original rule was "whatever start era the game was asked for",
/// and that stays the default so every launch flag, benchmark and supervisor
/// lane keeps meaning exactly what it meant. The other three are the setup
/// box's choices: a fresh roll per battle, one rung of the ladder, or a
/// hand-picked pool of rungs fielded together.
///
/// This is a Tactics rule rather than a start-era spelling because a series
/// re-rolls it: `Random` must be remembered as *Random* across battles, not
/// as whichever era the first battle happened to open in. A named scenario
/// ignores it entirely — Trafalgar is fought by 1805 fleets whatever the
/// lobby asked for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TacticsEra {
    /// The game's own start era, exactly as asked. The default, and the only
    /// value a save from before this choice existed can mean.
    #[default]
    Start,
    /// A fresh roll for every battle over the built rungs short of the
    /// Future — see [`random_battle_era`].
    Random,
    /// One rung of the ladder, every battle.
    Fixed(usize),
    /// A hand-picked pool of eras fielded together, as a bitmask over era
    /// indices. The battle opens with the pool's latest era researched, and
    /// each side's army is dealt across the whole pool — so one field can
    /// carry a Warrior company and a Modern Armor company at once.
    Pool(u16),
}

impl TacticsEra {
    /// Every built rung of the ladder, as the mask [`TacticsEra::Pool`] is
    /// clamped against.
    fn built_mask() -> u16 {
        playable_start_eras()
            .filter_map(|spec| spec.era)
            .fold(0, |mask, era| mask | 1 << era)
    }

    /// The eras of a pool, earliest first. Empty for every other choice.
    pub fn pool_eras(self) -> Vec<usize> {
        let Self::Pool(mask) = self else {
            return Vec::new();
        };
        (0..16).filter(|era| mask & 1 << era != 0).collect()
    }

    /// The era the battle with `seed` actually opens in, given the start era
    /// the game was `asked` for.
    pub fn start_era(self, asked: usize, seed: u64) -> usize {
        match self {
            Self::Start => asked,
            Self::Random => random_battle_era(seed),
            Self::Fixed(era) => era.min(last_start_era()),
            // The pool's latest rung: the battle is *played* under the whole
            // pool — the army mix is the deployment's job — but the world can
            // only be one era old, and it has to be the one whose research
            // makes every pooled unit real.
            Self::Pool(mask) => Self::Pool(mask)
                .pool_eras()
                .into_iter()
                .max()
                .unwrap_or(asked),
        }
    }

    /// The choice clamped to rungs that exist. A pool that names none — a
    /// hand-written mask, or a save from a build with a longer ladder — falls
    /// back to the arena's original rule rather than to an invented era.
    fn sanitized(self) -> Self {
        match self {
            Self::Fixed(era) => Self::Fixed(era.min(last_start_era())),
            Self::Pool(mask) => match mask & Self::built_mask() {
                0 => Self::Start,
                built => Self::Pool(built),
            },
            choice => choice,
        }
    }
}

/// The Tactics arena's economy.
///
/// An arena has no empire behind it, so nothing here is earned: every figure
/// is simply granted, identically to both sides, and exists only to keep a
/// battle supplied. That is the point of the mode as a tactical testbed — the
/// two sides differ in how they fight and in nothing else, so an outcome is
/// attributable to the fighting.
///
/// The three flat grants answer "how much", and `turns_per_tech` answers "how
/// fast", because a flat Science figure cannot: a technology costs some
/// seventeen times more in the Information era than in the Ancient, so a fixed
/// yield that opens the tree briskly at the start stops opening it at all
/// later. Asking for a pace instead makes the grant whatever this turn's
/// research actually costs, divided by the pace — steady unlocks in every era
/// and at every game speed, without a table to maintain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TacticsRules {
    /// Cities each side opens with: 0 for armies alone, or 1.
    pub cities: u8,
    /// Production that city collects each turn, flat.
    pub production: u32,
    /// Gold each side collects each turn, flat. An arena charges no unit
    /// upkeep, so this is upgrade money and nothing else.
    pub gold: u32,
    /// Turns a side spends on whichever technology it is researching, in any
    /// era. 0 stops research, freezing both sides at their starting era's
    /// units.
    pub turns_per_tech: u32,
    /// Turns each battle may run before neither side wins and the battle is
    /// recorded as a draw. The setup surface offers the values in
    /// [`Self::TURN_LIMITS`]; old saves predate the choice and load the
    /// 100-turn battle they were fought under — see [`legacy_turn_limit`].
    #[serde(default = "legacy_turn_limit")]
    pub turn_limit: u32,
    /// Battles the two civilizations fight before a match is decided. 1 is a
    /// single battle; any higher odd number is a series, taken by the first
    /// side to win more than half of it.
    ///
    /// The pairing is what a series is for: the same two civilizations fight
    /// the same arena rules on fresh ground each time, so the result is about
    /// the two of them rather than about one roll of the map.
    #[serde(default = "one_battle")]
    pub best_of: u32,
    /// Whether a civilization may field its own unique units.
    ///
    /// Off, both sides field the identical stock roster and the battle is a
    /// test of play alone — which is the arena's whole claim, so it is the
    /// default. On, each side's roster and its build menu substitute whatever
    /// its civilization replaces a stock unit with, and the match becomes a
    /// test of the two civilizations as well.
    #[serde(default)]
    pub unique_units: bool,
    /// Whether the field is fogged.
    ///
    /// On by default: each side sees only what its own units can, and finding
    /// the enemy is part of the battle. Off, both commanders see all of it.
    /// Either way the rule is symmetric, so it is a shape the match is given
    /// rather than a handicap one side is dealt.
    ///
    /// The default used to be off, for a measured reason: a fogged arena had
    /// both sides stand in their own deployment bands until the clock decided
    /// it. That standoff was diagnosed and fixed — it came from two world
    /// instincts that were never gated for the arena, the per-tile
    /// local-superiority brake and the danger-retreat — rather than from the
    /// fog, so the reason to keep the field lit no longer holds. Fog is also
    /// what the globes need: a diameter-20 world is 4002 tiles, and a battle
    /// on it with both commanders omniscient is a different game from one
    /// where a fleet has to be found.
    ///
    /// A save written before fog existed loads unfogged, which is how it was
    /// played. The default here governs a *new* battle, not the replay of an
    /// old one — see [`legacy_unfogged`].
    #[serde(default = "legacy_unfogged")]
    pub fog: bool,
    /// Whether the battle is decided by a flag instead of by cities.
    ///
    /// On, a single flag is planted on the ground the two sides reach as
    /// evenly as the field allows, and the battle is won the moment either
    /// side's unit takes its tile. There are no cities at all in this shape —
    /// the flag is the one objective, so `cities` is forced to 0 — and the
    /// clock can still run out first, which stays a draw. Off — the default —
    /// a battle is decided the way an arena always has been: by the armies,
    /// and by the city each side holds if the economy grants one.
    #[serde(default)]
    pub flag: bool,
    /// Whether units recover. Off, the arena's own rule since the mode
    /// existed: one engagement, permanent damage, a trade you win stays won.
    /// On, a campaign — the unit that steps back is whole again in a few
    /// turns, and preserving it is worth something. Off in every save that
    /// does not carry the field, which is every battle played before it.
    #[serde(default)]
    pub heal: bool,
    /// Which era arms the battle — see [`TacticsEra`]. A save from before the
    /// choice existed reads as [`TacticsEra::Start`], which is exactly the
    /// rule it was played under.
    #[serde(default)]
    pub era: TacticsEra,
}

fn one_battle() -> u32 {
    1
}

/// What `turn_limit` is worth in a save that does not carry it.
///
/// Deliberately not [`TacticsRules::default`]'s answer, for the same reason
/// [`legacy_unfogged`] is not: every save without the field was written when
/// the arena's only clock was 100 turns, and reading it back as the current
/// 250-turn default would restate how long those battles were allowed to
/// run. New battles get the current default; old ones keep their clock.
fn legacy_turn_limit() -> u32 {
    100
}

/// What `fog` is worth in a save that does not carry it.
///
/// Deliberately not [`TacticsRules::default`]'s answer. Every save without the
/// field was written before Tactics had fog and was therefore played with the
/// whole field lit; reading them as fogged would silently restate how those
/// battles were fought, and a rated ladder replayed from them would no longer
/// be the experiment it recorded. New battles get the current default; old
/// ones keep the rules they were played under.
fn legacy_unfogged() -> bool {
    false
}

impl TacticsRules {
    /// The largest figure any of the flat grants may be set to. A ceiling
    /// exists so a hand-written request cannot mint an arena where the first
    /// turn buys the whole tech tree's worth of units.
    pub const MAX_YIELD: u32 = 999;
    /// The slowest tech pace worth offering; beyond it a battle ends first.
    pub const MAX_TURNS_PER_TECH: u32 = 99;
    /// The longest series offered. Beyond this a match is a tournament, and
    /// `civvis tournament` is the instrument for that.
    pub const MAX_BEST_OF: u32 = 21;
    /// Battle clocks offered by every Tactics setup surface, shortest first.
    /// 250 is the stock clock — an Online-speed game's length, long enough
    /// that two armies with no reinforcements coming settle it on the field
    /// rather than on the bell.
    pub const TURN_LIMITS: [u32; 9] = [10, 20, 30, 40, 50, 100, 150, 200, 250];
    /// The battle clock a new arena is given.
    pub const DEFAULT_TURN_LIMIT: u32 = 250;

    /// Clamp a requested economy to what the mode can actually play.
    pub fn sanitized(self) -> Self {
        Self {
            // A capture-the-flag battle has one objective, and it is not a
            // city: the flag replaces the city outright rather than joining
            // it, so the flag shape always plays city-less.
            cities: if self.flag { 0 } else { self.cities.min(1) },
            production: self.production.min(Self::MAX_YIELD),
            gold: self.gold.min(Self::MAX_YIELD),
            turns_per_tech: self.turns_per_tech.min(Self::MAX_TURNS_PER_TECH),
            turn_limit: *Self::TURN_LIMITS
                .iter()
                .min_by_key(|limit| (self.turn_limit.abs_diff(**limit), **limit))
                .expect("the Tactics turn-limit ladder is nonempty"),
            // Odd, so a series cannot be split evenly by wins. Drawn battles
            // can still exhaust the schedule without either side reaching
            // `wins_needed`; that is an intentionally drawn match.
            best_of: (self.best_of.max(1) | 1).min(Self::MAX_BEST_OF),
            unique_units: self.unique_units,
            fog: self.fog,
            flag: self.flag,
            heal: self.heal,
            era: self.era.sanitized(),
        }
    }

    /// The economy this map is actually fought under.
    ///
    /// An arena's grants are the player's to choose. A scenario's are not:
    /// nobody launched a ship of the line into Trafalgar, nobody refitted a
    /// three-decker into a Dreadnought at noon, and the two fleets that were
    /// there are the two fleets that fight. So every yield is stripped, the
    /// cities and the flag with them — the battle is decided by the sixty
    /// ships on the water and by nothing else — and research is stopped so
    /// the fleets stay 1805 fleets for the whole action. Unique units go too:
    /// England's Sea Dog was not at Trafalgar either.
    ///
    /// What the player still owns: the battle clock and the length of the
    /// series. Both are how long you want to play for rather than claims
    /// about the battle. Fog is off, and deliberately — the Combined Fleet
    /// could see the British columns coming for four hours and its problem
    /// was never finding them.
    /// A naval battlefield keeps its city, and deliberately. A galley is a
    /// naval *melee* unit and takes a coastal city exactly as a warrior takes
    /// an inland one — the rule is Civ 6's and the engine already carries it
    /// — so the port is a real objective at sea rather than an unreachable
    /// one, and measured, it is the only thing that makes the two fleets sail
    /// at each other at all: stripped of it, both squadrons sat in their own
    /// anchorages for four hundred turns and lost one ship between them.
    pub fn for_script(self, script: MapScript) -> Self {
        if !script.is_scenario() {
            return self.sanitized();
        }
        Self {
            cities: 0,
            production: 0,
            gold: 0,
            turns_per_tech: 0,
            unique_units: false,
            fog: false,
            flag: false,
            heal: false,
            // The battle brings its own era: Trafalgar is 1805 whatever the
            // lobby's era control asked for.
            era: TacticsEra::Start,
            ..self.sanitized()
        }
    }

    /// Battles one side must win to take the match.
    pub fn wins_needed(self) -> u32 {
        self.sanitized().best_of / 2 + 1
    }
}

impl Default for TacticsRules {
    /// Two standing armies and nothing behind them: each side keeps the one
    /// city it is dropped in with — the objective the other side is coming
    /// for — but the city collects no Production and the side banks no Gold,
    /// so no unit that was not on the field at turn one ever joins it, and
    /// nothing is upgraded mid-battle. A technology every five turns still
    /// moves the tree, but with no money to upgrade and nothing to build the
    /// battle is decided by the armies as dealt. Production and Gold remain
    /// the player's to raise from every setup surface when reinforcements
    /// are wanted; this is the stock arena, not the only one. Fogged, so
    /// finding the enemy is part of the battle rather than given, and a
    /// 250-turn clock so two armies that must be careful with themselves
    /// have room to settle it on the field.
    ///
    /// Until 2026-08-15 the stock arena granted 30 Production and 30 Gold a
    /// turn on a 100-turn clock; the rating profile carries the grants and
    /// the ledgers written under it stay matched to their own arena.
    fn default() -> Self {
        Self {
            cities: 1,
            production: 0,
            gold: 0,
            turns_per_tech: 5,
            turn_limit: Self::DEFAULT_TURN_LIMIT,
            best_of: 1,
            unique_units: false,
            fog: true,
            flag: false,
            heal: false,
            era: TacticsEra::Start,
        }
    }
}

/// The running score of a Tactics match: one series of battles between the
/// same two civilizations.
///
/// Kept by civilization rather than by seat, because a match swaps the sides
/// over between battles. Whichever end of the field a civilization is set
/// down on, the battle it wins is its own — and a series in which one side
/// held the same corner every time would be measuring the corner.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MatchSeries {
    /// Battles the match is played over. Always odd, so wins cannot split it
    /// evenly; drawn battles can still use the series up with no match winner.
    pub best_of: u32,
    /// Battles won, by civilization, in roster order.
    pub wins: BTreeMap<String, u32>,
    /// Battles that reached the selected turn limit with neither side
    /// eliminated, counted rather than silently dropped so the played total
    /// always adds up.
    pub drawn: u32,
}

impl MatchSeries {
    /// A fresh match between these civilizations.
    pub fn new(best_of: u32, contenders: impl IntoIterator<Item = String>) -> Self {
        Self {
            best_of: (best_of.max(1) | 1).min(TacticsRules::MAX_BEST_OF),
            wins: contenders.into_iter().map(|civ| (civ, 0)).collect(),
            drawn: 0,
        }
    }

    /// Battles one civilization must win to take the match.
    pub fn wins_needed(&self) -> u32 {
        self.best_of / 2 + 1
    }

    pub fn played(&self) -> u32 {
        self.wins.values().sum::<u32>() + self.drawn
    }

    /// Record a finished battle. `None` is a battle nobody won.
    pub fn record(&mut self, winner: Option<&str>) {
        match winner {
            Some(civ) => *self.wins.entry(civ.to_string()).or_insert(0) += 1,
            None => self.drawn += 1,
        }
    }

    /// The civilization that has taken the match, if one has.
    pub fn winner(&self) -> Option<&str> {
        let needed = self.wins_needed();
        self.wins
            .iter()
            .find(|(_, won)| **won >= needed)
            .map(|(civ, _)| civ.as_str())
    }

    /// Whether the match is over — either somebody has clinched it, or every
    /// battle has been played and the draws have used the series up.
    pub fn decided(&self) -> bool {
        self.winner().is_some() || self.played() >= self.best_of
    }

    /// "Greece 3 – 1 Egypt", or "Greece 2 – 1 Egypt (best of 5)" mid-match.
    pub fn scoreline(&self) -> String {
        let mut sides: Vec<(&String, &u32)> = self.wins.iter().collect();
        // Leader first, then roster order, so the line reads as a result.
        sides.sort_by(|(first_civ, first), (second_civ, second)| {
            second.cmp(first).then_with(|| first_civ.cmp(second_civ))
        });
        let score = sides
            .iter()
            .map(|(civ, won)| format!("{civ} {won}"))
            .collect::<Vec<_>>()
            .join(" – ");
        let drawn = if self.drawn > 0 {
            format!(", {} drawn", self.drawn)
        } else {
            String::new()
        };
        if self.decided() {
            format!("{score}{drawn}")
        } else {
            format!("{score}{drawn} (best of {})", self.best_of)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSpeed {
    Online,
    Quick,
    #[default]
    Standard,
    Epic,
    Marathon,
}

impl GameSpeed {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Quick => "quick",
            Self::Standard => "standard",
            Self::Epic => "epic",
            Self::Marathon => "marathon",
        }
    }

    /// Percentage of Standard costs and turn durations.
    pub const fn cost_percent(self) -> u32 {
        match self {
            Self::Online => 50,
            Self::Quick => 67,
            Self::Standard => 100,
            Self::Epic => 150,
            Self::Marathon => 300,
        }
    }

    pub const fn turn_limit(self) -> u32 {
        match self {
            Self::Online => 250,
            Self::Quick => 330,
            Self::Standard => 500,
            Self::Epic => 750,
            Self::Marathon => 1500,
        }
    }

    pub fn scale(self, standard: f64) -> f64 {
        standard * self.cost_percent() as f64 / 100.0
    }

    /// Percentage a turn *duration* scales by, which is not the cost curve.
    /// `GameSpeed_Scalings` keeps a separate SCALING_HALF row per speed for
    /// exactly this, and it is far gentler: Marathon doubles a duration where
    /// it triples a cost, and Online keeps two thirds of one where it pays
    /// half of the other.
    pub const fn duration_percent(self) -> u32 {
        match self {
            Self::Online => 66,
            Self::Quick => 87,
            Self::Standard => 100,
            Self::Epic => 125,
            Self::Marathon => 200,
        }
    }

    /// Turns a duration of `standard` Standard-speed turns lasts at this
    /// speed. `GameSpeed_Durations` spells out every duration the shipped
    /// rules actually use and does not always agree with the multiplier -- 30
    /// turns becomes 25 on Quick where 87% would give 26 -- so the table wins
    /// and `duration_percent` covers anything outside it.
    pub fn scale_turns(self, standard: u32) -> u32 {
        // NumberOfTurnsOnStandard, then Online / Quick / Standard / Epic /
        // Marathon. Standard is the key itself; the table ships no row for it.
        const DURATIONS: [(u32, [u32; 5]); 6] = [
            (5, [5, 5, 5, 10, 15]),
            (10, [8, 9, 10, 15, 25]),
            (15, [10, 12, 15, 20, 30]),
            (29, [19, 24, 29, 39, 59]),
            (30, [20, 25, 30, 40, 60]),
            (60, [40, 50, 60, 80, 120]),
        ];
        let column = match self {
            Self::Online => 0,
            Self::Quick => 1,
            Self::Standard => 2,
            Self::Epic => 3,
            Self::Marathon => 4,
        };
        if let Some((_, scaled)) = DURATIONS.iter().find(|(turns, _)| *turns == standard) {
            return scaled[column];
        }
        (standard as u64 * self.duration_percent() as u64)
            .div_ceil(100)
            .max(1) as u32
    }

    pub fn from_id(id: &str) -> Option<Self> {
        CIV6_GAME_SPEEDS
            .iter()
            .find(|speed| speed.id == id)
            .map(|speed| speed.speed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GameSpeedSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub cost_percent: u32,
    pub turn_limit: u32,
    pub description: &'static str,
    #[serde(skip)]
    pub speed: GameSpeed,
}

pub const CIV6_GAME_SPEEDS: [GameSpeedSpec; 5] = [
    GameSpeedSpec {
        id: "online",
        name: "Online",
        cost_percent: 50,
        turn_limit: 250,
        description: "Double-speed game for online play.",
        speed: GameSpeed::Online,
    },
    GameSpeedSpec {
        id: "quick",
        name: "Quick",
        cost_percent: 67,
        turn_limit: 330,
        description: "Quick game (33% faster).",
        speed: GameSpeed::Quick,
    },
    GameSpeedSpec {
        id: "standard",
        name: "Standard",
        cost_percent: 100,
        turn_limit: 500,
        description: "Normal game speed.",
        speed: GameSpeed::Standard,
    },
    GameSpeedSpec {
        id: "epic",
        name: "Epic",
        cost_percent: 150,
        turn_limit: 750,
        description: "Prolonged game (50% slower).",
        speed: GameSpeed::Epic,
    },
    GameSpeedSpec {
        id: "marathon",
        name: "Marathon",
        cost_percent: 300,
        turn_limit: 1500,
        description: "Very prolonged game (200% slower).",
        speed: GameSpeed::Marathon,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MapSize {
    pub id: &'static str,
    pub name: &'static str,
    pub width: i32,
    pub height: i32,
    /// Subdivision frequency Planet builds this size's globe at, chosen so the
    /// globe holds within a few percent of the tiles the rectangle does. A
    /// frequency-`n` globe has `10n² + 2` tiles.
    pub globe_frequency: i32,
    pub default_players: usize,
    pub max_players: usize,
    pub default_city_states: usize,
    pub max_city_states: usize,
    pub max_religions: usize,
    pub natural_wonders: usize,
    pub continents: usize,
}

/// The six unmodified Civilization VI map-size rows (Base/Gameplay/Data/Maps.xml
/// plus the stock setup limits exposed by Advanced Setup), followed by four
/// larger worlds that Civilization VI does not ship.
///
/// The scaled rows are not invented numbers: every stock size holds about 580
/// tiles per major civilization on a 1.6:1 rectangle, seats exactly 1.5
/// city-states per major, and takes `players / 2` continents and
/// `players / 2 + 1` religions. Massive through Ludicrous continue all four
/// ratios. Only `natural_wonders` breaks the pattern, because a map cannot
/// draw more wonders than the ruleset defines: the largest worlds take the
/// whole 26-wonder catalogue and are correspondingly sparser in them.
pub const CIV6_MAP_SIZES: [MapSize; 10] = [
    MapSize {
        id: "duel",
        name: "Duel",
        width: 44,
        height: 26,
        globe_frequency: 11,
        default_players: 2,
        max_players: 4,
        default_city_states: 3,
        max_city_states: 6,
        max_religions: 2,
        natural_wonders: 2,
        continents: 1,
    },
    MapSize {
        id: "tiny",
        name: "Tiny",
        width: 60,
        height: 38,
        globe_frequency: 15,
        default_players: 4,
        max_players: 6,
        default_city_states: 6,
        max_city_states: 10,
        max_religions: 3,
        natural_wonders: 3,
        continents: 2,
    },
    MapSize {
        id: "small",
        name: "Small",
        width: 74,
        height: 46,
        globe_frequency: 18,
        default_players: 6,
        max_players: 10,
        default_city_states: 9,
        max_city_states: 14,
        max_religions: 4,
        natural_wonders: 4,
        continents: 3,
    },
    MapSize {
        id: "standard",
        name: "Standard",
        width: 84,
        height: 54,
        globe_frequency: 21,
        default_players: 8,
        max_players: 14,
        default_city_states: 12,
        max_city_states: 18,
        max_religions: 5,
        natural_wonders: 5,
        continents: 4,
    },
    MapSize {
        id: "large",
        name: "Large",
        width: 96,
        height: 60,
        globe_frequency: 24,
        default_players: 10,
        max_players: 16,
        default_city_states: 15,
        max_city_states: 22,
        max_religions: 6,
        natural_wonders: 6,
        continents: 5,
    },
    MapSize {
        id: "huge",
        name: "Huge",
        width: 106,
        height: 66,
        globe_frequency: 26,
        default_players: 12,
        max_players: 20,
        default_city_states: 18,
        max_city_states: 24,
        max_religions: 7,
        natural_wonders: 7,
        continents: 6,
    },
    MapSize {
        id: "massive",
        name: "Massive",
        width: 118,
        height: 74,
        globe_frequency: 30,
        default_players: 15,
        max_players: 20,
        default_city_states: 22,
        max_city_states: 30,
        max_religions: 8,
        natural_wonders: 8,
        continents: 7,
    },
    MapSize {
        id: "enormous",
        name: "Enormous",
        width: 136,
        height: 85,
        globe_frequency: 34,
        default_players: 20,
        max_players: 30,
        default_city_states: 30,
        max_city_states: 40,
        max_religions: 11,
        natural_wonders: 11,
        continents: 10,
    },
    MapSize {
        id: "colossal",
        name: "Colossal",
        width: 215,
        height: 135,
        globe_frequency: 54,
        default_players: 50,
        max_players: 75,
        default_city_states: 75,
        max_city_states: 100,
        max_religions: 26,
        natural_wonders: 26,
        continents: 25,
    },
    MapSize {
        id: "ludicrous",
        name: "Ludicrous",
        width: 305,
        height: 190,
        globe_frequency: 76,
        default_players: 100,
        max_players: 100,
        default_city_states: 150,
        max_city_states: 150,
        max_religions: 51,
        // `players / 2 + 1` is 51 here, capped at the roster the ruleset
        // carries. That cap was 26 while a quarter of the Natural Wonders
        // were missing; the full Civilization VI roster is 34.
        natural_wonders: 34,
        continents: 50,
    },
];

impl MapSize {
    /// Pick the smallest size whose default major-civilization count fits the
    /// requested game. Counts above Ludicrous retain Ludicrous' parameters.
    pub fn for_players(players: usize) -> &'static MapSize {
        CIV6_MAP_SIZES
            .iter()
            .find(|size| players <= size.default_players)
            .unwrap_or(&CIV6_MAP_SIZES[CIV6_MAP_SIZES.len() - 1])
    }

    pub fn from_dimensions(width: i32, height: i32) -> Option<&'static MapSize> {
        CIV6_MAP_SIZES.iter().find(|size| {
            (size.width == width && size.height == height)
                || (size.globe_width() == width && size.globe_height() == height)
        })
    }

    /// Columns in the rectangle Planet stores this size's globe in.
    pub const fn globe_width(&self) -> i32 {
        5 * self.globe_frequency
    }

    /// Rows in that rectangle, including the two single-tile pole rows.
    pub const fn globe_height(&self) -> i32 {
        2 * self.globe_frequency + 2
    }

    /// The rectangle a size uses under a given world shape: a globe is stored
    /// in a different shape from the cylinder a flat map lays out.
    pub const fn dimensions(&self, topology: MapTopology) -> (i32, i32) {
        if topology.is_globe() {
            (self.globe_width(), self.globe_height())
        } else {
            (self.width, self.height)
        }
    }
}

#[cfg(test)]
mod tests;
