//! `cold-war-then-heritage`: once a Culture target knows Cold War, its civic
//! slot takes Conservation and then Cultural Heritage before the government
//! ladder and the rest of the culture list.
//!
//! **Why (live Deity Online Kristina, 2026-11-10).** From Cold War on, our
//! visitors come from tourism, and Music Censorship can keep our Rock Bands
//! out of a rival's land; per-turn tourism is the part no rival can bar.
//! Conservation (+1/+2/+3 Tourism per Walls, Castle and Star Fort) and
//! Cultural Heritage (Heritage Tourism: art and artifacts x2) are the two
//! civics past Cold War that raise it. Sixteen live games from 16:00Z to
//! 23:00Z reached Cold War (t128-174, median about t150) and ended at
//! t159-202, a median 35 turns later. In every one the civic slot went from
//! Cold War to the tier-3 government civics (`government_ladder_goal`: "the
//! most valuable step toward class struggle, which culture needs", then
//! Suffrage), and most then walked Professional Sports, Space Race, Social
//! Media, Rapid Deployment and Globalization toward Corporate Libertarianism.
//! Conservation was learned 8 turns after Cold War in 173750Z, 29-37 turns
//! after it in three games, and never in twelve. At the culture rate held at
//! Cold War (77-260 a turn), the two civics cost about 770 and 980 Online
//! culture (Kristina's themed buildings had already inspired Cultural
//! Heritage in the games read), six to ten turns. Each of the four games
//! that learned Cultural Heritage slotted Heritage Tourism on every turn
//! after it, so the card needs no change.
//!
//! **The gene.** While a live Culture target knows Cold War and lacks either
//! civic, the civic slot's goal is Conservation, then Cultural Heritage (the
//! beeline takes any missing prerequisite on the way). It sits below the
//! goals that answer an emergency or a window: the Great Person, first
//! government and match-point civics, the museum unlock, the Cold War road
//! itself, `decisive-window` and the domination upgrade. It sits above the
//! Heritage unlock of `culture-cold-war-window`, the religion seat's civic,
//! the government ladder and the culture list. Nothing changes before Cold War
//! is known, on another target, or once the lane is released. With the gene
//! off nothing changes.

use super::*;

/// The civics this gene takes after Cold War, in order.
const HERITAGE_ROAD: [&str; 2] = ["conservation", "cultural_heritage"];

impl AdvancedAi {
    /// `Some(civic)` while the gene drives the civic slot: the first of
    /// [`HERITAGE_ROAD`] not yet known. `None` with the gene off, before Cold
    /// War, on another target, on a released lane, or with both known.
    pub(super) fn cold_war_then_heritage_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.cold_war_then_heritage
            || self.lane_lost
            || !g.victory_conditions.culture
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return None;
        }
        let civics = &g.players[pid].civics;
        if !civics.contains(&crate::name!("cold_war")) {
            return None;
        }
        HERITAGE_ROAD
            .into_iter()
            .find(|civic| !civics.contains(&Name::new(civic)))
    }

    /// [`Self::cold_war_then_heritage_goal`], journalled: called once, when
    /// the empty civic slot takes the goal.
    pub(super) fn cold_war_then_heritage_pick(&self, g: &Game, pid: usize) -> Option<&'static str> {
        let goal = self.cold_war_then_heritage_goal(g, pid)?;
        think!(self.journal(), Research, Decision,
            "Civic slot aims at {} after Cold War", plain(goal);
            "cold-war-then-heritage: per-turn tourism no rival can bar; conservation, then \
             cultural heritage, before the government ladder and the culture list");
        Some(goal)
    }
}

#[cfg(test)]
mod tests;
