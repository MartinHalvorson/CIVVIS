use super::*;
use crate::game::Item;

fn fixture() -> (Snapshot, StateSnapshot) {
    let plots = (0..12)
        .flat_map(|x| {
            (0..12).map(move |y| {
                let mut tile = plot(x, y, "TERRAIN_GRASS");
                tile.o = 0;
                tile.oc = Some(7);
                tile
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 79,
        width: 12,
        height: 12,
        chunk: 1,
        plots,
    }]);
    let state = state_from_json(
        r#"{
        "turn":79,"techs":["TECH_WRITING"],"cities":[{
            "id":7,"name":"Bogotá","x":5,"y":5,"pop":5,
            "producing":"UNIT_SPEARMAN","production_progress":16,
            "districts":[{"type":"DISTRICT_CAMPUS","x":6,"y":5,"complete":false}],
            "buildable":[{"t":"UNIT_SPEARMAN","c":65,"p":3},
                         {"t":"DISTRICT_CAMPUS","c":73,"p":4,"pr":40}]
        }]
    }"#,
    )
    .unwrap();
    (snapshot, state)
}

fn campus() -> Item {
    Item::District {
        district: crate::name!("campus"),
        pos: crate::hex::offset_to_axial(6, 5),
    }
}

fn build(snapshot: &Snapshot, state: &StateSnapshot) -> LiveMirror {
    LiveMirror::new(snapshot, state, 4, 1, 500, 0)
}

#[test]
fn paused_native_district_work_survives_fresh_reconstruction() {
    let (snapshot, state) = fixture();
    assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
    for _ in 0..2 {
        let mirror = build(&snapshot, &state);
        let cid = mirror.cid_of[&7];
        assert_eq!(mirror.game.cities[&cid].production, 16.0);
        assert_eq!(mirror.game.item_invested_production(cid, &campus()), 40.0);
        assert_eq!(
            mirror.game.item_remaining_cost_for_city(0, cid, &campus()),
            33.0
        );
        assert!(mirror.game.producible_items(0, cid).contains(&campus()));
        let elsewhere = Item::District {
            district: crate::name!("campus"),
            pos: (1, 1),
        };
        assert_eq!(mirror.game.item_invested_production(cid, &elsewhere), 0.0);
        assert_eq!(
            mirror.game.item_invested_production(
                cid,
                &Item::Building {
                    building: crate::name!("monument"),
                }
            ),
            0.0
        );
    }
}

#[test]
fn active_head_is_not_counted_twice_and_refresh_can_pause_it() {
    let (snapshot, mut state) = fixture();
    state.cities[0].producing = Some("DISTRICT_CAMPUS".into());
    state.cities[0].production_progress = 40.0;
    let mut mirror = build(&snapshot, &state);
    let cid = mirror.cid_of[&7];
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 40.0);
    assert!(mirror.game.cities[&cid].production_progress.is_empty());
    state.cities[0].producing = Some("UNIT_SPEARMAN".into());
    state.cities[0].production_progress = 16.0;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 40.0);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 40.0);
    state.cities[0].producing = Some("DISTRICT_CAMPUS".into());
    state.cities[0].production_progress = 50.0;
    state.cities[0].buildable.as_mut().unwrap()[1].pr = Some(50.0);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 50.0);
    assert!(mirror.game.cities[&cid].production_progress.is_empty());
}

#[test]
fn zero_and_completion_erase_saved_work_but_missing_is_unknown() {
    let (snapshot, mut state) = fixture();
    let mut mirror = build(&snapshot, &state);
    let cid = mirror.cid_of[&7];
    state.cities[0].buildable.as_mut().unwrap()[1].pr = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 40.0);
    assert_eq!(
        build(&snapshot, &state)
            .game
            .item_invested_production(cid, &campus()),
        0.0
    );
    state.cities[0].buildable.as_mut().unwrap()[1].pr = Some(0.0);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 0.0);
    state.cities[0].buildable.as_mut().unwrap()[1].pr = Some(40.0);
    mirror.sync(&snapshot, &state, 0);
    state.cities[0].districts[0].complete = true;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 0.0);
    assert!(mirror.game.cities[&cid]
        .districts
        .contains_key(crate::name!("campus")));
}

#[test]
fn legacy_and_invalid_menu_readings_do_not_invent_investment() {
    for progress in [None, Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        let (snapshot, mut state) = fixture();
        state.cities[0].buildable.as_mut().unwrap()[1].pr = progress;
        let mirror = build(&snapshot, &state);
        assert_eq!(
            mirror
                .game
                .item_invested_production(mirror.cid_of[&7], &campus()),
            0.0
        );
    }
}

#[test]
fn queue_tail_can_supply_exact_district_work_without_a_menu_reading() {
    let (snapshot, mut state) = fixture();
    state.cities[0].buildable.as_mut().unwrap()[1].pr = None;
    state.cities[0].queue = Some(vec![StateQueueItem {
        t: "DISTRICT_CAMPUS".into(),
        pr: Some(40.0),
        ..StateQueueItem::default()
    }]);
    let mirror = build(&snapshot, &state);
    let cid = mirror.cid_of[&7];
    assert_eq!(mirror.game.cities[&cid].queue.last(), Some(&campus()));
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 40.0);
}

#[test]
fn an_offer_is_not_a_foundation_and_duplicate_district_identity_is_unknown() {
    let (snapshot, mut state) = fixture();
    state.cities[0].districts.clear();
    state.cities[0].buildable.as_mut().unwrap()[1].s = Some(vec![StateMenuPlot { x: 6, y: 5 }]);
    state.cities[0].buildable.as_mut().unwrap()[1].n = Some(1);
    let mirror = build(&snapshot, &state);
    assert!(mirror.game.cities[&mirror.cid_of[&7]]
        .production_progress
        .is_empty());
    let (snapshot, mut state) = fixture();
    let mut other = state.cities[0].districts[0].clone();
    other.x = 7;
    state.cities[0].districts.push(other);
    let mirror = build(&snapshot, &state);
    assert!(mirror.game.cities[&mirror.cid_of[&7]]
        .production_progress
        .is_empty());
}

#[test]
fn saved_work_requires_the_exact_city_owned_foundation() {
    let (snapshot, state) = fixture();
    let mut mirror = build(&snapshot, &state);
    let cid = mirror.cid_of[&7];
    for wrong_city in [None, Some(cid + 1)] {
        mirror
            .game
            .cities
            .get_mut(&cid)
            .unwrap()
            .production_progress
            .clear();
        mirror
            .game
            .map
            .tiles
            .get_mut(&crate::hex::offset_to_axial(6, 5))
            .unwrap()
            .owner_city = wrong_city;
        apply_host_district_progress(&mut mirror.game, &state.cities, &BTreeMap::from([(cid, 7)]));
        assert_eq!(mirror.game.item_invested_production(cid, &campus()), 0.0);
    }
    let tile = mirror
        .game
        .map
        .tiles
        .get_mut(&crate::hex::offset_to_axial(6, 5))
        .unwrap();
    tile.owner_city = Some(cid);
    tile.district_foundation = None;
    apply_host_district_progress(&mut mirror.game, &state.cities, &BTreeMap::from([(cid, 7)]));
    assert_eq!(mirror.game.item_invested_production(cid, &campus()), 0.0);
}
