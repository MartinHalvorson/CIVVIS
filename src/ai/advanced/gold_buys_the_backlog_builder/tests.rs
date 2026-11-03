use super::*;

/// One major at turn 60 with two populous cities working unimproved hills, a
/// bank of `gold`, Mining known, no units and no income.
fn backlog_empire(gold: f64) -> Option<(Game, u32, u32)> {
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
    let at = game.cities[&capital].pos;
    let second_pos = (at.0 + 5, at.1);
    {
        let tile = game.map.tiles.get_mut(&second_pos)?;
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let second = game.place_city(0, second_pos, None);
    game.turn = 60;
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = 0.0;
    game.players[0].techs.insert(crate::name!("mining"));
    for cid in [capital, second] {
        game.cities.get_mut(&cid).unwrap().pop = 5;
        let center = game.cities[&cid].pos;
        let owned: Vec<Pos> = game.cities[&cid].owned_tiles.to_vec();
        for position in owned {
            if position != center && game.map.tiles[&position].district.is_none() {
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.terrain = crate::name!("grassland");
                tile.hills = true;
                tile.feature = None;
                tile.resource = None;
                tile.improvement = None;
            }
        }
    }
    Some((game, capital, second))
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_backlog_builder();
    ai
}

fn builders(game: &Game) -> usize {
    game.player_unit_ids(0)
        .into_iter()
        .filter(|uid| game.units[uid].kind == "builder")
        .count()
}

fn fixture(gold: f64) -> (Game, u32, u32) {
    let Some(fixture) = backlog_empire(gold) else {
        panic!("fixture: a two-city empire");
    };
    let (game, capital, _) = &fixture;
    assert!(
        BasicAi::unimproved_worked_tiles(game, 0) >= 3,
        "fixture: the cities work unimproved hills ({})",
        BasicAi::unimproved_worked_tiles(game, 0)
    );
    assert!(
        game.unit_purchase_cost(0, *capital, "builder", "gold")
            .is_some(),
        "fixture: a Builder is for sale"
    );
    fixture
}

#[test]
fn off_the_backlog_buys_nothing() {
    let (game, _, _) = fixture(2_000.0);
    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert!(!stock.backlog_builder_purchase(&mut off, 0, &plan));
    assert_eq!(builders(&off), 0);
    assert_eq!(off.players[0].gold, game.players[0].gold);
}

#[test]
fn on_a_short_backlog_buys_one_builder_where_the_backlog_is() {
    let (game, capital, second) = fixture(2_000.0);
    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(ai.backlog_builder_shortfall(&on, 0, &plan).is_some());
    assert!(ai.backlog_builder_purchase(&mut on, 0, &plan));
    assert_eq!(builders(&on), 1);
    assert!(on.players[0].gold < game.players[0].gold);
    let builder = on
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| on.units[uid].kind == "builder")
        .unwrap();
    let pos = on.units[&builder].pos;
    assert!(
        pos == on.cities[&capital].pos || pos == on.cities[&second].pos,
        "bought in one of our cities"
    );
    // Two cities ask for at most one Builder: the quota is met now.
    let plan = ai.assess(&on, 0);
    assert!(ai.backlog_builder_shortfall(&on, 0, &plan).is_none());
    assert!(!ai.backlog_builder_purchase(&mut on, 0, &plan));
    assert_eq!(builders(&on), 1);
}

#[test]
fn a_queued_builder_counts_and_an_empty_treasury_waits() {
    let (game, capital, _) = fixture(2_000.0);
    let ai = armed();
    let mut queued = game.clone();
    queued
        .apply(
            0,
            &Action::Produce {
                city: capital,
                item: Item::Unit {
                    unit: crate::name!("builder"),
                },
            },
        )
        .unwrap();
    let plan = ai.assess(&queued, 0);
    assert!(ai.backlog_builder_shortfall(&queued, 0, &plan).is_none());
    assert!(!ai.backlog_builder_purchase(&mut queued, 0, &plan));

    let (poor, _, _) = fixture(0.0);
    let mut poor = poor;
    let plan = ai.assess(&poor, 0);
    assert!(!ai.backlog_builder_purchase(&mut poor, 0, &plan));
    assert_eq!(builders(&poor), 0);
}

#[test]
fn the_window_closes_at_standard_turn_225() {
    let (mut game, _, _) = fixture(2_000.0);
    game.turn = game.standard_duration(BACKLOG_BUILDER_END_STANDARD);
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert!(ai.backlog_builder_shortfall(&game, 0, &plan).is_none());
}
