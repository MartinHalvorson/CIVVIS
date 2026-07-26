//! Civilization VI's stock game-setup presets.
//!
//! Keep these values in one place: browser games, CLI games, map generation,
//! city-state defaults, religion limits, and observation metadata all consume
//! the same profile instead of maintaining subtly different tables.

use serde::{Deserialize, Serialize};

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
    /// its rivers around but never its coastlines.
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

/// Whether the world has cold ends.
///
/// With poles, latitude runs the climate: the middle of the world is its
/// hottest ground and every step towards an extreme is colder, ending in
/// tundra, snow and sea ice. Without them the world has no cold end at all —
/// no ice, no snow, no tundra — and what terrain a tile gets is decided by
/// rainfall alone, so jungle and desert reach the top and bottom rows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapPoles {
    #[default]
    Poles,
    NoPoles,
}

impl MapPoles {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Poles => "poles",
            Self::NoPoles => "no_poles",
        }
    }

    pub const fn has_poles(self) -> bool {
        matches!(self, Self::Poles)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "poles" | "on" | "true" => Some(Self::Poles),
            "no_poles" | "none" | "off" | "false" => Some(Self::NoPoles),
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
        name: "Poles",
        description: "Hottest across the middle of the world, colder towards each extreme, ending in tundra, snow and sea ice.",
        poles: MapPoles::Poles,
    },
    MapPolesSpec {
        id: "no_poles",
        name: "No poles",
        description: "No cold ends: one warm climate from edge to edge, with no snow, tundra or ice anywhere.",
        poles: MapPoles::NoPoles,
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
pub const CIV6_MAP_SCRIPTS: [MapScriptSpec; 9] = [
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

    use crate::game::{Action, Game, Item};

    use super::{
        GameSpeed, MapPoles, MapScript, MapSize, MapTopology, CIV6_GAME_SPEEDS, CIV6_MAP_SCRIPTS,
        CIV6_MAP_SIZES, MAP_POLES, MAP_TOPOLOGIES,
    };

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
    /// what fills the world. Only Earth overrules the shape, because Earth is
    /// drawn from real longitudes and latitudes and closes on itself.
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
        assert!(!MapPoles::NoPoles.has_poles());
        // A flat world is what a lobby gets if it says nothing, and a world
        // with poles is: both are what CIVVIS shipped before either was a
        // choice, so a client that has not been taught about them is unmoved.
        assert_eq!(MapTopology::default(), MapTopology::Flat);
        assert_eq!(MapPoles::default(), MapPoles::Poles);
        assert_eq!(MapScript::default(), MapScript::Pangaea);

        // Only Earth is the same world every game, and it is the only type
        // whose shape is not the lobby's to choose.
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
