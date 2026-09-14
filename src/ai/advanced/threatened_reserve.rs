//! `threatened-city-reserve-2`: hold the actual bill for a local defender.
//!
//! V1 reserves the dearest ranged unit buildable anywhere, at four times its
//! production cost. V2 asks what a threatened city can buy, including its
//! discounts and refusals. Siege and recon units do not set a garrison bill.

use super::{AdvancedAi, StrategicPlan};
use crate::game::Game;
use crate::name::Name;

impl AdvancedAi {
    /// A strongest available ordinary land defender; lower price breaks a
    /// strength tie. This is a reserve quote, so affordability is not required.
    fn local_defender_quote(g: &Game, pid: usize, cid: u32) -> Option<f64> {
        let mut best: Option<(f64, f64, Name)> = None;
        for (name, spec) in &g.rules.units {
            if spec.class != "military"
                || !matches!(spec.domain.as_deref(), None | Some("land"))
                || spec.siege
                || spec.promotion_class == "recon"
                || !(spec.has_ranged_attack() || spec.is_melee_capable())
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, name.as_str(), "gold") else {
                continue;
            };
            if !price.is_finite() || price < 0.0 {
                continue;
            }
            let power = spec.strength.max(spec.ranged_attack_strength());
            if best.is_none_or(|(old_power, old_price, old_name)| {
                power
                    .total_cmp(&old_power)
                    .then_with(|| old_price.total_cmp(&price))
                    .then_with(|| old_name.cmp(name))
                    .is_gt()
            }) {
                best = Some((power, price, *name));
            }
        }
        best.map(|(_, price, _)| price)
    }

    pub(super) fn local_defender_gold_floor(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> f64 {
        if !self.threatened_city_reserve_2 {
            return 0.0;
        }
        let _memo = g.query_memo();
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                plan.threatened_city == Some(*cid) || self.native_city_emergency_on(g, pid, *cid)
            })
            .filter_map(|cid| Self::local_defender_quote(g, pid, cid))
            // Preserve the one-defender scope. Other stock and war reserves
            // remain binding through reserve_for_the_threatened_city.
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests;
