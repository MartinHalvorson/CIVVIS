//! `amphibious-siege-train`: a Domination siege train whose objective has no
//! dry road from our land crosses the water as one group: guns and an escort
//! gather at a coastal city of ours, embark together, sail a lane no visible
//! enemy warship watches, and come ashore together beside the target. A gun
//! never lands alone.
//!
//! **Why (2026-10-11/12 census, five live gate passers with logs).** When a
//! capital was the siege objective after turn 150, the top hold in two of the
//! five was water: civvis-20261010T190448Z (the Emperor win) logged 133 and
//! civvis-20261011T004440Z 342 "the bombard holds on land short of the water
//! | the march crosses water in N steps and come-ashore lands an embarked unit
//! at home; no dry road opens within 128 steps". `stage-march-keeps-to-land`
//! holds such a gun on land because a single gun that embarks is landed at
//! home again the next turn by `come-ashore` (an embarked unit has no siege
//! arm, so the siege doctrine passes it to the force mover, whose first act
//! is to bring it ashore). Across all passers, 51% of at-war capital samples
//! after t150 had no land unit of ours within six tiles, and the bombers
//! strike only a city the land army besieges.
//!
//! **Design.** The pieces reused:
//! - the trigger is the existing water hold: `stage_march` returning
//!   `StageMarch::Hold` (no dry road within `STAGE_DRY_LIMIT`) in the
//!   Stage march (`siege_stage_step`) or the Invest march
//!   (`close_to_staging`), and only for a pure water hold, never one a
//!   closed border causes (`siege-buys-the-passage` and
//!   `blocker-becomes-the-target` own those);
//! - the members are the land force group whose objective is the target:
//!   its siege guns and its melee or anti-cavalry bodies;
//! - the host's embarkation is the order layer's ordinary step: a land unit
//!   with the embark technology steps onto a Coast tile
//!   (`Game::has_embarkation`: Shipbuilding for every land unit in Gathering
//!   Storm, Ocean tiles behind Cartography's `ocean_navigation`), so the lane
//!   is read with [`Game::unit_can_traverse`] for a gun of the train and the
//!   sail is `Game::route_step_to_any` toward the water beside the landing;
//! - landing is a step from the water onto a free land tile beside the
//!   landing tile (`tactical_apply_move`), escorts first.
//!
//! The train owns its members from the first act of the unit step
//! (`advanced_military_step_with_decline`), ahead of the doctrine and the
//! force mover, so `come-ashore` never lands them at home mid-crossing; once
//! every member stands on the far shore the train releases them and the
//! ordinary siege march, which now finds a dry road, takes over. One train at
//! a time. The train abandons the crossing (and `come-ashore` brings any
//! embarked member home) when a visible enemy warship comes within
//! [`AMPHIB_LANE_SAFETY`] of the lane, the target changes hands or the war
//! ends, too few guns or escorts survive, or [`AMPHIB_PATIENCE`] turns pass.
//!
//! The landing tile lies within [`AMPHIB_LANDING_MAX`] of the target, the
//! nearest outside the city's own strike range preferred (three tiles, then
//! two, then one): a train that lands at two or one comes ashore under the
//! city's ranged attack. Off, nothing runs and no order changes.
//!
//! **Version two (`amphibious-siege-train-2`, a superset).** The first live
//! crossing (civvis-20261013T083300Z, Susa) took 22 turns from forming to
//! landing over a 12-water-tile lane, though the fewest-water road from the
//! port crossed ONE water tile: members on the mixed router hopped between
//! land and sea (each switch ends the turn), a gun was combined into an Army
//! at sea, and an escort was re-tasked to a builder guard and upgraded (a
//! new id), while the cohesion hold waited on it. Version two:
//! - **fords** a strait: when the fewest-water road from a gun of the group
//!   to the target's staging ring, on the target's own land (an islet in the
//!   ring stages nothing), crosses one stretch of at most
//!   [`AMPHIB_FORD_WATER_MAX`] water tiles, the train gathers on the near
//!   shore of that stretch (not at a port), crosses it, and lands on the far
//!   shore; the ordinary siege march takes over from there. Members cross as
//!   the water allows (one land unit a tile), so there is no cohesion wait
//!   and an escort lands at once; a gun still lands only beside an escort
//!   ashore. Each gun of the group is tried, and a ford that cannot form (no
//!   escorts within reach of its shore, a warship on it) falls back to the
//!   sea lane, so version two ships whatever version one would;
//! - **locks** its members: none is combined into a Corps or Army, upgraded,
//!   pooled by the objective board or bound as a civilian's guard while the
//!   train holds it ([`AdvancedAi::amphibious_locked`]);
//! - **drops a laggard** from the waits: on a turn no member closes on the
//!   gathering point (or the landing), the furthest member not yet in place
//!   counts a stalled turn; after [`AMPHIB_STALL_TURNS`] it no longer holds
//!   the others back. One that never gathered stays behind when the train
//!   sails; one at sea is released with the train;
//! - **sails on water only**: an embarked member steps along sailable water
//!   toward the beach (`water_step`), never ashore mid-crossing;
//! - **widens the beach**: the train counts as arrived within
//!   [`AMPHIB_ARRIVED_2`] of the landing and lands on any free tile within
//!   [`AMPHIB_BEACH_2`] of it joined to it over land (version one's two-tile
//!   test counted a gun on an islet two tiles out as ashore); a member
//!   ashore by the water steps inland to free the water's edge;
//! - **watches a ford closely, not widely**: a ford is held or abandoned only
//!   for a visible enemy warship beside a ford tile or able to reach one this
//!   turn over water ([`ford_watched`]); the sea lane keeps the three-tile
//!   watch;
//! - **gathers only the near shore**: a ford train takes only units joined
//!   over land to its near shore (live 204616Z forded from an islet with the
//!   column already on the target's side, which could never gather, and the
//!   train held it eleven turns); and it journals the gathering each turn
//!   ("Siege train gathers for X: N of M within 2 of the ford; waiting on
//!   …");
//! - **sails from where the dry road ends**: a sea lane never starts at a
//!   port on the objective's own landmass (a landing there would be on our
//!   own land, a dry road away), and drafts only the units on land joined to
//!   its port, so a unit with a dry road keeps marching overland (a replay
//!   of 204616Z shipped from Cuenca to land on the same mainland).

use super::*;

/// The longest sea lane, in water tiles from the port's shore to the water
/// beside the landing, the train will sail.
pub(super) const AMPHIB_SEA_LANE_MAX: usize = 12;

/// No visible warship of a power at war with us within this many tiles of
/// any tile of the lane.
pub(super) const AMPHIB_LANE_SAFETY: i32 = 3;

/// The landing tile lies within this many tiles of the target.
pub(super) const AMPHIB_LANDING_MAX: i32 = 3;

/// Members within this many tiles of the port are gathered.
pub(super) const AMPHIB_ASSEMBLY_RADIUS: i32 = 2;

/// Members are drawn from the objective's force group within this many tiles
/// of the port.
pub(super) const AMPHIB_MUSTER_RADIUS: i32 = 10;

/// Escort bodies (melee or anti-cavalry) the crossing needs.
pub(super) const AMPHIB_MIN_ESCORTS: usize = 2;

/// The most guns and the most escorts one train carries.
pub(super) const AMPHIB_MAX_GUNS: usize = 3;
pub(super) const AMPHIB_MAX_ESCORTS: usize = 3;

/// Live turns from forming after which an unfinished crossing is abandoned.
pub(super) const AMPHIB_PATIENCE: u32 = 24;

/// A member at sea this many tiles nearer the landing than the furthest
/// member waits for it.
pub(super) const AMPHIB_COHESION: i32 = 2;

/// Version two: the most water tiles a ford crosses.
pub(super) const AMPHIB_FORD_WATER_MAX: usize = 2;

/// Version two: the longest road, in steps, the ford search walks.
pub(super) const AMPHIB_FORD_ROUTE_MAX: usize = 128;

/// Version two: turns without closing after which a member stops holding
/// the others back.
pub(super) const AMPHIB_STALL_TURNS: u32 = 2;

/// Version two: the train counts as arrived when every waited-on member is
/// within this many tiles of the landing.
pub(super) const AMPHIB_ARRIVED_2: i32 = 3;

/// Version two: the beach is every free land tile within this many tiles of
/// the landing.
pub(super) const AMPHIB_BEACH_2: i32 = 2;

/// Where the crossing stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AmphibiousPhase {
    /// Members walk to the port.
    Assemble,
    /// Members embark, sail and land.
    Sail,
}

/// One crossing: see the module documentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AmphibiousTrain {
    pub(super) target: u32,
    pub(super) target_pos: Pos,
    pub(super) port: u32,
    pub(super) port_pos: Pos,
    pub(super) landing: Pos,
    /// The planned lane's water tiles, port shore to landing water.
    pub(super) lane: Vec<Pos>,
    pub(super) members: BTreeSet<u32>,
    pub(super) phase: AmphibiousPhase,
    pub(super) formed: u32,
    /// Version two: a ford over a strait; `port_pos` is the near shore (a
    /// land tile, no city) and `port` only names the nearest city of ours.
    pub(super) ford: bool,
    /// Version two: each member's best distance to its goal this phase and
    /// the turns since it last closed.
    pub(super) progress: BTreeMap<u32, (i32, u32)>,
    /// The turn `progress` was last read.
    pub(super) progress_turn: u32,
}

/// A siege gun of the train, read from its type (an embarked unit has no
/// siege arm).
fn is_gun(g: &Game, uid: u32) -> bool {
    g.units.get(&uid).is_some_and(|unit| {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military" && spec.siege && spec.has_ranged_attack()
    })
}

/// An escort body of the train: a land melee-capable soldier that is not a
/// gun.
fn is_escort(g: &Game, uid: u32) -> bool {
    g.units.get(&uid).is_some_and(|unit| {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && !spec.siege
            && spec.is_melee_capable()
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
    })
}

/// A land tile a unit may stand on: passable, not water, no city.
fn landable(g: &Game, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.city_at(pos).is_none()
}

/// A water tile `rep` can sail.
fn sailable(g: &Game, rep: u32, pos: Pos) -> bool {
    g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile)) && g.unit_can_traverse(rep, pos)
}

/// The water tiles beside `pos` that `rep` can sail.
fn shore_water(g: &Game, rep: u32, pos: Pos) -> Vec<Pos> {
    g.nbrs(pos)
        .into_iter()
        .filter(|n| sailable(g, rep, *n))
        .collect()
}

/// The shortest sea lane, over water `rep` can sail and no foreign unit
/// holds, from the water beside `from` to the water beside `to`: its tiles,
/// no longer than [`AMPHIB_SEA_LANE_MAX`].
pub(super) fn sea_lane(g: &Game, pid: usize, rep: u32, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    let goals: BTreeSet<Pos> = shore_water(g, rep, to).into_iter().collect();
    if goals.is_empty() {
        return None;
    }
    let open = |pos: Pos| {
        sailable(g, rep, pos)
            && g.unit_ids_at(pos)
                .iter()
                .all(|id| g.units.get(id).is_none_or(|unit| unit.owner == pid))
    };
    let mut parent: BTreeMap<Pos, Option<Pos>> = BTreeMap::new();
    let mut frontier: std::collections::VecDeque<(Pos, usize)> = std::collections::VecDeque::new();
    for start in shore_water(g, rep, from) {
        if open(start) && !parent.contains_key(&start) {
            parent.insert(start, None);
            frontier.push_back((start, 1));
        }
    }
    while let Some((pos, len)) = frontier.pop_front() {
        if goals.contains(&pos) {
            let mut lane = vec![pos];
            let mut cursor = pos;
            while let Some(Some(prev)) = parent.get(&cursor) {
                lane.push(*prev);
                cursor = *prev;
            }
            lane.reverse();
            return Some(lane);
        }
        if len >= AMPHIB_SEA_LANE_MAX {
            continue;
        }
        for next in g.nbrs(pos) {
            if !parent.contains_key(&next) && open(next) {
                parent.insert(next, Some(pos));
                frontier.push_back((next, len + 1));
            }
        }
    }
    None
}

/// Whether a visible warship of a power at war with `pid` stands within
/// [`AMPHIB_LANE_SAFETY`] of any tile of `lane`.
pub(super) fn lane_watched(g: &Game, pid: usize, lane: &[Pos]) -> bool {
    g.units.values().any(|unit| {
        unit.owner != pid
            && g.is_at_war(pid, unit.owner)
            && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            && g.unit_visible_to(unit.id, pid)
            && lane
                .iter()
                .any(|tile| g.wdist(*tile, unit.pos) <= AMPHIB_LANE_SAFETY)
    })
}

/// Version two, a ford of at most [`AMPHIB_FORD_WATER_MAX`] water tiles: a
/// visible warship of a power at war with `pid` beside a ford tile, or able
/// to reach one this turn over water it can sail. A ford is crossed in about
/// a turn, so only a warship that can strike it this turn holds it; the sea
/// lane keeps its three-tile watch ([`lane_watched`]). (Live Immortal
/// 204616Z abandoned one-tile fords on t129 and t132 for a Battleship three
/// tiles off on water joined to the ford: within its five moves, so it still
/// holds them.)
pub(super) fn ford_watched(g: &Game, pid: usize, ford: &[Pos]) -> bool {
    g.units.values().any(|unit| {
        unit.owner != pid
            && g.is_at_war(pid, unit.owner)
            && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            && g.unit_visible_to(unit.id, pid)
            && (ford.iter().any(|tile| g.wdist(*tile, unit.pos) <= 1)
                || warship_reaches(g, unit, ford))
    })
}

/// Whether `unit` reaches a tile of `ford` within its moves over water it
/// can sail, one move a tile.
fn warship_reaches(g: &Game, unit: &crate::game::Unit, ford: &[Pos]) -> bool {
    let moves = (g.rules.units[unit.kind].moves.floor() as i32).max(1);
    if ford.iter().all(|tile| g.wdist(*tile, unit.pos) > moves) {
        return false;
    }
    let goals: BTreeSet<Pos> = ford.iter().copied().collect();
    let mut seen: BTreeSet<Pos> = BTreeSet::from([unit.pos]);
    let mut frontier = vec![unit.pos];
    for _ in 0..moves {
        let mut next = Vec::new();
        for pos in frontier {
            for step in g.nbrs(pos) {
                let water = g.map.get(step).is_some_and(|tile| g.rules.is_water(tile));
                if water && g.unit_can_traverse(unit.id, step) && seen.insert(step) {
                    if goals.contains(&step) {
                        return true;
                    }
                    next.push(step);
                }
            }
        }
        frontier = next;
    }
    false
}

/// Whether the crossing in hand is watched: a ford by [`ford_watched`], a
/// sea lane by [`lane_watched`].
fn train_watched(g: &Game, pid: usize, train: &AmphibiousTrain) -> bool {
    if train.ford && train.lane.len() <= AMPHIB_FORD_WATER_MAX {
        ford_watched(g, pid, &train.lane)
    } else {
        lane_watched(g, pid, &train.lane)
    }
}

/// Version two: the units of `ashore` that can gather at a ford's near
/// `shore`: those on land joined to it over land within reach of the muster.
/// Live Immortal 204616Z forded from an islet of ours (t117, Aquileia) and
/// took the nearest guns and escorts by straight distance: the column on the
/// target's own side of the strait, which had a dry road and could never walk
/// to the islet's shore. The gathering held it eleven turns (t118-t128): a
/// Trebuchet sat on the landing tile counted as gathered (two tiles from the
/// shore, across the water), a second paced between the two tiles three out,
/// and a Horseman stalled behind it.
fn near_shore_members(g: &Game, shore: Pos, ashore: &[u32]) -> Vec<u32> {
    let land = land_joined(
        g,
        shore,
        Some(AMPHIB_MUSTER_RADIUS + AMPHIB_ASSEMBLY_RADIUS),
    );
    ashore
        .iter()
        .copied()
        .filter(|id| g.units.get(id).is_some_and(|unit| land.contains(&unit.pos)))
        .collect()
}

/// `from` and the land tiles joined to it over passable land, out to
/// `radius` tiles from it (`None`: its whole landmass).
fn land_joined(g: &Game, from: Pos, radius: Option<i32>) -> BTreeSet<Pos> {
    let walkable = |pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
    };
    let mut land: BTreeSet<Pos> = BTreeSet::from([from]);
    let mut frontier = vec![from];
    while let Some(pos) = frontier.pop() {
        for next in g.nbrs(pos) {
            if radius.is_none_or(|radius| g.wdist(next, from) <= radius)
                && walkable(next)
                && land.insert(next)
            {
                frontier.push(next);
            }
        }
    }
    land
}

/// The landing tiles for `target`, best first: land beside water `rep` can
/// sail, within [`AMPHIB_LANDING_MAX`] of the target, no foreign unit on it;
/// three tiles out (outside the city's strike) before two before one.
pub(super) fn landing_tiles(g: &Game, pid: usize, rep: u32, target: Pos) -> Vec<Pos> {
    let preference = |distance: i32| match distance {
        3 => 0,
        2 => 1,
        _ => 2,
    };
    let mut tiles: Vec<(i32, Pos)> = g
        .wdisk(target, AMPHIB_LANDING_MAX)
        .into_iter()
        .filter(|pos| {
            *pos != target
                && landable(g, *pos)
                && !shore_water(g, rep, *pos).is_empty()
                && g.unit_ids_at(*pos)
                    .iter()
                    .all(|id| g.units.get(id).is_none_or(|unit| unit.owner == pid))
        })
        .map(|pos| (preference(g.wdist(pos, target)), pos))
        .collect();
    tiles.sort_unstable();
    tiles.into_iter().map(|(_, pos)| pos).collect()
}

/// Whether the player can embark land units at all.
fn can_embark(g: &Game, pid: usize) -> bool {
    g.has_ability(pid, "mana") || g.tree_effect(pid, "land_unit_embark") > 0.0
}

/// Whether a unit of another player stands on `pos`.
fn foreign_at(g: &Game, pid: usize, pos: Pos) -> bool {
    g.unit_ids_at(pos)
        .iter()
        .any(|id| g.units.get(id).is_some_and(|unit| unit.owner != pid))
}

/// Version two: the fewest-water road for `rep` from `start` to a free land
/// tile within `range` of `target` on the target's own land (fewest water
/// tiles first, then fewest steps), as `(shore, water, landing)` when that road crosses ONE stretch of
/// one to [`AMPHIB_FORD_WATER_MAX`] water tiles: the near-shore land tile,
/// the stretch's water tiles in order, and the far-shore land tile. `None`
/// when the best road is dry, crosses more water or two stretches, or runs
/// past [`AMPHIB_FORD_ROUTE_MAX`] steps. Land tiles a foreign unit holds and
/// cities not ours are not walked.
pub(super) fn ford_crossing(
    g: &Game,
    pid: usize,
    rep: u32,
    start: Pos,
    target: Pos,
    range: i32,
) -> Option<(Pos, Vec<Pos>, Pos)> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let is_water = |pos: Pos| g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile));
    let passable = g.road_plan_passable(rep);
    let open = |pos: Pos| passable(pos) && !foreign_at(g, pid, pos);
    // The far shore must be the target's own land, joined to the city over
    // land: an islet within the staging ring stages nothing.
    let mut mainland: BTreeSet<Pos> = BTreeSet::new();
    let mut frontier: Vec<Pos> = g
        .nbrs(target)
        .into_iter()
        .filter(|pos| landable(g, *pos))
        .collect();
    mainland.extend(frontier.iter().copied());
    while let Some(pos) = frontier.pop() {
        for next in g.nbrs(pos) {
            if g.wdist(next, target) <= range + AMPHIB_ASSEMBLY_RADIUS
                && landable(g, next)
                && mainland.insert(next)
            {
                frontier.push(next);
            }
        }
    }
    let goal = |pos: Pos| g.wdist(pos, target) <= range && mainland.contains(&pos) && open(pos);
    let mut best: BTreeMap<Pos, (usize, usize)> = BTreeMap::new();
    let mut parent: BTreeMap<Pos, Pos> = BTreeMap::new();
    let mut heap = BinaryHeap::new();
    best.insert(start, (0, 0));
    heap.push(Reverse((0usize, 0usize, start)));
    let mut found = None;
    while let Some(Reverse((water, steps, pos))) = heap.pop() {
        if best.get(&pos).is_some_and(|known| (water, steps) > *known) {
            continue;
        }
        if goal(pos) {
            found = Some(pos);
            break;
        }
        if steps >= AMPHIB_FORD_ROUTE_MAX {
            continue;
        }
        for next in g.nbrs(pos) {
            if !open(next) {
                continue;
            }
            let key = (water + usize::from(is_water(next)), steps + 1);
            if key.0 > AMPHIB_FORD_WATER_MAX {
                continue;
            }
            if best.get(&next).is_none_or(|known| key < *known) {
                best.insert(next, key);
                parent.insert(next, pos);
                heap.push(Reverse((key.0, key.1, next)));
            }
        }
    }
    let end = found?;
    let mut path = vec![end];
    let mut cursor = end;
    while let Some(previous) = parent.get(&cursor) {
        path.push(*previous);
        cursor = *previous;
    }
    path.reverse();
    let wet: Vec<bool> = path.iter().map(|pos| is_water(*pos)).collect();
    let first = wet.iter().position(|w| *w)?;
    let last = wet.iter().rposition(|w| *w)?;
    if first == 0
        || last + 1 >= path.len()
        || wet[first..=last].iter().any(|w| !*w)
        || !landable(g, path[last + 1])
    {
        return None;
    }
    Some((path[first - 1], path[first..=last].to_vec(), path[last + 1]))
}

/// Version two: the first step of the shortest water-only road for `uid` to
/// any tile of `goals`, over sailable water no foreign unit holds, from the
/// unit's tile when it is at sea, else from the free water beside it. Only a
/// free first step is taken. `None` when the unit already stands on a goal,
/// no road exists, or no free first step leads to one.
fn water_step(
    g: &Game,
    pid: usize,
    uid: u32,
    rep: u32,
    goals: &std::collections::HashSet<Pos>,
) -> Option<Pos> {
    let here = g.units.get(&uid)?.pos;
    if goals.contains(&here) {
        return None;
    }
    let open = |pos: Pos| {
        sailable(g, rep, pos)
            && g.unit_ids_at(pos)
                .iter()
                .all(|id| g.units.get(id).is_none_or(|unit| unit.owner == pid))
    };
    let mut first: BTreeMap<Pos, Pos> = BTreeMap::new();
    let mut frontier: std::collections::VecDeque<Pos> = std::collections::VecDeque::new();
    for next in g.nbrs(here) {
        if open(next) && g.can_move(uid, next) && !first.contains_key(&next) {
            first.insert(next, next);
            frontier.push_back(next);
        }
    }
    while let Some(pos) = frontier.pop_front() {
        if goals.contains(&pos) {
            return first.get(&pos).copied();
        }
        let step = first[&pos];
        for next in g.nbrs(pos) {
            if next != here && !first.contains_key(&next) && open(next) {
                first.insert(next, step);
                frontier.push_back(next);
            }
        }
    }
    None
}

/// Version two's beach: the land within [`AMPHIB_BEACH_2`] of `landing`
/// joined to it over land inside that radius, so the near shore of a narrow
/// strait is never mistaken for the far one.
fn beach_zone(g: &Game, landing: Pos) -> BTreeSet<Pos> {
    let mut zone = BTreeSet::new();
    if !landable(g, landing) {
        return zone;
    }
    zone.insert(landing);
    let mut frontier = vec![landing];
    while let Some(pos) = frontier.pop() {
        for next in g.nbrs(pos) {
            if g.wdist(next, landing) <= AMPHIB_BEACH_2 && landable(g, next) && zone.insert(next) {
                frontier.push(next);
            }
        }
    }
    zone
}

/// The name a journal line gives the crossing's start: the port's name, or
/// the ford's shore.
fn start_name(g: &Game, train: &AmphibiousTrain) -> String {
    if train.ford {
        format!("the strait at {:?}", train.port_pos)
    } else {
        g.cities
            .get(&train.port)
            .map_or_else(|| format!("{:?}", train.port_pos), |city| city.name.clone())
    }
}

impl AdvancedAi {
    /// Whether either version of the gene runs.
    pub(super) fn amphibious_on(&self) -> bool {
        self.amphibious_siege_train || self.amphibious_siege_train_2
    }

    /// Version two's membership lock: `uid` belongs to the crossing in hand,
    /// so it is not combined, upgraded, pooled by the objective board or
    /// bound as a civilian's guard. Always false without version two.
    pub(super) fn amphibious_locked(&self, uid: u32) -> bool {
        self.amphibious_siege_train_2 && self.amphibious_member(uid)
    }

    /// Whether `uid` belongs to the crossing in hand. Always false with the
    /// gene off.
    pub(super) fn amphibious_member(&self, uid: u32) -> bool {
        self.amphibious_on()
            && self
                .amphibious_train
                .as_ref()
                .is_some_and(|train| train.members.contains(&uid))
    }

    /// The water hold's hook (`siege_stage_step`, `close_to_staging`): a
    /// member's turn when a crossing for `target` forms or is in hand, else
    /// `None` and the hold stands. `None` with the gene off.
    pub(super) fn amphibious_hold_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: u32,
    ) -> Option<bool> {
        if !self.amphibious_on() {
            return None;
        }
        let replaceable = self.amphibious_train.as_ref().is_some_and(|train| {
            train.phase == AmphibiousPhase::Assemble
                && train.target != target
                && !original_capital(g, train.target)
                && original_capital(g, target)
        });
        if self.amphibious_train.is_none() || replaceable {
            self.form_amphibious_train(g, pid, target);
        }
        if !self.amphibious_member(uid) {
            return None;
        }
        self.amphibious_member_step(g, pid, uid)
    }

    /// The unit step's first act for a member of the crossing in hand:
    /// `Some(acted)` while the train owns the unit, `None` otherwise and
    /// with the gene off.
    pub(super) fn amphibious_member_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.amphibious_member(uid) {
            return None;
        }
        if !self.amphibious_train_holds(g, pid) {
            return None;
        }
        let acted = self.amphibious_member_turn(g, pid, uid);
        // Version two hands a member the train let go (landed, or left
        // behind) back to its ordinary step this turn.
        if self.amphibious_siege_train_2 && !acted && !self.amphibious_member(uid) {
            return None;
        }
        Some(acted)
    }

    /// Form a crossing for `target` if the gate passes (module docs). True
    /// when one forms. A refusal is journalled once per turn and target
    /// ("Siege train cannot ship for"), so a water-locked game shows which
    /// gate held the guns.
    pub(super) fn form_amphibious_train(&mut self, g: &Game, pid: usize, target: u32) -> bool {
        match self.amphibious_gate(g, pid, target) {
            Ok(train) => {
                let guns = train.members.iter().filter(|id| is_gun(g, **id)).count();
                let target_name = g.cities[&target].name.clone();
                let port_name = start_name(g, &train);
                think!(self.journal(), Military, Decision,
                    "Siege train ships for {} from {}", target_name, port_name;
                    "amphibious-siege-train: no dry road reaches {}; {} gun(s) and {} escort(s) \
                     cross {} of {} water tiles, {}, to land at {:?}, {} tiles from the city",
                    target_name, guns, train.members.len() - guns,
                    if train.ford { "a ford" } else { "a lane" }, train.lane.len(),
                    if train.ford && train.lane.len() <= AMPHIB_FORD_WATER_MAX {
                        "no enemy warship beside it or within a turn's reach".to_string()
                    } else {
                        format!("no enemy warship within {AMPHIB_LANE_SAFETY}")
                    },
                    train.landing, g.wdist(train.landing, train.target_pos);
                    train.target_pos);
                self.amphibious_train = Some(train);
                true
            }
            Err(reason) => {
                if self.amphibious_refused != Some((g.turn, target)) {
                    self.amphibious_refused = Some((g.turn, target));
                    if let Some(city) = g.cities.get(&target) {
                        think!(self.journal(), Military, Detail,
                            "Siege train cannot ship for {}", city.name;
                            "amphibious-siege-train: {}", reason;
                            city.pos);
                    }
                }
                false
            }
        }
    }

    /// The gate (module docs): the crossing for `target`, or why not.
    fn amphibious_gate(
        &self,
        g: &Game,
        pid: usize,
        target: u32,
    ) -> Result<AmphibiousTrain, String> {
        let Some(city) = g.cities.get(&target) else {
            return Err("the city is gone".into());
        };
        if city.owner == pid || !g.is_at_war(pid, city.owner) {
            return Err("the city is not an enemy's".into());
        }
        if !can_embark(g, pid) {
            return Err("no embarkation technology".into());
        }
        let target_pos = city.pos;
        let Some(group) = self.force_groups.iter().find(|group| {
            group.domain == ForceDomain::Land && g.city_at(group.objective) == Some(target)
        }) else {
            return Err("no land force group has the city as its objective".into());
        };
        let ashore: Vec<u32> = group
            .units
            .iter()
            .copied()
            .filter(|id| {
                g.units
                    .get(id)
                    .is_some_and(|unit| unit.owner == pid && !g.is_embarked(unit) && unit.hp > 0)
            })
            .collect();
        let Some(rep) = ashore.iter().copied().find(|id| is_gun(g, *id)) else {
            return Err("the force group has no gun ashore".into());
        };
        // Version two: a strait of at most two water tiles is forded. A ford
        // that cannot form falls back to the sea lane, so version two ships
        // whatever version one would.
        let ford = if self.amphibious_siege_train_2 {
            self.amphibious_ford(g, pid, target, target_pos, &ashore)
        } else {
            None
        };
        let ford_refused = match ford {
            Some(Ok(train)) => return Ok(train),
            Some(Err(reason)) => Some(reason),
            None => None,
        };
        self.amphibious_lane(g, pid, target, target_pos, rep, &ashore)
            .map_err(|lane| match ford_refused {
                Some(ford) => format!("{ford}; by sea, {lane}"),
                None => lane,
            })
    }

    /// The sea-lane crossing (module docs): the shortest lane from a coastal
    /// city of ours to a landing beside the target, or why not.
    fn amphibious_lane(
        &self,
        g: &Game,
        pid: usize,
        target: u32,
        target_pos: Pos,
        rep: u32,
        ashore: &[u32],
    ) -> Result<AmphibiousTrain, String> {
        let landings = landing_tiles(g, pid, rep, target_pos);
        if landings.is_empty() {
            return Err(format!(
                "no free landing tile beside the water within {AMPHIB_LANDING_MAX} of the city"
            ));
        }
        let ports: Vec<(u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| (cid, g.cities[&cid].pos))
            .filter(|(_, pos)| !shore_water(g, rep, *pos).is_empty())
            .collect();
        if ports.is_empty() {
            return Err("no coastal city of ours".into());
        }
        // The guns and escorts a port musters from `pool`, nearest first.
        let muster = |port_pos: Pos, pool: &[u32]| {
            let mut near_port: Vec<(i32, u32)> = pool
                .iter()
                .map(|id| (g.wdist(g.units[id].pos, port_pos), *id))
                .filter(|(distance, _)| *distance <= AMPHIB_MUSTER_RADIUS)
                .collect();
            near_port.sort_unstable();
            let guns: Vec<u32> = near_port
                .iter()
                .map(|(_, id)| *id)
                .filter(|id| is_gun(g, *id))
                .take(AMPHIB_MAX_GUNS)
                .collect();
            let escorts: Vec<u32> = near_port
                .iter()
                .map(|(_, id)| *id)
                .filter(|id| is_escort(g, *id))
                .take(AMPHIB_MAX_ESCORTS)
                .collect();
            (guns, escorts)
        };
        // Version two: the crossing starts where the dry road ends. A port on
        // the objective's own landmass is passed over (its units march), and
        // a port drafts only the units on land joined to it, none of which
        // has a dry road; a port that musters too few is passed over too.
        // Live Immortal 204616Z (replayed with the ford fix) shipped from
        // Cuenca, on our mainland with Durocortorum, across one tile of a bay
        // to land at (11, 17) on that same mainland, drafting the column that
        // was marching there overland.
        let v2 = self.amphibious_siege_train_2;
        let mut drafted: BTreeMap<u32, (Vec<u32>, Vec<u32>)> = BTreeMap::new();
        let ports = if v2 {
            let objective_land = land_joined(g, target_pos, None);
            let off: Vec<(u32, Pos)> = ports
                .into_iter()
                .filter(|(_, pos)| !objective_land.contains(pos))
                .collect();
            if off.is_empty() {
                return Err(format!(
                    "every coastal city of ours stands on {}'s own landmass, with a dry road \
                     to it",
                    g.cities[&target].name
                ));
            }
            for (cid, pos) in &off {
                drafted.insert(*cid, muster(*pos, &near_shore_members(g, *pos, ashore)));
            }
            let mustered: Vec<(u32, Pos)> = off
                .iter()
                .copied()
                .filter(|(cid, _)| {
                    let (guns, escorts) = &drafted[cid];
                    !guns.is_empty() && escorts.len() >= AMPHIB_MIN_ESCORTS
                })
                .collect();
            if mustered.is_empty() {
                let (cid, (guns, escorts)) = drafted
                    .iter()
                    .max_by_key(|(cid, (guns, escorts))| {
                        (guns.len().min(1) + escorts.len(), std::cmp::Reverse(**cid))
                    })
                    .expect("a port off the objective's landmass");
                return Err(format!(
                    "{} gun(s) and {} escort(s) on {}'s land within {AMPHIB_MUSTER_RADIUS} of it \
                     (needs 1 and {AMPHIB_MIN_ESCORTS}); units with a dry road march",
                    guns.len(),
                    escorts.len(),
                    g.cities[cid].name
                ));
            }
            mustered
        } else {
            ports
        };
        // The shortest lane over every port and landing; nearer members
        // break a tie, then the board's order.
        #[allow(clippy::type_complexity)]
        let mut best: Option<((usize, i32, usize), u32, Pos, Pos, Vec<Pos>)> = None;
        for (rank, landing) in landings.iter().enumerate() {
            for (cid, port) in &ports {
                let Some(lane) = sea_lane(g, pid, rep, *port, *landing) else {
                    continue;
                };
                let near: i32 = ashore
                    .iter()
                    .map(|id| g.wdist(g.units[id].pos, *port))
                    .sum();
                let key = (lane.len(), near, rank);
                if best.as_ref().is_none_or(|(old, ..)| key < *old) {
                    best = Some((key, *cid, *port, *landing, lane));
                }
            }
        }
        let Some((_, port, port_pos, landing, lane)) = best else {
            return Err(format!(
                "no clear sea lane of at most {AMPHIB_SEA_LANE_MAX} tiles from {} port(s) to {} \
                 landing(s)",
                ports.len(),
                landings.len()
            ));
        };
        if lane_watched(g, pid, &lane) {
            return Err(format!(
                "an enemy warship within {AMPHIB_LANE_SAFETY} of the lane from {}",
                g.cities[&port].name
            ));
        }
        let (guns, escorts) = match drafted.remove(&port) {
            Some(mustered) => mustered,
            None => muster(port_pos, ashore),
        };
        if guns.is_empty() || escorts.len() < AMPHIB_MIN_ESCORTS {
            return Err(format!(
                "{} gun(s) and {} escort(s) within {AMPHIB_MUSTER_RADIUS} of {} (needs 1 and \
                 {AMPHIB_MIN_ESCORTS})",
                guns.len(),
                escorts.len(),
                g.cities[&port].name
            ));
        }
        Ok(AmphibiousTrain {
            target,
            target_pos,
            port,
            port_pos,
            landing,
            lane,
            members: guns.iter().chain(escorts.iter()).copied().collect(),
            phase: AmphibiousPhase::Assemble,
            formed: g.turn,
            ford: false,
            progress: BTreeMap::new(),
            progress_turn: g.turn,
        })
    }

    /// Version two's ford for `target` (module docs): `None` when the
    /// fewest-water road from the gun `rep` is no ford (the sea lane is tried
    /// next), else the ford train or why it cannot form.
    fn amphibious_ford(
        &self,
        g: &Game,
        pid: usize,
        target: u32,
        target_pos: Pos,
        ashore: &[u32],
    ) -> Option<Result<AmphibiousTrain, String>> {
        // From each gun ashore in turn (a group can stand on two shores):
        // the first ford that forms, else why the first found cannot.
        let mut refused = None;
        for rep in ashore.iter().copied().filter(|id| is_gun(g, *id)) {
            let Some((shore, water, landing)) = ford_crossing(
                g,
                pid,
                rep,
                g.units[&rep].pos,
                target_pos,
                super::siege_train::STAGING_FAR,
            ) else {
                continue;
            };
            match self.ford_train(g, pid, target, target_pos, ashore, shore, water, landing) {
                Ok(train) => return Some(Ok(train)),
                Err(reason) => {
                    refused.get_or_insert(reason);
                }
            }
        }
        refused.map(Err)
    }

    /// The ford train over `water` from `shore` to `landing`, or why not.
    #[allow(clippy::too_many_arguments)]
    fn ford_train(
        &self,
        g: &Game,
        pid: usize,
        target: u32,
        target_pos: Pos,
        ashore: &[u32],
        shore: Pos,
        water: Vec<Pos>,
        landing: Pos,
    ) -> Result<AmphibiousTrain, String> {
        if ford_watched(g, pid, &water) {
            return Err(format!(
                "an enemy warship beside the ford at {shore:?} or within a turn's reach of it"
            ));
        }
        // Only the units that can gather at the near shore.
        let ashore = near_shore_members(g, shore, ashore);
        let ashore = ashore.as_slice();
        let Some(port) = g
            .player_city_ids(pid)
            .into_iter()
            .min_by_key(|cid| (g.wdist(g.cities[cid].pos, shore), *cid))
        else {
            return Err("no city of ours".into());
        };
        let mut near: Vec<(i32, u32)> = ashore
            .iter()
            .map(|id| (g.wdist(g.units[id].pos, shore), *id))
            .filter(|(distance, _)| *distance <= AMPHIB_MUSTER_RADIUS)
            .collect();
        near.sort_unstable();
        let guns: Vec<u32> = near
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| is_gun(g, *id))
            .take(AMPHIB_MAX_GUNS)
            .collect();
        let escorts: Vec<u32> = near
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| is_escort(g, *id))
            .take(AMPHIB_MAX_ESCORTS)
            .collect();
        if guns.is_empty() || escorts.len() < AMPHIB_MIN_ESCORTS {
            return Err(format!(
                "{} gun(s) and {} escort(s) on the near shore's land within \
                 {AMPHIB_MUSTER_RADIUS} of the ford at {shore:?} (needs 1 and \
                 {AMPHIB_MIN_ESCORTS})",
                guns.len(),
                escorts.len()
            ));
        }
        Ok(AmphibiousTrain {
            target,
            target_pos,
            port,
            port_pos: shore,
            landing,
            lane: water,
            members: guns.iter().chain(escorts.iter()).copied().collect(),
            phase: AmphibiousPhase::Assemble,
            formed: g.turn,
            ford: true,
            progress: BTreeMap::new(),
            progress_turn: g.turn,
        })
    }

    /// Version two: read each member's closing once a turn, against the
    /// gathering point while assembling and the landing while sailing. A
    /// member in place (gathered ashore; ashore on the beach) or nearer its
    /// goal than ever is closing. On a turn no member closed, the furthest
    /// member still waited on and not in place counts a stalled turn: the
    /// straggler a cohesion wait holds the others for, or the one left
    /// behind once the rest are in place. A member queued at a ford's shore
    /// while the others cross is not stalled, since someone crossed.
    fn amphibious_track_progress(&mut self, g: &Game) {
        if !self.amphibious_siege_train_2 {
            return;
        }
        let Some(train) = self.amphibious_train.as_mut() else {
            return;
        };
        if train.progress_turn == g.turn && !train.progress.is_empty() {
            return;
        }
        train.progress_turn = g.turn;
        let beach = match train.phase {
            AmphibiousPhase::Assemble => BTreeSet::new(),
            AmphibiousPhase::Sail => beach_zone(g, train.landing),
        };
        let in_place = |unit: &crate::game::Unit| {
            !g.is_embarked(unit)
                && match train.phase {
                    AmphibiousPhase::Assemble => {
                        g.wdist(unit.pos, train.port_pos) <= AMPHIB_ASSEMBLY_RADIUS
                    }
                    AmphibiousPhase::Sail => beach.contains(&unit.pos),
                }
        };
        let goal = match train.phase {
            AmphibiousPhase::Assemble => train.port_pos,
            AmphibiousPhase::Sail => train.landing,
        };
        // (id, distance, in place, known best and stalled turns)
        let readings: Vec<(u32, i32, bool, (i32, u32))> = train
            .members
            .iter()
            .filter_map(|id| {
                let unit = g.units.get(id)?;
                let known = train.progress.get(id).copied().unwrap_or((i32::MAX, 0));
                Some((*id, g.wdist(unit.pos, goal), in_place(unit), known))
            })
            .collect();
        let progressed = readings
            .iter()
            .any(|(_, distance, _, (best, _))| *distance < *best);
        let furthest = readings
            .iter()
            .filter(|(_, _, placed, (_, stalled))| !placed && *stalled < AMPHIB_STALL_TURNS)
            .map(|(_, distance, ..)| *distance)
            .max();
        // The gathering, once a turn: who is in place and who is waited on.
        let gather = (train.phase == AmphibiousPhase::Assemble).then(|| {
            let placed = readings.iter().filter(|(_, _, placed, _)| *placed).count();
            let laggard = readings
                .iter()
                .filter(|(_, _, placed, _)| !placed)
                .max_by_key(|(id, distance, ..)| (*distance, std::cmp::Reverse(*id)))
                .map(|(id, distance, ..)| {
                    let kind = g
                        .units
                        .get(id)
                        .map_or_else(|| "unit".to_string(), |unit| unit.kind.to_string());
                    format!("{kind} {id} at d={distance}")
                });
            (
                g.cities
                    .get(&train.target)
                    .map_or_else(|| "its target".to_string(), |city| city.name.clone()),
                placed,
                readings.len(),
                if train.ford { "ford" } else { "port" },
                laggard,
                train.target_pos,
            )
        });
        train.progress = readings
            .into_iter()
            .map(|(id, distance, placed, (best, stalled))| {
                let entry = if placed || distance < best {
                    (distance.min(best), 0)
                } else if !progressed && Some(distance) == furthest {
                    (best, stalled + 1)
                } else {
                    (best, stalled)
                };
                (id, entry)
            })
            .collect();
        if let Some((name, placed, total, point, laggard, target_pos)) = gather {
            think!(self.journal(), Military, Detail,
                "Siege train gathers for {}", name;
                "amphibious-siege-train: {} of {} within {} of the {}; {}",
                placed, total, AMPHIB_ASSEMBLY_RADIUS, point,
                laggard.map_or_else(|| "all in place".to_string(), |who| format!("waiting on {who}"));
                target_pos);
        }
    }

    /// The members the waits hold for: under version two, every member but
    /// a laggard that has not closed for [`AMPHIB_STALL_TURNS`] turns (all of
    /// them if that would leave no gun or no escort); every member under
    /// version one.
    fn amphibious_waited_on(&self, g: &Game, train: &AmphibiousTrain) -> Vec<u32> {
        let all: Vec<u32> = train.members.iter().copied().collect();
        if !self.amphibious_siege_train_2 {
            return all;
        }
        let kept: Vec<u32> = all
            .iter()
            .copied()
            .filter(|id| {
                train
                    .progress
                    .get(id)
                    .is_none_or(|(_, stalled)| *stalled < AMPHIB_STALL_TURNS)
            })
            .collect();
        let guns = kept.iter().any(|id| is_gun(g, *id));
        let escorts = kept.iter().any(|id| is_escort(g, *id));
        if guns && escorts {
            kept
        } else {
            all
        }
    }

    /// Whether the crossing in hand still holds; abandons it (journal) and
    /// returns false when not. Prunes lost members.
    pub(super) fn amphibious_train_holds(&mut self, g: &Game, pid: usize) -> bool {
        let Some(mut train) = self.amphibious_train.take() else {
            return false;
        };
        train
            .members
            .retain(|id| g.units.get(id).is_some_and(|unit| unit.owner == pid));
        let guns = train.members.iter().filter(|id| is_gun(g, **id)).count();
        let escorts = train.members.iter().filter(|id| is_escort(g, **id)).count();
        let reason = match g.cities.get(&train.target) {
            None => Some("the city is gone".to_string()),
            Some(city) if city.owner == pid || !g.is_at_war(pid, city.owner) => {
                Some("the city is no longer an enemy's".to_string())
            }
            Some(_) if g.turn.saturating_sub(train.formed) > AMPHIB_PATIENCE => {
                Some(format!("{AMPHIB_PATIENCE} turns passed"))
            }
            Some(_) if guns == 0 => Some("no gun is left".to_string()),
            Some(_) if escorts == 0 => Some("no escort is left".to_string()),
            Some(_) if train_watched(g, pid, &train) => Some(
                if train.ford && train.lane.len() <= AMPHIB_FORD_WATER_MAX {
                    "an enemy warship beside the ford or within a turn's reach of it"
                } else {
                    "an enemy warship watches the lane"
                }
                .to_string(),
            ),
            Some(_) => None,
        };
        if let Some(reason) = reason {
            let name = g
                .cities
                .get(&train.target)
                .map_or_else(|| "its target".to_string(), |city| city.name.clone());
            think!(self.journal(), Military, Decision,
                "Siege train abandons the crossing to {}", name;
                "amphibious-siege-train: {}", reason;
                train.target_pos);
            return false;
        }
        self.amphibious_train = Some(train);
        true
    }

    /// One member's turn in the crossing (module docs).
    fn amphibious_member_turn(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        self.amphibious_track_progress(g);
        let Some(train) = self.amphibious_train.clone() else {
            return false;
        };
        let v2 = self.amphibious_siege_train_2;
        let rep = train
            .members
            .iter()
            .copied()
            .find(|id| is_gun(g, *id))
            .unwrap_or(uid);
        let at = |g: &Game, id: u32| g.units[&id].pos;
        let waited = self.amphibious_waited_on(g, &train);
        let start = start_name(g, &train);
        if train.phase == AmphibiousPhase::Assemble {
            let gathered = waited.iter().all(|id| {
                g.wdist(at(g, *id), train.port_pos) <= AMPHIB_ASSEMBLY_RADIUS
                    && !g.is_embarked(&g.units[id])
            });
            if gathered {
                // Version two leaves a member that never gathered behind.
                let left: Vec<u32> = train
                    .members
                    .iter()
                    .copied()
                    .filter(|id| !waited.contains(id))
                    .collect();
                if let Some(held) = self.amphibious_train.as_mut() {
                    held.phase = AmphibiousPhase::Sail;
                    held.members.retain(|id| !left.contains(id));
                    held.progress.clear();
                }
                think!(self.journal(), Military, Detail,
                    "Siege train embarks for {} from {}", g.cities[&train.target].name, start;
                    "amphibious-siege-train: {} member(s) gathered within {} tiles of {}{}",
                    waited.len(), AMPHIB_ASSEMBLY_RADIUS,
                    if train.ford { "the strait" } else { "the port" },
                    if left.is_empty() {
                        String::new()
                    } else {
                        format!("; {} that did not close for {} turns stay behind",
                            left.len(), AMPHIB_STALL_TURNS)
                    };
                    train.target_pos);
                if left.contains(&uid) {
                    return false;
                }
                return self.amphibious_member_turn(g, pid, uid);
            }
            if g.wdist(at(g, uid), train.port_pos) <= AMPHIB_ASSEMBLY_RADIUS {
                return self.base.fortify_or_stop(g, pid, uid);
            }
            let step = g
                .route_step_dry(
                    uid,
                    train.port_pos,
                    AMPHIB_ASSEMBLY_RADIUS,
                    super::siege_train::STAGE_DRY_LIMIT,
                )
                .map(|(step, _)| step)
                .or_else(|| g.route_step(uid, train.port_pos, AMPHIB_ASSEMBLY_RADIUS))
                .filter(|pos| landable(g, *pos) && g.can_move(uid, *pos));
            return match step {
                Some(pos) => self.base.tactical_apply_move(g, pid, uid, pos),
                None => self.base.fortify_or_stop(g, pid, uid),
            };
        }
        // Sail. The beach is the landing tile and the land beside it; under
        // version two, the land joined to it within two tiles.
        let beach: BTreeSet<Pos> = if v2 {
            beach_zone(g, train.landing)
        } else {
            std::iter::once(train.landing)
                .chain(g.nbrs(train.landing))
                .filter(|pos| landable(g, *pos))
                .collect()
        };
        let ashore_on_beach = |g: &Game, id: u32| {
            let unit = &g.units[&id];
            !g.is_embarked(unit)
                && if v2 {
                    beach.contains(&unit.pos)
                } else {
                    g.wdist(unit.pos, train.landing) <= 2
                }
        };
        if waited.iter().all(|id| ashore_on_beach(g, *id)) {
            think!(self.journal(), Military, Decision,
                "Siege train lands at {:?} before {}", train.landing, g.cities[&train.target].name;
                "amphibious-siege-train: {} member(s) ashore {} tiles from the city; the \
                 siege march takes them on",
                waited.len(), g.wdist(train.landing, train.target_pos);
                train.target_pos);
            self.amphibious_train = None;
            return false;
        }
        let here = at(g, uid);
        if ashore_on_beach(g, uid) {
            // Version two clears the water's edge for the members still
            // crossing: one step inland, escorts toward the city, guns away.
            if v2 {
                let coastal = |pos: Pos| !shore_water(g, rep, pos).is_empty();
                if coastal(here) {
                    let inland = g
                        .nbrs(here)
                        .into_iter()
                        .filter(|pos| {
                            beach.contains(pos) && !coastal(*pos) && g.can_move(uid, *pos)
                        })
                        .min_by_key(|pos| {
                            let toward = g.wdist(*pos, train.target_pos);
                            (if is_gun(g, uid) { -toward } else { toward }, *pos)
                        });
                    if let Some(pos) = inland {
                        return self.base.tactical_apply_move(g, pid, uid, pos);
                    }
                }
            }
            return self.base.fortify_or_stop(g, pid, uid);
        }
        let embarked = g.is_embarked(&g.units[&uid]);
        // A free beach tile beside a member at sea: escorts land once the
        // whole train is at sea within two tiles of the landing (version two:
        // the members waited on, within three; a ford's at once); a gun lands
        // only beside an escort already ashore.
        if embarked {
            let free: Vec<Pos> = g
                .nbrs(here)
                .into_iter()
                .filter(|pos| beach.contains(pos) && g.can_move(uid, *pos))
                .collect();
            if !free.is_empty() {
                let reach = if v2 { AMPHIB_ARRIVED_2 } else { 2 };
                let arrived = (v2 && train.ford)
                    || waited
                        .iter()
                        .all(|id| g.wdist(at(g, *id), train.landing) <= reach);
                if is_escort(g, uid) && arrived {
                    // The escort shields: the free tile nearest the city.
                    let pos = *free
                        .iter()
                        .min_by_key(|pos| (g.wdist(**pos, train.target_pos), **pos))
                        .expect("non-empty");
                    return self.base.tactical_apply_move(g, pid, uid, pos);
                }
                if is_gun(g, uid) {
                    // Beside an escort ashore (the beach is the landing tile
                    // and its ring, so two tiles reach across it), on the
                    // free tile furthest from the city.
                    let covered = free
                        .iter()
                        .copied()
                        .filter(|pos| {
                            train.members.iter().any(|id| {
                                is_escort(g, *id)
                                    && ashore_on_beach(g, *id)
                                    && g.wdist(at(g, *id), *pos) <= 2
                            })
                        })
                        .max_by_key(|pos| {
                            (g.wdist(*pos, train.target_pos), std::cmp::Reverse(*pos))
                        });
                    if let Some(pos) = covered {
                        return self.base.tactical_apply_move(g, pid, uid, pos);
                    }
                }
                // A gun with no escort ashore yet, or an escort waiting for
                // the rest of the train: hold beside the beach.
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }
        // Keep together: a member well ahead of the furthest waits (not over
        // a ford, which one land unit a tile crosses in turn).
        let mine = g.wdist(here, train.landing);
        let furthest = waited
            .iter()
            .filter(|id| !ashore_on_beach(g, **id))
            .map(|id| g.wdist(at(g, *id), train.landing))
            .max()
            .unwrap_or(mine);
        if embarked && !(v2 && train.ford) && mine + AMPHIB_COHESION < furthest {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        // Steer for water beside a FREE beach tile once any lies open, so a
        // member whose beach neighbour an escort took moves along the shore.
        let free_beach: Vec<Pos> = beach
            .iter()
            .copied()
            .filter(|pos| g.unit_ids_at(*pos).is_empty())
            .collect();
        let toward = if free_beach.is_empty() {
            beach.iter().copied().collect::<Vec<_>>()
        } else {
            free_beach
        };
        let anchorage: std::collections::HashSet<Pos> = toward
            .iter()
            .flat_map(|pos| shore_water(g, rep, *pos))
            .collect();
        if anchorage.contains(&here) {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        // Version two sails on water only; a member still ashore takes the
        // mixed road to the water when no water lies beside it.
        let step = if v2 {
            water_step(g, pid, uid, rep, &anchorage).or_else(|| {
                (!embarked)
                    .then(|| g.route_step_to_any(uid, &anchorage))
                    .flatten()
            })
        } else {
            g.route_step_to_any(uid, &anchorage)
        };
        match step.filter(|pos| g.can_move(uid, *pos)) {
            Some(pos) => self.base.tactical_apply_move(g, pid, uid, pos),
            None => self.base.fortify_or_stop(g, pid, uid),
        }
    }

    /// Follow the crossing in hand through a live board rebuild
    /// (`remap_siege_memory`): the target and the port by position, the
    /// members through `units` (old id to new, same owner). The live seat
    /// plays `--fresh-board`, which renumbers cities and units every turn;
    /// kept under the old ids, a replay of live Emperor 004440Z re-formed the
    /// train for Amsterdam on t156 two turns after it shipped, because the
    /// old target id no longer named Amsterdam. A train whose target or port
    /// the new board does not hold is dropped (a ford's shore holds no city;
    /// its port follows as the owner's nearest city); the next water hold
    /// re-forms it. Each member's closing follows its id.
    pub(super) fn remap_amphibious_train(
        &mut self,
        previous: &Game,
        next: &Game,
        units: &BTreeMap<u32, u32>,
    ) {
        let Some(mut train) = self.amphibious_train.take() else {
            return;
        };
        let Some(target) = next.city_at(train.target_pos) else {
            return;
        };
        // A ford's shore is no city: its port is the owner's nearest city.
        let port = next.city_at(train.port_pos).or_else(|| {
            let owner = previous.cities.get(&train.port)?.owner;
            next.player_city_ids(owner)
                .into_iter()
                .min_by_key(|cid| (next.wdist(next.cities[cid].pos, train.port_pos), *cid))
                .filter(|_| train.ford)
        });
        let Some(port) = port else {
            return;
        };
        train.target = target;
        train.port = port;
        let renumber = |old: u32| {
            let new = *units.get(&old)?;
            (previous.units.get(&old)?.owner == next.units.get(&new)?.owner).then_some(new)
        };
        train.members = std::mem::take(&mut train.members)
            .into_iter()
            .filter_map(renumber)
            .collect();
        train.progress = std::mem::take(&mut train.progress)
            .into_iter()
            .filter_map(|(old, entry)| Some((renumber(old)?, entry)))
            .collect();
        self.amphibious_train = Some(train);
    }
}

/// Whether `cid` is a major's original capital still held by its founder.
fn original_capital(g: &Game, cid: u32) -> bool {
    g.cities
        .get(&cid)
        .is_some_and(|city| city.is_capital && city.original_owner == city.owner)
}

#[cfg(test)]
pub(super) mod tests;
