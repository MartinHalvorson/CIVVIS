use super::*;

fn game_with_contacts(players: usize, seed: u64) -> Game {
    let mut game = Game::new_full(players, 26, 16, seed, 300, 0, false);
    for pid in 0..players {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a Settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for first in 0..players {
        for second in first + 1..players {
            game.record_contact(first, second);
        }
    }
    game
}

fn install_alliance(game: &mut Game, first: usize, second: usize) {
    let alliance = AllianceState {
        kind: "military".to_string(),
        points: 0.0,
        level: 1,
        ends: game.turn + 60,
    };
    game.players[first]
        .alliances
        .insert(second, alliance.clone());
    game.players[second].alliances.insert(first, alliance);
}

#[test]
fn casus_belli_profiles_cover_every_published_grievance_multiplier() {
    // The Civilopedia states declaration, capture, and raze penalties as
    // percentages of the Formal-War capture penalty. The engine stores
    // razing against its own 3x baseline, hence Liberation's 600% is 2x.
    for (name, declaration, capture, raze) in [
        ("surprise_war", 1.5, 1.5, 1.5),
        ("formal_war", 1.0, 1.0, 1.0),
        ("holy_war", 0.5, 0.5, 0.5),
        ("joint_war", 1.0, 1.0, 1.0),
        ("reconquest_war", 0.0, 1.0, 1.0),
        ("protectorate_war", 0.0, 1.0, 1.0),
        ("liberation_war", 0.0, 1.0, 2.0),
        ("colonial_war", 0.5, 0.5, 1.0),
        ("territorial_war", 0.75, 0.75, 0.5),
        ("golden_age_war", 0.25, 0.25, 1.0),
        ("retribution_war", 0.5, 0.5, 2.0 / 3.0),
        ("ideological_war", 0.5, 0.5, 0.5),
    ] {
        let profile = casus_belli_profile(name).expect("named casus exists");
        assert_eq!(profile.id, name);
        assert!((profile.declaration_multiplier - declaration).abs() < 1e-9, "{name}");
        assert!((profile.capture_multiplier - capture).abs() < 1e-9, "{name}");
        assert!((profile.raze_multiplier - raze).abs() < 1e-9, "{name}");
    }
}

#[test]
fn delegations_and_embassies_are_paid_directional_and_non_stacking() {
    let mut game = game_with_contacts(2, 91_101);
    game.players[0].gold = 100.0;
    game.players[1].gold = 0.0;
    let visibility_before = game.diplomatic_visibility(0, 1);
    let recipient_opinion_before = game.relationship_opinion(1, 0);
    let sender_opinion_before = game.relationship_opinion(0, 1);

    game.current = 0;
    game.apply(0, &Action::SendDelegation { player: 1 })
        .expect("a met major accepts a paid delegation");
    assert_eq!(game.players[0].gold, 90.0);
    assert_eq!(game.players[1].gold, 10.0);
    assert_eq!(
        game.diplomatic_mission_to(0, 1).map(|mission| mission.kind.as_str()),
        Some("delegation")
    );
    assert_eq!(game.diplomatic_visibility(0, 1), visibility_before + 1.0);
    assert_eq!(game.relationship_opinion(1, 0), recipient_opinion_before + 5.0);
    assert_eq!(game.relationship_opinion(0, 1), sender_opinion_before);

    game.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    game.apply(0, &Action::SendEmbassy { player: 1 })
        .expect("Diplomatic Service upgrades the mission");
    assert_eq!(game.players[0].gold, 65.0);
    assert_eq!(game.players[1].gold, 35.0);
    assert_eq!(
        game.diplomatic_mission_to(0, 1).map(|mission| mission.kind.as_str()),
        Some("embassy")
    );
    assert_eq!(
        game.diplomatic_visibility(0, 1),
        visibility_before + 1.0,
        "an Embassy replaces, rather than stacks with, a Delegation"
    );
    assert!(game.apply(0, &Action::SendDelegation { player: 1 }).is_err());

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.players[0].diplomatic_missions, game.players[0].diplomatic_missions);
}

#[test]
fn grievance_spillover_is_pairwise_and_never_cascades() {
    let mut game = game_with_contacts(4, 91_102);
    install_alliance(&mut game, 1, 2);
    for (first, second) in [(1, 3), (2, 3)] {
        game.players[first]
            .friends_until
            .insert(second, game.turn + 30);
        game.players[second]
            .friends_until
            .insert(first, game.turn + 30);
    }

    game.add_grievances(1, 0, 100.0);
    assert_eq!(game.players[1].grievances.get(&0), Some(&100.0));
    assert_eq!(game.players[2].grievances.get(&0), Some(&50.0));
    assert_eq!(game.players[3].grievances.get(&0), Some(&25.0));
    assert_eq!(
        game.players[3].grievances.get(&0),
        Some(&25.0),
        "a friend of the ally must not receive a second propagated share"
    );
}

#[test]
fn grievances_offset_to_one_signed_balance_before_reversing() {
    let mut game = game_with_contacts(2, 911_021);

    game.add_grievances(1, 0, 100.0);
    game.add_grievances(0, 1, 40.0);
    assert_eq!(game.players[1].grievances.get(&0), Some(&60.0));
    assert_eq!(game.players[0].grievances.get(&1), None);

    game.add_grievances(0, 1, 80.0);
    assert_eq!(game.players[1].grievances.get(&0), None);
    assert_eq!(game.players[0].grievances.get(&1), Some(&20.0));
}

#[test]
fn city_state_declaration_charges_its_suzerain_and_other_envoys() {
    let mut game = game_with_contacts(3, 911_022);
    let city_state = game.players.len();
    game.players.push(Player::new(city_state, "Geneva", true));
    game.record_contact(0, city_state);
    game.players[1].envoys.push((city_state, 3));
    game.players[2].envoys.push((city_state, 1));
    assert_eq!(game.suzerain_of(city_state), Some(1));

    game
        .do_declare_war(0, city_state)
        .expect("a met city-state can be declared on");

    assert_eq!(
        game.players[1].grievances.get(&0),
        Some(&CITY_STATE_SUZERAIN_WAR_GRIEVANCES)
    );
    assert_eq!(
        game.players[2].grievances.get(&0),
        Some(&CITY_STATE_ENVOY_WAR_GRIEVANCES)
    );
}

#[test]
fn city_capture_is_population_scaled_then_charged_again_when_ceded() {
    let mut game = game_with_contacts(2, 911_023);
    let target_position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .expect("the compact test map has room for one more city");
    let target = game.found_city_for(1, target_position, Some("Border Town".to_string()));
    let attacker_capital = game.player_city_ids(0)[0];
    let defender_capital = game.player_city_ids(1)[0];
    game.cities.get_mut(&attacker_capital).unwrap().pop = 12;
    game.cities.get_mut(&defender_capital).unwrap().pop = 6;
    game.cities.get_mut(&target).unwrap().pop = 4;

    game.do_declare_war(0, 1).unwrap();
    let declaration = game.players[1].grievances[&0];
    game.capture_city(target, 0);
    game.do_keep_city(0, target).unwrap();
    let capture = game.cities[&target]
        .occupation_grievance
        .expect("a kept city remembers its exact occupation charge");
    assert!(
        capture > 0.0 && capture < CITY_CAPTURE_GRIEVANCES * 1.5,
        "a below-average city stays below the Surprise-War capture cap"
    );
    assert!((game.players[1].grievances[&0] - declaration - capture).abs() < 1e-9);

    game.turn += game.standard_duration(WAR_MIN_TURNS);
    game.do_make_peace(0, 1).unwrap();

    assert!(
        (game.players[1].grievances[&0] - declaration - 2.0 * capture).abs() < 1e-9,
        "peace recognition repeats the city capture grievance"
    );
    assert_eq!(game.cities[&target].occupied_from, None);
    assert_eq!(game.cities[&target].occupation_grievance, None);
}

#[test]
fn returning_an_occupied_city_relieves_its_former_owner_only() {
    let mut game = game_with_contacts(3, 9_110_231);
    let target_position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .expect("the compact test map has room for one more city");
    let target = game.found_city_for(1, target_position, Some("Returned Town".to_string()));

    game.capture_city(target, 0);
    game.do_keep_city(0, target).unwrap();
    let charge = game.cities[&target]
        .occupation_grievance
        .expect("a kept city remembers its capture grievance");
    game.players[2].grievances.insert(0, charge + 30.0);

    game.transfer_city_items(
        0,
        1,
        &DealItems {
            cities: vec![target],
            ..DealItems::default()
        },
    );

    assert_eq!(game.cities[&target].owner, 1);
    assert_eq!(game.players[1].grievances.get(&0), None);
    assert!(
        (game.players[2].grievances[&0] - charge - 30.0).abs() < 1e-9,
        "return is bilateral; global goodwill belongs to liberation"
    );
    assert_eq!(game.cities[&target].occupation_grievance, None);
}

#[test]
fn liberation_relieves_a_city_value_without_erasing_unrelated_grievances() {
    let mut game = game_with_contacts(4, 911_024);
    let target_position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .expect("the compact test map has room for one more city");
    let target = game.found_city_for(1, target_position, Some("Liberation".to_string()));
    game.cities.get_mut(&target).unwrap().pop = 4;

    game.capture_city(target, 2);
    game.do_keep_city(2, target).unwrap();
    game.capture_city(target, 0);
    let relief = game.city_capture_base_grievances(target);
    game.players[1].grievances.insert(0, relief + 30.0);
    game.players[3].grievances.insert(0, relief + 30.0);

    game.do_liberate_city(0, target).unwrap();

    for observer in [1, 3] {
        assert!(
            (game.players[observer].grievances[&0] - 30.0).abs() < 1e-9,
            "liberation removes one city value for observer {observer}"
        );
    }
}

#[test]
fn an_alliance_never_auto_joins_but_a_defensive_pact_joins_once() {
    let mut game = game_with_contacts(4, 91_103);
    install_alliance(&mut game, 1, 2);
    install_alliance(&mut game, 2, 3);

    let mut alliance_only = game.clone();
    alliance_only
        .do_declare_war(0, 1)
        .expect("the principal declaration is legal");
    assert!(
        !alliance_only.is_at_war(0, 2),
        "an Alliance itself is not an invisible Defensive Pact"
    );

    for (first, second) in [(1, 2), (2, 3)] {
        game.players[first]
            .defensive_pacts
            .insert(second, game.turn + 30);
        game.players[second]
            .defensive_pacts
            .insert(first, game.turn + 30);
    }
    game.do_declare_war(0, 1).unwrap();
    assert!(game.is_at_war(0, 2), "the named pact responds");
    assert!(
        !game.is_at_war(0, 3),
        "a defender's Defensive Pact does not chain into a second pact"
    );
}

#[test]
fn joint_war_records_both_signatories_and_enforces_its_thirty_turn_term() {
    let mut game = game_with_contacts(3, 91_104);
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    game.players[1].civics.insert(crate::name!("foreign_trade"));
    game.current = 0;
    game.apply(
        0,
        &Action::ProposeJointWar {
            player: 1,
            target: 2,
        },
    )
    .unwrap();
    let offer = game.pending_deals.last().unwrap().id;

    game.current = 1;
    game.apply(1, &Action::AcceptDeal { deal: offer }).unwrap();
    let until = game.turn + game.standard_duration(STANDARD_DEAL_TURNS);
    for front in [pair(0, 2), pair(1, 2)] {
        let war = &game.wars[&front];
        assert_eq!(war.casus_belli.as_deref(), Some("joint_war"));
        assert_eq!(war.joint_war_until, Some(until));
    }
    assert!(game.do_make_peace(0, 2).is_err());
    game.turn = until - 1;
    assert!(game.do_make_peace(0, 2).is_err());
    game.turn = until;
    game.do_make_peace(0, 2).unwrap();

    let observed = crate::obs::observation(&game, 1);
    assert_eq!(observed["wars"][0]["casus_belli"], "joint_war");
    assert_eq!(observed["wars"][0]["joint_war_until"], until);
}

#[test]
fn city_state_promises_are_scoped_to_the_requesters_suzerainties() {
    let mut game = game_with_contacts(3, 911_041);
    let protected = game.players.len();
    game.players.push(Player::new(protected, "Geneva", true));
    let unrelated = game.players.len();
    game.players.push(Player::new(unrelated, "Kabul", true));
    game.players[0].envoys.push((protected, 3));
    assert_eq!(game.suzerain_of(protected), Some(0));
    game.players[1]
        .promises
        .entry(0)
        .or_default()
        .insert("no_city_state_attack".to_string(), game.turn + 30);

    game.break_promises_on_city_state_attack(1, unrelated);
    assert!(game.promise_active(1, 0, "no_city_state_attack"));
    assert!(!game.promise_request_incident_exists(0, 1, "no_city_state_attack"));

    game.break_promises_on_city_state_attack(1, protected);
    assert!(!game.promise_active(1, 0, "no_city_state_attack"));
    assert!(game.promise_request_incident_exists(0, 1, "no_city_state_attack"));
}

#[test]
fn promises_and_demands_feed_the_retribution_and_grievance_ledgers() {
    let mut game = game_with_contacts(2, 91_105);
    game.current = 0;
    assert!(game
        .apply(
            0,
            &Action::RequestPromise {
                player: 1,
                promise: "no_spying".to_string(),
            },
        )
        .is_err());
    // Discuss promises appear only after the requested leader performed
    // the matching action. Record the same directional incident that a
    // real Spy mission would have created.
    game.record_diplomatic_incident(0, 1, "no_spying");
    game.apply(
        0,
        &Action::RequestPromise {
            player: 1,
            promise: "no_spying".to_string(),
        },
    )
    .unwrap();
    let promise_offer = game.pending_deals.last().unwrap().id;
    game.current = 1;
    game.apply(1, &Action::AcceptDeal { deal: promise_offer })
        .unwrap();
    assert!(game.promise_active(1, 0, "no_spying"));

    assert!(game.break_promise(1, 0, "no_spying"));
    assert_eq!(
        game.players[0].grievances.get(&1),
        Some(&PROMISE_BROKEN_FIRST_GRIEVANCES)
    );
    game.turn += game.standard_duration(STANDARD_DEAL_TURNS);
    game.current = 0;
    game.apply(
        0,
        &Action::RequestPromise {
            player: 1,
            promise: "no_spying".to_string(),
        },
    )
    .unwrap();
    let repeat_promise_offer = game.pending_deals.last().unwrap().id;
    game.current = 1;
    game.apply(1, &Action::AcceptDeal { deal: repeat_promise_offer })
        .unwrap();
    assert!(game.break_promise(1, 0, "no_spying"));
    assert_eq!(
        game.players[0].grievances.get(&1),
        Some(&(PROMISE_BROKEN_FIRST_GRIEVANCES + PROMISE_BROKEN_FIRST_GRIEVANCES + PROMISE_BROKEN_REPEAT_GRIEVANCES)),
        "the second broken promise costs 125 after the initial 100"
    );
    game.players[0].civics.insert(crate::name!("early_empire"));
    game.current = 0;
    game.apply(0, &Action::Denounce { player: 1 }).unwrap();
    game.turn += game.standard_duration(5);
    assert!(game.casus_belli_available(0, 1, "retribution_war"));

    let mut demand = game_with_contacts(2, 91_106);
    demand.players[1].gold = 20.0;
    demand.current = 0;
    assert!(demand
        .apply(
            0,
            &Action::DemandGold {
                player: 1,
                gold: 20.0,
            },
        )
        .is_err());
    demand.apply(0, &Action::Denounce { player: 1 }).unwrap();
    assert!(demand
        .apply(
            0,
            &Action::DemandGold {
                player: 1,
                gold: 21.0,
            },
        )
        .is_err());
    demand
        .apply(
            0,
            &Action::DemandGold {
                player: 1,
                gold: 20.0,
            },
        )
        .unwrap();
    let demand_offer = demand.pending_deals.last().unwrap().id;
    demand.current = 1;
    demand
        .apply(1, &Action::RejectDeal { deal: demand_offer })
        .unwrap();
    assert_eq!(
        demand.players[0].grievances.get(&1),
        None,
        "the first 25-point refusal offsets the denounced leader's 25-point balance"
    );
    demand.current = 0;
    demand
        .apply(
            0,
            &Action::DemandGold {
                player: 1,
                gold: 20.0,
            },
        )
        .unwrap();
    let repeat_demand_offer = demand.pending_deals.last().unwrap().id;
    demand.current = 1;
    demand
        .apply(1, &Action::RejectDeal { deal: repeat_demand_offer })
        .unwrap();
    assert_eq!(
        demand.players[0].grievances.get(&1),
        Some(&(REQUEST_REFUSAL_FIRST_GRIEVANCES + REQUEST_REFUSAL_REPEAT_GRIEVANCES)),
        "the second refusal is the escalating 50-point event after the first offset"
    );

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(
        restored.players[0].diplomatic_incidents,
        game.players[0].diplomatic_incidents,
        "the conduct that unlocks a future promise survives save/load"
    );
}
