//! `capital-monument-after-the-second-city`: once the empire's second city
//! stands, the capital puts its Monument at the head of its queue, once,
//! ahead of the controller's routine claims.
//!
//! **Why (live Immortal 2026-10-28/29, 108 games reaching turn 81).**
//! - Our culture runs a quarter of the rivals' from the start: a median 3.9 a
//!   turn at t30 against 16, 6.6 against 35 at t45, 10.7 against 54 at t60.
//!   Civics follow, 3 / 6 / 8 against 8 / 12 / 16, and Political Philosophy
//!   (the first real government and its two extra policy slots) comes at a
//!   median t43 (quartiles 40-48), while 37 of 95 games still sat in
//!   Chiefdom's two slots at t45.
//! - Our t30 culture is population and the Palace. A Monument (30
//!   Production at Online, +1 culture and +1 more at full loyalty) stood in
//!   2% of our cities at t20, 12% at t30 and 20% at t45. **The capital had
//!   none by turn 85 in more than half the games** (first quartile t49).
//! - `monument-first` lets the capital stop for it only from three cities,
//!   and its step in `BasicAi::pick_item` is reached only when every earlier
//!   claim declines, which the settler pump's queue never does.
//! - Among the fast expanders (third city by t26), the 8 capitals with a
//!   Monument within 8 turns of the third city reached Political Philosophy
//!   at a median t34 against t42 for the other 44, with a t80 Production
//!   share of 0.47 against 0.32 (observational).
//!
//! **What the gene does.** From the second city on, an unthreatened capital
//! that can finish its Monument within [`CAPITAL_MONUMENT_MAX_STANDARD`]
//! standard turns puts it at the head of the queue. It takes an idle queue
//! even with a Settler due (otherwise the pump never yields it), or one over
//! a deferrable item, which keeps its progress. A Settler in progress, Walls,
//! a wonder, a project, a repair, anything finishing this turn, and every
//! claim while a major is at war with us keep their queue. The delegated
//! governor's rescoring then leaves the claimed Monument alone
//! ([`AdvancedAi::capital_monument_holds`]). It runs once: a capital with a
//! Monument cannot build another.
//!
//! Off, nothing runs.

use super::*;

/// The longest the capital's Monument may take, in standard turns (ten live
/// turns at Online).
pub(crate) const CAPITAL_MONUMENT_MAX_STANDARD: u32 = 20;

/// Cities standing (not settlers on the road) before the capital stops for
/// its Monument.
pub(crate) const CAPITAL_MONUMENT_MIN_CITIES: usize = 2;

impl AdvancedAi {
    /// The capital's Monument and its build turns when the gene wants it in
    /// `cid`: the capital, the second city standing, no threat, buildable in
    /// time. `None` otherwise and with the gene off.
    pub(super) fn capital_monument_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.capital_monument_after_the_second_city {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if !city.is_capital
            || city.owner != pid
            || g.player_city_ids(pid).len() < CAPITAL_MONUMENT_MIN_CITIES
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let monument = BasicAi::civ_building(g, pid, cid, "monument")?;
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &monument).unwrap_or_else(|| {
            let remaining = g.item_cost_for_city(pid, cid, &monument)
                - g.item_invested_production(cid, &monument);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= g.standard_duration(CAPITAL_MONUMENT_MAX_STANDARD) as f64)
            .then_some((monument, turns))
    }

    /// Whether `item` at the head of `cid` is the Monument this gene claims
    /// and the capital still wants it: the delegated governor's rescoring
    /// leaves it alone. Always false with the gene off.
    pub(super) fn capital_monument_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.capital_monument_item(g, pid, cid, plan)
            .is_some_and(|(monument, _)| monument == *item)
    }

    /// Whether the capital's queue head `item` may wait for the Monument:
    /// not a Settler, Walls, a wonder, a project or a repair, and not
    /// finishing this turn.
    fn capital_monument_may_defer(g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Unit { unit } => *unit != "settler" && g.rules.units.contains_key(unit),
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
            }),
            Item::District { .. } => true,
            _ => false,
        }
    }

    /// See the module: the capital claims its Monument once the second city
    /// stands. Exact no-op with the gene off.
    pub(super) fn claim_capital_monument(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.capital_monument_after_the_second_city {
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
        let Some(cid) = g
            .player_city_ids(pid)
            .into_iter()
            .find(|cid| g.cities[cid].is_capital)
        else {
            return;
        };
        let Some((monument, turns)) = self.capital_monument_item(g, pid, cid, plan) else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        if head.as_ref() == Some(&monument) {
            return;
        }
        if let Some(item) = &head {
            if !Self::capital_monument_may_defer(g, pid, cid, item) {
                return;
            }
        }
        let name = g.cities[&cid].name.clone();
        let cities = g.player_city_ids(pid).len();
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
                   "{name} claims its Monument for the civics";
                   "capital-monument-after-the-second-city: {cities} cities standing, \
                    {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
