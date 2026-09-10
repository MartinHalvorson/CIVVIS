use super::super::*;

fn at(q: i32, r: i32) -> Pos {
    (q, r)
}

fn islands() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 3431, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.owner_city = None;
    }
    let objective = at(13, 8);
    for center in [at(3, 8), at(2, 15), objective] {
        let radius = if center == objective { 2 } else { 1 };
        for pos in g.wdisk(center, radius) {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("grassland");
        }
    }
    g.map.tiles.get_mut(&at(12, 8)).unwrap().terrain = crate::name!("coast");
    g.found_city_for(0, at(3, 8), None);
    g.found_city_for(0, at(2, 15), None);
    let target = g.found_city_for(1, objective, None);
    for pos in g.wdisk(objective, 2) {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        if tile.terrain == "grassland" {
            tile.owner_city = Some(target);
        }
    }
    g.turn = 150;
    g.current = 0;
    g.at_war.clear();
    for pid in 0..2 {
        g.players[pid].civ = "Rome".into();
        g.players[pid].met.extend([0, 1]);
        g.players[pid].gold = 1000.0;
        g.players[pid].gold_per_turn = 20.0;
    }
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    g.players[0].techs.insert(crate::name!("cartography"));
    g.players[1].civics.insert(crate::name!("early_empire"));
    g.players[0].denounced_since.insert(1, 140);
    g.players[0].denounced_until.insert(1, 200);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_war_policy_via_board();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn coastal_campaign_can_assemble_offshore_declare_and_land() {
    let (mut g, mut ai, plan) = islands();
    let objective = g.cities[&plan.target_city.unwrap()].pos;
    let march = g.spawn_test_unit("musketman", 0, at(6, 8));
    let before = g.units[&march].pos;
    assert_eq!(
        ai.campaign_staging_step(&mut g, 0, march, &plan),
        Some(true)
    );
    assert_ne!(
        g.units[&march].pos, before,
        "an invasion must cross the channel before declaring"
    );
    g.remove_unit(march);
    let mut army = Vec::new();
    for pos in [at(10, 8), at(10, 9), at(10, 10), at(11, 11)] {
        let uid = g.spawn_test_unit("musketman", 0, pos);
        assert!(g.is_embarked(&g.units[&uid]));
        army.push(uid);
    }
    assert_eq!(ai.staged_campaign_units(&g, 0, 1, objective).len(), 4);
    assert!(matches!(
        ai.war_policy_declaration(&g, 0, 1, &plan),
        Some(Ok(()))
    ));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "a prepared landing force must be allowed to open its campaign"
    );
    g.apply(
        0,
        &Action::Move {
            unit: army[0],
            to: at(11, 8),
        },
    )
    .unwrap();
    assert!(
        !g.is_embarked(&g.units[&army[0]]),
        "the staged unit must actually land"
    );
}

#[test]
fn offshore_staging_requires_embarkation_and_an_enemy_landing_shore() {
    let (mut g, ai, plan) = islands();
    let objective = g.cities[&plan.target_city.unwrap()].pos;
    let uid = g.spawn_test_unit("musketman", 0, at(3, 8));
    g.players[0].techs.remove(&crate::name!("shipbuilding"));
    g.players[0].techs.remove(&crate::name!("cartography"));
    assert!(!ai.campaign_staging_position(&g, 0, 1, uid, objective, at(10, 8)));
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    assert!(ai.campaign_staging_position(&g, 0, 1, uid, objective, at(10, 8)));
    assert!(
        !ai.campaign_staging_position(&g, 0, 1, uid, objective, at(8, 8)),
        "open water five tiles away is not a landing station"
    );
    let ship = g.spawn_test_unit("galley", 0, at(10, 8));
    assert!(!ai.campaign_staging_position(&g, 0, 1, ship, objective, at(10, 8)));
}

#[test]
fn an_accessible_dry_stage_is_preferred_to_waiting_offshore() {
    let (mut g, mut ai, plan) = islands();
    let dry = at(9, 9);
    g.map.tiles.get_mut(&dry).unwrap().terrain = crate::name!("grassland");
    let uid = g.spawn_test_unit("musketman", 0, at(9, 8));
    assert_eq!(ai.campaign_staging_step(&mut g, 0, uid, &plan), Some(true));
    assert_eq!(g.units[&uid].pos, dry);
}
