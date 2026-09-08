use super::*;
use crate::game::{LeaderPool, VictoryConditions};
use crate::server::Params;
use crate::setup::{
    BaseRuleset, FutureEra, GameSpeed, MapPoles, MapScript, MapTopology, TacticsRules,
    TurnStructure,
};

fn params() -> Params {
    Params {
        map_topology: MapTopology::Flat,
        map_poles: MapPoles::Poles,
        mercy_rule: None,
        required_victory_types: 1,
        tactics: TacticsRules::default(),
        base_ruleset: BaseRuleset::Civ6,
        start_era: 0,
        future_era: FutureEra::Classic,
        turn_structure: TurnStructure::Sequential,
        num_players: 2,
        width: 20,
        height: 14,
        seed: 1,
        map_script: MapScript::Pangaea,
        game_speed: GameSpeed::Standard,
        max_turns: 500,
        victory_conditions: VictoryConditions::default(),
        num_city_states: 1,
        spectate: false,
        difficulty: crate::game::default_difficulty(),
        speed: crate::game::default_speed(),
        teams: Vec::new(),
        leader_pool: LeaderPool::Civ6,
        civs: Vec::new(),
        supervised: false,
    }
}

#[test]
fn mirror_load_never_publishes_an_unseated_frame() {
    let mut session = Session::new(params());
    let game = serde_json::to_value(&session.game).unwrap();
    session.set_view_player(Some(0)).unwrap();
    session.set_spectator_paused(true);
    let expected = session.state();
    for _ in 0..3 {
        load_uploaded(&mut session, &json!({"game": game, "mirror_player": 0})).unwrap();
        let frame = session.state();
        assert_eq!(frame["view_player"], 0);
        assert_eq!(frame["spectator_paused"], true);
        assert!(session.params.spectate);
        assert_eq!(frame["players"], expected["players"]);
        assert_eq!(frame["map"], expected["map"]);
        assert_eq!(frame["visible"], expected["visible"]);
    }
}

#[test]
fn invalid_mirror_seat_keeps_the_previous_frame() {
    let mut session = Session::new(params());
    session.set_view_player(Some(0)).unwrap();
    session.set_spectator_paused(true);
    let before = session.state();
    let game = serde_json::to_value(&session.game).unwrap();
    for seat in [json!(999), json!(-1), Value::Null, json!("0")] {
        assert!(
            load_uploaded(&mut session, &json!({"game": game, "mirror_player": seat})).is_err()
        );
        assert_eq!(session.state(), before);
    }
}

#[test]
fn ordinary_save_load_keeps_its_existing_behavior() {
    let mut session = Session::new(params());
    let game = serde_json::to_value(&session.game).unwrap();
    load_uploaded(&mut session, &json!({"game": game})).unwrap();
    assert!(!session.params.spectate);
}
