use super::*;
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
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    uid
}

fn gene_on() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_centre_keeps_its_garrison();
    ai
}

/// Our city at (8,8), theirs at (28,8), at war.
fn front() -> (Game, Pos) {
    let city = at(8, 8);
    let mut g = flat_board(11, &[city, at(28, 8)]);
    war(&mut g, 0, 1);
    (g, city)
}

/// Every unit's (id, position, health) and every city's owner: what a step
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
fn off_reads_nothing_and_leaves_the_board() {
    let (mut g, city) = front();
    let garrison = spawn(&mut g, "warrior", 0, city);
    spawn(&mut g, "warrior", 1, at(9, 8));
    let filler = spawn(&mut g, "archer", 0, at(10, 9));
    let mut ai = AdvancedAi::new();
    ai.disable_centre_keeps_its_garrison();
    let before = board_print(&g);
    assert!(!ai.keeps_the_centre(&g, 0, garrison));
    assert_eq!(ai.centre_to_fill(&g, 0, filler), None);
    assert_eq!(ai.centre_garrison_step(&mut g, 0, garrison), None);
    assert_eq!(ai.centre_garrison_step(&mut g, 0, filler), None);
    assert_eq!(board_print(&g), before, "the gene off touches nothing");
}

#[test]
fn a_threatened_centre_is_kept() {
    let (mut g, city) = front();
    let garrison = spawn(&mut g, "warrior", 0, city);
    spawn(&mut g, "swordsman", 1, at(10, 8));
    let ai = gene_on();
    assert!(ai.centre_threatened(&g, 0, city));
    assert!(ai.keeps_the_centre(&g, 0, garrison));
}

#[test]
fn no_threat_no_post() {
    let (mut g, city) = front();
    let garrison = spawn(&mut g, "warrior", 0, city);
    // Five tiles out: beyond the reach.
    spawn(&mut g, "swordsman", 1, at(13, 8));
    let ai = gene_on();
    assert!(!ai.keeps_the_centre(&g, 0, garrison));
}

#[test]
fn a_soldier_at_peace_is_no_threat() {
    let city = at(8, 8);
    let mut g = flat_board(11, &[city, at(28, 8)]);
    let garrison = spawn(&mut g, "warrior", 0, city);
    spawn(&mut g, "swordsman", 1, at(9, 8));
    let ai = gene_on();
    assert!(!ai.keeps_the_centre(&g, 0, garrison));
}

#[test]
fn a_scout_neither_keeps_nor_fills() {
    let (mut g, city) = front();
    let scout = spawn(&mut g, "scout", 0, city);
    spawn(&mut g, "swordsman", 1, at(10, 8));
    let ai = gene_on();
    assert!(!ai.keeps_the_centre(&g, 0, scout));
    let (mut g, city) = front();
    let _ = city;
    spawn(&mut g, "swordsman", 1, at(10, 8));
    let scout = spawn(&mut g, "scout", 0, at(7, 8));
    assert_eq!(ai.centre_to_fill(&g, 0, scout), None);
}

#[test]
fn the_shooter_keeper_fires_from_the_centre_and_stays() {
    let (mut g, city) = front();
    let archer = spawn(&mut g, "archer", 0, city);
    let enemy = spawn(&mut g, "warrior", 1, at(10, 8));
    let mut ai = gene_on();
    assert_eq!(ai.centre_keeper_shot(&g, 0, archer), Some(at(10, 8)));
    assert_eq!(ai.centre_garrison_step(&mut g, 0, archer), Some(true));
    assert_eq!(g.units[&archer].pos, city, "the archer keeps the centre");
    assert!(
        g.units.get(&enemy).is_none_or(|unit| unit.hp < 100),
        "it fired from the centre"
    );
}

/// Where the warrior garrison of a city stands after our whole turn, with a
/// wounded swordsman beside the walls: a blow that kills it would carry the
/// warrior out of the City Center.
fn melee_garrison_after_a_turn(on: bool, planner: bool) -> (Option<Pos>, Pos) {
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    let enemy = spawn(&mut g, "swordsman", 1, at(9, 8));
    g.units.get_mut(&enemy).unwrap().hp = 8;
    let mut ai = AdvancedAi::new();
    if planner {
        ai.enable_battle_planner_2();
    }
    if on {
        ai.enable_centre_keeps_its_garrison();
    }
    ai.take_turn(&mut g, 0);
    (g.units.get(&warrior).map(|unit| unit.pos), city)
}

#[test]
fn the_melee_keeper_holds_through_a_whole_turn() {
    // The kill prepass (no battle planner): off, the warrior takes the kill
    // and steps out; on, it holds.
    let (off, city) = melee_garrison_after_a_turn(false, false);
    assert_ne!(off, Some(city), "off, the kill carries the warrior out");
    let (on, city) = melee_garrison_after_a_turn(true, false);
    assert_eq!(
        on,
        Some(city),
        "on, the warrior ends its turn in the City Center"
    );
    // The battle planner offers it no melee blow either.
    let (on, city) = melee_garrison_after_a_turn(true, true);
    assert_eq!(on, Some(city), "on, under the battle planner too");
}

#[test]
fn an_empty_threatened_centre_takes_a_soldier_in_reach() {
    let (mut g, city) = front();
    spawn(&mut g, "swordsman", 1, at(10, 8));
    let archer = spawn(&mut g, "archer", 0, at(7, 9));
    let mut ai = gene_on();
    let cid = g.city_at(city).unwrap();
    assert_eq!(ai.centre_to_fill(&g, 0, archer), Some((cid, city)));
    assert_eq!(ai.centre_garrison_step(&mut g, 0, archer), Some(true));
    assert_eq!(g.units[&archer].pos, city);
}

#[test]
fn a_centre_out_of_reach_or_already_held_takes_nobody() {
    let (mut g, city) = front();
    spawn(&mut g, "swordsman", 1, at(10, 8));
    let far = spawn(&mut g, "archer", 0, at(3, 12));
    let ai = gene_on();
    assert_eq!(ai.centre_to_fill(&g, 0, far), None, "five tiles out");
    let (mut g, city2) = front();
    let _ = city;
    spawn(&mut g, "swordsman", 1, at(10, 8));
    spawn(&mut g, "spearman", 0, city2);
    let near = spawn(&mut g, "archer", 0, at(7, 9));
    assert_eq!(ai.centre_to_fill(&g, 0, near), None, "the centre is held");
}

// ----------------------------------------------------------------------
// Paired posed defences: the gene on against off on the same boards.
// ----------------------------------------------------------------------

/// The defence genes the live Gran Colombia seat armed on 2026-11-12
/// (`~/.civvis-live-force-on`), on top of the deployment genome. A tag this
/// build does not carry is skipped.
const LIVE_DEFENCE: &[&str] = &[
    "anvil",
    "anvil-follows-the-defense",
    "anvil-garrisons-its-melee",
    "anvil-spares-the-one-blow",
    "battle-planner-2",
    "capital-defense-holds",
    "capture-holds-the-ring",
    "city-strike-takes-the-gun",
    "city-under-fire-ranks-first",
    "counter-out-of-reach-takes-the-weak",
    "defence-holds-under-fire",
    "defenders-hold-the-walls",
    "every-shooter-at-the-walls",
    "garrison-before-the-surprise",
    "garrison-under-fire",
    "guns-stay-out-of-reach",
    "retainers-garrison-the-short-cities",
    "ring-fires-on-the-city",
    "shared-danger",
    "siege-at-the-walls-dies-first",
    "siege-holds-a-breach",
    "threatened-city-buys-its-defender-2",
    "war-defense-spends-the-runway",
    "war-reinforcement",
    "wall-sortie-skips-the-encampment",
];

fn our_seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_engine_repairs();
    for tag in LIVE_DEFENCE {
        if let Some(gene) = crate::ai::gene(tag) {
            (gene.enable)(&mut ai);
        }
    }
    if on {
        ai.enable_centre_keeps_its_garrison();
    } else {
        ai.disable_centre_keeps_its_garrison();
    }
    ai
}

fn attacker_seat() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.apply_gene_ledger();
    ai.retarget_against(VictoryTarget::Domination, 0);
    ai
}

/// Whether `pos` lies east of `of` (a larger offset column).
fn east_of(pos: Pos, of: Pos) -> bool {
    crate::hex::axial_to_offset(pos.0, pos.1).0 > crate::hex::axial_to_offset(of.0, of.1).0
}

/// One posed defence: two cities of ours, a garrison in each, a few soldiers
/// around the front city, and an attacking force two to three tiles out.
fn posed(seed: u64) -> (Game, String) {
    let mut rng = crate::rng::Rng::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5eed);
    let capital = at(8, 6);
    let mut g = flat_board(seed, &[capital, at(29, 8)]);
    let second = at(8 + rng.randint(-1, 1), 13 + rng.randint(0, 2));
    g.found_city_for(0, second, None);
    war(&mut g, 0, 1);
    let ours = [capital, second];
    let front = rng.below(2);
    const GARRISON: &[&str] = &["warrior", "slinger", "archer", "spearman", "warrior"];
    const OURS: &[&str] = &["warrior", "archer", "spearman", "horseman", "slinger"];
    const THEIRS: &[&str] = &[
        "swordsman",
        "horseman",
        "archer",
        "spearman",
        "catapult",
        "warrior",
        "swordsman",
    ];
    let mut label = String::new();
    for city in ours {
        let kind = GARRISON[rng.below(GARRISON.len())];
        spawn(&mut g, kind, 0, city);
        label.push_str(&kind[..2]);
        label.push('/');
    }
    let free = |g: &Game, pos: Pos| {
        g.map.get(pos).is_some() && g.city_at(pos).is_none() && g.unit_ids_at(pos).is_empty()
    };
    let ring = |g: &Game, centre: Pos, lo: i32, hi: i32| -> Vec<Pos> {
        g.wdisk(centre, hi)
            .into_iter()
            .filter(|pos| g.wdist(*pos, centre) >= lo && free(g, *pos))
            .collect()
    };
    let extra = rng.randint(1, 3);
    for _ in 0..extra {
        let tiles = ring(&g, ours[front], 1, 3);
        if tiles.is_empty() {
            break;
        }
        let pos = tiles[rng.below(tiles.len())];
        spawn(&mut g, OURS[rng.below(OURS.len())], 0, pos);
    }
    // The attackers come from the east side of the front city.
    let attackers = rng.randint(3, 6);
    for _ in 0..attackers {
        let tiles: Vec<Pos> = ring(&g, ours[front], 2, 3)
            .into_iter()
            .filter(|pos| east_of(*pos, ours[front]))
            .collect();
        if tiles.is_empty() {
            break;
        }
        let pos = tiles[rng.below(tiles.len())];
        spawn(&mut g, THEIRS[rng.below(THEIRS.len())], 1, pos);
    }
    let second_front = rng.chance(0.4);
    if second_front {
        for _ in 0..rng.randint(2, 3) {
            let tiles: Vec<Pos> = ring(&g, ours[1 - front], 2, 3)
                .into_iter()
                .filter(|pos| east_of(*pos, ours[1 - front]))
                .collect();
            if tiles.is_empty() {
                break;
            }
            let pos = tiles[rng.below(tiles.len())];
            spawn(&mut g, THEIRS[rng.below(THEIRS.len())], 1, pos);
        }
    }
    label.push_str(&format!(
        "+{extra} vs {attackers}{}",
        if second_front { "+2nd" } else { "" }
    ));
    (g, label)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    cities_lost: usize,
    ours_lost: usize,
    theirs_lost: usize,
    /// Our turns that ended with a threatened City Center of ours empty.
    empty_centre_turns: usize,
}

const POSED_TURNS: u32 = 10;

fn play(seed: u64, on: bool) -> Held {
    let (mut g, _) = posed(seed);
    let start = g.turn;
    let cities: Vec<Pos> = g
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.pos)
        .collect();
    let military = |g: &Game, pid: usize| -> BTreeSet<u32> {
        g.units
            .values()
            .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
            .map(|unit| unit.id)
            .collect()
    };
    let ours_start = military(&g, 0);
    let theirs_start = military(&g, 1);
    let probe = gene_on();
    let mut ais: Vec<Box<dyn Ai>> = vec![Box::new(our_seat(on)), Box::new(attacker_seat())];
    let mut held = Held::default();
    while g.winner.is_none() && g.turn < start + POSED_TURNS && g.players[0].alive {
        let pid = g.current;
        // A revolt wakes the Free Cities seat, which has no agent here: its
        // turn only ends.
        if let Some(ai) = ais.get_mut(pid) {
            ai.take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if pid == 0 {
            held.empty_centre_turns += g
                .cities
                .values()
                .filter(|city| city.owner == 0)
                .filter(|city| probe.centre_threatened(&g, 0, city.pos))
                .filter(|city| {
                    !g.unit_ids_at(city.pos).iter().any(|id| {
                        let unit = &g.units[id];
                        unit.owner == 0 && g.rules.units[unit.kind].class == "military"
                    })
                })
                .count();
        }
        war(&mut g, 0, 1);
        g.peace_treaties.remove(&(0, 1));
    }
    held.cities_lost = cities
        .iter()
        .filter(|pos| g.city_at(**pos).is_none_or(|cid| g.cities[&cid].owner != 0))
        .count();
    let ours_end = military(&g, 0);
    let theirs_end = military(&g, 1);
    held.ours_lost = ours_start.difference(&ours_end).count();
    held.theirs_lost = theirs_start.difference(&theirs_end).count();
    held
}

/// Gene off against on over the same 96 posed boards, ten turns each: cities
/// lost, our starting soldiers lost, theirs lost, and our turns that ended
/// with a threatened City Center empty. Prints the paired table (run with
/// `--nocapture` to read it on a pass); asserts only that the gene leaves
/// threatened centres empty less often.
#[test]
fn paired_posed_defences() {
    const SEEDS: u64 = 96;
    let seeds: Vec<u64> = (1..=SEEDS).collect();
    let workers = 4;
    let rows: Vec<(u64, String, Held, Held)> = std::thread::scope(|scope| {
        let chunks: Vec<Vec<u64>> = (0..workers)
            .map(|w| seeds.iter().copied().skip(w).step_by(workers).collect())
            .collect();
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|seed| (seed, posed(seed).1, play(seed, false), play(seed, true)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut rows: Vec<_> = handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a posed defence panicked"))
            .collect();
        rows.sort_by_key(|row| row.0);
        rows
    });
    let mut report = String::from(
        "centre-keeps-its-garrison paired posed defences (10 turns; off -> on)\n\
         seed  board                     cities lost  ours lost  theirs lost  empty-centre turns\n",
    );
    let (mut off, mut on) = (Held::default(), Held::default());
    let (mut better, mut worse) = (0, 0);
    for (seed, label, a, b) in &rows {
        report.push_str(&format!(
            "{seed:>4}  {label:<24}  {} -> {}         {:>2} -> {:<2}    {:>2} -> {:<2}      {:>2} -> {:<2}\n",
            a.cities_lost, b.cities_lost, a.ours_lost, b.ours_lost, a.theirs_lost, b.theirs_lost,
            a.empty_centre_turns, b.empty_centre_turns
        ));
        off.cities_lost += a.cities_lost;
        on.cities_lost += b.cities_lost;
        off.ours_lost += a.ours_lost;
        on.ours_lost += b.ours_lost;
        off.theirs_lost += a.theirs_lost;
        on.theirs_lost += b.theirs_lost;
        off.empty_centre_turns += a.empty_centre_turns;
        on.empty_centre_turns += b.empty_centre_turns;
        match b.cities_lost.cmp(&a.cities_lost) {
            std::cmp::Ordering::Less => better += 1,
            std::cmp::Ordering::Greater => worse += 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    report.push_str(&format!(
        "total {} boards: cities lost {} -> {} (on better {better}, worse {worse}); \
         ours lost {} -> {}; theirs lost {} -> {}; empty-centre turns {} -> {}\n",
        rows.len(),
        off.cities_lost,
        on.cities_lost,
        off.ours_lost,
        on.ours_lost,
        off.theirs_lost,
        on.theirs_lost,
        off.empty_centre_turns,
        on.empty_centre_turns
    ));
    eprintln!("{report}");
    // The mechanism, not the outcome: threatened City Centers stand empty on
    // fewer of our turn ends. (On the tree this was written on: 257 -> 169
    // over the 96 boards; cities lost 80 -> 75, ours lost 166 -> 161, theirs
    // lost 65 -> 50.)
    assert!(
        on.empty_centre_turns < off.empty_centre_turns,
        "the gene left threatened centres empty no less often\n{report}"
    );
}
