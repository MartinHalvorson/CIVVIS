//! The bounded newest-board selection (`latest_state_range_bounded`,
//! `latest_state_value_bounded`) against the full walks it replaces for a
//! request with no turn. Both must pick the same record on any log; the
//! generated logs mix the cases the walks' rules treat specially.

use super::*;

/// A small deterministic generator, so a failure names a reproducible seed.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn line(rng: &mut Lcg) -> String {
    let turn = rng.below(9);
    let frame = rng.below(3);
    let gold = rng.below(1000);
    match rng.below(20) {
        // Ordinary boards, the common case, often in reload order.
        0..=7 => format!(r#"{{"frame":{frame},"gold":{gold},"kind":"state","turn":{turn}}}"#),
        8 => format!(
            r#"{{"frame":{frame},"kind":"state","nested":{{"turn":{}}},"turn":{turn}}}"#,
            rng.below(20)
        ),
        9 => format!(
            r#"{{"frame":{frame},"kind":"state","turn":{turn},"turn":{}}}"#,
            rng.below(9)
        ),
        10 => match rng.below(5) {
            0 => r#"{"frame":1,"kind":"state","turn":null}"#.to_string(),
            1 => r#"{"frame":1,"kind":"state","turn":"7"}"#.to_string(),
            2 => r#"{"frame":1,"kind":"state","turn":7.0}"#.to_string(),
            3 => r#"{"frame":1,"kind":"state"}"#.to_string(),
            _ => r#"{"frame":1,"kind":"state","turn":4294967301}"#.to_string(),
        },
        // Lines that name "state" without being a board.
        11 => format!(
            r#"{{"kind":"turn","turn":{},"why":"state"}}"#,
            rng.below(12)
        ),
        12 => format!(r#"{{"kind":7,"note":"state","turn":{turn}}}"#),
        13 => format!(r#"{{"kind":"state","turn":{turn}"#),
        14 => format!(r#"{{"kind":"state","turn":{turn},"frame":"bad"}}"#),
        15 | 16 => format!(r#"{{"civ":"CIVILIZATION_{}","kind":"seat"}}"#, rng.below(4)),
        17 => r#"{"kind":"seat"}"#.to_string(),
        _ => format!(r#"{{"kind":"host_move","turn":{turn},"unit":{gold}}}"#),
    }
}

fn log(seed: u64) -> String {
    let mut rng = Lcg(seed);
    let count = 1 + rng.below(14) as usize;
    let mut text = String::new();
    for index in 0..count {
        text.push_str(&line(&mut rng));
        let last = index + 1 == count;
        match rng.below(6) {
            0 if last => {}
            1 => text.push_str("\r\n"),
            _ => text.push('\n'),
        }
    }
    text
}

fn assert_same(raw: &str, label: &str) {
    assert_eq!(
        latest_state_range_bounded(raw),
        latest_state_range_scan(raw, None),
        "typed selection differs on {label}:\n{raw}"
    );
    let bounded = latest_state_value_bounded(raw).unwrap_or_else(|| state_value_scan(raw, None));
    assert_eq!(
        bounded,
        state_value_scan(raw, None),
        "value selection differs on {label}:\n{raw}"
    );
}

#[test]
fn bounded_selection_matches_the_full_walks_on_generated_logs() {
    for seed in 0..4000 {
        assert_same(&log(seed), &format!("seed {seed}"));
    }
}

#[test]
fn bounded_selection_matches_on_the_named_edge_cases() {
    for (label, raw) in [
        ("empty", ""),
        ("no state", "{\"kind\":\"seat\",\"civ\":\"X\"}\n"),
        ("turn zero only", "{\"kind\":\"state\",\"turn\":0}\n{\"kind\":\"state\",\"turn\":0,\"frame\":1}\n"),
        ("reload back in time", "{\"kind\":\"state\",\"turn\":9}\n{\"kind\":\"state\",\"turn\":4}\n"),
        ("tie goes to the newer frame", "{\"kind\":\"state\",\"turn\":5,\"frame\":0}\n{\"kind\":\"state\",\"turn\":5,\"frame\":2}\n{\"kind\":\"state\",\"turn\":5,\"frame\":1}\n"),
        ("nested turn above the board", "{\"kind\":\"state\",\"turn\":3,\"deal\":{\"turn\":40}}\n{\"kind\":\"state\",\"turn\":6}\n"),
        ("turnless board after a real one", "{\"kind\":\"state\",\"turn\":6}\n{\"kind\":\"state\"}\n"),
        ("overflowing turn", "{\"kind\":\"state\",\"turn\":6}\n{\"kind\":\"state\",\"turn\":99999999999999999999}\n"),
        ("seat after the board", "{\"kind\":\"state\",\"turn\":6}\n{\"kind\":\"seat\",\"civ\":\"A\"}\n{\"kind\":\"seat\",\"civ\":\"B\"}\n"),
        ("unterminated last board", "{\"kind\":\"state\",\"turn\":2}\n{\"kind\":\"state\",\"turn\":3}"),
        ("crlf", "{\"kind\":\"state\",\"turn\":2}\r\n{\"kind\":\"state\",\"turn\":3,\"frame\":1}\r\n"),
    ] {
        assert_same(raw, label);
    }
}

#[test]
fn the_bounded_walk_picks_the_newest_board_of_a_live_shaped_log() {
    // The shape of every live log: one record per frame, turns never falling.
    let mut raw = String::new();
    for turn in 1..=300u32 {
        for frame in 0..3 {
            raw.push_str(&format!(
                "{{\"frame\":{frame},\"kind\":\"state\",\"turn\":{turn}}}\n{{\"kind\":\"host_move\",\"turn\":{turn}}}\n"
            ));
        }
    }
    let last = raw.rfind("{\"frame\":2").unwrap();
    assert_same(&raw, "monotonic live shape");
    let (range, stamp) = latest_state_range_bounded(&raw).unwrap();
    assert_eq!(stamp, (300, 2));
    assert_eq!(range.0, last);
    let value = latest_state_value_bounded(&raw).unwrap().unwrap();
    assert_eq!(value["turn"], 300);
    assert_eq!(value["frame"], 2);
}
