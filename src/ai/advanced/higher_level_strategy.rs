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
            Self::Culture if ai.culture_building_catchup_2 => "culture-building-catchup-2",
            Self::Culture => "culture-building-catchup",
            Self::Research if ai.research_building_catchup_2 => "research-building-catchup-2",
            Self::Research => "research-building-catchup",
            Self::Builder if ai.builder_workforce_recovery_2 => "builder-workforce-recovery-2",
            Self::Builder => "builder-workforce-recovery",
        }
    }

    fn matches(self, g: &Game, item: &Item) -> bool {
        match (self, item) {
            (Self::Expansion, Item::Unit { unit }) => unit == "settler",
            (Self::Builder, Item::Unit { unit }) => unit == "builder",
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
    }
    /// Withhold the disciplined variant.
    pub fn disable_research_building_catchup_2(&mut self) {
        self.research_building_catchup_2 = false;
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
    }
    /// Withhold the disciplined variant.
    pub fn disable_culture_building_catchup_2(&mut self) {
        self.culture_building_catchup_2 = false;
    }
    /// Enable the independently screenable disciplined variant.
    pub fn enable_builder_workforce_recovery_2(&mut self) {
        self.builder_workforce_recovery_2 = true;
        self.builder_workforce_recovery = false;
    }
    /// Withhold the disciplined variant.
    pub fn disable_builder_workforce_recovery_2(&mut self) {
        self.builder_workforce_recovery_2 = false;
    }
    /// Enable `builder-workforce-recovery` for screening.
    pub fn enable_builder_workforce_recovery(&mut self) {
        self.builder_workforce_recovery = true;
        self.builder_workforce_recovery_2 = false;
    }
    /// Withhold `builder-workforce-recovery`.
    pub fn disable_builder_workforce_recovery(&mut self) {
        self.builder_workforce_recovery = false;
    }
    /// Enable `culture-building-catchup` for screening.
    pub fn enable_culture_building_catchup(&mut self) {
        self.culture_building_catchup = true;
        self.culture_building_catchup_2 = false;
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
        if !((self.builder_workforce_recovery || self.builder_workforce_recovery_2)
            || (self.culture_building_catchup || self.culture_building_catchup_2)
            || (self.expansion_best_idle_city || self.expansion_best_idle_city_2 || wide_cadence)
            || (self.research_building_catchup || self.research_building_catchup_2)
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
        if (self.culture_building_catchup || self.culture_building_catchup_2)
            || (self.research_building_catchup || self.research_building_catchup_2)
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
        if self.culture_building_catchup_2 && !rival_cultures.is_empty() {
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
                (self.culture_building_catchup || self.culture_building_catchup_2)
                    && ours.culture < 0.7 * best_culture,
            ),
            (
                Debt::Research,
                (self.research_building_catchup || self.research_building_catchup_2)
                    && ours.science < 0.7 * best_science,
            ),
            (
                Debt::Builder,
                (self.builder_workforce_recovery || self.builder_workforce_recovery_2)
                    && cities.len() >= 2
                    && counts.builders == 0
                    && (counts.settlers > 0 || cities.len() >= self.expansion_pace_now(g))
                    && super::BasicAi::has_builder_work(g, pid),
            ),
        ];
        // A queued answer anywhere already services this debt. Do not start
        // an empire-wide wave before the first answer has had time to finish.
        let debts: Vec<Debt> = debts
            .into_iter()
            .filter(|(debt, enabled)| {
                *enabled
                    && !cities.iter().any(|cid| {
                        // `expansion-scales-with-difficulty`: a Settler queued
                        // in the CAPITAL is the stall this gene exists to
                        // clear, not proof the debt is serviced. Every other
                        // city, and every other debt, still closes it, and
                        // `expansion_wide_cadence_admits` above has already
                        // capped how many walkers may be in flight at once.
                        if wide_cadence && *debt == Debt::Expansion && g.cities[cid].is_capital {
                            return false;
                        }
                        g.cities[cid].queue.iter().any(|item| debt.matches(g, item))
                    })
            })
            .map(|(debt, _)| debt)
            .collect();
        if debts.is_empty() {
            return None;
        }
        let mut best: Option<(Debt, f64, u32, String, Item)> = None;
        for cid in cities {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || self.base.barbarian_local_alarm_for_controller(g, pid, cid)
                || self.requisition_production_item(g, pid, cid).is_some()
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
                let Some(debt) = debts.iter().copied().find(|debt| debt.matches(g, &item)) else {
                    continue;
                };
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
                let gain = match (&item, debt) {
                    (Item::Building { building }, Debt::Culture) => {
                        g.rules.buildings[building].yields.culture
                    }
                    (Item::Building { building }, Debt::Research) => {
                        g.rules.buildings[building].yields.science
                    }
                    _ => 1.0,
                };
                let cost_per_gain = turns / gain.max(1.0);
                let key = format!("{item:?}");
                let replace =
                    best.as_ref()
                        .is_none_or(|(old_debt, old_cost, old_city, old_key, _)| {
                            debt.cmp(old_debt)
                                .then_with(|| cost_per_gain.total_cmp(old_cost))
                                .then_with(|| cid.cmp(old_city))
                                .then_with(|| key.cmp(old_key))
                                .is_lt()
                        });
                if replace {
                    best = Some((debt, cost_per_gain, cid, key, item));
                }
            }
        }
        best.map(|(debt, _, cid, _, item)| (cid, item, debt))
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
            Debt::Research if self.research_building_catchup_2 => {
                // Even between unlocked projects, a pad city should retain
                // its production for the race and its production upgrades.
                !city
                    .districts
                    .keys()
                    .any(|d| g.district_family(*d) == "spaceport")
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
