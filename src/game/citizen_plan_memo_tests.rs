use super::*;

fn capitals() -> (Game, Vec<u32>) {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    for pid in 0..2 {
        while game.current != pid {
            let current = game.current;
            game.apply(current, &Action::EndTurn).unwrap();
        }
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    let ids: Vec<_> = game.cities.keys().copied().collect();
    for id in &ids {
        game.cities.get_mut(id).unwrap().pop = 3;
    }
    (game, ids)
}

#[test]
fn ordinary_assignments_match_without_a_memo_and_through_nested_scopes() {
    let (game, ids) = capitals();
    let expected: Vec<_> = ids.iter().map(|id| game.city_citizen_plan(*id)).collect();
    let _outer = game.query_memo();
    for _ in 0..3 {
        {
            let _inner = game.query_memo();
            for (id, plan) in ids.iter().zip(&expected) {
                assert_eq!(game.city_citizen_plan(*id), *plan);
            }
        }
        for (id, plan) in ids.iter().zip(&expected) {
            assert_eq!(game.city_citizen_plan(*id), *plan);
        }
    }
}

#[test]
fn returned_plans_are_owned_and_do_not_change_the_next_answer() {
    let (game, ids) = capitals();
    let id = ids[0];
    let expected = game.city_citizen_plan(id);
    let _memo = game.query_memo();
    let mut changed = game.city_citizen_plan(id);
    changed.worked_tiles.clear();
    changed.specialists.push("changed by the caller".into());
    changed.strategy.focus.clear();
    changed.strategy.weights.food = -100.0;
    assert_eq!(game.city_citizen_plan(id), expected);
}

#[test]
fn weighted_probes_neither_fill_nor_reuse_the_ordinary_assignment() {
    let (game, ids) = capitals();
    let id = ids[0];
    let weights = Yields {
        food: 0.0,
        production: 20.0,
        ..Yields::default()
    };
    let ordinary = game.city_citizen_plan_weighted_uncached(id, None);
    let weighted = game.city_citizen_plan_weighted_uncached(id, Some(weights));
    assert_ne!(ordinary.strategy.weights, weighted.strategy.weights);
    let _memo = game.query_memo();
    assert_eq!(game.city_citizen_plan_weighted(id, Some(weights)), weighted);
    assert_eq!(game.city_citizen_plan(id), ordinary);
    assert_eq!(game.city_citizen_plan_weighted(id, Some(weights)), weighted);
    assert_eq!(game.city_citizen_plan(id), ordinary);
}

#[test]
fn dropping_the_scope_allows_population_and_policy_changes() {
    let (mut game, ids) = capitals();
    let id = ids[0];
    let before = {
        let _memo = game.query_memo();
        game.city_citizen_plan(id)
    };
    game.cities.get_mut(&id).unwrap().pop += 3;
    let owner = game.cities[&id].owner;
    game.players[owner]
        .policies
        .insert(crate::name!("urban_planning"));
    let expected = game.city_citizen_plan_weighted_uncached(id, None);
    assert_ne!(expected, before);
    let _memo = game.query_memo();
    assert_eq!(game.city_citizen_plan(id), expected);
}

#[test]
fn a_search_branch_does_not_inherit_a_cached_assignment() {
    let (game, ids) = capitals();
    let id = ids[0];
    let _memo = game.query_memo();
    let original = game.city_citizen_plan(id);
    let mut branch = game.clone();
    branch.cities.get_mut(&id).unwrap().pop += 3;
    let expected = branch.city_citizen_plan_weighted_uncached(id, None);
    assert_ne!(expected, original);
    let _branch_memo = branch.query_memo();
    assert_eq!(branch.city_citizen_plan(id), expected);
    assert_eq!(game.city_citizen_plan(id), original);
}

#[test]
fn observed_assignments_stay_exact_and_weighted_probes_remain_counterfactual() {
    let (mut game, ids) = capitals();
    let id = ids[0];
    let position = game.cities[&id]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != game.cities[&id].pos)
        .unwrap();
    Arc::make_mut(&mut game.observed_city_worked_tiles).insert(id, vec![position]);
    Arc::make_mut(&mut game.observed_city_specialists).insert(id, vec!["campus".into()]);
    let ordinary = game.city_citizen_plan_weighted_uncached(id, None);
    let weights = Some(Yields {
        production: 30.0,
        ..Yields::default()
    });
    let weighted = game.city_citizen_plan_weighted_uncached(id, weights);
    let _memo = game.query_memo();
    assert_eq!(ordinary.worked_tiles, vec![position]);
    assert_eq!(ordinary.specialists, vec!["campus"]);
    assert_eq!(game.city_citizen_plan(id), ordinary);
    assert_eq!(game.city_citizen_plan_weighted(id, weights), weighted);
    assert_eq!(game.city_citizen_plan(id), ordinary);
}

#[test]
fn tracing_an_already_asked_plan_records_the_same_policy_questions() {
    let (game, ids) = capitals();
    let id = ids[0];
    let _memo = game.query_memo();
    game.city_citizen_plan(id);
    let actual = {
        let trace = game.trace_policy_reads(game.cities[&id].owner);
        game.city_citizen_plan(id);
        trace.finish()
    };
    let expected = {
        let trace = game.trace_policy_reads(game.cities[&id].owner);
        game.city_citizen_plan_weighted_uncached(id, None);
        trace.finish()
    };
    assert!(
        !expected.effects.is_empty(),
        "the comparison must observe real policy questions"
    );
    assert_eq!(actual.effects, expected.effects);
    assert_eq!(actual.cards, expected.cards);
    assert_eq!(actual.opaque, expected.opaque);
}
