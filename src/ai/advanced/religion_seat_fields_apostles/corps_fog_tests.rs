use super::*;
use crate::game::ObservedPublicEmpireStats;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(5, 72, 32, 936_315, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 16), None);
    g.players[0].religion = Some("Buddhism".into());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.enable_religion_seat_fields_apostles();
    (g, ai)
}

fn city(g: &mut Game, owner: usize, position: Pos, faith: &str) -> u32 {
    g.found_city_for(owner, position, None);
    let cid = *g.player_city_ids(owner).last().unwrap();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert(faith.into(), 500.0);
    cid
}

fn public_cities(g: &mut Game, owner: usize, count: Option<usize>) {
    Arc::make_mut(&mut g.observed_public_empire_stats).insert(
        owner,
        ObservedPublicEmpireStats {
            city_count: count,
            ..Default::default()
        },
    );
}

#[test]
fn the_native_russia_workload_includes_public_cities_without_locations() {
    let (mut g, ai) = board();
    // The native t100 frame has seven located Japanese cities, two following
    // our faith; three located Polish cities; and no located Colombian or
    // Ottoman cities. Their public city totals are 7, 5, 10 and 9.
    for index in 0..7 {
        city(
            &mut g,
            1,
            (12 + index * 6, 4),
            if index < 2 { "Buddhism" } else { "Hinduism" },
        );
    }
    for index in 0..3 {
        city(&mut g, 4, (12 + index * 6, 24), "Hinduism");
    }
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 8);
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 4);
    let located = g.cities.len();
    for (owner, count) in [(1, 7), (2, 10), (3, 9), (4, 5)] {
        public_cities(&mut g, owner, Some(count));
    }
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 29);
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 6);
    assert_eq!(
        g.cities.len(),
        located,
        "a public count creates no hidden city"
    );
    assert!(g.player_city_ids(2).is_empty());
    assert!(g.player_city_ids(3).is_empty());
}

#[test]
fn missing_and_stale_public_counts_keep_the_known_city_workload() {
    let (mut g, ai) = board();
    for index in 0..3 {
        city(&mut g, 1, (12 + index * 6, 4), "Hinduism");
    }
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 3);
    for count in [None, Some(0), Some(1), Some(3)] {
        public_cities(&mut g, 1, count);
        assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 3);
        assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 3);
    }
}

#[test]
fn confirmed_converted_majors_do_not_add_unseen_conversion_work() {
    let (mut g, ai) = board();
    let cid = city(&mut g, 1, (12, 4), "Hinduism");
    public_cities(&mut g, 1, Some(30));
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Buddhism".into());
    assert_eq!(
        AdvancedAi::unconverted_major_cities(&g, 0),
        1,
        "the existing known holdout contribution is retained"
    );
    g.cities.get_mut(&cid).unwrap().pressure.clear();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("Buddhism".into(), 500.0);
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 0);
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2);
}

#[test]
fn unresolved_majority_does_not_mean_the_rival_is_converted() {
    let (mut g, ai) = board();
    public_cities(&mut g, 1, Some(20));
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 20);
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 6);
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Hinduism".into());
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 20);
}

#[test]
fn our_empire_dead_majors_minors_and_barbarians_add_no_work() {
    let (mut g, ai) = board();
    for owner in 0..5 {
        public_cities(&mut g, owner, Some(100));
    }
    g.players[1].alive = false;
    g.players[2].is_minor = true;
    g.players[3].is_barbarian = true;
    g.players[4].alive = false;
    assert_eq!(AdvancedAi::unconverted_major_cities(&g, 0), 0);
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2);
}

#[test]
fn public_counts_do_not_bypass_the_corps_treatment_target_or_posture() {
    let (mut g, mut ai) = board();
    public_cities(&mut g, 1, Some(30));
    ai.disable_religion_seat_fields_apostles();
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2);
    ai.enable_religion_seat_fields_apostles();
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, false, 0), 0);
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_religion_seat_fields_apostles();
    assert_eq!(culture.religion_seat_apostle_cap(&g, 0, true, 2), 2);
    g.players[0].religion = None;
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2);
}

#[test]
fn a_reverted_native_majority_restores_the_public_workload() {
    let (mut g, ai) = board();
    public_cities(&mut g, 1, Some(20));
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Buddhism".into());
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2);
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Hinduism".into());
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 6);
}
