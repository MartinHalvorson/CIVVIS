use super::*;
use crate::game::{HostMenuEntry, HostPurchaseEntry};

fn quoted_force(unit: &str, bank: f64, price: f64) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut game = Game::new_full(2, 24, 16, 80_264, 200, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.found_city_for(0, (6, 6), None);
    game.found_city_for(1, (16, 6), None);
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    game.players[0].techs.extend([
        crate::name!("archery"),
        crate::name!("horseback_riding"),
        crate::name!("stirrups"),
    ]);
    for resource in ["horses", "iron"] {
        game.players[0]
            .strategic_resources
            .insert(Name::new(resource), 50.0);
    }
    game.players[0].government = Some("theocracy".into());
    game.players[0].faith = bank;
    game.players[0].gold = 100.0;
    game.players[0].gold_per_turn = 20.0;
    Arc::make_mut(&mut game.host_buildable).insert(
        home,
        [unit, "knight"]
            .into_iter()
            .map(|name| (format!("unit:{name}"), HostMenuEntry::default()))
            .collect(),
    );
    Arc::make_mut(&mut game.host_purchasable).insert(
        home,
        BTreeMap::from([(
            format!("unit:{unit}"),
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

fn purchase_score(
    game: &Game,
    ai: &AdvancedAi,
    plan: &StrategicPlan,
    city: u32,
    unit: &str,
) -> Option<f64> {
    let counts = ai.counts(game, 0);
    ai.military_faith_score(
        PurchaseScoreContext {
            g: game,
            pid: 0,
            plan,
            counts: &counts,
            bank: game.players[0].faith,
            reserve: 180.0,
        },
        &Action::Buy {
            city,
            unit: Name::new(unit),
            currency: "faith".into(),
            formation: 0,
        },
    )
}

// Native cea88ce70 at t109 bought Horseman80 from264 while Knight,
// Man-at-Arms and Pikeman were available to train. Combat credit must
// not turn the shared production scorer's obsolete-role veto into a bid.
#[test]
fn faith_army_quality_rejects_a_cheap_obsolete_native_offer() {
    for bank in [264.0, 650.0] {
        let (game, ai, plan, home) = quoted_force("horseman", bank, 80.0);
        assert!(game.can_produce(
            0,
            home,
            &Item::Unit {
                unit: crate::name!("knight")
            }
        ));
        assert!(game.legal_actions(0).contains(&Action::Buy {
            city: home,
            unit: crate::name!("horseman"),
            currency: "faith".into(),
            formation: 0,
        }));
        assert!(
            ai.production_value(
                &game,
                0,
                home,
                &Item::Unit {
                    unit: crate::name!("horseman")
                },
                &plan,
                &ai.counts(&game, 0)
            ) < 0.0
        );
        assert_eq!(purchase_score(&game, &ai, &plan, home, "horseman"), None);
    }
}

#[test]
fn faith_army_quality_does_not_spend_a_large_bank_on_a_rejected_role() {
    let (mut game, ai, plan, _) = quoted_force("horseman", 650.0, 80.0);
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 650.0);
    assert!(game.player_unit_ids(0).is_empty());
}

#[test]
fn faith_army_quality_keeps_an_affordable_wanted_shooter() {
    let (mut game, ai, plan, _) = quoted_force("archer", 650.0, 180.0);
    assert!(ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 470.0);
    assert!(game
        .units
        .values()
        .any(|unit| unit.owner == 0 && unit.kind == "archer"));
}

#[test]
fn faith_army_quality_holds_a_full_force_but_still_answers_a_threat() {
    let (mut game, ai, mut plan, home) = quoted_force("archer", 650.0, 90.0);
    for x in 3..11 {
        game.spawn_test_unit("archer", 0, (x, 5));
    }
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 650.0);
    plan.threatened_city = Some(home);
    assert!(ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.players[0].faith, 560.0);
    assert_eq!(game.player_unit_ids(0).len(), 9);
}

#[test]
fn faith_army_quality_keeps_purchase_legality_and_reserves() {
    let (game, ai, plan, home) = quoted_force("archer", 650.0, 500.0);
    assert_eq!(purchase_score(&game, &ai, &plan, home, "archer"), None);
    let (mut game, ai, plan, home) = quoted_force("archer", 650.0, 180.0);
    Arc::make_mut(&mut game.host_purchasable)
        .get_mut(&home)
        .unwrap()
        .clear();
    assert_eq!(purchase_score(&game, &ai, &plan, home, "archer"), None);
}
