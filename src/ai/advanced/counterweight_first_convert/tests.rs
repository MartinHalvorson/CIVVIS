use super::*;
use crate::game::install_test_district;
use std::sync::Arc;

/// Live Emperor G200's shape a few turns before the majority fell: Mongolia's
/// Buddhism holds Mongolia and the Mapuche and two of our five cities; Quito
/// follows the Mapuche's Confucianism with the Holy Site and Shrine that sell
/// its Missionaries; we have no religion and a full Faith bank.
fn first_converts() -> (Game, AdvancedAi, u32) {
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
    for x in [2, 8, 14, 20, 26] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let quito = cities[4];
    for cid in &cities[..2] {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Buddhism".into(), 100.0);
    }
    g.cities
        .get_mut(&quito)
        .unwrap()
        .pressure
        .insert("Confucianism".into(), 100.0);
    install_test_district(&mut g, quito, "holy_site");
    g.cities.get_mut(&quito).unwrap().buildings = vec![crate::name!("shrine")];
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[1].religion = Some("Buddhism".into());
    g.players[2].religion = Some("Confucianism".into());
    for pid in [1, 2] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, "Buddhism".into());
    }
    g.players[0].faith = 565.0;
    g.current = 0;
    g.turn = 98;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_counterweight_spends_the_bank();
    assert!(
        !g.civ_follows_religion(0, "Buddhism"),
        "no majority over us yet"
    );
    (g, ai, quito)
}

/// Two of five cities Buddhist: no majority, so the shipped need is zero and
/// nothing is held; from the first convert the need is the two cities it
/// holds, the cap three and the reserve three Missionaries.
#[test]
fn the_first_convert_raises_the_cap_and_the_reserve() {
    let (g, mut ai, quito) = first_converts();
    assert_eq!(AdvancedAi::counterweight_need(&g, 0, "Buddhism"), 0);
    assert_eq!(
        ai.counterweight_cap(&g, 0, "Buddhism", 2),
        2,
        "off: shipped"
    );
    assert_eq!(
        ai.counterweight_faith_reserve(&g, 0),
        0.0,
        "off: nothing held"
    );
    assert!(ai.first_convert_threat(&g, 0).is_none(), "off: no threat");

    ai.enable_counterweight_from_the_first_convert();
    assert_eq!(ai.first_convert_threat(&g, 0).as_deref(), Some("Buddhism"));
    assert_eq!(ai.counterweight_need_for(&g, 0, "Buddhism"), 2);
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 3);
    let price = g
        .unit_purchase_cost(0, quito, "missionary", "faith")
        .unwrap();
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), 3.0 * price);
    assert_eq!(
        ai.counterweight_need_for(&g, 0, "Confucianism"),
        0,
        "the counterfaith is no threat"
    );
}

/// A faith that holds none of the other majors and stands short of the
/// early-warning bar is not yet a live threat, however many of our cities
/// it holds below the majority.
#[test]
fn a_faith_short_of_the_bar_is_not_live() {
    let (mut g, mut ai, _) = first_converts();
    ai.enable_counterweight_from_the_first_convert();
    // This board gives Mongolia no city of its own, so its observed majority
    // would read as a foreign conversion in the lane table: clear both.
    for pid in [1, 2] {
        Arc::make_mut(&mut g.observed_majority_religion).remove(&pid);
    }
    assert!(!g.civ_follows_religion(2, "Buddhism"));
    assert!(!ai.first_convert_live(&g, 0, "Buddhism"));
    assert!(ai.first_convert_threat(&g, 0).is_none());
    assert_eq!(ai.counterweight_need_for(&g, 0, "Buddhism"), 0);
}

/// With no city of ours on a safe counterfaith, the shipped sanctuary has
/// nowhere to build; from the first convert it takes a city that follows no
/// religion yet. A city on a safe counterfaith keeps the shipped choice.
#[test]
fn the_sanctuary_falls_back_to_a_city_with_no_religion() {
    let (mut g, mut ai, quito) = first_converts();
    {
        let city = g.cities.get_mut(&quito).unwrap();
        city.pressure.clear();
        city.buildings.clear();
    }
    assert_eq!(g.city_religion(&g.cities[&quito]), None);
    let shipped = ai.adopted_faith_sanctuary_choice(&g, 0, None);
    assert!(shipped.is_none(), "off: no counterfaith city, no sanctuary");

    ai.enable_counterweight_from_the_first_convert();
    let (cid, item) = ai
        .adopted_faith_sanctuary_choice(&g, 0, None)
        .expect("a faithless city takes the sanctuary");
    assert_eq!(g.city_religion(&g.cities[&cid]), None);
    assert!(
        matches!(&item, crate::game::Item::District { district, .. }
            if g.district_family(*district).as_str() == "holy_site")
            || matches!(&item, crate::game::Item::Building { building }
                if g.building_is_family(building, crate::name!("shrine"))),
        "{item:?}"
    );
}

/// A founder is untouched, and so is every reading with the gene off.
#[test]
fn a_founder_is_untouched() {
    let (mut g, mut ai, _) = first_converts();
    ai.enable_counterweight_from_the_first_convert();
    g.players[0].religion = Some("Taoism".into());
    assert!(ai.first_convert_threat(&g, 0).is_none());
    assert_eq!(ai.counterweight_need_for(&g, 0, "Buddhism"), 0);
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 2);
}
