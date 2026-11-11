use super::*;

/// A coastal seat at turn 160 with no city-states and its starting units gone;
/// player 1 holds three cities, met but none seen. Returns the game and the
/// rival's city tiles, capital first.
fn racer() -> (Game, Vec<Pos>) {
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
            if sites.len() == 3 {
                break;
            }
            if game.wdist(pos, home) >= 6 && sites.iter().all(|site| game.wdist(pos, *site) >= 4) {
                sites.push(pos);
            }
        }
        if sites.len() < 3 {
            refusals.push(format!("{seed}: room for {} rival cities", sites.len()));
            continue;
        }
        game.place_city(0, home, None);
        for site in &sites {
            game.place_city(1, *site, None);
        }
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        for site in &sites {
            game.players[0].explored.remove(site);
        }
        game.turn = 160;
        if !BasicAi::empire_is_coastal(&game, 0) {
            refusals.push(format!("{seed}: home is not coastal"));
            continue;
        }
        return (game, sites);
    }
    panic!("fixture: no seed gave a coastal seat and its rival: {refusals:?}");
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_denial_charts_the_racer();
    }
    ai
}

#[test]
fn the_leading_science_rival_is_charted_until_every_city_is_seen() {
    let (mut game, sites) = racer();
    assert!(game.victory_conditions.science);
    for target in [VictoryTarget::Culture, VictoryTarget::Domination] {
        assert_eq!(
            seat(target, true).denial_racer(&game, 0),
            Some(1),
            "{target:?}"
        );
        assert_eq!(
            seat(target, false).denial_racer(&game, 0),
            None,
            "{target:?}"
        );
    }
    assert_eq!(
        seat(VictoryTarget::Science, true).denial_racer(&game, 0),
        None
    );
    assert_eq!(
        seat(VictoryTarget::Religion, true).denial_racer(&game, 0),
        None
    );
    // Before turn 100 Online, and with no rival on the Science clock: nothing.
    let mut early = game.clone();
    early.turn = 60;
    assert_eq!(
        seat(VictoryTarget::Culture, true).denial_racer(&early, 0),
        None
    );
    // Two of three seen: still charted; all three: done.
    for site in &sites[..2] {
        game.players[0].explored.insert(*site);
    }
    assert_eq!(
        seat(VictoryTarget::Culture, true).denial_racer(&game, 0),
        Some(1)
    );
    game.players[0].explored.insert(sites[2]);
    assert_eq!(
        seat(VictoryTarget::Culture, true).denial_racer(&game, 0),
        None
    );
}

#[test]
fn the_gap_and_the_water_parts_act_for_the_racer() {
    let (mut game, sites) = racer();
    game.players[0].explored.insert(sites[1]);
    assert!(!BasicAi::contact_fleet_gap(&game, 0), "met and located");
    let mut on = seat(VictoryTarget::Culture, true);
    on.base.lane_charting_gap = on.lane_charts_the_water(&game, 0);
    assert!(on.base.contact_gap_open(&game, 0));
    assert!(on.seat_charts_the_water(&game, 0));
    let mut off = seat(VictoryTarget::Culture, false);
    off.base.lane_charting_gap = off.lane_charts_the_water(&game, 0);
    assert!(!off.base.contact_gap_open(&game, 0));
    assert!(!off.seat_charts_the_water(&game, 0));
}

#[test]
fn the_hunt_row_walks_into_the_racer_once_its_ground_is_open() {
    let (mut game, sites) = racer();
    game.players[0].explored.insert(sites[1]);
    let on = seat(VictoryTarget::Culture, true);
    // Closed borders: the passage is worth paying for, and no row yet.
    assert!(on.find_capital_row(&game, 0).is_none());
    assert_eq!(
        on.find_the_capital_passage_gold(&game, 0, 1),
        Some(super::super::find_the_capital::FIND_CAPITAL_PASSAGE_GOLD)
    );
    assert_eq!(
        seat(VictoryTarget::Culture, false).find_the_capital_passage_gold(&game, 0, 1),
        None
    );
    let until = game.turn + 30;
    game.players[1].open_borders_until.insert(0, until);
    let row = on.find_capital_row(&game, 0).expect("a hunt row once open");
    assert!(!game.players[0].explored.contains(&row.at));
    assert!(seat(VictoryTarget::Culture, false)
        .find_capital_row(&game, 0)
        .is_none());
}
