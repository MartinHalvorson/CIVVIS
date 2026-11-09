use civvis::game::Game;
use civvis::mirror::{
    LiveMirror, Seat, Snapshot, StateCity, StateMinor, StatePublicEmpireStats, StateRival,
    StateSnapshot, TilesChunk,
};

fn city(name: &str, x: i32, faith: Option<&str>, observed: Option<bool>) -> StateCity {
    StateCity {
        name: name.into(),
        x,
        y: 8,
        pop: 5,
        religion: faith.map(str::to_string),
        religion_observed: observed,
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
        turn: 50,
        width: 26,
        height: 18,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 50,
        seat: Seat {
            players: 2,
            local_player: 0,
            ..Default::default()
        },
        public_stats: StatePublicEmpireStats {
            city_count: Some(1),
            ..Default::default()
        },
        cities: vec![city("Home", 3, Some("RELIGION_ISLAM"), None)],
        rivals: vec![StateRival {
            player: 1,
            cities: vec![city("Foreign", 13, Some("RELIGION_BUDDHISM"), Some(true))],
            ..Default::default()
        }],
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_KUMASI".into(),
            cities: vec![city("Kumasi", 21, Some("RELIGION_BUDDHISM"), Some(true))],
            ..Default::default()
        }],
        ..Default::default()
    };
    (snapshot, state)
}

fn named(game: &Game, name: &str) -> u32 {
    game.cities
        .values()
        .find(|city| city.name == name)
        .unwrap()
        .id
}

#[test]
fn revealed_foreign_cities_import_the_visible_majority_for_majors_and_minors() {
    let (snapshot, state) = fixture();
    let mirror = LiveMirror::new(&snapshot, &state, 2, 936_521, 250, 0);
    for (name, faith) in [
        ("Home", "Islam"),
        ("Foreign", "Buddhism"),
        ("Kumasi", "Buddhism"),
    ] {
        let cid = named(&mirror.game, name);
        assert_eq!(
            mirror.game.city_religion(&mirror.game.cities[&cid]),
            Some(faith)
        );
        assert!(mirror.game.observed_city_religion_markers.contains(&cid));
    }
}

#[test]
fn sync_replaces_a_foreign_majority_and_can_confirm_no_majority() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 936_522, 250, 0);
    state.rivals[0].cities[0].religion = Some("RELIGION_ISLAM".into());
    mirror.sync(&snapshot, &state, 0);
    let cid = named(&mirror.game, "Foreign");
    assert_eq!(
        mirror.game.city_religion(&mirror.game.cities[&cid]),
        Some("Islam")
    );
    state.rivals[0].cities[0].religion = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.city_religion(&mirror.game.cities[&cid]), None);
    assert!(mirror.game.cities[&cid].pressure.is_empty());
    assert!(
        mirror.game.observed_city_religion_markers.contains(&cid),
        "no synthetic spread arithmetic on host markers"
    );
}

#[test]
fn fog_and_legacy_exports_do_not_assert_a_conversion() {
    for observed in [None, Some(false)] {
        let (snapshot, mut state) = fixture();
        state.rivals[0].cities[0].religion_observed = observed;
        state.minors[0].cities[0].religion_observed = observed;
        let mirror = LiveMirror::new(&snapshot, &state, 2, 936_523, 250, 0);
        for name in ["Foreign", "Kumasi"] {
            let cid = named(&mirror.game, name);
            assert_eq!(mirror.game.city_religion(&mirror.game.cities[&cid]), None);
            assert!(!mirror.game.observed_city_religion_markers.contains(&cid));
        }
    }
}

#[test]
fn a_missing_or_hidden_read_does_not_clear_the_last_observed_majority() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 936_524, 250, 0);
    for observed in [Some(false), None] {
        state.rivals[0].cities[0].religion_observed = observed;
        state.rivals[0].cities[0].religion = None;
        mirror.sync(&snapshot, &state, 0);
        let cid = named(&mirror.game, "Foreign");
        assert_eq!(
            mirror.game.city_religion(&mirror.game.cities[&cid]),
            Some("Buddhism")
        );
    }
}

#[test]
fn a_native_empire_without_a_majority_verdict_is_not_inferred_from_city_memory() {
    for total in [None, Some(0), Some(1), Some(2), Some(7)] {
        let (snapshot, mut state) = fixture();
        state.rivals[0].public_stats.city_count = total;
        let mirror = LiveMirror::new(&snapshot, &state, 2, 936_525, 250, 0);
        let cid = named(&mirror.game, "Foreign");
        assert_eq!(
            mirror.game.city_religion(&mirror.game.cities[&cid]),
            Some("Buddhism")
        );
        let owner = mirror.game.cities[&cid].owner;
        assert!(
            !mirror.game.civ_follows_religion(owner, "Buddhism"),
            "native majority must not come from a possibly stale city subset (total {total:?})"
        );
    }
}

#[test]
fn the_hosts_empire_majority_overrules_a_visible_city_subset() {
    let (snapshot, mut state) = fixture();
    state.rivals[0].public_stats.city_count = Some(7);
    state.rivals[0].religion = Some("RELIGION_ISLAM".into());
    let mirror = LiveMirror::new(&snapshot, &state, 2, 936_526, 250, 0);
    let cid = named(&mirror.game, "Foreign");
    let owner = mirror.game.cities[&cid].owner;
    assert_eq!(
        mirror.game.city_religion(&mirror.game.cities[&cid]),
        Some("Buddhism")
    );
    assert!(mirror.game.civ_follows_religion(owner, "Islam"));
    assert!(!mirror.game.civ_follows_religion(owner, "Buddhism"));
}

#[test]
fn our_complete_owned_city_list_supplies_our_current_majority() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 936_527, 250, 0);
    assert!(mirror.game.civ_follows_religion(0, "Islam"));
    state.cities[0].religion = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.majority_religion_of(0), None);
}
