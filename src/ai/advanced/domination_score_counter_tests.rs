use super::*;

fn board() -> Game {
    let mut g = Game::new_full(3, 40, 24, 37_200, 250, 0, false);
    super::tests::found_capitals(&mut g);
    g.turn = 240;
    for rival in 1..3 {
        g.record_contact(0, rival);
    }
    let scores = std::sync::Arc::make_mut(&mut g.observed_score);
    scores.insert(0, 100);
    scores.insert(1, 1_000);
    scores.insert(2, 200);
    let capital = g.player_city_ids(1)[0];
    g.spawn_test_unit("scout", 0, g.cities[&capital].pos);
    g
}

fn controller(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_deny_while_targeted();
    ai.enable_denial_outranks_expansion();
    ai.enable_counter_in_lane();
    ai
}

#[test]
fn urgent_score_leader_keeps_domination_in_a_conquest_plan() {
    let g = board();
    let ai = controller(VictoryTarget::Domination);
    assert_eq!(ai.rival_pressure(&g, 1), (GrandStrategy::Expansion, 97));
    assert_eq!(ai.denial_target(&g, 0), Some((1, GrandStrategy::Conquest)));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest, "{plan:?}");
    assert_eq!(plan.target_player, Some(1));
    assert!(
        !g.is_at_war(0, 1),
        "selecting a counter does not declare war"
    );
}

#[test]
fn score_counter_preserves_other_contracts_and_existing_pressure_gates() {
    let g = board();
    let score = VictoryFocus {
        strategy: GrandStrategy::Expansion,
        progress: 97,
    };
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
    ] {
        let ai = controller(target);
        assert_eq!(
            ai.denial_response_for_pressure(&g, 0, 100, 1, score),
            Some(GrandStrategy::Expansion)
        );
    }
    let mut ai = controller(VictoryTarget::Domination);
    assert_eq!(
        ai.denial_response_for_pressure(
            &g,
            0,
            100,
            1,
            VictoryFocus {
                strategy: GrandStrategy::Science,
                progress: 97
            }
        ),
        Some(GrandStrategy::Science)
    );
    assert_eq!(
        ai.denial_response_for_pressure(
            &g,
            0,
            100,
            1,
            VictoryFocus {
                strategy: GrandStrategy::Expansion,
                progress: 60
            }
        ),
        None
    );
    assert_eq!(
        ai.denial_response_for_pressure(
            &g,
            0,
            0,
            1,
            VictoryFocus {
                strategy: GrandStrategy::Expansion,
                progress: 80
            }
        ),
        Some(GrandStrategy::Expansion),
        "an eligible but nonurgent score alarm keeps the existing policy"
    );
    ai.counter_stand_down = true;
    assert_eq!(ai.denial_response_for_pressure(&g, 0, 100, 1, score), None);
}
