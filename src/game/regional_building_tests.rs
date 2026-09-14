use super::*;

// The original loop is retained as an independent oracle for query reordering.
fn original_regional_effects(game: &Game, city: &City) -> (Yields, f64) {
    let mut groups: BTreeMap<&str, (Yields, f64)> = BTreeMap::new();
    let integrate_industry =
        game.governor_effect(city.owner, city.id, "regional_industry_all") > 0.0;
    let mexico_city_regional_range = game.grants_city_state_unique_bonus(city.owner, "Mexico City");
    for source in game
        .cities
        .values()
        .filter(|source| source.owner == city.owner)
    {
        for building in &source.buildings {
            if source.pillaged_buildings.contains(building)
                || !game.building_district_is_active(source, building)
            {
                continue;
            }
            let spec = &game.rules.buildings[building];
            let origin = spec
                .district
                .and_then(|district| game.city_district_family_position(source, district))
                .unwrap_or(source.pos);
            let regional_range = spec.regional_range
                + if mexico_city_regional_range
                    && spec.district.is_some_and(|district| {
                        game.district_is_family(district, crate::name!("industrial_zone"))
                            || game
                                .district_is_family(district, crate::name!("entertainment_complex"))
                            || game.district_is_family(district, crate::name!("water_park"))
                    })
                {
                    3
                } else {
                    0
                };
            if regional_range <= 0 || game.wdist(origin, city.pos) > regional_range {
                continue;
            }
            let group: &str = if !spec.regional_group.is_empty() {
                spec.regional_group.as_str()
            } else {
                spec.replaces
                    .map_or_else(|| building.as_str(), |name| name.as_str())
            };
            let integrate_this_group = integrate_industry
                && spec.district.is_some_and(|district| {
                    game.district_is_family(district, crate::name!("industrial_zone"))
                });
            let entry = groups.entry(group).or_default();
            let mut source_yields = spec.yields;
            let mut source_amenity = spec.amenity;
            if game.city_is_powered(source) {
                Game::add_powered_building_yields(spec, &mut source_yields);
                source_amenity += spec.effects.get("powered_amenity").copied().unwrap_or(0.0);
            }
            entry.0.food = entry.0.food.max(source_yields.food);
            if integrate_this_group {
                entry.0.production += source_yields.production;
            } else {
                entry.0.production = entry.0.production.max(source_yields.production);
            }
            entry.0.gold = entry.0.gold.max(source_yields.gold);
            entry.0.science = entry.0.science.max(source_yields.science);
            entry.0.culture = entry.0.culture.max(source_yields.culture);
            entry.0.faith = entry.0.faith.max(source_yields.faith);
            entry.1 = entry.1.max(source_amenity);
        }
    }
    groups.values().fold(
        (Yields::default(), 0.0),
        |(mut yields, amenities), (group_yields, group_amenities)| {
            yields.add(*group_yields);
            (yields, amenities + group_amenities)
        },
    )
}

fn board() -> (Game, u32, Pos, Vec<u32>) {
    let mut game = Game::new_full(1, 40, 28, 913_3550, 300, 0, false);
    game.players[0].civ = "Rome".to_string();
    let position = game
        .units
        .values()
        .find(|unit| unit.kind == "settler")
        .unwrap()
        .pos;
    let source = game.found_city_for(0, position, None);
    let district = game.nbrs(position)[0];
    let mut targets = vec![source];
    for distance in [3, 6, 9, 12] {
        let pos = game
            .map
            .tiles
            .iter()
            .find_map(|(pos, tile)| {
                (game.wdist(district, *pos) == distance
                    && tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile))
                .then_some(*pos)
            })
            .expect("fixture has a target at each radius");
        targets.push(game.found_city_for(0, pos, None));
    }
    (game, source, district, targets)
}

#[test]
fn regional_prefilter_matches_all_stock_buildings_and_mexico_city_bonus() {
    let (mut game, source, district, targets) = board();
    let buildings: Vec<Name> = game.rules.buildings.keys().copied().collect();
    let minor = game.players.len();
    game.players.push(Player::new(minor, "Mexico City", true));
    for suzerain in [false, true] {
        game.players[0].envoys = if suzerain { vec![(minor, 3)] } else { vec![] };
        assert_eq!(
            game.grants_city_state_unique_bonus(0, "Mexico City"),
            suzerain
        );
        for &building in &buildings {
            let family = game.rules.buildings[building].district;
            let city = game.cities.get_mut(&source).unwrap();
            city.buildings = vec![building];
            city.districts.clear();
            if let Some(family) = family {
                city.districts.insert(family, district);
            }
            for (pillaged_building, pillaged_district, powered) in [
                (false, false, false),
                (false, false, true),
                (true, false, true),
                (false, true, true),
            ] {
                game.cities.get_mut(&source).unwrap().pillaged_buildings = if pillaged_building {
                    [building].into_iter().collect()
                } else {
                    BTreeSet::new()
                };
                game.map.tiles.get_mut(&district).unwrap().pillaged = pillaged_district;
                game.players[0]
                    .city_power
                    .insert(source, if powered { 1000.0 } else { 0.0 });
                for &target in &targets {
                    let city = &game.cities[&target];
                    assert_eq!(
                        game.regional_building_effects_uncached(city),
                        original_regional_effects(&game, city),
                        "building={building}, target={target}, suzerain={suzerain}, building pillaged={pillaged_building}, district pillaged={pillaged_district}, powered={powered}"
                    );
                }
            }
        }
    }
}

#[test]
fn regional_prefilter_preserves_scope_expiry_after_pillage_and_repair() {
    let (mut game, source, district, targets) = board();
    let city = game.cities.get_mut(&source).unwrap();
    city.buildings = vec![crate::name!("factory")];
    city.districts
        .insert(crate::name!("industrial_zone"), district);
    game.players[0].city_power.insert(source, 1000.0);
    for pillaged in [false, true, false] {
        game.map.tiles.get_mut(&district).unwrap().pillaged = pillaged;
        let expected: Vec<_> = targets
            .iter()
            .map(|cid| original_regional_effects(&game, &game.cities[cid]))
            .collect();
        {
            let _memo = game.query_memo();
            for _ in 0..2 {
                for (&cid, value) in targets.iter().zip(&expected) {
                    assert_eq!(game.regional_building_effects(&game.cities[&cid]), *value);
                }
            }
        }
        assert!(game.query_memo.regional.borrow().is_none());
    }
}
