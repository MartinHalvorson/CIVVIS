//! `kristina-fills-to-theme`: Sweden's Great Works go where Minerva of the
//! North themes them.
//!
//! **The rule.** `DLC/Expansion2/Data/Expansion2_Leaders_Major.xml`:
//! `TRAIT_LEADER_KRISTINA_AUTO_THEME` carries `AUTO_THEME_AT_LEAST_3_SLOTS`
//! and `AUTO_THEME_AT_LEAST_2_SLOTS` — a building with three or more Great
//! Work slots (a wonder with two or more) themes itself once every slot is
//! full, whatever the works are — and theming doubles the Culture and Tourism
//! of the works it holds (`Buildings.xml` `ThemingYieldMultiplier` 100).
//!
//! **What went wrong.** Kristina Deity `civvis-20261110T140731Z`: a Great
//! Musician idled t111-150 and an Artist t124-150 for want of any slot, while
//! the only 3-slot building standing (Stockholm's Art Museum) was already
//! full. The Queen's Bibliotheque (2 Writing, 2 Art, 2 Music) was never built.
//!
//! **The gene**, on Sweden's seat only:
//! 1. A Writer, Artist or Musician the host reports without a slot asks for
//!    the Queen's Bibliotheque before the stock chain (Amphitheater, Art
//!    Museum, Broadcast Center), and for what makes it buildable (a Plaza, then
//!    the Ancestral Hall): it is the empire's earliest Music slot and a
//!    six-slot building that themes when full.
//! 2. A cultural person walking to an activation plot prefers the plot whose
//!    3+-slot building is closest to full, and one standing able to activate
//!    into a building that can never theme (the Palace, an Amphitheater)
//!    walks instead to a nearby plot that can, so the works fill the theming
//!    buildings rather than splitting across them. See
//!    `bin/civvis_orders/kristina_fills_to_theme.rs`.
//! 3. The stock Art Museum swaps stand down: they chase the stock theme
//!    (three works of one kind by three artists), which Kristina gets from
//!    any full museum, and the host re-scores every swap by the stock rule,
//!    so a swap that only fills a museum is refused anyway.
//!
//! With the gene off, or on any other seat, nothing changes.

use super::AdvancedAi;
use crate::ai::BasicAi;
use crate::game::{Game, Item};
use crate::mirror::StateSnapshot;

/// The Great Work slots of the buildings a cultural person can fill, by kind
/// (`Buildings.xml` / `Expansion2_Buildings_Major.xml`
/// `Building_GreatWorks`): `(building, [(kind, slots)])`.
const SLOT_BUILDINGS: [(&str, &[(&str, u32)]); 6] = [
    ("BUILDING_PALACE", &[("ANY", 1)]),
    ("BUILDING_AMPHITHEATER", &[("WRITING", 2)]),
    ("BUILDING_MUSEUM_ART", &[("ART", 3)]),
    ("BUILDING_MUSEUM_ARTIFACT", &[("ARTIFACT", 3)]),
    ("BUILDING_BROADCAST_CENTER", &[("MUSIC", 1)]),
    (
        "BUILDING_QUEENS_BIBLIOTHEQUE",
        &[("WRITING", 2), ("ART", 2), ("MUSIC", 2)],
    ),
];

/// Which buildings stand in each kind of district.
fn district_buildings(district: &str) -> &'static [&'static str] {
    match district {
        "DISTRICT_CITY_CENTER" => &["BUILDING_PALACE"],
        "DISTRICT_GOVERNMENT" => &["BUILDING_QUEENS_BIBLIOTHEQUE"],
        "DISTRICT_THEATER" | "DISTRICT_ACROPOLIS" => &[
            "BUILDING_AMPHITHEATER",
            "BUILDING_MUSEUM_ART",
            "BUILDING_MUSEUM_ARTIFACT",
            "BUILDING_BROADCAST_CENTER",
        ],
        _ => &[],
    }
}

/// The kind of work a Great Person class makes.
fn work_kind(class: &str) -> Option<&'static str> {
    match class {
        "GREAT_PERSON_CLASS_WRITER" => Some("WRITING"),
        "GREAT_PERSON_CLASS_ARTIST" => Some("ART"),
        "GREAT_PERSON_CLASS_MUSICIAN" => Some("MUSIC"),
        _ => None,
    }
}

impl AdvancedAi {
    /// The rank a plot gets when nothing on it themes.
    pub const KRISTINA_NO_THEME: u32 = 99;

    /// `kristina-fills-to-theme` is on and `pid` plays Sweden.
    pub fn kristina_fills_to_theme_on(&self, g: &Game, pid: usize) -> bool {
        self.base.kristina_fills_here(g, pid)
    }

    /// How close a work of `class` placed at plot `(x, y)` brings a building
    /// that Kristina themes to full: the empty slots that building would
    /// still have, lowest first. [`Self::KRISTINA_NO_THEME`] when the plot holds no
    /// building with three or more slots and a free one of this kind (a
    /// Palace, an Amphitheater, a wonder, or an unknown plot).
    pub fn kristina_theme_rank(state: &StateSnapshot, class: &str, x: i32, y: i32) -> u32 {
        let Some(kind) = work_kind(class) else {
            return Self::KRISTINA_NO_THEME;
        };
        let mut best = Self::KRISTINA_NO_THEME;
        for city in &state.cities {
            let Some(works) = city.great_works.as_ref() else {
                continue;
            };
            for district in city.districts.iter().filter(|d| d.x == x && d.y == y) {
                for building in district_buildings(&district.kind) {
                    if !city.buildings.iter().any(|b| b == building) {
                        continue;
                    }
                    let Some((_, slots)) = SLOT_BUILDINGS.iter().find(|(name, _)| name == building)
                    else {
                        continue;
                    };
                    let total: u32 = slots.iter().map(|(_, n)| n).sum();
                    let held = works.iter().filter(|w| &w.building == building).count() as u32;
                    let of_kind: u32 = slots
                        .iter()
                        .filter(|(k, _)| *k == kind || *k == "ANY")
                        .map(|(_, n)| n)
                        .sum();
                    let held_of_kind = works
                        .iter()
                        .filter(|w| &w.building == building && work_matches(&w.object, kind))
                        .count() as u32;
                    if total < 3 || held_of_kind >= of_kind || held >= total {
                        continue;
                    }
                    best = best.min(total - held - 1);
                }
            }
        }
        best
    }
}

/// Whether a Great Work object belongs to a slot kind.
fn work_matches(object: &str, kind: &str) -> bool {
    match kind {
        "WRITING" => object == "GREATWORKOBJECT_WRITING",
        "MUSIC" => object == "GREATWORKOBJECT_MUSIC",
        "ART" => matches!(
            object,
            "GREATWORKOBJECT_SCULPTURE"
                | "GREATWORKOBJECT_LANDSCAPE"
                | "GREATWORKOBJECT_PORTRAIT"
                | "GREATWORKOBJECT_RELIGIOUS"
        ),
        _ => false,
    }
}

impl BasicAi {
    /// Whether the gene is on and `pid` plays Sweden.
    pub(crate) fn kristina_fills_here(&self, g: &Game, pid: usize) -> bool {
        self.kristina_fills_to_theme
            && g.players
                .get(pid)
                .is_some_and(|player| player.civ == "Sweden")
    }

    /// The Bibliotheque, or the step that makes it buildable, for a Writer,
    /// Artist or Musician with no slot: the Bibliotheque itself; else, in a
    /// city whose Plaza has no tier-one building, the Ancestral Hall; else,
    /// with no Plaza anywhere, a Plaza in this city. `None` leaves the stock
    /// chain to answer (and is the answer once a Plaza building that excludes
    /// the Bibliotheque stands); `Some(None)` means another city is the one to
    /// build it, or already has it queued.
    pub(crate) fn kristina_slot_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        work: &str,
    ) -> Option<Option<Item>> {
        if !self.kristina_fills_here(g, pid) || !matches!(work, "writing" | "art" | "music") {
            return None;
        }
        let city_ids = g.player_city_ids(pid);
        let steps = [
            crate::name!("queens_bibliotheque"),
            crate::name!("ancestral_hall"),
        ];
        let queued = city_ids.iter().any(|city| {
            matches!(g.cities[city].queue.first(),
                Some(Item::Building { building }) if steps.contains(building))
        });
        if queued {
            return Some(None);
        }
        if let Some(item) = Self::civ_building(g, pid, cid, "queens_bibliotheque") {
            return Some(Some(item));
        }
        if city_ids
            .iter()
            .any(|city| Self::civ_building(g, pid, *city, "queens_bibliotheque").is_some())
        {
            return Some(None);
        }
        let plaza = crate::name!("government_plaza");
        let Some(plaza_city) = city_ids
            .iter()
            .copied()
            .find(|city| g.city_has_district_family(&g.cities[city], plaza))
        else {
            // No Plaza: this city founds one if it can, while none is queued.
            if !Self::plaza_absent(g, pid) {
                return Some(None);
            }
            return Self::plaza_site_item(g, pid, cid).map(Some);
        };
        let held = &g.cities[&plaza_city].buildings;
        let excluded = [
            "grand_masters_chapel",
            "intelligence_agency",
            "foreign_ministry",
        ]
        .iter()
        .any(|building| held.contains(&crate::name::Name::new(building)));
        let tier_one = ["ancestral_hall", "audience_chamber", "warlords_throne"]
            .iter()
            .any(|building| held.contains(&crate::name::Name::new(building)));
        if excluded || tier_one {
            // The Bibliotheque is ruled out, or waits on the host (a Tier2
            // government): the stock chain answers.
            return None;
        }
        if plaza_city != cid {
            return Some(None);
        }
        Self::civ_building(g, pid, cid, "ancestral_hall").map(Some)
    }
}

#[cfg(test)]
mod tests;
