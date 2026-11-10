//! PROBE ONLY, never committed: experiment switches for the last-civ doctrine.
use std::collections::HashMap;
use std::sync::OnceLock;

static KNOBS: OnceLock<HashMap<String, f64>> = OnceLock::new();

fn parse(spec: &str) -> HashMap<String, f64> {
    spec.split(',')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((key.trim().to_string(), value.trim().parse().ok()?))
        })
        .collect()
}

/// Fix the switches for this process, before anything reads them.
pub fn set(spec: &str) {
    let _ = KNOBS.set(parse(spec));
}

/// `LCS="bodies=4,patience=8,bill=1,noabort=1"`; a missing name reads 0.
pub fn get(name: &str) -> f64 {
    *KNOBS
        .get_or_init(|| parse(&std::env::var("LCS").unwrap_or_default()))
        .get(name)
        .unwrap_or(&0.0)
}
