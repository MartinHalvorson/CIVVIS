use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::install_test_district;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn settler() -> Item {
    AdvancedAi::early_settler_item()
}

/// Native quotes and pipeline shape from Indonesia003458Z40/1;
/// the terrain and remaining board are a model fixture, not a native replay.
fn board() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let mut g = Game::new_full(2, 36, 20, crate::rng::fixture_seed("AUTO_9", 95_344), 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
    }
    g.players[0].civ = "Indonesia".to_string();
    for tech in ["pottery", "writing", "mining", "masonry", "bronze_working"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let ids: Vec<_> = [(5, 5), (15, 5), (25, 5)]
        .into_iter()
        .map(|pos| g.found_city_for(0, pos, None))
        .collect();
    g.game_speed = GameSpeed::Online;
    g.turn = 40;
    for &id in &ids {
        let city = g.cities.get_mut(&id).unwrap();
        city.pop = 5;
        city.production = 0.0;
        city.queue = vec![Item::Building {
            building: crate::name!("walls"),
        }];
        let production = g.city_yields(id).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            id,
            Yields {
                production: 11.0 - production,
                ..Default::default()
            },
        );
    }
    let cid = ids[0];
    g.cities.get_mut(&cid).unwrap().queue = vec![settler()];
    g.cities.get_mut(&cid).unwrap().production = 20.0;
    g.spawn_unit("settler", 0, g.cities[&ids[1]].pos);
    let campus = BasicAi::first_campus_item(&g, 0, cid).unwrap();
    for (item, turns, cost) in [(campus.clone(), 5.0, 56.0), (settler(), 8.0, 85.0)] {
        Arc::make_mut(&mut g.host_buildable)
            .entry(cid)
            .or_default()
            .insert(
                Game::production_block_key(&item),
                crate::game::HostMenuEntry {
                    cost: Some(cost),
                    turns: Some(turns),
                },
            );
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.disable_campus_claims_the_free_slot();
    ai.disable_campus_claims_the_free_slot_2();
    ai.enable_early_settler_floor();
    ai.enable_science_campus_before_repeat_settler();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.early_settler_floor_holds(&g, 0, cid, &plan));
    assert!(ai.free_slot_campus_item(&g, 0, cid, &plan).is_some());
    (g, ai, cid, plan, campus)
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("science-campus-before-repeat-settler", |ai| {
        ai.science_campus_before_repeat_settler
    });
}

#[test]
fn short_native_campus_precedes_repeated_settler_without_losing_paid_work() {
    let (mut g, ai, cid, plan, campus) = board();
    let mut off = ai.clone();
    off.disable_science_campus_before_repeat_settler();
    let mut control = g.clone();
    off.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(control.cities[&cid].queue.first(), Some(&settler()));
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    assert_eq!(g.item_invested_production(cid, &settler()), 20.0);
    assert_eq!(ai.early_settler_floor(&g, &plan), 6);
}

#[test]
fn exception_requires_native_quotes_pipeline_and_safe_science_plan() {
    let (base, armed, cid, base_plan, campus) = board();
    for case in [
        "off",
        "culture",
        "science disabled",
        "rush",
        "threat",
        "recent attack",
        "war",
        "no walker",
        "first expansion",
        "no campus quote",
        "no settler quote",
        "six turn campus",
        "equal settler",
        "invalid campus quote",
        "not a settler",
    ] {
        let mut g = base.clone();
        let mut ai = armed.clone();
        let mut plan = base_plan.clone();
        match case {
            "off" => ai.disable_science_campus_before_repeat_settler(),
            "culture" => ai.victory_target = Some(VictoryTarget::Culture),
            "science disabled" => g.victory_conditions.science = false,
            "rush" => plan.rush = true,
            "threat" => plan.threatened_city = Some(cid),
            "recent attack" => g.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1,
            "war" => {
                g.at_war.insert((0, 1));
            }
            "no walker" => {
                for id in g.player_unit_ids(0) {
                    g.remove_unit(id);
                }
            }
            "first expansion" => {
                let others: Vec<_> = g
                    .player_city_ids(0)
                    .into_iter()
                    .filter(|id| *id != cid)
                    .collect();
                for id in others {
                    g.cities.get_mut(&id).unwrap().owner = 1;
                }
            }
            "no campus quote" | "no settler quote" => {
                let item = if case == "no campus quote" {
                    campus.clone()
                } else {
                    settler()
                };
                let key = Game::production_block_key(&item);
                Arc::make_mut(&mut g.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .remove(&key);
            }
            "six turn campus" | "invalid campus quote" => {
                Arc::make_mut(&mut g.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&campus))
                    .unwrap()
                    .turns = Some(if case == "six turn campus" {
                    6.0
                } else {
                    f64::NAN
                });
            }
            "equal settler" => {
                Arc::make_mut(&mut g.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&settler()))
                    .unwrap()
                    .turns = Some(5.0);
            }
            "not a settler" => g.cities.get_mut(&cid).unwrap().queue.clear(),
            _ => unreachable!(),
        }
        assert!(
            !ai.science_campus_precedes_repeat_settler(&g, 0, cid, &plan, &campus),
            "{case}"
        );
    }
}

#[test]
fn commitment_survives_zero_progress_and_native_repricing_without_a_walker() {
    let (mut g, ai, cid, plan, campus) = board();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    for id in g.player_unit_ids(0) {
        g.remove_unit(id);
    }
    Arc::make_mut(&mut g.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&Game::production_block_key(&campus))
        .unwrap()
        .turns = Some(6.0);
    assert!(AdvancedAi::early_settler_may_displace(&g, 0, cid));
    assert!(ai.science_campus_holds_repeat_settler(&g, 0, cid, &plan));
    let mut control = g.clone();
    let mut off = ai.clone();
    off.disable_science_campus_before_repeat_settler();
    off.claim_early_settler_floor(&mut control, 0, &plan);
    assert_eq!(control.cities[&cid].queue.first(), Some(&settler()));
    ai.claim_early_settler_floor(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
}

#[test]
fn midgame_settler_claim_also_respects_the_first_city_campus_commitment() {
    let (mut g, mut ai, cid, mut plan, campus) = board();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    g.turn = g
        .standard_duration(super::super::early_settler_floor::EARLY_SETTLER_FLOOR_STANDARD_TURN)
        + 1;
    plan.assessed_turn = g.turn;
    ai.enable_holdable_land_keeps_settling();
    // Make a new settlement a real alternative for the control. The walker
    // has left the pipeline and the empire has explored the nearby land.
    for id in g.player_unit_ids(0) {
        g.remove_unit(id);
    }
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let threats = AdvancedAi::holdable_land_threats(&g, 0);
    let site = ai.holdable_land_seat(&g, 0, g.cities[&cid].pos, 0, &threats);
    assert!(
        site.is_ok(),
        "the control needs an actual holdable seat: {site:?}"
    );
    let other = g
        .player_city_ids(0)
        .into_iter()
        .find(|id| *id != cid)
        .unwrap();
    install_test_district(&mut g, other, "campus");
    assert!(AdvancedAi::holdable_land_may_displace(&g, 0, cid));
    assert!(ai.science_campus_holds_repeat_settler(&g, 0, cid, &plan));
    let mut control = g.clone();
    let mut off = ai.clone();
    off.disable_science_campus_before_repeat_settler();
    assert!(off.holdable_land_window(&control));
    assert_eq!(AdvancedAi::early_settler_census(&control, 0), (0, 0));
    assert!(control.can_produce(0, cid, &settler()));
    assert!(
        control.host_production_turns(cid, &settler()).unwrap()
            <= control.standard_duration(super::super::early_settler_floor::EARLY_SETTLER_MAX_BUILD)
                as f64
    );
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(control.turn, 0);
    off.attach_journal(journal.handle());
    off.claim_holdable_land_settler(&mut control, 0, &plan);
    assert_eq!(
        control.cities[&cid].queue.first(),
        Some(&settler()),
        "the unprotected midgame claim would actually replace this Campus: {:?}",
        journal.since(0)
    );
    ai.claim_holdable_land_settler(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
}

#[test]
fn commitment_yields_to_new_defensive_or_victory_priorities() {
    let (mut g, ai, cid, plan, campus) = board();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(cid);
    assert!(!ai.science_campus_holds_repeat_settler(&g, 0, cid, &threatened));
    let mut other_goal = ai.clone();
    other_goal.victory_target = Some(VictoryTarget::Culture);
    assert!(!other_goal.science_campus_holds_repeat_settler(&g, 0, cid, &plan));
    assert!(ai.free_slot_campus_holds(&g, 0, cid, &plan, &campus));
}

#[test]
fn standalone_gene_does_not_enable_ordinary_campus_claims() {
    let (base, ai, cid, plan, _) = board();
    for head in [
        None,
        Some(Item::Building {
            building: crate::name!("granary"),
        }),
        Some(Item::Unit {
            unit: crate::name!("builder"),
        }),
    ] {
        let mut g = base.clone();
        g.cities.get_mut(&cid).unwrap().queue = head.clone().into_iter().collect();
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(g.cities[&cid].queue.first().cloned(), head);
    }
}
