//! `outclassed-siege-stands-down`.
use super::*;
use crate::ai::advanced::objective_board::{ObjectiveKey, ObjectiveKind};
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::game::Game;
use crate::name;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// Two empires on open grassland, capitals at (6, 8) and (20, 8), at war,
/// every tile explored, turn 200.
fn board() -> (Game, u32) {
    let capitals = [at(6, 8), at(20, 8)];
    let mut g = Game::new_full(2, 36, 22, 4394, 1_000, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
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
    for (pid, pos) in capitals.iter().enumerate() {
        g.found_city_for(pid, *pos, None);
    }
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.turn = 200;
    g.current = 0;
    let target = g.city_at(at(20, 8)).expect("their capital");
    (g, target)
}

fn spawn(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, pid, pos);
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    uid
}

/// Our medieval army of six at home; their defenders beside the target.
fn armies(g: &mut Game, defender: &str) -> Vec<u32> {
    let ours: Vec<u32> = [
        ("crossbowman", at(8, 8)),
        ("crossbowman", at(8, 9)),
        ("knight", at(7, 8)),
        ("knight", at(9, 8)),
        ("pikeman", at(9, 9)),
        ("trebuchet", at(7, 9)),
    ]
    .into_iter()
    .map(|(kind, pos)| spawn(g, kind, 0, pos))
    .collect();
    spawn(g, defender, 1, at(19, 8));
    spawn(g, defender, 1, at(21, 8));
    for pos in [at(17, 8), at(22, 10)] {
        // A picket of ours keeps the defenders in sight.
        let picket = spawn(g, "scout", 0, pos);
        g.units.get_mut(&picket).unwrap().hp = 100;
    }
    ours
}

fn conquest(g: &Game, target: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(g.cities[&target].owner),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn ai(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    if gene {
        ai.enable_outclassed_siege_stands_down();
    }
    ai
}

fn siege_row(ai: &AdvancedAi, target: u32) -> bool {
    ai.objective_board()
        .rows
        .iter()
        .any(|row| row.key == ObjectiveKey::Siege(target))
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "outclassed-siege-stands-down",
        |ai| ai.outclassed_siege_stands_down,
    );
}

/// Line Infantry (65) over Crossbows (30 defence), Knights (48) and a
/// Pikeman (41): every unit of ours sits 25 or more below the blow. Warriors
/// (20) outclass nobody.
#[test]
fn line_infantry_outclass_a_medieval_army_and_warriors_do_not() {
    let (mut g, target) = board();
    let ours = armies(&mut g, "line_infantry");
    let pos = g.cities[&target].pos;
    let read = field_outclass(&g, 0, 1, pos, &ours).expect("defenders in sight");
    assert!(read.outclassed(), "{read:?}");
    assert_eq!(read.ours, 6);
    assert!(field_outclasses(&g, 0, 1, pos));

    let (mut g, target) = board();
    let ours = armies(&mut g, "warrior");
    let pos = g.cities[&target].pos;
    let read = field_outclass(&g, 0, 1, pos, &ours).expect("defenders in sight");
    assert!(!read.outclassed(), "{read:?}");
    assert_eq!(read.one_blow, 0);
}

/// Assess the board on `turns` consecutive turns from `g.turn`, leaving
/// `g.turn` on the last; whether the Siege row stood on each.
fn assess(g: &mut Game, ai: &mut AdvancedAi, target: u32, turns: u32) -> Vec<bool> {
    let mut rows = Vec::new();
    for step in 0..turns {
        if step > 0 {
            g.turn += 1;
        }
        let plan = conquest(g, target);
        ai.rebuild_force_groups(g, 0, &plan);
        rows.push(siege_row(ai, target));
    }
    rows
}

/// The row stands through three outclassed turns and leaves the board on
/// the fourth with the gene on; with it off it never leaves. No Destroy row
/// is drawn onto the defenders it left.
#[test]
fn a_siege_outclassed_four_turns_running_leaves_the_board() {
    let (mut g, target) = board();
    armies(&mut g, "line_infantry");
    let mut off = ai(false);
    assert_eq!(assess(&mut g, &mut off, target, 5), vec![true; 5]);

    let (mut g, target) = board();
    armies(&mut g, "line_infantry");
    let mut on = ai(true);
    assert_eq!(
        assess(&mut g, &mut on, target, 5),
        vec![true, true, true, false, false]
    );
    let pos = g.cities[&target].pos;
    assert!(on
        .objective_board()
        .rows
        .iter()
        .all(|row| { row.kind != ObjectiveKind::Destroy || g.wdist(row.at, pos) > 4 }));
    assert!(on.siege_stood_down_at(&g, pos));
}

/// A warrior garrison is no reason to stand down; a falling city never is.
#[test]
fn a_weak_field_or_a_falling_city_keeps_the_row() {
    let (mut g, target) = board();
    armies(&mut g, "warrior");
    let mut on = ai(true);
    assert_eq!(assess(&mut g, &mut on, target, 5), vec![true; 5]);

    let (mut g, target) = board();
    armies(&mut g, "line_infantry");
    {
        let city = g.cities.get_mut(&target).unwrap();
        city.wall_hp = 0;
        city.hp = 60;
    }
    let mut on = ai(true);
    assert_eq!(
        assess(&mut g, &mut on, target, 5),
        vec![true; 5],
        "a falling city is still besieged"
    );
}

/// Out of sight is not gone: the row stays down for the window after the
/// last outclassed turn, then returns.
#[test]
fn the_stand_down_holds_for_its_window_then_lifts() {
    let (mut g, target) = board();
    armies(&mut g, "line_infantry");
    let mut on = ai(true);
    assess(&mut g, &mut on, target, STREAK_TURNS);
    assert!(!siege_row(&on, target));
    let last = g.turn;

    // The defenders drop out of sight.
    let theirs: Vec<u32> = g
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.id)
        .collect();
    for uid in theirs {
        g.remove_unit(uid);
    }
    g.turn = last + STAND_DOWN_TURNS - 1;
    assert_eq!(
        assess(&mut g, &mut on, target, 1),
        vec![false],
        "inside the window"
    );
    g.turn = last + STAND_DOWN_TURNS;
    assert_eq!(
        assess(&mut g, &mut on, target, 1),
        vec![true],
        "the window has passed"
    );
    assert!(!on.siege_stood_down_at(&g, g.cities[&target].pos));
}

/// A reading that sees defenders and is no longer outclassed lifts the
/// stand-down at once.
#[test]
fn a_field_that_no_longer_outclasses_lifts_it_at_once() {
    let (mut g, target) = board();
    armies(&mut g, "line_infantry");
    let mut on = ai(true);
    assess(&mut g, &mut on, target, STREAK_TURNS);
    assert!(!siege_row(&on, target));
    let theirs: Vec<u32> = g
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.id)
        .collect();
    for uid in theirs {
        g.remove_unit(uid);
    }
    spawn(&mut g, "warrior", 1, at(19, 8));
    g.turn += 1;
    assert_eq!(assess(&mut g, &mut on, target, 1), vec![true]);
}
