use super::*;

/// Two majors who have met: our founded capital (population 3) on bare
/// Grassland with one buyable forested Plains Hill on its border, the rival's
/// city 9-12 tiles off at 2.5 times our power, every unit removed and
/// everything within nine tiles of the capital in sight. Returns the game,
/// the capital and the Hill.
fn watched() -> Option<(Game, u32, Pos)> {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (9..=12).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, *sites.first()?, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 3;
    city.queue.clear();
    game.turn = 20;
    game.players[0].gold_per_turn = 0.0;
    game.host_observed = Arc::new(game.wdisk(home, 9).into_iter().collect());
    for pos in game.wdisk(home, 3) {
        if pos == home {
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
    let hill = game
        .wdisk(home, 3)
        .into_iter()
        .filter(|pos| game.plot_purchase_cost(0, capital, *pos).is_some())
        .min()?;
    let tile = game.map.tiles.get_mut(&hill)?;
    tile.terrain = crate::name!("plains");
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 250.0);
    Some((game, capital, hill))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

/// A land tile exactly `gap` tiles from the capital holding no unit and not
/// the Hill.
fn ground(game: &Game, capital: u32, gap: i32, hill: Pos) -> Pos {
    let home = game.cities[&capital].pos;
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == gap
                && **pos != hill
                && !game.rules.is_water(tile)
                && !game.units.values().any(|unit| unit.pos == **pos)
        })
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles[0]
}

fn tile_buyer() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_production_tile();
    ai
}

fn armed() -> AdvancedAi {
    let mut ai = tile_buyer();
    ai.enable_watched_capital_keeps_its_defender_gold();
    ai
}

/// The tile the buyer picks, after the seat has looked at its capital.
fn tile_pick(ai: &mut AdvancedAi, game: &Game) -> Option<Pos> {
    ai.remember_capital_watch_sightings(game, 0);
    let plan = ai.assess(game, 0);
    ai.production_tile_choice(game, 0, &plan)
        .map(|(_, _, pos)| pos)
}

/// A 2.5x neighbour's Warrior seven tiles from the unwalled, ungarrisoned
/// capital: with only the tile buyer the Gold goes on the Hill; with the gene
/// the bank keeps two defenders' price and the Hill waits, until the bank
/// holds the price and the Hill both.
#[test]
fn the_watched_capital_keeps_two_defenders_price_before_a_tile() {
    let Some((mut game, capital, hill)) = watched() else {
        panic!("fixture: two majors, a capital and a buyable Hill");
    };
    let at = ground(&game, capital, 7, hill);
    game.spawn_unit("warrior", 1, at);
    let tile_price = game
        .plot_purchase_cost(0, capital, hill)
        .expect("fixture: the Hill is for sale");

    let mut ai = armed();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.remember_capital_watch_sightings(&game, 0);
    let reserve = ai.watched_capital_defender_reserve(&game, 0);
    let warrior = AdvancedAi::capital_defender_price(&game, 0, capital, crate::name!("warrior"))
        .expect("fixture: the capital sells a Warrior");
    assert!(
        reserve > 0.0 && reserve <= 2.0 * warrior + 1e-9,
        "two missing defenders at the cheapest price: {reserve} vs Warrior {warrior}"
    );

    let floor = ai.purchase_upkeep_floor(&game, 0, 0.0);
    game.players[0].gold = floor + tile_price + reserve - 1.0;

    let mut stock = tile_buyer();
    assert_eq!(
        tile_pick(&mut stock, &game),
        Some(hill),
        "without the gene the Gold buys the Hill"
    );
    assert_eq!(
        stock.watched_capital_defender_reserve(&game, 0),
        0.0,
        "off: no reserve"
    );
    assert_eq!(
        tile_pick(&mut ai, &game),
        None,
        "the defenders' price stays in the bank"
    );
    ai.note_watched_capital_defender_reserve(&game, 0);
    assert!(
        ai.base
            .journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline == "The treasury keeps the capital's defender Gold"),
        "the reserve is journalled"
    );

    let mut rich = game.clone();
    rich.players[0].gold = floor + tile_price + reserve + 1.0;
    assert_eq!(
        tile_pick(&mut ai, &rich),
        Some(hill),
        "Gold above the reserve still buys the Hill"
    );
}

/// The reserve lapses when the Walls stand, when two of ours stand at the
/// capital, and with no rival soldier seen near it.
#[test]
fn the_reserve_lapses_with_walls_a_garrison_or_no_sighting() {
    let Some((game, capital, hill)) = watched() else {
        panic!("fixture: two majors, a capital and a buyable Hill");
    };
    let mut ai = armed();
    let mut quiet = game.clone();
    ai.remember_capital_watch_sightings(&quiet, 0);
    quiet.players[0].gold = 500.0;
    assert_eq!(
        ai.watched_capital_defender_reserve(&quiet, 0),
        0.0,
        "no soldier seen: no reserve"
    );

    let mut seen = game.clone();
    let at = ground(&seen, capital, 7, hill);
    seen.spawn_unit("warrior", 1, at);
    let mut ai = armed();
    ai.remember_capital_watch_sightings(&seen, 0);
    let two = ai.watched_capital_defender_reserve(&seen, 0);
    assert!(two > 0.0, "fixture: the watch reads");

    let mut one = seen.clone();
    let home = one.cities[&capital].pos;
    one.spawn_unit("warrior", 0, home);
    let half = ai.watched_capital_defender_reserve(&one, 0);
    assert!(
        (half - two / 2.0).abs() < 1e-6,
        "one of ours: one defender's price ({half} of {two})"
    );
    let beside = ground(&one, capital, 1, hill);
    one.spawn_unit("warrior", 0, beside);
    assert_eq!(
        ai.watched_capital_defender_reserve(&one, 0),
        0.0,
        "full garrison: no reserve"
    );

    let mut walled = seen.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert_eq!(
        ai.watched_capital_defender_reserve(&walled, 0),
        0.0,
        "walls standing: no reserve"
    );

    let mut even = seen.clone();
    power(&mut even, 1, 150.0);
    assert_eq!(
        ai.watched_capital_defender_reserve(&even, 0),
        0.0,
        "under twice our power: no reserve"
    );
}
