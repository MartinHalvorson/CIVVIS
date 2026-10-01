use super::*;

fn fixture() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 40, 24, 936101, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 12), None);
    g.found_city_for(0, (10, 18), None);
    let target = g.found_city_for(1, (22, 12), None);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 170;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: (22, 12),
        body_unit: crate::name!("cavalry"),
        body_is_cavalry: true,
        opened_at_war: true,
        phase: AirSurgePhase::Exploit,
        appointed_turn: 150,
        tech_turn: Some(160),
        declared_turn: None,
        last_reviewed_turn: 170,
        recovery_assessments: 0,
    });
    (g, ai, target)
}

/// A plan that names no campaign, so only the surge can raise a raid.
fn peace_plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 170,
        rush: false,
    }
}

/// A mine in the target's territory, three tiles from a spare cavalry.
fn mine_near(g: &mut Game, target: u32) -> Pos {
    let pos = (19, 12);
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.owner_city = Some(target);
    tile.improvement = Some(crate::name!("mine"));
    tile.pillaged = false;
    let owned = &mut g.cities.get_mut(&target).unwrap().owned_tiles;
    if !owned.contains(&pos) {
        owned.push(pos);
    }
    pos
}

#[test]
fn spare_cavalry_pillages_behind_the_wing_and_the_takers_stay() {
    let (mut g, mut ai, target) = fixture();
    let mine = mine_near(&mut g, target);
    // Two capture bodies nearest the objective, and one spare cavalry.
    let takers = [
        g.spawn_test_unit("cavalry", 0, (20, 12)),
        g.spawn_test_unit("cavalry", 0, (20, 13)),
    ];
    let raider = g.spawn_test_unit("cavalry", 0, (16, 12));
    let gold = g.players[0].gold;
    // The host pillages where the unit stands when the order runs, so a
    // raid that walks only walks on this frame.
    let raiders = ai.plan_air_surge_raids(&mut g, 0, &peace_plan(), &BTreeSet::new());
    assert_eq!(raiders, BTreeSet::from([raider]));
    assert_eq!(g.units[&raider].pos, mine);
    assert!(!g.map.tiles[&mine].pillaged, "no pillage queued behind the walk");
    for taker in takers {
        assert!(!g.units[&taker].acted, "the capture keeps its bodies");
    }
    // The next frame finds it on the tile and pillages from there.
    let unit = g.units.get_mut(&raider).unwrap();
    unit.acted = false;
    unit.moves_left = 2.0;
    let raiders = ai.plan_air_surge_raids(&mut g, 0, &peace_plan(), &BTreeSet::new());
    assert_eq!(raiders, BTreeSet::from([raider]));
    assert!(g.map.tiles[&mine].pillaged);
    assert!(g.players[0].gold > gold, "a mine's plunder is Gold");
}

#[test]
fn no_raid_outside_a_domination_surge_at_war_or_into_danger() {
    for case in ["peace", "legacy", "lane", "no_plan", "arm", "danger"] {
        let (mut g, mut ai, target) = fixture();
        mine_near(&mut g, target);
        for pos in [(20, 12), (20, 13)] {
            g.spawn_test_unit("cavalry", 0, pos);
        }
        let raider = g.spawn_test_unit("cavalry", 0, (16, 12));
        match case {
            "peace" => g.at_war.clear(),
            "legacy" => {
                ai.disable_air_surge_2();
                ai.air_surge = true;
            }
            "lane" => ai.victory_target = Some(VictoryTarget::Culture),
            "no_plan" => ai.air_surge_plan = None,
            "arm" => ai.air_surge_plan.as_mut().unwrap().phase = AirSurgePhase::Arm,
            "danger" => {
                for pos in [(18, 11), (18, 13), (19, 11), (19, 13)] {
                    g.spawn_test_unit("tank", 1, pos);
                }
            }
            _ => unreachable!(),
        }
        let raiders = ai.plan_air_surge_raids(&mut g, 0, &peace_plan(), &BTreeSet::new());
        assert!(!raiders.contains(&raider), "{case}");
    }
}

/// Without a surge, a Domination war's land campaign still raises raids:
/// live game seven sat at war at 2.5x power on 25 science a turn.
#[test]
fn a_domination_land_war_sends_spare_cavalry_raiding() {
    for case in ["campaign", "expansion", "peace"] {
        let (mut g, mut ai, target) = fixture();
        ai.air_surge_plan = None;
        let mine = mine_near(&mut g, target);
        for pos in [(20, 12), (20, 13)] {
            g.spawn_test_unit("cavalry", 0, pos);
        }
        // Staged on the siege ring: stays.
        let staged = g.spawn_test_unit("cavalry", 0, (21, 11));
        let raider = g.spawn_test_unit("cavalry", 0, (16, 12));
        let mut plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 170,
            rush: false,
        };
        match case {
            "campaign" => {}
            "expansion" => plan.strategy = GrandStrategy::Expansion,
            "peace" => g.at_war.clear(),
            _ => unreachable!(),
        }
        let raiders = ai.plan_air_surge_raids(&mut g, 0, &plan, &BTreeSet::new());
        assert!(!raiders.contains(&staged), "{case}");
        if case == "campaign" {
            assert_eq!(raiders, BTreeSet::from([raider]), "{case}");
            assert_eq!(g.units[&raider].pos, mine);
        } else {
            assert!(raiders.is_empty(), "{case}");
        }
    }
}
