use crate::game::{Action, Game};
use crate::hex::offset_to_axial;
use crate::mirror::{LiveMirror, Plot, Snapshot, StateSnapshot, TilesChunk};
use crate::Pos;

/// Minimized from live 20261110T164254Z, t158-t164. Our band stands at the
/// edge of Ethiopia (host player 2 here), which holds two wonders and slotted
/// Music Censorship at t154; the host offers it one stage, a wonder in Rome's
/// land (host player 3) beyond Ethiopia. Rows 30 and below belong to nobody,
/// so a walk around Ethiopia exists.
fn fixture(offered: &str, second_wonder: bool) -> (Snapshot, StateSnapshot) {
    let state = crate::mirror::state_from_json(&format!(
        r#"{{
            "turn":158,
            "cities":[{{"id":1,"name":"Stockholm","x":4,"y":15,"pop":8,"capital":true}}],
            "units":[
                {{"id":5701638,"kind":"UNIT_ROCK_BAND","x":9,"y":15,"hp":100,"moves":4,
                  "max_moves":4,"promotions":["PROMOTION_ALBUM_COVER_ART"],
                  "rock_band_level":1,"concert_plots":{offered}}}
            ],
            "rivals":[
                {{"player":2,"civ":"CIVILIZATION_ETHIOPIA",
                  "cities":[{{"id":2,"name":"Gondar","x":18,"y":15,"pop":10,"capital":true}}]}},
                {{"player":3,"civ":"CIVILIZATION_ROME",
                  "cities":[{{"id":3,"name":"Roma","x":38,"y":15,"pop":10,"capital":true}}]}}
            ]
        }}"#
    ))
    .unwrap();
    let mut plots = Vec::new();
    for y in 0..38 {
        for x in 0..52 {
            let owner = if x < 10 {
                0
            } else if y < 30 && x < 28 {
                2
            } else if y < 30 && x >= 30 {
                3
            } else {
                -1
            };
            let mut plot: Plot = serde_json::from_value(serde_json::json!({
                "x":x,"y":y,"t":"TERRAIN_GRASS","o":owner
            }))
            .unwrap();
            if matches!((x, y), (14, 15) | (32, 15)) || (second_wonder && (x, y) == (22, 15)) {
                plot.d = Some("DISTRICT_WONDER".into());
                plot.dc = Some(true);
                plot.wo = Some(
                    match x {
                        14 => "BUILDING_PYRAMIDS",
                        22 => "BUILDING_RUHR_VALLEY",
                        _ => "BUILDING_COLOSSEUM",
                    }
                    .into(),
                );
            }
            plots.push(plot);
        }
    }
    (
        Snapshot::from_chunks(&[TilesChunk {
            turn: 158,
            width: 52,
            height: 38,
            chunk: 1,
            plots,
        }]),
        state,
    )
}

fn seat_of_city(game: &Game, x: i32, y: i32) -> usize {
    game.cities
        .values()
        .find(|city| city.pos == offset_to_axial(x, y))
        .map(|city| city.owner)
        .unwrap()
}

fn owner_of(game: &Game, pos: Pos) -> Option<usize> {
    game.map
        .get(pos)
        .and_then(|tile| tile.owner_city)
        .and_then(|city| game.cities.get(&city))
        .map(|city| city.owner)
}

/// Walk the band along its planned route, one step a turn, until it stands
/// on `venue` or the planner offers no step. Returns every tile entered.
fn walk(game: &mut Game, band: u32, venue: Pos) -> Vec<Pos> {
    let mut walked = Vec::new();
    for _ in 0..120 {
        if game.units[&band].pos == venue {
            break;
        }
        let Some(next) = game.route_step(band, venue, 0) else {
            break;
        };
        let full = game.unit_max_moves(band);
        game.units.get_mut(&band).unwrap().moves_left = full;
        if game
            .apply(
                0,
                &Action::Move {
                    unit: band,
                    to: next,
                },
            )
            .is_err()
        {
            break;
        }
        walked.push(game.units[&band].pos);
    }
    walked
}

#[test]
fn a_rival_with_wonders_the_host_never_offers_is_closed_to_our_bands() {
    let (snapshot, state) = fixture(r#"[{"x":32,"y":15}]"#, true);
    let live = LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
    let ethiopia = seat_of_city(&live.game, 18, 15);
    let rome = seat_of_city(&live.game, 38, 15);
    assert_eq!(
        live.game.rock_band_barred_hosts,
        [(0, ethiopia)].into_iter().collect()
    );
    assert!(live.game.bars_foreign_rock_bands(ethiopia, 0));
    assert!(!live.game.bars_foreign_rock_bands(rome, 0));
}

#[test]
fn the_band_walks_around_the_closed_rival_to_the_stage_it_was_offered() {
    let (snapshot, state) = fixture(r#"[{"x":32,"y":15}]"#, true);
    let mut live = LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
    let band = live.uid_of[&5701638];
    let ethiopia = seat_of_city(&live.game, 18, 15);
    let venue = offset_to_axial(32, 15);

    // The stall: without the host's word, the planner walks straight into
    // Ethiopia, where the live host refused every step.
    let mut blind = live.game.clone();
    blind.rock_band_barred_hosts.clear();
    let blind_walk = walk(&mut blind, band, venue);
    assert!(blind_walk
        .iter()
        .any(|pos| owner_of(&blind, *pos) == Some(ethiopia)));

    let walked = walk(&mut live.game, band, venue);
    assert_eq!(walked.last(), Some(&venue), "{walked:?}");
    assert!(
        walked
            .iter()
            .all(|pos| owner_of(&live.game, *pos) != Some(ethiopia)),
        "{walked:?}"
    );
}

#[test]
fn no_offer_at_all_infers_nothing() {
    // A band the host has not named yet, or a turn every rival is closed:
    // an empty highlight list is no evidence about any one rival.
    for offered in ["[]", "null"] {
        let (snapshot, state) = fixture(offered, true);
        let live = LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
        assert!(live.game.rock_band_barred_hosts.is_empty(), "{offered}");
    }
}

#[test]
fn a_rival_with_a_single_wonder_plot_is_not_judged() {
    // `Plot:GetWonderType` names a wonder from the turn it is placed, so one
    // unoffered wonder plot may be a building site, not a stage.
    let (snapshot, state) = fixture(r#"[{"x":32,"y":15}]"#, false);
    let live = LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
    assert!(live.game.rock_band_barred_hosts.is_empty());
}
