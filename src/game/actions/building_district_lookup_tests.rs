use super::*;

fn board() -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(2, 24, 16, 91_493, 100, 0, false);
    let position = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "settler")
        .unwrap()
        .pos;
    let city = game.place_city(0, position, None);
    let positions = game.nbrs(position).to_vec();
    game.cities.get_mut(&city).unwrap().districts.clear();
    (game, city, positions)
}

/// The original scalar scan, including unique-district family resolution.
fn original(game: &Game, city: &City, building: Name) -> bool {
    let Some(family) = game.rules.buildings[building].district else {
        return true;
    };
    if family == "city_center" {
        return true;
    }
    let wanted = game.district_family(family);
    city.districts.iter().any(|(district, position)| {
        game.district_family(*district) == wanted
            && game.district_is_active(city, district, *position)
    })
}

#[test]
fn repeated_exact_districts_keep_active_later_placements_and_repairs() {
    let (mut game, cid, positions) = board();
    for &position in &positions[..2] {
        game.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(crate::name!("neighborhood"), position);
    }
    for (first, second, expected) in [
        (true, false, true),
        (true, true, false),
        (false, true, true),
    ] {
        game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = first;
        game.map.tiles.get_mut(&positions[1]).unwrap().pillaged = second;
        let city = &game.cities[&cid];
        let building = crate::name!("food_market");
        assert_eq!(game.building_district_is_active(city, building), expected);
        assert_eq!(
            game.building_district_is_active(city, building),
            original(&game, city, building)
        );
    }
}

#[test]
fn exact_pillage_falls_back_to_an_active_unique_and_honors_encampment_flag() {
    let (mut game, cid, positions) = board();
    for (building, exact, unique) in [
        (
            crate::name!("library"),
            crate::name!("campus"),
            crate::name!("seowon"),
        ),
        (
            crate::name!("barracks"),
            crate::name!("encampment"),
            crate::name!("ikanda"),
        ),
    ] {
        let city = game.cities.get_mut(&cid).unwrap();
        city.districts.clear();
        city.districts.insert(exact, positions[0]);
        city.districts.insert(unique, positions[1]);
        game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = true;
        game.map.tiles.get_mut(&positions[1]).unwrap().pillaged = false;
        for encampment in [false, true, false] {
            game.cities.get_mut(&cid).unwrap().encampment_pillaged = encampment;
            let city = &game.cities[&cid];
            assert_eq!(
                game.building_district_is_active(city, building),
                original(&game, city, building)
            );
            assert_eq!(
                game.building_district_is_active(city, building),
                exact != crate::name!("encampment") || !encampment
            );
        }
    }
}

#[test]
fn modded_requirement_uses_the_same_family_even_when_exact_kind_is_absent() {
    let (mut game, cid, positions) = board();
    Arc::make_mut(&mut game.rules)
        .buildings
        .get_interned_mut(crate::name!("library"))
        .unwrap()
        .district = Some(crate::name!("seowon"));
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), positions[0]);
    for pillaged in [false, true, false] {
        game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = pillaged;
        let city = &game.cities[&cid];
        assert_eq!(
            game.building_district_is_active(city, crate::name!("library")),
            !pillaged
        );
        assert_eq!(
            game.building_district_is_active(city, crate::name!("library")),
            original(&game, city, crate::name!("library"))
        );
    }
}

#[test]
fn edited_city_copies_read_their_own_districts_and_unrestricted_buildings_stay_active() {
    let (mut game, cid, positions) = board();
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), positions[0]);
    let mut edited = game.cities[&cid].clone();
    edited.districts.clear();
    assert!(game.building_district_is_active(&game.cities[&cid], crate::name!("library")));
    assert!(!game.building_district_is_active(&edited, crate::name!("library")));
    assert!(game.building_district_is_active(&edited, crate::name!("granary")));
    Arc::make_mut(&mut game.rules)
        .buildings
        .get_interned_mut(crate::name!("library"))
        .unwrap()
        .district = None;
    assert!(game.building_district_is_active(&edited, crate::name!("library")));
}
