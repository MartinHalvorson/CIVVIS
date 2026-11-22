use super::*;
use crate::rules::Yields;
use std::sync::Arc;

/// A one-city seat whose city can build a Galley (Sailing known, on the
/// coast), `short` Era Score below a Normal Age of 8, the era closing in five
/// turns, the city producing 40 a turn.
fn coastal_board(short: i64) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 14, 9_412, 120, 0, false);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.players[0].techs.insert(crate::name!("sailing"));
    let galley = Item::Unit {
        unit: crate::name!("galley"),
    };
    let mut tiles: Vec<Pos> = g.map.tiles.keys().copied().collect();
    tiles.sort_unstable();
    let mut city = None;
    for pos in tiles {
        let Some(tile) = g.map.get(pos) else {
            continue;
        };
        if g.rules.is_water(tile) || !g.rules.is_passable(tile) {
            continue;
        }
        let beside_water = g
            .wdisk(pos, 1)
            .into_iter()
            .any(|other| other != pos && g.map.get(other).is_some_and(|t| g.rules.is_water(t)));
        if !beside_water {
            continue;
        }
        let settler = g.spawn_test_unit("settler", 0, pos);
        if g.apply(0, &Action::FoundCity { unit: settler }).is_err() {
            g.remove_unit(settler);
            continue;
        }
        let cid = g.player_city_ids(0)[0];
        if g.can_produce(0, cid, &galley) {
            city = Some(cid);
            break;
        }
        panic!("fixture: the first founded coastal city cannot build a Galley");
    }
    let city = city.expect("fixture: a coastal city site");
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
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_first_ship_closes_the_dark_age();
    } else {
        ai.disable_first_ship_closes_the_dark_age();
    }
    ai
}

fn heads_a_ship(g: &Game, city: u32) -> bool {
    matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit })
        if g.rules.units[unit].domain.as_deref() == Some("sea"))
}

#[test]
fn the_gene_is_an_off_by_default_opt_in() {
    assert!(!AdvancedAi::new().first_ship_closes_the_dark_age);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(!live.first_ship_closes_the_dark_age);
    let enable = crate::ai::advanced::genes::GENES
        .iter()
        .find(|gene| gene.tag == "first-ship-closes-the-dark-age")
        .expect("an opt-in row")
        .enable;
    enable(&mut live);
    assert!(live.first_ship_closes_the_dark_age);
}

#[test]
fn two_short_with_the_era_closing_builds_the_first_ship() {
    for (gene, expected) in [(true, true), (false, false)] {
        let (mut g, city) = coastal_board(2);
        assert_eq!(seat(gene).claim_first_ship_for_the_age(&mut g, 0), expected);
        assert_eq!(heads_a_ship(&g, city), expected);
    }
}

#[test]
fn a_gap_the_ship_cannot_close_or_no_gap_is_left_alone() {
    for short in [0, 3, 5] {
        let (mut g, city) = coastal_board(short);
        assert!(
            seat(true).first_ship_age_short(&g, 0).is_none(),
            "short {short}"
        );
        assert!(!seat(true).claim_first_ship_for_the_age(&mut g, 0));
        assert!(!heads_a_ship(&g, city));
    }
}

#[test]
fn an_earned_moment_or_a_ship_afloat_is_left_alone() {
    // The host's award, as the mirror now books it.
    let (mut g, city) = coastal_board(1);
    g.players[0].counters.insert(
        "historic_moment_awards:MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA_IN_WORLD".into(),
        1,
    );
    assert!(!seat(true).claim_first_ship_for_the_age(&mut g, 0));
    assert!(!heads_a_ship(&g, city));

    // The engine's own first-ship guard.
    let (mut g, city) = coastal_board(1);
    g.players[0]
        .counters
        .insert("historic_moment:first_sea_unit".into(), 1);
    assert!(!seat(true).claim_first_ship_for_the_age(&mut g, 0));
    assert!(!heads_a_ship(&g, city));
}

#[test]
fn an_era_already_closed_is_left_alone() {
    let (mut g, city) = coastal_board(2);
    g.world_era_countdown_end = Some(g.turn);
    assert!(!seat(true).claim_first_ship_for_the_age(&mut g, 0));
    assert!(!heads_a_ship(&g, city));
}
