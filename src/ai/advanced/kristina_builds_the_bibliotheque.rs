//! `kristina-builds-the-bibliotheque`: Sweden's Government Plaza holds the
//! Queen's Bibliotheque.
//!
//! **The rule.** `DLC/Expansion2/Data/Expansion2_Buildings_Major.xml`:
//! `BUILDING_QUEENS_BIBLIOTHEQUE` (Kristina's
//! `TRAIT_LEADER_BUILDING_QUEENS_BIBLIOTHEQUE`; Sweden has no other leader)
//! stands in `DISTRICT_GOVERNMENT`, needs a Tier2 government and one of
//! `BUILDING_GOV_TALL`, `BUILDING_GOV_WIDE` or `BUILDING_GOV_CONQUEST`, and
//! excludes `BUILDING_GOV_CITYSTATES`, `BUILDING_GOV_SPIES` and
//! `BUILDING_GOV_FAITH`. It holds two Writing, two Art and two Music slots and
//! earns 2 Great Writer, Artist and Musician points a turn. Under Minerva of
//! the North (`TRAIT_LEADER_KRISTINA_AUTO_THEME`) a building with three or
//! more slots themes itself when full.
//!
//! **Why it was never built.** `BasicAi::plaza_building_item_for` offers only
//! the Warlord's Throne then the Grand Master's Chapel, or the Ancestral Hall
//! then the Intelligence Agency; the Chapel and the Agency exclude the
//! Bibliotheque, so once either stood it could never be built. And the
//! empire's first Plaza waits for an open Prophet race and only ever goes in
//! the capital. Kristina Deity `civvis-20261110T140731Z`: the Plaza was
//! buildable in Stockholm t40-85 while the Prophet race held it back (Holy
//! Sites started t88-90); from t90 Stockholm had no district slot left, and no
//! Plaza stood by t150. A Great Musician idled t111-150 and an Artist t124-150
//! for want of a slot.
//!
//! **The gene.** On Sweden's seat the Plaza's tier buildings are the Ancestral
//! Hall (no drawback; a free Builder for every new city) and then the
//! Bibliotheque. The first Plaza does not wait for the Prophet race, and when
//! the capital has no site for it, any idle original city of ours that has one
//! takes it. Every other seat, and Sweden with the gene off, is unchanged.

use crate::ai::BasicAi;
use crate::game::{Game, Item};

/// Sweden's Plaza tiers, in order. A Plaza already holding the Audience
/// Chamber or the Warlord's Throne cannot take the Hall and goes straight to
/// the Bibliotheque.
const SWEDEN_PLAZA_TIERS: [&str; 2] = ["ancestral_hall", "queens_bibliotheque"];

impl BasicAi {
    /// Whether the gene is on and `pid` plays Sweden.
    pub(crate) fn kristina_builds_here(&self, g: &Game, pid: usize) -> bool {
        self.kristina_builds_the_bibliotheque
            && g.players
                .get(pid)
                .is_some_and(|player| player.civ == "Sweden")
    }

    /// The standing Plaza's next tier building in `cid`: Sweden's tiers under
    /// the gene, else [`BasicAi::plaza_building_item_for`].
    pub(crate) fn plaza_tier_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.kristina_builds_here(g, pid) {
            return Self::plaza_building_item_for(g, pid, cid, self.plaza_serves_science);
        }
        if !g.city_has_district_family(&g.cities[&cid], crate::name!("government_plaza")) {
            return None;
        }
        SWEDEN_PLAZA_TIERS
            .into_iter()
            .find_map(|building| Self::civ_building(g, pid, cid, building))
    }

    /// Whether an open Prophet race holds the first Plaza back. Never on
    /// Sweden's seat under the gene.
    pub(crate) fn plaza_waits_for_the_prophet(&self, g: &Game, pid: usize) -> bool {
        !self.kristina_builds_here(g, pid) && self.plaza_prophet_race_open(g, pid)
    }

    /// Whether `pid`'s original capital has no site for a Government Plaza
    /// (or no longer stands).
    pub(crate) fn capital_cannot_hold_the_plaza(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .find(|cid| {
                let city = &g.cities[cid];
                city.is_capital && city.original_owner == pid
            })
            .is_none_or(|capital| Self::plaza_site_item(g, pid, capital).is_none())
    }

    /// The empire's first Plaza for `cid`: [`BasicAi::plaza_item`], and on
    /// Sweden's seat under the gene any original city of ours when the capital
    /// has no site for one.
    pub(crate) fn plaza_item_for_seat(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if let Some(item) = Self::plaza_item(g, pid, cid) {
            return Some(item);
        }
        if !self.kristina_builds_here(g, pid) {
            return None;
        }
        let city = &g.cities[&cid];
        if city.owner != pid
            || city.is_capital
            || city.original_owner != pid
            || !Self::plaza_absent(g, pid)
            || !Self::capital_cannot_hold_the_plaza(g, pid)
        {
            return None;
        }
        Self::plaza_site_item(g, pid, cid)
    }
}

#[cfg(test)]
mod tests;
