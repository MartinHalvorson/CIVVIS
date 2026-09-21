use super::super::*;

fn fixture() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 30, 24, 365_600, 300, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let home = g.found_city_for(0, (4, 4), None);
    let second = g.found_city_for(0, (10, 4), None);
    g.found_city_for(1, (18, 12), None);
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].holy_city = Some(home);
    g.players[0].religion_beliefs = vec!["work_ethic".into(), "tithe".into()];
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].civics.insert(crate::name!("theology"));
    for cid in [home, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.atheist_pressure = 0.0;
        city.pressure.clear();
        city.pressure.insert("Buddhism".into(), 1000.0);
    }
    crate::game::install_test_district(&mut g, home, "holy_site");
    g.cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .extend([crate::name!("shrine"), crate::name!("temple")]);
    g.current = 0;
    g.turn = 95;
    g.players[0].faith = 1000.0;
    (g, AdvancedAi::targeting(VictoryTarget::Domination), home)
}

#[test]
fn saves_for_apostle_instead_of_affordable_repeat_missionary() {
    let (mut g, mut ai, home) = fixture();
    let missionary = g
        .unit_purchase_cost(0, home, "missionary", "faith")
        .unwrap();
    let apostle = g.unit_purchase_cost(0, home, "apostle", "faith").unwrap();
    assert!(apostle > missionary);
    g.players[0].faith = missionary;
    ai.religious_spending(&mut g, 0, false);
    assert_eq!(g.players[0].faith, missionary);
    assert!(!g.units.values().any(|u| u.owner == 0));
    g.players[0].faith = apostle;
    ai.religious_spending(&mut g, 0, false);
    assert!(g
        .units
        .values()
        .any(|u| u.owner == 0 && u.kind == "apostle"));
}

#[test]
fn prepares_inquisition_before_any_home_city_flips_then_funds_first_defender() {
    let (mut g, mut ai, _) = fixture();
    ai.religious_spending(&mut g, 0, false);
    let apostle = g
        .units
        .values()
        .find(|u| u.owner == 0 && u.kind == "apostle")
        .unwrap()
        .id;
    g.units.get_mut(&apostle).unwrap().moves_left = 3.0;
    assert!(ai.advanced_religious_step(&mut g, 0, apostle, false));
    assert_eq!(g.players[0].counters.get("inquisition"), Some(&1));
    assert_eq!(g.players[0].religion_beliefs.len(), 2);
    ai.religious_spending(&mut g, 0, false);
    assert_eq!(
        g.units
            .values()
            .filter(|u| u.owner == 0 && u.kind == "inquisitor")
            .count(),
        1
    );
    assert!(!ai.prepare_defensive_inquisition(&mut g, 0));
}

#[test]
fn temple_and_own_faith_source_are_required() {
    let (mut g, ai, home) = fixture();
    assert!(ai.defensive_inquisition_ready(&g, 0));
    g.cities
        .get_mut(&home)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("temple"));
    assert!(!ai.defensive_inquisition_ready(&g, 0));
    g.cities.get_mut(&home).unwrap().pillaged_buildings.clear();
    g.cities.get_mut(&home).unwrap().pressure.clear();
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Orthodoxy".into(), 1000.0);
    assert!(!ai.defensive_inquisition_ready(&g, 0));
}

#[test]
fn other_lanes_nonfounders_and_disabled_religion_keep_existing_spending() {
    let (mut g, ai, _) = fixture();
    assert!(!AdvancedAi::targeting(VictoryTarget::Science).defensive_inquisition_ready(&g, 0));
    g.victory_conditions.religious = false;
    assert!(!ai.defensive_inquisition_ready(&g, 0));
    g.victory_conditions.religious = true;
    g.players[0].religion = None;
    assert!(!ai.defensive_inquisition_ready(&g, 0));
}

#[test]
fn initial_spread_precedes_reserve_in_a_mostly_unconverted_empire() {
    let (mut g, ai, home) = fixture();
    g.found_city_for(0, (4, 12), None);
    for cid in g.player_city_ids(0) {
        if cid != home {
            let city = g.cities.get_mut(&cid).unwrap();
            city.pressure.clear();
            city.atheist_pressure = 1000.0;
        }
    }
    assert!(!ai.defensive_inquisition_ready(&g, 0));
    let uid = g.spawn_test_unit("missionary", 0, (6, 4));
    g.units.get_mut(&uid).unwrap().religion = Some("Buddhism".into());
    assert!(ai.defensive_inquisition_ready(&g, 0));
}

#[test]
fn existing_apostle_prevents_duplicate_purchase_and_banks_for_its_inquisitor() {
    let (mut g, mut ai, _) = fixture();
    ai.religious_spending(&mut g, 0, false);
    let faith = g.players[0].faith;
    ai.religious_spending(&mut g, 0, false);
    assert_eq!(g.players[0].faith, faith);
    assert_eq!(
        g.units
            .values()
            .filter(|u| u.owner == 0 && u.kind == "apostle")
            .count(),
        1
    );
}

#[test]
fn host_menu_without_an_affordable_apostle_does_not_release_the_reserve() {
    let (mut g, mut ai, home) = fixture();
    g.players[0].faith = 100.0;
    let missionary = Item::Unit {
        unit: crate::name!("missionary"),
    };
    let mut menu = BTreeMap::new();
    menu.insert(
        Game::production_block_key(&missionary),
        crate::game::HostPurchaseEntry {
            gold: None,
            faith: Some(80.0),
        },
    );
    Arc::make_mut(&mut g.host_purchasable).insert(home, menu);
    assert_eq!(
        g.unit_purchase_cost(0, home, "missionary", "faith"),
        Some(80.0)
    );
    assert_eq!(g.unit_purchase_cost(0, home, "apostle", "faith"), None);
    ai.religious_spending(&mut g, 0, false);
    assert_eq!(g.players[0].faith, 100.0);
    assert!(!g.units.values().any(|u| u.owner == 0));
}

fn approaching_spreader(g: &mut Game, religion: &str, position: Pos) -> u32 {
    g.players[1].religion = Some("Orthodoxy".into());
    let uid = g.spawn_test_unit("missionary", 1, position);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.religion = Some(religion.into());
    unit.charges = 3;
    uid
}

#[test]
fn threatened_source_buys_affordable_cover_before_saving_for_apostle() {
    let (mut g, mut ai, home) = fixture();
    approaching_spreader(&mut g, "Orthodoxy", (5, 4));
    g.players[0].faith = g
        .unit_purchase_cost(0, home, "missionary", "faith")
        .unwrap();
    ai.religious_spending(&mut g, 0, false);
    assert!(g
        .units
        .values()
        .any(|u| u.owner == 0 && u.kind == "missionary"));
    g.players[0].faith = g
        .unit_purchase_cost(0, home, "missionary", "faith")
        .unwrap();
    let bank = g.players[0].faith;
    ai.religious_spending(&mut g, 0, false);
    assert_eq!(
        g.players[0].faith, bank,
        "one charged defender restores saving"
    );
    assert_eq!(g.units.values().filter(|u| u.owner == 0).count(), 1);
}

#[test]
fn funded_apostle_still_precedes_emergency_missionary() {
    let (mut g, mut ai, _) = fixture();
    approaching_spreader(&mut g, "Orthodoxy", (5, 4));
    ai.religious_spending(&mut g, 0, false);
    assert!(g
        .units
        .values()
        .any(|u| u.owner == 0 && u.kind == "apostle"));
    assert!(!g
        .units
        .values()
        .any(|u| u.owner == 0 && u.kind == "missionary"));
}

#[test]
fn harmless_or_distant_spreader_does_not_break_apostle_reserve() {
    for (religion, position, charges) in [
        ("Buddhism", (5, 4), 3),
        ("Orthodoxy", (18, 12), 3),
        ("Orthodoxy", (5, 4), 0),
    ] {
        let (mut g, mut ai, home) = fixture();
        let uid = approaching_spreader(&mut g, religion, position);
        g.units.get_mut(&uid).unwrap().charges = charges;
        g.players[0].faith = g
            .unit_purchase_cost(0, home, "missionary", "faith")
            .unwrap();
        let bank = g.players[0].faith;
        ai.religious_spending(&mut g, 0, false);
        assert_eq!(g.players[0].faith, bank);
        assert!(!g.units.values().any(|u| u.owner == 0));
    }
}
