//! Reserve early housing only when its projected Production repays the bill.
//!
//! Compare construction with waiting on independent board clones. Recompute
//! citizen assignments, consumption, housing and amenity allocation as the
//! target grows; measure the whole empire so displaced luxuries are not free.
//! Other cities, research, borders, improvements and military actions remain
//! fixed. This is a short investment forecast, not a future-game simulation.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Game, Item};

/// A growth investment must return its Production within sixty cost-scaled
/// Standard turns (thirty Online), including its construction time.
const GROWTH_PRODUCTION_WINDOW: f64 = 60.0;
const GROWTH_RETURN_MARGIN: f64 = 1.10;

struct GrowthProjection {
    production: f64,
    population: i32,
    completed: bool,
}

fn growth_production_projection(
    original: &Game,
    pid: usize,
    cid: u32,
    item: Option<&Item>,
    window: u32,
) -> Option<GrowthProjection> {
    let mut g = original.clone();
    let mut bill = item.map_or(0.0, |i| original.item_remaining_cost_for_city(pid, cid, i));
    if !bill.is_finite() || bill < 0.0 {
        return None;
    }
    let city = g.cities.get_mut(&cid)?;
    city.queue.clear();
    if let Some(item) = item {
        city.queue.push(item.clone());
    }
    let mut production = 0.0;
    let mut completed = item.is_none();
    for step in 0..window {
        // City income is paid at the beginning of its next player turn.
        // Expire known timed effects at that boundary before reading yields.
        g.turn = original.turn.saturating_add(step).saturating_add(1);
        let (earned, food, construction) = {
            let _memo = g.query_memo();
            let city = &g.cities[&cid];
            let yields = g.city_yields(cid);
            let food = g.city_growth_surplus(
                pid,
                cid,
                yields.food,
                g.city_housing(city),
                g.city_amenity_surplus(city),
            );
            let construction = yields.production * g.item_prod_mult(pid, cid, city.queue.first());
            let earned: f64 = g
                .player_city_ids(pid)
                .iter()
                .map(|id| g.city_yields(*id).production)
                .sum();
            (earned, food, construction)
        };
        if !earned.is_finite() || !food.is_finite() || !construction.is_finite() {
            return None;
        }
        production += earned;
        // process_city accrues this turn's food before completing production.
        // A Granary completed here changes assignments and growth next turn.
        let need = g.growth_cost(g.cities[&cid].pop);
        let city = g.cities.get_mut(&cid)?;
        city.food += food;
        if city.food >= need {
            city.food -= need;
            city.pop += 1;
        } else if city.food < 0.0 {
            city.food = 0.0;
            city.pop = (city.pop - 1).max(1);
        }
        if !completed {
            bill -= construction;
            if bill <= 0.0 {
                let Item::Building { building } = item? else {
                    return None;
                };
                city.buildings.push(*building);
                city.queue.clear();
                city.production = 0.0;
                completed = true;
            }
        }
    }
    Some(GrowthProjection {
        production,
        population: g.cities[&cid].pop,
        completed,
    })
}

impl AdvancedAi {
    pub(super) fn profitable_growth_foundation(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, f64)> {
        // The current experiment prices the actual Domination seat. Other
        // lanes and adaptive production retain their existing investment bids.
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy == GrandStrategy::Recovery
            || g.turn > g.standard_duration(140)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let granary = crate::name!("granary");
        let item = Item::Building { building: granary };
        let cities = g.player_city_ids(pid);
        if cities.len() < 2
            || city.owner != pid
            || !city.queue.is_empty()
            || city.buildings.contains(&granary)
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || city.loyalty < 75.0
            || g.city_amenity_surplus(city) < 0
            || g.players[pid].gold_per_turn < g.rules.buildings[granary].maintenance
            || !g.can_produce(pid, cid, &item)
            || cities.iter().any(|id| g.cities[id].queue.contains(&item))
        {
            return None;
        }
        let fielded = g
            .player_unit_ids(pid)
            .iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind.as_str()];
                spec.class == "military"
                    && spec.domain.as_deref().unwrap_or("land") == "land"
                    && (spec.strength > 0.0 || spec.ranged_strength > 0.0)
            })
            .count();
        if fielded < cities.len()
            || g.units.iter().any(|(uid, unit)| {
                g.is_at_war(pid, unit.owner)
                    && g.unit_visible_to(*uid, pid)
                    && g.map.distance(unit.pos, city.pos) <= 3
                    && g.rules.units[unit.kind.as_str()].class == "military"
            })
            || (city.is_capital
                && plan.strategy == GrandStrategy::Expansion
                && cities.len() < plan.desired_cities
                && self.counts(g, pid).settlers == 0)
        {
            return None;
        }
        let window = g.game_speed.scale(GROWTH_PRODUCTION_WINDOW).ceil() as u32;
        let window = if g.max_turns > 0 {
            window.min(g.max_turns.saturating_sub(g.turn))
        } else {
            window
        };
        let cost = g.item_remaining_cost_for_city(pid, cid, &item);
        let control = growth_production_projection(g, pid, cid, None, window)?;
        let candidate = growth_production_projection(g, pid, cid, Some(&item), window)?;
        let gain = candidate.production - control.production;
        if candidate.completed
            && candidate.population >= control.population
            && gain > 0.0
            && gain >= cost * GROWTH_RETURN_MARGIN
        {
            Some((item, gain, cost))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
