use super::*;
use crate::game::LiveGreatPersonActivationNeed;

/// Two cities of ours with a Theater and an Amphitheater each, every civic
/// and tech known, idle queues, and a waiting person of `kind` the host
/// cannot seat (no slot open anywhere).
fn fixture(gene: bool, kind: &str) -> (AdvancedAi, Game, Vec<u32>) {
    let (mut g, cities) =
        crate::ai::industrial_zones::tests::producer_empire("WORKSNEVERWAIT", 436_000, 2);
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
    g.players[0].live_open_great_work_slots = Some(Default::default());
    g.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: kind.into(),
            ..Default::default()
        });
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if gene {
        ai.enable_great_works_never_wait();
    }
    (ai, g, cities)
}

fn art_museum() -> Item {
    Item::Building {
        building: crate::name!("art_museum"),
    }
}

fn all_idle(g: &Game, cities: &[u32]) -> bool {
    cities.iter().all(|cid| g.cities[cid].queue.is_empty())
}

#[test]
fn great_works_never_wait_is_reversible_and_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "great-works-never-wait",
        |ai| ai.great_works_never_wait,
    );
}

#[test]
fn only_the_culture_seat_keeps_its_works_from_the_sale() {
    let (ai, g, _) = fixture(true, "artist");
    assert!(ai.culture_keeps_its_great_works(&g));
    assert!(ai.writers_skip_the_palace(&g));
    let (off, g, _) = fixture(false, "artist");
    assert!(!off.culture_keeps_its_great_works(&g));
    assert!(!off.writers_skip_the_palace(&g));
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_great_works_never_wait();
    assert!(!science.culture_keeps_its_great_works(&g));
}

#[test]
fn a_waiting_artist_gets_an_art_museum() {
    let (ai, mut g, cities) = fixture(true, "artist");
    assert!(g.can_produce(0, cities[0], &art_museum()));
    let plan = ai.assess(&g, 0);
    ai.claim_great_work_slot_building(&mut g, 0, &plan);
    let claimed: Vec<u32> = cities
        .iter()
        .copied()
        .filter(|cid| g.cities[cid].queue.first() == Some(&art_museum()))
        .collect();
    assert_eq!(claimed.len(), 1, "one Museum for the one Artist");
    assert!(ai.great_work_slot_building_holds(&g, 0, claimed[0], &plan, &art_museum()));

    // A second pass leaves the claim alone and starts no second Museum.
    ai.claim_great_work_slot_building(&mut g, 0, &plan);
    assert_eq!(
        cities
            .iter()
            .filter(|cid| g.cities[cid].queue.first() == Some(&art_museum()))
            .count(),
        1
    );
}

#[test]
fn no_waiting_artist_a_writer_or_the_gene_off_claims_nothing() {
    let (ai, mut g, cities) = fixture(true, "artist");
    g.players[0].live_great_person_activation_needs[0].required_great_work =
        Some("artifact".into());
    let plan = ai.assess(&g, 0);
    ai.claim_great_work_slot_building(&mut g, 0, &plan);
    assert!(
        all_idle(&g, &cities),
        "a person waiting on an Artifact, not a slot"
    );

    let (ai, mut g, cities) = fixture(true, "writer");
    let plan = ai.assess(&g, 0);
    ai.claim_great_work_slot_building(&mut g, 0, &plan);
    assert!(all_idle(&g, &cities), "a Writer has Amphitheaters");

    let (ai, mut g, cities) = fixture(false, "artist");
    let plan = ai.assess(&g, 0);
    ai.claim_great_work_slot_building(&mut g, 0, &plan);
    assert!(all_idle(&g, &cities), "gene off");
    assert!(!ai.great_work_slot_building_holds(&g, 0, cities[0], &plan, &art_museum()));
}

#[test]
fn a_threatened_city_keeps_its_queue() {
    let (ai, mut g, cities) = fixture(true, "artist");
    let mut plan = ai.assess(&g, 0);
    plan.threatened_city = Some(cities[0]);
    assert!(ai
        .great_work_slot_building_item(&g, 0, cities[0], &plan)
        .is_none());
    ai.claim_great_work_slot_building(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[0]].queue.first(), None);
}
