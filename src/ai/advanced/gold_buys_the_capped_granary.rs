//! `gold-buys-the-capped-granary` (opt-in): in the purchase phase, the
//! treasury buys the Granary of a city that stands within one of its housing
//! and has none, above the purchase upkeep floor.
//!
//! **Why (live Deity Online Korea, 2026-11-04, 4-9 games, turns 30-100).**
//! - Housing, not food, caps our population: 59% of our city-turns stood
//!   within two of their housing (the host's growth penalty), and only 15%
//!   grew slowly with housing to spare. The whole Lv 8 science gap is
//!   population (41 against the leader's 75 at turn 100; science per citizen
//!   at parity once the Deity bonus is taken out).
//! - In those capped cities (median five citizens) housing came from water
//!   3.8, buildings 1.4, improvements 0.6 and districts 0; 39% had no
//!   Granary although Pottery lands by turn 12-20. Their queues held Walls
//!   (31% of those turns), the Granary itself at two or three Production
//!   (29%), an Archer, a district, a Settler.
//! - The Granary is also the gate of `engineering-for-capped-fresh-water`
//!   (capped cities "with their Granary"), and the one Deity game that passed
//!   the turn-100 production gate researched Engineering at turn 57 with four
//!   Aqueducts by turn 80, against turns 72-74 and none or one in the games
//!   retired there.
//! - The treasury is idle: a median 83 / 127 / 145 Gold at turns 40-59 /
//!   60-79 / 80-99 against a 130-Gold Granary, affordable on 20% of the
//!   capped Granary-less city-turns. `buy_gold_infrastructure` already prices
//!   housing need, but it spends only above `spend_gold`'s 100 + 25 a city
//!   reserve; this buys above the upkeep floor `gold-buys-the-backlog-builder`
//!   uses. Walls are not displaced (a war-paced paired read without the walls
//!   genes lost +0.69 cities a game).
//!
//! **The rule.** After `gold-buys-the-backlog-builder`, while no city is
//! threatened and no city-saving purchase took the turn: among our cities
//! that housing really blocks -- within [`CAPPED_GRANARY_HEADROOM`] of their
//! housing with Food left over after their citizens eat -- with no Granary
//! (or the civilization's replacement), not attacked in four turns, and not
//! already building one well along ([`CAPPED_GRANARY_FINISH_TURNS`] turns
//! or [`CAPPED_GRANARY_INVESTED_SHARE`] of its cost invested), buy the
//! Granary of the most capped, then the weakest producer, then the cheapest,
//! when the Gold above the upkeep floor covers it -- and, while at war with a
//! major, the price of an emergency defender besides
//! (`emergency_defender_price`), so a Granary never spends a city-saving
//! purchase. One a turn. Off, nothing runs. (-af's caution from
//! `gold-buys-the-production-tile`, which spent above the upkeep floor in the
//! opening and read cities at t150 -0.63, z -2.1: judge this on cities lost
//! and cities at t150 at war pace, not on population alone.)

use super::*;

/// A city whose population stands at least this close to its housing is
/// capped: the host's growth penalty starts within one.
pub(super) const CAPPED_GRANARY_HEADROOM: f64 = 1.0;

/// A queued Granary this close to done finishes on its own.
pub(super) const CAPPED_GRANARY_FINISH_TURNS: f64 = 2.0;

/// A queued Granary with this share of its cost invested finishes on its own
/// rather than have its Production paid for twice.
pub(super) const CAPPED_GRANARY_INVESTED_SHARE: f64 = 0.3;

/// A city attacked within this many turns is not where the Granary is bought.
const CAPPED_GRANARY_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The Granary `cid` would buy -- the base building or the
    /// civilization's replacement -- if it holds neither yet.
    fn capped_granary_item(g: &Game, pid: usize, cid: u32) -> Option<Name> {
        let granary = crate::name!("granary");
        let city = &g.cities[&cid];
        let family = |building: &Name| {
            *building == granary
                || g.rules
                    .buildings
                    .get(building)
                    .is_some_and(|spec| spec.replaces == Some(granary))
        };
        if city.buildings.iter().any(family) {
            return None;
        }
        g.rules
            .buildings
            .keys()
            .copied()
            .filter(family)
            .find(|building| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::Building {
                        building: *building,
                    },
                )
            })
    }

    /// See the module. Exact no-op with the gene off.
    pub(in crate::ai::advanced) fn capped_granary_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.gold_buys_the_capped_granary
            || self.base.minor
            || self.base.barb
            || plan.threatened_city.is_some()
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
        // At war with a major, an emergency defender's price stays banked.
        let war_reserve = if at_major_war {
            self.emergency_defender_price(g, pid).unwrap_or(0.0)
        } else {
            0.0
        };
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0) - war_reserve;
        if budget <= 0.0 {
            return false;
        }
        // Most over its housing first, then the weakest producer, then the
        // cheapest price, then the lowest id.
        let mut best: Option<(i64, i64, i64, u32, Name, f64, f64)> = None;
        {
            let _memo = g.query_memo();
            for cid in g.player_city_ids(pid) {
                let city = &g.cities[&cid];
                if city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= CAPPED_GRANARY_ATTACK_MEMORY
                {
                    continue;
                }
                let housing = g.city_housing(city);
                let over = f64::from(city.pop) - housing;
                // Housing blocks only a city that would otherwise grow.
                let fed = g.city_yields(cid).food - 2.0 * f64::from(city.pop) > 0.0;
                if over + CAPPED_GRANARY_HEADROOM < 0.0 || !fed {
                    continue;
                }
                let Some(building) = Self::capped_granary_item(g, pid, cid) else {
                    continue;
                };
                let item = Item::Building { building };
                if g.purchase_is_blocked(cid, &item) {
                    continue;
                }
                let invested = g.item_invested_production(cid, &item);
                if invested >= g.item_cost_for_city(pid, cid, &item) * CAPPED_GRANARY_INVESTED_SHARE
                    || (city.queue.first() == Some(&item)
                        && self.production_build_turns(g, pid, cid, &item)
                            <= CAPPED_GRANARY_FINISH_TURNS)
                {
                    continue;
                }
                let Some(price) = g.building_purchase_cost(pid, cid, building.as_str(), "gold")
                else {
                    continue;
                };
                if price > budget + 1e-9 {
                    continue;
                }
                let production = g.city_yields(cid).production;
                let key = (
                    -(over * 100.0).round() as i64,
                    (production * 100.0).round() as i64,
                    (price * 100.0).round() as i64,
                    cid,
                    building,
                    over,
                    price,
                );
                if best
                    .as_ref()
                    .is_none_or(|old| (key.0, key.1, key.2, key.3) < (old.0, old.1, old.2, old.3))
                {
                    best = Some(key);
                }
            }
        }
        let Some((_, _, _, cid, building, over, price)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::BuyBuilding {
                city: cid,
                building,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Economy, Decision,
            "{} buys its Granary at its housing cap", g.cities[&cid].name;
            "gold-buys-the-capped-granary: population {:+.1} against housing; {price:.0} Gold of a {budget:.0} budget{}",
            over,
            if at_major_war { format!(" ({war_reserve:.0} held for a defender at war)") } else { String::new() });
        true
    }
}

#[cfg(test)]
mod tests;
