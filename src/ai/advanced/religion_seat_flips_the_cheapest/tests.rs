use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Our founder and two rivals: rival 1 with three cities, one already ours,
/// rival 2 with three cities, none ours.
fn board(target: VictoryTarget) -> (Game, AdvancedAi, Vec<u32>, Vec<u32>) {
    let mut g = Game::new_full(3, 60, 24, 936_313, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 12), None);
    for x in [12, 20, 28] {
        g.found_city_for(1, (x, 6), None);
    }
    for x in [12, 20, 28] {
        g.found_city_for(2, (x, 18), None);
    }
    g.players[0].religion = Some("Buddhism".into());
    let first = g.player_city_ids(1);
    let second = g.player_city_ids(2);
    for cid in first.iter().chain(second.iter()) {
        let city = g.cities.get_mut(cid).unwrap();
        city.pop = 5;
        city.pressure.insert("Hinduism".into(), 500.0);
    }
    let converted = g.cities.get_mut(&first[0]).unwrap();
    converted.pressure.clear();
    converted.pressure.insert("Buddhism".into(), 500.0);
    g.current = 0;
    g.turn = 90;
    (g, AdvancedAi::targeting(target), first, second)
}

fn adjustment(g: &Game, ai: &AdvancedAi, city: u32) -> i32 {
    ai.religion_seat_flip_adjustment(g, 0, &g.cities[&city], "Buddhism")
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("religion-seat-flips-the-cheapest", |ai| {
        ai.religion_seat_flips_the_cheapest
    });
}

/// Off, on another lane, for another faith or for our own city it adds nothing.
#[test]
fn it_reads_only_a_founders_foreign_targets() {
    let (g, ai, first, _) = board(VictoryTarget::Religion);
    assert_eq!(adjustment(&g, &ai, first[1]), 0, "gene off");

    let (g, mut ai, first, _) = board(VictoryTarget::Culture);
    ai.enable_religion_seat_flips_the_cheapest();
    assert_eq!(adjustment(&g, &ai, first[1]), 0, "a Culture seat");

    let (g, mut ai, first, _) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_flips_the_cheapest();
    assert_eq!(
        ai.religion_seat_flip_adjustment(&g, 0, &g.cities[&first[1]], "Hinduism"),
        0,
        "another faith's spreader"
    );
    let own = g.player_city_ids(0)[0];
    assert_eq!(adjustment(&g, &ai, own), 0, "our own city");
}

/// The civilization nearer its majority comes first.
#[test]
fn the_nearer_majority_comes_first() {
    let (g, mut ai, first, second) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_flips_the_cheapest();
    assert_eq!(AdvancedAi::flips_needed(&g, 1, "Buddhism"), 1);
    assert_eq!(AdvancedAi::flips_needed(&g, 2, "Buddhism"), 2);
    assert!(adjustment(&g, &ai, first[1]) > adjustment(&g, &ai, second[1]));
}

/// A smaller city beats a larger one, and a capital loses its shipped premium.
#[test]
fn a_small_city_comes_before_a_large_one() {
    let (mut g, mut ai, first, _) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_flips_the_cheapest();
    g.cities.get_mut(&first[2]).unwrap().pop = 12;
    assert!(adjustment(&g, &ai, first[1]) > adjustment(&g, &ai, first[2]));
    let plain = adjustment(&g, &ai, first[1]);
    g.cities.get_mut(&first[1]).unwrap().is_capital = true;
    assert_eq!(adjustment(&g, &ai, first[1]), plain - SHIPPED_CAPITAL_BONUS);
}

/// A city no faith holds is the cheapest conversion on the board.
#[test]
fn a_city_without_a_majority_is_preferred() {
    let (mut g, mut ai, first, _) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_flips_the_cheapest();
    let held = adjustment(&g, &ai, first[1]);
    g.cities.get_mut(&first[1]).unwrap().pressure.clear();
    assert_eq!(adjustment(&g, &ai, first[1]), held + NO_MAJORITY_BONUS);
}
