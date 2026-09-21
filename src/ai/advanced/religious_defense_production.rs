//! Keep the first usable Temple when a Domination founder must resist conversion.
use super::*;

impl AdvancedAi {
    /// A Temple's Relic slot does not make this defensive unlock a cultural
    /// diversion. This is one existing founder reservation, not a new faith
    /// economy: other lanes, safe founders, and further Temples keep their bids.
    pub(super) fn domination_defensive_temple(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        if !self.founder_temple
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
        {
            return false;
        }
        let Item::Building { building } = item else {
            return false;
        };
        if !g.building_is_family(building, crate::name!("temple")) {
            return false;
        }
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return false;
        };
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        // A purchased religious unit inherits this city's majority. Preserve
        // a factory for our defenders, not for the religion we must stop.
        if g.city_religion(city) != Some(religion) {
            return false;
        }
        let cities = g.player_city_ids(pid);
        if cities.iter().any(|city| {
            g.cities[city]
                .buildings
                .iter()
                .any(|building| g.building_is_family(building, crate::name!("temple")))
        }) {
            return false;
        }
        // Sequential queue decisions share one reservation. A deterministic
        // winner also resolves any pre-existing duplicate Temple orders.
        let reserved = cities
            .iter()
            .copied()
            .filter(|other| {
                let city = &g.cities[other];
                g.city_religion(city) == Some(religion)
                    && city.queue.first().is_some_and(|queued| {
                        matches!(queued, Item::Building { building }
                        if g.building_is_family(building, crate::name!("temple")))
                            && Self::production_commitment_is_legal(g, pid, *other, queued)
                    })
            })
            .min();
        if reserved.is_some_and(|reserved| reserved != cid) {
            return false;
        }
        cities
            .iter()
            .any(|city| Self::city_needs_religious_support(g, pid, &g.cities[city], religion))
    }
}

#[cfg(test)]
mod tests;
