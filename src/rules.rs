//! Ruleset loaded from the shared JSON data files (embedded at compile time).
use serde::{Deserialize, Serialize};

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
    /// Base terrains the wonder can stand on. `mountain` covers every coloured
    /// `TERRAIN_*_MOUNTAIN` variant, which CIVVIS spells as one terrain.
    #[serde(default)]
    pub terrain: Vec<String>,
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
    /// How many of those hexes are water rather than land. Only the Giant's
    /// Causeway, whose columns step off a headland into the sea, sets this.
    #[serde(default)]
    pub water_tiles: usize,
    /// At least one neighbour must be one of these terrains. Carries the
    /// `Coast` column for the shore wonders and `Feature_AdjacentTerrains` for
    /// the peaks, which must not be walled in by their own mountain range.
    #[serde(default)]
    pub adjacent_terrain: Vec<String>,
    /// No neighbour may be one of these. Carries `NoCoast` as `coast`.
    #[serde(default)]
    pub not_adjacent_terrain: Vec<String>,
    /// At least one neighbour must carry one of these features (Yosemite wants
    /// Woods, Ik-Kil wants Rainforest).
    #[serde(default)]
    pub adjacent_feature: Vec<String>,
    /// `Feature_NotNearFeatures`: no neighbour may carry one of these. Every
    /// shipped row names Sea Ice, keeping the water wonders out of the pack.
    #[serde(default)]
    pub avoid_feature: Vec<String>,
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
    /// Strategic and archaeological resources remain hidden until this node.
    #[serde(default)]
    pub tech: Option<String>,
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub yields: Yields,
    #[serde(default)]
    pub terrain: Vec<String>,
    #[serde(default)]
    pub feature: Vec<String>,
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
    pub tech: Option<String>,
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
    pub tech: Option<String>,
    /// `PlunderType` and `PlunderAmount`: which yield pillaging this pays and
    /// how much. Gold and heal pay 50, Science, Culture and Faith pay 25.
    #[serde(default)]
    pub plunder_type: Option<String>,
    #[serde(default)]
    pub plunder_amount: f64,
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub yields: Yields,
    #[serde(default)]
    pub housing: f64,
    #[serde(default)]
    pub terrain: Vec<String>,
    #[serde(default)]
    pub feature: Vec<String>,
    /// Features the improvement may also sit on once a civic is unlocked --
    /// Gathering Storm opens the Lumber Mill to Rainforest at Mercantilism.
    #[serde(default)]
    pub feature_after_civic: BTreeMap<String, String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub resource_only: bool,
    #[serde(default)]
    pub requires_hills: bool,
    #[serde(default)]
    pub hills_or_resource: bool,
    #[serde(default)]
    pub requires_flat: bool,
    #[serde(default)]
    pub unique_to: Option<String>,
    #[serde(default)]
    pub replaces: Option<String>,
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
    pub tech: Option<String>,
    #[serde(default)]
    pub requires_resource: Option<String>,
    /// Strategic material paid once when construction or purchase starts.
    #[serde(default)]
    pub resource_cost: f64,
    /// Strategic fuel consumed at the beginning of every owner turn.
    #[serde(default)]
    pub resource_maintenance: f64,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub unique_to: Option<String>, // civ that alone may build this unit
    #[serde(default)]
    pub replaces: Option<String>, // base unit this unique replaces
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
    pub requires_building: Option<String>,
    #[serde(default)]
    pub requires_district: Option<String>,
    /// Improvements this specialist can construct (builders use the whole
    /// ordinary improvement catalog; engineers/archaeologists are explicit).
    #[serde(default)]
    pub builds: Vec<String>,
    /// The unit this one becomes when upgraded for Gold, from the shipped
    /// `UnitUpgrades` table. A civilization's unique replacement stands in for
    /// the base unit whenever it owns one.
    #[serde(default, alias = "upgrades_to")]
    pub upgrade_to: Option<String>,
    /// The shipped `MandatoryObsoleteTech`. Once its owner researches this,
    /// the unit can no longer be trained or purchased; existing copies live on
    /// until they are upgraded.
    #[serde(default)]
    pub obsolete_tech: Option<String>,
    /// Data-driven auras and special unit rules. Support units currently use
    /// `adjacent_siege_range`, `adjacent_siege_bombard`, `adjacent_heal`, and
    /// `adjacent_movement`; unknown entries remain forward-compatible.
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
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
    pub tech: Option<String>,
    #[serde(default)]
    pub civic: Option<String>,
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
    pub replaces: Option<String>,
    /// IDs of district families that cannot coexist in the same city (for
    /// example Entertainment Complex and Water Park).
    #[serde(default)]
    pub excludes: Vec<String>,
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
    pub tech: Option<String>,
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub district: Option<String>,
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
    pub replaces: Option<String>,
    /// Buildings that must already exist in this city.
    #[serde(default)]
    pub requires: Vec<String>,
    /// At least one member of this list must exist. Replacement-family
    /// matching applies, so a unique replacement satisfies a base entry.
    #[serde(default)]
    pub requires_any: Vec<String>,
    /// Mutually exclusive buildings in the same tier or Government Plaza
    /// choice.
    #[serde(default)]
    pub excludes: Vec<String>,
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
    pub worship_belief: Option<String>,
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
    pub tech: Option<String>,
    #[serde(default)]
    pub civic: Option<String>,
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
    pub requires_buildings: Vec<String>,
    #[serde(default)]
    pub requires_any_buildings: Vec<String>,
    #[serde(default)]
    pub adjacent_district: Option<String>,
    #[serde(default)]
    pub adjacent_resource: Option<String>,
    #[serde(default)]
    pub adjacent_improvement: Option<String>,
    #[serde(default)]
    pub terrain: Vec<String>,
    #[serde(default)]
    pub feature: Vec<String>,
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
    pub requires: Vec<String>,
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
    pub tech: Option<String>,
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub district: Option<String>,
    #[serde(default)]
    pub alternate_districts: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub requires_buildings: Vec<String>,
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
    pub requires: Vec<String>,
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
    pub requires: Vec<String>,
}

/// Gathering Storm randomizes the Future-era technology and civic graphs once
/// per game. Both trees are global to the match, never per-player.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FutureTreeLayout {
    pub techs: BTreeMap<String, TreeLayoutEntry>,
    pub civics: BTreeMap<String, TreeLayoutEntry>,
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
    pub civic: Option<String>,
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
    pub terrain: Vec<String>,
    /// Every shipped terrain row for this civilization is a Hills variant, so
    /// the bias is really "hills", not the base terrain underneath it.
    #[serde(default)]
    pub terrain_hills: bool,
    #[serde(default)]
    pub terrain_tier: i32,
    #[serde(default)]
    pub feature: Vec<String>,
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
        if tier <= 0 { 0 } else { (6 - tier).max(1) }
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
    /// `building_production_pct`, `district_production_pct` and
    /// `wonder_production_pct` speed what a city is building;
    /// `combat_strength` and `unit_xp_pct` belong to its units. An ability
    /// that does something the engine cannot express this way — Rome's free
    /// monument, Scythia's healing — stays keyed by name in `has_ability`
    /// instead.
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

#[derive(Clone, Serialize, Deserialize)]
pub struct PolicySpec {
    pub slot: String, // military | economic | diplomatic | wildcard
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub replaces: Option<String>, // unlocking this obsoletes the named card
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
    pub requires: Vec<String>,
    #[serde(default)]
    pub effects: BTreeMap<String, f64>,
    #[serde(default)]
    pub note: String,
}

/// A reusable bundle of numeric engine effects.
///
/// Civ VI's `ATTACH_MODIFIER` effect composes named modifiers rather than
/// copying every argument onto every owning object. A rules object opts into
/// the same model with a `modifiers: ["name"]` field. The loader resolves the
/// graph once, adds the resulting values to that object's ordinary `effects`
/// map, and rejects dangling references or cycles before a game can start.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
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
            return Err(format!("modifier {name} has invalid building selector {building:?}"));
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
            return Err(format!("modifier {name} has invalid unit selector {unit:?}"));
        }
        *effects
            .entry(unit_purchase_discount_effect_key(unit))
            .or_insert(0.0) += value;
    }
    for ability in &spec.abilities {
        if ability.is_empty() || ability.contains(':') {
            return Err(format!("modifier {name} has invalid ability {ability:?}"));
        }
        *effects.entry(grant_ability_effect_key(ability)).or_insert(0.0) += 1.0;
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
    /// Flat Combat Strength added to every human unit.
    pub human_combat_strength: f64,
    /// Percentage added to human experience awards.
    pub human_xp_pct: f64,
    /// Extra Gold a human receives for clearing a Barbarian camp.
    pub human_camp_gold: f64,
    /// Scales the size of barbarian raiding parties.
    #[serde(default = "done")]
    pub barb_force_scale: f64,
    /// Scales how long a camp waits between spawns.
    /// `BarbarianAttackForces.SpawnRate` is 2 for every band up to Emperor and
    /// 1 from Immortal, so the top band assembles its forces twice as often.
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

fn dhundred() -> f64 {
    100.0
}

fn dstandard_turns() -> u32 {
    500
}

#[derive(Clone)]
pub struct Rules {
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
    /// The shipped WMDs table. Blast radius, fallout and ICBM range await a
    /// delivery mechanic; the per-turn Gold maintenance is charged today.
    pub wmds: SpecMap<WmdSpec>,
    /// Gathering Storm's random-disaster classes and their tuning.
    pub disasters: SpecMap<DisasterSpec>,
    /// Rise & Fall's Dedications, both halves.
    pub dedications: SpecMap<DedicationSpec>,
    /// The city-state seats this ruleset can hand out, in seating order.
    pub city_states: CityStateRoster,
    /// Which technologies grant each global effect, and which civics do.
    ///
    /// Asking what a player's trees add up to used to walk every node they
    /// had researched and ask each one whether it granted the effect — over a
    /// hundred lookups to answer a question whose answer usually comes from
    /// one or two nodes. The tables are inverted once, when the ruleset is
    /// built, and each list is in node order so the sum is added up in exactly
    /// the order it always was.
    pub tech_effects: SpecMap<Vec<(String, f64)>>,
    pub civic_effects: SpecMap<Vec<(String, f64)>>,
    /// Every node a given node depends on, however far back.
    ///
    /// Asking whether one technology leads to another used to walk the
    /// prerequisite graph from the target every time, re-exploring nodes that
    /// several paths reach. The closure is taken once instead.
    pub tech_ancestors: SpecMap<BTreeSet<String>>,
    pub civic_ancestors: SpecMap<BTreeSet<String>>,
    /// Which effect keys each family of specs declares at all.
    pub effect_index: EffectIndex,
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

fn shuffle_strings(values: &mut [String], rng: &mut Rng) {
    for index in (1..values.len()).rev() {
        let other = rng.below(index + 1);
        values.swap(index, other);
    }
}

/// Join two adjacent randomized columns with the smallest bipartite graph
/// that gives every node on both sides an edge. This is the shape the shipped
/// Future trees expose: no branch is orphaned and no breakthrough is free.
fn connect_random_layers(
    parents: &[String],
    children: &[String],
    rng: &mut Rng,
    requirements: &mut BTreeMap<String, Vec<String>>,
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
            prerequisites.push(parent.clone());
        }
    }
    Ok(())
}

fn random_tree_layout(
    kind: &str,
    tree: &SpecMap<TechSpec>,
    rng: &mut Rng,
) -> Result<BTreeMap<String, TreeLayoutEntry>, String> {
    let randomized: Vec<String> = tree
        .iter()
        .filter(|(_, spec)| spec.random_prereqs)
        .map(|(name, _)| name.clone())
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
            terminals.push(name.clone());
        } else if spec.random_costs.is_empty() {
            gateways.push(name.clone());
        } else if spec.random_costs.len() == 2 {
            regular.push(name.clone());
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
    let fixed_parents: BTreeSet<String> = tree
        .iter()
        .filter(|(_, spec)| !spec.random_prereqs)
        .flat_map(|(_, spec)| spec.requires.iter().cloned())
        .collect();
    let previous_leaves: Vec<String> = tree
        .iter()
        .filter(|(name, spec)| spec.era == previous_era && !fixed_parents.contains(*name))
        .map(|(name, _)| name.clone())
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
    let mut requirements: BTreeMap<String, Vec<String>> = randomized
        .iter()
        .map(|name| (name.clone(), Vec::new()))
        .collect();
    connect_random_layers(&previous_leaves, first, rng, &mut requirements)?;
    connect_random_layers(first, second, rng, &mut requirements)?;

    let terminal = terminals.pop().unwrap();
    if let Some(gateway) = gateways.pop() {
        requirements.insert(gateway.clone(), second.to_vec());
        requirements.insert(terminal.clone(), vec![gateway]);
    } else {
        requirements.insert(terminal.clone(), second.to_vec());
    }

    let first: BTreeSet<&str> = first.iter().map(String::as_str).collect();
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
    layout: &BTreeMap<String, TreeLayoutEntry>,
) -> Result<(), String> {
    let expected: BTreeSet<String> = tree
        .iter()
        .filter(|(_, spec)| spec.random_prereqs)
        .map(|(name, _)| name.clone())
        .collect();
    let actual: BTreeSet<String> = layout.keys().cloned().collect();
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
    let fixed_parents: BTreeSet<&str> = tree
        .iter()
        .filter(|(_, spec)| !spec.random_prereqs)
        .flat_map(|(_, spec)| spec.requires.iter().map(String::as_str))
        .collect();
    let previous_leaves: BTreeSet<String> = tree
        .iter()
        .filter(|(name, spec)| {
            spec.era == previous_era && !fixed_parents.contains(name.as_str())
        })
        .map(|(name, _)| name.clone())
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
            terminals.push(name.clone());
        } else if spec.random_costs.is_empty() {
            if entry.cost != spec.cost {
                return Err(format!(
                    "saved randomized {kind} gateway {name:?} has invalid cost {}",
                    entry.cost
                ));
            }
            gateways.push(name.clone());
        } else if spec.random_costs.len() != 2 {
            return Err(format!(
                "randomized {kind} {name:?} does not define exactly two column costs"
            ));
        } else if entry.cost == spec.random_costs[0] {
            first.insert(name.clone());
        } else if entry.cost == spec.random_costs[1] {
            second.insert(name.clone());
        } else {
            return Err(format!(
                "saved randomized {kind} {name:?} has invalid cost {}",
                entry.cost
            ));
        }
        let unique: BTreeSet<&str> = entry.requires.iter().map(String::as_str).collect();
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

    let validate_layer = |children: &BTreeSet<String>,
                          parents: &BTreeSet<String>,
                          label: &str|
     -> Result<(), String> {
        let mut used = BTreeSet::new();
        for child in children {
            let requires: BTreeSet<String> = layout[child].requires.iter().cloned().collect();
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
    let terminal_requires: BTreeSet<String> = layout[&terminal].requires.iter().cloned().collect();
    if let Some(gateway) = gateways.pop() {
        let gateway_requires: BTreeSet<String> =
            layout[&gateway].requires.iter().cloned().collect();
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
            .get_mut(name)
            .expect("the randomized tree entry was just checked");
        spec.cost = entry.cost;
        spec.requires = entry.requires.clone();
    }
    Ok(())
}

/// The transitive prerequisites of every node in a tree.
fn ancestry(nodes: &SpecMap<TechSpec>) -> SpecMap<BTreeSet<String>> {
    let mut ancestry: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, spec) in nodes.iter() {
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut pending: Vec<&str> = spec.requires.iter().map(String::as_str).collect();
        while let Some(node) = pending.pop() {
            // A node several paths reach is only followed once, which is also
            // what keeps a malformed cyclic tree from spinning here.
            if !reached.insert(node.to_string()) {
                continue;
            }
            if let Some(parent) = nodes.get(node) {
                pending.extend(parent.requires.iter().map(String::as_str));
            }
        }
        ancestry.insert(name.clone(), reached);
    }
    SpecMap::from(ancestry)
}

/// Invert a tree's per-node effect tables into per-effect node lists.
fn effect_sources(nodes: &SpecMap<TechSpec>) -> SpecMap<Vec<(String, f64)>> {
    let mut sources: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
    for (name, spec) in nodes.iter() {
        for (effect, value) in &spec.effects {
            sources
                .entry(effect.clone())
                .or_default()
                .push((name.clone(), *value));
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
    /// Chance an affected tile gains permanent fertility.
    #[serde(default)]
    pub fertility_chance: Vec<f64>,
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
    pub terrains: Vec<String>,
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
}

/// The city-state roster in seating order.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CityStateRoster {
    pub roster: Vec<CityStateSpec>,
}

/// Every ruleset file the engine ships, by the name a mod overlay uses.
pub const DATA_FILES: [(&str, &str); 29] = [
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
    ("city_states", include_str!("../data/city_states.json")),
];

fn add_effects(target: &mut BTreeMap<String, f64>, source: &BTreeMap<String, f64>) {
    for (effect, value) in source {
        *target.entry(effect.clone()).or_insert(0.0) += value;
    }
}

/// Resolve the modifier graph into bundles that contain no further links.
fn resolve_modifiers(
    source: &SpecMap<ModifierSpec>,
) -> Result<SpecMap<ModifierSpec>, String> {
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
            let owner = stack.last().map(String::as_str).unwrap_or("ruleset");
            return Err(format!("modifier {owner} attaches missing modifier {name}"));
        };
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
            add_effects(&mut effects, &nested.effects);
        }
        stack.pop();

        let flat = ModifierSpec {
            effects,
            building_yields: BTreeMap::new(),
            unit_purchase_discount_pct: BTreeMap::new(),
            abilities: BTreeSet::new(),
            modifiers: Vec::new(),
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

impl Rules {
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

    /// Build the immutable rules snapshot for one match. Gathering Storm's
    /// Future trees are the only rules rows that vary by game seed; cloning
    /// once here keeps that variation out of every hot-path query, while
    /// tactical/search clones continue to share the resulting [`Arc`].
    pub fn for_game(seed: u64, saved: Option<&FutureTreeLayout>) -> Arc<Rules> {
        let shared = Self::shared();
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
                        name.clone(),
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
            city_states: take(&mut files, "city_states")?,
            tech_effects: SpecMap::default(),
            civic_effects: SpecMap::default(),
            tech_ancestors: SpecMap::default(),
            civic_ancestors: SpecMap::default(),
            effect_index: EffectIndex::default(),
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
        rules.index_tree_unlocks();
        rules.tech_effects = effect_sources(&rules.techs);
        rules.civic_effects = effect_sources(&rules.civics);
        rules.tech_ancestors = ancestry(&rules.techs);
        rules.civic_ancestors = ancestry(&rules.civics);
        rules.effect_index = rules.build_effect_index();
        Ok(rules)
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
                any.insert(key.clone(), ());
            }
        }
        for key in self.tech_effects.keys().chain(self.civic_effects.keys()) {
            any.insert(key.clone(), ());
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
        let mut add = |kind: &str, id: &str, tech: &Option<String>, civic: &Option<String>| {
            if let Some(node) = tech {
                indexed.push((
                    true,
                    node.clone(),
                    TreeUnlock {
                        kind: kind.to_string(),
                        id: id.to_string(),
                    },
                ));
            }
            if let Some(node) = civic {
                indexed.push((
                    false,
                    node.clone(),
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

    pub fn is_water(&self, t: &Tile) -> bool {
        self.terrains[t.terrain].water
    }

    pub fn is_passable(&self, t: &Tile) -> bool {
        if let Some(f) = &t.feature {
            if self.features[f].impassable {
                return false;
            }
        }
        self.terrains[t.terrain].passable
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
        if t.road > 0 && !terrain.water {
            c = 1.0; // every route flattens terrain to at most 1 MP
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    const TECHS: &str = "
        pottery animal_husbandry mining sailing astrology irrigation archery writing masonry
        bronze_working wheel horseback_riding currency celestial_navigation iron_working
        shipbuilding mathematics construction engineering apprenticeship buttress machinery
        military_tactics stirrups castles education military_engineering banking cartography
        gunpowder mass_production printing astronomy metal_casting siege_tactics square_rigging
        ballistics industrialization military_science scientific_theory economics rifling
        sanitation steam_power flight refining replaceable_parts steel chemistry combustion
        electricity radio advanced_ballistics advanced_flight combined_arms plastics rocketry
        computers nuclear_fission synthetic_materials composites guidance_systems lasers
        satellites stealth_technology telecommunications nanotechnology nuclear_fusion robotics
        seasteads advanced_ai advanced_power_cells cybernetics smart_materials predictive_systems
        offworld_mission future_tech";

    const CIVICS: &str = "
        code_of_laws craftsmanship foreign_trade military_tradition mysticism early_empire
        state_workforce drama_poetry games_recreation political_philosophy military_training
        theology defensive_tactics recorded_history naval_tradition civil_service feudalism
        divine_right mercenaries guilds medieval_faires exploration reformed_church
        diplomatic_service humanism mercantilism the_enlightenment colonialism opera_ballet
        civil_engineering nationalism natural_history scorched_earth urbanization conservation
        mass_media mobilization capitalism class_struggle ideology suffrage totalitarianism
        nuclear_program cultural_heritage cold_war professional_sports rapid_deployment
        space_race environmentalism globalization social_media digital_democracy
        synthetic_technocracy corporate_libertarianism near_future_governance
        information_warfare global_warming_mitigation cultural_hegemony smart_power_doctrine
        exodus_imperative future_civic";

    const DISTRICTS: &str = "
        city_center campus holy_site commercial_hub harbor encampment theater_square
        industrial_zone entertainment_complex water_park aqueduct neighborhood canal dam
        aerodrome spaceport government_plaza diplomatic_quarter preserve observatory seowon
        acropolis lavra ikanda thanh suguba cothon royal_navy_dockyard hansa oppidum
        street_carnival hippodrome copacabana bath mbanza";

    const BUILDINGS: &str = "
        airport alchemical_society amphitheater ancestral_hall aquarium aquatics_center
        archaeological_museum arena armory art_museum audience_chamber bank barracks
        basilikoi_paides broadcast_center cathedral chancery coal_power_plant consulate
        dar_e_mehr electronics_factory factory ferris_wheel film_studio flood_barrier
        food_market foreign_ministry gilded_vault granary grand_bazaar grand_masters_chapel
        grove gurdwara hangar hydroelectric_dam intelligence_agency library lighthouse madrasa
        marae market medieval_walls meeting_house military_academy monument mosque
        national_history_museum navigation_school nuclear_power_plant oil_power_plant
        old_god_obelisk ordu pagoda palace palgum prasat queens_bibliotheque renaissance_walls
        research_lab royal_society sanctuary seaport sewer shipyard shopping_mall shrine stable
        stadium stave_church stock_exchange stupa sukiennice synagogue temple thermal_bath
        tlachtli tsikhe university walls war_department warlords_throne wat water_mill workshop zoo";

    const WONDERS: &str = "
        alhambra amundsen_scott_research_station angkor_wat apadana big_ben biosphere
        bolshoi_theatre broadway casa_de_contratacion chichen_itza colosseum colossus
        cristo_redentor eiffel_tower estadio_do_maracana etemenanki forbidden_city
        golden_gate_bridge great_bath great_library great_lighthouse great_zimbabwe hagia_sophia
        hanging_gardens hermitage huey_teocalli jebel_barkal kilwa_kisiwani kotoku_in
        machu_picchu mahabodhi_temple mausoleum_at_halicarnassus meenakshi_temple mont_st_michel
        oracle orszaghaz oxford_university panama_canal petra potala_palace pyramids ruhr_valley
        st_basils_cathedral statue_of_liberty statue_of_zeus stonehenge sydney_opera_house
        taj_mahal temple_artemis terracotta_army torre_de_belem university_of_sankore
        venetian_arsenal";

    #[test]
    fn named_modifiers_compose_and_attach_to_any_effect_bearing_spec() {
        let mut files = Rules::shipped_values();
        files.insert(
            "modifiers".to_string(),
            json!({
                "production_seed": {
                    "effects": {"city_production": 2, "builder_production_pct": 12},
                    "building_yields": {"library": {"science": 2}},
                    "unit_purchase_discount_pct": {"builder": 15},
                    "abilities": ["public_engineering"]
                },
                "production_bundle": {
                    "effects": {"builder_production_pct": 8},
                    "building_yields": {"library": {"science": 1}},
                    "unit_purchase_discount_pct": {"builder": 5},
                    "modifiers": ["production_seed"]
                }
            }),
        );
        files.get_mut("policies").unwrap()["urban_planning"]["modifiers"] =
            json!(["production_bundle"]);

        let rules = Rules::from_values(files).unwrap();
        // Urban Planning already carries one city Production. Attached values
        // add to local values rather than silently replacing them.
        assert_eq!(rules.policies["urban_planning"].effects["city_production"], 3.0);
        assert_eq!(
            rules.policies["urban_planning"].effects["builder_production_pct"],
            20.0
        );
        assert!(rules.modifiers["production_bundle"].modifiers.is_empty());
        assert_eq!(
            rules.modifiers["production_bundle"].effects["builder_production_pct"],
            20.0
        );
        assert_eq!(
            rules.modifiers["production_bundle"].effects
                [&building_yield_effect_key("library", "science")],
            3.0
        );
        assert_eq!(
            rules.modifiers["production_bundle"].effects
                [&unit_purchase_discount_effect_key("builder")],
            20.0
        );
        assert_eq!(
            rules.modifiers["production_bundle"].effects
                [&grant_ability_effect_key("public_engineering")],
            1.0
        );
        assert_eq!(
            rules.policies["urban_planning"].effects
                [&building_yield_effect_key("library", "science")],
            3.0
        );
        assert!(rules.modifiers["production_bundle"].building_yields.is_empty());
        assert!(rules.modifiers["production_bundle"]
            .unit_purchase_discount_pct
            .is_empty());
        assert!(rules.modifiers["production_bundle"].abilities.is_empty());
    }

    #[test]
    fn modifier_graph_rejects_dangling_references_and_cycles() {
        let mut dangling = Rules::shipped_values();
        dangling.insert(
            "modifiers".to_string(),
            json!({"outer": {"modifiers": ["missing"]}}),
        );
        let error = Rules::from_values(dangling).err().unwrap();
        assert!(error.contains("outer attaches missing modifier missing"), "{error}");

        let mut cycle = Rules::shipped_values();
        cycle.insert(
            "modifiers".to_string(),
            json!({
                "first": {"modifiers": ["second"]},
                "second": {"modifiers": ["first"]}
            }),
        );
        let error = Rules::from_values(cycle).err().unwrap();
        assert!(
            error.contains("modifier attachment cycle: first -> second -> first"),
            "{error}"
        );
    }

    #[test]
    fn rules_objects_cannot_attach_an_unknown_modifier() {
        let mut files = Rules::shipped_values();
        files.get_mut("policies").unwrap()["urban_planning"]["modifiers"] =
            json!(["missing"]);
        let error = Rules::from_values(files).err().unwrap();
        assert!(
            error.contains("policies.json.urban_planning attaches missing modifier missing"),
            "{error}"
        );
    }

    fn assert_complete_tree(
        tree: &SpecMap<TechSpec>,
        expected: &str,
        era_counts: [usize; 9],
    ) {
        let actual: BTreeSet<&str> = tree.keys().map(String::as_str).collect();
        let expected: BTreeSet<&str> = expected.split_whitespace().collect();
        assert_eq!(actual, expected);

        let mut counts = [0; 9];
        for (name, spec) in tree {
            assert!(spec.cost > 0.0, "{name} has no research cost");
            assert!(
                spec.era < ERA_NAMES.len(),
                "{name} has invalid era {}",
                spec.era
            );
            counts[spec.era] += 1;
            for prerequisite in &spec.requires {
                let parent = tree
                    .get(prerequisite)
                    .unwrap_or_else(|| panic!("{name} requires missing node {prerequisite}"));
                assert!(
                    parent.era <= spec.era,
                    "{name} requires later-era node {prerequisite}"
                );
            }
        }
        assert_eq!(counts, era_counts);

        // Repeatedly remove nodes whose prerequisites have been removed. If
        // anything remains, the graph contains a cycle or an unreachable root.
        let mut reached = BTreeSet::new();
        while reached.len() < tree.len() {
            let before = reached.len();
            for (name, spec) in tree {
                if spec.requires.iter().all(|node| reached.contains(node)) {
                    reached.insert(name.clone());
                }
            }
            assert!(reached.len() > before, "tree contains a dependency cycle");
        }
    }

    #[test]
    fn gathering_storm_technology_and_civics_trees_are_complete() {
        let rules = Rules::embedded();
        assert_complete_tree(&rules.techs, TECHS, [11, 8, 8, 9, 8, 8, 8, 9, 8]);
        assert_complete_tree(&rules.civics, CIVICS, [7, 7, 7, 6, 7, 9, 5, 7, 6]);
    }

    fn assert_randomized_tree_shape(
        tree: &SpecMap<TechSpec>,
        ancestors: &SpecMap<BTreeSet<String>>,
        previous: &[&str],
        gateway: Option<&str>,
        terminal: &str,
    ) {
        let regular: Vec<&str> = tree
            .iter()
            .filter(|(_, spec)| spec.random_costs.len() == 2)
            .map(|(name, _)| name.as_str())
            .collect();
        let first: BTreeSet<&str> = regular
            .iter()
            .copied()
            .filter(|name| tree[*name].cost == tree[*name].random_costs[0])
            .collect();
        let second: BTreeSet<&str> = regular
            .iter()
            .copied()
            .filter(|name| tree[*name].cost == tree[*name].random_costs[1])
            .collect();
        assert_eq!(first.len() + second.len(), regular.len());
        assert!(first.len() >= 2 && first.len() < regular.len());
        assert!(!second.is_empty());

        let previous: BTreeSet<&str> = previous.iter().copied().collect();
        let first_parents: BTreeSet<&str> = first
            .iter()
            .flat_map(|name| tree[*name].requires.iter().map(String::as_str))
            .collect();
        assert_eq!(first_parents, previous);
        for name in &first {
            assert!(!tree[*name].requires.is_empty());
            assert!(tree[*name]
                .requires
                .iter()
                .all(|parent| previous.contains(parent.as_str())));
        }

        let second_parents: BTreeSet<&str> = second
            .iter()
            .flat_map(|name| tree[*name].requires.iter().map(String::as_str))
            .collect();
        assert_eq!(second_parents, first);
        for name in &second {
            assert!(!tree[*name].requires.is_empty());
            assert!(tree[*name]
                .requires
                .iter()
                .all(|parent| first.contains(parent.as_str())));
        }

        let second_owned: BTreeSet<String> = second.iter().map(|name| (*name).to_string()).collect();
        match gateway {
            Some(gateway) => {
                assert_eq!(
                    tree[gateway].requires.iter().cloned().collect::<BTreeSet<_>>(),
                    second_owned
                );
                assert_eq!(tree[terminal].requires, [gateway.to_string()]);
            }
            None => assert_eq!(
                tree[terminal]
                    .requires
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                second_owned
            ),
        }
        for node in previous
            .iter()
            .copied()
            .chain(regular.iter().copied())
            .chain(gateway)
        {
            assert!(
                ancestors[terminal].contains(node),
                "{terminal} does not descend from {node}"
            );
        }
    }

    #[test]
    fn future_research_trees_use_the_shipped_two_column_shape() {
        let base = Rules::embedded();
        assert!(base
            .techs
            .iter()
            .filter(|(_, spec)| spec.random_prereqs)
            .all(|(_, spec)| spec.requires.is_empty()));
        assert!(base
            .civics
            .iter()
            .filter(|(_, spec)| spec.random_prereqs)
            .all(|(_, spec)| spec.requires.is_empty()));

        let mut layouts = BTreeSet::new();
        for seed in 0..64 {
            let rules = Rules::for_game(seed, None);
            assert_randomized_tree_shape(
                &rules.techs,
                &rules.tech_ancestors,
                &[
                    "nanotechnology",
                    "nuclear_fusion",
                    "robotics",
                    "stealth_technology",
                    "telecommunications",
                ],
                Some("offworld_mission"),
                "future_tech",
            );
            assert_randomized_tree_shape(
                &rules.civics,
                &rules.civic_ancestors,
                &[
                    "corporate_libertarianism",
                    "digital_democracy",
                    "near_future_governance",
                    "synthetic_technocracy",
                ],
                None,
                "future_civic",
            );
            let layout = rules.future_tree_layout();
            let restored = Rules::for_game(seed, Some(&layout));
            assert_eq!(restored.future_tree_layout(), layout);
            layouts.insert(format!("{layout:?}"));
        }
        assert!(
            layouts.len() > 32,
            "64 seeds produced only {} Future-tree layouts",
            layouts.len()
        );
    }

    #[test]
    fn malformed_saved_future_tree_is_rejected_before_installation() {
        let mut rules = Rules::embedded();
        let mut layout = FutureTreeLayout::generate(&rules, 919_191).unwrap();
        let first = layout
            .techs
            .iter()
            .find(|(name, entry)| {
                !rules.techs[*name].random_costs.is_empty()
                    && entry.cost == rules.techs[*name].random_costs[0]
            })
            .map(|(name, _)| name.clone())
            .unwrap();
        layout.techs.get_mut(&first).unwrap().requires = vec!["future_tech".to_string()];
        let error = apply_tree_layout("technology", &mut rules.techs, &layout.techs).unwrap_err();
        assert!(error.contains("outside the preceding column"), "{error}");

        let mut layout = FutureTreeLayout::generate(&rules, 919_191).unwrap();
        let prerequisite = layout.techs[&first].requires[0].clone();
        layout
            .techs
            .get_mut(&first)
            .unwrap()
            .requires
            .push(prerequisite);
        let error = apply_tree_layout("technology", &mut rules.techs, &layout.techs).unwrap_err();
        assert!(error.contains("repeats a prerequisite"), "{error}");
    }

    #[test]
    fn gathering_storm_unit_upgrade_graph_is_complete_and_acyclic() {
        let rules = Rules::embedded();
        let expected: BTreeSet<(&str, &str)> = [
            ("scout", "skirmisher"),
            ("skirmisher", "ranger"),
            ("ranger", "spec_ops"),
            ("warrior", "swordsman"),
            ("swordsman", "man_at_arms"),
            ("man_at_arms", "musketman"),
            ("musketman", "line_infantry"),
            ("line_infantry", "infantry"),
            ("infantry", "mechanized_infantry"),
            ("slinger", "archer"),
            ("archer", "crossbowman"),
            ("crossbowman", "field_cannon"),
            ("field_cannon", "machine_gun"),
            ("spearman", "pikeman"),
            ("pikeman", "pike_and_shot"),
            ("pike_and_shot", "at_crew"),
            ("at_crew", "modern_at"),
            ("horseman", "courser"),
            ("courser", "cavalry"),
            ("cavalry", "helicopter"),
            ("heavy_chariot", "knight"),
            ("knight", "cuirassier"),
            ("tagma", "cuirassier"),
            ("cuirassier", "tank"),
            ("tank", "modern_armor"),
            ("catapult", "trebuchet"),
            ("trebuchet", "bombard"),
            ("bombard", "artillery"),
            ("artillery", "rocket_artillery"),
            ("battering_ram", "siege_tower"),
            ("siege_tower", "medic"),
            ("medic", "supply_convoy"),
            ("observation_balloon", "drone"),
            ("anti_air_gun", "mobile_sam"),
            ("galley", "caravel"),
            ("caravel", "ironclad"),
            ("ironclad", "destroyer"),
            ("quadrireme", "frigate"),
            ("frigate", "battleship"),
            ("battleship", "missile_cruiser"),
            ("privateer", "submarine"),
            ("submarine", "nuclear_submarine"),
            ("biplane", "fighter"),
            ("fighter", "jet_fighter"),
            ("bomber", "jet_bomber"),
            ("legion", "man_at_arms"),
            ("hoplite", "pikeman"),
            ("eagle_warrior", "swordsman"),
            ("war_cart", "knight"),
            ("pitati_archer", "crossbowman"),
            ("maryannu_chariot_archer", "crossbowman"),
            ("saka_horse_archer", "crossbowman"),
            ("crouching_tiger", "field_cannon"),
        ]
        .into_iter()
        .collect();
        let actual: BTreeSet<(&str, &str)> = rules
            .units
            .iter()
            .filter_map(|(unit, spec)| {
                spec.upgrade_to
                    .as_deref()
                    .map(|target| (unit.as_str(), target))
            })
            .collect();
        assert_eq!(actual, expected);

        for (source, target) in &actual {
            let target_spec = rules
                .units
                .get(*target)
                .unwrap_or_else(|| panic!("{source} upgrades to missing unit {target}"));
            assert!(
                target_spec.buildable,
                "{source} upgrades to unbuildable {target}"
            );
            let mut seen = BTreeSet::new();
            let mut cursor = Some(*source);
            while let Some(unit) = cursor {
                assert!(seen.insert(unit), "unit upgrade cycle reaches {unit}");
                cursor = rules.units[unit].upgrade_to.as_deref();
            }
        }
    }

    #[test]
    fn every_tree_unlock_is_present_gated_and_runtime_indexed() {
        let rules = Rules::embedded();
        assert_eq!(rules.techs.len(), 77);
        assert_eq!(rules.civics.len(), 61);
        assert_eq!(rules.units.len(), 83);
        assert_eq!(rules.buildings.len(), 85);
        assert_eq!(rules.districts.len(), 35);
        assert_eq!(rules.wonders.len(), 53);
        assert_eq!(rules.improvements.len(), 36);
        assert_eq!(rules.resources.len(), 52);
        assert_eq!(rules.projects.len(), 25);
        // 118 civic-unlocked cards plus the seven Dark Age cards, which no
        // civic unlocks — a Dark Age is what puts them on offer.
        assert_eq!(rules.policies.len(), 125);
        assert_eq!(
            rules.policies.values().filter(|spec| spec.dark_age).count(),
            7
        );
        assert_eq!(rules.governments.len(), 13);

        let check_gate = |kind: &str, id: &str, tech: &Option<String>, civic: &Option<String>| {
            if let Some(node) = tech {
                let spec = rules
                    .techs
                    .get(node)
                    .unwrap_or_else(|| panic!("{kind} {id} references missing technology {node}"));
                assert!(
                    spec.unlocks
                        .iter()
                        .any(|unlock| unlock.kind == kind && unlock.id == id),
                    "technology {node} does not index {kind} {id}"
                );
            }
            if let Some(node) = civic {
                let spec = rules
                    .civics
                    .get(node)
                    .unwrap_or_else(|| panic!("{kind} {id} references missing civic {node}"));
                assert!(
                    spec.unlocks
                        .iter()
                        .any(|unlock| unlock.kind == kind && unlock.id == id),
                    "civic {node} does not index {kind} {id}"
                );
            }
        };

        for (id, spec) in &rules.units {
            check_gate("unit", id, &spec.tech, &spec.civic);
            assert!(
                spec.maintenance >= 0.0,
                "{id} has negative Gold maintenance"
            );
            if let Some(resource) = &spec.requires_resource {
                assert!(
                    rules.resources.contains_key(resource),
                    "{id} needs {resource}"
                );
                assert!(
                    spec.resource_cost > 0.0,
                    "{id} must define its Gathering Storm {resource} cost"
                );
                assert!(
                    spec.resource_maintenance >= 0.0,
                    "{id} has negative {resource} maintenance"
                );
            } else {
                assert_eq!(
                    spec.resource_cost, 0.0,
                    "{id} has a cost without a resource"
                );
                assert_eq!(
                    spec.resource_maintenance, 0.0,
                    "{id} has maintenance without a resource"
                );
            }
            if let Some(building) = &spec.requires_building {
                assert!(
                    rules.buildings.contains_key(building),
                    "{id} needs {building}"
                );
            }
            if let Some(district) = &spec.requires_district {
                assert!(
                    rules.districts.contains_key(district),
                    "{id} needs {district}"
                );
            }
            for improvement in &spec.builds {
                assert!(
                    rules.improvements.contains_key(improvement),
                    "{id} builds missing improvement {improvement}"
                );
            }
        }
        for (id, spec) in &rules.buildings {
            check_gate("building", id, &spec.tech, &spec.civic);
            assert!(
                spec.maintenance >= 0.0,
                "{id} has negative Gold maintenance"
            );
        }
        for (id, spec) in &rules.districts {
            check_gate("district", id, &spec.tech, &spec.civic);
            assert!(
                spec.maintenance >= 0.0,
                "{id} has negative Gold maintenance"
            );
        }
        for (id, spec) in &rules.wonders {
            check_gate("wonder", id, &spec.tech, &spec.civic);
        }
        for (id, spec) in &rules.improvements {
            check_gate("improvement", id, &spec.tech, &spec.civic);
            for resource in &spec.resources {
                assert!(
                    rules.resources.contains_key(resource),
                    "{id} references missing resource {resource}"
                );
            }
        }
        for (id, spec) in &rules.resources {
            check_gate("resource", id, &spec.tech, &spec.civic);
            if !spec.improvement.is_empty() {
                assert!(
                    rules.improvements.contains_key(&spec.improvement),
                    "{id} references missing improvement {}",
                    spec.improvement
                );
            }
        }
        for (id, spec) in &rules.projects {
            check_gate("project", id, &spec.tech, &spec.civic);
            if let Some(district) = &spec.district {
                assert!(
                    rules.districts.contains_key(district),
                    "{id} needs {district}"
                );
            }
            for prerequisite in &spec.requires {
                assert!(
                    rules.projects.contains_key(prerequisite),
                    "{id} requires missing project {prerequisite}"
                );
            }
            for building in &spec.requires_buildings {
                assert!(
                    rules.buildings.contains_key(building),
                    "{id} requires missing building {building}"
                );
            }
        }
        for (id, spec) in &rules.policies {
            check_gate("policy", id, &None, &spec.civic);
            assert!(
                matches!(
                    spec.slot.as_str(),
                    "military" | "economic" | "diplomatic" | "wildcard"
                ),
                "{id} has invalid slot {}",
                spec.slot
            );
            assert!(
                !spec.effects.is_empty(),
                "policy {id} has no runtime effect"
            );
            if let Some(replaced) = &spec.replaces {
                assert!(
                    rules.policies.contains_key(replaced),
                    "{id} replaces missing policy {replaced}"
                );
            }
        }
        for (id, spec) in &rules.governments {
            check_gate("government", id, &None, &spec.civic);
            let slots = spec.slots.military
                + spec.slots.economic
                + spec.slots.diplomatic
                + spec.slots.wildcard;
            assert!(slots > 0, "government {id} has no policy slots");
        }

        for (kind, tree) in [("technology", &rules.techs), ("civic", &rules.civics)] {
            for (node, spec) in tree {
                assert!(
                    !spec.unlocks.is_empty() || !spec.effects.is_empty(),
                    "{kind} {node} has neither a content unlock nor a runtime ability"
                );
                for unlock in &spec.unlocks {
                    let gate = match unlock.kind.as_str() {
                        "unit" => rules.units[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.units[&unlock.id].civic.as_ref()),
                        "building" => rules.buildings[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.buildings[&unlock.id].civic.as_ref()),
                        "district" => rules.districts[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.districts[&unlock.id].civic.as_ref()),
                        "wonder" => rules.wonders[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.wonders[&unlock.id].civic.as_ref()),
                        "improvement" => rules.improvements[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.improvements[&unlock.id].civic.as_ref()),
                        "resource" => rules.resources[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.resources[&unlock.id].civic.as_ref()),
                        "project" => rules.projects[&unlock.id]
                            .tech
                            .as_ref()
                            .or(rules.projects[&unlock.id].civic.as_ref()),
                        "policy" => rules.policies[&unlock.id].civic.as_ref(),
                        "government" => rules.governments[&unlock.id].civic.as_ref(),
                        other => panic!("{node} indexes unknown unlock kind {other}"),
                    };
                    assert_eq!(gate.map(String::as_str), Some(node.as_str()));
                }
            }
        }
    }

    #[test]
    fn gathering_storm_district_building_and_wonder_rosters_are_complete_and_linked() {
        let rules = Rules::embedded();
        fn expected(names: &str) -> BTreeSet<&str> {
            names.split_whitespace().collect()
        }
        assert_eq!(
            rules
                .districts
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected(DISTRICTS)
        );
        assert_eq!(
            rules
                .buildings
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected(BUILDINGS)
        );
        assert_eq!(
            rules
                .wonders
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected(WONDERS)
        );

        for (name, district) in &rules.districts {
            assert!(district.cost >= 0.0, "{name} has a negative cost");
            if let Some(tech) = &district.tech {
                assert!(
                    rules.techs.contains_key(tech),
                    "{name} has missing tech {tech}"
                );
            }
            if let Some(civic) = &district.civic {
                assert!(
                    rules.civics.contains_key(civic),
                    "{name} has missing civic {civic}"
                );
            }
            if let Some(base) = &district.replaces {
                assert!(
                    rules.districts.contains_key(base),
                    "{name} replaces missing {base}"
                );
                assert!(
                    district.unique_to.is_some(),
                    "{name} replacement is not unique"
                );
            }
            for excluded in &district.excludes {
                assert!(
                    rules.districts.contains_key(excluded),
                    "{name} excludes missing {excluded}"
                );
            }
        }

        for (name, building) in &rules.buildings {
            assert!(building.cost > 0.0, "{name} has no cost");
            if let Some(tech) = &building.tech {
                assert!(
                    rules.techs.contains_key(tech),
                    "{name} has missing tech {tech}"
                );
            }
            if let Some(civic) = &building.civic {
                assert!(
                    rules.civics.contains_key(civic),
                    "{name} has missing civic {civic}"
                );
            }
            if let Some(district) = &building.district {
                assert!(
                    rules.districts.contains_key(district),
                    "{name} has missing district {district}"
                );
            }
            for required in building.requires.iter().chain(&building.requires_any) {
                assert!(
                    rules.buildings.contains_key(required),
                    "{name} requires missing {required}"
                );
            }
            for excluded in &building.excludes {
                assert!(
                    rules.buildings.contains_key(excluded),
                    "{name} excludes missing {excluded}"
                );
            }
            if let Some(base) = &building.replaces {
                assert!(
                    rules.buildings.contains_key(base),
                    "{name} replaces missing {base}"
                );
            }
            assert!(
                !building.wonder,
                "{name} must be modeled as a map-placed wonder"
            );
        }

        for (name, wonder) in &rules.wonders {
            assert!(wonder.cost > 0.0, "{name} has no cost");
            if let Some(tech) = &wonder.tech {
                assert!(
                    rules.techs.contains_key(tech),
                    "{name} has missing tech {tech}"
                );
            }
            if let Some(civic) = &wonder.civic {
                assert!(
                    rules.civics.contains_key(civic),
                    "{name} has missing civic {civic}"
                );
            }
            if let Some(district) = &wonder.adjacent_district {
                assert!(
                    rules.districts.contains_key(district),
                    "{name} has missing adjacent district {district}"
                );
            }
            for required in wonder
                .requires_buildings
                .iter()
                .chain(&wonder.requires_any_buildings)
            {
                assert!(
                    rules.buildings.contains_key(required),
                    "{name} requires missing {required}"
                );
            }
            for terrain in &wonder.terrain {
                assert!(
                    rules.terrains.contains_key(terrain),
                    "{name} has missing terrain {terrain}"
                );
            }
            for feature in &wonder.feature {
                assert!(
                    rules.features.contains_key(feature),
                    "{name} has missing feature {feature}"
                );
            }
            if let Some(resource) = &wonder.adjacent_resource {
                assert!(
                    rules.resources.contains_key(resource),
                    "{name} has missing resource {resource}"
                );
            }
            if let Some(improvement) = &wonder.adjacent_improvement {
                assert!(
                    rules.improvements.contains_key(improvement),
                    "{name} has missing improvement {improvement}"
                );
            }
        }
    }

    #[test]
    fn modeled_unit_classes_have_complete_promotion_trees() {
        let rules = Rules::embedded();
        let classes: BTreeSet<_> = rules
            .units
            .values()
            // Spy promotions are resolved by the off-map espionage engine,
            // not the seven-node map-unit XP trees validated here.
            .filter(|unit| !unit.promotion_class.is_empty() && unit.promotion_class != "espionage")
            .map(|unit| unit.promotion_class.as_str())
            .collect();
        let promotion_count = |class: &str| match class {
            "religious_apostle" => 9,
            "rock_band" => 12,
            _ => 7,
        };
        let expected_promotions = classes
            .iter()
            .map(|class| promotion_count(class))
            .sum::<usize>();
        assert_eq!(
            rules.promotions.len(),
            expected_promotions,
            "modeled promotion classes: {classes:?}"
        );
        for class in classes {
            let nodes: Vec<_> = rules
                .promotions
                .iter()
                .filter(|(_, promotion)| promotion.class == class)
                .collect();
            let expected = promotion_count(class);
            assert_eq!(nodes.len(), expected, "{class} promotion tree");
            for (name, promotion) in nodes {
                assert!((1..=4).contains(&promotion.tier), "{name} tier");
                for prerequisite in &promotion.requires {
                    let required = rules
                        .promotions
                        .get(prerequisite)
                        .unwrap_or_else(|| panic!("{name} requires missing {prerequisite}"));
                    assert_eq!(required.class, class, "{name} crosses unit classes");
                    assert!(required.tier <= promotion.tier, "{name} prerequisite tier");
                }
                assert!(
                    promotion.requires.is_empty()
                        || promotion.requires.iter().any(|prerequisite| {
                            rules.promotions[prerequisite].tier < promotion.tier
                        }),
                    "{name} has no prerequisite from an earlier tier"
                );
            }
        }
    }

    /// Civ VI splits its Natural Wonders cleanly: a wonder a Citizen can stand
    /// on pays tile yields, and one that is Impassable cannot be worked at all,
    /// so it pays its neighbours instead — or, for Ik-Kil and Zhangye Danxia,
    /// nothing but Appeal. Six wonders used to be passable *and* pay tile
    /// yields nobody in the shipped game ever collects.
    ///
    /// The Bermuda Triangle is the shipped exception on the other side, and it
    /// is deliberate rather than sloppy: its plots are ordinary Ocean a Citizen
    /// *can* work, it pays nothing for standing there, and its whole yield is
    /// the +5 Science it gives every neighbour. `Feature_UnitMovements` bars
    /// passage through it while allowing a unit to end on it, which is a third
    /// state neither half of the split describes.
    #[test]
    fn an_impassable_natural_wonder_pays_its_neighbours_not_its_own_tile() {
        let rules = Rules::embedded();
        let wonders = rules
            .features
            .iter()
            .filter(|(_, spec)| spec.natural_wonder);
        let mut passable = 0;
        let mut blocked = 0;
        for (name, spec) in wonders {
            if spec.impassable {
                assert_eq!(
                    spec.yields,
                    Yields::default(),
                    "{name} is impassable, so no Citizen can ever work its tile"
                );
                blocked += 1;
            } else if name == "bermuda_triangle" {
                assert_eq!(spec.yields, Yields::default(), "{name} pays no tile yield");
                assert_ne!(
                    spec.adjacent_yields,
                    Yields::default(),
                    "{name} exists to pay its neighbours"
                );
                passable += 1;
            } else {
                assert_ne!(
                    spec.yields,
                    Yields::default(),
                    "{name} is workable and must be worth working"
                );
                assert_eq!(
                    spec.adjacent_yields,
                    Yields::default(),
                    "{name} is workable, so the shipped data pays its own tile"
                );
                passable += 1;
            }
        }
        assert!(
            passable > 0 && blocked > 0,
            "both halves of the split must be represented"
        );
    }

    /// The collection paths skip a whole sweep when the index says no spec in
    /// the family grants an effect, so an index that misses a key would make
    /// a real modifier silently stop applying. Every declared key must be
    /// present in its own family and in the union.
    #[test]
    fn the_effect_index_covers_every_key_any_spec_declares() {
        let rules = Rules::shipped();
        let index = &rules.effect_index;
        let mut checked = 0usize;
        let mut check = |family: &str, present: bool, in_any: bool, key: &str| {
            assert!(present, "{family} declares {key}, which its index omits");
            assert!(in_any, "{key} is declared by {family} but missing from the union");
        };
        for spec in rules.policies.values() {
            for key in spec.effects.keys() {
                check("policies", index.policies(key), index.any(key), key);
                checked += 1;
            }
        }
        for spec in rules.civs.values() {
            for key in spec.effects.keys() {
                check("civs", index.civs(key), index.any(key), key);
                checked += 1;
            }
        }
        for spec in rules.buildings.values() {
            for key in spec.effects.keys() {
                check("buildings", index.buildings(key), index.any(key), key);
                checked += 1;
            }
        }
        for spec in rules.districts.values() {
            for key in spec.effects.keys() {
                check("districts", index.districts(key), index.any(key), key);
                checked += 1;
            }
        }
        for spec in rules.wonders.values() {
            for key in spec.effects.keys() {
                check("wonders", index.wonders(key), index.any(key), key);
                checked += 1;
            }
        }
        // `modifiers` is deliberately absent: it is swapped in at runtime, so
        // the collection paths fall through on the seat's own attachment list
        // rather than trusting an index of it.
        for table in [
            &rules.beliefs.pantheon,
            &rules.beliefs.founder,
            &rules.beliefs.follower,
            &rules.beliefs.enhancer,
            &rules.beliefs.worship,
        ] {
            for spec in table.values() {
                for key in spec.effects.keys() {
                    check("beliefs", index.beliefs(key), index.any(key), key);
                    checked += 1;
                }
            }
        }
        for spec in rules.governors.values() {
            for key in spec
                .effects
                .keys()
                .chain(spec.promotions.values().flat_map(|p| p.effects.keys()))
            {
                check("governors", index.governors(key), index.any(key), key);
                checked += 1;
            }
        }
        for key in rules.tech_effects.keys().chain(rules.civic_effects.keys()) {
            assert!(index.any(key), "tree effect {key} is missing from the union");
            checked += 1;
        }
        assert!(checked > 500, "expected the shipped ruleset to declare many effects, saw {checked}");
    }

    /// The three namespaced families are ruled out on the selector alone, so
    /// every selector named by a declared key has to be indexed.
    #[test]
    fn the_effect_index_covers_every_namespaced_selector() {
        let rules = Rules::shipped();
        let index = &rules.effect_index;
        for key in index.any.keys() {
            if let Some(rest) = key.strip_prefix(BUILDING_YIELD_EFFECT_PREFIX) {
                let (selector, _) = rest.split_once(':').expect("a building yield key names a yield");
                assert!(
                    index.modifies_building_yields(selector),
                    "{key} names building selector {selector}, which the index omits"
                );
            } else if let Some(unit) = key.strip_prefix(UNIT_PURCHASE_EFFECT_PREFIX) {
                assert!(
                    index.discounts_unit_purchase(unit),
                    "{key} names unit {unit}, which the index omits"
                );
            } else if let Some(ability) = key.strip_prefix(GRANT_ABILITY_EFFECT_PREFIX) {
                assert!(
                    index.grants_ability(ability),
                    "{key} names ability {ability}, which the index omits"
                );
            }
        }
    }
}
