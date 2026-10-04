//! Reserve early housing when its additional citizens repay the production bill.
//!
//! Current improvements, other city populations and future actions stay fixed.
//! Citizen assignments are explicit model counterfactuals, including on a host
//! mirror; an observed worked-plot list is not a future population assignment.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Game, Item};

const CAPACITY_WINDOW: f64 = 60.0;
const CAPACITY_RETURN_MARGIN: f64 = 1.10;
const CAPACITY_GOLD_BUFFER: f64 = 20.0;

pub(super) struct CapacityInvestment {
    pub item: Item,
    pub returned: f64,
    pub cost: f64,
}

struct CapacityProjection {
    production: f64,
    population: i32,
    completed: bool,
}

fn capacity_projection(
    original: &Game,
    pid: usize,
    cid: u32,
    item: Option<&Item>,
    window: u32,
) -> Option<CapacityProjection> {
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
        g.turn = original.turn.saturating_add(step).saturating_add(1);
        let (earned, food, construction) = {
            let _memo = g.query_memo();
            let city = &g.cities[&cid];
            let weights = g.city_citizen_plan(cid).strategy.weights;
            let yields = g.city_yields_weighted(cid, weights);
            let food = g.city_growth_surplus(
                pid,
                cid,
                yields.food,
                g.city_housing(city),
                g.city_amenity_surplus(city),
            );
            let mut construction =
                yields.production * g.item_prod_mult(pid, cid, city.queue.first());
            if original.observed_city_worked_tiles.contains_key(&cid) {
                // A model assignment cannot promise that the host will move
                // its citizens during construction. Cap its build rate at the
                // currently observed production; future work remains a forecast.
                let observed =
                    original.city_yields(cid).production * original.item_prod_mult(pid, cid, item);
                construction = construction.min(observed);
            }
            let earned = g
                .player_city_ids(pid)
                .iter()
                .map(|other| {
                    if *other == cid {
                        yields.production
                    } else {
                        g.city_yields(*other).production
                    }
                })
                .sum::<f64>();
            (earned, food, construction)
        };
        if !earned.is_finite() || !food.is_finite() || !construction.is_finite() {
            return None;
        }
        production += earned;
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
        // The engine grows before completing production. Housing and its new
        // assignments begin contributing on the following player turn.
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
    Some(CapacityProjection {
        production,
        population: g.cities[&cid].pop,
        completed,
    })
}

impl AdvancedAi {
    pub(super) fn early_growth_capacity(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<CapacityInvestment> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy == GrandStrategy::Recovery
            || g.turn > g.standard_duration(150)
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
            || g.city_housing(city) - city.pop as f64 > 1.0
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || city.loyalty < 75.0
            || g.city_amenity_surplus(city) < -2
            || g.players[pid].bankruptcy_amenity_penalty > 0
            || !g.can_produce(pid, cid, &item)
            || cities
                .iter()
                .any(|other| g.cities[other].queue.contains(&item))
        {
            return None;
        }
        let counts = self.counts(g, pid);
        if city.is_capital
            && plan.strategy == GrandStrategy::Expansion
            && cities.len() < plan.desired_cities
            && counts.settlers == 0
        {
            return None;
        }
        let fielded = g
            .player_unit_ids(pid)
            .iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                spec.class == "military"
                    && spec.domain.as_deref().unwrap_or("land") == "land"
                    && (spec.strength > 0.0 || spec.ranged_strength > 0.0)
            })
            .count();
        if fielded < cities.len()
            || g.units.iter().any(|(uid, unit)| {
                g.is_at_war(pid, unit.owner)
                    && g.unit_visible_to(*uid, pid)
                    && g.wdist(unit.pos, city.pos) <= 3
                    && g.rules.units[unit.kind].class == "military"
            })
        {
            return None;
        }
        let mut window = g.game_speed.scale(CAPACITY_WINDOW).ceil() as u32;
        if g.max_turns > 0 {
            window = window.min(g.max_turns.saturating_sub(g.turn));
        }
        let upkeep = g.rules.buildings[granary].maintenance;
        let runway =
            g.players[pid].gold + (g.players[pid].gold_per_turn - upkeep).min(0.0) * window as f64;
        if !runway.is_finite() || runway < CAPACITY_GOLD_BUFFER {
            return None;
        }
        let cost = g.item_remaining_cost_for_city(pid, cid, &item);
        let control = capacity_projection(g, pid, cid, None, window)?;
        let candidate = capacity_projection(g, pid, cid, Some(&item), window)?;
        let returned = candidate.production - control.production;
        (candidate.completed
            && candidate.population > control.population
            && returned >= cost * CAPACITY_RETURN_MARGIN
            && returned > 0.0)
            .then_some(CapacityInvestment {
                item,
                returned,
                cost,
            })
    }
}

#[cfg(test)]
mod tests;
