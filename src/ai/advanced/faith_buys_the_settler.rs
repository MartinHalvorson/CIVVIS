//! `faith-buys-the-settler`: in a Golden or Heroic Age an empire with seats
//! still to settle dedicates to Monumentality and buys its Settlers with
//! Faith.
//!
//! ## Why
//!
//! Operator, 2026-11-11: *"Go wide and really push the benefits to going wide
//! to the extremes."*
//!
//! What stops a wide empire is the Production its Settlers cost. On the
//! site's own worlds, majors that trained eleven more Settlers by turn 150
//! had five oldest cities making 17.7 Science each against 34.6 for stock
//! majors: each late Settler costs more than a University, and
//! `no-city-limit` therefore stops paying for the eleventh Settler near
//! Online turn 107 and the sixteenth near turn 90
//! (`advanced/no_city_limit.rs`).
//!
//! Faith is the one currency most seats cannot spend: half the seats of a
//! six-major game never found a religion and end with about a thousand Faith
//! banked (`faith_patronage`). Monumentality's Golden half lets Settlers and
//! Builders be bought with Faith at 30% off (`Game::unit_purchase_cost`), so a
//! Settler bought that way costs the old cities no Production at all. Stock
//! takes Monumentality only by the alphabetical or measured order, and buys
//! no Settler with Faith.
//!
//! ## What the gene does
//!
//! While the empire's cities, walking Settlers and Settlers in production
//! are short of its settlement target (`settlement_target`, which
//! `no-city-limit` lifts to every seat that pays), the plan is not Recovery
//! and the seat is not on a Religion lane (there the Faith is the lane's):
//!
//! 1. **The Dedication.** In a Golden or Heroic Age that offers
//!    Monumentality, it is taken first; the ordinary choice fills any slot
//!    left.
//! 2. **The purchase.** While Monumentality's Golden half is on, the biggest
//!    city that can give a citizen, is not the plan's threatened city or
//!    beside it and has no raider in its ring buys a Settler the turn the
//!    Faith covers it, for a holdable seat within reach
//!    (`holdable_land_seat`). At most [`FAITH_SETTLER_EXTRA_WIDTH`] more
//!    Settlers than the ordinary pipeline allows are out at once.
//!
//! With `no-city-limit` on, the seat must also pay: the test is asked with
//! the Faith price counted at [`FAITH_SETTLER_PRODUCTION_PER_FAITH`]
//! Production a point.
//!
//! Off (the default) nothing here runs.
use super::*;

/// Settlers walking or in production past the ordinary pipeline's width that
/// Faith may add.
pub(crate) const FAITH_SETTLER_EXTRA_WIDTH: usize = 1;

/// What a point of Faith is worth in Production when `no-city-limit` asks
/// whether a Faith-bought Settler's city pays. A seat off the Religion lane
/// has little else to buy with it.
pub(crate) const FAITH_SETTLER_PRODUCTION_PER_FAITH: f64 = 0.25;

/// The smallest city that buys a Settler: the purchase takes a citizen.
pub(crate) const FAITH_SETTLER_MIN_POP: i32 = 3;

const MONUMENTALITY: &str = "monumentality";

impl AdvancedAi {
    /// Whether the empire still has a seat to settle that Faith may pay for:
    /// the gene is on, a major seat, off the Religion lane, not in Recovery,
    /// and short of its settlement target counting every Settler out or in
    /// production. `(cities, in_flight, target)` when it does.
    pub(super) fn faith_settler_wanted(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(usize, usize, usize)> {
        if !self.faith_buys_the_settler
            || self.base.minor
            || self.base.barb
            || plan.strategy == GrandStrategy::Recovery
            || plan.strategy == GrandStrategy::Religion
            || self.active_victory_target(g) == Some(VictoryTarget::Religion)
        {
            return None;
        }
        let cities = g.player_city_ids(pid).len();
        let (walkers, queued) = Self::early_settler_census(g, pid);
        let target = self.settlement_target(plan);
        (cities > 0 && cities + walkers + queued < target).then_some((
            cities,
            walkers + queued,
            target,
        ))
    }

    /// Whether Monumentality's Golden half is on for `pid`.
    pub(super) fn monumentality_active(g: &Game, pid: usize) -> bool {
        g.players.get(pid).is_some_and(|player| {
            matches!(player.age.as_str(), "golden" | "heroic")
                && player.dedications.contains(MONUMENTALITY)
        })
    }

    /// See the module: take Monumentality first in a Golden or Heroic Age
    /// that offers it, while a seat is still to settle. Returns whether it
    /// was taken. Exact no-op with the gene off.
    pub(super) fn dedicate_to_monumentality(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((cities, in_flight, target)) = self.faith_settler_wanted(g, pid, plan) else {
            return false;
        };
        if !matches!(g.players[pid].age.as_str(), "golden" | "heroic") {
            return false;
        }
        let Some(dedication) = g
            .available_dedications(pid)
            .into_iter()
            .find(|name| name.as_str() == MONUMENTALITY)
        else {
            return false;
        };
        let taken = g
            .apply(pid, &Action::ChooseDedication { dedication })
            .is_ok();
        if taken {
            let age = g.players[pid].age.clone();
            think!(self.journal(), Diplomacy, Decision,
                "Dedicating the {age} age to Monumentality";
                "faith-buys-the-settler: {cities} cities and {in_flight} Settler(s) out \
                 against {target} seats; its Golden half sells Settlers for Faith at 30% off");
        }
        taken
    }

    /// See the module: buy one Settler with Faith under Monumentality.
    /// Returns whether one was bought. Exact no-op with the gene off.
    pub(super) fn claim_faith_settler(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((cities, in_flight, target)) = self.faith_settler_wanted(g, pid, plan) else {
            return false;
        };
        if !Self::monumentality_active(g, pid) {
            return false;
        }
        let width =
            self.no_city_limit_width(g, pid).unwrap_or(1).max(1) + FAITH_SETTLER_EXTRA_WIDTH;
        if in_flight >= width {
            return false;
        }
        let bank = g.players[pid].faith;
        let settler = Self::early_settler_item();
        let threats = Self::holdable_land_threats(g, pid);
        let mut candidates: Vec<(i32, u32)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                city.pop >= FAITH_SETTLER_MIN_POP
                    && Self::no_city_limit_city_is_clear(g, plan, *cid)
                    && !Self::no_city_limit_raider_holds(g, pid, city.pos)
                    && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    && g.can_produce(pid, *cid, &settler)
                    && !g.purchase_is_blocked(*cid, &settler)
            })
            .map(|cid| (g.cities[&cid].pop, cid))
            .collect();
        candidates.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        for (_, cid) in candidates {
            let Some(price) = g.unit_purchase_cost(pid, cid, "settler", "faith") else {
                continue;
            };
            if bank + f64::EPSILON < price {
                continue;
            }
            let from = g.cities[&cid].pos;
            let Ok((site, worth)) = self.holdable_land_seat(g, pid, from, in_flight, &threats)
            else {
                continue;
            };
            if worth < self.faith_settler_pay_floor(g, price) {
                continue;
            }
            let action = Action::Buy {
                city: cid,
                unit: crate::name!("settler"),
                formation: 0,
                currency: "faith".to_string(),
            };
            if g.apply(pid, &action).is_ok() {
                let name = g.cities[&cid].name.clone();
                think!(self.journal(), Expansion, Decision,
                    "Buying a Settler for {name} with Faith";
                    "faith-buys-the-settler: {price:.0} Faith from a bank of {bank:.0} under \
                     Monumentality; {cities} cities and {in_flight} Settler(s) out against \
                     {target} seats, the best holdable seat {site:?} worth {worth:.1}");
                return true;
            }
        }
        false
    }

    /// The least a seat may be worth for a Settler bought for `price` Faith:
    /// `no-city-limit`'s pay test with the Faith counted at
    /// [`FAITH_SETTLER_PRODUCTION_PER_FAITH`], and nothing with that gene
    /// off.
    pub(super) fn faith_settler_pay_floor(&self, g: &Game, price: f64) -> f64 {
        if !self.no_city_limit {
            return 0.0;
        }
        let end = f64::from(g.max_turns) * super::no_city_limit::NO_CITY_LIMIT_EXPECTED_END_SHARE;
        let standard_per_turn = 90.0 / f64::from(g.standard_duration(90).max(1));
        Self::settler_pay_floor(
            end - f64::from(g.turn),
            standard_per_turn,
            price * FAITH_SETTLER_PRODUCTION_PER_FAITH,
        )
    }
}

#[cfg(test)]
mod tests;
