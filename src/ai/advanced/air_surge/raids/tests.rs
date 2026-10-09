use super::*;

fn fixture() -> (Game, AdvancedAi, u32) {
    fixture_with_players(2)
}

fn fixture_with_players(players: usize) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(players, 40, 24, 936101, 650, 0, false);
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
    assert!(
        !g.map.tiles[&mine].pillaged,
        "no pillage queued behind the walk"
    );
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

/// `invest-keeps-its-cavalry`: a land siege investing its city keeps the
/// cavalry within the muster's reach of it, though the raid would take them
/// off a train that only stands in Stage; off, the train's cavalry rides out.
/// Live Emperor game 219 closed on the Maori capital while its Knights rode
/// out to raid.
#[test]
fn an_investing_siege_keeps_its_cavalry_from_the_raid() {
    use super::super::super::siege_train::{Siege, SiegeStage};
    for (gene, stage, rides) in [
        (false, SiegeStage::Invest, true),
        (true, SiegeStage::Invest, false),
        (true, SiegeStage::Stage, true),
    ] {
        let (mut g, mut ai, target) = fixture();
        ai.air_surge_plan = None;
        if gene {
            ai.enable_invest_keeps_its_cavalry();
        }
        mine_near(&mut g, target);
        for pos in [(20, 12), (20, 13)] {
            g.spawn_test_unit("cavalry", 0, pos);
        }
        let member = g.spawn_test_unit("cavalry", 0, (16, 12));
        ai.sieges.insert(
            target,
            Siege {
                stage,
                taker: None,
                entered: 160,
                assessed: g.turn,
                posts: Default::default(),
                short_since: None,
            },
        );
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 170,
            rush: false,
        };
        let raiders = ai.plan_air_surge_raids(&mut g, 0, &plan, &BTreeSet::new());
        assert_eq!(
            raiders.contains(&member),
            rides,
            "gene {gene}, {stage:?}: the member six tiles out"
        );
    }
}

/// See `RAID_TOURISM_DENIAL`: against the countered culture rival, a raid
/// takes its Theater Square over a Campus beside it.
#[test]
fn a_raid_on_the_culture_rival_takes_its_theater_square() {
    let (mut g, mut ai, target) = fixture();
    ai.deny_leaders = true;
    let mut place = |g: &mut Game, pos: Pos, district: &str| {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.owner_city = Some(target);
        tile.district = Some(crate::name::Name::new(district));
        tile.improvement = None;
        tile.pillaged = false;
        let city = g.cities.get_mut(&target).unwrap();
        if !city.owned_tiles.contains(&pos) {
            city.owned_tiles.push(pos);
        }
        city.districts.insert(crate::name::Name::new(district), pos);
    };
    place(&mut g, (19, 11), "campus");
    place(&mut g, (19, 13), "theater_square");
    let raider = g.spawn_test_unit("cavalry", 0, (16, 12));
    // The rival is one visitor from its culture victory.
    let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
    stats.insert(
        0,
        crate::game::ObservedPublicEmpireStats {
            domestic_tourists: Some(20),
            foreign_tourists: Some(0),
            ..Default::default()
        },
    );
    stats.insert(
        1,
        crate::game::ObservedPublicEmpireStats {
            domestic_tourists: Some(20),
            foreign_tourists: Some(19),
            ..Default::default()
        },
    );
    assert!(ai.culture_clock_rival(&g, 1), "fixture: the culture clock");
    let pick = |ai: &AdvancedAi, g: &Game| {
        ai.air_surge_best_raid(g, 0, raider, (22, 12))
            .map(|(_, tile, _)| tile)
    };
    assert_eq!(
        pick(&ai, &g),
        Some((19, 11)),
        "the control: the Campus pays more"
    );
    ai.enable_raids_cut_tourism();
    assert_eq!(
        pick(&ai, &g),
        Some((19, 13)),
        "the Theater Square under the gene"
    );
}

/// An urgent rival redirects the army while the existing wing still fights
/// its first opponent. Keep the cavalry available for that active front;
/// a profitable mine at the old objective must not take it back.
#[test]
fn an_urgent_other_front_holds_the_old_wings_spare_cavalry() {
    let (mut g, mut ai, target) = fixture_with_players(3);
    mine_near(&mut g, target);
    for pos in [(20, 12), (20, 13)] {
        g.spawn_test_unit("cavalry", 0, pos);
    }
    let raider = g.spawn_test_unit("cavalry", 0, (16, 12));
    let origin = g.units[&raider].pos;
    let urgent = g.found_city_for(2, (34, 2), None);
    g.record_contact(0, 2);
    g.at_war.extend([(0, 2), (2, 0)]);
    g.players[2].dvp = 19;
    ai.deny_leaders = true;
    ai.enable_dvp_leader_is_the_front();
    ai.maintain_air_surge(&g, 0);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().target_player, 1);
    assert!(
        ai.air_surge_status.denial_elsewhere,
        "the real lifecycle derives the urgent handoff; the fixture does not set its flag"
    );
    let campaign = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(urgent),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let raids = ai.plan_air_surge_raids(&mut g, 0, &campaign, &BTreeSet::new());
    assert!(
        !raids.contains(&raider),
        "a profitable old-front raid must yield to the urgent campaign"
    );
    assert_eq!(g.units[&raider].pos, origin);
    assert!(
        !g.units[&raider].acted,
        "the campaign still has this unit's action"
    );
}
