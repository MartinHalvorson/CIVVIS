//! Exceptional routing around temporarily unavailable tiles.
use super::Game;
use crate::Pos;
use std::collections::{BTreeSet, VecDeque};

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
}
