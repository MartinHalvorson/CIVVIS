use super::super::{GrandStrategy, StrategicPlan};
use super::*;
use crate::name;

fn board(cities: &[(usize, Pos)]) -> Game {
    let mut game = Game::new_full(3, 36, 22, 91_777, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for (pid, pos) in cities {
        assert!(
            game.map.tiles.contains_key(pos),
            "fixture city must be on the board"
        );
        game.found_city_for(*pid, *pos, None);
    }
    for pid in 0..3 {
        game.players[pid]
            .met
            .extend((0..3).filter(|other| *other != pid));
        game.players[pid]
            .explored
            .extend(game.map.tiles.keys().copied());
    }
    game.at_war.clear();
    game.at_war.insert((0, 1));
    game.turn = 60;
    game.current = 0;
    game
}

fn campaign(game: &Game) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_city_campaign_2();
    ai.campaign = Some(CampaignPlan {
        target: 1,
        cities: vec![
            game.city_at((14, 12)).unwrap(),
            game.city_at((17, 15)).unwrap(),
        ],
        requirement: 150.0,
        bodies: 5,
        planned: 40,
        declared: Some(50),
        taken: 0,
    });
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: game.city_at((17, 15)),
        threatened_city: game.city_at((6, 12)),
        desired_cities: 6,
        assessed_turn: 59,
        rush: false,
    });
    ai
}

#[test]
fn a_rebuilt_campaign_does_not_count_a_reused_friendly_id_as_a_capture() {
    let previous = board(&[(0, (6, 12)), (1, (14, 12)), (1, (17, 15)), (2, (24, 12))]);
    let mut next = board(&[(1, (17, 15)), (0, (6, 12)), (2, (24, 12)), (1, (14, 12))]);
    let mut ai = campaign(&previous);
    assert_eq!(
        next.cities[&ai.campaign.as_ref().unwrap().cities[0]].owner,
        0
    );
    let mut unmapped = ai.clone();
    unmapped.maintain_city_campaign(&mut next.clone(), 0);
    assert_eq!(
        unmapped.campaign.as_ref().unwrap().taken,
        1,
        "the unremapped control falsely counts our unchanged home as captured"
    );
    assert_eq!(next.cities[&next.city_at((14, 12)).unwrap()].owner, 1);
    ai.remap_campaign_city_memory(&previous, &next);
    ai.maintain_city_campaign(&mut next, 0);
    let plan = ai.campaign.as_ref().unwrap();
    assert_eq!(plan.taken, 0, "reallocated IDs are not captures");
    assert_eq!(
        plan.cities,
        vec![
            next.city_at((14, 12)).unwrap(),
            next.city_at((17, 15)).unwrap()
        ]
    );
    assert_eq!(plan.declared, Some(50));
    assert_eq!(plan.planned, 40);
}

#[test]
fn strategic_attack_and_defense_targets_follow_their_cities() {
    let previous = board(&[(0, (6, 12)), (1, (14, 12)), (1, (17, 15)), (2, (24, 12))]);
    let next = board(&[(1, (17, 15)), (0, (6, 12)), (2, (24, 12)), (1, (14, 12))]);
    let mut ai = campaign(&previous);
    assert_ne!(
        ai.plan.as_ref().unwrap().target_city,
        next.city_at((17, 15))
    );
    assert_ne!(
        ai.plan.as_ref().unwrap().threatened_city,
        next.city_at((6, 12))
    );
    ai.remap_campaign_city_memory(&previous, &next);
    let plan = ai.plan.as_ref().unwrap();
    assert_eq!(plan.target_city, next.city_at((17, 15)));
    assert_eq!(plan.threatened_city, next.city_at((6, 12)));
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.assessed_turn, 59);
}

#[test]
fn genuine_captures_are_counted_after_remapping_but_missing_cities_are_not() {
    let previous = board(&[(0, (6, 12)), (1, (14, 12)), (1, (17, 15)), (2, (24, 12))]);
    let mut next = board(&[(2, (24, 12)), (0, (6, 12)), (0, (14, 12))]);
    let mut ai = campaign(&previous);
    let mut unmapped = ai.clone();
    unmapped.maintain_city_campaign(&mut next.clone(), 0);
    assert_eq!(
        unmapped.campaign.as_ref().unwrap().taken,
        2,
        "the unremapped control confuses the missing objective with another city"
    );
    ai.remap_campaign_city_memory(&previous, &next);
    assert_eq!(
        ai.plan.as_ref().unwrap().target_city,
        None,
        "the missing objective has no replacement"
    );
    ai.maintain_city_campaign(&mut next, 0);
    let plan = ai.campaign.as_ref().unwrap();
    assert_eq!(plan.taken, 1, "only the city now actually ours is captured");
    assert!(plan.cities.is_empty());
}

#[test]
fn completed_campaign_history_survives_a_rebuild_until_peace() {
    let previous = board(&[(0, (6, 12)), (1, (14, 12)), (1, (17, 15)), (2, (24, 12))]);
    let mut ai = campaign(&previous);
    let plan = ai.campaign.as_mut().unwrap();
    plan.cities.clear();
    plan.taken = 2;
    let expected = plan.clone();
    ai.remap_campaign_city_memory(&previous, &previous);
    assert_eq!(ai.campaign, Some(expected));
}

#[test]
fn completed_foothold_does_not_end_the_active_domination_front() {
    let mut game = board(&[
        (0, (6, 12)),
        (1, (14, 12)), // Required original capital remains with the rival.
        (1, (17, 15)), // This border city is the campaign's first capture.
        (2, (24, 12)),
    ]);
    let border = game.city_at((17, 15)).unwrap();
    let mut ai = campaign(&game);
    ai.campaign.as_mut().unwrap().cities = vec![border];
    game.cities.get_mut(&border).unwrap().owner = 0;
    ai.maintain_city_campaign(&mut game, 0);
    assert_eq!(ai.campaign.as_ref().unwrap().taken, 1);
    assert!(ai.campaign.as_ref().unwrap().cities.is_empty());

    let mut ordinary = ai.clone();
    let mut ordinary_board = game.clone();
    ordinary.city_campaign_diplomacy(&mut ordinary_board, 0);
    assert!(
        ordinary_board
            .pending_deals
            .iter()
            .any(|deal| { deal.from == 0 && deal.to == 1 && deal.peace }),
        "ordinary campaigns still close after their planned capture"
    );

    let mut domination = ai.clone();
    domination.victory_target = Some(super::super::VictoryTarget::Domination);
    domination.enable_one_war_at_a_time();
    domination.one_war_observe(&game, 0);
    assert_eq!(domination.one_war_front(), Some(1));
    domination.city_campaign_diplomacy(&mut game, 0);
    assert!(
        !game
            .pending_deals
            .iter()
            .any(|deal| { deal.from == 0 && deal.to == 1 && deal.peace }),
        "taking the border city must not sue for peace before the capital"
    );
    let plan = domination.plan.clone().unwrap();
    domination.advanced_diplomacy(&mut game, 0, &plan);
    assert!(
        !domination.peace_offers.contains(&1),
        "the full peace desk must keep the active front after its first capture"
    );

    let mut science = ai;
    science.victory_target = Some(super::super::VictoryTarget::Science);
    science.enable_one_war_at_a_time();
    science.one_war_observe(&game, 0);
    science.city_campaign_diplomacy(&mut game, 0);
    assert!(
        game.pending_deals
            .iter()
            .any(|deal| { deal.from == 0 && deal.to == 1 && deal.peace }),
        "only the active Domination front inherits the longer war"
    );
}
