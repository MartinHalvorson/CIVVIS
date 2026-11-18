use super::*;

/// The fog left around the unseen rival's city.
const FOG: i32 = 4;

struct Board {
    game: Game,
    /// The spreader standing beside the located rival's city, farthest from
    /// our capital.
    far: u32,
    /// Three spreaders at home.
    home: Vec<u32>,
    /// The located rival's city.
    located: Pos,
    /// The city of the rival we have met and never seen.
    unseen: Pos,
}

/// A full Missionary of our faith at `pos`.
fn spreader(game: &mut Game, pos: Pos) -> u32 {
    let uid = game.spawn_test_unit("missionary", 0, pos);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.religion = Some("Buddhism".to_string());
    unit.charges = 3;
    unit.hp = 100;
    uid
}

/// The 082214Z shape, small: a founded Religion seat that has met two rivals.
/// Player 1's one city is on the map and charted; player 2's city stands in
/// the only fog left, [`FOG`] tiles deep. Four Missionaries: three at home,
/// one beside player 1's city.
fn board() -> Board {
    let mut refusals = Vec::new();
    for seed in 0..120u64 {
        let mut game = Game::new_full(3, 44, 26, 918_200 + seed, 250, 0, false);
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
        let Some(capital) = land.first().copied() else {
            continue;
        };
        let home_city = game.place_city(0, capital, None);
        {
            // Our own city follows our faith: no charge is owed at home.
            let city = game.cities.get_mut(&home_city).unwrap();
            city.atheist_pressure = 0.0;
            city.pressure.clear();
            city.pressure.insert("Buddhism".to_string(), 100.0);
        }
        let home: Vec<u32> = (0..3).map(|_| spreader(&mut game, capital)).collect();
        let Some(located) = land.iter().copied().find(|pos| {
            (8..=12).contains(&game.wdist(*pos, capital))
                && game.route_step(home[0], *pos, 1).is_some()
        }) else {
            refusals.push(format!("{seed}: no located site on a road from home"));
            continue;
        };
        let Some(unseen) = land.iter().copied().find(|pos| {
            (9..=14).contains(&game.wdist(*pos, located))
                && game.wdist(*pos, capital) > game.wdist(located, capital) + FOG
                && game.route_step(home[0], *pos, 1).is_some()
        }) else {
            refusals.push(format!("{seed}: no unseen site beyond the located one"));
            continue;
        };
        let Some(post) = game.nbrs(located).into_iter().find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.wdist(*pos, capital) >= game.wdist(located, capital)
        }) else {
            refusals.push(format!("{seed}: no post beside the located city"));
            continue;
        };
        game.place_city(1, located, None);
        game.place_city(2, unseen, None);
        let far = spreader(&mut game, post);
        game.players[0].religion = Some("Buddhism".to_string());
        for rival in [1, 2] {
            game.players[0].met.insert(rival);
            game.players[rival].met.insert(0);
        }
        // Player 1 is charted by its one city; player 2 has three somewhere.
        for (rival, count) in [(1, 1), (2, 3)] {
            std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
                .entry(rival)
                .or_default()
                .city_count = Some(count);
        }
        let every: Vec<Pos> = game.map.tiles.keys().copied().collect();
        game.players[0].explored.extend(every);
        for pos in game.wdisk(unseen, FOG) {
            game.players[0].explored.remove(&pos);
        }
        game.turn = 120;
        if game.route_step(far, unseen, 1).is_none() {
            refusals.push(format!("{seed}: no road from the post to the unseen city"));
            continue;
        }
        return Board {
            game,
            far,
            home,
            located,
            unseen,
        };
    }
    panic!("fixture: no seed gave a founder a located and an unseen rival: {refusals:?}");
}

fn seat(target: VictoryTarget, charts: bool, seeks: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if charts {
        ai.enable_religion_seat_charts_the_rivals();
    }
    if seeks {
        ai.enable_religion_seat_seeks_the_unseen_rival();
    }
    ai
}

/// ★ The 082214Z shape: Portugal met at t119, its first city seen at t159,
/// our corps on its continent from t128. One spreader looks for the rival
/// with no city on the map, the one deepest in foreign ground; it walks into
/// the fog, keeps its charges, and the rest of the corps has its own turn.
#[test]
fn one_spreader_looks_for_a_rival_with_no_city_on_the_map() {
    let Board {
        game,
        far,
        home,
        located,
        unseen,
    } = board();
    assert_eq!(AdvancedAi::unseen_met_rivals(&game, 0), 1);
    assert!(
        !BasicAi::rival_is_under_charted(&game, 0, 1),
        "the located rival is charted: the charting scouts have nothing to do"
    );
    let on = seat(VictoryTarget::Religion, true, true);
    assert_eq!(
        on.unseen_rival_seekers(&game, 0, "Buddhism", true),
        vec![far],
        "one seeker, the spreader farthest from our capital"
    );

    let mut played = game.clone();
    let start = played.units[&far].pos;
    assert!(on.religion_seat_seeker_step(&mut played, 0, far, "Buddhism", true));
    let unit = &played.units[&far];
    assert_ne!(unit.pos, start, "the seeker sets out");
    assert_eq!(unit.charges, 3, "and keeps its charges");
    // Its goal is in the only fog on the board, where the unseen rival stands.
    let (goal, lead) = on.rival_seekers.borrow()[&far];
    assert!(!game.players[0].explored.contains(&goal));
    assert!(!lead, "a native board has no unattributed borders");
    assert!(game.wdist(goal, unseen) <= FOG);

    // The rest of the corps is not sent to look.
    for other in &home {
        let mut board = game.clone();
        let stood = board.units[other].pos;
        assert!(!on.religion_seat_seeker_step(&mut board, 0, *other, "Buddhism", true));
        assert_eq!(board.units[other].pos, stood);
    }

    // The whole turn: the seeker's step comes before the spread, so it walks
    // on past the located rival's city instead of standing beside it.
    let mut turn = game.clone();
    assert!(on.advanced_missionary_step(&mut turn, 0, far, true));
    assert_eq!(turn.units[&far].charges, 3);
    assert_eq!(turn.units[&far].pos, played.units[&far].pos);
    assert_eq!(game.wdist(start, located), 1, "past a city it stood beside");

    // Off, without `religion-seat-charts-the-rivals`, for a seat aiming
    // elsewhere, on the defensive and for another faith: no seeker.
    for (ai, offensive, religion) in [
        (seat(VictoryTarget::Religion, true, false), true, "Buddhism"),
        (seat(VictoryTarget::Religion, false, true), true, "Buddhism"),
        (seat(VictoryTarget::Culture, true, true), true, "Buddhism"),
        (seat(VictoryTarget::Religion, true, true), false, "Buddhism"),
        (seat(VictoryTarget::Religion, true, true), true, "Judaism"),
    ] {
        let mut board = game.clone();
        assert!(ai
            .unseen_rival_seekers(&board, 0, religion, offensive)
            .is_empty());
        assert!(!ai.religion_seat_seeker_step(&mut board, 0, far, religion, offensive));
        assert_eq!(board.units[&far].pos, start);
    }
    let mut unfounded = game.clone();
    unfounded.players[0].religion = None;
    assert!(on
        .unseen_rival_seekers(&unfounded, 0, "Buddhism", true)
        .is_empty());
}

/// A seeker stays the seeker when another spreader comes to stand farther
/// out, it keeps its goal while that is still fog, and the search ends the
/// turn the rival's first city is on the map.
#[test]
fn a_seeker_stays_one_until_its_rival_is_found() {
    let Board {
        mut game,
        far,
        unseen,
        ..
    } = board();
    let on = seat(VictoryTarget::Religion, true, true);
    // It has no move left this turn: the goal is taken and nothing changes
    // on the board.
    let start = game.units[&far].pos;
    game.units.get_mut(&far).unwrap().moves_left = 0.0;
    on.religion_seat_seeker_step(&mut game, 0, far, "Buddhism", true);
    assert_eq!(game.units[&far].pos, start);
    let kept = on.rival_seekers.borrow()[&far];

    // Another spreader now stands farther from the capital than the seeker.
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let outpost = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.players[0].explored.contains(pos)
                && game.city_at(**pos).is_none()
        })
        .map(|(pos, _)| *pos)
        .max_by_key(|pos| (game.wdist(capital, *pos), *pos))
        .unwrap();
    assert!(game.wdist(capital, outpost) > game.wdist(capital, start));
    let newcomer = spreader(&mut game, outpost);
    assert_eq!(
        on.unseen_rival_seekers(&game, 0, "Buddhism", true),
        vec![far]
    );
    // A fresh agent has no seeker yet and takes the farther unit.
    assert_eq!(
        seat(VictoryTarget::Religion, true, true).unseen_rival_seekers(&game, 0, "Buddhism", true),
        vec![newcomer]
    );

    // Fog opens right beside it: the goal it holds is kept all the same.
    let beside: Vec<Pos> = game
        .nbrs(start)
        .into_iter()
        .filter(|pos| game.city_at(*pos).is_none())
        .collect();
    for pos in &beside {
        game.players[0].explored.remove(pos);
    }
    assert_eq!(on.seeker_goal(&game, 0, far), Some(kept));
    for pos in beside {
        game.players[0].explored.insert(pos);
    }

    // The rival's city comes into view: nobody seeks, the memory is dropped,
    // and the unit has its ordinary turn back.
    for pos in game.wdisk(unseen, 1) {
        game.players[0].explored.insert(pos);
    }
    assert_eq!(AdvancedAi::unseen_met_rivals(&game, 0), 0);
    assert!(on
        .unseen_rival_seekers(&game, 0, "Buddhism", true)
        .is_empty());
    assert!(on.rival_seekers.borrow().is_empty());
    game.units.get_mut(&far).unwrap().moves_left = 2.0;
    assert!(!on.religion_seat_seeker_step(&mut game, 0, far, "Buddhism", true));
    assert_eq!(game.units[&far].pos, start);
}

/// ★ 082214Z at t150: Portuguese ground was first seen two tiles from Beja,
/// and the city nine turns later. Ground a met major owns whose city we have
/// not seen is the lead: the seeker takes the fog beside it before the fog
/// an explorer would take from where it stands.
#[test]
fn a_border_whose_city_is_unseen_is_the_lead() {
    let Board {
        mut game,
        far,
        unseen,
        ..
    } = board();
    // A second patch of fog, right beside the seeker and away from the rival.
    let start = game.units[&far].pos;
    let near_patch: Vec<Pos> =
        game.wdisk(start, 2)
            .into_iter()
            .filter(|pos| {
                *pos != start
                    && game.wdist(*pos, unseen) > game.wdist(start, unseen)
                    && game.city_at(*pos).is_none()
                    && game.map.get(*pos).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .collect();
    assert!(
        !near_patch.is_empty(),
        "fixture: dry ground behind the post"
    );
    for pos in &near_patch {
        game.players[0].explored.remove(pos);
    }
    let on = seat(VictoryTarget::Religion, true, true);

    // No border seen: an explorer's fog, wherever that is from here.
    let mut plain = game.clone();
    assert!(on.religion_seat_seeker_step(&mut plain, 0, far, "Buddhism", true));
    let (_, lead) = on.rival_seekers.borrow()[&far];
    assert!(!lead);

    // The rival's border is on the map, one tile outside the fog its city
    // stands in.
    let border = game
        .wring(unseen, FOG + 1)
        .into_iter()
        .filter(|pos| game.map.get(*pos).is_some())
        .min_by_key(|pos| (game.wdist(start, *pos), *pos))
        .expect("fixture: a tile outside the fog");
    game.unseen_major_borders.insert(border);
    let led = seat(VictoryTarget::Religion, true, true);
    let mut board = game.clone();
    assert!(led.religion_seat_seeker_step(&mut board, 0, far, "Buddhism", true));
    let (goal, lead) = led.rival_seekers.borrow()[&far];
    assert!(lead, "the border is the lead");
    assert!(board.wdist(goal, border) <= BORDER_LEAD_RADIUS);
    assert!(!board.players[0].explored.contains(&goal));
    assert!(
        !near_patch.contains(&goal),
        "not the nearer fog an explorer would take"
    );
    assert_ne!(board.units[&far].pos, start, "and it sets out for it");
    assert_eq!(board.units[&far].charges, 3);
}

/// A unit on the last charge it keeps, and one due home to heal, are not
/// sent to look: the next spreader out is.
#[test]
fn a_kept_last_charge_and_a_wounded_unit_do_not_seek() {
    let Board {
        game, far, home, ..
    } = board();
    let mut on = seat(VictoryTarget::Religion, true, true);

    let mut wounded = game.clone();
    wounded.units.get_mut(&far).unwrap().hp = super::super::apostle_army::RETREAT_FLOOR;
    let seekers = on.unseen_rival_seekers(&wounded, 0, "Buddhism", true);
    assert_eq!(seekers.len(), 1);
    assert!(
        home.contains(&seekers[0]),
        "a healthy spreader goes instead"
    );

    // `last-charge-joins-the-army` keeps a Missionary's last charge and gives
    // it its own scouting turn before the seeker's step is reached.
    on.enable_last_charge_joins_the_army();
    let mut spent = game.clone();
    spent.units.get_mut(&far).unwrap().charges = 1;
    assert!(on.keeps_its_last_charge(&spent, 0, far));
    let seekers = on.unseen_rival_seekers(&spent, 0, "Buddhism", true);
    assert_eq!(seekers.len(), 1);
    assert!(home.contains(&seekers[0]));

    // A corps of one spreads: its last spreader is never sent to look.
    let mut lone = game.clone();
    for uid in &home {
        lone.remove_unit(*uid);
    }
    assert!(seat(VictoryTarget::Religion, true, true)
        .unseen_rival_seekers(&lone, 0, "Buddhism", true)
        .is_empty());

    // Two rivals unseen: two seekers.
    let mut two = game.clone();
    for pos in two.wdisk(two.units[&far].pos, 1) {
        two.players[0].explored.remove(&pos);
    }
    two.players[0]
        .remembered_cities
        .retain(|_, city| city.owner != 1);
    assert_eq!(AdvancedAi::unseen_met_rivals(&two, 0), 2);
    assert_eq!(
        seat(VictoryTarget::Religion, true, true)
            .unseen_rival_seekers(&two, 0, "Buddhism", true)
            .len(),
        2
    );
}
