//! Preserve the model's observed ranged-unit history across fresh native boards.
//!
//! This is an estimate from unit types we actually saw, not a cached exact city
//! reading. Unknown native city readings remain unknown; current exact readings
//! still outrank the model in `Game::city_ranged_strength`.

use std::collections::BTreeMap;

use civvis::mirror::{host_major_seat_map, modeled_major_player_count, LiveMirror, StateSnapshot};

#[derive(Default)]
pub(super) struct History {
    // Host ids survive met-gated roster changes; civilization guards identity.
    strongest: BTreeMap<(usize, String), i64>,
    seat: Option<(i32, String, String, usize)>,
    turn: Option<u32>,
}

impl History {
    /// Call on the imported board BEFORE deciding, never after simulated actions.
    /// Read mapped real units, not counters the planner may have mutated.
    pub(super) fn observe_and_apply(&mut self, board: &mut LiveMirror, state: &StateSnapshot) {
        let seat = (
            state.seat.local_player,
            state.seat.civ.clone(),
            state.seat.leader.clone(),
            state.seat.players,
        );
        if self.seat.as_ref().is_some_and(|previous| *previous != seat)
            || self.turn.is_some_and(|previous| state.turn < previous)
        {
            self.strongest.clear();
        }
        self.seat = Some(seat);
        self.turn = Some(state.turn);

        let seats = host_major_seat_map(state, modeled_major_player_count(&board.game));
        let identities = usize::try_from(state.seat.local_player)
            .ok()
            .map(|host| (host, state.seat.civ.as_str()))
            .into_iter()
            .chain(
                state
                    .rivals
                    .iter()
                    .map(|rival| (rival.player, rival.civ.as_str())),
            );
        for (host, civ) in identities {
            let Some(&owner) = seats.get(&host) else {
                continue;
            };
            if owner >= board.game.players.len() {
                continue;
            }
            self.strongest.retain(|(previous_host, previous_civ), _| {
                *previous_host != host || previous_civ == civ
            });
            let observed = board
                .uid_of
                .values()
                .chain(board.foreign_uid_of.values())
                .filter_map(|uid| board.game.units.get(uid))
                .filter(|unit| unit.owner == owner)
                .map(|unit| &board.game.rules.units[unit.kind])
                .filter(|spec| spec.class == "military")
                // Nominal type strength only: no formation, promotion, damage,
                // terrain, policy or current attack bonuses become history.
                .map(|spec| spec.ranged_strength.round() as i64)
                .max()
                .unwrap_or(0);
            let remembered = self.strongest.entry((host, civ.to_owned())).or_default();
            *remembered = (*remembered).max(observed);
            if *remembered > 0 {
                let counter = board.game.players[owner]
                    .counters
                    .entry("strongest_ranged_built".into())
                    .or_default();
                *counter = (*counter).max(*remembered);
            }
        }
    }
}

#[cfg(test)]
#[path = "host_ranged_history/tests.rs"]
mod tests;
