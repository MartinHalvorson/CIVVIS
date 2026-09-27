use super::super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 20, 379_700, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let rear = g.found_city_for(0, (2, 8), None);
    let front = g.found_city_for(0, (14, 8), None);
    let objective = g.found_city_for(1, (18, 8), None);
    g.cities.get_mut(&front).unwrap().loyalty = 5.0;
    g.cities.get_mut(&objective).unwrap().pop = 40;
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 154;
    let bomber = g.spawn_test_unit("bomber", 0, (14, 8));
    let victim = g.spawn_test_unit("artillery", 1, (15, 8));
    g.units.get_mut(&victim).unwrap().hp = 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.air_surge = true;
    assert!(g.city_loyalty_per_turn(&g.cities[&front]) < -5.0);
    assert!(g.city_loyalty_per_turn(&g.cities[&rear]) >= 0.0);
    let rebase = Action::AirRebase {
        unit: bomber,
        to: (2, 8),
    };
    assert!(g.legal_actions(0).contains(&rebase));
    assert!(ai
        .immediate_kill_value(
            &g,
            0,
            &Action::AirStrike {
                unit: bomber,
                target: (15, 8),
            },
            &plan,
        )
        .is_some());
    (g, ai, plan, bomber, front, victim)
}

fn next_owned_turn(g: &mut Game) {
    loop {
        let pid = g.current;
        g.apply(pid, &Action::EndTurn).unwrap();
        if g.current == 0 {
            break;
        }
    }
}

#[test]
fn actual_dispatch_evacuates_before_a_profitable_kill_and_survives_revolt() {
    let (mut g, mut ai, plan, bomber, front, victim) = fixture();
    let mut stayed = g.clone();
    next_owned_turn(&mut stayed);
    assert_ne!(stayed.cities[&front].owner, 0);
    assert!(!stayed.units.contains_key(&bomber));

    ai.advanced_units(&mut g, 0, &plan);
    next_owned_turn(&mut g);
    assert_ne!(g.cities[&front].owner, 0, "real Loyalty tick must revolt");
    assert!(
        g.units.contains_key(&bomber),
        "evacuated Bomber must survive"
    );
    assert_eq!(g.units[&bomber].pos, (2, 8));
    assert!(
        g.units.contains_key(&victim),
        "safety must precede the kill pass"
    );
}
