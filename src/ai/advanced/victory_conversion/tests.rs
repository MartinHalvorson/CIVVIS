use super::*;

const TAGS: [&str; 10] = [
    "victory-deadline-budget",
    "culture-tourism-payback",
    "siege-positive-damage-budget",
    "culture-faith-reservation",
    "capital-campaign-router",
    "great-work-completion-value",
    "upgrade-window-campaign",
    "tourism-land-reservation",
    "reinforce-before-stall",
    "capture-hold-chain",
];

fn board(target: VictoryTarget) -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_119, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    let home = g.found_city_for(0, (4, 4), None);
    let enemy = g.found_city_for(1, (16, 8), None);
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 80;
    let ai = AdvancedAi::targeting(target);
    (g, ai, home, enemy)
}

fn plan(g: &Game, target: u32, strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn theater(g: &mut Game, cid: u32) {
    let pos = g.nbrs(g.cities[&cid].pos)[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), pos);
    g.map.tiles.get_mut(&pos).unwrap().district = Some(crate::name!("theater_square"));
    g.map.tiles.get_mut(&pos).unwrap().owner_city = Some(cid);
}

fn park(g: &mut Game, cid: u32) -> [Pos; 4] {
    let site = [(6, 4), (5, 5), (6, 5), (5, 6)];
    for pos in site {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.owner_city = Some(cid);
        tile.resource = None;
        tile.district = None;
        tile.wonder = None;
        tile.improvement = None;
        if !g.cities[&cid].owned_tiles.contains(&pos) {
            g.cities.get_mut(&cid).unwrap().owned_tiles.push(pos);
        }
    }
    g.map.tiles.get_mut(&site[0]).unwrap().terrain = crate::name!("grassland");
    g.spawn_test_unit("warrior", 0, site[0]);
    site
}

#[test]
fn all_ten_genes_are_independent_reversible_opt_ins() {
    let mut ai = AdvancedAi::legacy();
    for tag in TAGS {
        let gene = super::super::genes::GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .expect(tag);
        assert!(matches!(gene.kind, super::super::genes::Kind::OptIn));
        (gene.enable)(&mut ai);
        (gene.disable)(&mut ai);
    }
    assert!(
        !ai.victory_deadline_budget
            && !ai.culture_tourism_payback
            && !ai.siege_positive_damage_budget
            && !ai.culture_faith_reservation
            && !ai.capital_campaign_router
            && !ai.great_work_completion_value
            && !ai.upgrade_window_campaign
            && !ai.tourism_land_reservation
            && !ai.reinforce_before_stall
            && !ai.capture_hold_chain
    );
}

#[test]
fn deadline_requires_sustained_closing_in_the_same_lane() {
    let samples = VecDeque::from([
        (20, GrandStrategy::Culture, 50.0),
        (30, GrandStrategy::Culture, 70.0),
    ]);
    assert_eq!(closing_eta(&samples), Some(15.5));
    assert_eq!(
        closing_eta(&VecDeque::from([
            (20, GrandStrategy::Culture, 70.0),
            (30, GrandStrategy::Culture, 60.0)
        ])),
        None
    );
    assert_eq!(
        closing_eta(&VecDeque::from([
            (20, GrandStrategy::Science, 50.0),
            (30, GrandStrategy::Culture, 70.0)
        ])),
        None
    );
    assert_eq!(
        closing_eta(&VecDeque::from([
            (29, GrandStrategy::Culture, 50.0),
            (30, GrandStrategy::Culture, 70.0)
        ])),
        None
    );
}

#[test]
fn deadline_cuts_late_settlers_but_preserves_emergency_defense() {
    let (g, mut ai, home, enemy) = board(VictoryTarget::Domination);
    let mut p = plan(&g, enemy, GrandStrategy::Conquest);
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    ai.conversion.horizon = Some(12.0);
    assert_eq!(
        ai.conversion_production_adjustment(&g, 0, home, &settler, &p, 10.0, 1000.0),
        0.0
    );
    ai.enable_victory_deadline_budget();
    assert!(ai.conversion_production_adjustment(&g, 0, home, &settler, &p, 10.0, 1000.0) < 0.0);
    p.threatened_city = Some(home);
    assert_eq!(
        ai.conversion_production_adjustment(
            &g,
            0,
            home,
            &Item::Unit {
                unit: crate::name!("archer")
            },
            &p,
            20.0,
            1000.0
        ),
        0.0
    );
}

#[test]
fn tourism_payback_values_housing_that_activates_owned_works() {
    let (mut g, mut ai, home, enemy) = board(VictoryTarget::Culture);
    theater(&mut g, home);
    g.players[0].counters.insert("great_work:writing".into(), 4);
    let item = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let p = plan(&g, enemy, GrandStrategy::Culture);
    let before = g.tourism_per_turn_model(0);
    ai.enable_culture_tourism_payback();
    let early = ai.conversion_production_adjustment(&g, 0, home, &item, &p, 5.0, 1000.0);
    assert!(early > 0.0, "housing preview must activate owned writings");
    assert_eq!(
        g.tourism_per_turn_model(0),
        before,
        "preview must not mutate real game"
    );
    ai.enable_victory_deadline_budget();
    ai.conversion.horizon = Some(5.0);
    assert!(ai.conversion_production_adjustment(&g, 0, home, &item, &p, 10.0, 1000.0) < early);
}

#[test]
fn completion_gene_prices_a_filled_building_above_an_empty_one() {
    let (mut g, mut ai, home, enemy) = board(VictoryTarget::Culture);
    theater(&mut g, home);
    let item = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let p = plan(&g, enemy, GrandStrategy::Culture);
    ai.enable_great_work_completion_value();
    assert_eq!(
        ai.conversion_production_adjustment(&g, 0, home, &item, &p, 5.0, 1000.0),
        0.0
    );
    g.players[0].counters.insert("great_work:writing".into(), 4);
    assert!(ai.conversion_production_adjustment(&g, 0, home, &item, &p, 5.0, 1000.0) > 0.0);
}

#[test]
fn faith_reservation_requires_a_reachable_opportunity_and_releases_for_purchase() {
    let (mut g, mut ai, home, _) = board(VictoryTarget::Culture);
    ai.enable_culture_faith_reservation();
    assert_eq!(ai.conversion_faith_reserve(&g, 0, 700.0), 0.0);
    let site = park(&mut g, home);
    g.players[0].civic = Some("conservation".into());
    assert!(ai.conversion_faith_reserve(&g, 0, 100.0) > 100.0);
    assert!(!g.players[0].civics.contains(&crate::name!("conservation")));
    g.players[0].civics.insert(crate::name!("conservation"));
    assert!(g.national_park_sites(0).contains(&site));
    g.players[0].faith = 10_000.0;
    assert!(ai.conversion_culture_purchase(&mut g, 0));
    assert_eq!(
        g.units
            .values()
            .filter(|u| u.owner == 0 && u.kind == "naturalist")
            .count(),
        1
    );
    assert_eq!(
        ai.conversion_faith_reserve(&g, 0, 700.0),
        0.0,
        "do not reserve a second naturalist behind the first"
    );
}

#[test]
fn land_reservation_precedes_unlock_and_never_changes_real_terrain() {
    let (mut g, mut ai, home, enemy) = board(VictoryTarget::Culture);
    let site = park(&mut g, home);
    g.players[0].civics.insert(crate::name!("humanism"));
    ai.enable_tourism_land_reservation();
    ai.plan_tourism_land(&g, 0);
    assert!(site.iter().all(|p| ai.conversion.parks.contains(p)));
    assert!(ai.conversion_land_penalty(&g, site[0], "farm") < 0.0);
    let item = Item::District {
        district: crate::name!("campus"),
        pos: site[0],
    };
    assert!(
        ai.conversion_production_adjustment(
            &g,
            0,
            home,
            &item,
            &plan(&g, enemy, GrandStrategy::Culture),
            5.0,
            1000.0
        ) < 0.0
    );
    assert!(!g.players[0].civics.contains(&crate::name!("conservation")));
    assert!(g.map.tiles[&site[0]].improvement.is_none());
    ai.disable_tourism_land_reservation();
    assert_eq!(ai.conversion_land_penalty(&g, site[0], "farm"), 0.0);
}

#[test]
fn siege_budget_rejects_an_archer_only_train_and_accepts_a_capable_force() {
    let (mut g, mut ai, _, enemy) = board(VictoryTarget::Domination);
    g.at_war.insert((0, 1));
    let pos = g.nbrs(g.cities[&enemy].pos)[0];
    let archer = g.spawn_test_unit("archer", 0, pos);
    ai.enable_siege_positive_damage_budget();
    assert!(!ai.conversion_siege_ready(&g, 0, enemy, &[archer]));
    let mut force = vec![archer];
    for p in g.nbrs(g.cities[&enemy].pos) {
        force.push(g.spawn_test_unit("modern_armor", 0, p));
    }
    assert!(ai.conversion_siege_ready(&g, 0, enemy, &force));
    ai.disable_siege_positive_damage_budget();
    assert!(ai.conversion_siege_ready(&g, 0, enemy, &[archer]));
}

#[test]
fn reinforcement_orders_the_missing_breach_role_and_stops_at_the_deadline() {
    let (mut g, mut ai, home, enemy) = board(VictoryTarget::Domination);
    g.at_war.insert((0, 1));
    g.cities.get_mut(&enemy).unwrap().wall_hp = 100;
    g.cities
        .get_mut(&enemy)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    let pos = g.nbrs(g.cities[&enemy].pos)[0];
    g.spawn_test_unit("warrior", 0, pos);
    ai.enable_reinforce_before_stall();
    let p = plan(&g, enemy, GrandStrategy::Conquest);
    let siege = Item::Unit {
        unit: crate::name!("catapult"),
    };
    assert!(ai.conversion_production_adjustment(&g, 0, home, &siege, &p, 5.0, 1000.0) > 0.0);
    ai.enable_victory_deadline_budget();
    ai.conversion.horizon = Some(2.0);
    assert!(ai.conversion_production_adjustment(&g, 0, home, &siege, &p, 5.0, 1000.0) <= 0.0);
}

#[test]
fn capital_router_needs_a_known_reachable_objective_and_keeps_the_opponent() {
    let (mut g, mut ai, _, enemy) = board(VictoryTarget::Domination);
    ai.enable_capital_campaign_router();
    assert_eq!(ai.conversion_campaign_target(&g, 0, Some(1)), None);
    g.at_war.insert((0, 1));
    g.spawn_test_unit("warrior", 0, g.nbrs(g.cities[&enemy].pos)[0]);
    assert_eq!(ai.conversion_campaign_target(&g, 0, Some(1)), Some(enemy));
    assert_eq!(ai.conversion_campaign_target(&g, 0, None), None);
    assert_eq!(ai.conversion_campaign_target(&g, 0, Some(0)), None);
}

#[test]
fn upgrade_window_reserves_a_package_and_never_declares_before_it_is_ready() {
    let (mut g, mut ai, home, _) = board(VictoryTarget::Domination);
    let pos = g.cities[&home].pos;
    g.spawn_test_unit("warrior", 0, pos);
    g.spawn_test_unit("warrior", 0, g.nbrs(pos)[0]);
    let next = g.player_unit_replacement(0, g.rules.units["warrior"].upgrade_to.unwrap());
    let goal = g.rules.units[next].tech.unwrap();
    g.players[0].techs.extend(g.rules.techs.keys().copied());
    g.players[0].techs.remove(&goal);
    g.players[0].research = Some(goal.as_str().into());
    g.players[0].research_progress = g.tech_cost(goal.as_str()) - 1.0;
    ai.enable_upgrade_window_campaign();
    ai.plan_upgrade_window(&g, 0);
    assert_eq!(ai.conversion.upgrade_goal, Some(goal));
    assert_eq!(ai.conversion.upgrade_units.len(), 2);
    assert!(ai.conversion.upgrade_reserve > 0.0);
    assert!(!ai.conversion_upgrade_launch_ready(&g, 0));
    g.turn += 100;
    ai.plan_upgrade_window(&g, 0);
    assert_eq!(
        ai.conversion.upgrade_reserve, 0.0,
        "resource shortage must not bank forever"
    );
}

#[test]
fn capture_hold_uses_time_to_revolt_and_prioritizes_victor_and_repairs() {
    let (mut g, mut ai, home, enemy) = board(VictoryTarget::Domination);
    g.cities.get_mut(&enemy).unwrap().owner = 0;
    g.cities.get_mut(&enemy).unwrap().occupied_from = Some(1);
    g.cities.get_mut(&enemy).unwrap().loyalty = 60.0;
    g.cities.get_mut(&enemy).unwrap().hp = 100;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(enemy, -20.0);
    ai.enable_capture_hold_chain();
    assert_eq!(ai.conversion_garrison_priority(&g, &g.cities[&enemy]), 3.0);
    assert!(
        ai.conversion_garrison_priority(&g, &g.cities[&enemy])
            < ai.conversion_garrison_priority(&g, &g.cities[&home])
    );
    assert!(ai.conversion_hold_governor_bonus(&g, 0, enemy, "victor") > 800.0);
    assert_eq!(
        ai.conversion_hold_governor_bonus(&g, 0, enemy, "pingala"),
        0.0
    );
    let repair = Item::Repair {
        repair: crate::name!("monument"),
        pos: g.cities[&enemy].pos,
    };
    assert!(
        ai.conversion_production_adjustment(
            &g,
            0,
            enemy,
            &repair,
            &plan(&g, enemy, GrandStrategy::Conquest),
            3.0,
            1000.0
        ) > 0.0
    );
}

#[test]
fn all_genes_respect_recovery_and_other_victory_targets() {
    let (g, mut ai, home, enemy) = board(VictoryTarget::Science);
    for tag in TAGS {
        let gene = super::super::genes::GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .unwrap();
        (gene.enable)(&mut ai);
    }
    ai.observe_victory_conversion(&g, 0);
    let p = plan(&g, enemy, GrandStrategy::Science);
    assert_eq!(
        ai.conversion_production_adjustment(
            &g,
            0,
            home,
            &Item::Building {
                building: crate::name!("library")
            },
            &p,
            5.0,
            1000.0
        ),
        0.0
    );
    assert!(ai.conversion.parks.is_empty());
    assert!(ai.conversion.upgrade_units.is_empty());
    assert_eq!(ai.conversion_faith_reserve(&g, 0, 700.0), 700.0);
}

/// Manual, paired native Emperor measurement. Full games are intentionally
/// outside the unit-test gate; run this explicitly as documented in the
/// experiment note. Each arm changes only the measured seat's ten switches.
#[test]
#[ignore]
fn emperor_conversion_paired_census() {
    let seeds: usize = std::env::var("CIVVIS_CONVERSION_SEEDS")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .expect("positive seed count");
    assert!(seeds > 0);
    let mut results = Vec::new();
    for target in [VictoryTarget::Culture, VictoryTarget::Domination] {
        for index in 0..seeds {
            let seat = index % 4;
            for enabled in [false, true] {
                let mut game = Game::new_with(crate::game::GameOptions {
                    difficulty: "emperor".into(),
                    speed: "online".into(),
                    handicap_exempt: BTreeSet::from([seat]),
                    randomize_civs: true,
                    map_script: crate::setup::MapScript::Pangaea,
                    ..crate::game::GameOptions::new(4, 40, 24, 91_110_000 + index as u64, 250, 4)
                });
                game.native_competitions = true;
                let mut ais: Vec<_> = (0..game.players.len())
                    .map(|pid| {
                        let mut ai = AdvancedAi::new();
                        if pid == seat {
                            ai.retarget(target);
                            for tag in TAGS {
                                let gene = super::super::genes::GENES
                                    .iter()
                                    .find(|gene| gene.tag == tag)
                                    .unwrap();
                                if enabled {
                                    (gene.enable)(&mut ai);
                                } else {
                                    (gene.disable)(&mut ai);
                                }
                            }
                        } else if !game.players[pid].is_minor && !game.players[pid].is_barbarian {
                            ai.retarget(
                                [
                                    VictoryTarget::Science,
                                    VictoryTarget::Culture,
                                    VictoryTarget::Domination,
                                ][(pid + index) % 3],
                            );
                        }
                        ai
                    })
                    .collect();
                let started = std::time::Instant::now();
                crate::ai::run_game(&mut game, &mut ais);
                let requested = match target {
                    VictoryTarget::Culture => "culture",
                    _ => "domination",
                };
                let won = game.winning_players().contains(&seat);
                let row = serde_json::json!({"seed":91_110_000+index as u64,"seat":seat,"target":requested,"bundle_on":enabled,"winner":game.winner,"victory":game.victory_type,"target_win":won && game.victory_type.as_deref()==Some(requested),"any_win":won,"turn":game.reported_turn(),"seconds":started.elapsed().as_secs_f64(),"cities":game.player_city_ids(seat).len(),"foreign_tourists":game.foreign_tourists(seat)});
                println!("CONVERSION_RESULT {row}");
                results.push(row);
            }
        }
    }
    if let Ok(path) = std::env::var("CIVVIS_CONVERSION_OUT") {
        std::fs::write(path,serde_json::to_string_pretty(&serde_json::json!({"difficulty":"emperor","speed":"online","map":"pangaea","players":4,"city_states":4,"turn_cap":250,"handicap":"rivals only","genes":TAGS,"rows":results})).unwrap()).unwrap();
    }
}
