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

impl AdvancedAi {
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
