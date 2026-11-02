use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Six majors with founded capitals, our seat on the Domination target, the
/// clock at the hand-over turn.
fn stalled() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(6, 60, 40, 936_511, 650, 0, false);
    for pid in 0..g.players.len() {
        if let Some(settler) = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
        {
            let _ = g.apply(pid, &Action::FoundCity { unit: settler });
        }
    }
    g.turn = g.standard_duration(STALLED_CONQUEST_STANDARD);
    (g, AdvancedAi::targeting(VictoryTarget::Domination))
}

#[test]
fn stalled_conquest_takes_science_is_opt_in() {
    opt_in_off_in_both_controllers("stalled-conquest-takes-science", |ai| {
        ai.stalled_conquest_takes_science
    });
}

/// No rival capital at the hand-over turn: under the gene the seat retargets
/// to Science; off, it keeps Domination.
#[test]
fn a_capital_less_conquest_plays_science() {
    let (g, mut ai) = stalled();
    assert!(!AdvancedAi::holds_a_rival_capital(&g, 0));
    ai.hand_over_a_stalled_conquest(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Domination), "off");
    ai.enable_stalled_conquest_takes_science();
    ai.hand_over_a_stalled_conquest(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Science));
}

/// Before the turn, holding a rival capital, or on another target, the lane
/// is kept.
#[test]
fn an_early_turn_a_held_capital_or_another_lane_keeps_the_target() {
    let (mut g, mut ai) = stalled();
    ai.enable_stalled_conquest_takes_science();
    g.turn -= 1;
    ai.hand_over_a_stalled_conquest(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Domination), "early");

    let (mut g, mut ai) = stalled();
    ai.enable_stalled_conquest_takes_science();
    // A rival's original capital, captured: the city keeps `is_capital` and
    // its `original_owner`, as `Game::transfer_city` leaves it.
    let ours = g.player_city_ids(0)[0];
    let home = g.cities[&ours].pos;
    let site = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (6..=10).contains(&g.wdist(**pos, home))
                && tile.owner_city.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: a free land tile");
    let rival_capital = g.place_city(1, site, None);
    {
        let city = g.cities.get_mut(&rival_capital).unwrap();
        city.is_capital = true;
        city.original_owner = 1;
        city.owner = 0;
    }
    assert!(AdvancedAi::holds_a_rival_capital(&g, 0));
    ai.hand_over_a_stalled_conquest(&g, 0);
    assert_eq!(
        ai.victory_target,
        Some(VictoryTarget::Domination),
        "holds a capital"
    );

    let (g, _) = stalled();
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_stalled_conquest_takes_science();
    culture.hand_over_a_stalled_conquest(&g, 0);
    assert_eq!(culture.victory_target, Some(VictoryTarget::Culture));
}
