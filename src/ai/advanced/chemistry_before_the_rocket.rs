//! `chemistry-before-the-rocket`: a Science seat whose next victory technology
//! is Rocketry, and which already holds a University, researches Chemistry
//! (the Research Lab) before the rest of Rocketry's prerequisites.
//!
//! **Why (live Deity, 6-player Continents, Online, the 22 Science-seat games of
//! 2026-11-18 that reached t100).** The Research Lab was the largest single
//! Science line at t150 (a median 22 Science missing in the twelve games that
//! had Chemistry, and no Lab at all in the nine that did not). Chemistry came at
//! t122-129 in the strong games and at t134-163, or not by t150, in the weak
//! ones. It has 22 ancestors (the host's own `research_tree` agrees), yet the
//! seats held 41-44 techs when it landed: between Education and Chemistry a
//! median 40% of the beakers went to technologies that do not lead to it.
//! Cartography, Square Rigging, Mass Production, Military Tactics,
//! Industrialization and Steam Power were taken in 21-22 of the 22 games. All
//! six are Rocketry's other half: Rocketry requires Radio and Chemistry, the
//! Science lane beelines for Rocketry (`science_victory_tech_goal`), and the
//! beeline takes the highest-valued (or cheapest) legal step toward it, so the
//! Radio branch went first and Chemistry, the most expensive node, came last.
//! On each game's own Science curve, the beakers of Chemistry's remaining
//! ancestors were in hand a median 12 turns before Chemistry landed (quartiles
//! 11-16; 35-36 turns in two games). Labs waited little once Chemistry and a
//! University stood: a median 0.7 turns beyond their build time.
//! `unreachable_science_building_tech` already names Chemistry once a University
//! stands, but it is the last goal in the chain and the Science lane's Rocketry
//! goal takes the slot before it.
//!
//! **The gene.** While the seat is on the Science lane, Rocketry is the first
//! victory technology it lacks, and `unreachable_science_building_tech` names a
//! technology that leads to Rocketry (Chemistry once a Campus city holds its
//! University), that technology is the forced research goal, one slot ahead of
//! the Rocketry goal; its legal steps are admitted past the rolling era window.
//! Chemistry is an ancestor of Rocketry, so the lane researches the same nodes
//! in another order and loses nothing toward the launch. A war, barbarian,
//! wall, luxury, Harbor or bargain goal above it keeps its slot. Off, nothing
//! changes.
//!
//! Grep the journal for `chemistry-before-the-rocket:`.

use super::*;

impl AdvancedAi {
    /// The research goal this gene puts ahead of the Rocketry goal: the
    /// science building technology `unreachable_science_building_tech` names,
    /// when it leads to Rocketry and Rocketry is the lane's next victory
    /// technology. `None` with the gene off or off the Science lane.
    pub(super) fn chemistry_before_the_rocket_goal(
        &self,
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
        science_lane_active: bool,
    ) -> Option<&'static str> {
        if !self.chemistry_before_the_rocket || !science_lane_active {
            return None;
        }
        if Self::science_victory_tech_goal(g, pid, objective) != Some("rocketry") {
            return None;
        }
        let tech = self.unreachable_science_building_tech(g, pid)?;
        self.tech_leads_to(g, tech, "rocketry").then_some(tech)
    }

    /// The journal line for a research pick this gene's goal made.
    pub(super) fn explain_chemistry_before_the_rocket(&self, goal: &str, pick: &str) {
        think!(self.journal(), Research, Decision,
               "researches {pick} toward {goal} before the rest of Rocketry";
               "chemistry-before-the-rocket: a University stands, so the Research Lab's technology \
                comes before Rocketry's Radio half; {goal} leads to Rocketry, so no beaker is lost");
    }
}

#[cfg(test)]
mod tests;
