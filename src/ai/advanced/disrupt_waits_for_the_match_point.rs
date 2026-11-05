//! `disrupt-waits-for-the-match-point`: our Spies run Disrupt Rocketry only
//! against a racer on its last space projects; before that they steal, gain
//! sources or listen.
//!
//! ## Why
//!
//! Live Lv 8 Deity and Lv 7 Immortal seats, 11-02 to 11-05 (session -71's
//! census, re-IDs matched: the host re-creates a travelling or escaping Spy
//! under a new unit id, so a removal is a death only when no Spy with its XP
//! reappears): of 102 Disrupt Rocketry orders, 51 Spies died within 8 turns
//! (50%), against 5 of 78 for Gain Sources (6%) and 3 of 44 for Listening
//! Post (7%); and no disruption visibly delayed a winner. In the four science losses traced the winner's third
//! project landed 5-10 turns before its fourth (the Exoplanet expedition):
//! Spain t168 -> t178, Zulu t157 -> t162, Phoenicia t155 -> t164, India's
//! third at t163 before its win at t182. That window, and the laser projects
//! after it, are where a delay can change the result; earlier, a Disrupt
//! spends a Spy with even odds of dying on a race the rival has twenty turns to recover.
//! `disrupt-before-the-sources` sends a level-0 Spy to Disrupt from the
//! racer's first project.
//!
//! ## What the gene does
//!
//! While the owner of the city a Spy stands in has landed fewer than
//! [`MATCH_POINT_STAGES`] space projects, Disrupt Rocketry is withheld from
//! that Spy: dropped from its legal missions in the spy pass, skipped by
//! `science-denial-every-pad`, and not the reason `disrupt-before-the-sources`
//! sends a recruit in. Where it is withheld, `spies-steal-the-lead`'s steal
//! stands aside as well (steals in racers' cities lost 16 of 25 Spies), so
//! the Spy runs the safe operation, Listening Post or Gain Sources, and lives
//! to the match point; steals in other leaders' cities are untouched. From
//! the match point on, every path is the old one.
//! `Kind::OptIn`; off, nothing changes.

use super::*;

/// A racer with this many space projects landed is at its match point.
pub(crate) const MATCH_POINT_STAGES: usize = 3;

impl AdvancedAi {
    /// Whether Disrupt Rocketry is withheld from `spy`: the gene is on and
    /// the owner of the city it stands in has landed fewer than
    /// [`MATCH_POINT_STAGES`] space projects. False for a Spy in no city.
    pub(super) fn disrupt_held_for_the_match_point(&self, g: &Game, spy: u32) -> bool {
        if !self.disrupt_waits_for_the_match_point {
            return false;
        }
        g.spies
            .get(&spy)
            .and_then(|agent| agent.city)
            .and_then(|city| g.cities.get(&city))
            .is_some_and(|city| Self::science_denial_stages(g, city.owner) < MATCH_POINT_STAGES)
    }

    /// Whether the host or the board offers `spy` a Disrupt Rocketry this
    /// gene withholds: the Spy stands in a racer's city before the match
    /// point. `spies-steal-the-lead` stands aside there too.
    pub(super) fn disrupt_withheld_here(&self, g: &Game, spy: u32) -> bool {
        self.disrupt_held_for_the_match_point(g, spy)
            && g.spies.get(&spy).is_some_and(|agent| {
                Self::racer_spy_disrupt_offered(g, spy, &g.legal_spy_actions(agent.owner, spy))
            })
    }

    /// `legal` without Disrupt Rocketry while it is withheld from `spy`.
    pub(super) fn withhold_early_disrupts(
        &self,
        g: &Game,
        spy: u32,
        mut legal: Vec<Action>,
    ) -> Vec<Action> {
        if self.disrupt_held_for_the_match_point(g, spy) {
            legal.retain(|action| {
                !matches!(action, Action::SpyMission { mission, .. } if mission == "disrupt_rocketry")
            });
        }
        legal
    }

    /// The racer stages `disrupt-before-the-sources` reads before a recruit
    /// disrupts: its own floor, or the match point under this gene.
    pub(super) fn disrupt_from_stages(&self) -> usize {
        if self.disrupt_waits_for_the_match_point {
            MATCH_POINT_STAGES
        } else {
            super::disrupt_before_the_sources::DISRUPT_FROM_STAGES
        }
    }
}

#[cfg(test)]
mod tests;
