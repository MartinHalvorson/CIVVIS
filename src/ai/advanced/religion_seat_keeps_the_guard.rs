//! `religion-seat-keeps-the-guard`: the existing Holy City guard stays near
//! its source between casts instead of taking an ordinary target during the
//! spread cooldown. Only a Religion-target founder opts in.
//!
//! Native Deity Russia `civvis-20261110T071444Z` guarded St. Petersburg at
//! t63 and t67, then moved its guard to Novgorod and spread there at t68.
//! Its last Missionary disappeared at t74 as its Holy City lost its faith.
//! The run retired under `religion_lost`: it is censored, not an engine loss.
//! The original frozen persistent agent reproduced both guards' orders in
//! all 36 sealed frames from t60 through t74 (626 byte-checked actual reads).
//!
//! This opt-in keeps the nearest eligible charged spreader returning to or
//! waiting beside the guarded city during the existing cooldown. It does
//! not shorten that cooldown or buy extra units. Existing danger avoidance,
//! healing and last-charge army/scout roles still run before the guard.
//! Holding a unit costs offensive work, especially when a host warning names
//! a distant conversion; native survival and victory benefit are unmeasured.

use super::{AdvancedAi, Game, VictoryTarget};

impl AdvancedAi {
    pub(super) fn religion_seat_keeps_the_guard_active(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_keeps_the_guard
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }
}
