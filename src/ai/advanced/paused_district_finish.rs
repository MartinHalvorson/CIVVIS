//! `paused-district-finishes`: a district foundation the host quotes at two
//! turns or less, with work already banked on it, goes back to the head of
//! its city's queue while that city stands at full city and wall health with
//! no attack in reach; the defence handoff keeps it there for those turns.
//!
//! **Why (10-11 census, 53 live Emperor and Immortal Gran Colombia runs).**
//! In 51 of the 53 games a city held a paused foundation the host itself
//! priced at one or two turns while it built something else: 274 stretches,
//! 1,912 city-turns, 83% of them at full health, and 702 of those turns spent
//! on military units. Holy Sites (509 city-turns), Campuses (336), Government
//! Plazas (284) and Aqueducts (259) led; the capital alone held 63 stretches.
//! Immortal civvis-20261011T054318Z is the worst case: Bogotá's first Campus
//! stood at 46 of 56 from turn 53, one turn from done, while the war's
//! defence handoff answered "under fire or facing a confirmed siege with 0
//! combined city/wall health missing" with Horsemen, Heavy Chariots and
//! Swordsmen until turn 82. Our Science ran 0.33 of the rival median at turn
//! 100 there; across the runs the top third by Science per citizen had its
//! first Campus at turn 52, the bottom third at 72.
//!
//! The rule spends at most two turns of a city's production. Any damage to
//! the city or its walls, a hostile that can strike the City Center next
//! turn, or a head item the host finishes this turn leaves the queue to the
//! existing writers.
//!
//! The signal is a foundation on the live board (the planner's reservations
//! live only on its private clone) that the host's own menu quotes at two
//! turns or less. The banked work itself is journaled when the mirror read
//! it but is not required: `step_district_progress` resolves our cities
//! through `known_city_ids`, whose native ids collide with rival and
//! city-state cities (Bogotá's 65536 maps to a city-state in a fresh-board
//! replay of G054318Z), so a paused foundation's `pr` often never reaches
//! `production_progress`. The host menu is keyed through our own city ids.

use super::{AdvancedAi, CITY_MAX_HP};
use crate::game::{Action, Game, Item};
use crate::think;

/// Host turns within which a paused foundation is finished first.
pub(super) const PAUSED_DISTRICT_FINISH_TURNS: f64 = 2.0;

impl AdvancedAi {
    /// The paused district foundation this city finishes first, with the
    /// host's turns and the work banked on it when the mirror read it;
    /// `None` with the gene off or behind any guard. A foundation already at
    /// the head of the queue is returned too, so the defence handoff can keep
    /// it there.
    pub(super) fn paused_district_finish_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<(Item, f64, Option<f64>)> {
        if !self.paused_district_finishes || self.base.minor || self.base.barb {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid || city.hp < CITY_MAX_HP || city.wall_hp < g.city_max_wall_hp(city) {
            return None;
        }
        let head = city.queue.first();
        let mut best: Option<(Item, f64, Option<f64>)> = None;
        for pos in &city.owned_tiles {
            let Some(foundation) = g
                .map
                .tiles
                .get(pos)
                .and_then(|tile| tile.district_foundation.as_ref())
            else {
                continue;
            };
            let item = Item::District {
                district: foundation.district,
                pos: *pos,
            };
            // Work on the head lives in `City::production`; a paused
            // foundation's in `production_progress` when the mirror read the
            // host menu's `pr` for it (see the module note).
            let banked = if head == Some(&item) {
                Some(city.production)
            } else {
                city.production_progress
                    .get(&Game::item_progress_key(&item))
                    .copied()
            };
            let Some(turns) = g
                .host_production_turns(cid, &item)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
            else {
                continue;
            };
            if turns > PAUSED_DISTRICT_FINISH_TURNS || !g.can_produce(pid, cid, &item) {
                continue;
            }
            let better = best.as_ref().is_none_or(|(old, old_turns, _)| {
                turns < *old_turns
                    || (turns == *old_turns && format!("{item:?}") < format!("{old:?}"))
            });
            if better {
                best = Some((item, turns, banked));
            }
        }
        let (item, turns, banked) = best?;
        if let Some(head) = head.filter(|head| **head != item) {
            // The head the host finishes this turn keeps its turn.
            if g
                .host_production_turns(cid, head)
                .is_some_and(|turns| turns.is_finite() && turns > 0.0 && turns <= 1.0)
            {
                return None;
            }
        }
        if Self::imminent_city_attack(g, pid, cid, &g.player_vision_frame(pid)) {
            return None;
        }
        Some((item, turns, banked))
    }

    /// See [`Self::paused_district_finish_item`]: put each such foundation at
    /// the head of its city's queue. Runs after every ordinary production
    /// writer and before the surprise-defence and loyalty handoffs.
    pub(super) fn claim_paused_district_finishes(&self, g: &mut Game, pid: usize) {
        if !self.paused_district_finishes {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((item, turns, banked)) = self.paused_district_finish_item(g, pid, cid) else {
                continue;
            };
            let displaced = g.cities[&cid].queue.first().cloned();
            if displaced.as_ref() == Some(&item) {
                continue;
            }
            let cost = g
                .host_buildable
                .get(&cid)
                .and_then(|menu| menu.get(&Game::production_block_key(&item)))
                .and_then(|entry| entry.cost)
                .unwrap_or(0.0);
            if g
                .apply(
                    pid,
                    &Action::Produce {
                        city: cid,
                        item: item.clone(),
                    },
                )
                .is_ok()
            {
                let waits = displaced.map_or_else(
                    || "an empty queue".to_string(),
                    |item| Self::plain_item(&item),
                );
                let banked =
                    banked.map_or_else(|| "unread".to_string(), |banked| format!("{banked:.0}"));
                think!(self.journal(), Economy, Decision,
                    "{} finishes its paused {}", g.cities[&cid].name, Self::plain_item(&item);
                    "paused-district-finishes: {turns:.0} host turn(s) left ({banked} of {cost:.0} banked) at full health with no attack in reach; {waits} waits");
            }
        }
    }
}

#[cfg(test)]
mod tests;
