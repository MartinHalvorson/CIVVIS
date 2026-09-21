use super::*;

fn policy_board() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 24, 16, 364900, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let city = g.found_city_for(0, (6, 8), None);
    g.players[0].government = Some("chiefdom".into());
    g.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("state_workforce"),
        crate::name!("medieval_faires"),
        crate::name!("the_enlightenment"),
    ]);
    g.players[0].policies.clear();
    g.players[0].gold = 100.0;
    g.current = 0;
    g.turn = 100;
    g.at_war.clear();
    for _ in 0..2 {
        g.spawn_test_unit("warrior", 0, (6, 8));
    }
    (g, AdvancedAi::targeting(VictoryTarget::Domination), city)
}

#[test]
fn empty_multipliers_yield_to_a_productive_fallback_without_mutating_the_board() {
    let (g, ai, _) = policy_board();
    let before = g.players[0].policies.clone();
    let mut desired = vec!["aesthetics", "rationalism", "logistics"];
    let zero = ai.domination_productive_policy_fallbacks(&g, 0, &mut desired);
    assert!(zero.contains("aesthetics"));
    assert!(zero.contains("rationalism"));
    assert_eq!(desired, vec!["logistics", "urban_planning"]);
    assert_eq!(g.players[0].policies, before);
}

#[test]
fn strategic_policy_pass_replaces_empty_aesthetics_and_keeps_the_military_slot() {
    let (mut g, ai, _) = policy_board();
    g.players[0]
        .policies
        .extend([crate::name!("aesthetics"), crate::name!("conscription")]);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Conquest);
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("urban_planning")));
    assert!(!g.players[0].policies.contains(&crate::name!("aesthetics")));
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("conscription")));
}

#[test]
fn rationalism_eligibility_tracks_its_actual_population_threshold() {
    let (mut g, ai, cid) = policy_board();
    crate::game::install_test_district(&mut g, cid, "campus");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    g.cities.get_mut(&cid).unwrap().pop = 4;
    let mut low = vec!["rationalism"];
    assert!(ai
        .domination_productive_policy_fallbacks(&g, 0, &mut low)
        .contains("rationalism"));
    g.cities.get_mut(&cid).unwrap().pop = 15;
    let mut high = vec!["rationalism"];
    assert!(ai
        .domination_productive_policy_fallbacks(&g, 0, &mut high)
        .is_empty());
    assert_eq!(high, vec!["rationalism"]);
}

#[test]
fn another_victory_lane_retains_its_existing_policy_preferences() {
    let (g, _, _) = policy_board();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let mut desired = vec!["aesthetics", "rationalism"];
    assert!(ai
        .domination_productive_policy_fallbacks(&g, 0, &mut desired)
        .is_empty());
    assert_eq!(desired, vec!["aesthetics", "rationalism"]);
}
