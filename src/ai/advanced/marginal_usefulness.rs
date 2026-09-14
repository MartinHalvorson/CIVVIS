//! Compare the additional work a choice buys, against the same baseline.
//! These corrections belong to the named victory governor. Adaptive genomes
//! retain their screened behavior; this is not a claim of a global optimum.

use super::*;

impl AdvancedAi {
    /// Named victory cities reconsider their queues. A 25% gain must justify
    /// delaying the current payoff; the scorer already credits saved progress.
    /// The adaptive genome's disabled-by-default review remains unchanged.
    pub(super) fn production_review_margin(&self, g: &Game) -> f64 {
        if self.active_victory_target(g).is_some() && self.preempt_margin <= 1.0 {
            1.25
        } else {
            self.preempt_margin
        }
    }

    pub(super) fn counts_without_city_queue(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
    ) -> EmpireCounts {
        let mut counts = EmpireCounts::default();
        for uid in g.player_unit_ids(pid) {
            counts.add_unit(g, &g.units[&uid].kind);
        }
        for cid in g.player_city_ids(pid) {
            if cid != city {
                if let Some(item) = g.cities[&cid].queue.first() {
                    counts.add_item(g, item);
                }
            }
        }
        counts
    }

    pub(super) fn marginal_improvement_value(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let value = self.improvement_value_for(g, pid, pos, improvement, strategy);
        if self.active_victory_target(g).is_none() {
            return value;
        }
        let standing = &g.map.tiles[&pos];
        let existing = standing
            .improvement
            .filter(|_| !standing.pillaged)
            .map_or(0.0, |old| {
                self.improvement_value_for(g, pid, pos, &old, strategy)
            });
        value - existing
    }

    /// Compare work here with the same ranked, travel-priced jobs used when
    /// choosing a destination. Only move for a strictly better executable
    /// job; unavailable alternatives never consume the local work turn.
    pub(super) fn more_useful_builder_job(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> Option<bool> {
        if self.active_victory_target(g).is_none()
            || self.builder_support.contains_key(&uid)
            || g.units[&uid].moves_left <= 0.0
        {
            return None;
        }
        let current = g.units[&uid].pos;
        let mut reserved: HashSet<_> = self
            .builder_targets
            .iter()
            .filter(|(other, _)| **other != uid && g.units.contains_key(other))
            .map(|(_, pos)| *pos)
            .collect();
        if let Some((pos, until)) = self.builder_avoid.get(&uid) {
            if g.turn < *until {
                reserved.insert(*pos);
            }
        }
        // This unit already occupies its work tile, so another stale target
        // cannot hide the baseline from the comparison.
        reserved.remove(&current);
        let ranked = self.builder_jobs_ranked(g, pid, uid, strategy, &reserved);
        let city_shortfall = g.map.tiles[&current].owner_city.map_or(0.0, |city| {
            self.city_production_foundation_shortfall(g, pid, city)
        });
        let here = self
            .worthwhile_improvements(g, pid, current, strategy)
            .into_iter()
            .map(|improvement| {
                self.production_foundation_improvement_value(
                    g,
                    pid,
                    current,
                    &improvement,
                    strategy,
                    city_shortfall,
                )
            })
            .max_by(f64::total_cmp)?;
        for pos in ranked.into_iter().take(BUILDER_ROUTE_ATTEMPTS) {
            if pos == current {
                break;
            }
            let shortfall = g.map.tiles[&pos].owner_city.map_or(0.0, |city| {
                self.city_production_foundation_shortfall(g, pid, city)
            });
            let best = self
                .worthwhile_improvements(g, pid, pos, strategy)
                .into_iter()
                .map(|improvement| {
                    self.production_foundation_improvement_value(
                        g,
                        pid,
                        pos,
                        &improvement,
                        strategy,
                        shortfall,
                    ) - f64::from(g.wdist(current, pos)) * 0.7
                })
                .max_by(f64::total_cmp)?;
            if best <= here + 1e-9 {
                break;
            }
            // The route-step helper can approach an impassable goal. Leaving
            // useful work requires a complete route to the better job itself.
            if !g.can_stop(uid, pos) || g.route_step(uid, pos, 0).is_none() {
                continue;
            }
            let stepped = if self.civilian_reach_safety_on() {
                self.builder_step_out_of_reach(g, pid, uid, pos)
            } else {
                self.builder_step_toward_barbarian_safe(g, pid, uid, pos)
            };
            if stepped {
                self.builder_targets.insert(uid, pos);
                think!(self.journal(), Expansion, Decision, "Builder chooses more useful work";
                    "job at {pos:?} adds {best:.1} after travel, versus {here:.1} here; guard and route safety still apply"; pos);
                return Some(true);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
