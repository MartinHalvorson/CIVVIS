use super::*;

fn fixture() -> (Game, u32, Pos, Pos) {
    let mut game = Game::new_full(1, 24, 16, 4797, 250, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    let city = game.found_city_for(0, (6, 8), None);
    let site = (7, 8);
    let other = (6, 9);
    for pos in [site, other] {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = Some(crate::name!("forest"));
        tile.resource = Some(crate::name!("deer"));
        tile.owner_city = Some(city);
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.improvement = None;
    }
    game.players[0]
        .techs
        .extend(game.rules.techs.keys().copied());
    let builder = game.spawn_unit("builder", 0, site);
    game.units.get_mut(&builder).unwrap().moves_left = 2.0;
    (game, builder, site, other)
}

#[test]
fn refused_harvest_cannot_spend_the_builders_turn_in_speculation() {
    let (mut game, builder, site, other) = fixture();
    assert!(game
        .builder_operations(0, site)
        .iter()
        .any(|op| op == "harvest_resource"));
    std::sync::Arc::make_mut(&mut game.blocked_improvements)
        .entry(site)
        .or_default()
        .insert(crate::name!("harvest_resource"));
    let before = (
        game.units[&builder].charges,
        game.units[&builder].moves_left,
        game.log.len(),
    );
    assert!(!game
        .builder_operations(0, site)
        .iter()
        .any(|op| op == "harvest_resource"));
    assert!(game
        .builder_operations(0, site)
        .iter()
        .any(|op| op == "chop_woods"));
    assert!(game
        .builder_operations(0, other)
        .iter()
        .any(|op| op == "harvest_resource"));
    assert!(game
        .apply(
            0,
            &Action::Improve {
                unit: builder,
                improvement: crate::name!("harvest_resource")
            }
        )
        .is_err());
    assert_eq!(
        (
            game.units[&builder].charges,
            game.units[&builder].moves_left,
            game.log.len()
        ),
        before
    );
    assert_eq!(game.map.tiles[&site].resource.as_deref(), Some("deer"));
    assert!(game
        .valid_improvements(0, site)
        .contains(&crate::name!("camp")));
}

#[test]
fn site_refusals_cover_clearing_and_releasing_them_restores_it() {
    let (mut game, builder, site, _) = fixture();
    std::sync::Arc::make_mut(&mut game.blocked_improvement_sites).insert(site);
    assert!(game.builder_operations(0, site).is_empty());
    game.blocked_improvement_sites = Default::default();
    assert!(game
        .builder_operations(0, site)
        .iter()
        .any(|op| op == "harvest_resource"));
    game.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("harvest_resource"),
        },
    )
    .unwrap();
    assert!(game.map.tiles[&site].resource.is_none());
}

/// The host refuses both clearings under an improvement: 75 of 75 accepted
/// REMOVE_FEATURE orders in the 2026-11-09 Deity census stood on unimproved
/// ground, and 55 of 64 refusals on a Lumber Mill, Plantation, Camp, Mine,
/// Quarry or Farm.
#[test]
fn an_improved_tile_offers_no_clearing_and_no_harvest() {
    let (mut game, builder, site, _) = fixture();
    let open = game.builder_operations(0, site);
    assert!(open.iter().any(|op| op == "chop_woods"));
    assert!(open.iter().any(|op| op == "harvest_resource"));

    game.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("camp"));
    let improved = game.builder_operations(0, site);
    assert!(!improved.iter().any(|op| op == "chop_woods"));
    assert!(!improved.iter().any(|op| op == "harvest_resource"));
    assert!(game
        .do_improve(0, builder, "chop_woods")
        .unwrap_err()
        .contains("cannot perform"));
    assert_eq!(
        game.map.tiles[&site].feature,
        Some(crate::name!("forest")),
        "the refused clearing leaves the Forest standing"
    );
    assert!(game.do_improve(0, builder, "harvest_resource").is_err());
    assert_eq!(game.map.tiles[&site].resource, Some(crate::name!("deer")));
}

#[test]
fn a_builder_cannot_build_over_another_improvement() {
    let (mut game, _, site, _) = fixture();
    assert!(
        game.valid_improvements(0, site)
            .contains(&crate::name!("camp")),
        "fixture"
    );
    game.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("lumber_mill"));
    assert!(game.valid_improvements(0, site).is_empty());
}

#[test]
fn only_a_strategic_connection_replaces_an_improvement() {
    let (mut game, _, site, _) = fixture();
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.feature = None;
    tile.resource = Some(crate::name!("iron"));
    tile.improvement = Some(crate::name!("farm"));
    assert_eq!(
        game.valid_improvements(0, site),
        vec![crate::name!("mine")],
        "the agent removes the Farm and then builds the Mine"
    );

    game.map.tiles.get_mut(&site).unwrap().resource = Some(crate::name!("wheat"));
    assert!(
        game.valid_improvements(0, site).is_empty(),
        "a bonus resource is not worth the swap the host refuses"
    );
}

/// Measured on the host: a clearing pays its shipped base, times the game
/// speed, times `1 + 9 × the share of all technologies and civics done`,
/// rounded down. The world era is not part of it.
#[test]
fn a_clearing_pays_by_progress_and_game_speed_not_by_world_era() {
    let (mut game, _, site, _) = fixture();
    game.map.tiles.get_mut(&site).unwrap().resource = None;
    let forest = |game: &Game| -> f64 {
        game.builder_operation_payout(0, site, "chop_woods")
            .into_iter()
            .map(|(_, amount)| amount)
            .sum()
    };
    let speed = game.game_speed.scale(1.0);
    let all = (game.rules.techs.len() + game.rules.civics.len()) as f64;

    game.players[0].techs.clear();
    game.players[0].civics.clear();
    assert_eq!(forest(&game), (20.0 * speed).floor(), "nothing learned");

    let learned: Vec<_> = game.rules.techs.keys().copied().take(7).collect();
    game.players[0].techs.extend(learned);
    assert_eq!(
        forest(&game),
        (20.0 * (speed * (1.0 + 9.0 * 7.0 / all))).floor(),
        "seven of them learned"
    );

    let before = forest(&game);
    game.world_era += 3;
    assert_eq!(forest(&game), before, "a later world era changes nothing");

    let civics: Vec<_> = game.rules.civics.keys().copied().collect();
    let techs: Vec<_> = game.rules.techs.keys().copied().collect();
    game.players[0].civics.extend(civics);
    game.players[0].techs.extend(techs);
    assert_eq!(forest(&game), (200.0 * speed).floor(), "everything learned");
}

/// Measured on the host: a Forest cleared into a Settler under Colonization
/// paid 27 to 30 Production where the plain rule gives 19 to 20, and exactly
/// the plain rule with no card. Food carries no such bonus.
#[test]
fn a_clearing_is_paid_at_the_queue_items_production_bonus() {
    let (mut game, _, site, _) = fixture();
    game.map.tiles.get_mut(&site).unwrap().resource = None;
    let city = game.map.tiles[&site].owner_city.unwrap();
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    game.cities.get_mut(&city).unwrap().queue = vec![settler.clone()];
    let production = |game: &Game, operation: &str| -> f64 {
        game.builder_operation_payout(0, site, operation)
            .into_iter()
            .filter(|(yield_type, _)| yield_type == "production")
            .map(|(_, amount)| amount)
            .sum()
    };
    let scale = game.builder_harvest_scale(0);

    game.players[0].policies.remove(&Name::new("colonization"));
    let plain = game.item_prod_mult(0, city, Some(&settler));
    assert_eq!(
        production(&game, "chop_woods"),
        (20.0 * scale * plain).floor()
    );

    game.players[0].policies.insert(Name::new("colonization"));
    let carded = game.item_prod_mult(0, city, Some(&settler));
    assert!(
        (carded - plain - 0.5).abs() < 1e-9,
        "Colonization is +50% toward Settlers"
    );
    assert_eq!(
        production(&game, "chop_woods"),
        (20.0 * scale * carded).floor()
    );
    assert!(production(&game, "chop_woods") > (20.0 * scale * plain).floor());

    // The same card does nothing for a Monument in the queue.
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let other = game.item_prod_mult(0, city, Some(&monument));
    game.cities.get_mut(&city).unwrap().queue = vec![monument];
    assert_eq!(
        production(&game, "chop_woods"),
        (20.0 * scale * other).floor()
    );

    // Food is never multiplied: a Marsh pays its plain 20, scaled.
    game.cities.get_mut(&city).unwrap().queue = vec![settler];
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("marsh"));
    let food: f64 = game
        .builder_operation_payout(0, site, "clear_marsh")
        .into_iter()
        .map(|(_, amount)| amount)
        .sum();
    assert_eq!(food, (20.0 * scale).floor());
}
