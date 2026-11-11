use super::*;

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

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_no_city_limit();
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

/// A soldier beside each city, so the army has a body for every city.
fn garrison(game: &mut Game) {
    for cid in game.player_city_ids(0) {
        let pos = game.cities[&cid].pos;
        game.spawn_unit("warrior", 0, pos);
    }
}

/// The pay test has no city count in it: the bar is the Settler's price
/// against the turns left. Online figures (1.5 standard turns a turn) for a
/// game expected to end at turn 195.
#[test]
fn a_city_pays_by_its_settlers_price_and_the_turns_left() {
    let floor = |turn: f64, cost: f64| AdvancedAi::settler_pay_floor(195.0 - turn, 1.5, cost);
    assert_eq!(floor(50.0, 100.0), 0.0, "the opening: any seat pays");
    assert!(
        floor(100.0, 190.0) < 40.0,
        "the eleventh Settler at turn 100 pays on any seat the gate accepts"
    );
    let eleventh = floor(115.0, 190.0);
    assert!(
        (40.0..120.0).contains(&eleventh),
        "at turn 115 the eleventh Settler needs a good seat, {eleventh:.1}"
    );
    assert!(
        floor(120.0, 190.0).is_infinite(),
        "from turn 120 no seat repays the eleventh Settler"
    );
    let sixteenth = floor(95.0, 265.0);
    assert!(
        (40.0..120.0).contains(&sixteenth),
        "at turn 95 the sixteenth Settler needs a good seat, {sixteenth:.1}"
    );
    assert!(
        floor(110.0, 265.0).is_infinite(),
        "from turn 110 no seat repays the sixteenth Settler"
    );
    assert!(
        floor(85.0, 325.0) > floor(85.0, 265.0),
        "a dearer Settler needs a better seat"
    );
    assert!(
        floor(105.0, 265.0) > sixteenth,
        "a shorter game needs a better seat"
    );
    assert!(
        floor(95.0, 265.0 / 1.5) < 40.0,
        "Colonization's Settler pays where the plain one needs a good seat"
    );
    assert!(floor(195.0, 100.0).is_infinite(), "no turns left");
    assert!(
        floor(175.0, 40.0).is_infinite(),
        "a city that would never start earning"
    );
}

/// One Settler for a lone capital or an army short of a body per city, then
/// one more for every two cities, to six.
#[test]
fn the_pipeline_widens_with_the_empire_and_its_army() {
    let width = AdvancedAi::no_city_limit_width_for;
    assert_eq!(width(1, 5), 1, "a lone capital");
    assert_eq!(width(4, 3), 1, "an army short of a soldier per city");
    assert_eq!(width(2, 2), 2);
    assert_eq!(width(4, 4), 3);
    assert_eq!(width(7, 9), 4);
    assert_eq!(width(10, 12), NO_CITY_LIMIT_MAX_WIDTH);
    assert_eq!(width(20, 30), NO_CITY_LIMIT_MAX_WIDTH, "the ceiling");
}

/// The lift: with the gene on the settlement target the Settler paths read
/// is every free seat that pays, past the plan's own number, and the plan's
/// number is the one the stock agent computes. Off, nothing is noted.
#[test]
fn the_settlement_target_is_every_seat_that_pays_and_the_plan_keeps_its_own() {
    let (game, _, _) = two_idle_cities();
    let stock = AdvancedAi::new();
    let stock_plan = stock.assess(&game, 0);
    assert_eq!(
        stock.settlement_target(&stock_plan),
        stock_plan.desired_cities,
        "off: the plan's target"
    );
    assert_eq!(
        stock.no_city_limit_target.get(),
        NoCityLimitTarget::default(),
        "off: nothing is noted"
    );

    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert_eq!(
        plan.desired_cities, stock_plan.desired_cities,
        "the plan keeps its own number"
    );
    assert_eq!(plan.strategy, stock_plan.strategy);
    let origins: Vec<Pos> = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].pos)
        .collect();
    let floor = ai
        .no_city_limit_pay_floor(&game, 0)
        .max(AdvancedAi::settler_site_gate_floor(2));
    let seats = ai.holdable_seats_worth(&game, 0, &origins, &[], floor);
    let free = AdvancedAi::good_land_free_seats(&game, &seats, &[], 0, 24);
    assert!(free > 0, "fixture: open land beside the two cities");
    assert_eq!(
        ai.settlement_target(&plan),
        plan.desired_cities.max(2 + free),
        "two cities plus every free seat that pays"
    );
    let noted = ai.no_city_limit_target.get();
    assert_eq!(
        (noted.turn, noted.cities),
        (game.turn, ai.settlement_target(&plan))
    );
    assert_eq!(
        ai.no_city_limit_free_seats(&game, 0, &origins, floor, 0).0,
        free,
        "the lazy count agrees with the full scan"
    );
}

/// Past the horizon no seat pays, and the target is the plan's again.
#[test]
fn no_seat_pays_once_the_game_is_nearly_over() {
    let (mut game, _, _) = two_idle_cities();
    game.turn = (f64::from(game.max_turns) * NO_CITY_LIMIT_EXPECTED_END_SHARE) as u32 - 5;
    let ai = armed();
    assert!(ai.no_city_limit_pay_floor(&game, 0).is_infinite());
    let plan = ai.assess(&game, 0);
    assert_eq!(ai.settlement_target(&plan), plan.desired_cities);
}

/// A calibrated chair keeps its curve: the lift is not asked there.
#[test]
fn a_paced_chair_is_not_lifted() {
    let (game, _, _) = two_idle_cities();
    let mut ai = armed();
    ai.seat_city_pace = true;
    let plan = ai.assess(&game, 0);
    assert_eq!(ai.settlement_target(&plan), plan.desired_cities);
}

/// Several Settlers at once: with a soldier for each of two cities both
/// train a Settler in the opening, where the stock floors queue one; an army
/// short of that queues one.
#[test]
fn two_garrisoned_cities_train_two_settlers_at_once() {
    let (mut game, _, _) = two_idle_cities();
    garrison(&mut game);
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert!(
        ai.settlement_target(&plan) >= 4,
        "fixture: two free seats that pay"
    );
    let mut on = game.clone();
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    ai.claim_holdable_land_settler(&mut on, 0, &plan);
    assert_eq!(settlers_queued(&on).len(), 2, "one Settler in each city");
    for cid in settlers_queued(&on) {
        assert!(
            ai.early_settler_floor_holds(&on, 0, cid, &plan),
            "the routine claims' hold keeps each Settler"
        );
    }
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    ai.claim_holdable_land_settler(&mut on, 0, &plan);
    assert_eq!(settlers_queued(&on).len(), 2, "the width is two");

    let (bare, _, _) = two_idle_cities();
    let plan = ai.assess(&bare, 0);
    let mut thin = bare.clone();
    ai.claim_early_settler_floor(&mut thin, 0, &plan);
    ai.claim_holdable_land_settler(&mut thin, 0, &plan);
    assert_eq!(
        settlers_queued(&thin).len(),
        1,
        "no soldiers: one Settler at a time"
    );
}

/// A threatened city keeps its own queue and its neighbours'; a city far
/// from it still trains. Off, the whole empire waits.
#[test]
fn only_the_cities_at_the_threatened_one_wait() {
    let (game, capital, second) = two_idle_cities();
    let mut plan = armed().assess(&game, 0);
    plan.threatened_city = None;
    assert!(AdvancedAi::no_city_limit_city_is_clear(
        &game, &plan, second
    ));
    plan.threatened_city = Some(capital);
    assert!(!AdvancedAi::no_city_limit_city_is_clear(
        &game, &plan, capital
    ));
    let apart = game.wdist(game.cities[&capital].pos, game.cities[&second].pos);
    assert_eq!(
        AdvancedAi::no_city_limit_city_is_clear(&game, &plan, second),
        apart > NO_CITY_LIMIT_FRONT_TILES,
        "a city {apart} tiles from the threatened one"
    );
}

/// A count of free seats stands for a few turns while the empire is
/// unchanged, and is taken again the turn a city is founded.
#[test]
fn the_seat_count_stands_until_the_empire_changes() {
    let (mut game, _, _) = two_idle_cities();
    let ai = armed();
    let plan = ai.assess(&game, 0);
    let first = ai.no_city_limit_target.get();
    assert_eq!(first.counted_turn, game.turn);
    game.turn += 1;
    let later = ai.assess(&game, 0);
    let second = ai.no_city_limit_target.get();
    assert_eq!(second.turn, game.turn, "noted every turn");
    assert_eq!(second.counted_turn, first.counted_turn, "counted once");
    assert_eq!(
        ai.settlement_target(&later),
        later.desired_cities.max(2 + first.free)
    );
    assert!(ai.settlement_target(&plan) >= plan.desired_cities);
    game.turn += NO_CITY_LIMIT_RECOUNT_TURNS;
    ai.assess(&game, 0);
    assert_eq!(
        ai.no_city_limit_target.get().counted_turn,
        game.turn,
        "counted again after the stride"
    );
    let walker = game.cities[&game.player_city_ids(0)[0]].pos;
    game.turn += 1;
    game.spawn_unit("settler", 0, walker);
    ai.assess(&game, 0);
    assert_eq!(
        ai.no_city_limit_target.get().counted_turn,
        game.turn,
        "counted again when a Settler appears"
    );
}

/// A raider in the home ring stops an ungarrisoned city's Settler and not a
/// garrisoned one's.
#[test]
fn a_garrisoned_city_trains_through_raiders() {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, true);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let barbarians = game.barb_pid.expect("fixture: a barbarian seat");
    for uid in game.player_unit_ids(barbarians) {
        game.remove_unit(uid);
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    assert!(
        !AdvancedAi::early_settler_raider_near(&game, home),
        "fixture: no raider yet"
    );
    let beside = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            game.wdist(*pos, home) == 3
                && !game.rules.is_water(&game.map.tiles[pos])
                && game.rules.is_passable(&game.map.tiles[pos])
                && game.units.values().all(|unit| unit.pos != *pos)
        })
        .expect("fixture: a land tile three from the capital");
    game.spawn_unit("warrior", barbarians, beside);
    assert!(
        AdvancedAi::early_settler_raider_near(&game, home),
        "fixture: a raider in the ring"
    );
    assert!(AdvancedAi::no_city_limit_raider_holds(&game, 0, home));
    game.spawn_unit("warrior", 0, home);
    assert!(!AdvancedAi::no_city_limit_raider_holds(&game, 0, home));
}

/// At war a city with an enemy soldier inside nine tiles trains no Settler;
/// one farther away may.
#[test]
fn the_front_line_city_trains_no_settler() {
    let (game, capital, _) = two_idle_cities();
    let home = game.cities[&capital].pos;
    let near = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(*pos, home) == NO_CITY_LIMIT_WAR_CITY_TILES)
        .expect("fixture: a tile nine away");
    let far = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(*pos, home) == NO_CITY_LIMIT_WAR_CITY_TILES + 1)
        .expect("fixture: a tile ten away");
    assert!(AdvancedAi::no_city_limit_war_at_the_gates(
        &game,
        &[near],
        home
    ));
    assert!(!AdvancedAi::no_city_limit_war_at_the_gates(
        &game,
        &[far],
        home
    ));
    assert!(!AdvancedAi::no_city_limit_war_at_the_gates(
        &game,
        &[],
        home
    ));
}
