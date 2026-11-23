use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::install_test_district;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn settler() -> Item {
    AdvancedAi::early_settler_item()
}

fn quote(g: &mut Game, cid: u32, item: &Item, turns: f64, cost: f64) {
    Arc::make_mut(&mut g.host_buildable)
        .entry(cid)
        .or_default()
        .insert(
            Game::production_block_key(item),
            crate::game::HostMenuEntry {
                cost: Some(cost),
                turns: Some(turns),
            },
        );
}

/// `first-campus-before-the-settler`'s Online t35 board (a model board, not a
/// native replay): three cities, the capital training the floor's Settler with 20
/// Production paid and a five-turn Campus quoted there, the other two at one
/// citizen. Every tile is flat grassland hills, so every site ties.
fn board() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let mut g = Game::new_full(2, 36, 20, 95_344, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
    }
    g.players[0].civ = "Gran Colombia".to_string();
    for tech in ["pottery", "writing", "mining", "masonry", "bronze_working"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let ids: Vec<_> = [(5, 5), (15, 5), (25, 5)]
        .into_iter()
        .map(|pos| g.found_city_for(0, pos, None))
        .collect();
    g.game_speed = GameSpeed::Online;
    g.turn = 35;
    for &id in &ids {
        let city = g.cities.get_mut(&id).unwrap();
        city.pop = 1;
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
    let capital = g.cities.get_mut(&cid).unwrap();
    capital.pop = 5;
    capital.queue = vec![settler()];
    capital.production = 20.0;
    let campus = BasicAi::first_campus_item(&g, 0, cid).unwrap();
    quote(&mut g, cid, &campus, 5.0, 56.0);
    quote(&mut g, cid, &settler(), 8.0, 85.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_campus_claims_the_free_slot_2();
    ai.enable_early_settler_floor();
    ai.enable_science_seat_campus_before_the_settler();
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
    assert!(ai
        .free_slot_campus_item(&g, 0, cid, &plan)
        .is_some_and(|(item, turns)| item == campus && turns == 5.0));
    assert!(!AdvancedAi::empire_has_a_campus(&g, 0));
    (g, ai, cid, plan, campus)
}

fn other_city(g: &Game, cid: u32) -> u32 {
    g.player_city_ids(0)
        .into_iter()
        .find(|id| *id != cid)
        .unwrap()
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("science-seat-campus-before-the-settler", |ai| {
        ai.science_seat_campus_before_the_settler
    });
}

#[test]
fn a_science_seats_first_campus_goes_ahead_of_the_floor_settler() {
    let (mut g, mut ai, cid, plan, campus) = board();
    assert!(g.victory_conditions.science);
    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_settler();
    let mut control = g.clone();
    off.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(control.cities[&cid].queue.first(), Some(&settler()));

    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(g.turn, 0);
    ai.attach_journal(journal.handle());
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    assert_eq!(g.item_invested_production(cid, &settler()), 20.0);
    let thoughts = journal.since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought
            .detail
            .starts_with("science-seat-campus-before-the-settler:")),
        "{thoughts:?}"
    );
}

/// Only a Science seat with the Science victory on: the Domination gene's
/// own seats stay with that gene, and the other targets keep the Settler.
#[test]
fn only_a_science_seat_takes_it() {
    let (g, armed, cid, plan, campus) = board();
    let turns = g.host_production_turns(cid, &campus).unwrap();
    for (target, precedes) in [
        (VictoryTarget::Science, true),
        (VictoryTarget::Domination, false),
        (VictoryTarget::Score, false),
        (VictoryTarget::Culture, false),
        (VictoryTarget::Religion, false),
    ] {
        let mut ai = armed.clone();
        ai.victory_target = Some(target);
        assert_eq!(
            ai.first_campus_precedes_the_settler(&g, 0, cid, &plan, &campus, turns),
            precedes,
            "{target:?}"
        );
    }
    let mut no_science = g.clone();
    no_science.victory_conditions.science = false;
    assert!(!armed.first_campus_precedes_the_settler(&no_science, 0, cid, &plan, &campus, turns));
    let mut war = g.clone();
    war.at_war.insert((0, 1));
    assert!(!armed.first_campus_precedes_the_settler(&war, 0, cid, &plan, &campus, turns));
    let mut long = g.clone();
    quote(&mut long, cid, &campus, 8.0, 56.0);
    assert!(!armed.first_campus_precedes_the_settler(&long, 0, cid, &plan, &campus, 8.0));
}

#[test]
fn the_committed_campus_keeps_its_queue_against_the_floor() {
    let (mut g, ai, cid, plan, campus) = board();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    // A fresh replan: nothing paid into the Campus yet.
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    assert!(ai.science_campus_holds_repeat_settler(&g, 0, cid, &plan));

    let mut control = g.clone();
    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_settler();
    off.claim_early_settler_floor(&mut control, 0, &plan);
    assert_eq!(control.cities[&cid].queue.first(), Some(&settler()));

    ai.claim_early_settler_floor(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    let other = other_city(&g, cid);
    install_test_district(&mut g, other, "campus");
    assert!(!ai.first_campus_holds_the_settler(&g, 0, cid, &plan));
}
