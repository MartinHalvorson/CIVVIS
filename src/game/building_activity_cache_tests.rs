use super::*;

/// The pre-optimization scan, retained as an independent oracle.
fn scanned_activity(game: &Game, city: &City, building: Name) -> bool {
    let Some(family) = game.rules.buildings[building].district else {
        return true;
    };
    if family == "city_center" {
        return true;
    }
    let wanted = game.district_family(family);
    city.districts.iter().any(|(district, position)| {
        game.district_family(*district) == wanted
            && !game.map.tiles[position].pillaged
            && !(game.district_is_family(district, crate::name!("encampment"))
                && city.encampment_pillaged)
    })
}

fn board() -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(2, 24, 16, 91_192, 100, 0, false);
    let pos = game
        .units
        .values()
        .find(|u| u.owner == 0 && u.kind == "settler")
        .unwrap()
        .pos;
    let city = game.place_city(0, pos, None);
    let positions = game.nbrs(pos).to_vec();
    (game, city, positions)
}

#[test]
fn building_activity_matches_scan_for_every_stock_district_and_building() {
    let (mut game, cid, positions) = board();
    let districts: Vec<Name> = game.rules.districts.keys().copied().collect();
    let buildings: Vec<Name> = game.rules.buildings.keys().copied().collect();
    assert!(!districts.is_empty() && !buildings.is_empty());
    for district in districts {
        game.cities.get_mut(&cid).unwrap().districts.clear();
        // Repeatable placements: a pillaged first instance must not hide an
        // active second one, nor should a stale answer survive a repair.
        for &pos in &positions[..2] {
            game.cities
                .get_mut(&cid)
                .unwrap()
                .districts
                .insert(district, pos);
        }
        for (first, second, encampment) in [
            (false, false, false),
            (true, false, false),
            (true, true, false),
            (false, false, true),
            (false, false, false),
        ] {
            game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = first;
            game.map.tiles.get_mut(&positions[1]).unwrap().pillaged = second;
            game.cities.get_mut(&cid).unwrap().encampment_pillaged = encampment;
            for &building in &buildings {
                let city = &game.cities[&cid];
                assert_eq!(game.building_district_is_active(city, building),
                    scanned_activity(&game, city, building),
                    "district={district}, building={building}, pillaged={first}/{second}, encampment={encampment}");
            }
        }
    }
    game.cities.get_mut(&cid).unwrap().districts.clear();
    for building in buildings {
        assert_eq!(
            game.building_district_is_active(&game.cities[&cid], building),
            scanned_activity(&game, &game.cities[&cid], building)
        );
    }
}

#[test]
fn a_pillaged_exact_district_still_checks_an_active_replacement() {
    let (mut game, cid, positions) = board();
    let city = game.cities.get_mut(&cid).unwrap();
    city.districts.clear();
    city.districts.insert(crate::name!("campus"), positions[0]);
    city.districts.insert(crate::name!("seowon"), positions[1]);
    game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = true;
    game.map.tiles.get_mut(&positions[1]).unwrap().pillaged = false;
    assert!(game.building_district_is_active(&game.cities[&cid], crate::name!("library")));
    game.map.tiles.get_mut(&positions[1]).unwrap().pillaged = true;
    assert!(!game.building_district_is_active(&game.cities[&cid], crate::name!("library")));
}
