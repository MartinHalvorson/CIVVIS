use super::super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 364300, 500, 0, false);
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let home = g.found_city_for(0, (8, 12), None);
    let target = g.found_city_for(1, (16, 12), None);
    g.cities.get_mut(&home).unwrap().pop = 6;
    g.cities.get_mut(&home).unwrap().buildings.extend([
        crate::name!("granary"),
        crate::name!("monument"),
        crate::name!("water_mill"),
    ]);
    g.cities.get_mut(&target).unwrap().wall_hp = 300;
    g.cities.get_mut(&target).unwrap().buildings.extend([
        crate::name!("walls"),
        crate::name!("medieval_walls"),
        crate::name!("renaissance_walls"),
    ]);
    let ancestors = g.rules.tech_ancestors["metal_casting"].clone();
    g.players[0]
        .techs
        .extend(ancestors.iter().map(|name| Name::new(name)));
    g.players[0].techs.insert(crate::name!("metal_casting"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 25.0);
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = 1.0;
    for kind in [
        "warrior", "warrior", "archer", "archer", "catapult", "catapult",
    ] {
        g.spawn_test_unit(kind, 0, (8, 12));
    }
    g.spawn_test_unit("builder", 0, (8, 12));
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    g.turn = 132;
    g.current = 0;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, home, target)
}

fn missing(g: &Game, ai: &AdvancedAi, plan: &StrategicPlan, home: u32) -> bool {
    ai.missing_domination_siege(
        g,
        0,
        home,
        plan,
        &ai.counts(g, 0),
        &g.rules.units["bombard"],
    )
}

#[test]
fn obsolete_catapults_do_not_close_the_modern_wall_breaker_reserve() {
    let (mut g, ai, plan, home, _) = fixture();
    let bombard = Item::Unit {
        unit: crate::name!("bombard"),
    };
    assert!(g.can_produce(0, home, &bombard));
    assert!(g.unit_is_obsolete(0, crate::name!("catapult")));
    assert!(missing(&g, &ai, &plan, home));
    assert!(ai.production_value(&g, 0, home, &bombard, &plan, &ai.counts(&g, 0)) > 0.0);
    // Closing the missing role restores the ordinary army ceiling.
    g.spawn_test_unit("bombard", 0, (8, 12));
    assert_eq!(
        ai.production_value(&g, 0, home, &bombard, &plan, &ai.counts(&g, 0)),
        -2_000.0
    );
}

#[test]
fn a_current_field_or_queued_weapon_closes_the_reserve() {
    for queued in [false, true] {
        let (mut g, ai, plan, home, _) = fixture();
        if queued {
            g.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
                unit: crate::name!("bombard"),
            });
        } else {
            g.spawn_test_unit("bombard", 0, (8, 12));
        }
        assert!(!missing(&g, &ai, &plan, home));
    }
}

#[test]
fn the_modern_order_retains_its_reserve_but_other_cities_do_not_duplicate_it() {
    let (mut g, mut ai, mut plan, home, _) = fixture();
    let other = g.found_city_for(0, (5, 12), None);
    let bombard = Item::Unit {
        unit: crate::name!("bombard"),
    };
    g.cities.get_mut(&home).unwrap().queue.push(bombard.clone());
    assert!(ai.missing_domination_siege(
        &g,
        0,
        home,
        &plan,
        &ai.counts_without_city_queue(&g, 0, home),
        &g.rules.units["bombard"]
    ));
    assert!(!missing(&g, &ai, &plan, other));
    plan.target_city = None;
    plan.strategy = GrandStrategy::Recovery;
    // A started useful replacement survives the governor's investment check.
    // Zero-progress orders still compete with other production priorities.
    g.cities.get_mut(&home).unwrap().production = 1.0;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&bombard));
}

#[test]
fn strong_existing_formations_and_wounded_modern_weapons_still_count() {
    for formed in [true, false] {
        let (mut g, ai, plan, home, _) = fixture();
        if formed {
            let id = g
                .units
                .values()
                .find(|u| u.owner == 0 && u.kind == "catapult")
                .unwrap()
                .id;
            g.units.get_mut(&id).unwrap().formation = 2;
        } else {
            g.spawn_test_unit("bombard", 0, (8, 12));
            let id = g
                .units
                .values()
                .find(|u| u.owner == 0 && u.kind == "bombard")
                .unwrap()
                .id;
            g.units.get_mut(&id).unwrap().hp = 10;
        }
        assert!(!missing(&g, &ai, &plan, home));
    }
}

#[test]
fn modernization_does_not_expand_non_domination_peace_or_unwalled_armies() {
    for case in 0..3 {
        let (mut g, mut ai, plan, home, target) = fixture();
        match case {
            0 => ai.retarget(VictoryTarget::Science),
            1 => g.at_war.clear(),
            _ => g.cities.get_mut(&target).unwrap().wall_hp = 0,
        }
        assert!(!missing(&g, &ai, &plan, home));
    }
}

#[test]
fn a_queued_old_army_already_provides_comparable_siege_power() {
    let (mut g, ai, plan, home, _) = fixture();
    g.cities
        .get_mut(&home)
        .unwrap()
        .queue
        .push(Item::Formation {
            unit: crate::name!("catapult"),
            formation: 2,
        });
    assert!(!missing(&g, &ai, &plan, home));
}

fn field_cannon_without_niter_case() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let (mut g, ai, plan, home, _) = fixture();
    let guns: Vec<_> = g
        .units
        .values()
        .filter(|u| g.rules.units[u.kind].siege)
        .map(|u| u.id)
        .collect();
    for uid in guns {
        g.remove_unit(uid);
    }
    g.players[0].strategic_resources.clear();
    for tech in ["ballistics", "military_engineering"] {
        let ancestors = g.rules.tech_ancestors[tech].clone();
        g.players[0]
            .techs
            .extend(ancestors.iter().map(|name| Name::new(name)));
        g.players[0].techs.insert(Name::new(tech));
    }
    (g, ai, plan, home)
}

#[test]
fn field_cannon_does_not_veto_the_only_available_wall_breaker() {
    let (mut g, mut ai, plan, home) = field_cannon_without_niter_case();
    let weapon = Item::Unit {
        unit: crate::name!("trebuchet"),
    };
    assert!(g.can_produce(0, home, &weapon));
    assert!(g.can_produce(
        0,
        home,
        &Item::Unit {
            unit: crate::name!("field_cannon")
        }
    ));
    assert!(!g.can_produce(
        0,
        home,
        &Item::Unit {
            unit: crate::name!("bombard")
        }
    ));
    let counts = ai.counts(&g, 0);
    assert_eq!(counts.siege, 0);
    assert!(
        ai.production_value(&g, 0, home, &weapon, &plan, &counts) > 0.0,
        "a Field Cannon cannot replace the missing bombardment role"
    );
    // Infrastructure can still win an idle city's production comparison.
    // Once the campaign's weapon starts, ordinary ranged fire must not make
    // the governor abandon that commitment as obsolete.
    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: weapon.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&home).unwrap().production = 1.0;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&weapon));
}

#[test]
fn available_bombard_still_replaces_weaker_siege_production() {
    let (mut g, ai, plan, home) = field_cannon_without_niter_case();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 25.0);
    assert!(g.can_produce(
        0,
        home,
        &Item::Unit {
            unit: crate::name!("bombard")
        }
    ));
    let counts = ai.counts(&g, 0);
    assert_eq!(
        ai.production_value(
            &g,
            0,
            home,
            &Item::Unit {
                unit: crate::name!("trebuchet")
            },
            &plan,
            &counts
        ),
        -2_000.0
    );
    assert!(
        ai.production_value(
            &g,
            0,
            home,
            &Item::Unit {
                unit: crate::name!("bombard")
            },
            &plan,
            &counts
        ) > 0.0
    );
}

#[test]
fn an_existing_wall_breaker_closes_the_exception_to_field_fire_comparison() {
    let (mut g, ai, plan, home) = field_cannon_without_niter_case();
    g.spawn_test_unit("trebuchet", 0, g.cities[&home].pos);
    let counts = ai.counts(&g, 0);
    assert_eq!(counts.siege, 1);
    assert_eq!(
        ai.production_value(
            &g,
            0,
            home,
            &Item::Unit {
                unit: crate::name!("trebuchet")
            },
            &plan,
            &counts
        ),
        -2_000.0
    );
}
