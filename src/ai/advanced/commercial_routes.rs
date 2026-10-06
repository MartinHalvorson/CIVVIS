//! `commercial-hub-and-traders`, the route half: at peace with every major, a
//! Trader's route to a city-state or to a rival we are not targeting is
//! priced with a Gold premium, so the chooser takes the international Gold
//! route over a domestic Food and Production one. The production half (hub,
//! Market, Trader) lives in the delegated governor; see
//! `BasicAi::commercial_hub_and_traders`.
//!
//! The stock chooser (`trade_route_destination_value_from`) prices a route's
//! yields at the posture's weights: Gold at 0.9-1.4 against Production at
//! 2.0-2.8, so a domestic route's Food and Production outbid an
//! international route's Gold. Live Emperor G185-G198 ran at 0.15x the best
//! rival's Gold a turn by t100 with unit upkeep eating 45-65% of the cities'
//! gross Gold, and the AIs gold-buy walls and units the turn they are
//! threatened while our own purchases are Gold-limited. At war with a major
//! the stock pricing stands: the domestic route's Production feeds the army,
//! and a route into a rival's land is a target.

use super::*;

/// Value added per Gold a turn of a route to a city-state or an untargeted
/// rival while we are at peace with every major: lifts Gold from the
/// posture's 0.9-1.4 to about 2.4-2.9, level with Production.
pub(super) const INTERNATIONAL_ROUTE_GOLD_PREMIUM: f64 = 1.5;

impl AdvancedAi {
    /// `commercial-hub-and-traders`: the Gold premium of a route to a city of
    /// `owner` yielding `yields` -- `INTERNATIONAL_ROUTE_GOLD_PREMIUM` a Gold
    /// for a city-state's city or a major's we are neither at war with nor
    /// targeting (the plan's or the appointed war's target), while we are at
    /// war with no major. Zero for our own cities and with the gene off.
    pub(super) fn international_gold_route_premium(
        &self,
        g: &Game,
        pid: usize,
        owner: usize,
        yields: Yields,
    ) -> f64 {
        if !self.commercial_hub_and_traders || owner == pid {
            return 0.0;
        }
        let Some(player) = g.players.get(owner) else {
            return 0.0;
        };
        if player.is_barbarian || g.is_at_war(pid, owner) {
            return 0.0;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        if at_major_war {
            return 0.0;
        }
        let targeted = self.plan.as_ref().and_then(|plan| plan.target_player) == Some(owner)
            || self.war_plan.as_ref().map(|war| war.target_player) == Some(owner);
        if !player.is_minor && targeted {
            return 0.0;
        }
        INTERNATIONAL_ROUTE_GOLD_PREMIUM * yields.gold.max(0.0)
    }
}

#[cfg(test)]
mod tests;
