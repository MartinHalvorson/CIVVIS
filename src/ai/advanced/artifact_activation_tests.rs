use super::*;
use crate::game::LiveGreatPersonActivationNeed;

fn fixture() -> (AdvancedAi, Game, Vec<u32>, crate::Pos) {
    let (mut g, cities) =
        crate::ai::industrial_zones::tests::producer_empire("ARTIFACTACTIVATION", 436_000, 2);
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].civics = g.rules.civics.keys().copied().collect();
    for cid in &cities {
        let position = g.cities[cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|pos| *pos != g.cities[cid].pos && g.map.tiles[pos].district.is_none())
            .unwrap();
        g.map.tiles.get_mut(&position).unwrap().district = Some(crate::name!("theater_square"));
        let city = g.cities.get_mut(cid).unwrap();
        city.districts
            .insert(crate::name!("theater_square"), position);
        city.buildings.push(crate::name!("amphitheater"));
        city.queue.clear();
    }
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    for tile in g.map.tiles.values_mut() {
        if matches!(
            tile.resource.as_deref(),
            Some("antiquity_site" | "shipwreck")
        ) {
            tile.resource = None;
        }
    }
    let site = g.cities[&cities[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&cities[0]].pos && g.map.tiles[pos].district.is_none())
        .unwrap();
    let tile = g.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = Some(crate::name!("antiquity_site"));
    tile.improvement = None;
    tile.district_foundation = None;
    tile.wonder = None;
    tile.flooded = false;
    tile.submerged = false;
    g.players[0]
        .counters
        .insert("great_work:artifact".into(), 0);
    g.players[0].live_open_great_work_slots = Some(["artifact".into()].into_iter().collect());
    g.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: "scientist".into(),
            individual: Some("mary_leakey".into()),
            required_district: Some("theater_square".into()),
            required_great_work: Some("artifact".into()),
            ..Default::default()
        });
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_artifact_before_more_museums();
    assert!(g.can_produce(0, cities[0], &digger()));
    assert!(!g.excavation_sites(0).is_empty());
    (ai, g, cities, site)
}

fn digger() -> Item {
    Item::Unit {
        unit: crate::name!("archaeologist"),
    }
}

#[test]
fn artifact_activation_is_reversible_and_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers("artifact-before-more-museums", |ai| {
        ai.artifact_before_more_museums
    });
    let mut ai = AdvancedAi::new();
    assert!(!ai.base.artifact_before_more_museums);
    ai.enable_artifact_before_more_museums();
    assert!(ai.base.artifact_before_more_museums);
    ai.disable_artifact_before_more_museums();
    assert!(!ai.base.artifact_before_more_museums);
}

#[test]
fn artifact_activation_supplies_a_digger_instead_of_more_empty_museums() {
    let (ai, mut g, cities, _) = fixture();
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        Some(digger())
    );
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[1], "artifact"),
        None
    );
    let off = BasicAi::new();
    assert!(
        matches!(off.live_great_person_cultural_item(&g, 0, cities[1], "artifact"),
        Some(Item::Building { building }) if building == "archaeological_museum")
    );
    assert!(ai.base.prioritize_live_great_person_activation(&mut g, 0));
    assert_eq!(g.cities[&cities[0]].queue.first(), Some(&digger()));
    assert_eq!(
        g.cities
            .values()
            .filter(|c| c.queue.contains(&digger()))
            .count(),
        1
    );
    let plan = ai.assess(&g, 0);
    ai.reconcile_production_commitments(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[0]].queue.first(), Some(&digger()));
}

#[test]
fn artifact_activation_waits_for_existing_or_queued_digger() {
    let (ai, mut g, cities, _) = fixture();
    let uid = g.spawn_test_unit("archaeologist", 0, g.cities[&cities[0]].pos);
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
    g.remove_unit(uid);
    g.cities.get_mut(&cities[1]).unwrap().queue.push(digger());
    assert!(!ai.base.prioritize_live_great_person_activation(&mut g, 0));
}

#[test]
fn artifact_activation_requires_authoritative_zero_and_a_physical_need() {
    let (ai, mut g, cities, _) = fixture();
    g.players[0].counters.remove("great_work:artifact");
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
    g.players[0]
        .counters
        .insert("great_work:artifact".into(), 1);
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
    g.players[0]
        .counters
        .insert("great_work:artifact".into(), 0);
    g.players[0].live_great_person_activation_needs.clear();
    assert!(!ai.base.prioritize_live_great_person_activation(&mut g, 0));
}

#[test]
fn artifact_activation_respects_owned_land_digs_legal_builds_and_deadline() {
    let (ai, mut g, cities, site) = fixture();
    g.map.tiles.get_mut(&site).unwrap().owner_city = None;
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
    g.map.tiles.get_mut(&site).unwrap().owner_city = Some(cities[0]);
    g.players[0].civics.remove(&crate::name!("natural_history"));
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
    g.players[0].civics.insert(crate::name!("natural_history"));
    g.turn = g.max_turns - 1;
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
}

#[test]
fn artifact_activation_leaves_active_units_and_damaged_city_queues_alone() {
    let (ai, mut g, cities, _) = fixture();
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: crate::name!("builder"),
        });
    assert!(!ai.base.prioritize_live_great_person_activation(&mut g, 0));
    g.cities.get_mut(&cities[0]).unwrap().queue.clear();
    g.cities.get_mut(&cities[0]).unwrap().hp = 190;
    assert_eq!(
        ai.base
            .live_great_person_cultural_item(&g, 0, cities[0], "artifact"),
        None
    );
}

#[test]
fn artifact_activation_does_not_rebuild_capacity_when_slots_are_known_full() {
    let (ai, mut g, cities, _) = fixture();
    g.players[0].live_open_great_work_slots = Some(Default::default());
    assert!(!ai.base.prioritize_live_great_person_activation(&mut g, 0));
    assert!(cities.iter().all(|cid| g.cities[cid].queue.is_empty()));
}
