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

#[test]
fn observed_switch_banks_work_and_restores_each_nodes_own_progress() {
    let mut g = board();
    let a = crate::name!("pottery");
    let b = crate::name!("mining");
    g.players[0].techs.remove(&a);
    g.players[0].techs.remove(&b);
    quote(&mut g, 0, a, Some(100.0), Some(20.0));
    quote(&mut g, 0, b, Some(80.0), Some(35.0));
    g.players[0].research = Some(a.to_string());
    g.players[0].research_progress = 27.0;
    g.players[0].research_overflow = 9.0;
    g.players[0].boosted_techs.insert(b);
    let original = g.clone();
    g.apply(0, &Action::Research { tech: b }).unwrap();
    assert_eq!(
        g.players[0].research_progress, 35.0,
        "no second boost or transferred science"
    );
    assert_eq!(g.players[0].research_overflow, 9.0);
    assert_eq!(g.host_research_quotes[&0][&a].progress, Some(27.0));
    g.players[0].research_progress = 48.0;
    g.apply(0, &Action::Research { tech: a }).unwrap();
    assert_eq!(g.players[0].research_progress, 27.0);
    assert_eq!(g.host_research_quotes[&0][&b].progress, Some(48.0));
    assert_eq!(
        original.host_research_quotes[&0][&a].progress,
        Some(20.0),
        "clone isolation"
    );
}

#[test]
fn switches_require_both_valid_host_banks_and_a_legal_destination() {
    let mut base = board();
    base.players[0].techs.remove(&crate::name!("pottery"));
    base.players[0].techs.remove(&crate::name!("mining"));
    base.players[0].research = Some("pottery".into());
    base.players[0].research_progress = 12.0;
    for case in 0..5 {
        let mut g = base.clone();
        if case != 0 {
            quote(&mut g, 0, crate::name!("pottery"), Some(100.0), Some(12.0));
        }
        if case != 1 {
            quote(
                &mut g,
                0,
                crate::name!("mining"),
                Some(100.0),
                Some(if case == 2 { f64::NAN } else { 4.0 }),
            );
        }
        let destination = if case == 3 {
            crate::name!("writing")
        } else if case == 4 {
            crate::name!("pottery")
        } else {
            crate::name!("mining")
        };
        if case == 3 {
            quote(&mut g, 0, destination, Some(150.0), Some(0.0));
        }
        let quotes = g.host_research_quotes.clone();
        assert!(g.apply(0, &Action::Research { tech: destination }).is_err());
        assert_eq!(g.players[0].research.as_deref(), Some("pottery"));
        assert_eq!(g.players[0].research_progress, 12.0);
        assert!(
            Arc::ptr_eq(&quotes, &g.host_research_quotes),
            "failed switch changed bank"
        );
    }
}
