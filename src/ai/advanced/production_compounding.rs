//! Price the industrial foundation by time to repay the remaining investment.
//!
//! Named lanes already owe industrial buildings, but discretionary bids and
//! research reservations can still take every queue before that debt is paid.
//! Reserve profitable infrastructure and compare it with research explicitly.
//! Build time and projected production returned set the investment clock; the
//! separately screened adaptive treatment retains its measured expression.

use super::{AdvancedAi, ChainRung, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::rules::{BuildingSpec, Yields};
use crate::think;

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

    /// Purchase productive worker coverage without taking a construction queue.
    /// The forecast credits nearby charges generously; actual work and survival
    /// must still be measured in paired games rather than inferred from it.
    pub(super) fn productive_builder_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        reserve: f64,
    ) -> bool {
        let Some((cid, price, gain)) = self.productive_builder_purchase_candidate(g, pid, reserve)
        else {
            return false;
        };
        let action = Action::Buy {
            city: cid,
            unit: crate::name!("builder"),
            formation: 0,
            currency: "gold".into(),
        };
        if g.apply(pid, &action).is_err() {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let city_name = &g.cities[&cid].name;
            think!(self.journal(), Economy, Decision,
                "Buying productive Builder coverage for {city_name}";
                "{price:.0} Gold, retaining a {reserve:.0} reserve; {gain:.1} worked-tile Production forecast after nearby charge credits");
        }
        true
    }

    fn productive_builder_purchase_candidate(
        &self,
        g: &Game,
        pid: usize,
        reserve: f64,
    ) -> Option<(u32, f64, f64)> {
        if self.victory_target != Some(VictoryTarget::Domination)
            || self.base.minor
            || self.base.barb
            || g.turn > g.standard_duration(160)
            || g.players[pid].gold_per_turn <= 0.0
            || self.plan.as_ref().is_none_or(|plan| {
                plan.strategy == GrandStrategy::Recovery || plan.threatened_city.is_some()
            })
            || g.players.iter().enumerate().any(|(other, player)| {
                other != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.is_at_war(pid, other)
            })
        {
            return None;
        }
        let _memo = g.query_memo();
        let cities = g.player_city_ids(pid);
        if cities.len() < 2 || self.counts(g, pid).builders == 0 {
            return None;
        }
        let units = g.player_unit_ids(pid);
        let defenders = units
            .iter()
            .filter(|uid| {
                let spec = &g.rules.units[&g.units[uid].kind];
                spec.class == "military" && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .count();
        if defenders < cities.len() {
            return None;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        let due = g.standard_duration(10) as f64;
        let window = g.turn_limit().map_or(g.standard_duration(30), |limit| {
            g.standard_duration(30).min(limit.saturating_sub(g.turn))
        }) as f64;
        let earning = (window - due).max(0.0);
        let mut best: Option<(u32, f64, f64)> = None;
        for cid in &cities {
            let city = &g.cities[cid];
            if city.loyalty < 75.0
                || g.city_amenity_surplus(city) < 0
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || self.base.barbarian_local_alarm_for_controller(g, pid, *cid)
                || g.purchase_is_blocked(*cid, &builder)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, *cid, "builder", "gold") else {
                continue;
            };
            if g.players[pid].gold + f64::EPSILON < reserve + price
                || g.players[pid].gold_per_turn * due < price
            {
                continue;
            }
            let mut gains: Vec<_> = g
                .city_citizen_plan(*cid)
                .worked_tiles
                .into_iter()
                .filter_map(|pos| {
                    let tile = g.map.get(pos)?;
                    if pos == city.pos
                        || tile.owner_city != Some(*cid)
                        || tile.improvement.is_some()
                        || tile.district.is_some()
                        || g.wdist(city.pos, pos) > 3
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
                    (gain > 0.0).then_some(gain)
                })
                .collect();
            gains.sort_by(|a, b| b.total_cmp(a));
            let mut credited: usize = units
                .iter()
                .filter_map(|uid| {
                    let unit = &g.units[uid];
                    (unit.kind == "builder" && g.wdist(unit.pos, city.pos) <= 6)
                        .then_some(unit.charges.max(0) as usize)
                })
                .sum();
            for origin in &cities {
                let launch = &g.cities[origin];
                if matches!(launch.queue.first(), Some(Item::Unit { unit }) if unit == "builder")
                    && g.wdist(launch.pos, city.pos) <= 6
                    && self.production_build_turns(g, pid, *origin, &builder) <= due
                {
                    credited += g.builder_charges(pid).max(0) as usize
                        + g.governor_effect(pid, *origin, "builder_charges").max(0.0) as usize;
                }
            }
            let bought_charges = g.builder_charges(pid).max(0) as usize
                + g.governor_effect(pid, *cid, "builder_charges").max(0.0) as usize;
            let gain: f64 = gains.into_iter().skip(credited).take(bought_charges).sum();
            if gain < 3.0 || earning * gain < 1.25 * g.item_cost_for_city(pid, *cid, &builder) {
                continue;
            }
            if best.as_ref().is_none_or(|(old_cid, old_price, old_gain)| {
                gain / price > old_gain / old_price
                    || (gain / price == old_gain / old_price && cid < old_cid)
            }) {
                best = Some((*cid, price, gain));
            }
        }
        best
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod treasury_tests;
