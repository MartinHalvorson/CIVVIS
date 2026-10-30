//! `culture-seat-theater-first`: on a seat aiming at a Culture victory, every
//! unthreatened city puts its first Theater Square (the civilization's
//! replacement: Greece's Acropolis) at the head of its queue once the district
//! is unlocked, and the free-slot Campus claim yields that city's slot to it.
//!
//! **Why (live Prince Pericles, 2026-10-30, the first two Culture seats).**
//! - Drama and Poetry came at t62 and t63, but the first Acropolis was started
//!   only at t110 and t121: 50-60 turns of an unlocked culture district never
//!   built while the Campus claims (`campus-claims-the-free-slot-2`,
//!   `campus-before-the-army-2` and the rest, all forced on the domination
//!   lineage and none gated on the victory target) took every first slot.
//!   civvis-20261030T043401Z stood at 5 / 7 / 10 Campuses and 0 / 0 / 3
//!   Acropoleis at t80 / t100 / t138, with 0 Great Works at t138.
//! - Tourism ran 8 a turn from t60 to t120 in both games, against rivals
//!   already hosting visitors. 043401Z still won at t188, but 044531Z lost
//!   a Culture race to France at t228 with our tourism at 317 and rising:
//!   France held 24 visiting tourists at t150 when we had 0, and the
//!   Theater chain started 50 turns late is the gap.
//!
//! **What the gene does.** For a Culture seat, a city with no Theater Square
//! standing or queued that can finish one in time
//! ([`BasicAi::first_theater_item`]) claims it: an idle queue, or one over a
//! deferrable head that keeps its progress. A Settler, Walls, a wonder, a
//! project, a repair, the faith defence's Holy Site, Shrine or Temple, an
//! Aqueduct, anything finishing this turn, a military unit while a major is
//! at war with us, and the queues other claims hold keep their place. The
//! claim runs after the Campus claims; the free-slot Campus claim yields a
//! city this claim wants ([`AdvancedAi::culture_theater_item`]), and the
//! delegated governor's rescoring leaves the claimed Theater alone
//! ([`AdvancedAi::culture_theater_holds`]).
//!
//! Off, or for a seat not aiming at Culture, nothing changes.

use super::*;

impl AdvancedAi {
    /// The first Theater Square `cid` would claim and its build turns: a
    /// Culture seat's unthreatened city of ours with none standing or queued.
    /// `None` with the gene off or for another victory target.
    pub(super) fn culture_theater_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.culture_seat_theater_first
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let queued = city.queue.iter().any(|item| {
            matches!(item, Item::District { district, .. }
                if g.district_family(*district) == "theater_square")
        });
        if queued {
            return None;
        }
        let item = BasicAi::first_theater_item(g, pid, cid)?;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        Some((item, turns))
    }

    /// Whether `item` at the head of `cid` is the Theater Square this gene
    /// claims: the delegated governor's rescoring leaves it alone. Always
    /// false with the gene off.
    pub(super) fn culture_theater_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.culture_seat_theater_first
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return false;
        }
        match item {
            Item::District { district, .. } => {
                g.district_family(*district) == "theater_square"
                    && g.cities.get(&cid).is_some_and(|city| {
                        city.owner == pid
                            && plan.threatened_city != Some(cid)
                            && !g.city_has_district_family(city, crate::name!("theater_square"))
                    })
            }
            _ => false,
        }
    }

    /// Whether the queue head `item` may wait for the Theater Square.
    fn culture_theater_may_defer(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        at_major_war: bool,
    ) -> bool {
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Unit { unit } => {
                if *unit == "settler" {
                    return false;
                }
                match g.rules.units.get(unit) {
                    Some(spec) if spec.class == "military" => !at_major_war,
                    Some(_) => matches!(unit.as_str(), "builder" | "scout" | "trader"),
                    None => false,
                }
            }
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls" | "shrine" | "temple"
                    )
            }),
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                family != crate::name!("aqueduct")
                    && family != crate::name!("holy_site")
                    && family != crate::name!("theater_square")
            }
            _ => false,
        }
    }

    /// See the module: every eligible city of a Culture seat claims its first
    /// Theater Square. Exact no-op with the gene off or for another target.
    pub(super) fn claim_culture_theaters(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.culture_seat_theater_first
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        for cid in g.player_city_ids(pid) {
            let Some((theater, turns)) = self.culture_theater_item(g, pid, cid, plan) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if let Some(item) = &head {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.deterrence_unit_holds(g, pid, item)
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || self.backlog_builder_holds(g, pid, cid, plan, item)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                    || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: theater.clone(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Cities, Decision,
                       "{name} claims its {} for the Culture victory", Self::plain_item(&theater);
                       "culture-seat-theater-first: {turns:.1} turns, ahead of {}",
                       head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                           "{} (it keeps its progress)", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
