//! The ordinary army's approach through the role-spacing boundary.

use super::{AdvancedAi, ForcePosture};

impl AdvancedAi {
    /// Enable the independently measured ordinary army approach experiment.
    pub fn enable_role_spacing_continuity(&mut self) {
        self.role_spacing_continuity = true;
    }

    /// Restore the historical five-hex spacing boundary.
    pub fn disable_role_spacing_continuity(&mut self) {
        self.role_spacing_continuity = false;
    }

    pub(super) fn role_spacing_continues(&self, posture: ForcePosture) -> bool {
        self.role_spacing_continuity
            && !self.base.legacy_movement
            && matches!(
                posture,
                ForcePosture::Advance | ForcePosture::Engage | ForcePosture::Muster
            )
    }
}

#[cfg(test)]
mod tests;
