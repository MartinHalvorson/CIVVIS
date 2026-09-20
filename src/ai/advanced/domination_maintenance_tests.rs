use super::*;

fn deficit_campaign() -> (Game, AdvancedAi) {
    let mut game = Game::new(2, 24, 16, 936015, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].government = Some("chiefdom".to_string());
    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game.players[0]
        .policies
        .extend([crate::name!("discipline"), crate::name!("urban_planning")]);
    game.players[0].gold = 66.0;
    game.players[0].gold_per_turn = -8.8;
    game.at_war.insert((0, 1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.disable_war_economy();
    (game, ai)
}

#[test]
fn domination_deficit_slots_conscription_without_optional_war_economy() {
    let (mut game, ai) = deficit_campaign();
    ai.strategic_policies(&mut game, 0, GrandStrategy::Recovery);
    assert!(game.players[0]
        .policies
        .contains(&crate::name!("conscription")));
    assert!(!game.players[0]
        .policies
        .contains(&crate::name!("discipline")));
}

#[test]
fn domination_deficit_uses_the_available_successor() {
    let (mut game, ai) = deficit_campaign();
    game.players[0].civics.insert(crate::name!("mobilization"));
    ai.strategic_policies(&mut game, 0, GrandStrategy::Recovery);
    assert!(game.players[0]
        .policies
        .contains(&crate::name!("levee_en_masse")));
    assert!(!game.players[0]
        .policies
        .contains(&crate::name!("conscription")));
}

#[test]
fn domination_staged_campaign_preserves_maintenance_reserve() {
    let (mut game, mut ai) = deficit_campaign();
    game.at_war.clear();
    game.players[0]
        .civics
        .extend([crate::name!("mobilization"), crate::name!("scorched_earth")]);
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    });
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(game.players[0]
        .policies
        .contains(&crate::name!("levee_en_masse")));
    assert!(!game.players[0]
        .policies
        .contains(&crate::name!("total_war")));
}

#[test]
fn domination_maintenance_relief_requires_deficit_and_short_reserve() {
    for (gold, income) in [(125.0, -8.8), (66.0, 0.0), (66.0, -0.5)] {
        let (mut game, ai) = deficit_campaign();
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = income;
        ai.strategic_policies(&mut game, 0, GrandStrategy::Recovery);
        assert!(
            !game.players[0]
                .policies
                .contains(&crate::name!("conscription")),
            "{gold}/{income}"
        );
    }
}

#[test]
fn peaceful_domination_and_other_targets_keep_existing_policy_priority() {
    let (mut game, ai) = deficit_campaign();
    game.at_war.clear();
    ai.strategic_policies(&mut game, 0, GrandStrategy::Recovery);
    assert!(!game.players[0]
        .policies
        .contains(&crate::name!("conscription")));
    for target in [
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Culture),
    ] {
        let (mut game, _) = deficit_campaign();
        let ai = target
            .map(AdvancedAi::targeting)
            .unwrap_or_else(AdvancedAi::new);
        ai.strategic_policies(&mut game, 0, GrandStrategy::Recovery);
        assert!(!game.players[0]
            .policies
            .contains(&crate::name!("conscription")));
    }
}

#[test]
fn domination_keeps_relief_until_the_cash_reserve_recovers() {
    let (mut game, ai) = deficit_campaign();
    game.players[0].policies.remove(&crate::name!("discipline"));
    game.players[0]
        .policies
        .insert(crate::name!("conscription"));
    game.players[0].gold_per_turn = 2.0;
    game.players[0]
        .civics
        .insert(crate::name!("scorched_earth"));
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        game.players[0]
            .policies
            .contains(&crate::name!("conscription")),
        "the relief itself must not trigger its immediate removal"
    );
    game.players[0].gold = 125.0;
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        !game.players[0]
            .policies
            .contains(&crate::name!("conscription")),
        "a recovered reserve releases the ordinary policy portfolio"
    );
}
