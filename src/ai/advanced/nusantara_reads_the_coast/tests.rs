use super::*;

/// A one-city Indonesia with its capital founded; the Campus candidate is
/// the first owned non-centre plot, its neighbours rewritten as told.
fn indonesia(coast_neighbours: usize) -> (Game, Pos) {
    let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, false);
    game.players[0].civ = "Indonesia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    let center = game.cities[&cid].pos;
    let site = game
        .nbrs(center)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| tile.owner_city == Some(cid))
        })
        .expect("the capital owns a neighbour");
    let neighbours: Vec<Pos> = game
        .nbrs(site)
        .into_iter()
        .filter(|pos| *pos != center)
        .collect();
    for (index, pos) in neighbours.into_iter().enumerate() {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = if index < coast_neighbours {
            crate::name!("coast")
        } else {
            crate::name!("grassland")
        };
        tile.feature = None;
        tile.hills = false;
    }
    (game, site)
}

/// Four Coast neighbours are two points of Great Nusantara: +2 Science on a
/// Campus, +2 Production on an Industrial Zone, nothing on a Commercial Hub.
#[test]
fn the_planner_prices_two_points_for_four_coast_tiles() {
    let (mut game, site) = indonesia(4);
    let campus = crate::name!("campus");
    let base = game.district_yields(campus, site);
    assert_eq!(
        game.district_site_yields(campus, site),
        base,
        "no planner, no term"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_nusantara_reads_the_coast();
    ai.seat_nusantara_site_planner(&mut game, 0);
    assert_eq!(game.nusantara_site_planner, Some(0));
    assert_eq!(game.nusantara_site_term(campus, site).science, 2.0);
    assert_eq!(
        game.district_site_yields(campus, site).science,
        base.science + 2.0
    );
    assert_eq!(
        game.nusantara_site_term(crate::name!("industrial_zone"), site)
            .production,
        2.0
    );
    assert_eq!(
        game.nusantara_site_term(crate::name!("commercial_hub"), site),
        Yields::default()
    );
    assert_eq!(
        game.district_yields(campus, site),
        base,
        "real yields untouched"
    );
}

/// One Coast tile is half a point and pays nothing; three pay one.
#[test]
fn coast_pays_by_whole_pairs() {
    for (coast, points) in [(1, 0.0), (3, 1.0)] {
        let (mut game, site) = indonesia(coast);
        let mut ai = AdvancedAi::new();
        ai.enable_nusantara_reads_the_coast();
        ai.seat_nusantara_site_planner(&mut game, 0);
        assert_eq!(
            game.nusantara_site_term(crate::name!("campus"), site)
                .science,
            points,
            "{coast} Coast"
        );
    }
}

/// Off, or for another civilization, the seat names no planner.
#[test]
fn the_gene_off_or_another_civ_names_no_planner() {
    let (mut game, site) = indonesia(4);
    AdvancedAi::new().seat_nusantara_site_planner(&mut game, 0);
    assert_eq!(game.nusantara_site_planner, None, "gene off");

    game.players[0].civ = "Korea".to_string();
    let mut ai = AdvancedAi::new();
    ai.enable_nusantara_reads_the_coast();
    ai.seat_nusantara_site_planner(&mut game, 0);
    assert_eq!(game.nusantara_site_planner, None, "Korea");
    assert_eq!(
        game.nusantara_site_term(crate::name!("campus"), site),
        Yields::default()
    );
}
