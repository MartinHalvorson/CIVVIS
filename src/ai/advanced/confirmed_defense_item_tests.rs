use super::*;

fn fixture() -> (Game, u32, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 936025, 200, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let city = g.found_city_for(0, (8, 12), None);
    g.players[0].techs.extend([
        crate::name!("masonry"),
        crate::name!("animal_husbandry"),
        crate::name!("archery"),
    ]);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 25;
    g.cities.get_mut(&city).unwrap().hp = 170;
    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    (g, city, ai)
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

#[test]
fn a_later_local_defender_cannot_erase_the_confirmed_walls() {
    let (mut g, city, ai) = fixture();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city));
    assert_eq!(claim, Some((city, walls())));
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    g.apply(0, &Action::Produce { city, item: archer }).unwrap();
    // The claim survives even if a later pass consumes its threat evidence.
    g.at_war.clear();
    g.cities.get_mut(&city).unwrap().hp = 200;
    ai.reapply_confirmed_defense_queue(&mut g, 0, claim.as_ref());
    assert_eq!(g.cities[&city].queue.first(), Some(&walls()));
}

#[test]
fn the_selector_still_retains_an_already_running_local_defender() {
    let (mut g, city, ai) = fixture();
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    g.apply(
        0,
        &Action::Produce {
            city,
            item: archer.clone(),
        },
    )
    .unwrap();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city));
    assert_eq!(claim, Some((city, archer.clone())));
    ai.reapply_confirmed_defense_queue(&mut g, 0, claim.as_ref());
    assert_eq!(g.cities[&city].queue.first(), Some(&archer));
}

#[test]
fn an_unclaimed_queue_remains_with_its_production_writer() {
    let (mut g, city, ai) = fixture();
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    g.apply(
        0,
        &Action::Produce {
            city,
            item: archer.clone(),
        },
    )
    .unwrap();
    ai.reapply_confirmed_defense_queue(&mut g, 0, None);
    assert_eq!(g.cities[&city].queue.first(), Some(&archer));
}
