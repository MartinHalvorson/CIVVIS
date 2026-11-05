//! A Defend assignment has a city to protect, even when an easier contact
//! offers the best exchange elsewhere. Native Indonesia 214426Z assigned all
//! four soldiers to Defend Majapahit at turn 29, then formed toward a scout
//! five tiles away; its archer left the capital's reach. The city fell at 31.
//! This is evidence of a lost defensive remit, not proof of an avoidable loss.
//!
//! The independent opt-in keeps healthy combat formation slots within the
//! existing wall-defender radius and ranks contacts threatening that city.
//! Healing placements retain their ordinary safe retreat search. It changes
//! neither the kill search nor immediate legal strikes, Relieve assignments,
//! offensive forces, or arena formations. No native strength gain is claimed.
//! The later strike-opening pass also preserves the remit: a healthy member
//! may open a shot without moving farther from the city than its current
//! distance or the wall-defender radius. Otherwise that pass could undo a
//! correct formation and send an unplaced archer after the same distant scout.
use super::objective_board::ObjectiveKey;
use super::{AdvancedAi, ForceDomain, ForceGroup, ForcePosture};
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    pub(super) fn defensive_formation_anchor(
        &self,
        g: &Game,
        pid: usize,
        group: &ForceGroup,
    ) -> Option<Pos> {
        if !self.defenders_keep_the_city_in_reach
            || !self.objective_board
            || g.is_arena()
            || group.domain != ForceDomain::Land
        {
            return None;
        }
        let task = self
            .objective_board_state
            .forces
            .iter()
            .find(|task| task.id == group.id)?;
        let ObjectiveKey::Defend(cid) = task.objective_key else {
            return None;
        };
        let city = g.cities.get(&cid)?;
        (city.owner == pid && self.city_under_fire_for_the_hold(g, pid, cid)).then_some(city.pos)
    }

    pub(super) fn defensive_strike_stays_in_reach(
        &self,
        g: &Game,
        pid: usize,
        group: &ForceGroup,
        uid: u32,
        tile: Pos,
    ) -> bool {
        if group.posture != ForcePosture::Engage {
            return true;
        }
        let Some(anchor) = self.defensive_formation_anchor(g, pid, group) else {
            return true;
        };
        let unit = &g.units[&uid];
        unit.hp < super::battle_planner::ROTATE_HP
            || g.wdist(tile, anchor)
                <= g.wdist(unit.pos, anchor)
                    .max(super::defenders_hold_the_walls::WALL_DEFENDER_REACH)
    }
}

#[cfg(test)]
mod tests;
