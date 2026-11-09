//! `religion-seat-guards-the-race`: a Religion-target seat repairs a pillaged
//! Holy Site at once, and a Prayers project finishing this turn keeps its
//! queue.
//!
//! **Why (live Deity Russia civvis-20261109T141812Z, 2026-11-09).** The Lavra
//! stood at t11 and the seat held 8 Prophet points at t15. On t14 Prayers sat
//! at 15 of 17 at 5 Production, and an Archer took the queue ("delays its
//! opening for nearby barbarian pressure"), so the project's +10 never
//! landed. A lone barbarian Warrior then pillaged the Lavra on t16-17. It
//! stayed pillaged until t32 and the points froze at 11, though the repair
//! cost 14 Production and no hostile was in sight from t17 to t31. The seat
//! founded at t37; the two sibling games founded at t24.
//!
//! `pillaged-districts-repair-first` runs among the idle-queue claims, before
//! the opening book and the governors write the queue, and it never displaces
//! a Settler. The book's Settler, Builder and Monument held the queue until
//! t31. `near-done-work-finishes` does not restore a head a soldier displaced,
//! which is right in general and wrong for an item that finishes this turn.
//!
//! **What the gene does.** After every ordinary production writer:
//! - a Prayers project that headed the queue when the turn began and finishes
//!   this turn goes back to the head, whatever displaced it. The displacing
//!   item starts a turn later.
//! - each city with a pillaged Holy Site family district and no hostile soldier
//!   within [`REPAIR_SAFE_TILES`] of it takes the repair ahead of anything but
//!   Walls or a head finishing this turn. A threatened city or one attacked in
//!   the last four turns keeps its queue.
//!
//! Off, or for another target, nothing changes.

use super::pillaged_repairs_first::REPAIR_SAFE_TILES;
use super::*;

/// A city attacked this recently keeps its queue for the repair.
const RACE_REPAIR_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    fn race_guard_active(&self, g: &Game) -> bool {
        self.religion_seat_guards_the_race
            && !self.base.minor
            && !self.base.barb
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
    }

    fn race_guard_is_prayers(item: &Item) -> bool {
        matches!(item, Item::Project { project } if *project == "holy_site_prayers")
    }

    /// Whether `item`, already holding saved work or at the head of `cid`,
    /// finishes with this turn's Production.
    fn race_guard_finishes_this_turn(g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item)
            <= g.city_yields(cid).production
    }

    /// Each city's Prayers head before any production writer runs. Empty with
    /// the gene off or for another target.
    pub(super) fn race_guard_heads(&self, g: &Game, pid: usize) -> BTreeMap<u32, Item> {
        if !self.race_guard_active(g) {
            return BTreeMap::new();
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                g.cities[&cid]
                    .queue
                    .first()
                    .filter(|item| Self::race_guard_is_prayers(item))
                    .cloned()
                    .map(|item| (cid, item))
            })
            .collect()
    }

    fn race_guard_restore_prayers(&self, g: &mut Game, pid: usize, heads: &BTreeMap<u32, Item>) {
        for (cid, item) in heads {
            let Some(city) = g.cities.get(cid) else {
                continue;
            };
            if city.owner != pid
                || city.queue.first() == Some(item)
                || !g.can_produce(pid, *cid, item)
                || !Self::race_guard_finishes_this_turn(g, pid, *cid, item)
            {
                continue;
            }
            let displaced = city.queue.first().cloned();
            let name = city.name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: *cid,
                    item: item.clone(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Faith, Decision,
                    "{name} finishes its Prayers first";
                    "religion-seat-guards-the-race: it completes this turn, and {} follows next turn",
                    displaced.map_or_else(|| "nothing".to_string(), |head| Self::plain_item(&head)));
            }
        }
    }

    /// The repair of a pillaged Holy Site family district `cid` should take
    /// now. `None` when none is on offer, every site has a hostile soldier
    /// near it, the head is already a repair, or the city keeps its queue.
    pub(super) fn race_guard_repair(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<Item> {
        let city = &g.cities[&cid];
        if plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= RACE_REPAIR_ATTACK_MEMORY)
        {
            return None;
        }
        match city.queue.first() {
            Some(Item::Repair { .. }) => return None,
            Some(Item::Building { building })
                if building.as_str() == "walls"
                    || g.rules
                        .buildings
                        .get(building)
                        .is_some_and(|spec| spec.outer_defense > 0) =>
            {
                return None;
            }
            Some(head) if Self::race_guard_finishes_this_turn(g, pid, cid, head) => return None,
            _ => {}
        }
        g.producible_items(pid, cid).into_iter().find(|item| {
            let Item::Repair { repair, pos } = item else {
                return false;
            };
            *repair == "district"
                && g.map
                    .get(*pos)
                    .and_then(|tile| tile.district)
                    .is_some_and(|district| g.district_family(district) == "holy_site")
                && !Self::repair_site_threatened(g, pid, *pos)
        })
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn claim_religion_seat_guards_the_race(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        heads: &BTreeMap<u32, Item>,
    ) {
        if !self.race_guard_active(g) {
            return;
        }
        self.race_guard_restore_prayers(g, pid, heads);
        for cid in g.player_city_ids(pid) {
            let Some(repair) = self.race_guard_repair(g, pid, cid, plan) else {
                continue;
            };
            let displaced = g.cities[&cid].queue.first().cloned();
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: repair,
                },
            )
            .is_ok()
            {
                think!(self.journal(), Faith, Decision,
                    "{name} repairs its Holy Site";
                    "religion-seat-guards-the-race: ahead of {}",
                    displaced.map_or_else(|| "an idle queue".to_string(), |head| format!(
                        "{} (it keeps its progress)", Self::plain_item(&head))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
