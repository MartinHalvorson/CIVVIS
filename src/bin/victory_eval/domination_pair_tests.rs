use super::*;

fn args(extra: &[&str]) -> Vec<String> {
    [
        "--domination-pair",
        "siege-is-progress-3",
        "--out",
        "/tmp/probe.jsonl",
    ]
    .into_iter()
    .chain(extra.iter().copied())
    .map(str::to_string)
    .collect()
}

#[test]
fn explicit_profile_overrides_and_unknown_policies_are_rejected() {
    for extra in [
        vec!["--target", "science"],
        vec!["--players", "6"],
        vec!["--games", "0"],
        vec!["--games"],
        vec!["--games", "no"],
        vec!["--games", "2", "--games", "3"],
        vec!["--start-seed", "18446744073709551615", "--games", "2"],
    ] {
        assert!(parse(&args(&extra)).is_err(), "{extra:?}");
    }
    let mut unknown = args(&[]);
    unknown[1] = "missing-policy".to_string();
    assert!(parse(&unknown).is_err());
    assert!(parse(&args(&[])[..2]).is_err());
    let valid = parse(&args(&["--games", "2", "--start-seed", "99"])).unwrap();
    assert_eq!((valid.games, valid.start_seed), (2, 99));
}

#[test]
fn focal_identity_and_handicap_match_the_fixed_king_profile() {
    let game = Game::new_with(options(37140000));
    assert_eq!(game.players[0].civ, "Gran Colombia");
    assert_eq!((game.map.width, game.map.height), (60, 38));
    assert!(game.is_handicap_exempt(0));
    assert_eq!(game.handicap_yield_pct(0).science, 0.0);
    for pid in 1..4 {
        assert!(!game.is_handicap_exempt(pid));
        assert!(game.handicap_yield_pct(pid).science > 0.0);
    }
    let p = profile(37140000);
    assert_eq!(p["players"], 4);
    assert_eq!(
        (p["width"].as_i64(), p["height"].as_i64()),
        (Some(60), Some(38))
    );
    assert_eq!(p["map"], "Pangaea");
    assert_eq!(p["difficulty"], "king");
    assert_eq!(p["barbarian_difficulty"], "emperor");
    assert_eq!(game.barbarian_difficulty, "emperor");
    assert_eq!(p["speed"], "online");
    assert_eq!(p["max_turns"], 250);
    assert_eq!(p["city_states"], 6);
    assert_eq!(p["barbarians"], true);
    assert!(p["mercy_rule"].is_null());
    assert_eq!(p["required_victory_types"], 1);
    assert!(p["victories"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v == true));
}

#[test]
fn same_seed_seats_and_world_are_identical_before_policy_application() {
    let off = Game::new_with(options(37140001));
    let on = Game::new_with(options(37140001));
    assert_eq!(
        serde_json::to_vec(&off).unwrap(),
        serde_json::to_vec(&on).unwrap()
    );
}

#[test]
fn only_the_focal_seat_receives_the_policy_callback() {
    // Give the callback a visible effect so the test verifies which seat is
    // mutated without depending on private treatment fields.
    let policy = Gene {
        tag: "test-seat-selection",
        field: "test-seat-selection",
        kind: civvis::ai::Kind::OptIn,
        enable: |ai| *ai = AdvancedAi::targeting(VictoryTarget::Science),
        disable: |ai| *ai = AdvancedAi::targeting(VictoryTarget::Domination),
    };
    let game = Game::new_with(options(37140002));
    let off = fleet(&game, &policy, false);
    let on = fleet(&game, &policy, true);
    assert_eq!(off[0].victory_target(), Some(VictoryTarget::Domination));
    assert_eq!(on[0].victory_target(), Some(VictoryTarget::Science));
    for pid in 1..off.len() {
        assert_eq!(off[pid].victory_target(), None);
        assert_eq!(on[pid].victory_target(), None);
    }
}

#[test]
fn real_policy_keeps_domination_and_adaptive_rival_assignments() {
    let game = Game::new_with(options(37140003));
    for enabled in [false, true] {
        let ais = fleet(&game, gene("siege-is-progress-3").unwrap(), enabled);
        assert_eq!(ais[0].victory_target(), Some(VictoryTarget::Domination));
        assert!(ais[1..].iter().all(|ai| ai.victory_target().is_none()));
    }
}

#[test]
fn an_existing_result_file_is_preserved_before_any_game_runs() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "domination-pair-{}-{stamp}.jsonl",
        std::process::id()
    ));
    std::fs::write(&path, "previous evidence\n").unwrap();
    let mut input = args(&["--games", "1"]);
    input[3] = path.to_string_lossy().into_owned();
    let result = run(&input);
    let contents = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(result.is_err());
    assert_eq!(contents, "previous evidence\n");
}
