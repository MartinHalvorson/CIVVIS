use super::*;
use crate::ai::commercial_hub::tests::{emperor_empire, emperor_governor};
use crate::game::install_test_district;
use crate::name::Name;
use crate::rules::Yields;
use std::sync::Arc;

/// The Emperor pin's delegated governor (`emperor_governor`), with or
/// without `campus-buildings-first`, and the Commercial Hub step as asked.
fn governor(hub: bool, gene: bool) -> BasicAi {
    let mut ai = emperor_governor(hub);
    ai.campus_buildings_first = gene;
    ai
}

/// The six-city pick with the lent floor met (twelve units, six melee and
/// six ranged, one siege support), six Builders and one Trader, or with no
/// army at all.
fn pick(ai: &BasicAi, game: &Game, cid: u32, army: bool) -> Option<Item> {
    let (military, melee, ranged, siege) = if army { (12, 6, 6, 1) } else { (0, 0, 0, 0) };
    ai.pick_item(game, 0, cid, 6, 0, 6, 1, siege, military, melee, ranged)
}

fn building(name: &str) -> Option<Item> {
    Some(Item::Building {
        building: Name::new(name),
    })
}

fn is_district(item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { .. }))
}

fn is_hub(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "commercial_hub")
}

/// The Emperor empire at turn 70 Online with Education in hand: every city
/// holds its Campus and Library and can build its University.
fn empire_with_education(tag: &str, seed: u64) -> (Game, Vec<u32>) {
    let (mut game, ranked) = emperor_empire(tag, seed);
    for tech in ["education", "apprenticeship", "mathematics"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for cid in &ranked {
        assert_eq!(
            BasicAi::campus_building_item(&game, 0, *cid),
            building("university"),
            "fixture: the University is buildable"
        );
    }
    (game, ranked)
}

/// See `BasicAi::campus_buildings_first`: with the floor met the stock
/// governor opens another district beside a Campus whose Library and
/// Education stand; the gene builds the University first, and the Library
/// before it where the Library is missing. Without the gene, the stock pick.
#[test]
fn a_standing_campus_builds_its_university_before_another_district() {
    let (game, ranked) = empire_with_education("CAMPUSBUILDINGS", 92_101);
    let top = ranked[0];
    let stock = pick(&governor(false, false), &game, top, true);
    assert!(
        is_district(&stock),
        "fixture: the stock pick opens another district: {stock:?}"
    );
    assert_eq!(
        pick(&governor(false, true), &game, top, true),
        building("university"),
        "the University comes first"
    );
    // A Campus without its Library takes the Library, not the University.
    let mut bare = game.clone();
    bare.cities
        .get_mut(&top)
        .unwrap()
        .buildings
        .retain(|held| *held != "library");
    assert_eq!(
        pick(&governor(false, true), &bare, top, true),
        building("library")
    );
    // A city whose Campus holds every building it can build keeps the stock pick.
    let mut built = game.clone();
    built
        .cities
        .get_mut(&top)
        .unwrap()
        .buildings
        .push(crate::name!("university"));
    assert_eq!(BasicAi::campus_building_item(&built, 0, top), None);
    assert_eq!(
        pick(&governor(false, true), &built, top, true),
        pick(&governor(false, false), &built, top, true),
        "nothing left to raise"
    );
}

/// The step stands behind the military floor and the steps ahead of it:
/// with no army every city's pick is the stock pick -- the floor's unit, or
/// the industrial hub's zone in its city.
#[test]
fn the_military_floor_still_comes_first() {
    let (game, ranked) = empire_with_education("CAMPUSFLOOR", 92_103);
    let mut units = 0;
    for cid in &ranked {
        let stock = pick(&governor(false, false), &game, *cid, false);
        units += usize::from(matches!(stock, Some(Item::Unit { .. })));
        assert_eq!(pick(&governor(false, true), &game, *cid, false), stock);
    }
    assert!(
        units >= 4,
        "fixture: the floor takes most cities' builds ({units})"
    );
}

/// A city that would take more than `CAMPUS_BUILDING_MAX_TURNS` over its
/// University keeps the stock pick.
#[test]
fn a_slow_city_keeps_the_stock_pick() {
    let (mut game, ranked) = empire_with_education("CAMPUSSLOW", 92_105);
    let slow = ranked[5];
    // Two Production a turn: the adjustment that leaves the city at 2.
    let lent = game.observed_city_yield_adjustments[&slow].production;
    let production = lent - game.city_yields(slow).production + 2.0;
    Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        slow,
        Yields {
            production,
            ..Default::default()
        },
    );
    assert!(
        game.item_cost_for(0, &building("university").unwrap()) / game.city_yields(slow).production
            > CAMPUS_BUILDING_MAX_TURNS,
        "fixture: the University is slow here"
    );
    assert_eq!(BasicAi::campus_building_item(&game, 0, slow), None);
    assert_eq!(
        pick(&governor(false, true), &game, slow, true),
        pick(&governor(false, false), &game, slow, true)
    );
}

/// Under `commercial-hub-and-traders` the empire's first hub keeps its
/// slot; once a hub stands, the next hub's city builds its University in
/// that slot, ahead of the floor as the hub would have been. A standing
/// hub's Market is no new district and keeps its slot.
#[test]
fn a_second_commercial_hub_gives_its_slot_to_the_university() {
    let (mut game, ranked) = empire_with_education("CAMPUSHUB", 92_107);
    let first = pick(&governor(true, true), &game, ranked[0], false);
    assert!(
        is_hub(&game, &first),
        "the empire's first hub keeps its slot: {first:?}"
    );
    assert_eq!(first, pick(&governor(true, false), &game, ranked[0], false));
    install_test_district(&mut game, ranked[0], "commercial_hub");
    let second = pick(&governor(true, false), &game, ranked[1], false);
    assert!(
        is_hub(&game, &second),
        "fixture: the hub step opens a second hub: {second:?}"
    );
    assert_eq!(
        pick(&governor(true, true), &game, ranked[1], false),
        building("university"),
        "the second hub's slot goes to the University"
    );
    assert_eq!(
        pick(&governor(true, true), &game, ranked[0], false),
        building("market"),
        "the standing hub still takes its Market"
    );
    // Without the University the second hub opens as before.
    let mut built = game.clone();
    built
        .cities
        .get_mut(&ranked[1])
        .unwrap()
        .buildings
        .push(crate::name!("university"));
    assert_eq!(
        pick(&governor(true, true), &built, ranked[1], false),
        pick(&governor(true, false), &built, ranked[1], false)
    );
}
