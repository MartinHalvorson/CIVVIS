use super::*;

fn set_faith(g: &mut Game, cid: u32, faith: &str) {
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 4;
    city.atheist_pressure = 0.0;
    city.pressure.clear();
    city.pressure.insert(faith.into(), 1000.0);
}

fn fixture(faith: &str, adjacent: bool) -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(4, 36, 26, 374600, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    for (owner, pos, religion) in [
        (0, (5, 5), "Catholicism"),
        (0, (10, 5), "Catholicism"),
        (0, (5, 11), "Taoism"),
        (1, (23, 5), "Catholicism"),
        (2, (23, 13), "Taoism"),
        (3, (16, 20), "Catholicism"),
    ] {
        let cid = g.found_city_for(owner, pos, None);
        set_faith(&mut g, cid, religion);
    }
    g.players[0].religion = None;
    g.players[1].religion = Some("Catholicism".into());
    g.players[2].religion = Some("Taoism".into());
    g.at_war.insert((0, 2));
    g.current = 0;
    let guard = g.spawn_test_unit("warrior", 0, (6, 5));
    let missionary = g.spawn_test_unit("missionary", 2, if adjacent { (7, 5) } else { (6, 5) });
    g.units.get_mut(&missionary).unwrap().religion = Some(faith.into());
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        guard,
        missionary,
    )
}

#[test]
fn safe_competing_missionary_survives_without_spending_guard_moves() {
    for adjacent in [false, true] {
        let (mut g, mut ai, guard, missionary) = fixture("Taoism", adjacent);
        let before = (g.units[&guard].pos, g.units[&guard].moves_left);
        assert!(!ai.condemn_step(&mut g, 0, guard));
        assert!(g.units.contains_key(&missionary));
        assert_eq!((g.units[&guard].pos, g.units[&guard].moves_left), before);
    }
}

#[test]
fn dominant_foreign_faith_is_still_intercepted_even_by_another_owner() {
    let (mut g, mut ai, guard, missionary) = fixture("Catholicism", true);
    assert!(ai.condemn_step(&mut g, 0, guard));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn alternative_at_its_own_match_point_is_not_protected() {
    let (mut g, mut ai, guard, missionary) = fixture("Taoism", false);
    for cid in g.player_city_ids(1).into_iter().chain(g.player_city_ids(3)) {
        set_faith(&mut g, cid, "Taoism");
    }
    assert!(!AdvancedAi::safe_adopted_counterfaith(&g, 0, "Taoism"));
    assert!(ai.condemn_step(&mut g, 0, guard));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn founder_other_lane_and_disabled_religious_victory_keep_condemnation() {
    for case in 0..3 {
        let (mut g, mut ai, guard, missionary) = fixture("Taoism", false);
        match case {
            0 => g.players[0].religion = Some("Our Faith".into()),
            1 => ai = AdvancedAi::targeting(VictoryTarget::Culture),
            _ => g.victory_conditions.religious = false,
        }
        assert!(ai.condemn_step(&mut g, 0, guard));
        assert!(!g.units.contains_key(&missionary));
    }
}

#[test]
fn no_dominant_home_threat_keeps_condemnation() {
    let (mut g, mut ai, guard, missionary) = fixture("Taoism", false);
    for cid in g.player_city_ids(0) {
        g.cities.get_mut(&cid).unwrap().pressure.clear();
    }
    assert!(ai.condemn_step(&mut g, 0, guard));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn competing_faith_far_from_home_keeps_condemnation() {
    let (mut g, mut ai, guard, missionary) = fixture("Taoism", false);
    g.remove_unit(guard);
    g.remove_unit(missionary);
    let guard = g.spawn_test_unit("warrior", 0, (24, 19));
    let missionary = g.spawn_test_unit("missionary", 2, (24, 19));
    g.units.get_mut(&missionary).unwrap().religion = Some("Taoism".into());
    assert!(ai.condemn_step(&mut g, 0, guard));
    assert!(!g.units.contains_key(&missionary));
}
