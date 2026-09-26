use super::super::objective_board::ObjectiveKey;
use super::tests::plan_against;
use super::*;

fn staging_world(bodies: usize) -> (Game, u32) {
    let mut g = Game::new_full(2, 40, 24, 926_760, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (2, 10), None);
    let cid = g.found_city_for(1, (18, 10), None);
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    g.turn = 80;
    // A tech edge discounts the strategic campaign bill, but the deployed
    // siege controller still requires the undiscounted tactical strength.
    g.players[0]
        .techs
        .extend(g.rules.techs.keys().take(20).copied());
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    for _ in 0..bodies {
        g.spawn_unit("warrior", 0, (15, 10));
    }
    g.spawn_unit("warrior", 1, (17, 10));
    (g, cid)
}

#[test]
fn supplied_siege_can_clear_its_own_staging_requirement() {
    let (g, cid) = staging_world(12);
    let city = CityView::of(&g, cid).unwrap();
    let tactical = siege_bill(&g, 0, &city);
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    ai.disable_siege_train();
    ai.disable_siege_positive_damage_budget();
    let strategic = ai.siege_requirement(&g, 0, cid).strength;
    assert!(
        strategic < tactical,
        "fixture must expose the conflicting bills: {strategic} / {tactical}"
    );
    ai.enable_siege_train();
    let plan = plan_against(&g, cid);
    ai.rebuild_force_groups(&g, 0, &plan);
    let group = ai
        .force_groups
        .iter()
        .find(|f| f.objective == city.pos && f.domain == ForceDomain::Land)
        .unwrap()
        .clone();
    let force = ai.siege_force(&g, 0, &city, &plan, &group);
    let allocated: f64 = force.iter().map(|uid| unit_power(&g, *uid)).sum();
    assert!(
        allocated >= tactical,
        "allocator supplied {allocated} but staging requires {tactical}"
    );
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Invest);
}

#[test]
fn undersupplied_siege_requests_the_missing_staging_strength() {
    let (g, cid) = staging_world(1);
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    ai.enable_siege_train();
    ai.rebuild_force_groups(&g, 0, &plan_against(&g, cid));
    let row = ai
        .objective_board()
        .rows
        .iter()
        .find(|r| r.key == ObjectiveKey::Siege(cid))
        .unwrap();
    assert!(row.requirement.strength >= siege_bill(&g, 0, &city));
    assert!(ai
        .requisitions()
        .iter()
        .any(|r| r.label == row.label && r.unmet.strength > 0.0));
}

#[test]
fn staging_floor_only_applies_with_the_doctrine_and_never_reduces_a_budget() {
    let (mut g, cid) = staging_world(12);
    let mut ai = AdvancedAi::new();
    ai.disable_siege_train();
    ai.disable_siege_positive_damage_budget();
    let strategic = ai.siege_requirement(&g, 0, cid);
    ai.enable_siege_positive_damage_budget();
    assert!(ai.siege_requirement(&g, 0, cid).strength > strategic.strength);
    ai.disable_siege_positive_damage_budget();
    assert_eq!(ai.siege_requirement(&g, 0, cid), strategic);
    // Without a tech advantage, the campaign's larger margin must survive.
    g.players[0].techs.clear();
    let strategic = ai.siege_requirement(&g, 0, cid);
    assert!(strategic.strength > staging_strength_requirement(&g, 0, cid));
    ai.enable_siege_train();
    assert_eq!(ai.siege_requirement(&g, 0, cid), strategic);
}

#[test]
fn a_peacetime_campaign_keeps_its_existing_declaration_requirement() {
    let (mut g, cid) = staging_world(12);
    g.at_war.clear();
    let mut ai = AdvancedAi::new();
    ai.disable_siege_train();
    ai.disable_siege_positive_damage_budget();
    let strategic = ai.siege_requirement(&g, 0, cid);
    ai.enable_siege_train();
    assert_eq!(ai.siege_requirement(&g, 0, cid), strategic);
    ai.disable_siege_train();
    ai.enable_siege_positive_damage_budget();
    assert_eq!(ai.siege_requirement(&g, 0, cid), strategic);
}
