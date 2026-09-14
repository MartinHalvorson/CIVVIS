//! Food becoming citizens, shared by turn processing and housing forecasts.

use super::Game;

impl Game {
    /// Food reaching this city's growth bank after consumption and modifiers.
    /// Callers may substitute planned housing while retaining the current
    /// yields and amenities. The turn loop uses its already memoized readings.
    /// Negative food remains starvation; growth modifiers only scale surplus.
    pub(crate) fn city_growth_surplus(
        &self,
        pid: usize,
        cid: u32,
        food: f64,
        housing: f64,
        amenities: i64,
    ) -> f64 {
        let mut growth_bonus = self.empire_building_sum(pid, |b| b.growth_pct);
        growth_bonus += self.empire_wonder_effect(pid, "empire_growth_pct");
        growth_bonus += self.governor_effect(pid, cid, "growth_pct");
        if self.on_foreign_continent(pid, self.cities[&cid].pos) {
            growth_bonus += self.policy_effect(pid, "foreign_continent_growth_pct");
        }
        growth_bonus += self.pantheon_effect(pid, "growth_pct");
        if self.grants_city_state_unique_bonus(pid, "Mitla")
            && self.city_has_active_district_family(&self.cities[&cid], crate::name!("campus"))
        {
            growth_bonus += 15.0;
        }
        growth_bonus += self
            .city_resource_industry_effects(&self.cities[&cid])
            .growth_pct;
        if self.congress_effect_active("migration_treaty", "A", &pid.to_string()) {
            growth_bonus += 20.0;
        } else if self.congress_effect_active("migration_treaty", "B", &pid.to_string()) {
            growth_bonus -= 20.0;
        }
        let city = &self.cities[&cid];
        let mut surplus = food - 2.0 * city.pop as f64;
        if surplus > 0.0 {
            let headroom = housing - city.pop as f64;
            let hf = Self::housing_growth_mult(headroom);
            let af = Self::amenity_growth_mult(amenities);
            let lf = Self::loyalty_growth_mult(city.loyalty);
            surplus *= hf * af * lf * (1.0 + growth_bonus / 100.0);
        }
        surplus
    }
}

#[cfg(test)]
mod tests;
