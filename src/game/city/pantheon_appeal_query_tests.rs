use super::*;

fn owned_plain() -> (Game, Pos) {
    let mut game = Game::new_full(1, 24, 16, 91_986, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);
    let pos = *game.cities[&city]
        .owned_tiles
        .iter()
        .find(|pos| **pos != game.cities[&city].pos)
        .unwrap();
    let mut tile = Tile::new(pos);
    tile.terrain = crate::name!("plains");
    tile.owner_city = Some(city);
    *game.map.tiles.get_mut(&pos).unwrap() = tile;
    (game, pos)
}

fn set_earth_goddess(game: &mut Game, amount: f64) {
    game.players[0].pantheon = Some("earth_goddess".into());
    Arc::make_mut(&mut game.rules)
        .beliefs
        .pantheon
        .get_mut("earth_goddess")
        .unwrap()
        .effects
        .insert("breathtaking_appeal_faith".into(), amount);
}

#[test]
fn missing_or_unrelated_pantheons_do_not_request_appeal() {
    let (mut game, pos) = owned_plain();
    for pantheon in [None, Some("religious_settlements")] {
        game.players[0].pantheon = pantheon.map(str::to_owned);
        let _scope = game.query_memo();
        game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
        assert!(game.query_memo.appeal.borrow().as_ref().unwrap().is_empty());
    }
}

#[test]
fn dynamic_amounts_keep_the_breathtaking_threshold_and_exact_yields() {
    let (mut game, pos) = owned_plain();
    for appeal in [-5, 0, 3, 4, 7] {
        Arc::make_mut(&mut game.observed_appeal).insert(pos, appeal);
        game.players[0].pantheon = None;
        let base = game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
        for amount in [-2.5, -0.0, 0.0, 1.0, 3.25] {
            set_earth_goddess(&mut game, amount);
            let _scope = game.query_memo();
            let actual = game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
            let expected_faith = base.faith + if appeal >= 4 { amount } else { 0.0 };
            for (actual, expected) in [
                (actual.food, base.food),
                (actual.production, base.production),
                (actual.gold, base.gold),
                (actual.science, base.science),
                (actual.culture, base.culture),
                (actual.faith, expected_faith),
            ] {
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
        }
    }
}

#[test]
fn only_positive_zero_bypasses_the_appeal_branch() {
    let (mut game, pos) = owned_plain();
    Arc::make_mut(&mut game.observed_appeal).insert(pos, 4);
    for amount in [0.0, -0.0, 1.0] {
        set_earth_goddess(&mut game, amount);
        let _scope = game.query_memo();
        game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
        let queried = game
            .query_memo
            .appeal
            .borrow()
            .as_ref()
            .unwrap()
            .contains_key(&pos);
        assert_eq!(queried, amount.to_bits() != 0);
    }
}

#[test]
fn ordinary_improvements_skip_appeal_but_appeal_bonuses_still_query_it() {
    let (mut game, pos) = owned_plain();
    for improvement in ["farm", "mine", "alcazar"] {
        game.map.tiles.get_mut(&pos).unwrap().improvement = Some(Name::new(improvement));
        for pantheon in [None, Some("religious_settlements")] {
            game.players[0].pantheon = pantheon.map(str::to_owned);
            let _scope = game.query_memo();
            game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
            let queried = game
                .query_memo
                .appeal
                .borrow()
                .as_ref()
                .unwrap()
                .contains_key(&pos);
            assert_eq!(queried, improvement == "alcazar");
        }
    }
}
