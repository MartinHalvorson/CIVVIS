use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

#[test]
fn culture_seat_fills_its_district_slots_is_opt_in() {
    assert!(!AdvancedAi::new().culture_seat_fills_its_district_slots);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(!ai.culture_fills_its_slots());
    ai.enable_culture_seat_fills_its_district_slots();
    assert!(ai.culture_fills_its_slots());
    let mut other = AdvancedAi::targeting(VictoryTarget::Score);
    other.enable_culture_seat_fills_its_district_slots();
    assert!(!other.culture_fills_its_slots(), "only a Culture seat");
}

/// Two Theater cities of seven citizens with empty queues at Online turn 100,
/// every unit removed, the technologies and civics of the claim's districts
/// and of the Holy Site known, an Amphitheater in the first city (which opens
/// `culture-faith-site-per-theater-3`'s Faith sources). Returns the board, a
/// plan that names no threat, and a city that can place a Campus and a Holy
/// Site.
fn open_city() -> (Game, StrategicPlan, u32) {
    let mut g = Game::new_full(
        1,
        64,
        40,
        crate::rng::fixture_seed("CULTURESLOTS", 1),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    while g.player_city_ids(0).len() < 2 {
        let pos = g
            .map
            .tiles
            .values()
            .filter(|tile| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && g.cities
                        .values()
                        .all(|city| g.wdist(city.pos, tile.pos) >= 5)
            })
            .map(|tile| tile.pos)
            .min()
            .expect("room for the fixture cities");
        g.found_city_for(0, pos, None);
    }
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    g.game_speed = GameSpeed::Online;
    g.turn = 100;
    for tech in [
        "astrology",
        "writing",
        "currency",
        "celestial_navigation",
        "mining",
        "bronze_working",
        "pottery",
    ] {
        g.players[0].techs.insert(Name::new(tech));
    }
    for civic in ["drama_poetry", "mysticism"] {
        g.players[0].civics.insert(Name::new(civic));
    }
    let cities = g.player_city_ids(0);
    for &cid in &cities {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 7;
        city.queue.clear();
        crate::game::install_test_district(&mut g, cid, "theater_square");
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: 20.0,
                ..Default::default()
            },
        );
    }
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    let budget = g.standard_duration(CULTURE_SLOT_MAX_STANDARD) as f64;
    let city = cities
        .iter()
        .copied()
        .find(|cid| {
            BasicAi::first_district_item_with_budget(&g, 0, *cid, "campus", budget).is_some()
                && BasicAi::first_district_item(&g, 0, *cid, "holy_site").is_some()
        })
        .expect("fixture: a city can place a Campus and a Holy Site");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan, city)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if gene {
        ai.enable_culture_seat_fills_its_district_slots();
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

fn head_family(game: &Game, cid: u32) -> Option<String> {
    match game.cities[&cid].queue.first() {
        Some(Item::District { district, .. }) => {
            Some(game.district_family(*district).as_str().to_owned())
        }
        _ => None,
    }
}

/// An idle Theater city with a free slot takes a Campus first, a Commercial
/// Hub without Writing; off, on another target, before the opening turn, in
/// a small city and in a city without its Theater Square nothing moves.
#[test]
fn an_idle_theater_city_fills_its_free_slot() {
    let (game, plan, city) = open_city();
    let ai = seat(true);
    let (offer, cost) = ai
        .culture_slot_item(&game, 0, city, &plan)
        .expect("fixture: the city can place a district");
    assert!(cost > 0.0);
    let mut first = game.clone();
    head(&mut first, city, offer.clone());
    assert_eq!(head_family(&first, city).as_deref(), Some("campus"));

    let mut off = game.clone();
    seat(false).claim_culture_slots(&mut off, 0, &plan);
    assert!(off.cities[&city].queue.is_empty(), "off, nothing moves");

    let mut score = AdvancedAi::targeting(VictoryTarget::Score);
    score.enable_culture_seat_fills_its_district_slots();
    let mut elsewhere = game.clone();
    score.claim_culture_slots(&mut elsewhere, 0, &plan);
    assert!(
        elsewhere.cities[&city].queue.is_empty(),
        "only a Culture seat"
    );

    let mut early = game.clone();
    early.turn = early.standard_duration(CULTURE_SLOT_START_STANDARD) - 1;
    ai.claim_culture_slots(&mut early, 0, &plan);
    assert!(early.cities[&city].queue.is_empty(), "not before its turn");

    let mut small = game.clone();
    small.cities.get_mut(&city).unwrap().pop = CULTURE_SLOT_MIN_POP - 1;
    ai.claim_culture_slots(&mut small, 0, &plan);
    assert!(small.cities[&city].queue.is_empty(), "not a small city");

    let mut bare = game.clone();
    let theater = bare
        .cities
        .get_mut(&city)
        .unwrap()
        .districts
        .remove(crate::name!("theater_square"))
        .expect("fixture: the Theater Square");
    for pos in theater {
        bare.map.tiles.get_mut(&pos).unwrap().district = None;
    }
    assert!(
        ai.culture_slot_item(&bare, 0, city, &plan).is_none(),
        "only a city that holds its Theater Square"
    );

    let mut on = game.clone();
    ai.claim_culture_slots(&mut on, 0, &plan);
    assert_eq!(on.cities[&city].queue.first(), Some(&offer));
    // A second pass queues no second district while one is in the queue.
    let before = on.cities[&city].queue.clone();
    ai.claim_culture_slots(&mut on, 0, &plan);
    assert_eq!(on.cities[&city].queue, before);

    let mut no_writing = game.clone();
    no_writing.players[0].techs.remove(&crate::name!("writing"));
    ai.claim_culture_slots(&mut no_writing, 0, &plan);
    assert_eq!(
        head_family(&no_writing, city).as_deref(),
        Some("commercial_hub"),
        "the Commercial Hub when no Campus can go"
    );
}

/// A Neighborhood, a Spy or a plain building gives way when at most a quarter
/// paid and at least as dear as the district; a Theater Square building,
/// Walls and every other unit keep their queue.
#[test]
fn what_waits_for_the_district() {
    let (mut game, _, city) = open_city();
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
    assert!(AdvancedAi::culture_slot_may_defer(
        &game,
        0,
        city,
        &university,
        between
    ));
    game.cities.get_mut(&city).unwrap().production = 0.5 * dear;
    assert!(
        !AdvancedAi::culture_slot_may_defer(&game, 0, city, &university, between),
        "a building half paid finishes"
    );
    head(&mut game, city, granary.clone());
    assert!(
        !AdvancedAi::culture_slot_may_defer(&game, 0, city, &granary, between),
        "a cheaper building keeps its queue"
    );

    let pos = game.cities[&city].pos;
    for (item, waits) in [
        (
            Item::District {
                district: crate::name!("neighborhood"),
                pos,
            },
            true,
        ),
        (
            Item::Unit {
                unit: crate::name!("spy"),
            },
            true,
        ),
        (
            Item::District {
                district: crate::name!("holy_site"),
                pos,
            },
            false,
        ),
        (building("amphitheater"), false),
        (building("art_museum"), false),
        (building("broadcast_center"), false),
        (building("walls"), false),
        (
            Item::Unit {
                unit: crate::name!("settler"),
            },
            false,
        ),
        (
            Item::Unit {
                unit: crate::name!("builder"),
            },
            false,
        ),
    ] {
        head(&mut game, city, item.clone());
        assert_eq!(
            AdvancedAi::culture_slot_may_defer(&game, 0, city, &item, 1.0),
            waits,
            "{item:?}"
        );
    }
}

/// The claim takes a University's queue, leaves an Amphitheater's, and
/// passes over a struck city and a city that already queues a specialty
/// district.
#[test]
fn the_claim_reads_the_head_and_the_city() {
    let (game, plan, city) = open_city();
    let ai = seat(true);
    let (_, cost) = ai.culture_slot_item(&game, 0, city, &plan).unwrap();

    let university = building("university");
    let mut taken = game.clone();
    head(&mut taken, city, university.clone());
    let displaced = game.item_cost_for_city(0, city, &university) >= cost;
    ai.claim_culture_slots(&mut taken, 0, &plan);
    assert_eq!(
        head_family(&taken, city).is_some(),
        displaced,
        "a University gives way exactly when it costs at least the district"
    );

    let amphitheater = building("amphitheater");
    let mut kept = game.clone();
    head(&mut kept, city, amphitheater.clone());
    ai.claim_culture_slots(&mut kept, 0, &plan);
    assert_eq!(kept.cities[&city].queue.first(), Some(&amphitheater));

    let mut struck = game.clone();
    struck.cities.get_mut(&city).unwrap().last_attacked = struck.turn - 1;
    assert!(ai.culture_slot_item(&struck, 0, city, &plan).is_none());

    let mut threatened = plan.clone();
    threatened.threatened_city = Some(city);
    assert!(ai.culture_slot_item(&game, 0, city, &threatened).is_none());

    let site = BasicAi::first_district_item(&game, 0, city, "holy_site").unwrap();
    let mut queued = game.clone();
    let behind = queued.cities.get_mut(&city).unwrap();
    behind.queue.push(building("granary"));
    behind.queue.push(site);
    assert!(
        ai.culture_slot_item(&queued, 0, city, &plan).is_none(),
        "one specialty district in the queue at a time"
    );
}

/// With `culture-faith-site-per-theater-3` on, a Theater city's last free
/// slot stays the Holy Site's; once the Holy Site stands, or with that gene
/// off, the claim takes it.
#[test]
fn the_holy_site_keeps_its_slot() {
    let (mut game, plan, city) = open_city();
    // Four citizens: two slots, the Theater Square in one.
    game.cities.get_mut(&city).unwrap().pop = 4;
    let mut ai = seat(true);
    assert!(
        ai.culture_slot_item(&game, 0, city, &plan).is_some(),
        "with the faith gene off the free slot is the claim's"
    );
    ai.enable_culture_seat_theater_first();
    ai.base.pursue_religion = false;
    ai.base.skip_prophet_race = true;
    ai.enable_culture_faith_site_per_theater_3();
    assert!(
        ai.culture_faith_slot_is_kept(&game, 0, city),
        "fixture: the faith gene keeps the slot"
    );
    assert!(ai.culture_slot_item(&game, 0, city, &plan).is_none());
    let mut idle = game.clone();
    ai.claim_culture_slots(&mut idle, 0, &plan);
    assert!(idle.cities[&city].queue.is_empty());

    // The Holy Site stands; seven citizens give a third slot.
    crate::game::install_test_district(&mut game, city, "holy_site");
    game.cities.get_mut(&city).unwrap().pop = 7;
    assert!(!ai.culture_faith_slot_is_kept(&game, 0, city));
    ai.claim_culture_slots(&mut game, 0, &plan);
    assert_eq!(head_family(&game, city).as_deref(), Some("campus"));
}
