use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Game;

fn captured_frontier() -> (Game, StrategicPlan, u32) {
    let mut g = Game::new(2, 32, 20, 3800, 250, 0);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    g.players[0].civ = "Gran Colombia".to_string();
    g.found_city_for(0, (5, 8), None);
    g.found_city_for(0, (13, 8), None);
    g.found_city_for(1, (24, 8), None);
    let captured = *g
        .player_city_ids(0)
        .iter()
        .find(|cid| g.cities[cid].pos == (13, 8))
        .unwrap();
    g.cities.get_mut(&captured).unwrap().original_owner = 1;
    g.at_war.insert((0, 1));
    g.players[0].techs.extend([
        crate::name!("mining"),
        crate::name!("bronze_working"),
        crate::name!("archery"),
    ]);
    g.players[0].boosted_techs.insert(crate::name!("masonry"));
    for _ in 0..3 {
        g.spawn_test_unit("archer", 0, (5, 8));
    }
    g.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan, captured)
}

#[test]
fn captured_frontier_unlocks_walls_before_waiting_for_a_visible_attacker() {
    let (mut g, plan, cid) = captured_frontier();
    let mut ai = deployed(VictoryTarget::Domination);
    ai.enable_captured_city_wall_research();
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 0);
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));
}

fn deployed(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_live_bridge_universe();
    let forced = include_str!("../../../../deploy/live-force-on.txt")
        .trim()
        .split(',')
        .collect::<Vec<_>>();
    ai.apply_gene_ledger_with_forced_live(&forced);
    ai
}

fn same_research_with_policy(g: Game, plan: &StrategicPlan, mut ai: AdvancedAi) {
    let mut old = g.clone();
    ai.disable_captured_city_wall_research();
    ai.advanced_research(&mut old, 0, plan);
    ai.enable_captured_city_wall_research();
    assert!(ai.captured_city_walls_research_goal(&g, 0).is_none());
    let mut candidate = g;
    ai.advanced_research(&mut candidate, 0, plan);
    assert_eq!(candidate.players[0].research, old.players[0].research);
    assert_eq!(
        candidate.players[0].research_progress,
        old.players[0].research_progress
    );
}

#[test]
fn off_keeps_the_reproduced_wheel_choice_and_defaults_stay_off() {
    assert!(!AdvancedAi::new().captured_city_wall_research);
    assert!(!AdvancedAi::targeting(VictoryTarget::Domination).captured_city_wall_research);
    let (mut g, plan, _) = captured_frontier();
    let ai = deployed(VictoryTarget::Domination);
    assert!(!ai.captured_city_wall_research);
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("wheel"));
}

#[test]
fn founded_city_keeps_existing_research() {
    let (mut g, plan, cid) = captured_frontier();
    g.cities.get_mut(&cid).unwrap().original_owner = 0;
    same_research_with_policy(g, &plan, deployed(VictoryTarget::Domination));
}

#[test]
fn peace_with_the_original_owner_keeps_existing_research() {
    let (mut g, plan, _) = captured_frontier();
    g.at_war.clear();
    same_research_with_policy(g, &plan, deployed(VictoryTarget::Domination));
}

#[test]
fn walled_captured_city_keeps_existing_research() {
    let (mut g, plan, cid) = captured_frontier();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    same_research_with_policy(g, &plan, deployed(VictoryTarget::Domination));
}

#[test]
fn observed_walls_also_keep_existing_research() {
    let (mut g, plan, cid) = captured_frontier();
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 100);
    same_research_with_policy(g, &plan, deployed(VictoryTarget::Domination));
}

#[test]
fn known_masonry_keeps_existing_research() {
    let (mut g, plan, _) = captured_frontier();
    g.players[0].techs.insert(crate::name!("masonry"));
    same_research_with_policy(g, &plan, deployed(VictoryTarget::Domination));
}

#[test]
fn eliminated_or_nonmajor_original_owner_keeps_existing_research() {
    for kind in 0..3 {
        let (mut g, plan, _) = captured_frontier();
        match kind {
            0 => g.players[1].alive = false,
            1 => g.players[1].is_minor = true,
            _ => g.players[1].is_barbarian = true,
        }
        same_research_with_policy(g, &plan, deployed(VictoryTarget::Domination));
    }
}

#[test]
fn other_victory_targets_and_frozen_legacy_keep_existing_research() {
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
        VictoryTarget::Score,
    ] {
        let (g, plan, _) = captured_frontier();
        same_research_with_policy(g, &plan, deployed(target));
    }
    let (g, plan, _) = captured_frontier();
    same_research_with_policy(g, &plan, AdvancedAi::legacy());
    let (g, plan, _) = captured_frontier();
    let mut ai = deployed(VictoryTarget::Domination);
    ai.base.legacy_movement = true;
    same_research_with_policy(g, &plan, ai);
}

#[test]
fn in_progress_research_is_preserved_even_when_preparation_is_needed() {
    let (mut g, plan, _) = captured_frontier();
    let mut ai = deployed(VictoryTarget::Domination);
    ai.enable_captured_city_wall_research();
    assert!(ai.captured_city_walls_research_goal(&g, 0).is_some());
    g.players[0].research = Some("shipbuilding".to_string());
    g.players[0].research_progress = 67.0;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("shipbuilding"));
    assert_eq!(g.players[0].research_progress, 67.0);
}
