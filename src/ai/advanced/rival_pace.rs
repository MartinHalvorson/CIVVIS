//! `rival-pace` (`gene_screen --rival-pace`, 2026-10-31): a calibrated rival
//! chair's city horizon follows the live Immortal field's city count by turn.
//!
//! ## Why
//!
//! `gene_screen --rivals firaxis-faithful` seats the deployment genome in every
//! rival chair. Its shared city target starts at ten
//! ([`super::SHARED_CITY_TARGET_FLOOR`]), so a faithful rival holds 8.2 cities
//! at turn 60 and 10.0 at turn 100 on the 6p Small board. The live Firaxis
//! Immortal field holds 5.0 and 6.7: the rival mean of the 53-seat 4p Tiny Lv 7
//! census of 2026-10-30, which the first 6p Small seats of 2026-10-31 match to
//! within half a city through turn 100 (5.2 at turn 60, 6.6 at turn 100). The
//! extra width compounds into the faithful field's 1.5–3x production and
//! science against live, so the width has to be held to the live curve.
//!
//! At King the cap follows the live King field instead
//! ([`LIVE_KING_RIVAL_CITY_PACE`], 2026-11-01): about two cities a rival under
//! the Immortal curve through turn 175.
//!
//! ## What it changes
//!
//! With the flag set on a seat (`AdvancedAi::set_rival_city_pace`), the
//! strategic plan's `desired_cities` is capped at [`city_cap`]: the live curve
//! at this turn plus the seat's `extra`. The cap never falls below the cities
//! the seat already holds, so it stops settling and never razes or abandons
//! anything, and conquest is untouched. Every settling path in the agent reads
//! `plan.desired_cities`, which is why the cap is applied there and nowhere
//! else. Nothing reads it without the flag: no gene and no measured seat sets
//! it.

use crate::game::Game;

/// The live Immortal field's rival-mean city count, by raw Online turn (the
/// live seat plays Online): the 4p Tiny Lv 7 census of 2026-10-30, rounded to
/// the city. Turn 225 is the 6p Small seat that reached it.
pub const LIVE_RIVAL_CITY_PACE: [(u32, usize); 7] = [
    (40, 4),
    (60, 5),
    (80, 6),
    (100, 7),
    (150, 9),
    (175, 10),
    (225, 11),
];

/// The live King field's rival-mean city count, by raw Online turn: 17 live
/// 6p Small King games of 2026-11-01 (05:15Z-12:00Z), every met rival's public
/// city count, rounded to the city (2.4 / 3.8 / 4.5 / 5.4 / 6.9 / 8.0 / 9.8
/// at these marks). The Immortal curve held a King batch's faithful chairs to
/// 8.5 cities and 1.55x the live King field's production at turn 150, and
/// their science victories came at a median turn 225 against live's 237.
pub const LIVE_KING_RIVAL_CITY_PACE: [(u32, usize); 7] = [
    (40, 2),
    (60, 4),
    (80, 4),
    (100, 5),
    (150, 7),
    (175, 8),
    (225, 10),
];

/// `gene_screen --seat-pace` (2026-11-01): the live seat's OWN city count, by
/// raw Online turn: our seat in the live King (19 games), Emperor (14) and
/// Immortal (14) games of 2026-10-30/11-01 agree within a city through turn
/// 100 (King 2.7 / 4.0 / 5.1 / 6.0 / 7.9 / 8.8 / 10.1 at these marks; the
/// later marks read only seats past the turn-120 retire rule). A measured
/// harness seat holds 9-10 cities at turns 100-150 against live's 6 and 8,
/// and makes 2-3x live's Culture at turn 150 (-f6's King culture seats).
pub const LIVE_SEAT_CITY_PACE: [(u32, usize); 7] = [
    (40, 3),
    (60, 4),
    (80, 5),
    (100, 6),
    (150, 8),
    (175, 9),
    (225, 10),
];

/// The live field this game's rung plays: the King field's curve at King, the
/// only rung below Emperor with a live census; the Immortal field's on every
/// other rung (Emperor was fitted on it, and the native Prince default keeps
/// the shipped curve).
pub fn live_pace(g: &Game) -> &'static [(u32, usize)] {
    if g.difficulty == "king" {
        &LIVE_KING_RIVAL_CITY_PACE
    } else {
        &LIVE_RIVAL_CITY_PACE
    }
}

/// An Online turn read on this game's clock. Online plays two Standard turns
/// in three (`GameSpeed::scale_turns`), so an Online turn is 1.5 Standard
/// turns, and `standard_duration` brings that back to this game's speed.
fn on_this_clock(g: &Game, online_turn: u32) -> u32 {
    g.standard_duration(online_turn * 3 / 2).max(1)
}

/// The live curve's city count at this turn ([`live_pace`] picks the curve): linear between its marks and
/// rounded to the nearest city, so a Settler for the next city can be on its
/// way before the mark that counts it. Flat before the first mark and after
/// the last.
pub fn live_curve(g: &Game) -> usize {
    curve_at(g, live_pace(g))
}

/// The live seat's own curve at this turn ([`LIVE_SEAT_CITY_PACE`]).
pub fn seat_curve(g: &Game) -> usize {
    curve_at(g, &LIVE_SEAT_CITY_PACE)
}

/// `desired` held to the live seat's curve, and never below the `held`
/// cities already founded or taken. See [`LIVE_SEAT_CITY_PACE`].
pub fn seat_capped(g: &Game, desired: usize, held: usize) -> usize {
    desired.min(seat_curve(g)).max(held)
}

/// `pace` read at this turn: linear between its marks and rounded to the
/// nearest city, flat before the first mark and after the last.
fn curve_at(g: &Game, pace: &[(u32, usize)]) -> usize {
    let turn = g.turn;
    let first = pace[0];
    if turn <= on_this_clock(g, first.0) {
        return first.1;
    }
    for pair in pace.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let (start, end) = (on_this_clock(g, from.0), on_this_clock(g, to.0));
        if turn <= end {
            let span = end.saturating_sub(start).max(1) as f64;
            let along = turn.saturating_sub(start) as f64 / span;
            let cities = from.1 as f64 + along * (to.1 as f64 - from.1 as f64);
            return cities.round() as usize;
        }
    }
    pace[pace.len() - 1].1
}

/// The most cities a paced seat plans for at this turn: the live curve plus
/// the seat's own `extra`. `gene_screen` gives one chair per game `+2`, the
/// live field's best rival, which runs about two cities over the mean.
pub fn city_cap(g: &Game, extra: usize) -> usize {
    live_curve(g) + extra
}

/// `desired` held to the cap, and never below the `held` cities already
/// founded or taken.
pub fn capped(g: &Game, desired: usize, held: usize, extra: usize) -> usize {
    desired.min(city_cap(g, extra)).max(held)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AdvancedAi;

    fn online_game() -> Game {
        let mut game = Game::new_full(2, 74, 46, 11, 250, 0, false);
        game.game_speed = crate::setup::GameSpeed::Online;
        game
    }

    /// A King game follows the live King field's curve; Emperor keeps the
    /// Immortal field's.
    #[test]
    fn the_seat_cap_follows_the_live_seat_on_every_rung() {
        let mut game = online_game();
        for difficulty in ["king", "emperor", "immortal"] {
            game.difficulty = difficulty.to_string();
            for (turn, cities) in LIVE_SEAT_CITY_PACE {
                game.turn = turn;
                assert_eq!(seat_curve(&game), cities, "{difficulty} turn {turn}");
            }
        }
        game.turn = 100;
        assert_eq!(seat_capped(&game, 12, 4), 6);
        assert_eq!(seat_capped(&game, 12, 8), 8, "never below the held cities");
        assert_eq!(seat_capped(&game, 5, 4), 5);
    }

    #[test]
    fn a_king_game_follows_the_king_field() {
        let mut game = online_game();
        game.difficulty = "king".to_string();
        for (turn, cities) in LIVE_KING_RIVAL_CITY_PACE {
            game.turn = turn;
            assert_eq!(live_curve(&game), cities, "king turn {turn}");
        }
        game.turn = 150;
        assert_eq!(city_cap(&game, 2), 9);
        game.difficulty = "emperor".to_string();
        assert_eq!(live_curve(&game), 9, "Emperor keeps the Immortal curve");
    }

    /// The curve hits every live mark on the Online clock, climbs between
    /// them, and holds flat outside them.
    #[test]
    fn the_cap_follows_the_live_curve_on_the_online_clock() {
        let mut game = online_game();
        for (turn, cities) in LIVE_RIVAL_CITY_PACE {
            game.turn = turn;
            assert_eq!(live_curve(&game), cities, "turn {turn}");
            assert_eq!(city_cap(&game, 2), cities + 2, "turn {turn}");
        }
        game.turn = 1;
        assert_eq!(live_curve(&game), 4);
        game.turn = 125;
        assert_eq!(live_curve(&game), 8, "halfway from 7 at t100 to 9 at t150");
        game.turn = 400;
        assert_eq!(live_curve(&game), 11);
        let mut last = 0;
        for turn in 1..=250 {
            game.turn = turn;
            let cap = live_curve(&game);
            assert!(cap >= last, "the curve never falls: t{turn}");
            last = cap;
        }
        // The cap stops settling; it never asks a seat to give a city up.
        game.turn = 60;
        assert_eq!(capped(&game, 10, 3, 0), 5);
        assert_eq!(capped(&game, 10, 8, 0), 8);
        assert_eq!(capped(&game, 4, 3, 2), 4);
    }

    /// The plan reads the cap only on a paced seat: the deployment genome's
    /// ten-city shared floor is held to the curve with the flag and untouched
    /// without it.
    #[test]
    fn a_paced_seat_plans_the_live_curve_and_an_unpaced_one_does_not() {
        let mut game = online_game();
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("each major starts with a settler");
            let position = game.units[&settler].pos;
            game.found_city_for(pid, position, None);
            game.remove_unit(settler);
        }
        game.current = 0;
        let mut deployed = AdvancedAi::new();
        deployed.enable_engine_repairs();
        let mut paced = deployed.clone();
        paced.set_rival_city_pace(0);
        let mut best = deployed.clone();
        best.set_rival_city_pace(2);
        for turn in [20, 60, 100] {
            game.turn = turn;
            let free = deployed.assess(&game, 0).desired_cities;
            let held = paced.assess(&game, 0).desired_cities;
            assert!(
                free > city_cap(&game, 0),
                "t{turn}: the deployment genome plans {free}, wider than the curve"
            );
            assert_eq!(held, city_cap(&game, 0), "t{turn}");
            assert_eq!(
                best.assess(&game, 0).desired_cities,
                free.min(city_cap(&game, 2)),
                "t{turn}"
            );
        }
    }
}
