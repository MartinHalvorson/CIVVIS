use super::super::AdvancedAi;
use super::{
    threatens_our_walls, OPEN_CITY_HP, WALL_GUN_MARGIN, WALL_THREAT_DAMAGE_PREMIUM,
    WALL_THREAT_KILL_PREMIUM,
};
use crate::game::{Action, Game};

/// Two majors at war, each with a city on dry open ground; seat 0's capital
/// walled at full health. Returns the board and the capital's tile.
fn walls() -> (Game, crate::Pos) {
    let mut game = Game::new_full(2, 24, 16, 7_961, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let capital = game.cities[&cid].pos;
    (game, capital)
}

/// A free tile exactly `distance` from `from`, off any City Center.
fn free_at(game: &Game, from: crate::Pos, distance: i32) -> crate::Pos {
    game.wdisk(from, distance)
        .into_iter()
        .filter(|pos| {
            game.wdist(*pos, from) == distance
                && game.unit_ids_at(*pos).is_empty()
                && game.city_at(*pos).is_none()
        })
        .min()
        .expect("a free tile at that distance")
}

#[test]
fn a_gun_in_reach_of_our_walls_is_a_threat() {
    let (mut game, capital) = walls();
    let gun = game.spawn_test_unit("catapult", 1, free_at(&game, capital, 2));
    assert!(threatens_our_walls(&game, 0, gun), "in range of the walls");
    let reach = game.unit_attack_range(gun) + WALL_GUN_MARGIN;
    let one_move = game.spawn_test_unit("catapult", 1, free_at(&game, capital, reach));
    assert!(
        threatens_our_walls(&game, 0, one_move),
        "one move from firing"
    );
    let far = game.spawn_test_unit("catapult", 1, free_at(&game, capital, reach + 1));
    assert!(!threatens_our_walls(&game, 0, far), "beyond a move");
    assert!(
        !threatens_our_walls(&game, 1, gun),
        "our own gun is no threat"
    );
    game.at_war.remove(&(0, 1));
    assert!(!threatens_our_walls(&game, 0, gun), "at peace");
}

#[test]
fn a_taker_threatens_only_an_open_city() {
    let (mut game, capital) = walls();
    let cid = game.player_city_ids(0)[0];
    let taker = game.spawn_test_unit("warrior", 1, free_at(&game, capital, 1));
    assert!(
        !threatens_our_walls(&game, 0, taker),
        "walls up, full health"
    );
    game.cities.get_mut(&cid).unwrap().wall_hp = 0;
    assert!(
        !threatens_our_walls(&game, 0, taker),
        "walls down, full health"
    );
    game.cities.get_mut(&cid).unwrap().hp = OPEN_CITY_HP;
    assert!(
        threatens_our_walls(&game, 0, taker),
        "walls down, half its health"
    );
    let shooter = game.spawn_test_unit("archer", 1, free_at(&game, capital, 1));
    assert!(
        !threatens_our_walls(&game, 0, shooter),
        "a shooter takes no city"
    );
    let behind = game.spawn_test_unit("warrior", 1, free_at(&game, capital, 2));
    assert!(!threatens_our_walls(&game, 0, behind), "not yet beside it");
}

#[test]
fn the_gene_ships_off_and_gates_every_premium() {
    let (mut game, capital) = walls();
    let gun = game.spawn_test_unit("catapult", 1, free_at(&game, capital, 2));
    let off = AdvancedAi::new();
    assert!(!off.siege_at_the_walls_dies_first, "an opt-in ships off");
    assert!(!off.wall_threat(&game, 0, gun));
    assert_eq!(off.wall_threat_premium(&game, 0, gun, 30.0, false), 0.0);
    let mut on = AdvancedAi::new();
    on.enable_siege_at_the_walls_dies_first();
    assert!(on.wall_threat(&game, 0, gun));
    assert_eq!(
        on.wall_threat_premium(&game, 0, gun, 30.0, false),
        30.0 * WALL_THREAT_DAMAGE_PREMIUM
    );
    assert_eq!(
        on.wall_threat_premium(&game, 0, gun, 0.0, true),
        WALL_THREAT_KILL_PREMIUM
    );
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "siege_at_the_walls_dies_first"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "siege-at-the-walls-dies-first",
        |ai| ai.siege_at_the_walls_dies_first,
    );
}

/// The city's strike: the gun earns the premium, the soldier beside the
/// walls (a full-health city's) does not, and both read the same off.
#[test]
fn the_city_strike_prices_the_gun_first() {
    let (mut game, capital) = walls();
    let cid = game.player_city_ids(0)[0];
    let gun_at = free_at(&game, capital, 2);
    game.spawn_test_unit("catapult", 1, gun_at);
    let raider_at = free_at(&game, capital, 1);
    game.spawn_test_unit("warrior", 1, raider_at);
    let strike = |target| Action::CityStrike { city: cid, target };
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_siege_at_the_walls_dies_first();
    let gun_off = off.defensive_strike_value(&game, 0, &strike(gun_at));
    let gun_on = on.defensive_strike_value(&game, 0, &strike(gun_at));
    assert!(gun_off.is_finite(), "the gun is in the strike's reach");
    assert!(gun_on > gun_off, "{gun_on} > {gun_off}");
    assert_eq!(
        off.defensive_strike_value(&game, 0, &strike(raider_at)),
        on.defensive_strike_value(&game, 0, &strike(raider_at)),
        "a soldier beside walls that stand is priced as before"
    );
    let premium = on.wall_threat_blow_premium(&game, 0, &strike(gun_at));
    assert!(
        (premium - (gun_on - gun_off)).abs() < 1e-6,
        "the ladder's clone reads the same premium: {premium} against {}",
        gun_on - gun_off
    );
    assert_eq!(off.wall_threat_blow_premium(&game, 0, &strike(gun_at)), 0.0);
}
