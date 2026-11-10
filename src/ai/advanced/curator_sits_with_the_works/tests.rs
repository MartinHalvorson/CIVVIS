use super::*;
use crate::game::GovernorState;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// Two cities of ours, Pingala sitting in the first since turn 0 with
/// `promotions`, a Recovery plan, and the given Writing/Art/Music works housed
/// in each city (`works[0]` in Pingala's, `works[1]` in the other).
fn board(promotions: &[&str], works: [usize; 2]) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    let barbarians: Vec<u32> = g
        .units
        .iter()
        .filter(|(_, unit)| g.players[unit.owner].is_barbarian)
        .map(|(id, _)| *id)
        .collect();
    for id in barbarians {
        g.remove_unit(id);
    }
    g.barb_pid = None;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let old = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    let new = g.found_city_for(0, (10, 10), None);
    for cid in [old, new] {
        g.cities.get_mut(&cid).unwrap().queue.clear();
    }
    g.players[0].governor_roster.insert(
        "pingala".to_string(),
        GovernorState {
            city: Some(old),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: promotions.iter().map(|p| p.to_string()).collect(),
        },
    );
    g.players[0].governors = vec![old];
    g.turn = g.standard_duration(16) * 8;
    house(&mut g, old, works[0]);
    house(&mut g, new, works[1]);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_curator_sits_with_the_works();
    (g, ai, plan, old, new)
}

/// House `count` Writing works in `city` as the host reports them.
fn house(g: &mut Game, city: u32, count: usize) {
    let mut housing = g
        .observed_great_work_housing
        .as_deref()
        .cloned()
        .unwrap_or_default();
    housing.insert(city, [("writing".to_string(), count)].into_iter().collect());
    g.observed_great_work_housing = Some(Arc::new(housing));
}

const TREE: [&str; 4] = ["connoisseur", "researcher", "grants", "curator"];

#[test]
fn curator_sits_with_the_works_is_reversible_and_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "curator-sits-with-the-works",
        |ai| ai.curator_sits_with_the_works,
    );
}

#[test]
fn the_curator_moves_to_the_works_under_the_recovery_plan() {
    let (mut g, mut ai, plan, old, new) = board(&TREE, [0, 3]);
    assert_eq!(ai.curator_destination(&g, 0, &plan), Some((new, 3, 0)));
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
    assert_ne!(Some(old), g.players[0].governor_roster["pingala"].city);
}

#[test]
fn the_curator_stays_without_a_clear_margin_or_inside_the_stay() {
    let (g, ai, plan, _, _) = board(&TREE, [2, 3]);
    assert_eq!(
        ai.curator_destination(&g, 0, &plan),
        None,
        "one more is no margin"
    );
    let (g, ai, plan, _, new) = board(&TREE, [2, 5]);
    assert_eq!(ai.curator_destination(&g, 0, &plan), Some((new, 5, 2)));
    let (mut g, ai, plan, _, _) = board(&TREE, [2, 5]);
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = g.turn - 1;
    assert_eq!(
        ai.curator_destination(&g, 0, &plan),
        None,
        "inside the stay"
    );
    // A Pingala sitting with no works leaves as soon as he may.
    let (mut g, ai, plan, _, new) = board(&TREE, [0, 3]);
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = g.turn - 1;
    assert_eq!(ai.curator_destination(&g, 0, &plan), Some((new, 3, 0)));
}

#[test]
fn no_curator_another_governor_or_another_target_stays_put() {
    let (g, ai, plan, _, _) = board(&["connoisseur", "researcher", "grants"], [0, 5]);
    assert_eq!(ai.curator_destination(&g, 0, &plan), None, "no Curator");

    let (mut g, ai, plan, _, new) = board(&TREE, [0, 5]);
    let mut victor = g.players[0].governor_roster["pingala"].clone();
    victor.city = Some(new);
    g.players[0].governor_roster.insert("victor".into(), victor);
    assert_eq!(ai.curator_destination(&g, 0, &plan), None, "occupied");

    let (mut g, _, plan, old, _) = board(&TREE, [0, 5]);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_curator_sits_with_the_works();
    assert_eq!(
        science.curator_destination(&g, 0, &plan),
        None,
        "science seat"
    );
    let mut off = AdvancedAi::targeting(VictoryTarget::Culture);
    off.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(
        g.players[0].governor_roster["pingala"].city,
        Some(old),
        "gene off"
    );
}

#[test]
fn curator_waits_for_four_works_and_space_initiative_never_comes() {
    let (mut g, ai, plan, old, new) = board(&["connoisseur", "researcher", "grants"], [1, 2]);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 10);
    assert!(ai.curator_withholds_promotion(&g, 0, "pingala", "curator"));
    assert!(ai.curator_withholds_promotion(&g, 0, "pingala", "space_initiative"));
    assert!(!ai.curator_withholds_promotion(&g, 0, "magnus", "space_initiative"));
    assert_eq!(
        ai.preferred_governor_promotion(&g, 0, plan.strategy, "pingala"),
        None,
        "three works: neither Curator nor Space Initiative"
    );
    assert!(!ai.promote_the_curator(&mut g, 0));

    house(&mut g, old, 2);
    house(&mut g, new, 2);
    assert!(!ai.curator_withholds_promotion(&g, 0, "pingala", "curator"));
    assert_eq!(
        ai.preferred_governor_promotion(&g, 0, plan.strategy, "pingala")
            .as_deref(),
        Some("curator")
    );
    assert!(ai.promote_the_curator(&mut g, 0));
    assert!(g.players[0].governor_roster["pingala"]
        .promotions
        .contains("curator"));

    // Off, the Recovery plan's order stands: Space Initiative before Curator.
    let (mut g, _, plan, _, _) = board(&["connoisseur", "researcher", "grants"], [3, 3]);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 10);
    let off = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(
        off.preferred_governor_promotion(&g, 0, plan.strategy, "pingala")
            .as_deref(),
        Some("space_initiative")
    );
}
