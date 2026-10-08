use super::*;
use crate::game::install_test_district;
use std::sync::Arc;

/// Live Emperor G425 near turn 88: Georgia's Orthodoxy (seat 1) holds France
/// (seat 3) and two of our five cities; Caracas (our last city here) follows
/// Gaul's Hinduism (seat 2) with a finished Holy Site and no Shrine; we have
/// no religion, 470 Faith and no Gold.
fn g425_turn_88() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(4, 40, 24, 372_425, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for x in [2, 8, 14, 20, 26] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let caracas = cities[4];
    for cid in &cities[..2] {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Orthodoxy".into(), 100.0);
    }
    g.cities
        .get_mut(&caracas)
        .unwrap()
        .pressure
        .insert("Hinduism".into(), 100.0);
    install_test_district(&mut g, caracas, "holy_site");
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[1].religion = Some("Orthodoxy".into());
    g.players[2].religion = Some("Hinduism".into());
    for (pid, faith) in [(1, "Orthodoxy"), (3, "Orthodoxy"), (2, "Hinduism")] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, faith.into());
    }
    g.players[0].faith = 470.0;
    g.players[0].gold = 0.0;
    g.current = 0;
    g.turn = 88;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!g.civ_follows_religion(0, "Orthodoxy"));
    (g, ai, caracas)
}

fn has_shrine(g: &Game, cid: u32) -> bool {
    g.cities[&cid]
        .buildings
        .iter()
        .any(|building| building == "shrine")
}

/// Caracas is the one sanctuary while Orthodoxy presses us, and the whole
/// Faith bank is held from the other sinks; off, neither.
#[test]
fn the_one_sanctuary_and_the_held_bank() {
    let (g, mut ai, caracas) = g425_turn_88();
    assert_eq!(ai.counterweight_shrine_city(&g, 0), None, "off");
    assert_eq!(ai.counterweight_bank_held(&g, 0), 0.0, "off");

    ai.enable_counterweight_finishes_one_shrine();
    assert_eq!(
        ai.rival_faith_presses_us(&g, 0).as_deref(),
        Some("Orthodoxy")
    );
    assert_eq!(ai.counterweight_shrine_city(&g, 0), Some(caracas));
    assert_eq!(ai.counterweight_bank_held(&g, 0), 470.0);

    // No city of ours on a rival faith: nothing presses, nothing is held.
    let (mut quiet, mut ai, _) = g425_turn_88();
    ai.enable_counterweight_finishes_one_shrine();
    for cid in quiet.player_city_ids(0) {
        quiet.cities.get_mut(&cid).unwrap().pressure.clear();
    }
    assert!(ai.rival_faith_presses_us(&quiet, 0).is_none());
    assert_eq!(ai.counterweight_bank_held(&quiet, 0), 0.0);
}

/// G425: Caracas's queue held a Trebuchet every turn. The sanctuary still
/// names its Shrine there, the reservation queues it, and the siege
/// reservation must leave the city alone; a threatened Caracas is not taken.
#[test]
fn the_shrine_takes_a_held_queue_and_the_siege_leaves_it() {
    let (mut g, mut ai, caracas) = g425_turn_88();
    ai.enable_sanctuary_yields_a_held_queue();
    let trebuchet = Item::Unit {
        unit: crate::name!("trebuchet"),
    };
    g.cities.get_mut(&caracas).unwrap().queue = vec![trebuchet];
    ai.enable_counterweight_finishes_one_shrine();
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    assert_eq!(
        ai.adopted_faith_sanctuary_choice(&g, 0, None),
        Some((caracas, shrine.clone()))
    );
    assert_ne!(
        ai.adopted_faith_sanctuary_choice(&g, 0, Some(caracas))
            .map(|(cid, _)| cid),
        Some(caracas),
        "a threatened Caracas is not taken"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    assert_eq!(g.cities[&caracas].queue.first(), Some(&shrine));
    assert!(ai.counterweight_shrine_held(&g, 0, caracas));
}

/// With the treasury covering it the Shrine is bought outright.
#[test]
fn a_covered_shrine_is_bought() {
    let (mut g, mut ai, caracas) = g425_turn_88();
    ai.enable_counterweight_finishes_one_shrine();
    let price = g
        .building_gold_purchase_cost(0, caracas, "shrine")
        .expect("Caracas can buy its Shrine");
    g.players[0].gold = price;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    assert!(
        has_shrine(&g, caracas),
        "bought: {:?}",
        g.cities[&caracas].buildings
    );
    assert!(g.players[0].gold < price);
    assert_eq!(
        ai.counterweight_shrine_city(&g, 0),
        None,
        "the Shrine stands"
    );
}
