use super::*;
use crate::game::HostResearchQuote;

fn watched() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 24, 16, 936_221, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for pid in [0, 1] {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let city = g.player_city_ids(0)[0];
    let home = g.cities[&city].pos;
    let mut ground: Vec<_> = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| g.wdist(home, **pos) == 4 && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    ground.sort();
    g.spawn_unit("archer", 1, ground[0]);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.turn = 20;
    g.host_observed = Arc::new(g.wdisk(home, 9).into_iter().collect());
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 65.0), (1, 216.0)]);
    g.players[0].techs.insert(crate::name!("mining"));
    g.players[0].research = Some("bronze_working".into());
    g.players[0].research_progress = 17.0;
    g.set_host_research_prerequisites(BTreeMap::from([(
        crate::name!("masonry"),
        vec![crate::name!("mining")],
    )]))
    .unwrap();
    g.host_research_quotes = Arc::new(BTreeMap::from([(
        0,
        BTreeMap::from([
            (
                crate::name!("bronze_working"),
                HostResearchQuote {
                    cost: Some(60.0),
                    progress: Some(17.0),
                },
            ),
            (
                crate::name!("masonry"),
                HostResearchQuote {
                    cost: Some(40.0),
                    progress: Some(5.0),
                },
            ),
        ]),
    )]));
    g.host_research_eligibility = Arc::new(BTreeMap::from([(
        0,
        BTreeMap::from([(crate::name!("masonry"), true)]),
    )]));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_capital_walls_before_the_surprise();
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.capital_walls_research_threat(&g, 0).is_some());
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, city)
}

#[test]
fn native_warning_switches_to_walls_and_banks_the_interrupted_work() {
    let (mut g, ai, plan, _) = watched();
    let start = g.log.len();
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));
    assert_eq!(g.players[0].research_progress, 5.0);
    assert_eq!(
        g.host_research_quotes[&0][&crate::name!("bronze_working")].progress,
        Some(17.0)
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(
        g.log
            .since(start)
            .filter(|(_, a)| matches!(a, Action::Research { .. }))
            .count(),
        1
    );
    g.apply(
        0,
        &Action::Research {
            tech: "bronze_working".into(),
        },
    )
    .unwrap();
    assert_eq!(
        g.players[0].research_progress, 17.0,
        "the interrupted node resumes its bank"
    );
}

#[test]
fn off_absent_threat_and_completed_walls_keep_the_incumbent() {
    for case in 0..5 {
        let (mut g, mut ai, plan, city) = watched();
        match case {
            0 => ai.disable_capital_walls_before_the_surprise(),
            1 => ai.capital_watch_sightings.clear(),
            2 => {
                g.cities
                    .get_mut(&city)
                    .unwrap()
                    .buildings
                    .push(crate::name!("walls"));
            }
            3 => {
                g.players[0].techs.insert(crate::name!("masonry"));
            }
            _ => {
                Arc::make_mut(&mut g.observed_military_power).insert(1, 80.0);
            }
        }
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(
            g.players[0].research.as_deref(),
            Some("bronze_working"),
            "case {case}"
        );
        assert_eq!(g.players[0].research_progress, 17.0);
    }
}

#[test]
fn unknown_illegal_or_unbanked_native_research_keeps_the_incumbent() {
    for case in 0..6 {
        let (mut g, ai, plan, _) = watched();
        match case {
            0 => {
                Arc::make_mut(&mut g.host_research_quotes)
                    .get_mut(&0)
                    .unwrap()
                    .remove(&crate::name!("bronze_working"));
            }
            1 => {
                Arc::make_mut(&mut g.host_research_quotes)
                    .get_mut(&0)
                    .unwrap()
                    .remove(&crate::name!("masonry"));
            }
            2 => {
                Arc::make_mut(&mut g.host_research_eligibility)
                    .get_mut(&0)
                    .unwrap()
                    .insert(crate::name!("masonry"), false);
            }
            3 => {
                Arc::make_mut(&mut g.host_research_eligibility).clear();
            }
            4 => {
                g.set_host_research_prerequisites(BTreeMap::new()).unwrap();
            }
            _ => {
                g.host_research_quotes = Arc::new(BTreeMap::new());
            }
        }
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(
            g.players[0].research.as_deref(),
            Some("bronze_working"),
            "case {case}"
        );
        assert_eq!(g.players[0].research_progress, 17.0);
    }
}

#[test]
fn research_already_serving_the_wall_goal_is_not_restarted() {
    for current in ["masonry", "mining"] {
        let (mut g, ai, plan, _) = watched();
        g.players[0].research = Some(current.into());
        let start = g.log.len();
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(g.players[0].research.as_deref(), Some(current));
        assert_eq!(g.players[0].research_progress, 17.0);
        assert_eq!(
            g.log
                .since(start)
                .filter(|(_, a)| matches!(a, Action::Research { .. }))
                .count(),
            0
        );
    }
}
