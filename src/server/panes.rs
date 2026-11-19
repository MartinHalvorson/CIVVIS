//! Side sessions for the split Sim debugger: a world of its own for each half of the screen.
//!
//! The split view (`web/assets/app_debugger.js`) shows two panes, each a whole
//! page of its own, and every page asks the one engine behind it for the
//! world. So that the two halves can play different histories, a request from
//! a pane is answered from a session kept for that pane instead of from the
//! main one.
//!
//! A pane's session begins as a fork of the main session the first time it is
//! asked for (`Session::fork`): the same world, the same agents, and the same
//! timeline behind it to walk back along. From then on it is its own. Because
//! the halves are sessions of one engine rather than engines of their own,
//! nothing is lost when they change places: one half can be made a copy of the
//! other (`copy`), and the split closes by making the half that is kept the
//! main session, timeline and all (`adopt`).
//!
//! Both front ends keep their panes here. A pane names itself in its address
//! (`?pane=a-<split>`). On the socket server every request its page makes then
//! carries the name in `Referer`, with the page doing nothing; on the
//! published build the page's shim hands its requests to the engine of the
//! page that framed it, with the name on each one (`beta/shim.js`).
//!
//! Only the routes that read or change a world are answered from a pane.
//! Everything else — the page's files, the process's identity, the saves in
//! storage — is the same for a pane as for any page and is left to the main
//! router.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{query_value, Session};

/// One request, as either front end reads it off its transport.
#[derive(Clone, Copy)]
pub(super) struct Asked<'a> {
    pub method: &'a str,
    pub path: &'a str,
    /// The path with its query, which several routes read their parameters from.
    pub target: &'a str,
    pub parsed: &'a Value,
}

/// The sessions kept beside the main one, by pane name.
#[derive(Default)]
pub struct Panes(BTreeMap<String, Session>);

/// The pane a request is for: named on the request itself, or by the page
/// that sent it.
pub(super) fn named<'a>(target: &'a str, referer: Option<&'a str>) -> Option<&'a str> {
    query_value(target, "pane")
        .or_else(|| referer.and_then(|page| query_value(page, "pane")))
        .filter(|pane| well_formed(pane))
}

/// `a-` or `b-`, then the split's own tag.
fn well_formed(pane: &str) -> bool {
    (3..=24).contains(&pane.len())
        && (pane.starts_with("a-") || pane.starts_with("b-"))
        && pane[2..].bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Whether a request is one a pane answers from its own session.
pub(super) fn concerns_a_world(method: &str, path: &str) -> bool {
    const WORLD: [(&str, &str); 20] = [
        ("GET", "/state"),
        ("GET", "/rules"),
        ("GET", "/pedia"),
        ("GET", "/policy-effects"),
        ("GET", "/map-search-buildings"),
        ("GET", "/save"),
        ("POST", "/save"),
        ("POST", "/load"),
        ("POST", "/action"),
        ("POST", "/autoplay"),
        ("POST", "/debug"),
        ("POST", "/play-on"),
        ("POST", "/route"),
        ("POST", "/forecast"),
        ("POST", "/intel"),
        ("POST", "/city-focus"),
        ("POST", "/city-citizen"),
        ("POST", "/view"),
        ("POST", "/new"),
        ("POST", "/supervisor-new"),
    ];
    WORLD.contains(&(method, path))
}

impl Panes {
    /// The pane's session, forked from the main one the first time the pane
    /// asks. A new split replaces the panes of the one before it.
    pub(super) fn seat(&mut self, pane: &str, main: &Session) -> Result<&mut Session, String> {
        if !self.0.contains_key(pane) {
            if main.params.spectate {
                return Err("a split forks a game with a seat of its own".to_string());
            }
            let split = &pane[2..];
            self.0.retain(|name, _| &name[2..] == split);
            self.0.insert(pane.to_string(), main.fork());
        }
        Ok(self.0.get_mut(pane).expect("the pane was just seated"))
    }

    /// Make `to` a copy of `from` as it stands: the same world, agents and
    /// timeline. From there the two play apart again.
    pub(super) fn copy(&mut self, from: &str, to: &str) -> Result<(), String> {
        if !well_formed(from) || from == to || from[2..] != to[2..] {
            return Err("copy names the other half of this split".to_string());
        }
        let Some(source) = self.0.get(from) else {
            return Err("the other half of this split has not started".to_string());
        };
        let copied = source.fork();
        self.0.insert(to.to_string(), copied);
        Ok(())
    }

    /// Close the split onto one of its halves: that half's session becomes
    /// the main one, with every turn it can walk back to, and the other half
    /// is let go.
    pub(super) fn adopt(&mut self, pane: &str, main: &mut Session) -> Result<(), String> {
        let Some(kept) = self.0.remove(pane) else {
            return Err("no half of a split is called that".to_string());
        };
        *main = kept;
        self.0.clear();
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
}

/// Answer a world request from one session, the same way for either front
/// end. `None` is a request this does not serve; the two things a front end
/// owns itself, the saves in its storage and the state it decorates an answer
/// with, stay with the caller.
///
/// `whole` says whether the answer is a state document, which callers then
/// decorate exactly as they do the main session's; a save or a route preview
/// is answered bare.
///
/// `server_instance` is the socket server's process identity, which its
/// auto-play checks a request against and which the published build has none
/// of; it is also what says whose `/rules` this is.
pub(super) fn answer(
    session: &mut Session,
    asked: &Asked,
    server_instance: Option<u32>,
) -> Option<(Value, bool)> {
    let whole = |out: Value| Some((out, true));
    let bare = |out: Value| Some((out, false));
    let Asked {
        method,
        path,
        target,
        parsed,
    } = *asked;
    match (method, path) {
        ("GET", "/state") => {
            let mut observed = session.state();
            if query_value(target, "planet") == Some("1") {
                if let Some(geometry) = crate::obs::planet_geometry(&session.game) {
                    observed["map"]["planet"] = geometry;
                }
            }
            whole(observed)
        }
        ("GET", "/rules") => whole(crate::routes::rules(session, server_instance.is_some())),
        ("GET", "/pedia") => whole(crate::routes::pedia(session)),
        ("GET", "/policy-effects") => whole(crate::routes::policy_effects(session)),
        ("GET", "/map-search-buildings") => whole(crate::routes::map_search_buildings(session)),
        ("GET", "/save") => bare(crate::routes::save(session)),
        // Only an uploaded game: a save named in storage is the front end's
        // to read, and it arrives here as the game it held.
        ("POST", "/load") => {
            let result = crate::routes::load_uploaded(session, parsed);
            let mut out = session.state();
            out["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => json!(error),
            };
            whole(out)
        }
        ("POST", "/action") => whole(crate::routes::action(session, parsed).out),
        ("POST", "/autoplay") => whole(
            match crate::routes::autoplay(session, parsed, server_instance) {
                Ok(outcome) => outcome.out,
                Err(error) => error,
            },
        ),
        ("POST", "/debug") => whole(crate::routes::debug(session, parsed)),
        ("POST", "/play-on") => whole(match crate::routes::play_on(session, parsed) {
            Ok(outcome) => outcome.out,
            Err(error) => error,
        }),
        ("POST", "/route") => bare(crate::routes::route_step(session, parsed)),
        ("POST", "/forecast") => bare(crate::routes::forecast(session, parsed)),
        ("POST", "/intel") => bare(crate::routes::intel(session, parsed)),
        ("POST", "/city-focus") => whole(crate::routes::city_focus(session, parsed)),
        ("POST", "/city-citizen") => whole(crate::routes::city_citizen(session, parsed)),
        ("POST", "/view") => whole(crate::routes::view(session, parsed)),
        // A new game is the whole screen's to start, not half of it.
        ("POST", "/new") | ("POST", "/supervisor-new") => {
            let mut out = session.state();
            out["error"] = json!("close the split before starting a new game");
            whole(out)
        }
        _ => None,
    }
}

/// One pane's request against the main session and the panes beside it.
///
/// This is the whole of a pane's routing for a front end that holds both
/// behind one borrow, which the published build does; the socket server
/// wraps the same steps in its locks and adds the saves on its disk
/// (`socket`). `None` is a request that does not concern a world.
pub(super) fn serve(
    panes: &mut Panes,
    main: &Session,
    pane: &str,
    asked: &Asked,
    server_instance: Option<u32>,
) -> Option<(Value, bool)> {
    let (method, path, parsed) = (asked.method, asked.path, asked.parsed);
    if !concerns_a_world(method, path) {
        return None;
    }
    // One half made a copy of the other: the copy is seated first, and the
    // answer is the world it now holds.
    if (method, path) == ("POST", "/debug") && parsed["op"] == json!("copy") {
        let copied = panes.copy(parsed["from"].as_str().unwrap_or(""), pane);
        return Some(match (copied, panes.0.get(pane)) {
            (Ok(()), Some(session)) => {
                let mut out = session.state();
                out["error"] = Value::Null;
                (out, true)
            }
            (Err(error), _) => (json!({"error": error}), false),
            (Ok(()), None) => (json!({"error": "the copy was not seated"}), false),
        });
    }
    let session = match panes.seat(pane, main) {
        Ok(session) => session,
        Err(error) => return Some((json!({"error": error}), false)),
    };
    answer(session, asked, server_instance)
}

/// The main session's own word in a split: `POST /debug {"op": "adopt",
/// "pane": …}` closes the split onto that half. `None` is any other request.
pub(super) fn adopts(parsed: &Value) -> Option<&str> {
    (parsed["op"] == json!("adopt")).then(|| parsed["pane"].as_str().unwrap_or(""))
}

/// The socket server's side: its locks, and the saves on its disk.
pub(super) mod socket {
    use std::sync::Mutex;

    use serde_json::{json, Value};

    use super::super::{
        decorate, lock_or_recover, process_identity, save_path, write_save, Session, Shared,
    };
    use super::{Asked, Panes};

    /// The panes of a socket server, behind the lock its threads share.
    #[derive(Default)]
    pub struct Held(pub(in super::super) Mutex<Panes>);

    /// Answer a pane's request, or `None` for one that does not concern a
    /// world. The panes are locked before the main session, here and in
    /// `adopt`, so the two never wait on each other.
    pub(in super::super) fn route(sh: &Shared, pane: &str, asked: &Asked) -> Option<Value> {
        let (method, path, parsed) = (asked.method, asked.path, asked.parsed);
        if !super::concerns_a_world(method, path) {
            return None;
        }
        let mut panes = lock_or_recover(&sh.panes.0);
        // The saves on this server's disk, by name.
        let named_save = match (method, path) {
            ("POST", "/save") => true,
            ("POST", "/load") => parsed["name"].is_string(),
            _ => false,
        };
        let (mut out, whole) = if named_save {
            let session = {
                let main = lock_or_recover(&sh.session);
                match panes.seat(pane, &main) {
                    Ok(session) => session,
                    Err(error) => return Some(json!({"error": error})),
                }
            };
            let name = parsed["name"].as_str().unwrap_or("");
            if path == "/save" {
                let answer = match save_path(name) {
                    None => json!({"error": "a save name is letters, digits, - and _"}),
                    Some(file) => match write_save(&session.game, &file) {
                        Ok(()) => {
                            json!({"error": Value::Null, "name": name, "turn": session.game.turn})
                        }
                        Err(error) => json!({"error": format!("cannot write {name}: {error}")}),
                    },
                };
                (answer, false)
            } else {
                let result = load_named(session, name);
                let mut out = session.state();
                out["error"] = match result {
                    Ok(()) => Value::Null,
                    Err(error) => json!(error),
                };
                (out, true)
            }
        } else {
            let main = lock_or_recover(&sh.session);
            super::serve(&mut panes, &main, pane, asked, Some(process_identity()))?
        };
        drop(panes);
        if whole {
            decorate(&mut out, sh);
        }
        Some(out)
    }

    /// Close the split onto one half, and answer with the world that is now
    /// the main one.
    pub(in super::super) fn adopt(sh: &Shared, pane: &str) -> Value {
        let mut panes = lock_or_recover(&sh.panes.0);
        let mut main = lock_or_recover(&sh.session);
        let result = panes.adopt(pane, &mut main);
        let mut out = main.state();
        out["error"] = match result {
            Ok(()) => Value::Null,
            Err(error) => json!(error),
        };
        drop(main);
        drop(panes);
        decorate(&mut out, sh);
        out
    }

    /// Load one of this server's saves into a pane, under the rule the main
    /// route keeps: a save played under other mods is refused.
    fn load_named(session: &mut Session, name: &str) -> Result<(), String> {
        let file = save_path(name).ok_or("a save name is letters, digits, - and _")?;
        let raw = std::fs::read(&file).map_err(|error| format!("cannot read {name}: {error}"))?;
        let save = serde_json::from_slice::<Value>(&raw)
            .map_err(|error| format!("{name} is not JSON: {error}"))?;
        crate::routes::load_uploaded(session, &json!({"game": save}))
            .map_err(|error| format!("{name}: {error}"))
    }
}

#[cfg(test)]
#[path = "panes_tests.rs"]
mod tests;
