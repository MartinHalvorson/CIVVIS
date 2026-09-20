use super::*;

fn fixture() -> (Game, u32, Item, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 28, 18, 41_109, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|u| g.units[u].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.turn = 85;
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 20.0;
    g.players[0].techs.extend(
        ["pottery", "writing", "astrology", "education", "currency"].map(crate::name::Name::new),
    );
    g.cities.get_mut(&cid).unwrap().pop = 10;
    for pos in g.cities[&cid].owned_tiles.clone() {
        if pos == g.cities[&cid].pos {
            continue;
        }
        let t = g.map.tiles.get_mut(&pos).unwrap();
        t.terrain = crate::name!("plains");
        t.feature = None;
        t.hills = false;
        t.resource = None;
        t.improvement = None;
        t.district = None;
        t.wonder = None;
    }
    let campus = g.district_sites(cid, crate::name!("campus"))[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    g.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    g.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".into(),
            individual: Some("hildegard_of_bingen".into()),
            required_district: Some("holy_site".into()),
            required_missing_building: None,
            required_great_work: None,
        },
    );
    let mut ai = AdvancedAi::new();
    ai.victory_planning = true;
    ai.victory_target = Some(VictoryTarget::Domination);
    ai.preempt_margin = 1.25;
    assert!(ai.base.prioritize_live_great_person_activation(&mut g, 0));
    let item = g.cities[&cid].queue[0].clone();
    assert!(matches!(item, Item::District { district, .. } if district == "holy_site"));
    assert_eq!(g.item_invested_production(cid, &item), 0.0);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, item, ai, plan)
}

#[test]
fn live_scientist_activation_district_survives_the_economic_governor() {
    let (mut g, cid, item, mut ai, plan) = fixture();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(
        g.cities[&cid].queue.first(),
        Some(&item),
        "Hildegard's newly reserved Holy Site must survive ordinary rescoring"
    );
}

#[test]
fn ordinary_holy_site_is_still_reviewed_without_a_live_person() {
    let (mut g, cid, item, mut ai, plan) = fixture();
    g.players[0].live_great_person_activation_needs.clear();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn an_unrelated_person_does_not_reserve_the_holy_site() {
    let (mut g, cid, item, mut ai, plan) = fixture();
    g.players[0].live_great_person_activation_needs[0].required_district = Some("campus".into());
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn completed_activation_district_closes_the_reservation() {
    let (mut g, cid, item, _, _) = fixture();
    let pos = g.cities[&cid].pos;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), pos);
    assert!(!AdvancedAi::live_gp_district_commitment(&g, 0, &item));
}

#[test]
fn a_city_under_fire_can_replace_the_activation_district() {
    let (mut g, cid, item, mut ai, _) = fixture();
    ai.enable_garrison_under_fire();
    g.players[0].gold = 0.0;
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    g.cities.get_mut(&cid).unwrap().hp = 100;
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, None);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
    assert!(matches!(
        g.cities[&cid].queue.first(),
        Some(Item::Unit { .. })
    ));
}
