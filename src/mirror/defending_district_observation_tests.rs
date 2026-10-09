use super::*;

const CITY: (i32, i32) = (10, 10);
const ENCAMPMENT: (i32, i32) = (7, 10);
const OPPIDUM: (i32, i32) = (13, 10);
const TARGETS: [(i32, i32); 2] = [(6, 10), (14, 10)];

fn fixture(
    encampment: Option<(i32, i32)>,
    oppidum: Option<(i32, i32)>,
    pillaged: bool,
) -> crate::game::Game {
    let mut plots = Vec::new();
    for x in 0..20 {
        for y in 0..20 {
            let mut row = serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true});
            if (x, y) == CITY {
                row["o"] = 3.into();
                row["d"] = "DISTRICT_CITY_CENTER".into();
            }
            for (position, kind, observation) in [
                (ENCAMPMENT, "DISTRICT_ENCAMPMENT", encampment),
                (OPPIDUM, "DISTRICT_OPPIDUM", oppidum),
            ] {
                if (x, y) == position {
                    if let Some((damage, wall_damage)) = observation {
                        row["o"] = 3.into();
                        row["d"] = kind.into();
                        row["dc"] = true.into();
                        row["oc"] = 31.into();
                        row["p"] = (kind == "DISTRICT_OPPIDUM" && pillaged).into();
                        row["dh"] = serde_json::json!({"damage":damage,"max_damage":100,
                            "wall_damage":wall_damage,"max_wall_damage":200});
                    }
                }
            }
            plots.push(serde_json::from_value(row).unwrap());
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 180,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 180,
        cities: vec![StateCity {
            id: 31,
            x: 2,
            y: 2,
            pop: 5,
            ..StateCity::default()
        }],
        rivals: vec![StateRival {
            player: 3,
            civ: "CIVILIZATION_GAUL".into(),
            at_war: true,
            cities: vec![StateCity {
                id: 31,
                x: CITY.0,
                y: CITY.1,
                pop: 5,
                loyalty: 100.0,
                damage: 0.0,
                max_damage: 200.0,
                wall_damage: 0.0,
                max_wall_damage: 200.0,
                ..StateCity::default()
            }],
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    rebuild_from_state(&snapshot, &state, 4, 1, 500, 0).game
}

fn owner(game: &crate::game::Game) -> usize {
    game.cities[&game
        .city_at(crate::hex::offset_to_axial(CITY.0, CITY.1))
        .unwrap()]
        .owner
}

// Exercise legal actions and actual damage without naming a future action
// variant. Both target tiles lie outside the city center's strike range.
fn damaging_shot(
    game: &crate::game::Game,
    target: crate::Pos,
    uid: u32,
) -> Option<crate::game::Action> {
    let player = owner(game);
    let value = serde_json::to_value(target).unwrap();
    game.legal_actions(player).into_iter().find(|action| {
        if serde_json::to_value(action).unwrap().get("target") != Some(&value) {
            return false;
        }
        let mut candidate = game.clone();
        candidate.apply(player, action).is_ok()
            && candidate
                .units
                .get(&uid)
                .is_none_or(|unit| unit.hp < game.units[&uid].hp)
    })
}

fn target(game: &mut crate::game::Game, index: usize) -> (crate::Pos, u32) {
    let pos = crate::hex::offset_to_axial(TARGETS[index].0, TARGETS[index].1);
    let uid = game.spawn_test_unit("warrior", 0, pos);
    game.current = owner(game);
    assert!(game.is_at_war(0, game.current));
    let center = crate::hex::offset_to_axial(CITY.0, CITY.1);
    assert!(
        crate::hex::distance(center, pos) > 2,
        "city-center fire cannot satisfy this fixture"
    );
    (pos, uid)
}

fn shoot_once(game: &mut crate::game::Game, index: usize) {
    let (pos, uid) = target(game, index);
    let action = damaging_shot(game, pos, uid)
        .expect("observed healthy fort offers an actual damaging shot");
    game.apply(owner(game), &action)
        .expect("apply district shot");
    assert!(game.units.get(&uid).is_none_or(|unit| unit.hp < 100));
    if game.units.contains_key(&uid) {
        assert!(
            damaging_shot(game, pos, uid).is_none(),
            "each fort spends its own single strike"
        );
    }
}

#[test]
fn an_observed_encampment_still_offers_an_actual_shot() {
    shoot_once(&mut fixture(Some((0, 0)), None, false), 0);
}

#[test]
fn an_observed_oppidum_offers_an_actual_shot() {
    shoot_once(&mut fixture(None, Some((0, 0)), false), 1);
}

#[test]
fn a_city_with_both_forts_has_two_independent_actual_shots() {
    let mut game = fixture(Some((0, 0)), Some((0, 0)), false);
    shoot_once(&mut game, 0);
    shoot_once(&mut game, 1);
}

#[test]
fn a_depleted_encampment_does_not_suppress_a_healthy_oppidum() {
    let mut game = fixture(Some((100, 200)), Some((0, 0)), false);
    let (pos, uid) = target(&mut game, 0);
    assert!(damaging_shot(&game, pos, uid).is_none());
    shoot_once(&mut game, 1);
}

#[test]
fn a_depleted_oppidum_does_not_suppress_a_healthy_encampment() {
    let mut game = fixture(Some((0, 0)), Some((100, 200)), false);
    let (pos, uid) = target(&mut game, 1);
    assert!(damaging_shot(&game, pos, uid).is_none());
    shoot_once(&mut game, 0);
}

#[test]
fn a_pillaged_oppidum_has_no_damaging_shot() {
    let mut game = fixture(None, Some((0, 0)), true);
    let (pos, uid) = target(&mut game, 1);
    assert!(damaging_shot(&game, pos, uid).is_none());
}

#[test]
fn an_oppidum_keeps_its_industrial_zone_family_and_separate_roster_entry() {
    let game = fixture(Some((0, 0)), Some((0, 0)), false);
    let cid = game
        .city_at(crate::hex::offset_to_axial(CITY.0, CITY.1))
        .unwrap();
    let city = &game.cities[&cid];
    assert_eq!(
        game.district_family(crate::name!("oppidum")),
        crate::name!("industrial_zone")
    );
    assert_eq!(
        city.districts.get(crate::name!("oppidum")),
        Some(&crate::hex::offset_to_axial(OPPIDUM.0, OPPIDUM.1))
    );
    assert_eq!(
        city.districts.get(crate::name!("encampment")),
        Some(&crate::hex::offset_to_axial(ENCAMPMENT.0, ENCAMPMENT.1))
    );
}
