//! `first-campus-takes-a-filler`: once Writing is known and the empire holds
//! no Campus (standing, placed or queued), the safe city that finishes its
//! first Campus soonest puts it at the head of its queue, displacing a filler:
//! a Monument, a Granary, a Shrine, a Temple, or (at peace with every major)
//! a Warrior, Scout, Slinger, Archer or Spearman. It never displaces a
//! Settler, Builder, Trader, wonder, district, project or Walls, and leaves
//! a head the host finishes this turn.
//!
//! **Why (live King, 11-01/02).**
//! - Over 30 games the first Campus finished at a median t64-72; its turn
//!   correlates −0.59 with Science at t100 (n=28). The capital's Campus
//!   started a median 20 turns after Writing (3 of the 4 wins inside 6).
//!   Between Writing and the capital's Campus the capital built Settlers
//!   (47), Builders (45), Monuments (27), Archers (20), Granaries (19), Walls
//!   (11) and Temples (9).
//! - `campus-unlock-every-lane` (armed 11-02 08:55Z) moved Writing to t33 and
//!   t37 in its first two live games, against a baseline median of t48, but
//!   the first Campus still started at t50 and t62. The queues went to
//!   Archers, Warriors, Scouts, Builders, Granaries, Walls, Monuments,
//!   Theaters, a Shrine and a Temple, so the unlock alone does not move the
//!   Campus.
//! - `capital-campus-before-the-plaza` claims only an EMPTY capital queue,
//!   and `capital_campus_first` waits for three cities.
//!
//! Runs after every ordinary writer and claim and before the launch-pad
//! hold and the defence, declaration and loyalty handoffs, which keep the
//! last word. One claim a turn; once any Campus is committed the gene is
//! inert. Off, nothing runs.

use super::*;

/// Turns since a city was last attacked under which it is left alone.
const FIRST_CAMPUS_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether any city of ours holds a Campus, has one queued, or has one
    /// placed on its tiles.
    pub(super) fn empire_has_campus_commitment(g: &Game, pid: usize) -> bool {
        let campus = crate::name!("campus");
        g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            city.districts
                .keys()
                .any(|district| g.district_family(*district) == campus)
                || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                        if g.district_family(*district) == campus)
                })
                || city.owned_tiles.iter().any(|pos| {
                    g.map
                        .get(*pos)
                        .and_then(|tile| tile.district_foundation.as_ref())
                        .is_some_and(|foundation| g.district_family(foundation.district) == campus)
                })
        })
    }

    /// Whether `item` is a filler the first Campus may displace. Soldiers
    /// count only at peace with every major.
    pub(super) fn first_campus_filler(item: &Item, at_major_war: bool) -> bool {
        match item {
            Item::Building { building } => {
                *building == "monument"
                    || *building == "granary"
                    || *building == "shrine"
                    || *building == "temple"
            }
            Item::Unit { unit } => {
                !at_major_war
                    && (*unit == "warrior"
                        || *unit == "scout"
                        || *unit == "slinger"
                        || *unit == "archer"
                        || *unit == "spearman")
            }
            _ => false,
        }
    }

    /// The city and Campus the claim would place: `None` with the gene off,
    /// before Writing, once a Campus is committed, or when no safe city leads
    /// with a filler (or nothing) and finishes its first Campus within
    /// `BasicAi::first_campus_item`'s bound.
    pub(super) fn first_campus_filler_claim(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.first_campus_takes_a_filler
            || self.base.minor
            || self.base.barb
            || !g.players[pid].techs.contains(&crate::name!("writing"))
            || Self::empire_has_campus_commitment(g, pid)
        {
            return None;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        });
        let visible = g.player_vision_frame(pid);
        let mut best: Option<(f64, u32, Item)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= FIRST_CAMPUS_ATTACK_MEMORY;
            if plan.threatened_city == Some(cid)
                || attacked
                || Self::imminent_city_attack(g, pid, cid, &visible)
            {
                continue;
            }
            if let Some(head) = city.queue.first() {
                if !Self::first_campus_filler(head, at_major_war) {
                    continue;
                }
                // The head the host finishes this turn keeps its turn.
                if g.host_production_turns(cid, head)
                    .is_some_and(|turns| turns.is_finite() && turns > 0.0 && turns <= 1.0)
                {
                    continue;
                }
            }
            let Some(item) = BasicAi::first_campus_item(g, pid, cid) else {
                continue;
            };
            let turns = g
                .host_production_turns(cid, &item)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
                .unwrap_or_else(|| {
                    g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
                });
            if best.as_ref().is_none_or(|(seen, seen_cid, _)| {
                turns < *seen || (turns == *seen && cid < *seen_cid)
            }) {
                best = Some((turns, cid, item));
            }
        }
        best.map(|(_, cid, item)| (cid, item))
    }

    /// See the module: put the empire's first Campus at the head of the
    /// claimed city's queue. Exact no-op while the gene is off.
    pub(super) fn claim_first_campus_takes_a_filler(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((cid, item)) = self.first_campus_filler_claim(g, pid, plan) else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        if g.apply(
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
                |head| Self::plain_item(&head),
            );
            think!(self.journal(), Economy, Decision,
                "{} starts the empire's first Campus", g.cities[&cid].name;
                "first-campus-takes-a-filler: Writing is known and no Campus stands, is placed or is queued; {waits} waits");
        }
    }
}

#[cfg(test)]
mod tests;
