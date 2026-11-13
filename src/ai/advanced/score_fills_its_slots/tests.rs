use super::*;

#[test]
fn score_seat_fills_its_district_slots_is_opt_in() {
    assert!(!AdvancedAi::new().score_seat_fills_its_district_slots);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    assert!(!ai.score_fills_its_slots());
    ai.enable_score_seat_fills_its_district_slots();
    assert!(ai.score_fills_its_slots());
    let mut other = AdvancedAi::targeting(VictoryTarget::Science);
    other.enable_score_seat_fills_its_district_slots();
    assert!(!other.score_fills_its_slots(), "only a Score seat");
}

/// One city of seven citizens with an empty queue, every unit removed, the
/// technologies and civics of the claim's districts known, on the claim's
/// opening turn.
fn open_city() -> (Game, u32) {
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
    for tech in [
        "pottery",
        "writing",
        "bronze_working",
        "currency",
        "apprenticeship",
        "celestial_navigation",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for civic in ["drama_and_poetry", "games_and_recreation"] {
        game.players[0].civics.insert(Name::new(civic));
    }
    let city = game.player_city_ids(0)[0];
    let home = game.cities.get_mut(&city).unwrap();
    home.pop = 7;
    home.queue.clear();
    game.turn = game.standard_duration(SCORE_SLOT_START_STANDARD);
    (game, city)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if gene {
        ai.enable_score_seat_fills_its_district_slots();
    }
    ai
}

fn head(game: &mut Game, cid: u32, item: Item) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.production = 0.0;
    city.queue.push(item);
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn heads_a_slot_district(game: &Game, cid: u32) -> bool {
    matches!(
        game.cities[&cid].queue.first(),
        Some(Item::District { district, .. })
            if SCORE_SLOT_FAMILIES.contains(&game.district_family(*district).as_str())
    )
}

/// An idle city with a free slot takes the cheapest district it can place;
/// off, on another target, before the opening turn and in a small city
/// nothing moves.
#[test]
fn an_idle_city_fills_its_free_slot() {
    let (game, city) = open_city();
    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    let (offer, cost) = ai
        .score_slot_item(&game, 0, city, &plan)
        .expect("fixture: the city can place a district");
    assert!(cost > 0.0);

    let stock = seat(false);
    let mut off = game.clone();
    stock.claim_score_slots(&mut off, 0, &plan);
    assert!(off.cities[&city].queue.is_empty(), "off, nothing moves");

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_score_seat_fills_its_district_slots();
    let mut elsewhere = game.clone();
    science.claim_score_slots(&mut elsewhere, 0, &plan);
    assert!(
        elsewhere.cities[&city].queue.is_empty(),
        "only a Score seat"
    );

    let mut early = game.clone();
    early.turn -= 1;
    ai.claim_score_slots(&mut early, 0, &plan);
    assert!(early.cities[&city].queue.is_empty(), "not before its turn");

    let mut small = game.clone();
    small.cities.get_mut(&city).unwrap().pop = SCORE_SLOT_MIN_POP - 1;
    ai.claim_score_slots(&mut small, 0, &plan);
    assert!(small.cities[&city].queue.is_empty(), "not a small city");

    let mut on = game.clone();
    ai.claim_score_slots(&mut on, 0, &plan);
    assert_eq!(on.cities[&city].queue.first(), Some(&offer));
    assert!(heads_a_slot_district(&on, city));
    // No offer is cheaper by more than the slack.
    for family in SCORE_SLOT_FAMILIES {
        if let Some(item) =
            BasicAi::first_district_item_with_budget(&game, 0, city, family, f64::INFINITY)
        {
            assert!(cost <= SCORE_SLOT_COST_SLACK * game.item_cost_for_city(0, city, &item));
        }
    }
    // A second pass queues no second district while one is in the queue.
    let before = on.cities[&city].queue.clone();
    ai.claim_score_slots(&mut on, 0, &plan);
    assert_eq!(on.cities[&city].queue, before);
}

/// A building gives way only when it is not Walls or a wonder, at most a
/// quarter paid, and costs at least as much as the district.
#[test]
fn what_waits_for_the_district() {
    let (mut game, city) = open_city();
    let university = building("university");
    let granary = building("granary");
    let dear = game.item_cost_for_city(0, city, &university);
    let cheap = game.item_cost_for_city(0, city, &granary);
    assert!(
        cheap < dear,
        "fixture: a Granary costs less than a University"
    );
    let between = 0.5 * (cheap + dear);

    head(&mut game, city, university.clone());
    assert!(AdvancedAi::score_slot_may_defer(
        &game,
        0,
        city,
        &university,
        between
    ));
    game.cities.get_mut(&city).unwrap().production = 0.5 * dear;
    assert!(
        !AdvancedAi::score_slot_may_defer(&game, 0, city, &university, between),
        "a building half paid finishes"
    );

    head(&mut game, city, granary.clone());
    assert!(
        !AdvancedAi::score_slot_may_defer(&game, 0, city, &granary, between),
        "a cheaper building keeps its queue"
    );
    let walls = building("walls");
    head(&mut game, city, walls.clone());
    assert!(!AdvancedAi::score_slot_may_defer(
        &game, 0, city, &walls, 1.0
    ));
    for unit in ["settler", "builder", "warrior"] {
        let item = Item::Unit {
            unit: Name::new(unit),
        };
        head(&mut game, city, item.clone());
        assert!(
            !AdvancedAi::score_slot_may_defer(&game, 0, city, &item, 1.0),
            "{unit} keeps its queue"
        );
    }
}

/// The claim takes a University's queue and leaves a struck city and a city
/// that already queues a district alone.
#[test]
fn the_claim_reads_the_head_and_the_city() {
    let (game, city) = open_city();
    let ai = seat(true);
    let plan = ai.assess(&game, 0);
    let (offer, cost) = ai.score_slot_item(&game, 0, city, &plan).unwrap();

    let university = building("university");
    let mut taken = game.clone();
    head(&mut taken, city, university.clone());
    let displaced = game.item_cost_for_city(0, city, &university) >= cost;
    ai.claim_score_slots(&mut taken, 0, &plan);
    assert_eq!(
        heads_a_slot_district(&taken, city),
        displaced,
        "a University gives way exactly when it costs at least the district"
    );

    let mut struck = game.clone();
    struck.cities.get_mut(&city).unwrap().last_attacked = struck.turn - 1;
    assert!(ai.score_slot_item(&struck, 0, city, &plan).is_none());

    let mut queued = game.clone();
    let behind = queued.cities.get_mut(&city).unwrap();
    behind.queue.push(building("granary"));
    behind.queue.push(offer);
    assert!(
        ai.score_slot_item(&queued, 0, city, &plan).is_none(),
        "one district in the queue at a time"
    );
}
