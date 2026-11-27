//! What the simulator debugger promises about a played game's timeline.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::super::{stock_opening_params, Session};
use super::{
    Layer, Mark, Timeline, COARSEST_STRIDE, DENSE_MARKS, MAX_SAVE_POINTS, MEMORY_LINE,
    MEMORY_STEPS, MIN_MARKS, ROOMY_MARKS,
};
use crate::game::{Action, BlackSwanKind};
use crate::setup::{GameSpeed, MapScript, MapTopology};

/// A small world with a person at seat zero.
fn played() -> Session {
    let mut params = stock_opening_params(20_261_118);
    params.spectate = false;
    params.num_players = 2;
    params.num_city_states = 1;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    params.game_speed = GameSpeed::Standard;
    params.max_turns = 500;
    Session::new(params)
}

fn end_turn(session: &mut Session) {
    assert_eq!(session.act(&json!({"type": "end_turn"})), None);
}

/// The whole world as a save would carry it.
fn world(session: &Session) -> String {
    crate::protocol::save_value(&session.game)
        .expect("a game can be saved")
        .to_string()
}

fn marked(session: &Session) -> Vec<u64> {
    session.state()["debug"]["turns"]
        .as_array()
        .expect("the state names the marked turns")
        .iter()
        .map(|turn| turn.as_u64().unwrap())
        .collect()
}

/// The first thing the seat may do that is not ending its turn.
fn something_to_do(session: &Session) -> Value {
    let action = session
        .game
        .legal_actions(0)
        .into_iter()
        .find(|action| !matches!(action, Action::EndTurn))
        .expect("an opening turn offers a decision");
    serde_json::to_value(action).unwrap()
}

#[test]
fn a_played_game_walks_back_and_forward_along_its_own_turns() {
    let mut session = played();
    assert_eq!(marked(&session), [1]);
    let opening = world(&session);
    end_turn(&mut session);
    let second = world(&session);
    end_turn(&mut session);
    end_turn(&mut session);
    let fourth = world(&session);
    assert_eq!(marked(&session), [1, 2, 3, 4]);

    assert_eq!(session.debug_back(), Ok(3));
    assert_eq!(session.debug_jump(2), Ok(2));
    assert_eq!(world(&session), second);
    // Going back forgets nothing: the turns ahead are still there to return to.
    assert_eq!(marked(&session), [1, 2, 3, 4]);
    assert_eq!(session.debug_forward(), Ok(3));
    assert_eq!(session.debug_forward(), Ok(4));
    assert_eq!(world(&session), fourth);
    assert!(session.debug_forward().is_err());

    assert_eq!(session.debug_jump(1), Ok(1));
    assert_eq!(world(&session), opening);
    assert!(session.debug_back().is_err());
    // Played forward again, the old future is replaced by the new one.
    end_turn(&mut session);
    assert_eq!(marked(&session), [1, 2]);
}

/// A timeline with a mark on each of these turns, and room for so many.
fn timeline_of(turns: std::ops::RangeInclusive<u32>, room: usize) -> Timeline {
    let game = played().game;
    let mut timeline = Timeline {
        room: Some(room),
        ..Timeline::default()
    };
    for turn in turns {
        timeline.marks.push(Mark {
            turn,
            actions: 0,
            game: game.clone(),
            by: None,
            whole: false,
            minds: None,
            world: u64::from(turn),
        });
    }
    timeline
}

fn turns_of(timeline: &Timeline) -> Vec<u32> {
    timeline.marks.iter().map(|mark| mark.turn).collect()
}

#[test]
fn every_turn_keeps_its_mark_while_there_is_room_and_the_oldest_go_first() {
    let mut roomy = timeline_of(1..=100, 100);
    roomy.thin();
    assert_eq!(turns_of(&roomy), (1..=100).collect::<Vec<_>>());

    // Ten too many: the ten oldest odd turns go, and never the first.
    let mut tight = timeline_of(1..=100, 90);
    tight.thin();
    let mut left = vec![1];
    left.extend((2..=22).step_by(2));
    left.extend(23..=100);
    assert_eq!(turns_of(&tight), left);

    // Room for the newest sixty and as many again: those sixty are whole, and
    // behind them the oldest are down to one in four where the rest are still
    // one in two.
    let mut short = timeline_of(1..=200, 2 * DENSE_MARKS);
    short.thin();
    let mut left = vec![1];
    left.extend((4..=44).step_by(4));
    left.extend((46..=140).step_by(2));
    left.extend(141..=200);
    assert_eq!(turns_of(&short), left);
    assert_eq!(left.len(), 2 * DENSE_MARKS);

    // With hardly any room, half of it is the newest turns and the other
    // half is spread over everything before them. The room is never exceeded.
    let mut least = timeline_of(1..=100, 0);
    least.thin();
    assert_eq!(
        turns_of(&least),
        [1, 16, 32, 48, 64, 80, 95, 96, 97, 98, 99, 100]
    );
    // A game too long even for one turn in sixty-four loses its oldest.
    let mut longest = timeline_of(1..=1000, 0);
    longest.thin();
    assert_eq!(
        turns_of(&longest),
        [1, 704, 768, 832, 896, 960, 995, 996, 997, 998, 999, 1000]
    );
    assert_eq!(longest.marks.len(), MIN_MARKS);
}

/// A timeline whose world era changes on each of these turns.
fn with_eras(mut timeline: Timeline, dawns: &[u32]) -> Timeline {
    for mark in &mut timeline.marks {
        mark.game.world_era = dawns.iter().filter(|dawn| **dawn <= mark.turn).count();
    }
    timeline
}

#[test]
fn the_first_turn_of_each_era_is_kept_when_turns_are_thinned() {
    // Operator 2026-11-27: "lets add era back and era forward to the
    // controls. lets add a turn 1 option too ... we should make sure to save
    // these turns for fast jumping". The turns those go to stay, as the
    // first turn always has: an era's first turn among the odd ones the
    // stride lets go, and the room is still never exceeded.
    let dawns = [39, 77, 113, 151];
    let mut short = with_eras(timeline_of(1..=200, 2 * DENSE_MARKS), &dawns);
    short.thin();
    let left = turns_of(&short);
    assert_eq!(left.len(), 2 * DENSE_MARKS);
    for turn in [1, 39, 77, 113, 151] {
        assert!(left.contains(&turn), "turn {turn} was let go: {left:?}");
    }
    // With hardly any room they stay beside the newest turns.
    let mut least = with_eras(timeline_of(1..=100, 0), &[23, 47, 71]);
    least.thin();
    let left = turns_of(&least);
    assert_eq!(left.len(), MIN_MARKS);
    for turn in [1, 23, 47, 71, 95, 96, 97, 98, 99, 100] {
        assert!(left.contains(&turn), "turn {turn} was let go: {left:?}");
    }
    // More eras than there is room for: the oldest go, and the room holds.
    let mut crowded = with_eras(timeline_of(1..=100, 0), &(2..=100).collect::<Vec<_>>());
    crowded.thin();
    assert_eq!(turns_of(&crowded), [1, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100]);
}

#[test]
fn memory_past_the_line_takes_room_away_a_fifth_at_a_time() {
    let step = MEMORY_LINE / MEMORY_STEPS;
    let mut timeline = timeline_of(1..=200, 0);
    timeline.room = None;
    timeline.squeeze(MEMORY_LINE);
    assert_eq!(timeline.room(), ROOMY_MARKS);
    timeline.squeeze(MEMORY_LINE + 1);
    assert_eq!(timeline.room(), 160);
    timeline.thin();
    assert_eq!(timeline.marks.len(), 160);
    // Memory that has not grown by a step since has nothing new to say.
    timeline.squeeze(MEMORY_LINE + step);
    assert_eq!(timeline.room(), 160);
    timeline.squeeze(MEMORY_LINE + 1 + step);
    assert_eq!(timeline.room(), 128);
    // It goes on for as long as the memory grows, well below the newest
    // sixty, and stops at the fewest marks a timeline is left with.
    for grown in 2..40 {
        timeline.thin();
        timeline.squeeze(MEMORY_LINE + 1 + grown * step);
    }
    timeline.thin();
    assert_eq!(timeline.room(), MIN_MARKS);
    let left = turns_of(&timeline);
    assert_eq!(left.len(), MIN_MARKS);
    assert_eq!(left[0], 1);
    assert_eq!(left[MIN_MARKS - 6..], [195, 196, 197, 198, 199, 200]);

    // A lower line is listened to in smaller steps.
    let mut small = timeline_of(1..=200, 0);
    small.room = None;
    small.memory_line = Some(256 << 20);
    small.squeeze((256 << 20) + 1);
    assert_eq!(small.room(), 160);
    small.squeeze((256 << 20) + 1 + (8 << 20));
    assert_eq!(small.room(), 128);

    // A page that knows its device says where the line is.
    let mut session = played();
    let moved = session.debug(&json!({"op": "room", "megabytes": 2048}));
    assert_eq!(moved["error"], Value::Null);
    assert_eq!(moved["debug"]["memory_line"], json!(2048));
    assert_eq!(session.debugger.memory_line, Some(2048 << 20));
    assert!(session.debug(&json!({"op": "room"}))["error"].is_string());
}

#[test]
fn a_save_point_is_a_world_to_come_back_to_with_its_own_history() {
    let mut session = played();
    end_turn(&mut session);
    let top_of_two = world(&session);
    let first = something_to_do(&session);
    assert_eq!(session.act(&first), None);
    let kept = world(&session);
    let saved = session.debug(&json!({"op": "save_point", "name": "  before the fork "}));
    assert_eq!(saved["error"], Value::Null);
    assert_eq!(
        saved["debug"]["saves"],
        json!([{"id": 1, "name": "before the fork", "auto": false, "turn": 2, "acted": 1}])
    );
    end_turn(&mut session);
    end_turn(&mut session);
    assert_eq!(marked(&session), [1, 2, 3, 4]);

    // Another history, played from the first turn.
    assert_eq!(session.debug_jump(1), Ok(1));
    end_turn(&mut session);
    end_turn(&mut session);
    let left = world(&session);
    assert_eq!(marked(&session), [1, 2, 3]);

    let back = session.debug(&json!({"op": "goto_save", "id": 1}));
    assert_eq!(back["error"], Value::Null);
    assert_eq!(world(&session), kept);
    // The timeline is the save point's own: the first turn, which the two
    // histories share, and the top of the turn it was taken in.
    assert_eq!(marked(&session), [1, 2]);
    assert_eq!(back["debug"]["acted"], json!(1));
    // The world that was left has been kept, to go back to the same way.
    assert_eq!(
        back["debug"]["saves"][1],
        json!({"id": 2, "name": "Where you were", "auto": true, "turn": 3, "acted": 0})
    );
    let returned = session.debug(&json!({"op": "goto_save", "id": 2}));
    assert_eq!(returned["error"], Value::Null);
    assert_eq!(world(&session), left);
    assert_eq!(marked(&session), [1, 3]);
    // Leaving a world that is a save point already keeps nothing more.
    assert_eq!(returned["debug"]["saves"].as_array().unwrap().len(), 2);

    // A save point taken part-way through a turn can be undone from.
    assert_eq!(session.debug_goto_save(1), Ok(2));
    assert_eq!(session.debug_undo(), Ok(0));
    assert_eq!(world(&session), top_of_two);
}

#[test]
fn save_points_are_named_replaced_and_let_go() {
    let mut session = played();
    let unnamed = session.debug(&json!({"op": "save_point"}));
    assert_eq!(unnamed["debug"]["saves"][0]["name"], json!("Turn 1"));
    end_turn(&mut session);
    // A name already taken is given to the newer world.
    let again = session.debug(&json!({"op": "save_point", "name": "Turn 1"}));
    assert_eq!(
        again["debug"]["saves"],
        json!([{"id": 2, "name": "Turn 1", "auto": false, "turn": 2, "acted": 0}])
    );
    // Both halves of a split start with the save points there were.
    assert_eq!(
        session.fork().state()["debug"]["saves"],
        again["debug"]["saves"]
    );

    assert!(session.debug(&json!({"op": "goto_save"}))["error"].is_string());
    assert!(session.debug(&json!({"op": "goto_save", "id": 1}))["error"].is_string());
    assert_eq!(
        session.debug(&json!({"op": "drop_save", "id": 2}))["error"],
        Value::Null
    );
    assert!(session.debug(&json!({"op": "drop_save", "id": 2}))["error"].is_string());
    assert_eq!(session.state()["debug"]["saves"], json!([]));

    // A new world starts with none.
    assert_eq!(session.debug_save_point("kept"), Ok(3));
    session.debugger_begins();
    assert_eq!(session.state()["debug"]["saves"], json!([]));
}

#[test]
fn undo_takes_back_the_last_action_and_nothing_else() {
    let mut session = played();
    let untouched = world(&session);
    let first = something_to_do(&session);
    assert_eq!(session.act(&first), None);
    let after_one = world(&session);
    let second = something_to_do(&session);
    assert_eq!(session.act(&second), None);
    assert_eq!(session.state()["debug"]["acted"], json!(2));

    assert_eq!(session.debug_undo(), Ok(1));
    assert_eq!(world(&session), after_one);
    assert_eq!(session.debug_undo(), Ok(0));
    assert_eq!(world(&session), untouched);
    assert!(session.debug_undo().is_err());

    // Back from a turn the seat has acted in is the top of that turn.
    assert_eq!(session.act(&first), None);
    assert_eq!(session.debug_back(), Ok(1));
    assert_eq!(world(&session), untouched);
}

#[test]
fn a_step_plays_one_turn_and_stops_on_the_turn_it_was_told_to() {
    let mut session = played();
    let refused = session.debug(&json!({"op": "breakpoints", "categories": ["Nonsense"]}));
    assert!(refused["error"].is_string());
    let set = session.debug(&json!({"op": "breakpoints", "turns": [3]}));
    assert_eq!(set["error"], Value::Null);
    assert_eq!(set["debug"]["breakpoints"]["turns"], json!([3]));

    let stepped = session.debug(&json!({"op": "step"}));
    assert_eq!(stepped["turn"], json!(2));
    assert_eq!(stepped["debug"]["stopped"], Value::Null);
    let stopped = session.debug(&json!({"op": "step"}));
    assert_eq!(stopped["turn"], json!(3));
    assert_eq!(
        stopped["debug"]["stopped"],
        json!({"kind": "turn", "turn": 3})
    );
    assert_eq!(marked(&session), [1, 2, 3]);
}

/// An era breakpoint stops a run on the first turn of a new world era, and
/// the timeline says which turn each era it has been through began on.
#[test]
fn an_era_breakpoint_stops_on_the_first_turn_of_a_new_era_and_the_timeline_marks_it() {
    let mut session = played();
    let set = session.debug(&json!({"op": "breakpoints", "eras": true}));
    assert_eq!(set["error"], Value::Null);
    assert_eq!(set["debug"]["breakpoints"]["eras"], json!(true));
    assert_eq!(
        set["debug"]["eras"],
        json!([{"era": 0, "name": "ancient", "turn": 1}])
    );
    // The world's next era is due: the countdown the engine keeps ends now.
    session.game.world_era_countdown_end = Some(session.game.turn);
    let mut hit = Value::Null;
    for _ in 0..4 {
        hit = session.debug(&json!({"op": "step"}));
        if hit["debug"]["stopped"] != Value::Null {
            break;
        }
    }
    let began = hit["turn"].as_u64().expect("a turn");
    assert_eq!(
        hit["debug"]["stopped"],
        json!({"kind": "era", "turn": began, "era": 1})
    );
    assert_eq!(
        hit["debug"]["eras"],
        json!([
            {"era": 0, "name": "ancient", "turn": 1},
            {"era": 1, "name": "classical", "turn": began},
        ])
    );
    // An era that goes on does not stop every turn it goes on for.
    let next = session.debug(&json!({"op": "step"}));
    assert_eq!(next["debug"]["stopped"], Value::Null);
    // With the breakpoint off the same change stops nothing.
    let mut quiet = played();
    quiet.game.world_era_countdown_end = Some(quiet.game.turn);
    for _ in 0..4 {
        let out = quiet.debug(&json!({"op": "step"}));
        assert_eq!(out["debug"]["stopped"], Value::Null);
    }
    assert_eq!(quiet.game.world_era, 1);
}

#[test]
fn an_event_breakpoint_reads_what_the_turn_added_to_the_log() {
    let mut session = played();
    // Every category at once: the first turn that says anything stops the run.
    let all: Vec<&str> = crate::game::EVENT_CATEGORIES.to_vec();
    let set = session.debug(&json!({"op": "breakpoints", "categories": all, "every_seat": true}));
    assert_eq!(set["error"], Value::Null);
    let mut hit = Value::Null;
    for _ in 0..40 {
        let before = session.game.chronicle.events.len();
        let out = session.debug(&json!({"op": "step"}));
        hit = out["debug"]["stopped"].clone();
        if !hit.is_null() {
            assert!(
                session.game.chronicle.events.len() > before,
                "a stop names a new event"
            );
            break;
        }
        assert_eq!(
            session.game.chronicle.events.len(),
            before,
            "a turn that logged something should have stopped"
        );
    }
    assert_eq!(hit["kind"], json!("event"), "forty turns logged nothing");
    let text = hit["text"].as_str().unwrap();
    assert!(session
        .game
        .chronicle
        .events
        .iter()
        .any(|event| event.text == text));
}

#[test]
fn a_layer_takes_its_own_decisions_and_leaves_the_turn_open() {
    let mut session = played();
    let set = session.debug(&json!({"op": "layers", "layers": ["research"]}));
    assert_eq!(set["error"], Value::Null);
    assert!(session.debug(&json!({"op": "layers", "layers": ["everything"]}))["error"].is_string());

    let before = session.game.log.len();
    let out = session.debug(&json!({"op": "assist"}));
    let done: Vec<Action> = session
        .game
        .log
        .since(before)
        .map(|(seat, action)| {
            assert_eq!(*seat, 0);
            action.clone()
        })
        .collect();
    assert!(!done.is_empty(), "an opening turn has research to choose");
    assert_eq!(out["debug"]["assisted"], json!(done.len()));
    for action in &done {
        assert_eq!(Layer::of(action), Some(Layer::Research), "{action:?}");
    }
    assert_eq!(session.game.current, 0, "the turn is still the person's");
    assert_eq!(session.game.turn, 1);

    // Switched on, the layer takes its share of every turn the person opens.
    end_turn(&mut session);
    assert_eq!(session.game.turn, 2);
    assert_eq!(session.game.current, 0);
}

#[test]
fn every_layer_together_still_leaves_the_end_of_the_turn_to_the_person() {
    let mut session = played();
    let every: Vec<&str> = Layer::ALL.iter().map(|layer| layer.id()).collect();
    session.debug(&json!({"op": "layers", "layers": every}));
    session.debug(&json!({"op": "assist"}));
    assert_eq!(session.game.turn, 1);
    assert_eq!(session.game.current, 0);
    assert_eq!(Layer::of(&Action::EndTurn), None);
}

#[test]
fn a_watched_table_keeps_no_timeline() {
    let mut params = stock_opening_params(3);
    params.num_players = 2;
    params.num_city_states = 0;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    let mut session = Session::new(params);
    assert!(session.debug(&json!({"op": "back"}))["error"].is_string());
    assert!(session.state().get("debug").is_none());
}

/// An operation asked to answer `lean` does what it always does and answers
/// without the state document: where the world stands, each seat's score,
/// and the debugger's own block exactly as the document carries it.
#[test]
fn a_lean_answer_says_where_the_world_stands_without_the_state_document() {
    let mut session = played();
    let whole = session.debug(&json!({"op": "step"}));
    let lean = session.debug(&json!({"op": "status", "lean": true}));
    assert_eq!(lean["lean"], json!(true));
    assert_eq!(lean["error"], Value::Null);
    assert_eq!(lean["turn"], whole["turn"]);
    assert_eq!(lean["current"], whole["current"]);
    assert_eq!(lean["finished"], whole["finished"]);
    assert_eq!(lean["winner"], whole["winner"]);
    assert_eq!(lean["debug"], whole["debug"]);
    for part in ["map", "players", "cities", "units", "legal_actions"] {
        assert!(whole.get(part).is_some(), "the state document has {part}");
        assert!(lean.get(part).is_none(), "a lean answer has no {part}");
    }
    let scores = lean["scores"].as_array().expect("every seat's score");
    assert_eq!(scores.len(), 2, "the two civilizations, not the city-state");
    assert_eq!(scores[0]["id"], json!(0));
    assert_eq!(scores[0]["alive"], json!(true));
    assert_eq!(scores[0]["score"], json!(session.game.score(0)));
    let rival = scores[1]["id"].as_u64().expect("a seat") as usize;
    assert_eq!(scores[1]["score"], json!(session.game.score(rival)));

    // The operation itself is the same one: a turn stepped lean is the turn.
    let (mut one, mut other) = (played(), played());
    one.debug(&json!({"op": "step"}));
    let stepped = other.debug(&json!({"op": "step", "lean": true}));
    assert_eq!(world(&one), world(&other));
    assert_eq!(stepped["turn"], json!(other.game.turn));
    let back = other.debug(&json!({"op": "back", "lean": true}));
    assert_eq!(back["error"], Value::Null);
    assert_eq!(back["turn"], json!(other.game.turn));
    assert_eq!(one.debug(&json!({"op": "back"}))["turn"], back["turn"]);

    // A refusal is said the same way, and leaves the world as it was.
    let refused = other.debug(&json!({"op": "no such thing", "lean": true}));
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .starts_with("no debugger operation"));
    assert_eq!(refused["lean"], json!(true));
    assert_eq!(refused["turn"], back["turn"]);
}

/// A lean answer says how the played seat stands in the numbers its row of
/// the standings shows, and they are the ones the state document gives it:
/// what a search is asked to make the most of is what the page shows.
#[test]
fn a_lean_answer_gives_the_seat_its_row_of_the_standings() {
    let mut session = played();
    for _ in 0..6 {
        step_as(&mut session, "jack");
    }
    let whole = session.state();
    let lean = session.debug(&json!({"op": "status", "lean": true}));
    let seat = &lean["seat"];
    let mine = whole["players"]
        .as_array()
        .expect("the document lists the civilizations")
        .iter()
        .find(|player| player["id"] == json!(0))
        .expect("the seat is one of them");
    assert_eq!(seat["turn"], whole["turn"]);
    for key in [
        "alive",
        "score",
        "cities",
        "population",
        "gold_per_turn",
        "yields",
    ] {
        assert!(!seat[key].is_null(), "the reading has no {key}");
        assert_eq!(seat[key], mine[key], "{key}");
    }
    for key in ["food", "production", "gold", "science", "culture", "faith"] {
        assert!(seat["yields"][key].is_number(), "{key}");
    }
}

/// Operator 2026-11-24: "we can compute (a degree in advance) the futures of
/// the paths we are on", "we will compute the future of each path".
///
/// `ahead` plays on from the end of what the seated Civ AI has played and
/// comes back: the world on the table is the open turn's as it stood, the
/// turns played are kept ahead of it to be walked to, and the answer reads
/// the seat on each and says what was seen coming.
#[test]
fn ahead_plays_the_turns_to_come_and_comes_back_to_the_open_turn() {
    let (mut stepped, mut run) = (played(), played());
    step_as(&mut stepped, "jack");
    step_as(&mut run, "jack");
    let open = world(&run);
    let answer = run.debug(&json!({"op": "ahead", "turns": 3, "strategy": "jack", "lean": true}));
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["turn"], json!(2));
    assert_eq!(world(&run), open, "the open turn stands as it stood");
    assert_eq!(marked(&run), [1, 2, 3, 4, 5]);
    assert_eq!(answer["debug"]["last"], json!(5));
    let readings = answer["readings"].as_array().expect("a reading a turn");
    let turns: Vec<_> = readings
        .iter()
        .map(|reading| reading["turn"].clone())
        .collect();
    assert_eq!(turns, [json!(3), json!(4), json!(5)]);
    assert!(answer["alerts"].is_array());
    // Asked again, it goes on from the end of what is kept, not from here.
    let more = run.debug(&json!({"op": "ahead", "turns": 2, "strategy": "jack", "lean": true}));
    assert_eq!(more["error"], Value::Null);
    assert_eq!(more["turn"], json!(2));
    assert_eq!(world(&run), open);
    assert_eq!(marked(&run), [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(more["readings"][0]["turn"], json!(6));
    // The turns ahead are the ones stepping plays, and are walked to.
    for _ in 0..3 {
        step_as(&mut stepped, "jack");
    }
    assert_eq!(run.debug_jump(5), Ok(5));
    assert_eq!(world(&stepped), world(&run));
    // It needs a number of turns, as `play` does.
    let none = run.debug(&json!({"op": "ahead", "strategy": "jack", "lean": true}));
    assert!(none["error"]
        .as_str()
        .is_some_and(|error| error.contains("ahead needs turns")));
    // A turn the seat has played in is not left: coming back would undo it.
    let mut acted = played();
    step_as(&mut acted, "jack");
    let action = something_to_do(&acted);
    assert_eq!(acted.act(&action), None);
    let refused = acted.debug(&json!({"op": "ahead", "turns": 1, "lean": true}));
    assert!(refused["error"]
        .as_str()
        .is_some_and(|error| error.contains("played in")));
}

/// Operator 2026-11-24: "Right now Doctor Strange's Civ 6 goes 4 turns and
/// pauses. This should not happen. It should be a continuous effort. and
/// blazingly fast."
///
/// So that time runs on without stopping, a world's future is played a turn
/// at a time as its turns are shown, and each of those is another `ahead`. A
/// half of a split keeps no agents with its marks, so every one of them used
/// to be played by agents that had just sat down: another game than a run
/// plays, and each turn of it twice the work. The agents that played the
/// last turn ahead are kept to play the next (`Scouts`), so a future played
/// a turn at a time is the one played in one go, which is the one a run
/// plays.
#[test]
fn a_future_played_a_turn_at_a_time_is_the_one_played_in_one_go() {
    fn ahead(half: &mut Session, turns: u32, agent: &str) {
        let answer =
            half.debug(&json!({"op": "ahead", "turns": turns, "strategy": agent, "lean": true}));
        assert_eq!(answer["error"], Value::Null);
        assert_eq!(
            answer["readings"].as_array().map(Vec::len),
            Some(turns as usize)
        );
    }
    let mut session = played();
    for _ in 0..24 {
        step_as(&mut session, "jack");
    }
    let (mut whole, mut piecewise, mut run) = (session.fork(), session.fork(), session.fork());
    let open = world(&piecewise);
    ahead(&mut whole, 8, "jack");
    for _ in 0..8 {
        ahead(&mut piecewise, 1, "jack");
        assert_eq!(world(&piecewise), open, "the open turn stands as it stood");
    }
    for _ in 0..8 {
        step_as(&mut run, "jack");
    }
    assert_eq!(marked(&piecewise), marked(&whole));
    assert_eq!(prints(&piecewise), prints(&whole));
    assert_eq!(prints(&piecewise), prints(&run));

    // Time runs on: the open turn moves a turn along the line, and the line
    // is played a turn further from its end by the same agents.
    for turn in 26..=28 {
        assert_eq!(piecewise.debug_jump(turn), Ok(turn));
        ahead(&mut piecewise, 1, "jack");
        step_as(&mut run, "jack");
        assert_eq!(prints(&piecewise), prints(&run));
    }

    // A line that comes to the same turn through another world is not
    // theirs to play on from. The seat does something by hand and the turns
    // ahead are played again from that; walked back, what is ahead is played
    // on by agents that have just sat down, as in a half that kept none.
    let (mut kept, mut none) = (session.fork(), session.fork());
    ahead(&mut kept, 3, "jack");
    let action = something_to_do(&kept);
    for half in [&mut kept, &mut none] {
        assert_eq!(half.act(&action), None);
        for _ in 0..3 {
            step_as(half, "jack");
        }
        assert_eq!(half.debug_jump(26), Ok(26));
    }
    assert_eq!(prints(&kept), prints(&none));
    assert_ne!(prints(&kept)[..28], prints(&whole)[..28], "another world");
    ahead(&mut kept, 2, "jack");
    ahead(&mut none, 2, "jack");
    assert_eq!(prints(&kept), prints(&none));

    // Another Civ AI in the seat plays its own line from the open turn, and
    // the agents kept from the first one's are let go of.
    let (mut swapped, mut fresh) = (session.fork(), session.fork());
    ahead(&mut swapped, 3, "jack");
    ahead(&mut swapped, 2, "moonshot");
    ahead(&mut swapped, 2, "moonshot");
    ahead(&mut fresh, 4, "moonshot");
    assert_eq!(prints(&swapped), prints(&fresh));
}

/// What `foresee` answered was seen coming.
fn foreseen(session: &mut Session, request: Value) -> Value {
    let answer = session.debug(&request);
    assert_eq!(answer["error"], Value::Null);
    assert!(
        answer["foresight"].is_object(),
        "the answer says what is seen"
    );
    answer["foresight"].clone()
}

/// The turn and print of each turn a forecast says it opened.
fn foreseen_prints(seen: &Value) -> Vec<(u64, u64)> {
    seen["worlds"]
        .as_array()
        .expect("a forecast gives the print of each turn it opened")
        .iter()
        .map(|pair| (pair[0].as_u64().unwrap(), pair[1].as_u64().unwrap()))
        .collect()
}

/// Operator 2026-11-24: "each branch in doctor strange's civ 6 should
/// simulate out to the end of the branch."
///
/// `foresee` plays the world's future on a copy: the world on the table and
/// the turns it keeps are as they stood, the copy goes a stretch further
/// each time it is asked, and it comes to the end of the game. The turns it
/// plays are the ones the world itself goes on to play, which is what makes
/// what it saw a forecast of this world: turn for turn the print it gives is
/// the one the world's own mark is given.
#[test]
fn foresee_plays_the_future_to_the_end_of_the_game_and_leaves_the_world_where_it_is() {
    let short = || {
        let mut params = stock_opening_params(20_261_118);
        params.spectate = false;
        params.num_players = 2;
        params.num_city_states = 1;
        params.width = 20;
        params.height = 14;
        params.map_topology = MapTopology::Flat;
        params.map_script = MapScript::Pangaea;
        params.game_speed = GameSpeed::Standard;
        params.max_turns = 30;
        Session::new(params)
    };
    let (mut session, mut run) = (short(), short());
    for world in [&mut session, &mut run] {
        for _ in 0..4 {
            step_as(world, "jack");
        }
    }
    let (open, kept) = (world(&session), marked(&session));
    let first = foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 6, "strategy": "jack", "lean": true}),
    );
    assert_eq!(world(&session), open, "the world stands where it stood");
    assert_eq!(marked(&session), kept, "and keeps the turns it kept");
    assert_eq!(first["anew"], json!(true));
    assert_eq!(first["from"], json!(5));
    assert_eq!(first["turn"], json!(11));
    assert_eq!(first["played"], json!(6));
    assert_eq!(first["done"], json!(false));
    assert_eq!(first["seat"], json!("jack"));
    assert_eq!(first["limit"], json!(30));
    assert_eq!(first["eras"][0]["turn"], json!(5));
    assert!(first["eras"][0]["name"].is_string());
    // The turns it played are the turns a run plays from here.
    for _ in 0..6 {
        step_as(&mut run, "jack");
    }
    let line: Vec<(u64, u64)> = marked(&run).into_iter().zip(prints(&run)).collect();
    assert_eq!(foreseen_prints(&first), line[4..]);
    assert_eq!(first["base"].as_u64(), Some(line[4].1));

    // Asked again it goes on from where it stood, and says only what is new
    // to a page that says how far it was told.
    let more = foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 3, "known": 11, "strategy": "jack", "lean": true}),
    );
    assert_eq!(more["anew"], json!(false));
    assert_eq!(more["turn"], json!(14));
    let turns: Vec<u64> = foreseen_prints(&more)
        .iter()
        .map(|(turn, _)| *turn)
        .collect();
    assert_eq!(turns, [12, 13, 14]);
    // No turns reads what has been seen and plays nothing.
    let read = foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 0, "strategy": "jack", "lean": true}),
    );
    assert_eq!(read["turn"], json!(14));
    assert_eq!(read["played"], json!(0));
    assert_eq!(foreseen_prints(&read).len(), 10, "told whole");

    // To the end of the game, which is where it stops for good.
    let mut last = read;
    for _ in 0..4 {
        last = foreseen(
            &mut session,
            json!({"op": "foresee", "turns": 100, "strategy": "jack", "lean": true}),
        );
        if last["done"] == json!(true) {
            break;
        }
    }
    assert_eq!(last["done"], json!(true));
    let seen = last["seen"].as_array().expect("what was seen coming");
    let end = seen.last().expect("the end of the game is seen coming");
    assert!(
        end["kind"] == json!("won") || end["kind"] == json!("over"),
        "{end}"
    );
    assert!(end["era_name"].is_string());
    let after = foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 5, "strategy": "jack", "lean": true}),
    );
    assert_eq!(after["played"], json!(0));
    assert_eq!(after["turn"], last["turn"]);
    assert_eq!(world(&session), open);
    assert_eq!(marked(&session), kept);
    // The game it came to the end of is the one a run comes to the end of.
    for _ in 0..40 {
        if run.game.is_finished() {
            break;
        }
        let _ = run.debug(&json!({"op": "step", "strategy": "jack", "lean": true}));
    }
    assert!(run.game.is_finished());
    assert_eq!(last["turn"].as_u64(), Some(u64::from(run.game.turn)));
    let winner = run
        .game
        .winner
        .map(|seat| run.game.players[seat].civ.clone());
    match winner {
        Some(civ) => assert_eq!(end["text"], json!(format!("{civ} wins the game"))),
        None => assert_eq!(end["kind"], json!("over")),
    }
    // It needs to be told how many turns, none or more.
    let none = session.debug(&json!({"op": "foresee", "strategy": "jack", "lean": true}));
    assert!(none["error"]
        .as_str()
        .is_some_and(|error| error.contains("foresee needs turns")));
    // The page keeps what it is told (`strangeSeen`), and reads nothing off
    // an answer that the answer does not give.
    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let reader = page
        .split_once("\nfunction strangeSeen(")
        .and_then(|(_, rest)| rest.split_once("\nfunction "))
        .expect("the page keeps what a forecast says")
        .0;
    let mut read = BTreeSet::new();
    for (at, _) in reader.match_indices("seen.") {
        let before = reader[..at].chars().next_back();
        if before.is_some_and(|before| before == '.' || before.is_ascii_alphanumeric()) {
            continue;
        }
        let key: String = reader[at + 5..]
            .chars()
            .take_while(|letter| letter.is_ascii_lowercase() || *letter == '_')
            .collect();
        read.insert(key);
    }
    assert!(read.len() >= 12, "{read:?}");
    for key in &read {
        assert!(
            last.get(key.as_str()).is_some(),
            "the page reads `{key}`, which a forecast does not say"
        );
    }
}

/// A forecast is the future of one line. It goes on being that while the
/// world walks along the line and plays it further, and is made again, from
/// the end of the line as it then is, once the line is played another way:
/// another Civ AI in the seat, a black swan, something done by hand. A copy
/// of the world has none, and foresees for itself.
#[test]
fn a_forecast_is_kept_along_its_line_and_made_again_off_it() {
    fn ask(session: &mut Session, agent: &str) -> Value {
        foreseen(
            session,
            json!({"op": "foresee", "turns": 2, "strategy": agent, "lean": true}),
        )
    }
    let mut session = played();
    for _ in 0..6 {
        step_as(&mut session, "jack");
    }
    // Made from the end of the line: the last turn played ahead.
    let ahead =
        session.debug(&json!({"op": "ahead", "turns": 3, "strategy": "jack", "lean": true}));
    assert_eq!(ahead["error"], Value::Null);
    let first = ask(&mut session, "jack");
    assert_eq!((&first["anew"], &first["from"]), (&json!(true), &json!(10)));
    assert_eq!(first["turn"], json!(12));
    // The world walks its line and plays it on: the same future.
    assert_eq!(session.debug_jump(5), Ok(5));
    assert_eq!(ask(&mut session, "jack")["anew"], json!(false));
    assert_eq!(session.debug_jump(9), Ok(9));
    let ahead =
        session.debug(&json!({"op": "ahead", "turns": 4, "strategy": "jack", "lean": true}));
    assert_eq!(ahead["error"], Value::Null);
    let along = ask(&mut session, "jack");
    assert_eq!(
        (&along["anew"], &along["from"]),
        (&json!(false), &json!(10))
    );
    assert_eq!(along["turn"], json!(16));
    // The turns the world has since played ahead are the ones foreseen.
    let line: Vec<(u64, u64)> = marked(&session).into_iter().zip(prints(&session)).collect();
    let seen = foreseen_prints(&foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 0, "strategy": "jack", "lean": true}),
    ));
    for turn in 10..=13 {
        let held = line.iter().find(|(at, _)| *at == turn).expect("kept");
        let told = seen.iter().find(|(at, _)| *at == turn).expect("foreseen");
        assert_eq!(held, told, "turn {turn}");
    }

    // A copy foresees for itself.
    let mut copy = session.fork();
    assert_eq!(ask(&mut copy, "jack")["anew"], json!(true));
    // Asked for anew, it is made again.
    let again = foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 1, "anew": true, "strategy": "jack", "lean": true}),
    );
    assert_eq!((&again["anew"], &again["from"]), (&json!(true), &json!(14)));
    assert_eq!(again["turn"], json!(15));
    // Another Civ AI in the seat plays another future, from the open turn:
    // the turns ahead are the first one's.
    let other = ask(&mut session, "moonshot");
    assert_eq!((&other["anew"], &other["from"]), (&json!(true), &json!(9)));
    assert_eq!(other["seat"], json!("moonshot"));
    assert_eq!(ask(&mut session, "moonshot")["anew"], json!(false));
    // Something done by hand in the open turn is not in the future foreseen.
    let action = something_to_do(&session);
    assert_eq!(session.act(&action), None);
    let acted = ask(&mut session, "moonshot");
    assert_eq!((&acted["anew"], &acted["from"]), (&json!(true), &json!(9)));
    assert_eq!(ask(&mut session, "moonshot")["anew"], json!(false));
    // A black swan makes the open turn another world.
    let swan = session.debug(&json!({"op": "black_swan", "kind": "leader_collapse", "seed": 7}));
    assert_eq!(swan["error"], Value::Null);
    assert_eq!(ask(&mut session, "moonshot")["anew"], json!(true));
}

/// Operator 2026-11-24: "the top of each branch should have either: no
/// nuclear war in the next N turns, no nuclear war, or nuclear war in N turns
/// (Era). we should note wars as well."
///
/// What is seen coming is every detonation and every war declared in the
/// turns played, each on the turn it happens with the world era of that
/// turn, and none of what was there before the forecast was made. A nuclear
/// war is struck on the copy as it stands here, which is the surest way to
/// have both in a short game.
#[test]
fn a_forecast_notes_the_detonations_and_the_wars_declared_on_the_way() {
    let mut params = stock_opening_params(20_261_118);
    params.spectate = false;
    params.num_players = 3;
    params.num_city_states = 2;
    params.width = 30;
    params.height = 20;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    params.game_speed = GameSpeed::Online;
    params.max_turns = 500;
    let mut session = Session::new(params);
    for _ in 0..24 {
        step_as(&mut session, "jack");
    }
    let quiet = foreseen(
        &mut session,
        json!({"op": "foresee", "turns": 2, "strategy": "jack", "lean": true}),
    );
    assert_eq!(quiet["detonations"], json!(0));
    let mut seer = session
        .debugger
        .seer
        .0
        .take()
        .expect("the forecast is kept");
    let wars_before = seer.conflicts.len();
    let mut struck = None;
    for _ in 0..40 {
        if seer
            .session
            .game
            .black_swan(Some(BlackSwanKind::NuclearWar), 11)
            .is_ok()
        {
            struck = Some(seer.session.game.turn);
            break;
        }
        assert!(seer.play(), "the game goes on");
    }
    let struck = struck.expect("a nuclear war is struck within forty turns");
    assert!(seer.play());
    let told = seer.json(1, None);
    let seen = told["seen"].as_array().expect("what was seen coming");
    let of = |kind: &str| -> Vec<&Value> {
        seen.iter()
            .filter(|seen| seen["kind"] == json!(kind))
            .collect()
    };
    let (nuclear, wars) = (of("nuclear"), of("war"));
    assert!(!nuclear.is_empty(), "{seen:?}");
    assert!(told["detonations"].as_u64().is_some_and(|count| count >= 1));
    assert!(seer.conflicts.len() > wars_before);
    assert!(!wars.is_empty(), "{seen:?}");
    for noted in nuclear.iter().chain(wars.iter()) {
        assert_eq!(noted["turn"].as_u64(), Some(u64::from(struck)));
        assert!(noted["era_name"].is_string());
    }
    assert!(nuclear.iter().all(|noted| noted["text"]
        .as_str()
        .is_some_and(|text| text.contains("detonates"))));
    assert!(wars.iter().all(|noted| noted["text"]
        .as_str()
        .is_some_and(|text| text.contains(" declares war on "))));
    // Each is said once: the next turn notes neither again.
    let (counted, noted) = (told["detonations"].clone(), seen.len());
    assert!(seer.play());
    let later = seer.json(1, None);
    let again = |kind: &str| {
        later["seen"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|seen| {
                seen["kind"] == json!(kind) && seen["turn"].as_u64() == Some(u64::from(struck))
            })
            .count()
    };
    assert_eq!(again("nuclear"), nuclear.len());
    assert_eq!(again("war"), wars.len());
    assert!(later["seen"].as_array().unwrap().len() >= noted);
    assert!(later["detonations"].as_u64() >= counted.as_u64());
    // A forecast made on a world a nuclear war has already struck has it on
    // record, on the turn it struck: a world walked back along its line has
    // that ahead of it again.
    seer.session.debugger_begins();
    let after = seer
        .session
        .debug(&json!({"op": "foresee", "turns": 0, "lean": true}));
    assert_eq!(after["error"], Value::Null);
    let record = after["foresight"]["seen"]
        .as_array()
        .expect("what is on record");
    let on_record = |kind: &str| {
        record
            .iter()
            .filter(|seen| {
                seen["kind"] == json!(kind) && seen["turn"].as_u64() == Some(u64::from(struck))
            })
            .count()
    };
    assert_eq!(on_record("nuclear"), nuclear.len());
    assert!(on_record("war") >= wars.len());
    assert_eq!(after["foresight"]["detonations"], later["detonations"]);
    assert!(record
        .iter()
        .all(|seen| seen["turn"].as_u64() <= after["foresight"]["from"].as_u64()));
}

/// Operator 2026-11-24: "add a 'Generate Black Swan' - this will be a random,
/// unpredicted event that spawns and changes the game". `black_swan` strikes
/// one on the world as it stands (`game/black_swan.rs`): the turns kept
/// ahead of the open one were played in a world it did not happen in and are
/// let go of, the open turn is marked again with a print of its own, and who
/// played into the turn still did.
#[test]
fn a_black_swan_strikes_the_open_turn_and_lets_go_of_the_turns_ahead() {
    let (mut struck, mut spared) = (played(), played());
    for session in [&mut struck, &mut spared] {
        for _ in 0..4 {
            step_as(session, "jack");
        }
        let ahead =
            session.debug(&json!({"op": "ahead", "turns": 3, "strategy": "jack", "lean": true}));
        assert_eq!(ahead["error"], Value::Null);
    }
    assert_eq!(marked(&struck), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(prints(&struck), prints(&spared));
    assert_eq!(
        struck.state()["debug"]["played_by"],
        json!([{"from": 1, "to": 8, "by": "jack"}])
    );
    assert_eq!(struck.state()["debug"]["swans"], json!([]));
    assert_eq!(struck.state()["black_swans"], json!([]));

    let answer = struck.debug(&json!({"op": "black_swan", "kind": "leader_collapse", "seed": 7}));
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["turn"], json!(5), "it strikes the open turn");
    let swan = answer["black_swan"].clone();
    assert_eq!(swan["turn"], json!(5));
    assert_eq!(swan["kind"], json!("leader_collapse"));
    assert_eq!(swan["name"], json!("Leader collapse"));
    assert_eq!(
        swan["ongoing"],
        json!(true),
        "its Anarchy is still to serve"
    );
    assert!(swan["text"]
        .as_str()
        .is_some_and(|text| text.starts_with("Leader collapse. ") && text.contains("Anarchy")));
    let leader = swan["player"].as_u64().expect("it names who it struck") as usize;
    assert!(struck.game.in_anarchy(leader));
    // The world's own account of it, and the timeline's.
    assert_eq!(answer["black_swans"], json!([swan]));
    assert_eq!(answer["debug"]["swans"], json!([swan]));
    // The turns ahead are let go of, and the open turn is another world.
    assert_eq!(marked(&struck), [1, 2, 3, 4, 5]);
    let (after, before) = (prints(&struck), prints(&spared));
    assert_eq!(
        after[..4],
        before[..4],
        "the turns behind are the turns they were"
    );
    assert_ne!(
        after[4], before[4],
        "the struck turn has a print of its own"
    );
    // Jack still played into the turn; the swan did not.
    assert_eq!(
        answer["debug"]["played_by"],
        json!([{"from": 1, "to": 5, "by": "jack"}])
    );
    // Its future is played again, from the world it now is, and is another
    // future than the one that was kept.
    let there = world(&struck);
    let again = struck.debug(&json!({"op": "ahead", "turns": 3, "strategy": "jack", "lean": true}));
    assert_eq!(again["error"], Value::Null);
    assert_eq!(marked(&struck), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(world(&struck), there);
    assert_ne!(prints(&struck)[5..], prints(&spared)[5..]);
    // The swan is on the line by its own account, not among what is seen
    // coming, though the seat was told of it on this very turn.
    assert!(struck
        .game
        .events_for(0)
        .iter()
        .any(|event| event.turn == 5 && event.important && event.text.starts_with("Black swan: ")));
    let seen = again["alerts"].as_array().expect("what was seen coming");
    assert!(
        !seen.iter().any(|alert| alert["text"]
            .as_str()
            .is_some_and(|text| text.contains("Black swan"))),
        "{seen:?}"
    );
    // Gone back before it and walked forward along the kept turns, the
    // world on its turn is the world it struck. The line still holds it.
    assert_eq!(struck.debug_jump(4), Ok(4));
    assert!(struck.game.black_swans.is_empty());
    assert_eq!(struck.state()["debug"]["swans"][0]["turn"], json!(5));
    assert_eq!(
        struck.state()["debug"]["swans"][0]["ongoing"],
        json!(false),
        "on the turn before, it is not going on yet"
    );
    assert_eq!(struck.debug_jump(5), Ok(5));
    assert_eq!(world(&struck), there);

    // One drawn by a seed, asked for lean: the answer still says what struck.
    let drawn = spared.debug(&json!({"op": "black_swan", "seed": 3, "lean": true}));
    assert_eq!(drawn["error"], Value::Null);
    let kinds: Vec<Value> = drawn["debug"]["swan_kinds"]
        .as_array()
        .expect("the timeline says which swans there are")
        .iter()
        .map(|kind| kind["id"].clone())
        .collect();
    assert_eq!(
        kinds,
        [
            json!("nuclear_war"),
            json!("pandemic"),
            json!("radiation_leak"),
            json!("leader_collapse"),
            json!("microstates")
        ]
    );
    assert!(kinds.contains(&drawn["black_swan"]["kind"]));
    assert_eq!(drawn["debug"]["swans"], json!([drawn["black_swan"]]));
    assert_eq!(marked(&spared), [1, 2, 3, 4, 5]);
    // The same seed strikes the world it was the same way.
    let mut twin = played();
    for _ in 0..4 {
        step_as(&mut twin, "jack");
    }
    let alike = twin.debug(&json!({"op": "black_swan", "seed": 3, "lean": true}));
    assert_eq!(alike["black_swan"], drawn["black_swan"]);
    assert_eq!(world(&twin), world(&spared));
    assert_eq!(prints(&twin), prints(&spared));

    // A swan nobody has heard of is refused, and so is one this world has
    // nothing for; a refused swan strikes nothing.
    let stood = world(&spared);
    let unknown = spared.debug(&json!({"op": "black_swan", "kind": "asteroid", "seed": 1}));
    assert_eq!(unknown["error"], json!("no black swan named \"asteroid\""));
    let mut young = played();
    let early = world(&young);
    let nothing = young.debug(&json!({"op": "black_swan", "kind": "microstates", "seed": 1}));
    assert_eq!(
        nothing["error"],
        json!("no civilization has a second city to break away")
    );
    assert_eq!(world(&spared), stood);
    assert_eq!(world(&young), early);
    assert_eq!(marked(&young), [1]);
    // In the middle of a round no turn is open to mark.
    assert_eq!(
        young.debug(&json!({"op": "step_seat"}))["error"],
        Value::Null
    );
    let rivals = young.debug(&json!({"op": "black_swan", "seed": 1}));
    assert!(rivals["error"]
        .as_str()
        .is_some_and(|error| error.contains("the rivals are playing")));
}

/// A world goes on being a game whatever strikes it. Every kind of swan is
/// struck in turn through the opening of two games, with turns played
/// between by the agents that were going to play them and the future played
/// ahead after each, as the page plays it: none of it may leave a world the
/// rules, the agents or the page's state document cannot go on from.
#[test]
fn a_game_is_played_on_through_every_black_swan() {
    for seed in [20_261_118_u64, 20_261_124] {
        let mut params = stock_opening_params(seed);
        params.spectate = false;
        params.num_players = 3;
        params.num_city_states = 2;
        params.width = 30;
        params.height = 20;
        params.map_topology = MapTopology::Flat;
        params.map_script = MapScript::Pangaea;
        params.game_speed = GameSpeed::Online;
        params.max_turns = 500;
        let mut session = Session::new(params);
        let on = |session: &Session| !session.game.is_finished() && session.game.players[0].alive;
        for _ in 0..24 {
            step_as(&mut session, "jack");
        }
        let mut struck: Vec<String> = Vec::new();
        let kinds = [
            "pandemic",
            "radiation_leak",
            "leader_collapse",
            "microstates",
            "nuclear_war",
            "microstates",
            "nuclear_war",
        ];
        for (at, kind) in kinds.iter().enumerate() {
            if !on(&session) {
                break;
            }
            let answer = session.debug(
                &json!({"op": "black_swan", "kind": kind, "seed": seed + at as u64, "lean": true}),
            );
            // One this world has nothing for is refused, and strikes nothing.
            match answer["error"].as_str() {
                None => struck.push(kind.to_string()),
                Some(why) => assert!(
                    why.starts_with("no civilization") || why.starts_with("a nuclear war"),
                    "{kind} on seed {seed}: {why}"
                ),
            }
            let ahead = session
                .debug(&json!({"op": "ahead", "turns": 4, "strategy": "jack", "lean": true}));
            assert_eq!(ahead["error"], Value::Null, "{kind} on seed {seed}");
            for _ in 0..6 {
                if on(&session) {
                    step_as(&mut session, "jack");
                }
            }
        }
        assert!(
            struck.len() >= 5,
            "seed {seed} was struck by {struck:?} only"
        );
        assert!(struck.contains(&"nuclear_war".to_string()), "{struck:?}");
        assert_eq!(session.game.black_swans.len(), struck.len());
        // The world is whole: everything in it belongs to a seat there is,
        // it saves, and the page is given its state.
        let game = &session.game;
        assert!(game
            .cities
            .values()
            .all(|city| city.owner < game.players.len() && city.pop >= 1));
        assert!(game
            .units
            .values()
            .all(|unit| unit.owner < game.players.len()));
        assert!(!world(&session).is_empty());
        let state = session.state();
        assert_eq!(
            state["black_swans"].as_array().map(Vec::len),
            Some(struck.len())
        );
        assert_eq!(
            state["debug"]["swans"].as_array().map(Vec::len),
            Some(struck.len())
        );
    }
}

/// `play` is a run of steps asked for once: the same turns, the same marks to
/// walk back to, and how the seat stood as each turn opened. It stops where a
/// run stops.
#[test]
fn play_is_a_run_of_steps_and_reads_the_seat_each_turn() {
    let (mut stepped, mut run) = (played(), played());
    for _ in 0..5 {
        step_as(&mut stepped, "jack");
    }
    let answer = run.debug(&json!({"op": "play", "turns": 5, "strategy": "jack", "lean": true}));
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(world(&stepped), world(&run));
    assert_eq!(marked(&stepped), marked(&run));
    assert_eq!(answer["turn"], json!(6));
    let readings = answer["readings"].as_array().expect("a reading a turn");
    assert_eq!(readings.len(), 5);
    for (at, reading) in readings.iter().enumerate() {
        assert_eq!(reading["turn"], json!(at as u64 + 2));
    }
    // The last of them is how the seat stands now.
    assert_eq!(readings[4], answer["seat"]);
    // The whole document carries them as well when it is the answer.
    let whole = run.debug(&json!({"op": "play", "turns": 1, "strategy": "jack"}));
    assert_eq!(whole["error"], Value::Null);
    assert_eq!(whole["readings"].as_array().map(Vec::len), Some(1));
    assert!(whole.get("map").is_some());

    // A breakpoint stops it where it stops a run of steps.
    let set = run.debug(&json!({"op": "breakpoints", "turns": [9]}));
    assert_eq!(set["error"], Value::Null);
    let stopped = run.debug(&json!({"op": "play", "turns": 5, "strategy": "jack", "lean": true}));
    assert_eq!(stopped["error"], Value::Null);
    assert_eq!(stopped["turn"], json!(9));
    assert_eq!(stopped["readings"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        stopped["debug"]["stopped"],
        json!({"kind": "turn", "turn": 9})
    );

    // Asked for no turns, or for more than one request plays, it is refused
    // and nothing is played.
    for turns in [json!(null), json!(0), json!(101), json!("5")] {
        let refused = run.debug(&json!({"op": "play", "turns": turns, "lean": true}));
        assert!(
            refused["error"]
                .as_str()
                .unwrap_or("")
                .starts_with("play needs turns"),
            "{turns}: {}",
            refused["error"]
        );
        assert_eq!(refused["turn"], json!(9));
        assert!(refused.get("readings").is_none());
    }
}

/// What a search does with a fork (`docs/SIM_DEBUGGER.md`, "Autosearch"): it
/// plays one schedule of Civ AIs, walks back to a turn in the middle of it
/// and plays another from there. That has to be the second schedule played
/// straight through, or what a search found is not what playing it gives.
///
/// On the game itself a walk back seats the agents kept with the turn. A
/// half of a split keeps none, so the copy a search is run on is asked to
/// keep them from the turn the search begins on (`remember`), and the half
/// on the page, which is not, plays the schedule straight from that turn.
#[test]
fn a_schedule_walked_back_to_is_the_schedule_played_straight() {
    let play = |session: &mut Session, turns: u32, agent: &str| {
        let answer =
            session.debug(&json!({"op": "play", "turns": turns, "strategy": agent, "lean": true}));
        assert_eq!(answer["error"], Value::Null);
    };
    let walk_to = |session: &mut Session, turn: u32, agent: &str| {
        let answer =
            session.debug(&json!({"op": "jump", "turn": turn, "strategy": agent, "lean": true}));
        assert_eq!(answer["error"], Value::Null);
        assert_eq!(answer["turn"], json!(turn));
    };

    // The game itself: jack for three turns and siege for three, straight.
    let mut straight = played();
    play(&mut straight, 3, "jack");
    play(&mut straight, 3, "siege");
    let mut one = played();
    play(&mut one, 6, "jack");
    let all_jack = world(&one);
    // Walked back to the turn siege takes over on, from a line jack played.
    walk_to(&mut one, 4, "siege");
    play(&mut one, 3, "siege");
    assert_eq!(world(&one), world(&straight));
    // And back to jack's own line from the middle of it: its kept agent
    // sits down again and plays the turns it played.
    walk_to(&mut one, 4, "jack");
    play(&mut one, 3, "jack");
    assert_eq!(world(&one), all_jack);
    // The game keeps its agents already, and is asked nothing.
    let kept = one
        .debugger
        .marks
        .iter()
        .filter(|mark| mark.minds.is_some())
        .count();
    assert!(kept > 0);
    assert_eq!(
        one.debug(&json!({"op": "remember", "lean": true}))["error"],
        Value::Null
    );
    assert_eq!(
        one.debugger
            .marks
            .iter()
            .filter(|mark| mark.minds.is_some())
            .count(),
        kept
    );

    // A half of a split, three turns into a game: the page walks it to the
    // turn the search began on and plays the schedule straight from there.
    let mut host = played();
    play(&mut host, 2, "jack");
    let mut paged = host.fork();
    walk_to(&mut paged, 3, "jack");
    play(&mut paged, 3, "jack");
    play(&mut paged, 3, "siege");
    let mut whole = host.fork();
    walk_to(&mut whole, 3, "jack");
    play(&mut whole, 6, "jack");
    // The copy that is searched keeps its agents from that turn on, and none
    // with the turns it was handed.
    let mut searched = host.fork();
    assert!(searched.debugger.forgetful);
    assert!(searched
        .debugger
        .marks
        .iter()
        .any(|mark| mark.minds.is_some()));
    let asked = searched.debug(&json!({"op": "remember", "lean": true}));
    assert_eq!(asked["error"], Value::Null);
    assert!(!searched.debugger.forgetful);
    assert!(searched
        .debugger
        .marks
        .iter()
        .all(|mark| mark.minds.is_none()));
    walk_to(&mut searched, 3, "jack");
    play(&mut searched, 6, "jack");
    assert_eq!(world(&searched), world(&whole));
    assert!(searched
        .debugger
        .marks
        .iter()
        .filter(|mark| mark.turn > 3)
        .all(|mark| mark.minds.is_some()));
    walk_to(&mut searched, 6, "siege");
    play(&mut searched, 3, "siege");
    assert_eq!(world(&searched), world(&paged));
    walk_to(&mut searched, 6, "jack");
    play(&mut searched, 3, "jack");
    assert_eq!(world(&searched), world(&whole));
}

/// Operator 2026-11-22: "allow a reasonable objective function to be chosen.
/// score or prod or city or fpt or gpt"; and 2026-11-23, four lists in place
/// of the one number.
///
/// What a term of the four lists reads is a key of how the engine says the
/// seat stands (`obs::standing`, reached into with a dot), or `won` and
/// `lost`. Every key the page offers is one the engine reads a number for,
/// put to it as an objective is (`autosearch::Objective::check`), the
/// operator's five come first, and the page asks its copies and the climb in
/// operations the engine has.
#[test]
fn every_goal_autosearch_offers_reads_a_number_the_engine_gives() {
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let listed = script
        .split_once("const DEBUG_SEARCH_KEYS = [")
        .and_then(|(_, rest)| rest.split_once("];"))
        .expect("the keys are listed")
        .0;
    let keys: Vec<&str> = listed
        .split("[\"")
        .skip(1)
        .map(|row| row.split_once('"').expect("a key").0)
        .collect();
    assert!(keys.len() >= 5, "{keys:?}");
    assert_eq!(
        &keys[..5],
        [
            "score",
            "yields.production",
            "cities",
            "yields.faith",
            "gold_per_turn"
        ]
    );
    let mut session = played();
    for _ in 0..3 {
        step_as(&mut session, "jack");
    }
    for key in &keys {
        let set = session.debug(&json!({"op": "objective", "lean": true,
            "objective": {"want": [{"key": key}]}}));
        assert_eq!(set["error"], Value::Null, "{key}");
    }
    let played = session.debug(&json!({"op": "play", "turns": 1, "lean": true}));
    assert!(played["terms"][0]["want"][0].is_number(), "{played}");
    // A copy is made ready, walked back and played with these.
    for piece in [
        r#"debugSearchPost("/forks", {count: search.cores})"#,
        r#"debugSearchAsk(search, fork, {op: "breakpoints"})"#,
        r#"debugSearchAsk(search, fork, {op: "objective", objective: search.objective})"#,
        r#"debugSearchPost("/autosearch", {op: "begin", objective, from: search.from, end: search.end,"#,
        r#"debugSearchClimb(search, {op: "next", holding: fork.plan, at: fork.at})"#,
        r#"debugSearchClimb(search, {op: "took", plan, series: fork.series, last: fork.at})"#,
        r#"if (search.pane) refused(await debugSearchAsk(search, fork, {op: "remember"}));"#,
        r#"{op: "jump", turn, strategy: debugPlanWho(plan, turn)}"#,
        r#"{op: "play", turns: asked, strategy: debugPlanWho(plan, fork.at)}"#,
        // And the line that was found is played from the same turn the same way.
        r#"const there = await debugOp({op: "jump", turn: plan.from, strategy: stretch.name});"#,
    ] {
        assert!(script.contains(piece), "{piece}");
    }
    // The button is a size larger than the tools beside it (operator: "make
    // it a bit bigger than the others"); `beta/debugger_test.py` measures it.
    assert!(script.contains("#debugbar button.dbg-search-open {\n    height: 32px;"));
    assert!(script.contains("min-width: 26px; height: 26px; padding: 0 6px;"));
}

#[test]
fn both_front_ends_serve_the_debugger() {
    assert!(include_str!("../wasm.rs").contains("(\"POST\", \"/debug\")"));
    assert!(include_str!("../server.rs").contains("(\"POST\", \"/debug\")"));
    assert!(include_str!("../../beta/shim.js").contains("\"/debug\""));
}

#[test]
fn the_page_loads_the_bar_and_sends_only_operations_the_engine_answers() {
    let page = super::super::EMBEDDED_INDEX_HTML;
    let door = page.find("/assets/app_frontdoor.js").unwrap();
    let bar = page.find("/assets/app_debugger.js").unwrap();
    assert!(door < bar, "the bar reads what every other script declares");
    let served = String::from_utf8(super::super::index_with_app_instance(page, 7)).unwrap();
    assert!(served.contains(r#"<script src="/assets/app_debugger.js?instance=7"></script>"#));
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    assert!(script.starts_with("\"use strict\";"));

    // Read the operations out of the script and put each to the engine, so
    // one renamed on either side fails here and not in somebody's browser.
    // An operation of Autosearch's climb goes to `POST /autosearch`
    // (`autosearch::route`), and is put to that instead.
    let mut session = played();
    let mut climb = None;
    super::super::autosearch::route(
        &mut climb,
        &json!({"op": "begin", "from": 1, "turns": 5, "letters": ["jack"], "seated": "jack"}),
    );
    let mut rest = script;
    let mut sent = BTreeSet::new();
    while let Some(at) = rest.find("op: \"") {
        rest = &rest[at + 5..];
        let op = &rest[..rest.find('"').expect("a closed string")];
        let answer = session.debug(&json!({"op": op}));
        let mut refused = answer["error"].as_str().unwrap_or("").to_string();
        if refused.starts_with("no debugger operation") {
            let climbed = super::super::autosearch::route(&mut climb, &json!({"op": op}));
            refused = climbed["error"].as_str().unwrap_or("").to_string();
            assert!(
                !refused.starts_with("no autosearch operation"),
                "{op}: answered by neither the debugger nor the climb"
            );
        }
        sent.insert(op.to_string());
    }
    for op in [
        "back",
        "forward",
        "jump",
        "undo",
        "step",
        "step_action",
        "step_seat",
        "finish_round",
        "reveal",
        "assist",
        "layers",
        "breakpoints",
        "save_point",
        "goto_save",
        "drop_save",
        "room",
        "branch",
    ] {
        assert!(sent.contains(op), "the bar never sends {op}");
    }
}

/// Letting go of a slider reads where it was dragged to before anything
/// draws the bar again, because drawing puts a slider that is not being
/// dragged back on the open turn. Read after, every drag went nowhere.
#[test]
fn a_dragged_slider_is_read_before_the_bar_is_drawn_again() {
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let heard = script
        .find("bar.addEventListener(\"change\"")
        .expect("the bar hears a change");
    let handler = &script[heard..];
    let read = handler
        .find("const turn = target.value;")
        .expect("the slider is read");
    let drawn = handler.find("debugSay(\"\")").expect("the note is cleared");
    assert!(read < drawn, "the slider is read after the bar was redrawn");
    // The world's own slider, and the two lanes the host of a split shows.
    for slider in ["target.id === \"dbg-slider\"", "target.dataset.dbgHalf"] {
        assert!(handler[..read].contains(slider), "{slider}");
    }
}

/// The header box is fixed at the top of the window beside the left panel
/// (operator 2026-11-19: "left panel should go to the top of the screen.
/// debugger header box should start from the right side of the left panel"),
/// and the map area stands under it by its height. Three things the page
/// fixes to the window at a distance from its top do not move with the map
/// area, so the box's own stylesheet moves them, by name. A name that is gone
/// from the page would leave that rule moving nothing, and the thing it was
/// for under the box.
#[test]
fn the_docked_box_stands_beside_the_left_panel_and_moves_what_is_fixed_under_it() {
    let page = super::super::EMBEDDED_INDEX_HTML;
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    assert!(
        script.contains("position: fixed; z-index: 8000; inset: 0 0 auto var(--dbg-left, 0px);")
    );
    assert!(script.contains("html.dbg-docked #maparea { margin-top: var(--dbg-top, 0px); }"));
    assert!(
        !script.contains("html.dbg-docked body {"),
        "the body is padded again, and the left panel is under the box"
    );
    assert!(page.contains("<div id=\"maparea\">"));
    for (rule, moved) in [
        ("#moments {", "html.dbg-docked #moments"),
        ("#site-pick {", "html.dbg-docked #site-pick"),
        ("#side { position: fixed;", "html.dbg-docked #side"),
    ] {
        let at = page
            .find(rule)
            .unwrap_or_else(|| panic!("the page has no {rule}"));
        let body = &page[at..at + page[at..].find('}').expect("a closed rule")];
        assert!(body.contains("position: fixed"), "{rule} is not fixed");
        assert!(script.contains(moved), "nothing moves {rule} under the box");
    }
    // A split stands under the box, not over it, and its box has the whole
    // width: the two halves cover the left panel.
    assert!(script.contains("position: fixed; inset: var(--dbg-top, 0px) 0 0 0;"));
    assert!(script.contains("html.dbg-split-open #debugbar { z-index: 100001; left: 0; }"));
    // Auto-play moved into the box, and the left panel's copy is put away.
    assert!(page.contains("<div class=\"footer-actions\" id=\"humanfooter\">"));
    assert!(script.contains("html.dbg-docked #humanfooter { display: none !important; }"));
}

/// The header box as the operator laid it out on 2026-11-19: the timeline on
/// top, twice its old height; under it "Turn 1 | Era Start/Back | 10 turns
/// back | 1 turn back | next turn | 10 turns" with the turn counter between
/// one back and one on; and at the bottom left "civ ai, turns, auto-play",
/// layers, breakpoints, and to their right "Branch Timeline". Auto-play
/// counts the turns it has played while it plays. On 2026-11-21 the turn
/// buttons became "Turn 1, Era Back (... the same symbol as 10 back button
/// except swap 10 for the word era), 10 Back, 1 back, 1 next, 10 next, era
/// next, Play to End".
///
/// Later that day the Civ AI picker and the branch button left the bottom
/// row: "move 1202 ffor... box, from >>| arrow above the timeline. branch
/// should say "Branch" and should move around below where the timeline
/// "cursor" is. always rpesenting the option to branch" (the box that read
/// "1202 - ForwardSettle" is the Civ AI picker). The picker is the first
/// thing in the box, over the timeline, and Branch is part of the timeline's
/// marker, drawn under it. Where each stands in a browser is held by
/// `beta/debugger_test.py` (`check_the_bar`).
#[test]
fn the_header_box_has_the_timeline_on_top_and_its_controls_in_the_operators_order() {
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let built = &script[script.find("bar.innerHTML = `").expect("the box is built")..];
    let mut from = 0;
    for piece in [
        "<div class=\"dbg-time\">",
        "<div class=\"dbg-who\">",
        "id=\"dbg-agent\"",
        "debugLaneMarkup(\"\", \"dbg-slider\", \"Turn, on the whole game\")",
        "<div class=\"dbg-deck\">",
        "<select id=\"dbg-turns\"",
        "<button data-dbg=\"autoplay\"",
        "data-dbg-pop=\"layers\"",
        "data-dbg-pop=\"breaks\"",
        "<span class=\"dbg-word\">Turn 1</span>",
        "aria-label=\"Era Back\"><i>◀◀</i>Era</button>",
        "<button data-dbg=\"back10\"",
        "<button data-dbg=\"back\"",
        "id=\"dbg-turn\"",
        "<button data-dbg=\"next\"",
        "<button data-dbg=\"next10\"",
        "aria-label=\"Era Next\">Era<i>▶▶</i></button>",
        "<span class=\"dbg-word\">Play to End</span>",
    ] {
        let at = built[from..]
            .find(piece)
            .unwrap_or_else(|| panic!("{piece} is missing, or out of its order"));
        from += at + piece.len();
    }
    for piece in [
        // Twice the 34px the rail was. The rail is where the slider is, and
        // the sky is the rail as it is seen.
        "#debugbar .dbg-rail { position: relative; flex: 1; min-width: 0; height: 68px; border: 1px solid transparent; }",
        "position: absolute; inset: -1px; border-radius: 12px; pointer-events: none;",
        // What Auto-play offers, and Era is what it is set to when the box
        // is built (operator, 2026-11-21: "strat debugger auto play can
        // start on Era. have options 1, 3, 5, 10, 20, Era, 100, Indefinite").
        // Era plays until a new era begins, and Indefinite until it is
        // stopped.
        "  [\"1\", \"1 turn\"], [\"3\", \"3 turns\"], [\"5\", \"5 turns\"], [\"10\", \"10 turns\"], [\"20\", \"20 turns\"],\n  \
         [\"era\", \"Era\"], [\"100\", \"100 turns\"], [\"indefinite\", \"Indefinite\"]];",
        "const DEBUG_AUTOPLAY_PICK = \"era\";",
        "`<option value=\"${pick}\"${pick === DEBUG_AUTOPLAY_PICK ? \" selected\" : \"\"}>${word}</option>`).join(\"\")\n          }</select>",
        "if (pick === \"era\") { debugRun(null, state.world_era ?? 0); return; }",
        "debugRun(pick === \"indefinite\" ? null : state.turn + Math.max(1, parseInt(pick, 10) || 1));",
        // The count on Auto-play, of the turns asked for or so far.
        "const runCount = !deck.running ? \"\" : deck.asked === null ? `${deck.played}` : `${deck.played}/${deck.asked}`;",
        // Era Back, Era Next, the era breakpoint and the eras named on the
        // timeline. Era Next played on stops on the turn a new era opens.
        "const began = (timeline.eras || []).filter(era => era.turn < state.turn);",
        "return (timeline.eras || []).find(era => era.turn > state.turn) || null;",
        "if (await debugWalkOn(null)) await debugRun(null, state.world_era ?? 0);",
        "else if (dawn(next)) debugSayStop({kind: \"era\", turn: next.turn, era: next.world_era});",
        // Play to End walks the turns played, then plays the game out.
        "if (await debugWalkOn(null)) await debugRun(null);",
        "<input type=\"checkbox\" data-dbg-field=\"eras\"> A new era begins</label>",
        "eras: pop.querySelector(\"[data-dbg-field=eras]\").checked,",
        "eras: timeline.eras || [], goal: debugRunning ? debugGoal : null, idle: deck.idle});",
        // Branch is in the world's marker, so it is wherever the marker is,
        // and hangs under the rail. It says its name in a box of any width.
        "<div class=\"dbg-cursor\"><b></b>${key ? \"\" : `\n            <button class=\"dbg-branch\" data-dbg=\"split\"",
        "aria-label=\"Branch the timeline on this turn\" aria-haspopup=\"dialog\"><i>⑂</i>Branch</button>`}</div></div>",
        "position: absolute; left: 0; top: calc(100% + 6px); transform: translateX(-50%); pointer-events: auto;",
        "#debugbar:not(.dbg-split-on) .dbg-lane[data-dbg-lane=\"\"] { margin-bottom: 26px; }",
    ] {
        assert!(script.contains(piece), "the header box is missing {piece}");
    }
    // The picker and Branch are no longer among the tools under the timeline.
    let deck = &built[built
        .find("<div class=\"dbg-deck\">")
        .expect("the deck is built")..];
    let deck = &deck[..deck
        .find("<div class=\"dbg-hover\"")
        .expect("the deck ends where what is said under the pointer begins")];
    for gone in ["dbg-agent", "data-dbg=\"split\"", "Branch Timeline"] {
        assert!(!deck.contains(gone), "{gone} is still under the timeline");
    }
}

#[test]
fn a_fork_is_the_same_world_with_the_same_past_and_a_future_of_its_own() {
    let mut session = played();
    end_turn(&mut session);
    end_turn(&mut session);
    let mut fork = session.fork();
    assert_eq!(world(&fork), world(&session));
    assert_eq!(marked(&fork), [1, 2, 3]);

    end_turn(&mut fork);
    assert_eq!(fork.game.turn, 4);
    assert_eq!(fork.debug_jump(1), Ok(1));
    // Nothing the fork does reaches the session it was taken from.
    assert_eq!(session.game.turn, 3);
    assert_eq!(marked(&session), [1, 2, 3]);
    end_turn(&mut session);
    assert_eq!(marked(&session), [1, 2, 3, 4]);
    assert_eq!(marked(&fork), [1, 2, 3, 4], "the fork kept its own turn 4");
    assert_eq!(fork.game.turn, 1);
}

#[test]
fn the_split_speaks_one_set_of_messages_on_both_sides() {
    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let shim = include_str!("../../beta/shim.js");
    // On the published build a half has no engine: its shim asks the host's.
    // The page script takes no part in that, and each side of it is the shim.
    for message in ["pane-ask", "pane-answer"] {
        let quoted = format!("\"{message}\"");
        assert!(
            shim.matches(&quoted).count() >= 2,
            "the shim sends or hears {message}, not both"
        );
        assert!(!page.contains(&quoted), "{message} is the shim's");
    }
    // Everything else passes between the host and a pane inside the page
    // script: each message has a sender and a listener there.
    for message in [
        "pane-turn",
        "pane-follow",
        "pane-copy",
        "pane-camera",
        "pane-look",
        // Head to head: each half's numbers and Civ AI, and playing both.
        "pane-numbers",
        "pane-strategy",
        "pane-run",
        "pane-stop",
        "pane-ran",
        // The lock: whether the halves are locked and who is whose twin,
        // what a half did that left its turn where it was, and the one
        // panel of a locked split (a half's deck, and what is pressed or
        // changed on the host's).
        "pane-lock",
        "pane-did",
        "pane-op",
        "pane-deck",
        "pane-press",
        "pane-change",
        "pane-solo",
        "pane-go",
        // What stands on a half's timeline, for its lane in the host's bar:
        // its save points and its breakpoint turns.
        "pane-marks",
        // One screen becomes two in one move: a half that is not yet shown
        // says when it has painted itself, and is told when it is seen.
        "pane-shown",
        "pane-seen",
        // Autosearch: asked for on a half's own panel, a line that was
        // found for a half to play, each Civ AI of it as the half seats
        // it, and how the line came out.
        "pane-search",
        "pane-plan",
        "pane-seated",
        "pane-planned",
    ] {
        let quoted = format!("\"{message}\"");
        assert!(
            page.matches(&quoted).count() >= 2,
            "{message} is sent or heard, not both"
        );
    }
    // The split closes and copies through the engine, and the engine names
    // those two operations as a split's when no half is named.
    let mut session = played();
    for op in ["adopt", "copy"] {
        assert!(page.contains(&format!("op: \"{op}\"")), "{op}");
        let refused = session.debug(&json!({"op": op}));
        assert!(refused["error"].as_str().unwrap().contains("split"), "{op}");
    }
}

/// Operator 2026-11-20: "2 linked universes should have a lock connecting
/// timelines to show they are one. clicking on the lock can unlock. can only
/// lock if on the same turn. locking keeps them on the same turn. replacing
/// linking with locking. default open view for the multiverse is 2 window
/// views side by side. don't ask when button is clicked initially. just
/// open. if 2 games are identical, we should also not have to simulate both
/// (only if they are entirely identical). we can just simulate one. if two
/// windows are locked, the 2 top control panels should merge into 1 (as they
/// will be the same)".
///
/// What each of those does in a browser is held by `beta/debugger_test.py`
/// (`check_the_lock`); this holds the pieces to the page.
#[test]
fn a_split_opens_at_once_and_locked_with_a_lock_between_its_lanes_and_one_panel() {
    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    for piece in [
        // Branch is a button, and how the two are laid out is not asked:
        // side by side. What it asks is the one thing a branch is made
        // for, the Civ AI that plays it (operator, 2026-11-24: "they
        // should choose the new strategy when they branch"), and the
        // split opens when one is picked.
        "<button class=\"dbg-branch\" data-dbg=\"split\"",
        "debugAsk = debugAsk && debugAsk.kind === \"branch\" ? null : {kind: \"branch\"};",
        "debugSplitOpen(\"row\", null, ask.kind === \"branch\" ? button.value : ask.name);",
        // The lock stands in the lanes, shut as the split opens.
        "<div class=\"dbg-tie\"><button class=\"dbg-lock\" data-dbg-lock aria-pressed=\"true\">",
        "debugSplit = {dir, locked: true, settling: true,",
        "if (debugSplit) debugSplitLock(!debugSplit.locked);",
        // It closes only on two halves that stand on the same turn.
        "if (on && (a.turn === null || a.turn !== b.turn)) return;",
        "lock.disabled = debugSplit.busy || running || (!debugSplit.locked && !together);",
        // Locked, a half that moves is followed, and so is what it did
        // that left its turn where it was.
        "if (debugSplit.locked && data.how !== \"follow\" && data.how !== \"boot\")",
        "else debugSplitTell(other, {civvis: \"pane-op\", body: debugMirrored(data.body)});",
        // Locked, the host's deck is the one panel and each half's is away.
        "return debugSplit && debugSplit.locked ? debugSplit.panes.a.deck || null : null;",
        "#debugbar.dbg-split-on:not(.dbg-merged) .dbg-deck, #debugbar.dbg-split-on:not(.dbg-merged) .dbg-pop { display: none; }",
        "html.dbg-pane-locked #debugbar, html.dbg-pane-locked #humanfooter { display: none !important; }",
        "if (debugSplit) { debugRemote(\"pane-press\", event.target); return; }",
        // Two locked halves that seat the same Civ AI name each other as
        // twins, for the engine to play once if they are the same world.
        "const twins = debugSplit.locked && a.strategy !== undefined && a.strategy === b.strategy;",
        "body: JSON.stringify(led && debugTwin ? {...body, twin: debugTwin} : body),",
        "debugTwinned = led && !!(next.debug && next.debug.twinned);",
        "how: data.twinned ? \"twin\" : data.how});",
    ] {
        assert!(page.contains(piece), "the split is missing {piece}");
    }
    // Linking, and the panel that asked how to split, are gone.
    for gone in [
        "dbg-pop-split",
        "data-dbg-split=\"link\"",
        "debugSplit.linked",
        "split-row",
        "split-col",
        "split-versus",
    ] {
        assert!(!page.contains(gone), "{gone} is still on the page");
    }
}

/// The print of each marked turn's world, as the state document gives them.
fn prints(session: &Session) -> Vec<u64> {
    let state = session.state();
    let worlds: Vec<u64> = state["debug"]["worlds"]
        .as_array()
        .expect("the state gives a print for each marked turn")
        .iter()
        .map(|world| world.as_u64().expect("a print is a whole number"))
        .collect();
    assert_eq!(worlds.len(), marked(session).len(), "one print a turn");
    worlds
}

/// Operator 2026-11-26: "please take more of a doctor strange's civ 6
/// approach. branches should split from 1 to 2... not 2 to 1". A split's
/// timeline forks on the turn it was branched on, from the moment it is
/// made, and earlier only where the two worlds part earlier: one half walked
/// back behind that turn and played on another way. (From 2026-11-21 it
/// stayed one timeline for as long as the two held the same world.)
///
/// Which turns are the same world in two sessions is read off the print
/// every mark carries (`world_print`). What the two halves of a split say of
/// theirs is held by `panes_tests.rs`
/// (`halves_say_up_to_which_turn_they_hold_the_same_world`), and what the
/// timeline then looks like in a browser by `beta/debugger_test.py`
/// (`branched`). This holds what a print is, and the pieces to the page.
#[test]
fn a_branched_timeline_forks_on_the_turn_it_was_branched_on() {
    // A print is a turn's own, and carries the turns before it.
    let mut session = played();
    for _ in 0..3 {
        assert!(session.debug_step());
    }
    let line = prints(&session);
    assert_eq!(marked(&session), [1, 2, 3, 4]);
    assert_eq!(
        line.iter().collect::<BTreeSet<_>>().len(),
        4,
        "every turn is a world of its own"
    );
    assert!(
        line.iter().all(|world| *world < 1 << 53),
        "a print is a number a page holds exactly"
    );

    // Two more sessions on the same world give the same prints, and go on
    // giving the same ones for as long as they are played the same way.
    // (They are compared with each other and not with the session they were
    // taken from: a fork's agents have just sat down, `Session::fork`.)
    let mut one = session.fork();
    let mut two = session.fork();
    let mut other = session.fork();
    assert_eq!(prints(&one), line);
    assert!(one.debug_step());
    assert!(two.debug_step());
    assert_eq!(world(&one), world(&two));
    assert_eq!(prints(&one), prints(&two));

    // One that is played another way gives another print from the turn that
    // opens on another world, and for every turn after it: here the seat
    // does one thing by hand and ends the turn, where an agent played it.
    let moved = something_to_do(&other);
    assert_eq!(other.act(&moved), None);
    end_turn(&mut other);
    assert_ne!(world(&other), world(&one));
    let apart = prints(&other);
    assert_eq!(apart[..4], line[..]);
    assert_ne!(apart[4], prints(&one)[4]);
    assert!(one.debug_step());
    assert!(other.debug_step());
    assert_ne!(prints(&other)[5], prints(&one)[5]);
    assert_eq!(prints(&other)[..4], line[..]);

    // Walking a timeline changes no print on it.
    let held = prints(&session);
    assert_eq!(session.debug_jump(2), Ok(2));
    assert_eq!(prints(&session), held);

    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    for piece in [
        // A half tells the host the turns it holds and the print of each.
        "const turns = timeline.turns || [], worlds = timeline.worlds || null;",
        "window.parent.postMessage({civvis: \"pane-marks\", pane: DEBUG_PANE, flags, stops, beyond, eras, turns, worlds, by, swans, seat},",
        // The host reads off the two lists the last turn both give the same
        // print for, and the timeline forks there or on the turn of the
        // split, whichever is first, from the moment it is made
        // (`a_branch_begins_on_the_turn_it_is_made_on`).
        "function debugSplitFork() {",
        "if (theirs.get(lines.a.turns[at]) !== lines.a.worlds[at]) continue;",
        "return {turn: Math.min(shared, from), forked: true, first, last};",
        // The timeline is drawn from that: the trunk up to the turn, a rail
        // for each half from it, and one marker for two halves on one turn
        // of the trunk.
        "bar.classList.toggle(\"dbg-forked\", fork.forked);",
        "bar.style.setProperty(\"--dbg-fork\", along);",
        "const one = stands.a === stands.b && onTrunk(stands.a);",
        "goal, idle: false, marker: one ? stands.a : null, branches: fork.forked ? fork.turn : null});",
        "#debugbar.dbg-forked .dbg-lane[data-dbg-lane=\"\"] .dbg-deco { clip-path: inset(0 calc(100% * (1 - var(--dbg-fork, 1))) 0 0); }",
        "#debugbar.dbg-split-on .dbg-half .dbg-deco { clip-path: inset(0 0 0 calc(100% * var(--dbg-fork, 1))); }",
        "#debugbar:not(.dbg-forked) .dbg-half .dbg-sky { background: none; border-color: transparent; box-shadow: none; }",
        // The trunk is drawn and not pressed: each half keeps a slider the
        // length of the game.
        "#debugbar.dbg-split-on .dbg-lane[data-dbg-lane=\"\"] .dbg-rail { height: var(--dbg-row); align-self: center; pointer-events: none; }",
    ] {
        assert!(page.contains(piece), "the branching timeline is missing {piece}");
    }
    // The turn the screen was split on no longer says where the lanes part.
    assert!(!page.contains("fork: debugSplit.from"));
}

/// Operator, 2026-11-22: "the branching should form more of a distinct y
/// shape. or like 4 or h like (the one can be between the 2). but more of a
/// tree like timeline please". The page draws a split's timeline as a tree
/// on its side: the trunk is a rail in the middle, between the two halves'
/// rows, and from the turn the two worlds differ a branch leaves it for each
/// half, up to A's row and down to B's, with the box's dark between the two.
/// `beta/debugger_test.py` measures the shape in a browser; this holds the
/// page to the pieces it is made of.
#[test]
fn a_branched_timeline_is_drawn_as_a_tree() {
    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    for piece in [
        // The tree's measures, in one place for the style and the script:
        // how tall a rail is, the dark between the two branches, how far
        // along a branch is in its own row, and how far up or down that is
        // (half a row and half the gap).
        "const DEBUG_TREE = {row: 28, gap: 24, run: 32, rise: 26, tilt: Math.atan2(26, 32).toFixed(4)};",
        "--dbg-row: ${DEBUG_TREE.row}px; --dbg-gap: ${DEBUG_TREE.gap}px; --dbg-run: ${DEBUG_TREE.run}px; --dbg-rise: ${DEBUG_TREE.rise}px;",
        "--dbg-at: calc((100% - 36px) * var(--dbg-fork, 1) + 18px);",
        // Two rows with the dark between them, and the trunk in the middle.
        "column-gap: 8px; row-gap: var(--dbg-gap); align-items: center;",
        "#debugbar.dbg-split-on .dbg-lane[data-dbg-lane=\"\"] .dbg-rail { height: var(--dbg-row); align-self: center; pointer-events: none; }",
        // Until the worlds differ a half's lane is drawn where the trunk is.
        "#debugbar.dbg-split-on:not(.dbg-forked) .dbg-lane[data-dbg-lane=\"a\"] :is(.dbg-sky, .dbg-deco) { transform: translateY(var(--dbg-rise)); }",
        "#debugbar.dbg-split-on:not(.dbg-forked) .dbg-lane[data-dbg-lane=\"b\"] :is(.dbg-sky, .dbg-deco) { transform: translateY(calc(-1 * var(--dbg-rise))); }",
        // From the fork a half's own rail begins where its branch is in its
        // row, and the branch on its way there is the rail over again,
        // slanted from the trunk: A's up, B's down, each cut along the
        // middle of the tree.
        "#debugbar.dbg-forked .dbg-half .dbg-sky { left: calc(var(--dbg-at) + var(--dbg-run)); border-left: 0; border-radius: 0 12px 12px 0; }",
        "#debugbar.dbg-forked .dbg-half .dbg-deco { clip-path: inset(0 0 0 calc(100% * var(--dbg-fork, 1) + var(--dbg-run))); }",
        "key ? `<div class=\"dbg-arm\"><div class=\"dbg-arm-sky\"></div><div class=\"dbg-bent\"></div></div>` : `<div class=\"dbg-axis\"></div>`}",
        "#debugbar.dbg-forked .dbg-arm { display: block; }",
        "transform: skewY(-${DEBUG_TREE.tilt}rad);",
        "transform: skewY(${DEBUG_TREE.tilt}rad);",
        "calc(var(--dbg-at) + var(--dbg-run) + 1px) calc(50% + var(--dbg-rise)), calc(var(--dbg-at) - 1px) 50%);",
        "if (lane.bent) debugDrawn(lane.bent, half && tree.forked ? lane.deco.innerHTML.replace(",
        // The three lanes are told they are one tree, and what a half's
        // lane holds on the trunk, or on a branch still on its way to the
        // half's row, is drawn where the tree is.
        "const tree = {forked: fork.forked, turn: fork.turn, lean: debugSplit.locked ? 0 : 5};",
        "function debugOffRow(tree, turn, px, fork, travel) {",
        "return by ? `;transform:translateY(${key === \"a\" ? by : -by}px)` : \"\";",
        // A half's marker keeps to its own row, with a pin to the tree.
        "lane.cursor.classList.toggle(\"dbg-pinned\", off(shown) > 0);",
        "#debugbar .dbg-half .dbg-cursor.dbg-pinned::before {",
        // Each half's slider is pressed from its row to the middle of the tree.
        "#debugbar.dbg-split-on .dbg-half .dbg-rail input { height: calc(100% + var(--dbg-gap) / 2 + 1px); }",
        // The turns are numbered once for the whole tree, beside its rails.
        "function debugAxisMarkup(lo, hi, tree, at, travel) {",
        "#debugbar.dbg-split-on .dbg-ruler b { display: none; }",
        "if (lane.axis) debugDrawn(lane.axis, tree ? debugAxisMarkup(lo, hi, tree, at, travel) : \"\");",
        // The turn it forks on is said between the two branches.
        "position: absolute; z-index: 2; top: 50%; transform: translate(calc(var(--dbg-run) + 3px), -50%); padding: 2px 7px; border-radius: 999px;",
        // The lock stands at the root of the tree, and the trunk leaves it:
        // nothing of either branch is left of the trunk (operator,
        // 2026-11-26: "branches should split from 1 to 2... not 2 to 1").
        "display: block; grid-column: 1; grid-row: 1 / span 2; align-self: stretch; position: relative;",
    ] {
        assert!(page.contains(piece), "the tree is missing {piece}");
    }
    // The trunk is no longer as tall as both rows, and a branch no longer
    // begins flush against the other.
    for gone in [
        "grid-template-rows: 40px 40px;",
        "{ height: auto; align-self: stretch; pointer-events: none; }",
        "#debugbar .dbg-marks u.dbg-both { top: -44px; }",
    ] {
        assert!(!page.contains(gone), "the page still has {gone}");
    }
}

#[test]
fn stepping_in_takes_the_agents_turn_one_action_at_a_time() {
    let mut apart = played();
    let mut whole = played();
    assert!(whole.debug_step());
    let mut actions = 0;
    for _ in 0..400 {
        let before = apart.game.log.len();
        apart.debug_step_action().expect("a live game steps");
        if apart.game.turn == 2 {
            assert_eq!(
                apart.state()["debug"]["stepped"]["action"]["type"],
                json!("end_turn")
            );
            break;
        }
        actions += 1;
        assert_eq!(apart.game.log.len(), before + 1, "one action a step");
        assert_eq!(apart.game.log.last().map(|(seat, _)| *seat), Some(0));
        assert_eq!(apart.state()["debug"]["acted"], json!(actions));
    }
    assert_eq!(apart.game.turn, 2);
    assert!(actions > 0, "an opening turn has something to do");
    // Taken apart or taken whole, it is the same turn.
    assert_eq!(world(&apart), world(&whole));
}

#[test]
fn a_step_in_plans_again_once_the_person_has_acted() {
    let mut session = played();
    session.debug_step_action().expect("the first step");
    let by_hand = something_to_do(&session);
    assert_eq!(session.act(&by_hand), None);
    // The plan was made before that action; the next step is made on the
    // world as it now stands, and can be undone like any other.
    session
        .debug_step_action()
        .expect("a step after the person's own");
    if session.game.turn == 1 {
        assert_eq!(session.state()["debug"]["acted"], json!(3));
        assert_eq!(session.debug_undo(), Ok(2));
    }
}

#[test]
fn reveal_lifts_the_fog_from_the_page_and_changes_nothing_else() {
    let mut session = played();
    for _ in 0..3 {
        assert!(session.debug_step());
    }
    let fogged = session.state();
    let before = world(&session);
    assert!(session.debug(&json!({"op": "reveal"}))["error"].is_string());
    let answer = session.debug(&json!({"op": "reveal", "on": true}));
    assert_eq!(answer["error"], Value::Null);
    let revealed = session.state();
    assert_eq!(revealed["debug"]["reveal"], json!(true));
    let count = |state: &Value, key: &str| state[key].as_array().map_or(0, Vec::len);
    assert!(
        count(&revealed, "units") > count(&fogged, "units"),
        "a rival's units come into view"
    );
    assert_eq!(revealed["legal_actions"], fogged["legal_actions"]);
    assert_eq!(world(&session), before, "looking is not playing");
    // A world walked to, and a fork, are still shown revealed.
    assert_eq!(session.debug_jump(1), Ok(1));
    assert_eq!(session.state()["debug"]["reveal"], json!(true));
    assert_eq!(session.fork().state()["debug"]["reveal"], json!(true));
    session.debug(&json!({"op": "reveal", "on": false}));
    assert_eq!(session.state()["debug"]["reveal"], json!(false));
}

#[test]
fn a_condition_stops_a_run_when_the_state_crosses_its_line() {
    let mut session = played();
    for refused in [
        json!({"seat": 99, "key": "cities", "test": ">=", "value": 1}),
        json!({"seat": 0, "key": "no_such_reading", "test": ">", "value": 1}),
        json!({"seat": 0, "key": "cities", "test": "~", "value": 1}),
        json!({"seat": 0, "key": "cities", "test": ">="}),
        // A name can change, but it is not a number to hold to a line.
        json!({"seat": 0, "key": "civ", "test": ">", "value": 1}),
    ] {
        let answer = session.debug(&json!({"op": "breakpoints", "conditions": [refused.clone()]}));
        assert!(answer["error"].is_string(), "{refused}");
    }
    let set = session.debug(&json!({"op": "breakpoints", "conditions": [
        {"seat": 0, "key": "cities", "test": ">=", "value": 1},
        {"seat": 0, "key": "civ", "test": "changes"},
    ]}));
    assert_eq!(set["error"], Value::Null);
    assert_eq!(
        set["debug"]["breakpoints"]["conditions"][0],
        json!({"seat": 0, "key": "cities", "test": ">=", "value": 1.0})
    );
    assert!(set["debug"]["breakpoints"]["known_tests"]
        .as_array()
        .unwrap()
        .contains(&json!("changes")));

    let mut hit = Value::Null;
    for _ in 0..20 {
        let out = session.debug(&json!({"op": "step"}));
        hit = out["debug"]["stopped"].clone();
        if !hit.is_null() {
            break;
        }
    }
    assert_eq!(
        hit["kind"],
        json!("condition"),
        "twenty turns founded no city"
    );
    assert_eq!(hit["key"], json!("cities"));
    assert_eq!(hit["turn"], json!(session.game.turn));
    assert_eq!(hit["was"], json!(0.0));
    assert!(hit["now"].as_f64().unwrap() >= 1.0);
    // The line stays crossed from here on, and that does not stop every turn
    // after: a condition is the moment it becomes true.
    let next = session.debug(&json!({"op": "step"}));
    assert_eq!(next["debug"]["stopped"], Value::Null);
    // Clearing the breakpoints clears the conditions with them.
    let cleared = session.debug(&json!({"op": "breakpoints"}));
    assert_eq!(cleared["debug"]["breakpoints"]["conditions"], json!([]));
}

/// A step keeps what its conditions read of the world it left, for the step
/// after it. A run that does stops where a run that reads the world afresh
/// before every step stops, for the same reason, and what was kept is not
/// used for a world that has since been played on, walked away from, or
/// asked about under other conditions.
#[test]
fn a_reading_kept_from_the_last_step_is_the_reading_before_the_next() {
    let conditions = json!({"op": "breakpoints", "conditions": [
        {"seat": 0, "key": "cities", "test": ">=", "value": 1},
        {"seat": 1, "key": "cities", "test": "changes"},
        {"seat": 0, "key": "score", "test": "changes"},
    ]});
    let (mut kept, mut fresh) = (played(), played());
    for session in [&mut kept, &mut fresh] {
        assert_eq!(session.debug(&conditions)["error"], Value::Null);
    }
    let mut stops = 0;
    for step in 0..24 {
        // The twin reads the world itself, every step.
        fresh.debugger.read = None;
        let (a, b) = (
            kept.debug(&json!({"op": "step", "lean": true})),
            fresh.debug(&json!({"op": "step", "lean": true})),
        );
        assert_eq!(a["debug"]["stopped"], b["debug"]["stopped"], "step {step}");
        assert_eq!(world(&kept), world(&fresh), "step {step}");
        stops += usize::from(!a["debug"]["stopped"].is_null());
        if step == 0 {
            assert!(kept.debugger.read.is_some(), "a step keeps what it read");
        }
        // Part-way, the seat is walked back a turn and its conditions are
        // set again: neither world is the one the last step left.
        if step == 9 {
            for session in [&mut kept, &mut fresh] {
                session.debug(&json!({"op": "back"}));
                assert!(session.debugger.read.is_none(), "another operation lets go");
            }
        }
    }
    assert!(stops >= 2, "the conditions stopped the run {stops} times");

    // A world played on by hand is not the world that was read.
    let read = kept.debugger.read.clone().expect("the last step read it");
    let conditions = kept.debugger.breakpoints.conditions.clone();
    assert!(read.clone().of(&kept.game, &conditions).is_some());
    assert!(read.clone().of(&kept.game, &conditions[..1]).is_none());
    let mut moved = kept.fork();
    let action = moved
        .game
        .legal_actions(0)
        .into_iter()
        .find(|action| !matches!(action, Action::EndTurn))
        .expect("something to do");
    moved.game.apply(0, &action).expect("a legal action");
    assert!(read.clone().of(&moved.game, &conditions).is_none());
    // A fork is the same world, and may use it.
    assert!(read.of(&kept.fork().game, &conditions).is_some());
}

/// Head to head sets two halves side by side to see what a Civ AI changes.
/// That reads as it should only if nothing else differs: two halves of one
/// split, given the same agent, play the same game.
#[test]
fn two_halves_given_the_same_civ_ai_play_the_same_game() {
    let mut session = played();
    end_turn(&mut session);
    let (mut a, mut b) = (session.fork(), session.fork());
    for _ in 0..3 {
        assert_eq!(a.debug(&json!({"op": "step"}))["error"], Value::Null);
        assert_eq!(b.debug(&json!({"op": "step"}))["error"], Value::Null);
    }
    assert_eq!(a.game.turn, 5);
    assert_eq!(world(&a), world(&b));
}

/// Play `turns` turns with the seat's Civ AI and return the world after each.
fn stepped(session: &mut Session, turns: usize) -> Vec<String> {
    (0..turns)
        .map(|_| {
            assert_eq!(session.debug(&json!({"op": "step"}))["error"], Value::Null);
            world(session)
        })
        .collect()
}

/// A turn walked back to is played again as it was played: the agents that
/// were sitting when it opened sit down again with what they remembered.
/// An agent keeps plans from turn to turn, so a world put back and played on
/// by agents that had only just sat down was another game within a turn or
/// two (walked on civvis.ai/test, 2026-11-21: a jump back to turn 25 and the
/// same Civ AI played on parted from the turns kept ahead on turn 26).
#[test]
fn a_turn_walked_back_to_is_played_again_as_it_was() {
    let mut session = played();
    let first = stepped(&mut session, 8);
    for back in [6, 3] {
        assert_eq!(
            session.debug(&json!({"op": "jump", "turn": back}))["error"],
            Value::Null
        );
        assert_eq!(session.game.turn, back);
        let from = (back - 1) as usize;
        let again = stepped(&mut session, first.len() - from);
        for (turn, (was, is)) in first[from..].iter().zip(&again).enumerate() {
            assert!(
                was == is,
                "walked back to turn {back} and played on, turn {} is not the turn that was played",
                back as usize + turn + 1
            );
        }
    }
}

/// A save point is come back to with its agents too, wherever the game has
/// been played to since and whichever way.
#[test]
fn a_save_point_is_played_on_as_the_game_was() {
    let mut session = played();
    stepped(&mut session, 3);
    session
        .debug_save_point("here")
        .expect("a save point is set");
    let id = session.state()["debug"]["saves"][0]["id"]
        .as_u64()
        .expect("the save point has an id");
    let first = stepped(&mut session, 4);
    assert_eq!(
        session.debug(&json!({"op": "goto_save", "id": id}))["error"],
        Value::Null
    );
    assert_eq!(stepped(&mut session, 4), first);
}

/// An agent that remembers how many turns it has taken, and says each count
/// where a test can read it. It plays nothing: the server ends its turn.
#[derive(Clone, Default)]
struct Counting {
    taken: u32,
    said: std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
}

impl crate::ai::Ai for Counting {
    fn take_turn(&mut self, _game: &mut crate::game::Game, _seat: usize) {
        self.taken += 1;
        self.said
            .lock()
            .expect("the list is the test's own")
            .push(self.taken);
    }

    fn fork(&self) -> Option<Box<dyn crate::ai::Ai + Send>> {
        Some(Box::new(self.clone()))
    }
}

/// The agent that sits down again after a walk back is the one that was
/// sitting when that turn opened, with what it remembered then: not the one
/// sitting now, whose memory is of turns this world has not had, and not a
/// new one, which remembers nothing.
#[test]
fn a_walk_back_seats_the_agent_that_was_sitting_then() {
    let mut session = played();
    let said = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    session.ais[1] = Box::new(Counting {
        taken: 0,
        said: said.clone(),
    });
    let heard = || std::mem::take(&mut *said.lock().expect("the list is the test's own"));
    stepped(&mut session, 5);
    assert_eq!(heard(), [1, 2, 3, 4, 5]);
    assert_eq!(session.game.turn, 6);
    // Turn 3 opened after the rival's second turn.
    session.debug(&json!({"op": "jump", "turn": 3}));
    stepped(&mut session, 2);
    assert_eq!(heard(), [3, 4]);
    // A save point is come back to with its agents as well.
    session
        .debug_save_point("here")
        .expect("a save point is set");
    let id = session.state()["debug"]["saves"][0]["id"].clone();
    stepped(&mut session, 3);
    assert_eq!(heard(), [5, 6, 7]);
    assert_eq!(
        session.debug(&json!({"op": "goto_save", "id": id}))["error"],
        Value::Null
    );
    stepped(&mut session, 1);
    assert_eq!(heard(), [5]);
    // The agent kept with a turn is the one that was sitting then. The
    // timeline began before this one sat down, so turn 1 is not its turn.
    session.debug(&json!({"op": "jump", "turn": 1}));
    stepped(&mut session, 1);
    assert_eq!(heard(), [] as [u32; 0]);
}

/// A half of a split keeps no agents and seats none: a walk in a half seats
/// new agents, as it always has, so the two halves of a head to head differ
/// only in the Civ AI each seats. A half that is kept is the game itself,
/// and keeps them again.
#[test]
fn a_half_of_a_split_seats_new_agents_until_it_is_kept() {
    let minded = |session: &Session| matches!(session.debugger.marks.last(), Some(mark) if mark.minds.is_some());
    let mut session = played();
    let said = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    session.ais[1] = Box::new(Counting {
        taken: 0,
        said: said.clone(),
    });
    let heard = || std::mem::take(&mut *said.lock().expect("the list is the test's own"));
    stepped(&mut session, 4);
    assert!(minded(&session));
    assert_eq!(heard(), [1, 2, 3, 4]);
    let mut half = session.fork();
    assert!(half.debugger.forgetful && !session.debugger.forgetful);
    stepped(&mut half, 2);
    assert!(!minded(&half));
    // A walk back in the half does not seat the agents the game kept.
    half.debug(&json!({"op": "jump", "turn": 3}));
    stepped(&mut half, 1);
    assert_eq!(heard(), [] as [u32; 0]);
    half.debug_remembers();
    stepped(&mut half, 1);
    assert!(minded(&half));
}

/// The numbers `docs/SIM_DEBUGGER.md` gives for the timeline are the ones
/// this module uses.
#[test]
fn the_document_gives_the_numbers_the_timeline_uses() {
    let doc = include_str!("../../docs/SIM_DEBUGGER.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for said in [
        format!("Room is {ROOMY_MARKS} marks"),
        format!("({} GB unless the page names another", MEMORY_LINE >> 30),
        format!("the newest {DENSE_MARKS} stay whole"),
        format!("and so on to one in {COARSEST_STRIDE}"),
        format!("another 1/{MEMORY_STEPS} of the line"),
        format!("({} MB at 1 GB)", (MEMORY_LINE / MEMORY_STEPS) >> 20),
        format!("down to {MIN_MARKS} marks"),
        format!("A timeline holds {MAX_SAVE_POINTS} at once"),
    ] {
        assert!(doc.contains(&said), "the document no longer says: {said}");
    }
}

#[test]
fn a_round_stepped_seat_by_seat_is_the_round_ended_at_once() {
    let mut apart = played();
    let mut whole = played();
    end_turn(&mut whole);
    let mut seats = Vec::new();
    for _ in 0..40 {
        apart.debug_step_seat().expect("a live game steps");
        let state = apart.state();
        seats.push(state["debug"]["stepped"]["seat"].as_u64().unwrap());
        assert_eq!(state["debug"]["playing"], json!(apart.game.current));
        if apart.game.current == 0 {
            break;
        }
        // The middle of a round: the turn has not moved, and a rival's turn
        // is neither undone nor a place to keep.
        assert_eq!(apart.game.turn, 1);
        assert_eq!(state["debug"]["can_undo"], json!(false));
        assert!(apart.debug(&json!({"op": "undo"}))["error"].is_string());
        assert!(apart.debug(&json!({"op": "save_point", "name": "mid"}))["error"].is_string());
    }
    assert_eq!(seats[0], 0, "the seat's own turn ends first");
    assert!(
        seats.len() >= 3,
        "a rival and a city-state took turns of their own: {seats:?}"
    );
    assert_eq!(apart.game.turn, 2);
    assert_eq!(marked(&apart), [1, 2]);
    // Taken apart or taken whole, it is the same round.
    assert_eq!(world(&apart), world(&whole));
}

#[test]
fn a_step_in_the_middle_of_a_round_lets_the_round_finish_first() {
    let mut session = played();
    session.debug_step_seat().expect("the seat's turn ends");
    assert_ne!(session.game.current, 0);
    // A whole step finishes the round, and the turn that opens is held to
    // the breakpoints like any other.
    session.debug(&json!({"op": "breakpoints", "turns": [2]}));
    let out = session.debug(&json!({"op": "step"}));
    assert_eq!(out["turn"], json!(2));
    assert_eq!(session.game.current, 0);
    assert_eq!(out["debug"]["stopped"], json!({"kind": "turn", "turn": 2}));

    // The smallest step in the middle of a round is the next seat's turn.
    session.debug_step_seat().expect("the seat's turn ends");
    let up = session.game.current;
    let out = session.debug(&json!({"op": "step_action"}));
    assert_eq!(out["error"], Value::Null);
    assert_eq!(out["debug"]["stepped"]["seat"], json!(up));

    // Back from the middle of a round is the top of the seat's own turn.
    if session.game.current != 0 {
        assert_eq!(session.debug_back(), Ok(2));
    }
    assert_eq!(session.game.current, 0);

    // The rest of a round on request, and only when there is one.
    session.debug_step_seat().expect("the seat's turn ends");
    let out = session.debug(&json!({"op": "finish_round"}));
    assert_eq!(out["error"], Value::Null);
    assert_eq!(out["turn"], json!(3));
    assert_eq!(session.game.current, 0);
    assert!(session.debug(&json!({"op": "finish_round"}))["error"].is_string());
}

/// One whole turn played by the Civ AI named.
fn step_as(session: &mut Session, agent: &str) {
    let answer = session.debug(&json!({"op": "step", "strategy": agent, "lean": true}));
    assert_eq!(answer["error"], Value::Null);
}

/// The Civ AI picked for the seat, with nothing played.
fn pick(session: &mut Session, agent: &str) -> Value {
    let answer = session.debug(&json!({"op": "status", "strategy": agent}));
    assert_eq!(answer["error"], Value::Null);
    answer["debug"].clone()
}

/// A recording is the Civ AI's that made it. Walked back and handed to
/// another one, the seat's line ends on the open turn: the turns ahead are
/// kept, are not walked to, and are played over by the agent now seated.
/// What goes forward under a picked Civ AI is that agent's own play, never a
/// recording of another's.
#[test]
fn a_turn_ahead_is_walked_to_only_under_the_civ_ai_that_played_it() {
    let mut session = played();
    for _ in 0..4 {
        step_as(&mut session, "jack");
    }
    assert_eq!(session.game.turn, 5);
    let fifth = world(&session);
    assert_eq!(session.debug_jump(2), Ok(2));
    // Under the agent that played them, the turns ahead are this line.
    assert_eq!(marked(&session), [1, 2, 3, 4, 5]);
    assert_eq!(session.state()["debug"]["beyond"], Value::Null);

    // Another Civ AI is picked on the earlier turn, and nothing is played.
    let picked = pick(&mut session, "siege");
    assert_eq!(picked["seat"], json!("siege"));
    assert_eq!(picked["turns"], json!([1, 2]));
    assert_eq!(picked["last"], json!(2));
    assert_eq!(picked["can_forward"], json!(false));
    assert_eq!(picked["beyond"], json!({"from": 3, "to": 5, "by": "jack"}));
    // Neither way of walking reaches the other agent's turns.
    assert!(session.debug_forward().is_err());
    assert!(session.debug_jump(4).is_err());
    assert!(session.debug(&json!({"op": "jump", "turn": 5}))["error"].is_string());
    assert_eq!(session.game.turn, 2);

    // Picked again before anything is played, its turns are this line again.
    let again = pick(&mut session, "jack");
    assert_eq!(again["beyond"], Value::Null);
    assert_eq!(again["turns"], json!([1, 2, 3, 4, 5]));
    assert_eq!(session.debug_jump(5), Ok(5));
    assert_eq!(world(&session), fifth);

    // The other one plays from the earlier turn, over the recording.
    assert_eq!(session.debug_jump(2), Ok(2));
    step_as(&mut session, "siege");
    assert_eq!(session.game.turn, 3);
    assert_eq!(marked(&session), [1, 2, 3]);
    assert_eq!(session.state()["debug"]["beyond"], Value::Null);
    let newest = session.debugger.marks.last().expect("the turn is marked");
    assert_eq!(newest.by.as_deref(), Some("siege"));
}

/// The order does not matter: a Civ AI picked on the last turn played, and
/// the world then walked back, stands before another agent's turns too.
#[test]
fn a_civ_ai_picked_before_the_walk_back_does_not_walk_another_agents_turns() {
    let mut session = played();
    for _ in 0..3 {
        step_as(&mut session, "jack");
    }
    // On the last turn played there is nothing ahead to be another's.
    let picked = pick(&mut session, "siege");
    assert_eq!(picked["turns"], json!([1, 2, 3, 4]));
    assert_eq!(picked["beyond"], Value::Null);

    assert_eq!(session.debug_jump(2), Ok(2));
    let stands = session.state()["debug"].clone();
    assert_eq!(stands["turns"], json!([1, 2]));
    assert_eq!(stands["beyond"]["by"], json!("jack"));
    assert!(session.debug_forward().is_err());
    // Back is every turn behind, whoever played it.
    assert_eq!(session.debug_back(), Ok(1));
}

/// A turn the person ended is nobody else's game, whichever Civ AI is picked
/// to take the seat over: it is walked to as before.
#[test]
fn turns_ended_by_hand_are_walked_under_any_civ_ai() {
    let mut session = played();
    end_turn(&mut session);
    end_turn(&mut session);
    assert_eq!(session.debug_jump(1), Ok(1));
    let picked = pick(&mut session, "siege");
    assert_eq!(picked["turns"], json!([1, 2, 3]));
    assert_eq!(picked["beyond"], Value::Null);
    assert_eq!(session.debug_forward(), Ok(2));
}

/// Operator 2026-11-21: "lets label ai strats on the timeline too, to make
/// the branching more clear".
///
/// The state says whose play each stretch of the kept turns was, from the
/// turn it was played from to the last turn it opened: the page names each
/// stretch on the timeline (`debugStretchMarkup`). A timeline that passes
/// from one Civ AI to another says on which turn it did, which is what a
/// branch that was kept looks like.
#[test]
fn the_timeline_says_whose_play_each_stretch_was() {
    let mut session = played();
    assert_eq!(session.state()["debug"]["played_by"], json!([]));
    end_turn(&mut session);
    step_as(&mut session, "jack");
    step_as(&mut session, "jack");
    step_as(&mut session, "siege");
    assert_eq!(marked(&session), [1, 2, 3, 4, 5]);
    let whole = json!([
        {"from": 1, "to": 2, "by": null},
        {"from": 2, "to": 4, "by": "jack"},
        {"from": 4, "to": 5, "by": "siege"},
    ]);
    assert_eq!(session.state()["debug"]["played_by"], whole);
    // Walking the timeline changes nothing of whose it was. Siege holds the
    // seat, so Jack's turn ahead of turn 3 is not on the seat's line
    // (`Timeline::line`); it is kept, and still said to be Jack's.
    assert_eq!(session.debug_jump(3), Ok(3));
    let stands = session.state()["debug"].clone();
    assert_eq!(stands["turns"], json!([1, 2, 3]));
    assert_eq!(stands["beyond"], json!({"from": 4, "to": 5, "by": "jack"}));
    assert_eq!(stands["played_by"], whole);
    // Nor does who is picked for the seat.
    let picked = pick(&mut session, "jack");
    assert_eq!(picked["turns"], json!([1, 2, 3, 4]));
    assert_eq!(picked["played_by"], whole);
    // A fork carries it: a half of a split, and the game it is kept as.
    assert_eq!(session.fork().state()["debug"]["played_by"], whole);

    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    for piece in [
        "by: timeline.played_by || [], played: timeline.played_by || [],",
        "function debugStretchMarkup(model, at, travel, blocks, free) {",
        "return stretch.by ? debugAgentName(stretch.by) : \"By hand\";",
        "<div class=\"dbg-band\"></div><div class=\"dbg-band dbg-own\"></div><div class=\"dbg-by\"></div>",
        // A split names the trunk from one half and each branch from its own.
        "by: stretches(letter, fork.turn, kept(letter)), played: told(letter, \"by\"),",
    ] {
        assert!(page.contains(piece), "the named timeline is missing {piece}");
    }
}

/// Operator 2026-11-21: "a branch should fork the tree appropriate (1
/// timeline into 2 at the branch)", "each branch should have an X on the left
/// side. collapsing a branch should reduce the tree appropriately", and "if
/// the initial is deleted though, the branchbecomes the main tree,
/// transitioning from one ai strat to another".
///
/// A branch made behind the last turn played begins on the turn it is made
/// on: the half that is the branch lets go of the turns kept ahead
/// (`op: "branch"`), and the half that is the timeline as it was keeps them.
/// The page then draws the fork there (`debugSplitFork`), which a browser
/// holds in `beta/debugger_test.py` (`check_a_branch_forks_the_timeline`).
#[test]
fn a_branch_begins_on_the_turn_it_is_made_on() {
    let mut session = played();
    for _ in 0..4 {
        step_as(&mut session, "jack");
    }
    assert_eq!(session.debug_jump(3), Ok(3));
    let third = world(&session);
    let line = prints(&session);

    let mut initial = session.fork();
    let mut branch = session.fork();
    let answer = branch.debug(&json!({"op": "branch"}));
    assert_eq!(answer["error"], Value::Null);
    // The branch holds the turns behind it and none ahead, on the same world.
    assert_eq!(marked(&branch), [1, 2, 3]);
    assert_eq!(answer["debug"]["last"], json!(3));
    assert_eq!(answer["debug"]["can_forward"], json!(false));
    assert_eq!(answer["debug"]["beyond"], Value::Null);
    assert_eq!(world(&branch), third);
    assert_eq!(prints(&branch), line[..3]);
    // The timeline it was branched from is as it was.
    assert_eq!(marked(&initial), [1, 2, 3, 4, 5]);
    assert_eq!(prints(&initial), line);
    assert_eq!(initial.debug_forward(), Ok(4));

    // Another Civ AI plays the branch on, and the branch that is kept is a
    // line that passes from one to the other on the turn it was made on.
    step_as(&mut branch, "siege");
    step_as(&mut branch, "siege");
    assert_eq!(
        branch.state()["debug"]["played_by"],
        json!([{"from": 1, "to": 3, "by": "jack"}, {"from": 3, "to": 5, "by": "siege"}])
    );
    // Walked back and handed to Jack again, the line is Jack's as far as
    // Jack played it, and Siege's turns stand ahead of it.
    assert_eq!(branch.debug_jump(1), Ok(1));
    let picked = pick(&mut branch, "jack");
    assert_eq!(picked["turns"], json!([1, 2, 3]));
    assert_eq!(picked["beyond"], json!({"from": 4, "to": 5, "by": "siege"}));

    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    for piece in [
        // B is the branch, and is told so when turns are kept ahead.
        "branches: Math.max(timeline.last ?? state.turn, (timeline.beyond || {}).to ?? 0) > state.turn,",
        "if (letter === \"b\" && debugSplit.branches) debugSplitTell(\"b\", {civvis: \"pane-op\", body: {op: \"branch\"}});",
        // Until it says it holds none of them, they are left out of its line.
        "const cut = letter === \"b\" && debugSplit.branches ? from : Infinity;",
        // The ✕ of a branch closes it: the other is kept. It stands at the
        // right end of its branch's row (operator, 2026-11-22: "move x
        // deletes over to the right side please, right after each closable
        // timeline ends"), past the branch's letter and Civ AI, and the
        // strip between the halves drops a branch by the same name ("Should
        // have Drop A and Drop B over Keep").
        "<button class=\"dbg-close\" data-dbg-close=\"${key}\"",
        "if (debugSplit && !closed.disabled) debugSplitClose(closed.dataset.dbgClose === \"a\" ? \"b\" : \"a\");",
        "#debugbar.dbg-split-on .dbg-rail { grid-column: 3; }",
        "#debugbar.dbg-split-on .dbg-lane select.dbg-agent { grid-column: 5; }\n  #debugbar.dbg-split-on .dbg-close { grid-column: 6; }",
        "else if (action === \"drop-a\") debugSplitClose(\"b\");\n    else if (action === \"drop-b\") debugSplitClose(\"a\");",
        // Two branches are the most there are.
        "#debugbar.dbg-split-on .dbg-branch, #debugbar.dbg-split-on .dbg-who { display: none; }",
        // A branch is named on itself, by its letter and its Civ AI.
        "letter: fork.forked ? letter.toUpperCase() : null, seated: pane.strategy,",
        // A game opens on its one timeline, with the top-rated Civ AI in
        // its seat, and a second timeline is Branch's to make (operator,
        // 2026-11-24: "ai debugger should start with 1 strategy (maybe the
        // best strategy). players can branch if they want (they should
        // choose the new strategy when they branch)"). Two tracks at once
        // is what a page asks for by name, and Doctor Strange's Civ 6.
        "try { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); strangePaneResult(); debugOpenTheGame(); }",
        "if (best === null || Number(entry.elo) > Number(best.elo)) best = entry;",
        "const told = best && best !== debugStrategy() ? debugSwitchSeat(best) : null;",
        "const two = DEBUG_TRACKS_ASKED === null ? STRANGE : DEBUG_TRACKS_ASKED === 2;",
        // Branch asks which Civ AI plays the branch before there is one,
        // and makes it for the one picked: the two screens are asked
        // nothing, and neither is a game that opens on two (operator,
        // 2026-11-22: "branch b should only ask when branch is selected.
        // not just auto ask rom the start").
        "if (debugAsk && debugSplit !== null) debugAsk = null;",
        "<button data-dbg-ask=\"seat\" value=\"${escapeAttr(option.value)}\"${busy ? \" disabled\" : \"\"}>",
        // One screen and two change places in one move (operator,
        // 2026-11-22: "i do not like the whole transition between ai
        // debugger 1 screen and 2 screens"). The window is held as it
        // stands, the split is made unseen, and it is shown when both
        // halves are there. What that looks like in a browser is held by
        // `beta/debugger_test.py`
        // (`check_one_screen_and_two_change_places_in_one_move`).
        "shown: false, look,",
        "if (debugHeld) return;",
        "host.className = \"dbg-unseen\";",
        "if (there(split.panes.a) && there(split.panes.b) && split.panes.a.deck && !split.branches) debugSplitReveal();",
        "waiting: setTimeout(debugSplitReveal, DEBUG_SPLIT_WAIT_MS)};",
        // A game that opens this way waits under a veil instead.
        "debugSplitOpen(\"row\", null, null, true);",
        "if (fresh) debugVeil(true); else debugBar.classList.add(\"dbg-branching\");",
        // Another Civ AI picked on the one timeline is asked about first.
        "debugAsk = target.value && target.value !== debugStrategy() ? {kind: \"switch\", name: target.value} : null;",
    ] {
        assert!(page.contains(piece), "the branch is missing {piece}");
    }
}

/// The timeline says how it is used, Click to travel, until it has been
/// clicked to travel, and then never again in that browser (operator,
/// 2026-11-22: "have the initial timeline say click to travel so people
/// understand the mechanism. make the click to travel very visible at the
/// start and then not appear once they understand"). What that looks like,
/// and what counts as a click on the timeline, is held by
/// `beta/debugger_test.py` (`check_the_timeline_says_click_to_travel`).
#[test]
fn the_timeline_says_click_to_travel_until_it_is_clicked() {
    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    for piece in [
        "<div class=\"dbg-travel\" aria-hidden=\"true\" hidden><i>◀</i>Click to travel<i>▶</i></div>",
        "animation: dbg-travel-call 1.6s ease-out infinite;",
        "try { localStorage.setItem(DEBUG_TRAVEL_KEY, \"known\"); } catch (_) {}",
        "const call = !debugTravelKnown && !!model.travel;",
        // A slider let go of on another turn has travelled.
        "debugSliderHeld = false;\n      debugTravelLearnt();",
    ] {
        assert!(page.contains(piece), "the timeline is missing {piece}");
    }
}

/// A turn taken an action at a time is the agent's turn as much as one taken
/// whole, and the rest of it is not kept for an agent that did not plan it.
#[test]
fn a_turn_stepped_in_is_its_agents_and_its_plan_goes_with_the_seat() {
    let mut session = played();
    let step_in = |session: &mut Session, agent: &str| {
        let answer = session.debug(&json!({"op": "step_action", "strategy": agent, "lean": true}));
        assert_eq!(answer["error"], Value::Null);
    };
    step_in(&mut session, "jack");
    assert!(
        session.debugger.plan.is_some(),
        "an opening turn is planned"
    );
    // Another Civ AI takes the seat mid-turn: the plan was not its own.
    pick(&mut session, "siege");
    assert!(session.debugger.plan.is_none());
    for _ in 0..400 {
        if session.game.turn == 2 {
            break;
        }
        step_in(&mut session, "siege");
    }
    assert_eq!(session.game.turn, 2);
    let newest = session.debugger.marks.last().expect("the turn is marked");
    assert_eq!((newest.turn, newest.by.as_deref()), (2, Some("siege")));
}

/// One `walk`, as a run asks it before it plays (`debugRun`).
fn walk(session: &mut Session, request: Value) -> Value {
    let answer = session.debug(&request);
    assert_eq!(answer["error"], Value::Null);
    answer["debug"].clone()
}

/// Operator 2026-11-21: "when sim forward 50 turns, back 20, then forward
/// 30, turns 1-20 of the forward 30 step dont need to be recomputed. we
/// already have the saves. take the latest save on the path and start from
/// there". A run walks to the last kept turn it would only play again, and
/// the turns played from there are the turns the whole run would have played.
#[test]
fn a_run_takes_the_turns_its_civ_ai_already_played_from_their_marks() {
    let mut session = played();
    for _ in 0..6 {
        step_as(&mut session, "jack");
    }
    let seventh = world(&session);
    // The same game played on without a walk: back three, six turns on.
    assert_eq!(session.debug_jump(4), Ok(4));
    let played_again: Vec<String> = (0..6)
        .map(|_| {
            step_as(&mut session, "jack");
            world(&session)
        })
        .collect();
    assert_eq!(played_again[2], seventh);
    assert_eq!(session.game.turn, 10);

    // Back six, and nine turns on: the first six are kept.
    assert_eq!(session.debug_jump(4), Ok(4));
    assert_eq!(session.state()["debug"]["played_ahead"], json!(10));
    let actions = session.game.log.len();
    let walked = walk(
        &mut session,
        json!({"op": "walk", "turn": 13, "strategy": "jack"}),
    );
    assert_eq!(session.game.turn, 10);
    assert_eq!(walked["stopped"], Value::Null);
    assert_eq!(walked["played_ahead"], Value::Null);
    assert_eq!(world(&session), played_again[5]);
    // Nothing was played to get there, and nothing kept was let go.
    assert_eq!(session.game.log.len(), session.debugger.marks[9].actions);
    assert!(session.game.log.len() > actions);
    assert_eq!(marked(&session), (1..=10).collect::<Vec<u64>>());

    // A walk that stops short of the last kept turn keeps the rest.
    assert_eq!(session.debug_jump(4), Ok(4));
    walk(&mut session, json!({"op": "walk", "turn": 7}));
    assert_eq!(session.game.turn, 7);
    assert_eq!(world(&session), seventh);
    assert_eq!(marked(&session), (1..=10).collect::<Vec<u64>>());
    assert_eq!(session.state()["debug"]["played_ahead"], json!(10));

    // On the last turn played there is nothing to take, and nothing is said.
    assert_eq!(session.debug_jump(10), Ok(10));
    let stays = walk(&mut session, json!({"op": "walk", "turn": 30}));
    assert_eq!((session.game.turn, &stays["stopped"]), (10, &Value::Null));

    // Every run of the page asks for the walk before it plays, whichever
    // control began it: Auto-play, the turn box, the timeline, a half's run.
    let script = include_str!("../../web/assets/app_debugger.js");
    for piece in [
        "if ((debugTimeline().played_ahead ?? from) > from) {",
        "const next = await debugOp({op: \"walk\", turn: until, era: era !== null, strategy: debugStrategy()},",
        "on = !over(next) && !debugStopAsked;",
    ] {
        assert!(script.contains(piece), "a run no longer walks its kept turns: {piece}");
    }
}

/// A walk passes a breakpoint no more than the run that played the turns
/// would: a turn, an era and a state condition each stop it on their turn.
#[test]
fn a_walk_along_kept_turns_stops_where_a_breakpoint_would_have_stopped_the_run() {
    let mut session = played();
    for _ in 0..8 {
        step_as(&mut session, "jack");
    }
    assert_eq!(session.debug_jump(2), Ok(2));
    let set = session.debug(&json!({"op": "breakpoints", "turns": [5]}));
    assert_eq!(set["error"], Value::Null);
    let walked = walk(&mut session, json!({"op": "walk"}));
    assert_eq!(session.game.turn, 5);
    assert_eq!(walked["stopped"], json!({"kind": "turn", "turn": 5}));
    // From the breakpoint's own turn the walk goes on past it.
    let walked = walk(&mut session, json!({"op": "walk"}));
    assert_eq!((session.game.turn, &walked["stopped"]), (9, &Value::Null));

    // A condition is held against the turn before, as a step holds it: the
    // walk stops on the turn, and for the reason, that playing them does.
    assert_eq!(session.debug_jump(2), Ok(2));
    let changes = json!({"seat": 0, "key": "score", "test": "changes"});
    let set = session.debug(&json!({"op": "breakpoints", "conditions": [changes]}));
    assert_eq!(set["error"], Value::Null);
    let mut ran = Value::Null;
    while session.game.turn < 9 {
        let answer = session.debug(&json!({"op": "step", "strategy": "jack", "lean": true}));
        assert_eq!(answer["error"], Value::Null);
        if ran.is_null() {
            ran = answer["debug"]["stopped"].clone();
        }
    }
    assert_eq!(session.debug_jump(2), Ok(2));
    let walked = walk(&mut session, json!({"op": "walk"}));
    assert_eq!(walked["stopped"], ran);
    let stop = ran["turn"].as_u64().unwrap_or(9);
    assert_eq!(u64::from(session.game.turn), stop);

    // A run that plays until an era begins stops on the era's first kept
    // turn, breakpoint or none: here there is none ahead, so it does not.
    let clear = session.debug(&json!({"op": "breakpoints"}));
    assert_eq!(clear["error"], Value::Null);
    assert_eq!(session.debug_jump(2), Ok(2));
    let walked = walk(&mut session, json!({"op": "walk", "era": true}));
    assert_eq!((session.game.turn, &walked["stopped"]), (9, &Value::Null));
    let dawn = session.debugger.marks[5].game.world_era + 1;
    for mark in &mut session.debugger.marks[5..] {
        mark.game.world_era = dawn;
    }
    assert_eq!(session.debug_jump(2), Ok(2));
    let walked = walk(&mut session, json!({"op": "walk", "era": true}));
    assert_eq!(session.game.turn, 6);
    assert_eq!(
        walked["stopped"],
        json!({"kind": "era", "turn": 6, "era": dawn})
    );
}

/// Only a turn the seated Civ AI would play again exactly is taken from its
/// mark. A turn the person ended, a turn touched by hand before the agent
/// took it, another Civ AI's turn and a turn in a half of a split are played.
#[test]
fn a_walk_takes_no_turn_the_seated_civ_ai_would_not_play_again() {
    // Ended by hand: the agent's game from there is another game.
    let mut session = played();
    pick(&mut session, "jack");
    end_turn(&mut session);
    step_as(&mut session, "jack");
    step_as(&mut session, "jack");
    assert_eq!(session.debug_jump(1), Ok(1));
    assert_eq!(session.state()["debug"]["played_ahead"], Value::Null);
    walk(&mut session, json!({"op": "walk"}));
    assert_eq!(session.game.turn, 1);
    // From the turn the agent took over on, its turns are taken.
    assert_eq!(session.debug_jump(2), Ok(2));
    assert_eq!(session.state()["debug"]["played_ahead"], json!(4));

    // Touched first: the turn the agent finished is not the one it plays
    // from the top, and the turns after it are.
    let mut session = played();
    pick(&mut session, "jack");
    let hand = something_to_do(&session);
    assert_eq!(session.act(&hand), None);
    for _ in 0..3 {
        step_as(&mut session, "jack");
    }
    assert_eq!(session.debug_jump(1), Ok(1));
    assert_eq!(session.state()["debug"]["played_ahead"], Value::Null);
    assert_eq!(session.debug_jump(2), Ok(2));
    assert_eq!(session.state()["debug"]["played_ahead"], json!(4));
    // With something done in the open turn it no longer stands as it opened.
    let hand = something_to_do(&session);
    assert_eq!(session.act(&hand), None);
    assert_eq!(session.state()["debug"]["played_ahead"], Value::Null);
    walk(&mut session, json!({"op": "walk"}));
    assert_eq!(session.game.turn, 2);
    // Taken back, it does again.
    while session.debugger.acted(&session.game) > 0 {
        session
            .debug_undo()
            .expect("an action by hand is taken back");
    }
    assert_eq!(session.state()["debug"]["played_ahead"], json!(4));

    // Another Civ AI's turns: the walk names the one picked, and stays.
    walk(&mut session, json!({"op": "walk", "strategy": "siege"}));
    assert_eq!(session.game.turn, 2);
    step_as(&mut session, "siege");
    assert_eq!(marked(&session), [1, 2, 3]);

    // A half of a split seats new agents wherever it is walked to, so what
    // it plays from a kept turn is not the turn that was kept.
    let mut half = session.fork();
    for _ in 0..2 {
        step_as(&mut half, "siege");
    }
    assert_eq!(half.debug_jump(3), Ok(3));
    assert_eq!(half.state()["debug"]["played_ahead"], Value::Null);
}

/// Operator 2026-11-21: "when the debugger jumps back in time, then changes
/// the strategy, then autoplays, it must play under the new strategy. i think
/// it may be playing out the previous forward recording".
///
/// The engine's half is the seat's line
/// (`a_turn_ahead_is_walked_to_only_under_the_civ_ai_that_played_it`). The
/// page's is that a pick reaches the engine when it is made, from the box's
/// picker and in each half of a split, so the timeline it is answered with
/// is the picked agent's before anything else is pressed. What that does in
/// a browser is held by `beta/debugger_test.py`
/// (`check_a_picked_civ_ai_plays_its_own_turns`).
#[test]
fn the_page_says_a_picked_civ_ai_to_the_engine_when_it_is_picked() {
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let said = script
        .split_once("function debugSeat() {")
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .expect("debugSeat is a top-level function")
        .0;
    assert!(said.contains(r#"debugOp({op: "status", strategy: name})"#));
    // The branch that hears each picker, up to the branch after it.
    let branch = |heard: &str| {
        let rest = script.split_once(heard).expect("the picker is heard").1;
        &rest[..rest.find("} else if").expect("another branch follows")]
    };
    // The box's own picker. Another Civ AI than the one seated is asked about
    // first (operator, 2026-11-22: "when edited to a different strategy,
    // should ask to switch or branch"), and nothing is said until it is
    // answered. The one that is seated, and the one Switch answers with, go
    // through `debugSwitchSeat`, which says the seat to the engine at once.
    assert!(branch(r#"target.id === "dbg-agent""#)
        .contains("if (debugAsk) drawDebugBar(false); else debugSwitchSeat(target.value);"));
    let switched = script
        .split_once("function debugSwitchSeat(name) {")
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .expect("debugSwitchSeat is a top-level function")
        .0;
    assert!(switched.contains("debugSeat();"));
    assert!(script
        .contains(r#"if (how === "switch" && ask.kind === "switch") debugSwitchSeat(ask.name);"#));
    assert!(branch(r#"data.civvis === "pane-strategy""#).contains("await debugSeat();"));
    // Whose the turns ahead are is the engine's to say, and the page draws it.
    assert!(script.contains("timeline.beyond"));
    let answer = played().debug(&json!({"op": "status", "strategy": "jack"}));
    assert_eq!(answer["debug"]["seat"], json!("jack"));
    assert_eq!(answer["debug"]["beyond"], Value::Null);
}
