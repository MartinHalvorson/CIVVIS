use super::*;

impl AdvancedAi {
    /// When conversion closes our last source of a safe adopted faith, the
    /// remaining spreaders must reopen recruitment before spending themselves
    /// on ordinary population targets. An existing supplier releases this
    /// priority immediately; additional Holy Sites are not a second mission.
    pub(super) fn counterfaith_recruitment_targets(
        &self,
        g: &Game,
        pid: usize,
        faith: &str,
    ) -> Vec<u32> {
        let Some(threat) = self.adopted_faith_threat(g, pid) else {
            return Vec::new();
        };
        if faith == threat || !Self::safe_adopted_counterfaith(g, pid, faith) {
            return Vec::new();
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
