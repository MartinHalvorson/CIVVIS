//! Opt-in frontier fortification experiment. The initial checkpoint keeps
//! the missing behavior explicit so the production-path controls fail first.

use super::*;

impl AdvancedAi {
    pub(super) fn remember_frontier_watch_sightings(&mut self, _g: &Game, _pid: usize) {
        let _ = self.frontier_watch_sightings.is_empty();
    }

    pub(super) fn frontier_walls_research_goal(&self, _g: &Game, _pid: usize) -> Option<Name> {
        let _ = self.frontier_walls_before_the_surprise;
        None
    }

    pub(super) fn claim_frontier_walls_before_the_surprise(&self, _g: &mut Game, _pid: usize) {}
}

#[cfg(test)]
mod tests;
