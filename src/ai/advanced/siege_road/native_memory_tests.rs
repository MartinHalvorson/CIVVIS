use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};

const TARGET: Pos = (32, 6);

fn rebuilt_board(rotation: usize, turn: u32) -> (Game, u32, u32, u32) {
    let mut game = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let cities = [(0, (4, 6)), (1, (18, 6)), (2, TARGET)];
    for index in 0..cities.len() {
        let (owner, pos) = cities[(index + rotation) % cities.len()];
        game.found_city_for(owner, pos, None);
    }
    let screen = game.city_at((18, 6)).unwrap();
    for tile in game.map.tiles.values_mut() {
        if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
            tile.owner_city = Some(screen);
        }
    }
    game.players[1].borders_enforced = Some(true);
    game.at_war.clear();
    game.at_war.insert((0, 2));
    game.at_war.insert((2, 0));
    game.turn = turn;
    game.current = 0;
    let units = if rotation.is_multiple_of(2) {
        ["warrior", "archer"]
    } else {
        ["archer", "warrior"]
    };
    for kind in units {
        let pos = if kind == "warrior" { (8, 6) } else { (7, 6) };
        game.spawn_test_unit(kind, 0, pos);
    }
    let warrior = game
        .units
        .values()
        .find(|unit| unit.kind == "warrior")
        .unwrap()
        .id;
    let archer = game
        .units
        .values()
        .find(|unit| unit.kind == "archer")
        .unwrap()
        .id;
    let target = game.city_at(TARGET).unwrap();
    (game, target, warrior, archer)
}

fn target_plan(game: &Game, target: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    }
}

#[test]
fn road_tally_follows_city_and_surviving_units_without_the_envoy_treatment() {
    let (previous, old_target, old_warrior, old_archer) = rebuilt_board(0, 60);
    let (next, target, warrior, archer) = rebuilt_board(1, 61);
    assert_ne!(old_target, target);
    assert_ne!(old_warrior, warrior);
    assert_ne!(old_archer, archer);
    let mut agent = AdvancedAi::new();
    agent.enable_siege_target_needs_a_road();
    assert!(!agent.envoys_open_the_siege_road);
    agent.siege_road_tally.insert(
        old_target,
        RoadTally {
            turn: 60,
            held: [(old_warrior, Some(1)), (u32::MAX, None)].into(),
            marched: [old_archer, u32::MAX - 1].into(),
            counted: 60,
            streak: 2,
        },
    );
    let identities = [(old_warrior, warrior), (old_archer, archer)].into();
    agent.remap_siege_memory(&previous, &next, &identities);
    agent.remap_unit_memory(&identities);
    assert_eq!(agent.siege_road_tally.len(), 1);
    let tally = agent
        .siege_road_tally
        .get(&target)
        .expect("the same city's road tally");
    assert_eq!(tally.held, [(warrior, Some(1))].into());
    assert_eq!(tally.marched, [archer].into());
    assert_eq!((tally.turn, tally.counted, tally.streak), (60, 60, 2));
    assert!(!agent.siege_road_tally.contains_key(&old_target));
}

#[test]
fn stood_down_road_follows_its_city_and_still_reopens_with_the_border() {
    let (previous, old_target, old_warrior, old_archer) = rebuilt_board(0, 60);
    let (mut next, target, warrior, archer) = rebuilt_board(1, 65);
    let mut agent = AdvancedAi::new();
    agent.enable_siege_target_needs_a_road();
    agent.enable_siege_buys_the_passage();
    agent.capture_stood_down.insert(old_target, 80);
    agent.siege_road_closed.insert(
        old_target,
        RoadClosed {
            seat: 0,
            owner: 2,
            blocker: Some(1),
        },
    );
    let identities = [(old_warrior, warrior), (old_archer, archer)].into();
    agent.remap_siege_memory(&previous, &next, &identities);
    agent.remap_city_memory(&previous, &next, &identities);
    agent.remap_unit_memory(&identities);
    assert_eq!(agent.siege_road_closed.len(), 1);
    let closed = agent
        .siege_road_closed
        .get(&target)
        .expect("the same city's road closure");
    assert_eq!((closed.seat, closed.owner, closed.blocker), (0, 2, Some(1)));
    assert!(!agent.siege_road_closed.contains_key(&old_target));
    assert!(agent.capture_stood_down_holds(&next, target));
    assert_eq!(agent.siege_passage_blockers(&next, 0), [1].into());
    next.at_war.insert((0, 1));
    next.at_war.insert((1, 0));
    assert!(agent.siege_road_reopened(&next, target));
    assert!(!agent.capture_stood_down_holds(&next, target));
    assert!(agent.siege_passage_blockers(&next, 0).is_empty());
}

#[test]
fn three_blocked_turns_stand_down_even_when_each_rebuild_renumbers_cities() {
    for renumber in [false, true] {
        let (mut game, mut target, mut warrior, mut archer) = rebuilt_board(0, 60);
        let mut agent = AdvancedAi::targeting(VictoryTarget::Domination);
        agent.enable_capture_go_or_stand_down_2();
        agent.enable_siege_target_needs_a_road();
        for elapsed in 0..ROAD_HOLD_TURNS {
            agent.plan = Some(target_plan(&game, target));
            assert!(game
                .route_step_dry(warrior, TARGET, STAGING_FAR, STAGE_DRY_LIMIT)
                .is_none());
            assert!(agent
                .note_stage_march(&game, warrior, target, &StageMarch::Hold { wet: 30 })
                .is_some());
            agent.reconcile_commitments(&mut game, 0);
            let expected = elapsed + 1 == ROAD_HOLD_TURNS;
            assert_eq!(
                agent.capture_stood_down_holds(&game, target),
                expected,
                "renumber={renumber}, blocked turn {}",
                elapsed + 1
            );
            if !expected {
                assert_eq!(agent.siege_road_tally[&target].streak, elapsed + 1);
            }
            let rotation = if renumber { elapsed as usize + 1 } else { 0 };
            let (mut next, new_target, new_warrior, new_archer) =
                rebuilt_board(rotation, game.turn + 1);
            next.players[0].counters = game.players[0].counters.clone();
            let identities = [(warrior, new_warrior), (archer, new_archer)].into();
            agent.remap_siege_memory(&game, &next, &identities);
            agent.remap_city_memory(&game, &next, &identities);
            agent.remap_unit_memory(&identities);
            game = next;
            target = new_target;
            warrior = new_warrior;
            archer = new_archer;
        }
        assert!(agent.capture_stood_down_holds(&game, target));
        assert!(agent.siege_road_cuts_off(&game, 0, 2));
        assert_eq!(
            game.players[0]
                .counters
                .get("commit:capture:road_stand_downs"),
            Some(&1)
        );
    }
}

#[test]
fn a_city_missing_from_the_rebuilt_board_drops_both_road_records() {
    let (previous, target, warrior, archer) = rebuilt_board(0, 60);
    let mut next = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
    for pos in [(4, 6), (18, 6), (31, 6)] {
        next.found_city_for(1, pos, None);
    }
    assert!(next.city_at(TARGET).is_none());
    let mut agent = AdvancedAi::new();
    agent.enable_siege_target_needs_a_road();
    agent.siege_road_tally.insert(
        target,
        RoadTally {
            turn: 60,
            held: [(warrior, Some(1))].into(),
            marched: [archer].into(),
            counted: 60,
            streak: 2,
        },
    );
    agent.siege_road_closed.insert(
        target,
        RoadClosed {
            seat: 0,
            owner: 2,
            blocker: None,
        },
    );
    agent.remap_siege_memory(&previous, &next, &BTreeMap::new());
    agent.remap_unit_memory(&BTreeMap::new());
    assert!(agent.siege_road_tally.is_empty());
    assert!(agent.siege_road_closed.is_empty());
}
