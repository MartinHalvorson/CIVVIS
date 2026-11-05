//! `group-steps-read-the-field`: the group step (`coordinated_tactical_step`
//! in `tactics.rs`) prices an approach tile by the battle planner's danger
//! field as well as by `BasicAi::projected_counter_damage`, and takes the
//! larger.
//!
//! `projected_counter_damage` gives every visible hostile one approach step
//! before its blow (`attack range + 1`, none for siege). A unit with more
//! movement than that, or any aircraft, reaches tiles it does not count. The
//! danger field (`battle_planner::DangerField`) floods each hostile's full
//! movement and reads aircraft by their operation range, and prices each
//! blow with the engine's own pair functions.
//!
//! Live Lv 8 Deity Korea, 2026-11-05: of 90 full-health units of ours killed
//! in an enemy phase across the anvil-spares-the-one-blow carriers, 50 had
//! moved that turn in the field and 52 of the 90 have no journal line naming
//! the move, the group step's signature. Their killers were Line Infantry,
//! Modern Armor, Jet Fighters, Musketmen and Machine Guns: units the
//! one-step forecast under-reaches.
//!
//! With the gene on, the three danger-memory checks of the group step read
//! the larger of the two estimates; the rule they feed
//! (`remember_dangerous_approach`: retreat when the damage would cross the
//! withdrawal floor) is unchanged. One field serves a frame: it is rebuilt
//! only when the visible hostile units at war change (identity, tile or
//! health). Off, the group step reads the one-step forecast alone.

use super::battle_planner::DangerField;
use super::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};

/// The danger field the group step reads this frame, keyed by the turn,
/// the seat and the visible hostile units at war. Cloning an `AdvancedAi`
/// starts it empty.
#[derive(Default)]
pub(super) struct GroupStepField(RefCell<Option<(u64, DangerField)>>);

impl Clone for GroupStepField {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// The key a cached field is good for: the turn, the seat, and every
/// visible hostile military unit at war with its tile and health.
fn field_key(g: &Game, pid: usize) -> u64 {
    let mut hostiles: Vec<(u32, Pos, i32)> = g
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.rules.units[unit.kind].class == "military"
                && g.unit_visible_to(unit.id, pid)
        })
        .map(|unit| (unit.id, unit.pos, unit.hp))
        .collect();
    hostiles.sort_unstable();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (g.turn, pid, hostiles).hash(&mut hasher);
    hasher.finish()
}

impl AdvancedAi {
    /// The danger the group step reads for `uid` entering `pos`: the
    /// one-step forecast `projected`, or with the gene on the larger of it
    /// and the danger field's reading there.
    pub(super) fn group_step_danger(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        projected: f64,
    ) -> f64 {
        if !self.group_steps_read_the_field {
            return projected;
        }
        let key = field_key(g, pid);
        let mut slot = self.group_step_field.0.borrow_mut();
        if slot.as_ref().is_none_or(|(held, _)| *held != key) {
            *slot = Some((key, DangerField::with_reach(g, pid, self.strike_reach)));
        }
        let (_, field) = slot.as_mut().expect("just filled");
        projected.max(field.danger(pos, uid))
    }
}

#[cfg(test)]
mod tests;
