use super::*;
use crate::game::GovernorState;
use crate::setup::GameSpeed;

/// Our capital `post` holding Victor at full Loyalty, and a second city
/// `bleeding` at Loyalty 30. Each city's Loyalty rate is pinned through the
/// host-observed rate, as the live seat reads it.
fn board(post_rate: f64, bleeding_rate: f64) -> (Game, u32, u32) {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let post = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    let bleeding = g.found_city_for(0, (10, 10), None);
    g.cities.get_mut(&post).unwrap().loyalty = 100.0;
    g.cities.get_mut(&bleeding).unwrap().loyalty = 30.0;
    g.players[0].governor_roster.insert(
        "victor".to_string(),
        GovernorState {
            city: Some(post),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: BTreeSet::new(),
        },
    );
    g.players[0].governors = vec![post];
    g.turn = 60;
    let rates = std::sync::Arc::make_mut(&mut g.observed_city_loyalty_per_turn);
    rates.insert(post, post_rate);
    rates.insert(bleeding, bleeding_rate);
    (g, post, bleeding)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_governor_holds_the_flipping_city();
    ai
}

fn posted(g: &Game) -> Option<u32> {
    g.players[0]
        .governor_roster
        .iter()
        .find(|(name, _)| name.as_str() == "victor")
        .and_then(|(_, state)| state.city)
}

/// A city flipping in six turns, savable by +8, takes Victor from a capital
/// that keeps +4 without him; with the gene off nothing moves.
#[test]
fn a_bleeding_city_takes_the_governor_from_a_safe_post() {
    let (g, post, bleeding) = board(12.0, -5.0);
    let mut off = g.clone();
    assert!(!AdvancedAi::new().governor_holds_the_flipping_city(&mut off, 0));
    assert_eq!(posted(&off), Some(post));

    let ai = armed();
    let mut on = g.clone();
    assert!(ai.governor_holds_the_flipping_city(&mut on, 0));
    assert_eq!(posted(&on), Some(bleeding));
}

/// A slide one Governor cannot stop is left alone.
#[test]
fn a_city_beyond_one_governor_is_left() {
    let (g, _, _) = board(12.0, -20.0);
    assert_eq!(armed().flipping_city_governor_move(&g, 0), None);
}

/// A post that would itself bleed without its Governor keeps it.
#[test]
fn a_post_that_needs_its_governor_keeps_it() {
    let (g, _, _) = board(5.0, -5.0);
    assert_eq!(armed().flipping_city_governor_move(&g, 0), None);
}

/// A Governor not yet established at its post is not moved again, and a city
/// far from flipping is not a target.
#[test]
fn a_rested_governor_and_a_distant_flip_are_left() {
    let (mut g, _, _) = board(12.0, -5.0);
    let now = g.turn;
    g.players[0]
        .governor_roster
        .values_mut()
        .for_each(|state| state.assigned_turn = now);
    assert_eq!(armed().flipping_city_governor_move(&g, 0), None);

    let (g, _, _) = board(12.0, -0.5);
    assert_eq!(armed().flipping_city_governor_move(&g, 0), None);
}
