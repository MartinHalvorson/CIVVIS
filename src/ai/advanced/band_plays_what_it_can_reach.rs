//! `band-plays-what-it-can-reach`: a Rock Band never stands idle beside a
//! stage it cannot get into.
//!
//! **Why.** Live 20261110T164254Z (Kristina, Deity, Online): five bands, two
//! concerts. Ethiopia, the Culture bar holder and our neighbour, slotted Music
//! Censorship the turn after our first two concerts in its land, and Rome
//! toggled it on and off. The board could not see either policy, so every tour
//! was planned into or through Ethiopia; the host refused the walk (23
//! `did_not_move` across three bands), the tour kept the same target, and
//! `live-move-refusal-break` barred one step at a time, which left the band
//! with no step at all. 65 of 102 band-turns stood still. `bands-draw-from-the-bar`
//! did not cause it: on every turn of that game the host offered stages in
//! one rival's land only, where the bar weighting changes nothing.
//!
//! Two fixes, in two places:
//!
//! - Always on (rules fidelity): Music Censorship is part of the territory gate
//!   ([`Game::bars_foreign_rock_bands`]), so the route planner and the step no
//!   longer disagree, and the live mirror reads a rival's censorship from the
//!   host's concert highlights (`mirror::band_closed_hosts`).
//! - This gene, for every other refusal: each turn the band plays, or steps
//!   toward the best venue it can actually take a step toward (the next one
//!   down when the best one's step is refused here). A trip the host walked
//!   nowhere bars that venue's host for every band for
//!   `BAND_REFUSAL_BAR_TURNS` standard turns, so the next turn aims elsewhere;
//!   the bar holder is preferred only among what is left. The band plays the
//!   stage under it when that is the best venue left or nothing better can be
//!   started. The promotion chooser (`band-plays-its-stage`) counts only
//!   stages in land open to our bands and within the tour's fifteen turns: the
//!   same game took Reggae Rock for three Water Parks, two of them behind
//!   Music Censorship and the third a sea voyage away.
//!
//! Inert unless armed; only Rock Bands move or promote differently.

#[cfg(test)]
mod tests {
    use crate::ai::advanced::AdvancedAi;
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// Three majors on open grassland. Rival 1 holds a wall of land between
    /// our band and rival 2, with a wonder just inside it; rival 2 holds a
    /// wonder beyond the wall. Rows 12 and below are nobody's.
    fn wall() -> (Game, u32, Pos, Pos) {
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
        game.found_city_for(0, (4, 6), None);
        let walled = game.found_city_for(1, (15, 6), None);
        let beyond = game.found_city_for(2, (30, 6), None);
        let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
        for pos in positions {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.owner_city = if (12..=18).contains(&pos.0) && (0..=11).contains(&pos.1) {
                Some(walled)
            } else if pos.0 >= 24 && (0..=11).contains(&pos.1) {
                Some(beyond)
            } else if tile.owner_city == Some(walled) || tile.owner_city == Some(beyond) {
                None
            } else {
                tile.owner_city
            };
        }
        let near = (13, 6);
        let far = (26, 6);
        game.map.tiles.get_mut(&near).unwrap().wonder = Some(name!("pyramids"));
        game.map.tiles.get_mut(&far).unwrap().wonder = Some(name!("colosseum"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let band = game.spawn_test_unit("rock_band", 0, (9, 6));
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("album_cover_art"));
        (game, band, near, far)
    }

    fn owner_of(game: &Game, pos: Pos) -> Option<usize> {
        game.map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| game.cities.get(&city))
            .map(|city| city.owner)
    }

    /// Give the band a fresh turn, as the host would.
    fn new_turn(game: &mut Game, band: u32) {
        game.turn += 1;
        let full = game.unit_max_moves(band);
        game.units.get_mut(&band).unwrap().moves_left = full;
    }

    #[test]
    fn a_band_walks_around_music_censorship_to_a_stage_it_can_play() {
        // The live stall, natively: rival 1 slots Music Censorship. The tour
        // used to plan straight into its land (its wonder is the nearest
        // stage), the step was refused at the border and the band stood there
        // for good. Stock AI, no gene: this is the rules fix.
        let (mut game, band, near, far) = wall();
        game.players[1].policies.insert(name!("music_censorship"));
        assert!(game.bars_foreign_rock_bands(1, 0));
        assert_eq!(game.route_distance(band, near, 0), None);
        let ai = BasicAi::new();
        let mut reached = false;
        for _ in 0..40 {
            let Some(at) = game.units.get(&band).map(|unit| unit.pos) else {
                // Broke up after a concert: it played.
                reached = true;
                break;
            };
            assert_ne!(owner_of(&game, at), Some(1), "{at:?}");
            if at == far {
                reached = true;
                break;
            }
            new_turn(&mut game, band);
            assert!(ai.rock_band_step(&mut game, 0, band), "idle at {at:?}");
        }
        assert!(reached);
    }

    #[test]
    fn a_trip_the_host_walked_nowhere_turns_the_band_to_another_host() {
        let (mut game, band, near, far) = wall();
        let mut ai = BasicAi::new();
        ai.band_plays_what_it_can_reach = true;
        let start = game.units[&band].pos;
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert_eq!(ai.band_trips.borrow()[&i64::from(band)].2, near);
        // The host refuses the walk: next turn the band stands where it was.
        game.relocate(band, start);
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(ai
            .band_refused_hosts
            .borrow()
            .contains_key(&game.players[1].civ));
        assert_eq!(ai.band_trips.borrow()[&i64::from(band)].2, far);

        // Without the gene the same refusal changes nothing.
        let (mut game, band, near, _) = wall();
        let ai = BasicAi::new();
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        game.relocate(band, start);
        new_turn(&mut game, band);
        let before = game.wdist(start, near);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(game.wdist(game.units[&band].pos, near) < before);
    }

    #[test]
    fn a_band_on_a_stage_plays_it_when_the_better_trip_is_refused() {
        let (mut game, band, near, far) = wall();
        let mut ai = BasicAi::new();
        ai.band_plays_what_it_can_reach = true;
        // The band stands on rival 2's wonder, beside a better one of rival
        // 2's that the host would not let it reach.
        game.map.tiles.get_mut(&(25, 6)).unwrap().wonder = Some(name!("pyramids"));
        game.map.tiles.get_mut(&near).unwrap().wonder = None;
        game.relocate(band, far);
        new_turn(&mut game, band);
        let better = BasicAi::rock_band_tour_destination(&game, 0, band);
        assert_eq!(better, Some((25, 6)), "the fixture needs a better stage");
        ai.band_refused_hosts
            .borrow_mut()
            .insert(game.players[2].civ.clone(), game.turn + 8);
        // Both stages are rival 2's, so the bar skips the better one and the
        // band plays where it stands rather than standing idle.
        // It acted without moving: it played.
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(game.units.get(&band).is_none_or(|unit| unit.pos == far));
    }

    #[test]
    fn the_promotion_counts_only_stages_our_bands_can_reach() {
        let (mut game, band, near, far) = wall();
        // Five Water Parks, all in rival 1's land, which censors our bands;
        // one open wonder in rival 2's.
        game.map.tiles.get_mut(&near).unwrap().wonder = None;
        for q in 13..=17 {
            game.map.tiles.get_mut(&(q, 3)).unwrap().district = Some(name!("water_park"));
        }
        game.players[1].policies.insert(name!("music_censorship"));
        game.units.get_mut(&band).unwrap().promotions.clear();
        assert!(game.map.tiles[&far].wonder.is_some());
        let offered = [name!("reggae_rock"), name!("album_cover_art")];
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("reggae_rock"))
        );
        ai.base.band_plays_what_it_can_reach = true;
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("album_cover_art"))
        );
    }
}
