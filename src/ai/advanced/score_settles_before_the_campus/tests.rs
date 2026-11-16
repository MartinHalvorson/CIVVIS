use super::*;

const TAG: &str = "score-seat-settles-before-the-campus:";

#[test]
fn score_seat_settles_before_the_campus_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "score-seat-settles-before-the-campus",
        |ai| ai.score_seat_settles_before_the_campus,
    );
}

/// A founded capital and a second city four to six tiles away, both at
/// population 4, every unit removed, on Online turn 40 of a 200-turn game.
/// Each city's queue holds the empire's first Campus with 5 Production
/// paid, which the stock holdable-land claim never displaces while no Campus
/// stands (an unstarted district it does).
fn two_campus_cities() -> (Game, u32, u32) {
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
    for cid in [capital, second] {
        let campus = campus(&game, cid);
        head(&mut game, cid, campus);
        game.cities.get_mut(&cid).unwrap().production = 5.0;
    }
    (game, capital, second)
}

/// `no-city-limit` (the claim's machinery, armed live) on a `target` seat,
/// with or without the gene.
fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_no_city_limit();
    if gene {
        ai.enable_score_seat_settles_before_the_campus();
    }
    ai
}

fn campus(game: &Game, cid: u32) -> Item {
    Item::District {
        district: crate::name!("campus"),
        pos: game.cities[&cid].pos,
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn head(game: &mut Game, cid: u32, item: Item) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.queue.push(item);
    city.production = 0.0;
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

/// Run the holdable-land claim with a recording journal; return the
/// thoughts' details.
fn claim(ai: &mut AdvancedAi, game: &mut Game) -> Vec<String> {
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(game.turn, 0);
    ai.attach_journal(journal.handle());
    let plan = ai.assess(game, 0);
    ai.claim_holdable_land_settler(game, 0, &plan);
    journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect()
}

/// Off, an exact no-op: the first Campus keeps both queues and no line
/// carries the tag, on a Score seat as on any other.
#[test]
fn off_the_first_campus_keeps_its_queue() {
    let (game, capital, second) = two_campus_cities();
    let mut stock = seat(VictoryTarget::Score, false);
    let plan = stock.assess(&game, 0);
    assert!(
        stock.settlement_target(&plan) > 2,
        "fixture: the plan wants more cities than the two it holds"
    );
    let mut off = game.clone();
    let details = claim(&mut stock, &mut off);
    assert!(settlers_queued(&off).is_empty(), "stock keeps the Campus");
    for cid in [capital, second] {
        assert_eq!(off.cities[&cid].queue, game.cities[&cid].queue);
        assert_eq!(stock.score_settles_first_passes(&off, 0, cid), None);
    }
    assert!(!details.iter().any(|detail| detail.starts_with(TAG)));
    assert!(!stock.score_settles_first_open(&off, 0));
}

/// The blocked case: with the gene a Settler takes one of the two queues,
/// the Campus's 5 Production stays saved for it, and the line names the
/// Campus.
#[test]
fn a_campus_head_gives_way_to_the_settler_on_a_score_seat() {
    let (game, _, _) = two_campus_cities();
    let mut ai = seat(VictoryTarget::Score, true);
    assert!(ai.score_settles_first_open(&game, 0));
    let mut on = game.clone();
    let details = claim(&mut ai, &mut on);
    let queued = settlers_queued(&on);
    assert_eq!(queued.len(), 1, "one Settler: {details:?}");
    let cid = queued[0];
    let campus = campus(&on, cid);
    assert_eq!(
        on.item_invested_production(cid, &campus),
        5.0,
        "the Campus keeps its progress"
    );
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with(TAG) && detail.contains("Campus")),
        "{details:?}"
    );
}

/// The Library and the University give way too; a Granary, a Builder and
/// an empty queue are not the gene's.
#[test]
fn the_library_and_the_university_give_way_and_nothing_else_does() {
    let (mut game, capital, _) = two_campus_cities();
    let ai = seat(VictoryTarget::Score, true);
    for name in ["library", "university"] {
        let item = building(name);
        head(&mut game, capital, item.clone());
        assert_eq!(
            ai.score_settles_first_passes(&game, 0, capital),
            Some(item),
            "{name}"
        );
    }
    head(&mut game, capital, building("granary"));
    assert_eq!(ai.score_settles_first_passes(&game, 0, capital), None);
    head(
        &mut game,
        capital,
        Item::Unit {
            unit: crate::name!("builder"),
        },
    );
    assert_eq!(ai.score_settles_first_passes(&game, 0, capital), None);
    game.cities.get_mut(&capital).unwrap().queue.clear();
    assert_eq!(ai.score_settles_first_passes(&game, 0, capital), None);
}

/// A head within two turns of done keeps its queue: both Campuses all but
/// paid, no Settler.
#[test]
fn a_near_done_head_keeps_its_queue() {
    let (mut game, capital, second) = two_campus_cities();
    for cid in [capital, second] {
        let item = campus(&game, cid);
        let cost = game.item_cost_for_city(0, cid, &item);
        game.cities.get_mut(&cid).unwrap().production = cost - 0.5;
    }
    let mut ai = seat(VictoryTarget::Score, true);
    for cid in [capital, second] {
        assert_eq!(ai.score_settles_first_passes(&game, 0, cid), None);
    }
    let details = claim(&mut ai, &mut game);
    assert!(settlers_queued(&game).is_empty(), "{details:?}");
}

/// Other targets keep the stock queues with the gene on.
#[test]
fn other_seats_keep_their_queues() {
    let (game, capital, second) = two_campus_cities();
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Domination,
        VictoryTarget::Culture,
    ] {
        let mut ai = seat(target, true);
        assert!(!ai.score_settles_first_open(&game, 0), "{target:?}");
        let mut board = game.clone();
        claim(&mut ai, &mut board);
        assert!(settlers_queued(&board).is_empty(), "{target:?}");
        for cid in [capital, second] {
            assert_eq!(board.cities[&cid].queue, game.cities[&cid].queue);
        }
    }
}

/// The window: it closes at turn 120 Online (181 Standard) and at twelve
/// founded cities plus walking Settlers.
#[test]
fn the_window_closes_at_the_turn_and_the_city_target() {
    let (game, capital, _) = two_campus_cities();
    let ai = seat(VictoryTarget::Score, true);

    let mut speed = game.clone();
    speed.game_speed = crate::setup::GameSpeed::Online;
    assert_eq!(AdvancedAi::score_settles_first_close_turn(&speed), 120);
    speed.game_speed = crate::setup::GameSpeed::Standard;
    assert_eq!(AdvancedAi::score_settles_first_close_turn(&speed), 181);

    let mut late = game.clone();
    late.turn = AdvancedAi::score_settles_first_close_turn(&late);
    assert!(!ai.score_settles_first_open(&late, 0));
    late.turn -= 1;
    assert!(ai.score_settles_first_open(&late, 0));

    // Two cities and nine walking Settlers: eleven, still open; ten: closed.
    let mut wide = game.clone();
    let home = wide.cities[&capital].pos;
    for _ in 0..9 {
        wide.spawn_unit("settler", 0, home);
    }
    assert!(ai.score_settles_first_open(&wide, 0));
    wide.spawn_unit("settler", 0, home);
    assert!(!ai.score_settles_first_open(&wide, 0));
    assert_eq!(ai.score_settles_first_passes(&wide, 0, capital), None);
}
