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

/// An Online turn read on this game's clock. Online plays two Standard turns
/// in three (`GameSpeed::scale_turns`), so an Online turn is 1.5 Standard
/// turns, and `standard_duration` brings that back to this game's speed.
fn on_this_clock(g: &Game, online_turn: u32) -> u32 {
    g.standard_duration(online_turn * 3 / 2).max(1)
}

/// The live curve's city count at this turn: linear between its marks and
/// rounded to the nearest city, so a Settler for the next city can be on its
/// way before the mark that counts it. Flat before the first mark and after
/// the last.
pub fn live_curve(g: &Game) -> usize {
    let turn = g.turn;
    let first = LIVE_RIVAL_CITY_PACE[0];
    if turn <= on_this_clock(g, first.0) {
        return first.1;
    }
    for pair in LIVE_RIVAL_CITY_PACE.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let (start, end) = (on_this_clock(g, from.0), on_this_clock(g, to.0));
        if turn <= end {
            let span = end.saturating_sub(start).max(1) as f64;
            let along = turn.saturating_sub(start) as f64 / span;
            let cities = from.1 as f64 + along * (to.1 as f64 - from.1 as f64);
            return cities.round() as usize;
        }
    }
    LIVE_RIVAL_CITY_PACE[LIVE_RIVAL_CITY_PACE.len() - 1].1
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
