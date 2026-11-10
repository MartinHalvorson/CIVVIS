use super::*;

/// A coastal seat at turn 60 with no city-states and its starting units gone;
/// player 1 holds two cities, its capital first, met but neither seen.
/// Returns the game, the rival's capital tile and its town's tile.
fn coast() -> (Game, Pos, Pos) {
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
            if sites.len() == 2 {
                break;
            }
            if game.wdist(pos, home) >= 6 && sites.iter().all(|site| game.wdist(pos, *site) >= 4) {
                sites.push(pos);
            }
        }
        if sites.len() < 2 {
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
        game.turn = 60;
        if !BasicAi::empire_is_coastal(&game, 0) {
            refusals.push(format!("{seed}: home is not coastal"));
            continue;
        }
        let capital = sites.iter().copied().find(|site| {
            game.city_at(*site)
                .is_some_and(|cid| game.cities[&cid].is_capital)
        });
        let Some(capital) = capital else {
            refusals.push(format!("{seed}: no rival capital"));
            continue;
        };
        let town = sites.into_iter().find(|site| *site != capital).unwrap();
        return (game, capital, town);
    }
    panic!("fixture: no seed gave a coastal seat and its rival: {refusals:?}");
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_domination_charts_every_capital();
    }
    ai
}

/// See `contact_gap_open`: a met, located rival whose capital we have not
/// seen keeps the gap open for the gene's seat only.
#[test]
fn an_unseen_capital_keeps_the_contact_gap_open() {
    let (mut game, capital, town) = coast();
    game.players[0].explored.insert(town);
    assert!(
        !BasicAi::contact_fleet_gap(&game, 0),
        "met and located, no city-states"
    );
    assert_eq!(AdvancedAi::uncharted_rival_capitals(&game, 0), 1);
    for gene in [false, true] {
        let mut ai = seat(VictoryTarget::Domination, gene);
        ai.base.charts_rival_capitals = ai.domination_charts_every_capital_active(&game, 0);
        assert_eq!(ai.base.charts_rival_capitals, gene, "gene {gene}");
        assert_eq!(ai.base.contact_gap_open(&game, 0), gene, "gene {gene}");
    }
    // Seen: the gap closes.
    game.players[0].explored.insert(capital);
    assert_eq!(AdvancedAi::uncharted_rival_capitals(&game, 0), 0);
    assert!(!seat(VictoryTarget::Domination, true).domination_charts_every_capital_active(&game, 0));
    // An unmet rival counts as uncharted.
    game.players[0].met.remove(&1);
    assert_eq!(AdvancedAi::uncharted_rival_capitals(&game, 0), 1);
}

#[test]
fn only_a_domination_seat_after_the_opening() {
    let (mut game, _, _) = coast();
    assert!(seat(VictoryTarget::Domination, true).domination_charts_every_capital_active(&game, 0));
    assert!(!seat(VictoryTarget::Science, true).domination_charts_every_capital_active(&game, 0));
    assert!(!seat(VictoryTarget::Religion, true).seat_charts_the_water(&game, 0));
    game.turn = 20;
    assert!(!seat(VictoryTarget::Domination, true).domination_charts_every_capital_active(&game, 0));
}

/// The religion seat's charting parts act for the gene's seat: Cartography's
/// road, the hull at Buttress.
#[test]
fn the_way_to_cartography_and_the_early_hull() {
    let (mut game, _, _) = coast();
    let on = seat(VictoryTarget::Domination, true);
    let off = seat(VictoryTarget::Domination, false);
    for tech in ["shipbuilding", "buttress", "cartography"] {
        assert_eq!(
            on.religion_seat_cartography_value(&game, 0, tech),
            super::super::religion_seat_charts_the_rivals::CARTOGRAPHY_VALUE,
            "{tech}"
        );
        assert_eq!(off.religion_seat_cartography_value(&game, 0, tech), 0.0);
    }
    assert_eq!(on.religion_seat_cartography_value(&game, 0, "archery"), 0.0);
    assert!(!on.religion_seat_hull_before_cartography(&game, 0));
    game.players[0].techs.insert(Name::new("buttress"));
    assert!(on.religion_seat_hull_before_cartography(&game, 0));
    assert!(!off.religion_seat_hull_before_cartography(&game, 0));
    game.players[0].techs.insert(Name::new("cartography"));
    assert_eq!(
        on.religion_seat_cartography_value(&game, 0, "shipbuilding"),
        0.0
    );
}
