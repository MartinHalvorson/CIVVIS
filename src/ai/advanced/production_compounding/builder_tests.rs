use super::*;
use crate::game::Action;
use crate::setup::GameSpeed;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, Item) {
    let mut g = Game::new_full(2, 32, 22, 610_086, 150, 0, true);
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
    g.turn = 20;
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 20.0;
    g.players[0].techs.insert(crate::name!("mining"));
    let cid = g.found_city_for(0, (8, 10), None);
    let other = g.found_city_for(0, (20, 10), None);
    assert!(g.map.get((23, 10)).is_some());
    g.found_city_for(1, (23, 10), None);
    g.cities.get_mut(&cid).unwrap().pop = 2;
    for pos in [(8, 9), (9, 10)] {
        assert_eq!(g.map.tiles[&pos].owner_city, Some(cid));
        g.map.tiles.get_mut(&pos).unwrap().hills = true;
    }
    g.cities.get_mut(&other).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_builder_payback_reserve();
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
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    (g, ai, plan, cid, other, builder)
}

#[test]
fn repayable_worked_mines_start_a_builder_and_keep_its_first_frame() {
    let (mut g, mut ai, plan, cid, _, builder) = fixture();
    assert!(ai
        .production_builder_investment(&g, 0, cid, &plan)
        .is_some());
    let mut control = g.clone();
    let mut ordinary = ai.clone();
    ordinary.builder_payback_reserve = false;
    ordinary.advanced_production(&mut control, 0, &plan, false);
    assert_ne!(control.cities[&cid].queue.first(), Some(&builder));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    assert_eq!(g.item_invested_production(cid, &builder), 0.0);
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
}

#[test]
fn unlocked_work_existing_charges_and_other_queues_bound_the_reservation() {
    let (g, ai, plan, cid, other, builder) = fixture();
    let mut locked = g.clone();
    locked.players[0].techs.remove(&crate::name!("mining"));
    assert!(ai
        .production_builder_investment(&locked, 0, cid, &plan)
        .is_none());
    let mut covered = g.clone();
    covered.spawn_test_unit("builder", 0, covered.cities[&cid].pos);
    assert!(ai
        .production_builder_investment(&covered, 0, cid, &plan)
        .is_none());
    let mut queued = g.clone();
    queued.cities.get_mut(&other).unwrap().queue = vec![builder];
    assert!(ai
        .production_builder_investment(&queued, 0, cid, &plan)
        .is_none());
    let mut refused = g.clone();
    std::sync::Arc::make_mut(&mut refused.blocked_improvement_sites).insert((8, 9));
    assert!(ai
        .production_builder_investment(&refused, 0, cid, &plan)
        .is_none());
}

#[test]
fn forest_removal_and_expiring_clock_do_not_invent_payback() {
    let (g, ai, plan, cid, _, _) = fixture();
    let mut forest = g.clone();
    for pos in [(8, 9), (9, 10)] {
        forest.map.tiles.get_mut(&pos).unwrap().feature = Some(crate::name!("forest"));
    }
    // A controlled improvement removes a standing +1 forest before adding
    // its printed +1 production. Both sites are legal, but their net gain is zero.
    let mine = std::sync::Arc::make_mut(&mut forest.rules)
        .improvements
        .get_mut("mine")
        .unwrap();
    mine.feature = vec![crate::name!("forest")];
    mine.removes_feature = true;
    assert!(forest
        .valid_improvements(0, (8, 9))
        .contains(&crate::name!("mine")));
    assert_eq!(
        forest
            .improvement_yield_change(0, (8, 9), crate::name!("mine"))
            .production,
        0.0
    );
    assert!(ai
        .production_builder_investment(&forest, 0, cid, &plan)
        .is_none());
    let mut late = g.clone();
    late.max_turns = late.turn + 1;
    assert!(ai
        .production_builder_investment(&late, 0, cid, &plan)
        .is_none());
}

#[test]
fn siege_and_insolvency_retain_their_priority() {
    let (mut g, mut ai, mut plan, cid, _, builder) = fixture();
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -10.0;
    assert!(ai
        .production_builder_investment(&g, 0, cid, &plan)
        .is_none());
    g.players[0].gold_per_turn = 20.0;
    plan.threatened_city = Some(cid);
    assert!(ai
        .production_builder_investment(&g, 0, cid, &plan)
        .is_none());
    plan.threatened_city = None;
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: builder.clone(),
        },
    )
    .unwrap();
    let home = g.cities[&cid].pos;
    let positions: Vec<_> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(*pos, home) == 2 && g.city_at(*pos).is_none())
        .collect();
    let barb = g.barb_pid.unwrap();
    g.spawn_test_unit("warrior", barb, positions[0]);
    g.spawn_test_unit("warrior", barb, positions[1]);
    ai.base.siege_preempts_the_queue = true;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert!(
        matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit })
        if g.rules.units[unit].class == "military")
    );
}

#[test]
fn the_delegated_turn_driver_reserves_repayable_worked_production() {
    use crate::ai::Ai;
    let (mut g, mut ai, plan, cid, other, builder) = fixture();
    g.spawn_test_unit("warrior", 0, g.cities[&cid].pos);
    g.spawn_test_unit("warrior", 0, g.cities[&other].pos);
    ai.skip_opening_book();
    g.spawn_test_unit("scout", 0, (8, 11));
    g.spawn_test_unit("scout", 0, (9, 11));
    ai.enable_lane_delegates_production_2();
    ai.base.solvency_first_trade_slot = false;
    ai.plan = Some(plan.clone());
    assert!(ai.lane_delegates_now(Some(VictoryTarget::Domination), false));
    assert!(ai
        .production_builder_investment(&g, 0, cid, &plan)
        .is_some());
    let mut control = g.clone();
    let mut ordinary = ai.clone();
    ordinary.builder_payback_reserve = false;
    ordinary.take_turn(&mut control, 0);
    assert_ne!(control.cities[&cid].queue.first(), Some(&builder));
    ai.take_turn(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
}
