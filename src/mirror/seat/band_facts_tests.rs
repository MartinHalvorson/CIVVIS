use super::*;

fn fixture(kind: &str) -> (crate::game::Game, u32, StateUnit) {
    let mut game = crate::game::Game::new_full(2, 20, 14, 475_300, 300, 0, false);
    let pos = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(tile.pos).is_empty()
        })
        .map(|tile| tile.pos)
        .unwrap();
    let uid = game.spawn_test_unit(kind, 0, pos);
    let state = StateUnit {
        id: 77,
        kind: if kind == "rock_band" {
            "UNIT_ROCK_BAND"
        } else {
            "UNIT_WARRIOR"
        }
        .into(),
        hp: 100.0,
        xp: Some(77),
        level: Some(3),
        rock_band_level: Some(1),
        album_sales: Some(200),
        ..Default::default()
    };
    (game, uid, state)
}

fn observe(game: &mut crate::game::Game, uid: u32, state: &StateUnit) {
    record_host_unit_facts(game, uid, state);
    let progress = observed_unit_progress(&game.rules, state, &mut Vec::new());
    apply_unit_observation(game.units.get_mut(&uid).unwrap(), state, progress);
}

#[test]
fn native_band_performance_does_not_replace_its_promotion_level() {
    let (mut game, uid, state) = fixture("rock_band");
    observe(&mut game, uid, &state);
    let band = &game.units[&uid];
    assert_eq!(band.level, 3);
    assert_eq!(band.xp, 77);
    assert_eq!(band.album_sales, 200);
    assert_eq!(game.rock_band_performance_level(band, 0), 1);
    assert_eq!(game.rock_band_performance_level(band, 2), 3);
    assert_eq!(game.rock_band_performance_level(band, 10), 6);
}

#[test]
fn old_and_unreadable_band_facts_keep_legacy_level_and_known_sales() {
    let (mut game, uid, mut state) = fixture("rock_band");
    observe(&mut game, uid, &state);
    for absent_or_bad in [None, Some(-1)] {
        state.rock_band_level = absent_or_bad;
        state.album_sales = absent_or_bad.map(i64::from);
        observe(&mut game, uid, &state);
        assert_eq!(game.units[&uid].album_sales, 200);
        assert_eq!(game.rock_band_performance_level(&game.units[&uid], 0), 3);
    }
    let old: StateUnit =
        serde_json::from_str(r#"{"id":77,"kind":"UNIT_ROCK_BAND","x":0,"y":0,"level":3}"#).unwrap();
    assert_eq!(old.rock_band_level, None);
    assert_eq!(old.album_sales, None);
}

#[test]
fn native_zero_counters_are_preserved_and_other_units_ignore_band_facts() {
    let (mut game, uid, mut state) = fixture("rock_band");
    game.units.get_mut(&uid).unwrap().album_sales = 350;
    state.rock_band_level = Some(0);
    state.album_sales = Some(0);
    observe(&mut game, uid, &state);
    assert_eq!(game.units[&uid].album_sales, 0);
    assert_eq!(game.host_unit_facts[&uid].rock_band_level, Some(0));
    assert_eq!(game.rock_band_performance_level(&game.units[&uid], 0), 1);

    let (mut game, uid, state) = fixture("warrior");
    observe(&mut game, uid, &state);
    assert_eq!(game.units[&uid].level, 3);
    assert_eq!(game.units[&uid].album_sales, 0);
    assert_eq!(game.host_unit_facts[&uid].rock_band_level, None);
}

#[test]
fn band_fields_are_recognized_by_the_live_schema() {
    let snapshot = state_from_json(
        r#"{
        "kind":"state","turn":7,
        "units":[{"id":77,"kind":"UNIT_ROCK_BAND","x":0,"y":0,
            "level":3,"rock_band_level":1,"album_sales":200}]
    }"#,
    )
    .unwrap();
    assert!(
        snapshot.schema_gaps.is_empty(),
        "{:?}",
        snapshot.schema_gaps
    );
    assert_eq!(snapshot.units[0].level, Some(3));
    assert_eq!(snapshot.units[0].rock_band_level, Some(1));
    assert_eq!(snapshot.units[0].album_sales, Some(200));
}
