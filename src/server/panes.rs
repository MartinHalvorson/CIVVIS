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
//! Two halves that are the same world need playing only once. Each half
//! carries the number of the line it is on (`Half::line`): every request that
//! may change a half gives it a number no other half has, and a copy takes
//! the number of what it was copied from. Two halves with one number are
//! therefore the same world and timeline with the same agents seated, and
//! nothing has been asked of either since. A debugger request from one half may name the other as its
//! `twin`; when the two are on one line the request is answered once and the
//! twin is made a copy of the result, which costs a fork instead of a turn
//! (`serve`). The answer says whether that happened (`debug.twinned`).
//!
//! A copy is seated with agents of its own (`Session::fork`), and an agent
//! keeps plans from turn to turn. So of two halves played as one, only the
//! half that was asked has agents that lived through those turns. When the
//! two are then asked to play apart, that half is seated anew as well
//! (`Panes::part`): from there nothing but what is asked of each tells them
//! apart, which is what a head to head of two Civ AIs rests on.
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
use std::sync::{Arc, Mutex, MutexGuard};

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

/// One half of a split: its session, and the line it is on.
///
/// The session has a lock of its own. Doctor Strange's Civ 6 keeps up to a
/// dozen worlds as halves of one split, and the socket server answers each
/// page on a thread of its own: a request that plays one world holds that
/// world alone (`socket::route`), so the worlds play at the same time, and
/// the page that holds them is not kept waiting behind a world's turn.
struct Half {
    session: Arc<Mutex<Session>>,
    /// `UNTOUCHED` for a half that is still the main session as it stood
    /// when the screen was split. Otherwise the number the last request that
    /// may have changed this half was given, or the number of the half this
    /// one was copied from.
    line: u64,
    /// Whether the session is as `Session::fork` made it: nothing that may
    /// change it has been asked of it, and its agents hold no plans yet.
    fresh: bool,
}

/// The line of a half nothing has been asked to change since it was forked.
const UNTOUCHED: u64 = 0;

/// A half's session, held. A request that panicked part-way left the world
/// as it stood, which is what the next request is answered from.
fn held(session: &Mutex<Session>) -> MutexGuard<'_, Session> {
    session
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A half's own session, kept behind a lock of its own.
fn kept(session: Session) -> Arc<Mutex<Session>> {
    Arc::new(Mutex::new(session))
}

/// The sessions kept beside the main one, by pane name.
#[derive(Default)]
pub struct Panes {
    halves: BTreeMap<String, Half>,
    /// The last line number given out.
    lines: u64,
}

/// The pane a request is for: named on the request itself, or by the page
/// that sent it.
pub(super) fn named<'a>(target: &'a str, referer: Option<&'a str>) -> Option<&'a str> {
    query_value(target, "pane")
        .or_else(|| referer.and_then(|page| query_value(page, "pane")))
        .filter(|pane| well_formed(pane))
}

/// A lowercase letter and `-`, then the split's own tag. A split is two
/// halves, `a-` and `b-`; a page that sets more worlds than two side by side
/// (Doctor Strange's Civ 6) names the rest on down the alphabet.
fn well_formed(pane: &str) -> bool {
    let bytes = pane.as_bytes();
    (3..=24).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1] == b'-'
        && bytes[2..].iter().all(u8::is_ascii_alphanumeric)
}

/// The other half of `pane`'s split, when `name` is that.
fn other_half<'a>(pane: &str, name: &'a str) -> Option<&'a str> {
    let halves = well_formed(pane) && well_formed(name);
    (halves && name != pane && name[2..] == pane[2..]).then_some(name)
}

/// Whether a request leaves the world it is answered from as it was. These
/// are the routes that take the session by shared reference in `answer`.
fn only_reads(method: &str, path: &str) -> bool {
    method == "GET" || matches!(path, "/route" | "/forecast" | "/intel")
}

/// Whether a request is one a pane answers from its own session.
pub(super) fn concerns_a_world(method: &str, path: &str) -> bool {
    const WORLD: [(&str, &str); 21] = [
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
        ("POST", "/fog-honest"),
        ("POST", "/new"),
        ("POST", "/supervisor-new"),
    ];
    WORLD.contains(&(method, path))
}

impl Panes {
    /// The pane's session, forked from the main one the first time the pane
    /// asks. A new split replaces the panes of the one before it.
    ///
    /// The second half of a split is forked from the first while nothing has
    /// been asked to change the first: that is the main session as it stood
    /// when the screen was split, and it makes the two halves the same world
    /// whatever the main session has been asked since.
    pub(super) fn seat(
        &mut self,
        pane: &str,
        main: &Session,
    ) -> Result<Arc<Mutex<Session>>, String> {
        if !self.halves.contains_key(pane) {
            if main.params.spectate {
                return Err("a split forks a game with a seat of its own".to_string());
            }
            let split = &pane[2..];
            self.halves.retain(|name, _| &name[2..] == split);
            let begun = match self.halves.values().find(|half| half.line == UNTOUCHED) {
                Some(first) => held(&first.session).fork(),
                None => main.fork(),
            };
            // A first half that has already been played is not this world.
            let line = if self.halves.values().all(|half| half.line == UNTOUCHED) {
                UNTOUCHED
            } else {
                self.next_line()
            };
            self.halves.insert(
                pane.to_string(),
                Half {
                    session: kept(begun),
                    line,
                    fresh: true,
                },
            );
        }
        Ok(Arc::clone(
            &self
                .halves
                .get(pane)
                .expect("the pane was just seated")
                .session,
        ))
    }

    /// The session of a half that a request may be answered from with
    /// nothing else held: the half is seated, the request copies nothing and
    /// names no twin, and nothing about it parts this half from another.
    /// A request that may change the half puts it on a line of its own here,
    /// as `serve` does once it has answered.
    fn alone(&mut self, pane: &str, asked: &Asked) -> Option<Arc<Mutex<Session>>> {
        let (method, path, parsed) = (asked.method, asked.path, asked.parsed);
        let line = self.halves.get(pane)?.line;
        if (method, path) == ("POST", "/debug")
            && (parsed["op"] == json!("copy") || parsed["twin"].is_string())
        {
            return None;
        }
        let changes = !only_reads(method, path);
        let shared = self
            .halves
            .iter()
            .any(|(name, half)| name != pane && half.line == line);
        if changes && shared {
            return None;
        }
        if changes {
            self.changed(pane);
        }
        self.halves.get(pane).map(|half| Arc::clone(&half.session))
    }

    /// A line number no half is on.
    fn next_line(&mut self) -> u64 {
        self.lines += 1;
        self.lines
    }

    /// A request that may have changed this half was answered from it: the
    /// half is on a line of its own until it is copied, or copied from.
    fn changed(&mut self, pane: &str) {
        let line = self.next_line();
        if let Some(half) = self.halves.get_mut(pane) {
            half.line = line;
            half.fresh = false;
        }
    }

    /// This half is about to be asked something the other is not, while the
    /// two are alike. Of two halves played as one, the half that was asked
    /// has agents that lived through those turns and the half that was
    /// copied has agents seated anew, so the one that played is seated anew
    /// too. The world and the timeline are not touched.
    fn part(&mut self, pane: &str) {
        let Some(line) = self.halves.get(pane).map(|half| half.line) else {
            return;
        };
        let on_the_line = |half: &&Half| half.line == line;
        if self.halves.values().filter(on_the_line).count() < 2 {
            return;
        }
        for half in self.halves.values_mut() {
            if half.line == line && !half.fresh {
                let again = held(&half.session).fork();
                half.session = kept(again);
                half.fresh = true;
            }
        }
    }

    /// Whether two halves are the same world and timeline, with the same
    /// agents seated: one was made from the other, or both from the main
    /// session, and nothing has been asked to change either since.
    pub(super) fn alike(&self, a: &str, b: &str) -> bool {
        a != b
            && match (self.halves.get(a), self.halves.get(b)) {
                (Some(a), Some(b)) => a.line == b.line,
                _ => false,
            }
    }

    /// Make `to` a copy of `from` as it stands: the same world, agents and
    /// timeline. From there the two play apart again.
    pub(super) fn copy(&mut self, from: &str, to: &str) -> Result<(), String> {
        if other_half(to, from).is_none() {
            return Err("copy names the other half of this split".to_string());
        }
        let Some(source) = self.halves.get(from) else {
            return Err("the other half of this split has not started".to_string());
        };
        let copied = Half {
            session: kept(held(&source.session).fork()),
            line: source.line,
            fresh: true,
        };
        self.halves.insert(to.to_string(), copied);
        Ok(())
    }

    /// Close the split onto one of its halves: that half's session becomes
    /// the main one, with every turn it can walk back to, and the other half
    /// is let go.
    pub(super) fn adopt(&mut self, pane: &str, main: &mut Session) -> Result<(), String> {
        let Some(half) = self.halves.remove(pane) else {
            return Err("no half of a split is called that".to_string());
        };
        // A request this half is still being answered on holds it until
        // that answer is given; nothing can find the half here any more.
        let mut session = half.session;
        *main = loop {
            match Arc::try_unwrap(session) {
                Ok(only) => {
                    break only
                        .into_inner()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                }
                Err(shared) => {
                    session = shared;
                    #[cfg(not(target_arch = "wasm32"))]
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        };
        main.debug_remembers();
        self.halves.clear();
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.halves.len()
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
        ("POST", "/fog-honest") => whole(crate::routes::fog_honest(session, parsed)),
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
///
/// A debugger request may name the other half as its `twin`. When the two
/// halves are alike as the request arrives, it is answered from this half
/// and the twin is made a copy of what this half then holds, so the two are
/// alike again and only one of them was played. When they are not alike the
/// twin is left as it is, for its own page to ask the same of. Either way
/// the answer's `debug.twinned` says which.
///
/// Any other request that may change this half while the two are alike
/// parts them (`Panes::part`).
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
        return Some(match (copied, panes.halves.get(pane)) {
            (Ok(()), Some(half)) => {
                let mut out = held(&half.session).state();
                out["error"] = Value::Null;
                (out, true)
            }
            (Err(error), _) => (json!({"error": error}), false),
            (Ok(()), None) => (json!({"error": "the copy was not seated"}), false),
        });
    }
    let twin = if (method, path) == ("POST", "/debug") {
        parsed["twin"].as_str()
    } else {
        None
    };
    // Read before this half is asked anything: the request is about to put
    // it on a line of its own.
    let alike = twin.is_some_and(|twin| panes.alike(pane, twin));
    let changes = !only_reads(method, path);
    if changes && !alike {
        panes.part(pane);
    }
    let mut answered = match panes.seat(pane, main) {
        Ok(session) => answer(&mut held(&session), asked, server_instance),
        Err(error) => return Some((json!({"error": error}), false)),
    };
    if changes {
        panes.changed(pane);
    }
    if let Some(twin) = twin {
        let twinned = alike && panes.copy(pane, twin).is_ok();
        if let Some((out, _)) = answered.as_mut() {
            if out["debug"].is_object() {
                out["debug"]["twinned"] = json!(twinned);
            }
        }
    }
    answered
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
        // A half that is there, asked something of its own alone, is
        // answered holding that half and nothing else: another world's turn,
        // and the page's own requests, go on beside it.
        if !named_save {
            if let Some(session) = panes.alone(pane, asked) {
                drop(panes);
                let (mut out, whole) =
                    super::answer(&mut super::held(&session), asked, Some(process_identity()))?;
                if whole {
                    decorate(&mut out, sh);
                }
                return Some(out);
            }
        }
        let (mut out, whole) = if named_save {
            let seated = {
                let main = lock_or_recover(&sh.session);
                match panes.seat(pane, &main) {
                    Ok(session) => session,
                    Err(error) => return Some(json!({"error": error})),
                }
            };
            let mut session = super::held(&seated);
            let session = &mut *session;
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
                // A loaded game is another world: this half is on a line of
                // its own, as it is after any request `serve` answers.
                panes.changed(pane);
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
