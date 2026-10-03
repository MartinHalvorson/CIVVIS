use super::*;
use crate::game::ObservedPublicEmpireStats;

fn board(district: &str) -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 64, 40, 91_021, 650, 0, false);
    super::tests::found_capitals(&mut g);
    g.turn = 200;
    g.record_contact(0, 1);
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    let second_pos = super::tests::open_land_near(&g, home, 4);
    g.found_city_for(0, second_pos, None);
    let far = g.player_city_ids(1)[0];
    let far_pos = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .filter(|pos| {
            g.player_city_ids(0)
                .iter()
                .all(|cid| g.wdist(g.cities[cid].pos, *pos) > 22)
        })
        .min()
        .unwrap();
    g.cities.get_mut(&far).unwrap().pos = far_pos;
    g.cities
        .get_mut(&far)
        .unwrap()
        .districts
        .insert(crate::name::Name::new(district), far_pos);
    let near_pos = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .filter(|pos| (6..=10).contains(&g.wdist(home, *pos)))
        .min()
        .unwrap();
    let near = g.found_city_for(1, near_pos, Some("Frontier".into()));
    g.spawn_test_unit("scout", 0, far_pos);
    g.spawn_test_unit("scout", 0, near_pos);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&g, 0);
    assert!(!AdvancedAi::city_within_declaration_range(&g, 0, far_pos));
    assert!(AdvancedAi::city_within_declaration_range(&g, 0, near_pos));
    (g, ai, near, far)
}

fn pressure(strategy: GrandStrategy) -> VictoryFocus {
    VictoryFocus {
        strategy,
        progress: 95,
    }
}

#[test]
fn prewar_suppression_does_not_name_an_objective_the_declaration_gate_refuses() {
    for (strategy, district) in [
        (GrandStrategy::Culture, "theater_square"),
        (GrandStrategy::Science, "spaceport"),
        (GrandStrategy::Religion, "holy_site"),
    ] {
        let (g, ai, _, _) = board(district);
        assert_eq!(
            ai.victory_suppression_city(&g, 0, 1, pressure(strategy)),
            None
        );
    }
}

#[test]
fn prewar_suppression_prefers_reachable_infrastructure_over_a_cheaper_far_city() {
    let (mut g, ai, near, far) = board("theater_square");
    let near_pos = g.cities[&near].pos;
    g.cities
        .get_mut(&near)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), near_pos);
    g.cities.get_mut(&far).unwrap().hp = 1;
    g.cities.get_mut(&near).unwrap().wall_hp = 400;
    assert!(
        ai.campaign_city_value(&g, 0, &g.cities[&far], GrandStrategy::Conquest)
            < ai.campaign_city_value(&g, 0, &g.cities[&near], GrandStrategy::Conquest)
    );
    assert_eq!(
        ai.victory_suppression_city(&g, 0, 1, pressure(GrandStrategy::Culture)),
        Some(near)
    );
}

#[test]
fn distant_culture_infrastructure_falls_back_to_the_selected_rivals_frontier() {
    let (mut g, mut ai, near, _) = board("theater_square");
    let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
    for pid in 0..2 {
        stats.insert(
            pid,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(100),
                foreign_tourists: Some(if pid == 1 { 95 } else { 0 }),
                ..Default::default()
            },
        );
    }
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(near));
    assert!(g.player_city_ids(0).len() >= 2);
    assert!(ai.urgent_victory_threat(&g, 1));
    assert!(!ai.campaign_staged_for_war(&g, 0, 1, g.cities[&near].pos, true));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 1),
        "an objective alone is not a staged army"
    );
}

#[test]
fn existing_wars_keep_distant_victory_infrastructure_available() {
    let (mut g, ai, _, far) = board("theater_square");
    g.at_war.insert((0, 1));
    assert_eq!(
        ai.victory_suppression_city(&g, 0, 1, pressure(GrandStrategy::Culture)),
        Some(far)
    );
}

#[test]
fn reachable_infrastructure_keeps_priority_over_a_plain_frontier() {
    let (mut g, ai, near, far) = board("theater_square");
    g.cities.get_mut(&far).unwrap().districts.clear();
    let pos = g.cities[&near].pos;
    g.cities
        .get_mut(&near)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), pos);
    assert_eq!(
        ai.victory_suppression_city(&g, 0, 1, pressure(GrandStrategy::Culture)),
        Some(near)
    );
}
