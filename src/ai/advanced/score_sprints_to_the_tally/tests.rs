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

// ---- `score-seat-sprints-to-the-tally-2` ----

fn seat_two() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    ai.enable_score_seat_sprints_to_the_tally_2();
    ai
}

/// The Great Person classes the fixture offers whose patronage applies in
/// both currencies with a deep bank.
fn patronizable(game: &Game) -> Vec<String> {
    let classes: std::collections::BTreeSet<String> = game
        .rules
        .great_people
        .values()
        .map(|person| person.kind.clone())
        .collect();
    classes
        .into_iter()
        .filter(|kind| {
            ["gold", "faith"].iter().all(|currency| {
                let mut g = game.clone();
                g.players[0].gold = 1_000_000.0;
                g.players[0].faith = 1_000_000.0;
                g.players[0].gpp.insert(kind.clone(), 0.0);
                g.apply(
                    0,
                    &Action::PatronizeGreatPerson {
                        kind: kind.clone(),
                        currency: currency.to_string(),
                    },
                )
                .is_ok()
            })
        })
        .collect()
}

/// Offers one patronizable class only, with none of its points earned.
fn one_great_person(game: &mut Game) -> String {
    let kind = patronizable(game)
        .into_iter()
        .next()
        .expect("fixture: a Great Person the seat can patronize");
    game.players[0].live_great_person_offers = Some([kind.clone()].into_iter().collect());
    game.players[0].gpp.insert(kind.clone(), 0.0);
    kind
}

/// Great People of `kind` recruited: a live-offer board counts a native
/// recruitment in `gp_claimed` (`claim_great_person`).
fn claimed(game: &Game, kind: &str) -> i64 {
    game.players[0].gp_claimed.get(kind).copied().unwrap_or(0)
}

fn great_person_buy<'a>(buys: &'a [SprintBuy], kind: &str) -> &'a SprintBuy {
    buys.iter()
        .find(|buy| {
            matches!(&buy.action, Action::PatronizeGreatPerson { kind: offered, .. }
                if offered == kind)
        })
        .expect("the class is on the menu")
}

#[test]
fn version_two_is_opt_in_and_turns_version_one_off() {
    assert!(!AdvancedAi::new().score_seat_sprints_to_the_tally_2);
    let mut ai = seat(true);
    ai.enable_score_seat_sprints_to_the_tally_2();
    assert!(!ai.score_seat_sprints_to_the_tally, "one version plays");
    assert!(ai.score_sprint_on());
    assert_eq!(ai.score_sprint_tag(), "score-seat-sprints-to-the-tally-2");
    ai.disable_score_seat_sprints_to_the_tally_2();
    assert!(!ai.score_sprint_on());
    assert_eq!(
        seat(true).score_sprint_tag(),
        "score-seat-sprints-to-the-tally"
    );
    let mut other = AdvancedAi::targeting(VictoryTarget::Science);
    other.enable_score_seat_sprints_to_the_tally_2();
    assert!(!other.score_sprint_on(), "only a Score seat");
}

/// Version 1 is unchanged: its menu carries no Era Score, it names no
/// currency for a class, and its Great People open with the rest of it.
#[test]
fn version_one_is_unchanged() {
    let (mut game, _) = late_city();
    let kind = one_great_person(&mut game);
    game.players[0].gold = 100_000.0;
    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    let menu = ai.score_sprint_menu(&game, 0, "gold", true, &BTreeMap::new());
    let stock = AdvancedAi::score_sprint_buys(&game, 0, "gold");
    assert_eq!(menu.len(), stock.len());
    for (a, b) in menu.iter().zip(&stock) {
        assert_eq!((a.points, a.era, a.price), (b.points, 0.0, b.price));
    }
    assert_eq!(
        great_person_buy(&menu, &kind).points,
        TALLY_SPRINT_GREAT_PERSON_POINTS
    );
    assert!(ai
        .score_sprint_patrons(&game, 0, &plan, left(&game), true)
        .is_empty());
    for turn in [LIMIT - 30, LIMIT - 21, LIMIT - 20, LIMIT] {
        game.turn = turn;
        assert_eq!(
            ai.score_sprint_great_person_left(&game),
            ai.score_sprint_left(&game)
        );
    }
    assert_eq!(ai.score_sprint_era_note(3.0), "");
}

/// Off, on another target and before its window, version 2 moves no coin.
#[test]
fn version_two_off_its_window_is_an_exact_no_op() {
    let (mut game, city) = late_city();
    one_great_person(&mut game);
    game.players[0].gold = 5_000.0;
    game.players[0].faith = 5_000.0;
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_score_seat_sprints_to_the_tally_2();
    let early = {
        let mut early = game.clone();
        early.turn = LIMIT - 31;
        early
    };
    for (ai, board) in [(science, &game), (seat_two(), &early)] {
        let mut g = board.clone();
        let plan = ai.assess(&g, 0);
        ai.score_sprint_purchases(&mut g, 0, &plan);
        assert_eq!(g.cities[&city].buildings, board.cities[&city].buildings);
        assert_eq!(g.players[0].gp_claimed, board.players[0].gp_claimed);
        assert_eq!(g.players[0].gold, 5_000.0);
        assert_eq!(g.players[0].faith, 5_000.0);
    }
}

/// A Great Person whose points the Gold pays more than half of is five
/// points and the patronage moment's three; one already over half earned
/// pays the recruitment moment's one.
#[test]
fn a_great_person_is_priced_with_its_moment() {
    let (mut game, _) = late_city();
    let kind = one_great_person(&mut game);
    let ai = seat_two();
    let none = BTreeMap::new();
    let over_half =
        great_person_buy(&ai.score_sprint_menu(&game, 0, "gold", true, &none), &kind).clone();
    assert_eq!((over_half.points, over_half.era), (5.0 + 3.0, 3.0));
    let faith =
        great_person_buy(&ai.score_sprint_menu(&game, 0, "faith", true, &none), &kind).clone();
    assert_eq!((faith.points, faith.era), (5.0 + 3.0, 3.0));
    let cost = game.gp_cost(0, &kind);
    game.players[0].gpp.insert(kind.clone(), 0.75 * cost);
    let earned =
        great_person_buy(&ai.score_sprint_menu(&game, 0, "gold", true, &none), &kind).clone();
    assert_eq!((earned.points, earned.era), (5.0 + 1.0, 1.0));
    assert_eq!(ai.score_sprint_era_note(3.0), " (3.0 of them Era Score)");
}

/// Gold that cannot reach the price leaves the class to Faith, which buys
/// it; with both banks deep the idle Faith still takes it, and Gold alone
/// takes it when Faith cannot reach it.
#[test]
fn faith_patronizes_when_gold_is_short() {
    let (mut game, _) = late_city();
    let kind = one_great_person(&mut game);
    let price = game
        .great_person_patronage_price(0, &kind, "faith")
        .expect("a Faith price");
    game.players[0].faith = price + TALLY_SPRINT_FAITH_RESERVE + 1.0;
    game.players[0].gold = 100.0;
    let ai = seat_two();
    let plan = ai.assess(&game, 0);
    let rest = left(&game);
    let patrons = ai.score_sprint_patrons(&game, 0, &plan, rest, true);
    assert_eq!(patrons.get(&kind), Some(&"faith"));
    let mut bought = game.clone();
    ai.score_sprint_purchases(&mut bought, 0, &plan);
    assert_eq!(claimed(&bought, &kind), claimed(&game, &kind) + 1);
    assert!((bought.players[0].faith - (TALLY_SPRINT_FAITH_RESERVE + 1.0)).abs() < 1e-6);
    assert_eq!(bought.players[0].gold, 100.0);
    let mut deep = game.clone();
    deep.players[0].gold = 100_000.0;
    let patrons = ai.score_sprint_patrons(&deep, 0, &plan, rest, true);
    assert_eq!(
        patrons.get(&kind),
        Some(&"faith"),
        "Gold has buildings to buy"
    );
    let gold_menu = ai.score_sprint_menu(&deep, 0, "gold", true, &patrons);
    assert!(gold_menu
        .iter()
        .all(|buy| !matches!(buy.action, Action::PatronizeGreatPerson { .. })));
    deep.players[0].faith = 0.0;
    let patrons = ai.score_sprint_patrons(&deep, 0, &plan, rest, true);
    assert_eq!(patrons.get(&kind), Some(&"gold"));
}

/// A wonder of the world's era is fifteen points and its moment's four,
/// both times the chance to finish first, as in version 1.
#[test]
fn a_wonder_is_priced_fifteen_plus_four() {
    let (mut game, city) = late_city();
    let techs: Vec<Name> = game
        .rules
        .techs
        .iter()
        .filter(|(_, node)| node.era <= 1)
        .map(|(name, _)| *name)
        .collect();
    let civics: Vec<Name> = game
        .rules
        .civics
        .iter()
        .filter(|(_, node)| node.era <= 1)
        .map(|(name, _)| *name)
        .collect();
    game.players[0].techs.extend(techs);
    game.players[0].civics.extend(civics);
    let world = game.world_era;
    let wonder = game
        .producible_items(0, city)
        .into_iter()
        .find(|item| {
            matches!(item, Item::Wonder { wonder, .. }
                if AdvancedAi::score_wonder_era(&game, wonder) >= world)
        })
        .expect("fixture: a wonder of the world's era on the menu");
    let cost = game.item_cost_for_city(0, city, &wonder);
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&wonder), (cost - 10.0).max(0.0));
    let rest = left(&game);
    let offer = seat_two()
        .score_sprint_offer(&game, 0, city, &wonder, rest)
        .expect("a wonder that finishes in time scores");
    let Item::Wonder { wonder: name, .. } = &wonder else {
        unreachable!()
    };
    let era = AdvancedAi::score_wonder_era(&game, name);
    let chance = AdvancedAi::score_wonder_chance(era, world, offer.turns);
    assert!((offer.points - (15.0 + 4.0) * chance).abs() < 1e-9);
    assert!((offer.era - 4.0 * chance).abs() < 1e-9);
    assert_eq!(
        seat(true).score_sprint_offer(&game, 0, city, &wonder, rest),
        Some(offer),
        "version 1 prices it the same"
    );
}

/// The Great People open thirty Online turns before the limit, while the
/// queues and the other purchases keep version 1's twenty: in between the
/// bank patronizes and buys no building.
#[test]
fn the_great_person_window_opens_at_thirty() {
    let (mut game, city) = late_city();
    assert_eq!(
        game.standard_duration(TALLY_SPRINT_GREAT_PERSON_STANDARD_TURNS),
        30
    );
    let ai = seat_two();
    game.turn = LIMIT - 30;
    assert_eq!(ai.score_sprint_great_person_left(&game), Some(31));
    assert_eq!(
        ai.score_sprint_left(&game),
        None,
        "the queues keep version 1's window"
    );
    assert_eq!(seat(true).score_sprint_great_person_left(&game), None);
    game.turn = LIMIT - 31;
    assert_eq!(ai.score_sprint_great_person_left(&game), None);
    game.turn = LIMIT - 25;
    let kind = one_great_person(&mut game);
    let (_, building_price) = cheapest_gold_building(&mut game);
    let price = game
        .great_person_patronage_price(0, &kind, "gold")
        .expect("a Gold price");
    game.players[0].gold = price + building_price + TALLY_SPRINT_GOLD_RESERVE + 1.0;
    let before = game.cities[&city].buildings.clone();
    let people = claimed(&game, &kind);
    let plan = ai.assess(&game, 0);
    ai.score_sprint_purchases(&mut game, 0, &plan);
    assert_eq!(claimed(&game, &kind), people + 1);
    assert_eq!(
        game.cities[&city].buildings, before,
        "no building before turn 160"
    );
}
