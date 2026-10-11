use super::*;
use crate::{ai::VictoryTarget, name};
use std::sync::Arc;

/// Two founded empires at turn 110, ours with Military Engineering and its
/// ancestors known (Trebuchets unlocked), 20 Niter stocked, and science set
/// high enough that Metal Casting is a few turns away; the target city of the
/// Conquest plan stands behind `walls`.
fn board(walls: i32, science: f64) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 30, 20, 109_102_000, 300, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    g.current = 0;
    g.turn = 110;
    g.record_contact(0, 1);
    let ancestors = g.rules.tech_ancestors["military_engineering"].clone();
    g.players[0]
        .techs
        .extend(ancestors.iter().map(|tech| Name::new(tech)));
    g.players[0].techs.insert(name!("military_engineering"));
    g.players[0]
        .strategic_resources
        .insert(name!("niter"), 20.0);
    let mut adjustments = std::collections::BTreeMap::new();
    adjustments.insert(
        0,
        crate::rules::Yields {
            science,
            ..Default::default()
        },
    );
    g.observed_yield_adjustments = Arc::new(adjustments);
    let target = g.player_city_ids(1)[0];
    g.cities.get_mut(&target).unwrap().wall_hp = walls;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_domination_siege_research_2();
    ai.battlefront_observation = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

fn pick(g: &Game, ai: &AdvancedAi, plan: &StrategicPlan) -> Option<SiegeTierPick> {
    let goal = ai.domination_siege_research_goal(g, 0, plan);
    ai.siege_tier_before_the_beeline_goal(g, 0, plan, goal)
}

#[test]
fn medieval_walls_promote_the_next_fed_siege_tier_when_it_is_cheap() {
    let (g, mut ai, plan) = board(300, 2_000.0);
    assert_eq!(
        ai.domination_siege_research_goal(&g, 0, &plan),
        Some(name!("metal_casting")),
        "the fixture's siege goal is the Bombard's technology"
    );
    assert!(pick(&g, &ai, &plan).is_none(), "off by default");
    ai.enable_siege_tier_before_the_beeline();
    let chosen = pick(&g, &ai, &plan).expect("cheap Bombard path behind 300 walls");
    assert_eq!(chosen.tech, name!("metal_casting"));
    assert!(chosen.walls >= SIEGE_TIER_WALL_HP);
    assert!(chosen.turns <= g.standard_duration(SIEGE_TIER_MAX_RESEARCH_STANDARD) as f64);
    assert!(chosen.bomber_techs > SIEGE_TIER_YIELDS_WITHIN_TECHS);
    ai.disable_siege_tier_before_the_beeline();
    assert!(pick(&g, &ai, &plan).is_none());
}

#[test]
fn walls_below_the_medieval_tier_leave_the_beeline_alone() {
    let (mut g, mut ai, plan) = board(300, 2_000.0);
    ai.enable_siege_tier_before_the_beeline();
    let target = plan.target_city.unwrap();
    g.cities.get_mut(&target).unwrap().wall_hp = 100;
    if g.city_max_wall_hp(&g.cities[&target]) < SIEGE_TIER_WALL_HP {
        assert!(pick(&g, &ai, &plan).is_none());
    }
    // No siege goal at all: nothing to promote.
    assert!(ai
        .siege_tier_before_the_beeline_goal(&g, 0, &plan, None)
        .is_none());
}

#[test]
fn a_dear_path_or_a_near_bomber_keeps_the_beeline() {
    let (g, mut ai, plan) = board(300, 0.5);
    ai.enable_siege_tier_before_the_beeline();
    assert!(
        pick(&g, &ai, &plan).is_none(),
        "half a beaker a turn makes Metal Casting far dearer than the cap"
    );
    let (mut g, mut ai, plan) = board(300, 2_000.0);
    ai.enable_siege_tier_before_the_beeline();
    // Every Bomber prerequisite known except Metal Casting itself: the Bomber
    // is at most two technologies away and keeps the slot.
    let goal = super::super::air_surge::AIR_SURGE_GOAL_TECH;
    let chain: Vec<Name> = g.rules.tech_ancestors[goal]
        .iter()
        .map(|tech| Name::new(tech))
        .filter(|tech| *tech != name!("metal_casting"))
        .collect();
    g.players[0].techs.extend(chain);
    assert!(AdvancedAi::air_surge_missing_techs(&g, 0) <= SIEGE_TIER_YIELDS_WITHIN_TECHS);
    let goal = ai.domination_siege_research_goal(&g, 0, &plan);
    assert!(ai
        .siege_tier_before_the_beeline_goal(&g, 0, &plan, goal)
        .is_none());
}

#[test]
fn the_promoted_goal_owns_the_research_slot() {
    let (mut g, mut ai, plan) = board(300, 2_000.0);
    ai.enable_siege_tier_before_the_beeline();
    ai.advanced_research(&mut g, 0, &plan);
    let research = g.players[0]
        .research
        .clone()
        .expect("a technology is chosen");
    let research = Name::new(&research);
    assert!(
        research == name!("metal_casting") || ai.tech_leads_to(&g, &research, "metal_casting"),
        "research {research:?} walks toward Metal Casting"
    );
}
