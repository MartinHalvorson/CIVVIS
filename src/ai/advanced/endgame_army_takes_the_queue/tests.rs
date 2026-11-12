use super::*;

/// Two majors on plains; we hold three cities, the rival one. Turn 150 of
/// a 250-turn game (60% of the clock), income 20 a turn, an empty treasury,
/// and a Domination conquest of the rival's city.
fn fixture() -> (Game, StrategicPlan, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 366_401, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(0, (4, 10), None);
    g.found_city_for(0, (10, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = 150;
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = 20.0;
    let ours = g.player_city_ids(0);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan, ours)
}

fn on() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_endgame_army_takes_the_queue();
    ai
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn military_head(g: &Game, cid: u32) -> bool {
    matches!(
        g.cities[&cid].queue.first(),
        Some(Item::Unit { unit }) if g.rules.units[unit].class == "military"
    )
}

#[test]
fn off_or_for_another_target_nothing_changes() {
    let (mut g, plan, ours) = fixture();
    let off = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(off.endgame_army_clock(&g, 0), None);
    off.claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(ours.iter().all(|cid| g.cities[cid].queue.is_empty()));
    assert_eq!(off.endgame_purchase_reserve(&g, 0, 42.0), 42.0);

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_endgame_army_takes_the_queue();
    assert_eq!(science.endgame_army_clock(&g, 0), None);
    science.claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(ours.iter().all(|cid| g.cities[cid].queue.is_empty()));
}

#[test]
fn the_window_opens_at_sixty_percent_of_the_clock_without_a_rival_capital() {
    let (mut g, _, _) = fixture();
    g.turn = 149;
    assert_eq!(on().endgame_army_clock(&g, 0), None, "before 60% of 250");
    g.turn = 150;
    let clock = on().endgame_army_clock(&g, 0).expect("open at t150");
    assert!(clock.late_clock);
    assert_eq!(clock.turns_left, 100.0);
    // Holding a rival's original capital keeps the game clock shut.
    let capital = g.player_city_ids(1)[0];
    let city = g.cities.get_mut(&capital).unwrap();
    city.is_capital = true;
    city.original_owner = 1;
    city.owner = 0;
    assert!(AdvancedAi::holds_a_rival_capital(&g, 0));
    assert_eq!(on().endgame_army_clock(&g, 0), None);
}

#[test]
fn a_rival_space_launch_opens_the_window_early_and_sets_the_clock() {
    let (mut g, _, _) = fixture();
    g.turn = 100;
    assert_eq!(on().endgame_army_clock(&g, 0), None);
    g.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let clock = on()
        .endgame_army_clock(&g, 0)
        .expect("a satellite reads 50 turns");
    assert!(!clock.late_clock);
    assert_eq!(clock.rival, Some((1, SCIENCE_TURNS_AFTER_SATELLITE)));
    assert_eq!(clock.turns_left, SCIENCE_TURNS_AFTER_SATELLITE);
    g.players[1]
        .science_projects
        .insert("launch_moon_landing".to_string());
    assert_eq!(
        on().endgame_army_clock(&g, 0).map(|clock| clock.turns_left),
        Some(SCIENCE_TURNS_AFTER_MOON),
        "the latest launch sets the clock"
    );
}

#[test]
fn idle_cities_train_the_army_while_the_income_carries_it() {
    let (mut g, plan, ours) = fixture();
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(
        ours.iter().all(|cid| military_head(&g, *cid)),
        "every idle city trains a land unit: {:?}",
        ours.iter()
            .map(|cid| g.cities[cid].queue.first().cloned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn an_insolvent_treasury_claims_nothing_and_a_bank_pays_the_runway() {
    let (mut g, plan, ours) = fixture();
    g.players[0].gold_per_turn = -5.0;
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(
        ours.iter().all(|cid| g.cities[cid].queue.is_empty()),
        "income below zero and nothing banked: no unit starts"
    );
    // A treasury spread over the 100 turns left lifts the room above zero.
    g.players[0].gold = 1_000.0;
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(ours.iter().any(|cid| military_head(&g, *cid)));
}

#[test]
fn the_unit_bill_counts_against_the_room() {
    let (mut g, plan, ours) = fixture();
    g.players[0].techs.insert(crate::name!("bronze_working"));
    let spear = Item::Unit {
        unit: crate::name!("spearman"),
    };
    let upkeep = AdvancedAi::item_added_upkeep(&g, &spear);
    assert!(upkeep > 0.0, "the fixture needs a unit with upkeep");
    // Income carries exactly one Spearman.
    g.players[0].gold_per_turn = upkeep;
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    let started = ours.iter().filter(|cid| military_head(&g, **cid)).count();
    assert_eq!(started, 1, "one unit's upkeep fits the income");
}

#[test]
fn a_threatened_city_keeps_its_queue_for_its_defence() {
    let (mut g, mut plan, ours) = fixture();
    plan.threatened_city = Some(ours[0]);
    g.cities.get_mut(&ours[1]).unwrap().last_attacked = g.turn - 2;
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(
        g.cities[&ours[0]].queue.is_empty(),
        "the plan's threatened city"
    );
    assert!(
        g.cities[&ours[1]].queue.is_empty(),
        "a city hit two turns ago"
    );
    assert!(military_head(&g, ours[2]));
}

#[test]
fn a_fresh_long_payback_build_gives_its_queue_and_a_paid_one_finishes() {
    let (mut g, plan, ours) = fixture();
    let monument = building("monument");
    let walls = building("walls");
    g.cities.get_mut(&ours[0]).unwrap().queue = vec![monument.clone()];
    g.cities.get_mut(&ours[1]).unwrap().queue = vec![walls.clone()];
    let paid = {
        let cost = g.item_cost_for_city(0, ours[2], &monument);
        let city = g.cities.get_mut(&ours[2]).unwrap();
        city.queue = vec![monument.clone()];
        city.production = cost * 0.5;
        cost * 0.5
    };
    assert!(paid > 0.0);
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(
        military_head(&g, ours[0]),
        "a fresh Monument gives its queue"
    );
    assert_eq!(g.cities[&ours[1]].queue.first(), Some(&walls), "Walls stay");
    assert_eq!(
        g.cities[&ours[2]].queue.first(),
        Some(&monument),
        "a half-paid build finishes"
    );
}

#[test]
fn the_army_at_its_target_leaves_the_queues_alone() {
    let (mut g, plan, ours) = fixture();
    let target = on().endgame_army_target(&g, 0, ours.len());
    assert_eq!(target, ENDGAME_ARMY_PER_CITY * ours.len());
    for _ in 0..target {
        g.spawn_test_unit("warrior", 0, (5, 5));
    }
    on().claim_endgame_army_queues(&mut g, 0, &plan);
    assert!(ours.iter().all(|cid| g.cities[cid].queue.is_empty()));
}

#[test]
fn long_payback_reads_the_economy_not_the_army() {
    let (g, _, _) = fixture();
    let ai = on();
    let long = |item: &Item| ai.endgame_long_payback(&g, item);
    assert!(long(&building("library")));
    assert!(long(&building("monument")));
    assert!(!long(&building("walls")), "Walls defend");
    assert!(!long(&building("barracks")), "an Encampment building");
    assert!(!long(&building("hangar")), "an Aerodrome building");
    let market = &g.rules.buildings["market"];
    assert_eq!(
        long(&building("market")),
        market.yields.gold - market.maintenance <= 0.0,
        "an income building pays the army's upkeep"
    );
    let district = |name: &str| Item::District {
        district: Name::new(name),
        pos: (5, 5),
    };
    assert!(long(&district("campus")));
    assert!(!long(&district("encampment")));
    assert!(!long(&district("aerodrome")));
    let project = |name: &str| Item::Project {
        project: Name::new(name),
    };
    assert!(long(&project("theater_square_festival")));
    assert!(long(&project("convert_reactor_to_uranium")));
    assert!(!long(&project("commercial_hub_investment")));
    assert!(!long(&project("manhattan_project")));
    assert!(!long(&project("launch_earth_satellite")));
    assert!(!long(&Item::Unit {
        unit: crate::name!("builder"),
    }));
}

#[test]
fn the_endgame_treasury_keeps_three_turns_of_the_unit_bill() {
    let (mut g, _, _) = fixture();
    g.players[0].techs.insert(crate::name!("bronze_working"));
    for _ in 0..4 {
        g.spawn_test_unit("spearman", 0, (5, 5));
    }
    let bill = g.unit_gold_maintenance(0);
    assert!(bill > 0.0, "the fixture needs a unit bill");
    assert_eq!(
        on().endgame_purchase_reserve(&g, 0, 0.0),
        ENDGAME_UPKEEP_RESERVE_TURNS * bill
    );
    assert_eq!(
        on().endgame_purchase_reserve(&g, 0, 10_000.0),
        10_000.0,
        "a higher stock reserve stands"
    );
    g.turn = 100;
    assert_eq!(
        on().endgame_purchase_reserve(&g, 0, 0.0),
        0.0,
        "outside the window"
    );
}
