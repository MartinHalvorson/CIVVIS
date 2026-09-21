use super::super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 32, 24, 364500, 500, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.current = pid;
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    g.current = 0;
    g.turn = 110;
    let home = g.player_city_ids(0)[0];
    crate::game::install_test_district(&mut g, home, "holy_site");
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[0].religion = Some("Our Faith".into());
    g.players[0].holy_city = Some(home);
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = 15.0;
    let city = g.cities.get_mut(&home).unwrap();
    city.buildings.extend([
        crate::name!("shrine"),
        crate::name!("granary"),
        crate::name!("monument"),
    ]);
    city.atheist_pressure = 0.0;
    city.pressure.insert("Our Faith".into(), 100.0);
    city.pressure.insert("Rival Faith".into(), 70.0);
    city.queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_founder_temple();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, home)
}

fn temple() -> Item {
    Item::Building {
        building: crate::name!("temple"),
    }
}

#[test]
fn a_relic_slot_does_not_veto_the_first_defensive_temple() {
    let (g, ai, plan, home) = fixture();
    assert_eq!(g.city_religion(&g.cities[&home]), Some("Our Faith"));
    assert!(g.can_produce(0, home, &temple()));
    assert!(!g.rules.buildings["temple"].great_work_slots.is_empty());
    assert!(ai.production_value(&g, 0, home, &temple(), &plan, &ai.counts(&g, 0)) > 0.0);
}

#[test]
fn the_founder_reservation_survives_the_real_governor_without_banked_progress() {
    for strategy in [
        GrandStrategy::Expansion,
        GrandStrategy::Recovery,
        GrandStrategy::Conquest,
    ] {
        let (mut g, mut ai, mut plan, home) = fixture();
        plan.strategy = strategy;
        ai.founder_temple(&mut g, 0);
        assert_eq!(g.cities[&home].queue.first(), Some(&temple()));
        assert_eq!(g.item_invested_production(home, &temple()), 0.0);
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_eq!(
            g.cities[&home].queue.first(),
            Some(&temple()),
            "{strategy:?}"
        );
    }
}

#[test]
fn an_existing_temple_order_does_not_open_another_city_queue() {
    let (mut g, ai, plan, home) = fixture();
    let other = g.found_city_for(0, (3, 3), None);
    crate::game::install_test_district(&mut g, other, "holy_site");
    g.cities.get_mut(&other).unwrap().atheist_pressure = 0.0;
    g.cities
        .get_mut(&other)
        .unwrap()
        .pressure
        .insert("Our Faith".into(), 100.0);
    g.cities
        .get_mut(&other)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    g.cities.get_mut(&home).unwrap().queue = vec![temple()];
    assert!(g.can_produce(0, other, &temple()));
    assert_eq!(
        ai.production_value(&g, 0, other, &temple(), &plan, &ai.counts(&g, 0)),
        -10_000.0
    );
    ai.founder_temple(&mut g, 0);
    assert!(g.cities[&other].queue.is_empty());
    assert_eq!(g.cities[&home].queue.first(), Some(&temple()));
}

#[test]
fn the_exception_does_not_buy_cultural_buildings_or_extra_temples() {
    let (mut g, ai, plan, home) = fixture();
    for building in ["amphitheater", "national_history_museum"] {
        let item = Item::Building {
            building: Name::new(building),
        };
        assert_eq!(
            ai.production_value(&g, 0, home, &item, &plan, &ai.counts(&g, 0)),
            -10_000.0
        );
    }
    let other = g.found_city_for(0, (3, 3), None);
    g.cities
        .get_mut(&other)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));
    assert_eq!(
        ai.production_value(&g, 0, home, &temple(), &plan, &ai.counts(&g, 0)),
        -10_000.0
    );
}

#[test]
fn non_founders_other_lanes_and_safe_religions_keep_their_existing_scores() {
    for case in 0..6 {
        let (mut g, mut ai, plan, home) = fixture();
        match case {
            0 => g.players[0].religion = None,
            1 => ai.retarget(VictoryTarget::Science),
            2 => {
                g.cities
                    .get_mut(&home)
                    .unwrap()
                    .pressure
                    .remove("Rival Faith");
            }
            3 => g.victory_conditions.religious = false,
            4 => {
                g.cities
                    .get_mut(&home)
                    .unwrap()
                    .pressure
                    .insert("Rival Faith".into(), 200.0);
            }
            _ => ai.disable_founder_temple(),
        }
        assert_eq!(
            ai.production_value(&g, 0, home, &temple(), &plan, &ai.counts(&g, 0)),
            -10_000.0
        );
    }
}
