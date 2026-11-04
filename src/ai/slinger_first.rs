//! An opt-in first-slot support unit, independent of later recon replacement.
use super::BasicAi;
use crate::game::{Action, Game, Item};
use crate::think;

impl BasicAi {
    pub(super) fn play_slinger_first_opening(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
    ) -> bool {
        if !self.opening_ranged_support || self.minor || self.barb || self.book_pos != 0 {
            return false;
        }
        let city = &g.cities[&cid];
        if !city.is_capital || city.owner != pid || !city.queue.is_empty() {
            return false;
        }
        if g.player_city_ids(pid).len() != 1 {
            return false;
        }
        let home = city.pos;
        let units = g.player_unit_ids(pid);
        // Support a nearby land defender; a lone Slinger is not a substitute
        // for the first melee body. Do not duplicate support already fielded.
        let supported = units.iter().any(|uid| {
            let unit = &g.units[uid];
            let spec = &g.rules.units[&unit.kind];
            spec.class == "military"
                && spec.domain.as_deref().unwrap_or("land") == "land"
                && spec.promotion_class != "recon"
                && !g.is_embarked(unit)
                && spec.is_melee_capable()
                && g.wdist(home, unit.pos) <= 5
        });
        let has_ranged = units.iter().any(|uid| {
            let spec = &g.rules.units[&g.units[uid].kind];
            spec.class == "military" && spec.has_ranged_attack()
        });
        let item = Item::Unit {
            unit: crate::name!("slinger"),
        };
        if !supported || has_ranged || !g.can_produce(pid, cid, &item) {
            return false;
        }
        if g.apply(pid, &Action::Produce { city: cid, item }).is_err() {
            return false;
        }
        self.book_pos = 1;
        think!(self.journal, Cities, Decision,
               "{} opens with a Slinger", g.cities[&cid].name;
               "opening-ranged-support: a nearby melee defender gains ranged support before the first Settler; this replaces one opening slot without changing later recon settings");
        true
    }
}

#[cfg(test)]
mod tests;
