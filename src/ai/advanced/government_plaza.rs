//! `plaza-in-the-district-list`: the empire's first Government Plaza, then its
//! Warlord's Throne and Grand Master's Chapel, claimed at the first idle queue
//! ahead of the higher-level investment reservation.
//!
//! The governor's own plaza step (`BasicAi::pick_item`) is reached only when
//! every earlier step declines, and the capital's idle turns are taken first
//! by `reserve_higher_level_investment`. Live King
//! civvis-20261005T164323Z (game 144): the Plaza was buildable in Bogota from
//! turn 34; the capital's queue went Holy Site (the Prophet race), Settler,
//! Walls, Builder, then "Campus for research-building-catchup | one safe idle
//! queue services the development shortfall" at turn 48 and a Library at 64,
//! and no Plaza stood by turn 64. Without the Plaza there is no Warlord's
//! Throne and no Grand Master's Chapel, whose Faith purchase of land units is
//! the only use of the 1-3.5k Faith our games end on.

use super::{AdvancedAi, StrategicPlan};
use crate::ai::BasicAi;
use crate::game::{Action, Game, Item};
use crate::think;

impl AdvancedAi {
    /// One idle, unthreatened queue per turn: the standing Plaza's next tier
    /// building in its city, else the empire's first Plaza in the capital,
    /// else, while the capital is busy with a wonder or a Holy Site, in the
    /// idle city that produces most. An open Prophet race, a city due a
    /// Settler, a threatened or recently attacked city and an empire in
    /// economic recovery are all left alone.
    pub(super) fn reserve_government_plaza(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.plaza_in_the_district_list || self.base.minor || self.base.barb {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        // The economic-recovery bar `BasicAi::pick_item` uses.
        let recovery_reserve = 100.0 + 25.0 * city_ids.len() as f64;
        if g.players[pid].gold_per_turn < -0.5 && g.players[pid].gold < recovery_reserve {
            return;
        }
        let settlers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| g.units[unit].kind == "settler")
            .count()
            + city_ids
                .iter()
                .filter(|cid| {
                    matches!(g.cities[cid].queue.first(),
                        Some(Item::Unit { unit }) if unit == "settler")
                })
                .count();
        let free = |g: &Game, cid: u32| {
            let city = &g.cities[&cid];
            city.queue.is_empty()
                && plan.threatened_city != Some(cid)
                && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                && !self.base.settler_due(g, pid, cid, city_ids.len(), settlers)
        };
        // A standing Plaza's tier buildings, in the Plaza's own city.
        let claim = city_ids
            .iter()
            .copied()
            .filter(|cid| free(g, *cid))
            .find_map(|cid| BasicAi::plaza_building_item(g, pid, cid).map(|item| (cid, item)))
            .or_else(|| {
                if !BasicAi::plaza_absent(g, pid) || self.base.plaza_prophet_race_open(g, pid) {
                    return None;
                }
                let capital = city_ids.iter().copied().find(|cid| {
                    let city = &g.cities[cid];
                    city.is_capital && city.original_owner == pid
                })?;
                if free(g, capital) {
                    return BasicAi::plaza_site_item(g, pid, capital).map(|item| (capital, item));
                }
                // The capital is building a wonder or a Holy Site: the
                // idle city that produces most takes the Plaza instead.
                // `capital-campus-before-the-plaza`: so is its Campus.
                let capital_busy = self.capital_builds_its_campus(g, capital)
                    || match g.cities[&capital].queue.first() {
                        Some(Item::Building { building }) => g.rules.buildings[building].wonder,
                        Some(Item::District { district, .. }) => {
                            g.district_family(*district) == "holy_site"
                        }
                        _ => false,
                    };
                if !capital_busy {
                    return None;
                }
                city_ids
                    .iter()
                    .copied()
                    .filter(|cid| *cid != capital && free(g, *cid))
                    .filter_map(|cid| {
                        BasicAi::plaza_site_item(g, pid, cid)
                            .map(|item| (g.city_yields(cid).production, cid, item))
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
                    .map(|(_, cid, item)| (cid, item))
            });
        let Some((cid, item)) = claim else {
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
                "{} starts {} for plaza-in-the-district-list", g.cities[&cid].name, Self::plain_item(&item);
                "the Government Plaza and its Warlord's Throne and Grand Master's Chapel take one idle queue ahead of the development shortfall");
        }
    }
}

#[cfg(test)]
mod tests;
