//! Price the industrial foundation by time to repay the remaining investment.
//!
//! Named lanes already owe industrial buildings, but discretionary bids and
//! research reservations can still take every queue before that debt is paid.
//! Reserve profitable infrastructure and compare it with research explicitly.
//! Build time and projected production returned set the investment clock; the
//! separately screened adaptive treatment retains its measured expression.

use super::{AdvancedAi, ChainRung, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Game, Item};
use crate::rules::{BuildingSpec, Yields};
use crate::think;

pub(super) struct BuilderInvestment {
    pub(super) jobs: usize,
    pub(super) returned: f64,
    pub(super) cost: f64,
    pub(super) build_turns: f64,
}

impl AdvancedAi {
    /// Independent opt-in while the production experiment is evaluated.
    pub fn enable_builder_payback_reserve(&mut self) {
        self.builder_payback_reserve = true;
    }

    pub(super) fn production_builder_commitment(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
    ) -> bool {
        matches!(item, Item::Unit { unit } if unit == "builder")
            && self
                .production_builder_investment(g, pid, cid, plan)
                .is_some()
    }

    /// Count unlocked, currently worked land jobs using the improvement the
    /// ordinary Builder router prefers. Standing yields and removed features
    /// are deducted by the engine forecast. This is a bounded estimate, not
    /// a promise that the future route or citizen assignment stays unchanged.
    pub(super) fn production_builder_investment(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<BuilderInvestment> {
        self.production_builder_status(g, pid, cid, plan).ok()
    }

    fn production_builder_status(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Result<BuilderInvestment, &'static str> {
        if !self.builder_payback_reserve
            || self.active_victory_target(g).is_none()
            || self.base.minor
            || self.base.barb
            || plan.strategy == GrandStrategy::Recovery
            || g.turn > g.standard_duration(160)
            || plan.threatened_city == Some(cid)
        {
            return Err("inactive lane, recovery, age, or strategic threat");
        }
        let city = g.cities.get(&cid).ok_or("missing city")?;
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        if city.owner != pid
            || self.city_production_foundation_shortfall(g, pid, cid) <= 0.0
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || self.base.barbarian_local_alarm_for_controller(g, pid, cid)
            || !Self::production_commitment_is_legal(g, pid, cid, &builder)
        {
            return Err("city output, recent attack, local alarm, or production legality");
        }
        let cities = g.player_city_ids(pid);
        let counts = self.counts_without_city_queue(g, pid, cid);
        let quota = (super::PRODUCTION_BUILDERS_PER_CITY * cities.len() as f64).ceil() as usize;
        if counts.builders >= quota
            || self.live_war_economy_requires_recovery(g, pid, &counts)
            || cities
                .iter()
                .any(|other| *other != cid && g.cities[other].queue.first() == Some(&builder))
        {
            return Err("existing workforce, another queued Builder, or insolvency");
        }
        let local_charges: usize = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                (unit.kind == "builder" && g.wdist(unit.pos, city.pos) <= 6)
                    .then_some(unit.charges.max(0) as usize)
            })
            .sum();
        let _memo = g.query_memo();
        let shortfall = self.city_production_foundation_shortfall(g, pid, cid);
        let mut jobs: Vec<(f64, f64)> = g
            .city_citizen_plan(cid)
            .worked_tiles
            .into_iter()
            .filter_map(|pos| {
                let tile = g.map.get(pos)?;
                if pos == city.pos
                    || tile.owner_city != Some(cid)
                    || g.rules.is_water(tile)
                    || tile.pillaged
                    || g.wdist(city.pos, pos) > 3
                {
                    return None;
                }
                let improvement = self
                    .worthwhile_improvements(g, pid, pos, plan.strategy)
                    .into_iter()
                    .max_by(|a, b| {
                        self.production_foundation_improvement_value(
                            g,
                            pid,
                            pos,
                            a,
                            plan.strategy,
                            shortfall,
                        )
                        .total_cmp(&self.production_foundation_improvement_value(
                            g,
                            pid,
                            pos,
                            b,
                            plan.strategy,
                            shortfall,
                        ))
                        .then_with(|| b.cmp(a))
                    })?;
                let gain =
                    g.improvement_yield_change(pid, pos, crate::name::Name::new(&improvement));
                (gain.production > 0.0 && gain.food >= 0.0)
                    .then_some((gain.production, g.wdist(city.pos, pos) as f64))
            })
            .collect();
        // Credit existing nearby charges first. One new Builder needs two
        // distinct productive jobs and never increases the existing quota.
        jobs.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.total_cmp(&b.1)));
        let jobs: Vec<_> = jobs
            .into_iter()
            .skip(local_charges)
            .take(g.builder_charges(pid).max(0) as usize)
            .collect();
        if jobs.len() < 2 {
            return Err("fewer than two uncovered worked production jobs");
        }
        let build_turns = self.production_build_turns(g, pid, cid, &builder);
        let window = g.standard_duration(80) as f64;
        let remaining = g.turn_limit().map_or(window, |limit| {
            window.min(limit.saturating_sub(g.turn) as f64)
        });
        // Allow a turn per operation and local travel at two hexes/turn.
        // Discount the yield by 25% for route and assignment uncertainty.
        let returned: f64 = jobs
            .iter()
            .enumerate()
            .map(|(index, (gain, distance))| {
                (remaining - build_turns - distance / 2.0 - (index + 1) as f64).max(0.0)
                    * gain
                    * 0.75
            })
            .sum();
        let cost = g.item_remaining_cost_for_city(pid, cid, &builder);
        if returned < cost || !cost.is_finite() || !returned.is_finite() {
            return Err("worked production cannot repay the Builder within the window");
        }
        Ok(BuilderInvestment {
            jobs: jobs.len(),
            returned,
            cost,
            build_turns,
        })
    }

    /// Reach the delegated and strategic governors through their common
    /// turn driver. Direct previews retain the city-local check as well.
    pub(super) fn reserve_production_builder(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.builder_payback_reserve {
            return;
        }
        let cities = g.player_city_ids(pid);
        let counts = self.counts(g, pid);
        if counts.military < cities.len() {
            return;
        }
        let mut best: Option<(u32, BuilderInvestment)> = None;
        for cid in cities {
            if !g.cities[&cid].queue.is_empty()
                || (counts.traders == 0
                    && self
                        .base
                        .should_add_trader_in_city_for_controller(g, pid, cid, 0))
            {
                continue;
            }
            match self.production_builder_status(g, pid, cid, plan) {
                Ok(investment) => {
                    if best.as_ref().is_none_or(|(old_city, old)| {
                        investment
                            .build_turns
                            .total_cmp(&old.build_turns)
                            .then_with(|| cid.cmp(old_city))
                            .is_lt()
                    }) {
                        best = Some((cid, investment));
                    }
                }
                Err(reason) => {
                    think!(self.journal(), Economy, Detail,
                        "{} defers the worked-production Builder", g.cities[&cid].name;
                        "{reason}");
                }
            }
        }
        if let Some((cid, investment)) = best {
            let item = Item::Unit {
                unit: crate::name!("builder"),
            };
            if g.apply(pid, &crate::game::Action::Produce { city: cid, item })
                .is_ok()
            {
                think!(self.journal(), Economy, Decision,
                    "{} reserves a Builder for worked production", g.cities[&cid].name;
                    "{} uncovered jobs forecast {:.1} production returned against {:.1} remaining cost, after {:.1} build turns; common production driver",
                    investment.jobs,investment.returned,investment.cost,investment.build_turns);
            }
        }
    }

    /// The Science reservation must compare its owed research building with
    /// the production foundation using the same scorer as the normal queue.
    /// Otherwise a higher industrial bid can never affect the actual choice.
    pub(super) fn research_or_industrial_foundation(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        research: Item,
        plan: &StrategicPlan,
    ) -> Item {
        if self.active_victory_target(g) != Some(VictoryTarget::Science)
            || !g.can_produce(pid, cid, &research)
        {
            return research;
        }
        let counts = self.counts(g, pid);
        let mut best_score = self.production_value(g, pid, cid, &research, plan, &counts);
        let mut best = research;
        for (building, spec) in &g.rules.buildings {
            if spec.wonder
                || !spec
                    .district
                    .is_some_and(|d| g.district_family(d) == "industrial_zone")
            {
                continue;
            }
            let candidate = Item::Building {
                building: *building,
            };
            if !g.can_produce(pid, cid, &candidate) {
                continue;
            }
            let score = self.production_value(g, pid, cid, &candidate, plan, &counts);
            if score > best_score {
                best_score = score;
                best = candidate;
            }
        }
        best
    }

    /// Repayable industrial infrastructure precedes discretionary production
    /// in an idle, safe city. Research, survival, growth, solvency and amenity
    /// reservations run first; an active launch city keeps its project queue.
    pub(super) fn profitable_industrial_foundation(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<Item> {
        let target = self.active_victory_target(g)?;
        let city = &g.cities[&cid];
        if !city.queue.is_empty()
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || (target == VictoryTarget::Science && Self::city_has_spaceport(g, cid))
        {
            return None;
        }
        let counts = self.counts(g, pid);
        let mut best: Option<(f64, Item)> = None;
        for (building, spec) in &g.rules.buildings {
            if spec.wonder
                || spec.yields.production <= 0.0
                || !spec
                    .district
                    .is_some_and(|d| g.district_family(d) == "industrial_zone")
            {
                continue;
            }
            let item = Item::Building {
                building: *building,
            };
            if !g.can_produce(pid, cid, &item) {
                continue;
            }
            let regional =
                self.regional_production_reach(g, pid, city, building, spec, plan.strategy);
            if self.industrial_investment_horizon(g, pid, cid, &item, spec, regional, plan.strategy)
                < 1.0
            {
                continue;
            }
            let score = self.production_value(g, pid, cid, &item, plan, &counts);
            if score > 0.0 && best.as_ref().is_none_or(|(old, _)| score > *old) {
                best = Some((score, item));
            }
        }
        best.map(|(_, item)| item)
    }

    /// Full premium when the remaining production repays itself before the
    /// clock; proportional credit when only part can return. Regional reach
    /// retains its existing overlap exclusions and uncertainty discount.
    /// This changes investment priority, not rules, yields, or build costs.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn industrial_investment_horizon(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        spec: &BuildingSpec,
        regional: f64,
        strategy: GrandStrategy,
    ) -> f64 {
        let per_production = self.yield_value(
            Yields {
                production: 1.0,
                ..Yields::default()
            },
            strategy,
        ) * 42.0;
        let gain = spec.yields.production + regional / per_production;
        if gain <= 0.0 {
            // A plant can be valuable for power rather than a printed yield.
            // Leave that separate utility policy intact.
            return self.chain_horizon(g, ChainRung::Building);
        }
        let remaining = if g.max_turns > 0 {
            g.max_turns.saturating_sub(g.turn) as f64
        } else {
            // Unlimited play has no terminal date. Use a rolling, speed-aware
            // investment window rather than treating turn one as the end.
            g.game_speed.turn_limit() as f64 * 0.16
        };
        let earning_turns = (remaining - self.production_build_turns(g, pid, cid, item)).max(0.0);
        let cost = g.item_remaining_cost_for_city(pid, cid, item);
        if earning_turns <= 0.0 {
            return 0.0;
        }
        (earning_turns * gain / cost.max(1.0)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod builder_tests;
