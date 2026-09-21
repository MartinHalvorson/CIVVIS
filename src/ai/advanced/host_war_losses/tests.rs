use super::*;
use crate::mirror::{HostUnitDeath, StateSnapshot};

fn fixture() -> (Game, AdvancedAi, StateSnapshot) {
    let mut g = Game::new_full(3, 36, 22, 936_036, 500, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    for (owner, x) in [5, 16, 27].into_iter().enumerate() {
        g.found_city_for(owner, (x, 10), None);
        if owner != 0 {
            g.record_contact(0, owner);
        }
    }
    g.at_war.insert((0, 1));
    g.turn = 100;
    g.current = 0;
    let mut state = StateSnapshot::default();
    state.seat.players = 3;
    state.seat.local_player = 0;
    state.turn = g.turn;
    let mut ai = AdvancedAi::new();
    ai.enable_one_war_at_a_time();
    ai.enable_war_policy_via_board();
    ai.observe_confirmed_host_deaths(&g, &state);
    ai.one_war_observe(&g, 0);
    ai.war_policy_observe(&g, 0);
    assert!(
        g.wars.is_empty(),
        "the native mirror supplies at_war without simulator combat"
    );
    (g, ai, state)
}

fn losses(state: &mut StateSnapshot, player: usize, opponent: Option<usize>, n: i64) {
    for unit in 0..n {
        state.confirmed_unit_deaths.push(HostUnitDeath {
            player,
            unit,
            turn: 101,
            opponent,
        });
    }
}

fn observe(g: &mut Game, ai: &mut AdvancedAi, state: &mut StateSnapshot) {
    g.turn = 101;
    state.turn = g.turn;
    ai.observe_confirmed_host_deaths(g, state);
    ai.one_war_observe(g, 0);
    ai.war_policy_observe(g, 0);
}

#[test]
fn confirmed_host_losses_drive_rout_and_board_peace_without_a_simulator_war() {
    let (mut g, mut ai, mut state) = fixture();
    losses(&mut state, 0, Some(1), 5);
    losses(&mut state, 1, Some(0), 1);
    observe(&mut g, &mut ai, &mut state);
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(one_war::OneWarPeace::Rout));
    assert!(ai
        .war_policy_peace(&g, 0, 1)
        .is_some_and(|why| why.contains("tide has run against us")));
    assert!(g.wars.is_empty());
}

#[test]
fn favorable_confirmed_exchanges_do_not_trigger_retreat() {
    let (mut g, mut ai, mut state) = fixture();
    losses(&mut state, 0, Some(1), 1);
    losses(&mut state, 1, Some(0), 5);
    observe(&mut g, &mut ai, &mut state);
    assert!(ai.one_war_peace(&g, 0, 1).is_none());
    assert!(ai.war_policy_peace(&g, 0, 1).is_none());
}

#[test]
fn barbarian_unknown_and_other_rival_losses_do_not_blame_this_front() {
    for opponent in [None, Some(63), Some(2)] {
        let (mut g, mut ai, mut state) = fixture();
        losses(&mut state, 0, opponent, 5);
        observe(&mut g, &mut ai, &mut state);
        assert!(ai.one_war_peace(&g, 0, 1).is_none(), "{opponent:?}");
        assert!(ai.war_policy_peace(&g, 0, 1).is_none(), "{opponent:?}");
    }
}

#[test]
fn repeated_evidence_and_frames_do_not_count_a_death_twice() {
    let (mut g, mut ai, mut state) = fixture();
    losses(&mut state, 0, Some(1), 1);
    state.confirmed_unit_deaths = vec![state.confirmed_unit_deaths[0].clone(); 5];
    observe(&mut g, &mut ai, &mut state);
    for _ in 0..5 {
        observe(&mut g, &mut ai, &mut state);
    }
    assert!(ai.one_war_peace(&g, 0, 1).is_none());
}

#[test]
fn host_player_mapping_handles_a_nonzero_local_seat() {
    let (mut g, mut ai, mut state) = fixture();
    state.seat.local_player = 2;
    losses(&mut state, 2, Some(0), 4);
    observe(&mut g, &mut ai, &mut state);
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(one_war::OneWarPeace::Rout));
}

#[test]
fn future_deaths_do_not_reach_the_current_tide() {
    let (g, mut ai, mut state) = fixture();
    losses(&mut state, 0, Some(1), 5);
    state.turn = 101;
    ai.observe_confirmed_host_deaths(&g, &state);
    assert_eq!(ai.one_war_ledger(&g, 0, 1), (0, 0, 0, 0));
    let mut later = g.clone();
    later.turn = 101;
    state.turn = 100;
    ai.observe_confirmed_host_deaths(&later, &state);
    assert_eq!(ai.one_war_ledger(&later, 0, 1), (0, 0, 0, 0));
}

#[test]
fn a_new_war_seeds_its_clock_without_recounting_old_casualties() {
    let (mut g, mut ai, mut state) = fixture();
    losses(&mut state, 0, Some(1), 5);
    observe(&mut g, &mut ai, &mut state);
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(one_war::OneWarPeace::Rout));
    g.at_war.clear();
    g.turn += 1;
    ai.one_war_observe(&g, 0);
    ai.war_policy_observe(&g, 0);
    g.at_war.insert((0, 1));
    g.turn += 1;
    ai.one_war_observe(&g, 0);
    ai.war_policy_observe(&g, 0);
    assert!(ai.one_war_peace(&g, 0, 1).is_none());
    assert!(ai.war_policy_peace(&g, 0, 1).is_none());
}

#[test]
fn simulated_games_keep_their_ledger_but_speculative_kills_do_not_pollute_host_tides() {
    let (mut g, _, state) = fixture();
    g.at_war.clear();
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    let war = g.wars.get_mut(&(0, 1)).unwrap();
    war.losses.insert(
        0,
        crate::game::WarLosses {
            units: 4,
            cities: 1,
            ..Default::default()
        },
    );
    let mut simulated = AdvancedAi::new();
    assert_eq!(simulated.one_war_ledger(&g, 0, 1), (4, 0, 1, 0));
    simulated.observe_confirmed_host_deaths(&g, &state);
    assert_eq!(simulated.one_war_ledger(&g, 0, 1), (0, 0, 1, 0));
}
