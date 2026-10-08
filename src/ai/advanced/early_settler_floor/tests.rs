use super::*;

/// A founded capital and a second city four or more tiles away, both idle at
/// population 4, every unit removed, so no Settler walks and none is queued.
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
    game.turn = 20;
    (game, capital, second)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_early_settler_floor();
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

/// The blocked case: two idle cities short of the floor before the band
/// turn queue exactly one Settler, in the city that trains it soonest; off,
/// nothing moves.
#[test]
fn a_short_empire_queues_one_settler_before_the_band_turn() {
    let (game, _capital, _second) = two_idle_cities();
    assert!(
        game.turn < game.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN),
        "fixture inside the window"
    );
    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_early_settler_floor(&mut off, 0, &plan);
    assert!(settlers_queued(&off).is_empty(), "off: no claim");

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(
        ai.base.has_practical_settle_site(&on, 0),
        "fixture has a site"
    );
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    let queued = settlers_queued(&on);
    assert_eq!(queued.len(), 1, "one Settler at a time");
    assert!(
        ai.early_settler_floor_holds(&on, 0, queued[0], &plan),
        "the floor keeps the Settler it queued"
    );
    // A second pass with the Settler already queued adds nothing.
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    assert_eq!(
        settlers_queued(&on).len(),
        1,
        "no second Settler while one is in production"
    );
}

/// The bounds: past the band turn, with two walkers already out, or with
/// every city recently attacked or building its Walls, nothing is claimed.
#[test]
fn the_floor_yields_to_the_band_turn_walkers_and_local_defence() {
    let ai = armed();
    let (game, capital, second) = two_idle_cities();

    let mut late = game.clone();
    late.turn = late.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN);
    let plan = ai.assess(&late, 0);
    ai.claim_early_settler_floor(&mut late, 0, &plan);
    assert!(settlers_queued(&late).is_empty(), "past the window");

    let mut walking = game.clone();
    let home = walking.cities[&capital].pos;
    walking.spawn_unit("settler", 0, home);
    walking.spawn_unit("settler", 0, home);
    let plan = ai.assess(&walking, 0);
    ai.claim_early_settler_floor(&mut walking, 0, &plan);
    assert!(
        settlers_queued(&walking).is_empty(),
        "two walkers already out"
    );

    let mut attacked = game.clone();
    for cid in [capital, second] {
        attacked.cities.get_mut(&cid).unwrap().last_attacked = attacked.turn;
    }
    let plan = ai.assess(&attacked, 0);
    ai.claim_early_settler_floor(&mut attacked, 0, &plan);
    assert!(
        settlers_queued(&attacked).is_empty(),
        "attacked cities keep their queues"
    );

    let mut walled = game.clone();
    for cid in [capital, second] {
        walled.cities.get_mut(&cid).unwrap().queue = vec![Item::Building {
            building: crate::name!("walls"),
        }];
    }
    let plan = ai.assess(&walled, 0);
    ai.claim_early_settler_floor(&mut walled, 0, &plan);
    assert!(
        settlers_queued(&walled).is_empty(),
        "Walls keep their queue"
    );
}

/// The positive control for displacement: a routine building at the head of
/// both queues gives way to the Settler and keeps its progress behind it.
#[test]
fn a_routine_building_gives_way_and_keeps_its_progress() {
    let ai = armed();
    let (mut game, capital, second) = two_idle_cities();
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    for cid in [capital, second] {
        let city = game.cities.get_mut(&cid).unwrap();
        city.queue = vec![granary.clone()];
        city.production = 7.0;
    }
    let plan = ai.assess(&game, 0);
    ai.claim_early_settler_floor(&mut game, 0, &plan);
    let queued = settlers_queued(&game);
    assert_eq!(queued.len(), 1, "one routine queue gives way");
    assert!(
        game.item_invested_production(queued[0], &granary) >= 7.0 - 1e-9,
        "the displaced Granary keeps its progress"
    );
}

/// The conquest opening is the operator's call: while its reservation is
/// open the capital keeps its queue, and the floor's Settler goes to the
/// other city instead.
#[test]
fn the_open_conquest_reservation_keeps_the_capital() {
    let (game, capital, second) = two_idle_cities();
    let mut ai = armed();
    ai.enable_early_conquest_opening();
    ai.conquest_opening = Some(crate::ai::advanced::early_conquest::ConquestOpening {
        target: 0,
        city: capital,
        opened: game.turn,
        preparing_since: Some(game.turn),
        grace_until: Some(game.turn + 100),
        rally: game.cities[&capital].pos,
        force: std::collections::BTreeSet::new(),
        assembled: None,
        declared: None,
        kills_at_war: 0,
        losses: 0,
        taken: 0,
    });
    assert!(
        ai.conquest_reservation_open(&game),
        "fixture: the reservation is open"
    );
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    assert_eq!(
        settlers_queued(&on),
        vec![second],
        "only the non-capital city takes the Settler"
    );
}
