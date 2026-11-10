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
    for seed in [936_733u64, 41_005] {
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
