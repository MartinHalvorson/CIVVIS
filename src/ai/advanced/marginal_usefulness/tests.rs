use super::*;

fn fixture() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_515, 150, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
        tile.resource = None;
        tile.improvement = None;
    }
    g.current = 0;
    let founder = g.spawn_test_unit("settler", 0, (5, 5));
    g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.cities.get_mut(&city).unwrap().pop = 2;
    let builder = g.spawn_test_unit("builder", 0, (5, 4));
    g.players[0].techs.insert(crate::name!("mining"));
    g.players[0].techs.insert(crate::name!("bronze_working"));
    let job = (6, 5);
    g.map.tiles.get_mut(&job).unwrap().resource = Some(crate::name!("iron"));
    assert!(g.cities[&city].owned_tiles.contains(&job));
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Science),
        city,
        builder,
    )
}

#[test]
fn a_builder_compares_local_work_with_a_more_useful_resource_job() {
    let (mut g, mut ai, _, builder) = fixture();
    let start = g.units[&builder].pos;
    assert!(!ai
        .worthwhile_improvements(&g, 0, start, GrandStrategy::Science)
        .is_empty());
    let charges = g.units[&builder].charges;
    assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science));
    assert_ne!(g.units[&builder].pos, start);
    assert_eq!(
        g.units[&builder].charges, charges,
        "do not spend a charge on the lesser job"
    );
    assert!(g.map.tiles[&start].improvement.is_none());
    assert_eq!(ai.builder_targets[&builder], (6, 5));
}

#[test]
fn a_reserved_better_job_does_not_take_another_builders_work() {
    let (mut g, mut ai, _, builder) = fixture();
    let other = g.spawn_test_unit("builder", 0, (6, 4));
    ai.builder_targets.insert(other, (6, 5));
    assert_eq!(
        ai.more_useful_builder_job(&mut g, 0, builder, GrandStrategy::Science),
        None
    );
}

#[test]
fn unreachable_better_work_falls_back_to_the_current_tile() {
    let (mut g, mut ai, _, builder) = fixture();
    g.map.tiles.get_mut(&(6, 5)).unwrap().terrain = crate::name!("mountain");
    let start = g.units[&builder].pos;
    assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Science));
    assert!(g.map.tiles[&start].improvement.is_some());
}

#[test]
fn existing_improvement_value_is_not_credited_to_a_replacement() {
    let (mut g, ai, _, builder) = fixture();
    let pos = g.units[&builder].pos;
    let gross = ai.improvement_value_for(&g, 0, pos, "farm", GrandStrategy::Science);
    assert!(gross > 0.0);
    g.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("farm"));
    assert_eq!(
        ai.marginal_improvement_value(&g, 0, pos, "farm", GrandStrategy::Science),
        0.0
    );
    g.map.tiles.get_mut(&pos).unwrap().pillaged = true;
    assert_eq!(
        ai.marginal_improvement_value(&g, 0, pos, "farm", GrandStrategy::Science),
        gross
    );
}

#[test]
fn queue_comparison_counts_other_cities_and_existing_units_but_not_itself() {
    let (mut g, ai, city, _) = fixture();
    let founder = g.spawn_test_unit("settler", 0, (12, 5));
    g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let other = *g.player_city_ids(0).iter().find(|id| **id != city).unwrap();
    let item = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.cities.get_mut(&city).unwrap().queue = vec![item.clone()];
    g.cities.get_mut(&other).unwrap().queue = vec![item];
    assert_eq!(ai.counts(&g, 0).builders, 3);
    assert_eq!(ai.counts_without_city_queue(&g, 0, city).builders, 2);
    g.cities.get_mut(&other).unwrap().queue.clear();
    assert_eq!(ai.counts_without_city_queue(&g, 0, city).builders, 1);
}

#[test]
fn adaptive_controller_keeps_its_screened_builder_policy() {
    let (mut g, _, _, builder) = fixture();
    let mut ai = AdvancedAi::new();
    let pos = g.units[&builder].pos;
    g.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("farm"));
    assert_eq!(
        ai.marginal_improvement_value(&g, 0, pos, "farm", GrandStrategy::Science),
        ai.improvement_value_for(&g, 0, pos, "farm", GrandStrategy::Science)
    );
    assert_eq!(
        ai.more_useful_builder_job(&mut g, 0, builder, GrandStrategy::Science),
        None
    );
}

#[test]
fn a_needed_builder_does_not_cancel_itself_by_satisfying_its_own_quota() {
    let (mut g, mut ai, city, builder) = fixture();
    g.remove_unit(builder);
    let item = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.apply(
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
    )
    .unwrap();
    let cost = g.item_cost_for_city(0, city, &item);
    g.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert("unit:builder".to_string(), cost - 1.0);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    let wrong = ai.production_value(&g, 0, city, &item, &plan, &ai.counts(&g, 0));
    let correct = ai.production_value(
        &g,
        0,
        city,
        &item,
        &plan,
        &ai.counts_without_city_queue(&g, 0, city),
    );
    assert!(
        correct > wrong * 5.0,
        "a single queued Builder is not surplus: {correct} vs {wrong}"
    );
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&item));
}

#[test]
fn a_city_reconsiders_a_low_value_commitment_in_the_actual_production_path() {
    let (mut g, mut ai, city, builder) = fixture();
    g.remove_unit(builder);
    for pos in [(4, 5), (5, 4), (6, 4), (4, 4)] {
        g.spawn_test_unit("warrior", 0, pos);
    }
    let old = Item::Unit {
        unit: crate::name!("warrior"),
    };
    g.apply(
        0,
        &Action::Produce {
            city,
            item: old.clone(),
        },
    )
    .unwrap();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    let counts = ai.counts_without_city_queue(&g, 0, city);
    let before = ai.production_value(&g, 0, city, &old, &plan, &counts);
    ai.advanced_production(&mut g, 0, &plan, false);
    let selected = g.cities[&city].queue.first().unwrap();
    assert_ne!(
        selected, &old,
        "an occupied queue must not bypass comparison"
    );
    let after = ai.production_value(&g, 0, city, selected, &plan, &counts);
    assert!(after > before + before.abs() * 0.25, "{after} vs {before}");
}

#[test]
fn named_city_review_does_not_enable_adaptive_preemption() {
    let (g, ai, _, _) = fixture();
    assert_eq!(ai.production_review_margin(&g), 1.25);
    assert_eq!(AdvancedAi::new().production_review_margin(&g), 1.0);
}
