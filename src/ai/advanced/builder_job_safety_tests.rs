use super::*;

fn ranged_pressure_jobs() -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut game = Game::new_full(2, 24, 16, 43_252_000, 150, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    game.current = 0;
    let founder = game.spawn_test_unit("settler", 0, (5, 4));
    game.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let city = game.player_city_ids(0)[0];
    let preferred = (6, 5);
    let alternate = (5, 6);
    for job in [preferred, alternate] {
        let tile = game.map.tiles.get_mut(&job).unwrap();
        tile.owner_city = Some(city);
        tile.improvement = None;
        if !game.cities[&city].owned_tiles.contains(&job) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(job);
        }
    }
    game.map.tiles.get_mut(&preferred).unwrap().resource = Some(crate::name!("iron"));
    game.map.tiles.get_mut(&alternate).unwrap().hills = true;
    game.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("bronze_working")]);
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    game.players[1].is_barbarian = true;
    game.barb_pid = Some(1);
    game.spawn_test_unit("archer", 1, (8, 2));
    game.spawn_test_unit("scout", 0, (7, 2));
    let builder = game.spawn_test_unit("builder", 0, (5, 5));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_builder_barbarian_safety();
    ai.enable_builder_tries_the_next_tile();
    ai.enable_builders_work_through_raiders();
    (game, ai, builder, preferred, alternate)
}

fn assert_safe_retreat_fixture(game: &Game, ai: &AdvancedAi, builder: u32, preferred: Pos) {
    let visible = ai.battlefront_visibility(game, 0);
    assert_eq!(
        ai.builder_barbarian_capture_risk(game, 0, builder, preferred, &visible),
        0.0,
        "ranged pressure, not a direct-capture threshold, blocks productive progress"
    );
    assert_eq!(game.route_step(builder, preferred, 0), Some(preferred));
    assert!(game.can_move(builder, preferred));
    let mut control = game.clone();
    let before = control.units[&builder].pos;
    assert!(ai.base.step_toward(&mut control, 0, builder, preferred));
    let after = control.units[&builder].pos;
    assert_ne!(after, before);
    assert_ne!(after, preferred);
    assert!(game.wdist(after, preferred) >= game.wdist(before, preferred));
}

#[test]
fn a_safe_retreat_does_not_keep_a_pinned_improvement_job() {
    let (mut game, mut ai, builder, preferred, alternate) = ranged_pressure_jobs();
    assert_safe_retreat_fixture(&game, &ai, builder, preferred);
    ai.builder_targets.insert(builder, preferred);
    let charges = game.units[&builder].charges;
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert!(!ai.builder_targets.contains_key(&builder));
    assert_eq!(
        ai.builder_avoid.get(&builder),
        Some(&(
            preferred,
            game.turn + game.standard_duration(SETTLER_TARGET_HYSTERESIS_TURNS)
        ))
    );
    assert_eq!(game.units[&builder].charges, charges);
    assert!(game.map.tiles[&preferred].improvement.is_none());
    assert_eq!(game.units[&builder].pos, alternate);
    game.turn += 1;
    let unit = game.units.get_mut(&builder).unwrap();
    unit.moves_left = 2.0;
    unit.moved = false;
    unit.acted = false;
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(
        game.map.tiles[&alternate].improvement,
        Some(crate::name!("mine"))
    );
    assert_eq!(game.units[&builder].charges, charges - 1);
}

#[test]
fn a_new_improvement_assignment_does_not_pin_a_safe_retreat() {
    let (mut game, mut ai, builder, preferred, _) = ranged_pressure_jobs();
    assert_safe_retreat_fixture(&game, &ai, builder, preferred);
    assert_eq!(
        ai.builder_jobs_ranked(&game, 0, builder, GrandStrategy::Expansion, &HashSet::new())[0],
        preferred
    );
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert!(!ai.builder_targets.contains_key(&builder));
    assert_eq!(ai.builder_avoid[&builder].0, preferred);
}

#[test]
fn the_original_job_returns_after_the_temporary_backoff_expires() {
    let (mut game, mut ai, builder, preferred, _) = ranged_pressure_jobs();
    let raiders = game
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.id)
        .collect::<Vec<_>>();
    for raider in raiders {
        game.remove_unit(raider);
    }
    ai.builder_avoid.insert(builder, (preferred, game.turn));
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(game.units[&builder].pos, preferred);
    assert_eq!(ai.builder_targets.get(&builder), Some(&preferred));
}

#[test]
fn disabled_next_job_behavior_keeps_the_historical_assignment() {
    let (mut game, mut ai, builder, preferred, _) = ranged_pressure_jobs();
    ai.base.builder_tries_the_next_tile = false;
    ai.builder_targets.insert(builder, preferred);
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(ai.builder_targets.get(&builder), Some(&preferred));
    assert!(ai.builder_avoid.is_empty());
}

#[test]
fn a_routed_terrain_detour_keeps_its_improvement_assignment() {
    let (mut game, mut ai, builder, old_job, alternate) = ranged_pressure_jobs();
    let raiders = game
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.id)
        .collect::<Vec<_>>();
    for raider in raiders {
        game.remove_unit(raider);
    }
    game.map.tiles.get_mut(&old_job).unwrap().terrain = crate::name!("mountain");
    game.map.tiles.get_mut(&alternate).unwrap().improvement = Some(crate::name!("mine"));
    let city = game.map.tiles[&old_job].owner_city.unwrap();
    let target = (7, 5);
    let tile = game.map.tiles.get_mut(&target).unwrap();
    tile.owner_city = Some(city);
    tile.resource = Some(crate::name!("iron"));
    tile.improvement = None;
    game.cities.get_mut(&city).unwrap().owned_tiles.push(target);
    let before = game.units[&builder].pos;
    let routed = game.route_step(builder, target, 0).unwrap();
    assert!(game.wdist(routed, target) >= game.wdist(before, target));
    ai.builder_targets.insert(builder, target);
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(game.units[&builder].pos, routed);
    assert_eq!(ai.builder_targets.get(&builder), Some(&target));
    assert!(ai.builder_avoid.is_empty());
}

#[test]
fn leaving_local_work_for_a_better_job_also_records_a_retreat() {
    let (mut game, mut ai, builder, preferred, _) = ranged_pressure_jobs();
    let before = game.units[&builder].pos;
    game.map.tiles.get_mut(&before).unwrap().improvement = None;
    let charges = game.units[&builder].charges;
    assert!(ai
        .more_useful_builder_job(&mut game, 0, builder, GrandStrategy::Expansion)
        .unwrap());
    assert_eq!(game.units[&builder].charges, charges);
    assert!(!ai.builder_targets.contains_key(&builder));
    assert_eq!(ai.builder_avoid[&builder].0, preferred);
}
