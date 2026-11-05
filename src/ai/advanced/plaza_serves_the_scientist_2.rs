//! `plaza-serves-the-scientist-2`: a Science seat's Plaza city builds its next
//! tier building (the Ancestral Hall, then the Intelligence Agency) as soon as
//! it can, ahead of routine work.
//!
//! ## Why
//!
//! Version 1 (`plaza-serves-the-scientist`) chose WHICH tier buildings a
//! Science seat's Plaza takes; nothing made it take them soon. The tier claim
//! of `plaza-in-the-district-list` waits for the Plaza city's queue to stand
//! empty, and on the live seat it rarely does: Builders for the backlog,
//! Settlers and the production hints fill it first. The Intelligence Agency
//! (+1 Spy slot, every offensive operation one level likelier; plaza
//! buildings cannot be bought) stood buildable in the Plaza city and was never
//! built in 4 of 12 Deity Korea science games of 2026-11-05 after version 1
//! was armed (civvis-20261105T131016Z: buildable from turn 88, never queued in
//! 120 turns), and took a median 18 turns where it was. The one Deity win of
//! the lane (civvis-20261105T122513Z) had it at turn 93, 16 turns after it
//! became buildable, and stole 14 Eurekas.
//!
//! ## What the gene does
//!
//! Inert off, and off a Science aim. On, once a turn: the city holding a
//! Government Plaza whose next tier building
//! (`BasicAi::plaza_building_item_for`, the Science list) is buildable and not
//! already queued takes it at the head of its queue, when the head is routine
//! work (`spies-steal-the-lead`'s displacement rule: no defensive building,
//! military unit, Settler, Spy, Wonder or project) and the city is neither the
//! plan's threatened city nor attacked in the last [`PLAZA_TIER_ATTACK_MEMORY`]
//! turns. Version 2 carries version 1: either reads the Science tier list.
//! `Kind::OptIn`. Counter `plaza_tier_claims`.

use super::*;

/// Turns since a city was last attacked under which it keeps its queue.
const PLAZA_TIER_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether the Science tier list is in force: either version on, on a
    /// Science aim.
    pub(super) fn plaza_serves_science_on(&self) -> bool {
        (self.plaza_serves_the_scientist || self.plaza_serves_the_scientist_2)
            && self.victory_target == Some(VictoryTarget::Science)
    }

    /// See the module: the Plaza city's next Science tier building claims its
    /// queue. Exact no-op with version 2 off or off a Science aim.
    pub(super) fn claim_queue_for_the_plaza_tier(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.plaza_serves_the_scientist_2 || self.victory_target != Some(VictoryTarget::Science)
        {
            return;
        }
        let claim = g.player_city_ids(pid).into_iter().find_map(|cid| {
            let item = BasicAi::plaza_building_item_for(g, pid, cid, true)?;
            let city = &g.cities[&cid];
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= PLAZA_TIER_ATTACK_MEMORY;
            (!city.queue.contains(&item)
                && plan.threatened_city != Some(cid)
                && !attacked
                && city
                    .queue
                    .first()
                    .is_none_or(|head| Self::steal_spy_displaces(g, head))
                && g.can_produce(pid, cid, &item))
            .then_some((cid, item))
        });
        let Some((city, item)) = claim else {
            return;
        };
        let displaced = g.cities[&city]
            .queue
            .first()
            .map(Self::plain_item)
            .unwrap_or_else(|| "nothing".to_string());
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            *g.players[pid]
                .counters
                .entry("plaza_tier_claims".to_string())
                .or_insert(0) += 1;
            think!(self.journal(), Economy, Decision,
                   "{} starts {} for the Science plaza", g.cities[&city].name, Self::plain_item(&item);
                   "plaza-serves-the-scientist-2: the Plaza's next tier building goes ahead of {displaced}; \
                    the Intelligence Agency is a Spy slot and a level on every operation, and a plaza \
                    building cannot be bought");
        }
    }
}

#[cfg(test)]
mod tests;
