use super::*;
use crate::ai::Ai;
use crate::game::Game;
use crate::name;

/// A flat grassland board of `capitals.len()` empires, every starting unit
/// cleared, the map explored and everyone met, nobody at war, turn 60. (The
/// objective board's test board.)
fn flat_board(seed: u64, capitals: &[Pos]) -> Game {
    flat_board_with(seed, capitals, false)
}

/// [`flat_board`], with a barbarian seat when `barbarians` (its camps
/// cleared).
fn flat_board_with(seed: u64, capitals: &[Pos], barbarians: bool) -> Game {
    let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, barbarians);
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
    let west: Vec<u32> = [at(7, 7), at(7, 9), at(6, 8)]
        .into_iter()
        .map(|pos| spawn(&mut g, "spearman", 0, pos))
        .collect();
    let enemies: Vec<u32> = [at(10, 7), at(11, 8), at(10, 9)]
        .into_iter()
        .map(|pos| spawn(&mut g, "swordsman", 1, pos))
        .collect();
    // The ring tiles an enemy swordsman can stand on next turn, read before
    // anyone moves.
    let reach: BTreeSet<Pos> = enemies
        .iter()
        .flat_map(|uid| g.threat_reach(*uid))
        .collect();
    let threatened: Vec<Pos> = g
        .nbrs(city)
        .into_iter()
        .filter(|tile| reach.contains(tile))
        .collect();
    assert!(threatened.len() >= 2, "the board threatens the east ring");
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    let ring_posts: Vec<Pos> = west
        .iter()
        .filter_map(|uid| ai.defend_hold_plan.posts.get(uid))
        .filter(|post| post.role == HoldRole::Ring)
        .map(|post| post.at)
        .collect();
    // The two spearmen on the west ring can each reach an east tile this
    // turn; the third, two tiles west, cannot.
    assert!(
        ring_posts.len() >= 2,
        "the spearmen in reach block the threatened ring: {ring_posts:?} of {threatened:?}"
    );
    for tile in &ring_posts {
        assert!(
            threatened.contains(tile),
            "a block stands where the enemy can reach"
        );
        assert!(g.unit_ids_at(*tile).iter().any(|id| west.contains(id)));
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
// Version two.
// ----------------------------------------------------------------------

fn v2_on() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_defend_force_holds_the_city_2();
    ai
}

#[test]
fn version_two_turns_version_one_off_and_carries_its_reads() {
    let mut ai = AdvancedAi::new();
    ai.enable_defend_force_holds_the_city();
    ai.enable_defend_force_holds_the_city_2();
    assert!(!ai.defend_force_holds_the_city);
    assert!(ai.defend_force_holds_the_city_2);
    assert!(ai.defend_hold_on());
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    {
        let beside = ring_tile_toward(&g, city, at(12, 8));
        spawn(&mut g, "swordsman", 1, beside)
    };
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_eq!(ai.defend_hold_plan.posts[&warrior].role, HoldRole::Centre);
    assert!(ai.defend_hold_holds(g.city_at(city).unwrap()));
}

#[test]
fn barbarians_form_no_hold_under_version_two() {
    let city = at(8, 8);
    let build = || {
        let mut g = flat_board_with(11, &[city, at(28, 8)], true);
        let barbarian = g.barb_pid.expect("a barbarian seat");
        spawn(&mut g, "warrior", 0, city);
        spawn(&mut g, "spearman", 0, at(7, 9));
        {
            let beside = ring_tile_toward(&g, city, at(12, 8));
            spawn(&mut g, "horseman", barbarian, beside)
        };
        g
    };
    let mut g = build();
    let mut ai = v2_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert!(
        ai.defend_hold_plan.cities.is_empty(),
        "a raider is no capturer"
    );
    assert!(ai.defend_hold_plan.posts.is_empty());
    // Version one held it.
    let mut g = build();
    let mut ai = gene_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert_eq!(ai.defend_hold_plan.cities.len(), 1);
}

#[test]
fn no_ring_until_the_city_can_fall() {
    // One swordsman three tiles out against a full-health city: no fall
    // within two enemy turns, so the spearmen stand ready off the ring.
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let west: Vec<u32> = [at(7, 7), at(7, 9)]
        .into_iter()
        .map(|pos| spawn(&mut g, "spearman", 0, pos))
        .collect();
    spawn(&mut g, "swordsman", 1, at(11, 8));
    let mut ai = v2_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    for uid in &west {
        let post = &ai.defend_hold_plan.posts[uid];
        assert_ne!(post.role, HoldRole::Ring, "no ring post before it matters");
        assert!(g.wdist(post.at, city) <= 2, "it stands ready within two");
    }
}

#[test]
fn the_ring_forms_once_a_capturer_stands_beside_the_city() {
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let spearmen: Vec<u32> = [at(7, 7), at(7, 9)]
        .into_iter()
        .map(|pos| spawn(&mut g, "spearman", 0, pos))
        .collect();
    let beside = ring_tile_toward(&g, city, at(12, 8));
    spawn(&mut g, "warrior", 1, beside);
    spawn(&mut g, "warrior", 1, at(11, 7));
    let mut ai = v2_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    let ring = spearmen
        .iter()
        .filter(|uid| ai.defend_hold_plan.posts[*uid].role == HoldRole::Ring)
        .count();
    assert!(
        ring >= 1,
        "a capturer beside the centre: the ring is posted"
    );
}

#[test]
fn an_outclassed_ring_takes_no_soldier_one_blow_removes() {
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let warriors: Vec<u32> = [at(7, 7), at(7, 9)]
        .into_iter()
        .map(|pos| spawn(&mut g, "warrior", 0, pos))
        .collect();
    let beside = ring_tile_toward(&g, city, at(12, 8));
    spawn(&mut g, "tank", 1, beside);
    spawn(&mut g, "tank", 1, at(11, 7));
    spawn(&mut g, "tank", 1, at(11, 9));
    let mut ai = v2_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    for uid in &warriors {
        assert_ne!(
            ai.defend_hold_plan.posts.get(uid).map(|post| post.role),
            Some(HoldRole::Ring),
            "a warrior is not posted beside tanks"
        );
    }
}

#[test]
fn a_full_ring_is_posted_whatever_the_odds() {
    // Six warriors on the six ring tiles, two tanks three tiles out: each
    // warrior dies to one blow, but a full ring stops the capture this turn.
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let ring: Vec<Pos> = g.nbrs(city).into_iter().collect();
    let warriors: Vec<u32> = ring
        .iter()
        .map(|tile| spawn(&mut g, "warrior", 0, *tile))
        .collect();
    spawn(&mut g, "tank", 1, at(11, 8));
    spawn(&mut g, "tank", 1, at(11, 9));
    let mut ai = v2_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    let posted = warriors
        .iter()
        .filter(|uid| {
            ai.defend_hold_plan.posts.get(*uid).map(|post| post.role) == Some(HoldRole::Ring)
        })
        .count();
    // Without the full-ring rule no warrior beside a tank would be posted
    // (see `an_outclassed_ring_takes_no_soldier_one_blow_removes`); every
    // threatened tile is a warrior's, so all of them are.
    assert!(posted >= 2, "the threatened ring is held in full: {posted}");
}

#[test]
fn a_civilians_guard_keeps_guarding() {
    let (mut g, city) = front();
    spawn(&mut g, "archer", 0, city);
    let guard = spawn(&mut g, "spearman", 0, at(7, 9));
    spawn(&mut g, "builder", 0, at(7, 9));
    {
        let beside = ring_tile_toward(&g, city, at(12, 8));
        spawn(&mut g, "swordsman", 1, beside)
    };
    let mut ai = v2_on();
    ai.hold_the_cities(&mut g, 0, &BTreeSet::new());
    assert!(!ai.defend_hold_plan.posts.contains_key(&guard));
    assert_eq!(g.units[&guard].pos, at(7, 9));
}

#[test]
fn the_positions_plan_leaves_this_turns_posts_alone() {
    let (mut g, city) = front();
    let warrior = spawn(&mut g, "warrior", 0, city);
    {
        let beside = ring_tile_toward(&g, city, at(12, 8));
        spawn(&mut g, "swordsman", 1, beside)
    };
    let mut ai = v2_on();
    ai.preview_hold_members(&g, 0);
    assert!(ai.defend_hold_member_v2(warrior));
    // Version one leaves the planner as it was.
    let mut ai = gene_on();
    ai.preview_hold_members(&g, 0);
    assert!(!ai.defend_hold_member_v2(warrior));
    assert!(ai.defend_hold_members.is_empty());
}

// ----------------------------------------------------------------------
// Paired posed defences: off, version one, version two, and version two
// with `centre-keeps-its-garrison`, on the same boards.
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
    v1: bool,
    v2: bool,
    keep: bool,
}

const ARMS: [(&str, Arm); 4] = [
    (
        "off",
        Arm {
            v1: false,
            v2: false,
            keep: false,
        },
    ),
    (
        "v1",
        Arm {
            v1: true,
            v2: false,
            keep: false,
        },
    ),
    (
        "v2",
        Arm {
            v1: false,
            v2: true,
            keep: false,
        },
    ),
    (
        "v2+keep",
        Arm {
            v1: false,
            v2: true,
            keep: true,
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
    ai.disable_centre_keeps_its_garrison();
    ai.disable_defend_force_holds_the_city();
    ai.disable_defend_force_holds_the_city_2();
    if arm.keep {
        ai.enable_centre_keeps_its_garrison();
    }
    if arm.v1 {
        ai.enable_defend_force_holds_the_city();
    }
    if arm.v2 {
        ai.enable_defend_force_holds_the_city_2();
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

/// The board sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Set {
    /// #5198's 96 boards: ancient garrisons, 1-3 soldiers beside the front
    /// city, 3-6 ancient attackers.
    Light,
    /// A real force: medieval garrisons, 4-8 mixed soldiers about the front
    /// city, 5-8 medieval and Renaissance attackers.
    Force,
    /// Peace with the rival; 3-5 barbarian raiders about the front city,
    /// two Builders working beside it.
    Raiders,
    /// Medieval defenders against 5-8 industrial and modern attackers.
    Outclassed,
}

/// What a board set fields: our garrisons, our soldiers about the front
/// city and how many, the attackers, how many, and how far out.
struct Mix {
    garrison: &'static [&'static str],
    own: &'static [&'static str],
    theirs: &'static [&'static str],
    extra: (i32, i32),
    attackers: (i32, i32),
    near: i32,
    far: i32,
}

fn mix(set: Set) -> Mix {
    match set {
        Set::Light => Mix {
            garrison: &["warrior", "slinger", "archer", "spearman", "warrior"],
            own: &["warrior", "archer", "spearman", "horseman", "slinger"],
            theirs: &[
                "swordsman",
                "horseman",
                "archer",
                "spearman",
                "catapult",
                "warrior",
                "swordsman",
            ],
            extra: (1, 3),
            attackers: (3, 6),
            near: 2,
            far: 3,
        },
        Set::Force => Mix {
            garrison: &[
                "crossbowman",
                "pikeman",
                "man_at_arms",
                "archer",
                "spearman",
            ],
            own: &[
                "crossbowman",
                "pikeman",
                "man_at_arms",
                "knight",
                "swordsman",
                "archer",
                "spearman",
                "horseman",
            ],
            theirs: &[
                "knight",
                "crossbowman",
                "pikeman",
                "man_at_arms",
                "trebuchet",
                "musketman",
                "bombard",
                "cuirassier",
            ],
            extra: (4, 8),
            attackers: (5, 8),
            near: 2,
            far: 4,
        },
        Set::Raiders => Mix {
            garrison: &["warrior", "archer", "spearman", "slinger"],
            own: &["warrior", "archer", "spearman", "horseman"],
            theirs: &["warrior", "spearman", "horseman", "archer", "warrior"],
            extra: (1, 3),
            attackers: (3, 5),
            near: 2,
            far: 4,
        },
        Set::Outclassed => Mix {
            garrison: &["crossbowman", "pikeman", "musketman", "man_at_arms"],
            own: &[
                "crossbowman",
                "pikeman",
                "musketman",
                "man_at_arms",
                "knight",
                "archer",
            ],
            theirs: &[
                "line_infantry",
                "cavalry",
                "infantry",
                "artillery",
                "machine_gun",
                "field_cannon",
                "tank",
            ],
            extra: (3, 6),
            attackers: (5, 8),
            near: 2,
            far: 4,
        },
    }
}

/// One posed defence: two cities of ours, a garrison in each, soldiers
/// around the front city, and an attacking force east of it (the rival at
/// war, or barbarians at peace with the rival).
fn posed(seed: u64, set: Set) -> Game {
    let mut rng = crate::rng::Rng::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5eed);
    let capital = at(8, 6);
    let raiders = set == Set::Raiders;
    let mut g = flat_board_with(seed, &[capital, at(29, 8)], raiders);
    let second = at(8 + rng.randint(-1, 1), 13 + rng.randint(0, 2));
    g.found_city_for(0, second, None);
    let attacker = if raiders {
        g.barb_pid.expect("a barbarian seat")
    } else {
        war(&mut g, 0, 1);
        1
    };
    let ours = [capital, second];
    let front = rng.below(2);
    let Mix {
        garrison,
        own,
        theirs,
        extra,
        attackers,
        near,
        far,
    } = mix(set);
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
    if raiders {
        for _ in 0..2 {
            let tiles = ring(&g, ours[front], 1, 2);
            if let Some(pos) = tiles.get(rng.below(tiles.len().max(1))) {
                spawn(&mut g, "builder", 0, *pos);
            }
        }
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
        spawn(&mut g, theirs[rng.below(theirs.len())], attacker, pos);
    }
    if !raiders && rng.chance(0.4) {
        for _ in 0..rng.randint(2, 3) {
            let tiles: Vec<Pos> = ring(&g, ours[1 - front], near, far)
                .into_iter()
                .filter(|pos| east_of(*pos, ours[1 - front]))
                .collect();
            if tiles.is_empty() {
                break;
            }
            let pos = tiles[rng.below(tiles.len())];
            spawn(&mut g, theirs[rng.below(theirs.len())], attacker, pos);
        }
    }
    g
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    cities_lost: usize,
    ours_lost: usize,
    civilians_lost: usize,
    theirs_lost: usize,
    /// Our turns that ended with a threatened City Center of ours empty.
    empty_centre_turns: usize,
    /// Turns each starting city of ours stayed ours, summed.
    turns_held: usize,
    /// Posts drawn, and the mechanism checks: posts a unit ended our turn
    /// off (other than by its own blow, a refill or death), and tiles posted
    /// twice.
    posts: usize,
    ring_posts: usize,
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
    let ours_of = |g: &Game, class: &str| -> BTreeSet<u32> {
        g.units
            .values()
            .filter(|unit| unit.owner == 0 && g.rules.units[unit.kind].class == class)
            .map(|unit| unit.id)
            .collect()
    };
    let enemy_start: BTreeSet<u32> = g
        .units
        .values()
        .filter(|unit| unit.owner != 0 && g.rules.units[unit.kind].class == "military")
        .map(|unit| unit.id)
        .collect();
    let ours_start = ours_of(&g, "military");
    let civilians_start = ours_of(&g, "civilian");
    let probe = gene_on();
    let mut us = our_seat(arm);
    let mut them = attacker_seat();
    let mut others: BTreeMap<usize, AdvancedAi> = BTreeMap::new();
    let mut held = Held::default();
    let mut lost_at: BTreeMap<Pos, u32> = BTreeMap::new();
    let mut guard = 0;
    while g.winner.is_none() && g.turn < start + POSED_TURNS && g.players[0].alive {
        guard += 1;
        assert!(guard < 400, "the posed game advances");
        let pid = g.current;
        if pid == 0 {
            us.take_turn(&mut g, pid);
            let plan = us.defend_hold_plan.clone();
            if !(arm.v1 || arm.v2) {
                assert!(plan.posts.is_empty(), "the gene off draws no posts");
                assert!(us.defend_hold_members.is_empty());
            }
            let mut tiles = BTreeSet::new();
            for (uid, post) in &plan.posts {
                held.posts += 1;
                if post.role == HoldRole::Ring {
                    held.ring_posts += 1;
                }
                if post.role != HoldRole::March && !tiles.insert(post.at) {
                    held.shared_tiles += 1;
                }
                let Some(unit) = g.units.get(uid) else {
                    continue;
                };
                let centre = g.cities.get(&post.city).map(|city| city.pos);
                let struck = unit.attacks_left == 0;
                let on_post = unit.pos == post.at
                    || Some(unit.pos) == centre
                    || (struck && (arm.v2 || g.wdist(unit.pos, post.at) == 1));
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
        } else if pid == 1 && set != Set::Raiders {
            them.take_turn(&mut g, pid);
        } else {
            // `AdvancedAi::default()` is the blank agent; a seat plays `new()`.
            #[allow(clippy::unwrap_or_default)]
            let seat = others.entry(pid).or_insert_with(AdvancedAi::new);
            seat.take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if set != Set::Raiders {
            war(&mut g, 0, 1);
            g.peace_treaties.remove(&(0, 1));
        }
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
    held.ours_lost = ours_start.difference(&ours_of(&g, "military")).count();
    held.civilians_lost = civilians_start.difference(&ours_of(&g, "civilian")).count();
    held.theirs_lost = enemy_start
        .iter()
        .filter(|uid| !g.units.contains_key(uid))
        .count();
    held
}

fn add(total: &mut Held, one: &Held) {
    total.cities_lost += one.cities_lost;
    total.ours_lost += one.ours_lost;
    total.civilians_lost += one.civilians_lost;
    total.theirs_lost += one.theirs_lost;
    total.empty_centre_turns += one.empty_centre_turns;
    total.turns_held += one.turns_held;
    total.posts += one.posts;
    total.ring_posts += one.ring_posts;
    total.posts_broken += one.posts_broken;
    total.shared_tiles += one.shared_tiles;
}

/// Off, version one, version two, and version two with
/// `centre-keeps-its-garrison`, over the same posed boards, ten turns each:
/// #5198's 96 light boards, 64 boards where the defender has a real force,
/// 32 boards of barbarian raiders about a city at peace, and 32 boards of
/// medieval defenders against industrial and modern attackers. Prints the
/// arms side by side (run with `--nocapture` to read it on a pass); asserts
/// only the mechanism: posts are held, no tile is posted twice, the genes
/// off draw nothing, and version two forms no hold against raiders.
#[test]
fn paired_posed_defences_four_arms() {
    let boards: Vec<(Set, u64)> = (1..=96)
        .map(|seed| (Set::Light, seed))
        .chain((1001..=1064).map(|seed| (Set::Force, seed)))
        .chain((2001..=2032).map(|seed| (Set::Raiders, seed)))
        .chain((3001..=3032).map(|seed| (Set::Outclassed, seed)))
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
        rows.sort_by_key(|row| (row.0, row.1));
        rows
    });
    let mut report = String::from(
        "defend-force-holds-the-city-2 paired posed defences (10 turns; arms off | v1 | v2 | v2+keep)\n",
    );
    let mut broken = 0;
    let mut shared = 0;
    let mut posts = 0;
    let mut raider_posts_v2 = 0;
    for set in [Set::Light, Set::Force, Set::Raiders, Set::Outclassed] {
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
                "  {name:<28} {:>5} | {:>5} | {:>5} | {:>5}\n",
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
        report.push_str(&line("our civilians lost", &|h| h.civilians_lost));
        report.push_str(&line("enemy starting units killed", &|h| h.theirs_lost));
        report.push_str(&line("empty-centre turn ends", &|h| h.empty_centre_turns));
        report.push_str(&line("posts drawn", &|h| h.posts));
        report.push_str(&line("ring posts drawn", &|h| h.ring_posts));
        report.push_str(&format!(
            "  {:<28} {:>5} | {:>5} | {:>5} | {:>5}\n",
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
        if set == Set::Raiders {
            raider_posts_v2 = totals[2].posts + totals[3].posts;
        }
    }
    eprintln!("{report}");
    assert!(posts > 0, "the hold drew posts\n{report}");
    assert_eq!(broken, 0, "a posted unit left its post\n{report}");
    assert_eq!(shared, 0, "a tile was posted twice\n{report}");
    assert_eq!(
        raider_posts_v2, 0,
        "version two formed a hold against raiders\n{report}"
    );
    // WIP: the CI log is the read-out.
    panic!("{report}");
}
