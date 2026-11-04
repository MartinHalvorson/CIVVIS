use super::*;
use crate::game::install_test_district;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn at(x: i32, y: i32) -> Pos {
    crate::hex::offset_to_axial(x, y)
}

fn soldier() -> Item {
    Item::Unit {
        unit: crate::name!("heavy_chariot"),
    }
}

fn fixture(n_cities: usize) -> (Game, Vec<u32>, Vec<u32>, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 26, 441_300, 500, 0, true);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.players[0].civ = "Gran Colombia".to_string();
    let cities: Vec<_> = [at(8, 8), at(14, 8), at(20, 8)]
        .into_iter()
        .take(n_cities)
        .map(|pos| g.found_city_for(0, pos, None))
        .collect();
    g.found_city_for(1, at(34, 20), None);
    g.game_speed = GameSpeed::Standard;
    g.turn = 106;
    g.current = 0;
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    for tech in [
        "pottery",
        "writing",
        "wheel",
        "archery",
        "masonry",
        "engineering",
    ] {
        g.players[0].techs.insert(Name::new(tech));
    }
    for cid in &cities {
        let city = g.cities.get_mut(cid).unwrap();
        city.pop = 4;
        city.production = 0.0;
        city.queue = vec![soldier()];
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 8.0,
                ..Default::default()
            },
        );
        Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(*cid, 3);
        assert!(BasicAi::first_campus_item(&g, 0, *cid).is_some());
        assert!(g.item_cost_for_city(0, *cid, &soldier()) > g.city_yields(*cid).production);
    }
    let units = (0..6)
        .map(|i| g.spawn_test_unit("warrior", 0, at(8 + i, 10)))
        .collect();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cities, units, plan)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot_2();
    ai.enable_first_campus_between_the_units();
    ai
}

fn queued_campuses(g: &Game, cities: &[u32]) -> Vec<u32> {
    cities
        .iter()
        .copied()
        .filter(|id| {
            matches!(g.cities[id].queue.first(), Some(Item::District { district, .. })
                if g.district_family(*district) == "campus")
        })
        .collect()
}

#[test]
fn first_campus_between_units_changes_one_fresh_queue_and_withholds_cleanly() {
    let (g, cities, _, plan) = fixture(3);
    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot_2();
    let mut off = g.clone();
    ai.claim_free_slot_campus(&mut off, 0, &plan);
    assert!(queued_campuses(&off, &cities).is_empty());

    ai.enable_first_campus_between_the_units();
    let mut on = g.clone();
    ai.claim_free_slot_campus(&mut on, 0, &plan);
    assert_eq!(queued_campuses(&on, &cities).len(), 1);
    assert_eq!(
        cities
            .iter()
            .filter(|id| on.cities[*id].queue.first() == Some(&soldier()))
            .count(),
        2,
        "the other producers continue their military queues"
    );
    assert_eq!(on.player_unit_ids(0).len(), g.player_unit_ids(0).len());

    ai.disable_first_campus_between_the_units();
    let mut withheld = g;
    ai.claim_free_slot_campus(&mut withheld, 0, &plan);
    assert!(queued_campuses(&withheld, &cities).is_empty());
}

#[test]
fn first_campus_between_units_keeps_invested_and_one_turn_units() {
    for finishes_this_turn in [false, true] {
        let (mut g, cities, _, plan) = fixture(3);
        for cid in &cities {
            if finishes_this_turn {
                let cost = g.item_cost_for_city(0, *cid, &soldier());
                Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
                    *cid,
                    Yields {
                        production: cost + 10.0,
                        ..Default::default()
                    },
                );
                assert!(g.city_yields(*cid).production >= cost);
            } else {
                g.cities.get_mut(cid).unwrap().production = 1.0;
            }
        }
        armed().claim_free_slot_campus(&mut g, 0, &plan);
        assert!(queued_campuses(&g, &cities).is_empty());
    }
}

#[test]
fn first_campus_between_units_needs_three_cities_and_healthy_land_army() {
    let (mut small, cities, _, plan) = fixture(2);
    armed().claim_free_slot_campus(&mut small, 0, &plan);
    assert!(queued_campuses(&small, &cities).is_empty());

    for replacement in ["wounded", "scout", "galley"] {
        let (mut g, cities, units, plan) = fixture(3);
        g.remove_unit(units[5]);
        let uid = match replacement {
            "wounded" => {
                let id = g.spawn_test_unit("warrior", 0, at(12, 11));
                g.units.get_mut(&id).unwrap().hp = 49;
                id
            }
            "galley" => {
                g.map.tiles.get_mut(&at(30, 4)).unwrap().terrain = crate::name!("coast");
                g.spawn_test_unit("galley", 0, at(30, 4))
            }
            _ => g.spawn_test_unit("scout", 0, at(12, 11)),
        };
        armed().claim_free_slot_campus(&mut g, 0, &plan);
        assert!(queued_campuses(&g, &cities).is_empty(), "{replacement}");
        if replacement == "wounded" {
            g.units.get_mut(&uid).unwrap().hp = 50;
            armed().claim_free_slot_campus(&mut g, 0, &plan);
            assert_eq!(queued_campuses(&g, &cities).len(), 1);
        }
    }
}

#[test]
fn first_campus_between_units_stops_for_a_standing_or_queued_campus() {
    for queued in [false, true] {
        let (mut g, cities, _, plan) = fixture(3);
        if queued {
            let campus = BasicAi::first_campus_item(&g, 0, cities[1]).unwrap();
            g.cities.get_mut(&cities[1]).unwrap().queue = vec![campus];
        } else {
            install_test_district(&mut g, cities[1], "campus");
        }
        let before = queued_campuses(&g, &cities);
        armed().claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(queued_campuses(&g, &cities), before);
    }
}

#[test]
fn first_campus_between_units_retains_threat_recent_attack_and_siege_guards() {
    for hold in ["threat", "attacked", "siege"] {
        let (mut g, cities, _, mut plan) = fixture(3);
        // Only the first city's military queue could yield; Walls remain
        // protected in the other two cities.
        for cid in &cities[1..] {
            g.cities.get_mut(cid).unwrap().queue = vec![Item::Building {
                building: crate::name!("walls"),
            }];
        }
        match hold {
            "threat" => plan.threatened_city = Some(cities[0]),
            "attacked" => g.cities.get_mut(&cities[0]).unwrap().last_attacked = g.turn,
            _ => {
                g.cities.get_mut(&cities[0]).unwrap().queue = vec![Item::Unit {
                    unit: crate::name!("catapult"),
                }];
            }
        }
        armed().claim_free_slot_campus(&mut g, 0, &plan);
        assert!(queued_campuses(&g, &cities).is_empty(), "{hold}");
    }
}

#[test]
fn first_campus_between_units_does_not_arm_the_parent_claim() {
    let (mut g, cities, _, plan) = fixture(3);
    let mut ai = AdvancedAi::new();
    ai.enable_first_campus_between_the_units();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert!(queued_campuses(&g, &cities).is_empty());
}

#[test]
fn first_campus_between_units_yields_to_the_final_confirmed_siege_handoff() {
    for siege in [false, true] {
        let (mut g, cities, _, plan) = fixture(3);
        for cid in &cities[1..] {
            g.cities.get_mut(cid).unwrap().queue = vec![Item::Building {
                building: crate::name!("walls"),
            }];
        }
        if siege {
            for pos in [at(8, 9), at(9, 8), at(7, 8)] {
                let id = g.spawn_test_unit("knight", 1, pos);
                assert!(g.unit_visible_to(id, 0));
            }
        }
        let mut ai = armed();
        ai.base.garrison_under_fire = true;
        ai.victory_planning = true;
        ai.victory_target = Some(VictoryTarget::Domination);
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(queued_campuses(&g, &cities), vec![cities[0]]);
        assert_eq!(g.cities[&cities[0]].hp, CITY_MAX_HP);
        let threat = siege.then_some(cities[0]);
        let defence = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, threat);
        if siege {
            assert!(defence.is_some());
            assert!(queued_campuses(&g, &cities).is_empty());
        } else {
            assert!(defence.is_none());
            assert_eq!(queued_campuses(&g, &cities), vec![cities[0]]);
        }
    }
}
