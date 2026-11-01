//! `launch-pad-keeps-its-rung`, the production governor's half: an idle city
//! resumes its own placed, unfinished Spaceport while no pad of ours stands.
//!
//! The live seat's ordinary production goes through `BasicAi::pick_item`
//! (`lane-delegates-production-2`), where the Spaceport is a late fallback
//! behind every industry, Commercial Hub and Trader step, and whose "a pad is
//! on its way" test reads only a finished pad or one at the head of the
//! queue. Civilization VI keeps a placed district as a map foundation once a
//! claim borrows the queue, so after one Spy or Trader the pad's own city
//! went on to Markets, Banks and Stock Exchanges. Live Emperor
//! civvis-20261101T043921Z: St. Petersburg placed the pad at t161 and finished
//! it at t193, 8 of those 32 turns on it, resumed only through the Great
//! Person activation path.
use super::BasicAi;
use crate::game::{Game, Item};

impl BasicAi {
    /// The District order that resumes `cid`'s own placed Spaceport
    /// foundation, while no pad of ours stands and the seat runs the space
    /// race. `None` when the gene is off, for a Domination seat, once any pad
    /// stands, or when this city holds no Spaceport foundation.
    pub(super) fn launch_pad_resume_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.launch_pad_resumes_its_foundation
            || self.minor
            || self.barb
            || self.exclude_space_race
        {
            return None;
        }
        let pad = crate::name!("spaceport");
        if g.player_city_ids(pid)
            .into_iter()
            .any(|city| g.city_has_district_family(&g.cities[&city], pad))
        {
            return None;
        }
        let (district, pos) = g.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .find_map(|position| {
                let foundation = g.map.tiles.get(&position)?.district_foundation.as_ref()?;
                (g.district_family(foundation.district) == pad)
                    .then_some((foundation.district, position))
            })?;
        let item = Item::District { district, pos };
        g.can_produce(pid, cid, &item).then_some(item)
    }
}

#[cfg(test)]
mod tests;
