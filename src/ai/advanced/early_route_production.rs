//! `early-route-feeds-production`: before the t80 production gate, a Trader's
//! route is priced for the Production it brings home.
//!
//! A live rule retires a seat whose Production stays under a quarter of the
//! best rival's for three turns from t80. On the 6p Immortal lane, 16 of 20
//! seats died there, at a median share of 0.23, while both Lv 7 wins held
//! 0.35-0.47. The share counts raw yield only.
//!
//! The first Trader (around t21) went foreign or to a city-state for about 4
//! Gold in kept and retired seats alike (81 Immortal seats, 10-30/31): no
//! Production. `commercial-hub-and-traders` (live) adds 1.5 per Gold to a
//! foreign route, so a 5-Gold route scored about 12 under the Expansion
//! weights. A domestic route's +1 Food +1 Production, or +3 Production into
//! the city Magnus's Surplus Logistics serves, scored 4-7.
//!
//! With the gene on, within [`EARLY_ROUTE_STANDARD_TURNS`] (about 100 Online
//! turns) the foreign-Gold premium is dropped and each point of Production
//! the host quotes for the route adds [`EARLY_ROUTE_PRODUCTION_PREMIUM`]. A
//! domestic route then wins, and one into a Surplus Logistics city wins by
//! most. Later routes are priced as before. Off, nothing changes.

use super::*;

/// Standard turns from the start in which a route is priced for Production:
/// about 100 at Online speed, past the t80 gate.
pub(crate) const EARLY_ROUTE_STANDARD_TURNS: u32 = 150;

/// Value added per point of route Production inside the window: about twice
/// the Expansion weight for Production, so +1 Production outbids the 1.5
/// Gold premium it replaces.
pub(crate) const EARLY_ROUTE_PRODUCTION_PREMIUM: f64 = 4.0;

impl AdvancedAi {
    /// Whether the early-route window is open. `false` with the gene off.
    pub(super) fn early_route_window(&self, g: &Game) -> bool {
        self.early_route_feeds_production
            && g.turn <= g.standard_duration(EARLY_ROUTE_STANDARD_TURNS)
    }

    /// The premium for a route yielding `yields` inside the window: zero
    /// outside it or with the gene off.
    pub(super) fn early_route_production_premium(&self, g: &Game, yields: Yields) -> f64 {
        if !self.early_route_window(g) {
            return 0.0;
        }
        EARLY_ROUTE_PRODUCTION_PREMIUM * yields.production.max(0.0)
    }
}

#[cfg(test)]
mod tests;
