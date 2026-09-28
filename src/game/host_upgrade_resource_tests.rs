use super::*;

fn seat() -> (Game, Vec<u32>) {
    let mut game = Game::new_full(2, 26, 16, 74_120, 250, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    let pos = game.units[&settler].pos;
    game.found_city_for(0, pos, None);
    game.players[0].civ = "Egypt".into();
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0].gold = 1_000.0;
    let units = (0..2)
        .map(|_| game.spawn_test_unit("warrior", 0, pos))
        .collect();
    (game, units)
}

fn offer(game: &mut Game, uid: u32, target: Name, resource: Option<Name>, cost: f64) {
    Arc::make_mut(&mut game.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            upgrade: Some(HostUnitUpgrade {
                to: Some(target),
                cost: Some(25.0),
                resources: Some(HostUnitUpgradeResource { resource, cost }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
}

#[test]
fn host_upgrade_material_is_complete_and_sequential_deductions_are_exact() {
    let (mut game, units) = seat();
    game.players[0].policies.insert(crate::name!("retinues"));
    game.units.get_mut(&units[0]).unwrap().formation = 1;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    for uid in &units {
        offer(
            &mut game,
            *uid,
            crate::name!("swordsman"),
            Some(crate::name!("iron")),
            10.0,
        );
    }
    for (index, uid) in units.iter().enumerate() {
        assert_eq!(game.unit_gold_upgrade_offer(0, *uid).unwrap().2, 10.0);
        game.apply(0, &Action::UpgradeUnit { unit: *uid }).unwrap();
        assert_eq!(
            game.strategic_stockpile(0, crate::name!("iron")),
            10.0 - 10.0 * index as f64
        );
    }
    assert_eq!(game.players[0].gold, 950.0);
    assert!(game
        .apply(0, &Action::UpgradeUnit { unit: units[0] })
        .is_err());
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
}

#[test]
fn zero_and_a_later_native_successor_keep_their_exact_material_bill() {
    let (mut game, units) = seat();
    let uid = units[0];
    offer(&mut game, uid, crate::name!("swordsman"), None, 0.0);
    assert_eq!(game.unit_gold_upgrade_offer(0, uid).unwrap().2, 0.0);
    offer(
        &mut game,
        uid,
        crate::name!("musketman"),
        Some(crate::name!("niter")),
        7.0,
    );
    game.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 10.0);
    assert_eq!(
        game.unit_gold_upgrade_offer(0, uid).unwrap(),
        (crate::name!("musketman"), 25.0, 7.0)
    );
    game.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 10.0);
    game.apply(0, &Action::UpgradeUnit { unit: uid }).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 3.0);
}

#[test]
fn an_earlier_upgrade_can_spend_the_material_a_native_permission_observed() {
    let (mut game, units) = seat();
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 15.0);
    for uid in &units {
        offer(
            &mut game,
            *uid,
            crate::name!("swordsman"),
            Some(crate::name!("iron")),
            10.0,
        );
    }
    game.apply(0, &Action::UpgradeUnit { unit: units[0] })
        .unwrap();
    assert_eq!(
        game.unit_gold_upgrade_detail(0, units[1]),
        Err("not enough strategic material")
    );
    assert!(game
        .apply(0, &Action::UpgradeUnit { unit: units[1] })
        .is_err());
    assert_eq!(game.players[0].gold, 975.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 5.0);
}

#[test]
fn unknown_wrong_successor_or_resource_and_invalid_bills_keep_the_fallback() {
    let (mut game, units) = seat();
    let uid = units[0];
    let target = crate::name!("swordsman");
    assert_eq!(game.unit_upgrade_resource_price(0, uid, target), Some(20.0));
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        offer(&mut game, uid, target, Some(crate::name!("iron")), invalid);
        assert_eq!(game.unit_upgrade_resource_price(0, uid, target), Some(20.0));
    }
    offer(&mut game, uid, target, Some(crate::name!("niter")), 10.0);
    assert_eq!(game.unit_upgrade_resource_price(0, uid, target), Some(20.0));
    offer(
        &mut game,
        uid,
        crate::name!("musketman"),
        Some(crate::name!("iron")),
        10.0,
    );
    assert_eq!(game.unit_upgrade_resource_price(0, uid, target), Some(20.0));
    assert_eq!(game.unit_upgrade_resource_price(1, uid, target), None);
    offer(&mut game, uid, target, Some(crate::name!("iron")), 10.0);
    Arc::make_mut(&mut game.host_unit_facts)
        .get_mut(&uid)
        .unwrap()
        .upgrade
        .as_mut()
        .unwrap()
        .blocked = Some("RESOURCE".into());
    assert!(
        game.unit_gold_upgrade_offer(0, uid).is_none(),
        "a known bill grants no permission"
    );
}
