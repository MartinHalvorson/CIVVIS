//! `prophet-race-earns-its-points`: while a Great Prophet is still open to
//! this seat, the race's Holy Site city builds its Shrine at once and
//! Revelation takes the wildcard slot.
//!
//! A Tiny map has three Prophets. The seat's only Prophet points were its
//! Holy Site's one a turn, so a founding came 27-30 turns after the site
//! finished, and the Shrine (+1) came after the Prophet or after the race had
//! closed in every one of the 78 finished control games of October 4-5. Live
//! King civvis-20261005T173504Z (game 147): Holy Site finished at t37,
//! Shrine buildable for 35 production at 14 a turn, the last Prophet taken by
//! Scythia at t56 (our site had 19 of 30 points), Shrine built at t129, lost
//! to Scythia's religion at t192. Game 110504 (site t41, race closed t56) had
//! Revelation on offer from t40 to t55 under Oligarchy and never slotted it:
//! the deck prices a Great Person card at zero. The census: 41 founding games
//! lost to a religion once, 37 without a religion 12 times.

use super::{AdvancedAi, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::think;

impl AdvancedAi {
    /// Whether the race has a Prophet left for this seat to earn: no religion
    /// and no Prophet in hand, a slot open, a lane that admits the race, and
    /// the race not called off (`skip_prophet_race`).
    pub(super) fn race_points_wanted(&self, g: &Game, pid: usize) -> bool {
        if !self.prophet_race_earns_its_points || self.base.minor || self.base.barb {
            return false;
        }
        let player = &g.players[pid];
        if player.religion.is_some() || player.prophet_pending || self.base.skip_prophet_race {
            return false;
        }
        let target = self.active_victory_target(g);
        // The same lane gate as `take_turn_inner`'s `prophet_race_enabled`.
        let lane_admits = self.prophet_race_enabled_for(target)
            || (self.found_against_a_rival_faith && target == Some(VictoryTarget::Domination));
        lane_admits && self.prophet_race_open_for(g, pid)
    }

    fn race_shrine_item() -> Item {
        Item::Building {
            building: crate::name!("shrine"),
        }
    }

    fn city_has_shrine(g: &Game, cid: u32) -> bool {
        g.cities[&cid]
            .buildings
            .iter()
            .any(|building| building.as_str() == "shrine")
    }

    /// The Holy Site city whose Shrine the race wants now: one at a time, the
    /// city that produces most among those with a site and no Shrine.
    pub(super) fn race_shrine_city(&self, g: &Game, pid: usize) -> Option<u32> {
        if !self.race_points_wanted(g, pid) {
            return None;
        }
        let item = Self::race_shrine_item();
        let cities = g.player_city_ids(pid);
        if cities
            .iter()
            .any(|cid| g.cities[cid].queue.first() == Some(&item))
        {
            return None;
        }
        cities
            .into_iter()
            .filter(|cid| {
                g.city_has_district_family(&g.cities[cid], crate::name!("holy_site"))
                    && !Self::city_has_shrine(g, *cid)
                    && g.can_produce(pid, *cid, &item)
            })
            .map(|cid| (g.city_yields(cid).production, cid))
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, cid)| cid)
    }

    /// The governor keeps the race's Shrine through rescoring while the race
    /// still wants it. See `advanced_production`.
    pub(super) fn race_shrine_committed(&self, g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        *item == Self::race_shrine_item()
            && self.race_points_wanted(g, pid)
            && g.city_has_district_family(&g.cities[&cid], crate::name!("holy_site"))
            && !Self::city_has_shrine(g, cid)
    }

    /// Put the race's Shrine at the front of its Holy Site city's queue. A
    /// threatened or recently attacked city, a district or wonder under
    /// construction, and an item finishing this turn keep the queue.
    pub(super) fn reserve_race_shrine(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(cid) = self.race_shrine_city(g, pid) else {
            return;
        };
        let city = &g.cities[&cid];
        if plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return;
        }
        let displaced = city.queue.first().cloned();
        match &displaced {
            Some(Item::District { .. }) => return,
            Some(Item::Building { building }) if g.rules.buildings[building].wonder => return,
            Some(item) => {
                let remaining =
                    g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
                if remaining <= g.city_yields(cid).production {
                    return;
                }
            }
            None => {}
        }
        let item = Self::race_shrine_item();
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
            let claimed = g.religions_founded()
                + g.players
                    .iter()
                    .filter(|player| player.prophet_pending)
                    .count();
            think!(self.journal(), Faith, Decision,
                "{} starts its Shrine for the Great Prophet race", g.cities[&cid].name;
                "prophet-race-earns-its-points: {} of {} Prophets claimed; the Shrine doubles the \
                 Holy Site's point a turn{}",
                claimed, g.max_religions(),
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }

    /// `prophet-race-earns-its-points`: Revelation's +2 Prophet points a turn
    /// while the race has a Prophet for this seat, Holy Site or not.
    pub(super) fn race_wants_revelation(&self, g: &Game, pid: usize) -> bool {
        self.race_points_wanted(g, pid) && g.rules.policies.contains_key("revelation")
    }
}

#[cfg(test)]
mod tests;
