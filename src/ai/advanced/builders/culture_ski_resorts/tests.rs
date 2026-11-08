use super::*;
use std::sync::Arc;

fn fixture() -> (AdvancedAi, Game, u32, Pos) {
    let mut g = Game::new_full(2, 40, 24, 478_001, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.map.clear_rivers();
    for t in g.map.tiles.values_mut() {
        t.terrain = crate::name!("grassland");
        t.feature = None;
        t.resource = None;
        t.improvement = None;
        t.hills = false;
        t.owner_city = None;
    }
    let home = g.found_city_for(0, (8, 10), None);
    let owned: Vec<_> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|&p| g.wdist(p, (8, 10)) <= 7)
        .collect();
    for pos in owned {
        g.map.tiles.get_mut(&pos).unwrap().owner_city = Some(home);
    }
    g.players[0]
        .civics
        .insert(crate::name!("professional_sports"));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.observed_builders_avoid_mountains = true;
    let site = (9, 10);
    g.map.tiles.get_mut(&site).unwrap().terrain = crate::name!("mountain");
    Arc::make_mut(&mut g.observed_appeal).insert(site, 4);
    let uid = g.spawn_test_unit("builder", 0, (8, 10));
    (AdvancedAi::targeting(VictoryTarget::Culture), g, uid, site)
}

#[test]
fn adjacent_builder_targets_the_mountain_without_entering_it() {
    let (ai, g, uid, site) = fixture();
    assert!(!g.unit_can_traverse(uid, site));
    assert_eq!(
        ai.native_culture_ski_resort_jobs(&g, 0, &BTreeSet::new(), &BTreeSet::new()),
        vec![(uid, site, None)]
    );
}

#[test]
fn routes_to_adjacent_land_around_an_unwalkable_ridge() {
    let (ai, mut g, uid, site) = fixture();
    g.units.get_mut(&uid).unwrap().pos = (6, 10);
    g.map.tiles.get_mut(&(7, 10)).unwrap().terrain = crate::name!("mountain");
    Arc::make_mut(&mut g.observed_appeal).insert((7, 10), 0);
    for _ in 0..8 {
        let jobs = ai.native_culture_ski_resort_jobs(&g, 0, &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].1, site);
        let Some(step) = jobs[0].2 else {
            assert_eq!(g.wdist(g.units[&uid].pos, site), 1);
            return;
        };
        assert_ne!(g.map.tiles[&step].terrain, "mountain");
        assert!(g.can_move(uid, step));
        assert_eq!(g.wdist(g.units[&uid].pos, step), 1);
        g.units.get_mut(&uid).unwrap().pos = step;
    }
    panic!("route did not reach adjacent staging land");
}

#[test]
fn existing_jobs_unlocks_charges_and_other_victories_are_preserved() {
    let (ai, mut g, uid, _) = fixture();
    let empty = BTreeSet::new();
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &BTreeSet::from([uid]), &empty)
        .is_empty());
    for target in VictoryTarget::ALL
        .into_iter()
        .filter(|x| *x != VictoryTarget::Culture)
    {
        assert!(AdvancedAi::targeting(target)
            .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
            .is_empty());
    }
    g.players[0].civics.clear();
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
        .is_empty());
    g.players[0]
        .civics
        .insert(crate::name!("professional_sports"));
    g.units.get_mut(&uid).unwrap().charges = 0;
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
        .is_empty());
    g.units.get_mut(&uid).unwrap().charges = 3;
    g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
        .is_empty());
    g.units.get_mut(&uid).unwrap().moves_left = 2.0;
    g.victory_conditions.culture = false;
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
        .is_empty());
}

#[test]
fn reserves_ski_spacing_and_respects_parks_and_host_refusals() {
    let (ai, mut g, _, site) = fixture();
    let second = (9, 11);
    g.map.tiles.get_mut(&second).unwrap().terrain = crate::name!("mountain");
    Arc::make_mut(&mut g.observed_appeal).insert(second, 4);
    g.spawn_test_unit("builder", 0, (8, 11));
    let empty = BTreeSet::new();
    assert_eq!(
        ai.native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
            .len(),
        1
    );
    g.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("national_park"));
    Arc::make_mut(&mut g.blocked_improvements)
        .entry(second)
        .or_default()
        .insert(crate::name!("ski_resort"));
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
        .is_empty());
    g.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("ski_resort"));
    Arc::make_mut(&mut g.blocked_improvements).clear();
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &empty, &empty)
        .is_empty());
}

#[test]
fn danger_and_blocked_staging_do_not_spend_a_builder() {
    let (ai, mut g, uid, _) = fixture();
    g.units.get_mut(&uid).unwrap().pos = (6, 10);
    let blocked = g.nbrs((6, 10)).into_iter().collect();
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &BTreeSet::new(), &blocked)
        .is_empty());
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    g.spawn_test_unit("warrior", 1, (7, 10));
    assert!(ai
        .native_culture_ski_resort_jobs(&g, 0, &BTreeSet::new(), &BTreeSet::new())
        .is_empty());
}
