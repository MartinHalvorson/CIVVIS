//! `Game::domestic_tourists` on a native board: the tourists a civilization
//! generates (one per 100 lifetime Culture) less those visiting other
//! civilizations, never below an empty pool. The host's counter never reads
//! negative (0 of 174 live readings on 2026-10-31); a negative native value
//! let any rival clear the Culture Victory bar against that civilization.
use super::*;

fn two_majors() -> Game {
    Game::new_full(2, 24, 16, 4298, 200, 0, false)
}

#[test]
fn domestic_tourists_subtract_the_ones_abroad() {
    let mut g = two_majors();
    g.players[0].culture_lifetime = 1_000.0;
    Arc::make_mut(&mut g.observed_visiting_tourists).insert((1, 0), 3);
    assert_eq!(g.domestic_tourists(0), 7);
}

#[test]
fn domestic_tourists_never_go_negative() {
    let mut g = two_majors();
    g.players[0].culture_lifetime = 250.0;
    Arc::make_mut(&mut g.observed_visiting_tourists).insert((1, 0), 5);
    assert_eq!(
        g.domestic_tourists(0),
        0,
        "more tourists abroad than generated leaves an empty pool, not -3"
    );
}
