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
        vec!["--profile", "unknown-profile"],
        vec!["--target", "science"],
        vec!["--difficulty", "unknown-rung"],
        vec!["--max-turns", "30"],
        vec!["--max-turns", "251"],
        vec!["--difficulty", "prince", "--difficulty", "king"],
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
    assert_eq!(valid.max_turns, 250, "the natural clock unless shortened");
    let short = parse(&args(&["--max-turns", "160"])).unwrap();
    assert_eq!(short.max_turns, 160);
    assert_eq!(
        options(1, None, short.max_turns, ProbeProfile::default()).max_turns,
        160
    );
}

#[test]
fn macedon_continents_profile_matches_the_level_eight_verification_seat() {
    let config = parse(&args(&["--profile", "macedon-continents"])).unwrap();
    assert_eq!(config.difficulty, Some("deity"));
    let setup = options(
        37_140_007,
        config.difficulty,
        config.max_turns,
        config.setup,
    );
    assert_eq!(
        (setup.players, setup.width, setup.height, setup.city_states),
        (6, 74, 46, 9)
    );
    assert_eq!(setup.map_script, MapScript::Continents);
    let game = Game::new_with(setup.clone());
    assert_eq!(game.players[0].civ, "Macedon");
    assert_eq!(game.barbarian_difficulty, "deity");
    assert_eq!(game.handicap_yield_pct(0).science, 0.0);
    for pid in 1..6 {
        assert_eq!(game.handicap_yield_pct(pid).science, 32.0);
        assert_eq!(game.handicap_combat_strength(pid), 4.0);
    }
    assert_eq!(
        serde_json::to_vec(&game).unwrap(),
        serde_json::to_vec(&Game::new_with(setup)).unwrap()
    );
    let report = profile(
        37_140_007,
        config.difficulty,
        config.max_turns,
        config.setup,
    );
    assert_eq!(report["players"], 6);
    assert_eq!(report["civs"], serde_json::json!(["Macedon"]));
    assert_eq!(report["map"], "Continents");
    assert_eq!(report["difficulty"], "deity");
    assert_eq!(report["speed"], "online");
    assert_eq!(report["max_turns"], 250);
    let override_level = parse(&args(&[
        "--profile",
        "macedon-continents",
        "--difficulty",
        "immortal",
    ]))
    .unwrap();
    assert_eq!(override_level.difficulty, Some("immortal"));
}

#[test]
fn supplied_bundle_is_validated_and_replaces_the_compiled_bundle() {
    let path = std::env::temp_dir().join(format!("domination-bundle-{}.txt", std::process::id()));
    std::fs::write(
        &path,
        "siege-is-progress-3,\n siege-buys-the-gun-resource\n",
    )
    .unwrap();
    let input = args(&["--force-file", path.to_str().unwrap()]);
    let config = parse(&input).unwrap();
    assert_eq!(
        config.forced,
        ["siege-is-progress-3", "siege-buys-the-gun-resource"]
    );
    let game = Game::new_with(options(37_140_008, None, 250, ProbeProfile::default()));
    for enabled in [false, true] {
        let ais = fleet(&game, config.policy, enabled, &config.forced);
        assert!(ais[0].siege_buys_the_gun_resource_enabled());
    }
    for bad in ["no-such-gene", " ,\n"] {
        std::fs::write(&path, bad).unwrap();
        assert!(parse(&input).is_err());
    }
    std::fs::remove_file(&path).unwrap();
    assert!(parse(&input).is_err());
}

#[test]
fn focal_identity_and_handicap_match_the_fixed_king_profile() {
    let game = Game::new_with(options(37140000, None, 250, ProbeProfile::default()));
    assert_eq!(game.players[0].civ, "Gran Colombia");
    assert_eq!((game.map.width, game.map.height), (60, 38));
    assert!(game.is_handicap_exempt(0));
    assert_eq!(game.handicap_yield_pct(0).science, 0.0);
    for pid in 1..4 {
        assert!(!game.is_handicap_exempt(pid));
        assert!(game.handicap_yield_pct(pid).science > 0.0);
    }
    let p = profile(37140000, None, 250, ProbeProfile::default());
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

fn advanced_handicap(difficulty: &str, science: f64, production: f64, strength: f64, army: usize) {
    let config = parse(&args(&["--difficulty", difficulty])).unwrap();
    let game = Game::new_with(options(
        37_140_006,
        config.difficulty,
        config.max_turns,
        ProbeProfile::default(),
    ));
    let recorded = profile(
        37_140_006,
        config.difficulty,
        config.max_turns,
        ProbeProfile::default(),
    );
    assert_eq!(game.difficulty, difficulty);
    assert_eq!(game.barbarian_difficulty, difficulty);
    assert_eq!(recorded["difficulty"], difficulty);
    assert_eq!(recorded["barbarian_difficulty"], difficulty);
    assert_eq!(recorded["handicap_exempt"], serde_json::json!([0]));
    assert_eq!(game.players[0].civ, "Gran Colombia");
    assert_eq!(game.handicap_yield_pct(0).science, 0.0);
    assert_eq!(game.handicap_yield_pct(0).production, 0.0);
    assert_eq!(game.handicap_combat_strength(0), 0.0);
    let military = |pid| {
        game.units
            .values()
            .filter(|unit| {
                unit.owner == pid && game.rules.units[unit.kind.as_str()].class == "military"
            })
            .count()
    };
    assert_eq!(
        military(0),
        1,
        "the focal seat keeps its ordinary starting army"
    );
    for pid in 1..4 {
        assert!(!game.is_handicap_exempt(pid));
        assert_eq!(game.handicap_yield_pct(pid).science, science);
        assert_eq!(game.handicap_yield_pct(pid).production, production);
        assert_eq!(game.handicap_combat_strength(pid), strength);
        assert_eq!(
            military(pid),
            army,
            "rival {pid} receives the selected starting army"
        );
    }
}

#[test]
fn immortal_probe_records_and_applies_asymmetric_level_seven_handicaps() {
    advanced_handicap("immortal", 24.0, 60.0, 2.0, 4);
}

#[test]
fn deity_probe_records_and_applies_asymmetric_level_eight_handicaps() {
    advanced_handicap("deity", 32.0, 80.0, 4.0, 5);
}

#[test]
fn same_seed_seats_and_world_are_identical_before_policy_application() {
    let off = Game::new_with(options(37140001, None, 250, ProbeProfile::default()));
    let on = Game::new_with(options(37140001, None, 250, ProbeProfile::default()));
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
    let game = Game::new_with(options(37140002, None, 250, ProbeProfile::default()));
    let off = fleet(&game, &policy, false, &parse(&args(&[])).unwrap().forced);
    let on = fleet(&game, &policy, true, &parse(&args(&[])).unwrap().forced);
    assert_eq!(off[0].victory_target(), Some(VictoryTarget::Domination));
    assert_eq!(on[0].victory_target(), Some(VictoryTarget::Science));
    for pid in 1..off.len() {
        assert_eq!(off[pid].victory_target(), None);
        assert_eq!(on[pid].victory_target(), None);
    }
}

#[test]
fn real_policy_keeps_domination_and_adaptive_rival_assignments() {
    let game = Game::new_with(options(37140003, None, 250, ProbeProfile::default()));
    for enabled in [false, true] {
        let ais = fleet(
            &game,
            gene("siege-is-progress-3").unwrap(),
            enabled,
            &parse(&args(&[])).unwrap().forced,
        );
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

#[test]
fn conquest_observer_separates_defense_major_attacks_and_minor_attacks() {
    let mut game = Game::new_with(options(37140004, None, 250, ProbeProfile::default()));
    let minor = game
        .players
        .iter()
        .find(|p| p.is_minor && !p.is_barbarian)
        .unwrap()
        .id;
    let mut progress = ConquestProgress::default();
    progress.observe(&game);
    assert_eq!(progress.first_major_war_observed_turn, None);
    game.turn = 30;
    game.at_war.insert((0, 1));
    game.log.push(1, Action::DeclareWar { player: 0 });
    progress.observe(&game);
    assert_eq!(progress.first_major_war_observed_turn, Some(30));
    assert_eq!(progress.first_focal_major_declaration_observed_turn, None);
    game.turn = 40;
    game.log.push(0, Action::DeclareWar { player: minor });
    progress.observe(&game);
    assert_eq!(progress.focal_minor_declarations, 1);
    assert_eq!(progress.focal_major_declarations, 0);
    game.turn = 50;
    game.log.push(
        0,
        Action::DeclareWarWithCasusBelli {
            player: 2,
            casus_belli: "formal".to_string(),
        },
    );
    // The log records a declaration even if its war has ended by this sample.
    let before = serde_json::to_vec(&game).unwrap();
    progress.observe(&game);
    progress.observe(&game);
    assert_eq!(serde_json::to_vec(&game).unwrap(), before);
    assert_eq!(progress.focal_major_declarations, 1);
    assert_eq!(progress.focal_minor_declarations, 1);
    assert_eq!(
        progress.first_focal_major_declaration_observed_turn,
        Some(50)
    );
    assert_eq!(progress.first_major_war_observed_turn, Some(30));
}

#[test]
fn conquest_observer_distinguishes_capitals_from_minors_and_tracks_losses() {
    let mut game = Game::new_with(options(37140005, None, 250, ProbeProfile::default()));
    let minor_city = game
        .cities
        .values()
        .find(|c| game.players[c.owner].is_minor)
        .unwrap()
        .id;
    let mut capitals = Vec::new();
    for pid in 0..3 {
        let unit = game
            .units
            .values()
            .find(|u| u.owner == pid && u.kind.as_str() == "settler")
            .unwrap()
            .id;
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit }).unwrap();
        capitals.push(
            game.cities
                .values()
                .find(|c| c.owner == pid && c.is_capital)
                .unwrap()
                .id,
        );
    }
    let mut progress = ConquestProgress::default();
    game.cities.get_mut(&minor_city).unwrap().owner = 0;
    progress.observe(&game);
    assert_eq!(progress.first_major_city_held_observed_turn, None);
    assert!(progress.own_original_capital_held_at_end);
    game.turn = 60;
    game.cities.get_mut(&capitals[1]).unwrap().owner = 0;
    progress.observe(&game);
    assert_eq!(progress.first_foreign_capital_held_observed_turn, Some(60));
    assert_eq!(progress.foreign_capitals_held_at_end, 1);
    game.turn = 70;
    game.cities.get_mut(&capitals[0]).unwrap().owner = 2;
    game.cities.get_mut(&capitals[1]).unwrap().owner = 1;
    progress.observe(&game);
    assert_eq!(progress.foreign_capitals_held_at_end, 0);
    assert!(!progress.own_original_capital_held_at_end);
    game.turn = 80;
    game.cities.get_mut(&capitals[1]).unwrap().owner = 0;
    progress.observe(&game);
    assert_eq!(
        progress.foreign_capitals_observed_held,
        BTreeSet::from([capitals[1]])
    );
    assert_eq!(
        progress.foreign_major_cities_observed_held,
        BTreeSet::from([capitals[1]])
    );
    assert_eq!(progress.first_major_city_held_observed_turn, Some(60));
    assert_eq!(progress.foreign_capitals_held_at_end, 1);
}

#[test]
fn explicit_native_rung_reaches_player_and_barbarian_rules_and_provenance() {
    for difficulty in ["prince", "king", "emperor", "immortal"] {
        let config = parse(&args(&["--difficulty", difficulty])).unwrap();
        assert_eq!(config.difficulty, Some(difficulty));
        let game = Game::new_with(options(
            37860000,
            config.difficulty,
            config.max_turns,
            ProbeProfile::default(),
        ));
        let report = profile(
            37860000,
            config.difficulty,
            config.max_turns,
            ProbeProfile::default(),
        );
        assert_eq!(report["difficulty"], difficulty);
        assert_eq!(report["barbarian_difficulty"], difficulty);
        assert_eq!(game.barbarian_difficulty, difficulty);
        assert_eq!(game.players[0].civ, "Gran Colombia");
        assert_eq!(game.handicap_yield_pct(0).science, 0.0);
        for pid in 1..4 {
            assert_eq!(
                game.handicap_yield_pct(pid).science > 0.0,
                difficulty != "prince"
            );
        }
    }
}

/// ⭐ `one-owner-per-unit`'s evidence, played on CI: paired Domination games
/// with the gene off and on at the focal seat, the live bundle's Gran
/// Colombia on the default 4-player Pangaea, King. Prints the off arm's
/// crossing table (owner x step), the unordered unit-turn share, and the
/// paired combat ledgers. `cargo test --bin victory_eval -- --nocapture`
/// shows it; it asserts only that the instrument read a war.
#[test]
fn one_owner_per_unit_paired_evidence() {
    const SEEDS: std::ops::Range<u64> = 37_140_000..37_140_006;
    const TURNS: &str = "200";
    let config = parse(
        &[
            "--domination-pair",
            "one-owner-per-unit",
            "--out",
            "/tmp/one-owner-per-unit-evidence.jsonl",
            "--max-turns",
            TURNS,
        ]
        .map(str::to_string),
    )
    .expect("the evidence configuration parses");
    let jobs: Vec<(u64, bool)> = SEEDS
        .flat_map(|seed| [(seed, false), (seed, true)])
        .collect();
    let mut trials: Vec<(u64, bool, Trial)> = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .chunks(jobs.len().div_ceil(4))
            .map(|chunk| {
                let config = &config;
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn_scoped(scope, move || {
                        chunk
                            .iter()
                            .map(|&(seed, on)| (seed, on, trial(seed, config, on)))
                            .collect::<Vec<_>>()
                    })
                    .expect("a worker starts")
            })
            .collect();
        for handle in handles {
            trials.extend(handle.join().expect("a worker finishes"));
        }
    });
    trials.sort_by_key(|(seed, on, _)| (*seed, *on));
    let arm = |on: bool| trials.iter().filter(move |(_, o, _)| *o == on);
    let mut table: BTreeMap<String, [u32; 3]> = BTreeMap::new();
    let (mut unit_turns, mut unordered) = (0u32, 0u32);
    for (_, _, trial) in arm(false) {
        for (cell, counts) in &trial.outcome.focal_claim_crossings {
            let total = table.entry(cell.clone()).or_default();
            for (sum, n) in total.iter_mut().zip(counts) {
                *sum += n;
            }
        }
        unit_turns += trial.outcome.focal_unit_turns;
        unordered += trial.outcome.focal_unordered_unit_turns;
    }
    let crossed: u32 = table.values().map(|cell| cell[0]).sum();
    let deliberate: u32 = table.values().map(|cell| cell[1]).sum();
    let mut report = format!(
        "one-owner-per-unit evidence: focal Gran Colombia (live bundle + gene), 4p 60x38 \
         Pangaea, 6 city-states, King, Online, barbarians, {TURNS} turns max, seeds {}..{}\n\
         OFF crossing table (owner:step -> crossed, deliberate), {} unit-turns:\n",
        SEEDS.start,
        SEEDS.end - 1,
        unit_turns
    );
    let mut cells: Vec<_> = table.iter().collect();
    cells.sort_by(|a, b| (b.1[0], b.1[1]).cmp(&(a.1[0], a.1[1])).then(a.0.cmp(b.0)));
    for (cell, counts) in cells {
        report.push_str(&format!("  {cell:<40} {:>5} {:>5}\n", counts[0], counts[1]));
    }
    report.push_str(&format!(
        "OFF totals: {crossed} crossings ({:.2} per 100 unit-turns), {deliberate} deliberate; \
         unordered unit-turns {unordered} of {unit_turns} ({:.1}%)\n",
        100.0 * f64::from(crossed) / f64::from(unit_turns.max(1)),
        100.0 * f64::from(unordered) / f64::from(unit_turns.max(1)),
    ));
    let (mut refused, mut residual) = (0u32, 0u32);
    for (_, _, trial) in arm(true) {
        refused += trial
            .outcome
            .focal_claim_crossings
            .values()
            .map(|cell| cell[2])
            .sum::<u32>();
        residual += trial
            .outcome
            .focal_claim_crossings
            .values()
            .map(|cell| cell[0])
            .sum::<u32>();
    }
    report.push_str(&format!(
        "ON totals: {refused} refused, {residual} crossings left\n\
         paired focal ledger (seed: off -> on): units killed, units lost, cities taken, \
         cities lost, winner, turn\n"
    ));
    let mut sums = [[0i64; 4]; 2];
    for seed in SEEDS {
        let pick = |on: bool| {
            &trials
                .iter()
                .find(|(s, o, _)| *s == seed && *o == on)
                .expect("both arms played")
                .2
        };
        let (off, on) = (pick(false), pick(true));
        let read = |trial: &Trial| {
            let outcome = &trial.outcome;
            [
                i64::from(outcome.focal_units.units_killed),
                i64::from(outcome.focal_units.units_lost),
                outcome.focal_cities_taken,
                outcome.focal_cities_lost as i64,
            ]
        };
        let (a, b) = (read(off), read(on));
        for (index, values) in [a, b].iter().enumerate() {
            for (sum, value) in sums[index].iter_mut().zip(values) {
                *sum += value;
            }
        }
        report.push_str(&format!(
            "  {seed}: {a:?} -> {b:?}  winner {:?}/{:?} {:?}/{:?} turn {}/{}{}\n",
            off.outcome.winner,
            on.outcome.winner,
            off.outcome.victory,
            on.outcome.victory,
            off.outcome.turn,
            on.outcome.turn,
            if off.actions == on.actions {
                " (identical games)"
            } else {
                ""
            }
        ));
    }
    report.push_str(&format!("  sum: {:?} -> {:?}\n", sums[0], sums[1]));
    eprintln!("{report}");
    assert!(unit_turns > 0, "{report}");
    assert!(
        sums[0][0] + sums[0][1] > 0,
        "the worlds must fight: {report}"
    );
    panic!("REPORT\n{report}");
}
