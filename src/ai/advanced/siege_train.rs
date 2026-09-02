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
/// A City Center strikes this far; nothing stands inside it before the
/// train is staged.
pub(super) const CITY_STRIKE_RANGE: i32 = 2;
/// The bill is the defence within [`DEFENDER_RADIUS`] plus the city and its
/// walls at [`WALL_STRENGTH_PER_100_HP`] a hundred, times this.
pub(super) const BILL_MARGIN: f64 = 1.25;
pub(super) const DEFENDER_RADIUS: i32 = 6;
pub(super) const WALL_STRENGTH_PER_100_HP: f64 = 10.0;
/// Under this share of the bill the train falls back to the staging ring.
pub(super) const ABORT_SHARE: f64 = 0.8;
/// Melee holds the ring rather than swinging at a wall above this fraction
/// of its pool, unless a ram or tower stands beside the city.
pub(super) const MELEE_WALL_FRACTION: f64 = 0.2;
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

/// Which arm of the train a unit is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arm {
    Melee,
    Siege,
    Shooter,
    Other,
}

fn arm_of(g: &Game, uid: u32) -> Arm {
    let Some(unit) = g.units.get(&uid) else {
        return Arm::Other;
    };
    let spec = &g.rules.units[unit.kind];
    if spec.class != "military"
        || matches!(spec.domain.as_deref(), Some("sea" | "air"))
        || unit.linked_to.is_some()
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

/// The train: the group's land combat units and any of ours already within
/// reach of the city, so two groups on one city read one bill.
fn siege_force(g: &Game, pid: usize, city: &CityView, group_units: &[u32]) -> Vec<u32> {
    let mut force: BTreeSet<u32> = group_units
        .iter()
        .copied()
        .filter(|uid| arm_of(g, *uid) != Arm::Other)
        .collect();
    for uid in g.player_unit_ids(pid) {
        if arm_of(g, uid) != Arm::Other && g.wdist(g.units[&uid].pos, city.pos) <= OBJECTIVE_REACH {
            force.insert(uid);
        }
    }
    force.into_iter().collect()
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

/// Our battering ram and siege tower beside the city, in that order — the
/// adjacency `siege_support_effects` reads.
fn siege_support_adjacent(g: &Game, pid: usize, city_pos: Pos) -> (bool, bool) {
    let adjacent = |kind: &str| {
        g.nbrs(city_pos).into_iter().any(|pos| {
            g.unit_ids_at(pos)
                .iter()
                .any(|id| g.units[id].owner == pid && g.units[id].kind == kind)
        })
    };
    (adjacent("battering_ram"), adjacent("siege_tower"))
}

/// The taker's expected blow on the city as `do_attack` would land it: the
/// attacker's strength at its hit points against `city_strength`, at the
/// centre of the roll, routed through the wall pool.
pub(super) fn taker_blow(g: &Game, pid: usize, uid: u32, cid: u32) -> f64 {
    let (Some(unit), Some(city)) = (g.units.get(&uid), CityView::of(g, cid)) else {
        return 0.0;
    };
    let att = effective_strength(g.unit_strength(unit, false), unit.hp);
    let mean = expected_damage(att, g.city_strength(cid));
    let (_, tower) = siege_support_adjacent(g, pid, city.pos);
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

/// The melee-capable unit that walks in: adjacent to the city with the most
/// movement, else able to reach a free ring tile this turn.
fn designate_taker(g: &Game, city: &CityView, force: &[u32]) -> Option<u32> {
    let candidates: Vec<u32> = force
        .iter()
        .copied()
        .filter(|uid| arm_of(g, *uid) == Arm::Melee && g.units[uid].attacks_left > 0)
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

/// The train's posts for the turn. Melee already on the ring keep their
/// tile; the taker, then the rest by distance, take free ring tiles in the
/// spread-first order — the free tile furthest from every held or assigned
/// one, then the nearest. Guns, then shooters, keep a tile they can already
/// shoot the city from, else take a tile at their range behind a ring post
/// and away from hostiles. A unit with no tile left has no post.
fn siege_posts(
    g: &Game,
    pid: usize,
    city: &CityView,
    force: &[u32],
    taker: Option<u32>,
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
        .filter(|uid| arm_of(g, *uid) == Arm::Melee)
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
        let best = ring_free
            .iter()
            .copied()
            .filter(|pos| !ring_taken.contains(pos) && g.unit_can_traverse(uid, *pos))
            .min_by_key(|pos| {
                let spread = ring_taken
                    .iter()
                    .map(|held| g.wdist(*pos, *held))
                    .min()
                    .unwrap_or(i32::MAX);
                (Reverse(spread), g.wdist(here, *pos), *pos)
            });
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
        .filter(|uid| matches!(arm_of(g, *uid), Arm::Siege | Arm::Shooter))
        .collect();
    guns.sort_by_key(|uid| {
        (
            arm_of(g, *uid) != Arm::Siege,
            g.wdist(g.units[uid].pos, city.pos),
            *uid,
        )
    });
    let mut fire_taken: BTreeSet<Pos> = BTreeSet::new();
    for uid in guns {
        let here = g.units[&uid].pos;
        let range = g.unit_attack_range(uid).max(1);
        if g.wdist(here, city.pos) <= range
            && !fire_taken.contains(&here)
            && g.ranged_order_is_legal(pid, uid, city.pos, frame.as_ref(), &viewers)
        {
            posts.insert(uid, here);
            fire_taken.insert(here);
            continue;
        }
        let best = g
            .wring(city.pos, range)
            .into_iter()
            .filter(|pos| {
                *pos != here
                    && !fire_taken.contains(pos)
                    && !ring_taken.contains(pos)
                    && open_land(*pos)
                    && g.unit_can_traverse(uid, *pos)
                    && g.unit_ids_at(*pos).is_empty()
            })
            .min_by_key(|pos| {
                let behind = ring_taken.iter().any(|held| g.wdist(*held, *pos) == 1);
                let exposure = hostiles.iter().filter(|h| g.wdist(**h, *pos) <= 2).count();
                (!behind, exposure, g.wdist(here, *pos), *pos)
            });
        if let Some(pos) = best {
            posts.insert(uid, pos);
            fire_taken.insert(pos);
        }
    }
    posts
}

impl AdvancedAi {
    /// Whether the siege has reserved this unit as its taker — the hook a
    /// joint planner (`battle_planner`) reads so it does not spend the unit
    /// that walks in. Published here; the planner's read is its own change.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn unit_is_reserved(&self, uid: u32) -> bool {
        self.reserved_units.contains(&uid)
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
        if !self.siege_train && !self.anvil {
            return None;
        }
        if arm_of(g, uid) == Arm::Other || self.guard_is_bound_to_any_settler(uid) {
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
        if self.siege_train {
            if let Some(cid) = self.siege_city_of(g, pid, plan, &group) {
                return self.siege_train_step(g, pid, uid, cid, plan, &group);
            }
        }
        None
    }

    /// The enemy city a group is on: the one at its objective, or the
    /// plan's target city within reach while no city of ours is threatened.
    fn siege_city_of(
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

    /// The state machine, once a turn per city: the bill, the strength, the
    /// ring, the stage, the taker, the census and the journal line.
    fn assess_siege(&mut self, g: &Game, pid: usize, cid: u32, group: &ForceGroup) {
        let turn = g.turn;
        if self
            .sieges
            .get(&cid)
            .is_some_and(|siege| siege.assessed == turn)
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
        let force = siege_force(g, pid, &city, &group.units);
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

        let record = self.sieges.entry(cid).or_insert(Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: turn,
            assessed: turn,
            posts: BTreeMap::new(),
        });
        record.assessed = turn;
        let previous = record.stage;
        let mut stage = previous;
        if city.owner == pid {
            stage = SiegeStage::Hold;
        } else {
            if stage != SiegeStage::Stage && !arena && strength < ABORT_SHARE * bill {
                stage = SiegeStage::Stage;
            }
            match stage {
                SiegeStage::Stage => {
                    if (arena && gathered) || staged >= bill {
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
            taker = designate_taker(g, &city, &force);
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
            siege_posts(g, pid, &city, &force, taker)
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
            let taker_note = match taker {
                Some(uid) => format!(", taker {} reserved", g.units[&uid].kind),
                None => String::new(),
            };
            think!(self.journal(), Military, Decision,
                "Siege of {name}: {}", stage.as_str();
                "ring {sealed}/{ring} sealed, walls {}/{}, city {}/200, {} of {} units staged, \
                 {strength:.0} strength against a bill of {bill:.0}{taker_note}",
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
        self.assess_siege(g, pid, cid, group);
        let siege = self.sieges.get(&cid)?.clone();
        let city = CityView::of(g, cid)?;
        if city.owner == pid || siege.stage == SiegeStage::Hold {
            return None;
        }
        if siege.stage == SiegeStage::Stage {
            return Some(self.siege_stage_step(g, pid, uid, &city, plan));
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
        if let Some(acted) = self.siege_blow(g, pid, uid, city, plan, false) {
            return acted;
        }
        let here = g.units[&uid].pos;
        let distance = g.wdist(here, city.pos);
        if distance <= CITY_STRIKE_RANGE {
            let mut best: Option<((i32, i32, Reverse<Pos>), Pos)> = None;
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
            if let Some(next) = g
                .route_step(uid, city.pos, STAGING_FAR)
                .filter(|pos| g.can_move(uid, *pos) && g.wdist(*pos, city.pos) > CITY_STRIKE_RANGE)
            {
                return self.base.tactical_apply_move(g, pid, uid, next);
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
            let (ram, tower) = siege_support_adjacent(g, pid, city.pos);
            let allow_city = city.wall_fraction() <= MELEE_WALL_FRACTION || ram || tower;
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
            if let Some(next) = g
                .route_step(uid, city.pos, CITY_STRIKE_RANGE)
                .filter(|pos| g.can_move(uid, *pos))
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
            && !(unit.moved && g.promotion_effect(&unit, "attack_after_move") == 0.0);
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
        self.base.fortify_or_stop(g, pid, uid)
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
            if let Some(acted) = self.reliever_kill_shot(g, pid, uid, city) {
                return acted;
            }
            if city.wall_hp > 0 {
                if let Some(acted) = self.best_unit_shot(g, pid, uid) {
                    return acted;
                }
                if let Some(acted) = self.city_shot(g, pid, uid, city) {
                    return acted;
                }
            } else {
                if let Some(acted) = self.city_shot(g, pid, uid, city) {
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
        self.base.fortify_or_stop(g, pid, uid)
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

    /// Toward `goal` by explicit steps that never cross a ring tile of the
    /// city other than the goal. The engine's own route runs through the
    /// ring when that is shortest, and a unit entering the city's zone of
    /// control there is stopped on the wrong tile — measured on the
    /// three-warrior fixture, which clumped three adjacent tiles that way
    /// and left one side of the ring open. Each step closes on the goal;
    /// the router is asked only when no neighbour does, and its answer is
    /// held to the same two rules. A sideways step — no closer, but onto a
    /// tile with a closing step beyond it — is allowed once, before the
    /// unit has moved this turn, so a ring tile in the straight line can be
    /// gone round without the unit walking out and back.
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
            let Some(unit) = g.units.get(&uid) else {
                break;
            };
            let here = unit.pos;
            if here == goal || unit.moves_left <= 0.0 {
                break;
            }
            let distance = g.wdist(here, goal);
            let allow_sideways = !unit.moved;
            let ring_tile = |pos: Pos| pos != goal && g.wdist(pos, city_pos) <= 1;
            let rough = |pos: Pos| {
                g.map
                    .get(pos)
                    .is_some_and(|tile| tile.hills || tile.feature.is_some())
            };
            let onward = |pos: Pos| {
                g.nbrs(pos).into_iter().any(|next| {
                    !ring_tile(next)
                        && g.wdist(next, goal) < distance
                        && g.unit_can_traverse(uid, next)
                        && (next == goal || g.unit_ids_at(next).is_empty())
                })
            };
            let mut best: Option<((bool, i32, bool, Pos), Pos)> = None;
            for pos in g.nbrs(here) {
                if ring_tile(pos) || !g.can_move(uid, pos) {
                    continue;
                }
                let closer = g.wdist(pos, goal);
                let sideways = closer == distance;
                if closer > distance || (sideways && !(allow_sideways && onward(pos))) {
                    continue;
                }
                let key = (sideways, closer, rough(pos), pos);
                if best.as_ref().is_none_or(|(old, _)| key < *old) {
                    best = Some((key, pos));
                }
            }
            let next = match best {
                Some((_, pos)) => pos,
                None => {
                    let set: HashSet<Pos> = std::iter::once(goal).collect();
                    match g.route_step_to_any(uid, &set).filter(|pos| {
                        !ring_tile(*pos) && g.can_move(uid, *pos) && g.wdist(*pos, goal) <= distance
                    }) {
                        Some(pos) => pos,
                        None => break,
                    }
                }
            };
            if !self.base.tactical_apply_move(g, pid, uid, next) {
                break;
            }
            moved = true;
        }
        moved.then_some(true)
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
        if ranged && siege && unit.moved && g.promotion_effect(&unit, "attack_after_move") == 0.0 {
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
                result.eliminates_enemy_unit > *kill
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
        let mut best: Option<((bool, i64, Reverse<i32>, Reverse<Pos>), Pos)> = None;
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
        let mut best: Option<((bool, i64, Reverse<Pos>), Pos)> = None;
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

#[cfg(test)]
mod tests {
    use super::super::GrandStrategy;
    use super::*;
    use crate::doctrine::{build, position};

    /// `the_storming`'s board with its army removed: a 200-hit-point city
    /// of player 1 behind 100 points of wall, and nothing else.
    fn walled_city() -> (Game, u32) {
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

    fn plan_against(g: &Game, cid: u32) -> StrategicPlan {
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
    fn ring_of(g: &Game, cid: u32) -> Vec<Pos> {
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
    fn at_distance(g: &Game, cid: u32, distance: i32) -> Vec<Pos> {
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
    fn step_unit(ai: &mut AdvancedAi, g: &mut Game, pid: usize, uid: u32, plan: &StrategicPlan) {
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
