use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::game::Action;

/// Two majors; ours holds one coastal capital with a charted waterway whose
/// far end is unseen, the land is plain grassland, and a Monument is on the
/// queue. Seat 1 is unmet. Sailing and Cartography are on our tree.
fn board() -> (Game, u32) {
    let mut game = Game::new_full(2, 30, 18, 71_003, 100, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the first major starts with a Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the fixture founds a capital");
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let city = game.player_city_ids(0)[0];
    let city_pos = game.cities[&city].pos;
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let mut waterway = Vec::new();
    let mut current = city_pos;
    for distance in 1..=crate::ai::NAVAL_RECON_MIN_WATERWAY_TILES {
        current = game
            .nbrs(current)
            .into_iter()
            .find(|pos| {
                game.map.tiles.contains_key(pos) && game.wdist(city_pos, *pos) == distance as i32
            })
            .expect("an interior city has a straight coastal launch route");
        game.map.tiles.get_mut(&current).unwrap().terrain = crate::name!("coast");
        waterway.push(current);
    }
    game.players[0].explored.remove(waterway.last().unwrap());
    game.players[0].techs.insert(crate::name!("sailing"));
    game.players[0].techs.insert(crate::name!("cartography"));
    game.players[0].met.remove(&1);
    game.players[1].met.remove(&0);
    game.at_war.clear();
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::Building {
                building: crate::name!("monument"),
            },
        },
    )
    .expect("the fixture queues a Monument");
    (game, city)
}

fn plan(turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: turn,
        rush: false,
    }
}

/// The live arming the gene joins: naval recon is on.
fn stock() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    ai
}

fn calls() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_unmet_majors_call_for_a_ship();
    ai
}

fn head_is_a_ship(g: &Game, city: u32) -> bool {
    matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit })
        if g.rules.units.get(unit.as_str()).is_some_and(|spec| spec.domain.as_deref() == Some("sea")))
}

#[test]
fn unmet_majors_call_for_a_ship_is_opt_in() {
    opt_in_off_in_both_controllers("unmet-majors-call-for-a-ship", |ai| {
        ai.unmet_majors_call_for_a_ship
    });
}

#[test]
fn an_unmet_major_puts_a_sea_scout_ahead_of_a_busy_queue() {
    let (g, city) = board();
    assert!(!g.has_met(0, 1), "fixture: seat 1 is unmet");
    assert!(
        stock().base.naval_recon_is_the_missing_arm(&g, 0),
        "fixture: the naval eye is the missing arm"
    );
    let mut off = g.clone();
    let turn_plan = plan(off.turn);
    stock().unmet_majors_call_for_a_ship(&mut off, 0, &turn_plan);
    assert!(
        !head_is_a_ship(&off, city),
        "off: the Monument keeps the queue"
    );
    let mut on = g.clone();
    let turn_plan = plan(on.turn);
    stock().reserve_idle_naval_recon(&mut on, 0, &turn_plan);
    assert!(
        !head_is_a_ship(&on, city),
        "stock: the idle-queue reserve does not take a busy queue"
    );
    let turn_plan = plan(on.turn);
    calls().unmet_majors_call_for_a_ship(&mut on, 0, &turn_plan);
    assert!(head_is_a_ship(&on, city), "on: the sea scout goes first");
    assert!(
        on.cities[&city]
            .queue
            .iter()
            .any(|item| matches!(item, Item::Building { building } if *building == crate::name!("monument"))),
        "the Monument stays on the queue"
    );
}

#[test]
fn every_major_met_or_no_ocean_crossing_calls_no_ship() {
    let (mut met, city) = board();
    met.record_contact(0, 1);
    let turn_plan = plan(met.turn);
    calls().unmet_majors_call_for_a_ship(&mut met, 0, &turn_plan);
    assert!(!head_is_a_ship(&met, city), "every major met: no ship");

    let (mut inland, city) = board();
    inland.players[0].techs.remove(&crate::name!("cartography"));
    let turn_plan = plan(inland.turn);
    calls().unmet_majors_call_for_a_ship(&mut inland, 0, &turn_plan);
    assert!(
        !head_is_a_ship(&inland, city),
        "without Cartography a ship cannot reach another continent"
    );
}

#[test]
fn a_sea_scout_already_at_sea_is_the_one_ship() {
    let (mut g, city) = board();
    let water = g
        .map
        .tiles
        .iter()
        .find(|(_, tile)| tile.terrain == crate::name!("coast"))
        .map(|(pos, _)| *pos)
        .expect("the fixture has coast");
    g.spawn_test_unit("galley", 0, water);
    let turn_plan = plan(g.turn);
    calls().unmet_majors_call_for_a_ship(&mut g, 0, &turn_plan);
    assert!(!head_is_a_ship(&g, city), "one ship at a time");
}
