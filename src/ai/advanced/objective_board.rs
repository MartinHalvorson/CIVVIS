//! The Objective Board: what the army is *for* this turn, ranked, and the
//! task forces raised against it — in place of proximity force groups and
//! the posture ladder. Opt-in gene `objective-board`, off, byte-identical
//! when off (`rebuild_force_groups` never reaches this module).
//!
//! **What the shipped layer does.** `rebuild_force_groups` clusters every
//! field unit by proximity (a clique of `command_radius` six), anchors each
//! cluster on its medoid, aims it with `domain_objective` — one objective
//! for the whole empire: the threatened city, else the target city, else the
//! nearest enemy — and hands it a posture from a ladder of ratios. The
//! groups are rebuilt every turn and after every strike; a group's id is its
//! lowest unit, so no group exists for longer than a turn, nothing is ever
//! *asked* of the army, and two cities under pressure at once produce one
//! `threatened_city` and one relief — the argmax flips between them and the
//! second city gets nothing. `docs/LIVE_TACTICS.md` §1 records the shape: a
//! relief column that holds at its centroid, forty cities lost on the King
//! rung, a siege fed to a garrison a unit at a time.
//!
//! **What this does instead.** Once a turn the controller writes a board:
//!
//! - **Rows** ([`Objective`]) — `Defend` a city under pressure (danger at or
//!   over [`BASTION_PRESSURE`]), `Relieve` it from beyond six, `Siege` the
//!   plan's target city and the campaign's cities in order, `Destroy` a
//!   hostile force in the field, `ClearCamp` a barbarian camp within nine of
//!   a city before turn 100, `Escort` a civilian outside our borders,
//!   `Deter` the strongest bordering major while our power is under
//!   [`DETER_POWER_RATIO`] of theirs, and `Recon` an unexplored sector no
//!   scout holds. Every row carries a **value in hammers** — a unit is its
//!   production cost at its hit points, a city the replacement production of
//!   its districts and buildings (the lane's own district at
//!   [`LANE_PREMIUM`]) plus [`POP_VALUE`] a citizen, a settler its cost plus
//!   [`SETTLER_PREMIUM`], a camp [`CAMP_VALUE`] plus its guard — a
//!   **requirement** ([`ForceNeed`]: strength, melee, ranged, siege, bodies)
//!   and, where the board can name one, a **deadline** in turns: for a
//!   Defend, the turns until the city falls at the damage it has been taking
//!   (never under two); for an Escort, the turns until a known raider is in
//!   reach of the civilian.
//! - **Rank.** Rows sort by value over deadline (a row with no deadline is
//!   read at [`NO_DEADLINE_HORIZON`] turns), with two hard rules: a Defend
//!   whose deadline is inside the relief time of the nearest force outranks
//!   every offensive row, and no row ranks above one it depends on (Relieve
//!   after its Defend; the campaign's second city after its first).
//! - **Task forces** ([`TaskForce`]) — kept on the controller across turns,
//!   with an id that does not change when a member dies. Allocation walks
//!   the rows in rank order and takes the best contribution per travel turn
//!   until the row is met: a unit's contribution is its strength toward the
//!   unmet need (and a body toward an unmet melee, ranged, siege or body
//!   count) times an arrival factor — one inside the deadline,
//!   [`LATE_FACTOR`] to the power of the turns late after it. A served row
//!   is never stripped below its need by a lower one; a unit already in a
//!   force stays unless the gain is at least [`HYSTERESIS_GAIN`] or its row
//!   is done; an urgent Defend may pull anyone. Whatever is left forms the
//!   **Reserve** at the Deter row's tile, else the frontier city nearest the
//!   strongest met rival, else the capital. Sea units form their own forces
//!   for a coastal Siege, an embarked Escort and a naval Destroy; air units
//!   stay out, as the shipped layer keeps them out.
//! - **Integration.** With the gene on, `rebuild_force_groups` builds
//!   `force_groups` *from* the task forces — one `ForceGroup` per force,
//!   `objective` the row's tile, `anchor` where a standing force stands (the
//!   city for a Defend, the staging side for a mustering Siege, the civilian
//!   for an Escort, the Deter tile for the Reserve), `posture` from the
//!   row's doctrine: Defend/Relieve hold the city and engage on contact;
//!   Siege follows the siege train's stage when that gene is on (Muster
//!   while staging, Advance to invest, Engage to reduce and take), else
//!   musters, advances and engages on contact; Destroy engages when the
//!   exchange is at least [`DESTROY_ENGAGE_EXCHANGE`] and holds defensive
//!   ground otherwise; ClearCamp and Recon advance; Escort and Deter hold. So
//!   `battle_planner.rs`, `siege_train.rs` and the per-unit ladder keep
//!   working unchanged on `force_groups`. No `victory_planning` gate: the
//!   board runs for every major seat.
//! - **The record.** One "Military/Strategy" line a turn lists the top rows
//!   and the census counts rows, forces, reassignments and rows left short;
//!   [`AdvancedAi::requisitions`] publishes the shortfall per kind for a
//!   production consumer (the next change — nothing reads it yet).
//!
//! Priced on the arena first (`docs/DOCTRINE_ARENA.md`, "The gate for a
//! tactical gene"); the whole-game screen is the no-harm check. See
//! `docs/LIVE_TACTICS.md` §24.

use std::collections::{BTreeMap, BTreeSet};

use super::city_campaign::NeighbourAppraisal;
use super::siege_train::SiegeStage;
use super::{
    AdvancedAi, BasicAi, ForceDomain, ForceGroup, ForcePosture, GrandStrategy, StrategicPlan,
    UnitDoctrine, BASTION_PRESSURE, BELIEF_PRESSURE_HORIZON, BORDER_PARITY_CONTACT_RADIUS,
    THREAT_RELIEF_RADIUS,
};
use crate::game::{effective_strength, Game};
use crate::think;
use crate::Pos;

/// A Defend row's requirement is the hostile strength within
/// [`THREAT_RELIEF_RADIUS`] times this, less the city's own strength.
pub const DEFEND_MARGIN: f64 = 1.2;
/// A Siege row's requirement is the campaign bill times this.
pub const SIEGE_MARGIN: f64 = 1.25;
/// A camp's requirement is its guard's strength times this.
pub const CAMP_MARGIN: f64 = 1.5;
/// A Destroy row's requirement is the hostile force's strength times this.
pub const DESTROY_MARGIN: f64 = 1.5;
/// A camp is a row while it stands within this of one of our cities…
pub const CAMP_RADIUS: i32 = crate::ai::HOME_CAMP_RADIUS;
/// …before this turn (game-speed scaled).
pub const CAMP_TURN_LIMIT: u32 = 100;
/// The lane's own district counts this much more in a city's value.
pub const LANE_PREMIUM: f64 = 1.5;
/// Hammers a citizen is worth.
pub const POP_VALUE: f64 = 20.0;
/// Hammers over its cost a settler is worth: the city it would have been.
pub const SETTLER_PREMIUM: f64 = 200.0;
/// Hammers a camp is worth over its guard: the raids it stops producing.
pub const CAMP_VALUE: f64 = 120.0;
/// A Deter row is worth this share of the contact city.
pub const DETER_SHARE: f64 = 0.3;
/// Deter the strongest bordering major while our power is under this share
/// of theirs — `border_parity`'s own ratio.
pub const DETER_POWER_RATIO: f64 = 0.8;
/// The arrival factor per turn late.
pub const LATE_FACTOR: f64 = 0.7;
/// A unit leaves its force for another row only for this much more
/// contribution per travel turn.
pub const HYSTERESIS_GAIN: f64 = 1.25;
/// A row with no deadline is ranked as if it fell due in this many turns.
pub const NO_DEADLINE_HORIZON: f64 = 10.0;
/// A Destroy force engages at this exchange and holds ground below it.
pub const DESTROY_ENGAGE_EXCHANGE: f64 = 1.5;
/// A Defend deadline is never under this.
pub const DEFEND_DEADLINE_FLOOR: u32 = 2;
/// Hostile units this close to each other are one force.
pub const FORCE_LINK: i32 = 3;
/// A Destroy force that has lost its exact row keeps its identity for a row
/// within this of where it was aimed.
const DESTROY_REKEY_RADIUS: i32 = 4;
/// Recon sectors are this many offset columns and rows square…
pub const SECTOR: i32 = 8;
/// …count as unexplored under this share of explored tiles…
pub const SECTOR_EXPLORED_SHARE: f64 = 0.5;
/// …and are rows only within this of one of our cities.
pub const RECON_REACH: i32 = 24;
/// A scout within this of a sector's centre holds it.
const RECON_HOLD_RADIUS: i32 = 6;
/// At most this many Recon rows a turn.
const RECON_ROWS_MAX: usize = 6;
/// A body with no army to average has a Warrior's strength.
const DEFAULT_BODY_STRENGTH: f64 = 20.0;
/// Rows the journal line names.
const JOURNAL_ROWS: usize = 5;

/// What a row asks the army to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectiveKind {
    Defend,
    Relieve,
    Siege,
    Destroy,
    ClearCamp,
    Escort,
    Deter,
    Recon,
}

impl ObjectiveKind {
    pub const ALL: [ObjectiveKind; 8] = [
        ObjectiveKind::Defend,
        ObjectiveKind::Relieve,
        ObjectiveKind::Siege,
        ObjectiveKind::Destroy,
        ObjectiveKind::ClearCamp,
        ObjectiveKind::Escort,
        ObjectiveKind::Deter,
        ObjectiveKind::Recon,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ObjectiveKind::Defend => "Defend",
            ObjectiveKind::Relieve => "Relieve",
            ObjectiveKind::Siege => "Siege",
            ObjectiveKind::Destroy => "Destroy",
            ObjectiveKind::ClearCamp => "ClearCamp",
            ObjectiveKind::Escort => "Escort",
            ObjectiveKind::Deter => "Deter",
            ObjectiveKind::Recon => "Recon",
        }
    }

    /// Rows that take the fight to the enemy — the ones an urgent Defend
    /// outranks.
    fn offensive(self) -> bool {
        matches!(
            self,
            ObjectiveKind::Siege | ObjectiveKind::Destroy | ObjectiveKind::ClearCamp
        )
    }
}

/// What a row is about — stable across turns, so a task force keeps its
/// row while the row stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectiveKey {
    /// A city of ours, by id.
    Defend(u32),
    Relieve(u32),
    /// An enemy city, by id.
    Siege(u32),
    /// An arena flag, by tile.
    Flag(Pos),
    /// A hostile force, by its lowest unit id.
    Destroy(u32),
    /// A camp, by tile.
    Camp(Pos),
    /// A civilian, by id.
    Escort(u32),
    /// The contact city, by id.
    Deter(u32),
    /// A sector, by index.
    Recon(u32),
    /// The leftovers.
    Reserve,
}

/// What a row asks of the force that serves it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForceNeed {
    /// Defending strength at hit points, summed over the force.
    pub strength: f64,
    /// Melee-capable units without a ranged attack.
    pub melee: usize,
    /// Ranged units that are not siege.
    pub ranged: usize,
    /// Siege units.
    pub siege: usize,
    /// Any field unit.
    pub bodies: usize,
}

impl ForceNeed {
    fn is_zero(&self) -> bool {
        self.strength <= 1e-9
            && self.melee == 0
            && self.ranged == 0
            && self.siege == 0
            && self.bodies == 0
    }

    /// What `have` leaves unmet.
    fn unmet(&self, have: &ForceNeed) -> ForceNeed {
        ForceNeed {
            strength: (self.strength - have.strength).max(0.0),
            melee: self.melee.saturating_sub(have.melee),
            ranged: self.ranged.saturating_sub(have.ranged),
            siege: self.siege.saturating_sub(have.siege),
            bodies: self.bodies.saturating_sub(have.bodies),
        }
    }

    fn add(&mut self, unit: &UnitFacts) {
        self.strength += unit.strength;
        self.melee += usize::from(unit.melee);
        self.ranged += usize::from(unit.ranged);
        self.siege += usize::from(unit.siege);
        self.bodies += 1;
    }

    fn remove(&mut self, unit: &UnitFacts) {
        self.strength = (self.strength - unit.strength).max(0.0);
        self.melee = self.melee.saturating_sub(usize::from(unit.melee));
        self.ranged = self.ranged.saturating_sub(usize::from(unit.ranged));
        self.siege = self.siege.saturating_sub(usize::from(unit.siege));
        self.bodies = self.bodies.saturating_sub(1);
    }
}

/// Where a row stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// Nothing has been decided about it beyond the board.
    Open,
    /// The siege train's stage, when that gene runs the siege.
    Staging,
    Investing,
    Reducing,
    Taking,
    Held,
}

impl RowState {
    pub fn as_str(self) -> &'static str {
        match self {
            RowState::Open => "open",
            RowState::Staging => "staging",
            RowState::Investing => "investing",
            RowState::Reducing => "reducing",
            RowState::Taking => "taking",
            RowState::Held => "held",
        }
    }
}

/// One row of the board.
#[derive(Clone, Debug, PartialEq)]
pub struct Objective {
    pub kind: ObjectiveKind,
    pub key: ObjectiveKey,
    pub at: Pos,
    /// Hammers.
    pub value: f64,
    pub requirement: ForceNeed,
    /// Turns, when the board can name one.
    pub deadline: Option<u32>,
    pub state: RowState,
    /// A row this one never ranks above.
    pub depends_on: Option<ObjectiveKey>,
    /// Land units may serve it.
    pub land: bool,
    /// Sea units may serve it, in a force of their own.
    pub sea: bool,
    /// What it is about, for the journal.
    pub label: String,
    /// A Defend whose deadline is inside the relief time of the nearest
    /// force: outranks every offensive row and may pull anyone.
    pub urgent: bool,
}

impl Objective {
    fn score(&self) -> f64 {
        let horizon = self
            .deadline
            .map_or(NO_DEADLINE_HORIZON, |deadline| f64::from(deadline.max(1)));
        self.value / horizon
    }
}

/// A force raised against a row, kept across turns.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskForce {
    /// Stable across turns and the death of any member.
    pub id: u32,
    pub objective_key: ObjectiveKey,
    pub domain: ForceDomain,
    pub units: Vec<u32>,
    /// Where a new member joins and where a standing force stands.
    pub rally: Pos,
    /// The posture the force played last, for the record.
    pub doctrine_state: ForcePosture,
    /// Where the row was when the force was last aimed.
    pub aimed_at: Pos,
    /// The turn the force was raised.
    pub formed: u32,
}

/// The shortfall of one row: what production would have to supply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requisition {
    pub kind: ObjectiveKind,
    /// Units short.
    pub count: usize,
    /// The turn they are needed by, when the row has a deadline.
    pub by_turn: Option<u32>,
    /// Our city nearest the row.
    pub city: Option<u32>,
}

/// The board and its forces, kept on the controller.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveBoard {
    pub rows: Vec<Objective>,
    pub forces: Vec<TaskForce>,
    next_force_id: u32,
    /// `(turn, seat)` of the last assessment; once a turn.
    assessed: Option<(u32, usize)>,
    /// Each city's hit points plus wall at the last assessment, for the
    /// damage rate a Defend deadline is read from.
    city_health: BTreeMap<u32, (u32, i32)>,
    /// Hit points a city has been losing a turn.
    damage_rate: BTreeMap<u32, f64>,
    requisitions: Vec<Requisition>,
}

/// The facts about one of our field units the allocation reads.
#[derive(Clone, Copy, Debug)]
struct UnitFacts {
    pos: Pos,
    domain: ForceDomain,
    strength: f64,
    moves: f64,
    melee: bool,
    ranged: bool,
    siege: bool,
    recon: bool,
}

impl UnitFacts {
    fn of(g: &Game, uid: u32) -> Self {
        let unit = &g.units[&uid];
        let spec = &g.rules.units[unit.kind];
        let military = spec.class == "military";
        let ranged_attack = military && spec.has_ranged_attack();
        UnitFacts {
            pos: unit.pos,
            domain: AdvancedAi::force_domain(g, uid),
            strength: if military {
                effective_strength(g.unit_strength(unit, true), unit.hp)
            } else {
                0.0
            },
            moves: g.unit_max_moves(uid).max(1.0),
            melee: military && spec.is_melee_capable() && !ranged_attack,
            ranged: ranged_attack && !spec.siege,
            siege: ranged_attack && spec.siege,
            recon: BasicAi::unit_doctrine(g, uid) == UnitDoctrine::Recon,
        }
    }

    /// Turns to stand within `stop` of `to`, by hex distance.
    fn travel_turns(&self, g: &Game, to: Pos, stop: i32) -> u32 {
        let distance = (g.wdist(self.pos, to) - stop).max(0);
        if distance == 0 {
            0
        } else {
            (f64::from(distance) / self.moves).ceil() as u32
        }
    }

    /// Strength toward an unmet need, a body toward an unmet count.
    fn contribution(&self, unmet: &ForceNeed) -> f64 {
        let body = self.strength.max(10.0);
        let mut total = self.strength.min(unmet.strength);
        if unmet.melee > 0 && self.melee {
            total += body;
        }
        if unmet.ranged > 0 && self.ranged {
            total += body;
        }
        if unmet.siege > 0 && self.siege {
            total += body;
        }
        if unmet.bodies > 0 {
            total += body;
        }
        total
    }
}

/// A unit's production cost at its hit points, in hammers.
fn unit_value(g: &Game, uid: u32) -> f64 {
    let unit = &g.units[&uid];
    g.rules.units[unit.kind].cost * f64::from(unit.hp.clamp(0, 100)) / 100.0
}

/// The district the lane is built on, whose replacement counts at
/// [`LANE_PREMIUM`].
fn lane_district(strategy: GrandStrategy) -> Option<&'static str> {
    match strategy {
        GrandStrategy::Science => Some("campus"),
        GrandStrategy::Culture => Some("theater_square"),
        GrandStrategy::Religion => Some("holy_site"),
        GrandStrategy::Diplomacy => Some("government_plaza"),
        GrandStrategy::Conquest => Some("encampment"),
        GrandStrategy::Expansion | GrandStrategy::Recovery => None,
    }
}

/// A city's replacement production: its districts and buildings, the
/// lane's district at a premium, plus [`POP_VALUE`] a citizen.
fn city_value(g: &Game, cid: u32, lane: Option<&str>) -> f64 {
    let Some(city) = g.cities.get(&cid) else {
        return 0.0;
    };
    let districts: f64 = city
        .districts
        .keys()
        .map(|name| {
            let cost = g
                .rules
                .districts
                .get_interned(*name)
                .map_or(0.0, |spec| spec.cost);
            if lane.is_some_and(|lane| name.as_str() == lane) {
                cost * LANE_PREMIUM
            } else {
                cost
            }
        })
        .sum();
    let buildings: f64 = city
        .buildings
        .iter()
        .map(|name| {
            g.rules
                .buildings
                .get_interned(*name)
                .map_or(0.0, |spec| spec.cost)
        })
        .sum();
    districts + buildings + f64::from(city.pop.max(0)) * POP_VALUE
}

/// Whether `pos` lies inside `pid`'s borders.
fn inside_borders(g: &Game, pid: usize, pos: Pos) -> bool {
    g.map
        .get(pos)
        .and_then(|tile| tile.owner_city)
        .and_then(|cid| g.cities.get(&cid))
        .is_some_and(|city| city.owner == pid)
}

/// The medoid of a set of tiles.
fn medoid(g: &Game, tiles: &[Pos]) -> Option<Pos> {
    tiles.iter().copied().min_by_key(|pos| {
        (
            tiles.iter().map(|other| g.wdist(*pos, *other)).sum::<i32>(),
            *pos,
        )
    })
}

impl AdvancedAi {
    /// The board's shortfall, per row: what production would have to supply
    /// for every row the allocation left short. Empty with the gene off and
    /// before the first assessment. Published for a production consumer;
    /// nothing reads it yet.
    pub fn requisitions(&self) -> Vec<Requisition> {
        self.objective_board_state.requisitions.clone()
    }

    /// The board as last assessed.
    pub fn objective_board(&self) -> &ObjectiveBoard {
        &self.objective_board_state
    }

    /// `rebuild_force_groups` with the gene on: assess the board once a
    /// turn, then project the task forces onto `force_groups`.
    pub(super) fn board_rebuild_force_groups(
        &mut self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if self.objective_board_state.assessed != Some((g.turn, pid)) {
            self.assess_board(g, pid, plan);
            self.objective_board_state.assessed = Some((g.turn, pid));
        }
        self.project_forces(g, pid, plan);
    }

    /// Our field units: military and support, land and sea, not air, not a
    /// scout with somewhere to explore, not a guard bound to a settler.
    fn board_pool(&self, g: &Game, pid: usize) -> Vec<u32> {
        let mut pool: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                matches!(spec.class.as_str(), "military" | "support")
                    && spec.domain.as_deref() != Some("air")
                    && !(BasicAi::unit_doctrine(g, *uid) == UnitDoctrine::Recon
                        && self.base.has_exploration_target(g, pid, *uid))
                    && !self.guard_is_bound_to_any_settler(*uid)
                    && unit.linked_to.is_none()
            })
            .collect();
        pool.sort_unstable();
        pool
    }

    /// Whether a unit is in the turn-start frame — the shipped layer's own
    /// test, which is every unit when the controller does not observe.
    fn observed(
        &self,
        g: &Game,
        pid: usize,
        visible: &crate::world::TileBits,
        unit: &crate::game::Unit,
    ) -> bool {
        !self.battlefront_observation
            || (g.sees(visible, unit.pos) && self.battlefront_unit_visible(g, pid, unit.id))
    }

    /// Hostile military strength within `radius` of `at`, visible in the
    /// turn-start frame, with the remembered term the belief arm adds.
    fn hostile_strength_near(
        &self,
        g: &Game,
        pid: usize,
        at: Pos,
        radius: i32,
        visible: &crate::world::TileBits,
    ) -> f64 {
        let seen: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, at) <= radius)
            .filter(|unit| self.observed(g, pid, visible, unit))
            .map(|unit| effective_strength(g.unit_strength(unit, false), unit.hp))
            .sum();
        let remembered = if self.belief_pressure {
            self.belief.remembered_hidden_military_threat(
                g,
                pid,
                at,
                radius,
                BELIEF_PRESSURE_HORIZON,
            )
        } else {
            0.0
        };
        seen + remembered
    }

    /// Visible hostile military units clustered into forces: single linkage
    /// at [`FORCE_LINK`]. Barbarians count within [`THREAT_RELIEF_RADIUS`]
    /// of one of our cities; on an arena everything hostile counts.
    fn hostile_forces(
        &self,
        g: &Game,
        pid: usize,
        visible: &crate::world::TileBits,
        our_cities: &[(u32, Pos)],
    ) -> Vec<Vec<u32>> {
        let arena = g.is_arena();
        let mut hostiles: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                spec.class == "military" && spec.domain.as_deref() != Some("air")
            })
            .filter(|unit| self.observed(g, pid, visible, unit))
            .filter(|unit| {
                if arena {
                    return true;
                }
                let barbarian = Some(unit.owner) == g.barb_pid;
                if barbarian && BasicAi::is_barbarian_scout(g, unit.id) {
                    return false;
                }
                let home = our_cities
                    .iter()
                    .any(|(_, pos)| g.wdist(*pos, unit.pos) <= THREAT_RELIEF_RADIUS);
                !barbarian || home
            })
            .map(|unit| unit.id)
            .collect();
        hostiles.sort_unstable();
        let mut forces: Vec<Vec<u32>> = Vec::new();
        let mut remaining: BTreeSet<u32> = hostiles.into_iter().collect();
        while let Some(seed) = remaining.iter().next().copied() {
            remaining.remove(&seed);
            let mut force = vec![seed];
            let mut frontier = vec![seed];
            while let Some(from) = frontier.pop() {
                let near: Vec<u32> = remaining
                    .iter()
                    .copied()
                    .filter(|other| g.wdist(g.units[&from].pos, g.units[other].pos) <= FORCE_LINK)
                    .collect();
                for other in near {
                    remaining.remove(&other);
                    force.push(other);
                    frontier.push(other);
                }
            }
            force.sort_unstable();
            forces.push(force);
        }
        forces
    }

    /// The strongest bordering major we are not at war with, while our power
    /// is under [`DETER_POWER_RATIO`] of theirs: `(rival, our contact city)`.
    fn deter_target(
        &self,
        g: &Game,
        pid: usize,
        our_cities: &[(u32, Pos)],
    ) -> Option<(usize, u32)> {
        if our_cities.is_empty() {
            return None;
        }
        let at_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_barbarian
                && !player.is_minor
                && g.is_at_war(pid, player.id)
        });
        if at_war {
            return None;
        }
        let ours = g.military_power(pid).max(1.0);
        let mut strongest: Option<(f64, usize, u32)> = None;
        for city in g.cities.values() {
            let owner = city.owner;
            if owner == pid {
                continue;
            }
            let player = &g.players[owner];
            if !player.alive
                || player.is_barbarian
                || player.is_minor
                || g.same_team(pid, owner)
                || !g.has_met(pid, owner)
            {
                continue;
            }
            let Some((contact, distance)) = our_cities
                .iter()
                .map(|(cid, pos)| (*cid, g.wdist(*pos, city.pos)))
                .min_by_key(|(cid, distance)| (*distance, *cid))
            else {
                continue;
            };
            if distance > BORDER_PARITY_CONTACT_RADIUS {
                continue;
            }
            let power = g.military_power(owner);
            if strongest.is_none_or(|(best, _, _)| power > best) {
                strongest = Some((power, owner, contact));
            }
        }
        let (theirs, rival, contact) = strongest?;
        (ours < DETER_POWER_RATIO * theirs).then_some((rival, contact))
    }

    /// The Siege requirement: the campaign's own bill for the city, times
    /// [`SIEGE_MARGIN`], a melee taker, and siege if the city has walls.
    pub(super) fn siege_requirement(&self, g: &Game, pid: usize, cid: u32) -> ForceNeed {
        let city = &g.cities[&cid];
        let owner = city.owner;
        let appraisal = self.appraise_neighbour(g, pid, owner).unwrap_or_else(|| {
            let mine = g.player_city_ids(pid);
            let distance = mine
                .iter()
                .map(|ours| g.wdist(g.cities[ours].pos, city.pos))
                .min()
                .unwrap_or(0);
            NeighbourAppraisal {
                rival: owner,
                power_ratio: g.military_power(pid) / g.military_power(owner).max(1.0),
                tech_lead: g.players[pid].techs.len() as i64
                    - g.players
                        .get(owner)
                        .map_or(0, |player| player.techs.len() as i64),
                distance,
            }
        });
        let army = self.campaign_field_army(g, pid);
        let average_body = if army.is_empty() {
            DEFAULT_BODY_STRENGTH
        } else {
            Self::campaign_strength_of(g, &army) / army.len() as f64
        };
        let bill = self.campaign_city_requirement(g, pid, cid, &appraisal, average_body);
        ForceNeed {
            strength: bill.strength * SIEGE_MARGIN,
            melee: 1,
            ranged: 0,
            siege: usize::from(city.wall_hp > 0),
            bodies: 0,
        }
    }

    /// Every row of the board this turn, unranked.
    fn board_rows(
        &mut self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        visible: &crate::world::TileBits,
        pool: &[u32],
    ) -> Vec<Objective> {
        let mut rows: Vec<Objective> = Vec::new();
        let lane = lane_district(plan.strategy);
        let our_cities: Vec<(u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| (cid, g.cities[&cid].pos))
            .collect();
        let arena = g.is_arena();
        let turn = g.turn;

        // Defend and Relieve: every city under pressure.
        let mut defended: BTreeSet<u32> = BTreeSet::new();
        for (cid, pos) in &our_cities {
            let city = &g.cities[cid];
            let health = city.hp.max(0) + city.wall_hp.max(0);
            let rate = match self.objective_board_state.city_health.get(cid) {
                Some((last_turn, last_health)) if *last_turn < turn => {
                    f64::from((*last_health - health).max(0)) / f64::from(turn - *last_turn)
                }
                _ => 0.0,
            };
            if rate > 0.0 {
                self.objective_board_state.damage_rate.insert(*cid, rate);
            } else if city.last_attacked == 0 || turn.saturating_sub(city.last_attacked) > 3 {
                self.objective_board_state.damage_rate.remove(cid);
            }
            self.objective_board_state
                .city_health
                .insert(*cid, (turn, health));
            let danger = self.city_pressure_with_belief(g, pid, *cid, visible);
            if danger < BASTION_PRESSURE {
                continue;
            }
            defended.insert(*cid);
            let hostile = self.hostile_strength_near(g, pid, *pos, THREAT_RELIEF_RADIUS, visible);
            let need = ForceNeed {
                strength: (hostile * DEFEND_MARGIN - g.city_strength(*cid)).max(0.0),
                melee: 1,
                ranged: 0,
                siege: 0,
                bodies: 0,
            };
            let deadline = match self.objective_board_state.damage_rate.get(cid) {
                Some(rate) if *rate > 0.0 => {
                    ((f64::from(health) / rate).ceil() as u32).max(DEFEND_DEADLINE_FLOOR)
                }
                _ => {
                    // Not yet hit: the turns the nearest hostile needs to
                    // reach the city, never under the floor.
                    let nearest = g
                        .units
                        .values()
                        .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
                        .filter(|unit| g.rules.units[unit.kind].class == "military")
                        .filter(|unit| self.observed(g, pid, visible, unit))
                        .map(|unit| g.wdist(unit.pos, *pos))
                        .filter(|distance| *distance <= THREAT_RELIEF_RADIUS)
                        .min();
                    nearest
                        .map(|distance| {
                            ((f64::from(distance) / 2.0).ceil() as u32).max(DEFEND_DEADLINE_FLOOR)
                        })
                        .unwrap_or(THREAT_RELIEF_RADIUS as u32)
                }
            };
            let value = city_value(g, *cid, lane).max(POP_VALUE);
            rows.push(Objective {
                kind: ObjectiveKind::Defend,
                key: ObjectiveKey::Defend(*cid),
                at: *pos,
                value,
                requirement: need,
                deadline: Some(deadline),
                state: RowState::Open,
                depends_on: None,
                land: true,
                sea: false,
                label: city.name.clone(),
                urgent: false,
            });
            // Relieve: what the units within reach cannot supply.
            let local: f64 = pool
                .iter()
                .filter(|uid| g.wdist(g.units[uid].pos, *pos) <= THREAT_RELIEF_RADIUS)
                .map(|uid| UnitFacts::of(g, *uid).strength)
                .sum();
            let relief = need.strength - local;
            if relief > 0.0 {
                rows.push(Objective {
                    kind: ObjectiveKind::Relieve,
                    key: ObjectiveKey::Relieve(*cid),
                    at: *pos,
                    value,
                    requirement: ForceNeed {
                        strength: relief,
                        melee: 0,
                        ranged: 0,
                        siege: 0,
                        bodies: 0,
                    },
                    deadline: Some(deadline),
                    state: RowState::Open,
                    depends_on: Some(ObjectiveKey::Defend(*cid)),
                    land: true,
                    sea: false,
                    label: city.name.clone(),
                    urgent: false,
                });
            }
        }

        // Siege: the plan's target city and the campaign's cities, in order,
        // while their owner is at war with us; the arena's flag.
        let mut siege_cities: Vec<u32> = Vec::new();
        if let Some(target) = plan.target_city {
            siege_cities.push(target);
        }
        if self.city_campaign_stands(g, pid) {
            if let Some(campaign) = self.campaign.as_ref() {
                for cid in &campaign.cities {
                    if !siege_cities.contains(cid) {
                        siege_cities.push(*cid);
                    }
                }
            }
        }
        if arena {
            // An arena poses at most a city or two, and the fight is over it.
            let mut theirs: Vec<u32> = g
                .cities
                .values()
                .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
                .map(|city| city.id)
                .collect();
            theirs.sort_unstable();
            for cid in theirs {
                if !siege_cities.contains(&cid) {
                    siege_cities.push(cid);
                }
            }
        }
        let mut previous_siege: Option<ObjectiveKey> = None;
        for cid in siege_cities {
            let Some(city) = g.cities.get(&cid) else {
                continue;
            };
            if city.owner == pid || !g.is_at_war(pid, city.owner) {
                continue;
            }
            let state = if self.siege_train {
                match self.sieges.get(&cid).map(|siege| siege.stage) {
                    Some(SiegeStage::Stage) => RowState::Staging,
                    Some(SiegeStage::Invest) => RowState::Investing,
                    Some(SiegeStage::Reduce) => RowState::Reducing,
                    Some(SiegeStage::Take) => RowState::Taking,
                    Some(SiegeStage::Hold) => RowState::Held,
                    None => RowState::Open,
                }
            } else {
                RowState::Open
            };
            let key = ObjectiveKey::Siege(cid);
            rows.push(Objective {
                kind: ObjectiveKind::Siege,
                key,
                at: city.pos,
                value: city_value(g, cid, lane).max(POP_VALUE),
                requirement: self.siege_requirement(g, pid, cid),
                deadline: None,
                state,
                depends_on: previous_siege,
                land: true,
                sea: BasicAi::city_is_coastal(g, cid),
                label: city.name.clone(),
                urgent: false,
            });
            previous_siege = Some(key);
        }
        if arena {
            let from = pool.first().map_or((0, 0), |uid| g.units[uid].pos);
            if let Some(flag) = g.arena_enemy_flag(pid, from) {
                rows.push(Objective {
                    kind: ObjectiveKind::Siege,
                    key: ObjectiveKey::Flag(flag),
                    at: flag,
                    value: 1_000.0,
                    requirement: ForceNeed {
                        strength: 0.0,
                        melee: 1,
                        ranged: 0,
                        siege: 0,
                        bodies: 0,
                    },
                    deadline: None,
                    state: RowState::Open,
                    depends_on: None,
                    land: true,
                    sea: false,
                    label: "the flag".to_string(),
                    urgent: false,
                });
            }
        }

        // Destroy: hostile forces in the field not covered by a Defend, and
        // not the defenders of a city under Siege — the campaign bill already
        // counts those.
        let siege_at: Vec<Pos> = rows
            .iter()
            .filter(|row| row.kind == ObjectiveKind::Siege)
            .map(|row| row.at)
            .collect();
        for force in self.hostile_forces(g, pid, visible, &our_cities) {
            let tiles: Vec<Pos> = force.iter().map(|uid| g.units[uid].pos).collect();
            let Some(at) = medoid(g, &tiles) else {
                continue;
            };
            let covered = our_cities.iter().any(|(cid, pos)| {
                defended.contains(cid) && g.wdist(*pos, at) <= THREAT_RELIEF_RADIUS
            }) || siege_at
                .iter()
                .any(|pos| g.wdist(*pos, at) <= THREAT_RELIEF_RADIUS);
            if covered {
                continue;
            }
            let strength: f64 = force
                .iter()
                .map(|uid| {
                    effective_strength(g.unit_strength(&g.units[uid], true), g.units[uid].hp)
                })
                .sum();
            let value: f64 = force.iter().map(|uid| unit_value(g, *uid)).sum();
            let naval = force.iter().any(|uid| BasicAi::waterborne(g, *uid));
            let land = force.iter().any(|uid| !BasicAi::waterborne(g, *uid));
            let kind = &g.units[&force[0]].kind;
            rows.push(Objective {
                kind: ObjectiveKind::Destroy,
                key: ObjectiveKey::Destroy(force[0]),
                at,
                value: value.max(1.0),
                requirement: ForceNeed {
                    strength: strength * DESTROY_MARGIN,
                    melee: 1,
                    ranged: 0,
                    siege: 0,
                    bodies: 0,
                },
                deadline: None,
                state: RowState::Open,
                depends_on: None,
                land: land || !naval,
                sea: naval,
                label: format!("{} of {}", force.len(), crate::reasoning::plain(kind)),
                urgent: false,
            });
        }

        // ClearCamp: camps within reach of a city, early.
        if !arena && turn < g.standard_duration(CAMP_TURN_LIMIT) {
            for camp in g.barb_camps.keys() {
                let home = our_cities
                    .iter()
                    .any(|(_, pos)| g.wdist(*pos, *camp) <= CAMP_RADIUS);
                if !home || self.base.camp_is_a_neighbours_problem(g, pid, *camp) {
                    continue;
                }
                let guard = g
                    .unit_ids_at(*camp)
                    .iter()
                    .filter(|uid| {
                        let unit = &g.units[uid];
                        Some(unit.owner) == g.barb_pid
                            && g.rules.units[unit.kind].class == "military"
                    })
                    .max_by(|a, b| {
                        let power = |uid: &u32| {
                            effective_strength(
                                g.unit_strength(&g.units[uid], true),
                                g.units[uid].hp,
                            )
                        };
                        power(a).total_cmp(&power(b)).then_with(|| b.cmp(a))
                    })
                    .copied();
                let (guard_strength, guard_cost) = guard.map_or((0.0, 0.0), |uid| {
                    (
                        effective_strength(g.unit_strength(&g.units[&uid], true), g.units[&uid].hp),
                        g.rules.units[g.units[&uid].kind].cost,
                    )
                });
                rows.push(Objective {
                    kind: ObjectiveKind::ClearCamp,
                    key: ObjectiveKey::Camp(*camp),
                    at: *camp,
                    value: CAMP_VALUE + guard_cost,
                    requirement: ForceNeed {
                        strength: guard_strength * CAMP_MARGIN,
                        melee: 1,
                        ranged: 0,
                        siege: 0,
                        bodies: 0,
                    },
                    deadline: None,
                    state: RowState::Open,
                    depends_on: None,
                    land: true,
                    sea: false,
                    label: format!("camp at {camp:?}"),
                    urgent: false,
                });
            }
        }

        // Escort: every settler and builder outside our borders.
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            let settler = unit.kind == "settler";
            if !(settler || unit.kind == "builder") || inside_borders(g, pid, unit.pos) {
                continue;
            }
            let cost = g.rules.units[unit.kind].cost;
            let value = if settler {
                cost + SETTLER_PREMIUM
            } else {
                cost
            };
            let remembered =
                self.hostile_strength_near(g, pid, unit.pos, THREAT_RELIEF_RADIUS, visible) > 0.0;
            let reach = self.barbarian_reach(g, pid, unit.pos, THREAT_RELIEF_RADIUS + 2);
            let nearest = reach.nearest(g, unit.pos);
            let deadline =
                (nearest < i32::MAX).then(|| ((f64::from(nearest) / 2.0).ceil() as u32).max(1));
            // A guard already bound to the settler serves the row.
            let guarded = g.player_unit_ids(pid).into_iter().any(|guard| {
                g.units[&guard].linked_to == Some(uid)
                    && g.rules.units[g.units[&guard].kind].class == "military"
            });
            let embarked = g.is_embarked(unit);
            rows.push(Objective {
                kind: ObjectiveKind::Escort,
                key: ObjectiveKey::Escort(uid),
                at: unit.pos,
                value,
                requirement: ForceNeed {
                    strength: 0.0,
                    melee: usize::from(!guarded),
                    ranged: usize::from(remembered),
                    siege: 0,
                    bodies: 0,
                },
                deadline,
                state: RowState::Open,
                depends_on: None,
                land: !embarked,
                sea: embarked,
                label: crate::reasoning::plain(&unit.kind),
                urgent: false,
            });
        }

        // Deter: the strongest bordering major, while we are the weaker.
        if !arena {
            if let Some((_, contact)) = self.deter_target(g, pid, &our_cities) {
                if let Some(city) = g.cities.get(&contact) {
                    rows.push(Objective {
                        kind: ObjectiveKind::Deter,
                        key: ObjectiveKey::Deter(contact),
                        at: city.pos,
                        value: DETER_SHARE * city_value(g, contact, lane).max(POP_VALUE),
                        requirement: ForceNeed::default(),
                        deadline: None,
                        state: RowState::Open,
                        depends_on: None,
                        land: true,
                        sea: false,
                        label: city.name.clone(),
                        urgent: false,
                    });
                }
            }
        }

        // Recon: unexplored sectors near home that no scout holds.
        if !arena && !our_cities.is_empty() {
            let scouts: Vec<Pos> = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| BasicAi::unit_doctrine(g, *uid) == UnitDoctrine::Recon)
                .map(|uid| g.units[&uid].pos)
                .collect();
            let explored = &g.players[pid].explored;
            let scout_value = g.rules.units.get("scout").map_or(30.0, |spec| spec.cost);
            let columns = (g.map.width + SECTOR - 1) / SECTOR;
            let mut sectors: Vec<(i32, u32, Pos)> = Vec::new();
            for row in 0..(g.map.height + SECTOR - 1) / SECTOR {
                for column in 0..columns {
                    let mut tiles = 0usize;
                    let mut land = 0usize;
                    let mut seen = 0usize;
                    for r in row * SECTOR..((row + 1) * SECTOR).min(g.map.height) {
                        for c in column * SECTOR..((column + 1) * SECTOR).min(g.map.width) {
                            let pos = crate::hex::offset_to_axial(c, r);
                            let Some(tile) = g.map.get(pos) else {
                                continue;
                            };
                            tiles += 1;
                            if !g.rules.is_water(tile) {
                                land += 1;
                            }
                            if explored.contains(&pos) {
                                seen += 1;
                            }
                        }
                    }
                    if tiles == 0 || land * 3 < tiles {
                        continue;
                    }
                    if seen as f64 >= SECTOR_EXPLORED_SHARE * tiles as f64 {
                        continue;
                    }
                    let centre = crate::hex::offset_to_axial(
                        (column * SECTOR + SECTOR / 2).min(g.map.width - 1),
                        (row * SECTOR + SECTOR / 2).min(g.map.height - 1),
                    );
                    let home = our_cities
                        .iter()
                        .map(|(_, pos)| g.wdist(*pos, centre))
                        .min()
                        .unwrap_or(i32::MAX);
                    if home > RECON_REACH {
                        continue;
                    }
                    if scouts
                        .iter()
                        .any(|pos| g.wdist(*pos, centre) <= RECON_HOLD_RADIUS)
                    {
                        continue;
                    }
                    sectors.push((home, (row * columns + column) as u32, centre));
                }
            }
            sectors.sort_unstable();
            for (_, index, centre) in sectors.into_iter().take(RECON_ROWS_MAX) {
                rows.push(Objective {
                    kind: ObjectiveKind::Recon,
                    key: ObjectiveKey::Recon(index),
                    at: centre,
                    value: scout_value,
                    requirement: ForceNeed {
                        strength: 0.0,
                        melee: 0,
                        ranged: 0,
                        siege: 0,
                        bodies: 1,
                    },
                    deadline: None,
                    state: RowState::Open,
                    depends_on: None,
                    land: true,
                    sea: false,
                    label: format!("sector {index}"),
                    urgent: false,
                });
            }
        }
        rows
    }

    /// Rank the rows: value over deadline, an urgent Defend above every
    /// offensive row, and no row above one it depends on.
    fn rank_rows(
        g: &Game,
        rows: &mut Vec<Objective>,
        pool: &[u32],
        facts: &BTreeMap<u32, UnitFacts>,
    ) {
        // A Defend is urgent when its deadline is inside the relief time of
        // the nearest body that is not already within reach of the city.
        for row in rows.iter_mut() {
            if row.kind != ObjectiveKind::Defend {
                continue;
            }
            let Some(deadline) = row.deadline else {
                continue;
            };
            let relief = pool
                .iter()
                .filter_map(|uid| facts.get(uid))
                .filter(|unit| g.wdist(unit.pos, row.at) > THREAT_RELIEF_RADIUS)
                .map(|unit| unit.travel_turns(g, row.at, THREAT_RELIEF_RADIUS))
                .min()
                .unwrap_or(u32::MAX);
            row.urgent = deadline <= relief;
        }
        rows.sort_by(|a, b| {
            let tier = |row: &Objective| {
                if row.urgent {
                    0u8
                } else if row.kind.offensive() {
                    2
                } else {
                    1
                }
            };
            // The urgent Defend outranks every offensive row; everything else
            // is value over deadline. Tier one (defensive, not urgent) and
            // tier two (offensive) interleave by score; tier zero leads.
            let (ta, tb) = (tier(a), tier(b));
            match (ta == 0, tb == 0) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            b.score()
                .total_cmp(&a.score())
                .then_with(|| a.key.cmp(&b.key))
        });
        // No row above one it depends on.
        for _ in 0..rows.len() {
            let mut moved = false;
            for index in 0..rows.len() {
                let Some(dependency) = rows[index].depends_on else {
                    continue;
                };
                let Some(at) = rows.iter().position(|row| row.key == dependency) else {
                    continue;
                };
                if at > index {
                    let row = rows.remove(index);
                    rows.insert(at, row);
                    moved = true;
                    break;
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// The board, once a turn: rows, rank, allocation, requisitions, the
    /// census and the journal line.
    fn assess_board(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        let visible = self.battlefront_visibility(g, pid);
        let pool = self.board_pool(g, pid);
        let facts: BTreeMap<u32, UnitFacts> = pool
            .iter()
            .map(|uid| (*uid, UnitFacts::of(g, *uid)))
            .collect();
        let mut rows = self.board_rows(g, pid, plan, &visible, &pool);
        Self::rank_rows(g, &mut rows, &pool, &facts);

        // Last turn's forces, less the dead and the departed.
        let pool_set: BTreeSet<u32> = pool.iter().copied().collect();
        let mut forces = std::mem::take(&mut self.objective_board_state.forces);
        let previous_key: BTreeMap<u32, ObjectiveKey> = forces
            .iter()
            .flat_map(|force| {
                force
                    .units
                    .iter()
                    .map(move |uid| (*uid, force.objective_key))
            })
            .collect();
        for force in &mut forces {
            force.units.retain(|uid| pool_set.contains(uid));
        }
        // Match forces to rows: by key, a Destroy by where it was aimed, the
        // Reserve always.
        let mut taken_rows: BTreeSet<(ObjectiveKey, bool)> = BTreeSet::new();
        let row_keys: BTreeSet<ObjectiveKey> = rows.iter().map(|row| row.key).collect();
        for force in &mut forces {
            if force.objective_key == ObjectiveKey::Reserve {
                continue;
            }
            if row_keys.contains(&force.objective_key)
                && taken_rows.insert((force.objective_key, force.domain == ForceDomain::Sea))
            {
                continue;
            }
            if let ObjectiveKey::Destroy(_) = force.objective_key {
                let rekey = rows
                    .iter()
                    .filter(|row| matches!(row.key, ObjectiveKey::Destroy(_)))
                    .filter(|row| {
                        !taken_rows.contains(&(row.key, force.domain == ForceDomain::Sea))
                    })
                    .filter(|row| g.wdist(row.at, force.aimed_at) <= DESTROY_REKEY_RADIUS)
                    .min_by_key(|row| (g.wdist(row.at, force.aimed_at), row.key))
                    .map(|row| row.key);
                if let Some(key) = rekey {
                    force.objective_key = key;
                    taken_rows.insert((key, force.domain == ForceDomain::Sea));
                    continue;
                }
            }
            // The row is gone: the force dissolves.
            force.units.clear();
        }
        forces.retain(|force| {
            !force.units.is_empty() || force.objective_key == ObjectiveKey::Reserve
        });

        // Allocation.
        let mut assignment: BTreeMap<u32, usize> = BTreeMap::new();
        for (index, force) in forces.iter().enumerate() {
            for uid in &force.units {
                assignment.insert(*uid, index);
            }
        }
        let row_rank: BTreeMap<ObjectiveKey, usize> = rows
            .iter()
            .enumerate()
            .map(|(rank, row)| (row.key, rank))
            .collect();
        let need_of = |key: ObjectiveKey, rows: &[Objective]| -> ForceNeed {
            rows.iter()
                .find(|row| row.key == key)
                .map_or(ForceNeed::default(), |row| row.requirement)
        };
        let have_of = |force: &TaskForce, facts: &BTreeMap<u32, UnitFacts>| -> ForceNeed {
            let mut have = ForceNeed::default();
            for uid in &force.units {
                if let Some(unit) = facts.get(uid) {
                    have.add(unit);
                }
            }
            have
        };
        let mut next_id = self.objective_board_state.next_force_id.max(1);
        for rank in 0..rows.len() {
            let row = rows[rank].clone();
            if row.kind == ObjectiveKind::Deter || row.requirement.is_zero() {
                continue;
            }
            for domain in [ForceDomain::Land, ForceDomain::Sea] {
                let eligible = match domain {
                    ForceDomain::Land => row.land,
                    ForceDomain::Sea => row.sea,
                };
                if !eligible {
                    continue;
                }
                let mut force_index = forces
                    .iter()
                    .position(|force| force.objective_key == row.key && force.domain == domain);
                let mut have = force_index.map_or(ForceNeed::default(), |index| {
                    have_of(&forces[index], &facts)
                });
                loop {
                    let unmet = row.requirement.unmet(&have);
                    if unmet.is_zero() {
                        break;
                    }
                    let stop = match row.kind {
                        ObjectiveKind::Defend | ObjectiveKind::Relieve => 2,
                        ObjectiveKind::Siege => 3,
                        _ => 1,
                    };
                    let mut best: Option<(f64, u32)> = None;
                    for uid in &pool {
                        let unit = &facts[uid];
                        if unit.domain != domain {
                            continue;
                        }
                        if row.kind == ObjectiveKind::Recon && !unit.recon {
                            continue;
                        }
                        if unit.recon
                            && row.kind != ObjectiveKind::Recon
                            && row.requirement.strength > 0.0
                            && unit.strength < 15.0
                        {
                            // A scout is not a body for a fight.
                            continue;
                        }
                        let distance = g.wdist(unit.pos, row.at);
                        match row.kind {
                            ObjectiveKind::Defend
                                if !row.urgent && distance > THREAT_RELIEF_RADIUS =>
                            {
                                continue
                            }
                            ObjectiveKind::Relieve if distance <= THREAT_RELIEF_RADIUS => continue,
                            _ => {}
                        }
                        let here = unit.contribution(&unmet);
                        if here <= 0.0 {
                            continue;
                        }
                        let travel = unit.travel_turns(g, row.at, stop);
                        let late = row
                            .deadline
                            .map_or(0, |deadline| travel.saturating_sub(deadline));
                        let arrival = LATE_FACTOR.powi(late as i32);
                        let score_here = here * arrival / (1.0 + f64::from(travel));
                        if let Some(current) = assignment.get(uid).copied() {
                            if current == force_index.unwrap_or(usize::MAX) {
                                continue;
                            }
                            let there = &forces[current];
                            if there.objective_key != ObjectiveKey::Reserve {
                                // What the unit is worth where it stands: its
                                // contribution to what its row would lack
                                // without it.
                                let need_there = need_of(there.objective_key, &rows);
                                let mut have_there = have_of(there, &facts);
                                have_there.remove(unit);
                                let unmet_there = need_there.unmet(&have_there);
                                let contribution_there = unit.contribution(&unmet_there);
                                let travel_there = unit.travel_turns(g, there.aimed_at, 1);
                                let score_there =
                                    contribution_there / (1.0 + f64::from(travel_there));
                                let higher = row_rank
                                    .get(&there.objective_key)
                                    .is_some_and(|other| *other < rank);
                                if !row.urgent {
                                    if higher && contribution_there > 0.0 {
                                        // Never strip a served higher row.
                                        continue;
                                    }
                                    if score_here < HYSTERESIS_GAIN * score_there {
                                        continue;
                                    }
                                }
                            }
                        }
                        if best.is_none_or(|(score, id)| {
                            score_here > score || (score_here == score && *uid < id)
                        }) {
                            best = Some((score_here, *uid));
                        }
                    }
                    let Some((_, uid)) = best else {
                        break;
                    };
                    // Move the unit.
                    if let Some(current) = assignment.remove(&uid) {
                        forces[current].units.retain(|other| *other != uid);
                    }
                    let index = match force_index {
                        Some(index) => index,
                        None => {
                            forces.push(TaskForce {
                                id: next_id,
                                objective_key: row.key,
                                domain,
                                units: Vec::new(),
                                rally: row.at,
                                doctrine_state: ForcePosture::Muster,
                                aimed_at: row.at,
                                formed: g.turn,
                            });
                            next_id += 1;
                            force_index = Some(forces.len() - 1);
                            forces.len() - 1
                        }
                    };
                    forces[index].units.push(uid);
                    forces[index].aimed_at = row.at;
                    assignment.insert(uid, index);
                    have.add(&facts[&uid]);
                }
            }
        }
        // Every force re-aimed at its row.
        for force in &mut forces {
            if let Some(row) = rows.iter().find(|row| row.key == force.objective_key) {
                force.aimed_at = row.at;
            }
        }
        // Leftovers: the Reserve, per domain.
        let reserve_at = self.reserve_tile(g, pid, &rows);
        for domain in [ForceDomain::Land, ForceDomain::Sea] {
            let leftovers: Vec<u32> = pool
                .iter()
                .copied()
                .filter(|uid| facts[uid].domain == domain && !assignment.contains_key(uid))
                .collect();
            let existing = forces.iter().position(|force| {
                force.objective_key == ObjectiveKey::Reserve && force.domain == domain
            });
            match (existing, leftovers.is_empty()) {
                (Some(index), _) => {
                    forces[index].units = leftovers;
                    forces[index].aimed_at = reserve_at;
                    forces[index].rally = reserve_at;
                }
                (None, false) => {
                    forces.push(TaskForce {
                        id: next_id,
                        objective_key: ObjectiveKey::Reserve,
                        domain,
                        units: leftovers,
                        rally: reserve_at,
                        doctrine_state: ForcePosture::Hold,
                        aimed_at: reserve_at,
                        formed: g.turn,
                    });
                    next_id += 1;
                }
                (None, true) => {}
            }
        }
        forces.retain(|force| !force.units.is_empty());
        for force in &mut forces {
            force.units.sort_unstable();
        }
        forces.sort_by_key(|force| force.id);

        // Reassignments: a unit whose force changed row.
        let reassignments = forces
            .iter()
            .flat_map(|force| {
                force
                    .units
                    .iter()
                    .map(move |uid| (*uid, force.objective_key))
            })
            .filter(|(uid, key)| previous_key.get(uid).is_some_and(|before| before != key))
            .count() as u32;

        // Requisitions: what every row still lacks.
        let average_body = {
            let strengths: Vec<f64> = facts
                .values()
                .map(|unit| unit.strength)
                .filter(|s| *s > 0.0)
                .collect();
            if strengths.is_empty() {
                DEFAULT_BODY_STRENGTH
            } else {
                strengths.iter().sum::<f64>() / strengths.len() as f64
            }
        };
        let mut requisitions = Vec::new();
        let mut short_rows = 0u32;
        for row in &rows {
            if row.kind == ObjectiveKind::Deter || row.requirement.is_zero() {
                continue;
            }
            let mut have = ForceNeed::default();
            for force in forces.iter().filter(|force| force.objective_key == row.key) {
                let part = have_of(force, &facts);
                have.strength += part.strength;
                have.melee += part.melee;
                have.ranged += part.ranged;
                have.siege += part.siege;
                have.bodies += part.bodies;
            }
            let unmet = row.requirement.unmet(&have);
            if unmet.is_zero() {
                continue;
            }
            short_rows += 1;
            let counted = unmet.melee + unmet.ranged + unmet.siege + unmet.bodies;
            let by_strength = if unmet.strength > 0.0 {
                (unmet.strength / average_body.max(1.0)).ceil() as usize
            } else {
                0
            };
            let city = g
                .player_city_ids(pid)
                .into_iter()
                .min_by_key(|cid| (g.wdist(g.cities[cid].pos, row.at), *cid));
            requisitions.push(Requisition {
                kind: row.kind,
                count: counted.max(by_strength).max(1),
                by_turn: row.deadline.map(|deadline| g.turn + deadline),
                city,
            });
        }

        // The census and the record.
        self.census.board_rows += rows.len() as u32;
        self.census.board_forces += forces.len() as u32;
        self.census.board_reassignments += reassignments;
        self.census.board_short_rows += short_rows;
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            let named: Vec<String> = rows
                .iter()
                .take(JOURNAL_ROWS)
                .map(|row| {
                    let force = forces
                        .iter()
                        .find(|force| force.objective_key == row.key)
                        .map(|force| format!("force #{} of {}", force.id, force.units.len()))
                        .unwrap_or_else(|| "no force".to_string());
                    let deadline = row
                        .deadline
                        .map(|deadline| format!(", deadline {deadline}"))
                        .unwrap_or_default();
                    let state = if row.state == RowState::Open {
                        String::new()
                    } else {
                        format!(", {}", row.state.as_str())
                    };
                    format!(
                        "{} {} (value {:.0}, need {:.0}, {force}{deadline}{state})",
                        row.kind.as_str(),
                        row.label,
                        row.value,
                        row.requirement.strength
                    )
                })
                .collect();
            let by_kind: Vec<String> = ObjectiveKind::ALL
                .iter()
                .filter_map(|kind| {
                    let count = rows.iter().filter(|row| row.kind == *kind).count();
                    (count > 0).then(|| format!("{} {}", count, kind.as_str()))
                })
                .collect();
            let shortfall: Vec<String> = requisitions
                .iter()
                .map(|requisition| format!("{} {}", requisition.count, requisition.kind.as_str()))
                .collect();
            think!(self.journal(), Military, Strategy,
                "Board: {}", if named.is_empty() { "nothing to do".to_string() } else { named.join(" · ") };
                "rows {} [{}], {} force(s), {} reassigned, short: {}",
                rows.len(), by_kind.join(", "), forces.len(), reassignments,
                if shortfall.is_empty() { "nothing".to_string() } else { shortfall.join(", ") });
        }

        let board = &mut self.objective_board_state;
        board.rows = rows;
        board.forces = forces;
        board.next_force_id = next_id;
        board.requisitions = requisitions;
    }

    /// Where the Reserve stands: the Deter row's tile, else the frontier
    /// city nearest the strongest met rival, else the capital.
    fn reserve_tile(&self, g: &Game, pid: usize, rows: &[Objective]) -> Pos {
        if let Some(row) = rows.iter().find(|row| row.kind == ObjectiveKind::Deter) {
            return row.at;
        }
        let ours = g.player_city_ids(pid);
        let strongest = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.has_met(pid, player.id)
            })
            .max_by(|a, b| {
                g.military_power(a.id)
                    .total_cmp(&g.military_power(b.id))
                    .then_with(|| b.id.cmp(&a.id))
            })
            .map(|player| player.id);
        if let Some(rival) = strongest {
            let theirs: Vec<Pos> = g
                .player_city_ids(rival)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .collect();
            if let Some(frontier) = ours
                .iter()
                .filter_map(|cid| {
                    let pos = g.cities[cid].pos;
                    theirs
                        .iter()
                        .map(|other| g.wdist(pos, *other))
                        .min()
                        .map(|distance| (distance, *cid, pos))
                })
                .min()
            {
                return frontier.2;
            }
        }
        ours.iter()
            .find(|cid| g.cities[cid].is_capital)
            .or(ours.first())
            .map(|cid| g.cities[cid].pos)
            .or_else(|| {
                let units = g.player_unit_ids(pid);
                let tiles: Vec<Pos> = units.iter().map(|uid| g.units[uid].pos).collect();
                medoid(g, &tiles)
            })
            .unwrap_or((0, 0))
    }

    /// The far side of `objective` from the enemy near it: a passable land
    /// tile two or three out, farthest from the hostile centroid, nearest
    /// the force. The force's own medoid when nothing hostile is in sight.
    fn far_side(
        &self,
        g: &Game,
        pid: usize,
        objective: Pos,
        force_medoid: Pos,
        visible: &crate::world::TileBits,
    ) -> Pos {
        let hostile: Vec<Pos> = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, objective) <= THREAT_RELIEF_RADIUS + 2)
            .filter(|unit| self.observed(g, pid, visible, unit))
            .map(|unit| unit.pos)
            .collect();
        let Some(enemy) = medoid(g, &hostile) else {
            return force_medoid;
        };
        g.wdisk(objective, 3)
            .into_iter()
            .filter(|pos| g.wdist(*pos, objective) >= 2)
            .filter(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .max_by_key(|pos| (g.wdist(*pos, enemy), -g.wdist(*pos, force_medoid), *pos))
            .unwrap_or(force_medoid)
    }

    /// `force_groups` from the task forces: one group per force, the row's
    /// tile as objective, the posture from the row's doctrine.
    fn project_forces(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        self.force_groups.clear();
        let visible = self.battlefront_visibility(g, pid);
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
        let mut hostile_seats = enemies.clone();
        if let Some(barb) = g.barb_pid {
            if !hostile_seats.contains(&barb) {
                hostile_seats.push(barb);
            }
        }
        let objective_enemies = self.one_war_objective_enemies(g, plan.threatened_city, &enemies);
        let arena = g.is_arena();
        let muster_radius = self.base.w.muster_radius.round().max(1.0) as i32;
        let mut forces = std::mem::take(&mut self.objective_board_state.forces);
        let rows = self.objective_board_state.rows.clone();
        for force in &mut forces {
            force
                .units
                .retain(|uid| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
            if force.units.is_empty() {
                continue;
            }
            let units = force.units.clone();
            let row = rows.iter().find(|row| row.key == force.objective_key);
            let tiles: Vec<Pos> = units.iter().map(|uid| g.units[uid].pos).collect();
            let force_medoid = medoid(g, &tiles).unwrap_or(force.aimed_at);
            let objective = match row {
                Some(row) => row.at,
                // The reserve on an arena has one thing to do: the nearest
                // enemy, as the shipped objective falls back to it.
                None if arena => g
                    .units
                    .values()
                    .filter(|unit| {
                        hostile_seats.contains(&unit.owner)
                            && g.rules.units[unit.kind].class == "military"
                            && self.observed(g, pid, &visible, unit)
                    })
                    .min_by_key(|unit| (g.wdist(force_medoid, unit.pos), unit.id))
                    .map_or(force.aimed_at, |unit| unit.pos),
                None => force.aimed_at,
            };
            let kind = row.map(|row| row.kind);
            let focus_target = self.force_focus_target(g, pid, &units, &objective_enemies, plan);
            let local_strength_ratio =
                self.local_strength_ratio(g, pid, &units, &hostile_seats, objective);
            let average_hp = units.iter().map(|uid| g.units[uid].hp).sum::<i32>() as f64
                / units.len().max(1) as f64;
            let contact = units.iter().any(|uid| {
                g.units.values().any(|enemy| {
                    hostile_seats.contains(&enemy.owner)
                        && g.rules.units[enemy.kind].class == "military"
                        && self.observed(g, pid, &visible, enemy)
                        && g.wdist(g.units[uid].pos, enemy.pos) <= 2
                })
            });
            let forcing_focus = focus_target.is_some_and(|target| {
                let low_hp_unit = g.unit_ids_at(target).iter().any(|unit| {
                    hostile_seats.contains(&g.units[unit].owner)
                        && self.observed(g, pid, &visible, &g.units[unit])
                        && g.units[unit].hp <= 35
                });
                let capturable_city = g.city_at(target).is_some_and(|city| {
                    enemies.contains(&g.cities[&city].owner)
                        && (!self.battlefront_observation || g.sees(&visible, target))
                        && g.cities[&city].hp <= 40
                        && g.cities[&city].wall_hp <= 0
                        && units.iter().any(|unit| {
                            g.rules.units[g.units[unit].kind].is_melee_capable()
                                && g.wdist(g.units[unit].pos, target) <= 1
                        })
                });
                low_hp_unit || capturable_city
            });
            let force_strength: f64 = units
                .iter()
                .map(|uid| {
                    effective_strength(g.unit_strength(&g.units[uid], true), g.units[uid].hp)
                })
                .sum();
            let rally = match kind {
                Some(ObjectiveKind::Siege | ObjectiveKind::Defend | ObjectiveKind::Relieve) => {
                    self.far_side(g, pid, objective, force_medoid, &visible)
                }
                _ => force_medoid,
            };
            force.rally = rally;
            // The posture, and where a standing force stands.
            let (posture, anchor) = if !arena && average_hp <= self.base.w.withdraw_hp + 10.0 {
                (ForcePosture::Recover, force_medoid)
            } else {
                match kind {
                    Some(ObjectiveKind::Defend | ObjectiveKind::Relieve) => {
                        if contact || forcing_focus {
                            (ForcePosture::Engage, objective)
                        } else {
                            (ForcePosture::Hold, objective)
                        }
                    }
                    Some(ObjectiveKind::Siege) => {
                        let stage = match force.objective_key {
                            ObjectiveKey::Siege(cid) if self.siege_train => {
                                self.sieges.get(&cid).map(|siege| siege.stage)
                            }
                            _ => None,
                        };
                        match stage {
                            Some(SiegeStage::Stage) => (ForcePosture::Muster, rally),
                            Some(SiegeStage::Invest) => (ForcePosture::Advance, force_medoid),
                            Some(SiegeStage::Reduce | SiegeStage::Take) => {
                                (ForcePosture::Engage, force_medoid)
                            }
                            Some(SiegeStage::Hold) => (ForcePosture::Hold, objective),
                            None => {
                                let readiness = units
                                    .iter()
                                    .filter(|uid| {
                                        g.wdist(g.units[uid].pos, rally) <= muster_radius
                                            && g.units[uid].hp as f64 > self.base.w.withdraw_hp
                                    })
                                    .count() as f64
                                    / units.len().max(1) as f64;
                                if contact || forcing_focus {
                                    (ForcePosture::Engage, force_medoid)
                                } else if !arena
                                    && units.len() > 1
                                    && readiness + 1e-9 < self.base.w.muster_readiness
                                    && g.wdist(force_medoid, objective) > THREAT_RELIEF_RADIUS
                                {
                                    (ForcePosture::Muster, rally)
                                } else {
                                    (ForcePosture::Advance, force_medoid)
                                }
                            }
                        }
                    }
                    Some(ObjectiveKind::Destroy) => {
                        let hostile =
                            self.hostile_strength_near(g, pid, objective, FORCE_LINK, &visible);
                        let exchange = if hostile <= 0.0 {
                            f64::INFINITY
                        } else {
                            force_strength / hostile
                        };
                        if arena || forcing_focus || exchange >= DESTROY_ENGAGE_EXCHANGE {
                            (ForcePosture::Engage, force_medoid)
                        } else if contact {
                            // Hold defensive ground: the nearest city of ours
                            // within reach, else where the force stands.
                            let ground = g
                                .player_city_ids(pid)
                                .into_iter()
                                .map(|cid| g.cities[&cid].pos)
                                .filter(|pos| g.wdist(*pos, force_medoid) <= THREAT_RELIEF_RADIUS)
                                .min_by_key(|pos| (g.wdist(*pos, force_medoid), *pos))
                                .unwrap_or(force_medoid);
                            (ForcePosture::Hold, ground)
                        } else {
                            (ForcePosture::Hold, force_medoid)
                        }
                    }
                    Some(ObjectiveKind::ClearCamp) | Some(ObjectiveKind::Recon) => {
                        if forcing_focus {
                            (ForcePosture::Engage, force_medoid)
                        } else {
                            (ForcePosture::Advance, force_medoid)
                        }
                    }
                    Some(ObjectiveKind::Escort) => (ForcePosture::Hold, objective),
                    Some(ObjectiveKind::Deter) | None => {
                        if arena {
                            (ForcePosture::Engage, force_medoid)
                        } else {
                            (ForcePosture::Hold, force.aimed_at)
                        }
                    }
                }
            };
            force.doctrine_state = posture;
            let readiness = units
                .iter()
                .filter(|uid| {
                    g.wdist(g.units[uid].pos, anchor) <= muster_radius
                        && g.units[uid].hp as f64 > self.base.w.withdraw_hp
                })
                .count() as f64
                / units.len().max(1) as f64;
            self.force_groups.push(ForceGroup {
                id: force.id,
                domain: force.domain,
                units,
                anchor,
                objective,
                focus_target,
                posture,
                readiness,
                local_strength_ratio,
            });
        }
        forces.retain(|force| !force.units.is_empty());
        self.objective_board_state.forces = forces;
        self.force_groups.sort_by_key(|group| group.id);
        if self.journal().wants(crate::reasoning::Level::Decision) {
            for group in &self.force_groups {
                let row = self
                    .objective_board_state
                    .forces
                    .iter()
                    .find(|force| force.id == group.id)
                    .and_then(|force| rows.iter().find(|row| row.key == force.objective_key));
                let what = row.map_or("the reserve".to_string(), |row| {
                    format!("{} {}", row.kind.as_str(), row.label)
                });
                think!(self.journal(), Military, Decision,
                    "Task force #{}: a {} force of {} will {} for {what}",
                    group.id, group.domain.as_str(), group.units.len(), group.posture.as_str();
                    "objective {:?}, anchor {:?}, {:.0}% ready, {:.2} local strength",
                    group.objective, group.anchor, group.readiness * 100.0, group.local_strength_ratio;
                    group.objective);
            }
        }
        for group in &self.force_groups {
            self.census.count_posture(group.posture);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::name;

    /// A flat board of `majors` empires, every starting unit cleared, the
    /// map explored and everyone met; each capital founded at the position
    /// given, nobody at war, turn 60.
    fn flat_board(seed: u64, capitals: &[Pos], barbarians: bool) -> Game {
        let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, barbarians);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        for (pid, pos) in capitals.iter().enumerate() {
            game.found_city_for(pid, *pos, None);
        }
        for pid in 0..capitals.len() {
            for other in 0..capitals.len() {
                if pid != other {
                    game.players[pid].met.insert(other);
                }
            }
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.at_war.clear();
        game.turn = 60;
        game.current = 0;
        game
    }

    fn at(col: i32, row: i32) -> Pos {
        crate::hex::offset_to_axial(col, row)
    }

    fn war(g: &mut Game, a: usize, b: usize) {
        g.at_war.insert((a.min(b), a.max(b)));
    }

    fn spawn(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
        let uid = g.spawn_test_unit(kind, pid, pos);
        let moves = g.unit_max_moves(uid);
        let unit = g.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        uid
    }

    fn conquest(g: &Game, target: Option<u32>) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: target.map(|cid| g.cities[&cid].owner),
            target_city: target,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    fn on() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_objective_board();
        ai
    }

    fn city_of(g: &Game, pid: usize, pos: Pos) -> u32 {
        g.city_at(pos)
            .filter(|cid| g.cities[cid].owner == pid)
            .expect("a city of ours")
    }

    fn row(ai: &AdvancedAi, key: ObjectiveKey) -> Option<&Objective> {
        ai.objective_board().rows.iter().find(|row| row.key == key)
    }

    fn force_for(ai: &AdvancedAi, key: ObjectiveKey) -> Option<&TaskForce> {
        ai.objective_board()
            .forces
            .iter()
            .find(|force| force.objective_key == key)
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.objective_board, "an opt-in ships off");
        assert!(super::super::GENES.iter().any(|gene| gene.opt_in()
            && gene.tag == "objective-board"
            && gene.field == "objective_board"));
        let mut on = AdvancedAi::new();
        on.enable_objective_board();
        assert!(on.objective_board);
        on.disable_objective_board();
        assert!(!on.objective_board);
        super::super::test_support::opt_in_off_in_both_controllers("objective-board", |ai| {
            ai.objective_board
        });
        assert!(ai.requisitions().is_empty());
    }

    /// Off, the shipped layer runs and the board is never written.
    #[test]
    fn off_the_board_is_never_written() {
        let mut g = flat_board(11, &[at(6, 8), at(28, 8)], false);
        war(&mut g, 0, 1);
        spawn(&mut g, "warrior", 0, at(8, 8));
        spawn(&mut g, "warrior", 1, at(9, 8));
        let mut ai = AdvancedAi::new();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        assert!(ai.objective_board().rows.is_empty());
        assert!(ai.objective_board().forces.is_empty());
        assert_eq!(ai.force_groups.len(), 1, "the shipped proximity group");
    }

    /// Two cities under pressure at once produce two Defend rows, and both
    /// are served — the argmax-flip failure of `threatened_city` is gone.
    #[test]
    fn two_pressured_cities_produce_two_defend_rows_both_served() {
        let mut g = flat_board(3, &[at(6, 8), at(30, 8)], false);
        war(&mut g, 0, 1);
        let second = g.found_city_for(0, at(6, 16), None);
        let first = city_of(&g, 0, at(6, 8));
        // Two enemy warriors at each city, and two of ours beside each.
        for pos in [at(8, 8), at(7, 9)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        for pos in [at(8, 16), at(7, 17)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        let ours_first: Vec<u32> = [at(5, 8), at(5, 9)]
            .iter()
            .map(|pos| spawn(&mut g, "warrior", 0, *pos))
            .collect();
        let ours_second: Vec<u32> = [at(5, 16), at(5, 17)]
            .iter()
            .map(|pos| spawn(&mut g, "warrior", 0, *pos))
            .collect();
        let mut ai = on();
        let plan = conquest(&g, None);
        assert!(AdvancedAi::city_pressure(&g, 0, first) >= BASTION_PRESSURE);
        assert!(AdvancedAi::city_pressure(&g, 0, second) >= BASTION_PRESSURE);
        ai.rebuild_force_groups(&g, 0, &plan);
        let defend_first =
            row(&ai, ObjectiveKey::Defend(first)).expect("a Defend row for the first city");
        let defend_second =
            row(&ai, ObjectiveKey::Defend(second)).expect("a Defend row for the second city");
        assert!(defend_first.deadline.is_some() && defend_second.deadline.is_some());
        let force_first = force_for(&ai, ObjectiveKey::Defend(first)).expect("served");
        let force_second = force_for(&ai, ObjectiveKey::Defend(second)).expect("served");
        assert!(force_first.units.iter().all(|uid| ours_first.contains(uid)));
        assert!(force_second
            .units
            .iter()
            .all(|uid| ours_second.contains(uid)));
        assert!(!force_first.units.is_empty() && !force_second.units.is_empty());
        // And the projection: one group per force, holding or engaging at
        // its own city.
        assert_eq!(ai.force_groups.len(), 2);
        for group in &ai.force_groups {
            assert!(matches!(
                group.posture,
                ForcePosture::Hold | ForcePosture::Engage
            ));
            assert!(
                group.objective == g.cities[&first].pos || group.objective == g.cities[&second].pos
            );
        }
    }

    /// A row served first keeps its need: a lower row cannot strip it.
    #[test]
    fn a_force_is_not_stripped_below_its_need_by_a_lower_row() {
        let mut g = flat_board(5, &[at(6, 8), at(30, 8)], false);
        war(&mut g, 0, 1);
        let home = city_of(&g, 0, at(6, 8));
        // Three enemy warriors at our city: a Defend that needs everyone.
        for pos in [at(8, 8), at(7, 9), at(7, 7)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        // One enemy warrior in the field, fifteen tiles out, seen by a scout
        // of ours (no body for a fight): a Destroy row with nothing free.
        let stray = spawn(&mut g, "warrior", 1, at(18, 14));
        let scout = spawn(&mut g, "scout", 0, at(16, 14));
        let ours: Vec<u32> = [at(5, 8), at(5, 9), at(4, 8)]
            .iter()
            .map(|pos| spawn(&mut g, "warrior", 0, *pos))
            .collect();
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let board = ai.objective_board();
        let defend = board
            .rows
            .iter()
            .position(|row| row.key == ObjectiveKey::Defend(home))
            .expect("a Defend row");
        let destroy = board
            .rows
            .iter()
            .position(|row| row.key == ObjectiveKey::Destroy(stray))
            .expect("a Destroy row for the stray");
        assert!(defend < destroy, "the Defend ranks above the Destroy");
        let force = force_for(&ai, ObjectiveKey::Defend(home)).expect("served");
        assert_eq!(force.units, ours, "every warrior holds the city");
        assert!(
            force_for(&ai, ObjectiveKey::Destroy(stray)).is_none(),
            "nothing left to strip"
        );
        assert!(!force.units.contains(&scout));
        assert!(ai
            .requisitions()
            .iter()
            .any(|req| req.kind == ObjectiveKind::Destroy));
    }

    /// A task force keeps its id across turns and across the death of its
    /// lowest-id member.
    #[test]
    fn a_task_force_id_survives_the_death_of_its_lowest_member() {
        let mut g = flat_board(7, &[at(6, 8), at(30, 8)], false);
        war(&mut g, 0, 1);
        let home = city_of(&g, 0, at(6, 8));
        for pos in [at(8, 8), at(7, 9)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        let ours: Vec<u32> = [at(5, 8), at(5, 9), at(4, 8)]
            .iter()
            .map(|pos| spawn(&mut g, "warrior", 0, *pos))
            .collect();
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let before = force_for(&ai, ObjectiveKey::Defend(home))
            .expect("served")
            .clone();
        // Two warriors meet the need; the third is the reserve.
        assert_eq!(before.units, ours[..2].to_vec());
        assert!(force_for(&ai, ObjectiveKey::Reserve)
            .is_some_and(|reserve| reserve.units == vec![ours[2]]));
        let lowest = *before.units.iter().min().unwrap();
        g.remove_unit(lowest);
        g.turn += 1;
        ai.rebuild_force_groups(&g, 0, &plan);
        let after = force_for(&ai, ObjectiveKey::Defend(home)).expect("still served");
        assert_eq!(after.id, before.id, "the force's id is stable");
        assert!(!after.units.contains(&lowest));
        // The row is short again, so the reserve warrior is pulled in.
        assert_eq!(after.units, vec![ours[1], ours[2]]);
        assert_eq!(ai.census.board_reassignments, 1, "one unit changed row");
        let group = ai
            .force_groups
            .iter()
            .find(|group| group.id == before.id)
            .expect("the group carries the force's id");
        assert_eq!(group.units, after.units);
    }

    /// A Siege row asks the campaign's own bill times the margin, with a
    /// melee taker and siege while the walls stand.
    #[test]
    fn a_siege_rows_requirement_is_the_campaign_bill_times_the_margin_with_a_taker() {
        let mut g = flat_board(9, &[at(6, 8), at(20, 8)], false);
        war(&mut g, 0, 1);
        let target = city_of(&g, 1, at(20, 8));
        g.cities.get_mut(&target).unwrap().wall_hp = 50;
        for pos in [at(21, 8), at(19, 8)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        for pos in [at(8, 8), at(8, 9), at(7, 8), at(9, 8)] {
            spawn(&mut g, "warrior", 0, pos);
        }
        let mut ai = on();
        let plan = conquest(&g, Some(target));
        ai.rebuild_force_groups(&g, 0, &plan);
        let siege = row(&ai, ObjectiveKey::Siege(target)).expect("a Siege row");
        let appraisal = ai.appraise_neighbour(&g, 0, 1).expect("a neighbour");
        let army = ai.campaign_field_army(&g, 0);
        let average = AdvancedAi::campaign_strength_of(&g, &army) / army.len() as f64;
        let bill = ai.campaign_city_requirement(&g, 0, target, &appraisal, average);
        assert!((siege.requirement.strength - bill.strength * SIEGE_MARGIN).abs() < 1e-9);
        assert_eq!(siege.requirement.melee, 1, "a melee taker");
        assert_eq!(siege.requirement.siege, 1, "siege while the walls stand");
        assert!(siege.value > 0.0);
        let force = force_for(&ai, ObjectiveKey::Siege(target)).expect("a siege force");
        assert!(!force.units.is_empty());
        assert!(ai
            .force_groups
            .iter()
            .any(|group| group.objective == g.cities[&target].pos));
    }

    /// A Defend whose deadline is inside the relief time outranks a Siege of
    /// far higher value.
    #[test]
    fn a_defend_inside_relief_time_outranks_a_siege_of_higher_value() {
        let mut g = flat_board(13, &[at(6, 8), at(30, 8)], false);
        war(&mut g, 0, 1);
        let home = city_of(&g, 0, at(6, 8));
        let target = city_of(&g, 1, at(30, 8));
        // Their capital is worth a great deal…
        {
            let city = g.cities.get_mut(&target).unwrap();
            city.pop = 20;
            for _ in 0..3 {
                city.buildings.push(name!("monument"));
            }
        }
        // …and ours is under the gun, taking damage.
        for pos in [at(8, 8), at(7, 9), at(7, 7)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        let ours: Vec<u32> = [at(5, 8), at(5, 9), at(18, 8)]
            .iter()
            .map(|pos| spawn(&mut g, "warrior", 0, *pos))
            .collect();
        let mut ai = on();
        let plan = conquest(&g, Some(target));
        ai.rebuild_force_groups(&g, 0, &plan);
        // A turn of damage gives the Defend its rate and deadline.
        g.cities.get_mut(&home).unwrap().hp = 140;
        g.cities.get_mut(&home).unwrap().last_attacked = g.turn;
        g.turn += 1;
        ai.rebuild_force_groups(&g, 0, &plan);
        let board = ai.objective_board();
        let defend = board
            .rows
            .iter()
            .position(|row| row.key == ObjectiveKey::Defend(home))
            .expect("Defend");
        let siege = board
            .rows
            .iter()
            .position(|row| row.key == ObjectiveKey::Siege(target))
            .expect("Siege");
        assert!(
            board.rows[siege].value > board.rows[defend].value,
            "the siege is worth more"
        );
        assert!(
            board.rows[defend].urgent,
            "the deadline is inside the relief time"
        );
        assert!(defend < siege, "and the Defend ranks first");
        assert!(board.rows[defend]
            .deadline
            .is_some_and(|deadline| deadline >= DEFEND_DEADLINE_FLOOR));
        // The far warrior is pulled home by the urgent Defend.
        let force = force_for(&ai, ObjectiveKey::Defend(home)).expect("served");
        assert!(
            force.units.contains(&ours[2]),
            "an urgent Defend pulls anyone"
        );
    }

    /// A camp within nine of a city is a row before turn 100 and not after.
    #[test]
    fn a_camp_within_nine_is_a_row_before_turn_100_and_not_after() {
        let mut g = flat_board(17, &[at(6, 8), at(30, 8)], true);
        let camp = at(12, 10);
        g.barb_camps.insert(camp, g.turn);
        assert!(g.wdist(camp, at(6, 8)) <= CAMP_RADIUS);
        spawn(&mut g, "warrior", 0, at(7, 8));
        let mut ai = on();
        let plan = conquest(&g, None);
        g.turn = g.standard_duration(CAMP_TURN_LIMIT) - 1;
        ai.rebuild_force_groups(&g, 0, &plan);
        let camp_row = row(&ai, ObjectiveKey::Camp(camp)).expect("a ClearCamp row before turn 100");
        assert_eq!(camp_row.kind, ObjectiveKind::ClearCamp);
        assert!(
            (camp_row.value - CAMP_VALUE).abs() < 1e-9,
            "an unguarded camp is worth the base"
        );
        assert_eq!(camp_row.requirement.melee, 1);
        let force = force_for(&ai, ObjectiveKey::Camp(camp)).expect("a force walks to it");
        assert_eq!(force.doctrine_state, ForcePosture::Advance);
        g.turn = g.standard_duration(CAMP_TURN_LIMIT);
        ai.rebuild_force_groups(&g, 0, &plan);
        assert!(
            row(&ai, ObjectiveKey::Camp(camp)).is_none(),
            "no row after turn 100"
        );
        assert!(ai
            .objective_board()
            .forces
            .iter()
            .all(|force| force.objective_key == ObjectiveKey::Reserve));
    }

    /// A settler outside our borders is an Escort row; its shortfall is a
    /// requisition.
    #[test]
    fn a_settler_outside_the_borders_is_an_escort_row_and_a_requisition() {
        let mut g = flat_board(19, &[at(6, 8), at(30, 8)], false);
        let settler = spawn(&mut g, "settler", 0, at(16, 14));
        assert!(!inside_borders(&g, 0, at(16, 14)));
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let escort = row(&ai, ObjectiveKey::Escort(settler)).expect("an Escort row");
        let cost = g.rules.units.get("settler").unwrap().cost;
        assert!((escort.value - (cost + SETTLER_PREMIUM)).abs() < 1e-9);
        assert_eq!(escort.requirement.melee, 1);
        let requisitions = ai.requisitions();
        assert!(requisitions
            .iter()
            .any(|req| req.kind == ObjectiveKind::Escort && req.count == 1));
    }
}
