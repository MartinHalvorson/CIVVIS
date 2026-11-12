//! Building-effect memo lifetimes, edited city copies, and exact scalar sums.
use super::*;

const EFFECT: &str = "cereal_resource_food";

fn fixture() -> (Game, u32) {
    let mut game = Game::new_full(2, 24, 16, 91_173, 100, 0, false);
    let position = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "settler")
        .unwrap()
        .pos;
    let city = game.place_city(0, position, None);
    let buildings = &mut game.cities.get_mut(&city).unwrap().buildings;
    buildings.clear();
    buildings.push(crate::name!("water_mill"));
    (game, city)
}

fn original(game: &Game, city: &City, effect: &str) -> f64 {
    if !game.rules.effect_index.buildings(effect) {
        return 0.0;
    }
    city.buildings
        .iter()
        .filter_map(|building| {
            let value = game.rules.buildings[building]
                .effects
                .get(effect)
                .copied()?;
            (!city.pillaged_buildings.contains(building)
                && game.building_district_is_active(city, building))
            .then_some(value)
        })
        .sum()
}

#[test]
fn building_effect_memo_lives_only_until_the_outer_query_ends() {
    let (mut game, cid) = fixture();
    let expected = original(&game, &game.cities[&cid], EFFECT);
    assert!(expected > 0.0);
    assert_eq!(
        game.city_building_effect(&game.cities[&cid], EFFECT),
        expected
    );
    assert!(game.query_memo.building_effects.borrow().is_none());
    {
        let _outer = game.query_memo();
        {
            let _inner = game.query_memo();
            assert_eq!(
                game.city_building_effect(&game.cities[&cid], EFFECT),
                expected
            );
        }
        assert_eq!(
            game.query_memo.building_effects.borrow().as_ref().unwrap()[&cid][EFFECT],
            expected
        );
        assert_eq!(
            game.city_building_effect(&game.cities[&cid], EFFECT),
            expected
        );
    }
    assert!(game.query_memo.building_effects.borrow().is_none());
    game.cities
        .get_mut(&cid)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("water_mill"));
    let _next = game.query_memo();
    assert_eq!(
        game.city_building_effect(&game.cities[&cid], EFFECT)
            .to_bits(),
        original(&game, &game.cities[&cid], EFFECT).to_bits()
    );
    assert_eq!(game.city_building_effect(&game.cities[&cid], EFFECT), 0.0);
}

#[test]
fn building_effect_memo_does_not_answer_an_edited_copy_of_the_same_city() {
    let (game, cid) = fixture();
    let _memo = game.query_memo();
    let actual = &game.cities[&cid];
    let baseline = game.city_building_effect(actual, EFFECT);
    let mut edited = actual.clone();
    edited.buildings.clear();
    assert_eq!(edited.id, actual.id);
    assert_ne!(original(&game, &edited, EFFECT), baseline);
    assert_eq!(
        game.city_building_effect(&edited, EFFECT).to_bits(),
        original(&game, &edited, EFFECT).to_bits()
    );
    assert_eq!(game.city_building_effect(actual, EFFECT), baseline);
    // A detached city with an unknown id cannot create a cache entry either.
    edited.id = u32::MAX;
    game.city_building_effect(&edited, EFFECT);
    assert_eq!(
        game.query_memo
            .building_effects
            .borrow()
            .as_ref()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn building_effect_memo_keeps_all_ruleset_effects_equal_with_inactive_buildings() {
    let (mut game, cid) = fixture();
    let buildings = game.rules.buildings.keys().copied().collect::<Vec<_>>();
    let effects = game
        .rules
        .buildings
        .values()
        .flat_map(|spec| spec.effects.keys().cloned())
        .collect::<BTreeSet<_>>();
    game.cities.get_mut(&cid).unwrap().buildings = buildings;
    let position = game.nbrs(game.cities[&cid].pos)[0];
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), position);
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(crate::name!("campus"));
    for pillaged in [false, true] {
        game.map.tiles.get_mut(&position).unwrap().pillaged = pillaged;
        if pillaged {
            game.cities
                .get_mut(&cid)
                .unwrap()
                .pillaged_buildings
                .insert(crate::name!("water_mill"));
        }
        let _memo = game.query_memo();
        for effect in &effects {
            let expected = original(&game, &game.cities[&cid], effect).to_bits();
            for _ in 0..2 {
                assert_eq!(
                    game.city_building_effect(&game.cities[&cid], effect)
                        .to_bits(),
                    expected,
                    "{effect}, pillaged={pillaged}"
                );
            }
        }
    }
}

#[test]
fn building_effect_memo_preserves_signed_zero_nonfinite_values_and_sum_order() {
    let (mut game, cid) = fixture();
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .extend([crate::name!("granary"), crate::name!("monument")]);
    for (first, second, third) in [
        (0.0, -0.0, -0.0),
        (-0.0, -0.0, -0.0),
        (f64::NAN, 1.0, 0.0),
        (f64::INFINITY, f64::NEG_INFINITY, 2.0),
        (1e16, 1.0, -1e16),
        (-1e16, 1e16, 1.0),
    ] {
        let rules = Arc::make_mut(&mut game.rules);
        for (building, coefficient) in [
            ("water_mill", first),
            ("granary", second),
            ("monument", third),
        ] {
            rules
                .buildings
                .get_mut(building)
                .unwrap()
                .effects
                .insert(EFFECT.to_string(), coefficient);
        }
        let expected = original(&game, &game.cities[&cid], EFFECT).to_bits();
        let _memo = game.query_memo();
        for _ in 0..2 {
            assert_eq!(
                game.city_building_effect(&game.cities[&cid], EFFECT)
                    .to_bits(),
                expected
            );
        }
        assert_eq!(
            game.city_building_effect(&game.cities[&cid], "not_a_building_effect")
                .to_bits(),
            0.0_f64.to_bits()
        );
    }
}

#[test]
fn building_effect_memo_is_not_inherited_by_a_game_clone() {
    let (game, cid) = fixture();
    let _memo = game.query_memo();
    let baseline = game.city_building_effect(&game.cities[&cid], EFFECT);
    for mut branch in [game.clone(), game.speculative_clone()] {
        assert!(branch.query_memo.building_effects.borrow().is_none());
        branch.cities.get_mut(&cid).unwrap().buildings.clear();
        let _memo = branch.query_memo();
        let expected = original(&branch, &branch.cities[&cid], EFFECT);
        assert_ne!(expected, baseline);
        assert_eq!(
            branch
                .city_building_effect(&branch.cities[&cid], EFFECT)
                .to_bits(),
            expected.to_bits()
        );
    }
    assert_eq!(
        game.city_building_effect(&game.cities[&cid], EFFECT),
        baseline
    );
}

#[test]
fn building_effect_memo_is_used_normally_and_bypassed_during_policy_tracing() {
    let (game, cid) = fixture();
    let _memo = game.query_memo();
    let expected = original(&game, &game.cities[&cid], EFFECT);
    game.city_building_effect(&game.cities[&cid], EFFECT);
    // A deliberately distinct cached value makes the two paths observable.
    game.query_memo
        .building_effects
        .borrow_mut()
        .as_mut()
        .unwrap()
        .get_mut(&cid)
        .unwrap()
        .insert(EFFECT.to_string(), 917.0);
    assert_eq!(game.city_building_effect(&game.cities[&cid], EFFECT), 917.0);
    let trace = game.trace_policy_reads(0);
    assert_eq!(
        game.city_building_effect(&game.cities[&cid], EFFECT),
        expected
    );
    let reads = trace.finish();
    let reference_trace = game.trace_policy_reads(0);
    assert_eq!(original(&game, &game.cities[&cid], EFFECT), expected);
    let reference_reads = reference_trace.finish();
    for card in game.rules.policies.keys() {
        assert_eq!(
            reads.card_is_inert(&game.rules, *card),
            reference_reads.card_is_inert(&game.rules, *card)
        );
    }
}
