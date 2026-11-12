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
fn education_follows_the_library_is_opt_in_domination_only_and_ends_at_education() {
    let (mut g, mut ai, _, _) = equipped();
    assert!(!AdvancedAi::targeting(VictoryTarget::Domination).education_follows_the_library);
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).domination_education_goal(&g, 0),
        None
    );
    for target in VictoryTarget::ALL
        .into_iter()
        .filter(|t| *t != VictoryTarget::Domination)
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
