//! Spend an affordable envoy package on a city-state blocking an existing siege.
//! The package buys passage before the ordinary one-placement economic ranking.
use super::AdvancedAi;
use crate::game::Game;

impl AdvancedAi {
    /// The adapter's physical placement cap for one disposable planning board.
    /// Headless play has no cap. Only this opt-in consumes the adapter budget.
    pub fn set_envoy_order_budget(&mut self, placements: Option<usize>) {
        self.envoy_order_budget = placements;
    }

    pub(super) fn siege_road_envoy_package(
        &self,
        g: &Game,
        pid: usize,
        minor: usize,
        remaining: usize,
    ) -> Option<usize> {
        if !self.envoys_open_the_siege_road
            || !g.can_send_envoy(pid, minor)
            || g.is_at_war(pid, minor)
            || g.has_open_borders(pid, minor)
            || !self.siege_minor_passage_blockers(g, pid).contains(&minor)
        {
            return None;
        }
        let rival = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian && p.id != pid)
            .map(|p| g.envoys_at(p.id, minor))
            .max()
            .unwrap_or(0);
        // This shared model includes League, Containment and Amani. A tie
        // removes a rival's suzerainty but does not grant us passage.
        g.envoy_investment_options(pid, minor)
            .into_iter()
            .filter(|(placements, count, _)| {
                *count >= 3
                    && *count > rival
                    && usize::try_from(*placements).is_ok_and(|n| n <= remaining)
            })
            .filter_map(|(placements, _, _)| usize::try_from(placements).ok())
            .min()
    }
}

#[cfg(test)]
mod tests;
