use super::*;
use crate::game::Action;

/// One major: our capital and a second city of ours, turn 30, every unit
/// removed, 50 Production banked on the capital's Monument so the
/// `capital-monument-after-the-second-city` claim takes it. The second city
/// is busy, so the only queue a claim can fill is the previewed capital's.
fn board() -> (Game, u32, Item) {
    let mut game = Game::new_full(1, 32, 20, 936_431, 200, 0, false);
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=8).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(0, *sites.first().expect("fixture: a second site"), None);
    let second = game
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != capital)
        .expect("fixture: two cities");
    game.cities
        .get_mut(&second)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: crate::name!("warrior"),
        });
    game.turn = 30;
    game.current = 0;
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let city = game.cities.get_mut(&capital).unwrap();
    city.queue.clear();
    city.production_progress
        .insert(Game::item_progress_key(&monument), 50.0);
    (game, capital, monument)
}

fn seat(hint_claims: bool, book_done: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_monument_after_the_second_city();
    if hint_claims {
        ai.enable_hint_runs_the_claims();
    }
    if book_done {
        ai.base.book_pos = 4;
    }
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 30,
        rush: false,
    });
    ai
}

#[test]
fn the_hint_carries_the_claim_the_main_path_would_make() {
    let (game, capital, monument) = board();
    let mut main = game.clone();
    let mut ai = seat(false, true);
    let plan = ai.plan.clone().unwrap();
    ai.run_idle_queue_claims(&mut main, 0, &plan);
    assert_eq!(
        main.cities[&capital].queue.first(),
        Some(&monument),
        "the main path's claim chain takes the capital's empty queue"
    );
    assert_eq!(
        seat(true, true).preview_live_production(&game, 0, capital),
        Some(monument.clone()),
        "with the gene on the hint asks the same chain first"
    );
    assert!(
        game.cities[&capital].queue.is_empty(),
        "the preview is disposable"
    );
}

#[test]
fn off_or_inside_the_book_the_hint_is_unchanged() {
    let (game, capital, _) = board();
    let shipped = seat(false, true).preview_live_production(&game, 0, capital);
    assert_eq!(
        seat(false, false).preview_live_production(&game, 0, capital),
        seat(true, false).preview_live_production(&game, 0, capital),
        "inside the opening book the gene leaves the hint alone"
    );
    let mut ai = seat(false, true);
    ai.disable_hint_runs_the_claims();
    assert_eq!(ai.preview_live_production(&game, 0, capital), shipped);
}
