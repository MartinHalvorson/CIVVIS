//! `campus-buildings-first`: a city whose standing Campus can build its next
//! building -- the Library, else the University, else the Research Lab --
//! builds it before it opens another district or takes an ordinary building.
//! See `BasicAi::campus_buildings_first`.
//!
//! Live Emperor civvis-20261006T024058Z..T095649Z (35 games, 4p Tiny
//! Pangaea, Gran Colombia): per game, Campuses / Libraries / Universities
//! stood at 4 / 2 / 0 at t100, 5 / 4 / 1 at t125 and 7 / 6.5 / 3 at t150
//! (medians), and our Science per citizen was 1.25 against the rivals' 1.95
//! at t100. Education came at a median t91 against the rivals' t87, but a
//! University then waited: in 218 Campus cities holding a Library with
//! Education in hand it started a median 15 turns later (p75 32) and in 41 it
//! never started. Over those 6,471 city-turns the cities spent 17% on a new
//! district, 7% on another ordinary building and 4% on a repeatable district
//! project -- the picks this step outranks -- against 42% on soldiers and 12%
//! on Builders, Settlers and Traders, which it leaves alone. The Library
//! waited less (median 4 turns, p75 12), a third of that wait on new
//! districts, chiefly the culture-defense Theater (23% of the city-turns).
//! The Research Lab is held by the research pace instead (Chemistry at a
//! median t180 against the rivals' t153), which this step does not change.
use super::BasicAi;
use crate::game::{Game, Item};
use crate::think;

/// The slowest a city may build a Campus building under the step: a
/// University is 125 production and a Research Lab 220 at the live seat's
/// Online speed, so a city making fewer than about 6 (11) a turn keeps the
/// stock pick.
pub(crate) const CAMPUS_BUILDING_MAX_TURNS: f64 = 20.0;

impl BasicAi {
    /// The next building of the Campus `cid` holds -- the Library, else the
    /// University, else the Research Lab, each with its civilization's
    /// replacement -- the first the city can build now, when it finishes it
    /// within `CAMPUS_BUILDING_MAX_TURNS`. `None` for a city without a
    /// standing Campus.
    pub(crate) fn campus_building_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !g.city_has_district_family(g.cities.get(&cid)?, crate::name!("campus")) {
            return None;
        }
        let item = ["library", "university", "research_lab"]
            .into_iter()
            .find_map(|family| Self::civ_building(g, pid, cid, family))?;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= CAMPUS_BUILDING_MAX_TURNS).then_some(item)
    }

    /// `campus-buildings-first`: this city's next Campus building, at the
    /// step `BasicAi::pick_item` places ahead of `district-buildings-first`,
    /// the Harbor and the district list. `None` while the gene is off.
    pub(super) fn campus_buildings_first_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        if !self.campus_buildings_first || self.minor || self.barb {
            return None;
        }
        let item = Self::campus_building_item(g, pid, cid)?;
        think!(self.journal, Cities, Detail,
               "{} builds its Campus's {} before another district", g.cities[&cid].name,
               Self::item_label(&item);
               "campus-buildings-first: the standing Campus pays before the city opens a new \
                district or takes an ordinary building");
        Some(item)
    }

    /// `campus-buildings-first`: the Commercial Hub step's `item`, unless it
    /// is a new hub while the empire already holds or has queued one and this
    /// city can build its next Campus building; then that building, in the
    /// hub's slot. The empire's first hub keeps its slot.
    pub(super) fn campus_building_takes_the_hub_slot(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: Item,
    ) -> Item {
        if !self.campus_buildings_first
            || !matches!(&item, Item::District { district, .. }
                if g.district_family(*district) == "commercial_hub")
            || !Self::empire_holds_a_commercial_hub(g, pid)
        {
            return item;
        }
        match Self::campus_building_item(g, pid, cid) {
            Some(campus) => {
                think!(self.journal, Cities, Detail,
                       "{} builds its Campus's {} before another Commercial Hub",
                       g.cities[&cid].name, Self::item_label(&campus);
                       "campus-buildings-first: the empire already holds its first hub");
                campus
            }
            None => item,
        }
    }
}

#[cfg(test)]
mod tests;
