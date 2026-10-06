use super::*;

fn native_offer(kind: &str, individual: &str) -> Game {
    let mut game = Game::new_full(1, 24, 16, 202610060601, 300, 0, false);
    game.players[0].gold = 200.0;
    game.players[0].live_great_person_offers = Some([kind.to_string()].into_iter().collect());
    game.players[0]
        .live_great_person_offer_individuals
        .insert(kind.to_string(), individual.to_string());
    let cost = game.gp_cost(0, kind);
    game.players[0].gpp.insert(kind.to_string(), cost);
    game
}

fn recruit(game: &mut Game, kind: &str) -> Result<(), String> {
    game.apply(0, &Action::RecruitGreatPerson { kind: kind.into() })
}

#[test]
fn native_gp_unknown_merchant_does_not_pay_crassus_gold() {
    let mut game = native_offer("merchant", "masaru_ibuka");
    assert!(!game.rules.great_people.contains_key("masaru_ibuka"));
    assert_eq!(
        game.current_great_person("merchant").unwrap().0,
        "marcus_licinius_crassus"
    );
    assert!(game.legal_actions(0).iter().any(|action| matches!(
        action, Action::RecruitGreatPerson { kind } if kind == "merchant"
    )));
    recruit(&mut game, "merchant").unwrap();
    assert_eq!(
        game.players[0].gold, 200.0,
        "recruiting a physical unit is not an activation"
    );
    assert_eq!(game.players[0].gpp["merchant"], 0.0);
    assert!(
        game.players[0].great_people.is_empty(),
        "the unrelated modeled person was not retired"
    );
    assert!(game.retired_great_people.is_empty());
    assert_eq!(
        game.live_great_person_offer_individual(0, "merchant"),
        Some("masaru_ibuka")
    );
    assert!(!game.great_person_class_offered_now(0, "merchant"));
}

#[test]
fn native_gp_matching_named_merchant_still_requires_activation() {
    let mut game = native_offer("merchant", "marcus_licinius_crassus");
    recruit(&mut game, "merchant").unwrap();
    assert_eq!(game.players[0].gold, 200.0);
    assert!(game.players[0].great_people.is_empty());
    assert!(game.retired_great_people.is_empty());
}

#[test]
fn native_gp_unnamed_export_still_recruits_a_physical_unit() {
    let mut game = native_offer("merchant", "masaru_ibuka");
    game.players[0].live_great_person_offer_individuals.clear();
    recruit(&mut game, "merchant").unwrap();
    assert_eq!(game.players[0].gold, 200.0);
    assert!(!game.great_person_class_offered_now(0, "merchant"));
}

#[test]
fn native_gp_patronage_deducts_currency_without_an_activation_reward() {
    for currency in ["gold", "faith"] {
        let mut game = native_offer("merchant", "masaru_ibuka");
        let cost = game.gp_cost(0, "merchant");
        game.players[0].gpp.insert("merchant".into(), cost - 10.0);
        game.players[0].gold = 5_000.0;
        game.players[0].faith = 5_000.0;
        let price = game
            .great_person_patronage_price(0, "merchant", currency)
            .unwrap();
        game.apply(
            0,
            &Action::PatronizeGreatPerson {
                kind: "merchant".into(),
                currency: currency.into(),
            },
        )
        .unwrap();
        assert_eq!(
            game.players[0].gold,
            5_000.0 - if currency == "gold" { price } else { 0.0 }
        );
        assert_eq!(
            game.players[0].faith,
            5_000.0 - if currency == "faith" { price } else { 0.0 }
        );
        assert_eq!(game.players[0].gpp["merchant"], 0.0);
        assert!(game.players[0].great_people.is_empty());
        assert!(!game.great_person_class_offered_now(0, "merchant"));
        assert_eq!(
            game.great_person_patronage_price(0, "merchant", currency),
            None
        );
    }
}

#[test]
fn native_gp_consumed_offer_cannot_be_bought_again_in_the_same_plan() {
    let mut game = native_offer("merchant", "masaru_ibuka");
    game.players[0].gold = 50_000.0;
    recruit(&mut game, "merchant").unwrap();
    assert!(game
        .apply(
            0,
            &Action::PatronizeGreatPerson {
                kind: "merchant".into(),
                currency: "gold".into(),
            }
        )
        .is_err());
    assert_eq!(game.players[0].gold, 50_000.0);
    assert!(!game.legal_actions(0).iter().any(|action| matches!(
        action, Action::RecruitGreatPerson { kind } | Action::PatronizeGreatPerson { kind, .. }
        if kind == "merchant"
    )));
}

#[test]
fn native_gp_rejected_recruitment_keeps_currency_points_and_offer() {
    for blocker in [false, true] {
        let mut game = native_offer("merchant", "masaru_ibuka");
        if blocker {
            game.players[0]
                .live_great_person_offer_blockers
                .insert("merchant".into(), "requires Industrial Zone".into());
        } else {
            game.players[0].gpp.insert("merchant".into(), 0.0);
        }
        let points = game.players[0].gpp["merchant"];
        assert!(recruit(&mut game, "merchant").is_err());
        assert_eq!(game.players[0].gold, 200.0);
        assert_eq!(game.players[0].gpp["merchant"], points);
        assert!(game.great_person_class_offered_now(0, "merchant"));
        assert!(game.retired_great_people.is_empty());
    }
}

#[test]
fn native_gp_scientist_does_not_grant_unactivated_eurekas() {
    let mut game = native_offer("scientist", "unmodeled_native_scientist");
    let boosts = game.players[0].boosted_techs.clone();
    let progress = game.players[0].research_progress;
    recruit(&mut game, "scientist").unwrap();
    assert_eq!(game.players[0].boosted_techs, boosts);
    assert_eq!(game.players[0].research_progress, progress);
    assert!(game.players[0].great_people.is_empty());
}

#[test]
fn native_gp_general_does_not_promote_or_spawn_from_the_local_roster() {
    let mut game = native_offer("general", "unmodeled_native_general");
    let position = game.units.values().next().unwrap().pos;
    let unit = game.spawn_unit("swordsman", 0, position);
    let count = game.units.len();
    let (xp, level, formation) = (
        game.units[&unit].xp,
        game.units[&unit].level,
        game.units[&unit].formation,
    );
    recruit(&mut game, "general").unwrap();
    assert_eq!(game.units.len(), count);
    assert_eq!(
        (
            game.units[&unit].xp,
            game.units[&unit].level,
            game.units[&unit].formation
        ),
        (xp, level, formation)
    );
    assert!(game.players[0].great_people.is_empty());
}

#[test]
fn native_gp_failed_patronage_does_not_consume_the_offer() {
    for currency in ["gold", "faith", "invalid"] {
        let mut game = native_offer("merchant", "masaru_ibuka");
        let cost = game.gp_cost(0, "merchant");
        game.players[0].gpp.insert("merchant".into(), cost - 10.0);
        game.players[0].gold = 1.0;
        game.players[0].faith = 1.0;
        assert!(game
            .apply(
                0,
                &Action::PatronizeGreatPerson {
                    kind: "merchant".into(),
                    currency: currency.into(),
                }
            )
            .is_err());
        assert_eq!(game.players[0].gold, 1.0);
        assert_eq!(game.players[0].faith, 1.0);
        assert_eq!(game.players[0].gpp["merchant"], cost - 10.0);
        assert!(game.great_person_class_offered_now(0, "merchant"));
        assert_eq!(
            game.live_great_person_offer_individual(0, "merchant"),
            Some("masaru_ibuka")
        );
    }
}

#[test]
fn native_gp_headless_merchant_keeps_immediate_retirement() {
    let mut game = native_offer("merchant", "masaru_ibuka");
    game.players[0].live_great_person_offers = None;
    game.players[0].live_great_person_offer_individuals.clear();
    recruit(&mut game, "merchant").unwrap();
    assert_eq!(game.players[0].gold, 380.0);
    assert_eq!(game.players[0].great_people, ["marcus_licinius_crassus"]);
    assert!(game
        .retired_great_people
        .contains("marcus_licinius_crassus"));
}

fn mirror_offer() -> (crate::mirror::Snapshot, crate::mirror::StateSnapshot) {
    use crate::mirror::{Snapshot, StateSnapshot, TilesChunk};
    let chunk: TilesChunk = serde_json::from_value(serde_json::json!({
        "turn": 226, "width": 10, "height": 10,
        "plots": [
            {"x": 5, "y": 4, "t": "TERRAIN_GRASS", "o": 0},
            {"x": 6, "y": 4, "t": "TERRAIN_GRASS", "o": 0}
        ]
    }))
    .unwrap();
    let state: StateSnapshot = serde_json::from_value(serde_json::json!({
        "turn": 226, "gold": 200, "faith": 0,
        "cities": [{"id": 33, "name": "Bogota", "x": 5, "y": 4, "pop": 8,
            "districts": [{"type": "DISTRICT_INDUSTRIAL_ZONE", "x": 6, "y": 4, "complete": true}]}],
        "great_person_points": {"GREAT_PERSON_CLASS_MERCHANT": 660},
        "great_person_costs": {"GREAT_PERSON_CLASS_MERCHANT": 660},
        "great_person_offers": {"GREAT_PERSON_CLASS_MERCHANT": {
            "individual": "GREAT_PERSON_INDIVIDUAL_MASARU_IBUKA",
            "required_district": "DISTRICT_INDUSTRIAL_ZONE"
        }}
    }))
    .unwrap();
    (Snapshot::from_chunks(&[chunk]), state)
}

#[test]
fn native_gp_mirror_uses_host_price_without_the_unrelated_gold_reward() {
    let (snapshot, state) = mirror_offer();
    let mut game = crate::mirror::rebuild_from_state(&snapshot, &state, 2, 1, 300, 0).game;
    assert_eq!(
        game.live_great_person_offer_individual(0, "merchant"),
        Some("masaru_ibuka")
    );
    assert_eq!(game.gp_cost(0, "merchant"), 660.0);
    assert_eq!(game.live_great_person_offer_blocker(0, "merchant"), None);
    recruit(&mut game, "merchant").unwrap();
    assert_eq!(game.players[0].gold, 200.0);
    assert_eq!(game.players[0].gpp["merchant"], 0.0);
    assert!(!game.great_person_class_offered_now(0, "merchant"));
}

#[test]
fn native_gp_next_host_snapshot_can_restore_a_consumed_offer() {
    let (snapshot, mut state) = mirror_offer();
    let mut mirror = crate::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    recruit(&mut mirror.game, "merchant").unwrap();
    assert!(!mirror.game.great_person_class_offered_now(0, "merchant"));
    // The native request may fail or expose another person. Either way, the
    // next host frame replaces the plan's consumed-offer projection.
    state.frame += 1;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.great_person_class_offered_now(0, "merchant"));
    assert_eq!(mirror.game.players[0].gpp["merchant"], 660.0);
    assert_eq!(mirror.game.players[0].gold, 200.0);
    assert_eq!(
        mirror
            .game
            .live_great_person_offer_individual(0, "merchant"),
        Some("masaru_ibuka")
    );
    recruit(&mut mirror.game, "merchant").unwrap();
    assert_eq!(mirror.game.players[0].gold, 200.0);
}
