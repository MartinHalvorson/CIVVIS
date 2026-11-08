//! Keep the first host observation of a war across fresh-board rebuilds.
//! A reconstructed war ledger otherwise opens at the current model turn,
//! repeatedly making an established native war too young for peace.

use civvis::mirror::{host_major_seat_map, LiveMirror, StateSnapshot};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct History {
    since: BTreeMap<usize, u32>,
}

impl History {
    pub(super) fn observe_and_apply(&mut self, board: &mut LiveMirror, state: &StateSnapshot) {
        let active: BTreeSet<_> = state
            .rivals
            .iter()
            .filter(|rival| rival.at_war)
            .map(|rival| rival.player)
            .collect();
        self.since.retain(|host, _| active.contains(host));
        for host in active {
            self.since.entry(host).or_insert(state.turn);
        }

        let major_count = board
            .game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian && !player.is_free_city)
            .count();
        let seats = host_major_seat_map(state, major_count);
        board.game.sync_war_log();
        for (host, since) in &self.since {
            let Some(&seat) = seats.get(host).filter(|seat| **seat != 0) else {
                continue;
            };
            if let Some(war) = board.game.wars.get_mut(&(0, seat)) {
                // First observation is conservative if the process joins an
                // existing war. Never infer an earlier, hidden declaration.
                war.started = *since;
            }
        }
    }
}

#[cfg(test)]
#[path = "host_war_age/tests.rs"]
mod tests;
