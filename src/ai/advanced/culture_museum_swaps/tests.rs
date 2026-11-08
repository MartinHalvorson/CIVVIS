use super::*;
use crate::mirror::StateCity;
fn work(artist: &str, slot: i32) -> StateGreatWork {
    StateGreatWork {
        kind: format!("{artist}_{slot}"),
        object: "GREATWORKOBJECT_LANDSCAPE".into(),
        era: None,
        creator: artist.into(),
        building: "BUILDING_MUSEUM_ART".into(),
        slot,
    }
}
fn city(id: i64, artist: &str) -> StateCity {
    StateCity {
        id,
        buildings: vec!["BUILDING_MUSEUM_ART".into()],
        great_works: Some((0..3).map(|s| work(artist, s)).collect()),
        ..Default::default()
    }
}
fn fixture() -> (Game, StateSnapshot) {
    (
        Game::new_full(2, 18, 10, 903, 500, 0, false),
        StateSnapshot {
            cities: vec![city(10, "A"), city(20, "B")],
            ..Default::default()
        },
    )
}
#[test]
fn same_artist_museums_propose_a_real_inventory_gain() {
    let (g, s) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let swaps = ai.native_art_museum_swaps(&g, 0, &s);
    assert_eq!(swaps.len(), 9);
    assert_eq!(swaps[0], (10, "A_0".into(), 20, 0));
    let a = s.cities[0].great_works.as_ref().unwrap();
    let b = s.cities[1].great_works.as_ref().unwrap();
    assert_eq!(score(&[Some(&a[0]), Some(&a[1]), Some(&a[2])]), 5);
    assert_eq!(score(&[Some(&b[0]), Some(&a[1]), Some(&a[2])]), 7);
}
#[test]
fn art_theme_uses_object_type_and_three_distinct_artists() {
    let a = work("A", 0);
    let b = work("B", 1);
    let mut c = work("C", 2);
    assert_eq!(score(&[Some(&a), Some(&b), Some(&c)]), 18);
    c.object = "GREATWORKOBJECT_PORTRAIT".into();
    assert_eq!(score(&[Some(&a), Some(&b), Some(&c)]), 9);
}
#[test]
fn missing_or_unsupported_inventory_is_not_a_proposal() {
    let (g, mut s) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    s.cities[0].great_works = None;
    assert!(ai.native_art_museum_swaps(&g, 0, &s).is_empty());
    s.cities[0] = city(10, "A");
    s.cities[0].great_works.as_mut().unwrap()[0].creator.clear();
    assert!(ai.native_art_museum_swaps(&g, 0, &s).is_empty());
    s.cities[0] = city(10, "A");
    s.cities[0].great_works.as_mut().unwrap()[0].object = "GREATWORKOBJECT_RELIGIOUS".into();
    assert!(ai.native_art_museum_swaps(&g, 0, &s).is_empty());
    s.cities[0] = city(10, "A");
    s.cities[0].great_works.as_mut().unwrap()[0].slot = 1;
    assert!(ai.native_art_museum_swaps(&g, 0, &s).is_empty());
}
#[test]
fn other_lanes_and_lost_culture_lane_do_not_rearrange() {
    let (g, s) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(ai.native_art_museum_swaps(&g, 0, &s).is_empty());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.lane_lost = true;
    assert!(ai.native_art_museum_swaps(&g, 0, &s).is_empty());
}
