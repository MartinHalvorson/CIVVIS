//! The world civvis.ai plays, timed frame by frame on a native build: the stock opening world, the deployment genome, and the state document a page reads after every round.
//!
//! `civvis simulate` is what `tools/speed_ab.py` and `tools/profile_civvis.py`
//! have always measured, and it is not what a visitor's browser runs. It seats
//! the stock `AdvancedAi::new()` at every chair including the city-states,
//! where the site seats the deployment genome at the majors and `BasicAi` at
//! the minors, and it never writes a state document — which on the site is a
//! fifth of a whole game and most of every frame in the first twenty-five
//! turns. A change that only helps or only hurts the site was invisible to
//! both tools.
//!
//! This is the same frame the browser build's `/state?have=` route serves at
//! Lightning pace (`wasm.rs`, `advance_one_frame`): play seats until the round
//! is over, then build the document, the reasoning delta and the tile patch
//! and serialise them. It calls the same `Session` methods in the same order.
//! It is native, so it prices the engine and not WebAssembly; the published
//! module itself is driven by `tools/site_wasm_frames.mjs`.
//!
//! The digests are what make a timing mean something. Two builds that print
//! the same checkpoints played the same game and wrote the same documents, so
//! a time difference between them is overhead and nothing else.

use std::time::Instant;

use serde_json::Value;

use super::{spectator_step_completes_frame, stock_opening_params, tile_mark, Session};

/// Where a game's cost is reported. The bands are narrow early because the
/// opening is what a first visit sees, and wide late because turn cost has
/// stopped changing shape by then.
const BANDS: [(u32, u32); 8] = [
    (1, 10),
    (11, 25),
    (26, 50),
    (51, 75),
    (76, 100),
    (101, 150),
    (151, 200),
    (201, u32::MAX),
];

/// How often the game's state is fingerprinted, in turns.
const CHECKPOINT_EVERY: u32 = 25;

/// One band of turns, summed over the frames that started in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Band {
    pub first: u32,
    pub last: u32,
    pub frames: u32,
    /// Seconds the major civilizations' agents spent taking their turns,
    /// including the rules work their actions triggered.
    pub majors_seconds: f64,
    /// The same for city-states and the barbarian seat.
    pub minors_seconds: f64,
    /// Seconds spent building and serialising the page's documents.
    pub documents_seconds: f64,
    pub document_bytes: u64,
}

/// The game's state after a turn, as two fingerprints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub turn: u32,
    /// The save envelope: everything the rules know.
    pub save: u64,
    /// The page's document, without the fields that name the build or the
    /// process and without the reasoning journal, whose line order follows a
    /// `HashMap` and so differs between two runs of one binary.
    pub document: u64,
}

pub struct Report {
    pub bands: Vec<Band>,
    pub checkpoints: Vec<Checkpoint>,
    pub session: Session,
}

/// FNV-1a, written out so a digest means the same thing on every toolchain.
/// `DefaultHasher` promises no such thing between Rust releases, and a
/// baseline and a candidate are sometimes built by different ones.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn checkpoint(session: &Session) -> Checkpoint {
    let save = serde_json::to_vec(&crate::routes::save(session)).unwrap_or_default();
    let mut document = session.state();
    if let Value::Object(fields) = &mut document {
        fields.retain(|key, _| !key.starts_with("server_") && key != "ai_reasoning");
    }
    let document = serde_json::to_vec(&document).unwrap_or_default();
    Checkpoint {
        turn: session.game.turn,
        save: fnv1a(&save),
        document: fnv1a(&document),
    }
}

/// The document one frame sends: the state, what the agents thought since the
/// page last asked, and the tiles that changed since the frame it holds.
fn frame_document(session: &Session, marks: &mut Option<Vec<u64>>, cursor: &mut u64) -> String {
    let mut document = session.state();
    let thoughts = session.reasoning_json(*cursor);
    if let Some(next) = thoughts["cursor"].as_u64() {
        *cursor = next;
    }
    document["ai_reasoning"] = thoughts;
    if let Some(Value::Object(map)) = document.get_mut("map") {
        if let Some(Value::Array(tiles)) = map.remove("tiles") {
            let now: Vec<u64> = tiles.iter().map(tile_mark).collect();
            match marks.as_ref().filter(|prior| prior.len() == now.len()) {
                Some(prior) => {
                    let changed = tiles
                        .iter()
                        .enumerate()
                        .filter(|(at, _)| prior[*at] != now[*at])
                        .map(|(at, tile)| serde_json::json!([at, tile]))
                        .collect::<Vec<_>>();
                    map.insert("tiles_changed".to_string(), Value::Array(changed));
                }
                None => {
                    map.insert("tiles".to_string(), Value::Array(tiles));
                }
            }
            *marks = Some(now);
        }
    }
    serde_json::to_string(&document).unwrap_or_default()
}

/// Play the stock opening world for `turns` turns, a frame at a time.
///
/// `documents: false` plays the same game and skips the page's half of every
/// frame, which is what an unwatched exhibition costs.
pub fn run(seed: u64, turns: u32, documents: bool) -> Report {
    let mut session = Session::new(stock_opening_params(seed));
    let mut bands: Vec<Band> = BANDS
        .iter()
        .map(|(first, last)| Band {
            first: *first,
            last: *last,
            ..Band::default()
        })
        .collect();
    let mut checkpoints = Vec::new();
    let mut marks = None;
    let mut cursor = 0;
    if documents {
        // The page's opening read, which carries the whole map. It is the
        // baseline every later tile patch is taken against.
        frame_document(&session, &mut marks, &mut cursor);
    }
    while session.game.turn <= turns && !session.game.is_finished() {
        let turn = session.game.turn;
        let band = bands
            .iter_mut()
            .find(|band| band.first <= turn && turn <= band.last)
            .expect("the last band is open-ended");
        // One frame at Lightning: seats play until the round is over.
        loop {
            let turn_before = session.game.turn;
            let finished_before = session.game.is_finished();
            let seat = &session.game.players[session.game.current];
            let minor = seat.is_minor || seat.is_barbarian;
            let started = Instant::now();
            session.step();
            let spent = started.elapsed().as_secs_f64();
            if minor {
                band.minors_seconds += spent;
            } else {
                band.majors_seconds += spent;
            }
            if spectator_step_completes_frame(0, turn_before, finished_before, &session.game) {
                break;
            }
        }
        band.frames += 1;
        if documents {
            let started = Instant::now();
            let document = frame_document(&session, &mut marks, &mut cursor);
            band.documents_seconds += started.elapsed().as_secs_f64();
            band.document_bytes += document.len() as u64;
        }
        if turn % CHECKPOINT_EVERY == 0 || session.game.is_finished() {
            checkpoints.push(checkpoint(&session));
        }
    }
    if checkpoints.last().map(|last| last.turn) != Some(session.game.turn) {
        checkpoints.push(checkpoint(&session));
    }
    bands.retain(|band| band.frames > 0);
    Report {
        bands,
        checkpoints,
        session,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bench is only an instrument if it plays the site's world: four
    /// majors on the deployment genome, the stock roster of city-states, and
    /// one frame per round.
    #[test]
    fn the_bench_plays_the_stock_opening_world_a_round_per_frame() {
        let report = run(7, 3, true);
        let game = &report.session.game;
        let stock = stock_opening_params(7);
        assert_eq!(report.session.params.seed, 7);
        assert_eq!(report.session.params.num_players, stock.num_players);
        assert_eq!(report.session.params.num_city_states, stock.num_city_states);
        assert_eq!(game.turn, 4, "three rounds were played");
        let frames: u32 = report.bands.iter().map(|band| band.frames).sum();
        assert_eq!(frames, 3);
        assert!(report.bands.iter().all(|band| band.document_bytes > 0));
    }

    /// Two runs of one seed are one game. If this ever fails, a digest that
    /// differs between two builds stops meaning that they played differently.
    #[test]
    fn two_runs_of_one_seed_print_the_same_checkpoints() {
        let first = run(11, 6, true);
        let again = run(11, 6, true);
        assert!(!first.checkpoints.is_empty());
        assert_eq!(first.checkpoints, again.checkpoints);
    }

    /// Writing documents is a read. A game played with the page's half of the
    /// frame must be the game played without it.
    #[test]
    fn writing_the_documents_does_not_change_the_game() {
        let watched = run(13, 6, true);
        let unwatched = run(13, 6, false);
        let saves = |report: &Report| {
            report
                .checkpoints
                .iter()
                .map(|checkpoint| (checkpoint.turn, checkpoint.save))
                .collect::<Vec<_>>()
        };
        assert_eq!(saves(&watched), saves(&unwatched));
        assert!(unwatched.bands.iter().all(|band| band.document_bytes == 0));
    }
}
