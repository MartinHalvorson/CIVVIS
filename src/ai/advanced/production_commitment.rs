//! Finish useful interrupted work after urgent reservations, before new bids.
use super::*;

impl AdvancedAi {
    pub(super) fn reconcile_production_commitments(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.victory_planning {
            return;
        }
        for cid in g.player_city_ids(pid) {
            if self.replace_unbuildable_city_production(g, pid, cid, plan) {
                continue;
            }
            // Most cities have no paused work. Avoid a menu/scoring pass there.
            if !g.cities[&cid].queue.is_empty()
                || !g.cities[&cid]
                    .production_progress
                    .values()
                    .any(|p| *p > 0.0)
            {
                continue;
            }
            let counts = self.counts(g, pid);
            self.resume_city_production(g, pid, cid, plan, &counts);
        }
    }

    fn replace_unbuildable_city_production(
        &self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(current) = g.cities[&cid].queue.first().cloned() else {
            return false;
        };
        if g.can_produce(pid, cid, &current) {
            return false;
        }
        let counts = self.counts_without_city_queue(g, pid, cid);
        let mut candidates = {
            let _memo = g.query_memo();
            g.producible_items(pid, cid)
                .into_iter()
                .filter_map(|item| {
                    if let Item::Project { project } = &item {
                        if !g.rules.projects[project].repeatable
                            && g.cities.values().any(|other| {
                                other.owner == pid
                                    && other.id != cid
                                    && other.queue.first() == Some(&item)
                            })
                        {
                            return None;
                        }
                    }
                    let value = self.production_value(g, pid, cid, &item, plan, &counts);
                    (value.is_finite() && value > -1_000.0)
                        .then(|| (value, format!("{item:?}"), item))
                })
                .collect::<Vec<_>>()
        };
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        for (_, _, item) in candidates {
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: item.clone(),
                },
            )
            .is_ok()
            {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let city_name = g.cities[&cid].name.clone();
                    think!(self.journal(), Economy, Decision,
                        "{} replaces unavailable {} with {}", city_name,
                        Self::plain_item(&current), Self::plain_item(&item);
                        "the current build is no longer legal; a completion commitment cannot keep an impossible queue");
                }
                return true;
            }
        }
        // Preserve the authoritative queue if there is no accepted alternative.
        false
    }

    pub(super) fn resume_city_production(
        &self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> bool {
        let city = &g.cities[&cid];
        if !self.victory_planning
            || !city.queue.is_empty()
            || !city.production_progress.values().any(|p| *p > 0.0)
            || plan.strategy == GrandStrategy::Recovery
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || self.war_plan.is_some()
            || self.live_war_economy_requires_recovery(g, pid, counts)
            || self.base.barbarian_local_alarm_for_controller(g, pid, cid)
        {
            return false;
        }
        let mut candidates = {
            let _memo = g.query_memo();
            g.producible_items(pid, cid)
                .into_iter()
                .filter_map(|item| {
                    let invested = g.item_invested_production(cid, &item);
                    if !invested.is_finite() || invested <= 0.0 {
                        return None;
                    }
                    if let Item::Project { project } = &item {
                        // A repeatable filler is not a finite commitment; do
                        // not resume it ahead of missing infrastructure.
                        if g.rules.projects[project].repeatable
                            || g.cities.values().any(|other| {
                                other.owner == pid
                                    && other.id != cid
                                    && other.queue.first() == Some(&item)
                            })
                        {
                            return None;
                        }
                    }
                    let turns = self.production_build_turns(g, pid, cid, &item);
                    if matches!(&item, Item::Unit { unit } if unit == "settler")
                        && Self::settler_queue_loyalty_risk(g, cid, turns).is_some()
                    {
                        return None;
                    }
                    let score = self.production_value(g, pid, cid, &item, plan, counts);
                    if !score.is_finite() || score <= 0.0 || !turns.is_finite() {
                        return None;
                    }
                    Some((
                        matches!(&item, Item::Wonder { .. }),
                        turns,
                        score,
                        format!("{item:?}"),
                        item,
                    ))
                })
                .collect::<Vec<_>>()
        };
        // Wonders are contested; otherwise realize the nearest useful payoff.
        // Stable tie-breaking makes menu order irrelevant on every platform.
        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.total_cmp(&b.1))
                .then_with(|| b.2.total_cmp(&a.2))
                .then_with(|| a.3.cmp(&b.3))
        });
        for (_, turns, _, _, item) in candidates {
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: item.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let city_name = g.cities[&cid].name.clone();
                think!(self.journal(), Economy, Decision,
                    "{} resumes {}", city_name, Self::plain_item(&item);
                    "saved work remains useful and legal; about {turns:.1} production turns remain after urgent reservations");
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
