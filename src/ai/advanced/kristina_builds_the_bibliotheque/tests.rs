//! `kristina-builds-the-bibliotheque`: Sweden's Plaza tiers and first Plaza.
use super::*;
use crate::game::Action;
use crate::name::Name;

/// A founded Swedish capital that knows State Workforce.
fn sweden(seed: u64) -> (Game, u32) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("KRISTINAPLAZA", seed),
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
    g.cities.get_mut(&cid).unwrap().pop = 7;
    (g, cid)
}

fn place_plaza(g: &mut Game, cid: u32) {
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
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(plaza, site);
}

fn building(item: Option<Item>) -> Option<Name> {
    match item {
        Some(Item::Building { building }) => Some(building),
        _ => None,
    }
}

#[test]
fn swedens_plaza_takes_the_hall_then_the_bibliotheque() {
    let (mut g, cid) = sweden(61_301);
    place_plaza(&mut g, cid);
    let off = BasicAi::new();
    let mut on = BasicAi::new();
    on.kristina_builds_the_bibliotheque = true;
    assert_eq!(
        building(off.plaza_tier_item(&g, 0, cid)),
        Some(crate::name!("warlords_throne")),
        "off, the stock list"
    );
    assert_eq!(
        building(on.plaza_tier_item(&g, 0, cid)),
        Some(crate::name!("ancestral_hall"))
    );
    for first in ["ancestral_hall", "warlords_throne"] {
        let mut built = g.clone();
        built
            .cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(Name::new(first));
        assert_eq!(
            building(on.plaza_tier_item(&built, 0, cid)),
            Some(crate::name!("queens_bibliotheque")),
            "after the {first}"
        );
        assert_eq!(
            building(off.plaza_tier_item(&built, 0, cid)),
            Some(crate::name!("grand_masters_chapel")),
            "off, the Chapel that rules the Bibliotheque out"
        );
    }
    let mut rome = g.clone();
    rome.players[0].civ = "Rome".to_string();
    assert_eq!(
        on.plaza_tier_item(&rome, 0, cid),
        off.plaza_tier_item(&rome, 0, cid),
        "another civilization keeps the stock list"
    );
}

#[test]
fn the_prophet_race_does_not_hold_swedens_plaza() {
    let (g, _) = sweden(61_302);
    let mut off = BasicAi::new();
    off.enter_prophet_race = true;
    let mut on = off.clone();
    on.kristina_builds_the_bibliotheque = true;
    assert!(off.plaza_prophet_race_open(&g, 0));
    assert!(off.plaza_waits_for_the_prophet(&g, 0));
    assert!(!on.plaza_waits_for_the_prophet(&g, 0));
}

#[test]
fn a_city_takes_the_plaza_when_the_capital_cannot() {
    let (mut g, capital) = sweden(61_303);
    let off = BasicAi::new();
    let mut on = BasicAi::new();
    on.kristina_builds_the_bibliotheque = true;
    assert!(!BasicAi::capital_cannot_hold_the_plaza(&g, 0));
    assert!(
        on.plaza_item_for_seat(&g, 0, capital).is_some(),
        "the capital first"
    );
    // The capital is lost; the city that remains is not a capital.
    g.cities.get_mut(&capital).unwrap().is_capital = false;
    assert!(BasicAi::capital_cannot_hold_the_plaza(&g, 0));
    assert!(
        off.plaza_item_for_seat(&g, 0, capital).is_none(),
        "off, capitals only"
    );
    assert!(matches!(
        on.plaza_item_for_seat(&g, 0, capital),
        Some(Item::District { district, .. }) if g.district_family(district) == "government_plaza"
    ));
    // One Plaza only.
    place_plaza(&mut g, capital);
    assert!(on.plaza_item_for_seat(&g, 0, capital).is_none());
}
