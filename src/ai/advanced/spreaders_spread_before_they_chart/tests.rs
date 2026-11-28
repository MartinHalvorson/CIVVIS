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

/// A Religion seat with `religion-seat-charts-the-rivals`,
/// `religion-seat-seeks-the-unseen-rival` and `last-charge-joins-the-army`
/// on, and version two of this gene as given (version one otherwise).
fn budget_seat(v2: bool) -> AdvancedAi {
    let mut ai = seat(!v2, true);
    ai.enable_last_charge_joins_the_army();
    if v2 {
        ai.enable_spreaders_spread_before_they_chart_2();
    }
    ai
}

/// ★ The 064630Z shape (t90): three Missionaries on their last charge
/// already walk the fog, and version one still names a seeker, the corps'
/// unit farthest from home whatever its charges. Version two leaves the
/// search to the scouts already out, and with none out the seeker is the
/// most spent spreader, so the full ones keep the known work.
#[test]
fn the_last_charge_scouts_spend_the_search_budget() {
    let (mut game, first, _) = owed_city();
    let home = game.units[&first].pos;
    let mut corps = vec![first];
    for _ in 0..3 {
        corps.push(spreader(&mut game, home, 3));
    }
    corps.sort();
    // The most spent spreader is not the lowest id (version one's pick with
    // every unit at home).
    let spent = corps[2];
    game.units.get_mut(&spent).unwrap().charges = 2;
    let scouts: Vec<u32> = (0..3).map(|_| spreader(&mut game, home, 1)).collect();
    assert!(BasicAi::rival_is_under_charted(&game, 0, 1));

    // Version one: one seeker beside the scouts, the lowest id.
    let v1 = budget_seat(false);
    for scout in &scouts {
        assert!(v1.keeps_its_last_charge(&game, 0, *scout));
        v1.missionary_explore
            .borrow_mut()
            .entry(*scout)
            .or_default()
            .goal = Some(home);
    }
    assert_eq!(
        v1.unseen_rival_seekers(&game, 0, "Buddhism", true),
        vec![corps[0]]
    );
    assert!(v1.seekers_left_by_the_scouts(&game, 0, 4).is_none());

    // Version two, the scouts out: the budget (one in four of seven) is
    // spent, and every spreader works.
    let v2 = budget_seat(true);
    for scout in &scouts {
        v2.missionary_explore
            .borrow_mut()
            .entry(*scout)
            .or_default()
            .goal = Some(home);
    }
    assert_eq!(v2.last_charge_scouts_out(&game, 0), 3);
    assert_eq!(v2.seekers_left_by_the_scouts(&game, 0, 4), Some(0));
    assert!(v2
        .unseen_rival_seekers(&game, 0, "Buddhism", true)
        .is_empty());
    let mut played = game.clone();
    assert!(!v2.religion_seat_seeker_step(&mut played, 0, corps[0], "Buddhism", true));

    // Version two, no scout walking toward a goal (resting at home): one
    // seeker, the most spent spreader.
    let resting = budget_seat(true);
    assert_eq!(resting.last_charge_scouts_out(&game, 0), 0);
    assert_eq!(
        resting.unseen_rival_seekers(&game, 0, "Buddhism", true),
        vec![spent]
    );

    // Version two carries version one: the held spreader still waits.
    let (game, missionary, site) = owed_city();
    assert!(budget_seat(true).held_spreader_waits_for_its_city(
        &game,
        missionary,
        "Buddhism",
        &[site]
    ));
}

/// ★ Each searcher counted once by the scouts: version two picks the scouts
/// from the spreaders that are not seeking, so this turn's seeker, now the
/// most spent spreader and so at the head of the scouts' list, does not
/// spend a scout's slot a second time; and the surplus clause is less the
/// seekers out.
#[test]
fn the_scouts_count_each_seeker_once() {
    let (mut game, first, _) = owed_city();
    let home = game.units[&first].pos;
    let mut corps = vec![first];
    for _ in 0..7 {
        corps.push(spreader(&mut game, home, 3));
    }
    corps.sort();
    game.units.get_mut(&corps[3]).unwrap().charges = 1;
    game.units.get_mut(&corps[5]).unwrap().charges = 2;
    let v2 = budget_seat(true);
    // The seeker is the most spent spreader that is not on its last charge
    // (the one-charge unit is `last-charge-joins-the-army`'s).
    assert!(v2.keeps_its_last_charge(&game, 0, corps[3]));
    let seekers = v2.unseen_rival_seekers(&game, 0, "Buddhism", true);
    assert_eq!(seekers, vec![corps[5]]);

    let mut sorted: Vec<(i32, u32)> = corps
        .iter()
        .map(|id| (game.units[id].charges, *id))
        .collect();
    sorted.sort();
    // The share (one rival short, two in eight) less the seeker: none.
    assert_eq!(
        v2.scouts_after_the_searchers(&game, 0, "Buddhism", true, &sorted, 1, 0),
        Some(vec![])
    );
    // A surplus of three less the seeker: two, after the last-charge unit's
    // slot at the head, and never the seeker.
    assert_eq!(
        v2.scouts_after_the_searchers(&game, 0, "Buddhism", true, &sorted, 1, 3),
        Some(vec![corps[3], corps[0]])
    );
    // Off: no version-two quota.
    assert!(budget_seat(false)
        .scouts_after_the_searchers(&game, 0, "Buddhism", true, &sorted, 1, 3)
        .is_none());
}
