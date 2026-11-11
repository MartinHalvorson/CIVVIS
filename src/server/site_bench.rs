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
use crate::ai::SiteShortcuts;

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
    /// Seconds each seat's agent spent taking its turns, by player id. What
    /// a shortcut saves the seats that take it shows here, beside the seats
    /// in the same game that do not.
    pub seat_seconds: Vec<f64>,
    pub session: Session,
}

/// What a run is asked to do beyond playing the frames.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Build the page's document after every round.
    pub documents: bool,
    /// `Game::keep_rival_yields` for each seat's turn, as every session does.
    /// Off prices it: the game is the same game, derived the slow way.
    pub keeps_rival_yields: bool,
    /// `Game::audit_rival_yields`: derive every reused yield again and panic
    /// on a difference. Slower than keeping nothing.
    pub audits_rival_yields: bool,
    /// `Game::keep_own_yields`: the acting seat's own cities as well, as
    /// every session does. Off with the rivals' on prices the two apart.
    pub keeps_own_yields: bool,
    /// [`SiteShortcuts`] for the seats in `shortcut_seats`. Unlike everything
    /// above, these change what a seat decides.
    pub shortcuts: SiteShortcuts,
    /// Which seats take `shortcuts`, as a bit per player id. `None` is every
    /// seat, city-states and barbarians included.
    pub shortcut_seats: Option<u64>,
    /// Take every shortcut away from every seat first, the ones a session
    /// seats its city-states and barbarians with included: the game the site
    /// played before it took any. `shortcuts` is applied after this.
    pub plain_seats: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            documents: true,
            keeps_rival_yields: true,
            audits_rival_yields: false,
            keeps_own_yields: true,
            shortcuts: SiteShortcuts::default(),
            shortcut_seats: None,
            plain_seats: false,
        }
    }
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
    run_with(
        seed,
        turns,
        Options {
            documents,
            ..Options::default()
        },
    )
}

/// [`run`], with every switch.
pub fn run_with(seed: u64, turns: u32, options: Options) -> Report {
    let documents = options.documents;
    let mut session = Session::new(stock_opening_params(seed));
    session.keeps_rival_yields = options.keeps_rival_yields;
    session.keeps_own_yields = options.keeps_own_yields;
    if options.plain_seats {
        for seat in 0..session.game.players.len() {
            session.set_seat_shortcuts(seat, SiteShortcuts::default());
        }
    }
    if options.shortcuts != SiteShortcuts::default() {
        for seat in 0..session.game.players.len() {
            let taken = options
                .shortcut_seats
                .is_none_or(|seats| seat < 64 && seats & (1 << seat) != 0);
            if taken {
                session.set_seat_shortcuts(seat, options.shortcuts);
            }
        }
    }
    let mut seat_seconds = vec![0.0; session.game.players.len()];
    session.game.audit_rival_yields(options.audits_rival_yields);
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
            let acting = session.game.current;
            let seat = &session.game.players[acting];
            let minor = seat.is_minor || seat.is_barbarian;
            let started = Instant::now();
            session.step_quietly();
            let spent = started.elapsed().as_secs_f64();
            seat_seconds[acting] += spent;
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
        if turn.is_multiple_of(CHECKPOINT_EVERY) || session.game.is_finished() {
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
        seat_seconds,
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

    /// How far the yield tests play. Long enough for second cities, the first
    /// wars and a policy deck or two, which is where the passes that sweep
    /// every rival's cities start to run.
    const YIELD_TURNS: u32 = 45;

    /// Keeping yields between passes is an optimization, so the game it plays
    /// and the documents it writes must be the ones the slow way gives. This
    /// is the test that sees what the audit cannot: a caller that depends on
    /// a derivation having *run* (the policy scorer's read trace did, and
    /// the first build of this kept the acting seat's cities straight
    /// through it; every card read as inert and the saves parted by turn 26).
    #[test]
    fn keeping_yields_plays_the_same_game() {
        let derived = run_with(
            17,
            YIELD_TURNS,
            Options {
                keeps_rival_yields: false,
                ..Options::default()
            },
        );
        assert_eq!(derived.session.game.rival_yields_reused(), (0, 0));
        for keeps_own_yields in [true, false] {
            let kept = run_with(
                17,
                YIELD_TURNS,
                Options {
                    keeps_own_yields,
                    ..Options::default()
                },
            );
            assert_eq!(
                kept.checkpoints, derived.checkpoints,
                "own cities kept: {keeps_own_yields}"
            );
            let (reused, _) = kept.session.game.rival_yields_reused();
            assert!(reused > 0, "the run this compares must actually reuse");
        }
    }

    /// A shortcut is taken by the seats it is given to and by nobody else,
    /// and naming none leaves the game alone. This is the plumbing the paired
    /// strength test stands on: if "treated" seats were not the treated ones
    /// its verdict would be about nothing.
    #[test]
    fn a_shortcut_is_taken_only_by_the_seats_it_is_given_to() {
        let plain = run_with(23, 30, Options::default());
        // A session's own seating: the small seats take what the site gives
        // them, the majors nothing, and `plain_seats` takes it all away.
        let stripped = run_with(
            23,
            2,
            Options {
                plain_seats: true,
                ..Options::default()
            },
        );
        for (seat, player) in plain.session.game.players.iter().enumerate() {
            let seated = if player.is_minor || player.is_barbarian {
                crate::server::SMALL_SEAT_SHORTCUTS
            } else {
                SiteShortcuts::default()
            };
            assert_eq!(plain.session.seat_shortcuts(seat), seated, "seat {seat}");
            assert_eq!(
                stripped.session.seat_shortcuts(seat),
                SiteShortcuts::default(),
                "seat {seat}"
            );
        }
        let named_but_nobody = run_with(
            23,
            30,
            Options {
                shortcuts: SiteShortcuts::named("envelopes-across-own-moves").unwrap(),
                shortcut_seats: Some(0),
                ..Options::default()
            },
        );
        assert_eq!(plain.checkpoints, named_but_nobody.checkpoints);
        // Read back from the agents themselves, seat by seat: the first build
        // set every one of these and none of them heard it.
        let taken = SiteShortcuts::named("envelopes-across-own-moves").unwrap();
        let two_seats = run_with(
            23,
            2,
            Options {
                shortcuts: taken,
                shortcut_seats: Some(0b0101),
                ..Options::default()
            },
        );
        for seat in 0..4 {
            let expected = if seat % 2 == 0 {
                taken
            } else {
                SiteShortcuts::default()
            };
            assert_eq!(
                two_seats.session.seat_shortcuts(seat),
                expected,
                "seat {seat}"
            );
        }
        let every_seat = run_with(
            23,
            2,
            Options {
                shortcuts: taken,
                ..Options::default()
            },
        );
        for seat in 0..every_seat.session.game.players.len() {
            assert_eq!(
                every_seat.session.seat_shortcuts(seat),
                taken,
                "seat {seat}"
            );
        }
        assert_eq!(plain.seat_seconds.len(), plain.session.game.players.len());
        // Every major took its turns. A seat that never acts — a roster slot
        // kept for Free Cities, say — has nothing to time.
        for (seat, player) in plain.session.game.players.iter().enumerate() {
            if !player.is_minor && !player.is_barbarian {
                assert!(plain.seat_seconds[seat] > 0.0, "seat {seat}");
            }
        }
        assert!(SiteShortcuts::named("no-such-shortcut").is_err());
        assert_eq!(SiteShortcuts::named("").unwrap(), SiteShortcuts::default());
        for name in SiteShortcuts::NAMES {
            assert_ne!(
                SiteShortcuts::named(name).unwrap(),
                SiteShortcuts::default(),
                "{name} must turn something on"
            );
        }
    }

    /// The stronger form: every single reused yield is derived again on the
    /// spot and compared. A write to the board that no action, tile write or
    /// `Game::board_written` reported panics here with the city and the turn.
    #[test]
    fn every_kept_yield_is_what_the_board_gives() {
        let audited = run_with(
            19,
            YIELD_TURNS,
            Options {
                documents: false,
                audits_rival_yields: true,
                ..Options::default()
            },
        );
        let (reused, _) = audited.session.game.rival_yields_reused();
        assert!(reused > 0, "an audit of nothing proves nothing");
    }
}
