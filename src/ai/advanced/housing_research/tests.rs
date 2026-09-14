use super::*;
use crate::game::Action;

fn board() -> Game {
    let mut g = Game::new_full(2, 32, 24, 91_435_680, 250, 0, false);
    g.players[0].civ = "America".to_string();
    for _ in 0..2 {
        let pos = g
            .map
            .tiles
            .values()
            .find(|tile| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && tile.district.is_none()
                    && tile.owner_city.is_none()
                    && g.cities
                        .values()
                        .all(|city| g.wdist(city.pos, tile.pos) >= 5)
            })
            .unwrap()
            .pos;
        g.found_city_for(0, pos, None);
    }
    for cid in g.player_city_ids(0) {
        let housing = g.city_housing(&g.cities[&cid]);
        g.cities.get_mut(&cid).unwrap().pop = housing.ceil() as i32;
    }
    g.players[0].techs.clear();
    g
}

fn ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_housing_research_2();
    ai
}

fn deny_district_space(g: &mut Game) {
    for cid in g.player_city_ids(0) {
        let city = g.cities[&cid].clone();
        for pos in city.owned_tiles {
            if pos != city.pos {
                g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
            }
        }
    }
}

#[test]
fn versions_are_exclusive_and_v1_keeps_its_original_goal() {
    let g = board();
    let mut ai = ai();
    assert_eq!(ai.unreachable_housing_tech(&g, 0), Some("pottery"));
    ai.enable_housing_research();
    assert!(!ai.housing_research_2);
    assert_eq!(ai.unreachable_housing_tech(&g, 0), Some("engineering"));
    ai.enable_housing_research_2();
    assert!(!ai.housing_research);
    ai.disable_housing_research_2();
    assert_eq!(ai.unreachable_housing_tech(&g, 0), None);
    assert!(!AdvancedAi::new().housing_research_2);
    assert!(!AdvancedAi::legacy().housing_research_2);
}

#[test]
fn researches_the_granary_the_capped_cities_can_actually_build() {
    let mut g = board();
    let goal = ai().usable_housing_tech(&g, 0).unwrap();
    assert_eq!(goal, "pottery");
    for cid in g.player_city_ids(0) {
        assert!(!g.can_produce(
            0,
            cid,
            &Item::Building {
                building: crate::name!("granary")
            }
        ));
    }
    g.players[0].techs.insert(goal);
    for cid in g.player_city_ids(0) {
        let before = g.city_housing(&g.cities[&cid]);
        let item = Item::Building {
            building: crate::name!("granary"),
        };
        assert!(g.can_produce(0, cid, &item));
        g.apply(0, &Action::Produce { city: cid, item }).unwrap();
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("granary"));
        assert_eq!(g.city_housing(&g.cities[&cid]) - before, 2.0);
    }
}

#[test]
fn known_housing_stays_with_production_and_one_capped_city_does_not_detour() {
    let mut g = board();
    g.players[0].techs.insert(crate::name!("pottery"));
    assert_eq!(ai().usable_housing_tech(&g, 0), None);
    g.players[0].techs.clear();
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().pop = 1;
    assert_eq!(ai().usable_housing_tech(&g, 0), None);
}

#[test]
fn reaches_sewers_when_existing_aqueduct_technology_has_no_usable_site() {
    let mut g = board();
    deny_district_space(&mut g);
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].techs.remove(&crate::name!("sanitation"));
    for cid in g.player_city_ids(0) {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("granary"));
        let housing = g.city_housing(&g.cities[&cid]);
        g.cities.get_mut(&cid).unwrap().pop = housing.ceil() as i32;
        assert!(g.district_sites(cid, crate::name!("aqueduct")).is_empty());
    }
    assert_eq!(
        ai().usable_housing_tech(&g, 0),
        Some(crate::name!("sanitation"))
    );
    // Unlocking the tech does not stand in for a missing building prerequisite.
    std::sync::Arc::make_mut(&mut g.rules)
        .buildings
        .get_mut("sewer")
        .unwrap()
        .requires
        .push(crate::name!("library"));
    assert_eq!(ai().usable_housing_tech(&g, 0), None);
}

#[test]
fn no_aqueduct_site_cannot_justify_engineering() {
    let mut g = board();
    deny_district_space(&mut g);
    // Isolate the Aqueduct unlock, retaining every native placement rule.
    let rules = std::sync::Arc::make_mut(&mut g.rules);
    for spec in rules.buildings.values_mut() {
        spec.housing = 0.0;
    }
    assert_eq!(ai().usable_housing_tech(&g, 0), None);
    let mut v1 = AdvancedAi::new();
    v1.enable_housing_research();
    assert_eq!(v1.unreachable_housing_tech(&g, 0), Some("engineering"));
}

#[test]
fn the_complete_missing_path_outweighs_a_deceptively_cheap_leaf() {
    let mut g = board();
    deny_district_space(&mut g);
    std::sync::Arc::make_mut(&mut g.rules)
        .techs
        .get_mut("sanitation")
        .unwrap()
        .cost = 1.0;
    assert_eq!(
        ai().usable_housing_tech(&g, 0),
        Some(crate::name!("pottery"))
    );
    // Sanitation still needs Pottery: a cheap leaf cannot skip that ancestor.
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].techs.remove(&crate::name!("pottery"));
    g.players[0].techs.remove(&crate::name!("sanitation"));
    assert_eq!(
        ai().usable_housing_tech(&g, 0),
        Some(crate::name!("pottery"))
    );
    // Once the entire path and its current remedy are paid, reach the Sewer.
    g.players[0].techs.insert(crate::name!("pottery"));
    for cid in g.player_city_ids(0) {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("granary"));
        g.cities.get_mut(&cid).unwrap().pop += 2;
    }
    assert_eq!(
        ai().usable_housing_tech(&g, 0),
        Some(crate::name!("sanitation"))
    );
}

#[test]
fn boosts_and_overflow_match_the_engine_without_double_counting_active_progress() {
    for civ in ["America", "China"] {
        let mut g = board();
        g.players[0].civ = civ.to_string();
        let tech = crate::name!("pottery");
        let path = [tech].into_iter().collect();
        g.players[0].boosted_techs.insert(tech);
        g.players[0].research_overflow = 3.0;
        let before = AdvancedAi::housing_research_path_cost(&g, 0, &path);
        g.apply(0, &Action::Research { tech }).unwrap();
        let expected = g.tech_cost(tech.as_str()) - g.players[0].research_progress;
        assert!(
            (before - expected).abs() < 1e-9,
            "{civ}: {before} vs {expected}"
        );
        assert_eq!(
            AdvancedAi::housing_research_path_cost(&g, 0, &path),
            expected
        );
        g.players[0].research_progress += 2.0;
        assert_eq!(
            AdvancedAi::housing_research_path_cost(&g, 0, &path),
            expected - 2.0
        );
    }
}

#[test]
fn hypothetical_unlocks_do_not_mutate_the_board() {
    let g = board();
    let before = serde_json::to_value(&g).unwrap();
    assert!(ai().usable_housing_tech(&g, 0).is_some());
    assert_eq!(serde_json::to_value(&g).unwrap(), before);
}

#[test]
fn the_rewrite_changes_the_actual_research_order() {
    let mut original = board();
    let mut rewritten = original.clone();
    let plan = super::super::StrategicPlan {
        strategy: super::super::GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: original.turn,
        rush: false,
    };
    let mut controller = AdvancedAi::legacy();
    controller.enable_housing_research();
    controller.advanced_research(&mut original, 0, &plan);
    controller.enable_housing_research_2();
    controller.advanced_research(&mut rewritten, 0, &plan);
    assert_eq!(rewritten.players[0].research.as_deref(), Some("pottery"));
    assert_ne!(original.players[0].research, rewritten.players[0].research);
}
