//! `commercial-hub-and-traders`: from the middle game the delegated governor
//! raises a Commercial Hub in two or three of its best unthreatened cities,
//! then each hub's Market, and trains a Trader for every open route slot,
//! ahead of the military floor. See `BasicAi::commercial_hub_and_traders`.
//!
//! Live Emperor G185-G198 (civvis-20261006T024058Z..T053813Z, 14 runs, 4p
//! Tiny Pangaea, Gran Colombia): Gold a turn against the best rival fell from
//! 0.55x at t50 to 0.15x at t100 and 0.11x at t125 (net 14 against 63 at
//! t100) while unit upkeep rose 20 -> 42 and ate 45-65% of the cities' gross
//! Gold. All 13 games reaching t100 held no Commercial Hub and no Market
//! (0.9 hubs at t125, 1.6 at t150), and trade route capacity stood at 1-2
//! until t150. The delegated governor's district list reaches the hub only
//! after the Campus in a city with a free slot, and a city reaches the list
//! only once the military floor is met; at Emperor the AIs gold-buy walls
//! and units the turn they are threatened, and our own purchases are
//! Gold-limited.
use super::BasicAi;
use crate::game::{Game, Item};
use crate::think;

/// Standard turns before the step opens: turn 60 at the live seat's Online
/// speed (66% of Standard), once Currency and Foreign Trade are in hand.
pub(crate) const COMMERCIAL_HUB_FROM_STANDARD_TURN: u32 = 90;

/// The most Commercial Hubs (standing, founded or queued) the step raises.
pub(crate) const COMMERCIAL_HUB_MAX: usize = 3;

/// Cities from which the step wants its third hub; two before that.
const COMMERCIAL_HUB_THIRD_AT_CITIES: usize = 6;

/// The slowest a city may build its hub or the hub's Market.
const COMMERCIAL_HUB_MAX_TURNS: f64 = 20.0;

/// The slowest a city may train the Trader for an open route slot, in
/// standard turns.
const COMMERCIAL_TRADER_MAX_STANDARD_TURNS: u32 = 10;

impl BasicAi {
    /// Whether the step's window is open: the gene, a major's governor, and
    /// turn `COMMERCIAL_HUB_FROM_STANDARD_TURN` (standard) reached.
    pub(super) fn commercial_hub_window_open(&self, g: &Game) -> bool {
        self.commercial_hub_and_traders
            && !self.minor
            && !self.barb
            && g.turn >= g.standard_duration(COMMERCIAL_HUB_FROM_STANDARD_TURN)
    }

    /// The plan's threatened city (lent by `AdvancedAi::delegated_cities`)
    /// or a city attacked within four turns: the test the housing step uses.
    fn commercial_city_threatened(&self, g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        self.plan_threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
    }

    /// `commercial-hub-and-traders`: this city's next economy build, in
    /// order: the Market of the Commercial Hub it holds; a Trader while the
    /// empire's routes and Traders leave a route slot open and this city can
    /// start a route; its own Commercial Hub when it is one of the empire's
    /// best unthreatened cities still without one (`commercial_hub_item`).
    /// `None` from a threatened city, a city due its Granary (population
    /// within one of its housing with a Granary buildable), and below three
    /// cities. The caller checks the window, the floor's emergency and a due
    /// Settler.
    pub(super) fn commercial_hub_step(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        traders: usize,
    ) -> Option<Item> {
        if n_cities < 3 || self.commercial_city_threatened(g, cid) {
            return None;
        }
        let city = &g.cities[&cid];
        if (city.pop as f64) + 1.0 >= g.city_housing(city)
            && Self::civ_building(g, pid, cid, "granary").is_some()
        {
            return None;
        }
        if let Some(market) = Self::commercial_market_item(g, pid, cid) {
            think!(self.journal, Cities, Detail,
                   "{} builds its Commercial Hub's Market", city.name;
                   "the hub stands without its Market: Gold and a trade route slot");
            return Some(market);
        }
        if let Some(trader) = self.commercial_trader_item(g, pid, cid, traders) {
            think!(self.journal, Cities, Detail,
                   "{} trains a Trader for an open route slot", city.name;
                   "{} routes and {traders} Traders against a capacity of {}",
                   g.active_routes(pid), g.trade_capacity(pid));
            return Some(trader);
        }
        let hub = self.commercial_hub_item(g, pid, cid, n_cities)?;
        think!(self.journal, Cities, Detail,
               "{} opens a Commercial Hub", city.name;
               "one of the empire's {} best unthreatened cities without one",
               Self::commercial_hubs_wanted(n_cities));
        Some(hub)
    }

    /// The Market (or this civilization's replacement) of the Commercial Hub
    /// standing in `cid`, when the city finishes it within
    /// `COMMERCIAL_HUB_MAX_TURNS`.
    fn commercial_market_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        if !g.city_has_district_family(city, crate::name!("commercial_hub")) {
            return None;
        }
        let market = Self::civ_building(g, pid, cid, "market")?;
        (Self::commercial_build_turns(g, pid, cid, &market) <= COMMERCIAL_HUB_MAX_TURNS)
            .then_some(market)
    }

    /// A Trader while the empire's routes and Traders (queued included) leave
    /// a route slot open, from a city that can start a route safely, when it
    /// trains one within `COMMERCIAL_TRADER_MAX_STANDARD_TURNS`.
    fn commercial_trader_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        traders: usize,
    ) -> Option<Item> {
        if !self.should_add_trader_in_city_for_controller(g, pid, cid, traders)
            || !self.safe_trade_origin_for_controller(g, pid, cid)
        {
            return None;
        }
        let trader = Item::Unit {
            unit: crate::name!("trader"),
        };
        (g.can_produce(pid, cid, &trader)
            && Self::unit_build_turns(g, pid, cid, "trader")
                <= g.standard_duration(COMMERCIAL_TRADER_MAX_STANDARD_TURNS) as f64)
            .then_some(trader)
    }

    /// Hubs the step wants standing, founded or queued: two, three from
    /// `COMMERCIAL_HUB_THIRD_AT_CITIES` cities, never above
    /// `COMMERCIAL_HUB_MAX`.
    fn commercial_hubs_wanted(n_cities: usize) -> usize {
        if n_cities >= COMMERCIAL_HUB_THIRD_AT_CITIES {
            COMMERCIAL_HUB_MAX
        } else {
            2
        }
    }

    /// Whether `cid` holds a Commercial Hub, its foundation, or has one first
    /// in its queue.
    fn commercial_hub_held_or_queued(g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        g.city_has_district_family(city, crate::name!("commercial_hub"))
            || city.owned_tiles.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| {
                        g.district_family(foundation.district) == "commercial_hub"
                    })
            })
            || matches!(
                city.queue.first(),
                Some(Item::District { district, .. })
                    if g.district_family(*district) == "commercial_hub"
            )
    }

    /// Whether a city of `pid` holds a Commercial Hub, its foundation, or has
    /// one first in its queue. Read by `campus-buildings-first`.
    pub(super) fn empire_holds_a_commercial_hub(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .any(|city| Self::commercial_hub_held_or_queued(g, city))
    }

    /// See `commercial_hub_step`: this city's Commercial Hub on its best Gold
    /// site (river, Harbor and district adjacency as `district_yields` prices
    /// them), when the empire holds or has queued fewer hubs than
    /// `commercial_hubs_wanted` and this city is among the most productive
    /// of the unthreatened cities that lack one and have a site -- as many
    /// as the hubs still wanted -- and finishes it within
    /// `COMMERCIAL_HUB_MAX_TURNS`.
    fn commercial_hub_item(&self, g: &Game, pid: usize, cid: u32, n_cities: usize) -> Option<Item> {
        let hub = Self::civ_district(g, pid, "commercial_hub");
        let spec = g.rules.districts.get(&hub)?;
        let unlocked = spec
            .tech
            .as_ref()
            .is_none_or(|tech| g.players[pid].techs.contains(tech))
            && spec
                .civic
                .as_ref()
                .is_none_or(|civic| g.players[pid].civics.contains(civic));
        if !unlocked || Self::commercial_hub_held_or_queued(g, cid) {
            return None;
        }
        let own = g.player_city_ids(pid);
        let held = own
            .iter()
            .filter(|city| Self::commercial_hub_held_or_queued(g, **city))
            .count();
        let open = Self::commercial_hubs_wanted(n_cities).saturating_sub(held);
        if open == 0 {
            return None;
        }
        let best_site = |city: u32| {
            g.district_sites(city, hub)
                .into_iter()
                .map(|pos| (pos, g.district_yields(hub, pos)))
                .max_by(|a, b| {
                    a.1.gold
                        .total_cmp(&b.1.gold)
                        .then(a.1.total().total_cmp(&b.1.total()))
                        .then(b.0.cmp(&a.0))
                })
                .map(|(pos, _)| pos)
        };
        let mut ranked: Vec<(f64, u32)> = own
            .iter()
            .copied()
            .filter(|city| {
                !Self::commercial_hub_held_or_queued(g, *city)
                    && !self.commercial_city_threatened(g, *city)
            })
            .filter(|city| best_site(*city).is_some())
            .map(|city| (g.city_yields(city).production, city))
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        if !ranked.iter().take(open).any(|(_, city)| *city == cid) {
            return None;
        }
        let item = Item::District {
            district: hub,
            pos: best_site(cid)?,
        };
        (g.can_produce(pid, cid, &item)
            && Self::commercial_build_turns(g, pid, cid, &item) <= COMMERCIAL_HUB_MAX_TURNS)
            .then_some(item)
    }

    /// Turns `cid` needs for `item`: the host's figure where it gave one.
    fn commercial_build_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        g.host_production_turns(cid, item)
            .unwrap_or_else(|| g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5))
    }
}

#[cfg(test)]
pub(crate) mod tests;
