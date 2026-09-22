use super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 372100, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (2, 8), None);
    g.found_city_for(2, (18, 8), None);
    g.found_city_for(3, (28, 8), None);
    g.players[1].religion = Some("Islam".into());
    for pid in [0, 1, 2] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, "Islam".into());
    }
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 83;
    g.players[0].gold = 10000.0;
    let defender = g.spawn_test_unit("spearman", 0, (4, 8));
    let missionary = g.spawn_test_unit("missionary", 1, (4, 8));
    g.units.get_mut(&missionary).unwrap().religion = Some("Islam".into());
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.urgent_victory_threat(&g, 1));
    assert!(g.player_city_ids(1).is_empty());
    (g, ai, plan, defender, missionary)
}

#[test]
fn urgent_religious_interception_does_not_require_a_known_enemy_city() {
    let (mut g, mut ai, plan, _, missionary) = fixture();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "the nearby spreader is an actionable religious counter"
    );
    assert!(
        !g.units.contains_key(&missionary),
        "the opening must execute its promised interception"
    );
}

#[test]
fn religious_interception_preserves_nonurgent_and_executable_action_gates() {
    for control in 0..4 {
        let (mut g, mut ai, plan, defender, missionary) = fixture();
        match control {
            0 => {
                Arc::make_mut(&mut g.observed_majority_religion).remove(&2);
            }
            1 => {
                g.units.get_mut(&defender).unwrap().moves_left = 0.0;
            }
            2 => {
                ai = AdvancedAi::targeting(VictoryTarget::Science);
            }
            _ => {
                g.apply(0, &Action::DeclareWar { player: 2 }).unwrap();
            }
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(!g.is_at_war(0, 1), "control {control}");
        assert!(g.units.contains_key(&missionary));
    }
}
