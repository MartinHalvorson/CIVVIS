use super::*;
use crate::ai::VictoryTarget;
use crate::name;

fn fixture() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 91919, 1000, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(0, (6, 18), None);
    let target = g.found_city_for(1, (18, 12), None);
    g.record_contact(0, 1);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.turn = 30;
    g.current = 0;
    let uid = g.spawn_test_unit("archer", 0, (6, 12));
    let settler = g.spawn_test_unit("settler", 0, (6, 12));
    let spare = g.spawn_test_unit("warrior", 0, (7, 12));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.live_formationless_settler_shadow = true;
    ai.settler_targets.insert(settler, (12, 18));
    ai.city_target_floor = 6;
    ai.conquest_opening = Some(ConquestOpening {
        target: 1,
        city: target,
        opened: 19,
        rally: (15, 12),
        force: [uid].into_iter().collect(),
        assembled: Some(28),
        declared: Some(29),
        kills_at_war: 0,
        losses: 0,
        taken: 0,
    });
    ai.conquest_pin_the_campaign(&g);
    (g, ai, settler, uid, spare)
}

#[test]
fn new_settler_uses_the_spare_instead_of_a_committed_shooter() {
    let (g, ai, settler, shooter, spare) = fixture();
    let mut off = ai.clone();
    off.disable_early_conquest_opening();
    off.stacked_escort_pace(&mut g.clone(), 0, settler);
    assert_eq!(off.settler_guards.get(&settler), Some(&shooter));
    let mut on = ai;
    on.stacked_escort_pace(&mut g.clone(), 0, settler);
    assert_eq!(on.settler_guards.get(&settler), Some(&spare));
}

#[test]
fn existing_guard_is_not_stripped_from_its_settler() {
    let (mut g, mut ai, settler, shooter, _) = fixture();
    ai.bind_settler_guard(&g, settler, shooter);
    ai.stacked_escort_pace(&mut g, 0, settler);
    assert_eq!(ai.settler_guards.get(&settler), Some(&shooter));
}

#[test]
fn an_opening_that_no_longer_stands_releases_escort_candidates() {
    for case in 0..4 {
        let (mut g, mut ai, settler, shooter, _) = fixture();
        match case {
            0 => ai.conquest_opening.as_mut().unwrap().declared = None,
            1 => g.at_war.clear(),
            2 => ai.conquest_opening = None,
            3 => {
                let city = ai.conquest_opening.as_ref().unwrap().city;
                g.cities.get_mut(&city).unwrap().owner = 0;
            }
            _ => unreachable!(),
        }
        ai.stacked_escort_pace(&mut g, 0, settler);
        assert_eq!(
            ai.settler_guards.get(&settler),
            Some(&shooter),
            "case {case}"
        );
    }
}

#[test]
fn no_spare_guard_does_not_dismantle_the_committed_force() {
    let (mut g, mut ai, settler, shooter, spare) = fixture();
    g.remove_unit(spare);
    ai.stacked_escort_pace(&mut g, 0, settler);
    assert!(!ai.settler_guards.contains_key(&settler));
    assert!(ai
        .conquest_opening
        .as_ref()
        .unwrap()
        .force
        .contains(&shooter));
}

#[test]
fn formation_escort_selection_also_preserves_the_committed_shooter() {
    let (g, mut ai, settler, shooter, _) = fixture();
    ai.live_formationless_settler_shadow = false;
    ai.settlement_safety = true;
    let plan = ai.assess(&g, 0);
    let mut off = ai.clone();
    off.disable_early_conquest_opening();
    let mut before = g.clone();
    assert_eq!(
        off.settler_escort_step(&mut before, 0, shooter, &plan),
        Some(true)
    );
    assert_eq!(before.units[&shooter].linked_to, Some(settler));
    let mut after = g;
    assert_eq!(ai.settler_escort_step(&mut after, 0, shooter, &plan), None);
    assert_eq!(after.units[&shooter].linked_to, None);
}
