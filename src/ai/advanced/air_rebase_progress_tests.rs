use super::*;

fn fixture(
    kind: &str,
    current_distance: i32,
    destination_distance: i32,
) -> (Game, StrategicPlan, u32, Pos) {
    let mut g = Game::new_full(2, 48, 24, 400_000, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let target = (24, 10);
    let base = (24 - current_distance, 10);
    let home = g.found_city_for(0, base, None);
    let objective = g.found_city_for(1, target, None);
    let destination = g
        .wdisk(base, 2)
        .into_iter()
        .find(|pos| *pos != base && g.wdist(*pos, target) == destination_distance)
        .expect("a nearby base at the requested objective distance");
    let tile = g.map.tiles.get_mut(&destination).unwrap();
    tile.owner_city = Some(home);
    tile.improvement = Some(crate::name!("airstrip"));
    tile.pillaged = false;
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 154;
    let remembered = g.remember_city(&g.cities[&objective]);
    g.players[0].remembered_cities.insert(objective, remembered);
    g.players[0].explored.insert(target);
    let aircraft = g.spawn_test_unit(kind, 0, base);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert_eq!(g.wdist(base, target), current_distance);
    assert_eq!(g.wdist(destination, target), destination_distance);
    assert!(g
        .legal_doctrine_actions(0, aircraft)
        .contains(&Action::AirRebase {
            unit: aircraft,
            to: destination,
        }));
    (g, plan, aircraft, destination)
}

fn assert_no_profitable_mission(g: &Game, plan: &StrategicPlan, aircraft: u32) {
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    for action in g.legal_doctrine_actions(0, aircraft) {
        let value = match action {
            Action::AirStrike { target, .. } => ai.air_strike_value(g, 0, aircraft, target, plan),
            Action::AirPillage { target, .. } => ai.air_pillage_value(g, 0, aircraft, target),
            Action::PriorityTarget { target, .. } => {
                ai.priority_target_value(g, 0, aircraft, target)
            }
            _ => continue,
        };
        assert!(
            value <= 0.0,
            "fixture must exercise the rebase fallback: {action:?}"
        );
    }
}

#[test]
fn an_in_range_bomber_does_not_rebase_farther_without_a_mission() {
    let (g, plan, bomber, _) = fixture("bomber", 8, 9);
    assert_no_profitable_mission(&g, &plan, bomber);
    assert!(g.unit_attack_range(bomber) >= 9);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).advanced_air_action(&g, 0, bomber, &plan),
        None,
    );
}

#[test]
fn an_in_range_bomber_does_not_trade_equally_distant_bases() {
    let (g, plan, bomber, _) = fixture("bomber", 8, 8);
    assert_no_profitable_mission(&g, &plan, bomber);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).advanced_air_action(&g, 0, bomber, &plan),
        None,
    );
}

#[test]
fn a_bomber_still_rebases_closer_from_an_in_range_base() {
    let (mut g, plan, bomber, destination) = fixture("bomber", 8, 7);
    assert_no_profitable_mission(&g, &plan, bomber);
    let action = AdvancedAi::targeting(VictoryTarget::Domination)
        .advanced_air_action(&g, 0, bomber, &plan)
        .expect("forward staging remains useful");
    assert_eq!(
        action,
        Action::AirRebase {
            unit: bomber,
            to: destination
        }
    );
    g.apply(0, &action).unwrap();
    assert_eq!(g.units[&bomber].pos, destination);
    assert_eq!(g.units[&bomber].moves_left, 0.0);
    assert_eq!(g.units[&bomber].attacks_left, 0);
}

#[test]
fn a_bomber_still_rebases_across_the_attack_range_boundary() {
    let (g, plan, bomber, destination) = fixture("bomber", 11, 10);
    assert_no_profitable_mission(&g, &plan, bomber);
    assert_eq!(g.unit_attack_range(bomber), 10);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirRebase {
            unit: bomber,
            to: destination
        }),
    );
}

#[test]
fn a_bomber_still_stages_closer_when_both_bases_are_out_of_range() {
    let (g, plan, bomber, destination) = fixture("bomber", 13, 12);
    assert_no_profitable_mission(&g, &plan, bomber);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirRebase {
            unit: bomber,
            to: destination
        }),
    );
}

#[test]
fn a_profitable_immediate_kill_keeps_the_sortie() {
    let (mut g, plan, bomber, _) = fixture("bomber", 8, 7);
    let target = g.nbrs(g.units[&bomber].pos)[0];
    let victim = g.spawn_test_unit("artillery", 1, target);
    g.units.get_mut(&victim).unwrap().hp = 1;
    let action = AdvancedAi::targeting(VictoryTarget::Domination)
        .advanced_air_action(&g, 0, bomber, &plan)
        .expect("a visible immediate kill is profitable");
    assert_eq!(
        action,
        Action::AirStrike {
            unit: bomber,
            target
        }
    );
    g.apply(0, &action).unwrap();
    assert!(!g.units.contains_key(&victim));
}

#[test]
fn an_illegal_full_base_is_not_used_for_forward_staging() {
    let (mut g, plan, bomber, destination) = fixture("bomber", 8, 7);
    for _ in 0..3 {
        g.spawn_test_unit("fighter", 0, destination);
    }
    assert!(!g
        .legal_doctrine_actions(0, bomber)
        .contains(&Action::AirRebase {
            unit: bomber,
            to: destination,
        }));
    assert_no_profitable_mission(&g, &plan, bomber);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).advanced_air_action(&g, 0, bomber, &plan),
        None,
    );
}

#[test]
fn a_fighter_keeps_patrol_instead_of_trading_equally_distant_bases() {
    let (mut g, plan, fighter, _) = fixture("fighter", 4, 4);
    // The city is a known objective at peace; there is no competing strike.
    g.at_war.clear();
    assert_no_profitable_mission(&g, &plan, fighter);
    assert!(g.unit_attack_range(fighter) >= 4);
    let action = AdvancedAi::targeting(VictoryTarget::Domination)
        .advanced_air_action(&g, 0, fighter, &plan)
        .expect("the fighter has legal patrol coverage");
    assert!(matches!(action, Action::AirPatrol { unit, .. } if unit == fighter));
    g.apply(0, &action).unwrap();
    assert!(g.units[&fighter].air_patrol);
}

#[test]
fn the_frozen_bomber_anchor_keeps_its_original_rebase_score() {
    for destination_distance in [8, 9] {
        let (g, plan, bomber, destination) = fixture("bomber", 8, destination_distance);
        assert_no_profitable_mission(&g, &plan, bomber);
        assert_eq!(
            AdvancedAi::legacy().advanced_air_action(&g, 0, bomber, &plan),
            Some(Action::AirRebase {
                unit: bomber,
                to: destination
            }),
        );
    }
}

#[test]
fn the_frozen_fighter_anchor_keeps_its_original_rebase_score() {
    let (mut g, plan, fighter, destination) = fixture("fighter", 4, 4);
    g.at_war.clear();
    assert_eq!(
        AdvancedAi::legacy().advanced_air_action(&g, 0, fighter, &plan),
        Some(Action::AirRebase {
            unit: fighter,
            to: destination
        }),
    );
}

#[test]
fn a_remembered_objective_does_not_restart_a_two_base_cycle() {
    let (mut modern, plan, bomber, destination) = fixture("bomber", 8, 9);
    let original_base = modern.units[&bomber].pos;
    let objective = plan.target_city.unwrap();
    let mut legacy = modern.clone();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let anchor = AdvancedAi::legacy();
    for turn in 0..3 {
        let view = modern.player_decision_view(0);
        assert!(
            view.cities.contains_key(&objective),
            "the objective remains remembered"
        );
        assert!(!view.player_can_see(0, view.cities[&objective].pos));
        assert_no_profitable_mission(&view, &plan, bomber);
        assert_eq!(ai.advanced_air_action(&view, 0, bomber, &plan), None);
        assert_eq!(modern.units[&bomber].pos, original_base);

        let old_view = legacy.player_decision_view(0);
        let old_action = anchor
            .advanced_air_action(&old_view, 0, bomber, &plan)
            .unwrap();
        let to = if turn % 2 == 0 {
            destination
        } else {
            original_base
        };
        assert_eq!(old_action, Action::AirRebase { unit: bomber, to });
        legacy.apply(0, &old_action).unwrap();
        assert_eq!(legacy.units[&bomber].moves_left, 0.0);

        for game in [&mut modern, &mut legacy] {
            loop {
                game.apply(game.current, &Action::EndTurn).unwrap();
                if game.current == 0 {
                    break;
                }
            }
        }
    }
}
