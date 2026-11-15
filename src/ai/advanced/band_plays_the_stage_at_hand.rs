//! `band-plays-the-stage-at-hand`: a Rock Band plays a stage it can reach this
//! turn instead of a long trip to a stage at most one level better.
//!
//! **Why (live Deity Greece 20261115T144245Z and Russia 20261115T114552Z,
//! 34 bands, 579 band-turns, 82 concerts).** A band's concerts are worth more
//! the longer it lives: album sales add onto every later concert, so the band
//! that reached its 12th concert played its last ones for 9-15k Tourism. What
//! limits careers is the road, not the stage. Bands already played the best
//! level their host offered within 8 hexes in 75 of 82 concerts, but only 82
//! of 579 band-turns held a concert: 40 times a band set out on a trip of two
//! or more turns from beside a stage it could have played, and those trips
//! took 266 band-turns. 15 of them ended at a stage no better than the one it
//! left, and 6 never ended. 6160408 (Arena Rock) stood one hex from an
//! Entertainment Complex at t140, turned for another one 15 hexes off in the
//! bar holder's land, walked back, and played the first at t143.
//! 8650773 (Music Festival, album 150) walked its last 28 turns toward
//! natural wonders that kept closing (borders, a teleport home) past
//! level-1 stages 0-3 hexes away, and never played again. 7143449 (Goes To
//! 11, every stage level 1) walked 12 turns round the map to play a level-1
//! wonder in the bar holder's land, and broke up there.
//!
//! The trips come from two rules that price distance as if a band had all the
//! turns it wants. The career scorers divide a stage's value by only
//! `1 + 0.1 x turns`, so with the bar holder's factor of 2 a stage ten turns
//! away ties one at hand; `band-plays-at-level-three` ranks any higher-level
//! stage within the tour's fifteen turns above every lower one and forbids
//! the stage under the band while it is listed. A Culture seat's game ends at
//! t176-191, when a rival's Science victory lands, so a band bought at t150
//! has thirty turns, and a trip that fails (closed borders, a teleport, a
//! host that walks it nowhere) leaves it nothing. Bands covered 2.5-3.0 hexes
//! a turn live, so a "turn" below (a band's full movement in hexes, 4) is
//! closer to a turn and a half on the ground.
//!
//! **What the gene does.** Each step, the band takes the stage its tour ranks
//! first. A stage it can play this turn (the one under it, or one within one
//! turn at full movement) stands in for that stage when the trip there is
//! longer than the levels it gains are worth: more than
//! [`SAME_LEVEL_TRIP_TURNS`] turns for no level, more than
//! [`ONE_LEVEL_TRIP_TURNS`] for one, and never for two or more. So a level-1
//! stage never stands in for a level-3 one, `band-plays-at-level-three`'s case
//! (45% breakup at level 1 against 26% at level 3); a level-2 one stands in
//! for a level-3 one beyond four turns (34% against 26%), and a stage at the
//! same level beyond two. Among the stages that may stand in the band plays
//! the one under it when that is as high a level as any, otherwise it walks
//! to the highest-level one, the tour's order breaking ties. The tour resumes
//! the next turn, and a stage once played is never offered again. Bands
//! only; a band with no such stage at hand tours exactly as before.

use crate::ai::BasicAi;
use crate::game::{Action, Game};
use crate::Pos;

/// The longest trip, in turns at full movement, a band takes past a stage at
/// hand that plays at the same level as the trip's stage (or higher).
pub(crate) const SAME_LEVEL_TRIP_TURNS: f64 = 2.0;

/// The longest trip a band takes past a stage at hand one level below the
/// trip's stage. `band-plays-at-level-three` calls a level-3 stage within the
/// same four turns near.
pub(crate) const ONE_LEVEL_TRIP_TURNS: f64 = 4.0;

/// How far a stage at hand may be: one turn at full movement.
const AT_HAND_TURNS: f64 = 1.0;

/// Whether a stage at hand at `level` stands in for the tour's stage at
/// `target_level`, `target_trip` turns away.
pub(crate) fn stands_in(level: i32, target_level: i32, target_trip: f64) -> bool {
    match target_level - level {
        gain if gain <= 0 => target_trip > SAME_LEVEL_TRIP_TURNS,
        1 => target_trip > ONE_LEVEL_TRIP_TURNS,
        _ => false,
    }
}

impl BasicAi {
    /// `band-plays-the-stage-at-hand`: play, or walk to, a stage at hand
    /// instead of the tour's stage when the trip there is longer than its
    /// levels are worth ([`stands_in`]). `Some(acted)` when the gene took the
    /// step, `None` to leave the band to its tour.
    pub(crate) fn rock_band_stage_at_hand(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        here: Pos,
    ) -> Option<bool> {
        let ranked = self.rock_band_ranked_venues(g, pid, uid);
        let moves = g.unit_max_moves(uid).max(1.0);
        let trip = |g: &Game, pos: Pos| {
            g.route_distance(uid, pos, 0)
                .map(|distance| distance as f64 / moves)
        };
        // The stage the tour would walk to: its first, past any host whose
        // land walked a band nowhere (`band-plays-what-it-can-reach`).
        let target = ranked
            .iter()
            .copied()
            .find(|pos| *pos == here || !self.band_host_barred(g, *pos))?;
        if target == here {
            return None;
        }
        let target_trip = trip(g, target)?;
        if target_trip <= SAME_LEVEL_TRIP_TURNS {
            return None;
        }
        let target_level = g.rock_concert_performance_level(pid, uid, target)?;
        let here_level = g
            .rock_concert_tourism(pid, uid)
            .and_then(|_| g.rock_concert_performance_level(pid, uid, here))
            .filter(|level| stands_in(*level, target_level, target_trip));
        // The highest-level stage at hand that may stand in, the tour's order
        // breaking ties.
        let mut at_hand: Option<(i32, Pos)> = None;
        for pos in ranked.iter().copied().filter(|pos| *pos != here) {
            if self.band_host_barred(g, pos) || trip(g, pos).is_none_or(|t| t > AT_HAND_TURNS) {
                continue;
            }
            let Some(level) = g
                .rock_concert_performance_level(pid, uid, pos)
                .filter(|level| stands_in(*level, target_level, target_trip))
            else {
                continue;
            };
            if at_hand.is_none_or(|(best, _)| level > best) {
                at_hand = Some((level, pos));
            }
        }
        match (here_level, at_hand) {
            (Some(level), at_hand) if at_hand.is_none_or(|(best, _)| level >= best) => {
                if g.apply(pid, &Action::PerformConcert { unit: uid }).is_err() {
                    return None;
                }
                crate::think!(self.journal, Military, Detail,
                       "Rock Band plays the stage at hand";
                       "band-plays-the-stage-at-hand: level {level} here instead of a \
                        {target_trip:.1}-turn trip to {target:?} at level {target_level}";
                       here);
                Some(true)
            }
            (_, Some((level, stage))) => {
                let next = g.route_step(uid, stage, 0)?;
                if !self.path_move(g, pid, uid, next) {
                    return None;
                }
                crate::think!(self.journal, Military, Detail,
                       "Rock Band walks to the stage at hand";
                       "band-plays-the-stage-at-hand: {stage:?} at level {level} this turn \
                        instead of a {target_trip:.1}-turn trip to {target:?} at level \
                        {target_level}";
                       here);
                Some(true)
            }
            _ => None,
        }
    }

    /// Whether `band-plays-what-it-can-reach` bars the host of this stage
    /// this turn, as its tour does.
    fn band_host_barred(&self, g: &Game, stage: Pos) -> bool {
        Self::band_venue_host(g, stage).is_some_and(|host| {
            self.band_refused_hosts
                .borrow()
                .get(&g.players[host].civ)
                .is_some_and(|until| g.turn < *until)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// Two majors on open grassland, 80 hexes round. The rival's land holds a
    /// wonder two hexes from our band, a wonder under it, and a Theater
    /// Square with an Amphitheater either 26 hexes away (a trip of six and
    /// a half turns) or, with `near_theater`, 8 hexes away (two turns).
    fn board(near_theater: bool) -> (Game, u32, Pos, Pos, Pos) {
        let mut game = Game::new_full(2, 80, 16, 472_501, 250, 0, false);
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
        let host = game.found_city_for(1, (30, 6), None);
        let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
        for pos in positions {
            if (10..=50).contains(&pos.0) && (1..=11).contains(&pos.1) {
                game.map.tiles.get_mut(&pos).unwrap().owner_city = Some(host);
            }
        }
        let under = (12, 6);
        let beside = (14, 6);
        let theater = if near_theater { (20, 6) } else { (38, 6) };
        game.map.tiles.get_mut(&under).unwrap().wonder = Some(name!("pyramids"));
        game.map.tiles.get_mut(&beside).unwrap().wonder = Some(name!("colosseum"));
        game.map.tiles.get_mut(&theater).unwrap().district = Some(name!("theater_square"));
        let city = game.cities.get_mut(&host).unwrap();
        city.districts.insert(name!("theater_square"), theater);
        city.buildings.push(name!("amphitheater"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        let band = game.spawn_test_unit("rock_band", 0, under);
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
        (game, band, under, beside, theater)
    }

    /// The live seat's band genes, with or without this one.
    fn ai(at_hand: bool) -> BasicAi {
        let mut ai = BasicAi::new();
        ai.band_plays_its_stage = true;
        ai.band_plays_what_it_can_reach = true;
        ai.band_plays_at_level_three = true;
        ai.band_plays_the_stage_at_hand = at_hand;
        ai
    }

    fn played(game: &Game, band: u32, stage: Pos) -> bool {
        game.players[0]
            .counters
            .get(&format!("rock_concert:{band}:{},{}", stage.0, stage.1))
            .is_some_and(|count| *count > 0)
    }

    /// One turn of the unit loop: step until the band acts no more.
    fn take_turn(ai: &BasicAi, game: &mut Game, band: u32) {
        let full = game.unit_max_moves(band);
        game.units.get_mut(&band).unwrap().moves_left = full;
        for _ in 0..8 {
            if game
                .units
                .get(&band)
                .is_none_or(|unit| unit.moves_left <= 0.0)
            {
                break;
            }
            if !ai.rock_band_step(game, 0, band) {
                break;
            }
        }
    }

    /// Album Cover Art lifts both wonders to level 2 against the Theater
    /// Square's 3.
    fn cover_art(game: &mut Game, band: u32) {
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("album_cover_art"));
    }

    #[test]
    fn a_trip_is_worth_two_turns_a_level_from_two() {
        use super::stands_in;
        // The same level: a trip of more than two turns.
        assert!(!stands_in(2, 2, 2.0));
        assert!(stands_in(2, 2, 2.5));
        assert!(stands_in(3, 2, 2.5));
        // One level: more than four.
        assert!(!stands_in(2, 3, 4.0));
        assert!(stands_in(2, 3, 4.5));
        // Two levels: never.
        assert!(!stands_in(1, 3, 15.0));
    }

    #[test]
    fn off_the_band_walks_past_the_stage_under_it_on_a_long_trip() {
        let (mut game, band, under, _, theater) = board(false);
        cover_art(&mut game, band);
        assert_eq!(game.rock_concert_performance_level(0, band, under), Some(2));
        assert_eq!(
            game.rock_concert_performance_level(0, band, theater),
            Some(3)
        );
        let ai = ai(false);
        assert_eq!(ai.rock_band_ranked_venues(&game, 0, band)[0], theater);
        take_turn(&ai, &mut game, band);
        assert!(!played(&game, band, under));
        assert!(game.wdist(game.units[&band].pos, theater) < game.wdist(under, theater));
    }

    #[test]
    fn a_six_turn_trip_gives_way_to_the_stage_one_level_below_under_it() {
        let (mut game, band, under, _, theater) = board(false);
        cover_art(&mut game, band);
        let ai = ai(true);
        take_turn(&ai, &mut game, band);
        assert!(played(&game, band, under));
        assert!(!played(&game, band, theater));
    }

    #[test]
    fn a_stage_at_hand_is_walked_to_and_played_the_same_turn() {
        let (mut game, band, under, beside, _) = board(false);
        cover_art(&mut game, band);
        // The wonder under the band is already played; the next one is two
        // hexes on.
        game.players[0]
            .counters
            .insert(format!("rock_concert:{band}:{},{}", under.0, under.1), 1);
        let ai = ai(true);
        take_turn(&ai, &mut game, band);
        assert!(played(&game, band, beside));
    }

    #[test]
    fn a_stage_two_levels_below_never_stands_in_for_the_long_trip() {
        // Glam Rock alone: the wonders play at level 1, the Theater at 3.
        let (mut game, band, under, _, theater) = board(false);
        assert_eq!(game.rock_concert_performance_level(0, band, under), Some(1));
        let ai = ai(true);
        take_turn(&ai, &mut game, band);
        assert!(!played(&game, band, under));
        assert!(game.wdist(game.units[&band].pos, theater) < game.wdist(under, theater));
    }

    #[test]
    fn a_two_turn_trip_for_one_level_is_taken_as_before() {
        let (mut game, band, under, _, theater) = board(true);
        cover_art(&mut game, band);
        let ai = ai(true);
        take_turn(&ai, &mut game, band);
        assert!(!played(&game, band, under));
        assert!(game.wdist(game.units[&band].pos, theater) < game.wdist(under, theater));
    }
}
