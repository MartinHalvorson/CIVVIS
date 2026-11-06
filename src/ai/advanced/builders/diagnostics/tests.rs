use super::*;

fn fixture() -> (Game, AdvancedAi, u32, Pos) {
    let mut g = Game::new_full(2, 16, 12, 4708, 150, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.current = 0;
    let founder = g.spawn_test_unit("settler", 0, (5, 5));
    g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.cities.get_mut(&city).unwrap().pop = 3;
    g.players[0].techs.extend([
        crate::name!("pottery"),
        crate::name!("irrigation"),
        crate::name!("mining"),
        crate::name!("bronze_working"),
    ]);
    let hill = (5, 4);
    assert!(g.cities[&city].owned_tiles.contains(&hill));
    g.map.tiles.get_mut(&hill).unwrap().hills = true;
    let builder = g.spawn_test_unit("builder", 0, (5, 5));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_growth_prices_the_farm_2();
    ai.enable_builders_improve_the_worked_for_production();
    ai.enable_builder_prices_travel_in_turns();
    (g, ai, builder, hill)
}

#[test]
fn diagnostics_show_real_food_and_production_alternatives_and_scores() {
    let (g, ai, builder, _) = fixture();
    let report = ai
        .builder_job_diagnostics(&g, 0, builder, GrandStrategy::Science)
        .unwrap();
    let jobs = report["jobs"].as_array().unwrap();
    assert!(jobs.iter().any(|j| j["improvement"] == "farm"));
    assert!(jobs.iter().any(|j| j["improvement"] == "mine"));
    let ranked = ai.builder_jobs_ranked(&g, 0, builder, GrandStrategy::Science, &HashSet::new());
    assert_eq!(
        report["ranked_positions_without_reservations"],
        serde_json::json!(ranked)
    );
    for job in jobs {
        let sum: f64 = [
            "intrinsic_value",
            "first_luxury_premium",
            "growth_farm_premium",
            "growing_city_food_premium",
            "worked_production_premium",
        ]
        .iter()
        .map(|key| job[*key].as_f64().unwrap())
        .sum();
        assert!((sum - job["total_improvement_value"].as_f64().unwrap()).abs() < 1e-9);
        let pos = (
            job["position"][0].as_i64().unwrap() as i32,
            job["position"][1].as_i64().unwrap() as i32,
        );
        let improvement = Name::new(job["improvement"].as_str().unwrap());
        assert!(g.valid_improvements(0, pos).contains(&improvement));
        assert_eq!(
            job["worthwhile"],
            serde_json::json!(ai
                .worthwhile_improvements(&g, 0, pos, GrandStrategy::Science)
                .contains(&improvement))
        );
    }
}

#[test]
fn diagnostics_respect_host_refusals_and_district_exclusions() {
    let (mut g, ai, builder, hill) = fixture();
    let before = ai
        .builder_job_diagnostics(&g, 0, builder, GrandStrategy::Science)
        .unwrap();
    assert!(before["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|j| j["position"] == serde_json::json!(hill)));
    Arc::make_mut(&mut g.blocked_improvement_sites).insert(hill);
    let after = ai
        .builder_job_diagnostics(&g, 0, builder, GrandStrategy::Science)
        .unwrap();
    assert!(!after["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|j| j["position"] == serde_json::json!(hill)));
    let center = g.units[&builder].pos;
    assert!(!after["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|j| j["position"] == serde_json::json!(center)));
}

#[test]
fn diagnostics_are_repeatable_and_do_not_change_the_next_builder_step() {
    let (mut g, mut ai, builder, _) = fixture();
    let mut control_game = g.clone();
    let mut control_ai = ai.clone();
    let a = ai
        .builder_job_diagnostics(&g, 0, builder, GrandStrategy::Science)
        .unwrap();
    assert_eq!(
        Some(a),
        ai.builder_job_diagnostics(&g, 0, builder, GrandStrategy::Science)
    );
    assert_eq!(
        ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science),
        control_ai.advanced_builder_step(&mut control_game, 0, builder, GrandStrategy::Science)
    );
    assert_eq!(g.units[&builder].pos, control_game.units[&builder].pos);
    assert_eq!(
        g.units[&builder].charges,
        control_game.units[&builder].charges
    );
    assert_eq!(ai.builder_targets, control_ai.builder_targets);
    for (pos, tile) in &g.map.tiles {
        assert_eq!(tile.improvement, control_game.map.tiles[pos].improvement);
    }
}

#[test]
fn diagnostics_reject_a_foreign_nonbuilder_or_exhausted_unit() {
    let (mut g, ai, builder, _) = fixture();
    assert!(ai
        .builder_job_diagnostics(&g, 1, builder, GrandStrategy::Science)
        .is_none());
    assert!(ai
        .builder_job_diagnostics(&g, 0, u32::MAX, GrandStrategy::Science)
        .is_none());
    let warrior = g.spawn_test_unit("warrior", 0, (6, 5));
    assert!(ai
        .builder_job_diagnostics(&g, 0, warrior, GrandStrategy::Science)
        .is_none());
    g.units.get_mut(&builder).unwrap().charges = 0;
    assert!(ai
        .builder_job_diagnostics(&g, 0, builder, GrandStrategy::Science)
        .is_none());
}
