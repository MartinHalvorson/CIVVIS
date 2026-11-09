use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::sync::Arc;

fn campaign() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 936_511, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for (owner, pos) in [(0, (3, 8)), (1, (14, 8)), (2, (24, 8))] {
        g.found_city_for(owner, pos, None);
    }
    let home = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Taoism".into(), 500.0);
    g.players[0].religion = Some("Taoism".into());
    g.players[0].holy_city = Some(home);
    let nearby = g.player_city_ids(1)[0];
    g.cities.get_mut(&nearby).unwrap().pop = 12;
    let unit = g.spawn_test_unit("missionary", 0, (14, 9));
    g.units.get_mut(&unit).unwrap().religion = Some("Taoism".into());
    g.current = 0;
    g.turn = 60;
    let ai = AdvancedAi::targeting(VictoryTarget::Religion);
    (g, ai, nearby, unit)
}

#[test]
fn majority_campaign_is_opt_in() {
    opt_in_off_in_both_controllers("religion-finishes-majorities", |ai| {
        ai.religion_finishes_majorities
    });
}

// Exercise the action consumer, not just the eligibility helper: the baseline
// spends a charge into the adjacent city, while the treatment keeps its charge
// and closes on the other major. The same controller drives Apostles' spreads.
#[test]
fn corps_passes_a_city_state_or_an_already_converted_major() {
    for minor in [true, false] {
        for kind in ["missionary", "apostle"] {
            let (mut g, mut ai, nearby, uid) = campaign();
            g.players[1].is_minor = minor;
            if !minor {
                Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Taoism".into());
            }
            g.units.get_mut(&uid).unwrap().kind = Name::new(kind);
            let start = g.units[&uid].pos;
            let charges = g.units[&uid].charges;
            let mut off = g.clone();
            assert!(ai.advanced_missionary_step(&mut off, 0, uid, true));
            assert_eq!(
                off.units[&uid].charges,
                charges - 1,
                "baseline spreads into {nearby}"
            );

            ai.enable_religion_finishes_majorities();
            let holdout = g.cities[&g.player_city_ids(2)[0]].pos;
            assert!(ai.advanced_missionary_step(&mut g, 0, uid, true));
            assert_eq!(
                g.units[&uid].charges, charges,
                "keep the charge for the holdout"
            );
            assert!(g.wdist(g.units[&uid].pos, holdout) < g.wdist(start, holdout));
        }
    }
}

#[test]
fn native_majority_stops_at_two_of_three_and_reopens_after_a_flip() {
    let (mut g, mut ai, city, _) = campaign();
    for pos in [(14, 15), (22, 15)] {
        g.found_city_for(1, pos, None);
    }
    let others: Vec<u32> = g
        .player_city_ids(1)
        .into_iter()
        .filter(|cid| *cid != city)
        .collect();
    ai.enable_religion_finishes_majorities();
    for cid in &others {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Taoism".into(), 500.0);
    }
    assert!(!ai.religion_campaign_needs_city(&g, 0, &g.cities[&city], "Taoism"));
    g.cities.get_mut(&others[0]).unwrap().pressure.clear();
    assert!(ai.religion_campaign_needs_city(&g, 0, &g.cities[&city], "Taoism"));
}

#[test]
fn host_majority_overrules_visible_cities_but_missing_host_data_is_not_conversion() {
    let (mut g, mut ai, city, _) = campaign();
    ai.enable_religion_finishes_majorities();
    // Our one visible rival city is converted, but hidden cities may not be.
    g.cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert("Taoism".into(), 500.0);
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .city_count = Some(8);
    assert!(ai.religion_campaign_needs_city(&g, 0, &g.cities[&city], "Taoism"));
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Catholicism".into());
    assert!(ai.religion_campaign_needs_city(&g, 0, &g.cities[&city], "Taoism"));
    // Conversely, the host can confirm a majority despite no converted city
    // being visible. Its empire-wide answer is authoritative.
    g.cities.get_mut(&city).unwrap().pressure.clear();
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Taoism".into());
    assert!(!ai.religion_campaign_needs_city(&g, 0, &g.cities[&city], "Taoism"));
}

#[test]
fn home_defense_and_other_lanes_keep_their_spreads() {
    let (mut g, mut ai, city, uid) = campaign();
    ai.enable_religion_finishes_majorities();
    g.players[1].is_minor = true;
    ai.retarget(VictoryTarget::Culture);
    let charges = g.units[&uid].charges;
    assert!(ai.advanced_missionary_step(&mut g, 0, uid, true));
    assert_eq!(g.units[&uid].charges, charges - 1);

    let (mut g, mut ai, city, uid) = campaign();
    ai.enable_religion_finishes_majorities();
    let home = g.player_city_ids(0)[0];
    let home_pos = g.cities[&home].pos;
    g.cities.get_mut(&home).unwrap().pressure.clear();
    g.units.get_mut(&uid).unwrap().pos = (home_pos.0, home_pos.1 + 1);
    let charges = g.units[&uid].charges;
    assert!(ai.advanced_missionary_step(&mut g, 0, uid, false));
    assert_eq!(
        g.units[&uid].charges,
        charges - 1,
        "home defense remains active"
    );
    assert!(g.city_religion(&g.cities[&home]).is_some());
    assert!(ai.religion_campaign_needs_city(&g, 0, &g.cities[&home], "Taoism"));
    assert!(ai.religion_campaign_needs_city(&g, 0, &g.cities[&city], "Catholicism"));
}
