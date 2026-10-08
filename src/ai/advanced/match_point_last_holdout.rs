//! `match-point-interception-ignores-power`: when a rival's founded faith
//! holds every other living major and we are its last holdout, the religious
//! interception (a war that condemns its spreader inside our borders) opens
//! at [`LAST_HOLDOUT_INTERCEPTION_FLOOR`] times the faith's steady power,
//! beside a running war too, instead of the army-sized gates.
//!
//! The interception asks no siege: its war exists so our soldiers may
//! condemn a spreader standing in our territory. The shipped gates price it
//! like a campaign: 1.0 times the faith's current power alone, and beside a
//! running war `religious-match-point-defence`'s 1.2 times its steady power
//! and no less than it and every enemy together. Live Emperor G425
//! (civvis-20261008T175442Z) held it on turns 93-94 at 282 against
//! Georgia's steady 273 (needs 1.2x) while France's war ran, and Georgia won
//! on Religion at 98. With the faith holding every other major and us still
//! out, the loss is otherwise certain; the guard against a city of ours under
//! threat and the war-affordability gate stay.
//!
//! Replay note: in G425 itself we were not the last holdout when the
//! spreader appeared. At turn 93 five of our nine cities followed Orthodoxy
//! (our majority) and Gaul's Hinduism held out until 97; before 93 Gaul and
//! we were both still out. So this gene would not have fired there; it
//! answers the case its description names, where we are the one left.
//!
//! Off: unchanged.

use super::*;

/// Times the faith's steady power (`steady_rival_power`) the interception
/// needs when we are the faith's last holdout.
pub(crate) const LAST_HOLDOUT_INTERCEPTION_FLOOR: f64 = 0.7;

impl AdvancedAi {
    /// Whether `rival`'s founded faith holds every living major other than
    /// us (and its founder) while our majority does not follow it.
    pub(super) fn we_are_the_last_holdout(g: &Game, pid: usize, rival: usize) -> bool {
        let Some(faith) = g.players[rival].religion.as_deref() else {
            return false;
        };
        !g.civ_follows_religion(pid, faith)
            && g.players
                .iter()
                .filter(|p| {
                    p.alive && !p.is_minor && !p.is_barbarian && p.id != pid && p.id != rival
                })
                .all(|p| g.civ_follows_religion(p.id, faith))
    }

    /// The gene's case: on, and we are `rival`'s faith's last holdout.
    pub(super) fn last_holdout_interception(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.match_point_interception_ignores_power && Self::we_are_the_last_holdout(g, pid, rival)
    }

    /// Our power against [`LAST_HOLDOUT_INTERCEPTION_FLOOR`] times the
    /// faith's steady power.
    pub(super) fn last_holdout_power_suffices(&self, g: &Game, pid: usize, rival: usize) -> bool {
        g.military_power(pid)
            >= LAST_HOLDOUT_INTERCEPTION_FLOOR * self.steady_rival_power(g, rival).max(1.0)
    }
}

#[cfg(test)]
mod tests;
