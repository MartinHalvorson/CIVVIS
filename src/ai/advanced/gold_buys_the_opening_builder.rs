//! `gold-buys-the-opening-builder`: in the opening, while the empire holds no
//! Builder and has work for one, at peace with every major and with no city
//! threatened, the treasury buys the Builder above the purchase upkeep floor.
//! It buys in the city already training one (that city's Production then goes
//! to its next item), else in the cheapest city with no barbarian raider in
//! its home ring. One purchase a turn.
//!
//! **Why (live King, 30 games of 11-01/02, random-leader 6p Continents).**
//! - The first Builder came at a median t33.5 (t4 to t63); the median seat
//!   held no Builder at t10, t20 or t30, laid a median 1 improvement by t40
//!   and chopped nothing before t40 (`chop-for-expansion` is armed but has
//!   no Builder to chop with). It held 0 improved luxury types at t40 and
//!   1.5 at t60, so it had no surplus to sell and no sale before t80.
//! - A Builder sells for 100 Gold at Online speed from t15 to t30, and the
//!   treasury held a median 86-106 Gold over those turns. In 40-45% of the
//!   t20-t30 seat-turns the empire had no Builder and Gold for one. The
//!   early Gold went to Traders (41 buys before t80), Warriors (38),
//!   Monuments (28) and Archers (24); Builders were bought 9 times in 30
//!   games.
//! - At Immortal (-8b, 10-30) the improved share of owned land stood at 5%
//!   against the rivals' 26% at t40. `opening-builder-after-the-first-settler`
//!   (#4244) trained the opening Builder with the capital's Production, which
//!   competes with the Settlers, and read neutral. Gold does not compete with
//!   the Settler queue.
//!
//! Called in the purchase phase after the city-saving and breaker purchases,
//! ahead of `gold-buys-the-idle-trader`. Off, nothing runs.

use super::*;

/// The window, in standard turns: 120 standard turns is live turn 80 at the
/// seat's Online speed.
pub(crate) const OPENING_BUILDER_END_STANDARD: u32 = 120;

/// A city attacked within this many turns is not where the Builder is bought.
const OPENING_BUILDER_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether the window is open for `pid` this turn: the gene, the opening,
    /// peace with every major, no threatened city, no Builder afield, and
    /// work for one. `false` with the gene off.
    pub(super) fn opening_builder_purchase_open(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        if !self.gold_buys_the_opening_builder
            || self.base.minor
            || self.base.barb
            || plan.threatened_city.is_some()
            || g.turn >= g.standard_duration(OPENING_BUILDER_END_STANDARD)
        {
            return false;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        });
        if at_major_war {
            return false;
        }
        let afield = g
            .player_unit_ids(pid)
            .into_iter()
            .any(|uid| g.units[&uid].kind == "builder");
        !afield && BasicAi::has_builder_work(g, pid)
    }

    /// See the module. True when a Builder was bought. Exact no-op with the
    /// gene off.
    pub(in crate::ai::advanced) fn opening_builder_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.opening_builder_purchase_open(g, pid, plan) {
            return false;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        // A Builder at the head of a queue that its city finishes next turn is
        // already the empire's Builder: buying it now spends about 100 Gold for
        // one turn, while Gold binds from t30 to t60. Live: 4 of the 30 games
        // of 2026-11-03 bought their own head Builder with one turn left (for
        // example Vimayapura t29 and Caracas t30, 100 Gold each).
        if g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            let rate = g.city_yields(cid).production;
            matches!(city.queue.first(), Some(Item::Unit { unit }) if *unit == "builder")
                && rate > 0.0
                && g.item_cost_for_city(pid, cid, &builder) - city.production <= rate + 1e-9
        }) {
            return false;
        }
        // (not already training one, price, city): the city training a
        // Builder first, then the cheapest, then the lowest id.
        let mut best: Option<(bool, i64, u32)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= OPENING_BUILDER_ATTACK_MEMORY;
            let raided = g.units.values().any(|unit| {
                BasicAi::is_barbarian_raider(g, unit)
                    && g.wdist(unit.pos, city.pos) <= crate::ai::HOME_THREAT_RADIUS
            });
            if attacked
                || raided
                || !g.can_produce(pid, cid, &builder)
                || g.purchase_is_blocked(cid, &builder)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, "builder", "gold") else {
                continue;
            };
            if price > budget + 1e-9 {
                continue;
            }
            let training = city
                .queue
                .iter()
                .any(|item| matches!(item, Item::Unit { unit } if *unit == "builder"));
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
                unit: crate::name!("builder"),
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Economy, Decision,
            "{} buys the empire's Builder with Gold", g.cities[&cid].name;
            "gold-buys-the-opening-builder: no Builder afield and work for one; {:.0} Gold of a {:.0} budget",
            price as f64 / 100.0, budget);
        true
    }
}

#[cfg(test)]
mod tests;
