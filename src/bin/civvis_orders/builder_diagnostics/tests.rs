use super::*;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| s.to_string()).collect()
}

#[test]
fn strategy_is_explicit_and_never_inferred_from_the_victory_target() {
    assert!(strategy(&args(&["--victory", "science"])).is_err());
    assert!(strategy(&args(&["--builder-strategy", "unknown"])).is_err());
    assert_eq!(
        strategy(&args(&["--builder-strategy", "conquest"])).unwrap(),
        GrandStrategy::Conquest
    );
    assert_eq!(
        strategy(&args(&["--builder-strategy", "science"])).unwrap(),
        GrandStrategy::Science
    );
}

#[test]
fn diagnostic_cannot_prepend_output_to_the_server_protocol() {
    assert!(strategy(&args(&["--serve", "--builder-strategy", "science"])).is_err());
}

#[test]
fn missing_board_fails_without_a_planned_order() {
    let ai = AdvancedAi::targeting(civvis::ai::VictoryTarget::Science);
    let report = explain(
        &args(&["--builder-strategy", "science"]),
        &ai,
        4,
        250,
        6,
        &|_| None,
    );
    assert_eq!(report.unwrap_err(), "no revealed terrain or no state yet");
}
