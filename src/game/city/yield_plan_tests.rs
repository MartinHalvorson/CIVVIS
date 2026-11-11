//! Borrowed observation assignments preserve yields, probes and policy reads.

use super::*;

fn city(seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, seed, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    let cid = game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);
    game.cities.get_mut(&cid).unwrap().pop = 4;
    (game, cid)
}

fn bits(yields: Yields) -> [u64; 6] {
    [
        yields.food,
        yields.production,
        yields.gold,
        yields.science,
        yields.culture,
        yields.faith,
    ]
    .map(f64::to_bits)
}

#[test]
fn displayed_plans_match_independent_yield_derivations_across_city_changes() {
    let (mut game, cid) = city(91_991);
    for population in [1, 4, 9] {
        for queue in [
            vec![],
            vec![Item::Unit {
                unit: crate::name!("settler"),
            }],
            vec![Item::Building {
                building: crate::name!("granary"),
            }],
        ] {
            let state = game.cities.get_mut(&cid).unwrap();
            state.pop = population;
            state.queue = queue;
            let oracle = game.clone();
            let expected = oracle.city_yields(cid);
            let _memo = game.query_memo();
            let plan = game.city_citizen_plan(cid);
            assert_eq!(bits(game.city_yields_from_plan(cid, &plan)), bits(expected));
            assert_eq!(
                bits(game.city_yields(cid)),
                bits(expected),
                "ordinary reads share the same memo result"
            );
        }
    }
}

#[test]
fn observed_assignments_and_additive_corrections_are_preserved() {
    let (mut game, cid) = city(91_992);
    let pos = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != game.cities[&cid].pos)
        .unwrap();
    Arc::make_mut(&mut game.observed_city_worked_tiles).insert(cid, vec![pos]);
    Arc::make_mut(&mut game.observed_city_specialists).insert(cid, vec![]);
    Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            food: 0.125,
            production: -0.25,
            science: 1.75,
            ..Yields::default()
        },
    );
    let oracle = game.clone();
    let plan = game.city_citizen_plan(cid);
    assert_eq!(plan.worked_tiles, vec![pos]);
    assert_eq!(
        bits(game.city_yields_from_plan(cid, &plan)),
        bits(oracle.city_yields(cid))
    );
    assert_ne!(
        bits(oracle.city_yields(cid)),
        bits(oracle.city_yields_model(cid)),
        "the observed correction must be exercised"
    );
}

#[test]
fn a_borrowed_unweighted_plan_does_not_replace_weighted_probes() {
    let (game, cid) = city(91_993);
    let oracle = game.clone();
    let weights = Yields {
        food: 0.0,
        production: 12.0,
        science: 7.0,
        ..Yields::default()
    };
    let expected = oracle.city_yields_inner(cid, Some(weights), true);
    let _memo = game.query_memo();
    let plan = game.city_citizen_plan(cid);
    game.city_yields_from_plan(cid, &plan);
    assert_eq!(
        bits(game.city_yields_inner(cid, Some(weights), true)),
        bits(expected)
    );
}

#[test]
fn policy_traces_derive_the_plan_and_keep_the_original_nonempty_read_sets() {
    let (game, cid) = city(91_994);
    let oracle = game.clone();
    // Prime the same immutable inputs before each trace, as observation does.
    let mut plan = game.city_citizen_plan(cid);
    oracle.city_citizen_plan(cid);
    // If a trace accidentally reuses this supplied plan, its yields differ
    // and it omits the planner's policy questions.
    assert!(!plan.worked_tiles.is_empty());
    plan.worked_tiles.clear();
    plan.specialists.clear();
    let expected_trace = oracle.trace_policy_reads(0);
    let expected = oracle.city_yields(cid);
    let expected_reads = expected_trace.finish();
    let trace = game.trace_policy_reads(0);
    let actual = game.city_yields_from_plan(cid, &plan);
    let reads = trace.finish();
    assert_eq!(bits(actual), bits(expected));
    assert!(!expected_reads.effects.is_empty());
    assert_eq!(reads.effects, expected_reads.effects);
    assert_eq!(reads.cards, expected_reads.cards);
    assert_eq!(reads.opaque, expected_reads.opaque);
}
