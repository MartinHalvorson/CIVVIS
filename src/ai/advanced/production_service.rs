//! Measure whether a fast city can service nearby worked production sooner.
use super::*;

impl AdvancedAi {
    pub(super) fn productive_builder_dispatch_city(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> Option<u32> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.base.minor
            || self.base.barb
            || plan.strategy == GrandStrategy::Recovery
            || plan.threatened_city.is_some()
            || g.turn > g.standard_duration(160)
            || g.players[pid].gold_per_turn < 0.0
        {
            return None;
        }
        let _memo = g.query_memo();
        let cities = g.player_city_ids(pid);
        if cities.len() < 2
            || counts.builders >= (PRODUCTION_BUILDERS_PER_CITY * cities.len() as f64).ceil() as usize
            || cities.iter().any(|cid| {
                matches!(g.cities[cid].queue.first(), Some(Item::Unit { unit }) if unit == "builder")
            })
        {
            return None;
        }
        let standing_defenders = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !spec.siege
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .count();
        if standing_defenders < cities.len() {
            return None;
        }
        let jobs: Vec<_> = cities
            .iter()
            .flat_map(|cid| {
                g.city_citizen_plan(*cid)
                    .worked_tiles
                    .into_iter()
                    .filter_map(|pos| {
                        let tile = g.map.get(pos)?;
                        if pos == g.cities[cid].pos
                            || tile.owner_city != Some(*cid)
                            || tile.improvement.is_some()
                            || tile.district.is_some()
                            || g.wdist(g.cities[cid].pos, pos) > 3
                        {
                            return None;
                        }
                        let gain = g
                            .valid_improvements(pid, pos)
                            .into_iter()
                            .filter(|name| {
                                let spec = &g.rules.improvements[name];
                                spec.builder_buildable && !spec.removes_feature
                            })
                            .map(|name| g.improvement_yield_change(pid, pos, name).production)
                            .fold(0.0, f64::max);
                        (gain > 0.0).then_some((pos, gain))
                    })
            })
            .collect();
        let item = Item::Unit {
            unit: crate::name!("builder"),
        };
        let remaining = g
            .turn_limit()
            .unwrap_or(g.standard_duration(500))
            .saturating_sub(g.turn) as f64;
        let mut best: Option<(f64, u32)> = None;
        for cid in cities {
            let city = &g.cities[&cid];
            // Preserve all ordinary construction, defense and launch work.
            // Only idle queues or repeatable district projects are eligible.
            let discretionary = city.queue.first().is_none_or(|current| {
                matches!(current, Item::Project { project }
                    if g.rules.projects[project].repeatable
                        && g.rules.projects[project].district
                            .is_none_or(|district| g.district_family(district) != "spaceport"))
            });
            if !discretionary || city.loyalty < 70.0 || !g.can_produce(pid, cid, &item) {
                continue;
            }
            if self.base.solvency_first_trade_slot
                && counts.traders == 0
                && self
                    .base
                    .should_add_trader_in_city_for_controller(g, pid, cid, counts.traders)
            {
                continue;
            }
            let cost = g.item_remaining_cost_for_city(pid, cid, &item);
            let build = g
                .host_production_turns(cid, &item)
                .unwrap_or(cost / g.city_yields(cid).production.max(1.0));
            if build > g.game_speed.scale(16.0) {
                continue;
            }
            let charges: usize = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner == pid && unit.kind == "builder" && g.wdist(unit.pos, city.pos) <= 6
                })
                .map(|unit| unit.charges.max(0) as usize)
                .sum();
            let mut nearby: Vec<_> = jobs
                .iter()
                .filter(|(pos, _)| g.wdist(*pos, city.pos) <= 6)
                .map(|(_, gain)| *gain)
                .collect();
            nearby.sort_by(|a, b| b.total_cmp(a));
            // Existing local charges cover the best jobs first. Price at most
            // three additional charges, even when future policy would add more.
            let gain: f64 = nearby.into_iter().skip(charges).take(3).sum();
            // Six turns reserve travel and operations; routes and capture
            // safety remain the existing Builder controller's responsibility.
            let service = build + 6.0;
            if gain < 4.0 || (remaining - service).max(0.0) * gain < cost {
                continue;
            }
            let value = gain / (service + 1.0);
            if best.is_none_or(|(old, old_city)| value > old || (value == old && cid < old_city)) {
                best = Some((value, cid));
            }
        }
        best.map(|(_, city)| city)
    }
}

#[cfg(test)]
mod tests;
