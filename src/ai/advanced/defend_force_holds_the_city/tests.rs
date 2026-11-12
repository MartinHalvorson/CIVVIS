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
    ai.enable_defend_force_holds_the_city();
    ai
}

/// Our city at (8,8), theirs at (28,8), at war.
fn front() -> (Game, Pos) {
    let city = at(8, 8);
    let mut g = flat_board(11, &[city, at(28, 8)]);
    war(&mut g, 0, 1);
    (g, city)
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

/// A free ring tile of `city` on the side of `toward`.
fn ring_tile_toward(g: &Game, city: Pos, toward: Pos) -> Pos {
    g.nbrs(city)
        .into_iter()
        .min_by_key(|tile| (g.wdist(*tile, toward), *tile))
        .unwrap()
}

#[test]
fn off_draws_nothing_and_leaves_the_board() {
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    spawn(&mut g, "archer", 0, at(9, 9));
    spawn(&mut g, "spearman", 0, at(7, 9));
    spawn(&mut g, "swordsman", 1, at(11, 8));
    let mut ai = AdvancedAi::new();
    ai.disable_defend_force_holds_the_city();
    let before = board_print(&g);
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_eq!(ai.defend_hold_plan, HoldPlan::default());
    assert_eq!(board_print(&g), before, "the gene off touches nothing");
    assert_eq!(ai.defend_hold_step(&mut g, 0, warrior), None);
    let cid = g.city_at(city).unwrap();
    assert!(!ai.defend_hold_holds(cid));
}

#[test]
fn a_shooter_swaps_into_the_centre() {
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    let beside = ring_tile_toward(&g, city, at(12, 8));
    let archer = spawn(&mut g, "archer", 0, beside);
    spawn(&mut g, "swordsman", 1, at(11, 8));
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_eq!(g.units[&archer].pos, city, "the archer took the centre");
    assert_eq!(g.units[&warrior].pos, beside, "the warrior took its tile");
    let posts = &ai.defend_hold_plan.posts;
    assert_eq!(posts[&archer].role, HoldRole::Centre);
    assert_eq!(posts[&warrior].role, HoldRole::Ring);
    assert!(ai.defend_hold_holds(g.city_at(city).unwrap()));
}

#[test]
fn the_ring_tiles_the_enemy_melee_reaches_are_blocked() {
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let west: Vec<u32> = [at(7, 7), at(7, 9)]
        .into_iter()
        .map(|pos| spawn(&mut g, "spearman", 0, pos))
        .collect();
    spawn(&mut g, "swordsman", 1, at(11, 8));
    spawn(&mut g, "swordsman", 1, at(11, 9));
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    let enemies: Vec<Pos> = g
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.pos)
        .collect();
    for uid in &west {
        let post = &ai.defend_hold_plan.posts[uid];
        assert_eq!(post.role, HoldRole::Ring, "a spearman blocks the ring");
        assert_eq!(g.units[uid].pos, post.at);
        assert_eq!(g.wdist(post.at, city), 1);
        // Nearest threat first: each block faces the attackers.
        let nearest = enemies.iter().map(|e| g.wdist(*e, post.at)).min().unwrap();
        assert!(nearest <= 3, "the block stands on the threatened side");
    }
}

#[test]
fn a_post_its_expected_cost_would_kill_is_left_open() {
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let wounded = spawn(&mut g, "warrior", 0, at(6, 8));
    g.units.get_mut(&wounded).unwrap().hp = 5;
    for pos in [at(10, 7), at(10, 8), at(10, 9), at(11, 8)] {
        spawn(&mut g, "swordsman", 1, pos);
    }
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_ne!(
        ai.defend_hold_plan
            .posts
            .get(&wounded)
            .map(|post| post.role),
        Some(HoldRole::Ring),
        "a 5-hp warrior is not posted on the ring"
    );
}

#[test]
fn the_centre_melee_strikes_only_without_leaving() {
    // A full-health swordsman beside the walls: no blow of the warrior can
    // kill it, so the warrior strikes and stays.
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    let tile = ring_tile_toward(&g, city, at(12, 8));
    let enemy = spawn(&mut g, "warrior", 1, tile);
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_eq!(ai.defend_hold_plan.posts[&warrior].role, HoldRole::Centre);
    assert_eq!(ai.defend_hold_step(&mut g, 0, warrior), Some(true));
    assert_eq!(g.units[&warrior].pos, city, "the blow did not carry it out");
    assert!(g.units[&enemy].hp < 100);
    // A wounded one would die and draw the warrior out: with nobody to
    // refill the centre, no blow.
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    let tile = ring_tile_toward(&g, city, at(12, 8));
    let enemy = spawn(&mut g, "warrior", 1, tile);
    g.units.get_mut(&enemy).unwrap().hp = 5;
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    let _ = ai.defend_hold_step(&mut g, 0, warrior);
    assert_eq!(g.units[&warrior].pos, city, "no refill, so no killing blow");
    assert_eq!(g.units[&enemy].hp, 5);
}

#[test]
fn every_unit_holds_one_post_and_no_tile_two() {
    let (mut g, city) = front();
    let second = at(8, 14);
    g.found_city_for(0, second, None);
    for pos in [
        city,
        at(7, 9),
        at(9, 9),
        at(6, 8),
        second,
        at(7, 13),
        at(9, 13),
    ] {
        spawn(&mut g, "spearman", 0, pos);
    }
    spawn(&mut g, "archer", 0, at(8, 11));
    for pos in [at(11, 8), at(11, 9), at(11, 14), at(11, 13)] {
        spawn(&mut g, "swordsman", 1, pos);
    }
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_eq!(ai.defend_hold_plan.cities.len(), 2, "both cities are held");
    let mut tiles = BTreeSet::new();
    for (uid, post) in &ai.defend_hold_plan.posts {
        if post.role != HoldRole::March {
            assert!(tiles.insert(post.at), "no two posts share {:?}", post.at);
        }
        assert_eq!(g.units[uid].pos, post.at, "each unit stands on its post");
    }
}

// ----------------------------------------------------------------------
// Paired posed defences: off, `centre-keeps-its-garrison`,
// `defend-force-holds-the-city`, and both, on the same boards.
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct Arm {
    keep: bool,
    hold: bool,
}

const ARMS: [(&str, Arm); 4] = [
    (
        "off",
        Arm {
            keep: false,
            hold: false,
        },
    ),
    (
        "keep",
        Arm {
            keep: true,
            hold: false,
        },
    ),
    (
        "hold",
        Arm {
            keep: false,
            hold: true,
        },
    ),
    (
        "both",
        Arm {
            keep: true,
            hold: true,
        },
    ),
];

fn our_seat(arm: Arm) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_engine_repairs();
    for tag in LIVE_DEFENCE {
        if let Some(gene) = crate::ai::gene(tag) {
            (gene.enable)(&mut ai);
        }
    }
    if arm.keep {
        ai.enable_centre_keeps_its_garrison();
    } else {
        ai.disable_centre_keeps_its_garrison();
    }
    if arm.hold {
        ai.enable_defend_force_holds_the_city();
    } else {
        ai.disable_defend_force_holds_the_city();
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

/// The two board sets.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Set {
    /// #5198's 96 boards: ancient garrisons, 1-3 soldiers beside the front
    /// city, 3-6 ancient attackers.
    Light,
    /// A real force: medieval garrisons, 4-8 mixed soldiers about the front
    /// city, 5-8 medieval and Renaissance attackers.
    Force,
}

/// One posed defence: two cities of ours, a garrison in each, soldiers
/// around the front city, and an attacking force east of it.
fn posed(seed: u64, set: Set) -> Game {
    let mut rng = crate::rng::Rng::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5eed);
    let capital = at(8, 6);
    let mut g = flat_board(seed, &[capital, at(29, 8)]);
    let second = at(8 + rng.randint(-1, 1), 13 + rng.randint(0, 2));
    g.found_city_for(0, second, None);
    war(&mut g, 0, 1);
    let ours = [capital, second];
    let front = rng.below(2);
    let (garrison, own, theirs, extra, attackers, near, far): (
        &[&str],
        &[&str],
        &[&str],
        (i32, i32),
        (i32, i32),
        i32,
        i32,
    ) = match set {
        Set::Light => (
            &["warrior", "slinger", "archer", "spearman", "warrior"],
            &["warrior", "archer", "spearman", "horseman", "slinger"],
            &[
                "swordsman",
                "horseman",
                "archer",
                "spearman",
                "catapult",
                "warrior",
                "swordsman",
            ],
            (1, 3),
            (3, 6),
            2,
            3,
        ),
        Set::Force => (
            &[
                "crossbowman",
                "pikeman",
                "man_at_arms",
                "archer",
                "spearman",
            ],
            &[
                "crossbowman",
                "pikeman",
                "man_at_arms",
                "knight",
                "swordsman",
                "archer",
                "spearman",
                "horseman",
            ],
            &[
                "knight",
                "crossbowman",
                "pikeman",
                "man_at_arms",
                "trebuchet",
                "musketman",
                "bombard",
                "cuirassier",
            ],
            (4, 8),
            (5, 8),
            2,
            4,
        ),
    };
    for city in ours {
        spawn(&mut g, garrison[rng.below(garrison.len())], 0, city);
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
    for _ in 0..rng.randint(extra.0, extra.1) {
        let tiles = ring(&g, ours[front], 1, 3);
        if tiles.is_empty() {
            break;
        }
        let pos = tiles[rng.below(tiles.len())];
        spawn(&mut g, own[rng.below(own.len())], 0, pos);
    }
    for _ in 0..rng.randint(attackers.0, attackers.1) {
        let tiles: Vec<Pos> = ring(&g, ours[front], near, far)
            .into_iter()
            .filter(|pos| east_of(*pos, ours[front]))
            .collect();
        if tiles.is_empty() {
            break;
        }
        let pos = tiles[rng.below(tiles.len())];
        spawn(&mut g, theirs[rng.below(theirs.len())], 1, pos);
    }
    if rng.chance(0.4) {
        for _ in 0..rng.randint(2, 3) {
            let tiles: Vec<Pos> = ring(&g, ours[1 - front], near, far)
                .into_iter()
                .filter(|pos| east_of(*pos, ours[1 - front]))
                .collect();
            if tiles.is_empty() {
                break;
            }
            let pos = tiles[rng.below(tiles.len())];
            spawn(&mut g, theirs[rng.below(theirs.len())], 1, pos);
        }
    }
    g
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    cities_lost: usize,
    ours_lost: usize,
    theirs_lost: usize,
    /// Our turns that ended with a threatened City Center of ours empty.
    empty_centre_turns: usize,
    /// Turns each starting city of ours stayed ours, summed.
    turns_held: usize,
    /// Posts drawn, and posts a unit ended our turn off (other than by its
    /// own blow, a refill or death): the mechanism checks.
    posts: usize,
    posts_broken: usize,
    shared_tiles: usize,
}

const POSED_TURNS: u32 = 10;

fn play(seed: u64, set: Set, arm: Arm) -> Held {
    let mut g = posed(seed, set);
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
    let mut us = our_seat(arm);
    let mut them = attacker_seat();
    let mut held = Held::default();
    let mut lost_at: BTreeMap<Pos, u32> = BTreeMap::new();
    while g.winner.is_none() && g.turn < start + POSED_TURNS && g.players[0].alive {
        let pid = g.current;
        if pid == 0 {
            us.take_turn(&mut g, pid);
            // The mechanism: one post a unit, one unit a tile, and every
            // posted unit on its post at the end of our turn unless its own
            // blow carried it forward, it refilled the centre, or it died.
            let plan = us.defend_hold_plan.clone();
            if arm.hold {
                assert!(plan.turn == g.turn || plan.posts.is_empty());
            } else {
                assert!(plan.posts.is_empty(), "the gene off draws no posts");
            }
            let mut tiles = BTreeSet::new();
            for (uid, post) in &plan.posts {
                held.posts += 1;
                if post.role != HoldRole::March && !tiles.insert(post.at) {
                    held.shared_tiles += 1;
                }
                let Some(unit) = g.units.get(uid) else {
                    continue;
                };
                let centre = g.cities.get(&post.city).map(|city| city.pos);
                let on_post = unit.pos == post.at
                    || Some(unit.pos) == centre
                    || (unit.attacks_left == 0 && g.wdist(unit.pos, post.at) == 1);
                if !on_post {
                    held.posts_broken += 1;
                }
            }
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
        } else {
            them.take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        war(&mut g, 0, 1);
        g.peace_treaties.remove(&(0, 1));
        for pos in &cities {
            let ours = g.city_at(*pos).is_some_and(|cid| g.cities[&cid].owner == 0);
            if !ours {
                lost_at.entry(*pos).or_insert(g.turn - start);
            }
        }
    }
    held.cities_lost = cities
        .iter()
        .filter(|pos| g.city_at(**pos).is_none_or(|cid| g.cities[&cid].owner != 0))
        .count();
    held.turns_held = cities
        .iter()
        .map(|pos| lost_at.get(pos).copied().unwrap_or(POSED_TURNS) as usize)
        .sum();
    let ours_end = military(&g, 0);
    let theirs_end = military(&g, 1);
    held.ours_lost = ours_start.difference(&ours_end).count();
    held.theirs_lost = theirs_start.difference(&theirs_end).count();
    held
}

fn add(total: &mut Held, one: &Held) {
    total.cities_lost += one.cities_lost;
    total.ours_lost += one.ours_lost;
    total.theirs_lost += one.theirs_lost;
    total.empty_centre_turns += one.empty_centre_turns;
    total.turns_held += one.turns_held;
    total.posts += one.posts;
    total.posts_broken += one.posts_broken;
    total.shared_tiles += one.shared_tiles;
}

/// Off, `centre-keeps-its-garrison`, `defend-force-holds-the-city` and both
/// over the same posed boards, ten turns each: #5198's 96 light boards and 64
/// boards where the defender has a real force against medieval and
/// Renaissance attackers. Prints the arms side by side (run with
/// `--nocapture` to read it on a pass); asserts only the mechanism: posts are
/// held, no tile is posted twice, and the gene off draws nothing.
#[test]
fn paired_posed_defences_four_arms() {
    let boards: Vec<(Set, u64)> = (1..=96)
        .map(|seed| (Set::Light, seed))
        .chain((1001..=1064).map(|seed| (Set::Force, seed)))
        .collect();
    let workers = 4;
    let rows: Vec<(Set, u64, [Held; 4])> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let chunk: Vec<(Set, u64)> =
                    boards.iter().copied().skip(w).step_by(workers).collect();
                scope.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|(set, seed)| {
                            let arms = ARMS.map(|(_, arm)| play(seed, set, arm));
                            (set, seed, arms)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut rows: Vec<_> = handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a posed defence panicked"))
            .collect();
        rows.sort_by_key(|row| (row.0 == Set::Force, row.1));
        rows
    });
    let mut report = String::from(
        "defend-force-holds-the-city paired posed defences (10 turns; arms off | keep | hold | both)\n",
    );
    let mut broken = 0;
    let mut shared = 0;
    let mut posts = 0;
    for set in [Set::Light, Set::Force] {
        let mut totals = [Held::default(); 4];
        let mut better = [0usize; 4];
        let mut worse = [0usize; 4];
        let mut n = 0;
        for (_, _, arms) in rows.iter().filter(|row| row.0 == set) {
            n += 1;
            for (index, held) in arms.iter().enumerate() {
                add(&mut totals[index], held);
                match held.cities_lost.cmp(&arms[0].cities_lost) {
                    std::cmp::Ordering::Less => better[index] += 1,
                    std::cmp::Ordering::Greater => worse[index] += 1,
                    std::cmp::Ordering::Equal => {}
                }
            }
        }
        let line = |name: &str, read: &dyn Fn(&Held) -> usize| -> String {
            format!(
                "  {name:<26} {:>5} | {:>5} | {:>5} | {:>5}\n",
                read(&totals[0]),
                read(&totals[1]),
                read(&totals[2]),
                read(&totals[3])
            )
        };
        report.push_str(&format!("{set:?} boards: {n}, {} cities\n", 2 * n));
        report.push_str(&line("cities lost", &|h| h.cities_lost));
        report.push_str(&line("turns cities held", &|h| h.turns_held));
        report.push_str(&line("our starting units lost", &|h| h.ours_lost));
        report.push_str(&line("enemy starting units killed", &|h| h.theirs_lost));
        report.push_str(&line("empty-centre turn ends", &|h| h.empty_centre_turns));
        report.push_str(&line("posts drawn", &|h| h.posts));
        report.push_str(&format!(
            "  {:<26} {:>5} | {:>5} | {:>5} | {:>5}\n",
            "boards better/worse vs off",
            "-",
            format!("{}/{}", better[1], worse[1]),
            format!("{}/{}", better[2], worse[2]),
            format!("{}/{}", better[3], worse[3])
        ));
        for held in &totals {
            broken += held.posts_broken;
            shared += held.shared_tiles;
            posts += held.posts;
        }
    }
    eprintln!("{report}");
    assert!(posts > 0, "the hold drew posts\n{report}");
    assert_eq!(broken, 0, "a posted unit left its post\n{report}");
    assert_eq!(shared, 0, "a tile was posted twice\n{report}");
    // WIP: the CI log is the read-out.
    panic!("{report}");
}
