//! `gold-buys-the-idle-trader`: while a trade route slot has no route and no
//! Trader afield to answer it, no city of ours is threatened and no major is
//! at war with us, the treasury buys the Trader for Gold above the purchase
//! upkeep floor. It buys in the city that is already training one (its
//! Production then goes to the next item), else in the safe origin
//! (`BasicAi::safe_trade_origin_for_controller`) that sells it cheapest. One
//! purchase a turn.
//!
//! **Why (live Emperor GC, 51 games of 10-17/18).** The first route slot
//! stood empty at t30 in 45 games, at t40 in 37, at t50 in 29 and at t60 in
//! 24, while the treasury held a median 121 to 150 Gold from t20 to t60 on
//! +7 to +10 a turn, and a Trader sold for about 95. `trader-fills-the-idle-route`
//! claims a production queue for the Trader, but it yields to the Settler
//! floor, the Granary, the deterrent and every protected head, so the slot
//! waits. A route of the time pays 3 to 6 Gold, or Food and Production to
//! the origin, every turn and lays a road. `gold-buys-the-settler` never
//! bought because a Settler's price climbs past the income. A Trader's does
//! not, and the bank already holds it.
//!
//! Called in the purchase phase after the city-saving and breaker purchases.
//! Off, nothing runs.

use super::*;

impl AdvancedAi {
    /// See the module. True when a Trader was bought. Exact no-op with the
    /// gene off.
    pub(in crate::ai::advanced) fn idle_trader_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.gold_buys_the_idle_trader || plan.threatened_city.is_some() {
            return false;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        if at_major_war {
            return false;
        }
        let capacity = g.trade_capacity(pid);
        let afield = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "trader")
            .count() as i64;
        if capacity - g.active_routes(pid) - afield <= 0 {
            return false;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let trader = Item::Unit {
            unit: crate::name!("trader"),
        };
        // (not already training one, price, city): the city training a
        // Trader first, then the cheapest, then the lowest id.
        let mut best: Option<(bool, i64, u32)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            if (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || !self.base.safe_trade_origin_for_controller(g, pid, cid)
                || !g.can_produce(pid, cid, &trader)
                || g.purchase_is_blocked(cid, &trader)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, "trader", "gold") else {
                continue;
            };
            if price > budget + 1e-9 {
                continue;
            }
            let training = city
                .queue
                .iter()
                .any(|item| matches!(item, Item::Unit { unit } if *unit == "trader"));
            let key = (!training, (price * 100.0).round() as i64, cid);
            if best.as_ref().is_none_or(|old| key < *old) {
                best = Some(key);
            }
        }
        let Some((_, price, cid)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: cid,
                unit: crate::name!("trader"),
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Economy, Decision,
            "{} buys a Trader for the empty route slot", g.cities[&cid].name;
            "gold-buys-the-idle-trader: {} slot(s), {} route(s), {} Trader(s) afield; {:.0} Gold",
            capacity, g.active_routes(pid), afield, price as f64 / 100.0);
        true
    }
}
