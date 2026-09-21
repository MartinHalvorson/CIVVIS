use super::*;

impl AdvancedAi {
    /// When conversion closes our last source of a defensive faith, the
    /// remaining spreaders must reopen recruitment before spending themselves
    /// on ordinary population targets. An existing supplier releases this
    /// priority immediately; additional Holy Sites are not a second mission.
    pub(super) fn counterfaith_recruitment_targets(
        &self,
        g: &Game,
        pid: usize,
        faith: &str,
    ) -> Vec<u32> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
        {
            return Vec::new();
        }
        if let Some(founded) = g.players[pid].religion.as_deref() {
            // Our own faith also needs a working source of replacement
            // spreaders. A missing observed holy-city ID is not a reason to
            // abandon the city's usable recruitment infrastructure.
            if faith != founded {
                return Vec::new();
            }
        } else {
            let Some(threat) = self.adopted_faith_threat(g, pid) else {
                return Vec::new();
            };
            if faith == threat || !Self::safe_adopted_counterfaith(g, pid, faith) {
                return Vec::new();
            }
        }
        let equipped: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                city.buildings.iter().any(|building| {
                    g.building_is_family(building, crate::name!("shrine"))
                        && !city.pillaged_buildings.contains(building)
                }) && city.districts.iter().any(|(district, position)| {
                    g.district_family(*district).as_str() == "holy_site"
                        && g.map.get(*position).is_some_and(|tile| !tile.pillaged)
                })
            })
            .collect();
        if equipped
            .iter()
            .any(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
        {
            return Vec::new();
        }
        equipped
    }
}

#[cfg(test)]
mod tests;
