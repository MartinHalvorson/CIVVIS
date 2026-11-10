//! Scripted AIs (mirrors civvis/ai/). BasicAi reads full state (no fog) —
//! sparring partner, not a fair-play agent.
pub mod finishing;
pub mod player;
use crate::game::{
    effective_strength, expected_damage, Action, ActionFamilies, Game, Item, PolicyReadSet,
    TraversalClass,
};
use crate::name::{AsName, Name};
use crate::parallel::WorkPool;
use crate::reasoning::{plain, Journal};
use crate::rng::Rng;
use crate::think;
use crate::Pos;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// A bounded first-step initiative bonus breaks positional and formation ties
/// in favor of doing something useful with the turn. Four points can overcome
/// a couple of lost adjacency bonuses, but not the much larger penalty for
/// stepping into a dangerous attack envelope.
const FIRST_MOVE_SCORE_BONUS: f64 = 4.0;

/// A projected naval blow below ten hit points is nuisance fire rather than a
/// reason to recall the army or preempt a city's queue. The threshold is read
/// only by the opt-in `naval-threat-triage` treatment; every historical
/// controller continues to count every nearby raider.
const SERIOUS_NAVAL_DAMAGE: f64 = 10.0;

/// How many turns of a unit's recent whereabouts to keep. A livelock is not
/// visible in one decision — every individual step looks like the best one
/// available — so it can only be recognized from a unit's own recent past.
const LIVELOCK_WINDOW: usize = 6;

/// A unit whose whole recent past fits in this many tiles has not gone
/// anywhere. Three allows a genuine three-tile shuffle around an obstacle to
/// be recognized as one, while any unit on a real march leaves the footprint
/// within two turns and is never considered again.
const LIVELOCK_FOOTPRINT: usize = 3;

/// What leaving a proven-fruitless footprint is worth to a unit that is
/// circling inside it, in the same units the tactical scorers use. Two hexes
/// of positional error is a price worth paying to get out of a loop; walking
/// into an even fight, at roughly fifteen points of threat, is not — so this
/// redirects a stuck unit without ever ordering it to its death.
pub(crate) const LIVELOCK_ESCAPE_VALUE: f64 = 8.0;

/// `unique-unit-preference`: the strength `best_military` credits our own
/// unique unit for the abilities its strength column omits. Five is the same
/// credit `decisive-window` gives it (`decisive_window::UNIQUE_MARGIN`).
pub(crate) const UNIQUE_UNIT_POWER_CREDIT: f64 = 5.0;

/// See `BasicAi::lent_military_floor_base`: the most standard turns a city may
/// spend training toward a lent war target above the genome's own floor.
const LENT_FLOOR_MAX_BUILD_TURNS: u32 = 16;
/// See `BasicAi::front_weighted_floor`: how much sooner, in standard turns,
/// another city must put the floor's unit at the front for this one to skip it.
/// Two: in 160213Z the floor's median city stood 15 tiles from the objective
/// and the near city 4-8, which for Gran Colombia's three-move infantry
/// (Ejército Patriota, +1) is only two or three turns of walking.
const FRONT_ARRIVAL_MARGIN: u32 = 2;
/// See `BasicAi::front_weighted_floor`: below this share of its floor the
/// army is built wherever a queue is free.
const FRONT_FLOOR_SHARE: f64 = 0.75;
/// See `BasicAi::front_weighted_floor_2`: how close to the campaign's target
/// a city must stand to build the floor's unit ahead of its economy.
const FRONT_NEAR_TILES: i32 = 8;

/// The gates of `BasicAi::pick_item`'s Settler step; see
/// `BasicAi::settler_gates`.
/// Turns between one city's `journal_settler_hold` lines: a sample of every
/// game for the census without a line per city per turn.
const SETTLER_HOLD_JOURNAL_STRIDE: u32 = 3;

struct SettlerGates {
    room: bool,
    stranded: usize,
    pipeline: usize,
    parked_brake: bool,
    none_in_flight: bool,
    grown: bool,
    in_window: bool,
}

/// After this many fruitless turns the tabu has had every chance to work and
/// has not: whatever the unit is trying to reach, it cannot. Standing it down
/// is strictly better than another lap — it fortifies, heals, and stops
/// paying for a route search that keeps returning the same answer.
const LIVELOCK_STAND_DOWN_AFTER: u32 = 2 * LIVELOCK_WINDOW as u32;

/// Long enough for the neighbours, borders, and enemies that produced the
/// loop to have moved on before the unit tries again.
const LIVELOCK_STAND_DOWN_TURNS: u32 = 4;

/// Turns a unit may stand on one tile aiming at one exploration goal before the
/// goal is written off as ground the host will not walk it to. See
/// `BasicAi::explore_dead_targets`.
const EXPLORE_STUCK_TURNS: u32 = 2;

/// Turns an explorer may pursue one goal without coming any closer to it
/// before the goal is retired with a wider ring (`EXPLORE_NO_PROGRESS_RING`).
/// Live King 2026-10-01T000033Z: the Scout held goal (27, 14) from turn 13,
/// seven tiles off, and circled four tiles beside it for thirty turns -- a
/// cycle neither the same-tile count nor a two-tile bounce check can see --
/// while the Cree capital lay unseen to the east until turn 62.
const EXPLORE_NO_PROGRESS_TURNS: u32 = 3;

/// The ring retired around a goal the explorer could not close on. A goal
/// behind an obstacle has neighbours behind the same obstacle; one ring
/// hands the next pick a goal two tiles away and the same circuit.
const EXPLORE_NO_PROGRESS_RING: i32 = 2;

/// A retired exploration goal marks a bearing the host would not walk the
/// explorer along; a fresh candidate within sixty degrees of it, seen from
/// the explorer, is charged this much in the ranking (reveal is worth four a
/// tile, so sixty is fifteen unseen tiles). The 2026-10-01T000033Z Scout gave
/// up (27, 14) and then chose (20, 14), (20, 12) and (23, 14), all north
/// across the same lake: a distance shadow around the first goal cannot
/// reach goals seven tiles along the same far shore, a bearing can.
const EXPLORE_FAILED_BEARING_PENALTY: i32 = 60;
/// Only retired tiles at least this far from the explorer mark a bearing; a
/// retired ring beside the unit says nothing about direction.
const EXPLORE_FAILED_BEARING_MIN_DISTANCE: i32 = 3;

/// `explore_last`'s record: (goal, tile, same-tile turns, last turn, closest
/// distance to the goal, turns without coming closer).
type ExploreProgress = (Pos, Pos, u32, u32, i32, u32);

/// Consecutive turns the same issued step must be seen untaken before
/// `live-move-refusal-break` bars it. One refused turn is a report, not a
/// verdict — the trade-route block set's rule — and the second identical ask
/// against an unmoved unit is the host answering the same question the same
/// way twice.
const MOVE_REFUSAL_STRIKES: u8 = 2;

/// Standard turns a proved-refused step stays barred. Long enough for the
/// blocker the host saw — a unit in the way, a border, an embark rule the
/// mirror mis-guessed — to change, short enough that a transient one does not
/// disfigure routes for an era.
const MOVE_REFUSAL_BLOCK_TURNS: u32 = 8;
/// `band-plays-what-it-can-reach`: standard turns a host's venues are skipped
/// after it walked one of our bands nowhere. The rule's own blocker (Music
/// Censorship, a war, a closed passage) rarely lifts sooner.
const BAND_REFUSAL_BAR_TURNS: u32 = 8;
/// `band-plays-what-it-can-reach`: venues a band tries to step toward in one
/// call before it settles for the stage under it.
const BAND_TOUR_TRIES: usize = 6;
/// The longest trip, in turns at full movement, the career-priced tours take.
const ROCK_BAND_TOUR_MAX_TURNS: f64 = 15.0;
/// How long a written-off exploration goal stays retired. See
/// `BasicAi::explore_dead_targets`.
const EXPLORE_DEAD_TARGET_TURNS: u32 = 40;
/// How many turns a committed exploration goal is held before it is chosen
/// afresh, if it is not reached, revealed or written off first. See
/// `BasicAi::explore_commit`.
const EXPLORE_COMMIT_TURNS: u32 = 20;
/// How many rings past the first fogged one a committed explorer normally
/// looks when it picks a goal — twice the stock lookahead, because a goal
/// within a scout's own sight of the fringe is revealed by the walk toward it
/// and re-chosen next turn, which is the pacing this replaces. See
/// `BasicAi::explore_commit`.
const EXPLORE_COMMIT_LOOKAHEAD: i32 = 8;
/// An empire that still has city-states to meet needs one genuine expedition,
/// not another pass around its nearest fogged ring. This is about seven Scout
/// turns on open land — enough to cross a normal continent or use a coastal
/// passage — while the twenty-turn commitment still leaves room to respond to
/// a revealed danger. See `BasicAi::exploration_goal`.
const EXPLORE_CONTACT_SWEEP_LOOKAHEAD: i32 = 20;
/// The number of highest-value long-range contact targets whose actual routes
/// are checked before retaining one. A known closed border can make the
/// geometrically best horizon unreachable to a Scout; trying a bounded set
/// keeps that fact from turning a live expedition into a blind twenty-turn
/// commitment. If they are all blocked, the ordinary local commitment takes
/// over.
const EXPLORE_CONTACT_SWEEP_ROUTE_PROBES: usize = 24;
/// Another explorer's committed goal keeps this one out of the ground around
/// it, so two scouts fan out instead of shaving the same fringe. See
/// `BasicAi::explore_commit`.
const EXPLORE_COMMIT_SEPARATION: i32 = 4;
/// A visible hostile this close to a goal takes the goal off the table — a
/// scout that has just slipped out of a barbarian's reach must not be sent
/// straight back at the same fog behind it. See `BasicAi::explore_commit`.
const EXPLORE_COMMIT_THREAT_RADIUS: i32 = 3;

/// A sighting that makes the next tile a bad commitment is useful for longer
/// than the individual movement choice that discovered it. Keep it just long
/// enough for a unit to withdraw and reconsider from a safer square; longer
/// would make a vanished enemy freeze an otherwise healthy advance.
const UNIT_DANGER_MEMORY_TURNS: u32 = 3;

/// A unit that decides an approach would leave it below its withdrawal floor
/// gets two whole turns to create distance. The objective remains intact, so
/// this is a pause to survive rather than a silent abandonment of the campaign.
const UNIT_RETREAT_TURNS: u32 = 2;
/// `veterans-withdraw-early`: hit points a unit keeps in hand per promotion.
pub(crate) const VETERAN_HP_PER_PROMOTION: i32 = 10;
/// `veterans-withdraw-early`: the most a veteran keeps in hand (three promotions).
pub(crate) const VETERAN_HP_MARGIN_CAP: i32 = 30;
/// `ranged-hp-reserve`: hit points a ranged unit keeps in hand against the
/// lethal pool. An archer's value is the shots it takes over many turns; a
/// melee unit's is the blow it lands now. So the archer withdraws from a tile
/// where the enemy's exact next-turn attacks would leave it THIS close to
/// dead — and comes back to shoot again — where a warrior of the same health
/// stays and trades. Siege pieces are excluded: they must sit in reach of the
/// walls they exist to breach.
///
/// ⚠ 20, NOT 35. Measured in the archer-under-archer fixture: an even archer
/// volley lands for ~70, so a 35-point reserve read every duel as a loss
/// (65 left after our own shot + 35 ≥ 100) and the archer would never stand
/// anywhere a peer could reach it — no shots, no war. Twenty lets it trade at
/// full health and break off early instead of trading down.
pub(crate) const RANGED_HP_RESERVE: i32 = 20;

/// `game::damage` rolls every blow at `uniform(0.8, 1.2)` around the centre
/// this controller prices with, so the average is not the number a survival
/// question wants: a unit that lives through the mean still dies on the good
/// rolls. `one_shot_recovery` reads the top of the roll instead.
pub(crate) const COMBAT_ROLL_MAX: f64 = 1.2;

/// The engine's own ceiling on one blow (`game::damage` clamps to it), so a
/// roll scaled to its top cannot claim more damage than the game can deal.
const MAX_SINGLE_BLOW: f64 = 100.0;

/// Unlevied city-state forces defend the state and its immediate approaches;
/// ownership transfers to the Suzerain while levied, so those units naturally
/// use the major civilization's unrestricted tactical doctrine instead.
/// What a land unit gives up by taking its march across water instead of around.
///
/// `tactical_step`'s score is distance-to-objective plus adjacent threat and
/// support, with no terrain term — and a sea tile is both closer to an
/// objective across a bay and free of the adjacent-enemy penalty, because the
/// enemies stand on land. Open water therefore scored as the fastest and
/// safest road simultaneously. An embarked unit cannot attack, cannot fortify,
/// and defends at the era's flat `embarked_strength`, so the true cost of that
/// tile is most of the unit.
///
/// Ships fixed rather than as a gene: the genome is pinned at 40 with a
/// committed champion on that length, so `from_vec` indexing `v[40]` would
/// panic. Earn a gene on evidence.
pub(crate) const WATER_MARCH_PENALTY: f64 = 18.0;

/// Probe: how many times the deployed controller reaches `garrison_step`.
pub static GARRISON_STEP_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

const MINOR_DEFENSE_RADIUS: i32 = 6;
/// How far a major's soldier walks to take back a barbarian-held settler
/// under `barbarian-settler-capture`: two turns for a two-movement unit,
/// which is the last turn the walk is still cheaper than the settler.
pub(crate) const BARBARIAN_SETTLER_PURSUIT_RADIUS: i32 = 4;

/// How near one of our own cities an enemy has to stand before the empire owes
/// it an answer. Six is the same ring `nearest_enemy` already calls "near home"
/// when it decides whether to chase a barbarian, so the two agree on the word.
pub(crate) const HOME_THREAT_RADIUS: i32 = 6;
/// How far from a city a barbarian CAMP still counts as home ground under
/// `camp_reach`: Civilization VI's camps raise raiders toward cities well
/// beyond the six-tile raider radius, and a camp is not a raider — it does
/// not fight back, it keeps producing what does. See `camp_reach`.
pub(crate) const HOME_CAMP_RADIUS: i32 = 9;

/// The loyalty level the governor logic has always treated as an emergency.
/// Retained as a floor so making the rule rate-aware never makes it blinder
/// than it was.
/// `bleeding-capital-loyalty`: an ordinary Governor's establishment turns
/// (`data/governors.json`; Victor's are three). A Loyalty runway shorter
/// than this flips the city before that Governor's +8 arrives.
pub(crate) const VICTOR_SHORT_RUNWAY_TURNS: f64 = 5.0;
const LOYALTY_LEVEL_ALARM: f64 = 70.0;

/// The share of the army a bounded barbarian response may claim. Half, because
/// recalling everything to chase raiders loses the same game a different way.
const BARBARIAN_RESPONSE_MAX_SHARE: f64 = 0.5;

/// Answer a threat with this much more strength than it carries, so the units
/// sent are sent to win rather than to trade evenly and leave a wounded raider
/// healing on our ground.
const BARBARIAN_RESPONSE_MARGIN: f64 = 1.25;

/// A defender ten tiles from the threat spends five turns walking and arrives
/// after the damage. Past this range the unit keeps its offensive job.
const BARBARIAN_RESPONSE_RECALL_RANGE: i32 = 10;

/// How many camp-errand claims `camp_bounty` may hold at once. Two hunters
/// convert the opening's nearby camps without ever amounting to an army
/// diversion — the measured failure mode of the withdrawn major-war
/// garrison. Deliberately NOT a gene: the genome is pinned at 40.
const CAMP_BOUNTY_PARTY: usize = 2;

/// How much cheaper a barbarian is to attack than the same fight against a
/// major. See `barbarian_bargain`. Eight points is the size of the whole
/// `strength_drive` term, so it moves a marginal trade without ever inverting
/// the Recon penalty (+14) that keeps scouts out of fights.
pub(crate) const BARBARIAN_BARGAIN_DISCOUNT: f64 = 8.0;

/// A raider may operate this far from the camp that raised it. The camp's
/// scout supplies the target; this leash keeps a successful raid from turning
/// into an all-map chase that abandons the outpost.
pub(crate) const BARBARIAN_RAID_RADIUS: i32 = 10;
/// A unit that has been pushed farther than this returns to its camp before it
/// chooses another target. The small hysteresis prevents it from oscillating
/// on the boundary of the raid ring.
const BARBARIAN_RETURN_RADIUS: i32 = BARBARIAN_RAID_RADIUS + 2;
/// Trade is held while a real raider or camp is this close to one of our
/// cities. A barbarian Scout is an information unit, not a combat threat.
pub(crate) const BARBARIAN_TRADE_RISK_RADIUS: i32 = 7;
/// A nearby barbarian army must be materially better than our best land unit
/// before it can displace a victory or economy research lane. Aggregate
/// military power is deliberately not used here: ten Archers do not answer a
/// Crossbowman or Man-at-Arms at the city they are supposed to protect.
const BARBARIAN_MILITARY_GAP_RATIO: f64 = 1.25;
const BARBARIAN_MILITARY_GAP_MARGIN: f64 = 5.0;
/// A city with this many local barbarian pressure points must produce a
/// defender even when it is not formally at war with a major civilization.
const BARBARIAN_LOCAL_DEFENDER_RADIUS: i32 = 3;

/// How near a city a hostile must come before that city wants somebody actually
/// standing in it. Three tiles is one turn's move for most classical units, so a
/// garrison ordered at this range is in place before the attacker arrives.
pub(crate) const GARRISON_ALERT_RADIUS: i32 = 3;

/// How many recon units [`BasicAi::recon_is_the_missing_arm`] will rebuild
/// toward after the empire has expanded. Two independent scouts are a bounded
/// hedge against one being killed, trapped, or forced away from the frontier;
/// past that the ordinary military choice resumes, so reconnaissance cannot
/// become an endless appetite.
const RECON_ARM_MAX: usize = 2;

/// A capital needs one eye; a second city is the first point at which losing
/// that sole Scout can leave the whole empire blind. Do not spend a second
/// early build before expansion has paid for it.
const RECON_SECOND_EYE_CITY_FLOOR: usize = 2;

/// A developed empire that still has not met most city-states earns one more
/// independent land explorer. This is a contact sweep, not a general army
/// target: it is considered only after the ordinary two live eyes exist.
const RECON_CITY_STATE_SWEEP_ARM_MAX: usize = 3;
const RECON_CITY_STATE_SWEEP_CITY_FLOOR: usize = 6;
const RECON_CITY_STATE_SWEEP_UNMET_MIN: usize = 3;

/// A naval scout needs room for a complete Galley move after launch: the
/// launch tile plus its three movement points. Anything smaller is commonly
/// a lake where the only available order is to turn straight back, which
/// spends production without opening another contact or frontier.
/// How much one place on the shipped pantheon order is worth, in the same
/// yield-per-turn units the board score is measured in.
///
/// ⚠ One yield a turn per place, so the prior spans the WHOLE ROSTER — which
/// means the board only overrules the order when it is paying more than the
/// spread of the order itself. That is deliberately a high bar: the order
/// encodes real judgement about the pantheons whose value is not per tile, and
/// this treatment exists to be measured against it, not to assume it is wrong.
///
/// ⚠⚠ THE BAR THEREFORE MOVES WITH THE ROSTER, and the roster has changed
/// under it. This was written against eleven names and spanned eleven; the
/// last twelve pantheons landed on 2026-08-24 and it spans twenty-three, so a
/// board read now has to pay roughly twice as much to overrule the order.
/// That is the stated design holding, not drifting — but it is a real change
/// to what `pantheon_reads_the_board` does, and the arithmetic was
/// deliberately left alone rather than rescaled, because the gene has a
/// recorded screen (`docs/eval/2026-08-18-the-pantheon-is-chosen-from-the-land-not-from-a-fixed-list.md`,
/// parity at 60 pairs) and re-weighting it silently would price a measured arm
/// nobody re-measured.
const PANTHEON_PRIOR_STEP: f64 = 1.0;

const NAVAL_RECON_MIN_WATERWAY_TILES: usize = 4;
/// A major war fought on the water needs one hull for the battle and one to
/// keep charting the world. This is an exploration arm, not a fleet target:
/// the generic navy policy may still ask for more combat ships.
const NAVAL_RECON_WARTIME_ARM_MAX: usize = 2;

/// Once a scout reaches the nearest fog, inspect this many additional rings
/// before choosing where to go.  A short horizon is enough to distinguish a
/// dead-end nibble of fog from the edge of a broad unexplored region, without
/// making every scout sweep the whole world every turn.
const EXPLORATION_FRONTIER_LOOKAHEAD: i32 = 4;
/// Once the capital's connected land has at most this many independently
/// settleable sites left, the island-exploration gene starts looking offshore.
const ISLAND_EXPLORATION_HOME_SITE_LIMIT: usize = 2;

/// How far a reconnaissance unit will detour for a tribal village it has
/// already charted but cannot reach this turn. See `BasicAi::village_seeking`.
/// Two scout-turns of ground: far enough to catch the villages a sweep walked
/// past, near enough that the frontier is never abandoned for a long march.
const VILLAGE_SEEK_RADIUS: i32 = 6;

/// A nearby field unit can collect a charted village when no Scout is suitable
/// for it, but it must not become a map-wide errand. Two ordinary land-unit
/// turns lets an adjacent army cash in a discovery without replacing the recon
/// arm or pulling it away from its actual job.
const VILLAGE_MILITARY_SEEK_RADIUS: i32 = 4;

/// A meteor site is not an ordinary tribal-village roll: its one-entry table
/// grants a free, most-advanced Heavy Cavalry unit. Four turns is still a
/// bounded errand, but gives the nearby army a real chance to deny this rare
/// prize to a rival.
const METEOR_SEEK_RADIUS: i32 = 12;
const METEOR_MILITARY_SEEK_RADIUS: i32 = 8;

/// Railroads are valuable infrastructure, but every tile consumes one Iron
/// and one Coal. Keep enough of each material for an emergency unit upgrade
/// instead of letting an idle Engineer pave the stockpile down to zero.
const RAILROAD_RESOURCE_RESERVE: f64 = 4.0;

/// The ranked candidate for an immediate border purchase: its utility, a
/// deterministic city/position tie-breaker, and the legal action to replay.
type PlotPurchaseCandidate = (f64, std::cmp::Reverse<(u32, Pos)>, Action);

mod advanced;
/// Enemy attack envelopes: where the enemy can strike next turn. See
/// `ai/attack_envelopes.rs`.
mod attack_envelopes;
/// Barbarians: what they threaten and how the empire answers. See
/// `ai/barbarians.rs`.
mod barbarians;
/// Builders: what a Builder improves next and which city trains the next one.
/// See `ai/builders.rs`.
mod builders;
mod campus_buildings;
mod capital_keeps_its_governor;
mod commercial_hub;
mod deck_queue;
#[cfg(test)]
mod deck_queue_tests;
/// The baseline controller's diplomatic turn: deals, bilateral trade, levies
/// and spies. See `ai/diplomacy_turn.rs`.
mod diplomacy_turn;
/// The baseline controller's empire turn: research, government and policies,
/// pantheon and religion, Great People, governors. See `ai/empire_turn.rs`.
mod empire_turn;
mod first_pad_in_the_forge;
mod garrison_before_the_surprise;
/// Gold spending: what the AI buys with gold. See `ai/gold_spending.rs`.
mod gold_spending;
/// Live Great Person activation: what a claimed person needs and how to get
/// it. See `ai/great_people.rs`.
mod great_people;
mod industrial_zones;
mod launch_pad_resume;
mod movement_risk;
/// The navy: which ships, where they can launch and chart, and the water
/// research behind them. See `ai/naval.rs`.
mod naval;
mod naval_demand;
mod opening_builder;
/// The policy deck and Dedications: which cards to hold and which Dedications
/// to take. See `ai/policy_deck.rs`.
mod policy_deck;
#[cfg(test)]
mod policy_growth_tests;
/// `prod-tiles-call-the-builders`: the worked-backlog Builder quota counts
/// production ground in full. See `ai/prod_tiles_call_the_builders.rs`.
mod prod_tiles_call_the_builders;
/// What a city builds next: `pick_item`, its items, gold purchases and the
/// Trader decision. See `ai/production_choice.rs`.
mod production_choice;
mod retainers_garrison;
mod scout_first;
mod scout_inference;
/// Settlers: when the empire wants one, where it goes, and walking it there.
/// See `ai/settlers.rs`.
mod settlers;
mod siege_support;

/// The baseline controller's unit turn: `military_step`, tactics, retreat and
/// healing, barbarians, exploration and villages. See `ai/unit_turn.rs`.
mod unit_turn;
mod walls_under_fire;
/// `Weights`: the strategy weights steering `BasicAi`. See `ai/weights.rs`.
mod weights;

mod slinger_first;
pub use advanced::commitments::{CommitmentCensus, CommitmentLedger};
pub use advanced::victory_portfolio::{
    DevelopmentPhase, PortfolioReport, PortfolioTrace, VictoryEstimate,
};
pub use advanced::{
    deployment_treatments, gene, gene_ledger, gene_ledger_rows, host_only_tags, ledger_default_on,
    ledger_verdict, live_tags, repair_tags, repair_tags_on, screenable_genes, AdvancedAi,
    AirCityAssault, ArmyUpgradeMaterialWant, Axis, BarterSpare, ExpansionCensus, ForceDomain,
    ForceGroup, ForcePosture, Gene, GeneLedgerApplied, GeneVerdict, GrandStrategy, GunResourceWant,
    Kind, Measure, StrategicPlan, StrategyCensus, Verdict, VictoryTarget, GENES,
    LAND_GRAB_CITY_CEILING, LAND_GRAB_CITY_FLOOR, LAND_GRAB_PIPELINE_BASE,
    LAND_GRAB_TILES_PER_CITY, PRODUCTION_CITY_TARGET_FLOOR,
};
pub(crate) use policy_deck::*;
pub use weights::*;

const TECH_PRIORITY: [&str; 15] = [
    "pottery",
    "animal_husbandry",
    "mining",
    "writing",
    "archery",
    "bronze_working",
    "currency",
    "masonry",
    "irrigation",
    "iron_working",
    "mathematics",
    "construction",
    "engineering",
    "education",
    "machinery",
];
/// Number of consecutive eras in the unattended controller's permissible
/// research window. This lets a strategy start useful follow-up work without
/// abandoning an old branch indefinitely.
const RESEARCH_ERA_WINDOW: usize = 3;
const CIVIC_PRIORITY: [&str; 8] = [
    "code_of_laws",
    "craftsmanship",
    "foreign_trade",
    "early_empire",
    "state_workforce",
    "military_tradition",
    "drama_poetry",
    "political_philosophy",
];
const DISTRICT_PRIORITY: [&str; 4] = ["campus", "commercial_hub", "holy_site", "theater_square"];

/// The Amenity deficit from which the repair district outranks a city's lane
/// whatever the city holds: the ×0.80 band, not the ×0.90 one. See the
/// `amenity_districts` push in `pick_item`.
const AMENITY_DISTRICT_FIRST_BAND: f64 = 2.0;
/// Specialty districts a city must already hold before a one-Amenity shortfall
/// outranks its lane. See the `amenity_districts` push in `pick_item`.
const AMENITY_DISTRICT_LANE_SERVED: usize = 2;

/// The population at which Civilization VI lets a city start a Settler. See
/// `BasicAi::host_settler_pop`.
const HOST_SETTLER_MIN_POP: f64 = 2.0;

/// Standard turns a Settler needs after it is built to walk out, found, and
/// repay itself: `SETTLE_LAG` + `SETTLE_PAYBACK` in `AdvancedAi`. The land
/// grab's `pick_item` window closes this far before the turn limit instead of
/// at the genome's `settler_stop_turn`. See `BasicAi::land_grab`.
const LAND_GRAB_SETTLE_HORIZON: u32 = 18;

/// One coordinated force as an observer sees it: what it is, where it is
/// going, and how ready it is to fight when it gets there.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceReport {
    pub domain: &'static str,
    pub posture: &'static str,
    pub units: usize,
    pub objective: Pos,
    pub readiness: f64,
    pub strength_ratio: f64,
}

/// Observer-only state for one unified, appointed military campaign.
///
/// The controller owns the mutable plan. This report exposes both the current
/// package and cumulative lifecycle counters so evaluators can distinguish a
/// plan that formed from one that researched, mobilized, declared, and took
/// its objective quickly. Nothing in play reads this type.
#[derive(Clone, Debug, PartialEq)]
pub struct WarPlanReport {
    pub enabled: bool,
    /// Whether appointment and launch use the preregistered selective-v2
    /// policy. The mutable campaign still lives in the same `WarPlan`.
    pub selective: bool,
    /// Whether appointment additionally requires the preregistered v3
    /// ready-force package and short estimated launch window.
    pub rapid: bool,
    pub active: bool,
    pub phase: Option<&'static str>,
    pub target_player: Option<usize>,
    pub objective_city: Option<u32>,
    pub breakthrough_tech: Option<Name>,
    pub assault_unit: Option<Name>,
    pub predecessor: Option<Name>,
    pub breach_unit: Option<Name>,
    pub required_bodies: usize,
    pub ready_bodies: usize,
    pub staged_bodies: usize,
    pub breach_ready: bool,
    pub upgrade_gold_reserved: f64,
    pub appointed_turn: Option<u32>,
    pub appointments: u32,
    pub breakthroughs: u32,
    pub mobilizations: u32,
    pub declarations: u32,
    pub complete_package_declarations: u32,
    pub objectives_captured: u32,
    pub objectives_captured_within_ten: u32,
    pub appointment_to_tech_turns: u32,
    pub tech_to_declaration_turns: u32,
    pub declaration_to_capture_turns: u32,
    pub appointment_to_tech_samples: Vec<u32>,
    pub tech_to_declaration_samples: Vec<u32>,
    pub declaration_to_capture_samples: Vec<u32>,
    pub aborts: BTreeMap<&'static str, u32>,
}

/// Everything an agent is willing to say about its own medium-term
/// intentions. The spectator HUD reads this to explain *why* a civilization
/// is doing what it does instead of only showing the outcome; nothing here
/// feeds back into the simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanReport {
    pub strategy: &'static str,
    pub victory_target: Option<&'static str>,
    /// Whether the plan in force is the ancient-rush treatment. Observer-only:
    /// evaluators use this to measure treatment exposure, never to choose an
    /// action.
    pub rush: bool,
    pub target_player: Option<usize>,
    pub target_city: Option<u32>,
    pub threatened_city: Option<u32>,
    pub desired_cities: usize,
    pub assessed_turn: u32,
    /// Rivals this planner decided to offer peace this turn. The offer itself
    /// is `ProposeDeal { peace: true }`, answered on an ordinary board by the
    /// other seat's valuation — so a declined offer never reaches the
    /// applied-action log. A mirrored game must still ASK the real rival, so
    /// the made decision is reported as intent here. Unlike the retracted
    /// war-from-plan channel this exports a decision the planner took, not a
    /// preference upgraded into one.
    pub peace_offers: Vec<usize>,
    /// The subset of `peace_offers` the planner is ROUTED on — outmatched
    /// on the front (`my_power < theirs × 0.62`) or the one-war rout — and
    /// so the only offers a live tribute may ride on. Every other offer
    /// (a second front, fatigue, a religious lane, an envoy reclaim) is
    /// white peace: 2026-08-24 the live seat had paid 142 tributes at a
    /// median 116 Gold, three quarters of the treasury on every retry
    /// regardless of why the offer was made.
    pub peace_routed: Vec<usize>,
    /// The subset of `peace_offers` that asks the rival to cede a town
    /// (`peace-asks-a-city`): white offers made from at least
    /// `PEACE_CITY_ASK_RATIO` times the rival's power. The live bridge adds
    /// the town to the first ask only; the retry is white peace.
    pub peace_asks_city: Vec<usize>,
    pub forces: Vec<ForceReport>,
    /// The one authority spanning target selection, research, production,
    /// treasury, staging, declaration, and exploitation.
    pub war: Option<WarPlanReport>,
}

pub trait Ai {
    /// Whether this controller needs last-seen world memory for deliberation.
    fn uses_player_observation(&self) -> bool {
        false
    }
    fn take_turn(&mut self, g: &mut Game, pid: usize);

    fn strategy_label(&self) -> Option<&'static str> {
        None
    }

    /// The agent's current plan, for observers only. Stateless baselines
    /// have no plan to report.
    fn plan_report(&self) -> Option<PlanReport> {
        None
    }

    /// How many of this agent's macro reviews reached its search, for
    /// evaluators only. An agent that searches must be able to say when it
    /// did not: a cheap prior answering every review leaves a scripted
    /// agent under a searching agent's name, and a win rate cannot tell the
    /// difference. Agents without a search return `None`.
    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        None
    }

    /// How often an adaptive Expansion plan reached the Advanced-production
    /// dispatcher, and what that newly exposed call actually completed. This
    /// is observer-only telemetry for the default-off expansion experiments;
    /// agents without the instrument return `None`.
    fn expansion_census(&self) -> Option<ExpansionCensus> {
        None
    }

    /// Write this agent's reasoning into an observer's log.
    ///
    /// Every seat at a watched table is handed a handle on the *same*
    /// [`Journal`], so the record is one ordered account of a turn rather than
    /// one log per civilization that has to be interleaved afterwards. An
    /// agent with nothing to say about itself — the random baseline — ignores
    /// this, which is what the default does.
    fn attach_journal(&mut self, _journal: Journal) {}
}

impl<T: Ai + ?Sized> Ai for Box<T> {
    fn uses_player_observation(&self) -> bool {
        (**self).uses_player_observation()
    }
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        (**self).take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        (**self).strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        (**self).plan_report()
    }

    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        (**self).review_census()
    }

    fn expansion_census(&self) -> Option<ExpansionCensus> {
        (**self).expansion_census()
    }

    fn attach_journal(&mut self, journal: Journal) {
        (**self).attach_journal(journal);
    }
}

/// Play a game out headlessly. The turn bound is not belt-and-braces: the only
/// thing that reliably ends a game is the score victory `do_end_turn` awards at
/// the turn limit, and `set_winner` refuses that when the lobby switched score
/// off — a setting that is serialized and restored from saves. Without the
/// bound such a game runs past its limit forever. With score enabled the bound
/// never fires first, because `set_winner` runs inside `do_end_turn` before
/// this condition is tested again. A score-disabled world is recorded as a
/// draw when the bounded loop reaches its cap, matching the server stepper.
pub fn run_game<A: Ai>(g: &mut Game, ais: &mut [A]) {
    run_game_observed(g, ais, |_| {});
}

/// [`run_game`] with an observer called once at the start of every turn the
/// loop plays, before any seat acts on it — the whole world, read-only.
///
/// The gene screen prices science genes by research pace and needs one
/// mid-game read (techs known at the Standard-turn-150 mark) that a finished
/// world no longer holds; rather than teach every caller a second loop, the
/// loop takes a visitor. `run_game` is this with a visitor that does nothing,
/// so headless play is byte for byte what it was.
pub fn run_game_observed<A: Ai>(g: &mut Game, ais: &mut [A], mut observe: impl FnMut(&Game)) {
    // Production players deliberate from last-seen memory even in headless
    // tournaments. Historical controllers that do not read it keep the old
    // rollout cost and replay contract.
    g.set_fog_memory(ais.iter().any(Ai::uses_player_observation));
    // Same reasoning for the narrated war ledger: no observer reads a
    // half-finished headless turn, so the per-action re-sync buys nothing.
    // Declarations, peaces, and turn boundaries still sync it, so the ledger
    // in reports and saved games is unchanged.
    g.set_war_ledger(false);
    let mut observed_turn = None;
    while g.winner.is_none() && g.turn <= g.max_turns {
        if observed_turn != Some(g.turn) {
            observed_turn = Some(g.turn);
            observe(g);
        }
        let pid = g.current;
        ais[pid].take_turn(g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    g.finish_at_turn_limit();
}

// ----------------------------------------------------------------- RandomAi

pub struct RandomAi {
    rng: Rng,
}

impl RandomAi {
    pub fn new(seed: u64) -> RandomAi {
        RandomAi {
            rng: Rng::new(seed),
        }
    }
}

impl Ai for RandomAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        g.with_deferred_visibility(|g| self.take_turn_inner(g, pid));
    }
}

impl RandomAi {
    fn take_turn_inner(&mut self, g: &mut Game, pid: usize) {
        for _ in 0..60 {
            let acts: Vec<Action> = g
                .legal_actions(pid)
                .into_iter()
                .filter(|a| !matches!(a, Action::EndTurn))
                .collect();
            if acts.is_empty() {
                break;
            }
            let a = acts[self.rng.below(acts.len())].clone();
            let _ = g.apply(pid, &a);
            if g.winner.is_some() {
                break;
            }
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
}

// ------------------------------------------------------------------ BasicAi

const GOV_PRIORITY: [&str; 12] = [
    "digital_democracy",
    "synthetic_technocracy",
    "corporate_libertarianism",
    "democracy",
    "communism",
    "fascism",
    "merchant_republic",
    "monarchy",
    "classical_republic",
    "oligarchy",
    "autocracy",
    "chiefdom",
];

#[cfg(test)]
mod gene_table_tests;

/// Strategic job inferred from a unit's class and promotion line. Both AI
/// tiers use the same doctrine so independent movement and force coordination
/// do not disagree about what a unit is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnitDoctrine {
    Recon,
    Assault,
    Mobile,
    Ranged,
    Siege,
    Support,
    AirDefense,
    AirStrike,
    Carrier,
}

/// Everything about a unit that changes when it accomplishes something:
/// charges spent improving, building, or spreading; experience from a fight;
/// damage taken or healed; a promotion chosen; a concert played. Whatever else
/// a unit did with a turn, if none of this moved then the turn bought nothing.
type WorkMark = (i32, i64, i32, usize, i64);

fn work_mark(g: &Game, uid: u32) -> WorkMark {
    let unit = &g.units[&uid];
    (
        unit.charges,
        unit.xp,
        unit.hp,
        unit.promotions.len(),
        unit.album_sales,
    )
}

/// One unit's recent whereabouts, which is the only place a livelock is
/// visible. Every step of a loop is individually the best move available, so
/// no single decision can be blamed for it; only the unit's own history shows
/// that the decisions together are going nowhere.
#[derive(Clone, Default)]
struct UnitMotion {
    /// The tile this unit began each of its last `LIVELOCK_WINDOW` turns on,
    /// newest last.
    tiles: VecDeque<Pos>,
    /// The host turn that supplied the current sample when the live bridge is
    /// accounting for motion by host turn. A post-order replan is another
    /// view of that turn, not another fruitless turn for this unit.
    observed_turn: Option<u32>,
    /// The work fingerprint as of the last turn it changed.
    work: WorkMark,
    /// Consecutive turns since the fingerprint last changed.
    fruitless: u32,
    /// While this is in the future the unit holds its ground instead of
    /// issuing orders it has already proved are worthless.
    resume_turn: u32,
    /// This turn's verdict, settled once when the window is taken down. The
    /// tactical scorers ask about it for every candidate tile of every unit,
    /// which is far too hot a path to re-derive it in.
    looping: bool,
}

impl UnitMotion {
    /// How many distinct tiles the window covers.
    fn footprint(&self) -> usize {
        let mut seen: Vec<Pos> = Vec::with_capacity(self.tiles.len());
        for tile in &self.tiles {
            if !seen.contains(tile) {
                seen.push(*tile);
            }
        }
        seen.len()
    }

    /// A full window of turns spent moving between a handful of tiles while
    /// nothing about the unit changed. Both halves matter: a unit that has
    /// stopped is a different (already reported) problem, and a unit that is
    /// spending charges or trading blows is working, however small its circuit.
    fn circling(&self) -> bool {
        if self.tiles.len() < LIVELOCK_WINDOW || (self.fruitless as usize) < LIVELOCK_WINDOW {
            return false;
        }
        (2..=LIVELOCK_FOOTPRINT).contains(&self.footprint())
    }
}

/// The durable, unit-local job the AI has committed to.
///
/// This lives in the controller rather than in [`crate::game::Unit`]: it is
/// reasoning state, not a game rule, and it must survive the Civilization VI
/// mirror rebuilding its board with fresh internal unit IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitObjective {
    /// This unit is part of the force trying to take this specific city.
    CaptureCity {
        city: u32,
        target: Pos,
        started_turn: u32,
        last_confirmed_turn: u32,
    },
}

impl UnitObjective {
    /// The physical destination associated with this objective.
    pub fn target(self) -> Pos {
        match self {
            Self::CaptureCity { target, .. } => target,
        }
    }
}

/// A dangerous approach remembered by one unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitDangerMemory {
    /// The tile the unit deliberately declined to enter.
    pub position: Pos,
    /// Conservative expected counter-damage from the enemies visible to this
    /// planner when the warning was recorded.
    pub expected_damage: u32,
    pub observed_turn: u32,
    /// Exclusive turn at which the sighting is no longer trusted.
    pub expires_turn: u32,
}

/// What a tile invites from everything that can strike it next turn, read two
/// ways from one pass.
///
/// ★★★ THE SUM AND THE LARGEST BLOW ANSWER DIFFERENT QUESTIONS, and the sum
/// answers the survival one wrongly in both directions. Three shooters that
/// each take a third of a unit's hit points make a tile lethal without any of
/// them being able to kill it, and one Bombard that kills outright reads the
/// same as three Archers that between them merely wound. Route safety wants
/// `total`; whether a unit is one blow from death wants `worst`.
#[derive(Clone, Copy, Default)]
pub(crate) struct IncomingDamage {
    /// Every covering source's expected damage, added up.
    pub(crate) total: f64,
    /// The largest single one of them.
    pub(crate) worst: f64,
}

impl IncomingDamage {
    /// Nothing survives this: the unit is already gone from the board.
    const LETHAL: Self = Self {
        total: f64::INFINITY,
        worst: f64::INFINITY,
    };

    fn with(mut self, damage: f64) -> Self {
        self.total += damage;
        self.worst = self.worst.max(damage);
        self
    }

    fn merge(self, other: Self) -> Self {
        Self {
            total: self.total + other.total,
            worst: self.worst.max(other.worst),
        }
    }
}

/// A legal resting place scored for an endangered unit. Safety is deliberately
/// recorded separately from the healing rate: a City Center is excellent
/// recovery ground only when the enemy's next turn cannot harm its garrison.
#[derive(Clone, Copy)]
struct EvacuationTile {
    position: Pos,
    incoming: f64,
    healing: i32,
    city: bool,
    distance_from_danger: i32,
}

/// Inspectable, multi-turn memory owned by one individual unit.
///
/// An objective, a danger warning, and a temporary retreat are independent:
/// taking a safe step backward must not erase the city this unit was helping
/// to capture. Callers receive a copy through [`BasicAi::unit_memory`], so
/// observers cannot mutate the controller's reasoning state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnitMemory {
    pub objective: Option<UnitObjective>,
    pub danger: Option<UnitDangerMemory>,
    /// Exclusive turn through which the unit must prefer getting safe over
    /// resuming its objective.
    pub retreat_until: Option<u32>,
}

/// The hostile next-turn attack envelopes, remembered with the exact board
/// they were computed on. See `BasicAi::enemy_attack_envelopes`.
struct AttackEnvelopeCache {
    /// `(turn, viewer, board fingerprint)` — the fingerprint covers every
    /// unit's identity, owner, place and fighting state, every city's owner
    /// and place, the map epoch and the war ledger: everything `attack_reach`
    /// reads. Equal keys mean equal envelopes for every enemy.
    key: (u32, usize, u64),
    envelopes: std::sync::Arc<AttackEnvelopes>,
}

/// The tiles one hostile unit can strike next turn, ascending and distinct.
///
/// ★★★★ THIS WAS A `BTreeSet<Pos>` AND THE TREE WAS 9.0% OF THE MAIN THREAD.
/// `sample` on the standard screen shape (`docs/SIMULATOR_PERFORMANCE.md`,
/// 2026-08-22) put `BTreeMap<Pos, SetVal>` — a `BTreeSet<Pos>`, and these
/// envelopes are the largest holder of them — at 9.0% self time, ahead of a
/// share of the 7% in `malloc`/`free` and the 5.7% in `memmove` that its
/// per-tile nodes were paying for. An envelope is built once and then only
/// read, by `contains` and by whole-set iteration, so a tree buys nothing a
/// sorted slice does not: `binary_search` answers membership and the slice
/// iterates in the identical ascending order, from one contiguous buffer.
///
/// ⚠ THE ORDER IS LOAD-BEARING. `safe_healing_step` unions these and
/// `evacuation_tile_cmp` breaks ties on `Pos`, so a different iteration order
/// is a different game. The constructor enforces ascending-and-distinct
/// rather than trusting its caller, which is what makes `binary_search` sound
/// no matter where the tiles came from. The common cached path shares the
/// engine's already-sorted immutable target buffer; the standalone
/// constructor keeps enforcing that invariant for every other caller.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub(crate) struct EnvelopeReach(std::sync::Arc<[Pos]>);

impl EnvelopeReach {
    /// Adopt a list of struck tiles. `Game::attack_reach_from_flood` already
    /// returns them ascending and distinct, so the common path is the linear
    /// check and no work; an air unit's disk is not ordered, so it is sorted.
    fn from_tiles(mut tiles: Vec<Pos>) -> Self {
        if tiles.windows(2).any(|pair| pair[0] >= pair[1]) {
            tiles.sort_unstable();
            tiles.dedup();
        }
        Self(std::sync::Arc::from(tiles))
    }

    /// Adopt the engine's immutable, ascending-and-distinct raw target list.
    /// `Game::attack_reach_from_flood` establishes that order before its
    /// result enters `AttackReachFromFlood`; keeping the same allocation here
    /// avoids copying every target just to give it an envelope wrapper.
    fn from_shared_tiles(tiles: std::sync::Arc<[Pos]>) -> Self {
        Self(tiles)
    }

    pub(crate) fn contains(&self, position: &Pos) -> bool {
        self.0.binary_search(position).is_ok()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Pos> {
        self.0.iter()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn as_slice(&self) -> &[Pos] {
        &self.0
    }
}

/// The hostile envelopes one board hands back: each enemy, and the tiles it
/// can strike next turn. Shared, so a per-enemy cache hit costs a refcount.
pub(crate) type AttackEnvelopes = Vec<(u32, std::sync::Arc<EnvelopeReach>)>;

/// One envelope table and the union of every tile in it, kept together so a
/// reader can tell whether the union still describes the table it holds. See
/// [`BasicAi::covered_tiles`]. Unlike one raw envelope target list, the union
/// is freshly assembled here, so its outer `Arc<Vec<_>>` keeps that buffer
/// without converting it to a second shared slice allocation.
type CoveredTiles = (std::sync::Arc<AttackEnvelopes>, std::sync::Arc<Vec<Pos>>);

/// The three route priorities used by a recovering unit. They are immutable
/// while a board and hostile-envelope table are unchanged, so frontier clones
/// can share one full-map scan instead of repeating it per unit.
#[derive(Debug, PartialEq, Eq)]
struct SafeHealingTargets {
    cities: HashSet<Pos>,
    friendly_tiles: HashSet<Pos>,
    neutral_tiles: HashSet<Pos>,
}

/// A cache slot that a clone does NOT inherit.
///
/// ⚠⚠ Every other `Arc<Mutex<..>>` cache on `BasicAi` is keyed by CONTENT --
/// `covered_tiles_cache` by the envelope `Arc`'s identity, `envelope_board` by
/// `attack_envelope_fingerprint` -- so sharing it with a cloned AI is safe. The
/// safe-healing entry is keyed by `Game`'s monotonic COUNTERS, which answer "did
/// this board change since I looked" and cannot answer "are these two boards the
/// same": two siblings that each took one write carry the same counter
/// (`sibling_clones_share_a_healing_stamp_so_the_cache_may_not_cross_boards`
/// pins exactly that). Resetting on clone keeps the counter key sound, and costs
/// only the first scan on a branched board.
struct FreshOnClone<T>(std::sync::Mutex<Option<T>>);

// Derived `Default` would demand `T: Default`, which a cache entry has no
// meaningful empty value for; the slot's empty value is `None`.
impl<T> Default for FreshOnClone<T> {
    fn default() -> Self {
        Self(std::sync::Mutex::new(None))
    }
}

impl<T> Clone for FreshOnClone<T> {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(None))
    }
}

impl<T> FreshOnClone<T> {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<T>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// A safe-healing scan together with every input that selects its tiles.
struct SafeHealingTargetCache {
    pid: usize,
    stamp: (u64, u64, u64, u64),
    envelopes: std::sync::Arc<AttackEnvelopes>,
    targets: std::sync::Arc<SafeHealingTargets>,
}

/// One enemy's envelope, with the key that says when it may be reused.
///
/// ★★★★★ THE BOARD KEY IS TOO COARSE AND THAT IS MOST OF THE COST. The
/// board-wide key is exact and cheap to test, but it moves whenever *any*
/// unit moves — so a serial path that steps one own unit recomputes an
/// `attack_reach` flow field for every visible enemy on the map. A `sample` of
/// head put `enemy_attack_envelopes` at 74.9% of the main thread with the
/// board key already tightened to what reach reads (#2148).
///
/// An enemy's reach can only change if something *inside that reach* changed,
/// so each envelope carries a key over its own neighbourhood instead.
struct EnemyEnvelope {
    reach: std::sync::Arc<EnvelopeReach>,
    sensitive: std::sync::Arc<std::collections::HashSet<Pos>>,
}

/// The multiplier and the seed the three envelope fingerprints mix with — the
/// golden-ratio constant `vision_key` uses in `src/game.rs`. Shared so the
/// three cannot drift apart, never so that two of them may be compared: each
/// hashes a different set of fields and only ever meets its own previous
/// value.
const FINGERPRINT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// One unit, in exactly the terms an envelope can notice.
#[derive(PartialEq, Eq)]
struct EnvelopeUnit {
    pos: Pos,
    kind: u32,
    owner: usize,
    formation: u8,
    zoc_stopped: bool,
    patrol: Option<Pos>,
    /// `formation_movement_locked_by_zoc` reads all three, and all three move
    /// without the unit moving: a unit that acts where it stands loses its
    /// whole flood, and regains it next turn.
    locked: (bool, bool, bool),
    linked_to: Option<u32>,
}

impl EnvelopeUnit {
    /// One unit's board record. The field set is
    /// `the_delta_tracks_every_field_the_board_key_hashes`'s business, not
    /// this function's: it exists so the two rosters are read the same way.
    fn of(unit: &crate::game::Unit) -> Self {
        Self {
            pos: unit.pos,
            kind: unit.kind.id(),
            owner: unit.owner,
            formation: unit.formation,
            zoc_stopped: unit.zoc_stopped,
            patrol: unit.air_patrol.then_some(unit.air_patrol_pos).flatten(),
            locked: (unit.started_turn_in_zoc, unit.acted, unit.moved),
            linked_to: unit.linked_to,
        }
    }
}

/// The board the per-enemy envelopes were last measured against.
struct EnvelopeBoard {
    /// Map epoch, the belligerence fingerprint and the city fingerprint. Any
    /// of these moving is treated as "everything changed": they are rare and
    /// global.
    stamp: (u64, u64, u64),
    /// Every field of a unit that the board key hashes, plus the tile it is
    /// flying a patrol over.
    ///
    /// ★★★★★ THE DELTA MUST NOT BE WEAKER THAN THE KEY IT STANDS IN FOR.
    /// `attack_envelope_fingerprint` decides whether *any* envelope may be
    /// reused and hashes place, owner, kind, formation and `zoc_stopped`; this
    /// decides which *individual* envelopes may be, and shipped tracking only
    /// place, kind and owner. A unit forming a corps without moving therefore
    /// produced an **empty** delta and every envelope was reused across it.
    /// The two are now the same field set by construction, which
    /// `the_delta_tracks_every_field_the_board_key_hashes` holds them to.
    ///
    /// ⚠ The patrol is the one field the board key does *not* hash, and it
    /// belongs here because `can_enter_past` refuses a step onto a
    /// tile a hostile fighter is patrolling, and it finds that fighter by
    /// scanning **every unit in the world** — so the blocker can sit
    /// arbitrarily far from the tile it blocks, and starting or moving a
    /// patrol changes an envelope without the unit moving at all. Tracking
    /// only place, spec and owner let that change produce an *empty* delta, so
    /// every envelope was reused across it.
    ///
    /// ⚠ ASCENDING BY ID, WHICH IS WHAT MAKES THE DIFF A MERGE. `Game.units`
    /// is a `BTreeMap` and iterates in the same order, so the two boards are
    /// compared by walking them side by side — no hash of every id on every
    /// ask, and no second table allocated to be thrown away. It was a
    /// `HashMap` and the diff hashed each id twice, once to build the new
    /// board and once to look the old one up.
    units: Vec<(u32, EnvelopeUnit)>,
}

/// What [`BasicAi::envelope_board_change`] found, for the two questions the
/// envelope table asks of it.
struct EnvelopeChange {
    /// The tiles a change touched, or `None` for "assume everything changed".
    touched: Option<Vec<Pos>>,
    /// Whether any unit the previous board held is gone from this one — or,
    /// conservatively, `true` whenever the delta is `None` and nothing is
    /// known. Only the per-enemy store's cleanup reads it; see the call site.
    removed: bool,
}

/// `parity-upgrades-before-it-buys` (both versions): a unit this many eras or
/// more behind the world era is obsolete to buy.
pub(crate) const OBSOLETE_BUY_ERAS_BEHIND: usize = 2;

/// `parity-upgrades-before-it-buys-3`: a unit this many eras or more behind
/// the newest technology we know is obsolete to buy. Three, not two: a Science
/// beeline reaches one high-era technology long before the rest of the tree.
pub(crate) const OBSOLETE_BUY_TECH_ERAS_BEHIND: usize = 3;

#[derive(Clone)]
pub struct BasicAi {
    minor: bool,
    barb: bool,
    /// Apply the default early-game barbarian response. The frozen
    /// `advanced_v1` controller disables this so its recorded decision stream
    /// remains an honest control; Basic and live Advanced use it.
    pub(crate) barbarian_tactics: bool,
    /// Build hulls only where they have open water to sail into. **On in
    /// production since 2026-08-18**; see `city_has_open_water`, and
    /// `advanced_without_open_water_navy` for the withhold.
    pub open_water_navy: bool,
    /// Let the baseline governor build the district that repairs an Amenity
    /// deficit.
    ///
    /// ⚠ `DISTRICT_PRIORITY` is four families — Campus, Commercial Hub, Holy
    /// Site, Theater Square — and `entertainment_complex` is **not one of
    /// them**. So this governor cannot build one, ever, and it makes most of a
    /// deployed empire's build decisions: `advanced_production` is reached on
    /// 22.2% of an adaptive agent's planned turns (all of them Recovery), and
    /// everything else ends here.
    ///
    /// ⚠ **Correction to what this comment said when it merged.** It claimed
    /// CIVVIS "ordered an Entertainment Complex zero times" across every live
    /// game inspected. That was measured with
    /// `verb LIKE '%ENTERTAINMENT%'` against the order log, which **misses the
    /// unique replacements** — Kongo's Hippodrome and Brazil's Street Carnival
    /// both carry `replaces: entertainment_complex`. Counting the districts
    /// actually standing at the end of five completed games gives **7 across 33
    /// cities** (6 Hippodrome, 1 Water Street Carnival), not zero.
    ///
    /// The gap this treatment closes is real but narrower than that sentence
    /// implied: the baseline governor still cannot reach the family at all, and
    /// 7 in 33 cities is thin — but "never built" was wrong, and a filter that
    /// silently drops every unique replacement is worth remembering before the
    /// next `LIKE '%NAME%'` census.
    ///
    /// That is not a small omission, because Amenities are not a yield — they
    /// are a band that multiplies every yield the city makes.
    /// `Game::amenity_yield_mult_for`: `0` → 1.00, `-3` → **0.80**, `-6` →
    /// **0.70**. Host-exported figures from run `civvis-20260803T082856Z` at
    /// turn 251, seven cities:
    ///
    /// | city | amenities | surplus | multiplier |
    /// |---|---|---|---|
    /// | Kraków | 4/7 | -3 | 0.80 |
    /// | Wroclaw | 2/8 | **-6** | **0.70** |
    /// | Radom, Warsaw, Gdansk | 4/7 | -3 | 0.80 |
    /// | Bydgoszcz | 3/6 | -3 | 0.80 |
    ///
    /// ⚠⚠ Those are **Firaxis's own numbers**, not CIVVIS's model — #967 added
    /// `GetAmenities`/`GetAmenitiesNeeded` to the bridge. An earlier attempt at
    /// this axis (#975) was closed partly because it was priced against a field
    /// nobody was sending; that is no longer true.
    ///
    /// The empire is running at roughly 0.78 on everything, including the
    /// science these games are short of. Returning it to neutral is about
    /// **+28%** — larger than the Research Lab this session also chased.
    ///
    /// Off for the frozen native controllers, whose recorded ladders would
    /// otherwise shift underneath them.
    pub(crate) amenity_districts: bool,
    /// Treat a city that is LOSING HITPOINTS as besieged even when fog hides
    /// every attacker. Measured on live run civvis-20260807T181839Z, t115:
    /// Rome at damage 35/200 with the export's hostile list EMPTY -- ranged
    /// attackers firing from fog -- while production built an amphitheater;
    /// the capital fell at t117. The two-visible-besiegers gate below stays
    /// (its 24-map score evidence is untouched); a bleeding city is a THIRD
    /// proof of siege that fog cannot suppress, and it self-clears because
    /// Civ 6 city health regenerates once the siege lifts.
    pub(crate) garrison_under_fire: bool,
    /// `threatened-city-reserve`: Gold `spend_gold` must leave in the
    /// treasury on top of its own reserve. The deployed controller sets it
    /// to one emergency defender's price while a city of ours is threatened
    /// or bleeding, and back to 0.0 after the call, so the baseline buyer
    /// cannot spend the defender money on a Market in the city being shelled
    /// (run civvis-20260827T113726Z, t162). 0.0 everywhere else.
    pub(crate) reserve_floor: f64,
    /// Preserve pre-repair defender buying only in the frozen advanced_v1 control.
    legacy_defender_purchases: bool,
    /// Scale each district family by how much of the empire still lacks it.
    pub(crate) district_coverage: bool,
    /// Version 2 of `district_coverage`: the same coverage term with a
    /// deeper floor — a family every city holds falls to a quarter of its
    /// bred weight, not half. Version 1's record is a score-share gain (z
    /// +2.2 on the 08-24 screen) with no win column, so the lever is pushed
    /// further in the direction that paid; a probe of the opposite direction
    /// (queued districts counted as held) lost share. Implies version 1; its
    /// enable turns version 1 off. Opt-in gene `district-coverage-2`.
    pub(crate) district_coverage_2: bool,
    /// Break a production COST TIE by which great-work slots can actually be filled.
    pub(crate) slot_kind_tiebreak: bool,
    pursue_religion: bool,
    /// The advanced religious corps saved for an Apostle this turn. Keep the
    /// routine city governor from spending that bank on a Missionary.
    /// False for standalone and frozen controllers.
    pub(crate) bank_faith_for_apostle: bool,
    /// `religion-seat-charts-the-rivals` acts for this seat this turn: the
    /// contact gap also stays open while a met major is under-charted. Set
    /// each turn by the advanced agent; false for standalone controllers.
    /// See `advanced/religion_seat_charts_the_rivals.rs`.
    pub(crate) charts_rival_majorities: bool,
    /// Do not enter the finite Great Prophet race at all.
    ///
    /// `pursue_religion` alone is NOT this flag. It gates missionary purchases
    /// and the secret-society lean, while the Holy Site reservation below
    /// (search `prophet_race_closed`) still spends a district slot and hammers.
    /// This flag closes that reservation. A Prophet already earned can still
    /// found a religion without reopening the race or changing the victory lane.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `skip-the-prophet-race-2`
    /// and by `religion-race-is-closed`.
    pub(crate) skip_prophet_race: bool,
    /// Open the Great Prophet race on purpose: while this is set, the empire's
    /// FIRST Holy Site outranks every lane district in `pick_item`, so the
    /// district that pays the entry fee lands before the finite race closes
    /// rather than behind a Campus and a Commercial Hub in a city that must
    /// reach population 7 to hold a third district. The one-site reservation
    /// below is unchanged: a second Holy Site waits for a religion.
    ///
    /// Set per turn from `AdvancedAi::take_turn_inner` by the gene
    /// `enter-the-prophet-race-2`, and only while `prophet_race_open_for` holds.
    pub(crate) enter_prophet_race: bool,
    /// While `enter_prophet_race` holds and the empire has no Holy Site,
    /// built or queued, the Campus or Theater Square a campus-first or
    /// culture-defense step returns becomes this city's Holy Site
    /// (`race_takes_the_district_slot`). The boost above lives in the
    /// district loop, and those steps return before it: live King
    /// 2026-10-04T205431Z held the race open from turn 30 to 60 with
    /// Astrology in hand, opened Campuses at Maracaibo (t32) and Quito (t38),
    /// and built its first Holy Site at t75, for a Great Person; T212049Z had
    /// Astrology at t35, a Campus at t57 and a Theater Square at t62, and no
    /// Holy Site.
    ///
    /// Set from `AdvancedAi` by the opt-in gene
    /// `prophet-race-takes-a-district-slot`.
    pub(crate) prophet_race_takes_a_district_slot: bool,
    /// Version 2 of the gene above: the race's Holy Site goes to the city
    /// that builds it soonest (`race_site_city`), ahead of that city's
    /// campus-first step, and no other city's Campus or Theater slot is
    /// taken for it. Live King civvis-20261005T035848Z (game 97): version 1
    /// opened the site in Barinas at turn 32, 54 production at 3.6 a turn
    /// (about 15 turns), and all three religions were founded without us;
    /// the Cree won on Religion at 135 with 560 of our Faith unspent.
    pub(crate) prophet_race_takes_a_district_slot_2: bool,
    /// `bleeding-capital-loyalty` (AdvancedAi): the Loyalty emergency takes
    /// Victor, who establishes in three turns, ahead of an unassigned
    /// five-turn Governor when the city's runway is under
    /// [`VICTOR_SHORT_RUNWAY_TURNS`].
    pub(crate) victor_first_for_a_short_runway: bool,
    /// Opt-in: an original capital keeps a Governor needed for its Loyalty.
    pub(crate) capital_keeps_its_governor: bool,
    /// Build a building that MAKES SCIENCE before one that does not.
    ///
    /// Buildings are picked cheapest-first, and that order is deliberate policy
    /// everywhere else — this flag does not repeal it. It partitions on one
    /// question and keeps cheapest-first INSIDE each half, so the relative order
    /// of any two science buildings, and of any two others, is exactly what it
    /// was. What changes is only that a Library stops queueing behind every
    /// cheaper building in the city.
    ///
    /// The regime is the argument: 89% of these games end in a science victory
    /// and the empire finishes them 41 science against 112. A finished Campus
    /// waiting on its Library is the win condition idling behind a Monument.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `science-building-first`.
    pub(crate) science_building_first: bool,
    /// A city grown to its housing builds its Granary, then its Aqueduct,
    /// ahead of another Builder, the Monument, every district and every later
    /// building. Housing caps growth: a city at its housing grows at a quarter
    /// speed, one short at half. On King civvis-20261001T003717Z, 89% of the
    /// city-turns from turn 40 to 150 were housing-bound (55% at the cap) and
    /// 88% were bound without a Granary. Pottery came at turn 12, yet not one
    /// of nine cities had a Granary at turn 127, and none had an Aqueduct.
    /// Seven of nine cities sat at 2 to 7 population while the rivals' cores
    /// reached 13, and science trailed 46 against 93 to 143. This governor
    /// reaches a Granary only after every district a city still lacks, which
    /// a growing city never runs out of.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `first-granary-reserve-3`.
    pub(crate) housing_reserve: bool,
    /// A city's first Campus ahead of its Harbor in this governor. The
    /// coastal block below asked every coastal city for a Harbor before any
    /// specialty district, and the four-player Tiny Pangaea is mostly coast:
    /// live King Gran Colombia started ZERO Campuses between turns 20 and 65
    /// in two of three games (first Campus turn 86 after seven Harbors in
    /// 2026-10-01T030914Z), and science fell from parity at turn 40 to a third
    /// of the best rival's by turn 65 in every game but the one that opened a
    /// Campus at turn 43.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `campus-before-harbor`.
    pub(crate) campus_before_harbor: bool,
    /// The capital's first Campus before its next Settler, once the empire
    /// holds three cities. The capital is the settler pump of the land grab:
    /// on live King Gran Colombia it trained eight Settlers between turns 6
    /// and 57 (2026-10-01T053931Z) and sat at population 2-3 with no Campus
    /// until turn 66 (none by turn 75 in T050754Z), while science trailed
    /// 15 against 56 at turn 60.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `campus-before-harbor-2`.
    pub(crate) capital_campus_first: bool,
    /// A city's Monument ahead of the military floor and the Settler step;
    /// the capital first sends out two Settlers. This governor reached
    /// the Monument only after the floor, the Settler, the Builders and the
    /// Trader, so across the 14 live King Gran Colombia games of 2026-10-01/02
    /// only 54-57% of our cities older than 20 turns held one from turn 60 to
    /// 100 (27% at turn 30). A new city's first 40 turns went 3.5% to the
    /// Monument against 6.9% to Walls and about a quarter to soldiers and
    /// ships. Culture trailed 10.6 against the rival median's 27.3 at turn
    /// 60 (civics 9.5 against 12), and culture is also what grows a city's
    /// borders: our cities owned three or four unworked plots apiece.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `monument-first`.
    pub(crate) monument_first: bool,
    /// The genome's own Builder quota (`builder_per_city`, at most one per
    /// two cities) ahead of the Monument, the Settler, the military floor,
    /// the recon and the navy, behind only the siege, barbarian and
    /// economic-recovery steps. Stock reaches its Builder step after all of
    /// them, and on the live board that step starves: live King
    /// 2026-10-03T093332Z held zero Builders from t54 to t93 while the lent
    /// war floor (15-20 units for 5-8 cities), Walls, Settlers and Monuments
    /// took every queue. It had improved 5 of 73 owned land plots at t90; its
    /// capital sat at population 2 for 70 turns working two 1-Food mines; and
    /// at t100 it held 29 population and 18.5 Science against Persia's 61
    /// and 88.6. Across 25 live King games, those with no Builder on 20% or
    /// more of their turns (3+ cities, to t150) averaged 34 population and 32
    /// Science at t100, those under 10% averaged 46 and 52.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `builder-before-the-army`.
    pub(crate) builder_before_the_army: bool,
    /// `builder_before_the_army` with a quota of one: the step answers only
    /// while the empire has no Builder standing or queued, the live drought,
    /// and leaves an empire that has one to the stock order. Version 1's
    /// quota moved Builders ahead of the Settler and the first districts in
    /// the simulator too, where the drought does not happen: 16 domination
    /// pairs (seeds 37150000) gave 0.62 fewer districts at t60 (z -2.82), 0.69
    /// fewer cities (z -1.96) and 2.75 fewer citizens (z -1.79) at t100, and
    /// games 11.6 turns shorter (z -2.13).
    ///
    /// Set from `AdvancedAi` by the opt-in gene `builder-before-the-army-2`.
    pub(crate) builder_before_the_army_2: bool,
    /// `builder_before_the_army_2`, and past that first Builder a quota of
    /// one Builder per three worked tiles that stand unimproved but could
    /// take an improvement, at most one per two cities. The extra Builders
    /// come behind the Campus step and never while this city's Settler step
    /// is due (`settler_due`), so they displace only the military floor.
    ///
    /// Live King games since version 2 was armed (2026-10-03T131343Z on, 10
    /// games) worked 11.1 unimproved land tiles against 6.4 improved at t60
    /// and 20.7 against 14.5 at t100 — ten of them hills a Mine pays two
    /// Production on after Apprenticeship — with 1.6 build charges on the map
    /// across t41-80 and none at all on 32% of those turns. ~95% of the
    /// seat's production comes from worked tiles.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `builder-before-the-army-3`.
    pub(crate) builder_before_the_army_3: bool,
    /// `builders-before-the-lent-floor`: above the genome's own army floor,
    /// a unit for a Domination war's lent margin waits for the Builder
    /// backlog. Set from `AdvancedAi`.
    pub(crate) builders_before_the_lent_floor: bool,
    /// `best_military` credits the civilization's own unique unit with
    /// `UNIQUE_UNIT_POWER_CREDIT` strength for the abilities its strength
    /// column omits — the Llanero's adjacency bonus and Gran Colombia's
    /// extra move, the Hoplite's pairing — so a Llanero (62) is trained over
    /// the Line Infantry (65) and the Cuirassier (64) it is unlocked beside.
    /// Live King Gran Colombia, 30 games 10-01..04: not one Llanero at any
    /// checkpoint. The same gene points `AdvancedAi::tech_value`'s +55
    /// unique-unit research credit at `unique_to` instead of civs.json's
    /// `unique_unit`, which names one for only 14 of 105 civilizations.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `unique-unit-preference`.
    pub(crate) unique_unit_preference: bool,
    /// A city's first Campus, then its Library (`first_campus_item`,
    /// `campus_library_item`: each buildable here within
    /// `FIRST_CAMPUS_MAX_TURNS`), ahead of the Monument, the capital Settler
    /// and the military floor, once the empire holds three cities and while
    /// fewer than one city in two is already building either. Stock
    /// reaches its Campus after the floor, and `campus-before-harbor` only
    /// ahead of the Harbor: live King 2026-10-03T103619Z, with Builders
    /// restored, still had 5 of its 6 cities without a Campus from t70 to
    /// t120. Districts took 0% of production in t61-90 and 8% in t91-120,
    /// against 31-35% for soldiers under a floor of 12-16, while our power
    /// stood at 252-370 against the strongest rival's 161-182. Science at
    /// t100 was 21.6 against the top rival's 69.7.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `campus-before-the-army`.
    pub(crate) campus_before_the_army: bool,
    /// `campus_before_the_army`'s Campus and Library behind the Monument and
    /// the capital Settler, and never while this city's Settler step would
    /// train a Settler (`settler_due`), so they displace only the military
    /// floor and what follows it. Version 1 stood ahead of all three. Live
    /// King 2026-10-03T113755Z, its first game, held 3 cities from t42 to
    /// t100 against a target of 10, with its first Settler at t88, while
    /// Campuses, Libraries, Theaters, Builders and ships took the queues. 16 domination pairs
    /// (seeds 37150000) bought +1.44 techs at t100 (z +2.98) and +1.0
    /// Library (z +3.30), and paid 4.8 citizens (z -3.19), 0.62 civics
    /// (z -2.82) and 5.8 Culture (z -2.33): the culture and civics
    /// `monument-first` buys, and the capital's walkers.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `campus-before-the-army-2`.
    pub(crate) campus_before_the_army_2: bool,
    /// `campus_before_the_army_2`, on past the Library to the University and
    /// the Research Lab (each, with its civilization's replacement, within
    /// `FIRST_CAMPUS_MAX_TURNS`). Live King 2026-10-03T131343Z, the first game
    /// with version 2, held a Campus and a Library in all 10 cities at t150
    /// and a University in 4 of them; 5 Universities and 1 Research Lab at
    /// t200, while those cities trained Artillery, Spies and Builders. It
    /// lost a Technology victory at t238, the leader's Science going from
    /// 207 to 416 between t175 and t200 against our 148 and 166.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `campus-before-the-army-3`.
    pub(crate) campus_before_the_army_3: bool,
    /// The navy step (`naval < desired_navy`) waits while this city's
    /// Settler step would train a Settler (`settler_due`). It stands ahead of
    /// the Settler step, so at three cities against a target of ten live King
    /// 2026-10-03T113755Z built two Galleys and two Quadriremes from t80 to
    /// t89 in Cartagena, its first Settlers coming at t88. A replay with
    /// `campus-before-the-army-2` still trained Galleys and a Quadrireme at
    /// t52, t75 and t80 where the walkers were due.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `settler-before-the-navy`.
    pub(crate) settler_before_the_navy: bool,
    /// `housing_reserve_item` (a city within one of its housing: its Granary,
    /// else its Aqueduct within `FIRST_CAMPUS_MAX_TURNS`) ahead of the
    /// military floor, behind the Monument and the capital Settler, and never
    /// while the city is due a Settler. `first-granary-reserve-3` arms the
    /// stock step, which comes after the floor and the Settler, and the
    /// Advanced reserve ahead of the strategic scorer, which a delegated live
    /// seat never runs. Live King at t100: 2026-10-03T155014Z had 5 of 6
    /// cities at their housing and no Granary; 164758Z 3 of 9 with 3
    /// Granaries; 135713Z 4 of 10 with 1. Our cities held 4-5 citizens
    /// against the rivals' 7-9.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `granary-before-the-army`.
    pub(crate) granary_before_the_army: bool,
    /// The military floor's unit is built where it reaches the campaign's
    /// target soonest: a city skips the floor while another city of ours that
    /// can build the same unit would put it at `front_objective` at least
    /// `FRONT_ARRIVAL_MARGIN` standard turns sooner (build turns plus a
    /// straight-line walk at the unit's speed, civilization bonus included),
    /// so long as the army already
    /// holds `FRONT_FLOOR_SHARE` of its floor. The floor stays empire-wide, so
    /// the army is still built, nearer the front. Live King 2026-10-04: during
    /// sieges 69% of 160213Z's land-military city-turns were in cities more
    /// than 12 tiles from the objective (median 15), while a city of ours
    /// stood within 8 tiles on 171 of 189 siege turns and spent about three
    /// quarters of its turns on buildings and districts; 153748Z 52%, with a
    /// near city on every siege turn. The reinforcement walk was a median of
    /// 16 tiles.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `front-weighted-floor`.
    pub(crate) front_weighted_floor: bool,
    /// While the military floor is unmet, a city within
    /// `FRONT_NEAR_TILES` of the campaign's target whose arrival (build plus
    /// walk) is within `FRONT_ARRIVAL_MARGIN` of the best city's builds the
    /// floor's unit ahead of the Granary and Campus steps; cities farther out
    /// keep their order, so the army is never built slower. Version 1 only
    /// stopped far cities, and only while the army held three quarters of its
    /// floor: live King 2026-10-04T235724Z ran at 7-15 units against a lent
    /// floor of 18-30, so it never bound (74% of the siege army still built
    /// more than 12 tiles out). The near cities are the ones to move: in
    /// 160213Z they spent about three quarters of their siege turns on
    /// buildings and districts while the floor stood unmet.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `front-weighted-floor-2`.
    pub(crate) front_weighted_floor_2: bool,
    /// The campaign's target city, set by `AdvancedAi` for
    /// `front_weighted_floor` and `front_weighted_floor_2`; `None` with both
    /// off or no target.
    pub(crate) front_objective: Option<Pos>,
    /// `live_great_person_activation_resume_item` does not take a city whose
    /// queue head is a district already holding production: the district
    /// finishes, then the path resumes. Stock lets any queued district yield,
    /// and another rule that also overrides a queued district then thrashes
    /// with it. Live King 2026-10-04T205431Z: from t251 Bogota alternated
    /// the religious-defense Holy Site ("conversion threatens recruitment")
    /// and the activation path's Theater Square ("Resuming an activation
    /// path for a live Great Person") within each turn's frames, so both
    /// finished about twice as late.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `activation-resume-waits`.
    pub(crate) activation_resume_waits: bool,
    /// A Great Person activation path does not replace a queued building on
    /// any waiting person's slot chain. Stock checks only the chain of the need
    /// it is serving, so two needs that share a Theater Square undo each
    /// other: live King 2026-10-05T051413Z, Bogota (45 production) switched
    /// between the Art Museum (a Great Artist) and the Archaeological Museum
    /// (Mary Leakey) on every turn from t187 to t214, and the two exclude each
    /// other, so each switch dropped the turn's progress and neither was ever
    /// finished. 044201Z did the same in Guayaquil from t174.
    ///
    /// Set from `AdvancedAi` by the HostOnly gene `activation-keeps-its-building`.
    pub(crate) activation_keeps_its_building: bool,
    /// Native activation opt-in: an empty Museum needs a digger, not another Museum.
    pub(crate) artifact_before_more_museums: bool,
    /// `granary_before_the_army`, behind the Campus step and the Builder
    /// backlog instead of ahead of them, so the housing reserve displaces
    /// only the military floor. Version 1 stood ahead of
    /// `campus_before_the_army_2`: 16 domination pairs bought +2.1 citizens
    /// at t100 (z +2.5) and paid 23 Science at t150 (z -2.4) and 0.87
    /// Libraries. Live King games since 2026-10-03T131343Z held 3.9 of 8.4
    /// cities at their housing cap at t100 and 2.6 more within one.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `granary-before-the-army-2`.
    pub(crate) granary_before_the_army_2: bool,
    /// A city at (or one short of) its housing with no Granary builds the
    /// Granary next, ahead of the Builder, Holy Site, Campus, Monument,
    /// capital-Settler, Industry and Settler steps and the military floor
    /// (lent war target included); a city short of Amenities then takes a
    /// cheap Amenity building or a Builder for a luxury the empire lacks.
    /// Never in a city the plan names threatened or that was attacked within
    /// four turns, never during the floor's `emergency_defense`, and never
    /// while the city holds a Settler more than half built. See
    /// `housing_bound_granary_step`.
    ///
    /// Live Emperor G185-G198 (civvis-20261006T024058Z..T053813Z): our
    /// cities held 5.0 citizens at t100 against the best rival's 8.3 while
    /// production per citizen sat at parity; 49% of city-turns t60-120 were
    /// at or within one of the housing cap with no Granary though Pottery
    /// came by ~t22, and those cities built Campus 13%, Settler 11%, Builder
    /// 6% and Library, Walls and Monument 5% each against Granary 5%. 61% of
    /// cities were short of Amenities at t100 (about 0.6-1 each).
    ///
    /// Set from `AdvancedAi` by the opt-in gene
    /// `housing-bound-city-builds-its-granary`.
    pub(crate) housing_bound_city_builds_its_granary: bool,
    /// A city within one of its housing that already has its Granary (or
    /// cannot build one) builds its Aqueduct (Rome's Bath) next, ahead of the
    /// economy steps and the military floor, when the district finishes
    /// within `HOUSING_CAP_AQUEDUCT_MAX_TURNS`. An Aqueduct takes no district
    /// slot and lifts a city's water housing: to 6 without fresh water, by 2
    /// with it. Measured on the 38 Emperor games of October 6-8 at turn 100:
    /// 56% of our cities stood within one of their housing (median population
    /// 6 against housing 6, rivals' median city 10), 113 of those 153 already
    /// had a Granary, and 5 had an Aqueduct; about 45% of them had a legal
    /// Aqueduct site. The armed lineage reaches the same `housing_reserve_item`
    /// too late: `granary-before-the-army-2` only within
    /// `FIRST_CAMPUS_MAX_TURNS` and behind the Campus, the Builder backlog and
    /// a due Settler; `first-granary-reserve-3` (`housing_reserve`) only after
    /// the military floor, the Settler and every district step; and
    /// `housing-bound-city-builds-its-granary` stops at the Granary. At turn 150 the operator retires a game not top 2
    /// by Production (G365 120 against 448, G366 132 against 330).
    ///
    /// Set from `AdvancedAi` by the opt-in gene `housing-cap-builds-the-aqueduct`.
    pub(crate) housing_cap_builds_the_aqueduct: bool,
    /// Builders while the Builder charges in hand and queued cover less than
    /// 60% of the empire's unimproved worked tiles, one per city at most,
    /// from a city that finishes one within eight standard turns, ahead of
    /// the Monument, the Settler step and the military floor (a due Settler
    /// and local defence still win).
    ///
    /// Measured over the 10-06/07/08 Emperor runs (40-42 games): the worked
    /// but unimproved land grew from a median 6 tiles at t40 to 14 at t100
    /// and 15.5 at t120, while the empire held a median ONE Builder with 1-3
    /// charges from t40 to t90 (5 charges at t100) and no city was producing
    /// one. Builders were 5.7 of ~67 production starts t30-t100. The armed
    /// `builder-before-the-army-3` sizes Builders to the same backlog but
    /// counts bodies, not charges, caps them at one per two cities, and sits
    /// behind the Monument, the Settler, the housing reserve and the Campus,
    /// so an idle queue rarely reaches it. Worked-tile Production is the
    /// per-citizen gap: 1.24 a worked tile in the bottom third of games by
    /// Production per citizen against 1.74 in the top third at t100, where 56%
    /// of worked tiles stood improved against 49%. The operator retires a game
    /// at turn 150 that is not top 2 by Production.
    ///
    /// Live G383 and G384 (pins cbc51e678, 353e09be9) fired it 3 and 0 times
    /// by t100 while the backlog ran from ~11 to 21-25 tiles: the idle queues
    /// went to `pick_item`'s earlier Granary, Aqueduct and Settler steps and,
    /// before the delegated governor is ever asked, to the controller's own
    /// idle-queue claims (Plaza, Theater, Entertainment Complex, recon,
    /// development Campus). So under the gene `AdvancedAi` also offers the
    /// Builder one idle queue a turn ahead of those routine claims
    /// (`advanced/worked_backlog_claim.rs`), buys one with Gold above
    /// `spend_gold`'s reserve when no idle city can take it, and lets a slow
    /// city take up to `WORKED_BACKLOG_RUNAWAY_MAX_TURNS` while the backlog is
    /// twice the charges.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `builders-cover-the-worked-backlog`.
    pub(crate) builders_cover_the_worked_backlog: bool,
    /// With `builders_cover_the_worked_backlog`, the backlog quota asks one
    /// whole charge for every unimproved worked tile a production improvement
    /// fits, and half a charge for every good idle one, instead of 60% of the
    /// worked backlog. See `ai/prod_tiles_call_the_builders.rs`.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `prod-tiles-call-the-builders`.
    pub(crate) prod_tiles_call_the_builders: bool,
    /// From turn 90 standard (t60 Online), ahead of the military floor: the
    /// Market of a standing Commercial Hub, then a Trader while the empire's
    /// routes and Traders leave a route slot open, then a Commercial Hub on
    /// its best Gold site in each of the two (three from six cities) most
    /// productive unthreatened cities that lack one and have a free slot --
    /// never more than three hubs standing or queued. Never in the plan's
    /// threatened city or one attacked within four turns, never during the
    /// floor's `emergency_defense` or while a Settler is due, and never in a
    /// city due its Granary. See `commercial_hub_step`.
    ///
    /// Live Emperor G185-G198 (civvis-20261006T024058Z..T053813Z): Gold a
    /// turn against the best rival 0.55x at t50, 0.15x at t100 (net 14
    /// against 63), 0.11x at t125, with unit upkeep 20 -> 42 eating 45-65%
    /// of the cities' gross Gold; no Commercial Hub and no Market in any of
    /// the 13 games at t100 (0.9 hubs at t125, 1.6 at t150), trade route
    /// capacity 1-2 until t150.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `commercial-hub-and-traders`.
    pub(crate) commercial_hub_and_traders: bool,
    /// The strategic plan's threatened city, lent by
    /// `AdvancedAi::delegated_cities` for the call and `None` outside it.
    /// Read only by `housing_bound_granary_step` and `commercial_hub_step`.
    pub(crate) plan_threatened_city: Option<u32>,
    /// `industry-before-the-army`: the Industrial Zone, then its Workshop,
    /// then its Factory, ahead of the military floor for at most a third of
    /// the empire's cities at a time, from three cities on. A city opens a
    /// zone only once it holds its Campus (or cannot build one), never
    /// while its Settler step is due, and only where the zone's best site
    /// earns adjacency and finishes within `FIRST_CAMPUS_MAX_TURNS`. A
    /// Factory goes only to a city no standing Factory already reaches.
    ///
    /// The delegated governor's own district list (`DISTRICT_PRIORITY`)
    /// never names the zone. 28 live King games (2026-10-01..04) held 0.1
    /// zones at t100 and 0.9 at t200 against 7.9 Campuses, 0.2 Workshops
    /// and 0.1 Factories; ~95% of production came from worked tiles, and the
    /// strongest rival out-produced the seat 1.6x at t60-100 and 2x at t200.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `industry-before-the-army`.
    pub(crate) industry_before_the_army: bool,
    /// `industry_before_the_army`, keeping a slot for the Theater and the
    /// treasury: a city opens its zone only while it holds a Theater Square
    /// or has two free specialty slots, and the whole chain waits while the
    /// empire nets under `INDUSTRY_MIN_GOLD_PER_TURN`. Version 1 over 22
    /// domination pairs (seeds 37150000) bought +302 production by t150
    /// (z +2.8) and +1.2 Workshops, and paid 2.2 Gold a turn at t120
    /// (z -2.6), 3.9 more bankrupt turns and 2.3 Culture at t150, with
    /// rivals' Culture victories 12 of 22 against 6.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `industry-before-the-army-2`.
    pub(crate) industry_before_the_army_2: bool,
    /// `industry_before_the_army_2`, only while the standing army already
    /// meets its floor (`military >= military_floor`): the chain then takes
    /// the builds the floor would leave to Builders, Traders, Harbors and the
    /// district list, never a soldier. Version 2 over 32 domination pairs
    /// (seeds 37150000) bought +70 production by t120 (z +3.7) and still
    /// turned 5 rival Science wins into earlier Culture wins (none the other
    /// way), the thinner wartime army leaving the culture leader unpressed.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `industry-before-the-army-3`.
    pub(crate) industry_before_the_army_3: bool,
    /// `industry-in-the-district-list`: the Industrial Zone joins the
    /// delegated governor's own district list (`DISTRICT_PRIORITY`), ranked
    /// between the Campus and the Commercial Hub, and a city holding a zone
    /// builds its Workshop, then its Factory, just ahead of that list. Both
    /// sit behind the military floor, the Settler step and the Theater, so
    /// the zone takes only builds the army leaves: peacetime, and the slow
    /// cities the lent floor already passes over.
    ///
    /// `industry-before-the-army` put the same chain ahead of the floor; 32
    /// domination pairs (seeds 37150000) bought +75 production by t120
    /// (z +4.2) and +1.2 zones, and turned 8 rival Science wins into earlier
    /// Culture wins (none the other way) as the thinner wartime army stopped
    /// pressing the culture leader.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `industry-in-the-district-list`.
    pub(crate) industry_in_the_district_list: bool,
    /// `plaza-in-the-district-list`: the empire's first Government Plaza at
    /// the front of the capital's district list, then the Plaza's Warlord's
    /// Throne and Grand Master's Chapel ahead of the Harbor. The delegated
    /// governor's list was the Campus, Commercial Hub, Holy Site and Theater
    /// only, so the cities it builds for never opened a Plaza: 7 of 30 live
    /// runs (October 5) built one, the earliest at turn 81 and most after
    /// 159, against a district buildable from turns 27-40 at 30 production.
    /// Without it there is no Grand Master's Chapel, the one building that
    /// lets Faith buy land units (`faith_purchase_land_units`), and those runs
    /// ended on 960-3,590 unspent Faith. See `plaza_wanted_here` and
    /// `plaza_building_item`.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `plaza-in-the-district-list`.
    pub(crate) plaza_in_the_district_list: bool,
    /// `kristina-builds-the-bibliotheque`: on Sweden's seat the Government
    /// Plaza's tier buildings are the Ancestral Hall, then the Queen's
    /// Bibliotheque, and the Plaza itself waits neither for a Prophet race
    /// nor for a capital that has no site for it. See
    /// `advanced/kristina_builds_the_bibliotheque.rs`. Set from `AdvancedAi`.
    pub(crate) kristina_builds_the_bibliotheque: bool,
    /// `kristina-fills-to-theme`: on Sweden's seat a Writer, Artist or
    /// Musician with no slot asks for the Queen's Bibliotheque first, and the
    /// works go where they fill a building that Minerva of the North themes.
    /// See `advanced/kristina_fills_to_theme.rs`. Set from `AdvancedAi`.
    pub(crate) kristina_fills_to_theme: bool,
    /// `industrial-hub`: one Industrial Zone, in the city whose zone site
    /// reaches the most own cities within a Factory's regional range (at
    /// least `INDUSTRIAL_HUB_MIN_REACH`), then its Workshop, its Factory and
    /// a power plant the empire holds fuel for, ahead of the military floor
    /// in that city alone and never while its Settler step is due. The
    /// Factory and the plant also wait while the empire's net Gold is
    /// negative. A Factory pays its +3 Production (+3 more powered) to every
    /// own city it reaches: 22 live King games put 5.6 of 8.6 cities within
    /// one hub's reach at t130, about +35 Production for ~930 spent in a
    /// single city, against `industry_before_the_army`'s zone in every city
    /// that thinned the wartime army (rival Science wins turned to Culture
    /// wins 8:0 over 32 domination pairs).
    ///
    /// Set from `AdvancedAi` by the opt-in gene `industrial-hub`.
    pub(crate) industrial_hub: bool,
    /// `industrial-zone-in-the-producers`: once Apprenticeship is in hand,
    /// the empire's two most productive unthreatened cities (three from six
    /// cities) that have a free district slot and are not due their first
    /// Campus place an Industrial Zone on their best Production-adjacency
    /// site, and the most productive zone cities then build its Workshop.
    /// The step stands just behind the industrial hub, ahead of the
    /// Commercial Hub step, the military floor, the district list and every
    /// ordinary building; behind local defence, economic recovery, the
    /// housing Granary, the first-Campus, Builder and housing-reserve steps
    /// and a due Settler, never in the plan's threatened city or one attacked
    /// within four turns, and it gives its slot to the city's standing
    /// Campus's next building (the Library, or under `campus-buildings-first`
    /// that step's building). The strategic governor asks the same step for
    /// an idle queue and keeps a queued zone or Workshop against its review
    /// (`AdvancedAi::advanced_production`), and the culture-defense Theater
    /// reservation leaves a producer's slot to it.
    ///
    /// Live Emperor civvis-20261006T024058Z..T114112Z (42 games): the first
    /// zone came a median 40 turns after Apprenticeship (t80 -> t120), a
    /// second in 23 games at t180; 29 of 41 games held no zone at t100 and a
    /// median 1 zone, 1 Workshop, 0 Factories at t150, at 0.46x the best
    /// rival's Production. The three most productive cities, while a zone was
    /// placeable, built soldiers 44% of the time, ordinary buildings 21%, a
    /// Campus 8%, a Settler 8%. See `industrial_zones.rs`.
    ///
    /// Set from `AdvancedAi` by the opt-in gene
    /// `industrial-zone-in-the-producers`.
    pub(crate) industrial_zone_in_the_producers: bool,
    /// A standing district's first building before the city opens another
    /// district. This governor tried every district the city still lacked
    /// before any building, so a district stood without the building that
    /// makes it pay until the city ran out of district slots. Live King
    /// 2026-10-01/02, turn 100: 15 of 45 Campuses without a Library, 13 of
    /// 19 Commercial Hubs without a Market, 4 of 4 Industrial Zones without
    /// a Workshop; turn 120: 23 of 41 Hubs, 8 of 13 Entertainment Complexes
    /// without an Arena. Science per citizen trailed about 0.9 against the
    /// rivals' 2.0.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `district-buildings-first`.
    pub(crate) district_buildings_first: bool,
    /// The capital's Library ahead of its next Settler once it holds a
    /// Campus, at the slot `capital_campus_first` gives the Campus itself.
    /// `campus-before-harbor-2` opens the capital's Campus there, but its
    /// Library then waited behind the military floor, the Settlers and every
    /// district the city lacked: live King 2026-10-03T040354Z, the best game
    /// of the series, held 7 Campuses and 2 Libraries at turn 100.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `district-buildings-first-2`.
    pub(crate) capital_library_first: bool,
    /// A city whose standing Campus can build its next building -- the
    /// Library, else the University, else the Research Lab, each within
    /// `campus_buildings::CAMPUS_BUILDING_MAX_TURNS` -- builds it before the
    /// city opens another district or takes an ordinary building or a
    /// repeatable project: the step stands just ahead of
    /// `district-buildings-first`, the Harbor and the district list, behind
    /// the military floor and every step above it (local defence, the
    /// housing Granary, the Prophet race's Holy Site, the industrial hub),
    /// the Settler step, the housing reserve, the Builders, the Trader, the
    /// Monument and the Plaza. The Commercial Hub step's second and third
    /// hubs give their slot to it (the empire's first hub, a hub's Market and
    /// the Trader keep theirs), and so does the culture-defense Theater
    /// reservation (`AdvancedAi::reserve_culture_defense_theater`).
    ///
    /// Live Emperor civvis-20261006T024058Z..T095649Z (35 games): Campuses /
    /// Libraries / Universities 4 / 2 / 0 per game at t100 and 7 / 6.5 / 3 at
    /// t150; Education at a median t91 (the rivals' t87), then a University
    /// that started a median 15 turns after it was buildable (p75 32; 41 of
    /// 218 cities never), while those cities spent 17% of the wait on a new
    /// district, 7% on another ordinary building and 4% on a district
    /// project. Science per citizen 1.25 against the rivals' 1.95 at t100.
    /// See `campus_buildings.rs`.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `campus-buildings-first`.
    pub(crate) campus_buildings_first: bool,
    /// No Manhattan Project, Operation Ivy or device from the base picker's
    /// project fallbacks. The live bridge cannot launch a device, so on the
    /// live seat they are dead Production; live Immortal
    /// civvis-20261026T162405Z put three cities on the Manhattan Project at
    /// turn 200 through `pick_item` while the advanced scorer priced it at
    /// -10,000.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `nuclear-goal-needs-a-launcher`.
    pub(crate) nuclear_goal_needs_a_launcher: bool,
    /// A Theater Square ahead of the Harbor and every other district while
    /// the empire's Culture trails the strongest rival's, until half the
    /// cities hold one. Culture buys the civics a government and its policy
    /// slots wait on, and it is one bar of the culture-victory defense: a
    /// seat's domestic Tourists are its lifetime Culture over 100
    /// (`Game::domestic_tourists`), and a rival wins once its foreign
    /// Tourists outnumber the highest domestic count among the others
    /// (`Game::check_culture_victory`). Culture was the commonest live King
    /// loss (17 of 31 in late September); in 2026-09-30T221624Z ours was that
    /// highest count (53) when Byzantium won at 51 and climbing. Our Culture
    /// ran a third to a half of the strongest rival's through the 14 live
    /// games of 2026-10-01/02, which built one Theater Square between them:
    /// `theater_square` is the last `DISTRICT_PRIORITY` family in every bred
    /// genome, so a city reaches it only after the other three.
    ///
    /// Set from `AdvancedAi` by the opt-in gene `culture-defense-theater`.
    pub(crate) culture_defense_theater: bool,
    /// The genome's own `mil_per_city` while `AdvancedAi::delegated_cities`
    /// has lent this governor a Domination war's higher army target, `None`
    /// otherwise. Below the genome's own floor every city still builds the
    /// army. Above it, only a city that finishes its unit within
    /// `LENT_FLOOR_MAX_BUILD_TURNS` standard turns builds toward the lent
    /// margin. In a replay of King civvis-20261001T003717Z under the lent
    /// target of 3.0 a city, every idle city went military. A 4-production
    /// Cali started a Crossbowman due in 23 turns, a 3-production Popayan a
    /// Man-at-Arms due in 27, while 89% of the empire's city-turns were
    /// housing-bound. Those cities grow instead, and the fast cities raise
    /// the army.
    pub(crate) lent_military_floor_base: Option<f64>,
    /// A named domination seat must not let the delegated baseline governor
    /// build a Spaceport or launch project. The King Gran Colombia game won
    /// by Science at turn 229 while its public plan remained Conquest.
    pub(crate) exclude_space_race: bool,
    /// Lent every turn by an AdvancedAi seat with
    /// `domination-spends-the-stock`: the delegated governor's combined-arms
    /// pick takes a melee unit priced in a strategic resource whenever a city
    /// can start one. See `BasicAi::priced_melee_ready`.
    pub(crate) prefers_priced_melee: bool,
    /// Set every turn by an AdvancedAi seat assigned the Diplomatic lane
    /// (`diplomatic_lane_refuses_fossil_power`): the baseline governor builds
    /// no Coal or Oil Power Plant. Under the live `lane-delegates-production-2`
    /// this governor fills an assigned lane's routine queues for the whole
    /// game, so the strategic scorer's veto alone never reached the
    /// `industrial-hub` chain (coal first) or the cheapest-building fallback.
    /// See `fossil_power_excluded`.
    pub(crate) exclude_fossil_power: bool,
    /// Set by a targeted Science controller while its expansion plan still
    /// has a city deficit.  The flag applies to every route through the
    /// activation-item chooser for this turn, so an ordinary city-production
    /// pass cannot reopen a low-impact off-lane path that the activation
    /// reservation correctly deferred.
    defer_low_impact_science_activation_paths: bool,
    /// Choose the pantheon from the land this empire actually holds, instead of
    /// from a fixed order.
    ///
    /// ★★★ THE SHIPPED CHOICE CARRIES NO INFORMATION. The order is a constant,
    /// a pantheon is exclusive, and the deployment profile seats six majors
    /// against a roster of eleven — so every rated game assigns the same six
    /// pantheons to the same six seats, whatever the map looks like. In
    /// Civilization VI the pantheon is a read of the start: Goddess of the Hunt
    /// on a board of Deer and Furs, God of the Open Sky where the pastures are.
    /// Here it is a lookup table.
    ///
    /// With this on, each candidate is priced at what it would pay on the tiles
    /// the empire already owns, and the shipped order becomes a prior that
    /// still decides when the land says nothing. Reachable as
    /// `advanced_pantheon_board`.
    pub(crate) pantheon_reads_the_board: bool,
    /// Promote an Apostle for the job the empire actually has, instead of for
    /// the largest number on the card.
    ///
    /// ★★★★★ BOTH SHIPPED RULES RANK A PERCENTAGE AGAINST A COMBAT NUMBER, AND
    /// THE DEPLOYED ONE PROVES IT KNOWS BETTER.
    ///
    /// There are two promotion choosers. `BasicAi::prepare_unit_formations`
    /// scores by the sum of the ABSOLUTE VALUES of a promotion's effects.
    /// `AdvancedAi::advanced_promotions` — the one the deployed agent actually
    /// runs, because `advanced_units` replaces the Basic unit phase entirely —
    /// scores with `AdvancedAi::promotion_value`, which carries a **calibrated
    /// weight table** precisely because raw magnitudes are not comparable:
    /// `extra_attacks` 70, `range` 55, `movement` 20, `combat_all` 4.0,
    /// `attack_*`/`ranged_*`/`siege_*` 3.5. Not one religious effect appears in
    /// that table, so every one of them takes the `_ => 2.0` fallthrough and is
    /// multiplied by its own raw magnitude. The mechanism that exists to make
    /// units commensurable was never pointed at religion.
    ///
    /// Either way the nine Apostle promotions — a percentage, a gold figure, a
    /// combat strength, hit points and a charge count — are ranked against one
    /// another as bare numbers:
    ///
    /// | promotion | effect | number | unit | `promotion_value` |
    /// |---|---|---|---|---|
    /// | `translator` | `foreign_spread_pct` | 200 | percent | 400 |
    /// | `indulgence_vendor` | `gold_per_conversion` | 100 | gold | 200 |
    /// | `proselytizer` | `rival_pressure_removed_pct` | 50 | percent | 100 |
    /// | `chaplain` | `adjacent_heal` | 20 | hit points | 40 |
    /// | `debater` | `religious_strength` | 20 | combat strength | 40 |
    /// | `pilgrim` | `natural_wonder_charges` | 3 | charges | 6 |
    /// | `orator` | `religious_charges` | 2 | charges | 4 |
    /// | `heathen_conversion` | `convert_barbarians` | 1 | flag | 2 |
    /// | `martyr` | `relic_on_death` | 1 | flag | 2 |
    ///
    /// So the ranking inside any offer is a CONSTANT, in both choosers and for
    /// the same reason: a percent sign is worth ten of a strength point, not
    /// because spreading abroad is worth ten times winning a theological fight.
    /// The second-ranked promotion in the whole set is a GOLD one. The choice
    /// is real — `Game::available_promotions` truncates an Apostle's offer to
    /// three of the nine, and Civilization VI's Apostle takes its single
    /// promotion at birth and never takes another. It is simply never made.
    ///
    /// ⚠ The two promotions the rule can never reach are the DEFENSIVE ones,
    /// which is why this lives in the religion lane. `proselytizer` raises an
    /// Apostle's eviction of rival pressure from 0.25 to 0.75 in
    /// `Game::do_spread` — one spread into one of our own cities removes THREE
    /// TIMES as much of the faith that is taking it — and `debater` is the only
    /// promotion that touches `Game::theological_strength` at all, +20 on a
    /// base 110. Under the shipped rule an empire whose own cities are being
    /// converted promotes its Apostles for spreading abroad.
    ///
    /// The measurement that pointed here: a 300-pair `gene_screen` run at
    /// 4p/60x38/Online-250 ended **66% of its games in a religious victory** at
    /// a median of turn 149, and `docs/EVAL.md` has said since its first
    /// baseline that "the AIs under-invest in religious defense
    /// (inquisitors/theological combat) relative to how hard they push their
    /// own religion."
    ///
    /// ⚠⚠ AND THE DEFENSIVE HALF IS ALMOST UNREACHABLE, which is a finding
    /// about the engine and not about this flag. The Apostle is the **only
    /// promotable religious unit** — Missionary, Guru and Inquisitor all carry
    /// no `promotion_class` at all — and a defending empire buys exactly those:
    /// `religious_spending_with_reserve`'s non-offensive priority is
    /// `["missionary", "inquisitor"]`, and its under-pressure priority puts the
    /// Inquisitor first. So promotion choice can only ever reach an empire that
    /// is SPREADING. A reachability census over four whole games measured the
    /// two situational branches at exactly zero: 24 promotions taken, byte
    /// identical with the flag on and off.
    ///
    /// ⚠⚠⚠ THAT CENSUS ALSO CAUGHT THIS TREATMENT WIRED TO A FUNCTION THE
    /// DEPLOYED AGENT NEVER CALLS, which is the whole reason the cheap question
    /// is asked before the expensive one. The first two versions changed
    /// `prepare_unit_formations` only — and `AdvancedAi::advanced_units`
    /// replaces the Basic unit phase outright, promoting through
    /// `advanced_promotions` instead. Both censuses came back byte identical,
    /// which is exactly what a treatment attached to dead code looks like from
    /// the outside. It is now applied at both choosers.
    ///
    /// What that census did find is the mistake this treatment mostly fixes:
    /// `indulgence_vendor` — a GOLD promotion — was taken **6 of 24 times, as
    /// often as `translator`**, because 100 outranks `proselytizer`'s 50 and
    /// `orator`'s 2. A quarter of the empire's Apostles were promoted to earn
    /// gold per conversion instead of to convert. The offensive branch is
    /// therefore an explicit conversion-first order rather than a fallthrough.
    ///
    /// Off by default and listed in `PRODUCTION_OPT_INS`, so it is measurable
    /// by name before any promotion question is asked.
    pub(crate) apostle_promotion_by_role: bool,
    /// `band-plays-its-stage`: Rock Band promotions and tours priced by the
    /// band's survival. A concert's performance level is the band's level plus
    /// the venue's matching-promotion levels, and that level sets both the
    /// breakup odds and how fast album sales compound (a band's sales add
    /// straight onto every later concert). Live: 86% of 355 concerts ran at
    /// levels 1-2, 80 of 168 bands played once, and the only two-level stage
    /// pairings numbered 3. Off by default; opt-in gene.
    pub(crate) band_plays_its_stage: bool,
    /// `bands-draw-from-the-bar`: a Rock Band tours toward the rivals that
    /// bind the Culture bar. A visitor drawn from the rival with the most
    /// domestic tourists also lowers the bar, so it closes the race twice;
    /// one drawn from a rival far below the bar closes it once. Off by
    /// default; opt-in gene. See `advanced/bands_draw_from_the_bar.rs`.
    pub(crate) bands_draw_from_the_bar: bool,
    /// `band-plays-what-it-can-reach`: a Rock Band never stands idle beside a
    /// venue it cannot get into. Each turn it plays, or steps toward the best
    /// venue it can actually take a step toward (the next one down when the
    /// best one's step is refused), and a trip the host walked nowhere bars
    /// that venue's host for a few turns, so the next turn aims elsewhere. The
    /// promotion chooser counts only stages in land open to our bands and
    /// within the tour's reach. Live 20261110T164254Z: three bands spent 65 of
    /// 102 band-turns standing still at a closed border while the band itself
    /// stood on the only stage it could have played. Off by default; opt-in
    /// gene. See `advanced/band_plays_what_it_can_reach.rs`.
    pub(crate) band_plays_what_it_can_reach: bool,
    /// `bands-wait-at-the-door`: a Rock Band with no stage it can reach waits
    /// on the near side of the border of the closed rival it would most like
    /// to play (the Culture bar holder first, then by the stage), on the
    /// open tile nearest that rival's best stage, instead of at home or
    /// wherever it stopped. Off by default; opt-in gene. See
    /// `advanced/bands_wait_at_the_door.rs`.
    pub(crate) bands_wait_at_the_door: bool,
    /// `band-plays-what-it-can-reach`: each band's trip this turn, keyed by
    /// [`BasicAi::band_memory_key`]: the turn, the tile it stood on when it
    /// first moved that turn, and the venue it set out for.
    band_trips: RefCell<HashMap<i64, (u32, Pos, Pos)>>,
    /// `band-plays-what-it-can-reach`: hosts, by civilization, whose venues
    /// our bands skip until the turn given, because the host refused a trip
    /// toward one of them.
    band_refused_hosts: RefCell<HashMap<String, u32>>,
    /// The advanced live envoy planner has already chosen whether a held envoy
    /// has a productive destination this turn.  Its ancillary baseline pass
    /// must not replace that deliberate bank with a blind "highest count"
    /// placement.
    ///
    /// Off for normal and frozen controllers, which retain the historical
    /// baseline fallback.  The Civilization VI bridge enables it through
    /// [`AdvancedAi::enable_bank_envoys`](crate::ai::AdvancedAi::enable_bank_envoys).
    pub(crate) bank_envoys: bool,
    /// Enforce the live Firaxis rule that a religious unit inherits its
    /// purchase city's majority. Off for the frozen native controllers and
    /// enabled explicitly by the Civilization VI bridge.
    live_religious_purchase_guard: bool,
    /// Rebuild the recon arm when it is gone and there is still ground to
    /// chart. Off for the frozen native controllers. See
    /// `recon_is_the_missing_arm`.
    pub(crate) recon_replacement: bool,
    /// Buy one ship for an empire that has none while unexplored water lies
    /// off its coast, and let the advanced controller send it exploring.
    ///
    /// ★★★★ ONE OF FIVE RIVALS MET BY TURN 142, THIRTEEN PERCENT OF THE MAP
    /// SEEN. Run civvis-20260816T110555Z (Continents, 3,404 plots): the land
    /// recon arm charted its own continent — 251 land plots — and stopped at
    /// the shore; the empire held Sailing, Shipbuilding, Celestial Navigation
    /// and Cartography, and in 142 turns built exactly one Galley (t91–t101,
    /// dead at sea in ten turns). The other four rivals, the eventual winner
    /// among them, stayed unknown; no trade route, no delegation, no coastal
    /// site across the strait, and `fog_land_capacity` had nothing to read.
    /// `recon_is_the_missing_arm` asks only about ground a scout could walk to
    /// — correct for the scout, blind for the sea. Off for the frozen native
    /// controllers; on for the live bridge and the native repair bundle. See
    /// `naval_recon_is_the_missing_arm` and `AdvancedAi::naval_explorer`.
    pub(crate) naval_recon: bool,
    /// Version 2 of `naval_recon`: two peacetime eyes instead of one while
    /// unseen water remains. One Galley charted one coast and died; the
    /// second hull is the other direction, and the arm is rebuilt on loss
    /// exactly as before. Implies version 1; its enable turns version 1 off.
    /// Opt-in gene `naval-recon-2`.
    pub(crate) naval_recon_2: bool,
    /// Version 3 of `naval_recon`: retain one peacetime eye, but yield its
    /// idle production reservation to a simultaneously missing land scout.
    /// The wartime second-eye exception is unchanged. Opt-in gene
    /// `naval-recon-3`.
    pub(crate) naval_recon_3: bool,
    /// When the capital's landmass is nearly full, chart water toward known
    /// foreign landfalls rather than treating every coast as interchangeable.
    /// Opt-in gene `island-exploration`.
    pub(crate) island_exploration: bool,
    /// While a living major is unmet or unlocated, or a city-state is unmet,
    /// keep the one queued sea scout against the governor's rescoring, route
    /// ships toward foreign landfall, and prefer the faster hull. Opt-in gene
    /// `contact-fleet`; see `advanced/contact_fleet.rs`.
    pub(crate) contact_fleet: bool,
    /// In peacetime the whole field army answers home threats, and a camp
    /// inside the camp reach ranks above raiders in the countryside.
    ///
    /// ★★★★ A CAMP SEVEN TILES FROM ROME STOOD FOR 130 TURNS WITH THE REACH
    /// ON. Run civvis-20260816T200454Z: the camp at (1,43) was on the board
    /// from t20 (#1786) and inside the nine-tile reach (#1789), the barbarian
    /// seat was admitted as an enemy at home every turn, and still no unit
    /// ever walked toward it — the second city came at t39, the third at t68,
    /// three Settlers were carried off within sight of the capital, and the
    /// game read 381 against 609 at t154. Two gates in
    /// `barbarian_response_objective` did it. The recall budget is HALF the army
    /// with the garrison charged against it: a three-unit peacetime army with
    /// one unit holding the capital has a cap of ZERO responders, and five
    /// units yield one. And the camp ranks below every raider inside the
    /// six-tile ring — which, with the camp raising a new raider every few
    /// turns, is always. The one responder there was chased raiders for a
    /// hundred turns while the tap ran.
    ///
    /// The half share exists so an empire at war does not strip its
    /// offensive to chase raiders. In peacetime — the enemy list holds only
    /// the barbarian seat — there is no offensive to protect, so every field
    /// unit not garrisoned or healing may answer. And the camp ranks as a
    /// raider three tiles out: below anything at the gates, above the
    /// countryside. The party is sized to the camp's visible defender rather
    /// than to zero, so a spearman-held camp draws two units, not one that
    /// declines the attack forever. Off for the frozen native controllers;
    /// on for the live bridge and the native repair bundle.
    camp_party: bool,
    /// Keep the land army out of the water: exclude water from a land unit's
    /// exploration goals, bring an already-embarked unit ashore whether or not
    /// it has an upgrade waiting, and let `peacetime_step` know when
    /// `military_step` fell through to it *at war*.
    ///
    /// Measured across 133 live runs: land combat units spend a mean 15% of
    /// their unit-turns embarked (p90 48%, worst run 84%), and **21.7% mean —
    /// 92.8% at worst — while one of our own cities is taking damage**. An
    /// embarked unit cannot attack and defends at `embarked_strength`. See
    /// `disembark_step`.
    ///
    /// Off for the frozen native controllers, whose recorded ladders would
    /// otherwise shift underneath them, and enabled explicitly by the
    /// Civilization VI bridge.
    pub(crate) come_ashore: bool,
    /// A settler in the barbarians' hands is plunder to take back, not a
    /// duplicate to refuse: it is exempt from the duplicate/unusable-settler
    /// guard in `military_step`, outranks every other adjacent capture, and a
    /// capturable barbarian-held civilian within a few tiles is walked onto
    /// (`pursue_capturable_civilian`) rather than watched from adjacency.
    /// Major-owned settlers keep the guard. Gene `barbarian-settler-capture`
    /// (`Kind::Repair`), set through `AdvancedAi::enable_barbarian_settler_capture`.
    ///
    /// The second time this has been written. #2075 shipped it as
    /// `civilian_rescue` and #2509 cut it on the 2026-08-25 directive; on live
    /// run `civvis-20260826T194422Z` our own settler, taken at t65, then
    /// wandered unguarded 2–4 tiles from the capital for twenty turns while
    /// Heavy Chariots with full movement stood beside it and were told to
    /// fortify. The guard fired because `counts().settlers` also counts a
    /// settler at the head of a build queue, which an expansion seat nearly
    /// always has. And the 273 live runs that carried #2075 show it never
    /// captured either: 278 adjacent-with-moves turns, 65 bare `MOVE_TO`
    /// orders at the settler's tile, zero captures — the host will not walk
    /// onto an enemy without the attack modifier, so the capture now crosses
    /// as the `CAPTURE` verb (see `civvis_orders::translate`).
    pub(crate) barbarian_settler_capture: bool,
    /// Rank loyalty emergencies by TURNS TO FLIP rather than by level. Off for
    /// the frozen native controllers, enabled by the Civilization VI bridge.
    /// See `loyalty_emergency`.
    loyalty_rate_alarm: bool,
    /// Record every tactical step through `path_move` instead of applying it
    /// raw, so a unit stepped a second time in the same turn cannot walk back
    /// onto the tile it just left.
    ///
    /// **Off by default, live-bridge only**, like the other tactical adapters.
    /// A raw `g.apply(Move)` records nothing, so the
    /// same-turn reversal guard inside `path_move` never sees the first step;
    /// the second step is then free to undo it. Net zero ground, two emitted
    /// orders, and Civilization VI refuses the second as a MOVE_TO of the
    /// unit's own tile — 217 of 217 refused moves on run
    /// `civvis-20260801T224944Z` were exactly that pair. Gated because the
    /// call sites are shared with the frozen `advanced_v1` anchor, whose
    /// recorded ladders must keep replaying move-for-move.
    recorded_tactical_step: bool,
    /// Count a unit's livelock history once per Civilization VI turn, not once
    /// per live replan frame. The bridge can ask again after an order lands;
    /// that second snapshot sees the same host turn and used to accelerate a
    /// premature stand-down. Native play never supplies
    /// those mirror frames, so it remains off outside the live bridge.
    live_motion_turn_accounting: bool,
    /// Refuse a step back onto **any** tile this unit has already stood on
    /// this turn, not merely the one it just left.
    ///
    /// ★★★★★ THE SHIPPED GUARD IS ONE STEP DEEP AND THE LOOPS ARE THREE.
    /// `recorded_tactical_step` remembers a single previous tile, so `A -> B ->
    /// A` is refused and `A -> B -> C -> A` is not. Measured over three
    /// 6-player 200-turn games, the `live` bundle walks 329 routes that end on
    /// the tile they began: **23** are the two-hop shape the shipped guard
    /// covers and **294** are three hops or more. Every one of them spends a
    /// unit's whole turn to arrive nowhere, and on the Civilization VI bridge
    /// the wasted step is also a dropped order.
    ///
    /// Off for native tournament games, whose recorded ladders replay
    /// move-for-move, and off for the frozen anchor. On for the live bridge
    /// and for the native repair bundle — a loop back to the start is a wasted
    /// turn in either engine, so this is not a Firaxis semantic.
    whole_turn_backtrack_guard: bool,
    /// Drop attack candidates the engine will refuse — invisible target, no
    /// line of sight, wrong melee domain, unpayable entry cost — before they
    /// are scored, so a doomed order cannot win the argmax and shadow a legal
    /// one. `military_step` scores candidates statically rather than on a
    /// clone, which is how a refused shot could outscore a real one: a census
    /// of one 150-turn deployment game counted 519 authoritative combat-order
    /// refusals, every one from that candidate loop (see
    /// `Game::ranged_order_is_legal`).
    ///
    /// Off for the frozen identities — the `advanced_v1` anchor's legacy
    /// base, the dated `basic` tournament entrant, and the evaluator
    /// controls — whose recorded ladders must keep replaying the historical
    /// candidate set. Production `AdvancedAi` turns it on in
    /// `promoted_policy_envoy`; `advanced_unscreened_candidates` in
    /// `src/elo.rs` is the withholding twin that keeps the axis measurable.
    pub(crate) legal_tactical_candidates: bool,
    /// Let a controller retain one unit's campaign objective, dangerous
    /// approaches, and a short retreat commitment across turns.
    ///
    /// Off for Basic and the frozen `advanced_v1` anchor, and since 2026-08-14
    /// off for production Advanced too (the war-half removal; see
    /// `AdvancedAi::promoted_policy_envoy`). Evaluator arms opt in through
    /// `AdvancedAi::enable_unit_objective_memory`.
    unit_objective_memory: bool,
    /// Evacuate a unit when the exact attacks the enemy can make next turn
    /// are lethal, and use those same envelopes to choose recovery ground.
    ///
    /// Basic and the current Advanced controller use this protection. The
    /// frozen `advanced_v1` replay explicitly withholds it so its historical
    /// decision stream remains a stable control.
    precise_evacuation: bool,
    /// `veterans-withdraw-early`: a promoted unit reads its hit points with
    /// a margin in hand — `VETERAN_HP_PER_PROMOTION` per promotion, at most
    /// `VETERAN_HP_MARGIN_CAP` — so the withdrawal line, the one-blow test
    /// and the lethal-pool retreat all fire while it can still walk away.
    /// Experience only compounds on a unit that survives; a green unit
    /// reads its hit points as before. Set by `AdvancedAi`'s toggle.
    pub(crate) veteran_retreat_margin: bool,
    /// `ranged-hp-reserve`: a ranged, non-siege unit reads the lethal pool
    /// with `RANGED_HP_RESERVE` in hand, so it leaves a tile the enemy could
    /// nearly kill it on and keeps shooting from the next one. Only the
    /// lethal-pool test — the withdrawal line stays `one_shot_recovery`'s.
    pub(crate) ranged_hp_reserve: bool,
    /// A unit one enemy blow from death withdraws to safe healing ground, and
    /// leaves that ground again the moment an enemy can strike it.
    ///
    /// `withdraw_hp` is a constant, and a constant cannot know how hard the
    /// thing across the river hits. This reads the board instead: the largest
    /// single blow anything visible could land on this unit where it stands,
    /// at the top of the engine's damage roll, is the hit point total at which
    /// it must be somewhere else. It rides on the same envelopes
    /// `precise_evacuation` already computes, and pairs with it — with that
    /// protection off, the fetch below is a fresh flow field per unit.
    ///
    /// Off by default and listed in `PRODUCTION_OPT_INS`, so it is measurable
    /// natively before any promotion question is asked.
    one_shot_recovery: bool,
    w: Weights,
    book_pos: usize, // opening-book progress (capital builds played so far)
    /// The opening book's Settler slot, held back because the capital was
    /// below the host's population floor when its turn came. Played the turn
    /// the capital grows, ahead of whatever filler took the slot. See
    /// `opening_settler_waits` and `play_pending_book_settler`.
    book_settler_pending: bool,
    /// Units that have withdrawn from combat stay in recovery until they are
    /// healthy enough to rejoin it, instead of advancing again after one tick.
    recovering_units: HashSet<u32>,
    /// Persistent peacetime destinations keep surplus troops patrolling the
    /// empire's frontier instead of permanently stacking around the capital.
    patrol_targets: HashMap<u32, Pos>,
    /// Frontier posts are identical for military units in the same movement
    /// domain. Reuse the scan for the rest of this player's turn instead of
    /// walking the whole map once per idle unit.
    patrol_posts: HashMap<String, Vec<Pos>>,
    /// Parallel unit snapshots can be primed before they are cloned. Keep
    /// those primed scans separate by full traversal class so a Scout that can
    /// embark does not change the post list used by a land-only unit.
    patrol_posts_by_class: HashMap<(String, TraversalClass), Vec<Pos>>,
    /// Colonies, especially overseas ones, need a fixed destination. Re-scoring
    /// only a short local radius each step strands settlers on shorelines and
    /// can make them reverse course after embarking.
    settler_targets: HashMap<u32, Pos>,
    /// The complete objective/threat/retreat ledger for individual units.
    /// `RefCell` lets both serial and snapshot tactical planners update only
    /// their own unit's entry without changing the broad movement API.
    unit_memories: RefCell<BTreeMap<u32, UnitMemory>>,
    /// The last hostile attack envelopes computed, with the exact board they
    /// belong to. `enemy_attack_envelopes` was recomputed for every one of the
    /// seat's units on every turn — one `attack_reach` flow field per visible
    /// enemy per own unit — and #2059 (2026-08-18) took a 6-player 74×46
    /// 150-turn Online game from 16.7 s to 102.7 s on that alone.
    ///
    /// Shared across clones on purpose: the frontier plans each unit on a
    /// fresh clone of this controller and of the board and drops both, so a
    /// per-clone memo would be computed once per unit and thrown away — which
    /// is what the profile showed after a first, per-clone version of this
    /// cache. Every unit of one frontier batch is planned against the same
    /// board, so the first to compute serves the rest; every reader checks
    /// the key, so a worker that has applied an action recomputes.
    attack_envelope_cache:
        std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<AttackEnvelopeCache>>>>,
    /// Per enemy, its last envelope. Consulted only when the board key missed,
    /// and reused when [`Self::envelope_board_change`] says nothing inside that
    /// enemy's radius moved. See [`EnemyEnvelope`].
    enemy_envelope_cache: std::sync::Arc<std::sync::Mutex<HashMap<u32, EnemyEnvelope>>>,
    /// The board those envelopes were measured against. See [`EnvelopeBoard`].
    envelope_board: std::sync::Arc<std::sync::Mutex<Option<EnvelopeBoard>>>,
    /// Every tile any hostile envelope covers, unioned once and kept against
    /// the exact envelope table it was built from. See [`Self::covered_tiles`].
    covered_tiles_cache: std::sync::Arc<std::sync::Mutex<Option<CoveredTiles>>>,
    /// Safe recovery destinations, reused across this seat's own recovering
    /// units while the board state and hostile-envelope table stay identical.
    /// NOT inherited by a cloned AI -- see `FreshOnClone`.
    safe_healing_targets_cache: FreshOnClone<SafeHealingTargetCache>,
    /// Keep the hostile-envelope table across this seat's own unit moves.
    ///
    /// The exact key (see `attack_envelope_fingerprint`) covers every unit's
    /// place, own units included, because an own unit's zone of control ends
    /// an enemy's move — so a serial path that moves one unit between two
    /// asks recomputes every enemy's `attack_reach`, and the exact cache
    /// serves the frontier batches and little else (a deterministic 6p 40×30
    /// 80-turn game: 4,301 computes for 6,028 asks). With this on, the
    /// fingerprint leaves the viewer's own units out: the envelopes read the
    /// enemy's next turn against the positions this pass STARTED with, and
    /// are refreshed the moment an enemy moves, dies or appears, a city
    /// changes hands, or the map changes. The difference from exact is one
    /// tile of zone of control on the enemy's side of a unit that just moved
    /// — an estimate of a turn the enemy has not taken yet, against
    /// positions this pass is still changing.
    ///
    /// Measured 2026-08-19 (`speed_ab.py` shape, `ci`, one game): 67 s → 34 s
    /// and 87 s → 39 s beside the exact key, on a controller whose reports
    /// therefore differ. Evaluator arm `advanced_envelope_own_moves` prices
    /// it, and the gate has answered: `--matrix --pairs 40` RETAINED the
    /// exact key — parity on the fieldless profiles, but 43.8 % on the
    /// contested one (11 of 80 games against 21, twelve maps to two, sign
    /// p = 0.013). Off in production; the arm stays so the exact algorithmic
    /// route (see docs/SIMULATOR_PERFORMANCE.md, 2026-08-19) can be measured
    /// against it.
    envelope_cache_across_own_moves: bool,
    /// The source of each generic path step taken this turn. Do not immediately
    /// traverse the same edge backward: a greedy step into a cul-de-sac would
    /// otherwise be undone by A* with the unit's next movement point, and the
    /// identical round trip would repeat forever.
    /// Per unit: the turn, and every tile it has stepped *from* during that
    /// turn, in order. The last entry is the step it just took, which is all
    /// the shipped reversal guard reads; the rest of the trail is what
    /// [`BasicAi::whole_turn_backtrack_guard`] reads.
    last_path_step_from: RefCell<HashMap<u32, (u32, Vec<Pos>)>>,
    /// Give up an exploration target the host will not move the unit toward.
    ///
    /// ★★★★ AN ACCEPTED MOVE THAT NEVER MOVES IS INVISIBLE TO EVERY DETECTOR.
    /// The live mirror marks unrevealed plots as a bounded, traversable
    /// frontier so exploration can aim outward; Civilization VI accepts
    /// `MOVE_TO` toward such a plot (no `move_refused`) and then, if the plot is
    /// really ice, ocean or a mountain behind a hill, moves the unit nowhere.
    /// Run civvis-20260816T040537Z: the only Scout stood on (13,40) from t15 to
    /// t58 ordered `MOVE_TO (12,42)` on 34 consecutive turns, revealed nothing,
    /// and the empire met the neighbour that took its capital at t79 by being
    /// attacked — 172 plots revealed by t50 against 247 in the best game. The
    /// livelock detector deliberately ignores a footprint of one tile ("a unit
    /// that has stopped is a different problem"), so nothing ever fired.
    ///
    /// Set only through `AdvancedAi::enable_explore_dead_targets` (the
    /// Civilization VI bridge); a native engine moves what it accepts, so
    /// native constructors and the frozen anchor keep the plain goal.
    pub(crate) explore_dead_targets: bool,
    /// Per unit: the exploration goal it was last sent at, where it stood, how
    /// many consecutive turns it has stood there aiming at that goal, the last
    /// game turn observed, the closest it has come to that goal, and how many
    /// turns running it has failed to come closer. Replanning does not advance
    /// the clock.
    explore_last: RefCell<HashMap<u32, ExploreProgress>>,
    /// Per unit: exploration goals proved unreachable, with the turn each
    /// expires. See `explore_dead_targets`.
    explore_dead: RefCell<HashMap<u32, HashMap<Pos, u32>>>,
    /// Hold an exploration goal until it is reached, revealed, written off or
    /// `EXPLORE_COMMIT_TURNS` old; choose it from twice the fog lookahead,
    /// farthest from home among the most revealing; keep out of the ground
    /// around another explorer's committed goal.
    ///
    /// ★★★★ THE LONE SCOUT PACED A TEN-BY-TEN BOX FOR FORTY TURNS. Live run
    /// civvis-20260816T123936Z: the only Scout stood inside (41–50, 25–36) from
    /// t34 to t75 — (46,27)→(45,27)→(44,26)→(45,26)→(43,25)→(43,25)→(45,26)→
    /// (46,27) is one week of it — moving 1.21 tiles a turn over its life, and
    /// the empire had revealed 197 plots at t50 and 391 at t100 of 3,404. It
    /// met its second city-state at t99 and its first major at t90; the twelve
    /// Settler games of 2026-08-15/16 met 0–3 city-states by t50 and 2–5 by
    /// t100 of the nine on the map. Every first meeting is a free envoy, every
    /// tribal village a free reward, and every unmet major a war we cannot see
    /// coming.
    ///
    /// The stock goal is re-derived every turn from the first fogged ring plus
    /// four, ties to the nearest — so a goal two tiles into the fog is inside
    /// the scout's own sight, revealed by the walk toward it, and replaced by
    /// whatever fog is nearest from the new tile, which is as often behind as
    /// ahead; and a scout that has just stepped out of a barbarian's reach is
    /// aimed straight back at the same fog behind it, which is the other half
    /// of the box (11 flight steps in that run's t36–47).
    /// With this on, a chosen goal is held (`explore_goal`) until it is
    /// reached, revealed, written off (`explore_dead_targets`), within
    /// `EXPLORE_COMMIT_THREAT_RADIUS` of a visible hostile, or twenty turns
    /// old; a fresh one is picked from eight rings past the first fogged one.
    /// In a live verification game, a recon unit with city-states still unmet
    /// instead gets twenty rings: the most revealing first and, among those,
    /// the one FARTHEST FROM HOME — so the walk sweeps outward and along the
    /// frontier instead of hugging it. The long contact horizon is checked
    /// against a real route before it is held: a Scout does not spend twenty
    /// turns trying to cross known closed borders. Ground around a visible
    /// hostile is not a goal; and ground within four tiles of another own
    /// explorer's held goal is left to that explorer.
    ///
    /// Measured (`explore_commit_sweeps_more_ground`, 16 six-seat Continents
    /// boards at the live size, seat 0 the bridge controller, everyone else
    /// stock): revealed plots at t30/t50/t70 **208/333/474 against 181/284/384**
    /// (+15%/+17%/+23%), city-states met 1.62/2.56/3.31 against 1.69/2.25/3.06,
    /// majors met level. Commitment alone measured level with stock
    /// (178/277/387) and commitment + depth + outward without the threat rule
    /// 180/289/419: the box is the flee-and-return loop as much as the churn.
    /// Commitment is on for production Advanced (`promoted_policy_envoy`) and
    /// the Civilization VI bridge (`AdvancedAi::enable_explore_commit`); the
    /// extra contact expedition additionally requires the bridge's
    /// `explore_dead_targets` guard, so native screens retain their established
    /// cost profile. Basic and the frozen `advanced_v1` anchor keep the stock
    /// nearest-fog rule.
    pub(crate) explore_commit: bool,
    /// Per unit: the committed exploration goal and the turn it was chosen.
    /// See `explore_commit`.
    explore_goal: RefCell<HashMap<u32, (Pos, u32)>>,
    /// An explorer walks the unseen ring of a natural wonder within settling
    /// range of an own city before it picks a frontier, so a site beside the
    /// wonder exists to be priced: on run civvis-20260825T162542Z Mount
    /// Roraima stood three tiles from Rome and the four tiles a city could
    /// stand on beside it were still fog at turn 41, because the wonder blocks
    /// sight through itself and `explore_commit` prefers the deepest fog
    /// farthest from home. Opt-in gene `wonder-ring-recon`
    /// (`AdvancedAi::enable_wonder_ring_recon`); the goal search is
    /// `wonder_ring_goal` in `advanced/wonder_sites.rs`.
    pub(crate) wonder_ring_recon: bool,
    /// Settlers the opening's barbarian defense has displaced on a live
    /// board. See `OPENING_DEFENSE_SETTLER_PREEMPTS`.
    pub(crate) opening_defense_preempts: u32,
    /// `opening-defense-preempts-once`: on a live board the opening's
    /// barbarian defense displaces a queued Settler at most once rather than
    /// `OPENING_DEFENSE_SETTLER_PREEMPTS` times. See
    /// `opening_defense_settler_preempts`. Opt-in gene, off by default.
    pub(crate) opening_defense_preempts_once: bool,
    /// Version two preserves every wonder pocket V1 would scout, but among
    /// goals no more than one tile beyond V1's nearest it picks the tile that
    /// reveals the most of the pocket. It therefore buys information without
    /// turning the local errand into a longer expedition. Opt-in gene
    /// `wonder-ring-recon-2`; see `wonder_ring_goal_2` in
    /// `advanced/wonder_sites.rs`.
    pub(crate) wonder_ring_recon_2: bool,
    /// Walk an exploring unit onto a tribal village it can see and reach this
    /// turn before it marches past toward fog. A village is an economy prize
    /// (techs, boosts, builders, envoys, era score) that rivals consume first
    /// when unclaimed. Production Advanced turns this on; Basic and the
    /// frozen `advanced_v1` anchor keep the historical rule.
    pub(crate) hut_collection: bool,
    /// Walk an exploring unit toward a tribal village it has already charted
    /// but cannot reach this turn (within `VILLAGE_SEEK_RADIUS`), before
    /// opening more fog. `hut_collection` only ever grabs a village inside
    /// the current turn's reach, so a sweep that passed two tiles wide of one
    /// left it for a rival. Production Advanced turns this on; Basic and the
    /// frozen `advanced_v1` anchor keep the historical rule.
    pub(crate) village_seeking: bool,
    /// A barbarian raider on water counts toward `major_naval_war`, so the
    /// wartime second eye arrives during exactly the raid that sinks a lone
    /// explorer — the `desired_navy` test always counted barbarians, this arm
    /// did not. The dedicated barbarian response already matches responders by
    /// domain, so it needs no toggle. Entrant `advanced_sea_answers`; off in
    /// production pending its screen.
    pub(crate) sea_answers: bool,
    /// Deliberate camp clearing as a peacetime errand. The stock native
    /// controller has no longer-range camp errand, so every camp's 50-gold bounty,
    /// its Ancient–Medieval era score, and its Military Tradition / Bronze
    /// Working boost progress sit uncollected while a fortified guard holds
    /// a tile beside the capital for the whole game. This flag funds a
    /// bounded errand instead: at most `CAMP_BOUNTY_PARTY` claimed hunters,
    /// peacetime only, camps within `camp_radius()` of home, priced by the
    /// same exchange gate as every other attack, recon excluded. Entrant
    /// `advanced_camp_bounty`; off in production pending its screen.
    pub(crate) camp_bounty: bool,
    /// Walk onto a visible, undefended barbarian camp one legal step away —
    /// the clear IS the move, so no attack scan ever offers it, and without
    /// this a unit ends its turn beside a free 50-gold clear until the camp
    /// spawns the archer that kills it. Unlike `camp_bounty` there is no
    /// claim, no march, and no exchange gate: it fires only when the clear
    /// is immediate. Default-ON; OFF on the frozen anchor
    /// (`AdvancedAi::legacy`) so this controller treatment does not itself
    /// move the rating experiment; withheld for pricing by
    /// `advanced_without_adjacent_camp_clear`.
    /// ★★★★★ OUR OWN WALKERS ARE HOME GROUND TOO, AND EIGHT SETTLERS PAID
    /// FOR THE FACT THAT THEY WERE NOT.
    ///
    /// The admission test that lets the barbarian seat into a unit's enemy
    /// list at all — `barbarian_presence_at_home_with_camp_radius` — measures
    /// distance **from our cities**: a raider within six tiles of one, or a
    /// camp within `camp_radius`. A Settler walking ten tiles to its site is
    /// outside every one of those rings, so the raider shadowing it is in
    /// nobody's enemy list, no escort ever scans it as a target, and the walk
    /// ends with the Settler in barbarian hands. Live run
    /// civvis-20260821T130446Z lost **eight Settlers, two Builders, a Scout,
    /// two Warriors, a Slinger and an Archer in 104 turns** to exactly that.
    ///
    /// This flag adds the second reading: a barbarian military unit within
    /// `HOME_THREAT_RADIUS` of one of our civilians in the field admits the
    /// seat as well. Nothing downstream changes — `nearest_enemy`'s near-home
    /// and exchange-score gates, the tactical scan's pricing, and the
    /// half-army recall cap all still apply — so this widens WHO may be shot
    /// at, never how recklessly.
    ///
    /// ★★★ THE TRADE OBJECTIVE WAS FAIRNESS, NOT PROFIT. `bilateral_trade`
    /// and its Culture-lane twin chose the quote that maximised
    /// `min(our gain, their gain)` — the most *balanced* exchange on the
    /// board — and threw away the ordering `Game::quick_deals` had already
    /// produced by our gain. Operator, 2026-08-24: "very bad deals and just
    /// giving stuff away when more optimally we'd get more in exchange".
    /// With this on, a quote is chosen by our own net value; the engine's
    /// `validate_trade` still refuses anything the counterparty would lose
    /// on. Opt-in gene `deals-for-our-gain`.
    pub(crate) deals_for_our_gain: bool,
    /// ★★★ EVERY PEACETIME QUOTE SPLIT THE SURPLUS DOWN THE MIDDLE.
    /// `Game::quote_asset_trade` prices an asset at the midpoint between our
    /// walk-away and the counterparty's, so we never kept more than half of
    /// what an exchange was worth — while `war_eve_quote` proves the engine
    /// can price at the buyer's ceiling, and did, only on the eve of a
    /// declaration. With this on, the chosen quote's Gold is moved to the
    /// counterparty's walk-away less [`Self::DEAL_CEILING_MARGIN`] — a sale
    /// asks for more, a purchase pays less — and the shipped midpoint quote
    /// is the fallback if the sharper one is refused. Opt-in gene
    /// `deals-at-the-ceiling`.
    pub(crate) deals_at_the_ceiling: bool,
    /// ★★ ONE-WAY PASSAGE RODE FREE ON EVERY FRIENDSHIP ASK. Both alliance
    /// proposals bundled `open_borders` once Early Empire was in, and
    /// `do_accept_deal` grants borders from the proposer to the recipient —
    /// so every friendship or alliance ask handed out passage through the
    /// whole empire, priced at 9–33 Gold by our own book and the tourism
    /// vector `audit.rs` names as a gift, for nothing in return; a
    /// counterparty that wants a friendship accepts it without the rider.
    /// With this on, proposals carry no Open Borders; passage is sold
    /// through the quote lane like any other asset. Opt-in gene
    /// `no-free-passage`.
    pub(crate) no_free_passage: bool,
    /// ★★★★★ WE DO NOT LOSE THE FIGHTS. WE DO NOT TAKE ENOUGH OF THEM.
    ///
    /// MEASURED across every live run since the melee bridge was repaired
    /// (#2223), counting `combat` events:
    ///
    /// | | attacks | kills | attacker died |
    /// |---|---:|---:|---:|
    /// | ours, melee | 225 | 119 (53 %) | **6 (2.7 %)** |
    /// | ours, ranged | 65 | 34 (52 %) | **0** |
    /// | barbarians, melee | 479 | | |
    /// | barbarians, ranged | 388 | | |
    ///
    /// Our attacks are good: better than even odds of a kill, and the attacker
    /// dies once in thirty-seven. **The barbarians simply attack us three times
    /// as often — 867 against 290.** The early window is a 0.71 exchange
    /// (44 kills for 62 losses over seven runs) not because our swings are bad
    /// but because we take so few of them.
    ///
    /// `attack_threshold` prices a fight the way a major-war fight is priced.
    /// A barbarian is not that: there is no war weariness, no retaliation to
    /// invite, no peace to lose, the camp respawns whatever we do, and killing
    /// the unit is the only exit from the zone-of-control lock that strands our
    /// Settlers. So a trade that is marginal against Persia is worth taking
    /// against a raider, and this flag says so with one number.
    ///
    /// ⚠ Deliberately NOT a licence for Recon: the doctrine term already adds
    /// +14 for a scout, so a −8 discount still leaves it +6 and scouts keep out
    /// of fights. The barbarian controller never gets it (`!self.barb`).
    ///
    /// Entrant `live_without_barbarian_bargain`; treatment `barbarian-bargain`.
    pub(crate) barbarian_bargain: bool,
    /// ★★★★★ WE ANSWER SLINGERS AND ARCHERS WITH SPEARMEN.
    ///
    /// `barbarian_defense_item` asks for a MELEE-capable land defender
    /// (`best_military(.., Some(false))`) whatever the ring is made of. MEASURED
    /// across every live run since the melee bridge was repaired:
    ///
    /// | | attacks | share |
    /// |---|---:|---:|
    /// | barbarian melee | 479 | 55 % |
    /// | **barbarian ranged** | **388** | **45 %** |
    /// | our melee | 225 | 78 % |
    /// | **our ranged** | **65** | **22 %** |
    ///
    /// They field twice our proportion of ranged, and ranged is the safer half
    /// of the board by a distance: our 65 ranged attacks killed 34 and lost the
    /// attacker **zero** times, while 225 melee attacks killed 119 and lost 6.
    /// A Spearman walking at an Archer eats the shot on the way in and the
    /// counter when it arrives; an Archer of ours kills the same unit for
    /// nothing.
    ///
    /// So when the ring around a city is mostly shooters, the defender that
    /// city must produce is a shooter. Melee stays the answer to a melee ring —
    /// this changes WHICH defender, never how many, and the wall tiers below
    /// are untouched.
    ///
    /// Entrant `live_without_barbarian_ranged_answer`; treatment
    /// `barbarian-ranged-answer`.
    pub(crate) barbarian_ranged_answer: bool,
    /// `siege-preempts-the-queue`: a raider on a city's doorstep is answered
    /// with a body before anything else is built, bought when no defender
    /// exists, and a recon unit is not a defender. Read by
    /// `barbarian_local_defenders_for_controller` here and by
    /// `AdvancedAi::siege_preemption_item` (`advanced/siege_response.rs`).
    pub(crate) siege_preempts_the_queue: bool,
    pub(crate) adjacent_camp_clear: bool,
    /// Leave a barbarian camp standing when it is a neighbour's problem
    /// more than ours: a living major we are not allied with has a city
    /// strictly nearer the camp than any of ours, and the camp is outside
    /// our own worked ring. Every deliberate clear consults
    /// `camp_is_a_neighbours_problem`; raiders that reach home are still
    /// answered. Opt-in gene `enemy-of-my-enemy`; see
    /// `advanced/enemy_of_my_enemy.rs`.
    pub(crate) enemy_of_my_enemy: bool,
    /// The camp errand prefers the exact Barbarian Outpost a city-state's
    /// quest named — the only one whose clearing pays its Envoy — over a
    /// nearer unnamed camp, and reaches further out for it. Opt-in gene
    /// `quest-camp-errand`; see `advanced/city_state_quests.rs`.
    pub(crate) quest_camp_errand: bool,
    /// Claim the ground between us and the nearest neighbours first while
    /// the army can hold it, waive the border provocation there, and wall
    /// and garrison the frontier cities. The flag lives here because the
    /// peacetime garrison (`garrison_assignments`) is this
    /// controller's. Opt-in gene `contested-land-first`; see
    /// `advanced/contested_land.rs`.
    pub(crate) contested_land_first: bool,
    /// Price a strike with the two strengths the engine will resolve it
    /// with (`melee_exchange_strengths` / `ranged_strike_strengths`) rather
    /// than bare combat strength, so matchup, flanking, support, ground and
    /// the river reach the exchange evaluation. The flag lives here because
    /// `exchange_score` does. Opt-in gene `exchange-is-the-engines`; see
    /// `advanced/engine_pricing.rs`.
    pub(crate) exchange_is_the_engines: bool,
    /// Price the defender on the tile it is being asked about rather than
    /// the one it stands on, with that tile's own defence — the reading
    /// `incoming_damage` has always used and the movers never did. The flag
    /// lives here because `projected_counter_damage` does. Opt-in gene
    /// `defend-where-you-stand`; see `advanced/engine_pricing.rs`.
    pub(crate) defend_where_you_stand: bool,
    /// Frozen historical movement; false for every current controller.
    pub(crate) legacy_movement: bool,
    /// ★ THE BARBARIANS HUNT THE RELIGIOUS CORPS TOO. A Missionary beside a
    /// city it is converting stands still for three turns at zero movement,
    /// and in Civilization VI a raider that reaches it condemns it. Here the
    /// engine already allowed it — the barbarian seat is at war with
    /// everyone, and `Action::CondemnHeretic` is legal for any military unit
    /// on the same tile with movement left — but nothing on the barbarian
    /// side ever asked for a religious unit, so a corps was safe anywhere a
    /// Settler was not. On, a raider walks onto a religious unit inside its
    /// raid ring the way it walks onto a Builder, and spends the movement it
    /// arrives with (or starts its next turn with) on the condemnation.
    /// Default-ON in every current controller; OFF on the frozen anchor
    /// (`AdvancedAi::legacy`), exactly as `adjacent_camp_clear` is, so the
    /// rating experiment does not move. Operator directive 2026-08-24; the
    /// victim's seat counts it as `religious_lost_to_barbarians`.
    pub(crate) barbarian_heretic_hunt: bool,
    /// The camp errand's claims for the current turn: camp -> (turn,
    /// claimant unit). One hunter per camp and two camps at a time, so the
    /// bounty never becomes an army diversion.
    camp_bounty_claims: BTreeMap<Pos, (u32, u32)>,
    /// Subtract the empire's unit-maintenance bill (at the gold weight)
    /// inside the policy counterfactual, so `unit_maintenance_discount`
    /// cards — Conscription, Levee en Masse — stop scoring exactly 0.0. For
    /// every other card the bill cancels in the with/without difference, so
    /// no other ranking moves. Entrant `advanced_maintenance_deck`; off in
    /// production pending its screen.
    pub(crate) maintenance_aware_deck: bool,
    /// `merchant-confederation-earns-its-slot`: the policy counterfactual
    /// reads the policy income paid to the player (`player_policy_yields`)
    /// beside the cities' yields. Set by the Advanced gene; off by default.
    pub(crate) deck_reads_player_income: bool,
    /// `retainers-garrison-the-short-cities`: while Retainers is slotted, an
    /// idle land soldier garrisons each Amenity-short city. See
    /// `retainers_garrison.rs`. Set by the Advanced gene; off by default.
    pub(crate) retainers_garrison_the_short_cities: bool,
    /// Runtime state for `retainers_garrison_the_short_cities`: the city each
    /// claiming soldier holds.
    pub(crate) retainers_garrison_claims: BTreeMap<u32, u32>,
    /// `garrison-before-the-surprise`: a stronger major's soldier near a city
    /// with no soldier drafts the nearest idle one home. See
    /// `garrison_before_the_surprise.rs`. Set by the Advanced gene; off by
    /// default.
    pub(crate) garrison_before_the_surprise: bool,
    /// Runtime state for `garrison_before_the_surprise`: the turn and the
    /// called cities read from that turn's frame.
    pub(crate) garrison_call_cache: Option<(u32, Vec<Pos>)>,
    /// `denial-spies-count-their-losses`: set by `AdvancedAi` each turn while
    /// recent Spy losses pause training; the production fallback then offers
    /// no Spy. False otherwise and on a bare `BasicAi`.
    pub(crate) spy_training_paused: bool,
    /// `parity-upgrades-before-it-buys-2`: set by `AdvancedAi` each turn
    /// while the gene is on. `buy_gold_military` then buys no unit
    /// [`OBSOLETE_BUY_ERAS_BEHIND`] or more eras behind the world. False
    /// otherwise and on a bare `BasicAi`.
    pub(crate) obsolete_military_buys_barred: bool,
    /// `parity-upgrades-before-it-buys-3`: set by `AdvancedAi` each turn
    /// while the gene is on. `buy_gold_military` then buys no unit
    /// [`OBSOLETE_BUY_TECH_ERAS_BEHIND`] or more eras behind our newest
    /// technology. False otherwise and on a bare `BasicAi`.
    pub(crate) obsolete_buys_read_our_techs: bool,
    /// `plaza-serves-the-scientist`: set by `AdvancedAi` each turn while the
    /// gene is on and the seat aims at Science; the Government Plaza's tier
    /// buildings are then the Ancestral Hall and the Intelligence Agency
    /// instead of the Warlord's Throne and the Grand Master's Chapel. See
    /// `BasicAi::plaza_building_item_for`. False on a bare `BasicAi`.
    pub(crate) plaza_serves_science: bool,
    /// `launch-pad-keeps-its-rung`: an idle city resumes its own placed,
    /// unfinished Spaceport foundation while no pad of ours stands. See
    /// `launch_pad_resume.rs`. Set by the Advanced gene; off by default.
    pub(crate) launch_pad_resumes_its_foundation: bool,
    /// `culture-digs-one-museum`: while the empire owns no Archaeological
    /// Museum, the Museum tie breaks toward the Artifact slot. Lent by the
    /// Advanced gene to a Culture seat's delegated call; off by default.
    pub(crate) digs_one_museum: bool,
    /// `culture-digs-every-site`: the Archaeological Museums a Culture seat
    /// wants, built or queued; the Museum tie breaks toward the Artifact slot
    /// while fewer stand. Lent by the Advanced gene to a Culture seat's
    /// delegated call; zero (off) by default.
    pub(crate) dig_museums_wanted: usize,
    /// `religion-seat-takes-the-faith-beliefs`: the founding tries the
    /// Faith-scaling founder beliefs first. Set each turn by the Advanced
    /// gene for a Religion-target seat; off by default.
    pub(crate) founder_takes_the_faith_beliefs: bool,
    /// Set only while delegating a Culture seat after its tourism technologies.
    pub(crate) preserve_culture_resort_sites: bool,
    /// `deck-prices-growth`: the policy counterfactual prices each city's
    /// growth surplus (`Weights::pol_growth` at the Food weight), so housing
    /// and amenity cards are worth the growth they release. Set by the
    /// Advanced gene; off by default.
    pub(crate) deck_prices_growth: bool,
    /// `deck-follows-the-queue`: the policy deck re-reviews off its cadence on
    /// the turn the queue cards worth slotting change. See `ai/deck_queue.rs`.
    /// Set by the Advanced gene; off by default.
    pub(crate) deck_follows_the_queue: bool,
    /// `deck-follows-the-queue`: each seat's queue-card signature as of its
    /// last turn. Empty while the gene is off.
    deck_queue_signature: RefCell<BTreeMap<usize, BTreeSet<Name>>>,
    /// Reserve the first usable empty trade slot before ordinary production
    /// and judge safety at the city that can actually start the route. The
    /// old gate vetoes every Trader when any city in the empire has a local
    /// barbarian alarm, so one Galley beside a remote coast can leave a safe
    /// capital's route capacity empty through insolvency. Entrant
    /// `solvency-first-trade-slot`; Advanced production turns it on after its
    /// repeated positive standard-screen result.
    pub(crate) solvency_first_trade_slot: bool,
    /// Whether a Builder whose nearest improvable tile cannot be stepped
    /// toward tries the next one instead of giving up the turn. Opt-in gene
    /// `builder-tries-the-next-tile`; see `builder_step`.
    pub(crate) builder_tries_the_next_tile: bool,
    /// Count a nearby barbarian ship as a home emergency only when its
    /// terrain-accurate next-turn attack envelope reaches one of our assets
    /// for a meaningful blow. Harmless ships remain legal ranged targets so
    /// shooters can collect combat experience without recruiting the army or
    /// displacing civilian production. Entrant `naval-threat-triage`; off
    /// pending its screen.
    pub(crate) naval_threat_triage: bool,
    /// The same round trip spread over two turns instead of one, which nothing
    /// inside a single turn's reasoning can see. Each unit's recent
    /// whereabouts are remembered here, and a unit found circling is priced
    /// out of the tiles it has already proved are worthless.
    unit_motion: BTreeMap<u32, UnitMotion>,
    /// `live-move-refusal-break` (HostOnly): the host refuses ~14% of MOVE_TO
    /// orders as `did_not_move` and the refusal event carries no destination,
    /// so the same refused move was re-issued for up to eleven straight turns
    /// (run civvis-20260830T095742Z: a settler ordered to the identical tile
    /// on turns 36–39 with movement in hand, never moving, captured on t47).
    /// The livelock detector cannot see this — `UnitMotion::circling`
    /// deliberately excludes a footprint of one, because a unit that has
    /// STOPPED is normally a unit that chose to. These three maps close the
    /// carve-out for units the controller keeps ordering to move: the step
    /// each unit was issued this turn, consecutive turns it was seen not to
    /// have taken it, and the step it is barred from re-proposing while the
    /// bar lasts. All three are inert with the gene off.
    move_refusal_break: bool,
    move_refusal_watch: RefCell<HashMap<u32, (u32, Pos, Pos)>>,
    move_refusal_strikes: HashMap<u32, (Pos, Pos, u8)>,
    move_refusal_blocks: HashMap<u32, (Pos, u32)>,
    /// `own-column-is-not-a-refusal` (OptIn): a planned step the host never
    /// received, or one onto a tile another of our units held when the frame
    /// began, is not the host's refusal. Live King civvis-20261005T060002Z
    /// (G104), Siege of Sparta: the air-assault barrier deferred every ground
    /// order of frame 0 (`air_assault_phase_deferred=60` t216, `=52` t217),
    /// yet the frame-0 plan had already written this turn's watch and path
    /// trail. The next frame's replan found its own first step "already
    /// walked" by the same-turn reversal guard and issued nothing, and the
    /// following turn judged the step the host never saw as a refusal: a
    /// Rocket Artillery 13 tiles out was barred for 8 turns on t218, idle
    /// t216-223, and a second stood 14 turns at (23,20) until it "stands
    /// down; it is going nowhere" on t236. Inert with the gene off.
    own_column_is_not_a_refusal: bool,
    /// Per unit: the last host turn the live bridge reported on its planned
    /// moves, and whether any move order actually reached the host that turn.
    /// Written only by [`BasicAi::note_host_moves`].
    host_move_feedback: HashMap<u32, (u32, bool)>,
    /// Whether the live bridge has reported sent moves at all. Off the bridge
    /// nothing reports, and every applied move is the host's own act.
    host_move_feedback_active: bool,
    /// Per unit: the turn whose watched step was onto a tile one of our own
    /// units on the same stacking layer held when the frame began.
    move_refusal_congested: RefCell<HashMap<u32, u32>>,
    /// Where each of our units stood when this frame's movement began.
    frame_own_tiles: HashMap<Pos, Vec<u32>>,
    /// `guns-stay-out-of-reach`: per siege gun, the tiles it can reach this
    /// turn where one visible hostile's blow would destroy it, with that
    /// blow. Written by the advanced controller each frame; empty with the
    /// gene off. See `advanced/guns_stay_out_of_reach.rs`.
    gun_lethal_tiles: HashMap<u32, HashMap<Pos, f64>>,
    /// `guns-stay-out-of-reach-2`: per land military unit that is not a
    /// siege gun, the tiles it can reach this turn where a visible hostile's
    /// one blow would destroy it, with each such blow by its source. A
    /// source no longer on the board stops counting. Empty with the gene
    /// off. See `advanced/guns_stay_out_of_reach.rs`.
    unit_lethal_blows: HashMap<u32, HashMap<Pos, Vec<(u32, f64)>>>,
    /// Melee units the ancient-rush lane wants in hand, or 0 when no rush is
    /// running. Set once a turn by `AdvancedAi` from its strategic plan.
    ///
    /// The standing-army target is `mil_per_city * n_cities`, and
    /// `mil_per_city` defaults to 1.0 — so a two-city empire wants two
    /// military units, which is what `rush_census` measures it fielding
    /// (2.5 melee at turn 50, 1.1 of them near any rival capital). A siege
    /// needs four. Without this floor the rush plans a war it never builds
    /// the army for, which is the failure the census caught.
    pub(crate) rush_military_floor: usize,
    /// Discount motionless Settlers from the expansion gate's in-flight test.
    /// The Civilization VI bridge turns this on; native tournament games leave
    /// it off so their recorded ladders and the frozen `advanced_v1` rating
    /// anchor keep replaying the historical controller.
    pub(crate) settler_strand_discount: bool,
    /// `settler-backlog-brake`: once the empire holds
    /// `SETTLER_BACKLOG_MIN_CITIES` cities, no Settler may start while an
    /// owned Settler has stood on one tile for `SETTLER_BACKLOG_IDLE_TURNS`
    /// turns. The twin of `settler_strand_discount`, which OPENS the pipeline
    /// for a replacement when a Settler parks — and the replacement then
    /// refuses the same sites. See `parked_settlers`.
    pub(crate) settler_backlog_brake: bool,
    /// Version two of the backlog brake: V1's same full stop, but only after
    /// the empire has matured to `SETTLER_BACKLOG_V2_MIN_CITIES`. The first
    /// three-to-five-city growth wave retains the ordinary pipeline. Set
    /// through the Advanced gene toggle.
    pub(crate) settler_backlog_brake_2: bool,
    /// Let a second Settler enter the pipeline while one is already walking,
    /// once the empire holds two cities and is still at least two short of
    /// its target. Set only through `AdvancedAi::enable_parallel_settlers`
    /// (the Civilization VI bridge); every native constructor and the frozen
    /// `advanced_v1` anchor keep the one-at-a-time gate. See `pick_item`.
    pub(crate) parallel_settlers: bool,
    /// Build a Settler at the host's own population floor. Civilization VI
    /// lets a city start a Settler at population 2 (it drops to 1); the
    /// evolved genome the live seat plays (`g56-48`) carries
    /// `settler_min_pop` 2.456, which reads as "wait for population 3". Run
    /// civvis-20260816T021044Z: the capital reached population 2 on t12 and
    /// "the city is below settler_min_pop" refused a Settler on t12/17/21/23/27
    /// while it built a slinger, a galley, a builder, a trader, a granary and a
    /// slinger; population 3 came on t32, the second city on t40, and the
    /// empire read one city against Babylon's 164 score at t50. Set only
    /// through `AdvancedAi::enable_host_settler_pop` (the Civilization VI
    /// bridge); native constructors and the frozen anchor keep the genome's
    /// figure. See `pick_item`.
    pub(crate) host_settler_pop: bool,
    /// Expand to the land the empire can hold, at pipeline pace: the settler
    /// pipeline is `LAND_GRAB_PIPELINE_BASE + cities / 3` walkers bounded by
    /// the seats still short of `city_target`, and the window closes
    /// `LAND_GRAB_SETTLE_HORIZON` standard turns before the turn limit rather
    /// than at the genome's `settler_stop_turn`. Set only through
    /// `AdvancedAi::enable_land_grab` (the Civilization VI bridge), which
    /// also raises the target it threads through `delegated_cities`; native
    /// constructors and the frozen anchor keep the one-at-a-time gate and the
    /// gene. See `pick_item` and `AdvancedAi::land_grab`.
    pub(crate) land_grab: bool,
    /// The baseline-governor half of `rapid-city-expansion-2`.
    ///
    /// Unlike the culled version one, this never rewrites the opening book,
    /// pantheon, or site ranking. It reserves the capital's next empty production choice,
    /// uses the measured opening-band pipeline, and keeps the legal
    /// population and payback gates in step with the strategic controller.
    /// 🛑 2026-09-11, RETRACTED THE SAME DAY: the reading below is an artefact
    /// of the probe's own length. Keep reading; the retraction is the point.
    ///
    /// Priced by withholding at the shape the live seat plays — 60×38, 4
    /// players, standard speed, Emperor, `--rivals firaxis-mix --handicap
    /// rivals` — 90 games, 270 seats, `--turns 80 --p-default-on 0.5`:
    ///
    /// ```text
    ///     gene                              on     off    diff       z
    ///     rapid-city-expansion-2          2.66    2.93   -0.27   -2.69
    ///     expansion-scales-with-difficulty 2.70    2.89   -0.19   -1.88
    ///     expansion-schedule               2.83    2.78   +0.05   +0.48
    ///     settler-walk-deadline            2.80    2.81   -0.01   -0.13
    /// ```
    ///
    /// It persists rather than washing out: cities at turn 80 read −0.28
    /// (z −2.60) on the same split, and both city columns agree. The arms are
    /// balanced on the other three genes (largest imbalance 0.11), so it is not
    /// the draw.
    ///
    /// ⚠⚠ THIS IS NOT A PROMOTION ARGUMENT AND MUST NOT BE READ AS ONE. An
    /// 80-turn probe completes no game, so it carries no win column and is not
    /// a ledger source; the default rule reads the two win columns of native
    /// screens and nothing here touches them. Four genes were tested, so the
    /// family-wise bar is nearer z 2.50 than 2.00 and −2.69 clears it only
    /// narrowly. And an expansion gene may well pay after turn 80 — this says
    /// what it costs in the opening, not what it is worth in a game.
    ///
    /// ## 🛑🛑 RETRACTED — `--turns 80` MOVED THIS GENE'S OWN BAND TO TURN 19
    ///
    /// `rapid_city_expansion::band_turn` is `turn_limit() *
    /// EXPANSION_BAND_SHARE`, and that share is **0.24**. At the screen's normal
    /// 250 turns the band lands on turn 60, which is the measured opening band.
    /// At the `--turns 80` the probe used it lands on **turn 19** — so
    /// `pipeline_width` returned `None` for turns 19 to 80 and the caller's
    /// `unwrap_or(1)` held the pipeline at a single walker for the whole window
    /// the probe then measured. The probe shortened the game and the gene
    /// dutifully shortened its opening to match.
    ///
    /// Re-run at `--turns 250`, same shape, 60 games, 180 seats:
    ///
    /// ```text
    ///     column                    on      off     diff       z
    ///     cities_at_game_turn_60  2.918   2.867   +0.050   +0.42
    ///     cities (final)          7.649   8.892   -1.242   -2.75
    ///     score_share             0.204   0.212   -0.008   -0.70
    ///     win                     0.082   0.108   -0.026   -0.59
    /// ```
    ///
    /// **The opening cost is gone**: +0.05 against the claimed −0.27. What is
    /// left is a −1.24 on final cities at z −2.75, which is one column of seven
    /// at a bar that seven comparisons put near 2.7, and which **neither share
    /// nor win follows** (z −0.70 and −0.59). That is the shape of a mechanism
    /// moving without an outcome, and `docs/SURROGATE_ENDPOINT.md` is the
    /// standing reason not to bank it. The gene keeps shipping on.
    ///
    /// ## ⚠⚠ AND THE METHOD LESSON IS BIGGER THAN THE GENE
    ///
    /// `--turns N` does not truncate a game, it **rescales the agent**. There
    /// are 46 call sites of `Game::turn_limit()` and 54 of `standard_duration`
    /// across the AI: deadlines, bands, cadences and lane windows are all
    /// expressed as a share of the game's length. Shorten the game and every one
    /// of them moves.
    ///
    /// ⭐ It is not always fatal — the deployment-shape opening reads 2.59 at
    /// both 80 and 250 turns, because there the binding constraint is land and
    /// production rather than any schedule. But **no gene-level question may be
    /// answered on a shortened game**, and a probe that was cheap because it was
    /// short is not cheap if it answers a different question.
    pub(crate) rapid_city_expansion_2: bool,
    /// `capital-settler-after-completion`: once the capital is population two
    /// and has no queued work, start a legal Settler instead of letting the
    /// ordinary force or infrastructure ranking fill that opening. The city
    /// target, normal settlement window, site search, and all earlier
    /// emergency choices remain authoritative. Set through
    /// `AdvancedAi::enable_capital_settler_after_completion`.
    pub(crate) capital_settler_after_completion: bool,
    /// `opening-builder-after-the-first-settler`: once the capital's first
    /// Settler is out (walking or founded) and the empire has never held a
    /// Builder, the capital trains one before its next Settler. Live Immortal
    /// seats 10-30 (54 games): first Builder at a median turn 26, 5% of owned
    /// land improved at turn 40 against the rivals' 26%, and the rivals'
    /// population ran 1.6x ours from turn 20. Set through
    /// `AdvancedAi::enable_opening_builder_after_the_first_settler`.
    pub(crate) opening_builder_after_the_first_settler: bool,
    /// Receipt for `opening_builder_after_the_first_settler`: set the first
    /// time the empire holds or queues a Builder, so the step fires at most
    /// once a game and never after a Builder spends its last charge.
    pub(crate) opening_builder_paid: bool,
    /// `first-pad-in-the-forge`: while no Spaceport of ours is committed, a
    /// city may start the first one only within `FIRST_PAD_FORGE_SHARE` of
    /// the best placing producer. See `ai/first_pad_in_the_forge.rs`. Set
    /// through `AdvancedAi::enable_first_pad_in_the_forge`.
    pub(crate) first_pad_in_the_forge: bool,
    /// Independent first-slot exploration experiment.
    pub(crate) scout_first_opening: bool,
    /// Opt-in first-slot ranged support; later recon behavior stays enabled.
    pub(crate) opening_ranged_support: bool,
    /// Take the pantheon that founds a city. Civilization VI's Religious
    /// Settlements grants a free Settler in the capital
    /// (`RELIGIOUS_SETTLEMENTS_SETTLER_MODIFIER`, `Expansion2_Beliefs.xml`),
    /// and Fertility Rites a free Builder — the engine models both
    /// (`do_choose_pantheon`). The shipped prefix ranks Divine Spark first,
    /// so the live seat took Divine Spark in 40 of 40 recorded 2026-08-17/19
    /// runs (+1 Great Person point in districts it did not yet have) while
    /// the same 12.5 Faith could have stood a Settler at ~t20 — a city ~t24
    /// against a median third city at t38, and the capital's next 40–55
    /// production freed. With this on the two founding pantheons lead the
    /// prefix, in that order; every other rank is unchanged, and the board
    /// term of `pantheon_reads_the_board` still adds on top. Set only through
    /// `AdvancedAi::enable_expansion_pantheon` (the Civilization VI bridge);
    /// native constructors and the frozen anchor keep the shipped order.
    /// See `research_with_government` and `AdvancedAi::expansion_pantheon`.
    pub(crate) expansion_pantheon: bool,
    /// `prophet-race-takes-divine-spark`: this turn the seat is running the
    /// Prophet race (`AdvancedAi::race_points_wanted`), so Divine Spark's
    /// +1 Prophet point a turn on every Holy Site ranks right behind the
    /// founding pantheon and ahead of Fertility Rites. Set every turn by
    /// `AdvancedAi::take_turn_inner`; see
    /// `advanced/astrology_before_the_second_city.rs`.
    pub(crate) pantheon_for_the_prophet_race: bool,
    /// The opening book's Settler waits for the host's population floor
    /// instead of burning its slot.
    ///
    /// ★★★★ THE BOOK'S SECOND BUILD IS A SETTLER, AND HALF THE TIME IT NEVER
    /// PLAYS. The live genome's book is scout, settler, builder, slinger.
    /// Civilization VI (and `Game::can_produce`) refuse a Settler below
    /// population 2; the capital is founded at ~t2, finishes the Scout at
    /// ~t6 and grows to 2 at ~t7 (median over 102 recorded runs, range
    /// 4–10). When the Scout completes first, the book's `g.apply` of the
    /// Settler fails, `book_pos` has already advanced, and the slot is
    /// simply gone: the Builder plays, the Settler comes from `pick_item`
    /// once the Builder is done. Measured: openings that read
    /// `SCOUT,SETTLER,…` order the first Settler at t5–8 and found city 2 at
    /// t15–19; the `SCOUT,BUILDER,SETTLER,…` half order it at t9–13 and found
    /// city 2 at t19–24 — four turns of the whole game, on the turn-60 city
    /// count that gates every win (0 in 22 below four cities).
    ///
    /// The engine, like the host, banks a switched item's progress
    /// (`City::production_progress`), so with this on the refused slot is
    /// HELD rather than burned: the next slot plays now, and the turn the
    /// capital reaches the floor the Settler takes the queue ahead of that
    /// filler, which resumes afterwards with nothing lost. Set only through
    /// `AdvancedAi::enable_opening_settler_waits` (the Civilization VI
    /// bridge); native constructors and the frozen anchor keep the burning
    /// slot. See `cities` and `play_pending_book_settler`.
    pub(crate) opening_settler_waits: bool,
    /// Each owned Settler's last seen tile and how many consecutive turns it
    /// has stood on it. See [`BasicAi::stranded_settlers`] for why the
    /// expansion gate needs this and nothing else does.
    settler_idle: BTreeMap<u32, (Pos, u32)>,
    /// The turn `settler_idle` was last advanced, so a second production pass
    /// in one turn cannot double-count a settler as idle.
    settler_idle_turn: Option<u32>,
    /// Where this agent tells an observer what it is doing. Off unless a
    /// spectator attached one; see [`crate::reasoning`].
    pub(crate) journal: Journal,
}

/// Unit-scoped mutable planner state produced on a private search branch.
///
/// Advanced snapshot planning never publishes a cloned controller wholesale:
/// doing so would overwrite decisions made by earlier units in the serial
/// commit phase. This is the complete state one military unit's movement
/// helpers may update, extracted and merged by unit ID only.
pub(crate) struct BasicUnitPlanState {
    recovering: bool,
    patrol_target: Option<Pos>,
    settler_target: Option<Pos>,
    memory: Option<UnitMemory>,
    last_path_step: Option<(u32, Vec<Pos>)>,
    patrol_posts: HashMap<String, Vec<Pos>>,
}

impl Default for BasicAi {
    fn default() -> Self {
        Self::new()
    }
}

/// How many improvable tiles a Builder may try to route to in one turn before
/// giving the turn up.
///
/// ⚠ A REFUSED `step_toward` IS THE MOST EXPENSIVE SHAPE OF TURN IN THE GAME,
/// not the cheapest. `route_step` returns `None` only after A* has exhausted
/// everything the unit can reach, so each failed attempt pays a whole-region
/// search -- and a Builder in a pocket with no reachable job pays it every
/// turn for the rest of the game. At four attempts a 24-game probe measured
/// **+11.8 ± 6.3% wall seconds per completed turn**, the most expensive of the
/// 24 committed fires probes against a median of 1.9%. Two covers the failure
/// this bounds -- a single unreachable best tile -- and halves that worst case.
/// An ordinary turn succeeds on the first and pays nothing either way.
pub(crate) const BUILDER_ROUTE_ATTEMPTS: usize = 2;

impl BasicAi {
    pub(crate) fn unit_doctrine(g: &Game, uid: u32) -> UnitDoctrine {
        let spec = &g.rules.units[g.units[&uid].kind];
        if spec.class == "support" {
            return UnitDoctrine::Support;
        }
        if spec.domain.as_deref() == Some("air") {
            return if spec.siege {
                UnitDoctrine::AirStrike
            } else {
                UnitDoctrine::AirDefense
            };
        }
        if spec.class == "military" && !spec.is_melee_capable() && !spec.has_ranged_attack() {
            return UnitDoctrine::Support;
        }
        if spec.siege {
            return UnitDoctrine::Siege;
        }
        match spec.promotion_class.as_str() {
            "recon" => UnitDoctrine::Recon,
            "light_cavalry" | "naval_raider" | "naval_melee" => UnitDoctrine::Mobile,
            "ranged" | "naval_ranged" => UnitDoctrine::Ranged,
            "naval_carrier" => UnitDoctrine::Carrier,
            _ => UnitDoctrine::Assault,
        }
    }

    /// Innate class strength the rules engine applies against one opponent.
    /// Keep tactical projections on the same counter cycle as combat itself.
    pub(crate) fn class_matchup_strength(g: &Game, attacker: u32, defender: u32) -> f64 {
        let unit = &g.units[&attacker];
        let other_unit = &g.units[&defender];
        let spec = &g.rules.units[unit.kind];
        let other = &g.rules.units[other_unit.kind];
        if spec.promotion_class == "anti_cavalry"
            && (matches!(
                other.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            ) || (other.cavalry && other.promotion_class == "ranged"))
            && other_unit.kind != "war_cart"
        {
            10.0
        } else if spec.promotion_class == "melee" && other.promotion_class == "anti_cavalry" {
            5.0
        } else {
            0.0
        }
    }

    /// Expected damage enemies could deliver after one approach step. Ranged
    /// and siege units use this to prefer standoff tiles outside return reach.
    pub(crate) fn projected_counter_damage(
        &self,
        g: &Game,
        uid: u32,
        tile: Pos,
        hostile_units: &[u32],
    ) -> f64 {
        let defender = &g.units[&uid];
        hostile_units
            .iter()
            .filter_map(|enemy| g.units.get(enemy).map(|unit| (*enemy, unit)))
            .filter(|(_, enemy)| {
                let spec = &g.rules.units[enemy.kind];
                spec.class == "military" && (spec.is_melee_capable() || spec.has_ranged_attack())
            })
            .filter_map(|(enemy_id, enemy)| {
                let enemy_spec = &g.rules.units[enemy.kind];
                let ranged = enemy_spec.has_ranged_attack();
                let attack_range = if ranged {
                    g.unit_attack_range(enemy_id).max(1)
                } else {
                    1
                };
                // Price the enemy's next turn, when movement refreshes; its
                // leftover points from the previous turn are irrelevant.
                // Ordinary land units can take at least one approach step,
                // while siege cannot move and fire without a promotion.
                let approach = (!enemy_spec.siege) as i32;
                (g.wdist(tile, enemy.pos) <= attack_range + approach).then(|| {
                    let attack = if ranged {
                        g.unit_ranged_attack_strength(enemy)
                    } else {
                        g.unit_strength(enemy, false)
                    } + Self::class_matchup_strength(g, enemy_id, uid);
                    // `defend-where-you-stand`: the question is what the
                    // enemy does to us *on `tile`*, so the defender is
                    // priced there — the unit moved, plus that tile's own
                    // defence. `None` with the gene off, and then this is
                    // the strength it has always used. See
                    // `advanced/engine_pricing.rs`.
                    let defense = self
                        .defence_base_where_it_would_stand(g, uid, tile)
                        .unwrap_or_else(|| g.unit_strength(defender, true))
                        + Self::class_matchup_strength(g, uid, enemy_id);
                    let attack = effective_strength(attack, enemy.hp);
                    let defense = effective_strength(defense, defender.hp);
                    30.0 * ((attack - defense) / 25.0).exp()
                })
            })
            .sum()
    }

    fn tech_leads_to(g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules.techs.get(target).is_some_and(|spec| {
                spec.requires
                    .iter()
                    .any(|parent| Self::tech_leads_to(g, candidate, parent))
            })
    }

    fn civic_leads_to(g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules.civics.get(target).is_some_and(|spec| {
                spec.requires
                    .iter()
                    .any(|parent| Self::civic_leads_to(g, candidate, parent))
            })
    }

    fn has_building_family(g: &Game, pid: usize, family: impl AsName) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities[&cid]
                .buildings
                .iter()
                .any(|building| g.building_is_family(building, family))
        })
    }

    /// Follow through on the technology unlocked by infrastructure already in
    /// the empire. Without this, the fixed early-game priority list ends at
    /// Machinery and the fallback can keep taking whichever late branch the
    /// rules map happens to return first. In a completed spectator game that
    /// let a Market-owning city-state reach Electricity while permanently
    /// skipping Stirrups -> Banking, leaving more than 4,000 Gold with no Bank
    /// to buy. Replacement Markets and Banks count as their base families.
    fn economic_research_goal(g: &Game, pid: usize) -> Option<&'static str> {
        let player = &g.players[pid];
        if Self::has_building_family(g, pid, crate::name!("market"))
            && !player.techs.contains(&crate::name!("banking"))
        {
            return Some("banking");
        }
        if Self::has_building_family(g, pid, crate::name!("bank"))
            && !player.techs.contains(&crate::name!("economics"))
        {
            return Some("economics");
        }
        None
    }

    fn research_step_toward(g: &Game, avail: &[Name], goal: Option<&str>) -> Option<Name> {
        goal.and_then(|goal| {
            avail
                .iter()
                .find(|tech| tech.as_str() == goal)
                .cloned()
                .or_else(|| {
                    avail
                        .iter()
                        .filter(|tech| Self::tech_leads_to(g, tech, goal))
                        .min_by(|a, b| {
                            g.rules.techs[*a]
                                .cost
                                .partial_cmp(&g.rules.techs[*b].cost)
                                .unwrap()
                                .then(a.cmp(b))
                        })
                        .cloned()
                })
        })
    }

    /// The controller's research candidates inside the rolling era window.
    ///
    /// The rules correctly make every prerequisite-satisfied technology legal:
    /// that is what permits a human player to beeline. An unattended AI needs
    /// a stronger policy, though. A victory or military beeline can otherwise
    /// jump from an Ancient prerequisite to a Modern one and leave a useful
    /// side branch such as Bronze Working untouched for most of the game.
    /// Keep candidates in the oldest era with an unresearched nonrepeatable
    /// technology and the next two eras: at most three eras can be in flight.
    /// Once the oldest backlog clears, that three-era window rolls forward.
    /// Repeatable Future technologies are deliberately excluded from the
    /// floor, so completing the ordinary tree still leaves them selectable.
    pub(crate) fn era_window_techs(g: &Game, pid: usize) -> Vec<Name> {
        let earliest_incomplete_era = g
            .rules
            .techs
            .iter()
            .filter(|(tech, spec)| !spec.repeatable && !g.players[pid].techs.contains(*tech))
            .map(|(_, spec)| spec.era)
            .min();
        let latest_allowed_era = earliest_incomplete_era
            .map(|era| era.saturating_add(RESEARCH_ERA_WINDOW.saturating_sub(1)));

        g.available_techs(pid)
            .into_iter()
            .filter(|tech| {
                latest_allowed_era
                    .is_none_or(|era| g.rules.techs.get(tech).is_some_and(|spec| spec.era <= era))
            })
            .collect()
    }

    fn civic_step_toward(g: &Game, avail: &[Name], goal: Option<&str>) -> Option<Name> {
        goal.and_then(|goal| {
            avail
                .iter()
                .find(|civic| civic.as_str() == goal)
                .cloned()
                .or_else(|| {
                    avail
                        .iter()
                        .filter(|civic| Self::civic_leads_to(g, civic, goal))
                        .min_by(|a, b| {
                            g.rules.civics[*a]
                                .cost
                                .partial_cmp(&g.rules.civics[*b].cost)
                                .unwrap()
                                .then(a.cmp(b))
                        })
                        .cloned()
                })
        })
    }

    /// One-city states need a credible local defense, not an empire-sized
    /// standing army. Their budget grows when an actual hostile force reaches
    /// the city, but remains bounded so mature maps do not fill every tile
    /// with idle city-state units.
    fn minor_military_budget(g: &Game, pid: usize) -> usize {
        let enemies: Vec<usize> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_barbarian
                    && g.is_at_war(pid, player.id)
            })
            .map(|player| player.id)
            .collect();
        if enemies.is_empty() {
            // Once its three-unit garrison is filled, a peaceful city-state
            // may develop infrastructure or simply leave Production idle.
            return 3;
        }
        let cities = g.player_city_ids(pid);
        let nearby_hostiles = g
            .units
            .values()
            .filter(|unit| {
                enemies.contains(&unit.owner)
                    && cities
                        .iter()
                        .any(|city| g.wdist(g.cities[city].pos, unit.pos) <= 6)
            })
            .count();
        (4 + nearby_hostiles.div_ceil(2)).min(7)
    }

    fn minor_home(g: &Game, pid: usize) -> Option<Pos> {
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .min_by_key(|city| (city.original_owner != pid, city.id))
            .map(|city| city.pos)
    }

    fn minor_enemy_near_home(g: &Game, pid: usize, enemy: usize) -> bool {
        let Some(home) = Self::minor_home(g, pid) else {
            return false;
        };
        g.units
            .values()
            .any(|unit| unit.owner == enemy && g.wdist(home, unit.pos) <= MINOR_DEFENSE_RADIUS)
            || g.cities
                .values()
                .any(|city| city.owner == enemy && g.wdist(home, city.pos) <= MINOR_DEFENSE_RADIUS)
            || (g.barb_pid == Some(enemy)
                && g.barb_camps
                    .keys()
                    .any(|camp| g.wdist(home, *camp) <= MINOR_DEFENSE_RADIUS))
    }

    fn minor_district_family(g: &Game, pid: usize) -> &'static str {
        match g.cs_type(&g.players[pid].civ) {
            "scientific" => "campus",
            "cultural" => "theater_square",
            "religious" => "holy_site",
            "militaristic" => "encampment",
            "industrial" => "industrial_zone",
            _ => "commercial_hub",
        }
    }

    fn minor_tech_goal(g: &Game, pid: usize) -> Option<&'static str> {
        let goal = match g.cs_type(&g.players[pid].civ) {
            "scientific" => "writing",
            "religious" => "astrology",
            "militaristic" => "bronze_working",
            "industrial" => "apprenticeship",
            "trade" => "currency",
            _ => return None,
        };
        (!g.players[pid].techs.contains(&Name::new(goal))).then_some(goal)
    }

    fn has_exploration_target(&self, g: &Game, pid: usize, uid: u32) -> bool {
        g.map.tiles.iter().any(|(pos, _)| {
            !g.players[pid].explored.contains(pos) && g.unit_can_traverse(uid, *pos)
        })
    }

    /// Recon explores even during war. Without recon, one ordinary combat
    /// unit per movement domain scouts at peace so the empire is not blind,
    /// while the rest remain available for patrol and defense.
    fn should_explore(&self, g: &Game, pid: usize, uid: u32, at_war: bool) -> bool {
        let doctrine = Self::unit_doctrine(g, uid);
        if doctrine == UnitDoctrine::Recon {
            return true;
        }
        if at_war
            || matches!(
                doctrine,
                UnitDoctrine::Siege
                    | UnitDoctrine::Support
                    | UnitDoctrine::AirDefense
                    | UnitDoctrine::AirStrike
                    | UnitDoctrine::Carrier
            )
        {
            return false;
        }
        let domain = g.rules.units[g.units[&uid].kind]
            .domain
            .as_deref()
            .unwrap_or("land");
        let candidates = g.player_unit_ids(pid).into_iter().filter(|other| {
            let spec = &g.rules.units[g.units[other].kind];
            spec.class == "military"
                && spec.domain.as_deref().unwrap_or("land") == domain
                && !matches!(
                    Self::unit_doctrine(g, *other),
                    UnitDoctrine::Siege
                        | UnitDoctrine::AirDefense
                        | UnitDoctrine::AirStrike
                        | UnitDoctrine::Carrier
                )
        });
        let recon_exists = candidates
            .clone()
            .any(|other| Self::unit_doctrine(g, other) == UnitDoctrine::Recon);
        !recon_exists && candidates.min() == Some(uid)
    }

    /// Required exchange value for an attack. Dedicated assault and mobile
    /// units accept thinner advantages, high-strength units press them harder,
    /// recon avoids routine combat, and siege strongly prefers districts.
    pub(crate) fn attack_threshold(&self, g: &Game, uid: u32, target: Pos) -> f64 {
        let unit = &g.units[&uid];
        let doctrine = Self::unit_doctrine(g, uid);
        let role = match doctrine {
            UnitDoctrine::Recon => 14.0,
            UnitDoctrine::Assault => -2.0,
            UnitDoctrine::Mobile => -5.0,
            UnitDoctrine::Ranged => 0.0,
            UnitDoctrine::Siege => 5.0,
            UnitDoctrine::Support | UnitDoctrine::Carrier => 1_000.0,
            UnitDoctrine::AirDefense => -1.0,
            UnitDoctrine::AirStrike => -4.0,
        };
        let attack_strength = g
            .unit_strength(unit, false)
            .max(g.unit_ranged_attack_strength(unit));
        let strength_drive = ((attack_strength - 25.0) * 0.12).clamp(0.0, 8.0);
        let target_adjustment = if g.city_at(target).is_some()
            || g.map
                .get(target)
                .is_some_and(|tile| tile.district.is_some())
        {
            match doctrine {
                UnitDoctrine::Siege => -22.0,
                UnitDoctrine::Assault => -3.0,
                UnitDoctrine::Recon => 8.0,
                _ => 0.0,
            }
        } else {
            match doctrine {
                UnitDoctrine::Siege => 14.0,
                UnitDoctrine::Mobile
                    if g.unit_ids_at(target).iter().any(|other| {
                        g.rules.units[g.units[other].kind].class != "military"
                            || g.units[other].hp <= 40
                    }) =>
                {
                    -6.0
                }
                _ => 0.0,
            }
        };
        // A raider's life is cheaper than a major's: no war weariness, no
        // retaliation invited, no peace to lose, the camp respawns whatever we
        // do, and killing the unit is the only exit from the zone-of-control
        // lock that strands our Settlers. See `barbarian_bargain`.
        let bargain = if self.barbarian_bargain
            && !self.barb
            && g.barb_pid.is_some_and(|barb| {
                g.unit_ids_at(target)
                    .iter()
                    .any(|other| g.units[other].owner == barb)
            }) {
            BARBARIAN_BARGAIN_DISCOUNT
        } else {
            0.0
        };
        self.w.attack_floor + role + target_adjustment - strength_drive - bargain
    }

    /// Non-generic actions that define a unit's strategic job. Fast raiders
    /// exploit infrastructure, and aircraft use missions and rebasing instead
    /// of pretending to be land units with long range.
    fn air_pillage_score(g: &Game, target: Pos) -> i32 {
        let Some(tile) = g.map.get(target) else {
            return 0;
        };
        if let Some(improvement) = tile.improvement.as_deref() {
            return match improvement {
                "airstrip" => 145,
                "oil_well" | "offshore_oil_rig" | "mine" | "quarry" => 90,
                "farm" | "fishing_boats" => 55,
                _ => 70,
            };
        }
        let Some(district) = tile.district else {
            return 0;
        };
        if let Some(cost) = tile
            .owner_city
            .and_then(|city| g.cities.get(&city))
            .and_then(|city| {
                city.buildings
                    .iter()
                    .filter(|building| !city.pillaged_buildings.contains(*building))
                    .filter(|building| {
                        g.rules.buildings[building]
                            .district
                            .is_some_and(|family| g.district_family(district) == family)
                    })
                    .map(|building| g.rules.buildings[building].cost as i32)
                    .max()
            })
        {
            return 70 + cost / 5;
        }
        if !tile.pillaged {
            return match g.district_family(district).as_str() {
                "aerodrome" | "industrial_zone" | "campus" | "spaceport" => 135,
                "commercial_hub" | "harbor" | "holy_site" | "theater_square" => 115,
                _ => 90,
            };
        }
        65
    }

    fn priority_target_score(g: &Game, pid: usize, target: Pos) -> i32 {
        let Some(support) = g.priority_support_target_at(pid, target) else {
            return 0;
        };
        let unit = &g.units[&support];
        let spec = &g.rules.units[unit.kind];
        105 + (100 - unit.hp)
            + (spec.cost * 0.18) as i32
            + if spec.anti_air_strength > 0.0 {
                100
            } else if matches!(unit.kind.as_str(), "drone" | "observation_balloon") {
                45
            } else if matches!(unit.kind.as_str(), "medic" | "supply_convoy") {
                30
            } else {
                0
            }
    }

    pub(crate) fn doctrine_action(&self, g: &Game, pid: usize, uid: u32) -> Option<Action> {
        let doctrine = Self::unit_doctrine(g, uid);
        if !matches!(
            doctrine,
            UnitDoctrine::Mobile | UnitDoctrine::AirDefense | UnitDoctrine::AirStrike
        ) {
            return None;
        }
        let legal = g.legal_doctrine_actions(pid, uid);
        match doctrine {
            UnitDoctrine::Mobile => legal
                .iter()
                .find(|action| matches!(action, Action::CoastalRaid { unit, .. } if *unit == uid))
                .cloned()
                .or_else(|| {
                    legal
                        .iter()
                        .find(|action| matches!(action, Action::Pillage { unit } if *unit == uid))
                        .cloned()
                }),
            UnitDoctrine::AirDefense => legal
                .iter()
                .find(|action| match action {
                    Action::AirStrike { unit, target } if *unit == uid => {
                        g.unit_ids_at(*target).iter().any(|other| {
                            let other = &g.units[other];
                            other.owner != pid
                                && g.rules.units[other.kind].domain.as_deref() == Some("air")
                        })
                    }
                    _ => false,
                })
                .cloned()
                .or_else(|| {
                    legal
                        .iter()
                        .filter_map(|action| match action {
                            Action::PriorityTarget { unit, target } if *unit == uid => Some((
                                Self::priority_target_score(g, pid, *target),
                                *target,
                                action.clone(),
                            )),
                            _ => None,
                        })
                        .max_by_key(|(score, target, _)| (*score, std::cmp::Reverse(*target)))
                        .map(|(_, _, action)| action)
                })
                .or_else(|| {
                    legal
                        .iter()
                        .filter_map(|action| match action {
                            Action::AirPatrol { unit, to } if *unit == uid => {
                                let city_cover = g
                                    .cities
                                    .values()
                                    .filter(|city| city.owner == pid && g.wdist(*to, city.pos) <= 1)
                                    .map(|city| 100 + city.pop * 5)
                                    .sum::<i32>();
                                let unit_cover =
                                    g.units
                                        .values()
                                        .filter(|other| {
                                            other.owner == pid
                                                && other.id != uid
                                                && g.wdist(*to, other.pos) <= 1
                                                && g.rules.units[other.kind].class == "military"
                                        })
                                        .count() as i32
                                        * 12;
                                Some((city_cover + unit_cover, *to, action.clone()))
                            }
                            _ => None,
                        })
                        .max_by_key(|(score, to, _)| (*score, std::cmp::Reverse(*to)))
                        .map(|(_, _, action)| action)
                })
                .or_else(|| {
                    legal.into_iter().find(
                        |action| matches!(action, Action::AirStrike { unit, .. } if *unit == uid),
                    )
                }),
            UnitDoctrine::AirStrike => {
                let mission = legal
                    .iter()
                    .filter_map(|action| match action {
                        Action::AirStrike { unit, target } if *unit == uid => {
                            let target_hp = g
                                .unit_ids_at(*target)
                                .iter()
                                .filter_map(|other| {
                                    let other = &g.units[other];
                                    (other.owner != pid).then_some(other.hp)
                                })
                                .min()
                                .unwrap_or(100);
                            let city = g.city_at(*target).is_some() as i32;
                            Some((city * 120 + 100 - target_hp, *target, action.clone()))
                        }
                        Action::AirPillage { unit, target } if *unit == uid => {
                            Some((Self::air_pillage_score(g, *target), *target, action.clone()))
                        }
                        Action::PriorityTarget { unit, target } if *unit == uid => Some((
                            Self::priority_target_score(g, pid, *target),
                            *target,
                            action.clone(),
                        )),
                        _ => None,
                    })
                    .max_by_key(|(score, target, _)| (*score, std::cmp::Reverse(*target)))
                    .map(|(_, _, action)| action);
                mission
                    .or_else(|| {
                        let enemy_positions: Vec<Pos> = g
                            .units
                            .values()
                            .filter(|other| other.owner != pid && g.is_at_war(pid, other.owner))
                            .map(|other| other.pos)
                            .chain(
                                g.cities
                                    .values()
                                    .filter(|city| {
                                        city.owner != pid && g.is_at_war(pid, city.owner)
                                    })
                                    .map(|city| city.pos),
                            )
                            .collect();
                        if enemy_positions.is_empty() {
                            None
                        } else {
                            legal
                                .iter()
                                .filter_map(|action| match action {
                                    Action::AirRebase { unit, to } if *unit == uid => {
                                        let distance = enemy_positions
                                            .iter()
                                            .map(|enemy| g.wdist(*to, *enemy))
                                            .min()
                                            .unwrap_or(i32::MAX);
                                        Some((distance, *to, action.clone()))
                                    }
                                    _ => None,
                                })
                                .min_by_key(|(distance, to, _)| (*distance, *to))
                                .map(|(_, _, action)| action)
                        }
                    })
                    .or_else(|| {
                        legal
                            .into_iter()
                            .filter_map(|action| match action {
                                Action::AirPatrol { unit, to } if unit == uid => {
                                    let nearest_city = g
                                        .cities
                                        .values()
                                        .filter(|city| city.owner == pid)
                                        .map(|city| g.wdist(to, city.pos))
                                        .min()
                                        .unwrap_or(i32::MAX);
                                    Some((nearest_city, to, action))
                                }
                                _ => None,
                            })
                            .min_by_key(|(distance, to, _)| (*distance, *to))
                            .map(|(_, _, action)| action)
                    })
            }
            _ => None,
        }
    }

    pub fn new() -> BasicAi {
        BasicAi {
            minor: false,
            barb: false,
            barbarian_tactics: true,
            amenity_districts: false,
            garrison_under_fire: false,
            reserve_floor: 0.0,
            legacy_defender_purchases: false,
            district_coverage: false,
            district_coverage_2: false,
            slot_kind_tiebreak: false,
            pursue_religion: true,
            bank_faith_for_apostle: false,
            charts_rival_majorities: false,
            skip_prophet_race: false,
            enter_prophet_race: false,
            prophet_race_takes_a_district_slot: false,
            prophet_race_takes_a_district_slot_2: false,
            victor_first_for_a_short_runway: false,
            capital_keeps_its_governor: false,
            science_building_first: false,
            housing_reserve: false,
            campus_before_harbor: false,
            capital_campus_first: false,
            monument_first: false,
            builder_before_the_army: false,
            builder_before_the_army_2: false,
            builder_before_the_army_3: false,
            builders_before_the_lent_floor: false,
            unique_unit_preference: false,
            prefers_priced_melee: false,
            campus_before_the_army: false,
            campus_before_the_army_2: false,
            campus_before_the_army_3: false,
            settler_before_the_navy: false,
            granary_before_the_army: false,
            front_weighted_floor: false,
            front_objective: None,
            front_weighted_floor_2: false,
            activation_resume_waits: false,
            activation_keeps_its_building: false,
            artifact_before_more_museums: false,
            granary_before_the_army_2: false,
            housing_bound_city_builds_its_granary: false,
            housing_cap_builds_the_aqueduct: false,
            builders_cover_the_worked_backlog: false,
            prod_tiles_call_the_builders: false,
            commercial_hub_and_traders: false,
            plan_threatened_city: None,
            industry_before_the_army: false,
            industry_before_the_army_2: false,
            industry_before_the_army_3: false,
            industry_in_the_district_list: false,
            plaza_in_the_district_list: false,
            kristina_builds_the_bibliotheque: false,
            kristina_fills_to_theme: false,
            industrial_hub: false,
            industrial_zone_in_the_producers: false,
            district_buildings_first: false,
            capital_library_first: false,
            campus_buildings_first: false,
            nuclear_goal_needs_a_launcher: false,
            culture_defense_theater: false,
            lent_military_floor_base: None,
            exclude_space_race: false,
            exclude_fossil_power: false,
            defer_low_impact_science_activation_paths: false,
            pantheon_reads_the_board: false,
            apostle_promotion_by_role: false,
            band_plays_its_stage: false,
            bands_draw_from_the_bar: false,
            band_plays_what_it_can_reach: false,
            bands_wait_at_the_door: false,
            band_trips: RefCell::new(HashMap::new()),
            band_refused_hosts: RefCell::new(HashMap::new()),
            bank_envoys: false,
            live_religious_purchase_guard: false,
            recon_replacement: false,
            naval_recon: false,
            naval_recon_2: false,
            naval_recon_3: false,
            island_exploration: false,
            contact_fleet: false,
            camp_party: false,
            come_ashore: false,
            barbarian_settler_capture: false,
            loyalty_rate_alarm: false,
            recorded_tactical_step: false,
            live_motion_turn_accounting: false,
            whole_turn_backtrack_guard: false,
            legal_tactical_candidates: false,
            unit_objective_memory: false,
            precise_evacuation: true,
            veteran_retreat_margin: false,
            ranged_hp_reserve: false,
            one_shot_recovery: false,
            w: Weights::default(),
            book_pos: 0,
            book_settler_pending: false,
            recovering_units: HashSet::new(),
            patrol_targets: HashMap::new(),
            patrol_posts: HashMap::new(),
            patrol_posts_by_class: HashMap::new(),
            settler_targets: HashMap::new(),
            open_water_navy: false,
            unit_memories: RefCell::new(BTreeMap::new()),
            attack_envelope_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            enemy_envelope_cache: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            envelope_board: std::sync::Arc::new(std::sync::Mutex::new(None)),
            covered_tiles_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            safe_healing_targets_cache: FreshOnClone::default(),
            envelope_cache_across_own_moves: false,
            last_path_step_from: RefCell::new(HashMap::new()),
            explore_dead_targets: false,
            explore_last: RefCell::new(HashMap::new()),
            explore_dead: RefCell::new(HashMap::new()),
            explore_commit: false,
            wonder_ring_recon: false,
            opening_defense_preempts: 0,
            opening_defense_preempts_once: false,
            wonder_ring_recon_2: false,
            hut_collection: false,
            village_seeking: false,
            sea_answers: false,
            camp_bounty: false,
            adjacent_camp_clear: true,
            enemy_of_my_enemy: false,
            quest_camp_errand: false,
            contested_land_first: false,
            exchange_is_the_engines: false,
            defend_where_you_stand: false,
            legacy_movement: false,
            barbarian_heretic_hunt: true,
            deals_for_our_gain: false,
            deals_at_the_ceiling: false,
            no_free_passage: false,
            barbarian_bargain: false,
            barbarian_ranged_answer: false,
            siege_preempts_the_queue: false,
            camp_bounty_claims: BTreeMap::new(),
            maintenance_aware_deck: false,
            deck_reads_player_income: false,
            retainers_garrison_the_short_cities: false,
            retainers_garrison_claims: BTreeMap::new(),
            garrison_before_the_surprise: false,
            garrison_call_cache: None,
            spy_training_paused: false,
            obsolete_military_buys_barred: false,
            obsolete_buys_read_our_techs: false,
            plaza_serves_science: false,
            launch_pad_resumes_its_foundation: false,
            digs_one_museum: false,
            dig_museums_wanted: 0,
            founder_takes_the_faith_beliefs: false,
            preserve_culture_resort_sites: false,
            deck_prices_growth: false,
            deck_follows_the_queue: false,
            deck_queue_signature: RefCell::new(BTreeMap::new()),
            solvency_first_trade_slot: false,
            builder_tries_the_next_tile: false,
            naval_threat_triage: false,
            explore_goal: RefCell::new(HashMap::new()),
            unit_motion: BTreeMap::new(),
            move_refusal_break: false,
            move_refusal_watch: RefCell::new(HashMap::new()),
            move_refusal_strikes: HashMap::new(),
            move_refusal_blocks: HashMap::new(),
            own_column_is_not_a_refusal: false,
            host_move_feedback: HashMap::new(),
            host_move_feedback_active: false,
            move_refusal_congested: RefCell::new(HashMap::new()),
            frame_own_tiles: HashMap::new(),
            gun_lethal_tiles: HashMap::new(),
            unit_lethal_blows: HashMap::new(),
            rush_military_floor: 0,
            settler_strand_discount: false,
            settler_backlog_brake: false,
            settler_backlog_brake_2: false,
            parallel_settlers: false,
            host_settler_pop: false,
            land_grab: false,
            rapid_city_expansion_2: false,
            capital_settler_after_completion: false,
            opening_builder_after_the_first_settler: false,
            opening_builder_paid: false,
            first_pad_in_the_forge: false,
            scout_first_opening: false,
            opening_ranged_support: false,
            expansion_pantheon: false,
            pantheon_for_the_prophet_race: false,
            opening_settler_waits: false,
            settler_idle: BTreeMap::new(),
            settler_idle_turn: None,
            journal: Journal::default(),
        }
    }

    pub(crate) fn barbarian_tactics_enabled(&self) -> bool {
        self.barbarian_tactics
    }

    pub(crate) fn set_defer_low_impact_science_activation_paths(&mut self, defer: bool) {
        self.defer_low_impact_science_activation_paths = defer;
    }

    /// Open the second settler pipeline slot (see `parallel_settlers`). The
    /// Civilization VI bridge sets this through `AdvancedAi::enable_parallel_settlers`;
    /// exposed here so the bridge's `--explain-settler` probe can play the same gate.
    pub fn enable_parallel_settlers(&mut self) {
        self.parallel_settlers = true;
    }

    /// Build a Settler at the host's population floor (see `host_settler_pop`).
    /// The Civilization VI bridge sets this through
    /// `AdvancedAi::enable_host_settler_pop`; exposed here so the bridge's
    /// `--explain-settler` probe can play the same gate.
    pub fn enable_host_settler_pop(&mut self) {
        self.host_settler_pop = true;
    }

    /// Expand at pipeline pace to the land the empire can hold (see
    /// `land_grab`). The Civilization VI bridge sets this through
    /// `AdvancedAi::enable_land_grab`; exposed here so the bridge's
    /// `--explain-settler` probe can play the same gate.
    pub fn enable_land_grab(&mut self) {
        self.land_grab = true;
    }

    /// Enable the baseline half of the selective rapid-expansion rewrite.
    pub(crate) fn enable_rapid_city_expansion_2(&mut self) {
        self.rapid_city_expansion_2 = true;
    }

    /// Disable the baseline half of `rapid-city-expansion-2`.
    pub(crate) fn disable_rapid_city_expansion_2(&mut self) {
        self.rapid_city_expansion_2 = false;
    }

    /// Enable the baseline half of the capital-settler-after-completion gene.
    /// Whether the capital is still inside the opening book's first four
    /// builds AND a population-two Settler gene (`capital-settler-after-completion`
    /// or `rapid-city-expansion-2`) will queue a Settler the next time the
    /// capital's queue empties, ahead of anything the strategic layer asks
    /// for. `conquest-opening-needs-the-production` prices that Settler.
    pub(crate) fn book_settler_ahead(&self) -> bool {
        (self.capital_settler_after_completion || self.rapid_city_expansion_2) && self.book_pos < 4
    }

    pub(crate) fn enable_capital_settler_after_completion(&mut self) {
        self.capital_settler_after_completion = true;
    }

    /// Disable the baseline half of the capital-settler-after-completion gene.
    pub(crate) fn disable_capital_settler_after_completion(&mut self) {
        self.capital_settler_after_completion = false;
    }

    /// See `retainers_garrison_the_short_cities`.
    pub(crate) fn enable_retainers_garrison_the_short_cities(&mut self) {
        self.retainers_garrison_the_short_cities = true;
    }

    /// The twin of `enable_retainers_garrison_the_short_cities`.
    pub(crate) fn disable_retainers_garrison_the_short_cities(&mut self) {
        self.retainers_garrison_the_short_cities = false;
    }

    /// See `garrison_before_the_surprise`.
    pub(crate) fn enable_garrison_before_the_surprise(&mut self) {
        self.garrison_before_the_surprise = true;
    }

    /// The twin of `enable_garrison_before_the_surprise`.
    pub(crate) fn disable_garrison_before_the_surprise(&mut self) {
        self.garrison_before_the_surprise = false;
        self.garrison_call_cache = None;
    }

    /// The cities of `pid` holding an Archaeological Museum, built or in any
    /// queue (`culture-digs-every-site`).
    pub(crate) fn dig_museums_owned(g: &Game, pid: usize) -> usize {
        let dig = crate::name!("archaeological_museum");
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| {
                city.buildings.iter().any(|built| g.building_is_family(*built, dig))
                    || city.queue.iter().any(|item| {
                        matches!(item, Item::Building { building } if g.building_is_family(*building, dig))
                    })
            })
            .count()
    }

    /// Whether `pid` owns an Archaeological Museum, built or in any queue.
    pub(crate) fn owns_dig_museum(g: &Game, pid: usize) -> bool {
        let dig = crate::name!("archaeological_museum");
        g.cities.values().filter(|city| city.owner == pid).any(|city| {
            city.buildings.iter().any(|built| g.building_is_family(*built, dig))
                || city.queue.iter().any(|item| {
                    matches!(item, Item::Building { building } if g.building_is_family(*building, dig))
                })
        })
    }

    /// See `launch_pad_resumes_its_foundation`.
    pub(crate) fn enable_launch_pad_resumes_its_foundation(&mut self) {
        self.launch_pad_resumes_its_foundation = true;
    }

    /// The twin of `enable_launch_pad_resumes_its_foundation`.
    pub(crate) fn disable_launch_pad_resumes_its_foundation(&mut self) {
        self.launch_pad_resumes_its_foundation = false;
    }

    /// See `deck_prices_growth`.
    pub(crate) fn enable_deck_prices_growth(&mut self) {
        self.deck_prices_growth = true;
    }

    /// The twin of `enable_deck_prices_growth`.
    pub(crate) fn disable_deck_prices_growth(&mut self) {
        self.deck_prices_growth = false;
    }

    /// See `deck_follows_the_queue`.
    pub(crate) fn enable_deck_follows_the_queue(&mut self) {
        self.deck_follows_the_queue = true;
    }

    /// The twin of `enable_deck_follows_the_queue`.
    pub(crate) fn disable_deck_follows_the_queue(&mut self) {
        self.deck_follows_the_queue = false;
        self.deck_queue_signature.borrow_mut().clear();
    }

    /// See `opening_builder_after_the_first_settler`.
    pub(crate) fn enable_opening_builder_after_the_first_settler(&mut self) {
        self.opening_builder_after_the_first_settler = true;
    }

    /// The twin of `enable_opening_builder_after_the_first_settler`.
    pub(crate) fn disable_opening_builder_after_the_first_settler(&mut self) {
        self.opening_builder_after_the_first_settler = false;
    }

    /// Lead the pantheon prefix with the two that found a city (see
    /// `expansion_pantheon`). The Civilization VI bridge sets this through
    /// `AdvancedAi::enable_expansion_pantheon`.
    pub fn enable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = true;
    }

    /// Hold the opening book's Settler slot for the host's population floor
    /// instead of burning it (see `opening_settler_waits`). The Civilization
    /// VI bridge sets this through `AdvancedAi::enable_opening_settler_waits`.
    pub fn enable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = true;
    }

    /// Give up an exploration target the host will not move the unit toward
    /// (see `explore_dead_targets`). The Civilization VI bridge sets this
    /// through `AdvancedAi::enable_explore_dead_targets`.
    pub fn enable_explore_dead_targets(&mut self) {
        self.explore_dead_targets = true;
    }

    /// Let the live bridge count a persistent unit's motion once for each host
    /// turn, even when Firaxis asks for a same-turn replan after an order.
    pub(crate) fn enable_live_motion_turn_accounting(&mut self) {
        self.live_motion_turn_accounting = true;
    }

    /// The withholding twin for the live evaluator control arm.
    pub(crate) fn disable_live_motion_turn_accounting(&mut self) {
        self.live_motion_turn_accounting = false;
    }

    /// Hold an exploration goal and sweep outward from home (see
    /// `explore_commit`). The Civilization VI bridge sets this through
    /// `AdvancedAi::enable_explore_commit`.
    pub fn enable_explore_commit(&mut self) {
        self.explore_commit = true;
    }

    /// Let the plan-aware live envoy pass retain an envoy when no productive
    /// city-state placement is available this turn.
    pub(crate) fn enable_bank_envoys(&mut self) {
        self.bank_envoys = true;
    }

    pub(crate) fn disable_bank_envoys(&mut self) {
        self.bank_envoys = false;
    }

    /// Buy one ship for an empire that has none while unexplored water lies
    /// off its coast. See `naval_recon`.
    pub fn enable_naval_recon(&mut self) {
        self.naval_recon = true;
    }

    pub fn disable_naval_recon(&mut self) {
        self.naval_recon = false;
    }

    /// Count a barbarian camp within nine tiles of a city as home ground.
    /// Answer a ring of shooters with a shooter. See `barbarian_ranged_answer`.
    pub fn enable_barbarian_ranged_answer(&mut self) {
        self.barbarian_ranged_answer = true;
    }

    pub fn disable_barbarian_ranged_answer(&mut self) {
        self.barbarian_ranged_answer = false;
    }

    /// A raider on the doorstep is answered before anything else is built,
    /// bought when no defender exists, and a Scout is not a defender. See
    /// `siege_preempts_the_queue` and `advanced/siege_response.rs`.
    pub fn enable_siege_preempts_the_queue(&mut self) {
        self.siege_preempts_the_queue = true;
    }

    pub fn disable_siege_preempts_the_queue(&mut self) {
        self.siege_preempts_the_queue = false;
    }

    /// Price a raider's life below a major's. See `barbarian_bargain`.
    pub fn enable_barbarian_bargain(&mut self) {
        self.barbarian_bargain = true;
    }

    pub fn disable_barbarian_bargain(&mut self) {
        self.barbarian_bargain = false;
    }

    /// The whole peacetime field army answers home threats and a camp in
    /// reach outranks the countryside. See `camp_party`.
    pub fn enable_camp_party(&mut self) {
        self.camp_party = true;
    }

    pub fn disable_camp_party(&mut self) {
        self.camp_party = false;
    }

    pub fn with_weights(w: Weights) -> BasicAi {
        BasicAi {
            minor: false,
            barb: false,
            barbarian_tactics: true,
            amenity_districts: false,
            garrison_under_fire: false,
            reserve_floor: 0.0,
            legacy_defender_purchases: false,
            district_coverage: false,
            district_coverage_2: false,
            slot_kind_tiebreak: false,
            pursue_religion: true,
            bank_faith_for_apostle: false,
            charts_rival_majorities: false,
            skip_prophet_race: false,
            enter_prophet_race: false,
            prophet_race_takes_a_district_slot: false,
            prophet_race_takes_a_district_slot_2: false,
            victor_first_for_a_short_runway: false,
            capital_keeps_its_governor: false,
            science_building_first: false,
            housing_reserve: false,
            campus_before_harbor: false,
            capital_campus_first: false,
            monument_first: false,
            builder_before_the_army: false,
            builder_before_the_army_2: false,
            builder_before_the_army_3: false,
            builders_before_the_lent_floor: false,
            unique_unit_preference: false,
            prefers_priced_melee: false,
            campus_before_the_army: false,
            campus_before_the_army_2: false,
            campus_before_the_army_3: false,
            settler_before_the_navy: false,
            granary_before_the_army: false,
            front_weighted_floor: false,
            front_objective: None,
            front_weighted_floor_2: false,
            activation_resume_waits: false,
            activation_keeps_its_building: false,
            artifact_before_more_museums: false,
            granary_before_the_army_2: false,
            housing_bound_city_builds_its_granary: false,
            housing_cap_builds_the_aqueduct: false,
            builders_cover_the_worked_backlog: false,
            prod_tiles_call_the_builders: false,
            commercial_hub_and_traders: false,
            plan_threatened_city: None,
            industry_before_the_army: false,
            industry_before_the_army_2: false,
            industry_before_the_army_3: false,
            industry_in_the_district_list: false,
            plaza_in_the_district_list: false,
            kristina_builds_the_bibliotheque: false,
            kristina_fills_to_theme: false,
            industrial_hub: false,
            industrial_zone_in_the_producers: false,
            district_buildings_first: false,
            capital_library_first: false,
            campus_buildings_first: false,
            nuclear_goal_needs_a_launcher: false,
            culture_defense_theater: false,
            lent_military_floor_base: None,
            exclude_space_race: false,
            exclude_fossil_power: false,
            defer_low_impact_science_activation_paths: false,
            pantheon_reads_the_board: false,
            apostle_promotion_by_role: false,
            band_plays_its_stage: false,
            bands_draw_from_the_bar: false,
            band_plays_what_it_can_reach: false,
            bands_wait_at_the_door: false,
            band_trips: RefCell::new(HashMap::new()),
            band_refused_hosts: RefCell::new(HashMap::new()),
            bank_envoys: false,
            live_religious_purchase_guard: false,
            recon_replacement: false,
            naval_recon: false,
            naval_recon_2: false,
            naval_recon_3: false,
            island_exploration: false,
            contact_fleet: false,
            camp_party: false,
            come_ashore: false,
            barbarian_settler_capture: false,
            loyalty_rate_alarm: false,
            recorded_tactical_step: false,
            live_motion_turn_accounting: false,
            whole_turn_backtrack_guard: false,
            legal_tactical_candidates: false,
            unit_objective_memory: false,
            precise_evacuation: true,
            veteran_retreat_margin: false,
            ranged_hp_reserve: false,
            one_shot_recovery: false,
            w,
            book_pos: 0,
            book_settler_pending: false,
            recovering_units: HashSet::new(),
            patrol_targets: HashMap::new(),
            patrol_posts: HashMap::new(),
            patrol_posts_by_class: HashMap::new(),
            settler_targets: HashMap::new(),
            open_water_navy: false,
            unit_memories: RefCell::new(BTreeMap::new()),
            attack_envelope_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            enemy_envelope_cache: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            envelope_board: std::sync::Arc::new(std::sync::Mutex::new(None)),
            covered_tiles_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
            safe_healing_targets_cache: FreshOnClone::default(),
            envelope_cache_across_own_moves: false,
            last_path_step_from: RefCell::new(HashMap::new()),
            explore_dead_targets: false,
            explore_last: RefCell::new(HashMap::new()),
            explore_dead: RefCell::new(HashMap::new()),
            explore_commit: false,
            wonder_ring_recon: false,
            opening_defense_preempts: 0,
            opening_defense_preempts_once: false,
            wonder_ring_recon_2: false,
            hut_collection: false,
            village_seeking: false,
            sea_answers: false,
            camp_bounty: false,
            adjacent_camp_clear: true,
            enemy_of_my_enemy: false,
            quest_camp_errand: false,
            contested_land_first: false,
            exchange_is_the_engines: false,
            defend_where_you_stand: false,
            legacy_movement: false,
            barbarian_heretic_hunt: true,
            deals_for_our_gain: false,
            deals_at_the_ceiling: false,
            no_free_passage: false,
            barbarian_bargain: false,
            barbarian_ranged_answer: false,
            siege_preempts_the_queue: false,
            camp_bounty_claims: BTreeMap::new(),
            maintenance_aware_deck: false,
            deck_reads_player_income: false,
            retainers_garrison_the_short_cities: false,
            retainers_garrison_claims: BTreeMap::new(),
            garrison_before_the_surprise: false,
            garrison_call_cache: None,
            spy_training_paused: false,
            obsolete_military_buys_barred: false,
            obsolete_buys_read_our_techs: false,
            plaza_serves_science: false,
            launch_pad_resumes_its_foundation: false,
            digs_one_museum: false,
            dig_museums_wanted: 0,
            founder_takes_the_faith_beliefs: false,
            preserve_culture_resort_sites: false,
            deck_prices_growth: false,
            deck_follows_the_queue: false,
            deck_queue_signature: RefCell::new(BTreeMap::new()),
            solvency_first_trade_slot: false,
            builder_tries_the_next_tile: false,
            naval_threat_triage: false,
            explore_goal: RefCell::new(HashMap::new()),
            unit_motion: BTreeMap::new(),
            move_refusal_break: false,
            move_refusal_watch: RefCell::new(HashMap::new()),
            move_refusal_strikes: HashMap::new(),
            move_refusal_blocks: HashMap::new(),
            own_column_is_not_a_refusal: false,
            host_move_feedback: HashMap::new(),
            host_move_feedback_active: false,
            move_refusal_congested: RefCell::new(HashMap::new()),
            frame_own_tiles: HashMap::new(),
            gun_lethal_tiles: HashMap::new(),
            unit_lethal_blows: HashMap::new(),
            rush_military_floor: 0,
            settler_strand_discount: false,
            settler_backlog_brake: false,
            settler_backlog_brake_2: false,
            parallel_settlers: false,
            host_settler_pop: false,
            land_grab: false,
            rapid_city_expansion_2: false,
            capital_settler_after_completion: false,
            opening_builder_after_the_first_settler: false,
            opening_builder_paid: false,
            first_pad_in_the_forge: false,
            scout_first_opening: false,
            opening_ranged_support: false,
            expansion_pantheon: false,
            pantheon_for_the_prophet_race: false,
            opening_settler_waits: false,
            settler_idle: BTreeMap::new(),
            settler_idle_turn: None,
            journal: Journal::default(),
        }
    }

    pub fn fleet(g: &Game) -> Vec<BasicAi> {
        g.players.iter().map(|_| BasicAi::new()).collect()
    }

    /// Majors get `w`; minors/barbarians keep default weights.
    pub fn fleet_weighted(g: &Game, w: &Weights) -> Vec<BasicAi> {
        g.players
            .iter()
            .map(|p| {
                if p.is_minor || p.is_barbarian {
                    BasicAi::new()
                } else {
                    BasicAi::with_weights(w.clone())
                }
            })
            .collect()
    }
}

impl Ai for BasicAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        // Stamp the context once. Nothing below repeats the turn number or the
        // acting civilization; the journal carries both. `AdvancedAi` opens
        // the turn on the shared journal before delegating here, and doing it
        // again is the same statement, so the baseline running on its own is
        // recorded identically.
        self.journal.begin_turn(g.turn, pid);
        g.with_deferred_visibility(|g| self.take_turn_inner(g, pid));
    }

    fn attach_journal(&mut self, journal: Journal) {
        self.journal = journal;
    }
}

impl BasicAi {
    fn take_turn_inner(&mut self, g: &mut Game, pid: usize) {
        self.minor = g.players[pid].is_minor;
        // Free Cities are diplomatically hostile like barbarians, but unlike
        // camps they keep developing their inherited cities and training
        // defenders through the ordinary minor-civilization production AI.
        self.barb = g.players[pid].is_barbarian && !g.players[pid].is_free_city;
        self.resolve_city_dispositions(g, pid, false, false);
        if !self.barb {
            if self.minor {
                // Minor civilizations keep a technology/civic tree and develop
                // their city, but do not run a player's corporations, diplomacy,
                // espionage, government, religion, governor, or envoy agenda.
                self.minor_research(g, pid);
            } else {
                self.research(g, pid);
                self.corporations(g, pid);
                self.diplomacy(g, pid);
                self.spies(g, pid);
            }
            self.cities(g, pid);
        }
        Self::upgrade_units(g, pid);
        self.units(g, pid);
        self.resolve_city_dispositions(g, pid, false, false);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
}

impl BasicAi {
    /// Return a snapshot of one unit's durable reasoning state.
    ///
    /// This is intentionally observer-facing: a spectator, evaluator, or
    /// bridge can explain why one individual unit is moving without getting a
    /// mutable handle to the controller's internal maps.
    pub fn unit_memory(&self, uid: u32) -> Option<UnitMemory> {
        self.unit_memories.borrow().get(&uid).cloned()
    }

    /// Assign a military unit to the city the current campaign means to take.
    /// Reconfirming the same job preserves its original start turn, which is
    /// how an observer can distinguish a long-running siege from a retarget.
    pub(crate) fn remember_capture_objective(&self, g: &Game, pid: usize, uid: u32, city: u32) {
        if !self.unit_objective_memory || !g.units.get(&uid).is_some_and(|unit| unit.owner == pid) {
            return;
        }
        let Some(target_city) = g.cities.get(&city).filter(|city| city.owner != pid) else {
            return;
        };
        let target = target_city.pos;
        let mut memories = self.unit_memories.borrow_mut();
        let memory = memories.entry(uid).or_default();
        match &mut memory.objective {
            Some(UnitObjective::CaptureCity {
                city: known_city,
                target: known_target,
                last_confirmed_turn,
                ..
            }) if *known_city == city && *known_target == target => {
                *last_confirmed_turn = g.turn;
            }
            _ => {
                memory.objective = Some(UnitObjective::CaptureCity {
                    city,
                    target,
                    started_turn: g.turn,
                    last_confirmed_turn: g.turn,
                });
            }
        }
    }

    /// See `envelope_cache_across_own_moves`. Evaluator arm only.
    pub fn enable_envelope_cache_across_own_moves(&mut self) {
        self.envelope_cache_across_own_moves = true;
    }

    /// The total of `incoming_damage`, which is what every route and refuge
    /// decision has always read.
    fn evacuation_incoming_damage(
        g: &Game,
        pid: usize,
        uid: u32,
        position: Pos,
        envelopes: &[(u32, std::sync::Arc<EnvelopeReach>)],
    ) -> f64 {
        Self::incoming_damage(g, pid, uid, position, envelopes).total
    }

    /// Whether this hostile City Center can bombard `position` next turn.
    /// Named because `incoming_damage` and `safe_healing_step` must agree on
    /// it exactly: one prices the tile and the other skips pricing it, and a
    /// difference between the two is a different game, not a faster one.
    fn city_centre_strikes(g: &Game, pid: usize, city: &crate::game::City, position: Pos) -> bool {
        city.owner != pid
            && g.is_at_war(pid, city.owner)
            && city.wall_hp > 0
            && g.wdist(city.pos, position) <= 2
            && g.line_of_sight_from(city.pos, position)
    }

    /// The Encampment counterpart of [`Self::city_centre_strikes`].
    fn city_encampment_strikes(
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        position: Pos,
    ) -> bool {
        city.owner != pid
            && g.is_at_war(pid, city.owner)
            && g.defending_districts(city).any(|district| {
                district.hp > 0
                    && district.wall_hp > 0
                    && !district.pillaged
                    && g.wdist(district.pos, position) <= 2
                    && g.line_of_sight_from(district.pos, position)
            })
    }

    /// Whether any observed hostile unit, City Center or Encampment could
    /// damage a unit standing on `position` next turn. Exactly the union of
    /// the three source sets [`Self::incoming_damage`] folds over, so "no" is
    /// precisely the case where all three folds are empty and the answer is
    /// `IncomingDamage::default()`.
    pub(crate) fn anything_can_reach(
        g: &Game,
        pid: usize,
        position: Pos,
        envelopes: &[(u32, std::sync::Arc<EnvelopeReach>)],
    ) -> bool {
        Self::movement_hazard_damage(g, pid, position) > 0.0
            || envelopes.iter().any(|(enemy_id, reach)| {
                reach.contains(&position) && g.units.contains_key(enemy_id)
            })
            || g.cities
                .values()
                .any(|city| Self::city_centre_strikes(g, pid, city, position))
            || g.cities
                .values()
                .any(|city| Self::city_encampment_strikes(g, pid, city, position))
    }

    /// Conservative expected damage from the hostile units whose precise
    /// next-turn envelopes cover `position`, as a total and as the largest
    /// single blow among the same sources. The engine still rolls combat;
    /// this intentionally uses its unrandomized centre so a route decision is
    /// stable and does not consume the game RNG.
    ///
    /// ⚠ THE ANSWER IS ZERO FOR MOST TILES, AND THE PRICE OF THE ZERO IS WHAT
    /// THIS COSTS. Every source is filtered out before the arithmetic on a
    /// tile nothing covers, so the whole result is `IncomingDamage::default()`
    /// — but the defender's strength was priced first, on every call, and a
    /// garrisoned tile discarded three answers it had already paid for. The
    /// sources are gathered first now and the defender is priced only when one
    /// of them exists. Skipping work whose answer was already zero, never
    /// changing an answer: the empty folds this replaces returned exactly
    /// `IncomingDamage::default()`, and `default().merge(default())` is
    /// `default()`.
    pub(crate) fn incoming_damage(
        g: &Game,
        pid: usize,
        uid: u32,
        position: Pos,
        envelopes: &[(u32, std::sync::Arc<EnvelopeReach>)],
    ) -> IncomingDamage {
        let Some(unit) = g.units.get(&uid) else {
            return IncomingDamage::LETHAL;
        };
        // A garrisoned unit is not a combat target: attacks on a City Center
        // or Encampment damage that district, not the formation stationed in
        // it. That makes a friendly city a genuine safe refuge even while the
        // enemy can still bombard its walls.
        let hazard = IncomingDamage::default().with(Self::movement_hazard_damage(g, pid, position));
        let garrisoned =
            g.city_at(position).is_some() || g.defending_district_at(position).is_some();
        if garrisoned {
            return hazard;
        }
        if !Self::anything_can_reach(g, pid, position, envelopes) {
            return IncomingDamage::default();
        }
        let mut defender = unit.clone();
        defender.pos = position;
        if position != unit.pos {
            defender.fortified = false;
            defender.fortify_turns = 0;
        }
        let defense = effective_strength(
            g.unit_strength(&defender, true) + g.tile_defense_bonus(position),
            defender.hp,
        );
        let unit_damage: IncomingDamage = {
            envelopes
                .iter()
                .filter(|(_, reach)| reach.contains(&position))
                .filter_map(|(enemy_id, _)| g.units.get(enemy_id).map(|enemy| (*enemy_id, enemy)))
                .map(|(enemy_id, enemy)| {
                    let spec = &g.rules.units[enemy.kind];
                    let attack = if spec.has_ranged_attack() {
                        g.unit_ranged_attack_strength(enemy)
                    } else {
                        g.unit_strength(enemy, false)
                    } + Self::class_matchup_strength(g, enemy_id, uid);
                    let defender_strength =
                        defense + Self::class_matchup_strength(g, uid, enemy_id);
                    (30.0
                        * ((effective_strength(attack, enemy.hp) - defender_strength) / 25.0).exp())
                    .clamp(1.0, 100.0)
                })
                .fold(IncomingDamage::default(), IncomingDamage::with)
        };

        // A walled hostile City Center can strike on its next turn even when
        // it already spent this turn's strike. A garrison on a City Center or
        // Encampment is protected by that district rather than directly hit —
        // handled by the early return above.
        let city_damage: IncomingDamage = {
            g.cities
                .values()
                .filter(|city| Self::city_centre_strikes(g, pid, city, position))
                .map(|city| {
                    (30.0 * ((g.city_ranged_strength(city.id) - defense) / 25.0).exp())
                        .clamp(1.0, 100.0)
                })
                .fold(IncomingDamage::default(), IncomingDamage::with)
        };
        // Encampments carry an independent strike. As above, a strike spent
        // today is available again by the enemy's next turn, so only the
        // durable wall, health, and pillage state constrain this envelope.
        let encampment_damage: IncomingDamage = {
            g.cities
                .values()
                .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
                .flat_map(|city| {
                    g.defending_districts(city)
                        .filter(move |district| {
                            district.hp > 0
                                && district.wall_hp > 0
                                && !district.pillaged
                                && g.wdist(district.pos, position) <= 2
                                && g.line_of_sight_from(district.pos, position)
                        })
                        .map(move |_| {
                            (30.0 * ((g.city_ranged_strength(city.id) - defense) / 25.0).exp())
                                .clamp(1.0, 100.0)
                        })
                })
                .fold(IncomingDamage::default(), IncomingDamage::with)
        };
        unit_damage
            .merge(city_damage)
            .merge(encampment_damage)
            .merge(hazard)
    }

    /// Remove stale observations, completed objectives, and entries for units
    /// that no longer belong to this player. This must run before any unit
    /// moves, just like the older motion ledger it complements.
    fn refresh_unit_memories(&self, g: &Game, pid: usize) {
        self.unit_memories.borrow_mut().retain(|uid, memory| {
            let Some(unit) = g.units.get(uid) else {
                return false;
            };
            if unit.owner != pid {
                return false;
            }
            if let Some(UnitObjective::CaptureCity { city, target, .. }) = memory.objective {
                if g.cities
                    .get(&city)
                    .is_none_or(|known| known.owner == pid || known.pos != target)
                {
                    memory.objective = None;
                }
            }
            if memory
                .danger
                .is_some_and(|danger| g.turn >= danger.expires_turn)
            {
                memory.danger = None;
            }
            if memory.retreat_until.is_some_and(|until| g.turn >= until) {
                memory.retreat_until = None;
            }
            memory.objective.is_some() || memory.danger.is_some() || memory.retreat_until.is_some()
        });
    }

    /// Mark the scripted four-build capital opening as already played, so the
    /// utility planner governs production from the first turn this agent
    /// sees.
    ///
    /// ★★★★ A DECIDER RESTARTED MID-GAME REPLAYS THE OPENING BOOK OVER SEVEN
    /// CITIES. The live bridge restarts its persistent agent whenever
    /// `origin/main` advances (run civvis-20260816T213447Z: t139 and t157),
    /// and a fresh agent has `book_pos = 0` — so `AdvancedAi::take_turn`'s
    /// "preserve the proven four-build opening" branch hands EVERY city to
    /// `BasicAi::cities` until the capital has emptied its queue four more
    /// times. Measured: at t139 five cities started Skirmishers and at t140
    /// all seven switched to Rangers ("the empire has no eyes", one per city)
    /// — the Taj Mahal, St Basil's and two campus projects abandoned — and
    /// Rangers were built in every city until t150; at t157 the same again
    /// with Artillery, Bolshoi and the Hermitage dropped. Roughly ten turns of
    /// the whole empire's production per restart, on a seat that restarts
    /// several times a game. The book is the capital's first four builds of a
    /// game that starts at turn 1; an agent that first sees turn 139 has no
    /// opening to play. See `AdvancedAi::skip_opening_book`.
    pub fn skip_opening_book(&mut self) {
        self.book_pos = self.book_pos.max(4);
        self.book_settler_pending = false;
    }

    /// Whether the scripted opening has been played (or skipped). See
    /// `skip_opening_book`.
    pub fn opening_book_played(&self) -> bool {
        self.book_pos >= 4
    }

    /// Drop everything this agent remembers ABOUT INDIVIDUAL UNITS, keeping the rest.
    ///
    /// ★★★★★ FOR MIRRORING A GAME WHOSE UNIT IDS ARE REASSIGNED EVERY TURN.
    /// `civvis-orders --serve --fresh-board` keeps one agent alive across a real
    /// Civilization VI game and rebuilds the board each turn, because `take_turn`
    /// cannot be run twice on a board that has not advanced through `begin_turn`.
    /// Rebuilding reassigns unit ids, so every id-keyed map here silently describes a
    /// DIFFERENT unit than it did last turn.
    ///
    /// `unit_motion` is the one that does the damage: it is the livelock detector, so
    /// a unit whose recorded history jumps around looks like it is going in circles,
    /// and `hold_stood_down_unit` then fortifies it and suppresses it for several
    /// turns. Measured cost of not calling this — same code, same settings, one agent
    /// persisted across a real game: **about 1 unit order per turn against 13**, and by
    /// turn 62 the empire was ONE city and ONE unit where the no-continuity arm had 2
    /// cities and 25 units at turn 82. Continuity that poisons the unit layer is worse
    /// than no continuity.
    ///
    /// What is deliberately KEPT is the strategic plan, which is the whole reason to
    /// persist an agent: grand strategy, war target, city target — none of it keyed to
    /// a unit id.
    pub fn forget_unit_memory(&mut self) {
        self.recovering_units.clear();
        self.patrol_targets.clear();
        self.patrol_posts.clear();
        self.patrol_posts_by_class.clear();
        self.settler_targets.clear();
        self.unit_memories.get_mut().clear();
        self.unit_motion.clear();
        self.move_refusal_watch.get_mut().clear();
        self.band_trips.get_mut().clear();
        self.move_refusal_strikes.clear();
        self.move_refusal_blocks.clear();
        self.host_move_feedback.clear();
        self.move_refusal_congested.get_mut().clear();
        self.frame_own_tiles.clear();
        self.explore_last.get_mut().clear();
        self.explore_dead.get_mut().clear();
        self.explore_goal.get_mut().clear();
    }

    /// Carry unit-keyed memory across a board that was rebuilt underneath it.
    ///
    /// ★★★★★ FORGETTING IS WHY THE SETTLERS WANDER. The Civilization VI bridge
    /// rebuilds the board every turn (`Ai::take_turn` needs a turn that has advanced
    /// through the engine's own private `begin_turn`), and unit ids are reassigned
    /// when it does — so every unit-keyed map described a different unit and the only
    /// safe thing to do was drop it. The cost of dropping it is that the settler's
    /// DESTINATION is re-derived from scratch each turn, and a re-derived optimum
    /// flips: measured on run `civvis-20260731T055749Z`, one settler was told to walk
    /// to a site 23 tiles away on turns 14, 18 and 20 and to a different site 7 tiles
    /// away on turn 16. The livelock detector is unit-keyed too, so the ONE mechanism
    /// that exists to catch a unit going in circles could never fire in the bridge.
    ///
    /// The ids are recoverable, though: the mirror knows each board's Civ 6 id for
    /// every unit, so old id -> Civ 6 id -> new id is a total function on the units
    /// that still exist. Units that died simply drop out, which is what should happen
    /// to their memory anyway.
    pub fn remap_unit_memory(&mut self, map: &std::collections::BTreeMap<u32, u32>) {
        fn remap<V: Clone>(
            old: &HashMap<u32, V>,
            map: &std::collections::BTreeMap<u32, u32>,
        ) -> HashMap<u32, V> {
            old.iter()
                .filter_map(|(uid, value)| map.get(uid).map(|new| (*new, value.clone())))
                .collect()
        }
        self.recovering_units = self
            .recovering_units
            .iter()
            .filter_map(|uid| map.get(uid).copied())
            .collect();
        self.patrol_targets = remap(&self.patrol_targets, map);
        // Posts are cleared every turn by `begin_movement_turn` anyway, and they are
        // claims on ground rather than memory about a unit.
        self.patrol_posts.clear();
        self.patrol_posts_by_class.clear();
        self.settler_targets = remap(&self.settler_targets, map);
        let unit_memories = std::mem::take(self.unit_memories.get_mut());
        *self.unit_memories.get_mut() = unit_memories
            .into_iter()
            .filter_map(|(uid, memory)| map.get(&uid).map(|new| (*new, memory)))
            .collect();
        self.unit_motion = self
            .unit_motion
            .iter()
            .filter_map(|(uid, motion)| map.get(uid).map(|new| (*new, motion.clone())))
            .collect();
        // ⚠ Remapped, not cleared: the watch is judged on the turn AFTER it
        // was written, and on the live bridge the board (and every unit id)
        // is rebuilt in between — clearing here would mean no issued step
        // ever survives to be judged and the gene could never fire.
        let watch = std::mem::take(self.move_refusal_watch.get_mut());
        *self.move_refusal_watch.get_mut() = watch
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        self.move_refusal_strikes = std::mem::take(&mut self.move_refusal_strikes)
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        self.move_refusal_blocks = std::mem::take(&mut self.move_refusal_blocks)
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        self.host_move_feedback = std::mem::take(&mut self.host_move_feedback)
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        let congested = std::mem::take(self.move_refusal_congested.get_mut());
        *self.move_refusal_congested.get_mut() = congested
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        // Rebuilt by `begin_movement_turn` for every frame; a rebuilt board's
        // ids would only make a stale copy lie.
        self.frame_own_tiles.clear();
        // ⚠ Remapped, not cleared. A replan can rebuild the live board after a
        // host-accepted step but before that host turn ends. Clearing this trail
        // then forgets the step the reversal guard must reject, so the replan can
        // immediately walk the unit back where it started. The turn tag makes a
        // carried trail inert on the following turn, when a return step is legal.
        let path_steps = std::mem::take(&mut *self.last_path_step_from.borrow_mut());
        *self.last_path_step_from.borrow_mut() = path_steps
            .into_iter()
            .filter_map(|(uid, trail)| map.get(&uid).map(|new| (*new, trail)))
            .collect();
        let explore_last = std::mem::take(self.explore_last.get_mut());
        *self.explore_last.get_mut() = explore_last
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        let explore_dead = std::mem::take(self.explore_dead.get_mut());
        *self.explore_dead.get_mut() = explore_dead
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
        let explore_goal = std::mem::take(self.explore_goal.get_mut());
        *self.explore_goal.get_mut() = explore_goal
            .into_iter()
            .filter_map(|(uid, value)| map.get(&uid).map(|new| (*new, value)))
            .collect();
    }

    /// Reset caches whose contents depend on the current player's borders and
    /// movement capabilities, and take down where every unit is standing
    /// before any of them moves. Persistent destinations live across turns;
    /// the expensive all-map candidate scan does not need to.
    pub(crate) fn begin_movement_turn(&mut self, g: &Game, pid: usize) {
        self.patrol_posts.clear();
        self.patrol_posts_by_class.clear();
        self.refresh_unit_memories(g, pid);
        self.observe_unit_motion(g, pid);
        self.judge_move_refusals(g, pid);
        if self.own_column_is_not_a_refusal {
            self.frame_own_tiles.clear();
            for uid in g.player_unit_ids(pid) {
                self.frame_own_tiles
                    .entry(g.units[&uid].pos)
                    .or_default()
                    .push(uid);
            }
        }
    }

    /// `own-column-is-not-a-refusal`: whether `to` held another of our units
    /// on `uid`'s stacking layer when this frame's movement began. Firaxis
    /// checks stacking where a move ENDS, so a step the planning board freed
    /// by walking that unit away first is still refused if the host takes
    /// the two orders the other way round -- congestion in our own column,
    /// not ground the host will not let us walk.
    fn frame_start_friend_holds(&self, g: &Game, uid: u32, to: Pos) -> bool {
        self.frame_own_tiles.get(&to).is_some_and(|held| {
            held.iter()
                .any(|other| *other != uid && g.units_share_stacking_layer(uid, *other))
        })
    }

    /// `own-column-is-not-a-refusal`: the live bridge reports, after every
    /// frame's outbound filters have settled, which of the units the plan
    /// moved actually had a move order sent to Firaxis (`sent`) and which
    /// were planned to move but withheld (`unsent`: the air-assault barrier,
    /// a deferred follow-up). A withheld unit never walked, so the watched
    /// step and path trail this frame's plan wrote for it are the plan's, not
    /// the host's: both go back to what they were before the frame was
    /// planned (`before_paths`, `before_watches`), so the next frame may plan
    /// the very same step again. Returns how many units were rolled back.
    /// Inert with the gene off.
    pub(crate) fn note_host_moves(
        &mut self,
        g: &Game,
        before_paths: &HashMap<u32, (u32, Vec<Pos>)>,
        before_watches: &HashMap<u32, (u32, Pos, Pos)>,
        sent: &BTreeSet<u32>,
        unsent: &BTreeSet<u32>,
    ) -> usize {
        if !self.own_column_is_not_a_refusal {
            return 0;
        }
        self.host_move_feedback_active = true;
        for uid in sent {
            self.host_move_feedback.insert(*uid, (g.turn, true));
        }
        let mut rolled_back = 0;
        for uid in unsent.difference(sent) {
            if self.host_move_feedback.get(uid) != Some(&(g.turn, true)) {
                self.host_move_feedback.insert(*uid, (g.turn, false));
            }
            let mut changed = false;
            let trails = self.last_path_step_from.get_mut();
            if trails.get(uid) != before_paths.get(uid) {
                match before_paths.get(uid) {
                    Some(trail) => trails.insert(*uid, trail.clone()),
                    None => trails.remove(uid),
                };
                changed = true;
            }
            let watches = self.move_refusal_watch.get_mut();
            if watches.get(uid) != before_watches.get(uid) {
                match before_watches.get(uid) {
                    Some(watch) => watches.insert(*uid, *watch),
                    None => watches.remove(uid),
                };
                changed = true;
            }
            let watched_this_turn = before_watches
                .get(uid)
                .is_some_and(|(turn, _, _)| *turn == g.turn);
            if !watched_this_turn {
                let congested = self.move_refusal_congested.get_mut();
                if congested.get(uid) == Some(&g.turn) {
                    congested.remove(uid);
                }
            }
            if changed {
                rolled_back += 1;
            }
        }
        rolled_back
    }

    /// `own-column-is-not-a-refusal`: whether the live bridge reported a move
    /// order reaching the host for this unit on `turn`. Always true off the
    /// bridge, where every applied move is the host's own act.
    fn host_received_a_move(&self, uid: u32, turn: u32) -> bool {
        !self.own_column_is_not_a_refusal
            || !self.host_move_feedback_active
            || self.host_move_feedback.get(&uid) == Some(&(turn, true))
    }

    /// `live-move-refusal-break`: judge last turn's issued steps against where
    /// each unit actually stands, which on the live bridge is the host's own
    /// answer. A unit that was issued the same step on
    /// [`MOVE_REFUSAL_STRIKES`] consecutive turns and never left its tile has
    /// been refused by the host (`did_not_move` — 13.9% of all orders in the
    /// 08-30 measurement, the largest single failure), and the refusal event
    /// carries no destination for a block set to learn from. The proof here
    /// needs no new channel: the intended step is ours, and the fresh mirror
    /// shows the unit did not take it. The proved step is barred for
    /// [`MOVE_REFUSAL_BLOCK_TURNS`] standard turns so the pathing must choose
    /// differently; one refused turn stays a report, not a verdict, matching
    /// the trade-route rule that "a verdict needs two".
    fn judge_move_refusals(&mut self, g: &Game, pid: usize) {
        if !self.move_refusal_break {
            return;
        }
        self.move_refusal_blocks.retain(|uid, (_, until)| {
            g.units.get(uid).is_some_and(|unit| unit.owner == pid) && g.turn < *until
        });
        self.move_refusal_strikes
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        let watch = std::mem::take(&mut *self.move_refusal_watch.borrow_mut());
        let mut keep: HashMap<u32, (u32, Pos, Pos)> = HashMap::new();
        let mut blocked: Vec<(u32, Pos, Pos, String)> = Vec::new();
        for (uid, (turn, from, step)) in watch {
            if turn == g.turn {
                // A same-turn replan re-reads the board mid-turn; the host has
                // not answered yet, so this entry is not yet judgeable.
                keep.insert(uid, (turn, from, step));
                continue;
            }
            if turn + 1 != g.turn {
                continue;
            }
            let Some(unit) = g.units.get(&uid).filter(|unit| unit.owner == pid) else {
                continue;
            };
            if unit.pos != from {
                // It moved; whatever held it is gone, so the slate is clean
                // and the next refusal starts a fresh count.
                self.move_refusal_strikes.remove(&uid);
                continue;
            }
            // `own-column-is-not-a-refusal`: a step onto a tile our own column
            // held when the order went out, or one the host was never sent,
            // is no evidence about the ground. Neither a strike nor a reset.
            if self.own_column_is_not_a_refusal
                && (self.move_refusal_congested.get_mut().get(&uid) == Some(&turn)
                    || !self.host_received_a_move(uid, turn))
            {
                continue;
            }
            let strikes = match self.move_refusal_strikes.get(&uid) {
                Some((f, s, n)) if *f == from && *s == step => n + 1,
                _ => 1,
            };
            if strikes >= MOVE_REFUSAL_STRIKES {
                self.move_refusal_strikes.remove(&uid);
                self.move_refusal_blocks.insert(
                    uid,
                    (step, g.turn + g.standard_duration(MOVE_REFUSAL_BLOCK_TURNS)),
                );
                blocked.push((uid, from, step, unit.kind.to_string()));
            } else {
                self.move_refusal_strikes.insert(uid, (from, step, strikes));
            }
        }
        *self.move_refusal_watch.borrow_mut() = keep;
        if self.own_column_is_not_a_refusal {
            let turn = g.turn;
            self.move_refusal_congested
                .get_mut()
                .retain(|_, marked| *marked + 1 >= turn);
        }
        for (uid, from, step, kind) in blocked {
            think!(self.journal, Military, Detail,
                   "{kind} {uid} stops re-asking a move the host keeps refusing";
                   "ordered from {from:?} to {step:?} on {MOVE_REFUSAL_STRIKES} straight turns \
                    without moving; that step is barred for {MOVE_REFUSAL_BLOCK_TURNS} standard \
                    turns so the route must change";
                   from);
        }
    }

    /// Whether `live-move-refusal-break` currently bars a step for this unit.
    /// `AdvancedAi` reads it to retire a frozen Settler's destination through
    /// the dead-site machinery as well, rather than only bending its route.
    pub(crate) fn move_refusal_blocked(&self, g: &Game, uid: u32) -> bool {
        self.move_refusal_break
            && self
                .move_refusal_blocks
                .get(&uid)
                .is_some_and(|(_, until)| g.turn < *until)
    }

    pub(crate) fn unit_plan_state(&self, uid: u32) -> BasicUnitPlanState {
        BasicUnitPlanState {
            recovering: self.recovering_units.contains(&uid),
            patrol_target: self.patrol_targets.get(&uid).copied(),
            settler_target: self.settler_targets.get(&uid).copied(),
            memory: self.unit_memory(uid),
            last_path_step: self.last_path_step_from.borrow().get(&uid).cloned(),
            patrol_posts: self.patrol_posts.clone(),
        }
    }

    pub(crate) fn merge_unit_plan_state(&mut self, uid: u32, state: BasicUnitPlanState) {
        if state.recovering {
            self.recovering_units.insert(uid);
        } else {
            self.recovering_units.remove(&uid);
        }
        match state.patrol_target {
            Some(target) => {
                self.patrol_targets.insert(uid, target);
            }
            None => {
                self.patrol_targets.remove(&uid);
            }
        }
        match state.settler_target {
            Some(target) => {
                self.settler_targets.insert(uid, target);
            }
            None => {
                self.settler_targets.remove(&uid);
            }
        }
        match state.memory {
            Some(memory) => {
                self.unit_memories.get_mut().insert(uid, memory);
            }
            None => {
                self.unit_memories.get_mut().remove(&uid);
            }
        }
        match state.last_path_step {
            Some(step) => {
                self.last_path_step_from.borrow_mut().insert(uid, step);
            }
            None => {
                self.last_path_step_from.borrow_mut().remove(&uid);
            }
        }
        // Posts are immutable for the turn. A branch may have paid to build a
        // domain's list, so retain that work for later serial fallbacks.
        self.patrol_posts.extend(state.patrol_posts);
    }

    pub(crate) fn clear_prepared_patrol_posts(&mut self) {
        self.patrol_posts_by_class.clear();
    }

    /// Record this turn's starting tile for every unit, and judge each unit
    /// against the window that has just closed. This is the only point in the
    /// turn where a livelock can be seen at all: it is a property of a unit's
    /// history, not of any decision the unit is about to make.
    fn observe_unit_motion(&mut self, g: &Game, pid: usize) {
        let ids = g.player_unit_ids(pid);
        self.unit_motion
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        for uid in ids {
            let mark = work_mark(g, uid);
            let pos = g.units[&uid].pos;
            // `own-column-is-not-a-refusal`: a turn on which no move of this
            // unit reached the host is not a lap of any circle -- the unit
            // stood where the bridge left it. It neither counts as fruitless
            // nor enters the window.
            let inert = self.own_column_is_not_a_refusal
                && self.host_move_feedback_active
                && !g
                    .turn
                    .checked_sub(1)
                    .is_some_and(|last| self.host_received_a_move(uid, last));
            let (was_looping, looping, fruitless, footprint, stand_down) = {
                let motion = self.unit_motion.entry(uid).or_default();
                if self.live_motion_turn_accounting && motion.observed_turn == Some(g.turn) {
                    continue;
                }
                let observed_turn = self.live_motion_turn_accounting.then_some(g.turn);
                motion.observed_turn = observed_turn;
                let was_looping = motion.looping;
                if motion.tiles.is_empty() {
                    motion.work = mark;
                }
                if motion.work != mark {
                    // The unit achieved something, so whatever it was doing was
                    // worth doing. Judge it from here rather than against a
                    // history that has just been made irrelevant.
                    *motion = UnitMotion {
                        work: mark,
                        resume_turn: motion.resume_turn,
                        observed_turn,
                        ..UnitMotion::default()
                    };
                } else if !inert {
                    motion.fruitless += 1;
                }
                if !inert {
                    motion.tiles.push_back(pos);
                }
                while motion.tiles.len() > LIVELOCK_WINDOW {
                    motion.tiles.pop_front();
                }
                motion.looping = motion.circling();
                let looping = motion.looping;
                let fruitless = motion.fruitless;
                let footprint = motion.footprint();
                let stand_down = looping && fruitless >= LIVELOCK_STAND_DOWN_AFTER;
                if stand_down {
                    // The tabu has had a full second window to redirect this unit
                    // and has not. Stop paying for the same fruitless search, hold
                    // the ground, and come back to the problem with a clean slate
                    // once the world around it has changed.
                    *motion = UnitMotion {
                        work: mark,
                        resume_turn: g.turn + LIVELOCK_STAND_DOWN_TURNS,
                        observed_turn,
                        ..UnitMotion::default()
                    };
                }
                (was_looping, looping, fruitless, footprint, stand_down)
            };
            let retired_goal = (looping && !was_looping && self.explore_dead_targets)
                .then(|| self.explore_goal.borrow().get(&uid).map(|(goal, _)| *goal))
                .flatten();
            if let Some(goal) = retired_goal {
                // A live Scout can move enough to evade the no-progress probe
                // while still returning to the same blocked frontier approach.
                // The loop proves the held goal is not producing discovery, so
                // retire it now and let this turn choose a fresh outward route.
                self.retire_exploration_target(g, uid, goal);
            }
            // Say it once when the loop is first recognized, and once more if
            // it outlasts every attempt to steer out of it.
            let kind = g.units[&uid].kind.as_str();
            if stand_down {
                think!(self.journal, Military, Decision,
                       "{kind} {uid} stands down; it is going nowhere";
                       "{fruitless} turns inside {footprint} tiles with nothing to show for \
                        them, and steering it out did not work; holding for \
                        {LIVELOCK_STAND_DOWN_TURNS} turns";
                       pos);
            } else if let Some(goal) = retired_goal {
                think!(self.journal, Military, Detail,
                       "{kind} {uid} abandons looped reconnaissance goal {goal:?}";
                       "{fruitless} turns inside {footprint} tiles found no new ground; \
                        that target and its ring are retired for \
                        {EXPLORE_DEAD_TARGET_TURNS} turns";
                       pos);
            } else if looping && !was_looping {
                think!(self.journal, Military, Detail,
                       "{kind} {uid} is walking in circles";
                       "{fruitless} turns inside {footprint} tiles with nothing to show for \
                        them; anywhere outside them is now worth \
                        {LIVELOCK_ESCAPE_VALUE:.0} more";
                       pos);
            }
        }
    }

    /// Dig in a stood-down unit that took its whole turn and found nothing to
    /// do. This runs *after* the unit's own step, never instead of it: an
    /// earlier version pre-empted the turn and guessed at what the unit might
    /// have wanted, which cost more productive turns than the loops it broke.
    /// A unit that acted needs nothing from this; one that did not is standing
    /// in the open regardless, and is better off fortified and healing.
    /// A unit whose planner declined to give it anything to do.
    ///
    /// ⚠ Until 2026-08-11 this fortified **only inside a stand-down window**,
    /// so a unit that simply took no turn stood in the open instead. `audit` at
    /// the deployment shape measures what that is worth: of major-civ
    /// unit-turns, **7.73% are an unembarked land military unit standing still,
    /// unfortified, that could have fortified** — 10,477 unit-turns across
    /// eight games — against **3.59%** that are actually fortified. The agent
    /// leaves its army unfortified more than twice as often as it fortifies it.
    ///
    /// The price is exact rather than rhetorical: `Game::unit_strength` adds
    /// **3.0 per fortified turn, capped at two**, so each of those unit-turns
    /// declines **+6 defensive strength**, about 30% of a warrior's base.
    ///
    /// Nothing is given up by taking it. The planner has already declined to
    /// use this unit's movement this turn, so the `moves_left = 0` that
    /// `do_fortify` sets costs a move that was not going to be made, and the
    /// stance clears the moment the unit moves again.
    pub(crate) fn hold_stood_down_unit(&self, g: &mut Game, pid: usize, uid: u32) {
        let standing_down = self
            .unit_motion
            .get(&uid)
            .is_some_and(|motion| g.turn < motion.resume_turn);
        if standing_down && g.units.contains_key(&uid) {
            self.fortify_or_stop(g, pid, uid);
        }
    }

    /// Whether the live bridge has observed this unit circling without work.
    ///
    /// A normal tactical move must still respect the loop tabu: otherwise a
    /// one-hex dead end can consume the whole war.  The coordinated mover may
    /// make one exception for an A* route after this becomes true, because the
    /// route can need to cross an already-visited square before it exits the
    /// pocket.  The exception is independent of the live movement recorder:
    /// the motion ledger is also populated by the ordinary turn-based path,
    /// and the route escape still applies its own legality and danger checks.
    pub(crate) fn live_livelock_route_escape(&self, uid: u32) -> bool {
        self.unit_motion
            .get(&uid)
            .is_some_and(|motion| motion.looping)
    }

    /// Whether a plain pathing step should be refused because it walks back
    /// into a footprint this unit has already exhausted. Unlike the tactical
    /// scorers there is nothing to trade off here — the route is chosen for
    /// progress alone — so the tabu is absolute while it lasts, and it lasts
    /// only until the window slides off the loop or the stand-down fires.
    fn retreads_a_loop(&self, uid: u32, to: Pos) -> bool {
        self.unit_motion
            .get(&uid)
            .is_some_and(|motion| motion.looping && motion.tiles.contains(&to))
    }

    pub(crate) fn corporations(&self, g: &mut Game, pid: usize) {
        if let Some(action) = g
            .legal_actions_within(pid, ActionFamilies::CORPORATIONS)
            .into_iter()
            .find(|action| matches!(action, Action::FoundCorporation { .. }))
        {
            let _ = g.apply(pid, &action);
        }
    }

    /// Resolve mandatory conquest choices with explicit strategic tradeoffs.
    /// Capitals and developed bridgeheads are retained; diplomacy-oriented
    /// plans restore city-states, friends, and eliminated founders; only an
    /// aggressive plan razes a remote city whose long-run value is negligible.
    pub(crate) fn resolve_city_dispositions(
        &mut self,
        g: &mut Game,
        pid: usize,
        prefer_diplomacy: bool,
        prefer_conquest: bool,
    ) {
        loop {
            let legal = g.legal_city_disposition_actions(pid);
            let Some(cid) = legal.iter().find_map(|action| match action {
                Action::KeepCity { city }
                | Action::RazeCity { city }
                | Action::LiberateCity { city } => Some(*city),
                _ => None,
            }) else {
                break;
            };
            let city = g.cities[&cid].clone();
            let founder = city.original_owner;
            let can_liberate = legal
                .iter()
                .any(|action| matches!(action, Action::LiberateCity { city } if *city == cid));
            let diplomatic_liberation = can_liberate
                && (prefer_diplomacy
                    || g.active_emergencies.iter().any(|emergency| {
                        emergency.city == cid && emergency.members.contains(&pid)
                    })
                    || g.players[founder].is_minor
                    || !g.players[founder].alive
                    || g.are_friends(pid, founder)
                    || g.alliance_with(pid, founder).is_some());

            let nearest_core = g
                .cities
                .values()
                .filter(|other| other.owner == pid && other.id != cid)
                .map(|other| g.wdist(city.pos, other.pos))
                .min()
                .unwrap_or(i32::MAX);
            let durable_value = city.is_capital
                || city.pop >= 4
                || !city.districts.is_empty()
                || !city.wonders.is_empty()
                || nearest_core <= 8;
            let can_raze = legal
                .iter()
                .any(|action| matches!(action, Action::RazeCity { city } if *city == cid));
            let action = if self.minor && can_raze {
                Action::RazeCity { city: cid }
            } else if diplomatic_liberation {
                Action::LiberateCity { city: cid }
            } else if can_raze && prefer_conquest && !durable_value {
                Action::RazeCity { city: cid }
            } else {
                Action::KeepCity { city: cid }
            };
            if g.apply(pid, &action).is_err() {
                break;
            }
        }
    }

    fn research(&self, g: &mut Game, pid: usize) {
        self.research_with_government(g, pid, true, None);
    }

    /// City-states research enough to defend and express their type without
    /// inheriting the major civilization's ancillary government/religion pass.
    /// Lower difficulties reach Masonry first; Immortal and Deity already have
    /// setup-granted Walls and can move directly toward their specialty.
    fn minor_research(&self, g: &mut Game, pid: usize) {
        if g.players[pid].research.is_none() {
            let avail = Self::era_window_techs(g, pid);
            if !avail.is_empty() {
                let has_walls = g.player_city_ids(pid).into_iter().any(|city| {
                    g.cities[&city]
                        .buildings
                        .iter()
                        .any(|building| building == "walls")
                });
                let defensive_goal = (!has_walls
                    && !g.players[pid].techs.contains(&crate::name!("masonry")))
                .then_some("masonry");
                let pick = Self::research_step_toward(g, &avail, defensive_goal)
                    .or_else(|| {
                        Self::research_step_toward(g, &avail, Self::minor_tech_goal(g, pid))
                    })
                    .or_else(|| {
                        Self::research_step_toward(g, &avail, Self::economic_research_goal(g, pid))
                    })
                    .or_else(|| {
                        TECH_PRIORITY
                            .iter()
                            .find(|tech| avail.iter().any(|candidate| candidate == *tech))
                            .map(|tech| Name::new(tech))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(
                    pid,
                    &Action::Research {
                        tech: Name::new(&pick),
                    },
                );
            }
        }
        if g.players[pid].civic.is_none() {
            let avail = g.available_civics(pid);
            if !avail.is_empty() {
                let cultural_goal = (g.cs_type(&g.players[pid].civ) == "cultural"
                    && !g.players[pid]
                        .civics
                        .contains(&crate::name!("drama_poetry")))
                .then_some("drama_poetry");
                let pick = Self::civic_step_toward(g, &avail, cultural_goal)
                    .or_else(|| {
                        CIVIC_PRIORITY
                            .iter()
                            .find(|civic| avail.iter().any(|candidate| candidate == *civic))
                            .map(|civic| Name::new(civic))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(
                    pid,
                    &Action::Civic {
                        civic: Name::new(&pick),
                    },
                );
            }
        }
    }

    /// Choose research and run the ancillary pass without allowing the
    /// baseline government priority to compete with a strategic caller.
    /// `AdvancedAi` has its own plan-aware government selector later in the
    /// same turn; running both selectors made it adopt two Tier-1 governments
    /// back-to-back and then pay Anarchy when the baseline tried to undo the
    /// strategic choice on the next turn. A headless caller may also pass its
    /// persistent pool for the independent live-policy counterfactuals;
    /// interactive and baseline controllers pass `None`.
    pub(crate) fn research_without_government_with_pool(
        &self,
        g: &mut Game,
        pid: usize,
        pool: Option<&WorkPool>,
    ) {
        self.research_with_government(g, pid, false, pool);
    }

    /// Name a production item the way an observer's reasoning log says it.
    /// The wire carries rule keys everywhere else; this is the one place a
    /// person reads them, so a Corps is a Corps and a district says where it
    /// is going.
    pub(crate) fn item_label(item: &Item) -> String {
        match item {
            Item::Unit { unit } => plain(unit),
            Item::Formation { unit, formation } => match formation {
                2 => format!("an Army of {}", plain(unit)),
                _ => format!("a Corps of {}", plain(unit)),
            },
            Item::Building { building } => plain(building),
            Item::District { district, pos } => {
                format!("a {} district at {pos:?}", plain(district))
            }
            Item::Wonder { wonder, pos } => format!("the wonder {} at {pos:?}", plain(wonder)),
            Item::Repair { repair, pos } => format!("repairs to the {} at {pos:?}", plain(repair)),
            Item::Project { project } => format!("the {} project", plain(project)),
            Item::Product { product } => format!("the product {}", plain(product)),
        }
    }

    /// Where on the map a production item is going, when it is going
    /// somewhere in particular rather than into the city itself.
    pub(crate) fn item_focus(item: &Item, city: Pos) -> Pos {
        match item {
            Item::District { pos, .. } | Item::Wonder { pos, .. } | Item::Repair { pos, .. } => {
                *pos
            }
            _ => city,
        }
    }

    /// A settler that has not moved in this many consecutive turns is not
    /// walking anywhere. Same threshold the Civ VI mod already uses for the
    /// same decision (`STRANDED_SETTLER_TURNS` in `CivvisControlAgent.lua`),
    /// so the two halves of the live bridge cannot disagree about which
    /// settlers count.
    const STRANDED_SETTLER_TURNS: u32 = 12;

    /// A Settler that has stood on one tile this many turns is parked for the
    /// purpose of `settler-backlog-brake`: half the stranded threshold, so the
    /// brake closes the queue before `stranded-settler-discount` opens it.
    /// The King runs of 2026-08-27..29 started 21 Settlers behind one parked
    /// ≥5 turns (16 behind one parked ≥12); only 7 of the 21 founded by t150.
    pub(crate) const SETTLER_BACKLOG_IDLE_TURNS: u32 = 6;

    /// The brake never touches the opening: below this many cities the first
    /// Settlers keep their pace whatever a walker is doing.
    pub(crate) const SETTLER_BACKLOG_MIN_CITIES: usize = 3;

    /// V2 waits until the empire has a useful economic base before a parked
    /// civilian may end the settlement wave. In the live King evidence behind
    /// V1, the two low-city runs ended on five and six cities after contributing
    /// six starts behind parked Settlers; the other concentrated-incident runs
    /// ended with nine to fourteen cities. Six keeps the vulnerable growth wave
    /// open while retaining the mature-empire brake.
    pub(crate) const SETTLER_BACKLOG_V2_MIN_CITIES: usize = 6;

    fn cities(&mut self, g: &mut Game, pid: usize) {
        self.refresh_settler_idle(g, pid);
        let mut settlers: usize = 0;
        let mut builders = 0;
        let mut traders = 0;
        let mut siege_support = 0;
        let mut melee = 0;
        let mut ranged = 0;
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        // The standing-army target measures fighting power, not unit records.
        // Counting heads let a Warrior that survived to the Industrial era
        // occupy a whole city's military allowance, so the empire stopped
        // training anything and its army aged in place. Each unit instead
        // counts as the fraction of a current front-line unit it can field.
        let front_line = Self::front_line_strength(g, pid, &city_ids);
        let mut force = 0.0;
        for uid in g.player_unit_ids(pid) {
            let kind = g.units[&uid].kind;
            match kind.as_str() {
                "settler" => settlers += 1,
                "builder" => builders += 1,
                "trader" => traders += 1,
                "battering_ram" | "siege_tower" => siege_support += 1,
                _ => {
                    let spec = &g.rules.units[kind];
                    if spec.class == "military" {
                        force += Self::force_weight(g, &kind, front_line);
                        if spec.is_melee_capable() {
                            melee += 1;
                        }
                        if spec.has_ranged_attack() {
                            ranged += 1;
                        }
                    }
                }
            }
        }
        let active_settlers = settlers;
        if builders > 0 {
            self.opening_builder_paid = true;
        }
        // Treat queued units as part of the force plan. Without this, every
        // occupied city forgets what it is already building and the next
        // empty city can queue a duplicate settler, builder, or trader.
        for cid in &city_ids {
            if let Some(Item::Unit { unit }) = g.cities[cid].queue.first() {
                match unit.as_str() {
                    "settler" => settlers += 1,
                    "builder" => builders += 1,
                    "trader" => traders += 1,
                    "battering_ram" | "siege_tower" => siege_support += 1,
                    _ => {
                        let spec = &g.rules.units[unit];
                        if spec.class == "military" {
                            force += Self::force_weight(g, unit, front_line);
                            if spec.is_melee_capable() {
                                melee += 1;
                            }
                            if spec.has_ranged_attack() {
                                ranged += 1;
                            }
                        }
                    }
                }
            }
        }
        let mut military = force.round() as usize;
        // Settlement races can invalidate the final site after a Settler was
        // queued but before it finishes. Revalidate the queue every turn and
        // bank its progress behind a useful replacement instead of completing
        // a civilian that can never found a city.
        let practical_settle_site = self.has_practical_settle_site(g, pid);
        if settlers > active_settlers || (settlers > 0 && !practical_settle_site) {
            // A queued Settler is kept while the pipeline `pick_item` would
            // open for it still has room. Keeping only the first one when no
            // walker existed replaced every second Settler the pipeline had
            // just authorised: on the live King seat the capital alternated
            // Settler and Monument orders every turn (civvis-20261001T000033Z
            // t27-t30; eight such swaps by t51 in every game of 2026-09-30),
            // each replacement asking `pick_item` with the walker counted as
            // the one Settler in flight.
            let width = self.settler_pipeline_width(g, n_cities, settlers);
            let mut committed_settlers =
                active_settlers.saturating_sub(self.stranded_settlers(g, pid));
            for cid in &city_ids {
                if !matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Unit { unit }) if unit == "settler"
                ) {
                    continue;
                }
                if committed_settlers < width.max(1) && practical_settle_site {
                    committed_settlers += 1;
                    continue;
                }
                let replacement = self.pick_item(
                    g,
                    pid,
                    *cid,
                    n_cities,
                    settlers.saturating_sub(1),
                    builders,
                    traders,
                    siege_support,
                    military,
                    melee,
                    ranged,
                );
                let Some(item) = replacement else {
                    committed_settlers += 1;
                    continue;
                };
                if g.apply(
                    pid,
                    &Action::Produce {
                        city: *cid,
                        item: item.clone(),
                    },
                )
                .is_err()
                {
                    committed_settlers += 1;
                    continue;
                }
                settlers = settlers.saturating_sub(1);
                match &item {
                    Item::Unit { unit } if unit == "builder" => builders += 1,
                    Item::Unit { unit } if unit == "trader" => traders += 1,
                    Item::Unit { unit } if unit == "battering_ram" || unit == "siege_tower" => {
                        siege_support += 1
                    }
                    Item::Unit { unit } => {
                        let spec = &g.rules.units[unit];
                        if spec.class == "military" {
                            military += 1;
                            if spec.is_melee_capable() {
                                melee += 1;
                            }
                            if spec.has_ranged_attack() {
                                ranged += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        // Walls fire at raiders in range. Encampment strikes are collected
        // once after the city loop: rebuilding the complete action list for
        // every city also enumerates production, deals, Congress votes, and
        // every unit move, which becomes quadratic in a developed empire.
        for cid in &city_ids {
            if g.city_can_strike(&g.cities[cid]) {
                let cpos = g.cities[cid].pos;
                for pos in g.wdisk(cpos, 2) {
                    let hit = g.unit_ids_at(pos).iter().any(|oid| {
                        let o = &g.units[oid];
                        o.owner != pid && g.is_at_war(pid, o.owner)
                    });
                    if hit {
                        let _ = g.apply(
                            pid,
                            &Action::CityStrike {
                                city: *cid,
                                target: pos,
                            },
                        );
                        break;
                    }
                }
            }
        }
        loop {
            if !city_ids.iter().any(|cid| {
                g.defending_districts(&g.cities[cid])
                    .any(|district| g.defending_district_can_strike(&g.cities[cid], &district))
            }) {
                break;
            }
            let strike = g
                .legal_actions_within(pid, ActionFamilies::CORE)
                .into_iter()
                .find(|action| {
                    matches!(
                        action,
                        Action::EncampmentStrike { .. } | Action::DistrictStrike { .. }
                    )
                });
            let Some(strike) = strike else {
                break;
            };
            if g.apply(pid, &strike).is_err() {
                break;
            }
        }
        for cid in &city_ids {
            // The first opening defender is allowed to preempt a Settler or
            // Builder already in the capital's queue. This runs before the
            // pending-book Settler as well: a held opening cannot jump ahead
            // of the same local threat it was waiting to survive.
            if let Some(item) = self.opening_barbarian_defense_item(g, pid, *cid) {
                let displaced = g.cities[cid].queue.first().cloned();
                let displaces_settler =
                    matches!(&displaced, Some(Item::Unit { unit }) if unit == "settler");
                let live = pid == crate::game::MIRRORED_SEAT && !g.host_observed.is_empty();
                if live
                    && displaces_settler
                    && self.opening_defense_preempts >= self.opening_defense_settler_preempts()
                {
                    think!(self.journal, Military, Detail,
                           "{} keeps its Settler despite nearby barbarians", g.cities[cid].name;
                           "the opening has already given the Settler's queue to {} defenders \
                            and the board draws them off; the city keeps building",
                           self.opening_defense_preempts);
                } else if g
                    .apply(
                        pid,
                        &Action::Produce {
                            city: *cid,
                            item: item.clone(),
                        },
                    )
                    .is_ok()
                {
                    if live && displaces_settler {
                        self.opening_defense_preempts += 1;
                    }
                    if self.journal.wants(crate::reasoning::Level::Decision) {
                        let city = &g.cities[cid];
                        think!(self.journal, Military, Decision,
                               "{} delays its opening for nearby barbarian pressure", city.name;
                               "{} takes the queue before {}; a civilian-safe route is already blocked",
                               Self::item_label(&item),
                               displaced
                                   .as_ref()
                                   .map(Self::item_label)
                                   .unwrap_or_else(|| "the next opening item".to_string()));
                    }
                    if let Item::Unit { unit } = &item {
                        let spec = &g.rules.units[unit];
                        if spec.class == "military" {
                            military += 1;
                            if spec.is_melee_capable() {
                                melee += 1;
                            }
                            if spec.has_ranged_attack() {
                                ranged += 1;
                            }
                        }
                    }
                    continue;
                }
            }
            // The held opening-book Settler takes the capital's queue the turn
            // the city reaches the host's floor. See `opening_settler_waits`.
            if self.book_settler_pending
                && g.cities[cid].is_capital
                && self.play_pending_book_settler(g, pid)
            {
                settlers += 1;
                continue;
            }
            if !g.cities[cid].queue.is_empty() {
                continue;
            }
            // Reserve the first slot before population-two Settler genes can
            // claim it. Successful production consumes exactly one book slot.
            if self.play_slinger_first_opening(g, pid, *cid) {
                military += 1;
                ranged += 1;
                continue;
            }
            if self.play_scout_first_opening(g, pid, *cid) {
                continue;
            }
            // `opening-builder-after-the-first-settler`: the capital's first
            // Builder follows its first Settler. See `opening_builder.rs`.
            if self.play_opening_builder(g, pid, *cid, n_cities, active_settlers, builders) {
                builders += 1;
                continue;
            }
            // The normal production picker sits after the scripted opening
            // book. Let `capital-settler-after-completion` reserve the first
            // newly empty capital queue once it reaches population two,
            // rather than letting that book immediately fill it again. The
            // picker remains the authority on whether a Settler is presently
            // safe and useful, and the untouched book resumes after this
            // one production decision.
            if (self.capital_settler_after_completion || self.rapid_city_expansion_2)
                && !self.minor
                && !self.barb
                && g.cities[cid].is_capital
                && g.cities[cid].pop >= 2
                && self.book_pos < 4
            {
                let opening_item = self.pick_item(
                    g,
                    pid,
                    *cid,
                    n_cities,
                    settlers,
                    builders,
                    traders,
                    siege_support,
                    military,
                    melee,
                    ranged,
                );
                if matches!(
                    &opening_item,
                    Some(Item::Unit { unit }) if unit == "settler"
                ) {
                    let item = opening_item.expect("the matched opening item is a Settler");
                    if g.apply(pid, &Action::Produce { city: *cid, item }).is_ok() {
                        settlers += 1;
                        think!(self.journal, Cities, Decision,
                               "{} starts a Settler after completing its prior production", g.cities[cid].name;
                               "the capital is population two with an empty queue; the capital-settler-after-completion gene reserves this next build while preserving the rest of the opening book");
                        continue;
                    }
                }
            }
            // The live land-grab policy already opens two Settler seats from
            // the first city, but the opening book used to spend its next
            // Builder/Warrior slot before `pick_item` could fill that second
            // seat. On an Online-speed host that left city three walking out
            // of Rome at turn 29 and missing the turn-32 opening gate. Ask
            // the ordinary governor first while the first Settler is already
            // on the board: it retains every population, local-barbarian,
            // repair, and military-floor refusal, and leaves the book intact
            // whenever a Settler is not the safe choice.
            if self.land_grab
                && !self.minor
                && !self.barb
                && n_cities == 1
                && settlers > 0
                && g.cities[cid].is_capital
                && self.book_pos < 4
            {
                let opening_major_war = g.players.iter().any(|player| {
                    player.id != pid
                        && player.alive
                        && !player.is_minor
                        && !player.is_barbarian
                        && g.is_at_war(pid, player.id)
                });
                if !opening_major_war {
                    let opening_item = self.pick_item(
                        g,
                        pid,
                        *cid,
                        n_cities,
                        settlers,
                        builders,
                        traders,
                        siege_support,
                        military,
                        melee,
                        ranged,
                    );
                    if matches!(
                        &opening_item,
                        Some(Item::Unit { unit }) if unit == "settler"
                    ) {
                        let item = opening_item.expect("the matched opening item is a Settler");
                        if g.apply(pid, &Action::Produce { city: *cid, item }).is_ok() {
                            settlers += 1;
                            think!(self.journal, Cities, Decision,
                                   "{} keeps the opening Settler pipeline filled", g.cities[cid].name;
                                   "one Settler is already walking, the peaceful land-grab policy has a second open seat, and the remaining opening book waits");
                            continue;
                        }
                    }
                }
            }
            // chess-style opening book: scripted first capital builds
            if !self.minor && !self.barb && g.cities[cid].is_capital && self.book_pos < 4 {
                let mut played = false;
                while self.book_pos < 4 && !played {
                    // The live recon repair used to wake only after all four
                    // scripted builds. That made "replace our eyes when they
                    // are gone" powerless in the more important zero-scout
                    // opening. Use the repair's bounded predicate for the
                    // first slot, replacing one opener rather than adding a
                    // fifth build. Frozen controllers retain their genes.
                    let missing_opening_recon =
                        self.book_pos == 0 && self.recon_is_the_missing_arm(g, pid);
                    let gene = if missing_opening_recon {
                        0.0 // OPENING_MENU[0] is Scout.
                    } else {
                        [self.w.open0, self.w.open1, self.w.open2, self.w.open3][self.book_pos]
                    };
                    self.book_pos += 1;
                    let i = gene.max(0.0) as usize;
                    if i >= OPENING_MENU.len() {
                        continue; // "pass" gene: fall back to evaluation
                    }
                    let name = OPENING_MENU[i];
                    if name == "settler" && !self.has_practical_settle_site(g, pid) {
                        continue;
                    }
                    // ★★★★ THE SLOT IS HELD, NOT BURNED, WHEN THE CAPITAL IS
                    // STILL BELOW THE HOST'S FLOOR. See `opening_settler_waits`:
                    // the next slot plays now, and the Settler takes the queue
                    // the turn the city grows.
                    if name == "settler"
                        && self.opening_settler_waits
                        && (g.cities[cid].pop as f64) < HOST_SETTLER_MIN_POP
                    {
                        self.book_settler_pending = true;
                        think!(self.journal, Cities, Detail,
                               "{} holds the opening book's settler", g.cities[cid].name;
                               "the capital is population {} and the host asks {} of a Settler; \
                                the next opener plays now and the Settler takes the queue the \
                                turn the city grows",
                               g.cities[cid].pop, HOST_SETTLER_MIN_POP as u32);
                        continue;
                    }
                    let item = if name == "monument" {
                        Item::Building {
                            building: Name::new(name),
                        }
                    } else {
                        Item::Unit {
                            unit: Name::new(name),
                        }
                    };
                    if g.apply(
                        pid,
                        &Action::Produce {
                            city: *cid,
                            item: item.clone(),
                        },
                    )
                    .is_ok()
                    {
                        match &item {
                            Item::Unit { unit } if unit == "settler" => settlers += 1,
                            Item::Unit { unit } if unit == "builder" => builders += 1,
                            Item::Unit { unit } if unit == "trader" => traders += 1,
                            Item::Unit { unit }
                                if unit == "battering_ram" || unit == "siege_tower" =>
                            {
                                siege_support += 1
                            }
                            Item::Unit { unit } => {
                                let spec = &g.rules.units[unit];
                                if spec.class == "military" {
                                    military += 1;
                                    if spec.is_melee_capable() {
                                        melee += 1;
                                    }
                                    if spec.has_ranged_attack() {
                                        ranged += 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                        played = true;
                    }
                }
                if played {
                    continue;
                }
            }
            if let Some(item) = self.pick_item(
                g,
                pid,
                *cid,
                n_cities,
                settlers,
                builders,
                traders,
                siege_support,
                military,
                melee,
                ranged,
            ) {
                // Diagnostic only, no order reads it: why this pick is not a
                // Settler while the empire has room. See `journal_settler_hold`.
                self.journal_settler_hold(g, pid, *cid, n_cities, settlers, &item);
                if g.apply(
                    pid,
                    &Action::Produce {
                        city: *cid,
                        item: item.clone(),
                    },
                )
                .is_ok()
                {
                    if self.journal.wants(crate::reasoning::Level::Decision) {
                        let cost = g.item_cost_for_city(pid, *cid, &item);
                        let per_turn = g.city_yields(*cid).production.max(0.1);
                        let city = &g.cities[cid];
                        let turns = (cost / per_turn).ceil().max(1.0);
                        think!(self.journal, Cities, Decision,
                               "{} starts {}", city.name, Self::item_label(&item);
                               "{cost:.0} production, about {turns:.0} turn{} at \
                                {per_turn:.1} a turn; the empire holds {military} military \
                                for {n_cities} {} against a target of {:.1} each, and \
                                {settlers} settler{}, {builders} builder{}, {traders} trader{}",
                               if turns == 1.0 { "" } else { "s" },
                               if n_cities == 1 { "city" } else { "cities" },
                               self.w.mil_per_city,
                               if settlers == 1 { "" } else { "s" },
                               if builders == 1 { "" } else { "s" },
                               if traders == 1 { "" } else { "s" };
                               Self::item_focus(&item, city.pos));
                    }
                    match &item {
                        Item::Unit { unit } if unit == "settler" => settlers += 1,
                        Item::Unit { unit } if unit == "builder" => builders += 1,
                        Item::Unit { unit } if unit == "trader" => traders += 1,
                        Item::Unit { unit } if unit == "battering_ram" || unit == "siege_tower" => {
                            siege_support += 1
                        }
                        Item::Unit { unit } => {
                            let spec = &g.rules.units[unit];
                            if spec.class == "military" {
                                military += 1;
                                if spec.is_melee_capable() {
                                    melee += 1;
                                }
                                if spec.has_ranged_attack() {
                                    ranged += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.spend_gold(
            g, pid, &city_ids, settlers, builders, traders, military, melee, ranged,
        );
        // ★★★★ ASK THE ENGINE WHAT IT COSTS. `faith_builder` is a weight, not
        // a price: a Faith Builder is legal only under the Monumentality
        // dedication, so from the turn a civ first banks 120 Faith until the
        // game ends this fired every turn and was refused every turn — 540
        // refused orders in one 150-turn six-player game, third behind
        // `Fortify` and `ChooseSecretSociety`. `unit_purchase_cost` returns
        // `None` for every illegal purchase and the real speed-scaled,
        // discounted price otherwise, so it answers legality and
        // affordability in one question. The weight still gates: it is the
        // reserve the empire keeps back, above whatever the purchase costs.
        // ⚠⚠ THE THIRD SPELLING OF THE SAME RULE, AND THE ONE THAT DID NOT ASK.
        // `has_builder_work` gates the production path and the gold-purchase
        // path; this one counted cities and nothing else, so an empire with
        // every workable tile already improved went on buying Builders with
        // Faith. `advanced.rs`'s `PRODUCTION_BUILDERS_PER_CITY` states in prose
        // that "the existing `has_builder_work` gate stops production once
        // there is no yield to add" — true of two paths out of three until now.
        //
        // Measured over three 250-turn six-player games before this: builders
        // reached a decision 4,887 times and found no improvable tile anywhere
        // in the empire on 4,377 of them, flat across the whole game rather
        // than concentrated late. Blocked-by-terrain was 43.
        if g.players[pid].faith >= self.w.faith_builder
            && builders < n_cities
            && Self::has_builder_work(g, pid)
            && !city_ids.is_empty()
            && g.unit_purchase_cost(pid, city_ids[0], "builder", "faith")
                .is_some_and(|cost| g.players[pid].faith + f64::EPSILON >= cost)
        {
            let _ = g.apply(
                pid,
                &Action::Buy {
                    city: city_ids[0],
                    unit: crate::name!("builder"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
            );
        }
        if self.pursue_religion
            && !self.bank_faith_for_apostle
            && g.players[pid].religion.is_some()
            && g.players[pid].faith >= 250.0
        {
            let religion = g.players[pid].religion.clone().unwrap();
            let missionaries = g
                .units
                .values()
                .filter(|u| u.owner == pid && u.kind == "missionary")
                .count();
            if missionaries < 2 {
                for cid in &city_ids {
                    // Firaxis gives a purchased religious unit the city's
                    // majority religion. A converted Holy Site would spend
                    // our Faith strengthening the rival faith; on a live
                    // mirror this also produced a purchase the host refused
                    // every turn once Rome converted. The live adapter keeps
                    // looking when the first matching city cannot place it.
                    // ⚠ AND THE TILE MUST BE FREE OF A RELIGIOUS UNIT. Civilization VI
                    // refuses a purchase that would place a second unit of the same
                    // class on one plot, and it says so in as many words — the
                    // `purchase_refused` instrument recorded the host's own text,
                    // "Too many units of the same class in this location.", on the
                    // missionary buy.
                    //
                    // That refusal is 799 of the 08-04/08-05 orders, and it was NOT
                    // affordability: faith balance at the moment of refusal ran a
                    // median 474 against a median quoted cost of 90. The buyer only
                    // ever counted missionaries EMPIRE-WIDE (`missionaries < 2`) and
                    // never asked whether the city it was buying into already had one
                    // standing on it — which is exactly where a just-purchased
                    // missionary is still sitting.
                    //
                    // ⚠⚠ GATED ON THE LIVE ADAPTER, like the religion check beside it.
                    // This function is the FROZEN controller's too, and an unconditional
                    // guard would change the legacy path — which means bumping
                    // `ELO_PROTOCOL_VERSION` and starting a new ledger for a bug that
                    // only bites the live bridge. The refusals are all live.
                    let center = g.cities[cid].pos;
                    let occupied = self.live_religious_purchase_guard
                        && g.unit_ids_at(center).iter().any(|uid| {
                            let unit = &g.units[uid];
                            unit.owner == pid
                                && g.rules
                                    .units
                                    .get(unit.kind.as_str())
                                    .is_some_and(|spec| spec.class == "religious")
                        });
                    if occupied
                        || !g.cities[cid]
                            .districts
                            .contains_key(crate::name!("holy_site"))
                        || (self.live_religious_purchase_guard
                            && g.city_religion(&g.cities[cid]) != Some(religion.as_str()))
                    {
                        continue;
                    }
                    let applied = g
                        .apply(
                            pid,
                            &Action::Buy {
                                city: *cid,
                                unit: crate::name!("missionary"),
                                formation: 0,
                                currency: "faith".to_string(),
                            },
                        )
                        .is_ok();
                    // Preserve the frozen controller's historical first-Holy-Site
                    // exit. Only the live adapter may continue after a refusal.
                    if !self.live_religious_purchase_guard || applied {
                        break;
                    }
                }
            }
        }
    }

    /// Modernize the standing army before it moves. An empire that never
    /// spends Gold here fights the Information era with Slingers: production
    /// only ever replaces losses, so the units already on the map are exactly
    /// the ones that fall behind. Upgrades are taken strongest-gain-first and
    /// stop at a treasury floor so the ordinary purchase passes still have
    /// something to spend.
    pub(crate) fn upgrade_units(g: &mut Game, pid: usize) {
        if g.players[pid].is_barbarian {
            return;
        }
        let at_war = g
            .players
            .iter()
            .any(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id));
        // Nearby barbarians do not create a diplomatic war, but a materially
        // stronger raider creates the same immediate need to spend. Holding
        // 120 Gold back in that state leaves a two-upgrade army fighting with
        // the obsolete unit it already owns; keep only the wartime emergency
        // reserve until the local quality gap closes.
        let barbarian_gap = Self::barbarian_military_gap(g, pid);
        let floor = if at_war || barbarian_gap { 30.0 } else { 120.0 };
        Self::modernize_army(g, pid, floor, 0.0);
        Self::use_opportunistic_unit_tools(g, pid);
    }

    /// The upgrade loop itself: strongest gain per Gold first, never
    /// spending below `floor`. `veteran_weight` is the extra value per
    /// promotion (capped at four) a unit carries into its successor — a
    /// promoted unit keeps its promotions, so the same Gold buys a better
    /// unit; 0.0 ranks by strength alone. Returns the upgrades taken.
    pub(crate) fn modernize_army(
        g: &mut Game,
        pid: usize,
        floor: f64,
        veteran_weight: f64,
    ) -> usize {
        Self::modernize_army_with_filter(g, pid, floor, veteran_weight, |_, _, _, _| true)
    }

    /// Keep the shared ranking and legal host quotes while a strategic caller
    /// reserves material for another already viable military capability.
    pub(crate) fn modernize_army_with_filter(
        g: &mut Game,
        pid: usize,
        floor: f64,
        veteran_weight: f64,
        allowed: impl Fn(&Game, u32, Name, f64) -> bool,
    ) -> usize {
        let mut taken = 0;
        loop {
            let mut best: Option<(f64, f64, u32)> = None;
            for uid in g.player_unit_ids(pid) {
                let Some((target, gold, resources)) = g.unit_gold_upgrade_offer(pid, uid) else {
                    continue;
                };
                if g.players[pid].gold - gold < floor || !allowed(g, uid, target, resources) {
                    continue;
                }
                let from = &g.rules.units[g.units[&uid].kind];
                let to = &g.rules.units[target];
                let gain = to.strength.max(to.ranged_attack_strength())
                    - from.strength.max(from.ranged_attack_strength());
                // Support and civilian successors carry no combat strength;
                // rank those by the Production they save instead.
                let gain = if gain > 0.0 {
                    gain
                } else {
                    (to.cost - from.cost).max(0.0) / 20.0
                };
                if gain <= 0.0 {
                    continue;
                }
                let promotions = g.units[&uid].promotions.len().min(4) as f64;
                let value = gain * (1.0 + veteran_weight * promotions) / gold.max(1.0);
                let better = match &best {
                    None => true,
                    Some((top, top_gold, top_uid)) => {
                        value > *top + 1e-9
                            || ((value - *top).abs() <= 1e-9
                                && (gold < *top_gold - 1e-9
                                    || ((gold - *top_gold).abs() <= 1e-9 && uid < *top_uid)))
                    }
                };
                if better {
                    best = Some((value, gold, uid));
                }
            }
            let Some((_, _, uid)) = best else { break };
            if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
                break;
            }
            taken += 1;
        }
        taken
    }

    /// Execute rare, unambiguously beneficial unit tools before ordinary
    /// movement consumes the acting unit's turn.
    ///
    /// `AdvancedAi` deliberately calls this shared pre-movement pass, so these
    /// actions are part of the default strategic agent as well as BasicAI.
    fn use_opportunistic_unit_tools(g: &mut Game, pid: usize) -> usize {
        let conversion_ready = g.barb_pid.is_some()
            && g.player_unit_ids(pid).into_iter().any(|unit| {
                let unit = &g.units[&unit];
                unit.kind == "apostle"
                    && unit.moves_left > 0.0
                    && unit.charges > 0
                    && unit.promotions.iter().any(|promotion| {
                        g.rules
                            .promotions
                            .get(promotion)
                            .and_then(|spec| spec.effects.get("convert_barbarians"))
                            .is_some_and(|value| *value > 0.0)
                    })
            });
        if !conversion_ready {
            return 0;
        }
        let conversions: Vec<Action> = g
            .legal_actions_within(pid, ActionFamilies::UNITS)
            .into_iter()
            .filter(|action| matches!(action, Action::ConvertBarbarians { .. }))
            .collect();
        conversions
            .into_iter()
            .filter(|action| g.apply(pid, action).is_ok())
            .count()
    }

    fn best_military(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        want_ranged: Option<bool>,
    ) -> Option<String> {
        let mut best: Option<(f64, String)> = None;
        for (name, spec) in &g.rules.units {
            if spec.class != "military" || spec.domain.as_deref() == Some("sea") {
                continue;
            }
            let matches_role = match want_ranged {
                Some(true) => spec.has_ranged_attack(),
                Some(false) => spec.is_melee_capable(),
                None => spec.has_ranged_attack() || spec.is_melee_capable(),
            };
            if !matches_role {
                continue;
            }
            if !g.can_produce(pid, cid, &Item::Unit { unit: *name }) {
                continue;
            }
            let mut power = spec.strength.max(spec.ranged_attack_strength());
            if self.unique_unit_preference && self.unique_unit_wanted(g, pid, *name, spec) {
                power += UNIQUE_UNIT_POWER_CREDIT;
            }
            if best.as_ref().map(|(b, _)| power > *b).unwrap_or(true) {
                best = Some((power, name.to_string()));
            }
        }
        best.map(|(_, n)| n)
    }

    /// `unique-unit-preference`: whether `best_military` should credit this
    /// design as our own unique unit. Only while it is under half of the
    /// land army in its melee-or-ranged role (fielded plus each city's
    /// current build): the unique unit leads the army, it does not become
    /// all of it — a Llanero column still wants Line Infantry beside it
    /// against Pike and Shot.
    fn unique_unit_wanted(
        &self,
        g: &Game,
        pid: usize,
        kind: Name,
        spec: &crate::rules::UnitSpec,
    ) -> bool {
        if spec.unique_to.as_deref() != Some(g.players[pid].civ.as_str()) {
            return false;
        }
        let ranged = spec.has_ranged_attack();
        let same_role = |unit: Name| {
            let other = &g.rules.units[unit];
            other.class == "military"
                && !matches!(other.domain.as_deref(), Some("sea" | "air"))
                && other.has_ranged_attack() == ranged
        };
        let mut role = 0usize;
        let mut unique = 0usize;
        let fielded = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .map(|unit| unit.kind);
        let building = g.player_city_ids(pid).into_iter().filter_map(|cid| {
            match g.cities[&cid].queue.first() {
                Some(Item::Unit { unit }) => Some(*unit),
                _ => None,
            }
        });
        for unit in fielded.chain(building).filter(|unit| same_role(*unit)) {
            role += 1;
            unique += usize::from(unit == kind);
        }
        unique * 2 < role.max(2)
    }

    /// Strength of the strongest land and naval unit this empire can train
    /// right now. Zero for a domain with nothing available, in which case
    /// every unit of that domain counts in full.
    pub(crate) fn front_line_strength(g: &Game, pid: usize, city_ids: &[u32]) -> (f64, f64) {
        let mut land = 0.0_f64;
        let mut sea = 0.0_f64;
        for cid in city_ids {
            for (name, spec) in &g.rules.units {
                if spec.class != "military" {
                    continue;
                }
                let naval = spec.domain.as_deref() == Some("sea");
                let power = spec.strength.max(spec.ranged_attack_strength());
                // Checking the strength first keeps the expensive production
                // test off every unit that could not raise the maximum anyway.
                if power <= if naval { sea } else { land } {
                    continue;
                }
                if !g.can_produce(pid, *cid, &Item::Unit { unit: *name }) {
                    continue;
                }
                if naval {
                    sea = power;
                } else {
                    land = power;
                }
            }
        }
        (land, sea)
    }

    /// How much of a modern unit this one still represents. A unit always
    /// counts for something — even a Warrior can hold a tile — but a garrison
    /// three eras behind no longer satisfies the empire's force target, which
    /// is what keeps late-game production and Gold flowing into better units.
    pub(crate) fn force_weight(g: &Game, kind: &str, front_line: (f64, f64)) -> f64 {
        let spec = &g.rules.units[kind];
        let best = if spec.domain.as_deref() == Some("sea") {
            front_line.1
        } else {
            front_line.0
        };
        if best <= 0.0 {
            return 1.0;
        }
        (spec.strength.max(spec.ranged_attack_strength()) / best).clamp(0.2, 1.0)
    }

    /// Destinations not already served by this empire that at least one of
    /// its cities can legally reach. Routes are unique by owner/destination,
    /// so each entry is one concrete job for one available or queued Trader.
    fn open_trade_destinations(g: &Game, pid: usize) -> usize {
        let origins = g.player_city_ids(pid);
        g.cities
            .keys()
            .filter(|destination| {
                origins
                    .iter()
                    .any(|origin| g.can_establish_trade_route(pid, *origin, **destination))
            })
            .count()
    }

    fn should_add_trader(g: &Game, pid: usize, traders: usize) -> bool {
        g.active_routes(pid) + (traders as i64) < g.trade_capacity(pid)
            && traders < Self::open_trade_destinations(g, pid)
    }

    #[cfg(test)]
    fn should_add_trader_safely(g: &Game, pid: usize, traders: usize) -> bool {
        Self::should_add_trader(g, pid, traders)
            && !Self::barbarian_trade_risk(g, pid)
            && g.player_city_ids(pid)
                .into_iter()
                .all(|cid| Self::barbarian_threat_pressure(g, pid, cid) == 0)
    }

    fn should_add_trader_safely_for_controller(
        &self,
        g: &Game,
        pid: usize,
        traders: usize,
    ) -> bool {
        Self::should_add_trader(g, pid, traders)
            && !self.barbarian_trade_risk_for_controller(g, pid)
            && g.player_city_ids(pid)
                .into_iter()
                .all(|cid| self.barbarian_threat_pressure_for_controller(g, pid, cid) == 0)
    }

    /// Whether this city can turn a completed Trader straight into a route
    /// without exposing it inside a live barbarian ring. Safety belongs to
    /// the origin, not to every city in the empire: native routes complete
    /// immediately there and the live bridge starts the same host action.
    #[cfg(test)]
    pub(crate) fn safe_trade_origin(g: &Game, pid: usize, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| city.owner == pid)
            && !Self::barbarian_local_alarm(g, pid, cid)
            && Self::barbarian_threat_pressure(g, pid, cid) == 0
            && g.cities
                .keys()
                .any(|destination| g.can_establish_trade_route(pid, cid, *destination))
    }

    pub(crate) fn safe_trade_origin_for_controller(&self, g: &Game, pid: usize, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| city.owner == pid)
            && !self.barbarian_local_alarm_for_controller(g, pid, cid)
            && self.barbarian_threat_pressure_for_controller(g, pid, cid) == 0
            && g.cities
                .keys()
                .any(|destination| g.can_establish_trade_route(pid, cid, *destination))
    }

    fn has_safe_trade_origin(&self, g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| self.safe_trade_origin_for_controller(g, pid, cid))
    }

    fn should_add_trader_for_controller(&self, g: &Game, pid: usize, traders: usize) -> bool {
        if self.barbarian_tactics {
            if self.solvency_first_trade_slot {
                Self::should_add_trader(g, pid, traders) && self.has_safe_trade_origin(g, pid)
            } else {
                self.should_add_trader_safely_for_controller(g, pid, traders)
            }
        } else {
            Self::should_add_trader(g, pid, traders)
        }
    }

    /// What a city with enemies already at its gates should build instead of
    /// whatever the ordinary build order wanted.
    ///
    /// ⚠ The empire-wide floor cannot reach this case, and the ladder proves
    /// it. Run `civvis-20260802T205959Z` (Sweden), at war with the Mapuche:
    /// enemy units stood within three tiles of Uppsala on every turn from t60
    /// to t67 — a swordsman and a heavy chariot at range 1, a catapult at 2 —
    /// while Uppsala took 0 -> 39 -> 148 damage and fell on t68. The empire
    /// held 7 military against a floor of `1.0 * 4 cities`, so the floor was
    /// satisfied and **the city under the catapult spent those four turns
    /// building a MONUMENT**. Karlstad, also besieged, built a Commercial Hub;
    /// Helsingborg built a Builder every turn; 172-214 gold went unspent.
    ///
    /// A count of units spread across an empire says nothing about whether
    /// THIS city can hold. The live bridge treats lost city health as the
    /// proof that the local defence must answer, even when fog hides the
    /// attackers, and asks for walls first, then a melee-capable land
    /// defender. A siege piece belongs at a distant enemy wall and cannot
    /// hold the city it is being built in.
    fn besieged_city_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let bleeding = self.garrison_under_fire
            && g.cities
                .get(&cid)
                .is_some_and(|city| city.hp < 200 || Self::walls_need_local_defense(g, pid, cid));
        if !bleeding {
            return None;
        }
        for building in ["walls", "medieval_walls", "renaissance_walls"] {
            let wall = Item::Building {
                building: Name::new(building),
            };
            if g.can_produce(pid, cid, &wall) {
                return Some(wall);
            }
        }
        // Once the city is already taking damage, a range-one Slinger is
        // not the emergency fallback. Walls retain priority, then a missing
        // Archer; before Archery, retain the melee rescue.
        if let Some(shooter) = self.early_local_shooter_item(g, pid, cid) {
            if matches!(&shooter, Item::Unit { unit } if g.rules.units[unit].range >= 2) {
                return Some(shooter);
            }
        }
        // This damage-only path is live-bridge-only, so choose the local land
        // defender rather than the highest-bombard siege unit.
        let defender = self.best_military(g, pid, cid, Some(false));
        defender.map(|unit| Item::Unit {
            unit: Name::new(&unit),
        })
    }

    /// The live opening needs a shooter at home even when its Warrior escorts
    /// a Settler. Prefer a range-two defender; before its unlock, train one
    /// Slinger to defend now and upgrade later. Existing local shooters and
    /// nearby shooter queues satisfy this bounded request.
    fn early_local_shooter_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.garrison_under_fire || !advanced::AdvancedAi::early_archers_window_open(g, pid) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let shooter = |spec: &crate::rules::UnitSpec| {
            spec.class == "military"
                && matches!(spec.domain.as_deref(), None | Some("land"))
                && spec.promotion_class != "recon"
                && !spec.siege
                && spec.has_ranged_attack()
        };
        if g.units.values().any(|unit| {
            unit.owner == pid
                && g.wdist(unit.pos, city.pos) <= 2
                && shooter(&g.rules.units[unit.kind])
        }) || g.cities.values().any(|other| {
            other.owner == pid
                && other.id != cid
                && g.wdist(other.pos, city.pos) <= 4
                && other.queue.first().is_some_and(|item| match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => {
                        shooter(&g.rules.units[unit])
                    }
                    _ => false,
                })
        }) {
            return None;
        }
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| shooter(spec))
            .filter_map(|(unit, spec)| {
                let item = Item::Unit { unit: *unit };
                g.can_produce(pid, cid, &item).then_some((
                    spec.range >= 2,
                    spec.ranged_attack_strength(),
                    *unit,
                    item,
                ))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)))
            .map(|(_, _, _, item)| item)
    }

    /// Whether one district in this family is already finished or committed
    /// somewhere in the empire.
    ///
    /// The live mirror represents a Firaxis district that has been placed but
    /// is still under construction as a map `district_foundation`, rather than
    /// as a city queue entry. It is just as committed as either of the other
    /// two forms: issuing another order for the same family wastes a specialty
    /// slot and can spend hundreds of production on a duplicate Spaceport.
    fn empire_district_family_ready_or_queued(g: &Game, pid: usize, family: &str) -> bool {
        let family = Name::new(family);
        g.player_city_ids(pid)
            .into_iter()
            .any(|city| Self::district_family_held_or_queued(g, city, family))
    }

    /// Whether `cid` holds a district of `family`, its foundation, or has one
    /// first in its queue.
    fn district_family_held_or_queued(g: &Game, cid: u32, family: Name) -> bool {
        let city = &g.cities[&cid];
        g.city_has_district_family(city, family)
            || city.owned_tiles.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| g.district_family(foundation.district) == family)
            })
            || matches!(
                city.queue.first(),
                Some(Item::District { district, .. })
                    if g.district_family(*district) == family
            )
    }

    /// Pick a real placed wonder rather than treating it as an ordinary
    /// building. `producible_items` supplies fully validated sites; filtering
    /// globally queued names keeps two cities from entering the same race.
    fn cheapest_available_wonder(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let queued: HashSet<Name> = g
            .cities
            .values()
            .filter_map(|city| match city.queue.first() {
                Some(Item::Wonder { wonder, .. }) => Some(*wonder),
                _ => None,
            })
            .collect();
        g.producible_items(pid, cid)
            .into_iter()
            .filter(|item| matches!(item, Item::Wonder { wonder, .. } if !queued.contains(wonder)))
            .min_by(|left, right| {
                g.item_cost_for_city(pid, cid, left)
                    .total_cmp(&g.item_cost_for_city(pid, cid, right))
                    .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
            })
    }

    fn units(&mut self, g: &mut Game, pid: usize) {
        self.begin_movement_turn(g, pid);
        self.prepare_unit_formations(g, pid);
        self.recovering_units
            .retain(|uid| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        self.patrol_targets
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        self.settler_targets
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        for uid in g.player_unit_ids(pid) {
            let mut took_a_turn = false;
            for _ in 0..8 {
                if !g.units.contains_key(&uid) {
                    break;
                }
                if g.units[&uid].moves_left <= 0.0 {
                    break;
                }
                let kind = g.units[&uid].kind;
                let acted = match kind.as_str() {
                    "settler" => self.settler_step(g, pid, uid),
                    "builder" => self.builder_step(g, pid, uid),
                    "military_engineer" => self.military_engineer_step(g, pid, uid),
                    "naturalist" => self.naturalist_step(g, pid, uid),
                    "archaeologist" => self.archaeologist_step(g, pid, uid),
                    "trader" => self.trader_step(g, pid, uid),
                    "missionary" => self.missionary_step(g, pid, uid),
                    "battering_ram" | "siege_tower" => self.siege_support_step(g, pid, uid),
                    "rock_band" => self.rock_band_step(g, pid, uid),
                    _ => self.military_step(g, pid, uid),
                };
                if !acted {
                    break;
                }
                took_a_turn = true;
            }
            if !took_a_turn {
                self.hold_stood_down_unit(g, pid, uid);
            }
        }
    }

    /// How far from one of our cities a foreign religious unit still counts as
    /// a fight we are going to be in. An Apostle moves 3 and the enemy has to
    /// reach a City Center to spread, so this is about one turn of approach.
    const APOSTLE_HOME_WATCH: i32 = 4;

    /// Is one of our own cities being taken by somebody else's faith?
    ///
    /// Two ways to be in trouble, and the second is the one worth having: a
    /// city whose MAJORITY is already a foreign faith is lost and needs
    /// reconverting, and a city where a rival faith has reached half of our own
    /// pressure is being lost while there is still time to answer. The second
    /// test is deliberately the same shape as
    /// `AdvancedAi::city_needs_religious_support`, so the promotion and the
    /// faith purchase agree about what "under pressure" means.
    fn home_faith_is_contested(&self, g: &Game, pid: usize) -> bool {
        let ours = g.players[pid].religion.as_deref();
        g.player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .any(|city| {
                let majority = g.city_religion(city);
                if majority.is_some() && majority != ours {
                    return true;
                }
                let Some(ours) = ours else {
                    return false;
                };
                let mine = city.pressure.get(ours).copied().unwrap_or(0.0);
                city.pressure
                    .iter()
                    .any(|(faith, amount)| faith != ours && *amount * 2.0 >= mine)
            })
    }

    /// Is a foreign religious unit already inside our reach?
    ///
    /// Only an Apostle or an Inquisitor can start a theological attack, but a
    /// Missionary can be attacked — so any foreign religious unit near home is
    /// either a target or an attacker, and both are answered by the same +20.
    fn foreign_faith_unit_near_home(&self, g: &Game, pid: usize) -> bool {
        let ours = g.players[pid].religion.as_deref();
        let home: Vec<crate::Pos> = g
            .player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .map(|city| city.pos)
            .collect();
        if home.is_empty() {
            return false;
        }
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.rules
                    .units
                    .get(&unit.kind)
                    .is_some_and(|spec| spec.class == "religious")
                && unit.religion.as_deref() != ours
                && home
                    .iter()
                    .any(|pos| g.wdist(*pos, unit.pos) <= Self::APOSTLE_HOME_WATCH)
        })
    }

    /// The Apostle promotions this empire wants, best first.
    ///
    /// Three branches — reconvert, fight, spread — and every one of them names
    /// all nine promotions, so whichever three the offer holds, one of them is
    /// always found and the magnitude rule is never reached by accident. See
    /// [`BasicAi::apostle_promotion_by_role`] for why the shipped rule cannot
    /// express any of this.
    pub(crate) fn apostle_promotion_preference(
        &self,
        g: &Game,
        pid: usize,
    ) -> &'static [&'static str] {
        if self.home_faith_is_contested(g, pid) {
            // Reconversion. `proselytizer` triples what one spread evicts
            // (`do_spread`'s eviction goes 0.25 -> 0.75), `orator` buys five
            // spreads instead of three, and `debater` matters because the
            // rival's Apostles are usually standing in the city being taken.
            return &[
                "proselytizer",
                "orator",
                "debater",
                "chaplain",
                "translator",
                "indulgence_vendor",
                "pilgrim",
                "martyr",
                "heathen_conversion",
            ];
        }
        if self.foreign_faith_unit_near_home(g, pid) {
            // A fight we are in whether or not we chose it. `debater` is the
            // only promotion that touches `theological_strength`; `chaplain`
            // is what lets a unit survive a second one.
            return &[
                "debater",
                "chaplain",
                "proselytizer",
                "orator",
                "translator",
                "indulgence_vendor",
                "pilgrim",
                "martyr",
                "heathen_conversion",
            ];
        }
        // A quiet homeland: the Apostle's whole job is spreading abroad. The
        // shipped rule gets the FIRST choice right by luck — `translator` has
        // the biggest number — and then gets the next one wrong, because
        // `indulgence_vendor` is a GOLD promotion at 100 and outranks
        // `proselytizer` at 50 and `orator` at 2. The offer is three of nine,
        // so the second choice is made often: over four whole games the
        // reachability census counted `indulgence_vendor` taken SIX times out
        // of twenty-four, exactly as often as `translator`. A quarter of this
        // empire's Apostles were promoted to earn gold per conversion instead
        // of to convert.
        //
        // Conversion first, then the fight, then the trinkets.
        &[
            "translator",
            "proselytizer",
            "orator",
            "debater",
            "chaplain",
            "indulgence_vendor",
            "pilgrim",
            "martyr",
            "heathen_conversion",
        ]
    }

    /// Spend earned promotions before moving, then consolidate eligible
    /// military units into Corps/Armies and attach colocated support units.
    /// These actions otherwise never occur in headless self-play because they
    /// are neither movement nor attacks.
    pub(crate) fn prepare_unit_formations(&self, g: &mut Game, pid: usize) {
        // Computed at most once per turn, and only if an Apostle actually has
        // a promotion to take: it walks every city and every unit on the board.
        let mut apostle_preference: Option<&'static [&'static str]> = None;
        for uid in g.player_unit_ids(pid) {
            let offered = g.available_promotions(uid);
            if offered.is_empty() {
                continue;
            }
            let shipped = |offered: &[Name]| -> Option<Name> {
                offered.iter().copied().max_by(|a, b| {
                    let value = |name: &str| {
                        g.rules.promotions[name]
                            .effects
                            .values()
                            .map(|effect| effect.abs())
                            .sum::<f64>()
                    };
                    value(a)
                        .partial_cmp(&value(b))
                        .unwrap()
                        .then_with(|| b.cmp(a))
                })
            };
            let is_apostle = self.apostle_promotion_by_role
                && g.units.get(&uid).is_some_and(|unit| {
                    g.rules
                        .units
                        .get(&unit.kind)
                        .is_some_and(|spec| spec.promotion_class == "religious_apostle")
                });
            let promotion = if is_apostle {
                let ranked = *apostle_preference
                    .get_or_insert_with(|| self.apostle_promotion_preference(g, pid));
                ranked
                    .iter()
                    .find_map(|want| offered.iter().find(|name| **name == **want).copied())
                    // An offer out of a modded ruleset holding none of the nine
                    // named promotions: the shipped rule decides, exactly as it
                    // does today.
                    .or_else(|| shipped(&offered))
            } else {
                shipped(&offered)
            };
            let Some(promotion) = promotion else {
                continue;
            };
            let _ = g.apply(
                pid,
                &Action::Promote {
                    unit: uid,
                    promotion: Name::new(&promotion),
                },
            );
        }

        if g.players[pid].civics.contains(&crate::name!("nationalism")) {
            let reserve = (g.player_city_ids(pid).len() + 3).max(5);
            loop {
                let military = g
                    .player_unit_ids(pid)
                    .into_iter()
                    .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
                    .count();
                if military <= reserve {
                    break;
                }
                let action = g
                    .legal_actions_within(pid, ActionFamilies::FORMATIONS)
                    .into_iter()
                    .find(|action| matches!(action, Action::CombineUnits { .. }));
                let Some(action) = action else { break };
                if g.apply(pid, &action).is_err() {
                    break;
                }
            }
        }

        let has_link_candidate = |game: &Game| {
            let units = game.player_unit_ids(pid);
            units.iter().enumerate().any(|(index, unit)| {
                units[index + 1..].iter().any(|with| {
                    let a = &game.units[unit];
                    let b = &game.units[with];
                    if a.pos != b.pos || a.linked_to.is_some() || b.linked_to.is_some() {
                        return false;
                    }
                    let a_spec = &game.rules.units[a.kind];
                    let b_spec = &game.rules.units[b.kind];
                    let support = (a_spec.class == "support"
                        && a.kind != "military_engineer"
                        && b_spec.class == "military")
                        || (b_spec.class == "support"
                            && b.kind != "military_engineer"
                            && a_spec.class == "military");
                    let naval_settler = (a_spec.domain.as_deref() == Some("sea")
                        && b.kind == "settler")
                        || (b_spec.domain.as_deref() == Some("sea") && a.kind == "settler");
                    support || naval_settler
                })
            })
        };
        while has_link_candidate(g) {
            let action = g
                .legal_actions_within(pid, ActionFamilies::FORMATIONS)
                .into_iter()
                .find(|action| match action {
                    Action::LinkUnits { unit, with } => {
                        let a = &g.rules.units[g.units[unit].kind];
                        let b = &g.rules.units[g.units[with].kind];
                        let support = (a.class == "support"
                            && g.units[unit].kind != "military_engineer"
                            && b.class == "military")
                            || (b.class == "support"
                                && g.units[with].kind != "military_engineer"
                                && a.class == "military");
                        let naval_settler = (a.domain.as_deref() == Some("sea")
                            && g.units[with].kind == "settler")
                            || (b.domain.as_deref() == Some("sea")
                                && g.units[unit].kind == "settler");
                        support || naval_settler
                    }
                    _ => false,
                });
            let Some(action) = action else { break };
            if g.apply(pid, &action).is_err() {
                break;
            }
        }
    }

    fn step_toward(&self, g: &mut Game, pid: usize, uid: u32, target: Pos) -> bool {
        self.step_toward_range(g, pid, uid, target, 0)
    }

    /// A tactical step its caller has proven strictly nearer its goal than
    /// the tile it leaves (`gun-queues-behind-the-column`; by route length
    /// under `gun-queue-follows-the-road`), so no sequence of such steps can
    /// loop: the window's retread memory (`retreads_a_loop`)
    /// does not apply, while the same-turn reversal, the lethal-tile and the
    /// host-refusal bars still do. Other movers walk a siege gun back and
    /// forth through the staging band, and the memory then refused its
    /// every step forward to its post. Off `recorded_tactical_step`, the raw
    /// apply as in `tactical_apply_move`.
    pub(crate) fn tactical_apply_progress_move(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        to: Pos,
    ) -> bool {
        if !self.recorded_tactical_step {
            return g.apply(pid, &Action::Move { unit: uid, to }).is_ok();
        }
        self.path_move_inner(g, pid, uid, to, true)
    }

    /// Follow one A* step out of a *proven* live-bridge livelock.
    ///
    /// `path_move` normally forbids every historical retread. That is right
    /// for a greedy step, but it also rejects the first square of a valid
    /// detour when the route must briefly pass back through the pocket that
    /// diagnosed the livelock. The same-turn reversal check remains in force;
    /// only the multi-turn tabu is relaxed, and only after six fruitless
    /// recorded starts have established the loop.
    pub(crate) fn tactical_apply_livelock_route_escape(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        to: Pos,
    ) -> bool {
        if !self.live_livelock_route_escape(uid) {
            return false;
        }
        self.path_move_inner(g, pid, uid, to, true)
    }

    pub(crate) fn path_move(&self, g: &mut Game, pid: usize, uid: u32, to: Pos) -> bool {
        self.path_move_inner(g, pid, uid, to, false)
    }

    /// Order a whole walk rather than one step, for a destination the unit can
    /// only reach by crossing one of its own units.
    ///
    /// `Action::MoveTo` is the only action that may execute such a path — see
    /// [`Game::entry_at`]: Civilization VI checks the stacking layer at the
    /// end of a move, so the crossing is legal and no single `Move` onto the
    /// friendly tile ever is. The backtrack and livelock bookkeeping is the
    /// same as an ordinary step's, recorded against the tile walked from.
    pub(crate) fn path_walk_to(&self, g: &mut Game, pid: usize, uid: u32, to: Pos) -> bool {
        let from = g.units[&uid].pos;
        if !self.path_step_allowed(g, uid, from, to, false) {
            return false;
        }
        if g.apply(pid, &Action::MoveTo { unit: uid, to }).is_err() {
            return false;
        }
        self.record_path_step(g, uid, from);
        self.record_move_refusal_watch(g, uid, from, to);
        true
    }

    fn path_move_inner(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        to: Pos,
        allow_livelock_retread: bool,
    ) -> bool {
        let from = g.units[&uid].pos;
        if !self.path_step_allowed(g, uid, from, to, allow_livelock_retread) {
            return false;
        }
        // Settlers use the shared route-order tool exposed to network clients
        // and learned agents. The AI has already selected and validated this
        // adjacent step, so it remains behaviorally identical to Move without
        // making every military step pay for route reconstruction.
        let movement = if g.units[&uid].kind == "settler" {
            Action::MoveTo { unit: uid, to }
        } else {
            Action::Move { unit: uid, to }
        };
        if g.apply(pid, &movement).is_err() {
            return false;
        }
        self.record_path_step(g, uid, from);
        self.record_move_refusal_watch(g, uid, from, to);
        true
    }

    /// Remember the FIRST pathed step this unit was issued this turn, which is
    /// the step the live bridge's coalesced `MOVE_TO` starts with and the one
    /// a host `did_not_move` refusal falsifies. Later same-turn steps replan
    /// from tiles the host never saw the unit on, so only the first is
    /// evidence. Inert with `live-move-refusal-break` off.
    fn record_move_refusal_watch(&self, g: &Game, uid: u32, from: Pos, to: Pos) {
        if !self.move_refusal_break {
            return;
        }
        let mut watch = self.move_refusal_watch.borrow_mut();
        let fresh = watch.get(&uid).is_none_or(|(turn, _, _)| *turn != g.turn);
        let entry = watch.entry(uid).or_insert((g.turn, from, to));
        if entry.0 != g.turn {
            *entry = (g.turn, from, to);
        }
        if fresh && self.own_column_is_not_a_refusal {
            let mut congested = self.move_refusal_congested.borrow_mut();
            if self.frame_start_friend_holds(g, uid, to) {
                congested.insert(uid, g.turn);
            } else {
                congested.remove(&uid);
            }
        }
    }

    /// Every refusal a pathed move owes to the controller rather than to the
    /// rules: a minor leaving its defense area, a same-turn reversal, and a
    /// route that keeps re-proposing a tile the unit has been standing on.
    fn path_step_allowed(
        &self,
        g: &Game,
        uid: u32,
        from: Pos,
        to: Pos,
        allow_livelock_retread: bool,
    ) -> bool {
        if self.minor && (!self.barb || !self.barbarian_tactics) {
            let Some(home) = Self::minor_home(g, g.units[&uid].owner) else {
                return false;
            };
            let from_home = g.wdist(home, from);
            let to_home = g.wdist(home, to);
            // Once local, never step back outside the defense area. A unit
            // already stranded beyond it may still take a pathfinder detour
            // around terrain while returning home.
            if from_home <= MINOR_DEFENSE_RADIUS && to_home > MINOR_DEFENSE_RADIUS {
                return false;
            }
        }
        // `guns-stay-out-of-reach`: a gun does not step onto a tile where one
        // visible hostile's blow destroys it, unless the step lowers the
        // blow it stands under. Empty with the gene off.
        if let Some(lethal) = self.gun_lethal_tiles.get(&uid) {
            if let Some(blow) = lethal.get(&to) {
                if lethal.get(&from).is_none_or(|here| blow >= here) {
                    return false;
                }
            }
        }
        // `guns-stay-out-of-reach-2`: the same refusal for every other land
        // military unit, counting only blows whose hostile is still on the
        // board — a hostile killed earlier this frame vetoes nothing. Empty
        // with the gene off.
        if let Some(lethal) = self.unit_lethal_blows.get(&uid) {
            let strongest = |tile: &Pos| {
                lethal.get(tile).map_or(0.0, |blows| {
                    blows
                        .iter()
                        .filter(|(source, _)| g.units.contains_key(source))
                        .map(|(_, blow)| *blow)
                        .fold(0.0_f64, f64::max)
                })
            };
            let blow = strongest(&to);
            if blow > 0.0 && blow >= strongest(&from) {
                return false;
            }
        }
        // `live-move-refusal-break`: a step this unit has provably been
        // refused by the host — issued on consecutive turns while the unit
        // never left its tile — is not proposed again until the bar expires,
        // so the pathing picks a different step (or a different target)
        // instead of freezing. See `judge_move_refusals`.
        if self.move_refusal_break
            && self
                .move_refusal_blocks
                .get(&uid)
                .is_some_and(|(step, until)| *step == to && g.turn < *until)
        {
            return false;
        }
        // Shipped: only the step just taken is remembered, so `A -> B -> A` is
        // refused. With `whole_turn_backtrack_guard` on, every tile walked
        // this turn is remembered, which is what refuses the longer loops --
        // `A -> B -> C -> A` and up -- that a one-deep memory cannot see.
        // `allow_livelock_retread` still overrides both: a proven livelock
        // escape has to be allowed back through the pocket it came from.
        let whole_turn = self.whole_turn_backtrack_guard && !allow_livelock_retread;
        let revisits_this_turn =
            self.last_path_step_from
                .borrow()
                .get(&uid)
                .is_some_and(|(turn, trail)| {
                    *turn == g.turn
                        && if whole_turn {
                            trail.contains(&to)
                        } else {
                            trail.last() == Some(&to)
                        }
                });
        if revisits_this_turn {
            return false;
        }
        // The same refusal over the unit's last several turns rather than its
        // last several movement points. A route that keeps proposing a tile
        // this unit has already been standing on all window is not a route.
        if !allow_livelock_retread && self.retreads_a_loop(uid, to) {
            return false;
        }
        true
    }

    /// Remember the tile a pathed move left, which is what the same-turn
    /// reversal guard reads.
    fn record_path_step(&self, g: &Game, uid: u32, from: Pos) {
        let mut trails = self.last_path_step_from.borrow_mut();
        let entry = trails.entry(uid).or_insert((g.turn, Vec::new()));
        if entry.0 != g.turn {
            *entry = (g.turn, Vec::new());
        }
        entry.1.push(from);
    }

    /// Move toward a target without insisting on entering its tile. Religious
    /// units spread from an adjacent hex, so routing them to range zero makes
    /// the pathfinder reject foreign city centers and can strand an entire
    /// procession behind a mountain detour.
    pub(crate) fn step_toward_range(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        stop_range: i32,
    ) -> bool {
        if let Some(acted) = self.risk_aware_route_step(g, pid, uid, target, stop_range) {
            return acted;
        }
        let cur = g.units[&uid].pos;
        if g.wdist(cur, target) <= stop_range {
            return false;
        }
        let mut local: Vec<Pos> = g
            .nbrs(cur)
            .into_iter()
            .filter(|p| g.can_move(uid, *p))
            .collect();
        // Dry land first for a land unit. This greedy step ranked neighbours on
        // geometric distance alone, and `can_move` says yes to open water for
        // every land unit once embarkation unlocks — so a straight line across
        // a bay always beat walking around it, and the army marched into the
        // sea. Embarked it cannot attack (`Game::apply` refuses it) or fortify.
        //
        // ⚠ The loop below `break`s on the first neighbour that fails to make
        // progress, which is only sound while the order is by distance. So the
        // non-improving neighbours are dropped here rather than re-sorted
        // behind the dry ones — otherwise a dry tile that moves away from the
        // target would cut off the improving water tiles behind it and the
        // unit would simply stop. A water step is still reachable: it sorts
        // last, and the A* fallback below is untouched, so a genuine ocean
        // crossing still happens when no dry neighbour makes progress.
        let prefer_dry =
            self.come_ashore && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea");
        if prefer_dry {
            let here = g.wdist(cur, target);
            local.retain(|p| g.wdist(*p, target) < here);
        }
        local.sort_by_key(|p| {
            let wet = prefer_dry && g.map.get(*p).is_some_and(|tile| g.rules.is_water(tile));
            (wet, g.wdist(*p, target), *p)
        });
        for next in local {
            if g.wdist(next, target) >= g.wdist(cur, target) {
                break; // sorted: no remaining neighbor makes progress
            }
            // A neighbor can still be refused (stacking, ZOC); try the next
            // improving tile before paying for A*.
            if self.path_move(g, pid, uid, next) {
                return true;
            }
        }

        // The common case above stays as cheap as the original greedy AI;
        // invoke A* only when no legal neighbor makes geometric progress.
        if let Some(next) = g
            .route_step(uid, target, stop_range)
            .filter(|p| g.can_move(uid, *p))
        {
            if self.path_move(g, pid, uid, next) {
                return true;
            }
            // A peer can take the A* tile first; sidestep at equal distance so
            // a marching column keeps flowing around the blockage.
            for p in g.nbrs(cur) {
                if g.wdist(p, target) == g.wdist(cur, target)
                    && g.can_move(uid, p)
                    && self.path_move(g, pid, uid, p)
                {
                    return true;
                }
            }
        }
        // ★★★★ EVERYTHING ABOVE ASKS FOR A TILE THIS UNIT MAY BE LEFT
        // STANDING ON, AND A COLUMN IN A DEFILE HAS NONE: each unit's only
        // improving neighbour holds the one in front of it. Civilization VI
        // moves that unit through its own column and stops it beyond — see
        // `Game::entry_at` — which is one order and not one step, so it is
        // reached here and never by the greedy loop above. Deliberately last:
        // the unit is otherwise standing still this turn, so this replaces a
        // hold and never a march anybody already chose.
        if let Some(dest) = g.pass_through_destination(uid, target, stop_range) {
            if self.path_walk_to(g, pid, uid, dest) {
                return true;
            }
        }
        false
    }

    /// The connected dry ground underneath the Palace city. `Tile::continent`
    /// is a map-script annotation and can be absent or split a single island,
    /// so these genes use the map's actual dry-land connectivity instead.
    pub(crate) fn capital_landmass(g: &Game, pid: usize) -> BTreeSet<Pos> {
        let Some(home) = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .min_by_key(|city| (std::cmp::Reverse(g.city_has_palace(city)), city.id))
            .map(|city| city.pos)
        else {
            return BTreeSet::new();
        };
        let mut landmass = BTreeSet::new();
        // The set is the answer, but the flood asked it about every
        // neighbour of every plot — six ordered-set probes a plot, for every
        // naval explorer, every turn (1.5% of a ladder game's CPU in the
        // 2026-09-26 profile). A dense table in tile-vector order (see
        // `first_visit` in the settle-site sweep) answers the same question
        // with one index; it is marked exactly when the set is, so the two
        // never disagree, and a plot the grid cannot index (none in practice
        // — every neighbour is on the map) still asks the set.
        let mut seen = vec![false; g.map.tiles.len()];
        let mut frontier = VecDeque::from([home]);
        while let Some(pos) = frontier.pop_front() {
            if !landmass.insert(pos) {
                continue;
            }
            if let Some(index) = g.map.tiles.index_of(pos) {
                seen[index] = true;
            }
            for next in g.nbrs(pos) {
                let visited = match g.map.tiles.index_of(next) {
                    Some(index) => seen[index],
                    None => landmass.contains(&next),
                };
                if visited {
                    continue;
                }
                if g.map
                    .get(next)
                    .is_some_and(|tile| !g.rules.is_unknown(tile) && !g.rules.is_water(tile))
                {
                    frontier.push_back(next);
                }
            }
        }
        landmass
    }

    fn trader_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        // In the walking representation a newly built Trader can spend several
        // turns exposed before reaching an origin. During a local barbarian
        // alert, retreat it to a friendly city first; once it is on the city
        // tile, native route creation is immediate and safe to attempt.
        if self.barbarian_tactics
            && self.barbarian_trade_risk_for_controller(g, pid)
            && g.city_at(upos)
                .is_none_or(|cid| g.cities[&cid].owner != pid)
        {
            let target = g
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .min_by_key(|city| (g.wdist(upos, city.pos), city.id))
                .map(|city| city.pos);
            if let Some(target) = target {
                return self.step_toward(g, pid, uid, target);
            }
        }
        if let Some(origin) = g.city_at(upos).filter(|c| g.cities[c].owner == pid) {
            // best destination: most districts in range (domestic or foreign)
            let mut best: Option<(usize, usize, u32)> = None;
            for (cid, c) in &g.cities {
                if !g.can_establish_trade_route(pid, origin, *cid) {
                    continue;
                }
                let alliance_connection = g.alliance_with(pid, c.owner).is_some_and(|_| {
                    !g.routes.iter().any(|route| {
                        route.owner == pid
                            && route.ends > g.turn
                            && g.cities
                                .get(&route.dest)
                                .is_some_and(|destination| destination.owner == c.owner)
                    })
                }) as usize;
                let key = (alliance_connection, c.districts.len() + 1, *cid);
                if best.map(|old| key > old).unwrap_or(true) {
                    best = Some(key);
                }
            }
            if let Some((_, _, dest)) = best {
                return g
                    .apply(
                        pid,
                        &Action::TradeRoute {
                            unit: uid,
                            city: dest,
                        },
                    )
                    .is_ok();
            }
        }
        // A Trader can be completed in a city whose nearby destinations are
        // already reserved. Relocate it to the nearest origin with a legal
        // route instead of retrying an invalid assignment every turn.
        let target = g
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .filter(|origin| {
                g.cities
                    .values()
                    .any(|destination| g.can_establish_trade_route(pid, origin.id, destination.id))
            })
            .min_by_key(|c| (g.wdist(upos, c.pos), c.id))
            .map(|c| c.pos);
        match target {
            Some(t) => self.step_toward(g, pid, uid, t),
            None => false,
        }
    }

    fn missionary_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        // Spread the unit's own faith: a purchased Missionary carries its
        // city's majority religion, which for a civilization that never
        // founded one is an adopted faith the player religion cannot name.
        let religion = match g.units[&uid]
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        {
            Some(r) => r,
            None => return false,
        };
        let upos = g.units[&uid].pos;
        // Own cities first: reconverting the homeland both consolidates
        // pressure and is the entire job of a defensive adopted-faith unit.
        let mut targets: Vec<(bool, i32, u32, Pos)> = g
            .cities
            .values()
            .filter(|c| g.city_religion(c) != Some(religion.as_str()) && !g.is_at_war(pid, c.owner))
            .map(|city| {
                (
                    city.owner != pid,
                    g.wdist(upos, city.pos),
                    city.id,
                    city.pos,
                )
            })
            .collect();
        targets.sort();
        for (_, _, _, target) in targets {
            if g.wdist(upos, target) <= 1 {
                return g.apply(pid, &Action::Spread { unit: uid }).is_ok();
            }
            if self.step_toward_range(g, pid, uid, target, 1) {
                return true;
            }
        }
        false
    }

    fn siege_support_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        let support_kind = g.units[&uid].kind.as_str();
        let targets: Vec<Pos> = g
            .cities
            .values()
            .filter(|c| c.owner != pid && g.is_at_war(pid, c.owner))
            .filter(|c| c.wall_hp > 0 && g.city_allows_siege_support(c.id, support_kind))
            .map(|c| c.pos)
            .collect();
        if targets.is_empty() {
            return false;
        }

        // Follow an eligible military escort toward the breach.
        let escort = g
            .units
            .values()
            .filter(|u| u.owner == pid && u.id != uid)
            .filter(|u| {
                let spec = &g.rules.units[u.kind];
                siege_support::eligible_attacker(spec)
            })
            .min_by_key(|u| {
                let front = targets.iter().map(|t| g.wdist(u.pos, *t)).min().unwrap();
                (2 * front + g.wdist(upos, u.pos), g.wdist(upos, u.pos), u.id)
            })
            .map(|u| u.pos);
        match escort {
            Some(pos) if pos != upos => self.step_toward(g, pid, uid, pos),
            _ => false,
        }
    }

    pub(crate) fn rock_band_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let Some(here) = g.units.get(&uid).map(|unit| unit.pos) else {
            return false;
        };
        let can_perform_here = g.rock_concert_tourism(pid, uid).is_some();
        let acted = if self.band_plays_what_it_can_reach {
            self.rock_band_reachable_step(g, pid, uid, here, can_perform_here)
        } else {
            self.rock_band_tour_step(g, pid, uid, here, can_perform_here)
        };
        // `bands-wait-at-the-door`: nothing to play or walk to, so wait where
        // the land is most likely to open.
        acted || (self.bands_wait_at_the_door && self.rock_band_door_step(g, pid, uid, here))
    }

    /// The stock tour: walk toward the best venue, or play here.
    fn rock_band_tour_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        here: Pos,
        can_perform_here: bool,
    ) -> bool {
        // Passing through a legal venue must not abandon the better tour the
        // same scorer selected on the approach. Native 111731Z's Album Cover
        // Art band stopped at a Theater with a legal matching Wonder one hex
        // away. Rank the current venue at zero travel alongside the others.
        let tour = self.rock_band_ranked_venues(g, pid, uid).first().copied();
        if let Some(target) = tour.filter(|target| *target != here) {
            if let Some(next) = g.route_step(uid, target, 0) {
                if self.path_move(g, pid, uid, next) {
                    return true;
                }
            }
        }
        // Keep a concert when this venue wins, or a better trip cannot start.
        can_perform_here && g.apply(pid, &Action::PerformConcert { unit: uid }).is_ok()
    }

    /// Every venue this band could tour to, best first, by whichever scorer
    /// the seat plays (`bands-draw-from-the-bar`, `band-plays-its-stage`, or
    /// the stock tour).
    fn rock_band_ranked_venues(&self, g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        if self.bands_draw_from_the_bar {
            Self::rock_band_career_ranking(g, uid, |position| {
                g.rock_concert_bar_value(pid, uid, position)
            })
        } else if self.band_plays_its_stage {
            Self::rock_band_career_ranking(g, uid, |position| {
                g.rock_concert_stage_value(pid, uid, position)
            })
        } else {
            Self::rock_band_tour_ranking(g, pid, uid)
        }
    }

    /// `band-plays-what-it-can-reach`: play, or step toward a venue the band
    /// can really step toward, never stand. The ranking is the seat's own
    /// scorer; a venue whose step is refused here (a barred step, a danger
    /// veto) gives way to the next one, a host whose land walked a trip
    /// nowhere last turn is skipped for [`BAND_REFUSAL_BAR_TURNS`], and the
    /// band plays where it stands when that venue is the best one left or
    /// nothing better can be started.
    fn rock_band_reachable_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        here: Pos,
        can_perform_here: bool,
    ) -> bool {
        let key = Self::band_memory_key(g, uid);
        self.judge_band_trip(g, key, here);
        let refused: HashSet<String> = self
            .band_refused_hosts
            .borrow()
            .iter()
            .filter(|(_, until)| g.turn < **until)
            .map(|(civ, _)| civ.clone())
            .collect();
        let mut tries = 0;
        for target in self.rock_band_ranked_venues(g, pid, uid) {
            if target == here {
                break;
            }
            if Self::band_venue_host(g, target)
                .is_some_and(|host| refused.contains(&g.players[host].civ))
            {
                continue;
            }
            if tries == BAND_TOUR_TRIES {
                break;
            }
            tries += 1;
            let Some(next) = g.route_step(uid, target, 0) else {
                continue;
            };
            if self.path_move(g, pid, uid, next) {
                let mut trips = self.band_trips.borrow_mut();
                let from = match trips.get(&key) {
                    Some((turn, from, _)) if *turn == g.turn => *from,
                    _ => here,
                };
                trips.insert(key, (g.turn, from, target));
                return true;
            }
        }
        can_perform_here && g.apply(pid, &Action::PerformConcert { unit: uid }).is_ok()
    }

    /// The identity band memory is kept under: the band's Civilization VI id
    /// on a live mirror, whose board ids are reassigned every frame, and the
    /// board id everywhere else.
    fn band_memory_key(g: &Game, uid: u32) -> i64 {
        g.host_unit_facts
            .get(&uid)
            .and_then(|facts| facts.civ6_id)
            .unwrap_or(i64::from(uid))
    }

    /// The civilization whose land holds this venue.
    fn band_venue_host(g: &Game, venue: Pos) -> Option<usize> {
        g.map
            .get(venue)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner)
    }

    /// Judge the band's last trip, once, on the first frame of a later turn:
    /// a band still on the tile it set out from was walked nowhere by the
    /// host, so the host of the venue it set out for is skipped by every band
    /// for [`BAND_REFUSAL_BAR_TURNS`] standard turns. Live 20261110T164254Z:
    /// the host refused the same walk into Ethiopia 23 times while the
    /// planner kept choosing it.
    fn judge_band_trip(&self, g: &Game, key: i64, here: Pos) {
        let Some((turn, from, target)) = self.band_trips.borrow().get(&key).copied() else {
            return;
        };
        if turn == g.turn {
            // A same-turn replan: the host has not answered yet.
            return;
        }
        self.band_trips.borrow_mut().remove(&key);
        if turn + 1 != g.turn || here != from {
            return;
        }
        let Some(host) = Self::band_venue_host(g, target) else {
            return;
        };
        let civ = g.players[host].civ.clone();
        let until = g.turn + g.standard_duration(BAND_REFUSAL_BAR_TURNS);
        think!(self.journal, Military, Detail,
               "Rock Band skips {civ}'s venues; the host walked it nowhere";
               "set out from {from:?} for {target:?} last turn and is still there; \
                {civ}'s venues wait {BAND_REFUSAL_BAR_TURNS} standard turns";
               here);
        self.band_refused_hosts.borrow_mut().insert(civ, until);
    }

    /// Rank a tour by its existing concert valuation and the time spent
    /// reaching it. A distant premium venue must repay its longer journey.
    /// Route length is an estimate in hex steps, divided by the band's actual
    /// full movement allowance; it is not a terrain-exact arrival prediction.
    #[cfg(test)]
    fn rock_band_tour_destination(g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        Self::rock_band_tour_ranking(g, pid, uid).first().copied()
    }

    fn rock_band_tour_ranking(g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        let moves = g.unit_max_moves(uid).max(1.0);
        let mut venues: Vec<(f64, usize, Pos)> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter_map(|position| {
                let value = g.rock_concert_ai_value(pid, uid, position)?;
                let distance = g.route_distance(uid, position, 0)?;
                let travel_turns = distance as f64 / moves;
                Some((value / (1.0 + travel_turns), distance, position))
            })
            .collect();
        venues.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then(left.1.cmp(&right.1))
                .then(left.2.cmp(&right.2))
        });
        venues
            .into_iter()
            .map(|(_, _, position)| position)
            .collect()
    }

    /// `band-plays-its-stage`: the same tour, ranked by the concert plus what
    /// the band keeps earning if it survives it, weighted toward the rival
    /// holding the Culture bar. Travel is priced gently because the band's
    /// future, not this one concert, is what the trip buys; a trip beyond
    /// fifteen turns is off the table.
    #[cfg(test)]
    pub(crate) fn rock_band_stage_destination(g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        Self::rock_band_career_ranking(g, uid, |position| {
            g.rock_concert_stage_value(pid, uid, position)
        })
        .first()
        .copied()
    }

    /// `bands-draw-from-the-bar`: the same career-priced tour, weighted by how
    /// much of the Culture race a visitor from the host closes. See
    /// [`Game::rock_concert_bar_value`].
    #[cfg(test)]
    pub(crate) fn rock_band_bar_destination(g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        Self::rock_band_career_ranking(g, uid, |position| {
            g.rock_concert_bar_value(pid, uid, position)
        })
        .first()
        .copied()
    }

    fn rock_band_career_ranking(
        g: &Game,
        uid: u32,
        value_at: impl Fn(Pos) -> Option<f64>,
    ) -> Vec<Pos> {
        let moves = g.unit_max_moves(uid).max(1.0);
        let mut venues: Vec<(f64, usize, Pos)> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter_map(|position| {
                let value = value_at(position)?;
                let distance = g.route_distance(uid, position, 0)?;
                let travel_turns = distance as f64 / moves;
                (travel_turns <= ROCK_BAND_TOUR_MAX_TURNS).then_some((
                    value / (1.0 + 0.1 * travel_turns),
                    distance,
                    position,
                ))
            })
            .collect();
        venues.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then(left.1.cmp(&right.1))
                .then(left.2.cmp(&right.2))
        });
        venues
            .into_iter()
            .map(|(_, _, position)| position)
            .collect()
    }

    /// The district a civilization builds in place of `family`: its unique
    /// replacement where it has one, otherwise the stock district. The engine
    /// blocks the base district for civilizations with a replacement, exactly
    /// as it does for unique units.
    pub(crate) fn civ_district(g: &Game, pid: usize, family: &str) -> Name {
        let civ = g.players[pid].civ.as_str();
        g.rules
            .districts
            .iter()
            .find(|(_, spec)| {
                spec.replaces == Some(Name::new(family)) && spec.unique_to.as_deref() == Some(civ)
            })
            .map(|(name, _)| *name)
            .unwrap_or_else(|| Name::new(family))
    }

    /// The building this city should start in place of `family`: the stock
    /// building where it is available, otherwise whichever replacement this
    /// civilization or its secret society builds instead. `None` means the
    /// city already has one or cannot have it, so the caller moves on rather
    /// than proposing something the engine will refuse.
    /// Turns this city needs to train `unit` from scratch at its current
    /// production.
    fn unit_build_turns(g: &Game, pid: usize, cid: u32, unit: &str) -> f64 {
        let item = Item::Unit {
            unit: Name::new(unit),
        };
        g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.1)
    }

    /// The slowest a city may build its first Campus and still be asked under
    /// `campus-before-harbor`: a one-production outpost waits for the ordinary
    /// list rather than parking forty turns on a district.
    const FIRST_CAMPUS_MAX_TURNS: f64 = 15.0;

    /// Cities the empire holds before `capital_campus_first` puts the
    /// capital's Campus ahead of its next Settler: the pump has founded two
    /// others, so the land grab keeps a second and third source of walkers.
    const CAPITAL_CAMPUS_MIN_CITIES: usize = 3;

    /// The first Theater Square (or its replacement, Greece's Acropolis) on
    /// its best site; see `first_district_item`. `culture-seat-theater-first`.
    pub(crate) fn first_theater_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        Self::first_district_item(g, pid, cid, "theater_square")
    }

    /// This city's first district of `family` (its civilization's
    /// replacement where it has one) on the site with the highest total
    /// yield, when the city would finish it within `FIRST_CAMPUS_MAX_TURNS`.
    fn first_district_item(g: &Game, pid: usize, cid: u32, family: &str) -> Option<Item> {
        Self::first_district_item_with_budget(g, pid, cid, family, Self::FIRST_CAMPUS_MAX_TURNS)
    }

    fn first_district_item_with_budget(
        g: &Game,
        pid: usize,
        cid: u32,
        family: &str,
        max_turns: f64,
    ) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        if g.city_has_district_family(city, Name::new(family)) {
            return None;
        }
        let dname = Self::civ_district(g, pid, family);
        let spec = g.rules.districts.get(&dname)?;
        let unlocked = spec
            .tech
            .as_ref()
            .is_none_or(|tech| g.players[pid].techs.contains(tech))
            && spec
                .civic
                .as_ref()
                .is_none_or(|civic| g.players[pid].civics.contains(civic));
        if !unlocked {
            return None;
        }
        let best = g
            .district_sites(cid, Name::new(dname.as_str()))
            .into_iter()
            .max_by(|a, b| {
                g.district_site_yields(dname, *a)
                    .total()
                    .partial_cmp(&g.district_site_yields(dname, *b).total())
                    .unwrap()
                    .then(a.cmp(b))
            })?;
        let item = Item::District {
            district: dname,
            pos: best,
        };
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= max_turns).then_some(item)
    }

    /// A seat's Production a turn, read the way `seat_culture_per_turn` reads
    /// Culture: what its cities make, plus the host's correction for that
    /// seat, which carries a live rival's public Production total. Empty on a
    /// native board, where the cities are the whole reading.
    pub(crate) fn seat_production_per_turn(g: &Game, seat: usize) -> f64 {
        g.player_city_ids(seat)
            .into_iter()
            .map(|city| g.city_yields(city).production)
            .sum::<f64>()
            + g.observed_yield_adjustments
                .get(&seat)
                .map_or(0.0, |adjustment| adjustment.production)
    }

    fn civ_building(g: &Game, pid: usize, cid: u32, family: &str) -> Option<Item> {
        let base = Item::Building {
            building: Name::new(family),
        };
        if g.can_produce(pid, cid, &base) {
            return Some(base);
        }
        g.rules
            .buildings
            .iter()
            .filter(|(_, spec)| spec.replaces == Some(Name::new(family)))
            .map(|(name, _)| Item::Building { building: *name })
            .find(|item| g.can_produce(pid, cid, item))
    }

    /// Which improvement a tile should actually get. An improvement that
    /// matches the tile's resource comes first: it is the only way to work a
    /// strategic resource or connect a luxury, and paving Iron or Wine over
    /// with a Farm forfeits that permanently. Otherwise take the most
    /// valuable yield, weighted the way the rest of this AI values output.
    /// Whether an Amenity shortfall of `deficit` in a city holding `specialty`
    /// specialty districts asks for the repair district ahead of the lane: the
    /// deeper band, or a merely displeased city whose lane has been served.
    /// See the `amenity_districts` push in `pick_item`.
    pub(crate) fn amenity_repair_outranks_lane(deficit: f64, specialty: usize) -> bool {
        deficit >= AMENITY_DISTRICT_FIRST_BAND
            || (deficit > 0.0 && specialty >= AMENITY_DISTRICT_LANE_SERVED)
    }

    pub(crate) fn military_engineer_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        let target = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                let position = g.district_contribution_target(pid, city)?;
                if position != current {
                    g.route_step(uid, position, 0)?;
                }
                Some((g.wdist(current, position), position, city))
            })
            .min();
        if let Some((_, position, city)) = target {
            if current == position && g.can_contribute_district(pid, uid, city) {
                return g
                    .apply(pid, &Action::ContributeDistrict { unit: uid, city })
                    .is_ok();
            }
            return self.step_toward(g, pid, uid, position);
        }

        // Once Steam Power arrives, connect city centers with a continuous
        // Railroad. The current tile is laid before movement, then the next
        // turn continues toward the nearest center that is not connected yet.
        // District contributions remain the higher priority above: finishing
        // a Dam, Aqueduct, or Canal is worth an Engineer charge immediately.
        let has_rail_material = g.strategic_stockpile(pid, crate::name!("iron"))
            >= RAILROAD_RESOURCE_RESERVE + 1.0
            && g.strategic_stockpile(pid, crate::name!("coal")) >= RAILROAD_RESOURCE_RESERVE + 1.0;
        let railroad_target = has_rail_material
            .then(|| {
                g.player_city_ids(pid)
                    .into_iter()
                    .map(|city| g.cities[&city].pos)
                    .filter(|position| g.map.tiles[position].road < 5)
                    .filter(|position| {
                        *position == current || g.route_step(uid, *position, 0).is_some()
                    })
                    .min_by_key(|position| (g.wdist(current, *position), *position))
            })
            .flatten();
        if let Some(position) = railroad_target {
            if g.can_build_railroad(pid, uid) {
                return g.apply(pid, &Action::BuildRailroad { unit: uid }).is_ok();
            }
            if current != position {
                return self.step_toward(g, pid, uid, position);
            }
        }
        self.military_step(g, pid, uid)
    }

    fn naturalist_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if g.valid_improvements(pid, current)
            .iter()
            .any(|improvement| improvement == "national_park")
        {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: crate::name!("national_park"),
                    },
                )
                .is_ok();
        }
        let target = g
            .national_park_sites(pid)
            .into_iter()
            .filter_map(|site| {
                let appeal = site
                    .iter()
                    .map(|position| g.tile_appeal(*position).max(0))
                    .sum::<i32>();
                site.into_iter()
                    .filter(|position| g.rules.is_passable(&g.map.tiles[position]))
                    .filter(|position| g.route_step(uid, *position, 0).is_some())
                    .min_by_key(|position| (g.wdist(current, *position), *position))
                    .map(|position| {
                        (
                            appeal,
                            std::cmp::Reverse(g.wdist(current, position)),
                            position,
                        )
                    })
            })
            .max();
        target.is_some_and(|(_, _, position)| self.step_toward(g, pid, uid, position))
    }

    fn archaeologist_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if let Some(improvement) = g
            .valid_improvements(pid, current)
            .into_iter()
            .find(|name| matches!(name.as_str(), "archaeological_dig" | "shipwreck_excavation"))
        {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(&improvement),
                    },
                )
                .is_ok();
        }
        let target = {
            let _memo = g.query_memo();
            g.excavation_sites(pid)
                .into_iter()
                .filter(|(position, _)| g.route_step(uid, *position, 0).is_some())
                .min_by_key(|(position, improvement)| {
                    (
                        g.wdist(current, *position),
                        improvement == "shipwreck_excavation",
                        *position,
                    )
                })
                .map(|(position, _)| position)
        };
        target.is_some_and(|position| self.step_toward(g, pid, uid, position))
    }

    /// Chess-style static exchange evaluation: expected damage traded if we
    /// attack `pos` (combat model: 30·e^((att−def)/25), sans rng).
    fn exchange_score(&self, g: &Game, uid: u32, pos: Pos, ranged: bool) -> f64 {
        let u = &g.units[&uid];
        let att = effective_strength(g.unit_strength(u, false), u.hp);
        if let Some(cid) = g.city_at(pos) {
            let c = &g.cities[&cid];
            if c.owner != u.owner {
                // cities: press wounded ones, big bonus on a capturable one
                let mut s = 20.0 + 0.5 * (100 - c.hp) as f64;
                if !ranged && c.hp <= 40 && c.wall_hp <= 0 {
                    s += self.w.kill_bonus;
                }
                return s;
            }
        }
        let defender = g
            .unit_ids_at(pos)
            .iter()
            .map(|oid| &g.units[oid])
            .filter(|o| g.rules.units[o.kind].class == "military")
            .max_by(|a, b| {
                effective_strength(g.unit_strength(a, true), a.hp)
                    .partial_cmp(&effective_strength(g.unit_strength(b, true), b.hp))
                    .unwrap()
            });
        let o = match defender {
            None => return 15.0 + self.w.kill_bonus * 0.5, // undefended civilians
            Some(o) => o,
        };
        let def = effective_strength(g.unit_strength(o, true), o.hp);
        // `exchange-is-the-engines`: the two blows `do_attack`/`do_ranged`
        // will actually resolve, matchup and ground and support included.
        // `None` with the gene off, and then every line below is the
        // arithmetic this function has always used. See
        // `advanced/engine_pricing.rs`.
        let engine = self.engine_exchange(g, uid, o.id, pos, ranged);
        let deal = match &engine {
            Some((dealt, _)) => *dealt,
            None => 30.0 * ((att - def) / 25.0).exp(),
        };
        let mut s = deal.min(o.hp as f64);
        if deal >= o.hp as f64 {
            s += self.w.kill_bonus;
        } else if !ranged {
            let recv = match &engine {
                Some((_, returned)) => *returned,
                None => {
                    let their_att = effective_strength(g.unit_strength(o, false), o.hp);
                    let my_def = effective_strength(g.unit_strength(u, true), u.hp);
                    30.0 * ((their_att - my_def) / 25.0).exp()
                }
            };
            s -= self.w.trade_caution * recv.min(u.hp as f64);
            if recv >= u.hp as f64 {
                s -= 35.0; // don't suicide into a counter
            }
        }
        // Even trades against barbarians are worth taking: civs heal at home
        // while raiders respawn from camps, and a mirror matchup would
        // otherwise score exactly 0 and stall at the attack floor.
        if !self.barb && g.players[o.owner].is_barbarian {
            s += 10.0;
        }
        s
    }

    fn nearest_enemy_from(
        &self,
        g: &Game,
        _pid: usize,
        pos: Pos,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        g.cities
            .values()
            .filter(|city| enemy_ids.contains(&city.owner))
            .map(|city| (g.wdist(pos, city.pos), city.pos))
            .chain(
                g.units
                    .values()
                    .filter(|unit| enemy_ids.contains(&unit.owner))
                    .map(|unit| (g.wdist(pos, unit.pos), unit.pos)),
            )
            .min()
            .map(|(_, target)| target)
    }

    /// Retire one exploration target that the Firaxis host has shown it cannot
    /// turn into discovery. The target's ring goes with it because a blocked
    /// frontier is rarely one plot wide, and every destination cache is cleared
    /// so the next exploration step must choose a genuinely different route.
    fn retire_exploration_target(&self, g: &Game, uid: u32, target: Pos) {
        let mut dead = self.explore_dead.borrow_mut();
        let unit_dead = dead.entry(uid).or_default();
        unit_dead.retain(|_, expiry| *expiry > g.turn);
        let expiry = g.turn + EXPLORE_DEAD_TARGET_TURNS;
        unit_dead.insert(target, expiry);
        for neighbour in g.nbrs(target) {
            unit_dead.insert(neighbour, expiry);
        }
        drop(dead);
        self.explore_last.borrow_mut().remove(&uid);
        self.explore_goal.borrow_mut().remove(&uid);
    }

    fn patrol_tile(&self, g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        let sea_unit = g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea");
        let water = g.rules.is_water(tile);
        if sea_unit != water {
            return false;
        }
        let friendly_city = tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == pid);
        if !friendly_city {
            return false;
        }
        g.unit_can_traverse(uid, pos)
    }

    fn build_patrol_posts(&self, g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        let mut posts: Vec<Pos> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| self.patrol_tile(g, pid, uid, *pos))
            .filter(|pos| {
                // A frontier post borders land or water outside this empire.
                // Interior city centers remain fallback destinations below.
                g.nbrs(*pos).into_iter().any(|neighbor| {
                    g.map.get(neighbor).is_some_and(|tile| {
                        tile.owner_city
                            .and_then(|cid| g.cities.get(&cid))
                            .is_none_or(|city| city.owner != pid)
                    })
                })
            })
            .collect();
        if posts.is_empty() {
            posts = g
                .player_city_ids(pid)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .filter(|pos| self.patrol_tile(g, pid, uid, *pos))
                .collect();
        }
        posts.sort_unstable();
        posts.dedup();
        posts
    }

    fn patrol_posts_for(
        &mut self,
        g: &Game,
        pid: usize,
        uid: u32,
        domain: &str,
        class_specific: bool,
    ) -> Vec<Pos> {
        let post_memo = g.query_memo();
        let class_key = (domain.to_string(), g.traversal_class(uid));
        let mut posts = if let Some(posts) = self.patrol_posts_by_class.get(&class_key) {
            posts.clone()
        } else if !class_specific {
            if let Some(posts) = self.patrol_posts.get(domain) {
                posts.clone()
            } else {
                let posts = self.build_patrol_posts(g, pid, uid);
                self.patrol_posts.insert(domain.to_string(), posts.clone());
                posts
            }
        } else {
            let posts = self.build_patrol_posts(g, pid, uid);
            self.patrol_posts_by_class.insert(class_key, posts.clone());
            posts
        };
        // A conquest earlier in this same unit phase may have invalidated a
        // cached frontier tile. Keep the shared scan, but cheaply validate the
        // relatively small candidate list before routing to it.
        posts.retain(|pos| self.patrol_tile(g, pid, uid, *pos));
        drop(post_memo);
        posts
    }

    /// Populate the frontier scan before AdvancedAi clones a planner into
    /// parallel unit snapshots. Those snapshots are intentionally independent,
    /// so a lazy cache would otherwise pay for the same map scan once per
    /// branch. BasicAi keeps its ordinary lazy behavior when this hook is not
    /// used.
    pub(crate) fn prepare_patrol_posts(&mut self, g: &Game, pid: usize, uids: &[u32]) {
        let mut representatives: Vec<(String, TraversalClass, u32)> = Vec::new();
        for uid in uids {
            let Some(unit) = g.units.get(uid) else {
                continue;
            };
            if g.rules.units[unit.kind].class != "military" {
                continue;
            }
            let domain = g.rules.units[unit.kind]
                .domain
                .as_deref()
                .unwrap_or("land")
                .to_string();
            let class = g.traversal_class(*uid);
            if !representatives
                .iter()
                .any(|(old_domain, old_class, _)| *old_class == class && old_domain == &domain)
            {
                representatives.push((domain, class, *uid));
            }
        }
        for (domain, _, uid) in representatives {
            let _ = self.patrol_posts_for(g, pid, uid, &domain, true);
        }
    }

    /// End a unit's turn: fortify if the engine will take the order, and
    /// otherwise simply stop. Every controller ends a unit here, including
    /// civilians and embarked units, and the fortify order used to be issued
    /// for all of them — `Game::unit_can_fortify` refuses everything that is
    /// not an unembarked land military unit, so a Settler walking to its site
    /// spent one refused order per re-entry, up to eight a turn. Asking the
    /// engine's own predicate first changes no game state: the order it
    /// replaces was rejected before it could change any.
    pub(crate) fn fortify_or_stop(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if !g.units[&uid].fortified && g.unit_can_fortify(&g.units[&uid]) {
            let _ = g.apply(pid, &Action::Fortify { unit: uid });
        }
        false
    }
}

#[cfg(test)]
mod military_purchase_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod amenity_district_tests;

/// The Apostle promotion treatment: see
/// [`BasicAi::apostle_promotion_by_role`].
#[cfg(test)]
mod apostle_promotion_tests;

#[cfg(test)]
mod attack_envelope_key_tests;

#[cfg(test)]
mod recovery_project_tests;

#[cfg(test)]
mod opening_defense_tests;

#[cfg(test)]
mod exploration_replan_tests;

#[cfg(test)]
mod civilian_pursuit_tests;

#[cfg(test)]
mod settlement_site_tests;

#[cfg(test)]
mod rock_band_tour_tests;
