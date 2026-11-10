use super::*;

/// A Religion seat that has founded, on a coast, with every starting unit
/// gone and no city-states; player 1 holds `rival_cities` cities the seat has
/// met but not seen.
fn founder(rival_cities: usize) -> (Game, Vec<Pos>) {
    let mut refusals = Vec::new();
    for seed in 0..60u64 {
        let mut game = Game::new_full(2, 40, 24, 911_300 + seed, 250, 0, false);
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
        let Some(home) = land.iter().copied().find(|pos| {
            game.nbrs(*pos).into_iter().any(|next| {
                game.map
                    .get(next)
                    .is_some_and(|tile| game.rules.is_water(tile))
            })
        }) else {
            refusals.push(format!("{seed}: no coastal site"));
            continue;
        };
        let mut sites: Vec<Pos> = Vec::new();
        for pos in land {
            if sites.len() == rival_cities {
                break;
            }
            if game.wdist(pos, home) >= 6 && sites.iter().all(|site| game.wdist(pos, *site) >= 4) {
                sites.push(pos);
            }
        }
        if sites.len() < rival_cities {
            refusals.push(format!("{seed}: room for {} rival cities", sites.len()));
            continue;
        }
        game.place_city(0, home, None);
        for site in &sites {
            game.place_city(1, *site, None);
        }
        game.players[0].religion = Some("Buddhism".to_string());
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        for site in &sites {
            game.players[0].explored.remove(site);
        }
        game.turn = 60;
        if !BasicAi::empire_is_coastal(&game, 0) {
            refusals.push(format!("{seed}: home is not coastal"));
            continue;
        }
        return (game, sites);
    }
    panic!("fixture: no seed gave a coastal founder and its rival: {refusals:?}");
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_religion_seat_charts_the_rivals();
    }
    ai
}

fn see(game: &mut Game, sites: &[Pos]) {
    for site in sites {
        game.players[0].explored.insert(*site);
    }
}

/// ★ The 012017Z shape: a met rival of twelve cities, two of them seen. The
/// seat needs eight (a strict majority of seven, and one to spare), counted
/// against the host's public figure when the board shows only what was seen.
#[test]
fn a_rival_is_under_charted_until_a_majority_and_one_are_known() {
    let (mut game, sites) = founder(5);
    assert_eq!(BasicAi::rival_cities_known(&game, 0, 1), 0);
    assert!(BasicAi::rival_is_under_charted(&game, 0, 1));
    // Five cities: a majority is three, so four are wanted.
    see(&mut game, &sites[..3]);
    assert_eq!(BasicAi::rival_cities_known(&game, 0, 1), 3);
    assert!(BasicAi::rival_is_under_charted(&game, 0, 1));
    see(&mut game, &sites[..4]);
    assert!(!BasicAi::rival_is_under_charted(&game, 0, 1));
    assert_eq!(BasicAi::under_charted_majors(&game, 0), 0);

    // The host says twelve: four known of the eight wanted.
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .city_count = Some(12);
    assert!(BasicAi::rival_is_under_charted(&game, 0, 1));
    assert_eq!(BasicAi::under_charted_majors(&game, 0), 1);

    // A rival of one city is charted by its one city.
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .city_count = Some(1);
    assert!(!BasicAi::rival_is_under_charted(&game, 0, 1));

    // An unmet rival is the ordinary gap's business, not this count's.
    game.players[0].met.remove(&1);
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .city_count = Some(12);
    assert_eq!(BasicAi::under_charted_majors(&game, 0), 0);
}

/// The gap stays open past the first city only for a Religion-target founder
/// with the gene on.
#[test]
fn the_contact_gap_stays_open_only_for_an_active_religion_founder() {
    let (mut game, sites) = founder(5);
    see(&mut game, &sites[..1]);
    assert!(
        !BasicAi::contact_fleet_gap(&game, 0),
        "met and located, no city-state: the shipped gap is closed"
    );
    let arm = |game: &Game, target: VictoryTarget, gene: bool| {
        let mut ai = seat(target, gene);
        ai.base.charts_rival_majorities = ai.religion_seat_charts_the_rivals_active(game, 0);
        ai.base.contact_gap_open(game, 0)
    };
    assert!(arm(&game, VictoryTarget::Religion, true));
    assert!(!arm(&game, VictoryTarget::Religion, false));
    assert!(!arm(&game, VictoryTarget::Culture, true));

    let mut unfounded = game.clone();
    unfounded.players[0].religion = None;
    assert!(!arm(&unfounded, VictoryTarget::Religion, true));

    see(&mut game, &sites[..4]);
    assert!(
        !arm(&game, VictoryTarget::Religion, true),
        "four of five known closes it"
    );
}

/// Each step toward Cartography is worth more while a rival is unmet or
/// under-charted, and nothing once Cartography is known or every rival is
/// charted.
#[test]
fn the_way_to_cartography_is_worth_more_while_a_rival_is_under_charted() {
    let (mut game, sites) = founder(5);
    see(&mut game, &sites[..1]);
    let on = seat(VictoryTarget::Religion, true);
    // Gathering Storm: Cartography needs Buttress, which needs Shipbuilding
    // and Mathematics, so the Currency line is on the way as well.
    for tech in [
        "sailing",
        "shipbuilding",
        "currency",
        "mathematics",
        "buttress",
        "cartography",
    ] {
        assert_eq!(
            on.religion_seat_cartography_value(&game, 0, tech),
            CARTOGRAPHY_VALUE,
            "{tech}"
        );
    }
    for tech in ["archery", "bronze_working", "celestial_navigation"] {
        assert_eq!(
            on.religion_seat_cartography_value(&game, 0, tech),
            0.0,
            "{tech}"
        );
    }
    assert_eq!(
        seat(VictoryTarget::Religion, false).religion_seat_cartography_value(
            &game,
            0,
            "cartography"
        ),
        0.0
    );
    assert_eq!(
        seat(VictoryTarget::Science, true).religion_seat_cartography_value(&game, 0, "cartography"),
        0.0
    );

    // An unmet rival counts as well as an under-charted one.
    let mut unmet = game.clone();
    see(&mut unmet, &sites);
    assert_eq!(
        on.religion_seat_cartography_value(&unmet, 0, "shipbuilding"),
        0.0
    );
    unmet.players[0].met.remove(&1);
    assert_eq!(
        on.religion_seat_cartography_value(&unmet, 0, "shipbuilding"),
        CARTOGRAPHY_VALUE
    );

    game.players[0].techs.insert(Name::new("cartography"));
    assert_eq!(
        on.religion_seat_cartography_value(&game, 0, "shipbuilding"),
        0.0
    );
}

/// A founder whose every known city already follows its faith, beside a
/// rival the host says holds five cities of which it has seen one. Everything
/// is explored except the ground within three tiles of that city. Returns
/// the game, the Missionary and the rival city's tile.
fn charted_out() -> (Game, u32, Pos) {
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
        let missionary = game.spawn_test_unit("missionary", 0, home);
        {
            let unit = game.units.get_mut(&missionary).unwrap();
            unit.religion = Some("Buddhism".to_string());
            unit.charges = 3;
            unit.hp = 100;
        }
        let Some(site) = land.iter().copied().find(|pos| {
            (10..=16).contains(&game.wdist(*pos, home))
                && game.route_step(missionary, *pos, 1).is_some()
        }) else {
            refusals.push(format!("{seed}: no rival site on a road from home"));
            continue;
        };
        let rival_city = game.place_city(1, site, None);
        for cid in [home_city, rival_city] {
            let city = game.cities.get_mut(&cid).unwrap();
            city.atheist_pressure = 0.0;
            city.pressure.clear();
            city.pressure.insert("Buddhism".to_string(), 100.0);
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

/// ★ The 054430Z shape: nothing the seat knows is left to convert and the
/// rival holds four cities it has never seen. Off, the spreader stands; on,
/// it walks toward the unexplored ground beside the one city it knows, and
/// keeps its charges.
#[test]
fn an_idle_spreader_walks_into_an_under_charted_rivals_ground() {
    let (game, missionary, site) = charted_out();
    assert!(BasicAi::rival_is_under_charted(&game, 0, 1));

    let mut off = game.clone();
    let start = off.units[&missionary].pos;
    assert!(!seat(VictoryTarget::Religion, false)
        .advanced_missionary_step(&mut off, 0, missionary, true));
    assert_eq!(off.units[&missionary].pos, start, "off: nothing to do");

    let on_seat = seat(VictoryTarget::Religion, true);
    let goal = on_seat
        .religion_seat_charting_goal(&game, 0, missionary, "Buddhism", true)
        .expect("on: unexplored ground beside the rival's city is a goal");
    assert!(!game.players[0].explored.contains(&goal));
    assert!(
        (1..=3).contains(&game.wdist(goal, site)),
        "the goal is beside the known city"
    );
    assert!(game
        .map
        .get(goal)
        .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile)));
    let mut on = game.clone();
    assert!(on_seat.advanced_missionary_step(&mut on, 0, missionary, true));
    let unit = &on.units[&missionary];
    assert_ne!(unit.pos, start, "on: it sets out");
    assert_eq!(unit.charges, 3, "it keeps its charges");

    // Not for a seat aiming elsewhere, and not on the defensive.
    let mut elsewhere = game.clone();
    assert!(
        !seat(VictoryTarget::Culture, true).advanced_missionary_step(
            &mut elsewhere,
            0,
            missionary,
            true
        )
    );
    let mut defensive = game.clone();
    assert!(
        !seat(VictoryTarget::Religion, true).advanced_missionary_step(
            &mut defensive,
            0,
            missionary,
            false
        )
    );
}

/// Once the rival is charted the spreader stands again: the walk is for
/// finding cities, not for wandering.
#[test]
fn a_charted_rival_draws_no_walk() {
    let (mut game, missionary, _) = charted_out();
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .city_count = Some(1);
    assert!(!BasicAi::rival_is_under_charted(&game, 0, 1));
    let start = game.units[&missionary].pos;
    assert!(!seat(VictoryTarget::Religion, true)
        .advanced_missionary_step(&mut game, 0, missionary, true));
    assert_eq!(game.units[&missionary].pos, start);
}

/// ★ The 073607Z shape: the corps has work, so nobody idles, and the rival
/// stays unseen. One spreader in four charts instead: the one with the
/// fewest charges left.
#[test]
fn the_most_spent_spreader_scouts_while_the_corps_works() {
    let (mut game, spent, site) = charted_out();
    // The known rival city is a target again, so no spreader is idle.
    {
        let cid = game.city_at(site).unwrap();
        let city = game.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.atheist_pressure = 150.0;
    }
    let home = game.units[&spent].pos;
    game.units.get_mut(&spent).unwrap().charges = 1;
    let fresh: Vec<u32> = (0..3)
        .map(|_| {
            let uid = game.spawn_test_unit("missionary", 0, home);
            let unit = game.units.get_mut(&uid).unwrap();
            unit.religion = Some("Buddhism".to_string());
            unit.charges = 3;
            unit.hp = 100;
            uid
        })
        .collect();
    let on = seat(VictoryTarget::Religion, true);

    // Four spreaders and one under-charted rival: one scout, the spent one.
    let mut board = game.clone();
    assert!(!on.religion_seat_scout_charts(&mut board, 0, fresh[0], "Buddhism", true));
    assert_eq!(board.units[&fresh[0]].pos, home);
    assert!(on.religion_seat_scout_charts(&mut board, 0, spent, "Buddhism", true));
    assert_ne!(board.units[&spent].pos, home, "the scout sets out");
    assert_eq!(board.units[&spent].charges, 1, "and keeps its charge");

    // Not with the gene off, on the defensive, or for a seat aiming elsewhere.
    let mut board = game.clone();
    assert!(!seat(VictoryTarget::Religion, false)
        .religion_seat_scout_charts(&mut board, 0, spent, "Buddhism", true));
    assert!(!on.religion_seat_scout_charts(&mut board, 0, spent, "Buddhism", false));
    assert!(!seat(VictoryTarget::Culture, true)
        .religion_seat_scout_charts(&mut board, 0, spent, "Buddhism", true));

    // A corps of three sends nobody.
    let mut small = game.clone();
    small.remove_unit(fresh[2]);
    assert!(!on.religion_seat_scout_charts(&mut small, 0, spent, "Buddhism", true));

    // A charted rival needs no scout.
    let mut charted = game.clone();
    std::sync::Arc::make_mut(&mut charted.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .city_count = Some(1);
    assert!(!on.religion_seat_scout_charts(&mut charted, 0, spent, "Buddhism", true));
}

/// ★ The 100022Z shape: a corps of eight with one cheap target in sight and
/// four of the rival's cities unseen. The rows price the known work at one
/// charge, so all but the spreaders that work needs chart; without rows the
/// corps sends its one scout in four.
#[test]
fn a_corps_far_past_its_known_work_sends_the_surplus_to_chart() {
    let (mut game, first, site) = charted_out();
    let rival_city = game.city_at(site).unwrap();
    {
        let city = game.cities.get_mut(&rival_city).unwrap();
        city.pressure.clear();
        city.atheist_pressure = 150.0;
    }
    let home = game.units[&first].pos;
    let mut corps = vec![first];
    for _ in 0..7 {
        let uid = game.spawn_test_unit("missionary", 0, home);
        let unit = game.units.get_mut(&uid).unwrap();
        unit.religion = Some("Buddhism".to_string());
        unit.charges = 3;
        unit.hp = 100;
        corps.push(uid);
    }
    corps.sort();
    let on = seat(VictoryTarget::Religion, true);

    // No rows: one scout for the one under-charted rival, the oldest unit.
    let mut bare = game.clone();
    assert!(on.religion_seat_scout_charts(&mut bare, 0, corps[0], "Buddhism", true));
    assert!(!on.religion_seat_scout_charts(&mut bare, 0, corps[1], "Buddhism", true));

    // Rows: the one known city asks one Apostle charge. 24 charges in the
    // corps less twice that leaves 22, which is seven spreaders.
    std::sync::Arc::make_mut(&mut game.observed_city_faiths).insert(
        rival_city,
        vec![crate::game::ObservedCityFaith {
            religion: None,
            followers: 1,
            pressure: 150.0,
            per_turn: 0.0,
        }],
    );
    assert_eq!(
        AdvancedAi::campaign_charges_known(&game, 0, "Buddhism"),
        Some(1)
    );
    let mut board = game.clone();
    assert!(on.religion_seat_scout_charts(&mut board, 0, corps[6], "Buddhism", true));
    assert_ne!(board.units[&corps[6]].pos, home, "the seventh sets out");
    assert!(
        !on.religion_seat_scout_charts(&mut board, 0, corps[7], "Buddhism", true),
        "the eighth stays for the city in sight"
    );

    // Work that asks as much as the corps holds leaves only the one scout.
    let mut busy = game.clone();
    std::sync::Arc::make_mut(&mut busy.observed_city_faiths).insert(
        rival_city,
        vec![
            crate::game::ObservedCityFaith {
                religion: None,
                followers: 0,
                pressure: 150.0,
                per_turn: 0.0,
            },
            crate::game::ObservedCityFaith {
                religion: Some("Hinduism".to_string()),
                followers: 1,
                pressure: 60_000.0,
                per_turn: 0.0,
            },
        ],
    );
    assert_eq!(
        AdvancedAi::campaign_charges_known(&busy, 0, "Buddhism"),
        Some(12)
    );
    assert!(on.religion_seat_scout_charts(&mut busy, 0, corps[0], "Buddhism", true));
    assert!(!on.religion_seat_scout_charts(&mut busy, 0, corps[1], "Buddhism", true));
}

/// ★ The 110341Z shape: a goal picked afresh each turn sent the corps in
/// circles. A charting unit keeps its tile from turn to turn, and takes a new
/// one only once the old one has been seen.
#[test]
fn a_charting_spreader_keeps_its_goal_until_it_is_seen() {
    let (game, missionary, _) = charted_out();
    let on = seat(VictoryTarget::Religion, true);
    let first = on
        .religion_seat_charting_goal(&game, 0, missionary, "Buddhism", true)
        .expect("unexplored ground beside the rival's city is a goal");

    // A turn's walk later the goal is the same tile.
    let mut walked = game.clone();
    assert!(on.advanced_missionary_step(&mut walked, 0, missionary, true));
    assert_ne!(walked.units[&missionary].pos, game.units[&missionary].pos);
    assert_eq!(
        on.religion_seat_charting_goal(&walked, 0, missionary, "Buddhism", true),
        Some(first)
    );

    // Seen, it is dropped for another tile still in the dark.
    walked.players[0].explored.insert(first);
    let second = on.religion_seat_charting_goal(&walked, 0, missionary, "Buddhism", true);
    assert_ne!(second, Some(first));
    if let Some(second) = second {
        assert!(!walked.players[0].explored.contains(&second));
    }
}
