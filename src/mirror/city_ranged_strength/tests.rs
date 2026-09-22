use super::*;
use crate::game::Game;

fn city(name: &str, x: i32, strength: Option<f64>) -> StateCity {
    StateCity {
        name: name.into(),
        x,
        y: 8,
        pop: 5,
        defense: 75.0,
        ranged_strength: strength,
        damage: 0.0,
        max_damage: 200.0,
        wall_damage: 0.0,
        max_wall_damage: 400.0,
        loyalty: 100.0,
        ..Default::default()
    }
}

fn fixture() -> (Snapshot, StateSnapshot) {
    let plots = (0..26)
        .flat_map(|x| {
            (0..18).map(move |y| {
                serde_json::from_value(serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","o":-1}))
                    .unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 128,
        width: 26,
        height: 18,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 128,
        seat: Seat {
            players: 2,
            local_player: 0,
            ..Default::default()
        },
        cities: vec![city("Home", 3, Some(40.0))],
        rivals: vec![StateRival {
            player: 1,
            cities: vec![city("Miskolc", 13, Some(60.0))],
            ..Default::default()
        }],
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_KABUL".into(),
            cities: vec![city("Kabul", 21, Some(50.0))],
            ..Default::default()
        }],
        ..Default::default()
    };
    (snapshot, state)
}

fn named(game: &Game, name: &str) -> u32 {
    game.cities.values().find(|c| c.name == name).unwrap().id
}

#[test]
fn fresh_import_uses_native_fire_for_owned_rival_and_minor_cities() {
    let (snapshot, state) = fixture();
    let game = LiveMirror::new(&snapshot, &state, 2, 374200, 250, 0).game;
    for (name, expected) in [("Home", 40.0), ("Miskolc", 60.0), ("Kabul", 50.0)] {
        let id = named(&game, name);
        assert_eq!(game.city_ranged_strength(id), expected);
        assert_eq!(game.city_strength(id), 75.0, "melee defense is independent");
        assert_eq!(game.cities[&id].wall_hp, 400);
    }
    assert!(
        game.units.values().all(|u| u.owner == 0),
        "no visible enemy units are needed to measure city fire"
    );
}

#[test]
fn sync_replaces_observations_and_drops_unknown_or_invalid_values() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 374201, 250, 0);
    for value in [
        Some(65.0),
        Some(0.0),
        None,
        Some(-1.0),
        Some(f64::NAN),
        Some(f64::INFINITY),
    ] {
        state.rivals[0].cities[0].ranged_strength = value;
        mirror.sync(&snapshot, &state, 0);
        let id = named(&mirror.game, "Miskolc");
        let valid = value.filter(|v| v.is_finite() && *v >= 0.0);
        assert_eq!(
            mirror.game.observed_city_ranged_strength.get(&id).copied(),
            valid
        );
        assert_eq!(mirror.game.city_ranged_strength(id), valid.unwrap_or(3.0));
    }
}

#[test]
fn native_total_does_not_add_model_history_or_policy_twice() {
    let (snapshot, state) = fixture();
    let mut game = LiveMirror::new(&snapshot, &state, 2, 374202, 250, 0).game;
    let id = named(&game, "Miskolc");
    let owner = game.cities[&id].owner;
    game.players[owner]
        .counters
        .insert("strongest_ranged_built".into(), 100);
    game.players[owner]
        .policies
        .insert(crate::name!("bastions"));
    assert_eq!(game.policy_effect(owner, "city_ranged"), 5.0);
    assert_eq!(game.city_ranged_strength(id), 60.0);
    Arc::make_mut(&mut game.observed_city_ranged_strength).remove(&id);
    assert_eq!(
        game.city_ranged_strength(id),
        105.0,
        "legacy exports retain the historical model"
    );
}

#[test]
fn clone_save_and_legacy_load_preserve_observation_semantics() {
    let (snapshot, state) = fixture();
    let game = LiveMirror::new(&snapshot, &state, 2, 374203, 250, 0).game;
    let id = named(&game, "Miskolc");
    let mut clone = game.clone();
    Arc::make_mut(&mut clone.observed_city_ranged_strength).insert(id, 80.0);
    assert_eq!(game.city_ranged_strength(id), 60.0);
    let decoded: Game = serde_json::from_str(&serde_json::to_string(&clone).unwrap()).unwrap();
    assert_eq!(decoded.city_ranged_strength(id), 80.0);
    let mut legacy = serde_json::to_value(&decoded).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("observed_city_ranged_strength");
    let decoded: Game = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.city_ranged_strength(id), 3.0);
    let old_city: StateCity =
        serde_json::from_value(serde_json::json!({"x":13,"y":8,"defense":75})).unwrap();
    assert_eq!(old_city.ranged_strength, None);
}

#[test]
fn removing_or_resetting_mirror_cities_removes_fire_observations() {
    let (snapshot, state) = fixture();
    let mut game = LiveMirror::new(&snapshot, &state, 2, 374204, 250, 0).game;
    let id = named(&game, "Miskolc");
    game.mirror_remove_city(id);
    assert!(!game.observed_city_ranged_strength.contains_key(&id));
    assert!(!game.observed_city_ranged_strength.is_empty());
    game.clear_mirror_cities();
    assert!(game.observed_city_ranged_strength.is_empty());
}

#[test]
fn imported_fire_changes_the_simulated_hit_on_a_siege_unit() {
    let (snapshot, state) = fixture();
    let mut game = LiveMirror::new(&snapshot, &state, 2, 374205, 250, 0).game;
    let cid = named(&game, "Miskolc");
    let owner = game.cities[&cid].owner;
    let target = crate::hex::offset_to_axial(12, 8);
    let unit = game.spawn_test_unit("trebuchet", 0, target);
    game.spawn_test_unit("warrior", owner, game.cities[&cid].pos);
    game.at_war.insert((0, owner));
    game.current = owner;
    let mut legacy = game.clone();
    Arc::make_mut(&mut legacy.observed_city_ranged_strength).clear();
    let action = crate::game::Action::CityStrike { city: cid, target };
    legacy.apply(owner, &action).unwrap();
    game.apply(owner, &action).unwrap();
    let hp = |g: &Game| g.units.get(&unit).map_or(0, |u| u.hp);
    assert!(
        hp(&legacy) > hp(&game) + 30,
        "the same roll must price native fire instead of strength three"
    );
}
