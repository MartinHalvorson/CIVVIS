use super::*;

/// A founded capital and one more city four to six tiles away, every unit
/// removed, on a turn inside the payback horizon.
fn two_cities() -> Game {
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
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for site in sites {
        if game.wdist(site, home) < 4 || game.wdist(site, home) > 6 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            break;
        }
        game.remove_unit(settler);
    }
    assert_eq!(game.player_city_ids(0).len(), 2, "fixture: a second city");
    game.turn = 40;
    game
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_good_land_lifts_the_city_target();
    ai
}

fn origins(game: &Game) -> Vec<Pos> {
    game.player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].pos)
        .collect()
}

/// The arithmetic: one city per free good seat, a walking Settler keeps the
/// seat it set out for, never below what the plan wanted and never above
/// twelve.
#[test]
fn the_target_rises_one_city_per_free_good_seat_to_twelve() {
    let lifted = AdvancedAi::good_land_lifted_target;
    assert_eq!(lifted(10, 10, 0, 0), 10, "no good seat, no lift");
    assert_eq!(lifted(10, 10, 0, 1), 11);
    assert_eq!(lifted(10, 10, 0, 2), 12);
    assert_eq!(lifted(10, 10, 0, 5), GOOD_LAND_CITY_CEILING, "the ceiling");
    assert_eq!(
        lifted(10, 8, 0, 2),
        10,
        "seats the ordinary target would already fill"
    );
    assert_eq!(lifted(10, 8, 0, 3), 11);
    assert_eq!(lifted(10, 9, 1, 0), 10);
    assert_eq!(
        lifted(10, 10, 1, 0),
        11,
        "a Settler that set out under the lift keeps its seat"
    );
    assert_eq!(lifted(10, 11, 1, 3), GOOD_LAND_CITY_CEILING);
}

/// Seats are counted the way the Settler gates count them: one per
/// `SETTLEMENT_SPACING`, none beside a target a Settler already holds, and
/// one fewer for each Settler still without a target.
#[test]
fn free_seats_keep_their_spacing_and_seat_the_walkers_first() {
    let game = two_cities();
    let home = origins(&game)[0];
    let mut line: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == SETTLEMENT_SPACING * 2)
        .collect();
    line.sort();
    let far = line[0];
    let near_far = game
        .nbrs(far)
        .into_iter()
        .find(|pos| game.map.get(*pos).is_some())
        .expect("a neighbour on the map");
    assert!(game.wdist(far, near_far) < SETTLEMENT_SPACING);
    let sites = [(far, 120.0), (near_far, 110.0), (home, 90.0)];
    let free = |claimed: &[Pos], unseated, wanted| {
        AdvancedAi::good_land_free_seats(&game, &sites, claimed, unseated, wanted)
    };
    assert_eq!(free(&[], 0, 4), 2, "two plots a tile apart are one seat");
    assert_eq!(free(&[], 0, 1), 1, "the count stops at what is wanted");
    assert_eq!(free(&[far], 0, 4), 1, "a claimed target is not a free seat");
    assert_eq!(free(&[], 1, 4), 1, "a Settler without a target takes one");
    assert_eq!(free(&[], 2, 4), 0);
    assert_eq!(free(&[], 3, 4), 0, "never negative");
}

/// Every good seat is worth the bar, lies outside the holdable-land distance
/// of a charted rival city, and the list is best-first.
#[test]
fn good_seats_are_worth_the_bar_and_clear_of_rival_cities() {
    let game = two_cities();
    let ai = armed();
    let seats = ai.good_land_seats(&game, 0, &origins(&game));
    assert!(!seats.is_empty(), "fixture: good land within reach");
    assert!(seats
        .iter()
        .all(|(_, worth)| *worth >= GOOD_LAND_SEAT_WORTH));
    assert!(
        seats.windows(2).all(|pair| pair[0].1 >= pair[1].1),
        "best-first"
    );
    // The list any one city's Settler gate reads holds each of them at no
    // more than the worth counted here.
    let from_capital = ai.settle_sites(&game, 0, origins(&game)[0], 11);
    for (site, worth) in &from_capital {
        if let Some((_, counted)) = seats.iter().find(|(seat, _)| seat == site) {
            assert!(counted + 1e-9 >= *worth, "the best origin's worth is kept");
        }
    }

    // A charted rival city inside the holdable-land distance of the best
    // seat takes it off the list; one exactly at that distance does not.
    let (best, _) = seats[0];
    let at = |tiles: i32| {
        game.map
            .tiles
            .keys()
            .copied()
            .find(|pos| game.wdist(*pos, best) == tiles)
            .expect("a tile at that distance from the best seat")
    };
    let near = at(HOLDABLE_LAND_RIVAL_CITY_TILES - 1);
    let pressed = ai.good_land_seats_clear_of(&game, 0, &origins(&game), &[near]);
    assert!(
        pressed
            .iter()
            .all(|(seat, _)| game.wdist(*seat, near) >= HOLDABLE_LAND_RIVAL_CITY_TILES),
        "no good seat inside the holdable-land distance of a charted rival city"
    );
    assert!(!pressed.iter().any(|(seat, _)| *seat == best));
    let edge = at(HOLDABLE_LAND_RIVAL_CITY_TILES);
    let clear = ai.good_land_seats_clear_of(&game, 0, &origins(&game), &[edge]);
    assert!(
        clear.iter().any(|(seat, _)| *seat == best),
        "a rival city nine tiles out leaves the seat"
    );
}

/// The lift itself, and each of its bounds: off, far from the ordinary
/// target, at the ceiling, and past the payback horizon the target stays
/// what the plan wanted.
#[test]
fn the_lift_waits_for_the_gene_the_near_target_and_the_payback_horizon() {
    let game = two_cities();
    let cities = game.player_city_ids(0);
    let ai = armed();
    let seats = ai.good_land_seats(&game, 0, &origins(&game));
    let free = AdvancedAi::good_land_free_seats(&game, &seats, &[], 0, GOOD_LAND_CITY_CEILING);
    assert!(free >= 2, "fixture: at least two good seats, found {free}");

    let wanted = cities.len() + 1;
    assert_eq!(
        AdvancedAi::new().good_land_city_target(&game, 0, wanted, &cities),
        wanted,
        "off"
    );
    let lifted = ai.good_land_city_target(&game, 0, wanted, &cities);
    assert_eq!(
        lifted,
        (cities.len() + free).min(GOOD_LAND_CITY_CEILING),
        "one city per free good seat"
    );
    assert!(lifted > wanted);

    let far = cities.len() + GOOD_LAND_NEAR_TARGET + 1;
    assert_eq!(
        ai.good_land_city_target(&game, 0, far, &cities),
        far,
        "the ordinary target is not yet what paces the Settlers"
    );
    assert_eq!(
        ai.good_land_city_target(&game, 0, GOOD_LAND_CITY_CEILING, &cities),
        GOOD_LAND_CITY_CEILING,
        "already at the ceiling"
    );
    assert_eq!(
        ai.good_land_city_target(&game, 0, GOOD_LAND_CITY_CEILING + 3, &cities),
        GOOD_LAND_CITY_CEILING + 3,
        "never lowers a wider plan"
    );

    let mut late = game.clone();
    late.turn = late.max_turns - late.standard_duration(HOLDABLE_LAND_PAYBACK_STANDARD_TURNS);
    assert_eq!(
        ai.good_land_city_target(&late, 0, wanted, &cities),
        wanted,
        "a city founded now would not repay"
    );

    // A Settler on the map without a target takes the first free seat.
    let mut walking = game.clone();
    let home = origins(&walking)[0];
    walking.spawn_unit("settler", 0, home);
    assert_eq!(
        ai.good_land_city_target(&walking, 0, wanted, &cities),
        (cities.len() + 1 + (free - 1)).min(GOOD_LAND_CITY_CEILING),
        "the walker's seat is not counted twice"
    );
}

/// `assess` carries the lift into the plan, and only with the gene on.
#[test]
fn the_plan_reads_the_lift() {
    let game = two_cities();
    let stock = AdvancedAi::new();
    let wanted = stock.assess(&game, 0).desired_cities;
    assert!(
        game.player_city_ids(0).len() + GOOD_LAND_NEAR_TARGET >= wanted
            && wanted < GOOD_LAND_CITY_CEILING,
        "fixture: the empire is near its ordinary target of {wanted}"
    );
    let lifted = armed().assess(&game, 0).desired_cities;
    assert!(
        lifted >= wanted && lifted <= GOOD_LAND_CITY_CEILING,
        "the lift never lowers the plan and never passes the ceiling"
    );
    let ai = armed();
    let cities = game.player_city_ids(0);
    assert_eq!(
        lifted,
        ai.good_land_city_target(&game, 0, wanted, &cities)
            .max(wanted),
        "the plan's target is the lifted one"
    );
}

/// Past the shared target the holdable-land rule asks for a good seat; below
/// it, and with the gene off, it asks for nothing more than it did.
#[test]
fn past_the_shared_target_a_settler_needs_a_good_seat() {
    let ai = armed();
    assert_eq!(ai.good_land_seat_floor(SHARED_CITY_TARGET_FLOOR - 1), 0.0);
    assert_eq!(
        ai.good_land_seat_floor(SHARED_CITY_TARGET_FLOOR),
        GOOD_LAND_SEAT_WORTH
    );
    assert_eq!(
        ai.good_land_seat_floor(GOOD_LAND_CITY_CEILING),
        GOOD_LAND_SEAT_WORTH
    );
    let stock = AdvancedAi::new();
    assert_eq!(stock.good_land_seat_floor(SHARED_CITY_TARGET_FLOOR), 0.0);
    assert_eq!(stock.good_land_seat_floor(GOOD_LAND_CITY_CEILING), 0.0);
}
