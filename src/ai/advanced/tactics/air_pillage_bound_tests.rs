use super::*;

/// A bomber of seat 0 within range of an enemy city ringed with every kind of
/// pillage target: improvements of each price, a district with buildings, and
/// an enemy aircraft standing on one of them.
fn front() -> (Game, u32) {
    let mut game = Game::new_full(2, 24, 16, 71_016, 120, 0, false);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let land = |game: &Game, position: Pos| {
        game.map
            .get(position)
            .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
    };
    let enemy_center = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            land(&game, *position) && game.nbrs(*position).into_iter().all(|n| land(&game, n))
        })
        .expect("a land city site");
    let enemy_city = game.found_city_for(1, enemy_center, None);
    let ring: Vec<Pos> = game.nbrs(enemy_center).into_iter().collect();
    for (position, improvement) in ring.iter().zip(["airstrip", "mine", "farm", "pasture"]) {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.owner_city = Some(enemy_city);
        tile.improvement = Some(crate::name::Name::new(improvement));
        tile.district = None;
        tile.pillaged = false;
    }
    if let Some(position) = ring.get(4) {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.owner_city = Some(enemy_city);
        tile.improvement = None;
        tile.district = Some(crate::name!("campus"));
        tile.pillaged = false;
        let city = game.cities.get_mut(&enemy_city).unwrap();
        city.districts.insert(crate::name!("campus"), *position);
        city.buildings.push(crate::name!("library"));
    }
    if let Some(position) = ring.get(1) {
        game.spawn_test_unit("bomber", 1, *position);
    }
    let base = game
        .wdisk(enemy_center, game.rules.units["bomber"].range)
        .into_iter()
        .find(|position| game.wdist(*position, enemy_center) >= 4 && land(&game, *position))
        .expect("a base in range");
    game.found_city_for(0, base, None);
    let bomber = game.spawn_test_unit("bomber", 0, base);
    (game, bomber)
}

#[test]
fn no_pillage_is_worth_more_than_its_bound_and_the_same_target_wins() {
    let (game, bomber) = front();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let legal = game.legal_doctrine_actions(0, bomber);
    let targets: Vec<Pos> = legal
        .iter()
        .filter_map(|action| match action {
            Action::AirPillage { unit, target } if *unit == bomber => Some(*target),
            _ => None,
        })
        .collect();
    assert!(
        targets.len() >= 3,
        "the fixture offers several targets: {targets:?}"
    );
    let mut brute: Option<(f64, Pos)> = None;
    for target in &targets {
        let value = ai.air_pillage_value(&game, 0, bomber, *target);
        let bound = ai.air_pillage_bound(&game, 0, *target);
        assert!(
            bound >= value,
            "{target:?}: value {value} above its bound {bound}"
        );
        let wins = brute.is_none_or(|(best, at)| {
            value.total_cmp(&best).then_with(|| at.cmp(target)) != std::cmp::Ordering::Less
        });
        if wins {
            brute = Some((value, *target));
        }
    }
    let chosen = ai
        .best_air_pillage(&game, 0, bomber, &legal)
        .map(|(value, target, _)| (value, target));
    assert_eq!(chosen, brute);
}
