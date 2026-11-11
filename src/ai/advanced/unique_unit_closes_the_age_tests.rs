use super::super::{AdvancedAi, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::rules::Yields;
use std::sync::Arc;

/// A Greek capital with Bronze Working, 3 Era Score short of a Normal Age,
/// five turns before the era closes, producing 40 a turn.
fn board(short: i64) -> (Game, u32) {
    let mut g = Game::new_full(1, 18, 10, 9_301, 120, 0, false);
    g.players[0].civ = "Greece".into();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    for tech in ["mining", "bronze_working"] {
        g.players[0].techs.insert(crate::name::Name::new(tech));
    }
    g.players[0].normal_age_threshold = 8;
    g.players[0].era_score = 8 - short;
    g.world_era_countdown_end = Some(g.turn + 5);
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        city,
        Yields {
            production: 40.0,
            ..Default::default()
        },
    );
    (g, city)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if gene {
        ai.enable_unique_unit_closes_the_dark_age();
    }
    ai
}

fn hoplite() -> Item {
    Item::Unit {
        unit: crate::name!("hoplite"),
    }
}

#[test]
fn off_the_gene_never_claims() {
    let (mut g, city) = board(3);
    assert!(!seat(false).claim_unique_unit_for_the_age(&mut g, 0));
    assert_ne!(g.cities[&city].queue.first(), Some(&hoplite()));
}

#[test]
fn three_short_with_five_turns_left_trains_the_first_hoplite() {
    let (mut g, city) = board(3);
    assert!(seat(true).claim_unique_unit_for_the_age(&mut g, 0));
    assert_eq!(g.cities[&city].queue.first(), Some(&hoplite()));
}

#[test]
fn five_short_is_left_alone() {
    let (mut g, city) = board(5);
    assert!(!seat(true).claim_unique_unit_for_the_age(&mut g, 0));
    assert_ne!(g.cities[&city].queue.first(), Some(&hoplite()));
}

#[test]
fn a_seat_already_normal_is_left_alone() {
    let (mut g, _) = board(0);
    assert!(seat(true).unique_unit_age_short(&g, 0).is_none());
    assert!(!seat(true).claim_unique_unit_for_the_age(&mut g, 0));
}

#[test]
fn research_takes_bronze_working_when_it_is_missing() {
    let (mut g, _) = board(3);
    g.players[0].techs.remove(&crate::name!("bronze_working"));
    assert_eq!(
        seat(true).unique_unit_age_tech(&g, 0).as_deref(),
        Some("bronze_working")
    );
    assert_eq!(seat(false).unique_unit_age_tech(&g, 0), None);
}
