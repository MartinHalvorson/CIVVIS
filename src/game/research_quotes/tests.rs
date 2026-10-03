use super::*;
use crate::setup::GameSpeed;

fn board() -> Game {
    Game::new_full(2, 14, 14, 10_033_878, 500, 0, false)
}

fn quote(g: &mut Game, pid: usize, tech: Name, cost: Option<f64>, progress: Option<f64>) {
    Arc::make_mut(&mut g.host_research_quotes)
        .entry(pid)
        .or_default()
        .insert(tech, HostResearchQuote { cost, progress });
}

#[test]
fn research_quotes_are_specific_to_player_and_node() {
    let mut g = board();
    quote(&mut g, 0, crate::name!("radio"), Some(100.0), Some(75.0));
    quote(&mut g, 1, crate::name!("radio"), Some(100.0), Some(10.0));
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        Some(25.0)
    );
    assert_eq!(
        g.host_remaining_research_cost(1, crate::name!("radio")),
        Some(90.0)
    );
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("flight")),
        None
    );
}

#[test]
fn invalid_or_partial_research_quotes_are_unknown_not_zero() {
    let mut g = board();
    for (cost, progress) in [
        (None, Some(0.0)),
        (Some(100.0), None),
        (Some(-1.0), Some(0.0)),
        (Some(100.0), Some(-1.0)),
        (Some(f64::NAN), Some(0.0)),
        (Some(100.0), Some(f64::NAN)),
        (Some(f64::INFINITY), Some(0.0)),
        (Some(100.0), Some(f64::INFINITY)),
    ] {
        quote(&mut g, 0, crate::name!("radio"), cost, progress);
        assert_eq!(
            g.host_remaining_research_cost(0, crate::name!("radio")),
            None
        );
    }
}

#[test]
fn zero_and_excess_native_progress_saturate_at_zero_remaining() {
    let mut g = board();
    for (cost, progress) in [(0.0, 0.0), (100.0, 100.0), (100.0, 110.0)] {
        quote(&mut g, 0, crate::name!("radio"), Some(cost), Some(progress));
        assert_eq!(
            g.host_remaining_research_cost(0, crate::name!("radio")),
            Some(0.0)
        );
    }
}

#[test]
fn native_research_quotes_are_not_scaled_and_clone_updates_are_isolated() {
    let mut g = board();
    quote(&mut g, 0, crate::name!("radio"), Some(100.0), Some(75.0));
    for speed in [GameSpeed::Online, GameSpeed::Standard, GameSpeed::Marathon] {
        g.game_speed = speed;
        assert_eq!(
            g.host_remaining_research_cost(0, crate::name!("radio")),
            Some(25.0)
        );
    }
    let mut future = g.clone();
    quote(
        &mut future,
        0,
        crate::name!("radio"),
        Some(100.0),
        Some(80.0),
    );
    assert_eq!(
        future.host_remaining_research_cost(0, crate::name!("radio")),
        Some(20.0)
    );
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        Some(25.0)
    );
}

#[test]
fn host_research_quotes_are_rebuilt_after_save_load_like_host_menus() {
    let mut g = board();
    quote(&mut g, 0, crate::name!("radio"), Some(100.0), Some(75.0));
    let decoded: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert!(decoded.host_research_quotes.is_empty());
    assert_eq!(
        decoded.host_remaining_research_cost(0, crate::name!("radio")),
        None
    );
}
