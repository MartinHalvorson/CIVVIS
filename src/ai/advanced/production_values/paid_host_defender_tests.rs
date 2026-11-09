use super::*;

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: Name::new(kind),
    }
}

/// Native Pericles 062123Z/t90: 66/80 Man-at-Arms, one host turn left,
/// only 3 Iron after starting it; the menu still offers that continuation.
fn fixture() -> (Game, u32, Item) {
    let mut g = Game::new_full(2, 40, 24, 483_190, 200, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.players[0].civ = "Greece".into();
    let city = g.found_city_for(0, (8, 12), None);
    g.current = 0;
    g.turn = 90;
    g.at_war.insert((0, 1));
    g.players[0].techs.extend([
        crate::name!("masonry"),
        crate::name!("bronze_working"),
        crate::name!("apprenticeship"),
    ]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 3.0);
    let defender = unit("man_at_arms");
    let center = g.cities.get_mut(&city).unwrap();
    center.queue = vec![defender.clone()];
    center.production = 66.0;
    center.buildings.push(crate::name!("walls"));
    center.wall_hp = 85;
    center.hp = CITY_MAX_HP;
    center.strategic_resource_commitments.clear();
    g.spawn_test_unit("crossbowman", 1, (9, 12));
    g.replace_host_unit_resource_prices(
        [(city, [(Game::production_block_key(&defender), 10.0)].into())].into(),
    );
    let repair = Item::Project {
        project: crate::name!("repair_outer_defenses"),
    };
    g.replace_host_menus(
        [(
            city,
            [
                (defender.clone(), 80.0),
                (unit("hoplite"), 32.0),
                (repair, 0.0),
            ]
            .into_iter()
            .map(|(item, cost)| {
                (
                    Game::production_block_key(&item),
                    crate::game::HostMenuEntry {
                        cost: Some(cost),
                        turns: Some(1.0),
                    },
                )
            })
            .collect(),
        )]
        .into(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    assert!(!g.can_produce(0, city, &defender));
    (g, city, defender)
}

fn answer(g: &mut Game, city: u32, target: VictoryTarget) -> Option<(u32, Item)> {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_garrison_under_fire();
    ai.redirect_unsafe_city_queue_for_defense(g, 0, Some(city))
}

#[test]
fn every_victory_target_keeps_the_host_confirmed_paid_defender() {
    for target in VictoryTarget::ALL {
        let (mut g, city, defender) = fixture();
        assert_eq!(
            answer(&mut g, city, target),
            Some((city, defender.clone())),
            "{target:?}: a paid defender must not be tested as a fresh start"
        );
        assert_eq!(g.cities[&city].queue.first(), Some(&defender));
        assert_eq!(g.cities[&city].production, 66.0);
        assert_eq!(g.strategic_stockpile(0, crate::name!("iron")), 3.0);
    }
}

#[test]
fn an_explicit_host_refusal_still_replaces_the_defender() {
    let (mut g, city, defender) = fixture();
    g.replace_blocked_production([(city, [Game::production_block_key(&defender)].into())].into());
    let claim = answer(&mut g, city, VictoryTarget::Culture).unwrap();
    assert_ne!(claim.1, defender);
    assert_eq!(g.cities[&city].queue.first(), Some(&claim.1));
    assert!(g.can_produce(0, city, &claim.1));
}

#[test]
fn missing_host_confirmation_keeps_the_local_legality_check() {
    let (mut g, city, defender) = fixture();
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .get_mut(&city)
        .unwrap()
        .remove(&Game::production_block_key(&defender));
    let claim = answer(&mut g, city, VictoryTarget::Culture).unwrap();
    assert_ne!(claim.1, defender);
    assert!(g.can_produce(0, city, &claim.1));
}

#[test]
fn an_offered_paused_defender_does_not_receive_active_queue_credit() {
    let (mut g, city, defender) = fixture();
    g.cities.get_mut(&city).unwrap().queue = vec![unit("trader")];
    let claim = answer(&mut g, city, VictoryTarget::Culture).unwrap();
    assert_ne!(claim.1, defender);
    assert!(g.can_produce(0, city, &claim.1));
}

#[test]
fn host_confirmation_does_not_protect_an_offensive_siege_queue() {
    let (mut g, city, _) = fixture();
    let siege = unit("catapult");
    g.players[0].techs.insert(crate::name!("engineering"));
    g.cities.get_mut(&city).unwrap().queue = vec![siege.clone()];
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .get_mut(&city)
        .unwrap()
        .insert(
            Game::production_block_key(&siege),
            crate::game::HostMenuEntry {
                cost: Some(60.0),
                turns: Some(1.0),
            },
        );
    assert!(AdvancedAi::production_commitment_is_legal(
        &g, 0, city, &siege
    ));
    let claim = answer(&mut g, city, VictoryTarget::Culture).unwrap();
    assert_ne!(claim.1, siege);
    assert!(g.can_produce(0, city, &claim.1));
}
