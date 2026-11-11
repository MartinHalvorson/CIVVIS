//! `band-buys-near-the-stage`: a Rock Band is bought in the city nearest a
//! stage it can play.
//!
//! **Why (live Deity Kristina and Pericles, 2026-11-10, 125 bands in 14
//! games).** The purchase pass walks `player_city_ids` and buys in the first
//! city that accepts, so every band starts in one city. From that city the
//! nearest stage the host highlighted for our bands (`concert_plots`) was a
//! median 9 hexes away (mean 9.3); from the city nearest such a stage it was
//! 4 (mean 4.5). A band took a median 7 turns (mean 9.6) from purchase to its
//! first concert, at 0.52 turns a hex on the trips that went straight, and
//! 41 of the 125 bands never played before the game ended. Concert Tourism
//! compounds with every concert a band survives (`venue x (1 + result +
//! album sales / 100)`), so turns on the road come off the end of a band's
//! career, where its concerts are worth most. In 20261110T232642Z all 16
//! bands started 10-11 hexes from the nearest rival city.
//!
//! **What the gene does.** Our cities are tried nearest a stage first: the
//! stages are the union of the host's concert highlights for our bands, or,
//! before any band has them, the wonders and Theater Squares we have seen in
//! the land of living majors we have met, are at peace with and that admit
//! our bands. Distance is the world's hex distance to the nearest stage;
//! cities keep their order on a tie, and with no stage at all the order is
//! unchanged. Price and every other purchase rule are unchanged.

use super::*;

/// The tiles a new band could play: the host's highlights for our bands,
/// else the wonders and Theater Squares of open rivals we have seen.
fn band_stages(g: &Game, pid: usize) -> BTreeSet<Pos> {
    let highlighted: BTreeSet<Pos> = g
        .units
        .values()
        .filter(|unit| unit.owner == pid && unit.kind == "rock_band")
        .filter_map(|unit| g.host_unit_facts.get(&unit.id))
        .filter_map(|facts| facts.concert_plots.as_ref())
        .flatten()
        .copied()
        .collect();
    if !highlighted.is_empty() {
        return highlighted;
    }
    g.map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.wonder.is_some()
                || tile
                    .district
                    .is_some_and(|district| g.district_family(district) == "theater_square")
        })
        .filter(|(pos, tile)| {
            let Some(city) = tile.owner_city.and_then(|cid| g.cities.get(&cid)) else {
                return false;
            };
            let owner = &g.players[city.owner];
            city.owner != pid
                && owner.alive
                && !owner.is_minor
                && !owner.is_barbarian
                && g.has_met(pid, city.owner)
                && g.players[pid].explored.contains(*pos)
                && !g.is_at_war(pid, city.owner)
                && !g.bars_foreign_rock_bands(city.owner, pid)
        })
        .map(|(pos, _)| *pos)
        .collect()
}

impl AdvancedAi {
    /// Our cities in the order the band purchase tries them: nearest a stage
    /// first, the shipped order on a tie or with no stage known.
    pub(super) fn band_purchase_cities(&self, g: &Game, pid: usize) -> Vec<u32> {
        let mut cities = g.player_city_ids(pid);
        let stages = band_stages(g, pid);
        if stages.is_empty() {
            return cities;
        }
        let reach = |cid: &u32| {
            let pos = g.cities[cid].pos;
            stages
                .iter()
                .map(|stage| g.wdist(pos, *stage))
                .min()
                .unwrap_or(i32::MAX)
        };
        cities.sort_by_key(reach);
        if let Some(first) = cities.first() {
            think!(self.journal(), Faith, Detail,
                   "Rock Bands are bought in {}", g.cities[first].name;
                   "band-buys-near-the-stage: {} hexes from the nearest of {} stages",
                   reach(first), stages.len());
        }
        cities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::HostUnitFacts;
    use crate::name;
    use std::sync::Arc;

    /// Our capital far west, a second city of ours near rival 1's wonder.
    fn board() -> (Game, u32, u32, Pos) {
        let mut game = Game::new_full(2, 40, 16, 472_501, 250, 0, false);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.owner_city = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
        }
        let capital = game.found_city_for(0, (3, 6), None);
        let front = game.found_city_for(0, (20, 6), None);
        let host = game.found_city_for(1, (28, 6), None);
        let wonder = (26, 6);
        let tile = game.map.tiles.get_mut(&wonder).unwrap();
        tile.owner_city = Some(host);
        tile.wonder = Some(name!("pyramids"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        (game, capital, front, wonder)
    }

    #[test]
    fn the_band_is_bought_in_the_city_nearest_a_stage() {
        let (game, capital, front, _) = board();
        let ai = AdvancedAi::new();
        // The shipped pass tries the capital first.
        assert_eq!(game.player_city_ids(0), vec![capital, front]);
        assert_eq!(ai.band_purchase_cities(&game, 0), vec![front, capital]);
    }

    #[test]
    fn the_host_highlights_outrank_the_seen_wonders() {
        let (mut game, capital, front, _) = board();
        // A band of ours whose highlights hold one plot, west of the capital.
        let band = game.spawn_test_unit("rock_band", 0, (5, 6));
        Arc::make_mut(&mut game.host_unit_facts).insert(
            band,
            HostUnitFacts {
                concert_plots: Some(BTreeSet::from([(1, 6)])),
                ..Default::default()
            },
        );
        let ai = AdvancedAi::new();
        assert_eq!(ai.band_purchase_cities(&game, 0), vec![capital, front]);
    }

    #[test]
    fn a_closed_rival_offers_no_stage_and_the_order_stands() {
        let (mut game, capital, front, _) = board();
        game.players[1].policies.insert(name!("music_censorship"));
        let ai = AdvancedAi::new();
        assert_eq!(ai.band_purchase_cities(&game, 0), vec![capital, front]);
    }
}
