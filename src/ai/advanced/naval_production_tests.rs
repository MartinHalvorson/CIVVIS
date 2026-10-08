use super::*;
use crate::game::HostMenuEntry;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, u32, StrategicPlan) {
    let mut g = Game::new_full(2, 24, 18, 39_980_001, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }
    let city = g.found_city_for(0, (6, 6), None);
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].civics = g.rules.civics.keys().copied().collect();
    g.cities.get_mut(&city).unwrap().queue.clear();
    g.turn = 133;
    g.max_turns = 250;
    g.map.tiles.get_mut(&(7, 6)).unwrap().terrain = crate::name!("lake");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        city,
        plan,
    )
}

#[test]
fn lake_only_cities_refuse_single_hulls_and_formations_even_when_the_host_offers_them() {
    let (mut g, ai, city, plan) = board();
    for item in [
        Item::Unit {
            unit: crate::name!("ironclad"),
        },
        Item::Formation {
            unit: crate::name!("ironclad"),
            formation: 1,
        },
        Item::Formation {
            unit: crate::name!("ironclad"),
            formation: 2,
        },
    ] {
        Arc::make_mut(&mut g.host_buildable)
            .entry(city)
            .or_default()
            .insert(
                Game::production_block_key(&item),
                HostMenuEntry {
                    cost: Some(190.0),
                    turns: Some(1.0),
                },
            );
        assert!(
            g.can_produce(0, city, &item),
            "the native menu permits stranded hulls"
        );
        for threatened in [None, Some(city)] {
            let mut current = plan.clone();
            current.threatened_city = threatened;
            assert_eq!(
                ai.production_value(&g, 0, city, &item, &current, &ai.counts(&g, 0)),
                -10_000.0
            );
        }
    }
    let defender = Item::Unit {
        unit: crate::name!("warrior"),
    };
    assert!(ai.production_value(&g, 0, city, &defender, &plan, &ai.counts(&g, 0)) > -10_000.0);
}

#[test]
fn open_water_and_explicit_withholding_keep_naval_production_available() {
    let (mut g, mut ai, city, plan) = board();
    for item in [
        Item::Unit {
            unit: crate::name!("ironclad"),
        },
        Item::Formation {
            unit: crate::name!("ironclad"),
            formation: 1,
        },
    ] {
        ai.base.open_water_navy = false;
        assert!(ai.production_value(&g, 0, city, &item, &plan, &ai.counts(&g, 0)) > -10_000.0);
        ai.base.open_water_navy = true;
        g.map.tiles.get_mut(&(7, 6)).unwrap().terrain = crate::name!("coast");
        assert!(ai.production_value(&g, 0, city, &item, &plan, &ai.counts(&g, 0)) > -10_000.0);
        g.map.tiles.get_mut(&(7, 6)).unwrap().terrain = crate::name!("lake");
    }
}

#[test]
fn lakes_do_not_raise_open_water_fleet_demand_or_create_a_coastal_war() {
    let (mut g, mut ai, _, _) = board();
    g.map.tiles.get_mut(&(7, 6)).unwrap().terrain = crate::name!("coast");
    let lake_city = g.found_city_for(0, (12, 6), None);
    g.map.tiles.get_mut(&(13, 6)).unwrap().terrain = crate::name!("lake");
    let rival = g.found_city_for(1, (17, 6), None);
    g.map.tiles.get_mut(&(18, 6)).unwrap().terrain = crate::name!("lake");
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    assert_eq!(ai.base.desired_navy(&g, 0), 1);
    assert!(!ai.base.naval_city_can_launch(&g, lake_city));
    assert!(!ai.base.naval_city_can_launch(&g, rival));
    g.map.tiles.get_mut(&(18, 6)).unwrap().terrain = crate::name!("coast");
    assert_eq!(
        ai.base.desired_navy(&g, 0),
        2,
        "a real coastal war still funds its fleet"
    );
    ai.base.open_water_navy = false;
    assert_eq!(
        ai.base.desired_navy(&g, 0),
        3,
        "the withhold retains the old lake-inclusive budget"
    );
}

#[test]
fn inland_cities_can_launch_through_a_completed_open_water_harbor() {
    let (mut g, ai, city, plan) = board();
    let port = (8, 6);
    g.map.tiles.get_mut(&port).unwrap().terrain = crate::name!("coast");
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("harbor"), port);
    assert!(ai.base.naval_city_can_launch(&g, city));
    assert!(ai.base.desired_navy(&g, 0) > 0);
    for item in [
        Item::Unit {
            unit: crate::name!("ironclad"),
        },
        Item::Formation {
            unit: crate::name!("ironclad"),
            formation: 1,
        },
    ] {
        assert!(ai.production_value(&g, 0, city, &item, &plan, &ai.counts(&g, 0)) > -10_000.0);
    }
    g.map.tiles.get_mut(&port).unwrap().terrain = crate::name!("lake");
    assert!(!ai.base.naval_city_can_launch(&g, city));
}
