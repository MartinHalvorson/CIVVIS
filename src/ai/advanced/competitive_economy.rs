//! Competitive economic timing, independently screenable and off by default.
//! See docs/eval/2026-09-10-competitive-economy.md for sources and boundaries.

use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::name::Name;
use crate::rules::Yields;

impl AdvancedAi {
    /// Food has a concrete extra payoff when the next citizen releases a
    /// specialty district slot. Score turns saved inside a bounded investment
    /// window, using the actual origin (including the host's pair yields).
    /// International routes with food qualify too: the mechanism, not the
    /// route label or a particular governor's modded bonus, is what matters.
    pub(super) fn trade_growth_to_district_premium(
        &self,
        g: &Game,
        pid: usize,
        origin: Option<u32>,
        yields: Yields,
    ) -> f64 {
        if !self.trade_growth_to_district || yields.food <= 0.0 {
            return 0.0;
        }
        let Some(city) = origin.and_then(|id| g.cities.get(&id)) else {
            return 0.0;
        };
        // The first three extra slots are the economic development window.
        // A spare existing slot, housing or unhappiness bottleneck means food
        // cannot deliver the hypothesized district acceleration now.
        if city.owner != pid
            || !matches!(city.pop, 3 | 6 | 9)
            || g.city_specialty_district_count(city) < g.city_specialty_district_capacity(city)
            || g.city_housing_headroom(city) < 2.0
            || g.city_amenity_surplus(city) < 0
            || city.loyalty < 76.0
            || (g.players[pid].gold_per_turn < 0.0 && g.players[pid].gold < 100.0)
        {
            return 0.0;
        }
        let horizon = g.game_speed.scale(20.0);
        if g.max_turns > 0 && g.max_turns.saturating_sub(g.turn) as f64 <= horizon {
            return 0.0;
        }
        let remaining = (g.growth_cost(city.pop) - city.food).max(0.0);
        let surplus = g.city_yields(city.id).food - 2.0 * city.pop as f64;
        if remaining <= 0.0 || surplus + yields.food <= 0.0 {
            return 0.0;
        }
        let with_route = (remaining / (surplus + yields.food)).ceil();
        if with_route > horizon {
            return 0.0;
        }
        let without_route = if surplus > 0.0 {
            (remaining / surplus).ceil().min(horizon)
        } else {
            horizon
        };
        // A bounded tie-break on the existing per-route yield score. Speed
        // normalization keeps the same economic weight on Online and Standard.
        ((without_route - with_route).max(0.0) / g.game_speed.scale(1.0) * 2.0).min(24.0)
    }

    /// The completion half of the Feudalism wave. The base portfolio can
    /// protect a previously slotted economic card forever; a ready Builder
    /// needs Serfdom now. No speculative Builder queues or paid research delay.
    pub(super) fn builder_charge_window_card(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.builder_charge_window
            || (!g.has_policy(pid, "serfdom")
                && !g.available_policies(pid).contains(&crate::name!("serfdom")))
        {
            return None;
        }
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| {
                let Some(item @ Item::Unit { unit }) = g.cities[&cid].queue.first() else {
                    return false;
                };
                if unit != "builder" {
                    return false;
                }
                let rate = g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(item));
                rate > 0.0
                    && g.item_remaining_cost_for_city(pid, cid, item) / rate
                        <= g.game_speed.scale(3.0)
            })
            .then_some("serfdom")
    }

    pub(super) fn builder_window_can_replace(&self, g: &Game, pid: usize, current: &Name) -> bool {
        if g.rules.policies[current].slot != "economic"
            || matches!(
                current.as_str(),
                "serfdom" | "public_works" | "liberalism" | "civil_prestige" | "new_deal"
            )
        {
            return false;
        }
        // Do not finance the Builder wave by removing a live Settler wave's
        // production card. Typed slot legality remains the policy desk's job.
        !matches!(current.as_str(), "colonization" | "expropriation")
            || !g.player_city_ids(pid).into_iter().any(|cid| {
                matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit }) if unit == "settler")
            })
    }
}

#[cfg(test)]
mod tests;
