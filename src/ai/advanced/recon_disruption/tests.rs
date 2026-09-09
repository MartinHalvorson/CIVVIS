use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::AdvancedAi;
use super::*;
use crate::game::{Action, Game};
use crate::name;

#[test]
fn pass_picket_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("pass-picket", |ai| ai.pass_picket);
}

/// A flat two-major board at peace with every starting unit cleared, the
/// map explored by both, no terrain anywhere. Returns the game and a
/// patch of open neutral ground far from both capitals.
fn peace_field(seed: u64) -> (Game, Pos) {
    let mut game = Game::new_full(2, 28, 18, seed, 1_000, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("each fixture major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
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
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    for pid in 0..2 {
        game.players[pid]
            .explored
            .extend(game.map.tiles.keys().copied());
    }
    game.turn = 60;
    game.current = 0;
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let rival = game.cities[&game.player_city_ids(1)[0]].pos;
    let field = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.wdist(*position, home) >= 8
                && game.wdist(*position, home) <= 10
                && game.wdist(*position, rival) >= 8
                && game.map.tiles[position].owner_city.is_none()
                && game.wdisk(*position, 5).len() == game.wdisk(home, 5).len()
        })
        .min()
        .expect("the fixture board has open neutral ground near our capital");
    assert!(!game.is_at_war(0, 1));
    (game, field)
}

/// Start of turn for one unit: full movement, nothing spent.
fn fresh(game: &mut Game, uid: u32) {
    let moves = game.unit_max_moves(uid);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
}

fn mountain(game: &mut Game, pos: Pos) {
    let tile = game.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = name!("mountain");
    tile.hills = false;
}

#[test]
fn the_picket_is_off_by_default_and_draws_no_orders_when_off() {
    let (mut game, field) = peace_field(33);
    let settler = game.spawn_test_unit("settler", 1, field);
    let scout = game.spawn_test_unit("scout", 0, game.nbrs(field).into_iter().min().unwrap());
    fresh(&mut game, scout);
    fresh(&mut game, settler);
    let mut ai = AdvancedAi::new();
    ai.recon_disruption_plan(&game, 0);
    assert_eq!(ai.recon_disruption.turn, None);
    assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), None);
    assert_eq!(
        game.units[&scout].pos,
        game.nbrs(field).into_iter().min().unwrap()
    );
}

/// Two capitals joined by a one-tile corridor through mountains, every
/// other tile of the board a mountain: every corridor tile is a pass.
fn corridor_board() -> (Game, Vec<Pos>, Pos, Pos) {
    let (mut game, _) = peace_field(35);
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let away = game.cities[&game.player_city_ids(1)[0]].pos;
    let flat = |pos: Pos| game.map.get(pos).is_some();
    let walk = AdvancedAi::land_walk(&game, away, home, &flat, None).expect("a flat walk");
    let keep: BTreeSet<Pos> = game
        .wdisk(home, 3)
        .into_iter()
        .chain(game.wdisk(away, 3))
        .chain(walk.iter().copied())
        .collect();
    for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if !keep.contains(&pos) {
            mountain(&mut game, pos);
        }
    }
    (game, walk, home, away)
}

#[test]
fn an_idle_scout_holds_the_pass_toward_the_neighbour() {
    let (mut game, walk, home, away) = corridor_board();
    let scout = game.spawn_test_unit("scout", 0, home);
    fresh(&mut game, scout);
    let mut ai = AdvancedAi::new();
    ai.enable_pass_picket();
    ai.recon_disruption_plan(&game, 0);
    let post = *ai
        .recon_disruption
        .post(1)
        .expect("the neighbour has a post");
    assert!(post.pass, "every corridor tile cuts the walk");
    assert!(walk.contains(&post.at));
    assert!(game.wdist(post.at, away) >= PICKET_MIN_FROM_CITY);
    assert!(
        game.map.tiles[&post.at]
            .owner_city
            .and_then(|city| game.cities.get(&city))
            .is_none_or(|city| city.owner == 0),
        "the post is never inside the rival's borders"
    );
    // The forward-most pass: the post cuts the walk and no standable
    // tile nearer the rival city does (the open ground round the city).
    let passable = |pos: Pos| {
        game.map
            .get(pos)
            .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
    };
    assert!(AdvancedAi::land_walk(&game, away, home, &passable, Some(post.at)).is_none());
    for pos in walk.iter().copied().filter(|pos| {
        game.wdist(*pos, away) < game.wdist(post.at, away)
            && game.wdist(*pos, away) >= PICKET_MIN_FROM_CITY
            && AdvancedAi::post_stands(&game, 0, *pos)
    }) {
        assert!(
            AdvancedAi::land_walk(&game, away, home, &passable, Some(pos)).is_some(),
            "{pos:?} nearer the rival city does not cut the walk"
        );
    }
    assert_eq!(ai.recon_disruption.picket(scout), Some(post.at));
    // Nothing to explore: the Scout walks to the post over the turns and
    // then holds it.
    let mut turns = 0;
    loop {
        fresh(&mut game, scout);
        game.turn += 1;
        ai.recon_disruption_plan(&game, 0);
        let step = ai.recon_disruption_step(&mut game, 0, scout);
        if game.units[&scout].pos == post.at {
            assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), Some(false));
            break;
        }
        assert_eq!(step, Some(true), "the Scout walks toward its post");
        turns += 1;
        assert!(turns < 30, "the post is reached within thirty turns");
    }
    assert_eq!(
        ai.recon_disruption.post(1).map(|post| post.at),
        Some(post.at)
    );
}

#[test]
fn the_border_watch_is_the_first_tile_outside_the_border_when_nothing_cuts_the_walk() {
    let (mut game, _) = peace_field(36);
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let away = game.cities[&game.player_city_ids(1)[0]].pos;
    let scout = game.spawn_test_unit("scout", 0, home);
    fresh(&mut game, scout);
    let mut ai = AdvancedAi::new();
    ai.enable_pass_picket();
    ai.recon_disruption_plan(&game, 0);
    let post = *ai
        .recon_disruption
        .post(1)
        .expect("the neighbour has a post");
    assert!(
        !post.pass,
        "an open field has no single tile that cuts the walk"
    );
    assert!(
        game.wdist(post.at, away) < game.wdist(post.at, home),
        "the watch stands on their side"
    );
    assert!(game.map.tiles[&post.at]
        .owner_city
        .and_then(|city| game.cities.get(&city))
        .is_none_or(|city| city.owner == 0));
    assert!(game.wdist(post.at, away) >= PICKET_MIN_FROM_CITY);
}

/// The claim is checked in a real game: on a small four-major board with
/// both genes on for every seat, the plan draws screen stands and picket
/// posts — including a pass — before the turn limit.
#[test]
fn the_picket_draws_orders_in_a_real_game() {
    use crate::ai::Ai;
    let mut game = Game::new_full(4, 44, 28, 26_082_413, 140, 2, true);
    // This pins the recon gene on the board it was written against; the
    // barbarian seat's rung moved to Immortal by default on 2026-08-24
    // and is not what is under test here.
    game.set_barbarian_difficulty("prince").unwrap();
    game.set_fog_memory(false);
    game.set_war_ledger(false);
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_pass_picket();
            ai
        })
        .collect();
    let (mut pickets, mut passes) = (0usize, 0usize);
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        ais[pid].take_turn(&mut game, pid);
        let plan = &ais[pid].recon_disruption;
        if plan.turn == Some(game.turn) {
            pickets += plan.pickets.len();
            passes += plan.posts.values().filter(|post| post.pass).count();
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    assert!(pickets > 0, "a recon unit was sent to a post");
    assert!(
        passes > 0,
        "a pass was found on some walk ({pickets} picket-turns)"
    );
}

#[test]
fn version_two_holds_a_settlers_pass_while_version_one_explores() {
    let (mut game, walk, home, _) = corridor_board();
    let scout = game.spawn_test_unit("scout", 0, home);
    let mut ai = AdvancedAi::new();
    ai.enable_pass_picket_2();
    ai.recon_disruption_plan(&game, 0);
    let post = ai.recon_disruption.post(1).unwrap().at;
    game.relocate(scout, post);
    fresh(&mut game, scout);
    let settler_at = walk
        .iter()
        .copied()
        .find(|pos| game.wdist(*pos, post) == 1 && game.wdist(*pos, home) > game.wdist(post, home))
        .expect("the far side of the pass");
    let settler = game.spawn_test_unit("settler", 1, settler_at);
    fresh(&mut game, settler);
    let unexplored = walk
        .iter()
        .copied()
        .find(|pos| game.wdist(*pos, post) == 1 && game.wdist(*pos, home) < game.wdist(post, home))
        .expect("frontier on our side of the pass");
    game.players[0].explored.remove(&unexplored);
    assert!(ai.settler_at_picket(&game, 0, post));

    let mut original = ai.clone();
    original.enable_pass_picket();
    let mut explored = game.clone();
    assert_eq!(
        original.recon_disruption_step(&mut explored, 0, scout),
        Some(true)
    );
    assert_ne!(explored.units[&scout].pos, post, "v1 leaves to explore");
    assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), Some(false));
    assert_eq!(game.units[&scout].pos, post);
    assert!(
        !game.can_move(settler, post),
        "the occupation really blocks the settler"
    );
    let mut clear = game.clone();
    clear.remove_unit(scout);
    assert!(
        clear.can_move(settler, post),
        "the scout is the blocker, not terrain or borders"
    );
    assert!(!game.is_at_war(0, 1));

    game.remove_unit(settler);
    assert!(!ai.settler_at_picket(&game, 0, post));
    assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), Some(true));
    assert_ne!(
        game.units[&scout].pos, post,
        "exploration resumes after the opening closes"
    );
}

#[test]
fn settler_picket_versions_are_independent_opt_ins() {
    opt_in_off_in_both_controllers("pass-picket-2", |ai| ai.pass_picket_2);
    let mut ai = AdvancedAi::new();
    ai.enable_pass_picket_2();
    assert!(!ai.pass_picket);
    ai.disable_pass_picket();
    assert!(ai.pass_picket_2);
    ai.enable_pass_picket();
    assert!(!ai.pass_picket_2);
    ai.disable_pass_picket_2();
    assert!(ai.pass_picket);
}
