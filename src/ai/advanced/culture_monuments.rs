//! `culture-seat-monuments`: on a seat aiming at a Culture victory, every
//! unthreatened city of ours builds its Monument early, once the second city
//! stands, ahead of a deferrable head.
//!
//! **Why (live Emperor Pericles, 2026-10-30, civvis-20261030T090715Z — the
//! first Emperor Culture seat).**
//! - Six cities at t60 held ONE Monument between them (three at t80, five at
//!   t100), and the empire made 12 Culture a turn at t60 against Egypt's 56,
//!   55 at t100 against 121. The Culture race is our domestic tourists (our
//!   lifetime Culture) against the rivals', and the civics that open the
//!   Culture lane's buildings come from the same output.
//! - `culture-seat-theater-first` put the Acropolis in four of six cities by
//!   t80, but a Theater Square holds no Culture until its buildings and
//!   Great Works arrive; a Monument is two Culture a turn from the turn it is
//!   finished, for the cheapest building on the menu.
//! - `capital-monument-after-the-second-city` covers only the capital, and on
//!   the live seat the early Settler floor takes that queue first.
//!
//! **What the gene does.** For a Culture seat with two cities or more and no
//! major at war with us, a city with no Monument that can finish one within
//! `CAPITAL_MONUMENT_MAX_STANDARD` standard turns claims it: an idle queue,
//! or one over a deferrable head that keeps its progress (the Theater
//! claim's rule: a Settler, Walls, a wonder, a project, the faith defence's
//! buildings and anything finishing this turn keep their place). The heads
//! the other claims hold keep their place too, and the delegated governor's
//! rescoring leaves the claimed Monument alone
//! ([`AdvancedAi::culture_monument_holds`]).
//!
//! Off, or for a seat not aiming at Culture, nothing changes.

use super::capital_monument::{CAPITAL_MONUMENT_MAX_STANDARD, CAPITAL_MONUMENT_MIN_CITIES};
use super::*;

impl AdvancedAi {
    /// The Monument `cid` would claim and its build turns: a Culture seat's
    /// unthreatened city of ours with none built, once the second city
    /// stands. `None` with the gene off or for another victory target.
    pub(super) fn culture_monument_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.culture_seat_monuments
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || g.player_city_ids(pid).len() < CAPITAL_MONUMENT_MIN_CITIES
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let monument = BasicAi::civ_building(g, pid, cid, "monument")?;
        let turns = g.host_production_turns(cid, &monument).unwrap_or_else(|| {
            let remaining = g.item_cost_for_city(pid, cid, &monument)
                - g.item_invested_production(cid, &monument);
            remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= g.standard_duration(CAPITAL_MONUMENT_MAX_STANDARD) as f64)
            .then_some((monument, turns))
    }

    /// Whether `item` at the head of `cid` is the Monument this gene claims
    /// and the city still wants it: the delegated governor's rescoring leaves
    /// it alone. Always false with the gene off.
    pub(super) fn culture_monument_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.culture_monument_item(g, pid, cid, plan)
            .is_some_and(|(monument, _)| monument == *item)
    }

    /// See the module: every eligible city of a Culture seat claims its
    /// Monument. Exact no-op with the gene off or for another target.
    pub(super) fn claim_culture_monuments(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.culture_seat_monuments
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        if at_major_war {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((monument, turns)) = self.culture_monument_item(g, pid, cid, plan) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&monument) {
                continue;
            }
            if let Some(item) = &head {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.culture_theater_holds(g, pid, cid, plan, item)
                    || self.deterrence_unit_holds(g, pid, item)
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || self.backlog_builder_holds(g, pid, cid, plan, item)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                    || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: monument,
                },
            )
            .is_ok()
            {
                think!(self.journal(), Cities, Decision,
                       "{name} claims its Monument for the Culture victory";
                       "culture-seat-monuments: {turns:.1} turns, ahead of {}",
                       head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                           "{} (it keeps its progress)", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
