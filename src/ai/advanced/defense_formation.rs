//! Preserve a threatened city's defensive assignment across ranged firing
//! positions and the later formation/movement passes. Native Indonesia
//! 214426Z lost its capital at31; at29 its defending archer walked from three
//! to four tiles away to shoot a scout five tiles from the city.
//!
//! Two captured-input attempts changing only later passes left all emitted
//! orders unchanged. The joint combat planner must enforce the assignment
//! before it moves the shooter; later passes must not restore that excursion.
//!
//! Independent opt-in, off by default. Healthy ranged members may fire within
//! the existing two-tile wall-defender radius, or no farther from the city
//! than their current stand while arriving. Current-stand shots remain legal.
//! Formation combat slots stay within two and use contacts threatening the
//! assigned city. Wounded recovery, melee blows, relief, offensive assignments
//! and arena play retain their ordinary behavior.
use super::objective_board::ObjectiveKey;
use super::{AdvancedAi, ForceDomain, ForceGroup};
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

    /// Bound optional ranged firing stands, without limiting wounded retreat
    /// or replacing an absent/non-defensive assignment.
    pub(super) fn defensive_firing_radius(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<(Pos, i32)> {
        if !self.defenders_keep_the_city_in_reach {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid
            || unit.hp < super::battle_planner::ROTATE_HP
            || !g.rules.units[unit.kind].has_ranged_attack()
        {
            return None;
        }
        let group = self
            .force_groups
            .iter()
            .find(|group| group.units.contains(&uid))?;
        let anchor = self.defensive_formation_anchor(g, pid, group)?;
        Some((
            anchor,
            g.wdist(unit.pos, anchor)
                .max(super::defenders_hold_the_walls::WALL_DEFENDER_REACH),
        ))
    }
}

#[cfg(test)]
mod tests;
