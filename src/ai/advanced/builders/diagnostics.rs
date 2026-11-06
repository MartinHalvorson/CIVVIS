//! Read-only score inspection. No production selector calls this module.

use super::*;

impl AdvancedAi {
    /// Explain legal Builder improvement alternatives using the production
    /// scorers. `strategy` is explicit: this does not infer a historical plan,
    /// route reachability, safety, reservations, or the next emitted action.
    /// Coordinates are axial, with an offset twin for native comparisons.
    pub fn builder_job_diagnostics(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> Option<serde_json::Value> {
        let unit = g.units.get(&uid)?;
        if unit.owner != pid || unit.kind != "builder" || unit.charges <= 0 {
            return None;
        }
        let travel = self.builder_travel(g, uid);
        let _memo = g.query_memo();
        let mut jobs = Vec::new();
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let shortfall = self.city_production_foundation_shortfall(g, pid, cid);
            for &pos in &city.owned_tiles {
                let tile = &g.map.tiles[&pos];
                let worthwhile = self.worthwhile_improvements(g, pid, pos, strategy);
                let standing = tile.improvement.map_or(0.0, |old| {
                    self.improvement_value_for(g, pid, pos, &old, strategy)
                });
                for improvement in g.valid_improvements(pid, pos) {
                    if !g.rules.improvements[&improvement].builder_buildable {
                        continue;
                    }
                    let intrinsic = self.improvement_value(g, pos, &improvement, strategy);
                    let luxury = self.first_luxury_premium(g, pid, pos, &improvement);
                    let growth = self.growth_farm_premium(g, pid, pos, &improvement);
                    let food = self.growing_city_food_premium(g, pid, pos, &improvement, strategy);
                    let worked =
                        self.worked_production_premium(g, pid, pos, &improvement, strategy);
                    let total = self.improvement_value_for(g, pid, pos, &improvement, strategy);
                    let marginal =
                        self.marginal_improvement_value(g, pid, pos, &improvement, strategy);
                    let foundation = self.production_foundation_improvement_bonus(
                        g,
                        pos,
                        &improvement,
                        strategy,
                        shortfall,
                    );
                    let value = self.production_foundation_improvement_value(
                        g,
                        pid,
                        pos,
                        &improvement,
                        strategy,
                        shortfall,
                    );
                    let distance = g.wdist(unit.pos, pos);
                    let (x, y) = crate::hex::axial_to_offset(pos.0, pos.1);
                    jobs.push(serde_json::json!({
                        "city": cid, "city_name": city.name,
                        "position": [pos.0, pos.1], "offset": [x, y],
                        "improvement": improvement, "standing_improvement": tile.improvement,
                        "standing_pillaged": tile.pillaged,
                        "city_food": g.city_yields(cid).food,
                        "city_population": city.pop,
                        "city_housing_headroom": g.city_housing_headroom(city),
                        "intrinsic_value": intrinsic,
                        "first_luxury_premium": luxury,
                        "growth_farm_premium": growth,
                        "growing_city_food_premium": food,
                        "worked_production_premium": worked,
                        "total_improvement_value": total,
                        "standing_improvement_value": standing,
                        "worthwhile": worthwhile.contains(&improvement),
                        "marginal_value": marginal,
                        "production_foundation_bonus": foundation,
                        "job_value": value,
                        "distance": distance,
                        "travel_score": travel.score(value, distance),
                    }));
                }
            }
        }
        let ranked = self.builder_jobs_ranked(g, pid, uid, strategy, &HashSet::new());
        Some(serde_json::json!({
            "schema": 1, "turn": g.turn, "builder": uid,
            "strategy": strategy.as_str(), "charges": unit.charges,
            "travel_priced_in_turns": travel.priced_in_turns(),
            "ranked_positions_without_reservations": ranked,
            "jobs": jobs,
            "scope": "Legal model improvements and score components only; explicit strategy, no historical plan, reservations, route/safety test or next-action prediction."
        }))
    }
}

#[cfg(test)]
mod tests;
