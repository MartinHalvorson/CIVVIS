//! Ruleset loaded from the shared JSON data files (embedded at compile time).
use serde::{Deserialize, Serialize};

use crate::name::Name;
use crate::rng::Rng;
use crate::specmap::SpecMap;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

fn default_true() -> bool {
    true
}

fn default_one_limit() -> Option<usize> {
    Some(1)
}

use crate::world::Tile;

/// Whether Gathering Storm permits Volcanic Soil on this ground.
///
/// `Feature_ValidTerrains` names every Grassland, Plains, Desert, Tundra and
/// Snow flat/hills variant, but no Mountain or water terrain. CIVVIS stores
/// hills separately, so the five base terrain names are the complete rule.
pub(crate) fn volcanic_soil_valid_terrain(tile: &Tile) -> bool {
    matches!(
        tile.terrain.as_str(),
        "grassland" | "plains" | "desert" | "tundra" | "snow"
    )
}

pub const ERA_NAMES: [&str; 9] = [
    "ancient",
    "classical",
    "medieval",
    "renaissance",
    "industrial",
    "modern",
    "atomic",
    "information",
    "future",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Yields {
    pub food: f64,
    pub production: f64,
    pub gold: f64,
    pub science: f64,
    pub culture: f64,
    pub faith: f64,
}

impl Yields {
    /// Multiply every yield by one factor.
    pub fn scale(&mut self, factor: f64) {
        self.food *= factor;
        self.production *= factor;
        self.gold *= factor;
        self.science *= factor;
        self.culture *= factor;
        self.faith *= factor;
    }

    pub fn add(&mut self, o: Yields) {
        self.food += o.food;
        self.production += o.production;
        self.gold += o.gold;
        self.science += o.science;
        self.culture += o.culture;
        self.faith += o.faith;
    }
    pub fn total(&self) -> f64 {
        self.food + self.production + self.gold + self.science + self.culture + self.faith
    }

    pub fn add_scaled(&mut self, other: Yields, scale: f64) {
        self.food += other.food * scale;
        self.production += other.production * scale;
        self.gold += other.gold * scale;
        self.science += other.science * scale;
        self.culture += other.culture * scale;
        self.faith += other.faith * scale;
    }
}

fn dtrue() -> bool {
    true
}
fn done() -> f64 {
    1.0
}
fn dsight() -> i32 {
    2
}
fn done_i() -> i64 {
    1
}
fn done_usize() -> usize {
    1
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TerrainSpec {
    #[serde(default)]
    pub yields: Yields,
    /// Synthetic terrain used only when an external partial-map source has not
    /// disclosed what occupies a coordinate. It makes no land/water claim.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unknown: bool,
    #[serde(default)]
    pub water: bool,
    #[serde(default = "dtrue")]
    pub passable: bool,
    #[serde(default = "done")]
    pub move_cost: f64,
    /// The shipped ``Terrains.DefenseModifier``, added to a defender's Combat
    /// Strength. Every Hills terrain ships 3 and everything else 0, so CIVVIS'
    /// hills flag carries it.
    #[serde(default)]
    pub defense: f64,
}

/// The exact multi-hex silhouette a Natural Wonder uses.
///
/// `Features.Tiles` only gives the area. Firaxis's default feature placement
/// uses compact triangles and diamonds, while `CustomPlacement` names the
/// exceptional straight Zhangye Danxia and Mount Roraima's triangle-and-tail.
/// Keeping that distinction in the rules prevents four-hex wonders from being
/// grown as arbitrary paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WonderShape {
    #[default]
    Single,
    Adjacent,
    Triangle,
    /// A compact four-hex rhombus: two triangles sharing an edge.
    Diamond,
    /// Three hexes in one straight line (Zhangye Danxia).
    Straight,
    /// A three-hex line with a fourth hex beside its first edge.
    Roraima,
    /// Piopiotahi/Lysefjord's triangle with land behind and water in front.
    CoastalTriangle,
}

/// Where on the map a Natural Wonder is allowed to appear.
///
/// This is the shipped placement rule, transcribed from the game database:
/// `Feature_ValidTerrains` for the ground it stands on, `Feature_AdjacentTerrains`
/// / `Feature_NotAdjacentTerrains` (plus the `Coast` and `NoCoast` columns) for
/// the ground around it, `Feature_AdjacentFeatures` / `Feature_NotNearFeatures`
/// for the vegetation, `Features.Tiles` for how many hexes it covers, and
/// `MinDistanceLand` / `MaxDistanceLand` for how far offshore a water wonder
/// sits. It is what decides whether a wonder is eligible to be *rolled* for a
/// given map at all, so it is the input to the generation odds rather than a
/// cosmetic hint.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct FeaturePlacement {
    /// CIVVIS terrains the wonder can stand on.
    #[serde(default)]
    pub terrain: Vec<Name>,
    /// Base biomes allowed under `mountain`. Civ VI stores mountains as
    /// `TERRAIN_GRASS_MOUNTAIN`, `TERRAIN_DESERT_MOUNTAIN`, and so on, while
    /// CIVVIS renders them all as `mountain`; this retains the placement
    /// distinction. Empty means every mountain biome is valid.
    #[serde(default)]
    pub mountain_terrain: Vec<Name>,
    /// `Some(true)` for the hills-only wonders (the Cliffs of Dover),
    /// `Some(false)` for the flat-only ones (Crater Lake, Pantanal, Yosemite,
    /// the Dead Sea, Lake Retba, the Giant's Causeway), `None` when the shipped
    /// valid-terrain rows list both forms.
    #[serde(default)]
    pub hills: Option<bool>,
    /// `Features.Tiles`: the size of the wonder's footprint. Absent in the
    /// database means one hex.
    #[serde(default = "one_tile")]
    pub tiles: usize,
    /// The shipped default or `CustomPlacement` footprint for those tiles.
    #[serde(default)]
    pub shape: WonderShape,
    /// How many of those hexes are water rather than land. Only the Giant's
    /// Causeway, whose columns step off a headland into the sea, sets this.
    #[serde(default)]
    pub water_tiles: usize,
    /// At least one neighbour must be one of these terrains. Carries the
    /// `Coast` column for the shore wonders and `Feature_AdjacentTerrains` for
    /// the peaks, which must not be walled in by their own mountain range.
    #[serde(default)]
    pub adjacent_terrain: Vec<Name>,
    /// No neighbour may be one of these. Carries `NoCoast` as `coast`.
    #[serde(default)]
    pub not_adjacent_terrain: Vec<Name>,
    /// At least one neighbour must carry one of these features (Yosemite wants
    /// Woods, Ik-Kil wants Rainforest).
    #[serde(default)]
    pub adjacent_feature: Vec<Name>,
    /// `Feature_NotNearFeatures`: no neighbour may carry one of these. Every
    /// shipped row names Sea Ice, keeping the water wonders out of the pack.
    #[serde(default)]
    pub avoid_feature: Vec<Name>,
    /// `NoAdjacentFeatures`: no neighbour may carry *any* feature.
    #[serde(default)]
    pub no_adjacent_features: bool,
    /// `NoRiver`: the hex may not have a river on it.
    #[serde(default)]
    pub no_river: bool,
    /// `MinDistanceLand` / `MaxDistanceLand`: how many hexes of open water lie
    /// between this wonder and the nearest land. The Great Barrier Reef and Ha
    /// Long Bay hug the shore at 1, the Galapagos sit 2-3 hexes out.
    #[serde(default)]
    pub land_distance: Option<[i32; 2]>,
}

fn one_tile() -> usize {
    1
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FeatureSpec {
    #[serde(default)]
    pub yields: Yields,
    /// `Features.Appeal`: what standing beside this does to a tile. Most
    /// natural wonders are +2, but the Cliffs of Dover and Uluru are +4.
    #[serde(default)]
    pub appeal: f64,
    /// Natural wonders whose Civilopedia entry reads "to adjacent tiles"
    /// project these yields onto each neighbouring tile instead of their own.
    #[serde(default)]
    pub adjacent_yields: Yields,
    /// Movement added on top of the terrain cost, the game database's
    /// ``MovementChange`` column: Woods on Hills costs 1 + 1 + 1 = 3 MP.
    #[serde(default)]
    pub move_cost: f64,
    #[serde(default)]
    pub natural_wonder: bool,
    /// `Features_XP2.Volcano`: this feature is a volcanic cone that can go
    /// active and erupt. Gathering Storm ships the flag on four features, not
    /// one — the generic Volcano and the three volcanic Natural Wonders,
    /// Vesuvius, Kilimanjaro and Eyjafjallajokull. Reading only the generic
    /// cone left the three wonders permanently dormant, so no eruption ever
    /// came out of them and no Volcanic Soil was ever laid down around one.
    #[serde(default)]
    pub volcano: bool,
    /// Civilization VI refuses a district on this feature and a Builder cannot
    /// remove it: the Oasis is the shipped case. The rule was half-modelled —
    /// city founding knew it, district siting did not — so run
    /// civvis-20260811T230324Z asked the host for a Campus on one oasis tile
    /// 40 times and was refused every time.
    #[serde(default)]
    pub blocks_district: bool,
    /// Present on every Natural Wonder and on nothing else: the ground the map
    /// generator is allowed to seat it on.
    #[serde(default)]
    pub placement: FeaturePlacement,
    /// How much this feature adds to the height of the terrain under it for
    /// line of sight, the game database's ``SightThroughModifier``: Woods and
    /// Rainforest 1 — burnt over as well as standing — Everest and Yosemite 2,
    /// and every other Natural Wonder 0.
    #[serde(default)]
    pub sight_through: i32,
    #[serde(default)]
    pub impassable: bool,
    /// The shipped ``Features.DefenseModifier``, added to a defender's Combat
    /// Strength: Woods, Rainforest and Reef 3, Floodplains and Marsh -2.
    #[serde(default)]
    pub defense: f64,
    /// The shipped Feature_Removes yields a Builder collects for clearing
    /// this feature (base values; the payout scales with the era).
    #[serde(default)]
    pub chop: BTreeMap<String, f64>,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub class: String,
    /// The shipped `Resources.Frequency` land-placement weight. A zero means
    /// this resource does not participate in the ordinary land lottery; some
    /// resources are placed by a dedicated quota pass instead.
    #[serde(default)]
    pub frequency: u32,
    /// The shipped `Resources.SeaFrequency` water-placement weight. Firaxis
    /// keeps this separate from the land value: Fish (23) and Crabs (17) are
    /// intentionally much more common than Pearls and Whales (1 each).
    #[serde(default)]
    pub sea_frequency: u32,
    /// Strategic and archaeological resources remain hidden until this node.
    #[serde(default)]
    pub tech: Option<Name>,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub yields: Yields,
    #[serde(default)]
    pub terrain: Vec<Name>,
    #[serde(default)]
    pub feature: Vec<Name>,
    /// Some(true) for hills-only spawns (Sheep), Some(false) for flat-only
    /// (Wheat, Rice, Maize, Bananas), None when either form works.
    #[serde(default)]
    pub hills: Option<bool>,
    /// The shipped Resource_Harvests row: only these bonus resources can be
    /// harvested by a Builder, for this yield, from this technology on.
    #[serde(default)]
    pub harvest: Option<HarvestSpec>,
    /// Empty for luxuries no tile improvement works (Toys, Jeans, Perfume,
    /// Cosmetics — manufactured, never map-placed).
    #[serde(default)]
    pub improvement: String,
    /// The city effect associated with an Industry on this Luxury. A
    /// Corporation applies it twice and every housed Product applies it once.
    /// Keeping the selector on the resource makes all 28 Product projects use
    /// one execution path instead of a hardcoded resource-name switch.
    #[serde(default)]
    pub industry_effects: ResourceIndustryEffects,
    /// Effect attached to each housed Product. Usually this is one Industry
    /// bundle, but it is kept explicit because shipped Product modifiers are
    /// their own rows (Coffee is the notable distinct value).
    #[serde(default)]
    pub product_effects: ResourceIndustryEffects,
    /// Flat Great Work yields of one housed Product. These are distinct from
    /// the percentage/production/growth effect above and both apply.
    #[serde(default)]
    pub product_yields: Yields,
    /// What the Moon holds of this resource, if the ruleset says it holds any.
    /// Absent everywhere in the stock data: the Moon is a milestone there and
    /// not a place with ore in it. The Modified Future Era is what fills this
    /// in, and a resource without it is simply not on the Moon.
    #[serde(default)]
    pub lunar: Option<LunarDeposit>,
}

/// How much of one resource the Moon spawns with, as the range a game rolls
/// from.
///
/// There is one Moon and one set of piles on it, shared by everybody: this is
/// rolled once at setup, not once per civilization, and it is drawn down by
/// whoever gets a mass driver over it first. Unlike almost everything else a
/// civilization spends, it does not scale with game speed — the ore is a
/// physical quantity of rock, and a Quick game is a shorter race for the same
/// Moon rather than a smaller one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LunarDeposit {
    pub min: f64,
    pub max: f64,
}

/// One Industry-sized economic effect for a Luxury resource. Civ VI doubles
/// this bundle for the Corporation improvement and attaches one bundle to
/// each Product housed in a Stock Exchange or Seaport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceIndustryEffects {
    pub city_yield_pct: Yields,
    pub growth_pct: f64,
    pub housing: f64,
    pub military_unit_production_pct: f64,
    pub civilian_unit_production_pct: f64,
    pub building_production_pct: f64,
}

impl ResourceIndustryEffects {
    pub fn add_scaled(&mut self, other: Self, scale: f64) {
        self.city_yield_pct.add_scaled(other.city_yield_pct, scale);
        self.growth_pct += other.growth_pct * scale;
        self.housing += other.housing * scale;
        self.military_unit_production_pct += other.military_unit_production_pct * scale;
        self.civilian_unit_production_pct += other.civilian_unit_production_pct * scale;
        self.building_production_pct += other.building_production_pct * scale;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HarvestSpec {
    #[serde(rename = "yield")]
    pub yield_type: String,
    pub amount: f64,
    #[serde(default)]
    pub tech: Option<Name>,
}

/// Additional Standard-speed spoils from a pillage modifier. Non-healing
/// rewards scale with game speed and world era.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PillageReward {
    #[serde(rename = "yield")]
    pub yield_type: String,
    pub amount: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ImprovementSpec {
    #[serde(default)]
    pub tech: Option<Name>,
    /// `PlunderType` and `PlunderAmount`: which yield pillaging this pays and
    /// how much. Gold and heal pay 50, Science, Culture and Faith pay 25.
    #[serde(default)]
    pub plunder_type: Option<String>,
    #[serde(default)]
    pub plunder_amount: f64,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub yields: Yields,
    #[serde(default)]
    pub housing: f64,
    #[serde(default)]
    pub terrain: Vec<Name>,
    #[serde(default)]
    pub feature: Vec<Name>,
    /// Features the improvement may also sit on once a civic is unlocked --
    /// Gathering Storm opens the Lumber Mill to Rainforest at Mercantilism.
    #[serde(default)]
    pub feature_after_civic: BTreeMap<String, String>,
    #[serde(default)]
    pub resources: Vec<Name>,
    #[serde(default)]
    pub resource_only: bool,
    #[serde(default)]
    pub requires_hills: bool,
    #[serde(default)]
    pub hills_or_resource: bool,
    /// The plot may be Hills, or qualify through a valid resource or feature.
    /// Gathering Storm's Mine permits its normal resource route and Volcanic
    /// Soil as independent alternatives.
    #[serde(default)]
    pub hills_or_resource_or_feature: bool,
    /// The plot must be Hills unless it qualifies through a valid feature.
    /// Ethiopia's Rock-Hewn Church uses this for its Volcanic Soil route.
    #[serde(default)]
    pub hills_or_feature: bool,
    #[serde(default)]
    pub requires_flat: bool,
    /// Resource classes at least one adjacent plot must carry. Firaxis's
    /// `RequiresAdjacentBonusOrLuxury` uses both classes for the Mekewap.
    #[serde(default)]
    pub requires_adjacent_resource_classes: Vec<String>,
    /// Number of traversable land plots that must neighbour this improvement.
    /// Polders need three reclaimed-land neighbours; Portugal's Feitorias need
    /// one shore tile beside their water plot.
    #[serde(default)]
    pub requires_adjacent_passable_land: usize,
    /// Requires a neighbouring water tile that contains a resource. Indonesia's
    /// Kampung is the stock example.
    #[serde(default)]
    pub requires_adjacent_water_resource: bool,
    /// The improvement is placed in another civilization's territory rather
    /// than the builder owner's. This is deliberately a per-improvement rule:
    /// ordinary Builder work remains restricted to owned or suzerained land.
    #[serde(default)]
    pub requires_foreign_territory: bool,
    /// Foreign-territory placement additionally needs an Open Borders treaty.
    #[serde(default)]
    pub requires_open_borders: bool,
    /// Firaxis `CanBuildOutsideTerritory`: this improvement can also stand
    /// on neutral ground. Foreign owned land retains its existing rules.
    #[serde(default)]
    pub can_build_outside_territory: bool,
    /// Features that may not occur on any adjacent tile. Rapa Nui's Moai
    /// cannot stand beside Woods or Rainforest.
    #[serde(default)]
    pub forbids_adjacent_features: Vec<Name>,
    /// Minimum live tile Appeal required before this improvement is offered.
    /// Mapuche Chemamulls require a Breathtaking (4+) plot.
    #[serde(default)]
    pub min_appeal: Option<i32>,
    /// Some unique leisure and culture improvements are limited to one copy
    /// in each city territory, rather than one globally or per empire.
    #[serde(default)]
    pub one_per_city: bool,
    /// Firaxis `SameAdjacentValid`; false for improvements such as Sphinxes,
    /// Ski Resorts, City Parks, and Rock-Hewn Churches.
    #[serde(default = "default_true")]
    pub same_adjacent_valid: bool,
    #[serde(default)]
    pub unique_to: Option<String>,
    #[serde(default)]
    pub replaces: Option<Name>,
    #[serde(default)]
    pub removes_feature: bool,
    #[serde(default)]
    pub water: bool,
    #[serde(default)]
    pub unbuildable: bool,
    #[serde(default = "default_true")]
    pub builder_buildable: bool,
    /// Some improvements (Great Walls, Corporations, and National Parks) may
    /// be damaged by disasters but cannot be pillaged by units.
    #[serde(default = "default_true")]
    pub unit_pillageable: bool,
    /// Ability-keyed extra spoils, currently Harald's improvement bonuses.
    #[serde(default)]
    pub bonus_pillage: BTreeMap<String, PillageReward>,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UnitSpec {
    pub class: String,
    pub cost: f64,
    /// Gold paid every turn; formations apply their Civ VI 150%/200% factor.
    #[serde(default)]
    pub maintenance: f64,
    pub moves: f64,
    /// False for units which only enter play through a special effect.
    #[serde(default = "default_true")]
    pub buildable: bool,
    /// False only for units which can never exist as a Corps/Fleet or
    /// Army/Armada, including the Giant Death Robot.
    #[serde(default = "default_true")]
    pub can_formations: bool,
    /// Whether two copies may be combined in the field. Aircraft Carriers are
    /// the Civ VI exception: formations can be trained or purchased directly,
    /// but existing carriers cannot merge.
    #[serde(default = "default_true")]
    pub can_combine: bool,
    #[serde(default = "default_true")]
    pub earns_xp: bool,
    /// Theocracy and the Grand Master's Chapel enable Faith purchase by unit
    /// class, and the Giant Death Robot is its own class outside that list.
    #[serde(default = "default_true")]
    pub faith_purchasable: bool,
    /// Extra Movement when the unit begins its turn on clear terrain -- flat,
    /// with no Woods, Rainforest or Hills. The Chariot line carries it.
    #[serde(default)]
    pub clear_terrain_start_movement: f64,
    #[serde(default)]
    pub strength: f64,
    #[serde(default)]
    pub ranged_strength: f64, // 0 = no ranged attack
    #[serde(default)]
    pub bombard_strength: f64, // 0 = no anti-district bombard attack
    /// Automatic defense against hostile air missions. This is distinct from
    /// an ordinary ranged attack: anti-air support units cannot attack ground
    /// targets, while several late naval units expose both capabilities.
    #[serde(default)]
    pub anti_air_strength: f64,
    #[serde(default)]
    pub anti_air_range: i32,
    /// Explicit overrides for hybrid and interception-only units. Most units
    /// infer these capabilities from their strength profile; the Giant Death
    /// Robot can use both ordinary attacks.
    #[serde(default)]
    pub can_melee: Option<bool>,
    #[serde(default)]
    pub can_ranged: Option<bool>,
    #[serde(default)]
    pub range: i32,
    #[serde(default)]
    pub charges: i32,
    #[serde(default = "dsight")]
    pub sight: i32,
    #[serde(default)]
    pub tech: Option<Name>,
    #[serde(default)]
    pub requires_resource: Option<Name>,
    /// Strategic material paid once when construction or purchase starts.
    #[serde(default)]
    pub resource_cost: f64,
    /// Strategic fuel consumed at the beginning of every owner turn.
    #[serde(default)]
    pub resource_maintenance: f64,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub unique_to: Option<String>, // civ that alone may build this unit
    #[serde(default)]
    pub replaces: Option<Name>, // base unit this unique replaces
    #[serde(default)]
    pub promotion_class: String,
    #[serde(default)]
    pub zone_of_control: bool,
    #[serde(default)]
    pub cavalry: bool, // light, heavy, and ranged cavalry ignore enemy ZOC
    #[serde(default)]
    pub siege: bool, // full damage vs city walls
    #[serde(default)]
    pub religious_strength: f64,
    /// Base pressure from one Spread Religion charge.
    #[serde(default)]
    pub religious_spread: f64,
    /// Religious units are faith-purchased in a city containing this building.
    #[serde(default)]
    pub requires_building: Option<Name>,
    #[serde(default)]
    pub requires_district: Option<Name>,
    /// Improvements this specialist can construct (builders use the whole
    /// ordinary improvement catalog; engineers/archaeologists are explicit).
    #[serde(default)]
    pub builds: Vec<Name>,
    /// The unit this one becomes when upgraded for Gold, from the shipped
    /// `UnitUpgrades` table. A civilization's unique replacement stands in for
    /// the base unit whenever it owns one.
    #[serde(default, alias = "upgrades_to")]
    pub upgrade_to: Option<Name>,
    /// The shipped `MandatoryObsoleteTech`. Once its owner researches this,
    /// the unit can no longer be trained or purchased; existing copies live on
    /// until they are upgraded.
    #[serde(default)]
    pub obsolete_tech: Option<Name>,
    /// Data-driven auras and special unit rules. Support units currently use
    /// `adjacent_siege_range`, `adjacent_siege_bombard`, `adjacent_heal`, and
    /// `adjacent_movement`; unknown entries remain forward-compatible.
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    /// `Units.WMDCapable`: this unit can carry a nuclear or thermonuclear
    /// device to its target. The shipped rows are the Bomber, the Jet Bomber
    /// and the Nuclear Submarine (`Base/Assets/Gameplay/Data/Units.xml`).
    #[serde(default)]
    pub wmd_capable: bool,
}

impl UnitSpec {
    pub fn ranged_attack_strength(&self) -> f64 {
        self.ranged_strength.max(self.bombard_strength)
    }

    pub fn has_ranged_attack(&self) -> bool {
        self.can_ranged
            .unwrap_or_else(|| self.ranged_attack_strength() > 0.0)
    }

    pub fn is_melee_capable(&self) -> bool {
        self.can_melee.unwrap_or_else(|| {
            self.class == "military"
                && self.domain.as_deref() != Some("air")
                && !self.has_ranged_attack()
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DistrictSpec {
    pub cost: f64,
    /// `PlunderType` and `PlunderAmount`: which yield pillaging this pays and
    /// how much. Gold and heal pay 50, Science, Culture and Faith pay 25.
    #[serde(default)]
    pub plunder_type: Option<String>,
    #[serde(default)]
    pub plunder_amount: f64,
    #[serde(default)]
    pub maintenance: f64,
    #[serde(default)]
    pub tech: Option<Name>,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub yields: Yields,
    /// Yield of one citizen assigned as a specialist in this district.
    #[serde(default)]
    pub citizen_yields: Yields,
    #[serde(default)]
    pub adjacency: BTreeMap<String, Yields>,
    /// Great Person points produced by the completed district itself.
    /// Buildings contribute their own points separately.
    #[serde(default)]
    pub great_person_points: BTreeMap<String, f64>,
    #[serde(default)]
    pub water: bool,
    #[serde(default)]
    pub defense: f64,
    #[serde(default)]
    pub amenity: f64,
    #[serde(default)]
    pub housing: f64,
    /// Specialty districts consume the 1/4/7/... population capacity.
    #[serde(default = "default_true")]
    pub specialty: bool,
    #[serde(default = "default_true")]
    pub buildable: bool,
    /// `null` means that a city may construct multiple copies (for example
    /// Neighborhoods, Canals, eligible Dams, and Spaceports); omitted entries
    /// default to the normal one-per-city rule.
    #[serde(default = "default_one_limit")]
    pub max_per_city: Option<usize>,
    /// `null` means no empire-wide cap. Government Plaza and Diplomatic
    /// Quarter use one; ordinary districts omit it.
    #[serde(default)]
    pub max_per_empire: Option<usize>,
    #[serde(default)]
    pub unique_to: Option<String>,
    #[serde(default)]
    pub replaces: Option<Name>,
    /// IDs of district families that cannot coexist in the same city (for
    /// example Entertainment Complex and Water Park).
    #[serde(default)]
    pub excludes: Vec<Name>,
    /// Placement rule interpreted by `Game::district_sites`.
    #[serde(default)]
    pub placement: String,
    #[serde(default)]
    pub trade_route_capacity: i32,
    #[serde(default)]
    pub air_slots: i32,
    #[serde(default)]
    pub appeal: f64,
    #[serde(default)]
    pub loyalty: f64,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BuildingSpec {
    pub cost: f64,
    #[serde(default = "default_true")]
    pub buildable: bool,
    #[serde(default)]
    pub tech: Option<Name>,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub district: Option<Name>,
    #[serde(default)]
    pub yields: Yields,
    #[serde(default)]
    pub housing: f64,
    #[serde(default)]
    pub amenity: f64,
    #[serde(default)]
    pub wonder: bool,
    #[serde(default)]
    pub coastal: bool,
    #[serde(default)]
    pub growth_pct: f64,
    #[serde(default)]
    pub builder_charges: i32,
    #[serde(default)]
    pub unit_levels: i32,
    #[serde(default)]
    pub unique_to: Option<String>,
    #[serde(default)]
    pub replaces: Option<Name>,
    /// Buildings that must already exist in this city.
    #[serde(default)]
    pub requires: Vec<Name>,
    /// At least one member of this list must exist. Replacement-family
    /// matching applies, so a unique replacement satisfies a base entry.
    #[serde(default)]
    pub requires_any: Vec<Name>,
    /// Mutually exclusive buildings in the same tier or Government Plaza
    /// choice.
    #[serde(default)]
    pub excludes: Vec<Name>,
    #[serde(default)]
    pub power: f64,
    #[serde(default)]
    pub maintenance: f64,
    #[serde(default)]
    pub outer_defense: i32,
    #[serde(default)]
    pub citizen_slots: i32,
    #[serde(default)]
    pub great_work_slots: BTreeMap<String, i32>,
    #[serde(default)]
    pub great_person_points: BTreeMap<String, f64>,
    #[serde(default)]
    pub regional_range: i32,
    #[serde(default)]
    pub regional_group: String,
    #[serde(default)]
    pub trade_route_capacity: i32,
    /// Free-form numeric rule primitives used by named effects that are not
    /// plain yields (production modifiers, combat strength, tourism, etc.).
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    /// Worship buildings are selected by this religion belief and purchased
    /// with Faith rather than constructed with Production.
    #[serde(default)]
    pub worship_belief: Option<Name>,
    #[serde(default)]
    pub purchase_only: bool,
}

/// A world wonder occupies a map tile, unlike an ordinary district building.
/// Placement fields deliberately mirror the predicates used by stock Civ VI
/// so requirements remain data-driven and testable.
#[derive(Clone, Serialize, Deserialize)]
pub struct WonderSpec {
    pub cost: f64,
    #[serde(default)]
    pub tech: Option<Name>,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub yields: Yields,
    #[serde(default)]
    pub housing: f64,
    #[serde(default)]
    pub amenity: f64,
    /// Radius in which the wonder's listed yields and Amenities affect city
    /// centers. Zero means that the values belong only to the constructing
    /// city; Colosseum and other regional wonders set an explicit range.
    #[serde(default)]
    pub regional_range: i32,
    /// Loyalty per turn granted to every city inside `regional_range`.
    #[serde(default)]
    pub regional_loyalty: f64,
    #[serde(default)]
    pub great_work_slots: BTreeMap<String, i32>,
    #[serde(default)]
    pub great_person_points: BTreeMap<String, f64>,
    #[serde(default)]
    pub requires_buildings: Vec<Name>,
    #[serde(default)]
    pub requires_any_buildings: Vec<Name>,
    #[serde(default)]
    pub adjacent_district: Option<Name>,
    #[serde(default)]
    pub adjacent_resource: Option<Name>,
    #[serde(default)]
    pub adjacent_improvement: Option<Name>,
    #[serde(default)]
    pub terrain: Vec<Name>,
    #[serde(default)]
    pub feature: Vec<Name>,
    #[serde(default)]
    pub hills: Option<bool>,
    #[serde(default)]
    pub water: bool,
    #[serde(default)]
    pub coast: bool,
    #[serde(default)]
    pub river: bool,
    #[serde(default)]
    pub adjacent_mountain: bool,
    #[serde(default)]
    pub founded_religion: bool,
    #[serde(default)]
    pub placement: String,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

/// A named entry in the global Great Person market. Effects deliberately use
/// the same primitive keys as the rest of the ruleset so mods can add people
/// without engine-side ID checks.
#[derive(Clone, Serialize, Deserialize)]
pub struct GreatPersonSpec {
    pub name: String,
    pub kind: String,
    pub era: usize,
    pub cost: f64,
    #[serde(default = "done_usize")]
    pub charges: usize,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GovernorPromotionSpec {
    pub tier: i32,
    #[serde(default)]
    pub requires: Vec<Name>,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GovernorSpec {
    pub name: String,
    pub title: String,
    pub establish_turns: u32,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    #[serde(default)]
    pub promotions: BTreeMap<String, GovernorPromotionSpec>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProjectSpec {
    pub cost: f64,
    /// COST_PROGRESSION_GAME_PROGRESS maximum cost as a percentage of the
    /// base cost (1500 means the project grows linearly from 1x to 15x).
    #[serde(default)]
    pub cost_progression_max_pct: f64,
    #[serde(default)]
    pub tech: Option<Name>,
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub district: Option<Name>,
    #[serde(default)]
    pub alternate_districts: Vec<Name>,
    #[serde(default)]
    pub requires: Vec<Name>,
    #[serde(default)]
    pub requires_buildings: Vec<String>,
    /// Building families this project consumes on completion. Firaxis uses
    /// this for Climate Accords: every decommissioning project spends the
    /// matching power plant rather than becoming an infinite score source.
    #[serde(default)]
    pub consumes_buildings: Vec<String>,
    /// A host-tracked competition that must currently be active before this
    /// project is legal. These projects use Firaxis's `UnlocksFromEffect`
    /// rather than a normal tech or civic gate, so they must never appear in a
    /// native CIVVIS production menu.
    #[serde(default)]
    pub host_competition: Option<String>,
    /// Additional host competitions that can grant this same project. Firaxis
    /// uses one Send Aid project for both ordinary and military Aid Requests.
    #[serde(default)]
    pub host_competitions: Vec<String>,
    /// Competition points the host awards when this project completes.  Kept
    /// in the rules row beside the exact project cost, rather than in an AI
    /// name switch, so another host-unlocked competition can use the same
    /// legal-production and valuation path.
    #[serde(default)]
    pub competition_score: f64,
    #[serde(default)]
    pub repeatable: bool,
    /// Per-turn yield conversion percentages while this project is active.
    #[serde(default)]
    pub ongoing_yields: BTreeMap<String, f64>,
    /// Base completion points. Stock district projects scale these from 1x
    /// to 8x with the same whole-percent game-progress model as their cost.
    #[serde(default)]
    pub completion_gpp: BTreeMap<String, f64>,
    #[serde(default)]
    pub full_power_while_active: bool,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

impl ProjectSpec {
    /// Every authoritative host competition that can make this project legal.
    /// `host_competition` remains the compact, backwards-compatible form for
    /// the common one-host case.
    pub fn host_competition_kinds(&self) -> impl Iterator<Item = &str> {
        self.host_competition
            .as_deref()
            .into_iter()
            .chain(self.host_competitions.iter().map(String::as_str))
    }

    pub fn requires_host_competition(&self) -> bool {
        self.host_competition.is_some() || !self.host_competitions.is_empty()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BoostSpec {
    pub trigger: String,
    #[serde(default = "done_i")]
    pub count: i64,
    /// Research granted on triggering, in percent. The database ships 40 for
    /// every boost except Near Future Governance's 90.
    #[serde(default)]
    pub percent: Option<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TreeUnlock {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TechSpec {
    pub cost: f64,
    /// Zero-based historical era: Ancient through Future.
    pub era: usize,
    pub requires: Vec<Name>,
    /// Gathering Storm asks the game core to draw this node's prerequisites
    /// when the game is created instead of shipping fixed prerequisite rows.
    #[serde(default)]
    pub random_prereqs: bool,
    /// The costs the shipped database permits for the randomized columns.
    /// Empty for the fixed-cost gateway and repeatable terminal nodes.
    #[serde(default)]
    pub random_costs: Vec<f64>,
    #[serde(default)]
    pub boost: Option<BoostSpec>,
    /// Indexed from the reverse gates in the rules catalog at load time.
    #[serde(default)]
    pub unlocks: Vec<TreeUnlock>,
    /// Global abilities unlocked by the node. Every key has an engine handler.
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    #[serde(default)]
    pub repeatable: bool,
    /// Governor titles the node awards on completion. Fourteen civics carry
    /// one each; technologies carry none.
    #[serde(default)]
    pub governor_title: usize,
}

/// The part of a randomized research node that the game core decides at
/// setup. Keeping it separate from [`TechSpec`] lets every clone of one game
/// share a complete immutable rules snapshot while saves retain the exact
/// tree they were created with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeLayoutEntry {
    pub cost: f64,
    pub requires: Vec<Name>,
}

/// Gathering Storm randomizes the Future-era technology and civic graphs once
/// per game. Both trees are global to the match, never per-player.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FutureTreeLayout {
    pub techs: BTreeMap<Name, TreeLayoutEntry>,
    pub civics: BTreeMap<Name, TreeLayoutEntry>,
}

impl FutureTreeLayout {
    fn generate(rules: &Rules, seed: u64) -> Result<Self, String> {
        // Dedicated streams keep the map, runtime RNG, and the other tree
        // stable if one catalog gains content in a mod or later rules pass.
        let mut tech_rng = Rng::new(seed ^ 0x5445_4348_5452_4545);
        let mut civic_rng = Rng::new(seed ^ 0x4349_5649_4354_5245);
        Ok(Self {
            techs: random_tree_layout("technology", &rules.techs, &mut tech_rng)?,
            civics: random_tree_layout("civic", &rules.civics, &mut civic_rng)?,
        })
    }

    fn is_empty(&self) -> bool {
        self.techs.is_empty() && self.civics.is_empty()
    }
}

#[derive(Deserialize)]
struct TreeEffectsData {
    techs: BTreeMap<String, BTreeMap<String, f64>>,
    civics: BTreeMap<String, BTreeMap<String, f64>>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GovEffects {
    pub production_pct: f64,
    pub science_pct: f64,
    pub gold_pct: f64,
    pub governor_gold_pct: f64,
    pub governor_faith_per_pop: f64,
    pub governor_production_per_pop: f64,
    pub gold_purchase_discount_pct: f64,
    /// Theocracy's GOVERNMENTBONUS_FAITH_PURCHASES, the Faith-side twin of the
    /// Gold discount above.
    #[serde(default)]
    pub faith_purchase_discount_pct: f64,
    pub district_production_pct: f64,
    pub wonder_production_pct: f64,
    pub unit_production_pct: f64,
    pub war_weariness_reduction_pct: f64,
    pub commercial_encampment_production_pct: f64,
    pub improved_strategic_resource_rate: f64,
    pub power_per_city: f64,
    pub tourism_pct: f64,
    pub combat_strength: f64,
    pub amenity: f64,
    pub housing: f64,
    pub district_city_amenity: f64,
    pub district_city_housing: f64,
    pub wall_level_housing: f64,
    /// Diplomatic Favor per turn for every city holding Renaissance Walls,
    /// which Gathering Storm ships as `BUILDING_STAR_FORT`. Monarchy alone.
    pub walled_city_diplomatic_favor: f64,
    pub influence_pct: f64,
    pub great_people_pct: f64,
    pub production_per_pop: f64,
    pub faith_per_pop: f64,
    pub culture_per_district: f64,
    pub trade_food: f64,
    pub trade_production: f64,
    pub allied_suzerain_trade_food: f64,
    pub allied_suzerain_trade_production: f64,
    pub project_production_pct: f64,
    pub religious_strength: f64,
    pub trade_route_capacity: f64,
    pub capital_yields: Yields,
    pub government_building_yields: Yields,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicySlots {
    pub military: i64,
    pub economic: i64,
    pub diplomatic: i64,
    pub wildcard: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GovSpec {
    #[serde(default)]
    pub civic: Option<Name>,
    #[serde(default)]
    pub influence_per_turn: f64,
    #[serde(default)]
    pub influence_threshold: f64,
    #[serde(default)]
    pub envoys_per_threshold: i64,
    #[serde(default)]
    pub diplomatic_favor_per_turn: f64,
    #[serde(default)]
    pub effects: GovEffects,
    #[serde(default)]
    pub slots: PolicySlots,
}

/// Shipped `StartBias*` rows for a civilization. A lower `Tier` is a stronger
/// pull, so the weight a satisfied bias carries is `6 - tier`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StartBias {
    #[serde(default)]
    pub terrain: Vec<Name>,
    /// Every shipped terrain row for this civilization is a Hills variant, so
    /// the bias is really "hills", not the base terrain underneath it.
    #[serde(default)]
    pub terrain_hills: bool,
    #[serde(default)]
    pub terrain_tier: i32,
    #[serde(default)]
    pub feature: Vec<Name>,
    #[serde(default)]
    pub feature_tier: i32,
    #[serde(default)]
    pub resource: Vec<String>,
    #[serde(default)]
    pub resource_tier: i32,
    #[serde(default)]
    pub river_tier: i32,
}

impl StartBias {
    pub fn weight(tier: i32) -> i32 {
        if tier <= 0 {
            0
        } else {
            (6 - tier).max(1)
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CivSpec {
    pub leader: String,
    /// Key into `Rules::agendas`.
    #[serde(default)]
    pub agenda: Option<String>,
    /// The leader's preference traits, as the shipped data names them —
    /// `expansionist`, `science_major_civ`, `aggressive_military` and so on.
    #[serde(default)]
    pub traits: Vec<String>,
    pub ability: String,
    /// What the signature ability is worth, for the abilities whose whole
    /// effect is a modifier the engine already applies from somewhere else.
    /// The vocabulary is small and deliberately so — `city_food`,
    /// `city_production`, `city_gold`, `city_science`, `city_culture` and
    /// `city_faith` are flat yields every city of this civilization earns;
    /// `unit_production_pct`, `settler_production_pct`,
    /// `building_production_pct`, `district_production_pct`,
    /// `encampment_district_production_pct`, `holy_site_district_production_pct`,
    /// `theater_square_district_production_pct`, `dam_district_production_pct`
    /// and `wonder_production_pct` speed what a city is building;
    /// `happy_science_pct`, `happy_production_pct`,
    /// `happy_campus_scientist_gpp`, and `happy_industrial_engineer_gpp` are
    /// Scotland's Happy-city effects, which double in Ecstatic cities;
    /// `combat_strength` and `unit_xp_pct` belong to its units;
    /// `free_trading_posts` and `own_trading_post_route_gold` are Rome's
    /// All Roads Lead to Rome — every city holds a Trading Post from founding
    /// and a route pays that many Gold per own city it passes through
    /// (`Game::trading_post_route_gold`). An ability that does something the
    /// engine cannot express this way — Rome's free monument, Scythia's
    /// healing — stays keyed by name in `has_ability` instead.
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    #[serde(default)]
    pub unique_unit: Option<String>,
    #[serde(default)]
    pub note: String,
    /// Shipped start bias, absent for the civilizations that have none.
    #[serde(default)]
    pub start_bias: Option<StartBias>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BeliefSpec {
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BeliefsData {
    pub pantheon: BTreeMap<String, BeliefSpec>,
    pub founder: BTreeMap<String, BeliefSpec>,
    pub follower: BTreeMap<String, BeliefSpec>,
    #[serde(default)]
    pub enhancer: BTreeMap<String, BeliefSpec>,
    #[serde(default)]
    pub worship: BTreeMap<String, BeliefSpec>,
}

/// Read `"replaces": "x"` and `"replaces": ["x", "y"]` alike.
///
/// The one-card form is by far the common one and there are 19 of them already in
/// `data/policies.json`; rewriting every one of them into a single-element list to
/// express the three that need two would be a large diff for no gain.
fn one_or_many_names<'de, D>(deserializer: D) -> Result<Vec<Name>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(Name),
        Many(Vec<Name>),
    }
    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(name) => vec![name],
        OneOrMany::Many(names) => names,
    })
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PolicySpec {
    pub slot: String, // military | economic | diplomatic | wildcard
    #[serde(default)]
    pub civic: Option<Name>,
    /// Cards this one retires when it unlocks.
    ///
    /// ⚠ ONE CARD CAN RETIRE SEVERAL. Civilization VI's `ObsoletePolicies` is keyed
    /// by the *predecessor*, so a successor appears once per card it kills, and three
    /// of them kill two: Public Works (Bastions + Serfdom), Lightning Warfare (Limes
    /// + Maneuver) and Native Conquest (Discipline + Survey). A single `Option` could
    ///   only ever carry one of each pair, which is why this reads as a list.
    ///
    /// A bare string is still accepted, so entries naming one card stay one line.
    #[serde(default, deserialize_with = "one_or_many_names")]
    pub replaces: Vec<Name>,
    #[serde(default)]
    pub note: String,
    /// Numeric, data-driven policy primitives consumed by the game engine.
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    /// Unit-Production cards apply only to units of these eras. Agoge boosts
    /// Ancient and Classical infantry and nothing later; an empty list means
    /// the card is not era-gated. `ADJUST_UNIT_TAG_ERA_PRODUCTION` ships one
    /// row per (era, promotion class) pair, and each card in a ladder repeats
    /// its predecessor's eras rather than starting where that one stopped.
    #[serde(default)]
    pub unit_eras: Vec<usize>,
    /// Eras a single promotion class is missing from an otherwise covered
    /// window. Firaxis wrote the infantry ladder's rows one per era and left
    /// Classical out of the ranged set for every card after Agoge, which is
    /// invisible except to a Classical ranged unique.
    #[serde(default)]
    pub unit_era_gaps: BTreeMap<String, Vec<usize>>,
    /// Dark Age cards are not unlocked by a civic. They open a Wildcard slot
    /// to a civilization living through a Dark Age, and close again the moment
    /// it climbs out.
    #[serde(default)]
    pub dark_age: bool,
    /// Inclusive world-era span a Dark Age card is offered in, as indices into
    /// [`ERA_NAMES`]. Ignored for ordinary cards.
    #[serde(default)]
    pub eras: Option<(usize, usize)>,
}

impl PolicySpec {
    /// Whether this card is on offer to a civilization in `age` during
    /// `era`. Ordinary cards ignore both; Dark Age cards need the age and the
    /// era together.
    pub fn offered(&self, age: &str, era: usize) -> bool {
        if !self.dark_age {
            return true;
        }
        age == "dark"
            && self
                .eras
                .is_none_or(|(first, last)| era >= first && era <= last)
    }
}

/// A stock unit-promotion node. Effects are numeric flags so rules data can
/// add promotions without changing the action/state protocol.
#[derive(Clone, Serialize, Deserialize)]
pub struct PromotionSpec {
    pub class: String,
    pub tier: i32,
    #[serde(default)]
    pub requires: Vec<Name>,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    #[serde(default)]
    pub note: String,
}

/// The collection a modifier targets when it is attached to a player.
///
/// Civ VI names many more collection types than CIVVIS can execute today.
/// These three cover the player-wide, city-wide, and unit-wide paths that the
/// engine already has a stable consumer for. Keeping the scope in the data
/// means a newly imported modifier row cannot silently become an empire-wide
/// effect just because it happened to use the same numeric key.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModifierCollection {
    #[default]
    Player,
    PlayerCities,
    PlayerUnits,
}

/// One atomic predicate in a modifier requirement set.
///
/// Civ VI's requirement tables are much larger than this deliberately small
/// first slice. These predicates are the high-frequency player facts needed by
/// the existing consumers: government, identity, religion, age, and completed
/// research/cards. A row may combine fields with AND semantics; requirement
/// sets provide `all`, `any`, and `none` groups around them.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ModifierRequirement {
    #[serde(alias = "player_type")]
    pub player_type: Option<String>,
    #[serde(alias = "civ")]
    pub civilization: Option<String>,
    pub government: Option<String>,
    pub religion: Option<String>,
    pub pantheon: Option<String>,
    pub secret_society: Option<String>,
    pub age: Option<String>,
    pub policy: Option<String>,
    #[serde(alias = "tech")]
    pub technology: Option<String>,
    pub civic: Option<String>,
}

impl ModifierRequirement {
    fn is_empty(&self) -> bool {
        self.player_type.is_none()
            && self.civilization.is_none()
            && self.government.is_none()
            && self.religion.is_none()
            && self.pantheon.is_none()
            && self.secret_society.is_none()
            && self.age.is_none()
            && self.policy.is_none()
            && self.technology.is_none()
            && self.civic.is_none()
    }

    fn validate(&self, modifier: &str, group: &str, index: usize) -> Result<(), String> {
        if self.is_empty() {
            return Err(format!(
                "modifier {modifier} has an empty {group} requirement at index {index}"
            ));
        }
        for (field, value) in [
            ("player_type", self.player_type.as_deref()),
            ("civilization", self.civilization.as_deref()),
            ("government", self.government.as_deref()),
            ("religion", self.religion.as_deref()),
            ("pantheon", self.pantheon.as_deref()),
            ("secret_society", self.secret_society.as_deref()),
            ("age", self.age.as_deref()),
            ("policy", self.policy.as_deref()),
            ("technology", self.technology.as_deref()),
            ("civic", self.civic.as_deref()),
        ] {
            if value.is_some_and(str::is_empty) {
                return Err(format!(
                    "modifier {modifier} has an empty {field} value in {group} requirement {index}"
                ));
            }
        }
        Ok(())
    }

    fn matches(&self, context: &ModifierContext<'_>) -> bool {
        fn same(expected: Option<&String>, actual: Option<&str>) -> bool {
            expected.is_none_or(|expected| {
                actual.is_some_and(|actual| expected.eq_ignore_ascii_case(actual))
            })
        }
        fn contains(expected: Option<&String>, actual: Option<&BTreeSet<Name>>) -> bool {
            expected.is_none_or(|expected| {
                actual.is_some_and(|actual| actual.contains(&Name::new(expected)))
            })
        }

        same(self.player_type.as_ref(), context.player_type)
            && same(self.civilization.as_ref(), context.civilization)
            && same(self.government.as_ref(), context.government)
            && same(self.religion.as_ref(), context.religion)
            && same(self.pantheon.as_ref(), context.pantheon)
            && same(self.secret_society.as_ref(), context.secret_society)
            && same(self.age.as_ref(), context.age)
            && contains(self.policy.as_ref(), context.policies)
            && contains(self.technology.as_ref(), context.technologies)
            && contains(self.civic.as_ref(), context.civics)
    }
}

/// Facts supplied by the game state when a modifier is collected.
///
/// This is intentionally a borrowed view: checking a modifier must not clone
/// a player's policy or research sets on every yield query.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModifierContext<'a> {
    pub player_type: Option<&'a str>,
    pub civilization: Option<&'a str>,
    pub government: Option<&'a str>,
    pub religion: Option<&'a str>,
    pub pantheon: Option<&'a str>,
    pub secret_society: Option<&'a str>,
    pub age: Option<&'a str>,
    pub policies: Option<&'a BTreeSet<Name>>,
    pub technologies: Option<&'a BTreeSet<Name>>,
    pub civics: Option<&'a BTreeSet<Name>>,
}

/// A Civ VI-style requirement set with explicit Boolean grouping.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ModifierRequirements {
    #[serde(default)]
    pub all: Vec<ModifierRequirement>,
    #[serde(default)]
    pub any: Vec<ModifierRequirement>,
    #[serde(default)]
    pub none: Vec<ModifierRequirement>,
}

impl ModifierRequirements {
    pub fn is_empty(&self) -> bool {
        self.all.is_empty() && self.any.is_empty() && self.none.is_empty()
    }

    fn validate(&self, modifier: &str) -> Result<(), String> {
        for (group, requirements) in [("all", &self.all), ("any", &self.any), ("none", &self.none)]
        {
            for (index, requirement) in requirements.iter().enumerate() {
                requirement.validate(modifier, group, index)?;
            }
        }
        Ok(())
    }

    pub fn matches(&self, context: &ModifierContext<'_>) -> bool {
        self.all
            .iter()
            .all(|requirement| requirement.matches(context))
            && (self.any.is_empty()
                || self
                    .any
                    .iter()
                    .any(|requirement| requirement.matches(context)))
            && self
                .none
                .iter()
                .all(|requirement| !requirement.matches(context))
    }
}

/// A reusable bundle of numeric engine effects.
///
/// Civ VI's `ATTACH_MODIFIER` effect composes named modifiers rather than
/// copying every argument onto every owning object. A rules object opts into
/// the same model with a `modifiers: ["name"]` field. The loader resolves the
/// graph once, adds the resulting values to that object's ordinary `effects`
/// map, and rejects dangling references or cycles before a game can start.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModifierSpec {
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    /// Flat yield changes keyed by the building or replacement family they
    /// target. `"*"` targets every building. The loader compiles these into
    /// ordinary numeric effects so nested attachments retain additive
    /// composition and any effect-bearing rules object can consume them.
    #[serde(default)]
    pub building_yields: BTreeMap<String, Yields>,
    /// Percentage purchase discounts keyed by unit or replacement family.
    /// `"*"` targets every unit and the same modifier applies to Gold and
    /// Faith, matching `ADJUST_UNIT_PURCHASE_COST`.
    #[serde(default)]
    pub unit_purchase_discount_pct: BTreeMap<String, f64>,
    /// Ability identities granted while this modifier is active. Ability
    /// semantics remain in their normal engine consumers; this removes the
    /// fixed owner list from `GRANT_ABILITY` itself.
    #[serde(default)]
    pub abilities: BTreeSet<String>,
    /// Other named bundles this modifier attaches, in application order.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Collection targeted by this bundle when it is attached at runtime.
    #[serde(default)]
    pub collection: ModifierCollection,
    /// Optional player-state predicates. Requirements are evaluated when a
    /// live attachment is collected, not when the ruleset is loaded.
    #[serde(default)]
    pub requirements: ModifierRequirements,
}

const BUILDING_YIELD_EFFECT_PREFIX: &str = "building_yield:";
const UNIT_PURCHASE_EFFECT_PREFIX: &str = "unit_purchase_discount_pct:";
const GRANT_ABILITY_EFFECT_PREFIX: &str = "grant_ability:";

pub fn building_yield_effect_key(building: &str, yield_type: &str) -> String {
    format!("{BUILDING_YIELD_EFFECT_PREFIX}{building}:{yield_type}")
}

pub fn unit_purchase_discount_effect_key(unit: &str) -> String {
    format!("{UNIT_PURCHASE_EFFECT_PREFIX}{unit}")
}

pub fn grant_ability_effect_key(ability: &str) -> String {
    format!("{GRANT_ABILITY_EFFECT_PREFIX}{ability}")
}

fn compile_modifier_selectors(
    name: &str,
    spec: &ModifierSpec,
    effects: &mut BTreeMap<String, f64>,
) -> Result<(), String> {
    for (building, yields) in &spec.building_yields {
        if building.is_empty() || building.contains(':') {
            return Err(format!(
                "modifier {name} has invalid building selector {building:?}"
            ));
        }
        for (yield_type, value) in [
            ("food", yields.food),
            ("production", yields.production),
            ("gold", yields.gold),
            ("science", yields.science),
            ("culture", yields.culture),
            ("faith", yields.faith),
        ] {
            if value != 0.0 {
                *effects
                    .entry(building_yield_effect_key(building, yield_type))
                    .or_insert(0.0) += value;
            }
        }
    }
    for (unit, value) in &spec.unit_purchase_discount_pct {
        if unit.is_empty() || unit.contains(':') {
            return Err(format!(
                "modifier {name} has invalid unit selector {unit:?}"
            ));
        }
        *effects
            .entry(unit_purchase_discount_effect_key(unit))
            .or_insert(0.0) += value;
    }
    for ability in &spec.abilities {
        if ability.is_empty() || ability.contains(':') {
            return Err(format!("modifier {name} has invalid ability {ability:?}"));
        }
        *effects
            .entry(grant_ability_effect_key(ability))
            .or_insert(0.0) += 1.0;
    }
    Ok(())
}

/// A leader's historical agenda: the standing opinion they hold about how
/// other civilizations ought to behave.
///
/// Unciv gives each leader a personality vector and weights its AI by it,
/// which is what stops every AI civ from playing the same game. Civ VI ships
/// the content for the same idea — `Leaders.xml` assigns each leader an
/// agenda and a set of preference traits — so we take Unciv's shape and the
/// game's own assignments. See `docs/UNCIV_LESSONS.md`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AgendaSpec {
    pub name: String,
    pub description: String,
    /// What the leader measures other civilizations by. Each value has an
    /// engine handler in `Game::agenda_measure`.
    pub measure: String,
    /// `more` to approve of a high measure, `less` to approve of a low one.
    pub approves_of: String,
}

/// One row of Civilization VI's `StartingBuildings` table, as it applies to a
/// rung of the difficulty ladder: a *completed* building a city already holds
/// the moment the game opens, before anything is produced and regardless of
/// whether its own technology has been researched.
///
/// The shipped table has 24 rows and exactly one of them carries a
/// `MinDifficulty`, so exactly one is a property of the ladder rather than of
/// the start era. The other 23 are `MinorOnly = 0` with no `MinDifficulty` at
/// all: they are what a game *opened past the Ancient era* hands a major
/// civilization, which is a start-era rule and belongs nowhere near a
/// handicap. See `docs/FIDELITY.md`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct StartingBuildingSpec {
    /// The building's id in `buildings.json` — the shipped `Building` column
    /// under this engine's spelling.
    pub building: String,
    /// `StartingBuildings.MinorOnly`. True grants the building to city-states
    /// and to nobody else; false grants it to major civilizations only. The
    /// column is a partition, not a permission, which is why one flag decides
    /// both sides.
    pub minor_only: bool,
}

/// A difficulty level, in the Civ VI sense: a bag of handicaps applied to the
/// AI seats above Prince and to the human seats below it. Prince is the
/// reference level and carries no modifiers at all.
///
/// The numbers come from the scaling modifiers the game itself ships in
/// `Leaders.xml` (`HIGH_DIFFICULTY_SCIENCE_SCALING` and its siblings, each
/// declared `LinearScaleFromDefaultHandicap` off Prince), so a level here is
/// the shipped per-step delta multiplied by that level's distance from Prince.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct DifficultySpec {
    pub name: String,
    /// Position on the ladder, Settler 0 through Deity 7. Also the sort order.
    pub order: usize,
    /// Percentage added to each AI city yield of the named kind.
    pub ai_yield_pct: Yields,
    /// Flat Combat Strength added to every AI unit.
    pub ai_combat_strength: f64,
    /// Percentage added to AI experience awards.
    pub ai_xp_pct: f64,
    /// Random Eurekas and Inspirations granted to each AI on a new world era.
    pub ai_era_boosts: usize,
    /// Extra units each AI receives on its start tile.
    pub ai_bonus_units: BTreeMap<String, usize>,
    /// Completed buildings a city already holds when a game at this rung
    /// opens, from the shipped `StartingBuildings` table's `MinDifficulty`
    /// gate.
    ///
    /// That table gates one row on difficulty — `BUILDING_WALLS`,
    /// `ERA_ANCIENT`, `DISTRICT_CITY_CENTER`, `MinorOnly = 1`,
    /// `MinDifficulty = DIFFICULTY_IMMORTAL` — so Immortal and Deity
    /// city-states open behind Ancient Walls and every rung below them opens
    /// behind none. **This runs the opposite way to every other field here:**
    /// the rest of the ladder hands its bonuses to the AI *major* seats, and
    /// this one hardens the minor seats the challenger has to take a city off.
    /// Leaving it out did not make CIVVIS's Immortal harder than the game's,
    /// it made it *easier*, which is the direction an audit does not look for.
    ///
    /// No rung grants a major civilization anything: every major-side row of
    /// the shipped table is keyed on the start era instead, and CIVVIS's
    /// Advanced Start deliberately does not grant them (`open_in_start_era`).
    pub starting_buildings: Vec<StartingBuildingSpec>,
    /// Flat Combat Strength added to every human unit.
    pub human_combat_strength: f64,
    /// Percentage added to human experience awards.
    pub human_xp_pct: f64,
    /// Extra Gold a human receives for clearing a Barbarian camp.
    pub human_camp_gold: f64,
    /// Scales the size of barbarian raiding parties. Read from the barbarian
    /// seat's own rung (`Game::barbarian_spec`, Emperor by default), not the
    /// majors' — see `Game::default_barbarian_difficulty`.
    #[serde(default = "done")]
    pub barb_force_scale: f64,
    /// Scales how long a camp waits between spawns.
    /// `BarbarianAttackForces.SpawnRate` is 2 for every band up to Emperor and
    /// 1 from Immortal, so the top band assembles its forces twice as often.
    /// Read from the barbarian seat's own rung, like `barb_force_scale`.
    #[serde(default = "done")]
    pub barb_spawn_scale: f64,
}

/// A game speed: everything a civilization buys with a stockpiled yield scales
/// by `cost_pct`, and the game runs for `turns` turns. Both are the values the
/// shipped `GameSpeeds.xml` uses (`CostMultiplier`, and the sum of that speed's
/// turn-length table).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SpeedSpec {
    pub name: String,
    pub order: usize,
    #[serde(default = "dhundred")]
    pub cost_pct: f64,
    #[serde(default = "dstandard_turns")]
    pub turns: u32,
}

/// One positive Historic Moment from the shipped ruleset. Era numbers use
/// [`ERA_NAMES`] indices. Minimum and maximum bounds are inclusive; an
/// obsolete era is exclusive, matching the game's `ObsoleteEra` column.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct HistoricMomentSpec {
    pub era_score: i64,
    pub minimum_game_era: Option<usize>,
    pub maximum_game_era: Option<usize>,
    pub obsolete_era: Option<usize>,
}

fn dhundred() -> f64 {
    100.0
}

fn dstandard_turns() -> u32 {
    500
}

#[derive(Clone)]
pub struct Rules {
    /// Deterministic fingerprint of the effective source JSON before derived
    /// indexes are built. Tournament evidence binds to this so changing a
    /// stock rule or a mod's contents cannot silently reuse an old baseline.
    source_fingerprint: String,
    pub terrains: SpecMap<TerrainSpec>,
    pub features: SpecMap<FeatureSpec>,
    pub resources: SpecMap<ResourceSpec>,
    pub improvements: SpecMap<ImprovementSpec>,
    pub units: SpecMap<UnitSpec>,
    pub districts: SpecMap<DistrictSpec>,
    pub buildings: SpecMap<BuildingSpec>,
    pub wonders: SpecMap<WonderSpec>,
    pub great_people: SpecMap<GreatPersonSpec>,
    pub governors: SpecMap<GovernorSpec>,
    pub projects: SpecMap<ProjectSpec>,
    pub techs: SpecMap<TechSpec>,
    pub civics: SpecMap<TechSpec>,
    pub governments: SpecMap<GovSpec>,
    pub policies: SpecMap<PolicySpec>,
    pub promotions: SpecMap<PromotionSpec>,
    /// Named, recursively composable modifier bundles. Entries are flattened
    /// and cycle-free by the time a [`Rules`] value is constructed.
    pub modifiers: SpecMap<ModifierSpec>,
    pub beliefs: BeliefsData,
    pub civs: SpecMap<CivSpec>,
    pub agendas: SpecMap<AgendaSpec>,
    pub difficulties: SpecMap<DifficultySpec>,
    pub speeds: SpecMap<SpeedSpec>,
    /// Tribal village reward tables, the shipped seven categories.
    pub goody_huts: SpecMap<BTreeMap<String, GoodyRewardSpec>>,
    /// Per-era constants from the shipped Eras table, keyed by ERA_NAMES.
    pub eras: SpecMap<EraSpec>,
    /// The shipped WMDs table. Blast radius, fallout duration and ICBM range
    /// all drive `Action::WmdStrike` — launched from an unpillaged Missile
    /// Silo or carried by a `wmd_capable` unit — and the per-turn Gold
    /// maintenance is charged every turn a device is stockpiled.
    pub wmds: SpecMap<WmdSpec>,
    /// Gathering Storm's random-disaster classes and their tuning.
    pub disasters: SpecMap<DisasterSpec>,
    /// Rise & Fall's Dedications, both halves.
    pub dedications: SpecMap<DedicationSpec>,
    /// Every positive Historic Moment supported by the engine, including the
    /// six Monopoly and Corporations mode moments absent from the base GS
    /// `Moments` table.
    pub historic_moments: SpecMap<HistoricMomentSpec>,
    /// The city-state seats this ruleset can hand out, in seating order.
    pub city_states: CityStateRoster,
    /// Per-effect technology and civic sources, compiled when loading a ruleset.
    /// One lookup retrieves both lists. Each keeps node order so technologies
    /// are accumulated before civics without changing floating-point results.
    pub tree_effects: SpecMap<TreeEffectSources>,
    /// Every node a given node depends on, however far back.
    ///
    /// Asking whether one technology leads to another used to walk the
    /// prerequisite graph from the target every time, re-exploring nodes that
    /// several paths reach. The closure is taken once instead.
    pub tech_ancestors: SpecMap<BTreeSet<String>>,
    pub civic_ancestors: SpecMap<BTreeSet<String>>,
    /// Which effect keys each family of specs declares at all.
    pub effect_index: EffectIndex,
    /// For each district, the family each of its adjacency keys names — in
    /// `DistrictSpec::adjacency`'s own key order, `None` for a key that is not
    /// a district at all.
    ///
    /// The district adjacency count table resolves a key like
    /// `government_plaza` by interning it and walking `replaces`. It runs
    /// once per (plot, district, key), and a settlement scan asks it for
    /// nineteen plots at each of ~154 candidate sites: about twenty million
    /// `Name::new` calls in a six-player game, each one an `RwLock` read and a
    /// hash. `Name::new`'s own documentation says it belongs at the edges and
    /// that hot code should carry a `Name` it was given. This is that edge —
    /// the keys are fixed when the ruleset loads and never change afterwards.
    pub district_adjacency_families: SpecMap<Vec<Option<Name>>>,
}

/// The effect keys each family of specs actually grants something towards.
///
/// Every path that collects a numeric modifier ends in `spec.effects.get(key)`
/// over some family — a player's policies, their cities' buildings, the
/// wonders they have built, a Governor's promotions. A key that no spec in the
/// family declares can only contribute `None`, so the whole sweep is a
/// guaranteed zero. Asking the family first turns the common answer into one
/// lookup instead of a walk over every city, building or roster entry.
///
/// Built once with the ruleset, alongside the inverted tree tables and for the
/// same reason. A mod overlay merges into the raw JSON before a [`Rules`] is
/// constructed, so the index covers modded content too.
#[derive(Clone, Default)]
pub struct EffectIndex {
    pub policies: SpecMap<()>,
    pub civs: SpecMap<()>,
    pub buildings: SpecMap<()>,
    pub districts: SpecMap<()>,
    pub wonders: SpecMap<()>,
    pub beliefs: SpecMap<()>,
    pub governors: SpecMap<()>,
    /// The union of every family above, including the trees.
    ///
    /// This is what `Game::city_modifier_effect` and `modifier_grants_ability`
    /// are ruled out on, so it has to cover every *ruleset* source those two
    /// collect from — policies, the trees, civilization traits, buildings,
    /// districts, wonders, beliefs, pantheons and Governors. It deliberately
    /// does not cover families nothing collects this way (unit promotions,
    /// improvements, projects, Great People): a narrower union skips more.
    /// Add a family here the moment a collection path starts reading it, or
    /// that path will read a zero that is wrong.
    ///
    /// Runtime attachments are deliberately **absent**. `Rules::modifiers` is
    /// the one table swapped in after a ruleset is built — that is how a
    /// World Congress resolution installs an arbitrary bundle — so an index
    /// of it would go stale the moment it mattered. The collection paths
    /// instead fall through whenever the seat has any attachment at all,
    /// which needs no index and cannot be stale.
    pub any: SpecMap<()>,
    /// The selectors named by the three namespaced effect families.
    ///
    /// `building_yield:<building>:<yield>` and its two siblings are assembled
    /// with [`format!`] at the call site, so asking whether the key exists
    /// costs an allocation before the lookup can even miss. These sets are
    /// keyed on the selector alone, which the caller already holds borrowed,
    /// so the common "nothing modifies this" answer costs nothing.
    pub building_yield_selectors: SpecMap<()>,
    pub unit_purchase_selectors: SpecMap<()>,
    pub granted_abilities: SpecMap<()>,
}

impl EffectIndex {
    #[inline]
    pub fn policies(&self, effect: &str) -> bool {
        self.policies.contains_key(effect)
    }
    #[inline]
    pub fn civs(&self, effect: &str) -> bool {
        self.civs.contains_key(effect)
    }
    #[inline]
    pub fn buildings(&self, effect: &str) -> bool {
        self.buildings.contains_key(effect)
    }
    #[inline]
    pub fn districts(&self, effect: &str) -> bool {
        self.districts.contains_key(effect)
    }
    #[inline]
    pub fn wonders(&self, effect: &str) -> bool {
        self.wonders.contains_key(effect)
    }
    #[inline]
    pub fn beliefs(&self, effect: &str) -> bool {
        self.beliefs.contains_key(effect)
    }
    #[inline]
    pub fn governors(&self, effect: &str) -> bool {
        self.governors.contains_key(effect)
    }
    /// Whether anything at all in the ruleset grants this effect.
    #[inline]
    pub fn any(&self, effect: &str) -> bool {
        self.any.contains_key(effect)
    }
    /// Whether any modifier changes the yields a named building produces.
    #[inline]
    pub fn modifies_building_yields(&self, selector: &str) -> bool {
        self.building_yield_selectors.contains_key(selector)
    }
    /// Whether any modifier discounts the purchase of a named unit.
    #[inline]
    pub fn discounts_unit_purchase(&self, selector: &str) -> bool {
        self.unit_purchase_selectors.contains_key(selector)
    }
    /// Whether any modifier grants a named ability.
    #[inline]
    pub fn grants_ability(&self, ability: &str) -> bool {
        self.granted_abilities.contains_key(ability)
    }
}

/// Collect the effect keys a family of specs declares.
fn effect_key_set<'a>(keys: impl Iterator<Item = &'a String>) -> SpecMap<()> {
    let mut set = SpecMap::new();
    for key in keys {
        set.insert(key.clone(), ());
    }
    set
}

fn shuffle_strings(values: &mut [Name], rng: &mut Rng) {
    for index in (1..values.len()).rev() {
        let other = rng.below(index + 1);
        values.swap(index, other);
    }
}

/// Join two adjacent randomized columns with the smallest bipartite graph
/// that gives every node on both sides an edge. This is the shape the shipped
/// Future trees expose: no branch is orphaned and no breakthrough is free.
fn connect_random_layers(
    parents: &[Name],
    children: &[Name],
    rng: &mut Rng,
    requirements: &mut BTreeMap<Name, Vec<Name>>,
) -> Result<(), String> {
    if parents.is_empty() || children.is_empty() {
        return Err("a randomized research column is empty".to_string());
    }
    let mut parents = parents.to_vec();
    let mut children = children.to_vec();
    shuffle_strings(&mut parents, rng);
    shuffle_strings(&mut children, rng);
    for index in 0..parents.len().max(children.len()) {
        let parent = &parents[index % parents.len()];
        let child = &children[index % children.len()];
        let prerequisites = requirements
            .get_mut(child)
            .ok_or_else(|| format!("randomized child {child:?} is not in the layout"))?;
        if !prerequisites.contains(parent) {
            prerequisites.push(*parent);
        }
    }
    Ok(())
}

fn random_tree_layout(
    kind: &str,
    tree: &SpecMap<TechSpec>,
    rng: &mut Rng,
) -> Result<BTreeMap<Name, TreeLayoutEntry>, String> {
    let randomized: Vec<Name> = tree
        .iter()
        .filter(|(_, spec)| spec.random_prereqs)
        .map(|(name, _)| *name)
        .collect();
    if randomized.is_empty() {
        return Ok(BTreeMap::new());
    }

    let eras: BTreeSet<usize> = randomized.iter().map(|name| tree[name].era).collect();
    if eras.len() != 1 {
        return Err(format!(
            "randomized {kind} nodes span more than one era: {eras:?}"
        ));
    }
    let randomized_era = *eras.iter().next().unwrap();
    let previous_era = tree
        .values()
        .filter(|spec| spec.era < randomized_era)
        .map(|spec| spec.era)
        .max()
        .ok_or_else(|| format!("randomized {kind} tree has no preceding era"))?;

    let mut regular = Vec::new();
    let mut gateways = Vec::new();
    let mut terminals = Vec::new();
    for name in &randomized {
        let spec = &tree[name];
        if !spec.requires.is_empty() {
            return Err(format!(
                "randomized {kind} {name:?} also carries fixed prerequisites"
            ));
        }
        if spec.repeatable {
            if !spec.random_costs.is_empty() {
                return Err(format!(
                    "repeatable randomized {kind} {name:?} carries column costs"
                ));
            }
            terminals.push(*name);
        } else if spec.random_costs.is_empty() {
            gateways.push(*name);
        } else if spec.random_costs.len() == 2 {
            regular.push(*name);
        } else {
            return Err(format!(
                "randomized {kind} {name:?} needs exactly two column costs"
            ));
        }
    }
    if regular.len() < 2 {
        return Err(format!(
            "randomized {kind} tree needs at least two column nodes"
        ));
    }
    if gateways.len() > 1 || terminals.len() != 1 {
        return Err(format!(
            "randomized {kind} tree needs one repeatable terminal and at most one gateway"
        ));
    }

    // A previous-era leaf is a node with no fixed child. Every one feeds the
    // first randomized column, just as every visible Information-era branch
    // reaches the Future-era samples shipped by the game.
    let fixed_parents: BTreeSet<Name> = tree
        .iter()
        .filter(|(_, spec)| !spec.random_prereqs)
        .flat_map(|(_, spec)| spec.requires.iter().copied())
        .collect();
    let previous_leaves: Vec<Name> = tree
        .iter()
        .filter(|(name, spec)| spec.era == previous_era && !fixed_parents.contains(name))
        .map(|(name, _)| *name)
        .collect();
    if previous_leaves.is_empty() {
        return Err(format!(
            "randomized {kind} tree has no leaves in its preceding era"
        ));
    }

    shuffle_strings(&mut regular, rng);
    // The shipped samples use two non-empty random-cost columns. With more
    // than two nodes the first contains at least two; its upper bound leaves
    // at least one node for the second.
    let first_len = if regular.len() == 2 {
        1
    } else {
        2 + rng.below(regular.len() - 2)
    };
    let (first, second) = regular.split_at(first_len);
    let mut requirements: BTreeMap<Name, Vec<Name>> =
        randomized.iter().map(|name| (*name, Vec::new())).collect();
    connect_random_layers(&previous_leaves, first, rng, &mut requirements)?;
    connect_random_layers(first, second, rng, &mut requirements)?;

    let terminal = terminals.pop().unwrap();
    if let Some(gateway) = gateways.pop() {
        requirements.insert(gateway, second.to_vec());
        requirements.insert(terminal, vec![gateway]);
    } else {
        requirements.insert(terminal, second.to_vec());
    }

    let first: BTreeSet<&str> = first.iter().map(|name| name.as_str()).collect();
    let mut layout = BTreeMap::new();
    for name in randomized {
        let spec = &tree[&name];
        let cost = if spec.random_costs.is_empty() {
            spec.cost
        } else if first.contains(name.as_str()) {
            spec.random_costs[0]
        } else {
            spec.random_costs[1]
        };
        let mut requires = requirements.remove(&name).unwrap_or_default();
        requires.sort();
        requires.dedup();
        layout.insert(name, TreeLayoutEntry { cost, requires });
    }
    Ok(layout)
}

fn apply_tree_layout(
    kind: &str,
    tree: &mut SpecMap<TechSpec>,
    layout: &BTreeMap<Name, TreeLayoutEntry>,
) -> Result<(), String> {
    let expected: BTreeSet<Name> = tree
        .iter()
        .filter(|(_, spec)| spec.random_prereqs)
        .map(|(name, _)| *name)
        .collect();
    let actual: BTreeSet<Name> = layout.keys().copied().collect();
    if expected != actual {
        return Err(format!(
            "saved randomized {kind} nodes do not match the active ruleset"
        ));
    }

    let randomized_era = expected
        .iter()
        .map(|name| tree[name].era)
        .max()
        .ok_or_else(|| format!("saved randomized {kind} tree is empty"))?;
    let previous_era = tree
        .values()
        .filter(|spec| spec.era < randomized_era)
        .map(|spec| spec.era)
        .max()
        .ok_or_else(|| format!("saved randomized {kind} tree has no preceding era"))?;
    let fixed_parents: BTreeSet<Name> = tree
        .iter()
        .filter(|(_, spec)| !spec.random_prereqs)
        .flat_map(|(_, spec)| spec.requires.iter().copied())
        .collect();
    let previous_leaves: BTreeSet<Name> = tree
        .iter()
        .filter(|(name, spec)| spec.era == previous_era && !fixed_parents.contains(*name))
        .map(|(name, _)| *name)
        .collect();

    let mut first = BTreeSet::new();
    let mut second = BTreeSet::new();
    let mut gateways = Vec::new();
    let mut terminals = Vec::new();
    for (name, entry) in layout {
        let spec = tree
            .get(name)
            .ok_or_else(|| format!("saved randomized {kind} {name:?} is unknown"))?;
        if entry.cost <= 0.0 {
            return Err(format!(
                "saved randomized {kind} {name:?} has invalid cost {}",
                entry.cost
            ));
        }
        if spec.repeatable {
            if entry.cost != spec.cost {
                return Err(format!(
                    "saved randomized {kind} terminal {name:?} has invalid cost {}",
                    entry.cost
                ));
            }
            terminals.push(*name);
        } else if spec.random_costs.is_empty() {
            if entry.cost != spec.cost {
                return Err(format!(
                    "saved randomized {kind} gateway {name:?} has invalid cost {}",
                    entry.cost
                ));
            }
            gateways.push(*name);
        } else if spec.random_costs.len() != 2 {
            return Err(format!(
                "randomized {kind} {name:?} does not define exactly two column costs"
            ));
        } else if entry.cost == spec.random_costs[0] {
            first.insert(*name);
        } else if entry.cost == spec.random_costs[1] {
            second.insert(*name);
        } else {
            return Err(format!(
                "saved randomized {kind} {name:?} has invalid cost {}",
                entry.cost
            ));
        }
        let unique: BTreeSet<&str> = entry.requires.iter().map(|name| name.as_str()).collect();
        if unique.len() != entry.requires.len() {
            return Err(format!(
                "saved randomized {kind} {name:?} repeats a prerequisite"
            ));
        }
        for prerequisite in &entry.requires {
            if prerequisite == name || !tree.contains_key(prerequisite) {
                return Err(format!(
                    "saved randomized {kind} {name:?} has invalid prerequisite {prerequisite:?}"
                ));
            }
        }
    }
    if first.is_empty() || second.is_empty() || terminals.len() != 1 || gateways.len() > 1 {
        return Err(format!(
            "saved randomized {kind} tree does not contain two columns, one terminal, and at most one gateway"
        ));
    }

    let validate_layer = |children: &BTreeSet<Name>,
                          parents: &BTreeSet<Name>,
                          label: &str|
     -> Result<(), String> {
        let mut used = BTreeSet::new();
        for child in children {
            let requires: BTreeSet<Name> = layout[child].requires.iter().cloned().collect();
            if requires.is_empty() || !requires.is_subset(parents) {
                return Err(format!(
                    "saved randomized {kind} {label} node {child:?} has prerequisites outside the preceding column"
                ));
            }
            used.extend(requires);
        }
        if used != *parents {
            return Err(format!(
                "saved randomized {kind} {label} column leaves a preceding node disconnected"
            ));
        }
        Ok(())
    };
    validate_layer(&first, &previous_leaves, "first-column")?;
    validate_layer(&second, &first, "second-column")?;

    let terminal = terminals.pop().unwrap();
    let terminal_requires: BTreeSet<Name> = layout[&terminal].requires.iter().cloned().collect();
    if let Some(gateway) = gateways.pop() {
        let gateway_requires: BTreeSet<Name> = layout[&gateway].requires.iter().cloned().collect();
        if gateway_requires != second || terminal_requires != BTreeSet::from([gateway]) {
            return Err(format!(
                "saved randomized {kind} gateway and terminal do not close the second column"
            ));
        }
    } else if terminal_requires != second {
        return Err(format!(
            "saved randomized {kind} terminal does not close the second column"
        ));
    }

    for (name, entry) in layout {
        let spec = tree
            .get_interned_mut(*name)
            .expect("the randomized tree entry was just checked");
        spec.cost = entry.cost;
        spec.requires = entry.requires.clone();
    }
    Ok(())
}

/// The transitive prerequisites of every node in a tree.
pub(crate) fn ancestry(nodes: &SpecMap<TechSpec>) -> SpecMap<BTreeSet<String>> {
    let mut ancestry: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, spec) in nodes.iter() {
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut pending: Vec<&str> = spec.requires.iter().map(|name| name.as_str()).collect();
        while let Some(node) = pending.pop() {
            // A node several paths reach is only followed once, which is also
            // what keeps a malformed cyclic tree from spinning here.
            if !reached.insert(node.to_string()) {
                continue;
            }
            if let Some(parent) = nodes.get(node) {
                pending.extend(parent.requires.iter().map(|name| name.as_str()));
            }
        }
        ancestry.insert(name.to_string(), reached);
    }
    SpecMap::from(ancestry)
}

/// Ordered contributors to one global technology/civic effect.
#[derive(Clone, Default)]
pub struct TreeEffectSources {
    pub techs: Vec<(Name, f64)>,
    pub civics: Vec<(Name, f64)>,
}

/// Invert both trees while retaining their separate node orders.
fn effect_sources(
    techs: &SpecMap<TechSpec>,
    civics: &SpecMap<TechSpec>,
) -> SpecMap<TreeEffectSources> {
    let mut sources: BTreeMap<String, TreeEffectSources> = BTreeMap::new();
    for (nodes, technology) in [(techs, true), (civics, false)] {
        for (name, spec) in nodes.iter() {
            for (effect, value) in &spec.effects {
                let entry = sources.entry(effect.clone()).or_default();
                let list = if technology {
                    &mut entry.techs
                } else {
                    &mut entry.civics
                };
                list.push((*name, *value));
            }
        }
    }
    SpecMap::from(sources)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WmdSpec {
    pub blast_radius: i32,
    pub fallout_duration: u32,
    pub icbm_strike_range: i32,
    pub maintenance: f64,
}

/// The shipped per-era ladder: Great Person recruitment base cost, embarked
/// unit combat strength, and the warmonger weight of a declaration.
#[derive(Clone, Serialize, Deserialize)]
pub struct EraSpec {
    pub great_person_base_cost: f64,
    pub embarked_strength: f64,
    #[serde(default)]
    pub warmonger_points: f64,
    /// Shipped `Eras_XP1.LiberatedEnvoys`: how many Envoys liberating a
    /// city-state in this era grants its liberator.
    #[serde(default)]
    pub liberated_envoys: u32,
}

/// One tribal village reward: its selection weight within the rolled
/// category, the earliest turn it appears, whether it needs a founded city,
/// and what it grants.
#[derive(Clone, Serialize, Deserialize)]
pub struct GoodyRewardSpec {
    pub weight: i64,
    #[serde(default)]
    pub min_turn: u32,
    #[serde(default)]
    pub requires_city: bool,
    #[serde(default)]
    pub reward: BTreeMap<String, f64>,
}

/// One Gathering Storm random-disaster class.
///
/// The shipped `Expansion2_RandomEvents.xml` rates are not published outside an
/// installation, so the tuning lives here as data rather than in the engine:
/// `per_game` is the expected number of occurrences over a full Standard-speed
/// game at Moderate disaster intensity, and every per-severity list is indexed
/// by severity tier minus one. Civ VI rolls three tiers for floods, droughts and
/// eruptions and two for storms.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DisasterSpec {
    pub per_game: f64,
    #[serde(default = "one_u8")]
    pub severities: u8,
    /// HP removed from every unit caught in the affected area.
    #[serde(default)]
    pub unit_damage: f64,
    /// Chance an improvement or district in the area is pillaged.
    #[serde(default)]
    pub pillage_chance: Vec<f64>,
    /// Chance an affected tile gains a permanent point of FOOD fertility.
    ///
    /// Taken from `RandomEvent_Yields` in `Expansion2_RandomEvents.xml`, whose
    /// rows are per yield type and not one rate for "fertility": the same
    /// eruption that has a 50% chance of leaving Food has a separate 25% chance
    /// of leaving Production, and they are rolled apart. See
    /// [`Self::fertility_production_chance`].
    #[serde(default)]
    pub fertility_chance: Vec<f64>,
    /// Chance an affected tile gains a permanent point of PRODUCTION fertility,
    /// the `YIELD_PRODUCTION` half of `RandomEvent_Yields`.
    ///
    /// ⚠ `Tile::disaster_production` has been summed into a tile's yields since
    /// the disasters shipped and **nothing ever wrote it**: fertility was
    /// modelled as Food alone, so a plot the game paid 3 Food 3 Production
    /// after an eruption was paid 3 Food 2 Production here. An absent list
    /// reads as zero, which is what every ruleset older than this field meant.
    #[serde(default)]
    pub fertility_production_chance: Vec<f64>,
    /// Citizens a city in the area loses.
    #[serde(default)]
    pub population_loss: Vec<i32>,
    /// Turns the effect persists — droughts linger, storms drift.
    #[serde(default)]
    pub duration: Vec<u32>,
    /// Tile radius of the affected area.
    #[serde(default)]
    pub radius: Vec<i32>,
    /// Terrains a storm of this class forms over.
    #[serde(default)]
    pub terrains: Vec<Name>,
}

impl DisasterSpec {
    /// A per-severity entry, clamped to the list the ruleset supplies so a
    /// short or absent list degrades to its last value rather than panicking.
    fn tier<T: Copy + Default>(list: &[T], severity: u8) -> T {
        if list.is_empty() {
            return T::default();
        }
        list[(severity.max(1) as usize - 1).min(list.len() - 1)]
    }

    pub fn pillage_chance(&self, severity: u8) -> f64 {
        Self::tier(&self.pillage_chance, severity)
    }

    pub fn fertility_chance(&self, severity: u8) -> f64 {
        Self::tier(&self.fertility_chance, severity)
    }

    pub fn fertility_production_chance(&self, severity: u8) -> f64 {
        Self::tier(&self.fertility_production_chance, severity)
    }

    pub fn population_loss(&self, severity: u8) -> i32 {
        Self::tier(&self.population_loss, severity)
    }

    pub fn duration(&self, severity: u8) -> u32 {
        Self::tier(&self.duration, severity)
    }

    pub fn radius(&self, severity: u8) -> i32 {
        Self::tier(&self.radius, severity)
    }
}

fn one_u8() -> u8 {
    1
}

/// One Rise & Fall Dedication. Every Dedication has two halves: the Normal-Age
/// one, which turns the behaviour it names into Era Score whatever age chose
/// it, and the Golden-Age one, which only a Golden or Heroic Age turns on.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DedicationSpec {
    /// Inclusive world-era span the Dedication can be chosen in, as indices
    /// into [`ERA_NAMES`].
    pub eras: (usize, usize),
    /// Era Score this Dedication pays for each named trigger.
    #[serde(default)]
    pub triggers: BTreeMap<String, i64>,
    /// The Normal-Age text, for clients and the Civilopedia.
    #[serde(default)]
    pub normal: String,
    /// The Golden-Age text. Its effects live in the engine.
    #[serde(default)]
    pub golden: String,
}

impl DedicationSpec {
    pub fn available_in(&self, era: usize) -> bool {
        era >= self.eras.0 && era <= self.eras.1
    }
}

/// One city-state seat: the identity a game stores, the shipped type whose
/// 1/3/6 Envoy thresholds it pays, and the Suzerain bonus it carries.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CityStateSpec {
    pub name: String,
    /// `scientific`, `cultural`, `religious`, `militaristic`, `industrial` or
    /// `trade` — the shipped `MinorCivBonuses` rows.
    #[serde(rename = "type")]
    pub kind: String,
    /// Engine key for the bespoke Suzerain bonus, absent for a seat that
    /// carries only its type bonus.
    #[serde(default)]
    pub bonus: Option<String>,
    /// Whether the engine actually implements `bonus`. A seat whose bonus is
    /// declared but unimplemented still pays its type bonus; this flag is what
    /// `every_declared_suzerain_bonus_is_implemented` reads so an unfinished
    /// entry cannot be mistaken for a working one.
    #[serde(default)]
    pub implemented: bool,
    /// Whether Civilization VI itself seats this city-state. The roster keeps
    /// names beyond the shipped 48 so the largest maps have distinct
    /// identities to hand out.
    #[serde(default)]
    pub shipped: bool,
    /// The shipped Suzerain text, for clients and the Civilopedia.
    #[serde(default)]
    pub effect: String,
    /// Where this city-state really stood, in the WGS84 degrees
    /// `data/leader_roster.json` already uses for a civilization's homeland.
    /// True Start Earth seats it here; every other script ignores the position
    /// but the selector still reads it, to keep a game's city-states spread
    /// across the world rather than piled into one region.
    ///
    /// ⚠ OPTIONAL, AND DELIBERATELY NOT A `TrueStartPoint` WITH A `Default`.
    /// `CityStateSpec` derives `Default`, so a flattened point would make a
    /// roster row that forgot its coordinates read as 0°N 0°E — the Gulf of
    /// Guinea, a real place a thousand miles from anywhere this roster names,
    /// and indistinguishable from a genuine answer. A mod overlay that omits
    /// them should fall back to the regional model, so absence is a state the
    /// data can hold and `site()` is the only way to ask.
    ///
    /// Four sites carry two names apiece — Visby/Wisby, Turku City/Abo,
    /// Bandar Brunei/Brunei, Ayutthaya/Ayutthaya City — and both entries hold
    /// the same real coordinates, because that is what is true of them.
    /// Declining to seat both is the selector's job, not the datum's.
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
}

impl CityStateSpec {
    /// The real site, when the roster carries one.
    pub fn site(&self) -> Option<crate::leader_roster::TrueStartPoint> {
        match (self.latitude, self.longitude) {
            (Some(latitude), Some(longitude)) => Some(crate::leader_roster::TrueStartPoint {
                latitude,
                longitude,
            }),
            _ => None,
        }
    }
}

/// The city-state roster in seating order.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CityStateRoster {
    pub roster: Vec<CityStateSpec>,
}

/// Every ruleset file the engine ships, by the name a mod overlay uses.
pub const DATA_FILES: [(&str, &str); 30] = [
    ("terrains", include_str!("../data/terrains.json")),
    ("features", include_str!("../data/features.json")),
    ("resources", include_str!("../data/resources.json")),
    ("improvements", include_str!("../data/improvements.json")),
    ("units", include_str!("../data/units.json")),
    ("districts", include_str!("../data/districts.json")),
    ("buildings", include_str!("../data/buildings.json")),
    ("wonders", include_str!("../data/wonders.json")),
    ("great_people", include_str!("../data/great_people.json")),
    ("governors", include_str!("../data/governors.json")),
    ("projects", include_str!("../data/projects.json")),
    ("techs", include_str!("../data/techs.json")),
    ("civics", include_str!("../data/civics.json")),
    ("governments", include_str!("../data/governments.json")),
    ("policies", include_str!("../data/policies.json")),
    ("promotions", include_str!("../data/promotions.json")),
    ("modifiers", include_str!("../data/modifiers.json")),
    ("beliefs", include_str!("../data/beliefs.json")),
    ("civs", include_str!("../data/civs.json")),
    ("agendas", include_str!("../data/agendas.json")),
    ("difficulties", include_str!("../data/difficulties.json")),
    ("speeds", include_str!("../data/speeds.json")),
    ("goody_huts", include_str!("../data/goody_huts.json")),
    ("eras", include_str!("../data/eras.json")),
    ("wmds", include_str!("../data/wmds.json")),
    ("tree_effects", include_str!("../data/tree_effects.json")),
    ("disasters", include_str!("../data/disasters.json")),
    ("dedications", include_str!("../data/dedications.json")),
    (
        "historic_moments",
        include_str!("../data/historic_moments.json"),
    ),
    ("city_states", include_str!("../data/city_states.json")),
];

fn add_effects(target: &mut BTreeMap<String, f64>, source: &BTreeMap<String, f64>) {
    for (effect, value) in source {
        *target.entry(effect.clone()).or_insert(0.0) += value;
    }
}

/// Resolve the modifier graph into bundles that contain no further links.
fn resolve_modifiers(source: &SpecMap<ModifierSpec>) -> Result<SpecMap<ModifierSpec>, String> {
    fn resolve_one(
        name: &str,
        source: &SpecMap<ModifierSpec>,
        resolved: &mut SpecMap<ModifierSpec>,
        stack: &mut Vec<String>,
    ) -> Result<ModifierSpec, String> {
        if let Some(spec) = resolved.get(name) {
            return Ok(spec.clone());
        }
        let Some(spec) = source.get(name) else {
            let owner = stack.last().map(|name| name.as_str()).unwrap_or("ruleset");
            return Err(format!("modifier {owner} attaches missing modifier {name}"));
        };
        spec.requirements.validate(name)?;
        if let Some(start) = stack.iter().position(|entry| entry == name) {
            let mut cycle = stack[start..].to_vec();
            cycle.push(name.to_string());
            return Err(format!("modifier attachment cycle: {}", cycle.join(" -> ")));
        }

        stack.push(name.to_string());
        let mut effects = spec.effects.clone();
        compile_modifier_selectors(name, spec, &mut effects)?;
        for attached in &spec.modifiers {
            let nested = resolve_one(attached, source, resolved, stack)?;
            if nested.collection != spec.collection || !nested.requirements.is_empty() {
                return Err(format!("modifier {name} cannot flatten contextual attachment {attached}; nested modifiers must use the parent collection and no requirements"));
            }
            add_effects(&mut effects, &nested.effects);
        }
        stack.pop();

        let flat = ModifierSpec {
            effects,
            building_yields: BTreeMap::new(),
            unit_purchase_discount_pct: BTreeMap::new(),
            abilities: BTreeSet::new(),
            modifiers: Vec::new(),
            collection: spec.collection,
            requirements: spec.requirements.clone(),
        };
        resolved.insert(name.to_string(), flat.clone());
        Ok(flat)
    }

    let mut resolved = SpecMap::new();
    for name in source.keys() {
        resolve_one(name, source, &mut resolved, &mut Vec::new())?;
    }
    Ok(resolved)
}

/// Expand `modifiers: [..]` on any rules object into its local effect map.
/// Walking raw JSON keeps the primitive available uniformly to buildings,
/// policies, technologies, beliefs, promotions, and mod-defined content
/// without adding a parallel attachment field to every individual spec.
fn expand_modifier_attachments(
    file: &str,
    value: &mut serde_json::Value,
    modifiers: &SpecMap<ModifierSpec>,
) -> Result<(), String> {
    fn walk(
        value: &mut serde_json::Value,
        path: &str,
        modifiers: &SpecMap<ModifierSpec>,
    ) -> Result<(), String> {
        match value {
            serde_json::Value::Array(values) => {
                for (index, value) in values.iter_mut().enumerate() {
                    walk(value, &format!("{path}[{index}]"), modifiers)?;
                }
            }
            serde_json::Value::Object(object) => {
                let attached = object.remove("modifiers");
                for (name, value) in object.iter_mut() {
                    walk(value, &format!("{path}.{name}"), modifiers)?;
                }
                let Some(attached) = attached else {
                    return Ok(());
                };
                let serde_json::Value::Array(attached) = attached else {
                    return Err(format!("{path}.modifiers must be an array of names"));
                };
                let effects = object
                    .entry("effects".to_string())
                    .or_insert_with(|| serde_json::Value::Object(Default::default()));
                let Some(effects) = effects.as_object_mut() else {
                    return Err(format!("{path}.effects must be an object"));
                };
                for reference in attached {
                    let Some(name) = reference.as_str() else {
                        return Err(format!("{path}.modifiers must contain only names"));
                    };
                    let Some(modifier) = modifiers.get(name) else {
                        return Err(format!("{path} attaches missing modifier {name}"));
                    };
                    if modifier.collection != ModifierCollection::Player
                        || !modifier.requirements.is_empty()
                    {
                        return Err(format!("{path} attaches contextual modifier {name}; use a runtime player attachment instead"));
                    }
                    for (effect, value) in &modifier.effects {
                        let previous = effects
                            .get(effect)
                            .and_then(serde_json::Value::as_f64)
                            .unwrap_or(0.0);
                        let Some(sum) = serde_json::Number::from_f64(previous + value) else {
                            return Err(format!("{path}.{effect} is not a finite effect"));
                        };
                        effects.insert(effect.clone(), serde_json::Value::Number(sum));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    walk(value, &format!("{file}.json"), modifiers)
}

/// The ruleset every `Rules::embedded()` call sees. It is the shipped data
/// until a mod overlay is installed, which can only happen once, before a
/// game exists. Keeping it here rather than threading a ruleset through every
/// call site is what lets a save deserialize without knowing about mods.
static ACTIVE: OnceLock<Rules> = OnceLock::new();

/// The same ruleset behind a handle, so games can share one copy.
static SHARED: OnceLock<Arc<Rules>> = OnceLock::new();

/// The JSON the active ruleset was built from, kept so a per-game overlay can
/// merge onto the same data a mod already changed instead of onto the shipped
/// files underneath it. Absent until mods are installed, which is the ordinary
/// checkout: there the shipped values are the active ones.
static ACTIVE_VALUES: OnceLock<BTreeMap<String, serde_json::Value>> = OnceLock::new();

/// The active ruleset with the Modified Future Era merged on, built at most
/// once per process and shared exactly like the stock one.
static MODIFIED_FUTURE_ERA: OnceLock<Arc<Rules>> = OnceLock::new();

/// The Modified Future Era, as the same kind of JSON overlay any mod is.
///
/// It is embedded from `mods/modified-future-era/` rather than copied into
/// `data/`, so there is one source of truth for it: the lobby setting loads
/// these bytes, and `--mods mods/modified-future-era` loads the same folder
/// off disk. Growing it is editing those files, not this array.
pub const FUTURE_ERA_MODIFIED_FILES: [(&str, &str); 2] = [
    (
        "projects",
        include_str!("../mods/modified-future-era/projects.json"),
    ),
    (
        "resources",
        include_str!("../mods/modified-future-era/resources.json"),
    ),
];

impl Rules {
    /// Stable identity of the effective rules data. The algorithm name is
    /// part of the value so a future fingerprint upgrade is an explicit
    /// protocol change rather than an accidental mismatch.
    pub fn source_fingerprint(&self) -> &str {
        &self.source_fingerprint
    }

    /// The active ruleset — shipped data unless mods were installed.
    pub fn embedded() -> Rules {
        ACTIVE.get_or_init(Rules::shipped).clone()
    }

    /// The active ruleset, shared rather than copied.
    ///
    /// A game holds the whole ruleset, and the AI's tactical search clones a
    /// game for every branch it examines. Copying several hundred specs —
    /// each with its own strings and lists — for a search that never writes
    /// to any of them was the largest single cost in a simulated turn.
    pub fn shared() -> Arc<Rules> {
        SHARED
            .get_or_init(|| Arc::new(ACTIVE.get_or_init(Rules::shipped).clone()))
            .clone()
    }

    /// Record the JSON an installed ruleset was built from. Called once by
    /// [`crate::mods::activate`]; without it the shipped files are the active
    /// ones, which is what an unmodded checkout has.
    pub fn record_active_values(values: BTreeMap<String, serde_json::Value>) {
        let _ = ACTIVE_VALUES.set(values);
    }

    /// The JSON behind the active ruleset: the shipped files, with any
    /// installed mod already merged into them.
    pub fn active_values() -> BTreeMap<String, serde_json::Value> {
        ACTIVE_VALUES
            .get()
            .cloned()
            .unwrap_or_else(Rules::shipped_values)
    }

    /// The active ruleset plus the Modified Future Era.
    ///
    /// The overlay merges onto [`Rules::active_values`] rather than onto the
    /// shipped files, so a game played with both a mod and this Future Era
    /// gets both rather than silently losing the mod.
    pub fn modified_future_era() -> Arc<Rules> {
        MODIFIED_FUTURE_ERA
            .get_or_init(|| {
                let mut values = Rules::active_values();
                for (file, text) in FUTURE_ERA_MODIFIED_FILES {
                    let overlay: serde_json::Value =
                        serde_json::from_str(text).unwrap_or_else(|error| {
                            panic!("the Modified Future Era's {file}.json is malformed: {error}")
                        });
                    let base = values.get_mut(file).unwrap_or_else(|| {
                        panic!("the Modified Future Era overlays unknown file {file}.json")
                    });
                    crate::mods::merge(base, overlay).unwrap_or_else(|error| {
                        panic!("cannot merge the Modified Future Era's {file}.json: {error}")
                    });
                }
                Arc::new(Rules::from_values(values).unwrap_or_else(|error| {
                    panic!("the Modified Future Era does not load: {error}")
                }))
            })
            .clone()
    }

    /// Build the immutable rules snapshot for one match. Gathering Storm's
    /// Future trees are the only rules rows that vary by game seed; cloning
    /// once here keeps that variation out of every hot-path query, while
    /// tactical/search clones continue to share the resulting [`Arc`].
    ///
    /// `future_era` is the one lobby setting that changes the rules rather
    /// than the world, so it is resolved here too: the classic era is the
    /// active ruleset unchanged, and the modified one is that ruleset with the
    /// Moon's ore and the mass driver merged onto it.
    pub fn for_game(
        seed: u64,
        saved: Option<&FutureTreeLayout>,
        future_era: crate::setup::FutureEra,
    ) -> Arc<Rules> {
        let shared = match future_era {
            crate::setup::FutureEra::Classic => Self::shared(),
            crate::setup::FutureEra::Modified => Self::modified_future_era(),
        };
        let layout = match saved {
            Some(layout) => layout.clone(),
            None => FutureTreeLayout::generate(&shared, seed)
                .unwrap_or_else(|error| panic!("cannot generate Future research trees: {error}")),
        };
        if layout.is_empty() {
            return shared;
        }
        let mut rules = (*shared).clone();
        apply_tree_layout("technology", &mut rules.techs, &layout.techs)
            .unwrap_or_else(|error| panic!("cannot install Future technology tree: {error}"));
        apply_tree_layout("civic", &mut rules.civics, &layout.civics)
            .unwrap_or_else(|error| panic!("cannot install Future civic tree: {error}"));
        rules.tech_ancestors = ancestry(&rules.techs);
        rules.civic_ancestors = ancestry(&rules.civics);
        Arc::new(rules)
    }

    /// Extract the generated part of a live rules snapshot for save/restore.
    /// Persisting the concrete graph means a future engine update cannot
    /// silently reroll an in-progress match.
    pub fn future_tree_layout(&self) -> FutureTreeLayout {
        let extract = |tree: &SpecMap<TechSpec>| {
            tree.iter()
                .filter(|(_, spec)| spec.random_prereqs)
                .map(|(name, spec)| {
                    (
                        *name,
                        TreeLayoutEntry {
                            cost: spec.cost,
                            requires: spec.requires.clone(),
                        },
                    )
                })
                .collect()
        };
        FutureTreeLayout {
            techs: extract(&self.techs),
            civics: extract(&self.civics),
        }
    }

    /// The shipped ruleset, ignoring any installed mods.
    pub fn shipped() -> Rules {
        Rules::from_values(Rules::shipped_values()).expect("the shipped ruleset is well formed")
    }

    /// The shipped data as raw JSON, which is what a mod overlay merges into.
    pub fn shipped_values() -> BTreeMap<String, serde_json::Value> {
        DATA_FILES
            .iter()
            .map(|(name, text)| {
                let value = serde_json::from_str(text)
                    .unwrap_or_else(|error| panic!("shipped {name}.json is malformed: {error}"));
                (name.to_string(), value)
            })
            .collect()
    }

    /// The shipped ruleset files with extra bundles merged into the imported
    /// modifier catalog.
    ///
    /// Tests used to hand `Rules::from_values` a `modifiers` map containing
    /// only their own fixture. That was harmless while `data/modifiers.json`
    /// was empty and is not now: the catalog is imported from the shipped
    /// `Modifiers` tables, and civics, technologies, wonders, districts,
    /// buildings, governments, promotions and Great People attach its bundles
    /// by name. Replacing it outright leaves every one of those references
    /// dangling, so the ruleset refuses to build and the fixture's own subject
    /// is never reached.
    #[cfg(test)]
    pub(crate) fn shipped_values_with(
        bundles: serde_json::Value,
    ) -> BTreeMap<String, serde_json::Value> {
        let mut files = Rules::shipped_values();
        let catalog = files
            .get_mut("modifiers")
            .and_then(serde_json::Value::as_object_mut)
            .expect("the shipped modifier catalog is an object");
        let serde_json::Value::Object(bundles) = bundles else {
            panic!("test bundles must be a JSON object");
        };
        catalog.extend(bundles);
        files
    }

    /// Install a ruleset as the active one. Fails if a game has already read
    /// the ruleset, because half a game on one set of rules and half on
    /// another is not a state worth supporting.
    pub fn install(rules: Rules) -> Result<(), String> {
        ACTIVE
            .set(rules)
            .map_err(|_| "the ruleset is already in use and cannot be replaced".to_string())
    }

    /// Build a ruleset from raw JSON, one value per entry in [`DATA_FILES`].
    pub fn from_values(mut files: BTreeMap<String, serde_json::Value>) -> Result<Rules, String> {
        // BTreeMap and serde_json's default sorted object map make this byte
        // representation stable across processes. FNV-1a is used as a compact
        // change detector, not as a security boundary.
        let encoded = serde_json::to_vec(&files)
            .map_err(|error| format!("cannot fingerprint ruleset: {error}"))?;
        let mut fingerprint = 0xcbf29ce484222325u64;
        for byte in encoded {
            fingerprint ^= u64::from(byte);
            fingerprint = fingerprint.wrapping_mul(0x100000001b3);
        }
        let source_fingerprint = format!("fnv1a64:{fingerprint:016x}");

        fn take<T: serde::de::DeserializeOwned>(
            files: &mut BTreeMap<String, serde_json::Value>,
            name: &str,
        ) -> Result<T, String> {
            let value = files
                .remove(name)
                .ok_or_else(|| format!("ruleset is missing {name}.json"))?;
            serde_json::from_value(value).map_err(|error| format!("{name}.json: {error}"))
        }
        let modifiers = resolve_modifiers(&take(&mut files, "modifiers")?)?;
        for (name, value) in files.iter_mut() {
            expand_modifier_attachments(name, value, &modifiers)?;
        }
        let mut rules = Rules {
            source_fingerprint,
            terrains: take(&mut files, "terrains")?,
            features: take(&mut files, "features")?,
            resources: take(&mut files, "resources")?,
            improvements: take(&mut files, "improvements")?,
            units: take(&mut files, "units")?,
            districts: take(&mut files, "districts")?,
            buildings: take(&mut files, "buildings")?,
            wonders: take(&mut files, "wonders")?,
            great_people: take(&mut files, "great_people")?,
            governors: take(&mut files, "governors")?,
            projects: take(&mut files, "projects")?,
            techs: take(&mut files, "techs")?,
            civics: take(&mut files, "civics")?,
            governments: take(&mut files, "governments")?,
            policies: take(&mut files, "policies")?,
            promotions: take(&mut files, "promotions")?,
            modifiers,
            beliefs: take(&mut files, "beliefs")?,
            civs: take(&mut files, "civs")?,
            agendas: take(&mut files, "agendas")?,
            difficulties: take(&mut files, "difficulties")?,
            speeds: take(&mut files, "speeds")?,
            goody_huts: take(&mut files, "goody_huts")?,
            eras: take(&mut files, "eras")?,
            wmds: take(&mut files, "wmds")?,
            disasters: take(&mut files, "disasters")?,
            dedications: take(&mut files, "dedications")?,
            historic_moments: take(&mut files, "historic_moments")?,
            city_states: take(&mut files, "city_states")?,
            tree_effects: SpecMap::default(),
            tech_ancestors: SpecMap::default(),
            civic_ancestors: SpecMap::default(),
            effect_index: EffectIndex::default(),
            district_adjacency_families: SpecMap::default(),
        };
        let effects: TreeEffectsData = take(&mut files, "tree_effects")?;
        for (node, values) in effects.techs {
            let spec = rules
                .techs
                .get_mut(&node)
                .ok_or_else(|| format!("tree_effects.json references missing technology {node}"))?;
            add_effects(&mut spec.effects, &values);
        }
        for (node, values) in effects.civics {
            let spec = rules
                .civics
                .get_mut(&node)
                .ok_or_else(|| format!("tree_effects.json references missing civic {node}"))?;
            add_effects(&mut spec.effects, &values);
        }
        if rules.historic_moments.is_empty() {
            return Err("historic_moments.json contains no positive Moments".to_string());
        }
        for (moment, spec) in &rules.historic_moments {
            if !moment.starts_with("MOMENT_") || spec.era_score <= 0 {
                return Err(format!(
                    "historic_moments.json entry {moment} must be a positive MOMENT_* row"
                ));
            }
            for (field, era) in [
                ("minimum_game_era", spec.minimum_game_era),
                ("maximum_game_era", spec.maximum_game_era),
                ("obsolete_era", spec.obsolete_era),
            ] {
                if era.is_some_and(|era| era >= ERA_NAMES.len()) {
                    return Err(format!(
                        "historic_moments.json entry {moment} has invalid {field}"
                    ));
                }
            }
            if spec.minimum_game_era.unwrap_or(0)
                > spec.maximum_game_era.unwrap_or(ERA_NAMES.len() - 1)
                || spec.obsolete_era.is_some_and(|obsolete| {
                    obsolete <= spec.minimum_game_era.unwrap_or(0)
                        || spec
                            .maximum_game_era
                            .is_some_and(|maximum| maximum >= obsolete)
                })
            {
                return Err(format!(
                    "historic_moments.json entry {moment} has an empty era window"
                ));
            }
        }
        rules.index_tree_unlocks();
        rules.tree_effects = effect_sources(&rules.techs, &rules.civics);
        rules.tech_ancestors = ancestry(&rules.techs);
        rules.civic_ancestors = ancestry(&rules.civics);
        rules.effect_index = rules.build_effect_index();
        rules.district_adjacency_families = rules.build_district_adjacency_families();
        Ok(rules)
    }

    /// Resolve every district-naming adjacency key to the family it counts,
    /// once. See [`Rules::district_adjacency_families`].
    ///
    /// The membership test is `SpecMap::contains_key(&str)`, which probes by
    /// string hash and does not intern, so this is exactly the guard the count
    /// table applies — and because every key that passes it was already
    /// interned as a district id at load, the `Name::new` below cannot hand
    /// out a new id.
    fn build_district_adjacency_families(&self) -> SpecMap<Vec<Option<Name>>> {
        self.districts
            .iter()
            .map(|(district, spec)| {
                let families = spec
                    .adjacency
                    .keys()
                    .map(|key| {
                        self.districts
                            .contains_key(key.as_str())
                            .then(|| self.district_family_of(Name::new(key.as_str())))
                    })
                    .collect::<Vec<_>>();
                (district.to_string(), families)
            })
            .collect()
    }

    /// The base family `district` replaces, following the chain. The
    /// ruleset-side twin of `Game::district_family`, which cannot be used here
    /// because no `Game` exists while the ruleset is still being built.
    fn district_family_of(&self, district: Name) -> Name {
        let mut current = district;
        for _ in 0..self.districts.len() {
            let Some(parent) = self
                .districts
                .get_interned(current)
                .and_then(|spec| spec.replaces)
            else {
                break;
            };
            current = parent;
        }
        current
    }

    /// Index which effect keys each family of specs declares. See
    /// [`EffectIndex`] for why the collection paths ask this first.
    fn build_effect_index(&self) -> EffectIndex {
        let beliefs = [
            &self.beliefs.pantheon,
            &self.beliefs.founder,
            &self.beliefs.follower,
            &self.beliefs.enhancer,
            &self.beliefs.worship,
        ];
        let index = EffectIndex {
            policies: effect_key_set(self.policies.values().flat_map(|spec| spec.effects.keys())),
            civs: effect_key_set(self.civs.values().flat_map(|spec| spec.effects.keys())),
            buildings: effect_key_set(self.buildings.values().flat_map(|spec| spec.effects.keys())),
            districts: effect_key_set(self.districts.values().flat_map(|spec| spec.effects.keys())),
            wonders: effect_key_set(self.wonders.values().flat_map(|spec| spec.effects.keys())),
            beliefs: effect_key_set(
                beliefs
                    .into_iter()
                    .flat_map(|table| table.values())
                    .flat_map(|spec| spec.effects.keys()),
            ),
            // A Governor grants through the title itself and through each
            // promotion its holder has taken.
            governors: effect_key_set(self.governors.values().flat_map(|spec| {
                spec.effects.keys().chain(
                    spec.promotions
                        .values()
                        .flat_map(|promotion| promotion.effects.keys()),
                )
            })),
            any: SpecMap::new(),
            building_yield_selectors: SpecMap::new(),
            unit_purchase_selectors: SpecMap::new(),
            granted_abilities: SpecMap::new(),
        };
        // The union has to include the trees, which are indexed by effect
        // already, even though no caller asks about them on their own.
        let mut any = SpecMap::new();
        for family in [
            &index.policies,
            &index.civs,
            &index.buildings,
            &index.districts,
            &index.wonders,
            &index.beliefs,
            &index.governors,
        ] {
            for key in family.keys() {
                any.insert(key.to_string(), ());
            }
        }
        for key in self.tree_effects.keys() {
            any.insert(key.to_string(), ());
        }
        // Split the three namespaced families back into the selectors they
        // name. A selector may not itself contain a colon — the modifier
        // compiler rejects that — so the shape of each key is exact.
        let mut building_yield_selectors = SpecMap::new();
        let mut unit_purchase_selectors = SpecMap::new();
        let mut granted_abilities = SpecMap::new();
        for key in any.keys() {
            if let Some(rest) = key.strip_prefix(BUILDING_YIELD_EFFECT_PREFIX) {
                if let Some((selector, _yield_type)) = rest.split_once(':') {
                    building_yield_selectors.insert(selector.to_string(), ());
                }
            } else if let Some(unit) = key.strip_prefix(UNIT_PURCHASE_EFFECT_PREFIX) {
                unit_purchase_selectors.insert(unit.to_string(), ());
            } else if let Some(ability) = key.strip_prefix(GRANT_ABILITY_EFFECT_PREFIX) {
                granted_abilities.insert(ability.to_string(), ());
            }
        }
        EffectIndex {
            any,
            building_yield_selectors,
            unit_purchase_selectors,
            granted_abilities,
            ..index
        }
    }

    /// Build the one authoritative unlock list from each content object's
    /// technology/civic gate. This prevents the UI, legality checks, and tree
    /// documentation from drifting into three separate catalogs.
    fn index_tree_unlocks(&mut self) {
        let mut indexed: Vec<(bool, String, TreeUnlock)> = Vec::new();
        let mut add = |kind: &str, id: &str, tech: &Option<Name>, civic: &Option<Name>| {
            if let Some(node) = tech {
                indexed.push((
                    true,
                    node.to_string(),
                    TreeUnlock {
                        kind: kind.to_string(),
                        id: id.to_string(),
                    },
                ));
            }
            if let Some(node) = civic {
                indexed.push((
                    false,
                    node.to_string(),
                    TreeUnlock {
                        kind: kind.to_string(),
                        id: id.to_string(),
                    },
                ));
            }
        };
        for (id, spec) in &self.units {
            add("unit", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.buildings {
            add("building", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.wonders {
            add("wonder", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.districts {
            add("district", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.improvements {
            add("improvement", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.resources {
            add("resource", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.projects {
            add("project", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &self.policies {
            add("policy", id, &None, &spec.civic);
        }
        for (id, spec) in &self.governments {
            add("government", id, &None, &spec.civic);
        }

        for spec in self.techs.values_mut().chain(self.civics.values_mut()) {
            spec.unlocks.clear();
        }
        for (technology, node, unlock) in indexed {
            let tree = if technology {
                &mut self.techs
            } else {
                &mut self.civics
            };
            // A gate naming a node that does not exist is a ruleset defect,
            // and `civvis validate` reports it as one with the file and entry
            // to fix. Indexing simply skips it: panicking here would turn
            // every bad mod into a crash instead of a message.
            if let Some(spec) = tree.get_mut(&node) {
                spec.unlocks.push(unlock);
            }
        }
        for spec in self.techs.values_mut().chain(self.civics.values_mut()) {
            spec.unlocks
                .sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
        }
    }

    /// The catalogue's sum for a tile: terrain, hills, feature, resource,
    /// improvement. This is what map generation and settle valuation read, and
    /// its outputs are pinned by the cross-platform world hashes, so it keeps
    /// the plain additive shape. What a Citizen is PAID for the tile is
    /// [`Self::worked_tile_yields`], which applies the one host rule the sum
    /// misses.
    pub fn tile_yields(&self, t: &Tile) -> Yields {
        let mut ys = self.terrains[t.terrain].yields;
        if t.hills {
            ys.production += 1.0;
        }
        if let Some(f) = &t.feature {
            ys.add(self.features[f].yields);
        }
        if let Some(r) = &t.resource {
            ys.add(self.resources[r].yields);
        }
        if let Some(i) = &t.improvement {
            ys.add(self.improvements[i].yields);
        }
        ys
    }

    /// What a Citizen is paid for working the tile, as the host pays it.
    ///
    /// One rule on top of [`Self::tile_yields`]: a natural wonder's tile pays
    /// the wonder's own `Feature_YieldChanges` and nothing from the terrain or
    /// hills under it. Ubsunur Hollow (1 Food, 1 Production, 2 Faith) on Tundra
    /// reads exactly 1/1/2 in the host, not 2/1/2 — 389 worked-tile-turns of +1
    /// Food on run civvis-20260816T040537Z, every one the terrain's Food added
    /// on top; the same is why the Great Barrier Reef shows 3 Food 2 Science and
    /// no Coast Gold. Resources and improvements (Ha Long Bay's fishing boats)
    /// still add. Kept out of `tile_yields` because map generation reads that
    /// one to place starts, and its worlds are pinned by hash.
    pub fn worked_tile_yields(&self, t: &Tile) -> Yields {
        let natural_wonder = t
            .feature
            .as_ref()
            .is_some_and(|f| self.features[f].natural_wonder);
        if !natural_wonder {
            return self.tile_yields(t);
        }
        let mut ys = Yields::default();
        if let Some(f) = &t.feature {
            ys.add(self.features[f].yields);
        }
        if let Some(r) = &t.resource {
            ys.add(self.resources[r].yields);
        }
        if let Some(i) = &t.improvement {
            ys.add(self.improvements[i].yields);
        }
        ys
    }

    /// Add the synthetic terrain used by an external partial-map reconstruction.
    /// This deliberately happens after ruleset loading so a mirror-only knowledge
    /// marker cannot change the audited source fingerprint or invalidate ratings.
    pub(crate) fn enable_unknown_terrain(&mut self) {
        self.terrains.insert(
            "unknown".to_string(),
            TerrainSpec {
                yields: Yields::default(),
                unknown: true,
                water: false,
                passable: false,
                move_cost: 1.0,
                defense: 0.0,
            },
        );
    }

    pub fn is_water(&self, t: &Tile) -> bool {
        self.terrains[t.terrain].water
    }

    /// Whether this tile carries a volcanic cone — the shipped
    /// `Features_XP2.Volcano` flag rather than the generic `volcano` feature
    /// name, so Vesuvius, Kilimanjaro and Eyjafjallajokull count as the
    /// volcanoes Gathering Storm says they are.
    pub fn is_volcano(&self, t: &Tile) -> bool {
        t.feature
            .as_deref()
            .is_some_and(|feature| self.features[feature].volcano)
    }

    pub fn is_unknown(&self, t: &Tile) -> bool {
        self.terrains[t.terrain].unknown
    }

    pub fn is_passable(&self, t: &Tile) -> bool {
        if let Some(f) = &t.feature {
            if self.features[f].impassable {
                return false;
            }
        }
        let terrain = &self.terrains[t.terrain];
        if terrain.unknown {
            t.assumed_traversable
        } else {
            terrain.passable
        }
    }

    pub fn move_cost(&self, t: &Tile) -> f64 {
        // Civ 6 movement is additive: terrain cost, +1 for Hills (the
        // database ships Hills as separate terrain rows costing 2), plus the
        // feature's MovementChange.
        let terrain = &self.terrains[t.terrain];
        let mut c = terrain.move_cost;
        if t.hills {
            c += 1.0;
        }
        if let Some(f) = &t.feature {
            c += self.features[f].move_cost;
        }
        // This is terrain cost only. Route discounts require a connection
        // from the origin and are applied by Game::unit_step_cost. Flattening
        // this tile alone underprices entry onto a road from off-road.
        c
    }
}

#[cfg(test)]
mod tests;
