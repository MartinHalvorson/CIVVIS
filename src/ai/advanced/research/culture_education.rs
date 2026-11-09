//! Let an equipped Culture empire reach Universities before its tourism beeline.

use super::*;

impl AdvancedAi {
    pub(super) fn culture_education_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if self.active_victory_target(g) != Some(VictoryTarget::Culture) {
            return None;
        }
        // Reuse the science-building contract: a completed Campus and every
        // prerequisite building must stand in the same owned city. Restrict
        // this detour to Education, so it ends once Universities are unlocked
        // and never sends a Culture seat after Research Labs before tourism.
        self.unreachable_science_building_tech(g, pid)
            .filter(|tech| *tech == "education")
    }
}

#[cfg(test)]
mod tests;
