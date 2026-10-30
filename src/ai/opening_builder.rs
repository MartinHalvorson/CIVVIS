//! `opening-builder-after-the-first-settler`: the capital's first Builder
//! follows its first Settler.
//!
//! Live Immortal seats, 2026-10-30 (54 games): the capital opened Scout,
//! Settler, slinger, Settler, and the first Builder came at a median turn 26.
//! At turn 40 5% of our owned land stood improved against the rivals' 26%
//! (farms 2% against 15%), and their population ran 1.6x ours from turn 20.
//! The rung hands every rival chair a free Settler and one or two Builders
//! (`Eras.xml`, `AiOnly` rows), so the gap opens before turn 10. The empire's
//! Builder steps size to unimproved WORKED tiles behind a due Settler, which
//! a two-city opening never satisfies.
use super::BasicAi;
use crate::game::{Action, Game, Item};
use crate::think;

impl BasicAi {
    /// Queue the opening Builder in `cid` when it is due; true when it was
    /// queued. `active_settlers` counts Settlers on the board, `builders`
    /// those held and queued.
    pub(super) fn play_opening_builder(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        active_settlers: usize,
        builders: usize,
    ) -> bool {
        if !self.opening_builder_after_the_first_settler
            || self.opening_builder_paid
            || builders > 0
            || (n_cities < 2 && active_settlers == 0)
            || !self.opening_builder_due(g, pid, cid)
        {
            return false;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        if g.apply(pid, &Action::Produce { city: cid, item: builder }).is_err() {
            return false;
        }
        self.opening_builder_paid = true;
        think!(self.journal, Cities, Decision,
               "{} trains the opening Builder", g.cities[&cid].name;
               "opening-builder-after-the-first-settler: the first Settler is out \
                ({n_cities} cities, {active_settlers} walking) and the empire has never held \
                a Builder; it comes ahead of the next Settler");
        true
    }

    /// Whether `cid` is our capital, at peace with every major, free of a
    /// barbarian answer or a siege, with land left to improve and a Builder
    /// it can train.
    fn opening_builder_due(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let city = &g.cities[&cid];
        if self.minor || self.barb || !city.is_capital || city.owner != pid {
            return false;
        }
        let at_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_barbarian
                && !player.is_minor
                && g.is_at_war(pid, player.id)
        });
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        !at_major_war
            && self.besieged_city_item(g, pid, cid).is_none()
            && self.barbarian_defense_item(g, pid, cid).is_none()
            && Self::has_builder_work(g, pid)
            && g.can_produce(pid, cid, &builder)
    }
}

#[cfg(test)]
mod tests;
