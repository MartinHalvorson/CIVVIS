use super::*;

/// Two Archers with Crossbowmen unlocked, Mercenaries known, a full Monarchy
/// deck, and 200 Gold against a two-unit bill.
fn fixture(target: VictoryTarget, on: bool) -> (Game, AdvancedAi, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 366_400, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = 125;
    g.players[0].techs.insert(crate::name!("machinery"));
    g.players[0].gold = 200.0;
    g.players[0].gold_per_turn = 18.0;
    g.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "early_empire",
            "state_workforce",
            "political_philosophy",
            "military_tradition",
            "games_recreation",
            "defensive_tactics",
            "feudalism",
            "military_training",
            "mercenaries",
        ]
        .into_iter()
        .map(Name::new),
    );
    g.players[0].government = Some("monarchy".into());
    g.players[0].civic = None;
    g.players[0].policies = [
        "conscription",
        "retainers",
        "feudal_contract",
        "charismatic_leader",
        "urban_planning",
        "inspiration",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    let units = [(4, 4), (4, 5)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("archer", 0, pos))
        .collect();
    let mut ai = AdvancedAi::targeting(target);
    if on {
        ai.enable_upgrade_card_for_the_backlog();
    }
    (g, ai, units)
}

fn crossbow_price(g: &Game) -> f64 {
    g.unit_upgrade_price_in_formation(0, crate::name!("archer"), crate::name!("crossbowman"), 0)
        .unwrap()
        .0
}

fn holds_the_card(g: &Game) -> bool {
    g.players[0]
        .policies
        .contains(&crate::name!("professional_army"))
}

#[test]
fn off_the_score_seat_keeps_its_deck() {
    let (mut g, ai, _) = fixture(VictoryTarget::Score, false);
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
    let price = crossbow_price(&g);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Expansion);
    assert!(!holds_the_card(&g));
    assert_eq!(crossbow_price(&g), price);
}

#[test]
fn a_score_seat_with_a_backlog_slots_the_half_price_card() {
    let (mut g, ai, _) = fixture(VictoryTarget::Score, true);
    assert_eq!(ai.upgrade_card_backlog(&g, 0).0, 2);
    assert_eq!(ai.backlog_upgrade_card(&g, 0), Some("professional_army"));
    let full = crossbow_price(&g);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Expansion);
    assert!(holds_the_card(&g));
    assert!(crossbow_price(&g) < full);
}

#[test]
fn every_target_but_domination_takes_it() {
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
    ] {
        let (g, ai, _) = fixture(target, true);
        assert_eq!(
            ai.backlog_upgrade_card(&g, 0),
            Some("professional_army"),
            "{target:?}"
        );
    }
    // Domination keeps `domination_upgrade_policy`, which wants the same card.
    let (g, ai, _) = fixture(VictoryTarget::Domination, true);
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
    assert_eq!(
        ai.domination_upgrade_policy(&g, 0),
        Some("professional_army")
    );
}

#[test]
fn one_unit_or_a_treasury_that_clears_the_bill_wants_no_card() {
    let (mut g, ai, units) = fixture(VictoryTarget::Score, true);
    g.players[0].gold = 100.0 + 2.0 * crossbow_price(&g) + 1.0;
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
    g.players[0].gold = 200.0;
    g.remove_unit(units[1]);
    assert_eq!(ai.upgrade_card_backlog(&g, 0).0, 1);
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
}

#[test]
fn a_held_card_stays_while_one_unit_waits() {
    let (mut g, ai, units) = fixture(VictoryTarget::Score, true);
    g.players[0].policies.remove(&crate::name!("retainers"));
    g.players[0]
        .policies
        .insert(crate::name!("professional_army"));
    g.players[0].gold = 10_000.0;
    g.remove_unit(units[1]);
    assert_eq!(ai.backlog_upgrade_card(&g, 0), Some("professional_army"));
    g.remove_unit(units[0]);
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
}

#[test]
fn no_mercenaries_no_card_and_no_successor_no_backlog() {
    let (mut g, ai, _) = fixture(VictoryTarget::Score, true);
    g.players[0].civics.remove(&crate::name!("mercenaries"));
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
    let (mut g, ai, _) = fixture(VictoryTarget::Score, true);
    g.players[0].techs.remove(&crate::name!("machinery"));
    assert_eq!(ai.upgrade_card_backlog(&g, 0), (0, 0.0));
    assert_eq!(ai.backlog_upgrade_card(&g, 0), None);
}
