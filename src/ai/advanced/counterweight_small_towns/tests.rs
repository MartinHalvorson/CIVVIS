use super::*;
use std::sync::Arc;

/// Live Emperor G211317Z near turn 95, reduced to five cities: Russia's
/// Orthodoxy (seat 1) holds three of ours — a large one beside the
/// Catholic source and two small towns farther off — a fresh town on no
/// faith is next in its path, and a Catholic Missionary (Poland's faith,
/// seat 2) stands beside the large one. We have no religion.
fn g211317_turn_95() -> (Game, AdvancedAi, Vec<u32>, u32) {
    let mut g = Game::new_full(4, 40, 24, 211_317, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for x in [2, 6, 12, 18, 24] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let set = |g: &mut Game, cid: u32, pop: i32, faith: Option<(&str, f64)>| {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = pop;
        city.pressure.clear();
        if let Some((faith, pressure)) = faith {
            city.pressure.insert(faith.into(), pressure);
        }
    };
    // Bogotá, the Catholic source; Maracaibo, Orthodox and large, beside
    // it; Cuenca and Guayaquil, small Orthodox towns; Quito, on no faith
    // with Orthodoxy closing (the mirror's 1-point marker).
    set(&mut g, cities[0], 9, Some(("Catholicism", 100.0)));
    set(&mut g, cities[1], 9, Some(("Orthodoxy", 100.0)));
    set(&mut g, cities[2], 2, Some(("Orthodoxy", 100.0)));
    set(&mut g, cities[3], 3, Some(("Orthodoxy", 100.0)));
    set(&mut g, cities[4], 1, Some(("Orthodoxy", 1.0)));
    g.players[1].religion = Some("Orthodoxy".into());
    g.players[2].religion = Some("Catholicism".into());
    g.players[3].religion = Some("Shinto".into());
    for (pid, faith) in [(1, "Orthodoxy"), (2, "Catholicism"), (3, "Shinto")] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, faith.into());
    }
    let missionary = g.spawn_unit("missionary", 0, (5, 8));
    {
        let unit = g.units.get_mut(&missionary).unwrap();
        unit.religion = Some("Catholicism".into());
        unit.charges = 3;
        unit.hp = 100;
    }
    let max_moves = g.unit_max_moves(missionary);
    g.units.get_mut(&missionary).unwrap().moves_left = max_moves;
    g.current = 0;
    g.turn = 95;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(g.city_religion(&g.cities[&cities[1]]), Some("Orthodoxy"));
    assert_eq!(g.city_religion(&g.cities[&cities[4]]), None);
    (g, ai, cities, missionary)
}

/// The threat is the rival faith holding our cities; never the spreader's
/// own faith, and nothing with the gene off or no rival city of ours.
#[test]
fn the_threat_is_the_faith_holding_our_cities() {
    let (g, mut ai, cities, _) = g211317_turn_95();
    assert_eq!(ai.small_town_threat(&g, 0, "Catholicism"), None, "off");
    ai.enable_counterweight_flips_the_small_towns();
    assert_eq!(
        ai.small_town_threat(&g, 0, "Catholicism").as_deref(),
        Some("Orthodoxy")
    );
    // A spreader of the threat's own faith answers the next faith in our
    // cities, Catholicism (Bogotá), not itself.
    assert_eq!(
        ai.small_town_threat(&g, 0, "Orthodoxy").as_deref(),
        Some("Catholicism")
    );
    let (mut quiet, mut ai, _, _) = g211317_turn_95();
    ai.enable_counterweight_flips_the_small_towns();
    for cid in &cities[1..4] {
        quiet.cities.get_mut(cid).unwrap().pressure.clear();
    }
    assert_eq!(ai.small_town_threat(&quiet, 0, "Catholicism"), None);
}

/// The adjustment reverses the twelve-a-citizen preference for our cities
/// off the spreader's faith, adds the veto bonus where the threat holds or
/// closes, and leaves the spreader's own city and foreign cities alone.
#[test]
fn small_towns_outrank_large_cities() {
    let (g, _, cities, _) = g211317_turn_95();
    let adjust = |cid: u32| {
        AdvancedAi::small_town_target_adjustment(
            &g,
            0,
            &g.cities[&cid],
            "Catholicism",
            Some("Orthodoxy"),
        )
    };
    assert_eq!(adjust(cities[0]), 0, "the spreader's own faith");
    assert_eq!(adjust(cities[1]), -2 * 9 * 12 + 30);
    assert_eq!(adjust(cities[2]), -2 * 2 * 12 + 30);
    assert_eq!(
        adjust(cities[4]),
        -2 * 12 + 30,
        "no faith, Orthodoxy closing"
    );
    assert_eq!(
        AdvancedAi::small_town_target_adjustment(&g, 0, &g.cities[&cities[1]], "Catholicism", None),
        0,
        "no threat"
    );
}

/// Off, the Missionary standing beside Maracaibo spends a charge on it, as
/// G211317Z did five times; on, it keeps its charges and walks toward
/// Cuenca, the smallest Orthodox town.
#[test]
fn the_missionary_walks_to_the_small_town() {
    let (mut g, ai, cities, missionary) = g211317_turn_95();
    assert!(ai.advanced_missionary_step(&mut g, 0, missionary, false));
    assert_eq!(
        g.units.get(&missionary).map(|unit| unit.charges),
        Some(2),
        "off: a charge into Maracaibo"
    );

    let (mut g, mut ai, cities_on, missionary) = g211317_turn_95();
    assert_eq!(cities, cities_on);
    ai.enable_counterweight_flips_the_small_towns();
    let cuenca = g.cities[&cities[2]].pos;
    let before = g.wdist(g.units[&missionary].pos, cuenca);
    assert!(ai.advanced_missionary_step(&mut g, 0, missionary, false));
    let unit = &g.units[&missionary];
    assert_eq!(unit.charges, 3, "on: no charge into Maracaibo");
    assert!(
        g.wdist(unit.pos, cuenca) < before,
        "on: toward Cuenca, from {before} to {}",
        g.wdist(unit.pos, cuenca)
    );
}
