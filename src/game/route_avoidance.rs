//! Exceptional routing around temporarily unavailable tiles.
use super::Game;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

impl Game {
    /// Shortest tile route excluding a known unavailable tile. Like ordinary
    /// routing, the first edge checks current occupancy and later edges check
    /// terrain, territory access and city entry. Do not populate the ordinary
    /// route cache: its key does not include the temporary exclusion.
    pub(crate) fn route_step_avoiding(&self, uid: u32, target: Pos, avoid: Pos) -> Option<Pos> {
        self.route_step_avoiding_tiles(uid, target, &BTreeSet::from([avoid]))
    }

    /// Route around a formation's occupied or reserved tiles without changing
    /// the ordinary route cache or relaxing the engine's entry rules.
    pub(crate) fn route_step_avoiding_tiles(
        &self,
        uid: u32,
        target: Pos,
        avoid: &BTreeSet<Pos>,
    ) -> Option<Pos> {
        let unit = self.units.get(&uid)?;
        let start = unit.pos;
        if start == target || avoid.contains(&target) || self.formation_movement_locked_by_zoc(uid)
        {
            return None;
        }
        let access = self.unit_territory_access(unit);
        if !self.can_path_through_neighbor(uid, target, &access) {
            return None;
        }
        let _memo = self.query_memo();
        let mut seen = avoid.clone();
        seen.insert(start);
        let mut queue = VecDeque::from([(start, start)]);
        while let Some((current, first)) = queue.pop_front() {
            for next in self.nbrs(current) {
                if seen.contains(&next) {
                    continue;
                }
                let allowed = if current == start {
                    self.can_enter_neighbor(uid, current, next)
                } else {
                    self.can_path_through_neighbor(uid, next, &access)
                };
                if !allowed {
                    continue;
                }
                seen.insert(next);
                let first = if current == start { next } else { first };
                if next == target {
                    return Some(first);
                }
                queue.push_back((next, first));
            }
        }
        None
    }

    /// The first step and length of the shortest route over dry land to a
    /// tile within `range` of `target`, no longer than `max_len` steps. The
    /// ordinary router prices every step at 1, so a land unit that can embark
    /// crosses any bay that is shorter in tiles; in the host an embark spends
    /// the turn, an embarked unit is slow and defenceless, and moves out of
    /// the water are often refused. Like ordinary routing, the first edge
    /// checks current occupancy and later edges check terrain, territory
    /// access and city entry. No route cache is written.
    pub(crate) fn route_step_dry(
        &self,
        uid: u32,
        target: Pos,
        range: i32,
        max_len: usize,
    ) -> Option<(Pos, usize)> {
        let unit = self.units.get(&uid)?;
        let start = unit.pos;
        if self.wdist(start, target) <= range || self.formation_movement_locked_by_zoc(uid) {
            return None;
        }
        let access = self.unit_territory_access(unit);
        let _memo = self.query_memo();
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([(start, start, 0usize)]);
        while let Some((current, first, steps)) = queue.pop_front() {
            if steps >= max_len {
                continue;
            }
            for next in self.nbrs(current) {
                if seen.contains(&next)
                    || self
                        .map
                        .get(next)
                        .is_none_or(|tile| self.rules.is_water(tile))
                {
                    continue;
                }
                let allowed = if current == start {
                    self.can_enter_neighbor(uid, current, next)
                } else {
                    self.can_path_through_neighbor(uid, next, &access)
                };
                if !allowed {
                    continue;
                }
                seen.insert(next);
                let first = if current == start { next } else { first };
                if self.wdist(next, target) <= range {
                    return Some((first, steps + 1));
                }
                queue.push_back((next, first, steps + 1));
            }
        }
        None
    }

    /// A dry road [`Self::route_step_dry`] cannot start this turn: the length
    /// of the dry road, no longer than `max_len`, from `uid` to a tile within
    /// `range` of `target`, and whether an enemy zone of control holds the
    /// unit (else every dry neighbour refuses it as a stop, one of our own
    /// units standing there). `None` when `route_step_dry` finds a first step,
    /// when the unit already stands within `range`, and when no dry road
    /// exists even through occupied tiles: only that last case is a water
    /// hold. `route_step_dry` reads its first edge as a stop and returns
    /// nothing for a unit the zone of control has locked, so a gun behind a
    /// friend on the one land exit, or beside an enemy, read as cut off by
    /// water: 65 of 90 distinct "holds on land short of the water" rows of
    /// the 10-13 live Immortal runs had a dry road for a gun of that kind.
    pub(crate) fn dry_road_blocked_now(
        &self,
        uid: u32,
        target: Pos,
        range: i32,
        max_len: usize,
    ) -> Option<(usize, bool)> {
        let start = self.units.get(&uid)?.pos;
        if self.wdist(start, target) <= range
            || self.route_step_dry(uid, target, range, max_len).is_some()
        {
            return None;
        }
        let steps = *self
            .dry_road_steps(uid, target, range, max_len)
            .get(&start)?;
        Some((steps, self.formation_movement_locked_by_zoc(uid)))
    }

    /// The steps over dry land from each tile to a tile within `range` of
    /// `target`, as `uid` would walk them, at most `max_len` steps out: a
    /// breadth-first flood back from the dry tiles within `range`. Terrain,
    /// territory access and city entry are read as [`Self::route_step_dry`]
    /// reads them past its first edge; occupancy is not read at all, the
    /// field being a road map rather than this turn's route. A tile with no
    /// dry road is absent. No route cache is written.
    /// Whether `uid` may plan a road through `pos`: its terrain and domain
    /// (water for an embarking land unit), its owner's territory access, and
    /// no city but its owner's. Occupancy is not read. One territory reading
    /// serves every tile asked.
    pub(crate) fn road_plan_passable(&self, uid: u32) -> impl Fn(Pos) -> bool + '_ {
        let access = self
            .units
            .get(&uid)
            .map(|unit| self.unit_territory_access(unit));
        move |pos| {
            access
                .as_ref()
                .is_some_and(|access| self.can_path_through_neighbor(uid, pos, access))
        }
    }

    pub(crate) fn dry_road_steps(
        &self,
        uid: u32,
        target: Pos,
        range: i32,
        max_len: usize,
    ) -> BTreeMap<Pos, usize> {
        let mut steps = BTreeMap::new();
        let Some(unit) = self.units.get(&uid) else {
            return steps;
        };
        let access = self.unit_territory_access(unit);
        let _memo = self.query_memo();
        let walkable = |pos: Pos| {
            self.map
                .get(pos)
                .is_some_and(|tile| !self.rules.is_water(tile))
                && self.can_path_through_neighbor(uid, pos, &access)
        };
        let mut queue = VecDeque::new();
        for pos in self.wdisk(target, range) {
            if walkable(pos) {
                steps.insert(pos, 0);
                queue.push_back(pos);
            }
        }
        while let Some(current) = queue.pop_front() {
            let at = steps[&current];
            if at >= max_len {
                continue;
            }
            for next in self.nbrs(current) {
                if steps.contains_key(&next) || !walkable(next) {
                    continue;
                }
                steps.insert(next, at + 1);
                queue.push_back(next);
            }
        }
        steps
    }
}
