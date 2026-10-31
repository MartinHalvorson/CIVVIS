use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

/// 224544Z's shape on open grassland: two majors at peace, seat 1's city at
/// (26, 12) stands a Spaceport at (24, 12) we have seen, its Moon landed and
/// its Exoplanet not launched, and our army past 0.8 times its power.
fn fixture(target: VictoryTarget) -> (Game, AdvancedAi) {
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
    g.found_city_for(0, (14, 16), None);
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
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    for at in [(14, 10), (14, 12), (14, 14), (15, 11)] {
        g.spawn_test_unit("cavalry", 0, at);
    }
    for at in [(30, 10), (30, 12), (30, 14), (31, 11), (31, 13)] {
        g.spawn_test_unit("cavalry", 1, at);
    }
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 180;
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_launcher_war_ignores_the_edge();
    (g, ai)
}

#[test]
fn launcher_war_serves_every_lane_is_opt_in() {
    opt_in_off_in_both_controllers("launcher-war-serves-every-lane", |ai| {
        ai.launcher_war_serves_every_lane
    });
}

/// 224544Z: a Culture seat past 0.8 times the racer's steady power names the
/// racer past its Moon as a Conquest counter, under the gene only.
#[test]
fn a_culture_seat_counters_the_racer_past_its_moon_with_war() {
    let (mut g, mut ai) = fixture(VictoryTarget::Culture);
    assert!(
        ai.launcher_war_opens(&g, 0, 1),
        "fixture: the launcher war opens"
    );
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "off");
    ai.enable_launcher_war_serves_every_lane();
    assert_eq!(
        ai.launcher_lane_counter(&g, 0),
        Some((1, GrandStrategy::Conquest))
    );
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((1, GrandStrategy::Conquest)),
        "the counter the plan reads"
    );
    // Short of the Moon, or past the launch: no war.
    g.players[1].science_projects.remove("launch_moon_landing");
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "short of the Moon");
    g.players[1]
        .science_projects
        .insert("launch_moon_landing".to_string());
    g.players[1]
        .science_projects
        .insert("exoplanet_expedition".to_string());
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "past the launch");
}

/// 173341Z and 190332Z were Diplomatic seats; the counter is the same.
#[test]
fn a_diplomatic_seat_counters_the_racer_too() {
    let (g, mut ai) = fixture(VictoryTarget::Diplomacy);
    ai.enable_launcher_war_serves_every_lane();
    assert_eq!(
        ai.launcher_lane_counter(&g, 0),
        Some((1, GrandStrategy::Conquest))
    );
}

/// The launcher war's own gates hold: its edge gene, 0.8 times the racer's
/// power, a standing pad. A Domination seat keeps its own launch clocks.
#[test]
fn the_launcher_wars_gates_hold() {
    let (mut g, mut ai) = fixture(VictoryTarget::Culture);
    ai.enable_launcher_war_serves_every_lane();
    ai.disable_launcher_war_ignores_the_edge();
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "needs the edge gene");
    ai.enable_launcher_war_ignores_the_edge();
    let pad: Pos = (24, 12);
    g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "no standing pad");
    g.map.tiles.get_mut(&pad).unwrap().pillaged = false;
    for at in [(32, 10), (32, 12), (32, 14), (33, 11), (33, 13), (34, 12)] {
        g.spawn_test_unit("cavalry", 1, at);
    }
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "under 0.8 times");

    // 190332Z's ally Korea started projects while allied: a friend is no
    // target.
    let (mut g, mut ai) = fixture(VictoryTarget::Diplomacy);
    ai.enable_launcher_war_serves_every_lane();
    let until = g.turn + 30;
    g.players[0].friends_until.insert(1, until);
    g.players[1].friends_until.insert(0, until);
    assert_eq!(ai.launcher_lane_counter(&g, 0), None, "a friend");

    let (g, mut ai) = fixture(VictoryTarget::Domination);
    ai.enable_launcher_war_serves_every_lane();
    assert_eq!(
        ai.launcher_lane_counter(&g, 0),
        None,
        "Domination keeps its own"
    );
}
