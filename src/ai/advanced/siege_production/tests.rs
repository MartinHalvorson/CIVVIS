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
