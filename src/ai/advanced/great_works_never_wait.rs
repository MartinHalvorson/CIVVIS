//! `great-works-never-wait`: a Culture seat's Great Works never wait and
//! never leave.
//!
//! **Why (live Deity Kristina, 2026-11-10).**
//! - **Sales.** The bridge's work sale (`append_work_sale_order` in
//!   `civvis_orders`) sells one placed work to a rival whenever a Writer,
//!   Artist or Musician cannot reach a slot, to free the slot. On the Culture
//!   seat that hands a rival Tourism and Culture for a few Gold a turn:
//!   140731Z sold Cantemir's Music at t119 for 7 Gold and 12 a turn, 152217Z
//!   bought Bosch for 639 Gold at t123 and sold it at t143 for 5 a turn, and
//!   161648Z-cont2 sold Kengyo's Music at t161 for 14 a turn.
//! - **The Palace slot.** A Writer that can activate where it stands uses the
//!   Palace's one any-kind slot as readily as an Amphitheater's Writing slot.
//!   At t150 of 161648Z-cont2 the Palace held a Writing while three
//!   Amphitheaters had empty Writing slots, and a Great Musician, whose only
//!   slots before Radio are that Palace slot and a wonder's, idled t105-t160.
//! - **No slot for an Artist.** An Artist with works and no Art slot stands
//!   still (`slot_starved`) until the production governor happens to fill an
//!   idle queue with a Museum: Artists idled t80-t100 and t175-t187 in
//!   161648Z-cont2, t125-t140 in 152217Z and t130-t150 in 140731Z.
//!
//! **What the gene does**, for a seat whose active victory target is Culture:
//! - **No sale.** The bridge appends no work sale
//!   ([`AdvancedAi::culture_keeps_its_great_works`]).
//! - **Writers skip the Palace.** A Writer about to activate on, or walking
//!   to, the Palace's plot walks instead to the nearest host plot with a
//!   known-open slot that is not a city centre, within a detour; see
//!   `civvis_orders/great_works_never_wait.rs`
//!   ([`AdvancedAi::writers_skip_the_palace`]).
//! - **A slot for the waiting person.** While the host reports an Artist or
//!   Musician that cannot reach any slot, the Theater city that builds the
//!   slot building soonest (an Art Museum for Art, a Broadcast Center for
//!   Music once Radio allows it) puts it at the head of its queue over a
//!   deferrable head, within [`SLOT_BUILDING_MAX_STANDARD`] standard turns.
//!   A threatened or freshly attacked city keeps its queue, and so does a
//!   military unit at war (`culture_theater_may_defer`). The claim runs
//!   while `threatened-war-holds-the-claims` holds the economy's claims: on
//!   the live seat the person waits through the Recovery plan (140731Z t140
//!   and 161648Z-cont2 t124 were both held). The governor's rescoring leaves
//!   the claimed building alone
//!   ([`AdvancedAi::great_work_slot_building_holds`]).
//!
//! No civilization is assumed: Kristina's theming is
//! `kristina-fills-to-theme`'s, and the two compose (that gene's walk prefers
//! theming buildings, which never include the Palace). Off, or for another
//! target, nothing changes.

use super::*;

/// The longest slot building the claim takes, in standard turns.
pub(super) const SLOT_BUILDING_MAX_STANDARD: u32 = 30;

/// The building family that opens a slot for a waiting person's kind.
fn slot_building_for(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "artist" => Some(("art", "art_museum")),
        "musician" => Some(("music", "broadcast_center")),
        _ => None,
    }
}

impl AdvancedAi {
    /// Whether the gene acts for this seat now: on, an active Culture target,
    /// and the lane still open.
    pub(super) fn great_works_seat_active(&self, g: &Game) -> bool {
        self.great_works_never_wait
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// Whether the live bridge must not sell a placed Great Work this turn.
    /// False with the gene off or for another target.
    pub fn culture_keeps_its_great_works(&self, g: &Game) -> bool {
        self.great_works_seat_active(g)
    }

    /// Whether the live bridge walks a Writer past the Palace's plot to an
    /// open slot elsewhere. False with the gene off or for another target.
    pub fn writers_skip_the_palace(&self, g: &Game) -> bool {
        self.great_works_seat_active(g)
    }

    /// The `(work kind, building family)` pairs a waiting Artist or Musician
    /// needs: the host reports the person unable to reach any slot
    /// (`StateGreatPerson::slot_starved`). The host's verdict is the
    /// authority; the mirror's own open-slot list is not consulted.
    pub(super) fn starved_slot_buildings(
        g: &Game,
        pid: usize,
    ) -> BTreeSet<(&'static str, &'static str)> {
        g.players[pid]
            .live_great_person_activation_needs
            .iter()
            .filter(|need| need.required_great_work.is_none())
            .filter_map(|need| slot_building_for(&need.kind))
            .collect()
    }

    /// The slot building `cid` would claim and its build turns: a Theater
    /// city of ours, unthreatened, that can build it within the cap, while no
    /// city has one of the family at the head of its queue. `None` with the
    /// gene off or for another target.
    pub(super) fn great_work_slot_building_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.great_works_seat_active(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || !city.districts.iter().any(|(district, pos)| {
                g.district_family(*district) == "theater_square"
                    && g.map.tiles.get(pos).is_some_and(|tile| !tile.pillaged)
            })
        {
            return None;
        }
        let cap = g.standard_duration(SLOT_BUILDING_MAX_STANDARD) as f64;
        Self::starved_slot_buildings(g, pid)
            .into_iter()
            .filter(|(_, family)| {
                !g.player_city_ids(pid).into_iter().any(|other| {
                    other != cid
                        && matches!(
                            g.cities[&other].queue.first(),
                            Some(Item::Building { building })
                                if g.building_is_family(*building, Name::new(family))
                        )
                })
            })
            .filter_map(|(_, family)| BasicAi::civ_building(g, pid, cid, family))
            .filter_map(|item| {
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_remaining_cost_for_city(pid, cid, &item)
                        / g.city_yields(cid).production.max(0.5)
                });
                (turns.is_finite() && turns <= cap).then_some((item, turns))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
    }

    /// Whether `item` at the head of `cid` is the slot building this gene
    /// claims and a person still waits for it: the governor's rescoring
    /// leaves it alone. Always false with the gene off.
    pub(super) fn great_work_slot_building_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        matches!(item, Item::Building { .. })
            && self
                .great_work_slot_building_item(g, pid, cid, plan)
                .is_some_and(|(building, _)| building == *item)
    }

    /// See the module: the Theater city that builds the waiting person's slot
    /// soonest claims it. Exact no-op with the gene off or for another target.
    pub(super) fn claim_great_work_slot_building(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.great_works_seat_active(g) || Self::starved_slot_buildings(g, pid).is_empty() {
            return;
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
            let Some((building, turns)) = self.great_work_slot_building_item(g, pid, cid, plan)
            else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&building) {
                return;
            }
            if let Some(item) = &head {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.culture_theater_holds(g, pid, cid, plan, item)
                    || self.culture_monument_holds(g, pid, cid, plan, item)
                    || self.dig_archaeologist_holds(g, pid, cid, plan, item)
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
                best = Some((turns, cid, building, head));
            }
        }
        let Some((turns, cid, building, head)) = best else {
            return;
        };
        let name = g.cities[&cid].name.clone();
        let label = Self::plain_item(&building);
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: building,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                   "{name} claims {label} for a Great Person with no slot";
                   "great-works-never-wait: the host offers the waiting person no open slot; \
                    {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
