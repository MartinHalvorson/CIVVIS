use super::*;

/// Two majors who have met, each with one founded city, our capital idle at
/// population 4, every unit removed, at turn 40 (inside the window at the
/// test board's speed is checked by the caller). Returns the game, our
/// capital and the rival's city.
fn neighbours(max_gap: i32) -> Option<(Game, u32, u32)> {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let home = game.cities[&ours].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=max_gap).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    let site = *sites.first()?;
    let theirs = game.place_city(1, site, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&ours).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
    Some((game, ours, theirs))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_near_rival_deterrence();
    ai
}

fn land_units_queued(game: &Game) -> usize {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::Unit { unit }) if AdvancedAi::deterrence_land_unit(game, unit)
            )
        })
        .count()
}

/// The blocked case: a neighbour at peace with twice our power gets a land
/// unit queued against it; off, nothing moves; level, nothing moves.
#[test]
fn an_out_gunned_empire_trains_a_deterrent() {
    let Some((mut game, _ours, _theirs)) = neighbours(DETERRENCE_NEAR_TILES) else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    game.players[0].techs.insert(crate::name!("bronze_working"));
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_deterrence_unit(&mut off, 0, &plan);
    assert_eq!(land_units_queued(&off), 0, "off: no claim");

    let ai = armed();
    assert!(
        ai.deterrence_threat(&game, 0).is_some(),
        "fixture: the neighbour reads"
    );
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_deterrence_unit(&mut on, 0, &plan);
    assert_eq!(
        land_units_queued(&on),
        1,
        "one land unit against the neighbour"
    );
    let cid = on.player_city_ids(0)[0];
    let head = on.cities[&cid].queue.first().cloned().unwrap();
    assert!(
        ai.deterrence_unit_holds(&on, 0, &head),
        "the deterrent is held while out-gunned"
    );

    let mut level = game.clone();
    power(&mut level, 0, 200.0);
    let plan = ai.assess(&level, 0);
    ai.claim_deterrence_unit(&mut level, 0, &plan);
    assert_eq!(land_units_queued(&level), 0, "level power: no claim");
}

/// The bounds: at war, unmet, outside the window, or with the city on its
/// Walls, nothing is claimed.
#[test]
fn the_claim_yields_to_war_the_window_and_walls() {
    let ai = armed();
    let Some((mut game, ours, _theirs)) = neighbours(DETERRENCE_NEAR_TILES) else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);

    let mut late = game.clone();
    late.turn = late.standard_duration(DETERRENCE_END_STANDARD);
    assert!(ai.deterrence_threat(&late, 0).is_none(), "past the window");

    let mut early = game.clone();
    early.turn = early.standard_duration(DETERRENCE_START_STANDARD) - 1;
    assert!(
        ai.deterrence_threat(&early, 0).is_none(),
        "before the window"
    );

    let mut unmet = game.clone();
    unmet.players[0].met.remove(&1);
    assert!(
        ai.deterrence_threat(&unmet, 0).is_none(),
        "an unmet rival is not read"
    );

    let mut war = game.clone();
    war.at_war.insert((0, 1));
    assert!(
        ai.deterrence_threat(&war, 0).is_none(),
        "a rival at war is the war's"
    );

    let mut walled = game.clone();
    walled.cities.get_mut(&ours).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    let plan = ai.assess(&walled, 0);
    ai.claim_deterrence_unit(&mut walled, 0, &plan);
    assert_eq!(land_units_queued(&walled), 0, "Walls keep their queue");
}

/// The early city floor's Settler keeps its city.
#[test]
fn the_settler_floor_keeps_its_city() {
    let Some((mut game, ours, _theirs)) = neighbours(DETERRENCE_NEAR_TILES) else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    let mut ai = armed();
    ai.enable_early_settler_floor();
    game.cities.get_mut(&ours).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    let plan = ai.assess(&game, 0);
    assert!(
        ai.early_settler_floor_holds(&game, 0, ours, &plan),
        "fixture: the floor holds the Settler"
    );
    ai.claim_deterrence_unit(&mut game, 0, &plan);
    assert!(
        matches!(game.cities[&ours].queue.first(), Some(Item::Unit { unit }) if *unit == "settler"),
        "the Settler stays at the head"
    );
}
