use super::*;

impl AdvancedAi {
    /// A non-founder resisting conversion needs an alternative faith at home.
    /// Apply the same threat and safety checks used when buying defenders:
    /// removing that faith must not help the religion we are trying to stop.
    pub(super) fn preserve_competing_faith(
        &self,
        g: &Game,
        pid: usize,
        target: &crate::game::Unit,
    ) -> bool {
        let Some(faith) = target.religion.as_deref() else {
            return false;
        };
        if !g
            .player_city_ids(pid)
            .iter()
            .any(|cid| g.wdist(g.cities[cid].pos, target.pos) <= 6)
        {
            return false;
        }
        self.adopted_faith_threat(g, pid)
            .is_some_and(|threat| faith != threat && Self::safe_adopted_counterfaith(g, pid, faith))
    }
}

#[cfg(test)]
mod tests;
