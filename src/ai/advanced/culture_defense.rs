//! `culture-defense-theater` as a production reservation.
//!
//! The gene's `BasicAi::pick_item` step sits after the military floor, so on
//! a Domination seat at war it never binds. Live King 2026-10-03T081800Z
//! armed it and held ZERO Theater Squares at t132 with Culture 24 against
//! 126-150. A replay of every other turn from t125 to t161 shows the step
//! never reached: each delegated pick returned at the lent war floor ("13
//! military for 7 cities against a target of 2.0 each"), a Builder, a Spy or a
//! Caravel. Culture lost four of five games before it, so the Theater is
//! claimed here instead, ahead of the delegated governor, like the
//! higher-level investments: one idle, unthreatened city per call, at its
//! best site, while the empire's Culture trails and fewer than half its
//! cities hold or have queued one (`BasicAi::culture_defense_theater_item`).
//!
//! Only a city that already holds a Campus, and that the Settler step would
//! not send a Settler from, is claimed. Live King
//! 2026-10-03T090618Z, the first game with the reservation, claimed Quito at
//! t48 and the capital at t50, with 3 cities wanting 9 and no Campus anywhere:
//! a King rival's early Culture is four times ours, so the trailing rule holds
//! from Drama and Poetry on, while the culture victories it answers come after
//! t150. The Theater is a later district, never the first.
//!
//! Version 2 (`culture-defense-theater-2`) leaves a housing-bound city whose
//! next housing is a Granary to the governor. In 16 live King games
//! 2026-10-04/05, 11 of the 55 cities at their housing (food surplus 2+, no
//! Granary) were building this Theater at t75; 41 of the 45 capped cities at
//! t75 had no Granary, and the mean housing growth multiplier at t100 was 0.58.

use super::{AdvancedAi, StrategicPlan};
use crate::ai::BasicAi;
use crate::game::{Action, Game};
use crate::think;

impl AdvancedAi {
    pub(super) fn reserve_culture_defense_theater(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !(self.culture_defense_theater || self.culture_defense_theater_2)
            || self.base.minor
            || self.base.barb
        {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        // The economic-recovery bar `BasicAi::pick_item` uses: an empire
        // already paying more than it earns does not add a district's upkeep.
        let recovery_reserve = 100.0 + 25.0 * city_ids.len() as f64;
        if g.players[pid].gold_per_turn < -0.5 && g.players[pid].gold < recovery_reserve {
            return;
        }
        // A city the Settler step would send out a Settler from keeps it:
        // live King 2026-10-03T113755Z held 3 cities from t42 to t100 while
        // Theaters, Campuses and ships took the queues ahead of the walkers.
        let settlers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| g.units[unit].kind == "settler")
            .count()
            + city_ids
                .iter()
                .filter(|cid| {
                    matches!(g.cities[cid].queue.first(),
                        Some(crate::game::Item::Unit { unit }) if unit == "settler")
                })
                .count();
        let mut best = None;
        for &cid in &city_ids {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || !g.city_has_district_family(city, crate::name!("campus"))
                || self.base.settler_due(g, pid, cid, city_ids.len(), settlers)
                || plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            {
                continue;
            }
            // Version 2: a city at its housing whose next housing is a
            // Granary grows first; the governor's housing reserve builds it.
            if self.culture_defense_theater_2
                && matches!(
                    BasicAi::housing_reserve_item(g, pid, cid),
                    Some(crate::game::Item::Building { .. })
                )
            {
                continue;
            }
            let Some(item) = BasicAi::culture_defense_theater_item(g, pid, cid, city_ids.len())
            else {
                continue;
            };
            let production = g.city_yields(cid).production;
            if best
                .as_ref()
                .is_none_or(|(top, _, _)| production > *top)
            {
                best = Some((production, cid, item));
            }
        }
        let Some((_, cid, item)) = best else {
            return;
        };
        let item = self.base.race_takes_the_district_slot(g, pid, cid, item);
        let theater = matches!(
            &item,
            crate::game::Item::District { district, .. }
                if g.district_family(*district) == "theater_square"
        );
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
            && theater
        {
            think!(self.journal(), Economy, Decision,
                "{} starts a Theater Square while our Culture trails", g.cities[&cid].name;
                "culture-defense-theater: one idle queue, ahead of the delegated governor");
        }
    }
}

#[cfg(test)]
mod tests;
