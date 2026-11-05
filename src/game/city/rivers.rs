//! Rivers: the edges a river runs along, the components they form, and dams.
//!
//! The canonical form of a river edge and the edges at a tile, the
//! connected component a river belongs to and whether it already has a
//! dam, whether a dam has a river available (`dam_has_available_river`),
//! and the pantheon's river Holy Site check
//! (`pantheon_river_holy_site`). Generating rivers is in
//! `mapgen/rivers.rs`.
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    /// What River Goddess pays a district standing where the belief asks.
    ///
    /// ★ One predicate for both halves of the belief: Gathering Storm ships
    /// `RIVER_GODDESS_HOLY_SITE_AMENITIES` and `RIVER_GODDESS_HOLY_SITE_HOUSING`
    /// as two modifiers with the SAME subject requirement set,
    /// `PLOT_HAS_HOLY_SITE_RIVER_REQUIREMENTS` (`REQUIRES_PLOT_HAS_HOLY_SITE`
    /// and `REQUIRES_PLOT_ADJACENT_TO_RIVER`, tested ALL), so the plot test
    /// lives here once and the caller names which yield it is collecting.
    ///
    /// ⚠ `Expansion2_RemoveData.xml` DELETES the base game's
    /// `RIVER_GODDESS_HOLY_SITE_AMENITY` — a different id, +1 Amenity through
    /// `MODIFIER_CITY_DISTRICTS_ADJUST_CITY_AMENITIES_FROM_RELIGION` and no
    /// Housing at all. Reading the base row would underpay the Amenity and
    /// miss the Housing outright.
    pub(in crate::game) fn pantheon_river_holy_site(
        &self,
        district: &str,
        position: Pos,
        effect: &str,
    ) -> f64 {
        // ⚠ River first, and the district family last. This is asked once per
        // district per city on every Housing and Amenity read, and
        // `Name::new` takes a read lock on the global name registry — the
        // plot's own six river edges settle it for almost every caller
        // without interning anything.
        let Some(tile) = self.map.get(position) else {
            return 0.0;
        };
        if !tile.has_river() {
            return 0.0;
        }
        let Some(amount) = tile
            .owner_city
            .and_then(|city_id| self.cities.get(&city_id))
            .map(|city| self.pantheon_effect(city.owner, effect))
            .filter(|amount| *amount != 0.0)
        else {
            return 0.0;
        };
        if !self.district_is_family(Name::new(district), crate::name!("holy_site")) {
            return 0.0;
        }
        amount
    }

    pub(in crate::game) fn canonical_river_edge(a: Pos, b: Pos) -> (Pos, Pos) {
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    }

    pub(in crate::game) fn river_edges_at(&self, position: Pos) -> BTreeSet<(Pos, Pos)> {
        self.nbrs(position)
            .into_iter()
            .filter(|neighbor| self.map.has_river_edge(position, *neighbor))
            .map(|neighbor| Self::canonical_river_edge(position, neighbor))
            .collect()
    }

    /// River segments meet at the vertices of their shared hex edges. The
    /// generated map stores only edge masks, so recovering this connected
    /// component is the save-compatible equivalent of Civ VI's named river
    /// identity for one-Dam-per-river placement.
    pub(in crate::game) fn river_component(&self, origin: (Pos, Pos)) -> BTreeSet<(Pos, Pos)> {
        if !self.map.has_river_edge(origin.0, origin.1) {
            return BTreeSet::new();
        }
        let mut component = BTreeSet::from([origin]);
        let mut frontier = VecDeque::from([origin]);
        while let Some((a, b)) = frontier.pop_front() {
            let b_neighbors: BTreeSet<Pos> = self.nbrs(b).into_iter().collect();
            for common in self
                .nbrs(a)
                .into_iter()
                .filter(|position| *position != b && b_neighbors.contains(position))
            {
                for next in [
                    Self::canonical_river_edge(a, common),
                    Self::canonical_river_edge(b, common),
                ] {
                    if self.map.has_river_edge(next.0, next.1) && component.insert(next) {
                        frontier.push_back(next);
                    }
                }
            }
        }
        component
    }

    pub(in crate::game) fn river_component_has_dam(
        &self,
        component: &BTreeSet<(Pos, Pos)>,
    ) -> bool {
        self.map.tiles.iter().any(|(position, tile)| {
            let is_dam = tile
                .district
                .is_some_and(|district| self.district_is_family(district, crate::name!("dam")))
                || tile.district_foundation.as_ref().is_some_and(|foundation| {
                    self.district_is_family(foundation.district, crate::name!("dam"))
                });
            is_dam
                && self
                    .river_edges_at(*position)
                    .iter()
                    .any(|edge| component.contains(edge))
        })
    }

    /// A Dam needs two edges belonging to the same river, and that river is
    /// reserved as soon as another Dam foundation is placed anywhere on it.
    pub(in crate::game) fn dam_has_available_river(&self, position: Pos) -> bool {
        let incident = self.river_edges_at(position);
        let mut examined = BTreeSet::new();
        for origin in incident.iter().copied() {
            if examined.contains(&origin) {
                continue;
            }
            let component = self.river_component(origin);
            examined.extend(component.iter().copied());
            if incident
                .iter()
                .filter(|edge| component.contains(edge))
                .count()
                >= 2
                && !self.river_component_has_dam(&component)
            {
                return true;
            }
        }
        false
    }
}
