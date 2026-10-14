use super::*;

/// Two majors who have met: our founded capital (population 4, idle), the
/// rival's city 9-12 tiles off, every unit removed, everything within nine
/// tiles of the capital in sight, and the rival at 2.5 times our power.
/// Returns the game and our capital.
fn watched() -> Option<(Game, u32)> {
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
    game.place_city(1, *sites.first()?, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = 30;
    game.host_observed = Arc::new(game.wdisk(home, 9).into_iter().collect());
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 250.0);
    Some((game, capital))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
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
    ai.enable_capital_walls_before_the_surprise();
    ai
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

/// The research leg: a rival soldier seen within eight tiles at 2.5 times
/// our power puts Masonry next; off, at 1.5 times, past the window, or with
/// Masonry known, it does not.
#[test]
fn a_stronger_neighbour_seen_near_the_capital_puts_masonry_next() {
    let Some((mut game, capital)) = watched() else {
        panic!("fixture: two majors with a capital each");
    };
    let at = ground(&game, capital, CAPITAL_WATCH_TILES);
    game.spawn_unit("warrior", 1, at);
    let masonry = Some(crate::name!("masonry"));

    let mut stock = AdvancedAi::new();
    stock.observe_turn_start_hostiles(&game, 0);
    assert!(
        stock.capital_watch_sightings.is_empty(),
        "off: nothing remembered"
    );
    let plan = stock.assess(&game, 0);
    assert_ne!(
        stock.defensive_walls_research_goal(&game, 0, &plan),
        masonry
    );

    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(ai.capital_watch_sightings.get(&1), Some(&game.turn));
    let plan = ai.assess(&game, 0);
    assert_eq!(ai.defensive_walls_research_goal(&game, 0, &plan), masonry);

    let mut level = game.clone();
    power(&mut level, 1, 150.0);
    assert_ne!(
        ai.defensive_walls_research_goal(&level, 0, &plan),
        masonry,
        "1.5x: no"
    );

    let mut later = game.clone();
    later.turn += later.standard_duration(CAPITAL_WATCH_STANDARD) + 1;
    assert_ne!(
        ai.defensive_walls_research_goal(&later, 0, &plan),
        masonry,
        "stale sighting"
    );

    let mut known = game.clone();
    known.players[0].techs.insert(crate::name!("masonry"));
    assert_ne!(ai.defensive_walls_research_goal(&known, 0, &plan), masonry);

    let mut far = game.clone();
    let unit = far.units.values().find(|unit| unit.owner == 1).unwrap().id;
    far.remove_unit(unit);
    let beyond = ground(&far, capital, CAPITAL_WATCH_TILES + 1);
    far.spawn_unit("warrior", 1, beyond);
    let mut fresh = armed();
    fresh.observe_turn_start_hostiles(&far, 0);
    assert!(
        fresh.capital_watch_sightings.is_empty(),
        "nine tiles off: not at our door"
    );
}

/// The walls leg: with Masonry the capital's queue goes to its Walls and
/// they hold over the Builder; a started wall finishes after the sighting goes stale; a
/// Settler two turns from done finishes first; off, nothing moves.
#[test]
fn the_capital_takes_its_walls_and_finishes_them() {
    let Some((mut game, capital)) = watched() else {
        panic!("fixture: two majors with a capital each");
    };
    game.players[0].techs.insert(crate::name!("masonry"));
    assert!(
        game.can_produce(0, capital, &walls()),
        "fixture: walls are buildable"
    );
    let at = ground(&game, capital, 5);
    game.spawn_unit("warrior", 1, at);
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: builder.clone(),
        },
    )
    .unwrap();

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    stock.claim_capital_walls_before_the_surprise(&mut off, 0);
    assert_eq!(
        off.cities[&capital].queue.first(),
        Some(&builder),
        "off: no claim"
    );

    let mut ai = armed();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.observe_turn_start_hostiles(&game, 0);
    let mut on = game.clone();
    ai.claim_capital_walls_before_the_surprise(&mut on, 0);
    assert_eq!(on.cities[&capital].queue.first(), Some(&walls()));
    assert!(ai.capital_walls_hold(&on, 0, capital, &walls()));
    let thoughts = ai.journal().since(0).thoughts;
    assert!(
        thoughts
            .iter()
            .any(|thought| thought.detail.contains("capital-walls-before-the-surprise")),
        "the fire is journalled"
    );

    let mut stale = on.clone();
    stale.turn += stale.standard_duration(CAPITAL_WATCH_STANDARD) + 1;
    assert!(
        !ai.capital_walls_hold(&stale, 0, capital, &walls()),
        "an unstarted wall lapses with the sighting"
    );
    stale.cities.get_mut(&capital).unwrap().production = 5.0;
    assert!(
        ai.capital_walls_hold(&stale, 0, capital, &walls()),
        "a started wall finishes"
    );

    let mut settling = game.clone();
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    settling
        .apply(
            0,
            &Action::Produce {
                city: capital,
                item: settler.clone(),
            },
        )
        .unwrap();
    let production = settling.city_yields(capital).production.max(0.1);
    let cost = settling.item_cost_for_city(0, capital, &settler);
    settling.cities.get_mut(&capital).unwrap().production = cost - 1.5 * production;
    ai.claim_capital_walls_before_the_surprise(&mut settling, 0);
    assert_eq!(
        settling.cities[&capital].queue.first(),
        Some(&settler),
        "a Settler within two turns finishes first"
    );
    settling.cities.get_mut(&capital).unwrap().production = 0.0;
    ai.claim_capital_walls_before_the_surprise(&mut settling, 0);
    assert_eq!(settling.cities[&capital].queue.first(), Some(&walls()));
}

/// The defender leg: two rival soldiers within six tiles and no garrison
/// buy one land defender above the reserve; with no Gold, and no Masonry,
/// the capital trains one instead; with two of ours beside it, nothing.
#[test]
fn the_door_signal_buys_or_trains_a_defender() {
    let Some((mut game, capital)) = watched() else {
        panic!("fixture: two majors with a capital each");
    };
    for _ in 0..CAPITAL_DOOR_UNITS {
        let at = ground(&game, capital, 4);
        game.spawn_unit("warrior", 1, at);
    }
    let ours = |game: &Game| game.player_unit_ids(0).len();

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    off.players[0].gold = 500.0;
    assert!(
        !stock.capital_garrison_purchase(&mut off, 0),
        "off: no purchase"
    );

    let mut ai = armed();
    ai.base.journal = crate::reasoning::Journal::recording();
    let mut rich = game.clone();
    rich.players[0].gold = 500.0;
    assert!(
        ai.capital_garrison_purchase(&mut rich, 0),
        "a defender is bought"
    );
    assert_eq!(ours(&rich), 1);
    assert!(rich.players[0].gold < 500.0);
    let bought = rich.units.values().find(|unit| unit.owner == 0).unwrap();
    assert_ne!(
        rich.rules.units[bought.kind].promotion_class, "recon",
        "a soldier, never a Scout"
    );

    let mut poor = game.clone();
    poor.players[0].gold = 0.0;
    assert!(
        !ai.capital_garrison_purchase(&mut poor, 0),
        "no Gold, no purchase"
    );
    ai.claim_capital_walls_before_the_surprise(&mut poor, 0);
    assert!(
        matches!(
            poor.cities[&capital].queue.first(),
            Some(Item::Unit { unit }) if poor.rules.units[unit].class == "military"
        ),
        "without Masonry the capital trains the defender"
    );

    let mut held = game.clone();
    held.players[0].gold = 500.0;
    let home = held.cities[&capital].pos;
    held.spawn_unit("warrior", 0, home);
    let beside = ground(&held, capital, 1);
    held.spawn_unit("warrior", 0, beside);
    assert!(
        !ai.capital_garrison_purchase(&mut held, 0),
        "two of ours: nothing"
    );

    let mut topped = game.clone();
    topped.players[0].gold = 500.0;
    let beside = ground(&topped, capital, 1);
    topped.spawn_unit("slinger", 0, beside);
    assert!(
        ai.capital_garrison_purchase(&mut topped, 0),
        "one of ours: a second is bought"
    );
    ai.claim_capital_walls_before_the_surprise(&mut topped, 0);
    assert!(
        topped.cities[&capital].queue.is_empty(),
        "the bought defender fills the garrison; none is trained the same turn: {:?}",
        topped.cities[&capital].queue.first()
    );

    let mut lone = game.clone();
    lone.players[0].gold = 500.0;
    let first = lone.units.values().find(|unit| unit.owner == 1).unwrap().id;
    lone.remove_unit(first);
    assert!(
        !ai.capital_garrison_purchase(&mut lone, 0),
        "one soldier: no door signal"
    );
}
