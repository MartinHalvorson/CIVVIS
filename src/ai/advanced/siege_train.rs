//! Siege train and anvil: the two doctrines of a force whose objective is a
//! city — an enemy city to take (`siege-train`) or a city of ours to hold
//! (`anvil`). Two opt-in genes, one module, because both are the same
//! shape: a formation stated around a city tile, kept across turns, that
//! the per-unit ladder and the group mover otherwise never form.
//!
//! **What the arena found** (`docs/DOCTRINE_ARENA.md`, "The arena can pose a
//! siege"): on `the_storming` the deployed controller feeds a 520-material
//! siege train to a 165-material garrison over eleven turns of arrival
//! spread, takes the city three times in forty assaults, and loses the
//! position. `rush_siege_step` is the only ring seal in the tree and it is
//! gated on `plan.rush`; the group mover's role spacing puts shooters at
//! their range and melee at one, but nothing decides *when* the train
//! closes, *which* tiles seal the ring, *what* each arm shoots, or *who*
//! walks in. The live record is the same shape at two hundred turns.
//!
//! # `siege-train`
//!
//! A state machine per objective city, keyed by the city's id because force
//! groups are rebuilt every turn and a group's id is its lowest unit:
//!
//! - **Stage.** The train gathers on the staging ring — [`STAGING_NEAR`] to
//!   [`STAGING_FAR`] tiles out, never inside the City Center's own strike
//!   reach ([`CITY_STRIKE_RANGE`]) — until the strength standing there meets
//!   the bill: the defenders within [`DEFENDER_RADIUS`], the city's strength
//!   and its walls at [`WALL_STRENGTH_PER_100_HP`] a hundred, times
//!   [`BILL_MARGIN`]. A unit in the city's reach steps back out; a unit far
//!   off marches to the ring. Relievers that come out are fought on the
//!   exact forward model, as everywhere else in this module. On an arena the
//!   gate is arrival alone — the whole force within reach of the ring —
//!   because no reinforcement is coming and the shipped posture ladder makes
//!   the same exception for the same reason.
//! - **Invest.** Melee take ring tiles in `rush_siege_step`'s spread-first
//!   order — a zone of control covers a ring tile and both its ring
//!   neighbours, so two units three apart seal what two side by side do
//!   not — generalised to any at-war city. Siege units take a tile at their
//!   range behind a ring unit; shooters the same. The ring is sealed when
//!   every passable neighbour is held or in our zone of control, which is
//!   `Game::city_under_siege`'s own test and the condition under which the
//!   city stops healing twenty a turn.
//! - **Reduce.** Siege units shoot the city — walls first by the engine's
//!   own routing, then the garrison — unless a reliever within
//!   [`RELIEVER_RADIUS`] of the city can be killed with [`KILL_MARGIN`].
//!   Shooters kill a reliever if they can, shoot units while the wall
//!   stands, and turn on the city once it is down. Melee on the ring hold it
//!   and fortify: a swing at a wall above [`MELEE_WALL_FRACTION`] of its
//!   pool lands fifteen percent on the wall and one point on the city, and
//!   costs a return blow, so it is refused unless a ram or tower stands
//!   beside the city.
//! - **Take.** One melee-capable unit is the taker — the one adjacent to
//!   the city (or one move from it) with the most movement — and it is
//!   reserved: excluded from every other blow and move, and published
//!   through `reserved_units` so a joint planner can leave it alone. When
//!   the city's hit points are within the taker's expected blow — the
//!   engine's melee arithmetic against `city_strength`, routed through the
//!   wall pool the way `city_take_damage` routes it — the taker attacks, and
//!   the attack that reduces the city is the capture.
//! - **Hold.** After the capture the ladder's own `occupation_garrison_target`
//!   seats one unit; everyone else is released to a group whose objective
//!   has moved on.
//!
//! The train falls back to Stage when its strength drops under
//! [`ABORT_SHARE`] of the bill. Every turn writes one "Military/Decision"
//! line per siege and the census counts turns by stage, sealed rings and
//! captures.
//!
//! # `anvil`
//!
//! For `plan.threatened_city`, the land group nearest it holds the city as
//! a formation instead of the relief hold point: a ranged unit on the City
//! Center (the garrison bonus and the city's own strike), melee on the two
//! or three adjacent tiles that face the enemy with the best
//! `tile_defense_bonus`, everyone else within two so the city strike joins
//! their fight, and never zero units adjacent while a hostile stands within
//! [`ANVIL_HOSTILE_RADIUS`]. A unit under [`ANVIL_ROTATE_HP`] rotates into
//! the city to heal, trading places by `Action::Swap` with the fresh unit
//! standing there, which takes its tile. The formation engages relievers
//! only when the exchange favours it: a shot has no return, and a melee
//! blow is taken when the engine's own pair says it deals more than it
//! takes.
//!
//! `Kind::OptIn`, both off in `AdvancedAi::new()` and `legacy()`,
//! byte-identical when off: `siege_doctrine_step` returns before it reads
//! the board. Priced on the arena first (`doctrine_arena` on
//! `the_storming` and `the_relief`); the whole-game screen is the no-harm
//! check (`docs/DOCTRINE_ARENA.md`, "The gate for a tactical gene").

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{
    AdvancedAi, AppliedAttack, ForceDomain, ForceGroup, StrategicPlan, THREAT_RELIEF_RADIUS,
};
use crate::game::{effective_strength, expected_damage, Action, Game};
use crate::think;
use crate::Pos;

/// The staging ring: this far from the city while the train gathers.
pub(super) const STAGING_NEAR: i32 = 3;
pub(super) const STAGING_FAR: i32 = 5;

/// `declaration-waits-for-the-breaker`: the standard turns a staged
/// declaration holds for a breaker on one objective before it goes ahead.
pub(super) const DECLARATION_BREAKER_PATIENCE: u32 = 10;
/// A City Center strikes this far; nothing stands inside it before the
/// train is staged.
pub(super) const CITY_STRIKE_RANGE: i32 = 2;
/// A gun staging without a firing post must survive several enemy replies.
/// One-turn lethal checks let a hostile city wear it down before it arrives.
const STAGING_GUN_REPLY_TURNS: f64 = 3.0;
const STAGING_GUN_HP_RESERVE: f64 = 20.0;
/// `staging-gun-trusts-its-escort`: a gun with at least this many of our
/// land soldiers on or beside its next marching step budgets one reply turn
/// of danger there, not [`STAGING_GUN_REPLY_TURNS`]. A raider that strikes
/// the gun meets the escort the next turn. Budgeting three replies, a
/// catapult refuses any tile one enemy archer reaches (a single blow near
/// 60 against a limit near 27), so near a defended capital the breakers never
/// reach the staging ring: live King civvis-20261004T033533Z (game 46) held
/// Babylon's catapults six to ten tiles out from turn 135 to 153 with
/// "damage ready false"; civvis-20261004T040138Z (game 47) held Quebec
/// City's at ten.
pub(super) const STAGING_ESCORT_BODIES: usize = 2;
/// `staging-gun-remembers-hostiles`: a sighting this many turns old or newer
/// still prices a hostile on the Stage march. Older sightings scatter too
/// widely to steer a gun by: a Cuirassier seen two turns ago reaches twelve
/// tiles under the rule below.
const REMEMBERED_STRIKER_TURNS: u32 = 1;

/// `staging-gun-remembers-hostiles`: a hostile the seat saw this turn or last
/// and can no longer see. It may stand anywhere within its movement for every
/// turn since the sighting, and strike one more movement on: `reach` from
/// `from`. `blow` is its full-health melee blow on the gun asked about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RememberedStriker {
    pub(super) from: Pos,
    pub(super) reach: i32,
    pub(super) blow: f64,
}
/// The bill is the defence within [`DEFENDER_RADIUS`] plus the city and its
/// walls at [`WALL_STRENGTH_PER_100_HP`] a hundred, times this.
pub(super) const BILL_MARGIN: f64 = 1.25;
pub(super) const DEFENDER_RADIUS: i32 = 6;
pub(super) const WALL_STRENGTH_PER_100_HP: f64 = 10.0;
/// Under this share of the bill the train falls back to the staging ring.
pub(super) const ABORT_SHARE: f64 = 0.8;
/// `siege-holds-a-breach`: once the walls stand at or under this share of
/// their full strength, the train falls back only under
/// [`HELD_BREACH_ABORT_SHARE`] of the bill. Live King
/// civvis-20261003T103619Z (game 32) had Tskhumi's 400 walls down to 276 at
/// turn 162 with four engines at range; the train dropped to Stage, the
/// engines walked back five to nine tiles, and the walls stood at 400 again
/// by 170 (diagnosed by -60).
pub(super) const HELD_BREACH_WALL_SHARE: f64 = 0.75;
pub(super) const HELD_BREACH_ABORT_SHARE: f64 = 0.5;
/// The health a spotter needs before it steps into sight of an unseen city.
const SPOTTER_MIN_HP: i32 = 60;
/// Consecutive short assessments before an invested train falls back. The
/// bill counts every visible defender within [`DEFENDER_RADIUS`], so one unit
/// walking into or out of sight swings it. Live King civvis-20261001T024402Z,
/// unwalled Xanadu: bills of 51, 128, 181, 161, 54, 158 between turns 41 and
/// 61 against a force of 74-144. Each one-turn dip dropped the train to Stage
/// and restarted [`INVEST_PATIENCE`], so it never reduced, and the city was
/// never damaged in 25 turns. The damage budget is as noisy: around walled
/// Tushpa (civvis-20261001T042554Z, turns 195-198) the force's endurance read
/// 264, 10, 312 and 12 on consecutive turns, and every low reading dropped
/// the train out of Invest.
pub(super) const ABORT_PATIENCE: u32 = 2;
/// Melee holds the ring rather than swinging at a wall above this fraction
/// of its pool, unless a ram or tower stands beside the city.
pub(super) const MELEE_WALL_FRACTION: f64 = 0.2;
/// `breach-assault-closes-in`: how far off the ring a healthy melee unit is
/// called in from.
pub(super) const CLOSING_REACH: i32 = 4;
/// `breach-assault`: a melee unit joins the assault only from this health.
pub(super) const ASSAULT_MIN_HP: i32 = 60;
/// ... and only when it keeps at least this much after the city's reply.
pub(super) const ASSAULT_SURVIVOR_HP: i32 = 25;
/// `breach-assault`: the assault opens when the force's blows can take the
/// walls and the city within this many turns ...
pub(super) const ASSAULT_TURNS: f64 = 2.0;
/// ... counting one turn of the city's heal.
pub(super) const ASSAULT_HEAL: f64 = 20.0;
/// A hostile this close to the city is a reliever at the ring.
pub(super) const RELIEVER_RADIUS: i32 = 3;
/// Expected damage over hit points before a shot is counted as a kill: the
/// engine's roll is uniform on 0.8–1.2 of the centre.
pub(super) const KILL_MARGIN: f64 = 1.15;
/// Turns of an unsealed ring before the train reduces anyway, provided a
/// shooter is already in range.
pub(super) const INVEST_PATIENCE: u32 = 3;
/// A siege record nobody has assessed for this many turns is dropped.
const SIEGE_MEMORY: u32 = 3;
/// `anvil`: a defender under this rotates into the city to heal, if the
/// unit standing there is healthier by the margin.
pub(super) const ANVIL_ROTATE_HP: i32 = 50;
pub(super) const ANVIL_RELIEF_MARGIN: i32 = 25;
/// A hostile within this of the city keeps the ring manned.
pub(super) const ANVIL_HOSTILE_RADIUS: i32 = 6;
/// Front tiles the anvil mans, at most.
const ANVIL_FRONT_TILES: usize = 3;
/// A group farther than this from the objective is not on it.
const OBJECTIVE_REACH: i32 = 8;
/// `siege-needs-a-breaker`: shooters alone are the breaker when their
/// expected wall damage brings the walls down within this many turns. Live
/// King civvis-20261003T135713Z: Archers took Kwadukuza's Ancient Walls from
/// 100 to 26 in a dozen turns at about ten a shot, while Crossbows against its
/// Medieval Walls did five or six a shot and 200 walls stood.
pub(super) const SHOOTER_BREACH_TURNS: f64 = 6.0;
/// `siege-needs-a-breaker`: a train at least this many times its bill
/// gives its shooters [`SHOOTER_BREACH_TURNS_DOMINANT`] to breach. Live King
/// civvis-20261004T025448Z (game 45) outgunned Nubia four to seven times and
/// held its walled sieges for a gun that never came.
pub(super) const DOMINANT_BILL_SHARE: f64 = 2.5;
pub(super) const SHOOTER_BREACH_TURNS_DOMINANT: f64 = 12.0;
/// `siege-needs-a-breaker`: how far a ram or tower walks to join a siege.
const BREACH_SUPPORT_REACH: i32 = 15;
/// `siege-needs-a-breaker`: a gun this close to a walled city, or a city of
/// ours this close building one, is a breaker on its way. The reach
/// `domination_siege_train_mobilizing` already reads a walled target's
/// train by.
const BREAKER_COMING_REACH: i32 = 24;
/// `siege-needs-a-breaker`: standard turns a capture may wait on a breaker
/// before the ledger's stand-down clocks run again. Live King
/// civvis-20261003T145118Z (game 41)'s guns took about sixteen Online turns
/// to reach a walled siege; thirty standard turns is twenty Online ones.
pub(super) const BREAKER_WAIT_TURNS: u32 = 30;
/// `siege-needs-a-breaker`: standard turns a breaker on its way may go
/// without coming nearer before it no longer counts as coming. Live King
/// civvis-20261004T025448Z (game 45): a lone Catapult hovered eight to
/// fourteen tiles from Napata from turn 90 to 105, struck to 26 hp, while
/// the capture was held "for a wall-breaker on its way" every turn.
pub(super) const BREAKER_STALL_TURNS: u32 = 5;
/// `siege-needs-a-breaker`: a gun in production counts as coming only when
/// it finishes within this many standard turns.
pub(super) const BREAKER_BUILD_TURNS: u32 = 8;

/// `siege-needs-a-breaker`: one run of breakerless holds before a walled
/// city, kept by the city's position (stable through a live rebuild's id
/// churn). See `AdvancedAi::waiting_for_a_breaker`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BreakerWait {
    /// The first and the latest turn of the run.
    pub(super) since: u32,
    pub(super) last: u32,
    /// The nearest a siege gun, ram or tower of ours has stood to the city
    /// during the run, and the turn it first stood that near.
    pub(super) nearest: i32,
    pub(super) nearest_turn: u32,
}

/// A ranked choice of tile: the best key seen so far, and where it was.
type Pick<K> = Option<(K, Pos)>;
/// Stage's back-off tile: distance from the city (capped), friends beside
/// it, then the tile itself.
type BackOffKey = (i32, i32, Reverse<Pos>);
/// A shot: a kill first, then the damage, the lower hit points, the tile.
type ShotKey = (bool, i64, Reverse<i32>, Reverse<Pos>);
/// A melee blow: a kill first, then the margin dealt over taken, the tile.
type BlowKey = (bool, i64, Reverse<Pos>);

/// Where a siege stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SiegeStage {
    Stage,
    Invest,
    Reduce,
    Take,
    Hold,
}

impl SiegeStage {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            SiegeStage::Stage => "stage",
            SiegeStage::Invest => "invest",
            SiegeStage::Reduce => "reduce",
            SiegeStage::Take => "take",
            SiegeStage::Hold => "hold",
        }
    }
}

/// One siege, kept across turns on the controller and keyed by the city.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Siege {
    pub(super) stage: SiegeStage,
    /// The reserved melee unit that walks in.
    pub(super) taker: Option<u32>,
    /// The turn the stage was entered.
    pub(super) entered: u32,
    /// The turn the record was last assessed; once a turn.
    pub(super) assessed: u32,
    /// Every unit's post for the turn — a ring tile for melee, a firing tile
    /// for guns and shooters — drawn once at assessment so a unit's goal does
    /// not re-rank under it as it walks and two units never chase one tile.
    pub(super) posts: BTreeMap<u32, Pos>,
    /// The first turn of the current run of assessments that found the
    /// force short of the abort share or out of damage budget; `None` when
    /// the last assessment was not short. See [`ABORT_PATIENCE`].
    pub(super) short_since: Option<u32>,
}

/// The few facts about a city the doctrine reads, copied out so a step can
/// hold them while it mutates the board.
#[derive(Clone, Copy, Debug)]
struct CityView {
    id: u32,
    pos: Pos,
    owner: usize,
    hp: i32,
    wall_hp: i32,
    wall_max: i32,
}

impl CityView {
    fn of(g: &Game, cid: u32) -> Option<Self> {
        let city = g.cities.get(&cid)?;
        Some(CityView {
            id: cid,
            pos: city.pos,
            owner: city.owner,
            hp: city.hp,
            wall_hp: city.wall_hp.max(0),
            wall_max: g.city_max_wall_hp(city).max(0),
        })
    }

    fn wall_fraction(&self) -> f64 {
        if self.wall_max <= 0 || self.wall_hp <= 0 {
            0.0
        } else {
            f64::from(self.wall_hp) / f64::from(self.wall_max)
        }
    }

    /// `city_take_damage`'s routing of one blow through the wall pool: one
    /// point behind a healthy wall, half through a damaged one, the whole
    /// blow once breached or bare — or past the wall with a siege tower.
    fn through(&self, blow: f64, bypass: bool) -> f64 {
        if bypass || self.wall_hp <= 0 || self.wall_max <= 0 {
            return blow;
        }
        let fraction = self.wall_fraction();
        if fraction >= 0.8 {
            1.0
        } else if fraction >= 0.2 {
            (blow / 2.0).floor()
        } else {
            blow
        }
    }
}

/// `siege-needs-a-breaker`: walls a melee blow opens — none at all, or at or
/// under [`MELEE_WALL_FRACTION`] of the pool.
fn walls_open_to_melee(city: &CityView) -> bool {
    city.wall_hp <= 0 || city.wall_max <= 0 || city.wall_fraction() <= MELEE_WALL_FRACTION
}

/// A Battering Ram or Siege Tower whose effect works on this city's walls
/// (`Game::city_allows_siege_support`: a ram against Ancient Walls only, a
/// tower against Ancient and Medieval, neither against Urban Defenses).
fn breach_support_works(g: &Game, uid: u32, cid: u32) -> bool {
    g.units.get(&uid).is_some_and(|unit| {
        matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
            && g.city_allows_siege_support(cid, unit.kind.as_str())
    })
}

/// A land siege gun: the Catapult's line, the units that break walls whole.
fn land_gun(g: &Game, kind: crate::name::Name) -> bool {
    let spec = &g.rules.units[kind];
    spec.class == "military"
        && spec.siege
        && spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

/// `siege-needs-a-breaker`: the nearest a siege gun, or a ram or tower that
/// opens these walls, of ours stands to the city within
/// [`BREAKER_COMING_REACH`]; `i32::MAX` with none.
fn nearest_breaker(g: &Game, pid: usize, city: &CityView) -> i32 {
    g.units
        .values()
        .filter(|unit| {
            unit.owner == pid
                && (land_gun(g, unit.kind) || breach_support_works(g, unit.id, city.id))
        })
        .map(|unit| g.wdist(unit.pos, city.pos))
        .filter(|distance| *distance <= BREAKER_COMING_REACH)
        .min()
        .unwrap_or(i32::MAX)
}

impl AdvancedAi {
    /// `declaration-waits-for-the-breaker`: whether the city a war is about
    /// to open on can be breached by what stands on its ring now — walls a
    /// melee blow opens, or a fit siege gun of ours, or a ram or tower that
    /// works on these walls, within [`STAGING_FAR`]. The war-policy verdict
    /// weighs only the strength staged on the ring, so the war opens with the
    /// breaker still on the road, and a city at war raises its next wall tier
    /// while the train holds for it. Live King civvis-20261005T053701Z
    /// (game 103) declared on the Maori at turn 74, 373 power against 39,
    /// with Opango behind 100 walls and its Catapult 17 tiles out; the walls
    /// stood at 200 by turn 78, the siege held "for a wall-breaker on its
    /// way" to turn 96, and the city fell at 118.
    pub(super) fn declaration_breaker_at_hand(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = CityView::of(g, cid) else {
            return true;
        };
        if walls_open_to_melee(&city) {
            return true;
        }
        g.units.values().any(|unit| {
            unit.owner == pid
                && g.wdist(unit.pos, city.pos) <= STAGING_FAR
                && !g.is_embarked(unit)
                && ((land_gun(g, unit.kind) && self.siege_member_fit(g, unit.id))
                    || breach_support_works(g, unit.id, city.id))
        })
    }

    /// `declaration-waits-for-the-breaker`: whether the hold on `objective`
    /// still has patience. The clock starts the first turn the hold applies
    /// (`holdable`) on that tile, runs through turns the army reads
    /// unstaged, and resets only when the breaker arrives or the objective
    /// moves. After [`DECLARATION_BREAKER_PATIENCE`] standard turns the
    /// declaration goes ahead without it, so a breaker stuck on its column
    /// cannot hold the war forever.
    pub(super) fn declaration_hold_patience(
        &mut self,
        g: &Game,
        objective: Option<Pos>,
        breaker_missing: bool,
        holdable: bool,
    ) -> bool {
        let Some(pos) = objective.filter(|_| breaker_missing) else {
            self.declaration_breaker_hold = None;
            return false;
        };
        if self
            .declaration_breaker_hold
            .is_none_or(|(held, _)| held != pos)
        {
            if !holdable {
                return false;
            }
            self.declaration_breaker_hold = Some((pos, g.turn));
        }
        holdable
            && self.declaration_breaker_hold.is_some_and(|(_, since)| {
                g.turn.saturating_sub(since) < g.standard_duration(DECLARATION_BREAKER_PATIENCE)
            })
    }
}

/// A melee unit a ram or tower lends its effect to: `siege_support_effects`
/// answers only for the melee and anti-cavalry classes.
fn breach_support_user(g: &Game, uid: u32) -> bool {
    g.units
        .get(&uid)
        .is_some_and(|unit| crate::ai::siege_support::eligible_attacker(&g.rules.units[unit.kind]))
}

/// The wall damage one shot of this ranged unit is expected to do to the
/// city, as `do_ranged` resolves it: its ranged strength at its hit points,
/// less 17 for a land ranged unit, against `city_strength`, and half of it on
/// the wall unless the unit is siege.
fn wall_damage_per_shot(g: &Game, uid: u32, cid: u32) -> f64 {
    let unit = &g.units[&uid];
    let spec = &g.rules.units[unit.kind];
    let mut base =
        g.unit_ranged_attack_strength(unit) + g.promotion_effect(unit, "ranged_vs_district");
    if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
        base -= 17.0;
    }
    let dealt = expected_damage(effective_strength(base, unit.hp), g.city_strength(cid));
    dealt * if spec.siege { 1.0 } else { 0.5 }
}

/// `siege-needs-a-breaker`: what the train holds, this turn, that can bring
/// the city's walls down. Melee hold a ring they cannot hurt: a swing at a
/// wall above [`MELEE_WALL_FRACTION`] is refused unless a ram or tower stands
/// beside the city, and the city's strike lands on them every turn. Live King
/// civvis-20261003T135713Z held Kwadukuza's ring with two Knights and three
/// Men-at-Arms from turn 113 to 122, walls 198/200, and read "damage ready"
/// off a Bombard at five tiles the battle planner was healing from 29 to 89;
/// the walls fell 15 points in those ten turns, all of it to a Crossbow.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct BreachReading {
    /// Siege guns within the staging ring fit to fire.
    pub(super) guns: usize,
    /// Siege guns within the staging ring the battle planner holds out to heal.
    pub(super) wounded_guns: usize,
    /// Rams and towers that work on these walls, within the staging ring or
    /// carried by a member, while a melee member can use one.
    pub(super) support: usize,
    /// The expected wall damage a turn from the fit shooters within reach.
    pub(super) shooter_walls: f64,
    /// Turns the shooters are given to breach: [`SHOOTER_BREACH_TURNS`], or
    /// [`SHOOTER_BREACH_TURNS_DOMINANT`] for a dominant train.
    pub(super) horizon: f64,
}

impl BreachReading {
    fn at_hand(&self, city: &CityView) -> bool {
        walls_open_to_melee(city)
            || self.guns > 0
            || self.support > 0
            || self.shooter_walls * self.horizon >= f64::from(city.wall_hp)
    }
}

/// The wall rule shared by the train's melee orders and its damage budget.
pub(super) fn melee_wall_attack_allowed(g: &Game, pid: usize, uid: u32, cid: u32) -> bool {
    let (Some(city), Some(unit)) = (CityView::of(g, cid), g.units.get(&uid)) else {
        return false;
    };
    let (ram, tower) = g.siege_support_effects(
        pid,
        cid,
        city.pos,
        &g.rules.units[unit.kind].promotion_class,
    );
    city.wall_fraction() <= MELEE_WALL_FRACTION || ram || tower
}

/// Which arm of the train a unit is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arm {
    Melee,
    Siege,
    Shooter,
    Other,
}

/// A combat carrier remains part of the army while its support follows.
/// Civilian and religious escort commitments stay outside combat allocation.
/// Validate both ends so a stale or separated mirror link cannot supply force.
pub(super) fn linked_support_carrier(g: &Game, uid: u32) -> bool {
    let Some(unit) = g.units.get(&uid) else {
        return false;
    };
    let spec = &g.rules.units[unit.kind];
    if spec.class != "military" || matches!(spec.domain.as_deref(), Some("sea" | "air")) {
        return false;
    }
    unit.linked_to
        .and_then(|peer| g.units.get(&peer))
        .is_some_and(|peer| {
            let support = &g.rules.units[peer.kind];
            peer.owner == unit.owner
                && peer.pos == unit.pos
                && peer.linked_to == Some(uid)
                && support.class == "support"
                && !matches!(support.domain.as_deref(), Some("sea" | "air"))
        })
}

fn arm_of(g: &Game, uid: u32) -> Arm {
    let Some(unit) = g.units.get(&uid) else {
        return Arm::Other;
    };
    let spec = &g.rules.units[unit.kind];
    if spec.class != "military"
        || matches!(spec.domain.as_deref(), Some("sea" | "air"))
        || (unit.linked_to.is_some() && !linked_support_carrier(g, uid))
        || g.is_embarked(unit)
    {
        return Arm::Other;
    }
    if spec.siege && spec.has_ranged_attack() {
        Arm::Siege
    } else if spec.has_ranged_attack() {
        Arm::Shooter
    } else if spec.is_melee_capable() {
        Arm::Melee
    } else {
        Arm::Other
    }
}

/// A unit's fighting weight: its defending strength at its hit points.
fn unit_power(g: &Game, uid: u32) -> f64 {
    let unit = &g.units[&uid];
    effective_strength(g.unit_strength(unit, true), unit.hp)
}

/// What the city asks of the force that takes it.
fn siege_bill(g: &Game, pid: usize, city: &CityView) -> f64 {
    let defenders: f64 = g
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.rules.units[unit.kind].class == "military"
                && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                && g.unit_visible_to(unit.id, pid)
                && g.wdist(unit.pos, city.pos) <= DEFENDER_RADIUS
        })
        .map(|unit| effective_strength(g.unit_strength(unit, true), unit.hp))
        .sum();
    let walls = f64::from(city.wall_hp) / 100.0 * WALL_STRENGTH_PER_100_HP;
    (defenders + g.city_strength(city.id) + walls) * BILL_MARGIN
}

/// How many of the city's passable neighbours are held or covered — the
/// test `Game::city_under_siege` applies, read from outside the engine: an
/// off-map or impassable side counts as sealed, an occupied tile is held,
/// and a tile in a besieger's zone of control is covered.
pub(super) fn ring_state(g: &Game, cid: u32) -> (usize, usize) {
    let Some(city) = g.cities.get(&cid) else {
        return (0, 0);
    };
    let mut sealed = 0;
    let mut total = 0;
    for pos in g.wdisk(city.pos, 1) {
        if pos == city.pos {
            continue;
        }
        total += 1;
        let Some(tile) = g.map.get(pos) else {
            sealed += 1;
            continue;
        };
        if !g.rules.is_passable(tile) {
            sealed += 1;
            continue;
        }
        let held = g.unit_ids_at(pos).iter().any(|id| {
            let unit = &g.units[id];
            unit.owner != city.owner
                && g.is_at_war(city.owner, unit.owner)
                && g.rules.units[unit.kind].class == "military"
        });
        if held || g.in_enemy_zoc(city.owner, pos) {
            sealed += 1;
        }
    }
    (sealed, total)
}

/// The taker's expected blow on the city as `do_attack` would land it: the
/// attacker's strength at its hit points against `city_strength`, at the
/// centre of the roll, routed through the wall pool.
pub(super) fn taker_blow(g: &Game, pid: usize, uid: u32, cid: u32) -> f64 {
    let (Some((att, defense)), Some(city)) = (
        g.city_melee_exchange_strengths(uid, cid),
        CityView::of(g, cid),
    ) else {
        return 0.0;
    };
    let mean = expected_damage(att, defense);
    let (_, tower) = g.siege_support_effects(
        pid,
        cid,
        city.pos,
        &g.rules.units[g.units[&uid].kind].promotion_class,
    );
    city.through(mean, tower)
}

/// The strongest hostile military unit on a tile — the defender the engine
/// resolves a blow there against. Units inside a City Center or Encampment
/// are not targets: a blow on that tile is a blow on the district.
fn strongest_hostile_at(g: &Game, pid: usize, pos: Pos) -> Option<u32> {
    if g.city_at(pos).is_some() || g.encampment_at(pos).is_some() {
        return None;
    }
    g.unit_ids_at(pos)
        .iter()
        .copied()
        .filter(|id| {
            let other = &g.units[id];
            other.owner != pid
                && g.is_at_war(pid, other.owner)
                && g.rules.units[other.kind].class == "military"
                && g.unit_visible_to(*id, pid)
        })
        .max_by(|a, b| {
            unit_power(g, *a)
                .total_cmp(&unit_power(g, *b))
                .then_with(|| b.cmp(a))
        })
}

/// Visible hostile military positions within `radius` of `center`.
fn hostiles_near(g: &Game, pid: usize, center: Pos, radius: i32) -> Vec<Pos> {
    let mut out: Vec<Pos> = g
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.rules.units[unit.kind].class == "military"
                && g.unit_visible_to(unit.id, pid)
                && g.wdist(unit.pos, center) <= radius
        })
        .map(|unit| unit.pos)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Tiles a hostile city or Encampment other than the target can strike: a
/// gun posted there is shot by a city the siege is not reducing. Live King
/// civvis-20261004T070716Z (game 49): Nagoya's catapults were posted within
/// reach of a levied city-state's walled district; its strikes and a
/// crossbow took one to 44 and then dead and another to 34, and Nagoya's
/// Invest landed one blow in six turns.
fn third_strike_sources(g: &Game, pid: usize, target: u32) -> Vec<Pos> {
    let mut out = Vec::new();
    for city in g.cities.values() {
        if city.id == target || city.owner == pid || !g.is_at_war(pid, city.owner) {
            continue;
        }
        if city.wall_hp > 0 {
            out.push(city.pos);
        }
        if city.encampment_hp > 0 && city.encampment_wall_hp > 0 && !city.encampment_pillaged {
            if let Some(at) = g
                .wdisk(city.pos, 3)
                .into_iter()
                .find(|pos| g.encampment_at(*pos) == Some(city.id))
            {
                out.push(at);
            }
        }
    }
    out
}

/// The melee-capable unit that walks in: adjacent to the city with the most
/// movement, else able to reach a free ring tile this turn.
fn designate_taker(g: &Game, city: &CityView, force: &[u32]) -> Option<u32> {
    let candidates: Vec<u32> = force
        .iter()
        .copied()
        .filter(|uid| {
            let unit = &g.units[uid];
            let arm = arm_of(g, *uid);
            // A hybrid keeps firing during the reduction. Once the city is
            // ready, its melee capability can finish a siege too; treating
            // every shooter as unable to capture stranded lone robots at 1 HP.
            let hybrid_finisher = arm == Arm::Shooter
                && g.rules.units[unit.kind].is_melee_capable()
                && unit.moves_left > 0.0
                && city.wall_hp <= 0
                && f64::from(city.hp) <= taker_blow(g, unit.owner, *uid, city.id);
            unit.attacks_left > 0 && (arm == Arm::Melee || hybrid_finisher)
        })
        .collect();
    let rank = |uid: &u32| {
        let unit = &g.units[uid];
        (
            (unit.moves_left * 100.0).round() as i64,
            (unit_power(g, *uid) * 100.0).round() as i64,
            Reverse(*uid),
        )
    };
    let adjacent = candidates
        .iter()
        .copied()
        .filter(|uid| g.wdist(g.units[uid].pos, city.pos) <= 1)
        .max_by_key(rank);
    if adjacent.is_some() {
        return adjacent;
    }
    let ring: Vec<Pos> = g
        .wdisk(city.pos, 1)
        .into_iter()
        .filter(|pos| *pos != city.pos && g.unit_ids_at(*pos).is_empty())
        .collect();
    candidates
        .iter()
        .copied()
        .filter(|uid| {
            let reach = g.reachable(*uid);
            ring.iter().any(|pos| reach.contains(pos))
        })
        .max_by_key(rank)
}

/// A breach can heal before a melee unit crosses the outer staging ring.
/// Choose a healthy assigned capturer with a short legal route to an adjacent
/// tile, even when it cannot complete that route this turn.
fn approaching_breach_taker(g: &Game, pid: usize, city: &CityView, force: &[u32]) -> Option<u32> {
    force
        .iter()
        .copied()
        .filter(|uid| {
            let unit = &g.units[uid];
            arm_of(g, *uid) == Arm::Melee
                && unit.hp >= 70
                && unit.attacks_left > 0
                && g.wdist(unit.pos, city.pos) <= STAGING_FAR
                && f64::from(city.hp) <= taker_blow(g, pid, *uid, city.id) + 40.0
                && g.route_distance(*uid, city.pos, 1)
                    .is_some_and(|steps| steps <= (STAGING_FAR + 3) as usize)
        })
        .min_by(|left, right| {
            g.wdist(g.units[left].pos, city.pos)
                .cmp(&g.wdist(g.units[right].pos, city.pos))
                .then_with(|| unit_power(g, *right).total_cmp(&unit_power(g, *left)))
                .then_with(|| left.cmp(right))
        })
}

/// `anvil`: every member's post for the turn. The city tile goes to the
/// most wounded member when the board heals, else to a ranged unit; the
/// fresh unit displaced from the city takes the wounded one's tile; melee
/// take the front tiles facing the enemy with the best defence; everyone
/// else stands within two; and the ring is never empty while a hostile is
/// in reach.
fn anvil_orders_for(
    g: &Game,
    pid: usize,
    city: &CityView,
    members: &[u32],
    hostiles: &[Pos],
    heals: bool,
) -> BTreeMap<u32, Pos> {
    let mut posts: BTreeMap<u32, Pos> = BTreeMap::new();
    let mut taken: BTreeSet<Pos> = BTreeSet::new();
    let mut land: Vec<u32> = members
        .iter()
        .copied()
        .filter(|uid| arm_of(g, *uid) != Arm::Other)
        .collect();
    land.sort_unstable();
    if land.is_empty() {
        return posts;
    }
    let hostile_distance = |pos: Pos| {
        hostiles
            .iter()
            .map(|h| g.wdist(*h, pos))
            .min()
            .unwrap_or(i32::MAX)
    };
    let defence = |pos: Pos| -(g.tile_defense_bonus(pos) * 10.0).round() as i32;
    let open = |pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            && g.unit_ids_at(pos).iter().all(|id| g.units[id].owner == pid)
    };
    let occupant = g
        .unit_ids_at(city.pos)
        .iter()
        .copied()
        .find(|id| land.contains(id));

    // 1. The city tile.
    let wounded = heals
        .then(|| {
            land.iter()
                .copied()
                .filter(|uid| g.units[uid].hp < ANVIL_ROTATE_HP)
                .min_by_key(|uid| (g.units[uid].hp, *uid))
        })
        .flatten();
    if let Some(w) = wounded {
        posts.insert(w, city.pos);
        taken.insert(city.pos);
        if let Some(c) = occupant.filter(|c| *c != w) {
            if g.units[&c].hp >= g.units[&w].hp + ANVIL_RELIEF_MARGIN {
                let relieved = g.units[&w].pos;
                posts.insert(c, relieved);
                taken.insert(relieved);
            }
        }
    } else {
        let garrison = land
            .iter()
            .copied()
            .filter(|uid| arm_of(g, *uid) == Arm::Shooter)
            .min_by_key(|uid| {
                let unit = &g.units[uid];
                (
                    unit.pos != city.pos,
                    g.wdist(unit.pos, city.pos),
                    Reverse(unit.hp),
                    *uid,
                )
            });
        if let Some(gid) = garrison {
            posts.insert(gid, city.pos);
            taken.insert(city.pos);
        }
    }

    // 2. The front: adjacent tiles facing the enemy, best ground first.
    let mut ring: Vec<Pos> = g
        .wdisk(city.pos, 1)
        .into_iter()
        .filter(|pos| *pos != city.pos && open(*pos))
        .collect();
    ring.sort_by_key(|pos| (hostile_distance(*pos), defence(*pos), *pos));
    let mut melee: Vec<u32> = land
        .iter()
        .copied()
        .filter(|uid| arm_of(g, *uid) == Arm::Melee && !posts.contains_key(uid))
        .collect();
    let front: Vec<Pos> = ring
        .iter()
        .copied()
        .filter(|pos| !taken.contains(pos))
        .take(ANVIL_FRONT_TILES.min(melee.len()))
        .collect();
    for tile in front {
        let Some(pick) = melee.iter().copied().min_by_key(|uid| {
            (
                g.wdist(g.units[uid].pos, tile),
                Reverse(g.units[uid].hp),
                *uid,
            )
        }) else {
            break;
        };
        melee.retain(|uid| *uid != pick);
        posts.insert(pick, tile);
        taken.insert(tile);
    }

    // 3. Everyone else within two, behind the front.
    let mut near: Vec<Pos> = g
        .wdisk(city.pos, 2)
        .into_iter()
        .filter(|pos| *pos != city.pos && open(*pos))
        .collect();
    near.sort_by_key(|pos| {
        (
            Reverse(hostile_distance(*pos)),
            defence(*pos),
            g.wdist(*pos, city.pos),
            *pos,
        )
    });
    let rest: Vec<u32> = land
        .iter()
        .copied()
        .filter(|uid| !posts.contains_key(uid))
        .collect();
    for uid in rest {
        let here = g.units[&uid].pos;
        if g.wdist(here, city.pos) <= 2 && here != city.pos && !taken.contains(&here) {
            posts.insert(uid, here);
            taken.insert(here);
            continue;
        }
        if let Some(tile) = near.iter().copied().find(|pos| !taken.contains(pos)) {
            posts.insert(uid, tile);
            taken.insert(tile);
        }
    }

    // 4. Never zero adjacent while a hostile is in reach.
    if !hostiles.is_empty() && !posts.values().any(|pos| g.wdist(*pos, city.pos) == 1) {
        if let Some(tile) = ring.iter().copied().find(|pos| !taken.contains(pos)) {
            let pick = land
                .iter()
                .copied()
                .filter(|uid| posts.get(uid) != Some(&city.pos))
                .min_by_key(|uid| (g.wdist(g.units[uid].pos, tile), *uid));
            if let Some(pick) = pick {
                if let Some(old) = posts.insert(pick, tile) {
                    taken.remove(&old);
                }
                taken.insert(tile);
            }
        }
    }
    posts
}

/// The same exclusions must govern post assignment and the march to it.
/// Otherwise spread-first assignment can reserve a pocket whose only entry
/// crosses another ring tile, and the mover can never fulfill that order.
/// How much longer, in steps, a dry-land march may be than the ordinary
/// route before a land unit takes the ordinary (embarking) one instead.
const DRY_MARCH_SLACK: usize = 8;

/// The next step of a land unit's march toward `to`: over dry land while a
/// dry route exists no more than twice the ordinary route's length (and
/// [`DRY_MARCH_SLACK`] over it), else the ordinary step. The ordinary router
/// prices every step at 1, so a unit that can embark crosses any shorter bay.
/// Live King civvis-20261002T054346Z: the Siege of Tlacopan, across a bay,
/// staged nobody within five tiles for 45 turns with a force of 7-19. Five to
/// twenty-six of our units floated in the bay at a time, 165 of their moves
/// failed with the host's "cannot_start", and units on the shore stood down as
/// "going nowhere", while a 23-step road ran around the bay over land.
pub(super) fn march_step(g: &Game, uid: u32, to: Pos, range: i32) -> Option<Pos> {
    dry_march_step(g, uid, to, range).or_else(|| g.route_step(uid, to, range))
}

/// `stage-march-keeps-to-land`: the longest dry road a Stage or approach
/// march takes instead of the water. Twice the 60-column map's width, so a
/// road round a strait or through a neighbour is in reach.
pub(super) const STAGE_DRY_LIMIT: usize = 128;

/// `stage-march-keeps-to-land`: how a land unit standing on land marches
/// when its ordinary route crosses water. [`march_step`] takes a dry road
/// only up to twice the ordinary route (and never past 64 steps); beyond
/// that it embarks, and `come-ashore`'s `disembark_step` lands the unit
/// again at home the next turn, before the train moves it, so the crossing
/// never completes. Live King civvis-20261005T045443Z (game 101): the guns
/// for Wak Kab'nal stood 14 to 18 tiles out from turn 150 to 227, the end of
/// the record, stepping into the coast and back each turn; the strait was 10
/// to 15 tiles and the land road round it 51 to 57.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StageMarch {
    /// The ordinary [`march_step`], unchanged.
    Ordinary,
    /// The first step of the dry road, `dry` steps long against the
    /// ordinary route's `wet`.
    Dry { step: Pos, dry: usize, wet: usize },
    /// No dry road within [`STAGE_DRY_LIMIT`]; the unit holds on land.
    Hold { wet: usize },
}

/// See [`StageMarch`]. `Ordinary` unless `uid` is a land unit on land
/// whose ordinary route to within `range` of `to` is shorter than every dry
/// road: an ordinary route that is already dry, and an unreachable goal,
/// keep [`march_step`]. A dry road inside `march_step`'s own limit yields
/// the same first step here, the search being the same breadth-first one.
pub(super) fn stage_march(g: &Game, uid: u32, to: Pos, range: i32) -> StageMarch {
    if !keeps_to_land(g, uid) || g.wdist(g.units[&uid].pos, to) <= range {
        return StageMarch::Ordinary;
    }
    let Some(wet) = g.route_distance(uid, to, range) else {
        return StageMarch::Ordinary;
    };
    match g.route_step_dry(uid, to, range, STAGE_DRY_LIMIT) {
        Some((_, dry)) if dry <= wet => StageMarch::Ordinary,
        Some((step, dry)) => StageMarch::Dry { step, dry, wet },
        None => StageMarch::Hold { wet },
    }
}

/// The dry-land half of [`march_step`]: `None` for a unit at sea or not on
/// land, when no acceptable dry route exists, or when the ordinary route is
/// no longer than the dry one (it already keeps to land).
pub(super) fn dry_march_step(g: &Game, uid: u32, to: Pos, range: i32) -> Option<Pos> {
    let unit = g.units.get(&uid)?;
    let spec = &g.rules.units[unit.kind];
    if g.is_embarked(unit)
        || spec
            .domain
            .as_deref()
            .is_some_and(|domain| domain != "land")
    {
        return None;
    }
    let ordinary_len = g.route_distance(uid, to, range).unwrap_or(usize::MAX / 4);
    let limit = ordinary_len
        .saturating_mul(2)
        .max(ordinary_len.saturating_add(DRY_MARCH_SLACK))
        .min(64);
    // A dry road no longer than the ordinary route means the ordinary route
    // keeps to land already: keep its step and its tie-breaking.
    g.route_step_dry(uid, to, range, limit)
        .filter(|(_, steps)| *steps > ordinary_len)
        .map(|(step, _)| step)
}

fn siege_route_step(g: &Game, pid: usize, uid: u32, goal: Pos, city: Pos) -> Option<Pos> {
    let unit = g.units.get(&uid)?;
    let mut avoid: BTreeSet<Pos> = g
        .wdisk(city, 1)
        .into_iter()
        .filter(|pos| *pos != goal)
        .collect();
    avoid.extend(
        g.units
            .values()
            .filter(|other| {
                other.id != uid
                    && g.rules.units[other.kind].domain.as_deref() != Some("air")
                    && (other.owner != pid || g.rules.units[other.kind].class == "military")
            })
            .map(|other| other.pos),
    );
    avoid.remove(&unit.pos);
    // A land unit on land keeps to land on its way to a post. Embarking
    // spends its moves, and the next turn `disembark_step` lands it again
    // before the train moves it, so a wet first step is a turn lost and the
    // next one too. Live King civvis-20261004T100903Z (game 52): with our
    // own soldiers walling the land approach, four Trebuchets before Ray
    // shuttled between (31,27) and the water at (32,27), five and four tiles
    // out, every other turn from turn 109 to 145, and none fired; the siege
    // ran seventy turns and Ray never fell.
    if keeps_to_land(g, uid) {
        avoid.extend(
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| **pos != goal && g.rules.is_water(tile))
                .map(|(pos, _)| *pos),
        );
    }
    g.route_step_avoiding_tiles(uid, goal, &avoid)
}

/// A land unit standing on land: the train's steps never embark it. See
/// [`siege_route_step`].
fn keeps_to_land(g: &Game, uid: u32) -> bool {
    g.units.get(&uid).is_some_and(|unit| {
        !g.is_embarked(unit)
            && g.rules.units[unit.kind]
                .domain
                .as_deref()
                .is_none_or(|domain| domain == "land")
    })
}

/// Whether `uid` may end a train step on `pos`: anywhere for a unit at sea
/// or not on land, dry ground for one on land.
fn dry_stand(g: &Game, uid: u32, pos: Pos) -> bool {
    !keeps_to_land(g, uid) || g.map.get(pos).is_some_and(|tile| !g.rules.is_water(tile))
}

/// The train's posts for the turn. Melee already on the ring keep their
/// tile; the taker, then the rest by distance, take reachable free ring tiles in the
/// spread-first order — the free tile furthest from every held or assigned
/// one, then the nearest. Guns, then shooters, keep a tile they can already
/// shoot the city from, else take a tile at their range behind a ring post
/// and away from hostiles. A unit with no tile left has no post.
/// The tile two out that a breached city's taker walks through to its ring,
/// kept clear of firing posts. A coastal city can have only two or three land
/// ring tiles, and guns standing on every land tile two out seal the approach.
/// They have already fired when the breach is ready, so they cannot swap
/// back. Live King civvis-20261001T050754Z: three Trebuchets held the three
/// tiles two out from Uppsala, whose ring was three land tiles beside four of
/// coast. The city sat breached near 20 HP for eight turns, and no taker
/// could reach a ring tile (diagnosed by -c9). `None` while the walls stand,
/// once the taker is on the ring, or when no such tile exists.
fn taker_corridor(
    g: &Game,
    city: &CityView,
    taker: Option<u32>,
    ring_taken: &BTreeSet<Pos>,
    open_land: &impl Fn(Pos) -> bool,
) -> Option<Pos> {
    if city.wall_hp > 0 {
        return None;
    }
    let taker = taker?;
    let here = g.units.get(&taker)?.pos;
    if g.wdist(here, city.pos) <= 1 {
        return None;
    }
    g.wring(city.pos, 2)
        .into_iter()
        .filter(|pos| {
            open_land(*pos)
                && g.unit_can_traverse(taker, *pos)
                && g.nbrs(*pos).into_iter().any(|ring| {
                    g.wdist(ring, city.pos) == 1
                        && open_land(ring)
                        && g.unit_can_traverse(taker, ring)
                        && (!ring_taken.contains(&ring) || g.unit_ids_at(ring).is_empty())
                })
        })
        .min_by_key(|pos| (g.wdist(here, *pos), *pos))
}

fn siege_posts(
    g: &Game,
    pid: usize,
    city: &CityView,
    force: &[u32],
    taker: Option<u32>,
) -> BTreeMap<u32, Pos> {
    siege_posts_keeping(g, pid, city, force, taker, &BTreeMap::new())
}

/// [`siege_posts`], keeping each unit still walking in on last turn's post
/// (`previous`) while that post still serves.
fn siege_posts_keeping(
    g: &Game,
    pid: usize,
    city: &CityView,
    force: &[u32],
    taker: Option<u32>,
    previous: &BTreeMap<u32, Pos>,
) -> BTreeMap<u32, Pos> {
    let mut posts: BTreeMap<u32, Pos> = BTreeMap::new();
    let mut ring_taken: BTreeSet<Pos> = BTreeSet::new();
    let open_land = |pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
    };
    let mut melee: Vec<u32> = force
        .iter()
        .copied()
        .filter(|uid| arm_of(g, *uid) == Arm::Melee || Some(*uid) == taker)
        .collect();
    melee.sort_by_key(|uid| (g.wdist(g.units[uid].pos, city.pos), *uid));
    for uid in &melee {
        let here = g.units[uid].pos;
        if g.wdist(here, city.pos) <= 1 {
            posts.insert(*uid, here);
            ring_taken.insert(here);
        }
    }
    let ring_free: Vec<Pos> = g
        .wdisk(city.pos, 1)
        .into_iter()
        .filter(|pos| *pos != city.pos && open_land(*pos) && g.unit_ids_at(*pos).is_empty())
        .collect();
    let mut order: Vec<u32> = Vec::new();
    if let Some(taker) = taker.filter(|uid| !posts.contains_key(uid)) {
        order.push(taker);
    }
    order.extend(
        melee
            .iter()
            .copied()
            .filter(|uid| !posts.contains_key(uid) && Some(*uid) != taker),
    );
    for uid in order {
        let here = g.units[&uid].pos;
        // A unit still walking in keeps last turn's post while it stays free
        // and reachable. Redrawn spread-first every turn, the post swung
        // across the city as the ring filled and emptied: live King
        // civvis-20261004T144618Z (game 62) walked the Warrior carrying the
        // Siege Tower four, seven, four, seven tiles from Yaroslavl between
        // turns 89 and 95 (a frame-0 trace at turn 91 shows post_step taking
        // it from four tiles out to six), and the walls fell from 100 to 84
        // in twelve turns of Reduce.
        if let Some(kept) = previous.get(&uid).copied().filter(|pos| {
            ring_free.contains(pos)
                && !ring_taken.contains(pos)
                && g.unit_can_traverse(uid, *pos)
                && siege_route_step(g, pid, uid, *pos, city.pos).is_some()
        }) {
            posts.insert(uid, kept);
            ring_taken.insert(kept);
            continue;
        }
        let mut candidates: Vec<Pos> = ring_free
            .iter()
            .copied()
            .filter(|pos| !ring_taken.contains(pos) && g.unit_can_traverse(uid, *pos))
            .collect();
        candidates.sort_by_key(|pos| {
            let spread = ring_taken
                .iter()
                .map(|held| g.wdist(*pos, *held))
                .min()
                .unwrap_or(i32::MAX);
            (Reverse(spread), g.wdist(here, *pos), *pos)
        });
        let best = candidates
            .into_iter()
            .find(|pos| siege_route_step(g, pid, uid, *pos, city.pos).is_some());
        if let Some(pos) = best {
            posts.insert(uid, pos);
            ring_taken.insert(pos);
        }
    }

    let hostiles: Vec<Pos> = hostiles_near(g, pid, city.pos, OBJECTIVE_REACH)
        .into_iter()
        .filter(|pos| *pos != city.pos)
        .collect();
    let frame = g.player_vision_frame(pid);
    let viewers = g.visibility_viewers(pid);
    let mut guns: Vec<u32> = force
        .iter()
        .copied()
        .filter(|uid| Some(*uid) != taker)
        .filter(|uid| matches!(arm_of(g, *uid), Arm::Siege | Arm::Shooter))
        .collect();
    guns.sort_by_key(|uid| {
        (
            arm_of(g, *uid) != Arm::Siege,
            g.wdist(g.units[uid].pos, city.pos),
            *uid,
        )
    });
    let corridor = taker_corridor(g, city, taker, &ring_taken, &open_land);
    let third = third_strike_sources(g, pid, city.id);
    let third_exposed =
        |pos: Pos| third.iter().any(|source| g.wdist(*source, pos) <= CITY_STRIKE_RANGE);
    let mut fire_taken: BTreeSet<Pos> = BTreeSet::new();
    for uid in guns {
        let here = g.units[&uid].pos;
        let range = g.unit_attack_range(uid).max(1);
        if g.wdist(here, city.pos) <= range
            && Some(here) != corridor
            && !third_exposed(here)
            && !fire_taken.contains(&here)
            && g.ranged_order_is_legal(pid, uid, city.pos, frame.as_ref(), &viewers)
        {
            posts.insert(uid, here);
            fire_taken.insert(here);
            continue;
        }
        // The farthest band with a shot. Live King civvis-20261001T022028Z:
        // Cairo stood on Hills behind Wonder, Holy Site and Jungle tiles, no
        // range-2 tile had a line to it, and the guns stood three and four
        // tiles out for 22 turns of Invest while the walls went 164 -> 200.
        // An adjacent tile always has the line.
        // As for melee: last turn's firing post while it still serves.
        let kept = previous.get(&uid).copied().filter(|pos| {
            *pos != here
                && g.wdist(*pos, city.pos) <= range
                && Some(*pos) != corridor
                && !fire_taken.contains(pos)
                && !ring_taken.contains(pos)
                && open_land(*pos)
                && g.unit_can_traverse(uid, *pos)
                && g.unit_has_line_of_sight_from(uid, *pos, city.pos)
                && g.unit_ids_at(*pos).is_empty()
        });
        if let Some(pos) = kept {
            posts.insert(uid, pos);
            fire_taken.insert(pos);
            continue;
        }
        let best = (1..=range).rev().find_map(|distance| {
            g.wring(city.pos, distance)
                .into_iter()
                .filter(|pos| {
                    *pos != here
                        && Some(*pos) != corridor
                        && !fire_taken.contains(pos)
                        && !ring_taken.contains(pos)
                        && open_land(*pos)
                        && g.unit_can_traverse(uid, *pos)
                        && g.unit_has_line_of_sight_from(uid, *pos, city.pos)
                        && g.unit_ids_at(*pos).is_empty()
                })
                .min_by_key(|pos| {
                    // The gun's own side of the city first: a post across the
                    // city is a walk around its strike ring (Natal, game 30:
                    // guns on the west were posted east).
                    let far_side = g.wdist(here, *pos) > g.wdist(here, city.pos);
                    let behind = ring_taken.iter().any(|held| g.wdist(*held, *pos) == 1);
                    let exposure = hostiles.iter().filter(|h| g.wdist(**h, *pos) <= 2).count();
                    // Out of every other hostile city's strike first; see
                    // `third_strike_sources`.
                    (third_exposed(*pos), far_side, !behind, exposure, g.wdist(here, *pos), *pos)
                })
        });
        if let Some(pos) = best {
            posts.insert(uid, pos);
            fire_taken.insert(pos);
        }
    }
    posts
}

/// `siege-counts-posted-shooters`: does this shooter put its shot on the
/// walls? Only from a firing post `siege_posts` can give it — the range band
/// holds few free tiles, and a shooter with none fortifies where it stands
/// (live King civvis-20261004T122037Z, game 62: archer u43 three tiles from
/// Yaroslavl with range 2) — and only with no hostile unit within its reach of
/// that post, since `siege_shooter_step` shoots a unit before the city while
/// the walls stand.
fn shooter_hits_walls(g: &Game, pid: usize, uid: u32, posts: &BTreeMap<u32, Pos>) -> bool {
    let Some(post) = posts.get(&uid).copied() else {
        return false;
    };
    let range = g.unit_attack_range(uid).max(1);
    !g.wdisk(post, range)
        .into_iter()
        .any(|pos| pos != post && strongest_hostile_at(g, pid, pos).is_some())
}

/// A land shooter of the train: what [`posted_shooters`] gives posts to.
pub(super) fn is_siege_shooter(g: &Game, uid: u32) -> bool {
    arm_of(g, uid) == Arm::Shooter
}

/// `siege-counts-posted-shooters`: the shooters of `force` that hold a firing
/// post on `cid` (`.0`), and those of them whose shot goes to the walls
/// (`.1`). See [`shooter_hits_walls`].
pub(super) fn posted_shooters(
    g: &Game,
    pid: usize,
    cid: u32,
    force: &[u32],
) -> (BTreeSet<u32>, BTreeSet<u32>) {
    let Some(city) = CityView::of(g, cid) else {
        return (BTreeSet::new(), BTreeSet::new());
    };
    let posts = siege_posts(g, pid, &city, force, None);
    let posted: BTreeSet<u32> = force
        .iter()
        .copied()
        .filter(|uid| arm_of(g, *uid) == Arm::Shooter && posts.contains_key(uid))
        .collect();
    let walls = posted
        .iter()
        .copied()
        .filter(|uid| shooter_hits_walls(g, pid, *uid, &posts))
        .collect();
    (posted, walls)
}

impl AdvancedAi {
    /// Carry siege progress and assignments through a native board rebuild.
    /// City positions and host-unit mappings identify the same participants;
    /// mirror-local IDs can now name an unrelated city or soldier. Preserve
    /// the stage clock so rebuilding cannot restart the investment patience.
    pub fn remap_siege_memory(&mut self, previous: &Game, next: &Game, units: &BTreeMap<u32, u32>) {
        let remap_unit = |old: u32| {
            let new = *units.get(&old)?;
            (previous.units.get(&old)?.owner == next.units.get(&new)?.owner).then_some(new)
        };
        self.sieges = std::mem::take(&mut self.sieges)
            .into_iter()
            .filter_map(|(old, mut siege)| {
                let city = next.city_at(previous.cities.get(&old)?.pos)?;
                siege.taker = siege.taker.and_then(remap_unit);
                siege.posts = siege
                    .posts
                    .into_iter()
                    .filter_map(|(unit, pos)| remap_unit(unit).map(|unit| (unit, pos)))
                    .collect();
                Some((city, siege))
            })
            .collect();
        // Only surviving siege takers own these reservations. In particular,
        // losing an objective must not leave its old taker ID reserved.
        self.reserved_units = self
            .sieges
            .values()
            .filter_map(|siege| siege.taker)
            .collect();
    }

    /// Whether the siege has reserved this unit as its taker — the hook a
    /// joint planner (`battle_planner`) reads so it does not spend the unit
    /// that walks in. Published here; the planner's read is its own change.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn unit_is_reserved(&self, uid: u32) -> bool {
        self.reserved_units.contains(&uid)
    }

    /// Whether a Domination siege that has cleared its entry gate — Invest,
    /// Reduce or Take — owns this land unit's turn. The train priced the
    /// assault as a whole (the bill with its margin, the damage budget
    /// inside the force's endurance); the battle planner's per-unit veto
    /// charges every enemy blow to every one of our units at once and must
    /// not undo that verdict one unit at a time.
    pub(super) fn active_siege_member(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.siege_train
            && self.active_victory_target(g) == Some(super::VictoryTarget::Domination)
            && self.force_groups.iter().any(|group| {
                group.domain == ForceDomain::Land
                    && group.units.contains(&uid)
                    && g.city_at(group.objective).is_some_and(|cid| {
                        let owner = g.cities[&cid].owner;
                        owner != pid
                            && g.is_at_war(pid, owner)
                            && self.sieges.get(&cid).is_some_and(|siege| {
                                matches!(
                                    siege.stage,
                                    SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take
                                )
                            })
                    })
            })
    }

    /// Whether this unit belongs to a Domination siege still in Stage. The
    /// train's own Stage step prices its approach (`siege_stage_step`: a gun
    /// holds back on its own risk limit; the rest march to the staging
    /// ring), so the battle planner does not rotate a healthy member out "to
    /// heal". Live King civvis-20261004T033533Z (game 46), turn 140: all four
    /// 100-hp catapults before Babylon were rotated out at danger 96-100 and
    /// fortified; Stage never invested in 18 turns.
    pub(super) fn staging_siege_member(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.siege_train
            && self.active_victory_target(g) == Some(super::VictoryTarget::Domination)
            && self.force_groups.iter().any(|group| {
                group.domain == ForceDomain::Land
                    && group.units.contains(&uid)
                    && g.city_at(group.objective).is_some_and(|cid| {
                        let owner = g.cities[&cid].owner;
                        owner != pid
                            && g.is_at_war(pid, owner)
                            && self
                                .sieges
                                .get(&cid)
                                .is_some_and(|siege| siege.stage == SiegeStage::Stage)
                    })
            })
    }

    /// Whether this unit is a member of an active Domination siege (see
    /// `active_siege_member`) whose city stands without walls: its shot
    /// is the city's health, and the battle planner leaves it to the train.
    pub(super) fn unwalled_siege_member(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.active_siege_member(g, pid, uid)
            && self.force_groups.iter().any(|group| {
                group.units.contains(&uid)
                    && g.city_at(group.objective)
                        .is_some_and(|cid| g.cities[&cid].wall_hp <= 0)
            })
    }

    /// The doctrine's turn for one unit: `Some(acted)` when a siege or an
    /// anvil owns the unit's decision, `None` for the ladder. Returns before
    /// reading the board with both genes off.
    pub(super) fn siege_doctrine_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !self.siege_train && !self.siege_positive_damage_budget && !self.anvil {
            return None;
        }
        if self.guard_is_reserved_for_civilian(uid) {
            return None;
        }
        // An adjacent landing capture does not require a dry siege-ring post.
        // Keep embarked units out of the ordinary shooter and screen roles.
        if self.siege_train && g.units.get(&uid).is_some_and(|unit| g.is_embarked(unit)) {
            if let Some(city) = plan.target_city.and_then(|cid| CityView::of(g, cid)) {
                if city.wall_hp <= 0
                    && g.melee_order_is_legal(pid, uid, city.pos)
                    && f64::from(city.hp) <= taker_blow(g, pid, uid, city.id)
                {
                    return Some(self.taker_step(g, pid, uid, &city));
                }
            }
        }
        if self.siege_needs_a_breaker && self.siege_train {
            if let Some(acted) = self.breach_support_step(g, pid, uid, plan) {
                return Some(acted);
            }
        }
        if arm_of(g, uid) == Arm::Other {
            return None;
        }
        let group = self
            .force_groups
            .iter()
            .find(|group| group.units.contains(&uid))
            .cloned()?;
        if group.domain != ForceDomain::Land {
            return None;
        }
        if self.anvil {
            if let Some(acted) = self.anvil_step(g, pid, uid, plan, &group) {
                return Some(acted);
            }
        }
        if self.siege_train || self.siege_positive_damage_budget {
            if let Some(cid) = self.siege_city_of(g, pid, plan, &group) {
                return self.siege_train_step(g, pid, uid, cid, plan, &group);
            }
        }
        None
    }

    /// The enemy city a group is on: the one at its objective, or the
    /// plan's target city within reach while no city of ours is threatened.
    pub(super) fn siege_city_of(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Option<u32> {
        let enemy_city = |cid: u32| {
            g.cities
                .get(&cid)
                .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
                .map(|_| cid)
        };
        g.city_at(group.objective).and_then(enemy_city).or_else(|| {
            if plan.threatened_city.is_some() {
                return None;
            }
            plan.target_city
                .and_then(enemy_city)
                .filter(|cid| g.wdist(group.anchor, g.cities[cid].pos) <= OBJECTIVE_REACH)
        })
    }

    /// Only groups whose orders serve this city supply its siege roster.
    /// Neighboring sieges must not reserve each other's takers or posts.
    fn siege_force(
        &self,
        g: &Game,
        pid: usize,
        city: &CityView,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Vec<u32> {
        self.force_groups
            .iter()
            .chain(std::iter::once(group))
            .filter(|group| {
                group.domain == ForceDomain::Land
                    && self.siege_city_of(g, pid, plan, group) == Some(city.id)
            })
            .flat_map(|group| group.units.iter().copied())
            .filter(|uid| {
                g.units.get(uid).is_some_and(|unit| unit.owner == pid)
                    && arm_of(g, *uid) != Arm::Other
                    && !self.guard_is_reserved_for_civilian(*uid)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// `siege-needs-a-breaker`: whether a ranged member will be on the line
    /// this turn. The battle planner pulls a unit under `ROTATE_HP` out to
    /// heal and keeps it out until `RETURN_HP` (`battle_planner_recovering`);
    /// a gun it holds back fires nothing however near it stands.
    pub(super) fn siege_member_fit(&self, g: &Game, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let heals = !g.is_arena() || g.tactics.heal;
        !heals
            || (unit.hp >= super::battle_planner::ROTATE_HP
                && !self.battle_planner_recovering.contains(&uid))
    }

    /// `siege-needs-a-breaker`: whether the capture of `cid` is waiting on a
    /// breaker that is coming — its train held outside the city's reach for
    /// want of one (`BreachReading`) this turn or the last, for at most
    /// [`BREAKER_WAIT_TURNS`] standard turns, while a siege gun or a ram or
    /// tower that opens these walls stands within [`BREAKER_COMING_REACH`],
    /// or a city of ours that close is building a gun. The commitment ledger
    /// does not count such a turn against the capture
    /// (`capture-go-or-stand-down`): the train stands three to five tiles
    /// out, so it read as "nobody went" or "not winning" and the city was
    /// stood down while its guns were on the road.
    pub(super) fn waiting_for_a_breaker(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.siege_needs_a_breaker {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let Some(wait) = self.siege_breaker_waits.get(&city.pos) else {
            return false;
        };
        if g.turn.saturating_sub(wait.last) > 1
            || g.turn.saturating_sub(wait.since) > g.standard_duration(BREAKER_WAIT_TURNS)
        {
            return false;
        }
        // On its way: one has come nearer within BREAKER_STALL_TURNS. A gun
        // that stands off — driven back, healing, boxed in — is not coming.
        let closing = wait.nearest <= BREAKER_COMING_REACH
            && g.turn.saturating_sub(wait.nearest_turn) <= g.standard_duration(BREAKER_STALL_TURNS);
        // Or nearly built in a city of ours within reach.
        let building = g.cities.values().any(|own| {
            own.owner == pid
                && g.wdist(own.pos, city.pos) <= BREAKER_COMING_REACH
                && own.queue.first().is_some_and(|item| {
                    let crate::game::Item::Unit { unit } = item else {
                        return false;
                    };
                    if !land_gun(g, *unit) {
                        return false;
                    }
                    let left = (g.item_cost_for_city(pid, own.id, item) - own.production).max(0.0);
                    let rate = g.city_yields(own.id).production.max(0.1);
                    left / rate <= f64::from(g.standard_duration(BREAKER_BUILD_TURNS))
                })
        });
        closing || building
    }

    /// `siege-needs-a-breaker`: whether the siege train for `cid` is
    /// gathering on its staging ring — its record in Stage, assessed this
    /// turn or the last, for at most [`BREAKER_WAIT_TURNS`] standard turns,
    /// with a land soldier of ours within [`STAGING_FAR`]. Stage holds the
    /// train three to five tiles out until the staged strength meets the bill
    /// and the damage budget is ready, so the capture ledger read it as
    /// "nobody went" (no one within `CAPTURE_PRESENCE_RADIUS`) or, counted
    /// present, as "not winning". Live King civvis-20261003T162445Z
    /// (game 42r): Cairo, undefended, its train 126 strength near against a
    /// bill of 158 with a Bombard on the ring, was stood down at turn 114
    /// as "the objective nobody went to".
    pub(super) fn siege_mustering(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.siege_needs_a_breaker {
            return false;
        }
        let (Some(siege), Some(city)) = (self.sieges.get(&cid), g.cities.get(&cid)) else {
            return false;
        };
        siege.stage == SiegeStage::Stage
            && g.turn.saturating_sub(siege.assessed) <= 1
            && g.turn.saturating_sub(siege.entered) <= g.standard_duration(BREAKER_WAIT_TURNS)
            && g.units.values().any(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.wdist(unit.pos, city.pos) <= STAGING_FAR
            })
    }

    /// `siege-needs-a-breaker`: why the capture of `cid` is waiting on its
    /// own train this turn, if it is — a breaker on its way, or the train
    /// gathering on its staging ring. The commitment ledger counts such a
    /// turn as neither forgotten nor stalled.
    pub(super) fn capture_waits_on_the_train(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<&'static str> {
        if self.waiting_for_a_breaker(g, pid, cid) {
            Some("for a wall-breaker on its way")
        } else if self.siege_mustering(g, pid, cid) {
            Some("while its siege train gathers on the staging ring")
        } else {
            None
        }
    }

    /// `siege-needs-a-breaker`: what the train holds within the staging ring
    /// that can bring the walls down. See [`BreachReading`].
    fn breach_reading(&self, g: &Game, pid: usize, city: &CityView, force: &[u32]) -> BreachReading {
        let mut reading = BreachReading {
            horizon: SHOOTER_BREACH_TURNS,
            ..BreachReading::default()
        };
        // `siege-counts-posted-shooters`: see `shooter_hits_walls`.
        let wall_shooters = self
            .siege_counts_posted_shooters
            .then(|| posted_shooters(g, pid, city.id, force).1);
        let mut users = false;
        for uid in force {
            let Some(unit) = g.units.get(uid) else {
                continue;
            };
            if g.wdist(unit.pos, city.pos) > STAGING_FAR {
                continue;
            }
            match arm_of(g, *uid) {
                Arm::Siege if self.siege_member_fit(g, *uid) => reading.guns += 1,
                Arm::Siege => reading.wounded_guns += 1,
                Arm::Shooter if self.siege_member_fit(g, *uid) => {
                    if wall_shooters.as_ref().is_none_or(|shooters| shooters.contains(uid)) {
                        reading.shooter_walls += wall_damage_per_shot(g, *uid, city.id);
                    }
                }
                Arm::Melee if breach_support_user(g, *uid) => {
                    users = true;
                    // A carrier brings its ram or tower to the ring with it.
                    if unit
                        .linked_to
                        .is_some_and(|peer| breach_support_works(g, peer, city.id))
                    {
                        reading.support += 1;
                    }
                }
                _ => {}
            }
        }
        if users {
            reading.support += g
                .units
                .values()
                .filter(|unit| {
                    unit.owner == pid
                        && unit.linked_to.is_none()
                        && g.wdist(unit.pos, city.pos) <= STAGING_FAR
                        && breach_support_works(g, unit.id, city.id)
                })
                .count();
        }
        reading
    }

    /// `siege-needs-a-breaker`: a Battering Ram or Siege Tower whose effect
    /// works on a besieged city's walls joins a melee carrier of the train.
    /// The effect needs it beside the city next to the melee that swings
    /// (`Game::siege_support_effects`), and `advanced_formations` links it
    /// only to whatever military unit shares its tile: live King
    /// civvis-20261003T135713Z linked its Siege Tower to three different units
    /// between turns 119 and 147, and it stood seven tiles from Kwadukuza
    /// "walking in circles" through a siege against Medieval Walls, the walls a
    /// tower opens. So a support unit carried by a melee member of the train
    /// rides with it; one carried by anything else leaves it; and a free one
    /// walks onto the nearest melee member it can reach this turn and links,
    /// or closes on the train without ending alone inside the city's reach.
    /// `None` for anything that is not such a support unit near such a siege.
    fn breach_support_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        if unit.owner != pid
            || !matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
            || g.is_embarked(&unit)
        {
            return None;
        }
        let besieged = |cid: u32| {
            CityView::of(g, cid).filter(|city| {
                city.owner != pid
                    && g.is_at_war(pid, city.owner)
                    && !walls_open_to_melee(city)
                    && breach_support_works(g, uid, city.id)
                    && g.wdist(unit.pos, city.pos) <= BREACH_SUPPORT_REACH
            })
        };
        let city = self
            .sieges
            .iter()
            .filter(|(_, siege)| siege.stage != SiegeStage::Hold)
            .filter_map(|(cid, _)| besieged(*cid))
            .chain(plan.target_city.and_then(besieged))
            .min_by_key(|city| (g.wdist(unit.pos, city.pos), city.id))?;
        let members: BTreeSet<u32> = self
            .force_groups
            .iter()
            .filter(|group| {
                group.domain == ForceDomain::Land
                    && self.siege_city_of(g, pid, plan, group) == Some(city.id)
            })
            .flat_map(|group| group.units.iter().copied())
            .collect();
        let carrier_fit = |g: &Game, id: u32| {
            members.contains(&id)
                && g.units.get(&id).is_some_and(|carrier| {
                    carrier.owner == pid
                        && carrier.hp >= super::battle_planner::ROTATE_HP
                        && !g.is_embarked(carrier)
                        && arm_of(g, id) == Arm::Melee
                        && breach_support_user(g, id)
                })
        };
        if unit.linked_to.is_some_and(|peer| carrier_fit(g, peer)) {
            // The carrier's orders move the pair.
            return Some(false);
        }
        let carriers: Vec<u32> = members
            .iter()
            .copied()
            .filter(|id| carrier_fit(g, *id) && g.units[id].linked_to.is_none())
            .collect();
        if carriers.is_empty() {
            // Nothing of the train to ride with: whatever carries it now
            // keeps it, and the ladder moves it.
            return None;
        }
        if unit.linked_to.is_some() {
            if g.apply(pid, &Action::UnlinkUnits { unit: uid }).is_err() {
                return None;
            }
            self.force_groups_dirty = true;
        }
        let here = g.units[&uid].pos;
        let link = |g: &mut Game, carrier: u32| {
            g.apply(
                pid,
                &Action::LinkUnits {
                    unit: carrier,
                    with: uid,
                },
            )
            .is_ok()
        };
        let kind = unit.kind;
        let name = g.cities[&city.id].name.clone();
        // One already shares the tile: link.
        if let Some(carrier) = carriers.iter().copied().find(|id| g.units[id].pos == here) {
            if link(g, carrier) {
                self.force_groups_dirty = true;
                think!(self.journal(), Military, Decision,
                    "Siege of {name}: the {kind} joins the {}", g.units[&carrier].kind;
                    "walls {}/{} that a {kind} opens; it rides with a melee member of the train to the ring",
                    city.wall_hp, city.wall_max;
                    city.pos);
                return Some(true);
            }
        }
        // The nearest carrier reachable this turn: walk onto it and link.
        let reach = g.reachable(uid);
        if let Some(carrier) = carriers
            .iter()
            .copied()
            .filter(|id| reach.contains(&g.units[id].pos))
            .min_by_key(|id| (g.wdist(here, g.units[id].pos), *id))
        {
            let to = g.units[&carrier].pos;
            if self.base.path_walk_to(g, pid, uid, to) {
                self.force_groups_dirty = true;
                if g.units.get(&uid).is_some_and(|unit| unit.pos == to) && link(g, carrier) {
                    think!(self.journal(), Military, Decision,
                        "Siege of {name}: the {kind} joins the {}", g.units[&carrier].kind;
                        "walls {}/{} that a {kind} opens; it rides with a melee member of the train to the ring",
                        city.wall_hp, city.wall_max;
                        city.pos);
                }
                return Some(true);
            }
        }
        // Otherwise close on the nearest carrier, never ending alone inside
        // the city's reach. With no melee member to carry it, a lone ram or
        // tower has no business at the walls: the ladder keeps it.
        let goal = carriers
            .iter()
            .map(|id| g.units[id].pos)
            .min_by_key(|pos| (g.wdist(here, *pos), *pos))?;
        let next = march_step(g, uid, goal, 0).filter(|next| {
            g.can_move(uid, *next)
                && (g.wdist(*next, city.pos) > CITY_STRIKE_RANGE
                    || g.unit_ids_at(*next).iter().any(|id| {
                        g.units[id].owner == pid && g.rules.units[g.units[id].kind].class == "military"
                    }))
        });
        if let Some(next) = next {
            return Some(self.base.tactical_apply_move(g, pid, uid, next));
        }
        Some(self.base.fortify_or_stop(g, pid, uid))
    }

    /// The state machine, once a turn per city: the bill, the strength, the
    /// ring, the stage, the taker, the census and the journal line.
    fn assess_siege(
        &mut self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) {
        let turn = g.turn;
        // A capture on a disposable planning board can leave persistent memory
        // in Hold even when the next host frame still shows an enemy city.
        // Reconcile that contradiction before the once-per-turn cache: native
        // air-assault continuations can deliver a fresh board in the same turn.
        let held_enemy_city = self
            .sieges
            .get(&cid)
            .is_some_and(|siege| siege.stage == SiegeStage::Hold)
            && g.cities
                .get(&cid)
                .is_some_and(|city| city.owner != pid && g.is_at_war(pid, city.owner));
        if self
            .sieges
            .get(&cid)
            .is_some_and(|siege| siege.assessed == turn)
            && !held_enemy_city
        {
            return;
        }
        let Some(city) = CityView::of(g, cid) else {
            return;
        };
        self.sieges
            .retain(|_, siege| siege.assessed.saturating_add(SIEGE_MEMORY) >= turn);
        self.reserved_units
            .retain(|uid| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        let arena = g.is_arena();
        let force = self.siege_force(g, pid, &city, plan, group);
        let strength: f64 = force.iter().map(|uid| unit_power(g, *uid)).sum();
        let staged: f64 = force
            .iter()
            .filter(|uid| g.wdist(g.units[uid].pos, city.pos) <= STAGING_FAR)
            .map(|uid| unit_power(g, *uid))
            .sum();
        let gathered = force
            .iter()
            .all(|uid| g.wdist(g.units[uid].pos, city.pos) <= STAGING_FAR + 1);
        let bill = siege_bill(g, pid, &city);
        let (sealed, ring) = ring_state(g, cid);
        let shooter_in_range = force.iter().any(|uid| {
            matches!(arm_of(g, *uid), Arm::Siege | Arm::Shooter)
                && g.wdist(g.units[uid].pos, city.pos) <= g.unit_attack_range(*uid).max(1)
        });

        let damage_ready = self.conversion_siege_ready(g, pid, cid, &force);
        let damage_budget = self.conversion_siege_budget(g, pid, cid, &force);
        // Entry needs a margin for the first reply. Once the train has
        // invested, a modest dip below that entry margin must not pull its
        // guns back out of firing range every other turn. A force that cannot
        // finish within its estimated endurance still regroups.
        let damage_can_continue = !self.siege_positive_damage_budget
            || damage_budget.is_some_and(|(turns, endurance)| turns <= endurance);
        // A damaged wall is a running assault, even if losses briefly forced
        // the train back to Stage. Resume when it can finish within its full
        // endurance; waiting for the untouched-wall entry margin again lets
        // the defender's wall and the war-stall clock recover for free.
        let damage_entry_ready = damage_ready
            || (self.siege_positive_damage_budget
                && city.wall_hp > 0
                && city.wall_hp < city.wall_max
                && damage_can_continue);
        let breach_taker = (city.wall_hp <= 0 && city.hp <= 100 && strength >= bill)
            .then(|| {
                designate_taker(g, &city, &force)
                    .filter(|uid| {
                        let unit = &g.units[uid];
                        unit.hp >= 70
                            && g.wdist(unit.pos, city.pos) <= STAGING_FAR
                            && f64::from(city.hp) <= taker_blow(g, pid, *uid, cid) + 40.0
                    })
                    .or_else(|| approaching_breach_taker(g, pid, &city, &force))
            })
            .flatten();
        // `siege-needs-a-breaker`: walls nothing in the train can open are
        // not besieged — the melee would hold a ring under the city's fire
        // with nothing to show for it. See `BreachReading`.
        let breach = self.siege_needs_a_breaker.then(|| {
            let mut reading = self.breach_reading(g, pid, &city, &force);
            if strength >= DOMINANT_BILL_SHARE * bill {
                reading.horizon = SHOOTER_BREACH_TURNS_DOMINANT;
            }
            reading
        });
        let no_breaker = !arena && breach.is_some_and(|reading| !reading.at_hand(&city));
        // `siege-needs-a-breaker`: walls a breaker at hand can open are
        // opened before the taker comes. The damage budget reads a train
        // with no melee taker in reach as never finishing (`inf`), so a
        // force of shooters that would breach in a few turns staged instead.
        // Live King civvis-20261003T164758Z (game 43) held Thebes's 100
        // walls in Stage from turn 90 to 100 with Archers at 20 wall a turn,
        // the bill met, until a Horseman came. The train invests to open
        // the walls; the budget gates the assault once they are a quarter
        // down (`siege-holds-a-breach` holds that breach).
        let opens_walls = !arena
            && !walls_open_to_melee(&city)
            && breach.is_some_and(|reading| reading.at_hand(&city))
            && !force.iter().any(|uid| {
                arm_of(g, *uid) == Arm::Melee
                    && g.wdist(g.units[uid].pos, city.pos) <= STAGING_FAR
            });
        if self.siege_needs_a_breaker {
            if no_breaker {
                let nearest = nearest_breaker(g, pid, &city);
                let wait = self
                    .siege_breaker_waits
                    .entry(city.pos)
                    .or_insert(BreakerWait {
                        since: turn,
                        last: turn,
                        nearest,
                        nearest_turn: turn,
                    });
                wait.last = turn;
                if nearest < wait.nearest {
                    wait.nearest = nearest;
                    wait.nearest_turn = turn;
                }
            } else {
                self.siege_breaker_waits.remove(&city.pos);
            }
        }
        let record = self.sieges.entry(cid).or_insert(Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: turn,
            assessed: turn,
            posts: BTreeMap::new(),
            short_since: None,
        });
        record.assessed = turn;
        let previous = record.stage;
        let previous_taker = record.taker;
        let mut stage = if held_enemy_city {
            SiegeStage::Reduce
        } else {
            previous
        };
        if city.owner == pid {
            stage = SiegeStage::Hold;
        } else {
            // A wall rebuilt behind the ring can invalidate the damage
            // budget even while the nominal force still covers the bill.
            // Regroup for breach support instead of remaining in Reduce with
            // no attack that can finish before the force is exhausted.
            let invested = stage != SiegeStage::Stage && !arena;
            // `siege-holds-a-breach`: a wall already a quarter down is a
            // running assault; only a deep shortfall abandons it.
            let held_breach = self.siege_holds_a_breach
                && city.wall_max > 0
                && f64::from(city.wall_hp) <= HELD_BREACH_WALL_SHARE * f64::from(city.wall_max);
            let abort_share = if held_breach {
                HELD_BREACH_ABORT_SHARE
            } else {
                ABORT_SHARE
            };
            if invested
                && (strength < abort_share * bill
                    || (!damage_can_continue
                        && breach_taker.is_none()
                        && !held_breach
                        && !opens_walls)
                    || no_breaker)
            {
                // A Hold over a city that is not ours is a capture that never
                // landed, not a running assault: there is no dip to ride out,
                // so reopening it never bypasses the abort gate.
                let since = *record.short_since.get_or_insert(turn);
                if previous == SiegeStage::Hold || turn.saturating_sub(since) + 1 >= ABORT_PATIENCE {
                    stage = SiegeStage::Stage;
                }
            } else {
                record.short_since = None;
            }
            match stage {
                SiegeStage::Stage => {
                    // A finished wall and a city within one near taker's
                    // next blow are an opening that will heal away while the
                    // rest of the train fills distant staging posts. Keep
                    // the whole-force bill and positive-damage gate, but let
                    // a healthy, reachable capturer exploit that breach.
                    if ((arena && gathered) || staged >= bill || breach_taker.is_some())
                        && (damage_entry_ready || opens_walls)
                        && !no_breaker
                    {
                        stage = SiegeStage::Invest;
                    }
                }
                SiegeStage::Invest => {
                    let patience = turn.saturating_sub(record.entered) >= INVEST_PATIENCE;
                    if (ring > 0 && sealed == ring) || (patience && shooter_in_range) {
                        stage = SiegeStage::Reduce;
                    }
                }
                _ => {}
            }
        }
        let mut taker = None;
        if matches!(
            stage,
            SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take
        ) {
            let immediate = designate_taker(g, &city, &force);
            taker = immediate
                .filter(|uid| g.units[uid].hp >= 70)
                .or_else(|| {
                    previous_taker.filter(|uid| {
                        city.wall_hp <= 0
                            && force.contains(uid)
                            && g.units.get(uid).is_some_and(|unit| unit.hp >= 70)
                    })
                })
                .or(breach_taker)
                .or(immediate);
            let ready = taker.is_some_and(|uid| {
                g.wdist(g.units[&uid].pos, city.pos) <= 1
                    && (city.hp <= 0 || f64::from(city.hp) <= taker_blow(g, pid, uid, cid))
            });
            if ready {
                stage = SiegeStage::Take;
            } else if stage == SiegeStage::Take {
                stage = SiegeStage::Reduce;
            }
        }
        if stage != previous {
            record.entered = turn;
            record.stage = stage;
        }
        if let Some(old) = record.taker.take() {
            self.reserved_units.remove(&old);
        }
        record.taker = taker;
        if let Some(uid) = taker {
            self.reserved_units.insert(uid);
        }
        record.posts = if matches!(
            stage,
            SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take
        ) {
            siege_posts_keeping(g, pid, &city, &force, taker, &record.posts)
        } else {
            BTreeMap::new()
        };

        match stage {
            SiegeStage::Stage => self.census.siege_stage_turns += 1,
            SiegeStage::Invest => self.census.siege_invest_turns += 1,
            SiegeStage::Reduce => self.census.siege_reduce_turns += 1,
            SiegeStage::Take => self.census.siege_take_turns += 1,
            SiegeStage::Hold => self.census.siege_hold_turns += 1,
        }
        if ring > 0 && sealed == ring && stage != SiegeStage::Hold {
            self.census.siege_rings_sealed += 1;
        }
        if stage == SiegeStage::Hold && previous != SiegeStage::Hold {
            self.census.siege_captures += 1;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let name = g
                .cities
                .get(&cid)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let damage_budget = damage_budget
                .map(|(turns, endurance)| format!("{turns:.1} turns / {endurance:.1} endurance"))
                .unwrap_or_else(|| "unknown".to_string());
            let taker_note = match taker {
                Some(uid) => format!(", taker {} reserved", g.units[&uid].kind),
                None => String::new(),
            };
            let breach_note = match breach {
                Some(reading) => format!(
                    "; breakers: {} gun(s) fit, {} healing, {} ram/tower, shooters {:.0} wall a turn{}",
                    reading.guns,
                    reading.wounded_guns,
                    reading.support,
                    reading.shooter_walls,
                    if no_breaker {
                        " — nothing to open the walls, so the train holds outside the city's reach"
                    } else if opens_walls {
                        " — no taker in reach yet, so the train opens the walls first"
                    } else {
                        ""
                    }
                ),
                None => String::new(),
            };
            think!(self.journal(), Military, Decision,
                "Siege of {name}: {}", stage.as_str();
                "ring {sealed}/{ring} sealed, walls {}/{}, city {}/200, {} of {} units staged, \
                 {strength:.0} strength ({staged:.0} near) against a bill of {bill:.0}; \
                 damage ready {damage_ready} with {damage_budget}{taker_note}{breach_note}",
                city.wall_hp, city.wall_max, city.hp,
                force.iter().filter(|uid| g.wdist(g.units[uid].pos, city.pos) <= STAGING_FAR).count(),
                force.len();
                city.pos);
        }
    }

    fn siege_train_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        cid: u32,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Option<bool> {
        self.assess_siege(g, pid, cid, plan, group);
        let siege = self.sieges.get(&cid)?.clone();
        let city = CityView::of(g, cid)?;
        if city.owner == pid || siege.stage == SiegeStage::Hold {
            return None;
        }
        // `breach-assault` runs before the Stage return: a siege whose counted
        // members are still far off reads Stage however low the city is, and
        // a unit already beside a breached city must not walk away from it.
        if self.breach_assault && siege.taker != Some(uid) && arm_of(g, uid) == Arm::Melee {
            if let Some(acted) = self.breach_assault_blow(g, pid, uid, &city, plan, group) {
                return Some(acted);
            }
        }
        if siege.stage == SiegeStage::Stage {
            return Some(self.siege_stage_step(g, pid, uid, &city, plan));
        }
        if self.siege_spotter(g, pid, group, &city) == Some(uid) {
            if let Some(acted) = self.spotter_step(g, pid, uid, &city) {
                return Some(acted);
            }
        }
        if siege.taker == Some(uid) {
            return Some(self.taker_step(g, pid, uid, &city));
        }
        match arm_of(g, uid) {
            Arm::Melee => Some(self.siege_melee_step(g, pid, uid, &city, plan)),
            Arm::Siege => Some(self.siege_gun_step(g, pid, uid, &city)),
            Arm::Shooter => Some(self.siege_shooter_step(g, pid, uid, &city)),
            Arm::Other => None,
        }
    }

    /// The member that steps into sight of an invested city nobody of ours
    /// can see: the nearest healthy land soldier with moves within
    /// [`STAGING_FAR`]. `None` while the city is in sight. Every reading the
    /// train takes of an unseen city is memory: in sim seed 37140004 the
    /// Siege of Hastings sat in Invest for eighteen turns, its units 3-4 tiles
    /// out at the anchor, on a wall reading of 22/400 frozen from the last
    /// sighting (diagnosed by -c9).
    fn siege_spotter(
        &self,
        g: &Game,
        pid: usize,
        group: &ForceGroup,
        city: &CityView,
    ) -> Option<u32> {
        if g.sees(&g.player_vision_frame(pid), city.pos) {
            return None;
        }
        group
            .units
            .iter()
            .copied()
            .filter(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    let spec = &g.rules.units[unit.kind];
                    unit.owner == pid
                        && spec.class == "military"
                        && spec.domain.as_deref().is_none_or(|domain| domain == "land")
                        && !g.is_embarked(unit)
                        && unit.hp >= SPOTTER_MIN_HP
                        && unit.moves_left > 0.0
                        && g.wdist(unit.pos, city.pos) <= STAGING_FAR
                })
            })
            .min_by_key(|uid| (g.wdist(g.units[uid].pos, city.pos), *uid))
    }

    /// Walk the spotter to the tile it can reach this turn that sees the
    /// city, where its expected reply leaves it more than half its health.
    fn spotter_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        let sight = g.unit_sight(uid).max(1);
        let mut candidates: Vec<Pos> = g
            .reachable(uid)
            .into_iter()
            .filter(|pos| {
                *pos != unit.pos
                    && g.city_at(*pos).is_none()
                    && dry_stand(g, uid, *pos)
                    && g.wdist(*pos, city.pos) <= sight
                    && g.line_of_sight_from(*pos, city.pos)
            })
            .collect();
        candidates.sort_by_key(|pos| {
            (
                g.wdist(unit.pos, *pos),
                Reverse(g.wdist(*pos, city.pos)),
                *pos,
            )
        });
        let half = f64::from(unit.hp) * 0.5;
        let dest = candidates
            .into_iter()
            .take(12)
            .find(|pos| self.approach_danger(g, pid, *pos, uid) < half)?;
        if !self.base.path_walk_to(g, pid, uid, dest) {
            return None;
        }
        think!(self.journal(), Military, Detail,
            "Siege of {}: the {} steps into sight of the city", g.cities[&city.id].name, unit.kind;
            "nothing of ours saw it, so every wall and garrison reading was memory; {:?} sees it at {} tiles",
            dest, g.wdist(dest, city.pos);
            city.pos);
        Some(true)
    }

    /// Stage: fight what comes out, step out of the city's reach, march to
    /// the staging ring, and hold there as a body.
    fn siege_stage_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
        plan: &StrategicPlan,
    ) -> bool {
        // ★★★ STAGE NEVER STRUCK THE CITY. `allow_city` was false here, so a
        // unit beside a dying city could not finish it and the step below
        // walked it back out of the city's reach. Live King
        // civvis-20261004T114858Z (game 55): the Siege of Nicomedia read Stage
        // in all 82 assessments from turn 77 to 225; at turn 200 the city
        // stood at 41/200 behind 0/400 walls with a Llanero beside it, and
        // the Llanero walked three tiles out and fortified (diagnosed by -9c).
        // Under `breach-assault`, `siege_blow`'s own kill-or-paying-exchange
        // test may take a city whose walls no longer shield it.
        let allow_city = self.breach_assault && melee_wall_attack_allowed(g, pid, uid, city.id);
        if let Some(acted) = self.siege_blow(g, pid, uid, city, plan, allow_city) {
            return acted;
        }
        let here = g.units[&uid].pos;
        let distance = g.wdist(here, city.pos);
        // `shared-danger`, as the battle planner and the reinforcement step
        // read it: a hostile's one blow a turn is split among the units of
        // ours in its reach. Unshared, a gun inside its own army read every
        // ranger's blow as its alone and held at the edge: live King
        // civvis-20261004T033533Z (game 46) kept Babylon's catapults six to
        // ten tiles out from turn 135 to 153 ("damage ready false"), and
        // civvis-20261004T040138Z (game 47) held Quebec City's at ten.
        let mut gun_danger = (arm_of(g, uid) == Arm::Siege).then(|| {
            let mut field = super::battle_planner::DangerField::with_reach(g, pid, true);
            if self.shared_danger {
                field.share(g);
            }
            field
        });
        let gun_risk_limit = (f64::from(g.units[&uid].hp) - STAGING_GUN_HP_RESERVE).max(0.0)
            / STAGING_GUN_REPLY_TURNS;
        // `staging-gun-trusts-its-escort`: see `STAGING_ESCORT_BODIES`. The
        // board is a parameter, not a capture, so the step may still move the
        // unit while the limit is in scope.
        let trusts_escort = self.staging_gun_trusts_its_escort;
        let escorted_limit = |g: &Game, pos: Pos| -> f64 {
            let escorts = g
                .nbrs(pos)
                .into_iter()
                .chain(std::iter::once(pos))
                .flat_map(|n| g.unit_ids_at(n).iter().copied())
                .filter(|id| {
                    *id != uid
                        && g.units.get(id).is_some_and(|unit| {
                            let spec = &g.rules.units[unit.kind];
                            unit.owner == pid
                                && spec.class == "military"
                                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                                && arm_of(g, *id) != Arm::Siege
                        })
                })
                .count();
            if trusts_escort && escorts >= STAGING_ESCORT_BODIES {
                (f64::from(g.units[&uid].hp) - STAGING_GUN_HP_RESERVE).max(0.0)
            } else {
                gun_risk_limit
            }
        };
        // `staging-gun-remembers-hostiles`: the danger field reads only the
        // units on the board, and a hostile that walked into the fog is not
        // on it. Price the ones the seat saw this turn or last. Off, the
        // list is empty and every reading below is the field's alone.
        let remembered = if self.staging_gun_remembers_hostiles && arm_of(g, uid) == Arm::Siege {
            self.remembered_strikers(g, pid, uid)
        } else {
            Vec::new()
        };
        let fog_blow = |g: &Game, pos: Pos| -> f64 {
            remembered
                .iter()
                .filter(|striker| g.wdist(striker.from, pos) <= striker.reach)
                .map(|striker| striker.blow)
                .fold(0.0, f64::max)
        };
        let remembers = !remembered.is_empty();
        if let Some(field) = gun_danger.as_mut() {
            let risk_here = field.danger(here, uid);
            if risk_here > escorted_limit(g, here) {
                let safer = g
                    .nbrs(here)
                    .into_iter()
                    .filter(|pos| {
                        g.can_move(uid, *pos) && g.wdist(*pos, city.pos) > CITY_STRIKE_RANGE
                    })
                    .map(|pos| (field.danger(pos, uid), g.wdist(pos, city.pos), pos))
                    .filter(|(risk, _, _)| *risk + 1.0 < risk_here)
                    .min_by(|a, b| {
                        a.0.total_cmp(&b.0)
                            .then_with(|| a.1.cmp(&b.1))
                            .then_with(|| a.2.cmp(&b.2))
                    });
                if let Some((_, _, pos)) = safer {
                    return self.base.tactical_apply_move(g, pid, uid, pos);
                }
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }
        if distance <= CITY_STRIKE_RANGE {
            let mut best: Pick<BackOffKey> = None;
            for pos in g.nbrs(here) {
                let away = g.wdist(pos, city.pos);
                if away <= distance || !g.can_move(uid, pos) {
                    continue;
                }
                let friends = g
                    .nbrs(pos)
                    .into_iter()
                    .flat_map(|n| g.unit_ids_at(n).iter().copied())
                    .filter(|id| g.units[id].owner == pid)
                    .count() as i32;
                let key = (away.min(STAGING_NEAR), friends, Reverse(pos));
                if best.as_ref().is_none_or(|(old, _)| key > *old) {
                    best = Some((key, pos));
                }
            }
            if let Some((_, pos)) = best {
                return self.base.tactical_apply_move(g, pid, uid, pos);
            }
            return self.base.fortify_or_stop(g, pid, uid);
        }
        if distance > STAGING_FAR {
            // `stage-march-keeps-to-land`: see `StageMarch`. Off, or without
            // `come-ashore`, the march is the ordinary one.
            let dry_march = if self.stage_march_keeps_to_land && self.base.come_ashore {
                stage_march(g, uid, city.pos, STAGING_FAR)
            } else {
                StageMarch::Ordinary
            };
            if let StageMarch::Hold { wet } = dry_march {
                think!(self.journal(), Military, Detail,
                    "Siege of {}: the {} holds on land short of the water", g.cities[&city.id].name, g.units[&uid].kind;
                    "the march crosses water in {} steps and come-ashore lands an embarked unit at home; no dry road opens within {} steps",
                    wet, STAGE_DRY_LIMIT;
                    city.pos);
                return self.base.fortify_or_stop(g, pid, uid);
            }
            let marched = match dry_march {
                StageMarch::Dry { step, .. } => Some(step),
                _ => march_step(g, uid, city.pos, STAGING_FAR),
            };
            if let Some(next) = marched
                .filter(|pos| g.can_move(uid, *pos) && g.wdist(*pos, city.pos) > CITY_STRIKE_RANGE)
            {
                // `staging-column-passes-through`: a march step that brings
                // the unit no nearer is the router going round one of ours
                // in the defile: only the first step must be a tile it may
                // stop on, so a friend in the gap turns the route sideways,
                // and from the side tile the route turns back. Live King
                // civvis-20261005T003728Z (game 89): Mashhad, Persia's
                // capital, held in Stage from turn 183 to 225 with its
                // artillery stepping (26,17) -> (26,18) -> (26,17) every
                // frame behind a Rocket Artillery in the one gap of a
                // mountain ridge, twelve to fourteen tiles out. Cross the
                // friend instead, to the open tile beyond. A dry road round
                // the water leads no nearer by design, and the crossing's
                // destination is read by straight distance, toward the water.
                let on_dry_road = matches!(dry_march, StageMarch::Dry { .. });
                if self.staging_column_passes_through
                    && !on_dry_road
                    && g.wdist(next, city.pos) >= distance
                {
                    if let Some(dest) = g.pass_through_destination(uid, city.pos, STAGING_FAR) {
                        let kind = g.units[&uid].kind;
                        if gun_danger.as_mut().is_none_or(|field| {
                            if remembers {
                                field.danger(dest, uid) + fog_blow(g, dest) <= gun_risk_limit
                            } else {
                                field.danger(dest, uid) <= gun_risk_limit
                            }
                        }) && self.base.path_walk_to(g, pid, uid, dest)
                        {
                            think!(self.journal(), Military, Detail,
                                "Siege of {}: the {} crosses its own column toward the staging ring", g.cities[&city.id].name, kind;
                                "its march step led no nearer than {} tiles, round one of ours in the gap; {:?} stands {} tiles out",
                                distance, dest, g.wdist(dest, city.pos);
                                city.pos);
                            return true;
                        }
                    }
                }
                if let Some(field) = gun_danger.as_mut() {
                    let mut risk_at = |g: &Game, pos: Pos| -> f64 {
                        let seen = field.danger(pos, uid);
                        if remembers {
                            seen + fog_blow(g, pos)
                        } else {
                            seen
                        }
                    };
                    if risk_at(g, next) > escorted_limit(g, next) {
                        if remembers && fog_blow(g, next) > 0.0 {
                            think!(self.journal(), Military, Detail,
                                "Siege of {}: the {} holds short of a hostile seen in the fog", g.cities[&city.id].name, g.units[&uid].kind;
                                "{:?} lies in the reach of one the seat saw this turn or last; its blow there reads {:.0} against a limit of {:.0}",
                                next, fog_blow(g, next), escorted_limit(g, next);
                                city.pos);
                        }
                        let safe = g
                            .nbrs(here)
                            .into_iter()
                            .filter(|pos| {
                                g.can_move(uid, *pos)
                                    && g.wdist(*pos, city.pos) > CITY_STRIKE_RANGE
                                    && g.wdist(*pos, city.pos) <= distance
                            })
                            .map(|pos| (g.wdist(pos, city.pos), risk_at(g, pos), pos))
                            .filter(|(_, risk, _)| *risk <= gun_risk_limit)
                            .min_by(|a, b| {
                                a.0.cmp(&b.0)
                                    .then_with(|| a.1.total_cmp(&b.1))
                                    .then_with(|| a.2.cmp(&b.2))
                            });
                        if let Some((_, _, pos)) = safe {
                            return self.base.tactical_apply_move(g, pid, uid, pos);
                        }
                        return self.base.fortify_or_stop(g, pid, uid);
                    }
                }
                if let StageMarch::Dry { dry, wet, .. } = dry_march {
                    think!(self.journal(), Military, Detail,
                        "Siege of {}: the {} takes the land road toward the staging ring", g.cities[&city.id].name, g.units[&uid].kind;
                        "the march crosses water in {} steps and come-ashore lands an embarked unit at home; the dry road runs {} steps, first {:?}",
                        wet, dry, next;
                        city.pos);
                }
                return self.base.tactical_apply_move(g, pid, uid, next);
            }
            // A staging column can fill every legal adjacent stopping tile.
            // Walk through a friendly screen to an open tile beyond it, using
            // the same whole-path legality and movement bookkeeping as the
            // general mover. The destination remains outside the strike ring.
            // Not on a dry road: the crossing's destination is read by
            // straight distance, toward the water.
            if let Some(dest) = (!matches!(dry_march, StageMarch::Dry { .. }))
                .then(|| g.pass_through_destination(uid, city.pos, STAGING_FAR))
                .flatten()
            {
                if gun_danger.as_mut().is_none_or(|field| {
                    if remembers {
                        field.danger(dest, uid) + fog_blow(g, dest) <= gun_risk_limit
                    } else {
                        field.danger(dest, uid) <= gun_risk_limit
                    }
                }) && self.base.path_walk_to(g, pid, uid, dest)
                {
                    return true;
                }
            }
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// Invest and Reduce, melee: hold the ring and fortify; swing at the
    /// wall only when it is low or a ram or tower stands by; otherwise take
    /// the next ring tile, spread first.
    fn siege_melee_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
        plan: &StrategicPlan,
    ) -> bool {
        let here = g.units[&uid].pos;
        let distance = g.wdist(here, city.pos);
        if distance <= 1 {
            let allow_city = melee_wall_attack_allowed(g, pid, uid, city.id);
            if let Some(acted) = self.siege_blow(g, pid, uid, city, plan, allow_city) {
                return acted;
            }
            return self.base.fortify_or_stop(g, pid, uid);
        }
        if let Some(acted) = self.post_step(g, pid, uid, city) {
            return acted;
        }
        if let Some(acted) = self.siege_blow(g, pid, uid, city, plan, false) {
            return acted;
        }
        if distance > CITY_STRIKE_RANGE {
            if let Some(next) =
                march_step(g, uid, city.pos, CITY_STRIKE_RANGE).filter(|pos| g.can_move(uid, *pos))
            {
                return self.base.tactical_apply_move(g, pid, uid, next);
            }
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// Invest and Reduce, siege: a killable reliever, else the city — walls
    /// first by the engine's routing — else a firing tile at range behind
    /// the ring. A siege unit that moved cannot fire this turn and holds.
    fn siege_gun_step(&mut self, g: &mut Game, pid: usize, uid: u32, city: &CityView) -> bool {
        let unit = g.units[&uid].clone();
        if unit.attacks_left <= 0 {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        let range = g.unit_attack_range(uid).max(1);
        let distance = g.wdist(unit.pos, city.pos);
        let can_fire = unit.moves_left > 0.0
            && !(unit.moved && !g.siege_may_attack_after_moving(&unit));
        if distance <= range && can_fire {
            if let Some(acted) = self.reliever_kill_shot(g, pid, uid, city) {
                return acted;
            }
            if let Some(acted) = self.city_shot(g, pid, uid, city) {
                return acted;
            }
        }
        if let Some(acted) = self.post_step(g, pid, uid, city) {
            return acted;
        }
        if let Some(acted) = self.close_to_staging(g, pid, uid, city) {
            return acted;
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// A gun or shooter with no firing post this turn, still beyond the
    /// staging ring, walks up to it rather than fortifying where it stands.
    /// The firing band holds only so many tiles; the rest of a twenty-unit
    /// train used to hold at home for the whole siege. Live King
    /// civvis-20261001T080758Z: the Siege of Pella's force of 16-20 read
    /// 25-50% ready from turn 94 to 110 with one to three units within four
    /// tiles. Units already on the ring wait there for a post to free.
    fn close_to_staging(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
    ) -> Option<bool> {
        if g.wdist(g.units[&uid].pos, city.pos) <= STAGING_FAR {
            return None;
        }
        // `stage-march-keeps-to-land`: see `StageMarch`.
        let dry_march = if self.stage_march_keeps_to_land && self.base.come_ashore {
            stage_march(g, uid, city.pos, STAGING_FAR)
        } else {
            StageMarch::Ordinary
        };
        let next = match dry_march {
            StageMarch::Ordinary => march_step(g, uid, city.pos, STAGING_FAR),
            StageMarch::Dry { step, .. } => Some(step),
            StageMarch::Hold { .. } => return Some(self.base.fortify_or_stop(g, pid, uid)),
        }
        .filter(|pos| g.can_move(uid, *pos))?;
        Some(self.base.tactical_apply_move(g, pid, uid, next))
    }

    /// Invest and Reduce, shooters: a killable reliever, then units while
    /// the wall stands and the city once it is down, else a firing tile at
    /// range behind the ring.
    fn siege_shooter_step(&mut self, g: &mut Game, pid: usize, uid: u32, city: &CityView) -> bool {
        let unit = g.units[&uid].clone();
        if unit.attacks_left <= 0 {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        let range = g.unit_attack_range(uid).max(1);
        let distance = g.wdist(unit.pos, city.pos);
        if distance <= range && unit.moves_left > 0.0 {
            if city.wall_hp > 0 {
                if let Some(acted) = self.reliever_kill_shot(g, pid, uid, city) {
                    return acted;
                }
                if let Some(acted) = self.best_unit_shot(g, pid, uid) {
                    return acted;
                }
                if let Some(acted) = self.city_shot(g, pid, uid, city) {
                    return acted;
                }
            } else {
                // A city without walls is the shot: every reliever killed
                // instead let it heal. Live King civvis-20261003T035351Z,
                // unwalled Washington, turns 40-58 in Reduce with four to
                // nine units near: our archers shot units about sixty times
                // and the city five, and it stood at 200/200 until it built
                // walls at turn 59.
                if let Some(acted) = self.city_shot(g, pid, uid, city) {
                    return acted;
                }
                if let Some(acted) = self.reliever_kill_shot(g, pid, uid, city) {
                    return acted;
                }
                if let Some(acted) = self.best_unit_shot(g, pid, uid) {
                    return acted;
                }
            }
        }
        if let Some(acted) = self.post_step(g, pid, uid, city) {
            return acted;
        }
        if let Some(acted) = self.close_to_staging(g, pid, uid, city) {
            return acted;
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// The taker: onto the ring, then hold, reserved, until the city is
    /// within its blow — then the attack that is the capture.
    fn taker_step(&mut self, g: &mut Game, pid: usize, uid: u32, city: &CityView) -> bool {
        let unit = g.units[&uid].clone();
        if g.wdist(unit.pos, city.pos) > 1 {
            if let Some(acted) = self.post_step(g, pid, uid, city) {
                return acted;
            }
            return self.base.fortify_or_stop(g, pid, uid);
        }
        if unit.attacks_left > 0 && unit.moves_left > 0.0 {
            let blow = taker_blow(g, pid, uid, city.id);
            let action = if city.hp <= 0 && g.can_move(uid, city.pos) {
                Some(Action::Move {
                    unit: uid,
                    to: city.pos,
                })
            } else if city.hp <= 0 || f64::from(city.hp) <= blow {
                Some(Action::Attack {
                    unit: uid,
                    target: city.pos,
                })
            } else {
                None
            };
            if let Some(action) = action {
                if g.apply(pid, &action).is_ok() {
                    self.force_groups_dirty = true;
                    if g.cities.get(&city.id).is_some_and(|c| c.owner == pid) {
                        if let Some(siege) = self.sieges.get_mut(&city.id) {
                            siege.stage = SiegeStage::Hold;
                            siege.entered = g.turn;
                            siege.taker = None;
                        }
                        self.reserved_units.remove(&uid);
                        self.census.siege_captures += 1;
                        think!(self.journal(), Military, Decision,
                            "Siege of {}: taken by the {}", g.cities[&city.id].name, unit.kind;
                            "the city was at {} behind {} of wall against an expected blow of {blow:.0}",
                            city.hp, city.wall_hp;
                            city.pos);
                    }
                    return true;
                }
            }
        }
        if self.safe_taker_pressure(g, pid, uid, city) {
            return true;
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// `breach-assault`: a healthy melee unit beside a city whose walls no
    /// longer shield it (down, or opened by a ram or siege tower beside the
    /// attacker) strikes it once the whole force's blows can take the city
    /// within [`ASSAULT_TURNS`], even though one blow alone trades badly.
    ///
    /// ★★★ ONE BLOW PRICED ALONE NEVER OPENS AN ASSAULT. `siege_blow` keeps a
    /// melee attack only when its own exchange pays, and a Warrior against a
    /// full capital never does, so the ring held while two Archers shot. Live
    /// King 20261004T111442Z (game 53): Madrid stood without walls from turn
    /// 44 to 66 at 186-200 health with five to eight units staged and the
    /// siege reading "damage ready"; it healed every turn, built walls at 68,
    /// and never fell. The same shape: Washington turns 40-58 (035351Z),
    /// Tenochtitlan (110427Z-cont1). And with a ram: Live King
    /// 20261004T113554Z (game 54) besieged the Cree's Wihkasko-Kiseyin from
    /// turn 73, a Battering Ram riding with the train, and its walls stood at
    /// 100/100 at turn 83 while the Cree won by religion at 90; their two
    /// cities were the whole founder. Each melee unit here must start at
    /// [`ASSAULT_MIN_HP`] and survive the reply at [`ASSAULT_SURVIVOR_HP`];
    /// the reserved taker still finishes.
    fn breach_assault_blow(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Option<bool> {
        if self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
            || city.hp <= 0
            || (city.wall_hp > 0 && !melee_wall_attack_allowed(g, pid, uid, city.id))
        {
            return None;
        }
        let unit = g.units.get(&uid)?.clone();
        if g.wdist(unit.pos, city.pos) > 1 {
            // `breach-assault-closes-in`: a unit off the ring steps in toward
            // a city the force's blows can take; it strikes from beside it.
            if !self.breach_assault_closes_in
                || unit.moves_left <= 0.0
                || !self.closing_in(g, uid, city)
            {
                return None;
            }
            let volley = self.assault_volley(g, pid, city, plan, group);
            let to_take = f64::from(city.hp + city.wall_hp.max(0)) + ASSAULT_HEAL;
            if volley * ASSAULT_TURNS < to_take {
                return None;
            }
            let next = march_step(g, uid, city.pos, 1).filter(|pos| g.can_move(uid, *pos))?;
            if !self.base.tactical_apply_move(g, pid, uid, next) {
                return None;
            }
            think!(self.journal(), Military, Decision,
                "Siege of {}: the {} closes in for the assault", g.cities[&city.id].name, unit.kind;
                "the force's blows come to {volley:.0} a turn against {} health with no wall standing",
                city.hp;
                city.pos);
            return Some(true);
        }
        if unit.attacks_left <= 0
            || unit.moves_left <= 0.0
            || unit.hp < ASSAULT_MIN_HP
            || g.is_embarked(&unit)
            || !g.melee_order_is_legal(pid, uid, city.pos)
        {
            return None;
        }
        let volley = self.assault_volley(g, pid, city, plan, group);
        let to_take = f64::from(city.hp + city.wall_hp.max(0)) + ASSAULT_HEAL;
        if volley * ASSAULT_TURNS < to_take {
            return None;
        }
        let action = Action::Attack {
            unit: uid,
            target: city.pos,
        };
        let mut after = g.speculative_clone();
        after.apply(pid, &action).ok()?;
        if after.units.get(&uid).is_none_or(|survivor| survivor.hp < ASSAULT_SURVIVOR_HP) {
            return None;
        }
        g.apply(pid, &action).ok()?;
        self.force_groups_dirty = true;
        let captured = g.cities.get(&city.id).is_some_and(|c| c.owner == pid);
        if captured {
            if let Some(siege) = self.sieges.get_mut(&city.id) {
                if let Some(taker) = siege.taker.take() {
                    self.reserved_units.remove(&taker);
                }
                siege.stage = SiegeStage::Hold;
                siege.entered = g.turn;
            }
            self.census.siege_captures += 1;
        }
        let left = g.cities.get(&city.id).map_or(0, |c| if c.owner == pid { 0 } else { c.hp });
        think!(self.journal(), Military, Decision,
            "Siege of {}: the {} joins the assault", g.cities[&city.id].name, unit.kind;
            "the force's blows this turn come to {volley:.0} against {} health behind {} of wall; {} left; captured {captured}",
            city.hp, city.wall_hp, left;
            city.pos);
        Some(true)
    }

    /// `breach-assault-closes-in`: whether this healthy melee unit, off the
    /// ring but within [`CLOSING_REACH`] of a city with no wall standing, is
    /// one the assault can call in: a free land tile beside the city is open
    /// to it. The ring's melee wait outside the city's strike
    /// (`siege_melee_step` marches them only to [`CITY_STRIKE_RANGE`]), and
    /// `breach_assault_blow` and `assault_volley` counted only melee already
    /// beside the city, so an unwalled city whose ranged damage healed away
    /// never drew the assault. Zone of control ends a move beside the city,
    /// so the step in and the blow fall on different turns. Live King
    /// civvis-20261004T232618Z (game 86): The Hague stood without walls at
    /// 95-134 health from turn 88 to 95 beside eleven staged units; horsemen
    /// waited three tiles out with five moves left, the volley read 32.5
    /// against 95-134, and the city built walls at 96.
    fn closing_in(&self, g: &Game, uid: u32, city: &CityView) -> bool {
        if city.wall_hp > 0 {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let distance = g.wdist(unit.pos, city.pos);
        unit.hp >= ASSAULT_MIN_HP
            && !g.is_embarked(unit)
            && (2..=CLOSING_REACH).contains(&distance)
            && g.nbrs(city.pos).into_iter().any(|pos| {
                g.unit_ids_at(pos).is_empty()
                    && g.map
                        .get(pos)
                        .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
            })
    }

    /// The city damage the siege force can still deal this turn: ranged
    /// members in range at the land ranged-against-districts penalty, and
    /// healthy melee members already beside the city — or, under
    /// `breach-assault-closes-in`, ones the assault can call in from off the
    /// ring (`closing_in`).
    fn assault_volley(
        &self,
        g: &Game,
        pid: usize,
        city: &CityView,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> f64 {
        let defense = g.city_strength(city.id);
        self.siege_force(g, pid, city, plan, group)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| unit.attacks_left > 0 && unit.moves_left > 0.0 && !g.is_embarked(unit))
            .map(|unit| {
                let spec = &g.rules.units[unit.kind];
                let distance = g.wdist(unit.pos, city.pos);
                if spec.has_ranged_attack() {
                    if distance > g.unit_attack_range(unit.id).max(1) {
                        return 0.0;
                    }
                    let mut attack = g.unit_ranged_attack_strength(unit);
                    if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
                        attack += g.promotion_effect(unit, "ranged_vs_district") - 17.0;
                    }
                    crate::game::expected_damage(attack, defense)
                } else if spec.is_melee_capable()
                    && unit.hp >= ASSAULT_MIN_HP
                    && (distance <= 1
                        || (self.breach_assault_closes_in && self.closing_in(g, unit.id, city)))
                {
                    crate::game::expected_damage(g.unit_strength(unit, false), defense)
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// A healthy capture unit can contribute before the final blow when an
    /// unwalled city would otherwise heal away the siege's ranged damage.
    /// Keep enough health to survive the reply and remain a capture body.
    fn safe_taker_pressure(&mut self, g: &mut Game, pid: usize, uid: u32, city: &CityView) -> bool {
        let unit = &g.units[&uid];
        if self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
            || city.wall_hp > 0
            || unit.hp < 80
            || unit.attacks_left <= 0
            || unit.moves_left <= 0.0
            || g.is_embarked(unit)
            || !g.melee_order_is_legal(pid, uid, city.pos)
        {
            return false;
        }
        let action = Action::Attack {
            unit: uid,
            target: city.pos,
        };
        let before_hp = unit.hp;
        let mut after = g.speculative_clone();
        if after.apply(pid, &action).is_err() {
            return false;
        }
        let (Some(survivor), Some(target)) = (after.units.get(&uid), after.cities.get(&city.id))
        else {
            return false;
        };
        let dealt = city.hp - target.hp;
        let taken = before_hp - survivor.hp;
        if dealt <= 20 || dealt <= taken {
            return false;
        }
        let reply = super::battle_planner::strike_danger(&after, pid, survivor.pos, uid);
        if f64::from(survivor.hp) - reply < 60.0 {
            return false;
        }
        if g.apply(pid, &action).is_err() {
            return false;
        }
        self.force_groups_dirty = true;
        if g.cities[&city.id].owner == pid {
            if let Some(siege) = self.sieges.get_mut(&city.id) {
                siege.stage = SiegeStage::Hold;
                siege.entered = g.turn;
                siege.taker = None;
            }
            self.reserved_units.remove(&uid);
            self.census.siege_captures += 1;
        }
        think!(self.journal(), Military, Decision,
            "Siege of {}: the reserved {} helps reduce the unwalled city", g.cities[&city.id].name, g.units[&uid].kind;
            "{dealt} city damage for {taken} immediate damage; at least 60 hp remain after the predicted reply";
            city.pos);
        true
    }

    /// Toward the unit's post for the turn, when it has one it is not on.
    /// `None` when it has none or already stands there, so the caller holds.
    fn post_step(&mut self, g: &mut Game, pid: usize, uid: u32, city: &CityView) -> Option<bool> {
        let post = *self.sieges.get(&city.id)?.posts.get(&uid)?;
        if g.units[&uid].pos == post {
            return None;
        }
        self.approach(g, pid, uid, post, city.pos)
    }

    /// Follow a legal route to the assigned post while excluding the city's
    /// other ring tiles and occupied stands. Geometric distance may increase
    /// around a mountain or a friendly screen; route distance still decreases.
    /// The engine checks the first step and the movement guard rejects retreads.
    fn approach(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        goal: Pos,
        city_pos: Pos,
    ) -> Option<bool> {
        let mut moved = false;
        for _ in 0..4 {
            let unit = g.units.get(&uid)?;
            if unit.pos == goal || unit.moves_left <= 0.0 {
                break;
            }
            let Some(next) =
                siege_route_step(g, pid, uid, goal, city_pos).filter(|next| g.can_move(uid, *next))
            else {
                // The step-by-step route treats our own soldiers as walls, so
                // a crowded staging band boxes a gun in behind its own army.
                // Walk through them to the free post in one move, which may
                // pass friendly units. Live King civvis-20261003T093332Z: two
                // of three Bombards before Natal held posts for turns 160-170
                // and stood five to seven tiles out, never firing, while the
                // walls went 400 -> 68 under one Bombard's fire.
                let hp = f64::from(unit.hp);
                if let Some(dest) = (!moved)
                    .then(|| g.pass_through_destination(uid, goal, 0))
                    .flatten()
                    .filter(|dest| {
                        dry_stand(g, uid, *dest)
                            && (g.wdist(*dest, city_pos) > CITY_STRIKE_RANGE
                                || hp > self.approach_danger(g, pid, *dest, uid) + 20.0)
                    })
                {
                    moved = self.base.path_walk_to(g, pid, uid, dest);
                }
                break;
            };
            // A post inside the city's firing ring can be covered by more
            // than one city or Encampment. Do not march a body into a stand
            // where the forward model expects the next volley to finish it.
            if g.wdist(next, city_pos) <= CITY_STRIKE_RANGE
                && f64::from(unit.hp) <= self.approach_danger(g, pid, next, uid) + 20.0
            {
                break;
            }
            if !self.base.tactical_apply_move(g, pid, uid, next) {
                break;
            }
            moved = true;
        }
        moved.then_some(true)
    }

    /// The reply a unit expects on a post inside the city's firing ring.
    /// With `shared-danger` on, each hostile's blow is split among our land
    /// units in its reach, never below the strongest single blow, as the
    /// battle planner's rotation reads it. The full sum charged every
    /// defender to every gun. Live King civvis-20261001T022028Z: around
    /// walled Cairo, the guns of an 18-unit train stood three and four tiles
    /// out for the 22 turns of Invest, and the city was struck three times.
    fn approach_danger(&self, g: &Game, pid: usize, tile: Pos, uid: u32) -> f64 {
        if !self.shared_danger {
            return super::battle_planner::strike_danger(g, pid, tile, uid);
        }
        let mut field = super::battle_planner::DangerField::with_reach(g, pid, true);
        field.share(g);
        field.rotation_danger(tile, uid)
    }

    /// Walk to the first goal reachable this turn, else one step toward the
    /// first routable one — a step that does not put the goal further off,
    /// so a unit whose route is re-read after each step cannot walk out and
    /// back within the turn.
    fn move_toward_any(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        goals: &[Pos],
    ) -> Option<bool> {
        let here = g.units[&uid].pos;
        let reachable = g.reachable(uid);
        for goal in goals {
            if reachable.contains(goal) && self.base.path_walk_to(g, pid, uid, *goal) {
                return Some(true);
            }
        }
        for goal in goals {
            let set: HashSet<Pos> = std::iter::once(*goal).collect();
            if let Some(next) = g
                .route_step_to_any(uid, &set)
                .filter(|pos| g.can_move(uid, *pos) && g.wdist(*pos, *goal) <= g.wdist(here, *goal))
            {
                return Some(self.base.tactical_apply_move(g, pid, uid, next));
            }
        }
        None
    }

    /// The best blow this unit has on a hostile unit in its reach — and on
    /// the city when `allow_city` — priced on one speculative clone through
    /// the ladder's exact forward model. Taken when it kills or the exchange
    /// is positive and the attacker survives. `None` when nothing qualifies.
    fn siege_blow(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
        plan: &StrategicPlan,
        allow_city: bool,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let (ranged, melee, siege) = {
            let spec = &g.rules.units[unit.kind];
            (
                spec.has_ranged_attack(),
                spec.is_melee_capable(),
                spec.siege,
            )
        };
        if ranged && siege && unit.moved && !g.siege_may_attack_after_moving(&unit) {
            return None;
        }
        let radius = if ranged {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        let frame = g.player_vision_frame(pid);
        let viewers = g.visibility_viewers(pid);
        let mut actions: Vec<Action> = Vec::new();
        for pos in g.wdisk(unit.pos, radius) {
            if pos == unit.pos {
                continue;
            }
            let is_city = pos == city.pos;
            if is_city {
                if !allow_city {
                    continue;
                }
            } else if strongest_hostile_at(g, pid, pos).is_none() {
                continue;
            }
            if ranged && g.ranged_order_is_legal(pid, uid, pos, frame.as_ref(), &viewers) {
                actions.push(Action::Ranged {
                    unit: uid,
                    target: pos,
                });
            }
            if melee && g.melee_order_is_legal(pid, uid, pos) {
                actions.push(Action::Attack {
                    unit: uid,
                    target: pos,
                });
            }
        }
        let mut best: Option<(bool, f64, Action)> = None;
        for action in actions {
            let mut board = g.speculative_clone();
            let (result, applied) =
                Self::tactical_attack_result_in(&mut board, pid, uid, &action, plan);
            if !matches!(applied, AppliedAttack::Applied) || !result.attacker_survives {
                continue;
            }
            if !(result.eliminates_enemy_unit || result.value > 0.0) {
                continue;
            }
            let better = best.as_ref().is_none_or(|(kill, value, _)| {
                (result.eliminates_enemy_unit && !*kill)
                    || (result.eliminates_enemy_unit == *kill && result.value > *value)
            });
            if better {
                best = Some((result.eliminates_enemy_unit, result.value, action));
            }
        }
        let (_, _, action) = best?;
        if g.apply(pid, &action).is_err() {
            return None;
        }
        self.force_groups_dirty = true;
        Some(true)
    }

    /// A ranged blow that finishes a reliever within [`RELIEVER_RADIUS`] of
    /// the city with [`KILL_MARGIN`], lowest hit points first.
    fn reliever_kill_shot(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
    ) -> Option<bool> {
        self.ranged_shot(g, pid, uid, Some(city.pos), true)
    }

    /// The ranged blow on a hostile unit doing the most, a kill first.
    fn best_unit_shot(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        self.ranged_shot(g, pid, uid, None, false)
    }

    fn ranged_shot(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        near_city: Option<Pos>,
        kills_only: bool,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let range = g.unit_attack_range(uid).max(1);
        let frame = g.player_vision_frame(pid);
        let viewers = g.visibility_viewers(pid);
        let mut best: Pick<ShotKey> = None;
        for pos in g.wdisk(unit.pos, range) {
            if pos == unit.pos || near_city.is_some_and(|c| g.wdist(pos, c) > RELIEVER_RADIUS) {
                continue;
            }
            let Some(defender) = strongest_hostile_at(g, pid, pos) else {
                continue;
            };
            if !g.ranged_order_is_legal(pid, uid, pos, frame.as_ref(), &viewers) {
                continue;
            }
            let Some((att, def)) = g.ranged_strike_strengths(uid, defender, pos) else {
                continue;
            };
            let hp = g.units[&defender].hp;
            let dealt = expected_damage(att, def);
            let kill = dealt >= f64::from(hp) * KILL_MARGIN;
            if kills_only && !kill {
                continue;
            }
            let key = (
                kill,
                (dealt * 100.0).round() as i64,
                Reverse(hp),
                Reverse(pos),
            );
            if best.as_ref().is_none_or(|(old, _)| key > *old) {
                best = Some((key, pos));
            }
        }
        let (_, target) = best?;
        if g.apply(pid, &Action::Ranged { unit: uid, target }).is_err() {
            return None;
        }
        self.force_groups_dirty = true;
        Some(true)
    }

    /// A shot at the City Center, when the engine will accept it.
    fn city_shot(&mut self, g: &mut Game, pid: usize, uid: u32, city: &CityView) -> Option<bool> {
        let frame = g.player_vision_frame(pid);
        let viewers = g.visibility_viewers(pid);
        if !g.ranged_order_is_legal(pid, uid, city.pos, frame.as_ref(), &viewers) {
            return None;
        }
        if g.apply(
            pid,
            &Action::Ranged {
                unit: uid,
                target: city.pos,
            },
        )
        .is_err()
        {
            return None;
        }
        self.force_groups_dirty = true;
        Some(true)
    }

    /// `anvil`: the formation around a threatened city of ours, for the land
    /// group nearest it. `None` for every other unit and with the gene off.
    fn anvil_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Option<bool> {
        let cid = plan.threatened_city?;
        let city = CityView::of(g, cid).filter(|city| city.owner == pid)?;
        let nearest = self
            .force_groups
            .iter()
            .filter(|other| other.domain == ForceDomain::Land)
            .min_by_key(|other| (g.wdist(other.anchor, city.pos), other.id))
            .map(|other| other.id)?;
        if group.id != nearest || g.wdist(group.anchor, city.pos) > THREAT_RELIEF_RADIUS {
            return None;
        }
        let hostiles = hostiles_near(g, pid, city.pos, ANVIL_HOSTILE_RADIUS);
        if self.anvil_orders_turn != Some((g.turn, cid)) {
            let heals = !g.is_arena() || g.tactics.heal;
            self.anvil_orders = anvil_orders_for(g, pid, &city, &group.units, &hostiles, heals);
            self.anvil_orders_turn = Some((g.turn, cid));
            self.anvil_rotate(g, pid, &city);
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let on_ring = self
                    .anvil_orders
                    .values()
                    .filter(|pos| g.wdist(**pos, city.pos) == 1)
                    .count();
                let garrison = self
                    .anvil_orders
                    .iter()
                    .find(|(_, pos)| **pos == city.pos)
                    .map(|(uid, _)| g.units[uid].kind.to_string())
                    .unwrap_or_else(|| "nobody".to_string());
                think!(self.journal(), Military, Decision,
                    "Anvil at {}: {} on the ring, {garrison} in the city", g.cities[&cid].name, on_ring;
                    "{} of the force posted, {} hostile tile(s) within {ANVIL_HOSTILE_RADIUS}",
                    self.anvil_orders.len(), hostiles.len();
                    city.pos);
            }
        }
        let post = *self.anvil_orders.get(&uid)?;
        self.census.anvil_turns += 1;
        let unit = g.units[&uid].clone();
        if unit.pos == post {
            if let Some(acted) = self.anvil_blow(g, pid, uid) {
                return Some(acted);
            }
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        // A post one of ours still stands on is approached and then waited
        // for — the rotation or the occupant's own move opens it — never
        // walked at.
        let held_by_ours = g
            .unit_ids_at(post)
            .iter()
            .any(|id| g.units[id].owner == pid);
        if !held_by_ours {
            if let Some(acted) = self.move_toward_any(g, pid, uid, &[post]) {
                return Some(acted);
            }
        } else if g.wdist(unit.pos, post) > 1 {
            if let Some(next) = g
                .route_step(uid, post, 1)
                .filter(|pos| g.can_move(uid, *pos))
            {
                return Some(self.base.tactical_apply_move(g, pid, uid, next));
            }
        }
        if let Some(acted) = self.anvil_blow(g, pid, uid) {
            return Some(acted);
        }
        Some(self.base.fortify_or_stop(g, pid, uid))
    }

    /// `anvil`: the rotations, executed the moment the posts are drawn —
    /// before any unit has spent its movement fortifying — so a wounded
    /// unit posted to the city and standing beside it trades places with
    /// the fresh unit there, which takes its tile.
    fn anvil_rotate(&mut self, g: &mut Game, pid: usize, city: &CityView) {
        let wounded: Vec<u32> = self
            .anvil_orders
            .iter()
            .filter(|(uid, post)| {
                **post == city.pos
                    && g.units.get(uid).is_some_and(|unit| {
                        unit.pos != city.pos && g.wdist(unit.pos, city.pos) == 1
                    })
            })
            .map(|(uid, _)| *uid)
            .collect();
        for uid in wounded {
            let Some(unit) = g.units.get(&uid).cloned() else {
                continue;
            };
            let occupant = g
                .unit_ids_at(city.pos)
                .iter()
                .copied()
                .find(|id| g.units[id].owner == pid && arm_of(g, *id) != Arm::Other);
            let Some(other) = occupant else {
                continue;
            };
            if g.units[&other].hp < unit.hp + ANVIL_RELIEF_MARGIN
                || g.apply(pid, &Action::Swap { unit: uid, other }).is_err()
            {
                continue;
            }
            self.census.anvil_rotations += 1;
            self.force_groups_dirty = true;
            self.anvil_orders.insert(other, unit.pos);
            think!(self.journal(), Military, Decision,
                "Anvil at {}: the {} rotates into the city", g.cities[&city.id].name, unit.kind;
                "{} hp; the {} takes its tile", unit.hp, g.units[&other].kind;
                city.pos);
        }
    }

    /// `anvil`: engage only when the exchange favours us — any shot (a shot
    /// has no return), a melee blow that deals more than it takes and
    /// leaves the unit standing.
    fn anvil_blow(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let (ranged, melee) = {
            let spec = &g.rules.units[unit.kind];
            (spec.has_ranged_attack(), spec.is_melee_capable())
        };
        if ranged {
            return self.best_unit_shot(g, pid, uid);
        }
        if !melee {
            return None;
        }
        let mut best: Pick<BlowKey> = None;
        for pos in g.nbrs(unit.pos) {
            let Some(defender) = strongest_hostile_at(g, pid, pos) else {
                continue;
            };
            if !g.melee_order_is_legal(pid, uid, pos) {
                continue;
            }
            let Some((att, def)) = g.melee_exchange_strengths(uid, defender) else {
                continue;
            };
            let dealt = expected_damage(att, def);
            let taken = expected_damage(def, att);
            let kill = dealt >= f64::from(g.units[&defender].hp) * KILL_MARGIN;
            if taken >= f64::from(unit.hp) || !(kill || dealt > taken) {
                continue;
            }
            let key = (kill, ((dealt - taken) * 100.0).round() as i64, Reverse(pos));
            if best.as_ref().is_none_or(|(old, _)| key > *old) {
                best = Some((key, pos));
            }
        }
        let (_, target) = best?;
        if g.apply(pid, &Action::Attack { unit: uid, target }).is_err() {
            return None;
        }
        self.force_groups_dirty = true;
        Some(true)
    }
}

impl AdvancedAi {
    /// `staging-gun-remembers-hostiles`: every land melee hostile at war with
    /// `pid` that the seat saw within [`REMEMBERED_STRIKER_TURNS`] turns and
    /// cannot see now, as a [`RememberedStriker`] against the gun `uid`. A
    /// unit still in sight is the danger field's own and is left out, so the
    /// two never count one hostile twice. Live King civvis-20261005T033442Z
    /// (game 96): the Khmer Cuirassiers that one-shot two Trebuchets at turns
    /// 145 and 146 stood in sight at turn 144 and in the fog at 145, when the
    /// Stage march read 0 to 7 danger on the tiles it walked the guns to.
    pub(super) fn remembered_strikers(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Vec<RememberedStriker> {
        let Some(gun) = g.units.get(&uid) else {
            return Vec::new();
        };
        let defence = effective_strength(g.unit_strength(gun, true), gun.hp);
        let visible = g.player_vision_frame(pid);
        let in_sight: BTreeSet<i64> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid && g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| super::hostile_memory_key(g, unit))
            .collect();
        self.hostile_last_seen
            .iter()
            .filter_map(|(key, record)| {
                if in_sight.contains(key)
                    || record.owner >= g.players.len()
                    || record.owner == pid
                    || !g.is_at_war(pid, record.owner)
                    || record.when > g.turn
                    || g.turn - record.when > REMEMBERED_STRIKER_TURNS
                {
                    return None;
                }
                let spec = &g.rules.units[record.kind];
                if spec.class != "military"
                    || !spec.is_melee_capable()
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                {
                    return None;
                }
                let moves = (spec.moves.ceil() as i32).max(1);
                let elapsed = (g.turn - record.when) as i32;
                Some(RememberedStriker {
                    from: record.pos,
                    reach: moves * (elapsed + 1),
                    blow: expected_damage(effective_strength(spec.strength, 100), defence),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod ownership_tests;

#[cfg(test)]
mod capture_tests;

#[cfg(test)]
mod breach_tests;

#[cfg(test)]
mod breaker_tests;

#[cfg(test)]
mod landing_tests;

#[cfg(test)]
mod firing_tests;

#[cfg(test)]
mod staging_tests;

#[cfg(test)]
mod assault_tests;

#[cfg(test)]
mod tests {
    use super::super::GrandStrategy;
    use super::*;
    use crate::doctrine::{build, position};

    /// `the_storming`'s board with its army removed: a 200-hit-point city
    /// of player 1 behind 100 points of wall, and nothing else.
    pub(super) fn walled_city() -> (Game, u32) {
        let mut g = build(position("the_storming").expect("known"), 3).expect("buildable");
        let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
        for uid in seeded {
            g.remove_unit(uid);
        }
        let cid = *g.cities.keys().next().expect("the position states a city");
        assert_eq!(g.cities[&cid].owner, 1);
        assert_eq!((g.cities[&cid].hp, g.cities[&cid].wall_hp), (200, 100));
        (g, cid)
    }

    pub(super) fn plan_against(g: &Game, cid: u32) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(cid),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    fn plan_holding(g: &Game, cid: u32) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Recovery,
            target_player: Some(0),
            target_city: None,
            threatened_city: Some(cid),
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    /// The ring tiles of a city, sorted.
    pub(super) fn ring_of(g: &Game, cid: u32) -> Vec<Pos> {
        let pos = g.cities[&cid].pos;
        let mut ring: Vec<Pos> = g
            .wdisk(pos, 1)
            .into_iter()
            .filter(|p| *p != pos && g.map.get(*p).is_some_and(|t| g.rules.is_passable(t)))
            .collect();
        ring.sort_unstable();
        ring
    }

    /// Tiles at exactly `distance` from the city, sorted.
    pub(super) fn at_distance(g: &Game, cid: u32, distance: i32) -> Vec<Pos> {
        let pos = g.cities[&cid].pos;
        let mut out: Vec<Pos> = g
            .wring(pos, distance)
            .into_iter()
            .filter(|p| {
                g.map
                    .get(*p)
                    .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// Play every unit of `pid` through the doctrine, the way the ladder
    /// loops a unit while it acts.
    fn play(ai: &mut AdvancedAi, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        ai.rebuild_force_groups(g, pid, plan);
        let mut ids = g.player_unit_ids(pid);
        ids.sort_unstable();
        for uid in ids {
            for _ in 0..8 {
                if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                    break;
                }
                if ai.force_groups_dirty {
                    ai.rebuild_force_groups(g, pid, plan);
                    ai.force_groups_dirty = false;
                }
                match ai.siege_doctrine_step(g, pid, uid, plan) {
                    Some(true) => {}
                    _ => break,
                }
            }
        }
    }

    /// One unit through the doctrine alone, the rest of the force standing.
    pub(super) fn step_unit(
        ai: &mut AdvancedAi,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) {
        ai.rebuild_force_groups(g, pid, plan);
        for _ in 0..8 {
            if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                break;
            }
            if ai.siege_doctrine_step(g, pid, uid, plan) != Some(true) {
                break;
            }
        }
    }

    /// Both seats end their turn: the board heals, moves are restored.
    fn end_round(g: &mut Game) {
        assert!(g.apply(0, &Action::EndTurn).is_ok());
        assert!(g.apply(1, &Action::EndTurn).is_ok());
        assert_eq!(g.current, 0);
    }

    #[test]
    fn the_genes_ship_off_and_are_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.siege_train && !ai.anvil, "opt-ins ship off");
        for (tag, field) in [("siege-train", "siege_train"), ("anvil", "anvil")] {
            assert!(super::super::GENES
                .iter()
                .any(|gene| gene.opt_in() && gene.tag == tag && gene.field == field));
        }
        let mut on = AdvancedAi::new();
        on.enable_siege_train();
        on.enable_anvil();
        assert!(on.siege_train && on.anvil);
        on.disable_siege_train();
        on.disable_anvil();
        assert!(!on.siege_train && !on.anvil);
        super::super::test_support::opt_in_off_in_both_controllers("siege-train", |ai| {
            ai.siege_train
        });
        super::super::test_support::opt_in_off_in_both_controllers("anvil", |ai| ai.anvil);
    }

    /// Off, the doctrine reads nothing: no siege record, no reservation,
    /// nothing moved.
    #[test]
    fn off_the_doctrine_orders_nothing() {
        let (mut g, cid) = walled_city();
        let far = at_distance(&g, cid, 3);
        let units: Vec<u32> = far
            .iter()
            .take(3)
            .map(|pos| g.spawn_unit("warrior", 0, *pos))
            .collect();
        let before: Vec<Pos> = units.iter().map(|uid| g.units[uid].pos).collect();
        let mut ai = AdvancedAi::new();
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        let after: Vec<Pos> = units.iter().map(|uid| g.units[uid].pos).collect();
        assert_eq!(before, after);
        assert!(ai.sieges.is_empty() && ai.reserved_units.is_empty());
    }

    /// Three melee units against an unguarded walled city take alternating
    /// ring tiles — no two adjacent — and the city stops healing: the
    /// engine's own `city_under_siege` reads true at its next turn.
    #[test]
    fn three_melee_units_seal_the_ring_on_alternating_tiles() {
        let (mut g, cid) = walled_city();
        let start = at_distance(&g, cid, 3);
        let warriors: Vec<u32> = start
            .iter()
            .step_by(start.len() / 3)
            .take(3)
            .map(|pos| g.spawn_unit("warrior", 0, *pos))
            .collect();
        assert_eq!(warriors.len(), 3);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        let plan = plan_against(&g, cid);
        for _ in 0..4 {
            play(&mut ai, &mut g, 0, &plan);
            let (sealed, ring) = ring_state(&g, cid);
            if ring > 0 && sealed == ring {
                break;
            }
            end_round(&mut g);
        }
        let (sealed, ring) = ring_state(&g, cid);
        let ring_tiles = ring_of(&g, cid);
        let standing: Vec<Pos> = warriors.iter().map(|uid| g.units[uid].pos).collect();
        let covered: Vec<(Pos, bool)> = ring_tiles
            .iter()
            .map(|pos| {
                (
                    *pos,
                    g.in_enemy_zoc(1, *pos) || !g.unit_ids_at(*pos).is_empty(),
                )
            })
            .collect();
        assert_eq!(
            sealed, ring,
            "the ring is sealed: {sealed}/{ring}; warriors at {standing:?}, ring {covered:?}, \
             stage {:?}",
            ai.sieges[&cid].stage
        );
        // Whoever sealed it stands three apart: no two ring units adjacent.
        let on_ring: Vec<Pos> = standing
            .iter()
            .copied()
            .filter(|pos| ring_tiles.contains(pos))
            .collect();
        assert!(
            on_ring.len() >= 2,
            "two units three apart seal a ring of six: {on_ring:?}"
        );
        for a in &on_ring {
            for b in &on_ring {
                assert!(
                    a == b || g.wdist(*a, *b) == 2,
                    "ring units stand on alternating tiles: {on_ring:?}"
                );
            }
        }
        // The rest of the train arrives on the ring and it stays sealed.
        for _ in 0..3 {
            if warriors
                .iter()
                .all(|uid| ring_tiles.contains(&g.units[uid].pos))
            {
                break;
            }
            end_round(&mut g);
            play(&mut ai, &mut g, 0, &plan);
        }
        for uid in &warriors {
            assert!(
                ring_tiles.contains(&g.units[uid].pos),
                "every warrior stands on the ring: {:?}",
                warriors
                    .iter()
                    .map(|uid| g.units[uid].pos)
                    .collect::<Vec<_>>()
            );
        }
        let (sealed, ring) = ring_state(&g, cid);
        assert_eq!(sealed, ring, "and the ring stays sealed");
        let stage = ai.sieges[&cid].stage;
        assert!(
            matches!(stage, SiegeStage::Reduce | SiegeStage::Take),
            "a sealed ring is a reduced city: {stage:?}"
        );
        assert!(ai.census.siege_rings_sealed >= 1);
        // The engine agrees: a besieged city does not heal at its owner's
        // end of turn.
        g.cities.get_mut(&cid).unwrap().hp = 150;
        end_round(&mut g);
        assert_eq!(g.cities[&cid].hp, 150, "no twenty-point heal under siege");
        // And the same board with one warrior taken off the ring heals.
        g.remove_unit(warriors[0]);
        end_round(&mut g);
        assert_eq!(g.cities[&cid].hp, 170, "an open ring heals the city");
    }

    /// The taker is designated, reserved and held while the walls stand: it
    /// neither attacks the city nor leaves the ring, and the city is
    /// untouched by it.
    #[test]
    fn the_taker_is_reserved_and_does_not_attack_while_the_walls_stand() {
        let (mut g, cid) = walled_city();
        let ring = ring_of(&g, cid);
        let swordsman = g.spawn_unit("swordsman", 0, ring[0]);
        let warrior = g.spawn_unit("warrior", 0, ring[3]);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        let siege = &ai.sieges[&cid];
        assert_eq!(
            siege.taker,
            Some(swordsman),
            "the stronger melee unit adjacent is the taker"
        );
        assert!(ai.unit_is_reserved(swordsman));
        assert!(!ai.unit_is_reserved(warrior));
        assert_eq!(g.units[&swordsman].pos, ring[0], "it holds its tile");
        assert_eq!(
            (g.cities[&cid].hp, g.cities[&cid].wall_hp),
            (200, 100),
            "no melee blow on a standing wall"
        );
        assert_eq!(g.units[&swordsman].hp, 100, "and it took no return blow");
        assert!(g.units[&swordsman].fortified);
    }

    /// With the wall down and the city within its blow, the taker attacks
    /// and the attack is the capture.
    #[test]
    fn the_taker_attacks_when_the_city_is_within_its_blow() {
        let (mut g, cid) = walled_city();
        let ring = ring_of(&g, cid);
        let swordsman = g.spawn_unit("swordsman", 0, ring[0]);
        g.spawn_unit("warrior", 0, ring[3]);
        {
            let city = g.cities.get_mut(&cid).unwrap();
            city.wall_hp = 0;
            city.hp = 30;
        }
        let blow = taker_blow(&g, 0, swordsman, cid);
        assert!(
            blow >= 30.0,
            "the fixture puts the city within the blow: {blow}"
        );
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        assert_eq!(g.cities[&cid].owner, 0, "the city changed hands");
        assert_eq!(ai.census.siege_captures, 1);
        assert_eq!(ai.sieges[&cid].stage, SiegeStage::Hold);
        assert!(!ai.unit_is_reserved(swordsman), "the taker is released");
        // And the same city at 190 behind no wall is not within the blow, so
        // the taker holds.
        let (mut g, cid) = walled_city();
        let swordsman = g.spawn_unit("swordsman", 0, ring[0]);
        g.spawn_unit("warrior", 0, ring[3]);
        g.cities.get_mut(&cid).unwrap().wall_hp = 0;
        g.cities.get_mut(&cid).unwrap().hp = 190;
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        assert_eq!(g.cities[&cid].owner, 1);
        assert_eq!(ai.sieges[&cid].taker, Some(swordsman));
        assert_eq!(g.cities[&cid].hp, 190, "the taker waits for the guns");
    }

    /// A catapult in range of both the city and a healthy defender shoots
    /// the city — the wall goes down first — and once the wall is gone it
    /// shoots the city's hit points; a reliever it can kill takes priority.
    #[test]
    fn siege_units_target_the_walls_before_the_garrison() {
        let (mut g, cid) = walled_city();
        let ring = ring_of(&g, cid);
        let city_pos = g.cities[&cid].pos;
        // Two swordsmen on the ring, a catapult on the first tile at range
        // two with a legal shot, and an enemy warrior in the catapult's
        // range near the city.
        g.spawn_unit("swordsman", 0, ring[0]);
        g.spawn_unit("swordsman", 0, ring[3]);
        let frame = g.player_vision_frame(0);
        let viewers = g.visibility_viewers(0);
        let mut catapult = None;
        for pos in at_distance(&g, cid, 2) {
            let uid = g.spawn_unit("catapult", 0, pos);
            if g.ranged_order_is_legal(0, uid, city_pos, frame.as_ref(), &viewers) {
                catapult = Some(uid);
                break;
            }
            g.remove_unit(uid);
        }
        let catapult = catapult.expect("a firing tile with a clear shot");
        let cat_pos = g.units[&catapult].pos;
        let enemy_tile = at_distance(&g, cid, 2)
            .into_iter()
            .find(|pos| {
                *pos != cat_pos
                    && g.wdist(*pos, cat_pos) <= 2
                    && g.unit_ids_at(*pos).is_empty()
                    && g.wdist(*pos, city_pos) <= RELIEVER_RADIUS
                    // Beside neither swordsman, so the ring does not finish
                    // it before the gun's turn.
                    && g.wdist(*pos, ring[0]) > 1
                    && g.wdist(*pos, ring[3]) > 1
            })
            .expect("a tile for the reliever in the catapult's range");
        let reliever = g.spawn_unit("warrior", 1, enemy_tile);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        let plan = plan_against(&g, cid);
        step_unit(&mut ai, &mut g, 0, catapult, &plan);
        assert!(g.cities[&cid].wall_hp < 100, "the catapult shot the wall");
        assert_eq!(
            g.units[&reliever].hp, 100,
            "the healthy reliever was not the target"
        );
        assert_eq!(
            g.units[&catapult].pos, cat_pos,
            "the gun did not move to fire"
        );
        // The wall down: the next shot lands on the city's hit points.
        g.cities.get_mut(&cid).unwrap().wall_hp = 0;
        end_round(&mut g);
        let hp_before = g.cities[&cid].hp;
        step_unit(&mut ai, &mut g, 0, catapult, &plan);
        assert!(
            g.cities[&cid].hp < hp_before,
            "the garrison takes the blow now"
        );
        assert_eq!(g.units[&reliever].hp, 100);
        // A reliever it can kill outranks the city.
        end_round(&mut g);
        g.units.get_mut(&reliever).unwrap().hp = 10;
        let hp_before = g.cities[&cid].hp;
        step_unit(&mut ai, &mut g, 0, catapult, &plan);
        assert!(
            !g.units.contains_key(&reliever),
            "the wounded reliever is finished"
        );
        assert_eq!(
            g.cities[&cid].hp, hp_before,
            "the catapult's shot went to the reliever, not the city"
        );
    }

    /// The anvil: a ranged unit ends on the City Center and at least one
    /// unit stands adjacent while a hostile is within six.
    #[test]
    fn the_anvil_keeps_a_ranged_unit_on_the_city_and_a_unit_adjacent() {
        let (mut g, cid) = walled_city();
        g.current = 1;
        let city_pos = g.cities[&cid].pos;
        let near = at_distance(&g, cid, 2);
        let archer = g.spawn_unit("archer", 1, near[0]);
        let warrior = g.spawn_unit("warrior", 1, near[1]);
        let spearman = g.spawn_unit("spearman", 1, near[2]);
        let hostile_tile = at_distance(&g, cid, 5)[0];
        let hostile = g.spawn_unit("swordsman", 0, hostile_tile);
        let mut ai = AdvancedAi::new();
        ai.enable_anvil();
        let plan = plan_holding(&g, cid);
        for _ in 0..3 {
            play(&mut ai, &mut g, 1, &plan);
            if g.units[&archer].pos == city_pos {
                break;
            }
            assert!(g.apply(1, &Action::EndTurn).is_ok());
            assert!(g.apply(0, &Action::EndTurn).is_ok());
        }
        assert_eq!(
            g.units[&archer].pos, city_pos,
            "the shooter garrisons the city"
        );
        let adjacent = [warrior, spearman]
            .iter()
            .filter(|uid| g.wdist(g.units[uid].pos, city_pos) == 1)
            .count();
        assert!(adjacent >= 1, "at least one unit on the ring");
        assert!(g.units.contains_key(&hostile));
        assert!(ai.census.anvil_turns > 0);
        // Off, the same board is left to the ladder: nothing is ordered.
        let (mut g, cid) = walled_city();
        g.current = 1;
        let archer = g.spawn_unit("archer", 1, near[0]);
        g.spawn_unit("swordsman", 0, hostile_tile);
        let mut off = AdvancedAi::new();
        let plan = plan_holding(&g, cid);
        play(&mut off, &mut g, 1, &plan);
        assert_eq!(g.units[&archer].pos, near[0]);
        assert!(off.anvil_orders.is_empty());
    }

    /// A wounded anvil unit on the front swaps into the city; the fresh unit
    /// that stood there takes its tile.
    #[test]
    fn a_wounded_anvil_unit_swaps_into_the_city() {
        let (mut g, cid) = walled_city();
        g.current = 1;
        g.tactics.heal = true;
        let city_pos = g.cities[&cid].pos;
        let ring = ring_of(&g, cid);
        let archer = g.spawn_unit("archer", 1, city_pos);
        let hurt = g.spawn_unit("warrior", 1, ring[0]);
        g.units.get_mut(&hurt).unwrap().hp = 30;
        let hostile_tile = at_distance(&g, cid, 4)
            .into_iter()
            .min_by_key(|pos| (g.wdist(*pos, ring[0]), *pos))
            .expect("a tile facing the front");
        g.spawn_unit("swordsman", 0, hostile_tile);
        let mut ai = AdvancedAi::new();
        ai.enable_anvil();
        let plan = plan_holding(&g, cid);
        play(&mut ai, &mut g, 1, &plan);
        assert_eq!(
            g.units[&hurt].pos, city_pos,
            "the wounded unit is in the city"
        );
        assert_eq!(
            g.units[&archer].pos, ring[0],
            "the fresh unit holds its tile"
        );
        assert_eq!(ai.census.anvil_rotations, 1);
    }
}

#[cfg(test)]
mod rebuild_tests;

#[cfg(test)]
mod taker_pressure_tests;

#[cfg(test)]
mod support_tests;

#[cfg(test)]
mod linked_support_tests;

#[cfg(test)]
#[path = "siege_train/tests.rs"]
mod obstacle_routing_tests;
