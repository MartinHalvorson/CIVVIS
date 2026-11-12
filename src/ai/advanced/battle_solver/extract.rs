//! From the board to a battle: which fights exist, and what each one is.

use std::collections::BTreeMap;

use super::super::{AdvancedAi, StrategicPlan};
use super::model::{
    Board, CitySpec, Objective, State, Unit, F_CAPTURES, F_IGNORES_ZOC, F_MELEE, F_MOVE_AND_FIRE,
    F_RANGED, F_SIEGE, F_ZOC, IMPASSABLE, MP, NO_CITY, NO_TILE, OURS, THEIRS,
};
use crate::game::Game;
use crate::Pos;

/// A hostile this close to one of ours belongs to the same battle.
pub(super) const ENGAGE_RADIUS: i32 = 5;
/// Tiles of ground kept around each unit of the battle.
const GROUND_RADIUS: i32 = 6;
const MAX_TILES: usize = 420;
pub(super) const MAX_OURS: usize = 20;
pub(super) const MAX_HOSTILES: usize = 24;
/// How close a city must stand to a fight to be the fight's objective.
const OBJECTIVE_RADIUS: i32 = 6;
/// A city's worth to the score, in hammers.
const CITY_BASE_VALUE: f32 = 220.0;
const CITY_POP_VALUE: f32 = 45.0;

/// One local fight.
#[derive(Clone, Debug)]
pub(super) struct Theater {
    /// Our units the solver may order.
    pub ours: Vec<u32>,
    pub hostiles: Vec<u32>,
}

/// A battle reduced to the model's numbers, with the way back to game ids.
pub(super) struct Snapshot {
    pub board: Board,
    pub state: State,
    pub objective: Objective,
    pub unit_ids: Vec<u32>,
    pub tiles: Vec<Pos>,
    pub city_ids: Vec<u32>,
}

/// Land soldiers only: the model plays no ships or aircraft.
fn fights_on_land(g: &Game, uid: u32) -> bool {
    let unit = &g.units[&uid];
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && !g.is_embarked(unit)
        && (spec.is_melee_capable() || spec.has_ranged_attack())
}

/// The fights on the board: units of ours within reach of visible hostiles,
/// joined where they share a hostile.
pub(super) fn theaters(ai: &AdvancedAi, g: &Game, pid: usize) -> Vec<Theater> {
    let mut mine = Vec::new();
    for uid in g.player_unit_ids(pid) {
        let unit = &g.units[&uid];
        if fights_on_land(g, uid)
            && unit.linked_to.is_none()
            && !ai.solver_leaves_alone(g, pid, uid)
        {
            mine.push(uid);
        }
    }
    let mut hostile = Vec::new();
    for unit in g.units.values() {
        if unit.owner != pid
            && g.is_at_war(pid, unit.owner)
            && fights_on_land(g, unit.id)
            && g.unit_visible_to(unit.id, pid)
        {
            hostile.push(unit.id);
        }
    }
    hostile.sort_unstable();
    let total = mine.len() + hostile.len();
    let mut parent: Vec<usize> = (0..total).collect();
    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut root = x;
        while parent[root] != root {
            root = parent[root];
        }
        let mut cur = x;
        while parent[cur] != root {
            let next = parent[cur];
            parent[cur] = root;
            cur = next;
        }
        root
    }
    let mut linked = vec![false; total];
    for (m, uid) in mine.iter().enumerate() {
        let here = g.units[uid].pos;
        for (h, hid) in hostile.iter().enumerate() {
            if g.wdist(here, g.units[hid].pos) <= ENGAGE_RADIUS {
                linked[m] = true;
                linked[mine.len() + h] = true;
                let (a, b) = (find(&mut parent, m), find(&mut parent, mine.len() + h));
                parent[a] = b;
            }
        }
    }
    let mut groups: BTreeMap<usize, Theater> = BTreeMap::new();
    for i in 0..total {
        if !linked[i] {
            continue;
        }
        let root = find(&mut parent, i);
        let group = groups.entry(root).or_insert_with(|| Theater {
            ours: Vec::new(),
            hostiles: Vec::new(),
        });
        if i < mine.len() {
            group.ours.push(mine[i]);
        } else {
            group.hostiles.push(hostile[i - mine.len()]);
        }
    }
    groups
        .into_values()
        .filter(|t| !t.ours.is_empty() && !t.hostiles.is_empty())
        .collect()
}

fn class_of(classes: &mut Vec<(crate::name::Name, u32)>, g: &Game, uid: u32) -> u8 {
    let kind = g.units[&uid].kind;
    if let Some(found) = classes.iter().position(|(k, _)| *k == kind) {
        return found as u8;
    }
    classes.push((kind, uid));
    (classes.len() - 1) as u8
}

/// Read one theater off the board. `None` when there is no ground to model.
pub(super) fn snapshot(
    ai: &AdvancedAi,
    g: &Game,
    pid: usize,
    theater: &Theater,
    plan: &StrategicPlan,
) -> Option<Snapshot> {
    // Whoever else stands on this ground fights in the rollouts too; only
    // `theater.ours` take orders this turn.
    let mut members: Vec<u32> = theater.ours.clone();
    members.truncate(MAX_OURS);
    let mut hostiles = theater.hostiles.clone();
    if hostiles.len() > MAX_HOSTILES {
        let anchor = g.units.get(members.first()?)?.pos;
        hostiles.sort_by_key(|h| g.wdist(anchor, g.units[h].pos));
        hostiles.truncate(MAX_HOSTILES);
    }
    let mut ground: Vec<Pos> = Vec::new();
    for uid in members.iter().chain(hostiles.iter()) {
        ground.extend(g.wdisk(g.units[uid].pos, GROUND_RADIUS));
    }
    ground.sort_unstable();
    ground.dedup();
    if ground.len() > MAX_TILES {
        // Keep the tiles closest to any member.
        let anchors: Vec<Pos> = members
            .iter()
            .chain(hostiles.iter())
            .map(|uid| g.units[uid].pos)
            .collect();
        ground.sort_by_key(|p| anchors.iter().map(|a| g.wdist(*p, *a)).min().unwrap_or(99));
        ground.truncate(MAX_TILES);
        ground.sort_unstable();
    }
    let index: BTreeMap<Pos, u16> = ground
        .iter()
        .enumerate()
        .map(|(k, p)| (*p, k as u16))
        .collect();
    let tiles = ground.len();

    // Everyone on this ground.
    let mut unit_ids: Vec<u32> = members.clone();
    let others: Vec<u32> = g
        .units
        .values()
        .filter(|u| {
            !members.contains(&u.id)
                && index.contains_key(&u.pos)
                && fights_on_land(g, u.id)
                && (u.owner == pid
                    || (g.is_at_war(pid, u.owner)
                        && (hostiles.contains(&u.id) || g.unit_visible_to(u.id, pid))))
        })
        .map(|u| u.id)
        .collect();
    unit_ids.extend(others);
    let n = unit_ids.len();

    let mut classes: Vec<(crate::name::Name, u32)> = Vec::new();
    let mut units = Vec::with_capacity(n);
    for (i, uid) in unit_ids.iter().enumerate() {
        let u = &g.units[uid];
        let spec = &g.rules.units[u.kind];
        let side = if u.owner == pid { OURS } else { THEIRS };
        let mut flags = 0u8;
        if g.exerts_zoc(u) {
            flags |= F_ZOC;
        }
        if g.unit_ignores_zoc(*uid) {
            flags |= F_IGNORES_ZOC;
        }
        if spec.siege {
            flags |= F_SIEGE;
        } else if spec.has_ranged_attack() {
            flags |= F_RANGED;
        } else if spec.is_melee_capable() {
            flags |= F_MELEE | F_CAPTURES;
        }
        if g.siege_may_attack_after_moving(u) {
            flags |= F_MOVE_AND_FIRE;
        }
        let actor = i < members.len();
        units.push(Unit {
            tile: index[&u.pos],
            hp: u.hp as i16,
            mp: if actor {
                (u.moves_left * MP as f64).round() as u16
            } else {
                0
            },
            max_mp: (g.unit_max_moves(*uid) * MP as f64).round() as u16,
            value: spec.cost as f32,
            side,
            attacks: if actor {
                u.attacks_left.clamp(0, 5) as u8
            } else {
                0
            },
            max_attacks: g.battle_max_attacks(*uid).clamp(1, 5) as u8,
            fort: u.fortify_turns.clamp(0, 2) as u8,
            flags,
            range: if spec.has_ranged_attack() {
                g.unit_attack_range(*uid).clamp(1, 8) as u8
            } else {
                1
            },
            class: class_of(&mut classes, g, *uid),
            acted: u.acted,
        });
    }
    let classes_n = classes.len().max(1);

    // Ground: neighbours, rivers, footing, ownership of the tile.
    let mut nbrs = vec![[NO_TILE; 6]; tiles];
    let mut river = vec![0u8; tiles];
    let mut defense = vec![0.0f32; tiles];
    let mut land = vec![false; tiles];
    let mut blocked = vec![false; tiles];
    for (t, pos) in ground.iter().enumerate() {
        for (d, n) in g.nbrs(*pos).into_iter().enumerate().take(6) {
            if let Some(k) = index.get(&n) {
                nbrs[t][d] = *k;
                if g.battle_crosses_river(*pos, n) {
                    river[t] |= 1 << d;
                }
            }
        }
        defense[t] = g.tile_defense_bonus(*pos) as f32;
        land[t] = g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile));
        blocked[t] = g.unit_ids_at(*pos).iter().any(|id| {
            let other = &g.units[id];
            other.owner != pid
                && !g.is_at_war(pid, other.owner)
                && g.rules.units[other.kind].class == "military"
        });
    }
    let mut edge = vec![IMPASSABLE; classes_n * tiles * 6];
    for (c, (_, rep)) in classes.iter().enumerate() {
        for (t, pos) in ground.iter().enumerate() {
            for d in 0..6 {
                let k = nbrs[t][d];
                if k == NO_TILE || blocked[k as usize] {
                    continue;
                }
                if let Some(cost) = g.battle_step_cost(*rep, *pos, ground[k as usize]) {
                    edge[(c * tiles + t) * 6 + d] = (cost * MP as f64).round().max(1.0) as u16;
                }
            }
        }
    }
    let mut dist = vec![0u8; tiles * tiles];
    for a in 0..tiles {
        for b in 0..tiles {
            dist[a * tiles + b] = g.wdist(ground[a], ground[b]).min(250) as u8;
        }
    }

    // Cities on the ground: ours and those we are at war with.
    let mut city_at = vec![NO_CITY; tiles];
    let mut cities = Vec::new();
    let mut city_ids = Vec::new();
    let mut city_state = Vec::new();
    for (cid, city) in g.cities.iter() {
        let Some(tile) = index.get(&city.pos) else {
            continue;
        };
        let side = if city.owner == pid {
            OURS
        } else if g.is_at_war(pid, city.owner) {
            THEIRS
        } else {
            continue;
        };
        let c = cities.len() as u8;
        city_at[*tile as usize] = c;
        let wound = (10.0 - city.hp.clamp(0, 200) as f64 / 20.0).round();
        cities.push(CitySpec {
            tile: *tile,
            strength: (g.city_strength(*cid) + wound) as f32,
            wall_max: g.city_max_wall_hp(city) as i16,
            strike: if city.wall_hp > 0 {
                g.city_ranged_strength(*cid) as f32
            } else {
                0.0
            },
            value: CITY_BASE_VALUE + CITY_POP_VALUE * city.pop as f32,
            zoc: true,
        });
        city_ids.push(*cid);
        city_state.push((city.hp as i16, city.wall_hp as i16, side));
    }

    // Heal rates by ground and side, from a representative soldier of each.
    let rep_ours = unit_ids.iter().find(|id| g.units[id].owner == pid).copied();
    let rep_theirs = unit_ids.iter().find(|id| g.units[id].owner != pid).copied();
    let mut heal = vec![[0u8; 2]; tiles];
    for (t, pos) in ground.iter().enumerate() {
        if let Some(uid) = rep_ours {
            heal[t][OURS as usize] = g.unit_heal_rate_at(uid, *pos).clamp(0, 100) as u8;
        }
        if let Some(uid) = rep_theirs {
            heal[t][THEIRS as usize] = g.unit_heal_rate_at(uid, *pos).clamp(0, 100) as u8;
        }
    }

    // Strength tables.
    let city_n = cities.len();
    let mut melee_att = vec![0.0f32; n * n];
    let mut melee_def = vec![0.0f32; n * n];
    let mut ranged_att = vec![0.0f32; n * n];
    let mut ranged_def = vec![0.0f32; n * n];
    let mut city_melee = vec![0.0f32; n * city_n];
    let mut city_ranged = vec![0.0f32; n * city_n];
    for (i, a) in unit_ids.iter().enumerate() {
        for (j, d) in unit_ids.iter().enumerate() {
            if i == j {
                // The diagonal holds a unit's plain defence, for city strikes.
                let u = &g.units[d];
                melee_def[i * n + j] =
                    (g.unit_strength(u, true) - 3.0 * u.fortify_turns.clamp(0, 2) as f64) as f32;
                continue;
            }
            if let Some((att, def)) = g.melee_pair_bases(*a, *d) {
                melee_att[i * n + j] = att as f32;
                melee_def[i * n + j] = def as f32;
            }
            if units[i].flags & (F_RANGED | F_SIEGE) != 0 {
                if let Some((att, def)) = g.ranged_pair_bases(*a, *d) {
                    ranged_att[i * n + j] = att as f32;
                    ranged_def[i * n + j] = def as f32;
                }
            }
        }
        for (c, cid) in city_ids.iter().enumerate() {
            if let Some((melee, ranged)) = g.city_pair_bases(*a, *cid) {
                city_melee[i * city_n + c] = melee as f32;
                city_ranged[i * city_n + c] = ranged as f32;
            }
        }
    }

    // First-turn reach and line of sight, exactly as the engine reads them.
    let mut reach0 = vec![Vec::new(); n];
    let mut los0 = Vec::new();
    for (i, uid) in members.iter().enumerate() {
        let u = &g.units[uid];
        let mut reach = vec![(index[&u.pos], (u.moves_left * MP as f64).round() as u16)];
        if u.moves_left > 0.0 {
            for (pos, kept) in g.approach_arrivals(*uid) {
                if let Some(t) = index.get(&pos) {
                    reach.push((*t, (kept * MP as f64).round() as u16));
                }
            }
        }
        if units[i].flags & (F_RANGED | F_SIEGE) != 0 {
            let range = units[i].range;
            for (t, _) in &reach {
                for j in 0..n {
                    if units[j].side == units[i].side {
                        continue;
                    }
                    let to = units[j].tile;
                    if dist[*t as usize * tiles + to as usize] <= range
                        && g.battle_line_of_sight(ground[*t as usize], ground[to as usize])
                    {
                        los0.push((*t, to));
                    }
                }
                for city in &cities {
                    if dist[*t as usize * tiles + city.tile as usize] <= range
                        && g.battle_line_of_sight(ground[*t as usize], ground[city.tile as usize])
                    {
                        los0.push((*t, city.tile));
                    }
                }
            }
        }
        reach0[i] = reach;
    }
    los0.sort_unstable();
    los0.dedup();

    let board = Board {
        tiles,
        nbrs,
        river,
        defense,
        land,
        dist,
        edge,
        city_at,
        cities,
        heal,
        flank: [
            if g.battle_flanks_unlocked(pid) {
                2.0
            } else {
                0.0
            },
            rep_theirs.map_or(0.0, |uid| {
                if g.battle_flanks_unlocked(g.units[&uid].owner) {
                    2.0
                } else {
                    0.0
                }
            }),
        ],
        support: [
            if g.battle_flanks_unlocked(pid) {
                2.0
            } else {
                0.0
            },
            rep_theirs.map_or(0.0, |uid| {
                if g.battle_flanks_unlocked(g.units[&uid].owner) {
                    2.0
                } else {
                    0.0
                }
            }),
        ],
        n,
        melee_att,
        melee_def,
        ranged_att,
        ranged_def,
        city_melee,
        city_ranged,
        reach0,
        los0,
        has_los: true,
        march_on_objective: false,
        melee_breach_share: 0.0,
    };
    let state = State::new(&board, units, city_state);
    let objective = choose_objective(g, pid, &board, &state, &city_ids, plan);
    let _ = ai;
    Some(Snapshot {
        board,
        state,
        objective,
        unit_ids,
        tiles: ground,
        city_ids,
    })
}

/// What this fight is for. A city of ours under the enemy's reach is kept
/// first; the plan's target, or any enemy city we stand next to, is taken;
/// otherwise the fight is about the units in it.
fn choose_objective(
    g: &Game,
    pid: usize,
    board: &Board,
    state: &State,
    city_ids: &[u32],
    plan: &StrategicPlan,
) -> Objective {
    let near = |city: usize, side: u8, radius: i32| {
        state.units.iter().any(|u| {
            u.alive()
                && u.side == side
                && (board.d(u.tile, board.cities[city].tile) as i32) <= radius
        })
    };
    // Keep what is ours: nearest our city with a hostile within two.
    for (c, cid) in city_ids.iter().enumerate() {
        if g.cities[cid].owner == pid && near(c, THEIRS, 2) {
            return Objective::Hold(c as u8);
        }
    }
    // Take the plan's city if we are on it.
    if let Some(target) = plan.target_city {
        if let Some(c) = city_ids.iter().position(|cid| *cid == target) {
            if g.cities[&target].owner != pid && near(c, OURS, OBJECTIVE_RADIUS) {
                return Objective::Capture(c as u8);
            }
        }
    }
    for (c, cid) in city_ids.iter().enumerate() {
        if g.cities[cid].owner != pid && near(c, OURS, OBJECTIVE_RADIUS) {
            return Objective::Capture(c as u8);
        }
    }
    Objective::Field
}
