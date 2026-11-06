use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::{install_test_district, HostMenuEntry};
use std::sync::Arc;

fn royal() -> Item {
    Item::Building {
        building: crate::name!("royal_society"),
    }
}
fn quote(g: &mut Game, cid: u32, turns: f64) {
    Arc::make_mut(&mut g.host_buildable)
        .entry(cid)
        .or_default()
        .insert(
            Game::production_block_key(&royal()),
            HostMenuEntry {
                cost: Some(220.0),
                turns: Some(turns),
            },
        );
}
fn fixture(v2: bool) -> (Game, Vec<u32>, AdvancedAi) {
    let (mut g, cities) = producer_empire("ROYALPLAZALAUNCH", 94_403, 2);
    g.turn = 150;
    g.world_era = 4;
    g.players[0].techs.insert(crate::name!("rocketry"));
    g.players[0].civics.insert(crate::name!("class_struggle"));
    g.players[0].government = Some("communism".to_string());
    install_test_district(&mut g, cities[0], "spaceport");
    install_test_district(&mut g, cities[1], "government_plaza");
    g.cities.get_mut(&cities[1]).unwrap().buildings.extend([
        crate::name!("ancestral_hall"),
        crate::name!("intelligence_agency"),
    ]);
    quote(&mut g, cities[1], 9.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_science_victory_drive();
    if v2 {
        ai.enable_science_victory_drive_2();
    }
    ai.maintain_science_drive(&g, 0);
    assert_eq!(ai.science_drive_launch_city(&g, 0), Some(cities[0]));
    (g, cities, ai)
}

#[test]
fn off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "royal-society-serves-the-launch",
        |ai| ai.royal_society_serves_the_launch,
    );
}

#[test]
fn both_drives_extend_the_existing_credit_to_the_plaza_city() {
    for v2 in [false, true] {
        let (g, cities, mut ai) = fixture(v2);
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
            0.0
        );
        ai.enable_royal_society_serves_the_launch();
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
            400.0
        );
        ai.disable_royal_society_serves_the_launch();
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
            0.0
        );
    }
}

#[test]
fn the_old_launch_city_credit_is_not_double_paid() {
    for v2 in [false, true] {
        let (g, cities, mut ai) = fixture(v2);
        ai.science_drive.as_mut().unwrap().launch_city = Some(cities[1]);
        let before = ai.science_drive_production_bonus(&g, 0, cities[1], &royal());
        assert_eq!(before, 400.0);
        ai.enable_royal_society_serves_the_launch();
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
            before
        );
    }
}

#[test]
fn missing_invalid_or_nonpositive_native_quotes_keep_the_old_value() {
    let (mut g, cities, mut ai) = fixture(false);
    ai.enable_royal_society_serves_the_launch();
    for turns in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        quote(&mut g, cities[1], turns);
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
            0.0
        );
    }
    Arc::make_mut(&mut g.host_buildable).clear();
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
}

#[test]
fn no_drive_other_lanes_other_items_or_foreign_cities_gain_no_credit() {
    let (mut g, cities, mut ai) = fixture(false);
    ai.enable_royal_society_serves_the_launch();
    ai.victory_target = Some(VictoryTarget::Culture);
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
    ai.victory_target = Some(VictoryTarget::Science);
    let lab = Item::Building {
        building: crate::name!("research_lab"),
    };
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &lab),
        0.0
    );
    g.cities.get_mut(&cities[1]).unwrap().owner = 1;
    assert_eq!(
        ai.royal_society_serves_the_launch_bonus(&g, 0, cities[1], &royal()),
        None
    );
    g.cities.get_mut(&cities[1]).unwrap().owner = 0;
    ai.science_drive = None;
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
}

#[test]
fn a_royal_society_without_a_plaza_or_pad_commitment_keeps_the_old_value() {
    let (mut g, cities, mut ai) = fixture(false);
    ai.enable_royal_society_serves_the_launch();
    let pad = g
        .cities
        .get_mut(&cities[0])
        .unwrap()
        .districts
        .remove(crate::name!("spaceport"))
        .unwrap()[0];
    g.map.tiles.get_mut(&pad).unwrap().district = None;
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
    g.cities.get_mut(&cities[0]).unwrap().queue = vec![Item::District {
        district: crate::name!("spaceport"),
        pos: pad,
    }];
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        400.0
    );
    g.cities
        .get_mut(&cities[1])
        .unwrap()
        .districts
        .remove(crate::name!("government_plaza"));
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
}

#[test]
fn support_must_finish_before_the_known_horizon() {
    let (mut g, cities, mut ai) = fixture(false);
    ai.enable_royal_society_serves_the_launch();
    g.max_turns = g.turn + 9;
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
    g.max_turns = 0;
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        400.0
    );
    g.players[0].met.insert(1);
    g.players[1]
        .science_projects
        .insert("exoplanet_expedition".into());
    g.players[1].exoplanet_distance = crate::game::EXOPLANET_DESTINATION - 1.0;
    assert_eq!(
        ai.science_drive_production_bonus(&g, 0, cities[1], &royal()),
        0.0
    );
}
