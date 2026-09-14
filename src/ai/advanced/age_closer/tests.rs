use super::*;
use crate::ai::advanced::{genes, GrandStrategy};
use crate::name::Name;
use crate::setup::GameSpeed;
use std::collections::BTreeSet;
use std::sync::Arc;

fn district(g: &mut Game, cid: u32, kind: &str) {
    let pos = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&cid].pos && g.map.get(*pos).unwrap().district.is_none())
        .unwrap();
    g.map.tiles.get_mut(&pos).unwrap().district = Some(Name::new(kind));
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(Name::new(kind), pos);
}

fn board() -> (Game, u32) {
    let mut g = Game::new_full(2, 32, 24, 914_3575, 250, 0, false);
    g.current = 0;
    g.game_speed = GameSpeed::Standard;
    g.turn = 40;
    g.world_era = 1;
    g.world_era_countdown_end = Some(42);
    let pos = g.units[&g.player_unit_ids(0)[0]].pos;
    let cid = g.found_city_for(0, pos, None);
    district(&mut g, cid, "campus");
    Arc::make_mut(&mut g.rules)
        .great_people
        .retain(|id, _| id == "hypatia");
    g.players[0].civ = "Rome".to_string();
    g.players[0].gold = 5_000.0;
    g.players[0].faith = 0.0;
    g.players[0].era_score = 10;
    g.players[0].normal_age_threshold = 13;
    g.players[0].age = "normal".to_string();
    g.players[0].dedications.clear();
    g.players[0].gpp.clear();
    assert!(g.can_activate_current_great_person(0, "scientist"));
    (g, cid)
}

fn candidate() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_age_closer_2();
    ai
}

fn purchase(currency: &str) -> Action {
    Action::PatronizeGreatPerson {
        kind: "scientist".to_string(),
        currency: currency.to_string(),
    }
}

fn bought(g: &Game, kind: &str) -> i64 {
    g.players[0].gp_claimed.get(kind).copied().unwrap_or(0)
}

#[test]
fn the_second_version_is_separate_and_older_enabling_releases_it() {
    let gene = genes::gene("age-closer-2").unwrap();
    assert!(gene.screenable() && gene.opt_in());
    let mut ai = AdvancedAi::new();
    assert!(!ai.age_closer && !ai.age_closer_2 && !AdvancedAi::legacy().age_closer_2);
    ai.enable_age_closer();
    (gene.enable)(&mut ai);
    assert!(!ai.age_closer && ai.age_closer_2);
    ai.enable_age_closer();
    assert!(ai.age_closer && !ai.age_closer_2);
    (gene.enable)(&mut ai);
    (gene.disable)(&mut ai);
    assert!(!ai.age_closer && !ai.age_closer_2);
}

#[test]
fn a_legal_purchase_closes_three_points_through_the_actual_buyer() {
    for currency in ["gold", "faith"] {
        let (mut g, _) = board();
        if currency == "faith" {
            g.players[0].gold = 0.0;
            g.players[0].faith = 5_000.0;
        }
        candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
        assert_eq!(bought(&g, "scientist"), 1, "{currency}");
        assert_eq!(g.players[0].era_score, 13, "{currency}");
        assert_eq!(g.players[0].era_score, g.players[0].normal_age_threshold);
    }
}

#[test]
fn the_observed_player_board_keeps_the_deadline_and_reaches_the_purchase() {
    let (g, _) = board();
    let mut view = g.player_decision_view(0);
    let ai = candidate();
    assert_eq!(ai.age_closing_deadline(&view, 0), Some(42));
    ai.advanced_great_people(&mut view, 0, GrandStrategy::Science);
    assert_eq!(bought(&view, "scientist"), 1);
    assert_eq!(view.players[0].era_score, 13);
    assert_eq!(bought(&g, "scientist"), 0);
    assert_eq!(g.players[0].gold, 5_000.0);
}

#[test]
fn a_purchase_that_still_leaves_a_dark_age_does_not_get_the_exception() {
    let (mut g, _) = board();
    g.players[0].normal_age_threshold = 14;
    let mut old = g.clone();
    let mut v1 = AdvancedAi::new();
    v1.enable_age_closer();
    v1.advanced_great_people(&mut old, 0, GrandStrategy::Science);
    assert_eq!(bought(&old, "scientist"), 1);
    assert_eq!(old.players[0].era_score, 13);
    candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
    assert_eq!(g.players[0].gold, 5_000.0);
}

#[test]
fn the_half_price_boundary_reads_the_engines_one_point_moment() {
    let (mut g, _) = board();
    let cost = g.gp_cost(0, "scientist");
    g.players[0].gpp.insert("scientist".to_string(), cost / 2.0);
    assert!(!AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    g.players[0].normal_age_threshold = 11;
    assert!(AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    g.apply(0, &purchase("gold")).unwrap();
    assert_eq!(g.players[0].era_score, 11);
}

#[test]
fn taj_mahal_can_make_the_same_purchase_cover_four_points() {
    let (mut g, cid) = board();
    g.players[0].normal_age_threshold = 14;
    assert!(!AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    let pos = g.cities[&cid].pos;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .wonders
        .insert(crate::name!("taj_mahal"), pos);
    assert!(AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(g.players[0].era_score, 14);
    assert_eq!(bought(&g, "scientist"), 1);
}

#[test]
fn dedication_credit_uses_the_actual_age_and_civilization_rules() {
    let (mut g, _) = board();
    g.world_era = 7;
    g.players[0].normal_age_threshold = 14;
    g.players[0].dedications.insert("sky_and_stars".to_string());
    assert!(AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    g.players[0].age = "golden".to_string();
    assert!(!AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    g.players[0].civ = "Georgia".to_string();
    assert!(AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
}

#[test]
fn an_unknown_distant_or_expired_deadline_does_not_relax_patronage() {
    let (g, _) = board();
    let ai = candidate();
    for deadline in [None, Some(46), Some(39), Some(g.max_turns)] {
        let mut other = g.clone();
        other.world_era_countdown_end = deadline;
        assert_eq!(ai.age_closing_deadline(&other, 0), None, "{deadline:?}");
        ai.advanced_great_people(&mut other, 0, GrandStrategy::Science);
        assert_eq!(bought(&other, "scientist"), 0, "{deadline:?}");
    }
    let mut already_safe = g.clone();
    already_safe.players[0].era_score = already_safe.players[0].normal_age_threshold;
    assert_eq!(ai.age_closing_deadline(&already_safe, 0), None);
}

#[test]
fn the_window_scales_with_speed_and_includes_the_last_acting_turn() {
    let (mut g, _) = board();
    let ai = candidate();
    for speed in [GameSpeed::Online, GameSpeed::Standard, GameSpeed::Marathon] {
        g.game_speed = speed;
        let end = g.turn + g.standard_duration(5);
        g.world_era_countdown_end = Some(end);
        assert_eq!(ai.age_closing_deadline(&g, 0), Some(end));
        g.world_era_countdown_end = Some(end + 1);
        assert_eq!(ai.age_closing_deadline(&g, 0), None);
    }
    g.world_era_countdown_end = Some(g.turn);
    assert_eq!(ai.age_closing_deadline(&g, 0), Some(g.turn));
}

#[test]
fn the_lookahead_is_disposable_and_refuses_illegal_host_offers() {
    let (mut g, _) = board();
    let before = serde_json::to_value(&g).unwrap();
    assert!(AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    assert_eq!(serde_json::to_value(&g).unwrap(), before);
    g.players[0].live_great_person_offers = Some(BTreeSet::new());
    assert!(!AdvancedAi::purchase_reaches_normal_age(
        &g,
        0,
        &purchase("gold")
    ));
    candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
}

#[test]
fn emergency_patronage_keeps_the_operating_reserve() {
    let (mut g, _) = board();
    let price = g
        .great_person_patronage_price(0, "scientist", "gold")
        .unwrap();
    g.players[0].gold = price + 199.0;
    candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
    assert_eq!(g.players[0].gold, price + 199.0);
    g.players[0].gold += 1.0;
    candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 1);
    assert_eq!(g.players[0].gold, 200.0);
}

#[test]
fn a_verified_closer_takes_priority_over_a_stronger_ordinary_purchase() {
    let (mut g, cid) = board();
    district(&mut g, cid, "industrial_zone");
    let mut engineer = g.rules.great_people["hypatia"].clone();
    engineer.name = "Clock Engineer".to_string();
    engineer.kind = "engineer".to_string();
    engineer.effects.clear();
    Arc::make_mut(&mut g.rules)
        .great_people
        .insert("clock_engineer".to_string(), engineer);
    let cost = g.gp_cost(0, "scientist");
    g.players[0].gpp.insert("scientist".to_string(), cost * 0.8);
    let mut plain = g.clone();
    AdvancedAi::new().advanced_great_people(&mut plain, 0, GrandStrategy::Science);
    assert_eq!(bought(&plain, "scientist"), 1);
    assert_eq!(plain.players[0].era_score, 11);
    candidate().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
    assert_eq!(bought(&g, "engineer"), 1);
    assert_eq!(g.players[0].era_score, 13);
}
