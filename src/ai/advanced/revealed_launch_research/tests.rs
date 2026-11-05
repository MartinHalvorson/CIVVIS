use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::HostResearchQuote;

fn fixture() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new(2, 24, 16, 21, 250, 0);
    g.turn = 179;
    g.world_era = 8;
    g.players[0].techs = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|t| !["advanced_ai", "smart_materials", "offworld_mission"].contains(&t.as_str()))
        .collect();
    g.players[0].research = Some("advanced_ai".into());
    g.players[0].research_progress = 383.773;
    g.players[0]
        .science_projects
        .insert("launch_moon_landing".into());
    g.set_host_research_prerequisites(BTreeMap::from([(
        crate::name!("smart_materials"),
        vec![crate::name!("seasteads")],
    )]))
    .unwrap();
    g.host_research_quotes = Arc::new(BTreeMap::from([(
        0,
        BTreeMap::from([
            (
                crate::name!("advanced_ai"),
                HostResearchQuote {
                    cost: Some(1150.0),
                    progress: Some(383.773),
                },
            ),
            (
                crate::name!("smart_materials"),
                HostResearchQuote {
                    cost: Some(1150.0),
                    progress: Some(0.0),
                },
            ),
        ]),
    )]));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_revealed_launch_tech_takes_the_slot();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 179,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn treatment_ships_off_in_both_controllers() {
    opt_in_off_in_both_controllers("revealed-launch-tech-takes-the-slot", |ai| {
        ai.revealed_launch_tech_takes_the_slot
    });
}

#[test]
fn native_queue_yields_to_the_revealed_milestone_and_replans_do_not_repeat_it() {
    let (mut g, ai, plan) = fixture();
    let start = g.log.len();
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("smart_materials"));
    assert_eq!(g.players[0].research_progress, 0.0);
    assert_eq!(
        g.host_research_quotes[&0][&crate::name!("advanced_ai")].progress,
        Some(383.773)
    );
    ai.advanced_research(&mut g, 0, &plan);
    let research: Vec<_> = g
        .log
        .since(start)
        .filter(|(_, a)| matches!(a, Action::Research { .. }))
        .collect();
    assert_eq!(research.len(), 1);
}

#[test]
fn off_missing_hidden_and_prerequisite_cases_keep_the_incumbent() {
    for case in 0..5 {
        let (mut g, mut ai, plan) = fixture();
        match case {
            0 => ai.disable_revealed_launch_tech_takes_the_slot(),
            1 => g.set_host_research_prerequisites(BTreeMap::new()).unwrap(),
            2 => g.host_research_quotes = Arc::new(BTreeMap::new()),
            3 => g
                .set_host_research_prerequisites(BTreeMap::from([(
                    crate::name!("smart_materials"),
                    vec![crate::name!("advanced_ai")],
                )]))
                .unwrap(),
            _ => {
                g.players[0].science_projects.clear();
            }
        }
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(
            g.players[0].research.as_deref(),
            Some("advanced_ai"),
            "case {case}"
        );
        assert_eq!(g.players[0].research_progress, 383.773);
    }
}

#[test]
fn earlier_research_priorities_cannot_turn_the_reconsideration_into_another_switch() {
    let (mut g, ai, mut plan) = fixture();
    g.players[0].techs.remove(&crate::name!("horseback_riding"));
    plan.rush = true;
    assert_eq!(
        ai.revealed_launch_research_switch(&g, 0),
        Some("smart_materials")
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("advanced_ai"));
    assert_eq!(g.players[0].research_progress, 383.773);
}
