use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::Game;
use crate::name;

/// A flat grassland board of `capitals.len()` empires, no rivers, every
/// starting unit cleared, the map explored and everyone met, nobody at war,
/// turn 15.
fn flat_board(seed: u64, capitals: &[Pos]) -> Game {
    let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    game.map.clear_rivers();
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
    game.turn = 15;
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
    ai.enable_stacked_soldiers_step_apart();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

/// Our capital at offset (10,10) and a rival's city at (28,10), every tile
/// within fourteen of our capital in sight.
fn board() -> (Game, Pos) {
    let home = at(10, 10);
    let mut g = flat_board(15, &[home, at(28, 10)]);
    let cid = g.city_at(home).expect("our capital");
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.is_capital = true;
        city.original_owner = 0;
    }
    g.host_observed = Arc::new(g.wdisk(home, 14).into_iter().collect());
    (g, home)
}

/// Every unit's (id, position, health, owner, movement): what a pass that did
/// nothing leaves unchanged.
fn board_print(g: &Game) -> Vec<(u32, Pos, i32, usize, i64)> {
    let mut out: Vec<(u32, Pos, i32, usize, i64)> = g
        .units
        .values()
        .map(|unit| {
            (
                unit.id,
                unit.pos,
                unit.hp,
                unit.owner,
                (unit.moves_left * 100.0).round() as i64,
            )
        })
        .collect();
    out.sort();
    out
}

/// The journal lines this gene wrote.
fn gene_lines(ai: &AdvancedAi) -> Vec<String> {
    ai.base
        .journal
        .since(u64::MAX)
        .thoughts
        .into_iter()
        .filter(|thought| thought.detail.contains("stacked-soldiers-step-apart:"))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect()
}

/// Kraków, seat p2 of `hotseat-20261128T031456Z`, turn 15, moved onto this
/// board: Kraków's (12,36) is our (10,10), and the three barbarians the
/// turn's first frame showed at (14,35), (13,34) and (13,35) stand at the
/// same offsets from it. The Warrior that held the centre was at 84 health,
/// the one just trained at 100. Returns the board, the centre, the old and
/// the new Warrior and the barbarians' tiles.
fn krakow_turn_15() -> (Game, Pos, u32, u32, Vec<Pos>) {
    let (mut g, home) = board();
    war(&mut g, 0, 1);
    let old = spawn(&mut g, "warrior", 0, home);
    g.units.get_mut(&old).unwrap().hp = 84;
    let new = spawn(&mut g, "warrior", 0, home);
    let raiders = vec![at(12, 9), at(11, 8), at(11, 9)];
    for pos in &raiders {
        spawn(&mut g, "warrior", 1, *pos);
    }
    (g, home, old, new, raiders)
}

#[test]
fn stacked_soldiers_step_apart_ships_off_and_is_registered() {
    opt_in_off_in_both_controllers("stacked-soldiers-step-apart", |ai| {
        ai.stacked_soldiers_step_apart
    });
}

#[test]
fn off_the_two_warriors_on_krakow_stay_stacked() {
    let (mut g, home, old, new, _) = krakow_turn_15();
    let mut ai = AdvancedAi::new();
    ai.base.journal = crate::reasoning::Journal::recording();
    let before = board_print(&g);
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 0);
    assert_eq!(board_print(&g), before, "the gene off touches nothing");
    assert_eq!(g.units[&old].pos, home);
    assert_eq!(g.units[&new].pos, home);
    assert!(gene_lines(&ai).is_empty());
}

#[test]
fn on_the_wounded_warrior_keeps_krakow_and_the_fresh_one_steps_away_from_the_raiders() {
    let (mut g, home, old, new, raiders) = krakow_turn_15();
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 1);
    assert_eq!(
        g.units[&old].pos, home,
        "the wounded Warrior keeps the centre, where it heals"
    );
    let stand = g.units[&new].pos;
    assert_eq!(g.wdist(stand, home), 1, "the fresh one stands beside it");
    assert!(
        g.can_stop(old, home) && g.can_stop(new, stand),
        "nobody is stacked any more"
    );
    assert!(
        raiders.iter().all(|raider| g.wdist(stand, *raider) >= 2),
        "it does not step beside a raider: {stand:?} for {raiders:?}"
    );
    let lines = gene_lines(&ai);
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert!(
        lines[0].contains("steps off the tile it shared"),
        "{lines:#?}"
    );
    // Once apart there is nothing left to do.
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 0);
}

#[test]
fn a_soldier_alone_or_beside_a_civilian_or_a_ship_is_no_stack() {
    let (mut g, home) = board();
    let warrior = spawn(&mut g, "warrior", 0, home);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 0, "one soldier");
    // A Builder is on the civilian layer.
    spawn(&mut g, "builder", 0, home);
    // A Galley in port is on the naval layer: make the tile west of the
    // centre sea so the port is a port.
    let sea = at(9, 10);
    g.map.tiles.get_mut(&sea).unwrap().terrain = name!("coast");
    spawn(&mut g, "galley", 0, home);
    let before = board_print(&g);
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 0);
    assert_eq!(board_print(&g), before);
    assert_eq!(g.units[&warrior].pos, home);
    assert!(gene_lines(&ai).is_empty());
}

#[test]
fn a_soldier_with_no_movement_keeps_the_tile_and_the_fresh_one_steps() {
    let (mut g, home) = board();
    // The stronger Spearman would keep the centre, but it has spent its
    // movement: the Warrior that can still step is the one that goes.
    let spear = spawn(&mut g, "spearman", 0, home);
    g.units.get_mut(&spear).unwrap().moves_left = 0.0;
    let warrior = spawn(&mut g, "warrior", 0, home);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 1);
    assert_eq!(g.units[&spear].pos, home);
    assert_eq!(g.wdist(g.units[&warrior].pos, home), 1);
}

#[test]
fn the_stronger_soldier_keeps_the_tile_when_both_can_step() {
    let (mut g, home) = board();
    let warrior = spawn(&mut g, "warrior", 0, home);
    let spear = spawn(&mut g, "spearman", 0, home);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 1);
    assert_eq!(g.units[&spear].pos, home, "the Spearman keeps the centre");
    assert_eq!(g.wdist(g.units[&warrior].pos, home), 1);
}

#[test]
fn a_civilians_guard_keeps_its_charge() {
    let (mut g, home) = board();
    let settler = spawn(&mut g, "settler", 0, home);
    let guard = spawn(&mut g, "warrior", 0, home);
    g.units.get_mut(&guard).unwrap().linked_to = Some(settler);
    let spear = spawn(&mut g, "spearman", 0, home);
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 1);
    assert_eq!(
        g.units[&guard].pos, home,
        "the weaker guard keeps its Settler's tile"
    );
    assert_eq!(g.units[&settler].pos, home);
    assert_eq!(g.wdist(g.units[&spear].pos, home), 1);
}

#[test]
fn two_soldiers_with_no_movement_stay_and_the_journal_says_so() {
    let (mut g, home) = board();
    for kind in ["warrior", "spearman"] {
        let uid = spawn(&mut g, kind, 0, home);
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    let mut ai = gene_on();
    let before = board_print(&g);
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 0);
    assert_eq!(board_print(&g), before);
    let lines = gene_lines(&ai);
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert!(lines[0].contains("stay stacked"), "{lines:#?}");
}

#[test]
fn a_centre_ringed_by_our_own_soldiers_keeps_its_stack_and_says_so() {
    let (mut g, home) = board();
    spawn(&mut g, "warrior", 0, home);
    spawn(&mut g, "warrior", 0, home);
    for pos in g.nbrs(home).into_iter().collect::<Vec<_>>() {
        spawn(&mut g, "warrior", 0, pos);
    }
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 0);
    assert_eq!(g.unit_ids_at(home).len(), 2);
    let lines = gene_lines(&ai);
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert!(
        lines[0].contains("0 tile(s) beside it are open"),
        "{lines:#?}"
    );
}

#[test]
fn three_on_one_tile_leave_one() {
    let (mut g, home) = board();
    let units = [
        spawn(&mut g, "warrior", 0, home),
        spawn(&mut g, "warrior", 0, home),
        spawn(&mut g, "spearman", 0, home),
    ];
    let mut ai = gene_on();
    assert_eq!(ai.step_stacked_soldiers_apart(&mut g, 0), 2);
    assert_eq!(g.units[&units[2]].pos, home, "the Spearman keeps it");
    let stands: Vec<Pos> = units[..2].iter().map(|uid| g.units[uid].pos).collect();
    assert_ne!(stands[0], stands[1], "the two step onto different tiles");
    assert!(stands.iter().all(|stand| g.wdist(*stand, home) == 1));
}
