//! A native queued path can outlive the revelation that made it obsolete.
//! Reopen only the next committed launch milestone; the ordinary research
//! priority ladder still decides whether it may actually replace the incumbent.
use super::*;

impl AdvancedAi {
    pub(super) fn revealed_launch_research_switch(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        if !self.revealed_launch_tech_takes_the_slot {
            return None;
        }
        let current = g.players[pid].research.as_deref()?;
        let goal = self.science_endgame_research_goal(g, pid)?;
        let goal_name = Name::new(goal);
        if current == goal
            || self.tech_leads_to(g, current, goal)
            || !g.host_research_prerequisites_revealed(goal_name)
            || g.host_can_research(pid, goal_name) != Some(true)
            || !g.available_techs(pid).contains(&goal_name)
            || g.host_remaining_research_cost(pid, Name::new(current))
                .is_none()
            || g.host_remaining_research_cost(pid, goal_name).is_none()
        {
            return None;
        }
        Some(goal)
    }
}

#[cfg(test)]
mod tests;
