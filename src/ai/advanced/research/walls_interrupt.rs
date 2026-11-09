//! Native capital warnings must not wait for an unrelated technology to finish.
//! Use the existing host research bank and eligibility contract to retain its work.
use super::*;

impl AdvancedAi {
    pub(super) fn capital_walls_research_switch(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        let current = g.players[pid].research.as_deref()?;
        let goal = crate::name!("masonry");
        if self.capital_walls_research_threat(g, pid).is_none()
            || g.players[pid].techs.contains(&goal)
            || current == goal.as_str()
            || self.tech_leads_to(g, current, goal.as_str())
            || !g.host_research_prerequisites_revealed(goal)
            || g.host_can_research(pid, goal) != Some(true)
            || !g.available_techs(pid).contains(&goal)
            || g.host_remaining_research_cost(pid, Name::new(current))
                .is_none()
            || g.host_remaining_research_cost(pid, goal).is_none()
        {
            return None;
        }
        Some("masonry")
    }
}

#[cfg(test)]
mod tests;
