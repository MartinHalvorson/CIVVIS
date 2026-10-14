use super::super::city_campaign::CampaignPlan;
use super::tests::{at, conquest, flat_board, on, war};
use super::*;

/// Player 1 holds two cities walled 300 at a city strength of 50, both on
/// the campaign; the plan targets the first. Catapults are buildable and
/// none of ours stands within the muster's reach of either.
fn two_walled_cities() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = flat_board(611_207, &[at(4, 10), at(24, 6)], false);
    g.found_city_for(1, at(26, 15), None);
    war(&mut g, 0, 1);
    let first = g.city_at(at(24, 6)).unwrap();
    let second = g.city_at(at(26, 15)).unwrap();
    for cid in [first, second] {
        g.cities.get_mut(&cid).unwrap().wall_hp = 300;
        std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 50.0);
    }
    g.players[0]
        .techs
        .extend(["masonry", "engineering"].map(crate::name::Name::new));
    for pos in [at(4, 11), at(5, 10), at(3, 10), at(5, 11)] {
        g.spawn_test_unit("warrior", 0, pos);
    }
    let mut ai = on();
    ai.enable_city_campaign_2();
    ai.campaign = Some(CampaignPlan {
        target: 1,
        cities: vec![first, second],
        requirement: 150.0,
        bodies: 5,
        planned: 50,
        declared: Some(55),
        taken: 0,
    });
    let plan = conquest(&g, Some(first));
    (g, ai, plan, first, second)
}

fn siege_guns(ai: &AdvancedAi, cid: u32) -> usize {
    ai.objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Siege(cid))
        .expect("a Siege row")
        .requirement
        .siege
}

/// Off, both rows ask the shipped count; on, the campaign target's row asks
/// the matched four and the campaign's next city keeps the shipped count.
#[test]
fn only_the_campaign_targets_row_asks_the_matched_guns() {
    let (g, mut ai, plan, first, second) = two_walled_cities();
    ai.rebuild_force_groups(&g, 0, &plan);
    let stock = ai.siege_requirement(&g, 0, first).siege;
    assert_eq!(siege_guns(&ai, first), stock);
    assert_eq!(
        siege_guns(&ai, second),
        ai.siege_requirement(&g, 0, second).siege
    );

    let (g, mut ai, plan, first, second) = two_walled_cities();
    ai.enable_breakers_match_the_walls();
    ai.rebuild_force_groups(&g, 0, &plan);
    assert_eq!(ai.breakers_matched_guns(&g, 0, first), Some(4));
    assert!(4 > stock);
    assert_eq!(siege_guns(&ai, first), 4, "the target's walls ask four");
    assert_eq!(
        siege_guns(&ai, second),
        ai.siege_requirement(&g, 0, second).siege,
        "a campaign city that is not the target keeps the shipped count"
    );
}
