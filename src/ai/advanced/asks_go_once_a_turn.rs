//! `asks-go-once-a-turn` (opt-in, live bridge only): a delegation or
//! embassy, a friendship, a denouncement, an alliance, a deal with one rival
//! or a Trader's route that the seat sent in one frame of a host turn is not
//! sent again in a later frame of the same turn. The rule and its evidence
//! live with the order channel, `src/bin/civvis_orders/asks_once_a_turn.rs`;
//! this is the switch it reads. A native board has no frames and nothing here
//! changes it.

use super::*;

impl AdvancedAi {
    /// Whether `asks-go-once-a-turn` is on, for the live bridge.
    pub fn asks_go_once_a_turn_enabled(&self) -> bool {
        self.asks_go_once_a_turn
    }
}
