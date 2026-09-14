//! `first-granary-reserve-2`: reserve housing for growth it can deliver soon.
//!
//! V1 only checks population against housing. V2 compares the next citizen's
//! arrival with and without the housing lift, using the engine's growth rule
//! and the actual build bill. Normal production can still choose a Granary
//! outside this window; this gene decides when it deserves a reservation.

use super::{AdvancedAi, StrategicPlan};
use crate::game::{Game, Item};

/// The forced investment must deliver a citizen within thirty Standard turns.
/// Longer investments retain their ordinary production bid.
const GRANARY_GROWTH_WINDOW: f64 = 30.0;
/// A reservation must advance growth by at least two Standard turns.
const GRANARY_MIN_GROWTH_SAVING: f64 = 2.0;

/// Current-yield forecast, stopping at the first citizen. If the city would
/// grow during construction its population and yields change, so leave that
/// near-term growth alone and reassess after it happens.
fn growth_arrivals(need: f64, current: f64, improved: f64, build: f64) -> Option<(f64, f64)> {
    if !need.is_finite()
        || !current.is_finite()
        || !improved.is_finite()
        || !build.is_finite()
        || need <= 0.0
        || current < 0.0
        || improved <= current
        || build < 0.0
    {
        return None;
    }
    let without = if current > 0.0 {
        (need / current).ceil()
    } else {
        f64::INFINITY
    };
    if without <= build {
        return None;
    }
    // Growth precedes production completion in process_city, so the build's
    // final turn still accrues the old rate. The new rate begins next turn.
    let with = build + ((need - build * current) / improved).ceil().max(1.0);
    Some((without, with))
}

impl AdvancedAi {
    pub(super) fn granary_growth_pays(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.first_granary_reserve_2 || plan.threatened_city == Some(cid) {
            return false;
        }
        let city = &g.cities[&cid];
        let granary = crate::name!("granary");
        let item = Item::Building { building: granary };
        if city.owner != pid
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || city.buildings.contains(&granary)
            || !g.can_produce(pid, cid, &item)
        {
            return false;
        }
        let _memo = g.query_memo();
        let housing = g.city_housing(city);
        let housing_gain = g.rules.buildings[granary].housing;
        if housing - city.pop as f64 > 1.0 || housing_gain <= 0.0 {
            return false;
        }
        let yields = g.city_yields(cid);
        let production = yields.production * g.item_prod_mult(pid, cid, Some(&item));
        if production <= 0.0 || yields.food <= 2.0 * city.pop as f64 {
            return false;
        }
        let amenities = g.city_amenity_surplus(city);
        let current = g.city_growth_surplus(pid, cid, yields.food, housing, amenities);
        // Count only the housing lift: the Granary's extra food and changed
        // citizen assignment may improve this forecast but are not required.
        let improved =
            g.city_growth_surplus(pid, cid, yields.food, housing + housing_gain, amenities);
        let build = (g.item_remaining_cost_for_city(pid, cid, &item) / production)
            .ceil()
            .max(1.0);
        let need = g.growth_cost(city.pop) - city.food;
        let Some((without, with)) = growth_arrivals(need, current, improved, build) else {
            return false;
        };
        let mut window = g.game_speed.scale(GRANARY_GROWTH_WINDOW);
        if g.max_turns > 0 {
            window = window.min(g.max_turns.saturating_sub(g.turn) as f64);
        }
        with <= window && without - with >= g.game_speed.scale(GRANARY_MIN_GROWTH_SAVING)
    }
}

#[cfg(test)]
mod tests;
