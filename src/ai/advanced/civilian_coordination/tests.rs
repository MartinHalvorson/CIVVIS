use super::*;

fn fixture() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut game = Game::new_full(2, 24, 16, 91_515, 150, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
        tile.resource = None;
        tile.improvement = None;
    }
    game.current = 0;
    let founder = game.spawn_test_unit("settler", 0, (2, 5));
    game.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let builder = game.spawn_test_unit("builder", 0, (5, 5));
    let guard = game.spawn_test_unit("warrior", 0, (5, 5));
    let hostile = game.spawn_test_unit("slinger", 1, (6, 5));
    game.units.get_mut(&builder).unwrap().moves_left = 1.0;
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    game.players[1].is_barbarian = true;
    game.barb_pid = Some(1);
    let mut ai = AdvancedAi::new();
    ai.enable_live_settler_capture_lessons();
    (game, ai, builder, guard, hostile)
}

fn strategy() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

#[test]
fn joint_walk_uses_the_slower_units_budget_and_reserves_the_guard() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    let start = game.units[&builder].pos;
    ai.plan_builder_support(&game, 0);
    let support = ai.builder_support[&builder];
    assert_eq!(support.guard, guard);
    assert_ne!(support.destination, start, "the pair retreats toward home");
    assert!(game.wdist(start, support.destination) <= 1);
    assert!(ai.guard_is_reserved_for_civilian(guard));
    assert!(ai.all_reserved_civilian_guards().contains(&guard));
    assert_eq!(ai.builder_support_step(&mut game, 0, builder), Some(true));
    assert_eq!(game.units[&builder].pos, support.destination);
    assert_eq!(game.units[&guard].pos, support.destination);
    assert!(ai.builder_support_protects(&game, 0, builder, support.destination));
    ai.advanced_military_step_with_decline(&mut game, 0, guard, &strategy(), true);
    assert_eq!(game.units[&guard].pos, support.destination);
}

#[test]
fn both_military_prepasses_leave_the_reserved_guard_available() {
    let (mut game, mut ai, builder, guard, hostile) = fixture();
    game.units.get_mut(&hostile).unwrap().hp = 1;
    ai.plan_builder_support(&game, 0);
    assert!(ai.builder_support.contains_key(&builder));
    let start = game.units[&guard].pos;
    let moves = game.units[&guard].moves_left;
    ai.prioritize_immediate_kills(&mut game, 0, &strategy(), &BTreeSet::new());
    assert!(game.units.contains_key(&hostile));
    ai.fire_plan = true;
    ai.plan_fire(&game, 0);
    assert_eq!(ai.fire_plan_target(guard), None);
    ai.battle_planner = true;
    ai.plan_battle(&mut game, 0, &strategy());
    assert!(game.units.contains_key(&hostile));
    assert_eq!(game.units[&guard].pos, start);
    assert_eq!(game.units[&guard].moves_left, moves);
}

#[test]
fn idle_builder_keeps_a_guard_and_a_fresh_frame_releases_it_when_safe() {
    let (mut game, mut ai, builder, guard, hostile) = fixture();
    game.units.get_mut(&builder).unwrap().moves_left = 0.0;
    ai.plan_builder_support(&game, 0);
    assert_eq!(
        ai.builder_support[&builder].destination,
        game.units[&builder].pos
    );
    assert!(ai.guard_is_reserved_for_civilian(guard));
    ai.advanced_military_step_with_decline(&mut game, 0, guard, &strategy(), true);
    assert_eq!(game.units[&guard].pos, game.units[&builder].pos);
    game.remove_unit(hostile);
    ai.plan_builder_support(&game, 0);
    assert!(ai.builder_support.is_empty());
    assert!(!ai.guard_is_reserved_for_civilian(guard));
}

#[test]
fn a_safe_independent_escape_does_not_consume_a_guard() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    game.units.get_mut(&builder).unwrap().moves_left = 3.0;
    ai.plan_builder_support(&game, 0);
    assert!(ai.builder_support.is_empty());
    assert!(!ai.guard_is_reserved_for_civilian(guard));
}

#[test]
fn settler_bindings_and_doomed_guards_are_not_borrowed() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    let settler = game.spawn_test_unit("settler", 0, (3, 5));
    ai.settler_guards.insert(settler, guard);
    ai.plan_builder_support(&game, 0);
    assert!(ai.builder_support.is_empty());
    assert_eq!(ai.settler_guards[&settler], guard);
    ai.settler_guards.clear();
    game.units.get_mut(&guard).unwrap().hp = 1;
    ai.plan_builder_support(&game, 0);
    assert!(!ai.builder_support.contains_key(&builder));
}

#[test]
fn an_invalidated_pair_does_not_send_only_half_of_the_walk() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    ai.plan_builder_support(&game, 0);
    assert!(ai.builder_support.contains_key(&builder));
    let start = game.units[&guard].pos;
    game.units.get_mut(&builder).unwrap().moves_left = 0.0;
    assert_eq!(ai.builder_support_step(&mut game, 0, builder), Some(false));
    assert_eq!(game.units[&guard].pos, start);
    assert_eq!(ai.builder_support[&builder].destination, start);
    assert!(ai.guard_is_reserved_for_civilian(guard));
    ai.advanced_military_step_with_decline(&mut game, 0, guard, &strategy(), true);
    assert_eq!(game.units[&guard].pos, game.units[&builder].pos);
}

#[test]
fn ordinary_native_boards_keep_the_existing_policy() {
    let (game, _, _, _, _) = fixture();
    let mut native = AdvancedAi::new();
    native.plan_builder_support(&game, 0);
    assert!(native.builder_support.is_empty());
}

#[test]
fn the_real_unit_turn_builds_and_honors_the_shared_plan() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    ai.advanced_units(&mut game, 0, &strategy());
    let support = ai.builder_support[&builder];
    assert_eq!(game.units[&builder].pos, support.destination);
    assert_eq!(game.units[&guard].pos, support.destination);
}

#[test]
fn a_guard_without_movement_cannot_promise_to_follow() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    let start = game.units[&builder].pos;
    game.units.get_mut(&guard).unwrap().moves_left = 0.0;
    ai.plan_builder_support(&game, 0);
    assert_eq!(ai.builder_support[&builder].destination, start);
    assert_eq!(ai.builder_support_step(&mut game, 0, builder), None);
    assert_eq!(game.units[&builder].pos, start);
}

#[test]
fn fresh_host_id_mapping_discards_old_frame_reservations() {
    let (game, mut ai, builder, guard, _) = fixture();
    ai.plan_builder_support(&game, 0);
    assert!(ai.builder_support.contains_key(&builder));
    ai.remap_unit_memory(&BTreeMap::from([
        (builder, builder + 100),
        (guard, guard + 100),
    ]));
    assert!(ai.builder_support.is_empty());
    assert!(!ai.guard_is_reserved_for_civilian(guard));
}

#[test]
fn two_pairs_reserve_distinct_end_tiles() {
    let (mut game, mut ai, builder, guard, _) = fixture();
    let second = game.spawn_test_unit("builder", 0, (5, 4));
    let second_guard = game.spawn_test_unit("warrior", 0, (5, 4));
    game.units.get_mut(&second).unwrap().moves_left = 1.0;
    let shared_refuge = (4, 5);
    for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if ![(2, 5), (5, 5), (5, 4), (6, 5), shared_refuge].contains(&pos) {
            game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
        }
    }
    ai.plan_builder_support(&game, 0);
    let first_plan = ai.builder_support[&builder];
    let second_plan = ai.builder_support[&second];
    assert_eq!(first_plan.destination, shared_refuge);
    assert_ne!(
        first_plan.destination, second_plan.destination,
        "two civilians cannot promise to occupy the same end tile"
    );
    ai.builder_support_step(&mut game, 0, builder);
    ai.builder_support_step(&mut game, 0, second);
    assert_eq!(game.units[&builder].pos, game.units[&guard].pos);
    assert_eq!(game.units[&second].pos, game.units[&second_guard].pos);
}

fn productive_fixture() -> (Game, AdvancedAi, u32, u32) {
    let (mut g, _, _, _, _) = fixture();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let city = g.player_city_ids(0)[0];
    g.cities.get_mut(&city).unwrap().pop = 2;
    g.map.tiles.get_mut(&(3, 5)).unwrap().hills = true;
    g.players[0].techs.insert(crate::name!("mining"));
    let builder = g.spawn_test_unit("builder", 0, (2, 5));
    let guard = g.spawn_test_unit("warrior", 0, (3, 4));
    g.spawn_test_unit("slinger", 1, (4, 5));
    g.turn = 20;
    assert!(g.city_citizen_plan(city).worked_tiles.contains(&(3, 5)));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_live_settler_capture_lessons();
    (g, ai, builder, guard)
}

#[test]
fn productive_job_is_opt_in_and_finishes_both_walks_and_the_mine() {
    let (mut g, mut ai, builder, guard) = productive_fixture();
    ai.plan_productive_builder_support(&g, 0, &strategy());
    assert!(ai.builder_support.is_empty());
    assert!(!ai.productive_builder_escort_enabled());
    ai.enable_productive_builder_escort();
    ai.plan_productive_builder_support(&g, 0, &strategy());
    let support = ai.builder_support[&builder];
    assert_eq!(support.guard, guard);
    assert_eq!(support.destination, (3, 5));
    assert_eq!(support.job, Some(crate::name!("mine")));
    let city = g.player_city_ids(0)[0];
    let before = g.city_yields(city).production;
    assert_eq!(ai.builder_support_step(&mut g, 0, builder), Some(true));
    assert_eq!(g.units[&builder].pos, (3, 5));
    assert_eq!(g.units[&guard].pos, (3, 5));
    assert_eq!(g.map.tiles[&(3, 5)].improvement, Some(crate::name!("mine")));
    assert!(g.city_yields(city).production > before);
    assert!(ai.builder_support_protects(&g, 0, builder, (3, 5)));
}

#[test]
fn productive_job_does_not_borrow_a_settler_guard_or_a_doomed_guard() {
    let (mut g, mut ai, builder, guard) = productive_fixture();
    ai.enable_productive_builder_escort();
    let settler = g.spawn_test_unit("settler", 0, (3, 4));
    ai.settler_guards.insert(settler, guard);
    ai.plan_productive_builder_support(&g, 0, &strategy());
    assert!(!ai.builder_support.contains_key(&builder));
    ai.settler_guards.clear();
    g.units.get_mut(&guard).unwrap().hp = 1;
    ai.plan_productive_builder_support(&g, 0, &strategy());
    assert!(!ai.builder_support.contains_key(&builder));
}

#[test]
fn productive_job_keeps_the_final_garrison_in_its_city() {
    let (mut g, mut ai, builder, guard) = productive_fixture();
    ai.enable_productive_builder_escort();
    g.remove_unit(guard);
    let guard = g.spawn_test_unit("warrior", 0, (2, 5));
    ai.plan_productive_builder_support(&g, 0, &strategy());
    assert!(!ai.builder_support.contains_key(&builder));
    assert_eq!(g.units[&guard].pos, (2, 5));
}

#[test]
fn invalidated_productive_job_executes_neither_half_of_the_walk() {
    let (mut g, mut ai, builder, guard) = productive_fixture();
    ai.enable_productive_builder_escort();
    ai.plan_productive_builder_support(&g, 0, &strategy());
    assert!(ai.builder_support.contains_key(&builder));
    g.units.get_mut(&builder).unwrap().moves_left = 0.0;
    assert_eq!(ai.builder_support_step(&mut g, 0, builder), None);
    assert_eq!(g.units[&guard].pos, (3, 4));
    assert_eq!(g.units[&builder].pos, (2, 5));
    assert!(g.map.tiles[&(3, 5)].improvement.is_none());
}

#[test]
fn productive_job_reaches_the_full_unit_driver_before_military_spends_its_guard() {
    let (mut g, mut ai, builder, guard) = productive_fixture();
    ai.enable_productive_builder_escort();
    ai.advanced_units(&mut g, 0, &strategy());
    assert_eq!(g.map.tiles[&(3, 5)].improvement, Some(crate::name!("mine")));
    assert_eq!(g.units[&builder].pos, g.units[&guard].pos);
    assert!(ai.guard_is_reserved_for_civilian(guard));
}
