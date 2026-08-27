use super::*;

/// A site the HOST engine refused must stop being settleable, and must stop
/// nothing else.
///
/// The 2-city ceiling was a feedback gap: Civilization VI refused a FOUND_CITY
/// order, nothing carried that back, and CIVVIS re-derived the same site from the
/// same board every turn — 18 refusals on one tile across turns 20, 33 and 79,
/// 141 in another run. This is the mechanism that breaks the loop, so it is worth
/// a test that would fail if the check were dropped or inverted.
#[test]
fn a_host_refused_site_is_not_settleable_but_its_neighbour_still_is() {
    let mut game = Game::new(2, 24, 16, 1, 200, 0);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    // Somewhere open, well clear of any city so only the block can refuse it.
    let site = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            let tile = &game.map.tiles[pos];
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.tile_is_natural_wonder(tile)
                && game.cities.values().all(|c| game.wdist(c.pos, *pos) >= 4)
        })
        .expect("a standard map has open land");
    let settler = game.spawn_unit("settler", 0, site);

    assert!(
        game.can_found_city(settler),
        "the site must be settleable before anything blocks it, or the test proves nothing"
    );

    std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(site);
    assert!(
        !game.can_found_city(settler),
        "a site Civilization VI refused must not be offered again"
    );

    // ⚠ The block is per TILE, not a blanket ban on founding. A settler that
    // walks one tile off a refused site must be able to found there, otherwise
    // the cure is worse than the ceiling it was added to fix.
    let elsewhere = crate::hex::neighbors(site)
        .into_iter()
        .find(|pos| {
            game.map.tiles.get(pos).is_some_and(|tile| {
                !game.rules.is_water(tile)
                    && game.rules.is_passable(tile)
                    && !game.tile_is_natural_wonder(tile)
            })
        })
        .expect("an inland site has a passable neighbour");
    game.units.get_mut(&settler).unwrap().pos = elsewhere;
    assert!(
        game.can_found_city(settler),
        "blocking one tile must not block the ground beside it"
    );
}

/// A tile the HOST engine refused to improve must offer no improvements at all.
///
/// ⭐ A TILE ALREADY CARRYING AN IMPROVEMENT IS NOT OFFERED THAT IMPROVEMENT
/// AGAIN — and the whole live duplicate-order fix depends on it.
///
/// Measured on the live ladder 2026-08-11: CIVVIS orders an improvement, it
/// succeeds, and the identical order comes back 27–39 times a run. The cause
/// is not the planner: it is that the mirror only learned about a finished
/// improvement on the next periodic tile sweep, so the board still showed the
/// ground bare. #1565 and #1567 report the improvement immediately.
///
/// That fix is only sufficient because of the exclusion this test pins —
/// `valid_improvements` skips an improvement equal to the one already on the
/// tile. Remove it and the duplicates return with a correct board, and
/// nothing else in the suite would say so.
///
/// ⚠ I predicted the opposite from reading, and a grep that stopped seventy
/// lines short is why. The condition lives at the end of the improvement
/// loop, not beside the national-park check at the top.
#[test]
fn a_tile_is_not_offered_the_improvement_it_already_has() {
    let mut game = Game::new(2, 24, 16, 1, 200, 0);
    let centre = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            let tile = &game.map.tiles[pos];
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.tile_is_natural_wonder(tile)
        })
        .expect("a standard map has open land");
    game.place_city(0, centre, None);
    let Some((pos, improvement)) = crate::hex::ring(centre, 1)
        .into_iter()
        .chain(crate::hex::ring(centre, 2))
        .find_map(|pos| {
            game.valid_improvements(0, pos)
                .into_iter()
                .next()
                .map(|imp| (pos, imp))
        })
    else {
        panic!("the ground around a capital should contain an improvable tile");
    };

    // Put that very improvement on the tile, as a finished build would.
    game.map.tiles.get_mut(&pos).expect("the tile").improvement = Some(improvement);

    assert!(
        !game.valid_improvements(0, pos).contains(&improvement),
        "a tile holding {improvement} must not be offered {improvement} again, \
         or CIVVIS re-orders what it has just built"
    );
    // ⚠ And only that one. Replacing an improvement is a real decision in
    // Civilization VI — a Farm over a Mine — so the tile must still offer
    // something, or this would be a much bigger change than it looks.
    // ⚠ NOT asserted here: that a DIFFERENT improvement survives, which is
    // the other half of the rule (replacing a Mine with a Farm is a real
    // Civilization VI decision). No tile within two rings of a capital
    // offers two valid improvements on this map — terrain, feature and
    // resource constrain them to one each — so the fixture cannot state it
    // without inventing ground. Left unclaimed rather than asserted weakly.
}

/// The largest refusal category measured — 311 `IMPROVEMENT_MINE` refusals in one
/// run — because CIVVIS names improvements from its own terrain model and the two
/// rulesets disagree tile for tile. Gated in `valid_improvements` because that is
/// the single chokepoint every improvement decision passes through.
#[test]
fn a_host_refused_tile_offers_no_improvements() {
    let mut game = Game::new(2, 24, 16, 1, 200, 0);
    // A builder may only improve territory its own civilization holds, so the
    // seat needs a CITY before any tile is improvable at all. `Game::new` hands
    // out starting units, not cities, so one has to be founded here or the search
    // below finds nothing and the test proves nothing.
    let centre = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            let tile = &game.map.tiles[pos];
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.tile_is_natural_wonder(tile)
        })
        .expect("a standard map has open land");
    game.place_city(0, centre, None);
    let improvable = crate::hex::ring(centre, 1)
        .into_iter()
        .chain(crate::hex::ring(centre, 2))
        .find(|pos| !game.valid_improvements(0, *pos).is_empty());
    let Some(pos) = improvable else {
        // Nothing to prove if no tile near the capital can be improved; say so
        // rather than passing vacuously.
        panic!("the ground around a capital should contain an improvable tile");
    };

    std::sync::Arc::make_mut(&mut game.blocked_improvement_sites).insert(pos);
    assert!(
        game.valid_improvements(0, pos).is_empty(),
        "a tile Civilization VI refused must offer nothing, or the builder loops on it"
    );
}

/// A Great Person the host has on a plot occupies it in the civilian layer;
/// see `great_person_plots`. Measured on run civvis-20260816T003229Z: a
/// Builder was ordered onto the founded Prophet's tile 25 turns running.
#[test]
fn a_great_persons_plot_offers_nothing_and_blocks_the_civilian_layer() {
    let mut game = Game::new(2, 24, 16, 2, 200, 0);
    let centre = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            let tile = &game.map.tiles[pos];
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.tile_is_natural_wonder(tile)
                && crate::hex::ring(*pos, 1).iter().all(|n| {
                    game.map.get(*n).is_some_and(|t| {
                        !game.rules.is_water(t) && game.rules.is_passable(t)
                    })
                })
        })
        .expect("a standard map has open land with open neighbours");
    game.place_city(0, centre, None);
    let plot = crate::hex::ring(centre, 1)
        .into_iter()
        .find(|pos| !game.valid_improvements(0, *pos).is_empty())
        .expect("the ground around a capital should contain an improvable tile");
    let builder = game.spawn_unit("builder", 0, centre);
    let warrior = game.spawn_unit("warrior", 0, centre);
    assert!(game.can_move(builder, plot), "the plot is open before anyone stands on it");

    // One of ours: the civilian layer is taken, the military layer is not.
    game.great_person_plots.insert(plot, 0);
    assert!(
        game.valid_improvements(0, plot).is_empty(),
        "a plot our Great Person holds offers a builder nothing, or it loops on it"
    );
    assert!(!game.can_move(builder, plot), "and the builder cannot step onto it");
    assert!(game.can_move(warrior, plot), "while a military unit still can");

    // A rival's, at peace: nothing of ours may enter.
    game.great_person_plots.insert(plot, 1);
    assert!(!game.can_move(builder, plot));
    assert!(!game.can_move(warrior, plot), "a foreign civilian blocks the step at peace");
    game.at_war.insert((0, 1));
    assert!(game.can_move(warrior, plot), "and at war the military step is a capture");
    assert!(!game.can_move(builder, plot), "which a builder still cannot make");

    // Gone: the plot is ordinary ground again.
    game.great_person_plots.remove(&plot);
    assert!(!game.valid_improvements(0, plot).is_empty());
    assert!(game.can_move(builder, plot));
}

#[test]
fn terrain_route_does_not_build_through_an_incompatible_feature() {
    let mut game = Game::new(2, 24, 16, 2, 200, 0);
    let centre = game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        .map(|(pos, _)| *pos)
        .unwrap();
    let city = game.place_city(0, centre, None);
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != centre)
        .unwrap();
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    tile.resource = None;
    tile.improvement = None;
    game.players[0].techs.insert(crate::name!("mining"));

    assert!(
        !game.valid_improvements(0, site).contains(&crate::name!("mine")),
        "Firaxis does not offer the Hills terrain route through Woods"
    );
    game.map.tiles.get_mut(&site).unwrap().feature = None;
    assert!(game.valid_improvements(0, site).contains(&crate::name!("mine")));
}

#[test]
fn a_host_refused_trade_route_is_not_offered_again() {
    let mut game = Game::new(2, 24, 16, 3, 200, 0);
    let sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        .map(|(pos, _)| *pos)
        .collect();
    let origin = sites[0];
    let destination = sites
        .iter()
        .copied()
        .find(|pos| game.wdist(origin, *pos) >= 4 && game.wdist(origin, *pos) <= 15)
        .unwrap();
    let origin_city = game.place_city(0, origin, None);
    let destination_city = game.place_city(1, destination, None);
    assert!(game.can_establish_trade_route(0, origin_city, destination_city));

    std::sync::Arc::make_mut(&mut game.blocked_trade_routes).insert((origin, destination));
    assert!(!game.can_establish_trade_route(0, origin_city, destination_city));
}

#[test]
fn a_host_refused_trade_route_is_not_enumerated_either() {
    // The enumeration used to paraphrase the validator with a looser
    // inline filter that skipped `blocked_trade_routes`, so a run's notes
    // reported waiting routes for a trader whose every destination the
    // host had refused (run civvis-20260815T081505Z, "routes=2" for 54
    // turns). What `legal_actions` offers must be what `do_trade_route`
    // would take.
    let mut game = Game::new(2, 24, 16, 3, 200, 0);
    let sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        .map(|(pos, _)| *pos)
        .collect();
    let origin = sites[0];
    let destination = sites
        .iter()
        .copied()
        .find(|pos| game.wdist(origin, *pos) >= 4 && game.wdist(origin, *pos) <= 15)
        .unwrap();
    game.place_city(0, origin, None);
    let destination_city = game.place_city(1, destination, None);
    let trader = game.spawn_test_unit("trader", 0, origin);
    // The enumeration sits behind the empire capacity gate, which is zero
    // before Foreign Trade; the gate is not what this test is about.
    std::sync::Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);

    let offered = |game: &Game| {
        game.legal_actions(0).into_iter().any(|action| {
            matches!(
                action,
                Action::TradeRoute { unit, city }
                    if unit == trader && city == destination_city
            )
        })
    };
    assert!(offered(&game), "the fixture route must start legal");

    std::sync::Arc::make_mut(&mut game.blocked_trade_routes).insert((origin, destination));
    assert!(
        !offered(&game),
        "a host-refused route must vanish from the enumeration, not \
         linger as an action the engine would refuse"
    );
}

#[test]
fn every_civilization_has_a_deep_unique_city_name_pool() {
    for civilization in CIV_NAMES {
        let names = city_names(civilization);
        assert!(
            names.len() >= 16,
            "{civilization} should not exhaust its city names in a long standard game"
        );
        assert_eq!(
            names.iter().copied().collect::<BTreeSet<_>>().len(),
            names.len(),
            "{civilization} city names should be unique"
        );
    }
}
