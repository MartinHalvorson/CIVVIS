use super::*;
use std::sync::Arc;

/// A founded capital with Mining, two owned bare grassland hills no citizen
/// works and one owned grassland flat its one citizen works, as a live mirror
/// reports it. Returns the board, the capital and the tiles.
fn capital_with_idle_hills() -> (Game, u32, Pos, Pos, Pos) {
    let mut game = Game::new(2, 24, 16, 936015, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    game.players[0].techs.insert(crate::name!("mining"));
    let mut owned = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != home && game.wdist(home, *position) <= 3);
    let hill = owned.next().expect("the capital owns a plot");
    let second = owned.next().expect("the capital owns a second plot");
    let flat = owned.next().expect("the capital owns a third plot");
    for (pos, hills) in [(hill, true), (second, true), (flat, false)] {
        let tile = game.map.tiles.get_mut(&pos).expect("work plot");
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = hills;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    Arc::make_mut(&mut game.observed_city_worked_tiles).insert(capital, vec![flat]);
    for pos in [hill, second] {
        assert!(
            game.valid_improvements(0, pos)
                .contains(&crate::name!("mine")),
            "fixture: a Mine is legal on the hill"
        );
        let mut mined = game.workable_tile_yields(pos);
        mined.add(game.improvement_yield_change(0, pos, crate::name!("mine")));
        assert!(
            mined.production - game.workable_tile_yields(flat).production >= IDLE_PRODUCTION_MARGIN,
            "fixture: a mined hill beats the worked flat by the margin"
        );
    }
    (game, capital, hill, second, flat)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    ai.enable_builders_improve_the_worked_for_production();
    if gene {
        ai.enable_builders_improve_the_idle_for_production();
    }
    ai
}

/// The Builder's score for `improvement` on `pos`: the value every routing
/// site maximises.
fn builder_value(game: &Game, ai: &AdvancedAi, capital: u32, pos: Pos, improvement: &str) -> f64 {
    let shortfall = ai.city_production_foundation_shortfall(game, 0, capital);
    ai.production_foundation_improvement_value(
        game,
        0,
        pos,
        improvement,
        GrandStrategy::Conquest,
        shortfall,
    )
}

#[test]
fn builders_improve_the_idle_for_production_is_opt_in_and_off_is_stock() {
    assert!(!AdvancedAi::new().builders_improve_the_idle_for_production);
    let (game, capital, hill, _, flat) = capital_with_idle_hills();
    let stock = seat(false);
    assert_eq!(stock.idle_production_tile(&game, 0, capital), None);
    assert_eq!(stock.idle_production_jobs(&game, 0), 0);
    assert_eq!(
        stock.worked_backlog_with_idle(&game, 0),
        BasicAi::unimproved_worked_tiles(&game, 0)
    );
    let strategy = GrandStrategy::Conquest;
    assert_eq!(
        stock.worked_production_premium(&game, 0, hill, "mine", strategy),
        0.0,
        "off, an idle hill earns no worked premium"
    );
    let mut toggled = seat(true);
    toggled.disable_builders_improve_the_idle_for_production();
    for (pos, improvement) in [(hill, "mine"), (flat, "farm")] {
        assert_eq!(
            builder_value(&game, &toggled, capital, pos, improvement),
            builder_value(&game, &stock, capital, pos, improvement),
            "disabled, {improvement} is priced as stock"
        );
    }
}

/// The idle hill earns the worked premium, one hill a city at a time, and
/// outscores its stock price.
#[test]
fn the_idle_hill_is_priced_as_if_worked() {
    let (game, capital, hill, second, _) = capital_with_idle_hills();
    let ai = seat(true);
    let (pick, improvement) = ai
        .idle_production_tile(&game, 0, capital)
        .expect("an idle hill beats the worked flat");
    assert_eq!(improvement, "mine");
    // The map may hold other idle hills; the pick is one of them.
    assert!(!game.observed_city_worked_tiles[&capital].contains(&pick));
    assert!(game.map.tiles[&pick].improvement.is_none());
    let strategy = GrandStrategy::Conquest;
    let premium = ai.worked_production_premium(&game, 0, pick, "mine", strategy);
    assert!(premium > 0.0);
    for other in [hill, second].into_iter().filter(|pos| *pos != pick) {
        assert_eq!(
            ai.worked_production_premium(&game, 0, other, "mine", strategy),
            0.0,
            "one idle tile a city at a time"
        );
    }
    assert!(
        builder_value(&game, &ai, capital, pick, "mine")
            > builder_value(&game, &seat(false), capital, pick, "mine")
    );
}

/// The backlog gains one tile per city with a pick, and `BasicAi`'s step
/// reads it; off, it reads nothing.
#[test]
fn the_pick_counts_toward_the_backlog() {
    let (game, _, _, _, _) = capital_with_idle_hills();
    let mut ai = seat(true);
    assert_eq!(ai.idle_production_jobs(&game, 0), 1);
    assert_eq!(
        ai.worked_backlog_with_idle(&game, 0),
        BasicAi::unimproved_worked_tiles(&game, 0) + 1
    );
    ai.publish_idle_production_backlog(&game, 0);
    assert_eq!(ai.base.idle_production_backlog, 1);
    let mut stock = seat(false);
    stock.base.idle_production_backlog = 3;
    stock.publish_idle_production_backlog(&game, 0);
    assert_eq!(stock.base.idle_production_backlog, 0);
}

/// No pick in a struck city, on an improved or worked hill, or where the
/// mined hill does not beat the weakest worked tile by the margin.
#[test]
fn what_takes_no_pick() {
    let (game, capital, hill, second, flat) = capital_with_idle_hills();
    // A fresh seat per board: the pick is cached for the turn.
    let mut struck = game.clone();
    struck.cities.get_mut(&capital).unwrap().last_attacked = struck.turn.max(1);
    assert_eq!(seat(true).idle_production_tile(&struck, 0, capital), None);

    let mut improved = game.clone();
    for pos in [hill, second] {
        improved.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("mine"));
    }
    let ours =
        |pick: Option<(Pos, Name)>| pick.is_some_and(|(pos, _)| pos == hill || pos == second);
    assert!(
        !ours(seat(true).idle_production_tile(&improved, 0, capital)),
        "a Mine already stands on both hills"
    );

    let mut worked = game.clone();
    Arc::make_mut(&mut worked.observed_city_worked_tiles).insert(capital, vec![flat, hill]);
    if let Some((pick, _)) = seat(true).idle_production_tile(&worked, 0, capital) {
        assert_ne!(pick, hill, "a worked hill is the worked premium's");
    }

    // The citizen already works a tile as good as the mined hill.
    let mut rich = game.clone();
    let tile = rich.map.tiles.get_mut(&flat).unwrap();
    tile.hills = true;
    tile.improvement = Some(crate::name!("mine"));
    assert!(
        !ours(seat(true).idle_production_tile(&rich, 0, capital)),
        "neither grassland hill beats a worked grassland Mine by two Production"
    );
}
