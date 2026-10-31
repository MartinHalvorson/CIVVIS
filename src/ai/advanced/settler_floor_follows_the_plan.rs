//! `settler-floor-follows-the-plan`: past the band turn the early-settler
//! floor keeps queuing a Settler in the city that trains it soonest until the
//! plan's own city target is met.
//!
//! `early-settler-floor` actuates the opening. Before the band turn, while the
//! empire is short of six cities and walkers, the unthreatened city that
//! trains a Settler soonest puts one at the head of its queue, and every
//! routine claim yields to it. Past the band turn it stops, and the next
//! Settler waits on the strategic scorer, which in a grown empire prices
//! districts above it in every big city. The Settlers that do get built come
//! from whichever small city has an idle queue.
//!
//! Live Immortal 6p Continents, five armed `land-grab-outlasts-the-opening`
//! seats of 2026-10-31, turns 80-150 at two or more cities short of the
//! plan's target (223 turns):
//! - Only 0.62 cities a turn were building a Settler, with a pipeline width
//!   of four.
//! - 27% of those builds were in the top-production city. A Settler took a
//!   median 6.3 turns there and 14.5 anywhere else.
//! - civvis-20261031T111516Z wanted 13-15 cities from t82 and held 6 to
//!   t108. Only Gondar (pop 2-5) built a Settler, from t96, while Addis Ababa
//!   (pop 9-11) and Axum built Industrial Zones, Workshops and a University.
//!
//! With the gene on, past the band turn the floor is the plan's own
//! (room-capped) target, and the same claim runs. That means one Settler in
//! production at a time, at most one walker out, the fastest unthreatened
//! city, local defence first, and nothing while a city is threatened or a
//! major war finds the army short. Two late-game gates are added: the city
//! must still be inside the Settler's payback window, and the site gate must
//! hold a seat for it, so the floor never builds a Settler that would
//! strand.
//!
//! Off, or with `early-settler-floor` off: unchanged.
use super::*;

impl AdvancedAi {
    /// The floor past the band turn: the plan's target under the gene, else
    /// zero (the shipped stop).
    pub(super) fn plan_settler_floor(&self, plan: &StrategicPlan) -> usize {
        if self.settler_floor_follows_the_plan {
            plan.desired_cities.max(1)
        } else {
            0
        }
    }

    /// Whether the site gate holds a seat for one more Settler from `cid`.
    /// The claim asks it of its candidates soonest first, after the cheap
    /// filters (the payback window among them), so the scan usually runs once.
    pub(super) fn plan_settler_floor_has_a_seat(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        settlers: usize,
    ) -> bool {
        self.settler_site_gate(g, pid, g.cities[&cid].pos, settlers)
            .is_ok()
    }
}

#[cfg(test)]
mod tests;
