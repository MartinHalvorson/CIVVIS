use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn opening() -> (Game, u32) {
    let mut g = Game::new_full(6, 40, 24, 936_531, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let home = g.found_city_for(0, (5, 8), None);
    g.found_city_for(0, (5, 14), None);
    g.found_city_for(1, (27, 10), None);
    let site = (6, 8);
    g.cities
        .get_mut(&home)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), site);
    let tile = g.map.tiles.get_mut(&site).unwrap();
    tile.owner_city = Some(home);
    tile.district = Some(crate::name!("holy_site"));
    tile.pillaged = false;
    g.players[0].techs.insert(crate::name!("astrology"));
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 3;
        city.queue.clear();
    }
    g.current = 0;
    g.turn = 17;
    (g, home)
}

fn treatment() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.enable_religion_seat_earns_its_prophet();
    ai
}

#[test]
fn prophet_queue_is_independently_opt_in() {
    opt_in_off_in_both_controllers("religion-seat-earns-its-prophet", |ai| {
        ai.religion_seat_earns_its_prophet
    });
}

#[test]
fn the_full_opening_turn_keeps_the_completed_site_in_the_race() {
    let (g, home) = opening();
    let mut off = g.clone();
    AdvancedAi::targeting(VictoryTarget::Religion).take_turn(&mut off, 0);
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    assert_ne!(off.cities[&home].queue.first(), Some(&shrine));

    let mut on = g;
    treatment().take_turn(&mut on, 0);
    assert_eq!(on.cities[&home].queue.first(), Some(&shrine));
}

#[test]
fn a_shrine_hands_its_idle_queue_to_prayers() {
    let (mut g, home) = opening();
    g.cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    let ai = treatment();
    let mut plan = ai.assess(&g, 0);
    plan.threatened_city = None;
    ai.claim_religion_prophet_queue(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&home].queue.first(),
        Some(&Item::Project {
            project: crate::name!("holy_site_prayers"),
        })
    );
}

#[test]
fn the_full_turn_keeps_prayers_ahead_of_the_settler_handoff() {
    let (mut g, home) = opening();
    g.cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    let mut ai = treatment();
    ai.plan_city_target = true;
    ai.take_turn(&mut g, 0);
    assert_eq!(
        g.cities[&home].queue.first(),
        Some(&Item::Project {
            project: crate::name!("holy_site_prayers"),
        }),
        "the later governor must retain the Prophet project"
    );
}

#[test]
fn only_the_open_races_prayers_are_protected_from_settlement() {
    let (mut g, home) = opening();
    let prayers = Item::Project {
        project: crate::name!("holy_site_prayers"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: prayers.clone(),
        },
    )
    .unwrap();
    let mut on = treatment();
    on.plan_city_target = true;
    let mut plan = on.assess(&g, 0);
    plan.strategy = GrandStrategy::Religion;
    plan.desired_cities = 6;
    let mut off = AdvancedAi::targeting(VictoryTarget::Religion);
    off.plan_city_target = true;
    let mut baseline = g.clone();
    off.redirect_repeatable_projects_for_settlement_gap(&mut baseline, 0, &plan);
    assert_eq!(
        baseline.cities[&home].queue.first(),
        Some(&Item::Unit {
            unit: crate::name!("settler"),
        }),
        "the fixture reaches the actual Settler handoff"
    );
    on.redirect_repeatable_projects_for_settlement_gap(&mut g, 0, &plan);
    assert_eq!(g.cities[&home].queue.first(), Some(&prayers));

    g.players[0].live_great_person_exhausted = Some(["prophet".to_string()].into_iter().collect());
    on.redirect_repeatable_projects_for_settlement_gap(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&home].queue, baseline.cities[&home].queue,
        "a host-closed race releases the project to normal expansion"
    );
}

#[test]
fn the_race_cannot_take_a_busy_or_threatened_queue() {
    let (g, home) = opening();
    let ai = treatment();
    for condition in 0..3 {
        let mut board = g.clone();
        let mut plan = ai.assess(&board, 0);
        plan.threatened_city = None;
        match condition {
            0 => board.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
                unit: crate::name!("archer"),
            }),
            1 => plan.threatened_city = Some(home),
            _ => board.cities.get_mut(&home).unwrap().last_attacked = board.turn - 1,
        }
        let before = board.cities[&home].queue.clone();
        ai.claim_religion_prophet_queue(&mut board, 0, &plan);
        assert_eq!(board.cities[&home].queue, before);
    }
}

#[test]
fn a_founded_or_closed_race_and_other_targets_keep_their_queues() {
    let (g, home) = opening();
    for condition in 0..5 {
        let mut board = g.clone();
        let mut ai = treatment();
        match condition {
            0 => board.players[0].religion = Some("Buddhism".into()),
            1 => board.players[0].prophet_pending = true,
            2 => {
                board.players[0].live_great_person_exhausted =
                    Some(["prophet".to_string()].into_iter().collect())
            }
            3 => ai.retarget(VictoryTarget::Culture),
            _ => {
                board.cities.get_mut(&home).unwrap().districts.clear();
            }
        }
        let plan = ai.assess(&board, 0);
        ai.claim_religion_prophet_queue(&mut board, 0, &plan);
        assert!(board.cities[&home].queue.is_empty());
    }
}
