//! `kristina-fills-to-theme`: the Bibliotheque houses the starved person, and
//! the stock theme swaps stand down.
use super::*;
use crate::ai::advanced::VictoryTarget;
use crate::game::Action;
use crate::mirror::{StateCity, StateGreatWork};

/// A founded Swedish capital whose Plaza holds the Ancestral Hall.
fn sweden_with_a_hall(seed: u64) -> (Game, u32) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("KRISTINATHEME", seed),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].civ = "Sweden".to_string();
    g.players[0].civics.insert(crate::name!("state_workforce"));
    let cid = g.player_city_ids(0)[0];
    let center = g.cities[&cid].pos;
    let site = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != center
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    let plaza = crate::name!("government_plaza");
    g.map.tiles.get_mut(&site).unwrap().district = Some(plaza);
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 7;
    city.districts.insert(plaza, site);
    city.buildings.push(crate::name!("ancestral_hall"));
    (g, cid)
}

#[test]
fn a_starved_musician_asks_for_the_bibliotheque() {
    let (g, cid) = sweden_with_a_hall(71_401);
    let off = BasicAi::new();
    let mut on = BasicAi::new();
    on.kristina_fills_to_theme = true;
    assert_eq!(off.kristina_slot_item(&g, 0, cid, "music"), None, "off");
    assert_eq!(
        on.kristina_slot_item(&g, 0, cid, "artifact"),
        None,
        "not an artifact"
    );
    for work in ["writing", "art", "music"] {
        assert_eq!(
            on.kristina_slot_item(&g, 0, cid, work),
            Some(Some(Item::Building {
                building: crate::name!("queens_bibliotheque")
            })),
            "{work}"
        );
    }
    let mut queued = g.clone();
    queued.cities.get_mut(&cid).unwrap().queue = vec![Item::Building {
        building: crate::name!("queens_bibliotheque"),
    }];
    assert_eq!(on.kristina_slot_item(&queued, 0, cid, "music"), Some(None));
    let mut rome = g.clone();
    rome.players[0].civ = "Rome".to_string();
    assert_eq!(on.kristina_slot_item(&rome, 0, cid, "music"), None);
}

#[test]
fn the_stock_theme_swaps_stand_down_for_kristina() {
    let work = |artist: &str, slot: i32| StateGreatWork {
        kind: format!("{artist}_{slot}"),
        object: "GREATWORKOBJECT_LANDSCAPE".into(),
        era: None,
        creator: artist.into(),
        building: "BUILDING_MUSEUM_ART".into(),
        slot,
    };
    let city = |id: i64, artist: &str| StateCity {
        id,
        buildings: vec!["BUILDING_MUSEUM_ART".into()],
        great_works: Some((0..3).map(|s| work(artist, s)).collect()),
        ..Default::default()
    };
    let mut g = Game::new_full(2, 18, 10, 903, 500, 0, false);
    let state = StateSnapshot {
        cities: vec![city(10, "A"), city(20, "B")],
        ..Default::default()
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    g.players[0].civ = "Sweden".to_string();
    assert!(!ai.native_art_museum_swaps(&g, 0, &state).is_empty(), "off");
    ai.enable_kristina_fills_to_theme();
    assert!(ai.native_art_museum_swaps(&g, 0, &state).is_empty(), "on");
    g.players[0].civ = "Rome".to_string();
    assert!(
        !ai.native_art_museum_swaps(&g, 0, &state).is_empty(),
        "not Sweden"
    );
}
