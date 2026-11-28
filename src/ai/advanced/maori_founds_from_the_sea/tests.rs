use super::*;

/// A two-major game whose player 0 is the Māori, with its starting Settler
/// and Warrior moved onto a water tile beside dry land, as the live Māori
/// start places them. Returns the game and the Settler's id.
fn maori_at_sea(seed: u64) -> (Game, u32) {
    let mut g = Game::new_full(2, 40, 24, seed, 250, 0, false);
    g.players[0].civ = "Maori".to_string();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .expect("player 0 starts with a Settler");
    let home = g.units[&settler].pos;
    let sea = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_water(tile)
                && g.wdist(**pos, home) <= 6
                && g.nbrs(**pos).iter().any(|n| {
                    g.map
                        .get(*n)
                        .is_some_and(|t| !g.rules.is_water(t) && g.rules.is_passable(t))
                })
        })
        .map(|(pos, _)| *pos)
        .min_by_key(|pos| (g.wdist(*pos, home), *pos))
        .expect("fixture: a coastal water tile near the start");
    for unit in g.player_unit_ids(0) {
        g.relocate(unit, sea);
        let u = g.units.get_mut(&unit).unwrap();
        u.moves_left = 4.0;
        u.acted = false;
    }
    (g, settler)
}

fn play_until_a_city(mut g: Game, mut ai: AdvancedAi, turns: u32) -> (Game, bool) {
    let mut others = AdvancedAi::fleet(&g);
    let stop = g.turn + turns;
    while g.turn < stop && g.winner.is_none() {
        let pid = g.current;
        if pid == 0 {
            ai.take_turn(&mut g, 0);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if !g.player_city_ids(0).is_empty() {
            return (g, true);
        }
    }
    (g, false)
}

/// The live failure: a Māori Settler that starts embarked never founds the
/// capital. Whatever the cause, the seat must hold a city within three turns.
#[test]
fn an_embarked_maori_settler_founds_the_capital() {
    for seed in [936_731u64, 41_003, 77_219] {
        let (g, settler) = maori_at_sea(seed);
        assert!(
            g.is_embarked(&g.units[&settler]),
            "fixture {seed}: the Settler starts embarked"
        );
        let (_, founded) = play_until_a_city(g, AdvancedAi::new(), 3);
        assert!(founded, "seed {seed}: the stock AI founds within 3 turns");
    }
}

/// The same with the live seat's settler genes on.
#[test]
fn an_embarked_maori_settler_founds_under_the_live_settler_genes() {
    for seed in [936_731u64, 41_003, 77_219] {
        let (g, _) = maori_at_sea(seed);
        let mut ai = AdvancedAi::new();
        for tag in [
            "naval-escort-patience",
            "settle-sooner",
            "settler-guard-holds",
            "escort-patience-runs-out",
        ] {
            let gene = super::super::GENES
                .iter()
                .find(|gene| gene.tag == tag)
                .unwrap_or_else(|| panic!("gene {tag} exists"));
            (gene.enable)(&mut ai);
        }
        let (_, founded) = play_until_a_city(g, ai, 3);
        assert!(
            founded,
            "seed {seed}: the live settler genes found within 3 turns"
        );
    }
}

/// The live mirror's turn 1 for the Māori: every plot within 3 of the start is
/// revealed ocean and everything beyond is fog with no land or sea prior (the
/// land prior grows only from revealed land, the sea prior is read by ships
/// alone). Returns the game, the Settler and its start.
fn maori_in_the_fog(seed: u64) -> (Game, u32, Pos) {
    let (mut g, settler) = maori_at_sea(seed);
    std::sync::Arc::make_mut(&mut g.rules).enable_unknown_terrain();
    let start = g.units[&settler].pos;
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        let near = g.wdist(pos, start) <= 3;
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        if near {
            tile.terrain = crate::name!("ocean");
            tile.feature = None;
            tile.resource = None;
        } else {
            tile.terrain = crate::name!("unknown");
            tile.assumed_traversable = false;
            tile.assumed_navigable = false;
        }
    }
    g.players[0].explored = g.wdisk(start, 3).into_iter().collect();
    (g, settler, start)
}

fn live_settler_ai(sails: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();
    for tag in [
        "naval-escort-patience",
        "settle-sooner",
        "settler-guard-holds",
        "escort-patience-runs-out",
    ] {
        let gene = super::super::GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .unwrap_or_else(|| panic!("gene {tag} exists"));
        (gene.enable)(&mut ai);
    }
    if sails {
        ai.enable_founding_voyage();
    }
    ai
}

/// The voyage step on the live mirror's turn-1 view: it sails toward the edge
/// of the known sea nearest the middle of the map. The full live failure (the
/// decider issuing no order at all) reproduces only on the recorded mirror:
/// `civvis-orders --mirror <run> --turn 2` on 20261103T142545Z emits no order
/// without the gene and `MOVE_TO` for the Settler with it.
#[test]
fn a_fogged_sea_start_sails_toward_the_middle() {
    for seed in [936_731u64, 41_003] {
        let (mut g, settler, start) = maori_in_the_fog(seed);
        let centre = (g.map.width / 2, g.map.height / 2);
        let stock = live_settler_ai(false);
        assert!(
            !stock.founding_voyage_step(&mut g, 0, settler),
            "off: no voyage"
        );
        let ai = live_settler_ai(true);
        assert!(
            ai.founding_voyage_step(&mut g, 0, settler),
            "seed {seed}: on, it sails"
        );
        let to = g.units[&settler].pos;
        assert_ne!(to, start);
        assert!(
            g.wdist(to, centre) < g.wdist(start, centre),
            "seed {seed}: toward the middle of the map"
        );
    }
}

/// With a city, or standing on land, the voyage never moves the Settler.
#[test]
fn the_voyage_is_only_for_a_cityless_seat_at_sea() {
    let (mut g, settler, _) = maori_in_the_fog(936_731);
    let ai = live_settler_ai(true);
    let land = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            !g.rules.is_water(tile) && !g.rules.is_unknown(tile) && g.city_at(**pos).is_none()
        })
        .map(|(pos, _)| *pos);
    if let Some(land) = land {
        g.found_city_for(0, land, None);
        assert!(
            !ai.founding_voyage_step(&mut g, 0, settler),
            "a seat with a city does not sail"
        );
    }
    let (mut g, settler) = maori_at_sea(41_003);
    let home = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            !g.rules.is_water(tile) && g.rules.is_passable(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .expect("fixture: dry land");
    g.relocate(settler, home);
    assert!(
        !ai.founding_voyage_step(&mut g, 0, settler),
        "a Settler on land does not sail"
    );
}

#[test]
fn founding_voyage_is_opt_in() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers("founding-voyage", |ai| {
        ai.founding_voyage
    });
}

#[test]
fn founding_voyage_2_is_opt_in() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers("founding-voyage-2", |ai| {
        ai.founding_voyage_2
    });
}

/// The 37 plots seat p1 of hotseat `live-6p-20261128T0446Zd` (Kupe) had
/// revealed on turn 1, every one `TERRAIN_OCEAN` (its `tiles` record), in host
/// offset coordinates.
const KUPE_REVEALED: [(i32, i32); 37] = [
    (36, 21),
    (36, 22),
    (36, 23),
    (37, 19),
    (37, 20),
    (37, 21),
    (37, 22),
    (37, 23),
    (37, 24),
    (37, 25),
    (38, 19),
    (38, 20),
    (38, 21),
    (38, 22),
    (38, 23),
    (38, 24),
    (38, 25),
    (39, 19),
    (39, 20),
    (39, 21),
    (39, 22),
    (39, 23),
    (39, 24),
    (39, 25),
    (40, 19),
    (40, 20),
    (40, 21),
    (40, 22),
    (40, 23),
    (40, 24),
    (40, 25),
    (41, 20),
    (41, 21),
    (41, 22),
    (41, 23),
    (41, 24),
    (42, 22),
];

/// That seat's turn-1 board: the Small map (74 x 46), the 37 ocean plots and
/// fog everywhere else, the Settler embarked at (39,22) and the Warrior at
/// (38,22) with their 4 moves, no city. Returns the game, Settler and Warrior.
fn kupe_turn_1() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 74, 46, 20_261_128, 250, 0, false);
    g.players[0].civ = "Maori".to_string();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    std::sync::Arc::make_mut(&mut g.rules).enable_unknown_terrain();
    let revealed: Vec<Pos> = KUPE_REVEALED
        .iter()
        .map(|&(x, y)| crate::hex::offset_to_axial(x, y))
        .collect();
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        let seen = revealed.contains(&pos);
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        if seen {
            tile.terrain = crate::name!("ocean");
        } else {
            tile.terrain = crate::name!("unknown");
            tile.assumed_traversable = false;
            tile.assumed_navigable = false;
        }
    }
    let settler = g.spawn_test_unit("settler", 0, crate::hex::offset_to_axial(39, 22));
    let warrior = g.spawn_test_unit("warrior", 0, crate::hex::offset_to_axial(38, 22));
    for uid in [settler, warrior] {
        let unit = g.units.get_mut(&uid).unwrap();
        unit.moves_left = 4.0;
        unit.acted = false;
        unit.moved = false;
    }
    g.players[0].explored = revealed.iter().copied().collect();
    (g, settler, warrior)
}

/// Every voyage step the Settler takes this turn, as `advance_unit_serial`
/// asks for them. Returns how many it took.
fn sail_the_turn(ai: &AdvancedAi, g: &mut Game, settler: u32) -> usize {
    let mut steps = 0;
    while steps < 8 && ai.founding_voyage_step(g, 0, settler) {
        steps += 1;
    }
    steps
}

#[test]
fn on_kupes_board_version_one_sails_alone_and_version_two_takes_the_warrior() {
    let (g, settler, warrior) = kupe_turn_1();
    let start = g.units[&settler].pos;
    let guard_start = g.units[&warrior].pos;
    assert!(g.is_embarked(&g.units[&settler]) && g.is_embarked(&g.units[&warrior]));

    // Off: the live failure, nothing moves.
    let mut off = g.clone();
    assert_eq!(sail_the_turn(&live_settler_ai(false), &mut off, settler), 0);
    assert_eq!(off.units[&settler].pos, start);

    // Version one: the Settler sails, the Warrior stays where it stood.
    let mut one = g.clone();
    assert!(sail_the_turn(&live_settler_ai(true), &mut one, settler) >= 2);
    let sailed_to = one.units[&settler].pos;
    assert!(
        one.wdist(sailed_to, start) >= 2,
        "{start:?} -> {sailed_to:?}"
    );
    assert_eq!(
        one.units[&warrior].pos, guard_start,
        "version one leaves the guard"
    );

    // Version two: the same voyage, and the Warrior sails with it.
    let mut two = g.clone();
    let mut ai = live_settler_ai(false);
    ai.enable_founding_voyage_2();
    ai.base.journal = crate::reasoning::Journal::recording();
    assert!(sail_the_turn(&ai, &mut two, settler) >= 2);
    let settler_at = two.units[&settler].pos;
    let warrior_at = two.units[&warrior].pos;
    assert_eq!(settler_at, sailed_to, "the voyage itself is version one's");
    assert_ne!(warrior_at, guard_start, "the Warrior moved");
    assert!(
        two.wdist(warrior_at, settler_at) <= 1,
        "the Warrior ends beside the Settler: {warrior_at:?} for {settler_at:?}"
    );
    assert!(ai
        .base
        .journal
        .since(u64::MAX)
        .thoughts
        .iter()
        .any(|thought| thought.detail.contains("founding-voyage-2:")));
}

#[test]
fn version_two_leaves_a_land_start_and_a_seat_with_a_city_alone() {
    let mut ai = live_settler_ai(false);
    ai.enable_founding_voyage_2();
    // A Settler on dry land: no voyage, and its soldier is not touched.
    let (mut g, settler) = maori_at_sea(41_003);
    let home = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            !g.rules.is_water(tile) && g.rules.is_passable(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .expect("fixture: dry land");
    g.relocate(settler, home);
    let before: Vec<(u32, Pos)> = g
        .player_unit_ids(0)
        .into_iter()
        .map(|uid| (uid, g.units[&uid].pos))
        .collect();
    assert!(!ai.founding_voyage_step(&mut g, 0, settler));
    let after: Vec<(u32, Pos)> = g
        .player_unit_ids(0)
        .into_iter()
        .map(|uid| (uid, g.units[&uid].pos))
        .collect();
    assert_eq!(before, after, "a land start is unchanged");
    // A seat that holds a city does not sail, at sea or not.
    let (mut g, settler) = maori_at_sea(41_003);
    let land = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && g.unit_ids_at(**pos).is_empty()
                && g.wdist(**pos, g.units[&settler].pos) >= 4
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: dry land away from the Settler");
    g.found_city_for(0, land, None);
    let before: Vec<(u32, Pos)> = g
        .player_unit_ids(0)
        .into_iter()
        .map(|uid| (uid, g.units[&uid].pos))
        .collect();
    assert!(!ai.founding_voyage_step(&mut g, 0, settler));
    let after: Vec<(u32, Pos)> = g
        .player_unit_ids(0)
        .into_iter()
        .map(|uid| (uid, g.units[&uid].pos))
        .collect();
    assert_eq!(before, after, "a seat with a city does not sail");
}

#[test]
fn version_two_takes_the_soldier_on_the_fogged_sea_start_too() {
    for seed in [936_731u64, 41_003] {
        let (g, settler, _) = maori_in_the_fog(seed);
        let soldiers: Vec<u32> = g
            .player_unit_ids(0)
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
            .collect();
        assert!(
            !soldiers.is_empty(),
            "fixture {seed}: a soldier starts at sea"
        );
        let mut ai = live_settler_ai(false);
        ai.enable_founding_voyage_2();
        let mut two = g.clone();
        assert!(sail_the_turn(&ai, &mut two, settler) >= 2, "seed {seed}");
        let at = two.units[&settler].pos;
        assert!(
            soldiers
                .iter()
                .any(|soldier| two.wdist(two.units[soldier].pos, at) <= 1),
            "seed {seed}: a soldier sails with the Settler"
        );
        // Version one leaves every soldier two or more plots behind.
        let mut one = g.clone();
        sail_the_turn(&live_settler_ai(true), &mut one, settler);
        let at = one.units[&settler].pos;
        assert!(
            soldiers
                .iter()
                .all(|soldier| one.wdist(one.units[soldier].pos, at) >= 2),
            "seed {seed}: version one sails alone"
        );
    }
}
