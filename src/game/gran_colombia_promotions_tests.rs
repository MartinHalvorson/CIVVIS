use super::*;

fn fixture(civ: &str) -> (Game, u32) {
    let mut game = Game::new_full(2, 24, 16, 590_105, 250, 0, false);
    game.units.clear();
    game.barb_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.owner_city = None;
    }
    game.players[0].civ = civ.to_string();
    game.players[1].civ = "Rome".to_string();
    game.found_city_for(0, (4, 4), None);
    game.found_city_for(1, (18, 10), None);
    game.current = 0;
    game.turn = 44;
    let unit = game.spawn_test_unit("warrior", 0, (10, 6));
    let soldier = game.units.get_mut(&unit).unwrap();
    soldier.xp = 15;
    soldier.hp = 26;
    soldier.moves_left = 2.0;
    soldier.attacks_left = 1;
    assert!(game
        .available_promotions(unit)
        .contains(&crate::name!("tortoise")));
    (game, unit)
}

fn promote(game: &mut Game, unit: u32) -> Result<(), String> {
    game.apply(
        0,
        &Action::Promote {
            unit,
            promotion: crate::name!("tortoise"),
        },
    )
}

#[test]
fn gran_colombia_keeps_its_remaining_turn_after_healing_promotion() {
    let (mut game, unit) = fixture("Gran Colombia");
    assert_eq!(game.civ_effect(0, "promote_no_finish_moves"), 1.0);
    promote(&mut game, unit).unwrap();
    let soldier = &game.units[&unit];
    assert_eq!(soldier.hp, 76);
    assert_eq!(soldier.level, 2);
    assert_eq!(soldier.xp, 15);
    assert!(soldier.promotions.contains(&crate::name!("tortoise")));
    assert_eq!(soldier.moves_left, 2.0);
    assert_eq!(soldier.attacks_left, 1);
}

#[test]
fn a_gran_colombia_veteran_can_attack_after_promoting_in_the_same_turn() {
    let (mut game, unit) = fixture("Gran Colombia");
    game.at_war.insert((0, 1));
    game.spawn_test_unit("warrior", 1, (11, 6));
    let attack = Action::Attack {
        unit,
        target: (11, 6),
    };
    assert!(game.legal_actions(0).contains(&attack));
    promote(&mut game, unit).unwrap();
    game.apply(0, &attack).unwrap();
}

#[test]
fn promotion_does_not_refill_spent_movement_or_attacks() {
    let (mut game, unit) = fixture("Gran Colombia");
    let soldier = game.units.get_mut(&unit).unwrap();
    soldier.moves_left = 1.0;
    soldier.attacks_left = 0;
    soldier.acted = true;
    soldier.moved = true;
    promote(&mut game, unit).unwrap();
    assert_eq!(game.units[&unit].moves_left, 1.0);
    assert_eq!(game.units[&unit].attacks_left, 0);
}

#[test]
fn ordinary_promotions_still_finish_the_units_turn() {
    let (mut game, unit) = fixture("Rome");
    assert_eq!(game.civ_effect(0, "promote_no_finish_moves"), 0.0);
    promote(&mut game, unit).unwrap();
    assert_eq!(game.units[&unit].hp, 76);
    assert_eq!(game.units[&unit].moves_left, 0.0);
    assert_eq!(game.units[&unit].attacks_left, 0);
}

#[test]
fn keeping_movement_cannot_repeat_a_promotion_for_free_healing() {
    let (mut game, unit) = fixture("Gran Colombia");
    promote(&mut game, unit).unwrap();
    let hp = game.units[&unit].hp;
    let xp = game.units[&unit].xp;
    assert!(promote(&mut game, unit).is_err());
    assert_eq!(game.units[&unit].hp, hp);
    assert_eq!(game.units[&unit].xp, xp);
}

#[test]
fn the_civilization_trait_does_not_bypass_promotion_permissions() {
    let (mut game, unit) = fixture("Gran Colombia");
    game.units.get_mut(&unit).unwrap().xp = 0;
    assert!(promote(&mut game, unit).is_err());
    assert_eq!(game.units[&unit].hp, 26);
    game.units.get_mut(&unit).unwrap().xp = 15;
    game.units.get_mut(&unit).unwrap().owner = 1;
    assert!(promote(&mut game, unit).is_err());
    assert_eq!(game.units[&unit].hp, 26);
}
