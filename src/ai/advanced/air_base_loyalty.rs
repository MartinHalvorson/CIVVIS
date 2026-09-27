//! Preserve the Bomber wing when its owned city base is about to revolt.

use super::{AdvancedAi, StrategicPlan, VictoryTarget};
use crate::game::{Action, City, Game};
use crate::Pos;
use std::collections::BTreeSet;

impl AdvancedAi {
    fn air_base_loyalty_guard(&self, g: &Game) -> bool {
        self.air_surge_enabled() && self.active_victory_target(g) == Some(VictoryTarget::Domination)
    }

    // A Carrier or Airstrip is not a city center/Aerodrome. Do not infer its
    // destruction from the Loyalty of an arbitrary nearby or tile-owning city.
    fn aircraft_city_base(g: &Game, pid: usize, pos: Pos) -> Option<&City> {
        g.cities.values().find(|city| {
            city.owner == pid
                && (city.pos == pos
                    || city.districts.iter().any(|(district, district_pos)| {
                        *district_pos == pos
                            && g.district_family(*district) == crate::name!("aerodrome")
                    }))
        })
    }

    fn air_base_survives_ticks(g: &Game, city: &City, ticks: f64) -> bool {
        let rate = g.city_loyalty_per_turn(city);
        city.loyalty.is_finite() && rate.is_finite() && city.loyalty + ticks * rate.min(0.0) > 0.0
    }

    /// A destination needs an arrival tick and an operating tick. This uses
    /// the owner's public total, which the decision view retains even when
    /// foreign cities supplying population pressure are hidden.
    pub(super) fn air_base_rebase_allowed(&self, g: &Game, pid: usize, uid: u32, to: Pos) -> bool {
        if !self.air_base_loyalty_guard(g)
            || g.rules.units[g.units[&uid].kind].promotion_class != "air_bomber"
        {
            return true;
        }
        Self::aircraft_city_base(g, pid, to)
            .is_none_or(|city| Self::air_base_survives_ticks(g, city, 2.0))
    }

    /// Reserve the aircraft before any profitable kill or city volley can
    /// spend the last movement needed to leave a base that will revolt.
    pub(super) fn evacuate_air_bases(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> BTreeSet<u32> {
        let mut evacuated = BTreeSet::new();
        if !self.air_base_loyalty_guard(g) {
            return evacuated;
        }
        let objective = plan
            .target_city
            .or(plan.threatened_city)
            .and_then(|id| g.cities.get(&id).map(|city| city.pos));
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            if unit.moves_left <= 0.0 || g.rules.units[unit.kind].promotion_class != "air_bomber" {
                continue;
            }
            let from = unit.pos;
            let Some(city) = Self::aircraft_city_base(g, pid, from) else {
                continue;
            };
            let rate = g.city_loyalty_per_turn(city);
            // Unknown/nonfinite readings cannot establish an imminent revolt.
            if !city.loyalty.is_finite()
                || !rate.is_finite()
                || Self::air_base_survives_ticks(g, city, 1.0)
            {
                continue;
            }
            // Recompute legality after each reservation so planes cannot
            // reserve the same final slot at the destination.
            let best = g
                .legal_doctrine_actions(pid, uid)
                .into_iter()
                .filter_map(|action| match action {
                    Action::AirRebase { unit, to }
                        if unit == uid && self.air_base_rebase_allowed(g, pid, uid, to) =>
                    {
                        Some((
                            objective.map_or(0, |target| g.wdist(to, target)),
                            g.wdist(from, to),
                            to,
                            action,
                        ))
                    }
                    _ => None,
                })
                .min_by_key(|(front_distance, travel, to, _)| (*front_distance, *travel, *to));
            if let Some((_, _, _, action)) = best {
                if g.apply(pid, &action).is_ok() {
                    evacuated.insert(uid);
                }
            }
        }
        evacuated
    }
}

#[cfg(test)]
mod tests;
