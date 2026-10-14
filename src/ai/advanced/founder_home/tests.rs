use super::*;
use crate::game::install_test_district;
use std::sync::Arc;

/// A founder in live Emperor civvis-20261011T005957Z's shape near turn 60:
/// we founded Taoism in the Holy City (Holy Site and Shrine) and three of our
/// towns follow no faith, while a large foreign city of player 1, on no faith
/// either, stands a few tiles from our spreader.
fn founder_with_unconverted_towns() -> (Game, AdvancedAi, u32, Pos) {
    let mut g = Game::new_full(4, 40, 24, 372_102, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for x in [2, 8, 14, 20] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let holy = cities
        .iter()
        .copied()
        .find(|cid| g.cities[cid].pos.0 == 14)
        .unwrap();
    g.cities
        .get_mut(&holy)
        .unwrap()
        .pressure
        .insert("Taoism".into(), 100.0);
    install_test_district(&mut g, holy, "holy_site");
    g.cities.get_mut(&holy).unwrap().buildings = vec![crate::name!("shrine")];
    g.found_city_for(1, (14, 15), None);
    let abroad = g.player_city_ids(1)[0];
    g.cities.get_mut(&abroad).unwrap().pop = 12;
    let abroad_pos = g.cities[&abroad].pos;
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].religion = Some("Taoism".into());
    g.players[0].holy_city = Some(holy);
    g.players[1].religion = Some("Catholicism".into());
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Catholicism".into());
    g.current = 0;
    g.turn = 60;
    let uid = g.spawn_test_unit("missionary", 0, (14, 11));
    g.units.get_mut(&uid).unwrap().religion = Some("Taoism".into());
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    (g, ai, uid, abroad_pos)
}

fn nearest_own_city(g: &Game, pos: Pos) -> i32 {
    g.player_city_ids(0)
        .iter()
        .filter(|cid| g.city_religion(&g.cities[cid]) != Some("Taoism"))
        .map(|cid| g.wdist(pos, g.cities[cid].pos))
        .min()
        .unwrap()
}

/// Off, the large foreign city three tiles south outranks our towns on no
/// faith and the spreader walks toward it. On, three of our cities off our
/// faith keep it at home: it heads for one of our unconverted towns.
#[test]
fn a_founders_spreader_converts_our_towns_before_a_foreign_city() {
    for gene in [false, true] {
        let (mut g, mut ai, uid, abroad) = founder_with_unconverted_towns();
        if gene {
            ai.enable_founder_converts_its_own_cities();
        }
        let start = g.units[&uid].pos;
        let (abroad_before, home_before) = (g.wdist(start, abroad), nearest_own_city(&g, start));
        assert!(ai.advanced_missionary_step(&mut g, 0, uid, true));
        let now = g.units[&uid].pos;
        if gene {
            assert!(
                nearest_own_city(&g, now) < home_before,
                "on: the spreader closes on a town of ours ({home_before} -> {})",
                nearest_own_city(&g, now)
            );
        } else {
            assert!(
                g.wdist(now, abroad) < abroad_before,
                "off: the spreader closes on the foreign city ({abroad_before} -> {})",
                g.wdist(now, abroad)
            );
        }
    }
}

/// With every city of ours but one on our faith there is nothing to keep the
/// spreader home, and a faith we did not found is never kept.
#[test]
fn the_gene_needs_two_unconverted_cities_and_our_own_faith() {
    let (mut g, mut ai, _, _) = founder_with_unconverted_towns();
    ai.enable_founder_converts_its_own_cities();
    assert_eq!(ai.founder_keeps_spreaders_home(&g, 0, "Taoism"), Some(3));
    assert_eq!(ai.founder_keeps_spreaders_home(&g, 0, "Catholicism"), None);
    let mut converted = 0;
    for cid in g.player_city_ids(0) {
        if converted < 2 && g.city_religion(&g.cities[&cid]) != Some("Taoism") {
            g.cities
                .get_mut(&cid)
                .unwrap()
                .pressure
                .insert("Taoism".into(), 100.0);
            g.cities.get_mut(&cid).unwrap().atheist_pressure = 0.0;
            converted += 1;
        }
    }
    assert_eq!(ai.founder_keeps_spreaders_home(&g, 0, "Taoism"), None);
}
