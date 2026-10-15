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

/// What `/policy-effects` prints adds up: each measure's card tenths sum to
/// the net's. Rounded one by one, 0.25 and 0.25 printed 0.3 and 0.3 under a
/// deck of 0.5, and 11.94, 0.02 and 0.03 printed 11.9 under 12.0.
#[test]
fn printed_policy_shares_add_up_to_the_printed_net() {
    assert_eq!(tenths_summing_to(&[0.25, 0.25], 5), vec![3, 2]);
    assert_eq!(
        tenths_summing_to(&[11.94, 0.02, 0.03], 120),
        vec![120, 0, 0]
    );
    assert_eq!(tenths_summing_to(&[1.62, 0.33, 0.0], 20), vec![16, 4, 0]);
    assert_eq!(tenths_summing_to(&[-0.25, -0.25], -5), vec![-2, -3]);
    assert_eq!(tenths_summing_to(&[1.0, -0.36, 0.0], 6), vec![10, -4, 0]);
    assert_eq!(tenths_summing_to(&[], 0), Vec::<i64>::new());

    use crate::game::{EmpireReading, PolicyLedger, PolicyWorth};
    let card = |policy: &str, faith: f64, gold: f64| PolicyWorth {
        policy: crate::name::Name::new(policy),
        worth: EmpireReading {
            faith,
            gold,
            ..EmpireReading::default()
        },
    };
    let ledger = PolicyLedger {
        anarchy: false,
        with: EmpireReading::default(),
        net: EmpireReading {
            faith: 0.7,
            gold: 11.98,
            ..EmpireReading::default()
        },
        cards: vec![
            card("civil_prestige", 0.35, 1.93),
            card("retainers", 0.35, 10.05),
        ],
    };
    let answer = policy_effects_answer(84, 0, &ledger);
    assert_eq!(answer["net"], json!({"faith": 0.7, "gold": 12.0}));
    assert_eq!(
        answer["cards"][0]["worth"],
        json!({"faith": 0.4, "gold": 1.9}),
        "{answer}"
    );
    assert_eq!(
        answer["cards"][1]["worth"],
        json!({"faith": 0.3, "gold": 10.1}),
        "{answer}"
    );
}
