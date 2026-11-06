use super::*;

fn land(g: &Game) -> Vec<Pos> {
    g.map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        })
        .collect()
}

fn board(civ: &str) -> Game {
    let mut g = Game::new_full(1, 40, 26, 936_221, 200, 0, false);
    g.players[0].civ = civ.into();
    g
}

fn fresh_near(g: &Game, pos: Pos) -> bool {
    g.wdisk(pos, NUSANTARA_FRESH_WATER_TILES)
        .into_iter()
        .any(|at| {
            g.map
                .get(at)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                && g.city_site_water(0, at).0
        })
}

/// Off, and for any other civilization, nothing is added.
#[test]
fn only_an_indonesian_seat_with_the_gene_gains_anything() {
    let mut on = AdvancedAi::new();
    on.enable_nusantara_settles_the_shore();
    let off = AdvancedAi::new();
    let indonesia = board("Indonesia");
    let korea = board("Korea");
    for pos in land(&indonesia) {
        assert_eq!(off.nusantara_shore_site_bonus(&indonesia, 0, pos), 0.0);
    }
    for pos in land(&korea) {
        assert_eq!(on.nusantara_shore_site_bonus(&korea, 0, pos), 0.0);
    }
}

/// On every plot of the board: inland nothing; a shore seat with fresh water
/// the bonus; a coast-only seat the bonus only with no fresh water in reach.
/// The board must hold a paid seat and a shore seat the rule refuses.
#[test]
fn a_coast_only_seat_does_not_outbid_fresh_water_beside_it() {
    let mut ai = AdvancedAi::new();
    ai.enable_nusantara_settles_the_shore();
    let g = board("Indonesia");
    let (mut paid, mut refused) = (0, 0);
    for pos in land(&g) {
        let bonus = ai.nusantara_shore_site_bonus(&g, 0, pos);
        if !AdvancedAi::has_harbor_coast(&g, pos) {
            assert_eq!(bonus, 0.0, "inland");
            continue;
        }
        let expected = if g.city_site_water(0, pos).0 || !fresh_near(&g, pos) {
            NUSANTARA_SHORE_SITE_BONUS
        } else {
            0.0
        };
        assert_eq!(bonus, expected, "{pos:?}");
        if bonus > 0.0 {
            paid += 1;
        } else {
            refused += 1;
        }
    }
    assert!(paid > 0, "fixture: a shore seat earns the bonus");
    assert!(refused > 0, "fixture: a coast-only seat beside fresh water");
}

/// The full site score and the shortlist both carry the credit.
#[test]
fn the_site_score_and_the_shortlist_carry_the_bonus() {
    let mut on = AdvancedAi::new();
    on.enable_nusantara_settles_the_shore();
    let off = AdvancedAi::new();
    let g = board("Indonesia");
    let pos = land(&g)
        .into_iter()
        .find(|pos| on.nusantara_shore_site_bonus(&g, 0, *pos) > 0.0)
        .expect("a paid seat");
    let score = on.settlement_static_value_uncached(&g, 0, pos)
        - off.settlement_static_value_uncached(&g, 0, pos);
    assert!(
        (score - NUSANTARA_SHORE_SITE_BONUS).abs() < 1e-9,
        "the site score"
    );
    let shortlist = on.settlement_prefilter_score_for(&g, 0, pos)
        - off.settlement_prefilter_score_for(&g, 0, pos);
    assert!(
        (shortlist - NUSANTARA_SHORE_SITE_BONUS).abs() < 1e-9,
        "the shortlist"
    );
}
