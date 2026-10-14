//! Recon units spend their free turns opening distant frontiers.
use super::*;

impl AdvancedAi {
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
        let survey = self.air_resource_scout_goal(g, pid, uid);
        if let Some(goal) = survey {
            let mut goals = self.base.explore_goal.borrow_mut();
            let held = goals.entry(uid).or_insert((goal, g.turn));
            if held.0 != goal {
                *held = (goal, g.turn);
            }
        }
        // The existing explorer keeps its frontier commitment, separates from
        // other scouts, and routes around known threats. When no exploration
        // route remains, the ordinary fallback may use the idle unit.
        let before = g.units[&uid].pos;
        let acted = if survey.is_some() {
            // A coastal Loyalty neighborhood can contain water beyond land
            // sight. Only the selected recon eye gets that survey domain;
            // ordinary combat units keep their come-ashore exploration rule.
            if self.base.clear_adjacent_empty_barbarian_camp(g, pid, uid)
                || self.base.village_collection_step(g, pid, uid)
                || self.base.explore_step_with_domain(g, pid, uid, false)
            {
                Some(true)
            } else {
                None
            }
        } else {
            self.explorer_turn(g, pid, uid)
        }?;
        if acted && g.units.get(&uid).is_some_and(|unit| unit.pos != before) {
            let at = g.units[&uid].pos;
            think!(self.journal(), Military, Detail,
                "{} {uid} explores beyond home", g.units[&uid].kind;
                "a free recon unit follows its exploration route before routine military assignments"; at);
        }
        Some(acted)
    }

    /// Send one free, healthy eye to a known supply lead, never a hidden deposit.
    /// The ordinary explorer retains routing, danger and host-refusal handling.
    pub(in crate::ai::advanced) fn air_resource_scout_goal(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<Pos> {
        let source = self.air_resource_frontier(g, pid)?;
        let scout = g
            .units
            .values()
            .filter(|u| u.hp >= 75 && self.distance_scout_available(g, pid, u.id))
            .min_by_key(|u| (g.wdist(u.pos, source), u.id))?;
        if scout.id != uid {
            return None;
        }
        let visible = g.player_vision_frame(pid);
        let threats = g
            .units
            .values()
            .filter(|u| {
                u.owner != pid
                    && g.is_at_war(pid, u.owner)
                    && g.rules.units[u.kind].class == "military"
                    && g.sees(&visible, u.pos)
            })
            .map(|u| u.pos)
            .collect::<Vec<_>>();
        let known = &g.players[pid].explored;
        let dead = self.base.explore_dead.borrow();
        let usable = |pos: Pos| {
            g.wdist(pos, source) <= 9
                && !known.contains(&pos)
                && g.map.tiles.contains_key(&pos)
                && !dead.get(&uid).is_some_and(|tiles| tiles.get(&pos).is_some_and(|expiry| *expiry > g.turn))
                && !threats.iter().any(|enemy| g.wdist(*enemy, pos) <= super::super::EXPLORE_COMMIT_THREAT_RADIUS)
                // Only charted adjacent terrain is inspected. The destination
                // itself is unknown; its unseen terrain is not a ranking input.
                && g.nbrs(pos).into_iter().any(|p| known.contains(&p)
                    && g.map.get(p).is_some_and(|t| g.rules.is_passable(t)
                        && (!g.rules.is_water(t) || g.unit_can_traverse(uid, p))))
        };
        if let Some((goal, since)) = self.base.explore_goal.borrow().get(&uid).copied() {
            if g.turn.saturating_sub(since) <= super::super::EXPLORE_COMMIT_TURNS && usable(goal) {
                return Some(goal);
            }
        }
        g.wdisk(source, 9)
            .into_iter()
            .filter(|pos| usable(*pos))
            .max_by_key(|pos| {
                let reveals = g
                    .wdisk(*pos, g.unit_sight(uid))
                    .into_iter()
                    .filter(|p| g.wdist(*p, source) <= 9 && !known.contains(p))
                    .count() as i32;
                (
                    reveals * 4 - g.wdist(scout.pos, *pos),
                    std::cmp::Reverse(g.wdist(scout.pos, *pos)),
                    std::cmp::Reverse(*pos),
                )
            })
    }
}

#[cfg(test)]
mod tests;
