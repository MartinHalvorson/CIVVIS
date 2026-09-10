use super::*;
use crate::game::HostUnitFacts;
use std::collections::BTreeSet;
use std::sync::Arc;

fn tour() -> (Game, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 40, 18, 774_4070, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }
    g.record_contact(0, 1);
    g.found_city_for(0, (2, 1), None);
    g.found_city_for(1, (1, 8), None);
    g.found_city_for(1, (20, 4), None);
    let cities = g.player_city_ids(1);
    let near = (3, 6);
    let far = (18, 4);
    let near_city = cities
        .iter()
        .copied()
        .find(|cid| g.cities[cid].pos == (1, 8))
        .unwrap();
    let far_city = cities
        .iter()
        .copied()
        .find(|cid| g.cities[cid].pos == (20, 4))
        .unwrap();
    g.map.tiles.get_mut(&near).unwrap().owner_city = Some(near_city);
    g.map.tiles.get_mut(&near).unwrap().district = Some(crate::name!("theater_square"));
    g.cities
        .get_mut(&near_city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), near);
    g.cities
        .get_mut(&near_city)
        .unwrap()
        .buildings
        .push(crate::name!("broadcast_center"));
    g.map.tiles.get_mut(&far).unwrap().owner_city = Some(far_city);
    g.map.tiles.get_mut(&far).unwrap().wonder = Some(crate::name!("pyramids"));
    let band = g.spawn_test_unit("rock_band", 0, (4, 4));
    g.units
        .get_mut(&band)
        .unwrap()
        .promotions
        .insert(crate::name!("roadies"));
    g.current = 0;
    (g, band, near, far)
}

#[test]
fn culture_tour_reaches_a_smaller_concert_before_chasing_a_distant_wonder() {
    let (mut g, band, near, far) = tour();
    assert!(g.rock_concert_ai_value(0, band, far) > g.rock_concert_ai_value(0, band, near));
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(ai.culture_concert_destination(&g, 0, band), Some(near));
    // Execute the route and concert, not just a ranking formula.
    for _ in 0..4 {
        if g.rock_concert_tourism(0, band).is_some() {
            break;
        }
        assert!(ai.advanced_rock_band_step(&mut g, 0, band));
    }
    assert_eq!(g.units[&band].pos, near);
    assert!(ai.advanced_rock_band_step(&mut g, 0, band));
    assert!(g
        .log
        .iter()
        .any(|(_, action)| matches!(action, Action::PerformConcert { unit } if *unit == band)));
}

#[test]
fn native_concert_menu_still_bounds_the_tour() {
    let (mut g, band, near, far) = tour();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    Arc::make_mut(&mut g.host_unit_facts).insert(
        band,
        HostUnitFacts {
            concert_plots: Some(BTreeSet::from([far])),
            ..Default::default()
        },
    );
    assert_eq!(ai.culture_concert_destination(&g, 0, band), Some(far));
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&band)
        .unwrap()
        .concert_plots = Some(BTreeSet::new());
    assert_eq!(ai.culture_concert_destination(&g, 0, band), None);
    assert!(!ai.advanced_rock_band_step(&mut g, 0, band));
    assert_ne!(g.units[&band].pos, near);
}

#[test]
fn adaptive_controller_preserves_its_existing_route() {
    let (mut g, band, _, _) = tour();
    let mut control = g.clone();
    let ai = AdvancedAi::new();
    assert_eq!(
        ai.advanced_rock_band_step(&mut g, 0, band),
        ai.base.rock_band_step(&mut control, 0, band)
    );
    assert_eq!(g.units[&band].pos, control.units[&band].pos);
    let (mut targeted, band, _, _) = tour();
    assert!(
        AdvancedAi::targeting(VictoryTarget::Culture).advanced_rock_band_step(
            &mut targeted,
            0,
            band
        )
    );
    assert_ne!(targeted.units[&band].pos, control.units[&band].pos);
}
