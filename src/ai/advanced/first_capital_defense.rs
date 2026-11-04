//! `first-capital-before-the-camp`: opening defence before optional raids.
//!
//! Native Standard Immortal `civvis-20261104T060224Z` turn 12 gave the only
//! two-unit force to ClearCamp while Defend Bogota and Relieve had no force.
//! The same turn's opening defence displaced its first Settler. At turn 67
//! the empire still had only its capital. This does not prove a saved-city
//! or earlier-expansion counterfactual.
//!
//! An existing Defend row becomes urgent only for the sole capital, while
//! the strategic plan names it threatened, through 60 Standard-equivalent
//! turns. Existing board urgency makes it precede offense and permits it to
//! recall committed troops. No row is created without the normal observed
//! pressure/contact checks. The treatment changes neither Settler production
//! nor combat legality and is off by default.

use super::*;

const OPENING_DEFENSE_STANDARD_TURNS: u32 = 60;

impl AdvancedAi {
    pub(super) fn first_capital_defense_is_urgent(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.first_capital_before_the_camp
            && g.turn <= g.standard_duration(OPENING_DEFENSE_STANDARD_TURNS)
            && plan.threatened_city == Some(cid)
            && g.player_city_ids(pid).as_slice() == [cid]
            && g.cities.get(&cid).is_some_and(|city| city.is_capital)
    }
}

#[cfg(test)]
mod tests;
