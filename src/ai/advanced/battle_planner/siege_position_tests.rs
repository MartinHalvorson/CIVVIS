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

/// Live King civvis-20261004T033533Z (game 46), turn 140: the battle planner
/// rotated four 100-hp catapults before Babylon "out to heal" at danger
/// 96-100 while their siege stood in Stage, and Stage never invested. A
/// healthy member of a Stage siege is the train's to move, as an active
/// member already is; the same gun outside any siege still rotates. (A
/// frame-0 replay of that turn now rotates one catapult, outside the siege
/// force, instead of four.)
#[test]
fn a_healthy_stage_siege_gun_is_not_rotated_out() {
    let rotated = |staged: bool| {
        let (mut g, mut ai, _plan, gun, cid) = assault();
        // A campaign board: an arena rotates nobody out to heal.
        g.map_script = crate::setup::MapScript::LandOnly;
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        ai.victory_target = Some(crate::ai::VictoryTarget::Domination);
        // At the rotation line but not wounded (ROTATE_HP).
        g.units.get_mut(&gun).unwrap().hp = ROTATE_HP;
        let here = g.units[&gun].pos;
        let city = g.cities[&cid].pos;
        let mut ring: Vec<Pos> = g
            .wring(here, 3)
            .into_iter()
            .filter(|pos| {
                g.city_at(*pos).is_none()
                    && g.unit_ids_at(*pos).is_empty()
                    && g.map.get(*pos).is_some()
                    && g.wdist(*pos, city) > 1
            })
            .collect();
        ring.sort_by_key(|pos| (g.wdist(*pos, city), *pos));
        for pos in ring.iter().take(3) {
            g.spawn_unit("crossbowman", 1, *pos);
        }
        if staged {
            ai.sieges.insert(
                cid,
                crate::ai::advanced::siege_train::Siege {
                    stage: crate::ai::advanced::siege_train::SiegeStage::Stage,
                    taker: None,
                    entered: g.turn,
                    assessed: g.turn,
                    posts: Default::default(),
                    short_since: None,
                },
            );
        }
        assert_eq!(ai.staging_siege_member(&g, 0, gun), staged);
        assert!(!ai.active_siege_member(&g, 0, gun));
        let mut field = DangerField::with_reach(&g, 0, true);
        assert!(
            field.rotation_danger(here, gun) > f64::from(ROTATE_HP - ROTATE_DANGER_MARGIN),
            "the gun reads exposed"
        );
        let _ = ai.rotate_wounded(&mut g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
        ai.battle_planner_ordered.contains(&gun) || g.units[&gun].pos != here
    };
    assert!(rotated(false), "the control: an exposed gun outside any siege rotates out");
    assert!(!rotated(true), "a healthy gun of a Stage siege is left to the train");
}
