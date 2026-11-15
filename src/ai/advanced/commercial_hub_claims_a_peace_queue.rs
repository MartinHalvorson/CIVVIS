//! `commercial-hub-claims-a-peace-queue`: at peace, a Domination seat's
//! Commercial Hub city builds its Market, and the empire's first Commercial
//! Hub is claimed, ahead of a routine queue.
//!
//! **Why (live Deity Gran Colombia, 4-player Tiny Pangaea, Online: the 26
//! games of 2026-11-11 19:57Z-23:50Z and the two of 2026-11-12 after
//! 00:57Z).** At t100 the seat nets 11 Gold a turn against the best rival's
//! 89 (49 without the Deity AI's +80%). Its cities gross 32.5: worked tiles
//! 21.4, the Palace 5.3, trade routes 4.0, districts 0.4. Unit upkeep takes
//! 17-18 of that, and it pays for current-era soldiers (Crossbowmen,
//! Men-at-Arms, Trebuchets, Horsemen), not idle relics, so it is the army's
//! price rather than waste. The income side is where the gap is:
//!
//! - **No Commercial Hub.** A median 0 at t100 (18 of 21 games), none by the
//!   game's end in 14 of 28, the first one finished at a median t119.5.
//!   Currency comes at a median t55.
//! - **No Market, so no trade capacity.** In this ruleset (and live: the
//!   capacity of `civvis-20261111T212532Z` stayed at 1 with three Hubs at
//!   t85 and rose to 2 with its first Market at t88) a route slot comes with
//!   the Market, not the Hub. Capacity was 1 in 11 of 21 games at t100 and 2
//!   in 9. That game's Gold went from 29 to 54 a turn when its Market and
//!   second route landed.
//! - **The Hub step only fills an idle queue.** `commercial-hub-and-traders`
//!   and `commercial-hub-in-the-strategic-queue` (both armed) ask the Hub
//!   and its Market only for a queue nothing else claimed, and a live queue
//!   is almost never idle. In t60-100, a city that could have started a Hub
//!   and held none spent 137 city-turns a game on something else: 72.5 at
//!   peace, 22.9 at war without `threatened-war-holds-the-claims`, and 41.8
//!   under that hold (which this gene leaves alone). On the peace and
//!   no-hold turns those cities built soldiers (26), buildings (21),
//!   Settlers (13), other districts (11), Builders (11) and Campuses (10.5),
//!   and on 82 of the 95 the empire held no Hub at all.
//!
//! **The gene.** Each turn, while the active victory target is Domination or
//! Score ([`AdvancedAi::conquest_or_score_seat`]: a Hub is two score and its
//! Market one),
//! `commercial-hub-in-the-strategic-queue` is on, its window is open
//! (Currency, Foreign Trade, Online t60), and we are at war with no major:
//!
//! 1. a city holding a Commercial Hub without its Market starts the Market
//!    (a trade route slot, Gold and a citizen slot), and
//! 2. while the empire holds or has queued fewer than [`PEACE_HUB_CLAIMS`]
//!    Hubs, the Hub step's own pick (`BasicAi::commercial_hub_item`: one of
//!    its best unthreatened producers, best Gold site, done within 20 turns)
//!    starts its Hub.
//!
//! The item goes ahead of the queue's head only when that head may wait: a
//! soldier, a Builder or Scout, or a building that is not a wonder, Walls, a
//! faith building, a Campus building or a housing building, and only when the
//! head does not finish this turn. A Settler, a district, a wonder, a project
//! and a Trader keep their queue, and so does a threatened city or one
//! attacked within four turns. The strategic queue's review then keeps the
//! claimed Hub or Market (`commercial_hub_build_holds`); the Trader for the
//! new slot comes from `trader-fills-the-idle-route` and
//! `gold-buys-the-idle-trader`. One Hub by claim keeps the cost to about 170
//! Production (Hub and Market) in one city; further Hubs stay with the idle
//! step. Off, nothing runs. Grep the journal for
//! `commercial-hub-claims-a-peace-queue:`.

use super::*;

/// Commercial Hubs (standing, founded or queued) below which the claim opens
/// one. Further Hubs are left to the step's idle queue, so the claim's draw
/// on the army's Production stays one city's Hub and Market.
pub(crate) const PEACE_HUB_CLAIMS: usize = 1;

/// Turns after a hit on a city during which it keeps its queue: the
/// production scorer's four-turn `threatened` window.
const PEACE_HUB_ATTACK_MEMORY: u32 = 4;

/// Unimproved worked tiles past the Builder charges in hand at which a queued
/// Builder is left alone (the same test the worked-backlog Builder gene uses).
const PEACE_HUB_BUILDER_BACKLOG: i64 = 2;

/// Buildings that keep their queue: housing and the faith defence's.
const PEACE_HUB_KEPT_BUILDINGS: [&str; 8] = [
    "walls",
    "medieval_walls",
    "renaissance_walls",
    "granary",
    "water_mill",
    "sewer",
    "shrine",
    "temple",
];

impl AdvancedAi {
    /// Whether the queue head `head` of `cid` may wait for the Market or the
    /// Hub: a soldier, a Builder or Scout, or a building that is not a wonder,
    /// a kept building or a Campus building; never anything that finishes
    /// this turn.
    pub(super) fn peace_hub_may_displace(g: &Game, pid: usize, cid: u32, head: &Item) -> bool {
        let remaining =
            g.item_cost_for_city(pid, cid, head) - g.item_invested_production(cid, head);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match head {
            Item::Unit { unit } => {
                // A Builder the worked backlog is waiting on keeps its queue:
                // `the-worked-backlog-gets-its-builder-first` put it there, and
                // taking the head back would swap the two every turn.
                let backlog_builder = *unit == "builder"
                    && BasicAi::unimproved_worked_tiles(g, pid) as i64
                        - i64::from(BasicAi::builder_charge_cover(g, pid))
                        >= PEACE_HUB_BUILDER_BACKLOG;
                *unit != "settler"
                    && !backlog_builder
                    && (matches!(unit.as_str(), "builder" | "scout")
                        || g.rules
                            .units
                            .get(unit)
                            .is_some_and(|spec| spec.class == "military"))
            }
            Item::Building { building } => {
                let campus = crate::name!("campus");
                !PEACE_HUB_KEPT_BUILDINGS.contains(&building.as_str())
                    && g.rules.buildings.get(building).is_some_and(|spec| {
                        !spec.wonder
                            && spec.yields.science <= 0.0
                            && spec.district.map(|d| g.district_family(d)) != Some(campus)
                    })
            }
            _ => false,
        }
    }

    /// Whether `item` may be claimed in `cid`: not already queued, the city
    /// can produce it, and its head is empty or may wait.
    fn peace_hub_claimable(g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        let city = &g.cities[&cid];
        !city.queue.contains(item)
            && g.can_produce(pid, cid, item)
            && city
                .queue
                .first()
                .is_none_or(|head| Self::peace_hub_may_displace(g, pid, cid, head))
    }

    /// The claim this turn, if any: a standing Hub's Market first, else the
    /// empire's first Hub. `(city, item, is_market)`.
    fn peace_hub_claim(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item, bool)> {
        let city_ids = g.player_city_ids(pid);
        let calm = |cid: u32| {
            let city = &g.cities[&cid];
            plan.threatened_city != Some(cid)
                && !(city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= PEACE_HUB_ATTACK_MEMORY)
        };
        let market = city_ids
            .iter()
            .copied()
            .filter(|cid| calm(*cid))
            .find_map(|cid| {
                BasicAi::commercial_market_item(g, pid, cid)
                    .filter(|item| Self::peace_hub_claimable(g, pid, cid, item))
                    .map(|item| (cid, item, true))
            });
        if market.is_some() {
            return market;
        }
        let held = city_ids
            .iter()
            .filter(|cid| BasicAi::commercial_hub_held_or_queued(g, **cid))
            .count();
        if held >= PEACE_HUB_CLAIMS {
            return None;
        }
        let n_cities = city_ids.len();
        city_ids
            .iter()
            .copied()
            .filter(|cid| calm(*cid))
            .find_map(|cid| {
                self.base
                    .commercial_hub_item(g, pid, cid, n_cities)
                    .filter(|item| Self::peace_hub_claimable(g, pid, cid, item))
                    .map(|item| (cid, item, false))
            })
    }

    /// See the module: at peace, a Hub's Market, else the empire's first Hub,
    /// goes ahead of a routine queue. Exact no-op with the gene off.
    pub(super) fn claim_peace_queue_for_the_commercial_hub(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.commercial_hub_claims_a_peace_queue
            || !self.commercial_hub_in_the_strategic_queue
            // A Score seat too: see [`AdvancedAi::conquest_or_score_seat`].
            || !self.conquest_or_score_seat(g)
            || !self.base.commercial_hub_window_open(g)
            || g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_barbarian
                    && !other.is_minor
                    && g.is_at_war(pid, other.id)
            })
        {
            return;
        }
        // `commercial_hub_item` reads the plan's threatened city from the
        // governor, as `delegated_cities` lends it.
        let restore = self.base.plan_threatened_city;
        self.base.plan_threatened_city = plan.threatened_city;
        let claim = self.peace_hub_claim(g, pid, plan);
        self.base.plan_threatened_city = restore;
        let Some((city, item, is_market)) = claim else {
            return;
        };
        let displaced = g.cities[&city]
            .queue
            .first()
            .map(Self::plain_item)
            .unwrap_or_else(|| "an idle queue".to_string());
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            let name = g.cities[&city].name.clone();
            if is_market {
                think!(self.journal(), Economy, Decision,
                       "{name} builds its Commercial Hub's Market at peace";
                       "commercial-hub-claims-a-peace-queue: the Market goes ahead of {displaced} \
                        (it keeps its progress): a trade route slot and its Gold");
            } else {
                think!(self.journal(), Economy, Decision,
                       "{name} starts the empire's first Commercial Hub at peace";
                       "commercial-hub-claims-a-peace-queue: the Hub goes ahead of {displaced} \
                        (it keeps its progress); its Market opens a trade route slot");
            }
        }
    }
}

#[cfg(test)]
mod tests;
