//! Civilization VI's stock game-setup presets.
//!
//! Keep these values in one place: browser games, CLI games, map generation,
//! city-state defaults, religion limits, and observation metadata all consume
//! the same profile instead of maintaining subtly different tables.

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
/// Future is deliberately absent from the other end: a game that opens there
/// has nothing left to research and nowhere to go.
pub const START_ERAS: [StartEraSpec; 9] = [
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
];

/// The rungs a game can actually open on, earliest first.
pub fn playable_start_eras() -> impl Iterator<Item = &'static StartEraSpec> {
    START_ERAS.iter().filter(|spec| spec.is_playable())
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

/// What the world is made of, ordered from all land to all water.
///
/// The list is a spectrum rather than a menu: each entry down it leaves less
/// land than the one above, and breaks what land is left into more pieces.
/// Land Only and Water World are its two ends, at 95% of one and 95% of the
/// other, and the four Civ VI shapes everybody knows fill the middle. True
/// Start Earth sits outside the ordering because its coastlines are read
/// rather than rolled — Earth is whatever ratio Earth is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapScript {
    LandOnly,
    Lakes,
    InlandSea,
    GrandCanals,
    #[default]
    Pangaea,
    Continents,
    SmallContinents,
    Islands,
    WaterWorld,
    TrueStartEarth,
}

impl MapScript {
    pub const fn id(self) -> &'static str {
        match self {
            Self::LandOnly => "land_only",
            Self::Lakes => "lakes",
            Self::InlandSea => "inland_sea",
            Self::GrandCanals => "grand_canals",
            Self::Pangaea => "pangaea",
            Self::Continents => "continents",
            Self::SmallContinents => "small_continents",
            Self::Islands => "islands",
            Self::WaterWorld => "water_world",
            Self::TrueStartEarth => "true_start_earth",
        }
    }

    /// Whether the script draws a fixed world instead of rolling a new one.
    /// Earth is the same Earth every game, so the seed moves its resources and
    /// its rivers around but never its coastlines. Fixed geography is still
    /// independent of shape: it can be sampled onto a flat atlas or a globe.
    pub const fn is_fixed_geography(self) -> bool {
        matches!(self, Self::TrueStartEarth)
    }

    /// Roughly what share of the world this script leaves as land, as the
    /// generator aims for it. This is the order the list above is in, and the
    /// lobby prints it, so the two can never drift apart.
    pub const fn land_percent(self) -> u32 {
        match self {
            Self::LandOnly => 95,
            Self::Lakes => 81,
            Self::InlandSea => 68,
            // Ground nearly everywhere, less the six canals cut through it.
            // What the canals take is geometry rather than a share the
            // generator picks, so this is what is left once they have taken
            // it; see `mapgen::grand_canals`.
            Self::GrandCanals => 62,
            Self::Pangaea => 42,
            Self::Continents => 42,
            Self::SmallContinents => 36,
            Self::Islands => 22,
            Self::WaterWorld => 5,
            // Earth's own land share, which is the one number here that was
            // measured rather than chosen.
            Self::TrueStartEarth => 29,
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
            _ => {}
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

/// The world types in the order [`MapScript`] declares them: all land at the
/// top, all water at the bottom, Earth on the end.
pub const CIV6_MAP_SCRIPTS: [MapScriptSpec; 10] = [
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
        id: "grand_canals",
        name: "Grand Canals",
        description: "A world of solid ground cut by six canals that each circle it: two around the poles, two around each of the other two axes, crossing at twenty-four junctions.",
        script: MapScript::GrandCanals,
    },
    MapScriptSpec {
        id: "pangaea",
        name: "Pangaea",
        description: "One connected supercontinent surrounded by ocean.",
        script: MapScript::Pangaea,
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
    MapScriptSpec {
        id: "true_start_earth",
        name: "True Start Earth",
        description: "Earth itself, with every civilization founded in its own historic homeland.",
        script: MapScript::TrueStartEarth,
    },
];

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
        natural_wonders: 26,
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
mod tests {
    use std::collections::BTreeSet;

    use crate::game::{Action, Game, GameOptions, Item};

    use super::{
        last_start_era, playable_start_eras, start_era_from_id, start_era_id,
        stock_start_era_id, BaseRuleset, GameSpeed, MapPoles, MapScript, MapSize, MapTopology,
        BASE_RULESETS, CIV6_GAME_SPEEDS, CIV6_MAP_SCRIPTS, CIV6_MAP_SIZES, MAP_POLES,
        MAP_TOPOLOGIES, START_ERAS,
    };

    /// One ruleset is offered, and the setting still has to behave like a
    /// setting: it resolves by id, it round-trips, and an id from some other
    /// game is refused rather than quietly played as Civilization VI.
    #[test]
    fn the_base_ruleset_is_civ6_and_nothing_else_resolves() {
        assert_eq!(BASE_RULESETS.len(), 1);
        for spec in BASE_RULESETS {
            assert_eq!(BaseRuleset::from_id(spec.id), Some(spec.ruleset));
            assert_eq!(spec.ruleset.id(), spec.id);
        }
        assert_eq!(BaseRuleset::default(), BaseRuleset::Civ6);
        assert_eq!(BaseRuleset::Civ6.id(), "civ6");
        assert_eq!(BaseRuleset::from_id("civ5"), None);
        assert_eq!(BaseRuleset::from_id(""), None);
    }

    /// The ladder is a timeline and its order is a claim. The rungs that can
    /// be played are the ruleset's own eras, in the ruleset's own order — rung
    /// `n` *is* era `n`, which is what lets a start era cut the trees — and
    /// everything declared but unbuilt sits ahead of them, because what is
    /// missing from this list is prehistory rather than some later age.
    #[test]
    fn the_start_ladder_is_the_rulesets_own_eras_behind_the_prehistory_it_still_owes() {
        let mut seen = BTreeSet::new();
        let mut history_has_begun = false;
        for spec in START_ERAS {
            assert!(seen.insert(spec.id), "{} is listed twice", spec.id);
            // An unbuilt rung still has to be described, or the lobby has
            // nothing honest to show for it.
            assert!(!spec.description.is_empty(), "{} has no description", spec.id);
            match spec.era {
                Some(_) => history_has_begun = true,
                // Prehistory precedes history: no unbuilt rung may appear
                // after a playable one, or the ladder stops being a timeline.
                None => assert!(!history_has_begun, "{} sits after a playable era", spec.id),
            }
        }

        let playable: Vec<_> = playable_start_eras().collect();
        assert_eq!(playable.len(), crate::rules::ERA_NAMES.len() - 1);
        for (index, spec) in playable.iter().enumerate() {
            assert_eq!(spec.era, Some(index), "{}", spec.id);
            assert_eq!(spec.id, crate::rules::ERA_NAMES[index], "era {index}");
            assert_eq!(start_era_from_id(spec.id), Some(index));
            assert_eq!(start_era_id(index), spec.id);
        }
        assert_eq!(stock_start_era_id(), "ancient");
        assert_eq!(last_start_era(), playable.len() - 1);
        // Future is deliberately not a start, and an era off the end of the
        // ladder names the stock start rather than nothing at all.
        assert_eq!(start_era_from_id("future"), None);
        assert_eq!(start_era_id(playable.len()), "ancient");
        assert_eq!(start_era_from_id("holocene"), None);
    }

    /// The Stone Age is on the ladder ahead of Ancient and cannot be played
    /// yet. Being listed is the point — a lobby that opens on "Ancient" with
    /// nothing above it quietly claims human history begins with a Settler —
    /// but it resolves to nothing rather than to the stock start, so nobody is
    /// handed the Ancient era while believing they asked for something else.
    #[test]
    fn the_stone_age_is_offered_ahead_of_ancient_and_refused_rather_than_substituted() {
        let stone_age = START_ERAS[0];
        assert_eq!(stone_age.id, "stone_age");
        assert_eq!(START_ERAS[1].id, "ancient");
        assert!(!stone_age.is_playable());
        assert_eq!(stone_age.era, None);
        assert_eq!(start_era_from_id("stone_age"), None);
        // It is not one of the ruleset's eras under another name, so no index
        // can ever resolve back to it.
        assert!(!crate::rules::ERA_NAMES.contains(&stone_age.id));
        assert!((0..crate::rules::ERA_NAMES.len()).all(|era| start_era_id(era) != stone_age.id));

        // The lobby is told whether a rung can be chosen, and never the index
        // behind it: resolving an id stays the server's job.
        let wire = serde_json::to_value(START_ERAS).unwrap();
        assert_eq!(wire[0]["id"], "stone_age");
        assert_eq!(wire[0]["name"], "Stone Age");
        assert_eq!(wire[0]["playable"], false);
        assert_eq!(wire[1]["playable"], true);
        assert!(wire[0].get("era").is_none(), "the era index reached the wire");
    }

    /// A world set up to open in a later age of human history, built through
    /// the one real constructor rather than described in the lobby.
    fn world_opening_in(era: usize) -> Game {
        let size = MapSize::for_players(2);
        Game::new_with(GameOptions {
            barbarians: false,
            start_era: era,
            city_states: size.default_city_states,
            ..GameOptions::new(2, size.width, size.height, 909, 250, size.default_city_states)
        })
    }

    /// The start era is the setting that actually changes the game: a world
    /// that opens in the Renaissance opens with everything before it known,
    /// by everyone on the board, with an army to match — and it says it is in
    /// the Renaissance from its first turn, rather than reporting the era it
    /// has just finished.
    #[test]
    fn a_game_that_opens_past_the_first_age_starts_with_the_earlier_eras_behind_it() {
        let ancient = world_opening_in(0);
        assert_eq!(ancient.world_era, 0);
        assert_eq!(ancient.start_era, 0);
        assert!(ancient.players.iter().all(|player| player.techs.is_empty()));

        let era = start_era_from_id("renaissance").unwrap();
        let renaissance = world_opening_in(era);
        assert_eq!(renaissance.start_era, era);
        assert_eq!(renaissance.world_era, era);
        let earlier: BTreeSet<&String> = renaissance
            .rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.era < era)
            .map(|(name, _)| name)
            .collect();
        assert!(!earlier.is_empty());
        // Majors and city-states alike: a minor still holding Ancient spears
        // in a Renaissance world is free conquest, not a setting.
        for player in renaissance.players.iter().filter(|player| !player.is_barbarian) {
            for tech in &earlier {
                assert!(player.techs.contains(*tech), "{} lacks {tech}", player.civ);
            }
            assert!(
                player
                    .techs
                    .iter()
                    .all(|tech| renaissance.rules.techs[tech].era < era),
                "{} was handed a technology of its own era or later",
                player.civ
            );
            assert!(
                player
                    .civics
                    .iter()
                    .all(|civic| renaissance.rules.civics[civic].era < era),
                "{} was handed a civic of its own era or later",
                player.civ
            );
            // Nothing may be left researching what it already knows.
            assert!(player
                .research
                .as_ref()
                .is_none_or(|tech| !player.techs.contains(tech)));
        }
        // The starting army came up its own upgrade chain with the research.
        let kinds: BTreeSet<&str> = renaissance
            .units
            .values()
            .map(|unit| unit.kind.as_str())
            .collect();
        assert!(!kinds.contains("warrior"), "a Renaissance world still opens on Warriors: {kinds:?}");
        assert!(kinds.contains("settler"), "the Settler is not an upgradeable unit: {kinds:?}");

        // The whole setup survives a save, or a reloaded world would quietly
        // fall back to the Ancient era and undo its own floor.
        let restored: Game =
            serde_json::from_str(&serde_json::to_string(&renaissance).unwrap()).unwrap();
        assert_eq!(restored.start_era, era);
        assert_eq!(restored.base_ruleset, BaseRuleset::Civ6);
    }

    /// A rung past the end of the ladder is clamped rather than fatal, and may
    /// not produce a world that claims an era it cannot play.
    #[test]
    fn a_rung_past_the_end_of_the_ladder_opens_at_the_last_age_that_exists() {
        let last = last_start_era();
        let beyond = world_opening_in(last + 5);
        assert_eq!(beyond.start_era, last);
        assert_eq!(beyond.world_era, last);
    }

    /// The world types are a spectrum, and the lobby lists them along it: the
    /// first entry is the one with the most land and the last rolled entry the
    /// one with the least. Nothing else in the setup screen orders itself, so
    /// this is the one list whose order is a claim, and the claim is checked
    /// here rather than trusted to whoever edits the table next.
    #[test]
    fn the_world_types_are_listed_from_all_land_to_all_water() {
        let rolled: Vec<&super::MapScriptSpec> = CIV6_MAP_SCRIPTS
            .iter()
            .filter(|spec| !spec.script.is_fixed_geography())
            .collect();
        for pair in rolled.windows(2) {
            let (above, below) = (pair[0], pair[1]);
            assert!(
                above.script.land_percent() >= below.script.land_percent(),
                "{} ({}% land) is listed above {} ({}% land)",
                above.name,
                above.script.land_percent(),
                below.name,
                below.script.land_percent()
            );
        }
        // The two ends are the ones the ordering is anchored on.
        assert_eq!(rolled.first().map(|spec| spec.script), Some(MapScript::LandOnly));
        assert_eq!(rolled.last().map(|spec| spec.script), Some(MapScript::WaterWorld));
        assert_eq!(MapScript::LandOnly.land_percent(), 95);
        assert_eq!(MapScript::WaterWorld.land_percent(), 5);
        // Earth is outside the ordering, and is listed after all of it.
        assert_eq!(
            CIV6_MAP_SCRIPTS.last().map(|spec| spec.script),
            Some(MapScript::TrueStartEarth)
        );
        // Every type is reachable by the id the protocol carries, and the list
        // holds each of them exactly once.
        let mut seen = BTreeSet::new();
        for spec in CIV6_MAP_SCRIPTS {
            assert_eq!(MapScript::from_id(spec.id), Some(spec.script), "{}", spec.id);
            assert_eq!(spec.script.id(), spec.id);
            assert!(seen.insert(spec.id), "{} is listed twice", spec.id);
        }
    }

    /// The world's shape and its poles are settings of their own, orthogonal to
    /// what fills the world. This includes fixed geography: Earth's known
    /// longitudes and latitudes work on either projection.
    #[test]
    fn the_world_shape_and_its_poles_are_asked_for_separately_from_the_world_type() {
        for spec in MAP_TOPOLOGIES {
            assert_eq!(MapTopology::from_id(spec.id), Some(spec.topology));
            assert_eq!(spec.topology.id(), spec.id);
        }
        for spec in MAP_POLES {
            assert_eq!(MapPoles::from_id(spec.id), Some(spec.poles));
            assert_eq!(spec.poles.id(), spec.id);
        }
        assert!(MapTopology::Planet.is_globe());
        assert!(!MapTopology::Flat.is_globe());
        assert!(MapPoles::Poles.has_poles());
        // Randomized heat has cold ground but no cold *ends*, so it does not
        // get the polar sea-ice band or the polar cap on start placement.
        assert!(!MapPoles::Randomized.has_poles());
        // Heat is laid out two ways and no more: either latitude decides it or
        // noise does. A world with no cold end at all is not on offer, and the
        // spellings that used to ask for one — including the `off` of the era
        // when this was a boolean — name nothing rather than quietly landing on
        // a world nobody asked for.
        assert_eq!(MAP_POLES.len(), 2);
        assert_eq!(MAP_POLES[0].poles, MapPoles::Poles);
        assert_eq!(MAP_POLES[1].poles, MapPoles::Randomized);
        assert_eq!(MapPoles::from_id("on"), Some(MapPoles::Poles));
        assert_eq!(MapPoles::from_id("randomized"), Some(MapPoles::Randomized));
        for retired in ["no_poles", "none", "off", "false"] {
            assert_eq!(MapPoles::from_id(retired), None, "{retired} still names a world");
        }
        assert_eq!(MapPoles::from_id("hot_and_cold"), None);
        // A save is not a lobby. A checkpoint written while that world was on
        // offer still holds a whole game, so it reads back as the default
        // rather than failing to load at all.
        assert_eq!(
            serde_json::from_str::<MapPoles>("\"no_poles\"").unwrap(),
            MapPoles::Poles
        );
        // A flat world is what a lobby gets if it says nothing, and a world
        // with poles is: both are what CIVVIS shipped before either was a
        // choice, so a client that has not been taught about them is unmoved.
        assert_eq!(MapTopology::default(), MapTopology::Flat);
        assert_eq!(MapPoles::default(), MapPoles::Poles);
        assert_eq!(MapScript::default(), MapScript::Pangaea);

        // Only Earth is the same world every game. That says how its land is
        // chosen, not which of the independently selected shapes receives it.
        for spec in CIV6_MAP_SCRIPTS {
            assert_eq!(
                spec.script.is_fixed_geography(),
                spec.script == MapScript::TrueStartEarth,
                "{}",
                spec.id
            );
        }

        // `planet` named a world type before the globe became a shape of its
        // own. The old name still resolves, to the type that script generated.
        assert_eq!(MapScript::from_id("planet"), Some(MapScript::SmallContinents));
        assert_eq!(MapTopology::from_id("planet"), Some(MapTopology::Planet));
        assert_eq!(MapScript::from_id("pangea"), Some(MapScript::Pangaea));

        // Every size resolves from either shape of rectangle, and the two
        // shapes never collide across sizes.
        for size in CIV6_MAP_SIZES {
            assert_eq!(
                MapSize::from_dimensions(size.width, size.height).map(|found| found.id),
                Some(size.id)
            );
            assert_eq!(
                MapSize::from_dimensions(size.globe_width(), size.globe_height())
                    .map(|found| found.id),
                Some(size.id)
            );
            assert_eq!(
                size.dimensions(MapTopology::Planet),
                (size.globe_width(), size.globe_height())
            );
            assert_eq!(size.dimensions(MapTopology::Flat), (size.width, size.height));
        }
    }

    #[test]
    fn stock_game_speeds_scale_costs_durations_and_turn_limits() {
        // CostMultiplier and the GameSpeed_Turns increments summed, then the
        // SCALING_HALF DefaultCostMultiplier, then the GameSpeed_Durations row
        // for a 30-turn duration -- the length of an alliance and of a World
        // Congress session, so the one that matters most.
        let expected = [
            (GameSpeed::Online, 50, 250, 66, 20),
            (GameSpeed::Quick, 67, 330, 87, 25),
            (GameSpeed::Standard, 100, 500, 100, 30),
            (GameSpeed::Epic, 150, 750, 125, 40),
            (GameSpeed::Marathon, 300, 1500, 200, 60),
        ];
        assert_eq!(CIV6_GAME_SPEEDS.len(), expected.len());
        for (speed, percent, turns, duration_percent, thirty) in expected {
            assert_eq!(speed.cost_percent(), percent);
            assert_eq!(speed.turn_limit(), turns);
            assert_eq!(speed.scale(100.0), percent as f64);
            assert_eq!(speed.duration_percent(), duration_percent);
            assert_eq!(speed.scale_turns(30), thirty);
            assert_eq!(GameSpeed::from_id(speed.id()), Some(speed));
        }

        // A duration is not a cost. Marathon triples what a Settler costs but
        // only doubles how long a peace treaty holds, and Online halves the
        // cost while keeping two thirds of the duration.
        assert_eq!(GameSpeed::Marathon.scale(100.0), 300.0);
        assert_eq!(GameSpeed::Marathon.scale_turns(60), 120);
        assert_eq!(GameSpeed::Online.scale(100.0), 50.0);
        assert_eq!(GameSpeed::Online.scale_turns(60), 40);

        // Every row of GameSpeed_Durations, which does not follow the
        // multiplier: 87% of 29 is 26 but the table says 24, and no speed
        // shortens a 5-turn duration at all.
        let rows: [(u32, [u32; 5]); 6] = [
            (5, [5, 5, 5, 10, 15]),
            (10, [8, 9, 10, 15, 25]),
            (15, [10, 12, 15, 20, 30]),
            (29, [19, 24, 29, 39, 59]),
            (30, [20, 25, 30, 40, 60]),
            (60, [40, 50, 60, 80, 120]),
        ];
        let order = [
            GameSpeed::Online,
            GameSpeed::Quick,
            GameSpeed::Standard,
            GameSpeed::Epic,
            GameSpeed::Marathon,
        ];
        for (standard, scaled) in rows {
            for (speed, want) in order.iter().zip(scaled) {
                assert_eq!(speed.scale_turns(standard), want, "{speed:?} for {standard}");
            }
        }
    }

    #[test]
    fn every_speed_is_applied_to_live_research_growth_and_production_costs() {
        let size = MapSize::for_players(2);
        let mut game = Game::new_with_setup(
            2,
            size.width,
            size.height,
            701,
            GameSpeed::Online.turn_limit(),
            0,
            MapScript::Pangaea,
            GameSpeed::Online,
            false,
        );
        let settler = game
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "settler")
            .unwrap()
            .id;
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let monument = Item::Building {
            building: "monument".to_string(),
        };
        for speed in [
            GameSpeed::Online,
            GameSpeed::Quick,
            GameSpeed::Standard,
            GameSpeed::Epic,
            GameSpeed::Marathon,
        ] {
            game.game_speed = speed;
            let multiplier = speed.cost_percent() as f64 / 100.0;
            assert_eq!(
                game.tech_cost("pottery"),
                game.rules.techs["pottery"].cost * multiplier
            );
            assert_eq!(game.growth_cost(1), 15.0 * multiplier);
            // Durations ride the separate SCALING_HALF curve, not `multiplier`.
            assert_eq!(game.standard_duration(30), speed.scale_turns(30));
            assert_eq!(
                game.item_cost_for_city(0, city, &monument),
                game.rules.buildings["monument"].cost * multiplier
            );
        }

        game.map_script = MapScript::SmallContinents;
        let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(restored.game_speed, GameSpeed::Marathon);
        assert_eq!(restored.map_script, MapScript::SmallContinents);
    }

    #[test]
    fn requested_player_counts_use_civ6_dimensions_and_defaults() {
        let tiny = MapSize::for_players(4);
        assert_eq!((tiny.name, tiny.width, tiny.height), ("Tiny", 60, 38));
        assert_eq!(
            (
                tiny.default_city_states,
                tiny.natural_wonders,
                tiny.max_religions,
                tiny.continents
            ),
            (6, 3, 3, 2)
        );

        let small = MapSize::for_players(6);
        assert_eq!((small.name, small.width, small.height), ("Small", 74, 46));
        assert_eq!(
            (
                small.default_city_states,
                small.natural_wonders,
                small.max_religions,
                small.continents
            ),
            (9, 4, 4, 3)
        );
    }

    #[test]
    fn dimensions_round_trip_for_every_stock_size() {
        for players in [2, 4, 6, 8, 10, 12, 15, 20, 50, 100] {
            let size = MapSize::for_players(players);
            assert_eq!(
                MapSize::from_dimensions(size.width, size.height),
                Some(size)
            );
        }
    }

    /// The four worlds past Huge are extrapolations, not shipped rows, so what
    /// pins them is the ratios they extrapolate from. Every stock size holds
    /// about 580 tiles per major civilization on a roughly 1.6:1 rectangle and
    /// seats exactly 1.5 city-states per major; a scaled row that quietly drifts
    /// off those is a world that no longer plays like a Civilization VI map.
    #[test]
    fn the_scaled_worlds_keep_the_stock_ratios_and_stay_inside_the_roster() {
        let scaled = [("massive", 15), ("enormous", 20), ("colossal", 50), ("ludicrous", 100)];
        for (id, players) in scaled {
            let size = CIV6_MAP_SIZES.iter().find(|size| size.id == id).unwrap();
            assert_eq!(size.default_players, players, "{id} seats");
            assert_eq!(MapSize::for_players(players).id, id, "{id} is chosen for {players}");

            let tiles = (size.width * size.height) as f64;
            let per_civ = tiles / players as f64;
            assert!(
                (560.0..=600.0).contains(&per_civ),
                "{id} gives each civilization {per_civ:.0} tiles, outside the stock 567-583 band"
            );
            let aspect = size.width as f64 / size.height as f64;
            assert!((1.55..=1.65).contains(&aspect), "{id} is {aspect:.2}:1");
            // Exactly 1.5 per major, rounded down: an odd seat count cannot
            // halve, and Massive's fifteen majors take 22 rather than 22.5.
            assert_eq!(
                size.default_city_states,
                players * 3 / 2,
                "{id} should seat 1.5 city-states per major"
            );
            assert_eq!(size.continents, players / 2, "{id} continents");

            // Nothing may seat more majors than there are civilizations to
            // seat, or more city-states than the ruleset has identities for.
            assert!(
                size.max_players <= crate::game::CIV_NAMES.len(),
                "{id} seats up to {} majors but the roster holds {}",
                size.max_players,
                crate::game::CIV_NAMES.len()
            );
            assert!(
                size.max_city_states <= crate::game::CITY_STATE_NAMES.len(),
                "{id} seats up to {} city-states but the ruleset names {}",
                size.max_city_states,
                crate::game::CITY_STATE_NAMES.len()
            );
            // Religions and natural wonders both follow `players / 2 + 1`, but
            // a map cannot draw more wonders than the ruleset defines, so that
            // one is the rule capped by the catalogue. Ludicrous is the only
            // size the cap actually binds.
            assert_eq!(size.max_religions, players / 2 + 1, "{id} religions");
            let catalogue = crate::rules::Rules::embedded()
                .features
                .values()
                .filter(|feature| feature.natural_wonder)
                .count();
            assert_eq!(
                size.natural_wonders,
                (players / 2 + 1).min(catalogue),
                "{id} should draw players/2+1 wonders, capped at the {catalogue} the ruleset carries"
            );
        }
    }

    fn assert_generated_profile(players: usize, seed: u64) {
        let size = MapSize::for_players(players);
        let mut game = Game::new_full(
            players,
            size.width,
            size.height,
            seed,
            50,
            size.default_city_states,
            false,
        );
        assert_eq!((game.map.width, game.map.height), (size.width, size.height));
        assert_eq!(game.map.tiles.len(), (size.width * size.height) as usize);
        assert_eq!(game.players.iter().filter(|p| !p.is_minor).count(), players);
        assert_eq!(
            game.players
                .iter()
                .filter(|p| p.is_minor && !p.is_barbarian)
                .count(),
            size.default_city_states
        );
        let wonders: BTreeSet<&str> = game
            .map
            .tiles
            .values()
            .filter_map(|tile| {
                let feature = tile.feature.as_deref()?;
                game.rules.features[feature]
                    .natural_wonder
                    .then_some(feature)
            })
            .collect();
        assert_eq!(
            wonders.len(),
            size.natural_wonders,
            "{} generated unexpected natural wonders: {wonders:?}",
            size.name
        );
        let continents: BTreeSet<usize> = game
            .map
            .tiles
            .values()
            .filter_map(|tile| tile.continent)
            .collect();
        assert_eq!(continents.len(), size.continents);
        assert_eq!(game.max_religions(), size.max_religions);

        for pid in 0..size.max_religions {
            game.players[pid].religion = Some(format!("Religion {pid}"));
        }
        if size.max_religions < players {
            let blocked = size.max_religions;
            game.players[blocked].prophet_pending = true;
            assert!(!game
                .legal_actions(blocked)
                .iter()
                .any(|action| matches!(action, Action::FoundReligion { .. })));
        }
    }

    #[test]
    fn every_selectable_world_generates_its_complete_profile() {
        for (players, seed) in [(2, 21), (4, 41), (6, 61), (8, 81), (10, 101), (12, 121)] {
            assert_generated_profile(players, seed);
        }
    }
}
