use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, AllianceState, Game};
use std::sync::Arc;

/// Three majors, a capital each, every pair met, Civil Service everywhere,
/// no units: military power is the observed figure set per test (ours 100).
fn board() -> Game {
    let mut game = Game::new_full(3, 24, 16, 7_923, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for pid in 0..3 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    for player in game.players.iter_mut().filter(|p| !p.is_minor) {
        player.civics.insert(crate::name!("civil_service"));
    }
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 100.0);
    power(&mut game, 2, 100.0);
    // Turn 61 is no border turn for seat 0 (61 % 3 != 0).
    game.turn = 61;
    game.current = 0;
    game
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

fn plan(strategy: GrandStrategy, target: Option<usize>) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: target,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 0,
        rush: false,
    }
}

fn shield() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_military_alliance_shield();
    ai
}

fn asks(game: &Game) -> Vec<(usize, Option<String>, bool)> {
    game.pending_deals
        .iter()
        .filter(|deal| deal.from == 0)
        .map(|deal| (deal.to, deal.alliance.clone(), deal.friendship))
        .collect()
}

fn counter(game: &Game, key: &str) -> Option<i64> {
    game.players[0].counters.get(key).copied()
}

#[test]
fn military_alliance_shield_is_opt_in() {
    opt_in_off_in_both_controllers("military-alliance-shield", |ai| ai.military_alliance_shield);
}

/// At war with player 2, which player 1 is fighting too: player 1 is asked
/// for a Military Alliance (friendship bundled) on any turn and any lane,
/// player 2 never; off the gene nothing is asked.
#[test]
fn the_empire_fighting_our_enemy_is_asked_for_a_military_alliance() {
    for strategy in [GrandStrategy::Science, GrandStrategy::Conquest] {
        let mut game = board();
        game.at_war.insert((0, 2));
        game.at_war.insert((1, 2));
        let mut off = AdvancedAi::new();
        assert!(!off.military_alliance_shield_step(&mut game, 0, &plan(strategy, None), None));
        assert!(asks(&game).is_empty());
        assert_eq!(off.military_alliance_shield_partner(), None);

        let mut ai = shield();
        assert!(ai.military_alliance_shield_step(&mut game, 0, &plan(strategy, None), None));
        assert_eq!(
            asks(&game),
            vec![(1, Some("military".to_string()), true)],
            "{strategy:?}"
        );
        assert_eq!(ai.military_alliance_shield_partner(), Some(1));
        assert_eq!(counter(&game, "military_alliance_shield:asked"), Some(1));
        assert_eq!(
            counter(&game, "military_alliance_shield:asked_common_enemy"),
            Some(1)
        );

        // Accepted: the alliance stands, and nothing more is asked.
        let id = game.pending_deals.iter().find(|d| d.from == 0).unwrap().id;
        game.current = 1;
        game.apply(1, &Action::AcceptDeal { deal: id })
            .expect("the partner accepts");
        game.current = 0;
        assert_eq!(
            game.alliance_with(0, 1).map(|a| a.kind.as_str()),
            Some("military")
        );
        game.turn += 1;
        assert!(!ai.military_alliance_shield_step(&mut game, 0, &plan(strategy, None), None));
        assert_eq!(ai.military_alliance_shield_partner(), None);
    }
}

/// On the live seat our own war crosses as `at_war` and a war between two
/// rivals only as the host's report (`observed_rival_wars`): that report
/// finds the common enemy too.
#[test]
fn a_rivals_war_the_host_reports_finds_the_common_enemy() {
    let mut game = board();
    game.at_war.insert((0, 2));
    Arc::make_mut(&mut game.observed_rival_wars).insert((1, 2));
    assert!(!game.is_at_war(1, 2));
    let mut ai = shield();
    assert!(ai.military_alliance_shield_step(
        &mut game,
        0,
        &plan(GrandStrategy::Science, None),
        None
    ));
    assert_eq!(asks(&game), vec![(1, Some("military".to_string()), true)]);
    assert_eq!(
        counter(&game, "military_alliance_shield:asked_common_enemy"),
        Some(1)
    );
}

/// At peace, on the border cadence, the strongest neighbour over the threat
/// floor is asked; one under the floor is not; off the cadence, or on the
/// Domination lane, nobody is.
#[test]
fn the_strongest_neighbour_is_asked_off_the_domination_lane() {
    let science = plan(GrandStrategy::Science, None);
    let mut game = board();
    power(&mut game, 1, 30.0);
    power(&mut game, 2, 150.0);
    let mut ai = shield();
    // Turn 61: not a border turn, and no war.
    assert!(!ai.military_alliance_shield_step(&mut game, 0, &science, None));
    game.turn = 60;
    let mut domination = shield();
    assert!(!domination.military_alliance_shield_step(
        &mut game,
        0,
        &plan(GrandStrategy::Conquest, None),
        None
    ));
    let mut domination_lane = shield();
    domination_lane.victory_target = Some(VictoryTarget::Domination);
    assert!(!domination_lane.military_alliance_shield_step(&mut game, 0, &science, None));
    assert!(asks(&game).is_empty());

    assert!(ai.military_alliance_shield_step(&mut game, 0, &science, None));
    assert_eq!(asks(&game), vec![(2, Some("military".to_string()), true)]);
    assert_eq!(
        counter(&game, "military_alliance_shield:asked_border"),
        Some(1)
    );

    // Nobody over the floor: nobody is asked.
    let mut game = board();
    game.turn = 60;
    power(&mut game, 1, 30.0);
    power(&mut game, 2, 40.0);
    assert!(!shield().military_alliance_shield_step(&mut game, 0, &science, None));
    assert!(asks(&game).is_empty());
}

/// A declared friend outranks an equally strong stranger: the host asks it
/// the alliance itself. (Equal power, so only the nearness term differs, and
/// inside the reach it differs by less than [`SHIELD_FRIEND`].)
#[test]
fn a_declared_friend_is_asked_first() {
    for friend in [1, 2] {
        let mut game = board();
        game.turn = 60;
        game.players[0].friends_until.insert(friend, 90);
        game.players[friend].friends_until.insert(0, 90);
        let mut ai = shield();
        assert!(ai.military_alliance_shield_step(
            &mut game,
            0,
            &plan(GrandStrategy::Science, None),
            None
        ));
        assert_eq!(asks(&game)[0].0, friend);
    }
}

/// No second Military Alliance; never the plan's target, a denounced rival
/// or one we hold an alliance with already.
#[test]
fn the_partner_filters_hold() {
    let science = plan(GrandStrategy::Science, None);
    let military = |ends: u32| AllianceState {
        kind: "military".to_string(),
        points: 0.0,
        level: 1,
        ends,
    };

    // We hold a Military Alliance (with nobody on this board): no ask.
    let mut game = board();
    game.turn = 60;
    game.players[0].alliances.insert(5, military(90));
    assert!(!shield().military_alliance_shield_step(&mut game, 0, &science, None));

    // The plan's target and a denounced rival are passed over.
    let mut game = board();
    game.turn = 60;
    power(&mut game, 2, 150.0);
    game.players[0].denounced_until.insert(1, 90);
    assert!(!shield().military_alliance_shield_step(&mut game, 0, &science, Some(2)));
    assert!(asks(&game).is_empty());

    // Already allied (another kind) with the strongest: the next is asked.
    let mut game = board();
    game.turn = 60;
    power(&mut game, 2, 150.0);
    let research = AllianceState {
        kind: "research".to_string(),
        points: 0.0,
        level: 1,
        ends: 90,
    };
    game.players[0].alliances.insert(2, research.clone());
    game.players[2].alliances.insert(0, research);
    assert!(shield().military_alliance_shield_step(&mut game, 0, &science, None));
    assert_eq!(asks(&game)[0].0, 1);
}

/// With `alliance-remembers-the-refusal` on, a partner that did not ally is
/// not asked again inside its bar.
#[test]
fn a_refused_partner_waits_out_the_refusal_memory() {
    let science = plan(GrandStrategy::Science, None);
    let mut game = board();
    game.turn = 60;
    power(&mut game, 1, 30.0);
    power(&mut game, 2, 150.0);
    let mut ai = shield();
    ai.enable_alliance_remembers_the_refusal();
    assert!(ai.military_alliance_shield_step(&mut game, 0, &science, None));
    let id = game.pending_deals.iter().find(|d| d.from == 0).unwrap().id;
    game.current = 2;
    game.apply(2, &Action::RejectDeal { deal: id })
        .expect("refused");
    game.current = 0;
    game.turn = 63;
    assert!(!ai.military_alliance_shield_step(&mut game, 0, &science, None));
    assert!(asks(&game).is_empty());
}
