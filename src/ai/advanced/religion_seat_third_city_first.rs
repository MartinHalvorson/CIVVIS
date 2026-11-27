//! `religion-seat-third-city-first`: a two-city Religion seat's next Settler
//! takes the best city's queue before the band turn, ahead of the first
//! Shrine, Prayers and routine work.
//!
//! **Why (Tournament 8, Lv 8 Deity, Online, 2026-11-26).** Four Religion
//! seats founded their third city 25-51 turns after the second:
//! `civvis-t8-20261126T093036Z` t24 then t49, `…104734Z` t14 then t54,
//! `…113257Z` t15 then t66 and `…151149Z` t22 then t53. Each held 2-4 cities
//! at t60 against a rival's 6. `religion-seat-second-city-first` (armed)
//! gets the second city out; nothing guards the next one, and the same
//! claims take it:
//! - `093036Z`: founded t24, and `religion-seat-first-shrine` went "ahead of
//!   settler" in Addis Ababa at t25. No Settler was asked again until the
//!   early floor at t31.
//! - `104734Z`: Cairo ran Prayers t15-23 (`early-settler-floor` never
//!   displaces a project), founded t23, and the Shrine went ahead of the
//!   Settler at t24. The floor came back at t33 in Aleppo and t37 in Cairo;
//!   a siege at t42 then held both.
//! - `151149Z`: the capital's Settler asked at t23 lost to the Shrine the
//!   same turn and came back at t27; that Settler was then lost to raiders.
//!
//! **What the gene does.** While the seat aims at Religion, holds two cities
//! and no Settler is on the board or at a queue head, before the early
//! floor's band turn, the city at the host's Settler floor that trains a
//! Settler soonest (within the floor's longest build) ends the production
//! pass on one. Threatened cities and cities attacked in the last four turns
//! are passed over, as is a seat with no practical site. The head keeps its
//! place on exactly the second-city gene's terms
//! (`second_city_head_keeps`): a defender with hostiles in reach, Walls, a
//! Holy Site already under way, a head finishing this turn, or a Holy Site
//! or Prayers with one religion slot left before founding. The first
//! Shrine does not take that city's queue meanwhile; it waits until the
//! Settler is out. The claim runs right after the second-city claim, so the
//! paused-district, Prayers-guard and defence handoffs keep the last word.
//!
//! Off, for another target, with one city or three, or past the band turn,
//! nothing changes.

use super::early_settler_floor::{EARLY_SETTLER_FLOOR_STANDARD_TURN, EARLY_SETTLER_MAX_BUILD};
use super::*;

/// A city attacked this recently keeps its queue.
const THIRD_CITY_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The gene is on, the seat aims at Religion, holds exactly two cities
    /// and it is still before the band turn.
    fn third_city_applies(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_third_city_first
            && !self.base.minor
            && !self.base.barb
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.turn < g.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN)
            && g.player_city_ids(pid).len() == 2
    }

    /// Whether `cid` is training the third city's Settler, which the first
    /// Shrine then leaves alone.
    pub(super) fn third_city_keeps_its_settler(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.third_city_applies(g, pid)
            && matches!(
                g.cities[&cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
            && !g
                .player_unit_ids(pid)
                .into_iter()
                .any(|uid| g.units[&uid].kind == "settler")
    }

    /// The city that should be training the seat's third-city Settler now,
    /// or `None` (gene off, another target, not two cities, a Settler out or
    /// queued, past the band turn, nowhere to settle, or no city may).
    pub(super) fn third_city_settler_due(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        if !self.third_city_applies(g, pid) {
            return None;
        }
        let cities = g.player_city_ids(pid);
        let (walkers, queued) = Self::early_settler_census(g, pid);
        if walkers > 0 || queued > 0 || !self.base.has_practical_settle_site(g, pid) {
            return None;
        }
        let item = Self::second_city_settler();
        let max_turns = g.standard_duration(EARLY_SETTLER_MAX_BUILD) as f64;
        cities
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                if (city.pop as f64) < crate::ai::HOST_SETTLER_MIN_POP
                    || plan.threatened_city == Some(cid)
                    || (city.last_attacked > 0
                        && g.turn.saturating_sub(city.last_attacked) <= THIRD_CITY_ATTACK_MEMORY)
                    || !g.can_produce(pid, cid, &item)
                    || self.second_city_head_keeps(g, pid, cid).is_some()
                {
                    return None;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
                });
                (turns <= max_turns).then_some((turns, cid))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, cid)| cid)
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn claim_religion_seat_third_city(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some(cid) = self.third_city_settler_due(g, pid, plan) else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: Self::second_city_settler(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                "{} trains the third city's Settler", g.cities[&cid].name;
                "religion-seat-third-city-first: two cities, no Settler out, {} of {} religions founded, ahead of {}",
                g.religions_founded(), g.max_religions(),
                displaced.map_or_else(|| "an idle queue".to_string(), |head| format!(
                    "{} (it keeps its progress)", Self::plain_item(&head))));
        }
    }
}

#[cfg(test)]
mod tests;
