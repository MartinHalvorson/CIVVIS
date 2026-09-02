//! Reserve a city's third ring for science water wonders it can actually use.
//!
//! The ordinary settlement forecast deliberately works only rings one and
//! two: its four citizens need jobs now, rather than an optimistic list of
//! every plot a city may someday own. That otherwise leaves a precise blind
//! spot for Galápagos and the Bermuda Triangle. Civ VI lets a city work a
//! third-ring plot, and `district-planning-2` can already buy an exceptional
//! Science plot (and its one-tile connector). A coastal site whose third ring
//! contains those known, unowned plots should therefore survive the shortlist
//! and beat a similarly fertile site that cannot ever acquire them.
//!
//! This remains narrower than a general water or wonder bonus:
//!
//! - only passable water plots in the prospective city's third ring count;
//! - only Science projected by a *water* natural wonder counts, never a
//!   wonder's own tile yield or a generic coast yield;
//! - the plot and its projecting wonder must be explored, so the scorer does
//!   not turn native-map knowledge into fog-of-war knowledge;
//! - at most the first four citizen jobs are reserved, matching the normal
//!   settlement forecast.
//!
//! The credit is one normal settlement-yield valuation per future job. It is
//! intentionally smaller than the full forty-turn forecast paid to the same
//! tile in rings one and two, which makes the closest workable site win while
//! still recognizing a durable, purchasable third-ring science asset.

use super::{AdvancedAi, SETTLEMENT_FORECAST_POPULATION};
use crate::game::Game;
use crate::rules::Yields;
use crate::world::Tile;
use crate::Pos;

/// Civilization VI's outer citizen-work ring.
const CITY_WORK_RADIUS: i32 = 3;

impl AdvancedAi {
    /// Science a known water natural wonder projects onto `pos`. Galápagos
    /// projects +2 Science from each of its two impassable Coast tiles;
    /// Bermuda's passable Ocean tiles project +5 Science from each adjacent
    /// Triangle tile. Reading the feature's `adjacent_yields` is the same rule
    /// `Game::ground_tile_yields` uses for a worked plot, restricted here to
    /// the water-wonder Science that a new city needs to acquire.
    fn known_water_wonder_science(g: &Game, pid: usize, pos: Pos) -> f64 {
        g.nbrs(pos)
            .into_iter()
            .filter(|neighbor| g.players[pid].explored.contains(neighbor))
            .filter_map(|neighbor| {
                let tile = g.map.get(neighbor)?;
                (g.rules.is_water(tile) && g.tile_is_natural_wonder(tile))
                    .then_some(tile.feature.as_deref()?)
            })
            .map(|feature| g.rules.features[feature].adjacent_yields.science)
            .sum()
    }

    /// Whether a prospective city can eventually work and buy this water
    /// plot. Keep this in lockstep with the ordinary forecast's work-tile
    /// exclusions, plus the water requirement specific to this policy.
    fn is_unowned_workable_water_tile(g: &Game, tile: &Tile) -> bool {
        tile.owner_city.is_none()
            && g.rules.is_water(tile)
            && g.rules.is_passable(tile)
            && tile.district.is_none()
            && tile.district_foundation.is_none()
            && tile.wonder.is_none()
            && !tile.improvement.as_deref().is_some_and(|improvement| {
                g.rules.improvements[improvement]
                    .effects
                    .get("unworkable")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
            })
    }

    /// A founded city owns its first ring, so a known, neutral second-ring
    /// neighbour is the exact one-plot bridge `plot_purchase_cost` needs to
    /// expose a third-ring tile. Do not reserve science beyond a foreign or
    /// already-owned border that this new city cannot buy through.
    fn has_known_ring_two_purchase_bridge(
        g: &Game,
        pid: usize,
        city_pos: Pos,
        target: Pos,
    ) -> bool {
        if !g.annexes_tiles_with_own_yields(pid) {
            return false;
        }
        g.nbrs(target).into_iter().any(|bridge| {
            g.wdist(city_pos, bridge) == CITY_WORK_RADIUS - 1
                && g.players[pid].explored.contains(&bridge)
                && g.map
                    .get(bridge)
                    .is_some_and(|tile| tile.owner_city.is_none())
        })
    }

    /// The strongest known water-wonder Science jobs just beyond the ordinary
    /// two-ring forecast. The sort both caps the reservation at the four
    /// workers the forecast models and makes the returned selection stable in
    /// value regardless of map iteration order.
    fn water_science_wonder_ring_three_science(
        &self,
        g: &Game,
        pid: usize,
        city_pos: Pos,
    ) -> Vec<f64> {
        if !self.wonder_adjacent_sites_on() {
            return Vec::new();
        }
        let mut science = g
            .wdisk(city_pos, CITY_WORK_RADIUS)
            .into_iter()
            .filter(|pos| g.wdist(city_pos, *pos) == CITY_WORK_RADIUS)
            .filter(|pos| g.players[pid].explored.contains(pos))
            .filter_map(|pos| {
                let tile = g.map.get(pos)?;
                (Self::is_unowned_workable_water_tile(g, tile)
                    && Self::has_known_ring_two_purchase_bridge(g, pid, city_pos, pos))
                .then(|| Self::known_water_wonder_science(g, pid, pos))
            })
            .filter(|science| *science > 0.0)
            .collect::<Vec<_>>();
        science.sort_by(|left, right| right.total_cmp(left));
        science.truncate(SETTLEMENT_FORECAST_POPULATION);
        science
    }

    /// The bounded third-ring acquisition credit used by the final site value.
    /// The first two rings already receive the much richer growth forecast, so
    /// charging each ring-three job once at the canonical settlement yield
    /// weights prefers closeness without pretending every future citizen is
    /// working from turn one.
    pub(super) fn water_science_wonder_site_value(
        &self,
        g: &Game,
        pid: usize,
        city_pos: Pos,
    ) -> f64 {
        self.water_science_wonder_ring_three_science(g, pid, city_pos)
            .into_iter()
            .map(|science| {
                Self::settlement_yield_value(Yields {
                    science,
                    ..Yields::default()
                })
            })
            .sum()
    }

    /// Keep the global shortlist in agreement with the final site score.
    /// `wonder_sites.rs` owns the original first-two-ring prefilter; this
    /// wrapper deliberately keeps the water-science extension in its own
    /// module so recon and the general wonder family retain separate owners.
    pub(super) fn settlement_prefilter_score_with_water_science_wonder(
        &self,
        g: &Game,
        pid: usize,
        city_pos: Pos,
    ) -> f64 {
        self.settlement_prefilter_score_for(g, pid, city_pos)
            + self.water_science_wonder_site_value(g, pid, city_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name::Name;
    use crate::rules::Yields;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected:.6}, got {actual:.6}"
        );
    }

    /// A fully known, feature-free board with an interior prospective city.
    fn flat_board() -> (Game, Pos) {
        let mut game = Game::new_full(1, 20, 14, 91_091, 120, 0, false);
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = Name::new("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
            tile.owner_city = None;
            tile.river_edges = [false; 6];
            tile.cliff_edges = [false; 6];
        }
        game.players[0].explored = game.map.tiles.keys().copied().collect();
        let center = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdisk(*position, CITY_WORK_RADIUS).len() == 37)
            .expect("the test map has an interior tile");
        (game, center)
    }

    /// Put one Galápagos Coast tile immediately inside the candidate's second
    /// ring and its payable Coast neighbour in the third ring.
    fn galapagos_ring_three_board() -> (Game, Pos, Pos) {
        let (mut game, center) = flat_board();
        let target = game
            .wdisk(center, CITY_WORK_RADIUS)
            .into_iter()
            .filter(|position| game.wdist(center, *position) == CITY_WORK_RADIUS)
            .find(|position| {
                game.nbrs(*position)
                    .iter()
                    .any(|neighbor| game.wdist(center, *neighbor) == CITY_WORK_RADIUS - 1)
            })
            .expect("a third-ring target with a second-ring neighbour");
        let wonder = game
            .nbrs(target)
            .into_iter()
            .find(|neighbor| game.wdist(center, *neighbor) == CITY_WORK_RADIUS - 1)
            .expect("the target has a second-ring neighbour");
        game.map.tiles.get_mut(&target).unwrap().terrain = Name::new("coast");
        let wonder_tile = game.map.tiles.get_mut(&wonder).unwrap();
        wonder_tile.terrain = Name::new("coast");
        wonder_tile.feature = Some(Name::new("galapagos_islands"));
        (game, center, target)
    }

    #[test]
    fn known_unowned_galapagos_water_in_the_third_ring_reaches_the_site_scores() {
        let (game, center, target) = galapagos_ring_three_board();
        let stock = AdvancedAi::new();
        let mut gene = AdvancedAi::new();
        gene.enable_wonder_adjacent_sites();

        assert_eq!(
            gene.water_science_wonder_ring_three_science(&game, 0, center),
            vec![2.0],
            "the workable Coast tile receives Galápagos's exact +2 Science"
        );
        let expected = AdvancedAi::settlement_yield_value(Yields {
            science: 2.0,
            ..Yields::default()
        });
        assert_close(
            gene.water_science_wonder_site_value(&game, 0, center),
            expected,
        );
        assert_eq!(stock.water_science_wonder_site_value(&game, 0, center), 0.0);

        // The existing wonder prefilter may also see a first-ring neighbour
        // of this second-ring wonder. Compare against that established score
        // directly: this delta is solely the third-ring asset reaching the
        // shortlist.
        assert_close(
            gene.settlement_prefilter_score_with_water_science_wonder(&game, 0, center)
                - gene.settlement_prefilter_score_for(&game, 0, center),
            expected,
        );
        assert!(
            gene.settlement_static_value_uncached(&game, 0, center)
                > stock.settlement_static_value_uncached(&game, 0, center),
            "the final city-site score also prefers the city that can acquire {target:?}"
        );
    }

    #[test]
    fn water_science_wonder_credit_requires_known_workable_unowned_water() {
        let (game, center, target) = galapagos_ring_three_board();
        let mut gene = AdvancedAi::new();
        gene.enable_wonder_adjacent_sites();
        assert!(gene.water_science_wonder_site_value(&game, 0, center) > 0.0);

        let mut fog = game.clone();
        fog.players[0].explored.remove(&target);
        assert_eq!(gene.water_science_wonder_site_value(&fog, 0, center), 0.0);

        let mut owned = game.clone();
        owned.map.tiles.get_mut(&target).unwrap().owner_city = Some(999);
        assert_eq!(gene.water_science_wonder_site_value(&owned, 0, center), 0.0);

        let mut fenced = game.clone();
        for bridge in fenced.nbrs(target) {
            if fenced.wdist(center, bridge) == CITY_WORK_RADIUS - 1 {
                fenced.map.tiles.get_mut(&bridge).unwrap().owner_city = Some(999);
            }
        }
        assert_eq!(
            gene.water_science_wonder_site_value(&fenced, 0, center),
            0.0,
            "a city cannot reserve a third-ring tile without a neutral purchase bridge"
        );

        let mut land = game.clone();
        land.map.tiles.get_mut(&target).unwrap().terrain = Name::new("grassland");
        assert_eq!(gene.water_science_wonder_site_value(&land, 0, center), 0.0);

        let projector = game
            .nbrs(target)
            .into_iter()
            .find(|neighbor| {
                game.map.tiles[neighbor].feature.as_deref() == Some("galapagos_islands")
            })
            .expect("the target is adjacent to its Galápagos projector");
        let mut land_wonder = game;
        let projector_tile = land_wonder.map.tiles.get_mut(&projector).unwrap();
        projector_tile.terrain = Name::new("grassland");
        projector_tile.feature = Some(Name::new("mount_roraima"));
        assert_eq!(
            gene.water_science_wonder_site_value(&land_wonder, 0, center),
            0.0,
            "a land science wonder remains the general wonder scorer's job"
        );
    }

    #[test]
    fn bermudas_passable_triangle_tiles_are_priced_by_their_actual_science() {
        let (mut game, center) = flat_board();
        let (outer, inner, side) = game
            .wdisk(center, CITY_WORK_RADIUS)
            .into_iter()
            .filter(|outer| game.wdist(center, *outer) == CITY_WORK_RADIUS)
            .find_map(|outer| {
                let inner = game
                    .nbrs(outer)
                    .into_iter()
                    .find(|neighbor| game.wdist(center, *neighbor) == CITY_WORK_RADIUS - 1)?;
                let side = game.nbrs(outer).into_iter().find(|side| {
                    game.wdist(center, *side) == CITY_WORK_RADIUS && game.wdist(inner, *side) == 1
                })?;
                Some((outer, inner, side))
            })
            .expect("an interior ring has a triangular Bermuda placement");
        for position in [outer, inner, side] {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = Name::new("ocean");
            tile.feature = Some(Name::new("bermuda_triangle"));
            assert!(game.rules.is_passable(tile));
        }

        let mut gene = AdvancedAi::new();
        gene.enable_wonder_adjacent_sites();
        assert_eq!(
            gene.water_science_wonder_ring_three_science(&game, 0, center),
            vec![10.0, 10.0],
            "each reachable Bermuda tile receives +5 Science from its two triangle neighbours"
        );
        let expected = AdvancedAi::settlement_yield_value(Yields {
            science: 20.0,
            ..Yields::default()
        });
        assert_close(
            gene.water_science_wonder_site_value(&game, 0, center),
            expected,
        );
    }
}
