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
