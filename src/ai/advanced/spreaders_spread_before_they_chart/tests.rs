use super::*;

/// A founded Religion seat with a Missionary at home and one met rival city
/// on a road 10-16 tiles away, the rival's other four cities unseen (the
/// host's public count is five) and fog left beside the known one. The known
/// city is a target: nothing follows our faith there yet.
fn owed_city() -> (Game, u32, Pos) {
    let mut refusals = Vec::new();
    for seed in 0..80u64 {
        let mut game = Game::new_full(2, 40, 24, 912_700 + seed, 250, 0, false);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        game.current = 0;
        let mut land: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
            })
            .map(|(pos, _)| *pos)
            .collect();
        land.sort();
        let Some(home) = land.first().copied() else {
            continue;
        };
        let home_city = game.place_city(0, home, None);
        let missionary = spreader(&mut game, home, 3);
        let Some(site) = land.iter().copied().find(|pos| {
            (10..=16).contains(&game.wdist(*pos, home))
                && game.route_step(missionary, *pos, 1).is_some()
        }) else {
            refusals.push(format!("{seed}: no rival site on a road from home"));
            continue;
        };
        let rival_city = game.place_city(1, site, None);
        {
            let city = game.cities.get_mut(&home_city).unwrap();
            city.atheist_pressure = 0.0;
            city.pressure.clear();
            city.pressure.insert("Buddhism".to_string(), 100.0);
        }
        {
            let city = game.cities.get_mut(&rival_city).unwrap();
            city.pressure.clear();
            city.atheist_pressure = 150.0;
        }
        game.players[0].religion = Some("Buddhism".to_string());
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default()
            .city_count = Some(5);
        let every: Vec<Pos> = game.map.tiles.keys().copied().collect();
        game.players[0].explored.extend(every);
        let fog: Vec<Pos> = game
            .wdisk(site, 3)
            .into_iter()
            .filter(|pos| *pos != site)
            .collect();
        let dry_fog = fog
            .iter()
            .filter(|pos| {
                game.map
                    .get(**pos)
                    .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            })
            .count();
        if dry_fog < 3 {
            refusals.push(format!("{seed}: {dry_fog} dry tiles beside the rival city"));
            continue;
        }
        for pos in fog {
            game.players[0].explored.remove(&pos);
        }
        game.turn = 90;
        return (game, missionary, site);
    }
    panic!("fixture: no seed gave a founder a road to a rival city: {refusals:?}");
}

/// A healthy Missionary of our faith with `charges` charges.
fn spreader(game: &mut Game, pos: Pos, charges: i32) -> u32 {
    let uid = game.spawn_test_unit("missionary", 0, pos);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.religion = Some("Buddhism".to_string());
    unit.charges = charges;
    unit.hp = 100;
    uid
}

/// A Religion seat with `religion-seat-charts-the-rivals` on, and this gene
/// and `religion-seat-seeks-the-unseen-rival` as given.
fn seat(gene: bool, seekers: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.enable_religion_seat_charts_the_rivals();
    if seekers {
        ai.enable_religion_seat_seeks_the_unseen_rival();
    }
    if gene {
        ai.enable_spreaders_spread_before_they_chart();
    }
    ai
}

/// ★ The 081944Z shape: a spreader's march toward the city it owes was held
/// (a rival Apostle's reach), and the fall-through charted it away. Off, the
/// charting step walks it into the fog; on, it waits for the city still on
/// its road, and charts only when it has no target or none it can reach.
#[test]
fn a_held_spreader_waits_for_the_city_on_its_road() {
    let (game, missionary, site) = owed_city();
    assert!(BasicAi::rival_is_under_charted(&game, 0, 1));
    let start = game.units[&missionary].pos;

    // Off: what the fall-through does today, a walk into the fog.
    let off = seat(false, false);
    assert!(!off.held_spreader_waits_for_its_city(&game, missionary, "Buddhism", &[site]));
    let mut walked = game.clone();
    assert!(off.religion_seat_spreader_charts(&mut walked, 0, missionary, "Buddhism", true));
    assert_ne!(walked.units[&missionary].pos, start, "off: it charts away");

    // On: the owed city is on its road, so it waits for it.
    let on = seat(true, false);
    assert!(on.held_spreader_waits_for_its_city(&game, missionary, "Buddhism", &[site]));

    // No target, or another faith's spreader: unchanged.
    assert!(!on.held_spreader_waits_for_its_city(&game, missionary, "Buddhism", &[]));
    assert!(!on.held_spreader_waits_for_its_city(&game, missionary, "Hinduism", &[site]));

    // A target it has no route to does not hold it.
    let mut island = game.clone();
    let sea = island
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| island.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .find(|pos| island.route_step(missionary, *pos, 1).is_none());
    if let Some(sea) = sea {
        assert!(!on.held_spreader_waits_for_its_city(&island, missionary, "Buddhism", &[sea]));
    }

    // Before the founding the charting gene does not act.
    island.players[0].religion = None;
    assert!(!on.held_spreader_waits_for_its_city(&island, missionary, "Buddhism", &[site]));
}

/// ★ Two quotas for one gap: with the seekers armed, the located rival short
/// of a charted majority already has its seeker (the spreader farthest from
/// home, here the lowest id with all at home), so the scouts' quota for it
/// is spent and the spreader with the fewest charges stays with the corps'
/// work.
#[test]
fn the_scouts_leave_a_sought_rival_to_its_seeker() {
    let (mut game, first, _) = owed_city();
    let home = game.units[&first].pos;
    let mut corps = vec![first];
    for _ in 0..7 {
        corps.push(spreader(&mut game, home, 3));
    }
    corps.sort();
    // The most spent spreader, the scouts' pick, is not the seeker.
    game.units.get_mut(&corps[1]).unwrap().charges = 1;

    let seekers = seat(true, true).unseen_rival_seekers(&game, 0, "Buddhism", true);
    assert_eq!(
        seekers,
        vec![corps[0]],
        "one seeker for the one located rival"
    );

    // Off: the scouts take their own for the same rival.
    let mut off = game.clone();
    assert!(seat(false, true).religion_seat_scout_charts(&mut off, 0, corps[1], "Buddhism", true));
    assert_ne!(off.units[&corps[1]].pos, home, "off: a second unit charts");

    // On: the seeker covers it, and the spent spreader stays.
    let mut on = game.clone();
    assert!(!seat(true, true).religion_seat_scout_charts(&mut on, 0, corps[1], "Buddhism", true));
    assert_eq!(on.units[&corps[1]].pos, home);
    assert_eq!(
        seat(true, true).scouts_left_to_the_seekers(&game, 0, "Buddhism", true),
        1
    );

    // Without the seekers' gene the scouts keep their quota.
    let mut alone = game.clone();
    assert!(seat(true, false).religion_seat_scout_charts(&mut alone, 0, corps[1], "Buddhism", true));
    assert_eq!(
        seat(true, false).scouts_left_to_the_seekers(&game, 0, "Buddhism", true),
        0
    );
}
