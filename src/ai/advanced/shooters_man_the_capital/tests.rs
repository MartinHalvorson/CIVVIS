use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::Ai;
use crate::game::Game;
use crate::name;

/// A flat grassland board of `capitals.len()` empires, every starting unit
/// cleared, the map explored and everyone met, nobody at war, turn 60. (The
/// objective board's test board.)
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

fn spawn(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, pid, pos);
    refresh(g, uid);
    uid
}

/// A new turn's movement and attack for `uid`.
fn refresh(g: &mut Game, uid: u32) {
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
}

fn gene_on() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_shooters_man_the_capital();
    ai
}

/// Our original capital at (10,10), a rival's city at (28,10) and a third
/// major's at (20,18), everyone at peace, every tile within fourteen of our
/// capital in sight.
fn watch_board() -> (Game, Pos) {
    let home = at(10, 10);
    let mut g = flat_board(11, &[home, at(28, 10), at(20, 18)]);
    let cid = g.city_at(home).expect("our capital");
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.is_capital = true;
        city.original_owner = 0;
    }
    g.host_observed = Arc::new(g.wdisk(home, 14).into_iter().collect());
    assert_eq!(AdvancedAi::capital_watch_city(&g, 0), Some(cid));
    (g, home)
}

/// A free tile exactly `distance` from `home`, the lowest first, that is
/// also `from_other` or more from `other`.
fn tile_at(g: &Game, home: Pos, distance: i32, other: Pos, from_other: i32) -> Pos {
    let mut ring: Vec<Pos> = g
        .wdisk(home, distance)
        .into_iter()
        .filter(|pos| {
            g.wdist(*pos, home) == distance
                && g.wdist(*pos, other) >= from_other
                && g.unit_ids_at(*pos).is_empty()
                && g.city_at(*pos).is_none()
                && g.map.get(*pos).is_some()
        })
        .collect();
    ring.sort();
    *ring.first().expect("a free tile on the ring")
}

/// The tile `steps` east of `home` along the row.
fn east(home: Pos, g: &Game, steps: i32) -> Pos {
    let mut row: Vec<Pos> = g
        .wdisk(home, steps)
        .into_iter()
        .filter(|pos| g.wdist(*pos, home) == steps && pos.1 == home.1 && pos.0 > home.0)
        .collect();
    row.sort();
    row[0]
}

fn west(home: Pos, g: &Game, steps: i32) -> Pos {
    let mut row: Vec<Pos> = g
        .wdisk(home, steps)
        .into_iter()
        .filter(|pos| g.wdist(*pos, home) == steps && pos.1 == home.1 && pos.0 < home.0)
        .collect();
    row.sort();
    row[0]
}

fn spawn_east(g: &mut Game, kind: &str, pid: usize, home: Pos, steps: i32) -> u32 {
    let pos = east(home, g, steps);
    spawn(g, kind, pid, pos)
}

fn spawn_west(g: &mut Game, kind: &str, pid: usize, home: Pos, steps: i32) -> u32 {
    let pos = west(home, g, steps);
    spawn(g, kind, pid, pos)
}

/// A unit on the lowest free tile `distance` from `home` and `from_other` or
/// more from `other`.
fn spawn_ring(
    g: &mut Game,
    kind: &str,
    pid: usize,
    home: Pos,
    distance: i32,
    other: Pos,
    from_other: i32,
) -> u32 {
    let pos = tile_at(g, home, distance, other, from_other);
    spawn(g, kind, pid, pos)
}

/// Every unit's (id, position, health, owner) and every city's: what a pass
/// that did nothing leaves unchanged.
fn board_print(g: &Game) -> Vec<(u32, Pos, i32, usize)> {
    let mut out: Vec<(u32, Pos, i32, usize)> = g
        .units
        .values()
        .map(|unit| (unit.id, unit.pos, unit.hp, unit.owner))
        .collect();
    out.extend(
        g.cities
            .values()
            .map(|city| (city.id, city.pos, city.hp, city.owner)),
    );
    out.sort();
    out
}

#[test]
fn shooters_man_the_capital_ships_off_and_is_registered() {
    opt_in_off_in_both_controllers("shooters-man-the-capital", |ai| ai.shooters_man_the_capital);
}

#[test]
fn off_reads_nothing_and_leaves_the_board() {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    spawn_east(&mut g, "catapult", 1, home, 2);
    spawn_east(&mut g, "swordsman", 1, home, 3);
    spawn_west(&mut g, "archer", 0, home, 4);
    let mut ai = AdvancedAi::new();
    let before = board_print(&g);
    assert_eq!(ai.shooters_man_reading(&g, 0), None);
    assert!(ai.plan_shooters_man_the_capital(&mut g, 0).is_empty());
    assert_eq!(board_print(&g), before, "the gene off touches nothing");
}

#[test]
fn a_party_at_peace_is_watched_from_two_soldiers() {
    let (mut g, home) = watch_board();
    let ai = gene_on();
    assert_eq!(ai.shooters_man_reading(&g, 0), None, "an empty board");
    spawn_east(&mut g, "swordsman", 1, home, 5);
    assert_eq!(
        ai.shooters_man_reading(&g, 0),
        None,
        "one soldier at peace is a passer-by"
    );
    // A second soldier of ANOTHER major does not make a party.
    let other = spawn_west(&mut g, "swordsman", 2, home, 5);
    assert_eq!(ai.shooters_man_reading(&g, 0), None);
    g.remove_unit(other);
    // A Scout, and a soldier beyond six tiles, are not counted.
    spawn_east(&mut g, "scout", 1, home, 4);
    spawn_east(&mut g, "swordsman", 1, home, 7);
    assert_eq!(ai.shooters_man_reading(&g, 0), None);
    spawn_east(&mut g, "swordsman", 1, home, 6);
    let reading = ai.shooters_man_reading(&g, 0).expect("a party of two");
    assert_eq!(reading.home, home);
    assert!(!reading.war);
    assert_eq!(
        (reading.engines, reading.soldiers, reading.owner),
        (0, 2, 1)
    );
    assert_eq!(reading.wanted, 3, "the party and one more");
    assert!(reading.guns.is_empty(), "nothing to shoot at peace");
    // A declared friend cannot open a war on us.
    let until = g.turn + 30;
    g.players[0].friends_until.insert(1, until);
    g.players[1].friends_until.insert(0, until);
    assert_eq!(ai.shooters_man_reading(&g, 0), None, "a friend's party");
}

#[test]
fn an_engine_at_peace_calls_every_shooter() {
    for engine in ["catapult", "battering_ram", "siege_tower"] {
        let (mut g, home) = watch_board();
        let ai = gene_on();
        let far = spawn_east(&mut g, engine, 1, home, 9);
        assert_eq!(
            ai.shooters_man_reading(&g, 0),
            None,
            "{engine} nine tiles out"
        );
        g.remove_unit(far);
        spawn_east(&mut g, engine, 1, home, 8);
        let reading = ai
            .shooters_man_reading(&g, 0)
            .unwrap_or_else(|| panic!("{engine} eight tiles out is watched"));
        assert!(!reading.war);
        assert_eq!((reading.engines, reading.soldiers), (1, 0));
        assert_eq!(reading.wanted, usize::MAX, "an engine calls everyone");
        assert!(reading.guns.is_empty(), "no target at peace");
    }
}

#[test]
fn the_capital_alone_is_watched_and_only_what_we_see() {
    let (mut g, home) = watch_board();
    let ai = gene_on();
    spawn_east(&mut g, "catapult", 1, home, 3);
    assert!(ai.shooters_man_reading(&g, 0).is_some());
    // Out of sight, nothing is read.
    let mut dark = g.clone();
    dark.host_observed = Arc::new(dark.wdisk(home, 1).into_iter().collect());
    assert_eq!(
        ai.shooters_man_reading(&dark, 0),
        None,
        "an engine we do not see"
    );
    // A capital we took is not the original capital.
    let cid = g.city_at(home).unwrap();
    g.cities.get_mut(&cid).unwrap().original_owner = 1;
    assert_eq!(ai.shooters_man_reading(&g, 0), None);
}

#[test]
fn war_is_watched_from_the_first_soldier_and_a_gun_is_the_target() {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    let ai = gene_on();
    spawn_east(&mut g, "warrior", 1, home, 6);
    let reading = ai.shooters_man_reading(&g, 0).expect("a soldier at war");
    assert!(reading.war);
    assert_eq!((reading.engines, reading.soldiers), (0, 1));
    assert_eq!(reading.wanted, MAN_GUARD_MIN);
    let gun = spawn_east(&mut g, "catapult", 1, home, 3);
    let reading = ai.shooters_man_reading(&g, 0).expect("a gun at war");
    assert_eq!(reading.guns, vec![gun]);
    assert_eq!(reading.wanted, usize::MAX);
    // A barbarian is no party.
    let (mut g, home) = watch_board();
    if let Some(barb) = g.barb_pid {
        spawn_east(&mut g, "warrior", barb, home, 3);
        spawn_east(&mut g, "warrior", barb, home, 4);
        assert_eq!(ai.shooters_man_reading(&g, 0), None);
    }
}

#[test]
fn the_nearest_free_shooters_are_called() {
    let (mut g, home) = watch_board();
    let ai = gene_on();
    // A party of two: three shooters wanted.
    spawn_east(&mut g, "swordsman", 1, home, 5);
    spawn_east(&mut g, "swordsman", 1, home, 6);
    let far_side = east(home, &g, 6);
    let near = spawn_ring(&mut g, "archer", 0, home, 1, far_side, 6);
    let mid = spawn_ring(&mut g, "crossbowman", 0, home, 3, far_side, 7);
    let out = spawn_ring(&mut g, "archer", 0, home, 5, far_side, 8);
    let farther = spawn_ring(&mut g, "archer", 0, home, 7, far_side, 9);
    // Not shooters of the capital: a Slinger (reach one), a Warrior, a
    // Catapult of ours, and an Archer holding another city of ours.
    spawn_ring(&mut g, "slinger", 0, home, 2, far_side, 6);
    spawn_ring(&mut g, "warrior", 0, home, 2, far_side, 6);
    spawn_ring(&mut g, "catapult", 0, home, 2, far_side, 6);
    let second = tile_at(&g, home, 4, far_side, 8);
    g.found_city_for(0, second, None);
    let garrison = spawn(&mut g, "archer", 0, second);
    let reading = ai.shooters_man_reading(&g, 0).expect("a party");
    assert_eq!(reading.wanted, 3);
    assert_eq!(
        ai.shooters_man_called(&g, 0, &reading),
        vec![near, mid, out],
        "the three nearest shooters"
    );
    // An engine calls every free shooter within twelve tiles, and still
    // not the other city's garrison.
    spawn_east(&mut g, "catapult", 1, home, 8);
    let beyond = spawn_east(&mut g, "archer", 0, home, 13);
    let reading = ai.shooters_man_reading(&g, 0).expect("an engine");
    let called = ai.shooters_man_called(&g, 0, &reading);
    assert_eq!(called, vec![near, mid, out, farther]);
    assert!(!called.contains(&garrison) && !called.contains(&beyond));
}

#[test]
fn at_peace_the_called_shooters_walk_home_and_hold() {
    let (mut g, home) = watch_board();
    let mut ai = gene_on();
    let party = [
        spawn_east(&mut g, "swordsman", 1, home, 5),
        spawn_east(&mut g, "swordsman", 1, home, 6),
    ];
    let far_side = east(home, &g, 6);
    let archers = [
        spawn_ring(&mut g, "archer", 0, home, 4, far_side, 8),
        spawn_ring(&mut g, "archer", 0, home, 5, far_side, 9),
        spawn_ring(&mut g, "archer", 0, home, 6, far_side, 10),
    ];
    let mut distances: Vec<i32> = Vec::new();
    for _ in 0..4 {
        for uid in archers {
            refresh(&mut g, uid);
        }
        let held = ai.plan_shooters_man_the_capital(&mut g, 0);
        assert_eq!(held, archers.into_iter().collect::<BTreeSet<u32>>());
        distances = archers
            .iter()
            .map(|uid| g.wdist(g.units[uid].pos, home))
            .collect();
    }
    assert!(
        distances.iter().all(|distance| *distance <= 1),
        "all three in or beside the City Center after four turns: {distances:?}"
    );
    assert!(
        distances.contains(&0),
        "the empty City Center takes a shooter: {distances:?}"
    );
    for uid in party {
        assert_eq!(g.units[&uid].hp, 100, "nobody is shot at peace");
    }
    // In place, the pass moves nobody.
    for uid in archers {
        refresh(&mut g, uid);
    }
    let before = board_print(&g);
    let held = ai.plan_shooters_man_the_capital(&mut g, 0);
    assert_eq!(held.len(), 3, "still held while the watch reads");
    assert_eq!(board_print(&g), before);
}

#[test]
fn a_gun_in_reach_is_shot_before_a_soldier() {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    let mut ai = gene_on();
    // Our Archer holds the City Center. A Catapult stands two tiles east, and
    // a Warrior at 10 health (a certain kill) stands beside the centre.
    let archer = spawn(&mut g, "archer", 0, home);
    let gun = spawn_east(&mut g, "catapult", 1, home, 2);
    let warrior = spawn_west(&mut g, "warrior", 1, home, 1);
    g.units.get_mut(&warrior).unwrap().hp = 10;
    let held = ai.plan_shooters_man_the_capital(&mut g, 0);
    assert!(held.contains(&archer));
    assert!(
        g.units[&gun].hp < 100,
        "the Catapult takes the shot: {}",
        g.units[&gun].hp
    );
    assert_eq!(
        g.units.get(&warrior).map(|unit| unit.hp),
        Some(10),
        "the kill on the Warrior is passed over"
    );
    assert_eq!(g.units[&archer].pos, home, "it fired from the centre");
}

#[test]
fn without_a_gun_in_reach_it_takes_its_best_shot() {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    let mut ai = gene_on();
    let archer = spawn(&mut g, "archer", 0, home);
    let warrior = spawn_east(&mut g, "warrior", 1, home, 2);
    let held = ai.plan_shooters_man_the_capital(&mut g, 0);
    assert!(held.contains(&archer));
    assert!(
        g.units.get(&warrior).is_none_or(|unit| unit.hp < 100),
        "the Warrior is shot from the centre"
    );
    assert_eq!(g.units[&archer].pos, home);
}

#[test]
fn a_shooter_takes_a_stand_in_reach_of_the_gun_and_fires() {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    let mut ai = gene_on();
    // A melee soldier of ours holds the centre, so the stands are the tiles
    // beside it. The Catapult stands two tiles east of the centre.
    spawn(&mut g, "warrior", 0, home);
    let gun_at = east(home, &g, 2);
    let gun = spawn(&mut g, "catapult", 1, gun_at);
    // A stand beside the centre two tiles from the gun, and a start one step
    // from that stand, two from the centre and out of the Archer's reach.
    let stand = g
        .nbrs(home)
        .into_iter()
        .filter(|pos| g.wdist(*pos, gun_at) == 2)
        .min()
        .expect("a stand beside the centre");
    let start = g
        .nbrs(stand)
        .into_iter()
        .filter(|pos| {
            g.wdist(*pos, home) == 2 && g.wdist(*pos, gun_at) == 3 && g.city_at(*pos).is_none()
        })
        .min()
        .expect("a start one step out");
    let archer = spawn(&mut g, "archer", 0, start);
    let held = ai.plan_shooters_man_the_capital(&mut g, 0);
    assert!(held.contains(&archer));
    let pos = g.units[&archer].pos;
    assert_eq!(g.wdist(pos, home), 1, "it stands beside the City Center");
    assert!(g.wdist(pos, gun_at) <= 2, "with the gun in reach");
    assert!(
        g.units[&gun].hp < 100,
        "and fires the turn it arrives: {}",
        g.units[&gun].hp
    );
}

#[test]
fn a_shooter_far_out_walks_toward_the_capital_at_war() {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    let mut ai = gene_on();
    spawn_east(&mut g, "catapult", 1, home, 3);
    let start = west(home, &g, 6);
    let archer = spawn(&mut g, "archer", 0, start);
    let held = ai.plan_shooters_man_the_capital(&mut g, 0);
    assert!(held.contains(&archer));
    let distance = g.wdist(g.units[&archer].pos, home);
    assert!(distance < 6, "it walks toward the capital: {distance}");
}

/// A whole turn of the seat with the battle planner on: where the three
/// Archers end it (tiles from the City Center), the Catapult's health, and
/// the gene's journal lines.
fn a_turn(on: bool) -> (Vec<i32>, i32, Vec<String>) {
    let (mut g, home) = watch_board();
    war(&mut g, 0, 1);
    let gun = spawn_east(&mut g, "catapult", 1, home, 2);
    spawn_east(&mut g, "swordsman", 1, home, 3);
    let far_side = east(home, &g, 3);
    let archers = [
        spawn_ring(&mut g, "archer", 0, home, 2, far_side, 4),
        spawn_ring(&mut g, "archer", 0, home, 3, far_side, 5),
        spawn_ring(&mut g, "archer", 0, home, 3, far_side, 5),
    ];
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    if on {
        ai.enable_shooters_man_the_capital();
    }
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.take_turn(&mut g, 0);
    let mut notes: Vec<String> = ai
        .base
        .journal
        .since(u64::MAX)
        .thoughts
        .into_iter()
        .filter(|thought| thought.detail.contains("shooters-man-the-capital"))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect();
    // Who stands where at the end of the turn, for a failure's message.
    let mut board: Vec<String> = g
        .units
        .values()
        .map(|unit| {
            format!(
                "board: {} of {} at {:?}, {} from the centre, {} moves left",
                unit.kind.as_str(),
                unit.owner,
                unit.pos,
                g.wdist(unit.pos, home),
                unit.moves_left
            )
        })
        .collect();
    board.sort();
    notes.extend(board);
    (
        archers
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .map(|unit| g.wdist(unit.pos, home))
            .collect(),
        g.units.get(&gun).map_or(0, |unit| unit.hp),
        notes,
    )
}

#[test]
fn a_whole_turn_leaves_the_called_shooters_at_the_capital() {
    let (on, _, notes) = a_turn(true);
    assert_eq!(on.len(), 3, "nobody is lost on our own turn");
    assert!(
        notes.iter().any(|note| note.contains("3 called")),
        "the watch read and called all three: {notes:#?}"
    );
    assert!(
        on.iter().all(|distance| *distance <= 1),
        "on, every called shooter ends the turn in or beside the centre: {on:?} {notes:#?}"
    );
    let (_, _, quiet) = a_turn(false);
    assert!(
        !quiet.iter().any(|note| note.contains("shooters-man")),
        "off, the gene journals nothing"
    );
    // The same turn twice is the same turn.
    assert_eq!(a_turn(true), a_turn(true));
    assert_eq!(a_turn(false), a_turn(false));
}
