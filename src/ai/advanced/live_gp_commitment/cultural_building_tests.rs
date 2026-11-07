use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::{install_test_district, LiveGreatPersonActivationNeed};

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn fixture(kind: &str) -> (Game, u32, Item, AdvancedAi, StrategicPlan) {
    let (mut g, cities) = producer_empire("CULTURALACTIVATION", 471_701, 2);
    let cid = cities[0];
    g.players[0]
        .civics
        .extend(["drama_poetry", "humanism", "mass_media"].map(Name::new));
    g.players[0].techs.insert(crate::name!("education"));
    for other in &cities {
        let city = g.cities.get_mut(other).unwrap();
        city.pop = 10;
        city.queue = vec![building("walls")];
    }
    install_test_district(&mut g, cid, "theater_square");
    install_test_district(&mut g, cid, "industrial_zone");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    let item = building("art_museum");
    assert!(g.can_produce(0, cid, &item));
    g.cities.get_mut(&cid).unwrap().queue = vec![item.clone()];
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    g.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: kind.into(),
            required_district: Some("theater_square".into()),
            ..Default::default()
        });
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_industrial_zone_claims_the_producer_queue();
    ai.enable_campus_building_claims_the_queue();
    ai.enable_trader_fills_the_idle_route();
    ai.enable_backlog_builder_takes_the_queue();
    ai.enable_granary_claims_the_bound_queue();
    ai.enable_capital_monument_after_the_second_city();
    ai.enable_culture_seat_theater_first();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, item, ai, plan)
}

#[test]
fn a_waiting_artist_keeps_the_museum_through_the_industry_claim() {
    let (mut g, cid, item, ai, plan) = fixture("artist");
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn no_waiting_person_leaves_the_ordinary_workshop_claim_available() {
    let (mut g, cid, _, ai, plan) = fixture("artist");
    g.players[0].live_great_person_activation_needs.clear();
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&building("workshop")));
}

#[test]
fn a_waiting_artist_keeps_the_museum_through_the_campus_claim() {
    let (mut g, cid, item, ai, plan) = fixture("artist");
    ai.claim_campus_building(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn a_writer_does_not_reserve_an_artists_museum() {
    let (mut g, cid, _, ai, plan) = fixture("writer");
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&building("workshop")));
}

#[test]
fn a_required_artifact_does_not_reserve_an_empty_museum() {
    let (mut g, cid, _, ai, plan) = fixture("scientist");
    g.players[0].live_great_person_activation_needs[0].required_great_work =
        Some("artifact".into());
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&building("workshop")));
}

#[test]
fn the_musicians_missing_museum_rung_survives_the_claims_and_governor() {
    let (mut g, cid, item, mut ai, plan) = fixture("musician");
    ai.run_idle_queue_claims(&mut g, 0, &plan);
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

fn prepare_another_music_city(g: &mut Game, except: u32) {
    let other = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != except)
        .unwrap();
    install_test_district(g, other, "theater_square");
    g.cities
        .get_mut(&other)
        .unwrap()
        .buildings
        .extend([crate::name!("amphitheater"), crate::name!("art_museum")]);
}

#[test]
fn a_musician_releases_an_extra_museum_when_a_broadcast_city_is_prepared() {
    let (mut g, cid, item, ai, plan) = fixture("musician");
    prepare_another_music_city(&mut g, cid);
    assert!(!AdvancedAi::live_gp_cultural_building_commitment(
        &g, 0, cid, &item
    ));
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&building("workshop")));
}

#[test]
fn an_artists_own_museum_need_survives_a_prepared_music_path() {
    let (mut g, cid, item, ai, plan) = fixture("artist");
    prepare_another_music_city(&mut g, cid);
    g.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: "musician".into(),
            required_district: Some("theater_square".into()),
            ..Default::default()
        });
    assert!(AdvancedAi::live_gp_cultural_building_commitment(
        &g, 0, cid, &item
    ));
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn the_artists_zero_investment_museum_survives_the_claims_and_governor() {
    let (mut g, cid, item, mut ai, plan) = fixture("artist");
    ai.preempt_margin = 1.25;
    ai.run_idle_queue_claims(&mut g, 0, &plan);
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    assert_eq!(g.item_invested_production(cid, &item), 0.0);
}

#[test]
fn the_queue_is_released_when_the_host_clears_the_persons_need() {
    let (mut g, cid, item, ai, plan) = fixture("artist");
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    g.players[0].live_great_person_activation_needs.clear();
    ai.claim_producer_zone(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&building("workshop")));
}

#[test]
fn an_invalid_building_cannot_hold_the_queue() {
    let (mut g, cid, item, _, _) = fixture("artist");
    g.players[0].civics.remove(&crate::name!("humanism"));
    assert!(!AdvancedAi::live_gp_cultural_building_commitment(
        &g, 0, cid, &item
    ));
}

#[test]
fn local_defense_can_replace_the_artists_museum() {
    let (mut g, cid, item, mut ai, _) = fixture("artist");
    ai.enable_garrison_under_fire();
    g.players[0].gold = 0.0;
    g.cities.get_mut(&cid).unwrap().hp = 100;
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, None);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
    assert!(
        matches!(g.cities[&cid].queue.first(), Some(Item::Unit { .. }))
            || g.cities[&cid].queue.first() == Some(&building("walls"))
    );
}

#[test]
fn sciences_existing_insolvency_preemption_still_replaces_the_museum() {
    let (mut g, cid, item, mut ai, plan) = fixture("artist");
    ai.victory_target = Some(VictoryTarget::Science);
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -10.0;
    assert!(!AdvancedAi::live_gp_cultural_building_commitment(
        &g, 0, cid, &item
    ));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn an_artist_does_not_yield_its_museum_to_a_late_capital_monument() {
    let (mut g, cid, item, ai, plan) = fixture("artist");
    assert!(g.cities[&cid].is_capital);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|b| *b != "monument");
    ai.claim_capital_monument(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}
