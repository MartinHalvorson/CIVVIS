use super::*;

fn fixture(map: WorldMap) -> Game {
    let mut game = Game::new_full(2, 16, 12, 521_010, 250, 0, false);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    game.map = map;
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
    }
    for player in game.players.iter_mut() {
        player.civ = "Rome".into();
        player.policies.clear();
        player.government = None;
    }
    game
}

// Keep the owned, sorted disk reduction as an independent reference for the
// visitor's seam, edge, duplicate and centre membership semantics.
fn owned_aura(game: &Game, unit: &Unit, effect: &str, support_only: bool) -> f64 {
    game.wdisk(unit.pos, 1)
        .into_iter()
        .flat_map(|position| game.unit_ids_at(position))
        .filter(|other| {
            game.units[other].owner == unit.owner && (support_only || **other != unit.id)
        })
        .filter_map(|other| {
            let spec = &game.rules.units[game.units[other].kind];
            (!support_only || spec.class == "support")
                .then(|| spec.effects.get(effect).copied().unwrap_or(0.0))
        })
        .fold(0.0, f64::max)
}

fn check_map(map: WorldMap) {
    let mut game = fixture(map);
    for (kind, value) in [("medic", 3.0), ("drone", 8.0), ("tagma", 11.0)] {
        let spec = Arc::make_mut(&mut game.rules)
            .units
            .get_interned_mut(Name::new(kind))
            .unwrap();
        spec.effects.insert("test_aura".into(), value);
        spec.effects.insert("test_negative_aura".into(), -value);
    }
    let positions: Vec<_> = game.map.tiles.keys().copied().collect();
    for (index, &position) in positions.iter().enumerate() {
        let kind = ["medic", "drone", "tagma", "warrior"][index % 4];
        game.spawn_test_unit(kind, index % 2, position);
        if index % 3 == 0 {
            game.spawn_test_unit("drone", 0, position);
            game.spawn_test_unit("tagma", 1, position);
        }
    }
    let check = |game: &Game| {
        for unit in game.units.values() {
            for effect in [
                "test_aura",
                "test_negative_aura",
                "adjacent_combat_strength",
                "absent",
            ] {
                assert_eq!(
                    game.adjacent_support_effect(unit, effect).to_bits(),
                    owned_aura(game, unit, effect, true).to_bits(),
                    "support: {effect}, {:?}, unit {}",
                    game.map.topology,
                    unit.id,
                );
                assert_eq!(
                    game.adjacent_friendly_unit_effect(unit, effect).to_bits(),
                    owned_aura(game, unit, effect, false).to_bits(),
                    "friendly: {effect}, {:?}, unit {}",
                    game.map.topology,
                    unit.id,
                );
            }
        }
    };
    check(&game);
    // The scan must follow current occupancy after movement/removal too.
    for id in game.units.keys().copied().step_by(3).collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    check(&game);
}

#[test]
fn aura_disk_globes_keep_pentagons_poles_stacks_and_owner_filters() {
    for frequency in [1, 3] {
        check_map(WorldMap::globe(frequency));
    }
}

#[test]
fn aura_disk_cylinders_keep_seams_and_narrow_wrap_duplicates() {
    for width in [1, 2, 9] {
        check_map(WorldMap::new(width, 6));
    }
}

#[test]
fn aura_disk_bounded_arenas_keep_edges() {
    check_map(WorldMap::arena(9, 6));
}

#[test]
fn aura_disk_friendly_territory_keeps_the_exact_policy_bonus() {
    for map in [
        WorldMap::globe(2),
        WorldMap::new(9, 6),
        WorldMap::arena(9, 6),
    ] {
        let mut game = fixture(map);
        let positions: Vec<_> = game.map.tiles.keys().copied().collect();
        let home = positions[positions.len() / 2];
        game.found_city_for(0, home, None);
        for position in positions {
            let id = game.spawn_test_unit("warrior", 0, position);
            let unit = &game.units[&id];
            let expected = game.wdisk(position, 1).into_iter().any(|pos| {
                game.map
                    .get(pos)
                    .and_then(|tile| tile.owner_city)
                    .and_then(|cid| game.cities.get(&cid))
                    .is_some_and(|city| city.owner == unit.owner)
            });
            let baseline = game.unit_unembarked_strength(unit);
            game.players[0].policies.insert(crate::name!("finest_hour"));
            let with_policy = game.unit_unembarked_strength(&game.units[&id]);
            assert_eq!(with_policy - baseline, if expected { 5.0 } else { 0.0 });
            game.players[0].policies.clear();
            game.remove_unit(id);
        }
    }
}
