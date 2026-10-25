//! `capital-watch-arms-the-slinger`: one solvent, healthy prewar upgrade.
//!
//! Development baseline: the registered hook deliberately does nothing.
use super::*;

impl AdvancedAi {
    pub(super) fn capital_watch_slinger_upgrade(&mut self, _g: &mut Game, _pid: usize) -> bool {
        let _ = (
            self.capital_watch_arms_the_slinger,
            self.capital_watch_slinger_turn,
        );
        false
    }
}

#[cfg(test)]
mod tests;
