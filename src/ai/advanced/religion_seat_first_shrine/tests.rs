use super::*;
use crate::game::install_test_district;

fn plan_for(g: &Game, ai: &AdvancedAi) -> StrategicPlan {
    ai.assess(g, 0)
}

/// One founder city with a Holy Site, grassland around it and a Settler
/// well short of done at the head of its queue.
fn founder(target: VictoryTarget) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 60, 24, 936_312, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 8), None);
    let home = g.player_city_ids(0)[0];
    install_test_district(&mut g, home, "holy_site");
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].techs.insert(crate::name!("astrology"));
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Buddhism".into(), 500.0);
    g.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    g.current = 0;
    g.turn = 40;
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_religion_seat_first_shrine();
    (g, ai, home)
}

fn shrine() -> Item {
    Item::Building {
        building: crate::name!("shrine"),
    }
}

#[test]
fn a_founder_puts_its_first_shrine_ahead_of_a_settler() {
    let (mut g, ai, home) = founder(VictoryTarget::Religion);
    let plan = plan_for(&g, &ai);
    ai.claim_religion_seat_first_shrine(&mut g, 0, &plan);
    assert_eq!(g.cities[&home].queue.first(), Some(&shrine()));
}

#[test]
fn nothing_changes_off_or_for_another_target_or_before_founding() {
    let (mut off, _, home) = founder(VictoryTarget::Religion);
    let stock = AdvancedAi::targeting(VictoryTarget::Religion);
    let plan = plan_for(&off, &stock);
    stock.claim_religion_seat_first_shrine(&mut off, 0, &plan);
    assert_ne!(off.cities[&home].queue.first(), Some(&shrine()));

    let (mut science, ai, home) = founder(VictoryTarget::Science);
    let plan = plan_for(&science, &ai);
    ai.claim_religion_seat_first_shrine(&mut science, 0, &plan);
    assert_ne!(science.cities[&home].queue.first(), Some(&shrine()));

    let (mut unfounded, ai, home) = founder(VictoryTarget::Religion);
    unfounded.players[0].religion = None;
    let plan = plan_for(&unfounded, &ai);
    ai.claim_religion_seat_first_shrine(&mut unfounded, 0, &plan);
    assert_ne!(unfounded.cities[&home].queue.first(), Some(&shrine()));
}

/// The state the missing Missionaries let happen: a rival faith holds the
/// Holy Site city, and it still takes the Shrine.
#[test]
fn a_city_converted_by_a_rival_still_takes_it() {
    let (mut g, ai, home) = founder(VictoryTarget::Religion);
    let city = g.cities.get_mut(&home).unwrap();
    city.pressure.clear();
    city.pressure.insert("Islam".into(), 900.0);
    let plan = plan_for(&g, &ai);
    assert_ne!(g.city_religion(&g.cities[&home]), Some("Buddhism"));
    ai.claim_religion_seat_first_shrine(&mut g, 0, &plan);
    assert_eq!(g.cities[&home].queue.first(), Some(&shrine()));
}

#[test]
fn one_shrine_standing_or_queued_ends_the_claim() {
    let (mut standing, ai, home) = founder(VictoryTarget::Religion);
    standing
        .cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    let plan = plan_for(&standing, &ai);
    assert!(ai.first_shrine_choice(&standing, 0, &plan).is_none());

    let (mut queued, ai, home) = founder(VictoryTarget::Religion);
    queued.cities.get_mut(&home).unwrap().queue.push(shrine());
    let plan = plan_for(&queued, &ai);
    assert!(ai.first_shrine_choice(&queued, 0, &plan).is_none());
}

#[test]
fn an_attacked_city_and_walls_keep_their_queue() {
    let (mut attacked, ai, home) = founder(VictoryTarget::Religion);
    attacked.cities.get_mut(&home).unwrap().last_attacked = 39;
    let plan = plan_for(&attacked, &ai);
    assert!(ai.first_shrine_choice(&attacked, 0, &plan).is_none());

    let (mut walled, ai, home) = founder(VictoryTarget::Religion);
    walled.cities.get_mut(&home).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    let plan = plan_for(&walled, &ai);
    assert!(ai.first_shrine_choice(&walled, 0, &plan).is_none());
}

#[test]
fn a_head_finishing_this_turn_keeps_the_queue() {
    let (mut g, ai, home) = founder(VictoryTarget::Religion);
    let head = g.cities[&home].queue[0].clone();
    let cost = g.item_cost_for_city(0, home, &head);
    g.cities.get_mut(&home).unwrap().production = cost - 0.5;
    let plan = plan_for(&g, &ai);
    assert!(
        g.item_invested_production(home, &head) > 0.0,
        "the fixture must really invest in the head"
    );
    assert!(ai.first_shrine_choice(&g, 0, &plan).is_none());
}

fn soldier_at_the_head(g: &mut Game, home: u32) {
    g.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
}

/// Live t24: the Shrine took the queue from a Slinger chosen against nearby
/// barbarians.
#[test]
fn a_soldier_keeps_its_queue_with_hostiles_in_reach() {
    let (mut g, ai, home) = founder(VictoryTarget::Religion);
    soldier_at_the_head(&mut g, home);
    let barbarian = g
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .expect("fixture: the barbarian seat");
    let pos = g.cities[&home].pos;
    let near = g
        .wdisk(pos, 2)
        .into_iter()
        .find(|at| {
            g.wdist(*at, pos) == 2
                && g.map
                    .get(*at)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        })
        .expect("fixture: a dry tile two away");
    g.spawn_unit("warrior", barbarian, near);
    assert!(
        ai.hint_city_in_reach_of_war(&g, 0, home),
        "fixture: in reach"
    );
    let plan = plan_for(&g, &ai);
    assert!(ai.first_shrine_choice(&g, 0, &plan).is_none());
}

#[test]
fn a_soldier_gives_way_in_a_quiet_city() {
    let (mut g, ai, home) = founder(VictoryTarget::Religion);
    soldier_at_the_head(&mut g, home);
    assert!(!ai.hint_city_in_reach_of_war(&g, 0, home), "fixture: quiet");
    let plan = plan_for(&g, &ai);
    assert!(ai.first_shrine_choice(&g, 0, &plan).is_some());
}
