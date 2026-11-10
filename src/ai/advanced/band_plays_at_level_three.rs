//! `band-plays-at-level-three`: a Rock Band's first concerts are played where
//! its performance level reaches 3, and its promotions are taken to get it
//! there.
//!
//! **Why.** A concert's performance level is the band's Rock Band level plus
//! the matching promotion's levels at that stage (Glam Rock, Arena Rock and
//! Reggae Rock +2 at Theater Squares, Entertainment Complexes and Water Parks;
//! Album Cover Art, Music Festival, Space Rock and Surf Rock +1 at wonders,
//! natural wonders and national parks, Campuses and Spaceports, Harbors and
//! Seaside Resorts; `Expansion2_UnitPromotions.xml`,
//! `MODIFIER_PLAYER_UNIT_ADJUST_ROCK_BAND_LEVEL_*`), and the level sets the
//! breakup odds: 45% at level 1, 34% at 2, 25% at 3. Nothing else in the
//! shipped data raises it (`ROCK_BAND_MAX_LEVEL` 4; the Nobel Prize in
//! Literature competition adds album sales and a cheaper band, not levels);
//! Legends and Headliners results raise the band's own level by one.
//!
//! Live, the 104 concerts of 2026-11-10 played 39% at level 1 (51% of them
//! broke the band up), 19% at level 2 (40%), and 42% at 3 or more (13%).
//! 24 of 48 first concerts used no stage bonus at all and 11 used a +2 stage.
//! 205256Z bought nine bands, eight of which broke up within two concerts;
//! 173750Z's 109k concert Tourism came from the two bands that lived seven
//! and eight concerts. A band's first two concerts decide its career.
//!
//! **What the gene does.**
//! - Tour: a band below level 3 plays first wherever it would perform at level
//!   3 or more, among the stages it can reach within four turns, in the order
//!   the seat's scorer ranks them (so the bar holder first among those). With
//!   none in reach, the scorer's order stands.
//! - Promotion (inside `band-plays-its-stage`'s chooser): the offered
//!   promotion that lifts the most open, reachable stages to level 3 (for a
//!   fresh band, the +2 promotion with the most such stages); failing that,
//!   the one that raises the level at the most of them; failing that, the
//!   career-value choice as before. Stages in land closed to our bands, or
//!   beyond the tour's fifteen turns, do not count.

use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

/// The performance level from which a concert's breakup odds drop to 25%.
pub(crate) const LEVEL_THREE: i32 = 3;
/// How far, in turns at full movement, a band below level 3 travels for a
/// level-3 stage before it settles for the scorer's choice.
const LEVEL_THREE_TRIP_TURNS: f64 = 4.0;

impl BasicAi {
    /// `band-plays-at-level-three`: the scorer's ranking with the level-3
    /// stages within four turns moved to the front, for a band below level 3.
    pub(crate) fn rock_band_level_three_first(
        g: &Game,
        pid: usize,
        uid: u32,
        ranked: Vec<Pos>,
    ) -> Vec<Pos> {
        let Some(unit) = g.units.get(&uid) else {
            return ranked;
        };
        if g.rock_band_performance_level(unit, 0) >= LEVEL_THREE {
            return ranked;
        }
        let moves = g.unit_max_moves(uid).max(1.0);
        let (mut first, rest): (Vec<Pos>, Vec<Pos>) = ranked.into_iter().partition(|pos| {
            g.rock_concert_performance_level(pid, uid, *pos)
                .is_some_and(|level| level >= LEVEL_THREE)
                && g.route_distance(uid, *pos, 0)
                    .is_some_and(|distance| distance as f64 / moves <= LEVEL_THREE_TRIP_TURNS)
        });
        first.extend(rest);
        first
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::advanced::AdvancedAi;
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// Two majors on open grassland. The rival's land holds a wonder two hexes
    /// from our band and a Theater Square with an Amphitheater five hexes
    /// away.
    fn stages() -> (Game, u32, Pos, Pos) {
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
        game.found_city_for(0, (4, 6), None);
        let host = game.found_city_for(1, (16, 6), None);
        let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
        for pos in positions {
            if (10..=22).contains(&pos.0) && (2..=10).contains(&pos.1) {
                game.map.tiles.get_mut(&pos).unwrap().owner_city = Some(host);
            }
        }
        let wonder = (11, 6);
        let theater = (14, 6);
        game.map.tiles.get_mut(&wonder).unwrap().wonder = Some(name!("pyramids"));
        game.map.tiles.get_mut(&theater).unwrap().district = Some(name!("theater_square"));
        let city = game.cities.get_mut(&host).unwrap();
        city.districts.insert(name!("theater_square"), theater);
        city.buildings.push(name!("amphitheater"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        let band = game.spawn_test_unit("rock_band", 0, (9, 6));
        (game, band, wonder, theater)
    }

    #[test]
    fn a_level_one_band_plays_its_two_level_stage_first() {
        let (mut game, band, wonder, theater) = stages();
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
        assert_eq!(
            game.rock_concert_performance_level(0, band, wonder),
            Some(1)
        );
        assert_eq!(
            game.rock_concert_performance_level(0, band, theater),
            Some(3)
        );
        let stock = BasicAi::new();
        assert_eq!(stock.rock_band_ranked_venues(&game, 0, band)[0], wonder);
        let mut ai = BasicAi::new();
        ai.band_plays_at_level_three = true;
        assert_eq!(ai.rock_band_ranked_venues(&game, 0, band)[0], theater);
        // A band already at level 3 keeps the scorer's order.
        game.units.get_mut(&band).unwrap().level = 3;
        assert_eq!(ai.rock_band_ranked_venues(&game, 0, band)[0], wonder);
    }

    #[test]
    fn the_first_promotion_lifts_the_open_stages_to_level_three() {
        let (mut game, band, _, _) = stages();
        // Ten wonders against one Theater Square.
        for q in 12..=21 {
            game.map.tiles.get_mut(&(q, 9)).unwrap().wonder = Some(name!("colosseum"));
        }
        let offered = [name!("glam_rock"), name!("album_cover_art")];
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("album_cover_art")),
            "the career chooser takes the +1 stage it sees ten of"
        );
        ai.base.band_plays_at_level_three = true;
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("glam_rock"))
        );
        // Behind Music Censorship the Theater lifts nothing, and the career
        // chooser decides as before.
        game.players[1].policies.insert(name!("music_censorship"));
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("album_cover_art"))
        );
    }
}
