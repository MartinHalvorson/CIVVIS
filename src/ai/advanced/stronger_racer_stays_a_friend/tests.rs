use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Three met majors with a city each. Seat 2 is stronger than seat 0 and reads
/// as racing; seat 1 is a bystander.
fn board() -> Game {
    let mut game = Game::new_full(3, 24, 16, 7_923, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    let capital = game.cities[&game.player_city_ids(2)[0]].pos;
    for _ in 0..4 {
        game.spawn_test_unit("warrior", 2, capital);
    }
    game.players[2].dvp = 10;
    game.turn = 30;
    game
}

fn seat(target: Option<VictoryTarget>, gene: bool) -> AdvancedAi {
    let mut ai = match target {
        Some(target) => AdvancedAi::targeting(target),
        None => AdvancedAi::new(),
    };
    ai.enable_befriend_the_strongest();
    ai.enable_friendship_ends_at_the_race();
    if gene {
        ai.enable_stronger_racer_stays_a_friend();
    }
    ai
}

fn asked(ai: &AdvancedAi, mut game: Game) -> Option<usize> {
    ai.propose_protective_friendship(&mut game, 0);
    game.pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .map(|deal| deal.to)
}

#[test]
fn stronger_racer_stays_a_friend_is_opt_in() {
    opt_in_off_in_both_controllers("stronger-racer-stays-a-friend", |ai| {
        ai.stronger_racer_stays_a_friend
    });
}

/// Off, the racer is passed over. On, a Science seat asks it.
#[test]
fn a_stronger_racer_before_a_launch_is_asked() {
    let science = Some(VictoryTarget::Science);
    assert_eq!(asked(&seat(science, false), board()), Some(1));
    assert_eq!(asked(&seat(science, true), board()), Some(2));
}

/// A project landed puts the rival physically in the race: the launcher war
/// must stay legal.
#[test]
fn a_rival_with_a_project_landed_stays_out() {
    let mut game = board();
    game.players[2]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    assert_eq!(
        asked(&seat(Some(VictoryTarget::Science), true), game),
        Some(1)
    );
}

/// The evidence is from Science seats; another seat keeps the old rule.
#[test]
fn off_a_science_aim_the_racer_stays_out() {
    assert_eq!(asked(&seat(None, true), board()), Some(1));
}

/// A racer weaker than us is one the counter can reach.
#[test]
fn a_weaker_racer_stays_out() {
    let mut game = board();
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..12 {
        game.spawn_test_unit("warrior", 0, capital);
    }
    let ai = seat(Some(VictoryTarget::Science), true);
    assert!(game.military_power(0) > game.military_power(2));
    assert!(!ai.racer_stays_a_friend(&game, 0, 2));
}
