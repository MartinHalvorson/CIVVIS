use super::*;

#[test]
fn score_seat_builds_the_bath_is_opt_in() {
    assert!(!AdvancedAi::new().score_seat_builds_the_bath);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    assert!(!ai.score_builds_the_bath());
    ai.enable_score_seat_builds_the_bath();
    assert!(ai.score_builds_the_bath());
    let mut other = AdvancedAi::targeting(VictoryTarget::Science);
    other.enable_score_seat_builds_the_bath();
    assert!(!other.score_builds_the_bath(), "only a Score seat");
}

/// A Roman city of three citizens on the first land tile whose Bath has a
/// legal site, every unit removed, Engineering known, the queue empty, at
/// turn 40. Returns the game, the city and its Bath.
fn roman_city() -> (Game, u32, Item) {
    for seed in [936_221_u64, 424_242, 777_001, 31_337, 12_345] {
        let mut base = Game::new_full(1, 32, 20, seed, 200, 0, false);
        for uid in base.player_unit_ids(0) {
            base.remove_unit(uid);
        }
        base.players[0].civ = "Rome".into();
        for tech in ["pottery", "the_wheel", "engineering", "masonry", "mining"] {
            base.players[0].techs.insert(Name::new(tech));
        }
        let mut sites: Vec<Pos> = base
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| !base.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        sites.sort();
        for site in sites {
            let mut game = base.clone();
            let ours = game.place_city(0, site, None);
            let bath = BasicAi::civ_district(&game, 0, "aqueduct");
            assert_eq!(bath, "bath", "fixture: Rome's Aqueduct is the Bath");
            let Some(pos) = game
                .district_sites(ours, bath)
                .into_iter()
                .filter(|pos| {
                    game.can_produce(
                        0,
                        ours,
                        &Item::District {
                            district: bath,
                            pos: *pos,
                        },
                    )
                })
                .min()
            else {
                continue;
            };
            let city = game.cities.get_mut(&ours).unwrap();
            city.pop = 3;
            city.queue.clear();
            game.turn = 40;
            return (
                game,
                ours,
                Item::District {
                    district: bath,
                    pos,
                },
            );
        }
    }
    panic!("no land tile with a legal Bath on five seeds");
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if gene {
        ai.enable_score_seat_builds_the_bath();
    }
    ai
}

fn head(game: &mut Game, cid: u32, item: Item) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.production = 0.0;
    city.queue.push(item);
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

/// An idle city takes its Bath; off, on another target, and for a
/// civilization with the stock Aqueduct, nothing moves.
#[test]
fn an_idle_city_takes_its_bath() {
    let (game, city, bath) = roman_city();

    let stock = seat(false);
    let plan = stock.assess(&game, 0);
    let mut off = game.clone();
    stock.claim_score_bath(&mut off, 0, &plan);
    assert!(off.cities[&city].queue.is_empty(), "off, nothing moves");
    assert!(!stock.score_bath_holds(&game, 0, city));

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_score_seat_builds_the_bath();
    let plan = science.assess(&game, 0);
    let mut elsewhere = game.clone();
    science.claim_score_bath(&mut elsewhere, 0, &plan);
    assert!(elsewhere.cities[&city].queue.is_empty(), "only a Score seat");

    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    let mut on = game.clone();
    ai.claim_score_bath(&mut on, 0, &plan);
    assert_eq!(on.cities[&city].queue.first(), Some(&bath));
    assert!(ai.score_bath_holds(&on, 0, city));
    assert!(
        ai.early_settler_floor_holds(&on, 0, city, &plan),
        "the routine claims' hold keeps the Bath"
    );
    assert!(!stock.early_settler_floor_holds(&on, 0, city, &plan));
    // A second pass changes nothing.
    let before = on.cities[&city].queue.clone();
    ai.claim_score_bath(&mut on, 0, &plan);
    assert_eq!(on.cities[&city].queue, before);

    // The stock Aqueduct is two points like any district: not this gene's.
    let mut plain = game.clone();
    plain.players[0].civ = "Greece".into();
    assert_eq!(AdvancedAi::score_bath_district(&plain, 0), None);
    let plan = ai.assess(&plain, 0);
    ai.claim_score_bath(&mut plain, 0, &plan);
    assert!(plain.cities[&city].queue.is_empty());
}

/// What gives way to the Bath and what keeps its queue.
#[test]
fn what_waits_for_the_bath() {
    let (game, city, bath) = roman_city();
    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    let takes = |item: Item, paid: f64| {
        let mut board = game.clone();
        head(&mut board, city, item.clone());
        let cost = board.item_cost_for_city(0, city, &item);
        board.cities.get_mut(&city).unwrap().production = paid * cost;
        ai.claim_score_bath(&mut board, 0, &plan);
        board.cities[&city].queue.first() == Some(&bath)
    };
    assert!(takes(building("granary"), 0.0), "a Granary waits");
    assert!(takes(building("monument"), 0.0), "a Monument waits");
    assert!(takes(unit("builder"), 0.0), "a Builder waits");
    assert!(takes(unit("settler"), 0.0), "a fresh Settler waits");
    assert!(
        !takes(unit("settler"), 0.5),
        "a Settler half paid finishes first"
    );
    assert!(!takes(unit("warrior"), 0.0), "a soldier keeps its queue");
    assert!(!takes(building("walls"), 0.0), "Walls keep their queue");
    assert!(
        !takes(building("granary"), 0.95),
        "what finishes this turn finishes"
    );
}

/// A city too small, a city under attack and a city that already holds its
/// Bath are passed over.
#[test]
fn which_city_is_passed_over() {
    let (game, city, bath) = roman_city();
    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    assert!(ai.score_bath_item(&game, 0, city, &plan).is_some());

    let mut small = game.clone();
    small.cities.get_mut(&city).unwrap().pop = SCORE_BATH_MIN_POP - 1;
    assert!(ai.score_bath_item(&small, 0, city, &plan).is_none());

    let mut struck = game.clone();
    struck.cities.get_mut(&city).unwrap().last_attacked = struck.turn - 1;
    assert!(ai.score_bath_item(&struck, 0, city, &plan).is_none());

    let mut built = game.clone();
    let Item::District { district, pos } = bath else {
        unreachable!()
    };
    built.map.tiles.get_mut(&pos).unwrap().district = Some(district);
    built
        .cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(district, pos);
    assert!(ai.score_bath_item(&built, 0, city, &plan).is_none());
}
