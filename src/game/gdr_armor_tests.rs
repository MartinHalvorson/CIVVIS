use super::*;

fn armor_game() -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 20, 14, 3_880_001, 300, 0, false);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    for player in game.players.iter_mut() {
        player.civ = "Rome".to_string();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
    }
    let center = *game
        .map
        .tiles
        .keys()
        .find(|pos| game.wdisk(**pos, 2).len() == 19)
        .unwrap();
    let robot = game.spawn_unit("giant_death_robot", 1, center);
    let armor = game.spawn_unit("modern_armor", 0, game.nbrs(center)[0]);
    game.at_war.insert(pair(0, 1));
    (game, robot, armor)
}

#[test]
fn smart_materials_does_not_increase_gdr_attack_or_nominal_strength() {
    let (mut game, robot, armor) = armor_game();
    let nominal = game.unit_strength(&game.units[&robot], false);
    let melee = game.melee_exchange_strengths(robot, armor).unwrap();
    let ranged = game
        .ranged_strike_strengths(robot, armor, game.units[&armor].pos)
        .unwrap();
    game.players[1]
        .techs
        .insert(crate::name!("smart_materials"));
    assert_eq!(game.unit_strength(&game.units[&robot], false), nominal);
    assert_eq!(game.melee_exchange_strengths(robot, armor).unwrap(), melee);
    assert_eq!(
        game.ranged_strike_strengths(robot, armor, game.units[&armor].pos)
            .unwrap(),
        ranged
    );
}

#[test]
fn smart_materials_preserves_ten_strength_against_ground_melee_and_ranged() {
    let (mut game, robot, armor) = armor_game();
    let ranged = game.spawn_unit("machine_gun", 0, game.units[&armor].pos);
    let melee_before = game.melee_exchange_strengths(armor, robot).unwrap();
    let ranged_before = game
        .ranged_strike_strengths(ranged, robot, game.units[&robot].pos)
        .unwrap();
    game.players[1]
        .techs
        .insert(crate::name!("smart_materials"));
    let melee_after = game.melee_exchange_strengths(armor, robot).unwrap();
    let ranged_after = game
        .ranged_strike_strengths(ranged, robot, game.units[&robot].pos)
        .unwrap();
    assert_eq!(melee_after.0, melee_before.0);
    assert_eq!(melee_after.1, melee_before.1 + 10.0);
    assert_eq!(ranged_after.0, ranged_before.0);
    assert_eq!(ranged_after.1, ranged_before.1 + 10.0);
}

#[test]
fn smart_materials_does_not_stack_on_dedicated_air_defense() {
    let (mut game, robot, _) = armor_game();
    let before = game.unit_anti_air_strength(&game.units[&robot]);
    game.players[1]
        .techs
        .insert(crate::name!("smart_materials"));
    assert_eq!(game.unit_anti_air_strength(&game.units[&robot]), before);
    game.players[1].techs.insert(crate::name!("robotics"));
    game.players[1].techs.insert(crate::name!("advanced_ai"));
    assert_eq!(
        game.unit_anti_air_strength(&game.units[&robot]),
        before + 40.0
    );
}

#[test]
fn smart_materials_does_not_add_armor_to_other_unit_types() {
    let (mut game, _, armor) = armor_game();
    let attacking = game.unit_strength(&game.units[&armor], false);
    let defending = game.unit_strength(&game.units[&armor], true);
    game.players[0]
        .techs
        .insert(crate::name!("smart_materials"));
    assert_eq!(game.unit_strength(&game.units[&armor], false), attacking);
    assert_eq!(game.unit_strength(&game.units[&armor], true), defending);
}

#[test]
fn smart_materials_does_not_increase_gdr_melee_city_attack() {
    let (mut game, robot, armor) = armor_game();
    let city = game.found_city_for(0, game.units[&armor].pos, None);
    let before = game.city_melee_exchange_strengths(robot, city).unwrap();
    game.players[1]
        .techs
        .insert(crate::name!("smart_materials"));
    assert_eq!(
        game.city_melee_exchange_strengths(robot, city).unwrap(),
        before
    );
}
