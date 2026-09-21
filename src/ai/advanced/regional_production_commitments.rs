use super::*;

fn regional_group(building: Name, spec: &crate::rules::BuildingSpec) -> &str {
    if spec.regional_group.is_empty() {
        spec.replaces.unwrap_or(building).as_str()
    } else {
        &spec.regional_group
    }
}

impl AdvancedAi {
    /// Credit a regional production benefit once, against completed buildings
    /// and legal active commitments. Concurrent queues use estimated completion
    /// time and city id; an existing commitment precedes a new proposal.
    pub(super) fn committed_regional_production_value(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        building: &Name,
        spec: &crate::rules::BuildingSpec,
        strategy: GrandStrategy,
    ) -> f64 {
        if city.buildings.contains(building) {
            return 0.0;
        }
        let item = Item::Building {
            building: *building,
        };
        let eta = |source: &crate::game::City, item: &Item| {
            g.item_remaining_cost_for_city(pid, source.id, item)
                / g.city_yields(source.id).production.max(1.0)
        };
        let committed = city.queue.first() == Some(&item);
        let candidate_eta = if committed {
            eta(city, &item)
        } else {
            f64::INFINITY
        };
        let group = regional_group(*building, spec);
        let mut projection = g.clone();
        for source in g
            .cities
            .values()
            .filter(|source| source.owner == pid && source.id != city.id)
        {
            let Some(
                queued @ Item::Building {
                    building: queued_building,
                },
            ) = source.queue.first()
            else {
                continue;
            };
            let queued_spec = &g.rules.buildings[queued_building];
            if queued_spec.regional_range <= 0
                || regional_group(*queued_building, queued_spec) != group
                || !g.can_produce(pid, source.id, queued)
            {
                continue;
            }
            let earlier = eta(source, queued)
                .total_cmp(&candidate_eta)
                .then(source.id.cmp(&city.id))
                .is_lt();
            if earlier {
                projection
                    .cities
                    .get_mut(&source.id)
                    .unwrap()
                    .buildings
                    .push(*queued_building);
            }
        }
        // Use the engine's production queries rather than duplicating its
        // district origins, range bonuses, pillaging, replacement groups,
        // power, or Vertical Integration. Each memo ends before mutation.
        let other_cities: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|id| *id != city.id)
            .collect();
        let total = |board: &Game| {
            let _memo = board.query_memo();
            other_cities
                .iter()
                .map(|id| board.city_yields(*id).production)
                .sum::<f64>()
        };
        let before = total(&projection);
        projection
            .cities
            .get_mut(&city.id)
            .unwrap()
            .buildings
            .push(*building);
        let extra = (total(&projection) - before).max(0.0);
        self.yield_value(
            Yields {
                production: extra,
                ..Yields::default()
            },
            strategy,
        ) * 42.0
            * REGIONAL_PRODUCTION_REACH_DISCOUNT
    }
}

#[cfg(test)]
mod tests;
