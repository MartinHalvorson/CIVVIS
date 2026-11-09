use super::*;

/// One city of ours holding a single Archer, Machinery researched (the
/// Crossbowman unlocked) and `gold` in the treasury.
fn archer_seat(gold: f64) -> (Game, u32) {
    let mut game = Game::new_full(2, 40, 24, 91_776, 250, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("a starting settler");
    let pos = game.units[&settler].pos;
    game.remove_unit(settler);
    game.found_city_for(0, pos, None);
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let archer = game.spawn_test_unit("archer", 0, pos);
    game.players[0].techs.insert(crate::name!("machinery"));
    game.players[0].gold = gold;
    (game, archer)
}

/// With the gene on, parity's Gold upgrades the Archer we hold to a
/// Crossbowman, down to the parity reserve; off, nothing is upgraded here.
#[test]
fn parity_upgrades_the_army_we_hold_first() {
    let (game, archer) = archer_seat(1_000.0);
    assert!(
        game.unit_gold_upgrade_offer(0, archer).is_some(),
        "fixture: the Archer can upgrade"
    );
    let off = AdvancedAi::new();
    let mut untouched = game.clone();
    assert_eq!(
        off.parity_upgrades_first(&mut untouched, 0, 1, 50.0, 100.0),
        0
    );
    assert_eq!(untouched.units[&archer].kind, "archer");

    let mut on = AdvancedAi::new();
    on.enable_parity_upgrades_before_it_buys();
    let mut upgraded = game.clone();
    assert_eq!(
        on.parity_upgrades_first(&mut upgraded, 0, 1, 50.0, 100.0),
        1
    );
    let unit = upgraded
        .player_unit_ids(0)
        .into_iter()
        .next()
        .expect("the unit survives its upgrade");
    assert_eq!(upgraded.units[&unit].kind, "crossbowman");
    assert!(upgraded.players[0].gold >= BORDER_PARITY_RESERVE);

    // At the reserve, the army waits.
    let (mut poor, _) = archer_seat(BORDER_PARITY_RESERVE);
    assert_eq!(on.parity_upgrades_first(&mut poor, 0, 1, 50.0, 100.0), 0);
}

/// The buy is barred at twice our power, and for a unit two eras behind the
/// world; off, never.
#[test]
fn parity_buys_neither_out_of_reach_nor_obsolete() {
    let (mut game, _) = archer_seat(1_000.0);
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_parity_upgrades_before_it_buys();

    game.world_era = 0;
    assert_eq!(
        off.parity_buy_barred(&game, 0, "warrior", 100.0, 500.0),
        None
    );
    assert_eq!(
        on.parity_buy_barred(&game, 0, "warrior", 100.0, 200.0),
        Some("out_of_reach")
    );
    assert_eq!(
        on.parity_buy_barred(&game, 0, "warrior", 112.0, 200.0),
        None
    );

    game.world_era = game.unit_era("warrior") + PARITY_BUY_ERAS_BEHIND;
    assert_eq!(
        on.parity_buy_barred(&game, 0, "warrior", 112.0, 200.0),
        Some("obsolete")
    );
    assert_eq!(
        off.parity_buy_barred(&game, 0, "warrior", 112.0, 200.0),
        None
    );
}
