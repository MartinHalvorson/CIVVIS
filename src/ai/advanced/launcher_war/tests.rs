use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

/// Two majors at war on open grassland; seat 1's city at (26, 12) stands a
/// Spaceport at (24, 12) we have seen, and its Moon and Mars are landed.
fn fixture() -> (Game, AdvancedAi, u32, Pos) {
    let mut g = Game::new_full(2, 40, 24, 936301, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 12), None);
    let rival_city = g.found_city_for(1, (26, 12), None);
    let pad: Pos = (24, 12);
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(rival_city);
    tile.district = Some(crate::name!("spaceport"));
    tile.improvement = None;
    tile.pillaged = false;
    g.cities
        .get_mut(&rival_city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
    ] {
        g.players[1].science_projects.insert(project.to_string());
    }
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 195;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    (g, ai, rival_city, pad)
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

#[test]
fn both_genes_are_opt_in() {
    opt_in_off_in_both_controllers("war-raids-the-pads", |ai| ai.war_raids_the_pads);
    opt_in_off_in_both_controllers("no-peace-with-a-launcher", |ai| ai.no_peace_with_a_launcher);
}

/// G422's shape, on foot: a cavalry in reach of a standing pad walks onto
/// it and pillages it.
#[test]
fn a_raider_in_reach_walks_onto_the_pad_and_pillages_it() {
    let (mut g, mut ai, _, pad) = fixture();
    let raider = g.spawn_test_unit("cavalry", 0, (20, 12));
    let mut off = g.clone();
    assert!(ai
        .plan_pad_raids(&mut off, 0, &plan(), &BTreeSet::new())
        .is_empty());
    assert!(!off.map.tiles[&pad].pillaged, "off: the pad stands");
    ai.enable_war_raids_the_pads();
    let raiders = ai.plan_pad_raids(&mut g, 0, &plan(), &BTreeSet::new());
    assert_eq!(raiders, BTreeSet::from([raider]));
    assert_eq!(g.units[&raider].pos, pad);
    assert!(g.map.tiles[&pad].pillaged, "the pad is pillaged");
    assert_eq!(g.players[0].counters.get("pad_raid:pillaged"), Some(&1));
}

#[test]
fn no_raid_at_peace_short_of_the_moon_or_into_the_rivals_army() {
    for case in ["peace", "one_project", "guarded", "reserved"] {
        let (mut g, mut ai, _, pad) = fixture();
        ai.enable_war_raids_the_pads();
        let raider = g.spawn_test_unit("cavalry", 0, (20, 12));
        let mut reserved = BTreeSet::new();
        match case {
            "peace" => g.at_war.clear(),
            "one_project" => {
                g.players[1].science_projects.remove("launch_moon_landing");
                g.players[1].science_projects.remove("launch_mars_colony");
            }
            "guarded" => {
                for at in [(24, 11), (25, 11), (23, 13), (24, 13), (25, 13)] {
                    g.spawn_test_unit("cavalry", 1, at);
                }
            }
            _ => {
                reserved.insert(raider);
            }
        }
        let raiders = ai.plan_pad_raids(&mut g, 0, &plan(), &reserved);
        assert!(raiders.is_empty(), "{case}: no raid");
        assert!(!g.map.tiles[&pad].pillaged, "{case}: the pad stands");
    }
}

#[test]
fn the_war_with_a_launcher_is_kept_until_the_launch_or_our_cities_fall() {
    let (mut g, mut ai, _, pad) = fixture();
    assert!(
        !ai.launcher_keeps_the_war(&g, 0, 1),
        "off: peace stays open"
    );
    ai.enable_no_peace_with_a_launcher();
    assert!(ai.launcher_keeps_the_war(&g, 0, 1));
    // A city of ours falling reopens it.
    let home = g.player_city_ids(0)[0];
    {
        let city = g.cities.get_mut(&home).unwrap();
        city.last_attacked = g.turn;
        city.hp = 80;
    }
    assert!(
        !ai.launcher_keeps_the_war(&g, 0, 1),
        "a falling city may sue"
    );
    {
        let city = g.cities.get_mut(&home).unwrap();
        city.last_attacked = 0;
        city.hp = 200;
    }
    g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
    assert!(!ai.launcher_keeps_the_war(&g, 0, 1), "no standing pad");
    g.map.tiles.get_mut(&pad).unwrap().pillaged = false;
    g.players[1]
        .science_projects
        .insert("exoplanet_expedition".to_string());
    assert!(!ai.launcher_keeps_the_war(&g, 0, 1), "past the launch");
}

/// From the air: a Bomber based six tiles off pillages the pad before any
/// land unit walks.
#[test]
fn a_bomber_in_range_pillages_the_pad() {
    let (mut g, mut ai, _, pad) = fixture();
    ai.enable_war_raids_the_pads();
    g.found_city_for(0, (18, 12), None);
    let bomber = g.spawn_test_unit("bomber", 0, (18, 12));
    g.spawn_test_unit("scout", 0, (23, 12));
    let raiders = ai.plan_pad_raids(&mut g, 0, &plan(), &BTreeSet::new());
    assert!(raiders.contains(&bomber), "the Bomber flew: {raiders:?}");
    assert!(g.map.tiles[&pad].pillaged, "the pad is pillaged");
}
