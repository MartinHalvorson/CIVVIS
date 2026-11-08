use super::*;
use crate::ai::industrial_zones::tests::producer_empire;

fn fixture() -> (AdvancedAi, Game) {
    let (mut game, cities) = producer_empire("CULTUREWORKBUY", 472_301, 3);
    game.turn = 63;
    game.players[0].gold = 1_000.0;
    game.players[0].met.extend([1, 2]);
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    (AdvancedAi::targeting(VictoryTarget::Culture), game)
}

#[test]
fn spare_slots_and_money_can_buy_from_each_met_major() {
    let (ai, mut game) = fixture();
    let (first, ceiling, _, _) = ai
        .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
        .unwrap();
    assert_eq!(ceiling, 500);
    game.turn += 6;
    let (second, _, _, _) = ai
        .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
        .unwrap();
    assert_ne!(first, second, "a refusal must not monopolize every ask");
}

#[test]
fn keeps_cash_and_income_reserves_and_does_not_buy_on_unknown_income() {
    let (ai, mut game) = fixture();
    game.players[0].gold = 200.0; // Six cities: income replenishes the base reserve.
    let (_, total, cash, gpt) = ai
        .culture_work_purchase_ask(&game, 0, 200.0, Some(30.0), false)
        .unwrap();
    assert_eq!((total, cash, gpt), (500, 170, 20));
    let (_, total, cash, gpt) = ai
        .culture_work_purchase_ask(&game, 0, 200.0, Some(10.0), false)
        .unwrap();
    assert_eq!((total, cash, gpt), (230, 130, 4));
    assert!(ai
        .culture_work_purchase_ask(&game, 0, game.players[0].gold, None, false)
        .is_none());
    assert!(ai
        .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(f64::NAN), false)
        .is_none());
    game.players[0].gold = 50.0;
    let (_, total, cash, gpt) = ai
        .culture_work_purchase_ask(&game, 0, 50.0, Some(10.0), false)
        .unwrap();
    assert_eq!(
        (total, cash, gpt),
        (100, 0, 4),
        "only a GPT-only quote may use this budget"
    );
    assert!(ai
        .culture_work_purchase_ask(&game, 0, 50.0, Some(6.0), false)
        .is_none());
}

#[test]
fn a_replenishing_treasury_can_fund_the_observed_early_writing_quote() {
    let (ai, mut game) = fixture();
    game.players[0].gold = 160.0;
    let (_, ceiling, cash, gpt) = ai
        .culture_work_purchase_ask(&game, 0, 160.0, Some(32.0), false)
        .unwrap();
    // Native 192016Z/t75: an empty Writing economy, 160 Gold, 32 GPT,
    // and a quote rejected solely because its 108 Gold exceeded the old 70.
    let (quoted_gold, quoted_gpt) = (108, 11);
    assert!(quoted_gold <= cash && quoted_gpt <= gpt);
    assert!(quoted_gold + 25 * quoted_gpt <= ceiling);
    assert!(game.players[0].gold - quoted_gold as f64 >= 30.0);
    assert!(32.0 - quoted_gpt as f64 >= 5.0);

    let (_, _, committed_cash, _) = ai
        .culture_work_purchase_ask(&game, 0, 100.0, Some(32.0), false)
        .unwrap();
    assert!(
        committed_cash < quoted_gold,
        "income cannot spend cash already committed to a higher-priority purchase"
    );
}

#[test]
fn strong_income_never_creates_unowned_cash_or_overrides_danger() {
    let (mut ai, mut game) = fixture();
    game.players[0].gold = 20.0;
    let (_, ceiling, cash, gpt) = ai
        .culture_work_purchase_ask(&game, 0, 20.0, Some(100.0), false)
        .unwrap();
    assert_eq!(cash, 0, "future income is not a loan of spendable Gold");
    assert!(ceiling <= 500 && gpt <= 20);

    game.players[0].gold = 1_000.0;
    let mut plan = ai.assess(&game, 0);
    plan.threatened_city = Some(game.player_city_ids(0)[0]);
    ai.plan = Some(plan);
    assert!(ai
        .culture_work_purchase_ask(&game, 0, 1_000.0, Some(100.0), false)
        .is_none());
}

#[test]
fn other_objectives_people_and_off_cadence_keep_their_money() {
    let (ai, mut game) = fixture();
    assert!(ai
        .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), true)
        .is_none());
    for target in VictoryTarget::ALL
        .into_iter()
        .filter(|target| *target != VictoryTarget::Culture)
    {
        let other = AdvancedAi::targeting(target);
        assert!(other
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
            .is_none());
    }
    game.turn += 1;
    assert!(ai
        .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
        .is_none());
}

#[test]
fn existing_spending_war_counterparties_and_threatened_cities_preempt_the_purchase() {
    let (ai, mut game) = fixture();
    let (_, _, cash, _) = ai
        .culture_work_purchase_ask(&game, 0, 150.0, Some(30.0), false)
        .unwrap();
    assert_eq!(
        cash, 120,
        "cash committed elsewhere stays unavailable even to a financed quote"
    );
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    assert_eq!(
        ai.culture_work_purchase_ask(&game, 0, 1_000.0, Some(30.0), false)
            .unwrap()
            .0,
        2,
        "a remote war does not forbid trade with the other rival"
    );
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().last_attacked = game.turn;
    assert!(ai
        .culture_work_purchase_ask(&game, 0, 1_000.0, Some(30.0), false)
        .is_none());
}

#[test]
fn native_financed_quotes_fit_without_spending_the_cash_reserve() {
    let (ai, mut game) = fixture();
    let reserve = 60.0 + 5.0 * game.player_city_ids(0).len() as f64;
    for (cash_room, income, quoted_gold, quoted_gpt) in
        [(215.0, 48.0039, 118, 14), (227.0, 42.1328, 105, 12)]
    {
        game.players[0].gold = reserve + cash_room;
        let (_, total, cash, gpt) = ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(income), false)
            .unwrap();
        assert!(quoted_gold + 25 * quoted_gpt <= total);
        assert!(quoted_gold <= cash && quoted_gpt <= gpt);
        assert!(game.players[0].gold - quoted_gold as f64 >= reserve);
        assert!(income - quoted_gpt as f64 >= 5.0);
    }
}
