//! Coordinate two existing opening claims using native completion quotes.
//! No new jobs, quotas, build budgets or valuation constants are introduced.

use super::*;

impl AdvancedAi {
    pub(super) fn parallel_library_backlog_candidate(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.parallel_library_backlog
            || self.victory_target != Some(VictoryTarget::Science)
            || !self.science_heads_keep_their_queue
            || self.phase_specialization_active(g)
        {
            return None;
        }
        let (_, a, builder) = self.backlog_builder_claim_candidate(g, pid, plan, None)?;
        let (rank, _, library_city, library) =
            self.campus_building_claim_candidate(g, pid, plan, None)?;
        if rank != 0 || a != library_city || !g.cities[&a].queue.is_empty() {
            return None;
        }
        let (_, b, _) = self.backlog_builder_claim_candidate(g, pid, plan, Some(a))?;
        let (other_rank, _, old_library_city, other_library) =
            self.campus_building_claim_candidate(g, pid, plan, Some(a))?;
        if other_rank != 0 || b != old_library_city || !g.cities[&b].queue.is_empty() {
            return None;
        }
        let quote = |cid, item: &Item| {
            g.host_production_turns(cid, item)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
        };
        let builder_a = quote(a, &builder)?;
        let builder_b = quote(b, &builder)?;
        let library_a = quote(a, &library)?;
        let library_b = quote(b, &other_library)?;
        // Keep the total quoted completion time from increasing, while
        // strictly improving when both jobs are ready. This trades a later
        // Builder for earlier research; it is not a native strength result.
        if library_a >= library_b
            || library_a.max(builder_b) >= builder_a.max(library_b)
            || library_a + builder_b > builder_a + library_b
        {
            return None;
        }
        Some((a, library))
    }

    pub(super) fn claim_parallel_library_backlog(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        let (city, item) = self.parallel_library_backlog_candidate(g, pid, plan)?;
        g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .ok()?;
        think!(self.journal(), Cities, Decision,
            "{} reserves its Library beside the backlog Builder", g.cities[&city].name;
            "parallel-library-backlog: the quoted two-job completion window improves without increasing the sum; the existing Builder claim chooses the other producer");
        Some((city, item))
    }
}

#[cfg(test)]
mod tests;
