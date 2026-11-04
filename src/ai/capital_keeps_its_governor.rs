//! Opt-in protection for the original capital's Governor Loyalty anchor.

use super::*;
use std::sync::Arc;

impl BasicAi {
    /// Price the Governor's departure against the current measured rate.
    /// `Expansion1_Governors.xml:72` gives Magnus IdentityPressure="8".
    /// Other policy, building and Governor effects come from the rules model.
    /// A native rate is fixed metadata: removing the Governor from a clone
    /// alone would leave it unchanged, so use the modeled support difference
    /// with and without the Governor, then subtract it from the observed rate.
    pub(super) fn capital_governor_may_depart(&self, g: &Game, pid: usize, governor: &str) -> bool {
        if !self.capital_keeps_its_governor {
            return true;
        }
        let Some(cid) = g.players[pid]
            .governor_roster
            .get(governor)
            .and_then(|state| state.city)
        else {
            return true;
        };
        let Some(city) = g.cities.get(&cid) else {
            return true;
        };
        if city.owner != pid || city.original_owner != pid || !city.is_capital {
            return true;
        }
        let rate = g.city_loyalty_per_turn(city);
        let mut probe = g.clone();
        Arc::make_mut(&mut probe.observed_city_loyalty_per_turn).remove(&cid);
        let with_governor = probe.city_loyalty_per_turn(&probe.cities[&cid]);
        probe.players[pid]
            .governor_roster
            .get_mut(governor)
            .unwrap()
            .city = None;
        if !probe.players[pid]
            .governor_roster
            .values()
            .any(|state| state.city == Some(cid))
        {
            probe.players[pid].governors.retain(|city| *city != cid);
        }
        let without_governor = probe.city_loyalty_per_turn(&probe.cities[&cid]);
        let after_departure = rate - (with_governor - without_governor);
        after_departure.is_finite() && after_departure >= -f64::EPSILON
    }
}

#[cfg(test)]
mod tests;
