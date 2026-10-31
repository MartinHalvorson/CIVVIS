//! `launcher-war-serves-every-lane`: a Culture or Diplomatic seat fights the
//! space racer past its Moon, as a Domination seat already does.
//!
//! The launcher war (`launcher-war-ignores-the-edge`, `war-raids-the-pads`,
//! `launcher-war-skips-the-reserve`, `no-peace-with-a-launcher`; see
//! `advanced/launcher_war.rs`) opens from the declaration step, and that step
//! returns at once unless the plan is Conquest. On a Domination seat
//! `domination_military_counter` hands the plan a Conquest counter against a
//! launch clock. On every other lane the counter comes from
//! `denial_response_for_pressure`, and with `counter-in-lane` (live) a Science
//! threat is answered with GrandStrategy::Science: we race the racer. A seat
//! two or three projects behind cannot win that race. The launcher war's
//! genes, all armed live, could not fire on a Culture or Diplomatic seat.
//!
//! Live Lv 7 Immortal, 2026-10-30. Nine Culture/Diplomatic seats lost to a
//! Science Victory, and none opened a war on the racer between its Moon and
//! its launch:
//! - 224544Z (Culture, the best Lv 7 culture seat: 906 tourism and 162
//!   visitors at t200). The Ottomans landed the Moon at 161, Mars at 179, and
//!   launched at 197. Our power was at least 0.8x theirs on 35 of those 36
//!   turns (median 1.8x). From 179 the plan read "Grand strategy: science |
//!   countering Ottomans, its science race reads 84%", and Winnipeg started
//!   our own Moon Landing. Lost at 204.
//! - 160045Z (Culture). Poland: Moon 179, launch 220. Our power was at least
//!   0.97x on all 41 turns. Lost at 226.
//! - 162440Z (Culture). Babylon: Moon 184, launch 196. 1.16-1.41x on 12/12.
//! - 173341Z and 190332Z (Diplomatic) both lost with 18 of 20 points. Georgia
//!   (Moon 158, launch 191) and the Aztecs (Moon 148, launch 162) were fought
//!   from 0.8x on 21/33 and 7/14 turns. A few turns of delay were the game.
//!
//! Off Domination, a rival the launcher war would open on (past its Moon,
//! short of the launch, a pad of its seen, our power at least 0.8x its
//! steady power, no city of ours falling, neither friend nor ally) becomes
//! the counter in place of any counter that is not already a war: an in-lane
//! race, or none. A Conquest counter already chosen keeps its rival, so
//! `counter-culture-by-conquest` against a nearer culture finish stands.
//! The most launches landed go first, then the lower seat. The declaration's own
//! gates (one war at a time, legality, reach) still hold. The plan's first
//! objective is the pad city (`science-suppression-hits-the-pads`). Needs
//! `launcher-war-ignores-the-edge`.

use super::*;

impl AdvancedAi {
    /// See the module. `None` with the gene off, on a Domination seat, or
    /// when no racer passes [`Self::launcher_war_opens`] and the Conquest
    /// counter's actionable check.
    pub(super) fn launcher_lane_counter(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, GrandStrategy)> {
        if !self.launcher_war_serves_every_lane
            || !self.deny_leaders
            || !g.victory_conditions.science
            || self.active_victory_target(g) == Some(VictoryTarget::Domination)
        {
            return None;
        }
        let counter = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            // A friend or an ally cannot be declared on (190332Z's ally
            // Korea started projects at turn 160).
            .filter(|rival| self.campaign_target_legal(g, pid, rival.id))
            .filter(|rival| self.launcher_war_opens(g, pid, rival.id))
            .filter(|rival| {
                self.conquest_denial_actionable(g, pid, rival.id, GrandStrategy::Conquest)
            })
            .max_by_key(|rival| {
                (
                    Self::science_denial_stages(g, rival.id),
                    std::cmp::Reverse(rival.id),
                )
            })
            .map(|rival| rival.id)?;
        if self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                   "Countering {}'s launch with war", g.players[counter].civ;
                   "launcher-war-serves-every-lane: {} space projects landed and a pad standing; {:.0} power against their steady {:.0}, and racing them in lane cannot land the Exoplanet first",
                   Self::science_denial_stages(g, counter), g.military_power(pid),
                   self.steady_rival_power(g, counter));
        }
        Some((counter, GrandStrategy::Conquest))
    }
}

#[cfg(test)]
mod tests;
