use super::*;

fn fixture(target: VictoryTarget) -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 24, 16, 91_515, 150, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
    }
    let city = g.found_city_for(0, (5, 5), None);
    let wood = (6, 5);
    let start = (5, 6);
    assert!(g.cities[&city].owned_tiles.contains(&wood));
    assert!(g.cities[&city].owned_tiles.contains(&start));
    g.map.tiles.get_mut(&wood).unwrap().feature = Some(crate::name!("forest"));
    g.map.tiles.get_mut(&start).unwrap().hills = true;
    g.players[0].techs.insert(crate::name!("mining"));
    let c = g.cities.get_mut(&city).unwrap();
    c.pop = 2;
    c.production = 0.0;
    c.queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    let builder = g.spawn_test_unit("builder", 0, start);
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_chop_for_expansion();
    (g, ai, builder, start, wood)
}

#[test]
fn a_refused_clearing_leaves_remote_jobs_and_other_builders_usable() {
    for target in [
        VictoryTarget::Culture,
        VictoryTarget::Science,
        VictoryTarget::Religious,
        VictoryTarget::Diplomatic,
        VictoryTarget::Domination,
        VictoryTarget::Score,
    ] {
        let (mut g, mut ai, builder, start, wood) = fixture(target);
        let mut control = g.clone();
        let mut control_ai = ai.clone();
        assert!(control_ai.advanced_builder_step(
            &mut control,
            0,
            builder,
            GrandStrategy::Expansion
        ));
        assert_ne!(
            control.units[&builder].pos, start,
            "unblocked deadline sends the Builder to its chop"
        );

        let original = Arc::clone(&g.blocked_improvements);
        ai.set_refused_builder_operations(BTreeMap::from([(
            builder,
            BTreeSet::from([
                crate::name!("chop_woods"),
                crate::name!("chop_rainforest"),
                crate::name!("clear_marsh"),
            ]),
        )]));
        assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Expansion));
        assert_eq!(
            g.units[&builder].pos, start,
            "the refused worker takes the useful local mine"
        );
        assert_eq!(g.map.tiles[&start].improvement.as_deref(), Some("mine"));
        assert_eq!(g.map.tiles[&wood].feature.as_deref(), Some("forest"));
        assert_eq!(
            g.blocked_improvements, original,
            "the temporary mask is restored"
        );

        let other = g.spawn_test_unit("builder", 0, wood);
        assert!(ai.advanced_builder_step(&mut g, 0, other, GrandStrategy::Expansion));
        assert!(
            g.map.tiles[&wood].feature.is_none(),
            "another Builder can still chop"
        );
    }
}

#[test]
fn the_mask_restores_existing_exclusions_after_a_builders_last_charge() {
    let (mut g, mut ai, builder, start, _) = fixture(VictoryTarget::Culture);
    g.units.get_mut(&builder).unwrap().charges = 1;
    Arc::make_mut(&mut g.blocked_improvements)
        .entry((4, 5))
        .or_default()
        .insert(crate::name!("farm"));
    let original = Arc::clone(&g.blocked_improvements);
    ai.set_refused_builder_operations(BTreeMap::from([(
        builder,
        BTreeSet::from([crate::name!("chop_woods")]),
    )]));
    assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Expansion));
    assert_eq!(g.map.tiles[&start].improvement.as_deref(), Some("mine"));
    assert!(!g.units.contains_key(&builder));
    assert_eq!(g.blocked_improvements, original);
}
