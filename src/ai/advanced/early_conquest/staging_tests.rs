use super::super::GrandStrategy;
use super::*;
use crate::name;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(3, 40, 24, 91919, 1000, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let home = g.found_city_for(0, (6, 12), None);
    let target = g.found_city_for(1, (18, 12), None);
    let wrong = g.found_city_for(2, (6, 18), None);
    g.players[0].met.extend([1, 2]);
    g.players[1].met.insert(0);
    g.players[2].met.insert(0);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.at_war.clear();
    g.turn = 20;
    g.current = 0;
    let uid = g.spawn_test_unit("warrior", 0, (10, 12));
    let mut ai = AdvancedAi::new();
    ai.enable_early_conquest_opening();
    ai.conquest_opening = Some(ConquestOpening {
        target: 1,
        city: target,
        opened: 19,
        rally: (15, 12),
        force: [uid].into_iter().collect(),
        assembled: None,
        declared: None,
        kills_at_war: 0,
        losses: 0,
        taken: 0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: Some(2),
        target_city: Some(wrong),
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 20,
        rush: false,
    };
    assert_ne!(home, target);
    (g, ai, plan, uid)
}

#[test]
fn reserved_force_stages_for_its_opening_during_expansion() {
    let (mut g, mut ai, plan, uid) = fixture();
    let start = g.units[&uid].pos;
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    let mut off = ai.clone();
    off.disable_early_conquest_opening();
    assert_eq!(
        off.campaign_staging_step(&mut g.clone(), 0, uid, &plan),
        None
    );
    assert_eq!(ai.campaign_staging_step(&mut g, 0, uid, &plan), Some(true));
    assert!(g.wdist(g.units[&uid].pos, rally) < g.wdist(start, rally));
    assert!(!g.is_at_war(0, 1), "assembly never declares a war");
}

#[test]
fn opening_staging_reaches_the_military_dispatch() {
    let (mut g, mut ai, plan, uid) = fixture();
    let start = g.units[&uid].pos;
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    assert!(ai.advanced_military_step(&mut g, 0, uid, &plan));
    assert!(g.wdist(g.units[&uid].pos, rally) < g.wdist(start, rally));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn opening_staging_respects_defense_guards_health_and_existing_wars() {
    for case in 0..8 {
        let (mut g, mut ai, mut plan, uid) = fixture();
        match case {
            0 => ai.conquest_opening.as_mut().unwrap().force.clear(),
            1 => {
                plan.threatened_city = g.city_at((6, 12));
            }
            2 => {
                plan.strategy = GrandStrategy::Recovery;
            }
            3 => {
                g.at_war.insert((0, 1));
            }
            4 => {
                g.units.get_mut(&uid).unwrap().hp = 1;
            }
            5 => {
                ai.settler_guards.insert(999, uid);
            }
            6 => {
                g.turn = g.standard_duration(CONQUEST_COMMIT_DEADLINE);
            }
            7 => {
                ai.conquest_opening.as_mut().unwrap().declared = Some(19);
            }
            _ => unreachable!(),
        }
        let before = g.units[&uid].pos;
        assert_eq!(
            ai.campaign_staging_step(&mut g, 0, uid, &plan),
            None,
            "case {case}"
        );
        assert_eq!(g.units[&uid].pos, before, "case {case}");
    }
}

#[test]
fn an_opening_unit_on_its_legal_rally_ring_holds_position() {
    let (mut g, mut ai, plan, uid) = fixture();
    g.units.get_mut(&uid).unwrap().pos = (14, 12);
    assert_eq!(ai.campaign_staging_step(&mut g, 0, uid, &plan), Some(false));
    assert_eq!(g.units[&uid].pos, (14, 12));
    assert!(g.units[&uid].fortified);
    assert!(!g.is_at_war(0, 1));
}
