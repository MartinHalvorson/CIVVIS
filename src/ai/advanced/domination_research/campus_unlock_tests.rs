use super::*;
use crate::{ai::VictoryTarget, name};

fn opening() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 936012, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(1, (25, 12), None);
    g.spawn_test_unit("warrior", 0, (6, 12));
    g.spawn_test_unit("archer", 0, (12, 12));
    g.spawn_test_unit("builder", 0, (6, 12));
    g.players[0].techs.extend(
        [
            "mining",
            "animal_husbandry",
            "archery",
            "pottery",
            "irrigation",
        ]
        .map(Name::new),
    );
    g.at_war.clear();
    g.current = 0;
    g.turn = 24;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn a_two_city_domination_opening_unlocks_campuses_before_optional_branches() {
    let (mut g, ai, plan) = opening();
    assert!(g.available_techs(0).contains(&name!("writing")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("writing"));
}

#[test]
fn the_campus_opening_walks_the_missing_pottery_prerequisite() {
    let (mut g, ai, plan) = opening();
    g.players[0].techs.remove(&name!("pottery"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("pottery"));
}

#[test]
fn an_active_rush_keeps_its_military_unlock_first() {
    let (mut g, ai, mut plan) = opening();
    plan.rush = true;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("horseback_riding"));
}

#[test]
fn the_first_ranged_defender_keeps_priority_over_campuses() {
    let (mut g, mut ai, plan) = opening();
    g.players[0].techs.remove(&name!("archery"));
    for uid in g
        .units
        .values()
        .filter(|u| u.owner == 0 && u.kind == "archer")
        .map(|u| u.id)
        .collect::<Vec<_>>()
    {
        g.remove_unit(uid);
    }
    g.spawn_test_unit("slinger", 0, (6, 12));
    ai.base.garrison_under_fire = true;
    assert_eq!(ai.opening_archery_goal(&g, 0).as_deref(), Some("archery"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("archery"));
}

#[test]
fn the_campus_opening_does_not_interrupt_current_research() {
    let (mut g, ai, plan) = opening();
    g.players[0].research = Some("bronze_working".to_string());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("bronze_working"));
}

#[test]
fn the_campus_goal_is_bounded_to_a_missing_domination_unlock() {
    for case in 0..4 {
        let (mut g, mut ai, _) = opening();
        assert_eq!(ai.domination_campus_unlock_goal(&g, 0), Some("writing"));
        match case {
            0 => {
                g.players[0].techs.insert(name!("writing"));
            }
            1 => {
                ai.victory_target = Some(VictoryTarget::Culture);
            }
            2 => {
                g.victory_conditions.domination = false;
            }
            _ => {
                let second = g.player_city_ids(0)[1];
                g.cities.get_mut(&second).unwrap().owner = 1;
            }
        }
        assert_eq!(ai.domination_campus_unlock_goal(&g, 0), None);
    }
}

/// `campus-unlock-before-the-luxury`: with a Plantation luxury awaiting
/// Irrigation, the stock order connects the luxury first; under the gene
/// Writing goes first.
#[test]
fn the_campus_unlock_goes_ahead_of_the_luxury_connection_under_the_gene() {
    let pick = |gene: bool| {
        let (mut g, mut ai, plan) = opening();
        g.players[0].techs.remove(&name!("irrigation"));
        let city = g.player_city_ids(0)[0];
        let spot = g
            .map
            .disk(g.cities[&city].pos, 2)
            .into_iter()
            .find(|pos| {
                *pos != g.cities[&city].pos
                    && g.map.get(*pos).is_some_and(|t| t.owner_city == Some(city))
            })
            .expect("a tile the capital owns");
        g.map.tiles.get_mut(&spot).unwrap().resource = Some(name!("coffee"));
        ai.enable_connect_the_luxury();
        if gene {
            ai.enable_campus_unlock_before_the_luxury();
        }
        ai.advanced_research(&mut g, 0, &plan);
        g.players[0].research.clone()
    };
    let stock = pick(false);
    assert_ne!(stock.as_deref(), Some("writing"), "stock: {stock:?}");
    assert_eq!(pick(true).as_deref(), Some("writing"));
}
