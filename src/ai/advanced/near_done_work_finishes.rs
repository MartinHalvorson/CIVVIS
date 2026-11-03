//! `near-done-work-finishes` (opt-in): a head a production writer displaced
//! this turn goes back to the head when it finishes within two turns.
//!
//! **Why (live, 39 Immortal games with `saved-work-is-seen`, 2026-11-03).**
//! The host now reports every paused item's saved work. Of 1,204 items
//! paused, 47% of the saved work sat in items within two turns of finishing
//! when they were paused (177 items within one turn, 257 within two). In the
//! war games, 85% of the lane, `saved-work-is-seen` left stranded work
//! unchanged (5.0% vs 5.2% of city production): 71% of the paused-work
//! city-turns were in undamaged cities outside Recovery, so no resume gate
//! applied. The claims and governors displace the head every turn ("ahead
//! of X (it keeps its progress)"), and the resume path only ever sees an
//! empty queue.
//!
//! **The rule.** The head each city held before any production writer ran
//! is noted. After every ordinary writer (right after
//! `paused-district-finishes`, before the pad holds, the declaration
//! response and the defence handoffs), a city whose head changed gets the
//! old head back when it is still producible, holds saved work and finishes
//! within [`NEAR_DONE_MAX_TURNS`]. The displacing item then follows it, a
//! turn or two later, with the overflow.
//!
//! **Never** under Recovery, in a threatened city or one attacked within
//! four turns. Never when the displacing item is a soldier, a Settler, a
//! defence building (`outer_defense`), a Holy Site or Shrine (the Prophet
//! race), a Spaceport, a Wonder or any project (age-closer, Great Person and
//! space projects run against deadlines). Off, nothing runs.

use super::*;

/// A displaced head that finishes within this many turns goes back.
pub(crate) const NEAR_DONE_MAX_TURNS: f64 = 2.0;

/// A city attacked within this many turns keeps its new head.
const NEAR_DONE_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Either version of the family.
    pub(super) fn near_done_on(&self) -> bool {
        self.near_done_work_finishes || self.near_done_work_finishes_2
    }

    /// Whether this city may be touched at all this turn: not under
    /// Recovery, not threatened, not attacked within four turns.
    fn near_done_city_open(&self, g: &Game, cid: u32, plan: &StrategicPlan) -> bool {
        let city = &g.cities[&cid];
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= NEAR_DONE_ATTACK_MEMORY;
        plan.strategy != GrandStrategy::Recovery && plan.threatened_city != Some(cid) && !attacked
    }

    /// Version 2: an empty queue takes up the paused build that finishes
    /// soonest within [`NEAR_DONE_MAX_TURNS`], unless the city is closed
    /// (above) or due to train the next Settler. Returns whether it did.
    fn near_done_takes_the_idle_queue(
        &self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> bool {
        let n_cities = g.player_city_ids(pid).len();
        if !g.cities[&cid].queue.is_empty()
            || !self.near_done_city_open(g, cid, plan)
            || self
                .base
                .settler_due(g, pid, cid, n_cities, counts.settlers)
        {
            return false;
        }
        let best = {
            let _memo = g.query_memo();
            g.producible_items(pid, cid)
                .into_iter()
                .filter(|item| {
                    !matches!(item, Item::Project { project } if g.rules.projects[project].repeatable)
                        && g.item_invested_production(cid, item) > 0.0
                })
                .filter_map(|item| {
                    let turns = self.production_build_turns(g, pid, cid, &item);
                    (turns.is_finite()
                        && turns <= NEAR_DONE_MAX_TURNS
                        && self.production_value(g, pid, cid, &item, plan, counts) > 0.0)
                        .then(|| (turns, format!("{item:?}"), item))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        };
        let Some((turns, _, item)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let city_name = g.cities[&cid].name.clone();
            think!(self.journal(), Economy, Decision,
                "{} takes up its nearly finished {}", city_name, Self::plain_item(&item);
                "near-done-work-finishes-2: about {turns:.1} turns left on saved work; the idle queue finishes it before any new item");
        }
        true
    }

    /// Each of our cities' head before any production writer runs (after
    /// version 2 has filled the idle queues it may). Empty with the gene off.
    pub(super) fn near_done_heads(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> BTreeMap<u32, Item> {
        if !self.near_done_on() {
            return BTreeMap::new();
        }
        if self.near_done_work_finishes_2 {
            let counts = self.counts(g, pid);
            for cid in g.player_city_ids(pid) {
                self.near_done_takes_the_idle_queue(g, pid, cid, plan, &counts);
            }
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                g.cities[&cid]
                    .queue
                    .first()
                    .cloned()
                    .map(|item| (cid, item))
            })
            .collect()
    }

    /// Whether a displacing item runs against a deadline or a threat, so the
    /// displaced head never takes the queue back from it.
    pub(super) fn near_done_displacer_is_urgent(g: &Game, item: &Item) -> bool {
        match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                *unit == "settler"
                    || g.rules
                        .units
                        .get(unit)
                        .is_none_or(|spec| spec.class == "military")
            }
            Item::Building { building } => {
                *building == "shrine"
                    || g.rules
                        .buildings
                        .get(building)
                        .is_none_or(|spec| spec.outer_defense > 0 || spec.wonder)
            }
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                family == "spaceport" || family == "holy_site"
            }
            _ => true,
        }
    }

    pub(super) fn claim_near_done_work_finishes(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        heads: &BTreeMap<u32, Item>,
    ) {
        if !self.near_done_on() || heads.is_empty() || plan.strategy == GrandStrategy::Recovery {
            return;
        }
        for (cid, item) in heads {
            let Some(city) = g.cities.get(cid) else {
                continue;
            };
            if city.owner != pid {
                continue;
            }
            let Some(current) = city.queue.first().cloned() else {
                continue;
            };
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= NEAR_DONE_ATTACK_MEMORY;
            if current == *item
                || attacked
                || plan.threatened_city == Some(*cid)
                || Self::near_done_displacer_is_urgent(g, &current)
                || !g.can_produce(pid, *cid, item)
                || g.item_invested_production(*cid, item) <= 0.0
            {
                continue;
            }
            let turns = self.production_build_turns(g, pid, *cid, item);
            if !turns.is_finite() || turns > NEAR_DONE_MAX_TURNS {
                continue;
            }
            if g.apply(
                pid,
                &Action::Produce {
                    city: *cid,
                    item: item.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let city_name = g.cities[cid].name.clone();
                think!(self.journal(), Economy, Decision,
                    "{} finishes {} before {}", city_name,
                    Self::plain_item(item), Self::plain_item(&current);
                    "near-done-work-finishes: about {turns:.1} turns left on the displaced head; the displacing item follows it (it keeps its progress)");
            }
        }
    }
}

#[cfg(test)]
mod tests;
