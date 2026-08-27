use super::*;

fn game_with_capitals(players: usize, seed: u64, max_turns: u32) -> Game {
    let mut g = Game::new_full(players, 26, 16, seed, max_turns, 0, false);
    for pid in 0..players {
        let pos = g
            .player_unit_ids(pid)
            .into_iter()
            .find_map(|uid| {
                let u = &g.units[&uid];
                (u.kind == "settler").then_some(u.pos)
            })
            .unwrap();
        g.found_city_for(pid, pos, None);
    }
    g
}

fn play_to_the_end(game: &mut Game) {
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        let _ = game.apply(pid, &Action::EndTurn);
    }
}

#[test]
fn require_n_banks_types_until_the_set_is_complete() {
    let mut game = game_with_capitals(2, 91_500, 300);
    game.required_victory_types = 2;
    assert!(!game.set_winner(0, "science"), "one type is short of two");
    assert_eq!(game.winner, None);
    assert_eq!(game.victories_won[&0].len(), 1);
    assert!(
        !game.set_winner(0, "science"),
        "a re-fired condition banks nothing new"
    );
    assert_eq!(game.victories_won[&0].len(), 1);
    assert!(
        game.set_winner(0, "culture"),
        "the second distinct type completes the set"
    );
    assert_eq!(game.winner, Some(0));
    assert_eq!(game.victory_type.as_deref(), Some("culture"));
}

#[test]
fn require_n_clamps_to_the_enabled_victory_count() {
    let mut game = game_with_capitals(2, 91_501, 300);
    game.required_victory_types = 6;
    game.victory_conditions = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: false,
        score: false,
    };
    assert_eq!(game.effective_required_victories(), 2);
    assert!(!game.set_winner(0, "science"));
    assert!(
        game.set_winner(0, "culture"),
        "two enabled types are the whole reachable cap"
    );
    assert_eq!(game.winner, Some(0));
}

#[test]
fn require_n_turn_limit_crowns_the_most_banked_types() {
    let mut game = game_with_capitals(2, 91_502, 3);
    game.required_victory_types = 3;
    assert!(!game.set_winner(1, "science"));
    assert!(!game.set_winner(1, "culture"));
    // The wrap past the limit banks score for the leading scorer, then
    // ends the world on whoever holds the most banked types.
    play_to_the_end(&mut game);
    assert_eq!(game.winner, Some(1), "two banked types beat at most one");
    assert_eq!(game.victory_type.as_deref(), Some("score"));
}

#[test]
fn mercy_rule_ends_the_game_the_moment_the_bar_is_met() {
    let mut game = game_with_capitals(2, 91_503, 50);
    game.mercy_rule = Some(0.0);
    play_to_the_end(&mut game);
    assert_eq!(game.victory_type.as_deref(), Some("mercy"));
    assert!(game.winner.is_some());
    assert!(
        game.turn <= 3,
        "a floor of zero concedes on the first wrap, not turn {}",
        game.turn
    );
}

#[test]
fn without_a_mercy_rule_the_game_plays_to_its_natural_end() {
    let mut game = game_with_capitals(2, 91_504, 5);
    assert_eq!(game.mercy_rule, None, "the engine default is off");
    play_to_the_end(&mut game);
    assert_ne!(game.victory_type.as_deref(), Some("mercy"));
}

#[test]
fn mercy_and_required_types_round_trip_and_old_saves_default_off() {
    let mut game = game_with_capitals(2, 91_505, 300);
    game.mercy_rule = Some(0.97);
    game.required_victory_types = 3;
    let _ = game.set_winner(0, "science");
    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored.mercy_rule, Some(0.97));
    assert_eq!(restored.required_victory_types, 3);
    assert!(restored.victories_won[&0].contains("science"));

    let mut legacy = encoded;
    let save = legacy.as_object_mut().unwrap();
    save.remove("mercy_rule");
    save.remove("required_victory_types");
    save.remove("victories_won");
    let restored: Game = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.mercy_rule, None);
    assert_eq!(restored.effective_required_victories(), 1);
    assert!(restored.victories_won.is_empty());
}

#[test]
fn the_mercy_notation_names_its_lanes_and_never_holds_a_comma() {
    assert_eq!(mercy_label(&[]), "Mercy Rule");
    assert_eq!(
        mercy_label(&["science".to_string()]),
        "Mercy Rule - Science"
    );
    assert_eq!(
        mercy_label(&["science".to_string(), "domination".to_string()]),
        "Mercy Rule - Science + Domination"
    );
    // The joiner is load-bearing. This string is written into the
    // `victory` column of the league's `matches.csv`, and both readers of
    // that file cut its rows on commas, so one comma here would shift
    // every later column and take the whole recorded history with it.
    for lanes in [
        Vec::new(),
        vec!["religious".to_string()],
        vec!["culture".to_string(), "diplomatic".to_string()],
        VictoryConditions::NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
    ] {
        let label = mercy_label(&lanes);
        assert!(!label.contains(','), "{label}");
    }
}

#[test]
fn the_leading_lanes_are_the_open_races_tied_at_the_front() {
    let mut game = game_with_capitals(2, 91_506, 300);
    // A fresh two-seat board: each seat holds one of the two capitals, so
    // Domination alone reads 50% and no other race has started.
    assert_eq!(
        game.leading_victory_lanes(0),
        vec!["domination".to_string()]
    );

    // Two science projects put Science at 45%, behind Domination. A lane
    // that is merely under way is not the lane the game is being decided
    // on, and is not named.
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        game.players[0].science_projects.insert(project.to_string());
    }
    assert_eq!(
        game.leading_victory_lanes(0),
        vec!["domination".to_string()]
    );

    // A third puts Science at 65% and out in front on its own.
    game.players[0]
        .science_projects
        .insert("launch_mars_colony".to_string());
    assert_eq!(game.leading_victory_lanes(0), vec!["science".to_string()]);

    // A lane the lobby switched off is not a lane this world is deciding,
    // however far along the seat happens to be in it.
    game.victory_conditions.science = false;
    assert_eq!(
        game.leading_victory_lanes(0),
        vec!["domination".to_string()]
    );

    // Level on two open lanes is two things to say, not a choice between
    // them: ten of the twenty Diplomatic points is exactly the 50% the
    // held capital reads.
    game.players[0].dvp = DIPLOMATIC_VICTORY_POINTS / 2;
    assert_eq!(
        game.leading_victory_lanes(0),
        vec!["diplomatic".to_string(), "domination".to_string()]
    );

    // A seat can cross the odds threshold on standing and tempo with no
    // race under way at all. That is a real board, and the notation says
    // so by naming no lane rather than inventing one.
    game.victory_conditions.diplomatic = false;
    game.victory_conditions.domination = false;
    assert!(game.leading_victory_lanes(0).is_empty());
    assert_eq!(mercy_label(&game.leading_victory_lanes(0)), "Mercy Rule");
}

#[test]
fn a_mercy_ending_is_denoted_by_the_lane_it_ended_on() {
    let mut game = game_with_capitals(2, 91_507, 50);
    game.mercy_rule = Some(0.0);
    play_to_the_end(&mut game);

    // The recorded type is unchanged — the engine still answers "mercy",
    // and every rule that keys off it keeps working. What changed is how
    // the result is written down.
    assert_eq!(game.victory_type.as_deref(), Some(MERCY_VICTORY));
    assert_eq!(
        game.victory_label().as_deref(),
        Some("Mercy Rule - Domination"),
        "a floor of zero concedes on the opening board, where the seat's \
         own capital is the only race anybody has begun"
    );

    // The seats are told the notation, not that somebody won a "mercy
    // victory" — the rule is a concession, and the lane is the news.
    let declared = game
        .events
        .iter()
        .rev()
        .find(|event| event.text.contains("Mercy Rule - Domination"))
        .expect("the chronicle records the notation");
    assert!(
        declared.text.contains("won by"),
        "unexpected verdict: {}",
        declared.text
    );
    assert!(!declared.text.contains("mercy victory"));

    // The notation is composed, never stored, so a save carries the lanes
    // and rebuilds the same string.
    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored.mercy_lanes, game.mercy_lanes);
    assert_eq!(restored.victory_label(), game.victory_label());

    // A save written before the notation existed has no lanes to name and
    // keeps the bare rule rather than failing to load.
    let mut legacy = encoded;
    legacy.as_object_mut().unwrap().remove("mercy_lanes");
    let restored: Game = serde_json::from_value(legacy).unwrap();
    assert!(restored.mercy_lanes.is_empty());
    assert_eq!(restored.victory_label().as_deref(), Some("Mercy Rule"));

    // "One more turn" hands the verdict to `decided` and leaves the live
    // world with no lanes of its own, so a later ending of the extension
    // cannot inherit the notation of the game before it.
    let label = game.victory_label().unwrap();
    assert!(game.play_on(PlayOnMode::UntilNextVictory));
    assert!(game.mercy_lanes.is_empty());
    assert_eq!(game.decided.as_ref().unwrap().victory_label(), label);
}

/// A hand-built board is the easy case. This crosses on played-out worlds,
/// where the lanes are whatever the game made them, and holds every
/// resulting notation to its invariants.
///
/// The rung is well below the shipped ladder, and deliberately so. The
/// measurement in docs/ADJUDICATION.md is that 95% arrives only a handful
/// of turns before the rules end the game anyway; measured here, no seed
/// crosses anything at or above 0.45 inside a test-sized turn budget, and
/// the shipped rungs are what `civvis odds-audit` exists to exercise at
/// full length. What is under test is the notation on boards nobody wrote
/// by hand, and a low rung is how those get reached at all.
#[test]
fn played_out_mercy_endings_are_all_well_formed_notations() {
    let mut crossings = 0;
    for seed in 0..12u64 {
        let mut game = game_with_capitals(3, 91_600 + seed, 120);
        game.mercy_rule = Some(0.35);
        play_to_the_end(&mut game);
        if game.victory_type.as_deref() != Some(MERCY_VICTORY) {
            continue;
        }
        crossings += 1;
        let label = game.victory_label().expect("a crowned game has a label");
        assert_eq!(label, mercy_label(&game.mercy_lanes));
        assert!(label.starts_with("Mercy Rule"), "seed {seed}: {label}");
        // The invariant the league's comma-cut history depends on, held
        // against real boards rather than a hand-written lane list.
        assert!(!label.contains(','), "seed {seed}: {label}");
        let winner = game.winner.expect("a mercy ending has a winner");
        for lane in &game.mercy_lanes {
            assert!(
                game.victory_conditions.is_enabled(lane),
                "seed {seed}: named a lane this world switched off: {lane}"
            );
            assert_ne!(lane, "score", "Score is a standing, not a race");
        }
        assert_eq!(
            game.mercy_lanes,
            game.leading_victory_lanes(winner),
            "seed {seed}: the lanes are the winner's, read at the crossing"
        );
    }
    assert_eq!(
        crossings, 12,
        "every seed should concede at a rung this low; a run that stopped \
         crossing is testing nothing, not passing"
    );
}

#[test]
fn an_ordinary_victory_is_still_written_as_its_bare_type() {
    let mut game = game_with_capitals(2, 91_508, 300);
    assert_eq!(game.victory_label(), None, "a live game has no verdict");
    assert!(game.set_winner(0, "science"));
    assert_eq!(game.victory_label().as_deref(), Some("science"));
    assert!(game.mercy_lanes.is_empty(), "only mercy carries lanes");
    assert!(game
        .events
        .iter()
        .any(|event| event.text.contains("won a science victory")));
}

#[test]
fn every_victory_path_can_be_disabled_independently() {
    type VictoryConditionToggle = (&'static str, fn(&mut VictoryConditions) -> &mut bool);
    let conditions: [VictoryConditionToggle; 6] = [
        ("science", |v: &mut VictoryConditions| &mut v.science),
        ("culture", |v: &mut VictoryConditions| &mut v.culture),
        ("religious", |v: &mut VictoryConditions| &mut v.religious),
        ("diplomatic", |v: &mut VictoryConditions| &mut v.diplomatic),
        ("domination", |v: &mut VictoryConditions| &mut v.domination),
        ("score", |v: &mut VictoryConditions| &mut v.score),
    ];
    for (index, (victory_type, disable)) in conditions.into_iter().enumerate() {
        let mut game = game_with_capitals(2, 90_000 + index as u64, 300);
        *disable(&mut game.victory_conditions) = false;
        game.set_winner(0, victory_type);
        assert_eq!(game.winner, None, "disabled {victory_type} ended the game");

        *disable(&mut game.victory_conditions) = true;
        game.set_winner(0, victory_type);
        assert_eq!(game.winner, Some(0), "enabled {victory_type} did not win");
        assert_eq!(game.victory_type.as_deref(), Some(victory_type));
    }
}

/// "One more turn" has no turn cap. Its bounded form stops only for a
/// genuinely subsequent result; its indefinite form stops for none.
#[test]
fn playing_on_can_wait_for_the_next_victory_or_run_indefinitely() {
    let mut game = game_with_capitals(2, 90_100, 40);
    game.turn = 30;
    game.set_winner(1, "diplomatic");
    assert_eq!(game.winner, Some(1));

    assert!(game.play_on(PlayOnMode::UntilNextVictory));
    assert_eq!(game.winner, None);
    assert_eq!(
        game.max_turns, 40,
        "the configured game setting is retained"
    );
    assert_eq!(game.turn_limit(), None, "playing on has no turn cap");
    assert_eq!(game.decided.as_ref().map(|d| d.winner), Some(1));
    assert_eq!(
        game.decided.as_ref().map(|d| d.victory_type.as_str()),
        Some("diplomatic")
    );
    assert_eq!(game.decided.as_ref().map(|d| d.turn), Some(30));
    assert_eq!(
        game.decided.as_ref().map(|d| d.mode),
        Some(PlayOnMode::UntilNextVictory)
    );

    // Crossing the old cap does not manufacture a score result.
    game.turn = game.max_turns;
    game.current = 1;
    game.do_end_turn();
    assert!(game.turn > game.max_turns);
    assert_eq!(game.winner, None);

    // A persistent requirement may try to repeat the verdict that opened
    // the continuation. That exact result is not a next victory.
    assert!(!game.set_winner(1, "diplomatic"));
    assert_eq!(game.winner, None);

    // The exhibition checkpoints and resumes mid-game, so the verdict has
    // to survive a save together with the requested stopping rule.
    let raw = serde_json::to_string(&game).expect("a played-on game saves");
    let reloaded: Game = serde_json::from_str(&raw).expect("and loads");
    assert_eq!(reloaded.decided, game.decided);
    assert_eq!(reloaded.max_turns, game.max_turns);
    assert!(reloaded.played_on());

    // A different civilization can win through the same lane. It is a
    // genuinely later result even though the victory type is unchanged.
    assert!(game.set_winner(0, "diplomatic"));
    assert_eq!(game.winner, Some(0));

    // The second finish screen can choose the other answer. It records the
    // result on that screen, not the older result that preceded it.
    assert!(game.play_on(PlayOnMode::Indefinite));
    assert_eq!(game.decided.as_ref().map(|d| d.winner), Some(0));
    assert_eq!(
        game.decided.as_ref().map(|d| d.mode),
        Some(PlayOnMode::Indefinite)
    );
    for victory_type in VictoryConditions::NAMES {
        assert!(!game.set_winner(1, victory_type));
        assert_eq!(game.winner, None, "{victory_type} ended indefinite play");
    }

    // A live game has nothing to play on past.
    let mut untouched = game_with_capitals(2, 90_101, 40);
    assert!(!untouched.play_on(PlayOnMode::UntilNextVictory));
    assert!(untouched.decided.is_none());
}

/// The Civilization Players League publishes the lobby it plays
/// (https://cpl.gg/rules/in-game-rules/), and `docs/COMPETITIVE.md` maps
/// every line of it onto the setting that pins it here. This asserts the
/// mapping still holds, because the failure mode of a tournament preset is
/// silence: a setting the engine stops honouring produces a game that runs
/// perfectly and is not the game that was set up. Two of these assertions
/// are regressions — barbarians were unreachable from any lobby, and the
/// New Frontier game modes ran in every game whatever the lobby said.
#[test]
fn civilizations_are_seated_on_the_starts_their_bias_asks_for() {
    // Shipped StartBias rows decide which start a civilization gets. Over
    // a spread of seeds the biased civilizations should score better on
    // their own bias than the seat order alone would give them.
    let mut biased = 0;
    let mut unbiased = 0;
    for seed in 0..10u64 {
        let game = Game::new_full(8, 84, 54, 71_000 + seed, 250, 0, false);
        // Seats begin with a Settler on their start rather than a city.
        let sites: Vec<Pos> = (0..8)
            .map(|pid| {
                let unit = game.player_unit_ids(pid)[0];
                game.units[&unit].pos
            })
            .collect();
        for pid in 0..8 {
            let civ = game.players[pid].civ.clone();
            if game.rules.civs[&civ].start_bias.is_none() {
                continue;
            }
            let mine =
                crate::mapgen::start_bias_score(&game.rules, &game.map, sites[pid], civ.as_str());
            let others: i32 = sites
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != pid)
                .map(|(_, pos)| crate::mapgen::start_bias_score(&game.rules, &game.map, *pos, &civ))
                .sum::<i32>()
                / 7;
            if mine >= others {
                biased += 1;
            } else {
                unbiased += 1;
            }
        }
    }
    assert!(
        biased > unbiased * 3,
        "biased civilizations should usually beat the average site for their own bias: \
         {biased} better vs {unbiased} worse"
    );
}

#[test]
fn the_published_tournament_lobby_still_sets_up_the_game_it_describes() {
    let rules = Rules::embedded();
    // Game Speed: Online, Limit Turns: By Game Speed.
    let online = &rules.speeds["online"];
    assert_eq!(online.turns, 250);
    assert_eq!(online.cost_pct, 50.0);

    // Map Size and City States: Firaxis default for the player count.
    let size = crate::setup::MapSize::for_players(8);
    assert_eq!((size.id, size.width, size.height), ("standard", 84, 54));
    assert_eq!(size.default_city_states, 12);

    // A 4v4 teamers lobby: barbarians off, no game modes, every victory on.
    let mut options = GameOptions::new(
        8,
        size.width,
        size.height,
        90_311,
        online.turns,
        size.default_city_states,
    );
    options.speed = "online".to_string();
    options.teams = vec![
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        Some(1),
    ];
    options.barbarians = false;
    let game = Game::new_with(options);

    assert_eq!(game.max_turns, 250);
    assert_eq!(game.map_size().id, "standard");
    assert_eq!(game.max_religions(), 5);
    assert_eq!(
        game.players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .count(),
        12
    );

    // Barbarians ON for FFA and OFF for Teamers.
    assert!(game.barb_pid.is_none());
    assert!(game
        .players
        .iter()
        .filter(|player| player.is_barbarian)
        .all(|player| player.is_free_city));

    // All Game Modes: DISABLED.
    for mode in GAME_MODES {
        assert!(!game.game_mode(mode), "{mode} is not a stock lobby rule");
    }

    // Teams Share Visibility, and pre-game teams as assigned.
    assert!(game.same_team(0, 3));
    assert!(!game.same_team(0, 4));
    assert_eq!(game.team_members(0), vec![0, 1, 2, 3]);
    assert_eq!(game.team_members(7), vec![4, 5, 6, 7]);

    // All Victory Conditions: ENABLED.
    for victory in VictoryConditions::NAMES {
        assert!(game.victory_conditions.is_enabled(victory), "{victory}");
    }
}

#[test]
fn victory_settings_parse_the_lobby_list_and_refuse_a_misspelling() {
    let pinned = VictoryConditions::parse("science,culture, score").unwrap();
    assert_eq!(
        pinned,
        VictoryConditions {
            science: true,
            culture: true,
            religious: false,
            diplomatic: false,
            domination: false,
            score: true,
        }
    );
    assert_eq!(
        VictoryConditions::parse(&VictoryConditions::NAMES.join(",")).unwrap(),
        VictoryConditions::default()
    );
    // A name the lobby does not know must be refused. Accepting it would
    // leave every path switched off, and a game nobody can win looks
    // exactly like a working one until the turn limit arrives.
    assert!(VictoryConditions::parse("religion").is_err());
    assert!(VictoryConditions::parse("").is_err());
}

#[test]
fn victory_settings_round_trip_and_old_saves_default_to_enabled() {
    let mut game = game_with_capitals(2, 90_010, 300);
    game.victory_conditions.culture = false;
    game.victory_conditions.score = false;
    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored.victory_conditions, game.victory_conditions);

    let mut legacy = encoded;
    legacy.as_object_mut().unwrap().remove("victory_conditions");
    let restored: Game = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.victory_conditions, VictoryConditions::default());
}

#[test]
fn world_era_uses_all_nine_tree_eras_including_future() {
    let mut g = game_with_capitals(2, 400, 500);
    for player in g.players.iter_mut() {
        player.techs.clear();
        player.civics.clear();
    }
    assert_eq!(g.era_from_progress(), 0);

    for era in 0..crate::rules::ERA_NAMES.len() {
        let tech = g
            .rules
            .techs
            .iter()
            .find(|(_, spec)| spec.era == era)
            .map(|(name, _)| *name)
            .unwrap();
        g.players[0].techs.clear();
        g.players[0].techs.insert(tech);
        assert_eq!(g.era_from_progress(), era, "technology era {era}");

        let civic = g
            .rules
            .civics
            .iter()
            .find(|(_, spec)| spec.era == era)
            .map(|(name, _)| *name)
            .unwrap();
        g.players[0].techs.clear();
        g.players[0].civics.clear();
        g.players[0].civics.insert(civic);
        assert_eq!(g.era_from_progress(), era, "civic era {era}");
    }

    g.world_era = 3;
    g.players[0].civics.clear();
    g.players[0].techs.insert(crate::name!("smart_materials"));
    g.players[1].techs.insert(crate::name!("smart_materials"));
    // Half the living majors reaching the next era starts the shipped
    // ten-turn warning; the transition itself follows at its end.
    g.turn = 40;
    g.process_eras();
    assert_eq!(g.world_era, 3);
    g.turn = 50;
    g.process_eras();
    assert_eq!(
        g.world_era, 4,
        "late research advances the world without skipping Industrial"
    );
}

/// A live game's Spies are mirrored `UNIT_SPY` units, not `spies` entries,
/// so every "am I at capacity" test used to answer zero and the empire
/// re-ordered a Spy the host would refuse. `UNIT_SPY` was the second
/// most-requested production item in the fleet at 9.8% of all orders, 84%
/// of them refused.
#[test]
fn mirrored_spy_units_count_against_spy_capacity() {
    let mut g = game_with_capitals(2, 404, 500);
    let cid = g.player_city_ids(0)[0];
    for civic in ["diplomatic_service", "nationalism", "ideology", "cold_war"] {
        g.players[0].civics.insert(Name::new(civic));
    }
    let capacity = g.spy_capacity(0) as usize;
    assert_eq!(capacity, 4);
    let spy = Item::Unit {
        unit: crate::name!("spy"),
    };
    assert_eq!(g.spy_agents(0), 0);
    assert!(g.can_produce(0, cid, &spy), "an empty roster may train one");

    // The live shape: agents arrive as units and `spies` stays empty.
    let home = g.cities[&cid].pos;
    for _ in 0..capacity {
        g.spawn_unit("spy", 0, home);
    }
    assert!(g.spies.is_empty(), "the mirror never fills the agent map");
    assert_eq!(
        g.spy_agents(0),
        capacity,
        "the unit census is the live count"
    );
    assert!(
        !g.can_produce(0, cid, &spy),
        "a full roster of mirrored Spies must refuse another"
    );

    // Another player's Spies are not ours, and neither are other units.
    let mut h = game_with_capitals(2, 405, 500);
    let hcid = h.player_city_ids(0)[0];
    for civic in ["diplomatic_service", "nationalism", "ideology", "cold_war"] {
        h.players[0].civics.insert(Name::new(civic));
    }
    let hhome = h.cities[&hcid].pos;
    h.spawn_unit("builder", 0, hhome);
    h.spawn_unit("spy", 1, h.cities[&h.player_city_ids(1)[0]].pos);
    assert_eq!(h.spy_agents(0), 0, "only our own Spies count");
    assert_eq!(h.spy_agents(1), 1);
}

#[test]
fn late_tree_airlifts_and_repeatable_nodes_execute_from_rules_data() {
    let mut g = game_with_capitals(2, 404, 500);
    let cid = g.player_city_ids(0)[0];
    for civic in ["diplomatic_service", "nationalism", "ideology", "cold_war"] {
        g.players[0].civics.insert(Name::new(civic));
    }
    assert_eq!(g.spy_capacity(0), 4);

    let second_city_position = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.city_at(**pos).is_none()
                && g.wdist(g.cities[&cid].pos, **pos) >= 5
        })
        .map(|(pos, _)| *pos)
        .next()
        .unwrap();
    let second_city = g.found_city_for(0, second_city_position, None);
    let origin_aerodrome = install_test_district(&mut g, cid, "aerodrome");
    let destination_aerodrome = install_test_district(&mut g, second_city, "aerodrome");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("airport"));
    g.cities
        .get_mut(&second_city)
        .unwrap()
        .buildings
        .push(crate::name!("airport"));
    g.players[0].civics.insert(crate::name!("rapid_deployment"));
    let center_builder = g.spawn_unit("builder", 0, g.cities[&cid].pos);
    assert!(g.airlift_destinations(0, center_builder).is_empty());
    let airlifted = g.spawn_unit("builder", 0, origin_aerodrome);
    g.map.tiles.get_mut(&origin_aerodrome).unwrap().pillaged = true;
    assert!(g.airlift_destinations(0, airlifted).is_empty());
    g.map.tiles.get_mut(&origin_aerodrome).unwrap().pillaged = false;
    g.cities
        .get_mut(&second_city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("airport"));
    assert!(g.airlift_destinations(0, airlifted).is_empty());
    g.cities
        .get_mut(&second_city)
        .unwrap()
        .pillaged_buildings
        .remove(&Name::new("airport"));
    assert!(!g
        .airlift_destinations(0, airlifted)
        .contains(&second_city_position));
    assert!(g
        .airlift_destinations(0, airlifted)
        .contains(&destination_aerodrome));
    g.map
        .tiles
        .get_mut(&destination_aerodrome)
        .unwrap()
        .pillaged = true;
    assert!(g.airlift_destinations(0, airlifted).is_empty());
    g.map
        .tiles
        .get_mut(&destination_aerodrome)
        .unwrap()
        .pillaged = false;
    g.do_move(0, airlifted, destination_aerodrome).unwrap();
    assert_eq!(g.units[&airlifted].pos, destination_aerodrome);

    let position = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            *pos != g.cities[&cid].pos
                && !g.rules.is_water(&g.map.tiles[pos])
                && g.map.tiles[pos].district.is_none()
        })
        .unwrap();

    assert!(!g.resource_visible_to(0, "uranium"));
    g.players[0].techs.insert(crate::name!("combined_arms"));
    assert!(g.resource_visible_to(0, "uranium"));

    {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("forest"));
        tile.resource = None;
        tile.improvement = None;
        tile.hills = true;
        tile.pillaged = false;
    }
    let builder = g.spawn_unit("builder", 0, position);
    g.players[0].techs.remove(&Name::new("mining"));
    assert!(g
        .do_improve(0, builder, "chop_woods")
        .unwrap_err()
        .contains("cannot perform"));
    g.players[0].techs.insert(crate::name!("mining"));
    let production_before = g.cities[&cid].production;
    g.do_improve(0, builder, "chop_woods").unwrap();
    assert_eq!(g.map.tiles[&position].feature, None);
    assert!(g.cities[&cid].production > production_before);

    g.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("mine"));
    let base = g.rules.tile_yields(&g.map.tiles[&position]).production;
    g.players[0].techs.insert(crate::name!("apprenticeship"));
    g.players[0].techs.insert(crate::name!("industrialization"));
    assert_eq!(
        g.player_tile_yields(0, position, &g.map.tiles[&position])
            .production,
        base + 2.0
    );

    g.map.tiles.get_mut(&position).unwrap().improvement = None;
    assert!(!g
        .valid_improvements(0, position)
        .contains(&crate::name!("farm")));
    g.players[0]
        .civics
        .insert(crate::name!("civil_engineering"));
    assert!(g
        .valid_improvements(0, position)
        .contains(&crate::name!("farm")));

    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    let tourism_before_conservation = g.tourism_per_turn(0);
    g.players[0].civics.insert(crate::name!("conservation"));
    assert!(
        (g.tourism_per_turn(0) - tourism_before_conservation - 1.0).abs() < 1e-9,
        "Conservation's wall tourism must be driven by its tree effect"
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "walls");

    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 0);
    g.players[0].techs.insert(crate::name!("steel"));
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 400);

    g.players[0].techs.insert(crate::name!("offworld_mission"));
    g.players[0].techs.insert(crate::name!("future_tech"));
    assert!(g.available_techs(0).contains(&crate::name!("future_tech")));
    g.apply_tree_completion(0, true, "future_tech", false);
    g.apply_tree_completion(0, true, "future_tech", false);
    assert_eq!(g.players[0].counters["tree_completions:future_tech"], 2);

    for prerequisite in g.rules.civics["future_civic"].requires.clone() {
        g.players[0].civics.insert(prerequisite);
    }
    g.players[0].civics.insert(crate::name!("future_civic"));
    assert!(g
        .available_civics(0)
        .contains(&crate::name!("future_civic")));
    g.apply_tree_completion(0, false, "future_civic", false);
    assert_eq!(g.players[0].counters["district_governor_titles"], 1);
    assert_eq!(g.players[0].counters["diplomatic_favor"], 50);
}

#[test]
fn save_restore_preserves_the_games_randomized_future_trees() {
    let game = Game::new_full(2, 24, 16, 818_181, 80, 0, false);
    let expected = game.rules.future_tree_layout();
    let value = serde_json::to_value(&game).unwrap();
    assert_eq!(
        value["future_tree_layout"]["techs"]
            .as_object()
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        value["future_tree_layout"]["civics"]
            .as_object()
            .unwrap()
            .len(),
        6
    );

    let restored: Game = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(restored.rules.future_tree_layout(), expected);

    // Saves written before this field existed regenerate through a
    // domain-separated stream, so loading one cannot perturb the saved
    // runtime RNG and the same seed still recovers the same graph.
    let mut legacy = value;
    legacy.as_object_mut().unwrap().remove("future_tree_layout");
    let restored_legacy: Game = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored_legacy.rules.future_tree_layout(), expected);
    assert_eq!(restored_legacy.rng, game.rng);
}

#[test]
fn science_requires_the_space_race_and_exoplanet_arrival() {
    let protocol_item: Item =
        serde_json::from_str(r#"{"project":"launch_earth_satellite"}"#).unwrap();
    assert_eq!(
        protocol_item,
        Item::Project {
            project: crate::name!("launch_earth_satellite")
        }
    );

    let mut g = game_with_capitals(2, 401, 300);
    let all_techs: Vec<Name> = g.rules.techs.keys().cloned().collect();
    for tech in all_techs
        .iter()
        .filter(|t| t.as_str() != "offworld_mission")
    {
        g.players[0].techs.insert(Name::new(&tech.clone()));
    }
    g.players[0].research = Some("offworld_mission".to_string());
    g.players[0].research_progress = g.rules.techs["offworld_mission"].cost;
    g.begin_turn(0);
    assert_eq!(g.players[0].techs.len(), g.rules.techs.len());
    assert_eq!(
        g.winner, None,
        "finishing the technology tree is not a science victory"
    );

    let cid = g.player_city_ids(0)[0];
    let spaceport = g.cities[&cid].owned_tiles[1];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), spaceport);
    assert_eq!(g.rules.districts["spaceport"].cost, 1800.0);
    assert_eq!(g.rules.projects["launch_earth_satellite"].cost, 900.0);
    assert_eq!(g.rules.projects["launch_moon_landing"].cost, 1500.0);
    assert_eq!(g.rules.projects["launch_mars_colony"].cost, 1800.0);
    assert_eq!(g.rules.projects["exoplanet_expedition"].cost, 2100.0);

    let earth = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    let moon = Item::Project {
        project: crate::name!("launch_moon_landing"),
    };
    let mars = Item::Project {
        project: crate::name!("launch_mars_colony"),
    };
    let exoplanet = Item::Project {
        project: crate::name!("exoplanet_expedition"),
    };
    assert!(g.can_produce(0, cid, &earth));
    assert!(!g.can_produce(0, cid, &moon));
    g.players[0].explored.clear();
    assert!(g.complete_item(0, cid, &earth));
    assert_eq!(
        g.players[0].explored.len(),
        g.map.tiles.len(),
        "Earth Satellite reveals the whole map"
    );
    assert!(g.can_produce(0, cid, &moon));
    let science = g
        .player_city_ids(0)
        .into_iter()
        .map(|city_id| g.city_yields(city_id).science)
        .sum::<f64>();
    let culture_before = g.players[0].culture_lifetime;
    assert!(g.complete_item(0, cid, &moon));
    assert!(
        (g.players[0].culture_lifetime - culture_before - 10.0 * science).abs() < 1e-9,
        "Moon Landing grants Culture equal to ten turns of Science"
    );
    assert!(!g.can_produce(0, cid, &exoplanet));
    assert!(g.complete_item(0, cid, &mars));
    assert!(g.can_produce(0, cid, &exoplanet));
    assert!(g.complete_item(0, cid, &exoplanet));
    assert_eq!(g.winner, None, "launching is not the same as arriving");

    let laser = Item::Project {
        project: crate::name!("lagrange_laser_station"),
    };
    assert!(g.complete_item(0, cid, &laser));
    assert_eq!(g.exoplanet_speed(0), 2.0);
    for _ in 0..24 {
        g.advance_exoplanet(0);
    }
    assert_eq!(g.players[0].exoplanet_distance, 48.0);
    assert_eq!(g.winner, None);
    g.advance_exoplanet(0);
    assert_eq!(g.players[0].exoplanet_distance, EXOPLANET_DESTINATION);
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("science"));
}

/// The expedition goes to a real place, and which one is what a space
/// programme buys.
///
/// The Moon landing and the Mars colony were dead ends: the Moon paid a
/// one-off Culture bonus, Mars paid nothing at all, so a civilization
/// racing the science victory correctly skipped straight past both. They
/// are the survey now. The trip is the same length whichever world it is —
/// `EXOPLANET_DESTINATION` is unchanged and deliberately so, because the
/// distances in that roster span eleven to one and turning them loose on
/// the victory race is a balance change that has to be measured over whole
/// games before it ships. What the survey buys today is the world itself.
#[test]
fn the_expedition_goes_where_the_survey_reached() {
    let mut g = game_with_capitals(3, 420, 300);
    let cid = g.player_city_ids(0)[0];
    let project = |name: &str| Item::Project {
        project: Name::new(name),
    };

    // Surveyed nothing, so the only world it can name is the one anybody
    // would already have heard of.
    assert!(g.exoplanet_survey(0).is_empty());
    assert_eq!(g.exoplanet_choice(0), EXOPLANET_DEFAULT_TARGET);

    // The eye above the air finds the first three.
    assert!(g.complete_item(0, cid, &project("launch_earth_satellite")));
    let thin = g.exoplanet_survey(0);
    assert_eq!(thin.len(), 3);

    // And the rest of the programme finds more of them. The order is a
    // fact about the sky, so every civilization finds the same worlds in
    // the same order and a deeper survey is a strict superset of a
    // shallower one — a rival who has looked less can never know a world
    // this one does not.
    assert!(g.complete_item(0, cid, &project("launch_moon_landing")));
    assert!(g.complete_item(0, cid, &project("launch_mars_colony")));
    let deep = g.exoplanet_survey(0);
    assert_eq!(deep.len(), 7);
    assert_eq!(&deep[..3], &thin[..], "a survey only ever adds");

    let rival = g.player_city_ids(1)[0];
    assert!(g.complete_item(1, rival, &project("launch_earth_satellite")));
    assert_eq!(g.exoplanet_survey(1), thin, "one sky, one order");

    // The deeper survey reaches the better world.
    let near = |pid: usize| g.exoplanet_target(pid).grade;
    assert!(
        near(0) >= near(1),
        "seven candidates cannot grade worse than three of the same seven",
    );

    // The choice is made on the day the ship leaves and never revisited: a
    // survey that deepens afterwards does not turn it round, which is what
    // makes finishing the Moon and Mars *before* launching worth anything.
    assert!(g.complete_item(0, cid, &project("exoplanet_expedition")));
    let sent = g.players[0].exoplanet_target.clone();
    assert_eq!(sent.as_deref(), Some(g.exoplanet_choice(0)));
    *g.players[0]
        .counters
        .entry("project:lagrange_laser_station".to_string())
        .or_insert(0) += 3;
    assert!(g.exoplanet_survey(0).len() > 7, "the survey did deepen");
    assert_eq!(
        g.players[0].exoplanet_target, sent,
        "a launched expedition does not change its mind",
    );

    // A different world is a different sky. Two seeds must not agree on the
    // order the neighbourhood is found in, or the survey is a fixed list
    // and the Moon and Mars buy the same thing every game.
    let mut other = game_with_capitals(2, 999_331, 300);
    let there = other.player_city_ids(0)[0];
    assert!(other.complete_item(0, there, &project("launch_earth_satellite")));
    let mut differs = other.exoplanet_survey(0) != thin;
    for seed in [77_003_u64, 5_150_927, 31_337] {
        if differs {
            break;
        }
        let mut world = game_with_capitals(2, seed, 300);
        let city = world.player_city_ids(0)[0];
        assert!(world.complete_item(0, city, &project("launch_earth_satellite")));
        differs = world.exoplanet_survey(0) != thin;
    }
    assert!(differs, "the roster must be shuffled per game");

    // Every world the engine can send an expedition to is one the viewer
    // can draw. The two rosters are written out separately — the client
    // needs positions and palettes the engine has no use for — so the ids
    // are pinned against each other here rather than left to drift into a
    // destination that renders as nothing.
    let client = include_str!("../../web/assets/app.js");
    for target in EXOPLANET_TARGETS.iter() {
        assert!(
            client.contains(&format!("id:\"{}\"", target.id)),
            "{} is not in the viewer's roster",
            target.id,
        );
        assert!(
            client.contains(target.name),
            "{} is not named in the viewer",
            target.name,
        );
    }
}

#[test]
fn spaceport_projects_model_the_missing_late_game_production_stack() {
    let g = game_with_capitals(2, 410, 300);
    let cid = g.player_city_ids(0)[0];
    let launch = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    let repair = Item::Project {
        project: crate::name!("repair_outer_defenses"),
    };

    assert_eq!(g.item_prod_mult(0, cid, Some(&launch)), 2.0);
    assert_eq!(
        g.item_prod_mult(0, cid, Some(&repair)),
        1.0,
        "the production stack applies only to Spaceport projects"
    );
}

#[test]
fn domination_requires_every_original_capital_including_your_own() {
    let capital = |g: &Game, original_owner: usize| {
        g.cities
            .values()
            .find(|c| c.is_capital && c.original_owner == original_owner)
            .unwrap()
            .id
    };

    let mut g = game_with_capitals(3, 402, 300);
    let second = capital(&g, 1);
    let third = capital(&g, 2);
    g.capture_city(second, 0);
    g.do_keep_city(0, second).unwrap();
    assert_eq!(g.winner, None);
    g.capture_city(third, 0);
    g.do_keep_city(0, third).unwrap();
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("domination"));

    // "You must capture all original civilization Capitals" — a conqueror
    // holding both rivals' Capitals but not its own has not, and cannot
    // win until it takes its own back.
    let mut g = game_with_capitals(3, 402, 300);
    let own = capital(&g, 0);
    let second = capital(&g, 1);
    let third = capital(&g, 2);
    g.capture_city(second, 0);
    g.do_keep_city(0, second).unwrap();
    g.capture_city(third, 0);
    g.do_keep_city(0, third).unwrap();
    g.winner = None;
    g.victory_type = None;
    g.capture_city(own, 1);
    g.do_keep_city(1, own).unwrap();
    assert_eq!(g.cities[&own].owner, 1);
    g.check_domination();
    assert_eq!(g.winner, None, "its own Capital is in rival hands");

    g.capture_city(own, 0);
    g.do_keep_city(0, own).unwrap();
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("domination"));
}

#[test]
fn conquest_forces_a_city_fate_and_capitals_cannot_be_razed() {
    let mut g = game_with_capitals(2, 421, 300);
    let noncapital_pos = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let noncapital = g.found_city_for(1, noncapital_pos, Some("Prize".to_string()));
    g.capture_city(noncapital, 0);

    let legal = g.legal_actions(0);
    assert!(legal.contains(&Action::KeepCity { city: noncapital }));
    assert!(legal.contains(&Action::RazeCity { city: noncapital }));
    assert_eq!(legal.len(), 2, "other actions wait for the capture choice");
    let restored: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert!(restored
        .legal_actions(0)
        .contains(&Action::KeepCity { city: noncapital }));
    assert_eq!(restored.cities[&noncapital].occupied_from, Some(1));
    assert!(g.apply(0, &Action::EndTurn).is_err());
    g.apply(0, &Action::KeepCity { city: noncapital }).unwrap();
    assert_eq!(g.cities[&noncapital].captured_from, None);
    assert_eq!(g.cities[&noncapital].occupied_from, Some(1));
    assert_eq!(g.players[1].grievances.get(&0), Some(&50.0));

    let capital = g
        .cities
        .values()
        .find(|city| city.original_owner == 1 && city.is_capital)
        .unwrap()
        .id;
    g.capture_city(capital, 0);
    assert!(!g
        .legal_actions(0)
        .contains(&Action::RazeCity { city: capital }));
    assert!(g.do_raze_city(0, capital).is_err());
}

/// Urban Defenses floors the wall pool at 400 whatever is built, and
/// repair fills to that floor. Finishing another set of Walls then added
/// its rating on top of a pool that was already full, leaving the city
/// holding more wall than the pool allows - 500 against 400.
#[test]
fn finishing_walls_tops_up_the_pool_without_overfilling_it() {
    let mut g = game_with_capitals(2, 4_216, 300);
    let cid = g.player_city_ids(0)[0];
    g.players[0].techs.insert(crate::name!("steel"));
    assert!(g.tree_effect(0, "urban_defenses") > 0.0);

    // Walls and Medieval Walls built, then repaired up to the Steel floor.
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.buildings.push(crate::name!("walls"));
        city.buildings.push(crate::name!("medieval_walls"));
        city.wall_hp = 400;
    }
    let pool = g.city_max_wall_hp(&g.cities[&cid]);
    assert_eq!(pool, 400, "Urban Defenses floors the pool at 400");

    g.complete_item(
        0,
        cid,
        &Item::Building {
            building: crate::name!("renaissance_walls"),
        },
    );
    let city = &g.cities[&cid];
    assert!(city.buildings.iter().any(|b| b == "renaissance_walls"));
    assert_eq!(city.wall_hp, 400, "the pool was already full");
    assert!(city.wall_hp <= g.city_max_wall_hp(city));

    // A city short of its pool still gains the full rating it just built.
    g.cities.get_mut(&cid).unwrap().wall_hp = 150;
    g.complete_item(
        0,
        cid,
        &Item::Building {
            building: crate::name!("walls"),
        },
    );
    assert_eq!(g.cities[&cid].wall_hp, 150);
    let mut fresh = game_with_capitals(2, 4_216, 300);
    let other = fresh.player_city_ids(0)[0];
    fresh.complete_item(
        0,
        other,
        &Item::Building {
            building: crate::name!("walls"),
        },
    );
    assert_eq!(fresh.cities[&other].wall_hp, 100);
}

/// Changing owner always strips a city's constructed Walls, but only a
/// conquest used to empty the pool those Walls filled. A city that
/// defected on Loyalty, or was handed over in a deal, kept a hundred hit
/// points of walls it no longer had - and with them a city ranged strike
/// it had nothing left to fire from.
#[test]
fn a_city_that_defects_loses_the_walls_it_no_longer_has() {
    let mut g = game_with_capitals(2, 4_216, 300);
    let cid = g.player_city_ids(1)[0];
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.buildings.push(crate::name!("walls"));
        city.wall_hp = 100;
        city.encampment_wall_hp = 100;
        city.loyalty = 0.0;
    }
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 100);
    assert!(g.city_can_strike(&g.cities[&cid]));

    // Defect it the way Loyalty does: an ordinary transfer, no conquest.
    g.transfer_city(cid, 0, false);

    let city = &g.cities[&cid];
    assert_eq!(city.owner, 0);
    assert_eq!(city.captured_from, None, "this was not a conquest");
    assert!(!city.buildings.iter().any(|building| building == "walls"));
    assert_eq!(city.wall_hp, 0);
    assert_eq!(city.encampment_wall_hp, 0);
    assert!(city.wall_hp <= g.city_max_wall_hp(city));
    assert!(!g.city_can_strike(city));
}

#[test]
fn conquest_applies_population_damage_repairs_conversion_and_occupation() {
    let mut g = game_with_capitals(2, 4_216, 300);
    g.players[0].civ = "Korea".to_string();
    g.players[1].civ = "Greece".to_string();
    g.players[0]
        .techs
        .extend([crate::name!("writing"), crate::name!("bronze_working")]);
    g.players[0].civics.insert(crate::name!("drama_poetry"));
    let position = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let city = g.found_city_for(1, position, Some("Converted Prize".to_string()));
    let sites: Vec<Pos> = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|site| *site != position)
        .take(4)
        .collect();
    assert_eq!(sites.len(), 4);
    for (district, site) in [
        ("acropolis", sites[0]),
        ("campus", sites[1]),
        ("harbor", sites[2]),
        ("encampment", sites[3]),
    ] {
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), site);
        g.map.tiles.get_mut(&site).unwrap().district = Some(Name::new(district));
    }
    {
        let captured = g.cities.get_mut(&city).unwrap();
        captured.pop = 9;
        captured.wall_hp = 100;
        captured.encampment_hp = 100;
        captured.buildings = [
            "monument",
            "granary",
            "walls",
            "amphitheater",
            "library",
            "lighthouse",
        ]
        .map(Name::new)
        .to_vec();
    }
    g.players[1]
        .counters
        .insert("great_work:writing".to_string(), 1);
    g.map.tiles.get_mut(&sites[0]).unwrap().improvement = Some(crate::name!("sphinx"));
    g.map.tiles.get_mut(&sites[1]).unwrap().improvement = Some(crate::name!("farm"));
    let defender = g.spawn_test_unit("warrior", 1, sites[3]);
    let builder = g.spawn_test_unit("builder", 1, sites[3]);

    g.capture_city(city, 0);

    let captured = &g.cities[&city];
    assert_eq!(
        captured.pop, 7,
        "capture retains the ceiling of 75% Population"
    );
    assert_eq!(captured.occupied_from, Some(1));
    assert_eq!(captured.loyalty, 50.0);
    assert_eq!(captured.wall_hp, 0);
    assert_eq!(captured.encampment_wall_hp, 0);
    assert!(!captured.buildings.contains(&crate::name!("walls")));
    assert!(captured.pillaged_buildings.contains(&Name::new("monument")));
    assert!(captured.pillaged_buildings.contains(&Name::new("granary")));
    assert!(!captured.pillaged_buildings.contains(&Name::new("library")));
    assert!(captured
        .districts
        .contains_key(crate::name!("theater_square")));
    assert!(captured.districts.contains_key(crate::name!("seowon")));
    assert!(captured.districts.contains_key(crate::name!("encampment")));
    assert!(!captured.districts.contains_key(crate::name!("acropolis")));
    assert!(!captured.districts.contains_key(crate::name!("campus")));
    assert!(!captured.districts.contains_key(crate::name!("harbor")));
    assert!(!captured.buildings.contains(&crate::name!("lighthouse")));
    assert_eq!(g.map.tiles[&sites[0]].improvement, None);
    assert_eq!(g.map.tiles[&sites[1]].improvement.as_deref(), Some("farm"));
    assert!(!g.units.contains_key(&defender));
    assert_eq!(g.units[&builder].owner, 0);
    assert_eq!(g.players[1].counters["great_work:writing"], 0);
    assert_eq!(g.players[0].counters["great_work:writing"], 1);

    g.do_keep_city(0, city).unwrap();
    assert_eq!(g.cities[&city].captured_from, None);
    assert_eq!(g.cities[&city].occupied_from, Some(1));

    // The capture left player 1 aggrieved, and Gathering Storm charges
    // 25% of those Grievances (capped at 10) for as long as the city is
    // occupied. A garrison does not cancel that penalty; it pays the
    // separate +8 of Martial Law on top of it.
    let grievances = g.players[1].grievances[&0];
    let mut without_garrison = g.clone();
    let before = without_garrison.cities[&city].loyalty;
    without_garrison.process_loyalty(0);
    let ungarrisoned_gain = without_garrison.cities[&city].loyalty - before;
    let mut unoccupied = g.clone();
    unoccupied.cities.get_mut(&city).unwrap().occupied_from = None;
    let before = unoccupied.cities[&city].loyalty;
    unoccupied.process_loyalty(0);
    assert_eq!(
        unoccupied.cities[&city].loyalty - before,
        ungarrisoned_gain + (0.25 * grievances).clamp(0.0, 10.0),
        "occupation charges 25% of the founder's Grievances"
    );
    g.spawn_test_unit("warrior", 0, position);
    let before = g.cities[&city].loyalty;
    g.process_loyalty(0);
    assert_eq!(g.cities[&city].loyalty - before, ungarrisoned_gain + 8.0);
}

#[test]
fn conquest_removes_non_capturable_empire_unique_districts_and_their_buildings() {
    let mut game = game_with_capitals(2, 4_218, 300);
    game.players[0]
        .techs
        .extend([crate::name!("writing"), crate::name!("mathematics")]);
    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let city = game.found_city_for(1, position, Some("Administrative Prize".to_string()));
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|site| *site != position)
        .take(3)
        .collect();
    for (district, site) in [
        ("government_plaza", sites[0]),
        ("diplomatic_quarter", sites[1]),
        ("campus", sites[2]),
    ] {
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), site);
        game.map.tiles.get_mut(&site).unwrap().district = Some(Name::new(district));
    }
    game.cities.get_mut(&city).unwrap().buildings = ["ancestral_hall", "consulate", "library"]
        .map(Name::new)
        .to_vec();

    game.capture_city(city, 0);

    let captured = &game.cities[&city];
    assert!(!captured
        .districts
        .contains_key(crate::name!("government_plaza")));
    assert!(!captured
        .districts
        .contains_key(crate::name!("diplomatic_quarter")));
    assert!(!captured.buildings.contains(&crate::name!("ancestral_hall")));
    assert!(!captured.buildings.contains(&crate::name!("consulate")));
    assert_eq!(game.map.tiles[&sites[0]].district, None);
    assert_eq!(game.map.tiles[&sites[1]].district, None);
    assert!(captured.districts.contains_key(crate::name!("campus")));
    assert!(captured.buildings.contains(&crate::name!("library")));
    assert_eq!(
        game.map.tiles[&sites[2]].district,
        Some(crate::name!("campus"))
    );
}

#[test]
fn steel_captor_retains_one_quarter_urban_defenses() {
    let mut g = game_with_capitals(2, 4_217, 300);
    g.players[0].techs.insert(crate::name!("steel"));
    let city = g.player_city_ids(1)[0];
    g.capture_city(city, 0);
    assert_eq!(g.cities[&city].wall_hp, 100);
}

#[test]
fn loyalty_states_apply_stock_yield_and_growth_bands() {
    // One step per display band, straight off the shipped LoyaltyLevels
    // rows (YieldChange 0/-0.25/-0.5/-1.0, GrowthChange 1/0.75/0.25/0)
    // for LoyaltyMin..LoyaltyMax of 76-100, 51-75, 26-50 and 0-25.
    for (loyalty, yields, growth) in [
        (100.0, 1.0, 1.0),
        (76.0, 1.0, 1.0),
        (75.0, 0.75, 0.75),
        (51.0, 0.75, 0.75),
        (50.0, 0.50, 0.25),
        (26.0, 0.50, 0.25),
        (25.0, 0.0, 0.0),
        (0.0, 0.0, 0.0),
    ] {
        assert_eq!(Game::loyalty_yield_mult(loyalty), yields);
        assert_eq!(Game::loyalty_growth_mult(loyalty), growth);
    }
    // The display bands themselves keep Civ VI's four names.
    assert_eq!(Game::loyalty_state(76.0), "loyal");
    assert_eq!(Game::loyalty_state(75.0), "wavering");
    assert_eq!(Game::loyalty_state(50.0), "disloyal");
    assert_eq!(Game::loyalty_state(25.0), "unrest");
}

/// The loyalty band multiplies every yield but Food. `LoyaltyLevels.
/// YieldChange` never reaches Food in the host — its per-yield ledgers put
/// "from Disloyal" on culture, faith, gold, production and science and never
/// on food (59 banded city-turns on runs civvis-20260816T040537Z/T045316Z; a
/// city in Unrest read "+7 from Worked Tiles", total 7). Food pays through
/// `loyalty_growth_mult` where it becomes citizens.
#[test]
fn a_disloyal_city_keeps_its_food_and_loses_the_rest() {
    let mut g = game_with_capitals(2, 4_217, 300);
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().loyalty = 100.0;
    let loyal = g.city_yields(cid);
    g.cities.get_mut(&cid).unwrap().loyalty = 10.0;
    let unrest = g.city_yields(cid);
    assert!(
        (loyal.food - unrest.food).abs() < 1e-9,
        "food is not banded: {loyal:?} vs {unrest:?}"
    );
    assert!(loyal.production > 0.0);
    assert!(
        unrest.production.abs() < 1e-9,
        "unrest is -100% on the rest: {unrest:?}"
    );
    assert!(unrest.science.abs() < 1e-9);
}

#[test]
fn an_unemployed_citizen_pays_half_a_gold_and_nothing_else() {
    // Live Rome (run civvis-20260816T200454Z): every workable plot taken,
    // no specialist slot, and the host's Gold ledger read "+0.5 from
    // Population" for one idle citizen, "+1" for two, nothing for none;
    // the other five ledgers never moved. A capital with more citizens
    // than tiles is the same shape here.
    let mut g = game_with_capitals(2, 4_217, 300);
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().loyalty = 100.0;
    // Hold the Amenity band still across the growth by supplying it.
    std::sync::Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, 40);
    let workable = g.city_citizen_plan(cid).worked_tiles.len().max(1);
    let mut pin = |pop: i32| {
        g.cities.get_mut(&cid).unwrap().pop = pop;
        let plan = g.city_citizen_plan(cid);
        (
            plan.worked_tiles.len() + plan.specialists.len(),
            g.city_yields(cid),
        )
    };
    // Grow until the plan can no longer employ everyone.
    let mut pop = workable as i32;
    let (mut employed, mut full) = pin(pop);
    while employed >= pop as usize && pop < 60 {
        pop += 1;
        let (e, y) = pin(pop);
        employed = e;
        full = y;
    }
    assert!(
        employed < pop as usize,
        "the fixture never ran out of tiles at pop {pop}"
    );
    let (employed_more, more) = pin(pop + 2);
    assert_eq!(
        employed_more, employed,
        "two more citizens found no work either"
    );
    // The half-Gold is a base yield, so it wears the city's Amenity band
    // and difficulty handicap like every other Gold.
    let scale = 1.0
        + ((g.amenity_yield_mult(&g.cities[&cid]) - 1.0) * 100.0 + g.handicap_yield_pct(0).gold)
            / 100.0;
    assert!(
        (more.gold - full.gold - 1.0 * scale).abs() < 1e-9,
        "two idle citizens are +1 Gold (x{scale}): {} -> {}",
        full.gold,
        more.gold
    );
    // Science and Culture per citizen are paid whether or not the citizen
    // works (the host's "+6 from Population" Science on a pop-12 Rome
    // with two idle); Food, Production and Faith are not per citizen.
    assert!((more.food - full.food).abs() < 1e-9);
    assert!((more.production - full.production).abs() < 1e-9);
    assert!((more.faith - full.faith).abs() < 1e-9);
    assert!(more.science > full.science);
}

#[test]
fn the_hosts_route_posts_replace_the_straight_line_walk() {
    // Ostia -> Aquileia (run civvis-20260816T200454Z, t144-154) read "+2
    // from Outgoing Trade Routes": the host's road ran through Cumae's
    // post, the model's straight line did not. Where the mirror knows the
    // host's path, its posts pay — a Roman own-city post at the trait's
    // Gold, a foreign one at TRADING_POST_GOLD_IN_FOREIGN_CITY.
    let mut g = game_with_capitals(2, 4_218, 300);
    g.players[0].civ = "Rome".to_string();
    let origin = g.player_city_ids(0)[0];
    let dest = g.player_city_ids(1)[0];
    let walked = g.trading_post_route_gold(0, origin, dest);
    std::sync::Arc::make_mut(&mut g.observed_route_posts).insert((origin, dest), (2, 1));
    let own = g.civ_effect(0, "own_trading_post_route_gold");
    assert!(own > 0.0, "the fixture is Roman");
    assert_eq!(g.trading_post_route_gold(0, origin, dest), 2.0 * own + 1.0);
    // Only that route: nothing is known about the reverse one.
    assert!(!g.observed_route_posts.contains_key(&(dest, origin)));
    std::sync::Arc::make_mut(&mut g.observed_route_posts).clear();
    assert_eq!(g.trading_post_route_gold(0, origin, dest), walked);
}

#[test]
fn capital_citizens_double_their_loyalty_pressure_and_still_decay() {
    let mut game = game_with_capitals(2, 9_311, 300);
    let rival_capital = game.player_city_ids(1)[0];
    let source = game.cities[&rival_capital].pos;
    assert!(game.cities[&rival_capital].is_capital);
    let target_position = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.map.tiles[position].owner_city.is_none())
        .filter(|position| {
            game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
        })
        .filter(|position| (3..=9).contains(&game.wdist(source, *position)))
        .min_by_key(|position| (game.wdist(source, *position), *position))
        .unwrap();
    let distance = game.wdist(source, target_position);
    let target = game.found_city_for(0, target_position, Some("Pressured".to_string()));

    let with_capital = game.loyalty_change_for_city(0, target).foreign_pressure[&1];
    let mut plain = game.clone();
    plain.cities.get_mut(&rival_capital).unwrap().is_capital = false;
    let without_capital = plain.loyalty_change_for_city(0, target).foreign_pressure[&1];

    // A Citizen exerts a base pressure of 1 and a Capital Citizen 2, and
    // the Capital half falls off with distance exactly like the base half
    // rather than arriving undiminished at a tenth of the scale.
    let pop = game.cities[&rival_capital].pop as f64;
    assert_eq!(without_capital, pop * (10.0 - distance as f64));
    assert_eq!(with_capital, 2.0 * without_capital);
}

#[test]
fn razing_removes_the_city_and_triples_capture_grievances() {
    let mut g = game_with_capitals(2, 422, 300);
    let position = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let city = g.found_city_for(1, position, Some("Outpost".to_string()));
    let claimed = g.cities[&city].owned_tiles.clone();
    g.capture_city(city, 0);
    g.do_raze_city(0, city).unwrap();

    assert!(!g.cities.contains_key(&city));
    assert_eq!(g.city_at(position), None);
    assert!(claimed
        .iter()
        .all(|tile| g.map.tiles[tile].owner_city != Some(city)));
    assert_eq!(g.players[1].grievances.get(&0), Some(&150.0));
}

#[test]
fn liberation_restores_the_founder_and_awards_diplomatic_favor() {
    let mut g = game_with_capitals(3, 423, 300);
    let position = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let city = g.found_city_for(1, position, Some("Occupied".to_string()));
    g.capture_city(city, 2);
    g.do_keep_city(2, city).unwrap();
    g.players[1].grievances.insert(0, 40.0);

    g.capture_city(city, 0);
    assert!(g.legal_actions(0).contains(&Action::LiberateCity { city }));
    g.apply(0, &Action::LiberateCity { city }).unwrap();

    assert_eq!(g.cities[&city].owner, 1);
    assert_eq!(g.cities[&city].loyalty, 100.0);
    assert_eq!(g.players[1].grievances.get(&0), None);
    assert_eq!(g.players[0].diplomatic_favor, 50.0);
    assert_eq!(g.players[0].counters.get("cities_liberated"), Some(&1));
}

#[test]
fn occupied_cities_create_recurring_grievances_and_capitals_drain_favor() {
    let mut g = game_with_capitals(2, 424, 300);
    let refuge = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| g.city_at(*position).is_none())
        .unwrap();
    g.found_city_for(1, refuge, Some("Refuge".to_string()));
    let capital = g
        .cities
        .values()
        .find(|city| city.original_owner == 1 && city.is_capital)
        .unwrap()
        .id;
    g.capture_city(capital, 0);
    g.do_keep_city(0, capital).unwrap();
    g.players[0].diplomatic_favor = 10.0;

    g.process_diplomacy(0);

    assert_eq!(g.players[1].grievances.get(&0), Some(&53.0));
    assert!((g.players[0].diplomatic_favor - 4.47).abs() < 1e-9);
}

#[test]
fn the_city_state_and_grievance_thresholds_match_their_parameters() {
    // INFLUENCE_TOKENS_MINIMUM_FOR_SUZERAIN is 3: two Envoys is never
    // enough, three is, and a tie is nobody's.
    let mut g = Game::new_full(2, 26, 16, 91_781, 200, 1, false);
    let minor = g
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("a city-state");
    g.players[0].envoys.clear();
    g.players[1].envoys.clear();
    g.players[0].envoys.push((minor, 2));
    g.query_memo();
    assert_eq!(g.suzerain_of(minor), None, "two Envoys is not Suzerainty");
    g.players[0].envoys.clear();
    g.players[0].envoys.push((minor, 3));
    g.query_memo();
    assert_eq!(g.suzerain_of(minor), Some(0));
    g.players[1].envoys.push((minor, 3));
    g.query_memo();
    assert_eq!(g.suzerain_of(minor), None, "a tie leaves the seat empty");

    // GRIEVANCES_POSSESS_CAPITAL_PER_TURN 3 and _NON_CAPITAL_PER_TURN 1,
    // held as a decay offset. Ancient decay is 10, so holding the victim's
    // original Capital leaves 7 a turn and any other city 9.
    let mut held = game_with_capitals(2, 4_241, 300);
    held.world_era = 0;
    let capital = held
        .cities
        .values()
        .find(|city| city.original_owner == 0 && city.is_capital)
        .map(|city| city.id)
        .unwrap();
    held.players[0].grievances.insert(1, 100.0);
    held.process_diplomacy(0);
    assert_eq!(
        held.players[0].grievances.get(&1),
        Some(&90.0),
        "no city held"
    );
    held.cities.get_mut(&capital).unwrap().owner = 1;
    held.players[0].grievances.insert(1, 100.0);
    held.process_diplomacy(0);
    assert_eq!(
        held.players[0].grievances.get(&1),
        Some(&93.0),
        "Capital held"
    );
}

#[test]
fn grievance_decay_pauses_at_war_and_uses_capital_occupation_modifier() {
    let mut g = game_with_capitals(2, 4_241, 300);
    let capital = g
        .cities
        .values()
        .find(|city| city.original_owner == 0 && city.is_capital)
        .unwrap()
        .id;
    g.cities.get_mut(&capital).unwrap().owner = 1;
    g.players[0].grievances.insert(1, 100.0);
    g.world_era = 0;
    g.process_diplomacy(0);
    assert_eq!(g.players[0].grievances.get(&1), Some(&93.0));

    g.players[0].grievances.insert(1, 100.0);
    g.at_war.insert(pair(0, 1));
    g.process_diplomacy(0);
    assert_eq!(g.players[0].grievances.get(&1), Some(&100.0));
}

#[test]
fn final_civilization_capture_creates_world_grievances() {
    let mut g = game_with_capitals(3, 4_242, 300);
    let city = g.player_city_ids(1)[0];
    g.capture_city(city, 0);
    g.do_keep_city(0, city).unwrap();
    assert!(!g.players[1].alive);
    assert_eq!(g.players[2].grievances.get(&0), Some(&150.0));
}

#[test]
fn original_capitals_revolt_to_free_cities_before_joining_a_rival() {
    let mut g = game_with_capitals(2, 4_243, 300);
    let capital = g.player_city_ids(0)[0];
    let foreign = g.player_city_ids(1)[0];
    let neighbor = g.nbrs(g.cities[&capital].pos)[0];
    g.cities.get_mut(&foreign).unwrap().pos = neighbor;
    g.cities.get_mut(&foreign).unwrap().pop = 20;
    g.cities.get_mut(&capital).unwrap().loyalty = 1.0;

    g.process_loyalty(0);

    let free_cities = g
        .players
        .iter()
        .find(|player| player.is_free_city)
        .unwrap()
        .id;
    assert_eq!(g.cities[&capital].owner, free_cities);
    assert_eq!(g.cities[&capital].loyalty, 100.0);
    assert_eq!(g.cities[&capital].occupied_from, None);
    assert!(g.players[free_cities].alive);
    assert!(g.is_at_war(free_cities, 1));
    assert_eq!(
        g.player_unit_ids(free_cities)
            .into_iter()
            .filter(|unit| { g.rules.units[g.units[unit].kind].promotion_class == "melee" })
            .count(),
        2
    );

    for _ in 0..20 {
        if g.cities[&capital].owner != free_cities {
            break;
        }
        g.process_loyalty(free_cities);
    }
    assert_eq!(g.cities[&capital].owner, 1);
    assert_eq!(g.cities[&capital].loyalty, 100.0);
    assert!(g.cities[&capital].free_city_pressure.is_empty());
    assert!(!g.players[free_cities].alive);
}

#[test]
fn a_city_state_weighs_its_own_citizens_and_carries_its_base_strength() {
    let mut g = game_with_capitals(2, 4_245, 300);
    g.players[1].is_minor = true;
    let state = g.player_city_ids(1)[0];
    let major = g.player_city_ids(0)[0];
    let neighbor = g.nbrs(g.cities[&state].pos)[0];
    g.cities.get_mut(&major).unwrap().pos = neighbor;
    g.cities.get_mut(&major).unwrap().pop = 6;
    g.cities.get_mut(&state).unwrap().pop = 6;

    // Its own Citizens are the whole domestic side of the balance. Before
    // they counted, a city-state compared 0 against its neighbours and read
    // a permanent -20 that nothing ever applied. Here it holds its ground
    // against an equally sized Capital one tile away, whose Citizens press
    // at double rate, and the +20 base carries it clear.
    let change = g.city_loyalty_per_turn(&g.cities[&state].clone());
    assert!(
        change > 0.0,
        "a city-state that counts nothing for itself reads a permanent -20; got {change}"
    );
    // And that domestic side really is its own Population.
    g.cities.get_mut(&state).unwrap().pop = 12;
    assert!(
        g.city_loyalty_per_turn(&g.cities[&state].clone()) > change,
        "its own Citizens have to move the balance"
    );

    // And it is not exempt from the rules: swamp it and it revolts into a
    // Free City exactly as a major's city does. Foreign pressure alone tops
    // out at the -20 the +20 base cancels, so the push over the edge is the
    // Unrest amenity band (-6), the way a starving, miserable city goes —
    // its Food is not banded by loyalty (the host never touches Food), so
    // starvation cannot be manufactured from loyalty 1 alone.
    g.cities.get_mut(&major).unwrap().pop = 60;
    g.cities.get_mut(&state).unwrap().pop = 1;
    g.cities.get_mut(&state).unwrap().loyalty = 1.0;
    g.players[1].bankruptcy_amenity_penalty = 6;
    g.process_loyalty(1);
    let free_cities = g
        .players
        .iter()
        .find(|player| player.is_free_city)
        .unwrap()
        .id;
    assert_eq!(g.cities[&state].owner, free_cities);
    assert_eq!(g.cities[&state].loyalty, 100.0);
}

#[test]
fn a_minor_city_projects_no_pressure_onto_its_neighbours() {
    let mut g = game_with_capitals(3, 4_246, 300);
    g.players[2].is_minor = true;
    let target = g.player_city_ids(0)[0];
    let minor = g.player_city_ids(2)[0];
    let neighbor = g.nbrs(g.cities[&target].pos)[0];
    g.cities.get_mut(&minor).unwrap().pos = neighbor;

    let baseline = g.city_loyalty_per_turn(&g.cities[&target].clone());
    g.cities.get_mut(&minor).unwrap().pop = 40;
    assert_eq!(
        g.city_loyalty_per_turn(&g.cities[&target].clone()),
        baseline,
        "a city-state next door must not press a major's city"
    );
}

#[test]
fn free_cities_join_the_civilization_with_most_accumulated_pressure() {
    let mut g = game_with_capitals(3, 4_244, 300);
    let target = g.player_city_ids(0)[0];
    let first_rival = g.player_city_ids(1)[0];
    let second_rival = g.player_city_ids(2)[0];
    let target_position = g.cities[&target].pos;
    let nearby = g.nbrs(target_position)[0];
    let distant = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| g.wdist(target_position, *position) > 9)
        .unwrap();

    // The first rival triggers the initial revolt. Pressure is only
    // accumulated while the city is independent, so this does not give
    // that rival a head start in the Free City ledger.
    g.cities.get_mut(&first_rival).unwrap().pos = nearby;
    g.cities.get_mut(&first_rival).unwrap().pop = 20;
    g.cities.get_mut(&second_rival).unwrap().pos = distant;
    g.cities.get_mut(&target).unwrap().loyalty = 1.0;
    g.process_loyalty(0);
    let free_cities = g
        .players
        .iter()
        .find(|player| player.is_free_city)
        .unwrap()
        .id;
    assert_eq!(g.cities[&target].owner, free_cities);

    // Civilization 2 dominates for two turns, then civilization 1 is the
    // only current pressure source on the turn Loyalty reaches zero.
    // The prior accumulated pressure must still decide the destination.
    g.cities.get_mut(&first_rival).unwrap().pos = distant;
    g.cities.get_mut(&second_rival).unwrap().pos = nearby;
    g.cities.get_mut(&second_rival).unwrap().pop = 20;
    g.process_loyalty(free_cities);
    g.process_loyalty(free_cities);
    assert!(
        g.cities[&target].free_city_pressure[&2]
            > g.cities[&target]
                .free_city_pressure
                .get(&1)
                .copied()
                .unwrap_or(0.0)
    );

    g.cities.get_mut(&second_rival).unwrap().pos = distant;
    g.cities.get_mut(&first_rival).unwrap().pos = nearby;
    g.cities.get_mut(&target).unwrap().loyalty = 1.0;
    g.process_loyalty(free_cities);

    assert_eq!(g.cities[&target].owner, 2);
    assert_eq!(g.cities[&target].loyalty, 100.0);
    assert!(g.cities[&target].free_city_pressure.is_empty());
}

#[test]
fn eliminating_a_city_state_angers_other_majors_and_liberation_restores_it() {
    let mut g = Game::new_full(2, 26, 16, 425, 300, 1, false);
    let minor = g
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    // City-state elimination grievances are public only to leaders that
    // had already made contact with that city-state.
    g.record_contact(1, minor);
    let city = g.player_city_ids(minor)[0];
    g.capture_city(city, 0);
    assert!(!g
        .pending_city_capture_actions(0)
        .contains(&Action::RazeCity { city }));
    g.do_keep_city(0, city).unwrap();
    assert!(!g.players[minor].alive);
    assert_eq!(g.players[1].grievances.get(&0), Some(&50.0));

    g.players[1].techs.insert(crate::name!("steel"));
    assert!(!g.players[minor].techs.contains(&crate::name!("steel")));
    g.capture_city(city, 1);
    assert_eq!(g.cities[&city].wall_hp, 100);
    assert!(g
        .pending_city_capture_actions(1)
        .contains(&Action::LiberateCity { city }));
    g.do_liberate_city(1, city).unwrap();
    assert!(g.players[minor].alive);
    assert_eq!(g.cities[&city].owner, minor);
    assert_eq!(g.city_max_wall_hp(&g.cities[&city]), 0);
    assert_eq!(g.cities[&city].wall_hp, 0);
    assert_eq!(g.players[1].diplomatic_favor, 100.0);
    // Ancient-era liberation grants the shipped two Envoys, and two is
    // below the three a Suzerain needs -- so an early liberation buys
    // influence but not control. Both numbers are shipped values, so the
    // outcome is their conjunction rather than a choice made here; from
    // the Renaissance on, the six Envoys do carry suzerainty.
    assert_eq!(g.envoys_at(1, minor), 2);
    assert_eq!(g.suzerain_of(minor), None);
}

#[test]
fn city_state_liberation_envoys_and_border_growth_scale_by_world_era() {
    // Shipped Eras_XP1.LiberatedEnvoys: the Ancient era grants 2, not 3.
    for (era, expected) in [(0, 2), (1, 3), (2, 3), (3, 6), (4, 6), (5, 9), (8, 9)] {
        let mut game = Game::new_full(2, 26, 16, 42_500 + era as u64, 300, 1, false);
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        let city = game.player_city_ids(minor)[0];
        game.capture_city(city, 0);
        game.do_keep_city(0, city).unwrap();
        game.capture_city(city, 1);
        game.world_era = era;
        let tiles = game.cities[&city].owned_tiles.len();

        game.do_liberate_city(1, city).unwrap();

        assert_eq!(game.envoys_at(1, minor), expected, "world era {era}");
        assert_eq!(
            game.cities[&city].owned_tiles.len(),
            tiles + expected as usize,
            "liberation Envoys expand borders in world era {era}"
        );
    }
}

#[test]
fn religion_must_be_a_strict_majority_in_every_living_civ() {
    let mut g = game_with_capitals(2, 403, 300);
    let extra_pos = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let extra = g.found_city_for(1, extra_pos, None);
    let religion = "Test Religion".to_string();
    g.players[0].religion = Some(religion.clone());
    let own = g.player_city_ids(0)[0];
    let rival: Vec<u32> = g.player_city_ids(1);
    g.cities
        .get_mut(&own)
        .unwrap()
        .pressure
        .insert(religion.clone(), 100.0);
    g.cities
        .get_mut(&rival[0])
        .unwrap()
        .pressure
        .insert(religion.clone(), 100.0);
    g.check_religious_victory();
    assert_eq!(g.winner, None, "one of two rival cities is not a majority");
    g.cities
        .get_mut(&extra)
        .unwrap()
        .pressure
        .insert(religion.clone(), 100.0);
    g.check_religious_victory();
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("religious"));

    // The Civilopedia asks for "every other major civilization", so the
    // founder's own cities are not part of the test: losing the home
    // religion while every rival stays converted still wins.
    let mut g = game_with_capitals(2, 403, 300);
    let extra_pos = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let extra = g.found_city_for(1, extra_pos, None);
    g.players[0].religion = Some(religion.clone());
    let own = g.player_city_ids(0)[0];
    for city in [g.player_city_ids(1)[0], extra] {
        g.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert(religion.clone(), 100.0);
    }
    g.cities
        .get_mut(&own)
        .unwrap()
        .pressure
        .insert("Rival Creed".to_string(), 400.0);
    assert_ne!(
        g.city_religion(&g.cities[&own]),
        Some(religion.as_str()),
        "the founder's own capital has gone over to another religion"
    );
    g.check_religious_victory();
    assert_eq!(g.winner, Some(0));
}

/// A lobby with Religious Victory off can never crown a conversion, so
/// the per-turn conversion sweep is skipped outright. The gate sits
/// exactly where `set_winner`'s own refusal already drew the line: the
/// same fully converted world crowns nobody with the checkbox off and
/// the founder with it on.
#[test]
fn a_disabled_religious_victory_skips_the_sweep_and_crowns_nobody() {
    let mut g = game_with_capitals(2, 403, 300);
    let extra_pos = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.city_at(*pos).is_none())
        .unwrap();
    let extra = g.found_city_for(1, extra_pos, None);
    let religion = "Test Religion".to_string();
    g.players[0].religion = Some(religion.clone());
    for city in [g.player_city_ids(1)[0], extra] {
        g.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert(religion.clone(), 100.0);
    }
    g.victory_conditions.religious = false;
    g.check_religious_victory();
    assert_eq!(g.winner, None, "a switched-off victory path stays off");

    g.victory_conditions.religious = true;
    g.check_religious_victory();
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("religious"));
}

#[test]
fn the_last_civilization_standing_does_not_convert_itself_to_victory() {
    // An empty opponent list satisfies "every other civilization"
    // vacuously. Being alone is a conquest, and it must not read as a
    // conversion in a lobby that has domination switched off.
    let mut g = game_with_capitals(2, 4_031, 300);
    g.victory_conditions.domination = false;
    g.players[0].religion = Some("Test Religion".to_string());
    g.players[1].alive = false;
    g.check_religious_victory();
    assert_eq!(g.winner, None);
}

#[test]
fn culture_requires_more_visiting_tourists_than_the_best_rival_domestic_total() {
    let mut g = game_with_capitals(2, 404, 300);
    g.players[1].culture_lifetime = 1_000.0;
    g.players[0].tourism_lifetime = 2_000.0;
    assert_eq!(g.domestic_tourists(1), 5);
    assert_eq!(g.foreign_tourists(0), 5);
    g.check_culture_victory();
    assert_eq!(g.winner, None, "a tie in tourist counts is not a victory");
    g.players[0].tourism_lifetime = 2_400.0;
    assert_eq!(g.domestic_tourists(1), 4);
    assert_eq!(g.foreign_tourists(0), 6);
    g.check_culture_victory();
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("culture"));
}

#[test]
fn tourists_are_discrete_per_rival_and_vanish_with_an_eliminated_source() {
    let mut g = game_with_capitals(3, 4_040, 300);
    g.players[0].tourism_pressure.insert(1, 599.0);
    g.players[0].tourism_pressure.insert(2, 599.0);
    assert_eq!(
        g.foreign_tourists(0),
        0,
        "fractional tourists from separate civilizations cannot combine"
    );

    g.players[0].tourism_pressure.insert(1, 600.0);
    g.players[1].culture_lifetime = 500.0;
    assert_eq!(g.foreign_tourists(0), 1);
    assert_eq!(g.domestic_tourists(1), 4);

    g.players[1].alive = false;
    assert_eq!(g.foreign_tourists(0), 0);
}

#[test]
fn cristo_only_cancels_enlightenments_religious_tourism_reduction() {
    let mut g = game_with_capitals(2, 4_041, 300);
    let holy_city = g.player_city_ids(0)[0];
    g.players[0].holy_city = Some(holy_city);
    g.players[0]
        .counters
        .insert("great_work:relic".to_string(), 1);
    let wonder_position = g.cities[&holy_city].pos;
    g.cities
        .get_mut(&holy_city)
        .unwrap()
        .wonders
        .insert(crate::name!("st_basils_cathedral"), wonder_position);
    assert_eq!(g.religious_tourism_per_turn(0), 24.0);

    g.players[0].tourism_lifetime = 4_000.0;
    g.players[0].religious_tourism_lifetime = 4_000.0;
    g.players[1]
        .civics
        .insert(crate::name!("the_enlightenment"));
    assert_eq!(g.foreign_tourists(0), 5);

    g.cities
        .get_mut(&holy_city)
        .unwrap()
        .wonders
        .insert(crate::name!("cristo_redentor"), wonder_position);
    assert_eq!(g.foreign_tourists(0), 10);

    g.cities
        .get_mut(&holy_city)
        .unwrap()
        .wonders
        .remove(&Name::new("cristo_redentor"));
    g.players[0].religious_tourism_lifetime = 2_000.0;
    assert_eq!(
        g.foreign_tourists(0),
        7,
        "Cristo must not increase the secular half of lifetime Tourism"
    );
}

#[test]
fn international_tourism_modifiers_accumulate_per_rival_without_retroactivity() {
    let mut g = game_with_capitals(3, 4_042, 300);
    g.turn = 10;
    g.players[0].government = Some("democracy".to_string());
    g.players[1].government = Some("communism".to_string());
    g.players[2].government = Some("merchant_republic".to_string());
    g.players[0].religion = Some("source_faith".to_string());
    g.players[1].religion = Some("rival_faith".to_string());
    g.players[2].religion = Some("source_faith".to_string());
    g.players[1]
        .civics
        .insert(crate::name!("the_enlightenment"));

    // Granting our borders to them is deliberately the wrong direction
    // and does not improve our Tourism pressure against them.
    g.players[0].open_borders_until.insert(1, 40);
    assert_eq!(g.international_tourism_multiplier(0, 1, false), 0.6);
    g.players[1].open_borders_until.insert(0, 40);
    assert_eq!(g.international_tourism_multiplier(0, 1, false), 0.85);

    let origin = g.player_city_ids(0)[0];
    let destination = g.player_city_ids(1)[0];
    g.routes.push(TradeRoute {
        origin,
        dest: destination,
        owner: 0,
        ends: 40,
    });
    assert_eq!(g.international_tourism_multiplier(0, 1, false), 1.1);
    g.players[0]
        .policies
        .insert(crate::name!("online_communities"));
    assert_eq!(g.international_tourism_multiplier(0, 1, false), 1.6);
    assert!((g.international_tourism_multiplier(0, 1, true) - 0.6).abs() < 1e-9);

    g.add_passive_tourism(0, 100.0, 40.0, 0.0, 0.0);
    assert!((g.players[0].tourism_pressure[&1] - 120.0).abs() < 1e-9);
    assert!((g.players[0].tourism_pressure[&2] - 80.0).abs() < 1e-9);

    // Expiring favorable modifiers affects only new pressure. It cannot
    // rewrite the 120 points already accumulated against player 1.
    g.players[0]
        .policies
        .remove(&Name::new("online_communities"));
    g.players[1].open_borders_until.clear();
    g.routes.clear();
    g.players[1].government = Some("democracy".to_string());
    g.add_passive_tourism(0, 100.0, 40.0, 0.0, 0.0);
    assert!((g.players[0].tourism_pressure[&1] - 180.0).abs() < 1e-9);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(
        restored.players[0].tourism_pressure,
        g.players[0].tourism_pressure
    );
}

#[test]
fn film_studio_bonus_applies_only_against_each_modern_era_rival() {
    let mut g = game_with_capitals(3, 4_043, 300);
    let city = g.player_city_ids(0)[0];
    install_test_district(&mut g, city, "theater_square");
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("amphitheater"), crate::name!("film_studio")]);
    g.players[0]
        .counters
        .insert("great_work:writing".to_string(), 1);
    g.players[0].holy_city = Some(city);
    g.players[1].techs.insert(crate::name!("radio"));

    let (tourism, film_studio_tourism) = g.tourism_components_per_turn(0);
    let (religious, film_studio_religious) = g.religious_tourism_components_per_turn(0);
    assert!(film_studio_tourism > 0.0);
    assert!(film_studio_religious > 0.0);
    g.add_passive_tourism(
        0,
        tourism,
        religious,
        film_studio_tourism,
        film_studio_religious,
    );

    assert!((g.players[0].tourism_pressure[&1] - tourism - film_studio_tourism).abs() < 1e-9);
    assert!((g.players[0].tourism_pressure[&2] - tourism).abs() < 1e-9);
    assert_eq!(g.players[0].tourism_lifetime, tourism);
}

#[test]
fn great_work_tourism_uses_tree_and_slotted_policy_modifiers() {
    let mut g = game_with_capitals(2, 412, 300);
    let city = g.player_city_ids(0)[0];
    install_test_district(&mut g, city, "theater_square");
    g.cities.get_mut(&city).unwrap().buildings = vec![
        crate::name!("amphitheater"),
        crate::name!("art_museum"),
        crate::name!("broadcast_center"),
    ];
    g.players[0].gp_claimed.insert("artist".to_string(), 2);

    let base = g.tourism_per_turn(0);
    g.players[0].techs.insert(crate::name!("printing"));
    let printing = g.tourism_per_turn(0);
    assert!(
        (printing - base - 4.0).abs() < 1e-9,
        "base={base}, printing={printing}"
    );

    g.players[0]
        .policies
        .insert(crate::name!("heritage_tourism"));
    let heritage = g.tourism_per_turn(0);
    // Heritage Tourism doubles three works of art at their shipped 2.
    assert!(
        (heritage - printing - 6.0).abs() < 1e-9,
        "printing={printing}, heritage={heritage}, policy={}, art={}, global={}, housed={:?}",
        g.policy_effect(0, "art_artifact_tourism_pct"),
        g.great_work_tourism(0, "art"),
        g.tree_effect(0, "tourism_pct") + g.monopoly_bonuses(0).1,
        g.housed_great_works(0),
    );

    g.players[0]
        .policies
        .insert(crate::name!("satellite_broadcasts"));
    let broadcasts = g.tourism_per_turn(0);
    assert!(
        (broadcasts - heritage - 8.0).abs() < 1e-9,
        "heritage={heritage}, broadcasts={broadcasts}"
    );
}

#[test]
fn diplomacy_requires_twenty_victory_points() {
    let mut g = Game::new_full(2, 26, 16, 405, 300, 1, false);
    g.players[0].dvp = 17;
    g.world_era = 5;
    g.turn = 30;
    g.process_congress();
    assert!(g.congress.is_some(), "the session remains open for voting");
    let score_before = g.players[0].era_score;
    g.do_congress_vote(0, "world_leader", "A:0", 1).unwrap();
    g.turn = 35;
    g.process_congress();
    assert_eq!(g.players[0].dvp, DIPLOMATIC_VICTORY_POINTS);
    assert_eq!(g.players[0].era_score, score_before + 2);
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("diplomatic"));
}

#[test]
fn congress_tallies_outcome_before_target_and_refunds_partial_misses() {
    let mut g = game_with_capitals(2, 413, 300);
    g.turn = 30;
    g.players[0].diplomatic_favor = 30.0;
    g.players[1].diplomatic_favor = 30.0;
    g.congress = Some(CongressSession {
        convened: 30,
        closes: 35,
        resolutions: vec![CongressResolution {
            id: "urban_development_treaty".to_string(),
            title: "Urban Development Treaty".to_string(),
            choices: vec![
                "A:campus".to_string(),
                "A:theater_square".to_string(),
                "B:campus".to_string(),
                "B:theater_square".to_string(),
            ],
            ballots: BTreeMap::new(),
        }],
    });
    g.do_congress_vote(0, "urban_development_treaty", "A:campus", 2)
        .unwrap();
    g.do_congress_vote(1, "urban_development_treaty", "A:theater_square", 2)
        .unwrap();
    g.turn = 35;
    g.process_congress();
    assert_eq!(g.players[0].dvp, 1);
    assert_eq!(g.players[1].dvp, 0);
    assert_eq!(g.players[0].diplomatic_favor, 20.0);
    assert_eq!(g.players[1].diplomatic_favor, 25.0);
    assert!(g.congress_effect_active("urban_development_treaty", "A", "campus"));
}

#[test]
fn online_congress_uses_the_host_cost_curve_for_actions_and_refunds() {
    use crate::setup::GameSpeed;

    let mut g = game_with_capitals(2, 4_150, 300);
    g.game_speed = GameSpeed::Online;
    g.turn = 30;
    g.players[0].diplomatic_favor = 352.0;
    g.players[1].diplomatic_favor = 1_000.0;
    g.congress = Some(CongressSession {
        convened: 30,
        closes: 35,
        resolutions: vec![CongressResolution {
            id: "urban_development_treaty".to_string(),
            title: "Urban Development Treaty".to_string(),
            choices: vec!["A:campus".to_string(), "A:theater_square".to_string()],
            ballots: BTreeMap::new(),
        }],
    });

    assert_eq!(g.congress_vote_cost(13), 312.0);
    assert_eq!(g.congress_vote_cost(14), 364.0);
    assert_eq!(g.congress_affordable_votes(0), 13);
    assert_eq!(g.congress_affordable_votes(1), 22);
    assert!(
        g.legal_actions(0).contains(&Action::CongressVote {
            resolution: crate::name!("urban_development_treaty"),
            choice: "A:campus".to_string(),
            votes: 13,
        }),
        "the host-affordable ballot must enter the action space"
    );

    g.do_congress_vote(0, "urban_development_treaty", "A:campus", 13)
        .unwrap();
    assert_eq!(g.players[0].diplomatic_favor, 40.0);
    g.do_congress_vote(1, "urban_development_treaty", "A:theater_square", 14)
        .unwrap();
    g.turn = 35;
    g.process_congress();
    assert_eq!(
        g.players[0].diplomatic_favor, 196.0,
        "a right-outcome, wrong-target ballot refunds half of the same 312 Favor"
    );

    g.game_speed = GameSpeed::Standard;
    assert_eq!(g.congress_vote_cost(13), 780.0);
}

#[test]
fn congress_generates_two_stock_outcome_target_proposals_by_era() {
    let mut g = game_with_capitals(3, 4_131, 300);
    g.world_era = 2;
    g.turn = 30;
    g.process_congress();
    let medieval = &g.congress.as_ref().unwrap().resolutions;
    assert_eq!(medieval.len(), 2);
    for resolution in medieval {
        assert!(resolution
            .choices
            .iter()
            .any(|choice| choice.starts_with("A:")));
        assert!(resolution
            .choices
            .iter()
            .any(|choice| choice.starts_with("B:")));
    }

    g.congress = None;
    g.world_era = 5;
    g.turn = 60;
    g.process_congress();
    let modern = &g.congress.as_ref().unwrap().resolutions;
    assert_eq!(modern.len(), 3);
    assert!(modern.iter().any(|resolution| {
        resolution.id == "world_leader"
            && resolution.choices.contains(&"A:0".to_string())
            && resolution.choices.contains(&"B:0".to_string())
    }));
}

#[test]
fn a_stale_emergency_queue_does_not_cost_the_world_its_regular_session() {
    let mut g = game_with_capitals(3, 4_133, 300);
    g.world_era = 2;
    g.turn = 30;
    // Queued against a city that no longer exists, so the proposal is
    // dropped rather than seated. Nothing displaces the regular session.
    g.pending_emergencies.push(EmergencyProposal {
        id: 1,
        kind: "military".to_string(),
        target: 1,
        city: u32::MAX,
        original_owner: 2,
        eligible: BTreeSet::from([0, 2]),
        requested: 30,
    });
    g.process_congress();
    assert!(
        g.pending_emergencies.is_empty(),
        "the stale proposal is dropped"
    );
    let session = g
        .congress
        .as_ref()
        .expect("the regular session is still due");
    assert_eq!(session.resolutions.len(), 2);
    assert!(session
        .resolutions
        .iter()
        .all(|resolution| !resolution.title.contains("Emergency")));
}

#[test]
fn enacted_congress_rules_change_core_economy_combat_and_diplomacy() {
    let mut g = game_with_capitals(2, 4_132, 300);
    let city = g.player_city_ids(0)[0];
    let rival_city = g.player_city_ids(1)[0];
    let effect = |resolution: &str, outcome: &str, target: &str| CongressEffect {
        resolution: resolution.to_string(),
        outcome: outcome.to_string(),
        target: target.to_string(),
        expires: 100,
    };

    let warrior = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "warrior")
        .unwrap();
    let strength = g.unit_strength(&g.units[&warrior], false);
    g.active_congress_effects
        .push(effect("military_advisory", "A", "melee"));
    assert_eq!(g.unit_strength(&g.units[&warrior], false), strength + 5.0);

    let warrior_item = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let normal_cost = g.item_cost_for_city(0, city, &warrior_item);
    g.active_congress_effects
        .push(effect("mercenary_companies", "B", "production"));
    assert_eq!(
        g.item_cost_for_city(0, city, &warrior_item),
        normal_cost * 0.5
    );

    // Trade Policy A pays the CHOSEN player's destination city, not the
    // sender: the shipped effect is `EFFECT_ADJUST_TRADE_ROUTE_YIELD_FROM_OTHERS`
    // on the chosen player, and the host's city ledger reads "+4 from
    // Incoming Trade Routes" per foreign route.
    let route_gold = g.route_yields(rival_city, false).gold;
    let rival_gold_before = g.city_yields(rival_city).gold;
    g.routes.push(TradeRoute {
        origin: city,
        dest: rival_city,
        owner: 0,
        ends: g.turn + 30,
    });
    let rival_gold_with_route = g.city_yields(rival_city).gold;
    g.active_congress_effects
        .push(effect("trade_policy", "A", "1"));
    assert_eq!(g.route_yields(rival_city, false).gold, route_gold);
    assert_eq!(g.city_yields(rival_city).gold, rival_gold_with_route + 4.0);
    assert!(rival_gold_with_route >= rival_gold_before);
    g.routes.pop();
    assert_eq!(g.city_yields(rival_city).gold, rival_gold_before);

    let writing = g.great_work_tourism(0, "writing");
    g.active_congress_effects
        .push(effect("heritage_organization", "A", "writing"));
    assert_eq!(g.great_work_tourism(0, "writing"), writing * 2.0);

    g.active_congress_effects
        .push(effect("public_relations", "A", "0"));
    g.record_contact(0, 1);
    g.do_denounce(0, 1).unwrap();
    assert_eq!(g.players[1].grievances[&0], 50.0);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(restored.active_congress_effects, g.active_congress_effects);
    assert!(restored.congress_effect_active("trade_policy", "A", "1"));
}

#[test]
fn remaining_stock_congress_candidates_obey_their_era_windows() {
    let mut g = game_with_capitals(2, 4_133, 300);
    for player in g.players.iter_mut().take(2) {
        player.government = Some("democracy".to_string());
    }
    let feature_tile = *g.map.tiles.keys().next().unwrap();
    g.map.tiles.get_mut(&feature_tile).unwrap().feature = Some(crate::name!("forest"));

    g.world_era = 5;
    let modern: BTreeSet<String> = g
        .regular_congress_candidates()
        .into_iter()
        .map(|resolution| resolution.id)
        .collect();
    assert!(modern.contains("border_control_treaty"));
    assert!(modern.contains("world_ideology"));
    assert!(modern.contains("global_energy_treaty"));
    assert!(!modern.contains("arms_control"));
    assert!(!modern.contains("public_works_program"));
    assert!(!modern.contains("deforestation_treaty"));

    g.world_era = 6;
    let atomic: BTreeSet<String> = g
        .regular_congress_candidates()
        .into_iter()
        .map(|resolution| resolution.id)
        .collect();
    assert!(!atomic.contains("border_control_treaty"));
    assert!(atomic.contains("world_ideology"));
    assert!(atomic.contains("global_energy_treaty"));
    assert!(atomic.contains("arms_control"));
    assert!(atomic.contains("public_works_program"));
    assert!(atomic.contains("deforestation_treaty"));

    g.world_era = 8;
    let future: BTreeSet<String> = g
        .regular_congress_candidates()
        .into_iter()
        .map(|resolution| resolution.id)
        .collect();
    assert!(future.contains("arms_control"));
    assert!(!future.contains("public_works_program"));
    assert!(!future.contains("deforestation_treaty"));
}

#[test]
fn late_congress_rules_change_wmd_policy_projects_energy_borders_and_chops() {
    let mut g = game_with_capitals(2, crate::rng::fixture_seed("CONGRESS", 4_135), 300);
    let city = g.player_city_ids(0)[0];
    let effect = |resolution: &str, outcome: &str, target: &str| CongressEffect {
        resolution: resolution.to_string(),
        outcome: outcome.to_string(),
        target: target.to_string(),
        expires: 100,
    };

    g.players[0].government = Some("autocracy".to_string());
    let wildcard_slots = g.gov_slots(0).wildcard;
    g.active_congress_effects
        .push(effect("world_ideology", "A", "autocracy"));
    assert_eq!(g.gov_slots(0).wildcard, wildcard_slots + 1);
    g.active_congress_effects.clear();
    g.players[0].policies.insert(crate::name!("strategos"));
    g.active_congress_effects
        .push(effect("world_ideology", "B", "autocracy"));
    assert_eq!(g.gov_slots(0).wildcard, wildcard_slots.saturating_sub(1));
    g.trim_policies_to_slots(0);
    assert!(!g.players[0].policies.contains(&crate::name!("strategos")));

    g.active_congress_effects.clear();
    let project = Item::Project {
        project: crate::name!("manhattan_project"),
    };
    let normal_project_mult = g.item_prod_mult(0, city, Some(&project));
    g.active_congress_effects
        .push(effect("public_works_program", "A", "manhattan_project"));
    assert_eq!(
        g.item_prod_mult(0, city, Some(&project)),
        normal_project_mult + 1.0
    );
    g.active_congress_effects.clear();
    g.active_congress_effects
        .push(effect("public_works_program", "B", "manhattan_project"));
    assert_eq!(
        g.item_prod_mult(0, city, Some(&project)),
        normal_project_mult - 0.5
    );

    g.active_congress_effects.clear();
    let plant = Item::Building {
        building: crate::name!("coal_power_plant"),
    };
    let normal_plant_cost = g.item_cost_for_city(0, city, &plant);
    g.active_congress_effects
        .push(effect("global_energy_treaty", "A", "coal_power_plant"));
    assert_eq!(
        g.item_cost_for_city(0, city, &plant),
        normal_plant_cost * 0.5
    );
    g.active_congress_effects.clear();
    g.active_congress_effects
        .push(effect("global_energy_treaty", "B", "coal_power_plant"));
    assert!(g.item_cost_for_city(0, city, &plant).is_infinite());

    g.active_congress_effects.clear();
    let owned_before = g.cities[&city].owned_tiles.len();
    g.cities.get_mut(&city).unwrap().border_culture = 1_000.0;
    g.active_congress_effects
        .push(effect("border_control_treaty", "B", "0"));
    g.process_city(0, city);
    assert_eq!(g.cities[&city].owned_tiles.len(), owned_before);
    assert_eq!(g.cities[&city].border_culture, 1_000.0);

    g.active_congress_effects.clear();
    let district_site = g.district_sites(city, crate::name!("campus"))[0];
    g.active_congress_effects
        .push(effect("border_control_treaty", "A", "0"));
    assert!(g.complete_item(
        0,
        city,
        &Item::District {
            district: crate::name!("campus"),
            pos: district_site,
        }
    ));
    assert!(g.cities[&city].owned_tiles.len() > owned_before);

    g.active_congress_effects.clear();
    g.players[0].techs.insert(crate::name!("mining"));
    let chop_tile = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != g.cities[&city].pos
                && *position != district_site
                && g.unit_ids_at(*position).is_empty()
                && g.map.tiles[position].district.is_none()
                && g.map.tiles[position].wonder.is_none()
        })
        .unwrap();
    {
        let tile = g.map.tiles.get_mut(&chop_tile).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("forest"));
        tile.resource = None;
        tile.improvement = None;
    }
    let builder = g.spawn_test_unit("builder", 0, chop_tile);
    g.active_congress_effects
        .push(effect("deforestation_treaty", "B", "forest"));
    assert!(!g
        .builder_operations(0, chop_tile)
        .contains(&"chop_woods".to_string()));
    g.active_congress_effects.clear();
    g.active_congress_effects
        .push(effect("deforestation_treaty", "A", "forest"));
    let gold_before = g.players[0].gold;
    let clearing_reward = 20.0 * (g.world_era as f64 + 1.0);
    g.do_improve(0, builder, "chop_woods").unwrap();
    assert_eq!(g.players[0].gold, gold_before + clearing_reward);

    g.active_congress_effects.clear();
    g.players[0]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 1);
    g.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 0);
    g.players[1]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 3);
    g.players[1]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 2);
    g.congress = Some(CongressSession {
        convened: 30,
        closes: 35,
        resolutions: vec![CongressResolution {
            id: "arms_control".to_string(),
            title: "Arms Control".to_string(),
            choices: vec!["A:0".to_string(), "A:1".to_string()],
            ballots: BTreeMap::from([(0, ("A:1".to_string(), 1))]),
        }],
    });
    g.resolve_congress();
    for player in 0..2 {
        assert_eq!(
            g.players[player].counters["project_effect:nuclear_devices"],
            3
        );
        assert_eq!(
            g.players[player].counters["project_effect:thermonuclear_devices"],
            2
        );
    }
    assert!(g
        .active_congress_effects
        .iter()
        .all(|effect| effect.resolution != "arms_control"));

    g.congress = Some(CongressSession {
        convened: 60,
        closes: 65,
        resolutions: vec![CongressResolution {
            id: "arms_control".to_string(),
            title: "Arms Control".to_string(),
            choices: vec!["B:0".to_string(), "B:1".to_string()],
            ballots: BTreeMap::from([(0, ("B:1".to_string(), 1))]),
        }],
    });
    g.resolve_congress();
    assert_eq!(g.players[1].counters["project_effect:nuclear_devices"], 0);
    assert_eq!(
        g.players[1].counters["project_effect:thermonuclear_devices"],
        0
    );
}

#[test]
fn losing_a_unit_wearies_the_side_that_lost_it() {
    // WAR_WEARINESS_PER_UNIT_KILLED is 3, and it lands on the owner of the
    // dead unit rather than on the killer. CIVVIS charged only for the act
    // of fighting -- 1 or 2 by whose ground it happened on -- so a war of
    // attrition cost the losing side nothing extra for actually losing.
    let mut g = game_with_capitals(2, 4_163, 300);
    g.at_war.insert(pair(0, 1));
    let position = g.cities[&g.player_city_ids(1)[0]].pos;
    let victim = g.spawn_unit("warrior", 1, position);
    g.players[0].war_weariness = 0.0;
    g.players[1].war_weariness = 0.0;

    g.record_war_unit_loss(0, 1, "warrior", 0, position);
    assert_eq!(g.players[1].war_weariness, 3.0, "the loser pays");
    assert_eq!(g.players[0].war_weariness, 0.0, "the killer does not");

    // It rides the same reductions as every other source: Martial Law
    // takes a quarter off.
    g.players[1].war_weariness = 0.0;
    g.players[1].policies = [crate::name!("martial_law")].into_iter().collect();
    g.record_war_unit_loss(0, 1, "warrior", 0, position);
    assert_eq!(g.players[1].war_weariness, 2.25);

    // And nothing accrues outside a war.
    g.at_war.clear();
    g.players[1].war_weariness = 0.0;
    g.record_war_unit_loss(0, 1, "warrior", 0, position);
    assert_eq!(g.players[1].war_weariness, 0.0);
    let _ = victim;
}

#[test]
fn war_weariness_costs_every_city_an_amenity_and_decays_at_the_shipped_rates() {
    let mut g = game_with_capitals(2, 4_162, 300);
    let home = g.player_city_ids(0)[0];
    let baseline = g.city_local_amenities(&g.cities[&home]);

    // WAR_WEARINESS_POINTS_FOR_AMENITY_LOSS is 400, and the loss lands on
    // every city the civilization owns, not just the ones at war.
    g.players[0].war_weariness = 399.0;
    assert_eq!(g.war_weariness_amenity_loss(0), 0);
    assert_eq!(g.city_local_amenities(&g.cities[&home]), baseline);
    g.players[0].war_weariness = 400.0;
    assert_eq!(g.war_weariness_amenity_loss(0), 1);
    assert_eq!(g.city_local_amenities(&g.cities[&home]), baseline - 1);
    g.players[0].war_weariness = 1_200.0;
    assert_eq!(g.city_local_amenities(&g.cities[&home]), baseline - 3);

    // WAR_WEARINESS_DECAY_TURN_AT_WAR 50, _AT_PEACE 200.
    g.at_war.insert(pair(0, 1));
    g.process_diplomacy(0);
    assert_eq!(g.players[0].war_weariness, 1_150.0);
    g.at_war.remove(&pair(0, 1));
    g.process_diplomacy(0);
    assert_eq!(g.players[0].war_weariness, 950.0);
}

#[test]
fn fascism_endures_war_worse_than_every_other_government() {
    // EFFECT_ADJUST_WAR_WEARINESS signs its Amount: TOWORLDSEND is -100 for
    // "no war weariness", INCREASE_ENEMY is +100. FASCISM_WAR_WEARINESS is
    // {Amount: 20, Overall: 1} — the +50% unit Production and +5 Combat
    // Strength are paid for with a fifth MORE weariness, not less.
    let mut g = game_with_capitals(2, 4_162, 300);
    assert_eq!(g.war_weariness_multiplier(0, false), 1.0);

    g.players[0].government = Some("fascism".to_string());
    assert_eq!(g.war_weariness_multiplier(0, false), 1.2);
    assert_eq!(g.war_weariness_multiplier(0, true), 1.2);

    // The governments that really do reduce it still do, at their own rates:
    // MARTIALLAW_OVERALLWARWEARINESS -25 anywhere, and
    // DEFENSEOFMOTHERLAND_DOMESTICWARWEARINESS -100 with Domestic set, so
    // that card is free only on home soil.
    g.players[0].government = Some("oligarchy".to_string());
    g.players[0].policies = [crate::name!("martial_law")].into_iter().collect();
    assert_eq!(g.war_weariness_multiplier(0, false), 0.75);
    g.players[0].policies = [crate::name!("defense_of_motherland")]
        .into_iter()
        .collect();
    assert_eq!(g.war_weariness_multiplier(0, true), 0.0);
    assert_eq!(g.war_weariness_multiplier(0, false), 1.0);
}

#[test]
fn past_ages_shift_the_next_threshold_by_the_shipped_amounts() {
    // THRESHOLD_SHIFT_PER_CITY 1, _PER_PAST_DARK_AGE -5,
    // _PER_PAST_GOLDEN_AGE +5.
    let t = Game::normal_age_threshold;
    assert_eq!(t(1, 1, 0, 0), 15);
    // Each extra city past the first raises it by one.
    assert_eq!(t(1, 4, 0, 0), 18);
    // A past Dark Age makes the next Normal Age five points easier.
    assert_eq!(t(1, 4, 1, 0), 13);
    // ...and a past Golden Age makes it five points harder.
    assert_eq!(t(1, 4, 0, 1), 23);
    assert_eq!(t(1, 4, 1, 2), 23);
    // Ancient alone carries the shipped -3 EraScoreThresholdShift.
    assert_eq!(t(0, 1, 0, 0), 12);
}

#[test]
fn fighting_abroad_wearies_twice_as_fast_as_fighting_at_home() {
    // WAR_WEARINESS_PER_COMBAT_IN_FOREIGN_LANDS 2 against _IN_ALLIED_LANDS
    // 1; your own soil is the cheap case alongside an ally's.
    let mut g = game_with_capitals(2, 4_163, 300);
    g.at_war.insert(pair(0, 1));
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    let abroad = g.cities[&g.player_city_ids(1)[0]].pos;

    g.accrue_combat_weariness(0, home);
    assert_eq!(g.players[0].war_weariness, 1.0);
    g.accrue_combat_weariness(0, abroad);
    assert_eq!(g.players[0].war_weariness, 3.0);

    // Peace means no weariness accrues at all.
    g.at_war.remove(&pair(0, 1));
    g.accrue_combat_weariness(0, abroad);
    assert_eq!(g.players[0].war_weariness, 3.0);
}

#[test]
fn special_sessions_keep_the_shipped_fifteen_turn_spacing() {
    // WORLD_CONGRESS_MIN_TIME_BETWEEN_SPECIAL_SESSIONS is 15. Without it
    // a queue of Emergencies seats one after another, and since each one
    // displaces the regular Congress, a run of captures could starve the
    // World Congress -- and with it the Diplomatic Victory it awards.
    let mut g = game_with_capitals(3, 4_151, 300);
    let city = g.player_city_ids(1)[0];
    let proposal = |id: u32| EmergencyProposal {
        id,
        kind: "military".to_string(),
        target: 1,
        city,
        original_owner: 1,
        eligible: BTreeSet::from([0, 2]),
        requested: 0,
    };
    g.turn = 40;
    g.pending_emergencies = vec![proposal(1), proposal(2)];

    g.convene_pending_emergency();
    assert!(g.congress.is_some(), "the first Emergency seats at once");
    assert_eq!(g.last_special_session, 40);

    // Close that session and offer the next one one turn too early.
    let spacing = g.standard_duration(15);
    g.congress = None;
    g.turn = 40 + spacing - 1;
    g.convene_pending_emergency();
    assert!(
        g.congress.is_none(),
        "a second Special Session cannot open inside the spacing"
    );
    assert_eq!(
        g.pending_emergencies.len(),
        2,
        "held proposals wait rather than being dropped"
    );

    g.turn = 40 + spacing;
    g.convene_pending_emergency();
    assert!(g.congress.is_some(), "and seats once the spacing elapses");
    assert_eq!(g.last_special_session, 40 + spacing);
}

#[test]
fn congress_ties_use_the_largest_share_of_available_favor() {
    let mut g = game_with_capitals(2, 414, 300);
    g.turn = 30;
    g.players[0].diplomatic_favor = 100.0;
    g.players[1].diplomatic_favor = 30.0;
    g.congress = Some(CongressSession {
        convened: 30,
        closes: 35,
        resolutions: vec![CongressResolution {
            id: "urban_development_treaty".to_string(),
            title: "Urban Development Treaty".to_string(),
            choices: vec!["A:campus".to_string(), "A:theater_square".to_string()],
            ballots: BTreeMap::new(),
        }],
    });
    g.do_congress_vote(0, "urban_development_treaty", "A:campus", 2)
        .unwrap();
    g.do_congress_vote(1, "urban_development_treaty", "A:theater_square", 2)
        .unwrap();

    g.turn = 35;
    g.process_congress();

    assert!(g.congress_effect_active("urban_development_treaty", "A", "theater_square"));
    assert_eq!(g.players[0].dvp, 0);
    assert_eq!(g.players[1].dvp, 1);
    assert_eq!(g.players[0].diplomatic_favor, 95.0);
    assert_eq!(g.players[1].diplomatic_favor, 20.0);
}

#[test]
fn score_only_decides_the_game_after_the_turn_limit() {
    let mut g = game_with_capitals(2, 406, 3);
    let capital = g.player_city_ids(0)[0];
    // Citizens score 1 point each under the Gathering Storm rules, so an
    // overwhelming score needs a correspondingly enormous city.
    g.cities.get_mut(&capital).unwrap().pop = 600;
    assert!(g.score(0) > 500);
    g.current = 1;
    g.turn = 2;
    g.do_end_turn();
    assert_eq!(g.turn, 3);
    assert_eq!(g.winner, None);
    g.current = 1;
    g.do_end_turn();
    assert_eq!(g.turn, 4);
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("score"));
}

/// A three-turn game is won on turn three. The count that settles the
/// tiebreak is taken on the wrap into a fourth turn nobody ever plays, and
/// that wrap is bookkeeping the result is never dated by.
#[test]
fn a_score_victory_is_dated_on_the_turn_the_limit_names() {
    let mut g = game_with_capitals(2, 406, 3);
    let capital = g.player_city_ids(0)[0];
    g.cities.get_mut(&capital).unwrap().pop = 600;
    g.current = 1;
    g.turn = 3;
    g.do_end_turn();
    assert_eq!(g.winner, Some(0));
    assert_eq!(g.victory_type.as_deref(), Some("score"));
    assert_eq!(g.turn, 4, "the count is taken on the wrap past the limit");
    assert_eq!(g.reported_turn(), 3, "the game is won on the limit's turn");
    let declared = g
        .events
        .iter()
        .rev()
        .find(|event| event.text.contains("won a score victory"))
        .expect("the chronicle records the result");
    assert_eq!(declared.turn, 3);

    // "One more turn" borrows its turns from the result it was given, so
    // the verdict keeps the turn it was won on while the extension itself
    // is played past the limit the world has now left behind.
    assert!(g.play_on(PlayOnMode::UntilNextVictory));
    assert_eq!(g.decided.as_ref().map(|decided| decided.turn), Some(3));
    assert_eq!(g.turn, 4);
    assert_eq!(g.reported_turn(), 4);
}

#[test]
fn specialty_district_capacity_unlocks_at_population_one_four_and_seven() {
    let mut g = game_with_capitals(2, 407, 300);
    let cid = g.player_city_ids(0)[0];
    let center = g.cities[&cid].pos;
    let owned = g.cities[&cid].owned_tiles.clone();
    for pos in owned.iter().copied().filter(|pos| *pos != center) {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.district = None;
        tile.hills = false;
    }
    g.players[0].techs.extend([
        crate::name!("writing"),
        crate::name!("astrology"),
        crate::name!("rocketry"),
    ]);
    g.players[0].civics.insert(crate::name!("drama_poetry"));

    let campus_site = g.district_sites(cid, crate::name!("campus"))[0];
    assert!(g.complete_item(
        0,
        cid,
        &Item::District {
            district: crate::name!("campus"),
            pos: campus_site,
        }
    ));
    assert!(
        g.district_sites(cid, crate::name!("holy_site")).is_empty(),
        "population 1 supports only one specialty district"
    );
    assert!(
        !g.district_sites(cid, crate::name!("spaceport")).is_empty(),
        "Spaceports ignore the specialty-district population cap"
    );

    g.cities.get_mut(&cid).unwrap().pop = 4;
    let holy_site = g.district_sites(cid, crate::name!("holy_site"))[0];
    assert!(g.complete_item(
        0,
        cid,
        &Item::District {
            district: crate::name!("holy_site"),
            pos: holy_site,
        }
    ));
    assert!(
        g.district_sites(cid, crate::name!("theater_square"))
            .is_empty(),
        "population 4 supports exactly two specialty districts"
    );

    g.cities.get_mut(&cid).unwrap().pop = 7;
    assert!(!g
        .district_sites(cid, crate::name!("theater_square"))
        .is_empty());
}

#[test]
fn gathering_storm_amenities_require_connected_luxuries_and_ration_them() {
    let mut g = game_with_capitals(2, 408, 300);
    let capital = g.player_city_ids(0)[0];
    let occupied: BTreeSet<Pos> = g.cities.values().map(|city| city.pos).collect();
    let sites: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| !occupied.contains(pos))
        .take(5)
        .collect();
    for (n, pos) in sites.into_iter().enumerate() {
        g.found_city_for(0, pos, Some(format!("Amenity {n}")));
    }
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 1;
        city.buildings.clear();
        city.districts.clear();
    }
    g.players[0].government = None;
    g.players[0].policies.clear();
    g.players[0].governors.clear();

    let luxury_pos = g.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&capital].pos)
        .unwrap();
    let tile = g.map.tiles.get_mut(&luxury_pos).unwrap();
    tile.resource = Some(crate::name!("silk"));
    tile.improvement = None;
    assert_eq!(
        g.empire_luxuries(0),
        0,
        "an unimproved luxury supplies no Amenities"
    );

    g.map.tiles.get_mut(&luxury_pos).unwrap().improvement = Some(crate::name!("plantation"));
    assert_eq!(g.empire_luxuries(0), 1);
    let mut surpluses: Vec<i64> = g
        .player_city_ids(0)
        .into_iter()
        .map(|cid| g.city_amenity_surplus(&g.cities[&cid]))
        .collect();
    surpluses.sort();
    assert_eq!(
        surpluses,
        vec![-1, 0, 0, 0, 0, 1],
        "one luxury serves the four neediest cities; the Palace serves the capital with two"
    );

    let duplicate_pos = g.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&capital].pos && *pos != luxury_pos)
        .unwrap();
    let tile = g.map.tiles.get_mut(&duplicate_pos).unwrap();
    tile.resource = Some(crate::name!("silk"));
    tile.improvement = Some(crate::name!("plantation"));
    assert_eq!(
        g.empire_luxuries(0),
        1,
        "duplicate copies of a luxury do not supply more cities"
    );

    g.players[0].civ = "Aztec".to_string();
    let mut aztec_surpluses: Vec<i64> = g
        .player_city_ids(0)
        .into_iter()
        .map(|cid| g.city_amenity_surplus(&g.cities[&cid]))
        .collect();
    aztec_surpluses.sort();
    assert_eq!(
        aztec_surpluses,
        vec![0, 0, 0, 0, 0, 2],
        "Gifts for the Tlatoani extends each luxury from four to six cities"
    );
}

#[test]
fn gathering_storm_happiness_bands_apply_exact_growth_and_yield_modifiers() {
    let cases = [
        (7, 1.20, 1.20),
        (5, 1.20, 1.20),
        (4, 1.10, 1.10),
        (3, 1.10, 1.10),
        (2, 1.00, 1.00),
        (0, 1.00, 1.00),
        (-2, 0.90, 0.85),
        (-4, 0.80, 0.70),
        (-6, 0.70, 0.00),
        (-7, 0.60, 0.00),
    ];
    for (surplus, yields, growth) in cases {
        assert_eq!(Game::amenity_yield_mult_for(surplus), yields);
        assert_eq!(Game::amenity_growth_mult(surplus), growth);
    }
}

#[test]
fn gathering_storm_happiness_bands_apply_loyalty_at_exact_boundaries() {
    for (surplus, loyalty) in [
        (5, 6.0),
        (4, 3.0),
        (3, 3.0),
        (2, 0.0),
        (0, 0.0),
        (-1, -3.0),
        (-2, -3.0),
        (-3, -6.0),
        (-7, -6.0),
    ] {
        assert_eq!(Game::happiness_loyalty_delta(surplus), loyalty);
    }
}

#[test]
fn starvation_subtracts_exactly_four_loyalty_per_turn() {
    let mut starved = game_with_capitals(1, 4_245, 300);
    let city = starved.player_city_ids(0)[0];
    starved.cities.get_mut(&city).unwrap().pop = 10;
    for position in starved.cities[&city].owned_tiles.clone() {
        let tile = starved.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("snow");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.hills = false;
    }
    let mut fed = starved.clone();
    for position in fed.cities[&city].owned_tiles.clone() {
        let tile = fed.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.improvement = Some(crate::name!("farm"));
    }

    let consumption = 2.0 * starved.cities[&city].pop as f64;
    assert!(starved.city_yields(city).food < consumption);
    assert!(fed.city_yields(city).food >= consumption);
    assert_eq!(
        fed.city_loyalty_per_turn(&fed.cities[&city])
            - starved.city_loyalty_per_turn(&starved.cities[&city]),
        4.0
    );
}

#[test]
fn housing_uses_palace_aqueduct_lighthouse_and_exact_growth_bands() {
    let mut g = game_with_capitals(2, 409, 300);
    let cid = g.player_city_ids(0)[0];
    let center = g.cities[&cid].pos;
    g.map.clear_rivers();
    for pos in std::iter::once(center).chain(g.nbrs(center)) {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
    }
    assert_eq!(
        g.city_housing(&g.cities[&cid]),
        3.0,
        "a dry capital has 2 water Housing plus 1 from its Palace"
    );

    let coast = g.nbrs(center)[0];
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    assert_eq!(
        g.city_housing(&g.cities[&cid]),
        4.0,
        "a coastal capital starts with 3 + Palace"
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("lighthouse"));
    let harbor = install_test_district(&mut g, cid, "harbor");
    assert_eq!(
        g.city_housing(&g.cities[&cid]),
        6.0,
        "a coastal Lighthouse supplies 2 total Housing, plus the Palace"
    );

    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|b| b != "lighthouse");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .remove(crate::name!("harbor"));
    g.map.tiles.get_mut(&harbor).unwrap().district = None;
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("plains");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("aqueduct"), coast);
    assert_eq!(
        g.city_housing(&g.cities[&cid]),
        7.0,
        "an Aqueduct raises dry water Housing to 6, plus the Palace"
    );
    assert!(g.map.set_river_edge(center, g.nbrs(center)[1], true));
    assert_eq!(
        g.city_housing(&g.cities[&cid]),
        8.0,
        "a fresh-water Aqueduct adds 2 to 5, plus the Palace"
    );

    let cases = [
        (2.0, 1.0),
        (1.5, 0.5),
        (1.0, 0.5),
        (0.0, 0.25),
        (-3.5, 0.25),
        (-4.0, 0.0),
        (-5.0, 0.0),
    ];
    for (headroom, growth) in cases {
        assert_eq!(Game::housing_growth_mult(headroom), growth);
    }
}

#[test]
fn palace_supplies_the_stock_capital_yield_package_and_moves_when_captured() {
    let mut g = game_with_capitals(2, 410, 300);
    let original = g.player_city_ids(0)[0];
    let city = &g.cities[&original];
    assert!(g.city_has_palace(city));
    let yields = g.city_yields(original);
    assert!(yields.production >= 3.0);
    assert!(yields.gold >= 5.0);
    assert!(yields.science >= 2.5);
    assert!(yields.culture >= 1.3);

    let second_pos = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            g.rules.is_passable(&g.map.tiles[pos])
                && !g.rules.is_water(&g.map.tiles[pos])
                && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4)
        })
        .unwrap();
    let second = g.found_city_for(0, second_pos, Some("Fallback".to_string()));
    g.capture_city(original, 1);
    assert!(!g.city_has_palace(&g.cities[&original]));
    assert!(g.city_has_palace(&g.cities[&second]));
}

#[test]
fn production_switching_preserves_item_progress_without_banking_idle_turns() {
    let mut g = game_with_capitals(2, 411, 300);
    let pid = 1;
    let cid = g.player_city_ids(pid)[0];
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };

    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    g.process_city(pid, cid);
    assert_eq!(
        g.cities[&cid].production, 0.0,
        "idle turns do not bank Production"
    );

    g.do_produce(pid, cid, &monument).unwrap();
    g.cities.get_mut(&cid).unwrap().production = 20.0;
    g.do_produce(pid, cid, &builder).unwrap();
    assert_eq!(g.cities[&cid].production, 0.0);
    assert_eq!(
        g.item_remaining_cost_for_city(pid, cid, &monument),
        g.item_cost_for_city(pid, cid, &monument) - 20.0
    );
    g.cities.get_mut(&cid).unwrap().production = 10.0;
    g.do_produce(pid, cid, &monument).unwrap();
    assert_eq!(g.cities[&cid].production, 20.0);
    g.do_produce(pid, cid, &builder).unwrap();
    assert_eq!(g.cities[&cid].production, 10.0);
    assert_eq!(
        g.item_remaining_cost_for_city(pid, cid, &builder),
        g.item_cost_for_city(pid, cid, &builder) - 10.0
    );
}

#[test]
fn gathering_storm_overflow_removes_item_specific_production_bonus() {
    let mut g = game_with_capitals(2, 412, 300);
    let cid = g.player_city_ids(0)[0];
    g.players[0].techs.insert(crate::name!("masonry"));
    g.players[0].policies.insert(crate::name!("limes"));
    let walls = Item::Building {
        building: crate::name!("walls"),
    };
    g.do_produce(0, cid, &walls).unwrap();
    let base = g.city_yields(cid).production;
    let cost = g.item_cost_for(0, &walls);
    g.cities.get_mut(&cid).unwrap().production = cost - base;
    g.process_city(0, cid);
    assert!(g.cities[&cid].buildings.contains(&crate::name!("walls")));
    assert!(
        (g.cities[&cid].production - base / 2.0).abs() < 1e-9,
        "only the unused base Production survives a +100% Limes completion"
    );
}

#[test]
fn legacy_purchased_building_queue_is_discarded_without_a_duplicate() {
    let mut g = game_with_capitals(2, 414, 300);
    let cid = g.player_city_ids(0)[0];
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    let city = g.cities.get_mut(&cid).unwrap();
    city.buildings.retain(|building| building != "granary");
    city.buildings.push(crate::name!("granary"));
    city.queue = vec![granary.clone()];
    city.production = g.rules.buildings["granary"].cost;

    g.process_city(0, cid);

    assert!(g.cities[&cid].queue.is_empty());
    assert_eq!(
        g.cities[&cid]
            .buildings
            .iter()
            .filter(|building| building.as_str() == "granary")
            .count(),
        1
    );
}

#[test]
fn settlers_and_builders_scale_and_settlers_consume_population() {
    let mut g = game_with_capitals(2, 413, 300);
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().pop = 2;
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    assert_eq!(g.item_cost_for(0, &settler), 80.0);
    assert!(g.complete_item(0, cid, &settler));
    assert_eq!(g.cities[&cid].pop, 1);
    assert_eq!(g.item_cost_for(0, &settler), 110.0);
    assert!(!g.can_produce(0, cid, &settler));

    g.cities.get_mut(&cid).unwrap().pop = 2;
    g.players[0].gold = 1_000.0;
    g.do_buy(0, cid, "settler", "gold").unwrap();
    assert_eq!(g.cities[&cid].pop, 1);
    assert_eq!(g.players[0].gold, 560.0);
    assert_eq!(g.item_cost_for(0, &settler), 140.0);

    assert_eq!(g.item_cost_for(0, &builder), 50.0);
    assert!(g.complete_item(0, cid, &builder));
    assert_eq!(g.item_cost_for(0, &builder), 54.0);
}

#[test]
fn religions_convert_all_owned_holy_sites_and_temples_require_shrines() {
    let mut g = game_with_capitals(2, 414, 300);
    let first = g.player_city_ids(0)[0];
    let second_pos = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            g.rules.is_passable(&g.map.tiles[pos])
                && !g.rules.is_water(&g.map.tiles[pos])
                && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4)
        })
        .unwrap();
    let second = g.found_city_for(0, second_pos, Some("Second Holy Site".to_string()));
    let first_site = g.cities[&first]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&first].pos)
        .unwrap();
    let second_site = g.cities[&second]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&second].pos)
        .unwrap();
    g.cities
        .get_mut(&first)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), first_site);
    g.cities
        .get_mut(&second)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), second_site);
    g.players[0].civics.insert(crate::name!("theology"));
    let temple = Item::Building {
        building: crate::name!("temple"),
    };
    assert!(!g.can_produce(0, first, &temple));
    g.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    assert!(g.can_produce(0, first, &temple));

    g.players[0].prophet_pending = true;
    g.do_found_religion(0, "choral_music", "tithe").unwrap();
    let religion = g.players[0].religion.clone().unwrap();
    assert_eq!(g.cities[&first].pressure[&religion], 1_000.0);
    assert_eq!(g.cities[&second].pressure[&religion], 1_000.0);
}

/// Every Gathering Storm source of a Diplomatic Victory Point, and nothing
/// else.
///
/// ★★★★★ THE LANE WE LOSE TO IS THE LANE WE BARELY MODEL. Of the 74 live games
/// stolen by a rival's victory, **41 are diplomatic** — and CIVVIS's own
/// diplomacy lane almost never completes: the contested screen produced 2
/// diplomatic victories in 120 games and 3 in 60
/// (`docs/eval/2026-08-18-the-promotion-matrix-can-see-the-lanes-the-front-line-loses-.md`).
///
/// ⚠ **CORRECTION.** This comment first said the shipped database names
/// *six* modifiers adjusting `DIPLOMATIC_VICTORY_POINTS`. It names **ten**.
/// The search that found six matched `MODIFIER_PLAYER_ADJUST_DIPLOMATIC_VICTORY_POINTS`
/// and missed `MODIFIER_EMERGENCY_PLAYERS_ADJUST_DIPLOMATIC_VICTORY_POINTS`,
/// which is how a scored competition pays its winner. Searching for one
/// spelling of a thing and concluding the list is complete is the mistake, and
/// it is the second time in two days that a "complete" claim about this lane
/// has been wrong.
///
/// The threshold is 20 (`GlobalParameters.DIPLOMATIC_VICTORY_POINTS_REQUIRED`).
/// The seven **content** sources are below; the three competition sources are
/// in `a_native_competition_pays_its_winner_a_diplomatic_victory_point`,
/// because they are engine, not data.
///
/// | source | amount |
/// |---|---|
/// | `WC_RES_DIPLOVICTORY`, injected into every congress from the Modern era | ±2 |
/// | `BUILDING_MAHABODHI_TEMPLE` | 2 |
/// | `BUILDING_POTALA_PALACE` | 1 |
/// | `BUILDING_STATUE_LIBERTY` | 4 |
/// | `CIVIC_GLOBAL_WARMING_MITIGATION` | 1 |
/// | `TECH_SEASTEADS` | 1 |
///
/// ⚠⚠ ALL SIX ARE MODELLED, AND THIS TEST EXISTS BECAUSE THAT WAS NOT OBVIOUS.
/// The tech and the civic are not in `techs.json` or `civics.json` — they are
/// in `tree_effects.json`, the overlay `Rules::load` folds in with
/// `add_effects`. Looking only at the first two files makes them appear
/// missing, and *adding* them there does not replace the overlay, it **sums**
/// with it: an attempt to "fix" this shipped 2 points where Gathering Storm
/// grants 1, on both nodes, and this assertion is what caught it.
///
/// ⚠ Nothing else would have. `civ6_fidelity.py` reports zero divergent fields
/// with the doubled values in place, because it does not audit tree-node
/// effects; `civvis_inert.py` reports zero unconsumed keys, because the key was
/// consumed either way. The amounts below are read from the shipped
/// `ModifierArguments` and are the only check on them.
///
/// So the diplomatic lane is not starved of *sources*. It is starved of
/// *reach*: the same evidence file records **31 of 32** diplomatic games
/// finishing no qualifying wonder at all — Mahabodhi needs a founded religion,
/// a Holy Site and a Temple beside a forest tile; the Statue of Liberty needs a
/// Harbor and Civil Engineering — and the tech and civic that need no such
/// chain are Future-era, worth one point each.
#[test]
fn every_shipped_source_of_a_diplomatic_victory_point_is_modelled() {
    let rules = crate::rules::Rules::shipped();
    let from_wonders: Vec<(&str, f64)> = rules
        .wonders
        .iter()
        .filter_map(|(name, spec)| {
            spec.effects
                .get("diplomatic_victory_points")
                .map(|amount| (name.as_str(), *amount))
        })
        .collect();
    let from_tree: Vec<(&str, f64)> = rules
        .techs
        .iter()
        .map(|(name, spec)| (name, &spec.effects))
        .chain(
            rules
                .civics
                .iter()
                .map(|(name, spec)| (name, &spec.effects)),
        )
        .filter_map(|(name, effects)| {
            effects
                .get("diplomatic_victory_points")
                .map(|amount| (name.as_str(), *amount))
        })
        .collect();

    let mut all: Vec<(&str, f64)> = from_wonders.into_iter().chain(from_tree).collect();
    all.sort_by(|left, right| left.0.cmp(right.0));
    assert_eq!(
        all,
        vec![
            ("global_warming_mitigation", 1.0),
            ("mahabodhi_temple", 2.0),
            ("potala_palace", 1.0),
            ("seasteads", 1.0),
            ("statue_of_liberty", 4.0),
        ],
        "the shipped database names six modifiers adjusting DIPLOMATIC_VICTORY_POINTS; \
         five are content and the sixth is the World Congress resolution. A source \
         added or dropped here is a change to how a Diplomatic victory is won"
    );
    // And the congress half, which is not content: the resolution is injected
    // into every session from the Modern era, worth ±2 to its target.
    assert_eq!(DIPLOMATIC_VICTORY_POINTS, 20);
}

/// A native competition seats itself, scores, and pays its winner.
///
/// ★★★★★ THE LANE'S MISSING ENGINE. Gathering Storm pays a Diplomatic Victory
/// Point to the first-place finisher of a scored competition, and CIVVIS had no
/// way to run one until now; #2167 recorded the gap and this closes it. Off by default,
/// because turning it on changes what every participant faces.
#[test]
fn a_native_competition_pays_its_winner_a_diplomatic_victory_point() {
    let mut game = game_with_capitals(3, 61_000, 400);
    game.native_competitions = true;
    game.world_era = 8;
    // The Space Station competition needs someone holding a Spaceport, which is
    // what makes it offerable at all.
    let city = *game.cities.keys().next().expect("the fixture has a city");
    let pos = game.cities[&city].pos;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pos);

    assert!(game.competition.is_none(), "nothing is running yet");
    game.open_native_competition();
    let running = game
        .competition
        .as_ref()
        .expect("a Spaceport makes the Space Station competition offerable");
    assert_eq!(running.kind, "EMERGENCY_SPACE_STATION");
    let ends = running.ends;
    assert!(ends > game.turn, "a competition runs for a while");

    // It presents exactly as a mirrored one does, for every seat — which is why
    // the production catalog and the AI's valuation needed no change.
    let owner = game.cities[&city].owner;
    let standing = game
        .host_competition(owner, "EMERGENCY_SPACE_STATION")
        .expect("a native competition answers for any seat");
    assert_eq!(standing.ours, 0.0);
    assert_eq!(standing.ends, ends);

    // Completing the scoring project is what scores.
    game.complete_item(
        owner,
        city,
        &Item::Project {
            project: crate::name!("train_astronauts"),
        },
    );
    let scored = game.competition.as_ref().unwrap().scores[&owner];
    assert_eq!(
        scored, game.rules.projects["train_astronauts"].competition_score,
        "the score the project declares is the score it pays"
    );

    // ⚠ And Great Person Points score nothing here. The Space Station counts a
    // project and the districts an empire maintains; only the World's Fair
    // counts Great Person Points, and a scorer that ignored which competition
    // was running would pay the wrong race.
    let project_only = game.competition.as_ref().unwrap().scores[&owner];
    game.score_great_person_point_competition(owner, 40.0);
    assert_eq!(
        game.competition.as_ref().unwrap().scores[&owner],
        project_only,
        "Great Person Points must not score a project-scored competition"
    );

    // The Spaceport it holds scores 5 a turn on its own, which is
    // `SPACE_STATION_SCORE_SPACEPORTS` — "Maintaining Spaceport Districts".
    game.score_competition_holdings(owner);
    assert_eq!(
        game.competition.as_ref().unwrap().scores[&owner] - project_only,
        5.0,
        "a maintained Spaceport scores 5 a turn"
    );

    // And the clock running out pays first place.
    let before = game.players[owner].dvp;
    let favor = game.players[owner].diplomatic_favor;
    game.turn = ends;
    game.close_native_competition();
    assert!(
        game.competition.is_none(),
        "a finished competition is cleared"
    );
    assert_eq!(
        game.players[owner].dvp - before,
        1,
        "Gathering Storm pays the Space Station's winner one Diplomatic Victory Point"
    );
    assert_eq!(
        game.players[owner].diplomatic_favor - favor,
        50.0,
        "and `ISS_TOP_TIER_FAVOR` beside it: Gold Tier takes all Silver Tier rewards"
    );
    assert!(
        game.competition_lockout_until["EMERGENCY_SPACE_STATION"] > game.turn,
        "a finished competition locks its own kind out"
    );
    assert!(
        !game
            .competition_lockout_until
            .contains_key("EMERGENCY_CLIMATE_ACCORDS"),
        "and locks out only its own kind: the shipped `LockoutTime` is a column \
         on the emergency row, not a global clock, and one competition blocking \
         every other seated two in a 250-turn game"
    );
}

/// And none of it happens unless the game asked for it.
///
/// ⚠ This is the whole safety argument for an off-by-default rules mechanism,
/// so it checks the scorers too, not only the seating. Every path #2379 adds
/// runs off `Game::competition`, and nothing but `open_native_competition` and
/// `open_native_aid_request` can set it — both of which return immediately with
/// the flag off. A default game therefore faces exactly the board it always
/// did, and the frozen rating anchor does not move.
#[test]
fn native_competitions_are_off_unless_a_game_turns_them_on() {
    let mut game = game_with_capitals(3, 61_001, 400);
    assert!(!game.native_competitions, "off by default");
    game.world_era = 8;
    let city = *game.cities.keys().next().expect("the fixture has a city");
    let pos = game.cities[&city].pos;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pos);
    game.open_native_competition();
    assert!(
        game.competition.is_none(),
        "a seat that did not ask for native competitions must face the same \
         board it always did, or the frozen rating anchor moves without a \
         protocol bump"
    );

    // And the three per-turn scorers, which run in `begin_turn` for every
    // player of every game whether the flag is on or not.
    let dvp: Vec<i64> = game.players.iter().map(|player| player.dvp).collect();
    let favor: Vec<f64> = game
        .players
        .iter()
        .map(|player| player.diplomatic_favor)
        .collect();
    for pid in 0..game.players.len() {
        game.score_great_person_point_competition(pid, 100.0);
        game.score_favor_competition(pid, 100.0);
        game.score_competition_holdings(pid);
    }
    assert!(
        game.competition.is_none(),
        "no score table appears out of nothing"
    );
    game.turn += 40;
    game.close_native_competition();
    for player in &game.players {
        assert_eq!(player.dvp, dvp[player.id]);
        assert_eq!(player.diplomatic_favor, favor[player.id]);
    }
    assert!(game.competition_lockout_until.is_empty());
}

/// A competition nobody can score in is not offered.
///
/// ★★★★★ THE FIRST TRACE SPENT A LOCKOUT ON NOTHING. Seating Climate Accords
/// needs more than an Industrial Zone: its projects *decommission a power
/// plant*, and an empire holding none cannot score a single point. The first
/// run of the mechanism seated it on turn 100, closed it on 119 with an empty
/// score table, and spent the sixty-turn lockout — while the whole 250-turn
/// game seated two competitions and paid one point.
///
/// So eligibility asks for everything the scoring project consumes, not only
/// the district it sits in.
#[test]
fn a_competition_is_offered_only_where_its_project_could_be_built() {
    let mut game = game_with_capitals(3, 62_000, 400);
    game.native_competitions = true;
    game.world_era = 7;
    let city = *game.cities.keys().next().expect("the fixture has a city");
    let pos = game.cities[&city].pos;
    let owner = game.cities[&city].owner;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("industrial_zone"), pos);

    assert!(
        !game.can_score_competition(owner, "EMERGENCY_CLIMATE_ACCORDS"),
        "an Industrial Zone with no power plant in it cannot decommission one"
    );
    game.open_native_competition();
    assert_ne!(
        game.competition.as_ref().map(|c| c.kind.as_str()),
        Some("EMERGENCY_CLIMATE_ACCORDS"),
        "a competition nobody can score in must not be seated: it pays no one          and spends the lockout"
    );
    // World Games is seatable here: its athletes project needs no district.
    // Clear it so the check below is about Climate Accords becoming offerable,
    // not about which competition came first.
    game.competition = None;

    // Give it something to decommission and it becomes offerable.
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("coal_power_plant"));
    assert!(game.can_score_competition(owner, "EMERGENCY_CLIMATE_ACCORDS"));
    game.open_native_competition();
    assert_eq!(
        game.competition.as_ref().map(|c| c.kind.as_str()),
        Some("EMERGENCY_CLIMATE_ACCORDS")
    );
}

/// The World's Fair counts Great Person **Points**, and it is the one an empire
/// can always enter.
///
/// ⚠ It counted recruited *people* until this change, and the difference is not
/// cosmetic. All eight shipped rows are `WORLDS_FAIR_SCORE_GPP_<class>` with
/// `ScoreAmount="1"`, described `LOC_EMERGENCY_SCORE_GPP_DESC` — "Generating
/// [ICON_GREATPERSON] Great People Points Per Turn". A 29-turn competition
/// therefore scores in the hundreds rather than in ones and twos, which is what
/// makes it produce a first place at all: on a count of recruits, two empires
/// claiming one person each is a tie, and a tie pays nobody.
#[test]
fn the_worlds_fair_scores_the_great_person_points_an_empire_generates() {
    let mut game = game_with_capitals(3, 63_000, 400);
    game.native_competitions = true;
    game.world_era = 5;

    // No Spaceport and no power plant anywhere, so the other three are not
    // offerable — and the lane still gets a competition.
    game.open_native_competition();
    assert_eq!(
        game.competition.as_ref().map(|c| c.kind.as_str()),
        Some("EMERGENCY_WORLDS_FAIR"),
        "the World's Fair needs no ground to hold, which is why it is the one \
         that can recur from the first congress"
    );

    let owner = game.cities.values().next().unwrap().owner;
    let running = game.competition.as_ref().unwrap();
    let before = running.scores.get(&owner).copied();
    assert_eq!(before, None, "nobody has scored yet");

    // One point per point generated, whatever the class.
    game.score_great_person_point_competition(owner, 6.5);
    game.score_great_person_point_competition(owner, 3.5);
    assert_eq!(game.competition.as_ref().unwrap().scores[&owner], 10.0);

    let dvp = game.players[owner].dvp;
    let favor = game.players[owner].diplomatic_favor;
    game.turn = game.competition.as_ref().unwrap().ends;
    game.close_native_competition();
    assert_eq!(
        game.players[owner].dvp - dvp,
        1,
        "Gathering Storm pays the World's Fair winner one Diplomatic Victory Point"
    );
    assert_eq!(
        game.players[owner].diplomatic_favor - favor,
        50.0,
        "and `WORLD_FAIR_TOP_TIER_FAVOR`"
    );
}

/// Competition eras come from Gathering Storm's emergency requirements, not
/// from whether a scoring project happens to have a district prerequisite.
#[test]
fn native_competitions_honor_era_windows_and_world_games_pays_its_winner() {
    let mut game = game_with_capitals(3, 63_001, 400);
    game.native_competitions = true;
    let city_id = *game.cities.keys().next().expect("the fixture has a city");
    let owner = game.cities[&city_id].owner;
    let pos = game.cities[&city_id].pos;
    let city = game.cities.get_mut(&city_id).unwrap();
    city.districts.insert(crate::name!("spaceport"), pos);
    city.districts.insert(crate::name!("industrial_zone"), pos);
    city.buildings.push(crate::name!("coal_power_plant"));

    // No Sweden on this board, so the Nobel Peace Prize does not exist and the
    // Industrial era has nothing else to offer.
    for player in game.players.iter_mut() {
        if player.civ == "Sweden" {
            player.civ = "Rome".to_string();
        }
    }
    game.world_era = 4;
    game.open_native_competition();
    assert!(
        game.competition.is_none(),
        "without Sweden, no congress competition is available before Modern"
    );

    game.world_era = 5;
    game.open_native_competition();
    assert_eq!(
        game.competition
            .as_ref()
            .map(|competition| competition.kind.as_str()),
        Some("EMERGENCY_WORLDS_FAIR"),
        "World's Fair is Modern only"
    );
    game.competition = None;

    game.world_era = 6;
    game.open_native_competition();
    assert_eq!(
        game.competition
            .as_ref()
            .map(|competition| competition.kind.as_str()),
        Some("EMERGENCY_WORLD_GAMES"),
        "World Games begins in Atomic"
    );
    let declared_score = game.rules.projects["train_athletes"].competition_score;
    game.complete_item(
        owner,
        city_id,
        &Item::Project {
            project: crate::name!("train_athletes"),
        },
    );
    assert_eq!(
        game.competition.as_ref().unwrap().scores[&owner],
        declared_score,
        "World Games uses the shipped athletes-project score"
    );
    let before = game.players[owner].dvp;
    game.turn = game.competition.as_ref().unwrap().ends;
    game.close_native_competition();
    assert_eq!(
        game.players[owner].dvp - before,
        1,
        "World Games pays its first-place finisher one Diplomatic Victory Point"
    );

    game.world_era = 7;
    game.open_native_competition();
    assert_eq!(
        game.competition
            .as_ref()
            .map(|competition| competition.kind.as_str()),
        Some("EMERGENCY_CLIMATE_ACCORDS"),
        "Climate Accords begins in Information"
    );
    game.competition = None;

    game.world_era = 8;
    game.open_native_competition();
    assert_eq!(
        game.competition
            .as_ref()
            .map(|competition| competition.kind.as_str()),
        Some("EMERGENCY_SPACE_STATION"),
        "International Space Station begins in Future"
    );
}

/// The seventh competition: Sweden's Nobel Peace Prize, scored from Favor.
///
/// ★★★★★ THE LAST DIPLOMATIC VICTORY POINT SOURCE GATHERING STORM HAS.
/// `EmergencyRewards` gives a `NON_EMERGENCY_FIRST_PLACE_VICTORY_POINT` to four
/// competitions — the World's Fair, the World Games, the International Space
/// Station and **the Nobel Peace Prize** — and the aid requests and the Climate
/// Accords take the other two modifiers. Peace is the only Nobel that pays a
/// point: Literature's first place takes cheaper Rock Bands and Physics' a
/// technology boost, so neither belongs in this table at all.
///
/// Two shipped rules decide when it can run, and both are read off the install
/// rather than chosen here:
///
/// - `NOBEL_PRIZE_TARGET_REQUIREMENTS` requires the game era to be at least
///   `ERA_INDUSTRIAL`, which is the earliest era any scored competition opens
///   in; and
/// - the same set requires `REQUIREMENT_GAME_HAS_CIVILIZATION_OR_LEADER_TRAIT`
///   for `TRAIT_CIVILIZATION_NOBEL_PRIZE`, which `CivilizationTraits` gives to
///   `CIVILIZATION_SWEDEN` and to nothing else. **A game without Sweden in it
///   never sees a Nobel prize.** That makes this the rarest of the seven rather
///   than a route every empire has, and it is the shipped rule; loosening it so
///   the lane completes more often would be inventing one.
///
/// `NOBEL_PRIZE_PEACE_SCORE_FROM_FAVOR` is `FromFavor="true" ScoreAmount="1"`,
/// described "Generating [ICON_Favor] Diplomatic Favor" — the same "Generating
/// … Per Turn" cadence the World's Fair uses, so it counts the favor an empire
/// *generates*, which is what `process_diplomacy` computes each turn. It is not
/// the balance, and a congress refund, a trade or an emergency award is not
/// favor the empire generated.
#[test]
fn the_nobel_peace_prize_needs_sweden_and_scores_the_favor_an_empire_generates() {
    let mut game = game_with_capitals(3, 64_000, 400);
    game.native_competitions = true;
    game.world_era = 4;
    for player in game.players.iter_mut() {
        if player.civ == "Sweden" {
            player.civ = "Rome".to_string();
        }
    }

    game.open_native_competition();
    assert!(
        game.competition.is_none(),
        "no Sweden, no Nobel prize: the emergency requires \
         TRAIT_CIVILIZATION_NOBEL_PRIZE to be in the game at all"
    );

    game.players[1].civ = "Sweden".to_string();
    assert!(
        game.has_ability(1, "nobelinstitution"),
        "Sweden carries the Nobel Institution in civs.json"
    );
    game.world_era = 3;
    game.open_native_competition();
    assert!(
        game.competition.is_none(),
        "and not before the Industrial era, whoever is playing"
    );

    game.world_era = 4;
    game.open_native_competition();
    assert_eq!(
        game.competition.as_ref().map(|c| c.kind.as_str()),
        Some("EMERGENCY_NOBEL_PRIZE_PEACE"),
        "with Sweden on the board the Industrial era can seat one"
    );

    // Favor generated scores; the class of every other source does not.
    game.score_favor_competition(0, 7.0);
    game.score_great_person_point_competition(0, 30.0);
    assert_eq!(
        game.competition.as_ref().unwrap().scores[&0],
        7.0,
        "Peace counts Favor, and Great Person Points score the World's Fair"
    );

    let dvp = game.players[0].dvp;
    let favor = game.players[0].diplomatic_favor;
    game.turn = game.competition.as_ref().unwrap().ends;
    game.close_native_competition();
    assert_eq!(
        game.players[0].dvp - dvp,
        1,
        "NON_EMERGENCY_FIRST_PLACE_VICTORY_POINT is worth one point"
    );
    assert_eq!(
        game.players[0].diplomatic_favor, favor,
        "and no Favor: the Nobel prizes pay their tiers in Great People, not \
         Favor, so EmergencyRewards has no favor row for this one"
    );
}

/// A tie pays nobody.
///
/// ⚠ `SCORED_COMPETITION_FIRST_PLACE_REQUIREMENTS` is a single requirement,
/// `REQUIREMENT_PLAYER_GOT_FIRST_PLACE_IN_EMERGENCY`, and nothing in the
/// shipped data says how the engine breaks a tie for it. Paying both, paying
/// the lower player id, or paying whoever scored first would each be a rule
/// this repository invented — the #2049 mistake — so a tie pays nobody and
/// spends the lockout.
#[test]
fn a_tied_competition_pays_nobody_and_still_spends_its_lockout() {
    let mut game = game_with_capitals(3, 64_001, 400);
    game.native_competitions = true;
    game.world_era = 5;
    game.open_native_competition();
    assert_eq!(
        game.competition.as_ref().map(|c| c.kind.as_str()),
        Some("EMERGENCY_WORLDS_FAIR")
    );

    game.score_great_person_point_competition(0, 12.0);
    game.score_great_person_point_competition(1, 12.0);
    let dvp: Vec<i64> = game.players.iter().map(|player| player.dvp).collect();
    let favor: Vec<f64> = game
        .players
        .iter()
        .map(|player| player.diplomatic_favor)
        .collect();

    game.turn = game.competition.as_ref().unwrap().ends;
    game.close_native_competition();
    for player in &game.players {
        assert_eq!(
            player.dvp, dvp[player.id],
            "a tie has no first place, so nobody is paid a Diplomatic Victory Point"
        );
        assert_eq!(
            player.diplomatic_favor, favor[player.id],
            "and nobody is paid the Favor either"
        );
    }
    assert!(
        game.competition_lockout_until["EMERGENCY_WORLDS_FAIR"] > game.turn,
        "the competition still ran, so its lockout is still spent"
    );

    // ⚠ And only a seat that could win the game scores at all. `begin_turn`
    // runs for every seat including city-states and eliminated empires, and a
    // city-state holds Campuses and generates Great Person Points like anyone
    // else — but an emergency's members are the majors, so a non-member that
    // outscored them would take the point off the board for nobody.
    game.competition_lockout_until.clear();
    game.turn += 1;
    game.open_native_competition();
    game.players[2].alive = false;
    game.score_great_person_point_competition(2, 500.0);
    game.score_favor_competition(2, 500.0);
    game.score_competition_holdings(2);
    assert!(
        !game.competition.as_ref().unwrap().scores.contains_key(&2),
        "a seat that is not in the victory race cannot score a competition"
    );

    // One clear leader is paid.
    game.competition_lockout_until.clear();
    game.turn += 1;
    game.open_native_competition();
    game.score_great_person_point_competition(0, 12.0);
    game.score_great_person_point_competition(1, 11.0);
    game.turn = game.competition.as_ref().unwrap().ends;
    game.close_native_competition();
    assert_eq!(
        game.players[0].dvp - dvp[0],
        1,
        "a single highest score is a first place"
    );
    assert_eq!(game.players[1].dvp, dvp[1]);
}

/// Nothing is paid on the mirrored path.
///
/// ★★★★★ THE INVARIANT THAT KEEPS THE LIVE BRIDGE HONEST. A mirrored seat's
/// competition is the live host's, and the host has already scored it and
/// already paid its own Diplomatic Victory Points; `dvp` on that path is
/// mirrored from the host, not accumulated here. So a host competition must
/// never reach the native scorer or the native award — including in a game that
/// has `native_competitions` switched on, because the mirror sets the same
/// fields on the same `Game`.
#[test]
fn a_mirrored_competition_pays_nothing_natively() {
    let mut game = game_with_capitals(3, 64_002, 400);
    game.native_competitions = true;
    game.world_era = 5;
    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_WORLDS_FAIR".to_string(),
        ends: game.turn + 20,
        ours: 40.0,
        leader: 40.0,
    }]);
    assert!(
        game.host_competition(0, "EMERGENCY_WORLDS_FAIR").is_some(),
        "the mirrored seat can see the host's competition"
    );
    assert!(
        game.competition.is_none(),
        "and it is not a native competition: a host one lives in \
         `host_competitions`, which nothing native reads or writes"
    );

    let dvp: Vec<i64> = game.players.iter().map(|player| player.dvp).collect();
    let favor: Vec<f64> = game
        .players
        .iter()
        .map(|player| player.diplomatic_favor)
        .collect();
    game.score_great_person_point_competition(0, 40.0);
    game.score_favor_competition(0, 40.0);
    game.score_competition_holdings(0);
    assert!(
        game.competition.is_none(),
        "no native score table is created for a host competition"
    );

    game.turn += 25;
    game.close_native_competition();
    for player in &game.players {
        assert_eq!(
            player.dvp, dvp[player.id],
            "the host has already counted its own Diplomatic Victory Points"
        );
        assert_eq!(player.diplomatic_favor, favor[player.id]);
    }
    assert!(
        game.competition_lockout_until.is_empty(),
        "and a host competition does not lock out a native one either"
    );
}

/// Every competition Gathering Storm pays a Diplomatic Victory Point for, and
/// nothing else.
///
/// ★★★★★ THE OTHER HALF OF THE TWENTY. The content sources above are worth
/// about nine points across a 250-turn game and the congress resolution ±2 from
/// the Modern era; the rest of a Diplomatic victory is meant to come from
/// competitions, which recur for the whole second half. Joining the shipped
/// `EmergencyRewards` to `ModifierArguments` names exactly seven of them:
///
/// | modifier | Amount | emergencies |
/// |---|---|---|
/// | `NON_EMERGENCY_FIRST_PLACE_VICTORY_POINT` | 1 | Nobel Peace, World's Fair, Space Station, World Games |
/// | `AID_REQUEST_FIRST_PLACE_VICTORY_POINT` | 2 | Send Aid, Send Military Aid |
/// | `CLIMATE_ACCORDS_FIRST_PLACE_VICTORY_POINT` | 2 | Climate Accords |
///
/// ⚠ Nobel **Literature** and Nobel **Physics** are scored competitions and are
/// deliberately not here: `EmergencyRewards` gives neither a victory-point row
/// at all. Adding them because they look like their sibling would invent two
/// points a game.
///
/// ⚠ The Favor is the emergency's own top-tier amount, not a shared constant. A
/// flat 25 shipped here until #2379 and appears in no shipped table.
#[test]
fn every_competition_that_pays_a_diplomatic_victory_point_is_seated_natively() {
    let table: Vec<(&str, i64, f64)> = Game::NATIVE_COMPETITIONS
        .iter()
        .map(|competition| {
            (
                competition.kind,
                competition.diplomatic_victory_points,
                competition.first_place_favor,
            )
        })
        .collect();
    let mut sorted = table.clone();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    assert_eq!(
        sorted,
        vec![
            ("EMERGENCY_CLIMATE_ACCORDS", 2, 100.0),
            ("EMERGENCY_NOBEL_PRIZE_PEACE", 1, 0.0),
            ("EMERGENCY_SEND_AID", 2, 100.0),
            ("EMERGENCY_SEND_MILITARY_AID", 2, 100.0),
            ("EMERGENCY_SPACE_STATION", 1, 50.0),
            ("EMERGENCY_WORLDS_FAIR", 1, 50.0),
            ("EMERGENCY_WORLD_GAMES", 1, 50.0),
        ],
        "a competition added or dropped here, or an award changed, is a change \
         to how a Diplomatic victory is won"
    );
    for (kind, points, _) in &table {
        assert_eq!(
            Game::competition_victory_points(kind),
            *points,
            "the AI prices a competition from this table and must not carry its own copy"
        );
    }

    // The seating order is the simplification this file owes the reader: the
    // shipped data says which competitions exist and when, and nothing about
    // which the congress picks among them.
    let congress: Vec<(&str, usize, Option<usize>)> = Game::NATIVE_COMPETITIONS
        .iter()
        .filter(|competition| competition.trigger == NativeCompetitionTrigger::Congress)
        .map(|competition| {
            (
                competition.kind,
                competition.minimum_world_era,
                competition.maximum_world_era,
            )
        })
        .collect();
    assert_eq!(
        congress,
        vec![
            ("EMERGENCY_SPACE_STATION", 8, None),
            ("EMERGENCY_CLIMATE_ACCORDS", 7, None),
            ("EMERGENCY_WORLD_GAMES", 6, None),
            ("EMERGENCY_WORLDS_FAIR", 5, Some(5)),
            ("EMERGENCY_NOBEL_PRIZE_PEACE", 4, None),
        ],
        "newest era first, so the latest competition an era has unlocked takes \
         the seat and the Nobel Peace Prize takes the Industrial era and the \
         gaps another kind's lockout leaves"
    );
}
