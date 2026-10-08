use super::*;
use std::sync::Arc;

/// Live Emperor G415 after turn 104: four major seats on the board, Poland's
/// (seat 3) empty since we eliminated it; the host's standing lists three
/// living majors. Ethiopia (seat 1) founded Orthodoxy and Sumeria (seat 2)
/// does not follow it.
fn after_poland_fell() -> Game {
    let mut g = Game::new_full(4, 40, 24, 372_104, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (2, 8), None);
    g.found_city_for(1, (14, 8), None);
    g.found_city_for(2, (26, 8), None);
    g.players[1].religion = Some("Orthodoxy".into());
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Orthodoxy".into());
    g.players[0].live_living_majors = Some(3);
    g.current = 0;
    g
}

fn living(g: &Game) -> usize {
    g.players
        .iter()
        .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
        .count()
}

/// The empty seat beyond the host's count retires; the gene off, it stays,
/// and Orthodoxy keeps a dead "second holdout" that makes it a safe
/// counterweight.
#[test]
fn an_eliminated_major_leaves_the_living() {
    let mut g = after_poland_fell();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.retire_defeated_majors(&mut g, 0);
    assert_eq!(living(&g), 4, "off: the dead seat stays alive");
    assert!(AdvancedAi::counterfaith_leaves_two_holdouts_at(
        &g,
        0,
        "Orthodoxy"
    ));

    ai.enable_defeated_majors_leave_the_board();
    ai.retire_defeated_majors(&mut g, 0);
    assert_eq!(living(&g), 3);
    assert!(!g.players[3].alive, "the empty seat retires");
    assert!(g.players[1].alive && g.players[2].alive);
    assert!(
        !AdvancedAi::counterfaith_leaves_two_holdouts_at(&g, 0, "Orthodoxy"),
        "Sumeria is Orthodoxy's last holdout besides us"
    );
    // Idempotent.
    ai.retire_defeated_majors(&mut g, 0);
    assert_eq!(living(&g), 3);
}

/// No host count, a count that matches the board, or no empty seat to
/// retire leaves every seat alive.
#[test]
fn nothing_retires_without_an_empty_seat_beyond_the_count() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_defeated_majors_leave_the_board();

    let mut headless = after_poland_fell();
    headless.players[0].live_living_majors = None;
    ai.retire_defeated_majors(&mut headless, 0);
    assert_eq!(living(&headless), 4);

    let mut matching = after_poland_fell();
    matching.players[0].live_living_majors = Some(4);
    ai.retire_defeated_majors(&mut matching, 0);
    assert_eq!(living(&matching), 4);

    // The host says two, but only one seat is empty: seats with cities stay.
    let mut short = after_poland_fell();
    short.players[0].live_living_majors = Some(2);
    ai.retire_defeated_majors(&mut short, 0);
    assert_eq!(living(&short), 3);
    assert!(!short.players[3].alive);
}
