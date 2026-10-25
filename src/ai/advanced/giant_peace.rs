//! `the-giant-gets-the-peace-offer`: a major at war with us at
//! [`GIANT_PEACE_RATIO`] times our military power or more, that holds a city
//! founded by us or while a city of ours is falling, counts as outmatching us
//! in the peace gate, so peace is offered to it.
//!
//! Under `war-policy-via-board` the gate's `outmatched` reading is the war
//! board's `war_policy_peace`, and that peace's tide clause skips the one-war
//! front (`one_war_owns_tide`). The war the seat is losing outright is the
//! front, so it is the one war no offer reached.
//!
//! **Why (live Immortal 2026-10-25, 17 games).** 5 collapsed to one city.
//! - In civvis-20261025T190431Z France declared at turn 46 and stood at 2,461
//!   against our 132 by 182. The journal offered Egypt and China peace 164
//!   times and France never.
//! - 191538Z (England 1,794 against 68), 165306Z (Rome about 2.9 times) and
//!   170120Z (Gaul 1,247 against 143) went the same way.
//! - The host does make peace with a stronger rival: 33 of the 156 peace
//!   transitions in the retained games came with the rival at twice our power
//!   or more.
//!
//! Off, nothing changes.

use super::*;

/// A rival at this multiple of our military power is a giant.
pub(super) const GIANT_PEACE_RATIO: f64 = 2.5;

impl AdvancedAi {
    /// Whether `rival` is a giant the peace gate offers peace: see the
    /// module. Always false with the gene off.
    pub(super) fn giant_gets_the_peace_offer_from(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        self.the_giant_gets_the_peace_offer
            && g.players
                .get(rival)
                .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
            && g.is_at_war(pid, rival)
            && g.military_power(rival) >= GIANT_PEACE_RATIO * g.military_power(pid).max(1.0)
            && (Self::our_cities_falling(g, pid)
                || g.cities
                    .values()
                    .any(|city| city.owner == rival && city.original_owner == pid))
    }
}

#[cfg(test)]
mod tests;
