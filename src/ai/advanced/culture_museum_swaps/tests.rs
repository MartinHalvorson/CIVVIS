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
    assert_eq!(score(&[Some(&a[0]), Some(&a[1]), Some(&a[2])]), 4);
    assert_eq!(score(&[Some(&b[0]), Some(&a[1]), Some(&a[2])]), 5);
}
#[test]
fn art_theme_uses_object_type_and_three_distinct_artists() {
    let a = work("A", 0);
    let b = work("B", 1);
    let mut c = work("C", 2);
    assert_eq!(score(&[Some(&a), Some(&b), Some(&c)]), 12);
    c.object = "GREATWORKOBJECT_PORTRAIT".into();
    assert_eq!(score(&[Some(&a), Some(&b), Some(&c)]), 6);
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

fn evacuation_fixture() -> (Game, StateSnapshot) {
    let g = Game::new_full(2, 18, 10, 903, 500, 0, false);
    let writing = StateGreatWork {
        kind: "GREATWORK_CERVANTES_1".into(),
        object: "GREATWORKOBJECT_WRITING".into(),
        building: "BUILDING_AMPHITHEATER".into(),
        slot: 0,
        ..Default::default()
    };
    let source = StateCity {
        id: 10,
        damage: 50.0,
        loyalty: 100.0,
        buildings: vec!["BUILDING_AMPHITHEATER".into()],
        pillaged_buildings: Some(vec![]),
        great_works: Some(vec![writing]),
        ..Default::default()
    };
    let mut dest = source.clone();
    dest.id = 20;
    dest.damage = 0.0;
    dest.great_works = Some(vec![]);
    let state = StateSnapshot {
        cities: vec![source, dest],
        rivals: vec![crate::mirror::StateRival {
            at_war: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    (g, state)
}

#[test]
fn damaged_wartime_city_proposes_writing_to_known_empty_safe_slot() {
    let (g, s) = evacuation_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(
        ai.native_writing_evacuations(&g, 0, &s),
        vec![
            (10, "GREATWORK_CERVANTES_1".into(), 20, 0),
            (10, "GREATWORK_CERVANTES_1".into(), 20, 1),
        ]
    );
}

#[test]
fn evacuation_requires_native_danger_safe_inventory_and_culture_lane() {
    let (g, s) = evacuation_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let mut controls = Vec::new();
    let mut x = s.clone();
    x.rivals.clear();
    controls.push(x);
    let mut x = s.clone();
    x.cities[0].damage = 0.0;
    controls.push(x);
    let mut x = s.clone();
    x.cities[0].damage = f64::NAN;
    controls.push(x);
    let mut x = s.clone();
    x.cities[1].damage = 1.0;
    controls.push(x);
    let mut x = s.clone();
    x.cities[1].loyalty = 50.0;
    controls.push(x);
    let mut x = s.clone();
    x.cities[1].great_works = None;
    controls.push(x);
    let mut x = s.clone();
    x.cities[0].pillaged_buildings = None;
    controls.push(x);
    let mut x = s.clone();
    x.cities[1].pillaged_buildings = Some(vec!["BUILDING_AMPHITHEATER".into()]);
    controls.push(x);
    let mut x = s.clone();
    x.cities[0].great_works.as_mut().unwrap()[0].object = "GREATWORKOBJECT_ARTIFACT".into();
    controls.push(x);
    let mut x = s.clone();
    x.cities[1].great_works = Some(vec![
        StateGreatWork {
            slot: 0,
            ..s.cities[0].great_works.as_ref().unwrap()[0].clone()
        },
        StateGreatWork {
            slot: 1,
            ..s.cities[0].great_works.as_ref().unwrap()[0].clone()
        },
    ]);
    controls.push(x);
    for x in controls {
        assert!(ai.native_writing_evacuations(&g, 0, &x).is_empty());
    }
    assert!(AdvancedAi::targeting(VictoryTarget::Science)
        .native_writing_evacuations(&g, 0, &s)
        .is_empty());
    let mut ai = ai;
    ai.lane_lost = true;
    assert!(ai.native_writing_evacuations(&g, 0, &s).is_empty());
}
