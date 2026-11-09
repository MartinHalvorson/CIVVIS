use super::*;

fn two_majors() -> Game {
    Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("TOURISMCLAMP", 4_389),
        120,
        0,
        false,
    )
}

/// A per-turn Tourism total a hair below zero, as a mirrored board produced
/// one (-7.035e-5), used to panic in `f64::clamp` inside `begin_turn`, which
/// is on the live decider's planning path. It now counts as no Tourism.
#[test]
fn a_rounding_negative_tourism_total_is_no_tourism() {
    let mut game = two_majors();
    let before = game.players[0].tourism_lifetime;
    game.add_passive_tourism(0, -7.035e-5, 0.0, 0.0, 0.0);
    game.add_passive_tourism(0, -7.035e-5, 0.5, -1.0e-6, 0.25);
    assert_eq!(game.players[0].tourism_lifetime, before);
    assert!(game.players[0]
        .tourism_pressure
        .values()
        .all(|pressure| *pressure >= 0.0));
}

/// The guard changes nothing for an ordinary total.
#[test]
fn a_positive_tourism_total_still_counts() {
    let mut game = two_majors();
    let before = game.players[0].tourism_lifetime;
    game.add_passive_tourism(0, 4.0, 1.0, 0.0, 0.0);
    assert!(game.players[0].tourism_lifetime > before);
}

/// The legacy pressure reconstruction tolerates a lifetime total that
/// rounded below zero.
#[test]
fn legacy_pressure_tolerates_a_negative_lifetime() {
    let mut game = two_majors();
    game.players[0].tourism_lifetime = -1.0e-6;
    game.players[0].religious_tourism_lifetime = 0.0;
    let pressure = game.legacy_tourism_pressure(0, 1);
    assert!(pressure.is_finite());
}
