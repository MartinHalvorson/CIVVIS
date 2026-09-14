use super::*;
use crate::ai::advanced::{genes, StrategicPlan};
use crate::game::Action;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, u32) {
    let mut g = Game::new_full(2, 32, 24, 914_3573, 300, 0, false);
    g.current = 0;
    g.turn = 40;
    g.game_speed = GameSpeed::Standard;
    g.players[0].civ = "Rome".to_string();
    let pos = g.units[&g.player_unit_ids(0)[0]].pos;
    let cid = g.found_city_for(0, pos, None);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));
    g.players[0].government = Some("classical_republic".to_string());
    g.players[0].civic = None;
    g.players[0].civic_progress = 0.0;
    g.players[0].civic_overflow = 0.0;
    g.players[0].boosted_civics.clear();
    // Keep the two real tier-two choices and the current government, so the
    // test can name the whole candidate set while retaining real civic paths.
    Arc::make_mut(&mut g.rules).governments.retain(|name, _| {
        matches!(
            name,
            "chiefdom" | "classical_republic" | "monarchy" | "merchant_republic"
        )
    });
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            culture: 50.0,
            ..Yields::default()
        },
    );
    (g, cid)
}

fn grant_ancestors(g: &mut Game, goal: &str) {
    let path = g.rules.civic_ancestors[goal]
        .iter()
        .map(|civic| Name::new(civic))
        .collect::<Vec<_>>();
    g.players[0].civics.extend(path);
}

fn candidate() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_government_ladder_3();
    ai
}

fn goal(ai: &AdvancedAi, g: &Game) -> Option<&'static str> {
    ai.government_ladder_goal(g, 0, GrandStrategy::Science)
}

#[test]
fn the_third_version_is_independent_and_older_toggles_release_it() {
    let gene = genes::gene("government-ladder-3").unwrap();
    assert!(gene.screenable() && gene.opt_in());
    let mut ai = AdvancedAi::new();
    assert!(!ai.government_ladder_3 && !AdvancedAi::legacy().government_ladder_3);
    ai.enable_government_ladder();
    ai.enable_government_ladder_2();
    (gene.enable)(&mut ai);
    assert!(!ai.government_ladder && !ai.government_ladder_2 && ai.government_ladder_3);
    ai.enable_government_ladder();
    assert!(!ai.government_ladder_3);
    (gene.enable)(&mut ai);
    ai.enable_government_ladder_2();
    assert!(!ai.government_ladder_3);
    (gene.enable)(&mut ai);
    (gene.disable)(&mut ai);
    assert!(!ai.government_ladder && !ai.government_ladder_2 && !ai.government_ladder_3);
}

#[test]
fn the_real_remaining_path_can_reverse_the_printed_cost_order() {
    let (mut g, _) = board();
    grant_ancestors(&mut g, "exploration");
    assert!(!g.players[0].civics.contains(&crate::name!("divine_right")));
    assert!(g.rules.civics["divine_right"].cost < g.rules.civics["exploration"].cost);
    assert!(
        AdvancedAi::government_route_cost(&g, 0, crate::name!("divine_right"))
            > AdvancedAi::government_route_cost(&g, 0, crate::name!("exploration"))
    );
    let mut v2 = AdvancedAi::new();
    v2.enable_government_ladder_2();
    assert_eq!(goal(&v2, &g), Some("divine_right"));
    assert_eq!(goal(&candidate(), &g), Some("exploration"));
}

#[test]
fn the_research_entry_point_reaches_the_government_that_is_then_adopted() {
    let (mut g, _) = board();
    grant_ancestors(&mut g, "exploration");
    let ai = candidate();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].civic.as_deref(), Some("exploration"));
    g.players[0].civics.insert(crate::name!("exploration"));
    ai.strategic_government(&mut g, 0, GrandStrategy::Science);
    assert_eq!(
        g.players[0].government.as_deref(),
        Some("merchant_republic")
    );
}

#[test]
fn extra_slots_are_priced_per_unit_of_remaining_culture() {
    let (mut g, _) = board();
    grant_ancestors(&mut g, "divine_right");
    grant_ancestors(&mut g, "exploration");
    assert_eq!(goal(&candidate(), &g), Some("divine_right"));
    Arc::make_mut(&mut g.rules)
        .governments
        .get_mut("monarchy")
        .unwrap()
        .slots
        .wildcard -= 1;
    assert_eq!(
        goal(&candidate(), &g),
        Some("exploration"),
        "two slots at 440 beat one at 340"
    );
}

#[test]
fn an_earned_inspiration_can_change_the_cheapest_route() {
    let (mut g, _) = board();
    grant_ancestors(&mut g, "divine_right");
    grant_ancestors(&mut g, "exploration");
    assert_eq!(goal(&candidate(), &g), Some("divine_right"));
    g.players[0]
        .boosted_civics
        .insert(crate::name!("exploration"));
    assert_eq!(goal(&candidate(), &g), Some("exploration"));
}

#[test]
fn the_route_bill_matches_engine_credit_for_china_and_current_progress() {
    let (mut g, _) = board();
    grant_ancestors(&mut g, "exploration");
    g.players[0].civ = "China".to_string();
    g.game_speed = GameSpeed::Online;
    let civic = crate::name!("exploration");
    g.players[0].boosted_civics.insert(civic);
    let before = AdvancedAi::government_route_cost(&g, 0, civic);
    g.apply(0, &Action::Civic { civic }).unwrap();
    assert_eq!(
        before,
        g.civic_cost("exploration") - g.players[0].civic_progress
    );
    assert_eq!(AdvancedAi::government_route_cost(&g, 0, civic), before);
    g.players[0].civic_progress += 7.0;
    assert_eq!(
        AdvancedAi::government_route_cost(&g, 0, civic),
        before - 7.0
    );
}

#[test]
fn shared_prerequisites_and_overflow_are_counted_only_once() {
    let (mut g, _) = board();
    let civic = crate::name!("exploration");
    let path = g.rules.civic_ancestors["exploration"]
        .iter()
        .map(|node| Name::new(node))
        .chain(std::iter::once(civic))
        .filter(|node| !g.players[0].civics.contains(node))
        .collect::<BTreeSet<_>>();
    let expected: f64 = path.iter().map(|node| g.civic_cost(node.as_str())).sum();
    assert_eq!(AdvancedAi::government_route_cost(&g, 0, civic), expected);
    g.players[0].civic_overflow = 200.0;
    assert_eq!(
        AdvancedAi::government_route_cost(&g, 0, civic),
        expected - 200.0
    );
}

#[test]
fn a_path_without_time_to_use_the_government_stands_down() {
    let (mut g, cid) = board();
    grant_ancestors(&mut g, "divine_right");
    grant_ancestors(&mut g, "exploration");
    let current = g.city_yields(cid).culture;
    let adjustment = g.observed_city_yield_adjustments[&cid].culture;
    Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .get_mut(&cid)
        .unwrap()
        .culture = adjustment - current + 1.0;
    assert!((g.city_yields(cid).culture - 1.0).abs() < 1e-9);
    assert_eq!(goal(&candidate(), &g), None);
    let mut v2 = AdvancedAi::new();
    v2.enable_government_ladder_2();
    assert!(goal(&v2, &g).is_some());
}

#[test]
fn v2s_first_rung_and_behind_field_windows_are_preserved() {
    let (mut g, _) = board();
    let ai = candidate();
    g.players[0]
        .civics
        .remove(&crate::name!("political_philosophy"));
    assert_eq!(goal(&ai, &g), None);
    g.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));
    assert!(goal(&ai, &g).is_some());
    g.turn = 160;
    g.players[1].government = Some("classical_republic".to_string());
    assert_eq!(goal(&ai, &g), None);
    g.players[1].government = Some("monarchy".to_string());
    assert!(goal(&ai, &g).is_some());
    g.turn = 230;
    assert_eq!(goal(&ai, &g), None);
}

#[test]
fn an_already_unlocked_or_non_upgrading_government_is_not_researched() {
    let (mut g, _) = board();
    g.players[0]
        .civics
        .extend([crate::name!("divine_right"), crate::name!("exploration")]);
    assert_eq!(goal(&candidate(), &g), None);
    g.players[0].civics.remove(&crate::name!("divine_right"));
    g.players[0].government = Some("merchant_republic".to_string());
    assert_eq!(
        goal(&candidate(), &g),
        None,
        "a lateral six-slot government is not an upgrade"
    );
}
