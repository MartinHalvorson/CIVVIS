use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::Ai;
use crate::game::Game;
use crate::name;

/// A flat grassland board of `capitals.len()` empires, every starting unit
/// cleared, the map explored and everyone met, nobody at war, turn 60.
fn flat_board(seed: u64, capitals: &[Pos]) -> Game {
    let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for (pid, pos) in capitals.iter().enumerate() {
        game.found_city_for(pid, *pos, None);
    }
    for pid in 0..capitals.len() {
        for other in 0..capitals.len() {
            if pid != other {
                game.players[pid].met.insert(other);
            }
        }
        game.players[pid]
            .explored
            .extend(game.map.tiles.keys().copied());
    }
    game.at_war.clear();
    game.turn = 60;
    game.current = 0;
    game
}

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn war(g: &mut Game, a: usize, b: usize) {
    g.at_war.insert((a.min(b), a.max(b)));
}

/// A unit with a new turn's movement and attack.
fn spawn(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, pid, pos);
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
    uid
}

fn gene_on() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_stacked_shooter_steps_out();
    ai
}

/// Our original capital at (10,10) and a rival's city at (28,10), at peace,
/// every tile within fourteen of our capital in sight.
fn board() -> (Game, Pos) {
    let home = at(10, 10);
    let mut g = flat_board(11, &[home, at(28, 10)]);
    let cid = g.city_at(home).expect("our capital");
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.is_capital = true;
        city.original_owner = 0;
    }
    g.host_observed = Arc::new(g.wdisk(home, 14).into_iter().collect());
    (g, home)
}

/// The tile `steps` east of `home` along its row.
fn east(g: &Game, home: Pos, steps: i32) -> Pos {
    let mut row: Vec<Pos> = g
        .wdisk(home, steps)
        .into_iter()
        .filter(|pos| g.wdist(*pos, home) == steps && pos.1 == home.1 && pos.0 > home.0)
        .collect();
    row.sort();
    row[0]
}

fn west(g: &Game, home: Pos, steps: i32) -> Pos {
    let mut row: Vec<Pos> = g
        .wdisk(home, steps)
        .into_iter()
        .filter(|pos| g.wdist(*pos, home) == steps && pos.1 == home.1 && pos.0 < home.0)
        .collect();
    row.sort();
    row[0]
}

/// Every unit's (id, position, health, owner): what a pass that did nothing
/// leaves unchanged.
fn board_print(g: &Game) -> Vec<(u32, Pos, i32, usize)> {
    let mut out: Vec<(u32, Pos, i32, usize)> = g
        .units
        .values()
        .map(|unit| (unit.id, unit.pos, unit.hp, unit.owner))
        .collect();
    out.sort();
    out
}

fn legal_shot(g: &Game, uid: u32, target: Pos) -> bool {
    let frame = g.player_vision_frame(0);
    let viewers = g.visibility_viewers(0);
    g.ranged_order_is_legal(0, uid, target, frame.as_ref(), &viewers)
}

#[test]
fn stacked_shooter_steps_out_ships_off_and_is_registered() {
    opt_in_off_in_both_controllers("stacked-shooter-steps-out", |ai| {
        ai.stacked_shooter_steps_out
    });
}

#[test]
fn off_a_stacked_centre_stays_stacked_and_its_shooter_cannot_fire() {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let archer = spawn(&mut g, "archer", 0, home);
    spawn(&mut g, "warrior", 0, home);
    let gun = east(&g, home, 2);
    spawn(&mut g, "catapult", 1, gun);
    assert!(g.shooter_stacked_in_city(archer), "the host rule reads the stack");
    assert!(!legal_shot(&g, archer, gun), "stacked, the Archer has no shot");
    let mut ai = AdvancedAi::new();
    let before = board_print(&g);
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 0);
    assert_eq!(board_print(&g), before, "the gene off touches nothing");
}

#[test]
fn the_archer_keeps_the_centre_and_the_warrior_steps_beside_it() {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let archer = spawn(&mut g, "archer", 0, home);
    let warrior = spawn(&mut g, "warrior", 0, home);
    let gun = east(&g, home, 2);
    spawn(&mut g, "catapult", 1, gun);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 1);
    assert_eq!(g.units[&archer].pos, home, "the shooter keeps the centre");
    assert_eq!(
        g.wdist(g.units[&warrior].pos, home),
        1,
        "the Warrior stands beside the centre"
    );
    assert!(!g.shooter_stacked_in_city(archer));
    assert!(
        legal_shot(&g, archer, gun),
        "alone on the centre, the Archer has the Catapult in reach"
    );
    // Once unstacked there is nothing left to do.
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 0);
}

#[test]
fn of_two_archers_the_other_steps_to_a_tile_with_a_shot() {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let first = spawn(&mut g, "archer", 0, home);
    let second = spawn(&mut g, "archer", 0, home);
    // The weaker one steps out: the keeper is the healthier.
    g.units.get_mut(&second).unwrap().hp = 80;
    // A Swordsman three tiles east: out of reach of the centre.
    let enemy = east(&g, home, 3);
    spawn(&mut g, "swordsman", 1, enemy);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 1);
    assert_eq!(g.units[&first].pos, home, "the healthier Archer keeps it");
    let stand = g.units[&second].pos;
    assert_eq!(g.wdist(stand, home), 1);
    assert!(
        g.wdist(stand, enemy) <= 2,
        "it stepped toward its shot: {stand:?} for {enemy:?}"
    );
    assert!(g.units[&second].moves_left > 0.0, "with movement to fire");
    assert!(legal_shot(&g, second, enemy), "and the shot is legal");
}

#[test]
fn no_enemy_soldier_near_leaves_the_stack() {
    let (mut g, home) = board();
    spawn(&mut g, "archer", 0, home);
    spawn(&mut g, "archer", 0, home);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 0, "an empty board");
    // A rival's soldier at peace is no enemy.
    let near = east(&g, home, 2);
    let peace = spawn(&mut g, "swordsman", 1, near);
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 0, "at peace");
    g.remove_unit(peace);
    // At war: a Scout is recon, and a soldier five tiles out is too far.
    war(&mut g, 0, 1);
    let scout = spawn(&mut g, "scout", 1, near);
    let far = east(&g, home, 5);
    spawn(&mut g, "swordsman", 1, far);
    let before_war = board_print(&g);
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 0);
    assert_eq!(board_print(&g), before_war);
    g.remove_unit(scout);
    // An enemy we do not see is not read.
    let mut dark = g.clone();
    let four = east(&dark, home, 4);
    spawn(&mut dark, "swordsman", 1, four);
    dark.host_observed = Arc::new(dark.wdisk(home, 2).into_iter().collect());
    assert_eq!(ai.step_stacked_soldiers_out(&mut dark, 0), 0, "out of sight");
}

#[test]
fn two_soldiers_without_a_shooter_are_not_this_genes() {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    spawn(&mut g, "warrior", 0, home);
    spawn(&mut g, "spearman", 0, home);
    let near = east(&g, home, 2);
    spawn(&mut g, "swordsman", 1, near);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 0);
}

#[test]
fn a_civilians_guard_keeps_the_centre_and_the_shooter_steps_out() {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let settler = spawn(&mut g, "settler", 0, home);
    let guard = spawn(&mut g, "warrior", 0, home);
    g.units.get_mut(&guard).unwrap().linked_to = Some(settler);
    let archer = spawn(&mut g, "archer", 0, home);
    let near = east(&g, home, 3);
    spawn(&mut g, "swordsman", 1, near);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 1);
    assert_eq!(g.units[&guard].pos, home, "the guard keeps its charge");
    assert_eq!(g.wdist(g.units[&archer].pos, home), 1);
}

#[test]
fn the_step_takes_cover_and_shuns_the_enemy() {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let archer = spawn(&mut g, "archer", 0, home);
    let warrior = spawn(&mut g, "warrior", 0, home);
    // An enemy Warrior two tiles east: the eastern tiles beside the centre
    // stand beside it. The western tile is a wooded hill.
    let enemy = east(&g, home, 2);
    spawn(&mut g, "warrior", 1, enemy);
    let cover = west(&g, home, 1);
    {
        let tile = g.map.tiles.get_mut(&cover).unwrap();
        tile.hills = true;
    }
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_out(&mut g, 0), 1);
    assert_eq!(g.units[&archer].pos, home);
    let stand = g.units[&warrior].pos;
    assert!(
        g.wdist(stand, enemy) >= 2,
        "it does not step beside the enemy: {stand:?}"
    );
    assert_eq!(stand, cover, "the hill is the best cover");
}

/// A whole turn of the live-like seat (battle planner and
/// `shooters-man-the-capital` on) with two Archers stacked on the capital's
/// centre and an enemy Catapult two tiles east: the shots our Archers spent
/// and the Catapult's health.
fn a_turn(on: bool) -> (usize, i32, Vec<String>) {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let archers = [
        spawn(&mut g, "archer", 0, home),
        spawn(&mut g, "archer", 0, home),
    ];
    let gun_at = east(&g, home, 2);
    let gun = spawn(&mut g, "catapult", 1, gun_at);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai.enable_shooters_man_the_capital();
    if on {
        ai.enable_stacked_shooter_steps_out();
    }
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.take_turn(&mut g, 0);
    let notes: Vec<String> = ai
        .base
        .journal
        .since(u64::MAX)
        .thoughts
        .into_iter()
        .filter(|thought| {
            thought.detail.contains("stacked-shooter-steps-out")
                || thought.detail.contains("shooters-man-the-capital")
        })
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect();
    let fired = archers
        .iter()
        .filter(|uid| g.units.get(uid).is_some_and(|unit| unit.attacks_left == 0))
        .count();
    (fired, g.units.get(&gun).map_or(0, |unit| unit.hp), notes)
}

#[test]
fn a_whole_turn_on_both_archers_fire_and_off_neither_does() {
    let (fired_off, gun_off, notes_off) = a_turn(false);
    let (fired_on, gun_on, notes_on) = a_turn(true);
    assert_eq!(
        fired_off, 0,
        "off, the stacked Archers are held on the centre and cannot fire: {notes_off:#?}"
    );
    assert_eq!(
        fired_on, 2,
        "on, one keeps the centre and one steps out, and both fire: {notes_on:#?}"
    );
    assert!(
        gun_on < gun_off,
        "the Catapult takes the shots: {gun_on} on, {gun_off} off"
    );
    assert!(notes_on
        .iter()
        .any(|note| note.contains("stacked-shooter-steps-out")));
    assert!(!notes_off
        .iter()
        .any(|note| note.contains("stacked-shooter-steps-out")));
}
