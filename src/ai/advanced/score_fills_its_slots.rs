//! `score-seat-fills-its-district-slots`: a Score seat's city with a free
//! district slot puts the cheapest specialty district it can place at the
//! head of its queue, ahead of an idle queue or of a building that costs at
//! least as much for half the points.
//!
//! ## Why (live Deity Rome, 6-player Continents, Online, turn limit 170)
//!
//! A district is two points at the tally (`Game::score_parts`) and opens
//! three buildings at one point each. Ten games of 2026-11-13
//! (`civvis-20261113T104009Z` to `T145542Z`), read from the host's
//! `district_capacity` and `buildable` list in the state rows:
//!
//! - **Half the slots stand empty.** Slots against specialty districts
//!   placed: 9.8 / 4.4 at turn 60, 16.7 / 8.3 at turn 80, 22.9 / 12.7 at turn
//!   100, 28.4 / 17.7 at turn 120, 32.6 / 23.1 at turn 140 and 35.8 / 27.3 at
//!   turn 160. Nine or ten slots are free from turn 100 to the tally.
//! - **The districts are on offer.** From turn 60 to turn 140 a game holds
//!   424 city-turns in which a city has a free slot and the host offers it a
//!   specialty district: an Entertainment Complex on 95% of them, a
//!   Commercial Hub on 93%, an Encampment or a Theater Square on 88%, an
//!   Industrial Zone on 77%, a Harbor on 62%.
//! - **What the city builds instead.** A University on 10% of those
//!   city-turns, a Research Lab on 4%, a Builder on 5%, a Workshop, a Spy, a
//!   Granary, a Library; the queue read empty on 17%.
//! - **The price.** The host asks 85 Production for a district at turn 60
//!   and 107-112 at turn 80. From turn 100 the kinds the empire holds fewer
//!   of than the world's average cost 86-105 (Commercial Hub, Entertainment
//!   Complex, Encampment, Harbor) while a Campus, Theater Square or
//!   Industrial Zone costs 133-180. A University is 125 for one point, a Bank
//!   145, a Factory 165, a Research Lab 220.
//!
//! The districts a city does get come from one claim each (the first Campus,
//! the Industrial Zone, the first Commercial Hub of the empire, a Culture
//! seat's Theater Square); the governor's own district step reads an idle
//! queue the claims take first.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned victory target is Score, from standard turn
//! [`SCORE_SLOT_START_STANDARD`] (60 at Online). Off, an exact no-op.
//!
//! Every turn, each city of [`SCORE_SLOT_MIN_POP`] citizens or more, neither
//! threatened nor attacked lately, with no specialty district in its queue,
//! looks at the first district of each family in [`SCORE_SLOT_FAMILIES`] it
//! can place and finish within [`SCORE_SLOT_MAX_STANDARD`] standard turns,
//! and takes the cheapest (within a tenth, the earlier in the list:
//! [`AdvancedAi::score_slot_item`]). It goes to the head of the queue
//! ([`AdvancedAi::claim_score_slots`]) when the queue is idle or its head is
//! a building, not a wonder and not Walls, at most
//! [`SCORE_SLOT_HEAD_PAID_SHARE`] paid, whose remaining cost is at least the
//! district's ([`AdvancedAi::score_slot_may_defer`]): one point for the
//! price of two. Every unit, every district, a wonder, a project and any
//! cheaper building keep their queue; the displaced building keeps its
//! progress.
//!
//! The claim runs after the Campus claims, so a city's first Campus and its
//! Library keep their order, and before the defence, repair and endgame
//! claims, which may still take the queue.
//!
//! ⚠ Not here: a Holy Site (the faith genes read one as a commitment), an
//! Aqueduct or Bath (`score-seat-builds-the-bath`), a Neighborhood, and the
//! Government Plaza.
//!
//! Journal reasons start `score-seat-fills-its-district-slots:`.

use super::*;

/// The standard turn the claim opens at (60 at Online): before it a city's
/// queue belongs to its Settlers, its first Campus and its Library.
pub(crate) const SCORE_SLOT_START_STANDARD: u32 = 90;

/// The longest the claimed district may take, in standard turns (12 at
/// Online).
pub(crate) const SCORE_SLOT_MAX_STANDARD: u32 = 18;

/// The smallest city that claims: its second slot opens at four citizens,
/// and the first is the Campus claim's.
pub(crate) const SCORE_SLOT_MIN_POP: i32 = 4;

/// The most of the head building's cost that may already be paid.
pub(crate) const SCORE_SLOT_HEAD_PAID_SHARE: f64 = 0.25;

/// A district within this share of the cheapest one's cost counts as cheap
/// as it; the earlier family in [`SCORE_SLOT_FAMILIES`] then goes first.
pub(crate) const SCORE_SLOT_COST_SLACK: f64 = 1.1;

/// The district families the claim places, the more useful first.
pub(crate) const SCORE_SLOT_FAMILIES: [&str; 7] = [
    "commercial_hub",
    "theater_square",
    "industrial_zone",
    "campus",
    "harbor",
    "entertainment_complex",
    "encampment",
];

/// A city attacked within this many turns keeps its queue.
const SCORE_SLOT_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The gene is on and the seat aims at Score.
    pub(super) fn score_fills_its_slots(&self) -> bool {
        self.score_seat_fills_its_district_slots
            && self.victory_target == Some(VictoryTarget::Score)
    }

    /// Whether `item` is a specialty district of one of the claim's families.
    fn score_slot_district(g: &Game, item: &Item) -> bool {
        matches!(item, Item::District { district, .. }
            if SCORE_SLOT_FAMILIES.contains(&g.district_family(*district).as_str()))
    }

    /// The district `cid` would claim and its cost: see the module. `None`
    /// with the gene off.
    pub(super) fn score_slot_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.score_fills_its_slots() || g.turn < g.standard_duration(SCORE_SLOT_START_STANDARD)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || city.pop < SCORE_SLOT_MIN_POP
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= SCORE_SLOT_ATTACK_MEMORY)
            || city
                .queue
                .iter()
                .any(|item| Self::score_slot_district(g, item))
        {
            return None;
        }
        let budget = g.standard_duration(SCORE_SLOT_MAX_STANDARD) as f64;
        let offers: Vec<(Item, f64)> = SCORE_SLOT_FAMILIES
            .iter()
            .filter_map(|family| {
                let item = BasicAi::first_district_item_with_budget(g, pid, cid, family, budget)?;
                let cost = g.item_cost_for_city(pid, cid, &item);
                Some((item, cost))
            })
            .collect();
        let cheapest = offers
            .iter()
            .map(|(_, cost)| *cost)
            .fold(f64::INFINITY, f64::min);
        offers
            .into_iter()
            .find(|(_, cost)| *cost <= SCORE_SLOT_COST_SLACK * cheapest)
    }

    /// Whether the queue head `item` may wait for a district that costs
    /// `district_cost`. See the module.
    pub(super) fn score_slot_may_defer(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        district_cost: f64,
    ) -> bool {
        let Item::Building { building } = item else {
            return false;
        };
        let plain = g.rules.buildings.get(building).is_some_and(|spec| {
            !spec.wonder
                && !matches!(
                    building.as_str(),
                    "walls" | "medieval_walls" | "renaissance_walls"
                )
        });
        let cost = g.item_cost_for_city(pid, cid, item);
        let invested = g.item_invested_production(cid, item);
        plain && invested <= SCORE_SLOT_HEAD_PAID_SHARE * cost && cost - invested >= district_cost
    }

    /// See the module: every city with a free slot, a district it can place
    /// in time and a queue that may wait puts that district at its head.
    /// Exact no-op with the gene off.
    pub(super) fn claim_score_slots(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.score_fills_its_slots() {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((district, cost)) = self.score_slot_item(g, pid, cid, plan) else {
                continue;
            };
            let displaced = match g.cities[&cid].queue.first() {
                Some(head) => {
                    if !Self::score_slot_may_defer(g, pid, cid, head, cost) {
                        continue;
                    }
                    Some(head.clone())
                }
                None => None,
            };
            let name = g.cities[&cid].name.clone();
            let label = Self::plain_item(&district);
            let turns = g
                .host_production_turns(cid, &district)
                .unwrap_or_else(|| cost / g.city_yields(cid).production.max(0.5));
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: district,
                },
            )
            .is_ok()
            {
                think!(self.journal(), Economy, Decision,
                    "{name} fills its free district slot with {label}";
                    "score-seat-fills-its-district-slots: {cost:.0} Production in {turns:.1} turns \
                     for two points and the buildings it opens{}",
                    displaced.map_or(String::new(), |item| format!(
                        "; {} keeps its progress", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
