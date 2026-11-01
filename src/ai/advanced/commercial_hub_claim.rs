//! `commercial-hub-claims-the-route-slot`: from [`HUB_CLAIM_FROM_STANDARD`],
//! while our trade routes and Traders fill the route capacity, the city the
//! Commercial Hub step would pick (`BasicAi::commercial_hub_item`) starts its
//! hub, after every other claim.
//!
//! **Why (live 6p lane, 2026-10-31/11-01).**
//! - The first non-Diplomatic win of the lane had the route capacity the
//!   others never got. civvis-20261101T062425Z (João III, King, Science) ran
//!   3 routes at t60, 4 at t100 and 6 at t150 (Portugal's own bonus), with
//!   Science 108 against the best rival's 86 at t100. Every other King seat
//!   sat at a capacity of 1, full, until about t150, with Science 26-64.
//! - Nobody builds the hub that raises it. Across 48 lane seats since 10-31
//!   the empire held 0.00 Commercial Hubs at t60, 0.42 at t100 (9 of 48
//!   seats had one) and 1.81 at t150; capacity ran 1.12, 1.33, 2.68.
//! - The hub step (`commercial-hub-and-traders` with
//!   `commercial-hub-in-the-strategic-queue`, both live) opens only at
//!   standard t90, about t60 Online, and runs only in idle queues, which the
//!   claim genes fill first: the same failure that kept pillaged districts
//!   unrepaired (`pillaged-districts-repair-first`). Its own notes put the
//!   first hub a median 55 turns after the step opens.
//!
//! **What the gene does.** From [`HUB_CLAIM_FROM_STANDARD`] (about t40
//! Online), with at least three cities and every route slot taken by a route
//! or a Trader, the first of our cities `commercial_hub_item` would choose
//! (the most productive unthreatened city without a hub that builds it
//! within twenty turns, up to the step's own two or three hubs) starts it,
//! after every other claim. A head the early Settler floor holds, or one the
//! Theater claim's rule keeps (finishing this turn, a soldier at war), keeps
//! its queue; any other head keeps its progress behind the hub. One hub a
//! turn.
//!
//! Off, nothing changes.

use super::*;

/// Standard turns before the claim opens: about t40 at Online speed, two
/// thirds of the hub step's own `COMMERCIAL_HUB_FROM_STANDARD_TURN`.
pub(super) const HUB_CLAIM_FROM_STANDARD: u32 = 60;

/// The fewest cities the claim acts with, as the hub step's own floor.
pub(super) const HUB_CLAIM_MIN_CITIES: usize = 3;

impl AdvancedAi {
    /// Whether every route slot is taken by a route or a Trader.
    fn route_capacity_full(&self, g: &Game, pid: usize) -> bool {
        let traders = self.counts(g, pid).traders as i64;
        g.active_routes(pid) + traders >= g.trade_capacity(pid)
    }

    /// The city and hub the claim wants now. `None` with the gene off, before
    /// the window, below three cities, with a route slot open, or when no
    /// city is the hub step's pick.
    pub(super) fn route_slot_hub(&self, g: &Game, pid: usize) -> Option<(u32, Item)> {
        if !self.commercial_hub_claims_the_route_slot
            || g.turn < g.standard_duration(HUB_CLAIM_FROM_STANDARD)
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() < HUB_CLAIM_MIN_CITIES || !self.route_capacity_full(g, pid) {
            return None;
        }
        cities.into_iter().find_map(|cid| {
            self.base
                .commercial_hub_item(g, pid, cid, g.player_city_ids(pid).len())
                .map(|hub| (cid, hub))
        })
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn claim_route_slot_hub(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((cid, hub)) = self.route_slot_hub(g, pid) else {
            return;
        };
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let head = g.cities[&cid].queue.first().cloned();
        if let Some(item) = &head {
            if self.early_settler_floor_holds(g, pid, cid, plan)
                || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
            {
                return;
            }
        }
        let name = g.cities[&cid].name.clone();
        let (routes, capacity) = (g.active_routes(pid), g.trade_capacity(pid));
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: hub.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} opens a Commercial Hub for a route slot";
                   "commercial-hub-claims-the-route-slot: {routes} routes against a capacity of \
                    {capacity}, every slot taken; ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
