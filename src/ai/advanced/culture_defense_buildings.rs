//! Complete the culture investment before the delegated military floor
//! consumes the queue. This differs from generic culture catch-up: only
//! buildings in a completed Theater are eligible, and one such building
//! already under construction services the reservation across the empire.
//! Never interrupt a queue, open a district, or claim a threatened city.

use super::{AdvancedAi, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::think;

impl AdvancedAi {
    fn defensive_theater_building(g: &Game, item: &Item) -> bool {
        let Item::Building { building } = item else {
            return false;
        };
        let spec = &g.rules.buildings[building];
        !spec.wonder
            && spec.yields.culture > 0.0
            && spec
                .district
                .is_some_and(|d| g.district_family(d) == "theater_square")
    }

    pub(super) fn reserve_culture_defense_building(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.culture_defense_finishes_theater || self.base.minor || self.base.barb {
            return;
        }
        let cities = g.player_city_ids(pid);
        if (g.players[pid].gold_per_turn < -0.5
            && g.players[pid].gold < 100.0 + 25.0 * cities.len() as f64)
            || cities.iter().any(|cid| {
                g.cities[cid]
                    .queue
                    .first()
                    .is_some_and(|item| Self::defensive_theater_building(g, item))
            })
        {
            return;
        }
        let culture = |seat| {
            g.player_city_ids(seat)
                .into_iter()
                .map(|cid| g.city_yields(cid).culture)
                .sum::<f64>()
                + g.observed_yield_adjustments
                    .get(&seat)
                    .map_or(0.0, |y| y.culture)
        };
        let strongest = g
            .players
            .iter()
            .filter(|p| {
                p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.has_met(pid, p.id)
            })
            .map(|p| culture(p.id))
            .fold(0.0_f64, f64::max);
        if strongest <= 0.0 || culture(pid) >= 0.7 * strongest {
            return;
        }
        let mut best = None;
        for &cid in &cities {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || !g.city_has_district_family(city, crate::name!("theater_square"))
                || plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || self.base.barbarian_local_alarm_for_controller(g, pid, cid)
                || g.city_yields(cid).production <= 0.0
            {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                if !Self::defensive_theater_building(g, &item) {
                    continue;
                }
                let turns = self
                    .production_build_turns(g, pid, cid, &item)
                    .ceil()
                    .max(1.0);
                if g.turn_limit().is_some_and(|limit| {
                    turns + g.standard_duration(20) as f64 > limit.saturating_sub(g.turn) as f64
                }) {
                    continue;
                }
                let Item::Building { building } = &item else {
                    unreachable!()
                };
                // Equal-cost museums prefer slots filled by Great People,
                // rather than committing to an additional Archaeologist.
                let artifacts = g.rules.buildings[building]
                    .great_work_slots
                    .get("artifact")
                    .copied()
                    .unwrap_or(0);
                let rank = (turns, artifacts, cid, *building);
                if best.as_ref().is_none_or(|(old, _)| rank < *old) {
                    best = Some((rank, (cid, item)));
                }
            }
        }
        let Some((_, (cid, item))) = best else {
            return;
        };
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Economy, Decision,
                "{} completes its Theater's culture buildings", g.cities[&cid].name;
                "culture-defense-finishes-theater: one safe idle queue while Culture trails");
        }
    }
}

#[cfg(test)]
mod tests;
