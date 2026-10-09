use super::*;
use crate::game::HostPurchaseEntry;

fn quoted_army(bank: f64, price: f64) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut game = Game::new_full(2, 24, 16, 600_433, 200, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.found_city_for(0, (6, 6), None);
    game.found_city_for(1, (16, 6), None);
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].government = Some("theocracy".into());
    game.players[0].faith = bank;
    game.players[0].gold = 100.0;
    game.players[0].gold_per_turn = 20.0;
    Arc::make_mut(&mut game.host_purchasable).insert(
        home,
        BTreeMap::from([(
            "unit:archer".into(),
            HostPurchaseEntry {
                gold: None,
                faith: Some(price),
            },
        )]),
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: game.player_city_ids(1).first().copied(),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_solvent_faith_army();
    (game, ai, plan, home)
}

#[test]
fn native_faith_army_spends_a_quoted_surplus_below_600() {
    let (mut game, ai, plan, home) = quoted_army(433.0, 180.0);
    assert_eq!(
        game.unit_purchase_cost(0, home, "archer", "faith"),
        Some(180.0)
    );
    assert!(ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 253.0);
    assert!(game
        .units
        .values()
        .any(|unit| unit.owner == 0 && unit.kind == "archer"));
}

#[test]
fn native_faith_army_keeps_the_reserve_and_upkeep_guard() {
    let (mut game, ai, plan, _) = quoted_army(350.0, 180.0);
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 350.0);
    assert!(game.player_unit_ids(0).is_empty());

    let (mut game, ai, plan, _) = quoted_army(433.0, 180.0);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -3.0;
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 433.0);
    assert!(game.player_unit_ids(0).is_empty());
}

#[test]
fn native_faith_army_unquoted_model_keeps_the_old_floor() {
    let (mut game, ai, plan, _) = quoted_army(433.0, 180.0);
    Arc::make_mut(&mut game.host_purchasable).clear();
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 433.0);
    assert!(game.player_unit_ids(0).is_empty());
}

#[test]
fn native_faith_army_does_not_invent_a_formation_quote() {
    let (mut game, ai, plan, _) = quoted_army(433.0, 180.0);
    game.players[0].civics.insert(crate::name!("nationalism"));
    let log_start = game.log.len();
    assert!(ai.military_faith_spending(&mut game, 0, &plan));
    assert!(game.log.since(log_start).any(|(_, action)| matches!(
        action, Action::Buy { formation: 0, currency, .. } if currency == "faith"
    )));
    assert!(!game.log.since(log_start).any(|(_, action)| matches!(
        action, Action::Buy { formation: 1.., currency, .. } if currency == "faith"
    )));
}
