//! Route production pays extra when it brings a real launch forward.
//! See docs/eval/2026-09-14-science-trade-production.md for strategy sources,
//! the bounded timing estimate, and evaluation limits.

use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::rules::Yields;

impl AdvancedAi {
    pub fn enable_trade_production_to_launch(&mut self) {
        self.trade_production_to_launch = true;
    }

    pub fn disable_trade_production_to_launch(&mut self) {
        self.trade_production_to_launch = false;
    }

    /// An empire-wide Science weight cannot distinguish production in a
    /// launch city from production elsewhere. Price the marginal turns saved
    /// at this route's origin, including work already invested and the local
    /// project multiplier. Existing routes are already in city_yields, so
    /// successive Traders see diminishing returns.
    pub(super) fn trade_production_to_launch_premium(
        &self,
        g: &Game,
        pid: usize,
        origin: Option<u32>,
        yields: Yields,
    ) -> f64 {
        if !self.trade_production_to_launch
            || !g.victory_conditions.science
            || yields.production <= 0.0
            || (g.players[pid].gold_per_turn < 0.0 && g.players[pid].gold < 100.0)
        {
            return 0.0;
        }
        let Some(city) = origin.and_then(|id| g.cities.get(&id)) else {
            return 0.0;
        };
        if city.owner != pid
            || city.loyalty < 76.0
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return 0.0;
        }
        let Some(item @ Item::Project { project }) = city.queue.first() else {
            return 0.0;
        };
        if !g.rules.projects.get(project).is_some_and(|spec| {
            spec.district
                .is_some_and(|district| g.district_family(district) == "spaceport")
        }) || !g.can_produce(pid, city.id, item)
        {
            return 0.0;
        }
        let production = g.city_yields(city.id).production;
        let multiplier = g.item_prod_mult(pid, city.id, Some(item));
        let remaining = g.item_remaining_cost_for_city(pid, city.id, item);
        if production <= 0.0 || multiplier <= 0.0 || remaining <= 0.0 {
            return 0.0;
        }
        let horizon = g.game_speed.scale(30.0).min(if g.max_turns > 0 {
            g.max_turns.saturating_sub(g.turn) as f64
        } else {
            f64::INFINITY
        });
        // A live project's quoted duration includes modifiers the native
        // model may not reproduce. Anchor there when available, and estimate
        // the new duration from the proportional change in origin production.
        let (without_route, with_route) = if let Some(turns) = g
            .host_production_turns(city.id, item)
            .filter(|turns| turns.is_finite() && *turns > 0.0)
        {
            (
                turns.ceil(),
                (turns * production / (production + yields.production)).ceil(),
            )
        } else {
            (
                (remaining / (production * multiplier)).ceil(),
                (remaining / ((production + yields.production) * multiplier)).ceil(),
            )
        };
        if with_route > horizon {
            return 0.0;
        }
        let without_route = without_route.min(horizon);
        // The same completion turn earns nothing. The bounded bonus cannot
        // turn an arbitrarily stalled project into an infinite route score.
        ((without_route - with_route).max(0.0) / g.game_speed.scale(1.0) * 4.0).min(32.0)
    }
}

#[cfg(test)]
mod tests;
