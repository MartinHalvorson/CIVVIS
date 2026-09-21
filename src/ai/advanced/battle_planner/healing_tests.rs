use super::*;

fn embarked_siege() -> (Game, u32, Pos) {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    g.tactics.heal = true;
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let sea = crate::hex::offset_to_axial(10, 8);
    let tile = g.map.tiles.get_mut(&sea).unwrap();
    tile.terrain = crate::name!("coast");
    tile.owner_city = None;
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    let uid = g.spawn_unit("trebuchet", 0, sea);
    g.units.get_mut(&uid).unwrap().hp = 22;
    (g, uid, sea)
}

#[test]
fn wounded_embarked_siege_recovers_on_land_instead_of_waiting_at_sea() {
    let (mut g, uid, sea) = embarked_siege();
    assert!(g.is_embarked(&g.units[&uid]));
    assert_eq!(g.unit_heal_rate(uid), 0);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    let mut field = DangerField::new(&g, 0);
    ai.rotate_wounded(&mut g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
    assert_ne!(
        g.units[&uid].pos, sea,
        "neutral water cannot heal this unit"
    );
    assert!(!g.is_embarked(&g.units[&uid]));
    assert!(g.unit_heal_rate(uid) > 0);
    assert!(ai.battle_planner_claims(uid));
}

#[test]
fn projected_healing_uses_destination_without_mutating_the_unit() {
    let (mut g, uid, sea) = embarked_siege();
    let shore = crate::hex::offset_to_axial(11, 8);
    let tile = g.map.tiles.get_mut(&shore).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.owner_city = None;
    assert_eq!(g.unit_heal_rate_at(uid, sea), 0);
    assert_eq!(g.unit_heal_rate_at(uid, shore), 10);
    assert_eq!(g.units[&uid].pos, sea);
    assert_eq!(g.units[&uid].hp, 22);
    assert_eq!(g.unit_heal_rate(uid), 0);
    g.map.tiles.get_mut(&shore).unwrap().fallout_until = g.turn + 5;
    assert_eq!(g.unit_heal_rate_at(uid, shore), 0);
}

#[test]
fn an_embarked_unit_can_stay_in_friendly_water_to_recover() {
    let (mut g, uid, sea) = embarked_siege();
    let city = g.found_city_for(0, crate::hex::offset_to_axial(11, 8), None);
    g.map.tiles.get_mut(&sea).unwrap().owner_city = Some(city);
    assert_eq!(g.unit_heal_rate(uid), 20);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    let mut field = DangerField::new(&g, 0);
    ai.rotate_wounded(&mut g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
    assert_eq!(g.units[&uid].pos, sea);
    assert_eq!(g.unit_heal_rate(uid), 20);
    assert!(ai.battle_planner_claims(uid));
}
