use super::*;

/// Two majors, each with its founded capital and nothing else; no barbarian
/// units and no camps yet; seat 0 to move. (The same board as the camp
/// errand pins in `advanced/tests.rs`.)
fn camp_board(seed: u64) -> (Game, Pos) {
    let mut game = Game::new_full(2, 30, 20, seed, 120, 0, true);
    for player in 0..2 {
        let settler = game
            .player_unit_ids(player)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("each player opens with a settler");
        game.current = player;
        game.apply(player, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    for player in 0..2 {
        for uid in game.player_unit_ids(player) {
            game.remove_unit(uid);
        }
    }
    let barb = game.barb_pid.expect("a barbarian-seated game has barb_pid");
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        if game.units[&uid].owner == barb {
            game.remove_unit(uid);
        }
    }
    game.barb_camps.clear();
    game.current = 0;
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    (game, home)
}

fn open(game: &Game, pos: Pos) -> bool {
    game.map
        .get(pos)
        .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        && game.city_at(pos).is_none()
        && game.unit_ids_at(pos).is_empty()
}

/// A flat-enough open tile at `distance` from `anchor`, chosen
/// deterministically.
fn open_at(game: &Game, anchor: Pos, distance: i32) -> Pos {
    let mut ring: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, anchor) == distance && open(game, *pos))
        .collect();
    ring.sort_unstable();
    ring.into_iter()
        .next()
        .expect("open ground at the distance")
}

/// A camp three tiles from the capital and a Warrior of ours on an open tile
/// beside it that can step onto it this turn.
fn empty_camp_beside_a_warrior(seed: u64) -> (Game, Pos, u32) {
    let (mut game, home) = camp_board(seed);
    let mut chosen = None;
    let mut rings: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 3 && open(&game, *pos))
        .collect();
    rings.sort_unstable();
    'search: for camp in rings {
        let mut beside: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| game.wdist(*pos, camp) == 1 && open(&game, *pos))
            .collect();
        beside.sort_unstable();
        for stand in beside {
            let uid = game.spawn_test_unit("warrior", 0, stand);
            if game.reachable(uid).contains(&camp) {
                chosen = Some((camp, uid));
                break 'search;
            }
            game.remove_unit(uid);
        }
    }
    let (camp, uid) = chosen.expect("an open camp tile a Warrior beside it can enter");
    game.barb_camps.insert(camp, game.turn + 1_000);
    game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
    (game, camp, uid)
}

fn camp_ai(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = false;
    if on {
        ai.enable_emptied_camp_is_taken();
    } else {
        ai.disable_emptied_camp_is_taken();
    }
    ai
}

#[test]
fn the_gene_is_an_off_by_default_opt_in() {
    assert!(!AdvancedAi::new().emptied_camp_is_taken);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(
        !live.emptied_camp_is_taken,
        "nothing arms it on the live seat by itself"
    );
    let enable = crate::ai::advanced::genes::GENES
        .iter()
        .find(|gene| gene.tag == "emptied-camp-is-taken")
        .expect("an opt-in row")
        .enable;
    enable(&mut live);
    assert!(live.emptied_camp_is_taken);
}

/// The Gaul t36 case in miniature: the camp stands empty, a soldier beside it
/// can enter, and with the gene it walks on and the camp is gone, paid for
/// and counted. Off, the pass does nothing.
#[test]
fn a_soldier_beside_an_empty_camp_walks_onto_it() {
    for on in [true, false] {
        let (mut game, camp, warrior) = empty_camp_beside_a_warrior(91_210);
        let gold = game.players[0].gold;
        let mut ai = camp_ai(on);
        let mut field = ai.danger_field(&game, 0, ai.strike_reach);
        let taken = ai.take_emptied_camps(&mut game, 0, &mut field);
        if on {
            assert_eq!(taken, 1);
            assert_eq!(
                game.units[&warrior].pos, camp,
                "the Warrior must stand on the camp"
            );
            assert!(
                !game.barb_camps.contains_key(&camp),
                "the entry clears the camp"
            );
            assert!(
                game.players[0].gold >= gold + 50.0,
                "the camp pays its bounty"
            );
            assert!(ai.battle_planner_claims(warrior), "the plan keeps it there");
        } else {
            assert_eq!(taken, 0);
            assert_ne!(game.units[&warrior].pos, camp);
            assert!(game.barb_camps.contains_key(&camp));
        }
    }
}

/// A camp with a defender on it stays the kill plan's: the pass only walks
/// onto empty ground.
#[test]
fn an_occupied_camp_is_left_to_the_fight() {
    let (mut game, camp, warrior) = empty_camp_beside_a_warrior(91_211);
    let barb = game.barb_pid.unwrap();
    game.spawn_test_unit("warrior", barb, camp);
    let mut ai = camp_ai(true);
    let mut field = ai.danger_field(&game, 0, ai.strike_reach);
    assert_eq!(ai.take_emptied_camps(&mut game, 0, &mut field), 0);
    assert_ne!(game.units[&warrior].pos, camp);
    assert!(game.barb_camps.contains_key(&camp));
}

/// A city's garrison keeps its city, and a soldier that has already moved
/// its last point, or that the plan has already ordered, is not taken.
#[test]
fn the_garrison_a_spent_soldier_and_an_ordered_one_stay_put() {
    // The garrison: a camp beside the capital, the only soldier in the city.
    let (mut game, home) = camp_board(91_212);
    let camp = open_at(&game, home, 1);
    game.barb_camps.insert(camp, game.turn + 1_000);
    game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
    let garrison = game.spawn_test_unit("warrior", 0, home);
    let mut ai = camp_ai(true);
    let mut field = ai.danger_field(&game, 0, ai.strike_reach);
    assert_eq!(ai.take_emptied_camps(&mut game, 0, &mut field), 0);
    assert_eq!(game.units[&garrison].pos, home);

    // A spent soldier.
    let (mut game, camp, warrior) = empty_camp_beside_a_warrior(91_213);
    game.units.get_mut(&warrior).unwrap().moves_left = 0.0;
    let mut ai = camp_ai(true);
    let mut field = ai.danger_field(&game, 0, ai.strike_reach);
    assert_eq!(ai.take_emptied_camps(&mut game, 0, &mut field), 0);
    assert!(game.barb_camps.contains_key(&camp));

    // One the plan already ordered this turn.
    let (mut game, camp, warrior) = empty_camp_beside_a_warrior(91_214);
    let mut ai = camp_ai(true);
    ai.battle_planner_ordered.insert(warrior);
    let mut field = ai.danger_field(&game, 0, ai.strike_reach);
    assert_eq!(ai.take_emptied_camps(&mut game, 0, &mut field), 0);
    assert!(game.barb_camps.contains_key(&camp));
}

/// The hook: with `battle-planner-2` on, `plan_battle` itself steps onto the
/// empty camp; with the gene off the same plan leaves the camp standing on
/// this turn's pass.
#[test]
fn the_battle_plan_takes_the_empty_camp_before_its_positions() {
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 0,
        rush: false,
    };
    for on in [true, false] {
        let (mut game, camp, warrior) = empty_camp_beside_a_warrior(91_215);
        let mut ai = camp_ai(on);
        ai.enable_battle_planner_2();
        ai.rebuild_force_groups(&game, 0, &plan);
        let _ = ai.plan_battle(&mut game, 0, &plan);
        assert_eq!(
            !game.barb_camps.contains_key(&camp),
            on,
            "the camp is taken exactly when the gene is on"
        );
        if on {
            assert_eq!(game.units[&warrior].pos, camp);
        }
    }
}
