//! `religion-seat-first-shrine`: a Religion-target founder's first Shrine
//! takes the queue the turn it is founded.
//!
//! **Why (live Deity Russia, 2026-11-09).** A Missionary can be bought only
//! in a city with a Shrine, so without one a founder's Faith has nothing to
//! buy. `civvis-20261109T141812Z` founded at t37 and never ordered a Shrine.
//! Its capital built an Archer, then a Settler from t45 to t60. Islam held
//! the capital by t50 and every other city by t60, and the seat was
//! eliminated at t99 holding 859 unspent Faith. `civvis-20261109T140800Z`
//! founded at t24 and ended at t150 with no Shrine and 866 Faith.
//!
//! `religion-seat-builds-holy-sites` could not reach either. It gives way to
//! any unit at the queue head, and it skips a city that no longer follows the
//! seat's faith, which is the state the missing Missionaries let happen.
//!
//! **What the gene does.** Once founded, with no Shrine standing or queued
//! anywhere, the Holy Site city that can build one takes it at the head of
//! its queue, ahead of a Settler or any other routine item. A city following
//! our faith goes first, then the one that produces most. The displaced item
//! keeps its progress. A head finishing this turn, Walls, a city under
//! threat and a city attacked in the last four turns all keep their queue.
//!
//! Off, for another target, or before the seat has founded, nothing changes.

use super::*;

/// A city attacked this recently keeps its queue.
const FIRST_SHRINE_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    fn first_shrine_applies(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_first_shrine
            && !self.base.minor
            && !self.base.barb
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }

    fn city_queues_a_shrine(g: &Game, city: &crate::game::City) -> bool {
        city.queue.iter().any(|item| match item {
            Item::Building { building } => {
                building.as_str() == "shrine"
                    || g.rules
                        .buildings
                        .get(building)
                        .and_then(|spec| spec.replaces.as_deref())
                        == Some("shrine")
            }
            _ => false,
        })
    }

    /// Whether `cid`'s queue head may give way to the Shrine.
    fn first_shrine_may_displace(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(head) = g.cities[&cid].queue.first() else {
            return true;
        };
        if let Item::Building { building } = head {
            let walls = g.rules.buildings.get(building).is_some_and(|spec| {
                spec.outer_defense > 0 || spec.replaces.as_deref() == Some("walls")
            });
            if building.as_str() == "walls" || walls {
                return false;
            }
        }
        let remaining =
            g.item_cost_for_city(pid, cid, head) - g.item_invested_production(cid, head);
        remaining > g.city_yields(cid).production
    }

    /// The city and item for the first Shrine. `None` when the gene does not
    /// apply, when a Shrine stands or is queued anywhere, or when no Holy Site
    /// city can take one.
    pub(super) fn first_shrine_choice(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.first_shrine_applies(g, pid) {
            return None;
        }
        let religion = g.players[pid].religion.clone()?;
        let cities = g.player_city_ids(pid);
        if cities.iter().any(|cid| {
            let city = &g.cities[cid];
            Self::city_holds_family(g, city, "shrine") || Self::city_queues_a_shrine(g, city)
        }) {
            return None;
        }
        cities
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                if !g.city_has_district_family(city, crate::name!("holy_site"))
                    || plan.threatened_city == Some(cid)
                    || (city.last_attacked > 0
                        && g.turn.saturating_sub(city.last_attacked) <= FIRST_SHRINE_ATTACK_MEMORY)
                    || !Self::first_shrine_may_displace(g, pid, cid)
                {
                    return None;
                }
                let item = BasicAi::civ_building(g, pid, cid, "shrine")?;
                if !g.can_produce(pid, cid, &item) {
                    return None;
                }
                let follows = g.city_religion(city) == Some(religion.as_str());
                Some((follows, g.city_yields(cid).production, cid, item))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(b.2.cmp(&a.2)))
            .map(|(_, _, cid, item)| (cid, item))
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn claim_religion_seat_first_shrine(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((cid, item)) = self.first_shrine_choice(g, pid, plan) else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Faith, Decision,
                "{name} starts its first Shrine";
                "religion-seat-first-shrine: no Missionary can be bought without one, ahead of {}",
                displaced.map_or_else(|| "an idle queue".to_string(), |head| format!(
                    "{} (it keeps its progress)", Self::plain_item(&head))));
        }
    }
}

#[cfg(test)]
mod tests;
