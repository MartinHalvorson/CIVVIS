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

/// The deadline the gene reads only ever came from the simulator's own
/// `process_eras`; a live board, rebuilt from the host every turn, carried
/// `None`, so this gene could never fire live. The host's era countdown now
/// crosses (`era_countdown`), and a rebuilt board in the era's last turns
/// opens the window while one in mid-era does not.
#[test]
fn a_rebuilt_live_board_carries_the_hosts_era_deadline_into_the_window() {
    use crate::mirror::{rebuild_from_state, Snapshot, StateCity, StateSnapshot, TilesChunk};
    let chunk: TilesChunk = serde_json::from_value(serde_json::json!({
        "turn": 92, "width": 8, "height": 8, "chunk": 1,
        "plots": [{"x": 3, "y": 3, "t": "TERRAIN_GRASS", "o": 0, "w": false, "i": false,
                   "rv": 0, "ri": false, "cl": -1, "p": false, "rp": false, "np": false,
                   "vis": false}],
    }))
    .unwrap();
    let snapshot = Snapshot::from_chunks(&[chunk]);
    let board = |countdown: i64| {
        let state = StateSnapshot {
            turn: 92,
            world_era: Some(3),
            era_score: Some(40),
            normal_age_threshold: Some(43),
            era_countdown: Some(countdown),
            cities: vec![StateCity {
                id: 65_536,
                name: "Bogotá".to_string(),
                x: 3,
                y: 3,
                pop: 6,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        rebuild_from_state(&snapshot, &state, 4, 1, 250, 0).game
    };
    let ai = candidate();
    let last_turns = board(1);
    assert_eq!(last_turns.world_era_countdown_end, Some(94));
    assert_eq!(ai.age_closing_deadline(&last_turns, 0), Some(94));
    let mid_era = board(9);
    assert_eq!(mid_era.world_era_countdown_end, Some(102));
    assert_eq!(ai.age_closing_deadline(&mid_era, 0), None);
    let not_started = board(-1);
    assert_eq!(not_started.world_era_countdown_end, None);
    assert_eq!(ai.age_closing_deadline(&not_started, 0), None);
    // Off, the same last-turns board reads no deadline at all.
    assert_eq!(AdvancedAi::new().age_closing_deadline(&last_turns, 0), None);
}

/// G111's own numbers (civvis-20261005T080337Z t104, frame replayed on pin
/// 460f8bddd): era score 58 of 59 with the era ending at t107, the Scientist
/// 23 points short (545 gold), 468 gold in the bank, a 650-gold reserve at
/// ten cities. The window was open; nothing was affordable, so no version of
/// the gene could have bought the point.
fn g111_scientist(g: &mut Game) -> f64 {
    let cost = g.gp_cost(0, "scientist");
    g.players[0].gpp.insert("scientist".to_string(), cost - 23.0);
    g.players[0].era_score = 58;
    g.players[0].normal_age_threshold = 59;
    g.players[0].gold_per_turn = 8.4;
    let price = g.great_person_patronage_price(0, "scientist", "gold").unwrap();
    assert_eq!(price, 545.0);
    price
}

fn spender() -> AdvancedAi {
    let mut ai = candidate();
    ai.enable_age_closer_spends_the_reserve();
    ai
}

#[test]
fn g111_live_numbers_leave_no_affordable_closer() {
    let (mut g, _) = board();
    g111_scientist(&mut g);
    g.players[0].gold = 468.0;
    for ai in [candidate(), spender()] {
        let mut trial = g.clone();
        assert_eq!(ai.age_closing_deadline(&trial, 0), Some(42));
        ai.advanced_great_people(&mut trial, 0, GrandStrategy::Science);
        assert_eq!(bought(&trial, "scientist"), 0);
        assert_eq!(trial.players[0].era_score, 58);
    }
}

#[test]
fn a_verified_closer_may_spend_the_gold_reserve_under_the_gene() {
    let (mut g, _) = board();
    let price = g111_scientist(&mut g);
    // Enough for the price, not for the price plus the operating reserve.
    g.players[0].gold = price + 55.0;
    let mut kept = g.clone();
    candidate().advanced_great_people(&mut kept, 0, GrandStrategy::Science);
    assert_eq!(bought(&kept, "scientist"), 0, "age-closer-2 alone keeps the reserve");
    spender().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 1);
    assert_eq!(g.players[0].era_score, 59);
    assert_eq!(g.players[0].gold, 55.0);
}

#[test]
fn the_reserve_stays_when_the_purchase_would_not_close_or_the_books_bleed() {
    // Not a closer: two points short, the near-recruit pays one.
    let (mut g, _) = board();
    let price = g111_scientist(&mut g);
    g.players[0].era_score = 57;
    g.players[0].gold = price + 55.0;
    spender().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
    // A closer, but income runs at -30: the floor is five turns of deficit.
    let (mut g, _) = board();
    let price = g111_scientist(&mut g);
    g.players[0].gold_per_turn = -30.0;
    g.players[0].gold = price + 149.0;
    spender().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
    g.players[0].gold = price + 150.0;
    spender().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 1);
    // Outside the window the gene does nothing.
    let (mut g, _) = board();
    let price = g111_scientist(&mut g);
    g.world_era_countdown_end = Some(60);
    g.players[0].gold = price + 55.0;
    spender().advanced_great_people(&mut g, 0, GrandStrategy::Science);
    assert_eq!(bought(&g, "scientist"), 0);
}
