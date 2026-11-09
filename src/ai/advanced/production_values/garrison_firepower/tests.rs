use super::super::super::*;

fn fixture() -> (Game, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 478_701, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let cid = g.found_city_for(0, (8, 10), None);
    g.current = 0;
    g.turn = 76;
    g.at_war.insert((0, 1));
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    for tech in ["wheel", "masonry", "machinery"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let city = g.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.buildings.push(crate::name!("walls"));
    city.wall_hp = 100;
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 38.0);
    let guard = g.spawn_test_unit("heavy_chariot", 0, (8, 10));
    let enemy = g.spawn_test_unit("pike_and_shot", 1, (9, 10));
    let quotes = [("heavy_chariot", 1.0), ("crossbowman", 3.0)]
        .into_iter()
        .map(|(unit, turns)| {
            (
                Game::production_block_key(&Item::Unit {
                    unit: Name::new(unit),
                }),
                crate::game::HostMenuEntry {
                    cost: None,
                    turns: Some(turns),
                },
            )
        })
        .collect();
    std::sync::Arc::make_mut(&mut g.host_buildable).insert(cid, quotes);
    assert!(g.can_produce(
        0,
        cid,
        &Item::Unit {
            unit: crate::name!("crossbowman")
        }
    ));
    assert!(AdvancedAi::imminent_city_attack(
        &g,
        0,
        cid,
        &g.player_vision_frame(0)
    ));
    (g, cid, guard, enemy)
}

fn answer(g: &mut Game, cid: u32, target: VictoryTarget) -> Option<Item> {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_siege_preempts_the_queue();
    ai.redirect_unsafe_city_queue_for_defense(g, 0, Some(cid));
    g.cities[&cid].queue.first().cloned()
}

fn chariot() -> Option<Item> {
    Some(Item::Unit {
        unit: crate::name!("heavy_chariot"),
    })
}

#[test]
fn garrison_adds_firepower_for_every_victory_target() {
    for target in VictoryTarget::ALL {
        let (mut g, cid, _, _) = fixture();
        assert_eq!(
            answer(&mut g, cid, target),
            Some(Item::Unit {
                unit: crate::name!("crossbowman")
            }),
            "target {target:?}: a healthy walled garrison needs the stronger quoted firing unit"
        );
    }
}

#[test]
fn empty_city_keeps_its_melee_rescue() {
    let (mut g, cid, guard, _) = fixture();
    g.remove_unit(guard);
    assert_eq!(answer(&mut g, cid, VictoryTarget::Culture), chariot());
}

#[test]
fn injured_garrison_keeps_its_melee_rescue() {
    let (mut g, cid, guard, _) = fixture();
    g.units.get_mut(&guard).unwrap().hp = 20;
    assert_eq!(answer(&mut g, cid, VictoryTarget::Culture), chariot());
}

#[test]
fn missing_or_slow_native_completion_keeps_the_old_defender() {
    for turns in [None, Some(4.0)] {
        let (mut g, cid, _, _) = fixture();
        let key = Game::production_block_key(&Item::Unit {
            unit: crate::name!("crossbowman"),
        });
        std::sync::Arc::make_mut(&mut g.host_buildable)
            .get_mut(&cid)
            .unwrap()
            .get_mut(&key)
            .unwrap()
            .turns = turns;
        assert_eq!(answer(&mut g, cid, VictoryTarget::Culture), chariot());
    }
}

#[test]
fn weaker_attack_keeps_the_old_defender() {
    let (mut g, cid, _, enemy) = fixture();
    g.units.get_mut(&enemy).unwrap().kind = crate::name!("horseman");
    assert!(!AdvancedAi::imminent_city_attack(
        &g,
        0,
        cid,
        &g.player_vision_frame(0)
    ));
    assert_eq!(answer(&mut g, cid, VictoryTarget::Culture), chariot());
}

#[test]
fn unwalled_city_keeps_the_old_defender() {
    let (mut g, cid, _, _) = fixture();
    g.cities.get_mut(&cid).unwrap().buildings.clear();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    assert_eq!(answer(&mut g, cid, VictoryTarget::Culture), chariot());
}
