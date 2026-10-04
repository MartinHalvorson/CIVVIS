use super::*;
use crate::setup::GameSpeed;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Item) {
    let mut g = Game::new_full(2, 32, 22, 610_039, 150, 0, true);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }
    g.game_speed = GameSpeed::Online;
    g.current = 0;
    g.turn = 22;
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 20.0;
    let cid = g.found_city_for(0, (8, 10), None);
    let other = g.found_city_for(0, (20, 10), None);
    assert!(g.map.get((23, 10)).is_some());
    g.found_city_for(1, (23, 10), None);
    g.cities.get_mut(&cid).unwrap().pop = 4;
    g.cities.get_mut(&other).unwrap().pop = 4;
    g.cities.get_mut(&other).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    ai.enable_builder_workforce_recovery_2();
    // Make the ordinary review prefer another build, exposing the handoff
    // failure rather than relying on a particular menu's accidental ranking.
    ai.base.w.p_builder = 0.01;
    ai.preempt_margin = 1.25;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let item = Item::Unit {
        unit: crate::name!("builder"),
    };
    (g, ai, plan, cid, item)
}

#[test]
fn the_reserved_builder_survives_review_and_another_uninvested_frame() {
    let (mut g, mut ai, plan, cid, builder) = fixture();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    assert_eq!(g.item_invested_production(cid, &builder), 0.0);

    let mut control = g.clone();
    let mut ordinary = ai.clone();
    ordinary.builder_workforce_reservation.replace(None);
    ordinary.advanced_production(&mut control, 0, &plan, false);
    assert_ne!(control.cities[&cid].queue.first(), Some(&builder));

    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    // Observed execution can plan again before EndTurn invests production.
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
}

#[test]
fn a_previous_turns_uninvested_receipt_does_not_lock_a_builder() {
    let (mut g, mut ai, plan, cid, builder) = fixture();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    g.turn += 1;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&builder));
}

#[test]
fn a_refused_continuation_is_not_protected_by_its_receipt() {
    let (mut g, mut ai, plan, cid, builder) = fixture();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    std::sync::Arc::make_mut(&mut g.blocked_production)
        .entry(cid)
        .or_default()
        .insert(Game::production_block_key(&builder));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&builder));
}

#[test]
fn local_siege_defense_can_replace_a_fresh_builder_reservation() {
    let (mut g, mut ai, plan, cid, builder) = fixture();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    let home = g.cities[&cid].pos;
    let barb = g.barb_pid.unwrap();
    let ring = |distance| {
        g.map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.wdist(*pos, home) == distance && g.city_at(*pos).is_none())
            .collect::<Vec<_>>()
    };
    let adjacent = ring(1);
    let second = ring(2);
    g.spawn_test_unit("slinger", barb, adjacent[0]);
    g.spawn_test_unit("warrior", barb, second[0]);
    g.spawn_test_unit("warrior", barb, second[1]);
    g.players[0].gold = 0.0;
    ai.base.siege_preempts_the_queue = true;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&builder));
    assert!(
        matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit })
        if g.rules.units[unit].class == "military")
    );
}
