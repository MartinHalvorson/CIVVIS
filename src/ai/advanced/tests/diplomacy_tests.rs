//! Tests of `advanced/diplomacy.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn a_religion_plan_offers_peace_to_unblock_its_spread_lane() {
    // The t200 shape from run civvis-20260807T224914Z: strategy=religion,
    // met two rivals, at war with both, every revealed foreign city
    // excluded by the offensive-spread war filter.
    let (mut game, _, _) = timed_war_fixture(5);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));

    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };

    let mut treated = AdvancedAi::new();
    treated.enable_religion_sues_peace();
    treated.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        treated.peace_offers.contains(&1),
        "a religion plan at war must offer peace"
    );

    let mut frozen = AdvancedAi::new();
    frozen.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        !frozen.peace_offers.contains(&1),
        "frozen controllers keep the recorded posture"
    );

    let mut bridged = AdvancedAi::new();
    bridged.enable_live_bridge_universe();
    assert!(bridged.religion_sues_peace);
    bridged.disable_religion_sues_peace();
    assert!(!bridged.religion_sues_peace);
}

#[test]
fn a_siege_arrival_is_not_a_stalled_war_only_with_v2_gene() {
    // A march to a hostile city can outlast the twelve-turn fatigue window.
    // The retained opt-in resets that clock on its first land-unit arrival,
    // but does not keep resetting it while the unit waits on the same ring.
    assert!(
        GENES
            .iter()
            .any(|gene| gene.tag == "siege-is-progress-2" && gene.opt_in()),
        "the retained arrival behavior remains a standalone opt-in gene"
    );
    let (mut game, _, rival_capital) = timed_war_fixture(11);
    let rival_capital_position = game.cities[&rival_capital].pos;
    found_nearby_test_city(&mut game, 1, rival_capital_position);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    game.turn = 120;

    let landing = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (1..=2).contains(&game.wdist(**position, rival_capital_position))
        })
        .map(|(position, _)| *position)
        .min()
        .expect("the rival capital has a land approach within two tiles");
    game.spawn_test_unit("warrior", 0, landing);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };

    let mut arriving = AdvancedAi::new();
    arriving.enable_siege_is_progress_2();
    arriving.major_war_since = Some(80);
    arriving.last_campaign_progress = 80;
    // A controller mid-game has been counting all along; a zeroed count
    // would trip the frozen count-based clock on the first observation.
    arriving.last_city_count = game.player_city_ids(0).len();
    arriving.observe_campaign(&game, 0);
    assert_eq!(
        arriving.last_campaign_progress, game.turn,
        "the first land-unit arrival is campaign progress"
    );
    arriving.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        !arriving.peace_offers.contains(&1),
        "a campaign that has just reached its target must not be offered away"
    );

    game.turn = 121;
    arriving.observe_campaign(&game, 0);
    assert_eq!(
        arriving.last_campaign_progress, 120,
        "waiting on the same ring does not refresh the arrival reset"
    );

    let mut without_gene = AdvancedAi::new();
    without_gene.major_war_since = Some(80);
    without_gene.last_campaign_progress = 80;
    without_gene.last_city_count = game.player_city_ids(0).len();
    without_gene.observe_campaign(&game, 0);
    assert_eq!(
        without_gene.last_campaign_progress, 80,
        "without the opt-in, arrival does not alter the fatigue clock"
    );
    without_gene.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        without_gene.peace_offers.contains(&1),
        "without the opt-in, a fatigued campaign may be offered away"
    );
}

#[test]
fn siege_progress_v3_requires_damage_and_spends_one_reset_per_city() {
    let v3 = GENES
        .iter()
        .find(|gene| gene.tag == "siege-is-progress-3")
        .expect("version 3 is published for gene_screen");
    assert!(v3.opt_in() && v3.screenable() && !v3.live());
    assert!(!AdvancedAi::new().siege_is_progress_3);
    assert!(!AdvancedAi::legacy().siege_is_progress_3);

    let mut family = AdvancedAi::new();
    family.enable_siege_is_progress_2();
    assert!(family.siege_is_progress_2 && !family.siege_is_progress_3);
    family.enable_siege_is_progress_3();
    assert!(!family.siege_is_progress_2 && family.siege_is_progress_3);
    family.enable_siege_is_progress_2();
    assert!(family.siege_is_progress_2 && !family.siege_is_progress_3);
    family.enable_siege_is_progress_3();
    family.disable_siege_is_progress_3();
    assert!(!family.siege_is_progress_2 && !family.siege_is_progress_3);

    let (mut game, _, rival_capital) = timed_war_fixture(11);
    let rival_capital_position = game.cities[&rival_capital].pos;
    found_nearby_test_city(&mut game, 1, rival_capital_position);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    game.turn = 120;
    let landing = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (1..=2).contains(&game.wdist(**position, rival_capital_position))
        })
        .map(|(position, _)| *position)
        .min()
        .expect("the rival capital has a land approach within two tiles");
    game.spawn_test_unit("warrior", 0, landing);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut proven = AdvancedAi::new();
    proven.enable_siege_is_progress_3();
    proven.major_war_since = Some(80);
    proven.last_campaign_progress = 80;
    proven.last_city_count = game.player_city_ids(0).len();

    proven.observe_campaign(&game, 0);
    assert_eq!(
        proven.last_campaign_progress, 80,
        "mere proximity is not proof that the siege is progressing"
    );
    proven.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        proven.peace_offers.contains(&1),
        "an arrived but ineffective force may still end a fatigued war"
    );

    game.turn = 121;
    game.cities.get_mut(&rival_capital).unwrap().hp -= 20;
    proven.observe_campaign(&game, 0);
    assert_eq!(
        proven.last_campaign_progress, 121,
        "observed damage under nearby friendly pressure earns one reset"
    );
    assert!(proven.campaign_cities_pressured_v3.contains(&rival_capital));

    game.turn = 122;
    game.cities.get_mut(&rival_capital).unwrap().hp -= 20;
    proven.observe_campaign(&game, 0);
    assert_eq!(
        proven.last_campaign_progress, 121,
        "further damage to the same city cannot renew fatigue indefinitely"
    );
}

#[test]
fn banked_envoys_reclaim_a_nonobjective_hostile_suzerain_city_state() {
    // A city-state that follows its hostile Suzerain into war cannot take
    // an Envoy until peace with that major lands. The live bridge should
    // make that one tactical peace offer when its deliberately idle pool
    // can immediately flip the city-state, but never trade away an active
    // campaign or mistake a direct city-state war for a derived one.
    let mut game = Game::new_full(2, 24, 16, 7_925, 250, 1, false);
    let city_state = game
        .players
        .iter()
        .find(|player| player.alive && player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("fixture needs a city-state");
    game.turn = 80;
    game.current = 0;
    game.record_contact(0, 1);
    game.record_contact(0, city_state);
    game.players[0].envoys = vec![(city_state, 4)];
    game.players[1].envoys = vec![(city_state, 5)];
    game.players[0].envoys_free = 2;
    game.at_war.insert((0, 1));
    assert_eq!(game.suzerain_of(city_state), Some(1));
    assert!(game.is_at_war(0, city_state));
    assert!(
        !game.can_send_envoy(0, city_state),
        "a Suzerain-derived war makes the city-state Envoy-ineligible"
    );

    let nonobjective = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };

    let mut live_game = game.clone();
    let mut live = AdvancedAi::new();
    live.enable_bank_envoys();
    live.advanced_diplomacy(&mut live_game, 0, &nonobjective);
    assert!(
        live.peace_offers.contains(&1),
        "two banked Envoys take the city-state after peace with its Suzerain"
    );
    // The host reserves this exact cost while it submits the major peace.
    // On the authoritative post-peace frame the normal Envoy pass spends
    // it and flips the city-state.
    live_game.at_war.remove(&(0, 1));
    live.advanced_envoys(&mut live_game, 0, GrandStrategy::Science, None);
    assert_eq!(live_game.envoys_at(0, city_state), 6);
    assert_eq!(live_game.suzerain_of(city_state), Some(0));

    let mut stock_game = game.clone();
    let mut stock = AdvancedAi::new();
    stock.advanced_diplomacy(&mut stock_game, 0, &nonobjective);
    assert!(
        !stock.peace_offers.contains(&1),
        "ordinary controllers retain their historical diplomacy"
    );

    let active_campaign = StrategicPlan {
        target_player: Some(1),
        ..nonobjective.clone()
    };
    let mut campaign_game = game.clone();
    let mut protecting = AdvancedAi::new();
    protecting.enable_bank_envoys();
    protecting.advanced_diplomacy(&mut campaign_game, 0, &active_campaign);
    assert!(
        !protecting.peace_offers.contains(&1),
        "a city-state investment must not abandon the active campaign"
    );

    let mut direct_war_game = game;
    direct_war_game.at_war.insert((0, city_state));
    let mut direct_war = AdvancedAi::new();
    direct_war.enable_bank_envoys();
    direct_war.advanced_diplomacy(&mut direct_war_game, 0, &nonobjective);
    assert!(
        !direct_war.peace_offers.contains(&1),
        "peace with a Suzerain cannot unlock Envoys during a direct city-state war"
    );
}

#[test]
fn timed_war_holds_an_incomplete_package_then_declares_together() {
    let (mut game, _, objective) = timed_war_fixture(940_009);
    clear_military(&mut game, 0);
    let mut ai = AdvancedAi::new();
    ai.timed_war = true;
    game.players[0].techs.insert(crate::name!("nuclear_fusion"));
    ai.war_plan = Some(timed_plan(
        &game,
        objective,
        crate::name!("nuclear_fusion"),
        crate::name!("giant_death_robot"),
        None,
        None,
        WarPhase::Strike,
    ));
    ai.war_plan.as_mut().unwrap().tech_turn = Some(game.turn - 5);
    game.players[0].denounced_until.insert(1, game.turn + 25);
    game.players[0].denounced_since.insert(1, game.turn - 5);
    let objective_pos = game.cities[&objective].pos;
    let mut staged = Vec::new();
    for position in game.wdisk(objective_pos, 5) {
        if staged.len() == TIMED_WAR_BODIES {
            break;
        }
        if g_is_open_land(&game, position) && game.unit_ids_at(position).is_empty() {
            let unit = game.spawn_test_unit("giant_death_robot", 0, position);
            if ai.campaign_staging_position(&game, 0, 1, unit, objective_pos, position) {
                staged.push(unit);
            } else {
                game.remove_unit(unit);
            }
        }
    }
    assert_eq!(
        staged.len(),
        TIMED_WAR_BODIES,
        "fixture needs four lawful staging tiles"
    );
    let fourth = staged.pop().unwrap();
    let fourth_position = game.units[&fourth].pos;
    game.remove_unit(fourth);
    assert!(ai.timed_war_opening(&mut game, 0, 1));
    assert!(!game.is_at_war(0, 1), "three bodies cannot launch");
    game.spawn_test_unit("giant_death_robot", 0, fourth_position);
    assert!(ai.timed_war_opening(&mut game, 0, 1));
    assert!(game.is_at_war(0, 1));
    assert_eq!(ai.war_census.declarations, 1);
    assert_eq!(ai.war_census.complete_package_declarations, 1);
    assert_eq!(ai.war_plan.as_ref().unwrap().phase, WarPhase::Exploit);
}

/// Nobel Peace scores Favor generated during the competition. A city-state
/// suzerainty is a +1 Favor-per-turn source, so when the free Envoys already
/// in hand can secure one this turn it must beat a normally better-aligned
/// city-state. Other Nobel types and a Peace clock too short for next-turn
/// Favor retain the ordinary allocation.
#[test]
fn live_nobel_peace_envoys_price_an_affordable_suzerainty_favor_stream() {
    let board = || {
        let mut game = Game::new_full(2, 32, 24, 5_419, 250, 2, false);
        let minors: Vec<usize> = game
            .players
            .iter()
            .filter(|player| player.alive && player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        assert_eq!(minors.len(), 2, "the fixture needs two city-states");
        for minor in minors.iter().copied() {
            game.record_contact(0, minor);
        }
        // Contact may grant its own discovery Envoy. This test prices only
        // the three Envoys the player is choosing between now.
        game.players[0].envoys.clear();
        game.players[0].envoys_free = 3;
        let peace_target = minors[0];
        let science_target = minors[1];
        game.players[peace_target].civ = "Cahokia".to_string();
        game.players[science_target].civ = "Hattusa".to_string();
        // Hattusa is the ordinary Science favorite but eleven Envoys away
        // from a takeover. Cahokia is the one current three-Envoy route to a
        // new Favor source, so a live Peace clock must change the choice.
        game.players[1].envoys = vec![(science_target, 10)];
        (game, peace_target, science_target)
    };

    let (mut ordinary, peace_target, science_target) = board();
    AdvancedAi::new().advanced_envoys(&mut ordinary, 0, GrandStrategy::Science, None);
    assert_eq!(ordinary.envoys_at(0, peace_target), 0);
    assert_eq!(ordinary.envoys_at(0, science_target), 3);

    let (mut wrong_nobel, peace_target, science_target) = board();
    wrong_nobel.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_LITERATURE".to_string(),
        ends: wrong_nobel.turn + 30,
        ours: 0.0,
        leader: 30.0,
    }]);
    AdvancedAi::new().advanced_envoys(&mut wrong_nobel, 0, GrandStrategy::Science, None);
    assert_eq!(wrong_nobel.envoys_at(0, peace_target), 0);
    assert_eq!(wrong_nobel.envoys_at(0, science_target), 3);

    let (mut peace, peace_target, science_target) = board();
    peace.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: peace.turn + 30,
        ours: 0.0,
        leader: 30.0,
    }]);
    AdvancedAi::new().advanced_envoys(&mut peace, 0, GrandStrategy::Science, None);
    assert_eq!(
        peace.envoys_at(0, peace_target),
        3,
        "the current pool reaches a live Favor source immediately"
    );
    assert_eq!(peace.suzerain_of(peace_target), Some(0));
    assert_eq!(peace.envoys_at(0, science_target), 0);
    assert_eq!(peace.players[0].envoys_free, 0);

    let (mut expiring, peace_target, science_target) = board();
    expiring.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: expiring.turn + 1,
        ours: 0.0,
        leader: 30.0,
    }]);
    AdvancedAi::new().advanced_envoys(&mut expiring, 0, GrandStrategy::Science, None);
    assert_eq!(
        expiring.envoys_at(0, peace_target),
        0,
        "a suzerainty whose Favor starts after the clock receives no stale score"
    );
    assert_eq!(expiring.envoys_at(0, science_target), 3);
}

#[test]
fn explicit_non_diplomatic_targets_do_not_score_congress_points() {
    use crate::game::{CongressResolution, CongressSession};

    let session = || CongressSession {
        convened: 0,
        closes: 5,
        resolutions: vec![
            CongressResolution {
                id: "world_leader".to_string(),
                title: "Diplomatic Victory".to_string(),
                choices: vec!["0".to_string(), "1".to_string()],
                ballots: BTreeMap::new(),
            },
            CongressResolution {
                id: "international_aid".to_string(),
                title: "International Aid".to_string(),
                choices: vec!["0".to_string(), "1".to_string()],
                ballots: BTreeMap::new(),
            },
        ],
    };
    let plan = |strategy| StrategicPlan {
        strategy,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 0,
        rush: false,
    };

    let mut science_game = Game::new(2, 24, 16, 77, 80, 0);
    science_game.congress = Some(session());
    AdvancedAi::targeting(VictoryTarget::Science).advanced_diplomacy(
        &mut science_game,
        0,
        &plan(GrandStrategy::Science),
    );
    let science_resolutions = &science_game.congress.as_ref().unwrap().resolutions;
    assert!(!science_resolutions[0].ballots.contains_key(&0));
    assert!(!science_resolutions[1].ballots.contains_key(&0));

    let mut diplomacy_game = Game::new(2, 24, 16, 78, 80, 0);
    diplomacy_game.congress = Some(session());
    AdvancedAi::targeting(VictoryTarget::Diplomacy).advanced_diplomacy(
        &mut diplomacy_game,
        0,
        &plan(GrandStrategy::Diplomacy),
    );
    assert!(diplomacy_game.congress.as_ref().unwrap().resolutions[0]
        .ballots
        .contains_key(&0));
}

#[test]
fn diplomatic_congress_spends_the_full_affordable_online_bank() {
    use crate::game::{CongressResolution, CongressSession};
    use crate::setup::GameSpeed;

    let mut game = Game::new(2, 24, 16, 79, 80, 0);
    game.game_speed = GameSpeed::Online;
    game.players[0].diplomatic_favor = 352.0;
    game.congress = Some(CongressSession {
        convened: 0,
        closes: 5,
        resolutions: vec![CongressResolution {
            id: "world_leader".to_string(),
            title: "Diplomatic Victory".to_string(),
            choices: vec![
                "A:0".to_string(),
                "B:0".to_string(),
                "A:1".to_string(),
                "B:1".to_string(),
            ],
            ballots: BTreeMap::new(),
        }],
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 0,
        rush: false,
    };

    AdvancedAi::targeting(VictoryTarget::Diplomacy).advanced_diplomacy(&mut game, 0, &plan);

    assert_eq!(
        game.congress.as_ref().unwrap().resolutions[0].ballots[&0].1,
        13,
        "the diplomatic plan should use the host-affordable ballot"
    );
    assert_eq!(game.players[0].diplomatic_favor, 40.0);
}

#[test]
fn congress_strategy_contests_leaders_and_predicts_competitions() {
    let mut game = Game::new_full(3, 24, 16, 780, 200, 0, false);
    game.players[0].dvp = 10;
    game.players[1].dvp = 18;
    game.players[2].dvp = 3;
    game.players[1].culture_lifetime = 2_000.0;
    let ai = AdvancedAi::new();
    let resolution = |id: &str| CongressResolution {
        id: id.to_string(),
        title: id.to_string(),
        choices: vec!["0".to_string(), "1".to_string(), "2".to_string()],
        ballots: BTreeMap::new(),
    };

    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &resolution("world_leader"),
            GrandStrategy::Expansion,
        ),
        Some("2".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &resolution("world_leader"),
            GrandStrategy::Diplomacy,
        ),
        Some("0".to_string())
    );
    assert_eq!(
        ai.congress_choice(&game, 0, &resolution("world_fair"), GrandStrategy::Science,),
        Some("1".to_string())
    );
    assert_eq!(
        ai.congress_choice(&game, 0, &resolution("world_fair"), GrandStrategy::Culture,),
        Some("0".to_string())
    );

    let outcome_resolution = |id: &str, targets: &[&str]| CongressResolution {
        id: id.to_string(),
        title: id.to_string(),
        choices: ["A", "B"]
            .into_iter()
            .flat_map(|outcome| {
                targets
                    .iter()
                    .map(move |target| format!("{outcome}:{target}"))
            })
            .collect(),
        ballots: BTreeMap::new(),
    };
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("world_leader", &["0", "1", "2"]),
            GrandStrategy::Expansion,
        ),
        Some("B:1".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("world_leader", &["0", "1", "2"]),
            GrandStrategy::Diplomacy,
        ),
        Some("A:0".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("mercenary_companies", &["production", "gold", "faith"]),
            GrandStrategy::Conquest,
        ),
        Some("B:production".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution(
                "urban_development_treaty",
                &["campus", "theater_square", "holy_site"],
            ),
            GrandStrategy::Science,
        ),
        Some("A:campus".to_string())
    );

    game.players[1]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 3);
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("arms_control", &["0", "1", "2"]),
            GrandStrategy::Conquest,
        ),
        Some("B:1".to_string())
    );
    game.players[0]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 1);
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("arms_control", &["0", "1", "2"]),
            GrandStrategy::Diplomacy,
        ),
        Some("A:2".to_string())
    );

    game.players[0].government = Some("autocracy".to_string());
    game.players[1].government = Some("democracy".to_string());
    game.players[2].government = Some("democracy".to_string());
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("world_ideology", &["autocracy", "democracy"]),
            GrandStrategy::Science,
        ),
        Some("A:autocracy".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("border_control_treaty", &["0", "1", "2"]),
            GrandStrategy::Expansion,
        ),
        Some("A:0".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution(
                "public_works_program",
                &["launch_earth_satellite", "manhattan_project"],
            ),
            GrandStrategy::Science,
        ),
        Some("A:launch_earth_satellite".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution(
                "global_energy_treaty",
                &["coal_power_plant", "oil_power_plant", "nuclear_power_plant"],
            ),
            GrandStrategy::Science,
        ),
        Some("A:nuclear_power_plant".to_string())
    );
    assert_eq!(
        ai.congress_choice(
            &game,
            0,
            &outcome_resolution("deforestation_treaty", &["forest"]),
            GrandStrategy::Expansion,
        ),
        Some("A:forest".to_string())
    );
}

#[test]
fn strategic_diplomacy_prices_incoming_deals_and_rejects_victory_leaders() {
    let mut game = Game::new_full(2, 24, 16, 781, 300, 0, false);
    let ai = AdvancedAi::new();
    let mut plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let expires = game.turn + 10;
    let deal = |give_gold, request_gold, friendship, peace| DiplomaticDeal {
        id: 1,
        from: 1,
        to: 0,
        give_gold,
        request_gold,
        open_borders: false,
        friendship,
        peace,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires,
    };

    assert!(ai.incoming_deal_value(&game, 0, &deal(0.0, 100.0, true, false), &plan) < 0.0);
    assert!(ai.incoming_deal_value(&game, 0, &deal(10.0, 0.0, true, false), &plan) > 0.0);

    plan.strategy = GrandStrategy::Conquest;
    assert!(
        ai.incoming_deal_value(&game, 0, &deal(10.0, 0.0, true, false), &plan) < 0.0,
        "a campaign target must not be protected by a new friendship"
    );
    plan.strategy = GrandStrategy::Expansion;

    game.players[1].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
        "launch_mars_colony".to_string(),
        "exoplanet_expedition".to_string(),
    ]);
    game.players[1].exoplanet_distance = 42.0;
    assert!(ai.incoming_deal_value(&game, 0, &deal(10.0, 0.0, true, false), &plan) < 0.0);

    game.at_war.insert((0, 1));
    plan.strategy = GrandStrategy::Recovery;
    assert!(
        ai.incoming_deal_value(&game, 0, &deal(0.0, 100.0, false, true), &plan) < 0.0,
        "a strong Recovery posture must not abandon its active campaign target"
    );
    plan.target_player = None;
    assert!(ai.incoming_deal_value(&game, 0, &deal(0.0, 100.0, false, true), &plan) > 0.0);
}

#[test]
fn outmatched_major_must_negotiate_peace_with_the_winning_campaign() {
    let mut game = Game::new_full(2, 24, 16, 7_922, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let staging = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..3 {
        game.spawn_test_unit("modern_armor", 0, staging);
    }
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    game.turn = game
        .peace_available_at(0, 1)
        .expect("the new war has a mandatory minimum");
    assert!(game.military_power(1) < game.military_power(0) * 0.62);

    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut defender = AdvancedAi::new();
    defender.major_war_since = Some(60);
    game.current = 1;
    defender.advanced_diplomacy(&mut game, 1, &recovery);

    assert!(
        game.is_at_war(0, 1),
        "an outmatched defender cannot impose white peace unilaterally"
    );
    assert!(game
        .pending_deals
        .iter()
        .any(|deal| { deal.from == 1 && deal.to == 0 && deal.peace }));

    let winning_campaign = StrategicPlan {
        // A threatened home city can temporarily classify even a much
        // stronger attacker as Recovery. Its active campaign target must
        // still be able to refuse the defender's immediate white peace.
        strategy: GrandStrategy::Recovery,
        target_player: Some(1),
        target_city: game.player_city_ids(1).into_iter().next(),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut refused = game.clone();
    let mut conqueror = AdvancedAi::new();
    conqueror.major_war_since = Some(60);
    refused.current = 0;
    conqueror.advanced_diplomacy(&mut refused, 0, &winning_campaign);
    assert!(refused.is_at_war(0, 1));
    assert!(
        refused.pending_deals.iter().all(|deal| !deal.peace),
        "the stronger conquest plan should reject an immediate white peace"
    );

    let mut accepting = AdvancedAi::new();
    accepting.major_war_since = Some(60);
    game.current = 0;
    accepting.advanced_diplomacy(&mut game, 0, &recovery);
    assert!(!game.is_at_war(0, 1));
    assert_eq!(accepting.peace_until, game.turn + 30);
    assert!(accepting.major_war_since.is_none());
}

#[test]
fn an_explicit_science_lane_ends_a_losing_threatened_war_directly() {
    let mut game = Game::new_full(2, 24, 16, 7_923, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let staging = game.cities[&game.player_city_ids(0)[0]].pos;
    let defender_staging = game.cities[&game.player_city_ids(1)[0]].pos;
    game.spawn_test_unit("modern_armor", 0, staging);
    game.spawn_test_unit("modern_armor", 1, defender_staging);
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 0, staging);
    }
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    game.turn = game
        .peace_available_at(0, 1)
        .expect("the new war has a mandatory minimum");
    assert!(game.military_power(1) < game.military_power(0) * 0.85);
    assert!(game.military_power(1) >= game.military_power(0) * 0.62);

    let threatened_city = game.player_city_ids(1).into_iter().next();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.major_war_since = Some(60);
    game.current = 1;
    science.advanced_diplomacy(&mut game, 1, &plan);

    assert!(
        !game.is_at_war(0, 1),
        "a losing siege must not wait for the attacker to accept a peace offer"
    );
    assert_eq!(science.peace_until, game.turn + 30);
    assert!(science.major_war_since.is_none());
}

#[test]
fn advanced_ai_proposes_the_alliance_for_its_victory_plan() {
    let mut game = Game::new_full(3, 24, 16, 782, 300, 0, false);
    game.turn = 12;
    // There is nobody to ally with until the table has been introduced.
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    for player in game.players.iter_mut() {
        player.civics.insert(crate::name!("civil_service"));
        player.techs.insert(crate::name!("scientific_theory"));
    }
    game.players[1].techs.insert(crate::name!("radio"));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(game.legal_actions(0).iter().any(|action| {
        matches!(
            action,
            Action::ProposeDeal {
                alliance: Some(kind),
                ..
            } if kind == "research"
        )
    }));
    assert!(ai.rival_victory_pressure(&game, 1).progress < 82);
    ai.propose_strategic_alliance(&mut game, 0, &plan, None);
    let proposal = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .unwrap();
    assert_eq!(proposal.alliance.as_deref(), Some("research"));
    assert!(proposal.friendship);
}

/// A new alliance is a +1 Favor-per-turn stream. Nobel Peace scores that
/// stream on the host clock, so the ordinary twelve-turn proposal cadence must
/// not delay a legal strategic alliance until the contest has already lost
/// those points. Other Nobel kinds and a Peace clock too short for next-turn
/// Favor retain the quiet cadence.
#[test]
fn live_nobel_peace_proposes_an_alliance_without_waiting_for_its_cadence() {
    let board = || {
        let mut game = Game::new_full(3, 24, 16, 7_842, 300, 0, false);
        game.turn = 13;
        for pid in 0..3 {
            for other in pid + 1..3 {
                game.record_contact(pid, other);
            }
        }
        for player in game.players.iter_mut() {
            player.civics.insert(crate::name!("civil_service"));
            player.techs.insert(crate::name!("scientific_theory"));
        }
        game.players[1].techs.insert(crate::name!("radio"));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        (game, plan)
    };
    let propose = |game: &mut Game, plan: &StrategicPlan| {
        AdvancedAi::targeting(VictoryTarget::Science)
            .propose_strategic_alliance(game, 0, plan, None);
    };

    let (mut ordinary, plan) = board();
    assert_ne!(
        ordinary.turn % 12,
        0,
        "the fixture must miss the usual cadence"
    );
    propose(&mut ordinary, &plan);
    assert!(
        ordinary.pending_deals.is_empty(),
        "outside a host Peace contest the ordinary cadence stays unchanged"
    );

    let (mut wrong_nobel, plan) = board();
    wrong_nobel.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_LITERATURE".to_string(),
        ends: wrong_nobel.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    propose(&mut wrong_nobel, &plan);
    assert!(
        wrong_nobel.pending_deals.is_empty(),
        "Nobel Literature scores Great Person points, not the alliance Favor stream"
    );

    let (mut peace, plan) = board();
    peace.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: peace.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    propose(&mut peace, &plan);
    let proposal = peace
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .expect("a live Peace clock must ask the ready ally now");
    assert_eq!(proposal.alliance.as_deref(), Some("research"));
    assert!(proposal.friendship);

    let (mut expiring, plan) = board();
    expiring.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: expiring.turn + 1,
        ours: 0.0,
        leader: 50.0,
    }]);
    propose(&mut expiring, &plan);
    assert!(
        expiring.pending_deals.is_empty(),
        "an alliance whose Favor starts after the deadline does not bypass cadence"
    );
}

#[test]
fn strategic_alliance_skips_an_unmet_major_and_reaches_a_known_one() {
    let mut game = Game::new_full(3, 24, 16, 783, 300, 0, false);
    game.turn = 12;
    for player in 0..game.players.len() {
        game.players[player].met.clear();
        game.players[player]
            .civics
            .insert(crate::name!("civil_service"));
        game.players[player]
            .techs
            .insert(crate::name!("scientific_theory"));
    }
    game.record_contact(0, 2);
    // The hidden player has the larger Science complement and would win
    // the score if its unrevealed state were allowed into the ranking.
    game.players[1].techs.insert(crate::name!("radio"));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };

    AdvancedAi::targeting(VictoryTarget::Science)
        .propose_strategic_alliance(&mut game, 0, &plan, None);

    assert_eq!(game.pending_deals.len(), 1);
    assert_eq!(game.pending_deals[0].to, 2);
    assert_eq!(game.pending_deals[0].alliance.as_deref(), Some("research"));
}

#[test]
fn war_opening_waits_for_formal_war_but_interrupts_for_imminent_victory() {
    let mut game = Game::new_full(2, 24, 16, 712, 300, 0, false);
    // A denunciation needs somebody to denounce.
    game.record_contact(0, 1);
    game.current = 0;
    game.turn = 60;
    let ai = AdvancedAi::new();

    assert_eq!(
        ai.preferred_war_opening(&game, 0, 1),
        Some(Action::Denounce { player: 1 })
    );
    game.apply(0, &Action::Denounce { player: 1 }).unwrap();
    game.turn = 64;
    assert_eq!(ai.preferred_war_opening(&game, 0, 1), None);
    game.turn = 65;
    assert_eq!(
        ai.preferred_war_opening(&game, 0, 1),
        Some(Action::DeclareWarWithCasusBelli {
            player: 1,
            casus_belli: "formal_war".to_string(),
        })
    );

    let mut emergency = Game::new_full(2, 24, 16, 713, 300, 0, false);
    emergency.record_contact(0, 1);
    emergency.current = 0;
    emergency.turn = 60;
    emergency.players[1]
        .science_projects
        .insert("exoplanet_expedition".to_string());
    assert_eq!(
        ai.preferred_war_opening(&emergency, 0, 1),
        Some(Action::DeclareWar { player: 1 })
    );

    let mut religious = Game::new_full(4, 24, 16, 714, 300, 0, false);
    for pid in 0..4 {
        for other in pid + 1..4 {
            religious.record_contact(pid, other);
        }
    }
    for pid in 0..4 {
        religious.current = pid;
        let settler = religious
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| religious.units[unit].kind == "settler")
            .unwrap();
        religious
            .apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    religious.current = 0;
    religious.turn = 60;
    religious.players[1].religion = Some("Runaway Faith".to_string());
    for owner in [1, 2, 3] {
        let city = religious.player_city_ids(owner)[0];
        religious
            .cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Runaway Faith".to_string(), 1_000.0);
    }
    assert_eq!(
        ai.preferred_war_opening(&religious, 0, 1),
        Some(Action::DeclareWar { player: 1 }),
        "a religious match point cannot spend five turns preparing a formal war"
    );
}

#[test]
fn culture_quick_deals_buy_the_direction_that_increases_our_tourism() {
    let mut game = Game::new_full(2, 18, 10, 79_004, 200, 0, false);
    // A quick deal is offered to a counterparty, so there has to be one.
    game.record_contact(0, 1);
    game.turn = 6;
    game.players[0].gold = 1_000.0;
    game.players[1].gold = 1_000.0;
    game.players[0].civics.insert(crate::name!("early_empire"));
    game.players[1].civics.insert(crate::name!("early_empire"));

    AdvancedAi::targeting(VictoryTarget::Culture).strategic_bilateral_trade(
        &mut game,
        0,
        None,
        GrandStrategy::Expansion,
    );

    assert!(game.has_open_borders(0, 1));
    assert!(!game.has_open_borders(1, 0));
    assert_eq!(game.international_tourism_multiplier(0, 1, false), 1.25);
}

#[test]
fn culture_quick_deals_buy_housed_great_works_and_preserve_our_own() {
    let mut game = Game::new_full(2, 20, 12, 79_005, 200, 0, false);
    // A quick deal is offered to a counterparty, so there has to be one.
    game.record_contact(0, 1);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
        let city = game.player_city_ids(pid)[0];
        install_ai_test_district(&mut game, city, "theater_square");
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        game.players[pid].gold = 1_000.0;
    }
    game.current = 0;
    game.players[1]
        .counters
        .insert("great_work:writing".to_string(), 2);
    game.turn = 6;

    AdvancedAi::targeting(VictoryTarget::Culture).strategic_bilateral_trade(
        &mut game,
        0,
        None,
        GrandStrategy::Expansion,
    );
    assert_eq!(game.players[0].counters["great_work:writing"], 1);
    assert_eq!(game.players[1].counters["great_work:writing"], 1);

    let mut preserve = Game::new_full(2, 20, 12, 79_006, 200, 0, false);
    for pid in 0..2 {
        preserve.current = pid;
        let settler = preserve
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| preserve.units[unit].kind == "settler")
            .unwrap();
        preserve
            .apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
        let city = preserve.player_city_ids(pid)[0];
        install_ai_test_district(&mut preserve, city, "theater_square");
        preserve
            .cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        preserve.players[pid].gold = 1_000.0;
    }
    preserve.current = 0;
    preserve.players[0]
        .counters
        .insert("great_work:writing".to_string(), 2);
    preserve.turn = 6;
    AdvancedAi::targeting(VictoryTarget::Culture).strategic_bilateral_trade(
        &mut preserve,
        0,
        None,
        GrandStrategy::Expansion,
    );
    assert_eq!(preserve.players[0].counters["great_work:writing"], 2);
    assert_eq!(
        preserve.players[1]
            .counters
            .get("great_work:writing")
            .copied()
            .unwrap_or(0),
        0
    );
}

/// The census this guards: at the deployment shape the seat parked 36% of
/// its envoys at exactly one per city-state and held 0.3 suzerainties,
/// because the scorer amortises the next type bonus over the envoys needed
/// to reach it -- a fresh city-state sells its level-1 bonus for one envoy
/// while an opened one needs two more before it pays again, so the cheapest
/// step is always to open another.
///
/// Asserted in aggregate over many boards rather than on one. A single
/// board does not isolate the term: alignment reaches `(10 + 14) * 10 =
/// 240` for a unique-civ match, which outranks the whole prize, so any one
/// seed measures the draw as much as the rule. The first version of this
/// test asserted a single board and failed for exactly that reason.
#[test]
fn pricing_the_suzerainty_wins_more_city_states_with_the_same_envoys() {
    let run = |priced: bool| {
        let mut suzerainties = 0usize;
        let mut spent = 0i64;
        for seed in 1..40u64 {
            let mut g = Game::new(2, 24, 16, seed, 80, 4);
            let minors: Vec<usize> = g
                .players
                .iter()
                .filter(|player| player.is_minor && !player.is_barbarian)
                .map(|player| player.id)
                .collect();
            for minor in &minors {
                g.record_contact(0, *minor);
            }
            // Six envoys against four city-states: enough to take two
            // suzerainties outright, or to open every one of them and hold
            // none. Which of those happens is the whole question.
            g.players[0].envoys_free = 6;
            let mut ai = AdvancedAi::new();
            if priced {
                ai.enable_price_the_suzerainty();
            }
            ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
            spent += 6 - g.players[0].envoys_free;
            suzerainties += minors
                .iter()
                .filter(|minor| g.suzerain_of(**minor) == Some(0))
                .count();
        }
        (suzerainties, spent)
    };

    let (stock, stock_spent) = run(false);
    let (priced, priced_spent) = run(true);
    assert_eq!(
        stock_spent, priced_spent,
        "both arms must place the same envoys; this measures allocation, not income"
    );
    assert!(
        priced > stock,
        "pricing the suzerainty must convert the same envoys into more \
             suzerainties: stock {stock}, priced {priced} over 39 boards"
    );
}

/// ★★★★ See `bank_envoys`: the stock pass takes the best-scored placement
/// even when that score is negative, so a seat that has already secured
/// every met city-state past its paying tier keeps pouring envoys into
/// them; the live seat banks them for the next city-state met, and still
/// spends on one that is worth it.
#[test]
fn the_live_seat_banks_an_envoy_it_has_no_use_for() {
    let board = || {
        let mut g = Game::new(2, 24, 16, 4_242, 80, 2);
        let minors: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        assert!(minors.len() >= 2, "the fixture needs two city-states");
        for minor in &minors {
            g.record_contact(0, *minor);
        }
        (g, minors)
    };
    let secure_everything = |g: &mut Game, minors: &[usize]| {
        // Suzerain of every met city-state, past the six-envoy tier, with
        // no rival within reach: nothing left on the board pays.
        g.players[0].envoys = minors.iter().map(|minor| (*minor, 8)).collect();
        g.players[0].envoys_free = 3;
    };

    // `bank_envoys` stays bridge-only. `explore_commit` was bridge-only
    // when this PR added it and was promoted into production later (see
    // `promoted_policy_envoy`); the frozen anchor still never carries it.
    for controller in [AdvancedAi::new(), AdvancedAi::legacy()] {
        assert!(!controller.bank_envoys);
    }
    assert!(AdvancedAi::new().base.explore_commit);
    assert!(!AdvancedAi::legacy().base.explore_commit);

    let (mut stock_game, minors) = board();
    secure_everything(&mut stock_game, &minors);
    AdvancedAi::new().advanced_envoys(&mut stock_game, 0, GrandStrategy::Science, None);
    assert_eq!(
        stock_game.players[0].envoys_free, 0,
        "the stock pass spends every held envoy, worth it or not"
    );

    let (mut live_game, minors) = board();
    secure_everything(&mut live_game, &minors);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(
        live.bank_envoys,
        "the composite carries every controller switch the live bridge deploys"
    );
    live.advanced_envoys(&mut live_game, 0, GrandStrategy::Science, None);
    assert_eq!(
        live_game.players[0].envoys_free, 3,
        "with nothing on the board worth an envoy, the live seat banks all three"
    );
    assert!(
        live_game.players[0]
            .envoys
            .iter()
            .all(|(_, count)| *count == 8),
        "and none of the secure city-states took one: {:?}",
        live_game.players[0].envoys
    );

    // A fresh city-state is worth opening, and the banked envoys go there.
    let (mut fresh_game, minors) = board();
    fresh_game.players[0].envoys = vec![(minors[0], 8)];
    fresh_game.players[0].envoys_free = 3;
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    live.advanced_envoys(&mut fresh_game, 0, GrandStrategy::Science, None);
    assert_eq!(
        fresh_game.envoys_at(0, minors[1]),
        3,
        "the unopened city-state takes the bank: {:?}",
        fresh_game.players[0].envoys
    );
    assert_eq!(fresh_game.players[0].envoys_free, 0);
}

#[test]
fn live_envoy_bank_invests_the_pool_above_its_liquidity_reserve() {
    // The live historical failure was 50–80 Envoys held after every met
    // city-state was safely ours. Preserve a discovery-sized reserve, but
    // make the rest reinforce a real city-state rather than remaining an
    // unbounded zero-yield balance.
    let mut game = Game::new(2, 24, 16, 4_245, 80, 2);
    let minors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert!(minors.len() >= 2, "the fixture needs two city-states");
    for minor in &minors {
        game.record_contact(0, *minor);
    }
    game.players[0].envoys = minors.iter().map(|minor| (*minor, 8)).collect();
    game.players[0].envoys_free = BANKED_ENVOY_LIQUIDITY_RESERVE + 4;

    let mut live = AdvancedAi::new();
    live.enable_bank_envoys();
    assert_eq!(
        live.banked_envoy_liquidity_reserve(&game, 0),
        BANKED_ENVOY_LIQUIDITY_RESERVE,
        "no derived-war city-state needs a larger reclaim reserve"
    );
    live.advanced_envoys(&mut game, 0, GrandStrategy::Science, None);
    assert_eq!(
        game.players[0].envoys_free, BANKED_ENVOY_LIQUIDITY_RESERVE,
        "only the bounded discovery reserve remains idle"
    );
    assert_eq!(
        game.players[0]
            .envoys
            .iter()
            .map(|(_, count)| *count)
            .sum::<i64>(),
        8 * minors.len() as i64 + 4,
        "every Envoy above the reserve was invested in a city-state"
    );
}

#[test]
fn live_envoy_bank_keeps_the_price_of_a_derived_war_reclaim() {
    // Do not cap away the concrete use that the bank already supports:
    // after peace with this hostile Suzerain, nine Envoys immediately
    // reclaim its city-state. That price legitimately exceeds the normal
    // discovery reserve.
    let mut game = Game::new_full(2, 24, 16, 4_246, 120, 1, false);
    let minor = game
        .players
        .iter()
        .find(|player| player.alive && player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("the fixture needs a city-state");
    game.record_contact(0, 1);
    game.record_contact(0, minor);
    game.players[0].envoys.clear();
    game.players[1].envoys = vec![(minor, 8)];
    game.at_war.insert((0, 1));
    assert_eq!(game.suzerain_of(minor), Some(1));
    assert!(game.is_at_war(0, minor));
    assert!(
        !game.at_war.contains(&(0, minor)),
        "the city-state war is derived from the hostile Suzerain"
    );

    let mut live = AdvancedAi::new();
    live.enable_bank_envoys();
    assert_eq!(
        live.banked_envoy_liquidity_reserve(&game, 0),
        9,
        "the concrete reclaim price outranks the ordinary six-Envoy reserve"
    );
}

#[test]
fn live_envoy_bank_spends_past_the_reserve_for_an_active_campaigns_suzerain() {
    // A derived-war reclaim only pays after peace with its Suzerain. When
    // that Suzerain is the active campaign target, peace is deliberately
    // unavailable, so its nine-Envoy price must not become an inert pool.
    let mut game = Game::new_full(2, 24, 16, 4_247, 120, 2, false);
    let minors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.alive && player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert!(minors.len() >= 2, "the fixture needs two city-states");
    let hostile_minor = minors[0];
    let safe_minor = minors[1];
    game.record_contact(0, 1);
    for minor in &minors {
        game.record_contact(0, *minor);
    }
    game.players[0].envoys = vec![(safe_minor, 8)];
    game.players[1].envoys = vec![(hostile_minor, 8)];
    game.players[0].envoys_free = 10;
    game.players[safe_minor].civ = "Cahokia".to_string();
    game.at_war.insert((0, 1));
    assert_eq!(game.suzerain_of(hostile_minor), Some(1));
    assert!(game.is_at_war(0, hostile_minor));

    let mut live = AdvancedAi::new();
    live.enable_bank_envoys();
    live.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    });
    assert_eq!(
        live.banked_envoy_liquidity_reserve(&game, 0),
        BANKED_ENVOY_LIQUIDITY_RESERVE,
        "an active campaign cannot promise the peace required for this reclaim"
    );

    live.advanced_envoys(&mut game, 0, GrandStrategy::Science, None);
    assert_eq!(
        game.players[0].envoys_free, BANKED_ENVOY_LIQUIDITY_RESERVE,
        "Envoys above the real discovery reserve must be invested"
    );
    assert_eq!(
        game.envoys_at(0, safe_minor),
        12,
        "the four formerly unreachable Envoys reinforce a met city-state"
    );
}

#[test]
fn live_envoys_reach_another_city_states_yield_tier_before_padding_a_secure_suzerain() {
    let mut game = Game::new(2, 24, 16, 4_243, 80, 2);
    let city_states: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert!(city_states.len() >= 2, "the fixture needs two city-states");
    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    let nan_madol = city_states[0];
    let chinguetti = city_states[1];
    game.record_contact(0, nan_madol);
    game.record_contact(0, chinguetti);
    game.players[nan_madol].civ = "Nan Madol".to_string();
    game.players[chinguetti].civ = "Chinguetti".to_string();
    // Nan Madol is safely ours and exhausted past its six-envoy yield
    // tier. Chinguetti is one envoy short of its three-envoy yield tier.
    game.players[0].envoys = vec![(nan_madol, 8), (chinguetti, 1)];
    game.players[0].envoys_free = 2;

    let mut live = AdvancedAi::new();
    live.enable_bank_envoys();
    live.advanced_envoys(&mut game, 0, GrandStrategy::Culture, None);

    assert_eq!(
        game.envoys_at(0, nan_madol),
        8,
        "a secure suzerain past its final tier must not absorb the bank"
    );
    assert_eq!(
        game.envoys_at(0, chinguetti),
        3,
        "the available culture-aligned yield tier should receive both envoys"
    );
    assert_eq!(game.players[0].envoys_free, 0);
}

#[test]
fn live_envoys_still_defend_a_one_envoy_suzerainty_margin() {
    let mut game = Game::new(2, 24, 16, 4_244, 80, 2);
    let city_state = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("the fixture needs a city-state");
    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    game.record_contact(0, city_state);
    game.players[city_state].civ = "Nan Madol".to_string();
    game.players[0].envoys = vec![(city_state, 8)];
    game.players[1].envoys = vec![(city_state, 7)];
    game.players[0].envoys_free = 1;

    let mut live = AdvancedAi::new();
    live.enable_bank_envoys();
    live.advanced_envoys(&mut game, 0, GrandStrategy::Culture, None);

    assert_eq!(
        game.envoys_at(0, city_state),
        9,
        "a one-envoy suzerainty lead remains a defense priority"
    );
    assert_eq!(game.players[0].envoys_free, 0);
}

#[test]
fn diplomatic_strategy_concentrates_envoys_into_a_suzerainty() {
    let mut g = Game::new(2, 24, 16, 77, 80, 2);
    let city_states: Vec<usize> = g
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    for city_state in city_states {
        g.record_contact(0, city_state);
    }
    g.players[0].envoys_free = 3;
    AdvancedAi::new().advanced_envoys(&mut g, 0, GrandStrategy::Diplomacy, None);
    assert_eq!(g.players[0].envoys_free, 0);
    assert!(g.players[0].envoys.iter().any(|(_, count)| *count >= 3));
}

#[test]
fn envoy_strategy_prices_the_next_active_building_threshold_per_envoy() {
    let mut game = Game::new_full(1, 28, 18, 7_710, 120, 2, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);
    install_ai_test_district(&mut game, city, "commercial_hub");
    install_ai_test_district(&mut game, city, "harbor");
    install_ai_test_district(&mut game, city, "diplomatic_quarter");
    game.cities.get_mut(&city).unwrap().buildings.extend(
        ["stock_exchange", "seaport", "chancery"]
            .into_iter()
            .map(Name::new),
    );

    let states: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    let hattusa = states[0];
    let zanzibar = states[1];
    for city_state in states.iter().copied() {
        game.record_contact(0, city_state);
    }
    game.players[hattusa].civ = "Hattusa".to_string();
    game.players[zanzibar].civ = "Zanzibar".to_string();
    game.players[0].envoys = vec![(hattusa, 4), (zanzibar, 5)];
    game.players[0].envoys_free = 1;

    let (science_steps, science_gain) = game.next_envoy_type_bonus(0, hattusa).unwrap();
    let (gold_steps, gold_gain) = game.next_envoy_type_bonus(0, zanzibar).unwrap();
    assert_eq!((science_steps, science_gain.science), (2, 3.0));
    assert_eq!((gold_steps, gold_gain.gold), (1, 18.0));

    AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Science, None);
    assert_eq!(game.envoys_at(0, hattusa), 4);
    assert_eq!(game.envoys_at(0, zanzibar), 6);
}

#[test]
fn religious_envoys_prefer_yerevan_but_skip_a_bonus_shared_by_economic_alliance() {
    let mut game = Game::new_full(2, 32, 20, 7_711, 120, 2, false);
    let minors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert_eq!(minors.len(), 2);
    for city_state in minors.iter().copied() {
        game.record_contact(0, city_state);
    }
    // Keep the score test's influence table explicit; contact itself may
    // have awarded the first-discovery Envoy.
    game.players[0].envoys.clear();
    game.players[minors[0]].civ = "Kandy".to_string();
    game.players[minors[1]].civ = "Yerevan".to_string();
    game.players[0].envoys_free = 1;

    AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Religion, None);
    assert_eq!(game.envoys_at(0, minors[0]), 0);
    assert_eq!(game.envoys_at(0, minors[1]), 1);

    game.players[0].envoys.clear();
    game.players[0].envoys_free = 1;
    game.players[1].envoys = vec![(minors[1], 3)];
    let alliance = crate::game::AllianceState {
        kind: "economic".to_string(),
        points: 240.0,
        level: 3,
        ends: game.turn + 30,
    };
    game.players[0].alliances.insert(1, alliance.clone());
    game.players[1].alliances.insert(0, alliance);

    AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Religion, None);
    assert_eq!(game.envoys_at(0, minors[0]), 1);
    assert_eq!(game.envoys_at(0, minors[1]), 0);
}

#[test]
fn strategic_envoys_ignore_an_unmet_city_states_identity() {
    let mut game = Game::new_full(1, 24, 16, 90_733, 120, 2, false);
    let city_states: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert_eq!(city_states.len(), 2);
    let hidden = city_states[0];
    let known = city_states[1];
    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    game.players[hidden].civ = "Yerevan".to_string();
    game.players[known].civ = "Kandy".to_string();
    game.record_contact(0, known);
    // This fixture checks identity gating, not the first-discovery award.
    game.players[0].envoys.clear();
    game.players[0].envoys_free = 1;

    // Yerevan has the higher Religion score. Ranking it before contact
    // would both leak its identity and make apply reject the first send.
    AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Religion, None);

    assert_eq!(game.players[0].envoys_free, 0);
    assert_eq!(game.envoys_at(0, hidden), 0);
    assert_eq!(game.envoys_at(0, known), 1);
}

#[test]
fn advanced_ai_votes_in_special_sessions_and_liberates_emergency_objectives() {
    let mut vote_game = Game::new_full(3, 26, 16, 73_001, 120, 0, false);
    for player in 0..3 {
        let settler = vote_game
            .player_unit_ids(player)
            .into_iter()
            .find(|unit| vote_game.units[unit].kind == "settler")
            .unwrap();
        vote_game.found_city_for(player, vote_game.units[&settler].pos, None);
    }
    let objective = vote_game.player_city_ids(0)[0];
    vote_game.pending_emergencies = vec![crate::game::EmergencyProposal {
        id: 77,
        kind: "city_state".to_string(),
        target: 0,
        city: objective,
        original_owner: 1,
        eligible: [2].into_iter().collect(),
        requested: vote_game.turn,
    }];
    vote_game.congress = Some(crate::game::CongressSession {
        convened: vote_game.turn,
        closes: vote_game.turn + 5,
        resolutions: vec![CongressResolution {
            id: "emergency:77".to_string(),
            title: "City-State Emergency".to_string(),
            choices: vec!["A:support".to_string(), "B:oppose".to_string()],
            ballots: BTreeMap::new(),
        }],
    });
    vote_game.current = 2;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: vote_game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.advanced_diplomacy(&mut vote_game, 2, &plan);
    assert_eq!(
        vote_game.congress.as_ref().unwrap().resolutions[0].ballots[&2].0,
        "A:support"
    );
    assert_eq!(
        ai.congress_choice(
            &vote_game,
            0,
            &vote_game.congress.as_ref().unwrap().resolutions[0],
            GrandStrategy::Conquest,
        ),
        Some("B:oppose".to_string())
    );

    let mut conquest = Game::new_full(3, 26, 16, 73_002, 120, 0, false);
    for player in 0..3 {
        let settler = conquest
            .player_unit_ids(player)
            .into_iter()
            .find(|unit| conquest.units[unit].kind == "settler")
            .unwrap();
        conquest.found_city_for(player, conquest.units[&settler].pos, None);
    }
    let objective = conquest.player_city_ids(1)[0];
    {
        let city = conquest.cities.get_mut(&objective).unwrap();
        city.owner = 0;
        city.captured_from = None;
        city.occupied_from = Some(1);
    }
    conquest.active_emergencies = vec![crate::game::Emergency {
        id: 78,
        kind: "military".to_string(),
        target: 0,
        city: objective,
        original_owner: 1,
        members: [2].into_iter().collect(),
        contributions: BTreeMap::new(),
        started: conquest.turn,
        ends: conquest.turn + 30,
    }];
    conquest.current = 2;
    let emergency_plan = ai.assess(&conquest, 2);
    assert_eq!(emergency_plan.target_player, Some(0));
    assert_eq!(emergency_plan.target_city, Some(objective));
    {
        let city = conquest.cities.get_mut(&objective).unwrap();
        city.owner = 2;
        city.captured_from = Some(0);
        city.occupied_from = Some(0);
    }
    ai.resolve_city_dispositions(&mut conquest, 2, GrandStrategy::Science);
    assert_eq!(conquest.cities[&objective].owner, 1);
    assert!(conquest.active_emergencies.is_empty());
}

#[test]
fn a_spy_posted_to_a_razed_city_does_not_bring_the_server_down() {
    let mut game = Game::new_full(2, 24, 16, 109, 120, 0, false);
    let cities: Vec<u32> = (0..2)
        .map(|pid| {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None)
        })
        .collect();
    let spy = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 1,
            promotions: Default::default(),
            city: Some(cities[0]),
            ready_turn: game.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    // The city the agent is posted to is razed out from under it, which is
    // an ordinary wartime event. Indexing the city map for it panicked the
    // AI thread, and that poisoned the game mutex so every later HTTP
    // request died too: one razed city took the whole exhibition offline.
    game.cities.remove(&cities[0]);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: Some(1),
        target_city: Some(cities[1]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    AdvancedAi::targeting(VictoryTarget::Science).advanced_spies(&mut game, 0, &plan);
    assert!(game.spies.contains_key(&spy), "the agent survives the raze");
}

#[test]
fn science_strategy_uses_an_established_spy_to_steal_a_rival_technology() {
    let mut game = Game::new_full(2, 24, 16, 109, 120, 0, false);
    let cities: Vec<u32> = (0..2)
        .map(|pid| {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None)
        })
        .collect();
    let target = cities[1];
    let campus = game.cities[&target]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&target].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&target)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    install_ai_test_district(&mut game, target, "spaceport");
    game.players[1].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
    ]);
    game.players[1].techs.insert(crate::name!("writing"));
    let spy = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 2,
            promotions: ["technologist".to_string(), "disguise".to_string()]
                .into_iter()
                .collect(),
            city: Some(target),
            ready_turn: game.turn,
            mission: None,
            sources_city: Some(target),
            sources_until: game.turn + 24,
            captured_by: None,
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };

    AdvancedAi::targeting(VictoryTarget::Science).advanced_spies(&mut game, 0, &plan);
    assert_eq!(
        game.spies[&spy]
            .mission
            .as_ref()
            .map(|mission| mission.kind.as_str()),
        Some("steal_tech_boost")
    );
}

#[test]
fn science_strategy_disrupts_a_rival_spaceport_after_mars_launch() {
    let mut game = Game::new_full(2, 24, 16, 109, 120, 0, false);
    let cities: Vec<u32> = (0..2)
        .map(|pid| {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None)
        })
        .collect();
    let target = cities[1];
    install_ai_test_district(&mut game, target, "campus");
    install_ai_test_district(&mut game, target, "spaceport");
    game.players[1]
        .science_projects
        .insert("launch_mars_colony".to_string());
    game.players[1].techs.insert(crate::name!("writing"));

    let spy = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 2,
            promotions: ["technologist".to_string(), "disguise".to_string()]
                .into_iter()
                .collect(),
            city: Some(target),
            ready_turn: game.turn,
            mission: None,
            sources_city: Some(target),
            sources_until: game.turn + 24,
            captured_by: None,
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };

    AdvancedAi::targeting(VictoryTarget::Science).advanced_spies(&mut game, 0, &plan);
    assert_eq!(
        game.spies[&spy]
            .mission
            .as_ref()
            .map(|mission| mission.kind.as_str()),
        Some("disrupt_rocketry")
    );
}

#[test]
fn a_spy_order_in_flight_is_not_re_sent_to_a_rebuilt_board() {
    // The live mirror is rebuilt from the host export every decision, and the
    // export does not carry a running Spy operation — so the seated Spy
    // always reads `mission: None` and the same order went out every turn:
    // run civvis-20260818T155500Z sent SPY_GAIN_SOURCES 35 times over
    // t107–t141. The contract: with `spy_mission_patience` an ordered Spy is
    // left alone for the order's duration, a frozen controller keeps the
    // historical re-order, and after the window expires the seat may ask
    // again.
    let mut game = Game::new_full(2, 24, 16, 109, 120, 0, false);
    let cities: Vec<u32> = (0..2)
        .map(|pid| {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None)
        })
        .collect();
    let target = cities[1];
    let spy = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 0,
            promotions: Default::default(),
            city: Some(target),
            ready_turn: game.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let fresh = game.clone();

    let mut live = AdvancedAi::targeting(VictoryTarget::Science);
    live.enable_spy_mission_patience();
    live.advanced_spies(&mut game, 0, &plan);
    assert!(
        game.spies[&spy].mission.is_some(),
        "the first order goes out"
    );

    let mut rebuilt = fresh.clone();
    live.advanced_spies(&mut rebuilt, 0, &plan);
    assert!(
        rebuilt.spies[&spy].mission.is_none(),
        "an order still running on the host is not re-sent to the rebuilt board"
    );

    let mut expired = fresh.clone();
    expired.turn += expired.standard_duration(SPY_MISSION_ORDER_PATIENCE) + 1;
    if let Some(agent) = expired.spies.get_mut(&spy) {
        agent.ready_turn = expired.turn;
    }
    live.advanced_spies(&mut expired, 0, &plan);
    assert!(
        expired.spies[&spy].mission.is_some(),
        "after the patience window the seat may ask again"
    );

    let mut historical = AdvancedAi::targeting(VictoryTarget::Science);
    let mut first = fresh.clone();
    historical.advanced_spies(&mut first, 0, &plan);
    let mut re_sent = fresh.clone();
    historical.advanced_spies(&mut re_sent, 0, &plan);
    assert!(
        re_sent.spies[&spy].mission.is_some(),
        "frozen controllers keep the historical re-order"
    );

    let mut bridged = AdvancedAi::new();
    bridged.enable_live_bridge();
    assert!(
        bridged.spy_mission_patience,
        "the live bundle carries the repair"
    );
    bridged.disable_spy_mission_patience();
    assert!(!bridged.spy_mission_patience);
}

#[test]
fn one_war_offers_peace_on_the_second_front_and_keeps_the_campaign() {
    let (mut game, plan) = one_war_board();
    assert!(
        game.military_power(0) >= game.military_power(1) * 0.62
            && game.military_power(0) >= game.military_power(2) * 0.62,
        "precondition: neither front trips the ordinary outmatched clause"
    );

    let mut off = AdvancedAi::new();
    let mut off_board = game.clone();
    off.one_war_observe(&off_board, 0);
    off.advanced_diplomacy(&mut off_board, 0, &plan);
    assert!(
        off.peace_offers.is_empty(),
        "with the gene off a balanced two-front war offers nobody peace: {:?}",
        off.peace_offers
    );

    let mut ai = AdvancedAi::new();
    ai.enable_one_war_at_a_time();
    // The plan of the previous turn is what the observation reads.
    ai.plan = Some(plan.clone());
    ai.one_war_observe(&game, 0);
    assert_eq!(
        ai.one_war_front(),
        Some(1),
        "the plan's target is the front"
    );
    assert_eq!(
        ai.one_war_objective_enemies(&game, None, &[1, 2]),
        vec![1],
        "the force planner aims at the front alone"
    );
    ai.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        ai.peace_offers.contains(&2) && !ai.peace_offers.contains(&1),
        "the second front is offered peace and the front is not: {:?}",
        ai.peace_offers
    );
    assert!(
        game.pending_deals
            .iter()
            .any(|deal| deal.from == 0 && deal.to == 2 && deal.peace),
        "the offer is an outbound peace deal"
    );
    assert_eq!(
        game.players[0].counters.get("one_war:peace:second_front"),
        Some(&1)
    );
    assert!(
        ai.incoming_deal_value(&game, 0, &one_war_peace_deal(&game, 2, None), &plan) >= 320.0,
        "the second front's white peace is taken"
    );
    assert!(
        ai.incoming_deal_value(&game, 0, &one_war_peace_deal(&game, 1, None), &plan) < 0.0,
        "the front's white peace is still refused while the tide is not against us"
    );
    assert!(
        ai.incoming_deal_value(&game, 0, &one_war_peace_deal(&game, 2, Some(1)), &plan) <= -1_000.0,
        "a Joint War while a war burns is a second front by treaty"
    );
    // Peace with the second front concluded: the front is unchanged.
    game.at_war.remove(&(0, 2));
    ai.one_war_observe(&game, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    assert_eq!(ai.one_war_objective_enemies(&game, None, &[1]), vec![1]);
}

#[test]
fn one_war_presses_a_breaking_city_and_sues_on_a_rout() {
    let (mut game, plan) = one_war_board();
    game.at_war.remove(&(0, 2));
    let front_city = game.player_city_ids(1)[0];
    let siege_pos = game
        .wdisk(game.cities[&front_city].pos, 2)
        .into_iter()
        .find(|pos| {
            *pos != game.cities[&front_city].pos
                && game.city_at(*pos).is_none()
                && game
                    .map
                    .get(*pos)
                    .is_some_and(|tile| !game.rules.is_water(tile))
        })
        .expect("a land tile beside the front city");
    game.spawn_test_unit("modern_armor", 0, siege_pos);

    // A stalled war by the fatigue clause's own clock.
    let mut ai = AdvancedAi::new();
    ai.enable_one_war_at_a_time();
    ai.major_war_since = Some(20);
    ai.last_campaign_progress = 20;
    let mut off = AdvancedAi::new();
    off.major_war_since = Some(20);
    off.last_campaign_progress = 20;
    assert!(
        off.incoming_deal_value(&game, 0, &one_war_peace_deal(&game, 1, None), &plan) >= 0.0,
        "gene off: the fatigued war accepts the front's white peace"
    );

    ai.one_war_observe(&game, 0);
    game.cities.get_mut(&front_city).unwrap().hp = 140;
    game.turn += 1;
    ai.one_war_observe(&game, 0);
    assert!(
        ai.one_war_prizes_in_reach(&game, 0),
        "a front city whose health fell under our soldier is a prize in reach"
    );
    assert!(ai.one_war_presses(&game, 0, 1));
    assert_eq!(ai.one_war_peace(&game, 0, 1), None);
    assert!(
        ai.incoming_deal_value(&game, 0, &one_war_peace_deal(&game, 1, None), &plan) < 0.0,
        "gene on: the front's white peace is refused while its city is breaking"
    );
    ai.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        !ai.peace_offers.contains(&1),
        "the fatigue clause stands down while the city is breaking"
    );

    // Five of ours lost for none of theirs since the last observation: a rout.
    let key = (0usize, 1usize);
    let mut record = crate::game::WarRecord {
        conflict: 1,
        declarer: 0,
        target: 1,
        casus_belli: None,
        joint_war_until: None,
        aggressor: 0,
        defender: 1,
        started: 20,
        ended: None,
        losses: BTreeMap::new(),
        participants: Vec::new(),
        peace_terms: Vec::new(),
        highlights: Vec::new(),
        theater: Vec::new(),
    };
    record.losses.insert(
        0,
        crate::game::WarLosses {
            units: 5,
            ..Default::default()
        },
    );
    game.wars.insert(key, record);
    game.turn += 1;
    ai.one_war_observe(&game, 0);
    let front = ai.one_war.as_ref().expect("the front persists");
    assert_eq!(front.window_net(), -5);
    assert_eq!(
        ai.one_war_peace(&game, 0, 1),
        Some(super::super::one_war::OneWarPeace::Rout)
    );
    assert!(
        ai.incoming_deal_value(&game, 0, &one_war_peace_deal(&game, 1, None), &plan) >= 320.0,
        "a rout takes the white peace"
    );
    ai.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "a rout offers peace on the front"
    );
    assert_eq!(game.players[0].counters.get("one_war:peace:rout"), Some(&1));
}

/// ★★★★ AN APPOINTED OBJECTIVE IS A PERMANENT EXEMPTION FROM PEACE. The
/// shipped fatigue clause is written `!appointed_objective && fatigued`, so a
/// campaign that never lands keeps the empire at war for the rest of the
/// game. Live King seat `civvis-20260826T112920Z`: 172 of 248 turns at war
/// behind one objective appointed at turn 100 and never taken. See
/// `AdvancedAi::peace_when_war_does_not_pay`.
#[test]
fn a_war_that_takes_nothing_and_cannot_be_paid_for_sues_for_peace() {
    let (mut game, _second, their_capital) = timed_war_fixture(7);
    // The rival needs a second city: white peace is not offered to an empire
    // down to its last one.
    let their_position = game.cities[&their_capital].pos;
    found_nearby_test_city(&mut game, 1, their_position);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    game.turn = 140;
    // The live shape: a treasury that has been empty for fifty turns.
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -11.0;

    // A plan with an APPOINTED objective — the case the shipped clause
    // exempts from peace forever.
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(their_capital),
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };

    // `appointed_objective` reads the war PLAN in its exploit phase, not the
    // strategic plan's target city: that is the exemption under test.
    let appointed = |game: &Game| {
        timed_plan(
            game,
            their_capital,
            crate::name!("bronze_working"),
            crate::name!("swordsman"),
            None,
            None,
            WarPhase::Exploit,
        )
    };

    let mut shipped = AdvancedAi::new();
    shipped.war_plan = Some(appointed(&game));
    shipped.major_war_since = Some(game.turn - 40);
    shipped.last_campaign_progress = game.turn - 40;
    let mut untouched = game.clone();
    shipped.advanced_diplomacy(&mut untouched, 0, &plan);
    assert!(
        !shipped.peace_offers.contains(&1),
        "the shipped desk exempts an appointed objective from the fatigue clause"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_peace_when_war_does_not_pay();
    ai.war_plan = Some(appointed(&game));
    ai.major_war_since = Some(game.turn - 40);
    ai.last_campaign_progress = game.turn - 40;
    let mut treated = game.clone();
    ai.advanced_diplomacy(&mut treated, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "a forty-turn war that has taken nothing out of an empty treasury sues for peace"
    );

    // A campaign that is actually progressing keeps its war.
    let mut moving = AdvancedAi::new();
    moving.enable_peace_when_war_does_not_pay();
    moving.war_plan = Some(appointed(&game));
    moving.major_war_since = Some(game.turn - 40);
    moving.last_campaign_progress = game.turn;
    let mut solvent = game.clone();
    solvent.players[0].gold = 600.0;
    solvent.players[0].gold_per_turn = 12.0;
    moving.advanced_diplomacy(&mut solvent, 0, &plan);
    assert!(
        !moving.peace_offers.contains(&1),
        "a paid-for campaign that is still landing blows is not interrupted"
    );
}

/// ★★★★ THE LANE IS ASSIGNED, THE BALLOT IS SIZED BY THE WEATHER. The
/// Congress vote count reads `plan.strategy`, the posture of the turn the
/// session happens to sit on, not `victory_target`, the lane the seat is
/// actually playing. Live King seat `civvis-20260826T112920Z` was assigned
/// Diplomacy for 248 turns, held that posture on 66 of them, and cast one
/// vote on thirteen of twenty-five ballots while banking Favor it never
/// spent. See `AdvancedAi::lane_votes_its_favor`.
#[test]
fn the_assigned_diplomatic_lane_votes_the_favor_it_is_holding() {
    let mut game = Game::new_full(2, 28, 18, 91_301, 1_000, 0, false);
    game.current = 0;
    game.players[0].diplomatic_favor = 220.0;
    let affordable = game.congress_affordable_votes(0);
    assert!(
        affordable > 1,
        "the fixture must hold enough Favor for the question to mean something: {affordable}"
    );

    // The posture the live seat was in for most of its Congress sessions.
    let mut shipped = AdvancedAi::new();
    shipped.victory_target = Some(VictoryTarget::Diplomacy);
    assert_eq!(
        shipped.congress_ballot_size(&game, 0, GrandStrategy::Recovery),
        1,
        "shipped: a Recovery posture casts one vote however much Favor is banked"
    );
    assert_eq!(
        shipped.congress_ballot_size(&game, 0, GrandStrategy::Diplomacy),
        affordable,
        "shipped: only a Diplomacy posture spends the Favor"
    );

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai.enable_lane_votes_its_favor();
    assert_eq!(
        ai.congress_ballot_size(&game, 0, GrandStrategy::Recovery),
        affordable,
        "the assigned lane votes its Favor whatever this turn's posture is"
    );

    // A seat that was never assigned the lane is untouched: the gene reads
    // the assignment, not the bank balance.
    let mut science = AdvancedAi::new();
    science.victory_target = Some(VictoryTarget::Science);
    science.enable_lane_votes_its_favor();
    assert_eq!(
        science.congress_ballot_size(&game, 0, GrandStrategy::Recovery),
        1,
        "another lane's seat keeps the shipped ballot"
    );

    // Off, it is an exact no-op.
    ai.disable_lane_votes_its_favor();
    assert_eq!(
        ai.congress_ballot_size(&game, 0, GrandStrategy::Recovery),
        1
    );
}

#[test]
fn envoy_dividends_buy_three_without_chasing_the_rivals_delegation() {
    let (mut g, states) = envoy_dividend_board(10);
    g.players[0].envoys = vec![(states[0], 1), (states[1], 6), (states[2], 6)];
    g.players[1].envoys = vec![(states[0], 100)];
    g.players[0].envoys_free = 2;
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs_universe();
    ai.enable_envoy_building_dividends();
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
    assert_eq!(g.envoys_at(0, states[0]), 3);
    assert_eq!(g.envoys_at(0, states[1]), 6);
    assert_eq!(g.players[0].envoys_free, 0);
}

#[test]
fn envoy_dividends_open_libraries_before_an_empty_six_envoy_tier() {
    let (mut g, states) = envoy_dividend_board(10);
    g.players[0].envoys = vec![(states[0], 3), (states[2], 6)];
    g.players[1].envoys = vec![(states[1], 100)];
    g.players[0].envoys_free = 1;
    let mut ai = AdvancedAi::new();
    ai.enable_envoy_building_dividends();
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
    assert_eq!(g.envoys_at(0, states[0]), 3);
    assert_eq!(g.envoys_at(0, states[1]), 1);
}

#[test]
fn envoy_dividends_save_until_a_paying_package_is_affordable() {
    let (mut g, states) = envoy_dividend_board(10);
    g.players[0].envoys = vec![(states[0], 1), (states[1], 6), (states[2], 6)];
    g.players[1].envoys = vec![(states[0], 100)];
    g.players[0].envoys_free = 1;
    let mut ai = AdvancedAi::new();
    ai.enable_envoy_building_dividends();
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
    assert_eq!(g.players[0].envoys_free, 1);
    assert_eq!(g.envoys_at(0, states[0]), 1);
    g.players[0].envoys_free += 1;
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
    assert_eq!(g.envoys_at(0, states[0]), 3);
    assert_eq!(g.players[0].envoys_free, 0);
}

#[test]
fn envoy_dividends_keep_a_paying_six_tier_and_a_close_defense() {
    let (mut g, states) = envoy_dividend_board(10);
    for city in g.player_city_ids(0) {
        g.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(Name::new("research_lab"));
    }
    g.players[0].envoys = states.iter().map(|minor| (*minor, 6)).collect();
    g.players[0].envoys[0].1 = 3;
    g.players[0].envoys_free = 3;
    let mut ai = AdvancedAi::new();
    ai.enable_envoy_building_dividends();
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
    assert_eq!(g.envoys_at(0, states[0]), 6);
    g.players[1].envoys = vec![(states[0], 5)];
    g.players[0].envoys_free = 3;
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Science, None);
    assert_eq!(g.envoys_at(0, states[0]), 7);
    assert_eq!(
        g.players[0].envoys_free, 2,
        "one defensive placement, then save"
    );
}

#[test]
fn live_science_peace_offer_keeps_the_war_and_threats_until_host_acceptance() {
    let mut game = Game::new_full(2, 24, 16, 7_923, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let ours = game.player_city_ids(0)[0];
    let here = game.cities[&ours].pos;
    let theirs = game.cities[&game.player_city_ids(1)[0]].pos;
    game.spawn_test_unit("modern_armor", 0, here);
    game.spawn_test_unit("modern_armor", 1, theirs);
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 1, theirs);
    }
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    game.turn = game.peace_available_at(0, 1).unwrap();
    game.host_observed = Arc::new(BTreeSet::from([here, theirs]));
    assert!(game.military_power(0) < game.military_power(1) * 0.85);
    assert!(game.military_power(0) >= game.military_power(1) * 0.62);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: Some(ours),
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.major_war_since = Some(60);
    let prior_peace_until = science.peace_until;
    science.plan = Some(plan.clone());
    science.advanced_diplomacy(&mut game, 0, &plan);
    assert_eq!(science.plan_report().unwrap().peace_offers, vec![1]);
    assert!(
        science.peace_offers.contains(&1),
        "the bridge must still send the offer"
    );
    assert!(
        game.is_at_war(0, 1),
        "a proposed peace must not disable tactical threats"
    );
    assert_eq!(science.major_war_since, Some(60));
    assert_eq!(science.peace_until, prior_peace_until);
    assert!(!science.base.enemy_attack_envelopes(&game, 0).is_empty());
    // A refusal leaves the same war in the next host frame; a repeated plan
    // must continue to defend it rather than carry a speculative peace clock.
    science.advanced_diplomacy(&mut game, 0, &plan);
    assert!(game.is_at_war(0, 1));
    assert!(science.peace_offers.contains(&1));
    // Only the authoritative accepted state closes this mirrored war.
    game.apply(0, &Action::MakePeace { player: 1 }).unwrap();
    science.advanced_diplomacy(&mut game, 0, &plan);
    assert!(!game.is_at_war(0, 1));
    assert!(!science.peace_offers.contains(&1));
}

/// See `AdvancedAi::denial_reaches_far`: an urgent rival 25 tiles away is in
/// reach for an overwhelming Domination army under `denial-nearest-finish`,
/// and not otherwise.
#[test]
fn an_urgent_far_rival_is_in_reach_only_for_an_overwhelming_denial_army() {
    let mut g = Game::new_full(2, 60, 24, 936_211, 500, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    let home = g.found_city_for(0, (5, 12), None);
    let far = g.found_city_for(1, (30, 12), None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[1].dvp = 19;
    g.turn = 150;
    let objective = g.cities[&far].pos;
    let distance = g.wdist(g.cities[&home].pos, objective);
    assert!(
        distance > 18 && distance <= DENIAL_FAR_REACH_TILES,
        "{distance}"
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(
        ai.urgent_victory_threat(&g, 1),
        "a rival one vote from Diplomacy is urgent"
    );
    let at = g.cities[&home].pos;
    g.spawn_test_unit("warrior", 1, objective);
    for _ in 0..6 {
        g.spawn_test_unit("swordsman", 0, at);
    }
    assert!(
        !ai.denial_reaches_far(&g, 0, 1, objective),
        "the gene is off"
    );
    ai.enable_denial_nearest_finish();
    assert!(ai.denial_reaches_far(&g, 0, 1, objective));
    for _ in 0..8 {
        g.spawn_test_unit("swordsman", 1, objective);
    }
    assert!(
        !ai.denial_reaches_far(&g, 0, 1, objective),
        "a contested army keeps the ordinary range"
    );
}
