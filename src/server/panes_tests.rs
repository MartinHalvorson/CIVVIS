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
    for stray in ["c-1", "a", "a-", "a-../x", "ab-1", "a-%20"] {
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
