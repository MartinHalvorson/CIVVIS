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

#[test]
fn a_fogged_city_keeps_its_last_fire_across_a_fresh_board() {
    // Game 94: Porto read 60 while in sight and nothing on the turns it was
    // not; the board fell back to the 3 of a Builder and the siege invested.
    let (snapshot, mut state) = fixture();
    let mut memory = CityFireMemory::default();
    let mut seen = LiveMirror::new(&snapshot, &state, 2, 374206, 250, 0).game;
    memory.apply(&mut seen);
    assert_eq!(seen.city_ranged_strength(named(&seen, "Miskolc")), 60.0);

    state.turn += 1;
    state.rivals[0].cities[0].ranged_strength = None;
    let mut fogged = LiveMirror::new(&snapshot, &state, 2, 374207, 250, 0).game;
    let id = named(&fogged, "Miskolc");
    assert_eq!(
        fogged.city_ranged_strength(id),
        3.0,
        "the export alone reads a Builder's 3"
    );
    memory.apply(&mut fogged);
    assert_eq!(fogged.city_ranged_strength(id), 60.0);
    assert_eq!(
        fogged.city_ranged_strength(named(&fogged, "Home")),
        40.0,
        "our own cities keep the host's reading"
    );
}

#[test]
fn an_observed_reading_always_wins_over_the_memory() {
    let (snapshot, mut state) = fixture();
    let mut memory = CityFireMemory::default();
    memory.apply(&mut LiveMirror::new(&snapshot, &state, 2, 374212, 250, 0).game);
    state.turn += 1;
    state.rivals[0].cities[0].ranged_strength = Some(45.0);
    let mut game = LiveMirror::new(&snapshot, &state, 2, 374213, 250, 0).game;
    let id = named(&game, "Miskolc");
    let owner = game.cities[&id].owner;
    game.players[owner].techs.insert(crate::name!("ballistics"));
    memory.apply(&mut game);
    assert_eq!(
        game.city_ranged_strength(id),
        45.0,
        "neither the 60 seen nor the 60 researched"
    );
}

#[test]
fn a_city_that_changes_hands_drops_the_old_owner_s_fire() {
    let (snapshot, mut state) = fixture();
    state.seat.players = 3;
    state.rivals.push(StateRival {
        player: 2,
        cities: vec![city("Eger", 17, Some(25.0))],
        ..Default::default()
    });
    let mut memory = CityFireMemory::default();
    memory.apply(&mut LiveMirror::new(&snapshot, &state, 3, 374214, 250, 0).game);

    state.turn += 1;
    let mut taken = state.rivals[0].cities.remove(0);
    taken.ranged_strength = None;
    state.rivals[1].cities.push(taken);
    state.rivals[1].cities[0].ranged_strength = None;
    let mut game = LiveMirror::new(&snapshot, &state, 3, 374215, 250, 0).game;
    let id = named(&game, "Miskolc");
    let new_owner = game.cities[&id].owner;
    assert_eq!(new_owner, game.cities[&named(&game, "Eger")].owner);
    game.players[new_owner].techs.clear();
    memory.apply(&mut game);
    assert_eq!(
        game.city_ranged_strength(id),
        25.0,
        "the new owner's own reading, not the 60 Miskolc showed under its old one"
    );
}

#[test]
fn an_unknown_city_with_no_memory_strikes_with_what_its_seat_trains() {
    let (snapshot, mut state) = fixture();
    state.rivals[0].cities[0].ranged_strength = None;
    let mut game = LiveMirror::new(&snapshot, &state, 2, 374208, 250, 0).game;
    let id = named(&game, "Miskolc");
    let owner = game.cities[&id].owner;
    game.players[owner].techs.insert(crate::name!("ballistics"));
    // Robotics alone unlocks the Giant Death Robot, which needs Uranium.
    game.players[owner].techs.insert(crate::name!("robotics"));
    CityFireMemory::default().apply(&mut game);
    assert_eq!(game.city_ranged_strength(id), 60.0, "a Field Cannon's 60");
}

#[test]
fn a_fresher_reading_wins_and_research_floors_a_stale_one() {
    let (snapshot, mut state) = fixture();
    let mut memory = CityFireMemory::default();
    state.rivals[0].cities[0].ranged_strength = Some(25.0);
    memory.apply(&mut LiveMirror::new(&snapshot, &state, 2, 374209, 250, 0).game);

    state.turn += 5;
    state.rivals[0].cities[0].ranged_strength = None;
    let mut game = LiveMirror::new(&snapshot, &state, 2, 374210, 250, 0).game;
    let id = named(&game, "Miskolc");
    let owner = game.cities[&id].owner;
    game.players[owner].techs.insert(crate::name!("machinery"));
    memory.apply(&mut game);
    assert_eq!(
        game.city_ranged_strength(id),
        40.0,
        "a Crossbowman outranks a stale 25"
    );

    // A replay rewinds the timeline; what the later turns saw is forgotten.
    state.turn -= 3;
    let mut rewound = LiveMirror::new(&snapshot, &state, 2, 374211, 250, 0).game;
    let id = named(&rewound, "Miskolc");
    let owner = rewound.cities[&id].owner;
    rewound.players[owner].techs.clear();
    memory.apply(&mut rewound);
    assert_eq!(
        rewound.city_ranged_strength(id),
        15.0,
        "a Slinger needs no research"
    );
}
