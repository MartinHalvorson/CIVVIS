use super::super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, Item) {
    let mut g = Game::new_full(2, 40, 26, 374_300, 500, 0, false);
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let slow = g.found_city_for(0, (8, 12), None);
    let fast = g.found_city_for(0, (12, 12), None);
    let target = g.found_city_for(1, (20, 12), None);
    g.cities.get_mut(&target).unwrap().wall_hp = 100;
    g.at_war.insert((0, 1));
    g.players[0]
        .techs
        .extend(["archery", "masonry", "engineering"].map(Name::new));
    g.players[0].gold_per_turn = 20.0;
    g.turn = 102;
    g.current = 0;
    for _ in 0..20 {
        g.spawn_test_unit("archer", 0, (12, 12));
    }
    let item = Item::Unit {
        unit: crate::name!("catapult"),
    };
    g.cities.get_mut(&slow).unwrap().queue.push(item.clone());
    for (cid, turns) in [(slow, 12.0), (fast, 3.0)] {
        std::sync::Arc::make_mut(&mut g.host_buildable).insert(
            cid,
            [(
                Game::production_block_key(&item),
                crate::game::HostMenuEntry {
                    cost: None,
                    turns: Some(turns),
                },
            )]
            .into(),
        );
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        plan,
        slow,
        fast,
        item,
    )
}

#[test]
fn a_slow_queue_does_not_fill_an_undelivered_siege_requirement() {
    let (g, ai, plan, _, fast, item) = fixture();
    assert!(ai.faster_first_siege(&g, 0, fast, &plan, &item));
    assert!(ai.production_value(&g, 0, fast, &item, &plan, &ai.counts(&g, 0)) > 0.0);
}

#[test]
fn the_fast_commitment_survives_rescoring_and_blocks_a_slower_copy() {
    let (mut g, ai, plan, slow, fast, item) = fixture();
    g.cities.get_mut(&fast).unwrap().queue.push(item.clone());
    assert!(ai.faster_first_siege(&g, 0, fast, &plan, &item));
    assert!(!ai.faster_first_siege(&g, 0, slow, &plan, &item));
}

#[test]
fn fielded_equipment_closes_the_acceleration_reservation() {
    let (mut g, ai, plan, _, fast, item) = fixture();
    g.spawn_test_unit("catapult", 0, (12, 12));
    assert!(!ai.faster_first_siege(&g, 0, fast, &plan, &item));
}

#[test]
fn marginal_savings_do_not_duplicate_siege_equipment() {
    for slow_turns in [4.0, 5.0] {
        let (mut g, ai, plan, slow, fast, item) = fixture();
        std::sync::Arc::make_mut(&mut g.host_buildable)
            .get_mut(&slow)
            .unwrap()
            .get_mut(&Game::production_block_key(&item))
            .unwrap()
            .turns = Some(slow_turns);
        assert!(!ai.faster_first_siege(&g, 0, fast, &plan, &item));
    }
}

#[test]
fn acceleration_requires_a_current_hostile_walled_target() {
    for case in 0..3 {
        let (mut g, ai, mut plan, _, fast, item) = fixture();
        match case {
            0 => g.at_war.clear(),
            1 => {
                g.cities
                    .get_mut(&plan.target_city.unwrap())
                    .unwrap()
                    .wall_hp = 0
            }
            _ => plan.target_city = None,
        }
        assert!(!ai.faster_first_siege(&g, 0, fast, &plan, &item));
    }
}

#[test]
fn faster_building_does_not_pull_the_first_weapon_farther_from_the_front() {
    let (mut g, ai, plan, _, fast, item) = fixture();
    g.cities.get_mut(&fast).unwrap().pos = (2, 12);
    assert!(!ai.faster_first_siege(&g, 0, fast, &plan, &item));
}

#[test]
fn the_production_governor_orders_an_earlier_first_weapon() {
    let (mut g, mut ai, plan, _, fast, item) = fixture();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&fast].queue.first(), Some(&item));
}
