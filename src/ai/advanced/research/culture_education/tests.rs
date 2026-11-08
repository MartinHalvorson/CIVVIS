use super::*;

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
            "apprenticeship",
        ]
        .map(Name::new),
    );
    g.players[0].research = None;
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.turn = 65;
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
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
fn culture_education_opens_a_ready_university_chain() {
    let (mut g, mut ai, plan, _) = equipped();
    assert_eq!(ai.culture_education_goal(&g, 0), Some("education"));
    assert!(g.available_techs(0).contains(&crate::name!("education")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("education"));
}

#[test]
fn culture_education_requires_the_owned_campus_and_library() {
    let (mut g, ai, _, cid) = equipped();
    g.cities.get_mut(&cid).unwrap().buildings.clear();
    assert_eq!(ai.culture_education_goal(&g, 0), None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    g.cities.get_mut(&cid).unwrap().districts.clear();
    assert_eq!(ai.culture_education_goal(&g, 0), None);
    let pos = g.cities[&cid].pos;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), pos);
    g.cities.get_mut(&cid).unwrap().owner = 1;
    assert_eq!(ai.culture_education_goal(&g, 0), None);
}

#[test]
fn culture_education_ends_at_education_and_leaves_other_targets_alone() {
    let (mut g, ai, _, cid) = equipped();
    for target in VictoryTarget::ALL
        .into_iter()
        .filter(|t| *t != VictoryTarget::Culture)
    {
        assert_eq!(
            AdvancedAi::targeting(target).culture_education_goal(&g, 0),
            None
        );
    }
    g.players[0].techs.insert(crate::name!("education"));
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("university"));
    assert_eq!(
        ai.unreachable_science_building_tech(&g, 0),
        Some("chemistry")
    );
    assert_eq!(ai.culture_education_goal(&g, 0), None);
}

#[test]
fn culture_education_preserves_committed_research_and_rush_priority() {
    let (mut g, mut ai, mut plan, _) = equipped();
    g.players[0].research = Some(crate::name!("construction"));
    g.players[0].research_progress = 9.0;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("construction"));
    assert_eq!(g.players[0].research_progress, 9.0);
    g.players[0].research = None;
    g.players[0].research_progress = 0.0;
    plan.rush = true;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("horseback_riding"));
}
