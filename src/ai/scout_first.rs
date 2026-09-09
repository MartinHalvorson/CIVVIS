//! An independent, bounded opening experiment: buy eyes before expansion.
use super::{BasicAi, UnitDoctrine};
use crate::game::{Action, Game, Item};
use crate::think;

impl BasicAi {
    pub(super) fn play_scout_first_opening(&mut self, g: &mut Game, pid: usize, cid: u32) -> bool {
        if !self.scout_first_opening || self.minor || self.barb || self.book_pos != 0 {
            return false;
        }
        let city = &g.cities[&cid];
        if !city.is_capital || city.owner != pid || !city.queue.is_empty() {
            return false;
        }
        let home = city.pos;
        // A first Scout must not substitute for the starting defender, or
        // duplicate recon already standing or ordered elsewhere.
        let has_defender = g.player_unit_ids(pid).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[&unit.kind];
            spec.class == "military"
                && spec.domain.as_deref().unwrap_or("land") == "land"
                && spec.promotion_class != "recon"
                && g.wdist(home, unit.pos) <= 5
        });
        let has_recon = g
            .player_unit_ids(pid)
            .into_iter()
            .any(|uid| Self::unit_doctrine(g, uid) == UnitDoctrine::Recon)
            || g.player_city_ids(pid).into_iter().any(|city| {
                g.cities[&city].queue.iter().any(|item| match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => g
                        .rules
                        .units
                        .get(unit)
                        .is_some_and(|spec| spec.promotion_class == "recon"),
                    _ => false,
                })
            });
        // Only observed pressure counts. Even a hostile Scout can interfere
        // with the first settlers, so do not exclude recon from this veto.
        let threatened = g.units.values().any(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.players[pid].turn_visible.contains(&unit.pos)
                && g.wdist(home, unit.pos) <= 5
                && g.rules.units[&unit.kind].class == "military"
        });
        // Ask about a known land frontier, never the terrain under the fog.
        let frontier = g.players[pid].explored.iter().any(|pos| {
            g.wdist(home, *pos) <= 5
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.nbrs(*pos).into_iter().any(|next| {
                    g.map.tiles.contains_key(&next) && !g.players[pid].explored.contains(&next)
                })
        });
        let item = Item::Unit {
            unit: crate::name!("scout"),
        };
        if !has_defender || has_recon || threatened || !frontier || !g.can_produce(pid, cid, &item)
        {
            return false;
        }
        if g.apply(pid, &Action::Produce { city: cid, item }).is_err() {
            return false;
        }
        self.book_pos = 1;
        think!(self.journal, Cities, Decision,
               "{} opens with a Scout", g.cities[&cid].name;
               "scout-first-opening: a nearby defender covers the capital, no hostile is visible within five tiles, and land frontier remains; this replaces the first opening slot");
        true
    }
}

#[cfg(test)]
mod tests;
