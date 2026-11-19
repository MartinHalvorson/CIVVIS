//! Side sessions for the split Sim debugger: a world of its own for each half of the screen.
//!
//! The split view (`web/assets/app_debugger.js`) shows two panes, and each is
//! a whole page with a simulator of its own. On the published build that is an
//! engine per page. Here both pages would reach the one session this process
//! plays, so a request from a pane is answered from a session kept for that
//! pane instead.
//!
//! A pane names itself in its address (`?pane=a-<split>`), and so every
//! request its page makes carries the name in `Referer` without the page doing
//! anything. A pane's world begins as a fork of the main session the first
//! time it is asked for (`Session::fork`: the same world, the same agents, the
//! timeline behind it), and from then on it is its own. A new split replaces
//! the panes of the one before, so at most two are ever held.
//!
//! Only the routes that read or change a world are answered here. Everything
//! else — the page's files, the process's identity, the saves on disk — is the
//! same for a pane as for any page and falls through to the main router.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::{json, Value};

use super::{
    decorate, lock_or_recover, process_identity, query_value, save_path, write_save, Session,
    Shared,
};

/// The sessions kept beside the main one, by pane name.
#[derive(Default)]
pub struct Panes(Mutex<BTreeMap<String, Session>>);

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

/// Answer a pane's request from its own session. `None` is a request that
/// does not concern a world, which the main router answers as it would for
/// anyone.
pub(super) fn route(
    sh: &Shared,
    pane: &str,
    method: &str,
    path: &str,
    target: &str,
    parsed: &Value,
) -> Option<Value> {
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
    if !WORLD.contains(&(method, path)) {
        return None;
    }
    let mut panes = lock_or_recover(&sh.panes.0);
    if !panes.contains_key(pane) {
        let forked = {
            let main = lock_or_recover(&sh.session);
            if main.params.spectate {
                return Some(json!({
                    "error": "a split forks a game with a seat of its own",
                }));
            }
            main.fork()
        };
        // A new split replaces the one before it.
        let split = &pane[2..];
        panes.retain(|name, _| &name[2..] == split);
        panes.insert(pane.to_string(), forked);
    }
    let session = panes.get_mut(pane).expect("the pane was just seated");
    let mut out = match (method, path) {
        ("GET", "/state") => {
            let mut observed = session.state();
            if query_value(target, "planet") == Some("1") {
                if let Some(geometry) = crate::obs::planet_geometry(&session.game) {
                    observed["map"]["planet"] = geometry;
                }
            }
            observed
        }
        ("GET", "/rules") => crate::routes::rules(session, true),
        ("GET", "/pedia") => crate::routes::pedia(session),
        ("GET", "/policy-effects") => crate::routes::policy_effects(session),
        ("GET", "/map-search-buildings") => crate::routes::map_search_buildings(session),
        // Answered bare, like the main route: a save is not a state document.
        ("GET", "/save") => return Some(crate::routes::save(session)),
        ("POST", "/save") => {
            let name = parsed["name"].as_str().unwrap_or("");
            return Some(match save_path(name) {
                None => json!({"error": "a save name is letters, digits, - and _"}),
                Some(file) => match write_save(&session.game, &file) {
                    Ok(()) => {
                        json!({"error": Value::Null, "name": name, "turn": session.game.turn})
                    }
                    Err(error) => json!({"error": format!("cannot write {name}: {error}")}),
                },
            });
        }
        ("POST", "/load") => {
            let result = match parsed["name"].as_str() {
                None => crate::routes::load_uploaded(session, parsed),
                Some(name) => load_named(session, name),
            };
            let mut out = session.state();
            out["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => json!(error),
            };
            out
        }
        ("POST", "/action") => crate::routes::action(session, parsed).out,
        ("POST", "/autoplay") => {
            match crate::routes::autoplay(session, parsed, Some(process_identity())) {
                Ok(outcome) => outcome.out,
                Err(error) => error,
            }
        }
        ("POST", "/debug") => crate::routes::debug(session, parsed),
        ("POST", "/play-on") => match crate::routes::play_on(session, parsed) {
            Ok(outcome) => outcome.out,
            Err(error) => error,
        },
        ("POST", "/route") => return Some(crate::routes::route_step(session, parsed)),
        ("POST", "/forecast") => return Some(crate::routes::forecast(session, parsed)),
        ("POST", "/intel") => return Some(crate::routes::intel(session, parsed)),
        ("POST", "/city-focus") => crate::routes::city_focus(session, parsed),
        ("POST", "/city-citizen") => crate::routes::city_citizen(session, parsed),
        ("POST", "/view") => crate::routes::view(session, parsed),
        // A new game is the whole screen's to start, not half of it.
        _ => {
            let mut out = session.state();
            out["error"] = json!("close the split before starting a new game");
            out
        }
    };
    drop(panes);
    decorate(&mut out, sh);
    Some(out)
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

#[cfg(test)]
#[path = "panes_tests.rs"]
mod tests;
