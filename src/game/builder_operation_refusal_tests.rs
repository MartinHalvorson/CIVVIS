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
