use super::*;
use crate::game::HostPurchaseEntry;

// Native 030214 t119 bought Man-at-Arms with 77 Gold and -11.2891 income;
// readback verified the purchase at120. Its 3 upkeep alone fits the old
// 25-turn reserve, while the existing deficit consumes the same bank.
#[test]
fn native_faith_army_prices_the_existing_deficit_with_the_new_unit() {
    let (mut game, ai, _, _) = quoted_army(393.0, 180.0);
    game.players[0].gold = 77.0;
    game.players[0].gold_per_turn = -11.2891;
    assert!(!ai.faith_military_is_affordable(&game, 0, "man_at_arms"));
}

#[test]
fn native_faith_army_withholds_a_quoted_purchase_that_cannot_carry_the_deficit() {
    let (mut game, ai, plan, home) = quoted_army(393.0, 180.0);
    game.players[0].gold = 77.0;
    game.players[0].gold_per_turn = -11.2891;
    assert_eq!(
        game.unit_purchase_cost(0, home, "archer", "faith"),
        Some(180.0)
    );
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 393.0);
    assert!(game.player_unit_ids(0).is_empty());
}

#[test]
fn native_faith_army_can_carry_a_deficit_with_a_funded_bank() {
    let (mut game, ai, _, _) = quoted_army(393.0, 180.0);
    game.players[0].gold = 400.0;
    game.players[0].gold_per_turn = -11.2891;
    assert!(ai.faith_military_is_affordable(&game, 0, "man_at_arms"));
}

#[test]
fn native_faith_army_preserves_surplus_and_partial_surplus_admission() {
    let (mut game, ai, _, _) = quoted_army(393.0, 180.0);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = 3.0;
    assert!(ai.faith_military_is_affordable(&game, 0, "man_at_arms"));
    game.players[0].gold = 25.0;
    game.players[0].gold_per_turn = 0.5;
    assert!(ai.faith_military_is_affordable(&game, 0, "archer"));
    game.players[0].gold = 24.0;
    assert!(!ai.faith_military_is_affordable(&game, 0, "archer"));
}

#[test]
fn native_faith_army_preserves_zero_upkeep_and_the_explicit_withhold() {
    let (mut game, mut ai, _, _) = quoted_army(393.0, 180.0);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -20.0;
    assert!(ai.faith_military_is_affordable(&game, 0, "warrior"));
    ai.disable_solvent_faith_army();
    assert!(ai.faith_military_is_affordable(&game, 0, "man_at_arms"));
}

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

// Native f0ae0f319 bought a Scout at t114 and again at t116-t118,
// with banks of 234, 229 and 224. A covered recon role must not absorb
// each small surplus merely because it has positive combat strength.
#[test]
fn native_faith_army_keeps_small_surpluses_from_redundant_scouts() {
    for (bank, archer_price, buys_archer) in [(234.0, 180.0, false), (300.0, 90.0, true)] {
        let (mut game, ai, plan, home) = quoted_army(bank, archer_price);
        game.spawn_test_unit("scout", 0, game.cities[&home].pos);
        Arc::make_mut(&mut game.host_purchasable)
            .get_mut(&home)
            .unwrap()
            .insert(
                "unit:scout".into(),
                HostPurchaseEntry {
                    gold: None,
                    faith: Some(30.0),
                },
            );
        let scout = Action::Buy {
            city: home,
            unit: crate::name!("scout"),
            formation: 0,
            currency: "faith".into(),
        };
        assert!(game.legal_actions(0).contains(&scout));
        assert!(
            ai.production_value(
                &game,
                0,
                home,
                &Item::Unit {
                    unit: crate::name!("scout")
                },
                &plan,
                &ai.counts(&game, 0)
            ) < 0.0
        );
        assert_eq!(ai.military_faith_spending(&mut game, 0, &plan), buys_archer);
        assert_eq!(
            game.units
                .values()
                .filter(|unit| unit.owner == 0 && unit.kind == "scout")
                .count(),
            1
        );
        assert_eq!(
            game.players[0].faith,
            if buys_archer {
                bank - archer_price
            } else {
                bank
            }
        );
        assert_eq!(
            game.units
                .values()
                .any(|unit| unit.owner == 0 && unit.kind == "archer"),
            buys_archer
        );
    }
}
