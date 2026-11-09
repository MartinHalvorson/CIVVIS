use super::*;

/// A founded capital whose neighbours are all Coast but one, its opening
/// units removed, Shipbuilding known and Faith banked; a barbarian Warrior
/// stands on the land neighbour.
fn coastal_capital(civ: &str, faith: f64) -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, true);
    game.players[0].civ = civ.to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let cid = game.player_city_ids(0)[0];
    let center = game.cities[&cid].pos;
    let neighbours: Vec<Pos> = game.nbrs(center).into_iter().collect();
    for pos in &neighbours[1..] {
        let tile = game.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let land = neighbours[0];
    let tile = game.map.tiles.get_mut(&land).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.feature = None;
    tile.hills = false;
    for tech in ["sailing", "shipbuilding"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].faith = faith;
    let barbarian = game.barb_pid.expect("barbarians are on");
    game.spawn_unit("warrior", barbarian, land);
    (game, cid)
}

/// Gitarja's rule: an Indonesian player may buy a naval unit with Faith; no
/// other civilization may.
#[test]
fn only_indonesia_may_buy_a_ship_with_faith() {
    let (game, cid) = coastal_capital("Indonesia", 500.0);
    assert!(game
        .unit_purchase_cost(0, cid, "quadrireme", "faith")
        .is_some());
    let (game, cid) = coastal_capital("Korea", 500.0);
    assert!(game
        .unit_purchase_cost(0, cid, "quadrireme", "faith")
        .is_none());
}

/// Under the gene a threatened coastal Indonesian city buys the ranged
/// Quadrireme with Faith, not the Galley, and pays in Faith only.
#[test]
fn a_threatened_coastal_city_buys_its_ranged_guard_with_faith() {
    let (mut game, cid) = coastal_capital("Indonesia", 500.0);
    let gold = game.players[0].gold;
    let mut ai = AdvancedAi::new();
    ai.enable_exalted_goddess_buys_the_coastal_guard();
    assert!(ai.exalted_goddess_coastal_guard(&mut game, 0));
    let ships: Vec<Name> = game
        .player_unit_ids(0)
        .into_iter()
        .map(|uid| game.units[&uid].kind)
        .collect();
    assert_eq!(ships, vec![crate::name!("quadrireme")]);
    assert!(game.players[0].faith < 500.0, "paid in Faith");
    assert_eq!(game.players[0].gold, gold, "no Gold spent");
    assert!(game.cities.contains_key(&cid));
    assert!(
        !ai.exalted_goddess_coastal_guard(&mut game, 0),
        "the cooldown holds the second ship"
    );
}

/// Off, or for another civilization, or short of Faith, nothing is bought.
#[test]
fn off_or_another_civ_or_short_of_faith_buys_nothing() {
    let (mut game, _) = coastal_capital("Indonesia", 500.0);
    assert!(!AdvancedAi::new().exalted_goddess_coastal_guard(&mut game, 0));

    let mut ai = AdvancedAi::new();
    ai.enable_exalted_goddess_buys_the_coastal_guard();
    let (mut game, _) = coastal_capital("Korea", 500.0);
    assert!(!ai.exalted_goddess_coastal_guard(&mut game, 0));

    let (mut game, _) = coastal_capital("Indonesia", 10.0);
    assert!(!ai.exalted_goddess_coastal_guard(&mut game, 0));
}

/// Without Shipbuilding a Galley is the only ship. A late world era does not
/// bar it (that era is a clock); a Medieval technology of ours does not
/// either; a Renaissance technology does.
#[test]
fn a_galley_is_measured_against_our_own_technology() {
    let bought = |tech: &str, world_era: usize| {
        let (mut game, _) = coastal_capital("Indonesia", 500.0);
        game.players[0].techs.remove(&Name::new("shipbuilding"));
        game.players[0].techs.insert(Name::new(tech));
        game.world_era = world_era;
        let mut ai = AdvancedAi::new();
        ai.enable_exalted_goddess_buys_the_coastal_guard();
        ai.exalted_goddess_coastal_guard(&mut game, 0)
    };
    assert!(bought("machinery", 6), "Medieval tech, world era 6");
    assert!(!bought("gunpowder", 0), "Renaissance tech, world era 0");
}
