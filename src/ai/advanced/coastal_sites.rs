//! Price a city site's access to the coast before a Harbor exists.
//!
//! The modern settlement forecast already pays coastal Housing, and the
//! global prefilter retains the old `+6` coast credit.  The full static score
//! then prices only the site's worked yields and generic district adjacency:
//! a normal Harbor's `city_center` adjacency is two Gold, which enters that
//! latter term at half-weight and at Gold's `0.7` weight.  A useful coastal
//! city can therefore lose to an inland site before it has a district to make
//! that access visible.
//!
//! `coastal-city-sites` restores the prefilter's six-point coast signal in
//! the full score, but only where a neighbouring `coast` tile can host the
//! Harbor that the rules define.  Version 2 keeps that base and adds the best
//! local `coast_resource` Harbor adjacency in the city's radius-two footprint.
//! The resource read mirrors the adjacency engine's water-resource predicate;
//! it is deliberately small and capped because it is a site-time forecast,
//! not a promise that the district is available immediately.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

/// The coast credit the historical score and the global prefilter already
/// carry.  Matching it lets the screen price the missing final-score term,
/// rather than inventing a new scale.
const COASTAL_CITY_SITE_BASE: f64 = 6.0;
/// One water resource adjacent to the best prospective Harbor plot is worth
/// one additional site point in version 2.
const COASTAL_RESOURCE_SITE_VALUE: f64 = 1.0;
/// A hex has at most six neighbours; state the cap so the score stays bounded
/// if the local port search changes shape later.
const COASTAL_RESOURCE_SITE_CAP: f64 = 6.0;

impl AdvancedAi {
    /// One version of the coastal-city family plays at a time.
    pub(super) fn coastal_city_sites_on(&self) -> bool {
        self.coastal_city_sites || self.coastal_city_sites_2
    }

    /// The prefilter's exact fresh-water predicate, kept here because the
    /// coastal family must replace its old coast credit without double-paying
    /// it.  The full score's Housing term has a broader lake check; this is
    /// intentionally the predicate used by `settlement_prefilter_score`.
    pub(super) fn settlement_prefilter_has_fresh_water(g: &Game, pos: Pos) -> bool {
        g.map.tiles[&pos].has_river()
            || g.nbrs(pos).iter().any(|neighbor| {
                g.map.get(*neighbor).is_some_and(|tile| {
                    tile.terrain == "lake" || tile.feature.as_deref() == Some("oasis")
                })
            })
    }

    /// A Harbor is placed on `coast`, not merely any water tile.  Keeping the
    /// predicate this narrow means an ocean-only shore does not receive a
    /// bonus for a port it cannot build.
    fn has_harbor_coast(g: &Game, pos: Pos) -> bool {
        g.nbrs(pos).iter().any(|neighbor| {
            g.map
                .get(*neighbor)
                .is_some_and(|tile| tile.terrain == "coast")
        })
    }

    /// The score family's complete contribution at a city site.  Version 1
    /// pays the missing coast baseline; version 2 adds the strongest
    /// water-resource adjacency around a prospective coastal district plot.
    pub(super) fn coastal_city_site_bonus(&self, g: &Game, pos: Pos, positions: &[Pos]) -> f64 {
        if !self.coastal_city_sites_on() || !Self::has_harbor_coast(g, pos) {
            return 0.0;
        }
        let resource = if self.coastal_city_sites_2 {
            Self::coastal_resource_site_value(g, positions)
        } else {
            0.0
        };
        COASTAL_CITY_SITE_BASE + resource
    }

    /// Version 2's resource refinement, using the same `is_water &&
    /// resource.is_some()` condition that the district adjacency engine uses
    /// for `coast_resource`.  The max chooses one prospective Harbor rather
    /// than treating every water resource in the city footprint as one port's
    /// adjacency.
    fn coastal_resource_site_value(g: &Game, positions: &[Pos]) -> f64 {
        let best = positions
            .iter()
            .filter(|at| g.map.get(**at).is_some_and(|tile| tile.terrain == "coast"))
            .map(|coast| {
                g.nbrs(*coast)
                    .iter()
                    .filter(|neighbor| {
                        g.map
                            .get(**neighbor)
                            .is_some_and(|tile| g.rules.is_water(tile) && tile.resource.is_some())
                    })
                    .count() as f64
            })
            .fold(0.0, f64::max);
        (best * COASTAL_RESOURCE_SITE_VALUE).min(COASTAL_RESOURCE_SITE_CAP)
    }

    /// The amount the global prefilter must add so it orders sites as the
    /// family does.  Its old six-point coast term already covers a dry coastal
    /// site; only a fresh coastal site needs the base restored, while version
    /// 2 always needs its resource refinement carried into the shortlist.
    pub(super) fn coastal_city_site_prefilter_bonus(&self, g: &Game, pos: Pos) -> f64 {
        if !self.coastal_city_sites_on() || !Self::has_harbor_coast(g, pos) {
            return 0.0;
        }
        let positions = g.wdisk(pos, 2);
        let family_value = self.coastal_city_site_bonus(g, pos, &positions);
        let existing_coast_credit = if Self::settlement_prefilter_has_fresh_water(g, pos) {
            0.0
        } else {
            COASTAL_CITY_SITE_BASE
        };
        family_value - existing_coast_credit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name::Name;

    /// A flat map with a safely interior city site and one of its neighbours.
    fn flat_board() -> (Game, Pos, Pos) {
        let mut game = Game::new_full(1, 20, 14, 91_041, 120, 0, false);
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
            tile.wonder = None;
            tile.owner_city = None;
            tile.river_edges = [false; 6];
            tile.cliff_edges = [false; 6];
        }
        let center = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdisk(*position, 3).len() == 37)
            .expect("the test map has an interior tile");
        let beside = game
            .nbrs(center)
            .into_iter()
            .find(|position| game.map.get(*position).is_some())
            .expect("the center has a neighbour");
        (game, center, beside)
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected:.6}, got {actual:.6}"
        );
    }

    #[test]
    fn coastal_city_sites_repair_the_full_score_and_keep_the_prefilter_aligned() {
        let (mut game, center, coast) = flat_board();
        let fish = game
            .nbrs(coast)
            .into_iter()
            .find(|position| {
                game.map.get(*position).is_some() && game.wdist(center, *position) == 2
            })
            .expect("the coast tile has a radius-two neighbour");
        game.map.tiles.get_mut(&coast).unwrap().terrain = Name::new("coast");
        let fish_tile = game.map.tiles.get_mut(&fish).unwrap();
        fish_tile.terrain = Name::new("coast");
        fish_tile.resource = Some(Name::new("fish"));

        let stock = AdvancedAi::new();
        let mut first = AdvancedAi::new();
        first.enable_coastal_city_sites();
        let mut second = AdvancedAi::new();
        second.enable_coastal_city_sites_2();
        assert!(
            !second.coastal_city_sites,
            "version 2 selects itself instead of stacking two family flags"
        );

        let stock_static = stock.settlement_static_value_uncached(&game, 0, center);
        let first_static = first.settlement_static_value_uncached(&game, 0, center);
        let second_static = second.settlement_static_value_uncached(&game, 0, center);
        assert_close(first_static - stock_static, COASTAL_CITY_SITE_BASE);
        assert_close(second_static - first_static, COASTAL_RESOURCE_SITE_VALUE);

        let stock_legacy = stock.legacy_settle_value(&game, 0, center);
        let first_legacy = first.legacy_settle_value(&game, 0, center);
        assert_close(first_legacy - stock_legacy, COASTAL_CITY_SITE_BASE);

        let stock_prefilter = AdvancedAi::settlement_prefilter_score(&game, center);
        assert_close(
            first.settlement_prefilter_score_for(&game, center),
            stock_prefilter,
        );
        assert_close(
            second.settlement_prefilter_score_for(&game, center),
            stock_prefilter + COASTAL_RESOURCE_SITE_VALUE,
        );

        let (plain, inland, _) = flat_board();
        assert_close(
            first.settlement_static_value_uncached(&plain, 0, inland)
                - stock.settlement_static_value_uncached(&plain, 0, inland),
            0.0,
        );
    }
}
