use super::*;

fn game_with_spy_cities(seed: u64) -> (Game, u32, u32, Pos) {
    let mut game = Game::new_full(2, 24, 16, seed, 250, 0, false);
    // Nobody spies on, or ransoms an operative back from, an empire they
    // have never met.
    game.record_contact(0, 1);
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        cities.push(game.found_city_for(pid, game.units[&settler].pos, None));
    }
    let target = cities[1];
    let district = game.cities[&target]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&target].pos)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&district).unwrap();
        tile.district = Some(crate::name!("commercial_hub"));
        tile.feature = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    game.cities
        .get_mut(&target)
        .unwrap()
        .districts
        .insert(crate::name!("commercial_hub"), district);
    game.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("market"));
    game.players[0].explored.insert(game.cities[&target].pos);
    (game, cities[0], target, district)
}

#[test]
fn spies_train_to_capacity_assign_run_sources_and_survive_saves() {
    let (mut game, home, target, district) = game_with_spy_cities(774_260);
    game.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    let item = Item::Unit {
        unit: crate::name!("spy"),
    };
    assert!(game.can_produce(0, home, &item));
    assert!(game.complete_item(0, home, &item));
    let spy = *game.spies.keys().next().unwrap();
    assert!(!game.units.values().any(|unit| unit.kind == "spy"));
    assert!(!game.can_produce(0, home, &item), "capacity one is full");

    let assignment = Action::AssignSpy { spy, city: target };
    let legal = game.legal_spy_actions(0, spy);
    assert!(
        legal.contains(&assignment),
        "home={home} target={target} target_owner={} alive={} alliance={:?} spy_city={:?}; legal={legal:?}",
        game.cities[&target].owner,
        game.players[game.cities[&target].owner].alive,
        game.alliance_with(0, game.cities[&target].owner),
        game.spies[&spy].city
    );
    game.apply(0, &assignment).unwrap();
    game.turn = game.spies[&spy].ready_turn;
    assert!(game
        .legal_spy_actions(0, spy)
        .contains(&Action::SpyMission {
            spy,
            mission: "siphon_funds".to_string(),
            target: district,
        }));
    game.apply(
        0,
        &Action::SpyMission {
            spy,
            mission: "gain_sources".to_string(),
            target: game.cities[&target].pos,
        },
    )
    .unwrap();
    let ends = game.spies[&spy].mission.as_ref().unwrap().ends;
    assert_eq!(ends, game.turn + 8);

    let mut restored: Game =
        serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.spies[&spy].mission, game.spies[&spy].mission);
    restored.turn = ends;
    restored.process_spies(0);
    assert_eq!(restored.spies[&spy].sources_city, Some(target));
    assert_eq!(restored.spies[&spy].sources_until, ends + 24);
}

#[test]
fn counterspies_reduce_operation_odds_and_successful_siphons_transfer_gold() {
    let (mut game, home, target, district) = game_with_spy_cities(774_261);
    let attacker = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        attacker,
        Spy {
            id: attacker,
            owner: 0,
            level: 1,
            promotions: ["con_artist".to_string()].into_iter().collect(),
            city: Some(target),
            ready_turn: game.turn,
            mission: None,
            sources_city: Some(target),
            sources_until: game.turn + 24,
            captured_by: None,
        },
    );
    let defender = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        defender,
        Spy {
            id: defender,
            owner: 1,
            level: 2,
            promotions: ["seduction".to_string()].into_iter().collect(),
            city: Some(target),
            ready_turn: game.turn,
            mission: Some(SpyMission {
                kind: "counterspy".to_string(),
                city: target,
                target: district,
                started: game.turn,
                ends: game.turn + 16,
            }),
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    let mission = SpyMission {
        kind: "siphon_funds".to_string(),
        city: target,
        target: district,
        started: game.turn,
        ends: game.turn + 8,
    };
    let defended = game.spy_success_chance(attacker, &mission);
    let mut undefended = game.clone();
    undefended.spies.remove(&defender);
    assert!(undefended.spy_success_chance(attacker, &mission) > defended);

    game.spies.remove(&defender);
    game.players[1].gold = 200.0;
    let attacker_gold = game.players[0].gold;
    game.apply_spy_mission_effect(attacker, &mission, true);
    assert!(game.players[0].gold > attacker_gold);
    assert!(game.players[1].gold < 200.0);
    assert_eq!(game.spies[&attacker].city, Some(target));
    assert_eq!(game.cities[&home].owner, 0);
}

fn install_spy_district(game: &mut Game, city: u32, district: &str) -> Pos {
    let center = game.cities[&city].pos;
    let position = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| {
            *position != center
                && game.map.get(*position).is_some()
                && game.map.tiles[position].district.is_none()
        })
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.district = Some(Name::new(district));
        tile.feature = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    let city = game.cities.get_mut(&city).unwrap();
    if !city.owned_tiles.contains(&position) {
        city.owned_tiles.push(position);
    }
    city.districts.insert(Name::new(district), position);
    position
}

#[test]
fn spy_production_purchase_maintenance_and_promotion_rules_execute() {
    let (mut game, home, _, _) = game_with_spy_cities(774_262);
    assert_eq!(Game::SPY_PROMOTIONS.len(), 17);
    assert_eq!(
        Game::SPY_PROMOTIONS
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        17
    );
    game.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    let item = Item::Unit {
        unit: crate::name!("spy"),
    };
    let base_multiplier = game.item_prod_mult(0, home, Some(&item));
    game.players[0]
        .policies
        .insert(crate::name!("machiavellianism"));
    assert_eq!(
        game.item_prod_mult(0, home, Some(&item)),
        base_multiplier + 0.5
    );
    assert!(game.do_buy(0, home, "spy", "gold").is_err());
    let upkeep_before = game.unit_gold_maintenance(0);
    assert!(game.complete_item(0, home, &item));
    let spy_id = *game.spies.keys().next().unwrap();
    assert_eq!(game.unit_gold_maintenance(0), upkeep_before + 4.0);

    game.spies.get_mut(&spy_id).unwrap().level = 1;
    let first_offer = game.available_spy_promotions(spy_id);
    assert_eq!(first_offer.len(), 3);
    assert!(game.do_promote_spy(0, spy_id, &first_offer[0]).is_ok());
    game.spies.get_mut(&spy_id).unwrap().level = 2;
    game.players[0]
        .policies
        .insert(crate::name!("future_counter_science"));
    assert_eq!(game.available_spy_promotions(spy_id).len(), 16);
}

#[test]
fn every_targeted_spy_operation_has_an_executable_effect() {
    let (mut game, _, target, commercial) = game_with_spy_cities(774_263);
    game.turn = 20;
    game.world_era = 6;
    let campus = install_spy_district(&mut game, target, "campus");
    let theater = install_spy_district(&mut game, target, "theater_square");
    let industrial = install_spy_district(&mut game, target, "industrial_zone");
    let neighborhood = install_spy_district(&mut game, target, "neighborhood");
    let spaceport = install_spy_district(&mut game, target, "spaceport");
    let dam = install_spy_district(&mut game, target, "dam");
    let floodplain = game
        .wdisk(game.cities[&target].pos, 2)
        .into_iter()
        .find(|position| {
            game.map.get(*position).is_some()
                && !matches!(*position, p if p == game.cities[&target].pos)
                && !game.cities[&target]
                    .districts
                    .iter()
                    .any(|(_, p)| *p == *position)
        })
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&floodplain).unwrap();
        tile.owner_city = Some(target);
        tile.feature = Some(crate::name!("floodplains"));
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    game.cities
        .get_mut(&target)
        .unwrap()
        .owned_tiles
        .push(floodplain);
    game.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .extend([crate::name!("amphitheater"), crate::name!("workshop")]);
    game.players[1]
        .counters
        .insert("great_work:writing".to_string(), 1);
    game.players[1].techs.insert(crate::name!("writing"));
    game.players[1].gold = 500.0;
    game.players[1].governors.push(target);
    game.players[1].governor_roster.insert(
        "pingala".to_string(),
        GovernorState {
            city: Some(target),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: BTreeSet::new(),
        },
    );
    let barbarian = game.players.len();
    let mut barb = Player::new(barbarian, "Barbarians", true);
    barb.is_barbarian = true;
    game.players.push(barb);
    game.barb_pid = Some(barbarian);
    let spy_id = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy_id,
        Spy {
            id: spy_id,
            owner: 0,
            level: 1,
            promotions: BTreeSet::new(),
            city: Some(target),
            ready_turn: game.turn,
            mission: None,
            sources_city: Some(target),
            sources_until: game.turn + 24,
            captured_by: None,
        },
    );
    let legal: BTreeSet<String> = game
        .spy_operation_actions(&game.spies[&spy_id], &game.cities[&target])
        .into_iter()
        .filter_map(|action| match action {
            Action::SpyMission { mission, .. } => Some(mission),
            _ => None,
        })
        .collect();
    for mission in [
        "listening_post",
        "siphon_funds",
        "steal_tech_boost",
        "great_work_heist",
        "sabotage_production",
        "recruit_partisans",
        "foment_unrest",
        "neutralize_governor",
        "disrupt_rocketry",
        "breach_dam",
    ] {
        assert!(legal.contains(mission), "missing mission {mission}");
    }
    let mission_turn = game.turn;
    let target_center = game.cities[&target].pos;
    let mission = move |kind: &str, target_position: Pos| SpyMission {
        kind: kind.to_string(),
        city: target,
        target: target_position,
        started: mission_turn,
        ends: mission_turn + 8,
    };

    let attacker_gold = game.players[0].gold;
    game.apply_spy_mission_effect(spy_id, &mission("siphon_funds", commercial), true);
    assert!(game.players[0].gold > attacker_gold);
    game.apply_spy_mission_effect(spy_id, &mission("steal_tech_boost", campus), true);
    assert!(game.players[0].boosted_techs.contains(&crate::name!("writing")));
    game.apply_spy_mission_effect(spy_id, &mission("great_work_heist", theater), true);
    assert_eq!(game.players[0].counters["great_work:writing"], 1);
    game.apply_spy_mission_effect(spy_id, &mission("sabotage_production", industrial), true);
    assert!(game.cities[&target].pillaged_buildings.contains(&Name::new("workshop")));
    let partisans_before = game
        .units
        .values()
        .filter(|unit| unit.owner == barbarian)
        .count();
    game.apply_spy_mission_effect(spy_id, &mission("recruit_partisans", neighborhood), true);
    assert!(
        game.units
            .values()
            .filter(|unit| unit.owner == barbarian)
            .count()
            > partisans_before
    );
    game.apply_spy_mission_effect(spy_id, &mission("foment_unrest", target_center), true);
    // Shipped -15 base, -5 per Spy level, off a full-Loyalty city.
    assert_eq!(game.cities[&target].loyalty, 80.0);
    game.apply_spy_mission_effect(spy_id, &mission("neutralize_governor", target_center), true);
    // Shipped ESPIONAGE_NEUTRALIZE_GOVERNOR_BASE_TURNS is a flat 6; unlike
    // Foment Unrest and Fabricate Scandal it ships no per-level row.
    assert_eq!(
        game.players[1].governor_roster["pingala"].disabled_until,
        game.turn + game.standard_duration(6)
    );
    game.cities.get_mut(&target).unwrap().queue = vec![Item::Project {
        project: crate::name!("launch_earth_satellite"),
    }];
    game.cities.get_mut(&target).unwrap().production = 100.0;
    game.apply_spy_mission_effect(spy_id, &mission("disrupt_rocketry", spaceport), true);
    assert!(game.map.tiles[&spaceport].pillaged);
    assert_eq!(game.cities[&target].production, 0.0);
    game.apply_spy_mission_effect(spy_id, &mission("breach_dam", dam), true);
    assert!(game.map.tiles[&dam].pillaged);
    assert!(game.map.tiles[&floodplain].pillaged);
}

#[test]
fn listening_posts_and_city_state_scandals_change_diplomatic_information() {
    let mut game = Game::new_full(2, 24, 16, 774_264, 250, 1, false);
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
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    let spy_id = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy_id,
        Spy {
            id: spy_id,
            owner: 0,
            level: 2,
            promotions: BTreeSet::new(),
            city: Some(cities[1]),
            ready_turn: game.turn,
            mission: Some(SpyMission {
                kind: "listening_post".to_string(),
                city: cities[1],
                target: game.cities[&cities[1]].pos,
                started: game.turn,
                ends: game.turn + 8,
            }),
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    assert_eq!(game.diplomatic_visibility(0, 1), 2.0);

    game.players[0].envoys.push((minor, 3));
    game.players[1].envoys.push((minor, 7));
    let city_state = game.player_city_ids(minor)[0];
    let scandal = SpyMission {
        kind: "fabricate_scandal".to_string(),
        city: city_state,
        target: game.cities[&city_state].pos,
        started: game.turn,
        ends: game.turn + 16,
    };
    game.spies.get_mut(&spy_id).unwrap().city = Some(city_state);
    game.spies.get_mut(&spy_id).unwrap().mission = None;
    game.apply_spy_mission_effect(spy_id, &scandal, true);
    // Shipped ESPIONAGE_FABRICATE_SCANDAL_BASE_ENVOYS_REMOVED 2 plus
    // _LEVEL_ENVOYS_REMOVED 1 per Spy level, taken off the rival holding
    // the most Envoys there and nobody else.
    let removed = 2 + game.spies[&spy_id].level.max(0);
    assert_eq!(game.envoys_at(1, minor), 7 - removed);
    assert_eq!(game.envoys_at(0, minor), 3);
}

#[test]
fn a_civilization_cannot_imprison_its_own_spy() {
    let (mut game, _home, target, _) = game_with_spy_cities(774_266);
    // The operative's own civilization takes the target city while its
    // operations are still running inside it.
    game.cities.get_mut(&target).unwrap().owner = 0;

    // Detection, escape and capture are all dice rolls, so one resolution
    // proves nothing; walk the generator far enough to cover every branch
    // the old code could take.
    for _ in 0..64 {
        let spy_id = game.next_id;
        game.next_id += 1;
        game.spies.insert(
            spy_id,
            Spy {
                id: spy_id,
                owner: 0,
                level: 1,
                promotions: Default::default(),
                city: Some(target),
                ready_turn: 0,
                mission: Some(SpyMission {
                    kind: "fabricate_scandal".to_string(),
                    city: target,
                    target: game.cities[&target].pos,
                    started: game.turn,
                    ends: game.turn,
                }),
                sources_city: None,
                sources_until: 0,
                captured_by: None,
            },
        );
        game.resolve_spy_mission(spy_id);

        let spy = game
            .spies
            .get(&spy_id)
            .expect("an operative is not lost to its own police");
        assert_eq!(
            spy.captured_by, None,
            "there is no counterparty to ransom a spy back from yourself"
        );
        assert!(spy.mission.is_none(), "the operation has nothing left to rob");
        assert!(
            spy.ready_turn < u32::MAX,
            "the operative comes home usable rather than pinning its slot shut"
        );
    }
}

#[test]
fn captured_spies_remain_imprisoned_until_a_release_trade() {
    let (mut game, home, target, _) = game_with_spy_cities(774_265);
    let spy_id = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy_id,
        Spy {
            id: spy_id,
            owner: 0,
            level: 2,
            promotions: ["technologist".to_string(), "ace_driver".to_string()]
                .into_iter()
                .collect(),
            city: Some(target),
            ready_turn: u32::MAX,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: Some(1),
        },
    );

    game.turn = 120;
    game.process_spies(0);
    assert_eq!(game.spies[&spy_id].captured_by, Some(1));
    assert!(game.legal_spy_actions(0, spy_id).is_empty());

    game.players[0].gold = 500.0;
    assert!(game.quick_deals(0).iter().any(|deal| {
        deal.partner == 1
            && deal.category == "recover_spy"
            && deal.request.captured_spies == vec![spy_id]
    }));
    let offer = DealItems {
        gold: 220.0,
        ..DealItems::default()
    };
    let request = DealItems {
        captured_spies: vec![spy_id],
        ..DealItems::default()
    };
    game.do_trade(0, 1, &offer, &request).unwrap();
    let released = &game.spies[&spy_id];
    assert_eq!(released.captured_by, None);
    assert_eq!(released.city, Some(home));
    assert_eq!(released.ready_turn, game.turn);
    assert!(game
        .legal_spy_actions(0, spy_id)
        .iter()
        .any(|action| { matches!(action, Action::AssignSpy { spy, .. } if *spy == spy_id) }));
}

#[test]
fn espionage_pact_uses_its_stock_era_window_and_operation_effects() {
    let (mut game, _, target, commercial) = game_with_spy_cities(774_266);
    game.world_era = 3;
    assert!(!game
        .regular_congress_candidates()
        .iter()
        .any(|resolution| resolution.id == "espionage_pact"));
    game.world_era = 4;
    let pact = game
        .regular_congress_candidates()
        .into_iter()
        .find(|resolution| resolution.id == "espionage_pact")
        .unwrap();
    assert!(pact.choices.contains(&"A:siphon_funds".to_string()));
    assert!(pact.choices.contains(&"B:siphon_funds".to_string()));
    game.world_era = 7;
    assert!(!game
        .regular_congress_candidates()
        .iter()
        .any(|resolution| resolution.id == "espionage_pact"));

    let spy_id = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy_id,
        Spy {
            id: spy_id,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(target),
            ready_turn: game.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    let mission = SpyMission {
        kind: "siphon_funds".to_string(),
        city: target,
        target: commercial,
        started: game.turn,
        ends: game.turn + 8,
    };
    let baseline = game.spy_effective_level(spy_id, &mission);
    game.active_congress_effects.push(CongressEffect {
        resolution: "espionage_pact".to_string(),
        outcome: "A".to_string(),
        target: "siphon_funds".to_string(),
        expires: game.turn + 30,
    });
    assert_eq!(game.spy_effective_level(spy_id, &mission), baseline + 2);

    game.active_congress_effects.clear();
    game.active_congress_effects.push(CongressEffect {
        resolution: "espionage_pact".to_string(),
        outcome: "B".to_string(),
        target: "siphon_funds".to_string(),
        expires: game.turn + 30,
    });
    let operations = game.spy_operation_actions(&game.spies[&spy_id], &game.cities[&target]);
    assert!(!operations.iter().any(
        |action| matches!(action, Action::SpyMission { mission, .. } if mission == "siphon_funds")
    ));
    assert!(operations.iter().any(
        |action| matches!(action, Action::SpyMission { mission, .. } if mission == "foment_unrest")
    ));
}
