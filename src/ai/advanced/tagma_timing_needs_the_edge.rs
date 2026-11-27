//! `tagma-timing-needs-the-edge`: Basil's Tagma timing takes the empire's plan
//! to Conquest only with the army an elective war asks for.
//!
//! Shipped, `assess` reads Byzantium's timing arm as soon as Taxis holds a
//! religion or Divine Right: "Tagma timing is live", Conquest, whatever the
//! army is. It sits above the assigned lane, above `elective-war-yields-to-a-lane`
//! and above the power bar the elective war uses ("strong enough to take what
//! a neighbour has": more than 1.8 times the weakest rival plus 20). On a
//! Religion seat the religion founds and the very next assessment hands the
//! plan to a war the army cannot fight.
//!
//! The Indefinite Benchmarking Tourney's reviewer named it in 4 of 40 Lv 8
//! games on 2026-11-26 (`religion-lane-abandoned-for-futile-conquest`):
//! - 101919Z: Buddhism at t27, "Tagma timing is live" at t31 with power 66
//!   against the strongest rival's 377, again at t99 (560 against 783);
//! - 120405Z: founded t39, Conquest at t43 (63 against 517) and t74;
//! - 140214Z: founded t23, Conquest at t26 (34 against 376) to t220;
//! - 144622Z: founded t29, Conquest at t33 (72 against 316) to the end;
//! - conquest marks read 0 cities and 0 capitals taken at t100 and t150 in
//!   all four, and all four lost to a rival's victory.
//!
//! With the gene on the timing still reads its own conditions, and also asks
//! for that elective-war bar against the weakest rival (the one in campaign
//! reach when `elective-war-in-reach` is armed; none in reach holds it). Held,
//! the plan falls through to the lane. Tagma production does not read the
//! plan, so the army keeps growing and the timing fires the assessment it
//! clears the bar. Off, nothing moves.

use super::*;

/// The elective war's bar, as `assess` writes it for "strong enough to take
/// what a neighbour has": our power over this multiple of the weakest rival's
/// plus [`TAGMA_EDGE_FLOOR`].
pub(super) const TAGMA_EDGE_RATIO: f64 = 1.80;
/// See [`TAGMA_EDGE_RATIO`].
pub(super) const TAGMA_EDGE_FLOOR: f64 = 20.0;

/// Whether `my_power` clears the elective war's bar against `weakest_rival`.
/// An infinite `weakest_rival` (nobody to march on) never does.
pub(super) fn tagma_has_the_edge(my_power: f64, weakest_rival: f64) -> bool {
    my_power > weakest_rival * TAGMA_EDGE_RATIO + TAGMA_EDGE_FLOOR
}

impl AdvancedAi {
    /// See the module: the shipped Tagma timing, kept only with the edge when
    /// the gene is on. Returns `timing` unchanged with the gene off.
    pub(super) fn tagma_timing_with_the_edge(
        &self,
        timing: bool,
        my_power: f64,
        weakest_rival: f64,
    ) -> bool {
        if !timing || !self.tagma_timing_needs_the_edge {
            return timing;
        }
        let edge = tagma_has_the_edge(my_power, weakest_rival);
        if !edge {
            let bar = weakest_rival * TAGMA_EDGE_RATIO + TAGMA_EDGE_FLOOR;
            think!(self.journal(), Strategy, Detail,
                "Tagma timing waits for the edge";
                "tagma-timing-needs-the-edge: power {my_power:.0} against the bar {bar:.0} \
                 the weakest rival sets; the lane keeps the plan");
        }
        edge
    }
}

#[cfg(test)]
mod tests;
