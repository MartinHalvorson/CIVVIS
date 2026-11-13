//! Focused tests for `declaration-needs-a-capture`.
//!
//! The board is open grassland: our city in the west at (2, 8); player 1, at
//! peace with us, holds the city at [`TARGET`] with the walls and the defence
//! each test poses. The host allows a Formal War, so the shipped opening is a
//! declaration rather than a denouncement.

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::{Action, ObservedWarTypes};
use crate::reasoning::Journal;
use std::sync::Arc;

const TARGET: Pos = (18, 8);

/// Our city at (2, 8) and player 1's at [`TARGET`], at peace, turn 80, with a
/// Formal War on player 1 legal this turn.
fn front(walls: i32, defence: f64) -> (Game, u32) {
    let mut g = Game::new_full(2, 36, 22, 4_175, 300, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, (2, 8), None);
    let target = g.found_city_for(1, TARGET, None);
    {
        let city = g.cities.get_mut(&target).unwrap();
        if walls > 0 {
            city.buildings.push(crate::name!("walls"));
        }
        city.wall_hp = walls;
    }
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(target, walls);
    Arc::make_mut(&mut g.observed_city_strength).insert(target, defence);
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
    }
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 80;
    Arc::make_mut(&mut g.host_war_type_permissions).insert(
        (0, 1),
        ObservedWarTypes {
            turn: 80,
            permissions: BTreeMap::from([
                ("DECLARE_FORMAL_WAR".to_string(), true),
                ("DECLARE_SURPRISE_WAR".to_string(), true),
            ]),
        },
    );
    assert!(!g.is_at_war(0, 1));
    (g, target)
}

/// `kinds` spawned for player 0 on free land tiles `distance` from the target.
fn army(g: &mut Game, kinds: &[&str], distance: i32) -> Vec<u32> {
    let mut free: Vec<Pos> = g
        .wring(TARGET, distance)
        .into_iter()
        .filter(|pos| g.map.get(*pos).is_some() && g.city_at(*pos).is_none())
        .filter(|pos| g.unit_ids_at(*pos).is_empty())
        .collect();
    free.sort_by_key(|pos| (pos.0, pos.1));
    kinds
        .iter()
        .zip(free)
        .map(|(kind, pos)| g.spawn_test_unit(kind, 0, pos))
        .collect()
}

fn seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if on {
        ai.enable_declaration_needs_a_capture();
    }
    ai
}

fn declares(action: &Option<Action>) -> bool {
    matches!(
        action,
        Some(Action::DeclareWar { player: 1 } | Action::DeclareWarWithCasusBelli { player: 1, .. })
    )
}

#[test]
fn the_gene_ships_off() {
    opt_in_off_in_both_controllers("declaration-needs-a-capture", |ai| {
        ai.declaration_needs_a_capture
    });
}

#[test]
fn off_every_opening_is_what_it_was() {
    let (g, _) = front(0, 20.0);
    let ai = seat(false);
    assert!(!ai.declaration_lacks_a_capture(&g, 0, 1));
    let gated = ai.preferred_war_opening(&g, 0, 1);
    assert!(declares(&gated), "a Formal War is legal: {gated:?}");
    assert_eq!(gated, ai.preferred_war_opening_ungated(&g, 0, 1));
    assert!(declares(&ai.raid_opening(&g, 0, 1)));
}

#[test]
fn no_soldier_in_reach_holds_the_war() {
    let (mut g, _) = front(0, 20.0);
    // Soldiers at home, far out of reach of the target.
    g.spawn_test_unit("swordsman", 0, (3, 8));
    let journal = Journal::recording();
    let mut ai = seat(true);
    ai.attach_journal(journal.handle());
    assert_eq!(ai.capture_verdict(&g, 0, 1), CaptureVerdict::Hold(None));
    assert!(ai.declaration_lacks_a_capture(&g, 0, 1));
    assert_eq!(ai.preferred_war_opening(&g, 0, 1), None);
    assert!(
        declares(&ai.preferred_war_opening_ungated(&g, 0, 1)),
        "the faith openers' road is not gated"
    );
    assert_eq!(
        ai.raid_opening(&g, 0, 1),
        None,
        "nor the raid's surprise war"
    );
    // Asked again the same turn: one reading, one line.
    assert!(ai.declaration_lacks_a_capture(&g, 0, 1));
    let lines: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline.starts_with("Holding the war on"))
        .map(|thought| thought.detail.clone())
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("short of: bodies"), "{lines:?}");
}

#[test]
fn a_corps_beside_an_open_city_goes() {
    let (mut g, cid) = front(0, 20.0);
    let pool = army(&mut g, &["swordsman"; 6], 2);
    let ai = seat(true);
    match ai.capture_verdict(&g, 0, 1) {
        CaptureVerdict::Go(reading) => {
            assert_eq!(reading.city, cid);
            assert_eq!(reading.window, OPEN_WINDOW);
            assert!(reading.chance >= DECLARE_P, "{reading:?}");
            assert!(reading.corps <= pool.len(), "{reading:?}");
            assert_eq!(reading.pool, pool.len());
        }
        held => panic!("six Swordsmen beside an open city: {held:?}"),
    }
    assert!(!ai.declaration_lacks_a_capture(&g, 0, 1));
    assert!(declares(&ai.preferred_war_opening(&g, 0, 1)));
}

#[test]
fn a_lone_warrior_before_an_open_city_holds_short_of_more() {
    let (mut g, _) = front(0, 30.0);
    g.spawn_test_unit("spearman", 1, TARGET);
    army(&mut g, &["warrior"], 3);
    let ai = seat(true);
    match ai.capture_verdict(&g, 0, 1) {
        CaptureVerdict::Hold(Some(reading)) => {
            assert!(reading.chance < DECLARE_P, "{reading:?}");
            assert!(!reading.needs_breaker, "an open city: {reading:?}");
            assert!(reading.short_of.is_some(), "{reading:?}");
        }
        other => panic!("one Warrior: {other:?}"),
    }
    assert_eq!(ai.preferred_war_opening(&g, 0, 1), None);
}

#[test]
fn a_walled_city_without_a_breaker_holds_for_one() {
    let (mut g, cid) = front(200, 40.0);
    g.spawn_test_unit("spearman", 1, TARGET);
    army(&mut g, &["swordsman"; 6], 2);
    let ai = seat(true);
    match ai.capture_verdict(&g, 0, 1) {
        CaptureVerdict::Hold(Some(reading)) => {
            assert_eq!(reading.city, cid);
            assert_eq!(reading.window, WALLED_WINDOW);
            assert_eq!(reading.walls, 200);
            assert!(reading.needs_breaker, "{reading:?}");
            assert_eq!(reading.short_of, Some(ShortOf::Breaker));
        }
        other => panic!("walls and no breaker: {other:?}"),
    }
    assert!(ai.declaration_lacks_a_capture(&g, 0, 1));
}

#[test]
fn a_breaker_in_the_corps_opens_a_walled_city() {
    let (mut g, _) = front(100, 25.0);
    army(&mut g, &["swordsman"; 6], 2);
    army(&mut g, &["catapult"; 3], 3);
    let ai = seat(true);
    let verdict = ai.capture_verdict(&g, 0, 1);
    let reading = match &verdict {
        CaptureVerdict::Go(reading) | CaptureVerdict::Hold(Some(reading)) => reading.clone(),
        CaptureVerdict::Hold(None) => panic!("an army in reach reads"),
    };
    assert!(!reading.needs_breaker, "catapults open walls: {verdict:?}");
    assert_eq!(
        matches!(verdict, CaptureVerdict::Go(_)),
        reading.chance >= DECLARE_P,
        "with a breaker the reading alone decides: {verdict:?}"
    );
}

#[test]
fn a_war_already_running_and_the_faith_opening_are_not_gated() {
    let (mut g, _) = front(0, 20.0);
    let ai = seat(true);
    assert!(
        declares(&ai.faith_war_opening(&g, 0, 1)),
        "the faith war is exempt"
    );
    g.at_war.insert((0, 1));
    assert!(!ai.declaration_lacks_a_capture(&g, 0, 1));
}

#[test]
fn the_raw_bar_calibrates_to_the_declaration_bar() {
    assert_eq!(raw_for(0.1, 0), 0.1);
    for walls in [0, 100, 200, 400] {
        let raw = raw_for(DECLARE_P, walls);
        assert!(raw > DECLARE_P && raw <= 1.0, "walls {walls}: {raw}");
    }
    assert!(raw_for(DECLARE_P, 0) < raw_for(DECLARE_P, 400));
}
