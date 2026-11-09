//! The snapshot schema: what the Civilization VI control mod exports, as
//! the types `rebuild_from_state` reads.
//!
//! `StateSnapshot` is one seat's whole export. The types beside it are its
//! parts: our cities (`StateCity`, with districts, wonders, the worked plots,
//! queues, menus and purchases), units (`StateUnit`), Great People and their
//! offers, governors, religion, trade routes and route options, the rivals
//! we have met (`StateRival`, `StatePublicEmpireStats`, war declarations),
//! minors and envoys, quests, the World Congress and emergencies, the
//! climate, Great Works, and research quotes. The serde helpers that fill
//! their defaults (`unknown_*`, `minus_one_i64`, `name_or_nothing`,
//! `map_or_empty_sequence`) live here too.
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every `crate::mirror::State...` path is unchanged.
//! Private items, fields and methods the rest of `mirror` uses are
//! `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Accept a production field that is EITHER a type name or Civilization VI's raw hash.
///
/// ⚠⚠ THIS GUARD IS NOT HYPOTHETICAL — its absence was measured. Typing `producing`
/// as `Option<String>` made serde reject every state event carrying a city, because
/// runs recorded before the mod resolved the hash carry `producing: -1743686858`, a
/// NUMBER. `state_from_events` skips a state it cannot parse, silently, so the whole
/// mirror fell back to the newest state that happened to have no cities in it — turn
/// 3 of a 233-turn game, reported as an empty board with no error anywhere.
///
/// Any older run, and any run from a mod build that predates the fix, must keep
/// working: a schema change that silently invalidates recorded history is worse than
/// the missing field it was added for.
fn name_or_nothing<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text),
        // A bare hash is not a name and cannot be turned back into one here; it is
        // the same as knowing nothing, which is what the field meant before.
        _ => None,
    })
}

/// One district a city has placed, in OFFSET coordinates like everything the
/// export sends.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct StateDistrict {
    /// The Civilization VI type name, e.g. `DISTRICT_CAMPUS`.
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub pillaged: bool,
    /// False while Firaxis has placed the district but construction is unfinished.
    /// Historical events lack this field and treated every placement as complete.
    #[serde(default = "district_complete_default")]
    pub complete: bool,
    /// The district's hit points, as Firaxis reports them: DAMAGE taken against a
    /// maximum, for the garrison and for the outer defenses separately.
    ///
    /// ★★★★★ THE FIELD THAT DECIDED 121 TURNS OF PRODUCTION. `pillaged` is a
    /// boolean and a district can be damaged without being pillaged, so it never
    /// stood in for health. Nothing carried hit points, nothing set
    /// [`crate::game::City::encampment_hp`], and it defaults to **0**.
    ///
    /// `Game::can_produce` gates `repair_encampment` on `encampment_hp < 100`, so
    /// on every mirrored board that test passed for any city holding an
    /// Encampment, permanently. The AI queued the repair every turn,
    /// `civvis_orders` correctly refuses to translate a project Civilization VI
    /// does not have, the order was discarded — and nothing else was ordered for
    /// that city, so its queue stayed empty.
    ///
    /// Measured on live run `civvis-20260810T040916Z` (Rome/Trajan, Settler,
    /// Online): Ravenna and Lugdunum, exactly the two cities holding an
    /// Encampment, sat at `producing_hash 0, cost -1, progress -1` from turn 67 to
    /// turn 188 with production yields of 8 and 9 against Rome's 28. 238 discarded
    /// orders — **10.4% of every order CIVVIS issued that game** — and
    /// `ENDTURN_BLOCKING_PRODUCTION` was the run's dominant blocker because two
    /// queues were permanently empty.
    ///
    /// ⚠ AND THE RECORDED FIX WOULD HAVE MADE IT WORSE. The standing plan was to
    /// map `repair_encampment` onto a district BUILD. Both Encampments export as
    /// `pillaged: false, complete: true` — undamaged — so that mapping would have
    /// rebuilt two healthy districts from scratch. The missing translation was
    /// never the bug.
    ///
    /// `-1` means the host did not answer, which must not read as "destroyed".
    #[serde(default = "unknown_damage")]
    pub damage: i32,
    #[serde(default = "unknown_damage")]
    pub max_damage: i32,
    #[serde(default = "unknown_damage")]
    pub wall_damage: i32,
    #[serde(default = "unknown_damage")]
    pub max_wall_damage: i32,
}

/// Firaxis did not answer. Distinct from a real 0, which means "no damage taken".
pub(super) fn unknown_damage() -> i32 {
    -1
}

fn district_complete_default() -> bool {
    true
}

impl Default for StateDistrict {
    fn default() -> Self {
        Self {
            kind: String::new(),
            x: 0,
            y: 0,
            pillaged: false,
            complete: true,
            damage: unknown_damage(),
            max_damage: unknown_damage(),
            wall_damage: unknown_damage(),
            max_wall_damage: unknown_damage(),
        }
    }
}

/// One completed world wonder and the plot Firaxis built it on.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateWonder {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

/// Routes arriving at one city, by origin: foreign (another player's) and
/// domestic (this seat's own).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateIncomingRoutes {
    #[serde(default)]
    pub foreign: i64,
    #[serde(default)]
    pub domestic: i64,
    /// Where each foreign route comes from: the origin city's OFFSET
    /// coordinates and its host player id. With these the mirror seats the
    /// route on the board (`game.routes`, owner = that player's seat) instead
    /// of only counting it, so every destination-side rule that reads the
    /// route list — Zhang Qian's Gold, alliance yields, the World Congress
    /// Trade Policy — sees what the host sees. Empty on an older export.
    #[serde(default)]
    pub origins: Vec<StateRouteOrigin>,
}

/// One foreign route's origin, as `incoming_routes.origins[]` carries it.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateRouteOrigin {
    #[serde(default = "minus_one")]
    pub x: i32,
    #[serde(default = "minus_one")]
    pub y: i32,
    #[serde(default = "minus_one")]
    pub player: i32,
}

/// One World Congress resolution currently in effect, as the host reports it:
/// the `WC_RES_*` type, which option won (1 = A, 2 = B, 0 = the mod could not
/// tell), and the chosen target verbatim (a host player id for PLAYER-targeted
/// resolutions, a `RESOURCE_*`/`DISTRICT_*`/... type name otherwise).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateResolution {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub option: i64,
    #[serde(default)]
    pub target: String,
}

/// One city-state request, as the host's QuestsManager reports it. `type` is
/// the `Quests.QuestType` row (`QUEST_TRAIN_UNIT_TYPE`, …); `target` is the
/// type the quest names, recovered by the mod from the localized description
/// (the host exposes no target accessor), `None` for the three quests that
/// name nothing and where no name matched.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateQuest {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// One major's Envoy count at a city-state, by host player id.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEnvoyCount {
    #[serde(default = "minus_one_i64")]
    pub player: i64,
    #[serde(default)]
    pub envoys: i64,
}

/// The host's climate, as `GameClimate` answers the shipped ClimateScreen.
/// `level` is `GetClimateChangeLevel` (0–7), -1 when it could not be read;
/// every other field is `None` where its accessor was missing.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateClimate {
    #[serde(default = "minus_one_i64")]
    pub level: i64,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub co2_total: Option<f64>,
    #[serde(default)]
    pub co2_ours: Option<f64>,
    #[serde(default)]
    pub sea_level_turns: Option<i64>,
    #[serde(default)]
    pub tiles_flooded: Option<i64>,
    #[serde(default)]
    pub storm_pct: Option<f64>,
    #[serde(default)]
    pub flood_pct: Option<f64>,
    #[serde(default)]
    pub drought_pct: Option<f64>,
}

/// One legal route from one of our cities, priced by the host's own
/// `CanStartRoute` and `CalculateOriginYield…` (TradeRouteChooser.lua:227,
/// :864). Endpoints carry coordinates because Firaxis city ids are only
/// unique per player.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRouteOption {
    #[serde(default = "minus_one_i64")]
    pub origin: i64,
    #[serde(default = "minus_one")]
    pub origin_x: i32,
    #[serde(default = "minus_one")]
    pub origin_y: i32,
    #[serde(default = "minus_one_i64")]
    pub dest: i64,
    #[serde(default = "minus_one")]
    pub dest_player: i32,
    #[serde(default = "minus_one")]
    pub dest_x: i32,
    #[serde(default = "minus_one")]
    pub dest_y: i32,
    /// What the route would pay its origin per turn, summed as the
    /// active-route export sums it.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
}

/// One score row in Firaxis's World Congress emergency tracker.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEmergencyScore {
    #[serde(default)]
    pub player: i64,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub tier: Option<i64>,
}

/// The active seat's membership and score in an exported emergency.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEmergencyOurs {
    #[serde(default)]
    pub member: bool,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub tier: Option<i64>,
}

/// A World Congress emergency or scored competition as Firaxis's tracker
/// reports it. The mirror needs only the active member's score race; native
/// CIVVIS emergencies retain their separate city-capture representation.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEmergency {
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Firaxis's emergency recipient.  Aid Requests award score when this
    /// civilization receives a normal Gold deal, so the live bridge needs the
    /// host player id rather than merely the competition's leaderboard.
    #[serde(default = "minus_one_i64")]
    pub target: i64,
    /// Firaxis uses a negative value once the tracker entry is complete; zero
    /// remains an active final-turn opportunity.
    #[serde(default = "minus_one_i64")]
    pub turns_left: i64,
    #[serde(default)]
    pub begun: bool,
    #[serde(default)]
    pub scores: Vec<StateEmergencyScore>,
    #[serde(default)]
    pub ours: StateEmergencyOurs,
}

/// One population-worked Firaxis plot, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateWorkedPlot {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// The plot's own yields as the host computes them for its owner
    /// (`Plot:GetYield`), so a model-versus-host gap can be located to a tile
    /// rather than only sized per city. `None` on an export older than the
    /// field, or when the host could not read the plot.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
}

/// One Great Work in an exact Firaxis city slot.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGreatWork {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub era: Option<String>,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub building: String,
    #[serde(default)]
    pub slot: i32,
}

/// One appointed Governor exactly as Civilization VI reports it.
///
/// `None` at the snapshot level means the Governor API was unavailable or the
/// event predates this contract. `Some([])` means the player has appointed none.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGovernor {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default = "minus_one_i64")]
    pub city: i64,
    #[serde(default = "minus_one")]
    pub city_player: i32,
    #[serde(default = "minus_one")]
    pub x: i32,
    #[serde(default = "minus_one")]
    pub y: i32,
    #[serde(default)]
    pub established: bool,
    #[serde(default)]
    pub turns_on_site: i32,
    #[serde(default)]
    pub turns_to_establish: i32,
    #[serde(default)]
    pub neutralized_turns: i32,
    #[serde(default)]
    pub promotions: Vec<String>,
}

/// One row of the host's production menu for a city (`StateCity::buildable`):
/// the Civilization VI type name, the engine's production cost and turns for
/// it in that city, the formation tier of a Corps/Army row, and for a district
/// the number of plots the engine offers with up to sixteen of them.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateMenuItem {
    #[serde(default)]
    pub t: String,
    #[serde(default = "unknown_metric")]
    pub c: f64,
    #[serde(default = "unknown_metric")]
    pub p: f64,
    /// Observed work on a district type, including a paused foundation. This
    /// is not overflow and does not identify a plot by itself.
    #[serde(default)]
    pub pr: Option<f64>,
    /// Exact strategic-resource price for this city and formation, when the
    /// host accessor answered. Zero is a price; absence remains unknown.
    #[serde(default)]
    pub r: Option<f64>,
    #[serde(default)]
    pub f: Option<u8>,
    #[serde(default)]
    pub n: Option<i64>,
    #[serde(default)]
    pub s: Option<Vec<StateMenuPlot>>,
}

/// An offset-coordinate plot in a host menu row.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
pub struct StateMenuPlot {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

/// One row of the host's purchase menu (`StateCity::purchasable`): the type
/// name and the engine's price in Gold and in Faith, each absent where the
/// host will not sell for that currency.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StatePurchaseItem {
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub g: Option<f64>,
    #[serde(default)]
    pub f: Option<f64>,
}

/// A native purchase target with its quoted Gold price and prospective site test.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StatePlotPurchaseItem {
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default)]
    pub g: Option<f64>,
    #[serde(default)]
    pub can_have_acropolis: Option<bool>,
}

/// One entry of the build queue BEHIND the head (`StateCity::queue`): the type
/// name, the formation tier of a Corps/Army, and the production already
/// invested in it.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateQueueItem {
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub f: Option<u8>,
    #[serde(default)]
    pub pr: Option<f64>,
}

/// One city as Civilization VI reported it, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCity {
    /// CityDistricts:GetNumAllowedDistrictsRequiringPopulation(), including
    /// native bonuses that cannot be recovered from population alone.
    #[serde(default)]
    pub district_capacity: Option<i32>,
    #[serde(default)]
    pub id: i64,
    /// The name Civilization VI shows on the banner, e.g. `Pasargadae`.
    ///
    /// ⚠ Without this the reconstruction names cities from CIVVIS's own list for
    /// whatever civilization it happened to assign, so a Persian game showed
    /// ROME / OSTIA / ANTIUM and the two screens could not be compared at all.
    #[serde(default)]
    pub name: String,
    /// Civ 6 building type names this city has already finished.
    #[serde(default)]
    pub buildings: Vec<String>,
    /// The subset of `buildings` the host reports pillaged
    /// (`CityBuildings:IsPillaged`). A pillaged building pays nothing until
    /// repaired; without this the mirror paid Antium +6 Science on a raided
    /// Campus for twenty turns. `None` on an older export — an unknown, not
    /// an empty list.
    #[serde(default)]
    pub pillaged_buildings: Option<Vec<String>>,
    /// The religion this city actually follows, by Civ 6 type name, and the one
    /// converting it.
    ///
    /// ★★★★★ Null on all 26,954 city records ever exported before this — the same
    /// shape as `districts`: in the schema, never filled. A city can be converted
    /// away turn by turn and the mirror says nothing, so CIVVIS can neither pursue
    /// a religious victory nor defend against one. Two consecutive completed games
    /// were lost to the same victory type well before the turn limit while CIVVIS
    /// held hundreds of unspent faith.
    #[serde(default)]
    pub religion: Option<String>,
    /// Whether the foreign city's majority was read on a currently visible
    /// plot. `Some(true)` with no `religion` means an observed lack of majority;
    /// false or absent is unknown and must not clear a remembered religion.
    #[serde(default)]
    pub religion_observed: Option<bool>,
    /// The religion gaining on this city, if one is. `religion` alone is a level:
    /// a city holding steady and a city about to flip read identically — the same
    /// reason `loyalty_per_turn` exists beside `loyalty`.
    #[serde(default)]
    pub religion_next: Option<String>,
    /// Turns until `religion_next` takes the city, or -1 when nothing is.
    #[serde(default)]
    pub religion_turns: i64,
    /// Per-city copy of the active pantheon, retained as an export-contract
    /// diagnostic. CIVVIS applies the same belief through the owning player.
    #[serde(default)]
    pub pantheon_active: Option<String>,
    /// Districts this city has placed, each with the plot it sits on.
    ///
    /// ★★★★★ The plot is why this exists. `Item::District` carries a `pos`, so
    /// without one `civvis_production_item` had to refuse every district — and a
    /// city BUILDING a district then read as idle, so CIVVIS re-decided the same
    /// production every turn. Measured on run `civvis-20260731T163924Z`: 60
    /// `DISTRICT_GOVERNMENT` orders between t46 and t128, all `applied: true`, on a
    /// capital that still showed three buildings at t130.
    ///
    /// Empty when the export could not read the city's plots, which is the same
    /// "could not ask" the Lua side leaves nil — not an assertion that there are
    /// none.
    #[serde(default)]
    pub districts: Vec<StateDistrict>,
    #[serde(default)]
    pub wonders: Vec<StateWonder>,
    /// Exact citizen assignments. `None` means an older export or an unavailable
    /// Firaxis query; `Some([])` means this city works no non-centre plots/jobs.
    #[serde(default)]
    pub worked: Option<Vec<StateWorkedPlot>>,
    #[serde(default)]
    pub specialists: Option<Vec<String>>,
    /// Exact works housed by this city. Great People are physical host units but
    /// immediate CIVVIS effects, so this permanent result must cross separately.
    #[serde(default)]
    pub great_works: Option<Vec<StateGreatWork>>,
    /// Firaxis's final city yield vector after every local and empire modifier.
    /// Stored as an additive correction to preserve modeled counterfactual deltas.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
    /// What Civilization VI is CURRENTLY building here, by type name.
    ///
    /// ★★★★ Exported as a raw hash for the whole project (`producing:
    /// -1743686858`) and therefore unusable, so the mirror had no idea what any
    /// city already had underway and CIVVIS re-decided production every turn blind
    /// to work in progress.
    #[serde(default, deserialize_with = "name_or_nothing")]
    pub producing: Option<String>,
    /// Raw production hash retained so a failed name lookup is observable.
    #[serde(default)]
    pub producing_hash: Option<i64>,
    /// Production already invested in `producing`.
    #[serde(default = "unknown_metric")]
    pub production_progress: f64,
    /// ⚠⚠ THE PRODUCTION AVAILABLE TO THE CITY'S BUILD QUEUE, AND IT WAS BEING
    /// THROWN AWAY.
    ///
    /// PR #845 added `production`, `production_cost` and `production_turns` to the
    /// export precisely because they are a DECISION input and not only a diagnostic,
    /// and `StateCity` deserialized only `production_progress`. The other three
    /// arrived on every state event and were dropped — visible the whole time as
    /// `unmapped: schema:city.production,schema:city.production_cost,
    /// schema:city.production_turns` in the decider's own note, which is the
    /// instrument this file added for exactly this failure.
    ///
    /// Live on run `civvis-20260802T083838Z`: `production: 11`, `production_cost: 60`,
    /// `production_turns: 3` for a Quadrireme at `production_progress: 27`.
    /// This is the queue's whole-number production reading, not the exact
    /// per-city yield used by `City:GetYield(YieldTypes.PRODUCTION)`. The latter
    /// crosses in `yields.production` (and in `public_stats.production` for the
    /// empire total) and is the value an economy comparison must use.
    ///
    /// ⚠ This matters far more since #867, which stopped CIVVIS deferring to the
    /// mod's ladder and made it choose production for every city every turn. Choosing
    /// what to build against a production rate the bridge never supplied is the same
    /// shape as the bankruptcy detector reading a `gold_per_turn` nobody wrote.
    #[serde(default = "unknown_metric")]
    pub production: f64,
    /// What `producing` costs in production. With `production` and
    /// `production_progress`, this is what says whether a city can finish an item
    /// before the game ends.
    #[serde(default = "unknown_metric")]
    pub production_cost: f64,
    /// Civilization VI's own estimate of turns remaining on `producing`.
    #[serde(default = "unknown_metric")]
    pub production_turns: f64,
    /// ★★★★★ THE HOST'S OWN PRODUCTION MENU FOR THIS CITY — every unit,
    /// building, wonder, district and project `BuildQueue:CanProduce(hash,
    /// false, true)` says can be STARTED here now, with the engine's cost and
    /// turns for each: the `ProductionPanel.lua` loops, exported by
    /// `CivvisMenus.buildable`. Until this crossed the board chose production
    /// from its own catalogue and learned legality one refusal at a time
    /// (`refused_production`). `None` on an older mod or a failed read is
    /// unknown, not empty — the mirror gates nothing then.
    #[serde(default)]
    pub buildable: Option<Vec<StateMenuItem>>,
    /// The host's purchase menu — what `CityManager.CanStartCommand(city,
    /// PURCHASE, ...)` says can be BOUGHT here now and what
    /// `CityGold:GetPurchaseCost` charges, in Gold and in Faith. `None` is
    /// unknown; an empty list is the host saying nothing is for sale.
    #[serde(default)]
    pub purchasable: Option<Vec<StatePurchaseItem>>,
    /// Native legal plot purchases. None is unknown; an empty list is known
    /// to contain no observed purchase. Suitability does not authorize building.
    #[serde(default)]
    pub plot_purchases: Option<Vec<StatePlotPurchaseItem>>,
    /// The build queue BEHIND `producing` (`BuildQueue:GetAt(i)`, i >= 1).
    /// Only the head ever crossed before.
    #[serde(default)]
    pub queue: Option<Vec<StateQueueItem>>,
    /// Food stockpiled toward the next citizen.
    #[serde(default)]
    pub food: f64,
    /// Firaxis's own Housing for this city, and the part of it that comes from
    /// improvements.
    ///
    /// Population is the term every yield is a linear function of — five
    /// completed live games put science at **1.16 x pop**, with city *count*
    /// predicting nothing — and `Game::housing_growth_mult` gates growth on the
    /// headroom over population: `>= 2` full, `>= 1` **half**, below `-4`
    /// **zero**.
    ///
    /// ⚠ CIVVIS has been deriving this from its own rules on the reconstructed
    /// board with no way to check it — the position Amenities were in before
    /// #967, where a claim made from the model had to be retracted as
    /// unverifiable. This carries the number so the model can be **checked**.
    /// It is not a claim that the model is wrong.
    #[serde(default)]
    pub housing: Option<f64>,
    #[serde(default)]
    pub housing_from_improvements: Option<f64>,
    /// The rest of the host's housing ledger, one term each (`GetHousingFrom*`),
    /// so a modelled total that disagrees can name the term it got wrong.
    /// `None` on an older export; `-1` when the host could not read one term.
    #[serde(default)]
    pub housing_from_water: Option<f64>,
    #[serde(default)]
    pub housing_from_buildings: Option<f64>,
    #[serde(default)]
    pub housing_from_districts: Option<f64>,
    #[serde(default)]
    pub housing_from_civics: Option<f64>,
    #[serde(default)]
    pub housing_from_great_people: Option<f64>,
    #[serde(default)]
    pub housing_from_starting_era: Option<f64>,
    #[serde(default)]
    pub housing_from_great_works: Option<f64>,
    /// Growth as the host computes it: surplus after consumption, the threshold
    /// for the next citizen, the multipliers, and the host's own turn forecast.
    #[serde(default = "unknown_metric")]
    pub food_surplus: f64,
    #[serde(default = "unknown_metric")]
    pub growth_threshold: f64,
    #[serde(default = "unknown_metric")]
    pub growth_turns: f64,
    #[serde(default = "unknown_metric")]
    pub housing_growth_mult: f64,
    #[serde(default = "unknown_metric")]
    pub happiness_growth_mult: f64,
    #[serde(default = "unknown_metric")]
    pub overall_growth_mult: f64,
    /// Native citizen preferences (`CityPanel.lua:445-477`). Unknown on old
    /// exports; false must not be invented when the host accessor fails.
    #[serde(default)]
    pub food_favored: Option<bool>,
    #[serde(default)]
    pub food_disfavored: Option<bool>,
    /// The bridge owns this temporary Food preference, so it may release it.
    /// Preferences set elsewhere are observed but never claimed as ours.
    #[serde(default)]
    pub food_focus_managed: Option<bool>,
    /// Where each yield comes from, in the host's own words: the text behind the
    /// city panel's per-yield tooltip (`City:GetYieldToolTip`), icon markup
    /// stripped, one entry per yield name. Diagnostic — nothing in the
    /// reconstruction reads it; `tools/civ6_yield_drift.py` parses the amounts.
    #[serde(default)]
    pub yield_sources: Option<std::collections::BTreeMap<String, String>>,
    /// Routes other cities run INTO this one, foreign and domestic, as the host
    /// counts them (the shipped Trade Overview's walk of every other player's
    /// outgoing routes). The destination earns from them under a few rules —
    /// Zhang Qian's +2 Gold per incoming foreign route was live on Aquileia from
    /// t131 of run civvis-20260816T040537Z — and the mirror's own route list holds
    /// only routes this seat sends. Diagnostic for now; `None` on an older export.
    #[serde(default)]
    pub incoming_routes: Option<StateIncomingRoutes>,
    /// The city centre plot's own yields (`Plot:GetYield` on the centre), which
    /// Firaxis lists among the worked plots and CIVVIS floors to 2 Food /
    /// 1 Production before assigning citizens.
    #[serde(default)]
    pub center_yields: Option<crate::rules::Yields>,

    /// Civilization VI's own amenity ledger for this city, and the multiplier it
    /// puts on every non-food yield.
    ///
    /// ★★★★★ NONE OF THIS WAS EVER ASKED FOR. Neither mod exported an amenity,
    /// happiness or luxury field and this struct imported none, so CIVVIS's whole
    /// happiness picture was derived from its own rules on the reconstructed board
    /// and never once checked against the host.
    ///
    /// That derived number is not decoration: [`crate::game::Game::amenity_yield_mult_for`] bands
    /// it straight onto science, production, gold, **culture** and faith — `+5` →
    /// 1.20, `0` → 1.00, `-4` → 0.80, `-6` → 0.70. CIVVIS's model puts the live
    /// empires at `-4/-5`, i.e. paying a **25-30% tax on every yield**, which would
    /// be the largest single multiplier on the board.
    ///
    /// ⚠ **The economy drift line cannot settle this and must not be read as if it
    /// could.** It compares model totals against host totals; a spurious 0.75 here
    /// and an overestimate anywhere else cancel to a clean-looking number. Only the
    /// host's own figure decides it, which is what these carry.
    ///
    /// `happiness_yield_mult` is `GetHappinessNonFoodYieldModifier` — the host's own
    /// version of the very quantity CIVVIS bands for itself, so the two are directly
    /// comparable.
    ///
    /// ⚠ **UNKNOWN HAS TWO SHAPES HERE AND A CONSUMER MUST REJECT BOTH.** A field the
    /// host never sent defaults to [`unknown_metric`], which is `f64::NAN`; a field
    /// the host tried and failed to read arrives as the mod's `-1`. Neither is zero,
    /// and a city with genuinely zero amenities must not reconstruct like a city that
    /// said nothing — a mirror built before this export would otherwise read as a
    /// perfectly happy empire, which is the instrument inventing good news.
    ///
    /// The `>= 0.0` test used by [`host_amenity_report`] rejects both, because every
    /// comparison against `NAN` is false. Any new reader must do the same; `!= -1.0`
    /// alone would let `NAN` through.
    #[serde(default = "unknown_metric")]
    pub amenities: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_needed: f64,
    /// Civ 6's happiness *state*, an enum index, not a count. 4 is "content" —
    /// the shipped CityPanel special-cases exactly that value.
    #[serde(default = "unknown_metric")]
    pub happiness: f64,
    #[serde(default = "unknown_metric")]
    pub happiness_yield_mult: f64,
    /// Where the amenities come from, so a shortfall names its own repair rather
    /// than only its size. Luxuries and entertainment are the two CIVVIS can act on.
    #[serde(default = "unknown_metric")]
    pub amenities_luxuries: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_entertainment: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_civics: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_city_states: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_war_weariness: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_bankruptcy: f64,
    /// The remaining amenity sources the shipped CitySupport reads, so the host's
    /// count decomposes completely and a modelled total can name its wrong term.
    #[serde(default = "unknown_metric")]
    pub amenities_great_people: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_religion: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_national_parks: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_starting_era: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_improvements: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_districts: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_natural_wonders: f64,
    /// Loyalty CHANGE per turn. `loyalty` alone is a level, and a city at 100
    /// falling fast looks identical to one at 100 holding steady — which is exactly
    /// how a city was lost at t98 with loyalty reading 100.
    #[serde(default = "unknown_metric")]
    pub loyalty_per_turn: f64,
    /// Civilization VI's own verdict on who this city would defect to, by player id.
    #[serde(default)]
    pub falls_to: i64,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub pop: i32,
    #[serde(default)]
    pub capital: bool,
    /// Original capital used by Domination, unlike `capital` (current Palace).
    /// Absent in old exports or when the host method is unavailable.
    #[serde(default)]
    pub original_capital: Option<bool>,
    /// ★★★ THE CAPTURE DECISION THE HOST IS WAITING ON. The Firaxis player id
    /// this city was just taken from (`City:GetJustConqueredFrom()`,
    /// `Popups/RazeCity.lua:86`), exported on exactly the city the shipped
    /// popup would show — `Player:GetCities():GetNextCapturedCity()`
    /// (`RazeCity.lua:71`) — and absent once any keep/raze/liberate directive
    /// has been taken. The mirror maps it onto `City::captured_from` as a
    /// seat, so `pending_city_capture_actions` offers the board the same
    /// three choices the popup offers, and the order bridge carries the one
    /// it takes back as kind `city`. `None` on every export before this: the
    /// controller lists the host's `CONSIDER_RAZE_CITY` blocker as soft, so
    /// the host's default — keep — decided every capture unseen.
    #[serde(default)]
    pub captured_from: Option<i64>,
    /// The founder's Firaxis player id (`City:GetOriginalOwner()`,
    /// `RazeCity.lua:85`), whom LIBERATE returns the city to. Mapped onto
    /// `City::original_owner` when the founder sits on a mirrored seat; an
    /// unmapped founder leaves the board's default (the owner), which offers
    /// neither Raze nor Liberate — the honest degraded answer.
    #[serde(default)]
    pub original_owner: Option<i64>,
    #[serde(default = "unknown_metric")]
    pub defense: f64,
    /// Native city-center GetAttackStrength, only while owned or visible.
    #[serde(default)]
    pub ranged_strength: Option<f64>,
    /// Damage and capacity for the city garrison and outer-defense health pools.
    /// These are the same four values Firaxis's city banner displays.
    #[serde(default = "unknown_metric")]
    pub damage: f64,
    #[serde(default = "unknown_metric")]
    pub max_damage: f64,
    #[serde(default = "unknown_metric")]
    pub wall_damage: f64,
    #[serde(default = "unknown_metric")]
    pub max_wall_damage: f64,
    /// Current loyalty, 0-100. Below ~50 and falling, the city is on its way to
    /// revolting to whoever is pressing on it.
    #[serde(default = "unknown_strength")]
    pub loyalty: f64,
}

/// One unit as Civilization VI reported it, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateUnit {
    /// Rock Band activation highlights from the host, in offset coordinates.
    /// An empty list is authoritative; missing means the API was not read.
    #[serde(default)]
    pub concert_plots: Option<Vec<StateActivationPlot>>,
    #[serde(default)]
    pub id: i64,
    /// ★★★★★ `type` IS AN ALIAS AND IT WAS MISSING, SO EVERY BARBARIAN WAS DROPPED.
    ///
    /// Our own units are exported as `kind`; the `hostiles` list — which is the ONLY
    /// way barbarians reach this bridge, because `rivals` is built from
    /// `GetAliveMajorIDs()` and cannot contain them — exports `type`. With only
    /// `kind` deserialized every hostile arrived with an EMPTY name, failed the
    /// ruleset lookup, and was silently discarded.
    ///
    /// So `state.hostiles` has been exported, passed to `plant_unit`, and thrown away
    /// 100% of the time. CIVVIS has never once had a barbarian on its board in this
    /// bridge — which means the settler danger rule built on `captor_within` was
    /// looking at an empty threat list, and every "the settler walked into a
    /// barbarian" diagnosis was about a unit CIVVIS could not see.
    ///
    /// ⚠ Found only because `dropped_units` started naming what did not make it onto
    /// the board: the entries read `@37,14:untranslatable` with NO unit type at all,
    /// and an empty name is not a translation failure, it is a field that was never
    /// read. `unmapped` could not show it either — it records the offending name, and
    /// the name was `""`.
    #[serde(default, alias = "type")]
    pub kind: String,
    /// What this unit REPLACES, when it is a civilization unique — Civ 6's
    /// `UnitReplaces.ReplacesUnitType`. See the fallback in `plant_unit`.
    #[serde(default)]
    pub base: Option<String>,
    /// The unit's `PromotionClass` — the last honest rung for a STANDALONE
    /// unique that replaces nothing (Malón Raider, Varu, Nihang), whose `base`
    /// is therefore absent. See [`class_representative`].
    #[serde(default)]
    pub class: Option<String>,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub hp: f64,
    /// Static host combat values retained for export-contract diagnostics. CIVVIS
    /// gets the same base values from its audited ruleset.
    #[serde(default)]
    pub combat: f64,
    #[serde(default)]
    pub ranged: f64,
    /// Present on the aggregate hostile export; ownership is implicit elsewhere.
    #[serde(default = "minus_one_i64")]
    pub player: i64,
    /// Whether this hostile belongs to the Free Cities player (`IsFreeCities`),
    /// set by the mod on `hostiles[]` so the mirror can seat it without knowing
    /// Firaxis's index for that aggregate. Absent on an older export and on a
    /// barbarian; the exported Free Cities minor's `player` is the other key.
    #[serde(default)]
    pub free: bool,
    /// ⚠⚠⚠ NOT "movement available this turn", whatever it looks like. This is
    /// `GetMovesRemaining` sampled at the instant the export is written, and that
    /// instant is not the start of the seat's turn.
    ///
    /// It has now misled twice, in opposite directions, and both times the reader
    /// believed the old one-line description of this field:
    ///
    /// - Feeding it into `moves_left` **silenced CIVVIS completely** — the export
    ///   at the start of turn 31 of run `civvis-20260730T120107Z` had 7 of 8 units
    ///   at `moves: 0`, so `advanced_units` broke on `moves_left <= 0.0` for
    ///   almost every unit and logged 0 actions per turn. See the ★★★★★ note in
    ///   the persistent-sync path, which takes the full allowance instead.
    /// - **Joining it to refusal events by turn number produced a measurement that
    ///   was simply false** and three merged PRs rested on it (#1548, #1550,
    ///   #1552, corrected in #1557). It read `moves: 0` for builders that the mod
    ///   recorded at 2–4 movement at the instant of the refusal.
    ///
    /// So: for "could this unit act *then*", read the value the mod puts in the
    /// EVENT, taken at the point of the decision. For "what can it do *now*", use
    /// [`mirror_unit_moves`]. This field is the raw export and answers neither
    /// question on its own — keep it for fidelity checks and diagnostics.
    #[serde(default = "unknown_strength")]
    pub moves: f64,
    /// Where a multi-turn host path will carry this unit at the start of its
    /// NEXT turn (`UnitManager.GetQueuedDestination`), in offset coordinates.
    /// A unit with one enters the next turn having already spent movement on
    /// it before the brain can act; the mod cancels combat units' queued
    /// paths at turn start and caps every MOVE_TO to the turn's reach, so on a
    /// current mod this is empty. Absent on an older export.
    #[serde(default)]
    pub queued_dest: Option<(i32, i32)>,
    /// Whether the unit is embarked (`Unit:IsEmbarked`). Absent on an older
    /// export; the mirror then infers it from standing on water.
    #[serde(default)]
    pub embarked: Option<bool>,
    /// Attacks left this turn (`Unit:GetAttacksRemaining`). On a mid-turn
    /// combat frame a unit that already struck reports 0 and the board plans
    /// no second strike for it. Absent on an older export → the fresh-turn
    /// allowance.
    #[serde(default)]
    pub attacks_remaining: Option<i32>,
    /// The unit's live range (`Unit:GetRange()`, `Panels/UnitPanel.lua:2250`).
    /// For an aircraft this is the operational range from the plot it stands
    /// on, which is its base — a Civilization VI aircraft sits on its
    /// airfield, city or carrier between sorties, so `x`/`y` name the base and
    /// this names how far `AIR_ATTACK`, `REBASE` and `PATROL` reach from it.
    /// Exported for the seat's own units; absent on an older export.
    #[serde(default)]
    pub range: Option<i32>,
    /// ★★★ THE HOST'S OWN UPGRADE VERDICT (docs/FIDELITY.md, "The one-to-one
    /// map", item 9). `upgrade_to` is the successor `UnitCommandResults.
    /// UNIT_TYPE` names, `upgrade_cost` is `Unit:GetUpgradeCost()`, and
    /// `upgrade_blocked_reason` is present exactly when a successor exists and
    /// the strict `CanStartCommand(unit, UPGRADE, false, true)` cannot start
    /// it this turn (`Panels/UnitPanel.lua:468-483`). A unit with no successor
    /// exports none of the three; an older mod exports none of them for any
    /// unit, and the board's own rules then decide, unchanged.
    #[serde(default)]
    pub upgrade_to: Option<String>,
    #[serde(default)]
    pub upgrade_cost: Option<f64>,
    /// Both `GetUpgradeResourceCost()` returns, resolved by resource Index.
    #[serde(default)]
    pub upgrade_resource_cost: Option<f64>,
    #[serde(default)]
    pub upgrade_resource: Option<String>,
    #[serde(default)]
    pub upgrade_blocked_reason: Option<String>,
    /// The per-type upkeep the shipped Report screen sums, by formation
    /// (`UnitManager.GetUnitMaintenance` / `GetUnitCorpsMaintenance` /
    /// `GetUnitArmyMaintenance`, `Screens/ReportScreen.lua:314-334`), before
    /// the player's per-unit discount.
    #[serde(default)]
    pub maintenance: Option<f64>,
    /// `Unit:GetReligiousStrength()` and `Unit:GetMaxMoves()`, the unit
    /// panel's own stats (`UnitPanel.lua:2257`, `:2242`).
    #[serde(default)]
    pub religious_strength: Option<f64>,
    #[serde(default)]
    pub max_moves: Option<f64>,
    /// `UnitManager.GetActivityType` named as `WorldTracker.lua:544` does:
    /// `sleep`, `hold`, `operation`, `awake`.
    #[serde(default)]
    pub activity: Option<String>,
    /// A Spy's running operation (`Unit:GetSpyOperation()`, -1 → absent;
    /// `EspionageOverview.lua:659`), the turn it ends, and — for an IDLE Spy
    /// standing in a city — the operations `UnitManager.CanStartOperation`
    /// would let it start there (`Choosers/EspionageChooser.lua:196-213`),
    /// as host operation types. The menu is absent, never empty.
    #[serde(default)]
    pub spy_operation: Option<String>,
    #[serde(default)]
    pub spy_operation_end_turn: Option<i64>,
    #[serde(default)]
    pub spy_missions_available: Option<Vec<String>>,
    /// Beside the menu: each offered mission's target plot and host odds
    /// (`CivvisSpyMenu.read`). Absent on older agents.
    #[serde(default)]
    pub spy_mission_targets: Option<Vec<StateSpyTarget>>,
    /// The host purchase city of a Spy's plot, including a district operation:
    /// `UnitPanel.lua:2151-2155`, not a nearest-city or city-center guess.
    #[serde(default)]
    pub spy_city: Option<StateSpyCity>,
    /// Exact host experience and promotion state. Option distinguishes an older
    /// archive that never exported the facts from a level-one unit with none.
    #[serde(default)]
    pub xp: Option<i64>,
    #[serde(default)]
    pub level: Option<i32>,
    /// Rock Band component level, distinct from the generic promotion level.
    #[serde(default)]
    pub rock_band_level: Option<i32>,
    /// Native cumulative album sales; zero is an observed counter value.
    #[serde(default)]
    pub album_sales: Option<i64>,
    #[serde(default)]
    pub promotions: Option<Vec<String>>,
    /// Current native Rock Band menu; empty is an observed lack of choices.
    #[serde(default)]
    pub offered_promotions: Option<Vec<String>>,
    /// Civilization VI separates builder and religious charges. CIVVIS has one
    /// typed charge counter, so the mirror selects the applicable observed pool.
    #[serde(default)]
    pub build_charges: Option<i32>,
    #[serde(default)]
    pub spread_charges: Option<i32>,
    /// The religion physically carried by a religious unit, by Firaxis type name.
    #[serde(default)]
    pub religion: Option<String>,
    /// Already fortified. Civilization VI REFUSES `FORTIFY` on a unit that is, so a
    /// board that did not carry this re-ordered it every turn — 28 refusals in run
    /// 233331Z, exactly one per turn from t196 on.
    #[serde(default)]
    pub fortified: bool,
    #[serde(default)]
    pub fortify_turns: i32,
    /// ★★★★★ THE MERGE TIER, AND WHY `FORM_ARMY` COULD NEVER BE SENT LIVE.
    ///
    /// 0 = standard, 1 = Corps/Fleet, 2 = Army/Armada — Firaxis's own
    /// `Unit:GetMilitaryFormation()`, mapped through the shipped
    /// `MilitaryFormationTypes` enum by the mod's `CivvisMilitaryFormation`.
    ///
    /// #2373 wired `Action::CombineUnits` to `UNITCOMMAND_FORM_CORPS` and
    /// `UNITCOMMAND_FORM_ARMY` and picks between them from the mirror's
    /// [`crate::game::Unit::formation`]. The live seat runs `--fresh-board`, so
    /// the mirror is rebuilt from this export every turn — and the export
    /// carried no tier, so every unit was reconstructed as STANDARD and the seat
    /// could only ever ask for a Corps. Exporting this is what makes the Army
    /// half of the unit-consolidation layer reachable at all.
    ///
    /// ⚠ `None` — an older export that never carried the field — and the mod's
    /// own `-1` ("asked, could not answer") both mean UNKNOWN, and neither may
    /// be read as standard. [`apply_unit_observation`] accepts only 0..=2 and
    /// otherwise leaves the board's own tier alone; a fallback that read as
    /// standard is precisely the `GetDefenseStrength` sentinel trap, which
    /// answered −1 for the project's entire life without anyone being able to
    /// tell it from an answer.
    ///
    /// ⚠ NOT [`StateUnit::formation_count`] below. That is the ESCORT stack size
    /// (`GetFormationUnitCount`), which `LinkUnits` reconstructs; a Corps is one
    /// unit and reports a count of 1. Same word, different mechanism.
    #[serde(default)]
    pub formation: Option<i32>,
    /// Number of members in Firaxis's escort/support formation. The stock Unit
    /// Panel exposes this value, and it distinguishes two units sharing a plot from
    /// two units that move as one formation.
    #[serde(default)]
    pub formation_count: i32,
    /// Firaxis represents a Great Person as a physical unit and exposes the exact
    /// plots on which that named individual can activate. CIVVIS applies Great
    /// Person effects immediately, so the live bridge uses these host-validated
    /// targets to enact the same semantic without inventing placement rules.
    #[serde(default)]
    pub great_person: Option<StateGreatPerson>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGreatPerson {
    #[serde(default)]
    pub individual: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    /// Firaxis `ActionRequiresCompletedDistrictType` for this exact physical
    /// individual. The current market offer is no longer authoritative after
    /// recruitment, so the unit carries its own copy into the production AI.
    #[serde(default)]
    pub required_district: Option<String>,
    /// Firaxis `ActionRequiresMissingBuildingType` for this exact physical
    /// individual. The building must be absent: some activations, such as
    /// James of St. George's, supply it themselves.
    #[serde(default)]
    pub required_missing_building: Option<String>,
    /// Firaxis `ActionRequiresCityGreatWorkObjectType` for this exact physical
    /// individual, for example `GREATWORKOBJECT_ARTIFACT` for Mary Leakey.
    #[serde(default)]
    pub required_great_work: Option<String>,
    #[serde(default)]
    pub charges: i32,
    #[serde(default)]
    pub can_activate: bool,
    #[serde(default)]
    pub activation_plots: Vec<StateActivationPlot>,
    /// Empty Great Work slots anywhere in the empire that this person's work
    /// fits, counted by the host through its own `GreatWork_ValidSubTypes`
    /// table. `None` for classes that do not consume slots, and for exports
    /// from a mod that could not read the slot tables — never a defaulted 0,
    /// because 0 is a claim ("build capacity") and `None` is an absence.
    #[serde(default)]
    pub empty_slots: Option<u32>,
}

impl StateGreatPerson {
    /// This person has nowhere to put its work: it cannot activate, and no
    /// tile the host offers can take one.
    ///
    /// ★★★★★ `empty_slots == Some(0)` IS NOT THE SAME QUESTION, AND EVERY
    /// ESCAPE HATCH WAS ASKING IT. Live run `civvis-20260822T020434Z` reached
    /// turn 231 with **three Great Artists, three Great Writers, three Great
    /// Musicians and a Great Scientist stacked in Rome** — and
    /// `orders.sqlite` holds **not one order of any kind, ever, for any of
    /// the nine**. `ACTIVATE_GREAT_PERSON` fired 18 times that game, all of
    /// them Scientists, Merchants and one Engineer; no Writer, Artist or
    /// Musician was used once in 231 turns.
    ///
    /// Their export says why. Every one reads `can_activate: false` with
    /// **every single `activation_plot` at `slot_open: false`** — the host
    /// saying, tile by tile, that none of them can take the work — while
    /// `empty_slots` reads **24 for the Writers, 4 for the Musicians, 2 for
    /// the Artists**, because that field counts compatible empty slots
    /// EMPIRE-WIDE by the survey's reckoning, including slots on plots the
    /// engine will not offer this person at all.
    ///
    /// So all three exits were shut at once. The driver would not activate
    /// (`can_activate` false), would not walk (every plot known-full is never
    /// a destination, by design and correctly), the mirror's needs machinery
    /// would not ask for capacity, and the work-sale arm would not free a
    /// slot — the last two because both gate on `empty_slots == Some(0)` and
    /// it was 24. Nine Great People fell clean through every branch and idled
    /// for the whole game.
    ///
    /// The operative question is not how many slots the empire owns; it is
    /// whether this person can REACH one. The host answers that per plot with
    /// `slot_open`, so ask it there. `empty_slots == Some(0)` stays a
    /// sufficient condition — it still is one — and `None` keeps the older
    /// control mod's benefit of the doubt exactly as before, never read as
    /// either claim.
    ///
    /// A person the host offers NO plot at all is deliberately not starved
    /// here: that is a missing district, not a missing slot, and the needs
    /// machinery already has its own branch for it — which is how the same
    /// run's Great Scientist correctly asks for the Spaceport its
    /// `required_district` names.
    pub fn slot_starved(&self) -> bool {
        if self.can_activate {
            return false;
        }
        if self.empty_slots == Some(0) {
            return true;
        }
        !self.activation_plots.is_empty()
            && self
                .activation_plots
                .iter()
                .all(|plot| plot.slot_open == Some(false))
    }
}

/// One currently recruitable entry in Firaxis's Great Person timeline.
///
/// The enclosing map is keyed by `GREAT_PERSON_CLASS_*`; keeping the class
/// outside this value mirrors the live timeline's one-current-offer-per-class
/// shape and keeps the bridge's empty-map `nil` convention intact.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGreatPersonOffer {
    /// Firaxis's named individual, retained for a readable blocker and for
    /// telemetry. It may be absent on an older control mod.
    #[serde(default)]
    pub individual: Option<String>,
    /// Firaxis `ActionRequiresCompletedDistrictType`, when this individual
    /// cannot activate without an already-completed district of that family.
    #[serde(default)]
    pub required_district: Option<String>,
    /// Firaxis `ActionRequiresMissingBuildingType`. This is a negative gate:
    /// the named building must be absent in at least one eligible city.
    #[serde(default)]
    pub required_missing_building: Option<String>,
    /// Firaxis `ActionRequiresCityGreatWorkObjectType`, retained as the raw
    /// host object name until the mirror translates it to a CIVVIS work kind.
    #[serde(default)]
    pub required_great_work: Option<String>,
}

/// One founded religion as the host's Religion screen lists it: its type, the
/// host player id of its founder, and every belief it holds.
///
/// `taken_religion_beliefs` is the union across religions and says only what
/// is no longer available; it cannot say WHICH religion Divine Inspiration
/// belongs to. A city following Catholicism gets Catholicism's follower
/// beliefs — Rome's three Wonders paid twelve Faith under Divine Inspiration
/// in run civvis-20260816T123936Z while the mirror had that belief parked on
/// whichever seat happened to be zipped with the religion — so each religion's
/// beliefs have to sit on its founder's seat, and only there.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateReligion {
    /// `RELIGION_CATHOLICISM` and the like.
    #[serde(rename = "type", default)]
    pub religion: String,
    /// The founder's host player id; `-1` when the host could not say.
    #[serde(default = "unknown_player")]
    pub founder: i64,
    #[serde(default)]
    pub beliefs: Vec<String>,
}

fn unknown_player() -> i64 {
    -1
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateActivationPlot {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub distance: i32,
    /// Whether the host knows a compatible empty Great Work slot stands on
    /// this exact tile. The engine's highlight names a cultural person's
    /// districts whether or not a slot is free, so this is the difference
    /// between a destination and a wedge: `Some(true)` = a matching empty
    /// slot is here, `Some(false)` = one of our districts with no such slot
    /// (eleven people stood on one of these for a whole run), `None` =
    /// unknown — a wonder tile, a non-slot-consuming class, or an export
    /// from an older control mod. `None` must never be read as either claim.
    #[serde(default)]
    pub slot_open: Option<bool>,
}

/// One active outgoing trade route as Civilization VI reports it.
///
/// Firaxis keeps a Trader on the map while it services a route.  CIVVIS instead
/// removes that unit and stores the route, so carrying both the unit id and both
/// city ids is necessary: the former prevents re-ordering the busy Trader and the
/// latter preserves capacity and route yields in the reconstructed economy.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateTradeRoute {
    /// Civilization VI's id for the Trader travelling this route.
    #[serde(default)]
    pub trader: i64,
    /// Civilization VI city ids for the two endpoints.
    #[serde(default)]
    pub origin: i64,
    #[serde(default)]
    pub destination: i64,
    #[serde(default = "minus_one")]
    pub destination_player: i32,
    /// Endpoint positions make an international route recoverable even when its
    /// city id has not yet been retained by the visible-rival mirror.
    #[serde(default = "minus_one")]
    pub origin_x: i32,
    #[serde(default = "minus_one")]
    pub origin_y: i32,
    #[serde(default = "minus_one")]
    pub destination_x: i32,
    #[serde(default = "minus_one")]
    pub destination_y: i32,
    /// Trading Posts on the host's own path for this route (origin excluded,
    /// destination included), filed by owner: our cities and other players'.
    /// `None` on an older export or when the path could not be read; the
    /// model then walks its own straight line.
    #[serde(default)]
    pub posts_own: Option<i64>,
    #[serde(default)]
    pub posts_foreign: Option<i64>,
    /// What the route pays its origin per turn, as the host sums it
    /// (`CalculateOriginYieldsFromPotentialRoute` + `…FromPath` +
    /// `…FromModifiers`, times the international multiplier — the shipped
    /// TradeSupport recipe). `None` on an older export.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
}

/// Empire-wide facts Civilization VI exposes in standings without exposing
/// where the underlying cities, wonders, or weapons are located.
///
/// The values are optional because an older control mod has no `public_stats`
/// object, while `-1` remains the live export's per-field "could not read"
/// sentinel. Keeping the aggregate separate from `StateRival::cities` lets the
/// mirror remain fog-honest while the player HUD stays complete.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StatePublicEmpireStats {
    #[serde(default)]
    pub city_count: Option<i64>,
    #[serde(default)]
    pub population: Option<i64>,
    #[serde(default)]
    pub food: Option<f64>,
    #[serde(default)]
    pub production: Option<f64>,
    #[serde(default)]
    pub wonder_count: Option<i64>,
    #[serde(default)]
    pub suzerain_count: Option<i64>,
    #[serde(default)]
    pub nuclear_devices: Option<i64>,
    #[serde(default)]
    pub thermonuclear_devices: Option<i64>,
}

/// The World Congress diplomatic standing as of the last session, and the turn
/// that session was held. See [`StateSnapshot::congress_dvp`] for why a
/// met-gated rival list cannot answer "who is about to win diplomatically".
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCongressDvp {
    /// The game turn the seat was shown this standing.
    #[serde(default)]
    pub turn: Option<i64>,
    /// One entry per alive major, the seat included.
    #[serde(default)]
    pub points: Vec<StateCongressDvpEntry>,
}

/// One civilization's diplomatic-victory points as the congress reported them.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCongressDvpEntry {
    #[serde(default)]
    pub player: usize,
    #[serde(default)]
    pub points: i64,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateWarDeclaration {
    pub statement: String,
    #[serde(default)]
    pub allowed: Option<bool>,
}

/// One row of a rival's opinion of us (`StateRival::opinion_modifiers`).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateOpinionModifier {
    /// The LOC key the shipped screen looks up, e.g.
    /// `LOC_DIPLO_MODIFIER_DENOUNCED`.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub score: f64,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRival {
    #[serde(default)]
    pub player: usize,
    /// Civ 6 type name, e.g. `CIVILIZATION_NUBIA`. Mapped onto CIVVIS's roster by
    /// [`civvis_civ_name`] so the rival list reads the same on both screens.
    #[serde(default)]
    pub civ: String,
    #[serde(default)]
    pub leader: String,
    /// The government the rival's diplomacy panel currently shows. It is a
    /// public empire fact, so a fogged rival must not render as unformed.
    #[serde(default)]
    pub government: Option<String>,
    /// Firaxis's current age state for this met rival. These public flags keep
    /// the standings and loyalty lens honest without exporting private plans.
    #[serde(default)]
    pub dark_age: Option<bool>,
    #[serde(default)]
    pub golden_age: Option<bool>,
    #[serde(default)]
    pub heroic_golden_age: Option<bool>,
    /// Whether Civilization VI permits at least one supported war declaration
    /// right now. Major permissions come from IsDiplomaticActionValid for each
    /// named war; the order handler validates the selected type again.
    /// Missing on an older export is unknown, not an explicit refusal.
    #[serde(default)]
    pub can_declare: Option<bool>,
    /// Exact unilateral war permissions, not the aggregate above. Present
    /// observations authorize only explicit true entries; unknown types wait.
    /// None preserves archived/legacy aggregate-permission semantics.
    #[serde(default)]
    pub war_declarations: Option<Vec<StateWarDeclaration>>,
    #[serde(default)]
    pub score: i64,
    /// Firaxis's current Diplomatic Victory-point total for this rival.
    ///
    /// This is public standings information rather than a fogged tactical fact:
    /// `AdvancedAi` uses it to identify the diplomatic leader for victory denial
    /// and World Congress targeting. `None` means an older control mod could not
    /// export the value; a real zero must remain distinguishable from that case.
    #[serde(default)]
    pub dvp: Option<i64>,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    #[serde(default)]
    pub at_war: bool,
    /// The other majors this rival is at war with, by host player id, among
    /// those our seat has met (`IsAtWarWith` on the rival's own diplomacy;
    /// the diplomacy ribbon shows a met leader's wars). `None` on an older
    /// export. Mirrored into [`Game::observed_rival_wars`] by
    /// [`apply_rival_wars`].
    #[serde(default)]
    pub at_war_with: Option<Vec<usize>>,
    /// Whether this rival currently grants OUR seat Open Borders — the shipped
    /// overview's "received" direction (`HasOpenBordersFrom`). The import
    /// records the grant on the mirrored seat and stops sealing that rival's
    /// fogged border while it holds, so a passage the seat just bought is
    /// ground the planner can actually use. `None` on an older export.
    #[serde(default)]
    pub open_borders: Option<bool>,
    /// Whether this rival holds Early Empire, i.e. whether its border is
    /// enforced at all (`CIVIC_ENFORCE_BORDERS`). Without it anyone may walk
    /// through; with it only war, a grant, or an alliance opens the ground.
    /// The mirror writes it onto the seat as `Player::borders_enforced`.
    /// `None` on an older export — read as ENFORCED, the measured rule: run
    /// civvis-20260826T184456Z sent 37 military steps into a rival's closed
    /// border and none arrived.
    #[serde(default)]
    pub enforces_borders: Option<bool>,
    /// The host's own relationship state for this rival toward us —
    /// `GameInfo.DiplomaticStates[GetDiplomaticAI():GetDiplomaticStateIndex(us)]
    /// .StateType`: `DIPLO_STATE_WAR`, `_DENOUNCED`, `_UNFRIENDLY`, `_NEUTRAL`,
    /// `_FRIENDLY`, `_DECLARED_FRIEND`, `_ALLIED` (`DiplomacyActionView.lua:870`).
    /// `None` on an older export, which keeps the `can_declare` fallback below.
    /// See [`apply_host_diplomacy`] for what each state writes on the board.
    #[serde(default)]
    pub diplomatic_state: Option<String>,
    /// Grievances we hold against them and they against us, both `>= 0`. The
    /// host keeps ONE signed balance per pair (`GetGrievancesAgainst`,
    /// `DiplomacyActionView_WorldCongressTab.lua:42`); the mod splits it.
    #[serde(default)]
    pub our_grievances_against_them: Option<f64>,
    #[serde(default)]
    pub grievances_against_us: Option<f64>,
    /// `Game.GetGameDiplomacy():GetGrievanceChangePerTurn(them, us)` — the
    /// host's per-turn drift of that balance. Crosses for the record and
    /// `--dump-mirror`; no decision reads it yet (the board's own decay rule
    /// runs in `Game::process_diplomacy`).
    #[serde(default)]
    pub grievance_change_per_turn: Option<f64>,
    /// `GetAllianceType` as `GameInfo.Alliances[i].AllianceType`
    /// (`ALLIANCE_MILITARY`, …), absent when the host answers `-1`; the level
    /// and the turns until expiry from the shipped Alliance tab.
    #[serde(default)]
    pub alliance_type: Option<String>,
    #[serde(default)]
    pub alliance_level: Option<i32>,
    #[serde(default)]
    pub alliance_turns_left: Option<i64>,
    /// `GetDenounceTurn` from each side and `GetDeclaredFriendshipTurn`
    /// (`DiplomacyActionView.lua:1486-1511`), with the game's
    /// `GetDenounceTimeLimit`, so the Formal War wait on the board is the
    /// host's own clock. A value `<= 0` means no such turn.
    #[serde(default)]
    pub our_denounce_turn: Option<i64>,
    #[serde(default)]
    pub their_denounce_turn: Option<i64>,
    #[serde(default)]
    pub friendship_turn: Option<i64>,
    #[serde(default)]
    pub denounce_time_limit: Option<i64>,
    /// `GetVisibilityOn` both ways — the `GameInfo.Visibilities` index
    /// (0 none … 4 top secret). Written to `Player::observed_visibility`,
    /// which [`crate::game::Game::diplomatic_visibility`] prefers.
    #[serde(default)]
    pub visibility: Option<i64>,
    #[serde(default)]
    pub their_visibility_on_us: Option<i64>,
    /// Open Borders WE grant them (`theirs:HasOpenBordersFrom(us)`).
    #[serde(default)]
    pub open_borders_granted: Option<bool>,
    /// Our Delegation / Resident Embassy at their court and theirs at ours
    /// (`HasDelegationAt` / `HasEmbassyAt`, the accessors the delegation
    /// actuator already calls).
    #[serde(default)]
    pub delegation_at: Option<bool>,
    #[serde(default)]
    pub embassy_at: Option<bool>,
    #[serde(default)]
    pub their_delegation: Option<bool>,
    #[serde(default)]
    pub their_embassy: Option<bool>,
    /// `PromiseTypes` member names (`DONT_SETTLE_NEAR_ME`, …) for promises we
    /// made to them and they made to us (`IsPromiseMade`,
    /// `DiplomacyActionView_AllianceRow.lua:61-70`). `Some(vec![])` is the
    /// host's "none"; `None` an older export.
    #[serde(default)]
    pub promises_made: Option<Vec<String>>,
    #[serde(default)]
    pub promises_received: Option<Vec<String>>,
    /// How many technologies this rival has finished, or `-1` if the host
    /// could not be asked.
    ///
    /// ⚠⚠ THIS IS THE FIELD THAT MAKES THE SCORE GAP READABLE. Over 99
    /// completed runs CIVVIS leads in NONE: our score is a median 267 against
    /// the best rival's 1109, a ratio of 0.26. But on empire size we are at
    /// 0.75-0.80 — 3 cities against 4, population 28 against 35 — and our
    /// cities are individually LARGER (10.3 pop against 9.4). So most of the
    /// gap is in components that are neither cities nor population, and until
    /// now the export could not say which.
    ///
    /// ⚠ `#[serde(default = "unknown_metric")]` is load-bearing. A new
    /// `StateRival` field without a default makes the WHOLE snapshot fail to
    /// deserialize on any older export, which silently loses the board — the
    /// failure documented on `map_or_empty_sequence`.
    #[serde(default = "unknown_metric")]
    pub techs: f64,
    /// Civics finished, or `-1` if unavailable. See `techs`.
    #[serde(default = "unknown_metric")]
    pub civics: f64,
    /// The rival's tree by NAME — every `TechnologyType`/`CivicType` it
    /// holds, from the same `HasTech`/`HasCivic` loops the counts above run.
    /// The names go onto the rival's seat (`Player::techs`/`civics`) exactly
    /// as the local seat's do (`apply_rival_tree`), so its era
    /// (`Game::player_era`), the units it can field and its border
    /// (`Game::enforces_borders` reads Early Empire off the civic tree) are
    /// derived natively; `enforces_borders` above stays the override. `None`
    /// on an older export; an empty list is a real "nothing yet".
    #[serde(default)]
    pub tech_names: Option<Vec<String>>,
    #[serde(default)]
    pub civic_names: Option<Vec<String>>,
    /// The shipped World Rankings overview's own lane numbers for this rival
    /// (`g_victoryData`, WorldRankings.lua:27,44,55): `GetNumTechsResearched`,
    /// `GetMilitaryStrengthWithoutTreasury` and `GetNumCitiesFollowingReligion`.
    /// `None` on an older export or a refused read.
    #[serde(default)]
    pub techs_researched: Option<i64>,
    #[serde(default)]
    pub military_no_treasury: Option<f64>,
    #[serde(default)]
    pub cities_following_religion: Option<i64>,
    /// The religion a majority of this rival's cities follow
    /// (`GetReligionInMajorityOfCities`, WorldRankings.lua:2049) — the test
    /// the shipped religion tab runs to call a civilization converted. `None`
    /// when no religion holds a majority or the host could not be asked.
    #[serde(default)]
    pub religion: Option<String>,
    /// Tourists visiting US from this rival — the culture tab's "Visiting us"
    /// column (`GetTouristsFrom` on the local player's culture,
    /// WorldRankings.lua:1766): the per-rival term of the top-level
    /// `foreign_tourists`.
    #[serde(default)]
    pub tourists_visiting_us: Option<i64>,
    /// The rival's Era Score (`GetPlayerCurrentScore`, the accessor the
    /// top-level `era_score` reads for us).
    #[serde(default)]
    pub era_score: Option<i64>,
    /// The rival's outgoing trade routes whose both ends stand on revealed
    /// ground, by endpoint. Seated on the board as routes the rival's seat
    /// owns when both cities are on it (`restore_rival_outgoing_routes`).
    #[serde(default)]
    pub trade_routes: Option<Vec<StateRivalRoute>>,
    /// Luxury resource types this met rival can currently offer in the
    /// shipped diplomacy deal screen that the seat itself does not own.
    /// `Some([])` is the host's authoritative answer that no useful luxury is
    /// available; `None` means an older control mod could not query the deal
    /// catalogue. The luxury-buy arm uses this to avoid selecting a rich
    /// rival whose deal has nothing the seat lacks, which otherwise wastes the
    /// six-turn purchase window.
    #[serde(default)]
    pub tradeable_luxuries: Option<Vec<String>>,
    /// Strategic quantities offered on this met rival's public trade screen.
    /// These are current available offers, not total stockpiles or income.
    /// Unknown API/deal data is `None`; a queried empty catalogue is `Some({})`.
    /// Lua emits an empty table as `[]`, which must remain a known empty map.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub tradeable_strategics: Option<BTreeMap<String, i64>>,
    /// Our Spies this rival holds captive: the CAPTIVE items the shipped deal
    /// screen lists on its side of the table (`CivvisCaptivesHeld`). A
    /// captured Spy keeps its slot in our capacity until it comes home, and
    /// the unit export drops it, so this is the only witness of those slots.
    /// `None` from an older mod or an unreadable deal.
    #[serde(default)]
    pub captives_of_ours: Option<u32>,
    /// The rival's opinion of us, reason by reason: the non-zero rows the
    /// shipped Intel tab lists under the relationship bar
    /// (`DiplomacyActionView.lua:895`, `GetDiplomaticModifiers`), each a LOC
    /// key and its score. The diplomatic state says a neighbour is
    /// Unfriendly; this says why. `None` from an older mod or an unreadable
    /// AI; `Some([])` is a rival with no reasons either way.
    #[serde(default)]
    pub opinion_modifiers: Option<Vec<StateOpinionModifier>>,
    /// The rival's economy as the host reports it — per-turn Science and
    /// Culture, Tourism, treasury and Faith balances and their per-turn rates —
    /// every one an accessor the shipped World Rankings and Deal screens call
    /// on other players. Before these crossed, the standings' rival Science
    /// and Culture were CIVVIS's own guess from the rival's visible cities.
    /// `-1` when the host could not be asked; NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub science: f64,
    #[serde(default = "unknown_metric")]
    pub culture: f64,
    #[serde(default = "unknown_metric")]
    pub tourism: f64,
    /// Space-race milestones the host reports for this rival — the rows the
    /// shipped World Rankings science screen lists for OTHER players
    /// (`GetNumProjectsAdvanced` per launch project). Five of the twelve runs
    /// this seat was leading on 2026-08-16/17 ended early on a rival's
    /// culture, technology or diplomatic victory the tracker read as zero
    /// progress. `None` means an older control mod could not export the list;
    /// an empty list is a real "no milestone yet". `#[serde(default)]` is
    /// load-bearing: without it any older export fails to deserialize whole.
    #[serde(default)]
    pub science_projects: Option<Vec<String>>,
    /// Civilization VI's exact World Rankings science-victory position and
    /// current movement. These are not inferred from repeatable laser project
    /// counts: a Terrestrial Laser Station only helps while it is powered.
    /// `-1` is a refused host read; NaN means an older export omitted the key.
    #[serde(default = "unknown_metric")]
    pub science_victory_points: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_per_turn: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_needed: f64,
    /// The culture victory's own two numbers as the shipped screen shows them
    /// for every major: tourists visiting this rival, and its staycationers
    /// (which set the bar every other civilization must clear). `-1` when the
    /// host could not be asked; NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub foreign_tourists: f64,
    #[serde(default = "unknown_metric")]
    pub domestic_tourists: f64,
    /// The engine's own turns until this major wins on Culture
    /// (`GetCulture():GetTurnsUntilVictory()`, WorldRankings.lua:1678): `-1`
    /// when it reports none, NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub culture_turns_to_victory: f64,
    #[serde(default = "unknown_metric")]
    pub gold: f64,
    #[serde(default = "unknown_metric")]
    pub gold_per_turn: f64,
    #[serde(default = "unknown_metric")]
    pub faith: f64,
    #[serde(default = "unknown_metric")]
    pub faith_per_turn: f64,
    /// Public empire totals for the player HUD. These never contain positions
    /// or identity of unseen cities, units, or wonders.
    #[serde(default)]
    pub public_stats: StatePublicEmpireStats,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
}

/// One met city-state. Its cities remain remembered after sight, while units
/// are exported only under current visibility just like a major rival's.
/// One endpoint pair of a rival's outgoing trade route, as
/// `rivals[].trade_routes` carries it. `-1` on a coordinate the host could
/// not read; the mirror skips such a route rather than guess.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRivalRoute {
    #[serde(default = "minus_one")]
    pub origin_x: i32,
    #[serde(default = "minus_one")]
    pub origin_y: i32,
    #[serde(default = "minus_one")]
    pub destination_x: i32,
    #[serde(default = "minus_one")]
    pub destination_y: i32,
    #[serde(default = "minus_one")]
    pub destination_player: i32,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateMinor {
    #[serde(default)]
    pub player: usize,
    #[serde(default)]
    pub civ: String,
    #[serde(default)]
    pub score: i64,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    #[serde(default)]
    pub at_war: bool,
    /// The host's CanGiveInfluence AND CanGiveTokensToPlayer result, not an
    /// inference from war status. None is an older or unreadable export.
    #[serde(default)]
    pub can_send_envoy: Option<bool>,
    #[serde(default = "minus_one")]
    pub suzerain: i32,
    /// Whether this city-state holds Early Empire and so enforces its border
    /// against everyone but its Suzerain (and whoever is at war with it).
    /// `None` on an older export — read as ENFORCED: run
    /// civvis-20260826T184456Z sent 122 military steps into non-suzerain
    /// city-state land and 4 % arrived, against 51 % where we were Suzerain.
    #[serde(default)]
    pub enforces_borders: Option<bool>,
    #[serde(default)]
    pub envoys: i64,
    #[serde(default)]
    pub most_envoys: i64,
    /// What this city-state is asking US for, per the host's QuestsManager
    /// (`HasActiveQuestFromPlayer` over `GameInfo.Quests()`, the shipped
    /// CityStates panel's read). One quest per pair in the shipped rules, so
    /// the first entry is the request. `None` on an older export leaves the
    /// board's own roll alone; `Some([])` is a city-state asking nothing.
    #[serde(default)]
    pub quests: Option<Vec<StateQuest>>,
    /// Every alive major's Envoy count at this city-state
    /// (`GetTokensReceived(player)`, CityStates.lua:1458), zeros included.
    /// `None` on an older export, where a rival's delegation is seeded as the
    /// minimum that elects the Suzerain the host names.
    #[serde(default)]
    pub envoys_by_player: Option<Vec<StateEnvoyCount>>,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
}

impl StateMinor {
    pub(super) fn is_free_cities(&self) -> bool {
        self.civ == "CIVILIZATION_FREE_CITIES"
    }

    pub(super) fn is_barbarian(&self) -> bool {
        self.civ == "CIVILIZATION_BARBARIAN"
    }

    /// A real city-state actor: not the Free Cities aggregate, not the
    /// barbarian seat. Public for the order bridge's seat resolution.
    pub fn is_city_state(&self) -> bool {
        !self.civ.is_empty() && !self.is_free_cities() && !self.is_barbarian()
    }

    pub(super) fn is_present_free_cities(&self, state: &StateSnapshot) -> bool {
        self.is_free_cities()
            && (!self.cities.is_empty()
                || !self.units.is_empty()
                || state
                    .hostiles
                    .iter()
                    .any(|unit| hostile_is_free_cities(state, unit)))
    }
}

pub(super) fn unknown_strength() -> f64 {
    -1.0
}

pub(super) fn unknown_metric() -> f64 {
    f64::NAN
}

fn minus_one_i64() -> i64 {
    -1
}

/// Accept `{}`, `[]`, `null`, or a populated object for a map-valued host field.
///
/// ⚠ **The mod's JSON encoder emits `[]` for an empty table.** `encode` counts a
/// table's entries and takes the array branch whenever `#v == n`, which an empty
/// table satisfies because both are zero. So a Lua field that is logically an
/// empty map arrives as a JSON *array*, serde refuses to read a sequence into a
/// `BTreeMap`, and the failure is not scoped to the field — **the entire
/// `StateSnapshot` fails to deserialize and the board is silently lost.**
///
/// That happened. `great_person_points` shipped in #983 without this, every
/// player has zero Great Person points on turn 1, and three consecutive live
/// attempts reported "no revealed terrain or no state yet" with 0 orders from
/// turn 1, stalled at turn 6 on an unanswered research prompt, and were killed
/// by the watchdog. The mod now returns `nil` when the table is empty; this is
/// the second half of that repair, so a future map-valued export cannot take the
/// board down the same way before anyone notices the encoder's behaviour.
fn map_or_empty_sequence<'de, D, T>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum MapOrSequence<T> {
        Map(BTreeMap<String, T>),
        /// Only ever empty in practice; a populated array has no key to carry.
        ///
        /// The payload is never read, and rustc offers to replace it with `()`
        /// to say so. **Taking that suggestion puts the outage above back.**
        /// In an untagged enum the field's *type* is the matcher: only
        /// `Vec<_>` accepts a JSON array, and `()` accepts `null` — which the
        /// `Null` variant below already claims. Change it and `[]` matches no
        /// variant, the whole `StateSnapshot` fails, and the board is lost
        /// again. The field earns its place by its type, not its value.
        #[allow(dead_code)]
        Sequence(Vec<serde_json::Value>),
        Null,
    }

    Ok(match MapOrSequence::deserialize(deserializer)? {
        MapOrSequence::Map(map) => Some(map),
        MapOrSequence::Sequence(_) => Some(BTreeMap::new()),
        MapOrSequence::Null => None,
    })
}

/// One native cost/progress quote for an unfinished technology.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateResearchQuote {
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub c: Option<f64>,
    #[serde(default)]
    pub p: Option<f64>,
}

/// The whole board as one `state` event described it.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateSnapshot {
    /// Confirmed combat deaths preceding this exported frame, never predicted kills.
    #[serde(skip)]
    pub confirmed_unit_deaths: Vec<HostUnitDeath>,
    pub turn: u32,
    /// 0 for the turn's opening board; N for the Nth mid-turn combat frame
    /// (`CivvisFrames` in the mod), on which the brain re-plans the same turn
    /// with the units' movement and attacks as they now stand.
    #[serde(default)]
    pub frame: u32,
    /// Civ 6 type names of COMPLETED research, e.g. `TECH_BRONZE_WORKING`.
    #[serde(default)]
    pub techs: Vec<String>,
    #[serde(default)]
    pub civics: Vec<String>,
    /// One-time strategic `PROJECT_*` types Firaxis says this seat has completed.
    ///
    /// This is deliberately not every project the player has ever run. District
    /// conversion projects are repeatable, but these are milestones that change
    /// what the planner should build next: the nuclear prerequisites and the
    /// space-race stages. `None` means an older control mod did not export the
    /// fact, while `Some([])` is an authoritative early-game answer.
    #[serde(default)]
    pub science_projects: Option<Vec<String>>,
    /// Exact science-victory readings from Civilization VI's World Rankings:
    /// current points, their active per-turn increase, and the target for this
    /// game speed. These make the live tracker truthful after the Exoplanet
    /// Expedition, including the actual effects of laser stations.
    /// `-1` is a refused host read; NaN means an older export omitted the key.
    #[serde(default = "unknown_metric")]
    pub science_victory_points: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_per_turn: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_needed: f64,
    /// Civ 6 type names whose **boost is triggered but which are NOT yet
    /// researched** — the eureka discount waiting to be collected.
    ///
    /// ⚠⚠ 62 of 77 technologies carry a boost worth 40-50% of their cost, and
    /// `AdvancedAi::tech_value` already pays +28 for a boosted tech — but until
    /// this field existed nothing ever sent the fact, so the live agent's
    /// `boosted_techs` was whatever its own simulation derived rather than what
    /// Civilization VI granted. Same class as the Amenity export (#967) and the
    /// Housing export (#1007): the valuation is right and the input is absent.
    ///
    /// `#[serde(default)]` so an older mod that sends neither still parses.
    #[serde(default)]
    pub boosted_techs: Vec<String>,
    #[serde(default)]
    pub boosted_civics: Vec<String>,
    /// The active Civilization VI technology and the accumulated beakers on it.
    /// Completed technologies alone do not tell the planner whether changing
    /// course discards a nearly finished choice.
    #[serde(default)]
    pub research: Option<String>,
    #[serde(default)]
    pub research_progress: f64,
    /// Own-seat native prices/progress for all unfinished technologies, not
    /// only the active selection. An older exporter leaves the list empty.
    #[serde(default)]
    pub research_quotes: Vec<StateResearchQuote>,
    /// Complete revealed prerequisite lists from the host's active tech graph.
    /// Missing on older exporters; hidden future nodes are never inferred here.
    #[serde(default)]
    pub research_tree: Option<Vec<StateResearchTreeRow>>,
    /// The active Civilization VI civic and its accumulated culture.
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub civic_progress: f64,
    /// Civ 6 type name of the government in force, e.g. `GOVERNMENT_OLIGARCHY`.
    ///
    /// ⚠ Nothing carried this and the consequence was not silent, only unread: 62
    /// `cannot_change_government` refusals in 96 turns of run
    /// `civvis-20260731T052021Z`, one every turn. CIVVIS's mirrored player had no
    /// government, and a player with no government asks for one. Policy slots hang
    /// off the government, so it was also choosing cards for a constitution it did
    /// not know it had.
    #[serde(default)]
    pub government: Option<String>,
    /// Every Firaxis government this seat has held at any point in the game,
    /// current one included. Returning to one of these costs Anarchy, and the
    /// engine charges it — but only through `past_governments`, which a board
    /// rebuilt fresh each turn never carries. Without this field the planner
    /// prices the return switch as free, proposes it plus its deck every turn,
    /// and the bridge guard vetoes the switch while the deck is refused: run
    /// civvis-20260815T012010Z logged 127 guard blocks and 15 deck-refusal
    /// turns this way. A plain list; empty is ordinary early game.
    #[serde(default)]
    pub used_governments: Vec<String>,
    /// Civ 6 belief type of the pantheon this seat has founded, if any.
    ///
    /// ⚠ Its absence was not silent, only unread: 125 `pantheon` orders in 173 turns,
    /// every one counted applied, against one pantheon. A seat that does not know it
    /// has a pantheon keeps choosing one — and is also missing that belief's yields
    /// from every calculation it makes.
    #[serde(default)]
    pub pantheon: Option<String>,
    /// Player-level religion facts, distinct from each city's majority religion.
    #[serde(default)]
    pub founded_religion: Option<String>,
    /// Native map coordinates of the Holy City; city IDs are player-local.
    #[serde(default)]
    pub holy_city: Option<[i32; 2]>,
    /// Unknown on older hosts; false is an observed unlaunched Inquisition.
    #[serde(default)]
    pub inquisition_launched: Option<bool>,
    /// Every non-pantheon religion founded worldwide. Firaxis exposes this in
    /// the Religion screen even when its founder has not otherwise been met.
    #[serde(default)]
    pub founded_religions: Vec<String>,
    #[serde(default)]
    pub religion_beliefs: Vec<String>,
    /// Every belief already claimed worldwide, including religions outside vision.
    #[serde(default)]
    pub taken_religion_beliefs: Vec<String>,
    /// Every founded religion with its founder and its own beliefs. Empty on
    /// an older control mod, when the union above is all the mirror has.
    #[serde(default)]
    pub religions: Vec<StateReligion>,
    #[serde(default)]
    pub prophet_pending: bool,
    /// Civ 6 policy types currently slotted, e.g. `POLICY_DISCIPLINE`.
    ///
    /// ⚠ Same shape as `government` and `pantheon`: a fact the game holds that CIVVIS
    /// was never told, so it re-decided it every turn. 73 `no_slot_for_*` refusals and
    /// 23 `already_*` in 61 turns of run civvis-20260731T070956Z.
    #[serde(default)]
    pub policies: Vec<String>,
    /// Current native eligibility, including government exclusives and bans.
    /// None is unknown (older mod); Some(empty) means no available cards.
    #[serde(default)]
    pub available_policies: Option<Vec<String>>,
    /// How many policy slots this government actually has. Choosing a card for a slot
    /// that does not exist is an uninformed decision, not a bad one.
    #[serde(default)]
    pub policy_slots: i64,
    #[serde(default)]
    pub gold: i64,
    /// Firaxis's own NET income, `GetGoldYield() - GetTotalMaintenance()` — the
    /// figure the shipped TopPanel prints beside the treasury.
    ///
    /// ★★★★★ THE EMPIRE GOES BANKRUPT WITHOUT NOTICING WITHOUT THIS.
    /// [`LiveMirror::mirror_net_income`] derives the rate from the treasury delta
    /// between CONSECUTIVE turns and keeps `last_treasury` on the mirror, but the
    /// bridge runs `civvis_orders --serve --fresh-board`, which rebuilds that
    /// mirror every turn — so the predecessor is never there and the rate never
    /// lands. Measured at **0.00 in 963 of 963 calls**.
    ///
    /// Live run `civvis-20260810T191050Z` (Rome/Trajan, Settler) is what that
    /// costs: the treasury peaked at 319 on turn 60, hit **0 on turn 110 and
    /// stayed there for the remaining 75 turns**. With the bankruptcy guard
    /// blind, the empire kept units it could not pay for, Civilization VI
    /// disbanded them (`army` 12 → 0), and the cities fell at t173, t180 and
    /// t184 — **six cities became two**, final score 403 against Mongolia's 747.
    /// Tech and civics were competitive the whole game (44 vs 46, 35 vs 34), so
    /// this single number is the gap.
    ///
    /// `None` when the host did not answer. A real `0.0` is break-even and a
    /// missing answer is not; conflating them is the failure above.
    #[serde(default)]
    pub gold_per_turn: Option<f64>,
    /// The bill behind that net, by source — `PlayerTreasury:GetUnitMaintenance`,
    /// `GetBuildingMaintenance`, `GetDistrictMaintenance`, the top panel's own
    /// breakdown (`ToolTipHelper_PlayerYields.lua:22-26`). `None` when the
    /// host did not answer, exactly like `gold_per_turn`; they reach
    /// `Game::host_maintenance` and replace the board's own sums there.
    #[serde(default)]
    pub unit_maintenance_total: Option<f64>,
    #[serde(default)]
    pub building_maintenance_total: Option<f64>,
    #[serde(default)]
    pub district_maintenance_total: Option<f64>,
    /// Native policy transaction window, distinct from which cards are
    /// available. Absence preserves legacy behavior; partial readings veto a
    /// changed transaction until the host confirms a free window.
    #[serde(default)]
    pub policy_change: Option<StatePolicyChange>,
    /// Faith balance. Unlike the rate fields below, this is a stockpile and
    /// crosses directly, exactly like gold.
    #[serde(default)]
    pub faith: i64,
    /// Civilization VI's own Faith PER TURN — `Player:GetReligion():GetFaithYield()`,
    /// the top bar's figure — applied like `science` and `culture` below: a
    /// correction on the reconstructed empire yield, never a replacement.
    ///
    /// Until it was exported the board could not be right by construction:
    /// on run civvis-20260816T123936Z the host banked 100–113 Faith a turn
    /// from t231 while every city together made 49 (the rest is the Faith
    /// paid for unused Great Person points, `unused_great_person_faith`), and
    /// the mirror had no host figure to measure that against. `None` on an
    /// older control mod.
    #[serde(default)]
    pub faith_per_turn: Option<f64>,
    /// The host's own Faith ledger, `GetFaithYieldToolTip` compacted — "+N from
    /// Cities / Beliefs / Envoys / city-states you are Suzerain of / Other" —
    /// so a gap between the two games is named, not guessed at.
    #[serde(default)]
    pub faith_sources: Option<String>,
    /// Civilization VI's own science and culture PER TURN.
    ///
    /// Applied as a correction to the reconstructed empire yield, not as a flat
    /// replacement. That makes the displayed and timing baseline exact while still
    /// letting a counterfactual policy or building contribute its modeled delta.
    /// The raw derived gap remains in the per-turn diagnostic so missing host rules
    /// stay measurable. On `civvis-20260801T024428Z` at turn 60 it was:
    ///
    /// | | Civilization VI | CIVVIS | drift |
    /// |---|---|---|---|
    /// | science | 5.80 | 8.6 | **+48%** |
    /// | culture | 7.08 | 8.9 | **+26%** |
    ///
    /// That matters because research VALUATIONS are spent in these units — CIVVIS
    /// rates a tech "worth 42 to the expansion plan" and times its plan against a rate
    /// half again too fast. An axis nothing reports does not exist; this makes the
    /// approximation's size visible on every turn so it can be tracked, and so any
    /// future decision to close it starts from evidence rather than from a guess.
    /// `None` on an older export; zero is a real yield during the unsettled
    /// opening and must not be mistaken for an unavailable answer.
    #[serde(default)]
    pub science: Option<f64>,
    #[serde(default)]
    pub culture: Option<f64>,
    /// Exact public empire totals for the active seat. Its city records remain
    /// the source of detailed state; this is the fog-safe aggregate shared with
    /// every rival so a standing never becomes zero merely because the mirror
    /// does not retain every city.
    #[serde(default)]
    pub public_stats: StatePublicEmpireStats,
    #[serde(default)]
    pub score: i64,
    /// Civilization VI's current Diplomatic Victory-point tally.
    ///
    /// The board is reconstructed before each live decision, so leaving this at
    /// `Player::default()` makes the strategy engine believe Rome has zero points
    /// even after the host has awarded some. `None` is deliberately distinct from
    /// zero: it preserves compatibility with an older loaded control mod without
    /// erasing a value already held by a persistent mirror.
    #[serde(default)]
    pub dvp: Option<i64>,
    /// Civilization VI's current stock of Diplomatic Favor.
    ///
    /// Favor decides how many World Congress votes the live seat can actually
    /// afford. It is a stock, not a yield, so deriving it from a reconstructed
    /// history loses the fact the host already knows. As with [`Self::dvp`],
    /// `None` means unavailable rather than an authoritative zero.
    #[serde(default)]
    pub favor: Option<f64>,
    /// 🔴🔴🔴 THE DIPLOMATIC STANDING THE SEAT IS SHOWN AND THE TRACKER NEVER SAW.
    ///
    /// [`StateRival`] is met-gated, so `players[*].dvp` only ever carried the
    /// civilizations this seat had contacted. The World Congress seats every
    /// alive major and shows the seat all of their points, and the host's own
    /// `WC_RES_DIPLOVICTORY` ballot names them as targets — which is why
    /// `voteWorldCongress` has always been free to pick its leader from the
    /// full set. That knowledge stopped at the ballot box.
    ///
    /// Measured over the 50 live runs carrying a congress table, **40 (80%)
    /// showed a DVP standing higher than any rival the decider could see**, and
    /// in five the difference crossed the denial alarm. In
    /// `civvis-20260818T103630Z` the eventual winner sat at 22 points while the
    /// best visible rival read 14, so `urgent_victory_threat` never fired once
    /// in 222 turns and the seat lost a game it led by 213.
    ///
    /// This is a congress-time snapshot, not a live read: it is what the last
    /// session showed, stamped with the turn it showed it, and it goes stale
    /// between sessions the same way a human's memory of the last vote does.
    #[serde(default)]
    pub congress_dvp: Option<StateCongressDvp>,
    /// How many Spies Civilization VI will let this empire field, from the
    /// accessor the shipped Espionage Overview prints
    /// (`GetDiplomacy():GetSpyCapacity()`).
    ///
    /// Without it `block_live_spy_production` had to refuse Spy production
    /// unconditionally, and the seat has therefore never held one: measured
    /// over twelve completed live games it finished holding the Diplomatic
    /// Service civic in **12 of 12** and fielded **zero** Spies. `None` means
    /// an older control mod could not say, which keeps the old blanket block.
    #[serde(default)]
    pub spy_capacity: Option<i64>,
    /// Our own culture-victory counters, same accessors as each rival's
    /// (`GetTouristsTo`/`GetStaycationers`): OUR staycationers are the bar
    /// every rival's visiting tourists must clear. `-1` when the host could
    /// not be asked; NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub foreign_tourists: f64,
    #[serde(default = "unknown_metric")]
    pub domestic_tourists: f64,
    /// The engine's own turns until this major wins on Culture
    /// (`GetCulture():GetTurnsUntilVictory()`, WorldRankings.lua:1678): `-1`
    /// when it reports none, NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub culture_turns_to_victory: f64,
    /// Our tourism per turn as the host reports it (`GetStats():GetTourism()`,
    /// the accessor already used for each rival). `None` on an older export.
    /// The mirror writes it into `Game::observed_tourism_per_turn`, and it is
    /// the live side of the `tourism` divergence row, which read "no pairs"
    /// on every run until it crossed.
    #[serde(default)]
    pub tourism_per_turn: Option<f64>,
    /// Cities anywhere following our religion, the religion lane's own
    /// number (`GetNumCitiesFollowingReligion`, WorldRankings.lua:44), the
    /// same accessor as each rival's. `None` on an older export or a refused
    /// read.
    #[serde(default)]
    pub cities_following_religion: Option<i64>,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    /// Era Score and the age it decides, from Firaxis's own `Game.GetEras()`.
    ///
    /// ★★★★★ CIVVIS MODELS THE WHOLE AGE SYSTEM AND THE BRIDGE CARRIED NONE OF IT.
    /// [`crate::game::Player`] has `era_score`, `era_score_baseline`,
    /// `normal_age_threshold`, `golden_age_threshold` and `dedications`, and
    /// `docs/AGES.md` records a row-by-row audit of all 143 scoring Moments
    /// behind them. On a reconstructed live board every one of those was left at
    /// whatever `Player::default` happens to say — era score 0 against a golden
    /// threshold of 26 — so the age CIVVIS reasoned about was not the age Rome
    /// was in.
    ///
    /// Two decisions read exactly these fields, so both ran on fiction live:
    /// `ai::choose_dedications` is gated on `dedication_choices` (which must
    /// cross from the host at an era boundary), and `ai/advanced.rs` filters
    /// `rules.policies[card].dark_age`, so a real Dark Age's wildcard cards were
    /// never slotted — the same shape as the housing and loyalty cards that are
    /// never slotted.
    ///
    /// `normal_age_threshold` is this codebase's name for the score at or above
    /// which the next age is Normal rather than Dark, which is exactly Civ 6's
    /// **Dark Age threshold**: one boundary, named from opposite sides.
    ///
    /// `None`/negative means the host did not answer. A real 0 era score is
    /// ordinary on turn 1 and must not read as "unknown".
    #[serde(default)]
    pub era_score: Option<i64>,
    #[serde(default)]
    pub era_score_baseline: Option<i64>,
    #[serde(default)]
    pub normal_age_threshold: Option<i64>,
    #[serde(default)]
    pub golden_age_threshold: Option<i64>,
    /// Firaxis's world era index, which advances on the field rather than on
    /// this empire alone.
    #[serde(default)]
    pub world_era: Option<i64>,
    /// `Game.GetEras():GetNextEraCountdown()`: the turns left AFTER this one in
    /// the world era (0 on its last turn), negative before the countdown has
    /// started. `None` on an older export. The model's
    /// `Game::world_era_countdown_end` is derived from it; without it that
    /// deadline existed only in the simulator, so `age-closer-2` never fired live.
    #[serde(default)]
    pub era_countdown: Option<i64>,
    #[serde(default)]
    pub dark_age: Option<bool>,
    #[serde(default)]
    pub golden_age: Option<bool>,
    #[serde(default)]
    pub heroic_golden_age: Option<bool>,
    /// The Dedications (Commemorations) the host says this seat has active,
    /// as `COMMEMORATION_*` type names — what a Golden Age PAYS. `None` on an
    /// older export; an empty list is a seat with none.
    #[serde(default)]
    pub dedications: Option<Vec<String>>,
    /// How many dedication choices the host currently permits. `None` means an
    /// older export; a real zero means the seat has no pending era choice.
    #[serde(default)]
    pub dedication_choices: Option<i64>,
    /// Observed activity in the preceding native era, reconstructed from this log.
    /// Missing history stays unknown; it is never fabricated from the current board.
    #[serde(skip)]
    pub observed_dedication_activity: Option<BTreeMap<String, i64>>,
    /// The World Congress resolutions binding this turn (`GetResolutions`),
    /// mapped onto the model's own `active_congress_effects`. `None` on an
    /// older export leaves the model's Congress alone; `Some([])` is a world
    /// with nothing in effect.
    #[serde(default)]
    pub resolutions: Option<Vec<StateResolution>>,
    /// The host's active World Congress emergency tracker. `None` means an
    /// older control mod did not export it; `Some([])` is an authoritative
    /// statement that no live competition remains.
    #[serde(default)]
    pub emergencies: Option<Vec<StateEmergency>>,
    /// Turns until the next regular session — how long `resolutions` stay
    /// binding (`GetMeetingStatus().TurnsLeft`).
    #[serde(default)]
    pub congress_turns_left: Option<i64>,
    /// Gathering Storm's climate, off `GameClimate` the way the shipped
    /// ClimateScreen reads it. `None` on an older export or a ruleset without
    /// the object, where the board's own phase stands.
    #[serde(default)]
    pub climate: Option<StateClimate>,
    /// Where a Trader could go from each of our cities and what the host says
    /// each route would pay its origin, exported while a route slot is open
    /// (the 12 richest per origin). `None` when nothing can be started or on
    /// an older export.
    #[serde(default)]
    pub route_options: Option<Vec<StateRouteOption>>,
    /// Firaxis's own outgoing-route capacity. The model can differ because a
    /// mirrored empire does not reproduce every capacity modifier.
    #[serde(default)]
    pub trade_capacity: Option<i64>,
    /// Envoys we are HOLDING and have not placed, per Firaxis's own
    /// `GetTokensToGive`.
    ///
    /// ★★★★★ `minors[].envoys` has always said where our envoys LANDED. Nothing
    /// ever said how many were sitting unspent, and that single omission closed
    /// the whole axis: [`crate::game::Game::legal_actions`] gates
    /// `Action::SendEnvoy` behind `if p.envoys_free > 0`, and while `envoys_free`
    /// was not mirrored every reconstructed live board read 0 and **CIVVIS
    /// never enumerated sending an envoy at all**. That is why `SendEnvoy`
    /// appeared nowhere in the skipped-action tally, while `LevyMilitary` — which
    /// needs a suzerainty we never hold — appeared there 44 times.
    ///
    /// Measured over 36 live runs past turn 150: median envoys placed **1**,
    /// median suzerainties **0**, 16 of 36 runs ending with none placed anywhere.
    /// CIVVIS prices the payoff in full (`envoy_type_yields_for_count` pays a
    /// cultural city-state at the 1/3/6 thresholds), so the sim collects it and
    /// the live game collects nothing.
    ///
    /// **Now MIRRORED onto `Player::envoys_free`** (see
    /// [`apply_mirrored_envoys_free`]), so the deployed `advanced_envoys` pass
    /// enumerates and prices `SendEnvoy` on the live board exactly as it does
    /// natively, and `civvis_orders` translates each one into an `envoy` order
    /// the mod actuates. It was carried-and-reported only, for the crash the
    /// Lua `chooseEnvoy` lane was blamed for; that lane's own record since —
    /// the stale-handle write fixed, the governor-appointment prompt found to
    /// share the crash signature and the t44–47 cluster, and a 250-turn
    /// `EnvoyEnabled` run that placed 113 envoys without a fault — is why the
    /// board is allowed to want them now. The Lua chooser stays off; the
    /// decision is CIVVIS's, the mod only places what it is told to.
    ///
    /// `None`/negative means the host did not answer; a real 0 means we are
    /// genuinely holding none, and the two must not read the same.
    #[serde(default)]
    pub envoys_free: Option<i64>,
    /// Great Person POINTS by Civilization VI class type, e.g.
    /// `GREAT_PERSON_CLASS_SCIENTIST`. The points, not the people: the earned
    /// individuals already arrive as units.
    ///
    /// `district_project_value` prices every district project against the live
    /// Great Person race — how close this empire is to the next one of that
    /// class, and how close the leading rival is. Without this the race is all
    /// zeros in every live game, so the Campus project's entire reason to exist
    /// (Great Scientist points) is invisible to the planner that chooses it.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_points: Option<BTreeMap<String, f64>>,
    /// The same classes' points PER TURN (`GetPointsPerTurn`), the host's own
    /// figure for what the districts, buildings, wonders, policies and
    /// Governors add each turn. Nothing plans on it yet; it is exported so the
    /// Faith Firaxis pays for an unrecruitable class can be checked against
    /// the host's rate rather than the model's.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_points_per_turn: Option<BTreeMap<String, f64>>,
    /// The classes with nobody left to recruit anywhere on the host's
    /// timeline (`GREAT_PERSON_CLASS_SCIENTIST` once the last Great Scientist
    /// is claimed by anyone). Their points are what Firaxis pays out as
    /// Faith. An empty list is a real answer — everyone still available;
    /// `None` is an older control mod, and the cost map stands in.
    #[serde(default)]
    pub great_person_exhausted: Option<Vec<String>>,
    /// The seat's strategic stockpiles by host resource type
    /// (`RESOURCE_IRON` → amount). Without them `Game::strategic_stockpile`
    /// read 0 for everything on the live seat: no unit that costs a strategic
    /// resource was ever producible, and no unit was ever obsolete for want of
    /// a buildable successor. See `apply_strategic_stockpiles`.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub strategic_resources: Option<BTreeMap<String, f64>>,
    /// Our luxury copies by host resource type (`RESOURCE_MARBLE` → copies),
    /// the host's own count net of copies a deal has traded away; absent
    /// when none is held or on an older export. The mirrored board counts
    /// improved tiles and cannot see a copy that is out on a deal, so the
    /// gun-resource barter (`siege-buys-the-gun-resource`) caps its spare
    /// copies by this.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub luxury_counts: Option<BTreeMap<String, f64>>,
    /// Gross host strategic income: accumulation + imports + bonuses, before
    /// unit or power demand. Missing entries retain the modeled estimate.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub strategic_resource_income: Option<BTreeMap<String, f64>>,
    /// The live RECRUIT COST of each class's current unclaimed Great Person,
    /// by the same class-type key. Points without costs sent the planner's
    /// threshold check to CIVVIS's own market formula, which quoted 60-ish
    /// where the live timeline wanted hundreds: run civvis-20260815T033823Z
    /// recorded 45 `gp_cannot_recruit` refusals — the recruit order finally
    /// crossed the bridge (#1596) only for the live game to answer "not yet"
    /// every time, because the ask itself was priced against the wrong game.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_costs: Option<BTreeMap<String, f64>>,
    /// The named Great Person currently offered by Firaxis for each class,
    /// including the hard prerequisites the class label cannot express.
    ///
    /// A Great Scientist is not necessarily usable at a Campus: Hildegard of
    /// Bingen requires a Holy Site, while Mary Leakey requires a Theater
    /// district and an Archaeological Museum slot. James of St. George also
    /// requires the Castle building to be missing because his activation adds
    /// the Medieval Walls. The planner previously read only the class and
    /// could spend a whole Campus-project race on an individual it had no
    /// possible way to activate. Carry the host's three exact prerequisite
    /// columns without attempting to recreate every named effect in CIVVIS's
    /// ruleset.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_offers: Option<BTreeMap<String, StateGreatPersonOffer>>,
    /// Total Governor Titles obtained and spent according to Firaxis. These are
    /// separate from the roster because a title can be held unspent.
    #[serde(default)]
    pub governor_points: Option<i64>,
    #[serde(default)]
    pub governor_points_spent: Option<i64>,
    /// Authoritative appointed roster. `None` means unknown; `Some([])` means empty.
    #[serde(default)]
    pub governors: Option<Vec<StateGovernor>>,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
    /// Routes currently travelling for this seat.  The control mod gets these
    /// from each of our cities' `GetOutgoingRoutes()`; routes belonging to a
    /// rival remain hidden just as their unseen units do.
    #[serde(default)]
    pub trade_routes: Vec<StateTradeRoute>,
    #[serde(default)]
    pub rivals: Vec<StateRival>,
    #[serde(default)]
    pub minors: Vec<StateMinor>,
    /// Who this seat actually is, from the run's `seat` event.
    ///
    /// ⚠ Not part of the `state` event — [`state_from_events`] merges it in, so
    /// every existing caller gets identity without changing its signature.
    #[serde(default)]
    pub seat: Seat,
    /// Sites Civilization VI has refused to found on, AXIAL, from `found_refused`
    /// events. Merged in by [`state_from_events`] for the same reason as `seat`.
    #[serde(default)]
    pub refused_sites: std::collections::BTreeSet<crate::Pos>,
    /// Tiles Civilization VI refused to improve, AXIAL, from `improve_refused`
    /// events that do not name the improvement.
    #[serde(default)]
    pub refused_improves: std::collections::BTreeSet<crate::Pos>,
    /// Improvements Civilization VI refused, by AXIAL tile, from
    /// `improve_refused` events that name one (`want`). See
    /// `Game::blocked_improvements`.
    #[serde(default)]
    pub refused_improvement_names:
        std::collections::BTreeMap<crate::Pos, std::collections::BTreeSet<crate::name::Name>>,
    /// Promotions the host refused, per Civilization VI unit id.
    /// See `Game::blocked_promotions` for why this exists.
    ///
    /// ⚠ `#[serde(default)]` is LOAD-BEARING: this is merged in by
    /// [`state_from_events`] and never appears in the host's `state` JSON, so
    /// without it the whole `StateSnapshot` fails to deserialize and the board is
    /// silently lost — the failure documented on [`map_or_empty_sequence`]. Adding
    /// this field without the attribute took a replay from 4,339 orders to 0.
    #[serde(default)]
    pub refused_promotions: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Non-air strikes the host refused THIS TURN, as `(unit, x, y)` in Civilization VI
    /// ids and offset plots, from `range_attack_refused` and `war_refused`.
    /// Per turn, not cumulative: a shot the host refused for line of sight last
    /// turn may be open after a move, and a war refusal is answered by the
    /// diplomacy arm, not by never striking that plot again.
    /// Air missions use the orders bridge's repeated-refusal cooldown instead.
    /// See `Game::blocked_strikes`.
    ///
    /// ⚠ `#[serde(default)]` is load-bearing here for the same reason as above.
    #[serde(default)]
    pub refused_strikes: std::collections::BTreeSet<(i64, i32, i32)>,
    /// The host's answers to this turn's `preview` orders, keyed
    /// `(unit, x, y, verb)` in Civilization VI ids and offset plots. Per turn:
    /// a simulation is a reading of the board as it stood when it was asked.
    /// See `Game::host_previews`.
    ///
    /// ⚠ `#[serde(default)]` is load-bearing here for the same reason as above.
    #[serde(default)]
    pub host_previews:
        std::collections::BTreeMap<(i64, i32, i32, String), crate::game::HostStrikePreview>,
    /// Origin/destination pairs Firaxis rejected for trade-route pathing.
    #[serde(default)]
    pub refused_trade_routes: std::collections::BTreeSet<(crate::Pos, crate::Pos)>,
    /// Policy cards Civilization VI has retired, as its OWN names, harvested from the
    /// `obsolete_<POLICY>` refusal reasons already in the stream. Translated where the
    /// ruleset is in hand; see [`refused_policies`].
    #[serde(default)]
    pub refused_policy_names: std::collections::BTreeSet<String>,
    /// Pantheon beliefs Civilization VI refused as already taken by another
    /// player, as its OWN names, harvested from the `taken_<BELIEF>` refusal
    /// reasons already in the stream. Translated where the ruleset is in hand;
    /// see [`refused_pantheons`] and `Game::blocked_pantheons`.
    #[serde(default)]
    pub refused_pantheons: std::collections::BTreeSet<String>,
    /// Districts Civilization VI refused to place, by ITS city id, from
    /// `build_no_plot`. Mapped onto CIVVIS cities where `city_ids` is in hand; see
    /// [`refused_districts`].
    #[serde(default)]
    pub refused_districts: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Fresh, host-approved alternatives for a district CIVVIS asked to place
    /// on the wrong tile.  Like [`StateSnapshot::refused_districts`], these
    /// arrive through `build_no_plot` rather than the state payload; unlike a
    /// refusal, their coordinates are a positive, short-lived fact that lets
    /// the next decision name a plot Firaxis has already approved.
    #[serde(default)]
    pub host_district_sites: BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>>,
    /// Fresh, host-approved alternatives for a wonder CIVVIS asked to place on
    /// the wrong tile. They use the same short-lived `build_no_plot` evidence as
    /// districts, but the Firaxis event identifies wonders under `building`.
    #[serde(default)]
    pub host_wonder_sites: BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>>,
    /// Wonders Civilization VI refused to place, by ITS city id, from the same
    /// `build_no_plot` event. Kept apart from [`StateSnapshot::refused_districts`]
    /// because the mod reports a refused wonder under `building` and a refused
    /// district under `district`, and the two translate against different rulesets.
    #[serde(default)]
    pub refused_wonders: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// World-unique wonders a `build_no_plot` answer ruled out on every host
    /// location. This is deliberately separate from [`StateSnapshot::refused_wonders`]:
    /// a city-local refusal can be a terrain mismatch, while an explicit
    /// `offered: 0` is the host saying the requested wonder has no target at all.
    #[serde(default)]
    pub host_unavailable_wonders: std::collections::BTreeSet<String>,
    /// Production items the host has recently reported as unplayable, by its city id.
    /// These are translated and applied as a cooldown rather than a permanent ban.
    #[serde(default)]
    pub refused_production: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Purchases Civilization VI recently rejected, by its city id. Kept apart
    /// from production refusals so a failed purchase causes a build fallback
    /// instead of suppressing that build as well.
    #[serde(default)]
    pub refused_purchases: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Barbarian units this seat can SEE.
    ///
    /// ★★★★ The rival export is built from `GetAliveMajorIDs`, so barbarians could
    /// never appear in it and could never show `at_war`. A city lost to them read as
    /// "lost at peace with everyone", which is how the analysis of how cities are
    /// lost was made with an instrument blind to the likeliest cause.
    #[serde(default)]
    pub hostiles: Vec<StateUnit>,
    /// Raw export keys that this adapter does not recognize. These remain
    /// non-fatal, but flow into `unmapped` so adding a Lua field can never again
    /// look like successful mirroring while serde silently discards it.
    #[serde(skip)]
    pub schema_gaps: Vec<String>,
}
