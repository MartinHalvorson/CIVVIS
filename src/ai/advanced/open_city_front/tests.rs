use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use std::sync::Arc;

/// Open grassland: our capital in the west; player 1, at war with us, holds a
/// walled city and, six tiles east of it, an open (unwalled) city. A scout
/// on each enemy tile keeps both in sight. Returns the board, the walled
/// city and the open city.
fn front() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 36, 22, 4_175, 300, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (2, 8), None);
    let walled = g.found_city_for(1, (12, 8), None);
    let open = g.found_city_for(1, (18, 8), None);
    {
        let city = g.cities.get_mut(&walled).unwrap();
        city.buildings.push(crate::name!("walls"));
        city.wall_hp = 100;
    }
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 110;
    let walled_pos = g.cities[&walled].pos;
    let open_pos = g.cities[&open].pos;
    g.spawn_test_unit("scout", 0, walled_pos);
    g.spawn_test_unit("scout", 0, open_pos);
    (g, walled, open)
}

fn controller(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_commitment();
    if on {
        ai.enable_open_city_draws_the_front();
    }
    ai
}

/// A healthy heavy chariot beside the walled city: five tiles from the
/// open one, outside the open foothold's three.
fn column_at_the_walls(g: &mut Game) {
    g.spawn_test_unit("heavy_chariot", 0, (13, 8));
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("open-city-draws-the-front", |ai| {
        ai.open_city_draws_the_front
    });
}

#[test]
fn a_field_army_taker_within_the_march_turns_the_front() {
    let (mut g, walled, open) = front();
    column_at_the_walls(&mut g);
    assert_eq!(g.wdist((13, 8), g.cities[&open].pos), 5);
    assert_eq!(
        controller(false).capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        None,
        "gene off: the open foothold wants a taker within three tiles"
    );
    assert_eq!(
        controller(true).capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        Some(open),
        "gene on: a field-army taker five tiles out turns the front"
    );
}

#[test]
fn a_taker_past_the_march_does_not_count() {
    let (mut g, walled, _) = front();
    g.spawn_test_unit("heavy_chariot", 0, (8, 8));
    assert_eq!(
        controller(true).capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        None,
        "ten tiles from the open city is past the march"
    );
}

#[test]
fn recovery_and_expansion_turn_with_the_gene() {
    let (mut g, walled, open) = front();
    column_at_the_walls(&mut g);
    for strategy in [GrandStrategy::Recovery, GrandStrategy::Expansion] {
        assert_eq!(
            controller(false).capture_opportunity_city(&g, 0, walled, strategy),
            None
        );
        assert_eq!(
            controller(true).capture_opportunity_city(&g, 0, walled, strategy),
            Some(open)
        );
    }
    assert_eq!(
        controller(true).capture_opportunity_city(&g, 0, walled, GrandStrategy::Science),
        None,
        "only Recovery and Expansion join Conquest"
    );
}

#[test]
fn a_lightly_chipped_prior_still_yields() {
    let (mut g, walled, open) = front();
    column_at_the_walls(&mut g);
    g.cities.get_mut(&walled).unwrap().wall_hp = 80;
    assert_eq!(
        controller(false).capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        None,
        "gene off: any chip in the prior's walls keeps the siege"
    );
    assert_eq!(
        controller(true).capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        Some(open)
    );
    g.cities.get_mut(&walled).unwrap().wall_hp = 60;
    assert_eq!(
        controller(true).capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        None,
        "past a quarter of its walls the siege is progressing"
    );
}

#[test]
fn the_tier_floor_skips_an_open_city_beyond_our_melee() {
    let (mut g, walled, open) = front();
    column_at_the_walls(&mut g);
    let ai = controller(true);
    let best = ai.open_city_best_melee(&g, 0);
    assert!(best > 0.0);
    // The open foothold wants the open city weaker than the walled one.
    Arc::make_mut(&mut g.observed_city_strength).insert(walled, best + 40.0);
    Arc::make_mut(&mut g.observed_city_strength).insert(open, best + OPEN_CITY_TIER_MARGIN + 1.0);
    assert_eq!(
        ai.capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        None
    );
    Arc::make_mut(&mut g.observed_city_strength).insert(open, best + OPEN_CITY_TIER_MARGIN - 1.0);
    assert_eq!(
        ai.capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        Some(open)
    );
}

#[test]
fn a_city_the_front_left_is_not_turned_back_to() {
    let (mut g, walled, open) = front();
    column_at_the_walls(&mut g);
    let mut ai = controller(true);
    ai.open_city_left.insert(open, g.turn + 5);
    assert_eq!(
        ai.capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        None
    );
    g.turn += 5;
    assert_eq!(
        ai.capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        Some(open)
    );
}

fn plan_on(ai: &mut AdvancedAi, g: &Game, city: u32) {
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(city),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
}

#[test]
fn the_hold_starts_on_the_turn_and_gives_up_after_the_stall() {
    let (mut g, walled, open) = front();
    let mut ai = controller(true);
    plan_on(&mut ai, &g, walled);
    ai.open_city_observe(&g, 0);
    assert_eq!(ai.open_city_hold, None);
    g.turn += 1;
    plan_on(&mut ai, &g, open);
    ai.open_city_observe(&g, 0);
    let hold = ai
        .open_city_hold
        .clone()
        .expect("the turn to the open city is held");
    assert_eq!((hold.city, hold.from), (open, walled));
    // Progress resets the stall clock.
    g.turn += 4;
    g.cities.get_mut(&open).unwrap().hp = 150;
    ai.open_city_observe(&g, 0);
    assert_eq!(
        ai.open_city_hold.as_ref().map(|h| (h.best_hp, h.best_turn)),
        Some((150, g.turn))
    );
    let stall = g.standard_duration(OPEN_CITY_STALL_TURNS);
    g.turn += stall - 1;
    ai.open_city_observe(&g, 0);
    assert!(ai.open_city_hold.is_some(), "one turn short of the stall");
    g.turn += 1;
    ai.open_city_observe(&g, 0);
    assert_eq!(ai.open_city_hold, None, "stalled: given up");
    assert!(
        ai.capture_stood_down.contains_key(&open),
        "the open city is stood down"
    );
    assert!(ai.open_city_recently_left(&g, open));
}

#[test]
fn the_hold_ends_when_the_open_city_walls_up_or_falls() {
    let (mut g, walled, open) = front();
    let mut ai = controller(true);
    plan_on(&mut ai, &g, walled);
    ai.open_city_observe(&g, 0);
    plan_on(&mut ai, &g, open);
    ai.open_city_observe(&g, 0);
    assert!(ai.open_city_hold.is_some());
    let mut walls = g.clone();
    walls.cities.get_mut(&open).unwrap().wall_hp = 100;
    let mut walled_ai = ai.clone();
    walled_ai.open_city_observe(&walls, 0);
    assert_eq!(walled_ai.open_city_hold, None, "walled: the hold ends");
    assert!(
        !walled_ai.capture_stood_down.contains_key(&open),
        "walling is no stand-down"
    );
    g.cities.get_mut(&open).unwrap().owner = 0;
    ai.open_city_observe(&g, 0);
    assert_eq!(ai.open_city_hold, None, "captured: the hold ends");
}

#[test]
fn leaving_the_open_city_keeps_it_out_of_the_turn() {
    let (g, walled, open) = front();
    let mut ai = controller(true);
    plan_on(&mut ai, &g, walled);
    ai.open_city_observe(&g, 0);
    plan_on(&mut ai, &g, open);
    ai.open_city_observe(&g, 0);
    plan_on(&mut ai, &g, walled);
    ai.open_city_observe(&g, 0);
    assert_eq!(ai.open_city_hold, None);
    assert!(
        ai.open_city_recently_left(&g, open),
        "no turning back and forth"
    );
    assert!(
        !ai.capture_stood_down.contains_key(&open),
        "leaving is no stand-down"
    );
}

#[test]
fn observe_is_a_no_op_with_the_gene_off() {
    let (g, walled, open) = front();
    let mut ai = controller(false);
    plan_on(&mut ai, &g, walled);
    ai.open_city_observe(&g, 0);
    plan_on(&mut ai, &g, open);
    ai.open_city_observe(&g, 0);
    assert_eq!(ai.open_city_hold, None);
    assert_eq!(ai.open_city_last_target, None);
    assert!(ai.open_city_left.is_empty());
}
