use super::*;

fn team_game(players: usize, teams: Vec<Option<usize>>, seed: u64) -> Game {
    Game::new_with(GameOptions {
        barbarians: false,
        teams,
        ..GameOptions::new(players, 24, 16, seed, 80, 0)
    })
}

fn found_capitals(game: &mut Game) -> Vec<u32> {
    let players: Vec<usize> = (0..game.players.len())
        .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
        .collect();
    let mut capitals = Vec::with_capacity(players.len());
    for pid in players {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        capitals.push(city);
    }
    capitals
}

#[test]
fn teams_persist_share_sight_and_grant_completed_technology_eurekas() {
    let mut game = team_game(4, vec![Some(0), Some(0), Some(1), Some(1)], 88_001);
    assert!(game.same_team(0, 1));
    assert!(!game.same_team(0, 2));
    assert!(game.are_allied(0, 1));
    assert!(game.are_friends(0, 1));
    assert!(game.has_open_borders(0, 1));
    assert!(game.do_declare_war(0, 1).is_err());

    let scout = game.player_unit_ids(0)[0];
    let seen = game.units[&scout].pos;
    for unit in game
        .units
        .keys()
        .copied()
        .filter(|unit| *unit != scout)
        .collect::<Vec<_>>()
    {
        game.remove_unit(unit);
    }
    assert!(game.player_visibility(1).contains(&seen));
    assert!(!game.player_visibility(2).contains(&seen));

    game.players[1].research = Some("pottery".to_string());
    game.players[1].research_progress = 0.0;
    game.share_team_technology_boost(0, "pottery");
    assert!(game.players[1].boosted_techs.contains(&crate::name!("pottery")));
    assert_eq!(
        game.players[1].research_progress,
        0.4 * game.tech_cost("pottery")
    );
    assert!(!game.players[2].boosted_techs.contains(&crate::name!("pottery")));

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.team_members(0), vec![0, 1]);
    assert_eq!(restored.players[2].team, Some(1));
}

#[test]
fn declaring_and_ending_war_moves_both_complete_teams() {
    let mut game = team_game(4, vec![Some(0), Some(0), Some(1), Some(1)], 88_002);
    game.record_contact(0, 2);
    game.do_declare_war(0, 2).unwrap();
    for attacker in [0, 1] {
        for defender in [2, 3] {
            assert!(game.is_at_war(attacker, defender));
        }
    }
    assert!(!game.is_at_war(0, 1));
    assert!(!game.is_at_war(2, 3));
    assert_eq!(game.at_war.len(), 4);

    // A war runs its shipped minimum before either side may settle it.
    game.turn += 10;
    game.do_make_peace(1, 3).unwrap();
    for attacker in [0, 1] {
        for defender in [2, 3] {
            assert!(!game.is_at_war(attacker, defender));
        }
    }
    assert!(game.at_war.is_empty());
}

#[test]
fn teammate_population_is_neutral_to_loyalty_pressure() {
    let mut teamed = team_game(2, vec![Some(0), Some(0)], 88_003);
    let cities = found_capitals(&mut teamed);
    let own = cities[0];
    let teammate = cities[1];
    // Place the teammate inside the pressure radius and make its
    // population overwhelming. Only the relationship differs below.
    let own_pos = teamed.cities[&own].pos;
    let near = teamed
        .wdisk(own_pos, 2)
        .into_iter()
        .find(|position| *position != own_pos)
        .unwrap();
    let old = teamed.cities[&teammate].pos;
    teamed.city_by_pos.remove(&old);
    teamed.city_by_pos.insert(near, teammate);
    teamed.cities.get_mut(&teammate).unwrap().pos = near;
    teamed.cities.get_mut(&teammate).unwrap().pop = 20;
    teamed.cities.get_mut(&own).unwrap().loyalty = 50.0;
    let mut rival = teamed.clone();
    rival.players[1].team = Some(1);

    teamed.process_loyalty(0);
    rival.process_loyalty(0);
    assert!(teamed.cities[&own].loyalty > rival.cities[&own].loyalty);
    assert_eq!(teamed.cities[&own].owner, 0);
}

#[test]
fn team_domination_religion_score_and_terminal_credit_follow_stock_rules() {
    let mut domination =
        team_game(4, vec![Some(0), Some(0), Some(1), Some(1)], 88_004);
    let capitals = found_capitals(&mut domination);
    // A team victory needs both friendly capitals retained and every
    // opponent to lose its own; teammates need not personally hold them.
    domination.cities.get_mut(&capitals[2]).unwrap().owner = 3;
    domination.cities.get_mut(&capitals[3]).unwrap().owner = 2;
    domination.check_domination();
    assert_eq!(domination.winner, Some(0));
    assert_eq!(domination.winning_players(), vec![0, 1]);

    let mut religion = team_game(4, vec![Some(0), Some(0), Some(1), Some(1)], 88_005);
    let capitals = found_capitals(&mut religion);
    religion.players[0].religion = Some("First Faith".to_string());
    religion.players[1].religion = Some("Second Faith".to_string());
    for (city, faith) in [
        (capitals[2], "First Faith"),
        (capitals[3], "Second Faith"),
    ] {
        let city = religion.cities.get_mut(&city).unwrap();
        city.atheist_pressure = 0.0;
        city.pressure = BTreeMap::from([(faith.to_string(), 1_000.0)]);
    }
    religion.check_religious_victory();
    assert_eq!(religion.winning_players(), vec![0, 1]);

    let team_score = religion.team_score_rank_key(0).0;
    assert_eq!(team_score, religion.score(0) + religion.score(1));
}
