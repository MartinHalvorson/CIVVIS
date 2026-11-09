use super::*;
use crate::ai::industrial_zones::tests::producer_empire;

fn fixture(kind: &str, distance: i32) -> (Game, BasicAi, u32, u32) {
    let (mut game, cities) = producer_empire("WALLFIRE", 718_421, 2);
    let cid = cities[0];
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    for tech in ["archery", "machinery", "military_tactics"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    let max_wall = game.city_max_wall_hp(&game.cities[&cid]);
    game.cities.get_mut(&cid).unwrap().wall_hp = max_wall - 10;
    let center = game.cities[&cid].pos;
    let pos = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(center, tile.pos) == distance
        })
        .map(|tile| tile.pos)
        .min()
        .expect("land in the requested ring");
    let enemy = game.spawn_unit(kind, 1, pos);
    let mut ai = BasicAi::new();
    ai.garrison_under_fire = true;
    (game, ai, cid, enemy)
}

#[test]
fn wall_hits_reach_the_existing_defender_response_before_hull_damage() {
    let (game, ai, cid, _) = fixture("crossbowman", 2);
    assert_eq!(game.cities[&cid].hp, 200);
    let Some(Item::Unit { unit }) = ai.besieged_city_item(&game, 0, cid) else {
        panic!("damaged outer defenses with a nearby enemy need a defender");
    };
    assert_eq!(game.rules.units[unit].class, "military");
    assert!(!game.rules.units[unit].siege);
}

#[test]
fn full_walls_keep_the_existing_picker() {
    let (mut game, ai, cid, _) = fixture("crossbowman", 1);
    let max_wall = game.city_max_wall_hp(&game.cities[&cid]);
    game.cities.get_mut(&cid).unwrap().wall_hp = max_wall;
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
}

#[test]
fn old_wall_damage_without_a_hostile_in_range_keeps_the_economy() {
    let (mut game, ai, cid, enemy) = fixture("crossbowman", 2);
    game.remove_unit(enemy);
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
    let (game, ai, cid, _) = fixture("crossbowman", 4);
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
}

#[test]
fn peaceful_and_civilian_neighbors_do_not_trigger_defense() {
    let (mut game, ai, cid, enemy) = fixture("crossbowman", 1);
    game.at_war.remove(&(0, 1));
    assert!(!game.is_at_war(0, 1));
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
    game.units.get_mut(&enemy).unwrap().owner = 0;
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
    let (game, ai, cid, _) = fixture("builder", 1);
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
}

#[test]
fn melee_threats_must_be_adjacent() {
    let (game, ai, cid, _) = fixture("warrior", 1);
    assert!(ai.besieged_city_item(&game, 0, cid).is_some());
    let (game, ai, cid, _) = fixture("warrior", 2);
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
}

#[test]
fn the_live_repair_switch_and_unwalled_cities_keep_their_boundaries() {
    let (game, mut ai, cid, _) = fixture("crossbowman", 1);
    ai.garrison_under_fire = false;
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
    let (mut game, ai, cid, _) = fixture("crossbowman", 1);
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|b| *b != "walls");
    game.cities.get_mut(&cid).unwrap().wall_hp = 0;
    assert!(ai.besieged_city_item(&game, 0, cid).is_none());
}

#[test]
fn hull_damage_retains_its_existing_response_without_visible_attackers() {
    let (mut game, ai, cid, enemy) = fixture("crossbowman", 1);
    game.remove_unit(enemy);
    game.cities.get_mut(&cid).unwrap().hp = 199;
    assert!(ai.besieged_city_item(&game, 0, cid).is_some());
}

#[test]
fn an_enemy_that_spent_its_moves_still_threatens_the_next_turn() {
    let (mut game, ai, cid, enemy) = fixture("crossbowman", 2);
    game.units.get_mut(&enemy).unwrap().moves = 0.0;
    assert!(ai.besieged_city_item(&game, 0, cid).is_some());
}
