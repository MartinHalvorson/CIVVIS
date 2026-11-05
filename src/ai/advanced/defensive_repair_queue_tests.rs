use super::*;

fn fixture() -> (Game, u32, Item) {
    let mut g = Game::new_full(2, 40, 24, 936005, 200, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let city = g.found_city_for(0, (8, 12), None);
    g.players[0].techs.insert(crate::name!("masonry"));
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 80;
    let pos = (9, 12);
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.owner_city = Some(city);
    tile.district = Some(crate::name!("campus"));
    tile.pillaged = false;
    let c = g.cities.get_mut(&city).unwrap();
    c.hp = 170;
    c.buildings.push(crate::name!("library"));
    c.pillaged_buildings.insert(crate::name!("library"));
    let repair = Item::Repair {
        repair: crate::name!("library"),
        pos,
    };
    assert!(g.can_produce(0, city, &repair));
    g.apply(
        0,
        &Action::Produce {
            city,
            item: repair.clone(),
        },
    )
    .unwrap();
    (g, city, repair)
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

#[test]
fn emergency_defense_can_reclaim_an_economic_repair_queue() {
    let (mut g, city, repair) = fixture();
    let mut calm = g.clone();
    calm.at_war.clear();
    calm.cities.get_mut(&city).unwrap().hp = 200;
    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    assert!(ai
        .redirect_unsafe_city_queue_for_defense(&mut calm, 0, None)
        .is_none());
    assert_eq!(calm.cities[&city].queue.first(), Some(&repair));
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city));
    assert_eq!(g.cities[&city].queue.first(), Some(&walls()));
}

#[test]
fn economic_repairs_cannot_replace_an_already_confirmed_defense() {
    let (mut g, city, repair) = fixture();
    g.cities.get_mut(&city).unwrap().queue.clear();
    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city));
    assert_eq!(claim, Some((city, walls())));
    g.apply(0, &Action::Produce { city, item: repair }).unwrap();
    g.at_war.clear();
    ai.reapply_confirmed_defense_queue(&mut g, 0, claim.as_ref());
    assert_eq!(g.cities[&city].queue.first(), Some(&walls()));
}

#[test]
fn defense_queue_classification_keeps_fortifications_and_local_defenders() {
    let (mut g, _, repair) = fixture();
    assert!(!AdvancedAi::active_queue_answers_siege(&g, &repair));
    assert!(
        AdvancedAi::active_queue_is_defensive(&g, &repair),
        "the pre-war repair exemption remains conservative"
    );
    let pos = (9, 12);
    let district = Item::Repair {
        repair: crate::name!("district"),
        pos,
    };
    assert!(!AdvancedAi::active_queue_answers_siege(&g, &district));
    g.map.tiles.get_mut(&pos).unwrap().district = Some(crate::name!("encampment"));
    assert!(AdvancedAi::active_queue_answers_siege(&g, &district));
    for item in [
        walls(),
        Item::Project {
            project: crate::name!("repair_outer_defenses"),
        },
        Item::Unit {
            unit: crate::name!("archer"),
        },
    ] {
        assert!(AdvancedAi::active_queue_answers_siege(&g, &item));
    }
}

#[test]
fn a_damaged_city_can_reclaim_its_scout_queue_for_walls() {
    let (mut g, city, _) = fixture();
    g.at_war.clear();
    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    g.apply(
        0,
        &Action::Produce {
            city,
            item: scout.clone(),
        },
    )
    .unwrap();
    let mut disabled = g.clone();
    assert!(AdvancedAi::new()
        .redirect_unsafe_city_queue_for_defense(&mut disabled, 0, Some(city))
        .is_none());
    assert_eq!(disabled.cities[&city].queue.first(), Some(&scout));

    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    let mut calm = g.clone();
    calm.cities.get_mut(&city).unwrap().hp = 200;
    assert!(ai
        .redirect_unsafe_city_queue_for_defense(&mut calm, 0, None)
        .is_none());
    assert_eq!(calm.cities[&city].queue.first(), Some(&scout));

    assert_eq!(
        ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city)),
        Some((city, walls()))
    );
    assert_eq!(g.cities[&city].queue.first(), Some(&walls()));
}

#[test]
fn a_later_scout_cannot_replace_the_confirmed_wall_handoff() {
    let (mut g, city, _) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city));
    assert_eq!(claim, Some((city, walls())));
    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    g.apply(0, &Action::Produce { city, item: scout }).unwrap();
    g.at_war.clear();
    g.cities.get_mut(&city).unwrap().hp = 200;
    ai.reapply_confirmed_defense_queue(&mut g, 0, claim.as_ref());
    assert_eq!(g.cities[&city].queue.first(), Some(&walls()));
}

#[test]
fn recon_formations_do_not_answer_a_siege_but_local_defenders_still_do() {
    let (g, _, _) = fixture();
    for unit in [crate::name!("scout"), crate::name!("ranger")] {
        for item in [Item::Unit { unit }, Item::Formation { unit, formation: 1 }] {
            assert!(AdvancedAi::active_queue_is_defensive(&g, &item));
            assert!(!AdvancedAi::active_queue_answers_siege(&g, &item));
        }
    }
    for unit in [crate::name!("warrior"), crate::name!("archer")] {
        for item in [Item::Unit { unit }, Item::Formation { unit, formation: 1 }] {
            assert!(AdvancedAi::active_queue_answers_siege(&g, &item));
        }
    }
}
