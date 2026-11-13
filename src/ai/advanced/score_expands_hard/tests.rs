use super::super::early_settler_floor::EARLY_SETTLER_FLOOR_STANDARD_TURN;
use super::super::holdable_land_keeps_settling::HOLDABLE_LAND_PAYBACK_STANDARD_TURNS;
use super::*;

#[test]
fn score_seat_expands_hard_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("score-seat-expands-hard", |ai| {
        ai.score_seat_expands_hard
    });
}

/// A founded capital and a second city four to six tiles away, both idle at
/// population 4, every unit removed, on Online turn 40 of a 200-turn game.
fn two_idle_cities() -> (Game, u32, u32) {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let mut second = None;
    for site in sites {
        if game.wdist(site, home) < 4 || game.wdist(site, home) > 6 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            second = game
                .player_city_ids(0)
                .into_iter()
                .find(|cid| game.cities[cid].pos == site);
            break;
        }
        game.remove_unit(settler);
    }
    let second = second.expect("a legal second site");
    for cid in [capital, second] {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.queue.clear();
    }
    game.turn = 40;
    (game, capital, second)
}

/// `no-city-limit` on a `target` seat, with or without the gene.
fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_no_city_limit();
    if gene {
        ai.enable_score_seat_expands_hard();
    }
    ai
}

fn settlers_queued(game: &Game) -> Vec<u32> {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
        })
        .collect()
}

/// A soldier on each city, so the army has a body for every city.
fn garrison(game: &mut Game) {
    for cid in game.player_city_ids(0) {
        let pos = game.cities[&cid].pos;
        game.spawn_unit("warrior", 0, pos);
    }
}

fn head(game: &mut Game, cid: u32, item: Item) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.queue.push(item);
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

/// The gene needs `no-city-limit` under it and a Score target over it.
#[test]
fn it_acts_only_on_a_score_seat_that_has_no_city_limit() {
    assert!(seat(VictoryTarget::Score, true).score_expands_hard());
    assert!(!seat(VictoryTarget::Score, false).score_expands_hard());
    assert!(!seat(VictoryTarget::Science, true).score_expands_hard());
    assert!(!seat(VictoryTarget::Domination, true).score_expands_hard());
    let mut bare = AdvancedAi::targeting(VictoryTarget::Score);
    bare.enable_score_seat_expands_hard();
    assert!(!bare.score_expands_hard(), "no-city-limit is off");
}

/// One Settler more than the cities, to six; two while the army is thin.
#[test]
fn the_pipeline_is_one_wider_than_the_empire() {
    assert_eq!(AdvancedAi::score_expands_width_for(0, 0), 1);
    assert_eq!(AdvancedAi::score_expands_width_for(1, 0), 2);
    assert_eq!(AdvancedAi::score_expands_width_for(1, 1), 2);
    assert_eq!(AdvancedAi::score_expands_width_for(3, 3), 4);
    assert_eq!(AdvancedAi::score_expands_width_for(4, 3), 2, "a thin army");
    assert_eq!(AdvancedAi::score_expands_width_for(9, 12), 6, "the cap");
    let (mut game, _, _) = two_idle_cities();
    garrison(&mut game);
    assert_eq!(
        seat(VictoryTarget::Score, true).no_city_limit_width(&game, 0),
        Some(3)
    );
    assert_eq!(
        seat(VictoryTarget::Score, false).no_city_limit_width(&game, 0),
        Some(2),
        "off, the stock width"
    );
}

/// The blocked case: two garrisoned cities, each with an unstarted Builder
/// at the head of its queue. Stock, both keep the Builder. With the gene a
/// Settler takes each queue, and the routine claims' hold keeps it. A
/// soldier's queue is left alone.
#[test]
fn a_builders_queue_gives_way_and_a_soldiers_does_not() {
    let (mut game, capital, second) = two_idle_cities();
    garrison(&mut game);
    for cid in [capital, second] {
        head(&mut game, cid, unit("builder"));
    }
    let stock = seat(VictoryTarget::Score, false);
    let plan = stock.assess(&game, 0);
    let mut off = game.clone();
    stock.claim_early_settler_floor(&mut off, 0, &plan);
    stock.claim_early_settler_floor(&mut off, 0, &plan);
    assert!(settlers_queued(&off).is_empty(), "stock keeps the Builders");

    let ai = seat(VictoryTarget::Score, true);
    let plan = ai.assess(&game, 0);
    assert!(
        ai.settlement_target(&plan) >= 4,
        "fixture: two free seats that pay"
    );
    let mut on = game.clone();
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    assert_eq!(settlers_queued(&on).len(), 2, "a Settler in each city");
    for cid in settlers_queued(&on) {
        assert!(ai.early_settler_floor_holds(&on, 0, cid, &plan));
    }

    // Another target: the same board, nothing moves.
    let other = seat(VictoryTarget::Science, true);
    let plan = other.assess(&game, 0);
    let mut elsewhere = game.clone();
    other.claim_early_settler_floor(&mut elsewhere, 0, &plan);
    assert!(settlers_queued(&elsewhere).is_empty());

    // Soldiers keep their queues, with the gene too.
    let mut armed = game.clone();
    for cid in [capital, second] {
        head(&mut armed, cid, unit("archer"));
    }
    let plan = ai.assess(&armed, 0);
    ai.claim_early_settler_floor(&mut armed, 0, &plan);
    assert!(settlers_queued(&armed).is_empty(), "the Archers stay");
}

/// A soldier, Walls, a wonder and the Hall keep their queue; an unstarted
/// Builder gives way.
#[test]
fn what_keeps_its_queue() {
    let (mut game, capital, _) = two_idle_cities();
    head(&mut game, capital, unit("archer"));
    assert!(
        !AdvancedAi::score_expands_may_displace(&game, 0, capital),
        "a soldier keeps its queue"
    );
    head(&mut game, capital, unit("builder"));
    assert!(
        AdvancedAi::score_expands_may_displace(&game, 0, capital),
        "an unstarted Builder gives way"
    );
    for kept in ["walls", "ancestral_hall"] {
        head(
            &mut game,
            capital,
            Item::Building {
                building: Name::new(kept),
            },
        );
        assert!(
            !AdvancedAi::score_expands_may_displace(&game, 0, capital),
            "{kept} keeps its queue"
        );
    }
    head(
        &mut game,
        capital,
        Item::Building {
            building: Name::new("ancestral_hall"),
        },
    );
    assert!(AdvancedAi::score_expands_keeps_the_hall(&game, capital));
    game.cities.get_mut(&capital).unwrap().queue.clear();
    assert!(!AdvancedAi::score_expands_keeps_the_hall(&game, capital));
}

/// Past the band turn the stock floor is shut; the gene keeps it open while
/// a new city still has its payback turns, and no longer.
#[test]
fn the_floor_stays_open_past_the_band_turn() {
    let (mut game, _, _) = two_idle_cities();
    garrison(&mut game);
    game.turn = game.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN) + 10;
    // A limit far enough out that a city founded now still has its payback
    // turns, whatever speed the fixture plays.
    game.max_turns = game.turn + game.standard_duration(HOLDABLE_LAND_PAYBACK_STANDARD_TURNS) + 40;
    let stock = seat(VictoryTarget::Score, false);
    let plan = stock.assess(&game, 0);
    assert_eq!(stock.early_settler_floor(&game, &plan), 0, "stock: shut");
    let ai = seat(VictoryTarget::Score, true);
    let plan = ai.assess(&game, 0);
    assert!(ai.holdable_land_window(&game), "fixture: a city still pays");
    assert!(ai.early_settler_floor(&game, &plan) > 0, "the gene: open");
    game.turn = game.max_turns - 5;
    let plan = ai.assess(&game, 0);
    assert_eq!(
        ai.early_settler_floor(&game, &plan),
        0,
        "shut for the last turns"
    );
}

/// The capital queues its next Settler while the first still walks.
#[test]
fn the_capital_trains_back_to_back() {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    {
        let city = game.cities.get_mut(&capital).unwrap();
        city.pop = 4;
        city.queue.clear();
    }
    game.turn = 20;
    game.spawn_unit("settler", 0, home);
    let stock = seat(VictoryTarget::Score, false);
    let plan = stock.assess(&game, 0);
    let mut off = game.clone();
    stock.claim_early_settler_floor(&mut off, 0, &plan);
    assert!(
        settlers_queued(&off).is_empty(),
        "stock waits for the walker"
    );
    let ai = seat(VictoryTarget::Score, true);
    let plan = ai.assess(&game, 0);
    if ai.settlement_target(&plan) >= 3 {
        let mut on = game.clone();
        ai.claim_early_settler_floor(&mut on, 0, &plan);
        assert_eq!(settlers_queued(&on), vec![capital]);
    }
}

/// The Plaza's tier list under the gene: the Ancestral Hall, then the Grand
/// Master's Chapel; and the Hall takes the head of the Plaza city's queue.
#[test]
fn the_plaza_takes_the_ancestral_hall_first() {
    let (mut game, capital, _) = two_idle_cities();
    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    assert_eq!(BasicAi::plaza_hall_first_item(&game, 0, capital), None);
    let site = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &game.map.tiles[position];
            *position != game.cities[&capital].pos
                && tile.district.is_none()
                && !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
        })
        .unwrap();
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("government_plaza"));
    game.cities
        .get_mut(&capital)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), site);
    assert!(
        matches!(
            BasicAi::plaza_hall_first_item(&game, 0, capital),
            Some(Item::Building { building }) if building == "ancestral_hall"
        ),
        "the Hall first"
    );

    // The claim: the Hall goes ahead of a Granary; off, nothing moves.
    let granary = Item::Building {
        building: Name::new("granary"),
    };
    head(&mut game, capital, granary.clone());
    let stock = seat(VictoryTarget::Score, false);
    let plan = stock.assess(&game, 0);
    let mut off = game.clone();
    stock.claim_score_expansion_hall(&mut off, 0, &plan);
    assert_eq!(off.cities[&capital].queue.first(), Some(&granary));
    let ai = seat(VictoryTarget::Score, true);
    let plan = ai.assess(&game, 0);
    let mut on = game.clone();
    ai.claim_score_expansion_hall(&mut on, 0, &plan);
    assert!(AdvancedAi::score_expands_keeps_the_hall(&on, capital));
    assert!(
        ai.early_settler_floor_holds(&on, 0, capital, &plan),
        "the routine claims' hold keeps the Hall"
    );
    assert!(
        !stock.early_settler_floor_holds(&on, 0, capital, &plan),
        "off, the Hall is routine work"
    );

    // A soldier keeps its queue against the Hall as well.
    let mut guarded = game.clone();
    head(&mut guarded, capital, unit("archer"));
    let plan = ai.assess(&guarded, 0);
    ai.claim_score_expansion_hall(&mut guarded, 0, &plan);
    assert_eq!(
        guarded.cities[&capital].queue.first(),
        Some(&unit("archer"))
    );

    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("ancestral_hall"));
    assert!(
        matches!(
            BasicAi::plaza_hall_first_item(&game, 0, capital),
            Some(Item::Building { building }) if building == "grand_masters_chapel"
        ) || BasicAi::plaza_hall_first_item(&game, 0, capital).is_none(),
        "then the Chapel, once its government tier is reached"
    );
}
