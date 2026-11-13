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

/// The players of a frame without who is said to be playing them. A mirrored
/// frame names its seats for the Civilization VI game it repaints, which is
/// the one thing it does not share with the board it was rebuilt from.
fn without_seat_names(players: &Value) -> Value {
    let mut players = players.clone();
    for player in players.as_array_mut().unwrap() {
        let player = player.as_object_mut().unwrap();
        for key in ["ai_name", "ai_username", "ai_strat_label"] {
            player.remove(key);
        }
    }
    players
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
        assert_eq!(
            without_seat_names(&frame["players"]),
            without_seat_names(&expected["players"])
        );
        assert_eq!(frame["map"], expected["map"]);
        assert_eq!(frame["visible"], expected["visible"]);
    }
}

/// A mirrored game is Civilization VI's table, not ours. The live mirror was
/// dealing its rivals our top-rated site agents, so a Deity game read
/// SwingVote 1192, JackOfAllTrades 1211 and TouristTrap 1178 beside a seat of
/// ours called StrategicArchitect (2026-11-13). Operator, the same day: "for
/// validation games, the player should just be something like "Civ 6 Deity
/// AI" or whatever the level is. for self-play games where we control the
/// player ai strategy, then we should list out the actual strategy being
/// used."
#[test]
fn a_mirrored_table_names_civilization_vis_ai_and_the_lane_we_play() {
    let mut setup = params();
    setup.num_players = 4;
    let mut session = Session::new(setup);
    // The board `mirror::rebuild_from_state` hands over: the host's level,
    // our seat in the human slot, and rivals the seat has met.
    session.game.difficulty = "deity".to_string();
    assert!(session.game.human_seats.contains(&0));
    for rival in 1..4 {
        session.game.players[0].met.insert(rival);
    }
    let game = serde_json::to_value(&session.game).unwrap();
    let majors = |state: &Value| -> Vec<Value> {
        state["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["is_minor"] != json!(true) && p["is_barbarian"] != json!(true))
            .cloned()
            .collect()
    };

    load_uploaded(
        &mut session,
        &json!({"game": game, "mirror_player": 0, "mirror_strategy": "diplomatic"}),
    )
    .unwrap();
    let seats = majors(&session.state());
    assert_eq!(seats.len(), 4);
    for seat in &seats {
        if seat["id"] == json!(0) {
            assert_eq!(seat["ai_username"], json!("CIVVIS Diplomatic"));
        } else {
            assert_eq!(seat["ai_username"], json!("Civ 6 Deity AI"));
            assert_eq!(seat["ai_strat_label"], json!("Civilization VI's own AI"));
        }
        // Nobody at this table is one of our rated agents, and nothing in
        // this process is playing a seat for a name to be generated from.
        assert!(seat["ai_agent"].is_null(), "{seat}");
        assert!(seat["ai_elo"].is_null(), "{seat}");
        assert!(seat["ai_name"].is_null(), "{seat}");
    }

    // The level is the host's own, whatever it is.
    session.game.difficulty = "immortal".to_string();
    let game = serde_json::to_value(&session.game).unwrap();
    load_uploaded(
        &mut session,
        &json!({"game": game, "mirror_player": 0, "mirror_strategy": "civvis"}),
    )
    .unwrap();
    let seats = majors(&session.state());
    assert_eq!(seats[0]["ai_username"], json!("CIVVIS Adaptive"));
    assert_eq!(seats[1]["ai_username"], json!("Civ 6 Immortal AI"));

    // A follower that names no lane, or one the decider would refuse, still
    // publishes its frame: our seat is then our agent without a lane.
    for body in [
        json!({"game": game, "mirror_player": 0}),
        json!({"game": game, "mirror_player": 0, "mirror_strategy": "SwingVote"}),
        json!({"game": game, "mirror_player": 0, "mirror_strategy": 7}),
    ] {
        load_uploaded(&mut session, &body).unwrap();
        let seats = majors(&session.state());
        assert_eq!(seats[0]["ai_username"], json!("CIVVIS Advanced AI"));
        assert_eq!(seats[1]["ai_username"], json!("Civ 6 Immortal AI"));
    }

    // The same board as a world of our own is still dealt our agents: the
    // names above belong to the mirror, not to the save.
    let mut watched = params();
    watched.num_players = 4;
    watched.spectate = true;
    let mut watched = Session::new(watched);
    let mut save = session.game.clone();
    save.human_seats.clear();
    load_uploaded(
        &mut watched,
        &json!({"game": serde_json::to_value(&save).unwrap()}),
    )
    .unwrap();
    for seat in majors(&watched.state()) {
        let agent = seat["ai_agent"].as_str().expect("a rated agent");
        assert!(crate::strategies::find(agent).is_some());
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
