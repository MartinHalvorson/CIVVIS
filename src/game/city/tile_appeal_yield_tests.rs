use super::*;

const EFFECTS: [&str; 4] = [
    "appeal_science_percent",
    "appeal_culture_percent",
    "appeal_gold",
    "appeal_faith",
];

fn unowned_mine() -> (Game, Pos) {
    let mut game = Game::new_full(1, 24, 16, 91_986, 120, 0, false);
    let pos = *game.map.tiles.keys().next().unwrap();
    let mut tile = Tile::new(pos);
    tile.terrain = crate::name!("plains");
    tile.improvement = Some(crate::name!("mine"));
    *game.map.tiles.get_mut(&pos).unwrap() = tile;
    set_coefficients(&mut game, [0.0; 4]);
    (game, pos)
}

fn set_coefficients(game: &mut Game, coefficients: [f64; 4]) {
    let effects = &mut Arc::make_mut(&mut game.rules)
        .improvements
        .get_mut("mine")
        .unwrap()
        .effects;
    for (effect, coefficient) in EFFECTS.into_iter().zip(coefficients) {
        effects.insert(effect.into(), coefficient);
    }
}

#[test]
fn zero_appeal_bonuses_do_not_request_appeal() {
    let (game, pos) = unowned_mine();
    let _scope = game.query_memo();
    game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
    assert!(game.query_memo.appeal.borrow().as_ref().unwrap().is_empty());
}

#[test]
fn custom_appeal_bonuses_preserve_each_yield_and_negative_appeal_floor() {
    let (mut game, pos) = unowned_mine();
    for appeal in [-4, 0, 3, 7] {
        game.observed_appeal.insert(pos, appeal);
        set_coefficients(&mut game, [0.0; 4]);
        let base = game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
        for coefficients in [
            [25.5, -50.25, 2.0, -1.5],
            [0.0, 0.0, 0.0, 3.25],
            [0.0, 0.1, 0.0, 0.0],
            [-0.0; 4],
        ] {
            set_coefficients(&mut game, coefficients);
            let scope = game.query_memo();
            let actual = game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
            let nonnegative = appeal.max(0) as f64;
            for (actual, expected) in [
                (actual.food, base.food),
                (actual.production, base.production),
                (
                    actual.science,
                    base.science + coefficients[0] / 100.0 * nonnegative,
                ),
                (
                    actual.culture,
                    base.culture + coefficients[1] / 100.0 * nonnegative,
                ),
                (actual.gold, base.gold + coefficients[2] * nonnegative),
                (actual.faith, base.faith + coefficients[3] * nonnegative),
            ] {
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
            drop(scope);
        }
    }
}

#[test]
fn pillaged_improvements_do_not_pay_appeal_bonuses() {
    let (mut game, pos) = unowned_mine();
    game.observed_appeal.insert(pos, 7);
    game.map.tiles.get_mut(&pos).unwrap().pillaged = true;
    let base = game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
    set_coefficients(&mut game, [50.0, 75.0, 1.0, 2.0]);
    let actual = game.player_tile_yields(0, pos, &game.map.tiles[&pos]);
    assert_eq!(actual, base);
}
