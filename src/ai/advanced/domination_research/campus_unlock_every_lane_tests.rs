//! `campus-unlock-in-every-lane`: the Writing goal two founded cities take,
//! under every named victory target.
use super::*;
use crate::{ai::VictoryTarget, name};

fn opening(target: VictoryTarget) -> (Game, AdvancedAi, StrategicPlan) {
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
    let ai = AdvancedAi::targeting(target);
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

const LANES: [VictoryTarget; 4] = [
    VictoryTarget::Science,
    VictoryTarget::Culture,
    VictoryTarget::Diplomacy,
    VictoryTarget::Religion,
];

#[test]
fn every_named_lane_takes_the_writing_goal_under_the_gene() {
    for target in LANES {
        let (g, mut ai, _) = opening(target);
        assert_eq!(ai.domination_campus_unlock_goal(&g, 0), None, "{target:?} stock");
        ai.enable_campus_unlock_in_every_lane();
        assert_eq!(
            ai.domination_campus_unlock_goal(&g, 0),
            Some("writing"),
            "{target:?} under the gene"
        );
    }
}

#[test]
fn a_two_city_science_opening_researches_writing_under_the_gene() {
    let (mut g, mut ai, plan) = opening(VictoryTarget::Science);
    ai.enable_campus_unlock_in_every_lane();
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("writing"));
}

#[test]
fn the_gene_keeps_the_goal_bounded() {
    // A lone capital, Writing already known, and an unnamed seat all decline.
    let (mut g, mut ai, _) = opening(VictoryTarget::Culture);
    ai.enable_campus_unlock_in_every_lane();
    g.players[0].techs.insert(name!("writing"));
    assert_eq!(ai.domination_campus_unlock_goal(&g, 0), None);

    let (mut g, mut ai, _) = opening(VictoryTarget::Culture);
    ai.enable_campus_unlock_in_every_lane();
    let second = g.player_city_ids(0)[1];
    g.cities.get_mut(&second).unwrap().owner = 1;
    assert_eq!(ai.domination_campus_unlock_goal(&g, 0), None);

    let (g, mut ai, _) = opening(VictoryTarget::Culture);
    ai.enable_campus_unlock_in_every_lane();
    ai.victory_target = None;
    assert_eq!(ai.active_victory_target(&g), None);
    assert_eq!(ai.domination_campus_unlock_goal(&g, 0), None);
}

#[test]
fn domination_is_unchanged_by_the_gene() {
    let (g, mut ai, _) = opening(VictoryTarget::Domination);
    assert_eq!(ai.domination_campus_unlock_goal(&g, 0), Some("writing"));
    ai.enable_campus_unlock_in_every_lane();
    assert_eq!(ai.domination_campus_unlock_goal(&g, 0), Some("writing"));
}
