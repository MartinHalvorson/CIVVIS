use super::*;
use crate::game::Action;

/// Two majors (player 1 unsettled): our capital and a second city of ours,
/// turn 30, every unit
/// removed, 50 Production banked on the capital's Monument so the
/// `capital-monument-after-the-second-city` claim takes it. The second city
/// is busy, so the only queue a claim can fill is the previewed capital's.
fn board() -> (Game, u32, Item) {
    let mut game = Game::new_full(2, 32, 20, 936_431, 200, 0, false);
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

#[test]
fn a_city_in_the_wars_reach_keeps_the_governors_hint() {
    let (mut game, capital, _) = board();
    game.at_war.insert((0, 1));
    let home = game.cities[&capital].pos;
    let near = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && game.city_at(tile.pos).is_none()
                && game.wdist(home, tile.pos) == 2
        })
        .map(|tile| tile.pos)
        .min()
        .expect("fixture: a land tile two out");
    game.spawn_test_unit("warrior", 1, near);
    let on = seat(true, true);
    assert!(on.hint_city_in_reach_of_war(&game, 0, capital));
    let hint = on.preview_live_production(&game, 0, capital);
    assert_eq!(
        hint,
        seat(false, true).preview_live_production(&game, 0, capital),
        "a city with a hostile in reach keeps the governor's own hint"
    );
}

/// The capital has a competing Monument claim, three founded cities, and
/// a named but unassembled first-capital force. Other cities are busy.
fn first_capital_board() -> (Game, u32, Item, AdvancedAi) {
    use super::super::early_conquest::ConquestOpening;
    use std::collections::BTreeSet;

    let (mut game, capital, monument) = board();
    let site = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(**pos, city.pos) >= 4)
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: third city site");
    let third = game.found_city_for(0, site, None);
    game.cities.get_mut(&third).unwrap().queue.push(Item::Unit {
        unit: crate::name!("warrior"),
    });
    let rival_settler = game
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let rival = game.found_city_for(1, game.units[&rival_settler].pos, None);
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].gold = 100.0;
    game.players[0].gold_per_turn = 20.0;
    let mut ai = seat(true, true);
    ai.enable_early_conquest_opening();
    ai.enable_first_capital_before_the_walls();
    ai.victory_target = Some(crate::ai::VictoryTarget::Domination);
    ai.conquest_opening = Some(ConquestOpening {
        target: 1,
        city: rival,
        opened: game.turn,
        preparing_since: Some(game.turn),
        grace_until: None,
        rally: game.cities[&capital].pos,
        force: BTreeSet::new(),
        assembled: None,
        declared: None,
        kills_at_war: 0,
        losses: 0,
        taken: 0,
    });
    (game, capital, monument, ai)
}

#[test]
fn the_first_capital_claim_precedes_the_monument_and_reaches_the_hint() {
    let (game, capital, monument, mut ai) = first_capital_board();
    let plan = ai.plan.clone().unwrap();
    let mut off = ai.clone();
    off.disable_first_capital_before_the_walls();
    let mut untreated = game.clone();
    off.run_idle_queue_claims(&mut untreated, 0, &plan);
    assert_eq!(untreated.cities[&capital].queue.first(), Some(&monument));

    let mut claimed = game.clone();
    ai.run_idle_queue_claims(&mut claimed, 0, &plan);
    let chosen = claimed.cities[&capital].queue.first().unwrap();
    let Item::Unit { unit } = chosen else {
        panic!("the incomplete first-capital force must claim the capital, got {chosen:?}");
    };
    let spec = &game.rules.units[unit];
    assert!(AdvancedAi::conquest_ranged_body(spec) || AdvancedAi::conquest_melee_body(spec));
    assert_eq!(
        ai.preview_live_production(&game, 0, capital),
        Some(chosen.clone()),
        "a consumed hint must not restore the competing infrastructure claim"
    );
    let reserved = chosen.clone();
    ai.run_idle_queue_claims(&mut claimed, 0, &plan);
    assert_eq!(
        claimed.cities[&capital].queue.first(),
        Some(&reserved),
        "the Monument must not retake the assault unit on a later poll"
    );
    assert!(game.cities[&capital].queue.is_empty());
    for cid in game.player_city_ids(0) {
        if cid != capital {
            assert_eq!(claimed.cities[&cid].queue, game.cities[&cid].queue);
        }
    }
}

#[test]
fn the_first_capital_claim_keeps_existing_builds_and_closes_with_the_reservation() {
    let (game, capital, monument, ai) = first_capital_board();
    let plan = ai.plan.clone().unwrap();
    let mut busy = game.clone();
    busy.cities
        .get_mut(&capital)
        .unwrap()
        .queue
        .push(monument.clone());
    ai.clone().run_idle_queue_claims(&mut busy, 0, &plan);
    assert_eq!(busy.cities[&capital].queue, vec![monument.clone()]);

    for closed in [
        "unnamed",
        "declared",
        "expired",
        "other_lane",
        "before_expansion",
    ] {
        let mut seat = ai.clone();
        let mut board = game.clone();
        match closed {
            "unnamed" => seat.conquest_opening = None,
            "declared" => seat.conquest_opening.as_mut().unwrap().declared = Some(board.turn),
            "expired" => board.turn = 500,
            "other_lane" => seat.victory_target = Some(crate::ai::VictoryTarget::Culture),
            "before_expansion" => {
                let other = board
                    .player_city_ids(0)
                    .into_iter()
                    .find(|cid| *cid != capital)
                    .unwrap();
                board.cities.get_mut(&other).unwrap().owner = 1;
            }
            _ => unreachable!(),
        }
        seat.run_idle_queue_claims(&mut board, 0, &plan);
        assert_eq!(
            board.cities[&capital].queue.first(),
            Some(&monument),
            "{closed}"
        );
    }
}

#[test]
fn a_bound_capitals_granary_cannot_retake_the_last_reserved_body() {
    let (mut game, capital, _, mut ai) = first_capital_board();
    ai.disable_capital_monument_after_the_second_city();
    ai.enable_granary_claims_the_bound_queue();
    game.players[0].techs.insert(crate::name!("pottery"));
    let housing = game.city_housing(&game.cities[&capital]);
    game.cities.get_mut(&capital).unwrap().pop = (housing.ceil() as i32).max(2);
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    game.cities
        .get_mut(&capital)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&granary), 50.0);
    let plan = ai.plan.clone().unwrap();
    assert!(ai.bound_granary_item(&game, 0, capital, &plan).is_some());
    // An idle city may stand aside for a due Settler. Give the Granary
    // the actual competing military build it is allowed to interrupt.
    game.cities
        .get_mut(&capital)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: crate::name!("warrior"),
        });
    let mut off = ai.clone();
    off.disable_first_capital_before_the_walls();
    let mut untreated = game.clone();
    off.run_idle_queue_claims(&mut untreated, 0, &plan);
    assert_eq!(untreated.cities[&capital].queue.first(), Some(&granary));

    ai.run_idle_queue_claims(&mut game, 0, &plan);
    let reserved = game.cities[&capital].queue.first().unwrap().clone();
    assert!(matches!(reserved, Item::Unit { .. }));
    // All remaining demand is met, including queued replacements. The
    // already reserved last body must still finish before the Granary.
    let sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| !game.rules.is_water(tile) && game.city_at(**pos).is_none())
        .map(|(pos, _)| *pos)
        .take(16)
        .collect();
    for (index, pos) in sites.into_iter().enumerate() {
        game.spawn_test_unit(if index < 8 { "warrior" } else { "archer" }, 0, pos);
    }
    assert_eq!(
        ai.first_capital_shortfall(&game, 0, &ai.counts(&game, 0)),
        (0, 0)
    );
    ai.run_idle_queue_claims(&mut game, 0, &plan);
    assert_eq!(game.cities[&capital].queue.first(), Some(&reserved));
}
