use super::*;

impl AdvancedAi {
    /// Forget only kills the host confirmed before this frame. A unit missing
    /// from the visible roster may still be in fog; a simulated kill may fail
    /// to land. Neither is evidence that its capture threat has disappeared.
    pub fn observe_confirmed_host_deaths(
        &mut self,
        g: &Game,
        state: &crate::mirror::StateSnapshot,
    ) {
        if state.confirmed_unit_deaths.is_empty() {
            return;
        }
        let mut seats =
            crate::mirror::host_major_seat_map(state, crate::mirror::modeled_major_player_count(g));
        for (minor, seat) in crate::mirror::minor_actor_assignments(g, state) {
            seats.insert(minor.player, seat);
        }
        // Firaxis's barbarian actor is player 63; unlike met majors/minors it
        // lives in hostiles[], so it need not appear in the current roster.
        if let Some(barbarian) = g.barb_pid {
            seats.insert(63, barbarian);
        }
        for death in &state.confirmed_unit_deaths {
            if death.turn > g.turn {
                continue;
            }
            let Some(&owner) = seats.get(&death.player).filter(|owner| **owner != 0) else {
                continue;
            };
            let remembered = self
                .hostile_last_seen
                .get(&death.unit)
                .is_some_and(|record| record.owner == owner && record.when <= death.turn);
            let at_turn_start = self
                .turn_start_hostiles_turn
                .is_some_and(|turn| turn <= death.turn)
                && self
                    .turn_start_hostiles
                    .iter()
                    .any(|hostile| hostile.owner == owner && hostile.host_key == death.unit);
            if !remembered && !at_turn_start {
                continue;
            }
            // A newer authoritative sighting wins over an old casualty. This
            // also prevents reused ids or a contradictory stale export from
            // deleting a threat that the current board explicitly contains.
            if g.units
                .values()
                .any(|unit| unit.owner == owner && hostile_memory_key(g, unit) == death.unit)
            {
                continue;
            }
            if remembered {
                self.hostile_last_seen.remove(&death.unit);
            }
            if at_turn_start {
                self.turn_start_hostiles
                    .retain(|hostile| hostile.owner != owner || hostile.host_key != death.unit);
            }
        }
    }
}

#[cfg(test)]
mod tests;
