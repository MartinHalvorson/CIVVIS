use super::*;

pub(crate) fn fixture() -> (Game, u32, Pos, Pos) {
    let mut g = Game::new_full(4, 24, 18, 40_025, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
    }
    g.players[0].civ = "Gaul".into();
    g.players[1].civ = "Gran Colombia".into();
    g.at_war.insert(pair(0, 1));
    let center = crate::hex::offset_to_axial(10, 10);
    let city = g.found_city_for(0, center, None);
    g.cities.get_mut(&city).unwrap().pop = 7;
    for tech in ["mining", "bronze_working", "iron_working", "masonry"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let enc = crate::hex::offset_to_axial(7, 10);
    let opp = crate::hex::offset_to_axial(13, 10);
    for (kind, pos) in [("encampment", enc), ("oppidum", opp)] {
        g.map.tiles.get_mut(&pos).unwrap().owner_city = Some(city);
        g.cities.get_mut(&city).unwrap().owned_tiles.push(pos);
        assert!(
            g.complete_item(
                0,
                city,
                &Item::District {
                    district: Name::new(kind),
                    pos
                }
            ),
            "fixture must be able to complete {kind}"
        );
    }
    assert!(g.complete_item(
        0,
        city,
        &Item::Building {
            building: crate::name!("walls")
        }
    ));
    g.current = 0;
    (g, city, enc, opp)
}

#[test]
fn construction_initializes_both_forts_without_changing_economic_family() {
    let (g, city, enc, opp) = fixture();
    assert_eq!(g.defending_districts(&g.cities[&city]).count(), 2);
    assert_eq!(g.defending_district_state(city, enc).unwrap().wall_hp, 100);
    assert_eq!(g.defending_district_state(city, opp).unwrap().wall_hp, 100);
    assert_eq!(
        g.district_family(crate::name!("oppidum")),
        crate::name!("industrial_zone")
    );
    assert_eq!(
        g.city_district_family_position(&g.cities[&city], crate::name!("industrial_zone")),
        Some(opp)
    );
}

#[test]
fn damage_to_one_fort_preserves_the_other_fort_and_city_center() {
    let (mut g, city, enc, opp) = fixture();
    let before = g.defending_district_state(city, enc).unwrap();
    let center_hp = (g.cities[&city].hp, g.cities[&city].wall_hp);
    g.defending_district_take_damage(city, opp, 40, 1.0, false);
    let after = g.defending_district_state(city, opp).unwrap();
    assert_eq!((after.hp, after.wall_hp), (99, 60));
    assert_eq!(g.defending_district_state(city, enc), Some(before));
    assert_eq!((g.cities[&city].hp, g.cities[&city].wall_hp), center_hp);
}

#[test]
fn ranged_combat_hits_the_oppidum_pool_and_keeps_its_garrison_protected() {
    let (mut g, city, enc, opp) = fixture();
    let gun = g.spawn_test_unit("bombard", 1, (opp.0 + 1, opp.1));
    let guard = g.spawn_test_unit("warrior", 0, opp);
    let before = g.defending_district_state(city, enc).unwrap();
    g.current = 1;
    g.apply(
        1,
        &Action::Ranged {
            unit: gun,
            target: opp,
        },
    )
    .unwrap();
    assert!(g.defending_district_state(city, opp).unwrap().wall_hp < 100);
    assert_eq!(g.units[&guard].hp, 100);
    assert_eq!(g.defending_district_state(city, enc), Some(before));
}

#[test]
fn melee_enters_and_pillages_a_depleted_oppidum_without_capturing_the_city() {
    let (mut g, city, enc, opp) = fixture();
    let attacker = g.spawn_test_unit("warrior", 1, (opp.0 + 1, opp.1));
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.hp = 0;
    state.wall_hp = 0;
    g.set_defending_district_state(city, state);
    g.current = 1;
    assert_eq!(g.defending_district_at(opp), Some(city));
    g.apply(
        1,
        &Action::Attack {
            unit: attacker,
            target: opp,
        },
    )
    .unwrap();
    assert_eq!(g.units[&attacker].pos, opp);
    assert!(g.defending_district_state(city, opp).unwrap().pillaged);
    assert!(g.map.tiles[&opp].pillaged);
    assert_eq!(g.defending_district_at(opp), None);
    assert_eq!(g.cities[&city].owner, 0);
    assert_eq!(g.defending_district_state(city, enc).unwrap().hp, 100);
}

#[test]
fn turn_boundary_resets_each_fort_and_heals_only_its_own_pool() {
    let (mut g, city, enc, opp) = fixture();
    g.cities.get_mut(&city).unwrap().encampment_struck = true;
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.struck = true;
    state.extra_strikes_used = 1;
    state.hp = 65;
    g.set_defending_district_state(city, state);
    g.begin_turn(0);
    let after = g.defending_district_state(city, opp).unwrap();
    assert_eq!(
        (after.struck, after.extra_strikes_used, after.hp),
        (false, 0, 85)
    );
    assert!(!g.defending_district_state(city, enc).unwrap().struck);
}

#[test]
fn serialization_roundtrip_keeps_depleted_pools_and_spent_shots() {
    let (mut g, city, _, opp) = fixture();
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.hp = 0;
    state.wall_hp = 0;
    state.struck = true;
    g.set_defending_district_state(city, state);
    let restored: Game = serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
    assert_eq!(restored.defending_district_state(city, opp), Some(state));
}

#[test]
fn legacy_save_without_other_fort_state_gets_one_independent_healthy_pool() {
    let (g, city, _, opp) = fixture();
    let mut value = serde_json::to_value(&g).unwrap();
    value["cities"][city.to_string()]
        .as_object_mut()
        .unwrap()
        .remove("defending_districts");
    let restored: Game = serde_json::from_value(value).unwrap();
    assert_eq!(
        restored.defending_district_state(city, opp).unwrap().hp,
        100
    );
    assert_eq!(restored.cities[&city].defending_districts.len(), 1);
}

#[test]
fn public_memory_keeps_fort_health_without_revealing_shot_history() {
    let (mut g, city, _, opp) = fixture();
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.hp = 42;
    state.wall_hp = 19;
    state.struck = true;
    state.last_attacked = 99;
    g.set_defending_district_state(city, state);
    let memory = g.remember_city(&g.cities[&city]);
    let seen = memory.defending_districts[0];
    assert_eq!((seen.hp, seen.wall_hp), (42, 19));
    assert_eq!(
        (seen.struck, seen.extra_strikes_used, seen.last_attacked),
        (false, 0, 0)
    );
}

#[test]
fn seeing_a_center_does_not_reveal_an_unseen_oppidum() {
    let (g, city, enc, _) = fixture();
    let visible = BTreeSet::from([g.cities[&city].pos, enc]);
    let memory = g.remember_city_for_viewer(&g.cities[&city], 1, &visible);
    assert!(memory.defending_districts.is_empty());
}

#[test]
fn seeing_a_center_does_not_refresh_hidden_fort_health() {
    let (mut g, city, _, opp) = fixture();
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.hp = 77;
    g.set_defending_district_state(city, state);
    let seen = BTreeSet::from([g.cities[&city].pos, opp]);
    let memory = g.remember_city_for_viewer(&g.cities[&city], 1, &seen);
    g.players[1].remembered_cities.insert(city, memory);
    state.hp = 21;
    g.set_defending_district_state(city, state);
    let center_only = BTreeSet::from([g.cities[&city].pos]);
    let memory = g.remember_city_for_viewer(&g.cities[&city], 1, &center_only);
    assert_eq!(memory.defending_districts[0].hp, 77);
}

#[test]
fn repairing_a_pillaged_oppidum_restores_only_that_fort() {
    let (mut g, city, enc, opp) = fixture();
    let before = g.defending_district_state(city, enc).unwrap();
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.hp = 0;
    state.wall_hp = 0;
    state.pillaged = true;
    g.set_defending_district_state(city, state);
    g.map.tiles.get_mut(&opp).unwrap().pillaged = true;
    let repair = Item::Repair {
        repair: crate::name!("district"),
        pos: opp,
    };
    assert!(g.can_produce(0, city, &repair));
    assert!(g.complete_item(0, city, &repair));
    assert_eq!(g.defending_district_state(city, opp).unwrap().hp, 100);
    assert!(g.defending_district_can_strike(
        &g.cities[&city],
        &g.defending_district_state(city, opp).unwrap()
    ));
    assert_eq!(g.defending_district_state(city, enc), Some(before));
}

#[test]
fn capturing_a_city_removes_fort_state_when_the_roster_converts_to_an_industrial_zone() {
    let (mut g, city, _, opp) = fixture();
    g.capture_city(city, 1);
    assert_eq!(
        g.district_family(g.map.tiles[&opp].district.unwrap()),
        crate::name!("industrial_zone")
    );
    assert!(!g.cities[&city]
        .defending_districts
        .iter()
        .any(|state| state.pos == opp));
    assert_eq!(g.defending_district_at(opp), None);
}

#[test]
fn the_source_of_a_district_strike_is_in_its_wire_action() {
    let action = Action::DistrictStrike {
        city: 4,
        source: (1, -2),
        target: (3, -1),
    };
    let wire = serde_json::to_value(&action).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({"type":"district_strike","city":4,"source":[1,-2],"target":[3,-1]})
    );
    let roundtrip: Action = serde_json::from_value(wire).unwrap();
    assert_eq!(
        serde_json::to_value(roundtrip).unwrap(),
        serde_json::to_value(action).unwrap()
    );
}

#[test]
fn district_strike_rejects_another_citys_source_and_a_pillaged_source() {
    let (mut g, city, _, opp) = fixture();
    let target = (opp.0 + 1, opp.1);
    g.spawn_test_unit("warrior", 1, target);
    let other = g.found_city_for(0, crate::hex::offset_to_axial(3, 3), None);
    assert!(g
        .apply(
            0,
            &Action::DistrictStrike {
                city: other,
                source: opp,
                target
            }
        )
        .is_err());
    g.map.tiles.get_mut(&opp).unwrap().pillaged = true;
    assert!(g
        .apply(
            0,
            &Action::DistrictStrike {
                city,
                source: opp,
                target
            }
        )
        .is_err());
}
