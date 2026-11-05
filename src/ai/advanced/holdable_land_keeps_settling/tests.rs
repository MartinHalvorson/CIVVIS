use super::*;

/// A founded capital and a second city four to six tiles away, both idle at
/// population 4, every unit removed, so no Settler walks and none is queued.
/// The turn is the first one past the opening floor's window.
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
    game.turn = game.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN);
    (game, capital, second)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_holdable_land_keeps_settling();
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

/// The blocked case: past the opening floor, two idle cities short of the
/// plan's target queue exactly one Settler, and the claims' shared hold keeps
/// it; off, nothing moves.
#[test]
fn past_the_opening_a_short_empire_queues_one_holdable_settler() {
    let (game, _capital, _second) = two_idle_cities();
    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_holdable_land_settler(&mut off, 0, &plan);
    assert!(settlers_queued(&off).is_empty(), "off: no claim");

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(
        ai.settlement_target(&plan) > 2,
        "fixture: the plan wants more cities than the two it holds"
    );
    ai.claim_holdable_land_settler(&mut on, 0, &plan);
    let queued = settlers_queued(&on);
    assert_eq!(queued.len(), 1, "one Settler at a time");
    assert!(
        ai.early_settler_floor_holds(&on, 0, queued[0], &plan),
        "the routine claims' hold keeps the floor's Settler"
    );
    assert!(
        !stock.early_settler_floor_holds(&on, 0, queued[0], &plan),
        "off, the hold reads exactly the opening floor"
    );
    ai.claim_holdable_land_settler(&mut on, 0, &plan);
    assert_eq!(
        settlers_queued(&on).len(),
        1,
        "no second Settler while one is in production"
    );
}

/// The bounds: the opening floor's window, the payback horizon, two walkers,
/// a city at population two, a threatened city, Recovery and a war with a
/// major each leave every queue alone.
#[test]
fn the_floor_waits_for_its_window_its_walkers_and_quiet() {
    let ai = armed();
    let (game, capital, second) = two_idle_cities();

    let mut early = game.clone();
    early.turn = 20;
    let plan = ai.assess(&early, 0);
    ai.claim_holdable_land_settler(&mut early, 0, &plan);
    assert!(
        settlers_queued(&early).is_empty(),
        "the opening floor's window"
    );

    let mut late = game.clone();
    late.turn = late.max_turns - late.standard_duration(HOLDABLE_LAND_PAYBACK_STANDARD_TURNS);
    let plan = ai.assess(&late, 0);
    ai.claim_holdable_land_settler(&mut late, 0, &plan);
    assert!(settlers_queued(&late).is_empty(), "too late to repay");

    let mut walking = game.clone();
    let home = walking.cities[&capital].pos;
    walking.spawn_unit("settler", 0, home);
    walking.spawn_unit("settler", 0, home);
    let plan = ai.assess(&walking, 0);
    ai.claim_holdable_land_settler(&mut walking, 0, &plan);
    assert!(
        settlers_queued(&walking).is_empty(),
        "two walkers already out"
    );

    let mut small = game.clone();
    for cid in [capital, second] {
        small.cities.get_mut(&cid).unwrap().pop = HOLDABLE_LAND_MIN_POP - 1;
    }
    let plan = ai.assess(&small, 0);
    ai.claim_holdable_land_settler(&mut small, 0, &plan);
    assert!(settlers_queued(&small).is_empty(), "cities too small");

    let mut threatened = game.clone();
    let mut plan = ai.assess(&threatened, 0);
    plan.threatened_city = Some(second);
    ai.claim_holdable_land_settler(&mut threatened, 0, &plan);
    assert!(
        settlers_queued(&threatened).is_empty(),
        "a threatened city answers first"
    );

    let mut recovering = game.clone();
    let mut plan = ai.assess(&recovering, 0);
    plan.strategy = GrandStrategy::Recovery;
    ai.claim_holdable_land_settler(&mut recovering, 0, &plan);
    assert!(
        settlers_queued(&recovering).is_empty(),
        "Recovery keeps its queues"
    );

    let mut attacked = game.clone();
    for cid in [capital, second] {
        attacked.cities.get_mut(&cid).unwrap().last_attacked = attacked.turn;
    }
    let plan = ai.assess(&attacked, 0);
    ai.claim_holdable_land_settler(&mut attacked, 0, &plan);
    assert!(
        settlers_queued(&attacked).is_empty(),
        "attacked cities keep their queues"
    );
}

/// A tile exactly `distance` from `from`, if the board has one.
fn tile_at(game: &Game, from: Pos, distance: i32) -> Pos {
    game.map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(*pos, from) == distance)
        .expect("a tile at that distance")
}

/// The holdability filters, each at its edge: a charted rival city inside
/// nine tiles, a stronger major's soldier within six, a raider within three.
#[test]
fn rival_cities_strong_soldiers_and_raiders_make_a_seat_unholdable() {
    let (game, capital, _second) = two_idle_cities();
    let site = game.cities[&capital].pos;
    let clear = HoldableLandThreats::default();
    assert_eq!(AdvancedAi::holdable_land_refusal(&game, &clear, site), None);

    let at = |d| vec![tile_at(&game, site, d)];
    let rival = |d| HoldableLandThreats {
        rival_cities: at(d),
        ..HoldableLandThreats::default()
    };
    assert!(
        AdvancedAi::holdable_land_refusal(&game, &rival(HOLDABLE_LAND_RIVAL_CITY_TILES - 1), site)
            .is_some(),
        "a rival city eight tiles out"
    );
    assert_eq!(
        AdvancedAi::holdable_land_refusal(&game, &rival(HOLDABLE_LAND_RIVAL_CITY_TILES), site),
        None,
        "a rival city nine tiles out"
    );

    let strong = |d| HoldableLandThreats {
        strong_soldiers: at(d),
        ..HoldableLandThreats::default()
    };
    assert!(AdvancedAi::holdable_land_refusal(
        &game,
        &strong(HOLDABLE_LAND_STRONG_SOLDIER_TILES),
        site
    )
    .is_some());
    assert_eq!(
        AdvancedAi::holdable_land_refusal(
            &game,
            &strong(HOLDABLE_LAND_STRONG_SOLDIER_TILES + 1),
            site
        ),
        None
    );

    let raider = |d| HoldableLandThreats {
        raiders: at(d),
        ..HoldableLandThreats::default()
    };
    assert!(
        AdvancedAi::holdable_land_refusal(&game, &raider(HOLDABLE_LAND_RAIDER_TILES), site)
            .is_some()
    );
    assert_eq!(
        AdvancedAi::holdable_land_refusal(&game, &raider(HOLDABLE_LAND_RAIDER_TILES + 1), site),
        None
    );
}

/// The seat search reads the filters: with every tile marked as a rival city
/// no seat is holdable; with none, the site gate's own seat comes back.
#[test]
fn the_seat_search_reads_the_holdability_filters() {
    let ai = armed();
    let (game, capital, _second) = two_idle_cities();
    let from = game.cities[&capital].pos;
    let clear = HoldableLandThreats::default();
    assert!(
        ai.holdable_land_seat(&game, 0, from, 0, &clear).is_some(),
        "fixture: an acceptable seat in reach"
    );
    let everywhere = HoldableLandThreats {
        rival_cities: game.map.tiles.keys().copied().collect(),
        ..HoldableLandThreats::default()
    };
    assert_eq!(ai.holdable_land_seat(&game, 0, from, 0, &everywhere), None);
}
