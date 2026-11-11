//! Pathfinding: routes for units across the map.
//!
//! `path_to` and `paths_to`, `route_step` and its cache, the first step
//! and steps toward any of several goals, route distance and access keys,
//! movement relaxation (`relax_movement`, `relax_movement_into`), the
//! traversal checks, the movement-shortfall step, and invalidating cached
//! paths. Routing around temporarily unavailable tiles is in
//! `game/route_avoidance.rs`.
//!
//! Reach and flow fields: where a unit can get to this turn
//! (`reachable`, `reach_steps`, `step_cost_for`), how close it can
//! approach a target and who can arrive (`approach_reach`,
//! `approach_arrivals`, `pass_through_destination`), the threat reach an
//! enemy projects (`threat_reach`), routing zones and their cache stamp,
//! and the reverse flow fields that answer "how far is every tile from any
//! of these goals" (`reverse_goal_distance`, `build_reverse_flow_field`,
//! `march_reach_toward_any`, `flow`, `flow_past`). These moved verbatim out
//! of the main `impl Game` block in `game.rs` later the same day; the
//! rules a step obeys are in `game/movement.rs`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn can_move(&self, uid: u32, pos: Pos) -> bool {
        self.can_move_step(uid, pos, true)
    }

    /// The movement `uid` has left and the cost of one step onto `to`, when
    /// it cannot pay that step (see [`Self::can_pay_step`]). `None` when the
    /// step is affordable or the unit is unknown. Civilization VI charges the
    /// whole terrain cost before entering, so a unit with 1 point left stops
    /// in front of a 2-point hill and walks on next turn: the end of a move,
    /// not a hold.
    pub(crate) fn step_movement_shortfall(&self, uid: u32, to: Pos) -> Option<(f64, f64)> {
        let unit = self.units.get(&uid)?;
        (!self.can_pay_step(uid, unit.pos, to))
            .then(|| (unit.moves_left, self.unit_step_cost(uid, unit.pos, to)))
    }

    /// Every destination [`Self::path_to`] can answer this turn, with its
    /// exact path.
    ///
    /// A caller that compares several destinations from one unchanged board
    /// must not rebuild the same movement flood for each one.  Keep the
    /// `f64` arrival mode from `path_to` here: a tile inside enemy zone of
    /// control records zero movement, so the strict-improvement rule retains
    /// the same parent and tie-breaking path as an individual query.
    pub(crate) fn paths_to(&self, uid: u32) -> BTreeMap<Pos, Vec<Pos>> {
        let mut paths = BTreeMap::new();
        let Some(unit) = self.units.get(&uid) else {
            return paths;
        };
        let (start, moves) = (unit.pos, unit.moves_left);
        // `path_to` treats the current tile as an empty legal path before it
        // checks movement, formation locks, or whether the tile can stop a
        // different unit.  Preserve that observable answer in the bulk form.
        paths.insert(start, Vec::new());
        if !self.formation_movement_locked_by_zoc(uid) {
            let _memo = self.query_memo();
            let mut parent: BTreeMap<Pos, Pos> = BTreeMap::new();
            let reach: BTreeMap<Pos, f64> = self.relax_movement(
                uid,
                start,
                moves,
                |cur, next| self.can_pass_neighbor(uid, cur, next),
                |next, cur| {
                    parent.insert(next, cur);
                },
            );
            for (to, _) in reach {
                if to == start || !self.can_stop(uid, to) {
                    continue;
                }
                let mut path = vec![to];
                let mut cur = to;
                while let Some(previous) = parent.get(&cur) {
                    if *previous == start {
                        break;
                    }
                    path.push(*previous);
                    cur = *previous;
                }
                path.reverse();
                paths.insert(to, path);
            }
        }
        // `path_to` deliberately handles adjacent destinations through
        // `can_move`: that carries immediate movement/attack and linked-unit
        // gates which a longer-path flood has historically not asked.  Replace
        // the flood's answer for precisely those six destinations so this
        // bulk reader stays byte-for-byte equivalent to individual queries.
        for to in self.nbrs(start) {
            if self.can_move(uid, to) {
                paths.insert(to, vec![to]);
            } else {
                paths.remove(&to);
            }
        }
        paths
    }

    /// First step of a deterministic long-range route to `to`, stopping once
    /// the unit is within `stop_range`. Unlike `reachable`, this plans across
    /// future turns so AI units can detour around mountains, coastlines, and
    /// occupied choke points instead of getting stuck in a greedy local
    /// minimum. The returned step is still validated against the current
    /// turn by the caller's normal Move action.
    pub fn route_step(&self, uid: u32, to: Pos, stop_range: i32) -> Option<Pos> {
        let unit = self.units.get(&uid)?;
        let start = unit.pos;
        let range = stop_range.max(0);
        if self.formation_movement_locked_by_zoc(uid)
            || self.wdist_capped(start, to, range) <= range
        {
            return None;
        }
        if range == 0 {
            self.map.get(to)?;
            if !self.unit_can_traverse(uid, to) {
                return None;
            }
        }
        // Refresh the cheap epoch guard before consulting a retained path.
        // This lets a cache hit bypass the full connected-region proof while
        // still dropping paths after a map write, turn change, or city-count
        // change.
        self.refresh_routing_cache_stamp();
        let has_path = {
            let routing = self.routing.borrow();
            routing
                .paths
                .iter()
                .any(|route| route.unit == uid && route.target == to && route.range == range)
        };
        if has_path {
            if let Some(cached_step) = self.cached_route_step(unit, start, to, range) {
                return Some(cached_step);
            }
        }
        if !self.zone_connected(uid, to, range) {
            return None; // proven: no chain of traversable tiles links them
        }
        let territory_access = self.unit_territory_access(unit);
        if range == 0
            && self
                .territory_owner_at(to)
                .is_some_and(|owner| !territory_access[owner])
        {
            return None;
        }
        let access_key = self.route_access_key(unit, territory_access.as_slice());
        let traversal_zones = self.routing_zones(self.traversal_class(uid));

        // A* keeps known-target routing cheap enough for high-throughput
        // self-play: the zone check above has already rejected disconnected
        // targets, so the consistent hex-distance heuristic bounds the search
        // to roughly the corridor of the route itself, however long the
        // detour. Tuple ordering gives deterministic tie-breaking.
        // Same A*, same tie-breaking, but the frontier's bookkeeping is two
        // dense vectors indexed by the map's own tile numbering rather than
        // hash maps keyed by a hex coordinate; and the whole search asks the
        // unit how it moves once instead of at every neighbour it considers.
        const UNREACHED: i32 = i32::MAX;
        const NO_PARENT: u32 = u32::MAX;
        let _memo = self.query_memo();
        let tile_count = self.map.tiles.len();
        let start_index = self.map.tiles.index_of(start)?;
        let mut scratch = self.route_scratch.borrow_mut();
        if scratch.parent.len() < tile_count {
            scratch.parent.resize(tile_count, NO_PARENT);
        }
        if scratch.distance.len() < tile_count {
            scratch.distance.resize(tile_count, UNREACHED);
        }
        let touched = scratch.astar_touched.len();
        for index in 0..touched {
            let slot = scratch.astar_touched[index];
            scratch.parent[slot] = NO_PARENT;
            scratch.distance[slot] = UNREACHED;
        }
        scratch.astar_touched.clear();
        scratch.astar_frontier.clear();
        scratch.path.clear();
        scratch.parent[start_index] = NO_PARENT;
        scratch.distance[start_index] = 0;
        scratch.astar_touched.push(start_index);
        // On equal f-scores, prefer the node furthest along the route. This
        // avoids breadth-first expansion of an entire open plain/ocean before
        // following one of several equally short paths to the destination.
        scratch.astar_frontier.push(Reverse((
            self.wdist(start, to),
            Reverse(0),
            start,
            start_index,
        )));

        let mut goal = None;
        let map_tiles = self.map.tiles.values().as_slice();
        while let Some(Reverse((estimate, Reverse(traveled), cur, cur_index))) =
            scratch.astar_frontier.pop()
        {
            if traveled != scratch.distance[cur_index] {
                continue;
            }
            // The queued estimate is traveled + max(distance - range, 0).
            // It equals traveled exactly at a goal, so do not measure this
            // globe distance a second time when the node leaves the queue.
            // The initial node has a positive estimate and is already known
            // to be outside the goal range by the guard above.
            debug_assert_eq!(estimate == traveled, self.wdist(cur, to) <= range);
            if estimate == traveled {
                goal = Some(cur_index);
                break;
            }
            let neighbors = self.map.neighbor_indices(cur_index);
            for &neighbor_index in neighbors.as_slice() {
                let index = neighbor_index as usize;
                let n = map_tiles[index].pos;
                // As in `first_route_step`: the distance test is a array read
                // and `can_path_through_known_traversable` is a ruleset sweep, so test the
                // cheap one first. Every edge cost here is 1, so a neighbour
                // that cannot improve on the distance already recorded is the
                // common case, and both orders leave state untouched when
                // they skip.
                let next_distance = traveled + 1;
                if next_distance >= scratch.distance[index] {
                    continue;
                }
                let enterable = if cur == start {
                    self.can_enter_neighbor(uid, cur, n)
                } else {
                    if traversal_zones[index] == 0 {
                        continue;
                    }
                    self.can_path_through_known_traversable(uid, n, territory_access.as_slice())
                };
                if !enterable {
                    continue;
                }
                if scratch.distance[index] == UNREACHED {
                    scratch.astar_touched.push(index);
                }
                scratch.distance[index] = next_distance;
                scratch.parent[index] = cur_index as u32;
                let estimate = next_distance + (self.wdist(n, to) - range).max(0);
                scratch
                    .astar_frontier
                    .push(Reverse((estimate, Reverse(next_distance), n, index)));
            }
        }
        let mut cursor = goal?;
        scratch.path.push(map_tiles[cursor].pos);
        while cursor != start_index {
            let previous = scratch.parent[cursor];
            if previous == NO_PARENT {
                return None;
            }
            cursor = previous as usize;
            scratch.path.push(map_tiles[cursor].pos);
        }
        scratch.path.reverse();
        let step = *scratch.path.get(1)?;
        let mut routing = self.routing.borrow_mut();
        routing.paths.retain(|route| route.unit != uid);
        routing.paths.push(PlannedRoute {
            unit: uid,
            target: to,
            range,
            access_key,
            path: std::mem::take(&mut scratch.path),
        });
        Some(step)
    }

    fn cached_route_step(&self, unit: &Unit, start: Pos, to: Pos, range: i32) -> Option<Pos> {
        let territory_access = self.unit_territory_access(unit);
        if range == 0
            && self
                .territory_owner_at(to)
                .is_some_and(|owner| !territory_access[owner])
        {
            return None;
        }
        let access_key = self.route_access_key(unit, territory_access.as_slice());
        let cached_step = {
            let routing = self.routing.borrow();
            routing
                .paths
                .iter()
                .find(|route| {
                    route.unit == unit.id
                        && route.target == to
                        && route.range == range
                        && route.access_key == access_key
                })
                .and_then(|route| {
                    route
                        .path
                        .iter()
                        .position(|position| *position == start)
                        .and_then(|index| route.path.get(index + 1).copied())
                })
        };
        cached_step.filter(|next| self.can_enter(unit.id, start, *next))
    }

    /// Exact number of hex steps in the deterministic long-range route used by
    /// [`Self::route_step`], stopping within `stop_range` of `to`.
    ///
    /// Strategic planning needs the whole march length, not merely proof that
    /// a first step exists. Reuse the router and its cached path so campaign
    /// feasibility, staging movement, and launch estimates cannot disagree.
    pub fn route_distance(&self, uid: u32, to: Pos, stop_range: i32) -> Option<usize> {
        let unit = self.units.get(&uid)?;
        let start = unit.pos;
        let range = stop_range.max(0);
        if self.wdist_capped(start, to, range) <= range {
            return Some(0);
        }
        self.route_step(uid, to, range)?;
        let territory_access = self.unit_territory_access(unit);
        let access_key = self.route_access_key(unit, territory_access.as_slice());
        self.routing
            .borrow()
            .paths
            .iter()
            .find(|route| {
                route.unit == uid
                    && route.target == to
                    && route.range == range
                    && route.access_key == access_key
            })
            .and_then(|route| {
                route
                    .path
                    .iter()
                    .position(|position| *position == start)
                    .map(|index| route.path.len().saturating_sub(index + 1))
            })
    }

    /// Everything which can change whether this unit may cross a border
    /// without changing the map itself. Player access is already the complete
    /// shared predicate; city owners distinguish a same-turn city transfer
    /// whose tiles retain their existing owner-city ids.
    pub(super) fn route_access_key(&self, unit: &Unit, territory_access: &[bool]) -> u64 {
        let cached_city_owner_key = self.routing.borrow().city_owner_key;
        let city_owner_key = match cached_city_owner_key {
            Some(key) => key,
            None => {
                let mut key = vision_key(&[self.cities.len() as u64]);
                for city in self.cities.values() {
                    key = vision_key(&[key, city.id as u64, city.owner as u64]);
                }
                let mut routing = self.routing.borrow_mut();
                *routing.city_owner_key.get_or_insert(key)
            }
        };
        let mut key = vision_key(&[unit.owner as u64, self.players.len() as u64, city_owner_key]);
        for (owner, access) in territory_access.iter().enumerate() {
            key = vision_key(&[key, owner as u64, *access as u64]);
        }
        key
    }

    pub(super) fn invalidate_routing_paths(&self) {
        let mut routing = self.routing.borrow_mut();
        routing.paths.clear();
        routing.reverse_fields.clear();
        routing.city_owner_key = None;
    }

    /// Terrain/domain legality for a future route segment whose destination
    /// came directly from [`Self::nbrs`] for the current route tile. Dynamic
    /// unit occupancy is enforced on the returned first step; ignoring it
    /// deeper in the plan avoids expensive scans and lets moving units clear
    /// before the traveler arrives. Routes are recalculated whenever the
    /// immediate step remains blocked.
    pub(super) fn can_path_through_neighbor(
        &self,
        uid: u32,
        pos: Pos,
        territory_access: &[bool],
    ) -> bool {
        if !self.unit_can_traverse(uid, pos) {
            return false;
        }
        self.can_path_through_known_traversable(uid, pos, territory_access)
    }

    /// The dynamic half of [`Self::can_path_through_neighbor`]. The route A* already
    /// has a routing-zone label for every neighbor, so it can skip the static
    /// terrain/domain predicate on future segments and retain only these
    /// diplomacy and city gates.
    pub(super) fn can_path_through_known_traversable(
        &self,
        uid: u32,
        pos: Pos,
        territory_access: &[bool],
    ) -> bool {
        let unit = &self.units[&uid];
        if self
            .territory_owner_at(pos)
            .is_some_and(|owner| !territory_access[owner])
        {
            return false;
        }
        self.city_at(pos)
            .map(|cid| self.cities[&cid].owner == unit.owner)
            .unwrap_or(true)
    }

    /// Whether some chain of statically-traversable tiles links the unit to
    /// a tile within `range` of `to`. `false` is proof that no route exists;
    /// `true` still leaves per-step rules for the search to verify. The
    /// answer is a lookup in a cached region labeling, which is what lets
    /// the router refuse a target across an unreachable strait without
    /// flooding the map to find that out, every call, for every unit.
    fn zone_connected(&self, uid: u32, to: Pos, range: i32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let zones = self.routing_zones(self.traversal_class(uid));
        let start_zone = self.map.tiles.index_of(unit.pos).map_or(0, |i| zones[i]);
        if start_zone == 0 {
            // A unit somewhere its class says it cannot stand (a mod or a
            // scenario teleport): trust the search over the labeling.
            return true;
        }
        let in_start_zone =
            |p: Pos| self.map.tiles.index_of(p).map_or(0, |i| zones[i]) == start_zone;
        if range == 0 {
            in_start_zone(to)
        } else {
            // The route may stop on any traversable tile within range.
            self.wdisk(to, range).into_iter().any(in_start_zone)
        }
    }

    /// First step toward the nearest reachable member of `goals`. This is
    /// useful for exploration, where the geometrically nearest hidden tile
    /// may be on the far side of an impassable ridge or pre-embarkation sea.
    pub fn route_step_to_any(&self, uid: u32, goals: &HashSet<Pos>) -> Option<Pos> {
        if goals.is_empty() {
            return None;
        }
        let unit = self.units.get(&uid)?;
        let zones = self.routing_zones(self.traversal_class(uid));
        let start = unit.pos;
        let start_index = self.map.tiles.index_of(start)?;
        let start_zone = zones[start_index];
        // Goals the labeling can disprove would each cost the breadth-first
        // search below a full flood of the unit's region to disprove again.
        if start_zone != 0
            && !goals
                .iter()
                .any(|p| self.map.tiles.index_of(*p).map_or(0, |i| zones[i]) == start_zone)
        {
            return None;
        }
        // Keep the historical short-circuit: callers use a set that may also
        // contain the unit's current tile to mean "already at a valid goal".
        if goals.contains(&start) || self.formation_movement_locked_by_zoc(uid) {
            return None;
        }

        // A singleton target is usually a one-off city/ring query. Flooding
        // its whole region would cost more than the forward search that stops
        // as soon as that one target is found; reserve fields for the
        // multi-source queries that can amortize their construction across
        // units and turns within this routing epoch.
        if goals.len() < 2 {
            return self.first_route_step(uid, |position| goals.contains(&position));
        }

        // A malformed scenario can place a unit on a tile outside its static
        // traversal labeling. The reverse field deliberately labels only
        // ordinary interior tiles, so retain the old proof-producing search
        // for that exceptional starting state.
        if start_zone == 0 {
            return self.first_route_step(uid, |position| goals.contains(&position));
        }

        let _memo = self.query_memo();
        let distance = self.reverse_goal_distance(uid, goals, &zones)?;

        // The reverse field gives the distance from every interior tile to
        // its nearest goal. Read the first edge in the same neighbour order as
        // the forward BFS; among equal distances this preserves its stable
        // tie-break while avoiding a fresh flood for every unit.
        let mut best_distance = i32::MAX;
        let mut best = None;
        let map_tiles = self.map.tiles.values().as_slice();
        let neighbors = self.map.neighbor_indices(start_index);
        for &neighbor_index in neighbors.as_slice() {
            let index = neighbor_index as usize;
            let next = map_tiles[index].pos;
            if !self.can_enter_neighbor(uid, start, next) {
                continue;
            }
            let steps = distance[index];
            if steps < best_distance {
                best_distance = steps;
                best = Some(next);
            }
        }
        best
    }

    pub(super) fn first_route_step<F>(&self, uid: u32, is_goal: F) -> Option<Pos>
    where
        F: Fn(Pos) -> bool,
    {
        let unit = self.units.get(&uid)?;
        let start = unit.pos;
        if self.formation_movement_locked_by_zoc(uid) || is_goal(start) {
            return None;
        }
        let _memo = self.query_memo();
        let territory_access = self.unit_territory_access(unit);

        // Same breadth-first walk in the same neighbour order, but the map
        // already numbers its tiles, so the frontier's bookkeeping is two
        // dense vectors indexed by that number rather than hash maps keyed by
        // a hex coordinate. Every tile a unit may enter is on the map —
        // `can_enter_neighbor` and `can_path_through_neighbor` both go through
        // `unit_can_traverse`, which rejects a position the grid does not
        // hold — so an index always exists for a reachable neighbour.
        const NO_PARENT: u32 = u32::MAX;
        let tile_count = self.map.tiles.len();
        let mut scratch = self.route_scratch.borrow_mut();
        if scratch.parent.len() < tile_count {
            scratch.parent.resize(tile_count, NO_PARENT);
        }
        if scratch.seen.len() < tile_count {
            scratch.seen.resize(tile_count, false);
        }
        let touched = scratch.bfs_touched.len();
        for index in 0..touched {
            let slot = scratch.bfs_touched[index];
            scratch.parent[slot] = NO_PARENT;
            scratch.seen[slot] = false;
        }
        scratch.bfs_touched.clear();
        scratch.bfs_frontier.clear();
        let start_index = self.map.tiles.index_of(start)?;
        scratch.parent[start_index] = NO_PARENT;
        scratch.seen[start_index] = true;
        scratch.bfs_touched.push(start_index);
        scratch.bfs_frontier.push_back((start, start_index));
        let map_tiles = self.map.tiles.values().as_slice();

        let mut goal = None;
        'search: while let Some((cur, cur_index)) = scratch.bfs_frontier.pop_front() {
            let neighbors = self.map.neighbor_indices(cur_index);
            for &neighbor_index in neighbors.as_slice() {
                let index = neighbor_index as usize;
                let n = map_tiles[index].pos;
                // Settle the cheap disqualifiers before asking whether the
                // unit may enter. Every interior tile is reached as a
                // neighbour of all six of its own neighbours, so five of
                // those six arrivals find it already seen — and asking
                // `can_path_through_neighbor` first meant paying for a traversal
                // class, a passability sweep over the ruleset, a territory
                // owner and a city lookup, only to discard the answer.
                // Skipping earlier cannot change the walk: both orders
                // `continue` without touching any state.
                if scratch.seen[index] {
                    continue;
                }
                let enterable = if cur == start {
                    self.can_enter_neighbor(uid, cur, n)
                } else {
                    self.can_path_through_neighbor(uid, n, territory_access.as_slice())
                };
                if !enterable {
                    continue;
                }
                scratch.seen[index] = true;
                scratch.bfs_touched.push(index);
                scratch.parent[index] = cur_index as u32;
                if is_goal(n) {
                    goal = Some(index);
                    break 'search;
                }
                scratch.bfs_frontier.push_back((n, index));
            }
        }

        let mut step = goal?;
        while scratch.parent[step] != start_index as u32 {
            let previous = scratch.parent[step];
            if previous == NO_PARENT {
                return None;
            }
            step = previous as usize;
        }
        Some(self.map.tiles.values().as_slice()[step].pos)
    }

    /// The one movement relaxation in the engine: every tile this unit's
    /// remaining movement can occupy or cross, with what it keeps there.
    ///
    /// `passable` is the whole difference between a flood that reads the board
    /// as it stands and one that reads through what is parked on it; `V`
    /// carries the zone-of-control rule (see [`FloodArrival`]);
    /// `record_parent(tile, from)` is called on exactly the relaxations that
    /// improve a tile, so a caller that wants the walk gets the flood's own
    /// parents and a caller that does not pays nothing for them.
    ///
    /// ⚠ THE VISITATION ORDER IS PART OF THE ANSWER, not an implementation
    /// detail. The stack is LIFO and re-pushes a tile every time it improves,
    /// the neighbour order is `nbrs`, and the improvement test is a strict
    /// `>`; between them they decide which parent wins a tie, and therefore
    /// which of several equally long walks `path_to` returns. Two floods can
    /// agree on every distance and still hand back different paths. Change any
    /// of the three and the engine plays a different game.
    ///
    /// The caller opens the [`Game::query_memo`] scope and applies its own
    /// guards (`formation_movement_locked_by_zoc`) before calling: the three
    /// callers return different empty answers, and two of them read the flood's
    /// parents after it, still inside their scope.
    pub(super) fn relax_movement_into<V, P, R, O>(
        &self,
        uid: u32,
        start: Pos,
        moves: f64,
        passable: P,
        mut record_parent: R,
        output: &mut O,
    ) where
        V: FloodArrival,
        P: Fn(Pos, Pos) -> bool,
        R: FnMut(Pos, Pos),
        O: MovementFloodOutput<V>,
    {
        let max_moves = self.unit_max_moves(uid);
        // Both of these ask the unit, never the tile, and the loop below asks
        // them once per neighbour of every tile it expands — see
        // `Game::unit_moves_cap` and `Game::formation_zoc_test`.
        let cap = self.unit_moves_cap(uid);
        let zoc = self.formation_zoc_test(uid);
        const UNSEEN: f64 = f64::NEG_INFINITY;
        let tile_count = self.map.tiles.len();
        let mut scratch = self.route_scratch.borrow_mut();
        if scratch.movement_seen.len() < tile_count {
            scratch.movement_seen.resize(tile_count, false);
            scratch.movement_score.resize(tile_count, UNSEEN);
            scratch.movement_stopped.resize(tile_count, false);
            scratch.movement_zoc.resize(tile_count, None);
        }
        let touched = scratch.movement_touched.len();
        for index in 0..touched {
            let slot = scratch.movement_touched[index];
            scratch.movement_seen[slot] = false;
            scratch.movement_score[slot] = UNSEEN;
            scratch.movement_stopped[slot] = false;
            scratch.movement_zoc[slot] = None;
        }
        scratch.movement_touched.clear();
        scratch.movement_frontier.clear();
        let start_index =
            self.map.tiles.index_of(start).unwrap_or_else(|| {
                panic!("movement flood started outside the world map at {start:?}")
            });
        scratch.movement_seen[start_index] = true;
        scratch.movement_score[start_index] = moves;
        scratch.movement_touched.push(start_index);
        scratch.movement_frontier.push(start_index);
        let map_tiles = self.map.tiles.values().as_slice();
        while let Some(cur_index) = scratch.movement_frontier.pop() {
            let cur = map_tiles[cur_index].pos;
            let rem = scratch.movement_score[cur_index];
            let stopped = scratch.movement_stopped[cur_index];
            if rem <= 0.0 || stopped {
                continue;
            }
            let neighbors = self.map.neighbor_indices(cur_index);
            for &neighbor_index in neighbors.as_slice() {
                let neighbor_index = neighbor_index as usize;
                let n = map_tiles[neighbor_index].pos;
                let index = neighbor_index;
                // ⭐ SKIP THE ARRIVALS THAT CANNOT WIN, BEFORE PAYING FOR THEM.
                // An arrival never carries more movement than the tile it came
                // from: `unit_step_cost` ends `cost.max(0.0)` and asserts that
                // "a step never grants movement", `(rem - cost).max(0.0)` is at
                // most `rem` because `rem > 0` is already guaranteed above,
                // `.min(capped_moves_at(..))` only lowers it, and every
                // `FloodArrival::arrival` either keeps that value or zeroes it
                // for a zone of control. So `score <= rem`, and a tile already
                // holding `rem` or better can never lose the strict `>` test
                // below.
                //
                // Every interior tile is reached from all six of its neighbours
                // and re-pushed on each improvement, so roughly half of those
                // arrivals come from a tile nearer the source and are discarded
                // — after paying `passable`, `unit_step_cost`, `capped_moves_at`
                // and the zone-of-control walk. `in_enemy_zoc_for` alone visits
                // all six neighbours of the arrival, every unit standing on
                // them, and a `zone_of_control` effect lookup per neighbour.
                //
                // ⚠ This skips only where the improvement test is PROVABLY
                // false, so it writes nothing the test below would have written
                // and pushes nothing it would have pushed: the LIFO order, the
                // `nbrs` order and the strict `>` that together decide which
                // parent `path_to` returns are all untouched. The sibling BFS
                // above already skips on the same argument. Do not weaken `>=`
                // to `>` here — equal scores must still fall through to the
                // test, which rejects them.
                if scratch.movement_seen[index] && scratch.movement_score[index] >= rem {
                    continue;
                }
                if !passable(cur, n) {
                    continue;
                }
                let cost = self.unit_step_cost(uid, cur, n);
                let fresh = cur == start && rem >= max_moves;
                if rem < cost && !fresh {
                    continue; // MP paid up front (Civ 6)
                }
                let new_rem = (rem - cost)
                    .max(0.0)
                    .min(self.capped_moves_at(uid, &cap, n));
                // Entry may be reconsidered from several neighbors, especially
                // when ZOC zeroes an earlier arrival. Only the remaining
                // movement depends on the route: this predicate reads the
                // fixed formation, destination and board. Its result expires
                // with this flood, using the same touched list as its scores.
                let enters_zoc = if let Some(cached) = scratch.movement_zoc[index] {
                    debug_assert_eq!(cached, self.enters_enemy_zoc(&zoc, n));
                    cached
                } else {
                    let value = self.enters_enemy_zoc(&zoc, n);
                    scratch.movement_zoc[index] = Some(value);
                    value
                };
                let arrival = V::arrival(new_rem, enters_zoc);
                // ⚠ The zone-of-control rule is applied BEFORE the improvement
                // test, not after it: `flow_past` and `path_to` compare the
                // zeroed value, so a second arrival at a tile inside a zone of
                // control never improves on the first and never re-parents it.
                // Comparing the pre-ZOC movement instead leaves every distance
                // identical and hands `path_to` a different walk to a
                // destination under a zone of control.
                let score = arrival.remaining();
                // ⭐ THE SKIP ABOVE IS AN ARITHMETIC ARGUMENT, SO CHECK IT.
                // The guard at the top of this body drops an arrival whose tile
                // already holds `rem` or better, on the grounds that no arrival
                // can carry more movement than the tile it came from:
                // `unit_step_cost` ends `cost.max(0.0)`, `rem > 0` is guaranteed
                // above, `(rem - cost).max(0.0) <= rem`, `.min(capped_moves_at)`
                // only lowers it, and both `FloodArrival::arrival` impls keep
                // that value or zero it for a zone of control. Nothing in the
                // suite tested that chain -- it held by reading, and a later
                // change to any link would silently turn the skip into a
                // wrong answer that only a paired A/B would notice.
                //
                // `[profile.ci]` sets `debug-assertions = true` precisely so
                // guards like this run in the build that gates a merge, so this
                // is a real check on every one of the ~3,200 tests and costs
                // nothing in `release`.
                debug_assert!(
                    score <= rem,
                    "movement arrival at {n:?} kept {score} of the {rem} it \
                     came from: a step granted movement, which makes the \
                     unimprovable-arrival skip above unsound"
                );
                if !scratch.movement_seen[index] || score > scratch.movement_score[index] {
                    if !scratch.movement_seen[index] {
                        scratch.movement_seen[index] = true;
                        scratch.movement_touched.push(index);
                    }
                    scratch.movement_score[index] = score;
                    scratch.movement_stopped[index] = arrival.stopped();
                    record_parent(n, cur);
                    scratch.movement_frontier.push(index);
                }
            }
        }
        output.reserve_reached(scratch.movement_touched.len());
        for &index in &scratch.movement_touched {
            let value = if index == start_index {
                V::start(moves)
            } else {
                V::recorded(
                    scratch.movement_score[index],
                    scratch.movement_stopped[index],
                )
            };
            output.push_reached(map_tiles[index].pos, value);
        }
    }

    pub(super) fn relax_movement<V, P, R>(
        &self,
        uid: u32,
        start: Pos,
        moves: f64,
        passable: P,
        record_parent: R,
    ) -> BTreeMap<Pos, V>
    where
        V: FloodArrival,
        P: Fn(Pos, Pos) -> bool,
        R: FnMut(Pos, Pos),
    {
        let mut best = BTreeMap::new();
        self.relax_movement_into(uid, start, moves, passable, record_parent, &mut best);
        best
    }

    /// This turn's cheapest legal path to `to`, or `None` when the unit cannot
    /// finish there this turn.
    ///
    /// Intermediate hexes may hold the unit's own units — Civilization VI lets
    /// a unit walk through its own column and forbids only ending stacked, see
    /// [`Game::entry_at`] — so the returned path is a sequence of steps to
    /// execute together, not a list of places the unit may be left standing.
    /// `Action::MoveTo` is the only thing that may execute it.
    pub(crate) fn path_to(&self, uid: u32, to: Pos) -> Option<Vec<Pos>> {
        let (start, moves) = {
            let u = self.units.get(&uid)?;
            (u.pos, u.moves_left)
        };
        if start == to {
            return Some(vec![]);
        }
        if !self.can_stop(uid, to) {
            return None;
        }
        // MoveTo is also the protocol used for an AI route's already chosen
        // adjacent step. Do not flood the unit's whole remaining movement
        // area merely to rediscover that one-edge path.
        if self.wdist_capped(start, to, 1) == 1 {
            return self.can_move(uid, to).then_some(vec![to]);
        }
        if self.formation_movement_locked_by_zoc(uid) {
            return None;
        }
        let _memo = self.query_memo();
        let mut parent: BTreeMap<Pos, Pos> = BTreeMap::new();
        // Run for its parents: `f64` is the zone-of-control rule `flow` uses,
        // which is what this path has always been read against.
        let _reach: BTreeMap<Pos, f64> = self.relax_movement(
            uid,
            start,
            moves,
            |cur, n| self.can_pass_neighbor(uid, cur, n),
            |n, cur| {
                parent.insert(n, cur);
            },
        );
        parent.get(&to)?;
        let mut path = vec![to];
        let mut cur = to;
        while let Some(p) = parent.get(&cur) {
            if *p == start {
                break;
            }
            path.push(*p);
            cur = *p;
        }
        path.reverse();
        Some(path)
    }

    /// Every tile this unit can end a move on this turn, with the movement it
    /// keeps THERE for a blow, and the path that gets it there.
    ///
    /// The same flood as [`Game::reachable`] with one difference that matters
    /// to a planner: entering enemy zone of control still stops further
    /// movement (the flood does not expand past such a tile), but the
    /// movement the unit keeps on arrival is reported rather than zeroed —
    /// `flow` writes 0 there, which is right for "can it move on" and wrong
    /// for "can it still strike", since a unit that stops in a zone of
    /// control keeps its unused movement for the attack (`do_attack`,
    /// `do_ranged`, `can_pay_melee_entry`). Paths are the flood's own
    /// parents, so every step is one the engine will accept in sequence.
    ///
    /// A reading for candidate generation, never a permission: the engine
    /// re-decides every step when the line is played.
    ///
    /// ⚠ A path may cross tiles held by this unit's own units — Civilization
    /// VI forbids ending stacked, not passing through, see [`Game::entry_at`]
    /// — so a path is a single order and not a list of places to leave the
    /// unit. Replay one with `Action::MoveTo`; walking it with one `Move` per
    /// step stops dead at the first friendly tile.
    pub(crate) fn approach_reach(&self, uid: u32) -> BTreeMap<Pos, (f64, Vec<Pos>)> {
        let mut out = BTreeMap::new();
        let Some(unit) = self.units.get(&uid) else {
            return out;
        };
        let (start, moves) = (unit.pos, unit.moves_left);
        if moves <= 0.0 || self.formation_movement_locked_by_zoc(uid) {
            return out;
        }
        let _memo = self.query_memo();
        // Per tile: movement kept on arrival, and whether the arrival stopped
        // the unit (zone of control) so nothing expands from it.
        let mut parent: BTreeMap<Pos, Pos> = BTreeMap::new();
        let best: BTreeMap<Pos, (f64, bool)> = self.relax_movement(
            uid,
            start,
            moves,
            |cur, n| self.can_pass_neighbor(uid, cur, n),
            |n, cur| {
                parent.insert(n, cur);
            },
        );
        for (pos, (rem, _)) in best {
            // The flood crosses our own column and never ends on it, so a
            // stand behind a friendly unit is a step on the way and not a
            // candidate. See `Game::entry_at`.
            if pos == start || !self.can_stop(uid, pos) {
                continue;
            }
            let mut path = vec![pos];
            let mut cur = pos;
            while let Some(p) = parent.get(&cur) {
                if *p == start {
                    break;
                }
                path.push(*p);
                cur = *p;
            }
            path.reverse();
            out.insert(pos, (rem, path));
        }
        out
    }

    /// Attack arrival movement without materializing a path for each stand.
    /// Uses the same flood and stopping gates as `approach_reach`. Discovery
    /// order is private to readers that sort their resulting target set.
    pub(crate) fn approach_arrivals(&self, uid: u32) -> Vec<(Pos, f64)> {
        let Some(unit) = self.units.get(&uid) else {
            return Vec::new();
        };
        let (start, moves) = (unit.pos, unit.moves_left);
        if moves <= 0.0 || self.formation_movement_locked_by_zoc(uid) {
            return Vec::new();
        }
        let _memo = self.query_memo();
        let mut arrivals: Vec<(Pos, (f64, bool))> = Vec::new();
        self.relax_movement_into(
            uid,
            start,
            moves,
            |cur, next| self.can_pass_neighbor(uid, cur, next),
            |_, _| {},
            &mut arrivals,
        );
        arrivals
            .into_iter()
            .filter(|(pos, _)| *pos != start && self.can_stop(uid, *pos))
            .map(|(pos, (kept, _))| (pos, kept))
            .collect()
    }

    /// Where to walk toward `target` when the way forward is blocked by
    /// nothing but this unit's own units.
    ///
    /// The cheap gate first: unless some neighbour is a tile this unit may
    /// cross and may not stand on, there is no crossing to look for — and that
    /// is exactly one of ours on our own stacking layer, because every other
    /// refusal ([`Game::entry_at`]) blocks the crossing too. Past that gate the
    /// answer is the reachable tile that most improves the distance to
    /// `target` and whose walk actually crosses one of ours, which is what
    /// keeps this from quietly becoming a second mover competing with an
    /// ordinary march. Execute it with `Action::MoveTo`, the only action that
    /// may cross.
    ///
    /// Both a controller and the browser's travel order need this, and for the
    /// same reason: a router that only ever offers a tile the unit may stand
    /// on has nothing to say about a column in a defile.
    ///
    /// Deliberately unbounded: a caller reaches this only when every ordinary
    /// step has already been refused, so the unit is boxed in and its
    /// reachable set is small. A cap here would silently drop the far end of a
    /// long column.
    pub fn pass_through_destination(&self, uid: u32, target: Pos, stop_range: i32) -> Option<Pos> {
        let cur = self.units.get(&uid)?.pos;
        let here = self.wdist(cur, target);
        if here <= stop_range {
            return None;
        }
        if !self
            .nbrs(cur)
            .into_iter()
            .any(|n| self.can_pass_neighbor(uid, cur, n) && !self.can_stop(uid, n))
        {
            return None;
        }
        let mut candidates: Vec<(i32, Pos)> = self
            .reachable(uid)
            .into_iter()
            .map(|pos| (self.wdist(pos, target), pos))
            .filter(|(distance, _)| *distance < here && *distance >= stop_range)
            .collect();
        candidates.sort_unstable();
        candidates
            .into_iter()
            .find(|(_, pos)| {
                self.path_to(uid, *pos)
                    .is_some_and(|path| path.iter().any(|step| !self.can_stop(uid, *step)))
            })
            .map(|(_, pos)| pos)
    }

    /// The movement a unit would pay to enter `to` from `from` — the exact
    /// preflight a melee blow needs after a move (`can_pay_melee_entry`).
    pub(crate) fn step_cost_for(&self, uid: u32, from: Pos, to: Pos) -> f64 {
        self.unit_step_cost(uid, from, to)
    }

    /// All tiles the unit can reach this turn with its remaining movement
    /// (Dijkstra maximizing leftover MP; every intermediate tile must be
    /// legally crossable, and the destination legally stoppable).
    ///
    /// A tile occupied by one of this unit's own units on the same stacking
    /// layer is crossed, never offered: the flood expands through it and
    /// [`Game::can_stop`] then drops it. That is the Civilization VI rule, and
    /// it is why this is not simply the flood's key set — see
    /// [`Game::entry_at`].
    pub fn reachable(&self, uid: u32) -> Vec<Pos> {
        let (start, moves) = match self.units.get(&uid) {
            Some(u) => (u.pos, u.moves_left),
            None => return vec![],
        };
        let best = self.flow(uid, start, moves);
        best.into_keys()
            .filter(|p| *p != start && self.can_stop(uid, *p))
            .collect()
    }

    /// How far a unit reaches: everywhere it could stand at the end of a whole
    /// turn's movement, read as though nothing were parked in the way.
    ///
    /// Two deliberate differences from [`Game::reachable`], which answers what
    /// a unit's owner may legally do right now.
    ///
    /// It spends a *full* allowance rather than what is left. Outside its own
    /// turn a unit has no movement points at all, so the honest answer to
    /// "what can it do this instant" is almost always nothing — which is not
    /// the question anyone points at an enemy to ask. The threat a Knight
    /// poses does not switch off between its turns, and a reading that went
    /// blank for every unit except the one currently acting would be empty
    /// nearly all the time in the only mode that has no acting seat at all.
    ///
    /// And it reads through units. Whatever is standing on a tile can walk off
    /// it or die on it well before the threat expires, so for this question it
    /// is noise — while zone of control, terrain, cliffs, rivers, borders and
    /// hostile cities all stay exactly as binding as they are for a real move,
    /// because none of those move out of the way.
    ///
    /// This is a reading and never a permission. Nothing in the simulation may
    /// move a unit by it: a step onto an occupied tile is illegal and asking
    /// this does not make it legal.
    pub fn threat_reach(&self, uid: u32) -> Vec<Pos> {
        let Some(start) = self.units.get(&uid).map(|u| u.pos) else {
            return vec![];
        };
        let best = self.flow_past(uid, start, self.unit_max_moves(uid), true);
        best.into_keys().filter(|p| *p != start).collect()
    }

    /// Every legal single step inside this turn's remaining movement, as
    /// ordered `(from, to)` pairs: a step is listed when some legal move
    /// sequence this turn arrives at `from` with enough movement left to
    /// enter `to`. The per-edge companion to [`Game::reachable`] — it shows
    /// *how* the range is entered, not only where it ends. Direction matters:
    /// stepping into enemy ZOC ends movement, so the reverse step can be
    /// absent while the forward one is offered.
    ///
    /// Every pair here is a legal `Action::Move` on its own, which is the
    /// contract a client draws from. A tile held by one of this unit's own
    /// units is therefore neither an endpoint nor an origin: the flood crosses
    /// it, and the unit is never standing on it to take a step from. The whole
    /// crossing is one `Action::MoveTo` and appears in [`Game::path_to`]
    /// instead — see [`Game::entry_at`].
    pub fn reach_steps(&self, uid: u32) -> Vec<(Pos, Pos)> {
        let (start, moves) = match self.units.get(&uid) {
            Some(u) => (u.pos, u.moves_left),
            None => return vec![],
        };
        let max_moves = self.unit_max_moves(uid);
        if self.formation_movement_locked_by_zoc(uid) {
            return vec![];
        }
        let _memo = self.query_memo();
        let best = self.flow(uid, start, moves);
        let mut steps = Vec::new();
        for (&from, &rem) in &best {
            if rem <= 0.0 {
                continue;
            }
            // The flood crosses tiles held by our own units and never stands
            // on one, so a step *out of* one is not a single move anybody can
            // make. Every pair advertised here is a legal `Move` on its own.
            if from != start && !self.can_stop(uid, from) {
                continue;
            }
            let Some(from_index) = self.map.tiles.index_of(from) else {
                continue;
            };
            let neighbors = self.map.neighbor_indices(from_index);
            let map_tiles = self.map.tiles.values().as_slice();
            for &neighbor_index in neighbors.as_slice() {
                let to = map_tiles[neighbor_index as usize].pos;
                if !self.can_enter_neighbor(uid, from, to) {
                    continue;
                }
                let cost = self.unit_step_cost(uid, from, to);
                let fresh = from == start && rem >= max_moves;
                if rem < cost && !fresh {
                    continue; // MP paid up front (Civ 6), same rule as `flow`
                }
                steps.push((from, to));
            }
        }
        steps
    }

    /// Refresh the shared routing epoch and discard derived paths when a
    /// world write has made their static assumptions stale.
    pub(super) fn refresh_routing_cache_stamp(&self) -> u64 {
        // Cities grant naval passage but live beside the map, so fold their
        // count in with the tile epoch; the turn bounds any residual
        // staleness (a found-and-razed pair the same turn) to one turn.
        let stamp = vision_key(&[
            self.map.tiles.epoch(),
            self.turn as u64,
            self.cities.len() as u64,
        ]);
        let mut cache = self.routing.borrow_mut();
        if cache.stamp != stamp {
            cache.stamp = stamp;
            cache.zones.clear();
            cache.paths.clear();
            cache.reverse_fields.clear();
            cache.city_owner_key = None;
        }
        stamp
    }

    /// The region labeling for `class`, rebuilt lazily whenever the world
    /// stamp moves and shared by every unit of the class until then.
    pub(super) fn routing_zones(&self, class: TraversalClass) -> std::sync::Arc<Vec<u32>> {
        let stamp = self.refresh_routing_cache_stamp();
        {
            let cache = self.routing.borrow();
            if let Some((_, zones)) = cache.zones.iter().find(|(c, _)| *c == class) {
                return zones.clone();
            }
        }
        // Built outside the borrow: the flood only reads map and rules.
        let zones = std::sync::Arc::new(self.build_routing_zones(class));
        let mut cache = self.routing.borrow_mut();
        if cache.stamp == stamp && !cache.zones.iter().any(|(c, _)| *c == class) {
            cache.zones.push((class, zones.clone()));
        }
        zones
    }

    /// Label every tile with its connected region under `class`, 0 for
    /// impassable. A breadth-first flood in map order keeps labels
    /// deterministic, though only label *equality* ever matters.
    fn build_routing_zones(&self, class: TraversalClass) -> Vec<u32> {
        // ⚠ #2309 moved `improvements[name].effects["passage"]` behind
        // `Game::passage_improvements`, which builds a table over the whole
        // improvement list and keeps it for the memo scope — and *rebuilds it
        // per call* when there is no scope to keep it in. That is the right
        // trade for the movement flood, which always has one, and the wrong
        // one here: this sweep asks `class_can_traverse` for every tile on the
        // map with no scope open, so an improved tile paid a lookup per
        // improvement in the ruleset instead of one. Opening a scope over the
        // sweep is all it needs; the guard borrows the world immutably, so the
        // flood below cannot see a table it did not build.
        let _memo = self.query_memo();
        let mut zones = vec![0u32; self.map.tiles.len()];
        // The flood visits a tile once as a seed or neighbor and may inspect
        // it many more times from adjacent frontier tiles. Evaluate the
        // static terrain/domain predicate once per tile, then let the BFS
        // read the dense result. This preserves map-order labels while
        // avoiding repeated ruleset and city-passage lookups on Ludicrous
        // maps.
        let traversable: Vec<u8> = self
            .map
            .tiles
            .values()
            .map(|tile| self.class_can_traverse(class, tile) as u8)
            .collect();
        let mut label = 0u32;
        let mut queue = VecDeque::new();
        for index in 0..self.map.tiles.len() {
            if zones[index] != 0 {
                continue;
            }
            if traversable[index] == 0 {
                continue;
            }
            label += 1;
            zones[index] = label;
            queue.push_back(index);
            while let Some(cur_index) = queue.pop_front() {
                let neighbors = self.map.neighbor_indices(cur_index);
                for &neighbor_index in neighbors.as_slice() {
                    let slot = neighbor_index as usize;
                    if zones[slot] == 0 && traversable[slot] != 0 {
                        zones[slot] = label;
                        queue.push_back(slot);
                    }
                }
            }
        }
        zones
    }

    /// The reverse route field `route_step_to_any` walks: for every tile, the
    /// number of route steps from it to the nearest of `goals` (`i32::MAX`
    /// where none is reachable), cached per traversal class, territory access
    /// and goal set within the routing epoch.
    pub(super) fn reverse_goal_distance(
        &self,
        uid: u32,
        goals: &HashSet<Pos>,
        zones: &[u32],
    ) -> Option<Arc<Vec<i32>>> {
        let _memo = self.query_memo();
        let unit = self.units.get(&uid)?;
        let territory_access = self.unit_territory_access(unit);
        let access_key = self.route_access_key(unit, territory_access.as_slice());
        let class = self.traversal_class(uid);
        let mut ordered_goals: Vec<Pos> = goals.iter().copied().collect();
        ordered_goals.sort_unstable();
        let goals = Arc::new(ordered_goals);
        let stamp = self.refresh_routing_cache_stamp();

        let cached = {
            let routing = self.routing.borrow();
            routing
                .reverse_fields
                .iter()
                .find(|field| {
                    field.class == class
                        && field.access_key == access_key
                        && field.goals.as_slice() == goals.as_slice()
                })
                .map(|field| field.distance.clone())
        };
        let distance = match cached {
            Some(distance) => distance,
            None => {
                let distance = Arc::new(self.build_reverse_flow_field(
                    uid,
                    class,
                    territory_access.as_slice(),
                    goals.as_slice(),
                    zones,
                ));
                let mut routing = self.routing.borrow_mut();
                if routing.stamp == stamp {
                    routing.reverse_fields.retain(|field| {
                        !(field.class == class
                            && field.access_key == access_key
                            && field.goals.as_slice() == goals.as_slice())
                    });
                    if routing.reverse_fields.len() >= MAX_REVERSE_FLOW_FIELDS {
                        routing.reverse_fields.remove(0);
                    }
                    routing.reverse_fields.push(ReverseFlowField {
                        class,
                        access_key,
                        goals,
                        distance: distance.clone(),
                    });
                }
                distance
            }
        };
        Some(distance)
    }

    /// `march-uses-its-moves`: every tile `uid` can stop on this turn that
    /// has a route to `goals`, with the route steps from it to the nearest
    /// goal (read on the reverse field [`Game::route_step_to_any`] walks) and
    /// the movement left on arriving, nearest first, then most movement left,
    /// then position. The first value is the start tile's own step count.
    ///
    /// `None` wherever `route_step_to_any` declines before reading its field,
    /// for a unit that moves as a formation, and for one without movement.
    #[allow(clippy::type_complexity)]
    pub(crate) fn march_reach_toward_any(
        &self,
        uid: u32,
        goals: &HashSet<Pos>,
    ) -> Option<(i32, Vec<(Pos, i32, f64)>)> {
        if goals.is_empty() {
            return None;
        }
        let unit = self.units.get(&uid)?;
        if unit.linked_to.is_some() || unit.moves_left <= 0.0 {
            return None;
        }
        let zones = self.routing_zones(self.traversal_class(uid));
        let start = unit.pos;
        let start_index = self.map.tiles.index_of(start)?;
        let start_zone = zones[start_index];
        if start_zone == 0
            || goals.contains(&start)
            || self.formation_movement_locked_by_zoc(uid)
            || !goals
                .iter()
                .any(|p| self.map.tiles.index_of(*p).map_or(0, |i| zones[i]) == start_zone)
        {
            return None;
        }
        let _memo = self.query_memo();
        let distance = self.reverse_goal_distance(uid, goals, &zones)?;
        let start_steps = distance[start_index];
        if start_steps == i32::MAX {
            return None;
        }
        let mut reach: Vec<(Pos, i32, f64)> = self
            .flow(uid, start, unit.moves_left)
            .into_iter()
            .filter(|(pos, _)| *pos != start && self.can_stop(uid, *pos))
            .filter_map(|(pos, left)| {
                let steps = distance[self.map.tiles.index_of(pos)?];
                (steps != i32::MAX).then_some((pos, steps, left))
            })
            .collect();
        reach.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.2.total_cmp(&a.2))
                .then_with(|| a.0.cmp(&b.0))
        });
        Some((start_steps, reach))
    }

    /// Build a multi-source distance field by walking route edges backwards
    /// from every valid goal. Interior route edges are symmetric: after the
    /// static traversal zone has accepted a tile, the future-segment gate is
    /// only territory/city ownership on the destination. The live unit still
    /// validates its selected first edge with `can_enter` below.
    fn build_reverse_flow_field(
        &self,
        uid: u32,
        class: TraversalClass,
        territory_access: &[bool],
        goals: &[Pos],
        zones: &[u32],
    ) -> Vec<i32> {
        const UNREACHED: i32 = i32::MAX;
        let mut distance = vec![UNREACHED; self.map.tiles.len()];
        let map_tiles = self.map.tiles.values().as_slice();
        let mut queue = VecDeque::new();
        for &goal in goals {
            let Some(index) = self.map.tiles.index_of(goal) else {
                continue;
            };
            if zones[index] == 0
                || distance[index] != UNREACHED
                || !self.can_path_through_known_traversable(uid, goal, territory_access)
            {
                continue;
            }
            distance[index] = 0;
            queue.push_back(index);
        }

        while let Some(current_index) = queue.pop_front() {
            let next_distance = distance[current_index].saturating_add(1);
            let neighbors = self.map.neighbor_indices(current_index);
            for &neighbor_index in neighbors.as_slice() {
                let index = neighbor_index as usize;
                let predecessor = map_tiles[index].pos;
                if distance[index] != UNREACHED || zones[index] == 0 {
                    continue;
                }
                // `class` is passed explicitly so a field cannot silently use
                // a changed unit traversal answer while its owning memo is
                // still active. The zone already came from this class; the
                // assertion documents that invariant for debug builds.
                debug_assert!(self.class_can_traverse(class, &self.map.tiles[&predecessor]));
                if !self.can_path_through_known_traversable(uid, predecessor, territory_access) {
                    continue;
                }
                distance[index] = next_distance;
                queue.push_back(index);
            }
        }
        distance
    }

    pub(super) fn flow(&self, uid: u32, start: Pos, moves: f64) -> BTreeMap<Pos, f64> {
        self.flow_past(uid, start, moves, false)
    }

    /// `flow`, optionally reading the board as if the tiles in the way were
    /// empty. See [`Game::entry_at`]: nothing the rules decide uses that,
    /// only the range a watcher is shown for somebody else's unit.
    ///
    /// ⚠ The result is every tile this movement can **occupy or cross**, not
    /// every tile it may finish on: a tile held by one of the unit's own units
    /// on the same stacking layer is walked through and never landed on. Ask
    /// [`Game::can_stop`] before offering one of these as a destination —
    /// [`Game::reachable`] is the filtered form.
    pub(super) fn flow_past_into<O>(
        &self,
        uid: u32,
        start: Pos,
        moves: f64,
        through_units: bool,
        output: &mut O,
    ) where
        O: MovementFloodOutput<f64>,
    {
        if self.formation_movement_locked_by_zoc(uid) {
            return;
        }
        let _memo = self.query_memo();
        self.relax_movement_into(
            uid,
            start,
            moves,
            |cur, n| self.entry_at_neighbor(uid, cur, n, through_units) != Entry::Blocked,
            |_, _| {}, // nobody asks this flood for the walk
            output,
        );
    }

    pub(super) fn flow_past(
        &self,
        uid: u32,
        start: Pos,
        moves: f64,
        through_units: bool,
    ) -> BTreeMap<Pos, f64> {
        let mut best = BTreeMap::new();
        self.flow_past_into(uid, start, moves, through_units, &mut best);
        best
    }
}
