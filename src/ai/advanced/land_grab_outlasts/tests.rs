use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// A two-seat board with our capital founded, early enough that the ordinary
/// cadence still sits below the land grab's capacity.
fn board() -> Game {
    let mut g = Game::new(2, 40, 26, 71, 250, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.turn = 30;
    g
}

fn target(g: &Game, rapid: bool, land_grab: bool, gene: bool) -> usize {
    let mut ai = AdvancedAi::new();
    if rapid {
        ai.enable_rapid_city_expansion_2();
    }
    if land_grab {
        ai.enable_land_grab();
    }
    if gene {
        ai.enable_land_grab_outlasts_the_opening();
    }
    ai.assess(g, 0).desired_cities
}

#[test]
fn land_grab_outlasts_the_opening_is_opt_in() {
    opt_in_off_in_both_controllers("land-grab-outlasts-the-opening", |ai| {
        ai.land_grab_outlasts_the_opening
    });
}

/// Stock, `rapid-city-expansion-2` is asked first and the land grab's
/// capacity is never read. Under the gene the target is the larger of the
/// two answers.
#[test]
fn the_rapid_opening_keeps_the_land_grabs_horizon() {
    let g = board();
    let rapid_only = target(&g, true, false, false);
    let land_only = target(&g, false, true, false);
    assert!(
        land_only > rapid_only,
        "fixture: the land grab reads wider ({land_only}) than the rapid opening ({rapid_only})"
    );
    assert_eq!(
        target(&g, true, true, false),
        rapid_only,
        "stock: the rapid opening shadows the land grab"
    );
    assert_eq!(
        target(&g, true, true, true),
        land_only.max(rapid_only),
        "under the gene the land grab's horizon stands"
    );
}

/// With either expansion gene off the new gene changes nothing.
#[test]
fn either_expansion_gene_off_is_unchanged() {
    let g = board();
    assert_eq!(
        target(&g, true, false, true),
        target(&g, true, false, false)
    );
    assert_eq!(
        target(&g, false, true, true),
        target(&g, false, true, false)
    );
    assert_eq!(
        target(&g, false, false, true),
        target(&g, false, false, false)
    );
}
