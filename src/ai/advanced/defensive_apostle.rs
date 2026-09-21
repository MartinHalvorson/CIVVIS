use super::*;

impl AdvancedAi {
    /// Build the founder's defensive capability while its Temple still follows
    /// our faith. Waiting for a flipped city leaves only a few turns to save
    /// for an Apostle, launch the Inquisition, and buy its first defender.
    pub(super) fn defensive_inquisition_ready(&self, g: &Game, pid: usize) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
        {
            return false;
        }
        let Some(faith) = g.players[pid].religion.as_deref() else {
            return false;
        };
        if !g.players[pid]
            .holy_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == pid)
        {
            return false;
        }
        let spec = &g.rules.units["apostle"];
        if spec
            .tech
            .as_ref()
            .is_some_and(|tech| !g.players[pid].techs.contains(tech))
            || spec
                .civic
                .as_ref()
                .is_some_and(|civic| !g.players[pid].civics.contains(civic))
        {
            return false;
        }
        let cities = g.player_city_ids(pid);
        let established = cities
            .iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
            .count();
        let first_spreader = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == "missionary"
                && unit.religion.as_deref() == Some(faith)
                && unit.charges > 0
        });
        if established * 2 < cities.len() && !first_spreader {
            return false;
        }
        cities.iter().any(|cid| {
            let city = &g.cities[cid];
            g.city_religion(city) == Some(faith)
                && city.buildings.iter().any(|building| {
                    g.building_is_family(building, crate::name!("temple"))
                        && !city.pillaged_buildings.contains(building)
                })
                && city.districts.iter().any(|(district, position)| {
                    g.district_family(*district).as_str() == "holy_site"
                        && g.map.get(*position).is_some_and(|tile| !tile.pillaged)
                })
        })
    }

    /// Reserve Faith ahead of repeat Missionaries, using the actual purchase
    /// quote when the host offers the unit. An absent affordable menu entry
    /// is not a reason to spend the accumulated Faith on a cheaper unit.
    pub(super) fn prepare_defensive_inquisition(&self, g: &mut Game, pid: usize) -> bool {
        if !self.defensive_inquisition_ready(g, pid) {
            return false;
        }
        let launched = g.players[pid]
            .counters
            .get("inquisition")
            .copied()
            .unwrap_or(0)
            > 0;
        let wanted = if launched { "inquisitor" } else { "apostle" };
        let faith = g.players[pid].religion.clone().unwrap();
        if g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == wanted
                && unit.religion.as_deref() == Some(faith.as_str())
                && unit.charges > 0
        }) {
            // Keep funding the first Inquisitor while the Apostle travels.
            // Once the defender exists, ordinary spending resumes.
            return !launched;
        }
        for cid in g.player_city_ids(pid) {
            if g.city_religion(&g.cities[&cid]) != Some(faith.as_str()) {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, wanted, "faith") else {
                continue;
            };
            if g.players[pid].faith + f64::EPSILON < price {
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: cid,
                    unit: Name::new(wanted),
                    formation: 0,
                    currency: "faith".into(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Faith, Decision,
                    "Buying the first defensive {}", wanted;
                    "prepare an Inquisition before rival conversion removes our source");
                return true;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
