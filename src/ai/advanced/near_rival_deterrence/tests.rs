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

/// `deterrence-spares-the-faith-defence`: off, a Shrine or an uninvested Holy
/// Site at the head of the only queue gives way to the deterrent (G222142Z's
/// 17-turn churn); on, the faith defence keeps its queue.
#[test]
fn the_faith_defence_keeps_its_queue_with_the_gene() {
    let Some((mut game, ours, _theirs)) = neighbours(DETERRENCE_NEAR_TILES) else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    game.players[0].techs.insert(crate::name!("bronze_working"));
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    let home = game.cities[&ours].pos;
    let site = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 1)
        .min()
        .expect("fixture: a neighbouring tile");
    for head in [
        Item::Building {
            building: crate::name!("shrine"),
        },
        Item::District {
            district: crate::name!("holy_site"),
            pos: site,
        },
    ] {
        let mut queued = game.clone();
        queued.cities.get_mut(&ours).unwrap().queue = vec![head.clone()];

        let off = armed();
        let mut displaced = queued.clone();
        let plan = off.assess(&displaced, 0);
        off.claim_deterrence_unit(&mut displaced, 0, &plan);
        assert_eq!(
            land_units_queued(&displaced),
            1,
            "off: the deterrent displaces {head:?}"
        );

        let mut on = armed();
        on.enable_deterrence_spares_the_faith_defence();
        let mut kept = queued.clone();
        let plan = on.assess(&kept, 0);
        on.claim_deterrence_unit(&mut kept, 0, &plan);
        assert_eq!(land_units_queued(&kept), 0, "on: {head:?} keeps its queue");
        assert_eq!(kept.cities[&ours].queue.first(), Some(&head));
    }
}

fn armed_2() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_near_rival_deterrence_2();
    ai
}

/// Land tiles at exactly `ring` tiles from `home`, in a fixed order.
fn ring(game: &Game, home: Pos, ring: i32) -> Vec<Pos> {
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| game.wdist(**pos, home) == ring && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles
}

/// civvis-20261102T185013Z in miniature: our 280 power is Warriors, their
/// 215 is Musketmen in sight. Plain power reads us ahead and v1 stands down;
/// version 2 weighs the quality and trains.
fn warriors_against_musketmen(gap: i32) -> (Game, u32) {
    let Some((mut game, ours, _theirs)) = neighbours(gap) else {
        panic!("fixture: two capitals within {gap} tiles");
    };
    game.players[0].techs.insert(crate::name!("bronze_working"));
    let home = game.cities[&ours].pos;
    for _ in 0..4 {
        game.spawn_unit("warrior", 0, home);
    }
    for pos in ring(&game, home, 2).into_iter().take(3) {
        game.spawn_unit("musketman", 1, pos);
    }
    power(&mut game, 0, 280.0);
    power(&mut game, 1, 215.0);
    (game, ours)
}

#[test]
fn version_two_weighs_unit_quality() {
    let (game, _ours) = warriors_against_musketmen(DETERRENCE_NEAR_TILES);

    let v1 = armed();
    let mut plain = game.clone();
    let plan = v1.assess(&plain, 0);
    v1.claim_deterrence_unit(&mut plain, 0, &plan);
    assert_eq!(
        land_units_queued(&plain),
        0,
        "v1: 280 against 215 reads level"
    );

    let v2 = armed_2();
    let (rival, theirs) = v2
        .deterrence_threat(&game, 0)
        .expect("v2 reads the neighbour");
    assert_eq!(rival, 1);
    let ours = v2.deterrence_our_power(&game, 0);
    assert!(
        ours < theirs,
        "quality power: our {ours:.1} against their {theirs:.1}"
    );
    let mut weighed = game.clone();
    let plan = v2.assess(&weighed, 0);
    v2.claim_deterrence_unit(&mut weighed, 0, &plan);
    assert_eq!(land_units_queued(&weighed), 1, "v2 trains the deterrent");
}

/// Past v1's window, version 2 keeps watching a near neighbour whose
/// forecast stands, and a declared friend's hazard falls under the floor.
#[test]
fn version_two_watches_past_the_window() {
    let (mut game, _ours) = warriors_against_musketmen(7);
    game.turn = game.standard_duration(DETERRENCE_END_STANDARD) + 10;

    assert!(
        armed().deterrence_threat(&game, 0).is_none(),
        "v1 stops at its window"
    );
    let v2 = armed_2();
    assert!(
        v2.deterrence_threat(&game, 0).is_some(),
        "v2: a near neighbour with soldiers at our door"
    );
    let mut on = game.clone();
    let plan = v2.assess(&on, 0);
    v2.claim_deterrence_unit(&mut on, 0, &plan);
    assert_eq!(land_units_queued(&on), 1);

    let mut late = game.clone();
    late.turn = late.standard_duration(DETERRENCE_WATCH_END_STANDARD);
    assert!(
        v2.deterrence_threat(&late, 0).is_none(),
        "v2's watch ends at live turn 150"
    );

    let mut friends = game.clone();
    friends.players[0]
        .observed_diplomatic_state
        .insert(1, "DECLARED_FRIEND".to_string());
    assert!(
        v2.deterrence_threat(&friends, 0).is_none(),
        "a declared friend's hazard is under the floor past the window"
    );
}

/// While out-gunned, version 2 upgrades an obsolete unit above the gold
/// reserve, and not below it; v1 never upgrades.
#[test]
fn version_two_upgrades_above_the_reserve() {
    let (mut game, ours) = warriors_against_musketmen(DETERRENCE_NEAR_TILES);
    let home = game.cities[&ours].pos;
    game.players[0].techs.insert(crate::name!("archery"));
    let slinger = game.spawn_unit("slinger", 0, home);
    power(&mut game, 1, 400.0);
    game.players[0].gold = 10_000.0;
    let unit = game.units.get_mut(&slinger).unwrap();
    unit.acted = false;
    unit.moves_left = unit.moves_left.max(1.0);
    let Some((_, price, _)) = game.unit_gold_upgrade_offer(0, slinger) else {
        panic!(
            "fixture: the Slinger can become an Archer: {:?}",
            game.unit_gold_upgrade_detail(0, slinger)
        );
    };
    let reserve = DETERRENCE_UPGRADE_RESERVE_BASE + DETERRENCE_UPGRADE_RESERVE_PER_CITY;

    let mut rich = game.clone();
    rich.players[0].gold = reserve + price + 1.0;
    rich.players[0].gold_per_turn = 10.0;
    let v1 = armed();
    let mut plain = rich.clone();
    let plan = v1.assess(&plain, 0);
    v1.claim_deterrence_unit(&mut plain, 0, &plan);
    assert_eq!(plain.units[&slinger].kind, "slinger", "v1 never upgrades");

    let v2 = armed_2();
    let plan = v2.assess(&rich, 0);
    v2.claim_deterrence_unit(&mut rich, 0, &plan);
    assert!(
        rich.units
            .get(&slinger)
            .is_none_or(|unit| unit.kind != "slinger"),
        "v2 upgrades the Slinger"
    );

    let mut poor = game.clone();
    poor.players[0].gold = reserve + price - 1.0;
    poor.players[0].gold_per_turn = 10.0;
    let plan = v2.assess(&poor, 0);
    v2.claim_deterrence_unit(&mut poor, 0, &plan);
    assert_eq!(poor.units[&slinger].kind, "slinger", "the reserve holds");
}
