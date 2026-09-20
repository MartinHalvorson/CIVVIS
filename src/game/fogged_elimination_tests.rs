use super::*;
use crate::mirror::{
    rebuild_from_state, state_from_json, LiveMirror, Snapshot, StateSnapshot, TilesChunk,
};

fn board() -> Game {
    let mut g = Game::new_full(2, 40, 24, 3626, 400, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(1, (24, 12), None);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.turn = 100;
    g.current = 0;
    g.spawn_test_unit("archer", 0, (6, 12));
    let victim = g.spawn_test_unit("warrior", 1, (7, 12));
    g.units.get_mut(&victim).unwrap().hp = 1;
    g.spawn_test_unit("archer", 1, (8, 12));
    g
}

fn kill_visible_warrior(g: &mut Game) {
    let attacker = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "archer")
        .unwrap();
    let victim = g
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| g.units[uid].kind == "warrior")
        .unwrap();
    let target = g.units[&victim].pos;
    g.apply(
        0,
        &Action::Ranged {
            unit: attacker,
            target,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&victim));
}

#[test]
fn killing_a_visible_unit_does_not_eliminate_a_rival_with_hidden_cities() {
    let g = board();
    let mut view = g.player_decision_view(0);
    assert!(view.player_city_ids(1).is_empty());
    let survivor = view
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| view.units[uid].kind == "archer")
        .unwrap();
    kill_visible_warrior(&mut view);
    assert!(view.players[1].alive);
    assert!(view.units.contains_key(&survivor));
    assert!(view.is_at_war(0, 1));
}

#[test]
fn a_second_view_preserves_the_public_city_count_and_hidden_city_survival() {
    let view = board().player_decision_view(0);
    let mut again = view.player_decision_view(0);
    assert_eq!(again.observed_public_empire_stats[&1].city_count, Some(1));
    kill_visible_warrior(&mut again);
    assert!(again.players[1].alive);
}

#[test]
fn hidden_city_knowledge_survives_serialization() {
    let view = board().player_decision_view(0);
    let encoded = serde_json::to_string(&view).unwrap();
    let mut decoded: Game = serde_json::from_str(&encoded).unwrap();
    kill_visible_warrior(&mut decoded);
    assert!(decoded.players[1].alive);
}

#[test]
fn taking_the_last_visible_city_does_not_erase_a_hidden_city() {
    let mut g = board();
    let visible = g.found_city_for(1, (9, 12), None);
    g.spawn_test_unit("warrior", 0, (8, 13));
    let mut view = g.player_decision_view(0);
    assert_eq!(view.player_city_ids(1), vec![visible]);
    view.capture_city(visible, 0);
    view.do_keep_city(0, visible).unwrap();
    assert!(view.players[1].alive);
}

#[test]
fn taking_the_actual_last_city_still_eliminates_the_rival() {
    let mut g = board();
    let city = g.player_city_ids(1)[0];
    g.spawn_test_unit("warrior", 0, (23, 12));
    let mut view = g.player_decision_view(0);
    assert_eq!(view.player_city_ids(1), vec![city]);
    view.capture_city(city, 0);
    view.do_keep_city(0, city).unwrap();
    assert!(!view.players[1].alive);
}

#[test]
fn complete_boards_keep_their_existing_elimination_behavior() {
    let mut g = board();
    kill_visible_warrior(&mut g);
    assert!(g.players[1].alive);
    let city = g.player_city_ids(1)[0];
    g.capture_city(city, 0);
    g.do_keep_city(0, city).unwrap();
    assert!(!g.players[1].alive);
}

fn host_board() -> (Snapshot, StateSnapshot) {
    let state = state_from_json(r#"{
        "turn":105,
        "cities":[{"id":1,"name":"Home","x":6,"y":6,"pop":3}],
        "units":[{"id":10,"kind":"UNIT_ARCHER","x":6,"y":6,"hp":100,"moves":2,"attacks_remaining":1}],
        "rivals":[{"player":1,"at_war":true,"public_stats":{"city_count":6},
            "units":[{"id":20,"kind":"UNIT_WARRIOR","x":7,"y":6,"hp":1,"moves":2},
                     {"id":21,"kind":"UNIT_ARCHER","x":8,"y":6,"hp":100,"moves":2}]}]
    }"#).unwrap();
    let plots = (0..14)
        .flat_map(|y| {
            (0..20).map(move |x| {
                serde_json::from_value(
                    serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true}),
                )
                .unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 105,
        width: 20,
        height: 14,
        chunk: 1,
        plots,
    }]);
    (snapshot, state)
}

#[test]
fn the_host_city_total_protects_a_rival_whose_cities_are_all_unrevealed() {
    let (snapshot, state) = host_board();
    let mut g = rebuild_from_state(&snapshot, &state, 2, 3626, 400, 0).game;
    assert!(g.player_city_ids(1).is_empty());
    assert_eq!(g.observed_public_empire_stats[&1].city_count, Some(6));
    kill_visible_warrior(&mut g);
    assert!(g.players[1].alive);
}

#[test]
fn authoritative_sync_clears_hidden_cities_once_all_are_represented() {
    let (snapshot, mut state) = host_board();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 3626, 400, 0);
    kill_visible_warrior(&mut mirror.game);
    assert!(mirror.game.players[1].alive);
    state.rivals[0].cities = vec![serde_json::from_value(serde_json::json!({
        "id":2,"name":"Revealed","x":10,"y":6,"pop":3
    }))
    .unwrap()];
    state.rivals[0].public_stats.city_count = Some(1);
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.unseen_city_owners.is_empty());
    let city = mirror.game.player_city_ids(1)[0];
    mirror.game.capture_city(city, 0);
    mirror.game.do_keep_city(0, city).unwrap();
    assert!(!mirror.game.players[1].alive);
}

#[test]
fn razing_the_last_visible_city_preserves_an_unrevealed_city() {
    let mut g = board();
    let city = g.found_city_for(1, (9, 12), None);
    g.spawn_test_unit("warrior", 0, (8, 13));
    let mut view = g.player_decision_view(0);
    view.capture_city(city, 0);
    view.do_raze_city(0, city).unwrap();
    assert!(view.players[1].alive);
}
