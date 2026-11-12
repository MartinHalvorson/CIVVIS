use super::*;

/// A Domination seat at t65 with one city holding a Campus and a Library, and
/// every prerequisite of Education known. Mirrors `culture_education`'s
/// fixture so the two contracts read side by side.
fn equipped() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 24, 16, 936_331, 500, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    let pos = g.cities[&cid].pos;
    let city = g.cities.get_mut(&cid).unwrap();
    city.districts.insert(crate::name!("campus"), pos);
    city.buildings.push(crate::name!("library"));
    g.players[0].techs.extend(
        [
            "pottery",
            "writing",
            "animal_husbandry",
            "mining",
            "archery",
            "masonry",
            "irrigation",
            "currency",
            "mathematics",
            "horseback_riding",
            "apprenticeship",
        ]
        .map(Name::new),
    );
    g.players[0].research = None;
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.turn = 65;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_education_follows_the_library();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, cid)
}

#[test]
fn education_follows_the_library_researches_education_for_the_university() {
    let (mut g, ai, plan, _) = equipped();
    assert_eq!(AdvancedAi::education_ready_campus_cities(&g, 0), 1);
    assert_eq!(ai.domination_education_goal(&g, 0), Some("education"));
    assert!(g.available_techs(0).contains(&crate::name!("education")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("education"));
}

#[test]
fn education_follows_the_library_waits_for_an_owned_campus_with_a_library() {
    let (mut g, ai, _, cid) = equipped();
    g.cities.get_mut(&cid).unwrap().buildings.clear();
    assert_eq!(ai.domination_education_goal(&g, 0), None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    g.cities.get_mut(&cid).unwrap().districts.clear();
    assert_eq!(ai.domination_education_goal(&g, 0), None);
    let pos = g.cities[&cid].pos;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), pos);
    g.cities.get_mut(&cid).unwrap().owner = 1;
    assert_eq!(ai.domination_education_goal(&g, 0), None);
}

#[test]
fn education_follows_the_library_is_opt_in_for_conquest_or_score_and_ends_at_education() {
    let (mut g, mut ai, _, _) = equipped();
    assert!(!AdvancedAi::targeting(VictoryTarget::Domination).education_follows_the_library);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).domination_education_goal(&g, 0),
        None
    );
    for target in VictoryTarget::ALL
        .into_iter()
        .filter(|t| !matches!(t, VictoryTarget::Domination | VictoryTarget::Score))
    {
        let mut other = AdvancedAi::targeting(target);
        other.enable_education_follows_the_library();
        assert_eq!(other.domination_education_goal(&g, 0), None);
    }
    g.players[0].techs.insert(crate::name!("education"));
    assert_eq!(ai.domination_education_goal(&g, 0), None);
    g.players[0].techs.remove(&crate::name!("education"));
    ai.disable_education_follows_the_library();
    assert_eq!(ai.domination_education_goal(&g, 0), None);
}

#[test]
fn education_follows_the_library_keeps_committed_research() {
    let (mut g, ai, plan, _) = equipped();
    g.players[0].research = Some("construction".to_string());
    g.players[0].research_progress = 9.0;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("construction"));
    assert_eq!(g.players[0].research_progress, 9.0);
}

/// Puts the `equipped` seat at war with a major, with a standing army of
/// Warriors whose Swordsman upgrade is still unresearched: the wartime
/// modernization chain is live.
fn modernizing(g: &mut Game, cid: u32) -> Name {
    let pos = g.cities[&cid].pos;
    for _ in 0..3 {
        g.spawn_test_unit("warrior", 0, pos);
    }
    g.at_war.insert((0, 1));
    AdvancedAi::targeting(VictoryTarget::Domination)
        .wartime_modernization_tech(g, 0)
        .expect("a live wartime modernization chain")
}

#[test]
fn education_goes_ahead_of_the_wartime_modernization_chain() {
    let (mut g, mut ai, plan, cid) = equipped();
    let chain = modernizing(&mut g, cid);
    assert_eq!(ai.wartime_modernization_tech(&g, 0), Some(chain));
    assert_eq!(AdvancedAi::education_missing_prerequisites(&g, 0), 0);
    assert_eq!(
        AdvancedAi::domination_education_ahead_of_modernization(
            &g,
            0,
            ai.domination_education_goal(&g, 0),
            Some(chain.as_str()),
        ),
        Some("education")
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("education"));

    // Gene off, the same board steps toward the modernization.
    g.players[0].research = None;
    ai.disable_education_follows_the_library();
    ai.advanced_research(&mut g, 0, &plan);
    let picked = g.players[0].research.clone().expect("a research pick");
    assert!(
        ai.tech_leads_to(&g, &picked, chain.as_str()),
        "{picked} is not on the way to {}",
        chain.as_str()
    );
}

#[test]
fn education_path_goes_ahead_of_modernization_up_to_two_technologies_short() {
    let (mut g, ai, plan, cid) = equipped();
    let chain = modernizing(&mut g, cid);

    // One short: Apprenticeship.
    g.players[0].techs.remove(&crate::name!("apprenticeship"));
    assert_eq!(AdvancedAi::education_missing_prerequisites(&g, 0), 1);
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("apprenticeship"));

    // Two short: Mathematics and Apprenticeship.
    g.players[0].research = None;
    g.players[0].techs.remove(&crate::name!("mathematics"));
    assert_eq!(AdvancedAi::education_missing_prerequisites(&g, 0), 2);
    ai.advanced_research(&mut g, 0, &plan);
    let picked = g.players[0].research.clone().expect("a research pick");
    assert!(
        ["mathematics", "apprenticeship"].contains(&picked.as_str()),
        "{picked} is not a step toward Education"
    );

    // Three short: Horseback Riding too. The modernization keeps its slot and
    // the gene waits in its own.
    g.players[0].research = None;
    g.players[0].techs.remove(&crate::name!("horseback_riding"));
    assert_eq!(AdvancedAi::education_missing_prerequisites(&g, 0), 3);
    assert_eq!(ai.domination_education_goal(&g, 0), Some("education"));
    assert_eq!(
        AdvancedAi::domination_education_ahead_of_modernization(
            &g,
            0,
            ai.domination_education_goal(&g, 0),
            Some(chain.as_str()),
        ),
        None
    );
    ai.advanced_research(&mut g, 0, &plan);
    let picked = g.players[0].research.clone().expect("a research pick");
    assert!(
        ai.tech_leads_to(&g, &picked, chain.as_str()),
        "{picked} is not on the way to {}",
        chain.as_str()
    );

    // No modernization goal, no early slot.
    assert_eq!(
        AdvancedAi::domination_education_ahead_of_modernization(&g, 0, Some("education"), None),
        None
    );
}

/// The decisive window's live Gran Colombia board (see
/// `decisive_window/tests.rs`): two founded empires at t110 and a conquest
/// planned on player 1, behind Siege Tactics walls. Our first city also holds
/// a Campus with a Library, Education is researchable, and two Knights at war
/// make the wartime modernization chain live, so the early slot is armed. The
/// window's research still wins.
#[test]
fn education_ahead_of_modernization_still_yields_to_the_decisive_window() {
    let mut g = Game::new_full(2, 30, 20, 109_104_000, 300, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
        let home = g.cities[&g.player_city_ids(pid)[0]].pos;
        let second = g
            .wdisk(home, 6)
            .into_iter()
            .find(|pos| {
                g.wdist(*pos, home) >= 4
                    && !g.rules.is_water(&g.map.tiles[pos])
                    && g.units_at(*pos).is_empty()
                    && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4)
            })
            .unwrap();
        g.found_city_for(pid, second, None);
    }
    g.current = 0;
    g.turn = 110;
    g.record_contact(0, 1);
    g.players[0].civ = "Gran Colombia".to_string();
    g.players[1].civ = "Egypt".to_string();
    for (pid, techs) in [
        (1, &["siege_tactics", "gunpowder", "military_tactics"][..]),
        (
            0,
            &[
                "printing",
                "castles",
                "gunpowder",
                "animal_husbandry",
                "mathematics",
            ][..],
        ),
    ] {
        for tech in techs {
            let ancestors = g.rules.tech_ancestors[*tech].clone();
            g.players[pid]
                .techs
                .extend(ancestors.iter().map(|tech| Name::new(tech)));
            g.players[pid].techs.insert(Name::new(tech));
        }
    }
    for resource in [crate::name!("horses"), crate::name!("niter")] {
        g.players[0].strategic_resources.insert(resource, 40.0);
    }
    // Every unknown technology quoted cheap, so the window's horizon is met.
    let unknown: Vec<Name> = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|tech| !g.players[0].techs.contains(tech))
        .collect();
    let quotes = std::sync::Arc::make_mut(&mut g.host_research_quotes)
        .entry(0)
        .or_default();
    for tech in unknown {
        quotes.insert(
            tech,
            crate::game::HostResearchQuote {
                cost: Some(5.0),
                progress: Some(0.0),
            },
        );
    }
    let cid = g.player_city_ids(0)[0];
    let pos = g.cities[&cid].pos;
    let city = g.cities.get_mut(&cid).unwrap();
    city.districts.insert(crate::name!("campus"), pos);
    city.buildings.push(crate::name!("library"));
    for _ in 0..2 {
        g.spawn_test_unit("knight", 0, pos);
    }
    g.at_war.insert((0, 1));
    g.players[0].research = None;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_decisive_window();
    ai.enable_education_follows_the_library();
    ai.battlefront_observation = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(g.available_techs(0).contains(&crate::name!("education")));
    let chain = ai
        .wartime_modernization_tech(&g, 0)
        .expect("a live wartime modernization chain");
    assert_eq!(
        AdvancedAi::domination_education_ahead_of_modernization(
            &g,
            0,
            ai.domination_education_goal(&g, 0),
            Some(chain.as_str()),
        ),
        Some("education")
    );
    let window = ai.decisive_window(&g, 0, &plan);
    let goal = ai
        .decisive_window_research_goal(&g, 0, window.as_ref())
        .expect("the window owns research");
    ai.advanced_research(&mut g, 0, &plan);
    let picked = g.players[0].research.clone().expect("a research pick");
    assert_ne!(picked, "education");
    assert!(
        ai.tech_leads_to(&g, &picked, goal.as_str()),
        "{picked} is not on the way to the window's {}",
        goal.as_str()
    );
}

/// `conquest_or_score_seat`: a Score seat researches Education behind its
/// Library as a Domination seat does; a Culture, Science or Religion seat
/// does not take this goal.
#[test]
fn a_score_seat_follows_the_library_and_other_targets_do_not() {
    let (g, _, _, _) = equipped();
    for (target, goal) in [
        (VictoryTarget::Domination, Some("education")),
        (VictoryTarget::Score, Some("education")),
        (VictoryTarget::Culture, None),
        (VictoryTarget::Science, None),
        (VictoryTarget::Religion, None),
    ] {
        let mut ai = AdvancedAi::targeting(target);
        ai.enable_education_follows_the_library();
        assert_eq!(ai.domination_education_goal(&g, 0), goal, "{target:?}");
    }
}
