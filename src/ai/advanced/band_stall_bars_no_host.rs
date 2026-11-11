//! `band-stall-bars-no-host`: a Rock Band trip the host walked nowhere bars
//! its host only on the second turn running.
//!
//! **Why (live Deity Pericles 20261110T232642Z, the strongest culture economy
//! we have had).** `band-plays-what-it-can-reach` judges each band's trip once,
//! on the next turn: a band still on the tile it set out from was "walked
//! nowhere", and the host of the venue it was walking to is skipped by every
//! band for eight standard turns. At t143 the host executed no unit move of
//! ours at all (four Trader moves, 21 failed orders, no other `host_move`),
//! so at t144 all four bands that had set out were judged refused, and all
//! four were walking to Mapuche, the Culture bar holder. Every band turned
//! away from the bar holder, walked across the map to other rivals, and
//! walked back when the bar lapsed: three of them reached their first concert
//! 23-25 turns after purchase over 55-65 hexes of path, for a straight-line
//! trip of 17-22 hexes (the efficient first trips that day ran 0.52 turns a
//! hex). Two of the three were still alive with 300-500 album sales when a
//! rival's Science victory ended the game at t180, so the lost turns were
//! the most valuable concerts of their careers. The rule fired 31 times in
//! 9 of the 14 games of 2026-11-10 that bought bands; across those games 17
//! of 84 first trips moved away from the venue they finally played on two or
//! more turns, 164 band-turns over the efficient pace.
//!
//! A host that really refuses a walk (a war, a closed passage) refuses it
//! again the next turn; a turn the host moved nobody does not repeat.
//!
//! **What the gene does.** The first turn a band is walked nowhere records a
//! strike and bars nothing: the band plans as before and normally tries the
//! same trip again. A band walked nowhere on the next turn as well bars the
//! host of the venue it was walking to, exactly as before. A band that moves
//! clears its strike. Bands only; inert unless armed with
//! `band-plays-what-it-can-reach`, whose trip judgement it gates.

use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

impl BasicAi {
    /// Whether this is the second turn running that the host walked the band
    /// under `key` nowhere (this time toward `civ`'s land). The first such
    /// turn records a strike and returns false; the second clears it and
    /// returns true.
    pub(crate) fn band_stall_second_strike(
        &self,
        g: &Game,
        key: i64,
        civ: &str,
        here: Pos,
    ) -> bool {
        let mut strikes = self.band_stall_strikes.borrow_mut();
        let second = strikes
            .get(&key)
            .is_some_and(|(turn, _)| turn + 1 == g.turn);
        if second {
            strikes.remove(&key);
            return true;
        }
        strikes.insert(key, (g.turn, civ.to_string()));
        crate::think!(self.journal, Military, Detail,
               "Rock Band keeps {civ}'s venues after one stalled turn";
               "band-stall-bars-no-host: the host walked it nowhere last turn; \
                {civ} is skipped only if it happens again next turn";
               here);
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// Three majors on open grassland: rival 1's wonder near our band, rival
    /// 2's wonder beyond it.
    fn board() -> (Game, u32, Pos, Pos) {
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

    fn new_turn(game: &mut Game, band: u32) {
        game.turn += 1;
        let full = game.unit_max_moves(band);
        game.units.get_mut(&band).unwrap().moves_left = full;
    }

    fn armed() -> BasicAi {
        let mut ai = BasicAi::new();
        ai.band_plays_what_it_can_reach = true;
        ai.band_stall_bars_no_host = true;
        ai
    }

    #[test]
    fn one_stalled_turn_bars_no_host_and_the_band_keeps_its_trip() {
        let (mut game, band, near, _) = board();
        let ai = armed();
        let start = game.units[&band].pos;
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert_eq!(ai.band_trips.borrow()[&i64::from(band)].2, near);
        // The host moves nobody this turn.
        game.relocate(band, start);
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(ai.band_refused_hosts.borrow().is_empty());
        assert_eq!(ai.band_trips.borrow()[&i64::from(band)].2, near);
        assert!(game.wdist(game.units[&band].pos, near) < game.wdist(start, near));
        // It moved, so its strike is gone.
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(ai.band_stall_strikes.borrow().is_empty());
    }

    #[test]
    fn a_second_refusal_in_a_row_bars_the_host_as_before() {
        let (mut game, band, near, far) = board();
        let ai = armed();
        let start = game.units[&band].pos;
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        for _ in 0..2 {
            game.relocate(band, start);
            new_turn(&mut game, band);
            assert!(ai.rock_band_step(&mut game, 0, band));
        }
        assert!(ai
            .band_refused_hosts
            .borrow()
            .contains_key(&game.players[1].civ));
        assert_eq!(ai.band_trips.borrow()[&i64::from(band)].2, far);
        let _ = near;
    }

    #[test]
    fn off_one_stalled_turn_still_bars_the_host() {
        let (mut game, band, _, far) = board();
        let mut ai = BasicAi::new();
        ai.band_plays_what_it_can_reach = true;
        let start = game.units[&band].pos;
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        game.relocate(band, start);
        new_turn(&mut game, band);
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(ai
            .band_refused_hosts
            .borrow()
            .contains_key(&game.players[1].civ));
        assert_eq!(ai.band_trips.borrow()[&i64::from(band)].2, far);
        assert!(ai.band_stall_strikes.borrow().is_empty());
    }
}
