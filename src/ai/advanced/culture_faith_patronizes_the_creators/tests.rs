//! `culture-faith-patronizes-the-creators`: unit tests.
use super::*;

const TAG: &str = "culture-faith-patronizes-the-creators:";
const CREATORS: [&str; 3] = ["writer", "artist", "musician"];

/// One founded capital (its Palace holds an `any` slot) with `faith` in the
/// bank.
fn board(faith: f64) -> Game {
    let mut g = Game::new_full(
        1,
        44,
        26,
        crate::rng::fixture_seed("CULTUREPATRONAGE", 1),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].faith = faith;
    g
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_faith_patronizes_the_creators();
    }
    ai
}

/// Run the pass with a recording journal; return its thoughts' details.
fn patronize(ai: &mut AdvancedAi, g: &mut Game) -> Vec<String> {
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(g.turn, 0);
    ai.attach_journal(journal.handle());
    ai.culture_faith_patronizes_the_creators(g, 0);
    journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect()
}

fn cheapest(g: &Game) -> f64 {
    CREATORS
        .iter()
        .filter_map(|kind| g.great_person_patronage_price(0, kind, "faith"))
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn it_is_an_opt_in_off_by_default() {
    assert!(!AdvancedAi::targeting(VictoryTarget::Culture).culture_faith_patronizes_the_creators);
}

#[test]
fn a_culture_seat_buys_creators_with_faith_above_the_reserve() {
    let mut g = board(20_000.0);
    assert!(cheapest(&g).is_finite(), "a creator is offered");
    let cid = g.player_city_ids(0)[0];
    assert!(g.cities[&cid].buildings.iter().any(|b| *b == "palace"));
    let mut ai = seat(VictoryTarget::Culture, true);
    let lines = patronize(&mut ai, &mut g);
    assert!(g.players[0].faith < 20_000.0, "{lines:?}");
    assert!(g.players[0].faith >= 100.0);
    assert!(
        !lines.is_empty() && lines.iter().all(|line| line.starts_with(TAG)),
        "{lines:?}"
    );
    assert!(lines.len() <= 3, "one creator per class a turn: {lines:?}");
}

#[test]
fn the_reserve_and_the_band_reserve_hold() {
    let g = board(0.0);
    let price = cheapest(&g);
    // Price plus 99: the ordinary reserve of 100 is not met.
    let mut g = board(price + 99.0);
    let mut ai = seat(VictoryTarget::Culture, true);
    assert!(patronize(&mut ai, &mut g).is_empty());
    // Price plus 300 is enough before Cold War, not after.
    let mut g = board(price + 300.0);
    g.players[0].civics.insert(crate::name!("cold_war"));
    let mut ai = seat(VictoryTarget::Culture, true);
    assert!(patronize(&mut ai, &mut g).is_empty());
    let mut g = board(price + 300.0);
    let mut ai = seat(VictoryTarget::Culture, true);
    assert_eq!(patronize(&mut ai, &mut g).len(), 1);
}

#[test]
fn a_religion_another_lane_or_the_gene_off_spends_nothing() {
    let mut g = board(20_000.0);
    g.players[0].religion = Some("buddhism".to_string());
    let mut ai = seat(VictoryTarget::Culture, true);
    assert!(patronize(&mut ai, &mut g).is_empty());
    assert_eq!(g.players[0].faith, 20_000.0);

    let mut g = board(20_000.0);
    let mut ai = seat(VictoryTarget::Science, true);
    assert!(patronize(&mut ai, &mut g).is_empty());

    let mut g = board(20_000.0);
    let mut ai = seat(VictoryTarget::Culture, false);
    assert!(patronize(&mut ai, &mut g).is_empty());
    assert_eq!(g.players[0].faith, 20_000.0);
}
