use super::*;
use crate::rules::Yields;
use std::sync::Arc;

/// A one-city seat that can train a Horseman (Horseback Riding, 40 Horses in
/// stock), `short` Era Score below a Normal Age of 8, the era closing in five
/// turns, the city producing 40 a turn.
fn board(short: i64) -> (Game, u32) {
    let mut g = Game::new_full(1, 18, 10, 9_517, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    let city = g.player_city_ids(0)[0];
    g.players[0].techs.insert(crate::name!("horseback_riding"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("horses"), 40.0);
    g.players[0].normal_age_threshold = 8;
    g.players[0].era_score = 8 - short;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        city,
        Yields {
            production: 40.0,
            ..Default::default()
        },
    );
    // One turn more than the build needs: inside the closers' window.
    let (_, turns) =
        AdvancedAi::strategic_unit_for(&g, 0, city).expect("fixture: a strategic unit to train");
    g.world_era_countdown_end = Some(g.turn + turns as u32 + 1);
    (g, city)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_strategic_unit_closes_the_dark_age();
    } else {
        ai.disable_strategic_unit_closes_the_dark_age();
    }
    ai
}

fn heads_a_strategic_unit(g: &Game, city: u32) -> bool {
    matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit })
        if g.rules.units[unit].requires_resource.is_some())
}

#[test]
fn the_gene_is_an_off_by_default_opt_in() {
    assert!(!AdvancedAi::new().strategic_unit_closes_the_dark_age);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(!live.strategic_unit_closes_the_dark_age);
    let enable = crate::ai::advanced::genes::GENES
        .iter()
        .find(|gene| gene.tag == "strategic-unit-closes-the-dark-age")
        .expect("an opt-in row")
        .enable;
    enable(&mut live);
    assert!(live.strategic_unit_closes_the_dark_age);
}

#[test]
fn one_short_with_the_era_closing_trains_the_first_horseman() {
    for (gene, expected) in [(true, true), (false, false)] {
        let (mut g, city) = board(1);
        assert!(g.can_produce(
            0,
            city,
            &Item::Unit {
                unit: crate::name!("horseman")
            }
        ));
        assert_eq!(
            seat(gene).claim_strategic_unit_for_the_age(&mut g, 0),
            expected
        );
        assert_eq!(heads_a_strategic_unit(&g, city), expected);
    }
}

#[test]
fn two_short_or_none_short_is_left_alone() {
    for short in [0, 2, 4] {
        let (mut g, city) = board(short);
        assert!(
            seat(true).strategic_unit_age_short(&g, 0).is_none(),
            "short {short}"
        );
        assert!(!seat(true).claim_strategic_unit_for_the_age(&mut g, 0));
        assert!(!heads_a_strategic_unit(&g, city));
    }
}

#[test]
fn a_resource_already_used_pays_nothing() {
    // The engine's own per-resource guard.
    let (mut g, city) = board(1);
    g.players[0]
        .counters
        .insert("historic_moment:first_unit_using_resource:horses".into(), 1);
    assert!(!seat(true).claim_strategic_unit_for_the_age(&mut g, 0));
    assert!(!heads_a_strategic_unit(&g, city));

    // A Horseman of ours on the map.
    let (mut g, city) = board(1);
    let pos = g.cities[&city].pos;
    g.spawn_test_unit("horseman", 0, pos);
    assert!(!seat(true).claim_strategic_unit_for_the_age(&mut g, 0));
    assert!(!heads_a_strategic_unit(&g, city));
}

#[test]
fn a_unit_finished_on_the_eras_last_turn_is_too_late() {
    for (spare, expected) in [(0, false), (1, true)] {
        let (mut g, city) = board(1);
        let (_, turns) =
            AdvancedAi::strategic_unit_for(&g, 0, city).expect("a strategic unit it can train");
        g.world_era_countdown_end = Some(g.turn + turns as u32 + spare);
        g.players[0].gold = 0.0;
        assert_eq!(
            seat(true).claim_strategic_unit_for_the_age(&mut g, 0),
            expected,
            "{turns} whole turns, {spare} spare"
        );
        assert_eq!(heads_a_strategic_unit(&g, city), expected);
    }
}

#[test]
fn the_unique_units_head_is_left_alone() {
    let (mut g, city) = board(1);
    g.players[0].civ = "Greece".into();
    g.players[0].techs.insert(crate::name!("bronze_working"));
    let hoplite = Item::Unit {
        unit: crate::name!("hoplite"),
    };
    assert!(
        g.can_produce(0, city, &hoplite),
        "fixture: Greece trains Hoplites"
    );
    g.apply(
        0,
        &Action::Produce {
            city,
            item: hoplite.clone(),
        },
    )
    .unwrap();
    assert!(!seat(true).claim_strategic_unit_for_the_age(&mut g, 0));
    assert_eq!(g.cities[&city].queue.first(), Some(&hoplite));
}
