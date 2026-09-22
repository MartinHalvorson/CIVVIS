//! Five independently screenable responses to development bottlenecks.
//! See docs/eval/2026-09-08-higher-level-strategy.md for evidence and limits.
//! Reserve at most one idle queue per call, after victory-project reservations
//! and before either generic governor. Never replace work already under way.
use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::reasoning::Level;
use crate::think;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Debt {
    Trade,
    Expansion,
    Culture,
    Research,
    Builder,
}

impl Debt {
    fn tag(self, ai: &AdvancedAi) -> &'static str {
        match self {
            Self::Trade if ai.trade_building_before_bankruptcy_2 => {
                "trade-building-before-bankruptcy-2"
            }
            Self::Trade => "trade-building-before-bankruptcy",
            Self::Expansion if ai.expansion_best_idle_city_2 => "expansion-best-idle-city-2",
            Self::Expansion => "expansion-best-idle-city",
            Self::Culture if ai.culture_building_catchup_3 => "culture-building-catchup-3",
            Self::Culture if ai.culture_building_catchup_2 => "culture-building-catchup-2",
            Self::Culture => "culture-building-catchup",
            Self::Research if ai.research_building_catchup_3 => "research-building-catchup-3",
            Self::Research if ai.research_building_catchup_2 => "research-building-catchup-2",
            Self::Research => "research-building-catchup",
            Self::Builder if ai.builder_workforce_recovery_3 => "builder-workforce-recovery-3",
            Self::Builder if ai.builder_workforce_recovery_2 => "builder-workforce-recovery-2",
            Self::Builder => "builder-workforce-recovery",
        }
    }

    fn matches(self, ai: &AdvancedAi, g: &Game, item: &Item) -> bool {
        match (self, item) {
            (Self::Expansion, Item::Unit { unit }) => unit == "settler",
            (Self::Builder, Item::Unit { unit }) => unit == "builder",
            (Self::Research, Item::District { district, .. }) => {
                ai.active_victory_target(g) == Some(super::VictoryTarget::Domination)
                    && g.district_family(*district) == "campus"
            }
            (_, Item::Building { building }) => {
                let spec = &g.rules.buildings[building];
                if spec.wonder {
                    return false;
                }
                match self {
                    Self::Trade => {
                        g.building_is_family(*building, crate::name!("market"))
                            || g.building_is_family(*building, crate::name!("lighthouse"))
                    }
                    Self::Culture => spec.yields.culture > 0.0,
                    Self::Research => {
                        spec.yields.science > 0.0
                            && spec
                                .district
                                .is_some_and(|d| g.district_family(d) == "campus")
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// A foundation is eligible work, but does not replace a building that
    /// an already completed Campus can start now. Otherwise finishing one
    /// district suppresses Libraries elsewhere for its whole build time.
    fn queued_answer(self, ai: &AdvancedAi, g: &Game, item: &Item) -> bool {
        self.matches(ai, g, item)
            && !matches!((self, item), (Self::Research, Item::District { .. }))
    }

    fn prices_queued_yield(self, ai: &AdvancedAi) -> bool {
        match self {
            Self::Culture => ai.culture_building_catchup_3,
            Self::Research => ai.research_building_catchup_3,
            _ => false,
        }
    }

    fn building_gain(self, g: &Game, item: &Item) -> f64 {
        match (self, item) {
            (Self::Culture, Item::Building { building }) => {
                g.rules.buildings[building].yields.culture
            }
            (Self::Research, Item::Building { building }) => {
                g.rules.buildings[building].yields.science
            }
            (Self::Research, Item::District { district, pos }) => {
                // Credit only the district's own yield. The future Library
                // still needs its own construction and catch-up reservation.
                g.district_yields(*district, *pos).science.max(0.0)
            }
            _ => 1.0,
        }
    }
}

impl AdvancedAi {
    /// Enable the independently screenable disciplined variant.
    pub fn enable_trade_building_before_bankruptcy_2(&mut self) {
        self.trade_building_before_bankruptcy_2 = true;
        self.trade_building_before_bankruptcy = false;
    }
    /// Withhold the disciplined variant.
    pub fn disable_trade_building_before_bankruptcy_2(&mut self) {
        self.trade_building_before_bankruptcy_2 = false;
    }
    /// Enable the independently screenable disciplined variant.
    pub fn enable_research_building_catchup_2(&mut self) {
        self.research_building_catchup_2 = true;
        self.research_building_catchup = false;
        self.research_building_catchup_3 = false;
    }
    /// Withhold the disciplined variant.
    pub fn disable_research_building_catchup_2(&mut self) {
        self.research_building_catchup_2 = false;
    }

    /// Credit only the science queued to arrive within the next investment's
    /// useful window, and reserve enough additional yield to close the gap.
    pub fn enable_research_building_catchup_3(&mut self) {
        self.research_building_catchup = false;
        self.research_building_catchup_2 = false;
        self.research_building_catchup_3 = true;
    }

    pub fn disable_research_building_catchup_3(&mut self) {
        self.research_building_catchup_3 = false;
    }
    /// Enable the independently screenable disciplined variant.
    pub fn enable_expansion_best_idle_city_2(&mut self) {
        self.expansion_best_idle_city_2 = true;
        self.expansion_best_idle_city = false;
    }
    /// Withhold the disciplined variant.
    pub fn disable_expansion_best_idle_city_2(&mut self) {
        self.expansion_best_idle_city_2 = false;
    }
    /// Enable the independently screenable disciplined variant.
    pub fn enable_culture_building_catchup_2(&mut self) {
        self.culture_building_catchup_2 = true;
        self.culture_building_catchup = false;
        self.culture_building_catchup_3 = false;
    }
    /// Withhold the disciplined variant.
    pub fn disable_culture_building_catchup_2(&mut self) {
        self.culture_building_catchup_2 = false;
    }

    /// Fill the culture gap left by buildings already due to finish soon,
    /// retaining v2's median-rival target instead of chasing one specialist.
    pub fn enable_culture_building_catchup_3(&mut self) {
        self.culture_building_catchup = false;
        self.culture_building_catchup_2 = false;
        self.culture_building_catchup_3 = true;
    }

    pub fn disable_culture_building_catchup_3(&mut self) {
        self.culture_building_catchup_3 = false;
    }
    /// Enable the independently screenable disciplined variant.
    pub fn enable_builder_workforce_recovery_2(&mut self) {
        self.builder_workforce_recovery_2 = true;
        self.builder_workforce_recovery = false;
        self.builder_workforce_recovery_3 = false;
    }
    /// Withhold the disciplined variant.
    pub fn disable_builder_workforce_recovery_2(&mut self) {
        self.builder_workforce_recovery_2 = false;
    }

    /// Replace a lost Builder for three local new-improvement or repair jobs.
    pub fn enable_builder_workforce_recovery_3(&mut self) {
        self.builder_workforce_recovery = false;
        self.builder_workforce_recovery_2 = false;
        self.builder_workforce_recovery_3 = true;
    }

    pub fn disable_builder_workforce_recovery_3(&mut self) {
        self.builder_workforce_recovery_3 = false;
    }
    /// Enable `builder-workforce-recovery` for screening.
    pub fn enable_builder_workforce_recovery(&mut self) {
        self.builder_workforce_recovery = true;
        self.builder_workforce_recovery_2 = false;
        self.builder_workforce_recovery_3 = false;
    }
    /// Withhold `builder-workforce-recovery`.
    pub fn disable_builder_workforce_recovery(&mut self) {
        self.builder_workforce_recovery = false;
    }
    /// Enable `culture-building-catchup` for screening.
    pub fn enable_culture_building_catchup(&mut self) {
        self.culture_building_catchup = true;
        self.culture_building_catchup_2 = false;
        self.culture_building_catchup_3 = false;
    }
    /// Withhold `culture-building-catchup`.
    pub fn disable_culture_building_catchup(&mut self) {
        self.culture_building_catchup = false;
    }
    /// Enable `expansion-best-idle-city` for screening.
    pub fn enable_expansion_best_idle_city(&mut self) {
        self.expansion_best_idle_city = true;
        self.expansion_best_idle_city_2 = false;
    }
    /// Withhold `expansion-best-idle-city`.
    pub fn disable_expansion_best_idle_city(&mut self) {
        self.expansion_best_idle_city = false;
    }
    /// Enable `research-building-catchup` for screening.
    pub fn enable_research_building_catchup(&mut self) {
        self.research_building_catchup = true;
        self.research_building_catchup_2 = false;
        self.research_building_catchup_3 = false;
    }
    /// Withhold `research-building-catchup`.
    pub fn disable_research_building_catchup(&mut self) {
        self.research_building_catchup = false;
    }
    /// Enable `trade-building-before-bankruptcy` for screening.
    pub fn enable_trade_building_before_bankruptcy(&mut self) {
        self.trade_building_before_bankruptcy = true;
        self.trade_building_before_bankruptcy_2 = false;
    }
    /// Withhold `trade-building-before-bankruptcy`.
    pub fn disable_trade_building_before_bankruptcy(&mut self) {
        self.trade_building_before_bankruptcy = false;
    }

    fn higher_level_investment_target(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item, Debt)> {
        // `expansion-scales-with-difficulty` shares the Expansion arm with the
        // `expansion-best-idle-city` family and supplies its horizon, pace and
        // walker rule; all three are exactly the shipped ones while it is
        // off. See `advanced/expansion_scales_with_difficulty.rs`.
        let wide_cadence = self.expansion_wide_level(g).is_some();
        if !((self.builder_workforce_recovery
            || self.builder_workforce_recovery_2
            || self.builder_workforce_recovery_3)
            || (self.culture_building_catchup
                || self.culture_building_catchup_2
                || self.culture_building_catchup_3)
            || (self.expansion_best_idle_city || self.expansion_best_idle_city_2 || wide_cadence)
            || (self.research_building_catchup
                || self.research_building_catchup_2
                || self.research_building_catchup_3)
            || (self.trade_building_before_bankruptcy || self.trade_building_before_bankruptcy_2))
            || self.base.minor
            || self.base.barb
            || plan.strategy == GrandStrategy::Recovery
            || self.war_plan.is_some()
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        let counts = self.counts(g, pid);
        if self.live_war_economy_requires_recovery(g, pid, &counts) {
            return None;
        }
        // The same observed yield correction used by the existing culture
        // floor; only contacted, living majors set the catch-up bar.
        let yields = |seat| {
            let mut sum = crate::rules::Yields::default();
            for cid in g.player_city_ids(seat) {
                let y = g.city_yields(cid);
                sum.culture += y.culture;
                sum.science += y.science;
            }
            if let Some(y) = g.observed_yield_adjustments.get(&seat) {
                sum.culture += y.culture;
                sum.science += y.science;
            }
            sum
        };
        let ours = yields(pid);
        let mut best_culture = 0.0_f64;
        let mut rival_cultures = Vec::new();
        let mut best_science = 0.0_f64;
        if (self.culture_building_catchup
            || self.culture_building_catchup_2
            || self.culture_building_catchup_3)
            || (self.research_building_catchup
                || self.research_building_catchup_2
                || self.research_building_catchup_3)
        {
            for p in &g.players {
                if p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.has_met(pid, p.id)
                {
                    let y = yields(p.id);
                    best_culture = best_culture.max(y.culture);
                    rival_cultures.push(y.culture.max(0.0));
                    best_science = best_science.max(y.science);
                }
            }
        }
        if (self.culture_building_catchup_2 || self.culture_building_catchup_3)
            && !rival_cultures.is_empty()
        {
            rival_cultures.sort_by(f64::total_cmp);
            let n = rival_cultures.len();
            best_culture = (rival_cultures[(n - 1) / 2] + rival_cultures[n / 2]) / 2.0;
        }
        let capacity = g.trade_capacity(pid).max(0) as usize;
        let debts = [
            (
                Debt::Trade,
                (self.trade_building_before_bankruptcy || self.trade_building_before_bankruptcy_2)
                    && g.players[pid].gold_per_turn < 2.0 * cities.len() as f64
                    && capacity > 0
                    && counts.traders >= capacity,
            ),
            (
                Debt::Expansion,
                (self.expansion_best_idle_city || self.expansion_best_idle_city_2 || wide_cadence)
                    && g.turn <= self.expansion_cadence_horizon(g)
                    && cities.len() < self.expansion_pace_now(g)
                    && cities.len() < self.settlement_target(plan)
                    && self.expansion_wide_cadence_admits(g, pid, counts.settlers),
            ),
            (
                Debt::Culture,
                (self.culture_building_catchup
                    || self.culture_building_catchup_2
                    || self.culture_building_catchup_3)
                    && ours.culture < 0.7 * best_culture,
            ),
            (
                Debt::Research,
                (self.research_building_catchup
                    || self.research_building_catchup_2
                    || self.research_building_catchup_3)
                    && ours.science < 0.7 * best_science,
            ),
            (
                Debt::Builder,
                (self.builder_workforce_recovery
                    || self.builder_workforce_recovery_2
                    || self.builder_workforce_recovery_3)
                    && cities.len() >= 2
                    && counts.builders == 0
                    && (counts.settlers > 0 || cities.len() >= self.expansion_pace_now(g))
                    && (self.builder_workforce_recovery_3
                        || super::BasicAi::has_builder_work(g, pid)),
            ),
        ];
        // Earlier versions let any queued answer service the whole debt. V3
        // accounts for its size and completion time below, without changing
        // the one-at-a-time rule for expansion, builders or trade capacity.
        let debts: Vec<Debt> = debts
            .into_iter()
            .filter(|(debt, enabled)| {
                *enabled
                    && (debt.prices_queued_yield(self)
                        || !cities.iter().any(|cid| {
                            // `expansion-scales-with-difficulty`: a Settler queued
                            // in the CAPITAL is the stall this gene exists to
                            // clear, not proof the debt is serviced. Every other
                            // city, and every other debt, still closes it, and
                            // `expansion_wide_cadence_admits` above has already
                            // capped how many walkers may be in flight at once.
                            if wide_cadence && *debt == Debt::Expansion && g.cities[cid].is_capital
                            {
                                return false;
                            }
                            g.cities[cid]
                                .queue
                                .iter()
                                .any(|item| debt.queued_answer(self, g, item))
                        }))
            })
            .map(|(debt, _)| debt)
            .collect();
        if debts.is_empty() {
            return None;
        }
        // Only work actually at the front of a queue has this completion
        // estimate. A promised Library behind a long wonder is not an
        // imminent answer. Compute these once for all candidate cities.
        let queued_yields: Vec<(Debt, f64, f64)> = cities
            .iter()
            .filter_map(|cid| {
                let item = g.cities[cid].queue.first()?;
                let debt = debts
                    .iter()
                    .copied()
                    .find(|debt| debt.prices_queued_yield(self) && debt.matches(self, g, item))?;
                if g.city_yields(*cid).production <= 0.0 {
                    return None;
                }
                Some((
                    debt,
                    self.production_build_turns(g, pid, *cid, item)
                        .ceil()
                        .max(1.0),
                    debt.building_gain(g, item),
                ))
            })
            .collect();
        // Research catch-up may open its missing foundation, but only one
        // new Campus at a time. Libraries in completed districts still have
        // their own queue and retain priority over another new foundation.
        let campus_committed = cities.iter().any(|cid| {
            g.cities[cid].queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == "campus")
            })
        });
        let mut best: Option<(Debt, bool, f64, u32, String, Item)> = None;
        for cid in cities {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || self.base.barbarian_local_alarm_for_controller(g, pid, cid)
            {
                continue;
            }
            // An unfilled first route and local defence retain their reserves.
            if self
                .base
                .should_add_trader_in_city_for_controller(g, pid, cid, counts.traders)
                && counts.traders == 0
            {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let Some(debt) = debts
                    .iter()
                    .copied()
                    .find(|debt| debt.matches(self, g, &item))
                else {
                    continue;
                };
                let new_research_foundation = debt == Debt::Research
                    && matches!(&item, Item::District { .. })
                    && !Self::placed_campus_research_item(g, &item);
                if new_research_foundation
                    && (city.pop < super::CAMPUS_EVERY_CITY_POP_FLOOR || campus_committed)
                {
                    continue;
                }
                // Market and Lighthouse share one capacity tier in a city.
                if debt == Debt::Trade
                    && (city
                        .buildings
                        .iter()
                        .any(|b| g.building_is_family(*b, crate::name!("market")))
                        || city
                            .buildings
                            .iter()
                            .any(|b| g.building_is_family(*b, crate::name!("lighthouse"))))
                {
                    continue;
                }
                let turns = self
                    .production_build_turns(g, pid, cid, &item)
                    .ceil()
                    .max(1.0);
                // Complete with at least twenty standard turns left to use
                // the investment. Respect the actual clock when one exists.
                if g.turn_limit().is_some_and(|limit| {
                    turns + g.standard_duration(20) as f64 > limit.saturating_sub(g.turn) as f64
                }) {
                    continue;
                }
                if debt == Debt::Expansion
                    && (self.production_value(g, pid, cid, &item, plan, &counts) <= 0.0
                        || self
                            .settler_site_gate(g, pid, city.pos, counts.settlers)
                            .is_err())
                {
                    continue;
                }
                // Builders need usable work near their own launch city, not
                // merely an improvement opportunity on a distant island.
                if debt == Debt::Builder
                    && !self.builder_workforce_recovery_3
                    && !city.owned_tiles.iter().any(|pos| {
                        g.map.get(*pos).is_some_and(|tile| {
                            tile.improvement.is_none()
                                && !tile.pillaged
                                && tile.district.is_none()
                                && g.valid_improvements(pid, *pos)
                                    .iter()
                                    .any(|imp| g.rules.improvements[imp].builder_buildable)
                        })
                    })
                {
                    continue;
                }
                if !self.disciplined_investment_admitted(g, pid, cid, &item, debt, turns) {
                    continue;
                }
                let mut gain = debt.building_gain(g, &item);
                let cost_per_gain = if debt.prices_queued_yield(self) {
                    let shortfall = match debt {
                        Debt::Culture => 0.7 * best_culture - ours.culture,
                        Debt::Research => 0.7 * best_science - ours.science,
                        _ => unreachable!("only culture and research price queued yield"),
                    };
                    // Wait for enough queued yield that arrives within the
                    // same twenty-standard-turn payoff window required of
                    // this new investment. Otherwise credit only its unmet
                    // share when ranking the remaining candidate builds.
                    let due = turns + g.standard_duration(20) as f64;
                    let incoming: f64 = queued_yields
                        .iter()
                        .filter(|(queued_debt, eta, _)| *queued_debt == debt && *eta <= due)
                        .map(|(_, _, yield_gain)| yield_gain)
                        .sum();
                    gain = gain.min((shortfall - incoming).max(0.0));
                    if gain <= f64::EPSILON {
                        continue;
                    }
                    turns / gain
                } else {
                    turns / gain.max(1.0)
                };
                let key = format!("{item:?}");
                let replace = best.as_ref().is_none_or(
                    |(old_debt, old_foundation, old_cost, old_city, old_key, _)| {
                        debt.cmp(old_debt)
                            .then_with(|| new_research_foundation.cmp(old_foundation))
                            .then_with(|| cost_per_gain.total_cmp(old_cost))
                            .then_with(|| cid.cmp(old_city))
                            .then_with(|| key.cmp(old_key))
                            .is_lt()
                    },
                );
                if replace {
                    best = Some((debt, new_research_foundation, cost_per_gain, cid, key, item));
                }
            }
        }
        best.map(|(debt, _, _, cid, _, item)| (cid, item, debt))
    }

    /// Additional admission tests belong to the new versions only. The old
    /// arms remain reproducible when selected alone.
    fn disciplined_investment_admitted(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        debt: Debt,
        turns: f64,
    ) -> bool {
        let city = &g.cities[&cid];
        match debt {
            Debt::Expansion if self.expansion_best_idle_city_2 => {
                // Reserve population and a ten-standard-turn travel/founding
                // allowance; this is a conservative deadline, not a path ETA.
                city.pop >= 4
                    && turns + g.standard_duration(10) as f64
                        <= Self::expansion_band_turn(g).saturating_sub(g.turn) as f64
            }
            Debt::Trade if self.trade_building_before_bankruptcy_2 => {
                let trader = Item::Unit {
                    unit: crate::name!("trader"),
                };
                let trader_turns = self
                    .production_build_turns(g, pid, cid, &trader)
                    .ceil()
                    .max(1.0);
                let maintenance = match item {
                    Item::Building { building } => g.rules.buildings[building].maintenance,
                    _ => return false,
                };
                // Credit no speculative route income before the unit exists.
                let gold = g.players[pid].gold;
                let income = g.players[pid].gold_per_turn;
                let at_building = gold + income * turns;
                let at_trader = at_building + (income - maintenance) * trader_turns;
                at_building >= 0.0
                    && at_trader >= 0.0
                    && g.turn_limit().is_none_or(|limit| {
                        turns + trader_turns + g.standard_duration(20) as f64
                            <= limit.saturating_sub(g.turn) as f64
                    })
            }
            Debt::Research
                if self.research_building_catchup_2 || self.research_building_catchup_3 =>
            {
                // Even between unlocked projects, a pad city should retain
                // its production for the race and its production upgrades.
                !city
                    .districts
                    .keys()
                    .any(|d| g.district_family(*d) == "spaceport")
            }
            Debt::Builder if self.builder_workforce_recovery_3 => {
                // Count distinct local jobs, including repairs which do not
                // consume a charge. A stale tile claim earns no credit.
                let _memo = g.query_memo();
                city.owned_tiles
                    .iter()
                    .filter(|pos| {
                        g.map.get(**pos).is_some_and(|tile| {
                            tile.owner_city == Some(cid)
                                && ((tile.pillaged && tile.improvement.is_some())
                                    || (tile.improvement.is_none()
                                        && !tile.pillaged
                                        && tile.district.is_none()
                                        && g.valid_improvements(pid, **pos).iter().any(|imp| {
                                            g.rules.improvements[imp].builder_buildable
                                        })))
                        })
                    })
                    .take(3)
                    .count()
                    >= 3
            }
            Debt::Builder if self.builder_workforce_recovery_2 => {
                city.owned_tiles
                    .iter()
                    .filter(|pos| {
                        g.map.get(**pos).is_some_and(|tile| {
                            tile.improvement.is_none()
                                && !tile.pillaged
                                && tile.district.is_none()
                                && g.valid_improvements(pid, **pos)
                                    .iter()
                                    .any(|imp| g.rules.improvements[imp].builder_buildable)
                        })
                    })
                    .take(3)
                    .count()
                    >= 3
            }
            _ => true,
        }
    }

    pub(super) fn reserve_higher_level_investment(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((city, item, debt)) = self.higher_level_investment_target(g, pid, plan) else {
            return;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
            && self.journal().wants(Level::Decision)
        {
            think!(self.journal(), Economy, Decision,
                "{} starts {} for {}", g.cities[&city].name, Self::plain_item(&item), debt.tag(self);
                "one safe idle queue services the development shortfall");
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod repair_recovery_tests;

#[cfg(test)]
mod queued_yield_tests;

#[cfg(test)]
mod campus_foundation_tests;
