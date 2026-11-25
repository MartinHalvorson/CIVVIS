//! What a split screen's two halves are promised by the socket server.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::stock_opening_params;
use super::named;
use crate::setup::{GameSpeed, MapScript, MapTopology};

#[test]
fn a_pane_is_named_by_its_request_or_by_the_page_that_sent_it() {
    assert_eq!(named("/state?pane=a-k3x", None), Some("a-k3x"));
    assert_eq!(
        named(
            "/action",
            Some("http://127.0.0.1:8765/?pane=b-k3x&instance=4")
        ),
        Some("b-k3x")
    );
    // The request's own word wins over the page's.
    assert_eq!(
        named("/state?pane=a-1", Some("http://h/?pane=b-1")),
        Some("a-1")
    );
    assert_eq!(named("/state", None), None);
    assert_eq!(named("/state", Some("http://127.0.0.1:8765/")), None);
    // A page with more worlds than two names them on down the alphabet.
    assert_eq!(named("/state?pane=c-1", None), Some("c-1"));
    assert_eq!(named("/state?pane=l-k3x", None), Some("l-k3x"));
    for stray in ["A-1", "1-1", "a", "a-", "a-../x", "ab-1", "a-%20", "é-1"] {
        assert_eq!(
            named(&format!("/state?pane={stray}"), None),
            None,
            "{stray}"
        );
    }
}

fn http(port: u16, request: &str) -> Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("the server is up");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let body = response.split_once("\r\n\r\n").expect("a response body").1;
    serde_json::from_str(body).expect("a JSON answer")
}

fn get(port: u16, target: &str) -> Value {
    http(
        port,
        &format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"),
    )
}

fn post(port: u16, target: &str, referer: Option<&str>, body: &Value) -> Value {
    let body = body.to_string();
    let referer = referer.map_or(String::new(), |page| format!("Referer: {page}\r\n"));
    http(
        port,
        &format!(
            "POST {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n{referer}Content-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn each_half_of_a_split_plays_a_history_of_its_own() {
    let port = TcpListener::bind(("127.0.0.1", 0))
        .expect("a free port")
        .local_addr()
        .unwrap()
        .port();
    let mut params = stock_opening_params(20_261_119);
    params.spectate = false;
    params.num_players = 2;
    params.num_city_states = 1;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    params.game_speed = GameSpeed::Standard;
    params.max_turns = 500;
    std::thread::spawn(move || super::super::serve_with_game(port, false, params, None, false));
    let deadline = Instant::now() + Duration::from_secs(60);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "the server never came up");
        std::thread::sleep(Duration::from_millis(50));
    }
    let end_turn = json!({"action": {"type": "end_turn"}});

    // The main game moves on a turn before the screen is split.
    assert_eq!(post(port, "/action", None, &end_turn)["turn"], json!(2));

    // Pane A starts where the main game stands, with the turn behind it to
    // walk back to, and then plays on alone.
    let a = get(port, "/state?pane=a-s1");
    assert_eq!(a["turn"], json!(2));
    assert_eq!(a["debug"]["turns"], json!([1, 2]));
    assert_eq!(
        post(port, "/action?pane=a-s1", None, &end_turn)["turn"],
        json!(3)
    );
    assert_eq!(
        get(port, "/state")["turn"],
        json!(2),
        "the main game stayed"
    );

    // Pane B is named by its page, the way a browser names it, and forks the
    // main game too: A's turn is not its turn.
    let page = format!("http://127.0.0.1:{port}/?pane=b-s1&instance=7");
    let b = post(port, "/debug", Some(&page), &json!({"op": "back"}));
    assert_eq!(b["error"], Value::Null);
    assert_eq!(b["turn"], json!(1));
    assert_eq!(get(port, "/state?pane=a-s1")["turn"], json!(3));
    assert_eq!(get(port, "/state")["turn"], json!(2));

    // A save crosses from one half to the other as an uploaded game.
    let save = get(port, "/save?pane=a-s1");
    let copied = post(port, "/load", Some(&page), &json!({"game": save}));
    assert_eq!(copied["error"], Value::Null);
    assert_eq!(copied["turn"], json!(3));

    // Half a screen does not start a new game for the whole of it.
    let refused = post(port, "/new?pane=a-s1", None, &json!({}));
    assert!(refused["error"].is_string());
    assert_eq!(get(port, "/state")["turn"], json!(2));

    // One half is made a copy of the other inside the engine, so the copy
    // has the turns behind it too and not only the world.
    let stepped = post(port, "/debug?pane=a-s1", None, &json!({"op": "step"}));
    assert_eq!(stepped["turn"], json!(4));
    let copy = post(
        port,
        "/debug",
        Some(&page),
        &json!({"op": "copy", "from": "a-s1"}),
    );
    assert_eq!(copy["error"], Value::Null);
    assert_eq!(copy["turn"], json!(4));
    assert_eq!(copy["debug"]["turns"], stepped["debug"]["turns"]);
    for stray in ["b-s1", "a-other", "c-s1", ""] {
        let refused = post(
            port,
            "/debug",
            Some(&page),
            &json!({"op": "copy", "from": stray}),
        );
        assert!(refused["error"].is_string(), "copy from {stray:?}");
    }
    // The copy is its own again at once.
    assert_eq!(
        post(port, "/action", Some(&page), &end_turn)["turn"],
        json!(5)
    );
    assert_eq!(get(port, "/state?pane=a-s1")["turn"], json!(4));

    // A screen that is not split has nothing to adopt, and says so.
    let stray = post(
        port,
        "/debug",
        None,
        &json!({"op": "adopt", "pane": "a-none"}),
    );
    assert!(stray["error"].is_string());
    assert_eq!(stray["turn"], json!(2), "the main game stayed");

    // Closing the split onto B: B's session is the main one now, with every
    // turn B could walk back to, and the halves are gone.
    let before = get(port, "/state?pane=b-s1");
    let kept = post(
        port,
        "/debug",
        None,
        &json!({"op": "adopt", "pane": "b-s1"}),
    );
    assert_eq!(kept["error"], Value::Null);
    assert_eq!(kept["turn"], json!(5));
    assert_eq!(kept["debug"]["turns"], before["debug"]["turns"]);
    assert!(kept["debug"]["turns"].as_array().unwrap().len() >= 4);
    let main = get(port, "/state");
    assert_eq!(main["turn"], json!(5));
    assert_eq!(
        post(port, "/debug", None, &json!({"op": "back"}))["turn"],
        json!(4)
    );

    // The next split starts from the main game as it now stands.
    assert_eq!(get(port, "/state?pane=a-s2")["turn"], json!(4));
}

#[test]
fn panes_fork_copy_and_adopt_without_a_socket() {
    // The published build holds its panes behind one borrow and calls the
    // same three steps; this is that path, with no server around it.
    let mut params = stock_opening_params(20_261_119);
    params.spectate = false;
    params.num_players = 2;
    params.num_city_states = 1;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    params.game_speed = GameSpeed::Standard;
    params.max_turns = 500;
    let mut main = super::super::Session::new(params.clone());
    let mut panes = super::Panes::default();
    let end_turn = json!({"action": {"type": "end_turn"}});
    let ask = |panes: &mut super::Panes,
               main: &super::super::Session,
               pane: &str,
               method: &str,
               path: &str,
               body: &Value| {
        let asked = super::Asked {
            method,
            path,
            target: path,
            parsed: body,
        };
        super::serve(panes, main, pane, &asked, None)
    };
    let (a, whole) = ask(&mut panes, &main, "a-x", "POST", "/action", &end_turn).unwrap();
    assert!(whole);
    assert_eq!(a["turn"], json!(2));
    assert_eq!(main.game.turn, 1);
    // A request that does not concern a world is not a pane's to answer.
    assert!(ask(&mut panes, &main, "a-x", "GET", "/runtime", &Value::Null).is_none());
    let (save, whole) = ask(&mut panes, &main, "a-x", "GET", "/save", &Value::Null).unwrap();
    assert!(!whole, "a save is answered bare");
    assert!(save.get("save_format_version").is_some() || save.get("game").is_some());
    let copy = json!({"op": "copy", "from": "a-x"});
    let (b, _) = ask(&mut panes, &main, "b-x", "POST", "/debug", &copy).unwrap();
    assert_eq!(b["turn"], json!(2));
    assert_eq!(b["debug"]["turns"], json!([1, 2]));
    assert_eq!(panes.len(), 2);
    assert_eq!(panes.adopt("b-x", &mut main), Ok(()));
    assert_eq!(main.game.turn, 2);
    assert_eq!(main.state()["debug"]["turns"], json!([1, 2]));
    assert_eq!(panes.len(), 0);
    assert!(panes.adopt("b-x", &mut main).is_err());

    // A watched table has no seat to fork.
    params.spectate = true;
    let watched = super::super::Session::new(params);
    let (refused, _) = ask(&mut panes, &watched, "a-y", "GET", "/state", &Value::Null).unwrap();
    assert!(refused["error"].is_string());
}

/// A small table with a seat of its own, for the halves of a split to fork.
fn small_table() -> super::super::Session {
    let mut params = stock_opening_params(20_261_120);
    params.spectate = false;
    params.num_players = 2;
    params.num_city_states = 1;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    params.game_speed = GameSpeed::Standard;
    params.max_turns = 500;
    super::super::Session::new(params)
}

/// One request of one half, answered the way the published build answers it.
fn asked_of(
    panes: &mut super::Panes,
    main: &super::super::Session,
    pane: &str,
    method: &str,
    path: &str,
    body: &Value,
) -> Value {
    let asked = super::Asked {
        method,
        path,
        target: path,
        parsed: body,
    };
    super::serve(panes, main, pane, &asked, None)
        .expect("a request that concerns a world")
        .0
}

/// The whole world of one half, as a save would carry it.
fn world_of(panes: &mut super::Panes, main: &super::super::Session, pane: &str) -> String {
    asked_of(panes, main, pane, "GET", "/save", &Value::Null).to_string()
}

/// Two halves that are the same world are played once (operator 2026-11-20:
/// "if 2 games are identical, we should also not have to simulate both (only
/// if they are entirely identical). we can just simulate one"). A debugger
/// request that names the other half as its twin is answered from one half,
/// and the other is made a copy of what that half then holds.
#[test]
fn halves_that_are_alike_are_played_once() {
    let main = small_table();
    let mut panes = super::Panes::default();
    let none = Value::Null;
    let step_both = json!({"op": "step", "twin": "b-x"});

    // Both halves begin as the main session, and reading does not part them.
    let a = asked_of(&mut panes, &main, "a-x", "GET", "/state", &none);
    assert_eq!(a["turn"], json!(1));
    assert!(!panes.alike("a-x", "b-x"), "one half is not a pair");
    asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    asked_of(&mut panes, &main, "b-x", "POST", "/forecast", &json!({}));
    asked_of(&mut panes, &main, "a-x", "GET", "/rules", &none);
    assert!(panes.alike("a-x", "b-x"));

    // A is asked to step for both: B is on each turn without playing it.
    for turn in 2..=4 {
        let a = asked_of(&mut panes, &main, "a-x", "POST", "/debug", &step_both);
        assert_eq!(a["error"], none);
        assert_eq!(a["debug"]["twinned"], json!(true));
        assert_eq!(a["turn"], json!(turn));
        assert!(panes.alike("a-x", "b-x"));
    }
    let b = asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    assert_eq!(b["turn"], json!(4));
    assert_eq!(b["debug"]["turns"], json!([1, 2, 3, 4]));
    let together = world_of(&mut panes, &main, "b-x");
    assert_eq!(world_of(&mut panes, &main, "a-x"), together);

    // It is the game the two halves play when each is asked to play itself.
    let mut apart = super::Panes::default();
    for _ in 0..3 {
        for pane in ["a-y", "b-y"] {
            let stepped = asked_of(
                &mut apart,
                &main,
                pane,
                "POST",
                "/debug",
                &json!({"op": "step"}),
            );
            assert_eq!(stepped["error"], none);
            assert!(
                stepped["debug"].get("twinned").is_none(),
                "nobody named a twin"
            );
        }
    }
    assert!(
        !apart.alike("a-y", "b-y"),
        "each was asked to play turns of its own"
    );
    assert_eq!(world_of(&mut apart, &main, "a-y"), together);
    assert_eq!(world_of(&mut apart, &main, "b-y"), together);
}

/// A half that was asked anything of its own is not a twin. A request for
/// both then plays only the half it was asked of, and says so, so that the
/// page asks the other half as well. Nothing is copied over a half that has
/// a history of its own.
#[test]
fn a_half_with_a_history_of_its_own_is_not_played_for() {
    let main = small_table();
    let mut panes = super::Panes::default();
    let none = Value::Null;
    let step_both = json!({"op": "step", "twin": "b-x"});
    asked_of(&mut panes, &main, "a-x", "GET", "/state", &none);
    asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);

    // B is shown the whole map. Nothing in its world moved, but it was asked.
    let reveal = json!({"op": "reveal", "on": true});
    let b = asked_of(&mut panes, &main, "b-x", "POST", "/debug", &reveal);
    assert_eq!(b["error"], none);
    assert!(!panes.alike("a-x", "b-x"));
    let a = asked_of(&mut panes, &main, "a-x", "POST", "/debug", &step_both);
    assert_eq!(a["debug"]["twinned"], json!(false));
    assert_eq!(a["turn"], json!(2));
    let b = asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    assert_eq!(b["turn"], json!(1), "B is left for its own page to play");
    assert_eq!(b["debug"]["reveal"], json!(true));

    // A copy puts the two on one line again, in either direction.
    let copy = json!({"op": "copy", "from": "a-x"});
    assert_eq!(
        asked_of(&mut panes, &main, "b-x", "POST", "/debug", &copy)["turn"],
        json!(2)
    );
    assert!(panes.alike("a-x", "b-x"));
    let a = asked_of(&mut panes, &main, "a-x", "POST", "/debug", &step_both);
    assert_eq!(a["debug"]["twinned"], json!(true));
    let b = asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    assert_eq!(b["turn"], json!(3));
    assert_eq!(b["debug"]["reveal"], json!(false), "B is A's copy now");

    // Only the other half of this split is a twin.
    for stray in ["a-x", "b-other", "c-x", "b", ""] {
        asked_of(&mut panes, &main, "b-x", "POST", "/debug", &copy);
        assert!(panes.alike("a-x", "b-x"));
        let asked = json!({"op": "status", "twin": stray});
        let a = asked_of(&mut panes, &main, "a-x", "POST", "/debug", &asked);
        assert_eq!(a["debug"]["twinned"], json!(false), "{stray:?}");
        assert_eq!(panes.len(), 2, "{stray:?}");
    }

    // A request that is not the debugger's names no twin.
    asked_of(&mut panes, &main, "b-x", "POST", "/debug", &copy);
    let end_turn = json!({"action": {"type": "end_turn"}, "twin": "b-x"});
    let a = asked_of(&mut panes, &main, "a-x", "POST", "/action", &end_turn);
    assert_eq!(a["turn"], json!(4));
    assert!(a["debug"].get("twinned").is_none());
    assert!(!panes.alike("a-x", "b-x"));
    let b = asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    assert_eq!(b["turn"], json!(3));
}

/// Of two halves played as one, only the half that was asked has agents
/// that lived through those turns: the copy's agents are seated anew. When
/// the two are then played apart, the half that played is seated anew as
/// well, so the two go on to play the same game for as long as they are
/// asked the same. A head to head of two Civ AIs rests on that.
#[test]
fn halves_played_as_one_and_then_apart_go_on_alike() {
    let main = small_table();
    let mut panes = super::Panes::default();
    let none = Value::Null;
    let step = json!({"op": "step"});
    let step_both = json!({"op": "step", "twin": "b-x"});
    asked_of(&mut panes, &main, "a-x", "GET", "/state", &none);
    asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    for _ in 0..6 {
        let a = asked_of(&mut panes, &main, "a-x", "POST", "/debug", &step_both);
        assert_eq!(a["debug"]["twinned"], json!(true));
    }
    // Apart from here: each is asked for itself, turn about.
    for turn in 8..=13 {
        for pane in ["a-x", "b-x"] {
            let stepped = asked_of(&mut panes, &main, pane, "POST", "/debug", &step);
            assert_eq!(stepped["error"], none);
            assert_eq!(stepped["turn"], json!(turn));
        }
        assert!(!panes.alike("a-x", "b-x"));
        assert_eq!(
            world_of(&mut panes, &main, "a-x"),
            world_of(&mut panes, &main, "b-x"),
            "turn {turn}"
        );
    }
}

/// The turns a half holds and the print of each one's world, as its state
/// document gives them.
fn prints_of(
    panes: &mut super::Panes,
    main: &super::super::Session,
    pane: &str,
) -> Vec<(u64, u64)> {
    let state = asked_of(panes, main, pane, "GET", "/state", &Value::Null);
    let numbers = |key: &str| -> Vec<u64> {
        state["debug"][key]
            .as_array()
            .unwrap_or_else(|| panic!("debug.{key} is a list"))
            .iter()
            .map(|number| number.as_u64().expect("a whole number"))
            .collect()
    };
    let (turns, worlds) = (numbers("turns"), numbers("worlds"));
    assert_eq!(turns.len(), worlds.len(), "a print for every turn held");
    assert!(
        worlds.iter().all(|world| *world < 1 << 53),
        "a print is a number a page can hold exactly"
    );
    turns.into_iter().zip(worlds).collect()
}

/// A branched timeline is one timeline for as long as the two halves hold
/// the same world, and two from the turn their worlds differ (operator
/// 2026-11-21: "branching the timeline should keep 1 timeline where they
/// share the timeline and then branch to 2 when they actually diverge in
/// simulated worlds"). Which turn that is, is the engine's to say: every
/// turn a half holds comes with a print of its world, and two halves hold
/// the same world on a turn they both give the same print for.
#[test]
fn halves_say_up_to_which_turn_they_hold_the_same_world() {
    let main = small_table();
    let mut panes = super::Panes::default();
    let none = Value::Null;
    let step = json!({"op": "step"});
    asked_of(&mut panes, &main, "a-x", "GET", "/state", &none);
    asked_of(&mut panes, &main, "b-x", "GET", "/state", &none);
    assert_eq!(
        prints_of(&mut panes, &main, "a-x"),
        prints_of(&mut panes, &main, "b-x"),
        "the turns behind a split are the same world in both halves"
    );

    // Each half is asked to play for itself. Nothing ties the two sessions
    // together any more, and they are the same world turn after turn.
    for turn in 2..=5 {
        for pane in ["a-x", "b-x"] {
            let stepped = asked_of(&mut panes, &main, pane, "POST", "/debug", &step);
            assert_eq!(stepped["turn"], json!(turn));
        }
    }
    assert!(!panes.alike("a-x", "b-x"));
    let together = prints_of(&mut panes, &main, "a-x");
    assert_eq!(together, prints_of(&mut panes, &main, "b-x"));
    assert_eq!(
        together.iter().map(|(turn, _)| *turn).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    let mut seen = std::collections::BTreeSet::new();
    assert!(
        together.iter().all(|(_, world)| seen.insert(*world)),
        "every turn is a world of its own"
    );

    // B ends a turn as it stands, where A's agent plays it: the worlds part
    // on the turn that opens, and stay apart however alike they are played
    // from there. The turns before it are still the same world in both.
    let ended = json!({"action": {"type": "end_turn"}});
    asked_of(&mut panes, &main, "a-x", "POST", "/debug", &step);
    asked_of(&mut panes, &main, "b-x", "POST", "/action", &ended);
    for pane in ["a-x", "b-x"] {
        asked_of(&mut panes, &main, pane, "POST", "/debug", &step);
    }
    let (a, b) = (
        prints_of(&mut panes, &main, "a-x"),
        prints_of(&mut panes, &main, "b-x"),
    );
    assert_eq!(a[..5], together[..]);
    assert_eq!(b[..5], together[..]);
    assert_eq!((a[5].0, b[5].0), (6, 6));
    assert_ne!(a[5].1, b[5].1, "turn 6 opened on two worlds");
    assert_eq!((a[6].0, b[6].0), (7, 7));
    assert_ne!(a[6].1, b[6].1, "and so did turn 7");

    // Walking back changes nothing that is held, and the turn played again
    // from a world both hold is the same world again only if the play is.
    let back = json!({"op": "jump", "turn": 5});
    for pane in ["a-x", "b-x"] {
        asked_of(&mut panes, &main, pane, "POST", "/debug", &back);
    }
    assert_eq!(prints_of(&mut panes, &main, "a-x"), a);
    assert_eq!(prints_of(&mut panes, &main, "b-x"), b);
    for pane in ["a-x", "b-x"] {
        asked_of(&mut panes, &main, pane, "POST", "/debug", &step);
    }
    let (a, b) = (
        prints_of(&mut panes, &main, "a-x"),
        prints_of(&mut panes, &main, "b-x"),
    );
    assert_eq!(a.len(), 6, "the turns ahead were played over");
    assert_eq!(a, b, "one world again, from the turn both went back to");

    // A copy holds what it was copied from.
    asked_of(&mut panes, &main, "a-x", "POST", "/action", &ended);
    let copy = json!({"op": "copy", "from": "a-x"});
    asked_of(&mut panes, &main, "b-x", "POST", "/debug", &copy);
    assert_eq!(
        prints_of(&mut panes, &main, "a-x"),
        prints_of(&mut panes, &main, "b-x")
    );
}

/// Doctor Strange's Civ 6 keeps a dozen worlds as halves of one split, and
/// the socket server answers each page on a thread of its own. A request a
/// half answers alone holds that half and nothing else (`Panes::alone`), so
/// the worlds play at the same time; anything that concerns two halves, or a
/// half not yet seated, is answered as before, holding them all.
#[test]
fn a_half_asked_something_of_its_own_is_answered_alone() {
    let main = small_table();
    let mut panes = super::Panes::default();
    fn asked<'a>(method: &'a str, path: &'a str, parsed: &'a Value) -> super::Asked<'a> {
        super::Asked {
            method,
            path,
            target: path,
            parsed,
        }
    }
    let nothing = Value::Null;
    let step = json!({"op": "step"});
    let copy = json!({"op": "copy", "from": "a-z"});
    let twin = json!({"op": "step", "twin": "b-z"});

    // A half not yet seated is seated holding the main session.
    assert!(panes
        .alone("a-z", &asked("GET", "/state", &nothing))
        .is_none());
    asked_of(&mut panes, &main, "a-z", "GET", "/state", &Value::Null);
    asked_of(&mut panes, &main, "b-z", "GET", "/state", &Value::Null);
    assert!(
        panes.alike("a-z", "b-z"),
        "both are the main session as it stood"
    );

    // Reading is answered alone, and leaves the two alike.
    assert!(panes
        .alone("a-z", &asked("GET", "/state", &nothing))
        .is_some());
    assert!(panes.alike("a-z", "b-z"));
    // Playing one of two alike halves parts them first, which is `serve`'s.
    assert!(panes
        .alone("a-z", &asked("POST", "/debug", &step))
        .is_none());
    // So are a copy and a request that names its twin.
    assert!(panes
        .alone("b-z", &asked("POST", "/debug", &copy))
        .is_none());
    assert!(panes
        .alone("a-z", &asked("POST", "/debug", &twin))
        .is_none());

    // Once apart, a half that plays is answered alone, and is put on a line
    // of its own as `serve` would put it.
    asked_of(
        &mut panes,
        &main,
        "a-z",
        "POST",
        "/debug",
        &json!({"op": "step"}),
    );
    assert!(!panes.alike("a-z", "b-z"));
    let session = panes
        .alone("b-z", &asked("POST", "/debug", &step))
        .expect("a half of its own is answered alone");
    let played = super::answer(
        &mut super::held(&session),
        &asked("POST", "/debug", &step),
        None,
    )
    .expect("a world request")
    .0;
    assert_eq!(played["turn"], json!(2));
    assert!(!panes.alike("a-z", "b-z"));
    assert_eq!(
        asked_of(&mut panes, &main, "b-z", "GET", "/state", &Value::Null)["turn"],
        json!(2)
    );

    // A half answered alone still closes the split as any other does.
    drop(session);
    let mut main = main;
    assert_eq!(panes.adopt("b-z", &mut main), Ok(()));
    assert_eq!(main.game.turn, 2);
}
