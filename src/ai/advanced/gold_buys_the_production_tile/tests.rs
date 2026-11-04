use super::*;

/// One major, our capital founded at turn 20 at population 3 with every unit
/// removed, a bank of `gold`, and every land tile within three of the capital
/// made bare Grassland (2 Food, 0 Production). Returns the game and the
/// capital.
fn grassland_capital(gold: f64) -> Option<(Game, u32)> {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    game.turn = 20;
    game.cities.get_mut(&capital).unwrap().pop = 3;
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = 0.0;
    let center = game.cities[&capital].pos;
    for pos in game.wdisk(center, 3) {
        if pos == center {
            continue;
        }
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            tile.terrain = crate::name!("grassland");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }
    }
    Some((game, capital))
}

/// The first border tile the capital may buy, made a forested Plains Hill
/// (1 Food, 3 Production).
fn forested_hill(game: &mut Game, capital: u32) -> Option<Pos> {
    let center = game.cities[&capital].pos;
    let pos = game
        .wdisk(center, 3)
        .into_iter()
        .filter(|pos| game.plot_purchase_cost(0, capital, *pos).is_some())
        .min()?;
    let tile = game.map.tiles.get_mut(&pos)?;
    tile.terrain = crate::name!("plains");
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    Some(pos)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_production_tile();
    ai
}

/// A citizen on bare Grassland beside a buyable forested Hill: the gene buys
/// the Hill, and with the gene off the bank is untouched.
#[test]
fn a_city_on_grassland_buys_the_forested_hill() {
    let Some((mut game, capital)) = grassland_capital(500.0) else {
        panic!("fixture: a young empire");
    };
    let Some(hill) = forested_hill(&mut game, capital) else {
        panic!("fixture: a buyable border tile");
    };
    assert!(
        game.modeled_tile_yields(hill).production >= PRODUCTION_TILE_MIN_PRODUCTION,
        "fixture: the hill pays Production"
    );

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert!(!stock.production_tile_purchase(&mut off, 0, &plan));
    assert_eq!(off.players[0].gold, game.players[0].gold);
    assert_eq!(off.map.tiles[&hill].owner_city, None);

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert_eq!(
        ai.production_tile_choice(&on, 0, &plan)
            .map(|(_, _, pos)| pos),
        Some(hill)
    );
    assert!(ai.production_tile_purchase(&mut on, 0, &plan));
    assert_eq!(on.map.tiles[&hill].owner_city, Some(capital));
    assert!(on.players[0].gold < game.players[0].gold, "on: Gold spent");
}

/// A border tile no richer in Production than what the city already works is
/// not bought.
#[test]
fn a_tile_no_better_than_the_worked_ones_is_left() {
    let Some((mut game, capital)) = grassland_capital(500.0) else {
        panic!("fixture: a young empire");
    };
    // Every tile in range is a forested Plains Hill: nothing to upgrade to.
    let center = game.cities[&capital].pos;
    for pos in game.wdisk(center, 3) {
        if pos == center {
            continue;
        }
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            tile.terrain = crate::name!("plains");
            tile.hills = true;
            tile.feature = Some(crate::name!("forest"));
        }
    }
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert_eq!(ai.production_tile_choice(&game, 0, &plan), None);
}

/// An empty bank buys nothing, and the window shuts at the standard turn
/// limit.
#[test]
fn the_purchase_needs_the_gold_and_the_opening() {
    let ai = armed();
    let Some((mut poor, capital)) = grassland_capital(0.0) else {
        panic!("fixture: a young empire");
    };
    forested_hill(&mut poor, capital);
    let plan = ai.assess(&poor, 0);
    assert_eq!(ai.production_tile_choice(&poor, 0, &plan), None);

    let Some((mut late, capital)) = grassland_capital(500.0) else {
        panic!("fixture: a young empire");
    };
    forested_hill(&mut late, capital);
    late.turn = late.standard_duration(PRODUCTION_TILE_END_STANDARD);
    let plan = ai.assess(&late, 0);
    assert_eq!(ai.production_tile_choice(&late, 0, &plan), None);
}
