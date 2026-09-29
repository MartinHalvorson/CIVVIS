use super::super::*;

fn siege_gap_case() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 79_301, 500, 0, false);
    g.current = 0;
    let settlers: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "settler")
        .map(|u| (u.owner, u.pos))
        .collect();
    for (owner, pos) in settlers {
        g.found_city_for(owner, pos, None);
    }
    let home = g.player_city_ids(0)[0];
    let target = g.player_city_ids(1)[0];
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let pos = g.cities[&home].pos;
    for kind in ["warrior", "warrior", "archer", "archer"] {
        g.spawn_unit(kind, 0, pos);
    }
    g.spawn_unit("builder", 0, pos);
    g.players[0]
        .techs
        .extend(["archery", "masonry", "engineering"].map(crate::name::Name::new));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 30.0;
    g.cities.get_mut(&target).unwrap().wall_hp = 200;
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("medieval_walls"));
    g.turn = 125;
    g.at_war.insert((0, 1));
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

#[test]
fn a_full_army_can_add_missing_siege_for_medieval_walls() {
    let (mut g, mut ai, plan, home, _) = siege_gap_case();
    let catapult = Item::Unit {
        unit: crate::name!("catapult"),
    };
    assert!(g.can_produce(0, home, &catapult));
    let counts = ai.counts(&g, 0);
    assert!(ai.production_value(&g, 0, home, &catapult, &plan, &counts) > 0.0);
    for kind in ["warrior", "archer"] {
        assert_eq!(
            ai.production_value(
                &g,
                0,
                home,
                &Item::Unit {
                    unit: crate::name::Name::new(kind)
                },
                &plan,
                &counts
            ),
            -2_000.0
        );
    }
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&catapult));
}

#[test]
fn delegated_domination_reserves_one_real_wall_breaker() {
    let (mut g, mut ai, plan, home, target) = siege_gap_case();
    ai.enable_lane_delegates_production_2();
    assert!(ai.lane_delegates_now(Some(VictoryTarget::Domination), true));
    assert!(ai
        .reserve_delegated_domination_siege(&mut g, 0, &plan)
        .is_some());
    assert_eq!(
        g.cities[&home].queue.first(),
        Some(&Item::Unit {
            unit: crate::name!("catapult"),
        })
    );
    assert!(
        ai.reserve_delegated_domination_siege(&mut g, 0, &plan)
            .is_none(),
        "the queued gun closes the reservation"
    );

    g.cities.get_mut(&home).unwrap().queue.clear();
    let gun = g.spawn_unit("catapult", 0, g.cities[&home].pos);
    assert!(
        ai.reserve_delegated_domination_siege(&mut g, 0, &plan)
            .is_none(),
        "a fielded gun also closes it"
    );
    g.remove_unit(gun);
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    assert!(ai
        .reserve_delegated_domination_siege(&mut g, 0, &plan)
        .is_none());
    g.cities.get_mut(&target).unwrap().wall_hp = 100;
    g.at_war.clear();
    assert!(ai
        .reserve_delegated_domination_siege(&mut g, 0, &plan)
        .is_none());
}

/// On `civvis-20260929T003923Z` turn 146, every productive city had just
/// started a routine queue, so the delegated reservation picked a new
/// one-production city and forecast 144 turns for a bombard. A fresh trader
/// or building costs no production to defer; an invested or military queue
/// still owns its city.
#[test]
fn delegated_siege_uses_a_fast_fresh_queue_before_a_hundred_turn_idle_city() {
    let (mut g, ai, plan, home, target) = siege_gap_case();
    let home_pos = g.cities[&home].pos;
    let target_pos = g.cities[&target].pos;
    let remote_pos = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.wdist(**pos, home_pos) >= 6
                && g.wdist(**pos, target_pos) >= 6
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .next()
        .expect("a remote land site");
    let remote = g.found_city_for(0, remote_pos, None);
    let routine = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: routine.clone(),
        },
    )
    .expect("the capital can start a builder");
    assert_eq!(g.item_invested_production(home, &routine), 0.0);
    for (cid, production) in [(home, 25.0), (remote, 1.0)] {
        let before = g.city_yields(cid).production;
        std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
            .entry(cid)
            .or_default()
            .production += production - before;
    }
    let gun = Item::Unit {
        unit: crate::name!("catapult"),
    };
    assert!(g.can_produce(0, home, &gun) && g.can_produce(0, remote, &gun));
    let fast_arrival = ai.production_build_turns(&g, 0, home, &gun)
        + f64::from(g.wdist(home_pos, target_pos)) / g.rules.units[&crate::name!("catapult")].moves;
    let slow_arrival = ai.production_build_turns(&g, 0, remote, &gun)
        + f64::from(g.wdist(remote_pos, target_pos))
            / g.rules.units[&crate::name!("catapult")].moves;
    assert!(fast_arrival <= 30.0 && slow_arrival >= fast_arrival * 1.5 + 8.0);

    let mut invested = g.clone();
    invested.cities.get_mut(&home).unwrap().production = 10.0;
    assert_eq!(
        ai.reserve_delegated_domination_siege(&mut invested, 0, &plan),
        Some((remote, gun.clone())),
        "invested work keeps the capital's queue"
    );
    let mut military = g.clone();
    military.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("archer"),
    }];
    assert_eq!(
        ai.reserve_delegated_domination_siege(&mut military, 0, &plan),
        Some((remote, gun.clone())),
        "an active military queue is not displaced"
    );
    let mut walls = g.clone();
    walls.cities.get_mut(&home).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    assert_eq!(
        ai.reserve_delegated_domination_siege(&mut walls, 0, &plan),
        Some((remote, gun.clone())),
        "a city's wall order is not displaced"
    );
    assert_eq!(
        ai.reserve_delegated_domination_siege(&mut g, 0, &plan),
        Some((home, gun.clone())),
        "the fresh routine queue yields to the timely siege gun"
    );
    assert_eq!(g.cities[&home].queue.first(), Some(&gun));
}

#[test]
fn delegated_domination_reserves_another_gun_when_the_first_cannot_breach() {
    let (mut g, mut ai, plan, home, target) = siege_gap_case();
    ai.enable_lane_delegates_production_2();
    ai.enable_siege_positive_damage_budget();
    let pos = g.cities[&target].pos;
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("renaissance_walls"));
    g.cities.get_mut(&target).unwrap().wall_hp = 300;
    g.spawn_unit("modern_armor", 1, pos);
    let gun = g.spawn_unit("catapult", 0, (pos.0 - 2, pos.1));
    let taker = g.spawn_unit("swordsman", 0, (pos.0 - 1, pos.1));
    let (finish, endurance) = ai
        .conversion_siege_budget(&g, 0, target, &[gun, taker])
        .expect("the active siege is visible");
    assert!(finish > endurance * 0.8);
    assert_eq!(ai.counts(&g, 0).siege, 1);
    assert!(ai
        .reserve_delegated_domination_siege(&mut g, 0, &plan)
        .is_some());
    assert!(matches!(
        g.cities[&home].queue.first(),
        Some(Item::Unit { unit }) if g.rules.units[unit].siege
    ));
}

#[test]
fn delegated_siege_reservation_survives_an_appointed_war_plan() {
    let (mut g, mut ai, plan, home, target) = siege_gap_case();
    ai.enable_lane_delegates_production_2();
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: target,
        breakthrough_tech: crate::name!("engineering"),
        assault_unit: crate::name!("catapult"),
        predecessor: None,
        breach_unit: None,
        estimated_research_turns: 0,
        estimated_production_turns: 3,
        estimated_upgrade_gold: 0.0,
        estimated_march_turns: 2,
        phase: WarPhase::Strike,
        appointed_turn: g.turn - 5,
        tech_turn: Some(g.turn - 4),
        declared_turn: Some(g.turn - 3),
        last_reviewed_turn: g.turn,
        recovery_assessments: 0,
    });

    assert!(ai
        .reserve_delegated_domination_siege(&mut g, 0, &plan)
        .is_some());
    assert_eq!(
        g.cities[&home].queue.first(),
        Some(&Item::Unit {
            unit: crate::name!("catapult"),
        })
    );
}

#[test]
fn invested_wall_breaker_survives_a_later_city_order_unless_defense_claims_it() {
    let (mut g, ai, plan, home, target) = siege_gap_case();
    let catapult = Item::Unit {
        unit: crate::name!("catapult"),
    };
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: catapult.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&home).unwrap().production = 60.0;
    let claim = ai.invested_wall_breaker_queues(&g, 0, &plan);
    assert_eq!(claim, vec![(home, catapult.clone())]);

    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: warrior.clone(),
        },
    )
    .unwrap();
    let mut defending = g.clone();
    ai.restore_wall_breaker_queues(
        &mut defending,
        0,
        &plan,
        &claim,
        Some(&(home, warrior.clone())),
    );
    assert_eq!(defending.cities[&home].queue.first(), Some(&warrior));

    ai.restore_wall_breaker_queues(&mut g, 0, &plan, &claim, None);
    assert_eq!(g.cities[&home].queue.first(), Some(&catapult));
    assert_eq!(g.cities[&home].production, 60.0);

    g.at_war.clear();
    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: warrior.clone(),
        },
    )
    .unwrap();
    ai.restore_wall_breaker_queues(&mut g, 0, &plan, &claim, None);
    assert_eq!(g.cities[&home].queue.first(), Some(&warrior));
}

#[test]
fn fresh_delegated_siege_reservation_survives_the_same_pass() {
    let (mut g, ai, plan, home, _) = siege_gap_case();
    let claim = ai
        .reserve_delegated_domination_siege(&mut g, 0, &plan)
        .unwrap();
    assert_eq!(claim.0, home);
    g.apply(
        0,
        &Action::Produce {
            city: home,
            item: Item::Unit {
                unit: crate::name!("warrior"),
            },
        },
    )
    .unwrap();
    ai.restore_wall_breaker_queues(&mut g, 0, &plan, &[claim.clone()], None);
    assert_eq!(g.cities[&home].queue.first(), Some(&claim.1));
}

#[test]
fn an_existing_or_queued_siege_unit_closes_the_composition_exception() {
    for queued in [false, true] {
        let (mut g, ai, plan, home, _) = siege_gap_case();
        let item = Item::Unit {
            unit: crate::name!("catapult"),
        };
        if queued {
            g.cities.get_mut(&home).unwrap().queue.push(item.clone());
        } else {
            g.spawn_unit("catapult", 0, g.cities[&home].pos);
        }
        let counts = ai.counts(&g, 0);
        assert_eq!(counts.siege, 1);
        assert_eq!(
            ai.production_value(&g, 0, home, &item, &plan, &counts),
            -2_000.0
        );
    }
}

#[test]
fn peaceful_wall_free_and_disabled_domination_plans_keep_the_army_ceiling() {
    for case in 0..3 {
        let (mut g, ai, plan, home, target) = siege_gap_case();
        match case {
            0 => g.at_war.clear(),
            1 => g.cities.get_mut(&target).unwrap().wall_hp = 0,
            _ => g.victory_conditions.domination = false,
        }
        assert_eq!(
            ai.production_value(
                &g,
                0,
                home,
                &Item::Unit {
                    unit: crate::name!("catapult")
                },
                &plan,
                &ai.counts(&g, 0)
            ),
            -2_000.0
        );
    }
}
