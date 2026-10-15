//! The open-city roll: a journal line on a Siege row whose city stands
//! without walls while its owner is weak. It must decide nothing.
use super::tests::{at, conquest, flat_board, on, war};
use super::*;
use crate::ai::Ai;

/// Our capital at (6, 8), the rival's at (14, 8) without walls and with no
/// soldier of theirs; two archers of ours, one in range of it and one five
/// tiles out, and a warrior beside it.
fn open_rival() -> (Game, u32) {
    let mut g = flat_board(374520, &[at(6, 8), at(14, 8)], false);
    let rival = g.city_at(at(14, 8)).unwrap();
    war(&mut g, 0, 1);
    g.cities.get_mut(&rival).unwrap().wall_hp = 0;
    g.spawn_test_unit("archer", 0, at(12, 8));
    g.spawn_test_unit("archer", 0, at(9, 8));
    g.spawn_test_unit("warrior", 0, at(13, 8));
    (g, rival)
}

fn rolls(g: &Game, rival: u32) -> Vec<String> {
    let journal = crate::reasoning::Journal::recording();
    let mut ai = on();
    ai.attach_journal(journal.handle());
    ai.rebuild_force_groups(g, 0, &conquest(g, Some(rival)));
    journal
        .since(0)
        .thoughts
        .iter()
        .filter(|t| t.headline == "Siege open-city roll")
        .map(|t| t.detail.clone())
        .collect()
}

#[test]
fn an_open_city_with_a_weak_owner_is_rolled_once() {
    let (g, rival) = open_rival();
    let name = g.cities[&rival].name.clone();
    let lines = rolls(&g, rival);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with(&format!("{name} walls 0; in siege: ")),
        "{lines:?}"
    );
    assert!(lines[0].contains("none of theirs within 4"), "{lines:?}");
}

#[test]
fn a_walled_city_is_not_rolled() {
    let (mut g, rival) = open_rival();
    {
        let city = g.cities.get_mut(&rival).unwrap();
        city.buildings.push(crate::name!("walls"));
        city.wall_hp = 50;
    }
    assert!(rolls(&g, rival).is_empty());
}

#[test]
fn a_city_its_owner_still_holds_in_force_is_not_rolled() {
    let (mut g, rival) = open_rival();
    for pos in [
        at(15, 8),
        at(15, 7),
        at(15, 9),
        at(14, 7),
        at(14, 9),
        at(16, 8),
    ] {
        g.spawn_test_unit("spearman", 1, pos);
    }
    assert!(g.military_power(1) > 0.25 * g.military_power(0));
    assert!(rolls(&g, rival).is_empty());
}

#[test]
fn the_roll_decides_nothing() {
    let (g, rival) = open_rival();
    let plan = conquest(&g, Some(rival));
    let mut silent = on();
    silent.rebuild_force_groups(&g, 0, &plan);
    let journal = crate::reasoning::Journal::recording();
    let mut heard = on();
    heard.attach_journal(journal.handle());
    heard.rebuild_force_groups(&g, 0, &plan);
    assert!(journal
        .since(0)
        .thoughts
        .iter()
        .any(|t| t.headline == "Siege open-city roll"));
    assert_eq!(silent.objective_board(), heard.objective_board());
}
