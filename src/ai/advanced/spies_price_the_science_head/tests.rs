use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::{install_test_district, HostMenuEntry};
use std::sync::Arc;

fn spy() -> Item {
    Item::Unit {
        unit: crate::name!("spy"),
    }
}
fn quote(g: &mut Game, cid: u32, turns: f64) {
    Arc::make_mut(&mut g.host_buildable)
        .entry(cid)
        .or_default()
        .insert(
            Game::production_block_key(&spy()),
            HostMenuEntry {
                cost: Some(150.0),
                turns: Some(turns),
            },
        );
}
fn fixture() -> (Game, Vec<u32>, AdvancedAi) {
    let (mut g, cities) = producer_empire("SPYPRICESCIENCE", 94_402, 2);
    for cid in &cities {
        g.cities.get_mut(cid).unwrap().queue.clear();
        quote(&mut g, *cid, 50.0);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_spies_price_the_science_head();
    (g, cities, ai)
}
#[test]
fn off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "spies-price-the-science-head",
        |ai| ai.spies_price_the_science_head,
    );
}
#[test]
fn native_remaining_quote_beats_raw_production_and_off_keeps_old_order() {
    let (mut g, cities, mut ai) = fixture();
    quote(&mut g, cities[0], 12.0);
    quote(&mut g, cities[1], 5.0);
    let original = vec![(40.0, cities[0]), (20.0, cities[1])];
    let mut ranked = original.clone();
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked[0].1, cities[1]);
    ai.disable_spies_price_the_science_head();
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked, original);
}
#[test]
fn equal_time_spares_university_but_shorter_time_keeps_science_producer() {
    let (mut g, cities, ai) = fixture();
    g.cities.get_mut(&cities[0]).unwrap().queue = vec![Item::Building {
        building: crate::name!("university"),
    }];
    quote(&mut g, cities[0], 5.0);
    quote(&mut g, cities[1], 5.0);
    let mut ranked = vec![(40.0, cities[0]), (20.0, cities[1])];
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked[0].1, cities[1]);
    quote(&mut g, cities[0], 4.0);
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked[0].1, cities[0]);
}
#[test]
fn missing_invalid_or_nonpositive_quote_keeps_the_entire_original_ranking() {
    let (mut g, cities, ai) = fixture();
    quote(&mut g, cities[0], 12.0);
    for turns in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        quote(&mut g, cities[1], turns);
        let mut ranked = vec![(20.0, cities[1]), (40.0, cities[0])];
        ai.rank_spy_producers(&g, &mut ranked);
        assert_eq!(ranked[0].1, cities[0]);
    }
    Arc::make_mut(&mut g.host_buildable).remove(&cities[1]);
    let mut ranked = vec![(20.0, cities[1]), (40.0, cities[0])];
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked[0].1, cities[0]);
}
#[test]
fn other_assignments_keep_raw_order_and_complete_ties_are_deterministic() {
    let (mut g, cities, _) = fixture();
    quote(&mut g, cities[0], 12.0);
    quote(&mut g, cities[1], 5.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_spies_price_the_science_head();
    let mut ranked = vec![(20.0, cities[1]), (40.0, cities[0])];
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked[0].1, cities[0]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_spies_price_the_science_head();
    quote(&mut g, cities[0], 5.0);
    let mut ranked = vec![(20.0, cities[1]), (20.0, cities[0])];
    ai.rank_spy_producers(&g, &mut ranked);
    assert_eq!(ranked[0].1, cities[0].min(cities[1]));
}
#[test]
fn both_claims_use_the_same_earliest_native_completion() {
    for denial in [false, true] {
        let (mut g, cities, mut ai) = fixture();
        let site = g
            .map
            .tiles
            .values()
            .filter(|t| {
                g.rules.is_passable(t)
                    && !g.rules.is_water(t)
                    && t.owner_city.is_none()
                    && g.cities.values().all(|c| g.wdist(c.pos, t.pos) >= 5)
            })
            .map(|t| t.pos)
            .min()
            .unwrap();
        g.found_city_for(1, site, None);
        let rival = g.player_city_ids(1)[0];
        install_test_district(&mut g, rival, "campus");
        install_test_district(&mut g, rival, "spaceport");
        g.record_contact(0, 1);
        g.players[0].explored.insert(site);
        let district_tiles: Vec<_> = g.cities[&rival]
            .districts
            .iter()
            .map(|(_, pos)| pos)
            .collect();
        g.players[0].explored.extend(district_tiles);
        assert_eq!(
            AdvancedAi::steal_targets(&g, 0).len(),
            0,
            "rival technologies are added below"
        );
        for tech in ["education", "astronomy", "mathematics", "printing"] {
            g.players[1].techs.insert(Name::new(tech));
        }
        for civic in ["diplomatic_service", "nationalism"] {
            g.players[0].civics.insert(Name::new(civic));
        }
        assert!(
            AdvancedAi::steal_targets(&g, 0).contains(&1),
            "fixture has a visible Campus and technologies to steal"
        );
        assert!(g.spy_capacity(0) > 0);
        assert!(g.can_produce(0, cities[1], &spy()));
        quote(&mut g, cities[0], 12.0);
        quote(&mut g, cities[1], 5.0);
        let mut plan = ai.assess(&g, 0);
        plan.strategy = GrandStrategy::Science;
        plan.threatened_city = None;
        if denial {
            ai.enable_science_denial_trains_spies();
            for project in ["launch_earth_satellite", "launch_moon_landing"] {
                g.players[1].science_projects.insert(project.to_string());
            }
            ai.claim_queues_for_denial_spies(&mut g, 0, &plan);
        } else {
            ai.enable_spies_steal_the_lead();
            ai.claim_queues_for_steal_spies(&mut g, 0, &plan);
        }
        assert_eq!(
            g.cities[&cities[1]].queue.first(),
            Some(&spy()),
            "denial {denial}"
        );
        assert!(g.cities[&cities[0]].queue.is_empty());
    }
}
