//! A native purchase request for Coal when ready power would serve research.
//! A reconstruction of native 20261102T165603Z found three unpowered Labs
//! in reach of a buildable plant. Native exports show zero fuel income/bank
//! and peaceful rivals offering Coal.
//! This opt-in names a bounded stock purchase; it does not simulate a trade
//! or assert that the host accepted one. Strength needs native evaluation.

use super::*;

impl AdvancedAi {
    pub fn science_power_fuel_wants(&self, g: &Game, pid: usize) -> Vec<GunResourceWant> {
        if !self.science_power_buys_fuel
            || !g.victory_conditions.science
            || self.raced_target() != Some(VictoryTarget::Science)
            || self.diplomatic_lane_refuses_fossil_power("coal_power_plant")
            || g.players[pid].gold < 100.0
            || g.players[pid].gold_per_turn < 0.0
            || g.turn_limit()
                .is_some_and(|limit| limit.saturating_sub(g.turn) < 10)
            || g.strategic_resource_rate(pid, "coal") > 0.0
            || g.strategic_stockpile(pid, crate::name!("coal")) >= 10.0
        {
            return Vec::new();
        }
        let plant = Item::Building {
            building: crate::name!("coal_power_plant"),
        };
        let ready = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .any(|city| {
                // Another plant cannot use Coal. Never buy a conversion here.
                if city
                    .buildings
                    .iter()
                    .any(|b| matches!(b.as_str(), "oil_power_plant" | "nuclear_power_plant"))
                {
                    return false;
                }
                if !city.buildings.contains(&crate::name!("coal_power_plant"))
                    && !g.can_produce(pid, city.id, &plant)
                {
                    return false;
                }
                let origin = g
                    .city_district_family_position(city, crate::name!("industrial_zone"))
                    .unwrap_or(city.pos);
                g.cities
                    .values()
                    .filter(|other| {
                        other.owner == pid
                            && other.buildings.contains(&crate::name!("research_lab"))
                            && !g.city_is_powered(other)
                            && g.wdist(origin, other.pos) <= 6
                    })
                    .take(2)
                    .count()
                    >= 2
            });
        if !ready || self.threatened_city(g, pid).is_some() {
            return Vec::new();
        }
        // This existing transport record is also used for non-gun material
        // buys. Only resource, amount and minimum enter this purchase.
        vec![GunResourceWant {
            resource: "RESOURCE_COAL".to_string(),
            resource_id: "coal".to_string(),
            amount: 20,
            minimum: 10,
            unit: "coal_power_plant".to_string(),
            guns: 1,
            hit: 0.0,
            renewal: false,
        }]
    }
}

#[cfg(test)]
mod tests;
