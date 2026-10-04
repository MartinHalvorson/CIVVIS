//! Recon units spend their free turns opening distant frontiers.
use super::*;

/// How far from a met major's unattributed ground the lead scout reads the
/// fog for its cities. A tile is owned by a city at most a few rings away.
const RIVAL_LEAD_RADIUS: i32 = 3;

impl AdvancedAi {
    /// The fog nearest `uid` beside ground a met major owns whose city we
    /// have never seen (`Game::unseen_major_borders`), for the one lead scout.
    ///
    /// ★★★ A RIVAL WITH NO KNOWN CITY CANNOT BE COUNTERED. Live King
    /// civvis-20261003T152830Z (game 40) read Kongo's culture clock at 75 to
    /// 89 from turn 108 and could not act on it: `conquest_denial_actionable`
    /// needs a city of theirs, and the first Kongo city was seen at 140, a
    /// turn before Kongo won. In civvis-20261003T162445Z (game 42) the
    /// Ottomans, who won on Religion at 137, were never located, though their
    /// ground was seen at turn 70 thirteen tiles from Cuenca. Both games' free
    /// scouts swept the generic nearest frontier. A native board has no such
    /// ground, so only the live seat follows a lead.
    pub(super) fn rival_lead_goal(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        if g.unseen_major_borders.is_empty() {
            return None;
        }
        // One scout follows the lead; any other keeps the ordinary sweep.
        let lead = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|other| self.distance_scout_available(g, pid, *other));
        if lead != Some(uid) {
            return None;
        }
        let dead: Vec<Pos> = self
            .base
            .explore_dead
            .borrow()
            .get(&uid)
            .map(|dead| {
                dead.iter()
                    .filter(|(_, expiry)| **expiry > g.turn)
                    .map(|(pos, _)| *pos)
                    .collect()
            })
            .unwrap_or_default();
        let explored = &g.players[pid].explored;
        let origin = g.units[&uid].pos;
        let mut considered = std::collections::BTreeSet::new();
        g.unseen_major_borders
            .iter()
            .flat_map(|border| g.wdisk(*border, RIVAL_LEAD_RADIUS))
            .filter(|pos| considered.insert(*pos))
            .filter(|pos| {
                !explored.contains(pos) && !dead.contains(pos) && g.unit_can_traverse(uid, *pos)
            })
            .min_by_key(|pos| (g.wdist(origin, *pos), *pos))
    }

    /// Existing civilian protection is honored; routine posts cannot recruit
    /// the explorer. The production exploration switch preserves frozen AIs.
    pub(super) fn distance_scout_available(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.base.explore_commit
            && !g.is_arena()
            && !self.base.minor
            && !self.base.barb
            && g.units.get(&uid).is_some_and(|unit| {
                unit.owner == pid
                    && BasicAi::unit_doctrine(g, uid) == UnitDoctrine::Recon
                    && unit.linked_to.is_none()
                    && !self.guard_is_bound_to_any_settler(uid)
                    && !self.builder_guard_reserved(uid)
            })
    }

    pub(super) fn distance_scout_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.distance_scout_available(g, pid, uid) {
            return None;
        }
        // The existing explorer keeps its frontier commitment, separates from
        // other scouts, and routes around known threats. When no exploration
        // route remains, the ordinary fallback may use the idle unit.
        let before = g.units[&uid].pos;
        // See `rival_lead_goal`: the lead becomes the explorer's held goal,
        // so its routing, threat checks and dead-goal retirement still apply.
        if let Some(goal) = self.rival_lead_goal(g, pid, uid) {
            self.base
                .explore_goal
                .borrow_mut()
                .insert(uid, (goal, g.turn));
        }
        let acted = self.explorer_turn(g, pid, uid)?;
        if acted && g.units.get(&uid).is_some_and(|unit| unit.pos != before) {
            let at = g.units[&uid].pos;
            think!(self.journal(), Military, Detail,
                "{} {uid} explores beyond home", g.units[&uid].kind;
                "a free recon unit follows its exploration route before routine military assignments"; at);
        }
        Some(acted)
    }
}

#[cfg(test)]
mod tests;
