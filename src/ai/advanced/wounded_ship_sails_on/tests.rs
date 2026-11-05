use super::*;

/// Player 0 holds one coastal city and knows Cartography. A wounded Galley
/// stands in neutral water at least `away` tiles from the city, on a sea that
/// is otherwise dark. Returns the game, the city and the ship.
fn ship_at_sea(hp: i32, away: i32) -> (Game, u32, u32) {
    for seed in 0..80u64 {
        let mut game = Game::new_full(2, 40, 24, 512_300 + seed, 250, 0, false);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        game.current = 0;
        for tech in ["sailing", "shipbuilding", "cartography"] {
            game.players[0].techs.insert(Name::new(tech));
        }
        let site = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && game.nbrs(**pos).into_iter().any(|next| {
                        game.map
                            .get(next)
                            .is_some_and(|tile| game.rules.is_water(tile))
                    })
            })
            .map(|(pos, _)| *pos)
            .min();
        let Some(site) = site else {
            continue;
        };
        let city = game.place_city(0, site, None);
        game.turn = 100;
        let water = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && game.wdist(**pos, site) >= away
            })
            .map(|(pos, _)| *pos)
            .min();
        let Some(water) = water else {
            continue;
        };
        let ship = game.spawn_test_unit("galley", 0, water);
        game.units.get_mut(&ship).unwrap().hp = hp;
        let near: Vec<Pos> = game.wdisk(water, 2);
        game.players[0].explored.clear();
        game.players[0].explored.extend(near);
        if BasicAi::naval_recon_ship_can_chart(&game, 0, ship) {
            return (game, city, ship);
        }
    }
    panic!("fixture: no seed left a wounded ship with water to chart");
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    ai.enable_explore_commit();
    if gene {
        ai.enable_wounded_ship_sails_on();
    }
    ai
}

#[test]
fn a_ship_with_no_healing_water_in_reach_sails_on() {
    let (game, _, ship) = ship_at_sea(40, 8);
    assert_eq!(game.unit_heal_rate_at(ship, game.units[&ship].pos), 0);
    assert!(seat(true).wounded_ship_sails_on(&game, 0, ship));
}

#[test]
fn off_by_default() {
    let (game, _, ship) = ship_at_sea(40, 8);
    assert!(!seat(false).wounded_ship_sails_on(&game, 0, ship));
}

#[test]
fn friendly_water_in_reach_still_wins() {
    let (mut game, city, ship) = ship_at_sea(40, 8);
    let pos = game.units[&ship].pos;
    let tile = game.map.tiles.get_mut(&pos).unwrap();
    tile.owner_city = Some(city);
    assert!(game.unit_heal_rate_at(ship, pos) > 0, "fixture: friendly");
    assert!(!seat(true).wounded_ship_sails_on(&game, 0, ship));
}

#[test]
fn a_ship_with_nothing_left_to_chart_is_left_alone() {
    let (mut game, _, ship) = ship_at_sea(40, 8);
    let all: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(all);
    assert!(!seat(true).wounded_ship_sails_on(&game, 0, ship));
}

#[test]
fn a_land_unit_is_not_a_ship() {
    let (mut game, city, _) = ship_at_sea(40, 8);
    let home = game.cities[&city].pos;
    let scout = game.spawn_test_unit("warrior", 0, home);
    game.units.get_mut(&scout).unwrap().hp = 40;
    assert!(!seat(true).wounded_ship_sails_on(&game, 0, scout));
}
