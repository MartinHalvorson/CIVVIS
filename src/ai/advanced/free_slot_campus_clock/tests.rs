use super::*;
use crate::setup::GameSpeed;

fn fixture() -> (Game, AdvancedAi, u32, Item) {
    let mut g = Game::new_full(1, 44, 26, 950_108, 500, 0, false);
    let pos = g.units[&g.player_unit_ids(0)[0]].pos;
    let cid = g.found_city_for(0, pos, None);
    while g.player_city_ids(0).len() < 6 {
        let next = g
            .map
            .tiles
            .values()
            .filter(|t| {
                g.rules.is_passable(t)
                    && !g.rules.is_water(t)
                    && t.owner_city.is_none()
                    && t.district.is_none()
                    && g.cities.values().all(|c| g.wdist(c.pos, t.pos) >= 5)
            })
            .map(|t| t.pos)
            .min()
            .expect("six city sites");
        g.found_city_for(0, next, None);
    }
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.game_speed = GameSpeed::Standard;
    g.turn = 100;
    g.players[0].techs.insert(crate::name!("writing"));
    for city in g.cities.values_mut() {
        city.pop = 4;
    }
    let campus =
        BasicAi::first_campus_item_with_budget(&g, 0, cid, 1000.0).expect("legal Campus site");
    let mut menus = BTreeMap::new();
    menus.entry(cid).or_insert_with(BTreeMap::new).insert(
        Game::production_block_key(&campus),
        crate::game::HostMenuEntry {
            cost: Some(90.0),
            turns: Some(22.0),
        },
    );
    g.replace_host_menus(menus, BTreeMap::new(), BTreeMap::new());
    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot_2();
    (g, ai, cid, campus)
}

#[test]
fn free_slot_campus_clock_scales_the_ceiling_once_and_is_online_noop() {
    let (mut g, mut ai, _, _) = fixture();
    for (speed, expected) in [
        (GameSpeed::Online, 15.0),
        (GameSpeed::Quick, 20.1),
        (GameSpeed::Standard, 30.0),
        (GameSpeed::Epic, 45.0),
        (GameSpeed::Marathon, 90.0),
    ] {
        g.game_speed = speed;
        assert_eq!(ai.free_slot_campus_turn_budget(&g), 15.0);
        ai.enable_free_slot_campus_clock();
        assert!((ai.free_slot_campus_turn_budget(&g) - expected).abs() < 1e-9);
        ai.disable_free_slot_campus_clock();
    }
}

#[test]
fn free_slot_campus_clock_uses_native_quote_and_keeps_other_picker_unchanged() {
    let (g, mut ai, cid, campus) = fixture();
    let plan = ai.assess(&g, 0);
    assert!(ai.free_slot_campus_item(&g, 0, cid, &plan).is_none());
    ai.enable_free_slot_campus_clock();
    assert_eq!(
        ai.free_slot_campus_item(&g, 0, cid, &plan),
        Some((campus, 22.0))
    );
    assert!(BasicAi::first_campus_item(&g, 0, cid).is_none());
    let mut online = g.clone();
    online.game_speed = GameSpeed::Online;
    assert!(ai.free_slot_campus_item(&online, 0, cid, &plan).is_none());
}

#[test]
fn free_slot_campus_clock_requires_parent_and_preserves_eligibility_holds() {
    let (g, mut ai, cid, _) = fixture();
    ai.enable_free_slot_campus_clock();
    let plan = ai.assess(&g, 0);
    assert!(ai.free_slot_campus_item(&g, 0, cid, &plan).is_some());
    ai.disable_campus_claims_the_free_slot_2();
    assert!(ai.free_slot_campus_item(&g, 0, cid, &plan).is_none());
    ai.enable_campus_claims_the_free_slot_2();
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(cid);
    assert!(ai.free_slot_campus_item(&g, 0, cid, &threatened).is_none());
    let mut attacked = g.clone();
    attacked.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1;
    assert!(ai.free_slot_campus_item(&attacked, 0, cid, &plan).is_none());
    let mut small = g.clone();
    small.cities.get_mut(&cid).unwrap().pop = 1;
    assert!(ai.free_slot_campus_item(&small, 0, cid, &plan).is_none());
}

#[test]
fn free_slot_campus_clock_keeps_walls_settlers_and_builders_but_claims_routine_queue() {
    let (g, mut ai, cid, campus) = fixture();
    ai.enable_free_slot_campus_clock();
    let plan = ai.assess(&g, 0);
    for item in [
        Item::Building {
            building: crate::name!("walls"),
        },
        Item::Unit {
            unit: crate::name!("settler"),
        },
        Item::Unit {
            unit: crate::name!("builder"),
        },
    ] {
        let mut case = g.clone();
        case.cities.get_mut(&cid).unwrap().queue = vec![item.clone()];
        ai.claim_free_slot_campus(&mut case, 0, &plan);
        assert_eq!(case.cities[&cid].queue.first(), Some(&item));
    }
    let mut on = g;
    on.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("knight"),
    }];
    ai.claim_free_slot_campus(&mut on, 0, &plan);
    assert_eq!(on.cities[&cid].queue.first(), Some(&campus));
    assert!(ai.free_slot_campus_holds(&on, 0, cid, &plan, &campus));
}

#[test]
fn free_slot_campus_clock_also_uses_modeled_turns_without_native_quotes() {
    let (mut g, mut ai, cid, _) = fixture();
    g.replace_host_menus(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let campus = BasicAi::first_campus_item_with_budget(&g, 0, cid, 1000.0).unwrap();
    let target_production = g.item_cost_for(0, &campus) / 22.0;
    let adjustment = target_production - g.city_yields(cid).production;
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: adjustment,
            ..Default::default()
        },
    );
    let plan = ai.assess(&g, 0);
    assert!(ai.free_slot_campus_item(&g, 0, cid, &plan).is_none());
    ai.enable_free_slot_campus_clock();
    let (_, turns) = ai.free_slot_campus_item(&g, 0, cid, &plan).unwrap();
    assert!((turns - 22.0).abs() < 1e-8);
}
