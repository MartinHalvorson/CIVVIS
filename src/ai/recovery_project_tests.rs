use super::*;

fn recovery_city(projects: &[&str]) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 16, 91771, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.found_city_for(0, g.units[&settler].pos, None);
    let cid = g.player_city_ids(0)[0];
    let rules = std::sync::Arc::make_mut(&mut g.rules);
    rules.buildings.clear();
    rules.projects.retain(|name, _| projects.contains(&name));
    for spec in rules.projects.values_mut() {
        spec.district = None;
    }
    (g, cid)
}

#[test]
fn recovery_uses_science_instead_of_cost_tied_loyalty_project() {
    let (g, cid) = recovery_city(&["bread_and_circuses", "campus_research_grants"]);
    let item = BasicAi::new()
        .upkeep_free_recovery_item(&g, 0, cid)
        .unwrap();
    assert!(matches!(item, Item::Project { project } if project == "campus_research_grants"));
}

#[test]
fn recovery_prefers_gold_conversion_even_when_project_costs_more() {
    let (mut g, cid) = recovery_city(&[
        "campus_research_grants",
        "commercial_hub_investment",
        "harbor_shipping",
    ]);
    std::sync::Arc::make_mut(&mut g.rules)
        .projects
        .get_mut("commercial_hub_investment")
        .unwrap()
        .cost = 100.0;
    let item = BasicAi::new()
        .upkeep_free_recovery_item(&g, 0, cid)
        .unwrap();
    assert!(matches!(item, Item::Project { project } if project == "commercial_hub_investment"));
}

#[test]
fn recovery_keeps_last_available_project_and_respects_legality() {
    let (mut g, cid) = recovery_city(&["bread_and_circuses", "campus_research_grants"]);
    std::sync::Arc::make_mut(&mut g.rules)
        .projects
        .get_mut("campus_research_grants")
        .unwrap()
        .tech = Some(crate::name!("future_tech"));
    let item = BasicAi::new()
        .upkeep_free_recovery_item(&g, 0, cid)
        .unwrap();
    assert!(matches!(item, Item::Project { project } if project == "bread_and_circuses"));
}
