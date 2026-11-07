use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::{install_test_district, LiveGreatPersonActivationNeed};

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn fixture() -> (Game, u32, u32, BasicAi) {
    let (mut g, cities) = producer_empire("CULTURALACTIVATION", 471_701, 2);
    g.players[0]
        .civics
        .extend(["drama_poetry", "humanism"].map(Name::new));
    for cid in &cities {
        g.cities.get_mut(cid).unwrap().pop = 10;
        g.cities.get_mut(cid).unwrap().queue = vec![building("walls")];
    }
    for cid in &cities[..2] {
        install_test_district(&mut g, *cid, "theater_square");
        let city = g.cities.get_mut(cid).unwrap();
        city.buildings.push(crate::name!("amphitheater"));
        city.queue.clear();
    }
    g.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: "musician".into(),
            required_district: Some("theater_square".into()),
            ..Default::default()
        });
    let mut ai = BasicAi::new();
    ai.activation_keeps_its_building = true;
    (g, cities[0], cities[1], ai)
}

#[test]
fn a_prepared_museum_waits_for_radio_without_forcing_more_museums() {
    let (g, first, other, ai) = fixture();
    for museum in ["art_museum", "archaeological_museum"] {
        let mut g = g.clone();
        g.cities
            .get_mut(&first)
            .unwrap()
            .buildings
            .push(Name::new(museum));
        assert!(g.can_produce(0, other, &building("art_museum")));
        assert!(ai
            .live_great_person_cultural_item(&g, 0, other, "music")
            .is_none());
        assert!(ai.live_great_person_activation_item(&g, 0, other).is_none());
        g.cities.get_mut(&other).unwrap().queue = vec![building("art_museum")];
        assert!(ai
            .live_great_person_activation_resume_item(&g, 0, other)
            .is_none());
        assert_eq!(
            BasicAi::live_great_person_tech_goal(&g, 0).as_deref(),
            Some("radio")
        );
    }
}

#[test]
fn a_musician_still_prepares_its_first_museum_before_radio() {
    let (g, _, other, ai) = fixture();
    assert_eq!(
        ai.live_great_person_cultural_item(&g, 0, other, "music"),
        Some(building("art_museum"))
    );
}

#[test]
fn an_unneeded_queued_museum_does_not_block_the_available_broadcast_center() {
    let (mut g, first, other, ai) = fixture();
    g.players[0].techs.insert(crate::name!("radio"));
    g.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .push(crate::name!("art_museum"));
    g.cities.get_mut(&other).unwrap().queue = vec![building("art_museum")];
    assert!(g.can_produce(0, first, &building("broadcast_center")));
    assert_eq!(
        ai.live_great_person_cultural_item(&g, 0, first, "music"),
        Some(building("broadcast_center"))
    );
    assert_eq!(
        ai.live_great_person_activation_item(&g, 0, first),
        Some(building("broadcast_center"))
    );
}

#[test]
fn another_music_slot_can_prepare_a_museum_after_the_first_center_is_complete() {
    let (g, first, other, ai) = fixture();
    for center in ["broadcast_center", "film_studio"] {
        let mut g = g.clone();
        let city = g.cities.get_mut(&first).unwrap();
        city.buildings
            .extend([crate::name!("art_museum"), Name::new(center)]);
        assert_eq!(
            ai.live_great_person_cultural_item(&g, 0, other, "music"),
            Some(building("art_museum"))
        );
    }
}

#[test]
fn a_pillaged_museum_or_theater_is_not_a_prepared_music_path() {
    let (g, first, other, ai) = fixture();
    for district_pillaged in [false, true] {
        let mut g = g.clone();
        g.cities
            .get_mut(&first)
            .unwrap()
            .buildings
            .push(crate::name!("art_museum"));
        if district_pillaged {
            let pos = g.cities[&first]
                .districts
                .iter()
                .find_map(|(district, pos)| {
                    (g.district_family(*district) == "theater_square").then_some(*pos)
                })
                .unwrap();
            g.map.tiles.get_mut(&pos).unwrap().pillaged = true;
        } else {
            g.cities
                .get_mut(&first)
                .unwrap()
                .pillaged_buildings
                .insert(crate::name!("art_museum"));
        }
        assert_eq!(
            ai.live_great_person_cultural_item(&g, 0, other, "music"),
            Some(building("art_museum"))
        );
    }
}

#[test]
fn an_artist_can_still_open_art_slots_when_a_music_path_is_prepared() {
    let (mut g, first, other, ai) = fixture();
    g.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .push(crate::name!("art_museum"));
    g.players[0].live_great_person_activation_needs[0].kind = "artist".into();
    assert_eq!(
        ai.live_great_person_cultural_item(&g, 0, other, "art"),
        Some(building("art_museum"))
    );
    assert_eq!(
        ai.live_great_person_activation_item(&g, 0, other),
        Some(building("art_museum"))
    );
}
