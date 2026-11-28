use super::*;

/// Two majors who have met: our founded capital (population 3) building a
/// Builder, the rival's city 9-12 tiles off at three times our power, every
/// unit removed and everything within nine tiles of the capital in sight.
fn watched() -> Option<(Game, u32)> {
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
    game.cities.get_mut(&capital).unwrap().pop = 3;
    game.turn = 20;
    game.players[0].gold = 10.0;
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
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.cities.get_mut(&capital).unwrap().queue.clear();
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: builder,
        },
    )
    .ok()?;
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 300.0);
    Some((game, capital))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

/// An empty land tile exactly `gap` tiles from the capital.
fn ground(game: &Game, capital: u32, gap: i32) -> Pos {
    let home = game.cities[&capital].pos;
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == gap
                && !game.rules.is_water(tile)
                && !game.units.values().any(|unit| unit.pos == **pos)
        })
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles[0]
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_watched_capital_trains_its_defender();
    ai
}

fn head(game: &Game, capital: u32) -> Option<Item> {
    game.cities[&capital].queue.first().cloned()
}

fn is_land_soldier(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::Unit { unit }) if game.rules.units.get(unit).is_some_and(|spec|
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"))
}

/// A 3x neighbour's Warrior seven tiles from the unwalled, ungarrisoned
/// capital and 10 Gold in the bank (no defender to buy): off, the Builder
/// keeps the queue; on, the capital trains a land defender and journals it.
#[test]
fn the_watched_capital_trains_its_defender_over_the_builder() {
    let Some((mut game, capital)) = watched() else {
        panic!("fixture: two majors and a capital building a Builder");
    };
    let at = ground(&game, capital, 7);
    game.spawn_unit("warrior", 1, at);
    let builder = head(&game, capital);
    assert!(
        builder
            == Some(Item::Unit {
                unit: crate::name!("builder")
            }),
        "fixture: the Builder heads the queue"
    );

    let mut off = game.clone();
    let mut stock = AdvancedAi::new();
    stock.remember_capital_watch_sightings(&off, 0);
    stock.claim_watched_capital_defender(&mut off, 0);
    assert_eq!(
        head(&off, capital),
        builder,
        "off: the Builder keeps the queue"
    );

    let mut ai = armed();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.remember_capital_watch_sightings(&game, 0);
    ai.claim_watched_capital_defender(&mut game, 0);
    assert!(
        is_land_soldier(&game, &head(&game, capital)),
        "on: a land defender heads the queue, got {:?}",
        head(&game, capital)
    );
    assert!(
        ai.base
            .journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought
                .detail
                .contains("watched-capital-trains-its-defender:")),
        "the claim is journalled"
    );
}

/// The claim lapses with no sighting, with two of ours at the capital, under
/// the 2.5x bar, and with the Walls standing.
#[test]
fn the_claim_lapses_with_a_garrison_walls_a_low_ratio_or_no_sighting() {
    let Some((game, capital)) = watched() else {
        panic!("fixture: two majors and a capital building a Builder");
    };
    let builder = head(&game, capital);

    let mut quiet = game.clone();
    let mut ai = armed();
    ai.remember_capital_watch_sightings(&quiet, 0);
    ai.claim_watched_capital_defender(&mut quiet, 0);
    assert_eq!(head(&quiet, capital), builder, "no sighting: no claim");

    let mut seen = game.clone();
    let at = ground(&seen, capital, 7);
    seen.spawn_unit("warrior", 1, at);
    let mut ai = armed();
    ai.remember_capital_watch_sightings(&seen, 0);

    let mut kept = seen.clone();
    let home = kept.cities[&capital].pos;
    kept.spawn_unit("warrior", 0, home);
    let beside = ground(&kept, capital, 1);
    kept.spawn_unit("warrior", 0, beside);
    ai.claim_watched_capital_defender(&mut kept, 0);
    assert_eq!(head(&kept, capital), builder, "full garrison: no claim");

    let mut low = seen.clone();
    power(&mut low, 1, 240.0);
    ai.claim_watched_capital_defender(&mut low, 0);
    assert_eq!(head(&low, capital), builder, "2.4x: no claim");

    let mut walled = seen.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    ai.claim_watched_capital_defender(&mut walled, 0);
    assert_eq!(head(&walled, capital), builder, "walls standing: no claim");

    let mut short = seen.clone();
    ai.claim_watched_capital_defender(&mut short, 0);
    assert!(
        is_land_soldier(&short, &head(&short, capital)),
        "fixture: the same board without them claims"
    );
}
