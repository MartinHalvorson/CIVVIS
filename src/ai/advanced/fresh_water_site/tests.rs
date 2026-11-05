use super::*;
use crate::game::Game;

/// A plot with fresh water and one without on the same board.
fn plots(g: &Game) -> (Pos, Pos) {
    let land: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
        .collect();
    let fresh = land
        .iter()
        .copied()
        .find(|pos| g.city_site_water(0, *pos).0);
    let dry = land
        .iter()
        .copied()
        .find(|pos| !g.city_site_water(0, *pos).0);
    (
        fresh.expect("the board has a fresh-water plot"),
        dry.expect("the board has a plot without fresh water"),
    )
}

/// Off, nothing is added anywhere. On, only a fresh-water seat gains, by
/// exactly the bonus, and the full site score and the prefilter both carry it.
#[test]
fn a_fresh_water_seat_gains_the_bonus_only_with_the_gene_on() {
    let g = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let (fresh, dry) = plots(&g);
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_fresh_water_prices_the_site();
    assert_eq!(off.fresh_water_site_bonus(&g, 0, fresh), 0.0);
    assert_eq!(
        on.fresh_water_site_bonus(&g, 0, fresh),
        FRESH_WATER_SITE_BONUS
    );
    assert_eq!(on.fresh_water_site_bonus(&g, 0, dry), 0.0);
    let gain = |pos: Pos| {
        on.settlement_static_value_uncached(&g, 0, pos)
            - off.settlement_static_value_uncached(&g, 0, pos)
    };
    assert!(
        (gain(fresh) - FRESH_WATER_SITE_BONUS).abs() < 1e-9,
        "the site score"
    );
    assert!(gain(dry).abs() < 1e-9, "a dry seat reads as before");
    let prefilter = |pos: Pos| {
        on.settlement_prefilter_score_for(&g, 0, pos)
            - off.settlement_prefilter_score_for(&g, 0, pos)
    };
    assert!(
        (prefilter(fresh) - FRESH_WATER_SITE_BONUS).abs() < 1e-9,
        "the shortlist"
    );
    assert!(
        prefilter(dry).abs() < 1e-9,
        "a dry seat's shortlist rank is unchanged"
    );
}
