//! `culture-digs-every-site`: a Culture seat keeps an Archaeological Museum
//! for every three Antiquity Sites it can reach, puts an Archaeologist at the
//! head of a Museum city's queue while a slot and a site wait, and walks that
//! Archaeologist on to the next site when a step toward one fails.
//!
//! **Why (live Deity Kristina, 2026-11-10, six games read to t150 or later).**
//! No game dug an Artifact before t182, while the seat held a median four
//! Antiquity Sites in its own borders and one or two unowned ones.
//! - `culture-digs-one-museum` (armed) builds ONE Museum and stops: "every
//!   later Museum stays an Art Museum". 161648Z-cont2 owned five sites and
//!   two unowned ones at t150 and one Museum.
//! - The Museum stood in 144009Z by t110 and Natural History came at t127,
//!   but the capital built two Spies, a Trader, a Factory, a Government Plaza,
//!   its two tier buildings and two Settlers before the Archaeologist at
//!   t156-t161: the claims ahead of the delegated governor took every queue,
//!   and the scorer's Archaeologist (2,700 and up) never reached one. In
//!   161648Z-cont2 the first Archaeologist started at t159 and dug at t182.
//! - At t189-t193 of 161648Z-cont2 the Archaeologist stood beside an owned
//!   site and walked back and forth ("walks in circles"): `archaeologist_step`
//!   takes the nearest routable site and, when the step toward it fails,
//!   idles; it never tries the next site.
//! An Artifact pays Tourism and Culture from the turn it is dug to the end of
//! the game (3 each, doubled in a full Museum for Kristina).
//!
//! **What the gene does**, for a seat whose active victory target is Culture
//! (any plan, Recovery included):
//! - **Museums.** It wants one Archaeological Museum, built or queued, before
//!   Natural History reveals the sites (the `culture-digs-one-museum` rule,
//!   which this gene carries), and from Natural History one per
//!   [`SITES_PER_MUSEUM`] reachable sites (own, unowned, or open-border
//!   land; at most [`SITES_COUNTED`] counted), at least one and at most
//!   [`DIG_MUSEUM_CAP`]. While it owns fewer, the Museum keeps the
//!   `culture-digs-one-museum` bonus in the scorer and the delegated
//!   governor's tie-break.
//! - **The Archaeologist.** While a standing, unpillaged Archaeological Museum
//!   has an empty Artifact slot, a reachable site waits, and no Archaeologist
//!   is afield or queued elsewhere, the Museum city that trains one soonest
//!   (within [`ARCHAEOLOGIST_MAX_STANDARD`] standard turns) puts it at the
//!   head of its queue over a deferrable head. A threatened or freshly
//!   attacked city keeps its queue, and so does a military unit at war
//!   (`culture_theater_may_defer`). The claim runs even while
//!   `threatened-war-holds-the-claims` holds the economy's claims: the
//!   Archaeologist is one civilian, and the Recovery plan is the plan this
//!   seat plays from about t150. The governor's rescoring leaves the claimed
//!   unit alone ([`AdvancedAi::dig_archaeologist_holds`]). When the queue
//!   would take more than [`ARCHAEOLOGIST_BUY_AFTER_TURNS`] turns, the
//!   treasury buys it above the purchase floor.
//! - **The walk.** The Archaeologist digs where it stands, else walks to its
//!   own sites first, then unowned, then foreign ones, nearest first. When a
//!   step toward a site fails, that site is barred for
//!   [`DEAD_SITE_STANDARD_TURNS`] standard turns (keyed by tile, not by any
//!   live id) and the next site is tried in the same move.
//!
//! Off, or for another target, nothing changes.

use super::*;

/// At most this many Archaeological Museums.
pub(super) const DIG_MUSEUM_CAP: usize = 2;
/// The Artifact slots one Archaeological Museum holds.
pub(super) const SITES_PER_MUSEUM: usize = 3;
/// Reachable sites counted at most. The map generator buries about six
/// Antiquity Sites per civilization.
pub(super) const SITES_COUNTED: usize = 6;
/// The longest Archaeologist build the claim takes, in standard turns.
pub(super) const ARCHAEOLOGIST_MAX_STANDARD: u32 = 20;
/// Production turns above which the treasury buys the Archaeologist.
pub(super) const ARCHAEOLOGIST_BUY_AFTER_TURNS: f64 = 4.0;
/// Standard turns a site stays barred after a failed step toward it.
pub(super) const DEAD_SITE_STANDARD_TURNS: u32 = 8;

impl AdvancedAi {
    /// Whether the gene acts for this seat now: on, an active Culture target,
    /// and the lane still open.
    pub(super) fn dig_every_site_active(&self, g: &Game) -> bool {
        self.culture_digs_every_site
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// Antiquity Sites and Shipwrecks `pid` could dig now if a slot stood
    /// empty: visible, unlocked, and in its own, unowned or open-border land.
    pub(super) fn reachable_dig_sites(g: &Game, pid: usize) -> usize {
        g.excavation_sites_ignoring_slots(pid).len()
    }

    /// The Archaeological Museums this seat wants, built or queued. Zero with
    /// the gene off or for another target; see the module for the rule.
    pub(super) fn dig_museums_wanted(&self, g: &Game, pid: usize) -> usize {
        if !self.dig_every_site_active(g) {
            return 0;
        }
        if !g.players[pid]
            .civics
            .contains(&crate::name!("natural_history"))
        {
            return 1;
        }
        Self::reachable_dig_sites(g, pid)
            .min(SITES_COUNTED)
            .div_ceil(SITES_PER_MUSEUM)
            .clamp(1, DIG_MUSEUM_CAP)
    }

    /// Whether `cid` holds a standing, unpillaged Archaeological Museum.
    fn holds_dig_museum(g: &Game, cid: u32) -> bool {
        let dig = crate::name!("archaeological_museum");
        g.cities.get(&cid).is_some_and(|city| {
            city.buildings.iter().any(|built| {
                g.building_is_family(*built, dig) && !city.pillaged_buildings.contains(built)
            })
        })
    }

    /// Whether the empire has an empty Artifact slot, by the host's own list
    /// where the mirror carries it.
    fn artifact_slot_open(g: &Game, pid: usize) -> bool {
        g.can_house_additional_great_work(pid, "artifact")
            && g.players[pid]
                .live_open_great_work_slots
                .as_ref()
                .is_none_or(|slots| slots.contains("artifact"))
    }

    /// The Archaeologist `cid` would claim and its build turns. `None` with
    /// the gene off, for another target, or when the city, the slot, the
    /// sites or an existing digger say no; see the module.
    pub(super) fn dig_archaeologist_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.dig_every_site_active(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || !Self::holds_dig_museum(g, cid)
            || !Self::artifact_slot_open(g, pid)
            || g.excavation_sites(pid).is_empty()
        {
            return None;
        }
        let digger = |unit: &Name| *unit == "archaeologist";
        let afield = g
            .units
            .values()
            .any(|unit| unit.owner == pid && digger(&unit.kind));
        let queued_elsewhere = g.player_city_ids(pid).into_iter().any(|other| {
            other != cid
                && g.cities[&other]
                    .queue
                    .iter()
                    .any(|item| matches!(item, Item::Unit { unit } if digger(unit)))
        });
        if afield || queued_elsewhere {
            return None;
        }
        let item = Item::Unit {
            unit: crate::name!("archaeologist"),
        };
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_remaining_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
        });
        (turns.is_finite() && turns <= g.standard_duration(ARCHAEOLOGIST_MAX_STANDARD) as f64)
            .then_some((item, turns))
    }

    /// Whether `item` at the head of `cid` is the Archaeologist this gene
    /// claims and the city still wants it: the governor's rescoring leaves it
    /// alone. Always false with the gene off.
    pub(super) fn dig_archaeologist_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        matches!(item, Item::Unit { unit } if *unit == "archaeologist")
            && self
                .dig_archaeologist_item(g, pid, cid, plan)
                .is_some_and(|(digger, _)| digger == *item)
    }

    /// The Museum city that trains the Archaeologist soonest and whose head
    /// may give way: `(turns, city, item, head)`. `None` while a city already
    /// trains it at the head, or with the gene off.
    fn dig_archaeologist_candidate(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(f64, u32, Item, Option<Item>)> {
        if !self.dig_every_site_active(g) {
            return None;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let mut best: Option<(f64, u32, Item, Option<Item>)> = None;
        for cid in g.player_city_ids(pid) {
            let Some((digger, turns)) = self.dig_archaeologist_item(g, pid, cid, plan) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&digger) {
                return None;
            }
            if let Some(item) = &head {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.culture_theater_holds(g, pid, cid, plan, item)
                    || self.culture_monument_holds(g, pid, cid, plan, item)
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || self.backlog_builder_holds(g, pid, cid, plan, item)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                    || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            if best
                .as_ref()
                .is_none_or(|(old, old_city, _, _)| (turns, cid) < (*old, *old_city))
            {
                best = Some((turns, cid, digger, head));
            }
        }
        best
    }

    /// See the module: the Museum city's queue takes the Archaeologist. Exact
    /// no-op with the gene off or for another target.
    pub(super) fn claim_dig_archaeologist(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((turns, cid, digger, head)) = self.dig_archaeologist_candidate(g, pid, plan)
        else {
            return;
        };
        let sites = g.excavation_sites(pid).len();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: digger,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                   "{name} claims an Archaeologist for the empty Artifact slots";
                   "culture-digs-every-site: {sites} reachable site(s), {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }

    /// See the module: the treasury buys the claimed Archaeologist when its
    /// queue is slow. True when one was bought. Exact no-op with the gene off.
    pub(super) fn dig_archaeologist_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.dig_every_site_active(g) {
            return false;
        }
        let budget = g.players[pid].gold
            - self
                .purchase_upkeep_floor(g, pid, 0.0)
                .max(self.war_treasury_floor(g, pid));
        if budget <= 0.0 {
            return false;
        }
        let mut best: Option<(i64, u32)> = None;
        for cid in g.player_city_ids(pid) {
            let Some((digger, turns)) = self.dig_archaeologist_item(g, pid, cid, plan) else {
                continue;
            };
            if turns <= ARCHAEOLOGIST_BUY_AFTER_TURNS || g.purchase_is_blocked(cid, &digger) {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, "archaeologist", "gold") else {
                continue;
            };
            if !price.is_finite() || price > budget + 1e-9 {
                continue;
            }
            let key = ((price * 100.0).round() as i64, cid);
            if best.is_none_or(|old| key < old) {
                best = Some(key);
            }
        }
        let Some((price, cid)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: cid,
                unit: crate::name!("archaeologist"),
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Economy, Decision,
               "{} buys an Archaeologist for the empty Artifact slots", g.cities[&cid].name;
               "culture-digs-every-site: {} reachable site(s); {:.0} Gold",
               g.excavation_sites(pid).len(), price as f64 / 100.0);
        true
    }

    /// The order in which the Archaeologist tries sites: own land first, then
    /// unowned, then foreign; nearest first; the tile last for a stable tie.
    fn dig_site_rank(g: &Game, pid: usize, from: Pos, site: Pos) -> (u8, i32, Pos) {
        let owner = g
            .map
            .get(site)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        let land = match owner {
            Some(owner) if owner == pid => 0,
            None => 1,
            Some(_) => 2,
        };
        (land, g.wdist(from, site), site)
    }

    /// See the module: dig here, else step toward the best site still open,
    /// barring each site whose step fails and trying the next. Replaces
    /// `BasicAi::archaeologist_step` while the gene acts.
    pub(super) fn dig_every_site_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let Some(current) = g.units.get(&uid).map(|unit| unit.pos) else {
            return false;
        };
        if let Some(improvement) = g
            .valid_improvements(pid, current)
            .into_iter()
            .find(|name| matches!(name.as_str(), "archaeological_dig" | "shipwreck_excavation"))
        {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(&improvement),
                    },
                )
                .is_ok();
        }
        let turn = g.turn;
        self.dig_dead_sites.retain(|_, until| *until > turn);
        let mut sites: Vec<(u8, i32, Pos)> = {
            let _memo = g.query_memo();
            g.excavation_sites(pid)
                .into_iter()
                .map(|(site, _)| site)
                .filter(|site| !self.dig_dead_sites.contains_key(site))
                .filter(|site| g.route_step(uid, *site, 0).is_some())
                .map(|site| Self::dig_site_rank(g, pid, current, site))
                .collect()
        };
        sites.sort_unstable();
        let barred_for = g.standard_duration(DEAD_SITE_STANDARD_TURNS).max(1);
        for (_, _, site) in sites {
            if self.base.step_toward(g, pid, uid, site) {
                return true;
            }
            self.dig_dead_sites.insert(site, turn + barred_for);
            think!(self.journal(), Military, Detail,
                   "Archaeologist {} turns from the site at {:?}", uid, site;
                   "culture-digs-every-site: no step toward it from {:?}; the site waits {} turn(s) \
                    and the next one is tried", current, barred_for);
        }
        false
    }
}

#[cfg(test)]
mod tests;
