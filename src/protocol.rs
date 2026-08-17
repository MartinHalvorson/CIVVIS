//! Versioning and compatibility helpers for CIVVIS's external JSON surfaces.
//!
//! The engine's internal structs are intentionally serialized directly so
//! saves remain useful to the Rust code.  Anything that crosses an HTTP/WASM
//! boundary, or is kept as a save file, gets a small envelope instead.  That
//! gives clients a way to reject a newer contract before interpreting fields,
//! while the loader still accepts the pre-envelope saves shipped by older
//! builds.

use serde_json::{json, Map, Value};

use crate::game::Game;

/// The name carried by responses and save envelopes.
pub const PROTOCOL_NAME: &str = "civvis-json";
/// The highest request/response protocol version this build understands.
pub const PROTOCOL_VERSION: u32 = 1;
/// The save envelope version.  This is separate because a save can outlive
/// the HTTP API that produced it.
pub const SAVE_FORMAT_VERSION: u32 = 1;
const SAVE_FORMAT_NAME: &str = "civvis.save";

/// Add the current protocol identity to an externally visible JSON object.
///
/// Responses keep their existing top-level shape: clients can read the fields
/// they already know and use these two fields to choose whether to continue.
/// Non-object values are returned unchanged because every current public
/// response is an object and inventing a wrapper for an array would be a
/// breaking change of its own.
pub fn version_response(mut value: Value) -> Value {
    if let Value::Object(object) = &mut value {
        object
            .entry("protocol".to_string())
            .or_insert_with(|| json!(PROTOCOL_NAME));
        object
            .entry("protocol_version".to_string())
            .or_insert_with(|| json!(PROTOCOL_VERSION));
    }
    value
}

/// Validate the optional version marker on an incoming request.
///
/// Missing markers are deliberately accepted for old browser builds and
/// integrations.  A caller that sends a marker is asking for versioned
/// behavior, so malformed markers and versions newer than this binary are
/// refused before any action is decoded.
pub fn validate_request(value: &Value) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    let Some(version) = object.get("protocol_version") else {
        return Ok(());
    };
    let Some(version) = version.as_u64() else {
        return Err("protocol_version must be a non-negative integer".to_string());
    };
    if version == 0 || version > u64::from(PROTOCOL_VERSION) {
        return Err(format!(
            "unsupported protocol_version {version}; this server supports {PROTOCOL_VERSION}"
        ));
    }
    Ok(())
}

/// Serialize a game in the durable save format.
pub fn save_value(game: &Game) -> serde_json::Result<Value> {
    Ok(json!({
        "format": SAVE_FORMAT_NAME,
        "protocol": PROTOCOL_NAME,
        "protocol_version": PROTOCOL_VERSION,
        "save_format_version": SAVE_FORMAT_VERSION,
        "game": serde_json::to_value(game)?,
    }))
}

fn version_field(object: &Map<String, Value>, field: &str, expected: u32) -> Result<(), String> {
    let Some(value) = object.get(field) else {
        return Ok(());
    };
    let Some(value) = value.as_u64() else {
        return Err(format!("{field} must be a non-negative integer"));
    };
    if value != u64::from(expected) {
        return Err(format!(
            "unsupported {field} {value}; this build reads {expected}"
        ));
    }
    Ok(())
}

/// Decode either a current save envelope or the raw `Game` JSON written by
/// older CIVVIS builds.
pub fn game_from_save(value: Value) -> Result<Game, String> {
    let payload = match value {
        Value::Object(mut object)
            if object.contains_key("save_format_version")
                || object.contains_key("format")
                || (object.contains_key("protocol_version") && object.contains_key("game")) =>
        {
            if object.get("format").and_then(Value::as_str) != Some(SAVE_FORMAT_NAME) {
                return Err("that save has an unknown format".to_string());
            }
            version_field(&object, "protocol_version", PROTOCOL_VERSION)?;
            version_field(&object, "save_format_version", SAVE_FORMAT_VERSION)?;
            object
                .remove("game")
                .ok_or_else(|| "that save has no game payload".to_string())?
        }
        Value::Object(mut object) if object.len() == 1 && object.contains_key("game") => {
            // The native `/load` request has always accepted `{"game": ...}`.
            // Keep that wrapper usable when it is supplied as a file too.
            object
                .remove("game")
                .expect("the guard found the game field")
        }
        value => value,
    };

    serde_json::from_value(payload).map_err(|error| format!("that is not a save: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_envelope_round_trips_and_identifies_itself() {
        let game = Game::new(2, 20, 14, 7, 40, 0);
        let save = save_value(&game).expect("the game serializes");
        assert_eq!(save["format"], SAVE_FORMAT_NAME);
        assert_eq!(save["protocol"], PROTOCOL_NAME);
        assert_eq!(save["protocol_version"], PROTOCOL_VERSION);
        assert_eq!(save["save_format_version"], SAVE_FORMAT_VERSION);
        let restored = game_from_save(save).expect("the envelope loads");
        assert_eq!(restored.seed, game.seed);
        assert_eq!(restored.turn, game.turn);
    }

    #[test]
    fn a_legacy_raw_game_still_loads() {
        let game = Game::new(2, 20, 14, 8, 40, 0);
        let raw = serde_json::to_value(&game).expect("the game serializes");
        let restored = game_from_save(raw).expect("the legacy save loads");
        assert_eq!(restored.seed, game.seed);
    }

    #[test]
    fn future_save_and_request_versions_fail_before_deserialization() {
        let game = Game::new(2, 20, 14, 9, 40, 0);
        let mut save = save_value(&game).expect("the game serializes");
        save["save_format_version"] = json!(SAVE_FORMAT_VERSION + 1);
        let error = match game_from_save(save) {
            Ok(_) => panic!("a future save must be refused"),
            Err(error) => error,
        };
        assert!(error.contains("save_format_version"), "{error}");

        let request = json!({"protocol_version": PROTOCOL_VERSION + 1});
        let error = validate_request(&request).expect_err("a future request must be refused");
        assert!(error.contains("protocol_version"), "{error}");
    }

    #[test]
    fn version_response_preserves_existing_fields() {
        let response = version_response(json!({"turn": 7}));
        assert_eq!(response["turn"], 7);
        assert_eq!(response["protocol"], PROTOCOL_NAME);
        assert_eq!(response["protocol_version"], PROTOCOL_VERSION);
    }
}
