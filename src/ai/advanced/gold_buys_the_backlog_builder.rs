//! `gold-buys-the-backlog-builder` (opt-in): past the opening, while the
//! empire holds fewer Builders than its unimproved worked tiles call for, the
//! treasury buys one.
//!
//! **Why (live, 38 Emperor/Immortal games on 2026-11-02).**
//! - Of the worked land tiles, 71% stood unimproved at t40, 64% at t60, 48%
//!   at t80 and 36% at t100. Per game, 4.7, 6.5 and 6.3 of them could take an
//!   improvement now (a resource, hills, flat farmland) at t40, t60 and t80,
//!   against 1.0, 1.6 and 2.3 Builders afield.
//! - `builder_backlog_item` (`builder-before-the-army-3`, armed) asks for one
//!   Builder per three of those tiles, at most one per two cities. In 17 of
//!   38 games at t40 and 13 at t60 the Builders afield plus those training
//!   fell short of that quota; its one-per-two-cities cap bound in only 5-8.
//!   The queues were on Campus (`campus-unlock-every-lane`), Granary, Walls
//!   and Settler claims, each of which the backlog step sits behind.
//! - Gold does not compete with those queues. A displaced soldier costs
//!   cities at Immortal (`first-campus-takes-a-filler`, parked), so the
//!   Builder is bought rather than moved up the queue.
//!
//! **The rule.** In the purchase phase, after the opening Builder
//! (`gold-buys-the-opening-builder`) and the idle route's Trader, at peace
//! with every major, with no city threatened, before standard turn 225 (live
//! t150), with two or more cities: when Builders afield plus those in any
//! queue are below `builder_backlog_item`'s quota, buy one Builder above the
//! purchase upkeep floor. The city is the one owning the most unimproved
//! worked tiles, the weakest producer on a tie (Gold is worth most where
//! Production is scarce), then the cheapest. A city whose slotted cards
//! already speed a Builder (Ilkum, Public Works) trains its own and is
//! skipped, as is one attacked within four turns or with a barbarian raider
//! in its home ring. One purchase a turn. Off, nothing runs.

use super::*;

/// The window, in standard turns: 225 standard turns is live turn 150 at the
/// seat's Online speed. The unimproved share of worked land falls to about
/// a fifth by then.
pub(crate) const BACKLOG_BUILDER_END_STANDARD: u32 = 225;

/// A city attacked within this many turns is not where the Builder is bought.
const BACKLOG_BUILDER_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The Builders `builder_backlog_item` asks for and the ones the empire
    /// holds or trains, when the window is open. `None` with the gene off.
    pub(super) fn backlog_builder_shortfall(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(usize, usize)> {
        if !self.gold_buys_the_backlog_builder
            || self.base.minor
            || self.base.barb
            || plan.threatened_city.is_some()
            || g.turn >= g.standard_duration(BACKLOG_BUILDER_END_STANDARD)
            || self.opening_builder_purchase_open(g, pid, plan)
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() < 2 {
            return None;
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
            return None;
        }
        let afield = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "builder")
            .count();
        let training: usize = cities
            .iter()
            .map(|cid| {
                g.cities[cid]
                    .queue
                    .iter()
                    .filter(|item| matches!(item, Item::Unit { unit } if *unit == "builder"))
                    .count()
            })
            .sum();
        let quota = BasicAi::unimproved_worked_tiles(g, pid)
            .div_ceil(3)
            .min(cities.len().div_ceil(2));
        let held = afield + training;
        (held < quota).then_some((quota, held))
    }

    pub(in crate::ai::advanced) fn backlog_builder_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((quota, held)) = self.backlog_builder_shortfall(g, pid, plan) else {
            return false;
        };
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        // Most unimproved worked tiles first, then the weakest producer, then
        // the cheapest price, then the lowest id.
        let mut best: Option<(std::cmp::Reverse<usize>, i64, i64, u32)> = None;
        {
            let _memo = g.query_memo();
            for cid in g.player_city_ids(pid) {
                let city = &g.cities[&cid];
                let attacked = city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= BACKLOG_BUILDER_ATTACK_MEMORY;
                let raided = g.units.values().any(|unit| {
                    BasicAi::is_barbarian_raider(g, unit)
                        && g.wdist(unit.pos, city.pos) <= crate::ai::HOME_THREAT_RADIUS
                });
                if attacked
                    || raided
                    || !g.can_produce(pid, cid, &builder)
                    || g.purchase_is_blocked(cid, &builder)
                    || g.item_prod_mult(pid, cid, Some(&builder)) > 1.0 + 1e-9
                {
                    continue;
                }
                let own = BasicAi::unimproved_worked_tiles_in(g, pid, cid);
                if own == 0 {
                    continue;
                }
                let Some(price) = g.unit_purchase_cost(pid, cid, "builder", "gold") else {
                    continue;
                };
                if price > budget + 1e-9 {
                    continue;
                }
                let production = g.city_yields(cid).production;
                let key = (
                    std::cmp::Reverse(own),
                    (production * 100.0).round() as i64,
                    (price * 100.0).round() as i64,
                    cid,
                );
                if best.as_ref().is_none_or(|old| key < *old) {
                    best = Some(key);
                }
            }
        }
        let Some((std::cmp::Reverse(own), _, price, cid)) = best else {
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
            "{} buys a Builder for its unimproved worked tiles", g.cities[&cid].name;
            "gold-buys-the-backlog-builder: {held} Builders held or training against {quota} wanted; this city works {own} unimproved tiles; {:.0} Gold of a {:.0} budget",
            price as f64 / 100.0, budget);
        true
    }
}

#[cfg(test)]
mod tests;
