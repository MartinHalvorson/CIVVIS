use super::*;
use crate::ai::industrial_zones::tests::producer_empire;

fn fixture(gold: f64, income: f64) -> (Game, Vec<u32>, AdvancedAi, StrategicPlan) {
    let (mut game, ranked) = producer_empire("ZONECLAIM", 93_101, 1);
    game.turn = 79;
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = income;
    game.players[0]
        .civics
        .insert(crate::name!("games_recreation"));
    for (rank, cid) in ranked.iter().enumerate() {
        let surplus = if rank == 0 {
            -2
        } else if rank < 5 {
            -1
        } else {
            0
        };
        let modeled = game.city_amenity_surplus(&game.cities[cid]);
        std::sync::Arc::make_mut(&mut game.observed_city_amenity_adjustments)
            .insert(*cid, surplus - modeled);
    }
    assert!(AdvancedAi::widespread_amenity_crisis(&game, 0));
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: ranked.len(),
        assessed_turn: game.turn,
        rush: false,
    };
    (game, ranked, ai, plan)
}

fn recovery_for(ai: &AdvancedAi, game: &Game, cid: u32) -> Option<Item> {
    ai.base
        .economic_recovery_item(game, 0, cid, ai.counts(game, 0).traders)
}

#[test]
fn depleted_amenity_reservations_take_the_available_recovery_build() {
    let mut unexpected = Vec::new();
    for (gold, income) in [(81.0, -7.84375), (21.0, -10.8438), (0.0, -11.1914)] {
        let (mut game, ranked, ai, plan) = fixture(gold, income);
        let capital = ranked[0];
        let recovery = recovery_for(&ai, &game, capital)
            .expect("the depleted city has a legal ordinary recovery build");
        assert!(
            matches!(&recovery, Item::District { district, .. } if district == "commercial_hub")
        );
        assert!(game.can_produce(0, capital, &recovery));
        assert!(game.producible_items(0, capital).iter().any(|item| {
            matches!(item, Item::District { district, .. }
                if game.district_family(*district) == "entertainment_complex")
        }));
        ai.reserve_idle_entertainment_path_for_widespread_crisis(&mut game, 0, &plan);
        if game.cities[&capital].queue.first() != Some(&recovery) {
            unexpected.push((gold, income, game.cities[&capital].queue.clone()));
        }
        assert_eq!(
            ranked
                .iter()
                .filter(|cid| !game.cities[cid].queue.is_empty())
                .count(),
            1,
            "only the already-selected idle queue is reserved"
        );
    }
    assert!(
        unexpected.is_empty(),
        "available recovery takes the depleted queue: {unexpected:?}"
    );
}

#[test]
fn funded_and_break_even_amenity_reservations_keep_their_existing_choice() {
    for (gold, income) in [(250.0, -7.84375), (0.0, -0.5), (0.0, 0.0)] {
        let (mut game, ranked, ai, plan) = fixture(gold, income);
        assert!(recovery_for(&ai, &game, ranked[0]).is_some());
        ai.reserve_idle_entertainment_path_for_widespread_crisis(&mut game, 0, &plan);
        assert!(matches!(game.cities[&ranked[0]].queue.first(),
            Some(Item::District { district, .. }) if game.district_family(*district) == "entertainment_complex"));
    }
}

#[test]
fn an_amenity_repair_still_runs_without_an_available_recovery_item() {
    let (mut game, ranked, ai, plan) = fixture(81.0, -7.84375);
    let capital = ranked[0];
    crate::game::install_test_district(&mut game, capital, "entertainment_complex");
    assert!(recovery_for(&ai, &game, capital).is_none());
    let arena = Item::Building {
        building: crate::name!("arena"),
    };
    assert!(game.can_produce(0, capital, &arena));
    ai.reserve_idle_entertainment_path_for_widespread_crisis(&mut game, 0, &plan);
    assert_eq!(game.cities[&capital].queue.first(), Some(&arena));
}

#[test]
fn the_disabled_amenity_reservation_keeps_the_depleted_queues_untouched() {
    let (mut game, ranked, _, plan) = fixture(81.0, -7.84375);
    let ai = AdvancedAi::new();
    ai.reserve_idle_entertainment_path_for_widespread_crisis(&mut game, 0, &plan);
    assert!(ranked.iter().all(|cid| game.cities[cid].queue.is_empty()));
}
