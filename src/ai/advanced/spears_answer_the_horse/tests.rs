use super::*;

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: Name::new(kind),
    }
}

fn quote(g: &mut Game, cid: u32, item: &Item, cost: f64, turns: Option<f64>) {
    let mut menus = (*g.host_buildable).clone();
    menus.entry(cid).or_default().insert(
        Game::production_block_key(item),
        crate::game::HostMenuEntry {
            cost: Some(cost),
            turns,
        },
    );
    g.replace_host_menus(menus, BTreeMap::new(), BTreeMap::new());
}

fn fixture() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new(2, 24, 16, 774_4071, 250, 0);
    g.players[0].civ = "GranColombia".to_string();
    let center = g.units[&g.player_unit_ids(0)[0]].pos;
    let cid = g.found_city_for(0, center, None);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.spawn_test_unit("warrior", 0, center);
    let near = g
        .nbrs(center)
        .into_iter()
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .unwrap();
    let knight = g.spawn_test_unit("knight", 1, near);
    g.at_war.insert((0, 1));
    g.players[0].techs.extend([
        crate::name!("bronze_working"),
        crate::name!("wheel"),
        crate::name!("masonry"),
    ]);
    let city = g.cities.get_mut(&cid).unwrap();
    city.buildings.push(crate::name!("walls"));
    city.wall_hp = 100;
    city.hp = 180;
    quote(&mut g, cid, &unit("heavy_chariot"), 33.0, Some(4.0));
    quote(&mut g, cid, &unit("spearman"), 33.0, Some(3.0));
    quote(&mut g, cid, &unit("builder"), 35.0, Some(4.0));
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: unit("heavy_chariot"),
        },
    )
    .unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    ai.enable_spears_answer_the_horse();
    assert!(g.can_produce(0, cid, &unit("spearman")));
    assert_eq!(
        ai.base.best_military(&g, 0, cid, Some(false)).as_deref(),
        Some("heavy_chariot")
    );
    (g, ai, cid, knight)
}

#[test]
fn spears_answer_the_horse_changes_the_final_defense_queue_only_when_enabled() {
    let (g, mut ai, cid, _) = fixture();
    let mut off = g.clone();
    ai.disable_spears_answer_the_horse();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut off, 0, Some(cid));
    assert_eq!(claim, Some((cid, unit("heavy_chariot"))));
    assert_eq!(off.cities[&cid].queue.first(), Some(&unit("heavy_chariot")));
    ai.enable_spears_answer_the_horse();
    let mut on = g;
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut on, 0, Some(cid));
    assert_eq!(claim, Some((cid, unit("spearman"))));
    assert_eq!(on.cities[&cid].queue.first(), Some(&unit("spearman")));
    on.cities.get_mut(&cid).unwrap().queue = vec![unit("builder")];
    ai.reapply_confirmed_defense_queue(&mut on, 0, claim.as_ref());
    assert_eq!(on.cities[&cid].queue.first(), Some(&unit("spearman")));
}

#[test]
fn spears_answer_the_horse_keeps_paid_and_one_turn_defenders() {
    let (mut g, ai, cid, _) = fixture();
    for paid in [1.0, 32.0] {
        g.cities.get_mut(&cid).unwrap().production = paid;
        ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
        assert_eq!(g.cities[&cid].queue.first(), Some(&unit("heavy_chariot")));
        assert_eq!(g.cities[&cid].production, paid);
    }
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    quote(&mut g, cid, &unit("heavy_chariot"), 33.0, Some(1.0));
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_eq!(g.cities[&cid].queue.first(), Some(&unit("heavy_chariot")));
}

#[test]
fn spears_answer_the_horse_requires_legal_no_slower_no_costlier_host_offer() {
    let (g, ai, cid, _) = fixture();
    let selected = unit("heavy_chariot");
    for (cost, turns) in [
        (34.0, Some(3.0)),
        (33.0, Some(5.0)),
        (33.0, None),
        (33.0, Some(f64::NAN)),
        (33.0, Some(0.0)),
    ] {
        let mut case = g.clone();
        quote(&mut case, cid, &unit("spearman"), cost, turns);
        assert!(ai
            .spear_for_local_horse(&case, 0, cid, &selected, Some(&selected))
            .is_none());
    }
    for item in [selected.clone(), unit("spearman")] {
        let mut missing_cost = g.clone();
        let mut menus = (*missing_cost.host_buildable).clone();
        menus
            .get_mut(&cid)
            .unwrap()
            .get_mut(&Game::production_block_key(&item))
            .unwrap()
            .cost = None;
        missing_cost.replace_host_menus(menus, BTreeMap::new(), BTreeMap::new());
        assert!(ai
            .spear_for_local_horse(&missing_cost, 0, cid, &selected, Some(&selected))
            .is_none());
    }
    let mut illegal = g;
    let mut menus = (*illegal.host_buildable).clone();
    menus
        .get_mut(&cid)
        .unwrap()
        .remove(&Game::production_block_key(&unit("spearman")));
    illegal.replace_host_menus(menus, BTreeMap::new(), BTreeMap::new());
    assert!(ai
        .spear_for_local_horse(&illegal, 0, cid, &selected, Some(&selected))
        .is_none());
}

#[test]
fn spears_answer_the_horse_respects_war_cart_immunity_and_stronger_mixed_melee() {
    let (mut g, ai, cid, knight) = fixture();
    let pos = g.units[&knight].pos;
    let selected = unit("heavy_chariot");
    g.remove_unit(knight);
    let cart = g.spawn_test_unit("war_cart", 1, pos);
    assert!(ai
        .spear_for_local_horse(&g, 0, cid, &selected, Some(&selected))
        .is_none());
    g.remove_unit(cart);
    g.spawn_test_unit("knight", 1, pos);
    g.spawn_test_unit("musketman", 1, pos);
    assert!(ai
        .spear_for_local_horse(&g, 0, cid, &selected, Some(&selected))
        .is_none());
    assert_eq!(
        class_bonus(&g, crate::name!("spearman"), crate::name!("keshig")),
        10.0
    );
    assert_eq!(
        class_bonus(&g, crate::name!("musketman"), crate::name!("spearman")),
        5.0
    );
}

#[test]
fn spears_answer_the_horse_does_not_create_an_emergency_or_replace_fortification() {
    let (mut g, ai, cid, _) = fixture();
    let selected = unit("heavy_chariot");
    for protected in [
        Item::Building {
            building: crate::name!("walls"),
        },
        Item::Project {
            project: crate::name!("repair_outer_defenses"),
        },
        unit("archer"),
    ] {
        assert!(ai
            .spear_for_local_horse(&g, 0, cid, &selected, Some(&protected))
            .is_none());
        assert!(ai
            .spear_for_local_horse(&g, 0, cid, &protected, None)
            .is_none());
    }
    assert!(
        ai.spear_for_local_horse(&g, 0, cid, &unit("archer"), Some(&selected))
            .is_none(),
        "an existing shooter selection keeps its priority over a fresh melee head"
    );
    let mut unarmed = AdvancedAi::new();
    unarmed.enable_spears_answer_the_horse();
    assert_eq!(
        unarmed.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid)),
        None
    );
    assert_eq!(g.cities[&cid].queue.first(), Some(&selected));
}

#[test]
fn spears_answer_the_horse_fresh_refinement_yields_to_an_existing_unsafe_queue() {
    let (mut g, ai, cid, _) = fixture();
    let center = g.cities[&cid].pos;
    let pos = g
        .wdisk(center, 6)
        .into_iter()
        .find(|pos| {
            g.wdist(center, *pos) >= 4
                && g.map.get(*pos).is_some_and(|tile| {
                    !g.cities.values().any(|city| city.pos == *pos)
                        && g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                })
        })
        .unwrap();
    let other = g.found_city_for(0, pos, None);
    let city = g.cities.get_mut(&other).unwrap();
    city.buildings.push(crate::name!("walls"));
    city.wall_hp = 100;
    city.hp = 180;
    quote(&mut g, other, &unit("heavy_chariot"), 33.0, Some(4.0));
    quote(&mut g, other, &unit("spearman"), 33.0, Some(3.0));
    quote(&mut g, other, &unit("builder"), 35.0, Some(4.0));
    g.apply(
        0,
        &Action::Produce {
            city: other,
            item: unit("builder"),
        },
    )
    .unwrap();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_eq!(claim.as_ref().map(|(city, _)| *city), Some(other));
    assert_eq!(g.cities[&cid].queue.first(), Some(&unit("heavy_chariot")));
    assert!(
        matches!(g.cities[&other].queue.first(), Some(Item::Unit { unit }) if *unit != "builder")
    );
}

#[test]
fn spears_answer_the_horse_keeps_the_existing_one_turn_library_release() {
    let (mut g, mut ai, cid, knight) = fixture();
    g.remove_unit(knight);
    let center = g.cities[&cid].pos;
    let mut distant = None;
    for pos in g.wring(center, 6) {
        if !g
            .map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        {
            continue;
        }
        let uid = g.spawn_test_unit("knight", 1, pos);
        if g.route_distance(uid, center, 1)
            .is_some_and(|steps| steps <= 6)
            && !g.attack_reach(uid).contains(&center)
        {
            distant = Some(uid);
            break;
        }
        g.remove_unit(uid);
    }
    let distant = distant.expect("a reachable Knight outside its next-turn attack envelope");
    let observer = g
        .nbrs(g.units[&distant].pos)
        .into_iter()
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .unwrap();
    g.spawn_test_unit("scout", 0, observer);
    g.cities.get_mut(&cid).unwrap().hp = CITY_MAX_HP;
    g.players[0].techs.insert(crate::name!("writing"));
    crate::game::install_test_district(&mut g, cid, "campus");
    let library = Item::Building {
        building: crate::name!("library"),
    };
    quote(&mut g, cid, &library, 45.0, Some(1.0));
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: library.clone(),
        },
    )
    .unwrap();
    ai.victory_planning = true;
    ai.victory_target = Some(VictoryTarget::Domination);
    assert!(ai.campus_finishes_before_slow_defender(
        &g,
        0,
        cid,
        Some(&library),
        &unit("heavy_chariot")
    ));
    assert_eq!(
        ai.spear_for_local_horse(&g, 0, cid, &unit("heavy_chariot"), Some(&library)),
        Some(unit("spearman"))
    );
    assert_eq!(
        ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid)),
        Some((cid, library.clone()))
    );
    assert_eq!(g.cities[&cid].queue.first(), Some(&library));
}

#[test]
fn spears_answer_the_horse_requires_a_local_hostile_contact() {
    let (mut g, ai, cid, knight) = fixture();
    let selected = unit("heavy_chariot");
    g.at_war.clear();
    assert!(ai
        .spear_for_local_horse(&g, 0, cid, &selected, Some(&selected))
        .is_none());
    g.at_war.insert((0, 1));
    g.remove_unit(knight);
    let center = g.cities[&cid].pos;
    let far = g
        .wring(center, 7)
        .into_iter()
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .unwrap();
    g.spawn_test_unit("knight", 1, far);
    assert!(ai
        .spear_for_local_horse(&g, 0, cid, &selected, Some(&selected))
        .is_none());
}
