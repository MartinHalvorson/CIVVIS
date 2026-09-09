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
        // The existing explorer keeps its frontier commitment, separates from
        // other scouts, and routes around known threats. When no exploration
        // route remains, the ordinary fallback may use the idle unit.
        let before = g.units[&uid].pos;
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
