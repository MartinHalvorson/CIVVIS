use super::*;
use crate::game::{Action, ActionFamilies};

fn fixture(facts: serde_json::Value) -> (Snapshot, StateSnapshot) {
    let state = state_from_json(
        &serde_json::json!({
            "turn":53,
            "cities":[{"id":1,"name":"Bogotá","x":3,"y":3,"pop":5,"capital":true}],
            "rivals":[{
                "player":42,"civ":"CIVILIZATION_INCA","at_war":false,
                "can_declare":true,"diplomatic_state":"DIPLO_STATE_FRIENDLY",
                "our_denounce_turn":-1,"their_denounce_turn":-1,
                "war_declarations":facts,
                "cities":[{"id":1,"name":"Qusqu","x":8,"y":8,"pop":5,"capital":true}]
            }]
        })
        .to_string(),
    )
    .unwrap();
    assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
    let plots = [(3, 3), (8, 8)]
        .into_iter()
        .map(|(x, y)| {
            serde_json::from_value(serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS"})).unwrap()
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 53,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    (snapshot, state)
}

fn target(game: &crate::game::Game) -> usize {
    game.players.iter().find(|p| p.civ == "Inca").unwrap().id
}

fn formal(player: usize) -> Action {
    Action::DeclareWarWithCasusBelli {
        player,
        casus_belli: "formal_war".to_string(),
    }
}

#[test]
fn native_war_type_permissions_do_not_forge_a_formal_war_clock() {
    let (snapshot, state) = fixture(serde_json::json!([
        {"statement":"DECLARE_FORMAL_WAR","allowed":false},
        {"statement":"DECLARE_SURPRISE_WAR","allowed":true}
    ]));
    let mut rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game;
    let other = target(&rebuilt);
    assert_ne!(other, 42, "host target ids must be mapped");
    assert!(!rebuilt.players[0].denounced_until.contains_key(&other));
    let legal = rebuilt.legal_actions_within(0, ActionFamilies::DIPLOMACY);
    assert!(!legal.contains(&formal(other)));
    assert!(legal.contains(&Action::DeclareWar { player: other }));
    assert!(legal.contains(&Action::Denounce { player: other }));
    assert!(rebuilt.apply(0, &formal(other)).is_err());
    rebuilt
        .apply(0, &Action::DeclareWar { player: other })
        .unwrap();
    assert!(rebuilt.is_at_war(0, other));
}

#[test]
fn native_war_type_permissions_refresh_exact_types_on_same_turn_sync() {
    let (snapshot, state) = fixture(serde_json::json!([
        {"statement":"DECLARE_FORMAL_WAR","allowed":false},
        {"statement":"DECLARE_SURPRISE_WAR","allowed":true}
    ]));
    let mut live = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let other = target(&live.game);
    assert!(!live
        .game
        .legal_actions_within(0, ActionFamilies::DIPLOMACY)
        .contains(&formal(other)));
    let (_, opened) = fixture(serde_json::json!([
        {"statement":"DECLARE_FORMAL_WAR","allowed":true},
        {"statement":"DECLARE_SURPRISE_WAR","allowed":false}
    ]));
    live.sync(&snapshot, &opened, 0);
    let legal = live.game.legal_actions_within(0, ActionFamilies::DIPLOMACY);
    assert!(legal.contains(&formal(other)));
    assert!(!legal.contains(&Action::DeclareWar { player: other }));
    assert!(!live.game.players[0].denounced_until.contains_key(&other));
    assert!(live
        .game
        .apply(0, &Action::DeclareWar { player: other })
        .is_err());
    live.game.apply(0, &formal(other)).unwrap();
}

#[test]
fn native_war_type_permissions_unknown_clear_and_legacy_fallback() {
    let (snapshot, state) = fixture(serde_json::json!([
        {"statement":"DECLARE_FORMAL_WAR"},
        {"statement":"DECLARE_SURPRISE_WAR","allowed":false}
    ]));
    let mut live = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let other = target(&live.game);
    let legal = live.game.legal_actions_within(0, ActionFamilies::DIPLOMACY);
    assert!(!legal.iter().any(|a| matches!(
        a,
        Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
    )));
    let (_, mut legacy) = fixture(serde_json::Value::Null);
    live.sync(&snapshot, &legacy, 0);
    assert!(live.game.host_war_type_permissions.is_empty());
    assert!(live
        .game
        .legal_actions_within(0, ActionFamilies::DIPLOMACY)
        .contains(&formal(other)));
    live.sync(&snapshot, &state, 0);
    legacy.rivals.clear();
    live.sync(&snapshot, &legacy, 0);
    assert!(live.game.host_war_type_permissions.is_empty());
}
