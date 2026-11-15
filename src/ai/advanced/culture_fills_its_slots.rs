//! `culture-seat-fills-its-district-slots`: a Culture seat's Theater city
//! with a free specialty district slot puts a Campus, else a Commercial Hub,
//! else a Harbor at the head of its queue, ahead of an idle queue, a
//! Neighborhood, a Spy or a plain building.
//!
//! ## Why (live Deity Pericles, 6-player Continents, Online, `civvis-20261115T071444Z`)
//!
//! Greece aimed at Culture and held ten cities from turn 100. Read from the
//! host's `district_capacity` and `buildable` list in the state rows:
//!
//! - **Seven to nine slots stand empty to the end.** Specialty slots against
//!   districts placed: 27 / 20 at turn 100, 33 / 26 at turn 120, 39 / 30 at
//!   turn 140, 46 / 39 at turn 160, 47 / 39 at turn 175.
//! - **The districts are on offer.** From turn 60 the game holds 500
//!   city-turns in which a city has a free slot and no specialty district in
//!   production (6,889 Production spent on them). The host offers a
//!   Commercial Hub on 83% of them (median 116 Production), a Campus on 77%
//!   (133), a Harbor on 47% (95).
//! - **What the city builds instead.** A Builder on 12%, a Neighborhood on
//!   10%, an Amphitheater on 9%, an Art Museum, an Aqueduct or a Broadcast
//!   Center on 6% each, a Bombard, a Temple, an Archaeologist, a Spy.
//! - **What that leaves.** At turn 175: 12 Neighborhoods and 8 Aqueducts
//!   against 5 Campuses, 3 Commercial Hubs and 3 Harbors in 10 cities, every
//!   one of which holds its Theater Square (Acropolis) and its Holy Site.
//!   The cities work all of their 8-14 tiles, so a Neighborhood's new
//!   citizens can only be specialists. Science 186 against 488 for the best
//!   rival (Radio at turn 128, Computers at 154), Production 299 against
//!   906-1038.
//!
//! Radio and Computers are the Culture road's own late techs (the Broadcast
//! Center, the +Tourism of Computers), and a Commercial Hub or Harbor is a
//! trade route, which the per-civ tourism multipliers ride on.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned victory target is Culture, from standard
//! turn [`CULTURE_SLOT_START_STANDARD`] (60 at Online), the turn
//! `score-seat-fills-its-district-slots` opens at. Off, an exact no-op.
//!
//! Every turn, each city of [`CULTURE_SLOT_MIN_POP`] citizens or more that
//! already holds its Theater Square, neither threatened nor attacked lately,
//! with no specialty district in its queue, and whose last free slot
//! `culture-faith-site-per-theater-3` does not keep for a Holy Site
//! ([`AdvancedAi::culture_faith_slot_is_kept`]), takes the first district of
//! [`CULTURE_SLOT_FAMILIES`] it can place and finish within
//! [`CULTURE_SLOT_MAX_STANDARD`] standard turns
//! ([`AdvancedAi::culture_slot_item`]). It goes to the head of the queue
//! ([`AdvancedAi::claim_culture_slots`]) when the queue is idle or its head
//! is a Neighborhood, a Spy, or a building that is not a wonder, not Walls
//! and not a Theater Square building, at most
//! [`CULTURE_SLOT_HEAD_PAID_SHARE`] paid, whose remaining cost is at least
//! the district's ([`AdvancedAi::culture_slot_may_defer`]). Every other unit
//! (Settlers among them), every other district, a wonder, a project, an
//! Amphitheater, a Museum or a Broadcast Center, and any cheaper item keep
//! their queue; the displaced item keeps its progress.
//!
//! The claim runs beside the Score one, after the Campus claims (so a city's
//! first Campus and its Library keep their order) and before the Culture,
//! defence, repair and endgame claims, which may still take the queue.
//!
//! Journal reasons start `culture-seat-fills-its-district-slots:`.

use super::*;

/// The standard turn the claim opens at (60 at Online), the Score claim's.
pub(crate) const CULTURE_SLOT_START_STANDARD: u32 = 90;

/// The longest the claimed district may take, in standard turns (12 at
/// Online).
pub(crate) const CULTURE_SLOT_MAX_STANDARD: u32 = 18;

/// The smallest city that claims: its second slot opens at four citizens.
pub(crate) const CULTURE_SLOT_MIN_POP: i32 = 4;

/// The most of the head item's cost that may already be paid.
pub(crate) const CULTURE_SLOT_HEAD_PAID_SHARE: f64 = 0.25;

/// The district families the claim places, in order: Science for Radio and
/// Computers, then the trade routes.
pub(crate) const CULTURE_SLOT_FAMILIES: [&str; 3] = ["campus", "commercial_hub", "harbor"];

/// A city attacked within this many turns keeps its queue.
const CULTURE_SLOT_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The gene is on and the seat aims at Culture.
    pub(super) fn culture_fills_its_slots(&self) -> bool {
        self.culture_seat_fills_its_district_slots
            && self.victory_target == Some(VictoryTarget::Culture)
    }

    /// The district `cid` would claim and its cost: see the module. `None`
    /// with the gene off.
    pub(super) fn culture_slot_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.culture_fills_its_slots()
            || g.turn < g.standard_duration(CULTURE_SLOT_START_STANDARD)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || city.pop < CULTURE_SLOT_MIN_POP
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= CULTURE_SLOT_ATTACK_MEMORY)
            || !g.city_has_district_family(city, crate::name!("theater_square"))
            || city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.rules.districts.get(district).is_some_and(|spec| spec.specialty))
            })
            || self.culture_faith_slot_is_kept(g, pid, cid)
        {
            return None;
        }
        let budget = g.standard_duration(CULTURE_SLOT_MAX_STANDARD) as f64;
        CULTURE_SLOT_FAMILIES.iter().find_map(|family| {
            let item = BasicAi::first_district_item_with_budget(g, pid, cid, family, budget)?;
            let cost = g.item_cost_for_city(pid, cid, &item);
            Some((item, cost))
        })
    }

    /// Whether the queue head `item` may wait for a district that costs
    /// `district_cost`. See the module.
    pub(super) fn culture_slot_may_defer(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        district_cost: f64,
    ) -> bool {
        let yields = match item {
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
                    && spec
                        .district
                        .is_none_or(|district| g.district_family(district) != "theater_square")
            }),
            Item::District { district, .. } => g.district_family(*district) == "neighborhood",
            Item::Unit { unit } => unit == "spy",
            _ => false,
        };
        let cost = g.item_cost_for_city(pid, cid, item);
        let invested = g.item_invested_production(cid, item);
        yields
            && invested <= CULTURE_SLOT_HEAD_PAID_SHARE * cost
            && cost - invested >= district_cost
    }

    /// See the module: every Theater city with a free slot, a district it
    /// can place in time and a queue that may wait puts that district at its
    /// head. Exact no-op with the gene off.
    pub(super) fn claim_culture_slots(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.culture_fills_its_slots() {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((district, cost)) = self.culture_slot_item(g, pid, cid, plan) else {
                continue;
            };
            let displaced = match g.cities[&cid].queue.first() {
                Some(head) => {
                    if !Self::culture_slot_may_defer(g, pid, cid, head, cost) {
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
                    "culture-seat-fills-its-district-slots: {cost:.0} Production in {turns:.1} \
                     turns beside its Theater Square{}",
                    displaced.map_or(String::new(), |item| format!(
                        "; {} keeps its progress", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
