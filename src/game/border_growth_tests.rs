use super::*;

fn controlled_game(seed: u64) -> (Game, u32, Pos) {
    controlled_game_for(seed, false)
}

fn controlled_game_for(seed: u64, minor: bool) -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 20, seed, 120, 0, false);
    game.players[0].is_minor = minor;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = None;
    }
    let center = *game
        .map
        .tiles
        .keys()
        .find(|position| game.wdisk(**position, 5).len() == 91)
        .expect("controlled map has a complete five-tile city radius");
    let city = game.found_city_for(0, center, None);
    (game, city, center)
}

fn ring(game: &Game, center: Pos, radius: i32) -> Vec<Pos> {
    game.wdisk(center, radius)
        .into_iter()
        .filter(|position| game.wdist(*position, center) == radius)
        .collect()
}

fn claim(game: &mut Game, city: u32, position: Pos) {
    if game.map.tiles[&position].owner_city == Some(city) {
        return;
    }
    game.map.tiles.get_mut(&position).unwrap().owner_city = Some(city);
    game.cities
        .get_mut(&city)
        .unwrap()
        .owned_tiles
        .push(position);
}

#[test]
fn only_a_full_civilization_moves_its_border_with_its_own_culture() {
    // CivilizationLevels.CanAnnexTilesWithCulture is 1 for FULL_CIV and 0
    // for CITY_STATE. A city-state banking enough Culture to buy the next
    // plot several times over still does not take it; Envoys are its only
    // route to new ground.
    // Both open on the same centre-plus-first-ring, so the only thing
    // that separates the two counts afterwards is the annexation gate.
    for (minor, expected) in [(false, 8), (true, 7)] {
        let (mut game, city, _) = controlled_game_for(941_101, minor);
        let before = game.cities[&city].owned_tiles.len();
        assert_eq!(before, 7);
        game.cities.get_mut(&city).unwrap().border_culture = 500.0;
        game.process_city(0, city);
        assert_eq!(
            game.cities[&city].owned_tiles.len(),
            expected,
            "minor={minor}"
        );
    }
}

#[test]
fn a_city_state_cannot_buy_a_plot_with_gold() {
    // CanAnnexTilesWithGold is 0 for CITY_STATE, so no price is ever
    // quoted however much Gold the city-state is holding.
    for minor in [false, true] {
        let (mut game, city, center) = controlled_game_for(941_102, minor);
        game.players[0].gold = 10_000.0;
        let target = ring(&game, center, 2)
            .into_iter()
            .find(|position| game.map.tiles[position].owner_city.is_none())
            .expect("a second-ring plot is unowned");
        game.players[0].explored.insert(target);
        assert_eq!(
            game.plot_purchase_cost(0, city, target).is_some(),
            !minor,
            "minor={minor}"
        );
    }
}

#[test]
fn a_city_state_founds_owning_its_whole_first_ring() {
    // A founding city-state takes the same centre-plus-first-ring as a
    // full civilization: no plot inside its own ring is left neutral.
    for minor in [false, true] {
        let (game, city, center) = controlled_game_for(941_103, minor);
        assert_eq!(
            game.cities[&city].owned_tiles.len(),
            7,
            "centre plus six ring tiles, minor={minor}"
        );
        assert!(
            ring(&game, center, 1)
                .into_iter()
                .all(|position| game.map.tiles[&position].owner_city == Some(city)),
            "a ring plot was left unowned, minor={minor}"
        );
    }
}

#[test]
fn a_city_state_takes_no_ring_plot_that_a_neighbour_already_holds() {
    // The full ring is a grant of *free* ground, not a seizure: a plot a
    // neighbouring city already owns stays with that neighbour.
    let (mut game, held_by, _) = controlled_game_for(941_104, false);
    let center = *game
        .map
        .tiles
        .keys()
        .find(|position| {
            game.wdist(**position, game.cities[&held_by].pos) == 3
                && game.wdisk(**position, 5).len() == 91
        })
        .expect("a plot three tiles out with a complete radius");
    let taken = ring(&game, center, 1)
        .into_iter()
        .find(|position| game.wdist(*position, game.cities[&held_by].pos) == 2)
        .expect("a ring plot facing the neighbour");
    claim(&mut game, held_by, taken);

    game.players.push(Player::new(1, "Geneva", true));
    let minor_city = game.found_city_for(1, center, None);
    assert_eq!(game.map.tiles[&taken].owner_city, Some(held_by));
    assert_eq!(game.cities[&minor_city].owned_tiles.len(), 6);
}

#[test]
fn a_resource_one_ring_out_outbids_a_barren_nearer_plot() {
    // PLOT_INFLUENCE_RING_COST is 100 and PLOT_INFLUENCE_RESOURCE_COST is
    // -105, so one Resource is worth marginally more than one ring of
    // distance and the border reaches past a barren neighbour for it.
    let (mut game, city, center) = controlled_game(941_001);
    let second_ring = ring(&game, center, 2);
    let last_inner = second_ring[0];
    for position in second_ring.into_iter().skip(1) {
        claim(&mut game, city, position);
    }
    game.map.tiles.get_mut(&last_inner).unwrap().terrain = crate::name!("mountain");

    let third_ring_resource = ring(&game, center, 3)[0];
    game.map
        .tiles
        .get_mut(&third_ring_resource)
        .unwrap()
        .resource = Some(crate::name!("diamonds"));

    game.expand_borders(city);

    assert_eq!(game.map.tiles[&third_ring_resource].owner_city, Some(city));
    assert_eq!(game.map.tiles[&last_inner].owner_city, None);
}

#[test]
fn distance_still_decides_between_plots_of_equal_worth() {
    // The ring cost only loses to a Resource or Natural Wonder. Between
    // two identical plots the nearer one always wins, so ordinary growth
    // still fills outward ring by ring.
    let (mut game, city, center) = controlled_game(941_006);
    let second_ring = ring(&game, center, 2);
    let near = second_ring[0];
    for position in second_ring.into_iter().skip(1) {
        claim(&mut game, city, position);
    }
    let far = ring(&game, center, 3)[0];
    for position in [near, far] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }

    game.expand_borders(city);

    assert_eq!(game.map.tiles[&near].owner_city, Some(city));
    assert_eq!(game.map.tiles[&far].owner_city, None);
}

#[test]
fn natural_growth_can_claim_the_fifth_ring() {
    let (mut game, city, center) = controlled_game(941_002);
    for radius in 2..=4 {
        for position in ring(&game, center, radius) {
            claim(&mut game, city, position);
        }
    }
    let before: BTreeSet<Pos> = game.cities[&city].owned_tiles.iter().copied().collect();

    game.expand_borders(city);

    let added: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| !before.contains(position))
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(game.wdist(added[0], center), 5);

    for position in ring(&game, center, 5) {
        claim(&mut game, city, position);
    }
    let complete_five_ring_border = game.cities[&city].owned_tiles.len();
    game.expand_borders(city);
    assert_eq!(
        game.cities[&city].owned_tiles.len(),
        complete_five_ring_border,
        "natural border growth must not enter ring 6"
    );
}

#[test]
fn resource_influence_beats_a_richer_unresourced_tile_in_the_same_ring() {
    let (mut game, city, center) = controlled_game(941_003);
    let second_ring = ring(&game, center, 2);
    let resource = second_ring[0];
    let rich = second_ring[1];
    {
        let tile = game.map.tiles.get_mut(&resource).unwrap();
        tile.terrain = crate::name!("snow");
        tile.resource = Some(crate::name!("iron"));
    }
    {
        let tile = game.map.tiles.get_mut(&rich).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = Some(crate::name!("forest"));
    }

    game.expand_borders(city);

    assert_eq!(game.map.tiles[&resource].owner_city, Some(city));
    assert_eq!(game.map.tiles[&rich].owner_city, None);
}

#[test]
fn water_influence_penalty_keeps_barren_land_ahead_of_open_coast() {
    let (mut game, city, center) = controlled_game(941_004);
    let second_ring = ring(&game, center, 2);
    for position in &second_ring {
        game.map.tiles.get_mut(position).unwrap().terrain = crate::name!("coast");
    }
    let barren_land = second_ring[0];
    game.map.tiles.get_mut(&barren_land).unwrap().terrain = crate::name!("desert");

    game.expand_borders(city);

    assert_eq!(game.map.tiles[&barren_land].owner_city, Some(city));
}

#[test]
fn natural_wonder_influence_beats_ordinary_tile_yields() {
    let (mut game, city, center) = controlled_game(941_005);
    let second_ring = ring(&game, center, 2);
    let natural_wonder = second_ring[0];
    let ordinary = second_ring[1];
    {
        let tile = game.map.tiles.get_mut(&natural_wonder).unwrap();
        tile.terrain = crate::name!("desert");
        tile.feature = Some(crate::name!("uluru"));
    }
    {
        let tile = game.map.tiles.get_mut(&ordinary).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = Some(crate::name!("forest"));
    }

    game.expand_borders(city);

    assert_eq!(game.map.tiles[&natural_wonder].owner_city, Some(city));
    assert_eq!(game.map.tiles[&ordinary].owner_city, None);
}

#[test]
fn plot_purchase_curve_matches_measured_gathering_storm_prices() {
    let (mut game, _, _) = controlled_game(941_007);
    game.game_speed = GameSpeed::Marathon;
    game.players[0].techs.clear();
    game.players[0].civics.clear();
    game.players[0]
        .policies
        .insert(crate::name!("land_surveyors"));

    let techs: Vec<Name> = game.rules.techs.keys().take(11).cloned().collect();
    let civics: Vec<Name> = game.rules.civics.keys().take(11).cloned().collect();
    game.players[0].techs.extend(techs.iter().take(8).cloned());
    game.players[0]
        .civics
        .extend(civics.iter().take(8).cloned());
    assert_eq!(game.tile_purchase_cost(0, 50.0), 180.0);
    assert_eq!(game.tile_purchase_cost(0, 75.0), 272.0);

    game.players[0]
        .civics
        .extend(civics.iter().take(11).cloned());
    assert_eq!(game.tile_purchase_cost(0, 50.0), 204.0);
    assert_eq!(game.tile_purchase_cost(0, 75.0), 308.0);

    game.players[0].policies.clear();
    game.game_speed = GameSpeed::Standard;
    game.players[0].techs.clear();
    game.players[0].civics.clear();
    assert_eq!(game.tile_purchase_cost(0, 50.0), 50.0);
    assert_eq!(game.tile_purchase_cost(0, 75.0), 75.0);
}

#[test]
fn buying_a_plot_extends_only_that_citys_connected_three_ring_border() {
    let (mut game, city, center) = controlled_game(941_008);
    game.players[0].techs.clear();
    game.players[0].civics.clear();
    game.players[0].gold = 1_000.0;
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());

    let second = ring(&game, center, 2)[0];
    let connected_third = ring(&game, center, 3)
        .into_iter()
        .find(|position| game.nbrs(*position).contains(&second))
        .unwrap();
    let disconnected_third = ring(&game, center, 3)
        .into_iter()
        .find(|position| *position != connected_third && !game.nbrs(*position).contains(&second))
        .unwrap();

    assert_eq!(game.plot_purchase_cost(0, city, second), Some(50.0));
    assert_eq!(game.plot_purchase_cost(0, city, connected_third), None);
    assert_eq!(game.plot_purchase_cost(0, city, disconnected_third), None);
    assert!(game.legal_actions(0).contains(&Action::BuyPlot {
        city,
        pos: second,
        cost: 50.0,
    }));

    // The quote is informational, not authority: a forged zero-price
    // action still pays the live 50 Gold price.
    game.apply(
        0,
        &Action::BuyPlot {
            city,
            pos: second,
            cost: 0.0,
        },
    )
    .unwrap();
    assert_eq!(game.players[0].gold, 950.0);
    assert_eq!(game.map.tiles[&second].owner_city, Some(city));
    assert!(game.cities[&city].owned_tiles.contains(&second));
    assert_eq!(
        game.plot_purchase_cost(0, city, connected_third),
        Some(75.0)
    );
    assert_eq!(game.plot_purchase_cost(0, city, disconnected_third), None);

    game.players[0].gold = 74.0;
    let before = game.cities[&city].owned_tiles.clone();
    assert!(game
        .apply(
            0,
            &Action::BuyPlot {
                city,
                pos: connected_third,
                cost: 75.0,
            },
        )
        .is_err());
    assert_eq!(game.map.tiles[&connected_third].owner_city, None);
    assert_eq!(game.cities[&city].owned_tiles, before);
}
