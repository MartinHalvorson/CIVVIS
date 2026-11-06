use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::HostMenuEntry;
use std::sync::Arc;

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}
fn builder() -> Item {
    Item::Unit {
        unit: crate::name!("builder"),
    }
}
fn quote(g: &mut Game, cid: u32, item: &Item, turns: f64) {
    Arc::make_mut(&mut g.host_buildable)
        .entry(cid)
        .or_default()
        .insert(
            Game::production_block_key(item),
            HostMenuEntry {
                cost: Some(60.0),
                turns: Some(turns),
            },
        );
}
fn fixture() -> (Game, Vec<u32>, AdvancedAi, StrategicPlan) {
    let (mut g, cities) = producer_empire("PARALLELLIBRARY", 94_711, 1);
    g.turn = 66;
    for (i, cid) in cities.iter().enumerate() {
        g.cities.get_mut(cid).unwrap().queue = if i < 2 {
            vec![]
        } else {
            vec![building("walls")]
        };
        if i < 2 {
            g.cities
                .get_mut(cid)
                .unwrap()
                .buildings
                .retain(|b| b != "library");
        }
        let owned = g.cities[cid].owned_tiles.to_vec();
        for pos in owned {
            let t = g.map.tiles.get_mut(&pos).unwrap();
            if t.district.is_none() {
                t.terrain = crate::name!("grassland");
                t.hills = true;
                t.feature = None;
                t.resource = None;
                t.improvement = None;
            }
        }
    }
    for (cid, b, l) in [(cities[0], 3.0, 4.0), (cities[1], 5.0, 6.0)] {
        quote(&mut g, cid, &builder(), b);
        quote(&mut g, cid, &building("library"), l);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.base.w.city_target = 6.0;
    ai.enable_builders_cover_the_worked_backlog();
    ai.enable_backlog_builder_takes_the_queue();
    ai.enable_campus_building_claims_the_queue();
    ai.enable_science_heads_keep_their_queue();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    assert_eq!(
        ai.backlog_builder_claim_candidate(&g, 0, &plan, None)
            .unwrap()
            .1,
        cities[0]
    );
    assert_eq!(
        ai.campus_building_claim_candidate(&g, 0, &plan, None)
            .unwrap()
            .2,
        cities[0]
    );
    (g, cities, ai, plan)
}

#[test]
fn off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("parallel-library-backlog", |ai| {
        ai.parallel_library_backlog
    });
}

#[test]
fn the_whole_chain_swaps_jobs_and_keeps_one_of_each() {
    let (g, cities, mut ai, plan) = fixture();
    let mut off = g.clone();
    ai.run_idle_queue_claims(&mut off, 0, &plan);
    assert_eq!(off.cities[&cities[0]].queue.first(), Some(&builder()));
    assert_eq!(
        off.cities[&cities[1]].queue.first(),
        Some(&building("library"))
    );
    ai.enable_parallel_library_backlog();
    let mut on = g;
    ai.run_idle_queue_claims(&mut on, 0, &plan);
    assert_eq!(
        on.cities[&cities[0]].queue.first(),
        Some(&building("library"))
    );
    assert_eq!(on.cities[&cities[1]].queue.first(), Some(&builder()));
    assert_eq!(
        cities
            .iter()
            .filter(|c| on.cities[c].queue.first() == Some(&building("library")))
            .count(),
        1
    );
    assert_eq!(
        cities
            .iter()
            .filter(|c| on.cities[c].queue.first() == Some(&builder()))
            .count(),
        1
    );
}

#[test]
fn missing_or_invalid_native_quotes_preserve_the_old_path() {
    let (mut g, cities, mut ai, plan) = fixture();
    ai.enable_parallel_library_backlog();
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        quote(&mut g, cities[1], &builder(), value);
        assert!(ai
            .parallel_library_backlog_candidate(&g, 0, &plan)
            .is_none());
    }
    Arc::make_mut(&mut g.host_buildable).clear();
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
}

#[test]
fn ties_and_slower_total_work_do_not_reorder() {
    let (mut g, cities, mut ai, plan) = fixture();
    ai.enable_parallel_library_backlog();
    quote(&mut g, cities[1], &builder(), 6.0);
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
    quote(&mut g, cities[0], &builder(), 1.0);
    quote(&mut g, cities[1], &builder(), 5.0);
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
}

#[test]
fn guards_and_busy_heads_stay_in_force() {
    let (mut g, cities, mut ai, mut plan) = fixture();
    ai.enable_parallel_library_backlog();
    plan.threatened_city = Some(cities[0]);
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
    plan.threatened_city = None;
    g.cities.get_mut(&cities[0]).unwrap().last_attacked = g.turn;
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
    g.cities.get_mut(&cities[0]).unwrap().last_attacked = 0;
    g.cities.get_mut(&cities[0]).unwrap().queue = vec![builder()];
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
}

#[test]
fn other_lanes_and_absent_queue_protection_do_not_reorder() {
    let (g, _, mut ai, plan) = fixture();
    ai.enable_parallel_library_backlog();
    ai.victory_target = Some(VictoryTarget::Culture);
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
    ai.victory_target = Some(VictoryTarget::Science);
    ai.disable_science_heads_keep_their_queue();
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
}

#[test]
fn exclusion_matches_the_original_post_builder_library_choice() {
    let (g, cities, ai, plan) = fixture();
    let predicted = ai.campus_building_claim_candidate(&g, 0, &plan, Some(cities[0]));
    let mut actual = g;
    ai.claim_backlog_builder(&mut actual, 0, &plan);
    assert_eq!(
        predicted,
        ai.campus_building_claim_candidate(&actual, 0, &plan, None)
    );
}

#[test]
fn no_partner_or_no_builder_debt_cannot_create_a_job() {
    let (mut g, cities, mut ai, plan) = fixture();
    ai.enable_parallel_library_backlog();
    g.cities.get_mut(&cities[1]).unwrap().queue = vec![building("walls")];
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
    g.cities.get_mut(&cities[1]).unwrap().queue.clear();
    let home = g.cities[&cities[0]].pos;
    for _ in 0..10 {
        g.spawn_unit("builder", 0, home);
    }
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
}

#[test]
fn specialization_housing_and_settlers_keep_their_guards() {
    let (mut g, cities, mut ai, plan) = fixture();
    ai.enable_parallel_library_backlog();
    let mut late = g.clone();
    late.turn = late.max_turns;
    assert!(ai
        .parallel_library_backlog_candidate(&late, 0, &plan)
        .is_none());
    g.cities.get_mut(&cities[0]).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
    let city = g.cities.get_mut(&cities[0]).unwrap();
    city.queue.clear();
    city.buildings.retain(|b| b != "granary");
    city.pop = 10;
    quote(&mut g, cities[0], &building("granary"), 2.0);
    assert!(BasicAi::civ_building(&g, 0, cities[0], "granary").is_some());
    assert!(ai
        .parallel_library_backlog_candidate(&g, 0, &plan)
        .is_none());
}
