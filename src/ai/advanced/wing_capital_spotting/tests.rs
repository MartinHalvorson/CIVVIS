use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, u32, [u32; 2]) {
    let mut g = Game::new_full(3, 40, 24, 414_400, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let home = g.found_city_for(0, (3, 8), None);
    let front = g.found_city_for(0, (8, 8), None);
    let capital = g.found_city_for(1, (14, 8), None);
    let ordinary = g.found_city_for(1, (14, 16), None);
    g.cities.get_mut(&ordinary).unwrap().pop = 30;
    g.found_city_for(2, (28, 16), None);
    let base = (9, 8);
    let tile = g.map.tiles.get_mut(&base).unwrap();
    tile.owner_city = Some(front);
    tile.district = Some(crate::name!("aerodrome"));
    g.cities
        .get_mut(&front)
        .unwrap()
        .districts
        .insert(crate::name!("aerodrome"), base);
    g.current = 0;
    g.turn = 170;
    g.at_war.insert((0, 1));
    for player in [1, 2] {
        g.record_contact(0, player);
    }
    for cid in g.cities.keys().copied().collect::<Vec<_>>() {
        g.players[0].explored.insert(g.cities[&cid].pos);
    }
    let wing = [
        g.spawn_test_unit("bomber", 0, base),
        g.spawn_test_unit("bomber", 0, base),
    ];
    let spy = idle_spy(&mut g, home);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_wing_capital_spotting();
    ai.observe_air_assault_frame(BTreeSet::new(), 2);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(ordinary),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, spy, capital, ordinary, wing)
}

fn idle_spy(g: &mut Game, city: u32) -> u32 {
    let id = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        id,
        crate::game::Spy {
            id,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(city),
            ready_turn: g.turn,
            mission: None,
            sources_city: Some(city),
            sources_until: g.turn + 20,
            captured_by: None,
        },
    );
    id
}

fn candidate(g: &Game, ai: &AdvancedAi, plan: &StrategicPlan, spy: u32) -> Option<Action> {
    ai.wing_capital_spy_action(
        g,
        0,
        spy,
        plan,
        &g.legal_spy_actions(0, spy),
        None,
        &BTreeSet::new(),
    )
}

#[test]
fn real_spy_dispatch_posts_one_spare_to_the_reachable_capital() {
    let (mut g, mut ai, plan, spy, capital, ordinary, _) = fixture();
    let mut off = ai.clone();
    off.disable_wing_capital_spotting();
    let mut control = g.clone();
    off.advanced_spies(&mut control, 0, &plan);
    assert_eq!(control.spies[&spy].city, Some(ordinary));
    ai.advanced_spies(&mut g, 0, &plan);
    assert_eq!(g.spies[&spy].city, Some(capital));
    assert!(
        g.spies[&spy].ready_turn > g.turn,
        "arrival still takes native-modeled travel time"
    );
}

#[test]
fn two_idle_spies_do_not_both_take_the_capital() {
    let (mut g, mut ai, plan, first, capital, _, _) = fixture();
    let home = g.spies[&first].city.unwrap();
    idle_spy(&mut g, home);
    ai.advanced_spies(&mut g, 0, &plan);
    assert_eq!(
        g.spies
            .values()
            .filter(|spy| spy.city == Some(capital))
            .count(),
        1
    );
}

#[test]
fn a_posted_spy_runs_a_safe_operation_to_preserve_sight() {
    let (mut g, ai, plan, spy, capital, _, _) = fixture();
    let agent = g.spies.get_mut(&spy).unwrap();
    agent.city = Some(capital);
    agent.sources_city = Some(capital);
    assert!(
        matches!(candidate(&g, &ai, &plan, spy), Some(Action::SpyMission { mission, .. }) if mission == "listening_post")
    );
}

#[test]
fn posting_controls_do_not_move_a_spy() {
    let (g, ai, plan, spy, capital, _, wing) = fixture();
    let check =
        |g: &Game, ai: &AdvancedAi, p: &StrategicPlan| assert!(candidate(g, ai, p, spy).is_none());
    let mut off = ai.clone();
    off.disable_wing_capital_spotting();
    check(&g, &off, &plan);
    let mut p = plan.clone();
    p.strategy = GrandStrategy::Recovery;
    check(&g, &ai, &p);
    let mut p = plan.clone();
    p.target_player = Some(2);
    check(&g, &ai, &p);
    let mut b = g.clone();
    b.at_war.clear();
    check(&b, &ai, &plan);
    let mut b = g.clone();
    b.units.get_mut(&wing[0]).unwrap().hp = 49;
    check(&b, &ai, &plan);
    let mut b = g.clone();
    b.remove_unit(wing[1]);
    check(&b, &ai, &plan);
    let mut b = g.clone();
    b.cities.get_mut(&capital).unwrap().is_capital = false;
    check(&b, &ai, &plan);
    let mut a = ai.clone();
    a.observe_air_assault_frame([g.cities[&capital].pos].into(), 2);
    check(&g, &a, &plan);
    let mut b = g.clone();
    b.spies.get_mut(&spy).unwrap().ready_turn += 1;
    check(&b, &ai, &plan);
    let mut b = g.clone();
    b.spies.get_mut(&spy).unwrap().captured_by = Some(1);
    check(&b, &ai, &plan);
}

#[test]
fn one_jet_does_not_lend_its_range_to_an_older_bomber() {
    let (mut g, ai, plan, spy, _, _, wing) = fixture();
    for uid in wing {
        g.units.get_mut(&uid).unwrap().pos = (3, 8);
    }
    g.units.get_mut(&wing[0]).unwrap().kind = crate::name!("jet_bomber");
    assert!(candidate(&g, &ai, &plan, spy).is_none());
}

#[test]
fn an_existing_spy_reserves_the_post_during_travel() {
    let (mut g, ai, plan, spy, capital, _, _) = fixture();
    let reserved = idle_spy(&mut g, capital);
    g.spies.get_mut(&reserved).unwrap().ready_turn += 4;
    assert!(candidate(&g, &ai, &plan, spy).is_none());
}

#[test]
fn the_leading_racer_keeps_coverage_before_the_wing_gets_a_spare() {
    let (mut g, ai, plan, spy, capital, _, _) = fixture();
    let legal = g.legal_spy_actions(0, spy);
    assert!(ai
        .wing_capital_spy_action(&g, 0, spy, &plan, &legal, Some(2), &BTreeSet::new())
        .is_none());
    let racer_city = g.player_city_ids(2)[0];
    idle_spy(&mut g, racer_city);
    assert_eq!(
        ai.wing_capital_spy_action(&g, 0, spy, &plan, &legal, Some(2), &BTreeSet::new()),
        Some(Action::AssignSpy { spy, city: capital })
    );
}

#[test]
fn a_culture_racer_keeps_coverage_before_the_wing_gets_a_spare() {
    let (mut g, ai, plan, spy, capital, _, _) = fixture();
    let legal = g.legal_spy_actions(0, spy);
    let targets = [2].into();
    assert!(ai
        .wing_capital_spy_action(&g, 0, spy, &plan, &legal, None, &targets)
        .is_none());
    let racer_city = g.player_city_ids(2)[0];
    idle_spy(&mut g, racer_city);
    assert_eq!(
        ai.wing_capital_spy_action(&g, 0, spy, &plan, &legal, None, &targets),
        Some(Action::AssignSpy { spy, city: capital })
    );
}

#[test]
fn an_offered_disruption_is_not_traded_for_sight() {
    let (g, ai, plan, spy, capital, _, _) = fixture();
    let legal = [
        Action::AssignSpy { spy, city: capital },
        Action::SpyMission {
            spy,
            mission: "disrupt_rocketry".to_string(),
            target: (3, 8),
        },
    ];
    assert!(ai
        .wing_capital_spy_action(&g, 0, spy, &plan, &legal, None, &BTreeSet::new())
        .is_none());
}
