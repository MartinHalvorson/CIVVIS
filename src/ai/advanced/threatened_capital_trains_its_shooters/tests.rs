use super::*;

/// Two majors who have met: our founded capital (population 4) building a
/// Monument, the rival's city 9-12 tiles off, every unit removed, everything
/// within nine tiles in sight, Archery and Masonry known. Returns the game
/// and our capital.
fn board() -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital");
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (9..=12).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, sites[0], None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.turn = 40;
    game.host_observed = Arc::new(game.wdisk(home, 9).into_iter().collect());
    game.players[0].techs.extend([
        crate::name!("animal_husbandry"),
        crate::name!("archery"),
        crate::name!("mining"),
        crate::name!("masonry"),
    ]);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue = vec![monument()];
    city.production = 0.0;
    (game, capital)
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

fn archer() -> Item {
    Item::Unit {
        unit: crate::name!("archer"),
    }
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

/// A land tile exactly `gap` tiles from the capital not yet holding a unit.
fn ground(game: &Game, capital: u32, gap: i32) -> Pos {
    let home = game.cities[&capital].pos;
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == gap
                && !game.rules.is_water(tile)
                && !game.units.values().any(|unit| unit.pos == **pos)
        })
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles[0]
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_threatened_capital_trains_its_shooters();
    ai
}

fn head(game: &Game, capital: u32) -> Option<Item> {
    game.cities[&capital].queue.first().cloned()
}

#[test]
fn an_enemy_soldier_at_the_door_puts_a_shooter_at_the_head() {
    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 2);
    game.spawn_unit("warrior", 1, at);
    let ai = armed();
    assert_eq!(
        ai.claim_threatened_capital_shooters(&mut game, 0),
        Some(archer())
    );
    assert_eq!(head(&game, capital), Some(archer()));
}

#[test]
fn the_gene_off_leaves_the_queue_alone() {
    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 2);
    game.spawn_unit("warrior", 1, at);
    let ai = AdvancedAi::new();
    assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);
    assert_eq!(head(&game, capital), Some(monument()));
}

#[test]
fn a_soldier_at_peace_or_a_barbarian_does_not_arm_it() {
    let (mut game, capital) = board();
    let at = ground(&game, capital, 2);
    game.spawn_unit("warrior", 1, at);
    let ai = armed();
    assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);
    assert_eq!(head(&game, capital), Some(monument()));

    let (mut game, capital) = board();
    let barbarian = game
        .players
        .iter()
        .find(|player| player.is_barbarian)
        .map(|player| player.id);
    if let Some(barbarian) = barbarian {
        let at = ground(&game, capital, 1);
        game.spawn_unit("warrior", barbarian, at);
        assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);
        assert_eq!(head(&game, capital), Some(monument()));
    }
}

#[test]
fn a_soldier_four_tiles_out_waits_but_an_engine_there_arms_it() {
    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 4);
    game.spawn_unit("warrior", 1, at);
    let ai = armed();
    assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);

    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 4);
    game.spawn_unit("catapult", 1, at);
    assert_eq!(
        ai.claim_threatened_capital_shooters(&mut game, 0),
        Some(archer())
    );
}

#[test]
fn three_shooters_at_home_turn_the_head_to_the_walls() {
    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 2);
    game.spawn_unit("warrior", 1, at);
    for gap in [0, 1, 2] {
        let at = if gap == 0 {
            game.cities[&capital].pos
        } else {
            ground(&game, capital, gap)
        };
        game.spawn_test_unit("archer", 0, at);
    }
    assert_eq!(AdvancedAi::capital_shooters_near(&game, 0, capital), 3);
    let ai = armed();
    assert_eq!(
        ai.claim_threatened_capital_shooters(&mut game, 0),
        Some(walls())
    );
    assert_eq!(head(&game, capital), Some(walls()));
}

#[test]
fn a_head_that_finishes_this_turn_or_already_answers_is_kept() {
    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 2);
    game.spawn_unit("warrior", 1, at);
    let cost = game.item_cost_for_city(0, capital, &monument());
    game.cities.get_mut(&capital).unwrap().production = cost - 0.5;
    let ai = armed();
    assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);
    assert_eq!(head(&game, capital), Some(monument()));

    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 2);
    game.spawn_unit("warrior", 1, at);
    game.cities.get_mut(&capital).unwrap().queue = vec![walls()];
    assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);
    assert_eq!(head(&game, capital), Some(walls()));
}

#[test]
fn the_claim_holds_three_turns_after_the_party_steps_back() {
    let (mut game, capital) = board();
    game.at_war.insert((0, 1));
    let at = ground(&game, capital, 2);
    let warrior = game.spawn_unit("warrior", 1, at);
    let mut ai = armed();
    ai.remember_threatened_capital(&game, 0);
    assert_eq!(ai.threatened_capital_seen, Some(40));
    let away = ground(&game, capital, 6);
    game.units.get_mut(&warrior).unwrap().pos = away;
    game.turn = 40 + THREAT_HOLD_TURNS;
    ai.remember_threatened_capital(&game, 0);
    assert_eq!(
        ai.claim_threatened_capital_shooters(&mut game, 0),
        Some(archer())
    );

    game.cities.get_mut(&capital).unwrap().queue = vec![monument()];
    game.turn = 41 + THREAT_HOLD_TURNS;
    assert_eq!(ai.claim_threatened_capital_shooters(&mut game, 0), None);
    assert_eq!(head(&game, capital), Some(monument()));
}
