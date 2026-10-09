//! `capital-campus-before-the-plaza`: the capital's first Campus, then that
//! Campus's Library, claim the idle capital ahead of the Government Plaza
//! and the development shortfall's catch-up; the Plaza goes to the idle city
//! that produces most while the capital builds them.
//!
//! **Why (10-08/09 census, 145 live Emperor Gran Colombia runs).** The
//! capital's first specialty district was a Holy Site in 83 games and a
//! Government Plaza in 58, a Campus in none; the capital's Campus stood at a
//! median turn 86 (79 of 145 by turn 110) and its Library at 92, while the
//! Plaza stood at 46. `capital_campus_first` (`campus-before-harbor-2`) asks
//! for that Campus in `BasicAi::pick_item`, which an idle capital reaches
//! only after `reserve_government_plaza` and `reserve_higher_level_investment`
//! have had it: live civvis-20261009T084337Z put Bogotá (population 5, 11
//! Production, a Campus 5 turns off) on its Plaza at turn 44 and its
//! Warlord's Throne at 52, and opened the Campus at 60 "for
//! research-building-catchup". Our Science ran a median 0.21 of the best
//! rival's at turn 50, 0.26 at 75 and 0.39 at 100, and our technologies
//! 11/17/24 against 16/28/37: the gap opens before turn 75, when the capital
//! is most of the empire.
//!
//! The claim keeps every guard of the Plaza claim it runs beside: the opening
//! (`BasicAi::CAPITAL_CAMPUS_MIN_CITIES` cities), a capital due a Settler, a
//! threatened or recently attacked capital, an empire in economic recovery
//! and an open Prophet race all leave the capital alone, and the Campus or
//! Library must finish within `BasicAi::FIRST_CAMPUS_MAX_TURNS`.

use super::{AdvancedAi, StrategicPlan};
use crate::ai::BasicAi;
use crate::game::{Action, Game, Item};
use crate::think;

impl AdvancedAi {
    /// The capital's first Campus, else its Campus's Library, for the
    /// capital's idle queue; `None` with the gene off or behind any guard.
    pub(super) fn capital_campus_claim_item(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.capital_campus_before_the_plaza || self.base.minor || self.base.barb {
            return None;
        }
        let city_ids = g.player_city_ids(pid);
        if city_ids.len() < BasicAi::CAPITAL_CAMPUS_MIN_CITIES {
            return None;
        }
        // The economic-recovery bar `reserve_government_plaza` uses.
        let recovery_reserve = 100.0 + 25.0 * city_ids.len() as f64;
        if g.players[pid].gold_per_turn < -0.5 && g.players[pid].gold < recovery_reserve {
            return None;
        }
        if self.base.plaza_prophet_race_open(g, pid) {
            return None;
        }
        let capital = city_ids.iter().copied().find(|cid| {
            let city = &g.cities[cid];
            city.is_capital && city.original_owner == pid
        })?;
        let city = &g.cities[&capital];
        if !city.queue.is_empty()
            || plan.threatened_city == Some(capital)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let settlers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| g.units[unit].kind == "settler")
            .count()
            + city_ids
                .iter()
                .filter(|cid| {
                    matches!(g.cities[cid].queue.first(),
                        Some(Item::Unit { unit }) if unit == "settler")
                })
                .count();
        if self
            .base
            .settler_due(g, pid, capital, city_ids.len(), settlers)
        {
            return None;
        }
        BasicAi::first_campus_item(g, pid, capital)
            .or_else(|| BasicAi::campus_library_item(g, pid, capital))
            .map(|item| (capital, item))
    }

    /// See [`Self::capital_campus_claim_item`]: claims it. Runs just before
    /// `reserve_government_plaza`.
    pub(super) fn claim_capital_campus(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((cid, item)) = self.capital_campus_claim_item(g, pid, plan) else {
            return;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                "{} starts {} for capital-campus-before-the-plaza", g.cities[&cid].name, Self::plain_item(&item);
                "the capital's Campus and its Library take its idle queue ahead of the Plaza and the development shortfall; the Plaza goes to the idle city that produces most");
        }
    }

    /// `capital-campus-before-the-plaza`: whether the capital's queue leads
    /// with its Campus or a Campus building, which the Plaza claim treats as
    /// it treats a wonder or a Holy Site there (the Plaza goes elsewhere).
    pub(super) fn capital_builds_its_campus(&self, g: &Game, capital: u32) -> bool {
        self.capital_campus_before_the_plaza
            && match g.cities[&capital].queue.first() {
                Some(Item::District { district, .. }) => g.district_family(*district) == "campus",
                Some(Item::Building { building }) => g
                    .rules
                    .buildings
                    .get(building)
                    .and_then(|spec| spec.district)
                    .is_some_and(|district| g.district_family(district) == "campus"),
                _ => false,
            }
    }
}

#[cfg(test)]
mod tests;
