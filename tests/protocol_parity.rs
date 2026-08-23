//! The two front ends serve one protocol, and divergence has to be deliberate.
//!
//! `server.rs` (native) and `wasm.rs` (civvis.ai) each dispatch the same JSON
//! protocol. Nothing compared their route tables, so they drifted quietly:
//! `/next-game` reached only the browser, `/adjacency` and
//! `/saves` only the native server. The roadmap records the consequence as a
//! viewer bug — "panels that read native-only state are silently dead on
//! civvis.ai" — but the cause is that a route can be added to one side and the
//! other side never hears about it.
//!
//! This reads both route tables out of the source and fails when they differ,
//! unless the difference is written down below with a reason. A route that only
//! one build can serve is a legitimate thing to have; a route that only one
//! build serves *by accident* is the bug.

use std::collections::BTreeSet;
use std::path::Path;

/// Routes only the browser build serves, and why.
const WASM_ONLY: &[(&str, &str)] = &[
    (
        "/next-game",
        "The page queues the next simulation itself because a wasm module has no \
         supervisor process behind it. Native queues through the supervisor.",
    ),
];

/// Routes only the native build serves, and why.
const NATIVE_ONLY: &[(&str, &str)] = &[
    (
        "/adjacency",
        "A district-adjacency debugging read that answers from the full map. It \
         has no browser client and would ship map truth the page's fog hides.",
    ),
    (
        "/saves",
        "Lists save files on disk. The browser build has no filesystem; it saves \
         into the page's own storage, which is what `autosave_due` is for.",
    ),
];

fn source(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"))
}

/// Every `("METHOD", "/path")` match arm in a dispatcher, as `/path`.
fn routes(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for (index, _) in text.match_indices("(\"GET\", \"") {
        push_route(text, index + "(\"GET\", \"".len(), &mut found);
    }
    for verb in ["POST", "PUT", "DELETE"] {
        let needle = format!("(\"{verb}\", \"");
        for (index, _) in text.match_indices(needle.as_str()) {
            push_route(text, index + needle.len(), &mut found);
        }
    }
    found
}

fn push_route(text: &str, start: usize, into: &mut BTreeSet<String>) {
    if let Some(end) = text[start..].find('"') {
        let path = &text[start..start + end];
        if path.starts_with('/') {
            into.insert(path.to_string());
        }
    }
}

/// Static files are not the JSON protocol.
///
/// The native server doubles as the web server and hands out `/assets/*`
/// itself; in the browser those come from the page's own origin, long before
/// any wasm module is asked anything. Comparing them would demand that a wasm
/// module serve the file it was itself loaded from.
fn is_protocol_route(path: &str) -> bool {
    !path.starts_with("/assets/")
}

fn named(list: &[(&str, &str)]) -> BTreeSet<String> {
    list.iter().map(|(path, _)| path.to_string()).collect()
}

#[test]
fn every_route_is_served_by_both_builds_or_explained() {
    let native: BTreeSet<String> = routes(&source("server.rs"))
        .into_iter()
        .filter(|path| is_protocol_route(path))
        .collect();
    let wasm: BTreeSet<String> = routes(&source("wasm.rs"))
        .into_iter()
        .filter(|path| is_protocol_route(path))
        .collect();
    assert!(
        native.len() > 15,
        "only {} native routes parsed",
        native.len()
    );
    assert!(wasm.len() > 15, "only {} wasm routes parsed", wasm.len());

    let unexplained_wasm: Vec<_> = wasm
        .difference(&native)
        .filter(|path| !named(WASM_ONLY).contains(*path))
        .cloned()
        .collect();
    assert!(
        unexplained_wasm.is_empty(),
        "these routes reach civvis.ai and not the native server: {unexplained_wasm:?}\n\
         Add them to server.rs, or list them in WASM_ONLY with the reason only \
         the browser can serve them."
    );

    let unexplained_native: Vec<_> = native
        .difference(&wasm)
        .filter(|path| !named(NATIVE_ONLY).contains(*path))
        .cloned()
        .collect();
    assert!(
        unexplained_native.is_empty(),
        "these routes reach the native server and not civvis.ai: {unexplained_native:?}\n\
         A panel that calls one of these is silently dead in the browser. Add \
         them to wasm.rs, or list them in NATIVE_ONLY with the reason."
    );
}

#[test]
fn the_exception_lists_describe_routes_that_exist() {
    let native = routes(&source("server.rs"));
    let wasm = routes(&source("wasm.rs"));
    for (path, _) in WASM_ONLY {
        assert!(
            wasm.contains(*path),
            "WASM_ONLY lists {path}, which wasm.rs no longer serves; drop the entry"
        );
        assert!(
            !native.contains(*path),
            "WASM_ONLY lists {path}, but server.rs serves it now; drop the entry"
        );
    }
    for (path, _) in NATIVE_ONLY {
        assert!(
            native.contains(*path),
            "NATIVE_ONLY lists {path}, which server.rs no longer serves; drop the entry"
        );
        assert!(
            !wasm.contains(*path),
            "NATIVE_ONLY lists {path}, but wasm.rs serves it now; drop the entry"
        );
    }
}

#[test]
fn every_exception_carries_a_reason() {
    // An exception list whose entries say nothing is a list that grows.
    for (path, why) in WASM_ONLY.iter().chain(NATIVE_ONLY.iter()) {
        assert!(
            why.len() > 60,
            "{path} is excused without saying why one build cannot serve it"
        );
    }
}

#[test]
fn the_shared_handlers_are_called_by_both_front_ends() {
    // The point of `routes.rs` is that both sides call it. A handler only one
    // side uses is a handler that has been quietly re-duplicated in the other.
    let native = source("server.rs");
    let wasm = source("wasm.rs");
    for handler in [
        "route_step",
        "view",
        "action",
        "rules",
        "pedia",
        "save",
        "intel",
        "pace",
        "step",
        "autoplay",
        "play_on",
        "spectator_status",
        "next_game_settings",
        "new_game",
        "load_uploaded",
    ] {
        let call = format!("crate::routes::{handler}(");
        assert!(
            native.contains(&call),
            "server.rs no longer calls routes::{handler}; the native copy is back"
        );
        assert!(
            wasm.contains(&call),
            "wasm.rs no longer calls routes::{handler}; the browser copy is back"
        );
    }
}
