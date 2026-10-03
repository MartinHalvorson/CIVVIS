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
        if !self.culture_defense_theater || self.base.minor || self.base.barb {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        // The economic-recovery bar `BasicAi::pick_item` uses: an empire
        // already paying more than it earns does not add a district's upkeep.
        let recovery_reserve = 100.0 + 25.0 * city_ids.len() as f64;
        if g.players[pid].gold_per_turn < -0.5 && g.players[pid].gold < recovery_reserve {
            return;
        }
        let mut best = None;
        for &cid in &city_ids {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
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
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                "{} starts a Theater Square while our Culture trails", g.cities[&cid].name;
                "culture-defense-theater: one idle queue, ahead of the delegated governor");
        }
    }
}

#[cfg(test)]
mod tests;
