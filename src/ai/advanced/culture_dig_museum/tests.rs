use super::*;
use crate::game::{Action, Item};

fn culture_seat_with_a_city(enabled: bool) -> (AdvancedAi, Game, u32) {
    let mut game = Game::new_full(2, 30, 16, 79_104, 200, 0, false);
    let position = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(tile.pos).is_empty()
        })
        .map(|tile| tile.pos)
        .expect("a land tile");
    let settler = game.spawn_test_unit("settler", 0, position);
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.city_at(position).unwrap();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_digs_one_museum();
    }
    (ai, game, city)
}

#[test]
fn the_first_museum_is_the_dig_museum() {
    let (ai, game, _) = culture_seat_with_a_city(true);
    let dig = crate::name!("archaeological_museum");
    let art = crate::name!("art_museum");

    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), DIG_MUSEUM_BONUS);
    assert_eq!(ai.dig_museum_bonus(&game, 0, art), 0.0);
}

#[test]
fn one_dig_museum_built_or_queued_ends_the_bonus() {
    let dig = crate::name!("archaeological_museum");

    let (ai, mut game, city) = culture_seat_with_a_city(true);
    game.cities
        .get_mut(&city)
        .unwrap()
        .queue
        .push(Item::Building { building: dig });
    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), 0.0, "queued");

    let (ai, mut game, city) = culture_seat_with_a_city(true);
    game.cities.get_mut(&city).unwrap().buildings.push(dig);
    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), 0.0, "built");
}

#[test]
fn off_or_another_lane_adds_nothing() {
    let dig = crate::name!("archaeological_museum");

    let (ai, game, _) = culture_seat_with_a_city(false);
    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), 0.0, "gene off");

    let (_, game, _) = culture_seat_with_a_city(true);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_digs_one_museum();
    assert_eq!(science.dig_museum_bonus(&game, 0, dig), 0.0, "science seat");
}

#[test]
fn the_delegated_governor_lends_the_flag_only_to_a_culture_seat() {
    let (ai, game, _) = culture_seat_with_a_city(true);
    assert!(ai.dig_museum_lane_active(&game));

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_digs_one_museum();
    assert!(!science.dig_museum_lane_active(&game));
}

fn digger_fixture() -> (AdvancedAi, Game, Vec<u32>) {
    let (mut game, cities) =
        crate::ai::industrial_zones::tests::producer_empire("DIGGERV2", 435_600, 2);
    game.players[0]
        .civics
        .insert(crate::name!("natural_history"));
    game.players[0].civics.insert(crate::name!("humanism"));
    for cid in &cities {
        let city = game.cities.get_mut(cid).unwrap();
        city.queue.clear();
        city.buildings.push(crate::name!("archaeological_museum"));
    }
    for tile in game.map.tiles.values_mut() {
        if matches!(
            tile.resource.as_deref(),
            Some("antiquity_site" | "shipwreck")
        ) {
            tile.resource = None;
        }
    }
    let site = game.cities[&cities[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != game.cities[&cities[0]].pos)
        .unwrap();
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = Some(crate::name!("antiquity_site"));
    tile.improvement = None;
    tile.district = None;
    tile.district_foundation = None;
    tile.wonder = None;
    tile.flooded = false;
    tile.submerged = false;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_digs_one_museum_2();
    let item = Item::Unit {
        unit: crate::name!("archaeologist"),
    };
    assert!(
        game.can_produce(0, cities[0], &item),
        "fixture offers the digger"
    );
    assert!(!game.excavation_sites(0).is_empty(), "fixture offers a dig");
    (ai, game, cities)
}

#[test]
fn version_two_claims_one_idle_museum_and_keeps_its_investment() {
    let (ai, mut game, cities) = digger_fixture();
    let plan = ai.assess(&game, 0);
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), Some(cities[0]));
    ai.claim_culture_digger(&mut game, 0, &plan);
    let item = Item::Unit {
        unit: crate::name!("archaeologist"),
    };
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&item));
    assert!(ai.culture_digger_holds(&game, &item));
    ai.claim_culture_digger(&mut game, 0, &plan);
    assert_eq!(
        game.cities
            .values()
            .filter(|c| c.queue.first() == Some(&item))
            .count(),
        1
    );
}

#[test]
fn original_and_off_keep_their_original_museum_behavior() {
    let (_, mut game, cities) = digger_fixture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let plan = ai.assess(&game, 0);
    ai.claim_culture_digger(&mut game, 0, &plan);
    assert!(game.cities[&cities[0]].queue.is_empty());
    ai.enable_culture_digs_one_museum();
    assert!(ai.dig_museum_lane_active(&game));
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    ai.enable_culture_digs_one_museum_2();
    assert!(!ai.culture_digs_one_museum && ai.culture_digs_one_museum_2);
    ai.enable_culture_digs_one_museum();
    assert!(ai.culture_digs_one_museum && !ai.culture_digs_one_museum_2);
}

#[test]
fn a_live_or_queued_digger_prevents_a_duplicate() {
    let (ai, mut game, cities) = digger_fixture();
    let plan = ai.assess(&game, 0);
    let unit = game.spawn_test_unit("archaeologist", 0, game.cities[&cities[0]].pos);
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    game.remove_unit(unit);
    game.cities
        .get_mut(&cities[5])
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: crate::name!("archaeologist"),
        });
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
}

#[test]
fn a_busy_or_threatened_museum_keeps_its_queue() {
    let (ai, mut game, cities) = digger_fixture();
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().queue.push(Item::Unit {
            unit: crate::name!("builder"),
        });
    }
    let mut plan = ai.assess(&game, 0);
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    game.cities.get_mut(&cities[0]).unwrap().queue.clear();
    plan.threatened_city = Some(cities[0]);
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    plan.threatened_city = None;
    game.cities.get_mut(&cities[0]).unwrap().last_attacked = game.turn - 1;
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
}

#[test]
fn no_owned_land_dig_or_locked_unit_means_no_digger() {
    let (ai, mut game, _) = digger_fixture();
    let plan = ai.assess(&game, 0);
    game.players[0]
        .civics
        .remove(&crate::name!("natural_history"));
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    game.players[0]
        .civics
        .insert(crate::name!("natural_history"));
    for tile in game.map.tiles.values_mut() {
        if tile.resource.as_deref() == Some("antiquity_site") {
            tile.owner_city = None;
        }
    }
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
}

#[test]
fn another_lane_or_a_lost_culture_lane_does_not_claim() {
    let (_, game, _) = digger_fixture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_culture_digs_one_museum_2();
    assert_eq!(ai.culture_digger_city(&game, 0, &ai.assess(&game, 0)), None);
    let (mut ai, game, _) = digger_fixture();
    ai.lane_lost = true;
    assert_eq!(ai.culture_digger_city(&game, 0, &ai.assess(&game, 0)), None);
}

#[test]
fn too_late_to_build_and_use_it_does_not_claim() {
    let (ai, mut game, _) = digger_fixture();
    game.turn = game.max_turns - 2;
    assert_eq!(ai.culture_digger_city(&game, 0, &ai.assess(&game, 0)), None);
}

#[test]
fn war_recovery_and_missing_museums_leave_the_queue_for_other_needs() {
    let (ai, mut game, _) = digger_fixture();
    let mut plan = ai.assess(&game, 0);
    game.at_war.insert((0, 1));
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    game.at_war.clear();
    plan.strategy = GrandStrategy::Recovery;
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
    plan.strategy = GrandStrategy::Culture;
    for city in game.cities.values_mut() {
        city.buildings.retain(|b| b != "archaeological_museum");
    }
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), None);
}

#[test]
fn both_production_governors_keep_the_claimed_digger() {
    let (mut ai, mut game, cities) = digger_fixture();
    let plan = ai.assess(&game, 0);
    ai.claim_culture_digger(&mut game, 0, &plan);
    let digger = Item::Unit {
        unit: crate::name!("archaeologist"),
    };
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&digger));
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&digger));
    ai.delegated_cities(&mut game, 0, &plan);
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&digger));
}

#[test]
fn the_host_quote_controls_the_build_budget() {
    let (ai, mut game, cities) = digger_fixture();
    let item = Item::Unit {
        unit: crate::name!("archaeologist"),
    };
    let key = Game::production_block_key(&item);
    for cid in &cities {
        std::sync::Arc::make_mut(&mut game.host_buildable).insert(
            *cid,
            [(
                key.clone(),
                crate::game::HostMenuEntry {
                    cost: Some(200.0),
                    turns: Some(100.0),
                },
            )]
            .into_iter()
            .collect(),
        );
    }
    let plan = ai.assess(&game, 0);
    assert_eq!(
        ai.culture_digger_city(&game, 0, &plan),
        None,
        "model production cannot overrule a slow host quote"
    );
    std::sync::Arc::make_mut(&mut game.host_buildable)
        .get_mut(&cities[0])
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .turns = Some(8.0);
    assert_eq!(ai.culture_digger_city(&game, 0, &plan), Some(cities[0]));
}
