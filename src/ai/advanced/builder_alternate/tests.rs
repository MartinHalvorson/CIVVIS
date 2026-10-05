use super::*;

fn fixture() -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 24, 16, 91_617, 150, 0, true);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
        tile.resource = None;
        tile.improvement = Some(crate::name!("farm"));
    }
    g.current = 0;
    let founder = g.spawn_test_unit("settler", 0, (5, 5));
    g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.cities.get_mut(&city).unwrap().pop = 2;
    g.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("bronze_working")]);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let start = (5, 4);
    let alternate = (4, 4);
    let preferred = (7, 4);
    for pos in [alternate, preferred] {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.resource = Some(crate::name!("iron"));
        tile.owner_city = Some(city);
        if !g.cities[&city].owned_tiles.contains(&pos) {
            g.cities.get_mut(&city).unwrap().owned_tiles.push(pos);
        }
    }
    for pos in g.nbrs(preferred) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    let builder = g.spawn_test_unit("builder", 0, start);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_builder_productive_alternate();
    ai.builder_targets.insert(builder, preferred);
    (g, ai, builder, alternate, preferred)
}

#[test]
fn a_refused_preferred_route_completes_productive_work_this_turn() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    let city = g.map.tiles[&alternate].owner_city.unwrap();
    assert!(g.city_citizen_plan(city).worked_tiles.contains(&alternate));
    let before = g.city_yields(city);
    let charges = g.units[&builder].charges;
    assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science));
    assert_eq!(
        g.map.tiles[&alternate].improvement,
        Some(crate::name!("mine"))
    );
    assert_eq!(g.units[&builder].charges, charges - 1);
    assert!(g.city_yields(city).production > before.production);
    assert!(g.city_yields(city).food >= before.food);
    assert!(!ai.builder_targets.contains_key(&builder));
}

#[test]
fn disabled_fallback_preserves_the_refused_route_and_board() {
    let (mut g, mut ai, builder, _, preferred) = fixture();
    ai.disable_builder_productive_alternate();
    let before = serde_json::to_vec(&g).unwrap();
    assert!(!ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science));
    assert_eq!(serde_json::to_vec(&g).unwrap(), before);
    assert_eq!(ai.builder_targets.get(&builder), Some(&preferred));
}

#[test]
fn a_reserved_alternate_is_left_for_its_builder() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    let other = g.spawn_test_unit("builder", 0, (6, 5));
    ai.builder_targets.insert(other, alternate);
    let charges = g.units[&builder].charges;
    assert!(!ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science));
    assert_eq!(g.units[&builder].charges, charges);
    assert!(g.map.tiles[&alternate].improvement.is_none());
}

#[test]
fn exhausted_movement_cannot_be_borrowed_for_the_operation() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    g.map.tiles.get_mut(&alternate).unwrap().hills = true;
    let charges = g.units[&builder].charges;
    assert!(ai.builder_productive_alternate_step(
        &mut g,
        0,
        builder,
        GrandStrategy::Science,
        &HashSet::new()
    ));
    assert_eq!(g.units[&builder].pos, alternate);
    assert_eq!(g.units[&builder].moves_left, 0.0);
    assert_eq!(g.units[&builder].charges, charges);
    assert!(g.map.tiles[&alternate].improvement.is_none());
    assert_eq!(
        ai.builder_prepared_alternate_step(&mut g, 0, builder),
        Some(false)
    );
    assert_eq!(g.units[&builder].charges, charges);
    // Renew only the test world's native turn allowance, as the engine does.
    g.turn += 1;
    let allowance = g.unit_max_moves(builder);
    g.units.get_mut(&builder).unwrap().moves_left = allowance;
    assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science));
    assert_eq!(g.units[&builder].charges, charges - 1);
    assert_eq!(
        g.map.tiles[&alternate].improvement,
        Some(crate::name!("mine"))
    );
    assert!(!ai.builder_alternate_pending.contains_key(&builder));
}

#[test]
fn prepared_work_rechecks_a_new_capture_threat() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    g.map.tiles.get_mut(&alternate).unwrap().hills = true;
    assert!(ai.builder_productive_alternate_step(
        &mut g,
        0,
        builder,
        GrandStrategy::Science,
        &HashSet::new()
    ));
    g.turn += 1;
    let allowance = g.unit_max_moves(builder);
    g.units.get_mut(&builder).unwrap().moves_left = allowance;
    let barb = g.barb_pid.unwrap();
    g.spawn_test_unit("warrior", barb, (3, 4));
    let before = serde_json::to_vec(&g).unwrap();
    assert_eq!(ai.builder_prepared_alternate_step(&mut g, 0, builder), None);
    assert_eq!(serde_json::to_vec(&g).unwrap(), before);
    assert!(g.map.tiles[&alternate].improvement.is_none());
    assert!(!ai.builder_alternate_pending.contains_key(&builder));
}

#[test]
fn prepared_work_rechecks_the_production_payoff() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    g.map.tiles.get_mut(&alternate).unwrap().hills = true;
    assert!(ai.builder_productive_alternate_step(
        &mut g,
        0,
        builder,
        GrandStrategy::Science,
        &HashSet::new()
    ));
    g.turn += 1;
    let allowance = g.unit_max_moves(builder);
    g.units.get_mut(&builder).unwrap().moves_left = allowance;
    // Another accepted operation already delivered the quoted improvement.
    g.map.tiles.get_mut(&alternate).unwrap().improvement = Some(crate::name!("mine"));
    let before = serde_json::to_vec(&g).unwrap();
    assert_eq!(ai.builder_prepared_alternate_step(&mut g, 0, builder), None);
    assert_eq!(serde_json::to_vec(&g).unwrap(), before);
    assert!(!ai.builder_alternate_pending.contains_key(&builder));
}

#[test]
fn a_setup_too_late_to_finish_before_t75_is_refused() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    g.map.tiles.get_mut(&alternate).unwrap().hills = true;
    g.turn = 74;
    let before = serde_json::to_vec(&g).unwrap();
    assert!(!ai.builder_productive_alternate_step(
        &mut g,
        0,
        builder,
        GrandStrategy::Science,
        &HashSet::new()
    ));
    assert_eq!(serde_json::to_vec(&g).unwrap(), before);
}

#[test]
fn known_capture_reach_refuses_the_alternate_without_spending_a_charge() {
    let (mut g, mut ai, builder, alternate, _) = fixture();
    let barb = g.barb_pid.unwrap();
    let raider = g.spawn_test_unit("warrior", barb, (3, 4));
    g.players[0].explored.insert(g.units[&raider].pos);
    let before = serde_json::to_vec(&g).unwrap();
    assert!(!ai.builder_productive_alternate_step(
        &mut g,
        0,
        builder,
        GrandStrategy::Science,
        &HashSet::new()
    ));
    assert_eq!(serde_json::to_vec(&g).unwrap(), before);
    assert!(g.map.tiles[&alternate].improvement.is_none());
}

#[test]
fn adaptive_policy_does_not_get_the_named_lane_fallback() {
    let (mut g, _, builder, _, _) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_builder_productive_alternate();
    let before = serde_json::to_vec(&g).unwrap();
    assert!(!ai.builder_productive_alternate_step(
        &mut g,
        0,
        builder,
        GrandStrategy::Science,
        &HashSet::new()
    ));
    assert_eq!(serde_json::to_vec(&g).unwrap(), before);
}
