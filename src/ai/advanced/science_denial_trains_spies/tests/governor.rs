use super::*;

fn spy() -> Item {
    Item::Unit {
        unit: crate::name!("spy"),
    }
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

/// A cheap host-offered building wins ordinary rescoring. The denial
/// claim selects the only offered Spy in the strongest producer instead.
fn fixture() -> (Game, StrategicPlan, u32) {
    let (mut g, cities) = board();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.players[0].gold = 1_000.0;
    g.players[0].gold_per_turn = 100.0;
    let best = *top(&g, &cities, 1).iter().next().unwrap();
    let mut menus = BTreeMap::new();
    for cid in cities {
        let city = g.cities.get_mut(&cid).unwrap();
        // Rome starts with this building. The control needs a legal,
        // unfinished alternative, including after its current head changes.
        city.buildings.remove(&crate::name!("monument"));
        city.queue = vec![monument()];
        city.production = 0.0;
        city.production_progress.clear();
        let mut menu = BTreeMap::from([(
            Game::production_block_key(&monument()),
            crate::game::HostMenuEntry {
                cost: Some(1.0),
                turns: Some(1.0),
            },
        )]);
        if cid == best {
            menu.insert(
                Game::production_block_key(&spy()),
                crate::game::HostMenuEntry {
                    cost: Some(337.0),
                    turns: Some(7.0),
                },
            );
        }
        menus.insert(cid, menu);
    }
    g.replace_host_menus(menus, BTreeMap::new(), BTreeMap::new());
    for cid in g.player_city_ids(0) {
        assert!(g.can_produce(0, cid, &monument()));
    }
    armed().claim_queues_for_denial_spies(&mut g, 0, &plan());
    assert_eq!(training(&g), BTreeSet::from([best]));
    assert!(g.can_produce(0, best, &monument()));
    assert_eq!(g.item_invested_production(best, &spy()), 0.0);
    (g, plan(), best)
}

fn governor(target: VictoryTarget, enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.preempt_margin = 1.25;
    ai.base.book_pos = 4;
    if enabled {
        ai.enable_science_denial_trains_spies();
    } else {
        ai.disable_science_denial_trains_spies();
    }
    ai
}

#[test]
fn every_victory_governor_keeps_the_denial_spy_before_its_first_hammer() {
    for target in VictoryTarget::ALL {
        let (g, plan, city) = fixture();
        let mut off = g.clone();
        governor(target, false).advanced_production(&mut off, 0, &plan, false);
        assert_eq!(
            off.cities[&city].queue.first(),
            Some(&monument()),
            "{target:?}: the control must actually review this fresh queue"
        );
        let mut on = g;
        governor(target, true).advanced_production(&mut on, 0, &plan, false);
        assert_eq!(
            on.cities[&city].queue.first(),
            Some(&spy()),
            "{target:?}: the earlier denial claim keeps its selected Spy"
        );
        assert_eq!(on.cities[&city].production, 0.0);
    }
}

#[test]
fn delegated_governor_already_preserves_the_busy_spy_queue() {
    for target in VictoryTarget::ALL {
        let (mut g, plan, city) = fixture();
        governor(target, true).delegated_cities(&mut g, 0, &plan);
        assert_eq!(g.cities[&city].queue.first(), Some(&spy()), "{target:?}");
    }
}

#[test]
fn no_racer_releases_the_queue_to_ordinary_review() {
    let (mut g, plan, city) = fixture();
    g.players[1].science_projects.clear();
    g.players[1].techs.remove(&crate::name!("rocketry"));
    governor(VictoryTarget::Culture, true).advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn a_refused_spy_does_not_receive_a_reservation() {
    let (mut g, plan, city) = fixture();
    g.replace_blocked_production([(city, [Game::production_block_key(&spy())].into())].into());
    governor(VictoryTarget::Culture, true).advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn a_threatened_or_recently_attacked_city_releases_the_reservation() {
    for attacked in [false, true] {
        let (mut g, mut plan, city) = fixture();
        if attacked {
            g.cities.get_mut(&city).unwrap().last_attacked = g.turn;
        } else {
            plan.threatened_city = Some(city);
        }
        governor(VictoryTarget::Culture, true).advanced_production(&mut g, 0, &plan, false);
        assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
    }
}

#[test]
fn meeting_the_networks_demand_releases_the_queue() {
    let (mut g, plan, city) = fixture();
    for _ in 0..g.spy_capacity(0) {
        let id = g.next_id;
        g.next_id += 1;
        g.spies.insert(
            id,
            crate::game::Spy {
                id,
                owner: 0,
                level: 0,
                promotions: BTreeSet::new(),
                city: Some(city),
                ready_turn: g.turn,
                mission: None,
                sources_city: None,
                sources_until: 0,
                captured_by: None,
            },
        );
    }
    governor(VictoryTarget::Culture, true).advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn science_treasury_recovery_can_still_preempt_the_spy() {
    let (mut g, plan, city) = fixture();
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -20.0;
    governor(VictoryTarget::Science, true).advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn a_near_done_routine_head_keeps_its_later_handoff() {
    let (mut g, plan, city) = fixture();
    let mut ai = governor(VictoryTarget::Culture, true);
    ai.enable_near_done_work_finishes();
    let centre = g.cities.get_mut(&city).unwrap();
    centre.queue = vec![monument()];
    centre.production = 0.5;
    let heads = ai.near_done_heads(&g, 0);
    g.apply(0, &Action::Produce { city, item: spy() }).unwrap();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&spy()));
    ai.claim_near_done_work_finishes(&mut g, 0, &plan, &heads);
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn final_local_defense_can_replace_the_reserved_spy() {
    let (mut g, plan, city) = fixture();
    let mut ai = governor(VictoryTarget::Culture, true);
    ai.enable_garrison_under_fire();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&spy()));
    g.players[0].techs.insert(crate::name!("masonry"));
    g.cities.get_mut(&city).unwrap().hp = 100;
    g.cities.get_mut(&city).unwrap().last_attacked = g.turn;
    let wall = Item::Building {
        building: crate::name!("walls"),
    };
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .get_mut(&city)
        .unwrap()
        .insert(
            Game::production_block_key(&wall),
            crate::game::HostMenuEntry {
                cost: Some(40.0),
                turns: Some(1.0),
            },
        );
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(city));
    assert_eq!(g.cities[&city].queue.first(), Some(&wall));
}
