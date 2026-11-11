//! `research-follows-the-first-capital`: keep the preparing first-capital
//! army's breaker and unique melee research after the stepping stone ends.
//!
//! Native Deity Macedon 085706Z released Nalanda at turn 20 when Byzantium
//! became its suzerain, then named Constantinople. The stepping stone's
//! Masonry/unique-melee research contract ended with it: Pottery (25),
//! Writing (27), Sailing (29), Horseback Riding (31) and Construction (39)
//! preceded Bronze Working (51) and Iron Working (53). Warriors defended
//! against barbarian Crossbows, Swordsmen and Men-at-Arms by turn 50.
//!
//! While the first-capital opening is preparing against a living major,
//! reuse the stone's rules-derived breaker/unique melee goals. Capital walls
//! and opening Archery keep their earlier slots; an in-flight technology
//! finishes. Off, or outside that opening, this supplies no goal. This is
//! research intent, not a claim of material access, trained units or wins.

use super::*;

impl AdvancedAi {
    pub(super) fn first_capital_army_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.research_follows_the_first_capital
            || !self.first_capital_active()
            || !self.conquest_reservation_open(g)
        {
            return None;
        }
        let opening = self.conquest_opening.as_ref()?;
        if opening.declared.is_some() {
            return None;
        }
        let rival = g.players.get(opening.target)?;
        if !rival.alive || rival.is_minor || rival.is_barbarian {
            return None;
        }
        if g.cities.get(&opening.city)?.owner != opening.target {
            return None;
        }
        Self::city_state_stone_research_goals(g, pid)
            .into_iter()
            .next()
    }
}

#[cfg(test)]
mod tests;
