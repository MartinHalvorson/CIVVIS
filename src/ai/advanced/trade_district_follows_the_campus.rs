//! `trade-district-follows-the-campus`: on a Science or Diplomatic seat at
//! peace, a calm city whose Campus and Library stand claims a Commercial Hub
//! (or a Harbor where that site pays more Gold) for its next district, and a
//! city whose Commercial Hub or Harbor stands without a Market or Lighthouse
//! claims that building, ahead of a queue that may wait.
//!
//! ## Why (live Deity, 6-player Continents, Online, 250 turns)
//!
//! Trade route capacity is one from Foreign Trade plus one for each city that
//! holds a Market or a Lighthouse (`Game::trade_capacity`). The live ladder's
//! last 329 Emperor, Immortal and Deity games held a capacity of 1 at turn 50
//! in all 329, a median 1 at turn 100 (204 of 289 games) and a median 2 at
//! turn 150. Four Deity games of 2026-11-18 (Kongo `civvis-20261118T045850Z`
//! on the Science lane; Canada `T051842Z`, Sumeria `T053007Z` and Hungary
//! `T061033Z`, the day's one win, on the Diplomatic lane), read from the
//! state rows and `why.log`:
//!
//! - **The trade district comes last or never.** Currency landed at turns
//!   44-57 and the first Commercial Hub or Harbor was placed at turns 65, 68,
//!   94 and 110. Of 42 cities the host offered one, 22 never placed it (12 of
//!   those were founded at turn 130 or later) and the 20 that did waited a
//!   median 54 turns. Over those 2,716 city-turns the cities trained soldiers
//!   and ships (20%), built Campus buildings (11%), Builders (11%), the
//!   Campus (10%), Settlers (7%), a Granary or other housing (7%), Walls (6%)
//!   and an Aqueduct, Industrial Zone or Theater Square (9%). A slot was not
//!   the limit: 17 of 28 cities held a free specialty slot at turn 100. The
//!   Hub's own step (`commercial-hub-and-traders`) asks only an idle queue
//!   and stops at three Hubs, and `commercial-hub-claims-a-peace-queue`
//!   serves the Domination and Score seats alone.
//! - **A standing district waits for its building.** Of the 21 cities that
//!   finished a Commercial Hub or Harbor, 10 never built the Market or
//!   Lighthouse (Mbanza Kongo's Harbor stood 94 turns from turn 95, Ur's 72
//!   from turn 122) and the other 11 took a median 9 turns and up to 67
//!   (Kingston 41, behind a 15-turn University and three soldiers; the
//!   winner's first Hub 43). Those 596 city-turns went to soldiers (24%),
//!   Campus buildings (22%), a new Campus (12%) and other buildings (12%).
//!   No claim names the Lighthouse.
//! - **What a route paid.** An international route paid 3 Gold at turns
//!   30-50 and 3-6 at turns 75-100; over the 586 international route-turns
//!   the four games ran (most after turn 100) it paid 6.3-10.2 Gold a turn
//!   with about one each of Production, Science and Culture. A domestic route
//!   paid 2-4 Food and 1-3 Production. Capacity went 1 to 3 at turns 145-149
//!   in two of the losses and net Gold from 4-10 a turn to 26-29 by turn 150.
//! - **The winner was not trade-rich either.** Hungary held capacity 1 until
//!   turn 119 and 4 from turn 137, on 17 Gold a turn at turn 100 and 63 at
//!   turn 150. Early trade did not separate the win from the losses; the
//!   gene is an income repair, to be judged on what the paired games measure.
//!
//! ## What the gene does
//!
//! Every turn, while the seat's victory target is Science or Diplomacy and no
//! major is at war with us ([`AdvancedAi::trade_district_claims_open`]), each
//! city that is neither threatened nor attacked within
//! [`TRADE_CLAIM_ATTACK_MEMORY`] turns, is not the early Settler floor's and
//! is not due its Granary names one item ([`AdvancedAi::trade_claim_item`]):
//!
//! 1. the Market of its finished Commercial Hub, else the Lighthouse of its
//!    finished Harbor, while the city holds no building that carries a trade
//!    route; else
//! 2. while it holds a finished Campus with its Library and no Commercial Hub
//!    or Harbor (standing, founded or queued), the Commercial Hub on its best
//!    site, or the Harbor where the Harbor's site pays more Gold.
//!
//! Each must finish within [`TRADE_CLAIM_MAX_TURNS`]. Every such building is
//! claimed; one district is claimed a turn, the soonest done first. The item
//! goes to the head of an idle queue, or ahead of a head that may wait
//! ([`AdvancedAi::trade_claim_may_pass`]): the shared claim rule
//! (`capped_aqueduct_may_defer`: a soldier at peace, a Scout, a building
//! outside the defence, faith, housing, science and culture lists, a district
//! nothing is invested in), and besides it an Industrial Zone or Theater
//! Square nothing is invested in (the second slot is the trade district's)
//! and, for the Market or Lighthouse alone, a University, Research Lab or
//! Amphitheater. A Settler, a Builder, a Trader, Walls, a wonder, a project,
//! a Granary, a Library, a Monument, an Aqueduct, a Campus, the claimed
//! deterrent, Granary, Trader, charting hull, backlog Builder, Aqueduct and
//! Neighborhood, a wall breaker, and anything finishing this turn keep their
//! queue. The displaced item keeps its progress.
//!
//! The claim runs after the Campus claims. The Industrial Zone, Factory and
//! Campus building claims and the governor's rescoring leave the claimed
//! item alone ([`AdvancedAi::trade_district_claim_holds`]). The slot's
//! Trader comes from `trader-fills-the-idle-route` and
//! `gold-buys-the-idle-trader`. Off, nothing runs. Grep the journal for
//! `trade-district-follows-the-campus:`.
//!
//! ## Measured (2026-11-18)
//!
//! Science lane, 40 paired seeds (63000-63039: `gene_screen
//! --deployment-genome --live-genome`, the live Kongo genome with and without
//! the gene, Deity, six players, Online, 250 turns, the faithful rival
//! field). All 40 differ; no win in either arm. Trade capacity 1.27 to 2.55
//! at turn 100 (+1.27, SE 0.18) and 3.56 to 6.76 at turn 150 (+3.20, SE 0.42,
//! the 25 pairs that reached it); routes +1.12 (0.19) and +2.72 (0.55); net
//! Gold a turn +4.4 (2.7) and +10.1 (5.7); Science +1.7 (2.0) and +18.3
//! (8.3); Production +2.8 (3.0) and +23.2 (11.6); Culture at turn 100 -2.4
//! (1.1); final score +4.4 (9.2). More trade, and nothing measured down but
//! that Culture reading.
//!
//! The simulator cannot price the Gold a live route pays. Its seat runs
//! domestic routes (about 80% of them, where the live games ran about 78%
//! international) and its international routes pay 3.3-3.9 Gold against the
//! live 6-10 from turn 100. The Diplomatic lane was not simulated.
//!
//! On 76 live boards of the four games
//! (`docs/gene_screens/fires/trade-district-follows-the-campus.action-pairs.json`)
//! the gene changes the orders on 31 and journals 40 claims. 28 end the
//! frame as the city's order; `near-done-work-finishes` hands 9 back to a
//! head within two turns of done, and a later Caravel or Spy claim takes 3.
//!
//! ## `culture-trade-district-follows-the-theater`
//!
//! The same claim on a Culture seat, under its own opt-in flag: the Theater
//! Square and its Amphitheater stand in for the Campus and its Library
//! ([`CULTURE_TRADE_ANCHOR`]), and the Theater Square and every building
//! of it (Amphitheater, either Museum, Broadcast Center) keep their queue
//! besides the heads listed above, and an idle city that can build its next
//! Theater building leaves its queue to the routine pick.
//! `culture-theater-buildings-pass-the-claims`
//! runs later in the turn and puts a Theater building back ahead of a
//! claimed Market, Lighthouse, Hub or Harbor. Grep the journal for
//! `culture-trade-district-follows-the-theater:`.
//!
//! Why (live Deity, 6p Continents, Online, 2026-11-19): 47 of 51 Culture
//! seats held trade capacity 1 at turn 100; at turn 150 the median was 2 on
//! 39 Gold a turn, where the Science seats this module's first gene serves
//! held 2 at turn 100, 6 at turn 150 and 82 Gold a turn. Culture seats of
//! 8.5-10 cities and 6-8 Theater Squares split at turn 150 by capacity
//! 1 / 2 / 3+ into 5 / 9.5 / 18.5 great works, 31 / 41 / 80 tourism a turn
//! and 1 / 1 / 3 rivals reached by a route (each +25% tourism). The day's
//! one Culture win held capacity 4 on 124 Gold a turn at turn 150.

use super::*;

/// The district and building that must stand before a city claims its
/// Commercial Hub or Harbor on a Science or Diplomatic seat.
const SCIENCE_TRADE_ANCHOR: (&str, &str) = ("campus", "library");

/// The same pair on a Culture seat (`culture-trade-district-follows-the-theater`).
pub(super) const CULTURE_TRADE_ANCHOR: (&str, &str) = ("theater_square", "amphitheater");

/// The slowest a city may build the claimed district or building.
pub(crate) const TRADE_CLAIM_MAX_TURNS: f64 = 20.0;

/// Turns after a hit on a city during which it keeps its queue: the
/// production scorer's four-turn `threatened` window.
const TRADE_CLAIM_ATTACK_MEMORY: u32 = 4;

/// Buildings the Market or Lighthouse never passes: defence, faith, housing,
/// the Monument and the Library.
const TRADE_CLAIM_KEPT_BUILDINGS: [&str; 9] = [
    "walls",
    "medieval_walls",
    "renaissance_walls",
    "shrine",
    "temple",
    "granary",
    "water_mill",
    "monument",
    "library",
];

/// What a city claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TradeClaim {
    /// The Market or Lighthouse of a standing Commercial Hub or Harbor.
    Building,
    /// The Commercial Hub or Harbor itself.
    District,
}

impl AdvancedAi {
    /// The district and building whose pair opens a city's district claim on
    /// this seat: [`SCIENCE_TRADE_ANCHOR`] on a Science or Diplomatic seat
    /// with `trade-district-follows-the-campus`, [`CULTURE_TRADE_ANCHOR`] on
    /// a Culture seat with `culture-trade-district-follows-the-theater`.
    /// `None` on every other seat and with the seat's gene off.
    pub(super) fn trade_claim_anchor(&self, g: &Game) -> Option<(&'static str, &'static str)> {
        match self.active_victory_target(g) {
            Some(VictoryTarget::Science | VictoryTarget::Diplomacy)
                if self.trade_district_follows_the_campus =>
            {
                Some(SCIENCE_TRADE_ANCHOR)
            }
            Some(VictoryTarget::Culture) if self.culture_trade_district_follows_the_theater => {
                Some(CULTURE_TRADE_ANCHOR)
            }
            _ => None,
        }
    }

    /// Whether the claim runs this turn: a seat with an anchor
    /// ([`AdvancedAi::trade_claim_anchor`]) and no major at war with us.
    pub(super) fn trade_district_claims_open(&self, g: &Game, pid: usize) -> bool {
        (self.trade_district_follows_the_campus || self.culture_trade_district_follows_the_theater)
            && self.trade_claim_anchor(g).is_some()
            && !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_barbarian
                    && !other.is_minor
                    && g.is_at_war(pid, other.id)
            })
    }

    /// Whether `item` is a Commercial Hub or Harbor district.
    fn is_trade_district(g: &Game, item: &Item) -> bool {
        matches!(item, Item::District { district, .. } if {
            let family = g.district_family(*district);
            family == "commercial_hub" || family == "harbor"
        })
    }

    /// Whether `item` is a Theater Square or a building of one (the
    /// Amphitheater, either Museum, the Broadcast Center and their
    /// replacements): the heads a Culture seat's claim never passes.
    fn is_theater_item(g: &Game, item: &Item) -> bool {
        let theater = |district: Name| g.district_family(district) == "theater_square";
        match item {
            Item::District { district, .. } => theater(*district),
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .and_then(|spec| spec.district)
                .is_some_and(theater),
            _ => false,
        }
    }

    /// Turns `cid` needs for `item`: the host's figure where it gave one.
    fn trade_claim_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        g.host_production_turns(cid, item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
            remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
        })
    }

    /// The Market of the Commercial Hub `cid` holds, else the Lighthouse of
    /// its Harbor, while the city holds no building that carries a trade
    /// route. A civilization's replacement where it builds one.
    fn trade_claim_building(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        if city.buildings.iter().any(|built| {
            g.rules
                .buildings
                .get(built)
                .is_some_and(|spec| spec.trade_route_capacity > 0)
        }) {
            return None;
        }
        [("commercial_hub", "market"), ("harbor", "lighthouse")]
            .into_iter()
            .filter(|(district, _)| g.city_has_district_family(city, Name::new(district)))
            .find_map(|(_, building)| BasicAi::civ_building(g, pid, cid, building))
    }

    /// The Commercial Hub or Harbor `cid` would start: the one already in its
    /// queue, else, for a city with the seat's `anchor` district and building
    /// (a Campus and its Library, or a Theater Square and its Amphitheater)
    /// and neither trade district standing or founded, the Hub on its best
    /// site or the Harbor where the Harbor's site pays more Gold.
    fn trade_claim_district(g: &Game, pid: usize, cid: u32, anchor: (&str, &str)) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        if let Some(queued) = city
            .queue
            .iter()
            .find(|item| Self::is_trade_district(g, item))
        {
            return g.can_produce(pid, cid, queued).then(|| queued.clone());
        }
        let held = |family: &str| {
            let family = Name::new(family);
            g.city_has_district_family(city, family)
                || city.owned_tiles.iter().any(|pos| {
                    g.map
                        .get(*pos)
                        .and_then(|tile| tile.district_foundation.as_ref())
                        .is_some_and(|foundation| g.district_family(foundation.district) == family)
                })
        };
        if held("commercial_hub")
            || held("harbor")
            || !g.city_has_district_family(city, Name::new(anchor.0))
            || BasicAi::civ_building(g, pid, cid, anchor.1).is_some()
        {
            return None;
        }
        let offer = |family: &str| {
            let item = BasicAi::first_district_item_with_budget(
                g,
                pid,
                cid,
                family,
                TRADE_CLAIM_MAX_TURNS,
            )?;
            let Item::District { district, pos } = &item else {
                return None;
            };
            let gold = g.district_site_yields(*district, *pos).gold;
            Some((item, gold))
        };
        match (offer("commercial_hub"), offer("harbor")) {
            (Some((_, hub_gold)), Some((harbor, harbor_gold))) if harbor_gold > hub_gold => {
                Some(harbor)
            }
            (Some((hub, _)), _) => Some(hub),
            (None, harbor) => harbor.map(|(item, _)| item),
        }
    }

    /// What `cid` claims, which kind it is and how long it takes: see the
    /// module. `None` with the gene off, on another seat, at war with a
    /// major, and in a threatened or freshly attacked city.
    pub(super) fn trade_claim_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, TradeClaim, f64)> {
        if !self.trade_district_claims_open(g, pid) {
            return None;
        }
        let anchor = self.trade_claim_anchor(g)?;
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= TRADE_CLAIM_ATTACK_MEMORY)
        {
            return None;
        }
        let (item, kind) = match Self::trade_claim_building(g, pid, cid) {
            Some(building) => (building, TradeClaim::Building),
            None => (
                Self::trade_claim_district(g, pid, cid, anchor)?,
                TradeClaim::District,
            ),
        };
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let turns = Self::trade_claim_turns(g, pid, cid, &item);
        (turns <= TRADE_CLAIM_MAX_TURNS).then_some((item, kind, turns))
    }

    /// Whether `item` at the head of `cid` is the district or building this
    /// gene claims there: the Industrial Zone, Factory and Campus building
    /// claims and the governor's rescoring leave it alone. Always false with
    /// the gene off.
    pub(super) fn trade_district_claim_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        (self.trade_district_follows_the_campus || self.culture_trade_district_follows_the_theater)
            && self
                .trade_claim_item(g, pid, cid, plan)
                .is_some_and(|(claimed, _, _)| claimed == *item)
    }

    /// Whether the queue head `head` of `cid` may wait for the claim: see
    /// the module.
    pub(super) fn trade_claim_may_pass(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        head: &Item,
        kind: TradeClaim,
    ) -> bool {
        if self.deterrence_unit_holds(g, pid, head)
            || self.bound_granary_holds(g, pid, cid, plan, head)
            || self.idle_route_trader_holds(g, pid, cid, plan, head)
            || self.charting_hull_holds(g, pid, cid, plan, head)
            || self.backlog_builder_holds(g, pid, cid, plan, head)
            || self.capped_aqueduct_holds(g, pid, cid, plan, head)
            || self.capped_neighborhood_holds(g, pid, cid, plan, head)
            || Self::queued_wall_breaker(g, head)
            // `science-universities-pass-the-claims`' University. Always
            // false with that gene off.
            || self.science_university_holds(g, pid, cid, plan, head)
        {
            return false;
        }
        // `culture-trade-district-follows-the-theater`: a Culture seat's
        // Theater Square and its buildings keep their queue.
        if self.culture_trade_district_follows_the_theater
            && Self::is_theater_item(g, head)
            && self.trade_claim_anchor(g) == Some(CULTURE_TRADE_ANCHOR)
        {
            return false;
        }
        // The claim runs at peace with every major.
        if Self::capped_aqueduct_may_defer(g, pid, cid, head, false) {
            return true;
        }
        if Self::live_gp_cultural_building_commitment(g, pid, cid, head) {
            return false;
        }
        let remaining =
            g.item_cost_for_city(pid, cid, head) - g.item_invested_production(cid, head);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match head {
            // The second slot is the trade district's.
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                g.item_invested_production(cid, head) <= 0.0
                    && (family == "industrial_zone" || family == "theater_square")
            }
            // The Market or Lighthouse is a few turns' work.
            Item::Building { building } => {
                kind == TradeClaim::Building
                    && !TRADE_CLAIM_KEPT_BUILDINGS.contains(&building.as_str())
                    && g.rules
                        .buildings
                        .get(building)
                        .is_some_and(|spec| !spec.wonder)
            }
            _ => false,
        }
    }

    /// See the module: every calm city's Market or Lighthouse, and one
    /// Commercial Hub or Harbor a turn, go to the head of a queue that may
    /// wait. Exact no-op with the gene off.
    pub(super) fn claim_trade_district(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.trade_district_claims_open(g, pid) {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let settlers = self.counts(g, pid).settlers;
        let culture = self.trade_claim_anchor(g) == Some(CULTURE_TRADE_ANCHOR);
        let (tag, after) = if culture {
            (
                "culture-trade-district-follows-the-theater",
                "Theater Square",
            )
        } else {
            ("trade-district-follows-the-campus", "Campus")
        };
        let mut buildings: Vec<(u32, Item, f64)> = Vec::new();
        let mut district: Option<(f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((item, kind, turns)) = self.trade_claim_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            let city = &g.cities[&cid];
            // A city within one of its housing with a Granary still to build
            // leaves its queue to the Granary claim.
            if (city.pop as f64) + 1.0 >= g.city_housing(city)
                && BasicAi::civ_building(g, pid, cid, "granary").is_some()
            {
                continue;
            }
            match city.queue.first() {
                Some(head) => {
                    if *head == item || !self.trade_claim_may_pass(g, pid, cid, plan, head, kind) {
                        continue;
                    }
                }
                // An idle city with a Settler due leaves its queue to the
                // Settler step: the city count is what the gate reads.
                None => {
                    if self.base.settler_due(g, pid, cid, n_cities, settlers) {
                        continue;
                    }
                    // A Culture seat's idle city that can build its next
                    // Theater building leaves the queue to the routine
                    // pick: the claim never goes ahead of an Amphitheater or
                    // Museum.
                    if culture && self.culture_theater_building_next(g, pid, cid).is_some() {
                        continue;
                    }
                }
            }
            match kind {
                TradeClaim::Building => buildings.push((cid, item, turns)),
                TradeClaim::District => {
                    if district
                        .as_ref()
                        .is_none_or(|(old, old_city, _)| (turns, cid) < (*old, *old_city))
                    {
                        district = Some((turns, cid, item));
                    }
                }
            }
        }
        for (cid, item, turns) in buildings
            .into_iter()
            .chain(district.map(|(turns, cid, item)| (cid, item, turns)))
        {
            let head = g.cities[&cid].queue.first().cloned();
            let name = g.cities[&cid].name.clone();
            let label = Self::plain_item(&item);
            let is_district = matches!(item, Item::District { .. });
            if g.apply(pid, &Action::Produce { city: cid, item }).is_err() {
                continue;
            }
            let ahead = head.map_or_else(
                || "an idle queue".to_string(),
                |item| format!("{} (it keeps its progress)", Self::plain_item(&item)),
            );
            if is_district {
                think!(self.journal(), Cities, Decision,
                       "{name} claims its {label} after the {after}";
                       "{tag}: {turns:.1} turns, ahead of {ahead}; \
                        its Market or Lighthouse opens a trade route slot");
            } else {
                think!(self.journal(), Cities, Decision,
                       "{name} claims its {label} for a trade route slot";
                       "{tag}: {turns:.1} turns, ahead of {ahead}");
            }
        }
    }
}

#[cfg(test)]
mod tests;
