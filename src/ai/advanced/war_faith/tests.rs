use super::super::*;
use crate::game::HostPurchaseEntry;

/// Two majors at war, seat 0 in Theocracy with `bank` Faith and a host quote
/// of `price` Faith for an Archer in its capital; a Conquest plan on seat 1.
fn war_bank(bank: f64, price: f64) -> (Game, AdvancedAi, StrategicPlan) {
    let mut game = Game::new_full(2, 24, 16, 600_433, 200, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.found_city_for(0, (6, 6), None);
    game.found_city_for(1, (16, 6), None);
    game.current = 0;
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
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
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    (game, ai, plan)
}

fn archers(game: &Game) -> usize {
    game.units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "archer")
        .count()
}

#[test]
fn war_faith_runs_only_under_the_gene_at_war_for_domination() {
    let (mut game, mut ai, plan) = war_bank(200.0, 180.0);
    assert!(!ai.war_faith_buys_the_army_now(&game, 0, &plan), "off");
    ai.enable_war_faith_buys_the_army();
    assert!(ai.war_faith_buys_the_army_now(&game, 0, &plan));

    game.at_war.remove(&(0, 1));
    assert!(!ai.war_faith_buys_the_army_now(&game, 0, &plan), "at peace");
    game.at_war.insert((0, 1));

    let mut expansion = plan.clone();
    expansion.strategy = GrandStrategy::Expansion;
    assert!(!ai.war_faith_buys_the_army_now(&game, 0, &expansion));

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_war_faith_buys_the_army();
    assert!(!science.war_faith_buys_the_army_now(&game, 0, &plan));
}

/// 200 Faith against a 180 quote: the ordinary reserve (180 and up) refuses
/// the purchase, the war reserve (the inquisition's, here none) makes it.
#[test]
fn the_war_reserve_buys_what_the_ordinary_reserve_holds_back() {
    let (mut game, mut ai, plan) = war_bank(200.0, 180.0);
    ai.enable_war_faith_buys_the_army();
    assert!(!ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(archers(&game), 0, "fixture: the ordinary pass holds");

    let reserve = ai.war_faith_reserve(&game, 0);
    assert_eq!(reserve, 0.0, "fixture: no inquisition is due");
    assert!(ai.military_faith_spending_above(&mut game, 0, &plan, reserve));
    assert_eq!(archers(&game), 1);
    assert_eq!(game.players[0].faith, 20.0);
}
