use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::game::Game;
use std::sync::Arc;

/// Player 1's original capital is ours, stable, while we stay at war with
/// player 1's remnant town; player 2 holds the next original capital, at
/// peace, within the declaration range, and we outgun everyone.
fn captured_front() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(4, 36, 22, 91_120, 500, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (0, 8), None);
    let capital = g.found_city_for(0, (5, 8), None);
    {
        let c = g.cities.get_mut(&capital).unwrap();
        c.is_capital = true;
        c.original_owner = 1;
        c.loyalty = 100.0;
    }
    let town = g.found_city_for(1, (8, 8), None);
    g.cities.get_mut(&town).unwrap().is_capital = false;
    g.found_city_for(2, (16, 8), None);
    g.found_city_for(2, (21, 8), None);
    g.found_city_for(3, (25, 16), None);
    g.record_contact(0, 3);
    g.players[0].friends_until.insert(3, 1000);
    g.players[3].friends_until.insert(0, 1000);
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(capital, 10.0);
    for other in [1, 2] {
        g.record_contact(0, other);
    }
    for _ in 0..4 {
        g.spawn_test_unit("modern_armor", 0, (5, 8));
    }
    for _ in 0..2 {
        g.spawn_test_unit("modern_armor", 2, (18, 8));
    }
    Arc::make_mut(&mut g.observed_military_power).insert(0, 4000.0);
    g.players[0].gold = 3000.0;
    g.at_war.insert((0, 1));
    g.turn = 250;
    g.current = 0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(town),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    ai.one_war_observe(&g, 0);
    (g, ai, plan)
}

/// The closure clock as `one_war_observe` keeps it, set `turns` back.
fn refused_for(ai: &mut AdvancedAi, g: &Game, turns: u32) {
    ai.one_war.as_mut().expect("the front").closure_wanted_since = Some(g.turn - turns);
}

#[test]
fn a_refused_capital_peace_moves_the_army_on_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, plan) = captured_front();
        if gene {
            ai.enable_capital_taken_moves_on();
        }
        assert_eq!(
            ai.one_war_peace(&g, 0, 1),
            Some(OneWarPeace::CapitalSecured),
            "fixture: the beaten rival is offered the capital-secure peace"
        );
        refused_for(&mut ai, &g, 1);
        assert_eq!(ai.capital_moves_on_next(&g, 0), None, "a fresh refusal waits");
        refused_for(&mut ai, &g, CAPITAL_MOVES_ON_TURNS);
        assert_eq!(
            ai.capital_moves_on_next(&g, 0),
            if gene { Some(2) } else { None },
            "three refused turns name the next capital's owner only under the gene"
        );
        for _ in 0..12 {
            if g.is_at_war(0, 2) {
                break;
            }
            ai.advanced_diplomacy(&mut g, 0, &plan);
            g.turn += 1;
            g.current = 0;
            ai.one_war_observe(&g, 0);
            refused_for(&mut ai, &g, CAPITAL_MOVES_ON_TURNS);
        }
        if gene {
            assert!(g.is_at_war(0, 2), "gene on: the next capital's owner is declared on");
            assert!(g.is_at_war(0, 1), "the remnant war is held, not abandoned");
            ai.one_war_observe(&g, 0);
            assert_eq!(ai.one_war_front(), Some(2), "the front moves to the next capital's war");
        } else {
            assert!(!g.is_at_war(0, 2), "gene off: no staged siege, no declaration");
        }
    }
}

/// Player 1 a crushed Diplomatic Victory contender at 15 points, its capital
/// ours; player 2 holds the next capital.
fn contender_front() -> (Game, AdvancedAi) {
    let (mut g, mut ai, _) = captured_front();
    g.players[1].dvp = 15;
    ai.enable_diplomatic_contender_eliminated();
    ai.enable_capital_taken_moves_on();
    ai.one_war_observe(&g, 0);
    (g, ai)
}

#[test]
fn a_contender_with_no_shorter_clock_keeps_the_elimination_front() {
    let (g, ai) = contender_front();
    assert_eq!(ai.diplomatic_contender_base(&g, 0), Some(1), "fixture: the contender");
    assert!(!ai.elimination_yields_to_a_shorter_clock(&g, 0, 1));
    assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), Some(1));
    assert_eq!(ai.capital_moves_on_next(&g, 0), None, "the elimination front holds the army");
}

#[test]
fn a_shorter_science_clock_makes_the_elimination_front_yield() {
    let (mut g, mut ai) = contender_front();
    let stats = Arc::make_mut(&mut g.observed_public_empire_stats).entry(2).or_default();
    stats.science_victory_points = Some(20.0);
    stats.science_victory_points_needed = Some(25.0);
    stats.science_victory_points_per_turn = Some(1.0);
    assert!(
        ai.moves_on_race_clock(&g, 2).unwrap() + f64::from(g.standard_duration(5))
            < ai.moves_on_dvp_clock(&g, 1),
        "fixture: the science clock runs out first"
    );
    assert!(ai.elimination_yields_to_a_shorter_clock(&g, 0, 1));
    assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), None, "the front yields");
    assert_eq!(ai.capital_moves_on_next(&g, 0), Some(2), "to the shorter clock's capital");
    ai.disable_capital_taken_moves_on();
    assert_eq!(
        ai.diplomatic_contender_to_eliminate(&g, 0),
        Some(1),
        "gene off: the elimination front holds"
    );
}

#[test]
fn capital_taken_moves_on_is_a_reversible_opt_in() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.capital_taken_moves_on);
    ai.enable_capital_taken_moves_on();
    assert!(ai.capital_taken_moves_on);
    ai.disable_capital_taken_moves_on();
    assert!(!ai.capital_taken_moves_on);
}
