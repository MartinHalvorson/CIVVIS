use super::*;

/// The fixture's turn limit.
const LIMIT: u32 = 180;

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if gene {
        ai.enable_score_seat_sprints_to_the_tally();
    }
    ai
}

/// One city of seven citizens with an empty queue, every unit removed, a few
/// early technologies and civics known, twenty turns before the limit.
fn late_city() -> (Game, u32) {
    let mut game = Game::new_full(1, 32, 20, 936_221, LIMIT, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    for tech in [
        "pottery",
        "writing",
        "bronze_working",
        "masonry",
        "currency",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for civic in ["code_of_laws", "craftsmanship", "drama_and_poetry"] {
        game.players[0].civics.insert(Name::new(civic));
    }
    let city = game.player_city_ids(0)[0];
    let home = game.cities.get_mut(&city).unwrap();
    home.pop = 7;
    home.queue.clear();
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = 0.0;
    game.players[0].faith = 0.0;
    game.turn = LIMIT - 20;
    (game, city)
}

fn head(game: &mut Game, cid: u32, item: Item) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.production = 0.0;
    city.queue.push(item);
}

fn project() -> Item {
    Item::Project {
        project: Name::new("bread_and_circuses"),
    }
}

fn left(game: &Game) -> u32 {
    LIMIT - game.turn + 1
}

#[test]
fn score_seat_sprints_to_the_tally_is_opt_in() {
    assert!(!AdvancedAi::new().score_seat_sprints_to_the_tally);
    let (game, _) = late_city();
    let mut ai = seat(false);
    assert!(!ai.score_sprint_on());
    assert_eq!(ai.score_sprint_left(&game), None);
    ai.enable_score_seat_sprints_to_the_tally();
    assert_eq!(ai.score_sprint_left(&game), Some(21));
    let mut other = AdvancedAi::targeting(VictoryTarget::Science);
    other.enable_score_seat_sprints_to_the_tally();
    assert!(!other.score_sprint_on(), "only a Score seat");
}

/// The window opens `TALLY_SPRINT_STANDARD_TURNS` standard turns before the
/// limit and closes after it; a game without a limit never opens it.
#[test]
fn the_window_runs_from_its_opening_turn_to_the_limit() {
    let (mut game, _) = late_city();
    let ai = seat(true);
    let window = game.standard_duration(TALLY_SPRINT_STANDARD_TURNS);
    game.turn = LIMIT - window;
    assert_eq!(ai.score_sprint_left(&game), Some(window + 1));
    game.turn = LIMIT - window - 1;
    assert_eq!(ai.score_sprint_left(&game), None);
    game.turn = LIMIT;
    assert_eq!(ai.score_sprint_left(&game), Some(1));
    game.turn = LIMIT + 1;
    assert_eq!(ai.score_sprint_left(&game), None);
    game.max_turns = 0;
    game.turn = 170;
    assert_eq!(ai.score_sprint_left(&game), None);
}

/// Off, on another target and before the window, neither pass moves a queue
/// or a coin.
#[test]
fn off_the_gene_is_an_exact_no_op() {
    let (mut game, city) = late_city();
    head(&mut game, city, project());
    game.players[0].gold = 5_000.0;
    game.players[0].faith = 5_000.0;
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_score_seat_sprints_to_the_tally();
    let early = {
        let mut early = game.clone();
        early.turn = 100;
        early
    };
    for (ai, board) in [(seat(false), &game), (science, &game), (seat(true), &early)] {
        let mut g = board.clone();
        let plan = ai.assess(&g, 0);
        ai.claim_score_sprint_queues(&mut g, 0, &plan);
        ai.score_sprint_purchases(&mut g, 0, &plan);
        assert_eq!(g.cities[&city].queue, vec![project()]);
        assert_eq!(g.cities[&city].buildings, board.cities[&city].buildings);
        assert_eq!(g.players[0].gold, 5_000.0);
        assert_eq!(g.players[0].faith, 5_000.0);
    }
}

/// A project scores nothing: the claim replaces it with the menu's best
/// points per Production, at least every building's rate.
#[test]
fn a_project_head_gives_way_to_the_best_offer() {
    let (mut game, city) = late_city();
    head(&mut game, city, project());
    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    let rest = left(&game);
    let best = ai
        .score_sprint_best(&game, 0, city, rest)
        .expect("fixture: something on the menu finishes in twenty turns");
    for item in game.producible_items(0, city) {
        if let Item::Building { building } = &item {
            if game.rules.buildings[building].wonder {
                continue;
            }
            if let Some(offer) = ai.score_sprint_offer(&game, 0, city, &item, rest) {
                assert!(
                    best.rate() >= offer.rate(),
                    "{item:?} pays more than {best:?}"
                );
            }
        }
    }
    assert!(ai
        .score_sprint_offer(&game, 0, city, &project(), rest)
        .is_none());
    ai.claim_score_sprint_queues(&mut game, 0, &plan);
    assert_eq!(game.cities[&city].queue.first(), Some(&best.item));
}

/// A head that scores keeps its place against an offer within the slack.
#[test]
fn a_scoring_head_keeps_its_place() {
    let (mut game, city) = late_city();
    let ai = seat(true);
    let rest = left(&game);
    let best = ai
        .score_sprint_best(&game, 0, city, rest)
        .expect("fixture: something on the menu finishes in twenty turns");
    head(&mut game, city, best.item.clone());
    let plan = ai.assess(&game, 0);
    ai.claim_score_sprint_queues(&mut game, 0, &plan);
    assert_eq!(game.cities[&city].queue, vec![best.item]);
}

/// Nothing starts that cannot finish before the tally: on the last turn an
/// item that needs more than one turn is no offer, and a city whose menu
/// holds nothing that finishes keeps its queue.
#[test]
fn nothing_starts_that_cannot_finish() {
    let (mut game, city) = late_city();
    let ai = seat(true);
    let monument = Item::Building {
        building: Name::new("monument"),
    };
    assert!(ai
        .score_sprint_offer(&game, 0, city, &monument, 21)
        .is_some());
    let turns = game.item_remaining_cost_for_city(0, city, &monument)
        / game.city_yields(city).production.max(0.5);
    assert!(turns > 1.0, "fixture: a Monument takes more than a turn");
    game.turn = LIMIT;
    assert!(ai
        .score_sprint_offer(&game, 0, city, &monument, 1)
        .is_none());
    head(&mut game, city, project());
    let plan = ai.assess(&game, 0);
    let before = game.cities[&city].queue.clone();
    ai.claim_score_sprint_queues(&mut game, 0, &plan);
    let after = &game.cities[&city].queue;
    if after != &before {
        let started = after.first().expect("a replaced head is an item");
        assert!(
            ai.score_sprint_offer(&game, 0, city, started, 1).is_some(),
            "{started:?} cannot finish on the last turn"
        );
    }
}

/// The cheapest building the city sells for Gold and its price, read with a
/// bank deep enough for the whole menu; asserts no Great Person costs as
/// little.
fn cheapest_gold_building(game: &mut Game) -> (Name, f64) {
    game.players[0].gold = 100_000.0;
    let buys = AdvancedAi::score_sprint_buys(game, 0, "gold");
    game.players[0].gold = 0.0;
    let (cheapest, price) = buys
        .iter()
        .filter_map(|buy| match &buy.action {
            Action::BuyBuilding { building, .. } => Some((*building, buy.price)),
            _ => None,
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("fixture: the city sells a building for Gold");
    assert!(
        buys.iter()
            .filter(|buy| matches!(buy.action, Action::PatronizeGreatPerson { .. }))
            .all(|buy| buy.price > price + 1.0),
        "fixture: no Great Person costs as little as the cheapest building"
    );
    (cheapest, price)
}

/// The Gold above the reserve buys the building with the fewest Gold a
/// point, and nothing it cannot pay for.
#[test]
fn gold_buys_the_fewest_gold_a_point() {
    let (mut game, city) = late_city();
    let ai = seat(true);
    let (cheapest, price) = cheapest_gold_building(&mut game);
    assert!(game.cities[&city].buildings.iter().all(|b| *b != cheapest));
    // Enough for the cheapest only.
    let bank = TALLY_SPRINT_GOLD_RESERVE + price + 1.0;
    game.players[0].gold = bank;
    let plan = ai.assess(&game, 0);
    ai.score_sprint_purchases(&mut game, 0, &plan);
    assert!(
        game.cities[&city].buildings.contains(&cheapest),
        "the cheapest building a point was bought"
    );
    assert!((game.players[0].gold - (bank - price)).abs() < 1e-6);
    assert!(game.players[0].gold + 1e-6 >= TALLY_SPRINT_GOLD_RESERVE);
}

/// Below the reserve nothing is bought.
#[test]
fn the_reserve_stays_in_the_bank() {
    let (mut game, city) = late_city();
    let ai = seat(true);
    let (_, price) = cheapest_gold_building(&mut game);
    // One Gold short of the cheapest building over the reserve.
    let bank = TALLY_SPRINT_GOLD_RESERVE + price - 1.0;
    game.players[0].gold = bank;
    let before = game.cities[&city].buildings.clone();
    let plan = ai.assess(&game, 0);
    ai.score_sprint_purchases(&mut game, 0, &plan);
    assert_eq!(game.cities[&city].buildings, before);
    assert_eq!(game.players[0].gold, bank);
}
