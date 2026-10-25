//! Native capital access census: `docs/eval/2026-10-25-capital-assault-audit.md`.
//! The initial no-op stub lets the same real-spy dispatch regressions run
//! before this candidate's behavior is implemented.

use super::*;

impl AdvancedAi {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wing_capital_spy_action(
        &self,
        _g: &Game,
        _pid: usize,
        _spy_id: u32,
        _plan: &StrategicPlan,
        _legal: &[Action],
        _racer_leader: Option<usize>,
        _culture_targets: &BTreeSet<usize>,
    ) -> Option<Action> {
        if !self.wing_capital_spotting {
            return None;
        }
        None
    }
}

#[cfg(test)]
mod tests;
