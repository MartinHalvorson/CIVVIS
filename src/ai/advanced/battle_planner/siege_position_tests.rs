use super::super::{ForceDomain, GrandStrategy};
use super::*;
use crate::doctrine::{build, position};

fn assault() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = build(position("the_storming").unwrap(), 3).unwrap();
    for uid in (0..2)
        .flat_map(|pid| g.player_unit_ids(pid))
        .collect::<Vec<_>>()
    {
        g.remove_unit(uid);
    }
    let cid = *g.cities.keys().next().unwrap();
    let target = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        tile.terrain = "grassland".into();
        tile.feature = None;
        tile.hills = false;
    }
    let start = g
        .wring(target, 5)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some())
        .unwrap();
    let gun = g.spawn_unit("bombard", 0, start);
    for pos in g
        .wring(target, 1)
        .into_iter()
        .filter(|pos| g.map.get(*pos).is_some())
        .take(3)
        .collect::<Vec<_>>()
    {
        g.spawn_unit("musketman", 0, pos);
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(cid),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_3();
    ai.enable_siege_train();
    ai.force_groups = vec![ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: g.player_unit_ids(0),
        anchor: target,
        objective: target,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 3.0,
    }];
    (g, ai, plan, gun, cid)
}

#[test]
fn siege_gun_keeps_its_approach_turn_and_fires() {
    let (mut g, mut ai, plan, gun, cid) = assault();
    ai.plan_battle(&mut g, 0, &plan);
    assert!(
        !ai.battle_planner_claims(gun),
        "formation positioning consumed the siege gun's approach turn"
    );
    let walls = g.cities[&cid].wall_hp;
    for _ in 0..10 {
        for _ in 0..4 {
            if g.units[&gun].moves_left <= 0.0 {
                break;
            }
            ai.siege_doctrine_step(&mut g, 0, gun, &plan);
        }
        if g.cities[&cid].wall_hp < walls {
            return;
        }
        g.apply(0, &Action::EndTurn).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
        ai.plan_battle(&mut g, 0, &plan);
    }
    panic!("the siege gun never fired on the city");
}

#[test]
fn ordinary_formations_still_position_without_siege_doctrine() {
    let (mut g, mut ai, plan, gun, _) = assault();
    ai.disable_siege_train();
    ai.plan_battle(&mut g, 0, &plan);
    assert!(ai.battle_planner_claims(gun));
}

#[test]
fn nearby_city_campaign_fallback_also_owns_its_positions() {
    let (mut g, mut ai, plan, gun, _) = assault();
    ai.force_groups[0].objective = g.units[&gun].pos;
    ai.plan_battle(&mut g, 0, &plan);
    assert!(!ai.battle_planner_claims(gun));
}
