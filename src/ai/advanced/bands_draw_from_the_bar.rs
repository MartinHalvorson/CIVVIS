//! `bands-draw-from-the-bar`: a Rock Band's concert is weighted by how much of
//! the Culture race it closes.
//!
//! **Why.** A Culture victory needs more visitors than the largest domestic
//! count among the rivals (the bar), and a visitor comes out of its host's
//! domestic count. A visitor drawn from the rival holding the bar therefore
//! closes the gap twice; one drawn from a rival already below our visitors
//! closes it once. Live 031753Z drew 180 of its 226 visitors from a rival far
//! below the bar, and Russia 120557Z put 54% of its concert Tourism on the bar
//! holder. In the best Pericles games the top two domestic counts sat within
//! 10% of each other in 6 of 10. The analysis of 2026-11-10 put 041324Z at
//! 0.98 of the bar with three times the concert volume drawn evenly, and 1.36
//! with the same volume drawn from the top.
//!
//! `band-plays-its-stage` already weighted tours by `1 + host domestic / bar`,
//! which gives a rival at half the bar 75% of the bar holder's value where the
//! true marginal value is 50%, and it multiplied the band's whole future
//! career, which will be played wherever the band goes next.
//!
//! **What the gene does.** The tour (`BasicAi::rock_band_bar_destination`)
//! ranks venues by `Game::rock_concert_bar_value`: the concert's expected
//! Tourism times `Game::rock_concert_bar_factor` (2 for a rival within
//! `max(10, 5% of the bar)` of the bar, 1 for a rival at or below our own
//! visitors, the share of the way from our visitors to the bar between them),
//! plus the band's survival-weighted future career, unweighted. Travel is
//! priced as `band-plays-its-stage` prices it. Promotions are unchanged.
//! Only Rock Bands move differently, so the gene touches nothing but a
//! Culture seat's bands.

#[cfg(test)]
mod tests {
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;

    /// Five majors on open grassland: us, the bar holder (400 domestic), a
    /// rival just under it (390), one in between (250), and one we already
    /// draw 150 visitors from (100 generated, 0 left at home).
    fn race() -> Game {
        let mut game = Game::new_full(5, 40, 16, 472_501, 250, 0, false);
        for (pid, culture) in [(1, 40_000.0), (2, 39_000.0), (3, 25_000.0), (4, 10_000.0)] {
            game.players[pid].culture_lifetime = culture;
        }
        game.players[0]
            .tourism_pressure
            .insert(4, 5.0 * 200.0 * 150.0);
        game
    }

    #[test]
    fn the_factor_follows_the_race() {
        let game = race();
        assert_eq!(game.foreign_tourists(0), 150);
        assert_eq!(game.domestic_tourists(4), 0);
        // The bar holder and a rival within max(10, 5% of 400) of it.
        assert_eq!(game.rock_concert_bar_factor(0, 1), 2.0);
        assert_eq!(game.rock_concert_bar_factor(0, 2), 2.0);
        // Part of the way from our 150 visitors up to the bar of 400.
        assert!((game.rock_concert_bar_factor(0, 3) - 1.4).abs() < 1e-9);
        // Below our own visitors: the concert only raises our count.
        assert_eq!(game.rock_concert_bar_factor(0, 4), 1.0);
        // Our own land is never a host.
        assert_eq!(game.rock_concert_bar_factor(0, 0), 1.0);
    }

    #[test]
    fn with_the_bar_cleared_every_host_is_one() {
        let mut game = race();
        game.players[0]
            .tourism_pressure
            .insert(4, 5.0 * 200.0 * 1_000.0);
        assert!(game.foreign_tourists(0) > 400);
        for host in 1..5 {
            assert_eq!(game.rock_concert_bar_factor(0, host), 1.0);
        }
    }

    /// Two rivals, each with a wonder two hexes from our band.
    fn two_stages() -> (Game, u32) {
        let mut game = Game::new_full(3, 40, 16, 472_501, 250, 0, false);
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
        game.found_city_for(0, (20, 12), None);
        let east = game.found_city_for(1, (13, 6), None);
        let west = game.found_city_for(2, (1, 6), None);
        for (pos, city) in [((9, 6), east), ((5, 6), west)] {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.owner_city = Some(city);
            tile.wonder = Some(name!("pyramids"));
        }
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let band = game.spawn_test_unit("rock_band", 0, (7, 6));
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
        (game, band)
    }

    #[test]
    fn only_the_concert_is_weighted_never_the_career() {
        let (mut game, band) = two_stages();
        // No bar yet: both pricings read the venue at face value.
        let plain = game.rock_concert_bar_value(0, band, (9, 6)).unwrap();
        assert_eq!(
            plain,
            game.rock_concert_stage_value(0, band, (9, 6)).unwrap()
        );
        game.players[1].culture_lifetime = 50_000.0;
        let bar = game.rock_concert_bar_value(0, band, (9, 6)).unwrap();
        let stage = game.rock_concert_stage_value(0, band, (9, 6)).unwrap();
        // Both double something; only the stage pricing doubles the career.
        assert!(bar > plain, "{bar} vs {plain}");
        assert!(bar - plain < stage - plain, "{bar} vs {stage}");
    }

    #[test]
    fn the_band_tours_toward_the_bar_holder() {
        let (mut game, band) = two_stages();
        game.players[1].culture_lifetime = 50_000.0;
        game.players[2].culture_lifetime = 5_000.0;
        assert_eq!(
            BasicAi::rock_band_bar_destination(&game, 0, band),
            Some((9, 6))
        );
        game.players[1].culture_lifetime = 5_000.0;
        game.players[2].culture_lifetime = 50_000.0;
        assert_eq!(
            BasicAi::rock_band_bar_destination(&game, 0, band),
            Some((5, 6))
        );
    }

    #[test]
    fn the_gene_moves_the_band_to_the_bar_holder() {
        let (mut game, band) = two_stages();
        game.players[1].culture_lifetime = 5_000.0;
        game.players[2].culture_lifetime = 50_000.0;
        let mut ai = BasicAi::new();
        ai.bands_draw_from_the_bar = true;
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(game.units[&band].pos.0 < 7, "{:?}", game.units[&band].pos);
    }
}
