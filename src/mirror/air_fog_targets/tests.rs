use crate::game::Action;
use crate::hex::offset_to_axial;
use crate::mirror::{LiveMirror, Plot, Snapshot, StateSnapshot, TilesChunk};

// Minimized from civvis-20261015T090922Z turn170/frame0: Bomber11927570
// at36,25 repeatedly pillages Spaceport46,24, which the native export leaves
// in fog. A Spy nearby gives the reconstructed board additional sight.
fn fixture(shown: bool, modern: bool) -> (Snapshot, StateSnapshot) {
    let state = crate::mirror::state_from_json(
        r#"{
            "turn":170,
            "cities":[{"id":1,"name":"Base","x":36,"y":25,"pop":8,"capital":true}],
            "units":[
                {"id":11927570,"kind":"UNIT_BOMBER","x":36,"y":25,"hp":100,"moves":11,"max_moves":11,"attacks_remaining":1},
                {"id":78,"kind":"UNIT_SPY","x":45,"y":26,"hp":100,"moves":0}
            ],
            "rivals":[{"player":2,"civ":"CIVILIZATION_BABYLON_STK","at_war":true,
                "cities":[{"id":2,"name":"Enemy","x":45,"y":26,"pop":8,"capital":true}]}]
        }"#,
    )
    .unwrap();
    let mut plots = Vec::new();
    for y in 0..38 {
        for x in 0..52 {
            let mut plot: Plot = serde_json::from_value(serde_json::json!({
                "x":x,"y":y,"t":"TERRAIN_GRASS","o":if x >= 40 {2} else {0}
            }))
            .unwrap();
            plot.vis = modern && ((x == 36 && y == 25) || (x == 45 && y == 25));
            if (x, y) == (46, 24) {
                plot.d = Some("DISTRICT_SPACEPORT".into());
                plot.dc = Some(true);
                plot.vis = modern && shown;
            }
            if (x, y) == (45, 25) {
                plot.d = Some("DISTRICT_CAMPUS".into());
                plot.dc = Some(true);
            }
            plots.push(plot);
        }
    }
    (
        Snapshot::from_chunks(&[TilesChunk {
            turn: 170,
            width: 52,
            height: 38,
            chunk: 1,
            plots,
        }]),
        state,
    )
}

#[test]
fn native_fog_overrides_a_spys_reconstructed_air_pillage_sight() {
    let (snapshot, state) = fixture(false, true);
    let mut live = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let bomber = live.uid_of[&11927570];
    let target = offset_to_axial(46, 24);
    assert!(
        live.game.player_can_see(0, target),
        "the fixture exercises excess derived sight"
    );
    assert!(!live.game.host_observed.contains(&target));
    let denied = Action::AirPillage {
        unit: bomber,
        target,
    };
    assert!(!live
        .game
        .legal_doctrine_actions(0, bomber)
        .contains(&denied));
    assert!(!live.game.legal_actions(0).contains(&denied));
    let before = live.game.units[&bomber].attacks_left;
    assert!(live.game.apply(0, &denied).is_err());
    assert_eq!(live.game.units[&bomber].attacks_left, before);
    let alternative = Action::AirPillage {
        unit: bomber,
        target: offset_to_axial(45, 25),
    };
    assert!(live
        .game
        .legal_doctrine_actions(0, bomber)
        .contains(&alternative));
    live.game.apply(0, &alternative).unwrap();
}

#[test]
fn native_sight_refresh_reopens_a_previously_fogged_air_target() {
    let (fog, state) = fixture(false, true);
    let mut live = LiveMirror::new(&fog, &state, 2, 1, 650, 0);
    let bomber = live.uid_of[&11927570];
    let target = offset_to_axial(46, 24);
    assert!(live.game.blocked_strikes.contains(&(bomber, target)));
    let (shown, mut later) = fixture(true, true);
    later.turn += 1;
    live.sync(&shown, &later, 0);
    let bomber = live.uid_of[&11927570];
    assert!(!live.game.blocked_strikes.contains(&(bomber, target)));
    assert!(live.game.legal_actions(0).contains(&Action::AirPillage {
        unit: bomber,
        target
    }));
}

#[test]
fn an_export_without_native_sight_keeps_its_legacy_air_actions() {
    let (snapshot, state) = fixture(false, false);
    let live = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let bomber = live.uid_of[&11927570];
    let target = offset_to_axial(46, 24);
    assert!(live.game.legal_actions(0).contains(&Action::AirPillage {
        unit: bomber,
        target
    }));
}
